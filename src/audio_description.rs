use crate::ffmpeg_export::{
    AudioDescriptionExportOptions, AudioDescriptionMixCue, export_audio_description_mp3,
};
use crate::settings::{
    AudiobookPartAnnouncementMode, AudiobookPartNamingMode, DictionaryEntry, Language, TtsEngine,
};
use crate::tools::audio_description_bridge::{
    AudioDescriptionBridgeCallbacks, AudioDescriptionBridgeCheckpoint,
    AudioDescriptionBridgeRequest, AudioDescriptionBridgeResume, AudioDescriptionOverloadDecision,
    AudioDescriptionPreparedChunk, AudioDescriptionQuotaDecision, BridgeCharacter,
    BridgeDescription, BridgeInterval, run_audio_description_bridge,
};
use crate::tts_engine::{
    AudiobookCommonOptions, MixedAudiobookConfig, TtsChunk, audiobook_synthesis_parallelism,
    render_mixed_audiobook_part, split_into_tts_chunks,
};
use rodio::Source;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::HWND;

const MAX_SHIFT_SEC: f64 = 5.0;
const MIN_EXTENDED_ANCHOR_SEC: f64 = 1.0;
const EDGE_TRAILING_MIN_REMOVE_MS: u64 = 60;
const EDGE_TRAILING_KEEP_MS: u64 = 30;
const EDGE_TRAILING_SEEK_MS: u64 = 5;
const EDGE_TRAILING_WINDOW_MS: u64 = 60;
const PYANNOTE_SAMPLE_RATE: u32 = 16_000;
const GEMINI_CHUNK_SECONDS: u32 = 180;
const GEMINI_MAX_CHUNK_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const GEMINI_INLINE_TARGET_CHUNK_BYTES: u64 = 40 * 1024 * 1024;
const GEMINI_MIN_SEGMENT_SECONDS: u32 = 30;
const GEMINI_SEGMENT_RETRY_LIMIT: usize = 5;
const AUDIO_DESCRIPTION_DUCKING_DB: f32 = -12.0;
const AUDIO_DESCRIPTION_FADE_MS: u32 = 280;
const AUDIO_DESCRIPTION_PRE_DUCK_MS: u32 = 180;
const AUDIO_DESCRIPTION_RELEASE_MS: u32 = 600;
const AUDIO_DESCRIPTION_BITRATE_KBPS: u32 = 192;
const MAX_CHARACTER_DESCRIPTION_CHARS: usize = 2_000;
const AUDIO_DESCRIPTION_PARTIAL_FORMAT: &str = "sonarpad-audio-description-partial";
const AUDIO_DESCRIPTION_PARTIAL_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioDescriptionVerbosity {
    Brief,
    Standard,
    Detailed,
}

impl AudioDescriptionVerbosity {
    pub fn as_bridge_value(self) -> &'static str {
        match self {
            Self::Brief => "short",
            Self::Standard => "standard",
            Self::Detailed => "detailed",
        }
    }

    fn from_bridge_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "short" => Self::Brief,
            "standard" => Self::Standard,
            _ => Self::Detailed,
        }
    }
}

