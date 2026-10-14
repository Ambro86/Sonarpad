use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::sync::mpsc;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{WC_BUTTON, WC_COMBOBOXW, WC_STATIC};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    CB_ADDSTRING, CB_GETCURSEL, CB_GETITEMDATA, CB_RESETCONTENT, CB_SETCURSEL, CB_SETITEMDATA,
    CBN_SELCHANGE, CBS_DROPDOWNLIST, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GWLP_USERDATA, GetDlgItem, GetMessageW, GetWindowLongPtrW, HMENU, IDC_ARROW,
    IsDialogMessageW, IsWindow, MSG, RegisterClassW, SW_SHOW, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
    WM_COMMAND, WM_CREATE, WM_KEYDOWN, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE,
    WS_EX_DLGMODALFRAME, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::PCWSTR;

use crate::settings::{Language, TtsEngine, VoiceInfo};
use crate::{i18n, to_wide, with_state};

const ID_LANGUAGE: i32 = 1206;
const ID_LANGUAGE_LABEL: i32 = 1207;
const ID_ENGINE: i32 = 1201;
const ID_VOICE: i32 = 1202;
const ID_RATE: i32 = 1203;
const ID_VOLUME: i32 = 1204;
const ID_TEST: i32 = 1205;
const ID_PITCH: i32 = 1208;
const ID_OK: i32 = 1;
const ID_CANCEL: i32 = 2;

#[derive(Clone)]
pub struct AudioDescriptionVoiceSources {
    pub edge: Vec<VoiceInfo>,
    pub sapi5: Vec<VoiceInfo>,
    pub sapi4: Vec<VoiceInfo>,
    pub google: Vec<VoiceInfo>,
}

impl AudioDescriptionVoiceSources {
    fn voices(&self, engine: TtsEngine) -> &[VoiceInfo] {
        match engine {
            TtsEngine::Edge => &self.edge,
            TtsEngine::Sapi5 => &self.sapi5,
            TtsEngine::Sapi4 => &self.sapi4,
            TtsEngine::Google => &self.google,
        }
    }

    pub fn voices_for(&self, engine: TtsEngine) -> Vec<VoiceInfo> {
        self.voices(engine).to_vec()
    }
}

#[derive(Clone)]
pub struct AudioDescriptionVoiceSettings {
    pub engine: TtsEngine,
    pub voice: String,
    pub rate: i32,
    pub pitch: i32,
    pub volume: i32,
}

struct DialogData {
    language: Language,
    main_parent: HWND,
    sources: AudioDescriptionVoiceSources,
    default: AudioDescriptionVoiceSettings,
    pitch_engine: TtsEngine,
    voice_languages: Vec<String>,
    result: Option<AudioDescriptionVoiceSettings>,
}

fn engine_index(engine: TtsEngine) -> usize {
    match engine {
        TtsEngine::Edge => 0,
        TtsEngine::Sapi5 => 1,
        TtsEngine::Sapi4 => 2,
        TtsEngine::Google => 3,
    }
}

fn selected_engine(hwnd: HWND) -> TtsEngine {
    match unsafe {
        SendMessageW(
            GetDlgItem(hwnd, ID_ENGINE),
            CB_GETCURSEL,
            WPARAM(0),
            LPARAM(0),
        )
        .0
    } {
        1 => TtsEngine::Sapi5,
        2 => TtsEngine::Sapi4,
        3 => TtsEngine::Google,
        _ => TtsEngine::Edge,
    }
}

fn add_combo_item(combo: HWND, text: &str, data: isize) {
    unsafe {
        let wide = to_wide(text);
        let index = SendMessageW(
            combo,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        )
        .0;
        if index >= 0 {
            SendMessageW(combo, CB_SETITEMDATA, WPARAM(index as usize), LPARAM(data));
        }
    }
}

