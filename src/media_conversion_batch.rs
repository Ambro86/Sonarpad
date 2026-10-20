//! Sequential folder conversion; the caller supplies Sonarpad's library converter.
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) fn folder_files(folder: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(folder).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let extension = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        if path.is_file()
            && matches!(
                extension.as_str(),
                "mp3"
                    | "m4a"
                    | "mp4"
                    | "aac"
                    | "mkv"
                    | "avi"
                    | "mov"
                    | "m4v"
                    | "webm"
                    | "mpg"
                    | "mpeg"
                    | "ts"
                    | "m2ts"
                    | "mts"
                    | "wmv"
                    | "asf"
                    | "flv"
                    | "vob"
                    | "3gp"
                    | "flac"
                    | "ogg"
                    | "opus"
                    | "wma"
                    | "aif"
                    | "aiff"
                    | "m4b"
                    | "wav"
            )
        {
            files.push(path);
        }
    }
    files.sort_by_key(|p| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    });
    Ok(files)
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

pub(crate) fn plan_outputs(files: &[PathBuf], folder: &Path, extension: &str) -> Vec<PathBuf> {
    let mut reserved: HashSet<String> = files.iter().map(|p| path_key(p)).collect();
    files
        .iter()
        .map(|input| {
            let stem = input.file_stem().unwrap_or_default().to_string_lossy();
            let mut output = folder.join(format!("{stem}.{extension}"));
            let mut suffix = 1;
            while output.exists() || reserved.contains(&path_key(&output)) {
                let tail = if suffix == 1 {
                    "_converted".to_owned()
                } else {
                    format!("_converted_{suffix}")
                };
                output = folder.join(format!("{stem}{tail}.{extension}"));
                suffix += 1;
            }
            reserved.insert(path_key(&output));
            output
        })
        .collect()
}

#[derive(Debug, Default)]
pub(crate) struct BatchResult {
    pub succeeded: usize,
    pub failures: Vec<String>,
    pub cancelled: bool,
}