#[derive(Clone)]
pub struct AudioDescriptionJob {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub audio_stream_index: Option<i32>,
    pub language_code: String,
    pub tts_language: Language,
    pub verbosity: AudioDescriptionVerbosity,
    pub allow_extended_pauses: bool,
    pub recognize_characters: bool,
    pub recognize_screen_text: bool,
    pub character_catalog: Option<AudioDescriptionCharacterCatalogContext>,
    pub save_project: bool,
    pub tts_engine: TtsEngine,
    pub tts_voice: String,
    pub tts_rate: i32,
    pub tts_pitch: i32,
    pub tts_volume: i32,
    pub dictionary: Vec<DictionaryEntry>,
    pub gemini_api_key: String,
    pub sonarpad_ai_service_url: String,
    pub sonarpad_ai_access_code: String,
    pub sonarpad_ai_device_id: String,
    pub gemini_model: String,
    pub audiobook_bitrate_kbps: u32,
    pub resume_checkpoint_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct AudioDescriptionCharacterCatalogContext {
    pub name: String,
    pub path: PathBuf,
    pub characters: Vec<BridgeCharacter>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDescriptionCharacterCatalogSummary {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AudioDescriptionPartialCatalog {
    name: String,
    path: PathBuf,
    #[serde(default)]
    characters: Vec<BridgeCharacter>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AudioDescriptionPartialCheckpoint {
    format: String,
    version: u32,
    source_path: PathBuf,
    output_mp3_path: PathBuf,
    #[serde(default)]
    audio_stream_index: Option<i32>,
    source_file_size: u64,
    source_duration_sec: f64,
    language: Language,
    language_code: String,
    verbosity: String,
    allow_extended_pauses: bool,
    recognize_characters: bool,
    #[serde(default)]
    recognize_screen_text: bool,
    save_project: bool,
    tts_engine: TtsEngine,
    tts_voice: String,
    tts_rate: i32,
    tts_pitch: i32,
    tts_volume: i32,
    #[serde(default)]
    dictionary: Vec<DictionaryEntry>,
    gemini_model: String,
    audiobook_bitrate_kbps: u32,
    character_catalog: Option<AudioDescriptionPartialCatalog>,
    completed_chunks: usize,
    total_chunks: usize,
    #[serde(default)]
    descriptions: Vec<BridgeDescription>,
    #[serde(default)]
    character_glossary: Vec<BridgeCharacter>,
}

#[derive(Clone, Debug)]
pub struct AudioDescriptionResumeSettings {
    pub checkpoint_path: PathBuf,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub description_language: Language,
    pub verbosity: AudioDescriptionVerbosity,
    pub allow_extended_pauses: bool,
    pub recognize_characters: bool,
    pub recognize_screen_text: bool,
    pub save_project: bool,
    pub tts_engine: TtsEngine,
    pub tts_voice: String,
    pub gemini_model: String,
    pub completed_chunks: usize,
    pub total_chunks: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AudioDescriptionCharacterCatalogFile {
    format: String,
    version: u32,
    name: String,
    created_at_utc: String,
    updated_at_utc: String,
    #[serde(default)]
    characters: Vec<BridgeCharacter>,
}

fn normalized_catalog_character(character: &BridgeCharacter) -> Option<BridgeCharacter> {
    let name = character
        .name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let description = character
        .description
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if name.is_empty() || description.is_empty() {
        return None;
    }
    Some(BridgeCharacter {
        id: character.id.trim().to_string(),
        name,
        description,
    })
}

fn catalog_name_tokens(name: &str) -> Vec<String> {
    name.split_whitespace()
        .map(|token| {
            token
                .chars()
                .filter(|character| character.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        })
        .filter(|token| !token.is_empty())
        .collect()
}

fn find_catalog_identity(
    characters: &[BridgeCharacter],
    candidate: &BridgeCharacter,
) -> Option<usize> {
    let candidate_id = candidate.id.trim().to_lowercase();
    if !candidate_id.is_empty() {
        let mut matches = characters
            .iter()
            .enumerate()
            .filter(|(_, character)| character.id.trim().eq_ignore_ascii_case(&candidate.id))
            .map(|(index, _)| index);
        if let Some(first) = matches.next()
            && matches.next().is_none()
        {
            return Some(first);
        }
    }

    let mut name_matches = characters
        .iter()
        .enumerate()
        .filter(|(_, character)| character.name.trim().eq_ignore_ascii_case(&candidate.name))
        .map(|(index, _)| index);
    if let Some(first) = name_matches.next()
        && name_matches.next().is_none()
    {
        return Some(first);
    }

    let candidate_tokens = catalog_name_tokens(&candidate.name);
    if candidate_id.is_empty() || candidate_tokens.len() != 1 || candidate_tokens[0].len() < 3 {
        return None;
    }
    let candidate_token = &candidate_tokens[0];
    let id_prefix = format!("{candidate_id}_");
    let alias_matches = characters
        .iter()
        .enumerate()
        .filter(|(_, character)| {
            character.id.to_lowercase().starts_with(&id_prefix)
                && catalog_name_tokens(&character.name).contains(candidate_token)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match alias_matches.as_slice() {
        [index] => Some(*index),
        _ => None,
    }
}

fn catalog_description_tokens(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .map(|token| token.to_lowercase())
        .filter(|token| !token.is_empty())
        .fold(Vec::<String>::new(), |mut tokens, token| {
            if !tokens.contains(&token) {
                tokens.push(token);
            }
            tokens
        })
}

fn catalog_description_coverage(candidate: &str, established: &str) -> f32 {
    let candidate_tokens = catalog_description_tokens(candidate);
    if candidate_tokens.is_empty() {
        return 1.0;
    }
    let established_tokens = catalog_description_tokens(established);
    if established_tokens.is_empty() {
        return 0.0;
    }
    let shared = candidate_tokens
        .iter()
        .filter(|token| established_tokens.contains(token))
        .count();
    shared as f32 / candidate_tokens.len() as f32
}

fn catalog_description_sentences(text: &str) -> Vec<String> {
    text.split_inclusive(['.', '!', '?'])
        .map(str::trim)
        .filter(|sentence| !sentence.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn merge_catalog_description(existing: &str, observed: &str) -> String {
    let existing = existing.trim();
    let observed = observed.trim();
    if existing.is_empty() {
        return observed.to_string();
    }
    if observed.is_empty() {
        return existing.to_string();
    }

    let mut merged = existing.to_string();
    for sentence in catalog_description_sentences(observed) {
        let words = catalog_description_tokens(&sentence);
        if words.len() <= 2 {
            continue;
        }

        // The saved catalog is authoritative. Gemini often restates the same
        // biography with different punctuation, apostrophes, or one corrupted
        // word (for example "Padre di Dio" instead of "Padre di Flo"). If most
        // of the candidate sentence is already represented by the established
        // description, treat it as a paraphrase/corruption rather than new data.
        if catalog_description_coverage(&sentence, &merged) >= 0.65 {
            continue;
        }

        let separator = if matches!(merged.chars().last(), Some('.' | '!' | '?')) {
            " "
        } else {
            ". "
        };
        let candidate = format!("{merged}{separator}{sentence}");
        if candidate.chars().count() > MAX_CHARACTER_DESCRIPTION_CHARS {
            break;
        }
        merged = candidate;
    }
    merged
}

fn merge_catalog_characters(
    established: &[BridgeCharacter],
    detected: &[BridgeCharacter],
) -> Vec<BridgeCharacter> {
    let mut merged = Vec::<BridgeCharacter>::new();
    for character in established {
        let Some(candidate) = normalized_catalog_character(character) else {
            continue;
        };
        if let Some(index) = find_catalog_identity(&merged, &candidate) {
            let description =
                merge_catalog_description(&merged[index].description, &candidate.description);
            merged[index].description = description;
            if merged[index].id.is_empty() && !candidate.id.is_empty() {
                merged[index].id = candidate.id;
            }
        } else {
            merged.push(candidate);
        }
    }

    let authoritative_count = merged.len();
    for character in detected {
        let Some(candidate) = normalized_catalog_character(character) else {
            continue;
        };
        if let Some(index) = find_catalog_identity(&merged, &candidate) {
            let description =
                merge_catalog_description(&merged[index].description, &candidate.description);
            merged[index].description = description;
            if index >= authoritative_count
                && merged[index].id.is_empty()
                && !candidate.id.is_empty()
            {
                merged[index].id = candidate.id;
            }
        } else {
            merged.push(candidate);
        }
    }
    merged
}

fn normalize_catalog_characters(characters: &[BridgeCharacter]) -> Vec<BridgeCharacter> {
    merge_catalog_characters(&[], characters)
}

pub fn audio_description_character_catalog_dir(save_folder: &str) -> PathBuf {
    PathBuf::from(save_folder).join("Catalogs")
}

fn safe_catalog_file_stem(name: &str) -> String {
    let mut result = String::new();
    for character in name.trim().chars() {
        if matches!(
            character,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
        ) || character.is_control()
        {
            result.push('_');
        } else {
            result.push(character);
        }
    }
    let result = result
        .trim()
        .trim_end_matches(['.', ' '])
        .trim()
        .to_string();
    if result.is_empty() {
        "characters".to_string()
    } else {
        result.chars().take(120).collect()
    }
}

pub fn audio_description_character_catalog_path(save_folder: &str, name: &str) -> PathBuf {
    audio_description_character_catalog_dir(save_folder)
        .join(format!("{}.json", safe_catalog_file_stem(name)))
}

pub fn list_audio_description_character_catalogs(
    save_folder: &str,
) -> Vec<AudioDescriptionCharacterCatalogSummary> {
    let directory = audio_description_character_catalog_dir(save_folder);
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut catalogs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file()
            || !path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("json"))
        {
            continue;
        }
        let name = load_audio_description_character_catalog(&path)
            .map(|catalog| catalog.name)
            .unwrap_or_else(|_| {
                path.file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("Catalog")
                    .to_string()
            });
        catalogs.push(AudioDescriptionCharacterCatalogSummary { name, path });
    }
    catalogs.sort_by_key(|catalog| catalog.name.to_lowercase());
    catalogs
}

fn load_audio_description_character_catalog(
    path: &Path,
) -> Result<AudioDescriptionCharacterCatalogFile, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Audio description: could not read character catalog: {error}"))?;
    let mut catalog: AudioDescriptionCharacterCatalogFile = serde_json::from_str(&raw)
        .map_err(|error| format!("Audio description: invalid character catalog: {error}"))?;
    if catalog.format != "sonarpad-character-catalog" || catalog.version == 0 {
        return Err("Audio description: unsupported character catalog format".to_string());
    }
    catalog.name = catalog.name.trim().to_string();
    catalog.characters = normalize_catalog_characters(&catalog.characters);
    Ok(catalog)
}

pub fn load_audio_description_character_catalog_context(
    name: String,
    path: PathBuf,
) -> Result<AudioDescriptionCharacterCatalogContext, String> {
    if !path.exists() {
        return Ok(AudioDescriptionCharacterCatalogContext {
            name,
            path,
            characters: Vec::new(),
        });
    }
    let catalog = load_audio_description_character_catalog(&path)?;
    Ok(AudioDescriptionCharacterCatalogContext {
        name: if catalog.name.is_empty() {
            name
        } else {
            catalog.name
        },
        path,
        characters: catalog.characters,
    })
}

fn save_audio_description_character_catalog(
    context: &AudioDescriptionCharacterCatalogContext,
    characters: &[BridgeCharacter],
) -> Result<(), String> {
    let characters = merge_catalog_characters(&context.characters, characters);
    let parent = context.path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| {
        format!("Audio description: could not create character catalog folder: {error}")
    })?;
    let existing_created = load_audio_description_character_catalog(&context.path)
        .ok()
        .map(|catalog| catalog.created_at_utc)
        .filter(|value| !value.trim().is_empty());
    let now = chrono::Utc::now().to_rfc3339();
    let catalog = AudioDescriptionCharacterCatalogFile {
        format: "sonarpad-character-catalog".to_string(),
        version: 1,
        name: context.name.trim().to_string(),
        created_at_utc: existing_created.unwrap_or_else(|| now.clone()),
        updated_at_utc: now,
        characters,
    };
    let temporary = temporary_sibling_path(&context.path, "new");
    let raw = serde_json::to_vec_pretty(&catalog).map_err(|error| {
        format!("Audio description: character catalog serialization failed: {error}")
    })?;
    fs::write(&temporary, raw)
        .map_err(|error| format!("Audio description: character catalog write failed: {error}"))?;
    if context.path.exists() {
        fs::remove_file(&context.path).map_err(|error| {
            crate::log_if_err!(
                fs::remove_file(&temporary),
                "Audio description cleanup operation failed"
            );
            format!("Audio description: character catalog replacement failed: {error}")
        })?;
    }
    fs::rename(&temporary, &context.path).map_err(|error| {
        crate::log_if_err!(
            fs::remove_file(&temporary),
            "Audio description cleanup operation failed"
        );
        format!("Audio description: character catalog commit failed: {error}")
    })
}

pub type AudioDescriptionStatusCallback = Box<dyn FnMut(&str, &str) + Send>;
pub type AudioDescriptionProgressCallback = Box<dyn FnMut(u32) + Send>;
pub type AudioDescriptionQuotaCallback =
    Box<dyn FnMut(&str, &str) -> AudioDescriptionQuotaDecision + Send>;
pub type AudioDescriptionOverloadCallback =
    Box<dyn FnMut(&str, &str) -> AudioDescriptionOverloadDecision + Send>;

pub struct AudioDescriptionCallbacks {
    pub status: Option<AudioDescriptionStatusCallback>,
    pub progress: Option<AudioDescriptionProgressCallback>,
    pub quota: Option<AudioDescriptionQuotaCallback>,
    pub overload: Option<AudioDescriptionOverloadCallback>,
}

#[derive(Clone, Debug)]
pub struct AudioDescriptionOutcome {
    pub output_path: PathBuf,
    pub project_path: Option<PathBuf>,
    pub project_warning: Option<String>,
    pub character_catalog_path: Option<PathBuf>,
    pub character_catalog_warning: Option<String>,
    pub generated_descriptions: usize,
    pub normal_descriptions: usize,
    pub extended_pauses: usize,
    pub dropped_after_tts: usize,
}

#[derive(Clone)]
struct SynthesizedDescription {
    original_index: usize,
    text: String,
    desired_start_sec: f64,
    visual_start_sec: f64,
    visual_evidence_time_sec: Option<f64>,
    mandatory: bool,
    slot_start_sec: Option<f64>,
    slot_end_sec: Option<f64>,
    samples: Arc<[f32]>,
    sample_rate: u32,
    channels: u16,
}

#[derive(Clone)]
struct AudioDescriptionSynthesisTask {
    synthesis_index: usize,
    original_index: usize,
    text: String,
    desired_start_sec: f64,
    visual_start_sec: f64,
    visual_evidence_time_sec: Option<f64>,
    mandatory: bool,
    slot_start_sec: Option<f64>,
    slot_end_sec: Option<f64>,
}

#[derive(Clone)]
struct ScheduledDescription {
    original_index: usize,
    text: String,
    desired_start_sec: f64,
    visual_evidence_time_sec: Option<f64>,
    start_sec: f64,
    samples: Arc<[f32]>,
    sample_rate: u32,
    channels: u16,
    extended_pause: bool,
}

#[derive(Clone, Debug)]
struct DroppedDescription {
    original_index: usize,
    text: String,
    desired_start_sec: f64,
    tts_duration_sec: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioDescriptionProjectInterval {
    pub start_sec: f64,
    pub end_sec: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioDescriptionProjectDescription {
    pub id: usize,
    pub text: String,
    pub original_text: String,
    #[serde(default)]
    pub rendered_text: String,
    pub modified: bool,
    pub gemini_start_sec: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_evidence_time_sec: Option<f64>,
    pub source_start_sec: f64,
    pub output_start_sec: f64,
    pub output_end_sec: f64,
    pub tts_duration_sec: f64,
    pub extended_pause: bool,
    pub extended_pause_duration_sec: f64,
    pub duck_start_sec: Option<f64>,
    pub duck_end_sec: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioDescriptionProjectExcluded {
    pub id: usize,
    pub text: String,
    pub gemini_start_sec: f64,
    pub tts_duration_sec: f64,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioDescriptionProject {
    pub format: String,
    pub version: u32,
    pub created_at_utc: String,
    pub updated_at_utc: String,
    pub source_path: PathBuf,
    pub output_mp3_path: PathBuf,
    #[serde(default)]
    pub audio_stream_index: Option<i32>,
    pub source_duration_sec: f64,
    pub output_duration_sec: f64,
    pub language: Language,
    pub language_code: String,
    pub verbosity: String,
    pub allow_extended_pauses: bool,
    #[serde(default = "default_true")]
    pub recognize_characters: bool,
    pub gemini_model: String,
    pub tts_engine: TtsEngine,
    pub tts_voice: String,
    pub tts_rate: i32,
    pub tts_pitch: i32,
    pub tts_volume: i32,
    #[serde(default)]
    pub dictionary: Vec<DictionaryEntry>,
    pub bitrate_kbps: u32,
    pub ducking_db: f32,
    pub fade_ms: u32,
    pub protected_intervals: Vec<AudioDescriptionProjectInterval>,
    pub descriptions: Vec<AudioDescriptionProjectDescription>,
    pub excluded_descriptions: Vec<AudioDescriptionProjectExcluded>,
}

#[derive(Clone, Debug)]
pub struct AudioDescriptionProjectEditOutcome {
    pub project: AudioDescriptionProject,
    pub applied_count: usize,
}

#[derive(Debug)]
pub struct AudioDescriptionProjectBatchEditError {
    pub index: Option<usize>,
    pub error: AudioDescriptionProjectEditError,
}

#[derive(Debug)]
pub struct AudioDescriptionProjectPreviewAudio {
    path: PathBuf,
    cache_dir: PathBuf,
    duration_sec: f64,
}

impl AudioDescriptionProjectPreviewAudio {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn duration_sec(&self) -> f64 {
        self.duration_sec
    }
}

impl Drop for AudioDescriptionProjectPreviewAudio {
    fn drop(&mut self) {
        crate::log_if_err!(
            fs::remove_dir_all(&self.cache_dir),
            "Audio description cleanup operation failed"
        );
    }
}

#[derive(Debug)]
pub enum AudioDescriptionProjectEditError {
    Cancelled,
    TooLong {
        available_sec: f64,
        synthesized_sec: f64,
    },
    Other(String),
}

impl std::fmt::Display for AudioDescriptionProjectEditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("cancelled"),
            Self::TooLong {
                available_sec,
                synthesized_sec,
            } => write!(
                formatter,
                "Audio description: synthesized description is too long ({synthesized_sec:.3}s; available {available_sec:.3}s)"
            ),
            Self::Other(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for AudioDescriptionProjectEditError {}

#[derive(Clone, Debug)]
pub struct AudioDescriptionProjectVoiceSettings {
    pub engine: TtsEngine,
    pub voice: String,
    pub rate: i32,
    pub volume: i32,
}

#[derive(Debug)]
pub enum AudioDescriptionProjectVoiceError {
    Cancelled,
    DoesNotFit {
        source_start_sec: f64,
        synthesized_sec: f64,
    },
    Other(String),
}

impl std::fmt::Display for AudioDescriptionProjectVoiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("cancelled"),
            Self::DoesNotFit {
                source_start_sec,
                synthesized_sec,
            } => write!(
                formatter,
                "Audio description: selected voice does not fit near {source_start_sec:.3}s ({synthesized_sec:.3}s)"
            ),
            Self::Other(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for AudioDescriptionProjectVoiceError {}

fn default_true() -> bool {
    true
}

fn notify_status(callbacks: &mut AudioDescriptionCallbacks, stage: &str, message: &str) {
    if let Some(callback) = callbacks.status.as_mut() {
        callback(stage, message);
    }
}

fn notify_progress(callbacks: &mut AudioDescriptionCallbacks, progress: u32) {
    if let Some(callback) = callbacks.progress.as_mut() {
        callback(progress.min(100));
    }
}

pub fn language_code(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::English => "en",
        Language::German => "de",
        Language::Spanish => "es",
        Language::Portuguese => "pt",
        Language::PortugueseBrazilian => "pt-BR",
        Language::Swedish => "sv",
        Language::Vietnamese => "vi",
        Language::Czech => "cs",
        Language::Polish => "pl",
        Language::French => "fr",
        Language::Serbian => "sr",
        Language::Ukrainian => "uk",
        Language::Lithuanian => "lt",
        Language::Russian => "ru",
        Language::Chinese => "zh",
        Language::Hindi => "hi",
    }
}

fn temporary_job_dir() -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = crate::settings::settings_dir()
        .join("audio_description_cache")
        .join(format!("{}_{}", std::process::id(), stamp));
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Audio description: create cache failed: {error}"))?;
    Ok(dir)
}

fn write_pyannote_wav(
    input_path: &Path,
    output_path: &Path,
    preferred_audio_stream_index: Option<i32>,
    cancel: &Arc<AtomicBool>,
) -> Result<(), String> {
    let mut source = crate::ffmpeg_source::FfmpegSource::try_new(
        input_path,
        0,
        None,
        preferred_audio_stream_index,
    )
    .map_err(|error| format!("Audio description: FFmpeg audio decode failed: {error}"))?;
    let input_rate = source.sample_rate().max(1);
    let input_channels = source.channels().max(1) as usize;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: PYANNOTE_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(output_path, spec)
        .map_err(|error| format!("Audio description: create Pyannote WAV failed: {error}"))?;
    let output_step = input_rate as f64 / PYANNOTE_SAMPLE_RATE as f64;
    let mut next_output_position = 0.0_f64;
    let mut frame_index = 0_u64;
    let mut channel_count = 0_usize;
    let mut frame_sum = 0.0_f32;
    let mut previous_mono: Option<f32> = None;
    let mut written = 0_u64;

    for sample in source.by_ref() {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        frame_sum += sample;
        channel_count += 1;
        if channel_count < input_channels {
            continue;
        }

        let mono = (frame_sum / input_channels as f32).clamp(-1.0, 1.0);
        let current_position = frame_index as f64;
        if let Some(previous) = previous_mono {
            let previous_position = current_position - 1.0;
            while next_output_position <= current_position + f64::EPSILON {
                if next_output_position >= previous_position {
                    let fraction = (next_output_position - previous_position).clamp(0.0, 1.0);
                    let interpolated = previous + (mono - previous) * fraction as f32;
                    let pcm = (interpolated * i16::MAX as f32)
                        .round()
                        .clamp(i16::MIN as f32, i16::MAX as f32)
                        as i16;
                    writer.write_sample(pcm).map_err(|error| {
                        format!("Audio description: write Pyannote WAV failed: {error}")
                    })?;
                    written = written.saturating_add(1);
                }
                next_output_position += output_step;
            }
        } else {
            let pcm = (mono * i16::MAX as f32)
                .round()
                .clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            writer.write_sample(pcm).map_err(|error| {
                format!("Audio description: write Pyannote WAV failed: {error}")
            })?;
            written = written.saturating_add(1);
            next_output_position = output_step;
        }
        previous_mono = Some(mono);
        frame_index = frame_index.saturating_add(1);
        channel_count = 0;
        frame_sum = 0.0;
    }

    writer
        .finalize()
        .map_err(|error| format!("Audio description: finalize Pyannote WAV failed: {error}"))?;
    if written == 0 {
        crate::log_if_err!(
            fs::remove_file(output_path),
            "Audio description cleanup operation failed"
        );
        return Err("Audio description: decoded soundtrack is empty".to_string());
    }
    Ok(())
}

fn normalize_audio_description_source_duration(
    path: &Path,
    measured_duration_sec: f64,
    format_start_sec: f64,
) -> f64 {
    if !measured_duration_sec.is_finite() || measured_duration_sec <= 0.0 {
        return measured_duration_sec;
    }
    let is_matroska = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("mkv") || extension.eq_ignore_ascii_case("webm")
        });
    if !is_matroska
        || !format_start_sec.is_finite()
        || format_start_sec <= GEMINI_CHUNK_SECONDS as f64
        || measured_duration_sec <= format_start_sec
    {
        return measured_duration_sec;
    }

    // Matroska/WebM can report format duration as the absolute end timestamp
    // when the source starts far from zero. Sonarpad needs the local media span
    // for its analysis timeline, not that container clock value.
    let local_span_sec = measured_duration_sec - format_start_sec;
    if local_span_sec > 0.001 {
        local_span_sec
    } else {
        measured_duration_sec
    }
}

fn normalize_prepared_gemini_chunk_duration(
    measured_duration_sec: f64,
    format_start_sec: f64,
    segment_seconds: u32,
) -> f64 {
    let expected_sec = segment_seconds.max(1) as f64;
    if !measured_duration_sec.is_finite() || measured_duration_sec <= 0.0 {
        return expected_sec;
    }

    // Some Matroska chunks produced from sources with a very large non-zero
    // start_time expose AVFormatContext.duration as an absolute end timestamp
    // instead of a local duration. Only normalize that unmistakable case;
    // ordinary media, including files with small offsets, keeps the historical
    // duration path unchanged.
    if format_start_sec.is_finite()
        && format_start_sec > expected_sec * 4.0
        && measured_duration_sec > format_start_sec
    {
        let local_span_sec = measured_duration_sec - format_start_sec;
        if local_span_sec > 0.001 && local_span_sec <= expected_sec * 4.0 {
            return local_span_sec;
        }
    }

    measured_duration_sec
}

fn build_gemini_chunk_timeline(
    measured_chunks: &[(PathBuf, f64)],
    duration_sec: f64,
    reconcile_small_drift: bool,
) -> Option<Vec<AudioDescriptionPreparedChunk>> {
    if measured_chunks.is_empty() || !duration_sec.is_finite() || duration_sec <= 0.0 {
        return None;
    }

    let scale = if reconcile_small_drift {
        let measured_total = measured_chunks
            .iter()
            .map(|(_, measured)| *measured)
            .sum::<f64>();
        if !measured_total.is_finite() || measured_total <= duration_sec {
            return None;
        }
        let excess_ratio = (measured_total - duration_sec) / duration_sec;
        if !excess_ratio.is_finite() || excess_ratio <= 0.0 || excess_ratio > 0.02 {
            return None;
        }
        duration_sec / measured_total
    } else {
        1.0
    };

    let mut chunks = Vec::with_capacity(measured_chunks.len());
    let mut cursor = 0.0_f64;
    let chunk_count = measured_chunks.len();
    for (index, (path, measured)) in measured_chunks.iter().enumerate() {
        if !measured.is_finite() || *measured <= 0.0 {
            return None;
        }
        let start_sec = cursor;
        let end_sec = if index + 1 == chunk_count {
            duration_sec
        } else {
            (start_sec + measured * scale).min(duration_sec)
        };
        if !end_sec.is_finite() || end_sec <= start_sec {
            return None;
        }
        chunks.push(AudioDescriptionPreparedChunk {
            path: path.to_string_lossy().to_string(),
            start_sec,
            end_sec,
        });
        if index + 1 < chunk_count {
            cursor = end_sec;
        }
    }
    Some(chunks)
}

fn clear_prepared_gemini_chunks(cache_dir: &Path) -> Result<(), String> {
    for entry in fs::read_dir(cache_dir)
        .map_err(|error| format!("Audio description: read chunk folder failed: {error}"))?
    {
        let path = entry
            .map_err(|error| format!("Audio description: read chunk entry failed: {error}"))?
            .path();
        let is_prepared_chunk = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("gemini_chunk_") && name.ends_with(".mkv"));
        if is_prepared_chunk {
            fs::remove_file(&path).map_err(|error| {
                format!(
                    "Audio description: remove stale Gemini chunk {} failed: {error}",
                    path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn prepare_gemini_chunks(
    input_path: &Path,
    duration_sec: f64,
    cache_dir: &Path,
    preferred_audio_stream_index: Option<i32>,
    cancel: &Arc<AtomicBool>,
) -> Result<Vec<AudioDescriptionPreparedChunk>, String> {
    let input_size = fs::metadata(input_path)
        .map_err(|error| format!("Audio description: read media metadata failed: {error}"))?
        .len();
    if input_size == 0 {
        return Err(format!(
            "Audio description: Gemini input has unsupported size: {}",
            input_path.display()
        ));
    }
    if preferred_audio_stream_index.is_none()
        && duration_sec <= GEMINI_CHUNK_SECONDS as f64
        && input_size <= GEMINI_INLINE_TARGET_CHUNK_BYTES
    {
        return Ok(vec![AudioDescriptionPreparedChunk {
            path: input_path.to_string_lossy().to_string(),
            start_sec: 0.0,
            end_sec: duration_sec,
        }]);
    }

    let output_pattern = cache_dir.join("gemini_chunk_%04d.mkv");
    let mut segment_seconds = duration_sec.ceil().clamp(
        GEMINI_MIN_SEGMENT_SECONDS as f64,
        GEMINI_CHUNK_SECONDS as f64,
    ) as u32;
    let mut attempt = 1usize;
    let paths = loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        clear_prepared_gemini_chunks(cache_dir)?;

        let primary_segment_result = crate::ffmpeg_export::segment_media_file_for_analysis(
            input_path,
            &output_pattern,
            segment_seconds,
            1,
            preferred_audio_stream_index,
            None,
        );
        if let Err(primary_error) = primary_segment_result {
            if !primary_error.starts_with("FFmpeg: failed to write segment header:") {
                return Err(format!(
                    "Audio description: FFmpeg chunk preparation failed: {primary_error}"
                ));
            }

            crate::log_debug(&format!(
                "Audio description: Gemini chunk header failed with selected audio; retrying video-only analysis chunks. primary_error={primary_error}"
            ));
            clear_prepared_gemini_chunks(cache_dir)?;
            crate::ffmpeg_export::segment_media_file_for_analysis_video_only(
                input_path,
                &output_pattern,
                segment_seconds,
                1,
                None,
            )
            .map_err(|fallback_error| {
                format!(
                    "Audio description: FFmpeg chunk preparation failed: {primary_error}; video-only fallback failed: {fallback_error}"
                )
            })?;
            crate::log_debug(
                "Audio description: video-only Gemini chunk fallback succeeded; dialogue/silence analysis remains unchanged.",
            );
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }

        let mut prepared_paths = fs::read_dir(cache_dir)
            .map_err(|error| format!("Audio description: read chunk folder failed: {error}"))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("gemini_chunk_") && name.ends_with(".mkv"))
            })
            .collect::<Vec<_>>();
        prepared_paths.sort();
        if prepared_paths.is_empty() {
            return Err("Audio description: FFmpeg produced no Gemini chunks".to_string());
        }

        let max_chunk_bytes = prepared_paths
            .iter()
            .map(|path| {
                fs::metadata(path)
                    .map(|metadata| metadata.len())
                    .map_err(|error| {
                        format!("Audio description: read chunk metadata failed: {error}")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .max()
            .unwrap_or(0);
        if max_chunk_bytes == 0 {
            return Err("Audio description: FFmpeg produced an empty Gemini chunk".to_string());
        }
        if max_chunk_bytes <= GEMINI_INLINE_TARGET_CHUNK_BYTES {
            crate::log_debug(&format!(
                "Audio description: adaptive Gemini chunks ready segment_seconds={} max_chunk_mb={:.1}",
                segment_seconds,
                max_chunk_bytes as f64 / (1024.0 * 1024.0)
            ));
            break prepared_paths;
        }

        if attempt >= GEMINI_SEGMENT_RETRY_LIMIT || segment_seconds <= GEMINI_MIN_SEGMENT_SECONDS {
            crate::log_debug(&format!(
                "Audio description: adaptive Gemini chunk fallback segment_seconds={} max_chunk_mb={:.1}; Files API may be used",
                segment_seconds,
                max_chunk_bytes as f64 / (1024.0 * 1024.0)
            ));
            break prepared_paths;
        }

        let ratio = GEMINI_INLINE_TARGET_CHUNK_BYTES as f64 / max_chunk_bytes as f64;
        let proposed = (segment_seconds as f64 * ratio * 0.82).floor() as u32;
        let next_segment_seconds = proposed.max(GEMINI_MIN_SEGMENT_SECONDS).min(
            segment_seconds
                .saturating_sub(1)
                .max(GEMINI_MIN_SEGMENT_SECONDS),
        );
        crate::log_debug(&format!(
            "Audio description: adaptive Gemini chunk retry segment_seconds={} next_segment_seconds={} max_chunk_mb={:.1}",
            segment_seconds,
            next_segment_seconds,
            max_chunk_bytes as f64 / (1024.0 * 1024.0)
        ));
        segment_seconds = next_segment_seconds;
        attempt = attempt.saturating_add(1);
    };

    let path_count = paths.len();
    let mut measured_chunks = Vec::with_capacity(path_count);
    for path in paths {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        let file_size = fs::metadata(&path)
            .map_err(|error| format!("Audio description: read chunk metadata failed: {error}"))?
            .len();
        if file_size == 0 || file_size >= GEMINI_MAX_CHUNK_BYTES {
            return Err(format!(
                "Audio description: Gemini chunk has unsupported size: {}",
                path.display()
            ));
        }
        let (raw_measured, format_start_sec) =
            crate::ffmpeg_export::media_duration_and_start_seconds(&path)
                .unwrap_or((segment_seconds as f64, 0.0));
        let measured = normalize_prepared_gemini_chunk_duration(
            raw_measured,
            format_start_sec,
            segment_seconds,
        )
        .max(0.001);
        if (measured - raw_measured).abs() > 0.001 {
            crate::log_debug(&format!(
                "Audio description: normalized Gemini chunk duration {} raw={:.3}s start_time={:.3}s local={:.3}s",
                path.display(),
                raw_measured,
                format_start_sec,
                measured
            ));
        }
        measured_chunks.push((path, measured));
    }

    if let Some(chunks) = build_gemini_chunk_timeline(&measured_chunks, duration_sec, false) {
        return Ok(chunks);
    }

    // Keep the historical timeline untouched for every source where it is valid.
    // Some remuxed/segmented media report a small per-chunk duration overhead; over
    // many chunks that metadata drift can make the cursor reach the source duration
    // before the final file. Only in that already-invalid case, and only when the
    // total drift is small, proportionally reconcile measured chunk durations to the
    // known source duration. Large discrepancies remain hard failures rather than
    // being hidden by a fallback.
    let measured_total = measured_chunks
        .iter()
        .map(|(_, measured)| *measured)
        .sum::<f64>();
    let excess_ratio = if duration_sec > 0.0 {
        (measured_total - duration_sec) / duration_sec
    } else {
        f64::INFINITY
    };
    if measured_total.is_finite()
        && measured_total > duration_sec
        && excess_ratio.is_finite()
        && excess_ratio > 0.0
        && excess_ratio <= 0.02
    {
        crate::log_debug(&format!(
            "Audio description: Gemini chunk timeline fallback measured_total={:.3}s source_duration={:.3}s excess={:.3}% chunks={}",
            measured_total,
            duration_sec,
            excess_ratio * 100.0,
            path_count
        ));
        if let Some(chunks) = build_gemini_chunk_timeline(&measured_chunks, duration_sec, true) {
            return Ok(chunks);
        }
    }

    Err("Audio description: invalid Gemini chunk timeline".to_string())
}

fn read_wav_as_f32(path: &Path) -> Result<(Vec<f32>, u32, u16), String> {
    let reader = hound::WavReader::open(path)
        .map_err(|error| format!("Audio description: open synthesized WAV failed: {error}"))?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate.max(1);
    let channels = spec.channels.max(1);
    let mut samples = Vec::new();
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for sample in reader.into_samples::<f32>() {
                samples.push(
                    sample
                        .map_err(|error| format!("Audio description: WAV sample failed: {error}"))?
                        .clamp(-1.0, 1.0),
                );
            }
        }
        hound::SampleFormat::Int if spec.bits_per_sample <= 16 => {
            let reader = hound::WavReader::open(path).map_err(|error| {
                format!("Audio description: reopen synthesized WAV failed: {error}")
            })?;
            for sample in reader.into_samples::<i16>() {
                samples.push(
                    sample
                        .map_err(|error| format!("Audio description: WAV sample failed: {error}"))?
                        as f32
                        / 32768.0,
                );
            }
        }
        hound::SampleFormat::Int => {
            let reader = hound::WavReader::open(path).map_err(|error| {
                format!("Audio description: reopen synthesized WAV failed: {error}")
            })?;
            let denominator = ((1_i64 << (spec.bits_per_sample - 1)) - 1).max(1) as f32;
            for sample in reader.into_samples::<i32>() {
                samples.push(
                    sample
                        .map_err(|error| format!("Audio description: WAV sample failed: {error}"))?
                        as f32
                        / denominator,
                );
            }
        }
    }
    if samples.is_empty() {
        return Err("Audio description: synthesized audio is empty".to_string());
    }
    Ok((samples, sample_rate, channels))
}

fn rms_dbfs(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -120.0;
    }
    let sum = samples
        .iter()
        .map(|sample| {
            let value = *sample as f64;
            value * value
        })
        .sum::<f64>();
    let rms = (sum / samples.len() as f64).sqrt();
    if rms <= 1.0e-9 {
        -120.0
    } else {
        (20.0 * rms.log10()) as f32
    }
}

/// Reproduces Omni's Edge cleanup rule on decoded PCM: threshold is the louder
/// of -55 dBFS and average-35 dB, scan in 5 ms steps using a 60 ms window,
/// preserve 30 ms after the last non-silent window, and trim only at least 60 ms.
fn trim_edge_trailing_silence(samples: &mut Vec<f32>, sample_rate: u32, channels: u16) -> usize {
    if samples.is_empty() {
        return 0;
    }
    let channels = channels.max(1) as usize;
    let frames = samples.len() / channels;
    let minimum_input_frames = ((sample_rate as u64 * 100) / 1000).max(1) as usize;
    if frames < minimum_input_frames {
        return 0;
    }

    let threshold_db = (-55.0_f32).max(rms_dbfs(samples) - 35.0);
    let seek_frames = ((sample_rate as u64 * EDGE_TRAILING_SEEK_MS) / 1000).max(1) as usize;
    let window_frames = ((sample_rate as u64 * EDGE_TRAILING_WINDOW_MS) / 1000).max(1) as usize;
    if frames < window_frames {
        return 0;
    }
    let keep_frames = ((sample_rate as u64 * EDGE_TRAILING_KEEP_MS) / 1000) as usize;
    let minimum_remove_frames =
        ((sample_rate as u64 * EDGE_TRAILING_MIN_REMOVE_MS) / 1000).max(1) as usize;

    // Match pydub.detect_nonsilent(min_silence_len=60, seek_step=5):
    // collect every silent 60 ms window, merge overlapping windows, then take
    // the end of the final non-silent range and retain another 30 ms.
    let last_slice_start = frames.saturating_sub(window_frames);
    let mut slice_starts: Vec<usize> = (0..=last_slice_start).step_by(seek_frames).collect();
    if slice_starts.last().copied() != Some(last_slice_start) {
        slice_starts.push(last_slice_start);
    }
    let mut silent_starts = Vec::new();
    for start in slice_starts {
        let end = start.saturating_add(window_frames).min(frames);
        let start_sample = start.saturating_mul(channels);
        let end_sample = end.saturating_mul(channels).min(samples.len());
        if rms_dbfs(&samples[start_sample..end_sample]) <= threshold_db {
            silent_starts.push(start);
        }
    }
    let Some(mut previous) = silent_starts.first().copied() else {
        return 0;
    };
    let mut current_start = previous;
    let mut silent_ranges = Vec::new();
    for start in silent_starts.into_iter().skip(1) {
        let continuous = start == previous.saturating_add(seek_frames);
        let has_gap = start > previous.saturating_add(window_frames);
        if !continuous && has_gap {
            silent_ranges.push((
                current_start,
                previous.saturating_add(window_frames).min(frames),
            ));
            current_start = start;
        }
        previous = start;
    }
    silent_ranges.push((
        current_start,
        previous.saturating_add(window_frames).min(frames),
    ));

    if silent_ranges.len() == 1 && silent_ranges[0] == (0, frames) {
        return 0;
    }
    let mut previous_silence_end = 0_usize;
    let mut last_nonsilent_end = None;
    for (silence_start, silence_end) in &silent_ranges {
        if *silence_start > previous_silence_end {
            last_nonsilent_end = Some(*silence_start);
        }
        previous_silence_end = previous_silence_end.max(*silence_end);
    }
    if previous_silence_end < frames {
        last_nonsilent_end = Some(frames);
    }
    let Some(last_active_frame) = last_nonsilent_end else {
        return 0;
    };

    let keep_until = last_active_frame.saturating_add(keep_frames).min(frames);
    let removable_frames = frames.saturating_sub(keep_until);
    if removable_frames < minimum_remove_frames {
        return 0;
    }
    let old_len = samples.len();
    samples.truncate(keep_until.saturating_mul(channels));
    old_len.saturating_sub(samples.len())
}

fn audio_description_tts_chunks(text: &str, job: &AudioDescriptionJob) -> Vec<TtsChunk> {
    split_into_tts_chunks(text, false, &job.dictionary, job.tts_engine)
}

fn audio_description_samples_have_signal(samples: &[f32]) -> bool {
    samples
        .iter()
        .any(|sample| sample.is_finite() && sample.abs() > 0.00001)
}

fn audio_description_tts_error_is_empty_output(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("decoded audio contains no samples")
        || normalized.contains("audio contains no samples")
        || normalized.contains("empty wav")
        || normalized.contains("zero samples")
}

fn wait_for_empty_tts_retry(cancel: &AtomicBool) -> Result<(), String> {
    const RETRY_DELAY: Duration = Duration::from_millis(750);
    const POLL_DELAY: Duration = Duration::from_millis(75);
    let mut waited = Duration::ZERO;
    while waited < RETRY_DELAY {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        let remaining = RETRY_DELAY.saturating_sub(waited);
        let sleep_for = remaining.min(POLL_DELAY);
        std::thread::sleep(sleep_for);
        waited = waited.saturating_add(sleep_for);
    }
    Ok(())
}

fn synthesize_description(
    text: &str,
    index: usize,
    job: &AudioDescriptionJob,
    cache_dir: &Path,
    cancel: Arc<AtomicBool>,
) -> Result<(Arc<[f32]>, u32, u16), String> {
    let output = cache_dir.join(format!("description_{index:05}.wav"));
    let chunks = audio_description_tts_chunks(text, job);
    if chunks.is_empty() {
        return Err(format!(
            "Audio description: TTS cue {} is empty after dictionary/normalization",
            index
        ));
    }
    let options = AudiobookCommonOptions {
        voice: &job.tts_voice,
        output: &output,
        progress_hwnd: HWND(0),
        cancel: cancel.clone(),
        language: job.tts_language,
        part_naming_mode: AudiobookPartNamingMode::TitleNumber,
        part_announcement_mode: AudiobookPartAnnouncementMode::None,
        audiobook_title: "",
        audiobook_bitrate_kbps: job.audiobook_bitrate_kbps,
        rate: job.tts_rate,
        pitch: job.tts_pitch,
        volume: job.tts_volume,
        sapi4_threads: None,
    };
    let config = MixedAudiobookConfig {
        main_engine: job.tts_engine,
    };
    let mut empty_attempt = 0_u64;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        if output.exists() {
            crate::log_if_err!(
                fs::remove_file(&output),
                "Audio description stale TTS output cleanup failed"
            );
        }

        let mut progress = 0_usize;
        if let Err(error) =
            render_mixed_audiobook_part(&chunks, &mut progress, &output, &options, &config)
        {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".to_string());
            }
            if audio_description_tts_error_is_empty_output(&error) {
                empty_attempt = empty_attempt.saturating_add(1);
                crate::log_debug(&format!(
                    "Audio description: cue {} TTS renderer reported empty audio, retrying indefinitely; empty_attempt={} error={}",
                    index, empty_attempt, error
                ));
                crate::log_if_err!(
                    fs::remove_file(&output),
                    "Audio description empty TTS output cleanup failed"
                );
                wait_for_empty_tts_retry(cancel.as_ref())?;
                continue;
            }
            return Err(error);
        }
        if !output.is_file() {
            empty_attempt = empty_attempt.saturating_add(1);
            crate::log_debug(&format!(
                "Audio description: cue {} TTS returned success without an output WAV, retrying indefinitely; empty_attempt={} path={}",
                index,
                empty_attempt,
                output.display()
            ));
            wait_for_empty_tts_retry(cancel.as_ref())?;
            continue;
        }

        let output_len = fs::metadata(&output)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if output_len <= 44 {
            empty_attempt = empty_attempt.saturating_add(1);
            crate::log_debug(&format!(
                "Audio description: cue {} produced an empty WAV ({} bytes), retrying indefinitely; empty_attempt={}",
                index, output_len, empty_attempt
            ));
            crate::log_if_err!(
                fs::remove_file(&output),
                "Audio description empty TTS output cleanup failed"
            );
            wait_for_empty_tts_retry(cancel.as_ref())?;
            continue;
        }

        let (mut samples, sample_rate, channels) = match read_wav_as_f32(&output) {
            Ok(audio) => audio,
            Err(error) if output_len <= 128 => {
                empty_attempt = empty_attempt.saturating_add(1);
                crate::log_debug(&format!(
                    "Audio description: cue {} produced an unreadable tiny WAV ({} bytes: {}), retrying indefinitely; empty_attempt={}",
                    index, output_len, error, empty_attempt
                ));
                crate::log_if_err!(
                    fs::remove_file(&output),
                    "Audio description empty TTS output cleanup failed"
                );
                wait_for_empty_tts_retry(cancel.as_ref())?;
                continue;
            }
            Err(error) => return Err(error),
        };
        if job.tts_engine == TtsEngine::Edge {
            let removed = trim_edge_trailing_silence(&mut samples, sample_rate, channels);
            if removed > 0 {
                crate::log_debug(&format!(
                    "Audio description: removed {} trailing Edge PCM samples from cue {}",
                    removed, index
                ));
            }
        }
        crate::log_if_err!(
            fs::remove_file(&output),
            "Audio description cleanup operation failed"
        );

        if samples.is_empty() || !audio_description_samples_have_signal(&samples) {
            empty_attempt = empty_attempt.saturating_add(1);
            crate::log_debug(&format!(
                "Audio description: TTS cue {} contains no audible PCM signal, retrying indefinitely; empty_attempt={}",
                index, empty_attempt
            ));
            wait_for_empty_tts_retry(cancel.as_ref())?;
            continue;
        }
        return Ok((Arc::from(samples), sample_rate, channels));
    }
}

fn synthesize_description_tasks_parallel<F>(
    tasks: &[AudioDescriptionSynthesisTask],
    job: &AudioDescriptionJob,
    cache_dir: &Path,
    cancel: Arc<AtomicBool>,
    on_completed: F,
) -> Result<Vec<SynthesizedDescription>, String>
where
    F: FnMut(usize, usize),
{
    if tasks.is_empty() {
        return Ok(Vec::new());
    }
    let parallelism = audiobook_synthesis_parallelism(job.tts_engine, &job.tts_voice)
        .min(tasks.len())
        .max(1);
    crate::log_debug(&format!(
        "Audio description: parallel TTS enabled engine={:?} descriptions={} concurrency={}",
        job.tts_engine,
        tasks.len(),
        parallelism
    ));

    let (synthesized, final_limit) = run_description_batches(
        tasks.len(),
        job.tts_engine,
        parallelism,
        cancel.as_ref(),
        |batch_indices| {
            std::thread::scope(|scope| {
                let mut handles = Vec::with_capacity(batch_indices.len());
                for &index in batch_indices {
                    let task = &tasks[index];
                    let cancel = cancel.clone();
                    handles.push(scope.spawn(move || {
                        let (samples, sample_rate, channels) = synthesize_description(
                            &task.text,
                            task.synthesis_index,
                            job,
                            cache_dir,
                            cancel,
                        )?;
                        Ok::<SynthesizedDescription, String>(SynthesizedDescription {
                            original_index: task.original_index,
                            text: task.text.clone(),
                            desired_start_sec: task.desired_start_sec,
                            visual_start_sec: task.visual_start_sec,
                            visual_evidence_time_sec: task.visual_evidence_time_sec,
                            mandatory: task.mandatory,
                            slot_start_sec: task.slot_start_sec,
                            slot_end_sec: task.slot_end_sec,
                            samples,
                            sample_rate,
                            channels,
                        })
                    }));
                }
                handles
                    .into_iter()
                    .map(|handle| {
                        handle.join().unwrap_or_else(|_| {
                            Err("Audio description: parallel TTS worker panicked".to_string())
                        })
                    })
                    .collect::<Vec<_>>()
            })
        },
        on_completed,
    )?;
    if final_limit < parallelism {
        crate::tts_engine::remember_audio_description_sapi5_limit(&job.tts_voice, final_limit);
        crate::log_debug(&format!(
            "Audio description: SAPI5 export completed; remembering concurrency={final_limit} voice={:?}",
            job.tts_voice
        ));
    }
    Ok(synthesized)
}

fn run_description_batches<T, B, P>(
    count: usize,
    engine: TtsEngine,
    mut parallelism: usize,
    cancel: &AtomicBool,
    mut synthesize_batch: B,
    mut on_completed: P,
) -> Result<(Vec<T>, usize), String>
where
    B: FnMut(&[usize]) -> Vec<Result<T, String>>,
    P: FnMut(usize, usize),
{
    let mut synthesized: Vec<Option<T>> = (0..count).map(|_| None).collect();
    let mut pending: std::collections::VecDeque<usize> = (0..count).collect();
    let mut completed = 0_usize;
    while !pending.is_empty() {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        let batch_indices: Vec<usize> = pending.drain(..pending.len().min(parallelism)).collect();
        let batch_results = synthesize_batch(&batch_indices);
        if batch_results.len() != batch_indices.len() {
            return Err("Audio description: incomplete synthesis batch".to_string());
        }

        let mut worker_error = None;
        for (index, result) in batch_indices.into_iter().zip(batch_results) {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".to_string());
            }
            match result {
                Ok(description) => {
                    synthesized[index] = Some(description);
                    completed += 1;
                    on_completed(completed, count);
                }
                Err(error)
                    if audio_description_sapi5_retry_limit(engine, parallelism, &error)
                        .is_some() =>
                {
                    pending.push_front(index);
                    worker_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }
        if let Some(error) = worker_error {
            let previous = parallelism;
            parallelism = (parallelism / 2).max(1);
            crate::log_debug(&format!(
                "Audio description: isolated SAPI5 failure; concurrency={previous} retry_concurrency={parallelism} completed={completed} pending={} error={error}",
                pending.len()
            ));
        }
    }
    let result = synthesized
        .into_iter()
        .map(|item| item.ok_or_else(|| "Audio description: missing synthesis result".to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((result, parallelism))
}

fn audio_description_sapi5_retry_limit(
    engine: TtsEngine,
    current: usize,
    error: &str,
) -> Option<usize> {
    (engine == TtsEngine::Sapi5
        && current > 1
        && error.contains("SAPI5 isolated synthesis failed:"))
    .then(|| (current / 2).max(1))
}

fn normalize_intervals(intervals: &[BridgeInterval], duration_sec: f64) -> Vec<(f64, f64)> {
    let mut values: Vec<(f64, f64)> = intervals
        .iter()
        .filter_map(|interval| {
            let start = interval.start_sec.max(0.0).min(duration_sec);
            let end = interval.end_sec.max(start).min(duration_sec);
            (end > start).then_some((start, end))
        })
        .collect();
    values.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (start, end) in values {
        if let Some(last) = merged.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
}

fn speech_free_intervals(protected: &[(f64, f64)], duration_sec: f64) -> Vec<(f64, f64)> {
    let mut free = Vec::new();
    let mut cursor = 0.0_f64;
    for (start, end) in protected {
        if *start > cursor {
            free.push((cursor, *start));
        }
        cursor = cursor.max(*end);
    }
    if duration_sec > cursor {
        free.push((cursor, duration_sec));
    }
    free
}

fn choose_slot(
    free: &[(f64, f64)],
    desired_start: f64,
    visual_start: f64,
    required_duration: f64,
    earliest_start: f64,
) -> Option<f64> {
    let visual_lower = (visual_start - MAX_SHIFT_SEC).max(0.0);
    let visual_upper = visual_start + MAX_SHIFT_SEC;
    free.iter()
        .filter_map(|(gap_start, gap_end)| {
            let lower = gap_start.max(earliest_start).max(visual_lower);
            let upper = (gap_end - required_duration).min(visual_upper);
            if upper < lower {
                return None;
            }
            let start = desired_start.clamp(lower, upper);
            let distance_from_visual_origin = (start - visual_start).abs();
            (distance_from_visual_origin <= MAX_SHIFT_SEC + f64::EPSILON)
                .then_some(((start - desired_start).abs(), start))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, start)| start)
}

fn choose_pause_anchor(
    free: &[(f64, f64)],
    desired_start: f64,
    visual_start: f64,
    earliest_start: f64,
) -> Option<f64> {
    let visual_lower = (visual_start - MAX_SHIFT_SEC).max(0.0);
    let visual_upper = visual_start + MAX_SHIFT_SEC;
    free.iter()
        .filter_map(|(gap_start, gap_end)| {
            let lower = gap_start.max(earliest_start).max(visual_lower);
            let upper = (gap_end - MIN_EXTENDED_ANCHOR_SEC).min(visual_upper);
            if upper < lower {
                return None;
            }
            let start = desired_start.clamp(lower, upper);
            let distance_from_visual_origin = (start - visual_start).abs();
            (distance_from_visual_origin <= MAX_SHIFT_SEC + f64::EPSILON)
                .then_some(((start - desired_start).abs(), start))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, start)| start)
}

fn subtract_reserved_intervals(free: &[(f64, f64)], reserved: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut blocks = reserved.to_vec();
    blocks.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (start, end) in blocks {
        if let Some(last) = merged.last_mut()
            && start <= last.1
        {
            last.1 = last.1.max(end);
        } else if end > start {
            merged.push((start, end));
        }
    }

    let mut available = Vec::new();
    for (free_start, free_end) in free {
        let mut cursor = *free_start;
        for (block_start, block_end) in &merged {
            if *block_end <= cursor || *block_start >= *free_end {
                continue;
            }
            if *block_start > cursor {
                available.push((cursor, (*block_start).min(*free_end)));
            }
            cursor = cursor.max(*block_end);
            if cursor >= *free_end {
                break;
            }
        }
        if cursor < *free_end {
            available.push((cursor, *free_end));
        }
    }
    available
}

fn restrict_to_mandatory_slot(
    free: &[(f64, f64)],
    description: &SynthesizedDescription,
) -> Vec<(f64, f64)> {
    let (Some(slot_start), Some(slot_end)) = (description.slot_start_sec, description.slot_end_sec)
    else {
        return free.to_vec();
    };
    free.iter()
        .filter_map(|(start, end)| {
            let clipped_start = (*start).max(slot_start);
            let clipped_end = (*end).min(slot_end);
            (clipped_end > clipped_start).then_some((clipped_start, clipped_end))
        })
        .collect()
}

fn schedule_synthesized_descriptions(
    descriptions: &[SynthesizedDescription],
    protected_intervals: &[BridgeInterval],
    duration_sec: f64,
    allow_extended_pauses: bool,
) -> (Vec<ScheduledDescription>, Vec<DroppedDescription>) {
    let protected = normalize_intervals(protected_intervals, duration_sec);
    let free = speech_free_intervals(&protected, duration_sec);
    let mut mandatory: Vec<SynthesizedDescription> = descriptions
        .iter()
        .filter(|description| description.mandatory)
        .cloned()
        .collect();
    let mut optional: Vec<SynthesizedDescription> = descriptions
        .iter()
        .filter(|description| !description.mandatory)
        .cloned()
        .collect();
    mandatory.sort_by(|left, right| left.desired_start_sec.total_cmp(&right.desired_start_sec));
    optional.sort_by(|left, right| left.desired_start_sec.total_cmp(&right.desired_start_sec));

    let mut ordered = mandatory;
    ordered.extend(optional);
    let mut scheduled = Vec::new();
    let mut dropped = Vec::new();
    let mut reserved: Vec<(f64, f64)> = Vec::new();

    for description in ordered {
        let frames = description.samples.len() / description.channels.max(1) as usize;
        let required = frames as f64 / description.sample_rate.max(1) as f64;
        let available = subtract_reserved_intervals(&free, &reserved);
        let candidates = if description.mandatory {
            restrict_to_mandatory_slot(&available, &description)
        } else {
            available
        };

        if let Some(start) = choose_slot(
            &candidates,
            description.desired_start_sec,
            description.visual_start_sec,
            required.max(0.001),
            0.0,
        ) {
            reserved.push((start, start + required.max(0.001)));
            scheduled.push(ScheduledDescription {
                original_index: description.original_index,
                text: description.text,
                desired_start_sec: description.visual_start_sec,
                visual_evidence_time_sec: description.visual_evidence_time_sec,
                start_sec: start,
                samples: description.samples,
                sample_rate: description.sample_rate,
                channels: description.channels,
                extended_pause: false,
            });
            continue;
        }
        if allow_extended_pauses
            && let Some(anchor) = choose_pause_anchor(
                &candidates,
                description.desired_start_sec,
                description.visual_start_sec,
                0.0,
            )
        {
            reserved.push((anchor, anchor + MIN_EXTENDED_ANCHOR_SEC));
            scheduled.push(ScheduledDescription {
                original_index: description.original_index,
                text: description.text,
                desired_start_sec: description.visual_start_sec,
                visual_evidence_time_sec: description.visual_evidence_time_sec,
                start_sec: anchor,
                samples: description.samples,
                sample_rate: description.sample_rate,
                channels: description.channels,
                extended_pause: true,
            });
        } else {
            dropped.push(DroppedDescription {
                original_index: description.original_index,
                text: description.text,
                desired_start_sec: description.visual_start_sec,
                tts_duration_sec: required,
            });
        }
    }
    scheduled.sort_by(|left, right| left.start_sec.total_cmp(&right.start_sec));
    (scheduled, dropped)
}

pub fn audio_description_project_path(output_path: &Path) -> PathBuf {
    let mut path = output_path.to_path_buf();
    path.set_extension("sonarpad-ad.json");
    path
}

pub fn audio_description_partial_checkpoint_path(output_path: &Path) -> PathBuf {
    let mut path = output_path.to_path_buf();
    path.set_extension("sonarpad-ad.partial.json");
    path
}

fn load_audio_description_partial_checkpoint(
    path: &Path,
) -> Result<AudioDescriptionPartialCheckpoint, String> {
    let raw = fs::read(path).map_err(|error| {
        format!("Audio description: could not read partial checkpoint: {error}")
    })?;
    let checkpoint: AudioDescriptionPartialCheckpoint = serde_json::from_slice(&raw)
        .map_err(|error| format!("Audio description: invalid partial checkpoint: {error}"))?;
    if checkpoint.format != AUDIO_DESCRIPTION_PARTIAL_FORMAT
        || checkpoint.version != AUDIO_DESCRIPTION_PARTIAL_VERSION
    {
        return Err("Audio description: unsupported partial checkpoint format".to_string());
    }
    if checkpoint.total_chunks == 0 || checkpoint.completed_chunks > checkpoint.total_chunks {
        return Err("Audio description: invalid partial checkpoint progress".to_string());
    }
    let source_metadata = fs::metadata(&checkpoint.source_path).map_err(|error| {
        format!("Audio description: source video saved in the checkpoint is unavailable: {error}")
    })?;
    if source_metadata.len() != checkpoint.source_file_size {
        return Err(
            "Audio description: source video no longer matches the interrupted job".to_string(),
        );
    }
    Ok(checkpoint)
}

fn save_audio_description_partial_checkpoint(
    path: &Path,
    job: &AudioDescriptionJob,
    source_duration_sec: f64,
    checkpoint: &AudioDescriptionBridgeCheckpoint,
) -> Result<(), String> {
    let source_file_size = fs::metadata(&job.input_path)
        .map_err(|error| format!("Audio description: source file metadata failed: {error}"))?
        .len();
    let character_catalog =
        job.character_catalog
            .as_ref()
            .map(|catalog| AudioDescriptionPartialCatalog {
                name: catalog.name.clone(),
                path: catalog.path.clone(),
                characters: catalog.characters.clone(),
            });
    let value = AudioDescriptionPartialCheckpoint {
        format: AUDIO_DESCRIPTION_PARTIAL_FORMAT.to_string(),
        version: AUDIO_DESCRIPTION_PARTIAL_VERSION,
        source_path: job.input_path.clone(),
        output_mp3_path: job.output_path.clone(),
        audio_stream_index: job.audio_stream_index,
        source_file_size,
        source_duration_sec,
        language: job.tts_language,
        language_code: job.language_code.clone(),
        verbosity: job.verbosity.as_bridge_value().to_string(),
        allow_extended_pauses: job.allow_extended_pauses,
        recognize_characters: job.recognize_characters,
        recognize_screen_text: job.recognize_screen_text,
        save_project: job.save_project,
        tts_engine: job.tts_engine,
        tts_voice: job.tts_voice.clone(),
        tts_rate: job.tts_rate,
        tts_pitch: job.tts_pitch,
        tts_volume: job.tts_volume,
        dictionary: job.dictionary.clone(),
        gemini_model: if checkpoint.gemini_model.trim().is_empty() {
            job.gemini_model.clone()
        } else {
            checkpoint.gemini_model.trim().to_string()
        },
        audiobook_bitrate_kbps: job.audiobook_bitrate_kbps,
        character_catalog,
        completed_chunks: checkpoint.completed_chunks,
        total_chunks: checkpoint.total_chunks,
        descriptions: checkpoint.descriptions.clone(),
        character_glossary: checkpoint.character_glossary.clone(),
    };
    let raw = serde_json::to_vec_pretty(&value)
        .map_err(|error| format!("Audio description: checkpoint serialization failed: {error}"))?;
    let temporary = temporary_sibling_path(path, "partial");
    fs::write(&temporary, raw)
        .map_err(|error| format!("Audio description: checkpoint write failed: {error}"))?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| {
            crate::log_if_err!(
                fs::remove_file(&temporary),
                "Audio description checkpoint cleanup failed"
            );
            format!("Audio description: checkpoint replacement failed: {error}")
        })?;
    }
    fs::rename(&temporary, path).map_err(|error| {
        crate::log_if_err!(
            fs::remove_file(&temporary),
            "Audio description checkpoint cleanup failed"
        );
        format!("Audio description: checkpoint commit failed: {error}")
    })
}

pub fn load_audio_description_resume_settings(
    checkpoint_path: &Path,
) -> Result<AudioDescriptionResumeSettings, String> {
    let checkpoint = load_audio_description_partial_checkpoint(checkpoint_path)?;
    Ok(AudioDescriptionResumeSettings {
        checkpoint_path: checkpoint_path.to_path_buf(),
        input_path: checkpoint.source_path,
        output_path: checkpoint.output_mp3_path,
        description_language: checkpoint.language,
        verbosity: AudioDescriptionVerbosity::from_bridge_value(&checkpoint.verbosity),
        allow_extended_pauses: checkpoint.allow_extended_pauses,
        recognize_characters: checkpoint.recognize_characters,
        recognize_screen_text: checkpoint.recognize_screen_text,
        save_project: checkpoint.save_project,
        tts_engine: checkpoint.tts_engine,
        tts_voice: checkpoint.tts_voice,
        gemini_model: checkpoint.gemini_model,
        completed_chunks: checkpoint.completed_chunks,
        total_chunks: checkpoint.total_chunks,
    })
}

pub fn audio_description_job_from_checkpoint(
    checkpoint_path: &Path,
    gemini_api_key: String,
    sonarpad_ai_service_url: String,
    sonarpad_ai_access_code: String,
    sonarpad_ai_device_id: String,
) -> Result<AudioDescriptionJob, String> {
    let checkpoint = load_audio_description_partial_checkpoint(checkpoint_path)?;
    let character_catalog =
        checkpoint
            .character_catalog
            .map(|catalog| AudioDescriptionCharacterCatalogContext {
                name: catalog.name,
                path: catalog.path,
                characters: catalog.characters,
            });
    Ok(AudioDescriptionJob {
        input_path: checkpoint.source_path,
        output_path: checkpoint.output_mp3_path,
        audio_stream_index: checkpoint.audio_stream_index,
        language_code: checkpoint.language_code,
        tts_language: checkpoint.language,
        verbosity: AudioDescriptionVerbosity::from_bridge_value(&checkpoint.verbosity),
        allow_extended_pauses: checkpoint.allow_extended_pauses,
        recognize_characters: checkpoint.recognize_characters,
        recognize_screen_text: checkpoint.recognize_screen_text,
        character_catalog,
        save_project: checkpoint.save_project,
        tts_engine: checkpoint.tts_engine,
        tts_voice: checkpoint.tts_voice,
        tts_rate: checkpoint.tts_rate,
        tts_pitch: checkpoint.tts_pitch,
        tts_volume: checkpoint.tts_volume,
        dictionary: checkpoint.dictionary,
        gemini_api_key,
        sonarpad_ai_service_url,
        sonarpad_ai_access_code,
        sonarpad_ai_device_id,
        gemini_model: checkpoint.gemini_model,
        audiobook_bitrate_kbps: checkpoint.audiobook_bitrate_kbps,
        resume_checkpoint_path: Some(checkpoint_path.to_path_buf()),
    })
}

fn scheduled_duration_sec(description: &ScheduledDescription) -> f64 {
    let frames = description.samples.len() / description.channels.max(1) as usize;
    frames as f64 / description.sample_rate.max(1) as f64
}

fn build_audio_description_project(
    job: &AudioDescriptionJob,
    source_duration_sec: f64,
    output_duration_sec: f64,
    protected_intervals: &[BridgeInterval],
    scheduled: &[ScheduledDescription],
    dropped: &[DroppedDescription],
) -> AudioDescriptionProject {
    let now = chrono::Utc::now().to_rfc3339();
    let mut output_offset_sec = 0.0_f64;
    let mut descriptions = Vec::with_capacity(scheduled.len());
    for description in scheduled {
        let tts_duration_sec = scheduled_duration_sec(description);
        let output_start_sec = description.start_sec + output_offset_sec;
        let output_end_sec = output_start_sec + tts_duration_sec;
        let (duck_start_sec, duck_end_sec, extended_pause_duration_sec) =
            if description.extended_pause {
                output_offset_sec += tts_duration_sec;
                (None, None, tts_duration_sec)
            } else {
                (
                    Some(
                        (output_start_sec
                            - (AUDIO_DESCRIPTION_FADE_MS + AUDIO_DESCRIPTION_PRE_DUCK_MS) as f64
                                / 1000.0)
                            .max(0.0),
                    ),
                    Some(output_end_sec + AUDIO_DESCRIPTION_RELEASE_MS as f64 / 1000.0),
                    0.0,
                )
            };
        descriptions.push(AudioDescriptionProjectDescription {
            id: description.original_index,
            text: description.text.clone(),
            original_text: description.text.clone(),
            rendered_text: description.text.clone(),
            modified: false,
            gemini_start_sec: description.desired_start_sec,
            visual_evidence_time_sec: description.visual_evidence_time_sec,
            source_start_sec: description.start_sec,
            output_start_sec,
            output_end_sec,
            tts_duration_sec,
            extended_pause: description.extended_pause,
            extended_pause_duration_sec,
            duck_start_sec,
            duck_end_sec,
        });
    }
    let excluded_descriptions = dropped
        .iter()
        .map(|description| AudioDescriptionProjectExcluded {
            id: description.original_index,
            text: description.text.clone(),
            gemini_start_sec: description.desired_start_sec,
            tts_duration_sec: description.tts_duration_sec,
            reason: "no_safe_slot_after_real_tts_duration".to_string(),
        })
        .collect();
    AudioDescriptionProject {
        format: "sonarpad-audio-description-project".to_string(),
        version: 1,
        created_at_utc: now.clone(),
        updated_at_utc: now,
        source_path: job.input_path.clone(),
        output_mp3_path: job.output_path.clone(),
        audio_stream_index: job.audio_stream_index,
        source_duration_sec,
        output_duration_sec,
        language: job.tts_language,
        language_code: job.language_code.clone(),
        verbosity: job.verbosity.as_bridge_value().to_string(),
        allow_extended_pauses: job.allow_extended_pauses,
        recognize_characters: job.recognize_characters,
        gemini_model: job.gemini_model.clone(),
        tts_engine: job.tts_engine,
        tts_voice: job.tts_voice.clone(),
        tts_rate: job.tts_rate,
        tts_pitch: job.tts_pitch,
        tts_volume: job.tts_volume,
        dictionary: job.dictionary.clone(),
        bitrate_kbps: AUDIO_DESCRIPTION_BITRATE_KBPS,
        ducking_db: AUDIO_DESCRIPTION_DUCKING_DB,
        fade_ms: AUDIO_DESCRIPTION_FADE_MS,
        protected_intervals: protected_intervals
            .iter()
            .map(|interval| AudioDescriptionProjectInterval {
                start_sec: interval.start_sec,
                end_sec: interval.end_sec,
            })
            .collect(),
        descriptions,
        excluded_descriptions,
    }
}

fn temporary_sibling_path(path: &Path, label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("audio_description");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let file_name = if extension.is_empty() {
        format!(".{stem}.{label}.{}.{}", std::process::id(), stamp)
    } else {
        format!(
            ".{stem}.{label}.{}.{}.{}",
            std::process::id(),
            stamp,
            extension
        )
    };
    parent.join(file_name)
}

fn commit_audio_description_pair(
    temporary_mp3: &Path,
    final_mp3: &Path,
    temporary_project: &Path,
    final_project: &Path,
) -> Result<(), String> {
    let mp3_backup = temporary_sibling_path(final_mp3, "backup");
    let project_backup = temporary_sibling_path(final_project, "backup");
    let had_mp3 = final_mp3.exists();
    let had_project = final_project.exists();

    if had_mp3 {
        fs::rename(final_mp3, &mp3_backup)
            .map_err(|error| format!("Audio description: backup old MP3 failed: {error}"))?;
    }
    if had_project && let Err(error) = fs::rename(final_project, &project_backup) {
        if had_mp3 {
            crate::log_if_err!(
                fs::rename(&mp3_backup, final_mp3),
                "Audio description cleanup operation failed"
            );
        }
        return Err(format!(
            "Audio description: backup old project failed: {error}"
        ));
    }

    if let Err(error) = fs::rename(temporary_mp3, final_mp3) {
        if had_project {
            crate::log_if_err!(
                fs::rename(&project_backup, final_project),
                "Audio description cleanup operation failed"
            );
        }
        if had_mp3 {
            crate::log_if_err!(
                fs::rename(&mp3_backup, final_mp3),
                "Audio description cleanup operation failed"
            );
        }
        return Err(format!("Audio description: finalize MP3 failed: {error}"));
    }
    if let Err(error) = fs::rename(temporary_project, final_project) {
        crate::log_if_err!(
            fs::remove_file(final_mp3),
            "Audio description cleanup operation failed"
        );
        if had_mp3 {
            crate::log_if_err!(
                fs::rename(&mp3_backup, final_mp3),
                "Audio description cleanup operation failed"
            );
        }
        if had_project {
            crate::log_if_err!(
                fs::rename(&project_backup, final_project),
                "Audio description cleanup operation failed"
            );
        }
        return Err(format!(
            "Audio description: finalize project failed: {error}"
        ));
    }

    if had_mp3 {
        crate::log_if_err!(
            fs::remove_file(mp3_backup),
            "Audio description cleanup operation failed"
        );
    }
    if had_project {
        crate::log_if_err!(
            fs::remove_file(project_backup),
            "Audio description cleanup operation failed"
        );
    }
    Ok(())
}

pub fn load_audio_description_project(path: &Path) -> Result<AudioDescriptionProject, String> {
    let raw = fs::read(path)
        .map_err(|error| format!("Audio description: read project failed: {error}"))?;
    let mut project: AudioDescriptionProject = serde_json::from_slice(&raw)
        .map_err(|error| format!("Audio description: invalid project JSON: {error}"))?;
    if project.format != "sonarpad-audio-description-project" || project.version != 1 {
        return Err("Audio description: unsupported project format or version".to_string());
    }
    if project.descriptions.is_empty() {
        return Err("Audio description: project contains no inserted descriptions".to_string());
    }
    for description in &mut project.descriptions {
        if description.rendered_text.is_empty() {
            description.rendered_text = description.text.clone();
        }
    }
    Ok(project)
}

pub fn save_audio_description_project(
    path: &Path,
    project: &AudioDescriptionProject,
) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Audio description: create project folder failed: {error}"))?;
    }
    let temp_path = path.with_extension("sonarpad-ad.json.tmp");
    let bytes = serde_json::to_vec_pretty(project)
        .map_err(|error| format!("Audio description: serialize project failed: {error}"))?;
    fs::write(&temp_path, bytes)
        .map_err(|error| format!("Audio description: write temporary project failed: {error}"))?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("Audio description: replace old project failed: {error}"))?;
    }
    fs::rename(&temp_path, path)
        .map_err(|error| format!("Audio description: finalize project failed: {error}"))?;
    Ok(())
}

fn audio_description_job_from_project(project: &AudioDescriptionProject) -> AudioDescriptionJob {
    AudioDescriptionJob {
        input_path: project.source_path.clone(),
        output_path: project.output_mp3_path.clone(),
        audio_stream_index: project.audio_stream_index,
        language_code: project.language_code.clone(),
        tts_language: project.language,
        verbosity: verbosity_from_project(&project.verbosity),
        allow_extended_pauses: project.allow_extended_pauses,
        recognize_characters: project.recognize_characters,
        recognize_screen_text: false,
        character_catalog: None,
        save_project: true,
        tts_engine: project.tts_engine,
        tts_voice: project.tts_voice.clone(),
        tts_rate: project.tts_rate,
        tts_pitch: project.tts_pitch,
        tts_volume: project.tts_volume,
        dictionary: project.dictionary.clone(),
        gemini_api_key: String::new(),
        sonarpad_ai_service_url: String::new(),
        sonarpad_ai_access_code: String::new(),
        sonarpad_ai_device_id: String::new(),
        gemini_model: project.gemini_model.clone(),
        audiobook_bitrate_kbps: project.bitrate_kbps,
        resume_checkpoint_path: None,
    }
}

pub fn audio_description_project_edit_available_duration(
    project: &AudioDescriptionProject,
    index: usize,
) -> Result<Option<f64>, String> {
    let description = project.descriptions.get(index).ok_or_else(|| {
        "Audio description: selected project description does not exist".to_string()
    })?;
    if description.extended_pause {
        return Ok(None);
    }

    let protected: Vec<BridgeInterval> = project
        .protected_intervals
        .iter()
        .map(|interval| BridgeInterval {
            start_sec: interval.start_sec,
            end_sec: interval.end_sec,
        })
        .collect();
    let normalized = normalize_intervals(&protected, project.source_duration_sec);
    let free = speech_free_intervals(&normalized, project.source_duration_sec);
    let start = description.source_start_sec.max(0.0);
    let Some((_, gap_end)) = free
        .iter()
        .find(|(gap_start, gap_end)| start + 0.001 >= *gap_start && start <= *gap_end + 0.001)
    else {
        return Ok(Some(0.0));
    };
    let next_description_start = project
        .descriptions
        .iter()
        .enumerate()
        .filter(|(candidate_index, candidate)| {
            *candidate_index != index && candidate.source_start_sec > start + 0.001
        })
        .map(|(_, candidate)| candidate.source_start_sec)
        .min_by(f64::total_cmp);
    let available_end = next_description_start
        .map(|next_start| gap_end.min(next_start))
        .unwrap_or(*gap_end);
    Ok(Some((available_end - start).max(0.0)))
}

fn validate_audio_description_project_edit_duration(
    available_duration_sec: Option<f64>,
    synthesized_duration_sec: f64,
) -> Result<(), AudioDescriptionProjectEditError> {
    if let Some(available_sec) = available_duration_sec
        && synthesized_duration_sec > available_sec + 0.010
    {
        return Err(AudioDescriptionProjectEditError::TooLong {
            available_sec,
            synthesized_sec: synthesized_duration_sec,
        });
    }
    Ok(())
}

fn write_audio_description_preview_wav(
    output: &Path,
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: channels.max(1),
        sample_rate: sample_rate.max(1),
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(output, spec)
        .map_err(|error| format!("Audio description: create preview WAV failed: {error}"))?;
    for sample in samples {
        writer
            .write_sample(sample.clamp(-1.0, 1.0))
            .map_err(|error| format!("Audio description: write preview WAV failed: {error}"))?;
    }
    writer
        .finalize()
        .map_err(|error| format!("Audio description: finalize preview WAV failed: {error}"))
}

pub fn synthesize_audio_description_project_preview(
    project: &AudioDescriptionProject,
    index: usize,
    text: &str,
    cancel: Arc<AtomicBool>,
) -> Result<AudioDescriptionProjectPreviewAudio, String> {
    let normalized_text = text.trim();
    if normalized_text.is_empty() {
        return Err("Audio description: description text cannot be empty".to_string());
    }
    let current = project.descriptions.get(index).ok_or_else(|| {
        "Audio description: selected project description does not exist".to_string()
    })?;
    if project.tts_voice.trim().is_empty() {
        return Err("Audio description: project has no synthesis voice".to_string());
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }

    let job = audio_description_job_from_project(project);
    let cache_dir = temporary_job_dir()?;
    let synthesis_result = synthesize_description(
        normalized_text,
        current.id,
        &job,
        &cache_dir,
        cancel.clone(),
    );
    let (samples, sample_rate, channels) = match synthesis_result {
        Ok(values) => values,
        Err(error) => {
            crate::log_if_err!(
                fs::remove_dir_all(&cache_dir),
                "Audio description cleanup operation failed"
            );
            return Err(error);
        }
    };
    if cancel.load(Ordering::Relaxed) {
        crate::log_if_err!(
            fs::remove_dir_all(&cache_dir),
            "Audio description cleanup operation failed"
        );
        return Err("cancelled".to_string());
    }
    let frames = samples.len() / channels.max(1) as usize;
    let duration_sec = frames as f64 / sample_rate.max(1) as f64;
    let path = cache_dir.join("modified_description_preview.wav");
    if let Err(error) =
        write_audio_description_preview_wav(&path, samples.as_ref(), sample_rate, channels)
    {
        crate::log_if_err!(
            fs::remove_dir_all(&cache_dir),
            "Audio description cleanup operation failed"
        );
        return Err(error);
    }
    Ok(AudioDescriptionProjectPreviewAudio {
        path,
        cache_dir,
        duration_sec,
    })
}

pub fn apply_audio_description_project_batch_edits(
    project_path: &Path,
    project: &AudioDescriptionProject,
    edits: &[(usize, String)],
    cancel: Arc<AtomicBool>,
) -> Result<AudioDescriptionProjectEditOutcome, AudioDescriptionProjectBatchEditError> {
    if edits.is_empty() {
        return Ok(AudioDescriptionProjectEditOutcome {
            project: project.clone(),
            applied_count: 0,
        });
    }
    if project.tts_voice.trim().is_empty() {
        return Err(AudioDescriptionProjectBatchEditError {
            index: edits.first().map(|(index, _)| *index),
            error: AudioDescriptionProjectEditError::Other(
                "Audio description: project has no synthesis voice".to_string(),
            ),
        });
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AudioDescriptionProjectBatchEditError {
            index: None,
            error: AudioDescriptionProjectEditError::Cancelled,
        });
    }

    let mut normalized_edits = Vec::new();
    for (index, text) in edits {
        let normalized_text = text.trim();
        if normalized_text.is_empty() {
            return Err(AudioDescriptionProjectBatchEditError {
                index: Some(*index),
                error: AudioDescriptionProjectEditError::Other(
                    "Audio description: description text cannot be empty".to_string(),
                ),
            });
        }
        let current = project.descriptions.get(*index).ok_or_else(|| {
            AudioDescriptionProjectBatchEditError {
                index: Some(*index),
                error: AudioDescriptionProjectEditError::Other(
                    "Audio description: selected project description does not exist".to_string(),
                ),
            }
        })?;
        if current.text != normalized_text {
            normalized_edits.push((*index, normalized_text.to_string()));
        }
    }

    if normalized_edits.is_empty() {
        return Ok(AudioDescriptionProjectEditOutcome {
            project: project.clone(),
            applied_count: 0,
        });
    }

    let job = audio_description_job_from_project(project);
    let cache_dir = temporary_job_dir().map_err(|error| AudioDescriptionProjectBatchEditError {
        index: None,
        error: AudioDescriptionProjectEditError::Other(error),
    })?;

    let validation_result = (|| {
        for (index, text) in &normalized_edits {
            if cancel.load(Ordering::Relaxed) {
                return Err(AudioDescriptionProjectBatchEditError {
                    index: Some(*index),
                    error: AudioDescriptionProjectEditError::Cancelled,
                });
            }
            let current = project.descriptions.get(*index).ok_or_else(|| {
                AudioDescriptionProjectBatchEditError {
                    index: Some(*index),
                    error: AudioDescriptionProjectEditError::Other(
                        "Audio description: selected project description does not exist"
                            .to_string(),
                    ),
                }
            })?;
            let available_duration_sec = audio_description_project_edit_available_duration(
                project, *index,
            )
            .map_err(|error| AudioDescriptionProjectBatchEditError {
                index: Some(*index),
                error: AudioDescriptionProjectEditError::Other(error),
            })?;
            let synthesis_result =
                synthesize_description(text, current.id, &job, &cache_dir, cancel.clone());
            let (samples, sample_rate, channels) =
                synthesis_result.map_err(|error| AudioDescriptionProjectBatchEditError {
                    index: Some(*index),
                    error: if error == "cancelled" || cancel.load(Ordering::Relaxed) {
                        AudioDescriptionProjectEditError::Cancelled
                    } else {
                        AudioDescriptionProjectEditError::Other(error)
                    },
                })?;
            if cancel.load(Ordering::Relaxed) {
                return Err(AudioDescriptionProjectBatchEditError {
                    index: Some(*index),
                    error: AudioDescriptionProjectEditError::Cancelled,
                });
            }
            let frames = samples.len() / channels.max(1) as usize;
            let synthesized_duration_sec = frames as f64 / sample_rate.max(1) as f64;
            validate_audio_description_project_edit_duration(
                available_duration_sec,
                synthesized_duration_sec,
            )
            .map_err(|error| AudioDescriptionProjectBatchEditError {
                index: Some(*index),
                error,
            })?;
        }
        Ok(())
    })();

    crate::log_if_err!(
        fs::remove_dir_all(&cache_dir),
        "Audio description cleanup operation failed"
    );
    validation_result?;

    let mut updated = project.clone();
    for (index, text) in &normalized_edits {
        let description = updated.descriptions.get_mut(*index).ok_or_else(|| {
            AudioDescriptionProjectBatchEditError {
                index: Some(*index),
                error: AudioDescriptionProjectEditError::Other(
                    "Audio description: selected project description does not exist".to_string(),
                ),
            }
        })?;
        description.text = text.clone();
        description.modified = description.text != description.original_text;
    }
    updated.updated_at_utc = chrono::Utc::now().to_rfc3339();
    save_audio_description_project(project_path, &updated).map_err(|error| {
        AudioDescriptionProjectBatchEditError {
            index: None,
            error: AudioDescriptionProjectEditError::Other(error),
        }
    })?;

    Ok(AudioDescriptionProjectEditOutcome {
        project: updated,
        applied_count: normalized_edits.len(),
    })
}

pub fn change_audio_description_project_voice(
    project_path: &Path,
    project: &AudioDescriptionProject,
    settings: &AudioDescriptionProjectVoiceSettings,
    cancel: Arc<AtomicBool>,
    mut callbacks: AudioDescriptionCallbacks,
) -> Result<AudioDescriptionProject, AudioDescriptionProjectVoiceError> {
    let voice = settings.voice.trim();
    if voice.is_empty() {
        return Err(AudioDescriptionProjectVoiceError::Other(
            "Audio description: no synthesis voice is selected".to_string(),
        ));
    }
    if project.descriptions.is_empty() {
        return Err(AudioDescriptionProjectVoiceError::Other(
            "Audio description: project contains no descriptions".to_string(),
        ));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AudioDescriptionProjectVoiceError::Cancelled);
    }

    let mut job = audio_description_job_from_project(project);
    job.tts_engine = settings.engine;
    job.tts_voice = voice.to_string();
    job.tts_rate = settings.rate;
    job.tts_volume = settings.volume;
    notify_status(
        &mut callbacks,
        "voice_check",
        "Checking all project descriptions with the selected voice...",
    );
    notify_progress(&mut callbacks, 0);

    let cache_dir = temporary_job_dir().map_err(AudioDescriptionProjectVoiceError::Other)?;
    let tasks = project
        .descriptions
        .iter()
        .enumerate()
        .map(|(index, description)| AudioDescriptionSynthesisTask {
            synthesis_index: index,
            original_index: description.id,
            text: description.text.clone(),
            desired_start_sec: description.source_start_sec,
            visual_start_sec: description.gemini_start_sec,
            visual_evidence_time_sec: description.visual_evidence_time_sec,
            mandatory: false,
            slot_start_sec: None,
            slot_end_sec: None,
        })
        .collect::<Vec<_>>();
    let synthesis_result = synthesize_description_tasks_parallel(
        &tasks,
        &job,
        &cache_dir,
        cancel.clone(),
        |completed, total| {
            notify_progress(
                &mut callbacks,
                (completed as u32).saturating_mul(90) / total.max(1) as u32,
            );
        },
    )
    .map_err(|error| {
        if error == "cancelled" || cancel.load(Ordering::Relaxed) {
            AudioDescriptionProjectVoiceError::Cancelled
        } else {
            AudioDescriptionProjectVoiceError::Other(error)
        }
    });
    crate::log_if_err!(
        fs::remove_dir_all(&cache_dir),
        "Audio description cleanup operation failed"
    );
    let synthesized = synthesis_result?;

    if cancel.load(Ordering::Relaxed) {
        return Err(AudioDescriptionProjectVoiceError::Cancelled);
    }
    let protected_intervals = project
        .protected_intervals
        .iter()
        .map(|interval| BridgeInterval {
            start_sec: interval.start_sec,
            end_sec: interval.end_sec,
        })
        .collect::<Vec<_>>();
    let (scheduled, dropped) = schedule_synthesized_descriptions(
        &synthesized,
        &protected_intervals,
        project.source_duration_sec,
        project.allow_extended_pauses,
    );
    if let Some(first) = dropped.first() {
        let source_start_sec = project
            .descriptions
            .iter()
            .find(|description| description.id == first.original_index)
            .map(|description| description.source_start_sec)
            .unwrap_or(first.desired_start_sec);
        return Err(AudioDescriptionProjectVoiceError::DoesNotFit {
            source_start_sec,
            synthesized_sec: first.tts_duration_sec,
        });
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AudioDescriptionProjectVoiceError::Cancelled);
    }
    if !project.source_path.is_file() {
        return Err(AudioDescriptionProjectVoiceError::Other(format!(
            "Audio description: source file not found: {}",
            project.source_path.display()
        )));
    }
    if let Some(parent) = project.output_mp3_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            AudioDescriptionProjectVoiceError::Other(format!(
                "Audio description: create output folder failed: {error}"
            ))
        })?;
    }