fn fill_engine_combo(hwnd: HWND, language: Language, selected: TtsEngine) {
    let combo = unsafe { GetDlgItem(hwnd, ID_ENGINE) };
    unsafe { SendMessageW(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0)) };
    for (index, key) in [
        "options.engine.edge",
        "options.engine.sapi5",
        "options.engine.sapi4",
        "options.engine.google",
    ]
    .iter()
    .enumerate()
    {
        add_combo_item(combo, &i18n::tr(language, key), index as isize);
    }
    unsafe {
        SendMessageW(
            combo,
            CB_SETCURSEL,
            WPARAM(engine_index(selected)),
            LPARAM(0),
        );
    }
}

fn voice_language_code(voice: &VoiceInfo) -> String {
    voice
        .locale
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

fn fill_language_combo(hwnd: HWND, data: &DialogData) {
    let combo = unsafe { GetDlgItem(hwnd, ID_LANGUAGE) };
    add_combo_item(
        combo,
        &i18n::tr(data.language, "google_tts.voices.all_languages"),
        -1,
    );
    let preferred = data
        .sources
        .edge
        .iter()
        .find(|voice| voice.short_name.eq_ignore_ascii_case(&data.default.voice))
        .map(voice_language_code)
        .unwrap_or_else(|| super::locale_display_names::app_locale(data.language).to_string());
    let mut selected = 0;
    for (index, code) in data.voice_languages.iter().enumerate() {
        let key = format!("voice.lang.{code}");
        let translated = i18n::tr(data.language, &key);
        let label = if translated != key {
            translated
        } else {
            super::locale_display_names::language_name(data.language, code)
                .unwrap_or_else(|| code.to_ascii_uppercase())
        };
        add_combo_item(combo, &label, index as isize);
        if code == &preferred {
            selected = index + 1;
        }
    }
    unsafe {
        SendMessageW(combo, CB_SETCURSEL, WPARAM(selected), LPARAM(0));
    }
}

fn selected_language(hwnd: HWND, data: &DialogData) -> Option<&str> {
    let combo = unsafe { GetDlgItem(hwnd, ID_LANGUAGE) };
    let selected = unsafe { SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
    if selected < 0 {
        return None;
    }
    let index =
        unsafe { SendMessageW(combo, CB_GETITEMDATA, WPARAM(selected as usize), LPARAM(0)).0 };
    if index < 0 {
        return None;
    }
    data.voice_languages.get(index as usize).map(String::as_str)
}

fn fill_voice_combo(hwnd: HWND, preferred: &str) {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogData };
    if pointer.is_null() {
        return;
    }
    let data = unsafe { &*pointer };
    let engine = selected_engine(hwnd);
    let combo = unsafe { GetDlgItem(hwnd, ID_VOICE) };
    unsafe { SendMessageW(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0)) };
    let voices = data.sources.voices(engine);
    let filter = if engine == TtsEngine::Edge {
        selected_language(hwnd, data)
    } else {
        None
    };
    unsafe {
        EnableWindow(GetDlgItem(hwnd, ID_LANGUAGE), engine == TtsEngine::Edge);
        EnableWindow(
            GetDlgItem(hwnd, ID_LANGUAGE_LABEL),
            engine == TtsEngine::Edge,
        );
    }
    let mut visible_count = 0;
    let mut selected_index = 0usize;
    for (index, voice) in voices.iter().enumerate() {
        if filter.is_some_and(|code| voice_language_code(voice) != code) {
            continue;
        }
        let label = if voice.locale.trim().is_empty() {
            voice.short_name.clone()
        } else {
            format!("{} ({})", voice.short_name, voice.locale)
        };
        add_combo_item(combo, &label, index as isize);
        if voice.short_name.eq_ignore_ascii_case(preferred) {
            selected_index = visible_count;
        }
        visible_count += 1;
    }
    if visible_count > 0 {
        unsafe {
            SendMessageW(combo, CB_SETCURSEL, WPARAM(selected_index), LPARAM(0));
        }
    }
}

