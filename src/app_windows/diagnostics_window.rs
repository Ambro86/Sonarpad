use std::sync::{Arc, Mutex};

use crate::{i18n, settings::Language, to_wide};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{PBM_SETMARQUEE, PBS_MARQUEE, PROGRESS_CLASSW};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, IsWindowEnabled, SetFocus, VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const EMAIL: i32 = 107;
const NAME: i32 = 101;
const REPORT: i32 = 102;
const SEND: i32 = 103;
const SAVE: i32 = 104;
const CANCEL: i32 = 2;
const STATUS: i32 = 105;
const TIMER: usize = 1;
const PROGRESS: i32 = 106;

type Completion = Arc<Mutex<Option<Result<(), String>>>>;
struct State {
    language: Language,
    busy: bool,
    sent: bool,
    completion: Completion,
}

fn text(hwnd: HWND, id: i32) -> String {
    unsafe {
        let edit = GetDlgItem(hwnd, id);
        let mut buffer = vec![0u16; GetWindowTextLengthW(edit) as usize + 1];
        let length = GetWindowTextW(edit, &mut buffer);
        String::from_utf16_lossy(&buffer[..length as usize])
    }
}

fn set_status(hwnd: HWND, message: &str) {
    unsafe {
        crate::log_if_err!(crate::set_window_text_w_safe(
            GetDlgItem(hwnd, STATUS),
            PCWSTR(to_wide(message).as_ptr())
        ));
    }
}

fn controls(hwnd: HWND, state: &State) {
    unsafe {
        let progress = GetDlgItem(hwnd, PROGRESS);
        ShowWindow(progress, if state.busy { SW_SHOW } else { SW_HIDE });
        SendMessageW(
            progress,
            PBM_SETMARQUEE,
            WPARAM(usize::from(state.busy)),
            LPARAM(30),
        );
        for id in [NAME, EMAIL, REPORT, SAVE, CANCEL] {
            EnableWindow(GetDlgItem(hwnd, id), !state.busy);
        }
        EnableWindow(
            GetDlgItem(hwnd, SEND),
            !state.busy && !text(hwnd, REPORT).trim().is_empty(),
        );
    }
}

// Multiline EDIT controls request Tab themselves. Handle navigation before
// IsDialogMessage/TranslateMessage so a tab is never inserted in the report.
fn navigate_tab(hwnd: HWND, message: &MSG, backwards: bool) -> bool {
    if message.message != WM_KEYDOWN || message.wParam.0 != VK_TAB.0 as usize {
        return false;
    }
    unsafe {
        if message.hwnd != hwnd && !IsChild(hwnd, message.hwnd).as_bool() {
            return false;
        }
        let controls = [NAME, EMAIL, REPORT, SEND, SAVE, CANCEL].map(|id| GetDlgItem(hwnd, id));
        let current = controls.iter().position(|control| *control == GetFocus());
        let start = current.unwrap_or(if backwards { 0 } else { controls.len() - 1 });
        for offset in 1..=controls.len() {
            let index = if backwards {
                (start + controls.len() - offset) % controls.len()
            } else {
                (start + offset) % controls.len()
            };
            let target = controls[index];
            if target.0 != 0 && IsWindowEnabled(target).as_bool() {
                SetFocus(target);
                break;
            }
        }
    }
    true
}