    notify_status(
        &mut callbacks,
        "voice_export",
        "Rebuilding the project MP3 with the verified voice...",
    );
    crate::log_debug(
        "Audio description project voice: all descriptions fit; rebuilding MP3 from verified synthesized audio",
    );
    let mix_cues: Vec<AudioDescriptionMixCue> = scheduled
        .iter()
        .map(|description| AudioDescriptionMixCue {
            start_sec: description.start_sec,
            samples: description.samples.clone(),
            sample_rate: description.sample_rate,
            channels: description.channels,
            extended_pause: description.extended_pause,
        })
        .collect();
    let export_target = temporary_sibling_path(&project.output_mp3_path, "voice");
    let export_options = AudioDescriptionExportOptions {
        ducking_db: project.ducking_db,
        fade_ms: project.fade_ms,
        bitrate_kbps: project.bitrate_kbps,
        cancel: cancel.clone(),
    };
    let mut export_progress = |pct: u32| {
        notify_progress(&mut callbacks, 90 + pct.saturating_mul(10) / 100);
    };
    if let Err(error) = export_audio_description_mp3(
        &project.source_path,
        &export_target,
        project.audio_stream_index,
        &mix_cues,
        &export_options,
        Some(&mut export_progress),
    ) {
        if export_target.exists() {
            crate::log_if_err!(
                fs::remove_file(&export_target),
                "Audio description cleanup operation failed"
            );
        }
        return if error == "cancelled" || cancel.load(Ordering::Relaxed) {
            Err(AudioDescriptionProjectVoiceError::Cancelled)
        } else {
            Err(AudioDescriptionProjectVoiceError::Other(error))
        };
    }
    let output_metadata = fs::metadata(&export_target).map_err(|error| {
        AudioDescriptionProjectVoiceError::Other(format!(
            "Audio description: changed-voice MP3 validation failed: {error}"
        ))
    })?;
    if output_metadata.len() == 0 {
        crate::log_if_err!(
            fs::remove_file(&export_target),
            "Audio description cleanup operation failed"
        );
        return Err(AudioDescriptionProjectVoiceError::Other(
            "Audio description: changed-voice MP3 is empty".to_string(),
        ));
    }
    if cancel.load(Ordering::Relaxed) {
        crate::log_if_err!(
            fs::remove_file(&export_target),
            "Audio description cleanup operation failed"
        );
        return Err(AudioDescriptionProjectVoiceError::Cancelled);
    }