fn selected_voice(hwnd: HWND) -> String {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogData };
    if pointer.is_null() {
        return String::new();
    }
    let data = unsafe { &*pointer };
    let engine = selected_engine(hwnd);
    let combo = unsafe { GetDlgItem(hwnd, ID_VOICE) };
    let selected = unsafe { SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
    if selected < 0 {
        return String::new();
    }
    let item =
        unsafe { SendMessageW(combo, CB_GETITEMDATA, WPARAM(selected as usize), LPARAM(0)).0 };
    if item < 0 {
        return String::new();
    }
    data.sources
        .voices(engine)
        .get(item as usize)
        .map(|voice| voice.short_name.clone())
        .unwrap_or_default()
}

pub(crate) fn rate_items(language: Language) -> Vec<(String, i32)> {
    vec![
        (i18n::tr(language, "tts_tuning.speed.extremely_slow"), -100),
        (i18n::tr(language, "tts_tuning.speed.very_slow"), -60),
        (i18n::tr(language, "tts_tuning.speed.slow"), -35),
        (i18n::tr(language, "tts_tuning.speed.a_bit_slow"), -20),
        (i18n::tr(language, "tts_tuning.speed.slightly_slow"), -10),
        (i18n::tr(language, "tts_tuning.speed.normal"), 0),
        (i18n::tr(language, "tts_tuning.speed.slightly_fast"), 10),
        (i18n::tr(language, "tts_tuning.speed.a_bit_fast"), 20),
        (i18n::tr(language, "tts_tuning.speed.fast"), 35),
        (i18n::tr(language, "tts_tuning.speed.very_fast"), 50),
        (i18n::tr(language, "tts_tuning.speed.super_fast"), 100),
    ]
}

pub(crate) fn volume_items(language: Language) -> Vec<(String, i32)> {
    vec![
        (i18n::tr(language, "tts_tuning.volume.very_low"), 25),
        (i18n::tr(language, "tts_tuning.volume.low"), 40),
        (i18n::tr(language, "tts_tuning.volume.a_bit_low"), 55),
        (i18n::tr(language, "tts_tuning.volume.medium_low"), 70),
        (i18n::tr(language, "tts_tuning.volume.slightly_low"), 85),
        (i18n::tr(language, "tts_tuning.volume.normal"), 100),
        (i18n::tr(language, "tts_tuning.volume.slightly_high"), 115),
        (i18n::tr(language, "tts_tuning.volume.medium_high"), 130),
        (i18n::tr(language, "tts_tuning.volume.a_bit_high"), 145),
        (i18n::tr(language, "tts_tuning.volume.high"), 160),
        (i18n::tr(language, "tts_tuning.volume.very_high"), 180),
        (i18n::tr(language, "tts_tuning.volume.maximum"), 200),
    ]
}

fn convert_pitch_engine(value: i32, from: TtsEngine, to: TtsEngine) -> i32 {
    if from == to {
        return value;
    }
    if to == TtsEngine::Google {
        return crate::google_tts::normalize_google_pitch_internal(value);
    }
    if from == TtsEngine::Google {
        return ((crate::google_tts::google_pitch_percent_from_internal(value) as f32 * 24.0
            / 100.0)
            - 12.0)
            .round() as i32;
    }
    value.clamp(-12, 12)
}

fn fill_pitch_combo(hwnd: HWND, language: Language, engine: TtsEngine, pitch: i32) {
    let presets = [
        ("very_low", -12),
        ("low", -10),
        ("a_bit_low", -7),
        ("slightly_low", -5),
        ("a_little_lower", -2),
        ("normal", 0),
        ("a_little_higher", 2),
        ("slightly_high", 5),
        ("a_bit_high", 7),
        ("high", 9),
        ("very_high", 12),
    ];
    let mut values: Vec<_> = presets
        .into_iter()
        .map(|(label, value)| {
            (
                i18n::tr(language, &format!("tts_tuning.pitch.{label}")),
                if engine == TtsEngine::Google {
                    crate::google_tts::google_pitch_preset_internal(value)
                } else {
                    value
                },
            )
        })
        .collect();
    let selected = if engine == TtsEngine::Google {
        crate::google_tts::normalize_google_pitch_internal(pitch)
    } else {
        pitch.clamp(-12, 12)
    };
    // Opening and confirming the dialog must preserve an existing custom pitch.
    if !values.iter().any(|(_, value)| *value == selected) {
        let display = if engine == TtsEngine::Google {
            crate::google_tts::google_pitch_percent_from_internal(selected)
        } else {
            selected
        };
        values.push((
            format!(
                "{} ({display})",
                i18n::tr(language, "tts_tuning.label_pitch")
            ),
            selected,
        ));
    }
    fill_value_combo(unsafe { GetDlgItem(hwnd, ID_PITCH) }, &values, selected);
}

