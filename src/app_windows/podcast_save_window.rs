use crate::accessibility::{ES_READONLY, handle_accessibility, to_wide};
use crate::i18n;
use crate::settings::Language;
use crate::with_state;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{PBM_SETPOS, PBM_SETRANGE, WC_BUTTON, WC_EDIT};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, SetFocus, VK_ESCAPE, VK_RETURN, VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    ES_AUTOHSCROLL, GWLP_USERDATA, GetForegroundWindow, GetParent, HMENU, IDC_ARROW, IDYES,
    IsChild, LoadCursorW, MB_ICONWARNING, MB_YESNO, MSG, PostMessageW, RegisterClassW,
    SendMessageW, SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowTextW, WINDOW_STYLE,
    WM_ACTIVATE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_KEYDOWN, WM_NCDESTROY, WM_SETFOCUS,
    WM_SETFONT, WM_SYSKEYDOWN, WM_TIMER, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD,
    WS_EX_DLGMODALFRAME, WS_POPUP, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

pub const WM_PODCAST_SAVE_DONE: u32 = WM_APP + 70;
pub const WM_PODCAST_SAVE_CLOSED: u32 = WM_APP + 71;
pub const WM_PODCAST_SAVE_PROGRESS: u32 = WM_APP + 72;
pub const WM_PODCAST_SAVE_CANCEL: u32 = WM_APP + 73;
const SAVE_CLASS_NAME: &str = "SonarpadPodcastSave";
const SAVE_ID_CANCEL: usize = 12002;
const SAVE_PROGRESS_TIMER_ID: usize = 1;
const SAVE_PROGRESS_TICK_MS: u32 = 250;
const SAVE_PROGRESS_MAX_FAKE: usize = 95;
const SAVE_PROGRESS_FAKE_DELAY: Duration = Duration::from_secs(5);
const STATIC_CENTER_STYLE: u32 = 0x0000_0001;

pub struct SaveDialogLabels {
    pub title: String,
    pub in_progress: String,
    pub cancel: String,
    pub cancel_confirm: String,
}

struct SaveCreateParams {
    parent: HWND,
    language: Language,
    labels: SaveDialogLabels,
    show_cancel: bool,
    show_status_field: bool,
    disable_parent: bool,
}

struct SaveState {
    parent: HWND,
    label: HWND,
    status_field: HWND,
    progress: HWND,
    cancel_button: HWND,
    cancel_requested: bool,
    language: Language,
    status_text: String,
    current_pct: usize,
    has_real_progress: bool,
    // No made-up percentage before an external installer reports real progress.
    show_percentage: bool,
    opened_at: Instant,
    labels: SaveDialogLabels,
    show_cancel: bool,
    suppress_parent_restore: bool,
    disable_parent: bool,
    accessible_progress_context: bool,
    // Keep the dialog caption stable while the accessible status field changes.
    static_window_title: bool,
    // Only the explicit Japanese OCR installer and MP3 re-export use this mode.
    // Do not let IsDialogMessageW move Tab focus into an underlying editor.
    cancel_only_tab_navigation: bool,
}

fn save_labels(language: Language) -> SaveDialogLabels {
    SaveDialogLabels {
        title: i18n::tr(language, "podcast.save.title"),
        in_progress: i18n::tr(language, "podcast.save.in_progress"),
        cancel: i18n::tr(language, "podcast.save.cancel"),
        cancel_confirm: i18n::tr(language, "podcast.cancel_confirm"),
    }
}