/// Overall progress includes completed files and the current file's real progress.
pub(crate) fn run(
    files: &[PathBuf],
    outputs: &[PathBuf],
    cancel: &AtomicBool,
    mut convert: impl FnMut(&Path, &Path, &mut dyn FnMut(u32)) -> Result<(), String>,
    mut progress: impl FnMut(usize, &Path, u32),
) -> BatchResult {
    assert_eq!(files.len(), outputs.len());
    let mut result = BatchResult::default();
    for (index, (input, output)) in files.iter().zip(outputs).enumerate() {
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            break;
        }
        progress(index + 1, input, (index * 100 / files.len()) as u32);
        let converted = convert(input, output, &mut |pct| {
            progress(
                index + 1,
                input,
                ((index * 100 + pct.min(100) as usize) / files.len()) as u32,
            );
        });
        match converted {
            Ok(()) => result.succeeded += 1,
            Err(error) => {
                if !cancel.load(Ordering::Relaxed) {
                    result.failures.push(format!(
                        "{}: {error}",
                        input.file_name().unwrap_or_default().to_string_lossy()
                    ));
                }
            }
        }
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            break;
        }
        progress(index + 1, input, ((index + 1) * 100 / files.len()) as u32);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_or_pre_cancelled_queue_does_not_invoke_converter() {
        let cancel = AtomicBool::new(false);
        let result = run(
            &[],
            &[],
            &cancel,
            |_, _, _| panic!("empty queue"),
            |_, _, _| {},
        );
        assert_eq!(result.succeeded, 0);
        cancel.store(true, Ordering::Relaxed);
        let files = vec![PathBuf::from("input.wav")];
        let result = run(
            &files,
            &files,
            &cancel,
            |_, _, _| panic!("cancelled queue"),
            |_, _, _| {},
        );
        assert!(result.cancelled);
    }

    #[test]
    fn bundled_libraries_convert_batch_and_continue_past_corrupt_file() {
        use crate::ffmpeg_export::{ConvertAudioFormat, ConvertAudioQuality, ConvertAudioSettings};
        use std::sync::Arc;
        let dir = test_dir();
        for name in ["a.wav", "c.wav"] {
            let mut writer = hound::WavWriter::create(
                dir.join(name),
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
                        ((i as f32 * 440.0 * std::f32::consts::TAU / 48000.0).sin() * 16000.0)
                            as i16,
                    )
                    .unwrap();
            }
            writer.finalize().unwrap();
        }
        std::fs::write(dir.join("b.mp4"), b"deliberately invalid media").unwrap();
        let files = folder_files(&dir).unwrap();
        let output_dir = dir.join("Converted");
        std::fs::create_dir(&output_dir).unwrap();
        let outputs = plan_outputs(&files, &output_dir, "flac");
        let settings = ConvertAudioSettings {
            format: ConvertAudioFormat::Flac,
            quality: ConvertAudioQuality::FlacCompression(5),
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let mut progress = Vec::new();
        let result = run(
            &files,
            &outputs,
            &cancel,
            |input, output, report| {
                crate::media_conversion::convert(
                    input,
                    output,
                    &settings,
                    false,
                    None,
                    cancel.clone(),
                    report,
                )
            },
            |_, _, pct| progress.push(pct),
        );
        assert_eq!(result.succeeded, 2, "{:?}", result.failures);
        assert_eq!(result.failures.len(), 1);
        assert!(result.failures[0].contains("b.mp4"));
        assert!(!outputs[1].exists());
        for output in [&outputs[0], &outputs[2]] {
            assert!(
                (crate::ffmpeg_export::media_duration_seconds(output).unwrap() - 0.5).abs() < 0.05
            );
            assert!(
                !crate::ffmpeg_source::list_audio_streams(output)
                    .unwrap()
                    .is_empty()
            );
        }
        assert!(progress.windows(2).all(|p| p[0] <= p[1]));
        assert_eq!(progress.last(), Some(&100));
        assert!(dir.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(dir).unwrap();
    }
    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sonarpad-folder-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn scan_is_sorted_nonrecursive_and_accepts_uppercase_extensions() {
        let dir = test_dir();
        for name in ["z.MP4", "a.wav", "notes.txt"] {
            std::fs::write(dir.as_path().join(name), b"fixture").unwrap();
        }
        std::fs::create_dir(dir.as_path().join("nested")).unwrap();
        std::fs::write(dir.as_path().join("nested/hidden.mp3"), b"fixture").unwrap();
        let files = folder_files(dir.as_path()).unwrap();
        assert_eq!(
            files,
            vec![dir.as_path().join("a.wav"), dir.as_path().join("z.MP4")]
        );
        assert!(folder_files(&dir.as_path().join("missing")).is_err());
    }

    #[test]
    fn plan_preserves_sources_existing_files_and_case_insensitive_collisions() {
        let dir = test_dir();
        let files: Vec<_> = ["Song.mp3", "song.wav", "other.mp4"]
            .iter()
            .map(|n| dir.as_path().join(n))
            .collect();
        for p in &files {
            std::fs::write(p, b"original").unwrap();
        }
        std::fs::write(dir.as_path().join("other.mp3"), b"keep").unwrap();
        let outputs = plan_outputs(&files, dir.as_path(), "mp3");
        assert_eq!(outputs[0], dir.as_path().join("Song_converted.mp3"));
        assert_eq!(outputs[1], dir.as_path().join("song_converted_2.mp3"));
        assert_eq!(outputs[2], dir.as_path().join("other_converted.mp3"));
        assert_eq!(std::fs::read(&files[0]).unwrap(), b"original");
    }

    #[test]
    fn simulated_batch_continues_after_failure_and_reports_real_overall_progress() {
        let files = vec![
            PathBuf::from("one.wav"),
            PathBuf::from("bad.mp4"),
            PathBuf::from("three.flac"),
        ];
        let outputs = vec![
            PathBuf::from("one.mp3"),
            PathBuf::from("bad.mp3"),
            PathBuf::from("three.mp3"),
        ];
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();
        let result = run(
            &files,
            &outputs,
            &cancel,
            |input, _, report| {
                report(50);
                if input == Path::new("bad.mp4") {
                    Err("damaged".into())
                } else {
                    report(100);
                    Ok(())
                }
            },
            |_, _, pct| seen.push(pct),
        );
        assert_eq!(result.succeeded, 2);
        assert_eq!(result.failures, vec!["bad.mp4: damaged"]);
        assert!(!result.cancelled);
        assert!(seen.contains(&16));
        assert!(seen.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(seen.last(), Some(&100));
    }

    #[test]
    fn cancellation_stops_queue_and_keeps_completed_count() {
        let files = vec![
            PathBuf::from("one.wav"),
            PathBuf::from("two.wav"),
            PathBuf::from("three.wav"),
        ];
        let cancel = AtomicBool::new(false);
        let mut calls = 0;
        let result = run(
            &files,
            &files,
            &cancel,
            |_, _, _| {
                calls += 1;
                if calls == 2 {
                    cancel.store(true, Ordering::Relaxed);
                    Err("Conversion canceled.".into())
                } else {
                    Ok(())
                }
            },
            |_, _, _| {},
        );
        assert_eq!(calls, 2);
        assert_eq!(result.succeeded, 1);
        assert!(result.cancelled);
        assert!(result.failures.is_empty());
    }
}
