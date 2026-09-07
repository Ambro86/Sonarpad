use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY,
    OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Controls::{
    PBM_SETPOS, PBM_SETRANGE32, PROGRESS_CLASSW, WC_BUTTON, WC_COMBOBOXW, WC_EDIT, WC_LISTBOXW,
    WC_STATIC,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, SetFocus, VK_ESCAPE, VK_RETURN, VK_SHIFT, VK_SPACE, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, BS_DEFPUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_RESETCONTENT, CB_SETCURSEL,
    CBN_SELCHANGE, CBS_DROPDOWNLIST, CREATESTRUCTW, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyMenu, DestroyWindow, EN_CHANGE, ES_AUTOHSCROLL, ES_AUTOVSCROLL,
    ES_MULTILINE, ES_WANTRETURN, GetCursorPos, GetWindowLongPtrW, HMENU, IDC_ARROW, IDYES,
    IsWindowVisible, LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL, LBN_SELCHANGE,
    LBS_HASSTRINGS, LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, LoadCursorW, MB_ICONERROR,
    MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, MF_STRING, MessageBoxW, PostMessageW,
    SendMessageW, SetForegroundWindow, SetWindowLongPtrW, TPM_NONOTIFY, TPM_RETURNCMD,
    TrackPopupMenu, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_CREATE,
    WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_SETFOCUS, WM_SETFONT, WNDCLASSW, WS_CAPTION, WS_CHILD,
    WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
    WS_VSCROLL,
};
use windows::core::{PCWSTR, PWSTR};

use crate::accessibility::to_wide;
use crate::audio_description::{
    AudioDescriptionCallbacks, AudioDescriptionOutcome, AudioDescriptionProject,
    AudioDescriptionProjectBatchEditError, AudioDescriptionProjectDescription,
    AudioDescriptionProjectEditError, AudioDescriptionProjectEditOutcome,
    AudioDescriptionProjectPreviewAudio, AudioDescriptionProjectVoiceError,
    AudioDescriptionProjectVoiceSettings, apply_audio_description_project_batch_edits,
    change_audio_description_project_voice, delete_audio_description_project_description,
    load_audio_description_project, reexport_audio_description_project,
    synthesize_audio_description_project_preview,
};
use crate::bass_output::BassOutput;
use crate::i18n;
use crate::settings::{Language, TtsEngine, VoiceInfo, default_audio_description_save_folder};
use crate::{show_error, with_state};

const CLASS_NAME: &str = "SonarpadAudioDescriptionProject";
const ID_LIST: usize = 9671;
const ID_EDIT: usize = 9672;
const ID_APPLY: usize = 9673;
const ID_EXPORT: usize = 9674;
const ID_CANCEL: usize = 9675;
const ID_CLOSE: usize = 9676;
const ID_CONTEXT_PLAY: usize = 9677;
const ID_CONTEXT_DELETE: usize = 9678;
const ID_VOICE: usize = 9679;
const ID_ENGINE: usize = 9680;
const ID_CHANGE_VOICE: usize = 9681;
const ID_EXPORT_SRT: usize = 9682;
const ID_EXPORT_VTT: usize = 9683;
const ID_SEARCH: usize = 9684;
const ID_SEARCH_BUTTON: usize = 9685;
const ID_RATE: usize = 9686;
const ID_VOLUME: usize = 9687;
const ID_TEST_VOICE: usize = 9688;

const WM_PROJECT_PROGRESS: u32 = WM_APP + 192;
const WM_PROJECT_STATUS: u32 = WM_APP + 193;
const WM_PROJECT_DONE: u32 = WM_APP + 194;
const WM_PROJECT_APPLY_DONE: u32 = WM_APP + 195;
const WM_PROJECT_PLAY_SELECTED: u32 = WM_APP + 196;
const WM_PROJECT_DRAFT_PREVIEW_DONE: u32 = WM_APP + 197;
const WM_PROJECT_VOICES_LOADED: u32 = WM_APP + 198;
const WM_PROJECT_VOICE_DONE: u32 = WM_APP + 199;

struct DraftPreviewPayload {
    generation: u64,
    index: usize,
    text: String,
    result: Result<AudioDescriptionProjectPreviewAudio, String>,
}

struct VoiceLoadPayload {
    engine: TtsEngine,
    result: Result<Vec<VoiceInfo>, String>,
}

struct VoiceChangePayload {
    requested_voice: String,
    result: Result<AudioDescriptionProject, AudioDescriptionProjectVoiceError>,
}

struct Labels {
    title: String,
    open_title: String,
    descriptions: String,
    text: String,
    engine: String,
    voice: String,
    change_voice: String,
    speed: String,
    volume: String,
    test_voice: String,
    apply: String,
    search: String,
    search_button: String,
    export: String,
    export_srt: String,
    export_vtt: String,
    export_subtitle_title: String,
    export_subtitle_success: String,
    export_subtitle_error: String,
    cancel: String,
    close: String,
    ready: String,
    exporting: String,
    canceling: String,
    complete: String,
    no_selection: String,
    success: String,
    normal: String,
    extended: String,
    empty_text: String,
    play_description: String,
    delete_description: String,
    description_deleted: String,
    delete_confirm: String,
    delete_last_error: String,
    checking_duration: String,
    loading_voices: String,
    voice_checking: String,
    voice_changed_title: String,
    voice_changed: String,
    voice_too_long: String,
    voice_check_error: String,
    edit_saved_title: String,
    edit_saved: String,
    edit_saved_multiple: String,
    apply_before_export: String,
    unsaved_close: String,
}

struct WindowState {
    parent: HWND,
    owner: HWND,
    language: Language,
    project_path: PathBuf,
    project: AudioDescriptionProject,
    list: HWND,
    edit: HWND,
    details: HWND,
    engine_combo: HWND,
    voice_combo: HWND,
    rate_combo: HWND,
    volume_combo: HWND,
    test_voice_button: HWND,
    change_voice_button: HWND,
    voices: Vec<VoiceInfo>,
    progress: HWND,
    status: HWND,
    apply_button: HWND,
    search_edit: HWND,
    search_button: HWND,
    display_order: Vec<usize>,
    export_button: HWND,
    export_srt_button: HWND,
    export_vtt_button: HWND,
    cancel_button: HWND,
    close_button: HWND,
    selected_index: Option<usize>,
    drafts: HashMap<usize, String>,
    running: bool,
    cancel: Option<Arc<AtomicBool>>,
    preview_cancel: Option<Arc<AtomicBool>>,
    preview_generation: Arc<AtomicU64>,
}

fn post_boxed_message<T>(hwnd: HWND, message: u32, wparam: WPARAM, payload: Box<T>) {
    let raw_payload = Box::into_raw(payload);
    let posted = unsafe { PostMessageW(hwnd, message, wparam, LPARAM(raw_payload as isize)) };
    if posted.is_err() {
        unsafe {
            let _released_payload = Box::from_raw(raw_payload);
        }
    }
}

fn labels(language: Language) -> Labels {
    Labels {
        title: i18n::tr(language, "audio_description.project.title"),
        open_title: i18n::tr(language, "audio_description.project.open_title"),
        descriptions: i18n::tr(language, "audio_description.project.descriptions"),
        text: i18n::tr(language, "audio_description.project.text"),
        engine: i18n::tr(language, "audio_description.engine"),
        voice: i18n::tr(language, "audio_description.voice"),
        change_voice: i18n::tr(language, "audio_description.project.change_voice"),
        speed: i18n::tr(language, "tts_tuning.label_speed"),
        volume: i18n::tr(language, "tts_tuning.label_volume"),
        test_voice: i18n::tr(language, "audio_description.voice_settings.test"),
        apply: i18n::tr(language, "audio_description.project.apply"),
        search: i18n::tr(language, "audio_description.project.search"),
        search_button: i18n::tr(language, "audio_description.project.search_button"),
        export: i18n::tr(language, "audio_description.project.export"),
        export_srt: i18n::tr(language, "audio_description.project.export_srt"),
        export_vtt: i18n::tr(language, "audio_description.project.export_vtt"),
        export_subtitle_title: i18n::tr(
            language,
            "audio_description.project.export_subtitle_title",
        ),
        export_subtitle_success: i18n::tr(
            language,
            "audio_description.project.export_subtitle_success",
        ),
        export_subtitle_error: i18n::tr(
            language,
            "audio_description.project.export_subtitle_error",
        ),
        cancel: i18n::tr(language, "audio_description.cancel"),
        close: i18n::tr(language, "audio_description.close"),
        ready: i18n::tr(language, "audio_description.project.status.ready"),
        exporting: i18n::tr(language, "audio_description.project.status.exporting"),
        canceling: i18n::tr(language, "audio_description.status.canceling"),
        complete: i18n::tr(language, "audio_description.project.status.complete"),
        no_selection: i18n::tr(language, "audio_description.project.no_selection"),
        success: i18n::tr(language, "audio_description.project.success"),
        normal: i18n::tr(language, "audio_description.project.normal"),
        extended: i18n::tr(language, "audio_description.project.extended"),
        empty_text: i18n::tr(language, "audio_description.project.error_empty"),
        play_description: i18n::tr(language, "audio_description.project.play_description"),
        delete_description: i18n::tr(language, "audio_description.project.delete_description"),
        description_deleted: i18n::tr(language, "audio_description.project.description_deleted"),
        delete_confirm: i18n::tr(language, "audio_description.project.delete_confirm"),
        delete_last_error: i18n::tr(language, "audio_description.project.delete_last_error"),
        checking_duration: i18n::tr(
            language,
            "audio_description.project.status.checking_duration",
        ),
        loading_voices: i18n::tr(language, "audio_description.status.loading_voices"),
        voice_checking: i18n::tr(language, "audio_description.project.voice_checking"),
        voice_changed_title: i18n::tr(language, "audio_description.project.voice_changed_title"),
        voice_changed: i18n::tr(language, "audio_description.project.voice_changed"),
        voice_too_long: i18n::tr(language, "audio_description.project.voice_too_long"),
        voice_check_error: i18n::tr(language, "audio_description.project.voice_check_error"),
        edit_saved_title: i18n::tr(language, "audio_description.project.edit_saved_title"),
        edit_saved: i18n::tr(language, "audio_description.project.edit_saved"),
        edit_saved_multiple: i18n::tr(language, "audio_description.project.edit_saved_multiple"),
        apply_before_export: i18n::tr(language, "audio_description.project.apply_before_export"),
        unsaved_close: i18n::tr(language, "audio_description.project.unsaved_close"),
    }
}

