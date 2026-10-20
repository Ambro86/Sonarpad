use crate::accessibility::{normalize_to_crlf, to_wide};
use crate::i18n;
use crate::settings::Language;
use crate::with_state;
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::WC_BUTTON;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, GetKeyState, SetFocus, VK_ESCAPE, VK_RETURN, VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    ES_AUTOVSCROLL, ES_MULTILINE, ES_WANTRETURN, GWLP_USERDATA, GetWindowLongPtrW, HMENU,
    IDC_ARROW, IDCANCEL, IsChild, LoadCursorW, MSG, MoveWindow, PostMessageW, RegisterClassW,
    SendMessageW, SetForegroundWindow, SetWindowLongPtrW, SetWindowTextW, WINDOW_STYLE, WM_APP,
    WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_SETFOCUS, WM_SETFONT,
    WM_SIZE, WNDCLASSW, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_OVERLAPPEDWINDOW,
    WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, w};

const HELP_CLASS_NAME: &str = "SonarpadHelp";
const HELP_ID_OK: usize = 7003;
const WM_HELP_FOCUS_CONTENT: u32 = WM_APP + 1;
const READONLY_TEXT_CLASS_NAME: &str = "SonarpadReadonlyText";
const READONLY_TEXT_ID_OK: usize = 7013;
const DONATIONS_IT: &str = include_str!("../../donations_it.txt");
const DONATIONS_EN: &str = include_str!("../../donations_en.txt");
const DONATIONS_DE: &str = include_str!("../../donations_de.txt");
const DONATIONS_UK: &str = include_str!("../../donations_uk.txt");
const DONATIONS_ES: &str = include_str!("../../donations_es.txt");
const DONATIONS_PT: &str = include_str!("../../donations_pt.txt");
const DONATIONS_PT_BR: &str = include_str!("../../donations_pt_BR.txt");
const DONATIONS_SV: &str = include_str!("../../donations_sv.txt");
const DONATIONS_CS: &str = include_str!("../../donations_cs.txt");
const DONATIONS_PL: &str = include_str!("../../donations_pl.txt");
const DONATIONS_FR: &str = include_str!("../../donations_fr.txt");
const DONATIONS_SR: &str = include_str!("../../donations_sr.txt");
const DONATIONS_LT: &str = include_str!("../../donations_lt.txt");
const DONATIONS_RU: &str = include_str!("../../donations_ru.txt");
const DONATIONS_ZH: &str = include_str!("../../donations_zh.txt");

fn read_override_text(file_name: &str) -> Option<String> {
    let exe_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(err) => {
            crate::log_debug(&format!("Help content: failed to resolve exe path: {err}"));
            return None;
        }
    };
    let Some(dir) = exe_path.parent() else {
        crate::log_debug("Help content: exe path has no parent directory");
        return None;
    };
    let path = dir.join(file_name);
    if !path.exists() {
        return None;
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => Some(content),
        Err(err) => {
            crate::log_debug(&format!(
                "Help content: failed to read override {file_name}: {err}"
            ));
            None
        }
    }
}

#[derive(Clone, Copy)]
enum HelpWindowKind {
    Guide,
    Changelog,
    UpdateChangelog,
    Donations,
}

struct HelpWindowInit {
    parent: HWND,
    kind: HelpWindowKind,
    language: Language,
}

struct HelpWindowState {
    parent: HWND,
    edit: HWND,
    ok_button: HWND,
    kind: HelpWindowKind,
}

struct ReadonlyTextInit {
    content: String,
}

struct ReadonlyTextState {
    parent: HWND,
    edit: HWND,
    ok_button: HWND,
}

static READONLY_TEXT_WINDOWS: OnceLock<Mutex<Vec<HWND>>> = OnceLock::new();

fn readonly_text_windows() -> &'static Mutex<Vec<HWND>> {
    READONLY_TEXT_WINDOWS.get_or_init(|| Mutex::new(Vec::new()))
}