    let calculated_output_duration = project.source_duration_sec
        + scheduled
            .iter()
            .filter(|description| description.extended_pause)
            .map(scheduled_duration_sec)
            .sum::<f64>();
    let output_duration_sec = crate::ffmpeg_export::media_duration_seconds(&export_target)
        .unwrap_or(calculated_output_duration);
    let mut updated = build_audio_description_project(
        &job,
        project.source_duration_sec,
        output_duration_sec,
        &protected_intervals,
        &scheduled,
        &[],
    );
    updated.created_at_utc = project.created_at_utc.clone();
    updated.updated_at_utc = chrono::Utc::now().to_rfc3339();
    updated.bitrate_kbps = project.bitrate_kbps;
    updated.ducking_db = project.ducking_db;
    updated.fade_ms = project.fade_ms;
    for description in &mut updated.descriptions {
        if let Some(previous) = project
            .descriptions
            .iter()
            .find(|candidate| candidate.id == description.id)
        {
            description.original_text = previous.original_text.clone();
            description.modified = description.text != description.original_text;
            description.gemini_start_sec = previous.gemini_start_sec;
        }
    }
    let scheduled_ids = scheduled
        .iter()
        .map(|description| description.original_index)
        .collect::<std::collections::HashSet<_>>();
    let mut excluded = project.excluded_descriptions.clone();
    excluded.retain(|description| !scheduled_ids.contains(&description.id));
    updated.excluded_descriptions = excluded;