fn fill_value_combo(combo: HWND, values: &[(String, i32)], selected: i32) {
    unsafe { SendMessageW(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0)) };
    let mut selected_index = 0usize;
    let mut best_distance = i32::MAX;
    for (index, (label, value)) in values.iter().enumerate() {
        add_combo_item(combo, label, *value as isize);
        let distance = (*value - selected).abs();
        if distance < best_distance {
            selected_index = index;
            best_distance = distance;
        }
    }
    if !values.is_empty() {
        unsafe {
            SendMessageW(combo, CB_SETCURSEL, WPARAM(selected_index), LPARAM(0));
        }
    }
}

fn selected_value(combo: HWND, fallback: i32) -> i32 {
    let selected = unsafe { SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
    if selected < 0 {
        return fallback;
    }
    // The selected item is valid; -1 is also a legitimate custom pitch, even
    // though it shares the numeric value of CB_ERR.
    unsafe { SendMessageW(combo, CB_GETITEMDATA, WPARAM(selected as usize), LPARAM(0)).0 as i32 }
}

pub(crate) fn preview_voice_settings(
    main_parent: HWND,
    language: Language,
    engine: TtsEngine,
    voice: String,
    rate: i32,
    pitch: i32,
    volume: i32,
) {
    if voice.trim().is_empty() {
        return;
    }
    let text = i18n::tr(language, "tts.preview_text");
    if text.trim().is_empty() {
        return;
    }
    let (split_on_newline, dictionary) = with_state(main_parent, |state| {
        (
            state.settings.split_on_newline,
            state.settings.dictionary.clone(),
        )
    })
    .unwrap_or((true, Vec::new()));
    let chunks =
        crate::tts_engine::split_into_tts_chunks(&text, split_on_newline, &dictionary, engine);

    match engine {
        TtsEngine::Edge | TtsEngine::Google => {
            crate::tts_engine::start_tts_playback_with_chunks(
                crate::tts_engine::TtsPlaybackOptions {
                    hwnd: main_parent,
                    engine,
                    cleaned: text,
                    voice,
                    chunks,
                    initial_caret_pos: 0,
                    source_edit: HWND(0),
                    rate,
                    pitch,
                    volume,
                },
            );
        }
        TtsEngine::Sapi4 => {
            crate::tts_engine::stop_tts_playback(main_parent);
            let voice_index = if let Some(hash_pos) = voice.find('#') {
                let rest = &voice[hash_pos + 1..];
                rest.split('|')
                    .next()
                    .and_then(|value| value.parse::<i32>().ok())
                    .unwrap_or(1)
            } else {
                1
            };
            let cancel = Arc::new(AtomicBool::new(false));
            let (command_tx, command_rx) = mpsc::unbounded_channel();
            if with_state(main_parent, |state| {
                state.tts_session = Some(crate::tts_engine::TtsSession {
                    id: state.tts_next_session_id,
                    command_tx,
                    cancel: cancel.clone(),
                    paused: false,
                    initial_caret_pos: 0,
                    source_edit: HWND(0),
                });
                state.tts_next_session_id += 1;
            })
            .is_none()
            {
                crate::log_debug(
                    "Unable to register SAPI4 voice preview session: application state unavailable",
                );
            }
            crate::sapi4_engine::play_sapi4(
                voice_index,
                text,
                rate,
                pitch,
                volume,
                cancel,
                command_rx,
            );
        }
        TtsEngine::Sapi5 => {
            crate::tts_engine::stop_tts_playback(main_parent);
            let cancel = Arc::new(AtomicBool::new(false));
            let (command_tx, command_rx) = mpsc::unbounded_channel();
            if with_state(main_parent, |state| {
                state.tts_session = Some(crate::tts_engine::TtsSession {
                    id: state.tts_next_session_id,
                    command_tx,
                    cancel: cancel.clone(),
                    paused: false,
                    initial_caret_pos: 0,
                    source_edit: HWND(0),
                });
                state.tts_next_session_id += 1;
            })
            .is_none()
            {
                crate::log_debug(
                    "Unable to register SAPI5 voice preview session: application state unavailable",
                );
            }
            if let Err(error) = crate::sapi5_engine::play_sapi(
                vec![text],
                voice,
                rate,
                pitch,
                volume,
                cancel,
                command_rx,
            ) {
                crate::log_debug(&format!("Audio description voice preview failed: {error}"));
            }
        }
    }
}

fn preview_voice(hwnd: HWND) {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogData };
    if pointer.is_null() {
        return;
    }
    let data = unsafe { &*pointer };
    let engine = selected_engine(hwnd);
    let voice = selected_voice(hwnd);
    if voice.trim().is_empty() {
        return;
    }
    let rate = selected_value(unsafe { GetDlgItem(hwnd, ID_RATE) }, data.default.rate);
    let volume = selected_value(unsafe { GetDlgItem(hwnd, ID_VOLUME) }, data.default.volume);
    preview_voice_settings(
        data.main_parent,
        data.language,
        engine,
        voice,
        rate,
        selected_value(unsafe { GetDlgItem(hwnd, ID_PITCH) }, data.default.pitch),
        volume,
    );
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    crate::panic_guard::guard(
        "audio_description_voice_window_wndproc",
        || unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        || wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let create =
                    &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
                let pointer = create.lpCreateParams as *mut DialogData;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, pointer as isize);
                if pointer.is_null() {
                    return LRESULT(0);
                }
                let data = &*pointer;
                let language = data.language;
                let label_x = 16;
                let combo_x = 190;
                let label_width = 160;
                let combo_width = 320;
                let height = 24;
                let mut y = 18;

                let create_label = |text: String, y: i32| {
                    CreateWindowExW(
                        WINDOW_EX_STYLE(0),
                        WC_STATIC,
                        PCWSTR(to_wide(&text).as_ptr()),
                        WS_CHILD | WS_VISIBLE,
                        label_x,
                        y,
                        label_width,
                        height,
                        hwnd,
                        HMENU(0),
                        HINSTANCE(0),
                        None,
                    )
                };
                let create_combo = |id: i32, y: i32| {
                    CreateWindowExW(
                        WS_EX_CLIENTEDGE,
                        WC_COMBOBOXW,
                        PCWSTR::null(),
                        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                        combo_x,
                        y,
                        combo_width,
                        260,
                        hwnd,
                        HMENU(id as isize),
                        HINSTANCE(0),
                        None,
                    )
                };

                create_label(
                    i18n::tr(language, "audio_description.voice_settings.engine"),
                    y,
                );
                let engine_combo = create_combo(ID_ENGINE, y);
                y += 38;
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    WC_STATIC,
                    PCWSTR(to_wide(&i18n::tr(language, "options.label.voice_language")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    label_x,
                    y,
                    label_width,
                    height,
                    hwnd,
                    HMENU(ID_LANGUAGE_LABEL as isize),
                    HINSTANCE(0),
                    None,
                );
                create_combo(ID_LANGUAGE, y);
                y += 38;
                create_label(
                    i18n::tr(language, "audio_description.voice_settings.voice"),
                    y,
                );
                create_combo(ID_VOICE, y);
                y += 38;
                create_label(i18n::tr(language, "tts_tuning.label_speed"), y);
                create_combo(ID_RATE, y);
                y += 38;
                create_label(i18n::tr(language, "tts_tuning.label_pitch"), y);
                create_combo(ID_PITCH, y);
                y += 38;
                create_label(i18n::tr(language, "tts_tuning.label_volume"), y);
                create_combo(ID_VOLUME, y);
                y += 42;

                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    WC_BUTTON,
                    PCWSTR(
                        to_wide(&i18n::tr(language, "audio_description.voice_settings.test"))
                            .as_ptr(),
                    ),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    combo_x,
                    y,
                    125,
                    28,
                    hwnd,
                    HMENU(ID_TEST as isize),
                    HINSTANCE(0),
                    None,
                );
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "options.ok")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    combo_x + 135,
                    y,
                    85,
                    28,
                    hwnd,
                    HMENU(ID_OK as isize),
                    HINSTANCE(0),
                    None,
                );
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "options.cancel")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    combo_x + 230,
                    y,
                    90,
                    28,
                    hwnd,
                    HMENU(ID_CANCEL as isize),
                    HINSTANCE(0),
                    None,
                );

                fill_engine_combo(hwnd, language, data.default.engine);
                fill_language_combo(hwnd, data);
                fill_voice_combo(hwnd, &data.default.voice);
                fill_value_combo(
                    GetDlgItem(hwnd, ID_RATE),
                    &rate_items(language),
                    data.default.rate,
                );
                fill_pitch_combo(hwnd, language, data.default.engine, data.default.pitch);
                fill_value_combo(
                    GetDlgItem(hwnd, ID_VOLUME),
                    &volume_items(language),
                    data.default.volume,
                );
                SetFocus(engine_combo);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xffff) as i32;
                let notification = ((wparam.0 >> 16) & 0xffff) as u16;
                if id == ID_ENGINE && notification as u32 == CBN_SELCHANGE {
                    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogData;
                    if !pointer.is_null() {
                        let data = &mut *pointer;
                        let engine = selected_engine(hwnd);
                        let current =
                            selected_value(GetDlgItem(hwnd, ID_PITCH), data.default.pitch);
                        let pitch = convert_pitch_engine(current, data.pitch_engine, engine);
                        data.pitch_engine = engine;
                        fill_pitch_combo(hwnd, data.language, engine, pitch);
                        crate::tts_engine::stop_tts_playback(data.main_parent);
                    }
                    fill_voice_combo(hwnd, "");
                    return LRESULT(0);
                }
                if id == ID_LANGUAGE && notification as u32 == CBN_SELCHANGE {
                    let preferred = selected_voice(hwnd);
                    fill_voice_combo(hwnd, &preferred);
                    return LRESULT(0);
                }
                if id == ID_PITCH && notification as u32 == CBN_SELCHANGE {
                    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogData;
                    if !pointer.is_null() {
                        crate::tts_engine::stop_tts_playback((*pointer).main_parent);
                    }
                    return LRESULT(0);
                }
                if id == ID_TEST {
                    preview_voice(hwnd);
                    return LRESULT(0);
                }
                if id == ID_OK {
                    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogData;
                    if !pointer.is_null() {
                        let data = &mut *pointer;
                        let voice = selected_voice(hwnd);
                        if !voice.trim().is_empty() {
                            data.result = Some(AudioDescriptionVoiceSettings {
                                engine: selected_engine(hwnd),
                                voice,
                                rate: selected_value(GetDlgItem(hwnd, ID_RATE), data.default.rate),
                                pitch: selected_value(
                                    GetDlgItem(hwnd, ID_PITCH),
                                    data.default.pitch,
                                ),
                                volume: selected_value(
                                    GetDlgItem(hwnd, ID_VOLUME),
                                    data.default.volume,
                                ),
                            });
                            crate::tts_engine::stop_tts_playback(data.main_parent);
                            crate::log_if_err!(DestroyWindow(hwnd));
                        }
                    }
                    return LRESULT(0);
                }
                if id == ID_CANCEL {
                    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogData;
                    if !pointer.is_null() {
                        crate::tts_engine::stop_tts_playback((*pointer).main_parent);
                    }
                    crate::log_if_err!(DestroyWindow(hwnd));
                    return LRESULT(0);
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                let key = wparam.0 as u32;
                if key == windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE.0 as u32 {
                    SendMessageW(hwnd, WM_COMMAND, WPARAM(ID_CANCEL as usize), LPARAM(0));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogData;
                if !pointer.is_null() {
                    crate::tts_engine::stop_tts_playback((*pointer).main_parent);
                }
                crate::log_if_err!(DestroyWindow(hwnd));
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

pub fn open_dialog(
    parent: HWND,
    main_parent: HWND,
    language: Language,
    sources: AudioDescriptionVoiceSources,
    default: AudioDescriptionVoiceSettings,
) -> Option<AudioDescriptionVoiceSettings> {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide("SonarpadAudioDescriptionVoiceDialog");
        let title = i18n::tr(language, "audio_description.voice_settings.title");

        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let class = WNDCLASSW {
                hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                    windows::Win32::UI::WindowsAndMessaging::LoadCursorW(None, IDC_ARROW)
                        .unwrap_or_default()
                        .0,
                ),
                hInstance: hinstance,
                lpszClassName: PCWSTR(class_name.as_ptr()),
                lpfnWndProc: Some(wndproc),
                ..Default::default()
            };
            RegisterClassW(&class);
        });

        let mut voice_languages: Vec<String> = sources
            .edge
            .iter()
            .map(voice_language_code)
            .filter(|code| !code.is_empty())
            .collect();
        voice_languages.sort();
        voice_languages.dedup();
        let mut data = DialogData {
            language,
            main_parent,
            sources,
            pitch_engine: default.engine,
            default,
            voice_languages,
            result: None,
        };
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide(&title).as_ptr()),
            WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            550,
            361,
            parent,
            HMENU(0),
            hinstance,
            Some(&mut data as *mut _ as *const std::ffi::c_void),
        );
        if hwnd.0 == 0 {
            return None;
        }

        EnableWindow(parent, false);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);

        let mut message = MSG::default();
        while IsWindow(hwnd).as_bool() && GetMessageW(&mut message, HWND(0), 0, 0).into() {
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&message) {
                continue;
            }
            if !IsDialogMessageW(hwnd, &message).as_bool() {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        EnableWindow(parent, true);
        SetForegroundWindow(parent);
        data.result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_description_pitch_combo_preserves_negative_one_item_data() {
        unsafe {
            let combo = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                WC_COMBOBOXW,
                PCWSTR::null(),
                windows::Win32::UI::WindowsAndMessaging::WS_POPUP
                    | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                0,
                0,
                120,
                120,
                HWND(0),
                HMENU(0),
                HINSTANCE(0),
                None,
            );
            assert_ne!(combo.0, 0);
            fill_value_combo(combo, &[("Custom pitch".to_string(), -1)], -1);
            let pitch = selected_value(combo, 1050);
            DestroyWindow(combo).expect("destroy hidden test combo");
            assert_eq!(pitch, -1);
        }
    }

    #[test]
    fn audio_description_pitch_conversion_preserves_neutral_and_extremes() {
        for (legacy, google) in [(-12, 1000), (0, 1050), (12, 1100)] {
            assert_eq!(
                convert_pitch_engine(legacy, TtsEngine::Edge, TtsEngine::Google),
                google
            );
            assert_eq!(
                convert_pitch_engine(google, TtsEngine::Google, TtsEngine::Sapi5),
                legacy
            );
        }
        assert_eq!(
            convert_pitch_engine(1053, TtsEngine::Google, TtsEngine::Google),
            1053
        );
        assert_eq!(
            convert_pitch_engine(-3, TtsEngine::Sapi5, TtsEngine::Edge),
            -3
        );
    }
}