fn add_combo_item(combo: HWND, text: &str) {
    let wide = to_wide(text);
    unsafe {
        SendMessageW(
            combo,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
}

fn engine_from_combo(combo: HWND) -> TtsEngine {
    match unsafe { SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 } {
        1 => TtsEngine::Sapi5,
        2 => TtsEngine::Sapi4,
        3 => TtsEngine::Google,
        _ => TtsEngine::Edge,
    }
}

fn engine_combo_index(engine: TtsEngine) -> usize {
    match engine {
        TtsEngine::Edge => 0,
        TtsEngine::Sapi5 => 1,
        TtsEngine::Sapi4 => 2,
        TtsEngine::Google => 3,
    }
}

fn fill_project_value_combo(combo: HWND, items: &[(String, i32)], selected: i32) {
    unsafe { SendMessageW(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0)) };
    let mut selected_index = 0usize;
    let mut best_distance = i32::MAX;
    for (index, (label, value)) in items.iter().enumerate() {
        add_combo_item(combo, label);
        let distance = (*value - selected).abs();
        if distance < best_distance {
            selected_index = index;
            best_distance = distance;
        }
    }
    if !items.is_empty() {
        unsafe {
            SendMessageW(combo, CB_SETCURSEL, WPARAM(selected_index), LPARAM(0));
        }
    }
}

fn project_combo_value(combo: HWND, items: &[(String, i32)], fallback: i32) -> i32 {
    let selected = unsafe { SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
    if selected < 0 {
        return fallback;
    }
    items
        .get(selected as usize)
        .map(|(_, value)| *value)
        .unwrap_or(fallback)
}

fn selected_project_voice(state: &WindowState) -> Option<String> {
    let selected = unsafe { SendMessageW(state.voice_combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
    (selected >= 0)
        .then(|| state.voices.get(selected as usize))
        .flatten()
        .map(|voice| voice.short_name.clone())
}

fn selected_project_voice_settings(state: &WindowState) -> Option<(TtsEngine, String, i32, i32)> {
    let voice = selected_project_voice(state)?;
    let rate_items = super::audio_description_voice_window::rate_items(state.language);
    let volume_items = super::audio_description_voice_window::volume_items(state.language);
    Some((
        engine_from_combo(state.engine_combo),
        voice,
        project_combo_value(state.rate_combo, &rate_items, state.project.tts_rate),
        project_combo_value(state.volume_combo, &volume_items, state.project.tts_volume),
    ))
}

fn restore_project_voice_tuning(state: &WindowState) {
    let rate_items = super::audio_description_voice_window::rate_items(state.language);
    let volume_items = super::audio_description_voice_window::volume_items(state.language);
    fill_project_value_combo(state.rate_combo, &rate_items, state.project.tts_rate);
    fill_project_value_combo(state.volume_combo, &volume_items, state.project.tts_volume);
}

fn test_project_voice(state: &WindowState) {
    let Some((engine, voice, rate, volume)) = selected_project_voice_settings(state) else {
        return;
    };
    super::audio_description_voice_window::preview_voice_settings(
        state.parent,
        state.language,
        engine,
        voice,
        rate,
        state.project.tts_pitch,
        volume,
    );
}

fn load_project_voices(hwnd: HWND, engine: TtsEngine) {
    thread::spawn(move || {
        let result = match engine {
            TtsEngine::Edge => crate::app_windows::options_window::fetch_voice_list(),
            TtsEngine::Sapi5 => crate::sapi5_engine::list_sapi_voices(),
            TtsEngine::Sapi4 => Ok(crate::sapi4_engine::get_voices()),
            TtsEngine::Google => Ok(crate::google_tts::installed_voices()),
        };
        post_boxed_message(
            hwnd,
            WM_PROJECT_VOICES_LOADED,
            WPARAM(0),
            Box::new(VoiceLoadPayload { engine, result }),
        );
    });
}

fn refill_project_voice_combo(
    state: &mut WindowState,
    engine: TtsEngine,
    mut voices: Vec<VoiceInfo>,
) {
    let current = state.project.tts_voice.trim();
    if engine == state.project.tts_engine
        && !current.is_empty()
        && !voices
            .iter()
            .any(|voice| voice.short_name.eq_ignore_ascii_case(current))
    {
        voices.insert(
            0,
            VoiceInfo {
                short_name: current.to_string(),
                locale: state.project.language_code.clone(),
                is_multilingual: false,
            },
        );
    }
    state.voices = voices;
    unsafe {
        SendMessageW(state.voice_combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    }
    for voice in &state.voices {
        let display = if voice.locale.trim().is_empty() {
            voice.short_name.clone()
        } else {
            format!("{} ({})", voice.short_name, voice.locale)
        };
        add_combo_item(state.voice_combo, &display);
    }
    let language_code = state.project.language_code.to_ascii_lowercase();
    let selected = if engine == state.project.tts_engine {
        state
            .voices
            .iter()
            .position(|voice| {
                voice
                    .short_name
                    .eq_ignore_ascii_case(&state.project.tts_voice)
            })
            .or_else(|| {
                state.voices.iter().position(|voice| {
                    voice
                        .locale
                        .to_ascii_lowercase()
                        .starts_with(&language_code)
                })
            })
            .unwrap_or(0)
    } else {
        state
            .voices
            .iter()
            .position(|voice| {
                voice
                    .locale
                    .to_ascii_lowercase()
                    .starts_with(&language_code)
            })
            .unwrap_or(0)
    };
    unsafe {
        if state.voices.is_empty() {
            EnableWindow(state.voice_combo, false);
            EnableWindow(state.test_voice_button, false);
            EnableWindow(state.change_voice_button, false);
        } else {
            SendMessageW(state.voice_combo, CB_SETCURSEL, WPARAM(selected), LPARAM(0));
            EnableWindow(state.voice_combo, !state.running);
            EnableWindow(state.test_voice_button, !state.running);
            EnableWindow(state.change_voice_button, !state.running);
        }
    }
}

fn restore_project_voice_selection(hwnd: HWND, state: &mut WindowState) {
    restore_project_voice_tuning(state);
    let selected_engine = engine_from_combo(state.engine_combo);
    unsafe {
        SendMessageW(
            state.engine_combo,
            CB_SETCURSEL,
            WPARAM(engine_combo_index(state.project.tts_engine)),
            LPARAM(0),
        );
    }
    if selected_engine != state.project.tts_engine {
        state.voices.clear();
        unsafe {
            SendMessageW(state.voice_combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
            EnableWindow(state.voice_combo, false);
            EnableWindow(state.test_voice_button, false);
            EnableWindow(state.change_voice_button, false);
        }
        set_text(state.status, &labels(state.language).loading_voices);
        load_project_voices(hwnd, state.project.tts_engine);
        return;
    }
    if let Some(index) = state.voices.iter().position(|voice| {
        voice
            .short_name
            .eq_ignore_ascii_case(&state.project.tts_voice)
    }) {
        unsafe {
            SendMessageW(state.voice_combo, CB_SETCURSEL, WPARAM(index), LPARAM(0));
        }
    }
}

fn focus_descriptions_list(hwnd: HWND) {
    unsafe {
        let pointer =
            GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                as *mut WindowState;
        if !pointer.is_null() && crate::is_window_handle_valid((*pointer).list) {
            SetFocus((*pointer).list);
        }
    }
}

fn show_project_error(hwnd: HWND, language: Language, message: &str) {
    crate::log_debug(&format!("Audio description project error shown: {message}"));
    let text = to_wide(message);
    let title = to_wide(&i18n::tr(language, "app.error_title"));
    crate::watchdog::enter_modal_dialog();
    unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
    crate::watchdog::exit_modal_dialog();
    if crate::is_window_handle_valid(hwnd) && unsafe { IsWindowVisible(hwnd).as_bool() } {
        unsafe {
            SetForegroundWindow(hwnd);
        }
        focus_descriptions_list(hwnd);
    }
}

fn show_project_info_with_title(hwnd: HWND, title: &str, message: &str) {
    crate::log_debug(&format!(
        "Audio description project information shown: {message}"
    ));
    let text = to_wide(message);
    let title = to_wide(title);
    crate::watchdog::enter_modal_dialog();
    unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
    crate::watchdog::exit_modal_dialog();
    if crate::is_window_handle_valid(hwnd) && unsafe { IsWindowVisible(hwnd).as_bool() } {
        unsafe {
            SetForegroundWindow(hwnd);
        }
        focus_descriptions_list(hwnd);
    }
}

fn show_project_info(hwnd: HWND, language: Language, message: &str) {
    crate::log_debug(&format!(
        "Audio description project information shown: {message}"
    ));
    let text = to_wide(message);
    let title = to_wide(&i18n::tr(language, "app.info_title"));
    crate::watchdog::enter_modal_dialog();
    unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
    crate::watchdog::exit_modal_dialog();
    if crate::is_window_handle_valid(hwnd) && unsafe { IsWindowVisible(hwnd).as_bool() } {
        unsafe {
            SetForegroundWindow(hwnd);
        }
        focus_descriptions_list(hwnd);
    }
}

pub fn handle_navigation(hwnd: HWND, msg: &windows::Win32::UI::WindowsAndMessaging::MSG) -> bool {
    if msg.hwnd != hwnd && !crate::is_child_safe(hwnd, msg.hwnd) {
        return false;
    }
    if msg.message != WM_KEYDOWN {
        return false;
    }
    let key = msg.wParam.0 as u32;
    let current = crate::get_focus_safe();
    let (list, search_edit) = unsafe {
        let pointer =
            GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                as *mut WindowState;
        if pointer.is_null() {
            (None, None)
        } else {
            (Some((*pointer).list), Some((*pointer).search_edit))
        }
    };
    if key == VK_RETURN.0 as u32 && search_edit == Some(current) {
        unsafe {
            let pointer =
                GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                    as *mut WindowState;
            if !pointer.is_null() && !(*pointer).running {
                apply_description_search(&mut *pointer);
            }
        }
        return true;
    }
    if (key == VK_SPACE.0 as u32 || key == VK_RETURN.0 as u32) && list == Some(current) {
        crate::log_if_err!(
            crate::post_message_w_safe(hwnd, WM_PROJECT_PLAY_SELECTED, WPARAM(0), LPARAM(0),),
            "Audio description project: PostMessageW failed"
        );
        return true;
    }
    if key == VK_ESCAPE.0 as u32 {
        crate::log_if_err!(
            crate::post_message_w_safe(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)),
            "Audio description project: PostMessageW failed"
        );
        return true;
    }
    if key == VK_TAB.0 as u32 {
        let reverse = (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000_u16 as i16)) != 0;
        let next = if current.0 != 0 && (current == hwnd || crate::is_child_safe(hwnd, current)) {
            crate::get_next_dlg_tab_item_safe(hwnd, current, reverse)
        } else {
            HWND(0)
        };
        if next.0 != 0 {
            crate::set_focus_safe(next);
        } else {
            focus_descriptions_list(hwnd);
        }
        return true;
    }
    false
}

fn get_text(hwnd: HWND) -> String {
    let len = crate::get_window_text_length_w_safe(hwnd) as usize;
    let mut buffer = vec![0_u16; len.saturating_add(1)];
    let read = crate::get_window_text_w_safe(hwnd, &mut buffer) as usize;
    String::from_utf16_lossy(&buffer[..read.min(len)])
}

fn set_text(hwnd: HWND, text: &str) {
    let wide = to_wide(text);
    crate::log_if_err!(
        crate::set_window_text_w_safe(hwnd, PCWSTR(wide.as_ptr())),
        "Audio description project: SetWindowTextW failed"
    );
}

fn format_time(seconds: f64) -> String {
    let total_ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms / 60_000) % 60;
    let secs = (total_ms / 1000) % 60;
    let millis = total_ms % 1000;
    format!("{hours:02}:{minutes:02}:{secs:02}.{millis:03}")
}

fn row_text(
    project: &AudioDescriptionProject,
    index: usize,
    labels: &Labels,
    text_override: Option<&str>,
) -> String {
    let description = &project.descriptions[index];
    let kind = if description.extended_pause {
        &labels.extended
    } else {
        &labels.normal
    };
    let text = text_override.unwrap_or(&description.text);
    format!(
        "{}. {} - {} - {} - {}",
        index + 1,
        format_time(description.output_start_sec),
        format_time(description.output_end_sec),
        kind,
        text.replace(['\r', '\n'], " ")
    )
}

fn project_description_id(state: &WindowState, index: usize) -> Option<usize> {
    state
        .project
        .descriptions
        .get(index)
        .map(|description| description.id)
}

fn draft_text_for_index(state: &WindowState, index: usize) -> String {
    let Some(description) = state.project.descriptions.get(index) else {
        return String::new();
    };
    state
        .drafts
        .get(&description.id)
        .cloned()
        .unwrap_or_else(|| description.text.clone())
}

fn capture_current_draft(state: &mut WindowState) {
    let Some(index) = state.selected_index else {
        return;
    };
    let Some(description) = state.project.descriptions.get(index) else {
        return;
    };
    let description_id = description.id;
    let original = description.text.trim();
    let current = get_text(state.edit).trim().to_string();
    if current == original {
        state.drafts.remove(&description_id);
    } else {
        state.drafts.insert(description_id, current);
    }
}

fn select_project_description(state: &mut WindowState, project_index: usize) {
    if let Some(list_index) = state
        .display_order
        .iter()
        .position(|index| *index == project_index)
    {
        unsafe {
            SendMessageW(state.list, LB_SETCURSEL, WPARAM(list_index), LPARAM(0));
        }
    }
    state.selected_index = Some(project_index);
    set_text(state.edit, &draft_text_for_index(state, project_index));
    set_text(
        state.details,
        &details_text(
            &state.project,
            project_index,
            state.language,
            &labels(state.language),
        ),
    );
}

fn details_text(
    project: &AudioDescriptionProject,
    index: usize,
    language: Language,
    labels: &Labels,
) -> String {
    let description = &project.descriptions[index];
    let path = project.output_mp3_path.to_string_lossy();
    let source = format_time(description.source_start_sec);
    let start = format_time(description.output_start_sec);
    let end = format_time(description.output_end_sec);
    let duration = format!("{:.3}", description.tts_duration_sec);
    let mode = if description.extended_pause {
        labels.extended.as_str()
    } else {
        labels.normal.as_str()
    };
    i18n::tr_f(
        language,
        "audio_description.project.details",
        &[
            ("path", &path),
            ("source", &source),
            ("start", &start),
            ("end", &end),
            ("duration", &duration),
            ("mode", mode),
        ],
    )
}

fn description_search_order(project: &AudioDescriptionProject, query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return (0..project.descriptions.len()).collect();
    }

    let mut matching = Vec::new();
    let mut remaining = Vec::new();
    for (index, description) in project.descriptions.iter().enumerate() {
        if description.text.to_lowercase().contains(&query) {
            matching.push(index);
        } else {
            remaining.push(index);
        }
    }
    matching.extend(remaining);
    matching
}

fn refill_list(state: &mut WindowState, select_project_index: usize) {
    let labels = labels(state.language);
    state.display_order = description_search_order(&state.project, &get_text(state.search_edit));
    unsafe {
        SendMessageW(state.list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
        for project_index in &state.display_order {
            let override_text = project_description_id(state, *project_index)
                .and_then(|id| state.drafts.get(&id))
                .map(String::as_str);
            let row = to_wide(&row_text(
                &state.project,
                *project_index,
                &labels,
                override_text,
            ));
            SendMessageW(
                state.list,
                LB_ADDSTRING,
                WPARAM(0),
                LPARAM(row.as_ptr() as isize),
            );
        }
        if !state.display_order.is_empty() {
            let list_index = state
                .display_order
                .iter()
                .position(|index| *index == select_project_index)
                .unwrap_or(0);
            let project_index = state.display_order[list_index];
            SendMessageW(state.list, LB_SETCURSEL, WPARAM(list_index), LPARAM(0));
            state.selected_index = Some(project_index);
            set_text(state.edit, &draft_text_for_index(state, project_index));
            set_text(
                state.details,
                &details_text(&state.project, project_index, state.language, &labels),
            );
        } else {
            state.selected_index = None;
            set_text(state.edit, "");
            set_text(state.details, "");
        }
    }
}

fn apply_description_search(state: &mut WindowState) {
    capture_current_draft(state);
    let query = get_text(state.search_edit);
    let query = query.trim().to_lowercase();
    let current = state.selected_index.unwrap_or(0);
    let target = if query.is_empty() {
        current
    } else {
        state
            .project
            .descriptions
            .iter()
            .position(|description| description.text.to_lowercase().contains(&query))
            .unwrap_or(current)
    };
    refill_list(state, target);
    unsafe {
        SetFocus(state.list);
    }
}

fn selected_edit_text(state: &WindowState) -> Result<(usize, String), String> {
    let Some(index) = state.selected_index else {
        return Err(labels(state.language).no_selection);
    };
    let text = get_text(state.edit).trim().to_string();
    if text.is_empty() {
        return Err(labels(state.language).empty_text);
    }
    Ok((index, text))
}

fn has_unapplied_edit(state: &WindowState) -> bool {
    if !state.drafts.is_empty() {
        return true;
    }
    let Some(index) = state.selected_index else {
        return false;
    };
    state
        .project
        .descriptions
        .get(index)
        .is_some_and(|description| get_text(state.edit).trim() != description.text.trim())
}

fn stop_preview(state: &mut WindowState) {
    if let Some(cancel) = state.preview_cancel.take() {
        cancel.store(true, Ordering::Relaxed);
    }
    state.preview_generation.fetch_add(1, Ordering::Relaxed);
}

fn preview_error(hwnd: HWND, state: &WindowState, error: &str) {
    let message = i18n::tr_f(
        state.language,
        "audio_description.project.preview_error",
        &[("error", error)],
    );
    show_project_error(hwnd, state.language, &message);
}

fn play_exported_description(
    hwnd: HWND,
    state: &mut WindowState,
    description: &AudioDescriptionProjectDescription,
) {
    if !state.project.output_mp3_path.is_file() {
        preview_error(
            hwnd,
            state,
            state.project.output_mp3_path.to_string_lossy().as_ref(),
        );
        return;
    }

    stop_preview(state);
    let preview = match BassOutput::start(&state.project.output_mp3_path, 0, 1.0, 0.0, 1.0, true) {
        Ok(preview) => preview,
        Err(error) => {
            preview_error(hwnd, state, &error);
            return;
        }
    };
    if !preview.seek_to_seconds(description.output_start_sec) || !preview.play() {
        preview.stop();
        preview_error(hwnd, state, "seek/play");
        return;
    }

    let generation = state
        .preview_generation
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1);
    let stop_at = description
        .output_end_sec
        .max(description.output_start_sec + 0.050);
    let generation_state = state.preview_generation.clone();
    thread::spawn(move || {
        loop {
            if generation_state.load(Ordering::Relaxed) != generation {
                preview.stop();
                return;
            }
            if preview.is_stopped()
                || preview
                    .position_secs()
                    .is_some_and(|position| position + 0.010 >= stop_at)
            {
                preview.stop();
                return;
            }
            thread::sleep(std::time::Duration::from_millis(20));
        }
    });
}

fn start_draft_preview(hwnd: HWND, state: &mut WindowState, index: usize, text: String) {
    stop_preview(state);
    let cancel = Arc::new(AtomicBool::new(false));
    state.preview_cancel = Some(cancel.clone());
    let generation = state
        .preview_generation
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1);
    let project = state.project.clone();
    set_text(state.status, &labels(state.language).checking_duration);
    thread::spawn(move || {
        let result = synthesize_audio_description_project_preview(&project, index, &text, cancel);
        let payload = DraftPreviewPayload {
            generation,
            index,
            text,
            result,
        };
        post_boxed_message(
            hwnd,
            WM_PROJECT_DRAFT_PREVIEW_DONE,
            WPARAM(0),
            Box::new(payload),
        );
    });
}

fn play_modified_preview(
    hwnd: HWND,
    state: &mut WindowState,
    description: AudioDescriptionProjectDescription,
    preview_audio: AudioDescriptionProjectPreviewAudio,
) {
    let voice = match BassOutput::start(preview_audio.path(), 0, 1.0, 0.0, 1.0, true) {
        Ok(voice) => voice,
        Err(error) => {
            preview_error(hwnd, state, &error);
            return;
        }
    };

    let mut source = if description.extended_pause || !state.project.source_path.is_file() {
        None
    } else {
        let source_start = description.source_start_sec.max(0.0);
        let duck_gain = 10_f32.powf(state.project.ducking_db.min(0.0) / 20.0);
        match BassOutput::start_with_ffmpeg_at(
            &state.project.source_path,
            source_start,
            1.0,
            0.0,
            duck_gain,
            true,
            None,
        ) {
            Ok(source) => {
                crate::log_debug(&format!(
                    "Audio description project: original-audio preview opened at {:.3}s",
                    source_start
                ));
                Some(source)
            }
            Err(error) => {
                crate::log_debug(&format!(
                    "Audio description project: original-audio preview unavailable ({error}); playing voice only"
                ));
                None
            }
        }
    };

    if source.as_ref().is_some_and(|source| !source.play())
        && let Some(failed_source) = source.take()
    {
        failed_source.stop();
    }
    if !voice.play() {
        if let Some(source) = source.as_ref() {
            source.stop();
        }
        voice.stop();
        preview_error(hwnd, state, "play");
        return;
    }

    let generation = state.preview_generation.load(Ordering::Relaxed);
    let generation_state = state.preview_generation.clone();
    let duration_sec = preview_audio.duration_sec().max(0.050);
    set_text(state.status, &labels(state.language).ready);
    thread::spawn(move || {
        let _preview_audio = preview_audio;
        loop {
            if generation_state.load(Ordering::Relaxed) != generation
                || voice.is_stopped()
                || voice
                    .position_secs()
                    .is_some_and(|position| position + 0.010 >= duration_sec)
            {
                voice.stop();
                if let Some(source) = source.as_ref() {
                    source.stop();
                }
                return;
            }
            thread::sleep(std::time::Duration::from_millis(20));
        }
    });
}

fn play_selected_description(hwnd: HWND, state: &mut WindowState) {
    let (index, draft) = match selected_edit_text(state) {
        Ok(values) => values,
        Err(error) => {
            show_project_error(hwnd, state.language, &error);
            return;
        }
    };
    let Some(description) = state.project.descriptions.get(index).cloned() else {
        show_project_error(hwnd, state.language, &labels(state.language).no_selection);
        return;
    };
    if draft != description.text || description.rendered_text != description.text {
        start_draft_preview(hwnd, state, index, draft);
    } else {
        play_exported_description(hwnd, state, &description);
    }
}

fn delete_selected_description(hwnd: HWND, state: &mut WindowState) {
    capture_current_draft(state);
    let Some(index) = state.selected_index else {
        show_error(
            state.parent,
            state.language,
            &labels(state.language).no_selection,
        );
        return;
    };
    if state.project.descriptions.len() <= 1 {
        show_error(
            state.parent,
            state.language,
            &labels(state.language).delete_last_error,
        );
        return;
    }
    let deleted_description_id = project_description_id(state, index);
    let language_labels = labels(state.language);
    let text = to_wide(&language_labels.delete_confirm);
    let title = to_wide(&language_labels.title);
    let answer = unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_YESNO | MB_ICONQUESTION,
        )
    };
    if answer != IDYES {
        return;
    }
    stop_preview(state);
    match delete_audio_description_project_description(&state.project_path, &state.project, index) {
        Ok(project) => {
            if let Some(description_id) = deleted_description_id {
                state.drafts.remove(&description_id);
            }
            state.project = project;
            let next = index.min(state.project.descriptions.len().saturating_sub(1));
            refill_list(state, next);
            set_text(state.status, &language_labels.description_deleted);
            crate::accessibility::screen_reader_speak(&language_labels.description_deleted);
            focus_descriptions_list(hwnd);
        }
        Err(error) => show_error(state.parent, state.language, &error),
    }
}

