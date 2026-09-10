use crate::accessibility::to_wide;
use crate::tools::rss::{RssItem, RssSource};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(not(feature = "standalone"))]
use std::ffi::OsStr;
use std::io::Write;
#[cfg(not(feature = "standalone"))]
use std::os::windows::prelude::*;
use std::path::PathBuf;
#[cfg(not(feature = "standalone"))]
use std::path::{Component, Prefix};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, RwLock};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
#[cfg(not(feature = "standalone"))]
use windows::Win32::Storage::FileSystem::GetDriveTypeW;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_F4, VK_F5, VK_F6, VK_NEXT, VK_PRIOR};
use windows::Win32::UI::Shell::{FOLDERID_Documents, SHGetKnownFolderPath};
use windows::core::PCWSTR;

#[cfg(not(feature = "standalone"))]
pub const DRIVE_REMOVABLE: u32 = 2;

pub const TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
pub const VOICE_LIST_URL: &str =
    "https://speech.platform.bing.com/consumer/speech/synthesize/readaloud/voices/list";
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-flash-latest";
pub const GEMINI_FLASH_LITE_LATEST_MODEL: &str = "gemini-flash-lite-latest";
pub const DEFAULT_AUDIO_DESCRIPTION_GEMINI_MODEL: &str = "gemini-3.5-flash-lite";
const LEGACY_DEFAULT_GEMINI_MODEL: &str = "gemini-3.5-flash";

