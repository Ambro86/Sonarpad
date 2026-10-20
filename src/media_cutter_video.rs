use super::check_cancel;
use crate::ffmpeg_dyn::*;
use crate::ffmpeg_source::{FfmpegApi, ffmpeg_api};
use crate::media_cutter_model::{MediaPart, kept_duration, output_time};
use std::{ffi::CString, path::Path, ptr, sync::atomic::AtomicBool};
// Both frames are allocated YUV420P images. Respect plane stride, including padding.
unsafe fn rotate_yuv420(src: *const AVFrame, dst: *mut AVFrame, rotation: u16) {
    unsafe {
        for plane in 0..3 {
            let w = if plane == 0 {
                (*src).width
            } else {
                (*src).width / 2
            } as usize;
            let h = if plane == 0 {
                (*src).height
            } else {
                (*src).height / 2
            } as usize;
            for y in 0..h {
                for x in 0..w {
                    let (dx, dy) = match rotation {
                        90 => (h - 1 - y, x),
                        180 => (w - 1 - x, h - 1 - y),
                        270 => (y, w - 1 - x),
                        _ => (x, y),
                    };
                    *(*dst).data[plane]
                        .offset(dy as isize * (*dst).linesize[plane] as isize + dx as isize) =
                        *(*src).data[plane]
                            .offset(y as isize * (*src).linesize[plane] as isize + x as isize);
                }
            }
        }
    }
}
fn ffcheck(api: &FfmpegApi, result: i32) -> Result<(), String> {
    if result < 0 {
        Err(crate::ffmpeg_source::ffmpeg_err(api, result))
    } else {
        Ok(())
    }
}

// All allocations are released after the closure, including on early error or cancellation.
pub(super) fn encode_video(
    input: &Path,
    output: &Path,
    parts: &[MediaPart],
    rotation: u16,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u32),
) -> Result<(), String> {
    encode_video_sized(input, output, parts, rotation, None, cancel, progress)
}

