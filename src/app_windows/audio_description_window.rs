use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT, InvalidateRect};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY,
    OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Controls::{
    BST_CHECKED, PBM_SETPOS, PBM_SETRANGE32, PROGRESS_CLASSW, WC_BUTTON, WC_COMBOBOXW, WC_EDIT,
    WC_STATIC,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_AUTORADIOBUTTON, BS_DEFPUSHBUTTON, CB_ADDSTRING,
    CB_FINDSTRINGEXACT, CB_GETCOUNT, CB_GETCURSEL, CB_GETLBTEXT, CB_GETLBTEXTLEN, CB_RESETCONTENT,
    CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST, CREATESTRUCTW, CreateWindowExW, DefWindowProcW,
    DestroyWindow, EN_KILLFOCUS, ES_AUTOHSCROLL, ES_PASSWORD, ES_READONLY, GetWindowLongPtrW,
    HMENU, IDC_ARROW, IDNO, IDYES, IsWindowVisible, LoadCursorW, MB_ICONQUESTION, MB_YESNO,
    MB_YESNOCANCEL, MessageBoxW, PostMessageW, SW_HIDE, SW_SHOW, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, ShowWindow, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CREATE,
    WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_SETFOCUS, WM_SETFONT, WNDCLASSW, WS_CAPTION, WS_CHILD,
    WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_GROUP, WS_SYSMENU, WS_TABSTOP,
    WS_VISIBLE,
};
use windows::core::{PCWSTR, PWSTR};

use crate::accessibility::to_wide;
use crate::audio_description::{
    AudioDescriptionCallbacks, AudioDescriptionCharacterCatalogContext,
    AudioDescriptionCharacterCatalogSummary, AudioDescriptionJob, AudioDescriptionOutcome,
    AudioDescriptionResumeSettings, AudioDescriptionVerbosity,
    audio_description_character_catalog_path,
    audio_description_checkpoint_has_generated_descriptions, audio_description_job_from_checkpoint,
    create_audio_description, create_audio_description_dialogue_overlap_fallback, language_code,
    list_audio_description_character_catalogs, load_audio_description_character_catalog_context,
};
use crate::i18n;
use crate::settings::{
    DEFAULT_AUDIO_DESCRIPTION_GEMINI_MODEL, Language, TtsEngine, VoiceInfo,
    default_audio_description_save_folder, save_settings,
};
use crate::tools::audio_description_bridge::{
    AudioDescriptionOverloadDecision, AudioDescriptionQuotaDecision,
};
use crate::{show_error, show_info, with_state};

const CLASS_NAME: &str = "SonarpadCreateAudioDescription";
const ID_INPUT: usize = 9651;
const ID_INPUT_BROWSE: usize = 9652;
const ID_OUTPUT: usize = 9653;
const ID_OUTPUT_BROWSE: usize = 9654;
const ID_LANGUAGE: usize = 9655;
const ID_VERBOSITY: usize = 9656;
const ID_EXTENDED: usize = 9657;
const ID_ENGINE: usize = 9658;
const ID_VOICE: usize = 9659;
const ID_START: usize = 9660;
const ID_CANCEL: usize = 9661;
const ID_CLOSE: usize = 9662;
const ID_SAVE_PROJECT: usize = 9663;
const ID_MODIFY_PROJECT: usize = 9664;
const ID_GEMINI_API_KEY: usize = 9665;
const ID_GEMINI_GET_KEY: usize = 9666;
const ID_GEMINI_MODEL: usize = 9667;
const ID_GEMINI_REFRESH_MODELS: usize = 9668;
const ID_RECOGNIZE_CHARACTERS: usize = 9669;
const ID_KEEP_CHARACTER_CATALOG: usize = 9670;
const ID_CHARACTER_CATALOG: usize = 9671;
const ID_CHARACTER_CATALOG_NAME: usize = 9672;
const ID_CONTINUE_INTERRUPTED: usize = 9673;
const ID_DELETE_VIDEO_AFTER: usize = 9674;
const ID_GEMINI_SHOW_API_KEY: usize = 9675;
const ID_AI_PERSONAL: usize = 9676;
const ID_AI_SONARPAD: usize = 9677;
const ID_SONARPAD_CODE: usize = 9678;
const ID_SONARPAD_REQUEST_CODE: usize = 9679;
const ID_SONARPAD_SHOW_CODE: usize = 9680;
const ID_SONARPAD_BALANCE: usize = 9681;
const ID_VOICE_SETTINGS: usize = 9682;
const ID_RECOGNIZE_SCREEN_TEXT: usize = 9683;
const ID_CREATE_VIDEO: usize = 9684;
const EM_SETPASSWORDCHAR: u32 = 0x00CC;
const SONARPAD_AI_SERVICE_URL: &str = "https://sonarpad.com/sonarpad-ai";

const WM_AD_PROGRESS: u32 = WM_APP + 188;
const WM_AD_STATUS: u32 = WM_APP + 189;
const WM_AD_DONE: u32 = WM_APP + 190;
const WM_AD_VOICES_LOADED: u32 = WM_APP + 191;
const WM_AD_MODELS_LOADED: u32 = WM_APP + 192;
const WM_AD_QUOTA: u32 = WM_APP + 193;
const WM_AD_PLAYER_RETURN: u32 = WM_APP + 194;
const WM_AD_SET_INPUT: u32 = WM_APP + 195;
const WM_AD_SET_RESUME: u32 = WM_APP + 196;
const WM_AD_RESET_NEW: u32 = WM_APP + 197;
const WM_AD_OVERLOAD: u32 = WM_APP + 198;
const WM_AD_RESTORE_RUNNING_FOCUS: u32 = WM_APP + 199;
const WM_AD_SONARPAD_BALANCE: u32 = WM_APP + 200;
const AUDIO_DESCRIPTION_NO_DESCRIPTIONS_ERROR: &str =
    "Audio description: Gemini returned no descriptions";
const AUDIO_DESCRIPTION_NO_SAFE_TTS_SLOT_ERROR: &str =
    "Audio description: no synthesized description can be placed safely between dialogue";
const AUDIO_DESCRIPTION_OVERLAP_NO_RAW_ERROR: &str =
    "Audio description: final overlap fallback has no generated descriptions to reuse";
fn is_audio_description_no_usable_descriptions_error(error: &str) -> bool {
    error == AUDIO_DESCRIPTION_NO_DESCRIPTIONS_ERROR
        || error == AUDIO_DESCRIPTION_NO_SAFE_TTS_SLOT_ERROR
}

struct Labels {
    title: String,
    input: String,
    output: String,
    input_browse: String,
    output_browse: String,
    language: String,
    verbosity: String,
    verbosity_brief: String,
    verbosity_standard: String,
    verbosity_detailed: String,
    extended: String,
    recognize_characters: String,
    recognize_screen_text: String,
    keep_character_catalog: String,
    character_catalog_choose: String,
    character_catalog_selection_label: String,
    character_catalog_new_name_label: String,
    character_catalog_name_error: String,
    save_project: String,
    create_video: String,
    delete_video_after: String,
    modify_project: String,
    ai_access: String,
    ai_personal: String,
    ai_sonarpad: String,
    sonarpad_code: String,
    sonarpad_show_code: String,
    sonarpad_balance: String,
    sonarpad_balance_unavailable: String,
    sonarpad_request_code: String,
    gemini_api_key: String,
    gemini_show_api_key: String,
    gemini_get_key: String,
    gemini_model: String,
    gemini_refresh_models: String,
    gemini_loading_models: String,
    gemini_error_models: String,
    quota_title: String,
    quota_message: String,
    quota_model_prompt: String,
    quota_no_alternative_models: String,
    overload_title: String,
    overload_message: String,
    engine: String,
    voice: String,
    voice_settings: String,
    start: String,
    resume_start: String,
    resume_model: String,
    resume_title: String,
    resume_invalid: String,
    cancel: String,
    close: String,
    ready: String,
    loading_voices: String,
    running: String,
    canceling: String,
    complete: String,
    error_input: String,
    error_output: String,
    error_voice: String,
    error_api_key: String,
    error_sonarpad_code: String,
    error_model: String,
    error_same_path: String,
    character_catalog_saved: String,
    character_catalog_warning: String,
    open_title: String,
    save_title: String,
}

struct WindowState {
    parent: HWND,
    language: Language,
    input: HWND,
    output: HWND,
    language_combo: HWND,
    verbosity_combo: HWND,
    extended_checkbox: HWND,
    recognize_characters_checkbox: HWND,
    recognize_screen_text_checkbox: HWND,
    keep_character_catalog_checkbox: HWND,
    character_catalog_label: HWND,
    character_catalog_combo: HWND,
    character_catalog_name_label: HWND,
    character_catalog_name_edit: HWND,
    character_catalogs: Vec<AudioDescriptionCharacterCatalogSummary>,
    save_project_checkbox: HWND,
    create_video_checkbox: HWND,
    delete_video_after_checkbox: HWND,
    ai_personal_radio: HWND,
    ai_sonarpad_radio: HWND,
    sonarpad_code_label: HWND,
    sonarpad_code_edit: HWND,
    sonarpad_show_code_checkbox: HWND,
    sonarpad_balance_label: HWND,
    sonarpad_balance_edit: HWND,
    sonarpad_request_code_button: HWND,
    gemini_api_key_label: HWND,
    gemini_api_key_edit: HWND,
    gemini_get_key_button: HWND,
    gemini_show_api_key_checkbox: HWND,
    gemini_model_label: HWND,
    gemini_model_combo: HWND,
    gemini_refresh_models_button: HWND,
    engine_combo: HWND,
    voice_combo: HWND,
    voice_settings_button: HWND,
    tts_rate: i32,
    tts_volume: i32,
    progress: HWND,
    status: HWND,
    start_button: HWND,
    // Keep this HWND in state: validation errors for interrupted projects must restore focus here.
    continue_interrupted_button: HWND,
    cancel_button: HWND,
    close_button: HWND,
    setup_controls: Vec<HWND>,
    voices: Vec<VoiceInfo>,
    preferred_voice: String,
    running: bool,
    cancel: Option<Arc<AtomicBool>>,
    exhausted_gemini_models: Vec<String>,
    return_to_editor_after_player: bool,
    source_player_path: Option<PathBuf>,
    resume_checkpoint_path: Option<PathBuf>,
    resume_mode: bool,
    retry_job: Option<AudioDescriptionJob>,
    retry_input_to_trash: Option<PathBuf>,
    short_silence_retry_used: bool,
}

struct QuotaPromptRequest {
    model: String,
    error: String,
    response: mpsc::SyncSender<AudioDescriptionQuotaDecision>,
}

struct OverloadPromptRequest {
    model: String,
    error: String,
    response: mpsc::SyncSender<AudioDescriptionOverloadDecision>,
}

struct AudioDescriptionDonePayload {
    result: Result<AudioDescriptionOutcome, String>,
    input_to_trash: Option<PathBuf>,
}

struct SonarpadBalancePayload {
    access_code: String,
    result: Result<f64, String>,
}

#[repr(C)]
struct ShellFileOperationW {
    hwnd: isize,
    w_func: u32,
    p_from: *const u16,
    p_to: *const u16,
    flags: u16,
    any_operations_aborted: i32,
    name_mappings: *mut c_void,
    progress_title: *const u16,
}

#[link(name = "shell32")]
unsafe extern "system" {
    #[link_name = "SHFileOperationW"]
    fn shell_file_operation_w(operation: *mut ShellFileOperationW) -> i32;
}

fn should_move_input_to_recycle_bin(delete_requested: bool, save_project: bool) -> bool {
    delete_requested && !save_project
}

fn move_input_video_to_recycle_bin(owner: HWND, path: &Path) -> Result<(), String> {
    const FO_DELETE: u32 = 3;
    const FOF_SILENT: u16 = 0x0004;
    const FOF_NOCONFIRMATION: u16 = 0x0010;
    const FOF_ALLOWUNDO: u16 = 0x0040;
    const FOF_NOERRORUI: u16 = 0x0400;

    let full_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("could not resolve the source video path: {error}"))?
            .join(path)
    };
    if !full_path.is_file() {
        return Err(format!("source video not found: {}", full_path.display()));
    }

    // SHFileOperation expects a double-NUL-terminated list of fully qualified paths.
    let mut source: Vec<u16> = full_path.as_os_str().encode_wide().collect();
    source.push(0);
    source.push(0);
    let mut operation = ShellFileOperationW {
        hwnd: owner.0,
        w_func: FO_DELETE,
        p_from: source.as_ptr(),
        p_to: std::ptr::null(),
        flags: FOF_SILENT | FOF_NOCONFIRMATION | FOF_ALLOWUNDO | FOF_NOERRORUI,
        any_operations_aborted: 0,
        name_mappings: std::ptr::null_mut(),
        progress_title: std::ptr::null(),
    };
    let result = unsafe { shell_file_operation_w(&mut operation) };
    if result != 0 {
        return Err(format!(
            "Windows Recycle Bin operation failed with code {result}"
        ));
    }
    if operation.any_operations_aborted != 0 {
        return Err("Windows Recycle Bin operation was aborted".to_string());
    }
    Ok(())
}

#[derive(Clone)]
struct AudioDescriptionPlayerReturnContext {
    parent: isize,
    window: isize,
    output_path: PathBuf,
}

static AUDIO_DESCRIPTION_PLAYER_RETURN: OnceLock<
    Mutex<Option<AudioDescriptionPlayerReturnContext>>,
> = OnceLock::new();

fn player_return_context() -> &'static Mutex<Option<AudioDescriptionPlayerReturnContext>> {
    AUDIO_DESCRIPTION_PLAYER_RETURN.get_or_init(|| Mutex::new(None))
}

fn remember_player_return(window: HWND, parent: HWND, output_path: PathBuf) {
    if let Ok(mut stored) = player_return_context().lock() {
        *stored = Some(AudioDescriptionPlayerReturnContext {
            parent: parent.0,
            window: window.0,
            output_path,
        });
    }
}

fn clear_player_return_for_window(window: HWND) {
    if let Ok(mut stored) = player_return_context().lock()
        && stored
            .as_ref()
            .is_some_and(|context| context.window == window.0)
    {
        *stored = None;
    }
}

fn is_hidden_for_output_player(parent: HWND, window: HWND) -> bool {
    player_return_context()
        .lock()
        .ok()
        .and_then(|stored| stored.clone())
        .is_some_and(|context| context.parent == parent.0 && context.window == window.0)
}

fn is_running_audio_description_window(window: HWND) -> bool {
    if window.0 == 0 || !crate::is_window_handle_valid(window) {
        return false;
    }
    unsafe {
        let pointer = GetWindowLongPtrW(
            window,
            windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
        ) as *const WindowState;
        !pointer.is_null() && (*pointer).running
    }
}

pub(crate) fn blocks_parent_focus(parent: HWND, window: HWND) -> bool {
    if window.0 == 0
        || !crate::is_window_handle_valid(window)
        || is_hidden_for_output_player(parent, window)
    {
        return false;
    }

    // The audio-description window is intentionally detached from the main Win32
    // owner so it has its own taskbar/Alt+Tab lifetime. While a job is running,
    // delayed editor-focus retries must not steal focus while the progress window
    // (or another application) is in front. If the user explicitly activates the
    // Sonarpad main window with Alt+Tab/click, GetForegroundWindow is the parent: in
    // that case the editor must remain usable and the progress window must not be
    // forced back to the foreground.
    if is_running_audio_description_window(window) {
        unsafe {
            return windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() != parent;
        }
    }

    unsafe {
        let foreground = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if foreground == window {
            return true;
        }
        if foreground.0 != 0
            && GetWindowLongPtrW(
                foreground,
                windows::Win32::UI::WindowsAndMessaging::GWLP_HWNDPARENT,
            ) == window.0
        {
            return true;
        }
        GetWindowLongPtrW(
            window,
            windows::Win32::UI::WindowsAndMessaging::GWLP_HWNDPARENT,
        ) == parent.0
    }
}

pub(crate) fn blocks_main_window_close(parent: HWND, window: HWND) -> bool {
    window.0 != 0
        && crate::is_window_handle_valid(window)
        && !is_hidden_for_output_player(parent, window)
}

pub(crate) fn visible_window(parent: HWND) -> HWND {
    let window = with_state(parent, |state| state.audio_description_window).unwrap_or(HWND(0));
    if window.0 != 0
        && crate::is_window_handle_valid(window)
        && unsafe { IsWindowVisible(window).as_bool() }
    {
        window
    } else {
        HWND(0)
    }
}

pub(crate) fn restore_on_parent_activation(parent: HWND) -> bool {
    let window = with_state(parent, |state| state.audio_description_window).unwrap_or(HWND(0));
    if !blocks_parent_focus(parent, window) {
        return false;
    }

    unsafe {
        let foreground = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if foreground == window {
            // The dialog is already active.  Returning true prevents the main-window
            // focus retry timers from stealing focus from whichever child control the
            // user reached with Tab.
            return true;
        }
        if foreground != parent {
            // Never pull Sonarpad back in front of the desktop or another application.
            // This helper is only for the moment the owner itself is reactivated.
            return false;
        }
        ShowWindow(window, SW_SHOW);
        SetForegroundWindow(window);
        SendMessageW(window, WM_SETFOCUS, WPARAM(0), LPARAM(0));
    }
    true
}

