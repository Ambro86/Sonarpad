//! Frame/sample accurate media editing through the bundled FFmpeg DLLs.
use crate::ffmpeg_export::{ConvertAudioFormat, ConvertAudioQuality, ConvertAudioSettings};
use crate::ffmpeg_source::FfmpegSource;
use crate::media_cutter_model::{MediaPart, kept_duration};
use rodio::source::UniformSourceIterator;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[path = "media_cutter_timeline_video.rs"]
mod timeline_video;
#[path = "media_cutter_video.rs"]
mod video;
use crate::media_cutter_timeline::{self, MediaSpan, SourceClip};
#[derive(Clone, Debug)]
pub struct AddedTrack {
    pub path: PathBuf,
    pub original_volume: f32,
    pub new_volume: f32,
    pub loop_track: bool,
}
#[derive(Clone, Debug)]
pub struct ExportPlan {
    pub input: PathBuf,
    pub output: PathBuf,
    pub parts: Vec<MediaPart>,
    pub sources: Vec<MediaSpan>,
    pub rotation: u16,
    pub track: Option<AddedTrack>,
}
pub const AUDIO_EXTENSIONS: &[&str] = &["m4a", "mp3", "wav", "flac", "ogg", "opus", "aiff"];
pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "mov"];
fn output_settings(ext: &str) -> Result<ConvertAudioSettings, String> {
    use ConvertAudioFormat::*;
    use ConvertAudioQuality::*;
    let (format, quality) = match ext {
        "mp3" | "avi" => (Mp3, BitrateKbps(192)),
        "m4a" | "mp4" | "mkv" | "mov" => (Aac, BitrateKbps(192)),
        "wav" => (Wav, None),
        "aiff" => (Aiff, None),
        "flac" => (Flac, FlacCompression(5)),
        "ogg" => (Ogg, OggQuality(5)),
        "opus" => (Opus, BitrateKbps(128)),
        _ => return Err("Unsupported output format".into()),
    };
    Ok(ConvertAudioSettings { format, quality })
}
fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("__CANCELLED__".into())
    } else {
        Ok(())
    }
}
type Stereo = std::iter::Fuse<UniformSourceIterator<FfmpegSource>>;
fn audio(path: &Path, start: f64) -> Result<Stereo, String> {
    Ok(
        UniformSourceIterator::new(FfmpegSource::try_new_at(path, start, None, None)?, 2, 48000)
            .fuse(),
    )
}
pub fn export(
    plan: &ExportPlan,
    cancel: Arc<AtomicBool>,
    progress: &mut dyn FnMut(u32),
) -> Result<(), String> {
    check_cancel(&cancel)?;
    if crate::media_conversion::same_file(&plan.input, &plan.output)
        || plan
            .sources
            .iter()
            .any(|s| crate::media_conversion::same_file(&s.path, &plan.output))
        || plan
            .track
            .as_ref()
            .is_some_and(|t| crate::media_conversion::same_file(&t.path, &plan.output))
    {
        return Err("Input and output must be different files".into());
    }
    let duration = if let Some(last) = plan.sources.last() {
        media_cutter_timeline::validate(&plan.sources, last.end)?;
        for span in &plan.sources {
            let source_duration = crate::ffmpeg_export::media_duration_seconds(&span.path)
                .ok_or("Cannot determine inserted media duration")?;
            if span.source_start + span.end - span.start > source_duration + 0.1 {
                return Err(
                    "Inserted media has changed or is shorter than the selected range".into(),
                );
            }
        }
        last.end
    } else {
        crate::ffmpeg_export::media_duration_seconds(&plan.input)
            .ok_or("Cannot determine media duration")?
    };
    let mut previous = 0.0;
    for p in &plan.parts {
        if !p.start.is_finite()
            || !p.end.is_finite()
            || p.start < previous
            || p.end <= p.start
            || p.end > duration + 0.1
        {
            return Err("Invalid edit boundaries".into());
        }
        previous = p.end;
    }
    let kept: Vec<_> = plan.parts.iter().filter(|p| p.keep).cloned().collect();
    let total = kept_duration(&kept);
    if total < 0.01 {
        return Err("No media left to save".into());
    }
    if ![0, 90, 180, 270].contains(&plan.rotation) {
        return Err("Invalid video rotation".into());
    }
    let clips = if plan.sources.is_empty() {
        kept.iter()
            .map(|p| SourceClip {
                path: plan.input.clone(),
                start: p.start,
                end: p.end,
            })
            .collect()
    } else {
        media_cutter_timeline::resolve(&plan.sources, &kept)?
    };
    let mut media = std::collections::HashMap::new();
    for clip in &clips {
        if !media.contains_key(&clip.path) {
            media.insert(
                clip.path.clone(),
                (
                    crate::ffmpeg_source::has_real_video_stream(&clip.path)?,
                    !crate::ffmpeg_source::list_audio_streams(&clip.path)?.is_empty(),
                ),
            );
        }
    }
    let has_video = media.values().any(|v| v.0);
    let has_audio = media.values().any(|v| v.1);
    let extension = plan
        .output
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let settings = output_settings(&extension)?;
    let output_video = VIDEO_EXTENSIONS.contains(&extension.as_str());
    if output_video && !has_video && plan.sources.is_empty() {
        return Err("Input has no video stream".into());
    }
    if !output_video && !has_audio && plan.track.is_none() {
        return Err("Input has no audio stream".into());
    }
    let parent = plan.output.parent().unwrap_or(Path::new("."));
    let work = parent.join(format!(".sonarpad-cut-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&work).map_err(|e| e.to_string())?;
    let mut last = None;
    let mut report = |pct: u32| {
        if last.is_none_or(|v| pct > v) {
            last = Some(pct);
            progress(pct);
        }
    };
    let result = (|| {
        report(0);
        let wav = work.join("edited.wav");
        let audio_extension = if output_video {
            if extension == "avi" { "mp3" } else { "m4a" }
        } else {
            &extension
        };
        let encoded_audio = work.join(format!("audio.{audio_extension}"));
        let encoded_video = if plan.sources.is_empty() {
            work.join(format!("video.{extension}"))
        } else {
            work.join("video.ffconcat")
        };
        let staged = work.join(format!("final.{extension}"));
        let output_audio = has_audio || plan.track.is_some() || !plan.sources.is_empty();
        if output_audio {
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
            let mut background = plan
                .track
                .as_ref()
                .map(|t| audio(&t.path, 0.0))
                .transpose()?;
            let original_gain = plan
                .track
                .as_ref()
                .map_or(1.0, |t| t.original_volume.clamp(0.0, 2.0));
            let added_gain = plan
                .track
                .as_ref()
                .map_or(0.0, |t| t.new_volume.clamp(0.0, 2.0));
            let mut done = 0_u64;
            for part in &clips {
                check_cancel(&cancel)?;
                let mut original = if media.get(&part.path).is_some_and(|v| v.1) {
                    Some(audio(&part.path, part.start)?)
                } else {
                    None
                };
                let count = ((part.end - part.start) * 48000.0).round() as u64 * 2;
                let mut missing = 0_u64;
                for i in 0..count {
                    if i % 8192 == 0 {
                        check_cancel(&cancel)?;
                        report(((done + i) as f64 / (total * 96000.0) * 30.0) as u32);
                    }
                    let value = if let Some(source) = original.as_mut() {
                        match source.next() {
                            Some(v) => v,
                            None => {
                                missing += 1;
                                0.0
                            }
                        }
                    } else {
                        0.0
                    };
                    let mut added = background.as_mut().and_then(Iterator::next);
                    if added.is_none()
                        && let Some(t) = plan.track.as_ref()
                        && t.loop_track
                    {
                        background = Some(audio(&t.path, 0.0)?);
                        added = background.as_mut().and_then(Iterator::next);
                        if added.is_none() {
                            return Err("Added audio track is empty".into());
                        }
                    }
                    let limit = if plan.track.is_some() { 0.98 } else { 1.0 };
                    let mixed = (value * original_gain + added.unwrap_or(0.0) * added_gain)
                        .clamp(-limit, limit);
                    writer
                        .write_sample((mixed * i16::MAX as f32).round() as i16)
                        .map_err(|e| e.to_string())?;
                }
                if missing > 9600 {
                    return Err("Audio ended before the selected cut boundary".into());
                }
                done += count;
            }
            writer.finalize().map_err(|e| e.to_string())?;
            crate::ffmpeg_export::convert_audio_file(
                &wav,
                &encoded_audio,
                &settings,
                Some(cancel.clone()),
                Some(&mut |v| {
                    report(30 + v.min(10000) * if output_video { 15 } else { 69 } / 10000)
                }),
            )?;
        }
        if output_video {
            if plan.sources.is_empty() {
                video::encode_video(
                    &plan.input,
                    &encoded_video,
                    &kept,
                    plan.rotation,
                    &cancel,
                    &mut |p| report(45 + p.min(100) * 45 / 100),
                )?;
            } else {
                timeline_video::prepare(
                    &encoded_video,
                    &clips,
                    plan.rotation,
                    &cancel,
                    &mut |p| report(45 + p.min(100) * 45 / 100),
                )?;
            }
            if output_audio {
                crate::ffmpeg_export::remux_media_file_to_mp4_with_external_audio_stream(
                    &encoded_video,
                    &encoded_audio,
                    &staged,
                    Some(cancel.clone()),
                    Some(&mut |p| report(90 + p.min(100) * 9 / 100)),
                )?;
            } else {
                std::fs::rename(&encoded_video, &staged).map_err(|e| e.to_string())?;
            }
        } else {
            std::fs::rename(&encoded_audio, &staged).map_err(|e| e.to_string())?;
        }
        check_cancel(&cancel)?;
        let actual = crate::ffmpeg_export::media_duration_seconds(&staged)
            .ok_or("Cannot validate saved media")?;
        if (actual - total).abs() > 0.25 {
            return Err(format!(
                "Unexpected output duration: {actual:.3} instead of {total:.3}"
            ));
        }
        std::fs::rename(&staged, &plan.output).map_err(|e| e.to_string())?;
        report(100);
        Ok(())
    })();
    if let Err(e) = std::fs::remove_dir_all(&work) {
        crate::log_debug(&format!("Media cutter temporary cleanup: {e}"));
    }
    result
}

#[cfg(test)]
#[path = "media_cutter_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) fn test_video_frames(path: &Path) -> Result<Vec<(i32, i32, f64)>, String> {
    video::test_frames(path)
}