fn register_readonly_text_window(hwnd: HWND) {
    match readonly_text_windows().lock() {
        Ok(mut windows) => windows.push(hwnd),
        Err(_e) => crate::log_debug("Failed to lock readonly text windows"),
    }
}

fn unregister_readonly_text_window(hwnd: HWND) {
    match readonly_text_windows().lock() {
        Ok(mut windows) => windows.retain(|w| *w != hwnd),
        Err(_e) => crate::log_debug("Failed to lock readonly text windows"),
    }
}

pub fn handle_readonly_navigation(msg: &MSG) -> bool {
    unsafe {
        if msg.message != WM_KEYDOWN {
            return false;
        }
        let windows: Vec<HWND> = match readonly_text_windows().lock() {
            Ok(windows) => windows.clone(),
            Err(_e) => {
                crate::log_debug("Failed to lock readonly text windows");
                return false;
            }
        };
        let target = windows
            .into_iter()
            .find(|hwnd| msg.hwnd == *hwnd || IsChild(*hwnd, msg.hwnd).as_bool());
        let Some(hwnd) = target else {
            return false;
        };
        let key = msg.wParam.0 as u32;
        if key == VK_TAB.0 as u32 {
            let shift_down = GetKeyState(VK_SHIFT.0 as i32) < 0;
            if with_readonly_text_state(hwnd, |state| {
                if shift_down {
                    if GetFocus() == state.ok_button {
                        SetFocus(state.edit);
                    } else {
                        SetFocus(state.ok_button);
                    }
                } else if GetFocus() == state.edit {
                    SetFocus(state.ok_button);
                } else {
                    SetFocus(state.edit);
                }
            })
            .is_none()
            {
                crate::log_debug("Failed to access readonly text state");
            }
            return true;
        }
        if key == VK_ESCAPE.0 as u32 {
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            return true;
        }
        if key == VK_RETURN.0 as u32 {
            let mut handled = false;
            if with_readonly_text_state(hwnd, |state| {
                if GetFocus() == state.ok_button {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    handled = true;
                }
            })
            .is_none()
            {
                crate::log_debug("Failed to access readonly text state");
            }
            if handled {
                return true;
            }
        }
        false
    }
}

pub fn open(parent: HWND) {
    open_window(parent, HelpWindowKind::Guide);
}

pub fn open_changelog(parent: HWND) {
    open_window(parent, HelpWindowKind::Changelog);
}

pub fn open_update_changelog(parent: HWND) {
    open_window(parent, HelpWindowKind::UpdateChangelog);
}

pub fn open_donations(parent: HWND) {
    open_window(parent, HelpWindowKind::Donations);
}

pub fn open_readonly_text(parent: HWND, title: &str, content: &str) {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(READONLY_TEXT_CLASS_NAME);
        let wc = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
            ),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(readonly_text_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let init = Box::new(ReadonlyTextInit {
            content: normalize_to_crlf(content),
        });
        let init_ptr = Box::into_raw(init);
        let title_wide = to_wide(title);
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            720,
            540,
            parent,
            None,
            hinstance,
            Some(init_ptr as *const std::ffi::c_void),
        );
        if hwnd.0 != 0 {
            register_readonly_text_window(hwnd);
            SetForegroundWindow(hwnd);
        } else if !init_ptr.is_null() {
            let _unused_box = Box::from_raw(init_ptr);
        }
    }
}

