use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY,
    OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Controls::{WC_BUTTON, WC_COMBOBOXW, WC_EDIT, WC_STATIC};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_RESETCONTENT, CB_SETCURSEL, CBS_DROPDOWN,
    CBS_DROPDOWNLIST, CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DestroyWindow,
    ES_AUTOHSCROLL, GetWindowLongPtrW, HMENU, IDC_ARROW, IsWindow, LoadCursorW, PostMessageW,
    SendMessageW, SetForegroundWindow, SetWindowLongPtrW, ShowWindow, WINDOW_STYLE, WM_CLOSE,
    WM_COMMAND, WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_SETFOCUS, WM_SETFONT,
    WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME,
    WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{PCWSTR, PWSTR};

use crate::accessibility::to_wide;
use crate::app_windows::podcast_save_window::{self, SaveDialogLabels};
use crate::ffmpeg_export::{
    ConvertAudioFormat, ConvertAudioQuality, ConvertAudioSettings, build_ffmpeg_args,
    validate_mp3_bitrate,
};
use crate::i18n;
use crate::settings::{FileFormat, Language};
use crate::{log_debug, show_error, show_info, with_state};

const CONVERT_CLASS_NAME: &str = "SonarpadConvertAudio";

const CONVERT_ID_INPUT_EDIT: usize = 9601;
const CONVERT_ID_INPUT_BROWSE: usize = 9602;
const CONVERT_ID_OUTPUT_EDIT: usize = 9603;
const CONVERT_ID_OUTPUT_BROWSE: usize = 9604;
const CONVERT_ID_FORMAT: usize = 9605;
const CONVERT_ID_QUALITY_EDIT: usize = 9606;
const CONVERT_ID_QUALITY_COMBO: usize = 9607;
const CONVERT_ID_CONVERT: usize = 9608;
const CONVERT_ID_CLOSE: usize = 9609;
const CONVERT_ID_IMAGE_EDIT: usize = 9610;
const CONVERT_ID_IMAGE_BROWSE: usize = 9611;

const WM_CONVERT_INPUT_CHANGED: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 142;

const WM_CONVERT_PROGRESS: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 141;

const WM_CONVERT_DONE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 140;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MediaFormat {
    Mp3,
    Aac,
    M4b,
    Opus,
    Ogg,
    Flac,
    Wav,
    Aiff,
    Mp4,
    Avi,
    Mov,
}

struct ConvertLabels {
    title: String,
    input: String,
    output: String,
    choose_input: String,
    choose_output: String,
    choose_image: String,
    format: String,
    format_mp3: String,
    format_aac: String,
    format_m4b: String,
    format_opus: String,
    format_ogg: String,
    format_flac: String,
    format_wav: String,
    format_aiff: String,
    quality_bitrate: String,
    quality_ogg: String,
    quality_flac: String,
    quality_wav: String,
    wav_float: String,
    status_ready: String,
    status_running: String,
    status_done: String,
    button_convert: String,
    button_close: String,
    error_no_input: String,
    error_no_output: String,
    error_invalid_bitrate: String,
    error_same_path: String,
    error_failed: String,
    success: String,
    open_title: String,
    save_title: String,
}

struct ConvertWindowState {
    hwnd: HWND,
    parent: HWND,
    input_edit: HWND,
    input_probe: Arc<Mutex<(String, Option<bool>)>>,
    image_label: HWND,
    image_edit: HWND,
    image_browse: HWND,
    output_edit: HWND,
    format_combo: HWND,
    quality_label: HWND,
    quality_edit_combo: HWND,
    quality_combo: HWND,
    convert_button: HWND,
    close_button: HWND,
    status_label: HWND,
    language: Language,
    status_dialog: HWND,
    cancel_flag: Option<Arc<AtomicBool>>,
    running: bool,
}

fn labels(language: Language) -> ConvertLabels {
    ConvertLabels {
        title: i18n::tr(language, "convert_audio.title"),
        input: i18n::tr(language, "convert_audio.input"),
        output: i18n::tr(language, "convert_audio.output"),
        choose_input: i18n::tr(language, "convert_audio.choose_input"),
        choose_output: i18n::tr(language, "convert_audio.choose_output"),
        choose_image: i18n::tr(language, "convert_audio.choose_image"),
        format: i18n::tr(language, "convert_audio.format"),
        format_mp3: i18n::tr(language, "convert_audio.format.mp3"),
        format_aac: i18n::tr(language, "convert_audio.format.aac"),
        format_m4b: i18n::tr(language, "convert_audio.format.m4b"),
        format_opus: i18n::tr(language, "convert_audio.format.opus"),
        format_ogg: i18n::tr(language, "convert_audio.format.ogg"),
        format_flac: i18n::tr(language, "convert_audio.format.flac"),
        format_wav: i18n::tr(language, "convert_audio.format.wav"),
        format_aiff: i18n::tr(language, "convert_audio.format.aiff"),
        quality_bitrate: i18n::tr(language, "convert_audio.quality.bitrate"),
        quality_ogg: i18n::tr(language, "convert_audio.quality.ogg"),
        quality_flac: i18n::tr(language, "convert_audio.quality.flac"),
        quality_wav: i18n::tr(language, "convert_audio.quality.wav"),
        wav_float: i18n::tr(language, "convert_audio.wav_float"),
        status_ready: i18n::tr(language, "convert_audio.status.ready"),
        status_running: i18n::tr(language, "convert_audio.status.running"),
        status_done: i18n::tr(language, "convert_audio.status.done"),
        button_convert: i18n::tr(language, "convert_audio.button.convert"),
        button_close: i18n::tr(language, "convert_audio.button.close"),
        error_no_input: i18n::tr(language, "convert_audio.error.no_input"),
        error_no_output: i18n::tr(language, "convert_audio.error.no_output"),
        error_invalid_bitrate: i18n::tr(language, "convert_audio.error.invalid_bitrate"),
        error_same_path: i18n::tr(language, "convert_audio.error.same_path"),
        error_failed: i18n::tr(language, "convert_audio.error.failed"),
        success: i18n::tr(language, "convert_audio.success"),
        open_title: i18n::tr(language, "convert_audio.open_title"),
        save_title: i18n::tr(language, "convert_audio.save_title"),
    }
}