    let temporary_project = temporary_sibling_path(project_path, "voice");
    if let Err(error) = save_audio_description_project(&temporary_project, &updated) {
        crate::log_if_err!(
            fs::remove_file(&export_target),
            "Audio description cleanup operation failed"
        );
        return Err(AudioDescriptionProjectVoiceError::Other(error));
    }
    if let Err(error) = commit_audio_description_pair(
        &export_target,
        &project.output_mp3_path,
        &temporary_project,
        project_path,
    ) {
        crate::log_if_err!(
            fs::remove_file(&export_target),
            "Audio description cleanup operation failed"
        );
        crate::log_if_err!(
            fs::remove_file(&temporary_project),
            "Audio description cleanup operation failed"
        );
        return Err(AudioDescriptionProjectVoiceError::Other(error));
    }

    notify_progress(&mut callbacks, 100);
    Ok(updated)
}

pub fn delete_audio_description_project_description(
    project_path: &Path,
    project: &AudioDescriptionProject,
    index: usize,
) -> Result<AudioDescriptionProject, String> {
    if project.descriptions.len() <= 1 {
        return Err(
            "Audio description: the only project description cannot be deleted".to_string(),
        );
    }
    if index >= project.descriptions.len() {
        return Err("Audio description: selected project description does not exist".to_string());
    }
    let mut updated = project.clone();
    updated.descriptions.remove(index);
    updated.updated_at_utc = chrono::Utc::now().to_rfc3339();
    save_audio_description_project(project_path, &updated)?;
    Ok(updated)
}

fn validate_job(job: &AudioDescriptionJob) -> Result<(), String> {
    if !job.input_path.is_file() {
        return Err(format!(
            "Audio description: input file not found: {}",
            job.input_path.display()
        ));
    }
    if job.output_path.as_os_str().is_empty() {
        return Err("Audio description: output path is empty".to_string());
    }
    if job.gemini_api_key.trim().is_empty()
        && (job.sonarpad_ai_service_url.trim().is_empty()
            || job.sonarpad_ai_access_code.trim().is_empty()
            || job.sonarpad_ai_device_id.trim().is_empty())
    {
        return Err(
            "Audio description: Gemini API key or Sonarpad AI access is not configured".to_string(),
        );
    }
    if job.tts_voice.trim().is_empty() {
        return Err("Audio description: no synthesis voice is selected".to_string());
    }
    Ok(())
}

fn verbosity_from_project(value: &str) -> AudioDescriptionVerbosity {
    match value {
        "short" => AudioDescriptionVerbosity::Brief,
        "standard" => AudioDescriptionVerbosity::Standard,
        _ => AudioDescriptionVerbosity::Detailed,
    }
}