fn open_window(parent: HWND, kind: HelpWindowKind) {
    let existing = {
        with_state(parent, |state| match kind {
            HelpWindowKind::Guide => state.help_window,
            HelpWindowKind::Changelog | HelpWindowKind::UpdateChangelog => state.changelog_window,
            HelpWindowKind::Donations => state.donations_window,
        })
    }
    .unwrap_or(HWND(0));
    if existing.0 != 0 {
        crate::set_foreground_window_safe(existing);
        return;
    }

    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let class_name = to_wide(HELP_CLASS_NAME);
    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(unsafe {
            LoadCursorW(None, IDC_ARROW).unwrap_or_default().0
        }),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(help_wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    crate::register_class_w_safe(&wc);

    let language = { with_state(parent, |state| state.settings.language) }.unwrap_or_default();
    let title = to_wide(&help_title(language, kind));
    let init = Box::new(HelpWindowInit {
        parent,
        kind,
        language,
    });
    let init_ptr = Box::into_raw(init);
    let window = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            800,
            620,
            parent,
            None,
            hinstance,
            Some(init_ptr as *const std::ffi::c_void),
        )
    };

    if window.0 != 0 {
        if {
            with_state(parent, |state| match kind {
                HelpWindowKind::Guide => state.help_window = window,
                HelpWindowKind::Changelog | HelpWindowKind::UpdateChangelog => {
                    state.changelog_window = window
                }
                HelpWindowKind::Donations => state.donations_window = window,
            })
        }
        .is_none()
        {
            crate::log_debug("Failed to access help state");
        }
        crate::set_foreground_window_safe(window);
    } else if !init_ptr.is_null() {
        let _unused_box = crate::box_from_raw_safe(init_ptr);
    }
}

pub fn handle_tab(hwnd: HWND) {
    unsafe {
        if with_help_state(hwnd, |state| {
            let focus = GetFocus();

            if focus == state.edit {
                SetFocus(state.ok_button);
            } else {
                SetFocus(state.edit);
            }
        })
        .is_none()
        {
            crate::log_debug("Failed to access help state");
        }
    }
}