fn show_description_context_menu(hwnd: HWND, state: &mut WindowState, lparam: LPARAM) {
    if state.running || state.selected_index.is_none() {
        return;
    }
    let menu = match unsafe { CreatePopupMenu() } {
        Ok(menu) => menu,
        Err(error) => {
            crate::log_debug(&format!(
                "Audio description project: failed to create context menu: {error}"
            ));
            return;
        }
    };
    let language_labels = labels(state.language);
    let play = to_wide(&language_labels.play_description);
    let delete = to_wide(&language_labels.delete_description);
    if unsafe { AppendMenuW(menu, MF_STRING, ID_CONTEXT_PLAY, PCWSTR(play.as_ptr())) }.is_err()
        || unsafe { AppendMenuW(menu, MF_STRING, ID_CONTEXT_DELETE, PCWSTR(delete.as_ptr())) }
            .is_err()
    {
        crate::log_if_err!(unsafe { DestroyMenu(menu) });
        return;
    }
    let point = if lparam.0 == -1 {
        let mut point = POINT::default();
        if unsafe { GetCursorPos(&mut point) }.is_err() {
            crate::log_if_err!(unsafe { DestroyMenu(menu) });
            return;
        }
        point
    } else {
        POINT {
            x: (lparam.0 as u32 & 0xffff) as i16 as i32,
            y: ((lparam.0 as u32 >> 16) & 0xffff) as i16 as i32,
        }
    };
    let command = unsafe {
        TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY,
            point.x,
            point.y,
            0,
            hwnd,
            None,
        )
    };
    crate::log_if_err!(unsafe { DestroyMenu(menu) });
    match command.0 as usize {
        ID_CONTEXT_PLAY => play_selected_description(hwnd, state),
        ID_CONTEXT_DELETE => delete_selected_description(hwnd, state),
        _ => {}
    }
}