pub fn reexport_audio_description_project(
    project_path: &Path,
    project: &AudioDescriptionProject,
    cancel: Arc<AtomicBool>,
    mut callbacks: AudioDescriptionCallbacks,
) -> Result<AudioDescriptionOutcome, String> {
    if !project.source_path.is_file() {
        return Err(format!(
            "Audio description: source file not found: {}",
            project.source_path.display()
        ));
    }
    if project.descriptions.is_empty() {
        return Err("Audio description: project contains no descriptions to export".to_string());
    }
    if project.tts_voice.trim().is_empty() {
        return Err("Audio description: project has no synthesis voice".to_string());
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }

    let job = audio_description_job_from_project(project);
    if let Some(parent) = job.output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Audio description: create output folder failed: {error}"))?;
    }

    notify_status(
        &mut callbacks,
        "tts_edit",
        "Synthesizing the edited project descriptions with Sonarpad...",
    );
    notify_progress(&mut callbacks, 0);
    let cache_dir = temporary_job_dir()?;
    let tasks = project
        .descriptions
        .iter()
        .enumerate()
        .map(|(index, description)| AudioDescriptionSynthesisTask {
            synthesis_index: index,
            original_index: description.id,
            text: description.text.clone(),
            desired_start_sec: description.source_start_sec,
            visual_start_sec: description.gemini_start_sec,
            visual_evidence_time_sec: description.visual_evidence_time_sec,
            mandatory: false,
            slot_start_sec: None,
            slot_end_sec: None,
        })
        .collect::<Vec<_>>();
    let synthesis_result = synthesize_description_tasks_parallel(
        &tasks,
        &job,
        &cache_dir,
        cancel.clone(),
        |completed, total| {
            notify_progress(
                &mut callbacks,
                (completed as u32).saturating_mul(60) / total.max(1) as u32,
            );
        },
    );
    crate::log_if_err!(
        fs::remove_dir_all(&cache_dir),
        "Audio description cleanup operation failed"
    );
    let synthesized = synthesis_result?;

    let protected_intervals: Vec<BridgeInterval> = project
        .protected_intervals
        .iter()
        .map(|interval| BridgeInterval {
            start_sec: interval.start_sec,
            end_sec: interval.end_sec,
        })
        .collect();
    notify_status(
        &mut callbacks,
        "schedule_edit",
        "Checking edited descriptions against the saved Pyannote intervals...",
    );
    let (scheduled, dropped_descriptions) = schedule_synthesized_descriptions(
        &synthesized,
        &protected_intervals,
        project.source_duration_sec,
        project.allow_extended_pauses,
    );
    if scheduled.is_empty() {
        return Err("Audio description: no edited description can be placed safely".to_string());
    }

    let mix_cues: Vec<AudioDescriptionMixCue> = scheduled
        .iter()
        .map(|description| AudioDescriptionMixCue {
            start_sec: description.start_sec,
            samples: description.samples.clone(),
            sample_rate: description.sample_rate,
            channels: description.channels,
            extended_pause: description.extended_pause,
        })
        .collect();
    let export_target = temporary_sibling_path(&project.output_mp3_path, "edited");
    let export_options = AudioDescriptionExportOptions {
        ducking_db: project.ducking_db,
        fade_ms: project.fade_ms,
        bitrate_kbps: project.bitrate_kbps,
        cancel: cancel.clone(),
    };
    notify_status(
        &mut callbacks,
        "export_edit",
        "Exporting the edited MP3 with Sonarpad's Rust FFmpeg libraries...",
    );
    let mut export_progress = |pct: u32| {
        notify_progress(&mut callbacks, 65 + pct.saturating_mul(35) / 100);
    };
    if let Err(error) = export_audio_description_mp3(
        &project.source_path,
        &export_target,
        project.audio_stream_index,
        &mix_cues,
        &export_options,
        Some(&mut export_progress),
    ) {
        if export_target.exists() {
            crate::log_if_err!(
                fs::remove_file(&export_target),
                "Audio description cleanup operation failed"
            );
        }
        return Err(error);
    }
    let output_metadata = fs::metadata(&export_target)
        .map_err(|error| format!("Audio description: edited MP3 validation failed: {error}"))?;
    if output_metadata.len() == 0 {
        crate::log_if_err!(
            fs::remove_file(&export_target),
            "Audio description cleanup operation failed"
        );
        return Err("Audio description: edited MP3 is empty".to_string());
    }

    let calculated_output_duration = project.source_duration_sec
        + scheduled
            .iter()
            .filter(|description| description.extended_pause)
            .map(scheduled_duration_sec)
            .sum::<f64>();
    let output_duration_sec = crate::ffmpeg_export::media_duration_seconds(&export_target)
        .unwrap_or(calculated_output_duration);
    let mut updated = build_audio_description_project(
        &job,
        project.source_duration_sec,
        output_duration_sec,
        &protected_intervals,
        &scheduled,
        &dropped_descriptions,
    );
    updated.created_at_utc = project.created_at_utc.clone();
    updated.updated_at_utc = chrono::Utc::now().to_rfc3339();
    for description in &mut updated.descriptions {
        if let Some(previous) = project
            .descriptions
            .iter()
            .find(|candidate| candidate.id == description.id)
        {
            description.original_text = previous.original_text.clone();
            description.modified = description.text != description.original_text;
            description.gemini_start_sec = previous.gemini_start_sec;
        }
    }
    let scheduled_ids = scheduled
        .iter()
        .map(|description| description.original_index)
        .collect::<std::collections::HashSet<_>>();
    let mut excluded = project.excluded_descriptions.clone();
    excluded.retain(|description| !scheduled_ids.contains(&description.id));
    for description in &dropped_descriptions {
        excluded.retain(|candidate| candidate.id != description.original_index);
        excluded.push(AudioDescriptionProjectExcluded {
            id: description.original_index,
            text: description.text.clone(),
            gemini_start_sec: description.desired_start_sec,
            tts_duration_sec: description.tts_duration_sec,
            reason: "dropped_after_project_edit_real_tts_duration".to_string(),
        });
    }
    excluded.sort_by_key(|description| description.id);
    updated.excluded_descriptions = excluded;

    let temporary_project = temporary_sibling_path(project_path, "edited");
    if let Err(error) = save_audio_description_project(&temporary_project, &updated) {
        crate::log_if_err!(
            fs::remove_file(&export_target),
            "Audio description cleanup operation failed"
        );
        return Err(error);
    }
    if let Err(error) = commit_audio_description_pair(
        &export_target,
        &project.output_mp3_path,
        &temporary_project,
        project_path,
    ) {
        crate::log_if_err!(
            fs::remove_file(&export_target),
            "Audio description cleanup operation failed"
        );
        crate::log_if_err!(
            fs::remove_file(&temporary_project),
            "Audio description cleanup operation failed"
        );
        return Err(error);
    }

    let normal_descriptions = scheduled
        .iter()
        .filter(|description| !description.extended_pause)
        .count();
    let extended_pauses = scheduled
        .iter()
        .filter(|description| description.extended_pause)
        .count();
    notify_progress(&mut callbacks, 100);
    notify_status(
        &mut callbacks,
        "complete_edit",
        "Edited audio-description MP3 export complete.",
    );
    Ok(AudioDescriptionOutcome {
        output_path: project.output_mp3_path.clone(),
        project_path: Some(project_path.to_path_buf()),
        project_warning: None,
        character_catalog_path: None,
        character_catalog_warning: None,
        generated_descriptions: project.descriptions.len(),
        normal_descriptions,
        extended_pauses,
        dropped_after_tts: dropped_descriptions.len(),
    })
}

