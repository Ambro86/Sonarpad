//! A continuous editing clock mapped onto one or more original media files.
use crate::media_cutter_model::MediaPart;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaSpan {
    pub path: PathBuf,
    pub start: f64,
    pub end: f64,
    pub source_start: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SourceClip {
    pub path: PathBuf,
    pub start: f64,
    pub end: f64,
}

pub fn insert_before(
    parts: &mut Vec<MediaPart>,
    spans: &mut Vec<MediaSpan>,
    index: usize,
    path: PathBuf,
    length: f64,
) -> Result<f64, String> {
    if !length.is_finite() || length < crate::media_cutter_model::MIN_PART_SECONDS {
        return Err("Invalid inserted media duration".into());
    }
    let at = parts.get(index).ok_or("Select a part")?.start;
    let duration = spans.last().ok_or("Missing source media")?.end;
    validate(spans, duration)?;
    if at < 0.0 || at > duration || !(duration + length).is_finite() {
        return Err("Invalid insertion position".into());
    }
    let mut result = Vec::new();
    for span in spans.iter() {
        if span.start < at && span.end > at {
            result.push(MediaSpan {
                end: at,
                ..span.clone()
            });
            result.push(MediaSpan {
                start: at + length,
                end: span.end + length,
                source_start: span.source_start + at - span.start,
                path: span.path.clone(),
            });
        } else if span.start >= at {
            result.push(MediaSpan {
                start: span.start + length,
                end: span.end + length,
                ..span.clone()
            });
        } else {
            result.push(span.clone());
        }
    }
    result.push(MediaSpan {
        path,
        start: at,
        end: at + length,
        source_start: 0.0,
    });
    result.sort_by(|a, b| a.start.total_cmp(&b.start));
    validate(&result, duration + length)?;
    for part in parts.iter_mut().skip(index) {
        part.start += length;
        part.end += length;
    }
    parts.insert(
        index,
        MediaPart {
            start: at,
            end: at + length,
            keep: true,
        },
    );
    *spans = result;
    Ok(duration + length)
}

pub fn validate(spans: &[MediaSpan], duration: f64) -> Result<(), String> {
    let mut end = 0.0;
    for span in spans {
        if !span.start.is_finite()
            || !span.end.is_finite()
            || !span.source_start.is_finite()
            || (span.start - end).abs() > 0.000001
            || span.end <= span.start
            || span.source_start < 0.0
        {
            return Err("Invalid media timeline".into());
        }
        end = span.end;
    }
    if spans.is_empty() || !duration.is_finite() || (end - duration).abs() > 0.000001 {
        return Err("Incomplete media timeline".into());
    }
    Ok(())
}

pub fn resolve(spans: &[MediaSpan], ranges: &[MediaPart]) -> Result<Vec<SourceClip>, String> {
    let mut clips = Vec::new();
    for part in ranges.iter().filter(|p| p.keep) {
        let mut covered = 0.0;
        for span in spans {
            let start = part.start.max(span.start);
            let end = part.end.min(span.end);
            if end > start {
                clips.push(SourceClip {
                    path: span.path.clone(),
                    start: span.source_start + start - span.start,
                    end: span.source_start + end - span.start,
                });
                covered += end - start;
            }
        }
        if !part.start.is_finite()
            || !part.end.is_finite()
            || part.end <= part.start
            || (covered - part.duration()).abs() > 0.000001
        {
            return Err("Selected part is outside the media timeline".into());
        }
    }
    Ok(clips)
}

pub fn playback_edl(
    input: &Path,
    spans: &[MediaSpan],
    ranges: &[MediaPart],
) -> Result<String, String> {
    if spans.is_empty() {
        return Ok(crate::media_cutter_model::playback_edl(input, ranges));
    }
    let clips = resolve(spans, ranges)?;
    let segments: Vec<_> = clips
        .iter()
        .map(|clip| {
            let name = clip.path.to_string_lossy();
            format!(
                "%{}%{},{:.6},{:.6}",
                name.len(),
                name,
                clip.start,
                clip.end - clip.start
            )
        })
        .collect();
    Ok(format!("edl://{}", segments.join(";")))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn project() -> (Vec<MediaPart>, Vec<MediaSpan>) {
        (
            (0..5)
                .map(|i| MediaPart {
                    start: i as f64 * 2.0,
                    end: (i + 1) as f64 * 2.0,
                    keep: true,
                })
                .collect(),
            vec![MediaSpan {
                path: "original.wav".into(),
                start: 0.0,
                end: 10.0,
                source_start: 0.0,
            }],
        )
    }
    #[test]
    fn insertion_before_fifth_preserves_original_and_deleted_parts() {
        let (mut parts, mut spans) = project();
        parts[1].keep = false;
        assert_eq!(
            insert_before(&mut parts, &mut spans, 4, "insert.wav".into(), 3.0).unwrap(),
            13.0
        );
        assert_eq!((parts[4].start, parts[4].end), (8.0, 11.0));
        assert_eq!((parts[5].start, parts[5].end), (11.0, 13.0));
        assert!(!parts[1].keep);
        let clips = resolve(&spans, &parts).unwrap();
        assert_eq!(clips[3].path, PathBuf::from("insert.wav"));
        assert_eq!((clips[4].start, clips[4].end), (8.0, 10.0));
        assert_eq!(crate::media_cutter_model::kept_duration(&parts), 11.0);
    }
    #[test]
    fn nested_insertion_split_delete_restore_and_seek() {
        let (mut parts, mut spans) = project();
        insert_before(&mut parts, &mut spans, 0, "first.wav".into(), 4.0).unwrap();
        let index = crate::media_cutter_model::split_part_at(&mut parts, 2.0).unwrap();
        insert_before(&mut parts, &mut spans, index, "second.mp4".into(), 1.0).unwrap();
        let clips = resolve(&spans, &parts[..3]).unwrap();
        assert_eq!(
            clips.iter().map(|c| (c.start, c.end)).collect::<Vec<_>>(),
            [(0.0, 2.0), (0.0, 1.0), (2.0, 4.0)]
        );
        parts[1].keep = false;
        assert_eq!(
            crate::media_cutter_model::kept_seek_position(&parts, 1.5, 1.0),
            3.5
        );
        assert!(
            resolve(&spans, &parts)
                .unwrap()
                .iter()
                .all(|c| c.path != Path::new("second.mp4"))
        );
        parts[1].keep = true;
        assert_eq!(
            resolve(&spans, &parts).unwrap()[1].path,
            PathBuf::from("second.mp4")
        );
    }
    #[test]
    fn bad_insert_is_atomic_and_edl_escapes_unicode_names() {
        let (mut parts, mut spans) = project();
        let before = (parts.clone(), spans.clone());
        assert!(insert_before(&mut parts, &mut spans, 5, "bad".into(), 1.0).is_err());
        assert!(insert_before(&mut parts, &mut spans, 0, "bad".into(), f64::NAN).is_err());
        assert_eq!((parts.clone(), spans.clone()), before);
        insert_before(&mut parts, &mut spans, 0, "è,a;b.wav".into(), 1.0).unwrap();
        assert!(
            playback_edl(Path::new("original.wav"), &spans, &parts[..1])
                .unwrap()
                .starts_with("edl://%10%è,a;b.wav,0.000000,1.000000")
        );
    }
}