fn set_controls_enabled(state: &WindowState, enabled: bool) {
    unsafe {
        for control in [
            state.list,
            state.edit,
            state.apply_button,
            state.search_edit,
            state.search_button,
            state.engine_combo,
            state.rate_combo,
            state.volume_combo,
            state.test_voice_button,
            state.export_button,
            state.export_srt_button,
            state.export_vtt_button,
            state.close_button,
        ] {
            EnableWindow(control, enabled);
        }
        let voices_available = !state.voices.is_empty();
        EnableWindow(state.voice_combo, enabled && voices_available);
        EnableWindow(state.test_voice_button, enabled && voices_available);
        EnableWindow(state.change_voice_button, enabled && voices_available);
        EnableWindow(state.cancel_button, !enabled);
    }
}

fn open_project_dialog(parent: HWND, owner: HWND, language: Language) -> Option<PathBuf> {
    unsafe {
        let labels = labels(language);
        let filter = to_wide(
            "Progetti audiodescrizione Sonarpad (*.sonarpad-ad.json)\0*.sonarpad-ad.json\0File JSON (*.json)\0*.json\0Tutti i file (*.*)\0*.*\0\0",
        );
        let title = to_wide(&labels.open_title);
        let initial_folder = with_state(parent, |state| {
            state.settings.audio_description_save_folder.clone()
        })
        .filter(|folder| !folder.trim().is_empty())
        .unwrap_or_else(default_audio_description_save_folder);
        let initial_folder_wide = to_wide(&initial_folder);
        let mut buffer = [0_u16; 2048];
        let mut dialog = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: owner,
            lpstrFile: PWSTR(buffer.as_mut_ptr()),
            nMaxFile: buffer.len() as u32,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrTitle: PCWSTR(title.as_ptr()),
            lpstrInitialDir: PCWSTR(initial_folder_wide.as_ptr()),
            Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY,
            ..Default::default()
        };
        if !GetOpenFileNameW(&mut dialog).as_bool() {
            return None;
        }
        let len = buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(buffer.len());
        Some(PathBuf::from(String::from_utf16_lossy(&buffer[..len])))
    }
}