pub fn create_audio_description(
    job: &AudioDescriptionJob,
    cancel: Arc<AtomicBool>,
    mut callbacks: AudioDescriptionCallbacks,
) -> Result<AudioDescriptionOutcome, String> {
    validate_job(job)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }
    if let Some(parent) = job.output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Audio description: create output folder failed: {error}"))?;
    }

    let checkpoint_path = job
        .resume_checkpoint_path
        .clone()
        .unwrap_or_else(|| audio_description_partial_checkpoint_path(&job.output_path));

    notify_status(
        &mut callbacks,
        "analysis_prepare",
        "Preparing media with Sonarpad FFmpeg libraries...",
    );
    notify_progress(&mut callbacks, 0);

    let analysis_cache_dir = temporary_job_dir()?;
    let preparation_result =
        (|| -> Result<(f64, Option<PathBuf>, Vec<AudioDescriptionPreparedChunk>), String> {
            let (raw_duration_sec, format_start_sec) =
                crate::ffmpeg_export::media_duration_and_start_seconds(&job.input_path)
                    .ok_or_else(|| {
                        "Audio description: FFmpeg could not read media duration".to_string()
                    })?;
            let duration_sec = normalize_audio_description_source_duration(
                &job.input_path,
                raw_duration_sec,
                format_start_sec,
            );
            if (duration_sec - raw_duration_sec).abs() > 0.001 {
                crate::log_debug(&format!(
                    "Audio description: normalized source duration raw={:.3}s start_time={:.3}s local={:.3}s path={}",
                    raw_duration_sec,
                    format_start_sec,
                    duration_sec,
                    job.input_path.display()
                ));
            }
            if duration_sec <= 0.0 {
                return Err("Audio description: selected media is empty".to_string());
            }
            let has_video =
                crate::ffmpeg_source::has_real_video_stream(&job.input_path).map_err(|error| {
                    format!("Audio description: FFmpeg video inspection failed: {error}")
                })?;
            if !has_video {
                return Err(
                    "Audio description: selected file has no usable video stream".to_string(),
                );
            }
            let audio_streams =
                crate::ffmpeg_source::list_audio_streams(&job.input_path).map_err(|error| {
                    format!("Audio description: FFmpeg stream inspection failed: {error}")
                })?;
            let audio_wav_path = if audio_streams.is_empty() {
                None
            } else {
                notify_status(
                    &mut callbacks,
                    "pyannote_prepare",
                    "Decoding mono 16 kHz audio with Sonarpad FFmpeg libraries...",
                );
                let path = analysis_cache_dir.join("pyannote_input.wav");
                write_pyannote_wav(&job.input_path, &path, job.audio_stream_index, &cancel)?;
                Some(path)
            };
            notify_progress(&mut callbacks, 5);
            notify_status(
                &mut callbacks,
                "chunk_prepare",
                "Preparing Gemini video chunks with Sonarpad FFmpeg libraries...",
            );
            let chunks = prepare_gemini_chunks(
                &job.input_path,
                duration_sec,
                &analysis_cache_dir,
                job.audio_stream_index,
                &cancel,
            )?;
            notify_progress(&mut callbacks, 10);
            Ok((duration_sec, audio_wav_path, chunks))
        })();
    let (duration_sec, audio_wav_path, chunks) = match preparation_result {
        Ok(value) => value,
        Err(error) => {
            crate::log_if_err!(
                fs::remove_dir_all(&analysis_cache_dir),
                "Audio description cleanup operation failed"
            );
            return Err(error);
        }
    };

    let resume = if job.resume_checkpoint_path.is_some() {
        match load_audio_description_partial_checkpoint(&checkpoint_path) {
            Ok(checkpoint)
                if checkpoint.total_chunks == chunks.len()
                    && (checkpoint.source_duration_sec - duration_sec).abs() <= 0.5 =>
            {
                Some(AudioDescriptionBridgeResume {
                    completed_chunks: checkpoint.completed_chunks,
                    descriptions: checkpoint.descriptions,
                    character_glossary: checkpoint.character_glossary,
                })
            }
            Ok(checkpoint) => {
                crate::log_debug(&format!(
                    "Audio description: ignoring resume checkpoint after chunk layout change saved_chunks={} prepared_chunks={} saved_duration={:.3} prepared_duration={:.3}",
                    checkpoint.total_chunks,
                    chunks.len(),
                    checkpoint.source_duration_sec,
                    duration_sec
                ));
                None
            }
            Err(error) => {
                crate::log_debug(&format!(
                    "Audio description: ignoring invalid resume checkpoint {}: {}",
                    checkpoint_path.display(),
                    error
                ));
                None
            }
        }
    } else {
        None
    };
    let use_sonarpad_ai = !job.sonarpad_ai_service_url.trim().is_empty();
    if use_sonarpad_ai && !job.gemini_api_key.trim().is_empty() {
        return Err(
            "Audio description: invalid AI configuration: Sonarpad AI mode must not include a personal Gemini API key".to_string(),
        );
    }
    if !use_sonarpad_ai && job.gemini_api_key.trim().is_empty() {
        return Err(
            "Audio description: invalid AI configuration: personal Gemini mode requires an API key"
                .to_string(),
        );
    }
    crate::log_debug(if use_sonarpad_ai {
        "Audio description: AI access mode = Sonarpad AI (personal Gemini API key disabled)"
    } else {
        "Audio description: AI access mode = personal Gemini API key"
    });

    let bridge_request = AudioDescriptionBridgeRequest {
        input_path: job.input_path.to_string_lossy().to_string(),
        audio_wav_path: audio_wav_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        duration_sec,
        chunks,
        language: job.language_code.clone(),
        verbosity: job.verbosity.as_bridge_value().to_string(),
        allow_extended_pauses: job.allow_extended_pauses,
        recognize_characters: job.recognize_characters,
        recognize_screen_text: job.recognize_screen_text,
        initial_character_glossary: job
            .character_catalog
            .as_ref()
            .map(|catalog| catalog.characters.clone())
            .unwrap_or_default(),
        ai_access_mode: if job.sonarpad_ai_service_url.trim().is_empty() {
            "personal".to_string()
        } else {
            "sonarpad".to_string()
        },
        gemini_api_key: job.gemini_api_key.clone(),
        sonarpad_ai_service_url: job.sonarpad_ai_service_url.clone(),
        sonarpad_ai_access_code: job.sonarpad_ai_access_code.clone(),
        sonarpad_ai_device_id: job.sonarpad_ai_device_id.clone(),
        gemini_model: job.gemini_model.clone(),
        resume,
    };
    let callback_state = Arc::new(std::sync::Mutex::new(callbacks));
    let download_state = callback_state.clone();
    let progress_state = callback_state.clone();
    let status_state = callback_state.clone();
    let quota_state = callback_state.clone();
    let overload_state = callback_state.clone();
    let checkpoint_job = job.clone();
    let checkpoint_target = checkpoint_path.clone();
    let analysis_result = run_audio_description_bridge(
        &bridge_request,
        cancel.clone(),
        AudioDescriptionBridgeCallbacks {
            download: Some(Box::new(move |pct| {
                if let Ok(mut callbacks) = download_state.lock() {
                    notify_status(
                        &mut callbacks,
                        "download",
                        "Downloading the audio-description analysis module...",
                    );
                    notify_progress(&mut callbacks, (pct.max(0) as u32).saturating_mul(10) / 100);
                }
            })),
            progress: Some(Box::new(move |pct| {
                if let Ok(mut callbacks) = progress_state.lock() {
                    notify_progress(
                        &mut callbacks,
                        10 + (pct.max(0) as u32).saturating_mul(45) / 100,
                    );
                }
            })),
            status: Some(Box::new(move |stage, message| {
                if let Ok(mut callbacks) = status_state.lock() {
                    notify_status(&mut callbacks, stage, message);
                }
            })),
            quota: Some(Box::new(move |model, error| {
                let Ok(mut callbacks) = quota_state.lock() else {
                    return AudioDescriptionQuotaDecision::Stop;
                };
                callbacks
                    .quota
                    .as_mut()
                    .map(|callback| callback(model, error))
                    .unwrap_or(AudioDescriptionQuotaDecision::Wait)
            })),
            overload: Some(Box::new(move |model, error| {
                let Ok(mut callbacks) = overload_state.lock() else {
                    return AudioDescriptionOverloadDecision::Stop;
                };
                callbacks
                    .overload
                    .as_mut()
                    .map(|callback| callback(model, error))
                    .unwrap_or(AudioDescriptionOverloadDecision::Wait)
            })),
            checkpoint: Some(Box::new(move |checkpoint| {
                if let Err(error) = save_audio_description_partial_checkpoint(
                    &checkpoint_target,
                    &checkpoint_job,
                    duration_sec,
                    checkpoint,
                ) {
                    crate::log_debug(&format!(
                        "Audio description: partial checkpoint save failed: {error}"
                    ));
                } else {
                    crate::log_debug(&format!(
                        "Audio description: saved partial checkpoint after chunk {}/{} to {}",
                        checkpoint.completed_chunks,
                        checkpoint.total_chunks,
                        checkpoint_target.display()
                    ));
                }
            })),
        },
    );
    crate::log_if_err!(
        fs::remove_dir_all(&analysis_cache_dir),
        "Audio description cleanup operation failed"
    );
    let analysis = analysis_result?;
    let mut callbacks = Arc::try_unwrap(callback_state)
        .map_err(|_| "Audio description: callback state still in use".to_string())?
        .into_inner()
        .map_err(|_| "Audio description: callback state poisoned".to_string())?;
    let mut effective_job = job.clone();
    if !analysis.gemini_model.trim().is_empty() {
        effective_job.gemini_model = analysis.gemini_model.trim().to_string();
    }

    if analysis.descriptions.is_empty() {
        return Err("Audio description: Gemini returned no descriptions".to_string());
    }
    notify_status(
        &mut callbacks,
        "tts",
        "Synthesizing descriptions with the selected Sonarpad voice...",
    );
    let cache_dir = temporary_job_dir()?;
    let tasks = analysis
        .descriptions
        .iter()
        .enumerate()
        .map(|(index, description)| AudioDescriptionSynthesisTask {
            synthesis_index: index,
            original_index: index,
            text: description.text.clone(),
            desired_start_sec: description.start_sec,
            visual_start_sec: description
                .visual_start_sec
                .unwrap_or(description.start_sec),
            visual_evidence_time_sec: description.visual_evidence_time_sec,
            mandatory: description.mandatory,
            slot_start_sec: description.slot_start_sec,
            slot_end_sec: description.slot_end_sec,
        })
        .collect::<Vec<_>>();
    let synthesis_result = synthesize_description_tasks_parallel(
        &tasks,
        job,
        &cache_dir,
        cancel.clone(),
        |completed, total| {
            let pct = 55 + (completed as u32).saturating_mul(25) / total.max(1) as u32;
            notify_progress(&mut callbacks, pct);
        },
    );
    crate::log_if_err!(
        fs::remove_dir_all(&cache_dir),
        "Audio description cleanup operation failed"
    );
    let synthesized = synthesis_result?;

    notify_status(
        &mut callbacks,
        "schedule",
        "Checking the real TTS duration against Pyannote silences...",
    );
    let (scheduled, dropped_descriptions) = schedule_synthesized_descriptions(
        &synthesized,
        &analysis.protected_intervals,
        analysis.duration_sec,
        job.allow_extended_pauses,
    );
    let dropped_after_tts = dropped_descriptions.len();
    if scheduled.is_empty() {
        return Err(
            "Audio description: no synthesized description can be placed safely between dialogue"
                .to_string(),
        );
    }
    let normal_descriptions = scheduled
        .iter()
        .filter(|description| !description.extended_pause)
        .count();
    let extended_pauses = scheduled
        .iter()
        .filter(|description| description.extended_pause)
        .count();
    let mix_cues: Vec<AudioDescriptionMixCue> = scheduled
        .iter()
        .map(|description| AudioDescriptionMixCue {
            start_sec: description.start_sec,
            samples: description.samples.clone(),
            sample_rate: description.sample_rate,
            channels: description.channels,
            extended_pause: description.extended_pause,
        })
        .collect();

    notify_status(
        &mut callbacks,
        "export",
        "Applying ducking and exporting MP3 with Sonarpad's Rust FFmpeg libraries...",
    );
    let export_options = AudioDescriptionExportOptions {
        // Ducking is a Sonarpad export policy, not a worker setting.
        ducking_db: AUDIO_DESCRIPTION_DUCKING_DB,
        fade_ms: AUDIO_DESCRIPTION_FADE_MS,
        bitrate_kbps: AUDIO_DESCRIPTION_BITRATE_KBPS,
        cancel: cancel.clone(),
    };
    let export_target = if job.save_project {
        temporary_sibling_path(&job.output_path, "new")
    } else {
        job.output_path.clone()
    };
    let mut export_progress = |pct: u32| {
        notify_progress(&mut callbacks, 80 + pct.saturating_mul(20) / 100);
    };
    let export_result = export_audio_description_mp3(
        &job.input_path,
        &export_target,
        job.audio_stream_index,
        &mix_cues,
        &export_options,
        Some(&mut export_progress),
    );
    if let Err(error) = export_result {
        if export_target.exists() {
            crate::log_if_err!(
                fs::remove_file(&export_target),
                "Audio description cleanup operation failed"
            );
        }
        return Err(error);
    }
    let output_metadata = fs::metadata(&export_target)
        .map_err(|error| format!("Audio description: exported MP3 validation failed: {error}"))?;
    if output_metadata.len() == 0 {
        crate::log_if_err!(
            fs::remove_file(&export_target),
            "Audio description cleanup operation failed"
        );
        return Err("Audio description: exported MP3 is empty".to_string());
    }

    let mut project_path = None;
    let project_warning = None;
    if job.save_project {
        notify_status(
            &mut callbacks,
            "project",
            "Saving the descriptions actually inserted in the exported MP3...",
        );
        let path = audio_description_project_path(&job.output_path);
        let temporary_project = temporary_sibling_path(&path, "new");
        let calculated_output_duration = analysis.duration_sec
            + scheduled
                .iter()
                .filter(|description| description.extended_pause)
                .map(scheduled_duration_sec)
                .sum::<f64>();
        let output_duration_sec = crate::ffmpeg_export::media_duration_seconds(&export_target)
            .unwrap_or(calculated_output_duration);
        let project = build_audio_description_project(
            &effective_job,
            analysis.duration_sec,
            output_duration_sec,
            &analysis.protected_intervals,
            &scheduled,
            &dropped_descriptions,
        );
        if let Err(error) = save_audio_description_project(&temporary_project, &project) {
            crate::log_if_err!(
                fs::remove_file(&export_target),
                "Audio description cleanup operation failed"
            );
            return Err(error);
        }
        if let Err(error) = commit_audio_description_pair(
            &export_target,
            &job.output_path,
            &temporary_project,
            &path,
        ) {
            crate::log_if_err!(
                fs::remove_file(&export_target),
                "Audio description cleanup operation failed"
            );
            crate::log_if_err!(
                fs::remove_file(&temporary_project),
                "Audio description cleanup operation failed"
            );
            return Err(error);
        }
        project_path = Some(path);
    }

    let (character_catalog_path, character_catalog_warning) =
        if let Some(catalog) = job.character_catalog.as_ref() {
            match save_audio_description_character_catalog(catalog, &analysis.character_glossary) {
                Ok(()) => (Some(catalog.path.clone()), None),
                Err(error) => (None, Some(error)),
            }
        } else {
            (None, None)
        };

    notify_progress(&mut callbacks, 100);
    notify_status(
        &mut callbacks,
        "complete",
        "Audio-description MP3 export complete.",
    );
    if checkpoint_path.exists() {
        crate::log_if_err!(
            fs::remove_file(&checkpoint_path),
            "Audio description: remove completed partial checkpoint failed"
        );
    }
    Ok(AudioDescriptionOutcome {
        output_path: job.output_path.clone(),
        project_path,
        project_warning,
        character_catalog_path,
        character_catalog_warning,
        generated_descriptions: analysis.descriptions.len(),
        normal_descriptions,
        extended_pauses,
        dropped_after_tts,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        AudioDescriptionCharacterCatalogContext, AudioDescriptionJob,
        AudioDescriptionProjectEditError, AudioDescriptionVerbosity, ScheduledDescription,
        SynthesizedDescription, audio_description_character_catalog_dir,
        audio_description_character_catalog_path,
        audio_description_project_edit_available_duration, audio_description_project_path,
        audio_description_samples_have_signal, audio_description_tts_chunks,
        audio_description_tts_error_is_empty_output, build_audio_description_project,
        build_gemini_chunk_timeline, choose_slot, delete_audio_description_project_description,
        load_audio_description_character_catalog_context, load_audio_description_project,
        merge_catalog_characters, merge_catalog_description,
        normalize_audio_description_source_duration, normalize_catalog_characters,
        normalize_prepared_gemini_chunk_duration, save_audio_description_character_catalog,
        save_audio_description_project, schedule_synthesized_descriptions, scheduled_duration_sec,
        trim_edge_trailing_silence, validate_audio_description_project_edit_duration,
    };
    use crate::settings::{DictionaryEntry, Language, TtsEngine};
    use crate::tools::audio_description_bridge::{BridgeCharacter, BridgeInterval};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn audio_description_source_duration_normalizes_large_matroska_clock_offset() {
        let normalized = normalize_audio_description_source_duration(
            Path::new("movie.mkv"),
            37_234.103,
            31_234.103,
        );
        assert!((normalized - 6_000.0).abs() < 0.001);
    }

    #[test]
    fn audio_description_source_duration_keeps_other_containers_unchanged() {
        assert_eq!(
            normalize_audio_description_source_duration(
                Path::new("movie.mp4"),
                6_000.0,
                31_234.103,
            ),
            6_000.0
        );
    }

    #[test]
    fn audio_description_source_duration_keeps_small_matroska_offsets_unchanged() {
        assert_eq!(
            normalize_audio_description_source_duration(Path::new("movie.mkv"), 6_002.0, 2.0),
            6_002.0
        );
    }

    #[test]
    fn gemini_chunk_duration_keeps_normal_zero_based_media_unchanged() {
        assert_eq!(
            normalize_prepared_gemini_chunk_duration(51.125, 0.0, 51),
            51.125
        );
    }

    #[test]
    fn gemini_chunk_duration_keeps_duration_semantics_with_large_start_time() {
        assert_eq!(
            normalize_prepared_gemini_chunk_duration(51.125, 31_234.103, 51),
            51.125
        );
    }

    #[test]
    fn gemini_chunk_duration_normalizes_absolute_end_timestamp_from_large_start_time() {
        let normalized = normalize_prepared_gemini_chunk_duration(31_285.228, 31_234.103, 51);
        assert!((normalized - 51.125).abs() < 0.001);
    }

    #[test]
    fn gemini_chunk_duration_does_not_rewrite_small_timestamp_offsets() {
        assert_eq!(
            normalize_prepared_gemini_chunk_duration(120.0, 2.0, 51),
            120.0
        );
    }

    #[test]
    fn gemini_chunk_timeline_keeps_existing_valid_measurements_unchanged() {
        let measured = vec![
            (PathBuf::from("chunk1.mkv"), 45.25),
            (PathBuf::from("chunk2.mkv"), 44.75),
            (PathBuf::from("chunk3.mkv"), 10.0),
        ];
        let timeline = build_gemini_chunk_timeline(&measured, 100.0, false).unwrap();
        assert!((timeline[0].start_sec - 0.0).abs() < 0.001);
        assert!((timeline[0].end_sec - 45.25).abs() < 0.001);
        assert!((timeline[1].start_sec - 45.25).abs() < 0.001);
        assert!((timeline[1].end_sec - 90.0).abs() < 0.001);
        assert!((timeline[2].start_sec - 90.0).abs() < 0.001);
        assert!((timeline[2].end_sec - 100.0).abs() < 0.001);
    }

    #[test]
    fn gemini_chunk_timeline_can_reconcile_small_accumulated_duration_drift() {
        let measured = vec![
            (PathBuf::from("chunk1.mkv"), 50.5),
            (PathBuf::from("chunk2.mkv"), 50.5),
            (PathBuf::from("chunk3.mkv"), 0.1),
        ];
        assert!(build_gemini_chunk_timeline(&measured, 100.0, false).is_none());
        let timeline = build_gemini_chunk_timeline(&measured, 100.0, true).unwrap();
        assert_eq!(timeline.len(), 3);
        assert!(timeline[0].end_sec > timeline[0].start_sec);
        assert!(timeline[1].end_sec > timeline[1].start_sec);
        assert!(timeline[2].end_sec > timeline[2].start_sec);
        assert!((timeline[2].end_sec - 100.0).abs() < 0.001);
    }

    #[test]
    fn gemini_chunk_timeline_rejects_large_duration_mismatch_even_in_fallback() {
        let measured = vec![
            (PathBuf::from("chunk1.mkv"), 60.0),
            (PathBuf::from("chunk2.mkv"), 60.0),
            (PathBuf::from("chunk3.mkv"), 1.0),
        ];
        assert!(build_gemini_chunk_timeline(&measured, 100.0, true).is_none());
    }

    #[test]
    fn empty_tts_validation_rejects_silent_pcm_and_accepts_voice_signal() {
        assert!(!audio_description_samples_have_signal(&[]));
        assert!(!audio_description_samples_have_signal(&[0.0, 0.0, 0.0]));
        assert!(audio_description_samples_have_signal(&[0.0, 0.001, 0.0]));
    }

    #[test]
    fn empty_tts_renderer_errors_are_marked_for_indefinite_retry() {
        assert!(audio_description_tts_error_is_empty_output(
            "Segment decode failed: decoded audio contains no samples"
        ));
        assert!(audio_description_tts_error_is_empty_output(
            "Audio description: empty WAV returned by engine"
        ));
        assert!(!audio_description_tts_error_is_empty_output(
            "selected voice is not installed"
        ));
    }

    #[test]
    fn project_path_keeps_the_audio_name_and_adds_project_suffix() {
        assert_eq!(
            audio_description_project_path(PathBuf::from("movie.mp3").as_path()),
            PathBuf::from("movie.sonarpad-ad.json")
        );
    }

    #[test]
    fn audio_description_tts_uses_voice_dictionary_replacements() {
        let job = AudioDescriptionJob {
            input_path: PathBuf::from("movie.mkv"),
            output_path: PathBuf::from("movie.mp3"),
            audio_stream_index: None,
            language_code: "it".to_string(),
            tts_language: Language::Italian,
            verbosity: AudioDescriptionVerbosity::Detailed,
            allow_extended_pauses: false,
            recognize_characters: true,
            recognize_screen_text: false,
            character_catalog: None,
            save_project: false,
            tts_engine: TtsEngine::Edge,
            tts_voice: "it-IT-ElsaNeural".to_string(),
            tts_rate: 0,
            tts_pitch: 0,
            tts_volume: 100,
            dictionary: vec![DictionaryEntry {
                original: "Sonarpad".to_string(),
                replacement: "Sonar pad".to_string(),
                match_case: true,
                use_custom_voice: false,
                custom_voice_engine: None,
                custom_voice: None,
            }],
            gemini_api_key: String::new(),
            sonarpad_ai_service_url: String::new(),
            sonarpad_ai_access_code: String::new(),
            sonarpad_ai_device_id: String::new(),
            gemini_model: "gemini".to_string(),
            audiobook_bitrate_kbps: 192,
            resume_checkpoint_path: None,
        };
        let chunks = audio_description_tts_chunks("Sonarpad descrive la scena.", &job);
        assert!(!chunks.is_empty());
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.text_to_read.contains("Sonar pad"))
        );
        assert!(
            chunks
                .iter()
                .all(|chunk| !chunk.text_to_read.contains("Sonarpad"))
        );
    }

    #[test]
    fn project_timeline_includes_prior_extended_pauses() {
        let job = AudioDescriptionJob {
            input_path: PathBuf::from("movie.mkv"),
            output_path: PathBuf::from("movie.mp3"),
            audio_stream_index: None,
            language_code: "it".to_string(),
            tts_language: Language::Italian,
            verbosity: AudioDescriptionVerbosity::Detailed,
            allow_extended_pauses: true,
            recognize_characters: true,
            recognize_screen_text: false,
            character_catalog: None,
            save_project: true,
            tts_engine: TtsEngine::Edge,
            tts_voice: "it-IT-ElsaNeural".to_string(),
            tts_rate: 0,
            tts_pitch: 0,
            tts_volume: 100,
            dictionary: Vec::new(),
            gemini_api_key: String::new(),
            sonarpad_ai_service_url: String::new(),
            sonarpad_ai_access_code: String::new(),
            sonarpad_ai_device_id: String::new(),
            gemini_model: "gemini".to_string(),
            audiobook_bitrate_kbps: 192,
            resume_checkpoint_path: None,
        };
        let scheduled = vec![
            ScheduledDescription {
                original_index: 0,
                text: "Prima".to_string(),
                desired_start_sec: 2.0,
                visual_evidence_time_sec: None,
                start_sec: 2.0,
                samples: Arc::from(vec![0.1_f32; 30]),
                sample_rate: 10,
                channels: 1,
                extended_pause: true,
            },
            ScheduledDescription {
                original_index: 1,
                text: "Seconda".to_string(),
                desired_start_sec: 4.0,
                visual_evidence_time_sec: None,
                start_sec: 4.0,
                samples: Arc::from(vec![0.1_f32; 10]),
                sample_rate: 10,
                channels: 1,
                extended_pause: false,
            },
        ];
        let project = build_audio_description_project(&job, 10.0, 13.0, &[], &scheduled, &[]);
        assert!((project.descriptions[0].output_start_sec - 2.0).abs() < 0.001);
        assert!((project.descriptions[0].output_end_sec - 5.0).abs() < 0.001);
        assert!((project.descriptions[1].output_start_sec - 7.0).abs() < 0.001);
        assert!((project.descriptions[1].output_end_sec - 8.0).abs() < 0.001);
        assert!(project.recognize_characters);

        let mut legacy_json = serde_json::to_value(&project).expect("serialize project");
        legacy_json
            .as_object_mut()
            .expect("project object")
            .remove("recognize_characters");
        let legacy_project: super::AudioDescriptionProject =
            serde_json::from_value(legacy_json).expect("deserialize legacy project");
        assert!(legacy_project.recognize_characters);
    }

    #[test]
    fn exact_scheduler_moves_description_into_nearby_silence() {
        let free = vec![(0.0, 3.0), (5.0, 10.0)];
        assert_eq!(choose_slot(&free, 4.0, 4.0, 2.0, 0.0), Some(5.0));
    }

    #[test]
    fn exact_scheduler_enforces_global_shift_from_visual_origin() {
        let too_far = vec![(10.5, 20.0)];
        assert_eq!(choose_slot(&too_far, 10.0, 5.0, 1.0, 0.0), None);

        let boundary = vec![(10.0, 20.0)];
        assert_eq!(choose_slot(&boundary, 10.0, 5.0, 1.0, 0.0), Some(10.0));
    }

    #[test]
    fn exact_scheduler_reserves_mandatory_slot_before_optional_description() {
        let descriptions = vec![
            SynthesizedDescription {
                original_index: 0,
                text: "Optional".to_string(),
                desired_start_sec: 2.0,
                visual_start_sec: 2.0,
                visual_evidence_time_sec: None,
                mandatory: false,
                slot_start_sec: None,
                slot_end_sec: None,
                samples: Arc::from(vec![0.2_f32; 60]),
                sample_rate: 10,
                channels: 1,
            },
            SynthesizedDescription {
                original_index: 1,
                text: "Mandatory".to_string(),
                desired_start_sec: 5.0,
                visual_start_sec: 5.0,
                visual_evidence_time_sec: None,
                mandatory: true,
                slot_start_sec: Some(5.0),
                slot_end_sec: Some(8.0),
                samples: Arc::from(vec![0.2_f32; 30]),
                sample_rate: 10,
                channels: 1,
            },
        ];
        let (scheduled, dropped) =
            schedule_synthesized_descriptions(&descriptions, &[], 10.0, false);
        assert_eq!(scheduled.len(), 1);
        assert_eq!(scheduled[0].original_index, 1);
        assert!((scheduled[0].start_sec - 5.0).abs() < 0.001);
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].original_index, 0);
    }

    #[test]
    fn extended_mode_preserves_unfittable_description_as_pause() {
        let descriptions = vec![SynthesizedDescription {
            original_index: 0,
            text: "A description".to_string(),
            desired_start_sec: 3.0,
            visual_start_sec: 3.0,
            visual_evidence_time_sec: None,
            mandatory: false,
            slot_start_sec: None,
            slot_end_sec: None,
            samples: Arc::from(vec![0.2_f32; 4 * 44_100]),
            sample_rate: 44_100,
            channels: 1,
        }];
        let protected = vec![BridgeInterval {
            start_sec: 1.0,
            end_sec: 9.0,
        }];
        let (scheduled, dropped) =
            schedule_synthesized_descriptions(&descriptions, &protected, 10.0, true);
        assert!(dropped.is_empty());
        assert_eq!(scheduled.len(), 1);
        assert!(scheduled[0].extended_pause);
    }

    #[test]
    fn edge_trim_keeps_short_tail_but_removes_long_silence() {
        let rate = 1_000;
        let mut samples = vec![0.5_f32; 500];
        samples.extend(vec![0.0_f32; 300]);
        let removed = trim_edge_trailing_silence(&mut samples, rate, 1);
        assert!(removed >= 200);
        assert!(samples.len() >= 500);
        assert!(samples.len() <= 550);
    }

    #[test]
    fn edge_trim_does_not_delete_an_all_silent_cue() {
        let mut samples = vec![0.0_f32; 500];
        let removed = trim_edge_trailing_silence(&mut samples, 1_000, 1);
        assert_eq!(removed, 0);
        assert_eq!(samples.len(), 500);
    }

    #[test]
    fn omni_port_tts_trailing_silence_removed_without_cutting_speech() {
        let rate = 1_000;
        let mut samples = vec![0.45_f32; 500];
        samples.extend(vec![0.0_f32; 300]);
        let removed = trim_edge_trailing_silence(&mut samples, rate, 1);
        assert!(removed >= 200);
        assert!(
            samples[..500]
                .iter()
                .all(|sample| (*sample - 0.45).abs() < f32::EPSILON)
        );
        assert!((500..=550).contains(&samples.len()));
    }

    #[test]
    fn omni_port_short_tts_tail_is_preserved() {
        let rate = 1_000;
        let mut samples = vec![0.4_f32; 500];
        samples.extend(vec![0.0_f32; 40]);
        let original_len = samples.len();
        let removed = trim_edge_trailing_silence(&mut samples, rate, 1);
        assert_eq!(removed, 0);
        assert_eq!(samples.len(), original_len);
    }

    #[test]
    fn omni_port_json_project_round_trip_preserves_inserted_timeline() {
        let job = AudioDescriptionJob {
            input_path: PathBuf::from("film.mkv"),
            output_path: PathBuf::from("film-audiodescritto.mp3"),
            audio_stream_index: Some(2),
            language_code: "it".to_string(),
            tts_language: Language::Italian,
            verbosity: AudioDescriptionVerbosity::Detailed,
            allow_extended_pauses: true,
            recognize_characters: true,
            recognize_screen_text: false,
            character_catalog: None,
            save_project: true,
            tts_engine: TtsEngine::Edge,
            tts_voice: "it-IT-ElsaNeural".to_string(),
            tts_rate: 0,
            tts_pitch: 0,
            tts_volume: 100,
            dictionary: Vec::new(),
            gemini_api_key: String::new(),
            sonarpad_ai_service_url: String::new(),
            sonarpad_ai_access_code: String::new(),
            sonarpad_ai_device_id: String::new(),
            gemini_model: "gemini-3.5-flash-lite".to_string(),
            audiobook_bitrate_kbps: 192,
            resume_checkpoint_path: None,
        };
        let scheduled = vec![ScheduledDescription {
            original_index: 0,
            text: "Apre la porta.".to_string(),
            desired_start_sec: 2.0,
            visual_evidence_time_sec: None,
            start_sec: 2.5,
            samples: Arc::from(vec![0.2_f32; 20]),
            sample_rate: 10,
            channels: 1,
            extended_pause: false,
        }];
        let project = build_audio_description_project(
            &job,
            10.0,
            10.0,
            &[BridgeInterval {
                start_sec: 5.0,
                end_sec: 8.0,
            }],
            &scheduled,
            &[],
        );
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("sonarpad_ad_roundtrip_{stamp}.json"));
        save_audio_description_project(&path, &project).expect("save project");
        let loaded = load_audio_description_project(&path).expect("load project");
        crate::log_if_err!(
            std::fs::remove_file(path),
            "Audio description cleanup operation failed"
        );
        assert_eq!(loaded.descriptions.len(), 1);
        assert_eq!(loaded.descriptions[0].text, "Apre la porta.");
        assert!((loaded.descriptions[0].output_start_sec - 2.5).abs() < 0.001);
        assert_eq!(loaded.protected_intervals.len(), 1);
        assert_eq!(loaded.gemini_model, "gemini-3.5-flash-lite");
        assert_eq!(loaded.audio_stream_index, Some(2));
    }

    #[test]
    fn omni_port_legacy_project_defaults_character_recognition_to_true() {
        let job = AudioDescriptionJob {
            input_path: PathBuf::from("film.mkv"),
            output_path: PathBuf::from("film.mp3"),
            audio_stream_index: Some(3),
            language_code: "it".to_string(),
            tts_language: Language::Italian,
            verbosity: AudioDescriptionVerbosity::Detailed,
            allow_extended_pauses: true,
            recognize_characters: true,
            recognize_screen_text: false,
            character_catalog: None,
            save_project: true,
            tts_engine: TtsEngine::Edge,
            tts_voice: "voice".to_string(),
            tts_rate: 0,
            tts_pitch: 0,
            tts_volume: 100,
            dictionary: Vec::new(),
            gemini_api_key: String::new(),
            sonarpad_ai_service_url: String::new(),
            sonarpad_ai_access_code: String::new(),
            sonarpad_ai_device_id: String::new(),
            gemini_model: "gemini".to_string(),
            audiobook_bitrate_kbps: 192,
            resume_checkpoint_path: None,
        };
        let project = build_audio_description_project(&job, 1.0, 1.0, &[], &[], &[]);
        let mut value = serde_json::to_value(project).expect("serialize");
        let object = value.as_object_mut().expect("object");
        object.remove("recognize_characters");
        object.remove("audio_stream_index");
        let loaded: super::AudioDescriptionProject =
            serde_json::from_value(value).expect("legacy project");
        assert!(loaded.recognize_characters);
        assert_eq!(loaded.audio_stream_index, None);
    }

    #[test]
    fn omni_port_extended_pause_lengthens_output_timeline() {
        let descriptions = vec![SynthesizedDescription {
            original_index: 0,
            text: "Descrizione lunga".to_string(),
            desired_start_sec: 2.0,
            visual_start_sec: 2.0,
            visual_evidence_time_sec: None,
            mandatory: false,
            slot_start_sec: None,
            slot_end_sec: None,
            samples: Arc::from(vec![0.2_f32; 16_000]),
            sample_rate: 8_000,
            channels: 1,
        }];
        let protected = vec![
            BridgeInterval {
                start_sec: 0.0,
                end_sec: 2.0,
            },
            BridgeInterval {
                start_sec: 3.0,
                end_sec: 5.0,
            },
        ];
        let (scheduled, dropped) =
            schedule_synthesized_descriptions(&descriptions, &protected, 5.0, true);
        assert!(dropped.is_empty());
        assert_eq!(scheduled.len(), 1);
        assert!(scheduled[0].extended_pause);
        let final_duration = 5.0 + scheduled.iter().map(scheduled_duration_sec).sum::<f64>();
        assert!((final_duration - 7.0).abs() < 0.001);
    }
    #[test]
    fn omni_port_character_catalog_is_stored_below_audiodescriptions() {
        let directory = audio_description_character_catalog_dir("C:/Sonarpad/Audiodescriptions");
        assert!(directory.ends_with(PathBuf::from("Audiodescriptions").join("Catalogs")));
        let path = audio_description_character_catalog_path(
            "C:/Sonarpad/Audiodescriptions",
            "Serie: prova",
        );
        assert!(
            path.ends_with(
                PathBuf::from("Audiodescriptions")
                    .join("Catalogs")
                    .join("Serie_ prova.json")
            )
        );
    }

    #[test]
    fn omni_port_character_catalog_reuses_prior_episode_and_merges_new_characters() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("sonarpad-character-catalog-{stamp}"));
        let path = root.join("Catalogs").join("Serie.json");
        let context = AudioDescriptionCharacterCatalogContext {
            name: "Serie".to_string(),
            path: path.clone(),
            characters: Vec::new(),
        };
        save_audio_description_character_catalog(
            &context,
            &[
                BridgeCharacter {
                    id: "c1".to_string(),
                    name: "Anna".to_string(),
                    description: "Donna dai capelli scuri".to_string(),
                },
                BridgeCharacter {
                    id: "c2".to_string(),
                    name: "Marco".to_string(),
                    description: "Uomo con barba corta".to_string(),
                },
            ],
        )
        .expect("save first episode catalog");

        let loaded =
            load_audio_description_character_catalog_context("Serie".to_string(), path.clone())
                .expect("load catalog for second episode");
        assert_eq!(loaded.characters.len(), 2);
        let mut second_episode = loaded.characters.clone();
        second_episode.push(BridgeCharacter {
            id: "c1".to_string(),
            name: "Anna".to_string(),
            description: "Donna dai capelli scuri, occhi verdi e cappotto rosso".to_string(),
        });
        second_episode.push(BridgeCharacter {
            id: "c3".to_string(),
            name: "Luca".to_string(),
            description: "Ragazzo alto con capelli ricci".to_string(),
        });
        save_audio_description_character_catalog(&loaded, &second_episode)
            .expect("save second episode catalog");

        let updated = load_audio_description_character_catalog_context("Serie".to_string(), path)
            .expect("reload updated catalog");
        assert_eq!(updated.characters.len(), 3);
        let anna = updated
            .characters
            .iter()
            .find(|character| character.name == "Anna")
            .expect("Anna remains in catalog");
        assert!(anna.description.contains("occhi verdi"));
        assert_eq!(anna.id, "c1");
        assert!(
            updated
                .characters
                .iter()
                .any(|character| character.name == "Marco")
        );
        assert!(
            updated
                .characters
                .iter()
                .any(|character| character.name == "Luca")
        );
        crate::log_if_err!(
            std::fs::remove_dir_all(root),
            "Audio description cleanup operation failed"
        );
    }

    #[test]
    fn character_catalog_single_entry_normalization_does_not_index_empty_alias_matches() {
        let characters = vec![BridgeCharacter {
            id: "flo".to_string(),
            name: "Flo".to_string(),
            description: "Protagonista della serie.".to_string(),
        }];

        let normalized = normalize_catalog_characters(&characters);
        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].id, "flo");
        assert_eq!(normalized[0].name, "Flo");
    }

    #[test]
    fn character_catalog_preserves_authoritative_id_against_shortened_duplicate() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("sonarpad-character-id-{stamp}"));
        let path = root.join("Catalogs").join("Serie.json");
        let authoritative_description = format!(
            "{}{}",
            "Madre di Flo, Franz e Jack e moglie di Ernest. ",
            "Descrizione fisica stabile molto dettagliata. ".repeat(8)
        );
        let context = AudioDescriptionCharacterCatalogContext {
            name: "Serie".to_string(),
            path: path.clone(),
            characters: vec![BridgeCharacter {
                id: "anna_robinson".to_string(),
                name: "Anna Robinson".to_string(),
                description: authoritative_description.clone(),
            }],
        };
        save_audio_description_character_catalog(
            &context,
            &[BridgeCharacter {
                id: "anna".to_string(),
                name: "Anna".to_string(),
                description: "Indossa un abito azzurro chiaro con colletto alto.".to_string(),
            }],
        )
        .expect("save catalog with shortened Gemini alias");

        let loaded = load_audio_description_character_catalog_context("Serie".to_string(), path)
            .expect("reload authoritative catalog");
        assert_eq!(loaded.characters.len(), 1);
        assert_eq!(loaded.characters[0].id, "anna_robinson");
        assert_eq!(loaded.characters[0].name, "Anna Robinson");
        assert!(
            loaded.characters[0]
                .description
                .starts_with(&authoritative_description)
        );
        assert!(loaded.characters[0].description.contains("abito azzurro"));
        assert!(loaded.characters[0].description.chars().count() > 240);
        crate::log_if_err!(
            std::fs::remove_dir_all(root),
            "Audio description cleanup operation failed"
        );
    }

    #[test]
    fn character_catalog_rejects_repeated_and_corrupted_biography_sentences() {
        let existing = "Padre di Flo, Anna è sua moglie. Uomo adulto sui quarant’anni, medico, alto e robusto, con capelli castano scuro corti, folta barba e baffi scuri.";
        let observed = "Padre di Dio, Anna è sua moglie. Uomo adulto sui quarant'anni, medico, alto e robusto, con capelli castano scuro corti, folta barba e baffi scuri. Padre di Flo, medico robusto con barba e baffi scuri.";
        assert_eq!(merge_catalog_description(existing, observed), existing);
    }

    #[test]
    fn character_catalog_appends_only_genuinely_new_visual_sentence() {
        let existing = "Madre di Flo, Franz e Jack e moglie di Ernest. Donna adulta con capelli castano-ramati raccolti ordinatamente dietro la testa.";
        let observed = "Madre di Flo, Franz e Jack e moglie di Ernest. Indossa un abito azzurro chiaro con colletto alto volantato.";
        let merged = merge_catalog_description(existing, observed);
        assert!(merged.starts_with(existing));
        assert!(merged.contains("abito azzurro"));
        assert_eq!(merged.matches("Madre di Flo").count(), 1);
    }

    #[test]
    fn character_catalog_does_not_merge_ambiguous_first_name_only() {
        let established = vec![
            BridgeCharacter {
                id: "eric_capretto".to_string(),
                name: "Capretto Eric".to_string(),
                description: "Capretto giovane.".to_string(),
            },
            BridgeCharacter {
                id: "eric_beths".to_string(),
                name: "Eric Beths".to_string(),
                description: "Naufrago adulto.".to_string(),
            },
        ];
        let detected = vec![BridgeCharacter {
            id: "eric".to_string(),
            name: "Eric".to_string(),
            description: "Figura vista nel filmato.".to_string(),
        }];
        let merged = merge_catalog_characters(&established, &detected);
        assert_eq!(merged.len(), 3);
        assert!(
            merged
                .iter()
                .any(|character| character.id == "eric_capretto")
        );
        assert!(merged.iter().any(|character| character.id == "eric_beths"));
        assert!(merged.iter().any(|character| character.id == "eric"));
    }

    #[test]
    fn omni_port_project_edit_checks_the_exact_remaining_silence() {
        let job = AudioDescriptionJob {
            input_path: PathBuf::from("film.mkv"),
            output_path: PathBuf::from("film.mp3"),
            audio_stream_index: None,
            language_code: "it".to_string(),
            tts_language: Language::Italian,
            verbosity: AudioDescriptionVerbosity::Detailed,
            allow_extended_pauses: true,
            recognize_characters: true,
            recognize_screen_text: false,
            character_catalog: None,
            save_project: true,
            tts_engine: TtsEngine::Edge,
            tts_voice: "it-IT-ElsaNeural".to_string(),
            tts_rate: 0,
            tts_pitch: 0,
            tts_volume: 100,
            dictionary: Vec::new(),
            gemini_api_key: String::new(),
            sonarpad_ai_service_url: String::new(),
            sonarpad_ai_access_code: String::new(),
            sonarpad_ai_device_id: String::new(),
            gemini_model: "gemini-3.5-flash-lite".to_string(),
            audiobook_bitrate_kbps: 192,
            resume_checkpoint_path: None,
        };
        let scheduled = vec![
            ScheduledDescription {
                original_index: 0,
                text: "Prima".to_string(),
                desired_start_sec: 2.0,
                visual_evidence_time_sec: None,
                start_sec: 2.0,
                samples: Arc::from(vec![0.2_f32; 20]),
                sample_rate: 10,
                channels: 1,
                extended_pause: false,
            },
            ScheduledDescription {
                original_index: 1,
                text: "Seconda".to_string(),
                desired_start_sec: 7.0,
                visual_evidence_time_sec: None,
                start_sec: 7.0,
                samples: Arc::from(vec![0.2_f32; 10]),
                sample_rate: 10,
                channels: 1,
                extended_pause: false,
            },
        ];
        let project = build_audio_description_project(
            &job,
            12.0,
            12.0,
            &[
                BridgeInterval {
                    start_sec: 0.0,
                    end_sec: 1.0,
                },
                BridgeInterval {
                    start_sec: 10.0,
                    end_sec: 12.0,
                },
            ],
            &scheduled,
            &[],
        );
        let available = audio_description_project_edit_available_duration(&project, 0)
            .expect("available duration");
        assert_eq!(available, Some(5.0));
        assert!(validate_audio_description_project_edit_duration(available, 4.9).is_ok());
        assert!(matches!(
            validate_audio_description_project_edit_duration(available, 5.2),
            Err(AudioDescriptionProjectEditError::TooLong { .. })
        ));
    }

    #[test]
    fn omni_port_extended_pause_edit_has_no_fixed_duration_limit() {
        let job = AudioDescriptionJob {
            input_path: PathBuf::from("film.mkv"),
            output_path: PathBuf::from("film.mp3"),
            audio_stream_index: None,
            language_code: "it".to_string(),
            tts_language: Language::Italian,
            verbosity: AudioDescriptionVerbosity::Detailed,
            allow_extended_pauses: true,
            recognize_characters: true,
            recognize_screen_text: false,
            character_catalog: None,
            save_project: true,
            tts_engine: TtsEngine::Edge,
            tts_voice: "it-IT-ElsaNeural".to_string(),
            tts_rate: 0,
            tts_pitch: 0,
            tts_volume: 100,
            dictionary: Vec::new(),
            gemini_api_key: String::new(),
            sonarpad_ai_service_url: String::new(),
            sonarpad_ai_access_code: String::new(),
            sonarpad_ai_device_id: String::new(),
            gemini_model: "gemini-3.5-flash-lite".to_string(),
            audiobook_bitrate_kbps: 192,
            resume_checkpoint_path: None,
        };
        let scheduled = vec![ScheduledDescription {
            original_index: 0,
            text: "Pausa".to_string(),
            desired_start_sec: 2.0,
            visual_evidence_time_sec: None,
            start_sec: 2.0,
            samples: Arc::from(vec![0.2_f32; 20]),
            sample_rate: 10,
            channels: 1,
            extended_pause: true,
        }];
        let project = build_audio_description_project(&job, 5.0, 7.0, &[], &scheduled, &[]);
        let available = audio_description_project_edit_available_duration(&project, 0)
            .expect("available duration");
        assert_eq!(available, None);
        assert!(validate_audio_description_project_edit_duration(available, 120.0).is_ok());
    }

    #[test]
    fn omni_port_deleting_a_description_saves_the_project_immediately() {
        let job = AudioDescriptionJob {
            input_path: PathBuf::from("film.mkv"),
            output_path: PathBuf::from("film.mp3"),
            audio_stream_index: None,
            language_code: "it".to_string(),
            tts_language: Language::Italian,
            verbosity: AudioDescriptionVerbosity::Detailed,
            allow_extended_pauses: true,
            recognize_characters: true,
            recognize_screen_text: false,
            character_catalog: None,
            save_project: true,
            tts_engine: TtsEngine::Edge,
            tts_voice: "it-IT-ElsaNeural".to_string(),
            tts_rate: 0,
            tts_pitch: 0,
            tts_volume: 100,
            dictionary: Vec::new(),
            gemini_api_key: String::new(),
            sonarpad_ai_service_url: String::new(),
            sonarpad_ai_access_code: String::new(),
            sonarpad_ai_device_id: String::new(),
            gemini_model: "gemini-3.5-flash-lite".to_string(),
            audiobook_bitrate_kbps: 192,
            resume_checkpoint_path: None,
        };
        let scheduled = vec![
            ScheduledDescription {
                original_index: 0,
                text: "Prima".to_string(),
                desired_start_sec: 1.0,
                visual_evidence_time_sec: None,
                start_sec: 1.0,
                samples: Arc::from(vec![0.2_f32; 10]),
                sample_rate: 10,
                channels: 1,
                extended_pause: false,
            },
            ScheduledDescription {
                original_index: 1,
                text: "Seconda".to_string(),
                desired_start_sec: 3.0,
                visual_evidence_time_sec: None,
                start_sec: 3.0,
                samples: Arc::from(vec![0.2_f32; 10]),
                sample_rate: 10,
                channels: 1,
                extended_pause: false,
            },
        ];
        let project = build_audio_description_project(&job, 5.0, 5.0, &[], &scheduled, &[]);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("sonarpad_ad_delete_{stamp}.json"));
        save_audio_description_project(&path, &project).expect("initial save");
        let updated = delete_audio_description_project_description(&path, &project, 0)
            .expect("delete description");
        assert_eq!(updated.descriptions.len(), 1);
        assert_eq!(updated.descriptions[0].text, "Seconda");
        let reloaded = load_audio_description_project(&path).expect("reload saved project");
        assert_eq!(reloaded.descriptions.len(), 1);
        assert_eq!(reloaded.descriptions[0].text, "Seconda");
        crate::log_if_err!(
            std::fs::remove_file(path),
            "Audio description cleanup operation failed"
        );
    }
}