pub fn handle_navigation(hwnd: HWND, msg: &windows::Win32::UI::WindowsAndMessaging::MSG) -> bool {
    if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_ESCAPE.0 as u32 {
        if let Err(e) = crate::post_message_w_safe(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)) {
            crate::log_debug(&format!("Failed to post WM_CLOSE: {}", e));
        }
        return true;
    }
    false
}

pub fn open(parent: HWND) {
    let existing = { with_state(parent, |state| state.convert_audio_window).unwrap_or(HWND(0)) };
    if existing.0 != 0 {
        if !crate::is_window_handle_valid(existing) {
            let result = {
                with_state(parent, |state| {
                    state.convert_audio_window = HWND(0);
                })
            };
            if result.is_none() {
                crate::log_debug("Failed to access convert window state");
            }
        } else {
            unsafe {
                SetForegroundWindow(existing);
            }
            return;
        }
    }

    let language = { with_state(parent, |state| state.settings.language) }.unwrap_or_default();
    let class_name = to_wide(CONVERT_CLASS_NAME);
    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(unsafe {
            LoadCursorW(None, IDC_ARROW).unwrap_or_default().0
        }),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(convert_wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    crate::register_class_w_safe(&wc);

    let labels = labels(language);
    let title = to_wide(&labels.title);

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            140,
            140,
            660,
            390,
            parent,
            HMENU(0),
            hinstance,
            None,
        )
    };
    if hwnd.0 == 0 {
        return;
    }

    if {
        with_state(parent, |state| {
            state.convert_audio_window = hwnd;
        })
    }
    .is_none()
    {
        crate::log_debug("Failed to access convert window state");
    }
}
unsafe extern "system" fn convert_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "convert_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || convert_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn convert_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = &*(lparam.0 as *const CREATESTRUCTW);
                let parent = cs.hwndParent;
                let language =
                    with_state(parent, |state| state.settings.language).unwrap_or_default();
                let labels = labels(language);
                let hfont = with_state(parent, |state| state.hfont).unwrap_or(HFONT(0));

                let input_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.input).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    20,
                    18,
                    220,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let input_edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    20,
                    42,
                    380,
                    26,
                    hwnd,
                    HMENU(CONVERT_ID_INPUT_EDIT as isize),
                    HINSTANCE(0),
                    None,
                );
                let input_browse = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.choose_input).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    412,
                    42,
                    200,
                    26,
                    hwnd,
                    HMENU(CONVERT_ID_INPUT_BROWSE as isize),
                    HINSTANCE(0),
                    None,
                );

                let output_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.output).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    20,
                    82,
                    220,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let output_edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    20,
                    106,
                    380,
                    26,
                    hwnd,
                    HMENU(CONVERT_ID_OUTPUT_EDIT as isize),
                    HINSTANCE(0),
                    None,
                );
                let output_browse = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.choose_output).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    412,
                    106,
                    200,
                    26,
                    hwnd,
                    HMENU(CONVERT_ID_OUTPUT_BROWSE as isize),
                    HINSTANCE(0),
                    None,
                );

                let format_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.format).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    20,
                    152,
                    200,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let format_combo = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    240,
                    148,
                    200,
                    220,
                    hwnd,
                    HMENU(CONVERT_ID_FORMAT as isize),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(
                    format_combo,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&labels.format_mp3).as_ptr() as isize),
                );
                SendMessageW(
                    format_combo,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&labels.format_aac).as_ptr() as isize),
                );
                SendMessageW(
                    format_combo,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&labels.format_m4b).as_ptr() as isize),
                );
                SendMessageW(
                    format_combo,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&labels.format_opus).as_ptr() as isize),
                );
                SendMessageW(
                    format_combo,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&labels.format_ogg).as_ptr() as isize),
                );
                SendMessageW(
                    format_combo,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&labels.format_flac).as_ptr() as isize),
                );
                SendMessageW(
                    format_combo,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&labels.format_wav).as_ptr() as isize),
                );
                SendMessageW(
                    format_combo,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&labels.format_aiff).as_ptr() as isize),
                );
                SendMessageW(format_combo, CB_SETCURSEL, WPARAM(0), LPARAM(0));

                let quality_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.quality_bitrate).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    20,
                    188,
                    210,
                    34,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let quality_edit_combo = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWN as u32),
                    240,
                    184,
                    160,
                    220,
                    hwnd,
                    HMENU(CONVERT_ID_QUALITY_EDIT as isize),
                    HINSTANCE(0),
                    None,
                );
                let quality_combo = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    240,
                    184,
                    160,
                    220,
                    hwnd,
                    HMENU(CONVERT_ID_QUALITY_COMBO as isize),
                    HINSTANCE(0),
                    None,
                );

                for label in ["MP4", "AVI", "MOV"] {
                    SendMessageW(
                        format_combo,
                        CB_ADDSTRING,
                        WPARAM(0),
                        LPARAM(to_wide(label).as_ptr() as isize),
                    );
                }
                let image_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&i18n::tr(language, "convert_audio.image")).as_ptr()),
                    WS_CHILD,
                    20,
                    230,
                    200,
                    22,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let image_edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    240,
                    228,
                    160,
                    24,
                    hwnd,
                    HMENU(CONVERT_ID_IMAGE_EDIT as isize),
                    HINSTANCE(0),
                    None,
                );
                let image_browse = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.choose_image).as_ptr()),
                    WS_CHILD | WS_TABSTOP,
                    412,
                    226,
                    200,
                    28,
                    hwnd,
                    HMENU(CONVERT_ID_IMAGE_BROWSE as isize),
                    HINSTANCE(0),
                    None,
                );

                let status_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.status_ready).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    20,
                    292,
                    310,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let convert_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.button_convert).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    340,
                    286,
                    160,
                    30,
                    hwnd,
                    HMENU(CONVERT_ID_CONVERT as isize),
                    HINSTANCE(0),
                    None,
                );
                let close_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.button_close).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    512,
                    286,
                    100,
                    30,
                    hwnd,
                    HMENU(CONVERT_ID_CLOSE as isize),
                    HINSTANCE(0),
                    None,
                );

                for control in [
                    image_label,
                    image_edit,
                    image_browse,
                    input_label,
                    input_edit,
                    input_browse,
                    output_label,
                    output_edit,
                    output_browse,
                    format_label,
                    format_combo,
                    quality_label,
                    quality_edit_combo,
                    quality_combo,
                    status_label,
                    convert_button,
                    close_button,
                ] {
                    if control.0 != 0 && hfont.0 != 0 {
                        SendMessageW(control, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                }

                let mut state = Box::new(ConvertWindowState {
                    hwnd,
                    parent,
                    input_edit,
                    input_probe: Arc::new(Mutex::new((String::new(), None))),
                    image_label,
                    image_edit,
                    image_browse,
                    output_edit,
                    format_combo,
                    quality_label,
                    quality_edit_combo,
                    quality_combo,
                    convert_button,
                    close_button,
                    status_label,
                    language,
                    status_dialog: HWND(0),
                    cancel_flag: None,
                    running: false,
                });

                update_quality_controls(&mut state, MediaFormat::Mp3, &labels);
                ShowWindow(
                    state.quality_combo,
                    windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
                );
                if let Some(path) = current_media_path(parent) {
                    set_edit_text(state.input_edit, &path);
                    if get_edit_text(state.output_edit).is_empty()
                        && let Some(suggested) =
                            build_default_output_path(&path, current_format(state.format_combo))
                    {
                        set_edit_text(state.output_edit, &suggested);
                    }
                }

                SetWindowLongPtrW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                    Box::into_raw(state) as isize,
                );
                SetFocus(input_edit);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut ConvertWindowState;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *ptr;
                SetFocus(state.input_edit);
                LRESULT(0)
            }
            WM_COMMAND => {
                let cmd_id = wparam.0 & 0xFFFF;
                let notify = (wparam.0 >> 16) as u16;
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut ConvertWindowState;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *ptr;
                let labels = labels(state.language);

                match cmd_id {
                    CONVERT_ID_INPUT_EDIT
                        if notify as u32 == windows::Win32::UI::WindowsAndMessaging::EN_CHANGE =>
                    {
                        // Defer updates: SetWindowText can notify while another handler owns state.
                        if let Err(error) =
                            PostMessageW(hwnd, WM_CONVERT_INPUT_CHANGED, WPARAM(0), LPARAM(0))
                        {
                            log_debug(&format!("Convert media input notification failed: {error}"));
                        }
                    }
                    CONVERT_ID_IMAGE_BROWSE => {
                        if !state.running
                            && let Some(path) =
                                open_input_dialog(state.hwnd, state.language, &labels, true)
                        {
                            set_edit_text(state.image_edit, &path);
                        }
                    }
                    CONVERT_ID_INPUT_BROWSE => {
                        if state.running {
                            return LRESULT(0);
                        }
                        if let Some(path) =
                            open_input_dialog(state.hwnd, state.language, &labels, false)
                        {
                            set_edit_text(state.input_edit, &path);
                            if get_edit_text(state.output_edit).is_empty()
                                && let Some(suggested) = build_default_output_path(
                                    &path,
                                    current_format(state.format_combo),
                                )
                            {
                                set_edit_text(state.output_edit, &suggested);
                            }
                            SetForegroundWindow(state.hwnd);
                            SetFocus(state.input_edit);
                        }
                    }
                    CONVERT_ID_OUTPUT_BROWSE => {
                        if state.running {
                            return LRESULT(0);
                        }
                        let format = current_format(state.format_combo);
                        let initial = get_edit_text(state.output_edit);
                        let initial_path = if initial.is_empty() {
                            None
                        } else {
                            Some(PathBuf::from(initial))
                        };
                        if let Some(path) = open_output_dialog(
                            state.hwnd,
                            state.language,
                            &labels,
                            format,
                            initial_path.as_ref(),
                        ) {
                            set_edit_text(state.output_edit, &path);
                            SetForegroundWindow(state.hwnd);
                            SetFocus(state.output_edit);
                        }
                    }
                    CONVERT_ID_FORMAT
                        if notify as u32
                            == windows::Win32::UI::WindowsAndMessaging::CBN_SELCHANGE =>
                    {
                        let format = current_format(state.format_combo);
                        update_quality_controls(state, format, &labels);
                        let current_output = get_edit_text(state.output_edit);
                        if !current_output.is_empty() {
                            let mut path = PathBuf::from(current_output);
                            path.set_extension(extension_for_format(format));
                            set_edit_text(state.output_edit, &path);
                        }
                    }
                    CONVERT_ID_FORMAT => {}
                    CONVERT_ID_CONVERT => {
                        if state.running {
                            return LRESULT(0);
                        }
                        let input_text = get_edit_text(state.input_edit);
                        if input_text.is_empty() {
                            show_error(state.parent, state.language, &labels.error_no_input);
                            return LRESULT(0);
                        }
                        let output_text = get_edit_text(state.output_edit);
                        if output_text.is_empty() {
                            show_error(state.parent, state.language, &labels.error_no_output);
                            return LRESULT(0);
                        }
                        if crate::media_conversion::same_file(
                            Path::new(&input_text),
                            Path::new(&output_text),
                        ) {
                            show_error(state.parent, state.language, &labels.error_same_path);
                            return LRESULT(0);
                        }

                        let format = current_format(state.format_combo);
                        let quality = match read_quality(state, format, &labels) {
                            Ok(q) => q,
                            Err(err) => {
                                show_error(state.parent, state.language, &err);
                                return LRESULT(0);
                            }
                        };

                        let image_text = get_edit_text(state.image_edit);
                        let needs_image =
                            is_video(format) && is_audio_only_input(Path::new(&input_text));
                        if needs_image && image_text.trim().is_empty() {
                            show_error(
                                state.parent,
                                state.language,
                                &i18n::tr(state.language, "convert_audio.error.no_image"),
                            );
                            return LRESULT(0);
                        }
                        if needs_image
                            && !image_text.is_empty()
                            && crate::media_conversion::same_file(
                                Path::new(&image_text),
                                Path::new(&output_text),
                            )
                        {
                            show_error(state.parent, state.language, &labels.error_same_path);
                            return LRESULT(0);
                        }
                        let settings = ConvertAudioSettings {
                            format: map_format(format),
                            quality,
                        };

                        let args = build_ffmpeg_args(&settings);
                        log_debug(&format!("Convert media audio settings: {}", args.join(" ")));

                        state.running = true;
                        set_status(state, &labels.status_running);
                        set_controls_enabled(state, false);
                        let cancel_flag = Arc::new(AtomicBool::new(false));
                        state.cancel_flag = Some(cancel_flag.clone());
                        if state.status_dialog.0 == 0 {
                            let dialog_labels = SaveDialogLabels {
                                title: labels.title.clone(),
                                in_progress: labels.status_running.clone(),
                                cancel: i18n::tr(state.language, "podcast.save.cancel"),
                                cancel_confirm: i18n::tr(state.language, "podcast.cancel_confirm"),
                            };
                            let dialog = podcast_save_window::open_with_labels(
                                state.hwnd,
                                state.language,
                                dialog_labels,
                                true,
                            );
                            if dialog.0 != 0 {
                                state.status_dialog = dialog;
                                podcast_save_window::disable_fake_progress(dialog);
                            }
                        }

                        let input = PathBuf::from(input_text);
                        let output = PathBuf::from(output_text);
                        let hwnd_target = state.hwnd;

                        std::thread::spawn(move || {
                            let image = if !needs_image || image_text.trim().is_empty() {
                                None
                            } else {
                                Some(PathBuf::from(image_text))
                            };
                            let result = crate::media_conversion::convert(
                                &input,
                                &output,
                                &settings,
                                is_video(format),
                                image.as_deref(),
                                cancel_flag,
                                &mut |pct| {
                                    if let Err(error) = PostMessageW(
                                        hwnd_target,
                                        WM_CONVERT_PROGRESS,
                                        WPARAM(pct as usize),
                                        LPARAM(0),
                                    ) {
                                        log_debug(&format!(
                                            "Failed to post media conversion progress: {error}"
                                        ));
                                    }
                                },
                            )
                            .map(|_| output);
                            let boxed = Box::new(result);
                            if IsWindow(hwnd_target).as_bool() {
                                let raw = Box::into_raw(boxed);
                                if let Err(err) = PostMessageW(
                                    hwnd_target,
                                    WM_CONVERT_DONE,
                                    WPARAM(0),
                                    LPARAM(raw as isize),
                                ) {
                                    log_debug(&format!("Failed to post convert result: {}", err));
                                    let _cleanup = Box::from_raw(raw);
                                }
                            } else {
                                let _boxed = boxed;
                            }
                        });
                    }
                    CONVERT_ID_CLOSE => {
                        if let Err(e) = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)) {
                            log_debug(&format!("Failed to post WM_CLOSE: {}", e));
                        }
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 as u32 == VK_ESCAPE.0 as u32 {
                    if let Err(e) = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)) {
                        log_debug(&format!("Failed to post WM_CLOSE: {}", e));
                    }
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CONVERT_INPUT_CHANGED => {
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut ConvertWindowState;
                if !ptr.is_null() {
                    update_image_controls(&*ptr);
                }
                LRESULT(0)
            }
            WM_CONVERT_PROGRESS => {
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut ConvertWindowState;
                if !ptr.is_null() {
                    let state = &*ptr;
                    if state.running && state.status_dialog.0 != 0 {
                        SendMessageW(
                            state.status_dialog,
                            podcast_save_window::WM_PODCAST_SAVE_PROGRESS,
                            WPARAM(wparam.0.min(100)),
                            LPARAM(0),
                        );
                    }
                }
                LRESULT(0)
            }
            WM_CONVERT_DONE => {
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut ConvertWindowState;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *ptr;
                let labels = labels(state.language);

                let boxed = Box::from_raw(lparam.0 as *mut Result<PathBuf, String>);
                state.running = false;
                set_controls_enabled(state, true);
                state.cancel_flag = None;
                if state.status_dialog.0 != 0 {
                    let dialog = state.status_dialog;
                    state.status_dialog = HWND(0);
                    if IsWindow(dialog).as_bool()
                        && let Err(e) = PostMessageW(
                            dialog,
                            podcast_save_window::WM_PODCAST_SAVE_DONE,
                            WPARAM(0),
                            LPARAM(0),
                        )
                    {
                        log_debug(&format!("Failed to close status dialog: {}", e));
                    }
                }

                match *boxed {
                    Ok(path) => {
                        set_status(state, &labels.status_done);
                        set_edit_text(state.output_edit, &path);
                        show_info(state.parent, state.language, &labels.success);
                    }
                    Err(err) => {
                        if err == "Conversion canceled." {
                            log_debug("Convert media canceled by user");
                            set_status(state, &labels.status_ready);
                            show_info(
                                state.parent,
                                state.language,
                                &i18n::tr(state.language, "tts.cancelled"),
                            );
                        } else {
                            log_debug(&format!("Convert media failed: {}", err));
                            set_status(state, &labels.status_ready);
                            show_error(state.parent, state.language, &labels.error_failed);
                        }
                    }
                }
                LRESULT(0)
            }
            podcast_save_window::WM_PODCAST_SAVE_CANCEL => {
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut ConvertWindowState;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *ptr;
                if let Some(flag) = state.cancel_flag.as_ref() {
                    flag.store(true, Ordering::Relaxed);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                LRESULT(0)
            }
            WM_DESTROY => {
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut ConvertWindowState;
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    if state.status_dialog.0 != 0 {
                        crate::log_if_err!(DestroyWindow(state.status_dialog));
                        state.status_dialog = HWND(0);
                    }
                }
                let parent = windows::Win32::UI::WindowsAndMessaging::GetParent(hwnd);
                if with_state(parent, |state| {
                    state.convert_audio_window = HWND(0);
                })
                .is_none()
                {
                    log_debug("Failed to update convert window state");
                }
                crate::focus_editor(parent);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut ConvertWindowState;
                if !ptr.is_null() {
                    SetWindowLongPtrW(
                        hwnd,
                        windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                        0,
                    );
                    let _unused_box = Box::from_raw(ptr);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn is_video(format: MediaFormat) -> bool {
    matches!(
        format,
        MediaFormat::Mp4 | MediaFormat::Avi | MediaFormat::Mov
    )
}

fn current_format(combo: HWND) -> MediaFormat {
    let sel = crate::send_message_w_safe(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    match sel {
        1 => MediaFormat::Aac,
        2 => MediaFormat::M4b,
        3 => MediaFormat::Opus,
        4 => MediaFormat::Ogg,
        5 => MediaFormat::Flac,
        6 => MediaFormat::Wav,
        7 => MediaFormat::Aiff,
        8 => MediaFormat::Mp4,
        9 => MediaFormat::Avi,
        10 => MediaFormat::Mov,
        _ => MediaFormat::Mp3,
    }
}

fn map_format(format: MediaFormat) -> ConvertAudioFormat {
    match format {
        MediaFormat::Mp3 => ConvertAudioFormat::Mp3,
        MediaFormat::Aac | MediaFormat::M4b => ConvertAudioFormat::Aac,
        MediaFormat::Opus => ConvertAudioFormat::Opus,
        MediaFormat::Ogg => ConvertAudioFormat::Ogg,
        MediaFormat::Flac => ConvertAudioFormat::Flac,
        MediaFormat::Wav => ConvertAudioFormat::Wav,
        MediaFormat::Aiff => ConvertAudioFormat::Aiff,
        MediaFormat::Mp4 | MediaFormat::Mov => ConvertAudioFormat::Aac,
        MediaFormat::Avi => ConvertAudioFormat::Mp3,
    }
}

fn current_media_path(parent: HWND) -> Option<PathBuf> {
    {
        with_state(parent, |state| {
            if let Some(player) = &state.active_audiobook {
                return Some(player.path.clone());
            }
            if let Some(doc) = state.docs.get(state.current)
                && matches!(doc.format, FileFormat::Audiobook)
            {
                return doc.path.clone();
            }
            None
        })
        .unwrap_or(None)
    }
}

fn extension_for_format(format: MediaFormat) -> &'static str {
    match format {
        MediaFormat::Mp3 => "mp3",
        MediaFormat::Aac => "m4a",
        MediaFormat::M4b => "m4b",
        MediaFormat::Opus => "opus",
        MediaFormat::Ogg => "ogg",
        MediaFormat::Flac => "flac",
        MediaFormat::Wav => "wav",
        MediaFormat::Aiff => "aiff",
        MediaFormat::Mp4 => "mp4",
        MediaFormat::Avi => "avi",
        MediaFormat::Mov => "mov",
    }
}

fn build_default_output_path(input: &Path, format: MediaFormat) -> Option<PathBuf> {
    let parent = input.parent()?;
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("audio");
    let file_name = format!("{stem}_converted.{}", extension_for_format(format));
    Some(parent.join(file_name))
}

fn read_quality(
    state: &ConvertWindowState,
    format: MediaFormat,
    labels: &ConvertLabels,
) -> Result<ConvertAudioQuality, String> {
    match format {
        MediaFormat::Mp3 => {
            let text = get_combo_text(state.quality_edit_combo);
            let value = text
                .trim()
                .parse::<i32>()
                .map_err(|_| labels.error_invalid_bitrate.clone())?;
            validate_mp3_bitrate(value)
                .map(ConvertAudioQuality::BitrateKbps)
                .map_err(|_| labels.error_invalid_bitrate.clone())
        }
        MediaFormat::Aac
        | MediaFormat::M4b
        | MediaFormat::Opus
        | MediaFormat::Mp4
        | MediaFormat::Avi
        | MediaFormat::Mov => {
            let text = get_combo_text(state.quality_combo);
            let value = text
                .trim()
                .parse::<u32>()
                .map_err(|_| labels.error_invalid_bitrate.clone())?;
            Ok(ConvertAudioQuality::BitrateKbps(value))
        }
        MediaFormat::Ogg => {
            let text = get_combo_text(state.quality_combo);
            let q = text.trim().trim_start_matches('q');
            let value = q
                .parse::<u8>()
                .map_err(|_| labels.error_invalid_bitrate.clone())?;
            Ok(ConvertAudioQuality::OggQuality(value))
        }
        MediaFormat::Flac => {
            let text = get_combo_text(state.quality_combo);
            let value = text
                .trim()
                .parse::<u8>()
                .map_err(|_| labels.error_invalid_bitrate.clone())?;
            Ok(ConvertAudioQuality::FlacCompression(value))
        }
        MediaFormat::Wav => Ok(
            match unsafe { SendMessageW(state.quality_combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 }
            {
                1 => ConvertAudioQuality::WavPcm24,
                2 => ConvertAudioQuality::WavPcm32,
                3 => ConvertAudioQuality::WavFloat32,
                _ => ConvertAudioQuality::None,
            },
        ),
        MediaFormat::Aiff => Ok(ConvertAudioQuality::None),
    }
}

fn set_status(state: &ConvertWindowState, text: &str) {
    let wide = to_wide(text);
    if let Err(e) = crate::set_window_text_w_safe(state.status_label, PCWSTR(wide.as_ptr())) {
        log_debug(&format!("Failed to set status text: {}", e));
    }
}

fn set_controls_enabled(state: &ConvertWindowState, enabled: bool) {
    unsafe {
        EnableWindow(state.input_edit, enabled);
        EnableWindow(state.image_edit, enabled);
        EnableWindow(state.image_browse, enabled);
        EnableWindow(state.output_edit, enabled);
        EnableWindow(state.format_combo, enabled);
        EnableWindow(state.quality_edit_combo, enabled);
        EnableWindow(state.quality_combo, enabled);
        EnableWindow(state.convert_button, enabled);
        EnableWindow(state.close_button, enabled);
    }
}

// Inspect streams rather than extensions: MP4/WebM can contain audio only, and
// an attached album cover does not make an audio file a video source.
fn is_audio_only_input(path: &Path) -> bool {
    path.is_file()
        && crate::ffmpeg_source::has_real_video_stream(path).is_ok_and(|video| !video)
        && crate::ffmpeg_source::list_audio_streams(path).is_ok_and(|streams| !streams.is_empty())
}

fn update_image_controls(state: &ConvertWindowState) {
    let input = get_edit_text(state.input_edit);
    let video_output = is_video(current_format(state.format_combo));
    let mut visible = false;
    if let Ok(mut probe) = state.input_probe.lock() {
        if probe.0 != input {
            *probe = (input.clone(), None);
        }
        if video_output && !input.trim().is_empty() {
            if let Some(audio_only) = probe.1 {
                visible = audio_only;
            } else {
                // Probe off the UI thread. A pending/invalid input keeps the controls hidden.
                probe.1 = Some(false);
                let result = Arc::clone(&state.input_probe);
                let target = state.hwnd;
                std::thread::spawn(move || {
                    let audio_only = is_audio_only_input(Path::new(&input));
                    if let Ok(mut probe) = result.lock() {
                        if probe.0 != input {
                            return;
                        }
                        probe.1 = Some(audio_only);
                    }
                    unsafe {
                        if let Err(error) =
                            PostMessageW(target, WM_CONVERT_INPUT_CHANGED, WPARAM(0), LPARAM(0))
                        {
                            log_debug(&format!("Convert media probe notification failed: {error}"));
                        }
                    }
                });
            }
        }
    }
    unsafe {
        for control in [state.image_label, state.image_edit, state.image_browse] {
            ShowWindow(
                control,
                if visible {
                    windows::Win32::UI::WindowsAndMessaging::SW_SHOW
                } else {
                    windows::Win32::UI::WindowsAndMessaging::SW_HIDE
                },
            );
        }
    }
}

fn update_quality_controls(
    state: &mut ConvertWindowState,
    format: MediaFormat,
    labels: &ConvertLabels,
) {
    let (label_text, show_edit, show_combo) = match format {
        MediaFormat::Mp3 => (&labels.quality_bitrate, true, false),
        MediaFormat::Aac
        | MediaFormat::M4b
        | MediaFormat::Opus
        | MediaFormat::Mp4
        | MediaFormat::Avi
        | MediaFormat::Mov => (&labels.quality_bitrate, false, true),
        MediaFormat::Ogg => (&labels.quality_ogg, false, true),
        MediaFormat::Flac => (&labels.quality_flac, false, true),
        MediaFormat::Wav => (&labels.quality_wav, false, true),
        MediaFormat::Aiff => (&labels.quality_bitrate, false, false),
    };

    update_image_controls(state);
    let label_wide = to_wide(label_text);
    if let Err(e) = crate::set_window_text_w_safe(state.quality_label, PCWSTR(label_wide.as_ptr()))
    {
        log_debug(&format!("Failed to set quality label: {}", e));
    }

    unsafe {
        ShowWindow(
            state.quality_label,
            if show_edit || show_combo {
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW
            } else {
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE
            },
        );
        ShowWindow(
            state.quality_edit_combo,
            if show_edit {
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW
            } else {
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE
            },
        );
        ShowWindow(
            state.quality_combo,
            if show_combo {
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW
            } else {
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE
            },
        );
    }

    if show_edit {
        set_combo_items(
            state.quality_edit_combo,
            &[
                "64", "80", "96", "112", "128", "160", "192", "224", "256", "320",
            ],
        );
        if get_combo_text(state.quality_edit_combo).is_empty() {
            set_combo_text(state.quality_edit_combo, "192");
        }
    }
    if show_combo {
        let items: Vec<&str> = match format {
            MediaFormat::Aac
            | MediaFormat::M4b
            | MediaFormat::Mp4
            | MediaFormat::Avi
            | MediaFormat::Mov => vec![
                "64", "80", "96", "112", "128", "160", "192", "224", "256", "320",
            ],
            MediaFormat::Wav => vec!["16-bit", "24-bit", "32-bit", &labels.wav_float],
            MediaFormat::Opus => vec!["64", "96", "128", "160"],
            MediaFormat::Ogg => vec!["q3", "q4", "q5", "q6", "q7", "q8"],
            MediaFormat::Flac => vec!["0", "1", "2", "3", "4", "5", "6", "7", "8"],
            _ => Vec::new(),
        };
        set_combo_items(state.quality_combo, &items);
        let default_index = match format {
            MediaFormat::Aac
            | MediaFormat::M4b
            | MediaFormat::Mp4
            | MediaFormat::Avi
            | MediaFormat::Mov => 4,
            MediaFormat::Opus => 2,
            MediaFormat::Ogg => 2,
            MediaFormat::Flac => 5,
            _ => 0,
        };
        crate::send_message_w_safe(
            state.quality_combo,
            CB_SETCURSEL,
            WPARAM(default_index),
            LPARAM(0),
        );
    }
}

fn set_combo_items(hwnd: HWND, items: &[&str]) {
    unsafe {
        SendMessageW(hwnd, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        for item in items {
            SendMessageW(
                hwnd,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(item).as_ptr() as isize),
            );
        }
    }
}

fn set_combo_text(hwnd: HWND, text: &str) {
    let wide = to_wide(text);
    if let Err(e) = crate::set_window_text_w_safe(hwnd, PCWSTR(wide.as_ptr())) {
        log_debug(&format!("Failed to set combo text: {}", e));
    }
}

fn get_combo_text(hwnd: HWND) -> String {
    let len = crate::get_window_text_length_w_safe(hwnd) as usize;
    if len == 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len + 1];
    let read = crate::get_window_text_w_safe(hwnd, &mut buf) as usize;
    let used = read.min(len);
    String::from_utf16_lossy(&buf[..used])
}

fn get_edit_text(hwnd: HWND) -> String {
    let len = crate::get_window_text_length_w_safe(hwnd) as usize;
    if len == 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len + 1];
    let read = crate::get_window_text_w_safe(hwnd, &mut buf) as usize;
    let used = read.min(len);
    String::from_utf16_lossy(&buf[..used])
}

fn set_edit_text(hwnd: HWND, path: &Path) {
    let wide = to_wide(&path.to_string_lossy());
    if let Err(e) = crate::set_window_text_w_safe(hwnd, PCWSTR(wide.as_ptr())) {
        log_debug(&format!("Failed to set edit text: {}", e));
    }
}

fn open_input_dialog(
    parent: HWND,
    language: Language,
    labels: &ConvertLabels,
    image: bool,
) -> Option<PathBuf> {
    unsafe {
        let all = i18n::tr(language, "dialog.all_files");
        let description = if image {
            i18n::tr(language, "convert_audio.image")
        } else {
            labels.open_title.clone()
        };
        let extensions = if image {
            "*.jpg;*.jpeg;*.png;*.webp;*.bmp"
        } else {
            "*.mp3;*.m4a;*.mp4;*.aac;*.mkv;*.avi;*.mov;*.m4v;*.webm;*.mpg;*.mpeg;*.ts;*.m2ts;*.mts;*.wmv;*.asf;*.flv;*.vob;*.3gp;*.flac;*.ogg;*.opus;*.wma;*.aiff;*.aif;*.m4b;*.wav"
        };
        let filter_raw = format!("{description}\0{extensions}\0{all}\0*.*\0\0");
        let filter = to_wide(&filter_raw.replace("\\0", "\0"));
        let title = to_wide(&description);
        let mut buffer = [0u16; 1024];
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: parent,
            lpstrFile: PWSTR(buffer.as_mut_ptr()),
            nMaxFile: buffer.len() as u32,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrTitle: PCWSTR(title.as_ptr()),
            Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY,
            ..Default::default()
        };
        if !GetOpenFileNameW(&mut ofn).as_bool() {
            return None;
        }
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        if len == 0 {
            return None;
        }
        Some(PathBuf::from(String::from_utf16_lossy(&buffer[..len])))
    }
}
fn open_output_dialog(
    parent: HWND,
    language: Language,
    labels: &ConvertLabels,
    format: MediaFormat,
    initial: Option<&PathBuf>,
) -> Option<PathBuf> {
    unsafe {
        let all_label = i18n::tr(language, "dialog.all_files");
        let filter = format!("{} (*.*)\0*.*\0\0", all_label);
        let filter_wide = to_wide(&filter);
        let title = to_wide(&labels.save_title);
        let mut buffer = [0u16; 1024];
        if let Some(path) = initial {
            let path_w = to_wide(&path.to_string_lossy());
            let copy_len = path_w.len().min(buffer.len() - 1);
            buffer[..copy_len].copy_from_slice(&path_w[..copy_len]);
        }
        let ext_wide = to_wide(extension_for_format(format));
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: parent,
            lpstrFile: PWSTR(buffer.as_mut_ptr()),
            nMaxFile: buffer.len() as u32,
            lpstrFilter: PCWSTR(filter_wide.as_ptr()),
            lpstrTitle: PCWSTR(title.as_ptr()),
            lpstrDefExt: PCWSTR(ext_wide.as_ptr()),
            Flags: OFN_EXPLORER | OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST,
            ..Default::default()
        };
        if !GetSaveFileNameW(&mut ofn).as_bool() {
            return None;
        }
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        if len == 0 {
            return None;
        }
        let mut path = PathBuf::from(String::from_utf16_lossy(&buffer[..len]));
        if path.extension().is_none() {
            path.set_extension(extension_for_format(format));
        }
        Some(path)
    }
}