pub(crate) fn restore_after_player_stop(parent: HWND, stopped_path: Option<&Path>) -> bool {
    let Some(stopped_path) = stopped_path else {
        return false;
    };
    let context = player_return_context()
        .lock()
        .ok()
        .and_then(|stored| stored.clone())
        .filter(|context| {
            context.parent == parent.0 && context.output_path.as_path() == stopped_path
        });
    let Some(context) = context else {
        return false;
    };
    if let Ok(mut stored) = player_return_context().lock() {
        *stored = None;
    }
    let window = HWND(context.window);
    if window.0 == 0 || !crate::is_window_handle_valid(window) {
        return false;
    }
    if with_state(parent, |state| state.audio_description_window = window).is_none() {
        crate::log_debug(
            "Audio description: parent state unavailable while restoring player return window",
        );
    }
    unsafe {
        ShowWindow(window, SW_SHOW);
        SetForegroundWindow(window);
        crate::log_if_err!(
            PostMessageW(window, WM_AD_PLAYER_RETURN, WPARAM(0), LPARAM(0)),
            "Audio description: PostMessageW failed"
        );
    }
    true
}

fn open_result_in_player(window: HWND, parent: HWND, output_path: PathBuf) {
    remember_player_return(window, parent, output_path.clone());
    unsafe {
        ShowWindow(window, SW_HIDE);
        SetForegroundWindow(parent);
    }
    crate::queue_audio_files_and_play(parent, vec![output_path]);
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
        title: i18n::tr(language, "audio_description.title"),
        input: i18n::tr(language, "audio_description.input"),
        output: i18n::tr(language, "audio_description.output"),
        input_browse: i18n::tr(language, "audio_description.browse_input"),
        output_browse: i18n::tr(language, "audio_description.browse_output"),
        language: i18n::tr(language, "audio_description.language"),
        verbosity: i18n::tr(language, "audio_description.verbosity"),
        verbosity_brief: i18n::tr(language, "audio_description.verbosity.brief"),
        verbosity_standard: i18n::tr(language, "audio_description.verbosity.standard"),
        verbosity_detailed: i18n::tr(language, "audio_description.verbosity.detailed"),
        extended: i18n::tr(language, "audio_description.extended"),
        recognize_characters: i18n::tr(language, "audio_description.recognize_characters"),
        recognize_screen_text: i18n::tr(language, "audio_description.recognize_screen_text"),
        keep_character_catalog: i18n::tr(language, "audio_description.keep_character_catalog"),
        character_catalog_choose: i18n::tr(
            language,
            "audio_description.character_catalog.new_option",
        ),
        character_catalog_selection_label: i18n::tr(
            language,
            "audio_description.character_catalog.selection_label",
        ),
        character_catalog_new_name_label: i18n::tr(
            language,
            "audio_description.character_catalog.new_name_label",
        ),
        character_catalog_name_error: i18n::tr(
            language,
            "audio_description.character_catalog.name_error",
        ),
        save_project: i18n::tr(language, "audio_description.save_project"),
        create_video: i18n::tr(language, "audio_description.create_video"),
        delete_video_after: i18n::tr(language, "audio_description.delete_video_after"),
        modify_project: i18n::tr(language, "audio_description.modify_project"),
        ai_access: i18n::tr(language, "audio_description.ai_access"),
        ai_personal: i18n::tr(language, "audio_description.ai_access.personal"),
        ai_sonarpad: i18n::tr(language, "audio_description.ai_access.sonarpad"),
        sonarpad_code: i18n::tr(language, "audio_description.sonarpad_code"),
        sonarpad_show_code: i18n::tr(language, "audio_description.sonarpad_show_code"),
        sonarpad_balance: i18n::tr(language, "audio_description.sonarpad_balance"),
        sonarpad_balance_unavailable: i18n::tr(
            language,
            "audio_description.sonarpad_balance_unavailable",
        ),
        sonarpad_request_code: i18n::tr(language, "audio_description.sonarpad_request_code"),
        gemini_api_key: i18n::tr(language, "audio_description.gemini_api_key"),
        gemini_show_api_key: i18n::tr(language, "audio_description.show_api_key"),
        gemini_get_key: i18n::tr(language, "audio_description.gemini_get_key"),
        gemini_model: i18n::tr(language, "audio_description.gemini_model"),
        gemini_refresh_models: i18n::tr(language, "audio_description.gemini_refresh_models"),
        gemini_loading_models: i18n::tr(language, "audio_description.gemini_loading_models"),
        gemini_error_models: i18n::tr(language, "audio_description.gemini_error_models"),
        quota_title: i18n::tr(language, "audio_description.quota.title"),
        quota_message: i18n::tr(language, "audio_description.quota.message"),
        quota_model_prompt: i18n::tr(language, "audio_description.quota.model_prompt"),
        quota_no_alternative_models: i18n::tr(
            language,
            "audio_description.quota.no_alternative_models",
        ),
        overload_title: i18n::tr(language, "audio_description.overload.title"),
        overload_message: i18n::tr(language, "audio_description.overload.message"),
        engine: i18n::tr(language, "audio_description.engine"),
        voice: i18n::tr(language, "audio_description.voice"),
        voice_settings: i18n::tr(language, "audio_description.voice_settings"),
        start: i18n::tr(language, "audio_description.start"),
        resume_start: i18n::tr(language, "audio_description.resume.start"),
        resume_model: i18n::tr(language, "audio_description.resume.model"),
        resume_title: i18n::tr(language, "audio_description.resume.title"),
        resume_invalid: i18n::tr(language, "audio_description.resume.invalid"),
        cancel: i18n::tr(language, "audio_description.cancel"),
        close: i18n::tr(language, "audio_description.close"),
        ready: i18n::tr(language, "audio_description.status.ready"),
        loading_voices: i18n::tr(language, "audio_description.status.loading_voices"),
        running: i18n::tr(language, "audio_description.status.running"),
        canceling: i18n::tr(language, "audio_description.status.canceling"),
        complete: i18n::tr(language, "audio_description.status.complete"),
        error_input: i18n::tr(language, "audio_description.error.input"),
        error_output: i18n::tr(language, "audio_description.error.output"),
        error_voice: i18n::tr(language, "audio_description.error.voice"),
        error_api_key: i18n::tr(language, "audio_description.error.api_key"),
        error_sonarpad_code: i18n::tr(language, "audio_description.error.sonarpad_code"),
        error_model: i18n::tr(language, "audio_description.error.model"),
        error_same_path: i18n::tr(language, "audio_description.error.same_path"),
        character_catalog_saved: i18n::tr(language, "audio_description.character_catalog.saved"),
        character_catalog_warning: i18n::tr(
            language,
            "audio_description.character_catalog.warning",
        ),
        open_title: i18n::tr(language, "audio_description.open_title"),
        save_title: i18n::tr(language, "audio_description.save_title"),
    }
}

pub fn handle_navigation(hwnd: HWND, msg: &windows::Win32::UI::WindowsAndMessaging::MSG) -> bool {
    if !unsafe { IsWindowVisible(hwnd).as_bool() } {
        return false;
    }
    if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_ESCAPE.0 as u32 {
        crate::log_if_err!(
            crate::post_message_w_safe(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)),
            "Audio description: PostMessageW failed"
        );
        return true;
    }
    false
}

pub fn open(parent: HWND) {
    let hwnd = open_window(parent);
    if hwnd.0 == 0 || !crate::is_window_handle_valid(hwnd) {
        return;
    }
    unsafe {
        SendMessageW(hwnd, WM_AD_RESET_NEW, WPARAM(0), LPARAM(0));
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
    }
}

pub fn open_with_input(parent: HWND, input_path: PathBuf) {
    if !input_path.is_file() || !crate::file_handler::is_video_path(input_path.as_path()) {
        crate::log_debug(&format!(
            "Audio description: refusing automatic non-video input {}; opening empty window",
            input_path.display()
        ));
        open(parent);
        return;
    }

    let hwnd = open_window(parent);
    if hwnd.0 == 0 || !crate::is_window_handle_valid(hwnd) {
        return;
    }
    unsafe {
        SendMessageW(hwnd, WM_AD_RESET_NEW, WPARAM(0), LPARAM(0));
    }
    post_boxed_message(hwnd, WM_AD_SET_INPUT, WPARAM(0), Box::new(input_path));
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
    }
}

fn continue_interrupted_from_window(hwnd: HWND, state: &mut WindowState) {
    let available_models = combo_items(state.gemini_model_combo);
    let current_model = get_text(state.gemini_model_combo).trim().to_string();
    let Some(selection) =
        crate::app_windows::audio_description_resume_window::choose_resume_checkpoint(
            hwnd,
            state.parent,
            state.language,
            available_models,
            current_model,
        )
    else {
        crate::log_debug("Audio description: resume selector cancelled; closing creation window");
        crate::log_if_err!(
            crate::post_message_w_safe(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)),
            "Audio description: PostMessageW failed while closing cancelled resume window"
        );
        return;
    };

    state.resume_checkpoint_path = Some(selection.checkpoint_path);
    state.resume_mode = false;
    refill_gemini_model_combo(
        state.gemini_model_combo,
        combo_items(state.gemini_model_combo),
        &selection.gemini_model,
    );
    let current_labels = labels(state.language);
    set_text(hwnd, &current_labels.resume_title);
    crate::log_debug(&format!(
        "Audio description: resume selector accepted project and model {}; starting immediately",
        selection.gemini_model
    ));
    start_job(hwnd, state);
    if state.running {
        // The project/model selector runs its own modal message loop.  When that child
        // is destroyed Windows can briefly reactivate the main Sonarpad editor before
        // this WM_COMMAND returns.  Restore the running audio-description window on the
        // next message-loop turn, after all selector activation messages have drained.
        crate::log_if_err!(
            crate::post_message_w_safe(hwnd, WM_AD_RESTORE_RUNNING_FOCUS, WPARAM(0), LPARAM(0),),
            "Audio description: failed to schedule running-window focus after resume"
        );
    }
}

fn open_window(parent: HWND) -> HWND {
    let existing = with_state(parent, |state| state.audio_description_window).unwrap_or(HWND(0));
    if existing.0 != 0 && crate::is_window_handle_valid(existing) {
        return existing;
    }
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
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
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT
                | WS_EX_DLGMODALFRAME
                | windows::Win32::UI::WindowsAndMessaging::WS_EX_APPWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU,
            120,
            90,
            700,
            890,
            parent,
            HMENU(0),
            hinstance,
            None,
        )
    };
    if hwnd.0 != 0 {
        // The parent hwnd was supplied during WM_CREATE only so this window could load
        // Sonarpad settings and keep its existing callbacks. Detach the Win32 owner before
        // showing it: the audio-description UI then has its own taskbar/Alt+Tab lifetime
        // and the main Sonarpad window remains independently usable.
        let _previous_owner = unsafe {
            SetWindowLongPtrW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWLP_HWNDPARENT,
                0,
            )
        };
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
        }
        if with_state(parent, |state| state.audio_description_window = hwnd).is_none() {
            crate::log_debug(
                "Audio description: parent state unavailable while registering window",
            );
        }
    }
    hwnd
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "audio_description_window_proc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || window_proc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

pub(crate) fn localized_status_text(
    language: Language,
    stage: &str,
    fallback_message: &str,
) -> String {
    let key = match stage {
        "analysis_prepare" => Some("audio_description.progress.analysis_prepare"),
        "pyannote_prepare" => Some("audio_description.progress.pyannote_prepare"),
        "chunk_prepare" => Some("audio_description.progress.chunk_prepare"),
        "download" => Some("audio_description.progress.download"),
        "pyannote" | "pyannote_analyzing" => Some("audio_description.progress.pyannote_analyzing"),
        "pyannote_no_audio" => Some("audio_description.progress.pyannote_no_audio"),
        "gemini" | "gemini_processing" => Some("audio_description.progress.gemini_processing"),
        "gemini_start" => Some("audio_description.progress.gemini_start"),
        "gemini_uploading" => Some("audio_description.progress.gemini_uploading"),
        "gemini_waiting" => Some("audio_description.progress.gemini_waiting"),
        "gemini_contacting" => Some("audio_description.progress.gemini_contacting"),
        "gemini_response" => Some("audio_description.progress.gemini_response"),
        "gemini_repair" => Some("audio_description.progress.gemini_repair"),
        "gemini_retry" => Some("audio_description.progress.gemini_retry"),
        "language_correction" => Some("audio_description.progress.language_correction"),
        "finalize" => Some("audio_description.progress.finalize"),
        "ready_for_tts" => Some("audio_description.progress.ready_for_tts"),
        "tts" => Some("audio_description.progress.tts"),
        "schedule" => Some("audio_description.progress.schedule"),
        "export" => Some("audio_description.progress.export"),
        "project" => Some("audio_description.progress.project"),
        "complete" => Some("audio_description.status.complete"),
        "tts_edit" => Some("audio_description.progress.tts_edit"),
        "schedule_edit" => Some("audio_description.progress.schedule_edit"),
        "export_edit" => Some("audio_description.progress.export_edit"),
        "complete_edit" => Some("audio_description.progress.complete_edit"),
        _ => None,
    };

    if stage == "pyannote_done" {
        let count = fallback_message.trim().parse::<usize>().unwrap_or(0);
        let count_text = count.to_string();
        return i18n::tr_f(
            language,
            "audio_description.progress.pyannote_done",
            &[("count", &count_text)],
        );
    }
    if stage == "gemini_chunk" {
        let parsed = serde_json::from_str::<serde_json::Value>(fallback_message).ok();
        let current = parsed
            .as_ref()
            .and_then(|value| value.get("current"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1);
        let total = parsed
            .as_ref()
            .and_then(|value| value.get("total"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(current);
        let current_text = current.to_string();
        let total_text = total.to_string();
        return i18n::tr_f(
            language,
            "audio_description.progress.gemini_chunk",
            &[("current", &current_text), ("total", &total_text)],
        );
    }
    if let Some(translation_key) = key {
        let text = i18n::tr(language, translation_key);
        if !text.trim().is_empty() {
            return text;
        }
    }
    if stage.starts_with("gemini") {
        return i18n::tr(language, "audio_description.progress.gemini_processing");
    }
    fallback_message.to_string()
}

fn add_combo_item(combo: HWND, text: &str) {
    unsafe {
        SendMessageW(
            combo,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(to_wide(text).as_ptr() as isize),
        );
    }
}

fn create_label(hwnd: HWND, text: &str, x: i32, y: i32, width: i32, hfont: HFONT) -> HWND {
    unsafe {
        let control = CreateWindowExW(
            Default::default(),
            WC_STATIC,
            PCWSTR(to_wide(text).as_ptr()),
            WS_CHILD | WS_VISIBLE,
            x,
            y,
            width,
            18,
            hwnd,
            HMENU(0),
            HINSTANCE(0),
            None,
        );
        if hfont.0 != 0 {
            SendMessageW(control, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
        }
        control
    }
}

fn get_text(hwnd: HWND) -> String {
    let len = crate::get_window_text_length_w_safe(hwnd) as usize;
    let mut buffer = vec![0_u16; len.saturating_add(1)];
    let read = crate::get_window_text_w_safe(hwnd, &mut buffer) as usize;
    String::from_utf16_lossy(&buffer[..read.min(len)])
}

fn combo_items(combo: HWND) -> Vec<String> {
    let count = unsafe { SendMessageW(combo, CB_GETCOUNT, WPARAM(0), LPARAM(0)).0 };
    if count <= 0 {
        return Vec::new();
    }
    let mut items = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        let len = unsafe { SendMessageW(combo, CB_GETLBTEXTLEN, WPARAM(index), LPARAM(0)).0 };
        if len <= 0 {
            continue;
        }
        let mut buffer = vec![0_u16; len as usize + 1];
        unsafe {
            SendMessageW(
                combo,
                CB_GETLBTEXT,
                WPARAM(index),
                LPARAM(buffer.as_mut_ptr() as isize),
            );
        }
        let value = String::from_utf16_lossy(&buffer[..len as usize]);
        if !value.trim().is_empty() {
            items.push(value);
        }
    }
    items
}

fn suggested_alternative_model(combo: HWND, current: &str) -> String {
    let items = combo_items(combo);
    let current = canonical_gemini_model_id(current);
    let default = canonical_gemini_model_id(DEFAULT_AUDIO_DESCRIPTION_GEMINI_MODEL);
    if current != default
        && items
            .iter()
            .any(|model| canonical_gemini_model_id(model) == default)
    {
        return DEFAULT_AUDIO_DESCRIPTION_GEMINI_MODEL.to_string();
    }
    items
        .into_iter()
        .find(|model| canonical_gemini_model_id(model) != current)
        .unwrap_or_default()
}

fn set_text(hwnd: HWND, text: &str) {
    let wide = to_wide(text);
    crate::log_if_err!(
        crate::set_window_text_w_safe(hwnd, PCWSTR(wide.as_ptr())),
        "Audio description: SetWindowTextW failed"
    );
}

fn set_path(hwnd: HWND, path: &Path) {
    set_text(hwnd, &path.to_string_lossy());
}

fn configured_output_folder(parent: HWND) -> PathBuf {
    let folder = with_state(parent, |state| {
        state.settings.audio_description_save_folder.clone()
    })
    .filter(|folder| !folder.trim().is_empty())
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from(default_audio_description_save_folder()));
    if let Err(error) = std::fs::create_dir_all(&folder) {
        crate::log_debug(&format!(
            "Audio description: could not create configured output folder {}: {error}",
            folder.display()
        ));
    }
    folder
}

fn default_output(parent: HWND, input: &Path, create_video: bool) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("audio");
    let extension = if create_video { "mp4" } else { "mp3" };
    configured_output_folder(parent).join(format!("{stem}_audiodescritto.{extension}"))
}

fn open_input_dialog(parent: HWND, language: Language, labels: &Labels) -> Option<PathBuf> {
    unsafe {
        let filter_raw = i18n::tr(language, "podcasts.download_filter");
        let filter = to_wide(&filter_raw.replace("\\0", "\0"));
        let title = to_wide(&labels.open_title);
        let mut buffer = [0_u16; 2048];
        let mut dialog = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: parent,
            lpstrFile: PWSTR(buffer.as_mut_ptr()),
            nMaxFile: buffer.len() as u32,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrTitle: PCWSTR(title.as_ptr()),
            Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY,
            ..Default::default()
        };
        if !GetOpenFileNameW(&mut dialog).as_bool() {
            return None;
        }
        let len = buffer.iter().position(|value| *value == 0)?;
        (len > 0).then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..len])))
    }
}

