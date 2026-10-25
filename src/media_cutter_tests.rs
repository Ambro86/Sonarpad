use super::*;
use crate::media_cutter_model::*;
use rodio::Source;
use std::io::Write;
fn fixtures() -> (PathBuf, PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("sonarpad-cutter-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let audio = dir.join("audio, è; test.wav");
    let mut wav = hound::WavWriter::create(
        &audio,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..288000 {
        let hz = [440.0, 880.0, 1320.0][i / 96000];
        wav.write_sample(
            ((i as f64 * hz * std::f64::consts::TAU / 48000.0).sin() * 14000.0) as i16,
        )
        .unwrap();
    }
    wav.finalize().unwrap();
    let raw = dir.join("colors.y4m");
    let mut file = std::fs::File::create(&raw).unwrap();
    file.write_all(b"YUV4MPEG2 W64 H48 F25:1 Ip A1:1 C420jpeg\n")
        .unwrap();
    for i in 0..150 {
        file.write_all(b"FRAME\n").unwrap();
        let (y, u, v) = [(81, 90, 240), (145, 54, 34), (41, 240, 110)][i / 50];
        file.write_all(&vec![y; 64 * 48]).unwrap();
        file.write_all(&vec![u; 32 * 24]).unwrap();
        file.write_all(&vec![v; 32 * 24]).unwrap();
    }
    file.flush().unwrap();
    let video = dir.join("colors.mp4");
    crate::media_conversion::convert(
        &raw,
        &video,
        &ConvertAudioSettings {
            format: ConvertAudioFormat::Aac,
            quality: ConvertAudioQuality::BitrateKbps(192),
        },
        true,
        None,
        Arc::new(AtomicBool::new(false)),
        &mut |_| {},
    )
    .unwrap();
    let encoded_audio = dir.join("original.m4a");
    crate::ffmpeg_export::convert_audio_file(
        &audio,
        &encoded_audio,
        &ConvertAudioSettings {
            format: ConvertAudioFormat::Aac,
            quality: ConvertAudioQuality::BitrateKbps(192),
        },
        None,
        None,
    )
    .unwrap();
    let av = dir.join("source, è; video.mp4");
    crate::ffmpeg_export::remux_media_file_to_mp4_with_external_audio_stream(
        &video,
        &encoded_audio,
        &av,
        None,
        None,
    )
    .unwrap();
    (dir, audio, av)
}
fn plan(input: &Path, output: &Path) -> ExportPlan {
    ExportPlan {
        input: input.into(),
        sources: vec![],
        output: output.into(),
        parts: vec![
            MediaPart {
                start: 0.0,
                end: 2.0,
                keep: true,
            },
            MediaPart {
                start: 2.0,
                end: 4.0,
                keep: false,
            },
            MediaPart {
                start: 4.0,
                end: 6.0,
                keep: true,
            },
        ],
        rotation: 0,
        track: None,
    }
}
fn tones(path: &Path) {
    let source = FfmpegSource::try_new_at(path, 0.0, None, None).unwrap();
    let rate = source.sample_rate() as usize;
    let channels = source.channels() as usize;
    let samples: Vec<f32> = source.collect();
    assert!((samples.len() as f64 / channels as f64 / rate as f64 - 4.0).abs() < 0.12);
    for (second, hz) in [(0, 440.0), (2, 1320.0)] {
        let block: Vec<_> = samples
            .iter()
            .skip((second * rate + rate / 4) * channels)
            .step_by(channels)
            .take(rate / 2)
            .copied()
            .collect();
        let energy = |freq: f64| {
            let (re, im) = block.iter().enumerate().fold((0.0, 0.0), |(r, m), (i, x)| {
                let a = i as f64 * freq * std::f64::consts::TAU / rate as f64;
                (r + *x as f64 * a.cos(), m + *x as f64 * a.sin())
            });
            re.hypot(im)
        };
        assert!(energy(hz) > energy(880.0) * 20.0, "deleted tone leaked");
    }
}
#[test]
fn safe_reexport_preserves_backup_and_source() -> Result<(), String> {
    let (dir, input, _video) = fixtures();
    let output = dir.join("previously-edited.m4a");
    let plan = plan(&input, &output);
    let cancelled = Arc::new(AtomicBool::new(false));
    assert!(
        crate::media_cutter_project::export_with_safe_replacement(
            &plan,
            cancelled.clone(),
            &mut |_| {}
        )?
        .is_none()
    );
    let original_output = std::fs::read(&output).map_err(|e| e.to_string())?;
    let original_source = std::fs::read(&input).map_err(|e| e.to_string())?;
    let backup = crate::media_cutter_project::export_with_safe_replacement(
        &plan,
        cancelled.clone(),
        &mut |_| {},
    )?
    .ok_or("Missing backup for existing output")?;
    assert_eq!(
        std::fs::read(&backup).map_err(|e| e.to_string())?,
        original_output
    );
    tones(&output);
    assert_eq!(
        std::fs::read(&input).map_err(|e| e.to_string())?,
        original_source
    );
    let mut invalid = plan.clone();
    invalid.output = input.clone();
    assert!(
        crate::media_cutter_project::export_with_safe_replacement(&invalid, cancelled, &mut |_| {})
            .is_err()
    );
    assert_eq!(
        std::fs::read(&input).map_err(|e| e.to_string())?,
        original_source
    );
    std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    Ok(())
}
#[test]
fn native_cutter_real_exports_and_playback() -> Result<(), String> {
    let (dir, audio, av) = fixtures();
    for (input, ext) in [(&audio, "m4a"), (&av, "mp4")] {
        let output = dir.join(format!("edited.{ext}"));
        let p = plan(input, &output);
        let mut progress = vec![];
        export(&p, Arc::new(AtomicBool::new(false)), &mut |v| {
            progress.push(v)
        })?;
        tones(&output);
        assert_eq!(progress.first(), Some(&0));
        assert_eq!(progress.last(), Some(&100));
        assert!(progress.windows(2).all(|w| w[0] < w[1]));
        assert!(progress.iter().any(|v| *v > 0 && *v < 100));
        let saved = std::fs::read(&output).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let r = export(&p, cancel.clone(), &mut |v| {
            if v > 0 {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert!(r.is_err());
        assert_eq!(std::fs::read(&output).unwrap(), saved);
    }
    for rotation in [0, 90, 180, 270] {
        let mut p = plan(&av, &dir.join(format!("rotated-{rotation}.mp4")));
        p.rotation = rotation;
        export(&p, Arc::new(AtomicBool::new(false)), &mut |_| {})?;
        let frames = super::video::test_frames(&p.output)?;
        assert!((99..=101).contains(&frames.len()));
        let dims = if rotation == 90 || rotation == 270 {
            (48, 64)
        } else {
            (64, 48)
        };
        assert!(frames.iter().all(|f| (f.0, f.1) == dims));
        assert!(
            frames.iter().all(|f| f.2 < 110.0),
            "deleted green frames leaked: {frames:?}"
        );
        tones(&p.output);
    }
    let silent_plan = plan(&dir.join("colors.mp4"), &dir.join("silent.mp4"));
    export(&silent_plan, Arc::new(AtomicBool::new(false)), &mut |_| {})?;
    assert!(crate::ffmpeg_source::list_audio_streams(&silent_plan.output)?.is_empty());
    assert!(
        (crate::ffmpeg_export::media_duration_seconds(&silent_plan.output).unwrap() - 4.0).abs()
            < 0.1
    );

    // Looping and non-looping short tracks, and a video without an original audio stream.
    let short = dir.join("short.wav");
    let mut w = hound::WavWriter::create(
        &short,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..24000 {
        w.write_sample(
            ((i as f64 * 660.0 * std::f64::consts::TAU / 48000.0).sin() * 10000.0) as i16,
        )
        .unwrap();
    }
    w.finalize().unwrap();
    for loop_track in [false, true] {
        for input in [&audio, &dir.join("colors.mp4")] {
            let mut p = plan(
                input,
                &dir.join(if input == &audio {
                    "mixed.m4a"
                } else {
                    "mixed.mp4"
                }),
            );
            p.track = Some(AddedTrack {
                path: short.clone(),
                original_volume: 0.0,
                new_volume: 1.0,
                loop_track,
            });
            export(&p, Arc::new(AtomicBool::new(false)), &mut |_| {})?;
            let source = audio_at(&p.output, 2.0);
            let energy: f32 = source.iter().map(|x| x.abs()).sum::<f32>() / source.len() as f32;
            if loop_track {
                assert!(energy > 0.05)
            } else {
                assert!(energy < 0.005)
            }
        }
    }
    // Exact non-keyframe cuts and tail removal.
    let mut p = plan(&av, &dir.join("fractional.mp4"));
    p.parts = vec![
        MediaPart {
            start: 0.3,
            end: 1.1,
            keep: true,
        },
        MediaPart {
            start: 4.2,
            end: 5.1,
            keep: true,
        },
    ];
    export(&p, Arc::new(AtomicBool::new(false)), &mut |_| {})?;
    assert!((crate::ffmpeg_export::media_duration_seconds(&p.output).unwrap() - 1.7).abs() < 0.1);
    let mut p = plan(&audio, &audio);
    assert!(export(&p, Arc::new(AtomicBool::new(false)), &mut |_| {}).is_err());
    p.output = dir.join("empty.m4a");
    for part in &mut p.parts {
        part.keep = false;
    }
    assert!(export(&p, Arc::new(AtomicBool::new(false)), &mut |_| {}).is_err());
    assert!(!p.output.exists());
    assert!(std::fs::read_dir(&dir).unwrap().all(|p| {
        !p.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".sonarpad-cut-")
    }));
    std::fs::remove_dir_all(dir).unwrap();
    Ok(())
}
fn audio_at(path: &Path, start: f64) -> Vec<f32> {
    FfmpegSource::try_new_at(path, start, None, None)
        .unwrap()
        .take(24000)
        .collect()
}

fn with_insert(input: &Path, output: &Path, inserted: &Path, length: f64) -> ExportPlan {
    let mut p = plan(input, output);
    p.sources = vec![crate::media_cutter_timeline::MediaSpan {
        path: input.into(),
        start: 0.0,
        end: 6.0,
        source_start: 0.0,
    }];
    crate::media_cutter_timeline::insert_before(
        &mut p.parts,
        &mut p.sources,
        2,
        inserted.into(),
        length,
    )
    .unwrap();
    p
}

fn inserted_tones(path: &Path) {
    let source = FfmpegSource::try_new_at(path, 0.0, None, None).unwrap();
    let rate = source.sample_rate() as usize;
    let channels = source.channels() as usize;
    let samples: Vec<_> = source.collect();
    assert!((samples.len() as f64 / channels as f64 / rate as f64 - 5.0).abs() < 0.12);
    for (seconds, expected) in [(0.5, 440.0), (2.25, 660.0), (3.5, 1320.0)] {
        let block: Vec<_> = samples
            .iter()
            .skip((seconds * rate as f64) as usize * channels)
            .step_by(channels)
            .take(rate / 2)
            .copied()
            .collect();
        let energy = |frequency: f64| {
            let (re, im) = block.iter().enumerate().fold((0.0, 0.0), |(r, m), (i, x)| {
                let a = i as f64 * frequency * std::f64::consts::TAU / rate as f64;
                (r + *x as f64 * a.cos(), m + *x as f64 * a.sin())
            });
            re.hypot(im)
        };
        assert!(
            energy(expected) > energy(880.0) * 15.0,
            "wrong source or deleted audio at {seconds}"
        );
    }
}

#[test]
fn inserted_media_real_dll_exports_audio_video_and_cancellation() -> Result<(), String> {
    let (dir, audio, av) = fixtures();
    let inserted = dir.join("insert è, ;.wav");
    let mut writer = hound::WavWriter::create(
        &inserted,
        hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..44100 {
        writer
            .write_sample(
                ((i as f64 * 660.0 * std::f64::consts::TAU / 44100.0).sin() * 12000.0) as i16,
            )
            .unwrap();
    }
    writer.finalize().unwrap();
    let mut p = with_insert(&audio, &dir.join("inserted.wav"), &inserted, 1.0);
    let mut progress = Vec::new();
    export(&p, Arc::new(AtomicBool::new(false)), &mut |v| {
        progress.push(v)
    })?;
    inserted_tones(&p.output);
    assert_eq!(progress.first(), Some(&0));
    assert_eq!(progress.last(), Some(&100));
    assert!(progress.windows(2).all(|v| v[0] < v[1]));
    p.parts[2].keep = false;
    p.output = dir.join("insert-deleted.wav");
    export(&p, Arc::new(AtomicBool::new(false)), &mut |_| {})?;
    tones(&p.output);
    p.parts[2].keep = true;
    p.output = inserted.clone();
    let before = std::fs::read(&inserted).unwrap();
    assert!(export(&p, Arc::new(AtomicBool::new(false)), &mut |_| {}).is_err());
    assert_eq!(std::fs::read(&inserted).unwrap(), before);

    // A video with audio, then an audio-only insertion: black frames in the inserted interval.
    let mut p = with_insert(&av, &dir.join("insert-black.mp4"), &inserted, 1.0);
    p.rotation = 90;
    export(&p, Arc::new(AtomicBool::new(false)), &mut |_| {})?;
    inserted_tones(&p.output);
    let frames = super::video::test_frames(&p.output)?;
    assert!(
        (124..=126).contains(&frames.len()),
        "{} frames",
        frames.len()
    );
    assert!(frames.iter().all(|f| (f.0, f.1) == (48, 64)));
    assert!(
        frames[52..72].iter().all(|f| f.2 < 20.0),
        "audio must have black video"
    );
    assert!(
        frames[80..100].iter().all(|f| f.2 > 30.0 && f.2 < 60.0),
        "original blue section missing"
    );
    let saved = std::fs::read(&p.output).unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    let result = export(&p, flag.clone(), &mut |v| {
        if v > 50 {
            flag.store(true, Ordering::Relaxed);
        }
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(&p.output).unwrap(), saved);

    // Promote an audio project to video by inserting an entire video (including its audio).
    let p = with_insert(&audio, &dir.join("insert-video.mp4"), &av, 6.0);
    export(&p, Arc::new(AtomicBool::new(false)), &mut |_| {})?;
    assert!((crate::ffmpeg_export::media_duration_seconds(&p.output).unwrap() - 10.0).abs() < 0.15);
    let frames = super::video::test_frames(&p.output)?;
    assert!((249..=251).contains(&frames.len()));
    assert!(frames[5..40].iter().all(|f| f.2 < 20.0));
    assert!(frames[110..140].iter().all(|f| f.2 > 130.0));
    assert!(frames[210..240].iter().all(|f| f.2 < 20.0));

    // Mixed video sources normalize dimensions, and silent clips remain silent.
    let different = dir.join("different-size.mp4");
    crate::media_conversion::encode_video_sized(
        &dir.join("colors.mp4"),
        &different,
        None,
        Some((96, 64)),
        &AtomicBool::new(false),
        &mut |_| {},
    )?;
    let p = with_insert(&av, &dir.join("insert-silent.mkv"), &different, 6.0);
    export(&p, Arc::new(AtomicBool::new(false)), &mut |_| {})?;
    assert!(audio_at(&p.output, 3.0).iter().all(|v| v.abs() < 0.005));
    assert!(!super::video::test_frames(&p.output)?.is_empty());
    assert!(std::fs::read_dir(&dir).unwrap().all(|p| {
        !p.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".sonarpad-cut-")
    }));
    std::fs::remove_dir_all(dir).unwrap();
    Ok(())
}

#[test]
#[ignore = "requires Sonarpad's installed MPV runtime"]
fn inserted_media_real_mpv_preview_matches_export() -> Result<(), String> {
    let (dir, audio, av) = fixtures();
    for (case, inserted) in [&audio, &av].into_iter().enumerate() {
        let output = dir.join(format!("preview-inserted-{case}.wav"));
        // Insert the same six-second file: A, inserted A/B/C, C, with original B deleted.
        let p = with_insert(&audio, &output, inserted, 6.0);
        let ranges = remaining_kept_ranges(&p.parts, 0.0);
        let edl = crate::media_cutter_timeline::playback_edl(&audio, &p.sources, &ranges)?;
        // Capture the actual playback output. MPV encoding mode pads PCM packets at edits.
        let result = std::process::Command::new(crate::installed_mpv_runtime_executable()?)
            .args([
                "--no-config",
                "--no-terminal",
                "--vo=null",
                "--ao=pcm",
                "--initial-audio-sync=no",
            ])
            .arg(format!("--ao-pcm-file={}", output.display()))
            .arg(edl)
            .output()
            .map_err(|e| e.to_string())?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            (crate::ffmpeg_export::media_duration_seconds(&output).unwrap() - 10.0).abs() < 0.1
        );
        let exported = dir.join(format!("exported-inserted-{case}.wav"));
        export(
            &ExportPlan {
                output: exported.clone(),
                ..p
            },
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )?;
        for at in [0.5, 2.5, 4.5, 6.5, 8.5] {
            let read = |path: &Path| {
                UniformSourceIterator::new(
                    FfmpegSource::try_new_at(path, at, None, None).unwrap(),
                    2,
                    48000,
                )
                .take(24000)
                .collect::<Vec<_>>()
            };
            let actual = read(&output);
            let expected = read(&exported);
            let error = actual
                .iter()
                .zip(expected.iter())
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>()
                / actual.len() as f32;
            assert!(error < 0.002, "preview/export differ at {at}: {error}");
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
    Ok(())
}
#[test]
fn editing_and_seek_plan() {
    let mut p = plan(Path::new("input"), Path::new("output")).parts;
    assert_eq!(kept_seek_position(&p, 1.0, 2.0), 5.0);
    assert_eq!(kept_seek_position(&p, 5.0, -2.0), 1.0);
    assert_eq!(remaining_kept_ranges(&p, 3.0)[0].start, 4.0);
    p[1].keep = true;
    assert_eq!(kept_duration(&p), 6.0);
    let deleted = delete_range(&mut p, 0.3, 1.2, 6.0);
    assert_eq!(deleted, vec![(0.3, 1.2)]);
    assert!((kept_duration(&p) - 5.1).abs() < 0.0001);
}

#[test]
#[ignore = "user-selected media; set SONARPAD_CUTTER_REAL_INPUTS JSON array"]
fn real_media_thirty_second_cuts() -> Result<(), String> {
    let files: Vec<PathBuf> = serde_json::from_str(
        &std::env::var("SONARPAD_CUTTER_REAL_INPUTS").map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let dir = std::env::temp_dir().join(format!("sonarpad-cutter-real-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
    for (i, input) in files.iter().enumerate() {
        let video = crate::ffmpeg_source::has_real_video_stream(input)?;
        let duration = crate::ffmpeg_export::media_duration_seconds(input).ok_or("No duration")?;
        assert!(duration >= 35.0);
        let output = dir.join(format!("output-{i}.{}", if video { "mp4" } else { "m4a" }));
        let mut p = plan(input, &output);
        p.parts = vec![
            MediaPart {
                start: 0.0,
                end: 10.0,
                keep: true,
            },
            MediaPart {
                start: 10.0,
                end: 15.0,
                keep: false,
            },
            MediaPart {
                start: 15.0,
                end: 35.0,
                keep: true,
            },
        ];
        let mut values = vec![];
        export(&p, Arc::new(AtomicBool::new(false)), &mut |v| {
            values.push(v)
        })?;
        let actual =
            crate::ffmpeg_export::media_duration_seconds(&output).ok_or("No output duration")?;
        assert!((actual - 30.0).abs() < 0.15);
        assert_eq!(values.last(), Some(&100));
        assert!(values.iter().any(|v| *v > 0 && *v < 100));
        let samples = audio_at(&output, 10.0);
        assert!(!samples.is_empty());
        if video {
            assert!(!super::video::test_frames(&output)?.is_empty());
        }
        println!(
            "Real media {i}: duration {actual:.3}, video {video}, progress {} updates",
            values.len()
        );
    }
    std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    Ok(())
}

#[test]
#[ignore = "requires Sonarpad's installed MPV runtime"]
fn real_mpv_preview_skips_deleted_audio_and_video() -> Result<(), String> {
    let (dir, audio, av) = fixtures();
    let mpv = crate::installed_mpv_runtime_executable()?;
    let rendered = dir.join("mpv-render.wav");
    let p = plan(&audio, &rendered);
    let ranges = remaining_kept_ranges(&p.parts, 0.0);
    let result = std::process::Command::new(mpv)
        .args([
            "--no-config",
            "--no-terminal",
            "--vid=no",
            "--of=wav",
            "--oac=pcm_s16le",
        ])
        .arg(format!("--o={}", rendered.display()))
        .arg(playback_edl(&audio, &ranges))
        .output()
        .map_err(|e| e.to_string())?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    tones(&rendered);

    let rendered_video = dir.join("mpv-render.mkv");
    let result = std::process::Command::new(crate::installed_mpv_runtime_executable()?)
        .args([
            "--no-config",
            "--no-terminal",
            "--ovc=ffv1",
            "--oac=pcm_s16le",
        ])
        .arg(format!("--o={}", rendered_video.display()))
        .arg(playback_edl(&av, &ranges))
        .output()
        .map_err(|e| e.to_string())?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let frames = super::video::test_frames(&rendered_video)?;
    assert!((99..=101).contains(&frames.len()));
    assert!(
        frames.iter().all(|f| f.2 < 110.0),
        "MPV played deleted video frames"
    );
    tones(&rendered_video);
    std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    Ok(())
}

#[test]
fn cutter_output_format_matrix() -> Result<(), String> {
    let (dir, audio, av) = fixtures();
    for input in [&audio, &av] {
        for ext in AUDIO_EXTENSIONS {
            let output = dir.join(format!("audio-output.{ext}"));
            export(
                &plan(input, &output),
                Arc::new(AtomicBool::new(false)),
                &mut |_| {},
            )
            .map_err(|e| format!("Audio format {ext}: {e}"))?;
            assert!(!crate::ffmpeg_source::has_real_video_stream(&output)?);
            tones(&output);
        }
    }
    for ext in VIDEO_EXTENSIONS {
        let output = dir.join(format!("video-output.{ext}"));
        export(
            &plan(&av, &output),
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .map_err(|e| format!("Video format {ext}: {e}"))?;
        assert!(crate::ffmpeg_source::has_real_video_stream(&output)?);
        tones(&output);
        let frames = video::test_frames(&output)?;
        assert!(!frames.is_empty());
        assert!(
            frames.iter().all(|f| f.2 < 110.0),
            "deleted green frames leaked in {ext}"
        );
    }
    std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    Ok(())
}
