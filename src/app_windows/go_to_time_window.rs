use crate::accessibility::{handle_accessibility, nvda_speak, to_wide};
use crate::audio_player::{audiobook_duration_secs, parse_time_input, seek_audiobook_to};
use crate::i18n;
use crate::with_state;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, GetDlgItem,
    GetParent, GetWindowLongPtrW, HMENU, IDC_ARROW, LoadCursorW, RegisterClassW, SendMessageW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowTextW, WINDOW_STYLE, WM_COMMAND, WM_CREATE,
    WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE,
    WS_EX_DLGMODALFRAME, WS_POPUP, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

const GO_TO_TIME_CLASS: &str = "SonarpadGoToTime";
const GO_TO_TIME_EDIT_ID: usize = 1801;
const GO_TO_TIME_OK_ID: usize = 1802;
const GO_TO_TIME_CANCEL_ID: usize = 1803;
const GO_TO_TIME_STATUS_ID: usize = 1804;

struct GoToTimeState {
    parent: HWND,
    input: HWND,
    status: HWND,
    prev_focus: HWND,
}

pub fn open(parent: HWND) {
    unsafe {
        let existing = with_state(parent, |state| state.go_to_time_dialog).unwrap_or(HWND(0));
        if existing.0 != 0 {
            SetForegroundWindow(existing);
            return;
        }
        let has_player =
            with_state(parent, |state| state.active_audiobook.is_some()).unwrap_or(false);
        if !has_player && !crate::can_go_to_time_in_managed_mpv(parent) {
            return;
        }

        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(GO_TO_TIME_CLASS);
        let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
        let title_w = to_wide(&i18n::tr(language, "go_to_time.title"));

        let wc = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
            ),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(go_to_time_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let prev_focus = GetFocus();
        let state = Box::new(GoToTimeState {
            parent,
            input: HWND(0),
            status: HWND(0),
            prev_focus,
        });
        let state_ptr = Box::into_raw(state);
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            WS_POPUP | WS_CAPTION | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            360,
            180,
            parent,
            HMENU(0),
            hinstance,
            Some(state_ptr as *const _),
        );
        if hwnd.0 == 0 {
            let _unused_box = Box::from_raw(state_ptr);
            return;
        }
        EnableWindow(parent, false);
        with_state(parent, |state| state.go_to_time_dialog = hwnd);
    }
}

