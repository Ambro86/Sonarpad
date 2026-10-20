//! Media conversion using only Sonarpad's dynamically loaded FFmpeg libraries.
use crate::ffmpeg_dyn::*;
use crate::ffmpeg_export::{ConvertAudioFormat, ConvertAudioSettings};
use crate::ffmpeg_source::{FfmpegApi, ffmpeg_api};
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub(crate) fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn sibling(path: &Path, extension: &str) -> PathBuf {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    path.with_file_name(format!(
        ".sonarpad-convert-{}-{}.{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        extension
    ))
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Conversion canceled.".to_string())
    } else {
        Ok(())
    }
}

/// Stage every output beside its destination; errors/cancellation never replace an existing file.
pub(crate) fn convert(
    input: &Path,
    output: &Path,
    settings: &ConvertAudioSettings,
    video: bool,
    image: Option<&Path>,
    cancel: Arc<AtomicBool>,
    progress: &mut dyn FnMut(u32),
) -> Result<(), String> {
    if same_file(input, output) || image.is_some_and(|image| same_file(image, output)) {
        return Err("Input and output must be different files".to_string());
    }
    let extension = output.extension().and_then(|v| v.to_str()).unwrap_or("mp4");
    let staged = sibling(output, extension);
    let video_file = sibling(output, extension);
    let audio_file = sibling(
        output,
        if settings.format == ConvertAudioFormat::Mp3 {
            "mp3"
        } else {
            "m4a"
        },
    );
    let mut last_progress = None;
    let mut report = |pct: u32| {
        let pct = pct.min(100);
        if last_progress.is_none_or(|last| pct > last) {
            last_progress = Some(pct);
            crate::log_debug(&format!("Convert media progress: {pct}%"));
            progress(pct);
        }
    };
    let result = (|| {
        check_cancel(&cancel)?;
        report(0);
        if !video {
            // Audio callbacks use hundredths of a percent (0..10000), unlike video/mux (0..100).
            crate::ffmpeg_export::convert_audio_file(
                input,
                &staged,
                settings,
                Some(cancel.clone()),
                Some(&mut |pct| report(pct.min(10000) * 99 / 10000)),
            )?;
        } else {
            let has_video = crate::ffmpeg_source::has_real_video_stream(input)?;
            let has_audio = !crate::ffmpeg_source::list_audio_streams(input)?.is_empty();
            let (visual_source, still_duration) = if has_video {
                (input, None)
            } else {
                let image = image.ok_or_else(|| "Select an image for the video".to_string())?;
                let duration = crate::ffmpeg_export::media_duration_seconds(input)
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .ok_or_else(|| "Cannot determine audio duration".to_string())?;
                (image, Some(duration))
            };
            // Each pass contributes its actual processed fraction, not elapsed wall time.
            let video_share = if has_audio { 33 } else { 99 };
            encode_video(
                visual_source,
                &video_file,
                still_duration,
                &cancel,
                &mut |pct| report(pct.min(100) * video_share / 100),
            )?;
            if has_audio {
                crate::ffmpeg_export::convert_audio_file(
                    input,
                    &audio_file,
                    settings,
                    Some(cancel.clone()),
                    Some(&mut |pct| report(33 + pct.min(10000) * 33 / 10000)),
                )?;
                crate::ffmpeg_export::remux_media_file_to_mp4_with_external_audio_stream(
                    &video_file,
                    &audio_file,
                    &staged,
                    Some(cancel.clone()),
                    Some(&mut |pct| report(66 + pct.min(100) * 33 / 100)),
                )
                .map_err(|error| {
                    if cancel.load(Ordering::Relaxed) {
                        "Conversion canceled.".to_string()
                    } else {
                        error
                    }
                })?;
            } else {
                std::fs::rename(&video_file, &staged).map_err(|e| e.to_string())?;
            }
        }
        check_cancel(&cancel)?;
        std::fs::rename(&staged, output).map_err(|e| e.to_string())?;
        report(100);
        Ok(())
    })();
    for file in [&staged, &video_file, &audio_file] {
        if let Err(error) = std::fs::remove_file(file)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            crate::log_debug(&format!("Media conversion cleanup: {error}"));
        }
    }
    result
}

fn ffcheck(api: &FfmpegApi, result: i32) -> Result<(), String> {
    if result < 0 {
        Err(crate::ffmpeg_source::ffmpeg_err(api, result))
    } else {
        Ok(())
    }
}

// All allocations are released after the closure, including on early error or cancellation.
fn encode_video(
    input: &Path,
    output: &Path,
    still_duration: Option<f64>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u32),
) -> Result<(), String> {
    encode_video_sized(input, output, still_duration, None, cancel, progress)
}