static RAI_LUCE_CODE_CACHE: OnceLock<RwLock<Option<String>>> = OnceLock::new();
static RAI_LUCE_EXPLICIT_CLEAR_PENDING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize, Deserialize)]
pub struct VoiceInfo {
    pub short_name: String,
    pub locale: String,
    pub is_multilingual: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FavoriteVoice {
    pub engine: TtsEngine,
    pub short_name: String,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct StreamFavorite {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub url: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct SavedYtdlpSiteCredentials {
    username: String,
    password: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub original: String,
    pub replacement: String,
    #[serde(default = "default_true")]
    pub match_case: bool,
    #[serde(default)]
    pub use_custom_voice: bool,
    #[serde(default)]
    pub custom_voice_engine: Option<TtsEngine>,
    #[serde(default)]
    pub custom_voice: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AudiobookResult {
    pub success: bool,
    pub message: String,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, Debug)]
pub enum TextEncoding {
    #[serde(rename = "ansi")]
    Ansi,
    #[serde(rename = "utf8")]
    #[default]
    Utf8,
    #[serde(rename = "utf8bom")]
    Utf8Bom,
    #[serde(rename = "utf16le")]
    Utf16Le,
    #[serde(rename = "utf16be")]
    Utf16Be,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Text(TextEncoding),
    Docx,
    Doc,
    Pdf,
    Spreadsheet,
    Epub,
    KindleEbook,
    Daisy,
    Html,
    Ppt,
    Pptx,
    Odp,
    Odt,
    Audiobook,
}

impl Default for FileFormat {
    fn default() -> Self {
        FileFormat::Text(TextEncoding::Utf8)
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum OpenBehavior {
    #[serde(rename = "new_tab")]
    #[default]
    NewTab,
    #[serde(rename = "new_window")]
    NewWindow,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Language {
    #[serde(rename = "it")]
    #[default]
    Italian,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "de")]
    German,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "pt")]
    Portuguese,
    #[serde(rename = "pt-BR")]
    PortugueseBrazilian,
    #[serde(rename = "sv")]
    Swedish,
    #[serde(rename = "vi")]
    Vietnamese,
    #[serde(rename = "cs")]
    Czech,
    #[serde(rename = "pl")]
    Polish,
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "sr")]
    Serbian,
    #[serde(rename = "uk")]
    Ukrainian,
    #[serde(rename = "lt")]
    Lithuanian,
    #[serde(rename = "ru")]
    Russian,
    #[serde(rename = "zh")]
    Chinese,
    #[serde(rename = "hi")]
    Hindi,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ModifiedMarkerPosition {
    #[serde(rename = "end")]
    #[default]
    End,
    #[serde(rename = "beginning")]
    Beginning,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TtsEngine {
    #[serde(rename = "edge")]
    #[default]
    Edge,
    #[serde(rename = "sapi5")]
    Sapi5,
    #[serde(rename = "sapi4")]
    Sapi4,
    #[serde(rename = "google")]
    Google,
}

pub const DEFAULT_VOICE_PROFILE_NAME: &str = "Default";

fn default_voice_profile_name() -> String {
    DEFAULT_VOICE_PROFILE_NAME.to_string()
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct TtsTuning {
    pub rate: i32,
    pub pitch: i32,
    pub volume: i32,
}

impl TtsTuning {
    pub fn new(rate: i32, pitch: i32, volume: i32) -> Self {
        Self {
            rate,
            pitch,
            volume,
        }
    }
}

impl Default for TtsTuning {
    fn default() -> Self {
        Self {
            rate: 0,
            pitch: 0,
            volume: 100,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct VoiceProfile {
    pub name: String,
    pub tts_engine: TtsEngine,
    pub tts_voice: String,
    pub tts_only_multilingual: bool,
    pub tts_manual_tuning: bool,
    pub tts_rate: i32,
    pub tts_pitch: i32,
    pub tts_volume: i32,
    pub edge_tts_tuning: TtsTuning,
    pub google_tts_tuning: TtsTuning,
    pub sapi5_tts_tuning: TtsTuning,
    pub sapi4_tts_tuning: TtsTuning,
    pub use_dialogue_voice: bool,
    pub dialogue_tts_engine: TtsEngine,
    pub dialogue_voice: String,
    pub dialogue_voice_rate: i32,
    pub dialogue_voice_pitch: i32,
    pub dialogue_voice_volume: i32,
    pub dialogue_use_secondary_voice: bool,
    pub dialogue_secondary_tts_engine: TtsEngine,
    pub dialogue_secondary_voice: String,
    pub dialogue_secondary_voice_rate: i32,
    pub dialogue_secondary_voice_pitch: i32,
    pub dialogue_secondary_voice_volume: i32,
}

impl Default for VoiceProfile {
    fn default() -> Self {
        Self {
            name: default_voice_profile_name(),
            tts_engine: TtsEngine::Edge,
            tts_voice: "it-IT-IsabellaNeural".to_string(),
            tts_only_multilingual: false,
            tts_manual_tuning: false,
            tts_rate: 0,
            tts_pitch: 0,
            tts_volume: 100,
            edge_tts_tuning: TtsTuning::default(),
            google_tts_tuning: TtsTuning::default(),
            sapi5_tts_tuning: TtsTuning::default(),
            sapi4_tts_tuning: TtsTuning::default(),
            use_dialogue_voice: false,
            dialogue_tts_engine: TtsEngine::Edge,
            dialogue_voice: String::new(),
            dialogue_voice_rate: 0,
            dialogue_voice_pitch: 0,
            dialogue_voice_volume: 100,
            dialogue_use_secondary_voice: false,
            dialogue_secondary_tts_engine: TtsEngine::Edge,
            dialogue_secondary_voice: String::new(),
            dialogue_secondary_voice_rate: 0,
            dialogue_secondary_voice_pitch: 0,
            dialogue_secondary_voice_volume: 100,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SpellcheckLanguageMode {
    #[serde(rename = "follow")]
    #[default]
    FollowEditorLanguage,
    #[serde(rename = "fixed")]
    FixedLanguage,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum IndentationMode {
    #[serde(rename = "default")]
    #[default]
    Default,
    #[serde(rename = "tabs")]
    Tabs,
    #[serde(rename = "spaces")]
    Spaces,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PodcastFormat {
    #[serde(rename = "mp3")]
    #[default]
    Mp3,
    #[serde(rename = "wav")]
    Wav,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PodcastSystemCaptureMode {
    #[serde(rename = "all_system")]
    #[default]
    AllSystem,
    #[serde(rename = "single_app")]
    SingleApp,
    #[serde(rename = "selected_apps")]
    SelectedApps,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PodcastSearchProvider {
    #[serde(rename = "itunes")]
    #[default]
    Itunes,
    #[serde(rename = "podcastindex")]
    PodcastIndex,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SubtitleReadMode {
    #[serde(rename = "off")]
    #[default]
    Off,
    #[serde(rename = "nvda")]
    Nvda,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "sapi5")]
    Sapi5,
    #[serde(rename = "sapi4")]
    Sapi4,
    #[serde(rename = "edge")]
    Edge,
    #[serde(rename = "record")]
    Record,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RssDeleteConfirmMode {
    #[serde(rename = "feed")]
    Feed,
    #[serde(rename = "article")]
    Article,
    #[serde(rename = "both")]
    #[default]
    Both,
    #[serde(rename = "none")]
    None,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PodcastDeleteConfirmMode {
    #[serde(rename = "podcast")]
    Podcast,
    #[serde(rename = "episode")]
    Episode,
    #[serde(rename = "both")]
    #[default]
    Both,
    #[serde(rename = "none")]
    None,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RssQuickCopyMode {
    #[serde(rename = "title")]
    #[default]
    Title,
    #[serde(rename = "url")]
    Url,
    #[serde(rename = "content")]
    Content,
    #[serde(rename = "all")]
    All,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RssPodcastUnreadLabelPosition {
    #[serde(rename = "before")]
    #[default]
    Before,
    #[serde(rename = "after")]
    After,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ListDateDisplayMode {
    #[serde(rename = "always")]
    #[default]
    Always,
    #[serde(rename = "never")]
    Never,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ListTimeDisplayMode {
    #[serde(rename = "always")]
    Always,
    #[serde(rename = "never")]
    Never,
    #[serde(rename = "only_if_multiple_same_day")]
    #[default]
    OnlyIfMultipleSameDay,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AudiobookPartNamingMode {
    #[serde(rename = "title_number")]
    #[default]
    TitleNumber,
    #[serde(rename = "number_only")]
    NumberOnly,
    #[serde(rename = "number_title")]
    NumberTitle,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum AudiobookPartAnnouncementMode {
    #[serde(rename = "none")]
    #[default]
    None = 0,
    #[serde(rename = "title")]
    Title = 1,
    #[serde(rename = "title_part_number")]
    TitlePartNumber = 2,
    #[serde(rename = "file_name")]
    FileName = 3,
    #[serde(rename = "file_name_part_number")]
    FileNamePartNumber = 4,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShortcutBinding {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: u16,
}

impl ShortcutBinding {
    pub const fn new(ctrl: bool, shift: bool, alt: bool, key: u16) -> Self {
        Self {
            ctrl,
            shift,
            alt,
            key,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ShortcutSettings {
    pub read_pause_resume: ShortcutBinding,
    pub read_start: ShortcutBinding,
    pub read_previous_sentence: ShortcutBinding,
    pub read_next_sentence: ShortcutBinding,
    pub read_stop: ShortcutBinding,
    pub execute_file: ShortcutBinding,
    pub audiobook: ShortcutBinding,
    pub batch_audiobooks: ShortcutBinding,
    pub record_podcast: ShortcutBinding,
    pub dictation: ShortcutBinding,
    pub convert_audio: ShortcutBinding,
    pub open_rss: ShortcutBinding,
    pub open_podcasts: ShortcutBinding,
    pub open_paths_navigation: ShortcutBinding,
    pub open_radio: ShortcutBinding,
    pub open_calendar: ShortcutBinding,
    pub open_weather: ShortcutBinding,
    pub open_cinema: ShortcutBinding,
    pub open_dictionary: ShortcutBinding,
    pub open_options: ShortcutBinding,
    pub open_terminal: ShortcutBinding,
    pub import_wikipedia: ShortcutBinding,
    pub import_youtube: ShortcutBinding,
    pub find: ShortcutBinding,
    pub quote_lines: ShortcutBinding,
    pub unquote_lines: ShortcutBinding,
    pub media_prev: ShortcutBinding,
    pub media_next: ShortcutBinding,
    pub chapter_prev: ShortcutBinding,
    pub chapter_next: ShortcutBinding,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            read_pause_resume: ShortcutBinding::new(false, false, false, VK_F4.0),
            read_start: ShortcutBinding::new(false, false, false, VK_F5.0),
            read_previous_sentence: ShortcutBinding::new(false, true, false, VK_F5.0),
            read_next_sentence: ShortcutBinding::new(true, false, false, VK_F5.0),
            read_stop: ShortcutBinding::new(false, false, false, VK_F6.0),
            execute_file: ShortcutBinding::new(true, true, false, VK_F5.0),
            audiobook: ShortcutBinding::new(true, false, false, 'R' as u16),
            batch_audiobooks: ShortcutBinding::new(true, true, false, 'B' as u16),
            record_podcast: ShortcutBinding::new(true, true, false, 'R' as u16),
            dictation: ShortcutBinding::new(true, true, false, 0x20),
            convert_audio: ShortcutBinding::new(true, true, false, 'A' as u16),
            open_rss: ShortcutBinding::new(true, true, false, 'U' as u16),
            open_podcasts: ShortcutBinding::new(true, true, false, 'P' as u16),
            open_paths_navigation: ShortcutBinding::new(false, true, true, 'N' as u16),
            open_radio: ShortcutBinding::new(false, true, true, 'R' as u16),
            open_calendar: ShortcutBinding::new(true, true, false, 'K' as u16),
            open_weather: ShortcutBinding::new(false, true, true, 'M' as u16),
            open_cinema: ShortcutBinding::new(false, true, true, 'F' as u16),
            open_dictionary: ShortcutBinding::new(true, true, false, 'D' as u16),
            open_options: ShortcutBinding::new(true, true, false, 'O' as u16),
            open_terminal: ShortcutBinding::new(true, true, false, 'T' as u16),
            import_wikipedia: ShortcutBinding::new(false, true, true, 'W' as u16),
            import_youtube: ShortcutBinding::new(true, false, false, 'Y' as u16),
            find: ShortcutBinding::new(true, false, false, 'F' as u16),
            quote_lines: ShortcutBinding::new(true, false, false, 'Q' as u16),
            unquote_lines: ShortcutBinding::new(true, true, false, 'Q' as u16),
            media_prev: ShortcutBinding::new(true, false, false, VK_PRIOR.0),
            media_next: ShortcutBinding::new(true, false, false, VK_NEXT.0),
            chapter_prev: ShortcutBinding::new(true, false, true, VK_PRIOR.0),
            chapter_next: ShortcutBinding::new(true, false, true, VK_NEXT.0),
        }
    }
}

pub fn format_shortcut(binding: ShortcutBinding) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if binding.ctrl {
        parts.push("Ctrl");
    }
    if binding.alt {
        parts.push("Alt");
    }
    if binding.shift {
        parts.push("Shift");
    }
    parts.push(shortcut_key_name(binding.key));
    parts.join("+")
}

fn shortcut_key_name(key: u16) -> &'static str {
    match key {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x1B => "Esc",
        0x20 => "Space",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x2D => "Insert",
        0x2E => "Delete",
        0x70 => "F1",
        0x71 => "F2",
        0x72 => "F3",
        0x73 => "F4",
        0x74 => "F5",
        0x75 => "F6",
        0x76 => "F7",
        0x77 => "F8",
        0x78 => "F9",
        0x79 => "F10",
        0x7A => "F11",
        0x7B => "F12",
        _ => {
            if (b'0' as u16..=b'9' as u16).contains(&key)
                || (b'A' as u16..=b'Z' as u16).contains(&key)
            {
                return match key as u8 as char {
                    '0' => "0",
                    '1' => "1",
                    '2' => "2",
                    '3' => "3",
                    '4' => "4",
                    '5' => "5",
                    '6' => "6",
                    '7' => "7",
                    '8' => "8",
                    '9' => "9",
                    'A' => "A",
                    'B' => "B",
                    'C' => "C",
                    'D' => "D",
                    'E' => "E",
                    'F' => "F",
                    'G' => "G",
                    'H' => "H",
                    'I' => "I",
                    'J' => "J",
                    'K' => "K",
                    'L' => "L",
                    'M' => "M",
                    'N' => "N",
                    'O' => "O",
                    'P' => "P",
                    'Q' => "Q",
                    'R' => "R",
                    'S' => "S",
                    'T' => "T",
                    'U' => "U",
                    'V' => "V",
                    'W' => "W",
                    'X' => "X",
                    'Y' => "Y",
                    'Z' => "Z",
                    _ => "Key",
                };
            }
            "Key"
        }
    }
}

pub const PODCAST_DEVICE_DEFAULT: &str = "default";
#[derive(Clone, Serialize, Deserialize)]
pub struct RadioFavorite {
    pub language_code: String,
    pub name: String,
    pub stream_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WeatherCity {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub admin1: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub latitude: f64,
    #[serde(default)]
    pub longitude: f64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum WeatherTemperatureUnit {
    #[serde(rename = "celsius")]
    #[default]
    Celsius,
    #[serde(rename = "fahrenheit")]
    Fahrenheit,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TvFavorite {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub dash_url: Option<String>,
    pub category: String,
    #[serde(default)]
    pub stream_resolver: Option<String>,
    #[serde(default)]
    pub resolver_endpoint: Option<String>,
    #[serde(default)]
    pub resolver_realm: Option<String>,
    #[serde(default)]
    pub resolver_channel_id: Option<String>,
    #[serde(default)]
    pub tvg_id: String,
    #[serde(default)]
    pub tvg_name: String,
    #[serde(default)]
    pub http_user_agent: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub open_behavior: OpenBehavior,
    pub language: Language,
    pub modified_marker_position: ModifiedMarkerPosition,
    pub tts_engine: TtsEngine,
    pub tts_voice: String,
    #[serde(default)]
    pub use_dialogue_voice: bool,
    #[serde(default)]
    pub dialogue_voice: String,
    #[serde(default)]
    pub dialogue_use_secondary_voice: bool,
    #[serde(default)]
    pub dialogue_secondary_voice: String,
    #[serde(default = "default_dialogue_tts_engine")]
    pub dialogue_secondary_tts_engine: TtsEngine,
    #[serde(default = "default_dialogue_voice_rate")]
    pub dialogue_secondary_voice_rate: i32,
    #[serde(default = "default_dialogue_voice_pitch")]
    pub dialogue_secondary_voice_pitch: i32,
    #[serde(default = "default_dialogue_voice_volume")]
    pub dialogue_secondary_voice_volume: i32,
    #[serde(default = "default_dialogue_voice_rate")]
    pub dialogue_voice_rate: i32,
    #[serde(default = "default_dialogue_voice_pitch")]
    pub dialogue_voice_pitch: i32,
    #[serde(default = "default_dialogue_voice_volume")]
    pub dialogue_voice_volume: i32,
    #[serde(default = "default_dialogue_tts_engine")]
    pub dialogue_tts_engine: TtsEngine,
    #[serde(default = "default_dialogue_opening_quote")]
    pub dialogue_opening_quote: String,
    #[serde(default = "default_dialogue_closing_quote")]
    pub dialogue_closing_quote: String,
    #[serde(default)]
    pub dialogue_allow_multiline: bool,
    pub tts_only_multilingual: bool,
    pub tts_manual_tuning: bool,
    pub split_on_newline: bool,
    pub word_wrap: bool,
    #[serde(default = "default_true")]
    pub group_tools_menu_by_category: bool,
    #[serde(default = "default_true")]
    pub show_video_during_playback: bool,
    #[serde(default)]
    pub dark_mode: bool,
    pub wrap_width: u32,
    pub smart_quotes: bool,
    #[serde(default)]
    pub strip_markdown_keep_bullets: bool,
    pub quote_prefix: String,
    #[serde(default)]
    pub indentation_mode: IndentationMode,
    #[serde(default = "default_indent_tab_width")]
    pub indent_tab_width: u32,
    #[serde(default = "default_indent_space_width")]
    pub indent_space_width: u32,
    pub move_cursor_during_reading: bool,
    pub audiobook_skip_seconds: u32,
    pub audiobook_playback_speed: f32,
    pub audiobook_playback_pitch: f32,
    pub audiobook_playback_volume: f32,
    pub audiobook_m4b_bitrate: u32,
    pub audiobook_split: u32,
    pub audiobook_split_by_text: bool,
    pub audiobook_split_text: String,
    pub audiobook_split_text_requires_newline: bool,
    #[serde(default)]
    pub audiobook_split_by_epub_chapter: bool,
    #[serde(default)]
    pub audiobook_split_by_time: bool,
    #[serde(default = "default_audiobook_split_minutes")]
    pub audiobook_split_minutes: u32,
    #[serde(default = "default_audiobook_split_start_number")]
    pub audiobook_split_start_number: u32,
    #[serde(default)]
    pub audiobook_part_naming_mode: AudiobookPartNamingMode,
    #[serde(default)]
    pub audiobook_part_announcement_mode: AudiobookPartAnnouncementMode,
    #[serde(default)]
    pub subtitle_read_mode: SubtitleReadMode,
    #[serde(default)]
    pub subtitle_offset_ms: i32,
    #[serde(default)]
    pub subtitle_mix_export_on_play: bool,
    #[serde(default)]
    pub subtitle_mix_ducking: bool,
    pub podcast_include_microphone: bool,
    pub podcast_microphone_device_id: String,
    pub podcast_microphone_gain: f32,
    #[serde(default = "default_podcast_device_id")]
    pub dictation_microphone_device_id: String,
    pub podcast_include_system_audio: bool,
    #[serde(default)]
    pub podcast_split_sources: bool,
    pub podcast_system_device_id: String,
    pub podcast_system_gain: f32,
    #[serde(default)]
    pub podcast_system_capture_mode: PodcastSystemCaptureMode,
    #[serde(default)]
    pub podcast_include_single_app: bool,
    #[serde(default)]
    pub podcast_single_app_pid: u32,
    #[serde(default)]
    pub podcast_selected_app_pids: Vec<u32>,
    #[serde(default)]
    pub podcast_show_inactive_apps: bool,
    pub podcast_output_format: PodcastFormat,
    pub podcast_mp3_bitrate: u32,
    pub podcast_save_folder: String,
    #[serde(default)]
    pub radio_save_folder: String,
    #[serde(default)]
    pub tv_save_folder: String,
    #[serde(default = "default_audiobook_save_folder")]
    pub audiobook_save_folder: String,
    #[serde(default = "default_audio_description_save_folder")]
    pub audio_description_save_folder: String,
    #[serde(default)]
    pub audio_description_recent_project_folders: Vec<String>,
    #[serde(default = "default_media_save_folder")]
    pub media_save_folder: String,
    #[serde(default = "default_documents_save_folder")]
    pub documents_save_folder: String,
    pub podcast_include_video: bool,
    pub podcast_monitor_id: String,
    pub podcast_cache_limit_mb: u32,
    #[serde(default = "default_true")]
    pub show_media_save_confirmation: bool,
    pub podcast_index_api_key: String,
    pub podcast_index_api_secret: String,
    #[serde(default)]
    pub rai_luce_code: String,
    #[serde(default)]
    pub podcast_directory_country: String,
    #[serde(default)]
    pub route_country: String,
    #[serde(default)]
    pub podcast_search_provider: PodcastSearchProvider,
    #[serde(default)]
    pub radio_favorites: Vec<RadioFavorite>,
    #[serde(default)]
    pub tv_favorites: Vec<TvFavorite>,
    #[serde(default)]
    pub weather_city: Option<WeatherCity>,
    #[serde(default)]
    pub weather_recent_cities: Vec<WeatherCity>,
    #[serde(default)]
    pub weather_temperature_unit: WeatherTemperatureUnit,
    #[serde(default)]
    pub gemini_api_key: String,
    #[serde(default)]
    pub audio_description_use_sonarpad_ai: bool,
    #[serde(default)]
    pub sonarpad_ai_access_code: String,
    #[serde(default)]
    pub sonarpad_ai_device_id: String,
    #[serde(default = "default_gemini_model")]
    pub gemini_model: String,
    #[serde(default = "default_audio_description_gemini_model")]
    pub audio_description_gemini_model: String,
    #[serde(default)]
    pub audio_description_language: Option<Language>,
    #[serde(default)]
    pub audio_description_tts_engine: TtsEngine,
    #[serde(default)]
    pub audio_description_tts_voice: String,
    #[serde(default)]
    pub audio_description_tts_rate: Option<i32>,
    #[serde(default)]
    pub audio_description_tts_volume: Option<i32>,
    #[serde(default = "default_audio_description_verbosity")]
    pub audio_description_verbosity: u8,
    #[serde(default)]
    pub audio_description_extended_pauses: bool,
    #[serde(default = "default_true")]
    pub audio_description_recognize_characters: bool,
    #[serde(default = "default_true")]
    pub audio_description_recognize_screen_text: bool,
    #[serde(default)]
    pub audio_description_keep_character_catalog: bool,
    #[serde(default)]
    pub audio_description_character_catalog: String,
    #[serde(default)]
    pub audio_description_save_project: bool,
    #[serde(default)]
    pub audio_description_create_video: bool,
    #[serde(default)]
    pub audio_description_delete_video_after: bool,
    #[serde(default)]
    pub audio_description_after_tv_recording: bool,
    pub youtube_include_timestamps: bool,
    #[serde(default = "default_stream_audio_output_format")]
    pub stream_audio_default_format: String,
    #[serde(default)]
    pub stream_favorites: Vec<StreamFavorite>,
    #[serde(default)]
    pub whisper_model_profile: String,
    #[serde(default)]
    pub whisper_cuda_enabled: bool,
    #[serde(default)]
    pub whisper_audio_language: String,
    #[serde(default)]
    pub whisper_include_timestamps: bool,
    pub last_seen_changelog_version: String,
    pub favorite_voices: Vec<FavoriteVoice>,
    pub dictionary: Vec<DictionaryEntry>,
    pub dictionary_translation_language: String,
    #[serde(default = "default_dictionary_lookup_language")]
    pub dictionary_lookup_language: String,
    #[serde(default = "default_editor_translate_target_language")]
    pub editor_translate_target_language: String,
    #[serde(default)]
    pub editor_translate_recent_languages: Vec<String>,
    #[serde(default)]
    pub dictionary_search_history: Vec<String>,
    pub wikipedia_language: String,
    pub text_color: u32,
    pub text_size: i32,
    pub tts_rate: i32,
    pub tts_pitch: i32,
    pub tts_volume: i32,
    #[serde(default)]
    pub edge_tts_tuning: TtsTuning,
    #[serde(default)]
    pub google_tts_tuning: TtsTuning,
    #[serde(default)]
    pub sapi5_tts_tuning: TtsTuning,
    #[serde(default)]
    pub sapi4_tts_tuning: TtsTuning,
    #[serde(default)]
    pub voice_profiles: Vec<VoiceProfile>,
    #[serde(default = "default_voice_profile_name")]
    pub active_voice_profile: String,
    #[serde(default)]
    pub editor_font_face: String,
    #[serde(default)]
    pub editor_read_only: bool,
    #[serde(default)]
    pub editor_escape_closes_window: bool,
    #[serde(default)]
    pub editor_up_down_moves_to_line_start: bool,
    #[serde(default)]
    pub automatic_bookmark: bool,
    pub show_voice_panel: bool,
    pub show_favorite_panel: bool,
    pub check_updates_on_startup: bool,
    #[serde(default)]
    pub check_beta_updates_on_startup: bool,
    #[serde(default)]
    pub installed_release_tag: String,
    pub prompt_program: String,
    #[serde(default)]
    pub network_proxy_url: String,
    #[serde(default)]
    pub network_proxy_port: String,
    #[serde(default)]
    pub network_proxy_username: String,
    #[serde(default)]
    pub network_proxy_password: String,
    #[serde(default)]
    pub remember_bdciechi_credentials: bool,
    #[serde(default)]
    pub bdciechi_username: String,
    #[serde(default)]
    pub bdciechi_password: String,
    #[serde(default)]
    pub bdciechi_last_successful_login_unix: i64,
    #[serde(default)]
    pub ytdlp_site_credentials: HashMap<String, String>,
    pub prompt_auto_scroll: bool,
    pub prompt_strip_ansi: bool,
    pub prompt_beep_on_idle: bool,
    pub prompt_prevent_sleep: bool,
    pub prompt_announce_lines: bool,
    pub interpreter_path: String,
    pub context_menu_open_with: bool,
    #[serde(default = "default_true")]
    pub confirm_delete_rss_podcast: bool,
    #[serde(default)]
    pub rss_delete_confirm_mode: RssDeleteConfirmMode,
    #[serde(default)]
    pub podcast_delete_confirm_mode: PodcastDeleteConfirmMode,
    #[serde(default)]
    pub rss_quick_copy_mode: RssQuickCopyMode,
    #[serde(default)]
    pub rss_show_article_preview: bool,
    #[serde(default = "default_true")]
    pub announce_unread_rss_podcast_items: bool,
    #[serde(default)]
    pub rss_podcast_unread_label_position: RssPodcastUnreadLabelPosition,
    #[serde(default)]
    pub rss_articles_date_display: ListDateDisplayMode,
    #[serde(default)]
    pub rss_articles_time_display: ListTimeDisplayMode,
    #[serde(default)]
    pub podcast_episodes_date_display: ListDateDisplayMode,
    #[serde(default)]
    pub podcast_episodes_time_display: ListTimeDisplayMode,
    #[serde(default)]
    pub shortcuts: ShortcutSettings,
    pub spellcheck_enabled: bool,
    pub spellcheck_language_mode: SpellcheckLanguageMode,
    pub spellcheck_fixed_language: String,
    #[serde(default)]
    pub rss_sources: Vec<RssSource>,
    #[serde(default)]
    pub rss_sources_by_language: HashMap<String, Vec<RssSource>>,
    #[serde(default)]
    pub rss_folders_by_language: HashMap<String, Vec<Vec<String>>>,
    #[serde(default)]
    pub rss_news_language: String,
    #[serde(default)]
    pub rss_local_city: String,
    #[serde(default)]
    pub rss_favorite_articles: Vec<RssItem>,
    #[serde(default)]
    pub podcast_sources: Vec<RssSource>,
    #[serde(default)]
    pub rss_removed_default_en: Vec<String>,
    #[serde(default)]
    pub rss_default_en_keys: Vec<String>,
    #[serde(default)]
    pub rss_removed_default_de: Vec<String>,
    #[serde(default)]
    pub rss_default_de_keys: Vec<String>,
    #[serde(default)]
    pub rss_removed_default_it: Vec<String>,
    #[serde(default)]
    pub rss_default_it_keys: Vec<String>,
    #[serde(default)]
    pub rss_removed_default_es: Vec<String>,
    #[serde(default)]
    pub rss_default_es_keys: Vec<String>,
    #[serde(default)]
    pub rss_removed_default_pt: Vec<String>,
    #[serde(default)]
    pub rss_default_pt_keys: Vec<String>,
    #[serde(default)]
    pub rss_removed_default_pt_br: Vec<String>,
    #[serde(default)]
    pub rss_default_pt_br_keys: Vec<String>,
    #[serde(default)]
    pub rss_removed_default_vi: Vec<String>,
    #[serde(default)]
    pub rss_default_vi_keys: Vec<String>,
    #[serde(default)]
    pub rss_removed_default_cs: Vec<String>,
    #[serde(default)]
    pub rss_default_cs_keys: Vec<String>,
    #[serde(default)]
    pub rss_removed_default_pl: Vec<String>,
    #[serde(default)]
    pub rss_default_pl_keys: Vec<String>,
    #[serde(default)]
    pub rss_removed_default_fr: Vec<String>,
    #[serde(default)]
    pub rss_default_fr_keys: Vec<String>,
    #[serde(default)]
    pub rss_removed_default_sr: Vec<String>,
    #[serde(default)]
    pub rss_default_sr_keys: Vec<String>,
    #[serde(default)]
    pub rss_removed_default_hi: Vec<String>,
    #[serde(default)]
    pub rss_default_hi_keys: Vec<String>,
    #[serde(default)]
    pub rss_global_max_concurrency: usize,
    #[serde(default)]
    pub rss_per_host_max_concurrency: usize,
    #[serde(default)]
    pub rss_per_host_rps: u32,
    #[serde(default)]
    pub rss_per_host_burst: u32,
    #[serde(default)]
    pub rss_max_retries: usize,
    #[serde(default)]
    pub rss_backoff_max_secs: u64,
    #[serde(default)]
    pub rss_initial_page_size: usize,
    #[serde(default)]
    pub rss_next_page_size: usize,
    #[serde(default)]
    pub rss_max_items_per_feed: usize,
    #[serde(default)]
    pub rss_max_excerpt_chars: usize,
    #[serde(default)]
    pub rss_cooldown_blocked_secs: u64,
    #[serde(default)]
    pub rss_cooldown_not_found_secs: u64,
    #[serde(default)]
    pub rss_cooldown_rate_limited_secs: u64,
    /// Se true, invia crash report anonimi a Sentry
    #[serde(default = "default_true")]
    pub send_crash_reports: bool,
    /// Se true, usa il nome legacy "Novapad" per il titolo finestra e collegamenti.
    #[serde(default)]
    pub use_legacy_name: bool,
}

pub fn voice_profile_from_settings_fields(name: String, settings: &AppSettings) -> VoiceProfile {
    VoiceProfile {
        name,
        tts_engine: settings.tts_engine,
        tts_voice: settings.tts_voice.clone(),
        tts_only_multilingual: settings.tts_only_multilingual,
        tts_manual_tuning: settings.tts_manual_tuning,
        tts_rate: settings.tts_rate,
        tts_pitch: settings.tts_pitch,
        tts_volume: settings.tts_volume,
        edge_tts_tuning: settings.edge_tts_tuning,
        google_tts_tuning: settings.google_tts_tuning,
        sapi5_tts_tuning: settings.sapi5_tts_tuning,
        sapi4_tts_tuning: settings.sapi4_tts_tuning,
        use_dialogue_voice: settings.use_dialogue_voice,
        dialogue_tts_engine: settings.dialogue_tts_engine,
        dialogue_voice: settings.dialogue_voice.clone(),
        dialogue_voice_rate: settings.dialogue_voice_rate,
        dialogue_voice_pitch: settings.dialogue_voice_pitch,
        dialogue_voice_volume: settings.dialogue_voice_volume,
        dialogue_use_secondary_voice: settings.dialogue_use_secondary_voice,
        dialogue_secondary_tts_engine: settings.dialogue_secondary_tts_engine,
        dialogue_secondary_voice: settings.dialogue_secondary_voice.clone(),
        dialogue_secondary_voice_rate: settings.dialogue_secondary_voice_rate,
        dialogue_secondary_voice_pitch: settings.dialogue_secondary_voice_pitch,
        dialogue_secondary_voice_volume: settings.dialogue_secondary_voice_volume,
    }
}

pub fn apply_voice_profile_to_settings_fields(settings: &mut AppSettings, profile: &VoiceProfile) {
    settings.tts_engine = profile.tts_engine;
    settings.tts_voice = profile.tts_voice.clone();
    settings.tts_only_multilingual = profile.tts_only_multilingual;
    settings.tts_manual_tuning = profile.tts_manual_tuning;
    settings.edge_tts_tuning = profile.edge_tts_tuning;
    settings.google_tts_tuning = profile.google_tts_tuning;
    settings.sapi5_tts_tuning = profile.sapi5_tts_tuning;
    settings.sapi4_tts_tuning = profile.sapi4_tts_tuning;
    let active_tuning = tts_tuning_for_engine(settings, profile.tts_engine);
    settings.tts_rate = active_tuning.rate;
    settings.tts_pitch = active_tuning.pitch;
    settings.tts_volume = active_tuning.volume;
    settings.use_dialogue_voice = profile.use_dialogue_voice;
    settings.dialogue_tts_engine = profile.dialogue_tts_engine;
    settings.dialogue_voice = profile.dialogue_voice.clone();
    settings.dialogue_voice_rate = profile.dialogue_voice_rate;
    settings.dialogue_voice_pitch = profile.dialogue_voice_pitch;
    settings.dialogue_voice_volume = profile.dialogue_voice_volume;
    settings.dialogue_use_secondary_voice = profile.dialogue_use_secondary_voice;
    settings.dialogue_secondary_tts_engine = profile.dialogue_secondary_tts_engine;
    settings.dialogue_secondary_voice = profile.dialogue_secondary_voice.clone();
    settings.dialogue_secondary_voice_rate = profile.dialogue_secondary_voice_rate;
    settings.dialogue_secondary_voice_pitch = profile.dialogue_secondary_voice_pitch;
    settings.dialogue_secondary_voice_volume = profile.dialogue_secondary_voice_volume;
}

pub fn sync_active_voice_profile_from_settings_fields(settings: &mut AppSettings) {
    let active_name = if settings.active_voice_profile.trim().is_empty() {
        DEFAULT_VOICE_PROFILE_NAME.to_string()
    } else {
        settings.active_voice_profile.trim().to_string()
    };
    let updated = voice_profile_from_settings_fields(active_name.clone(), settings);
    if let Some(profile) = settings
        .voice_profiles
        .iter_mut()
        .find(|profile| profile.name.eq_ignore_ascii_case(&active_name))
    {
        *profile = updated;
    } else {
        settings.voice_profiles.push(updated);
    }
    settings.active_voice_profile = active_name;
}

pub fn tts_tuning_for_engine(settings: &AppSettings, engine: TtsEngine) -> TtsTuning {
    match engine {
        TtsEngine::Edge => settings.edge_tts_tuning,
        TtsEngine::Google => settings.google_tts_tuning,
        TtsEngine::Sapi5 => settings.sapi5_tts_tuning,
        TtsEngine::Sapi4 => settings.sapi4_tts_tuning,
    }
}

pub fn set_tts_tuning_for_engine(settings: &mut AppSettings, engine: TtsEngine, tuning: TtsTuning) {
    match engine {
        TtsEngine::Edge => settings.edge_tts_tuning = tuning,
        TtsEngine::Google => settings.google_tts_tuning = tuning,
        TtsEngine::Sapi5 => settings.sapi5_tts_tuning = tuning,
        TtsEngine::Sapi4 => settings.sapi4_tts_tuning = tuning,
    }
}

fn default_true() -> bool {
    true
}

fn default_dialogue_voice_rate() -> i32 {
    0
}

fn default_dialogue_voice_pitch() -> i32 {
    0
}

fn default_dialogue_voice_volume() -> i32 {
    100
}

fn default_dialogue_tts_engine() -> TtsEngine {
    TtsEngine::Edge
}

fn default_dialogue_opening_quote() -> String {
    "\"|\u{201C}|\u{00AB}|\u{201E}".to_string()
}

fn default_dialogue_closing_quote() -> String {
    // U+201C is also the standard closing quotation mark in Czech/Slovak
    // typography (for example: „dialogue“).
    "\"|\u{201D}|\u{00BB}|\u{201C}".to_string()
}

fn default_indent_tab_width() -> u32 {
    4
}

fn default_indent_space_width() -> u32 {
    4
}

fn default_audiobook_split_minutes() -> u32 {
    5
}

fn default_audiobook_split_start_number() -> u32 {
    1
}

fn default_gemini_model() -> String {
    DEFAULT_GEMINI_MODEL.to_string()
}

fn default_audio_description_gemini_model() -> String {
    DEFAULT_AUDIO_DESCRIPTION_GEMINI_MODEL.to_string()
}

fn default_audio_description_verbosity() -> u8 {
    2
}

fn default_dictionary_lookup_language() -> String {
    "auto".to_string()
}

fn default_editor_translate_target_language() -> String {
    "it".to_string()
}

const MAX_EDITOR_TRANSLATE_RECENT_LANGUAGES: usize = 12;

fn default_stream_audio_output_format() -> String {
    "auto".to_string()
}

fn default_podcast_device_id() -> String {
    PODCAST_DEVICE_DEFAULT.to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            open_behavior: OpenBehavior::NewTab,
            language: Language::Italian,
            modified_marker_position: ModifiedMarkerPosition::End,
            tts_engine: TtsEngine::Edge,
            tts_voice: "it-IT-IsabellaNeural".to_string(),
            use_dialogue_voice: false,
            dialogue_voice: String::new(),
            dialogue_use_secondary_voice: false,
            dialogue_secondary_voice: String::new(),
            dialogue_secondary_tts_engine: TtsEngine::Edge,
            dialogue_secondary_voice_rate: 0,
            dialogue_secondary_voice_pitch: 0,
            dialogue_secondary_voice_volume: 100,
            dialogue_voice_rate: 0,
            dialogue_voice_pitch: 0,
            dialogue_voice_volume: 100,
            dialogue_tts_engine: TtsEngine::Edge,
            dialogue_opening_quote: default_dialogue_opening_quote(),
            dialogue_closing_quote: default_dialogue_closing_quote(),
            dialogue_allow_multiline: false,
            tts_only_multilingual: false,
            tts_manual_tuning: false,
            split_on_newline: false,
            word_wrap: true,
            group_tools_menu_by_category: true,
            show_video_during_playback: true,
            dark_mode: false,
            wrap_width: 80,
            smart_quotes: false,
            strip_markdown_keep_bullets: false,
            quote_prefix: "> ".to_string(),
            indentation_mode: IndentationMode::Default,
            indent_tab_width: default_indent_tab_width(),
            indent_space_width: default_indent_space_width(),
            move_cursor_during_reading: false,
            audiobook_skip_seconds: 60,
            audiobook_playback_speed: 1.0,
            audiobook_playback_pitch: 0.0,
            audiobook_playback_volume: 1.0,
            audiobook_m4b_bitrate: 128,
            audiobook_split: 0,
            audiobook_split_by_text: false,
            audiobook_split_text: String::new(),
            audiobook_split_text_requires_newline: true,
            audiobook_split_by_epub_chapter: false,
            audiobook_split_by_time: false,
            audiobook_split_minutes: default_audiobook_split_minutes(),
            audiobook_split_start_number: default_audiobook_split_start_number(),
            audiobook_part_naming_mode: AudiobookPartNamingMode::TitleNumber,
            audiobook_part_announcement_mode: AudiobookPartAnnouncementMode::None,
            subtitle_read_mode: SubtitleReadMode::User,
            subtitle_offset_ms: 0,
            subtitle_mix_export_on_play: true,
            subtitle_mix_ducking: false,
            podcast_include_microphone: true,
            podcast_microphone_device_id: PODCAST_DEVICE_DEFAULT.to_string(),
            podcast_microphone_gain: 1.5,
            dictation_microphone_device_id: PODCAST_DEVICE_DEFAULT.to_string(),
            podcast_include_system_audio: true,
            podcast_split_sources: false,
            podcast_system_device_id: PODCAST_DEVICE_DEFAULT.to_string(),
            podcast_system_gain: 1.0,
            podcast_system_capture_mode: PodcastSystemCaptureMode::AllSystem,
            podcast_include_single_app: false,
            podcast_single_app_pid: 0,
            podcast_selected_app_pids: Vec::new(),
            podcast_show_inactive_apps: false,
            podcast_output_format: PodcastFormat::Mp3,
            podcast_mp3_bitrate: 128,
            podcast_save_folder: default_podcast_save_folder(),
            radio_save_folder: default_radio_save_folder(),
            tv_save_folder: default_tv_save_folder(),
            audiobook_save_folder: default_audiobook_save_folder(),
            audio_description_save_folder: default_audio_description_save_folder(),
            audio_description_recent_project_folders: Vec::new(),
            media_save_folder: default_media_save_folder(),
            documents_save_folder: default_documents_save_folder(),
            podcast_include_video: false,
            podcast_monitor_id: String::new(),
            podcast_cache_limit_mb: 500,
            show_media_save_confirmation: true,
            podcast_index_api_key: String::new(),
            radio_favorites: Vec::new(),
            tv_favorites: Vec::new(),
            weather_city: None,
            weather_recent_cities: Vec::new(),
            weather_temperature_unit: WeatherTemperatureUnit::Celsius,
            podcast_index_api_secret: String::new(),
            rai_luce_code: String::new(),
            podcast_directory_country: String::new(),
            route_country: String::new(),
            podcast_search_provider: PodcastSearchProvider::Itunes,
            gemini_api_key: String::new(),
            audio_description_use_sonarpad_ai: false,
            sonarpad_ai_access_code: String::new(),
            sonarpad_ai_device_id: format!("spdev_{}", uuid::Uuid::new_v4().simple()),
            gemini_model: default_gemini_model(),
            audio_description_gemini_model: default_audio_description_gemini_model(),
            audio_description_language: None,
            audio_description_tts_engine: TtsEngine::Edge,
            audio_description_tts_voice: String::new(),
            audio_description_tts_rate: None,
            audio_description_tts_volume: None,
            audio_description_verbosity: default_audio_description_verbosity(),
            audio_description_extended_pauses: false,
            audio_description_recognize_characters: true,
            audio_description_recognize_screen_text: true,
            audio_description_keep_character_catalog: false,
            audio_description_character_catalog: String::new(),
            audio_description_save_project: false,
            audio_description_create_video: false,
            audio_description_delete_video_after: false,
            audio_description_after_tv_recording: false,
            youtube_include_timestamps: true,
            stream_audio_default_format: default_stream_audio_output_format(),
            stream_favorites: Vec::new(),
            whisper_model_profile: String::new(),
            whisper_cuda_enabled: false,
            whisper_audio_language: String::new(),
            whisper_include_timestamps: false,
            last_seen_changelog_version: String::new(),
            favorite_voices: Vec::new(),
            dictionary: Vec::new(),
            dictionary_translation_language: "auto".to_string(),
            dictionary_lookup_language: default_dictionary_lookup_language(),
            editor_translate_target_language: default_editor_translate_target_language(),
            editor_translate_recent_languages: Vec::new(),
            dictionary_search_history: Vec::new(),
            wikipedia_language: "auto".to_string(),
            text_color: 0x000000,
            text_size: 12,
            tts_rate: 0,
            tts_pitch: 0,
            tts_volume: 100,
            edge_tts_tuning: TtsTuning::default(),
            google_tts_tuning: TtsTuning::default(),
            sapi5_tts_tuning: TtsTuning::default(),
            sapi4_tts_tuning: TtsTuning::default(),
            voice_profiles: Vec::new(),
            active_voice_profile: default_voice_profile_name(),
            editor_font_face: String::new(),
            editor_read_only: false,
            editor_escape_closes_window: false,
            editor_up_down_moves_to_line_start: false,
            automatic_bookmark: false,
            show_voice_panel: false,
            show_favorite_panel: false,
            check_updates_on_startup: true,
            check_beta_updates_on_startup: false,
            installed_release_tag: String::new(),
            prompt_program: "cmd.exe".to_string(),
            network_proxy_url: String::new(),
            network_proxy_port: String::new(),
            network_proxy_username: String::new(),
            network_proxy_password: String::new(),
            remember_bdciechi_credentials: false,
            bdciechi_username: String::new(),
            bdciechi_password: String::new(),
            bdciechi_last_successful_login_unix: 0,
            ytdlp_site_credentials: HashMap::new(),
            prompt_auto_scroll: true,
            prompt_strip_ansi: true,
            prompt_beep_on_idle: true,
            prompt_prevent_sleep: true,
            prompt_announce_lines: true,
            interpreter_path: "python.exe".to_string(),
            context_menu_open_with: false,
            confirm_delete_rss_podcast: true,
            rss_delete_confirm_mode: RssDeleteConfirmMode::Both,
            podcast_delete_confirm_mode: PodcastDeleteConfirmMode::Both,
            rss_quick_copy_mode: RssQuickCopyMode::Title,
            rss_show_article_preview: false,
            announce_unread_rss_podcast_items: true,
            rss_podcast_unread_label_position: RssPodcastUnreadLabelPosition::Before,
            rss_articles_date_display: ListDateDisplayMode::Always,
            rss_articles_time_display: ListTimeDisplayMode::Always,
            podcast_episodes_date_display: ListDateDisplayMode::Always,
            podcast_episodes_time_display: ListTimeDisplayMode::OnlyIfMultipleSameDay,
            shortcuts: ShortcutSettings::default(),
            spellcheck_enabled: false,
            spellcheck_language_mode: SpellcheckLanguageMode::FollowEditorLanguage,
            spellcheck_fixed_language: "en-US".to_string(),
            rss_sources: Vec::new(),
            rss_sources_by_language: HashMap::new(),
            rss_folders_by_language: HashMap::new(),
            rss_news_language: String::new(),
            rss_local_city: String::new(),
            rss_favorite_articles: Vec::new(),
            rss_removed_default_en: Vec::new(),
            rss_default_en_keys: Vec::new(),
            rss_removed_default_de: Vec::new(),
            rss_default_de_keys: Vec::new(),
            rss_removed_default_it: Vec::new(),
            rss_default_it_keys: Vec::new(),
            rss_removed_default_es: Vec::new(),
            rss_default_es_keys: Vec::new(),
            rss_removed_default_pt: Vec::new(),
            rss_default_pt_keys: Vec::new(),
            rss_removed_default_pt_br: Vec::new(),
            rss_default_pt_br_keys: Vec::new(),
            rss_removed_default_vi: Vec::new(),
            rss_default_vi_keys: Vec::new(),
            rss_removed_default_cs: Vec::new(),
            rss_default_cs_keys: Vec::new(),
            rss_removed_default_pl: Vec::new(),
            rss_default_pl_keys: Vec::new(),
            rss_removed_default_fr: Vec::new(),
            rss_default_fr_keys: Vec::new(),
            rss_removed_default_sr: Vec::new(),
            rss_default_sr_keys: Vec::new(),
            rss_removed_default_hi: Vec::new(),
            rss_default_hi_keys: Vec::new(),
            podcast_sources: Vec::new(),
            rss_global_max_concurrency: 8,
            rss_per_host_max_concurrency: 2,
            rss_per_host_rps: 1,
            rss_per_host_burst: 2,
            rss_max_retries: 4,
            rss_backoff_max_secs: 120,
            rss_initial_page_size: 100,
            rss_next_page_size: 100,
            rss_max_items_per_feed: 5000,
            rss_max_excerpt_chars: 512,
            rss_cooldown_blocked_secs: 3600,
            rss_cooldown_not_found_secs: 86400,
            rss_cooldown_rate_limited_secs: 300,
            send_crash_reports: true, // Attivato di default
            use_legacy_name: false,
        }
    }
}

pub fn app_display_name(settings: &AppSettings) -> &'static str {
    if settings.use_legacy_name {
        "Novapad"
    } else {
        "Sonarpad"
    }
}

#[cfg(not(feature = "standalone"))]
fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

#[cfg(not(feature = "standalone"))]
fn is_portable_folder(exe_dir: &std::path::Path) -> bool {
    exe_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| {
            n.eq_ignore_ascii_case("sonarpad portable")
                || n.eq_ignore_ascii_case("novapad portable")
        })
        .unwrap_or(false)
}

#[cfg(not(feature = "standalone"))]
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    let entries = std::fs::read_dir(src)?;
    for entry_result in entries {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(e) => {
                crate::log_debug(&format!(
                    "settings_migrate_read_dir_entry_failed {}: {}",
                    src.display(),
                    e
                ));
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(e) => {
                crate::log_debug(&format!(
                    "settings_migrate_file_type_failed {}: {}",
                    entry.path().display(),
                    e
                ));
                continue;
            }
        };
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&target) {
                crate::log_debug(&format!(
                    "settings_migrate_create_dir_failed {}: {}",
                    target.display(),
                    e
                ));
                continue;
            }
            if let Err(e) = copy_dir_recursive(&entry.path(), &target) {
                crate::log_debug(&format!(
                    "settings_migrate_copy_dir_failed {} -> {}: {}",
                    entry.path().display(),
                    target.display(),
                    e
                ));
            }
        } else if file_type.is_file() {
            if let Err(e) = std::fs::copy(entry.path(), &target) {
                crate::log_debug(&format!(
                    "settings_migrate_copy_file_failed {} -> {}: {}",
                    entry.path().display(),
                    target.display(),
                    e
                ));
            }
        } else {
            crate::log_debug(&format!(
                "settings_migrate_skip_special {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

#[cfg(not(feature = "standalone"))]
fn migrate_legacy_settings_dir(legacy_dir: &std::path::Path, new_dir: &std::path::Path) {
    if new_dir.exists() || !legacy_dir.exists() {
        return;
    }
    match std::fs::rename(legacy_dir, new_dir) {
        Ok(_) => {
            crate::log_debug(&format!(
                "settings_migrate_renamed {} -> {}",
                legacy_dir.display(),
                new_dir.display()
            ));
        }
        Err(e) => {
            crate::log_debug(&format!(
                "settings_migrate_rename_failed {} -> {}: {}",
                legacy_dir.display(),
                new_dir.display(),
                e
            ));
            if let Err(e) = std::fs::create_dir_all(new_dir) {
                crate::log_debug(&format!(
                    "settings_migrate_create_dir_failed {}: {}",
                    new_dir.display(),
                    e
                ));
                return;
            }
            if let Err(e) = copy_dir_recursive(legacy_dir, new_dir) {
                crate::log_debug(&format!(
                    "settings_migrate_copy_failed {} -> {}: {}",
                    legacy_dir.display(),
                    new_dir.display(),
                    e
                ));
            }
        }
    }
}

#[cfg(not(feature = "standalone"))]
fn migrate_legacy_log_files(settings_dir: &std::path::Path) {
    let legacy_log = settings_dir.join("Novapad.log");
    let new_log = settings_dir.join("Sonarpad.log");
    if legacy_log.exists() && !new_log.exists() {
        match std::fs::rename(&legacy_log, &new_log) {
            Ok(_) => {
                crate::log_debug(&format!(
                    "settings_migrate_log_renamed {} -> {}",
                    legacy_log.display(),
                    new_log.display()
                ));
            }
            Err(e) => {
                crate::log_debug(&format!(
                    "settings_migrate_log_rename_failed {} -> {}: {}",
                    legacy_log.display(),
                    new_log.display(),
                    e
                ));
                if let Err(e) = std::fs::copy(&legacy_log, &new_log) {
                    crate::log_debug(&format!(
                        "settings_migrate_log_copy_failed {} -> {}: {}",
                        legacy_log.display(),
                        new_log.display(),
                        e
                    ));
                }
            }
        }
    }
    let legacy_lock = settings_dir.join("Novapad.log.lock");
    let new_lock = settings_dir.join("Sonarpad.log.lock");
    if legacy_lock.exists()
        && !new_lock.exists()
        && let Err(e) = std::fs::rename(&legacy_lock, &new_lock)
    {
        crate::log_debug(&format!(
            "settings_migrate_log_lock_rename_failed {} -> {}: {}",
            legacy_lock.display(),
            new_lock.display(),
            e
        ));
    }
}

#[cfg(not(feature = "standalone"))]
fn exe_drive_type(exe: &std::path::Path) -> Option<u32> {
    match exe.components().next()? {
        Component::Prefix(p) => match p.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                let root = format!("{}:\\", letter as char);
                Some(unsafe { GetDriveTypeW(windows::core::PCWSTR(wide(&root).as_ptr())) })
            }
            _ => None,
        },
        _ => None,
    }
}

fn dir_is_writable(dir: &std::path::Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(format!(".probe_{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            crate::log_if_err!(std::fs::remove_file(&probe));
            true
        }
        Err(_) => false,
    }
}

#[cfg(feature = "standalone")]
fn resolve_settings_dir() -> PathBuf {
    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let exe_dir = exe_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();

    // Standalone: preferisce config/ locale, ma fallback su %APPDATA% se non scrivibile
    let portable_dir = exe_dir.join("config");
    let appdata_dir = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|p| p.join("Sonarpad"))
        .unwrap_or_else(|| portable_dir.clone());

    if dir_is_writable(&portable_dir) {
        crate::log_if_err!(std::fs::create_dir_all(&portable_dir));
        portable_dir
    } else {
        crate::log_if_err!(std::fs::create_dir_all(&appdata_dir));
        if dir_is_writable(&appdata_dir) {
            appdata_dir
        } else {
            crate::log_if_err!(std::fs::create_dir_all(&portable_dir));
            portable_dir
        }
    }
}

#[cfg(not(feature = "standalone"))]
fn resolve_settings_dir() -> PathBuf {
    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let exe_dir = exe_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();

    // Portable: <exe_dir>\config\settings.json
    let portable_dir = exe_dir.join("config");

    // Non-portable: %APPDATA%\Sonarpad\settings.json
    let appdata_root = std::env::var_os("APPDATA").map(PathBuf::from);
    let appdata_dir = appdata_root
        .as_ref()
        .map(|p| p.join("Sonarpad"))
        .unwrap_or_else(|| portable_dir.clone());
    if let Some(root) = appdata_root.as_ref() {
        let legacy_dir = root.join("Novapad");
        migrate_legacy_settings_dir(&legacy_dir, &appdata_dir);
        migrate_legacy_log_files(&appdata_dir);
    }

    // 1) "sonarpad portable" -> portable forzato
    // 2) drive removibile -> portable
    let preferred_dir = if is_portable_folder(&exe_dir)
        || matches!(exe_drive_type(&exe_path), Some(t) if t == DRIVE_REMOVABLE)
    {
        portable_dir.clone()
    }
    // 3) default -> AppData\Sonarpad
    else {
        appdata_dir
    };

    if dir_is_writable(&preferred_dir) {
        preferred_dir
    } else {
        crate::log_if_err!(std::fs::create_dir_all(&portable_dir));
        portable_dir
    }
}

pub fn settings_dir() -> PathBuf {
    use std::sync::OnceLock;
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(resolve_settings_dir).clone()
}

fn get_settings_path() -> PathBuf {
    resolve_settings_dir().join("settings.json")
}

fn rai_luce_trace_path() -> PathBuf {
    resolve_settings_dir().join("rai_luce_trace.txt")
}

fn rai_luce_backup_path() -> PathBuf {
    resolve_settings_dir().join("rai_luce_last_good.txt")
}

fn log_rai_luce_event(message: &str) {
    let dir = resolve_settings_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        crate::log_debug(&format!(
            "Failed to create settings directory for Rai Luce trace: {}",
            e
        ));
        return;
    }
    let line = format!(
        "[{}] {}\r\n",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        message
    );
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(rai_luce_trace_path())
    {
        Ok(mut file) => {
            if let Err(e) = file.write_all(line.as_bytes()) {
                crate::log_debug(&format!("Failed to write Rai Luce trace: {}", e));
            }
        }
        Err(e) => {
            crate::log_debug(&format!("Failed to open Rai Luce trace file: {}", e));
        }
    }
}

fn system_language() -> Language {
    let mut buffer = [0u16; 85];
    let len = crate::get_user_default_locale_name_safe(&mut buffer);
    if len > 0 {
        let locale = String::from_utf16_lossy(&buffer[..(len as usize).saturating_sub(1)]);
        let lower = locale.to_lowercase();
        if lower.starts_with("it") {
            return Language::Italian;
        }
        if lower.starts_with("es") {
            return Language::Spanish;
        }
        if lower.starts_with("pt-br") {
            return Language::PortugueseBrazilian;
        }
        if lower.starts_with("pt") {
            return Language::Portuguese;
        }
        if lower.starts_with("sv") {
            return Language::Swedish;
        }
        if lower.starts_with("vi") {
            return Language::Vietnamese;
        }
        if lower.starts_with("cs") {
            return Language::Czech;
        }
        if lower.starts_with("pl") {
            return Language::Polish;
        }
        if lower.starts_with("fr") {
            return Language::French;
        }
        if lower.starts_with("sr") {
            return Language::Serbian;
        }
        if lower.starts_with("uk") {
            return Language::Ukrainian;
        }
        if lower.starts_with("lt") {
            return Language::Lithuanian;
        }
        if lower.starts_with("ru") {
            return Language::Russian;
        }
        if lower.starts_with("zh") {
            return Language::Chinese;
        }
        if lower.starts_with("hi") {
            return Language::Hindi;
        }
        return Language::English;
    }
    Language::Italian
}

pub fn default_podcast_save_folder() -> String {
    let mut base = sonarpad_documents_root();
    base.push("Recordings");
    base.to_string_lossy().to_string()
}

pub fn default_radio_save_folder() -> String {
    PathBuf::from(default_podcast_save_folder())
        .join("Radio")
        .to_string_lossy()
        .to_string()
}

pub fn default_tv_save_folder() -> String {
    PathBuf::from(default_podcast_save_folder())
        .join("TV")
        .to_string_lossy()
        .to_string()
}

pub fn default_audiobook_save_folder() -> String {
    let mut base = sonarpad_documents_root();
    base.push("Audiobooks");
    base.to_string_lossy().to_string()
}

pub fn default_audio_description_save_folder() -> String {
    let mut base = sonarpad_documents_root();
    base.push("Audiodescriptions");
    base.to_string_lossy().to_string()
}

pub fn default_media_save_folder() -> String {
    let mut base = sonarpad_documents_root();
    base.push("Media");
    base.to_string_lossy().to_string()
}

pub fn default_documents_save_folder() -> String {
    let mut base = sonarpad_documents_root();
    base.push("Documents");
    base.to_string_lossy().to_string()
}

pub fn default_images_save_folder() -> String {
    let mut base = sonarpad_documents_root();
    base.push("Images");
    base.to_string_lossy().to_string()
}

fn legacy_podcast_save_folder() -> String {
    let mut base = known_documents_dir();
    base.push("Novapad Recordings");
    base.to_string_lossy().to_string()
}

fn legacy_sonarpad_recordings_folder() -> String {
    let mut base = known_documents_dir();
    base.push("Sonarpad Recordings");
    base.to_string_lossy().to_string()
}

fn legacy_audiobook_save_folder() -> String {
    let mut base = known_documents_dir();
    base.push("Sonarpad Audiobooks");
    base.to_string_lossy().to_string()
}

fn legacy_media_save_folder() -> String {
    let mut base = known_documents_dir();
    base.push("Sonarpad Media");
    base.to_string_lossy().to_string()
}

fn known_documents_dir() -> PathBuf {
    known_folder_path(&FOLDERID_Documents).unwrap_or_else(|| {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join("Documents")
    })
}

fn sonarpad_documents_root() -> PathBuf {
    let mut base = known_documents_dir();
    base.push("Sonarpad");
    base
}

fn migrate_legacy_folder(legacy: &std::path::Path, target: &std::path::Path) {
    if !legacy.exists() || target.exists() {
        return;
    }
    if let Err(e) = std::fs::rename(legacy, target) {
        crate::log_debug(&format!(
            "podcast_folder_rename_failed {} -> {}: {}",
            legacy.display(),
            target.display(),
            e
        ));
        if let Err(e) = copy_dir_recursive_simple(legacy, target) {
            crate::log_debug(&format!(
                "podcast_folder_copy_failed {} -> {}: {}",
                legacy.display(),
                target.display(),
                e
            ));
        } else if let Err(e) = std::fs::remove_dir_all(legacy) {
            crate::log_debug(&format!(
                "podcast_folder_remove_failed {}: {}",
                legacy.display(),
                e
            ));
        }
    }
}

fn copy_dir_recursive_simple(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    let entries = std::fs::read_dir(src)?;
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry_result in entries {
        let entry = entry_result?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive_simple(&entry.path(), &target)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn known_folder_path(folder: &windows::core::GUID) -> Option<PathBuf> {
    unsafe {
        let raw = SHGetKnownFolderPath(
            folder,
            windows::Win32::UI::Shell::KNOWN_FOLDER_FLAG(0),
            HANDLE(0),
        )
        .ok()?;
        if raw.is_null() {
            return None;
        }
        let path = raw.to_string().unwrap_or_default();
        CoTaskMemFree(Some(raw.0 as *const _));
        if path.is_empty() {
            None
        } else {
            Some(PathBuf::from(path))
        }
    }
}

fn normalize_voice_profiles(settings: &mut AppSettings) {
    set_tts_tuning_for_engine(
        settings,
        settings.tts_engine,
        TtsTuning::new(settings.tts_rate, settings.tts_pitch, settings.tts_volume),
    );
    for profile in &mut settings.voice_profiles {
        profile.name = profile.name.trim().to_string();
        match profile.tts_engine {
            TtsEngine::Edge => {
                profile.edge_tts_tuning =
                    TtsTuning::new(profile.tts_rate, profile.tts_pitch, profile.tts_volume);
            }
            TtsEngine::Google => {
                profile.google_tts_tuning =
                    TtsTuning::new(profile.tts_rate, profile.tts_pitch, profile.tts_volume);
            }
            TtsEngine::Sapi5 => {
                profile.sapi5_tts_tuning =
                    TtsTuning::new(profile.tts_rate, profile.tts_pitch, profile.tts_volume);
            }
            TtsEngine::Sapi4 => {
                profile.sapi4_tts_tuning =
                    TtsTuning::new(profile.tts_rate, profile.tts_pitch, profile.tts_volume);
            }
        }
    }
    settings.voice_profiles.retain(|p| !p.name.is_empty());

    let mut deduped: Vec<VoiceProfile> = Vec::with_capacity(settings.voice_profiles.len());
    for profile in settings.voice_profiles.drain(..) {
        if deduped
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(&profile.name))
        {
            continue;
        }
        deduped.push(profile);
    }
    settings.voice_profiles = deduped;

    if !settings
        .voice_profiles
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(DEFAULT_VOICE_PROFILE_NAME))
    {
        settings
            .voice_profiles
            .push(voice_profile_from_settings_fields(
                DEFAULT_VOICE_PROFILE_NAME.to_string(),
                settings,
            ));
    }

    if settings.active_voice_profile.trim().is_empty() {
        settings.active_voice_profile = DEFAULT_VOICE_PROFILE_NAME.to_string();
    }

    if !settings
        .voice_profiles
        .iter()
        .any(|p| p.name.eq_ignore_ascii_case(&settings.active_voice_profile))
    {
        settings.active_voice_profile = DEFAULT_VOICE_PROFILE_NAME.to_string();
    }

    if let Some(profile) = settings
        .voice_profiles
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(&settings.active_voice_profile))
        .cloned()
    {
        apply_voice_profile_to_settings_fields(settings, &profile);
    }
}

pub fn load_settings() -> AppSettings {
    let default_settings = AppSettings {
        language: system_language(),
        ..Default::default()
    };

    let path = get_settings_path();
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(data) => match serde_json::from_str::<AppSettings>(&data) {
                Ok(mut settings) => {
                    log_rai_luce_event(&format!(
                        "load_settings parsed settings.json raw_len={}",
                        settings.rai_luce_code.len()
                    ));
                    if settings.remember_bdciechi_credentials
                        && !settings.bdciechi_password.trim().is_empty()
                    {
                        match decrypt_bdciechi_password(&settings.bdciechi_password) {
                            Some(password) => settings.bdciechi_password = password,
                            None => {
                                crate::log_debug(
                                    "Failed to decrypt stored BDCiechi password; clearing saved credentials",
                                );
                                settings.bdciechi_username.clear();
                                settings.bdciechi_password.clear();
                                settings.bdciechi_last_successful_login_unix = 0;
                            }
                        }
                    }
                    if !settings.ytdlp_site_credentials.is_empty() {
                        let mut decrypted_credentials = HashMap::new();
                        for (site, value) in std::mem::take(&mut settings.ytdlp_site_credentials) {
                            match decrypt_ytdlp_site_credentials_entry(&value) {
                                Some(credentials) => {
                                    if let Some(serialized) =
                                        serialize_ytdlp_site_credentials(&credentials)
                                    {
                                        decrypted_credentials.insert(site, serialized);
                                    }
                                }
                                None => {
                                    crate::log_debug(&format!(
                                        "Failed to decrypt stored yt-dlp credentials for site {}; preserving stored entry",
                                        site
                                    ));
                                    if !value.trim().is_empty() {
                                        decrypted_credentials.insert(site, value);
                                    }
                                }
                            }
                        }
                        settings.ytdlp_site_credentials = decrypted_credentials;
                    }
                    if !settings.rai_luce_code.trim().is_empty() {
                        match decrypt_rai_luce_code(&settings.rai_luce_code) {
                            Some(code) => {
                                log_rai_luce_event(&format!(
                                    "load_settings decrypted Rai Luce code len={}",
                                    code.len()
                                ));
                                settings.rai_luce_code = code;
                            }
                            None => {
                                crate::log_debug(
                                    "Failed to decrypt stored Rai Luce code; clearing saved code",
                                );
                                log_rai_luce_event(
                                    "load_settings failed to decrypt Rai Luce code; clearing in-memory value",
                                );
                                settings.rai_luce_code.clear();
                            }
                        }
                    } else {
                        log_rai_luce_event(
                            "load_settings found empty Rai Luce code in settings.json",
                        );
                    }
                    let normalized = normalize_settings(settings);
                    update_cached_rai_luce_code(&normalized.rai_luce_code);
                    save_settings(normalized.clone());
                    return normalized;
                }
                Err(e) => {
                    log_rai_luce_event(&format!(
                        "load_settings failed to parse settings.json err={}",
                        e
                    ));
                    crate::log_debug(&format!(
                        "Failed to parse settings from {}: {}",
                        path.display(),
                        e
                    ));
                }
            },
            Err(e) => {
                log_rai_luce_event(&format!(
                    "load_settings failed to read settings.json err={}",
                    e
                ));
                crate::log_debug(&format!(
                    "Failed to read settings from {}: {}",
                    path.display(),
                    e
                ));
            }
        }

        let normalized = normalize_settings(default_settings);
        update_cached_rai_luce_code(&normalized.rai_luce_code);
        log_rai_luce_event(
            "load_settings returning normalized defaults because existing file was unreadable",
        );
        return normalized;
    }

    let normalized = normalize_settings(default_settings);
    update_cached_rai_luce_code(&normalized.rai_luce_code);
    log_rai_luce_event(
        "load_settings creating default settings because settings.json does not exist",
    );
    save_settings(normalized.clone());
    normalized
}

fn rss_favorite_item_key(item: &RssItem) -> String {
    if !item.guid.trim().is_empty() {
        return item.guid.trim().to_string();
    }
    if !item.link.trim().is_empty() {
        return item.link.trim().to_string();
    }
    item.title.trim().to_string()
}

fn normalize_rss_favorite_articles(items: &mut Vec<RssItem>) {
    let mut seen = std::collections::HashSet::new();
    items.retain_mut(|item| {
        item.title = item.title.trim().to_string();
        item.link = item.link.trim().to_string();
        item.description = item.description.trim().to_string();
        item.guid = item.guid.trim().to_string();
        item.is_folder = false;
        let key = rss_favorite_item_key(item);
        if key.is_empty() {
            return false;
        }
        seen.insert(key)
    });
}

fn normalize_settings(mut settings: AppSettings) -> AppSettings {
    settings.network_proxy_url = settings.network_proxy_url.trim().to_string();
    settings.network_proxy_port = settings.network_proxy_port.trim().to_string();
    settings.network_proxy_username = settings.network_proxy_username.trim().to_string();
    settings.network_proxy_password = settings.network_proxy_password.trim().to_string();
    settings.bdciechi_username = settings.bdciechi_username.trim().to_string();
    settings.bdciechi_password = settings.bdciechi_password.trim().to_string();
    settings.ytdlp_site_credentials = settings
        .ytdlp_site_credentials
        .into_iter()
        .filter_map(|(site, value)| {
            let site = normalize_ytdlp_site_key(&site)?;
            let mut credentials = match parse_ytdlp_site_credentials_entry(&value) {
                Some(credentials) => credentials,
                None => {
                    if value.trim().is_empty() {
                        return None;
                    }
                    return Some((site, value));
                }
            };
            credentials.username = credentials.username.trim().to_string();
            credentials.password = credentials.password.trim().to_string();
            if credentials.username.is_empty() || credentials.password.is_empty() {
                return None;
            }
            let serialized = serialize_ytdlp_site_credentials(&credentials)?;
            Some((site, serialized))
        })
        .collect();
    settings.rai_luce_code = settings.rai_luce_code.trim().to_string();
    normalize_shortcut_settings(&mut settings.shortcuts);
    settings.podcast_directory_country = settings
        .podcast_directory_country
        .trim()
        .to_ascii_lowercase();
    settings.route_country = settings.route_country.trim().to_ascii_lowercase();
    let invalid_weather_city = if let Some(city) = settings.weather_city.as_mut() {
        city.name = city.name.trim().to_string();
        city.admin1 = city.admin1.trim().to_string();
        city.country = city.country.trim().to_string();
        city.name.is_empty() || !city.latitude.is_finite() || !city.longitude.is_finite()
    } else {
        false
    };
    if invalid_weather_city {
        settings.weather_city = None;
    }
    let mut normalized_weather_cities: Vec<WeatherCity> = Vec::new();
    for mut city in settings.weather_recent_cities.drain(..) {
        city.name = city.name.trim().to_string();
        city.admin1 = city.admin1.trim().to_string();
        city.country = city.country.trim().to_string();
        if city.name.is_empty() || !city.latitude.is_finite() || !city.longitude.is_finite() {
            continue;
        }
        if normalized_weather_cities.iter().any(|existing| {
            (existing.latitude - city.latitude).abs() < 0.000_001
                && (existing.longitude - city.longitude).abs() < 0.000_001
        }) {
            continue;
        }
        normalized_weather_cities.push(city);
        if normalized_weather_cities.len() >= 50 {
            break;
        }
    }
    settings.weather_recent_cities = normalized_weather_cities;
    if !settings.remember_bdciechi_credentials {
        settings.bdciechi_username.clear();
        settings.bdciechi_password.clear();
        settings.bdciechi_last_successful_login_unix = 0;
    }
    settings.dialogue_opening_quote = settings.dialogue_opening_quote.trim().to_string();
    settings.dialogue_closing_quote = settings.dialogue_closing_quote.trim().to_string();
    if settings.dialogue_opening_quote.is_empty() || settings.dialogue_opening_quote == "\"" {
        settings.dialogue_opening_quote = default_dialogue_opening_quote();
    }
    if settings.dialogue_closing_quote.is_empty()
        || settings.dialogue_closing_quote == "\""
        || settings.dialogue_closing_quote == "\"|\u{201D}|\u{00BB}"
    {
        // Migrate the old default too, otherwise existing installations would
        // never receive support for Czech/Slovak closing quotes.
        settings.dialogue_closing_quote = default_dialogue_closing_quote();
    }
    let valid_indent = [2, 4, 6, 8];
    if !valid_indent.contains(&settings.indent_tab_width) {
        settings.indent_tab_width = default_indent_tab_width();
    }
    if !valid_indent.contains(&settings.indent_space_width) {
        settings.indent_space_width = default_indent_space_width();
    }
    if settings.podcast_save_folder.trim().is_empty() {
        settings.podcast_save_folder = default_podcast_save_folder();
    }
    settings.radio_save_folder = settings.radio_save_folder.trim().to_string();
    if settings.radio_save_folder.is_empty() {
        settings.radio_save_folder = PathBuf::from(settings.podcast_save_folder.as_str())
            .join("Radio")
            .to_string_lossy()
            .to_string();
    }
    settings.tv_save_folder = settings.tv_save_folder.trim().to_string();
    if settings.tv_save_folder.is_empty() {
        settings.tv_save_folder = PathBuf::from(settings.podcast_save_folder.as_str())
            .join("TV")
            .to_string_lossy()
            .to_string();
    }
    if settings.audiobook_save_folder.trim().is_empty() {
        settings.audiobook_save_folder = default_audiobook_save_folder();
    }
    settings.audio_description_save_folder =
        settings.audio_description_save_folder.trim().to_string();
    if settings.audio_description_save_folder.is_empty() {
        settings.audio_description_save_folder = default_audio_description_save_folder();
    }
    let mut normalized_audio_description_recent_project_folders = Vec::new();
    for folder in settings.audio_description_recent_project_folders.drain(..) {
        let folder = folder.trim().to_string();
        if folder.is_empty()
            || normalized_audio_description_recent_project_folders
                .iter()
                .any(|known: &String| known.eq_ignore_ascii_case(&folder))
        {
            continue;
        }
        normalized_audio_description_recent_project_folders.push(folder);
        if normalized_audio_description_recent_project_folders.len() >= 8 {
            break;
        }
    }
    settings.audio_description_recent_project_folders =
        normalized_audio_description_recent_project_folders;
    settings.media_save_folder = settings.media_save_folder.trim().to_string();
    if settings.media_save_folder.is_empty() {
        settings.media_save_folder = default_media_save_folder();
    }
    settings.documents_save_folder = settings.documents_save_folder.trim().to_string();
    if settings.documents_save_folder.is_empty() {
        settings.documents_save_folder = default_documents_save_folder();
    }
    if settings.podcast_index_api_key.trim().is_empty()
        || decrypt_podcast_index_secret(&settings.podcast_index_api_secret)
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        settings.podcast_search_provider = PodcastSearchProvider::Itunes;
    }
    if settings
        .podcast_save_folder
        .trim()
        .eq_ignore_ascii_case(&legacy_podcast_save_folder())
    {
        let legacy_path = PathBuf::from(legacy_podcast_save_folder());
        let new_path = PathBuf::from(default_podcast_save_folder());
        migrate_legacy_folder(&legacy_path, &new_path);
        settings.podcast_save_folder = new_path.to_string_lossy().to_string();
    }
    if settings
        .podcast_save_folder
        .trim()
        .eq_ignore_ascii_case(&legacy_sonarpad_recordings_folder())
    {
        settings.podcast_save_folder = PathBuf::from(default_podcast_save_folder())
            .to_string_lossy()
            .to_string();
    }
    migrate_legacy_folder(
        &PathBuf::from(legacy_sonarpad_recordings_folder()),
        &PathBuf::from(default_podcast_save_folder()),
    );
    let legacy_audiobooks_path = PathBuf::from(legacy_audiobook_save_folder());
    let new_audiobooks_path = PathBuf::from(default_audiobook_save_folder());
    migrate_legacy_folder(&legacy_audiobooks_path, &new_audiobooks_path);
    if settings
        .audiobook_save_folder
        .trim()
        .eq_ignore_ascii_case(&legacy_audiobook_save_folder())
    {
        settings.audiobook_save_folder = new_audiobooks_path.to_string_lossy().to_string();
    }
    let legacy_media_path = PathBuf::from(legacy_media_save_folder());
    let new_media_path = PathBuf::from(default_media_save_folder());
    migrate_legacy_folder(&legacy_media_path, &new_media_path);
    if settings
        .media_save_folder
        .trim()
        .eq_ignore_ascii_case(&legacy_media_save_folder())
    {
        settings.media_save_folder = new_media_path.to_string_lossy().to_string();
    }
    if settings.podcast_mp3_bitrate == 0 {
        settings.podcast_mp3_bitrate = 128;
    }
    settings.podcast_mp3_bitrate = settings.podcast_mp3_bitrate.clamp(64, 320);
    settings.podcast_selected_app_pids.retain(|pid| *pid != 0);
    settings.podcast_selected_app_pids.sort_unstable();
    settings.podcast_selected_app_pids.dedup();
    if matches!(
        settings.podcast_system_capture_mode,
        PodcastSystemCaptureMode::AllSystem
    ) && settings.podcast_include_single_app
    {
        settings.podcast_system_capture_mode = PodcastSystemCaptureMode::SingleApp;
    }
    if settings.podcast_single_app_pid != 0
        && settings
            .podcast_selected_app_pids
            .iter()
            .all(|pid| *pid != settings.podcast_single_app_pid)
    {
        settings
            .podcast_selected_app_pids
            .push(settings.podcast_single_app_pid);
    }
    settings.podcast_selected_app_pids.sort_unstable();
    settings.podcast_selected_app_pids.dedup();
    if !settings.podcast_include_system_audio {
        settings.podcast_system_capture_mode = PodcastSystemCaptureMode::AllSystem;
        settings.podcast_include_single_app = false;
    }
    if settings.audiobook_m4b_bitrate == 0 {
        settings.audiobook_m4b_bitrate = 128;
    }
    settings.audiobook_m4b_bitrate = settings.audiobook_m4b_bitrate.clamp(64, 320);
    settings.audiobook_split = settings.audiobook_split.clamp(0, 100);
    settings.audiobook_split_minutes = settings.audiobook_split_minutes.clamp(1, 60);
    settings.audiobook_split_start_number = settings.audiobook_split_start_number.clamp(1, 99);
    if settings.modified_marker_position == ModifiedMarkerPosition::Unknown {
        settings.modified_marker_position = ModifiedMarkerPosition::End;
    }
    if settings.confirm_delete_rss_podcast {
        if matches!(settings.rss_delete_confirm_mode, RssDeleteConfirmMode::None) {
            settings.rss_delete_confirm_mode = RssDeleteConfirmMode::Both;
        }
        if matches!(
            settings.podcast_delete_confirm_mode,
            PodcastDeleteConfirmMode::None
        ) {
            settings.podcast_delete_confirm_mode = PodcastDeleteConfirmMode::Both;
        }
    } else {
        settings.rss_delete_confirm_mode = RssDeleteConfirmMode::None;
        settings.podcast_delete_confirm_mode = PodcastDeleteConfirmMode::None;
    }
    if settings.rss_global_max_concurrency == 0 {
        settings.rss_global_max_concurrency = 8;
    }
    if settings.rss_per_host_max_concurrency == 0 {
        settings.rss_per_host_max_concurrency = 2;
    }
    if settings.rss_per_host_rps == 0 {
        settings.rss_per_host_rps = 1;
    }
    if settings.rss_per_host_burst == 0 {
        settings.rss_per_host_burst = 2;
    }
    if settings.rss_max_retries == 0 {
        settings.rss_max_retries = 4;
    }
    if settings.rss_backoff_max_secs == 0 {
        settings.rss_backoff_max_secs = 120;
    }
    if settings.rss_initial_page_size == 0 {
        settings.rss_initial_page_size = 100;
    }
    if settings.rss_next_page_size == 0 {
        settings.rss_next_page_size = 100;
    }
    if settings.rss_max_items_per_feed == 0 {
        settings.rss_max_items_per_feed = 5000;
    }
    if settings.rss_max_excerpt_chars == 0 {
        settings.rss_max_excerpt_chars = 512;
    }
    settings.podcast_cache_limit_mb = settings.podcast_cache_limit_mb.clamp(100, 2048);
    if settings.spellcheck_fixed_language.trim().is_empty() {
        settings.spellcheck_fixed_language = "en-US".to_string();
    }
    if settings.dictionary_translation_language.trim().is_empty() {
        settings.dictionary_translation_language = "auto".to_string();
    }
    if settings.dictionary_lookup_language.trim().is_empty() {
        settings.dictionary_lookup_language = default_dictionary_lookup_language();
    }
    settings.editor_translate_target_language = settings
        .editor_translate_target_language
        .trim()
        .to_ascii_lowercase();
    if settings.editor_translate_target_language.is_empty() {
        settings.editor_translate_target_language = default_editor_translate_target_language();
    }
    let mut editor_translate_recent_languages = Vec::new();
    for language in settings.editor_translate_recent_languages.drain(..) {
        let language = language.trim().to_ascii_lowercase();
        if language.is_empty()
            || editor_translate_recent_languages
                .iter()
                .any(|existing: &String| existing == &language)
        {
            continue;
        }
        editor_translate_recent_languages.push(language);
        if editor_translate_recent_languages.len() >= MAX_EDITOR_TRANSLATE_RECENT_LANGUAGES {
            break;
        }
    }
    settings.editor_translate_recent_languages = editor_translate_recent_languages;
    settings.gemini_api_key =
        decrypt_gemini_api_key(&settings.gemini_api_key).unwrap_or(settings.gemini_api_key);
    settings.gemini_api_key = settings.gemini_api_key.trim().to_string();
    settings.sonarpad_ai_access_code =
        decrypt_sonarpad_ai_access_code(&settings.sonarpad_ai_access_code)
            .unwrap_or(settings.sonarpad_ai_access_code);
    settings.sonarpad_ai_access_code = settings.sonarpad_ai_access_code.trim().to_string();
    settings.sonarpad_ai_device_id = settings.sonarpad_ai_device_id.trim().to_string();
    if settings.sonarpad_ai_device_id.is_empty() {
        settings.sonarpad_ai_device_id = format!("spdev_{}", uuid::Uuid::new_v4().simple());
    }
    settings.gemini_model = settings.gemini_model.trim().to_string();
    if settings.gemini_model.is_empty() || settings.gemini_model == LEGACY_DEFAULT_GEMINI_MODEL {
        settings.gemini_model = default_gemini_model();
    }
    settings.audio_description_gemini_model =
        settings.audio_description_gemini_model.trim().to_string();
    if settings.audio_description_gemini_model.is_empty() {
        settings.audio_description_gemini_model = default_audio_description_gemini_model();
    }
    settings.audio_description_tts_voice = settings.audio_description_tts_voice.trim().to_string();
    settings.audio_description_tts_rate = settings
        .audio_description_tts_rate
        .map(|value| value.clamp(-100, 100));
    settings.audio_description_tts_volume = settings
        .audio_description_tts_volume
        .map(|value| value.clamp(25, 200));
    settings.audio_description_character_catalog = settings
        .audio_description_character_catalog
        .trim()
        .to_string();
    if !settings.audio_description_recognize_characters {
        settings.audio_description_keep_character_catalog = false;
    }
    settings.audio_description_verbosity = settings.audio_description_verbosity.min(2);
    settings.stream_audio_default_format = settings.stream_audio_default_format.trim().to_string();
    if settings.stream_audio_default_format.is_empty() {
        settings.stream_audio_default_format = default_stream_audio_output_format();
    } else {
        settings.stream_audio_default_format =
            settings.stream_audio_default_format.to_ascii_lowercase();
        if !matches!(
            settings.stream_audio_default_format.as_str(),
            "auto" | "mp4" | "mp3" | "m4a" | "opus" | "ogg" | "wav" | "flac"
        ) {
            settings.stream_audio_default_format = default_stream_audio_output_format();
        }
    }
    for favorite in &mut settings.stream_favorites {
        favorite.label = favorite.label.trim().to_string();
        favorite.url = favorite.url.trim().to_string();
    }
    settings
        .stream_favorites
        .retain(|favorite| !favorite.url.is_empty());
    let mut deduped_stream_favorites = Vec::with_capacity(settings.stream_favorites.len());
    for favorite in settings.stream_favorites.drain(..) {
        if deduped_stream_favorites
            .iter()
            .any(|existing: &StreamFavorite| existing.url.eq_ignore_ascii_case(&favorite.url))
        {
            continue;
        }
        deduped_stream_favorites.push(favorite);
    }
    settings.stream_favorites = deduped_stream_favorites;
    settings.whisper_model_profile = settings.whisper_model_profile.trim().to_ascii_lowercase();
    if matches!(
        settings.whisper_model_profile.as_str(),
        "tiny_q5_1" | "base_q5_1"
    ) {
        settings.whisper_model_profile = "small_q5_1".to_string();
    }
    if !matches!(
        settings.whisper_model_profile.as_str(),
        "" | "small_q5_1" | "medium_q5_0" | "large_v3_turbo_q5_0"
    ) {
        settings.whisper_model_profile.clear();
    }
    settings.dictation_microphone_device_id =
        settings.dictation_microphone_device_id.trim().to_string();
    if settings.dictation_microphone_device_id.is_empty() {
        settings.dictation_microphone_device_id = default_podcast_device_id();
    }
    settings
        .dictionary_search_history
        .retain(|s| !s.trim().is_empty());
    if settings.dictionary_search_history.len() > 30 {
        settings.dictionary_search_history.truncate(30);
    }
    if settings.wikipedia_language.trim().is_empty() {
        settings.wikipedia_language = "auto".to_string();
    }
    settings.rss_cooldown_blocked_secs = settings.rss_cooldown_blocked_secs.clamp(60, 86_400);
    settings.rss_cooldown_not_found_secs = settings.rss_cooldown_not_found_secs.clamp(300, 604_800);
    settings.rss_cooldown_rate_limited_secs =
        settings.rss_cooldown_rate_limited_secs.clamp(30, 3_600);
    normalize_rss_favorite_articles(&mut settings.rss_favorite_articles);
    normalize_voice_profiles(&mut settings);
    settings
}

fn normalize_shortcut_settings(shortcuts: &mut ShortcutSettings) {
    let legacy_execute_file = ShortcutBinding::new(false, true, false, VK_F5.0);
    let default_previous_sentence = ShortcutSettings::default().read_previous_sentence;
    let default_execute_file = ShortcutSettings::default().execute_file;
    if shortcuts.execute_file == legacy_execute_file
        && shortcuts.read_previous_sentence == default_previous_sentence
    {
        shortcuts.execute_file = default_execute_file;
    }
}

fn dpapi_protect(data: &[u8]) -> Option<Vec<u8>> {
    unsafe {
        let in_blob = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        let ok = CryptProtectData(
            &in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .is_ok();
        if !ok || out_blob.pbData.is_null() {
            return None;
        }
        let out = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        LocalFree(HLOCAL(out_blob.pbData as *mut std::ffi::c_void));
        Some(out)
    }
}

fn dpapi_unprotect(data: &[u8]) -> Option<Vec<u8>> {
    unsafe {
        let in_blob = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut out_blob = CRYPT_INTEGER_BLOB::default();
        let ok = CryptUnprotectData(
            &in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .is_ok();
        if !ok || out_blob.pbData.is_null() {
            return None;
        }
        let out = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        LocalFree(HLOCAL(out_blob.pbData as *mut std::ffi::c_void));
        Some(out)
    }
}

pub fn encrypt_podcast_index_secret(secret: &str) -> String {
    if secret.trim().is_empty() {
        return String::new();
    }
    dpapi_protect(secret.as_bytes())
        .map(hex::encode)
        .unwrap_or_default()
}

pub fn decrypt_podcast_index_secret(secret: &str) -> Option<String> {
    if secret.trim().is_empty() {
        return None;
    }
    let decoded = match hex::decode(secret) {
        Ok(decoded) => decoded,
        Err(_) => return Some(secret.to_string()),
    };
    let bytes = dpapi_unprotect(&decoded)?;
    String::from_utf8(bytes).ok()
}

pub fn encrypt_gemini_api_key(api_key: &str) -> String {
    if api_key.trim().is_empty() {
        return String::new();
    }
    dpapi_protect(api_key.as_bytes())
        .map(hex::encode)
        .unwrap_or_default()
}

pub fn decrypt_gemini_api_key(api_key: &str) -> Option<String> {
    if api_key.trim().is_empty() {
        return None;
    }
    let decoded = match hex::decode(api_key) {
        Ok(decoded) => decoded,
        Err(_) => return Some(api_key.to_string()),
    };
    let bytes = dpapi_unprotect(&decoded)?;
    String::from_utf8(bytes).ok()
}

pub fn encrypt_sonarpad_ai_access_code(code: &str) -> String {
    if code.trim().is_empty() {
        return String::new();
    }
    dpapi_protect(code.as_bytes())
        .map(hex::encode)
        .unwrap_or_default()
}

pub fn decrypt_sonarpad_ai_access_code(code: &str) -> Option<String> {
    if code.trim().is_empty() {
        return None;
    }
    let decoded = match hex::decode(code) {
        Ok(decoded) => decoded,
        Err(_) => return Some(code.to_string()),
    };
    let bytes = dpapi_unprotect(&decoded)?;
    String::from_utf8(bytes).ok()
}

pub fn encrypt_bdciechi_password(password: &str) -> String {
    if password.trim().is_empty() {
        return String::new();
    }
    dpapi_protect(password.as_bytes())
        .map(hex::encode)
        .unwrap_or_default()
}

pub fn decrypt_bdciechi_password(password: &str) -> Option<String> {
    if password.trim().is_empty() {
        return None;
    }
    let decoded = match hex::decode(password) {
        Ok(decoded) => decoded,
        Err(_) => return Some(password.to_string()),
    };
    let bytes = dpapi_unprotect(&decoded)?;
    String::from_utf8(bytes).ok()
}

fn serialize_ytdlp_site_credentials(credentials: &SavedYtdlpSiteCredentials) -> Option<String> {
    serde_json::to_string(credentials).ok()
}

fn deserialize_ytdlp_site_credentials(value: &str) -> Option<SavedYtdlpSiteCredentials> {
    serde_json::from_str(value).ok()
}

fn parse_ytdlp_site_credentials_entry(value: &str) -> Option<SavedYtdlpSiteCredentials> {
    deserialize_ytdlp_site_credentials(value)
        .or_else(|| decrypt_ytdlp_site_credentials_entry(value))
}

fn encrypt_ytdlp_site_credentials_entry(credentials: &SavedYtdlpSiteCredentials) -> String {
    let Some(serialized) = serialize_ytdlp_site_credentials(credentials) else {
        return String::new();
    };
    dpapi_protect(serialized.as_bytes())
        .map(hex::encode)
        .unwrap_or_default()
}

fn decrypt_ytdlp_site_credentials_entry(value: &str) -> Option<SavedYtdlpSiteCredentials> {
    if value.trim().is_empty() {
        return None;
    }
    let serialized = match hex::decode(value) {
        Ok(decoded) => {
            let bytes = dpapi_unprotect(&decoded)?;
            String::from_utf8(bytes).ok()?
        }
        Err(_) => value.to_string(),
    };
    deserialize_ytdlp_site_credentials(&serialized)
}

pub fn normalize_ytdlp_site_key(site: &str) -> Option<String> {
    let site = site.trim().to_ascii_lowercase();
    if site.is_empty() { None } else { Some(site) }
}

pub fn get_ytdlp_site_credentials(settings: &AppSettings, site: &str) -> Option<(String, String)> {
    let site = normalize_ytdlp_site_key(site)?;
    let value = settings.ytdlp_site_credentials.get(&site)?;
    let credentials = parse_ytdlp_site_credentials_entry(value)?;
    Some((credentials.username, credentials.password))
}

pub fn set_ytdlp_site_credentials(
    settings: &mut AppSettings,
    site: &str,
    username: &str,
    password: &str,
) -> bool {
    let Some(site) = normalize_ytdlp_site_key(site) else {
        return false;
    };
    let username = username.trim();
    let password = password.trim();
    if username.is_empty() || password.is_empty() {
        return false;
    }
    let credentials = SavedYtdlpSiteCredentials {
        username: username.to_string(),
        password: password.to_string(),
    };
    let Some(serialized) = serialize_ytdlp_site_credentials(&credentials) else {
        return false;
    };
    settings.ytdlp_site_credentials.insert(site, serialized);
    true
}

pub fn clear_ytdlp_site_credentials(settings: &mut AppSettings, site: &str) -> bool {
    let Some(site) = normalize_ytdlp_site_key(site) else {
        return false;
    };
    settings.ytdlp_site_credentials.remove(&site).is_some()
}

pub fn list_ytdlp_site_credentials(settings: &AppSettings) -> Vec<String> {
    let mut sites = settings
        .ytdlp_site_credentials
        .keys()
        .filter_map(|site| normalize_ytdlp_site_key(site))
        .collect::<Vec<_>>();
    sites.sort_unstable();
    sites.dedup();
    sites
}

pub fn clear_all_ytdlp_site_credentials(settings: &mut AppSettings) -> bool {
    if settings.ytdlp_site_credentials.is_empty() {
        return false;
    }
    settings.ytdlp_site_credentials.clear();
    true
}

pub fn encrypt_rai_luce_code(code: &str) -> String {
    if code.trim().is_empty() {
        return String::new();
    }
    dpapi_protect(code.as_bytes())
        .map(hex::encode)
        .unwrap_or_default()
}

pub fn decrypt_rai_luce_code(code: &str) -> Option<String> {
    if code.trim().is_empty() {
        return None;
    }
    let decoded = match hex::decode(code) {
        Ok(decoded) => decoded,
        Err(_) => return Some(code.to_string()),
    };
    let bytes = dpapi_unprotect(&decoded)?;
    String::from_utf8(bytes).ok()
}

fn cached_rai_luce_code() -> Option<String> {
    let cache = RAI_LUCE_CODE_CACHE.get_or_init(|| RwLock::new(None));
    cache.read().ok().and_then(|value| value.clone())
}

fn update_cached_rai_luce_code(code: &str) {
    let trimmed = code.trim();
    let cache = RAI_LUCE_CODE_CACHE.get_or_init(|| RwLock::new(None));
    if let Ok(mut value) = cache.write() {
        *value = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
}

fn load_saved_rai_luce_code_from_path(path: &std::path::Path) -> Option<String> {
    let data = std::fs::read_to_string(path).ok()?;
    let raw = data.trim();
    if raw.is_empty() {
        return None;
    }
    decrypt_rai_luce_code(raw)
}

fn load_saved_rai_luce_code_from_settings_file() -> Option<String> {
    let path = get_settings_path();
    let data = std::fs::read_to_string(&path).ok()?;
    let settings = serde_json::from_str::<AppSettings>(&data).ok()?;
    decrypt_rai_luce_code(&settings.rai_luce_code)
}

fn persist_rai_luce_backup_from_encrypted(encrypted_code: &str) {
    let path = rai_luce_backup_path();
    if encrypted_code.trim().is_empty() {
        return;
    }
    if let Err(e) = std::fs::write(&path, encrypted_code) {
        crate::log_debug(&format!(
            "Failed to save Rai Luce backup to {}: {}",
            path.display(),
            e
        ));
        log_rai_luce_event(&format!(
            "backup_write_failed path={} err={}",
            path.display(),
            e
        ));
    } else {
        log_rai_luce_event(&format!(
            "backup_saved path={} len={}",
            path.display(),
            encrypted_code.len()
        ));
    }
}

fn delete_rai_luce_backup() {
    let path = rai_luce_backup_path();
    match std::fs::remove_file(&path) {
        Ok(()) => {
            log_rai_luce_event(&format!("backup_deleted path={}", path.display()));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            crate::log_debug(&format!(
                "Failed to delete Rai Luce backup {}: {}",
                path.display(),
                err
            ));
            log_rai_luce_event(&format!(
                "backup_delete_failed path={} err={}",
                path.display(),
                err
            ));
        }
    }
}

pub fn request_explicit_rai_luce_clear() {
    RAI_LUCE_EXPLICIT_CLEAR_PENDING.store(true, Ordering::SeqCst);
    log_rai_luce_event("explicit Rai Luce clear requested");
}

pub fn load_saved_rai_luce_code() -> Option<String> {
    if let Some(code) = cached_rai_luce_code() {
        log_rai_luce_event(&format!(
            "load_saved_rai_luce_code source=cache len={}",
            code.len()
        ));
        return Some(code);
    }

    if let Some(code) = load_saved_rai_luce_code_from_settings_file() {
        log_rai_luce_event(&format!(
            "load_saved_rai_luce_code source=settings_json len={}",
            code.len()
        ));
        update_cached_rai_luce_code(&code);
        return Some(code);
    }

    if let Some(code) = load_saved_rai_luce_code_from_path(&rai_luce_backup_path()) {
        log_rai_luce_event(&format!(
            "load_saved_rai_luce_code source=backup len={}",
            code.len()
        ));
        update_cached_rai_luce_code(&code);
        return Some(code);
    }

    log_rai_luce_event("load_saved_rai_luce_code source=none");
    None
}

pub fn save_settings(settings: AppSettings) {
    apply_network_proxy_settings(&settings);
    let mut persisted = settings;
    let explicit_clear_requested = RAI_LUCE_EXPLICIT_CLEAR_PENDING.swap(false, Ordering::SeqCst);
    if persisted.rai_luce_code.trim().is_empty() {
        if explicit_clear_requested {
            log_rai_luce_event("save_settings honoring explicit Rai Luce clear request");
            update_cached_rai_luce_code("");
            delete_rai_luce_backup();
        } else if let Some(restored) = cached_rai_luce_code()
            .or_else(load_saved_rai_luce_code_from_settings_file)
            .or_else(|| load_saved_rai_luce_code_from_path(&rai_luce_backup_path()))
        {
            log_rai_luce_event(&format!(
                "save_settings preserved existing Rai Luce code len={}",
                restored.len()
            ));
            persisted.rai_luce_code = restored;
        } else {
            log_rai_luce_event(
                "save_settings incoming Rai Luce code is empty and nothing to preserve",
            );
        }
    } else {
        log_rai_luce_event(&format!(
            "save_settings incoming Rai Luce code len={}",
            persisted.rai_luce_code.len()
        ));
    }
    update_cached_rai_luce_code(&persisted.rai_luce_code);
    if persisted.remember_bdciechi_credentials {
        persisted.bdciechi_password = encrypt_bdciechi_password(&persisted.bdciechi_password);
    } else {
        persisted.bdciechi_username.clear();
        persisted.bdciechi_password.clear();
        persisted.bdciechi_last_successful_login_unix = 0;
    }
    persisted.ytdlp_site_credentials = persisted
        .ytdlp_site_credentials
        .into_iter()
        .filter_map(|(site, value)| {
            let site = normalize_ytdlp_site_key(&site)?;
            let mut credentials = match parse_ytdlp_site_credentials_entry(&value) {
                Some(credentials) => credentials,
                None => {
                    if value.trim().is_empty() {
                        return None;
                    }
                    crate::log_debug(&format!(
                        "Failed to parse yt-dlp credentials for site {}; preserving stored entry",
                        site
                    ));
                    return Some((site, value));
                }
            };
            credentials.username = credentials.username.trim().to_string();
            credentials.password = credentials.password.trim().to_string();
            if credentials.username.is_empty() || credentials.password.is_empty() {
                return None;
            }
            let encrypted = encrypt_ytdlp_site_credentials_entry(&credentials);
            if encrypted.is_empty() {
                crate::log_debug(&format!(
                    "Failed to encrypt yt-dlp credentials for site {}; preserving stored entry",
                    site
                ));
                if value.trim().is_empty() {
                    None
                } else {
                    Some((site, value))
                }
            } else {
                Some((site, encrypted))
            }
        })
        .collect();
    persisted.rai_luce_code = encrypt_rai_luce_code(&persisted.rai_luce_code);
    if !persisted.rai_luce_code.trim().is_empty() {
        persist_rai_luce_backup_from_encrypted(&persisted.rai_luce_code);
    } else if explicit_clear_requested {
        delete_rai_luce_backup();
    }
    persisted.gemini_api_key = encrypt_gemini_api_key(&persisted.gemini_api_key);
    persisted.sonarpad_ai_access_code =
        encrypt_sonarpad_ai_access_code(&persisted.sonarpad_ai_access_code);
    let path = get_settings_path();
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        crate::log_debug(&format!("Failed to create settings directory: {}", e));
    }
    match serde_json::to_string_pretty(&persisted) {
        Ok(json) => {
            let tmp_path = path.with_extension("json.tmp");
            let save_result = (|| -> std::io::Result<()> {
                let mut file = std::fs::File::create(&tmp_path)?;
                file.write_all(json.as_bytes())?;
                file.sync_all()?;
                match std::fs::rename(&tmp_path, &path) {
                    Ok(()) => Ok(()),
                    Err(rename_err) => {
                        if path.exists() {
                            std::fs::remove_file(&path)?;
                            std::fs::rename(&tmp_path, &path)
                        } else {
                            Err(rename_err)
                        }
                    }
                }
            })();
            if let Err(e) = save_result {
                let _cleanup_result = std::fs::remove_file(&tmp_path);
                crate::log_debug(&format!(
                    "Failed to save settings to {}: {}",
                    path.display(),
                    e
                ));
            }
        }
        Err(e) => {
            crate::log_debug(&format!("Failed to serialize settings: {}", e));
        }
    }
}

pub fn build_network_proxy_url(
    proxy_url: &str,
    proxy_port: &str,
) -> Result<Option<String>, String> {
    let proxy_url = proxy_url.trim();
    let proxy_port = proxy_port.trim();
    if proxy_url.is_empty() {
        if proxy_port.is_empty() {
            return Ok(None);
        }
        return Err("La porta del proxy richiede un indirizzo proxy".to_string());
    }

    let mut url = reqwest::Url::parse(proxy_url)
        .map_err(|err| format!("Indirizzo proxy non valido: {err}"))?;
    if !matches!(
        url.scheme(),
        "http" | "https" | "socks4" | "socks4a" | "socks5" | "socks5h"
    ) {
        return Err("Protocollo proxy non supportato".to_string());
    }
    if url.host_str().is_none() {
        return Err("L'indirizzo proxy deve contenere un host".to_string());
    }
    if !proxy_port.is_empty() {
        let port = proxy_port.parse::<u16>().map_err(|_| {
            "La porta del proxy deve essere un numero compreso tra 1 e 65535".to_string()
        })?;
        if port == 0 {
            return Err("La porta del proxy deve essere compresa tra 1 e 65535".to_string());
        }
        url.set_port(Some(port))
            .map_err(|_| "Impossibile impostare la porta del proxy".to_string())?;
    }
    Ok(Some(url.to_string()))
}

pub fn apply_network_proxy_settings(settings: &AppSettings) {
    let proxy =
        match build_network_proxy_url(&settings.network_proxy_url, &settings.network_proxy_port) {
            Ok(Some(proxy)) => proxy,
            Ok(None) => {
                clear_network_proxy_environment();
                return;
            }
            Err(err) => {
                crate::log_debug(&format!("Invalid network proxy configuration: {err}"));
                clear_network_proxy_environment();
                return;
            }
        };

    let username = settings.network_proxy_username.trim();
    let password = settings.network_proxy_password.trim();
    let proxy_with_auth = if !username.is_empty() {
        inject_proxy_credentials(&proxy, username, password).unwrap_or(proxy)
    } else {
        proxy
    };

    unsafe {
        std::env::set_var("HTTP_PROXY", &proxy_with_auth);
        std::env::set_var("HTTPS_PROXY", &proxy_with_auth);
        std::env::set_var("ALL_PROXY", &proxy_with_auth);
        std::env::set_var("http_proxy", &proxy_with_auth);
        std::env::set_var("https_proxy", &proxy_with_auth);
        std::env::set_var("all_proxy", &proxy_with_auth);
    }
}

fn clear_network_proxy_environment() {
    unsafe {
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("ALL_PROXY");
        std::env::remove_var("http_proxy");
        std::env::remove_var("https_proxy");
        std::env::remove_var("all_proxy");
    }
}

fn inject_proxy_credentials(proxy: &str, username: &str, password: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(proxy).ok()?;
    if url.set_username(username).is_err() {
        return None;
    }
    if password.is_empty() {
        let _password_clear_result = url.set_password(None);
    } else if url.set_password(Some(password)).is_err() {
        return None;
    }
    Some(url.to_string())
}

#[cfg(test)]
mod network_proxy_tests {
    use super::build_network_proxy_url;

    #[test]
    fn proxy_port_is_added_to_url() {
        let url = build_network_proxy_url("http://proxy.example", "8080")
            .expect("valid proxy")
            .expect("configured proxy");
        assert_eq!(url, "http://proxy.example:8080/");
    }

    #[test]
    fn proxy_port_replaces_existing_port() {
        let url = build_network_proxy_url("https://proxy.example:3128", "8443")
            .expect("valid proxy")
            .expect("configured proxy");
        assert_eq!(url, "https://proxy.example:8443/");
    }

    #[test]
    fn proxy_port_must_be_in_valid_range() {
        assert!(build_network_proxy_url("http://proxy.example", "0").is_err());
        assert!(build_network_proxy_url("http://proxy.example", "65536").is_err());
        assert!(build_network_proxy_url("http://proxy.example", "abc").is_err());
    }

    #[test]
    fn proxy_port_is_optional() {
        let url = build_network_proxy_url("http://proxy.example:3128", "")
            .expect("valid proxy")
            .expect("configured proxy");
        assert_eq!(url, "http://proxy.example:3128/");
        assert!(
            build_network_proxy_url("", "")
                .expect("empty proxy")
                .is_none()
        );
    }
}

#[cfg(test)]
mod voice_profile_tests {
    use super::{
        AppSettings, DEFAULT_VOICE_PROFILE_NAME, TtsEngine, VoiceProfile, normalize_voice_profiles,
        sync_active_voice_profile_from_settings_fields,
    };

    #[test]
    fn syncing_active_profile_preserves_google_selection_after_reload_normalization() {
        let mut settings = AppSettings {
            active_voice_profile: DEFAULT_VOICE_PROFILE_NAME.to_string(),
            voice_profiles: vec![VoiceProfile {
                name: DEFAULT_VOICE_PROFILE_NAME.to_string(),
                tts_engine: TtsEngine::Sapi4,
                tts_voice: "SAPI4 voice".to_string(),
                ..Default::default()
            }],
            tts_engine: TtsEngine::Google,
            tts_voice: "Google italiano 6 (Natural)".to_string(),
            ..Default::default()
        };

        sync_active_voice_profile_from_settings_fields(&mut settings);

        let active_profile = settings
            .voice_profiles
            .iter()
            .find(|profile| profile.name == DEFAULT_VOICE_PROFILE_NAME)
            .expect("default voice profile");
        assert!(active_profile.tts_engine == TtsEngine::Google);
        assert_eq!(active_profile.tts_voice, "Google italiano 6 (Natural)");

        normalize_voice_profiles(&mut settings);
        assert!(settings.tts_engine == TtsEngine::Google);
        assert_eq!(settings.tts_voice, "Google italiano 6 (Natural)");
    }
}

pub fn save_settings_with_default_copy(settings: AppSettings, _keep_default_copy: bool) {
    save_settings(settings);
}

const CONTEXT_MENU_EXTENSIONS: &[&str] = &[
    "txt", "md", "pdf", "epub", "mp3", "m4a", "mp4", "aac", "mkv", "avi", "mov", "m4v", "webm",
    "mpg", "mpeg", "ts", "m2ts", "mts", "wmv", "asf", "flv", "vob", "3gp", "flac", "ogg", "opus",
    "wma", "aiff", "m4b", "doc", "docx", "xls", "xlsx", "rtf", "htm", "html", "ppt", "pptx", "py",
    "java", "js", "rb", "pl", "php", "lua", "ps1", "sh", "gdoc", "gsheet", "gslides",
];

pub fn register_application_capabilities() {
    let exe_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(err) => {
            crate::log_debug(&format!("Capabilities: failed to get exe path: {err}"));
            return;
        }
    };
    let exe_path_str = exe_path.to_string_lossy();
    let app_name = "Sonarpad";
    let prog_id_prefix = "Sonarpad.Assoc";

    // 1. Register App Class and Program IDs for each extension
    for ext in CONTEXT_MENU_EXTENSIONS {
        let prog_id = format!("{}.{}", prog_id_prefix, ext);
        let class_key = format!("Software\\Classes\\{}", prog_id);
        if let Some(key) = create_registry_key(&class_key) {
            set_registry_string_value(key, None, &format!("{} Document", app_name));
            unsafe {
                RegCloseKey(key);
            }
        }

        let shell_open_key = format!("{}\\shell\\open\\command", class_key);
        if let Some(key) = create_registry_key(&shell_open_key) {
            set_registry_string_value(key, None, &format!("\"{}\" \"%1\"", exe_path_str));
            unsafe {
                RegCloseKey(key);
            }
        }

        let icon_key = format!("{}\\DefaultIcon", class_key);
        if let Some(key) = create_registry_key(&icon_key) {
            set_registry_string_value(key, None, &format!("\"{}\",0", exe_path_str));
            unsafe {
                RegCloseKey(key);
            }
        }
    }

    // 2. Register Application Capabilities
    let capabilities_key = "Software\\Sonarpad\\Capabilities";
    if let Some(key) = create_registry_key(capabilities_key) {
        set_registry_string_value(key, Some("ApplicationName"), app_name);
        set_registry_string_value(
            key,
            Some("ApplicationDescription"),
            "A modern Notepad with PDF, DOCX, XLS and EPUB support and TTS capabilities.",
        );
        unsafe {
            RegCloseKey(key);
        }
    }

    let file_assoc_key = format!("{}\\FileAssociations", capabilities_key);
    if let Some(key) = create_registry_key(&file_assoc_key) {
        for ext in CONTEXT_MENU_EXTENSIONS {
            let prog_id = format!("{}.{}", prog_id_prefix, ext);
            set_registry_string_value(key, Some(&format!(".{}", ext)), &prog_id);
        }
        unsafe {
            RegCloseKey(key);
        }
    }

    // 3. Register in RegisteredApplications
    if let Some(key) = create_registry_key("Software\\RegisteredApplications") {
        set_registry_string_value(key, Some(app_name), capabilities_key);
        unsafe {
            RegCloseKey(key);
        }
    }
}

pub fn sync_context_menu(settings: &AppSettings) {
    let label = crate::i18n::tr(settings.language, "context_menu.open_with");
    let exe_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(err) => {
            crate::log_debug(&format!("Context menu: failed to get exe path: {err}"));
            return;
        }
    };
    let exe_path_str = exe_path.to_string_lossy();
    let command = format!("\"{}\" \"%1\"", exe_path_str);
    let icon = format!("\"{}\",0", exe_path_str);

    for ext in CONTEXT_MENU_EXTENSIONS {
        let base_key = format!(
            "Software\\Classes\\SystemFileAssociations\\.{}\\shell\\OpenWithSonarpad",
            ext
        );
        if settings.context_menu_open_with {
            create_context_menu_entry(&base_key, &label, &command, &icon);
        } else {
            delete_context_menu_entry(&base_key);
        }
    }
}

pub fn cleanup_legacy_context_menu_entries() {
    for ext in CONTEXT_MENU_EXTENSIONS {
        let base_key = format!(
            "Software\\Classes\\SystemFileAssociations\\.{}\\shell\\OpenWithNovapad",
            ext
        );
        delete_context_menu_entry(&base_key);
    }
}

pub fn sync_start_menu_shortcuts(settings: &AppSettings) {
    let (from, to) = if settings.use_legacy_name {
        ("Sonarpad", "Novapad")
    } else {
        ("Novapad", "Sonarpad")
    };
    for dir in start_menu_program_dirs() {
        rename_start_menu_entries(&dir, from, to);
    }
}

fn start_menu_program_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        dirs.push(
            PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    if let Ok(program_data) = std::env::var("ProgramData") {
        dirs.push(
            PathBuf::from(program_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    dirs
}

fn rename_start_menu_entries(dir: &std::path::Path, from: &str, to: &str) {
    if !dir.exists() {
        return;
    }
    let from_link = dir.join(format!("{from}.lnk"));
    let to_link = dir.join(format!("{to}.lnk"));
    if from_link.exists()
        && !to_link.exists()
        && let Err(e) = std::fs::rename(&from_link, &to_link)
    {
        crate::log_debug(&format!(
            "start_menu_rename_failed {} -> {}: {}",
            from_link.display(),
            to_link.display(),
            e
        ));
    }

    let from_folder = dir.join(from);
    let to_folder = dir.join(to);
    if from_folder.exists()
        && !to_folder.exists()
        && let Err(e) = std::fs::rename(&from_folder, &to_folder)
    {
        crate::log_debug(&format!(
            "start_menu_folder_rename_failed {} -> {}: {}",
            from_folder.display(),
            to_folder.display(),
            e
        ));
        return;
    }

    // If folder exists (either renamed or already there), rename inner shortcut.
    let target_folder = if to_folder.exists() {
        to_folder
    } else {
        from_folder
    };
    if target_folder.exists() {
        let inner_from = target_folder.join(format!("{from}.lnk"));
        let inner_to = target_folder.join(format!("{to}.lnk"));
        if inner_from.exists()
            && !inner_to.exists()
            && let Err(e) = std::fs::rename(&inner_from, &inner_to)
        {
            crate::log_debug(&format!(
                "start_menu_inner_rename_failed {} -> {}: {}",
                inner_from.display(),
                inner_to.display(),
                e
            ));
        }
    }
}

fn create_context_menu_entry(base_key: &str, label: &str, command: &str, icon: &str) {
    if let Some(key) = create_registry_key(base_key) {
        set_registry_string_value(key, None, label);
        unsafe {
            RegCloseKey(key);
        }
    }

    let icon_key = format!("{base_key}\\DefaultIcon");
    if let Some(key) = create_registry_key(&icon_key) {
        set_registry_string_value(key, None, icon);
        unsafe {
            RegCloseKey(key);
        }
    }

    let command_key = format!("{base_key}\\command");
    if let Some(key) = create_registry_key(&command_key) {
        set_registry_string_value(key, None, command);
        unsafe {
            RegCloseKey(key);
        }
    }
}

fn delete_context_menu_entry(base_key: &str) {
    let base_key_wide = to_wide(base_key);
    let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(base_key_wide.as_ptr())) };
    if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
        crate::log_debug(&format!(
            "Context menu: failed to delete key {base_key}: {status:?}"
        ));
    }
}

fn create_registry_key(path: &str) -> Option<windows::Win32::System::Registry::HKEY> {
    let path_wide = to_wide(path);
    let mut key = windows::Win32::System::Registry::HKEY::default();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path_wide.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
    };
    if status == ERROR_SUCCESS {
        Some(key)
    } else {
        crate::log_debug(&format!(
            "Context menu: failed to create key {path}: {status:?}"
        ));
        None
    }
}

fn set_registry_string_value(
    key: windows::Win32::System::Registry::HKEY,
    value_name: Option<&str>,
    value: &str,
) -> bool {
    let value_wide = to_wide(value);
    let name_wide;
    let name_ptr = if let Some(name) = value_name {
        name_wide = to_wide(name);
        PCWSTR(name_wide.as_ptr())
    } else {
        PCWSTR::null()
    };
    let value_bytes = unsafe {
        std::slice::from_raw_parts(value_wide.as_ptr() as *const u8, value_wide.len() * 2)
    };
    let status = unsafe { RegSetValueExW(key, name_ptr, 0, REG_SZ, Some(value_bytes)) };
    if status != ERROR_SUCCESS {
        crate::log_debug(&format!("Context menu: failed to set value: {status:?}"));
    }
    status == ERROR_SUCCESS
}

pub fn confirm_title(language: Language) -> String {
    crate::i18n::tr(language, "app.confirm_title")
}

pub fn error_title(language: Language) -> String {
    crate::i18n::tr(language, "app.error_title")
}

pub fn tts_no_text_message(language: Language) -> String {
    crate::i18n::tr(language, "app.tts_no_text")
}

pub fn move_podcast_feed_up(settings: &mut AppSettings, index: usize) -> Option<usize> {
    if index == 0 || index >= settings.podcast_sources.len() {
        return None;
    }
    settings.podcast_sources.swap(index, index - 1);
    Some(index - 1)
}

pub fn move_podcast_feed_down(settings: &mut AppSettings, index: usize) -> Option<usize> {
    if index + 1 >= settings.podcast_sources.len() {
        return None;
    }
    settings.podcast_sources.swap(index, index + 1);
    Some(index + 1)
}

pub fn move_podcast_feed_to_top(settings: &mut AppSettings, index: usize) -> Option<usize> {
    move_podcast_feed_to_index(settings, index, 0)
}

pub fn move_podcast_feed_to_bottom(settings: &mut AppSettings, index: usize) -> Option<usize> {
    let len = settings.podcast_sources.len();
    if len == 0 {
        return None;
    }
    move_podcast_feed_to_index(settings, index, len - 1)
}

pub fn move_podcast_feed_to_index(
    settings: &mut AppSettings,
    index: usize,
    target_index: usize,
) -> Option<usize> {
    let len = settings.podcast_sources.len();
    if index >= len {
        return None;
    }
    let target = target_index.min(len.saturating_sub(1));
    if target == index {
        return Some(index);
    }
    let item = settings.podcast_sources.remove(index);
    settings.podcast_sources.insert(target, item);
    Some(target)
}

pub fn audiobook_done_title(language: Language) -> String {
    crate::i18n::tr(language, "app.audiobook_done_title")
}

pub fn info_title(language: Language) -> String {
    crate::i18n::tr(language, "app.info_title")
}

pub fn pdf_loaded_message(language: Language) -> String {
    crate::i18n::tr(language, "app.pdf_loaded")
}

pub fn text_not_found_message(language: Language) -> String {
    crate::i18n::tr(language, "app.text_not_found")
}

pub fn find_title(language: Language) -> String {
    crate::i18n::tr(language, "app.find_title")
}

pub fn error_open_file_message(language: Language, _err: impl std::fmt::Display) -> String {
    crate::i18n::tr_f(
        language,
        "app.error_open_file",
        &[("err", &format!("{_err}"))],
    )
}

pub fn error_save_file_message(language: Language, _err: impl std::fmt::Display) -> String {
    crate::i18n::tr_f(
        language,
        "app.error_save_file",
        &[("err", &format!("{_err}"))],
    )
}

pub fn confirm_save_message(language: Language, title: &str) -> String {
    crate::i18n::tr_f(language, "app.confirm_save", &[("title", title)])
}

pub fn untitled_base(language: Language) -> String {
    crate::i18n::tr(language, "app.untitled_base")
}

pub fn untitled_title(language: Language, number: usize) -> String {
    let base = untitled_base(language);
    if number == 0 {
        base
    } else {
        format!("{} {}", base, number)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortOrder {
    TitleAsc,
    TitleDesc,
    DateNewest,
    DateOldest,
}

pub fn sort_rss_sources(settings: &mut AppSettings, order: SortOrder) {
    settings.rss_sources.sort_by(|a, b| match order {
        SortOrder::TitleAsc => {
            let ta = &a.title;
            let tb = &b.title;
            ta.to_lowercase().cmp(&tb.to_lowercase())
        }
        SortOrder::TitleDesc => {
            let ta = &a.title;
            let tb = &b.title;
            tb.to_lowercase().cmp(&ta.to_lowercase())
        }
        SortOrder::DateNewest => b
            .last_updated
            .unwrap_or(0)
            .cmp(&a.last_updated.unwrap_or(0)),
        SortOrder::DateOldest => a
            .last_updated
            .unwrap_or(0)
            .cmp(&b.last_updated.unwrap_or(0)),
    });
}

pub fn sort_podcast_sources(settings: &mut AppSettings, order: SortOrder) {
    settings.podcast_sources.sort_by(|a, b| match order {
        SortOrder::TitleAsc => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
        SortOrder::TitleDesc => b.title.to_lowercase().cmp(&a.title.to_lowercase()),
        SortOrder::DateNewest => b
            .last_updated
            .unwrap_or(0)
            .cmp(&a.last_updated.unwrap_or(0)),
        SortOrder::DateOldest => a
            .last_updated
            .unwrap_or(0)
            .cmp(&b.last_updated.unwrap_or(0)),
    });
}

#[cfg(test)]
mod audio_description_save_folder_tests {
    use std::path::PathBuf;

    use super::{AppSettings, default_audio_description_save_folder, normalize_settings};

    #[test]
    fn omni_port_audio_description_default_folder_is_under_sonarpad_documents() {
        let path = PathBuf::from(default_audio_description_save_folder());
        assert!(path.ends_with(PathBuf::from("Sonarpad").join("Audiodescriptions")));
    }

    #[test]
    fn omni_port_empty_audio_description_folder_is_restored_to_default() {
        let settings = normalize_settings(AppSettings {
            audio_description_save_folder: "   ".to_string(),
            ..Default::default()
        });
        assert_eq!(
            settings.audio_description_save_folder,
            default_audio_description_save_folder()
        );
    }

    #[test]
    fn omni_port_audio_description_preferences_have_legacy_compatible_defaults() {
        let settings = AppSettings::default();
        assert!(settings.audio_description_language.is_none());
        assert!(matches!(
            settings.audio_description_tts_engine,
            super::TtsEngine::Edge
        ));
        assert!(settings.audio_description_tts_voice.is_empty());
        assert_eq!(settings.audio_description_verbosity, 2);
        assert!(settings.audio_description_recent_project_folders.is_empty());
        assert!(!settings.audio_description_extended_pauses);
        assert!(settings.audio_description_recognize_characters);
        assert!(settings.audio_description_recognize_screen_text);
        assert!(!settings.audio_description_keep_character_catalog);
        assert!(settings.audio_description_character_catalog.is_empty());
        assert!(!settings.audio_description_save_project);
        assert!(!settings.audio_description_create_video);
        assert!(!settings.audio_description_delete_video_after);
        assert!(!settings.audio_description_after_tv_recording);
    }

    #[test]
    fn omni_port_audio_description_preferences_are_normalized() {
        let settings = normalize_settings(AppSettings {
            audio_description_tts_voice: "  it-IT-IsabellaNeural  ".to_string(),
            audio_description_character_catalog: "  My Series  ".to_string(),
            audio_description_verbosity: 9,
            ..Default::default()
        });
        assert_eq!(settings.audio_description_tts_voice, "it-IT-IsabellaNeural");
        assert_eq!(settings.audio_description_character_catalog, "My Series");
        assert_eq!(settings.audio_description_verbosity, 2);
    }
    #[test]
    fn omni_port_character_catalog_is_disabled_when_character_recognition_is_off() {
        let settings = normalize_settings(AppSettings {
            audio_description_recognize_characters: false,
            audio_description_keep_character_catalog: true,
            audio_description_character_catalog: "Series".to_string(),
            ..Default::default()
        });
        assert!(!settings.audio_description_keep_character_catalog);
        assert_eq!(settings.audio_description_character_catalog, "Series");
    }
}