unsafe extern "system" fn go_to_time_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "go_to_time_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || go_to_time_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn go_to_time_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const CREATESTRUCTW;
                let init_ptr = (*cs).lpCreateParams as *mut GoToTimeState;
                if init_ptr.is_null() {
                    return LRESULT(0);
                }
                SetWindowLongPtrW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                    init_ptr as isize,
                );
                let parent = (*init_ptr).parent;
                let language =
                    with_state(parent, |state| state.settings.language).unwrap_or_default();

                let label = i18n::tr(language, "go_to_time.label_time");
                let hint = i18n::tr(language, "go_to_time.hint");
                let ok_text = i18n::tr(language, "go_to_time.ok");
                let cancel_text = i18n::tr(language, "go_to_time.cancel");

                let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    10,
                    12,
                    330,
                    16,
                    hwnd,
                    HMENU(1),
                    hinstance,
                    None,
                );
                let input = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    10,
                    30,
                    160,
                    24,
                    hwnd,
                    HMENU(GO_TO_TIME_EDIT_ID as isize),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&hint).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    10,
                    58,
                    330,
                    16,
                    hwnd,
                    HMENU(2),
                    hinstance,
                    None,
                );
                let status = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE,
                    10,
                    78,
                    330,
                    16,
                    hwnd,
                    HMENU(GO_TO_TIME_STATUS_ID as isize),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&ok_text).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    170,
                    110,
                    80,
                    26,
                    hwnd,
                    HMENU(GO_TO_TIME_OK_ID as isize),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&cancel_text).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    260,
                    110,
                    80,
                    26,
                    hwnd,
                    HMENU(GO_TO_TIME_CANCEL_ID as isize),
                    hinstance,
                    None,
                );

                (*init_ptr).input = input;
                (*init_ptr).status = status;
                SetFocus(input);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 as u32
                    == windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE.0 as u32
                {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                if id == GO_TO_TIME_CANCEL_ID || id == 2 {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                if id == GO_TO_TIME_OK_ID || id == 1 {
                    let parent = GetParent(hwnd);
                    let language =
                        with_state(parent, |state| state.settings.language).unwrap_or_default();
                    let input = GetDlgItem(hwnd, GO_TO_TIME_EDIT_ID as i32);
                    let len = SendMessageW(
                        input,
                        windows::Win32::UI::WindowsAndMessaging::WM_GETTEXTLENGTH,
                        WPARAM(0),
                        LPARAM(0),
                    )
                    .0;
                    let mut buf = vec![0u16; len as usize + 1];
                    SendMessageW(
                        input,
                        windows::Win32::UI::WindowsAndMessaging::WM_GETTEXT,
                        WPARAM(buf.len()),
                        LPARAM(buf.as_mut_ptr() as isize),
                    );
                    let text = String::from_utf16_lossy(&buf[..len as usize]);
                    let target = match parse_time_input(&text) {
                        Ok(v) => v,
                        Err(_) => {
                            let msg = i18n::tr(language, "go_to_time.invalid_time");
                            let status = GetDlgItem(hwnd, GO_TO_TIME_STATUS_ID as i32);
                            let wide = to_wide(&msg);
                            crate::log_if_err!(SetWindowTextW(status, PCWSTR(wide.as_ptr())));
                            nvda_speak(&msg);
                            SetFocus(input);
                            return LRESULT(0);
                        }
                    };
                    let managed_mpv = crate::can_go_to_time_in_managed_mpv(parent);
                    let duration = if managed_mpv {
                        crate::managed_mpv_duration_for_go_to_time(parent)
                    } else {
                        let path = with_state(parent, |state| {
                            state.active_audiobook.as_ref().map(|p| p.path.clone())
                        })
                        .flatten();
                        let Some(path) = path else {
                            crate::log_if_err!(crate::destroy_window_safe(hwnd));
                            return LRESULT(0);
                        };
                        audiobook_duration_secs(&path)
                    };
                    let mut seek_target = target as u64;
                    if let Some(max_secs) = duration
                        && seek_target > max_secs
                    {
                        seek_target = max_secs;
                        let msg = i18n::tr(language, "go_to_time.clamped");
                        let status = GetDlgItem(hwnd, GO_TO_TIME_STATUS_ID as i32);
                        let wide = to_wide(&msg);
                        crate::log_if_err!(SetWindowTextW(status, PCWSTR(wide.as_ptr())));
                        nvda_speak(&msg);
                    }
                    let seek_result = if managed_mpv {
                        crate::seek_managed_mpv_to_time(parent, seek_target)
                    } else {
                        seek_audiobook_to(parent, seek_target)
                    };
                    if let Err(err) = seek_result {
                        crate::log_debug(&format!("Go to time: absolute seek failed: {err}"));
                        let status = GetDlgItem(hwnd, GO_TO_TIME_STATUS_ID as i32);
                        let wide = to_wide(&err);
                        crate::log_if_err!(SetWindowTextW(status, PCWSTR(wide.as_ptr())));
                        nvda_speak(&err);
                        SetFocus(input);
                        return LRESULT(0);
                    }
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_DESTROY => {
                let parent = GetParent(hwnd);
                if parent.0 != 0 {
                    EnableWindow(parent, true);
                    SetForegroundWindow(parent);
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut GoToTimeState;
                if !ptr.is_null() {
                    let state = Box::from_raw(ptr);
                    let parent = state.parent;
                    with_state(parent, |s| s.go_to_time_dialog = HWND(0));
                    if parent.0 != 0 && state.prev_focus.0 != 0 {
                        SetFocus(state.prev_focus);
                    }
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

pub fn handle_navigation(hwnd: HWND, msg: &windows::Win32::UI::WindowsAndMessaging::MSG) -> bool {
    handle_accessibility(hwnd, msg)
}
