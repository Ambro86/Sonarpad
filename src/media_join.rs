//! Ordered media joining, entirely through Sonarpad's bundled libraries.
use crate::ffmpeg_export::{ConvertAudioFormat, ConvertAudioSettings};
use crate::ffmpeg_source::FfmpegSource;
use rodio::source::UniformSourceIterator;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Conversion canceled.".into())
    } else {
        Ok(())
    }
}

fn dimensions(path: &Path) -> Result<(i32, i32), String> {
    use crate::ffmpeg_dyn::*;
    let api = crate::ffmpeg_source::ffmpeg_api()?;
    let name =
        std::ffi::CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    let mut context = std::ptr::null_mut();
    unsafe {
        let result = (|| {
            let ret = (api.avformat_open_input)(
                &mut context,
                name.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            if ret < 0 {
                return Err(crate::ffmpeg_source::ffmpeg_err(api, ret));
            }
            let ret = (api.avformat_find_stream_info)(context, std::ptr::null_mut());
            if ret < 0 {
                return Err(crate::ffmpeg_source::ffmpeg_err(api, ret));
            }
            let index = (api.av_find_best_stream)(
                context,
                AVMediaType_AVMEDIA_TYPE_VIDEO,
                -1,
                -1,
                std::ptr::null_mut(),
                0,
            );
            if index < 0 {
                return Err("Input has no video stream".into());
            }
            let params = (*(*(*context).streams.add(index as usize))).codecpar;
            Ok(((*params).width, (*params).height))
        })();
        (api.avformat_close_input)(&mut context);
        result
    }
}

pub(crate) fn join(
    inputs: &[PathBuf],
    output: &Path,
    settings: &ConvertAudioSettings,
    video: bool,
    cancel: Arc<AtomicBool>,
    progress: &mut dyn FnMut(u32),
) -> Result<(), String> {
    cancelled(&cancel)?;
    if inputs.len() < 2 {
        return Err("Select at least two media files".into());
    }
    if inputs
        .iter()
        .any(|p| crate::media_conversion::same_file(p, output))
    {
        return Err("Input and output must be different files".into());
    }
    let mut durations = Vec::new();
    let mut audio = Vec::new();
    for input in inputs {
        cancelled(&cancel)?;
        if video && !crate::ffmpeg_source::has_real_video_stream(input)? {
            return Err(format!("Input has no video stream: {}", input.display()));
        }
        durations.push(
            crate::ffmpeg_export::media_duration_seconds(input)
                .filter(|d| d.is_finite() && *d > 0.0)
                .ok_or("Cannot determine media duration")?,
        );
        audio.push(!crate::ffmpeg_source::list_audio_streams(input)?.is_empty());
    }
    if !video && audio.iter().any(|a| !a) {
        return Err("Input has no audio stream".into());
    }
    let total: f64 = durations.iter().sum();
    let work = output
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!(".sonarpad-join-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&work).map_err(|e| e.to_string())?;
    let mut last = None;
    let mut report = |pct| {
        if last.is_none_or(|old| pct > old) {
            last = Some(pct);
            progress(pct);
        }
    };
    let result = (|| {
        report(0);
        let wav = work.join("joined.wav");
        let mut writer = hound::WavWriter::create(
            &wav,
            hound::WavSpec {
                channels: 2,
                sample_rate: 48000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .map_err(|e| e.to_string())?;
        let mut completed = 0.0;
        for (index, input) in inputs.iter().enumerate() {
            let mut source = if audio[index] {
                Some(
                    UniformSourceIterator::new(
                        FfmpegSource::try_new_at(input, 0.0, None, None)?,
                        2,
                        48000,
                    )
                    .fuse(),
                )
            } else {
                None
            };
            let count = (durations[index] * 48000.0).round() as u64 * 2;
            let mut samples = 0_u64;
            loop {
                if samples.is_multiple_of(8192) {
                    cancelled(&cancel)?;
                    report(
                        ((completed + samples as f64 / 96000.0) / total * 30.0).min(30.0) as u32,
                    );
                }
                if video && samples >= count {
                    break;
                }
                let next = source.as_mut().and_then(Iterator::next);
                if !video && next.is_none() {
                    break;
                }
                let value = (next.unwrap_or(0.0).clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
                writer.write_sample(value).map_err(|e| e.to_string())?;
                samples += 1;
            }
            if samples == 0 {
                return Err(format!("Empty audio: {}", input.display()));
            }
            completed += durations[index];
        }
        writer.finalize().map_err(|e| e.to_string())?;
        let extension = output
            .extension()
            .and_then(|s| s.to_str())
            .ok_or("Missing output format")?;
        let staged = work.join(format!("result.{extension}"));
        if video {
            let size = dimensions(&inputs[0])?;
            let mut manifest = String::from("ffconcat version 1.0\n");
            let mut done = 0.0;
            for (index, input) in inputs.iter().enumerate() {
                cancelled(&cancel)?;
                let name = format!("clip{index}.mp4");
                crate::media_conversion::encode_video_sized(
                    input,
                    &work.join(&name),
                    None,
                    Some(size),
                    &cancel,
                    &mut |pct| {
                        report(
                            30 + ((done + durations[index] * pct as f64 / 100.0) / total * 40.0)
                                as u32,
                        )
                    },
                )?;
                manifest.push_str(&format!(
                    "file '{name}'\nduration {:.9}\n",
                    durations[index]
                ));
                done += durations[index];
            }
            let list = work.join("joined.ffconcat");
            std::fs::write(&list, manifest).map_err(|e| e.to_string())?;
            let encoded_audio = work.join(if settings.format == ConvertAudioFormat::Mp3 {
                "audio.mp3"
            } else {
                "audio.m4a"
            });
            crate::ffmpeg_export::convert_audio_file(
                &wav,
                &encoded_audio,
                settings,
                Some(cancel.clone()),
                Some(&mut |pct| report(70 + pct.min(10000) * 20 / 10000)),
            )?;
            crate::ffmpeg_export::remux_media_file_to_mp4_with_external_audio_stream(
                &list,
                &encoded_audio,
                &staged,
                Some(cancel.clone()),
                Some(&mut |pct| report(90 + pct.min(100) * 9 / 100)),
            )?;
        } else {
            crate::ffmpeg_export::convert_audio_file(
                &wav,
                &staged,
                settings,
                Some(cancel.clone()),
                Some(&mut |pct| report(30 + pct.min(10000) * 69 / 10000)),
            )?;
        }
        cancelled(&cancel)?;
        std::fs::rename(&staged, output).map_err(|e| e.to_string())?;
        report(100);
        Ok(())
    })();
    if let Err(error) = std::fs::remove_dir_all(&work) {
        crate::log_debug(&format!("Media join cleanup: {error}"));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffmpeg_export::{ConvertAudioQuality, media_duration_seconds};

    fn fixture() -> (PathBuf, Vec<PathBuf>) {
        let dir = std::env::temp_dir().join(format!("sonarpad-join-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let paths = vec![dir.join("first.wav"), dir.join("second.wav")];
        for (index, path) in paths.iter().enumerate() {
            let rate = if index == 0 { 44100 } else { 48000 };
            let channels = if index == 0 { 1 } else { 2 };
            let mut writer = hound::WavWriter::create(
                path,
                hound::WavSpec {
                    channels,
                    sample_rate: rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for _ in 0..rate * channels as u32 {
                writer
                    .write_sample(if index == 0 { 6000_i16 } else { -6000_i16 })
                    .unwrap();
            }
            writer.finalize().unwrap();
        }
        (dir, paths)
    }

    #[test]
    fn real_audio_join_normalizes_rates_channels_and_preserves_order() {
        let (dir, files) = fixture();
        let output = dir.join("joined.wav");
        let settings = ConvertAudioSettings {
            format: ConvertAudioFormat::Wav,
            quality: ConvertAudioQuality::None,
        };
        let mut progress = Vec::new();
        join(
            &files,
            &output,
            &settings,
            false,
            Arc::new(AtomicBool::new(false)),
            &mut |p| progress.push(p),
        )
        .unwrap();
        let reader = hound::WavReader::open(&output).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(reader.spec().sample_rate, 48000);
        let samples: Vec<i16> = reader.into_samples().map(Result::unwrap).collect();
        assert!((samples.len() as i64 - 192000).abs() < 300);
        assert!(samples[24000..48000].iter().all(|s| *s > 5000));
        assert!(samples[120000..144000].iter().all(|s| *s < -5000));
        assert_eq!(progress.first(), Some(&0));
        assert_eq!(progress.last(), Some(&100));
        assert!(progress.windows(2).all(|p| p[0] < p[1]));
        // Reordering affects the exported content, not only the displayed list.
        let reversed = vec![files[1].clone(), files[0].clone()];
        join(
            &reversed,
            &output,
            &settings,
            false,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .unwrap();
        let samples: Vec<i16> = hound::WavReader::open(&output)
            .unwrap()
            .into_samples()
            .map(Result::unwrap)
            .collect();
        assert!(samples[24000..48000].iter().all(|s| *s < -5000));
        assert!(samples[120000..144000].iter().all(|s| *s > 5000));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancellation_and_invalid_inputs_preserve_sources_and_destination() {
        let (dir, files) = fixture();
        let settings = ConvertAudioSettings {
            format: ConvertAudioFormat::Flac,
            quality: ConvertAudioQuality::FlacCompression(5),
        };
        let source = std::fs::read(&files[0]).unwrap();
        assert!(
            join(
                &files,
                &files[0],
                &settings,
                false,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {}
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&files[0]).unwrap(), source);
        let output = dir.join("keep.flac");
        std::fs::write(&output, b"keep me").unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let result = join(
            &files,
            &output,
            &settings,
            false,
            cancel.clone(),
            &mut |p| {
                if p > 0 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        );
        assert_eq!(result.unwrap_err(), "Conversion canceled.");
        assert_eq!(std::fs::read(&output).unwrap(), b"keep me");
        let bad = dir.join("bad.mp4");
        std::fs::write(&bad, b"invalid video").unwrap();
        assert!(
            join(
                &[files[0].clone(), bad],
                &output,
                &settings,
                false,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {}
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&output).unwrap(), b"keep me");
        assert!(!std::fs::read_dir(&dir).unwrap().any(|p| {
            p.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".sonarpad-join")
        }));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn real_video_join_and_audio_extraction_use_bundled_libraries() {
        let (dir, audio) = fixture();
        let settings = ConvertAudioSettings {
            format: ConvertAudioFormat::Aac,
            quality: ConvertAudioQuality::BitrateKbps(192),
        };
        let flag = Arc::new(AtomicBool::new(false));
        let mut videos = Vec::new();
        for (index, source) in audio.iter().enumerate() {
            let (w, h) = if index == 0 {
                (32_u32, 24_u32)
            } else {
                (48, 32)
            };
            let length = 54 + w * h * 3;
            let mut bmp = vec![0_u8; length as usize];
            bmp[..2].copy_from_slice(b"BM");
            bmp[2..6].copy_from_slice(&length.to_le_bytes());
            bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
            bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
            bmp[18..22].copy_from_slice(&w.to_le_bytes());
            bmp[22..26].copy_from_slice(&h.to_le_bytes());
            bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
            bmp[28..30].copy_from_slice(&24_u16.to_le_bytes());
            for pixel in bmp[54..].chunks_mut(3) {
                pixel[if index == 0 { 2 } else { 1 }] = 220;
            }
            let image = dir.join(format!("image{index}.bmp"));
            std::fs::write(&image, bmp).unwrap();
            let video = dir.join(format!("video{index}.mp4"));
            crate::media_conversion::convert(
                source,
                &video,
                &settings,
                true,
                Some(&image),
                flag.clone(),
                &mut |_| {},
            )
            .unwrap();
            videos.push(video);
        }
        for ext in ["mp4", "avi", "mov"] {
            let output = dir.join(format!("joined.{ext}"));
            let audio_settings = if ext == "avi" {
                ConvertAudioSettings {
                    format: ConvertAudioFormat::Mp3,
                    quality: ConvertAudioQuality::BitrateKbps(192),
                }
            } else {
                settings.clone()
            };
            let mut progress = Vec::new();
            join(
                &videos,
                &output,
                &audio_settings,
                true,
                flag.clone(),
                &mut |p| progress.push(p),
            )
            .unwrap();
            let duration = media_duration_seconds(&output).unwrap();
            assert!((duration - 2.0).abs() < 0.2, "{ext}: {duration}");
            assert!(crate::ffmpeg_source::has_real_video_stream(&output).unwrap());
            assert!(
                !crate::ffmpeg_source::list_audio_streams(&output)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(dimensions(&output).unwrap(), (32, 24));
            let frames = crate::media_cutter_export::test_video_frames(&output).unwrap();
            assert!(
                (48..=54).contains(&frames.len()),
                "{ext}: {} frames",
                frames.len()
            );
            assert!(frames.iter().all(|f| f.0 == 32 && f.1 == 24));
            assert!(
                frames[5].2 + 25.0 < frames[frames.len() - 5].2,
                "Both source scenes must survive joining"
            );

            assert_eq!(progress.last(), Some(&100));
            assert!(progress.windows(2).all(|p| p[0] < p[1]));
        }
        let silent = dir.join("silent.mp4");
        crate::media_conversion::encode_video_sized(
            &videos[0],
            &silent,
            None,
            None,
            &flag,
            &mut |_| {},
        )
        .unwrap();
        let silent_joined = dir.join("with-silence.mp4");
        join(
            &[silent, videos[1].clone()],
            &silent_joined,
            &settings,
            true,
            flag.clone(),
            &mut |_| {},
        )
        .unwrap();
        assert!((media_duration_seconds(&silent_joined).unwrap() - 2.0).abs() < 0.2);
        assert!(
            !crate::ffmpeg_source::list_audio_streams(&silent_joined)
                .unwrap()
                .is_empty()
        );
        let output = dir.join("extracted.flac");
        let settings = ConvertAudioSettings {
            format: ConvertAudioFormat::Flac,
            quality: ConvertAudioQuality::FlacCompression(5),
        };
        join(&videos, &output, &settings, false, flag, &mut |_| {}).unwrap();
        assert!((media_duration_seconds(&output).unwrap() - 2.0).abs() < 0.2);
        assert!(!crate::ffmpeg_source::has_real_video_stream(&output).unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
