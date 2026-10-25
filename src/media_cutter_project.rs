//! Non-destructive Sonarpad media editing projects and safe export replacement.
use crate::media_cutter_model::{MIN_PART_SECONDS, MediaPart};
use crate::media_cutter_timeline::{self, MediaSpan};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const PROJECT_VERSION: u32 = 1;
const MAX_PROJECT_BYTES: u64 = 4 * 1024 * 1024;
static UNIQUE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTrack {
    pub path: PathBuf,
    pub original_volume: f64,
    pub new_volume: f64,
    pub loop_track: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutterProject {
    pub version: u32,
    pub input: PathBuf,
    pub duration: f64,
    pub has_video: bool,
    pub parts: Vec<MediaPart>,
    pub sources: Vec<MediaSpan>,
    pub position: f64,
    pub rotation: usize,
    pub track: Option<ProjectTrack>,
    pub mode: usize,
    pub movement: usize,
    pub goldwave_keys: bool,
}

impl CutterProject {
    pub fn new(input: PathBuf, duration: f64, has_video: bool, parts: Vec<MediaPart>) -> Self {
        Self {
            version: PROJECT_VERSION,
            input,
            duration,
            has_video,
            parts,
            sources: Vec::new(),
            position: 0.0,
            rotation: 0,
            track: None,
            mode: 0,
            movement: 0,
            goldwave_keys: false,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != PROJECT_VERSION {
            return Err("Unsupported Sonarpad project version".into());
        }
        if !self.duration.is_finite()
            || self.duration < MIN_PART_SECONDS
            || !self.position.is_finite()
            || !(0.0..=self.duration).contains(&self.position)
            || self.rotation >= 4
            || self.mode >= 2
            || self.movement >= 5
            || self.parts.is_empty()
            || self.parts.len() > 100_000
        {
            return Err("Invalid media project values".into());
        }
        if !self.input.is_file() {
            return Err(format!(
                "Original media file missing: {}",
                self.input.display()
            ));
        }
        // Advanced editing may trim the beginning or end of a part, leaving gaps.
        // The export engine accepts ordered non-overlapping parts, not necessarily a
        // contiguous partition of the original file.
        let mut previous = 0.0;
        for p in &self.parts {
            if !p.start.is_finite()
                || !p.end.is_finite()
                || p.start < 0.0
                || p.start < previous - 0.000001
                || p.end - p.start < 0.000001
                || p.end > self.duration + 0.002
            {
                return Err("Invalid media project segments".into());
            }
            previous = p.end;
        }
        if !self.sources.is_empty() {
            media_cutter_timeline::validate(&self.sources, self.duration)?;
            for span in &self.sources {
                if !span.path.is_file() {
                    return Err(format!(
                        "Project media file missing: {}",
                        span.path.display()
                    ));
                }
            }
        }
        if let Some(track) = &self.track
            && (!track.path.is_file()
                || !track.original_volume.is_finite()
                || !track.new_volume.is_finite()
                || !(0.0..=200.0).contains(&track.original_volume)
                || !(0.0..=200.0).contains(&track.new_volume))
        {
            return Err("Invalid or missing additional audio track".into());
        }
        Ok(())
    }
}

pub fn save(path: &Path, project: &CutterProject) -> Result<(), String> {
    project.validate()?;
    if !path
        .extension()
        .is_some_and(|v| v.to_string_lossy().eq_ignore_ascii_case("sonarcut"))
    {
        return Err("A Sonarpad project must have the .sonarcut extension".into());
    }
    let source_paths = std::iter::once(&project.input)
        .chain(project.sources.iter().map(|span| &span.path))
        .chain(project.track.iter().map(|track| &track.path));
    if source_paths
        .into_iter()
        .any(|source| crate::media_conversion::same_file(source, path))
    {
        return Err("A project must not overwrite any source media file".into());
    }
    let json = serde_json::to_vec_pretty(project).map_err(|e| e.to_string())?;
    if json.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Project exceeds the supported size".into());
    }
    let temporary = unused_sibling(path, "project-tmp")?;
    fs::write(&temporary, json).map_err(|e| format!("Cannot write project: {e}"))?;
    let result = if path.exists() {
        backup_and_replace(path, &temporary).map(|_| ())
    } else {
        fs::rename(&temporary, path).map_err(|e| format!("Cannot save project: {e}"))
    };
    if result.is_err()
        && let Err(error) = fs::remove_file(&temporary)
    {
        crate::log_debug(&format!("Failed to remove temporary cutter file: {error}"));
    }
    result
}

pub fn load(path: &Path) -> Result<CutterProject, String> {
    if fs::metadata(path)
        .map_err(|e| format!("Cannot open project: {e}"))?
        .len()
        > MAX_PROJECT_BYTES
    {
        return Err("Project file is too large".into());
    }
    let bytes = fs::read(path).map_err(|e| format!("Cannot read project: {e}"))?;
    let project: CutterProject =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid Sonarpad project: {e}"))?;
    project.validate()?;
    Ok(project)
}

fn unused_sibling(path: &Path, tag: &str) -> Result<PathBuf, String> {
    let name = path
        .file_stem()
        .ok_or("Missing file name")?
        .to_string_lossy();
    let ext = path
        .extension()
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_default();
    for _ in 0..100 {
        let seq = UNIQUE.fetch_add(1, Ordering::Relaxed);
        let candidate = path.with_file_name(if ext.is_empty() {
            format!("{name}.{tag}-{}-{seq}", std::process::id())
        } else {
            format!("{name}.{tag}-{}-{seq}.{ext}", std::process::id())
        });
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("Could not find an unused temporary filename".into())
}

pub fn backup_and_replace(destination: &Path, temporary: &Path) -> Result<PathBuf, String> {
    if !destination.is_file() || !temporary.is_file() {
        return Err("Missing destination or completed export".into());
    }
    let backup = unused_sibling(destination, "sonarpad-backup")?;
    fs::rename(destination, &backup)
        .map_err(|e| format!("Could not create original-file backup: {e}"))?;
    if let Err(err) = fs::rename(temporary, destination) {
        let restore = fs::rename(&backup, destination);
        return Err(match restore {
            Ok(()) => format!("Could not install exported file; original restored: {err}"),
            Err(restore_err) => format!(
                "Could not install exported file: {err}. CRITICAL: original remains in backup {}: {restore_err}",
                backup.display()
            ),
        });
    }
    Ok(backup)
}

pub fn export_with_safe_replacement(
    plan: &crate::media_cutter_export::ExportPlan,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    progress: &mut dyn FnMut(u32),
) -> Result<Option<PathBuf>, String> {
    export_with_replacement_policy(plan, cancel, progress, false)
}

/// Explicitly requested in-place editing: encode beside the original, validate,
/// and preserve the entire previous file as a backup before replacing it.
pub fn export_overwriting_original(
    plan: &crate::media_cutter_export::ExportPlan,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    progress: &mut dyn FnMut(u32),
) -> Result<Option<PathBuf>, String> {
    if !crate::media_conversion::same_file(&plan.input, &plan.output)
        || !plan.sources.is_empty()
        || !plan.input.is_file()
    {
        return Err("Original replacement requires the unmodified source timeline".into());
    }
    let extension = plan
        .input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !crate::media_cutter_export::AUDIO_EXTENSIONS.contains(&extension.as_str())
        && !crate::media_cutter_export::VIDEO_EXTENSIONS.contains(&extension.as_str())
    {
        return Err("The original format does not support safe in-place export".into());
    }
    export_with_replacement_policy(plan, cancel, progress, true)
}

fn export_with_replacement_policy(
    plan: &crate::media_cutter_export::ExportPlan,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    progress: &mut dyn FnMut(u32),
    allow_original: bool,
) -> Result<Option<PathBuf>, String> {
    let is_original = crate::media_conversion::same_file(&plan.input, &plan.output);
    if (is_original && !allow_original)
        || plan
            .sources
            .iter()
            .any(|span| crate::media_conversion::same_file(&span.path, &plan.output))
        || plan
            .track
            .as_ref()
            .is_some_and(|track| crate::media_conversion::same_file(&track.path, &plan.output))
    {
        return Err("Cannot overwrite source media during export".into());
    }
    if !plan.output.exists() {
        if allow_original {
            return Err("Original file is no longer available; replacement cancelled".into());
        }
        crate::media_cutter_export::export(plan, cancel, progress)?;
        return Ok(None);
    }
    let target = plan.output.clone();
    let temporary = unused_sibling(&target, "sonarpad-export-tmp")?;
    let mut safe_plan = plan.clone();
    safe_plan.output = temporary.clone();
    let result = crate::media_cutter_export::export(&safe_plan, cancel.clone(), progress);
    if let Err(error) = result {
        if let Err(error) = fs::remove_file(&temporary) {
            crate::log_debug(&format!("Failed to remove temporary cutter file: {error}"));
        }
        return Err(error);
    }
    if cancel.load(Ordering::Relaxed) {
        if let Err(error) = fs::remove_file(&temporary) {
            crate::log_debug(&format!("Failed to remove temporary cutter file: {error}"));
        }
        return Err("Export cancelled before replacing the original".into());
    }
    match backup_and_replace(&target, &temporary) {
        Ok(backup) => Ok(Some(backup)),
        Err(error) => {
            if let Err(error) = fs::remove_file(&temporary) {
                crate::log_debug(&format!("Failed to remove temporary cutter file: {error}"));
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sonarpad-project-test-{}-{}",
            std::process::id(),
            UNIQUE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
    #[test]
    fn projects_roundtrip_and_reject_invalid_timelines() {
        let dir = temp_dir();
        let media = dir.join("recording.mp3");
        fs::write(&media, b"dummy fixture").unwrap();
        let mut project = CutterProject::new(
            media,
            30.0,
            false,
            vec![
                MediaPart {
                    start: 0.0,
                    end: 10.0,
                    keep: true,
                },
                MediaPart {
                    start: 10.0,
                    end: 20.0,
                    keep: false,
                },
                MediaPart {
                    start: 20.0,
                    end: 30.0,
                    keep: true,
                },
            ],
        );
        project.goldwave_keys = true;
        let file = dir.join("podcast.sonarcut");
        save(&file, &project).unwrap();
        let loaded = load(&file).unwrap();
        assert_eq!(loaded.parts.len(), 3);
        assert!(!loaded.parts[1].keep);
        assert!(loaded.goldwave_keys);
        // A gap is valid after adjusting an advanced cut boundary.
        project.parts[1].start = 12.0;
        save(&file, &project).unwrap();
        assert_eq!(load(&file).unwrap().parts[1].start, 12.0);
        // Overlapping parts, however, must be rejected.
        project.parts[1].start = 9.0;
        assert!(save(&file, &project).is_err());
        assert_eq!(load(&file).unwrap().parts[1].start, 12.0);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn backup_preserves_original_and_installs_new_file() {
        let dir = temp_dir();
        let original = dir.join("episode.mp3");
        let edited = dir.join("staging.mp3");
        fs::write(&original, b"original audio").unwrap();
        fs::write(&edited, b"new audio").unwrap();
        let backup = backup_and_replace(&original, &edited).unwrap();
        assert_eq!(fs::read(&original).unwrap(), b"new audio");
        assert_eq!(fs::read(&backup).unwrap(), b"original audio");
        assert!(!edited.exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