pub fn open_from_dialog(parent: HWND, owner: HWND) {
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    if let Some(path) = open_project_dialog(parent, owner, language) {
        open(parent, owner, &path);
    }
}

pub fn open(parent: HWND, owner: HWND, project_path: &Path) {
    let existing =
        with_state(parent, |state| state.audio_description_project_window).unwrap_or(HWND(0));
    if existing.0 != 0 && crate::is_window_handle_valid(existing) {
        unsafe { SetForegroundWindow(existing) };
        focus_descriptions_list(existing);
        return;
    }
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    let project = match load_audio_description_project(project_path) {
        Ok(project) => project,
        Err(error) => {
            show_error(parent, language, &error);
            return;
        }
    };
    let class_name = to_wide(CLASS_NAME);
    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(unsafe {
            LoadCursorW(None, IDC_ARROW).unwrap_or_default().0
        }),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(window_proc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    crate::register_class_w_safe(&wc);
    let title = to_wide(&labels(language).title);
    let payload = Box::new((parent, owner, project_path.to_path_buf(), project));
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            140,
            80,
            820,
            830,
            owner,
            HMENU(0),
            hinstance,
            Some(Box::into_raw(payload) as *const std::ffi::c_void),
        )
    };
    if hwnd.0 != 0
        && with_state(parent, |state| {
            state.audio_description_project_window = hwnd
        })
        .is_none()
    {
        crate::log_debug(
            "Audio description project: parent state unavailable while registering window",
        );
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "audio_description_project_window_proc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || window_proc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn start_apply(hwnd: HWND, state: &mut WindowState) {
    capture_current_draft(state);

    let mut edits = Vec::new();
    let mut empty_index = None;
    for (index, description) in state.project.descriptions.iter().enumerate() {
        if let Some(text) = state.drafts.get(&description.id) {
            if text.trim().is_empty() {
                empty_index = Some(index);
                break;
            }
            edits.push((index, text.clone()));
        }
    }
    if let Some(index) = empty_index {
        refill_list(state, index);
        show_project_error(hwnd, state.language, &labels(state.language).empty_text);
        unsafe {
            SetFocus(state.edit);
        }
        return;
    }
    if edits.is_empty() {
        set_text(state.status, &labels(state.language).edit_saved);
        crate::accessibility::screen_reader_speak(&labels(state.language).edit_saved);
        return;
    }

    stop_preview(state);
    let project_path = state.project_path.clone();
    let project = state.project.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    state.cancel = Some(cancel.clone());
    state.running = true;
    set_controls_enabled(state, false);
    set_text(state.status, &labels(state.language).checking_duration);
    unsafe { SendMessageW(state.progress, PBM_SETPOS, WPARAM(0), LPARAM(0)) };

    thread::spawn(move || {
        let result =
            apply_audio_description_project_batch_edits(&project_path, &project, &edits, cancel);
        post_boxed_message(hwnd, WM_PROJECT_APPLY_DONE, WPARAM(0), Box::new(result));
    });
}

fn start_voice_change(hwnd: HWND, state: &mut WindowState) {
    if has_unapplied_edit(state) {
        show_project_error(
            hwnd,
            state.language,
            &labels(state.language).apply_before_export,
        );
        restore_project_voice_selection(hwnd, state);
        return;
    }
    let Some((candidate_engine, candidate, candidate_rate, candidate_volume)) =
        selected_project_voice_settings(state)
    else {
        restore_project_voice_selection(hwnd, state);
        return;
    };
    if candidate_engine == state.project.tts_engine
        && candidate.eq_ignore_ascii_case(&state.project.tts_voice)
        && candidate_rate == state.project.tts_rate
        && candidate_volume == state.project.tts_volume
    {
        return;
    }

    stop_preview(state);
    let project_path = state.project_path.clone();
    let project = state.project.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    state.cancel = Some(cancel.clone());
    state.running = true;
    set_controls_enabled(state, false);
    set_text(state.status, &labels(state.language).voice_checking);
    unsafe { SendMessageW(state.progress, PBM_SETPOS, WPARAM(0), LPARAM(0)) };

    thread::spawn(move || {
        let progress_hwnd = hwnd;
        let requested_voice = candidate.clone();
        let voice_settings = AudioDescriptionProjectVoiceSettings {
            engine: candidate_engine,
            voice: candidate.clone(),
            rate: candidate_rate,
            volume: candidate_volume,
        };
        let result = change_audio_description_project_voice(
            &project_path,
            &project,
            &voice_settings,
            cancel,
            AudioDescriptionCallbacks {
                status: None,
                progress: Some(Box::new(move |pct| unsafe {
                    crate::log_if_err!(
                        PostMessageW(
                            progress_hwnd,
                            WM_PROJECT_PROGRESS,
                            WPARAM(pct as usize),
                            LPARAM(0),
                        ),
                        "Audio description project: PostMessageW failed"
                    );
                })),
                quota: None,
                overload: None,
            },
        );
        post_boxed_message(
            hwnd,
            WM_PROJECT_VOICE_DONE,
            WPARAM(0),
            Box::new(VoiceChangePayload {
                requested_voice,
                result,
            }),
        );
    });
}

fn format_project_subtitle_timestamp(seconds: f64, millisecond_separator: char) -> String {
    let safe_seconds = if seconds.is_finite() {
        seconds.max(0.0)
    } else {
        0.0
    };
    let total_ms = (safe_seconds * 1000.0).round() as u64;
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms / 60_000) % 60;
    let secs = (total_ms / 1000) % 60;
    let millis = total_ms % 1000;
    format!("{hours:02}:{minutes:02}:{secs:02}{millisecond_separator}{millis:03}")
}