pub(crate) fn encode_video_sized(
    input: &Path,
    output: &Path,
    still_duration: Option<f64>,
    dimensions: Option<(i32, i32)>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u32),
) -> Result<(), String> {
    let duration = still_duration
        .or_else(|| crate::ffmpeg_export::media_duration_seconds(input))
        .filter(|duration| duration.is_finite() && *duration > 0.0);
    let api = ffmpeg_api()?;
    let input_name = CString::new(input.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    let output_name =
        CString::new(output.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    let mut source = ptr::null_mut();
    let mut destination: *mut AVFormatContext = ptr::null_mut();
    let mut decoder = ptr::null_mut();
    let mut encoder = ptr::null_mut();
    let mut decoded = ptr::null_mut();
    let mut scaled = ptr::null_mut();
    let mut packet = ptr::null_mut();
    let mut encoded = ptr::null_mut();
    let mut scaler: *mut libc::c_void = ptr::null_mut();
    unsafe {
        let result = (|| -> Result<(), String> {
            ffcheck(
                api,
                (api.avformat_open_input)(
                    &mut source,
                    input_name.as_ptr(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ),
            )?;
            ffcheck(
                api,
                (api.avformat_find_stream_info)(source, ptr::null_mut()),
            )?;
            let index = (api.av_find_best_stream)(
                source,
                AVMediaType_AVMEDIA_TYPE_VIDEO,
                -1,
                -1,
                ptr::null_mut(),
                0,
            );
            ffcheck(api, index)?;
            let input_stream = *(*source).streams.add(index as usize);
            let decode_codec = (api.avcodec_find_decoder)((*(*input_stream).codecpar).codec_id);
            let encode_codec = (api.avcodec_find_encoder)(AVCodecID_AV_CODEC_ID_MPEG4);
            if decode_codec.is_null() || encode_codec.is_null() {
                return Err("Video codec unavailable".to_string());
            }
            decoder = (api.avcodec_alloc_context3)(decode_codec);
            encoder = (api.avcodec_alloc_context3)(encode_codec);
            if decoder.is_null() || encoder.is_null() {
                return Err("Video context allocation failed".to_string());
            }
            ffcheck(
                api,
                (api.avcodec_parameters_to_context)(decoder, (*input_stream).codecpar),
            )?;
            ffcheck(
                api,
                (api.avcodec_open2)(decoder, decode_codec, ptr::null_mut()),
            )?;
            ffcheck(
                api,
                (api.avformat_alloc_output_context2)(
                    &mut destination,
                    ptr::null_mut(),
                    ptr::null(),
                    output_name.as_ptr(),
                ),
            )?;
            if destination.is_null() {
                return Err("Video output allocation failed".to_string());
            }
            let stream = (api.avformat_new_stream)(destination, ptr::null());
            if stream.is_null() {
                return Err("Video stream allocation failed".to_string());
            }
            (*encoder).width = (dimensions.map_or((*decoder).width, |d| d.0) + 1) & !1;
            (*encoder).height = (dimensions.map_or((*decoder).height, |d| d.1) + 1) & !1;
            (*encoder).pix_fmt = AVPixelFormat_AV_PIX_FMT_YUV420P;
            (*encoder).time_base = AVRational { num: 1, den: 30000 };
            (*encoder).framerate = if still_duration.is_some() {
                AVRational { num: 25, den: 1 }
            } else {
                (*input_stream).avg_frame_rate
            };
            if output
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("avi"))
            {
                let rate = (*encoder).framerate;
                (*encoder).time_base = if rate.num > 0 && rate.num <= 65535 && rate.den > 0 {
                    AVRational {
                        num: rate.den,
                        den: rate.num,
                    }
                } else {
                    AVRational { num: 1, den: 25 }
                };
            }
            let ticks_per_second =
                (*encoder).time_base.den as f64 / (*encoder).time_base.num as f64;
            (*encoder).sample_aspect_ratio = if dimensions.is_some() {
                AVRational { num: 1, den: 1 }
            } else {
                (*decoder).sample_aspect_ratio
            };
            (*encoder).gop_size = 12;
            (*encoder).max_b_frames = 0;
            (*encoder).flags |= 1 << 1; // AV_CODEC_FLAG_QSCALE, matching Mobile q=4.
            (*encoder).global_quality = 4 * 118;
            if (*(*destination).oformat).flags & 0x40 != 0 {
                (*encoder).flags |= 1 << 22;
            }
            ffcheck(
                api,
                (api.avcodec_open2)(encoder, encode_codec, ptr::null_mut()),
            )?;
            ffcheck(
                api,
                (api.avcodec_parameters_from_context)((*stream).codecpar, encoder),
            )?;
            (*stream).time_base = (*encoder).time_base;
            (*stream).avg_frame_rate = (*encoder).framerate;
            (*stream).sample_aspect_ratio = (*encoder).sample_aspect_ratio;
            ffcheck(
                api,
                (api.avio_open)(&mut (*destination).pb, output_name.as_ptr(), 2),
            )?;
            ffcheck(
                api,
                (api.avformat_write_header)(destination, ptr::null_mut()),
            )?;
            decoded = (api.av_frame_alloc)();
            scaled = (api.av_frame_alloc)();
            packet = (api.av_packet_alloc)();
            encoded = (api.av_packet_alloc)();
            if decoded.is_null() || scaled.is_null() || packet.is_null() || encoded.is_null() {
                return Err("Video buffer allocation failed".to_string());
            }
            (*scaled).width = (*encoder).width;
            (*scaled).height = (*encoder).height;
            (*scaled).format = (*encoder).pix_fmt;
            ffcheck(api, (api.av_frame_get_buffer)(scaled, 32))?;
            let drain_encoder = || -> Result<(), String> {
                loop {
                    let ret = (api.avcodec_receive_packet)(encoder, encoded);
                    if ret == -11 || ret == -541478725 {
                        return Ok(());
                    }
                    ffcheck(api, ret)?;
                    (*encoded).stream_index = (*stream).index;
                    (api.av_packet_rescale_ts)(encoded, (*encoder).time_base, (*stream).time_base);
                    let ret = (api.av_interleaved_write_frame)(destination, encoded);
                    (api.av_packet_unref)(encoded);
                    ffcheck(api, ret)?;
                }
            };
            let mut origin = None;
            let mut last_pts = -1;
            let mut count = 0_u64;
            let mut finished_still = false;
            let mut scaled_source = None;
            let mut receive_frames = || -> Result<(), String> {
                if finished_still {
                    return Ok(());
                }
                loop {
                    check_cancel(cancel)?;
                    let ret = (api.avcodec_receive_frame)(decoder, decoded);
                    if ret == -11 || ret == -541478725 {
                        return Ok(());
                    }
                    ffcheck(api, ret)?;
                    let source_dimensions =
                        ((*decoded).width, (*decoded).height, (*decoded).format);
                    let (scaled_width, scaled_height) = if dimensions.is_some() {
                        let sar = (*decoder).sample_aspect_ratio;
                        let pixel_ratio = if sar.num > 0 && sar.den > 0 {
                            sar.num as f64 / sar.den as f64
                        } else {
                            1.0
                        };
                        let width = (*decoded).width as f64 * pixel_ratio;
                        let height = (*decoded).height as f64;
                        let ratio = ((*encoder).width as f64 / width)
                            .min((*encoder).height as f64 / height);
                        (
                            ((width * ratio).round() as i32 & !1).clamp(2, (*encoder).width),
                            ((height * ratio).round() as i32 & !1).clamp(2, (*encoder).height),
                        )
                    } else {
                        ((*encoder).width, (*encoder).height)
                    };
                    if scaled_source != Some(source_dimensions) {
                        if !scaler.is_null() {
                            (api.sws_free_context)(scaler);
                        }
                        scaler = (api.sws_get_context)(
                            (*decoded).width,
                            (*decoded).height,
                            (*decoded).format,
                            scaled_width,
                            scaled_height,
                            (*encoder).pix_fmt,
                            2,
                            ptr::null_mut(),
                            ptr::null_mut(),
                            ptr::null(),
                        );
                        if scaler.is_null() {
                            return Err("Video color conversion failed".to_string());
                        }
                        scaled_source = Some(source_dimensions);
                    }
                    let repeats = still_duration
                        .map(|d| (d * 25.0).ceil() as u64)
                        .unwrap_or(1);
                    let pts = if (*decoded).best_effort_timestamp != i64::MIN {
                        let timestamp = (*decoded).best_effort_timestamp;
                        let first = *origin.get_or_insert(timestamp);
                        let tb = (*input_stream).time_base;
                        (((timestamp - first) as f64 * tb.num as f64 / tb.den as f64)
                            * ticks_per_second)
                            .round() as i64
                    } else {
                        let rate = (*input_stream).avg_frame_rate;
                        let step = if rate.num > 0 && rate.den > 0 {
                            ticks_per_second * rate.den as f64 / rate.num as f64
                        } else {
                            ticks_per_second / 25.0
                        };
                        (count as f64 * step).round() as i64
                    };
                    for frame_index in 0..repeats {
                        check_cancel(cancel)?;
                        ffcheck(api, (api.av_frame_make_writable)(scaled))?;
                        let mut target_data = (*scaled).data;
                        if dimensions.is_some() {
                            let left = (((*encoder).width - scaled_width) / 2) & !1;
                            let top = (((*encoder).height - scaled_height) / 2) & !1;
                            for (plane, target) in target_data.iter_mut().enumerate().take(3) {
                                let divisor = if plane == 0 { 1 } else { 2 };
                                let stride = (*scaled).linesize[plane];
                                for row in 0..(*encoder).height / divisor {
                                    ptr::write_bytes(
                                        (*scaled).data[plane].offset((row * stride) as isize),
                                        if plane == 0 { 16 } else { 128 },
                                        ((*encoder).width / divisor) as usize,
                                    );
                                }
                                *target = (*scaled).data[plane]
                                    .offset(((top / divisor) * stride + left / divisor) as isize);
                            }
                        }

                        ffcheck(
                            api,
                            (api.sws_scale)(
                                scaler,
                                (*decoded).data.as_ptr().cast(),
                                (*decoded).linesize.as_ptr(),
                                0,
                                (*decoded).height,
                                target_data.as_ptr(),
                                (*scaled).linesize.as_ptr(),
                            ),
                        )?;
                        (*scaled).pts = if still_duration.is_some() {
                            (frame_index as f64 * ticks_per_second / 25.0).round() as i64
                        } else {
                            pts.max(last_pts + 1)
                        };
                        (*scaled).quality = (*encoder).global_quality;
                        last_pts = (*scaled).pts;
                        ffcheck(api, (api.avcodec_send_frame)(encoder, scaled))?;
                        drain_encoder()?;
                        count += 1;
                        if let Some(duration) = duration {
                            let processed = if still_duration.is_some() {
                                count as f64 / 25.0
                            } else {
                                last_pts as f64 / ticks_per_second
                            };
                            progress(((processed / duration * 100.0) as u32).min(99));
                        }
                    }
                    (api.av_frame_unref)(decoded);
                    if still_duration.is_some() {
                        finished_still = true;
                        return Ok(());
                    }
                }
            };
            loop {
                check_cancel(cancel)?;
                let ret = (api.av_read_frame)(source, packet);
                if ret == -541478725 {
                    break;
                }
                ffcheck(api, ret)?;
                if (*packet).stream_index == index {
                    let ret = (api.avcodec_send_packet)(decoder, packet);
                    (api.av_packet_unref)(packet);
                    ffcheck(api, ret)?;
                    receive_frames()?;
                    if still_duration.is_some() {
                        break;
                    }
                } else {
                    (api.av_packet_unref)(packet);
                }
            }
            ffcheck(api, (api.avcodec_send_packet)(decoder, ptr::null()))?;
            receive_frames()?;
            if count == 0 || (still_duration.is_some() && !finished_still) {
                return Err("No video frames decoded".to_string());
            }
            ffcheck(api, (api.avcodec_send_frame)(encoder, ptr::null()))?;
            drain_encoder()?;
            ffcheck(api, (api.av_write_trailer)(destination))?;
            progress(100);
            Ok(())
        })();
        if !scaler.is_null() {
            (api.sws_free_context)(scaler);
        }
        (api.av_packet_free)(&mut packet);
        (api.av_packet_free)(&mut encoded);
        (api.av_frame_free)(&mut decoded);
        (api.av_frame_free)(&mut scaled);
        (api.avcodec_free_context)(&mut decoder);
        (api.avcodec_free_context)(&mut encoder);
        (api.avformat_close_input)(&mut source);
        if !destination.is_null() {
            if !(*destination).pb.is_null() {
                (api.avio_closep)(&mut (*destination).pb);
            }
            (api.avformat_free_context)(destination);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffmpeg_export::{ConvertAudioQuality, media_duration_seconds};

    #[test]
    fn progress_tracks_audio_video_and_cancellation_without_early_completion() -> Result<(), String>
    {
        let (dir, audio, image) = fixtures();
        for (video, extension) in [(false, "mp3"), (true, "mp4")] {
            let output = dir.join(format!("progress.{extension}"));
            let settings = ConvertAudioSettings {
                format: if video {
                    ConvertAudioFormat::Aac
                } else {
                    ConvertAudioFormat::Mp3
                },
                quality: ConvertAudioQuality::BitrateKbps(128),
            };
            let mut values = Vec::new();
            super::convert(
                &audio,
                &output,
                &settings,
                video,
                Some(&image),
                Arc::new(AtomicBool::new(false)),
                &mut |pct| {
                    if pct == 100 {
                        assert!(output.is_file());
                    }
                    values.push(pct);
                },
            )?;
            assert_eq!(values.first(), Some(&0));
            assert_eq!(values.last(), Some(&100));
            assert!(values.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(values.iter().any(|&pct| pct > 0 && pct < 100));
            if video {
                assert!(values.iter().any(|&pct| pct > 0 && pct < 33));
                assert!(values.iter().any(|&pct| pct > 33 && pct < 66));
                assert!(values.iter().any(|&pct| pct > 66 && pct < 100));
            }
            let saved = std::fs::read(&output).map_err(|error| error.to_string())?;
            let cancel = Arc::new(AtomicBool::new(false));
            let mut cancelled_progress = Vec::new();
            let result = super::convert(
                &audio,
                &output,
                &settings,
                video,
                Some(&image),
                cancel.clone(),
                &mut |pct| {
                    cancelled_progress.push(pct);
                    if pct > 0 {
                        cancel.store(true, Ordering::Relaxed);
                    }
                },
            );
            assert_eq!(result, Err("Conversion canceled.".to_string()));
            assert!(!cancelled_progress.contains(&100));
            assert_eq!(
                std::fs::read(&output).map_err(|error| error.to_string())?,
                saved
            );
        }
        std::fs::remove_dir_all(dir).map_err(|error| error.to_string())
    }

    fn convert(
        input: &Path,
        output: &Path,
        settings: &ConvertAudioSettings,
        video: bool,
        image: Option<&Path>,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let mut values = Vec::new();
        let result = super::convert(input, output, settings, video, image, cancel, &mut |pct| {
            values.push(pct)
        });
        assert!(values.windows(2).all(|pair| pair[0] < pair[1]));
        if result.is_ok() {
            assert_eq!(values.first(), Some(&0));
            assert_eq!(values.last(), Some(&100));
            assert!(values.iter().any(|&pct| pct > 0 && pct < 100));
        } else {
            assert!(!values.contains(&100));
        }
        result
    }

    /// A short packet-copy fixture, produced with the same bundled DLLs (no CLI).
    fn excerpt(input: &Path, output: &Path, seconds: f64) -> Result<(), String> {
        let api = ffmpeg_api()?;
        let input = CString::new(input.to_string_lossy().as_bytes()).unwrap();
        let output = CString::new(output.to_string_lossy().as_bytes()).unwrap();
        let mut source = ptr::null_mut();
        let mut destination: *mut AVFormatContext = ptr::null_mut();
        let mut packet = ptr::null_mut();
        unsafe {
            let result = (|| {
                ffcheck(
                    api,
                    (api.avformat_open_input)(
                        &mut source,
                        input.as_ptr(),
                        ptr::null_mut(),
                        ptr::null_mut(),
                    ),
                )?;
                ffcheck(
                    api,
                    (api.avformat_find_stream_info)(source, ptr::null_mut()),
                )?;
                ffcheck(
                    api,
                    (api.avformat_alloc_output_context2)(
                        &mut destination,
                        ptr::null_mut(),
                        ptr::null(),
                        output.as_ptr(),
                    ),
                )?;
                if destination.is_null() {
                    return Err("Excerpt allocation failed".to_string());
                }
                let mut mapping = vec![None; (*source).nb_streams as usize];
                for (index, target) in mapping.iter_mut().enumerate() {
                    let stream = *(*source).streams.add(index);
                    let tb = (*stream).time_base;
                    eprintln!(
                        "INPUT stream {index}: start={} duration={} timebase={}/{}",
                        (*stream).start_time,
                        (*stream).duration,
                        tb.num,
                        tb.den
                    );
                    let kind = (*(*stream).codecpar).codec_type;
                    if kind != AVMediaType_AVMEDIA_TYPE_AUDIO
                        && kind != AVMediaType_AVMEDIA_TYPE_VIDEO
                    {
                        continue;
                    }
                    let out = (api.avformat_new_stream)(destination, ptr::null());
                    if out.is_null() {
                        return Err("Excerpt stream allocation failed".to_string());
                    }
                    ffcheck(
                        api,
                        (api.avcodec_parameters_copy)((*out).codecpar, (*stream).codecpar),
                    )?;
                    (*(*out).codecpar).codec_tag = 0;
                    (*out).time_base = (*stream).time_base;
                    *target = Some(out);
                }
                ffcheck(
                    api,
                    (api.avio_open)(&mut (*destination).pb, output.as_ptr(), 2),
                )?;
                ffcheck(
                    api,
                    (api.avformat_write_header)(destination, ptr::null_mut()),
                )?;
                packet = (api.av_packet_alloc)();
                if packet.is_null() {
                    return Err("Excerpt packet allocation failed".to_string());
                }
                let mut origin = None;
                loop {
                    let ret = (api.av_read_frame)(source, packet);
                    if ret == -541478725 {
                        break;
                    }
                    ffcheck(api, ret)?;
                    let index = (*packet).stream_index as usize;
                    let Some(out) = mapping.get(index).copied().flatten() else {
                        (api.av_packet_unref)(packet);
                        continue;
                    };
                    let stream = *(*source).streams.add(index);
                    let tb = (*stream).time_base;
                    let pts = if (*packet).pts == i64::MIN {
                        (*packet).dts
                    } else {
                        (*packet).pts
                    };
                    if pts != i64::MIN {
                        let time = pts as f64 * tb.num as f64 / tb.den as f64;
                        let start = *origin.get_or_insert(time);
                        if time - start >= seconds {
                            break;
                        }
                    }
                    (*packet).stream_index = (*out).index;
                    (api.av_packet_rescale_ts)(packet, tb, (*out).time_base);
                    let ret = (api.av_interleaved_write_frame)(destination, packet);
                    (api.av_packet_unref)(packet);
                    ffcheck(api, ret)?;
                }
                ffcheck(api, (api.av_write_trailer)(destination))
            })();
            (api.av_packet_free)(&mut packet);
            (api.avformat_close_input)(&mut source);
            if !destination.is_null() {
                if !(*destination).pb.is_null() {
                    (api.avio_closep)(&mut (*destination).pb);
                }
                (api.avformat_free_context)(destination);
            }
            result
        }
    }

    #[test]
    #[ignore = "requires user-selected local media; set SONARPAD_MEDIA_TEST_INPUTS and SONARPAD_MEDIA_TEST_OUTPUT"]
    fn real_media_thirty_second_conversion_matrix() {
        let inputs: Vec<PathBuf> = serde_json::from_str(
            &std::env::var("SONARPAD_MEDIA_TEST_INPUTS").expect("input paths JSON"),
        )
        .unwrap();
        let output = PathBuf::from(
            std::env::var("SONARPAD_MEDIA_TEST_OUTPUT").expect("isolated output directory"),
        );
        std::fs::create_dir_all(&output).unwrap();
        let (fixture_dir, _, cover) = fixtures();
        let cancel = Arc::new(AtomicBool::new(false));
        for (index, input) in inputs.iter().enumerate() {
            let metadata = std::fs::metadata(input).unwrap();
            let clip = output.join(format!("source-{index}.mkv"));
            excerpt(input, &clip, 30.0).unwrap();
            let duration = media_duration_seconds(&clip).unwrap();
            assert!(
                (28.0..32.0).contains(&duration),
                "excerpt duration {duration}"
            );
            for (extension, format, quality) in [
                (
                    "mp3",
                    ConvertAudioFormat::Mp3,
                    ConvertAudioQuality::BitrateKbps(192),
                ),
                (
                    "m4a",
                    ConvertAudioFormat::Aac,
                    ConvertAudioQuality::BitrateKbps(192),
                ),
                (
                    "m4b",
                    ConvertAudioFormat::Aac,
                    ConvertAudioQuality::BitrateKbps(192),
                ),
                (
                    "opus",
                    ConvertAudioFormat::Opus,
                    ConvertAudioQuality::BitrateKbps(128),
                ),
                (
                    "ogg",
                    ConvertAudioFormat::Ogg,
                    ConvertAudioQuality::OggQuality(5),
                ),
                (
                    "flac",
                    ConvertAudioFormat::Flac,
                    ConvertAudioQuality::FlacCompression(5),
                ),
                ("wav", ConvertAudioFormat::Wav, ConvertAudioQuality::None),
                ("aiff", ConvertAudioFormat::Aiff, ConvertAudioQuality::None),
                (
                    "wav24.wav",
                    ConvertAudioFormat::Wav,
                    ConvertAudioQuality::WavPcm24,
                ),
                (
                    "wav32.wav",
                    ConvertAudioFormat::Wav,
                    ConvertAudioQuality::WavPcm32,
                ),
                (
                    "wavfloat.wav",
                    ConvertAudioFormat::Wav,
                    ConvertAudioQuality::WavFloat32,
                ),
            ] {
                let target = output.join(format!("source-{index}.{extension}"));
                convert(
                    &clip,
                    &target,
                    &ConvertAudioSettings { format, quality },
                    false,
                    None,
                    cancel.clone(),
                )
                .unwrap_or_else(|e| panic!("source {index} -> {extension}: {e}"));
                let decoded =
                    crate::ffmpeg_source::FfmpegSource::try_new(&target, 0, None, None).unwrap();
                use rodio::Source;
                let sample_rate = decoded.sample_rate();
                let channels = decoded.channels();
                let (samples, energy) = decoded.fold((0_u64, 0_f64), |(count, energy), value| {
                    (count + 1, energy + f64::from(value).powi(2))
                });
                let decoded_duration = samples as f64 / sample_rate as f64 / channels as f64;
                assert!(
                    (decoded_duration - duration).abs() < 0.5,
                    "source {index} -> {extension}: {decoded_duration} vs {duration}"
                );
                assert!(energy > 0.01, "source {index} -> {extension}: empty audio");
                eprintln!(
                    "PASS source {index} -> {extension}: {decoded_duration:.3}s, decoded {samples} samples"
                );
            }
            let mut video_source = clip.clone();
            for extension in ["mp4", "avi", "mov", "mp4"] {
                let target = output.join(format!(
                    "source-{index}-{}.{}",
                    if video_source.extension().is_some_and(|e| e == "mov") {
                        "verified"
                    } else {
                        "video"
                    },
                    extension
                ));
                let settings = ConvertAudioSettings {
                    format: if extension == "avi" {
                        ConvertAudioFormat::Mp3
                    } else {
                        ConvertAudioFormat::Aac
                    },
                    quality: ConvertAudioQuality::BitrateKbps(192),
                };
                convert(
                    &video_source,
                    &target,
                    &settings,
                    true,
                    Some(&cover),
                    cancel.clone(),
                )
                .unwrap_or_else(|e| panic!("source {index} -> {extension}: {e}"));
                assert!(crate::ffmpeg_source::has_real_video_stream(&target).unwrap());
                assert!((media_duration_seconds(&target).unwrap() - duration).abs() < 0.5);
                assert_eq!(
                    crate::ffmpeg_source::list_audio_streams(&target)
                        .unwrap()
                        .len(),
                    1
                );
                eprintln!(
                    "PASS source {index} -> {extension}: {:.3}s, video+audio",
                    media_duration_seconds(&target).unwrap()
                );
                video_source = target;
            }
            assert_eq!(metadata.len(), std::fs::metadata(input).unwrap().len());
            assert_eq!(
                metadata.modified().unwrap(),
                std::fs::metadata(input).unwrap().modified().unwrap()
            );
        }
        std::fs::remove_dir_all(fixture_dir).unwrap();
    }

    fn fixtures() -> (PathBuf, PathBuf, PathBuf) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "sonarpad-media-test-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        let audio = dir.join("audio.wav");
        let mut writer = hound::WavWriter::create(
            &audio,
            hound::WavSpec {
                channels: 1,
                sample_rate: 48000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for i in 0..24000 {
            writer
                .write_sample(
                    ((i as f32 * 440.0 * std::f32::consts::TAU / 48000.0).sin() * 16000.0) as i16,
                )
                .unwrap();
        }
        writer.finalize().unwrap();
        // Odd-sized BMP exercises the padding needed for YUV420P video.
        let image = dir.join("cover.bmp");
        let width = 31_u32;
        let height = 23_u32;
        let row = (width * 3 + 3) & !3;
        let size = 54 + row * height;
        let mut bmp = vec![0_u8; size as usize];
        bmp[..2].copy_from_slice(b"BM");
        bmp[2..6].copy_from_slice(&size.to_le_bytes());
        bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&width.to_le_bytes());
        bmp[22..26].copy_from_slice(&height.to_le_bytes());
        bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24_u16.to_le_bytes());
        bmp[54..].fill(120);
        std::fs::write(&image, bmp).unwrap();
        (dir, audio, image)
    }

    #[test]
    fn native_video_formats_image_video_and_audio_extraction() {
        let (dir, audio, image) = fixtures();
        let cancel = Arc::new(AtomicBool::new(false));
        for extension in ["mp4", "avi", "mov"] {
            let settings = ConvertAudioSettings {
                format: if extension == "avi" {
                    ConvertAudioFormat::Mp3
                } else {
                    ConvertAudioFormat::Aac
                },
                quality: ConvertAudioQuality::BitrateKbps(128),
            };
            let movie = dir.join(format!("image.{extension}"));
            convert(
                &audio,
                &movie,
                &settings,
                true,
                Some(&image),
                cancel.clone(),
            )
            .unwrap();
            assert!(crate::ffmpeg_source::has_real_video_stream(&movie).unwrap());
            assert_eq!(
                crate::ffmpeg_source::list_audio_streams(&movie)
                    .unwrap()
                    .len(),
                1
            );
            assert!((media_duration_seconds(&movie).unwrap() - 0.5).abs() < 0.15);
            let recoded = dir.join(format!("video.{extension}"));
            convert(&movie, &recoded, &settings, true, None, cancel.clone()).unwrap();
            assert!(crate::ffmpeg_source::has_real_video_stream(&recoded).unwrap());
            assert!((media_duration_seconds(&recoded).unwrap() - 0.5).abs() < 0.2);
            let extracted = dir.join(format!("extracted-{extension}.wav"));
            convert(
                &recoded,
                &extracted,
                &ConvertAudioSettings {
                    format: ConvertAudioFormat::Wav,
                    quality: ConvertAudioQuality::None,
                },
                false,
                None,
                cancel.clone(),
            )
            .unwrap();
            let mut reader = hound::WavReader::open(&extracted).unwrap();
            assert!((reader.duration() as f64 / 48000.0 - 0.5).abs() < 0.2);
            assert!(
                reader
                    .samples::<i16>()
                    .any(|s| s.unwrap().unsigned_abs() > 1000)
            );
        }
        let silent = dir.join("silent.mp4");
        encode_video(&image, &silent, Some(0.5), &cancel, &mut |_| {}).unwrap();
        let silent_mov = dir.join("silent.mov");
        convert(
            &silent,
            &silent_mov,
            &ConvertAudioSettings {
                format: ConvertAudioFormat::Aac,
                quality: ConvertAudioQuality::BitrateKbps(128),
            },
            true,
            None,
            cancel,
        )
        .unwrap();
        assert!(
            crate::ffmpeg_source::list_audio_streams(&silent_mov)
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn native_wav_depths_and_safe_failure() {
        let (dir, audio, image) = fixtures();
        let ogg = dir.join("mono-input.ogg");
        convert(
            &audio,
            &ogg,
            &ConvertAudioSettings {
                format: ConvertAudioFormat::Ogg,
                quality: ConvertAudioQuality::OggQuality(5),
            },
            false,
            None,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        let decoded = crate::ffmpeg_source::FfmpegSource::try_new(&ogg, 0, None, None).unwrap();
        use rodio::Source;
        let rate = decoded.sample_rate();
        let channels = decoded.channels();
        let samples = decoded.count();
        assert!((samples as f64 / f64::from(rate) / f64::from(channels) - 0.5).abs() < 0.05);
        for (index, quality, bits, sample_format) in [
            (0, ConvertAudioQuality::None, 16, hound::SampleFormat::Int),
            (
                1,
                ConvertAudioQuality::WavPcm24,
                24,
                hound::SampleFormat::Int,
            ),
            (
                2,
                ConvertAudioQuality::WavPcm32,
                32,
                hound::SampleFormat::Int,
            ),
            (
                3,
                ConvertAudioQuality::WavFloat32,
                32,
                hound::SampleFormat::Float,
            ),
        ] {
            let output = dir.join(format!("depth{index}.wav"));
            let settings = ConvertAudioSettings {
                format: ConvertAudioFormat::Wav,
                quality,
            };
            convert(
                &audio,
                &output,
                &settings,
                false,
                None,
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
            let reader = hound::WavReader::open(&output).unwrap();
            assert_eq!(reader.spec().bits_per_sample, bits);
            assert_eq!(reader.spec().sample_format, sample_format);
            assert_eq!(reader.duration(), 24000);
        }
        let output = dir.join("existing.mp4");
        std::fs::write(&output, b"original file").unwrap();
        let settings = ConvertAudioSettings {
            format: ConvertAudioFormat::Aac,
            quality: ConvertAudioQuality::BitrateKbps(128),
        };
        assert_eq!(
            convert(
                &audio,
                &output,
                &settings,
                true,
                Some(&image),
                Arc::new(AtomicBool::new(true))
            )
            .unwrap_err(),
            "Conversion canceled."
        );
        assert_eq!(std::fs::read(&output).unwrap(), b"original file");
        assert!(
            convert(
                &audio,
                &output,
                &settings,
                true,
                None,
                Arc::new(AtomicBool::new(false))
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&output).unwrap(), b"original file");
        let original = std::fs::read(&audio).unwrap();
        assert!(
            convert(
                &audio,
                &audio,
                &settings,
                false,
                None,
                Arc::new(AtomicBool::new(false))
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&audio).unwrap(), original);
        assert!(std::fs::read_dir(&dir).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".sonarpad-convert-")
        }));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
