//! Source-time editing and MPV playback plans, shared by preview and DLL export.
use std::path::Path;
pub const MIN_PART_SECONDS: f64 = 0.01;
#[derive(Clone, Debug, PartialEq)]
pub struct MediaPart {
    pub start: f64,
    pub end: f64,
    pub keep: bool,
}
impl MediaPart {
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}
pub fn remaining_kept_ranges(parts: &[MediaPart], position: f64) -> Vec<MediaPart> {
    parts
        .iter()
        .filter(|p| p.keep && p.duration() > 0.000001 && p.end > position)
        .map(|p| MediaPart {
            start: p.start.max(position),
            end: p.end,
            keep: true,
        })
        .collect()
}

pub fn kept_position(parts: &[MediaPart], elapsed: f64) -> f64 {
    let mut remaining = elapsed.max(0.0);
    let mut end = 0.0;
    for part in parts.iter().filter(|p| p.keep && p.duration() > 0.0) {
        end = part.end;
        if remaining < part.duration() {
            return part.start + remaining;
        }
        remaining -= part.duration();
    }
    end
}

pub fn kept_seek_position(parts: &[MediaPart], position: f64, delta: f64) -> f64 {
    let offset: f64 = parts
        .iter()
        .filter(|p| p.keep && p.duration() > 0.000001)
        .map(|p| (position - p.start).clamp(0.0, p.duration()))
        .sum();
    kept_position(parts, offset + delta)
}

pub fn playback_edl(path: &Path, ranges: &[MediaPart]) -> String {
    let name = path.to_string_lossy();
    let segments: Vec<String> = ranges
        .iter()
        .map(|p| {
            format!(
                "%{}%{},{:.6},{:.6}",
                name.len(),
                name,
                p.start,
                p.duration()
            )
        })
        .collect();
    format!("edl://{}", segments.join(";"))
}

pub fn clamp_parts(parts: &mut Vec<MediaPart>, duration: f64) {
    parts.retain(|p| p.end - p.start > 0.000001);
    for p in parts.iter_mut() {
        p.start = p.start.clamp(0.0, duration);
        p.end = p.end.clamp(0.0, duration);
    }
    parts.sort_by(|a, b| a.start.total_cmp(&b.start));
}

pub fn split_part_at(parts: &mut Vec<MediaPart>, point: f64) -> Option<usize> {
    for i in 0..parts.len() {
        if parts[i].keep && point > parts[i].start + 0.000001 && point < parts[i].end - 0.000001 {
            let original = parts[i].clone();
            parts[i].end = point;
            parts.insert(
                i + 1,
                MediaPart {
                    start: point,
                    end: original.end,
                    keep: original.keep,
                },
            );
            return Some(i + 1);
        }
    }
    None
}

pub fn delete_range(
    parts: &mut Vec<MediaPart>,
    start: f64,
    end: f64,
    duration: f64,
) -> Vec<(f64, f64)> {
    if end - start < MIN_PART_SECONDS {
        return Vec::new();
    }
    if split_part_at(parts, start).is_none() {
        crate::log_debug("Media cutter: cut start requires no additional split");
    }
    if split_part_at(parts, end).is_none() {
        crate::log_debug("Media cutter: cut end requires no additional split");
    }
    let mut deleted = Vec::new();
    for part in parts.iter_mut() {
        if part.end > start && part.start < end && part.keep {
            part.keep = false;
            deleted.push((part.start, part.end));
        }
    }
    clamp_parts(parts, duration);
    deleted
}

pub fn adjust_part_boundary(
    parts: &mut [MediaPart],
    index: usize,
    move_start: bool,
    delta: f64,
    duration: f64,
) -> bool {
    if index >= parts.len() {
        return false;
    }
    if move_start {
        let min = if index == 0 {
            0.0
        } else {
            parts[index - 1].start + MIN_PART_SECONDS
        };
        let max = parts[index].end - MIN_PART_SECONDS;
        let new_boundary = (parts[index].start + delta)
            .clamp(min, max)
            .clamp(0.0, duration);
        if (new_boundary - parts[index].start).abs() < 0.0001 {
            return false;
        }
        if index > 0 {
            parts[index - 1].end = new_boundary;
        }
        parts[index].start = new_boundary;
        true
    } else {
        let min = parts[index].start + MIN_PART_SECONDS;
        let max = if index + 1 >= parts.len() {
            duration
        } else {
            parts[index + 1].end - MIN_PART_SECONDS
        };
        let new_boundary = (parts[index].end + delta)
            .clamp(min, max)
            .clamp(0.0, duration);
        if (new_boundary - parts[index].end).abs() < 0.0001 {
            return false;
        }
        parts[index].end = new_boundary;
        if index + 1 < parts.len() {
            parts[index + 1].start = new_boundary;
        }
        true
    }
}

pub fn output_time(parts: &[MediaPart], source_time: f64) -> Option<f64> {
    let mut offset = 0.0;
    for p in parts.iter().filter(|p| p.keep) {
        if source_time >= p.start && source_time < p.end {
            return Some(offset + source_time - p.start);
        }
        offset += p.duration();
    }
    None
}
pub fn kept_duration(parts: &[MediaPart]) -> f64 {
    parts
        .iter()
        .filter(|p| p.keep)
        .map(MediaPart::duration)
        .sum()
}