pub fn open(parent: HWND) {
    unsafe {
        let language = crate::with_state(parent, |app| app.settings.language).unwrap_or_default();
        let instance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = w!("SonarpadDiagnosticsReport");
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            RegisterClassW(&WNDCLASSW {
                hInstance: instance,
                lpszClassName: class_name,
                lpfnWndProc: Some(wndproc),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
                ..Default::default()
            });
        });
        let mut state = State {
            language,
            busy: false,
            sent: false,
            completion: Arc::new(Mutex::new(None)),
        };
        let previous_focus = GetFocus();
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class_name,
            PCWSTR(to_wide(&i18n::tr(language, "diagnostics.send_title")).as_ptr()),
            WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            640,
            576,
            parent,
            HMENU(0),
            instance,
            Some(&mut state as *mut _ as *const std::ffi::c_void),
        );
        if hwnd.0 == 0 {
            return;
        }
        EnableWindow(parent, false);
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(GetDlgItem(hwnd, NAME));
        let mut message = MSG::default();
        while IsWindow(hwnd).as_bool() {
            let result = GetMessageW(&mut message, HWND(0), 0, 0).0;
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(message.wParam.0 as i32);
                }
                if IsWindow(hwnd).as_bool() {
                    crate::log_if_err!(DestroyWindow(hwnd));
                }
                break;
            }
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&message) {
                continue;
            }
            if navigate_tab(hwnd, &message, GetKeyState(VK_SHIFT.0 as i32) < 0) {
                continue;
            }
            if !IsDialogMessageW(hwnd, &message).as_bool() {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        EnableWindow(parent, true);
        SetForegroundWindow(parent);
        if IsWindow(previous_focus).as_bool() {
            SetFocus(previous_focus);
        }
        // Only the main window contains AppState in GWLP_USERDATA.
        // Show confirmation after the report dialog is destroyed and its owner restored.
        if state.sent {
            crate::show_info(parent, language, &i18n::tr(language, "diagnostics.sent"));
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    crate::panic_guard::guard(
        "diagnostics_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || process(hwnd, msg, wparam, lparam),
    )
}

fn process(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_CREATE {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
        }
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if pointer.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let state = &mut *pointer;
        match msg {
            WM_CREATE => {
                let label = |key: &str| i18n::tr(state.language, key);
                let create = |class, value: &str, id, y, height, style| {
                    CreateWindowExW(
                        Default::default(),
                        class,
                        PCWSTR(to_wide(value).as_ptr()),
                        WS_CHILD | WS_VISIBLE | style,
                        18,
                        y,
                        590,
                        height,
                        hwnd,
                        HMENU(id as isize),
                        HINSTANCE(0),
                        None,
                    )
                };
                create(
                    w!("STATIC"),
                    &label("diagnostics.name"),
                    0,
                    14,
                    22,
                    WINDOW_STYLE(0),
                );
                let name = create(
                    w!("EDIT"),
                    "",
                    NAME,
                    40,
                    28,
                    WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                );
                SendMessageW(name, 0x00C5, WPARAM(200), LPARAM(0)); // EM_LIMITTEXT
                create(
                    w!("STATIC"),
                    &label("diagnostics.email"),
                    0,
                    80,
                    22,
                    WINDOW_STYLE(0),
                );
                let email = create(
                    w!("EDIT"),
                    "",
                    EMAIL,
                    106,
                    28,
                    WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                );
                SendMessageW(email, 0x00C5, WPARAM(254), LPARAM(0));
                create(
                    w!("STATIC"),
                    &label("diagnostics.report"),
                    0,
                    146,
                    22,
                    WINDOW_STYLE(0),
                );
                let report = create(
                    w!("EDIT"),
                    "",
                    REPORT,
                    172,
                    160,
                    WS_TABSTOP
                        | WS_BORDER
                        | WS_VSCROLL
                        | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32),
                );
                SendMessageW(report, 0x00C5, WPARAM(8000), LPARAM(0));
                create(
                    w!("STATIC"),
                    &label("diagnostics.notice"),
                    0,
                    346,
                    58,
                    WINDOW_STYLE(0),
                );
                for (id, key, x, width) in [
                    (SEND, "diagnostics.send", 18, 130),
                    (SAVE, "diagnostics.save", 158, 280),
                    (CANCEL, "options.cancel", 448, 160),
                ] {
                    CreateWindowExW(
                        Default::default(),
                        w!("BUTTON"),
                        PCWSTR(to_wide(&label(key)).as_ptr()),
                        WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                        x,
                        416,
                        width,
                        32,
                        hwnd,
                        HMENU(id as isize),
                        HINSTANCE(0),
                        None,
                    );
                }
                create(
                    PROGRESS_CLASSW,
                    &label("diagnostics.sending"),
                    PROGRESS,
                    456,
                    16,
                    WINDOW_STYLE(PBS_MARQUEE),
                );
                create(w!("STATIC"), "", STATUS, 480, 44, WINDOW_STYLE(0));
                SetTimer(hwnd, TIMER, 100, None);
                controls(hwnd, state);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xffff) as i32;
                let notification = (wparam.0 >> 16) as u32;
                if id == REPORT && notification == EN_CHANGE {
                    controls(hwnd, state);
                } else if id == SEND && !state.busy {
                    let name = text(hwnd, NAME).trim().to_string();
                    let email = text(hwnd, EMAIL).trim().to_string();
                    if !crate::diagnostics::valid_reply_email(&email) {
                        crate::message_box_modal(
                            hwnd,
                            PCWSTR(
                                to_wide(&i18n::tr(state.language, "diagnostics.email_invalid"))
                                    .as_ptr(),
                            ),
                            PCWSTR(to_wide(&crate::settings::error_title(state.language)).as_ptr()),
                            MB_OK | MB_ICONERROR,
                        );
                        SetFocus(GetDlgItem(hwnd, EMAIL));
                        return LRESULT(0);
                    }
                    let report = text(hwnd, REPORT).trim().to_string();
                    if report.is_empty() {
                        return LRESULT(0);
                    }
                    state.busy = true;
                    controls(hwnd, state);
                    set_status(hwnd, &i18n::tr(state.language, "diagnostics.sending"));
                    let completion = state.completion.clone();
                    std::thread::spawn(move || {
                        let result = std::panic::catch_unwind(|| {
                            crate::diagnostics::send_report(&name, &report, &email)
                        })
                        .unwrap_or_else(|_| Err("Diagnostic worker failed".into()));
                        *completion.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
                    });
                } else if id == SAVE && !state.busy {
                    crate::export_diagnostics_dialog_for_language(hwnd, state.language);
                } else if id == CANCEL && !state.busy {
                    crate::log_if_err!(DestroyWindow(hwnd));
                }
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == TIMER => {
                let result = state
                    .completion
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take();
                if let Some(result) = result {
                    state.busy = false;
                    controls(hwnd, state);
                    match result {
                        Ok(()) => {
                            state.sent = true;
                            crate::log_if_err!(DestroyWindow(hwnd));
                        }
                        Err(error) => {
                            crate::log_debug(&format!("Diagnostics report failed: {error}"));
                            let message = i18n::tr(state.language, "diagnostics.failed");
                            set_status(hwnd, &message);
                            // This dialog stores State, not the main AppState.
                            crate::message_box_modal(
                                hwnd,
                                PCWSTR(to_wide(&message).as_ptr()),
                                PCWSTR(
                                    to_wide(&crate::settings::error_title(state.language)).as_ptr(),
                                ),
                                MB_OK | MB_ICONERROR,
                            );
                            SetFocus(GetDlgItem(hwnd, SEND));
                        }
                    }
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                if !state.busy {
                    crate::log_if_err!(DestroyWindow(hwnd));
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                crate::log_if_err!(KillTimer(hwnd, TIMER));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;

    #[test]
    fn diagnostic_form_requires_description_and_blocks_duplicate_submissions() {
        unsafe {
            let class = w!("SonarpadDiagnosticsFormTest");
            RegisterClassW(&WNDCLASSW {
                lpszClassName: class,
                lpfnWndProc: Some(wndproc),
                ..Default::default()
            });
            let mut state = State {
                language: Language::Italian,
                busy: false,
                sent: false,
                completion: Arc::new(Mutex::new(None)),
            };
            let hwnd = CreateWindowExW(
                Default::default(),
                class,
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                640,
                576,
                HWND(0),
                HMENU(0),
                HINSTANCE(0),
                Some(&mut state as *mut _ as *const std::ffi::c_void),
            );
            assert_ne!(hwnd.0, 0);
            assert!(!IsWindowEnabled(GetDlgItem(hwnd, SEND)).as_bool());
            crate::set_window_text_w_safe(GetDlgItem(hwnd, REPORT), w!("  \r\n ")).unwrap();
            controls(hwnd, &state);
            assert!(!IsWindowEnabled(GetDlgItem(hwnd, SEND)).as_bool());
            crate::set_window_text_w_safe(GetDlgItem(hwnd, REPORT), w!("Playback stopped"))
                .unwrap();
            controls(hwnd, &state);
            assert!(IsWindowEnabled(GetDlgItem(hwnd, SEND)).as_bool());
            assert!(text(hwnd, NAME).is_empty()); // Name is optional.
            // Exercise the actual key handler with real native controls.
            SetFocus(GetDlgItem(hwnd, NAME));
            for expected in [EMAIL, REPORT, SEND, SAVE, CANCEL, NAME] {
                let message = MSG {
                    hwnd: GetFocus(),
                    message: WM_KEYDOWN,
                    wParam: WPARAM(VK_TAB.0 as usize),
                    ..Default::default()
                };
                assert!(navigate_tab(hwnd, &message, false));
                assert_eq!(GetFocus(), GetDlgItem(hwnd, expected));
            }
            for expected in [CANCEL, SAVE, SEND, REPORT, EMAIL, NAME] {
                let message = MSG {
                    hwnd: GetFocus(),
                    message: WM_KEYDOWN,
                    wParam: WPARAM(VK_TAB.0 as usize),
                    ..Default::default()
                };
                assert!(navigate_tab(hwnd, &message, true));
                assert_eq!(GetFocus(), GetDlgItem(hwnd, expected));
            }
            assert_eq!(text(hwnd, REPORT), "Playback stopped");
            crate::set_window_text_w_safe(GetDlgItem(hwnd, REPORT), w!("")).unwrap();
            controls(hwnd, &state);
            SetFocus(GetDlgItem(hwnd, REPORT));
            let message = MSG {
                hwnd: GetFocus(),
                message: WM_KEYDOWN,
                wParam: WPARAM(VK_TAB.0 as usize),
                ..Default::default()
            };
            assert!(navigate_tab(hwnd, &message, false));
            assert_eq!(GetFocus(), GetDlgItem(hwnd, SAVE)); // Skip disabled Send.
            crate::set_window_text_w_safe(GetDlgItem(hwnd, REPORT), w!("Playback stopped"))
                .unwrap();

            state.busy = true;
            controls(hwnd, &state);
            for id in [SEND, SAVE, NAME, EMAIL, REPORT, CANCEL] {
                assert!(!IsWindowEnabled(GetDlgItem(hwnd, id)).as_bool());
            }
            SendMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
            assert!(IsWindow(hwnd).as_bool());
            state.busy = false;
            controls(hwnd, &state);
            assert_eq!(text(hwnd, REPORT), "Playback stopped");
            assert!(IsWindowEnabled(GetDlgItem(hwnd, SEND)).as_bool());
            state.busy = true;
            controls(hwnd, &state);
            assert_ne!(GetDlgItem(hwnd, PROGRESS).0, 0);
            assert_ne!(
                GetWindowLongW(GetDlgItem(hwnd, PROGRESS), GWL_STYLE) as u32 & WS_VISIBLE.0,
                0
            );
            *state.completion.lock().unwrap() = Some(Ok(()));
            SendMessageW(hwnd, WM_TIMER, WPARAM(TIMER), LPARAM(0));
            assert!(state.sent);
            assert!(!IsWindow(hwnd).as_bool());
        }
    }
}