pub fn handle_navigation(hwnd: HWND, msg: &MSG) -> bool {
    // IsDialogMessageW is too permissive here: on some NVDA/Windows setups it
    // exposes focusable controls belonging to the disabled owner window.
    // Consume Tab BEFORE the generic dialog handler, in both directions.
    if msg.message == WM_KEYDOWN
        && msg.wParam.0 as u32 == VK_TAB.0 as u32
        && (msg.hwnd == hwnd || crate::is_child_safe(hwnd, msg.hwnd))
        && with_save_state(hwnd, |state| state.cancel_only_tab_navigation).unwrap_or(false)
    {
        focus_cancel_button(hwnd);
        return true;
    }
    if msg.message == WM_KEYDOWN || msg.message == WM_SYSKEYDOWN {
        let key = msg.wParam.0 as u32;
        if key == VK_ESCAPE.0 as u32 {
            if let Err(_e) =
                crate::post_message_w_safe(hwnd, WM_COMMAND, WPARAM(SAVE_ID_CANCEL), LPARAM(0))
            {
                crate::log_debug(&format!("Error: {:?}", _e));
            }
            return true;
        }
        if key == VK_RETURN.0 as u32 {
            let focus = crate::get_focus_safe();
            let cancel = with_save_state(hwnd, |state| state.cancel_button).unwrap_or(HWND(0));
            if cancel.0 != 0 && focus == cancel {
                if let Err(_e) = crate::post_message_w_safe(
                    hwnd,
                    WM_COMMAND,
                    WPARAM(SAVE_ID_CANCEL),
                    LPARAM(cancel.0),
                ) {
                    crate::log_debug(&format!("Error: {:?}", _e));
                }
                return true;
            }
        }
    }
    handle_accessibility(hwnd, msg)
}

pub fn focus_cancel_button(hwnd: HWND) {
    crate::log_debug(&format!(
        "podcast_save_window focus_cancel_button: hwnd={:?}",
        hwnd
    ));
    if with_save_state(hwnd, |state| {
        if state.cancel_button.0 != 0 {
            crate::log_debug(&format!(
                "podcast_save_window focusing cancel button: dialog={:?} cancel={:?}",
                hwnd, state.cancel_button
            ));
            crate::set_focus_safe(state.cancel_button);
        } else {
            crate::log_debug(&format!(
                "podcast_save_window focusing dialog fallback: dialog={:?} label={:?}",
                hwnd, state.label
            ));
            crate::set_focus_safe(hwnd);
        }
    })
    .is_none()
    {
        crate::log_debug("Failed to access save state for focus");
    }
}

/// The installer and MP3 re-export are waiting dialogs, not forms. Tab and
/// Shift+Tab must stay on Cancel; status is a STATIC label and a native progress
/// bar, never a keyboard-editable status field.
pub fn restrict_tab_to_cancel(hwnd: HWND) {
    if with_save_state(hwnd, |state| {
        // Both callers use open_with_labels (show_status_field=false).
        // Fail closed if a future change accidentally adds another tab stop:
        // the message loop will keep focusing Cancel instead of that control.
        state.cancel_only_tab_navigation = true;
        if state.status_field.0 != 0 {
            crate::log_debug(
                "Progress cancel-only mode: unexpected status Edit field (Tab is blocked)",
            );
        }
        if state.cancel_button.0 != 0 {
            crate::set_focus_safe(state.cancel_button);
        }
    })
    .is_none()
    {
        crate::log_debug("Failed to enable cancel-only Tab navigation");
    }
}

pub fn focus_primary_control(hwnd: HWND) {
    if with_save_state(hwnd, |state| {
        if state.status_field.0 != 0 {
            crate::log_debug(&format!(
                "podcast_save_window focusing accessible status: dialog={:?} status={:?}",
                hwnd, state.status_field
            ));
            refresh_accessible_status_field(state);
            crate::set_focus_safe(state.status_field);
        } else if state.cancel_button.0 != 0 {
            crate::set_focus_safe(state.cancel_button);
        } else {
            crate::set_focus_safe(hwnd);
        }
    })
    .is_none()
    {
        crate::log_debug("Failed to access save state for primary focus");
    }
}

