use crate::editor_manager::get_edit_text;
use crate::file_handler::{is_epub_path, read_epub_chapters};
use crate::i18n;
use crate::settings;
use crate::settings::{
    AudiobookPartAnnouncementMode, AudiobookPartNamingMode, AudiobookResult, DictionaryEntry,
    Language, TRUSTED_CLIENT_TOKEN, TtsEngine,
};
use crate::{get_active_edit, log_debug, save_audio_dialog, show_error, with_state};
use chrono::Local;
use cpal::Sample;
use futures_util::{SinkExt, StreamExt, future::join_all};
use rand::Rng;
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStreamBuilder, Sink, Source};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{BufWriter, Cursor, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async, tungstenite, tungstenite::client::IntoClientRequest,
    tungstenite::http::HeaderValue, tungstenite::protocol::Message,
};
use url::Url;
use uuid::Uuid;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Diagnostics::Debug::{
    SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SetErrorMode,
};
use windows::Win32::System::Power::{ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState};
use windows::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
};
use windows::Win32::UI::Controls::RichEdit::{CHARRANGE, EM_EXGETSEL, EM_GETTEXTRANGE, TEXTRANGEW};
use windows::Win32::UI::WindowsAndMessaging::{
    PostMessageW, SendMessageW, WM_APP, WM_GETTEXTLENGTH,
};
use windows::core::PWSTR;

use crate::audio_utils::WavWriter;
use crate::subtitle_wasapi::{decode_mp3_to_pcm, resample_pcm};

pub const WSS_URL_BASE: &str =
    "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1";
pub const MAX_TTS_TEXT_LEN: usize = 3000;
pub const MAX_TTS_TEXT_LEN_LONG: usize = 2000;
pub const MAX_TTS_FIRST_CHUNK_LEN_LONG: usize = 800;
pub const TTS_LONG_TEXT_THRESHOLD: usize = MAX_TTS_TEXT_LEN;
const GOOGLE_PLAYBACK_FIRST_CHUNK_MAX_CHARS: usize = 100;
pub(crate) const PAUSE_TAG_MIN_MS: u32 = 50;
pub(crate) const PAUSE_TAG_MAX_MS: u32 = 60_000;
// Some voices silently truncate long SSML payloads without returning an error.
// Keep Edge chunks conservative to avoid partial audiobook exports.
const EDGE_TTS_MAX_BYTES: usize = 1800;
const KEEP_EDGE_TEMP_AFTER_CONVERSION: bool = false;

fn lower_current_audiobook_worker_priority(context: &str) {
    unsafe {
        if let Err(err) = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) {
            crate::log_debug(&format!(
                "Audiobook {}: failed to lower worker priority: {}",
                context, err
            ));
        }
    }
}

pub(crate) const SAPI4_MAX_PARALLEL_WORKERS: usize = 64;

fn requested_sapi4_worker_limit(requested: usize) -> usize {
    requested.clamp(1, SAPI4_MAX_PARALLEL_WORKERS)
}

fn sapi4_next_lower_worker_limit(current: usize) -> usize {
    match current {
        0..=2 => 1,
        3..=4 => 2,
        5..=8 => 4,
        9..=12 => 8,
        13..=20 => 12,
        21..=32 => 20,
        33..=48 => 32,
        _ => 48,
    }
}

pub const WM_TTS_PLAYBACK_DONE: u32 = WM_APP + 3;
pub const WM_TTS_PLAYBACK_ERROR: u32 = WM_APP + 5;
pub const WM_TTS_CHUNK_START: u32 = WM_APP + 7;

pub enum TtsCommand {
    Pause,
    Resume,
    Stop,
}

pub struct TtsSession {
    pub id: u64,
    pub command_tx: mpsc::UnboundedSender<TtsCommand>,
    pub cancel: Arc<AtomicBool>,
    pub paused: bool,
    pub initial_caret_pos: i32,
    pub source_edit: HWND,
}

#[derive(Clone)]
pub struct TtsChunk {
    pub text_to_read: String,
    pub original_len: usize,
    pub override_voice: Option<VoiceOverride>,
    pub pause_ms: Option<u32>,
}

type TtsAudioPacket = (Vec<u8>, usize, String, Option<u32>);
type EdgeAudioTx<'a> = Option<&'a mpsc::Sender<Result<TtsAudioPacket, String>>>;

#[derive(Clone)]
pub struct VoiceOverride {
    pub engine: TtsEngine,
    pub voice: String,
    pub rate: Option<i32>,
    pub pitch: Option<i32>,
    pub volume: Option<i32>,
}

fn parse_voice_tag_override(tag: &str, default_engine: TtsEngine) -> Option<VoiceOverride> {
    let raw = tag.trim();
    if raw.is_empty() {
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    let mut engine: Option<TtsEngine> = None;
    let mut voice: Option<String> = None;

    for key in ["engine", "tts"] {
        if let Some(pos) = lower.find(key)
            && let Some(eq_pos) = lower[pos..].find('=')
        {
            let val_start = pos + eq_pos + 1;
            let val = raw[val_start..].trim_start();
            let val = if let Some(rest) = val.strip_prefix('"') {
                rest.split('"').next().unwrap_or("").to_string()
            } else {
                val.split_whitespace().next().unwrap_or("").to_string()
            };
            engine = match val.to_ascii_lowercase().as_str() {
                "edge" => Some(TtsEngine::Edge),
                "sapi4" => Some(TtsEngine::Sapi4),
                "sapi5" => Some(TtsEngine::Sapi5),
                "google" => Some(TtsEngine::Google),
                _ => None,
            };
        }
    }
    for key in ["voice", "name"] {
        if let Some(pos) = lower.find(key)
            && let Some(eq_pos) = lower[pos..].find('=')
        {
            let val_start = pos + eq_pos + 1;
            let val = raw[val_start..].trim_start();
            let val = if let Some(rest) = val.strip_prefix('"') {
                rest.split('"').next().unwrap_or("").to_string()
            } else {
                val.split_whitespace().next().unwrap_or("").to_string()
            };
            if !val.is_empty() {
                voice = Some(val);
            }
        }
    }

    let mut rate: Option<i32> = None;
    let mut pitch: Option<i32> = None;
    let mut volume: Option<i32> = None;

    for key in ["rate", "speed", "pitch", "volume"] {
        if let Some(pos) = lower.find(key)
            && let Some(eq_pos) = lower[pos..].find('=')
        {
            let val_start = pos + eq_pos + 1;
            let val = raw[val_start..].trim_start();
            let val = if let Some(rest) = val.strip_prefix('"') {
                rest.split('"').next().unwrap_or("")
            } else {
                val.split_whitespace().next().unwrap_or("")
            };
            if let Ok(parsed) = val.parse::<i32>() {
                match key {
                    "rate" | "speed" => rate = Some(parsed),
                    "pitch" => pitch = Some(parsed),
                    "volume" => volume = Some(parsed),
                    _ => {}
                }
            }
        }
    }

    const KNOWN_KEYS: &[&str] = &[
        "engine", "tts", "voice", "name", "rate", "speed", "pitch", "volume",
    ];
    let is_known_attr = |token: &str| {
        let t = token.to_ascii_lowercase();
        KNOWN_KEYS.iter().any(|k| t.starts_with(&format!("{k}=")))
    };

    let mut tokens: Vec<&str> = raw.split_whitespace().collect();
    if let Some(first) = tokens.first().copied() {
        let first_lower = first.to_ascii_lowercase();
        if engine.is_none() {
            engine = match first_lower.as_str() {
                "edge" => Some(TtsEngine::Edge),
                "sapi4" => Some(TtsEngine::Sapi4),
                "sapi5" => Some(TtsEngine::Sapi5),
                "google" => Some(TtsEngine::Google),
                _ => None,
            };
            if engine.is_some() {
                tokens.remove(0);
            }
        }
    }
    if voice.is_none() && !tokens.is_empty() {
        let name_tokens: Vec<&str> = tokens.into_iter().filter(|t| !is_known_attr(t)).collect();
        let merged = name_tokens.join(" ");
        if !merged.is_empty() {
            voice = Some(merged);
        }
    }

    let voice = voice?;
    let engine = engine.unwrap_or(default_engine);

    Some(VoiceOverride {
        engine,
        voice,
        rate,
        pitch,
        volume,
    })
}

pub(crate) fn has_voice_tags(text: &str) -> bool {
    text.to_ascii_lowercase().contains("<voice")
}

pub(crate) fn has_pause_tags(text: &str) -> bool {
    text.to_ascii_lowercase().contains("<pause")
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn decode_basic_xml_entities(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

pub(crate) fn split_voice_tag_spans(
    text: &str,
    default_engine: TtsEngine,
) -> Vec<(String, Option<VoiceOverride>, usize)> {
    if text.is_empty() {
        return Vec::new();
    }
    let lower = text.to_ascii_lowercase();
    let mut segments = Vec::new();
    let mut cursor = 0usize;
    let mut pending_len = 0usize;
    let mut current_override: Option<VoiceOverride> = None;
    // Dialogue voice tags inserted automatically by Sonarpad are playback
    // metadata, not characters that exist in the editor. Keep track of those
    // tags so cursor offsets remain relative to the user's original text.
    let mut current_voice_tag_is_synthetic = false;

    let mut i = 0usize;
    let bytes = lower.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let remaining = &lower[i..];
            let is_open = remaining.starts_with("<voice");
            let is_close = remaining.starts_with("</voice");
            if (is_open || is_close)
                && let Some(end_rel) = remaining.find('>')
            {
                let end = i + end_rel + 1;
                if i > cursor {
                    let chunk = &text[cursor..i];
                    if !chunk.is_empty() {
                        let mut orig_len = utf16_len(chunk);
                        if pending_len > 0 {
                            orig_len += pending_len;
                            pending_len = 0;
                        }
                        segments.push((
                            decode_basic_xml_entities(chunk),
                            current_override.clone(),
                            orig_len,
                        ));
                    }
                }
                let tag_text = &text[i..end];
                let tag_len = utf16_len(tag_text);
                if is_open {
                    let tag_inner = text[i + "<voice".len()..end - 1].trim();
                    let is_synthetic_dialogue_tag = tag_inner.contains("sonarpad-dialogue=\"1\"");
                    if !is_synthetic_dialogue_tag {
                        pending_len += tag_len;
                    }
                    current_voice_tag_is_synthetic = is_synthetic_dialogue_tag;
                    current_override = parse_voice_tag_override(tag_inner, default_engine);
                } else {
                    if !current_voice_tag_is_synthetic {
                        pending_len += tag_len;
                    }
                    current_voice_tag_is_synthetic = false;
                    current_override = None;
                }
                cursor = end;
                i = end;
                continue;
            }
        }
        i += 1;
    }

    if cursor < text.len() {
        let tail = &text[cursor..];
        if !tail.is_empty() {
            let mut orig_len = utf16_len(tail);
            if pending_len > 0 {
                orig_len += pending_len;
            }
            segments.push((
                decode_basic_xml_entities(tail),
                current_override.clone(),
                orig_len,
            ));
        }
    } else if pending_len > 0
        && let Some(last) = segments.last_mut()
    {
        last.2 += pending_len;
    }

    if segments.is_empty() {
        segments.push((text.to_string(), None, utf16_len(text)));
    }
    for (i, (txt, ov, len)) in segments.iter().enumerate() {
        crate::log_debug(&format!(
            "split_voice_tag_spans[{}]: text_preview={:?} override_engine={} override_voice={} rate={:?} pitch={:?} vol={:?} len={}",
            i,
            preview_for_log(txt, 120),
            ov.as_ref()
                .map(|o| match o.engine {
                    TtsEngine::Edge => "edge",
                    TtsEngine::Sapi5 => "sapi5",
                    TtsEngine::Sapi4 => "sapi4",
                    TtsEngine::Google => "google",
                })
                .unwrap_or("(none)"),
            ov.as_ref().map(|o| o.voice.as_str()).unwrap_or("(none)"),
            ov.as_ref().and_then(|o| o.rate),
            ov.as_ref().and_then(|o| o.pitch),
            ov.as_ref().and_then(|o| o.volume),
            len,
        ));
    }
    segments
}

pub struct TtsPlaybackOptions {
    pub hwnd: HWND,
    pub engine: TtsEngine,
    pub cleaned: String,
    pub voice: String,
    pub chunks: Vec<TtsChunk>,
    pub initial_caret_pos: i32,
    pub source_edit: HWND,
    pub rate: i32,
    pub pitch: i32,
    pub volume: i32,
}

struct TtsQueuedPlayback {
    hwnd: HWND,
    engine: TtsEngine,
    text: String,
    voice: String,
    split_on_newline: bool,
    dictionary: Vec<DictionaryEntry>,
    initial_caret_pos: i32,
    source_edit: HWND,
    rate: i32,
    pitch: i32,
    volume: i32,
}

pub struct AudiobookCommonOptions<'a> {
    pub voice: &'a str,
    pub output: &'a Path,
    pub progress_hwnd: HWND,
    pub cancel: Arc<AtomicBool>,
    pub language: Language,
    pub part_naming_mode: AudiobookPartNamingMode,
    pub part_announcement_mode: AudiobookPartAnnouncementMode,
    pub audiobook_title: &'a str,
    pub audiobook_bitrate_kbps: u32,
    pub rate: i32,
    pub pitch: i32,
    pub volume: i32,
    pub sapi4_threads: Option<u32>,
}

fn post_audiobook_progress(hwnd: HWND, current: usize) {
    if hwnd.0 == 0 {
        return;
    }
    unsafe {
        if let Err(e) = PostMessageW(hwnd, crate::WM_UPDATE_PROGRESS, WPARAM(current), LPARAM(0)) {
            crate::log_debug(&format!("Failed to post WM_UPDATE_PROGRESS: {}", e));
        }
    }
}

fn set_audiobook_progress_total(hwnd: HWND, total: usize) {
    if hwnd.0 == 0 {
        return;
    }
    unsafe {
        if let Err(e) = PostMessageW(
            hwnd,
            crate::app_windows::audiobook_window::WM_SET_PROGRESS_TOTAL,
            WPARAM(total.max(1)),
            LPARAM(0),
        ) {
            crate::log_debug(&format!("Failed to post WM_SET_PROGRESS_TOTAL: {}", e));
        }
    }
}

fn set_audiobook_progress_phase(hwnd: HWND, finalizing: bool) {
    if hwnd.0 == 0 {
        return;
    }
    unsafe {
        if let Err(e) = PostMessageW(
            hwnd,
            crate::app_windows::audiobook_window::WM_SET_PROGRESS_PHASE,
            WPARAM(usize::from(finalizing)),
            LPARAM(0),
        ) {
            crate::log_debug(&format!("Failed to post WM_SET_PROGRESS_PHASE: {}", e));
        }
    }
}

fn post_finalization_progress_range(
    progress_hwnd: HWND,
    progress_10000: u32,
    start: usize,
    span: usize,
    total: usize,
) {
    if progress_hwnd.0 == 0 || total == 0 || span == 0 {
        return;
    }
    let clamped = progress_10000.min(10_000) as usize;
    let offset = (clamped * span) / 10_000;
    let current = (start + offset).min(total.saturating_sub(1));
    post_audiobook_progress(progress_hwnd, current);
}

fn cancelled_message(language: Language) -> String {
    i18n::tr(language, "tts.cancelled")
}

pub fn prevent_sleep(enable: bool) {
    unsafe {
        if enable {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
        } else {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

fn post_tts_chunk_offset(hwnd: HWND, session_id: u64, offset: usize) {
    if hwnd.0 == 0 {
        return;
    }
    unsafe {
        if let Err(e) = PostMessageW(
            hwnd,
            WM_TTS_CHUNK_START,
            WPARAM(session_id as usize),
            LPARAM(offset as isize),
        ) {
            crate::log_debug(&format!("Failed to post WM_TTS_CHUNK_START: {}", e));
        }
    }
}

pub fn post_tts_error(hwnd: HWND, session_id: u64, message: String) {
    log_debug(&format!("TTS error: {message}"));
    let payload = Box::new(message);
    unsafe {
        if let Err(e) = PostMessageW(
            hwnd,
            WM_TTS_PLAYBACK_ERROR,
            WPARAM(session_id as usize),
            LPARAM(Box::into_raw(payload) as isize),
        ) {
            crate::log_debug(&format!("Failed to post WM_TTS_PLAYBACK_ERROR: {}", e));
        }
    }
}

pub fn start_tts_from_caret(hwnd: HWND) {
    let Some(hwnd_edit) = get_active_edit(hwnd) else {
        return;
    };
    let (
        language,
        split_on_newline,
        tts_engine,
        dictionary,
        tts_rate,
        tts_pitch,
        tts_volume,
        move_cursor_during_reading,
    ) = {
        with_state(hwnd, |state| {
            (
                state.settings.language,
                state.settings.split_on_newline,
                state.settings.tts_engine,
                state.settings.dictionary.clone(),
                state.settings.tts_rate,
                state.settings.tts_pitch,
                state.settings.tts_volume,
                state.settings.move_cursor_during_reading,
            )
        })
    }
    .unwrap_or((
        Language::Italian,
        true,
        TtsEngine::Edge,
        Vec::new(),
        0,
        0,
        100,
        false,
    ));

    let (mut text, initial_caret_pos) = get_text_from_caret(hwnd_edit);
    if with_state(hwnd, |state| {
        state.tts_sentence_nav_anchor = Some((hwnd_edit, initial_caret_pos.max(0)));
    })
    .is_none()
    {
        crate::log_debug("Failed to update TTS sentence navigation anchor");
    }
    let dialogue_settings =
        { with_state(hwnd, |state| state.settings.clone()) }.unwrap_or_default();
    text = crate::dialogue_voice::apply_dialogue_tags_from_settings(&text, &dialogue_settings);
    if text.trim().is_empty() {
        show_error(hwnd, language, &settings::tts_no_text_message(language));
        return;
    }
    let voice = {
        with_state(hwnd, |state| state.settings.tts_voice.clone())
            .unwrap_or_else(|| "it-IT-IsabellaNeural".to_string())
    };
    let has_tags = has_voice_tags(&text);
    let has_pause = has_pause_tags(&text);

    // Edge and Google already use the shared chunked playback path.
    // When cursor following is enabled, route SAPI4 and SAPI5 through the same
    // path as well: every engine then reports progress from the actual decoded
    // audio and uses the same sentence/clause boundaries (. ! ? ; :).
    let needs_shared_cursor_progress =
        move_cursor_during_reading && matches!(tts_engine, TtsEngine::Sapi4 | TtsEngine::Sapi5);
    if (has_tags && tts_engine != TtsEngine::Edge) || has_pause || needs_shared_cursor_progress {
        if needs_shared_cursor_progress {
            let engine_label = match tts_engine {
                TtsEngine::Sapi4 => "sapi4",
                TtsEngine::Sapi5 => "sapi5",
                TtsEngine::Edge => "edge",
                TtsEngine::Google => "google",
            };
            crate::log_debug(&format!(
                "TTS: shared cursor progress enabled for engine={engine_label}"
            ));
            if tts_engine == TtsEngine::Sapi5 {
                crate::log_debug(&format!(
                    "SAPI5 DIAG: route selected=shared_cursor voice={} rate={} pitch={} volume={} has_tags={} has_pause={}",
                    voice, tts_rate, tts_pitch, tts_volume, has_tags, has_pause
                ));
            }
        }
        queue_tts_playback_from_text(TtsQueuedPlayback {
            hwnd,
            engine: tts_engine,
            text,
            voice,
            split_on_newline,
            dictionary,
            initial_caret_pos,
            source_edit: hwnd_edit,
            rate: tts_rate,
            pitch: tts_pitch,
            volume: tts_volume,
        });
        return;
    }

    match tts_engine {
        TtsEngine::Edge | TtsEngine::Google => queue_tts_playback_from_text(TtsQueuedPlayback {
            hwnd,
            engine: tts_engine,
            text,
            voice,
            split_on_newline,
            dictionary,
            initial_caret_pos,
            source_edit: hwnd_edit,
            rate: tts_rate,
            pitch: tts_pitch,
            volume: tts_volume,
        }),
        TtsEngine::Sapi4 => {
            stop_tts_playback(hwnd);
            let voice_idx = if let Some(hash_pos) = voice.find("#") {
                let rest = &voice[hash_pos + 1..];
                if let Some(pipe_pos) = rest.find("|") {
                    rest[..pipe_pos].parse::<i32>().unwrap_or(1)
                } else {
                    rest.parse::<i32>().unwrap_or(1)
                }
            } else {
                1
            };
            let cancel = Arc::new(AtomicBool::new(false));
            let (command_tx, command_rx) = mpsc::unbounded_channel();
            if {
                with_state(hwnd, |state| {
                    state.tts_session = Some(TtsSession {
                        id: state.tts_next_session_id,
                        command_tx,
                        cancel: cancel.clone(),
                        paused: false,
                        initial_caret_pos,
                        source_edit: hwnd_edit,
                    });
                    state.tts_next_session_id += 1;
                })
            }
            .is_none()
            {
                crate::log_debug("Failed to update TTS session state");
            }
            crate::sapi4_engine::play_sapi4(
                voice_idx, text, tts_rate, tts_pitch, tts_volume, cancel, command_rx,
            );
        }
        TtsEngine::Sapi5 => {
            crate::log_debug(&format!(
                "SAPI5 DIAG: route selected=direct voice={} rate={} pitch={} volume={}",
                voice, tts_rate, tts_pitch, tts_volume
            ));
            // Stop any existing playback
            stop_tts_playback(hwnd);
            let cancel = Arc::new(AtomicBool::new(false));
            let (command_tx, command_rx) = mpsc::unbounded_channel();
            if {
                with_state(hwnd, |state| {
                    state.tts_session = Some(TtsSession {
                        id: state.tts_next_session_id,
                        command_tx,
                        cancel: cancel.clone(),
                        paused: false,
                        initial_caret_pos,
                        source_edit: hwnd_edit,
                    });
                    state.tts_next_session_id += 1;
                })
            }
            .is_none()
            {
                crate::log_debug("Failed to update TTS session state");
            }

            let chunks = split_into_tts_chunks(&text, split_on_newline, &dictionary, tts_engine);
            let chunk_strings: Vec<String> = chunks.into_iter().map(|c| c.text_to_read).collect();
            if let Err(e) = crate::sapi5_engine::play_sapi(
                chunk_strings,
                voice,
                tts_rate,
                tts_pitch,
                tts_volume,
                cancel,
                command_rx,
            ) {
                crate::log_debug(&format!("SAPI5 playback error: {}", e));
            }
        }
    }
}

pub fn speak_text_once(hwnd: HWND, text: String) {
    let text = text.trim().to_string();
    if text.is_empty() {
        return;
    }
    let (split_on_newline, tts_engine, dictionary, voice, tts_rate, tts_pitch, tts_volume) =
        with_state(hwnd, |state| {
            (
                state.settings.split_on_newline,
                state.settings.tts_engine,
                state.settings.dictionary.clone(),
                state.settings.tts_voice.clone(),
                state.settings.tts_rate,
                state.settings.tts_pitch,
                state.settings.tts_volume,
            )
        })
        .unwrap_or((
            true,
            TtsEngine::Edge,
            Vec::new(),
            "it-IT-IsabellaNeural".to_string(),
            0,
            0,
            100,
        ));

    let initial_caret_pos = 0;
    if has_pause_tags(&text) {
        queue_tts_playback_from_text(TtsQueuedPlayback {
            hwnd,
            engine: tts_engine,
            text,
            voice,
            split_on_newline,
            dictionary,
            initial_caret_pos,
            source_edit: HWND(0),
            rate: tts_rate,
            pitch: tts_pitch,
            volume: tts_volume,
        });
        return;
    }
    match tts_engine {
        TtsEngine::Edge | TtsEngine::Google => queue_tts_playback_from_text(TtsQueuedPlayback {
            hwnd,
            engine: tts_engine,
            text,
            voice,
            split_on_newline,
            dictionary,
            initial_caret_pos,
            source_edit: HWND(0),
            rate: tts_rate,
            pitch: tts_pitch,
            volume: tts_volume,
        }),
        TtsEngine::Sapi4 => {
            stop_tts_playback(hwnd);
            let voice_idx = if let Some(hash_pos) = voice.find("#") {
                let rest = &voice[hash_pos + 1..];
                if let Some(pipe_pos) = rest.find("|") {
                    rest[..pipe_pos].parse::<i32>().unwrap_or(1)
                } else {
                    rest.parse::<i32>().unwrap_or(1)
                }
            } else {
                1
            };
            let cancel = Arc::new(AtomicBool::new(false));
            let (command_tx, command_rx) = mpsc::unbounded_channel();
            if with_state(hwnd, |state| {
                state.tts_session = Some(TtsSession {
                    id: state.tts_next_session_id,
                    command_tx,
                    cancel: cancel.clone(),
                    paused: false,
                    initial_caret_pos,
                    source_edit: HWND(0),
                });
                state.tts_next_session_id += 1;
            })
            .is_none()
            {
                crate::log_debug("Failed to update subtitle TTS session state");
            }
            crate::sapi4_engine::play_sapi4(
                voice_idx, text, tts_rate, tts_pitch, tts_volume, cancel, command_rx,
            );
        }
        TtsEngine::Sapi5 => {
            stop_tts_playback(hwnd);
            let cancel = Arc::new(AtomicBool::new(false));
            let (command_tx, command_rx) = mpsc::unbounded_channel();
            if with_state(hwnd, |state| {
                state.tts_session = Some(TtsSession {
                    id: state.tts_next_session_id,
                    command_tx,
                    cancel: cancel.clone(),
                    paused: false,
                    initial_caret_pos,
                    source_edit: HWND(0),
                });
                state.tts_next_session_id += 1;
            })
            .is_none()
            {
                crate::log_debug("Failed to update subtitle TTS session state");
            }

            let chunks = split_into_tts_chunks(&text, split_on_newline, &dictionary, tts_engine);
            let chunk_strings: Vec<String> = chunks.into_iter().map(|c| c.text_to_read).collect();
            if let Err(e) = crate::sapi5_engine::play_sapi(
                chunk_strings,
                voice,
                tts_rate,
                tts_pitch,
                tts_volume,
                cancel,
                command_rx,
            ) {
                crate::log_debug(&format!("SAPI5 subtitle playback error: {}", e));
            }
        }
    }
}

fn google_playback_startup_split_index(text: &str, max_chars: usize) -> Option<usize> {
    if max_chars == 0 || text.chars().count() <= max_chars {
        return None;
    }

    let min_natural_boundary = (max_chars / 3).max(1);
    let mut natural_boundary = None;
    let mut whitespace_boundary = None;
    let mut hard_boundary = None;
    for (char_index, (byte_index, ch)) in text.char_indices().enumerate() {
        let char_count = char_index + 1;
        if char_count > max_chars {
            break;
        }
        let after_char = byte_index + ch.len_utf8();
        hard_boundary = Some(after_char);
        if ch.is_whitespace() {
            whitespace_boundary = Some(byte_index);
        }
        if char_count >= min_natural_boundary
            && matches!(
                ch,
                '.' | ':' | ';' | '?' | '!' | ',' | '\n' | '\r' | '—' | '–'
            )
        {
            natural_boundary = Some(after_char);
        }
    }

    natural_boundary.or(whitespace_boundary).or(hard_boundary)
}

fn optimize_google_playback_startup(chunks: &mut Vec<TtsChunk>, default_engine: TtsEngine) {
    let Some(first_playable_index) = chunks.iter().position(|chunk| chunk.pause_ms.is_none())
    else {
        return;
    };
    let first = &chunks[first_playable_index];
    let engine = first
        .override_voice
        .as_ref()
        .map(|voice| voice.engine)
        .unwrap_or(default_engine);
    if engine != TtsEngine::Google {
        return;
    }

    let original_chars = first.text_to_read.chars().count();
    let Some(split_index) = google_playback_startup_split_index(
        &first.text_to_read,
        GOOGLE_PLAYBACK_FIRST_CHUNK_MAX_CHARS,
    ) else {
        return;
    };
    let head_text = first.text_to_read[..split_index].trim().to_string();
    let tail_text = first.text_to_read[split_index..].trim().to_string();
    if head_text.is_empty() || tail_text.is_empty() {
        return;
    }

    let total_text_len = utf16_len(&first.text_to_read).max(1);
    let mut head_original_len =
        first.original_len.saturating_mul(utf16_len(&head_text)) / total_text_len;
    if first.original_len >= 2 {
        head_original_len = head_original_len.clamp(1, first.original_len - 1);
    }
    let tail_original_len = first.original_len.saturating_sub(head_original_len);
    let override_voice = first.override_voice.clone();
    chunks[first_playable_index] = TtsChunk {
        text_to_read: head_text,
        original_len: head_original_len,
        override_voice: override_voice.clone(),
        pause_ms: None,
    };
    chunks.insert(
        first_playable_index + 1,
        TtsChunk {
            text_to_read: tail_text,
            original_len: tail_original_len,
            override_voice,
            pause_ms: None,
        },
    );
    crate::log_debug(&format!(
        "Google playback startup split: original_chars={} first_chars={} remaining_chars={}",
        original_chars,
        chunks[first_playable_index].text_to_read.chars().count(),
        chunks[first_playable_index + 1]
            .text_to_read
            .chars()
            .count()
    ));
}

fn queue_tts_playback_from_text(options: TtsQueuedPlayback) {
    std::thread::spawn(move || {
        let mut chunks = split_into_tts_chunks(
            &options.text,
            options.split_on_newline,
            &options.dictionary,
            options.engine,
        );
        optimize_google_playback_startup(&mut chunks, options.engine);
        let payload = Box::new(TtsPlaybackOptions {
            hwnd: options.hwnd,
            engine: options.engine,
            cleaned: options.text,
            voice: options.voice,
            chunks,
            initial_caret_pos: options.initial_caret_pos,
            source_edit: options.source_edit,
            rate: options.rate,
            pitch: options.pitch,
            volume: options.volume,
        });
        unsafe {
            let payload_ptr = Box::into_raw(payload);
            if PostMessageW(
                options.hwnd,
                crate::WM_TTS_START,
                WPARAM(0),
                LPARAM(payload_ptr as isize),
            )
            .is_err()
            {
                crate::log_debug("Failed to post WM_TTS_START");
                let _unused_box = Box::from_raw(payload_ptr);
            }
        }
    });
}

pub fn toggle_tts_pause(hwnd: HWND) {
    if {
        with_state(hwnd, |state| {
            let Some(session) = &mut state.tts_session else {
                return;
            };
            if session.paused {
                prevent_sleep(true);
                if let Err(e) = session.command_tx.send(TtsCommand::Resume) {
                    crate::log_debug(&format!("Failed to send Resume command: {}", e));
                }
                session.paused = false;
            } else {
                prevent_sleep(false);
                if let Err(e) = session.command_tx.send(TtsCommand::Pause) {
                    crate::log_debug(&format!("Failed to send Pause command: {}", e));
                }
                session.paused = true;
            }
        })
    }
    .is_none()
    {
        crate::log_debug("Failed to access TTS session state for pause/resume");
    }
}

pub fn stop_tts_playback(hwnd: HWND) {
    crate::telemetry::set_tts_active(false);
    prevent_sleep(false);
    if {
        with_state(hwnd, |state| {
            if let Some(session) = &state.tts_session {
                session.cancel.store(true, Ordering::SeqCst);
                if let Err(e) = session.command_tx.send(TtsCommand::Stop) {
                    crate::log_debug(&format!("Failed to send Stop command: {}", e));
                }
            }
            state.tts_session = None;
            // A future playback session starts from the editor caret and its
            // chunk offsets are relative to that new position.  Keeping the
            // previous session's offset would make the monotonicity guard
            // clamp the new progress to the old value after Stop -> Start.
            state.tts_last_offset = 0;
            state.tts_pending_start_pos = None;
        })
    }
    .is_none()
    {
        crate::log_debug("Failed to access TTS session state for stop");
    }
}

fn handle_tts_command(
    cmd: TtsCommand,
    sink: &Sink,
    cancel_flag: &AtomicBool,
    paused: &mut bool,
) -> bool {
    match cmd {
        TtsCommand::Pause => {
            sink.pause();
            *paused = true;
            false
        }
        TtsCommand::Resume => {
            sink.play();
            *paused = false;
            false
        }
        TtsCommand::Stop => {
            cancel_flag.store(true, Ordering::SeqCst);
            sink.stop();
            true
        }
    }
}

fn temp_wav_path(prefix: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let id = Uuid::new_v4().simple().to_string();
    path.push(format!("sonarpad_{prefix}_{id}.wav"));
    path
}

fn synthesize_sapi5_bytes(
    text: &str,
    voice: &str,
    rate: i32,
    pitch: i32,
    volume: i32,
    language: Language,
    cancel: Arc<AtomicBool>,
) -> Result<Vec<u8>, String> {
    let path = temp_wav_path("sapi5");
    let chunks = vec![text.to_string()];
    crate::sapi5_engine::speak_sapi_to_file(
        crate::sapi5_engine::SapiExportOptions {
            chunks: &chunks,
            voice_name: voice,
            output_path: &path,
            language,
            rate,
            pitch,
            volume,
            audiobook_bitrate_kbps: 128,
            cancel,
        },
        |_chunk_idx| {},
    )?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::remove_file(&path) {
        crate::log_debug(&format!("Failed to remove temp SAPI5 wav: {}", e));
    }
    Ok(bytes)
}

fn synthesize_sapi4_bytes(
    text: &str,
    voice: &str,
    rate: i32,
    pitch: i32,
    volume: i32,
    cancel: Arc<AtomicBool>,
) -> Result<Vec<u8>, String> {
    let path = temp_wav_path("sapi4");
    let voice_idx = parse_sapi4_voice_index(voice);
    let chunks = vec![text.to_string()];
    crate::sapi4_engine::speak_sapi4_to_file(
        &chunks,
        voice_idx,
        &path,
        crate::sapi4_engine::Sapi4Options {
            rate,
            pitch,
            volume,
            mp3_bitrate_kbps: 128,
            cancel,
        },
        |_chunk_idx| {},
    )?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::remove_file(&path) {
        crate::log_debug(&format!("Failed to remove temp SAPI4 wav: {}", e));
    }
    Ok(bytes)
}

#[derive(Clone)]
struct SynthesisConfig {
    engine: TtsEngine,
    voice: String,
    rate: i32,
    pitch: i32,
    volume: i32,
    language: Language,
    cancel: Arc<AtomicBool>,
}

async fn synthesize_segment_bytes(text: &str, config: &SynthesisConfig) -> Result<Vec<u8>, String> {
    match config.engine {
        TtsEngine::Edge => {
            let request_id = Uuid::new_v4().simple().to_string();
            download_audio_chunk_with_cancel(DownloadAudioChunkRequest {
                text,
                voice: &config.voice,
                request_id: &request_id,
                tts_rate: config.rate,
                tts_pitch: config.pitch,
                tts_volume: config.volume,
                language: config.language,
                cancel: Some(config.cancel.as_ref()),
            })
            .await
        }
        TtsEngine::Google => {
            let text = text.to_string();
            let voice = config.voice.clone();
            let rate = config.rate;
            let pitch = config.pitch;
            let volume = config.volume;
            let cancel = config.cancel.clone();
            tokio::task::spawn_blocking(move || {
                crate::google_tts::synthesize_wav_bytes(&text, &voice, rate, pitch, volume, &cancel)
            })
            .await
            .map_err(|e| e.to_string())?
        }
        TtsEngine::Sapi5 => {
            let text = text.to_string();
            let voice = config.voice.clone();
            let rate = config.rate;
            let pitch = config.pitch;
            let volume = config.volume;
            let language = config.language;
            let cancel = config.cancel.clone();
            tokio::task::spawn_blocking(move || {
                synthesize_sapi5_bytes(&text, &voice, rate, pitch, volume, language, cancel)
            })
            .await
            .map_err(|e| e.to_string())?
        }
        TtsEngine::Sapi4 => {
            let text = text.to_string();
            let voice = config.voice.clone();
            let rate = config.rate;
            let pitch = config.pitch;
            let volume = config.volume;
            let cancel = config.cancel.clone();
            tokio::task::spawn_blocking(move || {
                synthesize_sapi4_bytes(&text, &voice, rate, pitch, volume, cancel)
            })
            .await
            .map_err(|e| e.to_string())?
        }
    }
}

fn decode_wav_to_pcm(bytes: &[u8]) -> Result<(Vec<f32>, u32, u16), String> {
    let cursor = Cursor::new(bytes.to_vec());
    let decoder = Decoder::new(cursor).map_err(|e| e.to_string())?;
    let sample_rate = decoder.sample_rate();
    let channels = decoder.channels();
    let samples: Vec<f32> = decoder.map(|s| s.to_sample::<f32>()).collect();
    Ok((samples, sample_rate, channels))
}

fn write_wav_from_pcm(
    path: &Path,
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<(), String> {
    let mut writer =
        WavWriter::create(path, sample_rate, channels, 16).map_err(|e| e.to_string())?;
    writer
        .write_samples_f32(samples)
        .map_err(|e| e.to_string())?;
    writer.finalize().map_err(|e| e.to_string())?;
    Ok(())
}

fn silence_sample_count(milliseconds: u32, sample_rate: u32, channels: u16) -> usize {
    let frames = (u64::from(milliseconds) * u64::from(sample_rate)).div_ceil(1000);
    frames
        .saturating_mul(u64::from(channels.max(1)))
        .min(usize::MAX as u64) as usize
}

fn silence_samples(milliseconds: u32, sample_rate: u32, channels: u16) -> Vec<f32> {
    vec![0.0; silence_sample_count(milliseconds, sample_rate, channels)]
}

fn write_silence_wav(
    path: &Path,
    milliseconds: u32,
    sample_rate: u32,
    channels: u16,
) -> Result<(), String> {
    let samples = silence_samples(milliseconds, sample_rate, channels);
    write_wav_from_pcm(path, &samples, sample_rate, channels)
}

#[derive(Clone, Copy)]
struct TargetAudio {
    sample_rate: u32,
    channels: u16,
}

async fn synthesize_segment_to_wav(
    text: &str,
    config: &SynthesisConfig,
    target: TargetAudio,
) -> Result<PathBuf, String> {
    let wav_path = temp_wav_path("mix");
    let bytes = if config.engine == TtsEngine::Sapi5 {
        let text = text.to_string();
        let config = config.clone();
        tokio::task::spawn_blocking(move || synthesize_sapi5_isolated_bytes(&text, &config))
            .await
            .map_err(|error| format!("SAPI5 isolated synthesis failed: {error}"))??
    } else {
        synthesize_segment_bytes(text, config).await?
    };
    let (samples, src_rate, src_channels) = if config.engine == TtsEngine::Edge {
        match decode_mp3_to_pcm(&bytes) {
            Ok(v) => v,
            Err(mp3_err) => match decode_wav_to_pcm(&bytes) {
                Ok(v) => v,
                Err(wav_err) => {
                    return Err(format!(
                        "Segment decode failed (mp3='{}', wav='{}')",
                        mp3_err, wav_err
                    ));
                }
            },
        }
    } else {
        decode_wav_to_pcm(&bytes)?
    };
    if samples.is_empty() {
        return Err("Segment decode failed: decoded audio contains no samples".to_string());
    }
    let resampled = resample_pcm(
        &samples,
        src_rate,
        src_channels,
        target.sample_rate,
        target.channels,
    );
    write_wav_from_pcm(&wav_path, &resampled, target.sample_rate, target.channels)?;
    Ok(wav_path)
}

pub fn start_tts_playback_with_chunks(options: TtsPlaybackOptions) {
    // Record telemetry
    crate::telemetry::record_action("tts_start", format!("chunks={}", options.chunks.len()));
    crate::telemetry::set_tts_active(true);

    stop_tts_playback(options.hwnd);
    prevent_sleep(true);
    if options.chunks.is_empty() {
        return;
    }

    let language =
        { with_state(options.hwnd, |state| state.settings.language) }.unwrap_or_default();
    let (tx, rx) = mpsc::unbounded_channel::<TtsCommand>();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_flag = cancel.clone();
    let session_id = {
        with_state(options.hwnd, |state| {
            let id = state.tts_next_session_id;
            state.tts_next_session_id = state.tts_next_session_id.saturating_add(1);
            state.tts_session = Some(TtsSession {
                id,
                command_tx: tx.clone(),
                cancel: cancel.clone(),
                paused: false,
                initial_caret_pos: options.initial_caret_pos,
                source_edit: options.source_edit,
            });
            id
        })
        .unwrap_or(0)
    };
    let hwnd_copy = options.hwnd;
    let chunks = options.chunks;
    let cleaned = options.cleaned;
    let voice = options.voice;
    let tts_rate = options.rate;
    let tts_pitch = options.pitch;
    let tts_volume = options.volume;
    let tts_engine = options.engine;

    std::thread::spawn(move || {
        // Wrap entire TTS playback in catch_unwind to prevent panics from crashing the app
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tts_playback_inner(TtsPlaybackRequest {
                hwnd_copy,
                session_id,
                chunks,
                cleaned,
                voice,
                tts_rate,
                tts_pitch,
                tts_volume,
                tts_engine,
                language,
                cancel_flag,
                rx,
            });
        }));

        if let Err(panic_info) = result {
            let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic in TTS playback".to_string()
            };
            crate::log_debug(&format!("TTS thread panic caught: {}", panic_msg));
            post_tts_error(
                hwnd_copy,
                session_id,
                format!("Audio playback error: {}", panic_msg),
            );
        }
    });
}

struct TtsPlaybackRequest {
    hwnd_copy: HWND,
    session_id: u64,
    chunks: Vec<crate::TtsChunk>,
    cleaned: String,
    voice: String,
    tts_rate: i32,
    tts_pitch: i32,
    tts_volume: i32,
    tts_engine: TtsEngine,
    language: Language,
    cancel_flag: Arc<AtomicBool>,
    rx: mpsc::UnboundedReceiver<TtsCommand>,
}

fn tts_progress_char_weight(ch: char) -> u64 {
    match ch {
        ',' => 35,
        ';' | ':' => 45,
        '.' | '!' | '?' => 70,
        '—' | '–' => 35,
        '\n' | '\r' => 75,
        _ => 10,
    }
}

fn build_tts_progress_weight_prefix(text: &str) -> Vec<u64> {
    let mut prefix = Vec::with_capacity(text.len().saturating_add(1));
    let mut total = 0u64;
    prefix.push(total);
    for ch in text.chars() {
        total = total.saturating_add(tts_progress_char_weight(ch));
        prefix.push(total);
    }
    prefix
}

/// Return the number of interleaved PCM samples stored in a RIFF/WAVE file.
/// Google, SAPI4 and SAPI5 synthesize complete WAV buffers before playback.
/// Reading the data and format chunks gives cursor progress a reliable duration
/// even when rodio/symphonia does not expose `total_duration()` for that WAV.
fn wav_interleaved_sample_count(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }

    let mut offset = 12usize;
    let mut channels = None;
    let mut block_align = None;
    let mut data_size = None;

    while offset.checked_add(8)? <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size =
            u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
        let data_start = offset.checked_add(8)?;
        let available = bytes.len().saturating_sub(data_start);
        let actual_size = chunk_size.min(available);

        if chunk_id == b"fmt " && actual_size >= 16 {
            channels = Some(u16::from_le_bytes(
                bytes[data_start + 2..data_start + 4].try_into().ok()?,
            ));
            block_align = Some(u16::from_le_bytes(
                bytes[data_start + 12..data_start + 14].try_into().ok()?,
            ));
        } else if chunk_id == b"data" {
            data_size = Some(actual_size as u64);
        }

        if channels.is_some() && block_align.is_some() && data_size.is_some() {
            break;
        }

        let padded_size = chunk_size.saturating_add(chunk_size & 1);
        offset = data_start.checked_add(padded_size)?;
    }

    let channels = u64::from(channels?.max(1));
    let block_align = u64::from(block_align?);
    if block_align == 0 {
        return None;
    }
    let frames = data_size? / block_align;
    Some(frames.saturating_mul(channels))
}

struct TtsPlaybackProgressSource<S> {
    inner: S,
    hwnd: HWND,
    session_id: u64,
    chunk_start_offset: usize,
    chunk_len: usize,
    total_samples: Option<u64>,
    emitted_samples: u64,
    next_report_sample: u64,
    report_interval_samples: u64,
    last_reported_offset: Option<usize>,
    reported_end: bool,
    progress_weight_prefix: Vec<u64>,
    progress_weight_total: u64,
}

impl<S> TtsPlaybackProgressSource<S>
where
    S: Source<Item = f32>,
{
    fn new(
        inner: S,
        hwnd: HWND,
        session_id: u64,
        chunk_start_offset: usize,
        chunk_len: usize,
        progress_text: String,
        known_total_samples: Option<u64>,
    ) -> Self {
        let channels = u64::from(inner.channels()).max(1);
        let sample_rate = u64::from(inner.sample_rate()).max(1);
        let total_samples = known_total_samples.or_else(|| {
            inner.total_duration().map(|duration| {
                (duration.as_secs_f64() * sample_rate as f64 * channels as f64).round() as u64
            })
        });
        let report_interval_samples = (sample_rate * channels / 5).max(1);
        let progress_weight_prefix = build_tts_progress_weight_prefix(&progress_text);
        let progress_weight_total = progress_weight_prefix.last().copied().unwrap_or(0);

        Self {
            inner,
            hwnd,
            session_id,
            chunk_start_offset,
            chunk_len,
            total_samples: total_samples.filter(|samples| *samples > 0),
            emitted_samples: 0,
            next_report_sample: report_interval_samples,
            report_interval_samples,
            last_reported_offset: None,
            reported_end: false,
            progress_weight_prefix,
            progress_weight_total,
        }
    }

    fn report_absolute_offset(&mut self, absolute_offset: usize) {
        let clamped = self
            .chunk_start_offset
            .saturating_add(self.chunk_len)
            .min(absolute_offset);
        if self.last_reported_offset != Some(clamped) {
            post_tts_chunk_offset(self.hwnd, self.session_id, clamped);
            self.last_reported_offset = Some(clamped);
        }
    }

    fn report_progress(&mut self) {
        if self.last_reported_offset.is_none() {
            self.report_absolute_offset(self.chunk_start_offset);
        }
        let Some(total_samples) = self.total_samples else {
            return;
        };
        if self.chunk_len == 0 || self.emitted_samples < self.next_report_sample {
            return;
        }
        let current = self.emitted_samples.min(total_samples);
        let offset_in_chunk = self.weighted_offset_in_chunk(current, total_samples);
        self.report_absolute_offset(self.chunk_start_offset.saturating_add(offset_in_chunk));
        while self.next_report_sample <= self.emitted_samples {
            self.next_report_sample = self
                .next_report_sample
                .saturating_add(self.report_interval_samples);
        }
    }

    fn weighted_offset_in_chunk(&self, current_samples: u64, total_samples: u64) -> usize {
        if self.chunk_len == 0 {
            return 0;
        }
        if total_samples == 0 || self.progress_weight_total == 0 {
            return self.chunk_len;
        }
        let weighted_target = ((current_samples as u128 * self.progress_weight_total as u128)
            / total_samples as u128)
            .min(self.progress_weight_total as u128) as u64;
        let consumed_chars = self
            .progress_weight_prefix
            .partition_point(|weight| *weight <= weighted_target)
            .saturating_sub(1);
        let total_chars = self.progress_weight_prefix.len().saturating_sub(1);
        if total_chars == 0 {
            return ((current_samples as u128 * self.chunk_len as u128) / total_samples as u128)
                .min(self.chunk_len as u128) as usize;
        }
        ((consumed_chars as u128 * self.chunk_len as u128) / total_chars as u128)
            .min(self.chunk_len as u128) as usize
    }

    fn report_end(&mut self) {
        if !self.reported_end {
            self.report_absolute_offset(self.chunk_start_offset.saturating_add(self.chunk_len));
            self.reported_end = true;
        }
    }
}

impl<S> Iterator for TtsPlaybackProgressSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.inner.next();
        if sample.is_some() {
            self.emitted_samples = self.emitted_samples.saturating_add(1);
            self.report_progress();
        } else {
            self.report_end();
        }
        sample
    }
}

impl<S> Source for TtsPlaybackProgressSource<S>
where
    S: Source<Item = f32>,
{
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

fn tts_playback_inner(req: TtsPlaybackRequest) {
    let TtsPlaybackRequest {
        hwnd_copy,
        session_id,
        chunks,
        cleaned,
        voice,
        tts_rate,
        tts_pitch,
        tts_volume,
        tts_engine,
        language,
        cancel_flag,
        rx,
    } = req;
    let mut rx = rx;
    let start = Instant::now();
    let mut total_audio_bytes = 0usize;
    let mut first_audio_at: Option<Instant> = None;
    let log_end = |reason: &str, total_bytes: usize, first_audio_at: Option<Instant>| {
        let elapsed_ms = start.elapsed().as_millis();
        if let Some(first) = first_audio_at {
            let first_ms = first.duration_since(start).as_millis();
            log_debug(&format!(
                "TTS end: reason={} elapsed_ms={} audio_bytes={} first_audio_ms={}",
                reason, elapsed_ms, total_bytes, first_ms
            ));
        } else {
            log_debug(&format!(
                "TTS end: reason={} elapsed_ms={} audio_bytes={} first_audio_ms=na",
                reason, elapsed_ms, total_bytes
            ));
        }
    };
    let engine_label = match tts_engine {
        TtsEngine::Edge => "edge",
        TtsEngine::Google => "google",
        TtsEngine::Sapi4 => "sapi4",
        TtsEngine::Sapi5 => "sapi5",
    };
    log_debug(&format!(
        "TTS start: engine={} voice={voice} chunks={} text_len={} rate={} pitch={} volume={}",
        engine_label,
        chunks.len(),
        cleaned.len(),
        tts_rate,
        tts_pitch,
        tts_volume
    ));
    let stream_handle = match OutputStreamBuilder::open_default_stream() {
        Ok(handle) => handle,
        Err(_) => {
            post_tts_error(
                hwnd_copy,
                session_id,
                "Audio output device not available.".to_string(),
            );
            log_end(
                "output_device_unavailable",
                total_audio_bytes,
                first_audio_at,
            );
            return;
        }
    };
    let sink = Sink::connect_new(stream_handle.mixer());
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            post_tts_error(hwnd_copy, session_id, err.to_string());
            log_end("runtime_init_failed", total_audio_bytes, first_audio_at);
            return;
        }
    };

    let (audio_tx, mut audio_rx) = mpsc::channel::<Result<TtsAudioPacket, String>>(10);
    let cancel_downloader = cancel_flag.clone();
    let chunks_downloader = chunks.clone();
    let voice_downloader = voice.clone();
    let rate = tts_rate;
    let pitch = tts_pitch;
    let volume = tts_volume;

    rt.spawn(async move {
        let all_edge = tts_engine == TtsEngine::Edge
            && chunks_downloader.iter().all(|chunk| {
                chunk
                    .override_voice
                    .as_ref()
                    .map(|v| v.engine == TtsEngine::Edge)
                    .unwrap_or(true)
            });

        if all_edge {
            const WS_RETRY_MAX: usize = 10;
            const EDGE_FRESH_CHUNK_RETRIES: usize = 6;
            let mut next_index = 0usize;
            let mut attempt = 1usize;
            loop {
                if cancel_downloader.load(Ordering::SeqCst) {
                    return;
                }
                let ws_result = download_edge_chunks_ws(
                    &chunks_downloader[next_index..],
                    &voice_downloader,
                    rate,
                    pitch,
                    volume,
                    cancel_downloader.as_ref(),
                    Some(&audio_tx),
                )
                .await;
                match ws_result {
                    Ok(processed_count) => {
                        if processed_count == 0 {
                            let msg = "Edge WS: no progress".to_string();
                            if let Err(err) = audio_tx.send(Err(msg)).await {
                                crate::log_debug(&format!("Failed to send audio error: {:?}", err));
                            }
                            return;
                        }
                        next_index = (next_index + processed_count).min(chunks_downloader.len());
                        if next_index >= chunks_downloader.len() {
                            return;
                        }
                        crate::log_debug(&format!(
                            "Edge WS: partial progress processed={} remaining={} (attempt {}/{})",
                            processed_count,
                            chunks_downloader.len().saturating_sub(next_index),
                            attempt,
                            WS_RETRY_MAX
                        ));
                    }
                    Err(e) => {
                        if cancel_downloader.load(Ordering::SeqCst) {
                            return;
                        }
                        if is_edge_forbidden_error(&e) {
                            crate::log_debug(
                                "Edge WS reuse returned 403; switching to fresh Edge sessions for remaining chunks",
                            );
                            for (offset, chunk_obj) in chunks_downloader[next_index..].iter().enumerate() {
                                if cancel_downloader.load(Ordering::SeqCst) {
                                    return;
                                }
                                if let Some(ms) = chunk_obj.pause_ms {
                                    if audio_tx
                                        .send(Ok((
                                            Vec::new(),
                                            chunk_obj.original_len,
                                            String::new(),
                                            Some(ms),
                                        )))
                                        .await
                                        .is_err()
                                    {
                                        return;
                                    }
                                    continue;
                                }
                                let (chunk_voice, chunk_rate, chunk_pitch, chunk_volume) =
                                    if let Some(ov) = &chunk_obj.override_voice {
                                        (
                                            ov.voice.as_str(),
                                            ov.rate.unwrap_or(0),
                                            ov.pitch.unwrap_or(0),
                                            ov.volume.unwrap_or(100),
                                        )
                                    } else {
                                        (voice_downloader.as_str(), rate, pitch, volume)
                                    };
                                let options = EdgeStreamOptions {
                                    voice: chunk_voice,
                                    rate: chunk_rate,
                                    pitch: chunk_pitch,
                                    volume: chunk_volume,
                                    language,
                                    cancel: cancel_downloader.as_ref(),
                                    progress_hwnd: hwnd_copy,
                                    allow_http_fallback: true,
                                };
                                match download_edge_chunk_ws_with_retry(
                                    chunk_obj,
                                    &options,
                                    next_index + offset,
                                    EDGE_FRESH_CHUNK_RETRIES,
                                )
                                .await
                                {
                                    Ok(audio) => {
                                        let len = chunk_obj.original_len;
                                        if audio_tx
                                            .send(Ok((
                                                audio,
                                                len,
                                                chunk_obj.text_to_read.clone(),
                                                None,
                                            )))
                                            .await
                                            .is_err()
                                        {
                                            return;
                                        }
                                    }
                                    Err(fresh_err) => {
                                        if let Err(err) = audio_tx.send(Err(fresh_err)).await {
                                            crate::log_debug(&format!(
                                                "Failed to send audio error: {:?}",
                                                err
                                            ));
                                        }
                                        return;
                                    }
                                }
                            }
                            return;
                        }
                        let retry_forever = is_edge_retry_forever_error(&e);
                        crate::log_debug(&format!(
                            "Edge WS reuse failed (attempt {}/{}): {}",
                            attempt,
                            if retry_forever {
                                "inf".to_string()
                            } else {
                                WS_RETRY_MAX.to_string()
                            },
                            e
                        ));
                        if !retry_forever && attempt >= WS_RETRY_MAX {
                            if let Err(err) = audio_tx.send(Err(e)).await {
                                crate::log_debug(&format!("Failed to send audio error: {:?}", err));
                            }
                            return;
                        }
                        tokio::time::sleep(Duration::from_millis(edge_retry_delay_ms(&e, attempt)))
                            .await;
                        attempt = attempt.saturating_add(1);
                    }
                }
            }
        }

        // Mixed-engine path: synthesise chunks sequentially to avoid
        // COM conflicts between SAPI4/SAPI5 running on parallel threads.
        for chunk_obj in &chunks_downloader {
            if cancel_downloader.load(Ordering::SeqCst) {
                break;
            }
            if let Some(ms) = chunk_obj.pause_ms {
                if audio_tx
                    .send(Ok((
                        Vec::new(),
                        chunk_obj.original_len,
                        String::new(),
                        Some(ms),
                    )))
                    .await
                    .is_err()
                {
                    break;
                }
                continue;
            }
            let (engine, chunk_voice, chunk_rate, chunk_pitch, chunk_volume) =
                if let Some(ov) = &chunk_obj.override_voice {
                    (
                        ov.engine,
                        ov.voice.as_str(),
                        ov.rate.unwrap_or(0),
                        ov.pitch.unwrap_or(0),
                        ov.volume.unwrap_or(100),
                    )
                } else {
                    (tts_engine, voice_downloader.as_str(), rate, pitch, volume)
                };
            let config = SynthesisConfig {
                engine,
                voice: chunk_voice.to_string(),
                rate: chunk_rate,
                pitch: chunk_pitch,
                volume: chunk_volume,
                language,
                cancel: cancel_downloader.clone(),
            };
            let result = synthesize_segment_bytes(&chunk_obj.text_to_read, &config).await;
            match result {
                Ok(data) => {
                    if audio_tx
                        .send(Ok((
                            data,
                            chunk_obj.original_len,
                            chunk_obj.text_to_read.clone(),
                            None,
                        )))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    if let Err(err) = audio_tx.send(Err(e)).await {
                        crate::log_debug(&format!("Failed to send audio error: {:?}", err));
                    }
                    break;
                }
            }
        }
    });

    let mut appended_any = false;
    let mut paused = false;
    let mut current_offset: usize = 0;
    let mut end_reason = "completed";

    loop {
        if cancel_flag.load(Ordering::SeqCst) {
            end_reason = "cancelled";
            break;
        }

        let packet = rt.block_on(async {
            loop {
                if cancel_flag.load(Ordering::SeqCst) {
                    return None;
                }
                while let Ok(cmd) = rx.try_recv() {
                    if handle_tts_command(cmd, &sink, cancel_flag.as_ref(), &mut paused) {
                        return None;
                    }
                }
                if paused {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                tokio::select! {
                    res = audio_rx.recv() => return res,
                    cmd_opt = rx.recv() => {
                        if let Some(cmd) = cmd_opt
                            && handle_tts_command(cmd, &sink, cancel_flag.as_ref(), &mut paused)
                        {
                            return None;
                        }
                    }
                }
            }
        });

        let Some(res) = packet else {
            if !appended_any {
                end_reason = "no_audio";
                crate::log_debug(&format!(
                    "TTS no audio received (engine={}, cancelled={})",
                    engine_label,
                    cancel_flag.load(Ordering::SeqCst)
                ));
            }
            break;
        };
        let (audio, orig_len, progress_text, pause_ms) = match res {
            Ok(data) => data,
            Err(e) => {
                post_tts_error(hwnd_copy, session_id, e);
                end_reason = "download_error";
                break;
            }
        };

        if let Some(ms) = pause_ms {
            let samples = silence_samples(ms, 44_100, 1);
            let source = SamplesBuffer::new(1, 44_100, samples);
            let progress_source = TtsPlaybackProgressSource::new(
                source,
                hwnd_copy,
                session_id,
                current_offset,
                orig_len,
                progress_text,
                None,
            );
            sink.append(progress_source);
            appended_any = true;
            while !sink.empty() {
                if cancel_flag.load(Ordering::SeqCst) {
                    sink.stop();
                    end_reason = "cancelled";
                    break;
                }
                while let Ok(cmd) = rx.try_recv() {
                    if handle_tts_command(cmd, &sink, cancel_flag.as_ref(), &mut paused) {
                        end_reason = "stopped";
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            current_offset = current_offset.saturating_add(orig_len.max(1));
            continue;
        }

        if audio.is_empty() {
            current_offset = current_offset.saturating_add(orig_len.max(1));
            continue;
        }

        total_audio_bytes = total_audio_bytes.saturating_add(audio.len());
        if first_audio_at.is_none() {
            first_audio_at = Some(Instant::now());
            let first_ms = first_audio_at
                .as_ref()
                .map(|t| t.duration_since(start).as_millis())
                .unwrap_or(0);
            log_debug(&format!("TTS first audio: elapsed_ms={}", first_ms));
        }

        if tts_engine == TtsEngine::Edge {
            // Decode the complete Edge MP3 segment before playback.  Using the
            // exact PCM sample count avoids the small MP3 duration rounding
            // errors that made cursor progress less precise than SAPI/Google.
            let (samples, sample_rate, channels) = match decode_mp3_to_pcm(&audio) {
                Ok(decoded) => decoded,
                Err(_) => {
                    post_tts_error(hwnd_copy, session_id, "Failed to decode audio.".to_string());
                    end_reason = "decode_error";
                    break;
                }
            };
            let known_total_samples = u64::try_from(samples.len()).ok();
            let source = SamplesBuffer::new(channels, sample_rate, samples);
            let progress_source = TtsPlaybackProgressSource::new(
                source,
                hwnd_copy,
                session_id,
                current_offset,
                orig_len,
                progress_text,
                known_total_samples,
            );
            sink.append(progress_source);
        } else {
            let known_total_samples = wav_interleaved_sample_count(&audio);
            let cursor = std::io::Cursor::new(audio);
            let source = match Decoder::new(cursor) {
                Ok(source) => source,
                Err(_) => {
                    post_tts_error(hwnd_copy, session_id, "Failed to decode audio.".to_string());
                    end_reason = "decode_error";
                    break;
                }
            };
            let progress_source = TtsPlaybackProgressSource::new(
                source,
                hwnd_copy,
                session_id,
                current_offset,
                orig_len,
                progress_text,
                known_total_samples,
            );
            sink.append(progress_source);
        }
        appended_any = true;
        while !sink.empty() {
            if cancel_flag.load(Ordering::SeqCst) {
                sink.stop();
                end_reason = "cancelled";
                break;
            }
            while let Ok(cmd) = rx.try_recv() {
                if handle_tts_command(cmd, &sink, cancel_flag.as_ref(), &mut paused) {
                    end_reason = "stopped";
                    break;
                }
            }
            if cancel_flag.load(Ordering::SeqCst) {
                sink.stop();
                end_reason = "cancelled";
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if cancel_flag.load(Ordering::SeqCst) {
            end_reason = "cancelled";
            break;
        }
        current_offset = current_offset.saturating_add(orig_len.max(1));
    }

    if appended_any {
        unsafe {
            if let Err(e) = PostMessageW(
                hwnd_copy,
                WM_TTS_PLAYBACK_DONE,
                WPARAM(session_id as usize),
                LPARAM(0),
            ) {
                crate::log_debug(&format!("Failed to post WM_TTS_PLAYBACK_DONE: {}", e));
            }
        }
    }
    log_end(end_reason, total_audio_bytes, first_audio_at);
}

pub fn play_edge_bytes_async(bytes: Vec<u8>, volume: i32) -> Arc<AtomicBool> {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_flag = cancel.clone();
    std::thread::spawn(move || {
        let stream_handle = match OutputStreamBuilder::open_default_stream() {
            Ok(handle) => handle,
            Err(_) => {
                crate::log_debug("Edge TTS: audio output device not available.");
                return;
            }
        };
        let sink = Sink::connect_new(stream_handle.mixer());
        let vol = (volume as f32 / 100.0).clamp(0.0, 1.0);
        sink.set_volume(vol);
        let cursor = std::io::Cursor::new(bytes);
        let source = match Decoder::new(cursor) {
            Ok(source) => source,
            Err(err) => {
                crate::log_debug(&format!("Edge TTS: decoder failed: {}", err));
                return;
            }
        };
        sink.append(source);
        loop {
            if cancel_flag.load(Ordering::SeqCst) {
                sink.stop();
                break;
            }
            if sink.empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    });
    cancel
}

fn is_edge_connection_retry_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    err.contains("os error 10054")
        || lower.contains("connection reset")
        || lower.contains("forcibly closed by the remote host")
}

fn is_edge_forbidden_error(err: &str) -> bool {
    err.to_ascii_lowercase().contains("403 forbidden")
}

fn is_edge_retry_forever_error(err: &str) -> bool {
    is_edge_connection_retry_error(err) || is_edge_forbidden_error(err)
}

fn is_edge_audiobook_transient_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    is_edge_retry_forever_error(err)
        || lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("temporar")
        || lower.contains("connection reset")
        || lower.contains("connection refused")
        || lower.contains("connection aborted")
        || lower.contains("connection closed")
        || lower.contains("failed to connect")
        || lower.contains("forcibly closed")
        || lower.contains("websocket")
        || lower.contains("web socket")
        || lower.contains("network")
        || lower.contains("broken pipe")
        || lower.contains("unexpected eof")
        || lower.contains("closed without")
        || lower.contains("too many requests")
        || lower.contains("429")
        || lower.contains("500 internal server error")
        || lower.contains("502 bad gateway")
        || lower.contains("503 service unavailable")
        || lower.contains("504 gateway timeout")
        || lower.contains("no audio sent")
        || lower.contains("empty audio")
        || lower.contains("empty payload")
        || lower.contains("invalid audio")
        || lower.contains("decode failed")
        || lower.contains("segment decode failed")
}

fn edge_retry_delay_ms(err: &str, attempt: usize) -> u64 {
    let lower = err.to_ascii_lowercase();
    let base_ms = if is_edge_retry_forever_error(err) || lower.contains("timeout") {
        250
    } else {
        400
    };
    (attempt as u64).saturating_mul(base_ms as u64).min(2000)
}

pub async fn download_audio_chunk(
    text: &str,
    voice: &str,
    request_id: &str,
    tts_rate: i32,
    tts_pitch: i32,
    tts_volume: i32,
    language: Language,
) -> Result<Vec<u8>, String> {
    download_audio_chunk_with_cancel(DownloadAudioChunkRequest {
        text,
        voice,
        request_id,
        tts_rate,
        tts_pitch,
        tts_volume,
        language,
        cancel: None,
    })
    .await
}

struct DownloadAudioChunkRequest<'a> {
    text: &'a str,
    voice: &'a str,
    request_id: &'a str,
    tts_rate: i32,
    tts_pitch: i32,
    tts_volume: i32,
    language: Language,
    cancel: Option<&'a AtomicBool>,
}

async fn async_sleep_with_cancellation(cancel: &AtomicBool, duration: Duration) -> bool {
    let started = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let elapsed = started.elapsed();
        if elapsed >= duration {
            return true;
        }
        let remaining = duration.saturating_sub(elapsed);
        tokio::time::sleep(remaining.min(Duration::from_millis(100))).await;
    }
}

async fn download_audio_chunk_with_cancel(
    request: DownloadAudioChunkRequest<'_>,
) -> Result<Vec<u8>, String> {
    let max_retries = 40;
    let mut attempt = 1usize;

    let last_error = loop {
        if request
            .cancel
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return Err(cancelled_message(request.language));
        }
        match download_audio_chunk_attempt(
            request.text,
            request.voice,
            request.request_id,
            request.tts_rate,
            request.tts_pitch,
            request.tts_volume,
            request.cancel,
        )
        .await
        {
            Ok(data) => return Ok(data),
            Err(e) => {
                let retry_forever = is_edge_retry_forever_error(&e);
                let max_label = if retry_forever {
                    "inf".to_string()
                } else {
                    max_retries.to_string()
                };
                let msg = i18n::tr_f(
                    request.language,
                    "tts.chunk_download_retry",
                    &[
                        ("attempt", &attempt.to_string()),
                        ("max", &max_label),
                        ("err", &e),
                    ],
                );
                log_debug(&msg);
                if !retry_forever && attempt >= max_retries {
                    break e;
                }
                let delay = Duration::from_millis(edge_retry_delay_ms(&e, attempt));
                if let Some(cancel) = request.cancel {
                    if !async_sleep_with_cancellation(cancel, delay).await {
                        return Err(cancelled_message(request.language));
                    }
                } else {
                    tokio::time::sleep(delay).await;
                }
                attempt = attempt.saturating_add(1);
            }
        }
    };
    Err(i18n::tr_f(
        request.language,
        "tts.chunk_download_error",
        &[("err", &last_error)],
    ))
}

async fn download_audio_chunk_attempt(
    text: &str,
    voice: &str,
    request_id: &str,
    tts_rate: i32,
    tts_pitch: i32,
    tts_volume: i32,
    cancel: Option<&AtomicBool>,
) -> Result<Vec<u8>, String> {
    let sec_ms_gec = generate_sec_ms_gec();
    let sec_ms_gec_version = "1-132.0.2917.39";

    let url_str = format!(
        "{}?TrustedClientToken={}&ConnectionId={}&Sec-MS-GEC={}&Sec-MS-GEC-Version={}",
        WSS_URL_BASE, TRUSTED_CLIENT_TOKEN, request_id, sec_ms_gec, sec_ms_gec_version
    );
    let url = Url::parse(&url_str).map_err(|err| err.to_string())?;

    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|e: tungstenite::Error| e.to_string())?;
    let headers = request.headers_mut();
    headers.insert("Pragma", HeaderValue::from_static("no-cache"));
    headers.insert("Cache-Control", HeaderValue::from_static("no-cache"));
    headers.insert(
        "Origin",
        HeaderValue::from_static("chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold"),
    );
    headers.insert("User-Agent", HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36 Edg/132.0.0.0"));
    headers.insert(
        "Accept-Encoding",
        HeaderValue::from_static("gzip, deflate, br"),
    );
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("en-US,en;q=0.9"),
    );
    let cookie = format!("muid={};", generate_muid());
    headers.insert(
        "Cookie",
        HeaderValue::from_str(&cookie).map_err(|err| err.to_string())?,
    );

    let connect_timeout = Duration::from_secs(5);
    let (ws_stream, _) = match tokio::time::timeout(connect_timeout, connect_async(request)).await {
        Ok(res) => res.map_err(|e: tungstenite::Error| e.to_string())?,
        Err(_) => {
            return Err("WebSocket connect timeout".to_string());
        }
    };
    if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
        return Err("cancelled".to_string());
    }
    let (mut write, mut read): (
        futures_util::stream::SplitSink<_, Message>,
        futures_util::stream::SplitStream<_>,
    ) = ws_stream.split();

    let config_msg = format!(
        "X-Timestamp:{}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n{{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"false\"}},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}",
        get_date_string()
    );
    write
        .send(Message::Text(config_msg.into()))
        .await
        .map_err(|e: tungstenite::Error| e.to_string())?;
    if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
        return Err("cancelled".to_string());
    }

    let sanitized_text = sanitize_edge_text(text);
    if !is_edge_text_usable(&sanitized_text) {
        crate::log_debug("Edge WS: skipping empty chunk after sanitization (HTTP fallback path).");
        return Ok(Vec::new());
    }
    let ssml = mkssml(&sanitized_text, voice, tts_rate, tts_pitch, tts_volume);
    let ssml_msg = format!(
        "X-RequestId:{}\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:{}Z\r\nPath:ssml\r\n\r\n{}",
        request_id,
        get_date_string(),
        ssml
    );
    write
        .send(Message::Text(ssml_msg.into()))
        .await
        .map_err(|e: tungstenite::Error| e.to_string())?;

    let mut audio_data = Vec::new();
    loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return Err("cancelled".to_string());
        }
        let next = match tokio::time::timeout(Duration::from_millis(100), read.next()).await {
            Ok(next) => next,
            Err(_) => continue,
        };
        let Some(msg) = next else {
            break;
        };
        let msg: Message = msg.map_err(|e: tungstenite::Error| e.to_string())?;
        match msg {
            Message::Text(text) if text.contains("Path:turn.end") => {
                break;
            }
            Message::Text(_) => {}
            Message::Binary(data) => match parse_edge_binary_audio_payload(&data) {
                Ok(Some(audio)) => {
                    audio_data.extend_from_slice(&audio);
                }
                Ok(None) => continue,
                Err(err) => {
                    log_debug(&err);
                    continue;
                }
            },
            Message::Close(_) => break,
            _ => {}
        }
    }
    Ok(audio_data)
}

async fn read_edge_audio_turn(
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) -> Result<Vec<u8>, String> {
    let mut audio_data = Vec::new();
    while let Some(msg) = read.next().await {
        let msg: Message = msg.map_err(|e: tungstenite::Error| e.to_string())?;
        match msg {
            Message::Text(text) if text.contains("Path:turn.end") => {
                break;
            }
            Message::Text(_) => {}
            Message::Binary(data) => match parse_edge_binary_audio_payload(&data) {
                Ok(Some(audio)) => {
                    audio_data.extend_from_slice(&audio);
                }
                Ok(None) => continue,
                Err(err) => {
                    log_debug(&err);
                    continue;
                }
            },
            Message::Close(_) => break,
            _ => {}
        }
    }
    Ok(audio_data)
}

async fn read_edge_audio_turn_to_writer(
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    buffer: &mut Vec<u8>,
) -> Result<bool, String> {
    let mut audio_received = false;
    while let Some(msg) = read.next().await {
        let msg: Message = msg.map_err(|e: tungstenite::Error| e.to_string())?;
        match msg {
            Message::Text(text) if text.contains("Path:turn.end") => {
                break;
            }
            Message::Text(_) => {}
            Message::Binary(data) => match parse_edge_binary_audio_payload(&data) {
                Ok(Some(audio)) => {
                    buffer.extend_from_slice(&audio);
                    audio_received = true;
                }
                Ok(None) => continue,
                Err(err) => {
                    log_debug(&err);
                    continue;
                }
            },
            Message::Close(_) => break,
            _ => {}
        }
    }
    Ok(audio_received)
}

async fn download_edge_chunks_ws(
    chunks: &[TtsChunk],
    default_voice: &str,
    tts_rate: i32,
    tts_pitch: i32,
    tts_volume: i32,
    cancel: &AtomicBool,
    audio_tx: EdgeAudioTx<'_>,
) -> Result<usize, String> {
    let first_audio_timeout = Duration::from_secs(60);
    const FIRST_AUDIO_RETRIES: usize = 1;
    let sec_ms_gec = generate_sec_ms_gec();
    let sec_ms_gec_version = "1-132.0.2917.39";
    let request_id = Uuid::new_v4().simple().to_string();

    let url_str = format!(
        "{}?TrustedClientToken={}&ConnectionId={}&Sec-MS-GEC={}&Sec-MS-GEC-Version={}",
        WSS_URL_BASE, TRUSTED_CLIENT_TOKEN, request_id, sec_ms_gec, sec_ms_gec_version
    );
    let url = Url::parse(&url_str).map_err(|err| err.to_string())?;

    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|e: tungstenite::Error| e.to_string())?;
    let headers = request.headers_mut();
    headers.insert("Pragma", HeaderValue::from_static("no-cache"));
    headers.insert("Cache-Control", HeaderValue::from_static("no-cache"));
    headers.insert(
        "Origin",
        HeaderValue::from_static("chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold"),
    );
    headers.insert("User-Agent", HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36 Edg/132.0.0.0"));
    headers.insert(
        "Accept-Encoding",
        HeaderValue::from_static("gzip, deflate, br"),
    );
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("en-US,en;q=0.9"),
    );
    let cookie = format!("muid={};", generate_muid());
    headers.insert(
        "Cookie",
        HeaderValue::from_str(&cookie).map_err(|err| err.to_string())?,
    );

    let connect_timeout = Duration::from_secs(10);
    let (ws_stream, _) = match tokio::time::timeout(connect_timeout, connect_async(request)).await {
        Ok(res) => res.map_err(|e: tungstenite::Error| e.to_string())?,
        Err(_) => {
            return Err("WebSocket connect timeout".to_string());
        }
    };
    crate::log_debug("Edge WS: connected.");
    let (mut write, mut read) = ws_stream.split();

    let config_msg = format!(
        "X-Timestamp:{}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n{{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"false\"}},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}",
        get_date_string()
    );
    write
        .send(Message::Text(config_msg.into()))
        .await
        .map_err(|e: tungstenite::Error| e.to_string())?;

    let mut sent_count = 0usize;
    let mut processed_count = 0usize;
    for (idx, chunk) in chunks.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }
        if let Some(ms) = chunk.pause_ms {
            if let Some(tx) = audio_tx
                && tx
                    .send(Ok((
                        Vec::new(),
                        chunk.original_len,
                        String::new(),
                        Some(ms),
                    )))
                    .await
                    .is_err()
            {
                return Ok(processed_count);
            }
            processed_count = processed_count.saturating_add(1);
            continue;
        }
        let (voice, chunk_rate, chunk_pitch, chunk_volume) = if let Some(ov) = &chunk.override_voice
        {
            (
                ov.voice.as_str(),
                ov.rate.unwrap_or(0),
                ov.pitch.unwrap_or(0),
                ov.volume.unwrap_or(100),
            )
        } else {
            (default_voice, tts_rate, tts_pitch, tts_volume)
        };
        crate::log_debug(&format!(
            "Edge WS chunk {}: voice={} rate={} pitch={} volume={} override={:?} text={:?}",
            idx,
            voice,
            chunk_rate,
            chunk_pitch,
            chunk_volume,
            chunk
                .override_voice
                .as_ref()
                .map(|ov| (ov.rate, ov.pitch, ov.volume)),
            chunk.text_to_read,
        ));
        let req_id = Uuid::new_v4().simple().to_string();
        let sanitized_text = sanitize_edge_text(&chunk.text_to_read);
        if !is_edge_text_usable(&sanitized_text) {
            crate::log_debug(&format!(
                "Edge WS: skipping unusable chunk {} text={:?}",
                idx, chunk.text_to_read
            ));
            processed_count = processed_count.saturating_add(1);
            continue;
        }
        let ssml = mkssml(
            &sanitized_text,
            voice,
            chunk_rate,
            chunk_pitch,
            chunk_volume,
        );
        let ssml_msg = format!(
            "X-RequestId:{}\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:{}Z\r\nPath:ssml\r\n\r\n{}",
            req_id,
            get_date_string(),
            ssml
        );
        if let Err(e) = write.send(Message::Text(ssml_msg.into())).await {
            if processed_count > 0 {
                crate::log_debug(&format!(
                    "Edge WS: write failed after partial progress, processed={}: {}",
                    processed_count, e
                ));
                return Ok(processed_count);
            }
            return Err(e.to_string());
        }
        let audio = if sent_count == 0 {
            let mut audio = None;
            for attempt in 1..=FIRST_AUDIO_RETRIES {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled".to_string());
                }
                match tokio::time::timeout(first_audio_timeout, read_edge_audio_turn(&mut read))
                    .await
                {
                    Ok(res) => {
                        match res {
                            Ok(data) => {
                                audio = Some(data);
                            }
                            Err(e) => {
                                if processed_count > 0 {
                                    crate::log_debug(&format!(
                                        "Edge WS: read failed after partial progress, processed={}: {}",
                                        processed_count, e
                                    ));
                                    return Ok(processed_count);
                                }
                                return Err(e);
                            }
                        }
                        break;
                    }
                    Err(_) => {
                        crate::log_debug(&format!(
                            "Edge WS: first audio timeout (attempt {}/{} chunk_index={} text_len={} voice={})",
                            attempt,
                            FIRST_AUDIO_RETRIES,
                            idx,
                            chunk.text_to_read.len(),
                            voice
                        ));
                        if attempt < FIRST_AUDIO_RETRIES {
                            tokio::time::sleep(Duration::from_secs((attempt * 2) as u64)).await;
                        }
                    }
                }
            }
            audio.ok_or_else(|| "Edge WS: first audio timeout".to_string())?
        } else {
            match read_edge_audio_turn(&mut read).await {
                Ok(data) => data,
                Err(e) => {
                    if processed_count > 0 {
                        crate::log_debug(&format!(
                            "Edge WS: read failed after partial progress, processed={}: {}",
                            processed_count, e
                        ));
                        return Ok(processed_count);
                    }
                    return Err(e);
                }
            }
        };
        if audio.is_empty() {
            crate::log_debug(&format!(
                "Edge WS: received empty audio chunk, skipping chunk_index={}",
                idx
            ));
            processed_count = processed_count.saturating_add(1);
            continue;
        }
        if let Some(tx) = audio_tx {
            let len = chunk.original_len;
            if tx
                .send(Ok((audio, len, chunk.text_to_read.clone(), None)))
                .await
                .is_err()
            {
                return Ok(processed_count);
            }
            sent_count = sent_count.saturating_add(1);
        } else {
            sent_count = sent_count.saturating_add(1);
        }
        processed_count = processed_count.saturating_add(1);
    }
    Ok(processed_count)
}

struct EdgeStreamOptions<'a> {
    voice: &'a str,
    rate: i32,
    pitch: i32,
    volume: i32,
    language: Language,
    cancel: &'a AtomicBool,
    progress_hwnd: HWND,
    allow_http_fallback: bool,
}

async fn download_edge_chunk_ws(
    chunk: &TtsChunk,
    options: &EdgeStreamOptions<'_>,
    idx: usize,
) -> Result<Vec<u8>, String> {
    let first_audio_timeout = Duration::from_secs(60);
    let sec_ms_gec = generate_sec_ms_gec();
    let sec_ms_gec_version = "1-132.0.2917.39";
    let request_id = Uuid::new_v4().simple().to_string();

    let url_str = format!(
        "{}?TrustedClientToken={}&ConnectionId={}&Sec-MS-GEC={}&Sec-MS-GEC-Version={}",
        WSS_URL_BASE, TRUSTED_CLIENT_TOKEN, request_id, sec_ms_gec, sec_ms_gec_version
    );
    let url = Url::parse(&url_str).map_err(|err| err.to_string())?;

    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|e: tungstenite::Error| e.to_string())?;
    let headers = request.headers_mut();
    headers.insert("Pragma", HeaderValue::from_static("no-cache"));
    headers.insert("Cache-Control", HeaderValue::from_static("no-cache"));
    headers.insert(
        "Origin",
        HeaderValue::from_static("chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold"),
    );
    headers.insert("User-Agent", HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36 Edg/132.0.0.0"));
    headers.insert(
        "Accept-Encoding",
        HeaderValue::from_static("gzip, deflate, br"),
    );
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("en-US,en;q=0.9"),
    );
    let cookie = format!("muid={};", generate_muid());
    headers.insert(
        "Cookie",
        HeaderValue::from_str(&cookie).map_err(|err| err.to_string())?,
    );

    let connect_timeout = Duration::from_secs(10);
    let (ws_stream, _) = match tokio::time::timeout(connect_timeout, connect_async(request)).await {
        Ok(res) => res.map_err(|e: tungstenite::Error| e.to_string())?,
        Err(_) => {
            return Err("WebSocket connect timeout".to_string());
        }
    };
    crate::log_debug(&format!("Edge WS: connected (chunk {}).", idx));
    let (mut write, mut read) = ws_stream.split();

    let config_msg = format!(
        "X-Timestamp:{}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n{{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"false\"}},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}",
        get_date_string()
    );
    write
        .send(Message::Text(config_msg.into()))
        .await
        .map_err(|e: tungstenite::Error| e.to_string())?;

    if options.cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }
    let (voice, chunk_rate, chunk_pitch, chunk_volume) = if let Some(ov) = &chunk.override_voice {
        (
            ov.voice.as_str(),
            ov.rate.unwrap_or(0),
            ov.pitch.unwrap_or(0),
            ov.volume.unwrap_or(100),
        )
    } else {
        (options.voice, options.rate, options.pitch, options.volume)
    };
    let req_id = Uuid::new_v4().simple().to_string();
    let sanitized_text = sanitize_edge_text(&chunk.text_to_read);
    if !is_edge_text_usable(&sanitized_text) {
        crate::log_debug(&format!(
            "Edge WS: skipping empty chunk after sanitization (chunk_index={}).",
            idx
        ));
        return Ok(Vec::new());
    }
    let ssml = mkssml(
        &sanitized_text,
        voice,
        chunk_rate,
        chunk_pitch,
        chunk_volume,
    );
    let ssml_msg = format!(
        "X-RequestId:{}\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:{}Z\r\nPath:ssml\r\n\r\n{}",
        req_id,
        get_date_string(),
        ssml
    );
    write
        .send(Message::Text(ssml_msg.into()))
        .await
        .map_err(|e: tungstenite::Error| e.to_string())?;

    let mut chunk_buffer: Vec<u8> = Vec::new();
    let audio_received = match tokio::time::timeout(
        first_audio_timeout,
        read_edge_audio_turn_to_writer(&mut read, &mut chunk_buffer),
    )
    .await
    {
        Ok(res) => res?,
        Err(_) => {
            crate::log_debug(&format!(
                "Edge WS: first audio timeout (chunk_index={} text_len={} voice={})",
                idx,
                chunk.text_to_read.len(),
                voice
            ));
            false
        }
    };

    if !audio_received {
        crate::log_debug(&format!(
            "Edge WS: no audio sent to writer (chunk_index={} bytes_len={} utf16_len={}).",
            idx,
            chunk.text_to_read.len(),
            utf16_len(&chunk.text_to_read)
        ));
        return Err("Edge WS: no audio sent".to_string());
    }

    let text_utf16_len = utf16_len(&chunk.text_to_read);
    let text_bytes_len = chunk.text_to_read.len();
    let audio_bytes_len = chunk_buffer.len();
    crate::log_debug(&format!(
        "Edge WS: chunk completed (chunk_index={} text_bytes={} utf16_len={} audio_bytes={})",
        idx, text_bytes_len, text_utf16_len, audio_bytes_len
    ));
    if audio_bytes_len < 4096 && text_utf16_len > 600 {
        crate::log_debug(&format!(
            "Edge WS: suspiciously small audio payload (chunk_index={} utf16_len={} audio_bytes={})",
            idx, text_utf16_len, audio_bytes_len
        ));
    }

    Ok(chunk_buffer)
}

async fn download_edge_chunk_ws_with_retry(
    chunk: &TtsChunk,
    options: &EdgeStreamOptions<'_>,
    idx: usize,
    max_retries: usize,
) -> Result<Vec<u8>, String> {
    let mut attempt = 1usize;
    let last_err = loop {
        if options.cancel.load(Ordering::Relaxed) {
            return Err(cancelled_message(options.language));
        }
        match download_edge_chunk_ws(chunk, options, idx).await {
            Ok(audio) => return Ok(audio),
            Err(err) => {
                let err_str = err.as_str();
                crate::log_debug(&format!(
                    "Edge WS: chunk download failed (attempt {}/{} chunk_index={}): {}",
                    attempt,
                    if is_edge_retry_forever_error(err_str) {
                        "inf".to_string()
                    } else {
                        max_retries.to_string()
                    },
                    idx,
                    err_str
                ));
                let retry_forever = is_edge_retry_forever_error(err_str);
                if !retry_forever && attempt >= max_retries {
                    break err;
                }
                tokio::time::sleep(Duration::from_millis(edge_retry_delay_ms(err_str, attempt)))
                    .await;
                attempt = attempt.saturating_add(1);
            }
        }
    };
    if options.allow_http_fallback && !options.cancel.load(Ordering::Relaxed) {
        crate::log_debug(&format!(
            "Edge WS: falling back to HTTP chunk (chunk_index={})",
            idx
        ));
        let request_id = Uuid::new_v4().simple().to_string();
        return download_audio_chunk_with_cancel(DownloadAudioChunkRequest {
            text: &chunk.text_to_read,
            voice: options.voice,
            request_id: &request_id,
            tts_rate: options.rate,
            tts_pitch: options.pitch,
            tts_volume: options.volume,
            language: options.language,
            cancel: Some(options.cancel),
        })
        .await;
    }
    Err(last_err)
}

async fn download_edge_audiobook_chunk_validated(
    chunk: &TtsChunk,
    options: &EdgeStreamOptions<'_>,
    idx: usize,
    max_retries: usize,
    adaptive_lithuanian: bool,
) -> Result<Vec<u8>, String> {
    let sanitized_text = sanitize_edge_text(&chunk.text_to_read);
    if !is_edge_text_usable(&sanitized_text) {
        crate::log_debug(&format!(
            "Edge audiobook: chunk {} contains no speakable text after sanitization",
            idx
        ));
        return Err(i18n::tr(options.language, "app.tts_no_text"));
    }

    let mut validation_attempt = 1usize;
    loop {
        if options.cancel.load(Ordering::Relaxed) {
            return Err(cancelled_message(options.language));
        }

        let download_result = if adaptive_lithuanian {
            match download_edge_chunk_ws_adaptive_lt(chunk, options, idx, max_retries, 0).await {
                Ok(mut audio) => {
                    if is_lt_audio_suspicious(&chunk.text_to_read, audio.len())
                        && let Ok(strict_audio) =
                            download_edge_chunk_ws_strict_small_lt(chunk, options, idx, max_retries)
                                .await
                        && strict_audio.len() > audio.len()
                    {
                        audio = strict_audio;
                    }
                    Ok(audio)
                }
                Err(err) => Err(err),
            }
        } else {
            download_edge_chunk_ws_with_retry(chunk, options, idx, max_retries).await
        };

        let err = match download_result {
            Ok(audio) if audio.is_empty() => "Edge audiobook: empty audio payload".to_string(),
            Ok(audio) => match decode_mp3_to_pcm(&audio) {
                Ok((samples, _rate, _channels)) if !samples.is_empty() => return Ok(audio),
                Ok((_samples, _rate, _channels)) => {
                    "Edge audiobook: invalid audio with empty decoded samples".to_string()
                }
                Err(err) => format!("Edge audiobook: audio decode failed: {}", err),
            },
            Err(err) => err,
        };
        let retry_until_cancelled = is_edge_audiobook_transient_error(&err);
        crate::log_debug(&format!(
            "Edge audiobook: validated chunk failed chunk_index={} validation_attempt={} retry_until_cancelled={} error={}",
            idx, validation_attempt, retry_until_cancelled, err
        ));
        if !retry_until_cancelled {
            return Err(err);
        }

        tokio::time::sleep(Duration::from_millis(edge_retry_delay_ms(
            &err,
            validation_attempt,
        )))
        .await;
        validation_attempt = validation_attempt.saturating_add(1);
    }
}

fn normalize_spoken_label(value: &str) -> String {
    let value = value
        .trim()
        .trim_end_matches(['.', ',', ';', ':', '!', '?']);
    value
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn audiobook_part_announcement_text(
    options: &AudiobookCommonOptions<'_>,
    part_output: &Path,
    part_number: u32,
) -> Option<String> {
    if options.part_announcement_mode == AudiobookPartAnnouncementMode::None {
        return None;
    }
    let title = normalize_spoken_label(options.audiobook_title);
    let fallback_title = part_output
        .file_stem()
        .and_then(|value| value.to_str())
        .map(normalize_spoken_label)
        .unwrap_or_default();
    let title = if title.is_empty() {
        fallback_title.as_str()
    } else {
        title.as_str()
    };
    let file_name = part_output
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| {
            let cleaned = if let Some(marker_pos) = value.find(".chapbuild.") {
                let prefix = &value[..marker_pos];
                let suffix = value[marker_pos..]
                    .find(" Part ")
                    .map(|offset| &value[marker_pos + offset..])
                    .unwrap_or("");
                format!("{prefix}{suffix}")
            } else {
                value.to_string()
            };
            normalize_spoken_label(&cleaned)
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| title.to_string());
    let number = part_number.to_string();
    let text = match options.part_announcement_mode {
        AudiobookPartAnnouncementMode::None => return None,
        AudiobookPartAnnouncementMode::Title => title.to_string(),
        AudiobookPartAnnouncementMode::TitlePartNumber => i18n::tr_f(
            options.language,
            "audiobook.part_announcement.title_part_number",
            &[("title", title), ("number", &number)],
        ),
        AudiobookPartAnnouncementMode::FileName => file_name,
        AudiobookPartAnnouncementMode::FileNamePartNumber => i18n::tr_f(
            options.language,
            "audiobook.part_announcement.file_name_part_number",
            &[("file_name", &file_name), ("number", &number)],
        ),
    };
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(format!("{text}."))
    }
}

pub(crate) fn prepend_part_announcement(
    chunks: &[String],
    options: &AudiobookCommonOptions<'_>,
    part_output: &Path,
    part_number: u32,
) -> Vec<String> {
    let Some(announcement) = audiobook_part_announcement_text(options, part_output, part_number)
    else {
        return chunks.to_vec();
    };
    let mut result = Vec::with_capacity(chunks.len() + 1);
    result.push(announcement);
    result.extend_from_slice(chunks);
    result
}

pub(crate) fn prepend_mixed_part_announcement(
    chunks: &[TtsChunk],
    options: &AudiobookCommonOptions<'_>,
    part_output: &Path,
    part_number: u32,
) -> Vec<TtsChunk> {
    let Some(announcement) = audiobook_part_announcement_text(options, part_output, part_number)
    else {
        return chunks.to_vec();
    };
    let mut result = Vec::with_capacity(chunks.len() + 1);
    result.push(TtsChunk {
        original_len: utf16_len(&announcement),
        text_to_read: announcement,
        override_voice: None,
        pause_ms: None,
    });
    result.extend_from_slice(chunks);
    result
}

pub(crate) fn prepend_mixed_announcement_to_audio_file(
    audio_file: &Path,
    part_number: u32,
    options: &AudiobookCommonOptions<'_>,
    main_engine: TtsEngine,
) -> Result<(), String> {
    let Some(announcement) = audiobook_part_announcement_text(options, audio_file, part_number)
    else {
        return Ok(());
    };
    let stem = audio_file
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("part");
    let ext = audio_file
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("mp3");
    let announcement_file = audio_file.with_file_name(format!("{stem}.announcement.tmp.{ext}"));
    let merged_file = audio_file.with_file_name(format!("{stem}.merged.tmp.{ext}"));
    let chunk = TtsChunk {
        original_len: utf16_len(&announcement),
        text_to_read: announcement,
        override_voice: None,
        pause_ms: None,
    };
    let mut progress = 0usize;
    let config = MixedAudiobookConfig { main_engine };
    if let Err(error) = render_mixed_audiobook_part(
        &[chunk],
        &mut progress,
        &announcement_file,
        options,
        &config,
    ) {
        if let Err(cleanup_error) = std::fs::remove_file(&announcement_file) {
            crate::log_debug(&format!(
                "Failed to remove incomplete part announcement: {cleanup_error}"
            ));
        }
        return Err(error);
    }
    let merge_result = crate::ffmpeg_export::concatenate_audio_files_copy(
        &[announcement_file.clone(), audio_file.to_path_buf()],
        &merged_file,
    );
    if let Err(error) = std::fs::remove_file(&announcement_file) {
        crate::log_debug(&format!(
            "Failed to remove temporary part announcement: {error}"
        ));
    }
    if let Err(error) = merge_result {
        if let Err(cleanup_error) = std::fs::remove_file(&merged_file) {
            crate::log_debug(&format!(
                "Failed to remove incomplete merged audiobook part: {cleanup_error}"
            ));
        }
        return Err(error);
    }
    std::fs::remove_file(audio_file).map_err(|error| {
        format!("Failed to replace audiobook part after adding announcement: {error}")
    })?;
    std::fs::rename(&merged_file, audio_file)
        .map_err(|error| format!("Failed to finalize audiobook part announcement: {error}"))
}

pub(crate) fn format_audiobook_part_filename(
    stem: &str,
    ext: &str,
    number: u32,
    width: usize,
    naming_mode: AudiobookPartNamingMode,
) -> String {
    match naming_mode {
        AudiobookPartNamingMode::TitleNumber => {
            format!("{stem} Part {number:0width$}.{ext}")
        }
        AudiobookPartNamingMode::NumberOnly => format!("{number:0width$}.{ext}"),
        AudiobookPartNamingMode::NumberTitle => format!("{number:0width$} - {stem}.{ext}"),
    }
}

fn time_split_part_output(
    base: &Path,
    part_index: u32,
    start_number: u32,
    naming_mode: AudiobookPartNamingMode,
) -> PathBuf {
    const TIME_SPLIT_PART_WIDTH: usize = 4;
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("audiobook");
    let ext = base
        .extension()
        .and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("mp3");
    let number = start_number.saturating_add(part_index);
    base.with_file_name(format_audiobook_part_filename(
        stem,
        ext,
        number,
        TIME_SPLIT_PART_WIDTH,
        naming_mode,
    ))
}

fn split_part_output(
    base: &Path,
    part_index: usize,
    total_parts: usize,
    naming_mode: AudiobookPartNamingMode,
) -> PathBuf {
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("audiobook");
    let ext = base
        .extension()
        .and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("mp3");
    let width = std::cmp::max(2, total_parts.to_string().len());
    let number = (part_index + 1) as u32;
    base.with_file_name(format_audiobook_part_filename(
        stem,
        ext,
        number,
        width,
        naming_mode,
    ))
}

fn parse_part_index_from_stem(
    stem_name: &str,
    stem: &str,
    naming_mode: AudiobookPartNamingMode,
) -> Option<usize> {
    match naming_mode {
        AudiobookPartNamingMode::TitleNumber => stem_name
            .strip_prefix(&format!("{stem} Part "))?
            .parse::<usize>()
            .ok(),
        AudiobookPartNamingMode::NumberOnly => stem_name.parse::<usize>().ok(),
        AudiobookPartNamingMode::NumberTitle => stem_name
            .strip_suffix(&format!(" - {stem}"))?
            .parse::<usize>()
            .ok(),
    }
}

fn collect_split_part_files(
    base: &Path,
    naming_mode: AudiobookPartNamingMode,
) -> Result<Vec<PathBuf>, String> {
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("audiobook");
    let ext = base
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_else(|| "m4b".to_string());
    let parent = base
        .parent()
        .ok_or_else(|| "Output path has no parent directory".to_string())?;
    let mut found: Vec<(usize, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(parent).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let Some(stem_name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(path_ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if !path_ext.eq_ignore_ascii_case(&ext) {
            continue;
        }
        let Some(idx) = parse_part_index_from_stem(stem_name, stem, naming_mode) else {
            continue;
        };
        if idx == 0 {
            continue;
        }
        found.push((idx, path));
    }
    found.sort_by_key(|(idx, _)| *idx);
    Ok(found.into_iter().map(|(_, p)| p).collect())
}

fn merge_m4b_parts_with_chapters(
    part_files: &[PathBuf],
    output: &Path,
    chapter_titles: Option<&[String]>,
) -> Result<(), String> {
    crate::ffmpeg_export::merge_audio_files_with_chapters_copy(part_files, output, chapter_titles)
}

fn split_parts_output_in_subfolder(output: &Path) -> PathBuf {
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("audiobook");
    let file_name = output
        .file_name()
        .map(|s| s.to_owned())
        .unwrap_or_else(|| "audiobook.mp3".into());
    let base_dir = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_default();
    base_dir.join(stem).join(file_name)
}

fn finish_time_split_part(
    part_output: &Path,
    wav_output: Option<PathBuf>,
    options: &AudiobookCommonOptions,
    is_aac: bool,
    is_mp3: bool,
) -> Result<(), String> {
    if !is_aac && !is_mp3 {
        return Ok(());
    }
    let Some(wav_path) = wav_output else {
        return Ok(());
    };
    let res = if is_aac {
        let settings = crate::ffmpeg_export::ConvertAudioSettings {
            format: crate::ffmpeg_export::ConvertAudioFormat::Aac,
            quality: crate::ffmpeg_export::ConvertAudioQuality::BitrateKbps(
                options.audiobook_bitrate_kbps,
            ),
        };
        let mut progress = |_p: u32| {};
        crate::ffmpeg_export::convert_audio_file(
            &wav_path,
            part_output,
            &settings,
            None,
            Some(&mut progress),
        )
    } else {
        let settings = crate::ffmpeg_export::ConvertAudioSettings {
            format: crate::ffmpeg_export::ConvertAudioFormat::Mp3,
            quality: crate::ffmpeg_export::ConvertAudioQuality::BitrateKbps(
                options.audiobook_bitrate_kbps,
            ),
        };
        let mut progress = |_p: u32| {};
        crate::ffmpeg_export::convert_audio_file(
            &wav_path,
            part_output,
            &settings,
            None,
            Some(&mut progress),
        )
    };
    if let Err(e) = std::fs::remove_file(&wav_path) {
        crate::log_debug(&format!(
            "Failed to remove temp wav after time split: {}",
            e
        ));
    }
    if let Err(e) = res {
        return Err(i18n::tr_f(
            options.language,
            "sapi5.mf_error",
            &[("err", &e)],
        ));
    }
    Ok(())
}

fn filter_time_split_chunks(chunks: &[String], engine: TtsEngine) -> Vec<String> {
    let mut out = Vec::new();
    let edge_max_bytes = EDGE_TTS_MAX_BYTES.saturating_sub(512).min(3000);
    for (idx, chunk) in chunks.iter().enumerate() {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            crate::log_debug(&format!("Time-split: skipping empty chunk index={}", idx));
            continue;
        }
        if engine == TtsEngine::Edge && !is_edge_text_usable(&sanitize_edge_text(trimmed)) {
            crate::log_debug(&format!(
                "Time-split: skipping Edge chunk with no speakable text after sanitization index={}",
                idx
            ));
            continue;
        }
        let len_utf16 = utf16_len(chunk);
        let len_trim_utf16 = utf16_len(trimmed);
        let byte_len = chunk.len();
        crate::log_debug(&format!(
            "Time-split: chunk index={} utf16_len={} trimmed_utf16_len={} bytes_len={}",
            idx, len_utf16, len_trim_utf16, byte_len
        ));
        if engine == TtsEngine::Edge && byte_len > edge_max_bytes {
            crate::log_debug(&format!(
                "Time-split: chunk index={} exceeds edge byte limit ({} > {}), re-splitting",
                idx, byte_len, edge_max_bytes
            ));
            for (sub_idx, sub) in split_long_sentence_edge_with_limit(chunk, edge_max_bytes)
                .into_iter()
                .enumerate()
            {
                if sub.trim().is_empty() || !is_edge_text_usable(&sanitize_edge_text(&sub)) {
                    crate::log_debug(&format!(
                        "Time-split: skipping empty or non-speakable Edge sub-chunk parent_index={} sub_index={}",
                        idx, sub_idx
                    ));
                    continue;
                }
                crate::log_debug(&format!(
                    "Time-split: sub-chunk parent_index={} sub_index={} bytes_len={}",
                    idx,
                    sub_idx,
                    sub.len()
                ));
                out.push(sub);
            }
            continue;
        }
        if len_utf16 > MAX_TTS_TEXT_LEN {
            crate::log_debug(&format!(
                "Time-split: chunk index={} exceeds max len ({} > {}), re-splitting",
                idx, len_utf16, MAX_TTS_TEXT_LEN
            ));
            for (sub_idx, sub) in split_text_for_engine(chunk, engine).into_iter().enumerate() {
                if sub.trim().is_empty()
                    || (engine == TtsEngine::Edge
                        && !is_edge_text_usable(&sanitize_edge_text(&sub)))
                {
                    crate::log_debug(&format!(
                        "Time-split: skipping empty or non-speakable sub-chunk parent_index={} sub_index={}",
                        idx, sub_idx
                    ));
                    continue;
                }
                crate::log_debug(&format!(
                    "Time-split: sub-chunk parent_index={} sub_index={} bytes_len={}",
                    idx,
                    sub_idx,
                    sub.len()
                ));
                out.push(sub);
            }
            continue;
        }
        out.push(chunk.clone());
    }
    out
}

fn run_split_audiobook_by_time_edge(
    chunks: &[String],
    split_minutes: u32,
    start_number: u32,
    options: AudiobookCommonOptions,
) -> Result<(), String> {
    if chunks.is_empty() {
        return Ok(());
    }
    let filtered_chunks = filter_time_split_chunks(chunks, TtsEngine::Edge);
    if filtered_chunks.is_empty() {
        return Err("No readable text after time-split filtering".to_string());
    }
    let split_seconds = split_minutes.saturating_mul(60) as f64;
    let extension = options
        .output
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let is_aac = extension == "m4b" || extension == "m4a" || extension == "mp4";
    let is_mp3 = extension == "mp3";

    let edge_chunks: Vec<TtsChunk> = filtered_chunks
        .iter()
        .map(|chunk| TtsChunk {
            text_to_read: chunk.clone(),
            original_len: utf16_len(chunk),
            override_voice: None,
            pause_ms: None,
        })
        .collect();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| err.to_string())?;

    rt.block_on(async {
        let stream_options = EdgeStreamOptions {
            voice: options.voice,
            rate: options.rate,
            pitch: options.pitch,
            volume: options.volume,
            language: options.language,
            cancel: options.cancel.as_ref(),
            progress_hwnd: options.progress_hwnd,
            allow_http_fallback: true,
        };

        const MAX_PARALLEL_CHUNKS: usize = 8;
        const CHUNK_RETRIES: usize = 6;
        let is_lithuanian_voice = options.voice.to_ascii_lowercase().starts_with("lt-");

        let mut pending: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
        let mut next_to_write = 0usize;
        let mut part_index: u32 = 0;
        let mut current_duration = 0.0f64;
        let mut part_writer: Option<BufWriter<std::fs::File>> = None;
        let mut wav_writer: Option<WavWriter> = None;
        let mut wav_path: Option<PathBuf> = None;
        let mut wav_rate: Option<u32> = None;
        let mut wav_channels: Option<u16> = None;
        let mut current_global_progress: usize = 0;

        let stream_options_ref = &stream_options;
        let mut stream = futures_util::stream::iter(edge_chunks.iter().enumerate())
            .map(|(idx, chunk)| {
                let opts = stream_options_ref;
                async move {
                    let res = download_edge_audiobook_chunk_validated(
                        chunk,
                        opts,
                        idx,
                        CHUNK_RETRIES,
                        is_lithuanian_voice,
                    )
                    .await;
                    (idx, res)
                }
            })
            .buffer_unordered(MAX_PARALLEL_CHUNKS);

        while let Some((idx, res)) = stream.next().await {
            if options.cancel.load(Ordering::Relaxed) {
                return Err(cancelled_message(options.language));
            }
            let audio = res?;
            pending.insert(idx, audio);

            while let Some(data) = pending.remove(&next_to_write) {
                if options.cancel.load(Ordering::Relaxed) {
                    return Err(cancelled_message(options.language));
                }

                let (samples, src_rate, src_channels) = decode_mp3_to_pcm(&data).map_err(|err| {
                    format!(
                        "Edge audiobook: validated chunk {} could not be decoded during time-split writing: {}",
                        next_to_write, err
                    )
                })?;
                if samples.is_empty() {
                    return Err(format!(
                        "Edge audiobook: validated chunk {} contained no decoded samples during time-split writing",
                        next_to_write
                    ));
                }
                let chunk_duration = samples.len() as f64 / (src_rate as f64 * src_channels as f64);

                if split_seconds > 0.0
                    && current_duration > 0.0
                    && current_duration + chunk_duration > split_seconds
                {
                    if let Some(mut writer) = part_writer.take()
                        && let Err(e) = writer.flush()
                    {
                        crate::log_debug(&format!("Failed to flush time-split part: {}", e));
                    }
                    if let Some(mut w) = wav_writer.take()
                        && let Err(e) = w.finalize()
                    {
                        crate::log_debug(&format!("Failed to finalize time-split wav: {}", e));
                    }
                    let part_output = time_split_part_output(
                        options.output,
                        part_index,
                        start_number,
                        options.part_naming_mode,
                    );
                    finish_time_split_part(
                        &part_output,
                        wav_path.take(),
                        &options,
                        is_aac,
                        is_mp3,
                    )?;
                    part_index = part_index.saturating_add(1);
                    current_duration = 0.0;
                    wav_rate = None;
                    wav_channels = None;
                }

                if part_writer.is_none() && wav_writer.is_none() {
                    let part_output = time_split_part_output(
                        options.output,
                        part_index,
                        start_number,
                        options.part_naming_mode,
                    );
                    if is_aac || is_mp3 {
                        let tmp_wav = part_output.with_extension("wav.tmp");
                        wav_path = Some(tmp_wav.clone());
                        wav_rate = Some(src_rate);
                        wav_channels = Some(src_channels);
                        let writer = WavWriter::create(&tmp_wav, src_rate, src_channels, 16)
                            .map_err(|e| e.to_string())?;
                        wav_writer = Some(writer);
                    } else {
                        let file =
                            std::fs::File::create(&part_output).map_err(|e| e.to_string())?;
                        part_writer = Some(BufWriter::new(file));
                    }
                    if let Some(announcement) = audiobook_part_announcement_text(
                        &options,
                        &part_output,
                        start_number.saturating_add(part_index),
                    ) {
                        let announcement_chunk = TtsChunk {
                            original_len: utf16_len(&announcement),
                            text_to_read: announcement,
                            override_voice: None,
                            pause_ms: None,
                        };
                        let announcement_audio = download_edge_audiobook_chunk_validated(
                            &announcement_chunk,
                            &stream_options,
                            usize::MAX.saturating_sub(part_index as usize),
                            CHUNK_RETRIES,
                            is_lithuanian_voice,
                        )
                        .await?;
                        if let Some(ref mut w) = wav_writer {
                            let (announcement_samples, announcement_rate, announcement_channels) =
                                decode_mp3_to_pcm(&announcement_audio)?;
                            let target_rate = wav_rate.unwrap_or(announcement_rate);
                            let target_channels = wav_channels.unwrap_or(announcement_channels);
                            let announcement_duration = announcement_samples.len() as f64
                                / (announcement_rate as f64 * announcement_channels as f64);
                            let output_samples = if announcement_rate != target_rate
                                || announcement_channels != target_channels
                            {
                                resample_pcm(
                                    &announcement_samples,
                                    announcement_rate,
                                    announcement_channels,
                                    target_rate,
                                    target_channels,
                                )
                            } else {
                                announcement_samples
                            };
                            w.write_samples_f32(&output_samples).map_err(|e| e.to_string())?;
                            current_duration += announcement_duration;
                        } else if let Some(ref mut writer) = part_writer {
                            writer.write_all(&announcement_audio).map_err(|e| e.to_string())?;
                        }
                    }
                }

                if let Some(ref mut w) = wav_writer {
                    let target_rate = wav_rate.unwrap_or(src_rate);
                    let target_channels = wav_channels.unwrap_or(src_channels);
                    let out_samples = if src_rate != target_rate || src_channels != target_channels
                    {
                        resample_pcm(
                            &samples,
                            src_rate,
                            src_channels,
                            target_rate,
                            target_channels,
                        )
                    } else {
                        samples
                    };
                    w.write_samples_f32(&out_samples)
                        .map_err(|e| e.to_string())?;
                } else if let Some(ref mut writer) = part_writer {
                    writer.write_all(&data).map_err(|e| e.to_string())?;
                }

                current_duration += chunk_duration;
                current_global_progress = current_global_progress.saturating_add(1);
                if options.progress_hwnd.0 != 0 {
                    unsafe {
                        if let Err(e) = PostMessageW(
                            options.progress_hwnd,
                            crate::WM_UPDATE_PROGRESS,
                            WPARAM(current_global_progress),
                            LPARAM(0),
                        ) {
                            crate::log_debug(&format!("Failed to post WM_UPDATE_PROGRESS: {}", e));
                        }
                    }
                }
                next_to_write = next_to_write.saturating_add(1);
            }
        }

        if next_to_write != edge_chunks.len() {
            return Err(format!(
                "Edge audiobook time-split integrity check failed: produced {} chunks out of {}",
                next_to_write,
                edge_chunks.len()
            ));
        }

        if let Some(mut writer) = part_writer.take()
            && let Err(e) = writer.flush()
        {
            crate::log_debug(&format!("Failed to flush time-split part: {}", e));
        }
        if let Some(mut w) = wav_writer.take()
            && let Err(e) = w.finalize()
        {
            crate::log_debug(&format!("Failed to finalize time-split wav: {}", e));
        }
        if next_to_write > 0 {
            let part_output = time_split_part_output(
                options.output,
                part_index,
                start_number,
                options.part_naming_mode,
            );
            finish_time_split_part(&part_output, wav_path.take(), &options, is_aac, is_mp3)?;
        }
        Ok(())
    })
}

fn run_split_audiobook_by_time_sapi(
    engine: TtsEngine,
    chunks: &[String],
    split_minutes: u32,
    start_number: u32,
    options: AudiobookCommonOptions,
) -> Result<(), String> {
    if chunks.is_empty() {
        return Ok(());
    }
    let filtered_chunks = filter_time_split_chunks(chunks, engine);
    if filtered_chunks.is_empty() {
        return Err("No readable text after time-split filtering".to_string());
    }
    let split_seconds = split_minutes.saturating_mul(60) as f64;
    let extension = options
        .output
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let is_aac = extension == "m4b" || extension == "m4a" || extension == "mp4";
    let is_mp3 = extension == "mp3";

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let mut current_duration = 0.0f64;
    let mut part_index: u32 = 0;
    let mut wav_writer: Option<WavWriter> = None;
    let mut wav_path: Option<PathBuf> = None;
    let mut wav_rate: Option<u32> = None;
    let mut wav_channels: Option<u16> = None;
    let mut current_global_progress: usize = 0;

    for chunk in filtered_chunks.iter() {
        if options.cancel.load(Ordering::Relaxed) {
            return Err(cancelled_message(options.language));
        }
        let config = SynthesisConfig {
            engine,
            voice: options.voice.to_string(),
            rate: options.rate,
            pitch: options.pitch,
            volume: options.volume,
            language: options.language,
            cancel: options.cancel.clone(),
        };
        let bytes = rt.block_on(synthesize_segment_bytes(chunk, &config))?;
        let (samples, src_rate, src_channels) = decode_wav_to_pcm(&bytes)?;
        let chunk_duration = samples.len() as f64 / (src_rate as f64 * src_channels as f64);

        if split_seconds > 0.0
            && current_duration > 0.0
            && current_duration + chunk_duration > split_seconds
        {
            if let Some(mut w) = wav_writer.take()
                && let Err(e) = w.finalize()
            {
                crate::log_debug(&format!("Failed to finalize time-split wav: {}", e));
            }
            let part_output = time_split_part_output(
                options.output,
                part_index,
                start_number,
                options.part_naming_mode,
            );
            finish_time_split_part(&part_output, wav_path.take(), &options, is_aac, is_mp3)?;
            part_index = part_index.saturating_add(1);
            current_duration = 0.0;
            wav_rate = None;
            wav_channels = None;
        }

        if wav_writer.is_none() {
            let part_output = time_split_part_output(
                options.output,
                part_index,
                start_number,
                options.part_naming_mode,
            );
            if is_aac || is_mp3 {
                let tmp_wav = part_output.with_extension("wav.tmp");
                wav_path = Some(tmp_wav.clone());
                wav_rate = Some(src_rate);
                wav_channels = Some(src_channels);
                let writer = WavWriter::create(&tmp_wav, src_rate, src_channels, 16)
                    .map_err(|e| e.to_string())?;
                wav_writer = Some(writer);
            } else {
                wav_rate = Some(src_rate);
                wav_channels = Some(src_channels);
                let writer = WavWriter::create(&part_output, src_rate, src_channels, 16)
                    .map_err(|e| e.to_string())?;
                wav_writer = Some(writer);
            }
            if let Some(announcement) = audiobook_part_announcement_text(
                &options,
                &part_output,
                start_number.saturating_add(part_index),
            ) {
                let announcement_bytes =
                    rt.block_on(synthesize_segment_bytes(&announcement, &config))?;
                let (announcement_samples, announcement_rate, announcement_channels) =
                    decode_wav_to_pcm(&announcement_bytes)?;
                let target_rate = wav_rate.unwrap_or(announcement_rate);
                let target_channels = wav_channels.unwrap_or(announcement_channels);
                let announcement_duration = announcement_samples.len() as f64
                    / (announcement_rate as f64 * announcement_channels as f64);
                let output_samples = if announcement_rate != target_rate
                    || announcement_channels != target_channels
                {
                    resample_pcm(
                        &announcement_samples,
                        announcement_rate,
                        announcement_channels,
                        target_rate,
                        target_channels,
                    )
                } else {
                    announcement_samples
                };
                if let Some(ref mut writer) = wav_writer {
                    writer
                        .write_samples_f32(&output_samples)
                        .map_err(|e| e.to_string())?;
                }
                current_duration += announcement_duration;
            }
        }

        if let Some(ref mut w) = wav_writer {
            let target_rate = wav_rate.unwrap_or(src_rate);
            let target_channels = wav_channels.unwrap_or(src_channels);
            let out_samples = if src_rate != target_rate || src_channels != target_channels {
                resample_pcm(
                    &samples,
                    src_rate,
                    src_channels,
                    target_rate,
                    target_channels,
                )
            } else {
                samples
            };
            w.write_samples_f32(&out_samples)
                .map_err(|e| e.to_string())?;
        }

        current_duration += chunk_duration;
        current_global_progress = current_global_progress.saturating_add(1);
        if options.progress_hwnd.0 != 0 {
            unsafe {
                if let Err(e) = PostMessageW(
                    options.progress_hwnd,
                    crate::WM_UPDATE_PROGRESS,
                    WPARAM(current_global_progress),
                    LPARAM(0),
                ) {
                    crate::log_debug(&format!("Failed to post WM_UPDATE_PROGRESS: {}", e));
                }
            }
        }
    }

    if let Some(mut w) = wav_writer.take()
        && let Err(e) = w.finalize()
    {
        crate::log_debug(&format!("Failed to finalize time-split wav: {}", e));
    }
    if !chunks.is_empty() {
        let part_output = time_split_part_output(
            options.output,
            part_index,
            start_number,
            options.part_naming_mode,
        );
        finish_time_split_part(&part_output, wav_path.take(), &options, is_aac, is_mp3)?;
    }

    Ok(())
}

async fn download_edge_chunks_ws_parallel_to_writer(
    chunks: &[TtsChunk],
    start_index: usize,
    options: &EdgeStreamOptions<'_>,
    writer: &mut dyn std::io::Write,
    current_global_progress: &mut usize,
) -> Result<usize, String> {
    const MAX_PARALLEL_CHUNKS: usize = 8;
    const CHUNK_RETRIES: usize = 6;
    let is_lithuanian_voice = options.voice.to_ascii_lowercase().starts_with("lt-");

    if start_index >= chunks.len() {
        return Ok(chunks.len());
    }

    let mut pending: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    let mut next_to_write = start_index;
    let mut written_chunks = 0usize;
    let mut total_audio_bytes = 0usize;

    let mut stream = futures_util::stream::iter(chunks.iter().enumerate().skip(start_index))
        .map(|(idx, chunk)| async move {
            let result = download_edge_audiobook_chunk_validated(
                chunk,
                options,
                idx,
                CHUNK_RETRIES,
                is_lithuanian_voice,
            )
            .await;
            (idx, result)
        })
        .buffer_unordered(MAX_PARALLEL_CHUNKS);

    while let Some((idx, res)) = stream.next().await {
        if options.cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }
        let audio = res?;
        pending.insert(idx, audio);
        while let Some(data) = pending.remove(&next_to_write) {
            writer.write_all(&data).map_err(|err| err.to_string())?;
            writer.flush().map_err(|err| err.to_string())?;
            written_chunks = written_chunks.saturating_add(1);
            total_audio_bytes = total_audio_bytes.saturating_add(data.len());

            *current_global_progress += 1;
            if options.progress_hwnd.0 != 0 {
                unsafe {
                    if let Err(e) = PostMessageW(
                        options.progress_hwnd,
                        crate::WM_UPDATE_PROGRESS,
                        WPARAM(*current_global_progress),
                        LPARAM(0),
                    ) {
                        crate::log_debug(&format!("Failed to post WM_UPDATE_PROGRESS: {}", e));
                    }
                }
            }
            next_to_write = next_to_write.saturating_add(1);
        }
    }

    let expected_chunks = chunks.len().saturating_sub(start_index);
    crate::log_debug(&format!(
        "Edge WS export summary: expected_chunks={} written_chunks={} total_audio_bytes={}",
        expected_chunks, written_chunks, total_audio_bytes
    ));
    if written_chunks != expected_chunks {
        return Err(format!(
            "Edge audiobook integrity check failed: produced {} chunks out of {}",
            written_chunks, expected_chunks
        ));
    }

    Ok(chunks.len())
}

fn get_text_from_caret(hwnd_edit: HWND) -> (String, i32) {
    unsafe {
        let mut range = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut range as *mut _ as isize),
        );
        let caret_pos = range.cpMin.min(range.cpMax).max(0);
        let total_len = SendMessageW(hwnd_edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as i32;
        if total_len <= 0 {
            return (String::new(), 0);
        }
        let full_text = get_text_range(hwnd_edit, 0, total_len);

        // Se siamo all'inizio, o se siamo alla fine del testo, leggi tutto dall'inizio
        if caret_pos == 0 {
            return (full_text, 0);
        }

        // Se la posizione del cursore Š oltre la lunghezza del testo (fine file),
        // ricomincia a leggere dall'inizio come richiesto.
        if caret_pos >= total_len {
            return (full_text, 0);
        }

        let prefix = get_text_range(hwnd_edit, 0, caret_pos);
        let caret_utf16 = prefix.encode_utf16().count() as i32;

        let wide: Vec<u16> = full_text.encode_utf16().collect();

        let adjusted_pos = adjust_tts_caret_pos(&full_text, caret_utf16);
        let adjusted_pos = adjusted_pos.max(0) as usize;
        if adjusted_pos >= wide.len() {
            return (full_text, 0);
        }
        (
            String::from_utf16_lossy(&wide[adjusted_pos..]),
            adjusted_pos as i32,
        )
    }
}

fn get_text_range(hwnd_edit: HWND, start: i32, end: i32) -> String {
    unsafe {
        let len = (end - start).max(0) as usize;
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len + 1];
        let mut text_range = TEXTRANGEW {
            chrg: CHARRANGE {
                cpMin: start,
                cpMax: end,
            },
            lpstrText: PWSTR(buf.as_mut_ptr()),
        };
        let copied = SendMessageW(
            hwnd_edit,
            EM_GETTEXTRANGE,
            WPARAM(0),
            LPARAM(&mut text_range as *mut _ as isize),
        )
        .0 as usize;
        let used = copied.min(len);
        String::from_utf16_lossy(&buf[..used])
    }
}

fn adjust_tts_caret_pos(text: &str, pos: i32) -> i32 {
    if pos <= 0 {
        return 0;
    }
    let mut items: Vec<(usize, usize, bool)> = Vec::new();
    let mut offset = 0usize;
    for ch in text.chars() {
        let start = offset;
        let len = ch.len_utf16();
        let end = start + len;
        let is_word = ch.is_alphanumeric() || ch == '_';
        items.push((start, end, is_word));
        offset = end;
    }
    if offset == 0 {
        return pos;
    }
    let mut pos_usize = pos as usize;
    if pos_usize > offset {
        pos_usize = offset;
    }

    let mut prev: Option<usize> = None;
    let mut next: Option<usize> = None;
    for (idx, (start, end, _)) in items.iter().enumerate() {
        if *end <= pos_usize {
            prev = Some(idx);
            continue;
        }
        if *start >= pos_usize {
            next = Some(idx);
            break;
        }
        next = Some(idx);
        break;
    }

    let prev_is_word = prev
        .and_then(|idx| items.get(idx))
        .map(|v| v.2)
        .unwrap_or(false);
    let next_is_word = next
        .and_then(|idx| items.get(idx))
        .map(|v| v.2)
        .unwrap_or(false);
    if prev_is_word
        && next_is_word
        && let Some(mut idx) = prev
    {
        while idx > 0 && items[idx - 1].2 {
            idx -= 1;
        }
        return items[idx].0 as i32;
    }
    pos
}
fn generate_sec_ms_gec() -> String {
    let win_epoch = 11644473600i64;
    let ticks = Local::now().timestamp() + win_epoch;
    let ticks = (ticks - (ticks % 300)) * 10_000_000;
    let str_to_hash = format!("{}{}", ticks, TRUSTED_CLIENT_TOKEN);
    let mut hasher = Sha256::new();
    hasher.update(str_to_hash);
    hex::encode(hasher.finalize()).to_uppercase()
}

fn generate_muid() -> String {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 16];
    rng.fill(&mut bytes);
    hex::encode(bytes).to_uppercase()
}

fn get_date_string() -> String {
    Local::now()
        .format("%a %b %d %Y %H:%M:%S GMT+0000 (Coordinated Universal Time)")
        .to_string()
}

fn format_rate(rate: i32) -> String {
    format!("{:+}%", rate)
}

fn format_pitch(pitch: i32) -> String {
    format!("{:+}Hz", pitch)
}

fn format_volume(volume: i32) -> String {
    let delta = volume.saturating_sub(100);
    format!("{:+}%", delta)
}

fn mkssml(text: &str, voice: &str, tts_rate: i32, tts_pitch: i32, tts_volume: i32) -> String {
    let lang = voice.split('-').collect::<Vec<_>>();
    let lang = if lang.len() >= 3 {
        lang[0..2].join("-")
    } else {
        "en-US".to_string()
    };
    let rendered_text = render_edge_ssml_text_with_pause_tags(text);
    format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='{}'><voice name='{}'><prosody pitch='{}' rate='{}' volume='{}'>{}</prosody></voice></speak>",
        lang,
        voice,
        format_pitch(tts_pitch),
        format_rate(tts_rate),
        format_volume(tts_volume),
        rendered_text
    )
}

fn parse_pause_tag_milliseconds(tag: &str) -> Option<u32> {
    let trimmed = tag.trim();
    let inner = trimmed
        .strip_prefix('<')?
        .strip_suffix('>')?
        .trim()
        .trim_end_matches('/')
        .trim();
    let rest = inner.strip_prefix("pause")?.trim();
    if rest.is_empty() {
        return None;
    }
    for token in rest.split_whitespace() {
        let value = token
            .strip_prefix("ms=")
            .or_else(|| token.strip_prefix("milliseconds="))
            .unwrap_or(token)
            .trim_matches(['"', '\'']);
        if let Ok(ms) = value.parse::<u32>()
            && (PAUSE_TAG_MIN_MS..=PAUSE_TAG_MAX_MS).contains(&ms)
        {
            return Some(ms);
        }
    }
    None
}

enum PauseSplitSegment {
    Text(String, usize),
    Pause(u32, usize),
}

fn split_pause_tag_segments(text: &str) -> Vec<PauseSplitSegment> {
    if text.is_empty() {
        return Vec::new();
    }
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut cursor = 0usize;
    let mut i = 0usize;
    let bytes = lower.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let remaining = &lower[i..];
            if remaining.starts_with("<pause")
                && let Some(end_rel) = remaining.find('>')
            {
                let end = i + end_rel + 1;
                if let Some(ms) = parse_pause_tag_milliseconds(&lower[i..end]) {
                    if i > cursor {
                        let text_part = decode_basic_xml_entities(&text[cursor..i]);
                        if !text_part.is_empty() {
                            out.push(PauseSplitSegment::Text(
                                text_part,
                                utf16_len(&text[cursor..i]),
                            ));
                        }
                    }
                    out.push(PauseSplitSegment::Pause(ms, utf16_len(&text[i..end])));
                    cursor = end;
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    if cursor < text.len() {
        let text_part = decode_basic_xml_entities(&text[cursor..]);
        if !text_part.is_empty() {
            out.push(PauseSplitSegment::Text(
                text_part,
                utf16_len(&text[cursor..]),
            ));
        }
    }
    if out.is_empty() {
        out.push(PauseSplitSegment::Text(
            decode_basic_xml_entities(text),
            utf16_len(text),
        ));
    }
    out
}

fn render_ssml_text_with_pause_tags(text: &str, pause_markup: impl Fn(u32) -> String) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut i = 0usize;
    let bytes = lower.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let remaining = &lower[i..];
            if remaining.starts_with("<pause")
                && let Some(end_rel) = remaining.find('>')
            {
                let end = i + end_rel + 1;
                let tag = &text[i..end];
                if let Some(ms) = parse_pause_tag_milliseconds(&lower[i..end]) {
                    if i > cursor {
                        out.push_str(&escape_xml(&text[cursor..i]));
                    }
                    out.push_str(&pause_markup(ms));
                    cursor = end;
                    i = end;
                    continue;
                }
                if i > cursor {
                    out.push_str(&escape_xml(&text[cursor..i]));
                }
                out.push_str(&escape_xml(tag));
                cursor = end;
                i = end;
                continue;
            }
        }
        i += 1;
    }
    if cursor < text.len() {
        out.push_str(&escape_xml(&text[cursor..]));
    }
    out
}

pub(crate) fn render_edge_ssml_text_with_pause_tags(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut i = 0usize;
    let bytes = lower.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let remaining = &lower[i..];
            if remaining.starts_with("<pause")
                && let Some(end_rel) = remaining.find('>')
            {
                let end = i + end_rel + 1;
                if let Some(ms) = parse_pause_tag_milliseconds(&lower[i..end]) {
                    out.push_str(&text[cursor..i]);
                    out.push_str(&format!("<break time=\"{ms}ms\"/>"));
                    cursor = end;
                    i = end;
                    continue;
                }
            }
        } else if bytes[i] == b'&' {
            let remaining = &lower[i..];
            if remaining.starts_with("&lt;pause")
                && let Some(end_rel) = remaining.find("&gt;")
            {
                let end = i + end_rel + "&gt;".len();
                let decoded = decode_basic_xml_entities(&lower[i..end]);
                if let Some(ms) = parse_pause_tag_milliseconds(&decoded) {
                    out.push_str(&text[cursor..i]);
                    out.push_str(&format!("<break time=\"{ms}ms\"/>"));
                    cursor = end;
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    if cursor < text.len() {
        out.push_str(&text[cursor..]);
    }
    out
}

pub(crate) fn render_sapi_ssml_text_with_pause_tags(text: &str) -> String {
    render_ssml_text_with_pause_tags(text, |ms| format!("<silence msec=\"{ms}\"/>"))
}

pub fn remove_long_dash_runs(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut dash_run = 0;
    for ch in line.chars() {
        if ch == '-' {
            dash_run += 1;
            continue;
        }
        if dash_run > 0 {
            if dash_run < 3 {
                out.extend(std::iter::repeat_n('-', dash_run));
            }
            dash_run = 0;
        }
        out.push(ch);
    }
    if dash_run > 0 && dash_run < 3 {
        out.extend(std::iter::repeat_n('-', dash_run));
    }
    out
}

pub fn strip_dashed_lines(text: &str) -> String {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return Some(String::new());
            }
            let cleaned = remove_long_dash_runs(line);
            if cleaned.trim().is_empty() {
                None
            } else {
                Some(cleaned)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_weird_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            // Remove zero-width formatting chars that can confuse TTS segmentation.
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}' => {}
            // Convert uncommon Unicode spaces to ASCII space.
            '\u{00A0}' | '\u{1680}' | '\u{2000}' | '\u{2001}' | '\u{2002}' | '\u{2003}'
            | '\u{2004}' | '\u{2005}' | '\u{2006}' | '\u{2007}' | '\u{2008}' | '\u{2009}'
            | '\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
            // Visible symbols for spaces/tabs used in some copied texts.
            | '\u{2420}' | '\u{2423}' | '\u{2409}' => out.push(' '),
            // Visible symbols for LF/CR/newline.
            '\u{240A}' | '\u{240D}' | '\u{2424}' => out.push('\n'),
            _ => out.push(ch),
        }
    }
    out
}

#[derive(Clone, Copy)]
enum TtsSanitizeProfile {
    Safe,
    Strict,
}

fn normalize_tts_strict_chars(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let mapped = match ch {
            // Smart/directional double quotes -> plain quote
            '“' | '”' | '„' | '‟' | '«' | '»' | '〝' | '〞' => '"',
            // Smart/directional single quotes/apostrophes -> plain apostrophe
            '‘' | '’' | '‚' | '‛' | '‹' | '›' | '`' | '´' => '\'',
            // Long dashes/minus variants -> hyphen-minus
            '–' | '—' | '―' | '−' => '-',
            _ => ch,
        };

        // Horizontal ellipsis -> three dots
        if mapped == '…' {
            out.push_str("...");
            continue;
        }

        // Keep only letters/digits/whitespace and a conservative punctuation set.
        let safe_punct = matches!(
            mapped,
            '.' | ','
                | ';'
                | ':'
                | '!'
                | '?'
                | '"'
                | '\''
                | '-'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '/'
                | '\\'
                | '@'
                | '#'
                | '%'
                | '&'
                | '+'
                | '='
                | '*'
                | '_'
                | '<'
                | '>'
                | '|'
                | '~'
        );

        if mapped.is_alphanumeric() || mapped.is_whitespace() || safe_punct {
            out.push(mapped);
        } else {
            out.push(' ');
        }
    }
    out
}

fn preview_for_log(text: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(max_chars);
    for ch in text.chars().take(max_chars) {
        if ch == '\n' || ch == '\r' || ch == '\t' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

fn normalize_for_tts_with_profile(
    text: &str,
    split_on_newline: bool,
    profile: TtsSanitizeProfile,
) -> String {
    let normalized_spaces = normalize_weird_spaces(text);
    let mut normalized = if split_on_newline {
        normalized_spaces
    } else {
        normalized_spaces.replace('\n', " ").replace('\r', "")
    };
    match profile {
        TtsSanitizeProfile::Safe => normalized.replace(['«', '»'], ""),
        TtsSanitizeProfile::Strict => {
            normalized = normalize_tts_strict_chars(&normalized);
            normalized
        }
    }
}

pub fn normalize_for_tts(text: &str, split_on_newline: bool) -> String {
    normalize_for_tts_with_profile(text, split_on_newline, TtsSanitizeProfile::Safe)
}

fn apply_dictionary(text: &str, dictionary: &[DictionaryEntry]) -> String {
    if dictionary.is_empty() {
        return text.to_string();
    }
    let mut out = text.to_string();
    for entry in dictionary {
        if entry.original.is_empty() {
            continue;
        }
        out = if entry.match_case {
            out.replace(&entry.original, &entry.replacement)
        } else {
            replace_case_insensitive(&out, &entry.original, &entry.replacement)
        };
    }
    out
}

fn replace_case_insensitive(text: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    while cursor < text.len() {
        if let Some(match_len) = match_len_case_insensitive(text, cursor, needle) {
            out.push_str(replacement);
            cursor += match_len;
            continue;
        }
        let Some(ch) = text[cursor..].chars().next() else {
            break;
        };
        out.push(ch);
        cursor += ch.len_utf8();
    }
    out
}

fn match_len_case_insensitive(text: &str, start: usize, needle: &str) -> Option<usize> {
    let mut consumed = 0;
    let mut text_chars = text[start..].chars();
    for needle_ch in needle.chars() {
        let text_ch = text_chars.next()?;
        if text_ch.to_lowercase().to_string() != needle_ch.to_lowercase().to_string() {
            return None;
        }
        consumed += text_ch.len_utf8();
    }
    Some(consumed)
}

fn normalize_ellipsis_for_tts(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut dot_run = 0usize;

    let flush_dots = |out: &mut String, dot_run: &mut usize| {
        if *dot_run >= 3 {
            out.push('.');
        } else {
            out.extend(std::iter::repeat_n('.', *dot_run));
        }
        *dot_run = 0;
    };

    for ch in text.chars() {
        if ch == '.' {
            dot_run += 1;
            continue;
        }
        flush_dots(&mut out, &mut dot_run);
        out.push(ch);
    }
    flush_dots(&mut out, &mut dot_run);
    out
}

pub(crate) fn prepare_tts_text(
    text: &str,
    split_on_newline: bool,
    dictionary: &[DictionaryEntry],
) -> String {
    let normalized = normalize_for_tts(text, split_on_newline);
    let prepared = apply_dictionary(&normalized, dictionary);
    normalize_ellipsis_for_tts(&prepared)
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub(crate) struct MarkerEntry {
    pub(crate) pos: usize,
    pub(crate) label: String,
}

fn marker_label_for_position(text: &str, pos: usize, marker: &str) -> String {
    let start = text[..pos].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let end = text[pos..]
        .find('\n')
        .map(|idx| pos + idx)
        .unwrap_or(text.len());
    let line = text[start..end].trim();
    if line.is_empty() {
        marker.to_string()
    } else {
        line.to_string()
    }
}

pub(crate) fn collect_marker_entries(
    text: &str,
    marker: &str,
    require_newline: bool,
) -> (String, Vec<MarkerEntry>) {
    let normalized = normalize_newlines(text);
    if marker.trim().is_empty() {
        return (normalized, Vec::new());
    }

    let mut entries = Vec::new();
    for (idx, _) in normalized.match_indices(marker) {
        // If require_newline is true, we ensure the marker is at the start of a line.
        // normalized only has \n (no \r), but we check both just in case or for robustness.
        // We also allow the start of the file (idx == 0).
        if require_newline && idx > 0 {
            let prefix = &normalized[..idx];
            if !prefix.ends_with('\n') {
                continue;
            }
        }
        let label = marker_label_for_position(&normalized, idx, marker);
        entries.push(MarkerEntry { pos: idx, label });
    }

    (normalized, entries)
}

fn split_text_by_positions(text: &str, positions: &[usize]) -> Option<Vec<String>> {
    if positions.is_empty() {
        return None;
    }

    let mut positions = positions.to_vec();
    positions.sort_unstable();
    positions.dedup();

    let mut parts = Vec::new();
    let mut start = 0usize;
    for pos in positions.iter() {
        if *pos == 0 {
            continue;
        }
        // Ensure we don't go out of bounds and that we advance
        if *pos > start && *pos <= text.len() {
            parts.push(text[start..*pos].to_string());
            start = *pos;
        }
    }
    // Push the remainder of the text
    if start < text.len() {
        parts.push(text[start..].to_string());
    } else if start == text.len() && parts.is_empty() {
        // Edge case: empty text or positions at end?
        // If text is not empty, but start reached end, we are done.
    }
    Some(parts)
}

pub(crate) fn build_audiobook_parts_from_sections(
    sections: &[String],
    split_on_newline: bool,
    dictionary: &[DictionaryEntry],
    engine: TtsEngine,
) -> Option<Vec<Vec<String>>> {
    let mut parts = Vec::new();
    for (idx, section) in sections.iter().enumerate() {
        let cleaned = strip_dashed_lines(section);
        let prepared = prepare_tts_text(&cleaned, split_on_newline, dictionary);
        let chunks = split_text_for_engine(&prepared, engine);
        if chunks.is_empty() {
            crate::log_debug(&format!("EPUB split: skipping empty chapter index={}", idx));
            continue;
        }
        parts.push(chunks);
    }
    if parts.is_empty() { None } else { Some(parts) }
}

pub(crate) fn build_audiobook_parts_by_positions(
    text: &str,
    positions: &[usize],
    split_on_newline: bool,
    dictionary: &[DictionaryEntry],
    engine: TtsEngine,
) -> Option<Vec<Vec<String>>> {
    let parts_text = split_text_by_positions(text, positions)?;
    let mut parts_chunks = Vec::new();

    for part_text in parts_text {
        let prepared = prepare_tts_text(&part_text, split_on_newline, dictionary);
        let chunks = split_text_for_engine(&prepared, engine);
        // Even if chunks is empty (e.g. only whitespace part), we might want to keep the structure?
        // But run_tts_audiobook_part handles empty chunks by skipping.
        if !chunks.is_empty() {
            parts_chunks.push(chunks);
        } else {
            // If a part is empty, we push an empty vec to maintain index alignment if needed,
            // though currently the caller just iterates.
            parts_chunks.push(Vec::new());
        }
    }

    if parts_chunks.is_empty() {
        None
    } else {
        Some(parts_chunks)
    }
}

pub(crate) fn build_mixed_audiobook_parts_from_sections(
    sections: &[String],
    split_on_newline: bool,
    dictionary: &[DictionaryEntry],
    tts_engine: TtsEngine,
) -> Option<Vec<Vec<TtsChunk>>> {
    let mut parts = Vec::new();
    for (idx, section) in sections.iter().enumerate() {
        let cleaned = strip_dashed_lines(section);
        let chunks = split_into_tts_chunks(&cleaned, split_on_newline, dictionary, tts_engine);
        if chunks.is_empty() {
            crate::log_debug(&format!(
                "EPUB split (mixed): skipping empty chapter index={}",
                idx
            ));
            continue;
        }
        parts.push(chunks);
    }
    if parts.is_empty() { None } else { Some(parts) }
}

pub(crate) fn build_mixed_audiobook_parts_by_positions(
    text: &str,
    positions: &[usize],
    split_on_newline: bool,
    dictionary: &[DictionaryEntry],
    tts_engine: TtsEngine,
) -> Option<Vec<Vec<TtsChunk>>> {
    let parts_text = split_text_by_positions(text, positions)?;
    let mut parts_chunks = Vec::new();

    for part_text in parts_text {
        let chunks = split_into_tts_chunks(&part_text, split_on_newline, dictionary, tts_engine);
        parts_chunks.push(chunks);
    }

    if parts_chunks.is_empty() {
        None
    } else {
        Some(parts_chunks)
    }
}

pub fn split_text(text: &str) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let text = text.trim();
    if text.is_empty() {
        return chunks;
    }

    let char_len = text.chars().count();
    let is_long = char_len > TTS_LONG_TEXT_THRESHOLD;
    let max_len = if is_long {
        MAX_TTS_TEXT_LEN_LONG
    } else {
        MAX_TTS_TEXT_LEN
    };
    let target_chunk_len = if is_long {
        MAX_TTS_FIRST_CHUNK_LEN_LONG
    } else {
        max_len
    };

    let sentences = split_sentences(text);
    let mut current = String::new();
    let mut current_limit = target_chunk_len;

    for sentence in sentences {
        let sentence_trim = sentence.trim();
        if sentence_trim.is_empty() {
            continue;
        }
        let sentence_len = sentence_trim.chars().count();
        if current.is_empty() {
            if sentence_len > current_limit {
                let parts = split_long_sentence_by_whitespace(sentence_trim, current_limit);
                for part in parts {
                    chunks.push(part);
                }
                current_limit = max_len;
                continue;
            }
            current.push_str(sentence_trim);
        } else {
            let combined_len = current
                .chars()
                .count()
                .saturating_add(1)
                .saturating_add(sentence_len);
            if combined_len > current_limit {
                chunks.push(current.trim().to_string());
                current.clear();
                current_limit = max_len;

                if sentence_len > current_limit {
                    let parts = split_long_sentence_by_whitespace(sentence_trim, current_limit);
                    for part in parts {
                        chunks.push(part);
                    }
                } else {
                    current.push_str(sentence_trim);
                }
            } else {
                current.push(' ');
                current.push_str(sentence_trim);
            }
        }
    }

    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }

    chunks
}

fn sanitize_edge_text(text: &str) -> String {
    let normalized_spaces = normalize_weird_spaces(text);
    let mut out = String::with_capacity(normalized_spaces.len());
    let mut dot_run = 0usize;
    let mut bang_run = 0usize;
    let mut question_run = 0usize;

    for ch in normalized_spaces.chars() {
        if ch == '.' {
            dot_run += 1;
            continue;
        }
        if ch == '!' {
            bang_run += 1;
            continue;
        }
        if ch == '?' {
            question_run += 1;
            continue;
        }

        flush_edge_punctuation_runs(&mut out, &mut dot_run, &mut bang_run, &mut question_run);
        let code = ch as u32;
        if (0..=8).contains(&code) || (11..=12).contains(&code) || (14..=31).contains(&code) {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    flush_edge_punctuation_runs(&mut out, &mut dot_run, &mut bang_run, &mut question_run);
    normalize_edge_terminal_punctuation(&out)
}

fn is_edge_text_usable(text: &str) -> bool {
    text.chars().any(|ch| ch.is_alphanumeric())
}

fn normalize_edge_terminal_punctuation(text: &str) -> String {
    // Edge can fail on some terminal combinations coming from headlines, e.g. `...":`.
    // Keep behavior minimal: only normalize dot + quote + colon sequences to a plain sentence end.
    text.replace(".\":", ". ")
        .replace(".':", ". ")
        .replace(".”:", ". ")
        .replace(".’:", ". ")
}

fn flush_edge_punctuation_runs(
    out: &mut String,
    dot_run: &mut usize,
    bang_run: &mut usize,
    question_run: &mut usize,
) {
    let trim_trailing_spaces = |s: &mut String| {
        while s.chars().last().is_some_and(|ch| ch.is_whitespace()) {
            s.pop();
        }
    };
    if *dot_run >= 3 {
        trim_trailing_spaces(out);
        out.push('.');
    } else if *dot_run > 0 {
        trim_trailing_spaces(out);
        out.extend(std::iter::repeat_n('.', *dot_run));
    }
    if *bang_run >= 3 {
        trim_trailing_spaces(out);
        out.push('!');
    } else if *bang_run > 0 {
        trim_trailing_spaces(out);
        out.extend(std::iter::repeat_n('!', *bang_run));
    }
    if *question_run >= 3 {
        trim_trailing_spaces(out);
        out.push('?');
    } else if *question_run > 0 {
        trim_trailing_spaces(out);
        out.extend(std::iter::repeat_n('?', *question_run));
    }
    *dot_run = 0;
    *bang_run = 0;
    *question_run = 0;
}

fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn adjust_split_for_xml_entity(text: &str, mut split_at: usize) -> usize {
    while split_at > 0 {
        let prefix = &text[..split_at];
        let Some(amp_idx) = prefix.rfind('&') else {
            break;
        };
        if prefix[amp_idx..].contains(';') {
            break;
        }
        split_at = amp_idx;
    }
    split_at
}

fn find_last_newline_or_space_within_limit(text: &str, max_bytes: usize) -> usize {
    let mut last_newline = 0usize;
    let mut last_space = 0usize;
    for (idx, ch) in text.char_indices() {
        let end = idx + ch.len_utf8();
        if end > max_bytes {
            break;
        }
        if ch == '\n' {
            last_newline = end;
        } else if ch.is_whitespace() {
            last_space = end;
        }
    }
    if last_newline > 0 {
        last_newline
    } else {
        last_space
    }
}

fn find_safe_utf8_split_idx(text: &str, max_bytes: usize) -> usize {
    let mut last_end = 0usize;
    for (idx, ch) in text.char_indices() {
        let end = idx + ch.len_utf8();
        if end > max_bytes {
            break;
        }
        last_end = end;
    }
    last_end
}

fn find_edge_split_idx(text: &str, max_bytes: usize) -> usize {
    let total_bytes = text.len();
    if total_bytes <= max_bytes {
        return text.len();
    }

    let mut split_at = find_last_newline_or_space_within_limit(text, max_bytes);
    if split_at == 0 {
        split_at = find_safe_utf8_split_idx(text, max_bytes);
    }
    split_at = adjust_split_for_xml_entity(text, split_at);
    if split_at == 0 {
        split_at = find_safe_utf8_split_idx(text, max_bytes);
    }
    split_at
}

fn split_text_edge(text: &str) -> Vec<String> {
    let cleaned = sanitize_edge_text(text);
    let escaped = escape_xml(&cleaned);
    let sentences = split_sentences(&escaped);
    let mut out = Vec::new();
    let mut current = String::new();

    for sentence in sentences {
        let sentence_trim = sentence.trim();
        if sentence_trim.is_empty() {
            continue;
        }

        let sentence_len = sentence_trim.len();
        if current.is_empty() {
            if sentence_len > EDGE_TTS_MAX_BYTES {
                let parts = split_long_sentence_edge(sentence_trim);
                out.extend(parts);
                continue;
            }
            current.push_str(sentence_trim);
        } else {
            let combined_len = current.len().saturating_add(1).saturating_add(sentence_len);
            if combined_len > EDGE_TTS_MAX_BYTES {
                out.push(current.trim().to_string());
                current.clear();

                if sentence_len > EDGE_TTS_MAX_BYTES {
                    let parts = split_long_sentence_edge(sentence_trim);
                    out.extend(parts);
                } else {
                    current.push_str(sentence_trim);
                }
            } else {
                current.push(' ');
                current.push_str(sentence_trim);
            }
        }
    }

    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }

    out
}

fn split_text_edge_with_limit(text: &str, max_bytes: usize) -> Vec<String> {
    let cleaned = sanitize_edge_text(text);
    let escaped = escape_xml(&cleaned);
    let sentences = split_sentences(&escaped);
    let mut out = Vec::new();
    let mut current = String::new();

    for sentence in sentences {
        let sentence_trim = sentence.trim();
        if sentence_trim.is_empty() {
            continue;
        }

        let sentence_len = sentence_trim.len();
        if current.is_empty() {
            if sentence_len > max_bytes {
                out.extend(split_long_sentence_edge_with_limit(
                    sentence_trim,
                    max_bytes,
                ));
            } else {
                current.push_str(sentence_trim);
            }
        } else {
            let combined_len = current.len().saturating_add(1).saturating_add(sentence_len);
            if combined_len > max_bytes {
                out.push(current.trim().to_string());
                current.clear();
                if sentence_len > max_bytes {
                    out.extend(split_long_sentence_edge_with_limit(
                        sentence_trim,
                        max_bytes,
                    ));
                } else {
                    current.push_str(sentence_trim);
                }
            } else {
                current.push(' ');
                current.push_str(sentence_trim);
            }
        }
    }

    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }

    out
}

fn split_edge_chunk_in_two(text: &str) -> Option<(String, String)> {
    if text.trim().is_empty() || text.len() < 8 {
        return None;
    }
    let mid = text.len() / 2;
    let mut split_at = find_edge_split_idx(text, mid);
    if split_at == 0 || split_at >= text.len() {
        split_at = find_edge_split_idx(text, text.len().saturating_sub(1));
    }
    if split_at == 0 || split_at >= text.len() {
        return None;
    }
    let (a, b) = text.split_at(split_at);
    let a = a.trim().to_string();
    let b = b.trim().to_string();
    if a.is_empty() || b.is_empty() {
        return None;
    }
    Some((a, b))
}

fn is_lt_audio_suspicious(text: &str, audio_len: usize) -> bool {
    let text_len = utf16_len(text);
    if text_len < 120 {
        return false;
    }
    let min_expected = text_len.saturating_mul(120);
    audio_len < min_expected
}

async fn download_edge_chunk_ws_adaptive_lt(
    chunk: &TtsChunk,
    options: &EdgeStreamOptions<'_>,
    idx: usize,
    max_retries: usize,
    depth: usize,
) -> Result<Vec<u8>, String> {
    let audio = download_edge_chunk_ws_with_retry(chunk, options, idx, max_retries).await?;
    if !is_lt_audio_suspicious(&chunk.text_to_read, audio.len()) {
        return Ok(audio);
    }
    if depth >= 3 {
        return Ok(audio);
    }
    let Some((left, right)) = split_edge_chunk_in_two(&chunk.text_to_read) else {
        return Ok(audio);
    };

    let left_chunk = TtsChunk {
        text_to_read: left,
        original_len: chunk.original_len / 2,
        override_voice: chunk.override_voice.clone(),
        pause_ms: None,
    };
    let right_chunk = TtsChunk {
        text_to_read: right,
        original_len: chunk.original_len.saturating_sub(left_chunk.original_len),
        override_voice: chunk.override_voice.clone(),
        pause_ms: None,
    };

    let left_audio = Box::pin(download_edge_chunk_ws_adaptive_lt(
        &left_chunk,
        options,
        idx,
        max_retries,
        depth + 1,
    ))
    .await?;
    let right_audio = Box::pin(download_edge_chunk_ws_adaptive_lt(
        &right_chunk,
        options,
        idx,
        max_retries,
        depth + 1,
    ))
    .await?;

    let mut merged = Vec::with_capacity(left_audio.len().saturating_add(right_audio.len()));
    merged.extend_from_slice(&left_audio);
    merged.extend_from_slice(&right_audio);
    Ok(merged)
}

async fn download_edge_chunk_ws_strict_small_lt(
    chunk: &TtsChunk,
    options: &EdgeStreamOptions<'_>,
    idx: usize,
    max_retries: usize,
) -> Result<Vec<u8>, String> {
    const STRICT_LIMIT: usize = 320;
    let parts = split_text_edge_with_limit(&chunk.text_to_read, STRICT_LIMIT);
    if parts.len() <= 1 {
        return download_edge_chunk_ws_with_retry(chunk, options, idx, max_retries).await;
    }
    let mut out = Vec::new();
    for part in parts {
        let sub = TtsChunk {
            text_to_read: part,
            original_len: chunk.original_len,
            override_voice: chunk.override_voice.clone(),
            pause_ms: None,
        };
        let audio = download_edge_chunk_ws_with_retry(&sub, options, idx, max_retries).await?;
        out.extend_from_slice(&audio);
    }
    Ok(out)
}

fn split_sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let chars: Vec<(usize, char)> = text.char_indices().collect();

    for (index, (idx, ch)) in chars.iter().copied().enumerate() {
        if matches!(ch, '.' | '!' | '?' | ';' | ':') {
            let prev = index
                .checked_sub(1)
                .and_then(|i| chars.get(i))
                .map(|(_, c)| *c);
            let next = chars.get(index + 1).map(|(_, c)| *c);
            let next_is_space = next.map(|c| c.is_whitespace()).unwrap_or(true);
            let is_numeric_separator = matches!(ch, '.' | ':')
                && prev.is_some_and(|c| c.is_ascii_digit())
                && next.is_some_and(|c| c.is_ascii_digit());
            if is_numeric_separator {
                continue;
            }
            if next_is_space {
                let end = idx + ch.len_utf8();
                if end > start {
                    let candidate = &text[start..end];
                    if candidate.chars().any(|c| c.is_alphanumeric()) {
                        out.push(candidate);
                        start = end;
                    }
                }
            }
        }
    }

    if start < text.len() {
        out.push(&text[start..]);
    }

    out
}

fn split_long_sentence_by_whitespace(text: &str, max_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            if word.chars().count() > max_chars {
                out.push(word.to_string());
            } else {
                current.push_str(word);
            }
            continue;
        }

        let combined_len = current
            .chars()
            .count()
            .saturating_add(1)
            .saturating_add(word.chars().count());
        if combined_len > max_chars {
            out.push(current.trim().to_string());
            current.clear();
            current.push_str(word);
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }

    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }

    out
}

fn split_long_sentence_edge(text: &str) -> Vec<String> {
    split_long_sentence_edge_with_limit(text, EDGE_TTS_MAX_BYTES)
}

fn split_long_sentence_edge_with_limit(text: &str, max_bytes: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut remaining = text;
    while remaining.len() > max_bytes {
        let split_at = find_edge_split_idx(remaining, max_bytes);
        if split_at == 0 {
            // Guard against infinite loop on malformed input: consume at least one char.
            if let Some(first_char) = remaining.chars().next() {
                let advance = first_char.len_utf8();
                let chunk = remaining[..advance].trim();
                if !chunk.is_empty() {
                    out.push(chunk.to_string());
                }
                remaining = &remaining[advance..];
                continue;
            }
            break;
        }
        if split_at >= remaining.len() {
            break;
        }
        let (head, tail) = remaining.split_at(split_at);
        let chunk = head.trim();
        if !chunk.is_empty() {
            out.push(chunk.to_string());
        }
        remaining = tail;
    }
    let tail = remaining.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

fn parse_edge_binary_audio_payload(data: &[u8]) -> Result<Option<Vec<u8>>, String> {
    if data.len() < 2 {
        return Err("Edge WS: binary frame missing header length".to_string());
    }

    let be_len = u16::from_be_bytes([data[0], data[1]]) as usize;
    let le_len = u16::from_le_bytes([data[0], data[1]]) as usize;
    let header_len = if be_len > 0 && data.len() >= be_len + 2 {
        be_len
    } else if le_len > 0 && data.len() >= le_len + 2 {
        le_len
    } else {
        return Err("Edge WS: invalid binary header length".to_string());
    };

    let header_bytes = &data[2..2 + header_len];
    let payload = &data[2 + header_len..];
    let header_text = String::from_utf8_lossy(header_bytes);
    let mut path: Option<&str> = None;
    let mut content_type: Option<&str> = None;
    for line in header_text.split("\r\n") {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            if key.eq_ignore_ascii_case("Path") {
                path = Some(val);
            } else if key.eq_ignore_ascii_case("Content-Type") {
                content_type = Some(val);
            }
        }
    }

    if path != Some("audio") {
        return Err("Edge WS: binary frame path is not audio".to_string());
    }

    match content_type {
        Some(ct) if ct.eq_ignore_ascii_case("audio/mpeg") => {
            if payload.is_empty() {
                return Err("Edge WS: audio frame has empty payload".to_string());
            }
            Ok(Some(payload.to_vec()))
        }
        Some(_) => Err("Edge WS: unexpected binary content type".to_string()),
        None => {
            if payload.is_empty() {
                Ok(None)
            } else {
                Err("Edge WS: missing content type with non-empty payload".to_string())
            }
        }
    }
}

pub(crate) fn split_text_for_engine(text: &str, engine: TtsEngine) -> Vec<String> {
    if engine == TtsEngine::Edge {
        split_text_edge(text)
    } else {
        split_text(text)
    }
}

fn split_by_custom_dictionary(
    text: &str,
    _custom_entries: &[DictionaryEntry],
) -> Vec<(String, Option<VoiceOverride>, usize)> {
    if text.is_empty() {
        Vec::new()
    } else {
        vec![(text.to_string(), None, utf16_len(text))]
    }
}

pub fn split_into_tts_chunks(
    text: &str,
    split_on_newline: bool,
    dictionary: &[DictionaryEntry],
    default_engine: TtsEngine,
) -> Vec<TtsChunk> {
    let voice_spans = split_voice_tag_spans(text, default_engine);
    if voice_spans.is_empty() {
        return Vec::new();
    }

    let mut chunks: Vec<TtsChunk> = Vec::new();

    for (span_text, span_override, span_orig_len) in voice_spans {
        let span_engine = span_override
            .as_ref()
            .map(|v| v.engine)
            .unwrap_or(default_engine);
        let span_extra_len = span_orig_len.saturating_sub(utf16_len(&span_text));
        let mut span_pending_len = span_extra_len;
        let pause_segments = split_pause_tag_segments(&span_text);
        for pause_segment in pause_segments {
            let (span_text, span_orig_len) = match pause_segment {
                PauseSplitSegment::Pause(ms, tag_len) => {
                    chunks.push(TtsChunk {
                        text_to_read: String::new(),
                        original_len: tag_len.saturating_add(span_pending_len),
                        override_voice: span_override.clone(),
                        pause_ms: Some(ms),
                    });
                    span_pending_len = 0;
                    continue;
                }
                PauseSplitSegment::Text(text, len) => {
                    let orig_len = len.saturating_add(span_pending_len);
                    span_pending_len = 0;
                    (text, orig_len)
                }
            };
            if span_text.trim().is_empty() {
                if let Some(last) = chunks.last_mut() {
                    last.original_len += span_orig_len;
                }
                continue;
            }
            let mut sentences = Vec::new();
            let mut current_sentence = String::new();
            let mut current_len = 0usize;
            let chars: Vec<char> = span_text.chars().collect();
            for (idx, ch) in chars.iter().copied().enumerate() {
                current_sentence.push(ch);
                current_len += ch.len_utf16();
                let is_terminal = matches!(ch, '.' | '!' | '?' | ';' | ':');
                let next_ch = chars.get(idx + 1).copied();
                let dot_run_continues = ch == '.' && matches!(next_ch, Some('.'));
                if is_terminal && !dot_run_continues {
                    let should_split = current_sentence.chars().any(|c| c.is_alphanumeric());
                    if should_split && !current_sentence.trim().is_empty() {
                        sentences.push((current_sentence.clone(), current_len));
                        current_sentence.clear();
                        current_len = 0;
                    }
                }
            }
            if !current_sentence.trim().is_empty() {
                sentences.push((current_sentence, current_len));
            }

            let extra_len = span_orig_len.saturating_sub(utf16_len(&span_text));
            let mut pending_len = extra_len;

            for (s_text, s_len) in sentences.into_iter() {
                let cleaned = strip_dashed_lines(&s_text);
                let dict_segments = split_by_custom_dictionary(&cleaned, &[]);
                for (dict_text, _override_voice, _dict_len) in dict_segments {
                    let prepared = prepare_tts_text(&dict_text, split_on_newline, dictionary);
                    if prepared.trim().is_empty() {
                        pending_len += s_len;
                        continue;
                    }
                    let orig_len = s_len + pending_len;
                    pending_len = 0;
                    if span_engine == TtsEngine::Edge {
                        let parts = split_text_edge(&prepared);
                        if parts.is_empty() {
                            continue;
                        }
                        let base = orig_len / parts.len();
                        let extra = orig_len % parts.len();
                        for (idx, part) in parts.into_iter().enumerate() {
                            let part_len = base + if idx < extra { 1 } else { 0 };
                            chunks.push(TtsChunk {
                                text_to_read: part,
                                original_len: part_len,
                                override_voice: span_override.clone(),
                                pause_ms: None,
                            });
                        }
                    } else {
                        chunks.push(TtsChunk {
                            text_to_read: prepared,
                            original_len: orig_len,
                            override_voice: span_override.clone(),
                            pause_ms: None,
                        });
                    }
                }
                if pending_len > 0
                    && let Some(last) = chunks.last_mut()
                {
                    last.original_len += pending_len;
                }
            }
        }
    }
    chunks
}

fn split_tts_chunks_by_parts(chunks: &[TtsChunk], parts: usize) -> Vec<Vec<TtsChunk>> {
    if chunks.is_empty() {
        return Vec::new();
    }
    let parts = if parts == 0 { 1 } else { parts };
    let parts = if chunks.len() < parts {
        chunks.len()
    } else {
        parts
    };
    let per_part = chunks.len().div_ceil(parts);
    let mut out = Vec::new();
    for part_idx in 0..parts {
        let start = part_idx * per_part;
        let end = std::cmp::min(start + per_part, chunks.len());
        if start >= end {
            break;
        }
        out.push(chunks[start..end].to_vec());
    }
    out
}

fn start_audiobook_with_text(
    hwnd: HWND,
    mut text: String,
    suggested_name: Option<String>,
    mut epub_chapters: Option<Vec<String>>,
    is_unsaved_doc: bool,
    _doc_path: Option<&Path>,
) {
    let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
    if text.trim().is_empty() {
        show_error(hwnd, language, &settings::tts_no_text_message(language));
        return;
    }

    let (
        voice,
        split_on_newline,
        audiobook_split,
        audiobook_split_by_text,
        audiobook_split_text,
        audiobook_split_text_requires_newline,
        audiobook_split_by_epub_chapter,
        audiobook_split_by_time,
        audiobook_split_minutes,
        audiobook_split_start_number,
        audiobook_part_naming_mode,
        audiobook_part_announcement_mode,
        initial_audiobook_m4b_bitrate,
        tts_engine,
        dictionary,
        tts_rate,
        tts_pitch,
        tts_volume,
    ) = {
        with_state(hwnd, |state| {
            (
                state.settings.tts_voice.clone(),
                state.settings.split_on_newline,
                state.settings.audiobook_split,
                state.settings.audiobook_split_by_text,
                state.settings.audiobook_split_text.clone(),
                state.settings.audiobook_split_text_requires_newline,
                state.settings.audiobook_split_by_epub_chapter,
                state.settings.audiobook_split_by_time,
                state.settings.audiobook_split_minutes,
                state.settings.audiobook_split_start_number,
                state.settings.audiobook_part_naming_mode,
                state.settings.audiobook_part_announcement_mode,
                state.settings.audiobook_m4b_bitrate,
                state.settings.tts_engine,
                state.settings.dictionary.clone(),
                state.settings.tts_rate,
                state.settings.tts_pitch,
                state.settings.tts_volume,
            )
        })
    }
    .unwrap_or((
        "it-IT-IsabellaNeural".to_string(),
        true,
        0,
        false,
        String::new(),
        true,
        false,
        false,
        5,
        1,
        AudiobookPartNamingMode::TitleNumber,
        AudiobookPartAnnouncementMode::None,
        128,
        TtsEngine::Edge,
        Vec::new(),
        0,
        0,
        100,
    ));
    let dialogue_settings =
        { with_state(hwnd, |state| state.settings.clone()) }.unwrap_or_default();
    text = crate::dialogue_voice::apply_dialogue_tags_from_settings(&text, &dialogue_settings);
    if let Some(chapters) = epub_chapters.as_mut() {
        for chapter in chapters {
            *chapter = crate::dialogue_voice::apply_dialogue_tags_from_settings(
                chapter,
                &dialogue_settings,
            );
        }
    }

    let base_split_option_visible = audiobook_split_by_time
        || audiobook_split_by_text
        || audiobook_split > 1
        || (audiobook_split_by_epub_chapter && epub_chapters.as_ref().is_some());

    let cleaned = strip_dashed_lines(&text);
    let use_epub_split = audiobook_split_by_epub_chapter && epub_chapters.as_ref().is_some();
    let mixed_needed = tts_engine == TtsEngine::Google
        || if use_epub_split {
            epub_chapters.as_ref().is_some_and(|chapters| {
                chapters
                    .iter()
                    .any(|chapter| has_voice_tags(chapter) || has_pause_tags(chapter))
            })
        } else {
            has_voice_tags(&cleaned) || has_pause_tags(&cleaned)
        };
    let mut split_by_time = audiobook_split_by_time;
    let split_minutes = audiobook_split_minutes.clamp(1, 60);
    let split_start_number = audiobook_split_start_number.clamp(1, 99);
    let mut split_parts = audiobook_split;
    let mut split_by_text = audiobook_split_by_text;
    if use_epub_split {
        split_by_time = false;
        split_by_text = false;
        split_parts = 0;
    }
    if split_by_time {
        split_parts = 0;
        split_by_text = false;
    }
    let mut marker_parts: Option<Vec<Vec<String>>> = None;
    let mut marker_positions: Option<Vec<usize>> = None;
    let mut marker_text: Option<String> = None;
    let mut selected_marker_titles: Option<Vec<String>> = None;
    let mut mixed_marker_parts: Option<Vec<Vec<TtsChunk>>> = None;
    let mut sapi4_threads: Option<u32> = None;

    if use_epub_split {
        let Some(chapters) = epub_chapters.as_ref() else {
            crate::log_debug("EPUB split requested without chapter data.");
            show_error(hwnd, language, &settings::tts_no_text_message(language));
            return;
        };
        if mixed_needed {
            mixed_marker_parts = build_mixed_audiobook_parts_from_sections(
                chapters,
                split_on_newline,
                &dictionary,
                tts_engine,
            );
        } else {
            marker_parts = build_audiobook_parts_from_sections(
                chapters,
                split_on_newline,
                &dictionary,
                tts_engine,
            );
        }
        if marker_parts.is_none() && mixed_marker_parts.is_none() {
            show_error(hwnd, language, &settings::tts_no_text_message(language));
            return;
        }
    }

    if tts_engine == TtsEngine::Sapi4 {
        let title = i18n::tr(language, "audiobook.sapi4_threads_title");

        let body = i18n::tr(language, "audiobook.sapi4_threads_body");

        if let Some(val_str) =
            crate::app_windows::prompt_window::prompt_user(hwnd, &title, &body, "30", language)
        {
            if let Ok(val) = val_str.parse::<u32>() {
                sapi4_threads = Some(val.clamp(1, SAPI4_MAX_PARALLEL_WORKERS as u32));
            }
        } else {
            // User cancelled the prompt, abort the whole process

            return;
        }
    }

    if split_by_text {
        let (normalized, entries) = collect_marker_entries(
            &cleaned,
            &audiobook_split_text,
            audiobook_split_text_requires_newline,
        );
        if entries.is_empty() {
            split_parts = 0;
        } else {
            let labels: Vec<String> = entries.iter().map(|entry| entry.label.clone()).collect();
            let selected = crate::app_windows::marker_select_window::select_marker_entries(
                hwnd, &labels, language,
            );
            let Some(selected) = selected else {
                return;
            };
            let positions: Vec<usize> = selected
                .iter()
                .filter_map(|idx| entries.get(*idx).map(|e| e.pos))
                .collect();
            selected_marker_titles = Some(
                selected
                    .iter()
                    .filter_map(|idx| entries.get(*idx).map(|e| e.label.clone()))
                    .collect(),
            );
            if positions.is_empty() {
                split_parts = 0;
            } else {
                marker_positions = Some(positions.clone());
                marker_text = Some(normalized.clone());
                marker_parts = build_audiobook_parts_by_positions(
                    &normalized,
                    &positions,
                    split_on_newline,
                    &dictionary,
                    tts_engine,
                );
                if marker_parts.is_none() {
                    split_parts = 0;
                }
            }
        }
    }

    let prepared = if marker_parts.is_some() || mixed_needed {
        String::new()
    } else {
        prepare_tts_text(&cleaned, split_on_newline, &dictionary)
    };

    let chunks = if marker_parts.is_some() || mixed_needed {
        Vec::new()
    } else {
        split_text_for_engine(&prepared, tts_engine)
    };

    let mixed_chunks = if mixed_needed && marker_parts.is_none() && mixed_marker_parts.is_none() {
        Some(split_into_tts_chunks(
            &cleaned,
            split_on_newline,
            &dictionary,
            tts_engine,
        ))
    } else {
        None
    };
    if mixed_needed && mixed_marker_parts.is_none() {
        mixed_marker_parts = match (marker_text.as_ref(), marker_positions.as_ref()) {
            (Some(text), Some(positions)) => build_mixed_audiobook_parts_by_positions(
                text,
                positions,
                split_on_newline,
                &dictionary,
                tts_engine,
            ),
            _ => None,
        };
    }

    let expected_multi_file_split = if split_by_time {
        true
    } else if let Some(parts) = &marker_parts {
        parts.iter().filter(|p| !p.is_empty()).count() > 1
    } else if let Some(parts) = &mixed_marker_parts {
        parts.iter().filter(|p| !p.is_empty()).count() > 1
    } else if split_parts > 1 {
        std::cmp::min(split_parts as usize, chunks.len()) > 1
    } else {
        false
    };
    let split_into_multiple_files = expected_multi_file_split;
    let chapter_titles = if use_epub_split {
        epub_chapters.as_ref().map(|chs| {
            chs.iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
    } else {
        selected_marker_titles.clone()
    };
    crate::log_debug(&format!(
        "Audiobook: split summary split_by_time={} split_parts={} chunks={} marker_parts={} mixed_marker_parts={} expected_multi_file_split={}",
        split_by_time,
        split_parts,
        chunks.len(),
        marker_parts.as_ref().map(|p| p.len()).unwrap_or(0),
        mixed_marker_parts.as_ref().map(|p| p.len()).unwrap_or(0),
        expected_multi_file_split
    ));
    let split_option_visible = if is_unsaved_doc {
        expected_multi_file_split
    } else {
        base_split_option_visible
    };
    let Some(save_result) =
        save_audio_dialog(hwnd, suggested_name.as_deref(), split_option_visible)
    else {
        return;
    };
    let mut output = save_result.path;
    let create_parts_folder = save_result.create_parts_folder;
    let audiobook_m4b_bitrate = {
        with_state(hwnd, |state| state.settings.audiobook_m4b_bitrate)
            .unwrap_or(initial_audiobook_m4b_bitrate)
    };
    crate::log_debug(&format!(
        "Audiobook: settings bitrate resolved to {} kbps for output {:?}",
        audiobook_m4b_bitrate, output
    ));

    if create_parts_folder && split_into_multiple_files {
        let nested_output = split_parts_output_in_subfolder(&output);
        if let Some(parent) = nested_output.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            show_error(
                hwnd,
                language,
                &i18n::tr_f(language, "app.error_save_file", &[("err", &e.to_string())]),
            );
            return;
        }
        output = nested_output;
    }

    let audiobook_title = output
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("audiobook")
        .to_string();

    let chunks_len = if let Some(parts) = &mixed_marker_parts {
        parts.iter().map(|part| part.len()).sum()
    } else if let Some(parts) = &marker_parts {
        parts.iter().map(|part| part.len()).sum()
    } else if let Some(chunks) = &mixed_chunks {
        chunks.len()
    } else {
        chunks.len()
    };
    let progress_total = chunks_len;

    let cancel_token = Arc::new(AtomicBool::new(false));
    let progress_hwnd = {
        let h = crate::app_windows::audiobook_window::open(hwnd, progress_total);
        if with_state(hwnd, |state| {
            state.audiobook_progress = h;
            state.audiobook_cancel = Some(cancel_token.clone());
        })
        .is_none()
        {
            crate::log_debug("Failed to update audiobook progress state");
        }
        h
    };

    let cancel_clone = cancel_token.clone();
    std::thread::spawn(move || {
        lower_current_audiobook_worker_priority("coordinator");
        let final_output = output.clone();
        let extension = final_output
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_aac = extension == "m4b" || extension == "m4a" || extension == "mp4";
        let aac_with_chapters = is_aac && expected_multi_file_split;
        let emit_part_files = !is_aac || aac_with_chapters;
        let chapter_work_output = if aac_with_chapters {
            let stem = final_output
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("audiobook");
            let ext = final_output
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("m4b");
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            final_output.with_file_name(format!("{stem}.chapbuild.{stamp}.{ext}"))
        } else {
            final_output.clone()
        };
        let work_output_for_cleanup = chapter_work_output.clone();

        let options = AudiobookCommonOptions {
            voice: &voice,
            output: &chapter_work_output,
            progress_hwnd,
            cancel: cancel_clone.clone(),
            language,
            part_naming_mode: audiobook_part_naming_mode,
            part_announcement_mode: audiobook_part_announcement_mode,
            audiobook_title: &audiobook_title,
            audiobook_bitrate_kbps: audiobook_m4b_bitrate,
            rate: tts_rate,
            pitch: tts_pitch,
            volume: tts_volume,
            sapi4_threads,
        };
        let part_naming_mode = options.part_naming_mode;
        let engine_name = match tts_engine {
            TtsEngine::Edge => "edge",
            TtsEngine::Sapi5 => "sapi5",
            TtsEngine::Sapi4 => "sapi4",
            TtsEngine::Google => "google",
        };
        crate::log_debug(&format!(
            "Audiobook: export start engine={} bitrate={} output={:?}",
            engine_name, options.audiobook_bitrate_kbps, options.output
        ));

        let mut time_split_done = false;
        let mut result = if mixed_needed {
            let mut current_global_progress = 0usize;
            let mixed_config = MixedAudiobookConfig {
                main_engine: tts_engine,
            };
            (|| {
                if let Some(ref parts) = mixed_marker_parts {
                    let parts_len = parts.len();
                    for (part_idx, part_chunks) in parts.iter().enumerate() {
                        if part_chunks.is_empty() {
                            continue;
                        }
                        let part_output = if parts_len > 1 && emit_part_files {
                            split_part_output(
                                options.output,
                                part_idx,
                                parts_len,
                                options.part_naming_mode,
                            )
                        } else {
                            options.output.to_path_buf()
                        };
                        let announced_chunks = if parts_len > 1 && emit_part_files {
                            prepend_mixed_part_announcement(
                                part_chunks,
                                &options,
                                &part_output,
                                (part_idx + 1) as u32,
                            )
                        } else {
                            part_chunks.to_vec()
                        };
                        render_mixed_audiobook_part(
                            &announced_chunks,
                            &mut current_global_progress,
                            &part_output,
                            &options,
                            &mixed_config,
                        )?;
                    }
                } else if let Some(ref chunks) = mixed_chunks {
                    let parts_count = if emit_part_files {
                        split_parts as usize
                    } else {
                        1
                    };
                    let parts = split_tts_chunks_by_parts(chunks, parts_count);
                    let parts_len = parts.len();
                    for (part_idx, part_chunks) in parts.iter().enumerate() {
                        let part_output = if parts_len > 1 && emit_part_files {
                            split_part_output(
                                options.output,
                                part_idx,
                                parts_len,
                                options.part_naming_mode,
                            )
                        } else {
                            options.output.to_path_buf()
                        };
                        let announced_chunks = if parts_len > 1 && emit_part_files {
                            prepend_mixed_part_announcement(
                                part_chunks,
                                &options,
                                &part_output,
                                (part_idx + 1) as u32,
                            )
                        } else {
                            part_chunks.to_vec()
                        };
                        render_mixed_audiobook_part(
                            &announced_chunks,
                            &mut current_global_progress,
                            &part_output,
                            &options,
                            &mixed_config,
                        )?;
                    }
                }
                Ok(())
            })()
        } else {
            match tts_engine {
                TtsEngine::Edge => {
                    if let Some(ref parts) = marker_parts {
                        if is_aac && !aac_with_chapters {
                            // Merge all marker parts into one for AAC
                            let all_chunks: Vec<String> = parts.iter().flatten().cloned().collect();
                            run_split_audiobook(&all_chunks, 0, options)
                        } else {
                            run_marker_split_audiobook(parts, options)
                        }
                    } else if split_by_time {
                        time_split_done = true;
                        run_split_audiobook_by_time_edge(
                            &chunks,
                            split_minutes,
                            split_start_number,
                            options,
                        )
                    } else {
                        run_split_audiobook(
                            &chunks,
                            if emit_part_files { split_parts } else { 0 },
                            options,
                        )
                    }
                }
                TtsEngine::Sapi4 => {
                    let voice_idx = parse_sapi4_voice_index(&voice);
                    if let Some(ref parts) = marker_parts {
                        if is_aac && !aac_with_chapters {
                            let all_chunks: Vec<String> = parts.iter().flatten().cloned().collect();
                            run_split_sapi4_audiobook(&all_chunks, voice_idx, 0, options)
                        } else {
                            run_marker_split_sapi4_audiobook(parts, voice_idx, options)
                        }
                    } else if split_by_time {
                        time_split_done = true;
                        run_split_audiobook_by_time_sapi(
                            TtsEngine::Sapi4,
                            &chunks,
                            split_minutes,
                            split_start_number,
                            options,
                        )
                    } else {
                        run_split_sapi4_audiobook(
                            &chunks,
                            voice_idx,
                            if emit_part_files { split_parts } else { 0 },
                            options,
                        )
                    }
                }
                TtsEngine::Google => {
                    Err("Google TTS audiobook export must use the generic renderer.".to_string())
                }
                TtsEngine::Sapi5 => {
                    if let Some(ref parts) = marker_parts {
                        if is_aac && !aac_with_chapters {
                            let all_chunks: Vec<String> = parts.iter().flatten().cloned().collect();
                            run_split_sapi_audiobook(&all_chunks, 0, options)
                        } else {
                            run_marker_split_sapi_audiobook(parts, options)
                        }
                    } else if split_by_time {
                        time_split_done = true;
                        run_split_audiobook_by_time_sapi(
                            TtsEngine::Sapi5,
                            &chunks,
                            split_minutes,
                            split_start_number,
                            options,
                        )
                    } else {
                        run_split_sapi_audiobook(
                            &chunks,
                            if emit_part_files { split_parts } else { 0 },
                            options,
                        )
                    }
                }
            }
        };
        if result.is_ok() && split_by_time && !time_split_done {
            let stem = final_output
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("audiobook");
            let ext = final_output
                .extension()
                .and_then(|s| s.to_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("mp3");
            const TIME_SPLIT_PART_WIDTH: usize = 4;
            let pattern = final_output.with_file_name(match part_naming_mode {
                AudiobookPartNamingMode::TitleNumber => {
                    format!("{stem} Part %0{TIME_SPLIT_PART_WIDTH}d.{ext}")
                }
                AudiobookPartNamingMode::NumberOnly => {
                    format!("%0{TIME_SPLIT_PART_WIDTH}d.{ext}")
                }
                AudiobookPartNamingMode::NumberTitle => {
                    format!("%0{TIME_SPLIT_PART_WIDTH}d - {stem}.{ext}")
                }
            });
            let segment_seconds = split_minutes.saturating_mul(60);
            result = crate::ffmpeg_export::segment_audio_file(
                &final_output,
                &pattern,
                segment_seconds,
                split_start_number,
            );
            if result.is_ok()
                && audiobook_part_announcement_mode != AudiobookPartAnnouncementMode::None
            {
                let announcement_options = AudiobookCommonOptions {
                    voice: &voice,
                    output: &final_output,
                    progress_hwnd: HWND(0),
                    cancel: cancel_clone.clone(),
                    language,
                    part_naming_mode: audiobook_part_naming_mode,
                    part_announcement_mode: audiobook_part_announcement_mode,
                    audiobook_title: &audiobook_title,
                    audiobook_bitrate_kbps: audiobook_m4b_bitrate,
                    rate: tts_rate,
                    pitch: tts_pitch,
                    volume: tts_volume,
                    sapi4_threads,
                };
                let mut part_index = 0u32;
                loop {
                    let part_file = time_split_part_output(
                        &final_output,
                        part_index,
                        split_start_number,
                        audiobook_part_naming_mode,
                    );
                    if !part_file.exists() {
                        break;
                    }
                    let part_number = split_start_number.saturating_add(part_index);
                    if let Err(error) = prepend_mixed_announcement_to_audio_file(
                        &part_file,
                        part_number,
                        &announcement_options,
                        tts_engine,
                    ) {
                        result = Err(error);
                        break;
                    }
                    part_index = part_index.saturating_add(1);
                }
            }
            if result.is_ok()
                && let Err(e) = std::fs::remove_file(&final_output)
            {
                crate::log_debug(&format!(
                    "Failed to remove original audiobook after segmenting: {}",
                    e
                ));
            }
        }

        let mut success = result.is_ok();
        if success && aac_with_chapters {
            match collect_split_part_files(&work_output_for_cleanup, part_naming_mode) {
                Ok(part_files) if part_files.len() > 1 => {
                    match merge_m4b_parts_with_chapters(
                        &part_files,
                        &final_output,
                        chapter_titles.as_deref(),
                    ) {
                        Ok(()) => {
                            for file in &part_files {
                                if let Err(e) = std::fs::remove_file(file) {
                                    crate::log_debug(&format!(
                                        "M4B chapter merge cleanup: failed to remove part {}: {}",
                                        file.display(),
                                        e
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            crate::log_debug(&format!("M4B chapter merge failed: {}", e));
                            result = Err(e);
                            success = false;
                        }
                    }
                }
                Ok(_) => {
                    result = Err("M4B chapter merge: not enough part files".to_string());
                    success = false;
                }
                Err(e) => {
                    result = Err(e);
                    success = false;
                }
            }
            if work_output_for_cleanup.exists()
                && let Err(e) = std::fs::remove_file(&work_output_for_cleanup)
            {
                crate::log_debug(&format!(
                    "M4B chapter merge cleanup: failed to remove temp output {}: {}",
                    work_output_for_cleanup.display(),
                    e
                ));
            }
        }

        if success && is_aac && !split_by_time && !aac_with_chapters {
            // Set metadata for the single M4B file
            let file_title = final_output
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Audiobook");
            let mut comment = String::new();

            // If it was supposed to be split, we can provide a "table of contents" in comments
            if split_parts > 1 || marker_parts.is_some() {
                comment.push_str("Chapters:\n");
                // Note: accurate timestamps would require dry-run or parsing generated audio.
                // For now, we provide the labels if using markers.
                if let Some(ref parts) = marker_parts {
                    for (i, _p) in parts.iter().enumerate() {
                        comment.push_str(&format!("Part {}\n", i + 1));
                    }
                }
            }

            if crate::audio_utils::set_file_metadata(
                &final_output,
                Some(file_title),
                Some("Sonarpad"),
                if comment.is_empty() {
                    None
                } else {
                    Some(&comment)
                },
            )
            .is_err()
            {}
        }

        let message = match result {
            Ok(()) => i18n::tr(language, "tts.audiobook_saved"),
            Err(err) => {
                if err == "Cancelled" {
                    cancelled_message(language)
                } else {
                    err
                }
            }
        };
        let payload = Box::new(AudiobookResult { success, message });
        unsafe {
            if let Err(e) = PostMessageW(
                hwnd,
                crate::WM_TTS_AUDIOBOOK_DONE,
                WPARAM(0),
                LPARAM(Box::into_raw(payload) as isize),
            ) {
                crate::log_debug(&format!("Failed to post WM_TTS_AUDIOBOOK_DONE: {}", e));
            }
        }
    });
}

pub fn start_audiobook(hwnd: HWND) {
    let Some(hwnd_edit) = get_active_edit(hwnd) else {
        return;
    };
    let text = get_edit_text(hwnd_edit);
    let (suggested_name, doc_path, split_epub, language, is_unsaved_doc) = {
        with_state(hwnd, |state| {
            state.docs.get(state.current).map(|doc| {
                let p = Path::new(&doc.title);
                (
                    p.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&doc.title)
                        .to_string(),
                    doc.path.clone(),
                    state.settings.audiobook_split_by_epub_chapter,
                    state.settings.language,
                    doc.path.is_none(),
                )
            })
        })
    }
    .flatten()
    .unwrap_or((String::new(), None, false, Language::Italian, true));

    let mut epub_chapters = None;
    if split_epub
        && let Some(ref path) = doc_path
        && is_epub_path(path)
    {
        match read_epub_chapters(path, language) {
            Ok(chapters) => {
                epub_chapters = Some(chapters);
            }
            Err(err) => {
                show_error(hwnd, language, &err);
                return;
            }
        }
    }

    let suggested_name = if suggested_name.is_empty() {
        None
    } else {
        Some(suggested_name)
    };
    start_audiobook_with_text(
        hwnd,
        text,
        suggested_name,
        epub_chapters,
        is_unsaved_doc,
        doc_path.as_deref(),
    );
}

pub fn start_audiobook_from_selection(hwnd: HWND) {
    let Some(hwnd_edit) = get_active_edit(hwnd) else {
        return;
    };
    let text = crate::editor_manager::get_selected_text(hwnd_edit);
    let Some(text) = text else {
        let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
        show_error(hwnd, language, &settings::tts_no_text_message(language));
        return;
    };
    let suggested_name = {
        with_state(hwnd, |state| {
            state.docs.get(state.current).map(|doc| {
                let p = Path::new(&doc.title);
                (
                    p.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&doc.title)
                        .to_string(),
                    doc.path.clone(),
                    doc.path.is_none(),
                )
            })
        })
    }
    .flatten();
    let (suggested_name, doc_path, is_unsaved_doc) = suggested_name
        .map(|(name, path, unsaved)| (Some(name), path, unsaved))
        .unwrap_or((None, None, true));
    start_audiobook_with_text(
        hwnd,
        text,
        suggested_name,
        None,
        is_unsaved_doc,
        doc_path.as_deref(),
    );
}

pub(crate) fn parse_sapi4_voice_index(voice: &str) -> i32 {
    if let Some(hash_pos) = voice.find('#') {
        let rest = &voice[hash_pos + 1..];
        if let Some(pipe_pos) = rest.find('|') {
            rest[..pipe_pos].parse::<i32>().unwrap_or(1)
        } else {
            rest.parse::<i32>().unwrap_or(1)
        }
    } else {
        1
    }
}

fn run_split_audiobook(
    chunks: &[String],
    split_parts: u32,
    options: AudiobookCommonOptions,
) -> Result<(), String> {
    let parts = if split_parts == 0 {
        1
    } else {
        split_parts as usize
    };
    let total_chunks = chunks.len();

    // Se ci sono meno chunks delle parti richieste, riduciamo le parti
    let parts = if total_chunks < parts {
        total_chunks
    } else {
        parts
    };
    let chunks_per_part = total_chunks.div_ceil(parts);

    let mut current_global_progress = 0;

    for part_idx in 0..parts {
        let start_idx = part_idx * chunks_per_part;
        let end_idx = std::cmp::min(start_idx + chunks_per_part, total_chunks);
        if start_idx >= end_idx {
            break;
        }

        let part_chunks = &chunks[start_idx..end_idx];

        let part_output = if parts > 1 {
            split_part_output(options.output, part_idx, parts, options.part_naming_mode)
        } else {
            options.output.to_path_buf()
        };

        // Create a temporary options struct with the correct output path for this part
        let part_options = AudiobookCommonOptions {
            voice: options.voice,
            output: &part_output,
            progress_hwnd: options.progress_hwnd,
            cancel: options.cancel.clone(),
            language: options.language,
            part_naming_mode: options.part_naming_mode,
            part_announcement_mode: options.part_announcement_mode,
            audiobook_title: options.audiobook_title,
            audiobook_bitrate_kbps: options.audiobook_bitrate_kbps,
            rate: options.rate,
            pitch: options.pitch,
            volume: options.volume,
            sapi4_threads: options.sapi4_threads,
        };

        let announced_chunks = if parts > 1 {
            prepend_part_announcement(
                part_chunks,
                &part_options,
                &part_output,
                (part_idx + 1) as u32,
            )
        } else {
            part_chunks.to_vec()
        };
        run_tts_audiobook_part(
            &announced_chunks,
            &mut current_global_progress,
            &part_options,
        )?;
    }
    Ok(())
}

fn run_marker_split_audiobook(
    parts: &[Vec<String>],
    options: AudiobookCommonOptions,
) -> Result<(), String> {
    let parts_len = parts.len();
    let mut current_global_progress = 0;

    for (part_idx, part_chunks) in parts.iter().enumerate() {
        if part_chunks.is_empty() {
            continue;
        }
        let part_output = if parts_len > 1 {
            split_part_output(
                options.output,
                part_idx,
                parts_len,
                options.part_naming_mode,
            )
        } else {
            options.output.to_path_buf()
        };

        let part_options = AudiobookCommonOptions {
            voice: options.voice,
            output: &part_output,
            progress_hwnd: options.progress_hwnd,
            cancel: options.cancel.clone(),
            language: options.language,
            part_naming_mode: options.part_naming_mode,
            part_announcement_mode: options.part_announcement_mode,
            audiobook_title: options.audiobook_title,
            audiobook_bitrate_kbps: options.audiobook_bitrate_kbps,
            rate: options.rate,
            pitch: options.pitch,
            volume: options.volume,
            sapi4_threads: options.sapi4_threads,
        };

        let announced_chunks = if parts_len > 1 {
            prepend_part_announcement(
                part_chunks,
                &part_options,
                &part_output,
                (part_idx + 1) as u32,
            )
        } else {
            part_chunks.to_vec()
        };
        run_tts_audiobook_part(
            &announced_chunks,
            &mut current_global_progress,
            &part_options,
        )?;
    }
    Ok(())
}

fn run_split_sapi4_audiobook(
    chunks: &[String],
    voice_idx: i32,
    split_parts: u32,
    options: AudiobookCommonOptions,
) -> Result<(), String> {
    let parts_count = if split_parts == 0 {
        1
    } else {
        split_parts as usize
    };
    let total_chunks = chunks.len();
    let parts_count = if total_chunks < parts_count {
        total_chunks
    } else {
        parts_count
    };

    let chunks_per_part = total_chunks.div_ceil(parts_count);
    let mut current_global_progress = 0;

    for part_idx in 0..parts_count {
        let start_idx = part_idx * chunks_per_part;
        let end_idx = std::cmp::min(start_idx + chunks_per_part, total_chunks);
        if start_idx >= end_idx {
            break;
        }

        let part_chunks = &chunks[start_idx..end_idx];
        let part_output = if parts_count > 1 {
            split_part_output(
                options.output,
                part_idx,
                parts_count,
                options.part_naming_mode,
            )
        } else {
            options.output.to_path_buf()
        };

        let part_options = AudiobookCommonOptions {
            voice: options.voice,
            output: &part_output,
            progress_hwnd: options.progress_hwnd,
            cancel: options.cancel.clone(),
            language: options.language,
            part_naming_mode: options.part_naming_mode,
            part_announcement_mode: options.part_announcement_mode,
            audiobook_title: options.audiobook_title,
            audiobook_bitrate_kbps: options.audiobook_bitrate_kbps,
            rate: options.rate,
            pitch: options.pitch,
            volume: options.volume,
            sapi4_threads: options.sapi4_threads,
        };

        let announced_chunks = if parts_count > 1 {
            prepend_part_announcement(
                part_chunks,
                &part_options,
                &part_output,
                (part_idx + 1) as u32,
            )
        } else {
            part_chunks.to_vec()
        };
        run_sapi4_parallel_part(
            &announced_chunks,
            voice_idx,
            &mut current_global_progress,
            &part_options,
        )?;
    }
    Ok(())
}

fn run_marker_split_sapi4_audiobook(
    parts: &[Vec<String>],
    voice_idx: i32,
    options: AudiobookCommonOptions,
) -> Result<(), String> {
    let mut current_global_progress = 0;
    for (part_idx, part_chunks) in parts.iter().enumerate() {
        if part_chunks.is_empty() {
            continue;
        }

        let part_output = if parts.len() > 1 {
            split_part_output(
                options.output,
                part_idx,
                parts.len(),
                options.part_naming_mode,
            )
        } else {
            options.output.to_path_buf()
        };

        let part_options = AudiobookCommonOptions {
            voice: options.voice,
            output: &part_output,
            progress_hwnd: options.progress_hwnd,
            cancel: options.cancel.clone(),
            language: options.language,
            part_naming_mode: options.part_naming_mode,
            part_announcement_mode: options.part_announcement_mode,
            audiobook_title: options.audiobook_title,
            audiobook_bitrate_kbps: options.audiobook_bitrate_kbps,
            rate: options.rate,
            pitch: options.pitch,
            volume: options.volume,
            sapi4_threads: options.sapi4_threads,
        };

        let announced_chunks = if parts.len() > 1 {
            prepend_part_announcement(
                part_chunks,
                &part_options,
                &part_output,
                (part_idx + 1) as u32,
            )
        } else {
            part_chunks.to_vec()
        };
        run_sapi4_parallel_part(
            &announced_chunks,
            voice_idx,
            &mut current_global_progress,
            &part_options,
        )?;
    }
    Ok(())
}

pub(crate) fn run_sapi4_parallel_part(
    chunks: &[String],
    voice_idx: i32,
    global_progress: &mut usize,
    options: &AudiobookCommonOptions,
) -> Result<(), String> {
    if chunks.is_empty() {
        return Ok(());
    }

    let extension = options
        .output
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let is_mp3 = extension == "mp3";

    // Parallel processing for chunks within this part
    let temp_dir = options
        .output
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!(
            "sapi4_tmp_{}",
            options
                .output
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("part")
        ));
    std::fs::create_dir_all(&temp_dir).ok();

    struct Sapi4WorkUnit {
        chunks: Vec<String>,
        wav_output: PathBuf,
    }

    enum Sapi4UnitOutcome {
        Completed(PathBuf),
        RetryableError(String),
        FatalError(String),
        Cancelled,
    }

    let requested_pool_size = options.sapi4_threads.unwrap_or(30) as usize;
    let selected_pool_size = requested_sapi4_worker_limit(requested_pool_size);
    let chunks_count = chunks.len();
    let initial_pool_size = selected_pool_size.min(chunks_count);
    crate::log_debug(&format!(
        "Audiobook SAPI4: requested_workers={} selected_workers={} effective_workers={} chunks={} max_allowed={}",
        requested_pool_size,
        selected_pool_size,
        initial_pool_size,
        chunks_count,
        SAPI4_MAX_PARALLEL_WORKERS
    ));

    let sub_parts_count = initial_pool_size;
    let chunks_per_sub = chunks_count.div_ceil(sub_parts_count);
    let mut units = Vec::with_capacity(sub_parts_count);
    for i in 0..sub_parts_count {
        let start = i * chunks_per_sub;
        let end = std::cmp::min(start + chunks_per_sub, chunks_count);
        if start >= end {
            break;
        }
        units.push(Sapi4WorkUnit {
            chunks: chunks[start..end].to_vec(),
            wav_output: temp_dir.join(format!("sub_{}.wav", i)),
        });
    }

    let units = Arc::new(units);
    let progress_start = *global_progress;
    let progress_counter = Arc::new(std::sync::atomic::AtomicUsize::new(progress_start));
    let mut completed_files: Vec<Option<PathBuf>> = vec![None; units.len()];
    let mut pending_indices: Vec<usize> = (0..units.len()).collect();
    let mut worker_limit = initial_pool_size;
    let mut attempt = 1usize;
    let mut final_error: Option<String> = None;

    while !pending_indices.is_empty() {
        if options.cancel.load(Ordering::Relaxed) {
            std::fs::remove_dir_all(&temp_dir).ok();
            return Err(cancelled_message(options.language));
        }

        let actual_workers = worker_limit.min(pending_indices.len());
        crate::log_debug(&format!(
            "Audiobook SAPI4: attempt={} workers={} pending_units={}",
            attempt,
            actual_workers,
            pending_indices.len()
        ));

        for &unit_index in &pending_indices {
            let wav = &units[unit_index].wav_output;
            std::fs::remove_file(wav).ok();
            std::fs::remove_file(wav.with_extension("mp3")).ok();
        }

        let pending_for_attempt = pending_indices.clone();
        let parts_shared = Arc::new(Mutex::new(pending_for_attempt.clone().into_iter()));
        let (tx, rx) = std::sync::mpsc::channel::<(usize, Sapi4UnitOutcome)>();
        let mut handles = Vec::with_capacity(actual_workers);
        let report_progress = attempt == 1;

        for _ in 0..actual_workers {
            let tx = tx.clone();
            let parts_shared = parts_shared.clone();
            let units = units.clone();
            let progress_counter = progress_counter.clone();
            let cancel_token = options.cancel.clone();
            let (rate, pitch, volume) = (options.rate, options.pitch, options.volume);
            let mp3_bitrate_kbps = options.audiobook_bitrate_kbps;
            let progress_hwnd = options.progress_hwnd;

            let handle = std::thread::spawn(move || {
                lower_current_audiobook_worker_priority("SAPI4 worker");
                loop {
                    let unit_index = {
                        let mut guard = parts_shared.lock().unwrap_or_else(|e| e.into_inner());
                        guard.next()
                    };
                    let Some(unit_index) = unit_index else {
                        break;
                    };
                    let unit = &units[unit_index];

                    if cancel_token.load(Ordering::Relaxed) {
                        tx.send((unit_index, Sapi4UnitOutcome::Cancelled)).ok();
                        break;
                    }

                    let synthesis = crate::sapi4_engine::speak_sapi4_to_file(
                        &unit.chunks,
                        voice_idx,
                        &unit.wav_output,
                        crate::sapi4_engine::Sapi4Options {
                            rate,
                            pitch,
                            volume,
                            mp3_bitrate_kbps,
                            cancel: cancel_token.clone(),
                        },
                        |_| {
                            if report_progress {
                                let current = progress_counter.fetch_add(1, Ordering::SeqCst) + 1;
                                if progress_hwnd.0 != 0 {
                                    unsafe {
                                        let _post_result = PostMessageW(
                                            progress_hwnd,
                                            crate::WM_UPDATE_PROGRESS,
                                            WPARAM(current),
                                            LPARAM(0),
                                        );
                                    }
                                }
                            }
                        },
                    );

                    let result = match synthesis {
                        Err(error) => {
                            std::fs::remove_file(&unit.wav_output).ok();
                            std::fs::remove_file(unit.wav_output.with_extension("mp3")).ok();
                            Sapi4UnitOutcome::RetryableError(error)
                        }
                        Ok(()) => {
                            let wav_valid = std::fs::metadata(&unit.wav_output)
                                .is_ok_and(|metadata| metadata.len() > 44);
                            if !wav_valid {
                                std::fs::remove_file(&unit.wav_output).ok();
                                Sapi4UnitOutcome::RetryableError(format!(
                                    "SAPI4 bridge produced an empty or invalid WAV for unit {}",
                                    unit_index
                                ))
                            } else if is_mp3 {
                                let encoded_sub = unit.wav_output.with_extension("mp3");
                                let ff_settings = crate::ffmpeg_export::ConvertAudioSettings {
                                    format: crate::ffmpeg_export::ConvertAudioFormat::Mp3,
                                    quality: crate::ffmpeg_export::ConvertAudioQuality::BitrateKbps(
                                        mp3_bitrate_kbps,
                                    ),
                                };
                                let mut ff_progress = |_progress: u32| {};
                                match crate::ffmpeg_export::convert_audio_file(
                                    &unit.wav_output,
                                    &encoded_sub,
                                    &ff_settings,
                                    None,
                                    Some(&mut ff_progress),
                                ) {
                                    Ok(()) => {
                                        std::fs::remove_file(&unit.wav_output).ok();
                                        let mp3_valid = std::fs::metadata(&encoded_sub)
                                            .is_ok_and(|metadata| metadata.len() > 0);
                                        if mp3_valid {
                                            Sapi4UnitOutcome::Completed(encoded_sub)
                                        } else {
                                            std::fs::remove_file(&encoded_sub).ok();
                                            Sapi4UnitOutcome::FatalError(format!(
                                                "Parallel SAPI4 MP3 encode produced an empty file for unit {}",
                                                unit_index
                                            ))
                                        }
                                    }
                                    Err(error) => {
                                        std::fs::remove_file(&unit.wav_output).ok();
                                        std::fs::remove_file(&encoded_sub).ok();
                                        Sapi4UnitOutcome::FatalError(format!(
                                            "Parallel audio encode failed: {}",
                                            error
                                        ))
                                    }
                                }
                            } else {
                                Sapi4UnitOutcome::Completed(unit.wav_output.clone())
                            }
                        }
                    };

                    tx.send((unit_index, result)).ok();
                }
            });
            handles.push(handle);
        }
        {
            let _sender = tx;
        }

        let mut received = vec![false; units.len()];
        let mut failed_indices = Vec::new();
        let mut attempt_error: Option<String> = None;
        let mut fatal_error: Option<String> = None;
        let mut was_cancelled = false;
        for (unit_index, outcome) in rx {
            if unit_index >= units.len() {
                continue;
            }
            received[unit_index] = true;
            match outcome {
                Sapi4UnitOutcome::Completed(path) => {
                    completed_files[unit_index] = Some(path);
                }
                Sapi4UnitOutcome::RetryableError(error) => {
                    crate::log_debug(&format!(
                        "Audiobook SAPI4: attempt={} unit={} failed: {}",
                        attempt, unit_index, error
                    ));
                    if attempt_error.is_none() {
                        attempt_error = Some(error);
                    }
                    failed_indices.push(unit_index);
                }
                Sapi4UnitOutcome::FatalError(error) => {
                    crate::log_debug(&format!(
                        "Audiobook SAPI4: attempt={} unit={} fatal error: {}",
                        attempt, unit_index, error
                    ));
                    if fatal_error.is_none() {
                        fatal_error = Some(error);
                    }
                    failed_indices.push(unit_index);
                }
                Sapi4UnitOutcome::Cancelled => {
                    was_cancelled = true;
                    failed_indices.push(unit_index);
                }
            }
        }

        for handle in handles {
            if handle.join().is_err() {
                crate::log_debug(&format!(
                    "Audiobook SAPI4: a worker thread panicked during attempt {}",
                    attempt
                ));
            }
        }

        for &unit_index in &pending_for_attempt {
            if !received[unit_index] {
                failed_indices.push(unit_index);
                if attempt_error.is_none() {
                    attempt_error = Some(format!(
                        "SAPI4 worker ended without returning a result for unit {}",
                        unit_index
                    ));
                }
            }
        }
        failed_indices.sort_unstable();
        failed_indices.dedup();

        if options.cancel.load(Ordering::Relaxed) || was_cancelled {
            std::fs::remove_dir_all(&temp_dir).ok();
            return Err(cancelled_message(options.language));
        }

        if let Some(error) = fatal_error {
            std::fs::remove_dir_all(&temp_dir).ok();
            return Err(error);
        }

        if failed_indices.is_empty() {
            crate::log_debug(&format!(
                "Audiobook SAPI4: attempt={} completed successfully with {} workers",
                attempt, actual_workers
            ));
            break;
        }

        final_error = attempt_error;
        if worker_limit <= 1 {
            std::fs::remove_dir_all(&temp_dir).ok();
            return Err(final_error
                .unwrap_or_else(|| "SAPI4 audiobook creation failed with one worker".to_string()));
        }

        let lower_limit = sapi4_next_lower_worker_limit(worker_limit).min(failed_indices.len());
        crate::log_debug(&format!(
            "Audiobook SAPI4: retrying {} failed units; workers {} -> {}",
            failed_indices.len(),
            actual_workers,
            lower_limit
        ));
        pending_indices = failed_indices;
        worker_limit = lower_limit;
        attempt += 1;
    }

    let mut produced_files: Vec<PathBuf> = completed_files.into_iter().flatten().collect();
    if produced_files.len() != units.len() {
        std::fs::remove_dir_all(&temp_dir).ok();
        return Err(final_error.unwrap_or_else(|| {
            format!(
                "SAPI4 produced {} parts out of {}",
                produced_files.len(),
                units.len()
            )
        }));
    }

    produced_files.sort_by_key(|p: &PathBuf| {
        let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        parse_sapi4_part_index(name)
    });

    let result = if is_mp3 {
        merge_and_finalize_sapi4_mp3(
            &produced_files,
            options.output,
            options.language,
            options.audiobook_bitrate_kbps,
            options.progress_hwnd,
        )
    } else {
        // This covers M4B (AAC) and standard WAV.
        // For M4B, it joins WAV chunks and then encodes to AAC in one pass.
        merge_and_finalize_sapi4_audio(&produced_files, options.output, options.language, options)
    };

    std::fs::remove_dir_all(&temp_dir).ok();
    let exact_progress = progress_start.saturating_add(chunks.len());
    progress_counter.store(exact_progress, Ordering::SeqCst);
    if options.progress_hwnd.0 != 0 {
        unsafe {
            let _post_result = PostMessageW(
                options.progress_hwnd,
                crate::WM_UPDATE_PROGRESS,
                WPARAM(exact_progress),
                LPARAM(0),
            );
        }
    }
    *global_progress = exact_progress;
    result
}

fn parse_sapi4_part_index(name: &str) -> usize {
    if let Some(rest) = name.strip_prefix("sub_")
        && let Ok(value) = rest.parse::<usize>()
    {
        return value;
    }
    if let Some(rest) = name.strip_prefix("part_")
        && let Ok(value) = rest.parse::<usize>()
    {
        return value;
    }
    let mut digits = Vec::new();
    for ch in name.chars().rev() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if !digits.is_empty() {
            break;
        }
    }
    if digits.is_empty() {
        return 0;
    }
    digits.reverse();
    digits
        .into_iter()
        .collect::<String>()
        .parse::<usize>()
        .unwrap_or(0)
}

fn merge_and_finalize_sapi4_mp3(
    mp3_files: &[PathBuf],
    output: &Path,
    language: Language,
    bitrate_kbps: u32,
    progress_hwnd: HWND,
) -> Result<(), String> {
    if mp3_files.is_empty() {
        return Ok(());
    }

    let output_name = output
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("audiobook.mp3");
    let concat_output = output.with_file_name(format!("{output_name}.concat.tmp.mp3"));

    let mut out_file = std::fs::File::create(&concat_output).map_err(|e| e.to_string())?;
    for path in mp3_files {
        let mut in_file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        std::io::copy(&mut in_file, &mut out_file).map_err(|e| e.to_string())?;
    }

    set_audiobook_progress_phase(progress_hwnd, true);
    set_audiobook_progress_total(progress_hwnd, 100);
    post_audiobook_progress(progress_hwnd, 0);
    let settings = crate::ffmpeg_export::ConvertAudioSettings {
        format: crate::ffmpeg_export::ConvertAudioFormat::Mp3,
        quality: crate::ffmpeg_export::ConvertAudioQuality::BitrateKbps(bitrate_kbps),
    };
    let mut last_progress = 0usize;
    let mut progress = |p: u32| {
        let current = ((p as usize) / 100).min(99);
        if current > last_progress {
            last_progress = current;
            post_audiobook_progress(progress_hwnd, current);
        }
    };
    let convert_result = crate::ffmpeg_export::convert_audio_file(
        &concat_output,
        output,
        &settings,
        None,
        Some(&mut progress),
    );
    if let Err(err) = std::fs::remove_file(&concat_output) {
        crate::log_debug(&format!(
            "Failed to remove temporary concatenated MP3 {:?}: {}",
            concat_output, err
        ));
    }
    match convert_result {
        Ok(()) => {
            post_audiobook_progress(progress_hwnd, 100);
            Ok(())
        }
        Err(err) => Err(i18n::tr_f(language, "sapi5.mf_error", &[("err", &err)])),
    }
}

#[cfg(test)]
mod tests {
    use crate::settings::DictionaryEntry;

    use super::{
        GOOGLE_PLAYBACK_FIRST_CHUNK_MAX_CHARS, TtsChunk, TtsEngine,
        build_audiobook_parts_by_positions, collect_marker_entries, find_edge_split_idx,
        is_edge_text_usable, normalize_for_tts, optimize_google_playback_startup,
        parse_edge_binary_audio_payload, parse_sapi4_part_index, parse_voice_tag_override,
        prepare_tts_text, preview_for_log, render_edge_ssml_text_with_pause_tags,
        render_sapi_ssml_text_with_pause_tags, sanitize_edge_text,
        sapi5_minimum_plausible_duration_ms, sapi5_mp3_duration_from_size_ms, sapi5_worker_levels,
        split_into_tts_chunks, split_long_sentence_edge_with_limit, split_sentences,
        split_text_for_engine, split_voice_tag_spans, strip_dashed_lines, utf16_len,
        wav_interleaved_sample_count,
    };

    #[test]
    fn wav_sample_count_uses_pcm_frames_and_channels() {
        let channels = 2u16;
        let bits_per_sample = 16u16;
        let frames = 4u32;
        let block_align = channels * (bits_per_sample / 8);
        let data_size = frames * u32::from(block_align);
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36u32 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&44_100u32.to_le_bytes());
        wav.extend_from_slice(&(44_100u32 * u32::from(block_align)).to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        wav.resize(wav.len() + data_size as usize, 0);

        assert_eq!(
            wav_interleaved_sample_count(&wav),
            Some(u64::from(frames) * u64::from(channels))
        );
    }

    #[test]
    fn wav_sample_count_rejects_non_wave_data() {
        assert_eq!(wav_interleaved_sample_count(b"not a wav"), None);
    }

    #[test]
    fn google_playback_uses_a_short_natural_first_chunk() {
        let text = format!(
            "{}{}, poi il testo continua ancora per simulare una prima frase molto lunga",
            "Titolo introduttivo ".repeat(2),
            "naturale"
        );
        let original_len = utf16_len(&text);
        let mut chunks = vec![TtsChunk {
            text_to_read: text.clone(),
            original_len,
            override_voice: None,
            pause_ms: None,
        }];

        optimize_google_playback_startup(&mut chunks, TtsEngine::Google);

        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].text_to_read.chars().count() <= GOOGLE_PLAYBACK_FIRST_CHUNK_MAX_CHARS);
        assert!(chunks[0].text_to_read.ends_with(','));
        assert_eq!(
            chunks.iter().map(|chunk| chunk.original_len).sum::<usize>(),
            original_len
        );
        assert_eq!(
            chunks
                .iter()
                .flat_map(|chunk| chunk.text_to_read.split_whitespace())
                .collect::<Vec<_>>(),
            text.split_whitespace().collect::<Vec<_>>()
        );
    }

    #[test]
    fn google_playback_hard_fallback_keeps_first_chunk_bounded() {
        let text = "a".repeat(GOOGLE_PLAYBACK_FIRST_CHUNK_MAX_CHARS + 75);
        let mut chunks = vec![TtsChunk {
            original_len: utf16_len(&text),
            text_to_read: text.clone(),
            override_voice: None,
            pause_ms: None,
        }];

        optimize_google_playback_startup(&mut chunks, TtsEngine::Google);

        assert_eq!(chunks.len(), 2);
        assert_eq!(
            chunks[0].text_to_read.chars().count(),
            GOOGLE_PLAYBACK_FIRST_CHUNK_MAX_CHARS
        );
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text_to_read.as_str())
                .collect::<String>(),
            text
        );
    }

    #[test]
    fn startup_optimization_does_not_change_non_google_playback() {
        let text = "Una frase iniziale molto lunga senza una conclusione ".repeat(5);
        let mut chunks = vec![TtsChunk {
            original_len: utf16_len(&text),
            text_to_read: text,
            override_voice: None,
            pause_ms: None,
        }];

        optimize_google_playback_startup(&mut chunks, TtsEngine::Edge);

        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn prepare_tts_text_keeps_dictionary_entries_case_sensitive_by_default() {
        let dictionary = vec![DictionaryEntry {
            original: "ciao".to_string(),
            replacement: "salve".to_string(),
            match_case: true,
            use_custom_voice: false,
            custom_voice_engine: None,
            custom_voice: None,
        }];

        assert_eq!(
            prepare_tts_text("Ciao ciao", false, &dictionary),
            "Ciao salve"
        );
    }

    #[test]
    fn prepare_tts_text_supports_case_insensitive_dictionary_entries() {
        let dictionary = vec![DictionaryEntry {
            original: "ciao".to_string(),
            replacement: "salve".to_string(),
            match_case: false,
            use_custom_voice: false,
            custom_voice_engine: None,
            custom_voice: None,
        }];

        assert_eq!(
            prepare_tts_text("Ciao cIAO ciao", false, &dictionary),
            "salve salve salve"
        );
    }

    #[test]
    fn prepare_tts_text_collapses_ellipsis_for_all_engines() {
        assert_eq!(prepare_tts_text("Ciao amico...", false, &[]), "Ciao amico.");
        assert_eq!(
            prepare_tts_text("Ciao amico....", false, &[]),
            "Ciao amico."
        );
        assert_eq!(prepare_tts_text("Ciao..", false, &[]), "Ciao..");
    }

    #[test]
    fn sapi5_ellipsis_does_not_create_punctuation_only_chunks() {
        let chunks =
            split_into_tts_chunks("Ciao amico... Come stai?", false, &[], TtsEngine::Sapi5);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text_to_read, "Ciao amico.");
        assert_eq!(chunks[1].text_to_read, " Come stai?");
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.text_to_read.chars().any(char::is_alphanumeric))
        );
    }
    #[test]
    fn pause_tags_render_as_edge_breaks_from_raw_or_escaped_text() {
        assert_eq!(
            render_edge_ssml_text_with_pause_tags("Ciao <pause ms=\"500\"/> dopo"),
            "Ciao <break time=\"500ms\"/> dopo"
        );
        assert_eq!(
            render_edge_ssml_text_with_pause_tags("Ciao &lt;pause ms=&quot;1000&quot;/&gt; dopo"),
            "Ciao <break time=\"1000ms\"/> dopo"
        );
    }

    #[test]
    fn pause_tags_render_as_sapi_silence() {
        assert_eq!(
            render_sapi_ssml_text_with_pause_tags("Ciao <pause ms=\"500\"/> dopo"),
            "Ciao <silence msec=\"500\"/> dopo"
        );
    }

    #[test]
    fn pause_tags_become_silence_chunks_for_all_engines() {
        let chunks =
            split_into_tts_chunks("Ciao <pause ms=\"500\"/> dopo", true, &[], TtsEngine::Edge);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text_to_read, "Ciao");
        assert_eq!(chunks[1].pause_ms, Some(500));
        assert_eq!(chunks[2].text_to_read, "dopo");
        let chunks =
            split_into_tts_chunks("Ciao <pause ms=\"500\"/> dopo", true, &[], TtsEngine::Sapi5);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[1].pause_ms, Some(500));
        let chunks =
            split_into_tts_chunks("Ciao <pause ms=\"500\"/> dopo", true, &[], TtsEngine::Sapi4);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[1].pause_ms, Some(500));
    }

    #[test]
    fn chunk_original_lengths_track_utf16_editor_offsets() {
        let text = "Prima riga.\r\nSeconda riga 😀.\r\nFine.";
        let chunks = split_into_tts_chunks(text, true, &[], TtsEngine::Sapi5);
        let total_original_len: usize = chunks.iter().map(|chunk| chunk.original_len).sum();
        assert_eq!(total_original_len, utf16_len(text));
    }

    #[test]
    fn chunk_original_lengths_preserve_offsets_when_newlines_are_normalized() {
        let text = "Prima riga.\r\nSeconda riga.\r\nFine.";
        let chunks = split_into_tts_chunks(text, false, &[], TtsEngine::Sapi5);
        let total_original_len: usize = chunks.iter().map(|chunk| chunk.original_len).sum();
        assert_eq!(total_original_len, utf16_len(text));
    }

    #[test]
    fn parse_sapi4_part_index_prefers_named_prefix() {
        assert_eq!(parse_sapi4_part_index("sub_0"), 0);
        assert_eq!(parse_sapi4_part_index("sub_12"), 12);
        assert_eq!(parse_sapi4_part_index("part_3"), 3);
        assert_eq!(parse_sapi4_part_index("part_20"), 20);
    }

    #[test]
    fn parse_sapi4_part_index_falls_back_to_trailing_digits() {
        assert_eq!(parse_sapi4_part_index("segment_0007"), 7);
        assert_eq!(parse_sapi4_part_index("chunk99"), 99);
        assert_eq!(parse_sapi4_part_index("audio"), 0);
        assert_eq!(parse_sapi4_part_index("sub_foo_5"), 5);
    }

    #[test]
    fn sapi4_part_sort_orders_by_index() {
        let mut paths = vec![
            std::path::PathBuf::from("sub_2.mp3"),
            std::path::PathBuf::from("sub_10.mp3"),
            std::path::PathBuf::from("sub_1.mp3"),
            std::path::PathBuf::from("part_3.mp3"),
            std::path::PathBuf::from("foo9.mp3"),
        ];
        paths.sort_by_key(|path| {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            parse_sapi4_part_index(stem)
        });
        let names: Vec<String> = paths
            .iter()
            .map(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        assert_eq!(
            names,
            vec![
                "sub_1.mp3",
                "sub_2.mp3",
                "part_3.mp3",
                "foo9.mp3",
                "sub_10.mp3"
            ]
        );
    }

    #[test]
    fn sapi4_part_sort_handles_dirs_and_extensions() {
        let mut paths = vec![
            std::path::PathBuf::from("C:\\tmp\\sapi4\\sub_2.wav"),
            std::path::PathBuf::from("C:\\tmp\\sapi4\\part_12.mp3"),
            std::path::PathBuf::from("C:\\tmp\\sapi4\\foo9.m4a"),
            std::path::PathBuf::from("C:\\tmp\\sapi4\\sub_1.mp3"),
            std::path::PathBuf::from("C:\\tmp\\sapi4\\chunk_0003.wav"),
        ];
        paths.sort_by_key(|path| {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            parse_sapi4_part_index(stem)
        });
        let names: Vec<String> = paths
            .iter()
            .map(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        assert_eq!(
            names,
            vec![
                "sub_1.mp3",
                "sub_2.wav",
                "chunk_0003.wav",
                "foo9.m4a",
                "part_12.mp3"
            ]
        );
    }

    #[test]
    fn split_voice_tag_spans_decodes_basic_xml_entities_in_text() {
        let input = r#"<voice engine="edge" voice="it-IT-DiegoNeural">&quot;Ciao &amp; mondo&quot;</voice>"#;
        let spans = split_voice_tag_spans(input, TtsEngine::Edge);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, "\"Ciao & mondo\"");
        assert!(spans[0].1.is_some());
    }

    #[test]
    fn synthetic_dialogue_voice_tags_do_not_advance_editor_cursor_offsets() {
        let input = r#"<voice engine="edge" voice="it-IT-DiegoNeural" sonarpad-dialogue="1">"Ciao"</voice> dopo"#;
        let spans = split_voice_tag_spans(input, TtsEngine::Edge);
        let total_original_len: usize = spans.iter().map(|span| span.2).sum();
        assert_eq!(total_original_len, utf16_len("\"Ciao\" dopo"));
    }

    #[test]
    fn explicit_voice_tags_still_advance_editor_cursor_offsets() {
        let input = r#"<voice engine="edge" voice="it-IT-DiegoNeural">"Ciao"</voice> dopo"#;
        let spans = split_voice_tag_spans(input, TtsEngine::Edge);
        let total_original_len: usize = spans.iter().map(|span| span.2).sum();
        assert_eq!(total_original_len, utf16_len(input));
    }

    #[test]
    fn parse_voice_tag_override_accepts_rate_pitch_volume() {
        let tag = r#"engine="edge" voice="it-IT-DiegoNeural" rate="-80" pitch="8" volume="140""#;
        let ov = parse_voice_tag_override(tag, TtsEngine::Edge).expect("override expected");
        assert_eq!(ov.rate, Some(-80));
        assert_eq!(ov.pitch, Some(8));
        assert_eq!(ov.volume, Some(140));
    }

    #[test]
    fn edge_normalizes_ascii_ellipsis_to_single_dot() {
        assert_eq!(sanitize_edge_text("Ciao... come va..."), "Ciao. come va.");
    }

    #[test]
    fn edge_sanitize_handles_weird_spaces_and_symbols() {
        assert_eq!(
            sanitize_edge_text("ciao\u{00A0}\u{200B}mondo\u{2409}!!!???"),
            "ciao mondo!?"
        );
    }

    #[test]
    fn edge_sanitize_normalizes_dot_quote_colon_sequences() {
        let text = "\"Addio Juve, secondo me andrà...\": parla chi gli ha cambiato la vita";
        let sanitized = sanitize_edge_text(text);
        assert!(!sanitized.contains(".\":"));
        assert!(sanitized.contains("andrà."));
    }

    #[test]
    fn edge_split_does_not_emit_quote_colon_only_chunk_after_ellipsis() {
        let chunks = split_into_tts_chunks(
            "\"Vlahovic distrugge reti e lascia! Addio Juve, secondo me andrà...\": parla chi gli ha cambiato la vita",
            true,
            &[],
            TtsEngine::Edge,
        );
        assert!(!chunks.is_empty());
        let has_bad_chunk = chunks.iter().any(|c| {
            let probe = c
                .text_to_read
                .replace("&quot;", "\"")
                .replace("&apos;", "'")
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>();
            !probe.chars().any(|ch| ch.is_alphanumeric())
        });
        assert!(
            !has_bad_chunk,
            "Edge chunk split produced punctuation-only chunk"
        );
    }

    #[test]
    fn edge_text_usable_rejects_punctuation_only() {
        assert!(!is_edge_text_usable("...?!"));
        assert!(is_edge_text_usable("ciao."));
    }

    #[test]
    fn edge_split_into_chunks_does_not_create_dot_only_chunks_from_ellipsis() {
        let chunks = split_into_tts_chunks(
            "ciao io sono ambro... e sono un campione... e sono un grande atleta...",
            true,
            &[],
            TtsEngine::Edge,
        );
        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|c| c.text_to_read.trim() != "."));
    }

    #[test]
    fn split_sentences_keeps_number_groups_intact() {
        let text = "Sono chiamati alle urne 51.424.729 cittadini, tra cui 5.477.619 residenti all’estero. Si vota alle 12:30.";
        assert_eq!(
            split_sentences(text),
            vec![
                "Sono chiamati alle urne 51.424.729 cittadini, tra cui 5.477.619 residenti all’estero.",
                " Si vota alle 12:30."
            ]
        );
    }

    #[test]
    fn edge_split_idx_prefers_newline_or_space_and_stays_utf8_safe() {
        let text = "uno due\ntre quattro cinque";
        let split = find_edge_split_idx(text, 10);
        assert!(split > 0 && split <= 10);
        assert!(text.is_char_boundary(split));
    }

    #[test]
    fn edge_split_long_sentence_never_loops_and_preserves_order() {
        let text = "áááááááááááááááááááá";
        let parts = split_long_sentence_edge_with_limit(text, 3);
        assert!(!parts.is_empty());
        let joined: String = parts.join("");
        assert_eq!(joined, text);
    }

    #[test]
    fn edge_binary_audio_payload_parses_valid_audio_frame() {
        let header = b"Path:audio\r\nContent-Type:audio/mpeg";
        let payload = b"\x01\x02\x03";
        let mut frame = Vec::new();
        frame.extend_from_slice(&(header.len() as u16).to_be_bytes());
        frame.extend_from_slice(header);
        frame.extend_from_slice(payload);
        let out = parse_edge_binary_audio_payload(&frame).expect("valid frame should parse");
        assert_eq!(out, Some(payload.to_vec()));
    }

    #[test]
    fn edge_binary_audio_payload_rejects_non_audio_path() {
        let header = b"Path:turn.end\r\nContent-Type:audio/mpeg";
        let payload = b"\x01";
        let mut frame = Vec::new();
        frame.extend_from_slice(&(header.len() as u16).to_be_bytes());
        frame.extend_from_slice(header);
        frame.extend_from_slice(payload);
        let err = parse_edge_binary_audio_payload(&frame).expect_err("non-audio path must fail");
        assert!(err.contains("path is not audio"));
    }

    #[test]
    fn marker_split_with_chapter_and_m4b_merge_does_not_fail() {
        let text = "Chapter 1\nHello world.\n\nChapter 2\nMore text.\n";
        let (normalized, entries) = collect_marker_entries(text, "Chapter", true);
        assert_eq!(entries.len(), 2);
        let positions: Vec<usize> = entries.iter().map(|e| e.pos).collect();
        let parts =
            build_audiobook_parts_by_positions(&normalized, &positions, true, &[], TtsEngine::Edge)
                .expect("marker split should produce parts");
        let all_chunks: Vec<String> = parts.iter().flatten().cloned().collect();
        assert!(!all_chunks.is_empty());
    }

    #[test]
    fn sapi5_adaptive_worker_levels_reduce_progressively() {
        assert_eq!(sapi5_worker_levels(12), vec![12, 8, 6, 4, 2, 1]);
        assert_eq!(sapi5_worker_levels(6), vec![6, 4, 2, 1]);
        assert_eq!(sapi5_worker_levels(1), vec![1]);
    }

    #[test]
    fn sapi5_size_based_duration_matches_cbr_bitrate() {
        assert_eq!(sapi5_mp3_duration_from_size_ms(8_000_000, 64), 1_000_000);
        assert_eq!(sapi5_mp3_duration_from_size_ms(8_000_000, 128), 500_000);
    }

    #[test]
    fn sapi5_plausibility_guard_flags_large_normal_rate_truncation() {
        let minimum = sapi5_minimum_plausible_duration_ms(30_000, 0);
        assert!(minimum > 420_000);
        assert!(minimum < 900_000);
    }

    #[test]
    fn sapi4_audiobook_chunks_merge_in_order() {
        let mut produced_files = vec![
            std::path::PathBuf::from("sub_10.wav"),
            std::path::PathBuf::from("sub_2.wav"),
            std::path::PathBuf::from("sub_1.wav"),
            std::path::PathBuf::from("sub_3.wav"),
        ];
        produced_files.sort_by_key(|p| {
            let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            parse_sapi4_part_index(name)
        });
        let names: Vec<String> = produced_files
            .iter()
            .map(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        assert_eq!(
            names,
            vec!["sub_1.wav", "sub_2.wav", "sub_3.wav", "sub_10.wav"]
        );
    }

    #[test]
    fn audiobook_liturgia_no_skipped_or_reordered_lines() {
        let path_owned = std::env::var("SONARPAD_TTS_TEST_PATH")
            .unwrap_or_else(|_| r"C:\Users\ambro\Downloads\05-02-2026.txt".to_string());
        let path = path_owned.as_str();
        let raw = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Skipping test – cannot read {path}: {e}");
                return;
            }
        };

        let cleaned = strip_dashed_lines(&raw);
        let chunks = split_into_tts_chunks(&cleaned, true, &[], TtsEngine::Edge);

        assert!(
            !chunks.is_empty(),
            "split_into_tts_chunks produced 0 chunks"
        );

        // Reassemble all chunk text into a single string for searching
        let reassembled: String = chunks
            .iter()
            .map(|c| c.text_to_read.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        // Normalize the cleaned text the same way: split_on_newline=true, no dictionary
        let normalized_input = normalize_for_tts(&cleaned, true);

        // Collect non-empty input lines with their 1-based line numbers
        let input_lines: Vec<(usize, &str)> = normalized_input
            .lines()
            .enumerate()
            .filter_map(|(i, line)| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some((i + 1, trimmed))
                }
            })
            .collect();

        assert!(!input_lines.is_empty(), "Input file has no non-empty lines");

        // For each input line, extract significant words (3+ chars) and check they
        // appear in the reassembled chunks. Pure punctuation or very short tokens
        // may be merged or transformed by XML escaping, so we check word presence.
        let mut missing_lines: Vec<(usize, String)> = Vec::new();

        for &(line_num, line_text) in &input_lines {
            // Extract words of 3+ alphabetic chars from this line
            let words: Vec<&str> = line_text
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| w.len() >= 3)
                .collect();

            if words.is_empty() {
                // Line is only short tokens / punctuation – skip
                continue;
            }

            // Check that at least half the significant words appear in the reassembled text
            let found_count = words.iter().filter(|w| reassembled.contains(**w)).count();

            if found_count < (words.len() + 1) / 2 {
                missing_lines.push((line_num, line_text.to_string()));
            }
        }

        if !missing_lines.is_empty() {
            let report: String = missing_lines
                .iter()
                .take(50)
                .map(|(num, text)| format!("  line {num}: {text}"))
                .collect::<Vec<_>>()
                .join("\n");
            panic!(
                "{} lines appear to be skipped by the TTS chunking pipeline:\n{report}",
                missing_lines.len()
            );
        }

        // Verify ordering: for each pair of consecutive input lines, check that
        // the first significant word of line N appears before the first significant
        // word of line N+1 in the reassembled text.
        let mut out_of_order: Vec<(usize, usize)> = Vec::new();
        let mut last_pos: usize = 0;

        for &(line_num, line_text) in &input_lines {
            let first_word = line_text
                .split(|c: char| !c.is_alphanumeric())
                .find(|w| w.len() >= 3);

            let Some(word) = first_word else {
                continue;
            };

            if let Some(pos) = reassembled[last_pos..].find(word) {
                last_pos += pos + word.len();
            } else if let Some(pos) = reassembled.find(word) {
                // Word found but before last_pos → out of order
                if pos < last_pos {
                    out_of_order.push((line_num, pos));
                }
            }
        }

        if !out_of_order.is_empty() {
            let report: String = out_of_order
                .iter()
                .take(50)
                .map(|(num, pos)| {
                    format!("  line {num} found at chunk position {pos} (before previous line)")
                })
                .collect::<Vec<_>>()
                .join("\n");
            panic!(
                "{} lines appear to be recorded out of order:\n{report}",
                out_of_order.len()
            );
        }

        eprintln!(
            "OK: {} non-empty lines processed into {} TTS chunks, all in order, none skipped.",
            input_lines.len(),
            chunks.len()
        );
    }

    #[test]
    fn audiobook_real_pipeline_chunk_count_for_env_file() {
        let path_owned = std::env::var("SONARPAD_TTS_TEST_PATH")
            .unwrap_or_else(|_| r"C:\Users\ambro\Downloads\05-02-2026.txt".to_string());
        let path = path_owned.as_str();
        let raw = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Skipping test – cannot read {path}: {e}");
                return;
            }
        };

        let cleaned = strip_dashed_lines(&raw);
        let prepared = prepare_tts_text(&cleaned, true, &[]);
        let chunks = split_text_for_engine(&prepared, TtsEngine::Edge);

        assert!(
            !chunks.is_empty(),
            "real audiobook pipeline produced 0 chunks"
        );

        let first_preview = preview_for_log(&chunks[0], 140);
        let last_preview = preview_for_log(&chunks[chunks.len() - 1], 140);
        eprintln!(
            "REAL PIPELINE: cleaned_chars={} prepared_chars={} chunks={} first=\"{}\" last=\"{}\"",
            cleaned.chars().count(),
            prepared.chars().count(),
            chunks.len(),
            first_preview,
            last_preview
        );
    }
}

fn merge_and_finalize_sapi4_audio(
    wav_files: &[PathBuf],
    output: &Path,
    language: Language,
    options: &AudiobookCommonOptions,
) -> Result<(), String> {
    if wav_files.is_empty() {
        return Ok(());
    }

    let extension = output
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let is_mp3 = extension == "mp3";
    let is_aac = extension == "m4b" || extension == "m4a" || extension == "mp4";
    let final_wav = if is_mp3 || is_aac {
        output.with_extension("wav.tmp")
    } else {
        output.to_path_buf()
    };

    crate::audio_utils::join_wav_files(wav_files, &final_wav)
        .map_err(|e| format!("Failed to join audio parts: {}", e))?;

    if is_mp3 || is_aac {
        let res = if is_aac {
            let settings = crate::ffmpeg_export::ConvertAudioSettings {
                format: crate::ffmpeg_export::ConvertAudioFormat::Aac,
                quality: crate::ffmpeg_export::ConvertAudioQuality::BitrateKbps(
                    options.audiobook_bitrate_kbps,
                ),
            };
            let mut progress = |_p: u32| {};
            crate::ffmpeg_export::convert_audio_file(
                &final_wav,
                output,
                &settings,
                None,
                Some(&mut progress),
            )
        } else {
            let settings = crate::ffmpeg_export::ConvertAudioSettings {
                format: crate::ffmpeg_export::ConvertAudioFormat::Mp3,
                quality: crate::ffmpeg_export::ConvertAudioQuality::BitrateKbps(
                    options.audiobook_bitrate_kbps,
                ),
            };
            let mut progress = |_p: u32| {};
            crate::ffmpeg_export::convert_audio_file(
                &final_wav,
                output,
                &settings,
                None,
                Some(&mut progress),
            )
        };
        std::fs::remove_file(&final_wav).ok();
        if let Err(e) = res {
            return Err(i18n::tr_f(language, "sapi5.mf_error", &[("err", &e)]));
        }
    }
    Ok(())
}

const SAPI5_MAX_PARALLEL_WORKERS: usize = 12;
const SAPI5_MIN_OUTPUT_BYTES: u64 = 1_024;
const SAPI5_FINAL_DURATION_TOLERANCE_PERCENT: u64 = 82;

#[derive(Clone)]
struct Sapi5ParallelUnit {
    index: usize,
    start_chunk: usize,
    end_chunk: usize,
    chunks: Vec<String>,
    output: PathBuf,
    text_chars: usize,
}

struct Sapi5PartMetrics {
    bytes: u64,
    size_duration_ms: u64,
    decoded_duration_ms: Option<u64>,
    minimum_duration_ms: u64,
    suspicious_reason: Option<String>,
}

fn sapi5_diagnostic_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn sapi5_compatibility_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn sapi5_diagnostic_path() -> PathBuf {
    crate::settings::settings_dir().join("sapi5_audiobook_diagnostic.log")
}

fn sapi5_compatibility_path() -> PathBuf {
    crate::settings::settings_dir().join("sapi5_audiobook_compatibility_process.json")
}

fn sapi5_active_attempt_path() -> PathBuf {
    crate::settings::settings_dir().join("sapi5_audiobook_active_attempt.json")
}

fn sapi5_next_lower_worker_limit(current: usize) -> usize {
    sapi5_worker_levels(current)
        .into_iter()
        .find(|candidate| *candidate < current.max(1))
        .unwrap_or(1)
}

fn sapi5_extract_log_field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let marker = format!("{}=", key);
    let start = line.find(&marker)?.saturating_add(marker.len());
    let rest = &line[start..];
    if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        Some(&stripped[..end])
    } else {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        Some(&rest[..end])
    }
}

fn sapi5_recover_interrupted_attempt_from_log() -> Option<(String, usize, u128)> {
    let contents = std::fs::read_to_string(sapi5_diagnostic_path()).ok()?;
    let mut latest: Option<(String, usize, u128, bool)> = None;
    for line in contents.lines() {
        if line.contains("SESSION_START") && line.contains("isolation=process") {
            let id = sapi5_extract_log_field(line, "id")?.parse::<u128>().ok()?;
            let voice = sapi5_extract_log_field(line, "voice")?.to_string();
            let limit = sapi5_extract_log_field(line, "initial_limit")?
                .parse::<usize>()
                .ok()?;
            latest = Some((voice, limit, id, false));
            continue;
        }
        let Some((_voice, _limit, id, completed)) = latest.as_mut() else {
            continue;
        };
        let id_marker = format!("id={}", id);
        if line.contains(&id_marker)
            && (line.contains("SESSION_SUCCESS")
                || line.contains("SESSION_ERROR")
                || line.contains("SESSION_RECOVERED"))
        {
            *completed = true;
        }
    }
    latest.and_then(|(voice, limit, id, completed)| (!completed).then_some((voice, limit, id)))
}

fn sapi5_read_active_attempt() -> Option<(String, usize, u128)> {
    let contents = std::fs::read_to_string(sapi5_active_attempt_path()).ok()?;
    let value: serde_json::Value = serde_json::from_str(&contents).ok()?;
    if value.get("isolation").and_then(|mode| mode.as_str()) != Some("process") {
        // Ignore and remove guards created by the previous in-process implementation.
        // The isolated worker model must receive a fresh trial rather than inheriting
        // a lower limit from a crash mode that it was specifically introduced to fix.
        sapi5_clear_active_attempt();
        return None;
    }
    let voice = value.get("voice")?.as_str()?.to_string();
    let worker_limit = value.get("worker_limit")?.as_u64()? as usize;
    let session_id = value.get("session_id")?.as_u64()? as u128;
    Some((voice, worker_limit, session_id))
}

fn sapi5_write_active_attempt(voice: &str, worker_limit: usize, session_id: u128) {
    let path = sapi5_active_attempt_path();
    if let Some(parent) = path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
    {
        crate::log_debug(&format!(
            "SAPI5 recovery: failed to create settings directory: {}",
            err
        ));
        return;
    }
    let value = serde_json::json!({
        "voice": voice,
        "worker_limit": worker_limit.clamp(1, SAPI5_MAX_PARALLEL_WORKERS),
        "session_id": session_id.min(u128::from(u64::MAX)) as u64,
        "isolation": "process",
    });
    match serde_json::to_string_pretty(&value) {
        Ok(json) => {
            if let Err(err) = std::fs::write(&path, json) {
                crate::log_debug(&format!(
                    "SAPI5 recovery: failed to write {:?}: {}",
                    path, err
                ));
            }
        }
        Err(err) => crate::log_debug(&format!(
            "SAPI5 recovery: failed to serialize active attempt: {}",
            err
        )),
    }
}

fn sapi5_clear_active_attempt() {
    let path = sapi5_active_attempt_path();
    if path.exists()
        && let Err(err) = std::fs::remove_file(&path)
    {
        crate::log_debug(&format!(
            "SAPI5 recovery: failed to remove {:?}: {}",
            path, err
        ));
    }
}

fn sapi5_recover_interrupted_attempt() {
    let recovered = sapi5_read_active_attempt().or_else(sapi5_recover_interrupted_attempt_from_log);
    let Some((voice, failed_limit, session_id)) = recovered else {
        return;
    };
    let lower_limit = sapi5_next_lower_worker_limit(failed_limit);
    let existing_limit = load_sapi5_worker_limit(&voice);
    let recovered_limit = existing_limit
        .map(|limit| limit.min(lower_limit))
        .unwrap_or(lower_limit);
    save_sapi5_worker_limit(&voice, recovered_limit);
    sapi5_clear_active_attempt();
    sapi5_log_diagnostic(&format!(
        "SESSION_RECOVERED id={} voice={:?} interrupted_worker_limit={} recovered_worker_limit={} reason=previous process ended before attempt completion",
        session_id, voice, failed_limit, recovered_limit
    ));
}

fn sapi5_log_diagnostic(message: &str) {
    let _guard = sapi5_diagnostic_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = sapi5_diagnostic_path();
    if let Some(parent) = path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
    {
        crate::log_debug(&format!(
            "SAPI5 diagnostic: failed to create log directory: {}",
            err
        ));
        return;
    }
    let opened = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path);
    let mut file = match opened {
        Ok(file) => file,
        Err(err) => {
            crate::log_debug(&format!(
                "SAPI5 diagnostic: failed to open {:?}: {}",
                path, err
            ));
            return;
        }
    };
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    if writeln!(file, "{} {}", timestamp, message).is_err() {
        crate::log_debug("SAPI5 diagnostic: failed to append a log line");
    }
}

fn sapi5_voice_compatibility_key(voice: &str) -> String {
    voice.trim().to_lowercase()
}

fn load_sapi5_compatibility_map() -> BTreeMap<String, usize> {
    let path = sapi5_compatibility_path();
    let Ok(json) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    serde_json::from_str(&json).unwrap_or_default()
}

fn load_sapi5_worker_limit(voice: &str) -> Option<usize> {
    let _guard = sapi5_compatibility_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    load_sapi5_compatibility_map()
        .get(&sapi5_voice_compatibility_key(voice))
        .copied()
        .map(|value| value.clamp(1, SAPI5_MAX_PARALLEL_WORKERS))
}

fn save_sapi5_worker_limit(voice: &str, limit: usize) {
    let _guard = sapi5_compatibility_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = sapi5_compatibility_path();
    let mut map = load_sapi5_compatibility_map();
    map.insert(
        sapi5_voice_compatibility_key(voice),
        limit.clamp(1, SAPI5_MAX_PARALLEL_WORKERS),
    );
    if let Some(parent) = path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
    {
        crate::log_debug(&format!(
            "SAPI5 compatibility: failed to create directory: {}",
            err
        ));
        return;
    }
    match serde_json::to_string_pretty(&map) {
        Ok(json) => {
            if let Err(err) = std::fs::write(&path, json) {
                crate::log_debug(&format!(
                    "SAPI5 compatibility: failed to write {:?}: {}",
                    path, err
                ));
            }
        }
        Err(err) => crate::log_debug(&format!(
            "SAPI5 compatibility: failed to serialize worker limits: {}",
            err
        )),
    }
}

fn sapi5_worker_levels(initial: usize) -> Vec<usize> {
    let initial = initial.clamp(1, SAPI5_MAX_PARALLEL_WORKERS);
    let mut levels = vec![initial];
    for candidate in [8usize, 6, 4, 2, 1] {
        if candidate < initial && !levels.contains(&candidate) {
            levels.push(candidate);
        }
    }
    levels
}

fn sapi5_text_char_count(chunks: &[String]) -> usize {
    chunks
        .iter()
        .flat_map(|chunk| chunk.chars())
        .filter(|ch| ch.is_alphanumeric())
        .count()
}

fn sapi5_max_plausible_chars_per_second(rate: i32) -> u64 {
    let mapped_rate = (rate / 10).clamp(-10, 10);
    if mapped_rate >= 0 {
        45u64.saturating_add((mapped_rate as u64).saturating_mul(4))
    } else {
        45u64.saturating_sub(((-mapped_rate) as u64).min(10))
    }
    .max(20)
}

fn sapi5_minimum_plausible_duration_ms(text_chars: usize, rate: i32) -> u64 {
    if text_chars == 0 {
        return 0;
    }
    let chars_per_second = sapi5_max_plausible_chars_per_second(rate);
    let chars = text_chars as u64;
    chars
        .saturating_mul(1_000)
        .saturating_div(chars_per_second.max(1))
        .max(500)
}

fn sapi5_mp3_duration_from_size_ms(bytes: u64, bitrate_kbps: u32) -> u64 {
    if bitrate_kbps == 0 {
        return 0;
    }
    bytes
        .saturating_mul(8)
        .saturating_div(u64::from(bitrate_kbps))
}

fn sapi5_decoded_duration_ms(path: &Path) -> Option<u64> {
    let mp3_ms = mp3_duration::from_path(path)
        .ok()
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64);
    let mf_ms = crate::mf_encoder::get_audio_duration_mf(path)
        .ok()
        .map(|seconds| seconds.saturating_mul(1_000));
    match (mp3_ms, mf_ms) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn validate_sapi5_parallel_output(
    unit: &Sapi5ParallelUnit,
    bitrate_kbps: u32,
    rate: i32,
) -> Sapi5PartMetrics {
    let bytes = std::fs::metadata(&unit.output)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let size_duration_ms = sapi5_mp3_duration_from_size_ms(bytes, bitrate_kbps);
    let decoded_duration_ms = sapi5_decoded_duration_ms(&unit.output);
    let minimum_duration_ms = sapi5_minimum_plausible_duration_ms(unit.text_chars, rate);
    let suspicious_reason = if bytes == 0 {
        Some("missing or empty MP3 output".to_string())
    } else if bytes < SAPI5_MIN_OUTPUT_BYTES && unit.text_chars >= 100 {
        Some(format!("MP3 output is only {} bytes", bytes))
    } else if unit.text_chars >= 400 && size_duration_ms < minimum_duration_ms {
        Some(format!(
            "size-based duration {} ms is below conservative minimum {} ms for {} text characters",
            size_duration_ms, minimum_duration_ms, unit.text_chars
        ))
    } else {
        None
    };
    Sapi5PartMetrics {
        bytes,
        size_duration_ms,
        decoded_duration_ms,
        minimum_duration_ms,
        suspicious_reason,
    }
}

const SAPI5_WORKER_MODE_ARG: &str = "--sapi5-audiobook-worker";
const SAPI5_WORKER_POLL_MS: u64 = 100;
const SAPI5_WORKER_MIN_STALL_TIMEOUT_SECS: u64 = 300;
const SAPI5_WORKER_MAX_STALL_TIMEOUT_SECS: u64 = 900;
const CREATE_NO_WINDOW_FLAG: u32 = 0x0800_0000;
const SAPI5_ATTEMPT_ABORTED_MSG: &str =
    "Retry requested because another isolated SAPI5 worker failed";

#[derive(Serialize, Deserialize)]
struct Sapi5WorkerRequest {
    chunks: Vec<String>,
    voice_name: String,
    output_path: PathBuf,
    language: Language,
    rate: i32,
    pitch: i32,
    volume: i32,
    audiobook_bitrate_kbps: u32,
    heartbeat_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct Sapi5WorkerResult {
    success: bool,
    error: Option<String>,
    completed_chunks: usize,
}

#[derive(Serialize, Deserialize)]
struct Sapi5WorkerHeartbeat {
    completed_chunks: usize,
    updated_unix_ms: u64,
    phase: String,
}

struct Sapi5ProcessWorkerContext {
    voice: String,
    language: Language,
    rate: i32,
    pitch: i32,
    volume: i32,
    audiobook_bitrate_kbps: u32,
    cancel: Arc<AtomicBool>,
    attempt_abort: Arc<AtomicBool>,
    progress_hwnd: HWND,
    progress_counter: Arc<std::sync::atomic::AtomicUsize>,
    report_progress: bool,
}

fn sapi5_worker_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn sapi5_write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Invalid worker file path: {:?}", path))?;
    std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    let json = serde_json::to_vec(value).map_err(|err| err.to_string())?;
    let temporary = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("json")
    ));
    std::fs::write(&temporary, json).map_err(|err| err.to_string())?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|err| err.to_string())?;
    }
    std::fs::rename(&temporary, path).map_err(|err| err.to_string())
}

fn sapi5_write_worker_heartbeat(
    path: &Path,
    completed_chunks: usize,
    phase: &str,
) -> Result<(), String> {
    sapi5_write_json_atomic(
        path,
        &Sapi5WorkerHeartbeat {
            completed_chunks,
            updated_unix_ms: sapi5_worker_now_ms(),
            phase: phase.to_string(),
        },
    )
}

fn sapi5_read_worker_heartbeat(path: &Path) -> Option<Sapi5WorkerHeartbeat> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn sapi5_worker_stall_timeout(chunks: &[String], rate: i32) -> Duration {
    let largest_chunk_chars = chunks
        .iter()
        .map(|chunk| chunk.chars().filter(|ch| ch.is_alphanumeric()).count())
        .max()
        .unwrap_or(0) as u64;
    let mapped_rate = (rate / 10).clamp(-10, 10);
    let conservative_chars_per_second = if mapped_rate >= 0 {
        5u64.saturating_add(mapped_rate as u64)
    } else {
        5u64.saturating_sub(((-mapped_rate) as u64).min(3))
    }
    .max(2);
    let estimated_seconds = largest_chunk_chars
        .saturating_div(conservative_chars_per_second)
        .saturating_add(180);
    Duration::from_secs(estimated_seconds.clamp(
        SAPI5_WORKER_MIN_STALL_TIMEOUT_SECS,
        SAPI5_WORKER_MAX_STALL_TIMEOUT_SECS,
    ))
}

fn sapi5_cleanup_worker_protocol_files(paths: &[&Path]) {
    for path in paths {
        if path.exists()
            && let Err(err) = std::fs::remove_file(path)
        {
            crate::log_debug(&format!(
                "SAPI5 process worker: failed to remove protocol file {:?}: {}",
                path, err
            ));
        }
    }
}

fn sapi5_worker_protocol_dir() -> PathBuf {
    std::env::temp_dir().join("sonarpad_sapi5_workers")
}

fn sapi5_cleanup_stale_worker_protocol_files() {
    let directory = sapi5_worker_protocol_dir();
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return;
    };
    let maximum_age = Duration::from_secs(24 * 60 * 60);
    for entry in entries.flatten() {
        let path = entry.path();
        let is_stale = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|elapsed| elapsed >= maximum_age);
        if is_stale && let Err(err) = std::fs::remove_file(&path) {
            crate::log_debug(&format!(
                "SAPI5 process worker: failed to remove stale protocol file {:?}: {}",
                path, err
            ));
        }
    }
}

pub(crate) fn run_sapi5_audiobook_worker_from_args(args: &[String]) -> Option<i32> {
    let index = args
        .iter()
        .position(|argument| argument == SAPI5_WORKER_MODE_ARG)?;
    let request_path = match args.get(index + 1) {
        Some(value) => PathBuf::from(value),
        None => return Some(2),
    };
    let result_path = match args.get(index + 2) {
        Some(value) => PathBuf::from(value),
        None => return Some(2),
    };
    Some(run_sapi5_audiobook_worker(&request_path, &result_path))
}

fn run_sapi5_audiobook_worker(request_path: &Path, result_path: &Path) -> i32 {
    unsafe {
        let _previous_error_mode = SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }
    lower_current_audiobook_worker_priority("isolated SAPI5 process");
    let request_bytes = match std::fs::read(request_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            crate::log_debug(&format!(
                "SAPI5 process worker: failed to read request {:?}: {}",
                request_path, err
            ));
            return 2;
        }
    };
    let request: Sapi5WorkerRequest = match serde_json::from_slice(&request_bytes) {
        Ok(request) => request,
        Err(err) => {
            crate::log_debug(&format!(
                "SAPI5 process worker: invalid request {:?}: {}",
                request_path, err
            ));
            return 2;
        }
    };
    if let Err(err) = sapi5_write_worker_heartbeat(&request.heartbeat_path, 0, "starting") {
        crate::log_debug(&format!(
            "SAPI5 process worker: failed to write initial heartbeat: {}",
            err
        ));
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let mut completed_chunks = 0usize;
    let synthesis_result = crate::sapi5_engine::speak_sapi_to_file(
        crate::sapi5_engine::SapiExportOptions {
            chunks: &request.chunks,
            voice_name: &request.voice_name,
            output_path: &request.output_path,
            language: request.language,
            rate: request.rate,
            pitch: request.pitch,
            volume: request.volume,
            audiobook_bitrate_kbps: request.audiobook_bitrate_kbps,
            cancel,
        },
        |completed| {
            completed_chunks = completed_chunks.max(completed);
            if let Err(err) = sapi5_write_worker_heartbeat(
                &request.heartbeat_path,
                completed_chunks,
                "synthesizing",
            ) {
                crate::log_debug(&format!(
                    "SAPI5 process worker: failed to update heartbeat: {}",
                    err
                ));
            }
        },
    );

    let (success, error, exit_code) = match synthesis_result {
        Ok(()) => (true, None, 0),
        Err(err) => (false, Some(err), 1),
    };
    let phase = if success { "completed" } else { "error" };
    if let Err(err) = sapi5_write_worker_heartbeat(&request.heartbeat_path, completed_chunks, phase)
    {
        crate::log_debug(&format!(
            "SAPI5 process worker: failed to write final heartbeat: {}",
            err
        ));
    }
    let result = Sapi5WorkerResult {
        success,
        error,
        completed_chunks,
    };
    if let Err(err) = sapi5_write_json_atomic(result_path, &result) {
        crate::log_debug(&format!(
            "SAPI5 process worker: failed to write result {:?}: {}",
            result_path, err
        ));
        return 2;
    }
    exit_code
}

fn sapi5_post_completed_chunks(
    count: usize,
    progress_counter: &Arc<std::sync::atomic::AtomicUsize>,
    progress_hwnd: HWND,
) {
    for _ in 0..count {
        let current = progress_counter.fetch_add(1, Ordering::SeqCst) + 1;
        if progress_hwnd.0 != 0 {
            unsafe {
                if let Err(err) = PostMessageW(
                    progress_hwnd,
                    crate::WM_UPDATE_PROGRESS,
                    WPARAM(current),
                    LPARAM(0),
                ) {
                    crate::log_debug(&format!(
                        "Failed to post WM_UPDATE_PROGRESS from SAPI5 process worker: {}",
                        err
                    ));
                }
            }
        }
    }
}

fn run_sapi5_unit_subprocess(
    unit: &Sapi5ParallelUnit,
    worker_slot: usize,
    actual_workers: usize,
    attempt: usize,
    context: &Sapi5ProcessWorkerContext,
) -> Result<Sapi5PartMetrics, String> {
    // Keep the worker protocol on a short system-temp path. Long audiobook
    // titles can otherwise make the JSON/heartbeat paths exceed legacy Windows limits.
    let protocol_dir = sapi5_worker_protocol_dir();
    let protocol_prefix = format!(
        "p{}_a{}_u{}_{}",
        std::process::id(),
        attempt,
        unit.index,
        Uuid::new_v4().simple()
    );
    let request_path = protocol_dir.join(format!("{}.request.json", protocol_prefix));
    let result_path = protocol_dir.join(format!("{}.result.json", protocol_prefix));
    let heartbeat_path = protocol_dir.join(format!("{}.heartbeat.json", protocol_prefix));
    sapi5_cleanup_worker_protocol_files(&[&request_path, &result_path, &heartbeat_path]);

    let request = Sapi5WorkerRequest {
        chunks: unit.chunks.clone(),
        voice_name: context.voice.clone(),
        output_path: unit.output.clone(),
        language: context.language,
        rate: context.rate,
        pitch: context.pitch,
        volume: context.volume,
        audiobook_bitrate_kbps: context.audiobook_bitrate_kbps,
        heartbeat_path: heartbeat_path.clone(),
    };
    sapi5_write_json_atomic(&request_path, &request)?;

    let executable = std::env::current_exe().map_err(|err| {
        format!(
            "Unable to locate Sonarpad executable for SAPI5 worker: {}",
            err
        )
    })?;
    let mut command = Command::new(&executable);
    command
        .arg(SAPI5_WORKER_MODE_ARG)
        .arg(&request_path)
        .arg(&result_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env("SONARPAD_SAPI5_WORKER", "1")
        .creation_flags(CREATE_NO_WINDOW_FLAG);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            sapi5_cleanup_worker_protocol_files(&[&request_path, &result_path, &heartbeat_path]);
            return Err(format!("Unable to start isolated SAPI5 worker: {}", err));
        }
    };

    sapi5_log_diagnostic(&format!(
        "WORKER_PROCESS_START attempt={} worker_slot={}/{} unit={} pid={} executable={:?} request={:?}",
        attempt,
        worker_slot + 1,
        actual_workers,
        unit.index,
        child.id(),
        executable,
        request_path
    ));

    let stall_timeout = sapi5_worker_stall_timeout(&unit.chunks, context.rate);
    let monitor_started = Instant::now();
    let mut last_activity = Instant::now();
    let mut last_completed_chunks = 0usize;

    let status = loop {
        if context.cancel.load(Ordering::Relaxed) {
            if let Err(err) = child.kill() {
                crate::log_debug(&format!(
                    "SAPI5 process worker: failed to terminate cancelled worker {}: {}",
                    child.id(),
                    err
                ));
            }
            if let Err(err) = child.wait() {
                crate::log_debug(&format!(
                    "SAPI5 process worker: failed to wait for cancelled worker: {}",
                    err
                ));
            }
            sapi5_cleanup_worker_protocol_files(&[&request_path, &result_path, &heartbeat_path]);
            return Err("Cancelled".to_string());
        }

        if context.attempt_abort.load(Ordering::Acquire) {
            let pid = child.id();
            if let Err(err) = child.kill() {
                crate::log_debug(&format!(
                    "SAPI5 process worker: failed to terminate worker {} after peer failure: {}",
                    pid, err
                ));
            }
            if let Err(err) = child.wait() {
                crate::log_debug(&format!(
                    "SAPI5 process worker: failed to wait for worker {} after peer failure: {}",
                    pid, err
                ));
            }
            sapi5_log_diagnostic(&format!(
                "WORKER_PROCESS_ABORTED attempt={} unit={} pid={} reason=peer worker failure",
                attempt, unit.index, pid
            ));
            sapi5_cleanup_worker_protocol_files(&[&request_path, &result_path, &heartbeat_path]);
            return Err(SAPI5_ATTEMPT_ABORTED_MSG.to_string());
        }

        if let Some(heartbeat) = sapi5_read_worker_heartbeat(&heartbeat_path) {
            let completed = heartbeat.completed_chunks.min(unit.chunks.len());
            if completed > last_completed_chunks {
                let delta = completed.saturating_sub(last_completed_chunks);
                if context.report_progress {
                    sapi5_post_completed_chunks(
                        delta,
                        &context.progress_counter,
                        context.progress_hwnd,
                    );
                }
                last_completed_chunks = completed;
                last_activity = Instant::now();
                sapi5_log_diagnostic(&format!(
                    "WORKER_HEARTBEAT attempt={} unit={} pid={} completed_chunks={}/{} phase={} updated_unix_ms={}",
                    attempt,
                    unit.index,
                    child.id(),
                    completed,
                    unit.chunks.len(),
                    heartbeat.phase,
                    heartbeat.updated_unix_ms
                ));
            }
        }

        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(err) => {
                if let Err(kill_err) = child.kill() {
                    crate::log_debug(&format!(
                        "SAPI5 process worker: failed to terminate worker after wait error: {}",
                        kill_err
                    ));
                }
                if let Err(wait_err) = child.wait() {
                    crate::log_debug(&format!(
                        "SAPI5 process worker: failed to wait after wait error: {}",
                        wait_err
                    ));
                }
                sapi5_cleanup_worker_protocol_files(&[
                    &request_path,
                    &result_path,
                    &heartbeat_path,
                ]);
                return Err(format!("Unable to monitor isolated SAPI5 worker: {}", err));
            }
        }

        if last_activity.elapsed() >= stall_timeout {
            let pid = child.id();
            if let Err(err) = child.kill() {
                crate::log_debug(&format!(
                    "SAPI5 process worker: failed to terminate stalled worker {}: {}",
                    pid, err
                ));
            }
            if let Err(err) = child.wait() {
                crate::log_debug(&format!(
                    "SAPI5 process worker: failed to wait for stalled worker {}: {}",
                    pid, err
                ));
            }
            sapi5_log_diagnostic(&format!(
                "WORKER_PROCESS_STALLED attempt={} unit={} pid={} elapsed_ms={} no_progress_ms={} timeout_ms={}",
                attempt,
                unit.index,
                pid,
                monitor_started.elapsed().as_millis(),
                last_activity.elapsed().as_millis(),
                stall_timeout.as_millis()
            ));
            sapi5_cleanup_worker_protocol_files(&[&request_path, &result_path, &heartbeat_path]);
            return Err(format!(
                "Isolated SAPI5 worker stopped responding for {} seconds",
                stall_timeout.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(SAPI5_WORKER_POLL_MS));
    };

    if let Some(heartbeat) = sapi5_read_worker_heartbeat(&heartbeat_path) {
        let completed = heartbeat.completed_chunks.min(unit.chunks.len());
        if completed > last_completed_chunks {
            if context.report_progress {
                sapi5_post_completed_chunks(
                    completed.saturating_sub(last_completed_chunks),
                    &context.progress_counter,
                    context.progress_hwnd,
                );
            }
            last_completed_chunks = completed;
        }
    }

    let exit_code = status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    let result = std::fs::read(&result_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Sapi5WorkerResult>(&bytes).ok());
    let result_completed_chunks = result
        .as_ref()
        .map(|worker_result| worker_result.completed_chunks.min(unit.chunks.len()))
        .unwrap_or(0);
    if result_completed_chunks > last_completed_chunks && context.report_progress {
        sapi5_post_completed_chunks(
            result_completed_chunks.saturating_sub(last_completed_chunks),
            &context.progress_counter,
            context.progress_hwnd,
        );
    }

    sapi5_log_diagnostic(&format!(
        "WORKER_PROCESS_EXIT attempt={} unit={} pid={} elapsed_ms={} success={} exit_code={} result_present={} completed_chunks={}/{}",
        attempt,
        unit.index,
        child.id(),
        monitor_started.elapsed().as_millis(),
        status.success(),
        exit_code,
        result.is_some(),
        result_completed_chunks,
        unit.chunks.len()
    ));
    sapi5_cleanup_worker_protocol_files(&[&request_path, &result_path, &heartbeat_path]);

    if let Some(worker_result) = result.as_ref()
        && !worker_result.success
    {
        return Err(worker_result
            .error
            .clone()
            .unwrap_or_else(|| "Isolated SAPI5 worker reported an unknown error".to_string()));
    }
    if !status.success() {
        return Err(format!(
            "Isolated SAPI5 worker terminated unexpectedly (exit code {})",
            exit_code
        ));
    }
    if result.is_none() {
        return Err("Isolated SAPI5 worker exited without returning a result".to_string());
    }

    if unit
        .output
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
    {
        let bytes = std::fs::read(&unit.output).map_err(|error| error.to_string())?;
        let (samples, rate, channels) = decode_wav_to_pcm(&bytes)?;
        if samples.is_empty() || rate == 0 || channels == 0 {
            return Err("Isolated SAPI5 worker produced an empty WAV".to_string());
        }
        return Ok(Sapi5PartMetrics {
            bytes: bytes.len() as u64,
            size_duration_ms: 0,
            decoded_duration_ms: Some(
                samples.len() as u64 * 1000 / u64::from(rate) / u64::from(channels),
            ),
            minimum_duration_ms: 0,
            suspicious_reason: None,
        });
    }
    Ok(validate_sapi5_parallel_output(
        unit,
        context.audiobook_bitrate_kbps,
        context.rate,
    ))
}

// Reuse the existing child-process protocol for short mixed-export segments too.
// The child dispatch calls speak_sapi_to_file directly, never this parent helper.
fn synthesize_sapi5_isolated_bytes(
    text: &str,
    config: &SynthesisConfig,
) -> Result<Vec<u8>, String> {
    let output = temp_wav_path("sapi5_isolated");
    let unit = Sapi5ParallelUnit {
        index: 0,
        start_chunk: 0,
        end_chunk: 1,
        chunks: vec![text.to_string()],
        output: output.clone(),
        text_chars: text.chars().count(),
    };
    let context = Sapi5ProcessWorkerContext {
        voice: config.voice.clone(),
        language: config.language,
        rate: config.rate,
        pitch: config.pitch,
        volume: config.volume,
        audiobook_bitrate_kbps: 128,
        cancel: config.cancel.clone(),
        attempt_abort: Arc::new(AtomicBool::new(false)),
        progress_hwnd: HWND(0),
        progress_counter: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        report_progress: false,
    };
    let result = run_sapi5_unit_subprocess(&unit, 0, 1, 1, &context)
        .and_then(|_| std::fs::read(&output).map_err(|error| error.to_string()));
    if output.exists() {
        crate::log_if_err!(std::fs::remove_file(&output));
    }
    result.map_err(|error| format!("SAPI5 isolated synthesis failed: {error}"))
}

pub(crate) fn remember_audio_description_sapi5_limit(voice: &str, limit: usize) {
    let previous = load_sapi5_worker_limit(voice).unwrap_or(SAPI5_MAX_PARALLEL_WORKERS);
    save_sapi5_worker_limit(voice, previous.min(limit));
}

fn run_sapi5_unit_batch(
    units: Vec<Sapi5ParallelUnit>,
    worker_limit: usize,
    attempt: usize,
    options: &AudiobookCommonOptions,
    progress_counter: Arc<std::sync::atomic::AtomicUsize>,
    report_progress: bool,
) -> Vec<(Sapi5ParallelUnit, Result<Sapi5PartMetrics, String>)> {
    if units.is_empty() {
        return Vec::new();
    }
    let actual_workers = units.len().min(worker_limit.max(1));
    let expected_results = units.len();
    let (tx, rx) =
        std::sync::mpsc::channel::<(Sapi5ParallelUnit, Result<Sapi5PartMetrics, String>)>();
    let units_shared = Arc::new(Mutex::new(units.into_iter()));
    let attempt_abort = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::with_capacity(actual_workers);

    for worker_idx in 0..actual_workers {
        let tx = tx.clone();
        let units_shared = units_shared.clone();
        let progress_counter = progress_counter.clone();
        let cancel_token = options.cancel.clone();
        let attempt_abort = attempt_abort.clone();
        let progress_hwnd = options.progress_hwnd;
        let voice = options.voice.to_string();
        let language = options.language;
        let rate = options.rate;
        let pitch = options.pitch;
        let volume = options.volume;
        let bitrate = options.audiobook_bitrate_kbps;

        handles.push(std::thread::spawn(move || {
            lower_current_audiobook_worker_priority("SAPI5 worker");
            // Avoid activating every native COM voice in the same scheduler instant.
            // The delay is below half a second even with 12 workers and does not
            // reduce steady-state parallelism once synthesis has started.
            let startup_delay_ms = (worker_idx as u64).saturating_mul(35).min(385);
            if startup_delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(startup_delay_ms));
            }
            let process_context = Sapi5ProcessWorkerContext {
                voice,
                language,
                rate,
                pitch,
                volume,
                audiobook_bitrate_kbps: bitrate,
                cancel: cancel_token,
                attempt_abort,
                progress_hwnd,
                progress_counter,
                report_progress,
            };
            loop {
                let unit = {
                    let mut guard = units_shared
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    guard.next()
                };
                let Some(unit) = unit else {
                    break;
                };
                if process_context.cancel.load(Ordering::Relaxed) {
                    if tx.send((unit, Err("Cancelled".to_string()))).is_err() {
                        break;
                    }
                    continue;
                }
                if process_context.attempt_abort.load(Ordering::Acquire) {
                    if tx
                        .send((unit, Err(SAPI5_ATTEMPT_ABORTED_MSG.to_string())))
                        .is_err()
                    {
                        break;
                    }
                    continue;
                }
                if unit.output.exists()
                    && let Err(err) = std::fs::remove_file(&unit.output)
                {
                    crate::log_debug(&format!(
                        "SAPI5 adaptive: failed to remove old part {:?}: {}",
                        unit.output, err
                    ));
                }

                let started = Instant::now();
                sapi5_log_diagnostic(&format!(
                    "PART_START attempt={} worker_slot={}/{} unit={} chunks={}..{} chunk_count={} chars={} output={:?}",
                    attempt,
                    worker_idx + 1,
                    actual_workers,
                    unit.index,
                    unit.start_chunk + 1,
                    unit.end_chunk,
                    unit.chunks.len(),
                    unit.text_chars,
                    unit.output
                ));
                let result = run_sapi5_unit_subprocess(
                    &unit,
                    worker_idx,
                    actual_workers,
                    attempt,
                    &process_context,
                );

                if let Err(err) = result.as_ref()
                    && err != "Cancelled"
                    && err != SAPI5_ATTEMPT_ABORTED_MSG
                {
                    process_context.attempt_abort.store(true, Ordering::Release);
                    sapi5_log_diagnostic(&format!(
                        "ATTEMPT_ABORT_REQUESTED attempt={} unit={} reason={}",
                        attempt,
                        unit.index,
                        err.replace(['\r', '\n'], " ")
                    ));
                }

                let checked = match result {
                    Ok(metrics) => {
                        sapi5_log_diagnostic(&format!(
                            "PART_DONE attempt={} unit={} elapsed_ms={} bytes={} size_duration_ms={} decoded_duration_ms={} minimum_duration_ms={} status={} reason={}",
                            attempt,
                            unit.index,
                            started.elapsed().as_millis(),
                            metrics.bytes,
                            metrics.size_duration_ms,
                            metrics
                                .decoded_duration_ms
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "unavailable".to_string()),
                            metrics.minimum_duration_ms,
                            if metrics.suspicious_reason.is_some() {
                                "SUSPICIOUS"
                            } else {
                                "OK"
                            },
                            metrics
                                .suspicious_reason
                                .as_deref()
                                .unwrap_or("none")
                        ));
                        Ok(metrics)
                    }
                    Err(err) => {
                        sapi5_log_diagnostic(&format!(
                            "PART_ERROR attempt={} unit={} elapsed_ms={} error={}",
                            attempt,
                            unit.index,
                            started.elapsed().as_millis(),
                            err.replace(['\r', '\n'], " ")
                        ));
                        if unit.output.exists()
                            && let Err(remove_err) = std::fs::remove_file(&unit.output)
                        {
                            crate::log_debug(&format!(
                                "Failed to remove SAPI5 subpart after process error: {}",
                                remove_err
                            ));
                        }
                        Err(err)
                    }
                };
                if tx.send((unit, checked)).is_err() {
                    break;
                }
            }
        }));
    }
    {
        let _sender = tx;
    }

    let mut results = Vec::with_capacity(expected_results);
    for result in rx {
        results.push(result);
    }
    for handle in handles {
        if let Err(err) = handle.join() {
            crate::log_debug(&format!("SAPI5 adaptive worker join error: {:?}", err));
        }
    }
    if results.len() != expected_results {
        sapi5_log_diagnostic(&format!(
            "BATCH_RESULT_MISMATCH attempt={} expected={} received={}",
            attempt,
            expected_results,
            results.len()
        ));
    }
    results
}

fn run_sapi5_parallel_part(
    chunks: &[String],
    global_progress: &mut usize,
    options: &AudiobookCommonOptions,
) -> Result<(), String> {
    if chunks.is_empty() {
        return Ok(());
    }

    sapi5_cleanup_stale_worker_protocol_files();

    let extension = options
        .output
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if extension != "mp3" {
        return Err("SAPI5 parallel mode supports MP3 output only".to_string());
    }

    let temp_dir = options
        .output
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!(
            "sapi5_tmp_{}",
            options
                .output
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("part")
        ));
    if temp_dir.exists()
        && let Err(err) = std::fs::remove_dir_all(&temp_dir)
    {
        crate::log_debug(&format!(
            "SAPI5 adaptive: failed to remove stale temp directory {:?}: {}",
            temp_dir, err
        ));
    }
    std::fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

    let session_started = Instant::now();
    let chunks_count = chunks.len();
    let automatic_limit = chunks_count
        .min(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .clamp(2, SAPI5_MAX_PARALLEL_WORKERS),
        )
        .max(1);
    sapi5_recover_interrupted_attempt();
    let stored_limit = load_sapi5_worker_limit(options.voice);
    let initial_limit = stored_limit
        .unwrap_or(automatic_limit)
        .min(automatic_limit)
        .max(1);
    let chunks_per_unit = chunks_count.div_ceil(initial_limit.max(1));
    let mut units = Vec::new();
    for index in 0..initial_limit {
        let start_chunk = index.saturating_mul(chunks_per_unit);
        let end_chunk = std::cmp::min(start_chunk.saturating_add(chunks_per_unit), chunks_count);
        if start_chunk >= end_chunk {
            break;
        }
        let unit_chunks = chunks[start_chunk..end_chunk].to_vec();
        units.push(Sapi5ParallelUnit {
            index,
            start_chunk,
            end_chunk,
            text_chars: sapi5_text_char_count(&unit_chunks),
            chunks: unit_chunks,
            output: temp_dir.join(format!("sub_{}.mp3", index)),
        });
    }

    let session_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    sapi5_log_diagnostic(&format!(
        "SESSION_START id={} voice={:?} output={:?} chunks={} automatic_limit={} stored_limit={} initial_limit={} units={} bitrate_kbps={} rate={} pitch={} volume={} isolation=process",
        session_id,
        options.voice,
        options.output,
        chunks_count,
        automatic_limit,
        stored_limit
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        initial_limit,
        units.len(),
        options.audiobook_bitrate_kbps,
        options.rate,
        options.pitch,
        options.volume
    ));

    let base_progress = *global_progress;
    let progress_counter = Arc::new(std::sync::atomic::AtomicUsize::new(base_progress));
    let worker_levels = sapi5_worker_levels(initial_limit);
    let mut pending_units = units;
    let mut successful_parts: BTreeMap<usize, (PathBuf, Sapi5PartMetrics)> = BTreeMap::new();
    let mut effective_limit = initial_limit;
    let mut had_adaptive_retry = false;
    let mut final_error: Option<String> = None;

    for (attempt_index, worker_limit) in worker_levels.iter().copied().enumerate() {
        if pending_units.is_empty() {
            break;
        }
        if options.cancel.load(Ordering::Relaxed) {
            final_error = Some("Cancelled".to_string());
            break;
        }
        sapi5_log_diagnostic(&format!(
            "ATTEMPT_START id={} attempt={} requested_worker_limit={} pending_units={}",
            session_id,
            attempt_index + 1,
            worker_limit,
            pending_units.len()
        ));
        let current_units = std::mem::take(&mut pending_units);
        let expected_results = current_units.len();
        sapi5_write_active_attempt(options.voice, worker_limit, session_id);
        sapi5_log_diagnostic(&format!(
            "ATTEMPT_GUARD_ARMED id={} attempt={} worker_limit={}",
            session_id,
            attempt_index + 1,
            worker_limit
        ));
        let results = run_sapi5_unit_batch(
            current_units,
            worker_limit,
            attempt_index + 1,
            options,
            progress_counter.clone(),
            attempt_index == 0,
        );
        sapi5_clear_active_attempt();
        sapi5_log_diagnostic(&format!(
            "ATTEMPT_GUARD_CLEARED id={} attempt={} worker_limit={}",
            session_id,
            attempt_index + 1,
            worker_limit
        ));
        if results.len() != expected_results {
            final_error = Some(format!(
                "SAPI5 adaptive integrity check failed: expected {} results, got {}",
                expected_results,
                results.len()
            ));
            break;
        }

        for (unit, result) in results {
            match result {
                Ok(metrics) => {
                    if let Some(reason) = metrics.suspicious_reason.as_deref() {
                        had_adaptive_retry = true;
                        sapi5_log_diagnostic(&format!(
                            "PART_RETRY_SCHEDULED id={} unit={} current_worker_limit={} reason={}",
                            session_id, unit.index, worker_limit, reason
                        ));
                        if unit.output.exists()
                            && let Err(err) = std::fs::remove_file(&unit.output)
                        {
                            crate::log_debug(&format!(
                                "SAPI5 adaptive: failed to remove suspicious part {:?}: {}",
                                unit.output, err
                            ));
                        }
                        pending_units.push(unit);
                    } else {
                        successful_parts.insert(unit.index, (unit.output.clone(), metrics));
                    }
                }
                Err(err) => {
                    if err == "Cancelled" {
                        final_error = Some(err);
                        break;
                    }
                    had_adaptive_retry = true;
                    sapi5_log_diagnostic(&format!(
                        "PART_RETRY_SCHEDULED id={} unit={} current_worker_limit={} reason=generation error: {}",
                        session_id,
                        unit.index,
                        worker_limit,
                        err.replace(['\r', '\n'], " ")
                    ));
                    pending_units.push(unit);
                }
            }
        }
        if final_error.is_some() {
            break;
        }
        if pending_units.is_empty() {
            effective_limit = worker_limit;
            sapi5_log_diagnostic(&format!(
                "ATTEMPT_SUCCESS id={} attempt={} worker_limit={} total_valid_parts={}",
                session_id,
                attempt_index + 1,
                worker_limit,
                successful_parts.len()
            ));
            break;
        }
        sapi5_log_diagnostic(&format!(
            "ATTEMPT_RETRY id={} attempt={} failed_or_suspicious_units={} next_worker_limit={}",
            session_id,
            attempt_index + 1,
            pending_units.len(),
            worker_levels
                .get(attempt_index + 1)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
    }

    if final_error.is_none() && !pending_units.is_empty() {
        final_error = Some(format!(
            "SAPI5 adaptive validation failed for {} part(s) even with one worker. Temporary files kept in {:?}",
            pending_units.len(),
            temp_dir
        ));
    }
    if let Some(error) = final_error {
        sapi5_clear_active_attempt();
        sapi5_log_diagnostic(&format!(
            "SESSION_ERROR id={} elapsed_ms={} error={} temp_dir={:?}",
            session_id,
            session_started.elapsed().as_millis(),
            error.replace(['\r', '\n'], " "),
            temp_dir
        ));
        if error == "Cancelled" {
            if let Err(err) = std::fs::remove_dir_all(&temp_dir) {
                crate::log_debug(&format!("Failed to remove SAPI5 temp dir: {}", err));
            }
            return Err(cancelled_message(options.language));
        }
        return Err(error);
    }

    let expected_parts = successful_parts.len();
    if expected_parts == 0 {
        return Err("SAPI5 adaptive validation produced no audio parts".to_string());
    }
    let produced_files: Vec<PathBuf> = successful_parts
        .values()
        .map(|(path, _metrics)| path.clone())
        .collect();
    let expected_duration_ms = successful_parts
        .values()
        .map(|(_path, metrics)| metrics.size_duration_ms)
        .fold(0u64, u64::saturating_add);

    sapi5_log_diagnostic(&format!(
        "MERGE_START id={} parts={} expected_size_duration_ms={}",
        session_id, expected_parts, expected_duration_ms
    ));
    if let Err(err) = merge_and_finalize_sapi4_mp3(
        &produced_files,
        options.output,
        options.language,
        options.audiobook_bitrate_kbps,
        options.progress_hwnd,
    ) {
        sapi5_log_diagnostic(&format!(
            "SESSION_ERROR id={} elapsed_ms={} error=merge failed: {} temp_dir={:?}",
            session_id,
            session_started.elapsed().as_millis(),
            err.replace(['\r', '\n'], " "),
            temp_dir
        ));
        return Err(err);
    }

    let final_bytes = std::fs::metadata(options.output)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let final_size_duration_ms =
        sapi5_mp3_duration_from_size_ms(final_bytes, options.audiobook_bitrate_kbps);
    let final_decoded_duration_ms = sapi5_decoded_duration_ms(options.output);
    let minimum_final_duration_ms = expected_duration_ms
        .saturating_mul(SAPI5_FINAL_DURATION_TOLERANCE_PERCENT)
        .saturating_div(100);
    sapi5_log_diagnostic(&format!(
        "MERGE_DONE id={} final_bytes={} final_size_duration_ms={} final_decoded_duration_ms={} minimum_expected_ms={} status={}",
        session_id,
        final_bytes,
        final_size_duration_ms,
        final_decoded_duration_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unavailable".to_string()),
        minimum_final_duration_ms,
        if final_size_duration_ms >= minimum_final_duration_ms {
            "OK"
        } else {
            "TRUNCATED"
        }
    ));

    if expected_duration_ms > 0 && final_size_duration_ms < minimum_final_duration_ms {
        if options.output.exists()
            && let Err(err) = std::fs::remove_file(options.output)
        {
            crate::log_debug(&format!(
                "SAPI5 adaptive: failed to remove incomplete final MP3 {:?}: {}",
                options.output, err
            ));
        }
        sapi5_log_diagnostic(&format!(
            "SESSION_ERROR id={} elapsed_ms={} error=final MP3 duration guard failed temp_dir={:?}",
            session_id,
            session_started.elapsed().as_millis(),
            temp_dir
        ));
        return Err(format!(
            "SAPI5 final MP3 integrity check failed: expected at least about {} seconds, generated about {} seconds. Temporary parts were kept in {:?}. See {:?}",
            minimum_final_duration_ms / 1_000,
            final_size_duration_ms / 1_000,
            temp_dir,
            sapi5_diagnostic_path()
        ));
    }

    let remembered_limit = if had_adaptive_retry {
        effective_limit
    } else {
        initial_limit
    };
    save_sapi5_worker_limit(options.voice, remembered_limit);
    sapi5_clear_active_attempt();
    sapi5_log_diagnostic(&format!(
        "SESSION_SUCCESS id={} elapsed_ms={} initial_limit={} effective_limit={} adaptive_retry={} remembered_limit={} output={:?}",
        session_id,
        session_started.elapsed().as_millis(),
        initial_limit,
        effective_limit,
        had_adaptive_retry,
        remembered_limit,
        options.output
    ));

    if let Err(err) = std::fs::remove_dir_all(&temp_dir) {
        crate::log_debug(&format!("Failed to remove SAPI5 temp dir: {}", err));
    }
    *global_progress = base_progress.saturating_add(chunks_count);
    Ok(())
}

fn run_split_sapi_audiobook(
    chunks: &[String],
    split_parts: u32,
    options: AudiobookCommonOptions,
) -> Result<(), String> {
    let parts = if split_parts == 0 {
        1
    } else {
        split_parts as usize
    };
    let total_chunks = chunks.len();
    let parts = if total_chunks < parts {
        total_chunks
    } else {
        parts
    };
    let chunks_per_part = total_chunks.div_ceil(parts);
    let mut current_global_progress = 0;

    for part_idx in 0..parts {
        let start_idx = part_idx * chunks_per_part;
        let end_idx = std::cmp::min(start_idx + chunks_per_part, total_chunks);
        if start_idx >= end_idx {
            break;
        }

        let part_chunks = &chunks[start_idx..end_idx];
        let part_output = if parts > 1 {
            split_part_output(options.output, part_idx, parts, options.part_naming_mode)
        } else {
            options.output.to_path_buf()
        };
        let part_options = AudiobookCommonOptions {
            voice: options.voice,
            output: &part_output,
            progress_hwnd: options.progress_hwnd,
            cancel: options.cancel.clone(),
            language: options.language,
            part_naming_mode: options.part_naming_mode,
            part_announcement_mode: options.part_announcement_mode,
            audiobook_title: options.audiobook_title,
            audiobook_bitrate_kbps: options.audiobook_bitrate_kbps,
            rate: options.rate,
            pitch: options.pitch,
            volume: options.volume,
            sapi4_threads: options.sapi4_threads,
        };
        let announced_chunks = if parts > 1 {
            prepend_part_announcement(
                part_chunks,
                &part_options,
                &part_output,
                (part_idx + 1) as u32,
            )
        } else {
            part_chunks.to_vec()
        };

        let extension = part_output
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_aac = extension == "m4b" || extension == "m4a" || extension == "mp4";
        if extension == "mp3" {
            run_sapi5_parallel_part(
                &announced_chunks,
                &mut current_global_progress,
                &part_options,
            )?;
            continue;
        }
        let actual_output = if is_aac {
            part_output.with_extension("wav.tmp")
        } else {
            part_output.clone()
        };

        let progress_hwnd_clone = options.progress_hwnd;
        let cancel_clone = options.cancel.clone();

        crate::sapi5_engine::speak_sapi_to_file(
            crate::sapi5_engine::SapiExportOptions {
                chunks: &announced_chunks,
                voice_name: options.voice,
                output_path: &actual_output,
                language: options.language,
                rate: options.rate,
                pitch: options.pitch,
                volume: options.volume,
                audiobook_bitrate_kbps: options.audiobook_bitrate_kbps,
                cancel: cancel_clone,
            },
            |_chunk_idx| {
                current_global_progress += 1;
                if progress_hwnd_clone.0 != 0 {
                    unsafe {
                        if let Err(e) = PostMessageW(
                            progress_hwnd_clone,
                            crate::WM_UPDATE_PROGRESS,
                            WPARAM(current_global_progress),
                            LPARAM(0),
                        ) {
                            crate::log_debug(&format!("Failed to post WM_UPDATE_PROGRESS: {}", e));
                        }
                    }
                }
            },
        )
        .map_err(|e| {
            if let Err(rem_err) = std::fs::remove_file(&actual_output) {
                crate::log_debug(&format!(
                    "Failed to remove part output after error {}: {}",
                    e, rem_err
                ));
            }
            e
        })?;

        if is_aac {
            let settings = crate::ffmpeg_export::ConvertAudioSettings {
                format: crate::ffmpeg_export::ConvertAudioFormat::Aac,
                quality: crate::ffmpeg_export::ConvertAudioQuality::BitrateKbps(
                    options.audiobook_bitrate_kbps,
                ),
            };
            let mut progress = |_p: u32| {};
            let res = crate::ffmpeg_export::convert_audio_file(
                &actual_output,
                &part_output,
                &settings,
                None,
                Some(&mut progress),
            );
            std::fs::remove_file(&actual_output).ok();
            res?;
        }
    }
    Ok(())
}

fn run_marker_split_sapi_audiobook(
    parts: &[Vec<String>],
    options: AudiobookCommonOptions,
) -> Result<(), String> {
    let parts_len = parts.len();
    let mut current_global_progress = 0;

    for (part_idx, part_chunks) in parts.iter().enumerate() {
        if part_chunks.is_empty() {
            continue;
        }
        let part_output = if parts_len > 1 {
            split_part_output(
                options.output,
                part_idx,
                parts_len,
                options.part_naming_mode,
            )
        } else {
            options.output.to_path_buf()
        };
        let part_options = AudiobookCommonOptions {
            voice: options.voice,
            output: &part_output,
            progress_hwnd: options.progress_hwnd,
            cancel: options.cancel.clone(),
            language: options.language,
            part_naming_mode: options.part_naming_mode,
            part_announcement_mode: options.part_announcement_mode,
            audiobook_title: options.audiobook_title,
            audiobook_bitrate_kbps: options.audiobook_bitrate_kbps,
            rate: options.rate,
            pitch: options.pitch,
            volume: options.volume,
            sapi4_threads: options.sapi4_threads,
        };
        let announced_chunks = if parts_len > 1 {
            prepend_part_announcement(
                part_chunks,
                &part_options,
                &part_output,
                (part_idx + 1) as u32,
            )
        } else {
            part_chunks.to_vec()
        };

        let extension = part_output
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_aac = extension == "m4b" || extension == "m4a" || extension == "mp4";
        if extension == "mp3" {
            run_sapi5_parallel_part(
                &announced_chunks,
                &mut current_global_progress,
                &part_options,
            )?;
            continue;
        }
        let actual_output = if is_aac {
            part_output.with_extension("wav.tmp")
        } else {
            part_output.clone()
        };

        let progress_hwnd_clone = options.progress_hwnd;
        let cancel_clone = options.cancel.clone();

        crate::sapi5_engine::speak_sapi_to_file(
            crate::sapi5_engine::SapiExportOptions {
                chunks: &announced_chunks,
                voice_name: options.voice,
                output_path: &actual_output,
                language: options.language,
                rate: options.rate,
                pitch: options.pitch,
                volume: options.volume,
                audiobook_bitrate_kbps: options.audiobook_bitrate_kbps,
                cancel: cancel_clone,
            },
            |_chunk_idx| {
                current_global_progress += 1;
                if progress_hwnd_clone.0 != 0 {
                    unsafe {
                        if let Err(e) = PostMessageW(
                            progress_hwnd_clone,
                            crate::WM_UPDATE_PROGRESS,
                            WPARAM(current_global_progress),
                            LPARAM(0),
                        ) {
                            crate::log_debug(&format!("Failed to post WM_UPDATE_PROGRESS: {}", e));
                        }
                    }
                }
            },
        )
        .map_err(|e| {
            if let Err(rem_err) = std::fs::remove_file(&actual_output) {
                crate::log_debug(&format!(
                    "Failed to remove part output after error {}: {}",
                    e, rem_err
                ));
            }
            e
        })?;

        if is_aac {
            let settings = crate::ffmpeg_export::ConvertAudioSettings {
                format: crate::ffmpeg_export::ConvertAudioFormat::Aac,
                quality: crate::ffmpeg_export::ConvertAudioQuality::BitrateKbps(
                    options.audiobook_bitrate_kbps,
                ),
            };
            let mut progress = |_p: u32| {};
            let res = crate::ffmpeg_export::convert_audio_file(
                &actual_output,
                &part_output,
                &settings,
                None,
                Some(&mut progress),
            );
            std::fs::remove_file(&actual_output).ok();
            res?;
        }
    }
    Ok(())
}

pub(crate) fn run_google_audiobook_part(
    chunks: &[String],
    current_global_progress: &mut usize,
    options: &AudiobookCommonOptions,
) -> Result<(), String> {
    let generic_chunks: Vec<TtsChunk> = chunks
        .iter()
        .filter_map(|chunk| {
            let normalized =
                normalize_for_tts_with_profile(chunk, true, TtsSanitizeProfile::Strict);
            if normalized.trim().is_empty() {
                None
            } else {
                Some(TtsChunk {
                    original_len: utf16_len(&normalized),
                    text_to_read: normalized,
                    override_voice: None,
                    pause_ms: None,
                })
            }
        })
        .collect();
    let config = MixedAudiobookConfig {
        main_engine: TtsEngine::Google,
    };
    render_mixed_audiobook_part(
        &generic_chunks,
        current_global_progress,
        options.output,
        options,
        &config,
    )
}

pub(crate) fn run_tts_audiobook_part(
    chunks: &[String],
    current_global_progress: &mut usize,
    options: &AudiobookCommonOptions,
) -> Result<(), String> {
    let is_lithuanian_voice = options.voice.to_ascii_lowercase().starts_with("lt-");
    const LT_EDGE_MAX_BYTES: usize = 900;

    let chunk_texts: Vec<String> = {
        let mut out = Vec::new();
        for chunk in chunks {
            // Force strict sanitization for audiobook edge chunks by default.
            // Safe profile remains available for non-audiobook paths.
            let normalized =
                normalize_for_tts_with_profile(chunk, true, TtsSanitizeProfile::Strict);
            let trimmed = normalized.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !is_edge_text_usable(&sanitize_edge_text(&normalized)) {
                crate::log_debug(&format!(
                    "Edge audiobook: ignoring chunk with no speakable text after sanitization: {:?}",
                    preview_for_log(&normalized, 120)
                ));
                continue;
            }
            if is_lithuanian_voice && normalized.len() > LT_EDGE_MAX_BYTES {
                out.extend(
                    split_text_edge_with_limit(&normalized, LT_EDGE_MAX_BYTES)
                        .into_iter()
                        .filter(|text| {
                            !text.trim().is_empty()
                                && is_edge_text_usable(&sanitize_edge_text(text))
                        }),
                );
            } else {
                out.push(normalized);
            }
        }
        out
    };

    if chunk_texts.is_empty() {
        return Err(i18n::tr(options.language, "app.tts_no_text"));
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| err.to_string())?;

    let edge_render_result = rt.block_on(async {
        if options.cancel.load(Ordering::Relaxed) {
            return Err(cancelled_message(options.language));
        }

        let edge_chunks: Vec<TtsChunk> = chunk_texts
            .iter()
            .map(|chunk| TtsChunk {
                text_to_read: chunk.clone(),
                original_len: utf16_len(chunk),
                override_voice: None,
                pause_ms: None,
            })
            .collect();

        const WS_RETRY_MAX: usize = 3;
        let mut next_index = 0usize;

        let file = std::fs::File::create(options.output).map_err(|err| err.to_string())?;
        let mut writer = BufWriter::new(file);

        let stream_options = EdgeStreamOptions {
            voice: options.voice,
            rate: options.rate,
            pitch: options.pitch,
            volume: options.volume,
            language: options.language,
            cancel: options.cancel.as_ref(),
            progress_hwnd: options.progress_hwnd,
            allow_http_fallback: true,
        };

        let mut attempt = 1usize;
        let last_err = loop {
            if options.cancel.load(Ordering::Relaxed) {
                return Err(cancelled_message(options.language));
            }

            let base_progress = *current_global_progress;
            let res = download_edge_chunks_ws_parallel_to_writer(
                &edge_chunks,
                next_index,
                &stream_options,
                &mut writer,
                current_global_progress,
            )
            .await;

            match res {
                Ok(_) => break None,
                Err(err) => {
                    let completed = current_global_progress.saturating_sub(base_progress);
                    next_index = (next_index + completed).min(edge_chunks.len());
                    let err_str = err.as_str();
                    let retry_forever = is_edge_audiobook_transient_error(err_str);
                    crate::log_debug(&format!(
                        "Edge WS export failed (attempt {}/{}): {}",
                        attempt,
                        if retry_forever {
                            "inf".to_string()
                        } else {
                            WS_RETRY_MAX.to_string()
                        },
                        err_str
                    ));
                    if next_index >= edge_chunks.len() {
                        break Some(err);
                    }
                    if !retry_forever && attempt >= WS_RETRY_MAX {
                        break Some(err);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(edge_retry_delay_ms(
                        err_str, attempt,
                    )));
                    attempt = attempt.saturating_add(1);
                }
            }
        };
        if let Some(err) = last_err {
            return Err(err);
        }
        Ok(())
    });
    if let Err(err) = edge_render_result {
        if let Err(remove_err) = std::fs::remove_file(options.output)
            && remove_err.kind() != std::io::ErrorKind::NotFound
        {
            crate::log_debug(&format!(
                "Edge audiobook: failed to remove partial output {:?} after synthesis error: {}",
                options.output, remove_err
            ));
        }
        crate::log_debug(&format!(
            "Edge audiobook: render aborted; partial output removed and no segment was omitted: {}",
            err
        ));
        return Err(err);
    }

    let extension = options
        .output
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let is_mp3 = extension == "mp3";
    let is_aac = extension == "m4b" || extension == "m4a" || extension == "mp4";

    if is_aac {
        set_audiobook_progress_phase(options.progress_hwnd, true);
        set_audiobook_progress_total(options.progress_hwnd, 200);
        post_audiobook_progress(options.progress_hwnd, 0);
        let source_mp3 = options.output.with_extension("edge_source.tmp.mp3");
        let source_wav = options.output.with_extension("edge_source.tmp.wav");
        if let Err(e) = std::fs::rename(options.output, &source_mp3) {
            return Err(format!("Failed to prepare AAC conversion: {}", e));
        }

        let wav_result = (|| -> Result<(), String> {
            let bytes = std::fs::read(&source_mp3)
                .map_err(|e| format!("Failed to read source MP3 for AAC conversion: {}", e))?;
            let (samples, src_rate, src_channels) = decode_mp3_to_pcm(&bytes)?;
            let target_rate = 48_000u32;
            let target_channels = 2u16;
            let resampled = resample_pcm(
                &samples,
                src_rate,
                src_channels,
                target_rate,
                target_channels,
            );
            write_wav_from_pcm(&source_wav, &resampled, target_rate, target_channels)
        })();
        post_audiobook_progress(options.progress_hwnd, 100);
        if let Err(err) = wav_result {
            crate::log_debug(&format!(
                "AAC post-process skipped: WAV conversion failed: {}",
                err
            ));
            if let Err(restore_err) = std::fs::rename(&source_mp3, options.output) {
                crate::log_debug(&format!(
                    "Failed to restore source MP3 after AAC conversion failure: {}",
                    restore_err
                ));
                return Err(format!(
                    "Failed to restore source MP3 after AAC conversion failure: {}",
                    restore_err
                ));
            }
            if let Err(remove_err) = std::fs::remove_file(&source_wav) {
                crate::log_debug(&format!(
                    "Failed to remove temp WAV after AAC conversion failure: {}",
                    remove_err
                ));
            }
            return Ok(());
        }

        let aac_settings = crate::ffmpeg_export::ConvertAudioSettings {
            format: crate::ffmpeg_export::ConvertAudioFormat::Aac,
            quality: crate::ffmpeg_export::ConvertAudioQuality::BitrateKbps(
                options.audiobook_bitrate_kbps,
            ),
        };
        let mut ffmpeg_progress = |p: u32| {
            post_finalization_progress_range(options.progress_hwnd, p, 100, 99, 200);
        };
        let convert_result = crate::ffmpeg_export::convert_audio_file(
            &source_wav,
            options.output,
            &aac_settings,
            None,
            Some(&mut ffmpeg_progress),
        );
        post_audiobook_progress(options.progress_hwnd, 199);
        if let Err(err) = convert_result {
            crate::log_if_err!(std::fs::remove_file(options.output));
            if let Err(restore_err) = std::fs::rename(&source_mp3, options.output) {
                crate::log_debug(&format!(
                    "Failed to restore source MP3 after AAC re-encode failure: {}",
                    restore_err
                ));
            }
            if let Err(remove_err) = std::fs::remove_file(&source_wav) {
                crate::log_debug(&format!(
                    "Failed to remove temp WAV after AAC re-encode failure: {}",
                    remove_err
                ));
            }
            return Err(err);
        }
        if !KEEP_EDGE_TEMP_AFTER_CONVERSION {
            if let Err(remove_err) = std::fs::remove_file(&source_wav) {
                crate::log_debug(&format!(
                    "Failed to remove temp WAV after AAC re-encode: {}",
                    remove_err
                ));
            }
            if let Err(remove_err) = std::fs::remove_file(&source_mp3) {
                crate::log_debug(&format!(
                    "Failed to remove temp source MP3 after AAC re-encode: {}",
                    remove_err
                ));
            }
        } else {
            crate::log_debug("Keeping AAC edge temp files (debug mode)");
        }
        post_audiobook_progress(options.progress_hwnd, 200);
    } else if is_mp3 {
        set_audiobook_progress_phase(options.progress_hwnd, true);
        set_audiobook_progress_total(options.progress_hwnd, 200);
        post_audiobook_progress(options.progress_hwnd, 0);
        let source_mp3 = options.output.with_extension("edge_source.tmp.mp3");
        let source_wav = options.output.with_extension("edge_source.tmp.wav");
        if let Err(e) = std::fs::rename(options.output, &source_mp3) {
            return Err(format!("Failed to prepare MP3 conversion: {}", e));
        }

        let wav_settings = crate::ffmpeg_export::ConvertAudioSettings {
            format: crate::ffmpeg_export::ConvertAudioFormat::Wav,
            quality: crate::ffmpeg_export::ConvertAudioQuality::None,
        };
        const MP3_TO_WAV_MAX_ATTEMPTS: usize = 12;
        let mut wav_result = Err("MP3->WAV conversion not attempted".to_string());
        for attempt in 1..=MP3_TO_WAV_MAX_ATTEMPTS {
            let mut wav_progress = |p: u32| {
                post_finalization_progress_range(options.progress_hwnd, p, 0, 99, 200);
            };
            wav_result = crate::ffmpeg_export::convert_audio_file_with_channels(
                &source_mp3,
                &source_wav,
                &wav_settings,
                None,
                Some(&mut wav_progress),
                Some(2),
            );
            if wav_result.is_ok() {
                break;
            }
            if let Err(remove_err) = std::fs::remove_file(&source_wav)
                && remove_err.kind() != std::io::ErrorKind::NotFound
            {
                crate::log_debug(&format!(
                    "Failed to remove partial WAV output after MP3->WAV attempt {}: {}",
                    attempt, remove_err
                ));
            }
            if attempt < MP3_TO_WAV_MAX_ATTEMPTS {
                if let Err(err) = &wav_result {
                    crate::log_debug(&format!(
                        "MP3->WAV attempt {}/{} failed: {}. Retrying...",
                        attempt, MP3_TO_WAV_MAX_ATTEMPTS, err
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
        }
        post_audiobook_progress(options.progress_hwnd, 100);
        if let Err(err) = wav_result {
            crate::log_if_err!(std::fs::remove_file(options.output));
            if let Err(restore_err) = std::fs::rename(&source_mp3, options.output) {
                crate::log_debug(&format!(
                    "Failed to restore original MP3 after WAV conversion failure: {}",
                    restore_err
                ));
                return Err(format!(
                    "MP3->WAV failed after {} attempts ({}) and restore failed: {}",
                    MP3_TO_WAV_MAX_ATTEMPTS, err, restore_err
                ));
            }
            if let Err(remove_err) = std::fs::remove_file(&source_wav)
                && remove_err.kind() != std::io::ErrorKind::NotFound
            {
                crate::log_debug(&format!(
                    "Failed to remove temp WAV after conversion failure: {}",
                    remove_err
                ));
            }
            return Err(format!(
                "MP3->WAV failed after {} attempts: {}",
                MP3_TO_WAV_MAX_ATTEMPTS, err
            ));
        }

        let mp3_settings = crate::ffmpeg_export::ConvertAudioSettings {
            format: crate::ffmpeg_export::ConvertAudioFormat::Mp3,
            quality: crate::ffmpeg_export::ConvertAudioQuality::BitrateKbps(
                options.audiobook_bitrate_kbps,
            ),
        };
        let mut last_convert_pct = u32::MAX;
        let mut ffmpeg_progress = |p: u32| {
            let pct = (p / 100).min(100);
            if pct != last_convert_pct {
                last_convert_pct = pct;
                post_finalization_progress_range(options.progress_hwnd, p, 100, 99, 200);
            }
        };
        const MP3_REENCODE_MAX_ATTEMPTS: usize = 12;
        let mut reencode_result = Err("MP3 re-encode not attempted".to_string());
        for attempt in 1..=MP3_REENCODE_MAX_ATTEMPTS {
            reencode_result = crate::ffmpeg_export::convert_audio_file(
                &source_wav,
                options.output,
                &mp3_settings,
                None,
                Some(&mut ffmpeg_progress),
            );
            if reencode_result.is_ok() {
                break;
            }

            if let Err(remove_err) = std::fs::remove_file(options.output)
                && remove_err.kind() != std::io::ErrorKind::NotFound
            {
                crate::log_debug(&format!(
                    "Failed to remove partial MP3 output after re-encode attempt {}: {}",
                    attempt, remove_err
                ));
            }

            if attempt < MP3_REENCODE_MAX_ATTEMPTS {
                if let Err(err) = &reencode_result {
                    crate::log_debug(&format!(
                        "MP3 re-encode attempt {}/{} failed: {}. Retrying...",
                        attempt, MP3_REENCODE_MAX_ATTEMPTS, err
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
        }
        post_audiobook_progress(options.progress_hwnd, 199);
        if let Err(err) = reencode_result {
            crate::log_if_err!(std::fs::remove_file(options.output));
            if let Err(restore_err) = std::fs::rename(&source_mp3, options.output) {
                crate::log_debug(&format!(
                    "Failed to restore original MP3 after re-encode failure: {}",
                    restore_err
                ));
            }
            if let Err(remove_err) = std::fs::remove_file(&source_wav)
                && remove_err.kind() != std::io::ErrorKind::NotFound
            {
                crate::log_debug(&format!(
                    "Failed to remove temp WAV after re-encode failure: {}",
                    remove_err
                ));
            }
            return Err(format!(
                "MP3 re-encode failed after {} attempts: {}",
                MP3_REENCODE_MAX_ATTEMPTS, err
            ));
        }
        if !KEEP_EDGE_TEMP_AFTER_CONVERSION {
            if let Err(remove_err) = std::fs::remove_file(&source_wav)
                && remove_err.kind() != std::io::ErrorKind::NotFound
            {
                crate::log_debug(&format!(
                    "Failed to remove temp WAV after MP3 re-encode: {}",
                    remove_err
                ));
            }
            if let Err(remove_err) = std::fs::remove_file(&source_mp3) {
                crate::log_debug(&format!(
                    "Failed to remove source MP3 after re-encode: {}",
                    remove_err
                ));
            }
        } else {
            crate::log_debug("Keeping MP3 edge temp files (debug mode)");
        }
        post_audiobook_progress(options.progress_hwnd, 200);
    }

    Ok(())
}

pub(crate) struct MixedAudiobookConfig {
    pub(crate) main_engine: TtsEngine,
}

fn mixed_chunk_has_usable_content(text: &str) -> bool {
    let decoded = decode_basic_xml_entities(text);
    let trimmed = decoded.trim();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.chars().any(|ch| ch.is_alphanumeric())
}

fn resolve_mixed_chunk_synth<'a>(
    chunk: &'a TtsChunk,
    options: &'a AudiobookCommonOptions,
    config: &'a MixedAudiobookConfig,
) -> (TtsEngine, &'a str, i32, i32, i32) {
    if let Some(ov) = &chunk.override_voice {
        (
            ov.engine,
            ov.voice.as_str(),
            ov.rate.unwrap_or(0),
            ov.pitch.unwrap_or(0),
            ov.volume.unwrap_or(100),
        )
    } else {
        (
            config.main_engine,
            options.voice,
            options.rate,
            options.pitch,
            options.volume,
        )
    }
}

const GOOGLE_AUDIOBOOK_BATCH_MAX_CHARS: usize = 800;

#[derive(Clone, PartialEq, Eq)]
struct MixedSynthKey {
    engine: TtsEngine,
    voice: String,
    rate: i32,
    pitch: i32,
    volume: i32,
}

#[derive(Clone)]
struct MixedAudiobookUnit {
    first_chunk_index: usize,
    source_chunk_count: usize,
    chunk: TtsChunk,
}

fn mixed_synth_key(
    chunk: &TtsChunk,
    options: &AudiobookCommonOptions,
    config: &MixedAudiobookConfig,
) -> MixedSynthKey {
    let (engine, voice, rate, pitch, volume) = resolve_mixed_chunk_synth(chunk, options, config);
    MixedSynthKey {
        engine,
        voice: voice.to_string(),
        rate,
        pitch,
        volume,
    }
}

fn append_google_batch_text(target: &mut String, incoming: &str) {
    if target.is_empty() {
        target.push_str(incoming);
        return;
    }
    let target_has_space = target.chars().next_back().is_some_and(char::is_whitespace);
    let incoming_has_space = incoming.chars().next().is_some_and(char::is_whitespace);
    if !target_has_space && !incoming_has_space {
        target.push(' ');
    }
    target.push_str(incoming);
}

fn append_google_source_text(target: &mut String, incoming: &str) {
    if !target.is_empty() {
        target.push('\n');
    }
    target.push_str(incoming);
}

fn split_google_text_at_whitespace(text: &str, max_chars: usize) -> Vec<String> {
    let mut remaining = text.trim();
    let mut parts = Vec::new();
    while remaining.chars().count() > max_chars {
        let hard_split = remaining
            .char_indices()
            .nth(max_chars)
            .map(|(index, _)| index)
            .unwrap_or(remaining.len());
        let split_at = remaining[..hard_split]
            .char_indices()
            .rev()
            .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
            .filter(|index| *index > 0)
            .unwrap_or(hard_split);
        let (head, tail) = remaining.split_at(split_at);
        let head = head.trim();
        if !head.is_empty() {
            parts.push(head.to_string());
        }
        remaining = tail.trim_start();
    }
    if !remaining.is_empty() {
        parts.push(remaining.to_string());
    }
    parts
}

fn google_line_starts_with_uppercase(line: &str) -> bool {
    line.trim_start()
        .chars()
        .next()
        .is_some_and(char::is_uppercase)
}

fn split_oversized_google_sentence(text: &str, max_chars: usize) -> Vec<String> {
    let mut line_parts = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let combined_chars = current
            .chars()
            .count()
            .saturating_add(usize::from(!current.is_empty()))
            .saturating_add(line.chars().count());
        if !current.is_empty()
            && combined_chars > max_chars
            && google_line_starts_with_uppercase(line)
        {
            line_parts.push(current);
            current = String::new();
        }
        append_google_batch_text(&mut current, line);
    }
    if !current.is_empty() {
        line_parts.push(current);
    }

    line_parts
        .into_iter()
        .flat_map(|part| split_google_text_at_whitespace(&part, max_chars))
        .collect()
}

fn split_google_audiobook_text(text: &str, max_chars: usize) -> Vec<String> {
    let max_chars = max_chars.max(1);
    let mut parts = Vec::new();
    let mut current = String::new();

    for sentence in split_sentences(text) {
        let sentence = sentence.trim();
        if sentence.is_empty() {
            continue;
        }
        let sentence_parts = if sentence.chars().count() > max_chars {
            split_oversized_google_sentence(sentence, max_chars)
        } else {
            vec![sentence.to_string()]
        };
        for sentence_part in sentence_parts {
            let combined_chars = current
                .chars()
                .count()
                .saturating_add(usize::from(!current.is_empty()))
                .saturating_add(sentence_part.chars().count());
            if !current.is_empty() && combined_chars > max_chars {
                parts.push(current);
                current = String::new();
            }
            append_google_batch_text(&mut current, &sentence_part);
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

fn coalesce_google_audiobook_chunks(
    chunks: &[TtsChunk],
    options: &AudiobookCommonOptions,
    config: &MixedAudiobookConfig,
) -> Vec<MixedAudiobookUnit> {
    let mut units = Vec::with_capacity(chunks.len());
    let mut index = 0usize;
    while index < chunks.len() {
        let first = &chunks[index];
        let key = mixed_synth_key(first, options, config);
        if key.engine != TtsEngine::Google || first.pause_ms.is_some() {
            units.push(MixedAudiobookUnit {
                first_chunk_index: index,
                source_chunk_count: 1,
                chunk: first.clone(),
            });
            index += 1;
            continue;
        }

        let first_chunk_index = index;
        let mut combined_text = String::new();
        let mut source_chunk_count = 0usize;
        let mut next_index = index + 1;
        append_google_source_text(&mut combined_text, &first.text_to_read);
        source_chunk_count += 1;
        while next_index < chunks.len() {
            let next = &chunks[next_index];
            if next.pause_ms.is_some() || mixed_synth_key(next, options, config) != key {
                break;
            }
            append_google_source_text(&mut combined_text, &next.text_to_read);
            source_chunk_count = source_chunk_count.saturating_add(1);
            next_index += 1;
        }

        let batches = split_google_audiobook_text(&combined_text, GOOGLE_AUDIOBOOK_BATCH_MAX_CHARS);
        let batch_count = batches.len().max(1);
        let mut assigned_source_chunks = 0usize;
        for (batch_index, text_to_read) in batches.into_iter().enumerate() {
            let batch_first_chunk_index = first_chunk_index.saturating_add(assigned_source_chunks);
            let cumulative_source_chunks =
                (batch_index + 1).saturating_mul(source_chunk_count) / batch_count;
            let batch_source_chunks =
                cumulative_source_chunks.saturating_sub(assigned_source_chunks);
            assigned_source_chunks = cumulative_source_chunks;
            let mut chunk = first.clone();
            chunk.original_len = utf16_len(&text_to_read);
            chunk.text_to_read = text_to_read;
            units.push(MixedAudiobookUnit {
                first_chunk_index: batch_first_chunk_index,
                source_chunk_count: batch_source_chunks,
                chunk,
            });
        }
        index = next_index;
    }
    units
}

const NON_GOOGLE_AUDIOBOOK_MAX_ATTEMPTS: usize = 5;

fn audiobook_retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(250u64.saturating_mul(attempt.min(20) as u64))
}

fn is_permanent_google_audiobook_error(error: &str) -> bool {
    let normalized = error.to_lowercase();
    normalized.contains("selected google tts voice is not installed")
        || normalized.contains("no google tts voice packages are installed")
        || normalized.contains("google chrome and microsoft edge were not found")
        || normalized.contains("invalid google tts voice path")
        || normalized.contains("unknown google tts voice package")
        || normalized.contains("google tts voice checksum mismatch")
}

fn should_retry_audiobook_segment(engine: TtsEngine, attempt: usize, error: &str) -> bool {
    match engine {
        TtsEngine::Google => !is_permanent_google_audiobook_error(error),
        TtsEngine::Edge => {
            is_edge_audiobook_transient_error(error) || attempt < NON_GOOGLE_AUDIOBOOK_MAX_ATTEMPTS
        }
        TtsEngine::Sapi5 | TtsEngine::Sapi4 => attempt < NON_GOOGLE_AUDIOBOOK_MAX_ATTEMPTS,
    }
}

fn sleep_with_audiobook_cancellation(cancel: &Arc<AtomicBool>, duration: Duration) -> bool {
    let mut remaining = duration;
    while !remaining.is_zero() {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let step = remaining.min(Duration::from_millis(100));
        std::thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
    !cancel.load(Ordering::Relaxed)
}

fn audiobook_segment_failed_message(
    language: Language,
    engine: &str,
    chunk_idx: usize,
    error: &str,
) -> String {
    i18n::tr(language, "audiobook.segment_failed")
        .replace("{engine}", engine)
        .replace("{chunk}", &(chunk_idx + 1).to_string())
        .replace("{error}", error)
}

async fn synthesize_mixed_chunk_with_retry(
    chunk_idx: usize,
    chunk: &TtsChunk,
    synth: &SynthesisConfig,
    target: TargetAudio,
) -> Result<Option<PathBuf>, String> {
    crate::log_debug(&format!(
        "Mixed audiobook synth: chunk={} engine={} voice={:?} rate={} pitch={} volume={} override={} text_preview={:?}",
        chunk_idx,
        match synth.engine {
            TtsEngine::Edge => "edge",
            TtsEngine::Sapi5 => "sapi5",
            TtsEngine::Sapi4 => "sapi4",
            TtsEngine::Google => "google",
        },
        synth.voice,
        synth.rate,
        synth.pitch,
        synth.volume,
        chunk.override_voice.is_some(),
        preview_for_log(&chunk.text_to_read, 120)
    ));

    if let Some(ms) = chunk.pause_ms {
        let path = std::env::temp_dir().join(format!(
            "sonarpad_pause_{}_{}.wav",
            std::process::id(),
            Uuid::new_v4().simple()
        ));
        return match write_silence_wav(&path, ms, target.sample_rate, target.channels) {
            Ok(()) => Ok(Some(path)),
            Err(err) => {
                crate::log_debug(&format!(
                    "Mixed audiobook: failed to create pause chunk={} duration_ms={}: {}",
                    chunk_idx, ms, err
                ));
                Err(audiobook_segment_failed_message(
                    synth.language,
                    "pause",
                    chunk_idx,
                    &err.to_string(),
                ))
            }
        };
    }

    if !mixed_chunk_has_usable_content(&chunk.text_to_read) {
        crate::log_debug(&format!(
            "Mixed audiobook: dropping non-usable chunk={} engine={} voice={:?} text_preview={:?}",
            chunk_idx,
            match synth.engine {
                TtsEngine::Edge => "edge",
                TtsEngine::Sapi5 => "sapi5",
                TtsEngine::Sapi4 => "sapi4",
                TtsEngine::Google => "google",
            },
            synth.voice,
            preview_for_log(&chunk.text_to_read, 120)
        ));
        return Ok(None);
    }

    let engine_name = match synth.engine {
        TtsEngine::Edge => "edge",
        TtsEngine::Sapi5 => "sapi5",
        TtsEngine::Sapi4 => "sapi4",
        TtsEngine::Google => "google",
    };
    let mut attempt = 1usize;
    loop {
        if synth.cancel.load(Ordering::Relaxed) {
            return Err(cancelled_message(synth.language));
        }
        match synthesize_segment_to_wav(&chunk.text_to_read, synth, target).await {
            Ok(wav_path) => return Ok(Some(wav_path)),
            Err(err) => {
                let retry_delay = audiobook_retry_delay(attempt);
                let retry_until_cancelled = synth.engine == TtsEngine::Google
                    || (synth.engine == TtsEngine::Edge && is_edge_audiobook_transient_error(&err));
                crate::log_debug(&format!(
                    "Mixed audiobook: synth failed chunk={} engine={} voice={:?} attempt={} retry_until_cancelled={} retry_delay_ms={} text_preview={:?} error={}",
                    chunk_idx,
                    engine_name,
                    synth.voice,
                    attempt,
                    retry_until_cancelled,
                    retry_delay.as_millis(),
                    preview_for_log(&chunk.text_to_read, 120),
                    err
                ));
                if err.contains("SAPI5 isolated synthesis failed:") {
                    return Err(err);
                }
                let should_retry = should_retry_audiobook_segment(synth.engine, attempt, &err);
                if !should_retry {
                    crate::log_debug(&format!(
                        "Mixed audiobook: aborting render because chunk={} engine={} failed after attempt={}; no segment will be omitted",
                        chunk_idx, engine_name, attempt
                    ));
                    return Err(audiobook_segment_failed_message(
                        synth.language,
                        engine_name,
                        chunk_idx,
                        &err,
                    ));
                }
                if !async_sleep_with_cancellation(synth.cancel.as_ref(), retry_delay).await {
                    return Err(cancelled_message(synth.language));
                }
                attempt = attempt.saturating_add(1);
            }
        }
    }
}

const GOOGLE_AUDIOBOOK_MAX_WORKERS: usize = 8;

fn google_audiobook_worker_limit_for_cores(logical_cores: usize) -> usize {
    match logical_cores {
        20.. => 8,
        16..=19 => 7,
        12..=15 => 6,
        8..=11 => 5,
        6..=7 => 4,
        4..=5 => 3,
        2..=3 => 2,
        _ => 1,
    }
}

fn google_audiobook_worker_limit() -> usize {
    if let Ok(raw) = std::env::var("SONARPAD_GOOGLE_TTS_WORKERS")
        && let Ok(requested) = raw.trim().parse::<usize>()
    {
        return requested.clamp(1, GOOGLE_AUDIOBOOK_MAX_WORKERS);
    }
    let logical_cores = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(4);
    google_audiobook_worker_limit_for_cores(logical_cores)
}

/// Returns the same engine-specific synthesis concurrency used by Sonarpad's
/// audiobook pipeline. Audio descriptions use this to parallelize independent
/// cues without changing their final ordering or scheduling.
pub(crate) fn audiobook_synthesis_parallelism(engine: TtsEngine, voice: &str) -> usize {
    match engine {
        TtsEngine::Edge => MIXED_EDGE_PARALLELISM,
        TtsEngine::Google => google_audiobook_worker_limit(),
        TtsEngine::Sapi5 => {
            sapi5_recover_interrupted_attempt();
            let automatic = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .clamp(2, SAPI5_MAX_PARALLEL_WORKERS);
            load_sapi5_worker_limit(voice)
                .unwrap_or(automatic)
                .min(automatic)
                .max(1)
        }
        TtsEngine::Sapi4 => requested_sapi4_worker_limit(30),
    }
}

struct GoogleAudiobookTask {
    order: usize,
    first_chunk_index: usize,
    source_chunk_count: usize,
    chunk: TtsChunk,
    voice: String,
    rate: i32,
    pitch: i32,
    volume: i32,
}

type GoogleAudiobookSynthesisResult = Result<Option<PathBuf>, String>;
type OrderedGoogleAudiobookResult = Option<GoogleAudiobookSynthesisResult>;

struct GoogleAudiobookTaskResult {
    order: usize,
    first_chunk_index: usize,
    source_chunk_count: usize,
    synthesis_result: GoogleAudiobookSynthesisResult,
    elapsed_ms: u128,
}

fn synthesize_google_audiobook_task(
    worker_id: usize,
    session: &mut crate::google_tts::GoogleTtsWorkerSession,
    task: &GoogleAudiobookTask,
    target: TargetAudio,
    cancel: &Arc<AtomicBool>,
    language: Language,
) -> GoogleAudiobookSynthesisResult {
    if let Some(ms) = task.chunk.pause_ms {
        let path = std::env::temp_dir().join(format!(
            "sonarpad_google_pause_{}_{}_{}.wav",
            std::process::id(),
            worker_id,
            Uuid::new_v4().simple()
        ));
        return match write_silence_wav(&path, ms, target.sample_rate, target.channels) {
            Ok(()) => Ok(Some(path)),
            Err(err) => {
                crate::log_debug(&format!(
                    "Google audiobook worker {}: failed to create pause first_chunk={} duration_ms={}: {}",
                    worker_id, task.first_chunk_index, ms, err
                ));
                Err(audiobook_segment_failed_message(
                    language,
                    "pause",
                    task.first_chunk_index,
                    &err.to_string(),
                ))
            }
        };
    }

    if !mixed_chunk_has_usable_content(&task.chunk.text_to_read) {
        crate::log_debug(&format!(
            "Google audiobook worker {}: dropping non-usable first_chunk={} text_preview={:?}",
            worker_id,
            task.first_chunk_index,
            preview_for_log(&task.chunk.text_to_read, 120)
        ));
        return Ok(None);
    }

    let mut attempt = 1usize;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(cancelled_message(language));
        }
        crate::log_debug(&format!(
            "Google audiobook worker {}: synth start order={} first_chunk={} source_chunks={} attempt={} retry_limit=unlimited text_chars={} voice={:?} rate={} pitch={} volume={}",
            worker_id,
            task.order,
            task.first_chunk_index,
            task.source_chunk_count,
            attempt,
            task.chunk.text_to_read.chars().count(),
            task.voice.as_str(),
            task.rate,
            task.pitch,
            task.volume
        ));
        let synth_started = Instant::now();
        let result = session
            .synthesize_wav_bytes(
                &task.chunk.text_to_read,
                &task.voice,
                task.rate,
                task.pitch,
                task.volume,
                cancel,
            )
            .and_then(|bytes| {
                let (samples, src_rate, src_channels) = decode_wav_to_pcm(&bytes)?;
                let resampled = resample_pcm(
                    &samples,
                    src_rate,
                    src_channels,
                    target.sample_rate,
                    target.channels,
                );
                let path = temp_wav_path("google_parallel");
                write_wav_from_pcm(&path, &resampled, target.sample_rate, target.channels)?;
                Ok(path)
            });
        match result {
            Ok(path) => {
                crate::log_debug(&format!(
                    "Google audiobook worker {}: synth complete order={} first_chunk={} elapsed_ms={} output={:?}",
                    worker_id,
                    task.order,
                    task.first_chunk_index,
                    synth_started.elapsed().as_millis(),
                    path
                ));
                return Ok(Some(path));
            }
            Err(err) => {
                crate::log_debug(&format!(
                    "Google audiobook worker {}: synth failed order={} first_chunk={} attempt={} retry_limit=unlimited elapsed_ms={} error={}",
                    worker_id,
                    task.order,
                    task.first_chunk_index,
                    attempt,
                    synth_started.elapsed().as_millis(),
                    err
                ));
                if !should_retry_audiobook_segment(TtsEngine::Google, attempt, &err) {
                    crate::log_debug(&format!(
                        "Google audiobook worker {}: permanent synthesis error order={} first_chunk={}; the audiobook will stop and no unit will be omitted: {}",
                        worker_id, task.order, task.first_chunk_index, err
                    ));
                    return Err(audiobook_segment_failed_message(
                        language,
                        "google",
                        task.first_chunk_index,
                        &err,
                    ));
                }
                let retry_delay = audiobook_retry_delay(attempt);
                crate::log_debug(&format!(
                    "Google audiobook worker {}: unit order={} first_chunk={} will be retried; retry_delay_ms={} text_preview={:?}",
                    worker_id,
                    task.order,
                    task.first_chunk_index,
                    retry_delay.as_millis(),
                    preview_for_log(&task.chunk.text_to_read, 120)
                ));
                if !sleep_with_audiobook_cancellation(cancel, retry_delay) {
                    return Err(cancelled_message(language));
                }
                attempt = attempt.saturating_add(1);
            }
        }
    }
}

fn render_google_audiobook_units_parallel(
    units: Vec<MixedAudiobookUnit>,
    temp_wavs: &mut Vec<PathBuf>,
    current_global_progress: &mut usize,
    options: &AudiobookCommonOptions,
    config: &MixedAudiobookConfig,
    target: TargetAudio,
) -> Result<(), String> {
    if units.is_empty() {
        return Ok(());
    }

    let logical_cores = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    let worker_count = units.len().min(google_audiobook_worker_limit()).max(1);
    crate::log_debug(&format!(
        "Google audiobook parallel mode: units={} workers={} logical_cores={} max_workers={} env_override={:?}",
        units.len(),
        worker_count,
        logical_cores,
        GOOGLE_AUDIOBOOK_MAX_WORKERS,
        std::env::var("SONARPAD_GOOGLE_TTS_WORKERS").ok()
    ));

    let tasks: Vec<GoogleAudiobookTask> = units
        .into_iter()
        .enumerate()
        .map(|(order, unit)| {
            let chunk = unit.chunk;
            let (engine, voice, rate, pitch, volume) =
                resolve_mixed_chunk_synth(&chunk, options, config);
            let voice = voice.to_string();
            if engine != TtsEngine::Google {
                crate::log_debug(&format!(
                    "Google audiobook parallel mode received non-Google unit order={} engine mismatch",
                    order
                ));
            }
            GoogleAudiobookTask {
                order,
                first_chunk_index: unit.first_chunk_index,
                source_chunk_count: unit.source_chunk_count,
                chunk,
                voice,
                rate,
                pitch,
                volume,
            }
        })
        .collect();
    let expected_results = tasks.len();
    let task_iter = Arc::new(Mutex::new(tasks.into_iter()));
    let (result_rx, handles) = {
        let (result_tx, result_rx) = std::sync::mpsc::channel::<GoogleAudiobookTaskResult>();
        let mut handles = Vec::with_capacity(worker_count);

        for worker_offset in 0..worker_count {
            let worker_id = worker_offset + 1;
            let task_iter = task_iter.clone();
            let result_tx = result_tx.clone();
            let cancel = options.cancel.clone();
            let language = options.language;
            let handle = std::thread::spawn(move || {
                lower_current_audiobook_worker_priority("Google worker");
                let startup_delay =
                    Duration::from_millis((worker_offset as u64).saturating_mul(150));
                if !startup_delay.is_zero() {
                    crate::log_debug(&format!(
                        "Google audiobook worker {}: staggered startup delay_ms={}",
                        worker_id,
                        startup_delay.as_millis()
                    ));
                    if !sleep_with_audiobook_cancellation(&cancel, startup_delay) {
                        crate::log_debug(&format!(
                            "Google audiobook worker {}: stopped during startup delay because the audiobook was cancelled",
                            worker_id
                        ));
                        return;
                    }
                }
                let mut session = crate::google_tts::GoogleTtsWorkerSession::new(worker_id);
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let task = {
                        let mut guard = task_iter.lock().unwrap_or_else(|err| err.into_inner());
                        guard.next()
                    };
                    let Some(task) = task else {
                        break;
                    };
                    let started = Instant::now();
                    let synthesis_result = synthesize_google_audiobook_task(
                        worker_id,
                        &mut session,
                        &task,
                        target,
                        &cancel,
                        language,
                    );
                    if result_tx
                        .send(GoogleAudiobookTaskResult {
                            order: task.order,
                            first_chunk_index: task.first_chunk_index,
                            source_chunk_count: task.source_chunk_count,
                            synthesis_result,
                            elapsed_ms: started.elapsed().as_millis(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                crate::log_debug(&format!(
                    "Google audiobook worker {}: stopped cancelled={}",
                    worker_id,
                    cancel.load(Ordering::Relaxed)
                ));
            });
            handles.push(handle);
        }
        (result_rx, handles)
    };

    let mut ordered_results: Vec<OrderedGoogleAudiobookResult> = vec![None; expected_results];
    let mut received_results = 0usize;
    for result in result_rx {
        received_results = received_results.saturating_add(1);
        let produced = matches!(&result.synthesis_result, Ok(Some(_)));
        let failed = result.synthesis_result.is_err();
        crate::log_debug(&format!(
            "Google audiobook parallel result: order={} first_chunk={} source_chunks={} elapsed_ms={} produced={} failed={}",
            result.order,
            result.first_chunk_index,
            result.source_chunk_count,
            result.elapsed_ms,
            produced,
            failed
        ));
        if result.order < ordered_results.len() {
            ordered_results[result.order] = Some(result.synthesis_result);
        } else if let Ok(Some(path)) = result.synthesis_result {
            crate::log_if_err!(std::fs::remove_file(path));
        }
        *current_global_progress =
            (*current_global_progress).saturating_add(result.source_chunk_count);
        if options.progress_hwnd.0 != 0 {
            unsafe {
                if let Err(err) = PostMessageW(
                    options.progress_hwnd,
                    crate::WM_UPDATE_PROGRESS,
                    WPARAM(*current_global_progress),
                    LPARAM(0),
                ) {
                    crate::log_debug(&format!(
                        "Failed to post Google parallel WM_UPDATE_PROGRESS: {}",
                        err
                    ));
                }
            }
        }
    }

    for handle in handles {
        if handle.join().is_err() {
            crate::log_debug("Google audiobook worker panicked");
        }
    }

    let cleanup_ordered_results = |results: &mut [OrderedGoogleAudiobookResult]| {
        for result in results.iter_mut() {
            if let Some(Ok(Some(path))) = result.take() {
                crate::log_if_err!(std::fs::remove_file(path));
            }
        }
    };

    if options.cancel.load(Ordering::Relaxed) {
        cleanup_ordered_results(&mut ordered_results);
        return Err(cancelled_message(options.language));
    }
    if received_results != expected_results || ordered_results.iter().any(Option::is_none) {
        let missing_ordered_results = ordered_results
            .iter()
            .filter(|entry| entry.is_none())
            .count();
        cleanup_ordered_results(&mut ordered_results);
        crate::log_debug(&format!(
            "Google audiobook workers stopped early: received_results={} expected_results={} missing_ordered_results={}",
            received_results, expected_results, missing_ordered_results
        ));
        return Err(i18n::tr(
            options.language,
            "audiobook.google_workers_stopped_early",
        ));
    }
    if let Some(error) = ordered_results.iter().find_map(|entry| match entry {
        Some(Err(error)) => Some(error.clone()),
        _ => None,
    }) {
        cleanup_ordered_results(&mut ordered_results);
        crate::log_debug(&format!(
            "Google audiobook render aborted because a synthesis unit failed; no unit was omitted: {}",
            error
        ));
        return Err(error);
    }

    for result in ordered_results {
        if let Some(Ok(Some(path))) = result {
            temp_wavs.push(path);
        }
    }
    Ok(())
}

const MIXED_EDGE_PARALLELISM: usize = 8;

fn mixed_edge_batch_parallelism(engine: TtsEngine) -> usize {
    match engine {
        TtsEngine::Edge => MIXED_EDGE_PARALLELISM,
        TtsEngine::Google | TtsEngine::Sapi5 | TtsEngine::Sapi4 => 1,
    }
}

pub(crate) fn render_mixed_audiobook_part(
    chunks: &[TtsChunk],
    current_global_progress: &mut usize,
    output: &Path,
    options: &AudiobookCommonOptions,
    config: &MixedAudiobookConfig,
) -> Result<(), String> {
    if chunks.is_empty() {
        return Ok(());
    }
    let render_started = Instant::now();
    crate::log_debug(&format!(
        "Mixed audiobook render start: chunks={} output={:?} main_engine={} rate={} pitch={} volume={}",
        chunks.len(),
        output,
        match config.main_engine {
            TtsEngine::Edge => "edge",
            TtsEngine::Google => "google",
            TtsEngine::Sapi5 => "sapi5",
            TtsEngine::Sapi4 => "sapi4",
        },
        options.rate,
        options.pitch,
        options.volume
    ));
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let extension = output
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let is_aac = extension == "m4b" || extension == "m4a" || extension == "mp4";
    let is_mp3 = extension == "mp3";
    let actual_output = if is_aac || is_mp3 {
        output.with_extension("wav.tmp")
    } else {
        output.to_path_buf()
    };

    let target = TargetAudio {
        sample_rate: 44_100,
        channels: 1,
    };
    let mut temp_wavs: Vec<PathBuf> = Vec::new();
    let edge_only = chunks.iter().all(|chunk| {
        let (engine, _, _, _, _) = resolve_mixed_chunk_synth(chunk, options, config);
        engine == TtsEngine::Edge
    });

    if edge_only {
        let edge_parallelism = mixed_edge_batch_parallelism(TtsEngine::Edge);
        crate::log_debug(&format!(
            "Mixed audiobook: edge-only mode, parallel synthesis enabled with concurrency={}",
            edge_parallelism
        ));
        for batch_start in (0..chunks.len()).step_by(edge_parallelism) {
            if options.cancel.load(Ordering::Relaxed) {
                return Err(cancelled_message(options.language));
            }
            let batch_end = std::cmp::min(batch_start + edge_parallelism, chunks.len());
            let batch = &chunks[batch_start..batch_end];
            let results = rt.block_on(async {
                let futures = batch.iter().enumerate().map(|(offset, chunk)| {
                    let idx = batch_start + offset;
                    let (engine, voice, rate, pitch, volume) =
                        resolve_mixed_chunk_synth(chunk, options, config);
                    let synth = SynthesisConfig {
                        engine,
                        voice: voice.to_string(),
                        rate,
                        pitch,
                        volume,
                        language: options.language,
                        cancel: options.cancel.clone(),
                    };
                    async move {
                        (
                            idx,
                            synthesize_mixed_chunk_with_retry(idx, chunk, &synth, target).await,
                        )
                    }
                });
                join_all(futures).await
            });

            if let Some((failed_idx, error)) = results
                .iter()
                .find_map(|(idx, result)| result.as_ref().err().map(|error| (*idx, error.clone())))
            {
                crate::log_debug(&format!(
                    "Mixed audiobook render aborted at chunk={} because synthesis failed; no chunk was omitted: {}",
                    failed_idx, error
                ));
                for (_, result) in results {
                    if let Ok(Some(path)) = result {
                        crate::log_if_err!(std::fs::remove_file(path));
                    }
                }
                for path in temp_wavs.drain(..) {
                    crate::log_if_err!(std::fs::remove_file(path));
                }
                return Err(error);
            }

            for (_idx, wav_result) in results {
                if let Ok(Some(wav_path)) = wav_result {
                    temp_wavs.push(wav_path);
                }
                *current_global_progress += 1;
                if options.progress_hwnd.0 != 0 {
                    unsafe {
                        if let Err(e) = PostMessageW(
                            options.progress_hwnd,
                            crate::WM_UPDATE_PROGRESS,
                            WPARAM(*current_global_progress),
                            LPARAM(0),
                        ) {
                            crate::log_debug(&format!("Failed to post WM_UPDATE_PROGRESS: {}", e));
                        }
                    }
                }
            }
        }
    } else {
        let units = coalesce_google_audiobook_chunks(chunks, options, config);
        let google_units = units
            .iter()
            .filter(|unit| {
                mixed_synth_key(&unit.chunk, options, config).engine == TtsEngine::Google
            })
            .count();
        crate::log_debug(&format!(
            "Mixed audiobook: Google batching input_chunks={} synthesis_units={} google_units={} max_chars={}",
            chunks.len(),
            units.len(),
            google_units,
            GOOGLE_AUDIOBOOK_BATCH_MAX_CHARS
        ));
        let google_only = google_units == units.len();
        if google_only && units.len() > 1 {
            render_google_audiobook_units_parallel(
                units,
                &mut temp_wavs,
                current_global_progress,
                options,
                config,
                target,
            )?;
        } else {
            for unit in units {
                if options.cancel.load(Ordering::Relaxed) {
                    return Err(cancelled_message(options.language));
                }
                let chunk = &unit.chunk;
                let (engine, voice, rate, pitch, volume) =
                    resolve_mixed_chunk_synth(chunk, options, config);
                let synth = SynthesisConfig {
                    engine,
                    voice: voice.to_string(),
                    rate,
                    pitch,
                    volume,
                    language: options.language,
                    cancel: options.cancel.clone(),
                };
                let unit_started = Instant::now();
                let wav_result = rt.block_on(synthesize_mixed_chunk_with_retry(
                    unit.first_chunk_index,
                    chunk,
                    &synth,
                    target,
                ));
                let produced = matches!(&wav_result, Ok(Some(_)));
                crate::log_debug(&format!(
                    "Mixed audiobook unit complete: first_chunk={} source_chunks={} engine={} text_chars={} elapsed_ms={} produced={} failed={}",
                    unit.first_chunk_index,
                    unit.source_chunk_count,
                    match engine {
                        TtsEngine::Edge => "edge",
                        TtsEngine::Google => "google",
                        TtsEngine::Sapi5 => "sapi5",
                        TtsEngine::Sapi4 => "sapi4",
                    },
                    chunk.text_to_read.chars().count(),
                    unit_started.elapsed().as_millis(),
                    produced,
                    wav_result.is_err()
                ));
                match wav_result {
                    Ok(Some(wav_path)) => temp_wavs.push(wav_path),
                    Ok(None) => {}
                    Err(error) => {
                        crate::log_debug(&format!(
                            "Mixed audiobook render aborted at first_chunk={} because synthesis failed; no chunk was omitted: {}",
                            unit.first_chunk_index, error
                        ));
                        for path in temp_wavs.drain(..) {
                            crate::log_if_err!(std::fs::remove_file(path));
                        }
                        return Err(error);
                    }
                }
                *current_global_progress =
                    (*current_global_progress).saturating_add(unit.source_chunk_count);
                if options.progress_hwnd.0 != 0 {
                    unsafe {
                        if let Err(e) = PostMessageW(
                            options.progress_hwnd,
                            crate::WM_UPDATE_PROGRESS,
                            WPARAM(*current_global_progress),
                            LPARAM(0),
                        ) {
                            crate::log_debug(&format!("Failed to post WM_UPDATE_PROGRESS: {}", e));
                        }
                    }
                }
            }
        }
    }

    if temp_wavs.is_empty() {
        return Err(i18n::tr(options.language, "audiobook.no_valid_segments"));
    }
    crate::log_debug(&format!(
        "Mixed audiobook synthesis phase complete: wav_segments={} elapsed_ms={}",
        temp_wavs.len(),
        render_started.elapsed().as_millis()
    ));
    let join_started = Instant::now();
    crate::audio_utils::join_wav_files(&temp_wavs, &actual_output).map_err(|e| e.to_string())?;
    crate::log_debug(&format!(
        "Mixed audiobook WAV join complete: segments={} elapsed_ms={} output={:?}",
        temp_wavs.len(),
        join_started.elapsed().as_millis(),
        actual_output
    ));
    for path in temp_wavs {
        if let Err(e) = std::fs::remove_file(&path) {
            crate::log_debug(&format!("Failed to remove temp wav {:?}: {}", path, e));
        }
    }

    if is_aac {
        let conversion_started = Instant::now();
        let settings = crate::ffmpeg_export::ConvertAudioSettings {
            format: crate::ffmpeg_export::ConvertAudioFormat::Aac,
            quality: crate::ffmpeg_export::ConvertAudioQuality::BitrateKbps(
                options.audiobook_bitrate_kbps,
            ),
        };
        let mut progress = |_p: u32| {};
        let res = crate::ffmpeg_export::convert_audio_file(
            &actual_output,
            output,
            &settings,
            None,
            Some(&mut progress),
        );
        std::fs::remove_file(&actual_output).ok();
        res?;
        crate::log_debug(&format!(
            "Mixed audiobook AAC conversion complete: elapsed_ms={} output={:?}",
            conversion_started.elapsed().as_millis(),
            output
        ));
    } else if is_mp3 {
        let conversion_started = Instant::now();
        let settings = crate::ffmpeg_export::ConvertAudioSettings {
            format: crate::ffmpeg_export::ConvertAudioFormat::Mp3,
            quality: crate::ffmpeg_export::ConvertAudioQuality::BitrateKbps(
                options.audiobook_bitrate_kbps,
            ),
        };
        let mut progress = |_p: u32| {};
        let res = crate::ffmpeg_export::convert_audio_file(
            &actual_output,
            output,
            &settings,
            None,
            Some(&mut progress),
        );
        std::fs::remove_file(&actual_output).ok();
        res?;
        crate::log_debug(&format!(
            "Mixed audiobook MP3 conversion complete: elapsed_ms={} output={:?}",
            conversion_started.elapsed().as_millis(),
            output
        ));
    }

    crate::log_debug(&format!(
        "Mixed audiobook render complete: total_elapsed_ms={} output={:?}",
        render_started.elapsed().as_millis(),
        output
    ));
    Ok(())
}

#[cfg(test)]
mod sapi5_adaptive_recovery_tests {
    use super::*;

    #[test]
    fn interrupted_worker_limit_moves_to_the_next_lower_level() {
        assert_eq!(sapi5_next_lower_worker_limit(12), 8);
        assert_eq!(sapi5_next_lower_worker_limit(8), 6);
        assert_eq!(sapi5_next_lower_worker_limit(6), 4);
        assert_eq!(sapi5_next_lower_worker_limit(4), 2);
        assert_eq!(sapi5_next_lower_worker_limit(2), 1);
        assert_eq!(sapi5_next_lower_worker_limit(1), 1);
    }

    #[test]
    fn diagnostic_field_parser_reads_quoted_and_plain_values() {
        let line = r#"SESSION_START id=123 voice="Code Factory Vocalizer Paola (Italiano) - Embedded Pro" initial_limit=12"#;
        assert_eq!(sapi5_extract_log_field(line, "id"), Some("123"));
        assert_eq!(
            sapi5_extract_log_field(line, "voice"),
            Some("Code Factory Vocalizer Paola (Italiano) - Embedded Pro")
        );
        assert_eq!(sapi5_extract_log_field(line, "initial_limit"), Some("12"));
    }

    #[test]
    fn isolated_worker_protocol_uses_a_fixed_short_directory() {
        let directory = sapi5_worker_protocol_dir();
        assert_eq!(
            directory.file_name().and_then(|name| name.to_str()),
            Some("sonarpad_sapi5_workers")
        );
        assert!(
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.len() <= 32)
        );
    }

    #[test]
    fn isolated_worker_stall_timeout_stays_within_safe_bounds() {
        let short = vec!["testo breve".to_string()];
        assert_eq!(
            sapi5_worker_stall_timeout(&short, 0),
            Duration::from_secs(SAPI5_WORKER_MIN_STALL_TIMEOUT_SECS)
        );

        let very_long = vec!["a".repeat(20_000)];
        assert_eq!(
            sapi5_worker_stall_timeout(&very_long, -100),
            Duration::from_secs(SAPI5_WORKER_MAX_STALL_TIMEOUT_SECS)
        );
    }
}

#[cfg(test)]
mod sapi4_worker_tests {
    use super::*;

    #[test]
    fn sapi4_user_worker_limit_is_respected_up_to_sixty_four() {
        assert_eq!(requested_sapi4_worker_limit(0), 1);
        assert_eq!(requested_sapi4_worker_limit(1), 1);
        assert_eq!(requested_sapi4_worker_limit(30), 30);
        assert_eq!(requested_sapi4_worker_limit(64), 64);
        assert_eq!(requested_sapi4_worker_limit(100), 64);
    }

    #[test]
    fn sapi4_retry_worker_limit_decreases_progressively() {
        assert_eq!(sapi4_next_lower_worker_limit(64), 48);
        assert_eq!(sapi4_next_lower_worker_limit(48), 32);
        assert_eq!(sapi4_next_lower_worker_limit(30), 20);
        assert_eq!(sapi4_next_lower_worker_limit(20), 12);
        assert_eq!(sapi4_next_lower_worker_limit(12), 8);
        assert_eq!(sapi4_next_lower_worker_limit(8), 4);
        assert_eq!(sapi4_next_lower_worker_limit(4), 2);
        assert_eq!(sapi4_next_lower_worker_limit(2), 1);
        assert_eq!(sapi4_next_lower_worker_limit(1), 1);
    }
}

#[cfg(test)]
mod google_audiobook_optimization_tests {
    use super::*;

    #[test]
    fn google_batch_splitter_preserves_unicode_sentence_boundaries() {
        let sentence = "Titolo accentato: È una domanda? Sì, è una risposta! Questa è la fine. ";
        let text = sentence.repeat(80);
        let parts = split_google_audiobook_text(&text, GOOGLE_AUDIOBOOK_BATCH_MAX_CHARS);

        assert!(parts.len() > 1);
        assert!(
            parts
                .iter()
                .all(|part| part.chars().count() <= GOOGLE_AUDIOBOOK_BATCH_MAX_CHARS)
        );
        assert!(parts.iter().all(|part| {
            part.ends_with('.')
                || part.ends_with(':')
                || part.ends_with(';')
                || part.ends_with('?')
                || part.ends_with('!')
        }));
        assert_eq!(
            parts
                .iter()
                .flat_map(|part| part.split_whitespace())
                .collect::<Vec<_>>(),
            text.split_whitespace().collect::<Vec<_>>()
        );
    }

    #[test]
    fn google_batch_splitter_emergency_splits_an_exceptionally_long_sentence() {
        let text = format!("{}.", "parola ".repeat(300));
        let parts = split_google_audiobook_text(&text, GOOGLE_AUDIOBOOK_BATCH_MAX_CHARS);

        assert!(parts.len() > 1);
        assert!(
            parts
                .iter()
                .all(|part| part.chars().count() <= GOOGLE_AUDIOBOOK_BATCH_MAX_CHARS)
        );
        assert_eq!(
            parts
                .iter()
                .flat_map(|part| part.split_whitespace())
                .collect::<Vec<_>>(),
            text.split_whitespace().collect::<Vec<_>>()
        );
    }

    #[test]
    fn google_batch_splitter_uses_uppercase_newline_even_after_hyphen() {
        let first_line = format!("{}-", "Prima parte molto lunga ".repeat(30));
        let second_line = format!("{}.", "Nuova riga maiuscola ".repeat(20));
        let text = format!("{first_line}\n{second_line}");

        let parts = split_google_audiobook_text(&text, GOOGLE_AUDIOBOOK_BATCH_MAX_CHARS);

        assert!(parts.len() >= 2);
        assert!(parts[0].ends_with('-'));
        assert!(parts[1].starts_with("Nuova"));
        assert!(
            parts
                .iter()
                .all(|part| part.chars().count() <= GOOGLE_AUDIOBOOK_BATCH_MAX_CHARS)
        );
    }

    #[test]
    fn oversized_google_chunk_is_split_without_overcounting_progress() {
        let text = "Una frase italiana termina chiaramente qui. ".repeat(250);
        let chunk = TtsChunk {
            original_len: utf16_len(&text),
            text_to_read: text.clone(),
            override_voice: None,
            pause_ms: None,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let options = AudiobookCommonOptions {
            voice: "google-test",
            output: Path::new("test.wav"),
            progress_hwnd: HWND(0),
            cancel,
            language: Language::Italian,
            part_naming_mode: AudiobookPartNamingMode::TitleNumber,
            part_announcement_mode: AudiobookPartAnnouncementMode::None,
            audiobook_title: "",
            audiobook_bitrate_kbps: 128,
            rate: 0,
            pitch: 0,
            volume: 100,
            sapi4_threads: None,
        };
        let config = MixedAudiobookConfig {
            main_engine: TtsEngine::Google,
        };

        let units = coalesce_google_audiobook_chunks(&[chunk], &options, &config);

        assert!(units.len() > 1);
        assert!(units.iter().all(|unit| {
            unit.chunk.text_to_read.chars().count() <= GOOGLE_AUDIOBOOK_BATCH_MAX_CHARS
        }));
        assert!(
            units
                .iter()
                .all(|unit| unit.chunk.text_to_read.ends_with('.'))
        );
        assert_eq!(
            units
                .iter()
                .map(|unit| unit.source_chunk_count)
                .sum::<usize>(),
            1
        );
        let original_words = text.split_whitespace().collect::<Vec<_>>();
        let split_words = units
            .iter()
            .flat_map(|unit| unit.chunk.text_to_read.split_whitespace())
            .collect::<Vec<_>>();
        assert_eq!(split_words, original_words);
    }

    #[test]
    fn google_batching_reassembles_sentences_split_across_source_chunks() {
        let chunks = [
            TtsChunk {
                original_len: 12,
                text_to_read: "Questa frase".to_string(),
                override_voice: None,
                pause_ms: None,
            },
            TtsChunk {
                original_len: 31,
                text_to_read: "continua e termina qui. Seconda".to_string(),
                override_voice: None,
                pause_ms: None,
            },
            TtsChunk {
                original_len: 15,
                text_to_read: "frase conclusa!".to_string(),
                override_voice: None,
                pause_ms: None,
            },
        ];
        let options = AudiobookCommonOptions {
            voice: "google-test",
            output: Path::new("test.wav"),
            progress_hwnd: HWND(0),
            cancel: Arc::new(AtomicBool::new(false)),
            language: Language::Italian,
            part_naming_mode: AudiobookPartNamingMode::TitleNumber,
            part_announcement_mode: AudiobookPartAnnouncementMode::None,
            audiobook_title: "",
            audiobook_bitrate_kbps: 128,
            rate: 0,
            pitch: 0,
            volume: 100,
            sapi4_threads: None,
        };
        let config = MixedAudiobookConfig {
            main_engine: TtsEngine::Google,
        };

        let units = coalesce_google_audiobook_chunks(&chunks, &options, &config);

        assert_eq!(units.len(), 1);
        assert_eq!(
            units[0]
                .chunk
                .text_to_read
                .split_whitespace()
                .collect::<Vec<_>>(),
            "Questa frase continua e termina qui. Seconda frase conclusa!"
                .split_whitespace()
                .collect::<Vec<_>>()
        );
        assert!(units[0].chunk.text_to_read.contains('\n'));
        assert!(units[0].chunk.text_to_read.ends_with('!'));
        assert_eq!(units[0].source_chunk_count, chunks.len());
    }

    #[test]
    fn audiobook_retry_policy_never_drops_transient_google_failures() {
        let transient = "Impossibile accedere al file. Il file è utilizzato da un altro processo. (os error 32)";
        assert!(should_retry_audiobook_segment(
            TtsEngine::Google,
            1,
            transient
        ));
        assert!(should_retry_audiobook_segment(
            TtsEngine::Google,
            500,
            transient
        ));
        assert!(should_retry_audiobook_segment(
            TtsEngine::Google,
            500,
            "Google TTS produced no audio."
        ));
        assert!(should_retry_audiobook_segment(
            TtsEngine::Edge,
            NON_GOOGLE_AUDIOBOOK_MAX_ATTEMPTS - 1,
            "temporary failure"
        ));
        assert!(should_retry_audiobook_segment(
            TtsEngine::Edge,
            500,
            "temporary WebSocket timeout"
        ));
        assert!(!should_retry_audiobook_segment(
            TtsEngine::Edge,
            NON_GOOGLE_AUDIOBOOK_MAX_ATTEMPTS,
            "invalid voice configuration"
        ));
        assert!(!should_retry_audiobook_segment(
            TtsEngine::Sapi5,
            NON_GOOGLE_AUDIOBOOK_MAX_ATTEMPTS,
            "temporary failure"
        ));
        assert!(!should_retry_audiobook_segment(
            TtsEngine::Sapi4,
            NON_GOOGLE_AUDIOBOOK_MAX_ATTEMPTS,
            "temporary failure"
        ));
    }

    #[test]
    fn edge_audiobook_transient_failures_retry_until_user_cancellation() {
        for error in [
            "WebSocket timeout",
            "429 Too Many Requests",
            "503 Service Unavailable",
            "connection reset by peer",
            "connection refused",
            "Edge WS: no audio sent",
            "Edge audiobook: audio decode failed: invalid frame",
        ] {
            assert!(
                is_edge_audiobook_transient_error(error),
                "expected transient Edge audiobook error: {error}"
            );
            assert!(should_retry_audiobook_segment(TtsEngine::Edge, 500, error));
        }
        assert!(!is_edge_audiobook_transient_error(
            "invalid voice configuration"
        ));
        assert!(!is_edge_audiobook_transient_error(
            "invalid connection configuration"
        ));
    }

    #[test]
    fn edge_time_split_filters_only_chunks_without_speakable_text() {
        let chunks = vec!["...?!".to_string(), "Testo da sintetizzare.".to_string()];
        let filtered = filter_time_split_chunks(&chunks, TtsEngine::Edge);
        assert_eq!(filtered, vec!["Testo da sintetizzare.".to_string()]);
    }

    #[test]
    fn google_audiobook_permanent_configuration_errors_are_not_retried_forever() {
        assert!(is_permanent_google_audiobook_error(
            "The selected Google TTS voice is not installed."
        ));
        assert!(is_permanent_google_audiobook_error(
            "Google Chrome and Microsoft Edge were not found."
        ));
        assert!(!is_permanent_google_audiobook_error(
            "Impossibile accedere al file. Il file è utilizzato da un altro processo. (os error 32)"
        ));
        assert!(!is_permanent_google_audiobook_error(
            "Google TTS produced no audio."
        ));
    }

    #[test]
    fn google_audiobook_retry_delay_grows_and_is_capped() {
        assert_eq!(audiobook_retry_delay(1), Duration::from_millis(250));
        assert_eq!(audiobook_retry_delay(5), Duration::from_millis(1_250));
        assert_eq!(audiobook_retry_delay(20), Duration::from_secs(5));
        assert_eq!(audiobook_retry_delay(100), Duration::from_secs(5));
    }

    #[test]
    fn google_batches_keep_distinct_source_chunk_positions() {
        let chunks = (0..45)
            .map(|index| {
                let text = format!(
                    "Frase numero {index} sufficientemente lunga per verificare la suddivisione del testo."
                );
                TtsChunk {
                    original_len: utf16_len(&text),
                    text_to_read: text,
                    override_voice: None,
                    pause_ms: None,
                }
            })
            .collect::<Vec<_>>();
        let options = AudiobookCommonOptions {
            voice: "google-test",
            output: Path::new("test.wav"),
            progress_hwnd: HWND(0),
            cancel: Arc::new(AtomicBool::new(false)),
            language: Language::Italian,
            part_naming_mode: AudiobookPartNamingMode::TitleNumber,
            part_announcement_mode: AudiobookPartAnnouncementMode::None,
            audiobook_title: "",
            audiobook_bitrate_kbps: 128,
            rate: 0,
            pitch: 0,
            volume: 100,
            sapi4_threads: None,
        };
        let config = MixedAudiobookConfig {
            main_engine: TtsEngine::Google,
        };

        let units = coalesce_google_audiobook_chunks(&chunks, &options, &config);

        assert!(units.len() > 1);
        let mut expected_first_chunk = 0usize;
        for unit in &units {
            assert_eq!(unit.first_chunk_index, expected_first_chunk);
            expected_first_chunk = expected_first_chunk.saturating_add(unit.source_chunk_count);
        }
        assert_eq!(expected_first_chunk, chunks.len());
    }

    #[test]
    fn google_worker_limit_scales_on_modern_cpus() {
        assert_eq!(google_audiobook_worker_limit_for_cores(1), 1);
        assert_eq!(google_audiobook_worker_limit_for_cores(4), 3);
        assert_eq!(google_audiobook_worker_limit_for_cores(8), 5);
        assert_eq!(google_audiobook_worker_limit_for_cores(12), 6);
        assert_eq!(google_audiobook_worker_limit_for_cores(16), 7);
        assert_eq!(google_audiobook_worker_limit_for_cores(24), 8);
    }
}

#[cfg(test)]
mod omni_audio_description_tts_port_tests {
    use super::{
        MIXED_EDGE_PARALLELISM, is_edge_audiobook_transient_error, mixed_edge_batch_parallelism,
        should_retry_audiobook_segment,
    };
    use crate::settings::TtsEngine;

    #[test]
    fn omni_port_edge_no_audio_is_retryable_without_attempt_cap() {
        assert!(is_edge_audiobook_transient_error("Edge WS: no audio sent"));
        assert!(should_retry_audiobook_segment(
            TtsEngine::Edge,
            500,
            "Edge WS: no audio sent",
        ));
    }

    #[test]
    fn omni_port_edge_invalid_parameter_is_not_retried_after_limit() {
        assert!(!is_edge_audiobook_transient_error(
            "invalid voice parameter"
        ));
        assert!(!should_retry_audiobook_segment(
            TtsEngine::Edge,
            500,
            "invalid voice parameter",
        ));
    }

    #[test]
    fn omni_port_edge_empty_or_decode_failure_is_retryable() {
        for error in [
            "Edge TTS produced empty audio payload",
            "Edge audiobook: audio decode failed: invalid frame",
        ] {
            assert!(is_edge_audiobook_transient_error(error), "{error}");
            assert!(should_retry_audiobook_segment(TtsEngine::Edge, 500, error));
        }
    }

    #[test]
    fn omni_port_edge_parallelism_is_bounded_to_eight() {
        assert_eq!(mixed_edge_batch_parallelism(TtsEngine::Edge), 8);
        assert_eq!(MIXED_EDGE_PARALLELISM, 8);
    }

    #[test]
    fn omni_port_local_sapi_engines_do_not_use_edge_parallel_batches() {
        assert_eq!(mixed_edge_batch_parallelism(TtsEngine::Sapi5), 1);
        assert_eq!(mixed_edge_batch_parallelism(TtsEngine::Sapi4), 1);
    }
}