fn open_output_dialog(
    parent: HWND,
    language: Language,
    labels: &Labels,
    initial: &Path,
    create_video: bool,
) -> Option<PathBuf> {
    unsafe {
        let all_files = i18n::tr(language, "dialog.all_files");
        let (filter_text, default_ext) = if create_video {
            (
                format!(
                    "MP4 video (*.mp4)\0*.mp4\0Matroska video (*.mkv)\0*.mkv\0{} (*.*)\0*.*\0\0",
                    all_files
                ),
                "mp4",
            )
        } else {
            (
                format!("MP3 (*.mp3)\0*.mp3\0{} (*.*)\0*.*\0\0", all_files),
                "mp3",
            )
        };
        let filter = to_wide(&filter_text);
        let title = to_wide(&labels.save_title);
        let extension = to_wide(default_ext);
        let mut buffer = [0_u16; 2048];
        let initial_wide = to_wide(&initial.to_string_lossy());
        let copy_len = initial_wide.len().min(buffer.len() - 1);
        buffer[..copy_len].copy_from_slice(&initial_wide[..copy_len]);
        let mut dialog = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: parent,
            lpstrFile: PWSTR(buffer.as_mut_ptr()),
            nMaxFile: buffer.len() as u32,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrTitle: PCWSTR(title.as_ptr()),
            lpstrDefExt: PCWSTR(extension.as_ptr()),
            Flags: OFN_EXPLORER | OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST,
            ..Default::default()
        };
        if !GetSaveFileNameW(&mut dialog).as_bool() {
            return None;
        }
        let len = buffer.iter().position(|value| *value == 0)?;
        let mut path = PathBuf::from(String::from_utf16_lossy(&buffer[..len]));
        if path.extension().is_none() {
            path.set_extension(default_ext);
        }
        Some(path)
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

fn checkbox_checked(control: HWND) -> bool {
    unsafe { SendMessageW(control, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == BST_CHECKED.0 as isize }
}

fn update_gemini_api_key_visibility(state: &WindowState) {
    let show = checkbox_checked(state.gemini_show_api_key_checkbox);
    let password_char = if show { 0 } else { '*' as usize };
    unsafe {
        SendMessageW(
            state.gemini_api_key_edit,
            EM_SETPASSWORDCHAR,
            WPARAM(password_char),
            LPARAM(0),
        );
        if !InvalidateRect(state.gemini_api_key_edit, None, true).as_bool() {
            crate::log_debug("Audio description: failed to redraw Gemini API key field");
        }
    }
}

fn update_sonarpad_code_visibility(state: &WindowState) {
    let show = checkbox_checked(state.sonarpad_show_code_checkbox);
    let password_char = if show { 0 } else { '*' as usize };
    unsafe {
        SendMessageW(
            state.sonarpad_code_edit,
            EM_SETPASSWORDCHAR,
            WPARAM(password_char),
            LPARAM(0),
        );
        if !InvalidateRect(state.sonarpad_code_edit, None, true).as_bool() {
            crate::log_debug("Audio description: failed to redraw Sonarpad AI code field");
        }
    }
}

fn format_sonarpad_balance(language: Language, balance: f64) -> String {
    let value = format!("{balance:.2}");
    let localized = match language {
        Language::English => value,
        _ => value.replace('.', ","),
    };
    format!("{localized} €")
}

fn refresh_sonarpad_balance(hwnd: HWND, state: &WindowState) {
    if state.running || !using_sonarpad_ai(state) {
        return;
    }
    let access_code = get_text(state.sonarpad_code_edit).trim().to_string();
    if !access_code.starts_with("sp_") {
        set_text(
            state.sonarpad_balance_edit,
            &labels(state.language).sonarpad_balance_unavailable,
        );
        return;
    }
    let device_id = with_state(state.parent, |app| {
        app.settings.sonarpad_ai_device_id.clone()
    })
    .unwrap_or_default();
    if device_id.trim().is_empty() {
        set_text(
            state.sonarpad_balance_edit,
            &labels(state.language).sonarpad_balance_unavailable,
        );
        return;
    }
    set_text(state.sonarpad_balance_edit, "…");
    thread::spawn(move || {
        let result = (|| -> Result<f64, String> {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|err| err.to_string())?;
            let activate_url = format!("{SONARPAD_AI_SERVICE_URL}/v1/activate");
            let activate = client
                .post(&activate_url)
                .json(&serde_json::json!({
                    "code": access_code.clone(),
                    "device_id": device_id,
                    "device_name": "Sonarpad Windows",
                }))
                .send()
                .map_err(|err| err.to_string())?;
            if !activate.status().is_success() {
                return Err(format!("HTTP {}", activate.status().as_u16()));
            }
            let activate_json: serde_json::Value =
                activate.json().map_err(|err| err.to_string())?;
            let session_token = activate_json
                .get("session_token")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            if !session_token.starts_with("sst_") {
                return Err("invalid session response".to_string());
            }
            let account_url = format!("{SONARPAD_AI_SERVICE_URL}/v1/account");
            let account = client
                .get(&account_url)
                .bearer_auth(session_token)
                .send()
                .map_err(|err| err.to_string())?;
            if !account.status().is_success() {
                return Err(format!("HTTP {}", account.status().as_u16()));
            }
            let account_json: serde_json::Value = account.json().map_err(|err| err.to_string())?;
            account_json
                .get("balance_eur")
                .and_then(serde_json::Value::as_f64)
                .ok_or_else(|| "missing balance".to_string())
        })();
        post_boxed_message(
            hwnd,
            WM_AD_SONARPAD_BALANCE,
            WPARAM(0),
            Box::new(SonarpadBalancePayload {
                access_code,
                result,
            }),
        );
    });
}

fn selected_voice_name(state: &WindowState) -> Option<String> {
    let index = unsafe { SendMessageW(state.voice_combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
    (index >= 0)
        .then(|| state.voices.get(index as usize))
        .flatten()
        .map(|voice| voice.short_name.clone())
}

fn selected_character_catalog_name(state: &WindowState) -> String {
    let index = unsafe {
        SendMessageW(
            state.character_catalog_combo,
            CB_GETCURSEL,
            WPARAM(0),
            LPARAM(0),
        )
        .0
    };
    if index <= 0 {
        String::new()
    } else {
        state
            .character_catalogs
            .get(index as usize - 1)
            .map(|catalog| catalog.name.clone())
            .unwrap_or_default()
    }
}

fn suggested_character_catalog_name(input: &str) -> String {
    Path::new(input)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|stem| !stem.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn ensure_new_character_catalog_name(state: &WindowState) {
    let selected = unsafe {
        SendMessageW(
            state.character_catalog_combo,
            CB_GETCURSEL,
            WPARAM(0),
            LPARAM(0),
        )
        .0
    };
    if selected == 0
        && get_text(state.character_catalog_name_edit)
            .trim()
            .is_empty()
    {
        let suggested = suggested_character_catalog_name(&get_text(state.input));
        if !suggested.is_empty() {
            set_text(state.character_catalog_name_edit, &suggested);
        }
    }
}

fn refill_character_catalog_combo(state: &WindowState, preferred_name: &str) {
    let labels = labels(state.language);
    unsafe {
        SendMessageW(
            state.character_catalog_combo,
            CB_RESETCONTENT,
            WPARAM(0),
            LPARAM(0),
        );
    }
    add_combo_item(
        state.character_catalog_combo,
        &labels.character_catalog_choose,
    );
    for catalog in &state.character_catalogs {
        add_combo_item(state.character_catalog_combo, &catalog.name);
    }
    let selected = state
        .character_catalogs
        .iter()
        .position(|catalog| catalog.name.eq_ignore_ascii_case(preferred_name))
        .map(|index| index + 1)
        .unwrap_or(0);
    unsafe {
        SendMessageW(
            state.character_catalog_combo,
            CB_SETCURSEL,
            WPARAM(selected),
            LPARAM(0),
        );
    }
}

fn update_character_catalog_visibility(state: &WindowState) {
    let recognize = checkbox_checked(state.recognize_characters_checkbox);
    if !recognize && checkbox_checked(state.keep_character_catalog_checkbox) {
        unsafe {
            SendMessageW(
                state.keep_character_catalog_checkbox,
                BM_SETCHECK,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
    let keep = recognize && checkbox_checked(state.keep_character_catalog_checkbox);
    unsafe {
        ShowWindow(
            state.keep_character_catalog_checkbox,
            if recognize { SW_SHOW } else { SW_HIDE },
        );
        EnableWindow(
            state.keep_character_catalog_checkbox,
            recognize && !state.running,
        );
        ShowWindow(
            state.character_catalog_label,
            if keep { SW_SHOW } else { SW_HIDE },
        );
        ShowWindow(
            state.character_catalog_combo,
            if keep { SW_SHOW } else { SW_HIDE },
        );
        EnableWindow(state.character_catalog_combo, keep && !state.running);
        let selected = SendMessageW(
            state.character_catalog_combo,
            CB_GETCURSEL,
            WPARAM(0),
            LPARAM(0),
        )
        .0;
        let show_new_name = keep && selected == 0;
        ShowWindow(
            state.character_catalog_name_label,
            if show_new_name { SW_SHOW } else { SW_HIDE },
        );
        ShowWindow(
            state.character_catalog_name_edit,
            if show_new_name { SW_SHOW } else { SW_HIDE },
        );
        EnableWindow(
            state.character_catalog_name_edit,
            show_new_name && !state.running,
        );
    }
    if keep {
        ensure_new_character_catalog_name(state);
    }
}

enum CharacterCatalogPreparation {
    Disabled,
    Cancelled,
    Ready(AudioDescriptionCharacterCatalogContext),
}

fn prepare_character_catalog(
    hwnd: HWND,
    state: &mut WindowState,
    labels: &Labels,
) -> Result<CharacterCatalogPreparation, String> {
    if !checkbox_checked(state.recognize_characters_checkbox)
        || !checkbox_checked(state.keep_character_catalog_checkbox)
    {
        return Ok(CharacterCatalogPreparation::Disabled);
    }

    let selected_index = unsafe {
        SendMessageW(
            state.character_catalog_combo,
            CB_GETCURSEL,
            WPARAM(0),
            LPARAM(0),
        )
        .0
    };
    if selected_index > 0
        && let Some(catalog) = state.character_catalogs.get(selected_index as usize - 1)
    {
        return load_audio_description_character_catalog_context(
            catalog.name.clone(),
            catalog.path.clone(),
        )
        .map(CharacterCatalogPreparation::Ready);
    }

    let name_value = get_text(state.character_catalog_name_edit);
    let name = name_value.trim();
    if name.is_empty() {
        show_audio_description_error_and_focus(
            hwnd,
            state,
            &labels.character_catalog_name_error,
            state.character_catalog_name_edit,
        );
        return Ok(CharacterCatalogPreparation::Cancelled);
    }
    let save_folder = with_state(state.parent, |app| {
        app.settings.audio_description_save_folder.clone()
    })
    .unwrap_or_else(default_audio_description_save_folder);
    let path = audio_description_character_catalog_path(&save_folder, name);
    let existing_index = state
        .character_catalogs
        .iter()
        .position(|catalog| catalog.path == path || catalog.name.eq_ignore_ascii_case(name));
    let context = if let Some(index) = existing_index {
        let catalog = &state.character_catalogs[index];
        load_audio_description_character_catalog_context(
            catalog.name.clone(),
            catalog.path.clone(),
        )?
    } else {
        let catalog = AudioDescriptionCharacterCatalogSummary {
            name: name.to_string(),
            path: path.clone(),
        };
        state.character_catalogs.push(catalog);
        state
            .character_catalogs
            .sort_by_key(|catalog| catalog.name.to_lowercase());
        refill_character_catalog_combo(state, name);
        load_audio_description_character_catalog_context(name.to_string(), path)?
    };
    persist_audio_description_preferences(state);
    Ok(CharacterCatalogPreparation::Ready(context))
}

fn persist_audio_description_preferences(state: &WindowState) {
    let description_language = language_from_combo(state.language_combo);
    let engine = engine_from_combo(state.engine_combo);
    let voice = selected_voice_name(state);
    let verbosity =
        unsafe { SendMessageW(state.verbosity_combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
    let extended = checkbox_checked(state.extended_checkbox);
    let recognize_characters = checkbox_checked(state.recognize_characters_checkbox);
    let recognize_screen_text = checkbox_checked(state.recognize_screen_text_checkbox);
    let keep_character_catalog =
        recognize_characters && checkbox_checked(state.keep_character_catalog_checkbox);
    let character_catalog = selected_character_catalog_name(state);
    let save_project = checkbox_checked(state.save_project_checkbox);
    let create_video = checkbox_checked(state.create_video_checkbox);
    let delete_video_after = checkbox_checked(state.delete_video_after_checkbox);
    let use_sonarpad_ai = using_sonarpad_ai(state);
    // Keep the credentials for both AI access modes independently persisted.
    // Switching modes must never erase the inactive credential.
    let gemini_api_key = get_text(state.gemini_api_key_edit).trim().to_string();
    let gemini_model = get_text(state.gemini_model_combo).trim().to_string();
    let sonarpad_ai_access_code = get_text(state.sonarpad_code_edit).trim().to_string();
    if with_state(state.parent, |app| {
        app.settings.audio_description_language = Some(description_language);
        app.settings.audio_description_tts_engine = engine;
        if let Some(voice) = voice {
            app.settings.audio_description_tts_voice = voice;
        }
        app.settings.audio_description_tts_rate = Some(state.tts_rate);
        app.settings.audio_description_tts_volume = Some(state.tts_volume);
        app.settings.audio_description_verbosity = if verbosity >= 0 {
            (verbosity as u8).min(2)
        } else {
            2
        };
        app.settings.audio_description_extended_pauses = extended;
        app.settings.audio_description_recognize_characters = recognize_characters;
        app.settings.audio_description_recognize_screen_text = recognize_screen_text;
        app.settings.audio_description_keep_character_catalog = keep_character_catalog;
        app.settings.audio_description_character_catalog = character_catalog;
        app.settings.audio_description_save_project = save_project;
        app.settings.audio_description_create_video = create_video;
        app.settings.audio_description_delete_video_after = delete_video_after;
        app.settings.audio_description_use_sonarpad_ai = use_sonarpad_ai;
        app.settings.gemini_api_key = gemini_api_key;
        if !gemini_model.is_empty() {
            app.settings.audio_description_gemini_model = gemini_model;
        }
        app.settings.sonarpad_ai_access_code = sonarpad_ai_access_code;
        save_settings(app.settings.clone());
    })
    .is_none()
    {
        crate::log_debug("Audio description: app state unavailable while saving preferences");
    }
}

fn update_delete_video_visibility(state: &WindowState) {
    let save_project = checkbox_checked(state.save_project_checkbox);
    unsafe {
        ShowWindow(
            state.delete_video_after_checkbox,
            if save_project { SW_HIDE } else { SW_SHOW },
        );
    }
}

fn update_video_output_mode(state: &WindowState) {
    let create_video = checkbox_checked(state.create_video_checkbox);
    unsafe {
        // Extended pauses change the audio timeline. Keep the user's preference intact,
        // but disable the control and ignore it while fast video output is selected.
        EnableWindow(state.extended_checkbox, !create_video && !state.running);
    }
    let output_text = get_text(state.output);
    if !output_text.trim().is_empty() {
        let mut output = PathBuf::from(output_text.trim());
        let extension = output
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if extension.is_empty() || extension == "mp3" || extension == "mkv" || extension == "mp4" {
            output.set_extension(if create_video { "mp4" } else { "mp3" });
            set_path(state.output, &output);
        }
    }
}

fn language_from_combo(combo: HWND) -> Language {
    match unsafe { SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 } {
        1 => Language::English,
        2 => Language::German,
        3 => Language::Spanish,
        4 => Language::Portuguese,
        5 => Language::PortugueseBrazilian,
        6 => Language::Swedish,
        7 => Language::Vietnamese,
        8 => Language::Czech,
        9 => Language::Polish,
        10 => Language::French,
        11 => Language::Serbian,
        12 => Language::Ukrainian,
        13 => Language::Lithuanian,
        14 => Language::Russian,
        15 => Language::Chinese,
        16 => Language::Hindi,
        _ => Language::Italian,
    }
}

fn language_combo_index(language: Language) -> usize {
    match language {
        Language::Italian => 0,
        Language::English => 1,
        Language::German => 2,
        Language::Spanish => 3,
        Language::Portuguese => 4,
        Language::PortugueseBrazilian => 5,
        Language::Swedish => 6,
        Language::Vietnamese => 7,
        Language::Czech => 8,
        Language::Polish => 9,
        Language::French => 10,
        Language::Serbian => 11,
        Language::Ukrainian => 12,
        Language::Lithuanian => 13,
        Language::Russian => 14,
        Language::Chinese => 15,
        Language::Hindi => 16,
    }
}

fn selected_verbosity(combo: HWND) -> AudioDescriptionVerbosity {
    match unsafe { SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 } {
        0 => AudioDescriptionVerbosity::Brief,
        1 => AudioDescriptionVerbosity::Standard,
        _ => AudioDescriptionVerbosity::Detailed,
    }
}

fn load_voices(hwnd: HWND, engine: TtsEngine) {
    thread::spawn(move || {
        let result = match engine {
            TtsEngine::Edge => crate::app_windows::options_window::fetch_voice_list(),
            TtsEngine::Sapi5 => crate::sapi5_engine::list_sapi_voices(),
            TtsEngine::Sapi4 => Ok(crate::sapi4_engine::get_voices()),
            TtsEngine::Google => Ok(crate::google_tts::installed_voices()),
        };
        let payload = Box::new(result);
        post_boxed_message(hwnd, WM_AD_VOICES_LOADED, WPARAM(0), payload);
    });
}

fn audio_description_voice_sources(
    state: &WindowState,
) -> crate::app_windows::audio_description_voice_window::AudioDescriptionVoiceSources {
    let (mut edge, mut sapi5) = with_state(state.parent, |app| {
        (app.edge_voices.clone(), app.sapi_voices.clone())
    })
    .unwrap_or_default();
    let current_engine = engine_from_combo(state.engine_combo);
    if current_engine == TtsEngine::Edge && edge.is_empty() {
        edge = state.voices.clone();
    }
    if current_engine == TtsEngine::Sapi5 && sapi5.is_empty() {
        sapi5 = state.voices.clone();
    }
    crate::app_windows::audio_description_voice_window::AudioDescriptionVoiceSources {
        edge,
        sapi5,
        sapi4: crate::sapi4_engine::get_voices(),
        google: crate::google_tts::installed_voices(),
    }
}

fn open_audio_description_voice_settings(hwnd: HWND, state: &mut WindowState) {
    let sources = audio_description_voice_sources(state);
    let current_voice = selected_voice_name(state)
        .filter(|voice| !voice.trim().is_empty())
        .unwrap_or_else(|| state.preferred_voice.clone());
    let preview_pitch = with_state(state.parent, |app| app.settings.tts_pitch).unwrap_or(0);
    let default =
        crate::app_windows::audio_description_voice_window::AudioDescriptionVoiceSettings {
            engine: engine_from_combo(state.engine_combo),
            voice: current_voice,
            rate: state.tts_rate,
            volume: state.tts_volume,
        };
    let result = crate::app_windows::audio_description_voice_window::open_dialog(
        hwnd,
        state.parent,
        state.language,
        sources.clone(),
        default,
        preview_pitch,
    );
    if let Some(selected) = result {
        unsafe {
            SendMessageW(
                state.engine_combo,
                CB_SETCURSEL,
                WPARAM(engine_combo_index(selected.engine)),
                LPARAM(0),
            );
        }
        state.preferred_voice = selected.voice;
        state.tts_rate = selected.rate;
        state.tts_volume = selected.volume;
        refill_voice_combo(state, sources.voices_for(selected.engine));
        persist_audio_description_preferences(state);
        crate::log_debug(&format!(
            "Audio description: voice adjusted engine={:?} rate={} volume={}",
            selected.engine, selected.rate, selected.volume
        ));
    }
    unsafe {
        SetFocus(state.voice_settings_button);
    }
}

fn refill_voice_combo(state: &mut WindowState, voices: Vec<VoiceInfo>) {
    state.voices = voices;
    unsafe { SendMessageW(state.voice_combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0)) };
    for voice in &state.voices {
        let display = if voice.locale.trim().is_empty() {
            voice.short_name.clone()
        } else {
            format!("{} ({})", voice.short_name, voice.locale)
        };
        add_combo_item(state.voice_combo, &display);
    }
    let selected_language = language_from_combo(state.language_combo);
    let code = language_code(selected_language);
    let engine = engine_from_combo(state.engine_combo);
    let default_preferred = if engine == TtsEngine::Edge && selected_language == Language::Italian {
        "it-IT-IsabellaNeural"
    } else {
        ""
    };
    let index = state
        .voices
        .iter()
        .position(|voice| {
            !state.preferred_voice.is_empty()
                && voice
                    .short_name
                    .eq_ignore_ascii_case(&state.preferred_voice)
        })
        .or_else(|| {
            state.voices.iter().position(|voice| {
                !default_preferred.is_empty() && voice.short_name == default_preferred
            })
        })
        .or_else(|| {
            state.voices.iter().position(|voice| {
                voice
                    .locale
                    .to_ascii_lowercase()
                    .starts_with(&code.to_ascii_lowercase())
            })
        })
        .unwrap_or(0);
    unsafe {
        if !state.voices.is_empty() {
            SendMessageW(state.voice_combo, CB_SETCURSEL, WPARAM(index), LPARAM(0));
            state.preferred_voice = state.voices[index].short_name.clone();
        } else {
            state.preferred_voice.clear();
        }
        EnableWindow(state.voice_combo, !state.voices.is_empty());
    }
}

fn canonical_gemini_model_id(model: &str) -> String {
    model
        .trim()
        .strip_prefix("models/")
        .unwrap_or(model.trim())
        .to_ascii_lowercase()
}

fn refill_gemini_model_combo(combo: HWND, mut models: Vec<String>, selected: &str) {
    if !models
        .iter()
        .any(|model| model == DEFAULT_AUDIO_DESCRIPTION_GEMINI_MODEL)
    {
        models.push(DEFAULT_AUDIO_DESCRIPTION_GEMINI_MODEL.to_string());
    }
    if !selected.trim().is_empty() && !models.iter().any(|model| model == selected) {
        models.push(selected.to_string());
    }
    models.sort();
    models.dedup();
    unsafe {
        SendMessageW(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    }
    for model in &models {
        add_combo_item(combo, model);
    }
    let preferred = if selected.trim().is_empty() {
        DEFAULT_AUDIO_DESCRIPTION_GEMINI_MODEL
    } else {
        selected
    };
    let wide = to_wide(preferred);
    let index = unsafe {
        SendMessageW(
            combo,
            CB_FINDSTRINGEXACT,
            WPARAM(usize::MAX),
            LPARAM(wide.as_ptr() as isize),
        )
        .0
    };
    unsafe {
        SendMessageW(
            combo,
            CB_SETCURSEL,
            WPARAM(if index >= 0 { index as usize } else { 0 }),
            LPARAM(0),
        );
    }
}

fn persist_gemini_settings(parent: HWND, api_key: &str, model: &str) {
    let api_key = api_key.trim().to_string();
    let model = if model.trim().is_empty() {
        DEFAULT_AUDIO_DESCRIPTION_GEMINI_MODEL.to_string()
    } else {
        model.trim().to_string()
    };
    if with_state(parent, |app| {
        app.settings.gemini_api_key = api_key;
        app.settings.audio_description_gemini_model = model;
        save_settings(app.settings.clone());
    })
    .is_none()
    {
        crate::log_debug("Audio description: app state unavailable while saving Gemini settings");
    }
}

fn using_sonarpad_ai(state: &WindowState) -> bool {
    checkbox_checked(state.ai_sonarpad_radio)
}

fn update_ai_access_visibility(state: &WindowState) {
    let service = using_sonarpad_ai(state);
    unsafe {
        for control in [
            state.gemini_api_key_label,
            state.gemini_api_key_edit,
            state.gemini_show_api_key_checkbox,
            state.gemini_get_key_button,
            state.gemini_refresh_models_button,
        ] {
            ShowWindow(control, if service { SW_HIDE } else { SW_SHOW });
            EnableWindow(control, !service && !state.running);
        }
        for control in [
            state.sonarpad_code_label,
            state.sonarpad_code_edit,
            state.sonarpad_show_code_checkbox,
            state.sonarpad_balance_label,
            state.sonarpad_balance_edit,
            state.sonarpad_request_code_button,
        ] {
            ShowWindow(control, if service { SW_SHOW } else { SW_HIDE });
            EnableWindow(control, service && !state.running);
        }
        EnableWindow(state.gemini_model_combo, !service && !state.running);
    }
}

fn restore_audio_description_control_focus(hwnd: HWND, control: HWND) {
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        if control.0 != 0 {
            SetFocus(control);
        }
    }
}

fn show_audio_description_error_and_focus(
    hwnd: HWND,
    state: &WindowState,
    message: &str,
    control: HWND,
) {
    show_error(state.parent, state.language, message);
    restore_audio_description_control_focus(hwnd, control);
}

fn refresh_gemini_models(hwnd: HWND, state: &WindowState) {
    let labels = labels(state.language);
    let api_key = get_text(state.gemini_api_key_edit).trim().to_string();
    if api_key.is_empty() {
        show_audio_description_error_and_focus(
            hwnd,
            state,
            &labels.error_api_key,
            state.gemini_api_key_edit,
        );
        return;
    }
    let selected_model = get_text(state.gemini_model_combo).trim().to_string();
    persist_gemini_settings(state.parent, &api_key, &selected_model);
    set_text(
        state.gemini_refresh_models_button,
        &labels.gemini_loading_models,
    );
    unsafe {
        EnableWindow(state.gemini_refresh_models_button, false);
    }
    thread::spawn(move || {
        let result = crate::app_windows::options_window::fetch_gemini_models_for_key(&api_key);
        let payload = Box::new(result);
        post_boxed_message(hwnd, WM_AD_MODELS_LOADED, WPARAM(0), payload);
    });
}

fn set_controls_enabled(state: &WindowState, enabled: bool) {
    unsafe {
        for control in &state.setup_controls {
            EnableWindow(*control, enabled);
            ShowWindow(*control, if enabled { SW_SHOW } else { SW_HIDE });
        }
        if enabled && state.resume_mode {
            for control in &state.setup_controls {
                ShowWindow(*control, SW_HIDE);
            }
            for control in [
                state.gemini_model_label,
                state.gemini_model_combo,
                state.start_button,
                state.close_button,
            ] {
                EnableWindow(control, true);
                ShowWindow(control, SW_SHOW);
            }
        }
        ShowWindow(
            state.progress,
            if enabled && state.resume_mode {
                SW_HIDE
            } else {
                SW_SHOW
            },
        );
        ShowWindow(state.status, SW_SHOW);
        ShowWindow(
            state.cancel_button,
            if enabled && state.resume_mode {
                SW_HIDE
            } else {
                SW_SHOW
            },
        );
        EnableWindow(state.cancel_button, !enabled);
        if enabled {
            if state.resume_mode {
                SetFocus(state.gemini_model_combo);
            } else {
                update_character_catalog_visibility(state);
                update_delete_video_visibility(state);
                update_video_output_mode(state);
                update_ai_access_visibility(state);
                SetFocus(state.start_button);
            }
        } else {
            SetFocus(state.cancel_button);
        }
    }
}

fn audio_description_track_label(
    ordinal: usize,
    track: &crate::ffmpeg_source::AudioStreamInfo,
) -> String {
    let mut names = Vec::new();
    if let Some(title) = track
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        names.push(title.to_string());
    }
    if let Some(language) = track
        .language
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && !names
            .iter()
            .any(|value| value.eq_ignore_ascii_case(language))
    {
        names.push(language.to_string());
    }
    if names.is_empty() {
        names.push(track.codec.clone());
    }
    format!(
        "{}. {} ({}; {} ch)",
        ordinal + 1,
        names.join(" - "),
        track.codec,
        track.channels.max(1)
    )
}

fn choose_audio_description_track(
    hwnd: HWND,
    state: &WindowState,
    input: &Path,
) -> Result<Option<Option<i32>>, String> {
    let tracks = crate::ffmpeg_source::list_audio_streams(input)
        .map_err(|error| format!("Audio description: FFmpeg stream inspection failed: {error}"))?;
    if tracks.len() <= 1 {
        return Ok(Some(None));
    }

    let default_selection = tracks
        .iter()
        .position(|track| track.is_default)
        .unwrap_or(0);
    let options = tracks
        .iter()
        .enumerate()
        .map(|(index, track)| audio_description_track_label(index, track))
        .collect::<Vec<_>>();
    crate::log_debug(&format!(
        "Audio description: multiple audio tracks detected count={} default_selection={}",
        tracks.len(),
        default_selection
    ));
    let selected = crate::app_windows::youtube_transcript_window::choose_combo_option_dialog(
        hwnd,
        state.language,
        i18n::tr(state.language, "playback.audio_track"),
        i18n::tr(state.language, "playback.audio_track"),
        options,
        default_selection,
    );
    let Some(selected) = selected else {
        crate::log_debug(
            "Audio description: audio track selection cancelled; closing creation window",
        );
        return Ok(None);
    };
    let track = tracks
        .get(selected)
        .ok_or_else(|| "Audio description: selected audio track no longer exists".to_string())?;
    crate::log_debug(&format!(
        "Audio description: selected audio track stream_index={} title={:?} language={:?} codec={} channels={} default={}",
        track.index, track.title, track.language, track.codec, track.channels, track.is_default
    ));
    Ok(Some(Some(track.index)))
}

fn start_job(hwnd: HWND, state: &mut WindowState) {
    let labels = labels(state.language);
    let use_sonarpad_ai = using_sonarpad_ai(state);
    let gemini_api_key = get_text(state.gemini_api_key_edit).trim().to_string();
    let sonarpad_ai_access_code = get_text(state.sonarpad_code_edit).trim().to_string();
    if use_sonarpad_ai {
        if sonarpad_ai_access_code.is_empty() {
            show_audio_description_error_and_focus(
                hwnd,
                state,
                &labels.error_sonarpad_code,
                state.sonarpad_code_edit,
            );
            return;
        }
    } else if gemini_api_key.is_empty() {
        show_audio_description_error_and_focus(
            hwnd,
            state,
            &labels.error_api_key,
            state.gemini_api_key_edit,
        );
        return;
    }
    let Some(ai_settings) = with_state(state.parent, |app| app.settings.clone()) else {
        return;
    };
    let service_url = if use_sonarpad_ai {
        SONARPAD_AI_SERVICE_URL.to_string()
    } else {
        String::new()
    };
    let service_code = if use_sonarpad_ai {
        sonarpad_ai_access_code.clone()
    } else {
        String::new()
    };
    let service_device_id = if use_sonarpad_ai {
        ai_settings.sonarpad_ai_device_id.clone()
    } else {
        String::new()
    };

    let job = if let Some(checkpoint_path) = state.resume_checkpoint_path.as_ref() {
        match audio_description_job_from_checkpoint(
            checkpoint_path,
            if use_sonarpad_ai {
                String::new()
            } else {
                gemini_api_key.clone()
            },
            service_url.clone(),
            service_code.clone(),
            service_device_id.clone(),
        ) {
            Ok(mut job) => {
                let selected_model = if use_sonarpad_ai {
                    "gemini-3.8-flash".to_string()
                } else {
                    get_text(state.gemini_model_combo).trim().to_string()
                };
                if selected_model.is_empty() {
                    show_audio_description_error_and_focus(
                        hwnd,
                        state,
                        &labels.error_model,
                        state.gemini_model_combo,
                    );
                    return;
                }
                let Some(voice) =
                    selected_voice_name(state).filter(|voice| !voice.trim().is_empty())
                else {
                    show_audio_description_error_and_focus(
                        hwnd,
                        state,
                        &labels.error_voice,
                        state.voice_settings_button,
                    );
                    return;
                };
                let engine = engine_from_combo(state.engine_combo);
                crate::log_debug(&format!(
                    "Audio description: resume voice selection checkpoint_engine={:?} checkpoint_voice={:?} selected_engine={:?} selected_voice={:?} rate={} pitch={} volume={}",
                    job.tts_engine,
                    job.tts_voice,
                    engine,
                    voice,
                    state.tts_rate,
                    ai_settings.tts_pitch,
                    state.tts_volume
                ));
                // The checkpoint supplies completed analysis; the visible controls
                // supply the voice used for this export, just as for a new job.
                job.tts_engine = engine;
                job.tts_voice = voice;
                job.tts_rate = state.tts_rate;
                job.tts_pitch = ai_settings.tts_pitch;
                job.tts_volume = state.tts_volume;
                job.gemini_model = selected_model;
                job.recognize_screen_text = checkbox_checked(state.recognize_screen_text_checkbox);
                if !use_sonarpad_ai {
                    persist_gemini_settings(state.parent, &gemini_api_key, &job.gemini_model);
                }
                persist_audio_description_preferences(state);
                (job, None)
            }
            Err(error) => {
                let message = labels.resume_invalid.replace("{error}", &error);
                show_audio_description_error_and_focus(
                    hwnd,
                    state,
                    &message,
                    state.continue_interrupted_button,
                );
                return;
            }
        }
    } else {
        let input = PathBuf::from(get_text(state.input).trim());
        if !input.is_file() {
            show_audio_description_error_and_focus(hwnd, state, &labels.error_input, state.input);
            return;
        }
        let create_video_output = checkbox_checked(state.create_video_checkbox);
        let mut output = PathBuf::from(get_text(state.output).trim());
        if output.as_os_str().is_empty() {
            output = default_output(state.parent, &input, create_video_output);
        }
        if create_video_output {
            let extension = output
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if extension != "mp4" && extension != "mkv" {
                output.set_extension("mp4");
            }
        } else {
            output.set_extension("mp3");
        }
        set_path(state.output, &output);
        if output.as_os_str().is_empty() {
            show_audio_description_error_and_focus(hwnd, state, &labels.error_output, state.output);
            return;
        }
        if input == output {
            show_audio_description_error_and_focus(
                hwnd,
                state,
                &labels.error_same_path,
                state.output,
            );
            return;
        }
        let audio_stream_index = match choose_audio_description_track(hwnd, state, &input) {
            Ok(Some(index)) => index,
            Ok(None) => {
                unsafe {
                    crate::log_if_err!(PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)));
                }
                return;
            }
            Err(error) => {
                show_audio_description_error_and_focus(hwnd, state, &error, state.start_button);
                return;
            }
        };
        crate::app_windows::audio_description_resume_window::remember_project_folder(
            state.parent,
            &output,
        );
        let voice_index =
            unsafe { SendMessageW(state.voice_combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
        let Some(voice) = (voice_index >= 0)
            .then(|| state.voices.get(voice_index as usize))
            .flatten()
            .map(|voice| voice.short_name.clone())
        else {
            show_audio_description_error_and_focus(
                hwnd,
                state,
                &labels.error_voice,
                state.voice_settings_button,
            );
            return;
        };
        let Some(settings) = with_state(state.parent, |app| app.settings.clone()) else {
            return;
        };
        let gemini_model = if use_sonarpad_ai {
            "gemini-3.8-flash".to_string()
        } else {
            get_text(state.gemini_model_combo).trim().to_string()
        };
        if gemini_model.is_empty() {
            show_audio_description_error_and_focus(
                hwnd,
                state,
                &labels.error_model,
                state.gemini_model_combo,
            );
            return;
        }
        if !use_sonarpad_ai {
            persist_gemini_settings(state.parent, &gemini_api_key, &gemini_model);
        }

        let description_language = language_from_combo(state.language_combo);
        let extended = checkbox_checked(state.extended_checkbox) && !create_video_output;
        let recognize_characters = checkbox_checked(state.recognize_characters_checkbox);
        let character_catalog = match prepare_character_catalog(hwnd, state, &labels) {
            Ok(CharacterCatalogPreparation::Disabled) => None,
            Ok(CharacterCatalogPreparation::Cancelled) => return,
            Ok(CharacterCatalogPreparation::Ready(catalog)) => Some(catalog),
            Err(error) => {
                show_audio_description_error_and_focus(hwnd, state, &error, state.start_button);
                return;
            }
        };
        let save_project = checkbox_checked(state.save_project_checkbox);
        let input_to_trash = should_move_input_to_recycle_bin(
            checkbox_checked(state.delete_video_after_checkbox),
            save_project,
        )
        .then(|| input.clone());
        persist_audio_description_preferences(state);
        let job = AudioDescriptionJob {
            input_path: input,
            output_path: output,
            audio_stream_index,
            language_code: language_code(description_language).to_string(),
            tts_language: description_language,
            verbosity: selected_verbosity(state.verbosity_combo),
            allow_extended_pauses: extended,
            recognize_characters,
            recognize_screen_text: checkbox_checked(state.recognize_screen_text_checkbox),
            character_catalog,
            save_project,
            create_video_output,
            tts_engine: engine_from_combo(state.engine_combo),
            tts_voice: voice,
            tts_rate: state.tts_rate,
            tts_pitch: settings.tts_pitch,
            tts_volume: state.tts_volume,
            dictionary: settings.dictionary.clone(),
            gemini_api_key: if use_sonarpad_ai {
                String::new()
            } else {
                gemini_api_key
            },
            sonarpad_ai_service_url: service_url,
            sonarpad_ai_access_code: service_code,
            sonarpad_ai_device_id: service_device_id,
            gemini_model,
            audiobook_bitrate_kbps: settings.audiobook_m4b_bitrate,
            resume_checkpoint_path: None,
        };
        (job, input_to_trash)
    };
    let (job, input_to_trash) = job;
    state.retry_job = Some(job.clone());
    state.retry_input_to_trash = input_to_trash.clone();
    state.short_silence_retry_used = false;
    launch_audio_description_job(hwnd, state, job, input_to_trash);
}

fn launch_audio_description_dialogue_overlap_fallback(
    hwnd: HWND,
    state: &mut WindowState,
    job: AudioDescriptionJob,
    input_to_trash: Option<PathBuf>,
) {
    let cancel = Arc::new(AtomicBool::new(false));
    state.cancel = Some(cancel.clone());
    state.exhausted_gemini_models.clear();
    state.running = true;
    set_controls_enabled(state, false);
    set_text(
        state.status,
        &i18n::tr(
            state.language,
            "audio_description.dialogue_overlap_retrying",
        ),
    );
    unsafe { SendMessageW(state.progress, PBM_SETPOS, WPARAM(0), LPARAM(0)) };

    thread::spawn(move || {
        let status_hwnd = hwnd;
        let progress_hwnd = hwnd;
        let result = create_audio_description_dialogue_overlap_fallback(
            &job,
            cancel,
            AudioDescriptionCallbacks {
                status: Some(Box::new(move |stage, message| {
                    let payload = Box::new((stage.to_string(), message.to_string()));
                    post_boxed_message(status_hwnd, WM_AD_STATUS, WPARAM(0), payload);
                })),
                progress: Some(Box::new(move |pct| unsafe {
                    crate::log_if_err!(
                        PostMessageW(
                            progress_hwnd,
                            WM_AD_PROGRESS,
                            WPARAM(pct as usize),
                            LPARAM(0),
                        ),
                        "Audio description overlap fallback: PostMessageW failed"
                    );
                })),
                quota: None,
                overload: None,
            },
        );
        let payload = Box::new(AudioDescriptionDonePayload {
            result,
            input_to_trash,
        });
        post_boxed_message(hwnd, WM_AD_DONE, WPARAM(0), payload);
    });
}

fn launch_audio_description_job(
    hwnd: HWND,
    state: &mut WindowState,
    job: AudioDescriptionJob,
    input_to_trash: Option<PathBuf>,
) {
    let labels = labels(state.language);
    let cancel = Arc::new(AtomicBool::new(false));
    state.cancel = Some(cancel.clone());
    state.exhausted_gemini_models.clear();
    state.running = true;
    set_controls_enabled(state, false);
    set_text(state.status, &labels.running);
    unsafe { SendMessageW(state.progress, PBM_SETPOS, WPARAM(0), LPARAM(0)) };

    thread::spawn(move || {
        let status_hwnd = hwnd;
        let progress_hwnd = hwnd;
        let quota_cancel = cancel.clone();
        let overload_cancel = cancel.clone();
        let result = create_audio_description(
            &job,
            cancel,
            AudioDescriptionCallbacks {
                status: Some(Box::new(move |stage, message| {
                    let payload = Box::new((stage.to_string(), message.to_string()));
                    post_boxed_message(status_hwnd, WM_AD_STATUS, WPARAM(0), payload);
                })),
                progress: Some(Box::new(move |pct| unsafe {
                    crate::log_if_err!(
                        PostMessageW(
                            progress_hwnd,
                            WM_AD_PROGRESS,
                            WPARAM(pct as usize),
                            LPARAM(0),
                        ),
                        "Audio description: PostMessageW failed"
                    );
                })),
                quota: Some(Box::new(move |model, error| {
                    let (response, receiver) = mpsc::sync_channel(1);
                    let payload = Box::new(QuotaPromptRequest {
                        model: model.to_string(),
                        error: error.to_string(),
                        response,
                    });
                    let raw_payload = Box::into_raw(payload);
                    let posted = unsafe {
                        PostMessageW(hwnd, WM_AD_QUOTA, WPARAM(0), LPARAM(raw_payload as isize))
                    };
                    if posted.is_err() {
                        unsafe {
                            let _released_payload = Box::from_raw(raw_payload);
                        }
                        return AudioDescriptionQuotaDecision::Stop;
                    }
                    loop {
                        match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                            Ok(decision) => return decision,
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                if quota_cancel.load(Ordering::Relaxed) {
                                    return AudioDescriptionQuotaDecision::Stop;
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
                                return AudioDescriptionQuotaDecision::Stop;
                            }
                        }
                    }
                })),
                overload: Some(Box::new(move |model, error| {
                    let (response, receiver) = mpsc::sync_channel(1);
                    let payload = Box::new(OverloadPromptRequest {
                        model: model.to_string(),
                        error: error.to_string(),
                        response,
                    });
                    let raw_payload = Box::into_raw(payload);
                    let posted = unsafe {
                        PostMessageW(
                            hwnd,
                            WM_AD_OVERLOAD,
                            WPARAM(0),
                            LPARAM(raw_payload as isize),
                        )
                    };
                    if posted.is_err() {
                        unsafe {
                            let _released_payload = Box::from_raw(raw_payload);
                        }
                        return AudioDescriptionOverloadDecision::Stop;
                    }
                    loop {
                        match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                            Ok(decision) => return decision,
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                if overload_cancel.load(Ordering::Relaxed) {
                                    return AudioDescriptionOverloadDecision::Stop;
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
                                return AudioDescriptionOverloadDecision::Stop;
                            }
                        }
                    }
                })),
            },
        );
        let payload = Box::new(AudioDescriptionDonePayload {
            result,
            input_to_trash,
        });
        post_boxed_message(hwnd, WM_AD_DONE, WPARAM(0), payload);
    });
}

fn window_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let create = &*(lparam.0 as *const CREATESTRUCTW);
                let parent = create.hwndParent;
                let (
                    language,
                    hfont,
                    gemini_api_key,
                    gemini_model,
                    use_sonarpad_ai,
                    sonarpad_ai_access_code,
                    description_language,
                    tts_engine,
                    tts_voice,
                    tts_rate,
                    tts_volume,
                    verbosity,
                    extended_pauses,
                    recognize_characters,
                    recognize_screen_text,
                    keep_character_catalog,
                    selected_character_catalog,
                    audio_description_save_folder,
                    save_project,
                    create_video,
                    delete_video_after,
                ) = with_state(parent, |state| {
                    (
                        state.settings.language,
                        state.hfont,
                        state.settings.gemini_api_key.clone(),
                        state.settings.audio_description_gemini_model.clone(),
                        state.settings.audio_description_use_sonarpad_ai,
                        state.settings.sonarpad_ai_access_code.clone(),
                        state
                            .settings
                            .audio_description_language
                            .unwrap_or(state.settings.language),
                        state.settings.audio_description_tts_engine,
                        state.settings.audio_description_tts_voice.clone(),
                        state
                            .settings
                            .audio_description_tts_rate
                            .unwrap_or(state.settings.tts_rate),
                        state
                            .settings
                            .audio_description_tts_volume
                            .unwrap_or(state.settings.tts_volume),
                        state.settings.audio_description_verbosity,
                        state.settings.audio_description_extended_pauses,
                        state.settings.audio_description_recognize_characters,
                        state.settings.audio_description_recognize_screen_text,
                        state.settings.audio_description_keep_character_catalog,
                        state.settings.audio_description_character_catalog.clone(),
                        state.settings.audio_description_save_folder.clone(),
                        state.settings.audio_description_save_project,
                        state.settings.audio_description_create_video,
                        state.settings.audio_description_delete_video_after,
                    )
                })
                .unwrap_or((
                    Language::default(),
                    HFONT(0),
                    String::new(),
                    DEFAULT_AUDIO_DESCRIPTION_GEMINI_MODEL.to_string(),
                    false,
                    String::new(),
                    Language::default(),
                    TtsEngine::Edge,
                    String::new(),
                    0,
                    100,
                    2,
                    false,
                    true,
                    false,
                    false,
                    String::new(),
                    default_audio_description_save_folder(),
                    false,
                    false,
                    false,
                ));
                let labels = labels(language);
                let character_catalogs =
                    list_audio_description_character_catalogs(&audio_description_save_folder);

                let input_label = create_label(hwnd, &labels.input, 16, 16, 150, hfont);
                let input = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    16,
                    36,
                    520,
                    24,
                    hwnd,
                    HMENU(ID_INPUT as isize),
                    HINSTANCE(0),
                    None,
                );
                let input_browse = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.input_browse).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    544,
                    36,
                    105,
                    24,
                    hwnd,
                    HMENU(ID_INPUT_BROWSE as isize),
                    HINSTANCE(0),
                    None,
                );
                let output_label = create_label(hwnd, &labels.output, 16, 68, 150, hfont);
                let output = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    16,
                    88,
                    520,
                    24,
                    hwnd,
                    HMENU(ID_OUTPUT as isize),
                    HINSTANCE(0),
                    None,
                );
                let output_browse = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.output_browse).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    544,
                    88,
                    105,
                    24,
                    hwnd,
                    HMENU(ID_OUTPUT_BROWSE as isize),
                    HINSTANCE(0),
                    None,
                );

                let language_label = create_label(hwnd, &labels.language, 16, 126, 180, hfont);
                let language_combo = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    16,
                    146,
                    200,
                    260,
                    hwnd,
                    HMENU(ID_LANGUAGE as isize),
                    HINSTANCE(0),
                    None,
                );
                for key in [
                    "it", "en", "de", "es", "pt", "pt-BR", "sv", "vi", "cs", "pl", "fr", "sr",
                    "uk", "lt", "ru", "zh", "hi",
                ] {
                    let translated = i18n::tr(language, &format!("voice.lang.{key}"));
                    add_combo_item(language_combo, &translated);
                }
                SendMessageW(
                    language_combo,
                    CB_SETCURSEL,
                    WPARAM(language_combo_index(description_language)),
                    LPARAM(0),
                );

                let verbosity_label = create_label(hwnd, &labels.verbosity, 236, 126, 190, hfont);
                let verbosity_combo = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    236,
                    146,
                    200,
                    150,
                    hwnd,
                    HMENU(ID_VERBOSITY as isize),
                    HINSTANCE(0),
                    None,
                );
                add_combo_item(verbosity_combo, &labels.verbosity_brief);
                add_combo_item(verbosity_combo, &labels.verbosity_standard);
                add_combo_item(verbosity_combo, &labels.verbosity_detailed);
                SendMessageW(
                    verbosity_combo,
                    CB_SETCURSEL,
                    WPARAM(usize::from(verbosity.min(2))),
                    LPARAM(0),
                );

                let extended_checkbox = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.extended).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    16,
                    184,
                    620,
                    24,
                    hwnd,
                    HMENU(ID_EXTENDED as isize),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(
                    extended_checkbox,
                    BM_SETCHECK,
                    WPARAM(if extended_pauses {
                        BST_CHECKED.0 as usize
                    } else {
                        0
                    }),
                    LPARAM(0),
                );

                let recognize_characters_checkbox = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.recognize_characters).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    16,
                    212,
                    650,
                    24,
                    hwnd,
                    HMENU(ID_RECOGNIZE_CHARACTERS as isize),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(
                    recognize_characters_checkbox,
                    BM_SETCHECK,
                    WPARAM(if recognize_characters {
                        BST_CHECKED.0 as usize
                    } else {
                        0
                    }),
                    LPARAM(0),
                );

                let keep_catalog_style = WS_CHILD
                    | WS_TABSTOP
                    | WINDOW_STYLE(BS_AUTOCHECKBOX as u32)
                    | if recognize_characters {
                        WS_VISIBLE
                    } else {
                        WINDOW_STYLE(0)
                    };
                let keep_character_catalog_checkbox = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.keep_character_catalog).as_ptr()),
                    keep_catalog_style,
                    16,
                    240,
                    650,
                    24,
                    hwnd,
                    HMENU(ID_KEEP_CHARACTER_CATALOG as isize),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(
                    keep_character_catalog_checkbox,
                    BM_SETCHECK,
                    WPARAM(if recognize_characters && keep_character_catalog {
                        BST_CHECKED.0 as usize
                    } else {
                        0
                    }),
                    LPARAM(0),
                );

                let catalog_controls_visible = recognize_characters && keep_character_catalog;
                let character_catalog_label = create_label(
                    hwnd,
                    &labels.character_catalog_selection_label,
                    16,
                    268,
                    190,
                    hfont,
                );
                ShowWindow(
                    character_catalog_label,
                    if catalog_controls_visible {
                        SW_SHOW
                    } else {
                        SW_HIDE
                    },
                );
                let catalog_combo_style = WS_CHILD
                    | WS_TABSTOP
                    | WINDOW_STYLE(CBS_DROPDOWNLIST as u32)
                    | if catalog_controls_visible {
                        WS_VISIBLE
                    } else {
                        WINDOW_STYLE(0)
                    };
                let character_catalog_combo = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    catalog_combo_style,
                    210,
                    264,
                    439,
                    220,
                    hwnd,
                    HMENU(ID_CHARACTER_CATALOG as isize),
                    HINSTANCE(0),
                    None,
                );
                add_combo_item(character_catalog_combo, &labels.character_catalog_choose);
                for catalog in &character_catalogs {
                    add_combo_item(character_catalog_combo, &catalog.name);
                }
                let selected_catalog_index = character_catalogs
                    .iter()
                    .position(|catalog| {
                        catalog
                            .name
                            .eq_ignore_ascii_case(&selected_character_catalog)
                    })
                    .map(|index| index + 1)
                    .unwrap_or(0);
                SendMessageW(
                    character_catalog_combo,
                    CB_SETCURSEL,
                    WPARAM(selected_catalog_index),
                    LPARAM(0),
                );
                let character_catalog_name_label = create_label(
                    hwnd,
                    &labels.character_catalog_new_name_label,
                    16,
                    300,
                    190,
                    hfont,
                );
                let character_catalog_name_edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    210,
                    296,
                    439,
                    24,
                    hwnd,
                    HMENU(ID_CHARACTER_CATALOG_NAME as isize),
                    HINSTANCE(0),
                    None,
                );
                let show_new_catalog_name = catalog_controls_visible && selected_catalog_index == 0;
                ShowWindow(
                    character_catalog_name_label,
                    if show_new_catalog_name {
                        SW_SHOW
                    } else {
                        SW_HIDE
                    },
                );
                ShowWindow(
                    character_catalog_name_edit,
                    if show_new_catalog_name {
                        SW_SHOW
                    } else {
                        SW_HIDE
                    },
                );

                let save_project_checkbox = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.save_project).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    16,
                    332,
                    324,
                    24,
                    hwnd,
                    HMENU(ID_SAVE_PROJECT as isize),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(
                    save_project_checkbox,
                    BM_SETCHECK,
                    WPARAM(if save_project {
                        BST_CHECKED.0 as usize
                    } else {
                        0
                    }),
                    LPARAM(0),
                );

                let create_video_checkbox = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.create_video).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    350,
                    332,
                    316,
                    24,
                    hwnd,
                    HMENU(ID_CREATE_VIDEO as isize),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(
                    create_video_checkbox,
                    BM_SETCHECK,
                    WPARAM(if create_video {
                        BST_CHECKED.0 as usize
                    } else {
                        0
                    }),
                    LPARAM(0),
                );
                if create_video {
                    EnableWindow(extended_checkbox, false);
                }

                let recognize_screen_text_checkbox = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.recognize_screen_text).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    16,
                    362,
                    650,
                    24,
                    hwnd,
                    HMENU(ID_RECOGNIZE_SCREEN_TEXT as isize),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(
                    recognize_screen_text_checkbox,
                    BM_SETCHECK,
                    WPARAM(if recognize_screen_text {
                        BST_CHECKED.0 as usize
                    } else {
                        0
                    }),
                    LPARAM(0),
                );

                let delete_video_after_checkbox = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.delete_video_after).as_ptr()),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    16,
                    390,
                    650,
                    24,
                    hwnd,
                    HMENU(ID_DELETE_VIDEO_AFTER as isize),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(
                    delete_video_after_checkbox,
                    BM_SETCHECK,
                    WPARAM(if delete_video_after {
                        BST_CHECKED.0 as usize
                    } else {
                        0
                    }),
                    LPARAM(0),
                );
                ShowWindow(
                    delete_video_after_checkbox,
                    if save_project { SW_HIDE } else { SW_SHOW },
                );

                let ai_access_label = create_label(hwnd, &labels.ai_access, 16, 424, 120, hfont);
                let ai_personal_radio = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.ai_personal).as_ptr()),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WS_GROUP
                        | WINDOW_STYLE(BS_AUTORADIOBUTTON as u32),
                    136,
                    418,
                    230,
                    26,
                    hwnd,
                    HMENU(ID_AI_PERSONAL as isize),
                    HINSTANCE(0),
                    None,
                );
                let ai_sonarpad_radio = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.ai_sonarpad).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTORADIOBUTTON as u32),
                    370,
                    418,
                    290,
                    26,
                    hwnd,
                    HMENU(ID_AI_SONARPAD as isize),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(
                    ai_personal_radio,
                    BM_SETCHECK,
                    WPARAM(if use_sonarpad_ai {
                        0
                    } else {
                        BST_CHECKED.0 as usize
                    }),
                    LPARAM(0),
                );
                SendMessageW(
                    ai_sonarpad_radio,
                    BM_SETCHECK,
                    WPARAM(if use_sonarpad_ai {
                        BST_CHECKED.0 as usize
                    } else {
                        0
                    }),
                    LPARAM(0),
                );

                let gemini_api_key_label =
                    create_label(hwnd, &labels.gemini_api_key, 16, 452, 210, hfont);
                let gemini_api_key_edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR(to_wide(&gemini_api_key).as_ptr()),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WINDOW_STYLE(ES_AUTOHSCROLL as u32 | ES_PASSWORD as u32),
                    16,
                    472,
                    420,
                    24,
                    hwnd,
                    HMENU(ID_GEMINI_API_KEY as isize),
                    HINSTANCE(0),
                    None,
                );
                let gemini_show_api_key_checkbox = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.gemini_show_api_key).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    446,
                    470,
                    220,
                    24,
                    hwnd,
                    HMENU(ID_GEMINI_SHOW_API_KEY as isize),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(
                    gemini_show_api_key_checkbox,
                    BM_SETCHECK,
                    WPARAM(0),
                    LPARAM(0),
                );
                let gemini_get_key_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.gemini_get_key).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    446,
                    498,
                    220,
                    28,
                    hwnd,
                    HMENU(ID_GEMINI_GET_KEY as isize),
                    HINSTANCE(0),
                    None,
                );

                let sonarpad_code_label =
                    create_label(hwnd, &labels.sonarpad_code, 16, 452, 210, hfont);
                let sonarpad_code_edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR(to_wide(&sonarpad_ai_access_code).as_ptr()),
                    WS_CHILD
                        | WS_TABSTOP
                        | WINDOW_STYLE(ES_AUTOHSCROLL as u32 | ES_PASSWORD as u32),
                    16,
                    472,
                    420,
                    24,
                    hwnd,
                    HMENU(ID_SONARPAD_CODE as isize),
                    HINSTANCE(0),
                    None,
                );
                let sonarpad_show_code_checkbox = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.sonarpad_show_code).as_ptr()),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    446,
                    470,
                    220,
                    24,
                    hwnd,
                    HMENU(ID_SONARPAD_SHOW_CODE as isize),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(
                    sonarpad_show_code_checkbox,
                    BM_SETCHECK,
                    WPARAM(0),
                    LPARAM(0),
                );
                let sonarpad_balance_label =
                    create_label(hwnd, &labels.sonarpad_balance, 16, 504, 120, hfont);
                let sonarpad_balance_edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR(to_wide(&labels.sonarpad_balance_unavailable).as_ptr()),
                    WS_CHILD
                        | WS_TABSTOP
                        | WINDOW_STYLE(ES_AUTOHSCROLL as u32 | ES_READONLY as u32),
                    136,
                    500,
                    180,
                    24,
                    hwnd,
                    HMENU(ID_SONARPAD_BALANCE as isize),
                    HINSTANCE(0),
                    None,
                );
                let sonarpad_request_code_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.sonarpad_request_code).as_ptr()),
                    WS_CHILD | WS_TABSTOP,
                    446,
                    498,
                    220,
                    28,
                    hwnd,
                    HMENU(ID_SONARPAD_REQUEST_CODE as isize),
                    HINSTANCE(0),
                    None,
                );

                let gemini_model_label =
                    create_label(hwnd, &labels.gemini_model, 16, 534, 210, hfont);
                let gemini_model_combo = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    16,
                    554,
                    420,
                    220,
                    hwnd,
                    HMENU(ID_GEMINI_MODEL as isize),
                    HINSTANCE(0),
                    None,
                );
                refill_gemini_model_combo(
                    gemini_model_combo,
                    vec![DEFAULT_AUDIO_DESCRIPTION_GEMINI_MODEL.to_string()],
                    &gemini_model,
                );
                let gemini_refresh_models_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.gemini_refresh_models).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    446,
                    552,
                    203,
                    28,
                    hwnd,
                    HMENU(ID_GEMINI_REFRESH_MODELS as isize),
                    HINSTANCE(0),
                    None,
                );

                // Engine and voice are now configured exclusively from the "Regola voce"
                // dialog. Keep these controls hidden as the internal source of truth so the
                // existing generation/persistence path remains unchanged and regression-free.
                let engine_label = create_label(hwnd, &labels.engine, 16, 590, 200, hfont);
                ShowWindow(engine_label, SW_HIDE);
                let engine_combo = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    16,
                    610,
                    200,
                    150,
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
                    WPARAM(engine_combo_index(tts_engine)),
                    LPARAM(0),
                );

                let voice_label = create_label(hwnd, &labels.voice, 236, 590, 200, hfont);
                ShowWindow(voice_label, SW_HIDE);
                let voice_combo = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    236,
                    610,
                    413,
                    220,
                    hwnd,
                    HMENU(ID_VOICE as isize),
                    HINSTANCE(0),
                    None,
                );
                EnableWindow(voice_combo, false);

                let progress = CreateWindowExW(
                    Default::default(),
                    PROGRESS_CLASSW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    654,
                    650,
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
                    684,
                    650,
                    48,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let voice_settings_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.voice_settings).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    16,
                    758,
                    110,
                    30,
                    hwnd,
                    HMENU(ID_VOICE_SETTINGS as isize),
                    HINSTANCE(0),
                    None,
                );
                let modify_project_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.modify_project).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    136,
                    758,
                    180,
                    30,
                    hwnd,
                    HMENU(ID_MODIFY_PROJECT as isize),
                    HINSTANCE(0),
                    None,
                );
                let continue_interrupted_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.resume_title).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    326,
                    758,
                    180,
                    30,
                    hwnd,
                    HMENU(ID_CONTINUE_INTERRUPTED as isize),
                    HINSTANCE(0),
                    None,
                );
                let start_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.start).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    516,
                    758,
                    150,
                    30,
                    hwnd,
                    HMENU(ID_START as isize),
                    HINSTANCE(0),
                    None,
                );
                let cancel_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.cancel).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    390,
                    798,
                    100,
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
                    510,
                    798,
                    100,
                    30,
                    hwnd,
                    HMENU(ID_CLOSE as isize),
                    HINSTANCE(0),
                    None,
                );

                for control in [
                    input_label,
                    output_label,
                    language_label,
                    verbosity_label,
                    ai_access_label,
                    ai_personal_radio,
                    ai_sonarpad_radio,
                    sonarpad_code_label,
                    sonarpad_code_edit,
                    sonarpad_show_code_checkbox,
                    sonarpad_balance_label,
                    sonarpad_balance_edit,
                    sonarpad_request_code_button,
                    gemini_api_key_label,
                    gemini_model_label,
                    engine_label,
                    voice_label,
                    input,
                    input_browse,
                    output,
                    output_browse,
                    language_combo,
                    verbosity_combo,
                    extended_checkbox,
                    recognize_characters_checkbox,
                    recognize_screen_text_checkbox,
                    keep_character_catalog_checkbox,
                    character_catalog_label,
                    character_catalog_combo,
                    character_catalog_name_label,
                    character_catalog_name_edit,
                    save_project_checkbox,
                    create_video_checkbox,
                    delete_video_after_checkbox,
                    gemini_api_key_edit,
                    gemini_show_api_key_checkbox,
                    gemini_get_key_button,
                    gemini_model_combo,
                    gemini_refresh_models_button,
                    engine_combo,
                    voice_combo,
                    status,
                    voice_settings_button,
                    start_button,
                    continue_interrupted_button,
                    cancel_button,
                    close_button,
                    modify_project_button,
                    continue_interrupted_button,
                ] {
                    if hfont.0 != 0 {
                        SendMessageW(control, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                }

                let state = Box::new(WindowState {
                    parent,
                    language,
                    input,
                    output,
                    language_combo,
                    verbosity_combo,
                    extended_checkbox,
                    recognize_characters_checkbox,
                    recognize_screen_text_checkbox,
                    keep_character_catalog_checkbox,
                    character_catalog_label,
                    character_catalog_combo,
                    character_catalog_name_label,
                    character_catalog_name_edit,
                    character_catalogs,
                    save_project_checkbox,
                    create_video_checkbox,
                    delete_video_after_checkbox,
                    ai_personal_radio,
                    ai_sonarpad_radio,
                    sonarpad_code_label,
                    sonarpad_code_edit,
                    sonarpad_show_code_checkbox,
                    sonarpad_balance_label,
                    sonarpad_balance_edit,
                    sonarpad_request_code_button,
                    gemini_api_key_label,
                    gemini_api_key_edit,
                    gemini_show_api_key_checkbox,
                    gemini_get_key_button,
                    gemini_model_label,
                    gemini_model_combo,
                    gemini_refresh_models_button,
                    engine_combo,
                    voice_combo,
                    voice_settings_button,
                    tts_rate,
                    tts_volume,
                    progress,
                    status,
                    start_button,
                    continue_interrupted_button,
                    cancel_button,
                    close_button,
                    setup_controls: vec![
                        input_label,
                        output_label,
                        language_label,
                        verbosity_label,
                        gemini_api_key_label,
                        gemini_model_label,
                        engine_label,
                        voice_label,
                        input,
                        input_browse,
                        output,
                        output_browse,
                        language_combo,
                        verbosity_combo,
                        extended_checkbox,
                        recognize_characters_checkbox,
                        recognize_screen_text_checkbox,
                        keep_character_catalog_checkbox,
                        character_catalog_label,
                        character_catalog_combo,
                        character_catalog_name_label,
                        character_catalog_name_edit,
                        save_project_checkbox,
                        create_video_checkbox,
                        delete_video_after_checkbox,
                        ai_access_label,
                        ai_personal_radio,
                        ai_sonarpad_radio,
                        sonarpad_code_label,
                        sonarpad_code_edit,
                        sonarpad_show_code_checkbox,
                        sonarpad_balance_label,
                        sonarpad_balance_edit,
                        sonarpad_request_code_button,
                        gemini_api_key_label,
                        gemini_api_key_edit,
                        gemini_show_api_key_checkbox,
                        gemini_get_key_button,
                        gemini_model_combo,
                        gemini_refresh_models_button,
                        engine_combo,
                        voice_combo,
                        modify_project_button,
                        continue_interrupted_button,
                        voice_settings_button,
                        start_button,
                        close_button,
                    ],
                    voices: Vec::new(),
                    preferred_voice: tts_voice,
                    running: false,
                    cancel: None,
                    exhausted_gemini_models: Vec::new(),
                    return_to_editor_after_player: false,
                    source_player_path: crate::current_playback_media_path(parent),
                    resume_checkpoint_path: None,
                    resume_mode: false,
                    retry_job: None,
                    retry_input_to_trash: None,
                    short_silence_retry_used: false,
                });
                let state_pointer = Box::into_raw(state);
                SetWindowLongPtrW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                    state_pointer as isize,
                );
                update_character_catalog_visibility(&*state_pointer);
                update_ai_access_visibility(&*state_pointer);
                update_sonarpad_code_visibility(&*state_pointer);
                if using_sonarpad_ai(&*state_pointer) {
                    refresh_sonarpad_balance(hwnd, &*state_pointer);
                }
                set_text(status, &labels.loading_voices);
                load_voices(hwnd, tts_engine);
                if !using_sonarpad_ai(&*state_pointer)
                    && !get_text((*state_pointer).gemini_api_key_edit)
                        .trim()
                        .is_empty()
                {
                    crate::log_debug(
                        "Audio description: refreshing full Gemini model list on window open",
                    );
                    refresh_gemini_models(hwnd, &*state_pointer);
                }
                SetFocus(input);
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
                let labels = labels(state.language);
                match id {
                    ID_INPUT_BROWSE if !state.running => {
                        if let Some(path) = open_input_dialog(hwnd, state.language, &labels) {
                            set_path(state.input, &path);
                            set_path(
                                state.output,
                                &default_output(
                                    state.parent,
                                    &path,
                                    checkbox_checked(state.create_video_checkbox),
                                ),
                            );
                            ensure_new_character_catalog_name(state);
                            SetForegroundWindow(hwnd);
                        }
                    }
                    ID_OUTPUT_BROWSE if !state.running => {
                        let initial_text = get_text(state.output);
                        let initial = if initial_text.trim().is_empty() {
                            let input = PathBuf::from(get_text(state.input));
                            default_output(
                                state.parent,
                                &input,
                                checkbox_checked(state.create_video_checkbox),
                            )
                        } else {
                            PathBuf::from(initial_text)
                        };
                        if let Some(path) = open_output_dialog(
                            hwnd,
                            state.language,
                            &labels,
                            &initial,
                            checkbox_checked(state.create_video_checkbox),
                        ) {
                            set_path(state.output, &path);
                            SetForegroundWindow(hwnd);
                        }
                    }
                    ID_CREATE_VIDEO if !state.running => {
                        update_video_output_mode(state);
                        persist_audio_description_preferences(state);
                    }
                    ID_AI_PERSONAL | ID_AI_SONARPAD if !state.running => {
                        let use_service = id == ID_AI_SONARPAD;
                        SendMessageW(
                            state.ai_personal_radio,
                            BM_SETCHECK,
                            WPARAM((!use_service) as usize),
                            LPARAM(0),
                        );
                        SendMessageW(
                            state.ai_sonarpad_radio,
                            BM_SETCHECK,
                            WPARAM(use_service as usize),
                            LPARAM(0),
                        );
                        update_ai_access_visibility(state);
                        persist_audio_description_preferences(state);
                        if use_service {
                            refresh_sonarpad_balance(hwnd, state);
                        }
                    }
                    ID_SONARPAD_SHOW_CODE if !state.running => {
                        update_sonarpad_code_visibility(state);
                    }
                    ID_GEMINI_API_KEY if notification == EN_KILLFOCUS && !state.running => {
                        persist_audio_description_preferences(state);
                    }
                    ID_SONARPAD_CODE if notification == EN_KILLFOCUS && !state.running => {
                        persist_audio_description_preferences(state);
                        refresh_sonarpad_balance(hwnd, state);
                    }
                    ID_SONARPAD_REQUEST_CODE if !state.running => {
                        if let Err(err) = crate::audio_utils::open_url_in_browser(
                            "https://sonarpad.com/contact.php",
                        ) {
                            eprintln!(
                                "Audio description: unable to open Sonarpad AI request page: {err}"
                            );
                        }
                    }
                    ID_GEMINI_SHOW_API_KEY if !state.running => {
                        update_gemini_api_key_visibility(state);
                    }
                    ID_GEMINI_GET_KEY if !state.running => {
                        crate::app_windows::options_window::open_gemini_api_key_page();
                    }
                    ID_GEMINI_REFRESH_MODELS if !state.running && !using_sonarpad_ai(state) => {
                        refresh_gemini_models(hwnd, state);
                    }
                    ID_VOICE_SETTINGS if !state.running => {
                        open_audio_description_voice_settings(hwnd, state);
                    }
                    ID_ENGINE if notification == CBN_SELCHANGE => {
                        state.preferred_voice.clear();
                        state.voices.clear();
                        SendMessageW(state.voice_combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
                        EnableWindow(state.voice_combo, false);
                        persist_audio_description_preferences(state);
                        set_text(state.status, &labels.loading_voices);
                        load_voices(hwnd, engine_from_combo(state.engine_combo));
                    }
                    ID_LANGUAGE if notification == CBN_SELCHANGE => {
                        state.preferred_voice.clear();
                        if !state.voices.is_empty() {
                            let voices = state.voices.clone();
                            refill_voice_combo(state, voices);
                        }
                        persist_audio_description_preferences(state);
                    }
                    ID_VERBOSITY if notification == CBN_SELCHANGE => {
                        persist_audio_description_preferences(state);
                    }
                    ID_VOICE if notification == CBN_SELCHANGE => {
                        if let Some(voice) = selected_voice_name(state) {
                            state.preferred_voice = voice;
                        }
                        persist_audio_description_preferences(state);
                    }
                    ID_RECOGNIZE_CHARACTERS if !state.running => {
                        update_character_catalog_visibility(state);
                        persist_audio_description_preferences(state);
                    }
                    ID_KEEP_CHARACTER_CATALOG if !state.running => {
                        update_character_catalog_visibility(state);
                        persist_audio_description_preferences(state);
                    }
                    ID_CHARACTER_CATALOG if notification == CBN_SELCHANGE && !state.running => {
                        update_character_catalog_visibility(state);
                        persist_audio_description_preferences(state);
                    }
                    ID_EXTENDED | ID_SAVE_PROJECT if !state.running => {
                        if id == ID_SAVE_PROJECT {
                            update_delete_video_visibility(state);
                        }
                        persist_audio_description_preferences(state);
                    }
                    ID_DELETE_VIDEO_AFTER if !state.running => {
                        persist_audio_description_preferences(state);
                    }
                    ID_RECOGNIZE_SCREEN_TEXT if !state.running => {
                        persist_audio_description_preferences(state);
                    }
                    ID_MODIFY_PROJECT if !state.running => {
                        crate::app_windows::audio_description_project_window::open_from_dialog(
                            state.parent,
                            hwnd,
                        );
                    }
                    ID_CONTINUE_INTERRUPTED if !state.running => {
                        continue_interrupted_from_window(hwnd, state);
                    }
                    ID_START => start_job(hwnd, state),
                    ID_CANCEL => {
                        if let Some(cancel) = state.cancel.as_ref() {
                            crate::log_debug(
                                "Audio description: cancellation requested from dialog",
                            );
                            cancel.store(true, Ordering::SeqCst);
                            set_text(state.status, &labels.canceling);
                            EnableWindow(state.cancel_button, false);
                        }
                    }
                    ID_CLOSE if !state.running => {
                        crate::log_if_err!(
                            DestroyWindow(hwnd),
                            "Audio description: DestroyWindow failed"
                        );
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_AD_MODELS_LOADED => {
                let payload = lparam.0 as *mut Result<Vec<String>, String>;
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
                let labels = labels(state.language);
                EnableWindow(state.gemini_refresh_models_button, !state.running);
                set_text(
                    state.gemini_refresh_models_button,
                    &labels.gemini_refresh_models,
                );
                match result {
                    Ok(models) => {
                        let selected = get_text(state.gemini_model_combo);
                        refill_gemini_model_combo(state.gemini_model_combo, models, &selected);
                        let model = get_text(state.gemini_model_combo);
                        let api_key = get_text(state.gemini_api_key_edit);
                        persist_gemini_settings(state.parent, &api_key, &model);
                    }
                    Err(error) => {
                        show_audio_description_error_and_focus(
                            hwnd,
                            state,
                            &labels.gemini_error_models.replace("{error}", &error),
                            state.gemini_refresh_models_button,
                        );
                    }
                }
                LRESULT(0)
            }
            WM_AD_VOICES_LOADED => {
                let payload = lparam.0 as *mut Result<Vec<VoiceInfo>, String>;
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
                match result {
                    Ok(voices) => {
                        refill_voice_combo(state, voices);
                        persist_audio_description_preferences(state);
                        set_text(state.status, &labels(state.language).ready);
                    }
                    Err(error) => {
                        state.voices.clear();
                        set_text(state.status, &error);
                    }
                }
                LRESULT(0)
            }
            WM_AD_PROGRESS => {
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() {
                    let state = &mut *pointer;
                    SendMessageW(
                        state.progress,
                        PBM_SETPOS,
                        WPARAM(wparam.0.min(100)),
                        LPARAM(0),
                    );
                }
                LRESULT(0)
            }
            WM_AD_STATUS => {
                let payload = lparam.0 as *mut (String, String);
                if payload.is_null() {
                    return LRESULT(0);
                }
                let (stage, message) = *Box::from_raw(payload);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() {
                    let localized = localized_status_text((*pointer).language, &stage, &message);
                    set_text((*pointer).status, &localized);
                }
                LRESULT(0)
            }
            WM_AD_OVERLOAD => {
                let payload = lparam.0 as *mut OverloadPromptRequest;
                if payload.is_null() {
                    return LRESULT(0);
                }
                let request = *Box::from_raw(payload);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if pointer.is_null() {
                    crate::log_if_err!(
                        request
                            .response
                            .send(AudioDescriptionOverloadDecision::Stop),
                        "Audio description: overload response channel closed"
                    );
                    return LRESULT(0);
                }
                let state = &mut *pointer;
                let labels = labels(state.language);
                let message = labels
                    .overload_message
                    .replace("{model}", &request.model)
                    .replace("{error}", &request.error);
                crate::watchdog::enter_modal_dialog();
                let choice = MessageBoxW(
                    hwnd,
                    PCWSTR(to_wide(&message).as_ptr()),
                    PCWSTR(to_wide(&labels.overload_title).as_ptr()),
                    MB_YESNO | MB_ICONQUESTION,
                );
                crate::watchdog::exit_modal_dialog();
                let decision = if choice == IDYES {
                    AudioDescriptionOverloadDecision::Wait
                } else {
                    AudioDescriptionOverloadDecision::Stop
                };
                crate::log_debug(&format!(
                    "Audio description: Gemini overload user decision={}",
                    if matches!(decision, AudioDescriptionOverloadDecision::Wait) {
                        "wait_forever"
                    } else {
                        "stop"
                    }
                ));
                crate::log_if_err!(
                    request.response.send(decision),
                    "Audio description: overload response channel closed"
                );
                LRESULT(0)
            }
            WM_AD_QUOTA => {
                let payload = lparam.0 as *mut QuotaPromptRequest;
                if payload.is_null() {
                    return LRESULT(0);
                }
                let request = *Box::from_raw(payload);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if pointer.is_null() {
                    crate::log_if_err!(
                        request.response.send(AudioDescriptionQuotaDecision::Stop),
                        "Audio description: quota response channel closed"
                    );
                    return LRESULT(0);
                }
                let state = &mut *pointer;
                let labels = labels(state.language);
                let exhausted_model = canonical_gemini_model_id(&request.model);
                if !state
                    .exhausted_gemini_models
                    .iter()
                    .any(|model| model == &exhausted_model)
                {
                    state.exhausted_gemini_models.push(exhausted_model);
                }
                let message = labels
                    .quota_message
                    .replace("{model}", &request.model)
                    .replace("{error}", &request.error);
                crate::watchdog::enter_modal_dialog();
                let choice = MessageBoxW(
                    hwnd,
                    PCWSTR(to_wide(&message).as_ptr()),
                    PCWSTR(to_wide(&labels.quota_title).as_ptr()),
                    MB_YESNOCANCEL | MB_ICONQUESTION,
                );
                let decision = if choice == IDYES {
                    let current_api_key = get_text(state.gemini_api_key_edit);
                    let mut available_models = combo_items(state.gemini_model_combo);
                    if !current_api_key.trim().is_empty() {
                        match crate::app_windows::options_window::fetch_gemini_models_for_key(
                            &current_api_key,
                        ) {
                            Ok(models) => {
                                crate::log_debug(&format!(
                                    "Audio description quota model selector refreshed {} compatible Gemini model(s) without mutating the main model combo",
                                    models.len()
                                ));
                                available_models = models;
                            }
                            Err(error) => crate::log_debug(&format!(
                                "Audio description quota model selector could not refresh Gemini models; using cached list: {error}"
                            )),
                        }
                    }
                    available_models.retain(|model| {
                        if model.trim().is_empty() {
                            return false;
                        }
                        let candidate = canonical_gemini_model_id(model);
                        !state.exhausted_gemini_models.contains(&candidate)
                    });
                    available_models.sort_by_key(|model| canonical_gemini_model_id(model));
                    available_models.dedup_by(|left, right| {
                        canonical_gemini_model_id(left) == canonical_gemini_model_id(right)
                    });

                    if available_models.is_empty() {
                        show_info(hwnd, state.language, &labels.quota_no_alternative_models);
                        AudioDescriptionQuotaDecision::Stop
                    } else {
                        let suggested =
                            suggested_alternative_model(state.gemini_model_combo, &request.model);
                        let suggested = available_models
                            .iter()
                            .find(|model| model.eq_ignore_ascii_case(&suggested))
                            .cloned()
                            .unwrap_or_else(|| available_models[0].clone());
                        match crate::app_windows::prompt_window::prompt_user_choice(
                            hwnd,
                            &labels.quota_title,
                            &labels.quota_model_prompt,
                            &available_models,
                            &suggested,
                            state.language,
                        ) {
                            Some(model) if !model.trim().is_empty() => {
                                let model = model.trim().to_string();
                                let main_models = combo_items(state.gemini_model_combo);
                                refill_gemini_model_combo(
                                    state.gemini_model_combo,
                                    main_models,
                                    &model,
                                );
                                persist_gemini_settings(state.parent, &current_api_key, &model);
                                AudioDescriptionQuotaDecision::SwitchModel(model)
                            }
                            _ => AudioDescriptionQuotaDecision::Wait,
                        }
                    }
                } else if choice == IDNO {
                    AudioDescriptionQuotaDecision::Wait
                } else {
                    AudioDescriptionQuotaDecision::Stop
                };
                crate::watchdog::exit_modal_dialog();
                crate::log_if_err!(
                    request.response.send(decision),
                    "Audio description: quota response channel closed"
                );
                LRESULT(0)
            }
            WM_AD_SONARPAD_BALANCE => {
                let payload = lparam.0 as *mut SonarpadBalancePayload;
                if payload.is_null() {
                    return LRESULT(0);
                }
                let SonarpadBalancePayload {
                    access_code,
                    result,
                } = *Box::from_raw(payload);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if pointer.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *pointer;
                if !using_sonarpad_ai(state)
                    || get_text(state.sonarpad_code_edit).trim() != access_code
                {
                    return LRESULT(0);
                }
                match result {
                    Ok(balance) => {
                        set_text(
                            state.sonarpad_balance_edit,
                            &format_sonarpad_balance(state.language, balance),
                        );
                    }
                    Err(err) => {
                        crate::log_debug(&format!(
                            "Audio description: Sonarpad AI balance unavailable: {err}"
                        ));
                        set_text(
                            state.sonarpad_balance_edit,
                            &labels(state.language).sonarpad_balance_unavailable,
                        );
                    }
                }
                LRESULT(0)
            }
            WM_AD_DONE => {
                let payload = lparam.0 as *mut AudioDescriptionDonePayload;
                if payload.is_null() {
                    return LRESULT(0);
                }
                let AudioDescriptionDonePayload {
                    result,
                    input_to_trash,
                } = *Box::from_raw(payload);
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
                if using_sonarpad_ai(state) {
                    refresh_sonarpad_balance(hwnd, state);
                }
                let labels = labels(state.language);
                match result {
                    Ok(outcome) => {
                        SendMessageW(state.progress, PBM_SETPOS, WPARAM(100), LPARAM(0));
                        set_text(state.status, &labels.complete);
                        let mut message = i18n::tr_f(
                            state.language,
                            "audio_description.success_details",
                            &[
                                ("path", &outcome.output_path.to_string_lossy()),
                                ("count", &outcome.generated_descriptions.to_string()),
                                ("normal", &outcome.normal_descriptions.to_string()),
                                ("pauses", &outcome.extended_pauses.to_string()),
                                ("dropped", &outcome.dropped_after_tts.to_string()),
                            ],
                        );
                        if let Some(project_path) = outcome.project_path.as_ref() {
                            message.push_str("\r\n\r\n");
                            message.push_str(&i18n::tr_f(
                                state.language,
                                "audio_description.project_saved",
                                &[("path", &project_path.to_string_lossy())],
                            ));
                        }
                        if let Some(warning) = outcome.project_warning.as_ref() {
                            message.push_str("\r\n\r\n");
                            message.push_str(&i18n::tr_f(
                                state.language,
                                "audio_description.project_warning",
                                &[("error", warning)],
                            ));
                        }
                        if let Some(catalog_path) = outcome.character_catalog_path.as_ref() {
                            message.push_str("\r\n\r\n");
                            message.push_str(
                                &labels
                                    .character_catalog_saved
                                    .replace("{path}", &catalog_path.to_string_lossy()),
                            );
                        }
                        if let Some(warning) = outcome.character_catalog_warning.as_ref() {
                            message.push_str("\r\n\r\n");
                            message.push_str(
                                &labels.character_catalog_warning.replace("{error}", warning),
                            );
                        }
                        if let Some(path) = input_to_trash.as_ref() {
                            message.push_str("\r\n\r\n");
                            match move_input_video_to_recycle_bin(hwnd, path) {
                                Ok(()) => {
                                    crate::log_debug(&format!(
                                        "Audio description: moved source video to Recycle Bin: {}",
                                        path.display()
                                    ));
                                    message.push_str(&i18n::tr_f(
                                        state.language,
                                        "audio_description.input_trashed",
                                        &[("path", &path.to_string_lossy())],
                                    ));
                                }
                                Err(error) => {
                                    crate::log_debug(&format!(
                                        "Audio description: failed to move source video to Recycle Bin: {}: {error}",
                                        path.display()
                                    ));
                                    message.push_str(&i18n::tr_f(
                                        state.language,
                                        "audio_description.input_trash_failed",
                                        &[("path", &path.to_string_lossy()), ("error", &error)],
                                    ));
                                }
                            }
                        }
                        crate::show_info_owned_by(hwnd, state.parent, state.language, &message);
                        crate::recover_main_window_after_audio_description(
                            state.parent,
                            "completion_message_closed",
                        );
                        state.resume_checkpoint_path = None;
                        state.resume_mode = false;
                        state.retry_job = None;
                        state.retry_input_to_trash = None;
                        state.short_silence_retry_used = false;
                        set_text(state.gemini_model_label, &labels.gemini_model);
                        set_text(state.start_button, &labels.start);
                        set_text(hwnd, &labels.title);
                        set_controls_enabled(state, true);
                        open_result_in_player(hwnd, state.parent, outcome.output_path.clone());
                    }
                    Err(error) if error == "cancelled" => {
                        crate::log_debug(
                            "Audio description: cancellation completed; worker stopped",
                        );
                        set_text(state.status, &labels.ready);
                    }
                    Err(error) if is_audio_description_no_usable_descriptions_error(&error) => {
                        let no_result =
                            i18n::tr(state.language, "audio_description.no_silence_result");
                        if state.short_silence_retry_used {
                            let prompt = i18n::tr(
                                state.language,
                                "audio_description.dialogue_overlap_retry_prompt",
                            );
                            let prompt_title = i18n::tr(
                                state.language,
                                "audio_description.dialogue_overlap_retry_title",
                            );
                            crate::watchdog::enter_modal_dialog();
                            let choice = MessageBoxW(
                                hwnd,
                                PCWSTR(to_wide(&prompt).as_ptr()),
                                PCWSTR(to_wide(&prompt_title).as_ptr()),
                                MB_YESNO | MB_ICONQUESTION,
                            );
                            crate::watchdog::exit_modal_dialog();
                            if choice == IDYES {
                                if let Some(mut retry_job) = state.retry_job.clone() {
                                    retry_job.verbosity = AudioDescriptionVerbosity::Brief;
                                    retry_job.resume_checkpoint_path = None;
                                    crate::log_debug(
                                        "Audio description: brief-description retry also produced no safely placeable descriptions; user accepted isolated final dialogue-overlap fallback",
                                    );
                                    let retry_input_to_trash = state.retry_input_to_trash.clone();
                                    launch_audio_description_dialogue_overlap_fallback(
                                        hwnd,
                                        state,
                                        retry_job,
                                        retry_input_to_trash,
                                    );
                                } else {
                                    set_text(state.status, &no_result);
                                    show_audio_description_error_and_focus(
                                        hwnd,
                                        state,
                                        &no_result,
                                        state.start_button,
                                    );
                                }
                            } else {
                                crate::log_debug(
                                    "Audio description: isolated final dialogue-overlap fallback declined by user",
                                );
                                set_text(state.status, &labels.ready);
                                SetFocus(state.start_button);
                            }
                        } else if let Some(retry_job) = state
                            .retry_job
                            .clone()
                            .filter(|job| job.verbosity == AudioDescriptionVerbosity::Brief)
                        {
                            if audio_description_checkpoint_has_generated_descriptions(
                                &retry_job.output_path,
                            ) {
                                crate::log_debug(
                                    "Audio description: initial analysis already used Brief verbosity; skipping redundant Brief retry and offering isolated dialogue-overlap fallback",
                                );
                                let prompt = i18n::tr(
                                    state.language,
                                    "audio_description.dialogue_overlap_direct_prompt",
                                );
                                let prompt_title = i18n::tr(
                                    state.language,
                                    "audio_description.dialogue_overlap_retry_title",
                                );
                                crate::watchdog::enter_modal_dialog();
                                let choice = MessageBoxW(
                                    hwnd,
                                    PCWSTR(to_wide(&prompt).as_ptr()),
                                    PCWSTR(to_wide(&prompt_title).as_ptr()),
                                    MB_YESNO | MB_ICONQUESTION,
                                );
                                crate::watchdog::exit_modal_dialog();
                                if choice == IDYES {
                                    crate::log_debug(
                                        "Audio description: user accepted direct isolated dialogue-overlap fallback after an initial Brief analysis",
                                    );
                                    let retry_input_to_trash = state.retry_input_to_trash.clone();
                                    launch_audio_description_dialogue_overlap_fallback(
                                        hwnd,
                                        state,
                                        retry_job,
                                        retry_input_to_trash,
                                    );
                                } else {
                                    crate::log_debug(
                                        "Audio description: direct isolated dialogue-overlap fallback declined after an initial Brief analysis",
                                    );
                                    set_text(state.status, &labels.ready);
                                    SetFocus(state.start_button);
                                }
                            } else {
                                crate::log_debug(
                                    "Audio description: initial Brief analysis produced no reusable generated descriptions; skipping redundant retry and final overlap prompt",
                                );
                                set_text(state.status, &no_result);
                                show_audio_description_error_and_focus(
                                    hwnd,
                                    state,
                                    &no_result,
                                    state.start_button,
                                );
                            }
                        } else {
                            let prompt = i18n::tr(
                                state.language,
                                "audio_description.short_silence_retry_prompt",
                            );
                            let prompt_title = i18n::tr(
                                state.language,
                                "audio_description.short_silence_retry_title",
                            );
                            crate::watchdog::enter_modal_dialog();
                            let choice = MessageBoxW(
                                hwnd,
                                PCWSTR(to_wide(&prompt).as_ptr()),
                                PCWSTR(to_wide(&prompt_title).as_ptr()),
                                MB_YESNO | MB_ICONQUESTION,
                            );
                            crate::watchdog::exit_modal_dialog();
                            if choice == IDYES {
                                if let Some(mut retry_job) = state.retry_job.clone() {
                                    retry_job.verbosity = AudioDescriptionVerbosity::Brief;
                                    retry_job.resume_checkpoint_path = None;
                                    state.short_silence_retry_used = true;
                                    crate::log_debug(
                                        "Audio description: no usable descriptions remained after the normal analysis; user accepted isolated brief-description retry",
                                    );
                                    set_text(
                                        state.status,
                                        &i18n::tr(
                                            state.language,
                                            "audio_description.short_silence_retrying",
                                        ),
                                    );
                                    let retry_input_to_trash = state.retry_input_to_trash.clone();
                                    launch_audio_description_job(
                                        hwnd,
                                        state,
                                        retry_job,
                                        retry_input_to_trash,
                                    );
                                } else {
                                    set_text(state.status, &no_result);
                                    show_audio_description_error_and_focus(
                                        hwnd,
                                        state,
                                        &no_result,
                                        state.start_button,
                                    );
                                }
                            } else {
                                crate::log_debug(
                                    "Audio description: isolated brief-description retry declined by user",
                                );
                                set_text(state.status, &labels.ready);
                                SetFocus(state.start_button);
                            }
                        }
                    }
                    Err(error) if error == AUDIO_DESCRIPTION_OVERLAP_NO_RAW_ERROR => {
                        let no_result = i18n::tr(
                            state.language,
                            "audio_description.dialogue_overlap_unavailable",
                        );
                        set_text(state.status, &no_result);
                        show_audio_description_error_and_focus(
                            hwnd,
                            state,
                            &no_result,
                            state.start_button,
                        );
                    }
                    Err(error) => {
                        set_text(state.status, &error);
                        show_audio_description_error_and_focus(
                            hwnd,
                            state,
                            &error,
                            state.start_button,
                        );
                    }
                }
                LRESULT(0)
            }
            WM_AD_PLAYER_RETURN => {
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() {
                    (*pointer).return_to_editor_after_player = true;
                    SetFocus((*pointer).close_button);
                }
                LRESULT(0)
            }
            WM_AD_RESET_NEW => {
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() && !(*pointer).running {
                    let state = &mut *pointer;
                    clear_player_return_for_window(hwnd);
                    state.return_to_editor_after_player = false;
                    state.source_player_path = None;
                    state.resume_checkpoint_path = None;
                    state.resume_mode = false;
                    state.cancel = None;
                    state.retry_job = None;
                    state.retry_input_to_trash = None;
                    state.short_silence_retry_used = false;
                    state.exhausted_gemini_models.clear();

                    let current_labels = labels(state.language);
                    set_text(state.input, "");
                    set_text(state.output, "");
                    set_text(state.character_catalog_name_edit, "");
                    set_text(state.gemini_model_label, &current_labels.gemini_model);
                    set_text(state.start_button, &current_labels.start);
                    set_text(hwnd, &current_labels.title);
                    set_text(state.status, &current_labels.ready);
                    SendMessageW(state.progress, PBM_SETPOS, WPARAM(0), LPARAM(0));
                    set_controls_enabled(state, true);
                    update_character_catalog_visibility(state);
                    update_delete_video_visibility(state);
                    update_video_output_mode(state);
                    SetFocus(state.input);
                    crate::log_debug(
                        "Audio description: reset creation window for a fresh empty job",
                    );
                }
                LRESULT(0)
            }
            WM_AD_SET_RESUME => {
                let payload = lparam.0 as *mut AudioDescriptionResumeSettings;
                if payload.is_null() {
                    return LRESULT(0);
                }
                let resume = *Box::from_raw(payload);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() && !(*pointer).running {
                    let state = &mut *pointer;
                    state.resume_checkpoint_path = Some(resume.checkpoint_path);
                    state.resume_mode = true;
                    set_path(state.input, &resume.input_path);
                    set_path(state.output, &resume.output_path);
                    SendMessageW(
                        state.language_combo,
                        CB_SETCURSEL,
                        WPARAM(language_combo_index(resume.description_language)),
                        LPARAM(0),
                    );
                    let verbosity_index = match resume.verbosity {
                        AudioDescriptionVerbosity::Brief => 0,
                        AudioDescriptionVerbosity::Standard => 1,
                        AudioDescriptionVerbosity::Detailed => 2,
                    };
                    SendMessageW(
                        state.verbosity_combo,
                        CB_SETCURSEL,
                        WPARAM(verbosity_index),
                        LPARAM(0),
                    );
                    SendMessageW(
                        state.extended_checkbox,
                        BM_SETCHECK,
                        WPARAM(if resume.allow_extended_pauses {
                            BST_CHECKED.0 as usize
                        } else {
                            0
                        }),
                        LPARAM(0),
                    );
                    SendMessageW(
                        state.recognize_characters_checkbox,
                        BM_SETCHECK,
                        WPARAM(if resume.recognize_characters {
                            BST_CHECKED.0 as usize
                        } else {
                            0
                        }),
                        LPARAM(0),
                    );
                    SendMessageW(
                        state.recognize_screen_text_checkbox,
                        BM_SETCHECK,
                        WPARAM(if resume.recognize_screen_text {
                            BST_CHECKED.0 as usize
                        } else {
                            0
                        }),
                        LPARAM(0),
                    );
                    SendMessageW(
                        state.save_project_checkbox,
                        BM_SETCHECK,
                        WPARAM(if resume.save_project {
                            BST_CHECKED.0 as usize
                        } else {
                            0
                        }),
                        LPARAM(0),
                    );
                    SendMessageW(
                        state.create_video_checkbox,
                        BM_SETCHECK,
                        WPARAM(if resume.create_video_output {
                            BST_CHECKED.0 as usize
                        } else {
                            0
                        }),
                        LPARAM(0),
                    );
                    update_delete_video_visibility(state);
                    update_video_output_mode(state);
                    SendMessageW(
                        state.engine_combo,
                        CB_SETCURSEL,
                        WPARAM(engine_combo_index(resume.tts_engine)),
                        LPARAM(0),
                    );
                    state.preferred_voice = resume.tts_voice;
                    refill_gemini_model_combo(
                        state.gemini_model_combo,
                        combo_items(state.gemini_model_combo),
                        &resume.gemini_model,
                    );
                    let current_labels = labels(state.language);
                    set_text(state.gemini_model_label, &current_labels.resume_model);
                    set_text(state.start_button, &current_labels.resume_start);
                    set_text(hwnd, &current_labels.resume_title);
                    set_text(
                        state.status,
                        &format!(
                            "{} {}/{}",
                            current_labels.resume_start,
                            resume.completed_chunks,
                            resume.total_chunks
                        ),
                    );
                    set_controls_enabled(state, true);
                }
                LRESULT(0)
            }
            WM_AD_SET_INPUT => {
                let payload = lparam.0 as *mut PathBuf;
                if payload.is_null() {
                    return LRESULT(0);
                }
                let input_path = *Box::from_raw(payload);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() && !(*pointer).running {
                    let state = &mut *pointer;
                    clear_player_return_for_window(hwnd);
                    state.return_to_editor_after_player = false;
                    state.source_player_path = Some(input_path.clone());
                    state.resume_checkpoint_path = None;
                    state.resume_mode = false;
                    let current_labels = labels(state.language);
                    set_text(state.gemini_model_label, &current_labels.gemini_model);
                    set_text(state.start_button, &current_labels.start);
                    set_text(hwnd, &current_labels.title);
                    set_controls_enabled(state, true);
                    set_path((*pointer).input, &input_path);
                    set_path(
                        state.output,
                        &default_output(
                            (*pointer).parent,
                            &input_path,
                            checkbox_checked((*pointer).create_video_checkbox),
                        ),
                    );
                    ensure_new_character_catalog_name(state);
                    SetFocus(state.input);
                }
                LRESULT(0)
            }
            WM_AD_RESTORE_RUNNING_FOCUS => {
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() && (*pointer).running {
                    let state = &*pointer;
                    ShowWindow(hwnd, SW_SHOW);
                    SetForegroundWindow(hwnd);
                    SetFocus(state.cancel_button);
                    crate::log_debug(
                        "Audio description: restored running resume window after selector close",
                    );
                }
                LRESULT(0)
            }
            WM_SETFOCUS => {
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() {
                    let state = &*pointer;
                    if state.running {
                        SetFocus(state.cancel_button);
                    } else if state.return_to_editor_after_player {
                        SetFocus(state.close_button);
                    } else if state.resume_mode {
                        SetFocus(state.gemini_model_combo);
                    } else {
                        SetFocus(state.input);
                    }
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 as u32 == VK_ESCAPE.0 as u32 {
                    crate::log_if_err!(
                        PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)),
                        "Audio description: PostMessageW failed"
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
                        crate::log_debug(
                            "Audio description: cancellation requested by closing dialog",
                        );
                        cancel.store(true, Ordering::SeqCst);
                    }
                    set_text((*pointer).status, &labels((*pointer).language).canceling);
                    return LRESULT(0);
                }
                if !pointer.is_null() {
                    persist_audio_description_preferences(&*pointer);
                }
                crate::log_if_err!(
                    DestroyWindow(hwnd),
                    "Audio description: DestroyWindow failed"
                );
                LRESULT(0)
            }
            WM_DESTROY => {
                clear_player_return_for_window(hwnd);
                let pointer =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut WindowState;
                if !pointer.is_null() {
                    let parent = (*pointer).parent;
                    let return_to_editor_after_player = (*pointer).return_to_editor_after_player;
                    let source_player_path = (*pointer).source_player_path.clone();
                    if with_state(parent, |state| state.audio_description_window = HWND(0))
                        .is_none()
                    {
                        crate::log_debug(
                            "Audio description: parent state unavailable during WM_DESTROY",
                        );
                    }
                    crate::recover_main_window_after_audio_description(
                        parent,
                        "audio_description_window_destroy",
                    );
                    if return_to_editor_after_player {
                        crate::finish_audio_description_after_output_preview(
                            parent,
                            source_player_path.as_deref(),
                        );
                    } else if crate::is_window_handle_valid(parent) {
                        // Hand focus back only after this secondary window is being destroyed.
                        // WM_FOCUS_EDITOR uses Sonarpad's normal editor focus/accessibility path.
                        SetForegroundWindow(parent);
                        crate::log_debug(
                            "Audio description: scheduling editor focus after window close",
                        );
                        crate::log_if_err!(PostMessageW(
                            parent,
                            crate::WM_FOCUS_EDITOR,
                            WPARAM(0),
                            LPARAM(0),
                        ));
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

#[cfg(test)]
mod tests {
    use super::{localized_status_text, should_move_input_to_recycle_bin};
    use crate::settings::Language;

    #[test]
    fn deleting_source_is_never_allowed_when_project_saving_is_enabled() {
        assert!(should_move_input_to_recycle_bin(true, false));
        assert!(!should_move_input_to_recycle_bin(true, true));
        assert!(!should_move_input_to_recycle_bin(false, false));
    }

    #[test]
    fn omni_port_audio_description_status_uses_localized_pyannote_count() {
        let text = localized_status_text(Language::Italian, "pyannote_done", "7");
        assert!(text.contains('7'));
        assert!(text.contains("intervalli"));
        assert!(!text.contains("protected speech intervals"));
    }

    #[test]
    fn omni_port_audio_description_status_localizes_gemini_chunk_numbers() {
        let text = localized_status_text(
            Language::German,
            "gemini_chunk",
            r#"{"current":2,"total":5}"#,
        );
        assert!(text.contains('2'));
        assert!(text.contains('5'));
        assert!(text.contains("Segment"));
    }
    #[test]
    fn omni_port_audio_description_status_localizes_worker_upload_without_fallback() {
        let fallback = "Uploading video to Gemini...";
        let text = localized_status_text(Language::Italian, "gemini_uploading", fallback);
        assert!(text.contains("Caricamento"));
        assert!(!text.contains("Uploading"));
    }

    #[test]
    fn omni_port_audio_description_unknown_gemini_stage_never_leaks_english() {
        let fallback = "Contacting Gemini API...";
        let text = localized_status_text(Language::Italian, "gemini_future_stage", fallback);
        assert!(text.contains("Gemini"));
        assert!(!text.contains("Contacting"));
    }
}