/// Do not append the current status to this window title. The separate status
/// field continues to receive the same updates and the Cancel button is unchanged.
pub fn keep_window_title_static(hwnd: HWND) {
    if with_save_state(hwnd, |state| {
        state.static_window_title = true;
        unsafe {
            if let Err(err) = SetWindowTextW(hwnd, PCWSTR(to_wide(&state.labels.title).as_ptr())) {
                crate::log_debug(&format!(
                    "Failed to keep progress window title static: {}",
                    err
                ));
            }
        }
    })
    .is_none()
    {
        crate::log_debug("Failed to keep progress window title static");
    }
}

pub fn enable_accessible_progress_context(hwnd: HWND) {
    if with_save_state(hwnd, |state| {
        state.accessible_progress_context = true;
        refresh_accessible_status_field(state);
        if !state.static_window_title {
            let title = format!("{} - {}", state.labels.title, state.status_text);
            unsafe {
                if let Err(err) = SetWindowTextW(hwnd, PCWSTR(to_wide(&title).as_ptr())) {
                    crate::log_debug(&format!("Failed to set accessible progress title: {}", err));
                }
            }
        }
    })
    .is_none()
    {
        crate::log_debug("Failed to enable accessible progress context");
    }
}

pub fn suppress_parent_restore_on_close(hwnd: HWND) {
    if with_save_state(hwnd, |state| {
        state.suppress_parent_restore = true;
    })
    .is_none()
    {
        crate::log_debug("Failed to set suppress_parent_restore on save window");
    }
}

/// Keep the progress dialog accessible while the native installer has not yet
/// reported a real percentage. Prevents the generic timer from inventing 1..95%.
pub fn show_indeterminate_progress(hwnd: HWND) {
    if with_save_state(hwnd, |state| {
        state.has_real_progress = true;
        state.show_percentage = false;
        state.current_pct = 0;
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                GWL_STYLE, GetWindowLongPtrW, SetWindowLongPtrW,
            };
            let style = GetWindowLongPtrW(state.progress, GWL_STYLE);
            SetWindowLongPtrW(state.progress, GWL_STYLE, style | 8); // PBS_MARQUEE
        }
        crate::send_message_w_safe(
            state.progress,
            windows::Win32::UI::Controls::PBM_SETMARQUEE,
            WPARAM(1),
            LPARAM(40),
        );

        crate::send_message_w_safe(state.progress, PBM_SETPOS, WPARAM(0), LPARAM(0));
        update_progress_label(state);
        if state.accessible_progress_context {
            refresh_accessible_status_field(state);
        }
    })
    .is_none()
    {
        crate::log_debug("Failed to switch installation progress to indeterminate");
    }
}

pub fn disable_fake_progress(hwnd: HWND) {
    if with_save_state(hwnd, |state| {
        state.has_real_progress = true;
        state.current_pct = 0;
        crate::send_message_w_safe(state.progress, PBM_SETPOS, WPARAM(0), LPARAM(0));
        update_progress_label(state);
    })
    .is_none()
    {
        crate::log_debug("Failed to disable fake progress on save window");
    }
}