#[cfg(test)]
mod isolated_sapi5_description_tests {
    use super::audio_description_sapi5_retry_limit;
    use crate::settings::TtsEngine;

    #[test]
    fn batch_recovery_keeps_successes_and_original_order() -> Result<(), String> {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut attempts = [0; 10];
        let mut progress = Vec::new();
        let (results, limit) = super::run_description_batches(
            10,
            TtsEngine::Sapi5,
            8,
            &cancel,
            |indices| {
                indices
                    .iter()
                    .map(|&index| {
                        attempts[index] += 1;
                        if (index == 1 || index == 3) && attempts[index] == 1 {
                            Err("SAPI5 isolated synthesis failed: simulated native crash"
                                .to_string())
                        } else {
                            Ok(index)
                        }
                    })
                    .collect()
            },
            |done, total| progress.push((done, total)),
        )?;
        assert_eq!(results, (0..10).collect::<Vec<_>>());
        assert_eq!(limit, 4);
        assert_eq!(attempts, [1, 2, 1, 2, 1, 1, 1, 1, 1, 1]);
        assert_eq!(progress, (1..=10).map(|n| (n, 10)).collect::<Vec<_>>());
        Ok(())
    }

    #[test]
    fn persistent_worker_failure_stops_at_one_and_cancel_does_not_retry() {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut calls = 0;
        let result = super::run_description_batches::<usize, _, _>(
            1,
            TtsEngine::Sapi5,
            8,
            &cancel,
            |_| {
                calls += 1;
                vec![Err("SAPI5 isolated synthesis failed: crash".to_string())]
            },
            |_, _| {},
        );
        assert!(result.is_err());
        assert_eq!(calls, 4);
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        let result = super::run_description_batches::<usize, _, _>(
            1,
            TtsEngine::Sapi5,
            8,
            &cancel,
            |_| {
                calls += 1;
                vec![Ok(0)]
            },
            |_, _| {},
        );
        assert_eq!(result, Err("cancelled".to_string()));
        assert_eq!(calls, 4);
    }

    #[test]
    fn only_isolated_sapi5_failures_reduce_concurrency_and_retries_are_bounded() {
        let crash = "SAPI5 isolated synthesis failed: worker terminated unexpectedly";
        assert_eq!(
            audio_description_sapi5_retry_limit(TtsEngine::Sapi5, 8, crash),
            Some(4)
        );
        assert_eq!(
            audio_description_sapi5_retry_limit(TtsEngine::Sapi5, 4, crash),
            Some(2)
        );
        assert_eq!(
            audio_description_sapi5_retry_limit(TtsEngine::Sapi5, 2, crash),
            Some(1)
        );
        assert_eq!(
            audio_description_sapi5_retry_limit(TtsEngine::Sapi5, 1, crash),
            None
        );
        assert_eq!(
            audio_description_sapi5_retry_limit(TtsEngine::Edge, 8, crash),
            None
        );
        assert_eq!(
            audio_description_sapi5_retry_limit(TtsEngine::Sapi5, 8, "disk full"),
            None
        );
        assert_eq!(
            audio_description_sapi5_retry_limit(TtsEngine::Sapi5, 8, "cancelled"),
            None
        );
    }
}