fn normalized_project_subtitle_text(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .to_string()
}

fn render_project_srt(project: &AudioDescriptionProject) -> String {
    let mut output = String::new();
    let mut cue_number = 1usize;
    for description in &project.descriptions {
        let text = normalized_project_subtitle_text(&description.text);
        if text.is_empty() {
            continue;
        }
        let start = description.output_start_sec.max(0.0);
        let end = description.output_end_sec.max(start + 0.001);
        output.push_str(&format!(
            "{cue_number}\r\n{} --> {}\r\n{}\r\n\r\n",
            format_project_subtitle_timestamp(start, ','),
            format_project_subtitle_timestamp(end, ','),
            text.replace('\n', "\r\n")
        ));
        cue_number += 1;
    }
    output
}

fn render_project_vtt(project: &AudioDescriptionProject) -> String {
    let mut output = String::from("WEBVTT\r\n\r\n");
    for description in &project.descriptions {
        let text = normalized_project_subtitle_text(&description.text);
        if text.is_empty() {
            continue;
        }
        let start = description.output_start_sec.max(0.0);
        let end = description.output_end_sec.max(start + 0.001);
        output.push_str(&format!(
            "{} --> {}\r\n{}\r\n\r\n",
            format_project_subtitle_timestamp(start, '.'),
            format_project_subtitle_timestamp(end, '.'),
            text.replace('\n', "\r\n")
        ));
    }
    output
}

fn choose_project_subtitle_output(
    owner: HWND,
    language: Language,
    project: &AudioDescriptionProject,
    extension: &str,
) -> Option<PathBuf> {
    unsafe {
        let labels = labels(language);
        let format_name = extension.to_ascii_uppercase();
        let title = to_wide(
            &labels
                .export_subtitle_title
                .replace("{format}", &format_name),
        );
        let all_files = i18n::tr(language, "dialog.all_files");
        let format_label = if extension.eq_ignore_ascii_case("vtt") {
            "WebVTT"
        } else {
            "SubRip"
        };
        let filter = to_wide(&format!(
            "{format_label} (*.{extension})\0*.{extension}\0{all_files} (*.*)\0*.*\0\0"
        ));
        let extension_wide = to_wide(extension);
        let stem = project
            .output_mp3_path
            .file_stem()
            .and_then(|value| value.to_str())
            .or_else(|| {
                project
                    .source_path
                    .file_stem()
                    .and_then(|value| value.to_str())
            })
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("audiodescrizione");
        let mut initial = project.output_mp3_path.clone();
        if let Some(parent) = initial.parent().map(Path::to_path_buf) {
            initial = parent.join(format!("{stem}.{extension}"));
        } else {
            initial = PathBuf::from(format!("{stem}.{extension}"));
        }
        let initial_wide = to_wide(&initial.to_string_lossy());
        let mut buffer = [0_u16; 2048];
        let copy_len = initial_wide.len().min(buffer.len() - 1);
        buffer[..copy_len].copy_from_slice(&initial_wide[..copy_len]);
        let mut dialog = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: owner,
            lpstrFile: PWSTR(buffer.as_mut_ptr()),
            nMaxFile: buffer.len() as u32,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrTitle: PCWSTR(title.as_ptr()),
            lpstrDefExt: PCWSTR(extension_wide.as_ptr()),
            Flags: OFN_EXPLORER | OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST,
            ..Default::default()
        };
        if !GetSaveFileNameW(&mut dialog).as_bool() {
            return None;
        }
        let len = buffer.iter().position(|value| *value == 0)?;
        let mut path = PathBuf::from(String::from_utf16_lossy(&buffer[..len]));
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case(extension))
        {
            path.set_extension(extension);
        }
        Some(path)
    }
}

fn export_project_subtitles(hwnd: HWND, state: &WindowState, extension: &str) {
    if has_unapplied_edit(state) {
        show_project_error(
            hwnd,
            state.language,
            &labels(state.language).apply_before_export,
        );
        return;
    }
    let Some(path) =
        choose_project_subtitle_output(hwnd, state.language, &state.project, extension)
    else {
        return;
    };
    let contents = if extension.eq_ignore_ascii_case("vtt") {
        render_project_vtt(&state.project)
    } else {
        render_project_srt(&state.project)
    };
    let format_name = extension.to_ascii_uppercase();
    match std::fs::write(&path, contents.as_bytes()) {
        Ok(()) => {
            let message = labels(state.language)
                .export_subtitle_success
                .replace("{format}", &format_name)
                .replace("{path}", &path.to_string_lossy());
            show_project_info(hwnd, state.language, &message);
        }
        Err(error) => {
            let message = labels(state.language)
                .export_subtitle_error
                .replace("{format}", &format_name)
                .replace("{error}", &error.to_string());
            show_project_error(hwnd, state.language, &message);
        }
    }
}

fn start_export(hwnd: HWND, state: &mut WindowState) {
    if has_unapplied_edit(state) {
        show_error(
            state.parent,
            state.language,
            &labels(state.language).apply_before_export,
        );
        return;
    }
    stop_preview(state);
    let project_path = state.project_path.clone();
    let project = state.project.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    state.cancel = Some(cancel.clone());
    state.running = true;
    set_controls_enabled(state, false);
    set_text(state.status, &labels(state.language).exporting);
    unsafe { SendMessageW(state.progress, PBM_SETPOS, WPARAM(0), LPARAM(0)) };

    thread::spawn(move || {
        let status_hwnd = hwnd;
        let progress_hwnd = hwnd;
        let result = reexport_audio_description_project(
            &project_path,
            &project,
            cancel,
            AudioDescriptionCallbacks {
                status: Some(Box::new(move |stage, message| {
                    let payload = Box::new((stage.to_string(), message.to_string()));
                    post_boxed_message(status_hwnd, WM_PROJECT_STATUS, WPARAM(0), payload);
                })),
                progress: Some(Box::new(move |pct| unsafe {
                    crate::log_if_err!(
                        PostMessageW(
                            progress_hwnd,
                            WM_PROJECT_PROGRESS,
                            WPARAM(pct as usize),
                            LPARAM(0),
                        ),
                        "Audio description project: PostMessageW failed"
                    );
                })),
                quota: None,
                overload: None,
            },
        );
        let payload = Box::new(result);
        post_boxed_message(hwnd, WM_PROJECT_DONE, WPARAM(0), payload);
    });
}