pub(super) fn encode_video_sized(
    input: &Path,
    output: &Path,
    parts: &[MediaPart],
    rotation: u16,
    dimensions: Option<(i32, i32)>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u32),
) -> Result<(), String> {
    let duration = Some(kept_duration(parts));
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
    let mut rotated: *mut AVFrame = ptr::null_mut();
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
            let source_width = (*encoder).width;
            let source_height = (*encoder).height;
            if rotation == 90 || rotation == 270 {
                (*encoder).width = source_height;
                (*encoder).height = source_width;
            }
            (*encoder).pix_fmt = AVPixelFormat_AV_PIX_FMT_YUV420P;
            (*encoder).time_base = AVRational { num: 1, den: 30000 };
            (*encoder).framerate = (*input_stream).avg_frame_rate;
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
            (*scaled).width = source_width;
            (*scaled).height = source_height;
            (*scaled).format = (*encoder).pix_fmt;
            ffcheck(api, (api.av_frame_get_buffer)(scaled, 32))?;
            rotated = (api.av_frame_alloc)();
            if rotated.is_null() {
                return Err("Video rotation allocation failed".into());
            }
            (*rotated).width = (*encoder).width;
            (*rotated).height = (*encoder).height;
            (*rotated).format = (*encoder).pix_fmt;
            ffcheck(api, (api.av_frame_get_buffer)(rotated, 32))?;
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
            let media_origin = if (*source).start_time == i64::MIN {
                0.0
            } else {
                (*source).start_time as f64 / 1_000_000.0
            };
            let mut last_pts = -1;
            let mut count = 0_u64;
            let finished = std::cell::Cell::new(false);
            let mut decoded_count = 0_u64;
            let last_end = parts.last().map_or(0.0, |p| p.end);
            let mut scaled_source = None;
            let mut receive_frames = || -> Result<(), String> {
                if finished.get() {
                    return Ok(());
                }
                loop {
                    check_cancel(cancel)?;
                    let ret = (api.avcodec_receive_frame)(decoder, decoded);
                    if ret == -11 || ret == -541478725 {
                        return Ok(());
                    }
                    ffcheck(api, ret)?;
                    let dimensions = ((*decoded).width, (*decoded).height, (*decoded).format);
                    if scaled_source != Some(dimensions) {
                        if !scaler.is_null() {
                            (api.sws_free_context)(scaler);
                        }
                        scaler = (api.sws_get_context)(
                            (*decoded).width,
                            (*decoded).height,
                            (*decoded).format,
                            source_width,
                            source_height,
                            (*encoder).pix_fmt,
                            2,
                            ptr::null_mut(),
                            ptr::null_mut(),
                            ptr::null(),
                        );
                        if scaler.is_null() {
                            return Err("Video color conversion failed".to_string());
                        }
                        scaled_source = Some(dimensions);
                    }
                    let pts = if (*decoded).best_effort_timestamp != i64::MIN {
                        let timestamp = (*decoded).best_effort_timestamp;
                        let tb = (*input_stream).time_base;
                        ((timestamp as f64 * tb.num as f64 / tb.den as f64 - media_origin)
                            * ticks_per_second)
                            .round() as i64
                    } else {
                        let rate = (*input_stream).avg_frame_rate;
                        let step = if rate.num > 0 && rate.den > 0 {
                            ticks_per_second * rate.den as f64 / rate.num as f64
                        } else {
                            ticks_per_second / 25.0
                        };
                        (decoded_count as f64 * step).round() as i64
                    };
                    decoded_count += 1;
                    let source_seconds = pts as f64 / ticks_per_second;
                    if source_seconds >= last_end {
                        finished.set(true);
                        (api.av_frame_unref)(decoded);
                        return Ok(());
                    }
                    let Some(edited_seconds) = output_time(parts, source_seconds) else {
                        (api.av_frame_unref)(decoded);
                        continue;
                    };
                    let pts = (edited_seconds * ticks_per_second).round() as i64;
                    {
                        check_cancel(cancel)?;
                        ffcheck(api, (api.av_frame_make_writable)(scaled))?;
                        ffcheck(
                            api,
                            (api.sws_scale)(
                                scaler,
                                (*decoded).data.as_ptr().cast(),
                                (*decoded).linesize.as_ptr(),
                                0,
                                (*decoded).height,
                                (*scaled).data.as_ptr(),
                                (*scaled).linesize.as_ptr(),
                            ),
                        )?;
                        (*scaled).pts = pts.max(last_pts + 1);
                        (*scaled).quality = (*encoder).global_quality;
                        last_pts = (*scaled).pts;
                        ffcheck(api, (api.av_frame_make_writable)(rotated))?;
                        rotate_yuv420(scaled, rotated, rotation);
                        (*rotated).pts = (*scaled).pts;
                        (*rotated).quality = (*scaled).quality;
                        ffcheck(api, (api.avcodec_send_frame)(encoder, rotated))?;
                        drain_encoder()?;
                        count += 1;
                        if let Some(duration) = duration {
                            let processed = last_pts as f64 / ticks_per_second;
                            progress(((processed / duration * 100.0) as u32).min(99));
                        }
                    }
                    (api.av_frame_unref)(decoded);
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
                    if finished.get() {
                        break;
                    }
                } else {
                    (api.av_packet_unref)(packet);
                }
            }
            if !finished.get() {
                ffcheck(api, (api.avcodec_send_packet)(decoder, ptr::null()))?;
                receive_frames()?;
            }
            if count == 0 {
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
        (api.av_frame_free)(&mut rotated);
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
pub(super) fn test_frames(path: &Path) -> Result<Vec<(i32, i32, f64)>, String> {
    let api = ffmpeg_api()?;
    let mut source = ptr::null_mut();
    let mut decoder = ptr::null_mut();
    let mut frame = ptr::null_mut();
    let mut packet = ptr::null_mut();
    unsafe {
        let result = (|| {
            let name = CString::new(path.to_string_lossy().as_bytes()).unwrap();
            ffcheck(
                api,
                (api.avformat_open_input)(
                    &mut source,
                    name.as_ptr(),
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
            let stream = *(*source).streams.add(index as usize);
            let codec = (api.avcodec_find_decoder)((*(*stream).codecpar).codec_id);
            decoder = (api.avcodec_alloc_context3)(codec);
            ffcheck(
                api,
                (api.avcodec_parameters_to_context)(decoder, (*stream).codecpar),
            )?;
            ffcheck(api, (api.avcodec_open2)(decoder, codec, ptr::null_mut()))?;
            frame = (api.av_frame_alloc)();
            packet = (api.av_packet_alloc)();
            let mut out = vec![];
            let mut receive = || -> Result<(), String> {
                loop {
                    let r = (api.avcodec_receive_frame)(decoder, frame);
                    if r == -11 || r == -541478725 {
                        return Ok(());
                    }
                    ffcheck(api, r)?;
                    let mut sum = 0u64;
                    for y in 0..(*frame).height {
                        for x in 0..(*frame).width {
                            sum += *(*frame).data[0].offset((y * (*frame).linesize[0] + x) as isize)
                                as u64;
                        }
                    }
                    out.push((
                        (*frame).width,
                        (*frame).height,
                        sum as f64 / ((*frame).width * (*frame).height) as f64,
                    ));
                    (api.av_frame_unref)(frame);
                }
            };
            loop {
                let r = (api.av_read_frame)(source, packet);
                if r == -541478725 {
                    break;
                }
                ffcheck(api, r)?;
                if (*packet).stream_index == index {
                    ffcheck(api, (api.avcodec_send_packet)(decoder, packet))?;
                    receive()?;
                }
                (api.av_packet_unref)(packet);
            }
            ffcheck(api, (api.avcodec_send_packet)(decoder, ptr::null()))?;
            receive()?;
            Ok(out)
        })();
        (api.av_packet_free)(&mut packet);
        (api.av_frame_free)(&mut frame);
        (api.avcodec_free_context)(&mut decoder);
        (api.avformat_close_input)(&mut source);
        result
    }
}