unsafe extern "system" fn help_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "help_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || help_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn help_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let create_struct = lparam.0 as *const CREATESTRUCTW;
                let init_ptr = (*create_struct).lpCreateParams as *mut HelpWindowInit;
                if init_ptr.is_null() {
                    return LRESULT(0);
                }
                let init = Box::from_raw(init_ptr);
                let parent = init.parent;
                let hfont = with_state(parent, |state| state.hfont).unwrap_or(HFONT(0));

                let edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_VSCROLL
                        | WINDOW_STYLE(ES_MULTILINE as u32)
                        | WINDOW_STYLE(ES_AUTOVSCROLL as u32)
                        | WINDOW_STYLE(ES_WANTRETURN as u32)
                        | WS_TABSTOP,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(
                    edit,
                    windows::Win32::UI::Controls::EM_SETREADONLY,
                    WPARAM(1),
                    LPARAM(0),
                );
                if hfont.0 != 0 {
                    SendMessageW(edit, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                }

                let ok_text = i18n::tr(init.language, "options.ok");
                let ok_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&ok_text).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(HELP_ID_OK as isize),
                    HINSTANCE(0),
                    None,
                );
                if hfont.0 != 0 && ok_button.0 != 0 {
                    SendMessageW(ok_button, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                }

                let content = match init.kind {
                    HelpWindowKind::Guide => match init.language {
                        Language::Italian => include_str!("../../guida.txt").to_string(),
                        Language::Ukrainian => include_str!("../../guida_uk.txt").to_string(),
                        Language::German => include_str!("../../guida_de.txt").to_string(),
                        Language::English => include_str!("../../guida_en.txt").to_string(),
                        Language::Spanish => include_str!("../../guida_es.txt").to_string(),
                        Language::Portuguese => include_str!("../../guida_pt.txt").to_string(),
                        Language::PortugueseBrazilian => {
                            include_str!("../../guida_pt_BR.txt").to_string()
                        }
                        Language::Swedish => read_override_text("guida_sv.txt")
                            .unwrap_or_else(|| include_str!("../../guida_sv.txt").to_string()),
                        Language::Vietnamese => include_str!("../../guida_vi.txt").to_string(),
                        Language::Czech => read_override_text("guida_cs.txt")
                            .unwrap_or_else(|| include_str!("../../guida_cs.txt").to_string()),
                        Language::Polish => read_override_text("guida_pl.txt")
                            .unwrap_or_else(|| include_str!("../../guida_pl.txt").to_string()),
                        Language::French => read_override_text("guida_fr.txt")
                            .unwrap_or_else(|| include_str!("../../guida_fr.txt").to_string()),
                        Language::Serbian => read_override_text("guida_sr.txt")
                            .unwrap_or_else(|| include_str!("../../guida_sr.txt").to_string()),
                        Language::Lithuanian => include_str!("../../guida_lt.txt").to_string(),
                        Language::Russian => include_str!("../../guida_ru.txt").to_string(),
                        Language::Chinese => include_str!("../../guida_zh.txt").to_string(),
                        Language::Hindi => include_str!("../../guida_hi.txt").to_string(),
                        Language::Indonesian => include_str!("../../guida_id.txt").to_string(),
                    },
                    HelpWindowKind::Changelog => changelog_content(init.language),
                    HelpWindowKind::UpdateChangelog => {
                        update_completed_changelog_content(init.language)
                    }
                    HelpWindowKind::Donations => donations_content(init.language),
                };
                let content = normalize_to_crlf(&content);
                let content_wide = to_wide(&content);
                if let Err(e) = SetWindowTextW(edit, PCWSTR(content_wide.as_ptr())) {
                    crate::log_debug(&format!("Failed to set help content: {}", e));
                }
                let state = Box::new(HelpWindowState {
                    parent,
                    edit,
                    ok_button,
                    kind: init.kind,
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
                if let Err(err) = PostMessageW(hwnd, WM_HELP_FOCUS_CONTENT, WPARAM(0), LPARAM(0)) {
                    crate::log_debug(&format!(
                        "Failed to post help content focus message: {}",
                        err
                    ));
                    SetFocus(edit);
                }
                LRESULT(0)
            }
            WM_HELP_FOCUS_CONTENT => {
                if with_help_state(hwnd, |state| {
                    SetFocus(state.edit);
                })
                .is_none()
                {
                    crate::log_debug("Failed to focus help content");
                }
                LRESULT(0)
            }
            WM_SETFOCUS => {
                if with_help_state(hwnd, |state| {
                    SetFocus(state.edit);
                })
                .is_none()
                {
                    crate::log_debug("Failed to access help state");
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let cmd_id = wparam.0 & 0xffff;
                if cmd_id == HELP_ID_OK || cmd_id == IDCANCEL.0 as usize {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SIZE => {
                let width = (lparam.0 & 0xffff) as i32;
                let height = ((lparam.0 >> 16) & 0xffff) as i32;
                if with_help_state(hwnd, |state| {
                    let button_width = 100;
                    let button_height = 30;
                    let margin = 12;
                    let edit_height = (height - button_height - (margin * 3)).max(0);
                    crate::log_if_err!(MoveWindow(
                        state.edit,
                        margin,
                        margin,
                        (width - (margin * 2)).max(0),
                        edit_height,
                        true,
                    ));
                    crate::log_if_err!(MoveWindow(
                        state.ok_button,
                        width - button_width - margin,
                        height - button_height - margin,
                        button_width,
                        button_height,
                        true,
                    ));
                })
                .is_none()
                {
                    crate::log_debug("Failed to access help state");
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                let (parent, kind) = with_help_state(hwnd, |state| (state.parent, state.kind))
                    .unwrap_or((HWND(0), HelpWindowKind::Guide));
                if parent.0 != 0
                    && with_state(parent, |state| match kind {
                        HelpWindowKind::Guide => state.help_window = HWND(0),
                        HelpWindowKind::Changelog | HelpWindowKind::UpdateChangelog => {
                            state.changelog_window = HWND(0)
                        }
                        HelpWindowKind::Donations => state.donations_window = HWND(0),
                    })
                    .is_none()
                {
                    crate::log_debug("Failed to access help state");
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HelpWindowState;
                if !ptr.is_null() {
                    let _unused_box = Box::from_raw(ptr);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 as u32 == VK_RETURN.0 as u32 {
                    if with_help_state(hwnd, |state| {
                        if GetFocus() == state.ok_button {
                            crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        }
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access help state");
                    }
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn with_help_state<F, R>(hwnd: HWND, f: F) -> Option<R>
where
    F: FnOnce(&mut HelpWindowState) -> R,
{
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut HelpWindowState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn help_title(language: Language, kind: HelpWindowKind) -> String {
    match kind {
        HelpWindowKind::Guide => i18n::tr(language, "help.window.guide"),
        HelpWindowKind::Changelog => i18n::tr(language, "help.window.changelog"),
        HelpWindowKind::UpdateChangelog => update_completed_changelog_title(language),
        HelpWindowKind::Donations => i18n::tr(language, "help.window.donations"),
    }
}

fn update_completed_changelog_title(language: Language) -> String {
    let completed = i18n::tr(language, "updater.info.completed");
    let completed = completed.trim().trim_end_matches(['.', '。', '।']);
    let changelog = i18n::tr(language, "help.window.changelog");
    format!(
        "{} – {} – Sonarpad {}",
        completed,
        changelog,
        crate::app_display_version()
    )
}

fn changelog_content(language: Language) -> String {
    match language {
        Language::Italian => include_str!("../../CHANGELOG_IT.md").to_string(),
        Language::German => include_str!("../../CHANGELOG_DE.md").to_string(),
        Language::English | Language::Indonesian => include_str!("../../CHANGELOG.md").to_string(),
        Language::Ukrainian => include_str!("../../CHANGELOG_UK.md").to_string(),
        Language::Hindi => include_str!("../../CHANGELOG-HI.txt").to_string(),
        Language::Spanish => include_str!("../../CHANGELOG_ES.md").to_string(),
        Language::Portuguese => include_str!("../../CHANGELOG_PT.md").to_string(),
        Language::PortugueseBrazilian => include_str!("../../CHANGELOG_PT_BR.md").to_string(),
        Language::Swedish => include_str!("../../CHANGELOG_SV.md").to_string(),
        Language::Vietnamese => include_str!("../../CHANGELOG_VI.md").to_string(),
        Language::Czech => include_str!("../../CHANGELOG_CS.md").to_string(),
        Language::Polish => include_str!("../../CHANGELOG_PL.md").to_string(),
        Language::French => include_str!("../../CHANGELOG_FR.md").to_string(),
        Language::Serbian => include_str!("../../CHANGELOG_SR.md").to_string(),
        Language::Lithuanian => include_str!("../../CHANGELOG_LT.md").to_string(),
        Language::Russian => include_str!("../../CHANGELOG_RU.md").to_string(),
        Language::Chinese => include_str!("../../CHANGELOG_ZH.md").to_string(),
    }
}

fn update_completed_changelog_content(language: Language) -> String {
    format!(
        "{}\r\n\r\n{}",
        i18n::tr(language, "updater.info.completed"),
        changelog_content(language)
    )
}

fn donations_content(language: Language) -> String {
    match language {
        Language::Italian => DONATIONS_IT.to_string(),
        Language::Ukrainian => DONATIONS_UK.to_string(),
        Language::German => DONATIONS_DE.to_string(),
        Language::English => DONATIONS_EN.to_string(),
        Language::Spanish => DONATIONS_ES.to_string(),
        Language::Portuguese => DONATIONS_PT.to_string(),
        Language::PortugueseBrazilian => DONATIONS_PT_BR.to_string(),
        Language::Swedish => {
            read_override_text("donations_sv.txt").unwrap_or_else(|| DONATIONS_SV.to_string())
        }
        Language::Vietnamese => DONATIONS_EN.to_string(),
        Language::Czech => {
            read_override_text("donations_cs.txt").unwrap_or_else(|| DONATIONS_CS.to_string())
        }
        Language::Polish => {
            read_override_text("donations_pl.txt").unwrap_or_else(|| DONATIONS_PL.to_string())
        }
        Language::French => {
            read_override_text("donations_fr.txt").unwrap_or_else(|| DONATIONS_FR.to_string())
        }
        Language::Serbian => {
            read_override_text("donations_sr.txt").unwrap_or_else(|| DONATIONS_SR.to_string())
        }
        Language::Lithuanian => DONATIONS_LT.to_string(),
        Language::Russian => DONATIONS_RU.to_string(),
        Language::Chinese => DONATIONS_ZH.to_string(),
        Language::Hindi => DONATIONS_EN.to_string(),
        Language::Indonesian => include_str!("../../donations_id.txt").to_string(),
    }
}

unsafe extern "system" fn readonly_text_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "readonly_text_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || readonly_text_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn readonly_text_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create_struct = lparam.0 as *const CREATESTRUCTW;
            let init_ptr = unsafe { (*create_struct).lpCreateParams as *mut ReadonlyTextInit };
            if init_ptr.is_null() {
                return LRESULT(0);
            }
            let init = crate::box_from_raw_safe(init_ptr);

            let edit = unsafe {
                CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_VSCROLL
                        | WINDOW_STYLE(ES_MULTILINE as u32)
                        | WINDOW_STYLE(ES_AUTOVSCROLL as u32)
                        | WINDOW_STYLE(ES_WANTRETURN as u32)
                        | WS_TABSTOP,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                )
            };
            crate::send_message_w_safe(
                edit,
                windows::Win32::UI::Controls::EM_SETREADONLY,
                WPARAM(1),
                LPARAM(0),
            );

            let ok_button = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    w!("OK"),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(READONLY_TEXT_ID_OK as isize),
                    HINSTANCE(0),
                    None,
                )
            };

            let content_wide = to_wide(&init.content);
            let _res = crate::set_window_text_w_safe(edit, PCWSTR(content_wide.as_ptr()));
            crate::set_focus_safe(edit);

            let state = Box::new(ReadonlyTextState {
                parent: unsafe { (*create_struct).hwndParent },
                edit,
                ok_button,
            });
            crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
            LRESULT(0)
        }
        WM_SETFOCUS => {
            if with_readonly_text_state(hwnd, |state| crate::set_focus_safe(state.edit)).is_none() {
                crate::log_debug("Failed to access readonly text state");
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd_id = wparam.0 & 0xffff;
            if cmd_id == READONLY_TEXT_ID_OK || cmd_id == IDCANCEL.0 as usize {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                return LRESULT(0);
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_SIZE => {
            let width = (lparam.0 & 0xffff) as i32;
            let height = ((lparam.0 >> 16) & 0xffff) as i32;
            if with_readonly_text_state(hwnd, |state| {
                let button_width = 100;
                let button_height = 30;
                let margin = 12;
                let edit_height = (height - button_height - (margin * 3)).max(0);
                crate::log_if_err!(unsafe {
                    MoveWindow(
                        state.edit,
                        margin,
                        margin,
                        (width - (margin * 2)).max(0),
                        edit_height,
                        true,
                    )
                });
                crate::log_if_err!(unsafe {
                    MoveWindow(
                        state.ok_button,
                        width - button_width - margin,
                        height - button_height - margin,
                        button_width,
                        button_height,
                        true,
                    )
                });
            })
            .is_none()
            {
                crate::log_debug("Failed to access readonly text state");
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            unregister_readonly_text_window(hwnd);
            let ptr =
                crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut ReadonlyTextState;
            if !ptr.is_null() {
                let state = crate::box_from_raw_safe(ptr);
                if state.parent.0 != 0 {
                    crate::set_foreground_window_safe(state.parent);
                    crate::app_windows::podcasts_window::focus_library(state.parent);
                    crate::app_windows::rss_window::focus_library(state.parent);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 as u32 == VK_RETURN.0 as u32 {
                if with_readonly_text_state(hwnd, |state| {
                    if crate::get_focus_safe() == state.ok_button {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access readonly text state");
                }
                return LRESULT(0);
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        _ => crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
    }
}

fn with_readonly_text_state<F, R>(hwnd: HWND, f: F) -> Option<R>
where
    F: FnOnce(&mut ReadonlyTextState) -> R,
{
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut ReadonlyTextState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}