fn window_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let create = &*(lparam.0 as *const CREATESTRUCTW);
                let owner = create.hwndParent;
                let payload =
                    create.lpCreateParams as *mut (HWND, HWND, PathBuf, AudioDescriptionProject);
                if payload.is_null() {
                    return LRESULT(-1);
                }
                let (parent, payload_owner, project_path, project) = *Box::from_raw(payload);
                let owner = if payload_owner.0 != 0 {
                    payload_owner
                } else {
                    owner
                };
                let language =
                    with_state(parent, |state| state.settings.language).unwrap_or_default();
                let hfont = with_state(parent, |state| state.hfont).unwrap_or(HFONT(0));
                let labels = labels(language);
                if with_state(parent, |state| {
                    state.audio_description_project_window = hwnd;
                })
                .is_none()
                {
                    crate::log_debug(
                        "Audio description project: parent state unavailable during WM_CREATE",
                    );
                }

                let descriptions_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.descriptions).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    16,
                    370,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let list = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_LISTBOXW,
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WS_VSCROLL
                        | WINDOW_STYLE((LBS_NOTIFY | LBS_HASSTRINGS | LBS_NOINTEGRALHEIGHT) as u32),
                    16,
                    40,
                    760,
                    245,
                    hwnd,
                    HMENU(ID_LIST as isize),
                    HINSTANCE(0),
                    None,
                );
                let text_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.text).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    296,
                    370,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WS_VSCROLL
                        | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32),
                    16,
                    320,
                    760,
                    110,
                    hwnd,
                    HMENU(ID_EDIT as isize),
                    HINSTANCE(0),
                    None,
                );
                let details = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    438,
                    760,
                    38,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let apply_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.apply).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    16,
                    482,
                    120,
                    30,
                    hwnd,
                    HMENU(ID_APPLY as isize),
                    HINSTANCE(0),
                    None,
                );
                let search_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.search).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    150,
                    486,
                    150,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let search_edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    300,
                    482,
                    346,
                    26,
                    hwnd,
                    HMENU(ID_SEARCH as isize),
                    HINSTANCE(0),
                    None,
                );
                let search_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.search_button).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    656,
                    482,
                    120,
                    28,
                    hwnd,
                    HMENU(ID_SEARCH_BUTTON as isize),
                    HINSTANCE(0),
                    None,
                );
                let engine_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.engine).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    526,
                    224,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let engine_combo = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    250,
                    522,
                    526,
                    180,
                    hwnd,
                    HMENU(ID_ENGINE as isize),
                    HINSTANCE(0),
                    None,
                );
                for key in [
                    "options.engine.edge",
                    "options.engine.sapi5",
                    "options.engine.sapi4",
                    "options.engine.google",
                ] {
                    add_combo_item(engine_combo, &i18n::tr(language, key));
                }
                SendMessageW(
                    engine_combo,
                    CB_SETCURSEL,
                    WPARAM(engine_combo_index(project.tts_engine)),
                    LPARAM(0),
                );
                let voice_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.voice).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    560,
                    96,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let voice_combo = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    120,
                    556,
                    462,
                    220,
                    hwnd,
                    HMENU(ID_VOICE as isize),
                    HINSTANCE(0),
                    None,
                );
                EnableWindow(voice_combo, false);
                let speed_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.speed).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    594,
                    96,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let rate_combo = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    120,
                    590,
                    250,
                    220,
                    hwnd,
                    HMENU(ID_RATE as isize),
                    HINSTANCE(0),
                    None,
                );
                let volume_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.volume).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    388,
                    594,
                    96,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let volume_combo = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    492,
                    590,
                    284,
                    220,
                    hwnd,
                    HMENU(ID_VOLUME as isize),
                    HINSTANCE(0),
                    None,
                );
                let rate_items = super::audio_description_voice_window::rate_items(language);
                let volume_items = super::audio_description_voice_window::volume_items(language);
                fill_project_value_combo(rate_combo, &rate_items, project.tts_rate);
                fill_project_value_combo(volume_combo, &volume_items, project.tts_volume);
                let test_voice_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.test_voice).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    388,
                    624,
                    184,
                    30,
                    hwnd,
                    HMENU(ID_TEST_VOICE as isize),
                    HINSTANCE(0),
                    None,
                );
                let change_voice_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.change_voice).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    592,
                    624,
                    184,
                    30,
                    hwnd,
                    HMENU(ID_CHANGE_VOICE as isize),
                    HINSTANCE(0),
                    None,
                );
                EnableWindow(change_voice_button, false);
                EnableWindow(test_voice_button, false);
                let progress = CreateWindowExW(
                    Default::default(),
                    PROGRESS_CLASSW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    664,
                    760,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(progress, PBM_SETRANGE32, WPARAM(0), LPARAM(100));
                let status = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.ready).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    690,
                    760,
                    28,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let export_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.export).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    126,
                    726,
                    180,
                    30,
                    hwnd,
                    HMENU(ID_EXPORT as isize),
                    HINSTANCE(0),
                    None,
                );
                let export_srt_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.export_srt).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    316,
                    726,
                    110,
                    30,
                    hwnd,
                    HMENU(ID_EXPORT_SRT as isize),
                    HINSTANCE(0),
                    None,
                );
                let export_vtt_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.export_vtt).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    436,
                    726,
                    110,
                    30,
                    hwnd,
                    HMENU(ID_EXPORT_VTT as isize),
                    HINSTANCE(0),
                    None,
                );
                let cancel_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.cancel).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    556,
                    726,
                    90,
                    30,
                    hwnd,
                    HMENU(ID_CANCEL as isize),
                    HINSTANCE(0),
                    None,
                );
                EnableWindow(cancel_button, false);
                let close_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.close).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    656,
                    726,
                    100,
                    30,
                    hwnd,
                    HMENU(ID_CLOSE as isize),
                    HINSTANCE(0),
                    None,
                );
                for control in [
                    descriptions_label,
                    list,
                    text_label,
                    edit,
                    details,
                    apply_button,
                    search_label,
                    search_edit,
                    search_button,
                    engine_label,
                    engine_combo,
                    voice_label,
                    voice_combo,
                    speed_label,
                    rate_combo,
                    volume_label,
                    volume_combo,
                    test_voice_button,
                    change_voice_button,
                    status,
                    export_button,
                    export_srt_button,
                    export_vtt_button,
                    cancel_button,
                    close_button,
                ] {
                    if hfont.0 != 0 {
                        SendMessageW(control, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                }
                let mut state = Box::new(WindowState {
                    parent,
                    owner,
                    language,
                    project_path,
                    project,
                    list,
                    edit,
                    details,
                    engine_combo,
                    voice_combo,
                    rate_combo,
                    volume_combo,
                    test_voice_button,
                    change_voice_button,
                    voices: Vec::new(),
                    progress,
                    status,
                    apply_button,
                    search_edit,
                    search_button,
                    display_order: Vec::new(),
                    export_button,
                    export_srt_button,
                    export_vtt_button,
                    cancel_button,
                    close_button,
                    selected_index: None,
                    drafts: HashMap::new(),
                    running: false,
                    cancel: None,
                    preview_cancel: None,
                    preview_generation: Arc::new(AtomicU64::new(0)),
                });
                refill_list(&mut state, 0);
                let project_engine = state.project.tts_engine;
                SetWindowLongPtrW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                    Box::into_raw(state) as isize,
                );
                set_text(status, &labels.loading_voices);
                load_project_voices(hwnd, project_engine);
                SetFocus(list);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let notification = (wparam.0 >> 16) as u32;
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if pointer.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *pointer;
                match id {
                    ID_LIST if notification == LBN_SELCHANGE => {
                        stop_preview(state);
                        capture_current_draft(state);
                        let selected =
                            SendMessageW(state.list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
                        if selected >= 0 {
                            let list_index = selected as usize;
                            let index = state
                                .display_order
                                .get(list_index)
                                .copied()
                                .unwrap_or(list_index);
                            select_project_description(state, index);
                        }
                    }
                    ID_SEARCH if notification == EN_CHANGE && !state.running => {
                        if get_text(state.search_edit).trim().is_empty() {
                            capture_current_draft(state);
                            let selected = state.selected_index.unwrap_or(0);
                            refill_list(state, selected);
                        }
                    }
                    ID_SEARCH_BUTTON if !state.running => apply_description_search(state),
                    ID_ENGINE if notification == CBN_SELCHANGE && !state.running => {
                        state.voices.clear();
                        SendMessageW(state.voice_combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
                        EnableWindow(state.voice_combo, false);
                        EnableWindow(state.test_voice_button, false);
                        EnableWindow(state.change_voice_button, false);
                        set_text(state.status, &labels(state.language).loading_voices);
                        load_project_voices(hwnd, engine_from_combo(state.engine_combo));
                    }
                    ID_TEST_VOICE if !state.running => test_project_voice(state),
                    ID_CHANGE_VOICE if !state.running => start_voice_change(hwnd, state),
                    ID_APPLY if !state.running => start_apply(hwnd, state),
                    ID_EXPORT if !state.running => start_export(hwnd, state),
                    ID_EXPORT_SRT if !state.running => export_project_subtitles(hwnd, state, "srt"),
                    ID_EXPORT_VTT if !state.running => export_project_subtitles(hwnd, state, "vtt"),
                    ID_CANCEL => {
                        if let Some(cancel) = state.cancel.as_ref() {
                            cancel.store(true, Ordering::Relaxed);
                            set_text(state.status, &labels(state.language).canceling);
                            EnableWindow(state.cancel_button, false);
                        }
                    }
                    ID_CLOSE if !state.running => {
                        crate::log_if_err!(
                            PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)),
                            "Audio description project: PostMessageW failed"
                        );
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_CONTEXTMENU => {
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() {
                    let state = &mut *pointer;
                    let source = HWND(wparam.0 as isize);
                    if source == state.list || crate::get_focus_safe() == state.list {
                        show_description_context_menu(hwnd, state, lparam);
                        return LRESULT(0);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_PROJECT_PLAY_SELECTED => {
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() && !(*pointer).running {
                    play_selected_description(hwnd, &mut *pointer);
                }
                LRESULT(0)
            }
            WM_PROJECT_DRAFT_PREVIEW_DONE => {
                let payload = lparam.0 as *mut DraftPreviewPayload;
                if payload.is_null() {
                    return LRESULT(0);
                }
                let payload = *Box::from_raw(payload);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if pointer.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *pointer;
                if state.preview_generation.load(Ordering::Relaxed) != payload.generation
                    || state.selected_index != Some(payload.index)
                    || get_text(state.edit).trim() != payload.text
                {
                    if !state.running {
                        set_text(state.status, &labels(state.language).ready);
                    }
                    return LRESULT(0);
                }
                state.preview_cancel = None;
                match payload.result {
                    Ok(preview_audio) => {
                        let Some(description) =
                            state.project.descriptions.get(payload.index).cloned()
                        else {
                            return LRESULT(0);
                        };
                        play_modified_preview(hwnd, state, description, preview_audio);
                    }
                    Err(error) if error == "cancelled" => {}
                    Err(error) => {
                        set_text(state.status, &labels(state.language).ready);
                        preview_error(hwnd, state, &error);
                    }
                }
                LRESULT(0)
            }
            WM_PROJECT_APPLY_DONE => {
                let payload = lparam.0
                    as *mut Result<
                        AudioDescriptionProjectEditOutcome,
                        AudioDescriptionProjectBatchEditError,
                    >;
                if payload.is_null() {
                    return LRESULT(0);
                }
                let result = *Box::from_raw(payload);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if pointer.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *pointer;
                state.running = false;
                state.cancel = None;
                set_controls_enabled(state, true);
                match result {
                    Ok(outcome) => {
                        let applied_count = outcome.applied_count;
                        state.project = outcome.project;
                        state.drafts.clear();
                        refill_list(state, state.selected_index.unwrap_or(0));
                        SendMessageW(state.progress, PBM_SETPOS, WPARAM(100), LPARAM(0));
                        let language_labels = labels(state.language);
                        let count = applied_count.to_string();
                        let message = language_labels
                            .edit_saved_multiple
                            .replace("{count}", &count);
                        set_text(state.status, &message);
                        show_project_info_with_title(
                            hwnd,
                            &language_labels.edit_saved_title,
                            &message,
                        );
                    }
                    Err(batch_error) => {
                        if let Some(index) = batch_error.index {
                            refill_list(state, index);
                        }
                        match batch_error.error {
                            AudioDescriptionProjectEditError::Cancelled => {
                                set_text(state.status, &labels(state.language).ready);
                            }
                            AudioDescriptionProjectEditError::TooLong {
                                available_sec,
                                synthesized_sec,
                            } => {
                                let available = format!("{available_sec:.3}");
                                let actual = format!("{synthesized_sec:.3}");
                                let message = i18n::tr_f(
                                    state.language,
                                    "audio_description.project.error_too_long",
                                    &[("available", &available), ("actual", &actual)],
                                );
                                set_text(state.status, &message);
                                show_project_error(hwnd, state.language, &message);
                                if batch_error.index.is_some() {
                                    SetFocus(state.edit);
                                }
                            }
                            AudioDescriptionProjectEditError::Other(error) => {
                                set_text(state.status, &error);
                                show_project_error(hwnd, state.language, &error);
                                if batch_error.index.is_some() {
                                    SetFocus(state.edit);
                                }
                            }
                        }
                    }
                }
                LRESULT(0)
            }
            WM_PROJECT_VOICES_LOADED => {
                let payload = lparam.0 as *mut VoiceLoadPayload;
                if payload.is_null() {
                    return LRESULT(0);
                }
                let payload = *Box::from_raw(payload);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if pointer.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *pointer;
                if payload.engine != engine_from_combo(state.engine_combo) {
                    return LRESULT(0);
                }
                match payload.result {
                    Ok(voices) => {
                        refill_project_voice_combo(state, payload.engine, voices);
                        if !state.running {
                            set_text(state.status, &labels(state.language).ready);
                        }
                    }
                    Err(error) => {
                        set_text(state.status, &error);
                        EnableWindow(state.voice_combo, false);
                        EnableWindow(state.test_voice_button, false);
                        EnableWindow(state.change_voice_button, false);
                    }
                }
                LRESULT(0)
            }
            WM_PROJECT_VOICE_DONE => {
                let payload = lparam.0 as *mut VoiceChangePayload;
                if payload.is_null() {
                    return LRESULT(0);
                }
                let payload = *Box::from_raw(payload);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if pointer.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *pointer;
                state.running = false;
                state.cancel = None;
                set_controls_enabled(state, true);
                match payload.result {
                    Ok(project) => {
                        state.project = project;
                        restore_project_voice_selection(hwnd, state);
                        SendMessageW(state.progress, PBM_SETPOS, WPARAM(100), LPARAM(0));
                        let message = labels(state.language)
                            .voice_changed
                            .replace("{voice}", &payload.requested_voice);
                        set_text(state.status, &message);
                        show_project_info_with_title(
                            hwnd,
                            &labels(state.language).voice_changed_title,
                            &message,
                        );
                    }
                    Err(AudioDescriptionProjectVoiceError::Cancelled) => {
                        restore_project_voice_selection(hwnd, state);
                        set_text(state.status, &labels(state.language).ready);
                    }
                    Err(AudioDescriptionProjectVoiceError::DoesNotFit {
                        source_start_sec,
                        synthesized_sec,
                    }) => {
                        restore_project_voice_selection(hwnd, state);
                        let message = labels(state.language)
                            .voice_too_long
                            .replace("{time}", &format_time(source_start_sec))
                            .replace("{actual}", &format!("{synthesized_sec:.3}"));
                        set_text(state.status, &message);
                        show_project_error(hwnd, state.language, &message);
                    }
                    Err(AudioDescriptionProjectVoiceError::Other(error)) => {
                        restore_project_voice_selection(hwnd, state);
                        let message = labels(state.language)
                            .voice_check_error
                            .replace("{error}", &error);
                        set_text(state.status, &message);
                        show_project_error(hwnd, state.language, &message);
                    }
                }
                LRESULT(0)
            }
            WM_PROJECT_PROGRESS => {
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() {
                    SendMessageW(
                        (*pointer).progress,
                        PBM_SETPOS,
                        WPARAM(wparam.0.min(100)),
                        LPARAM(0),
                    );
                }
                LRESULT(0)
            }
            WM_PROJECT_STATUS => {
                let payload = lparam.0 as *mut (String, String);
                if payload.is_null() {
                    return LRESULT(0);
                }
                let (stage, message) = *Box::from_raw(payload);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() {
                    let localized = super::audio_description_window::localized_status_text(
                        (*pointer).language,
                        &stage,
                        &message,
                    );
                    set_text((*pointer).status, &localized);
                }
                LRESULT(0)
            }
            WM_PROJECT_DONE => {
                let payload = lparam.0 as *mut Result<AudioDescriptionOutcome, String>;
                if payload.is_null() {
                    return LRESULT(0);
                }
                let result = *Box::from_raw(payload);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if pointer.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *pointer;
                state.running = false;
                state.cancel = None;
                set_controls_enabled(state, true);
                match result {
                    Ok(_outcome) => {
                        match load_audio_description_project(&state.project_path) {
                            Ok(project) => {
                                state.project = project;
                                refill_list(state, state.selected_index.unwrap_or(0));
                            }
                            Err(error) => {
                                show_project_error(hwnd, state.language, &error);
                            }
                        }
                        SendMessageW(state.progress, PBM_SETPOS, WPARAM(100), LPARAM(0));
                        set_text(state.status, &labels(state.language).complete);
                        show_project_info(hwnd, state.language, &labels(state.language).success);
                    }
                    Err(error) if error == "cancelled" => {
                        set_text(state.status, &labels(state.language).ready);
                    }
                    Err(error) => {
                        set_text(state.status, &error);
                        show_project_error(hwnd, state.language, &error);
                    }
                }
                LRESULT(0)
            }
            WM_SETFOCUS => {
                focus_descriptions_list(hwnd);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 as u32 == VK_ESCAPE.0 as u32 {
                    crate::log_if_err!(
                        PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)),
                        "Audio description project: PostMessageW failed"
                    );
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() && (*pointer).running {
                    if let Some(cancel) = (*pointer).cancel.as_ref() {
                        cancel.store(true, Ordering::Relaxed);
                    }
                    set_text((*pointer).status, &labels((*pointer).language).canceling);
                    return LRESULT(0);
                }
                if !pointer.is_null() {
                    let state = &mut *pointer;
                    capture_current_draft(state);
                    if has_unapplied_edit(state) {
                        let language_labels = labels(state.language);
                        let text = to_wide(&language_labels.unsaved_close);
                        let title = to_wide(&language_labels.title);
                        let answer = MessageBoxW(
                            hwnd,
                            PCWSTR(text.as_ptr()),
                            PCWSTR(title.as_ptr()),
                            MB_YESNO | MB_ICONQUESTION,
                        );
                        if answer != IDYES {
                            return LRESULT(0);
                        }
                    }
                    stop_preview(state);
                }
                crate::log_if_err!(
                    DestroyWindow(hwnd),
                    "Audio description project: DestroyWindow failed"
                );
                LRESULT(0)
            }
            WM_DESTROY => {
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() {
                    stop_preview(&mut *pointer);
                    let parent = (*pointer).parent;
                    let owner = (*pointer).owner;
                    if with_state(parent, |state| {
                        state.audio_description_project_window = HWND(0);
                    })
                    .is_none()
                    {
                        crate::log_debug(
                            "Audio description project: parent state unavailable during WM_DESTROY",
                        );
                    }
                    if owner.0 != 0
                        && crate::is_window_handle_valid(owner)
                        && IsWindowVisible(owner).as_bool()
                    {
                        SetForegroundWindow(owner);
                        SetFocus(owner);
                    }
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() {
                    SetWindowLongPtrW(
                        hwnd,
                        windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                        0,
                    );
                    let _released_state = Box::from_raw(pointer);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