pub fn open(parent: HWND) -> HWND {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(SAVE_CLASS_NAME);
        let main = GetParent(parent);
        let language = if main.0 != 0 {
            with_state(main, |state| state.settings.language).unwrap_or_default()
        } else {
            crate::app_windows::podcast_window::language_for_window(parent).unwrap_or_default()
        };
        let labels = save_labels(language);

        let wc = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
            ),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(save_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let params = Box::new(SaveCreateParams {
            parent,
            language,
            labels,
            show_cancel: true,
            show_status_field: false,
            disable_parent: true,
        });
        let params_ptr = Box::into_raw(params);
        let window = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_POPUP | WS_CAPTION | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            300,
            150,
            parent,
            HMENU(0),
            hinstance,
            Some(params_ptr as *const std::ffi::c_void),
        );
        if window.0 == 0 {
            let _unused = Box::from_raw(params_ptr);
            return window;
        }

        if window.0 != 0 {
            EnableWindow(parent, false);
            SetForegroundWindow(window);

            let mut rc_parent = RECT::default();
            let mut rc_dlg = RECT::default();
            crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::GetWindowRect(
                parent,
                &mut rc_parent
            ));
            crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::GetWindowRect(
                window,
                &mut rc_dlg
            ));
            let dlg_w = rc_dlg.right - rc_dlg.left;
            let dlg_h = rc_dlg.bottom - rc_dlg.top;
            let parent_w = rc_parent.right - rc_parent.left;
            let parent_h = rc_parent.bottom - rc_parent.top;
            let x = rc_parent.left + (parent_w - dlg_w) / 2;
            let y = rc_parent.top + (parent_h - dlg_h) / 2;
            use windows::Win32::UI::WindowsAndMessaging::{HWND_TOP, SWP_SHOWWINDOW, SetWindowPos};
            if let Err(e) = SetWindowPos(window, HWND_TOP, x, y, dlg_w, dlg_h, SWP_SHOWWINDOW) {
                crate::log_debug(&format!("Failed to position save window: {}", e));
            }
        }
        window
    }
}

pub fn open_with_labels(
    parent: HWND,
    language: Language,
    labels: SaveDialogLabels,
    show_cancel: bool,
) -> HWND {
    open_with_labels_and_status_field(parent, language, labels, show_cancel, false)
}

pub fn open_with_labels_and_status_field(
    parent: HWND,
    language: Language,
    labels: SaveDialogLabels,
    show_cancel: bool,
    show_status_field: bool,
) -> HWND {
    open_with_labels_and_status_field_parent_mode(
        parent,
        language,
        labels,
        show_cancel,
        show_status_field,
        true,
    )
}

pub fn open_with_labels_and_status_field_parent_mode(
    parent: HWND,
    language: Language,
    labels: SaveDialogLabels,
    show_cancel: bool,
    show_status_field: bool,
    disable_parent: bool,
) -> HWND {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(SAVE_CLASS_NAME);
        let wc = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
            ),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(save_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let params = Box::new(SaveCreateParams {
            parent,
            language,
            labels,
            show_cancel,
            show_status_field,
            disable_parent,
        });
        let params_ptr = Box::into_raw(params);
        let window = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_POPUP | WS_CAPTION | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            300,
            if show_status_field { 180 } else { 150 },
            parent,
            HMENU(0),
            hinstance,
            Some(params_ptr as *const std::ffi::c_void),
        );
        if window.0 == 0 {
            let _unused = Box::from_raw(params_ptr);
            return window;
        }

        if disable_parent {
            EnableWindow(parent, false);
        }
        SetForegroundWindow(window);

        let mut rc_parent = RECT::default();
        let mut rc_dlg = RECT::default();
        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::GetWindowRect(
            parent,
            &mut rc_parent
        ));
        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::GetWindowRect(
            window,
            &mut rc_dlg
        ));
        let dlg_w = rc_dlg.right - rc_dlg.left;
        let dlg_h = rc_dlg.bottom - rc_dlg.top;
        let parent_w = rc_parent.right - rc_parent.left;
        let parent_h = rc_parent.bottom - rc_parent.top;
        let x = rc_parent.left + (parent_w - dlg_w) / 2;
        let y = rc_parent.top + (parent_h - dlg_h) / 2;
        use windows::Win32::UI::WindowsAndMessaging::{HWND_TOP, SWP_SHOWWINDOW, SetWindowPos};
        if let Err(e) = SetWindowPos(window, HWND_TOP, x, y, dlg_w, dlg_h, SWP_SHOWWINDOW) {
            crate::log_debug(&format!("Failed to position save window: {}", e));
        }

        window
    }
}

