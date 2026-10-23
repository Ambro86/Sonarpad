use crate::accessibility::{handle_accessibility, to_wide};
use crate::app_windows::podcast_save_window;
use crate::audio_monitor::{
    MonitorHandle, start_monitoring, start_process_monitoring, start_processes_monitoring,
    start_system_monitoring,
};
use crate::log_debug;
// VIDEO REMOVED: MonitorInfo and list_monitors imports removed
use crate::podcast_recorder::{
    AudioApp, AudioDevice, RecorderConfig, RecorderHandle, RecorderStatus, default_output_folder,
    list_audio_apps, list_input_devices, list_output_devices, probe_device_with_name,
    probe_process_loopback, start_recording,
};
use crate::settings::{
    AppSettings, Language, PODCAST_DEVICE_DEFAULT, PodcastFormat, PodcastSystemCaptureMode,
    confirm_title, default_podcast_save_folder, save_settings,
};
use crate::{i18n, show_error, with_state};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT};
use windows::Win32::UI::Controls::{
    BST_CHECKED, BST_UNCHECKED, WC_BUTTON, WC_COMBOBOXW, WC_EDIT, WC_LISTBOXW, WC_STATIC,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, SetFocus, VK_CONTROL, VK_ESCAPE, VK_SHIFT,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    BM_CLICK, BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_GROUPBOX,
    CB_ADDSTRING, CB_GETCURSEL, CB_RESETCONTENT, CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST,
    CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow, EN_CHANGE,
    GWLP_USERDATA, GetDlgItem, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, HMENU,
    IDC_ARROW, IDOK, IDYES, KillTimer, LB_ADDSTRING, LB_GETCARETINDEX, LB_GETCOUNT, LB_GETSEL,
    LB_RESETCONTENT, LB_SETSEL, LBN_SELCHANGE, LBS_MULTIPLESEL, LBS_NOINTEGRALHEIGHT, LBS_NOTIFY,
    LoadCursorW, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_OKCANCEL, MB_YESNO,
    MSG, MessageBoxW, PostMessageW, RegisterClassW, SW_SHOWNORMAL, SendMessageW,
    SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowTextW, ShowWindow, WINDOW_STYLE,
    WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_SETFOCUS, WM_SETFONT,
    WM_TIMER, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_SYSMENU,
    WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::PCWSTR;

const PODCAST_CLASS_NAME: &str = "SonarpadPodcast";
const PODCAST_TIMER_ID: usize = 1;
const PODCAST_SYSTEM_TEST_TIMER_ID: usize = 2;
const PODCAST_SYSTEM_TEST_DURATION_MS: u32 = 300;
const PODCAST_MP3_BITRATES: &[u32] = &[64, 96, 128, 160, 192, 224, 256, 320];

const PODCAST_ID_INCLUDE_MIC: usize = 11001;
const PODCAST_ID_MIC_DEVICE: usize = 11002;
const PODCAST_ID_MIC_GAIN: usize = 11021;
const PODCAST_ID_SYSTEM_GAIN: usize = 11022;
const PODCAST_ID_INCLUDE_SYSTEM: usize = 11003;
const PODCAST_ID_SYSTEM_DEVICE: usize = 11004;
const PODCAST_ID_INCLUDE_VIDEO: usize = 11023;
const PODCAST_ID_MONITOR: usize = 11024;
const PODCAST_ID_FORMAT: usize = 11005;
const PODCAST_ID_BITRATE: usize = 11006;
const PODCAST_ID_SAVE_PATH: usize = 11007;
const PODCAST_ID_BROWSE: usize = 11008;
const PODCAST_ID_FILENAME_PREVIEW: usize = 11009;
const PODCAST_ID_START: usize = 11010;
const PODCAST_ID_PAUSE: usize = 11011;
const PODCAST_ID_RESUME: usize = 11012;
const PODCAST_ID_STOP: usize = 11013;
const PODCAST_ID_CLOSE: usize = 11014;
const PODCAST_ID_STATUS: usize = 11015;
const PODCAST_ID_ELAPSED: usize = 11016;
const PODCAST_ID_LEVEL_MIC: usize = 11017;
const PODCAST_ID_LEVEL_SYSTEM: usize = 11018;
const PODCAST_ID_HINT: usize = 11019;
const PODCAST_ID_SYSTEM_UNAVAILABLE: usize = 11020;
const PODCAST_ID_SOURCE: usize = 11025;
const PODCAST_ID_MONITOR_CHECK: usize = 11026;
const PODCAST_ID_SYSTEM_CAPTURE_MODE: usize = 11027;
const PODCAST_ID_SINGLE_APP: usize = 11028;
const PODCAST_ID_REFRESH_SINGLE_APP: usize = 11029;
const PODCAST_ID_TEST_SINGLE_APP_AUDIO: usize = 11030;
const PODCAST_ID_SELECTED_APPS: usize = 11031;
const PODCAST_ID_SHOW_INACTIVE_APPS: usize = 11032;
const PODCAST_ID_SPLIT_SOURCES: usize = 11033;
const WM_PODCAST_SAVE_RESULT: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 74;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveMonitorKind {
    Microphone,
    SingleApp,
    SystemAudioTest,
}

struct PodcastSaveResult {
    success: bool,
    message: String,
    saved_folder: Option<PathBuf>,
}

pub fn handle_navigation(hwnd: HWND, msg: &MSG) -> bool {
    if msg.message == WM_KEYDOWN {
        if with_podcast_state(hwnd, |state| {
            if msg.hwnd == state.selected_apps {
                let key = msg.wParam.0 as u32;
                if key == windows::Win32::UI::Input::KeyboardAndMouse::VK_UP.0 as u32
                    || key == windows::Win32::UI::Input::KeyboardAndMouse::VK_DOWN.0 as u32
                {
                    let count = crate::send_message_w_safe(
                        state.selected_apps,
                        LB_GETCOUNT,
                        WPARAM(0),
                        LPARAM(0),
                    )
                    .0;
                    let caret = crate::send_message_w_safe(
                        state.selected_apps,
                        LB_GETCARETINDEX,
                        WPARAM(0),
                        LPARAM(0),
                    )
                    .0;
                    if count > 0
                        && ((key == windows::Win32::UI::Input::KeyboardAndMouse::VK_UP.0 as u32
                            && caret <= 0)
                            || (key
                                == windows::Win32::UI::Input::KeyboardAndMouse::VK_DOWN.0 as u32
                                && caret >= count - 1))
                    {
                        return true;
                    }
                }
            }
            false
        })
        .unwrap_or(false)
        {
            return true;
        }
        let ctrl = (crate::get_key_state_safe(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
        let shift = (crate::get_key_state_safe(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
        let key = msg.wParam.0 as u32;

        if key == VK_ESCAPE.0 as u32 {
            crate::send_message_w_safe(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
            return true;
        }
        if ctrl && !shift {
            if key == 'S' as u32 {
                click_button(hwnd, PODCAST_ID_START);
                return true;
            }
            if key == 'P' as u32 {
                click_button(hwnd, PODCAST_ID_PAUSE);
                return true;
            }
            if key == 'T' as u32 {
                click_button(hwnd, PODCAST_ID_STOP);
                return true;
            }
        }
    }
    handle_accessibility(hwnd, msg)
}

struct PodcastState {
    parent: HWND,
    language: Language,
    include_mic: HWND,
    mic_device: HWND,
    mic_gain: HWND,
    include_system: HWND,
    split_sources: HWND,
    system_device: HWND,
    system_gain: HWND,
    system_capture_mode: HWND,
    label_single_app: HWND,
    single_app: HWND,
    label_selected_apps: HWND,
    selected_apps: HWND,
    show_inactive_apps: HWND,
    test_single_app_audio: HWND,
    refresh_single_app: HWND,
    monitor_check: HWND,
    // VIDEO REMOVED: include_video, monitor_combo, video_unavailable_text removed
    format_combo: HWND,
    bitrate_combo: HWND,
    save_path: HWND,
    filename_preview: HWND,
    start_button: HWND,
    pause_button: HWND,
    resume_button: HWND,
    stop_button: HWND,
    status_text: HWND,
    source_text: HWND,
    elapsed_text: HWND,
    level_mic_text: HWND,
    level_system_text: HWND,
    hint_text: HWND,
    system_unavailable_text: HWND,
    // VIDEO REMOVED: video_unavailable_text removed
    mic_devices: Vec<AudioDevice>,
    system_devices: Vec<AudioDevice>,
    single_apps: Vec<AudioApp>,
    selected_apps_items: Vec<AudioApp>,
    // VIDEO REMOVED: monitors removed
    recorder: Option<RecorderHandle>,
    monitor_handle: Option<MonitorHandle>,
    active_monitor: Option<ActiveMonitorKind>,
    system_available: bool,
    saving_dialog: HWND,
    save_cancel: Option<Arc<AtomicBool>>,
}

struct PodcastLabels {
    title: String,
    input_group: String,
    output_group: String,
    controls_group: String,
    status_group: String,
    include_mic: String,
    monitor_mic: String,
    mic_device: String,
    mic_gain_label: String,
    system_gain_label: String,
    include_system: String,
    split_sources: String,
    system_capture_mode: String,
    system_capture_mode_all_system: String,
    system_capture_mode_single_app: String,
    system_capture_mode_selected_apps: String,
    system_device: String,
    single_app: String,
    single_app_default: String,
    single_app_none_running: String,
    selected_apps: String,
    show_inactive_apps: String,
    selected_apps_refresh: String,
    single_app_refresh: String,
    test_system_audio: String,
    test_single_app_audio: String,
    test_selected_apps_audio: String,
    system_unavailable: String,
    // VIDEO REMOVED: include_video, monitor, video_unavailable removed
    format: String,
    bitrate: String,
    save_path: String,
    browse: String,
    filename: String,
    start: String,
    pause: String,
    resume: String,
    stop: String,
    close: String,
    status_label: String,
    elapsed_label: String,
    level_mic: String,
    level_system: String,
    status_idle: String,
    status_recording: String,
    status_paused: String,
    status_saving: String,
    status_error: String,
    default_device: String,
    hint_select_source: String,
    confirm_close_recording: String,
    error_system_audio: String,
    error_microphone: String,
    error_single_app_required: String,
    error_single_app_unavailable: String,
    error_selected_apps_required: String,
    error_selected_apps_unavailable: String,
    single_app_audio_unavailable: String,
    selected_apps_audio_unavailable: String,
}
fn labels(language: Language) -> PodcastLabels {
    PodcastLabels {
        title: i18n::tr(language, "podcast.title"),
        input_group: i18n::tr(language, "podcast.group.input"),
        output_group: i18n::tr(language, "podcast.group.output"),
        controls_group: i18n::tr(language, "podcast.group.controls"),
        status_group: i18n::tr(language, "podcast.group.status"),
        include_mic: i18n::tr(language, "podcast.include_mic"),
        monitor_mic: i18n::tr(language, "podcast.monitor_mic"),
        mic_device: i18n::tr(language, "podcast.mic_device"),
        mic_gain_label: i18n::tr(language, "podcast.mic_gain"),
        system_gain_label: i18n::tr(language, "podcast.system_gain"),
        include_system: i18n::tr(language, "podcast.include_system"),
        split_sources: i18n::tr(language, "podcast.split_sources"),
        system_capture_mode: i18n::tr(language, "podcast.system_capture_mode"),
        system_capture_mode_all_system: i18n::tr(
            language,
            "podcast.system_capture_mode.all_system",
        ),
        system_capture_mode_single_app: i18n::tr(
            language,
            "podcast.system_capture_mode.single_app",
        ),
        system_capture_mode_selected_apps: i18n::tr(
            language,
            "podcast.system_capture_mode.selected_apps",
        ),
        system_device: i18n::tr(language, "podcast.system_device"),
        single_app: i18n::tr(language, "podcast.single_app"),
        single_app_default: i18n::tr(language, "podcast.single_app.default"),
        single_app_none_running: i18n::tr(language, "podcast.single_app.none_running"),
        selected_apps: i18n::tr(language, "podcast.selected_apps"),
        show_inactive_apps: i18n::tr(language, "podcast.show_inactive_apps"),
        selected_apps_refresh: i18n::tr(language, "podcast.selected_apps.refresh"),
        single_app_refresh: i18n::tr(language, "podcast.single_app.refresh"),
        test_system_audio: i18n::tr(language, "podcast.test_system_audio"),
        test_single_app_audio: i18n::tr(language, "podcast.test_single_app_audio"),
        test_selected_apps_audio: i18n::tr(language, "podcast.test_selected_apps_audio"),
        system_unavailable: i18n::tr(language, "podcast.system_unavailable"),
        // VIDEO REMOVED: include_video, monitor, video_unavailable removed
        format: i18n::tr(language, "podcast.format"),
        bitrate: i18n::tr(language, "podcast.bitrate"),
        save_path: i18n::tr(language, "podcast.save_path"),
        browse: i18n::tr(language, "podcast.browse"),
        filename: i18n::tr(language, "podcast.filename_preview"),
        start: i18n::tr(language, "podcast.start"),
        pause: i18n::tr(language, "podcast.pause"),
        resume: i18n::tr(language, "podcast.resume"),
        stop: i18n::tr(language, "podcast.stop"),
        close: i18n::tr(language, "podcast.close"),
        status_label: i18n::tr(language, "podcast.status_label"),
        elapsed_label: i18n::tr(language, "podcast.elapsed_label"),
        level_mic: i18n::tr(language, "podcast.level_mic"),
        level_system: i18n::tr(language, "podcast.level_system"),
        status_idle: i18n::tr(language, "podcast.status.idle"),
        status_recording: i18n::tr(language, "podcast.status.recording"),
        status_paused: i18n::tr(language, "podcast.status.paused"),
        status_saving: i18n::tr(language, "podcast.status.saving"),
        status_error: i18n::tr(language, "podcast.status.error"),
        default_device: i18n::tr(language, "podcast.device.default"),
        hint_select_source: i18n::tr(language, "podcast.hint.select_source"),
        confirm_close_recording: i18n::tr(language, "podcast.confirm_close_recording"),
        error_system_audio: i18n::tr(language, "podcast.error.system_audio"),
        error_microphone: i18n::tr(language, "podcast.error.microphone"),
        error_single_app_required: i18n::tr(language, "podcast.error.single_app_required"),
        error_single_app_unavailable: i18n::tr(language, "podcast.error.single_app_unavailable"),
        error_selected_apps_required: i18n::tr(language, "podcast.error.selected_apps_required"),
        error_selected_apps_unavailable: i18n::tr(
            language,
            "podcast.error.selected_apps_unavailable",
        ),
        single_app_audio_unavailable: i18n::tr(language, "podcast.single_app_audio_unavailable"),
        selected_apps_audio_unavailable: i18n::tr(
            language,
            "podcast.selected_apps_audio_unavailable",
        ),
    }
}

pub fn open(parent: HWND) {
    let existing = { with_state(parent, |state| state.podcast_window) }.unwrap_or(HWND(0));
    if existing.0 != 0 {
        unsafe {
            SetForegroundWindow(existing);
        }
        return;
    }

    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let class_name = to_wide(PODCAST_CLASS_NAME);
    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(unsafe {
            LoadCursorW(None, IDC_ARROW).unwrap_or_default().0
        }),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(podcast_wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&wc);
    }

    let language = { with_state(parent, |state| state.settings.language) }.unwrap_or_default();
    let title = to_wide(&labels(language).title);

    let window = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            720,
            860,
            None,
            None,
            hinstance,
            Some(parent.0 as *const std::ffi::c_void),
        )
    };

    if window.0 != 0 {
        unsafe {
            if with_state(parent, |state| {
                state.podcast_window = window;
            })
            .is_none()
            {
                crate::log_debug("Failed to access podcast state");
            }
            SetForegroundWindow(window);
        }
    }
}
unsafe extern "system" fn podcast_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "podcast_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || podcast_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn podcast_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let create_struct = lparam.0 as *const CREATESTRUCTW;
                let parent = HWND((*create_struct).lpCreateParams as isize);
                let language =
                    with_state(parent, |state| state.settings.language).unwrap_or_default();
                let labels = labels(language);
                let hfont = with_state(parent, |state| state.hfont).unwrap_or(HFONT(0));

                let group_input = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.input_group).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_GROUPBOX as u32),
                    10,
                    10,
                    680,
                    350,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let include_mic = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.include_mic).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    20,
                    35,
                    220,
                    22,
                    hwnd,
                    HMENU(PODCAST_ID_INCLUDE_MIC as isize),
                    None,
                    None,
                );

                let label_mic = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.mic_device).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    40,
                    62,
                    180,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let mic_device = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    230,
                    58,
                    430,
                    220,
                    hwnd,
                    HMENU(PODCAST_ID_MIC_DEVICE as isize),
                    None,
                    None,
                );

                let label_mic_gain = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.mic_gain_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    40,
                    85,
                    180,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let mic_gain = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    230,
                    81,
                    140,
                    200,
                    hwnd,
                    HMENU(PODCAST_ID_MIC_GAIN as isize),
                    None,
                    None,
                );

                let monitor_check = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.monitor_mic).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    40,
                    108,
                    220,
                    22,
                    hwnd,
                    HMENU(PODCAST_ID_MONITOR_CHECK as isize),
                    None,
                    None,
                );

                let include_system = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.include_system).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    20,
                    135,
                    220,
                    22,
                    hwnd,
                    HMENU(PODCAST_ID_INCLUDE_SYSTEM as isize),
                    None,
                    None,
                );

                let label_system = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.system_device).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    40,
                    162,
                    180,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let system_device = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    230,
                    158,
                    430,
                    220,
                    hwnd,
                    HMENU(PODCAST_ID_SYSTEM_DEVICE as isize),
                    None,
                    None,
                );

                let label_system_gain = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.system_gain_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    40,
                    185,
                    180,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let system_gain = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    230,
                    181,
                    140,
                    200,
                    hwnd,
                    HMENU(PODCAST_ID_SYSTEM_GAIN as isize),
                    None,
                    None,
                );

                let system_unavailable_text = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.system_unavailable).as_ptr()),
                    WS_CHILD,
                    40,
                    260,
                    620,
                    18,
                    hwnd,
                    HMENU(PODCAST_ID_SYSTEM_UNAVAILABLE as isize),
                    None,
                    None,
                );

                let label_system_capture_mode = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.system_capture_mode).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    40,
                    212,
                    180,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let system_capture_mode = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    230,
                    208,
                    300,
                    220,
                    hwnd,
                    HMENU(PODCAST_ID_SYSTEM_CAPTURE_MODE as isize),
                    None,
                    None,
                );

                let label_single_app = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.single_app).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    60,
                    235,
                    160,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let single_app = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    230,
                    231,
                    300,
                    240,
                    hwnd,
                    HMENU(PODCAST_ID_SINGLE_APP as isize),
                    None,
                    None,
                );

                let label_selected_apps = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.selected_apps).as_ptr()),
                    WS_CHILD,
                    60,
                    235,
                    160,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let selected_apps = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_LISTBOXW,
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_TABSTOP
                        | WS_VSCROLL
                        | WINDOW_STYLE(
                            (LBS_NOTIFY | LBS_MULTIPLESEL | LBS_NOINTEGRALHEIGHT) as u32,
                        ),
                    230,
                    231,
                    300,
                    78,
                    hwnd,
                    HMENU(PODCAST_ID_SELECTED_APPS as isize),
                    None,
                    None,
                );

                let show_inactive_apps = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.show_inactive_apps).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    230,
                    315,
                    300,
                    22,
                    hwnd,
                    HMENU(PODCAST_ID_SHOW_INACTIVE_APPS as isize),
                    None,
                    None,
                );

                let test_single_app_audio = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.test_single_app_audio).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    230,
                    341,
                    300,
                    22,
                    hwnd,
                    HMENU(PODCAST_ID_TEST_SINGLE_APP_AUDIO as isize),
                    None,
                    None,
                );

                let refresh_single_app = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.single_app_refresh).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    550,
                    339,
                    110,
                    26,
                    hwnd,
                    HMENU(PODCAST_ID_REFRESH_SINGLE_APP as isize),
                    None,
                    None,
                );

                // VIDEO REMOVED: All video controls completely removed

                let group_output = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.output_group).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_GROUPBOX as u32),
                    10,
                    380,
                    680,
                    190,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let label_format = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.format).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    24,
                    405,
                    120,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let format_combo = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    155,
                    401,
                    170,
                    220,
                    hwnd,
                    HMENU(PODCAST_ID_FORMAT as isize),
                    None,
                    None,
                );

                let label_bitrate = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.bitrate).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    350,
                    405,
                    120,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let bitrate_combo = CreateWindowExW(
                    Default::default(),
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    480,
                    401,
                    170,
                    220,
                    hwnd,
                    HMENU(PODCAST_ID_BITRATE as isize),
                    None,
                    None,
                );

                let label_save = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.save_path).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    24,
                    440,
                    240,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let save_path = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    24,
                    462,
                    500,
                    26,
                    hwnd,
                    HMENU(PODCAST_ID_SAVE_PATH as isize),
                    None,
                    None,
                );

                let browse_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.browse).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    535,
                    462,
                    125,
                    26,
                    hwnd,
                    HMENU(PODCAST_ID_BROWSE as isize),
                    None,
                    None,
                );

                let label_filename = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.filename).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    24,
                    498,
                    240,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let filename_preview = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE,
                    24,
                    520,
                    636,
                    18,
                    hwnd,
                    HMENU(PODCAST_ID_FILENAME_PREVIEW as isize),
                    None,
                    None,
                );

                let split_sources = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.split_sources).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    24,
                    542,
                    636,
                    22,
                    hwnd,
                    HMENU(PODCAST_ID_SPLIT_SOURCES as isize),
                    None,
                    None,
                );

                let group_controls = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.controls_group).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_GROUPBOX as u32),
                    10,
                    585,
                    680,
                    90,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let start_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.start).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    30,
                    612,
                    110,
                    30,
                    hwnd,
                    HMENU(PODCAST_ID_START as isize),
                    None,
                    None,
                );

                let pause_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.pause).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    155,
                    612,
                    110,
                    30,
                    hwnd,
                    HMENU(PODCAST_ID_PAUSE as isize),
                    None,
                    None,
                );

                let resume_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.resume).as_ptr()),
                    WS_CHILD,
                    155,
                    612,
                    110,
                    30,
                    hwnd,
                    HMENU(PODCAST_ID_RESUME as isize),
                    None,
                    None,
                );

                let stop_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.stop).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    405,
                    612,
                    120,
                    30,
                    hwnd,
                    HMENU(PODCAST_ID_STOP as isize),
                    None,
                    None,
                );

                let close_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.close).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    540,
                    612,
                    120,
                    30,
                    hwnd,
                    HMENU(PODCAST_ID_CLOSE as isize),
                    None,
                    None,
                );

                let group_status = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.status_group).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_GROUPBOX as u32),
                    10,
                    690,
                    680,
                    115,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let status_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.status_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    24,
                    715,
                    90,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let status_text = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.status_idle).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    125,
                    715,
                    200,
                    18,
                    hwnd,
                    HMENU(PODCAST_ID_STATUS as isize),
                    None,
                    None,
                );

                let source_text = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.hint_select_source).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    24,
                    738,
                    636,
                    18,
                    hwnd,
                    HMENU(PODCAST_ID_SOURCE as isize),
                    None,
                    None,
                );

                let elapsed_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.elapsed_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    350,
                    715,
                    130,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let elapsed_text = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide("00:00:00").as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    490,
                    715,
                    150,
                    18,
                    hwnd,
                    HMENU(PODCAST_ID_ELAPSED as isize),
                    None,
                    None,
                );

                let level_mic_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.level_mic).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    24,
                    762,
                    170,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let level_mic_text = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide("0").as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    200,
                    762,
                    80,
                    18,
                    hwnd,
                    HMENU(PODCAST_ID_LEVEL_MIC as isize),
                    None,
                    None,
                );

                let level_system_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.level_system).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    350,
                    762,
                    170,
                    18,
                    hwnd,
                    HMENU(0),
                    None,
                    None,
                );

                let level_system_text = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide("0").as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    526,
                    762,
                    80,
                    18,
                    hwnd,
                    HMENU(PODCAST_ID_LEVEL_SYSTEM as isize),
                    None,
                    None,
                );

                let hint_text = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.hint_select_source).as_ptr()),
                    WS_CHILD,
                    24,
                    785,
                    636,
                    18,
                    hwnd,
                    HMENU(PODCAST_ID_HINT as isize),
                    None,
                    None,
                );

                let controls = [
                    group_input,
                    include_mic,
                    label_mic,
                    mic_device,
                    label_mic_gain,
                    mic_gain,
                    monitor_check,
                    include_system,
                    split_sources,
                    label_system,
                    system_device,
                    label_system_gain,
                    system_gain,
                    label_system_capture_mode,
                    system_capture_mode,
                    label_single_app,
                    single_app,
                    label_selected_apps,
                    selected_apps,
                    show_inactive_apps,
                    test_single_app_audio,
                    refresh_single_app,
                    system_unavailable_text,
                    // VIDEO REMOVED: include_video, label_monitor, monitor_combo, video_unavailable_text removed
                    group_output,
                    label_format,
                    format_combo,
                    label_bitrate,
                    bitrate_combo,
                    label_save,
                    save_path,
                    browse_button,
                    label_filename,
                    filename_preview,
                    group_controls,
                    start_button,
                    pause_button,
                    resume_button,
                    stop_button,
                    close_button,
                    group_status,
                    status_label,
                    status_text,
                    elapsed_label,
                    elapsed_text,
                    level_mic_label,
                    level_mic_text,
                    level_system_label,
                    level_system_text,
                    hint_text,
                ];
                for control in controls {
                    SendMessageW(control, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                }

                populate_combos(
                    format_combo,
                    bitrate_combo,
                    (mic_device, mic_gain),
                    (system_device, system_gain),
                    system_capture_mode,
                    language,
                );

                let (mic_devices, system_devices, system_available) = load_devices(language);
                // VIDEO REMOVED: monitors loading removed
                let settings =
                    with_state(parent, |state| state.settings.clone()).unwrap_or_default();
                let mut state = PodcastState {
                    parent,
                    language,
                    include_mic,
                    mic_device,
                    mic_gain,
                    include_system,
                    split_sources,
                    system_device,
                    system_gain,
                    system_capture_mode,
                    label_single_app,
                    single_app,
                    label_selected_apps,
                    selected_apps,
                    show_inactive_apps,
                    test_single_app_audio,
                    refresh_single_app,
                    monitor_check,
                    // VIDEO REMOVED: include_video, monitor_combo, video_unavailable_text removed
                    format_combo,
                    bitrate_combo,
                    save_path,
                    filename_preview,
                    start_button,
                    pause_button,
                    resume_button,
                    stop_button,
                    status_text,
                    source_text,
                    elapsed_text,
                    level_mic_text,
                    level_system_text,
                    hint_text,
                    system_unavailable_text,
                    // VIDEO REMOVED: video_unavailable_text removed
                    mic_devices,
                    system_devices,
                    single_apps: Vec::new(),
                    selected_apps_items: Vec::new(),
                    // VIDEO REMOVED: monitors removed
                    recorder: None,
                    monitor_handle: None,
                    active_monitor: None,
                    system_available,
                    saving_dialog: HWND(0),
                    save_cancel: None,
                };

                // VIDEO REMOVED: populate_monitors removed
                apply_settings_to_ui(&mut state, &settings);
                ensure_system_audio_for_application_capture(&state);
                update_source_controls(&state);
                update_format_controls(&state);
                update_filename_preview(&state);
                update_recording_controls(&state);
                update_status_text(&state, RecorderStatus::Idle);

                let boxed = Box::new(state);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(boxed) as isize);

                if SetTimer(hwnd, PODCAST_TIMER_ID, 500, None) == 0 {
                    crate::log_debug("Failed to set PODCAST_TIMER");
                }
                SetFocus(include_mic);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PodcastState;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let state = &mut *ptr;
                SetFocus(state.include_mic);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let code = (wparam.0 >> 16) as u16;
                let mut handled = false;
                if with_podcast_state(hwnd, |state| match id {
                    PODCAST_ID_INCLUDE_MIC
                    | PODCAST_ID_INCLUDE_SYSTEM
                    | PODCAST_ID_INCLUDE_VIDEO => {
                        if id == PODCAST_ID_INCLUDE_SYSTEM && !is_checked(state.include_system) {
                            crate::log_if_err!(KillTimer(hwnd, PODCAST_SYSTEM_TEST_TIMER_ID));
                            SendMessageW(
                                state.test_single_app_audio,
                                BM_SETCHECK,
                                WPARAM(BST_UNCHECKED.0 as usize),
                                LPARAM(0),
                            );
                            if matches!(
                                state.active_monitor,
                                Some(
                                    ActiveMonitorKind::SingleApp
                                        | ActiveMonitorKind::SystemAudioTest
                                )
                            ) {
                                stop_active_monitor(state);
                            }
                        }
                        if id == PODCAST_ID_INCLUDE_MIC {
                            ensure_system_audio_for_application_capture(state);
                        }
                        update_source_controls(state);
                        update_recording_controls(state);
                        update_filename_preview(state);
                        persist_settings(state);
                        // Stop monitor if mic is disabled
                        if !is_checked(state.include_mic)
                            && state.active_monitor == Some(ActiveMonitorKind::Microphone)
                        {
                            stop_active_monitor(state);
                            SendMessageW(
                                state.monitor_check,
                                BM_SETCHECK,
                                WPARAM(BST_UNCHECKED.0 as usize),
                                LPARAM(0),
                            );
                        }
                        handled = true;
                    }
                    PODCAST_ID_SPLIT_SOURCES => {
                        update_filename_preview(state);
                        persist_settings(state);
                        handled = true;
                    }
                    PODCAST_ID_SYSTEM_CAPTURE_MODE => {
                        if code == CBN_SELCHANGE as u16 {
                            crate::log_if_err!(KillTimer(hwnd, PODCAST_SYSTEM_TEST_TIMER_ID));
                            stop_active_monitor(state);
                            SendMessageW(
                                state.test_single_app_audio,
                                BM_SETCHECK,
                                WPARAM(BST_UNCHECKED.0 as usize),
                                LPARAM(0),
                            );
                            ensure_system_audio_for_application_capture(state);
                            update_source_controls(state);
                            update_recording_controls(state);
                            update_filename_preview(state);
                            persist_settings(state);
                        }
                        handled = true;
                    }
                    PODCAST_ID_MONITOR_CHECK => {
                        let checked = is_checked(state.monitor_check);
                        if checked {
                            SendMessageW(
                                state.test_single_app_audio,
                                BM_SETCHECK,
                                WPARAM(BST_UNCHECKED.0 as usize),
                                LPARAM(0),
                            );
                            restart_mic_monitor(state);
                        } else if state.active_monitor == Some(ActiveMonitorKind::Microphone) {
                            stop_active_monitor(state);
                        }
                        handled = true;
                    }
                    PODCAST_ID_TEST_SINGLE_APP_AUDIO => {
                        let checked = is_checked(state.test_single_app_audio);
                        if checked {
                            crate::log_if_err!(KillTimer(hwnd, PODCAST_SYSTEM_TEST_TIMER_ID));
                            SendMessageW(
                                state.monitor_check,
                                BM_SETCHECK,
                                WPARAM(BST_UNCHECKED.0 as usize),
                                LPARAM(0),
                            );
                            restart_single_app_monitor(state);
                            if state.active_monitor == Some(ActiveMonitorKind::SystemAudioTest)
                                && SetTimer(
                                    hwnd,
                                    PODCAST_SYSTEM_TEST_TIMER_ID,
                                    PODCAST_SYSTEM_TEST_DURATION_MS,
                                    None,
                                ) == 0
                            {
                                crate::log_debug("Failed to set PODCAST_SYSTEM_TEST_TIMER");
                            }
                        } else if matches!(
                            state.active_monitor,
                            Some(ActiveMonitorKind::SingleApp | ActiveMonitorKind::SystemAudioTest)
                        ) {
                            crate::log_if_err!(KillTimer(hwnd, PODCAST_SYSTEM_TEST_TIMER_ID));
                            stop_active_monitor(state);
                        }
                        handled = true;
                    }
                    PODCAST_ID_MIC_DEVICE => {
                        if code == CBN_SELCHANGE as u16 {
                            persist_settings(state);
                            if state.active_monitor == Some(ActiveMonitorKind::Microphone) {
                                restart_mic_monitor(state);
                            }
                        }
                        handled = true;
                    }
                    PODCAST_ID_MIC_GAIN => {
                        if code == CBN_SELCHANGE as u16 {
                            persist_settings(state);
                            if state.active_monitor == Some(ActiveMonitorKind::Microphone) {
                                restart_mic_monitor(state);
                            }
                        }
                        handled = true;
                    }
                    PODCAST_ID_SYSTEM_DEVICE | PODCAST_ID_SYSTEM_GAIN | PODCAST_ID_MONITOR => {
                        if code == CBN_SELCHANGE as u16 {
                            persist_settings(state);
                            if matches!(
                                state.active_monitor,
                                Some(
                                    ActiveMonitorKind::SingleApp
                                        | ActiveMonitorKind::SystemAudioTest
                                )
                            ) {
                                restart_single_app_monitor(state);
                            }
                        }
                        handled = true;
                    }
                    PODCAST_ID_SINGLE_APP => {
                        if code == CBN_SELCHANGE as u16 {
                            ensure_system_audio_for_application_capture(state);
                            update_source_controls(state);
                            update_recording_controls(state);
                            update_filename_preview(state);
                            persist_settings(state);
                            if state.active_monitor == Some(ActiveMonitorKind::SingleApp) {
                                restart_single_app_monitor(state);
                            }
                            update_active_single_app_recording(state);
                        }
                        handled = true;
                    }
                    PODCAST_ID_SELECTED_APPS => {
                        if code == LBN_SELCHANGE as u16 {
                            ensure_system_audio_for_application_capture(state);
                            update_source_controls(state);
                            update_recording_controls(state);
                            update_filename_preview(state);
                            persist_settings(state);
                            if state.active_monitor == Some(ActiveMonitorKind::SingleApp) {
                                restart_single_app_monitor(state);
                            }
                        }
                        handled = true;
                    }
                    PODCAST_ID_SHOW_INACTIVE_APPS => {
                        let preferred_pid = selected_single_app(state).map(|app| app.pid);
                        let preferred_selected_pids = selected_app_pids(state);
                        refresh_single_app_list(state, preferred_pid, &preferred_selected_pids);
                        persist_settings(state);
                        if state.active_monitor == Some(ActiveMonitorKind::SingleApp) {
                            restart_single_app_monitor(state);
                        }
                        handled = true;
                    }
                    PODCAST_ID_REFRESH_SINGLE_APP => {
                        let preferred_pid = selected_single_app(state).map(|app| app.pid);
                        let preferred_selected_pids = selected_app_pids(state);
                        refresh_single_app_list(state, preferred_pid, &preferred_selected_pids);
                        persist_settings(state);
                        if state.active_monitor == Some(ActiveMonitorKind::SingleApp) {
                            restart_single_app_monitor(state);
                        }
                        update_active_single_app_recording(state);
                        handled = true;
                    }
                    PODCAST_ID_FORMAT => {
                        if code == CBN_SELCHANGE as u16 {
                            update_format_controls(state);
                            update_filename_preview(state);
                            persist_settings(state);
                        }
                        handled = true;
                    }
                    PODCAST_ID_BITRATE => {
                        if code == CBN_SELCHANGE as u16 {
                            persist_settings(state);
                        }
                        handled = true;
                    }
                    PODCAST_ID_SAVE_PATH => {
                        if code == EN_CHANGE as u16 {
                            update_filename_preview(state);
                            persist_settings(state);
                        }
                        handled = true;
                    }
                    PODCAST_ID_BROWSE => {
                        if let Some(folder) = browse_for_folder(hwnd, state.language) {
                            let path = folder.to_string_lossy().to_string();
                            let wide = to_wide(&path);
                            if let Err(_e) = SetWindowTextW(state.save_path, PCWSTR(wide.as_ptr()))
                            {
                                crate::log_debug(&format!("Error: {:?}", _e));
                            }
                            update_filename_preview(state);
                            persist_settings(state);
                        }
                        handled = true;
                    }
                    PODCAST_ID_START => {
                        handled = true;
                        start_recording_action(state, hwnd);
                    }
                    PODCAST_ID_PAUSE => {
                        if let Some(recorder) = state.recorder.as_ref() {
                            if recorder.status() == RecorderStatus::Recording {
                                recorder.pause();
                                update_recording_controls(state);
                                update_status_text(state, RecorderStatus::Paused);
                            } else if recorder.status() == RecorderStatus::Paused {
                                recorder.resume();
                                update_recording_controls(state);
                                update_status_text(state, RecorderStatus::Recording);
                            }
                        }
                        handled = true;
                    }
                    PODCAST_ID_RESUME => {
                        if let Some(recorder) = state.recorder.as_ref() {
                            recorder.resume();
                            update_recording_controls(state);
                            update_status_text(state, RecorderStatus::Recording);
                        }
                        handled = true;
                    }
                    PODCAST_ID_STOP => {
                        handled = true;
                        stop_recording_action(state, hwnd);
                    }
                    PODCAST_ID_CLOSE => {
                        SendMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
                        handled = true;
                    }
                    _ => {}
                })
                .is_none()
                {
                    crate::log_debug("Failed to access podcast state");
                }
                if handled {
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_TIMER => {
                if wparam.0 == PODCAST_TIMER_ID {
                    if with_podcast_state(hwnd, |state| {
                        update_status_from_recorder(state);
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access podcast state");
                    }
                    return LRESULT(0);
                }
                if wparam.0 == PODCAST_SYSTEM_TEST_TIMER_ID {
                    crate::log_if_err!(KillTimer(hwnd, PODCAST_SYSTEM_TEST_TIMER_ID));
                    if with_podcast_state(hwnd, |state| {
                        crate::send_message_w_safe(
                            state.test_single_app_audio,
                            BM_SETCHECK,
                            WPARAM(BST_UNCHECKED.0 as usize),
                            LPARAM(0),
                        );
                        if state.active_monitor == Some(ActiveMonitorKind::SystemAudioTest) {
                            stop_active_monitor(state);
                        }
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access podcast state");
                    }
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            podcast_save_window::WM_PODCAST_SAVE_CLOSED => {
                if with_podcast_state(hwnd, |state| {
                    state.saving_dialog = HWND(0);
                    state.save_cancel = None;
                    if with_state(state.parent, |app| {
                        app.podcast_save_window = HWND(0);
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access podcast state");
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access podcast state");
                }
                LRESULT(0)
            }
            podcast_save_window::WM_PODCAST_SAVE_CANCEL => {
                if with_podcast_state(hwnd, |state| {
                    if let Some(cancel) = state.save_cancel.as_ref() {
                        cancel.store(true, Ordering::Relaxed);
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access podcast state");
                }
                LRESULT(0)
            }
            WM_PODCAST_SAVE_RESULT => {
                if lparam.0 == 0 {
                    return LRESULT(0);
                }
                let result = Box::from_raw(lparam.0 as *mut PodcastSaveResult);
                if with_podcast_state(hwnd, |state| {
                    if result.success && state.saving_dialog.0 != 0 {
                        podcast_save_window::set_status_text(state.saving_dialog, &result.message);
                    }
                    let title = if result.success {
                        i18n::tr(state.language, "podcast.done_title")
                    } else {
                        crate::settings::error_title(state.language)
                    };
                    let title_w = to_wide(&title);
                    let msg_w = to_wide(&result.message);
                    if result.success {
                        let open_label =
                            i18n::tr(state.language, "podcasts.save_confirm_open_folder");
                        let prompt = format!("{}\n\n{}?", result.message, open_label);
                        let prompt_w = to_wide(&prompt);
                        let response = MessageBoxW(
                            hwnd,
                            PCWSTR(prompt_w.as_ptr()),
                            PCWSTR(title_w.as_ptr()),
                            MB_YESNO | MB_ICONINFORMATION,
                        );
                        if response == IDYES
                            && let Some(folder) = result.saved_folder.as_ref()
                        {
                            let folder_w = to_wide(&folder.to_string_lossy());
                            let verb_w = to_wide("open");
                            let open_result = ShellExecuteW(
                                hwnd,
                                PCWSTR(verb_w.as_ptr()),
                                PCWSTR(folder_w.as_ptr()),
                                PCWSTR::null(),
                                PCWSTR::null(),
                                SW_SHOWNORMAL,
                            );
                            if open_result.0 as isize <= 32 {
                                crate::log_debug(&format!(
                                    "Podcast recording: failed to open save folder path={} code={}",
                                    folder.to_string_lossy(),
                                    open_result.0
                                ));
                            }
                        }
                    } else {
                        MessageBoxW(
                            hwnd,
                            PCWSTR(msg_w.as_ptr()),
                            PCWSTR(title_w.as_ptr()),
                            MB_OK | MB_ICONERROR,
                        );
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access podcast state");
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let mut should_close = true;
                if with_podcast_state(hwnd, |state| {
                    if let Some(recorder) = state.recorder.as_ref()
                        && matches!(
                            recorder.status(),
                            RecorderStatus::Recording | RecorderStatus::Paused
                        )
                    {
                        let labels = labels(state.language);
                        let text = to_wide(&labels.confirm_close_recording);
                        let title = to_wide(&confirm_title(state.language));
                        let result = windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                            hwnd,
                            PCWSTR(text.as_ptr()),
                            PCWSTR(title.as_ptr()),
                            MB_OKCANCEL | MB_ICONWARNING,
                        );
                        if result == IDOK {
                            stop_recording_action(state, hwnd);
                        } else {
                            should_close = false;
                        }
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access podcast state");
                }
                if should_close {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                if with_podcast_state(hwnd, |state| {
                    crate::log_if_err!(KillTimer(hwnd, PODCAST_SYSTEM_TEST_TIMER_ID));
                    stop_active_monitor(state);
                    if let Some(recorder) = state.recorder.take()
                        && let Err(e) = recorder.stop()
                    {
                        crate::log_debug(&format!("Failed to stop recorder: {}", e));
                    }
                    if state.saving_dialog.0 != 0 {
                        crate::log_if_err!(DestroyWindow(state.saving_dialog));
                        state.saving_dialog = HWND(0);
                        state.save_cancel = None;
                        if with_state(state.parent, |app| {
                            app.podcast_save_window = HWND(0);
                        })
                        .is_none()
                        {
                            crate::log_debug("Failed to access podcast state");
                        }
                    }
                    if let Err(e) =
                        PostMessageW(state.parent, crate::WM_FOCUS_EDITOR, WPARAM(0), LPARAM(0))
                    {
                        crate::log_debug(&format!("Failed to post WM_FOCUS_EDITOR: {}", e));
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access podcast state");
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let parent = with_podcast_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
                if parent.0 != 0
                    && with_state(parent, |state| {
                        state.podcast_window = HWND(0);
                    })
                    .is_none()
                {
                    crate::log_debug("Failed to access podcast state");
                }
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                if ptr != 0 {
                    let _unused_box = Box::from_raw(ptr as *mut PodcastState);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn with_podcast_state<T>(hwnd: HWND, f: impl FnOnce(&mut PodcastState) -> T) -> Option<T> {
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut PodcastState;
    if ptr.is_null() {
        return None;
    }
    crate::with_raw_mut_ptr_safe(ptr, f)
}

pub(crate) fn language_for_window(hwnd: HWND) -> Option<Language> {
    with_podcast_state(hwnd, |state| state.language)
}
fn populate_combos(
    format_combo: HWND,
    bitrate_combo: HWND,
    mic_controls: (HWND, HWND),
    system_controls: (HWND, HWND),
    system_capture_mode_combo: HWND,
    language: Language,
) {
    unsafe {
        let (mic_combo, mic_gain_combo) = mic_controls;
        let (system_combo, system_gain_combo) = system_controls;
        SendMessageW(format_combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        let mp3 = to_wide("MP3");
        let wav = to_wide("WAV");
        SendMessageW(
            format_combo,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(mp3.as_ptr() as isize),
        );
        SendMessageW(
            format_combo,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wav.as_ptr() as isize),
        );

        SendMessageW(bitrate_combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        for bitrate in PODCAST_MP3_BITRATES {
            let text = to_wide(&format!("{bitrate} kbps"));
            SendMessageW(
                bitrate_combo,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(text.as_ptr() as isize),
            );
        }

        // Populate microphone gain combo with Italian text
        SendMessageW(mic_gain_combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        let gain_options = [
            "podcast.gain.quarter",
            "podcast.gain.third",
            "podcast.gain.half",
            "podcast.gain.three_quarters",
            "podcast.gain.normal",
            "podcast.gain.one_half",
            "podcast.gain.double",
            "podcast.gain.triple",
            "podcast.gain.quadruple",
        ];
        for key in gain_options {
            let text = to_wide(&i18n::tr(language, key));
            SendMessageW(
                mic_gain_combo,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(text.as_ptr() as isize),
            );
        }

        // Populate system gain combo with Italian text
        SendMessageW(system_gain_combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        for key in gain_options {
            let text = to_wide(&i18n::tr(language, key));
            SendMessageW(
                system_gain_combo,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(text.as_ptr() as isize),
            );
        }

        SendMessageW(mic_combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        SendMessageW(system_combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        SendMessageW(
            system_capture_mode_combo,
            CB_RESETCONTENT,
            WPARAM(0),
            LPARAM(0),
        );
        for text in [
            labels(language).system_capture_mode_all_system,
            labels(language).system_capture_mode_single_app,
            labels(language).system_capture_mode_selected_apps,
        ] {
            let wide = to_wide(&text);
            SendMessageW(
                system_capture_mode_combo,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(wide.as_ptr() as isize),
            );
        }
        // Note: devices are added later in apply_settings_to_ui from mic_devices/system_devices
        // which already include "Default" as the first entry
    }
}

fn capture_mode_to_index(mode: PodcastSystemCaptureMode) -> usize {
    match mode {
        PodcastSystemCaptureMode::AllSystem => 0,
        PodcastSystemCaptureMode::SingleApp => 1,
        PodcastSystemCaptureMode::SelectedApps => 2,
    }
}

fn selected_capture_mode(state: &PodcastState) -> PodcastSystemCaptureMode {
    let sel = crate::send_message_w_safe(
        state.system_capture_mode,
        CB_GETCURSEL,
        WPARAM(0),
        LPARAM(0),
    )
    .0;
    match sel {
        1 => PodcastSystemCaptureMode::SingleApp,
        2 => PodcastSystemCaptureMode::SelectedApps,
        _ => PodcastSystemCaptureMode::AllSystem,
    }
}

fn load_devices(language: Language) -> (Vec<AudioDevice>, Vec<AudioDevice>, bool) {
    let mut mic_devices = vec![AudioDevice {
        id: PODCAST_DEVICE_DEFAULT.to_string(),
        name: labels(language).default_device,
    }];
    if let Ok(list) = list_input_devices() {
        mic_devices.extend(list);
    }
    let mut system_devices = vec![AudioDevice {
        id: PODCAST_DEVICE_DEFAULT.to_string(),
        name: labels(language).default_device,
    }];
    let mut system_available = false;
    if let Ok(list) = list_output_devices() {
        system_available = !list.is_empty();
        system_devices.extend(list);
    }
    (mic_devices, system_devices, system_available)
}

// VIDEO REMOVED: load_monitors function removed

fn load_single_apps(language: Language, include_inactive: bool) -> Vec<AudioApp> {
    let labels = labels(language);
    let mut apps = vec![AudioApp {
        pid: 0,
        display_name: labels.single_app_default,
    }];
    match list_audio_apps(include_inactive) {
        Ok(list) if !list.is_empty() => apps.extend(list),
        Ok(_) | Err(_) => apps.push(AudioApp {
            pid: 0,
            display_name: labels.single_app_none_running,
        }),
    }
    apps
}

fn apply_settings_to_ui(state: &mut PodcastState, settings: &AppSettings) {
    unsafe {
        SendMessageW(
            state.monitor_check,
            BM_SETCHECK,
            WPARAM(BST_UNCHECKED.0 as usize),
            LPARAM(0),
        );
        SendMessageW(
            state.test_single_app_audio,
            BM_SETCHECK,
            WPARAM(BST_UNCHECKED.0 as usize),
            LPARAM(0),
        );
        SendMessageW(
            state.include_mic,
            BM_SETCHECK,
            WPARAM(if settings.podcast_include_microphone {
                BST_CHECKED.0 as usize
            } else {
                BST_UNCHECKED.0 as usize
            }),
            LPARAM(0),
        );
        SendMessageW(
            state.include_system,
            BM_SETCHECK,
            WPARAM(if settings.podcast_include_system_audio {
                BST_CHECKED.0 as usize
            } else {
                BST_UNCHECKED.0 as usize
            }),
            LPARAM(0),
        );
        SendMessageW(
            state.show_inactive_apps,
            BM_SETCHECK,
            WPARAM(if settings.podcast_show_inactive_apps {
                BST_CHECKED.0 as usize
            } else {
                BST_UNCHECKED.0 as usize
            }),
            LPARAM(0),
        );
        SendMessageW(
            state.system_capture_mode,
            CB_SETCURSEL,
            WPARAM(capture_mode_to_index(settings.podcast_system_capture_mode)),
            LPARAM(0),
        );

        let displays = crate::result_labels::accessible_result_labels(
            state
                .mic_devices
                .iter()
                .map(|device| device.name.clone())
                .collect(),
        );
        for ((index, device), display) in state.mic_devices.iter().enumerate().zip(displays) {
            let name = to_wide(&display);
            SendMessageW(
                state.mic_device,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(name.as_ptr() as isize),
            );
            if device.id == settings.podcast_microphone_device_id {
                SendMessageW(state.mic_device, CB_SETCURSEL, WPARAM(index), LPARAM(0));
            }
        }
        if SendMessageW(state.mic_device, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 == -1 {
            SendMessageW(state.mic_device, CB_SETCURSEL, WPARAM(0), LPARAM(0));
        }

        // Set mic gain
        let mic_gain_index = gain_to_index(settings.podcast_microphone_gain);
        SendMessageW(
            state.mic_gain,
            CB_SETCURSEL,
            WPARAM(mic_gain_index),
            LPARAM(0),
        );

        // VIDEO REMOVED: include_video and monitor setup removed

        SendMessageW(
            state.split_sources,
            BM_SETCHECK,
            WPARAM(if settings.podcast_split_sources {
                BST_CHECKED.0 as usize
            } else {
                BST_UNCHECKED.0 as usize
            }),
            LPARAM(0),
        );

        let displays = crate::result_labels::accessible_result_labels(
            state
                .system_devices
                .iter()
                .map(|device| device.name.clone())
                .collect(),
        );
        for ((index, device), display) in state.system_devices.iter().enumerate().zip(displays) {
            let name = to_wide(&display);
            SendMessageW(
                state.system_device,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(name.as_ptr() as isize),
            );
            if device.id == settings.podcast_system_device_id {
                SendMessageW(state.system_device, CB_SETCURSEL, WPARAM(index), LPARAM(0));
            }
        }
        if SendMessageW(state.system_device, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 == -1 {
            SendMessageW(state.system_device, CB_SETCURSEL, WPARAM(0), LPARAM(0));
        }

        // Set system gain
        let system_gain_index = gain_to_index(settings.podcast_system_gain);
        SendMessageW(
            state.system_gain,
            CB_SETCURSEL,
            WPARAM(system_gain_index),
            LPARAM(0),
        );
        refresh_single_app_list(
            state,
            Some(settings.podcast_single_app_pid),
            &settings.podcast_selected_app_pids,
        );

        let format_index = match settings.podcast_output_format {
            PodcastFormat::Mp3 => 0,
            PodcastFormat::Wav => 1,
        };
        SendMessageW(
            state.format_combo,
            CB_SETCURSEL,
            WPARAM(format_index),
            LPARAM(0),
        );

        let bitrate_index = PODCAST_MP3_BITRATES
            .iter()
            .position(|&b| b == settings.podcast_mp3_bitrate)
            .unwrap_or(2);
        SendMessageW(
            state.bitrate_combo,
            CB_SETCURSEL,
            WPARAM(bitrate_index),
            LPARAM(0),
        );

        let path = if settings.podcast_save_folder.trim().is_empty() {
            default_podcast_save_folder()
        } else {
            settings.podcast_save_folder.clone()
        };
        let path_w = to_wide(&path);
        if let Err(_e) = SetWindowTextW(state.save_path, PCWSTR(path_w.as_ptr())) {
            crate::log_debug(&format!("Error: {:?}", _e));
        }
    }
}

fn ensure_system_audio_for_application_capture(state: &PodcastState) {
    let capture_mode = selected_capture_mode(state);
    if is_checked(state.include_mic)
        && matches!(
            capture_mode,
            PodcastSystemCaptureMode::SingleApp | PodcastSystemCaptureMode::SelectedApps
        )
        && !is_checked(state.include_system)
    {
        unsafe {
            SendMessageW(
                state.include_system,
                BM_SETCHECK,
                WPARAM(BST_CHECKED.0 as usize),
                LPARAM(0),
            );
        }
    }
}

fn split_sources_available(state: &PodcastState) -> bool {
    is_checked(state.include_mic) && is_checked(state.include_system)
}

fn update_source_controls(state: &PodcastState) {
    unsafe {
        let mic_checked = is_checked(state.include_mic);
        let system_checked = is_checked(state.include_system);
        let capture_mode = selected_capture_mode(state);
        let single_app_mode = capture_mode == PodcastSystemCaptureMode::SingleApp;
        let selected_apps_mode = capture_mode == PodcastSystemCaptureMode::SelectedApps;
        let labels = labels(state.language);
        let split_available = split_sources_available(state);
        if !split_available && is_checked(state.split_sources) {
            SendMessageW(
                state.split_sources,
                BM_SETCHECK,
                WPARAM(BST_UNCHECKED.0 as usize),
                LPARAM(0),
            );
        }
        EnableWindow(state.split_sources, split_available);
        // VIDEO REMOVED: video_checked removed
        EnableWindow(state.mic_device, mic_checked);
        EnableWindow(state.mic_gain, mic_checked);
        EnableWindow(state.monitor_check, mic_checked);
        EnableWindow(
            state.system_device,
            system_checked && matches!(capture_mode, PodcastSystemCaptureMode::AllSystem),
        );
        EnableWindow(state.system_gain, system_checked);
        EnableWindow(
            state.system_capture_mode,
            system_checked && state.system_available,
        );
        EnableWindow(
            state.single_app,
            system_checked && state.system_available && single_app_mode,
        );
        EnableWindow(
            state.refresh_single_app,
            system_checked
                && state.system_available
                && !matches!(capture_mode, PodcastSystemCaptureMode::AllSystem),
        );
        EnableWindow(
            state.test_single_app_audio,
            system_checked && state.system_available,
        );
        EnableWindow(
            state.show_inactive_apps,
            system_checked
                && state.system_available
                && !matches!(capture_mode, PodcastSystemCaptureMode::AllSystem),
        );
        EnableWindow(
            state.selected_apps,
            system_checked && state.system_available && selected_apps_mode,
        );
        // VIDEO REMOVED: monitor_combo removed

        ShowWindow(
            state.label_single_app,
            if system_checked && single_app_mode {
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW
            } else {
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE
            },
        );
        ShowWindow(
            state.single_app,
            if system_checked && single_app_mode {
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW
            } else {
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE
            },
        );
        ShowWindow(
            state.label_selected_apps,
            if system_checked && selected_apps_mode {
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW
            } else {
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE
            },
        );
        ShowWindow(
            state.selected_apps,
            if system_checked && selected_apps_mode {
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW
            } else {
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE
            },
        );
        ShowWindow(
            state.show_inactive_apps,
            if system_checked && !matches!(capture_mode, PodcastSystemCaptureMode::AllSystem) {
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW
            } else {
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE
            },
        );
        ShowWindow(
            state.test_single_app_audio,
            if system_checked {
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW
            } else {
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE
            },
        );
        ShowWindow(
            state.refresh_single_app,
            if system_checked && !matches!(capture_mode, PodcastSystemCaptureMode::AllSystem) {
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW
            } else {
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE
            },
        );
        let test_label = if matches!(capture_mode, PodcastSystemCaptureMode::AllSystem) {
            &labels.test_system_audio
        } else if selected_apps_mode {
            &labels.test_selected_apps_audio
        } else {
            &labels.test_single_app_audio
        };
        let test_label_w = to_wide(test_label);
        if let Err(_e) = SetWindowTextW(state.test_single_app_audio, PCWSTR(test_label_w.as_ptr()))
        {
            crate::log_debug(&format!("Error: {:?}", _e));
        }
        let refresh_label = if selected_apps_mode {
            &labels.selected_apps_refresh
        } else {
            &labels.single_app_refresh
        };
        let refresh_label_w = to_wide(refresh_label);
        if let Err(_e) = SetWindowTextW(state.refresh_single_app, PCWSTR(refresh_label_w.as_ptr()))
        {
            crate::log_debug(&format!("Error: {:?}", _e));
        }

        if !state.system_available {
            EnableWindow(state.include_system, false);
            EnableWindow(state.system_device, false);
            EnableWindow(state.system_capture_mode, false);
            EnableWindow(state.single_app, false);
            EnableWindow(state.selected_apps, false);
            EnableWindow(state.show_inactive_apps, false);
            EnableWindow(state.refresh_single_app, false);
            EnableWindow(state.test_single_app_audio, false);
            ShowWindow(
                state.system_unavailable_text,
                windows::Win32::UI::WindowsAndMessaging::SW_SHOW,
            );
        } else {
            ShowWindow(
                state.system_unavailable_text,
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
            );
        }

        // VIDEO REMOVED: video availability check removed

        let hint = if !mic_checked && !system_checked {
            windows::Win32::UI::WindowsAndMessaging::SW_SHOW
        } else {
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE
        };
        ShowWindow(state.hint_text, hint);
    }
}

// VIDEO REMOVED: populate_monitors function removed

fn update_format_controls(state: &PodcastState) {
    unsafe {
        let format = selected_format(state);
        EnableWindow(state.bitrate_combo, format == PodcastFormat::Mp3);
    }
}

fn update_recording_controls(state: &PodcastState) {
    unsafe {
        let labels = labels(state.language);
        let has_sources = is_checked(state.include_mic) || is_checked(state.include_system);
        let status = state
            .recorder
            .as_ref()
            .map(|recorder| recorder.status())
            .unwrap_or(RecorderStatus::Idle);
        let recording = matches!(status, RecorderStatus::Recording);
        let paused = matches!(status, RecorderStatus::Paused);
        let pause_label = if paused { labels.resume } else { labels.pause };
        let pause_wide = to_wide(&pause_label);

        EnableWindow(
            state.split_sources,
            split_sources_available(state) && !recording && !paused,
        );
        EnableWindow(state.start_button, has_sources && !recording && !paused);
        EnableWindow(state.pause_button, recording || paused);
        if let Err(_e) = SetWindowTextW(state.pause_button, PCWSTR(pause_wide.as_ptr())) {
            crate::log_debug(&format!("Error: {:?}", _e));
        }
        EnableWindow(state.resume_button, false);
        ShowWindow(
            state.resume_button,
            windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
        );
        EnableWindow(state.stop_button, recording || paused);
    }
}

fn update_active_single_app_recording(state: &mut PodcastState) {
    if state.recorder.is_none()
        || !is_checked(state.include_system)
        || !matches!(
            selected_capture_mode(state),
            PodcastSystemCaptureMode::SingleApp
        )
    {
        return;
    }
    let labels = labels(state.language);
    let Some(app) = selected_single_app(state) else {
        show_error(
            state.parent,
            state.language,
            &labels.error_single_app_required,
        );
        return;
    };
    if let Err(err) = probe_process_loopback(app.pid) {
        show_error(
            state.parent,
            state.language,
            &format!("{} {}", labels.error_single_app_unavailable, err),
        );
        return;
    }
    let update_result = state
        .recorder
        .as_ref()
        .map(|recorder| recorder.update_single_app_process(app.pid));
    match update_result {
        Some(Ok(())) => {
            let mic_name = if is_checked(state.include_mic) {
                Some(device_display_name(
                    &state.mic_devices,
                    &selected_device_id(state, true),
                    &labels.default_device,
                ))
            } else {
                None
            };
            update_source_info_text(state, mic_name, None, Some(app.display_name), None);
        }
        Some(Err(err)) => {
            crate::log_debug(&format!(
                "Podcast recorder: failed to update single-app capture: {}",
                err
            ));
            show_error(state.parent, state.language, &err);
        }
        None => {}
    }
}

fn update_status_text(state: &PodcastState, status: RecorderStatus) {
    let labels = labels(state.language);
    let text = match status {
        RecorderStatus::Idle => labels.status_idle,
        RecorderStatus::Recording => labels.status_recording,
        RecorderStatus::Paused => labels.status_paused,
        RecorderStatus::Saving => labels.status_saving,
        RecorderStatus::Error => labels.status_error,
    };
    let wide = to_wide(&text);
    unsafe {
        if let Err(_e) = SetWindowTextW(state.status_text, PCWSTR(wide.as_ptr())) {
            crate::log_debug(&format!("Error: {:?}", _e));
        }
    }
}

fn update_source_info_text(
    state: &PodcastState,
    mic_name: Option<String>,
    system_name: Option<String>,
    single_app_name: Option<String>,
    selected_apps_names: Option<Vec<String>>,
) {
    let labels = labels(state.language);
    let mut parts = Vec::new();
    if let Some(mic) = mic_name {
        parts.push(format!("{}: {}", labels.mic_device, mic));
    }
    if let Some(system) = system_name {
        parts.push(format!("{}: {}", labels.system_device, system));
    }
    if let Some(single_app) = single_app_name {
        parts.push(format!("{}: {}", labels.single_app, single_app));
    }
    if let Some(selected_apps) = selected_apps_names
        && !selected_apps.is_empty()
    {
        parts.push(format!(
            "{}: {}",
            labels.selected_apps,
            selected_apps.join(", ")
        ));
    }
    let text = if parts.is_empty() {
        labels.hint_select_source
    } else {
        parts.join("  ")
    };
    let wide = to_wide(&text);
    unsafe {
        if let Err(_e) = SetWindowTextW(state.source_text, PCWSTR(wide.as_ptr())) {
            crate::log_debug(&format!("Error: {:?}", _e));
        }
    }
}

fn update_status_from_recorder(state: &mut PodcastState) {
    if let Some(recorder) = state.recorder.as_ref() {
        let status = recorder.status();
        update_status_text(state, status);
        let elapsed = recorder.elapsed();
        let total_secs = elapsed.as_secs();
        let hours = total_secs / 3600;
        let mins = (total_secs / 60) % 60;
        let secs = total_secs % 60;
        let time_text = format!("{:02}:{:02}:{:02}", hours, mins, secs);
        let time_w = to_wide(&time_text);
        unsafe {
            if let Err(_e) = SetWindowTextW(state.elapsed_text, PCWSTR(time_w.as_ptr())) {
                crate::log_debug(&format!("Error: {:?}", _e));
            }
        }
        let levels = recorder.levels();
        let mic_text = levels.mic_peak.to_string();
        let sys_text = levels.system_peak.to_string();
        unsafe {
            let mic_w = to_wide(&mic_text);
            let sys_w = to_wide(&sys_text);
            if let Err(_e) = SetWindowTextW(state.level_mic_text, PCWSTR(mic_w.as_ptr())) {
                crate::log_debug(&format!("Error: {:?}", _e));
            }
            if let Err(_e) = SetWindowTextW(state.level_system_text, PCWSTR(sys_w.as_ptr())) {
                crate::log_debug(&format!("Error: {:?}", _e));
            }
        }
        if let Some(err) = recorder.take_error() {
            show_error(state.parent, state.language, &err);
        }
    } else {
        update_status_text(state, RecorderStatus::Idle);
    }
}

fn update_filename_preview(state: &PodcastState) {
    let format = selected_format(state);
    let ext = match format {
        PodcastFormat::Mp3 => "mp3",
        PodcastFormat::Wav => "wav",
    };
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let name = if is_checked(state.split_sources) && split_sources_available(state) {
        format!("Podcast_{timestamp}_microphone.{ext}; Podcast_{timestamp}_system_audio.{ext}")
    } else {
        format!("Podcast_{timestamp}.{ext}")
    };
    let wide = to_wide(&name);
    unsafe {
        if let Err(_e) = SetWindowTextW(state.filename_preview, PCWSTR(wide.as_ptr())) {
            crate::log_debug(&format!("Error: {:?}", _e));
        }
    }
}

fn restart_mic_monitor(state: &mut PodcastState) {
    if !is_checked(state.monitor_check) {
        if state.active_monitor == Some(ActiveMonitorKind::Microphone) {
            stop_active_monitor(state);
        }
        return;
    }
    stop_active_monitor(state);
    let device_id = selected_device_id(state, true);
    let device_name = selected_device_name(state, true);
    let gain = selected_mic_gain(state);
    log_debug(&format!(
        "Podcast monitor: mic device_id='{}' name='{}' gain={}",
        device_id, device_name, gain
    ));
    match start_monitoring(device_id, device_name, gain) {
        Ok(handle) => {
            state.monitor_handle = Some(handle);
            state.active_monitor = Some(ActiveMonitorKind::Microphone);
        }
        Err(e) => {
            crate::send_message_w_safe(
                state.monitor_check,
                BM_SETCHECK,
                WPARAM(BST_UNCHECKED.0 as usize),
                LPARAM(0),
            );
            show_error(state.parent, state.language, &e);
        }
    }
}

fn restart_single_app_monitor(state: &mut PodcastState) {
    if !is_checked(state.test_single_app_audio) {
        if state.active_monitor == Some(ActiveMonitorKind::SingleApp) {
            stop_active_monitor(state);
        }
        return;
    }
    let labels = labels(state.language);
    stop_active_monitor(state);
    let gain = selected_system_gain(state);
    let capture_mode = selected_capture_mode(state);
    let monitor_result = match capture_mode {
        PodcastSystemCaptureMode::SingleApp => {
            let Some(app) = selected_single_app(state) else {
                crate::send_message_w_safe(
                    state.test_single_app_audio,
                    BM_SETCHECK,
                    WPARAM(BST_UNCHECKED.0 as usize),
                    LPARAM(0),
                );
                show_error(
                    state.parent,
                    state.language,
                    &labels.error_single_app_required,
                );
                return;
            };
            log_debug(&format!(
                "Podcast monitor: single app pid='{}' name='{}' gain={}",
                app.pid, app.display_name, gain
            ));
            start_process_monitoring(app.pid, gain)
                .map_err(|e| format!("{} {}", labels.single_app_audio_unavailable, e))
        }
        PodcastSystemCaptureMode::SelectedApps => {
            let process_ids = selected_app_pids(state);
            if process_ids.is_empty() {
                crate::send_message_w_safe(
                    state.test_single_app_audio,
                    BM_SETCHECK,
                    WPARAM(BST_UNCHECKED.0 as usize),
                    LPARAM(0),
                );
                show_error(
                    state.parent,
                    state.language,
                    &labels.error_selected_apps_required,
                );
                return;
            }
            log_debug(&format!(
                "Podcast monitor: selected apps count='{}' gain={}",
                process_ids.len(),
                gain
            ));
            start_processes_monitoring(&process_ids, gain)
                .map_err(|e| format!("{} {}", labels.selected_apps_audio_unavailable, e))
        }
        PodcastSystemCaptureMode::AllSystem => {
            let device_id = selected_device_id(state, false);
            let device_name = selected_device_name(state, false);
            log_debug(&format!(
                "Podcast monitor: system loopback device_id='{}' name='{}' gain={}",
                device_id, device_name, gain
            ));
            start_system_monitoring(device_id, device_name, gain)
                .map_err(|e| format!("{} {}", labels.error_system_audio, e))
        }
    };
    match monitor_result {
        Ok(handle) => {
            state.monitor_handle = Some(handle);
            state.active_monitor = Some(match capture_mode {
                PodcastSystemCaptureMode::AllSystem => ActiveMonitorKind::SystemAudioTest,
                _ => ActiveMonitorKind::SingleApp,
            });
        }
        Err(e) => {
            crate::send_message_w_safe(
                state.test_single_app_audio,
                BM_SETCHECK,
                WPARAM(BST_UNCHECKED.0 as usize),
                LPARAM(0),
            );
            show_error(state.parent, state.language, &e);
        }
    }
}

fn stop_active_monitor(state: &mut PodcastState) {
    if let Some(handle) = state.monitor_handle.take() {
        handle.stop();
    }
    state.active_monitor = None;
}

fn start_recording_action(state: &mut PodcastState, _hwnd: HWND) {
    if state.recorder.is_some() {
        return;
    }
    let labels = labels(state.language);
    let include_mic = is_checked(state.include_mic);
    let include_system = is_checked(state.include_system);
    let capture_mode = selected_capture_mode(state);
    if !include_mic && !include_system {
        return;
    }

    // Stop microphone monitor if active to avoid conflicts
    stop_active_monitor(state);
    crate::send_message_w_safe(
        state.monitor_check,
        BM_SETCHECK,
        WPARAM(BST_UNCHECKED.0 as usize),
        LPARAM(0),
    );
    crate::send_message_w_safe(
        state.test_single_app_audio,
        BM_SETCHECK,
        WPARAM(BST_UNCHECKED.0 as usize),
        LPARAM(0),
    );

    let mic_device_id = selected_device_id(state, true);
    let system_device_id = selected_device_id(state, false);
    let mic_device_name = selected_device_name(state, true);
    let system_device_name = selected_device_name(state, false);
    let selected_single_app = selected_single_app(state);
    let selected_app_process_ids = selected_app_pids(state);
    match capture_mode {
        PodcastSystemCaptureMode::SingleApp if include_system && selected_single_app.is_none() => {
            show_error(
                state.parent,
                state.language,
                &labels.error_single_app_required,
            );
            return;
        }
        PodcastSystemCaptureMode::SelectedApps
            if include_system && selected_app_process_ids.is_empty() =>
        {
            show_error(
                state.parent,
                state.language,
                &labels.error_selected_apps_required,
            );
            return;
        }
        _ => {}
    }
    log_debug(&format!(
        "Podcast record: mic device_id='{}' name='{}' gain={} system device_id='{}' name='{}' gain={}",
        mic_device_id,
        mic_device_name,
        selected_mic_gain(state),
        system_device_id,
        system_device_name,
        selected_system_gain(state)
    ));
    if include_mic
        && let Err(err) =
            probe_device_with_name(&mic_device_id, &selected_device_name(state, true), false)
    {
        show_error(
            state.parent,
            state.language,
            &format!("{} {}", labels.error_microphone, err),
        );
        if include_system {
            crate::send_message_w_safe(
                state.include_mic,
                BM_SETCHECK,
                WPARAM(BST_UNCHECKED.0 as usize),
                LPARAM(0),
            );
        }
    }
    if include_system
        && matches!(capture_mode, PodcastSystemCaptureMode::AllSystem)
        && let Err(err) =
            probe_device_with_name(&system_device_id, &selected_device_name(state, false), true)
    {
        show_error(
            state.parent,
            state.language,
            &format!("{} {}", labels.error_system_audio, err),
        );
        if include_mic {
            crate::send_message_w_safe(
                state.include_system,
                BM_SETCHECK,
                WPARAM(BST_UNCHECKED.0 as usize),
                LPARAM(0),
            );
        }
    }
    if include_system
        && matches!(capture_mode, PodcastSystemCaptureMode::SingleApp)
        && let Some(app) = selected_single_app.as_ref()
        && let Err(err) = probe_process_loopback(app.pid)
    {
        show_error(
            state.parent,
            state.language,
            &format!("{} {}", labels.error_single_app_unavailable, err),
        );
        return;
    }
    if include_system && matches!(capture_mode, PodcastSystemCaptureMode::SelectedApps) {
        for process_id in &selected_app_process_ids {
            if let Err(err) = probe_process_loopback(*process_id) {
                show_error(
                    state.parent,
                    state.language,
                    &format!("{} {}", labels.error_selected_apps_unavailable, err),
                );
                return;
            }
        }
    }

    let include_system = is_checked(state.include_system);
    let include_mic = is_checked(state.include_mic);
    update_source_controls(state);
    update_filename_preview(state);
    if !include_mic && !include_system {
        update_recording_controls(state);
        update_source_controls(state);
        return;
    }

    let default_device_label = labels.default_device.clone();
    let config = RecorderConfig {
        include_mic,
        mic_device_id: selected_device_id(state, true),
        mic_device_name: selected_device_name(state, true),
        mic_gain: selected_mic_gain(state),
        include_system,
        split_mic_system: is_checked(state.split_sources) && split_sources_available(state),
        system_device_id: selected_device_id(state, false),
        system_device_name: selected_device_name(state, false),
        system_gain: selected_system_gain(state),
        single_app_process_id: selected_single_app.as_ref().map(|app| app.pid),
        selected_app_process_ids: selected_app_process_ids.clone(),
        output_format: selected_format(state),
        mp3_bitrate: selected_bitrate(state),
        save_folder: selected_save_folder(state),
    };
    match start_recording(config) {
        Ok(recorder) => {
            state.recorder = Some(recorder);
            let mic_name =
                device_display_name(&state.mic_devices, &mic_device_id, &default_device_label);
            let system_name = device_display_name(
                &state.system_devices,
                &system_device_id,
                &default_device_label,
            );
            update_source_info_text(
                state,
                Some(mic_name),
                if include_system && matches!(capture_mode, PodcastSystemCaptureMode::AllSystem) {
                    Some(system_name)
                } else {
                    None
                },
                if matches!(capture_mode, PodcastSystemCaptureMode::SingleApp) {
                    selected_single_app.map(|app| app.display_name)
                } else {
                    None
                },
                if matches!(capture_mode, PodcastSystemCaptureMode::SelectedApps) {
                    Some(selected_apps_display_names(state))
                } else {
                    None
                },
            );
            update_recording_controls(state);
            update_status_text(state, RecorderStatus::Recording);
            unsafe {
                SetFocus(state.pause_button);
            }
        }
        Err(err) => {
            show_error(state.parent, state.language, &err);
            update_recording_controls(state);
        }
    }
}

fn stop_recording_action(state: &mut PodcastState, hwnd: HWND) {
    if state.recorder.is_none() {
        return;
    }
    if state.saving_dialog.0 == 0 {
        let dialog = podcast_save_window::open(hwnd);
        if dialog.0 != 0 {
            state.saving_dialog = dialog;
            {
                if with_state(state.parent, |app| {
                    app.podcast_save_window = dialog;
                })
                .is_none()
                {
                    crate::log_debug("Failed to access podcast state");
                }
            }
        }
    }
    let cancel_flag = Arc::new(AtomicBool::new(false));
    state.save_cancel = Some(cancel_flag.clone());
    let saved_folder = selected_save_folder(state);
    if let Some(recorder) = state.recorder.take() {
        update_status_text(state, RecorderStatus::Saving);
        let language = state.language;
        let dialog = state.saving_dialog;
        let cancel = cancel_flag;
        std::thread::spawn(move || {
            let result = recorder.stop_with_progress(
                |pct| {
                    if dialog.0 != 0 {
                        unsafe {
                            if let Err(_e) = PostMessageW(
                                dialog,
                                podcast_save_window::WM_PODCAST_SAVE_PROGRESS,
                                WPARAM(pct as usize),
                                LPARAM(0),
                            ) {}
                        }
                    }
                },
                Some(cancel.clone()),
            );
            let cancelled = cancel.load(Ordering::Relaxed);
            let mut notify = None;
            if let Err(err) = result {
                if !(cancelled && err == "Saving canceled.") {
                    notify = Some(PodcastSaveResult {
                        success: false,
                        message: err,
                        saved_folder: None,
                    });
                }
            } else {
                notify = Some(PodcastSaveResult {
                    success: true,
                    message: i18n::tr(language, "podcast.saved"),
                    saved_folder: Some(saved_folder),
                });
            }
            if let Some(payload) = notify {
                unsafe {
                    if let Err(e) = PostMessageW(
                        hwnd,
                        WM_PODCAST_SAVE_RESULT,
                        WPARAM(0),
                        LPARAM(Box::into_raw(Box::new(payload)) as isize),
                    ) {
                        crate::log_debug(&format!("Failed to post WM_PODCAST_SAVE_RESULT: {}", e));
                    }
                };
            }
            if dialog.0 != 0 {
                unsafe {
                    if let Err(_e) = PostMessageW(
                        dialog,
                        podcast_save_window::WM_PODCAST_SAVE_DONE,
                        WPARAM(0),
                        LPARAM(0),
                    ) {
                        crate::log_debug(&format!("Error: {:?}", _e));
                    }
                }
            }
        });
    }
    update_recording_controls(state);
}

fn persist_settings(state: &PodcastState) {
    let include_mic = is_checked(state.include_mic);
    let include_system = is_checked(state.include_system);
    let split_sources = is_checked(state.split_sources) && split_sources_available(state);
    let mic_device_id = selected_device_id(state, true);
    let mic_gain = selected_mic_gain(state);
    let system_device_id = selected_device_id(state, false);
    let system_gain = selected_system_gain(state);
    let system_capture_mode = if include_system {
        selected_capture_mode(state)
    } else {
        PodcastSystemCaptureMode::AllSystem
    };
    let show_inactive_apps = is_checked(state.show_inactive_apps);
    let single_app_pid = selected_single_app(state).map(|app| app.pid).unwrap_or(0);
    let selected_app_pids = selected_app_pids(state);
    let output_format = selected_format(state);
    let bitrate = selected_bitrate(state);
    let save_folder = selected_save_folder(state).to_string_lossy().to_string();
    {
        if with_state(state.parent, |app| {
            app.settings.podcast_include_microphone = include_mic;
            app.settings.podcast_microphone_device_id = mic_device_id;
            app.settings.podcast_microphone_gain = mic_gain;
            app.settings.podcast_include_system_audio = include_system;
            app.settings.podcast_split_sources = split_sources;
            app.settings.podcast_system_device_id = system_device_id;
            app.settings.podcast_system_gain = system_gain;
            app.settings.podcast_system_capture_mode = system_capture_mode;
            app.settings.podcast_include_single_app = include_system
                && matches!(system_capture_mode, PodcastSystemCaptureMode::SingleApp);
            app.settings.podcast_single_app_pid = single_app_pid;
            app.settings.podcast_selected_app_pids = selected_app_pids;
            app.settings.podcast_show_inactive_apps = show_inactive_apps;
            app.settings.podcast_output_format = output_format;
            app.settings.podcast_mp3_bitrate = bitrate;
            app.settings.podcast_save_folder = save_folder;
            save_settings(app.settings.clone());
        })
        .is_none()
        {
            crate::log_debug("Failed to access podcast state");
        }
    }
    update_source_info_text(state, None, None, None, None);
}

// VIDEO REMOVED: selected_monitor_id function removed

fn selected_format(state: &PodcastState) -> PodcastFormat {
    let sel = crate::send_message_w_safe(state.format_combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel == 1 {
        PodcastFormat::Wav
    } else {
        PodcastFormat::Mp3
    }
}

fn selected_bitrate(state: &PodcastState) -> u32 {
    let sel = crate::send_message_w_safe(state.bitrate_combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    PODCAST_MP3_BITRATES
        .get(sel.max(0) as usize)
        .copied()
        .unwrap_or(128)
}

fn selected_mic_gain(state: &PodcastState) -> f32 {
    selected_gain(state.mic_gain)
}

fn selected_system_gain(state: &PodcastState) -> f32 {
    selected_gain(state.system_gain)
}

fn selected_gain(combo: HWND) -> f32 {
    let sel = crate::send_message_w_safe(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    match sel {
        0 => 0.25, // Un quarto del volume
        1 => 0.33, // Un terzo del volume
        2 => 0.5,  // Metà del volume
        3 => 0.75, // Tre quarti del volume
        4 => 1.0,  // Volume normale
        5 => 1.5,  // Una volta e mezza
        6 => 2.0,  // Il doppio del volume
        7 => 3.0,  // Il triplo del volume
        8 => 4.0,  // Il quadruplo del volume
        _ => 1.0,  // Default: Volume normale
    }
}

fn gain_to_index(gain: f32) -> usize {
    if gain <= 0.25 {
        0 // Un quarto del volume
    } else if gain <= 0.33 {
        1 // Un terzo del volume
    } else if gain <= 0.5 {
        2 // Metà del volume
    } else if gain <= 0.75 {
        3 // Tre quarti del volume
    } else if gain <= 1.0 {
        4 // Volume normale
    } else if gain <= 1.5 {
        5 // Una volta e mezza
    } else if gain <= 2.0 {
        6 // Il doppio del volume
    } else if gain <= 3.0 {
        7 // Il triplo del volume
    } else {
        8 // Il quadruplo del volume
    }
}

fn selected_device_id(state: &PodcastState, mic: bool) -> String {
    let combo = if mic {
        state.mic_device
    } else {
        state.system_device
    };
    let list = if mic {
        &state.mic_devices
    } else {
        &state.system_devices
    };
    let sel = crate::send_message_w_safe(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    let index = if sel < 0 { 0 } else { sel as usize };
    list.get(index)
        .map(|d| d.id.clone())
        .unwrap_or_else(|| PODCAST_DEVICE_DEFAULT.to_string())
}

fn device_display_name(devices: &[AudioDevice], device_id: &str, fallback: &str) -> String {
    devices
        .iter()
        .find(|d| d.id == device_id)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| fallback.to_string())
}

fn selected_device_name(state: &PodcastState, mic: bool) -> String {
    let list = if mic {
        &state.mic_devices
    } else {
        &state.system_devices
    };
    let id = selected_device_id(state, mic);
    list.iter()
        .find(|d| d.id == id)
        .map(|d| d.name.clone())
        .unwrap_or_default()
}

fn selected_single_app(state: &PodcastState) -> Option<AudioApp> {
    let sel = crate::send_message_w_safe(state.single_app, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    let index = if sel < 0 { 0 } else { sel as usize };
    let app = state.single_apps.get(index)?.clone();
    if app.pid == 0 { None } else { Some(app) }
}

fn selected_app_pids(state: &PodcastState) -> Vec<u32> {
    let count =
        crate::send_message_w_safe(state.selected_apps, LB_GETCOUNT, WPARAM(0), LPARAM(0)).0;
    let mut process_ids = Vec::new();
    for idx in 0..count {
        let is_selected = crate::send_message_w_safe(
            state.selected_apps,
            LB_GETSEL,
            WPARAM(idx as usize),
            LPARAM(0),
        )
        .0;
        if is_selected > 0
            && let Some(app) = state.selected_apps_items.get(idx as usize)
            && app.pid != 0
        {
            process_ids.push(app.pid);
        }
    }
    process_ids
}

fn selected_apps_display_names(state: &PodcastState) -> Vec<String> {
    let count =
        crate::send_message_w_safe(state.selected_apps, LB_GETCOUNT, WPARAM(0), LPARAM(0)).0;
    let mut names = Vec::new();
    for idx in 0..count {
        let is_selected = crate::send_message_w_safe(
            state.selected_apps,
            LB_GETSEL,
            WPARAM(idx as usize),
            LPARAM(0),
        )
        .0;
        if is_selected > 0
            && let Some(app) = state.selected_apps_items.get(idx as usize)
            && app.pid != 0
        {
            names.push(app.display_name.clone());
        }
    }
    names
}

fn refresh_single_app_list(
    state: &mut PodcastState,
    preferred_pid: Option<u32>,
    preferred_selected_pids: &[u32],
) {
    unsafe {
        SendMessageW(state.single_app, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        SendMessageW(state.selected_apps, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    }
    state.single_apps = load_single_apps(state.language, is_checked(state.show_inactive_apps));
    state.selected_apps_items = state
        .single_apps
        .iter()
        .filter(|app| app.pid != 0)
        .cloned()
        .collect();
    let mut selected_index = 0usize;
    let displays = crate::result_labels::accessible_result_labels(
        state
            .single_apps
            .iter()
            .map(|app| app.display_name.clone())
            .collect(),
    );
    for ((index, app), display) in state.single_apps.iter().enumerate().zip(displays) {
        let name = to_wide(&display);
        unsafe {
            SendMessageW(
                state.single_app,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(name.as_ptr() as isize),
            );
        }
        if Some(app.pid) == preferred_pid {
            selected_index = index;
        }
    }
    for (index, app) in state.selected_apps_items.iter().enumerate() {
        let name = to_wide(&app.display_name);
        unsafe {
            SendMessageW(
                state.selected_apps,
                LB_ADDSTRING,
                WPARAM(0),
                LPARAM(name.as_ptr() as isize),
            );
            if preferred_selected_pids.contains(&app.pid) {
                SendMessageW(
                    state.selected_apps,
                    LB_SETSEL,
                    WPARAM(1),
                    LPARAM(index as isize),
                );
            }
        }
    }
    unsafe {
        SendMessageW(
            state.single_app,
            CB_SETCURSEL,
            WPARAM(selected_index),
            LPARAM(0),
        );
    }
}

fn selected_save_folder(state: &PodcastState) -> PathBuf {
    unsafe {
        let len = GetWindowTextLengthW(state.save_path) as usize;
        let mut buf = vec![0u16; len + 1];
        let read = GetWindowTextW(state.save_path, &mut buf);
        let text = String::from_utf16_lossy(&buf[..read as usize]);
        if text.trim().is_empty() {
            default_output_folder()
        } else {
            PathBuf::from(text)
        }
    }
}

fn is_checked(hwnd: HWND) -> bool {
    crate::send_message_w_safe(hwnd, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == BST_CHECKED.0 as isize
}

fn click_button(hwnd: HWND, id: usize) {
    unsafe {
        let button = GetDlgItem(hwnd, id as i32);
        if button.0 != 0 {
            SendMessageW(button, BM_CLICK, WPARAM(0), LPARAM(0));
        }
    }
}

fn browse_for_folder(owner: HWND, language: Language) -> Option<PathBuf> {
    crate::app_windows::find_in_files_window::browse_for_folder(owner, language)
}