unsafe extern "system" fn save_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "save_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || save_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn save_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create_struct = lparam.0 as *const CREATESTRUCTW;
            let params_ptr = unsafe { (*create_struct).lpCreateParams as *mut SaveCreateParams };
            let params = crate::box_from_raw_safe(params_ptr);
            let parent = params.parent;
            let language = params.language;
            let labels = params.labels;
            let show_cancel = params.show_cancel;
            let show_status_field = params.show_status_field;
            let disable_parent = params.disable_parent;
            let main = crate::get_parent_safe(parent);
            let hfont = { with_state(main, |state| state.hfont) }.unwrap_or(HFONT(0));
            let label_text = format!("{} 0%", labels.in_progress);

            let label = unsafe {
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&label_text).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WINDOW_STYLE(STATIC_CENTER_STYLE),
                    20,
                    20,
                    260,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                )
            };

            let progress = unsafe {
                CreateWindowExW(
                    Default::default(),
                    w!("msctls_progress32"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE,
                    20,
                    if show_status_field { 80 } else { 50 },
                    260,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                )
            };

            let status_field = if show_status_field {
                unsafe {
                    CreateWindowExW(
                        Default::default(),
                        WC_EDIT,
                        PCWSTR(to_wide(&labels.in_progress).as_ptr()),
                        WS_CHILD
                            | WS_VISIBLE
                            | WS_TABSTOP
                            | WS_BORDER
                            | WINDOW_STYLE((ES_AUTOHSCROLL as u32) | ES_READONLY),
                        20,
                        45,
                        260,
                        24,
                        hwnd,
                        HMENU(0),
                        HINSTANCE(0),
                        None,
                    )
                }
            } else {
                HWND(0)
            };

            let cancel_button = if show_cancel {
                unsafe {
                    CreateWindowExW(
                        Default::default(),
                        WC_BUTTON,
                        PCWSTR(to_wide(&labels.cancel).as_ptr()),
                        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                        95,
                        if show_status_field { 115 } else { 80 },
                        90,
                        28,
                        hwnd,
                        HMENU(SAVE_ID_CANCEL as isize),
                        HINSTANCE(0),
                        None,
                    )
                }
            } else {
                HWND(0)
            };

            if let Err(e) =
                crate::set_window_text_w_safe(hwnd, PCWSTR(to_wide(&labels.title).as_ptr()))
            {
                crate::log_debug(&format!("Failed to set title: {}", e));
            }
            unsafe {
                SendMessageW(label, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                SendMessageW(
                    status_field,
                    WM_SETFONT,
                    WPARAM(hfont.0 as usize),
                    LPARAM(1),
                );
                SendMessageW(progress, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
            }
            if cancel_button.0 != 0 {
                crate::send_message_w_safe(
                    cancel_button,
                    WM_SETFONT,
                    WPARAM(hfont.0 as usize),
                    LPARAM(1),
                );
            }
            unsafe {
                SendMessageW(progress, PBM_SETRANGE, WPARAM(0), LPARAM((100isize) << 16));
                SendMessageW(progress, PBM_SETPOS, WPARAM(0), LPARAM(0));
            }

            let state = SaveState {
                parent,
                label,
                status_field,
                progress,
                cancel_button,
                cancel_requested: false,
                language,
                status_text: labels.in_progress.clone(),
                current_pct: 0,
                has_real_progress: false,
                show_percentage: true,
                opened_at: Instant::now(),
                labels,
                show_cancel,
                suppress_parent_restore: false,
                disable_parent,
                accessible_progress_context: false,
                static_window_title: false,
                cancel_only_tab_navigation: false,
            };
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(Box::new(state)) as isize);
                crate::log_debug(&format!(
                    "podcast_save_window WM_CREATE: hwnd={:?} parent={:?} label={:?} progress={:?} cancel_button={:?} show_cancel={}",
                    hwnd, parent, label, progress, cancel_button, show_cancel
                ));
                if status_field.0 != 0 {
                    SetFocus(status_field);
                } else if cancel_button.0 != 0 {
                    SetFocus(cancel_button);
                } else {
                    SetFocus(hwnd);
                }
            }
            if unsafe { SetTimer(hwnd, SAVE_PROGRESS_TIMER_ID, SAVE_PROGRESS_TICK_MS, None) } == 0 {
                crate::log_debug("Failed to set SAVE_PROGRESS_TIMER");
            }
            LRESULT(0)
        }
        WM_SETFOCUS => {
            crate::log_debug(&format!("podcast_save_window WM_SETFOCUS: hwnd={:?}", hwnd));
            if with_save_state(hwnd, |state| {
                if state.status_field.0 != 0 {
                    if state.accessible_progress_context {
                        refresh_accessible_status_field(state);
                    }
                    crate::set_focus_safe(state.status_field);
                } else if state.cancel_button.0 != 0 {
                    crate::log_debug(&format!(
                        "podcast_save_window WM_SETFOCUS focusing cancel: hwnd={:?} cancel={:?}",
                        hwnd, state.cancel_button
                    ));
                    crate::set_focus_safe(state.cancel_button);
                }
            })
            .is_none()
            {
                crate::log_debug("Failed to access save state");
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            if (wparam.0 & 0xffff) != 0 {
                let should_focus_status = with_save_state(hwnd, |state| {
                    if state.accessible_progress_context && state.status_field.0 != 0 {
                        refresh_accessible_status_field(state);
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false);
                if should_focus_status {
                    focus_primary_control(hwnd);
                }
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_COMMAND => {
            let id = wparam.0 & 0xffff;
            if id == SAVE_ID_CANCEL || id == 2 {
                request_cancel(hwnd);
                return LRESULT(0);
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if wparam.0 as u32 == VK_ESCAPE.0 as u32 {
                request_cancel(hwnd);
                return LRESULT(0);
            }
            if wparam.0 as u32 == VK_TAB.0 as u32
                && let Some((cancel, show_cancel)) =
                    with_save_state(hwnd, |state| (state.cancel_button, state.show_cancel))
                && show_cancel
                && cancel.0 != 0
            {
                let focus = crate::get_focus_safe();
                let status_field =
                    with_save_state(hwnd, |state| state.status_field).unwrap_or(HWND(0));
                let shift_down =
                    (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
                if !shift_down && status_field.0 != 0 && focus == status_field {
                    crate::set_focus_safe(cancel);
                    return LRESULT(0);
                }
                if shift_down && status_field.0 != 0 && focus == cancel {
                    crate::set_focus_safe(status_field);
                    return LRESULT(0);
                }
                if !shift_down && focus != cancel {
                    crate::set_focus_safe(cancel);
                    return LRESULT(0);
                }
                if shift_down && focus == cancel {
                    return LRESULT(0);
                }
            }
            if wparam.0 as u32 == VK_RETURN.0 as u32
                && let Some(cancel) = with_save_state(hwnd, |state| state.cancel_button)
                && cancel.0 != 0
                && crate::get_focus_safe() == cancel
            {
                if let Err(_e) = crate::post_message_w_safe(
                    hwnd,
                    WM_COMMAND,
                    WPARAM(SAVE_ID_CANCEL),
                    LPARAM(cancel.0),
                ) {}
                return LRESULT(0);
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_PODCAST_SAVE_PROGRESS => {
            // Keep the watchdog heartbeat for every posted progress message,
            // including duplicates, so a busy nested message pump stays alive.
            crate::watchdog::heartbeat();
            let pct = wparam.0.min(100);
            let progress_changed = with_save_state(hwnd, |state| {
                if pct == 0 && state.current_pct == 0 && !state.has_real_progress {
                    return false;
                }
                // Avoid redundant logging, native progress updates and accessible
                // label changes for repeated percentages. The first real update
                // must still be applied after fake/timer-based progress.
                if state.has_real_progress && state.show_percentage && state.current_pct == pct {
                    return false;
                }
                if !state.show_percentage {
                    crate::send_message_w_safe(
                        state.progress,
                        windows::Win32::UI::Controls::PBM_SETMARQUEE,
                        WPARAM(0),
                        LPARAM(0),
                    );
                    unsafe {
                        use windows::Win32::UI::WindowsAndMessaging::{
                            GWL_STYLE, GetWindowLongPtrW, SetWindowLongPtrW,
                        };
                        let style = GetWindowLongPtrW(state.progress, GWL_STYLE);
                        SetWindowLongPtrW(state.progress, GWL_STYLE, style & !8);
                    }
                }
                crate::send_message_w_safe(state.progress, PBM_SETPOS, WPARAM(pct), LPARAM(0));
                state.has_real_progress = true;
                state.show_percentage = true;
                state.current_pct = pct;
                update_progress_label(state);
                true
            });
            match progress_changed {
                Some(true) => crate::log_debug(&format!(
                    "podcast_save_window progress message: hwnd={:?} pct={}",
                    hwnd, pct
                )),
                Some(false) => {}
                None => crate::log_debug("Failed to access save state"),
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == SAVE_PROGRESS_TIMER_ID {
                // A processed timer proves the progress UI is still responsive.
                // If the UI thread really hangs, this timer cannot be dispatched
                // and the watchdog will still detect the missing heartbeat.
                crate::watchdog::heartbeat();
                if with_save_state(hwnd, |state| {
                    if !state.has_real_progress
                        && state.current_pct < SAVE_PROGRESS_MAX_FAKE
                        && state.opened_at.elapsed() >= SAVE_PROGRESS_FAKE_DELAY
                    {
                        state.current_pct = (state.current_pct + 1).min(SAVE_PROGRESS_MAX_FAKE);
                        crate::send_message_w_safe(
                            state.progress,
                            PBM_SETPOS,
                            WPARAM(state.current_pct),
                            LPARAM(0),
                        );
                        update_progress_label(state);
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access save state");
                }
                return LRESULT(0);
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_PODCAST_SAVE_DONE => {
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            LRESULT(0)
        }
        WM_CLOSE => {
            request_cancel(hwnd);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let (
                parent,
                show_cancel,
                suppress_parent_restore,
                disable_parent,
                accessible_progress_context,
            ) = with_save_state(hwnd, |state| {
                (
                    state.parent,
                    state.show_cancel,
                    state.suppress_parent_restore,
                    state.disable_parent,
                    state.accessible_progress_context,
                )
            })
            .unwrap_or((HWND(0), false, false, false, false));
            let foreground = unsafe { GetForegroundWindow() };
            let progress_was_foreground = foreground == hwnd
                || (foreground.0 != 0 && unsafe { IsChild(hwnd, foreground).as_bool() });
            crate::log_debug(&format!(
                "podcast_save_window WM_NCDESTROY: hwnd={:?} parent={:?} show_cancel={} suppress_parent_restore={} disable_parent={} accessible_progress_context={} progress_was_foreground={}",
                hwnd,
                parent,
                show_cancel,
                suppress_parent_restore,
                disable_parent,
                accessible_progress_context,
                progress_was_foreground
            ));
            if let Err(e) = crate::kill_timer_safe(hwnd, SAVE_PROGRESS_TIMER_ID) {
                crate::log_debug(&format!("Failed to kill SAVE_PROGRESS_TIMER: {}", e));
            }
            if parent.0 != 0 {
                if disable_parent && !suppress_parent_restore {
                    crate::enable_window_safe(parent, true);
                }
                // For transient probe/progress dialogs without cancel, avoid briefly
                // bouncing focus back to the editor before the next modal opens.
                if disable_parent
                    && show_cancel
                    && !suppress_parent_restore
                    && (!accessible_progress_context || progress_was_foreground)
                {
                    crate::set_foreground_window_safe(parent);
                } else if disable_parent
                    && show_cancel
                    && !suppress_parent_restore
                    && accessible_progress_context
                    && !progress_was_foreground
                {
                    crate::log_debug(
                        "podcast_save_window: respecting external foreground on progress close",
                    );
                }
                if let Err(_e) = crate::post_message_w_safe(
                    parent,
                    WM_PODCAST_SAVE_CLOSED,
                    WPARAM(0),
                    LPARAM(hwnd.0),
                ) {
                    crate::log_debug(&format!("Error: {:?}", _e));
                }
            }
            let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA);
            if ptr != 0 {
                let _unused_box = crate::box_from_raw_safe(ptr as *mut SaveState);
            }
            LRESULT(0)
        }
        _ => crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
    }
}

fn with_save_state<T>(hwnd: HWND, f: impl FnOnce(&mut SaveState) -> T) -> Option<T> {
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut SaveState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn update_progress_label(state: &SaveState) {
    let text = if state.show_percentage {
        format!("{} {}%", state.status_text, state.current_pct)
    } else {
        state.status_text.clone()
    };
    unsafe {
        if let Err(e) = SetWindowTextW(state.label, PCWSTR(to_wide(&text).as_ptr())) {
            crate::log_debug(&format!("Failed to set label text: {}", e));
        }
        if state.status_field.0 != 0
            && !state.accessible_progress_context
            && let Err(e) = SetWindowTextW(
                state.status_field,
                PCWSTR(to_wide(&state.status_text).as_ptr()),
            )
        {
            crate::log_debug(&format!("Failed to set status field text: {}", e));
        }
    }
}

fn refresh_accessible_status_field(state: &SaveState) {
    if state.status_field.0 == 0 {
        return;
    }
    let text = if state.show_percentage {
        format!("{} {}%", state.status_text, state.current_pct)
    } else {
        state.status_text.clone()
    };
    unsafe {
        if let Err(err) = SetWindowTextW(state.status_field, PCWSTR(to_wide(&text).as_ptr())) {
            crate::log_debug(&format!(
                "Failed to refresh accessible progress status: {}",
                err
            ));
        }
    }
}

pub fn set_status_text(hwnd: HWND, text: &str) {
    if with_save_state(hwnd, |state| {
        state.status_text = text.to_string();
        update_progress_label(state);
        if state.accessible_progress_context {
            refresh_accessible_status_field(state);
            if !state.static_window_title {
                let title = format!("{} - {}", state.labels.title, state.status_text);
                unsafe {
                    if let Err(err) = SetWindowTextW(hwnd, PCWSTR(to_wide(&title).as_ptr())) {
                        crate::log_debug(&format!(
                            "Failed to update accessible progress title: {}",
                            err
                        ));
                    }
                }
            }
        }
    })
    .is_none()
    {
        crate::log_debug("Failed to access save state");
    }
}

fn request_cancel(hwnd: HWND) {
    let mut should_post = false;
    if with_save_state(hwnd, |state| {
        if !state.show_cancel || state.cancel_button.0 == 0 {
            return;
        }
        if state.cancel_requested {
            return;
        }
        let msg = state.labels.cancel_confirm.clone();
        let title = i18n::tr(state.language, "app.confirm_title");
        let msg_w = to_wide(&msg);
        let title_w = to_wide(&title);
        let result = crate::message_box_w_safe(
            hwnd,
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_YESNO | MB_ICONWARNING,
        );
        if result == IDYES {
            state.cancel_requested = true;
            unsafe {
                EnableWindow(state.cancel_button, false);
            }
            should_post = true;
        }
    })
    .is_none()
    {
        crate::log_debug("Failed to access save state");
    }
    if should_post {
        let parent = crate::get_parent_safe(hwnd);
        if parent.0 != 0 {
            unsafe {
                if let Err(_e) =
                    PostMessageW(parent, WM_PODCAST_SAVE_CANCEL, WPARAM(0), LPARAM(hwnd.0))
                {
                    crate::log_debug(&format!("Error: {:?}", _e));
                }
            }
        }
    }
}
