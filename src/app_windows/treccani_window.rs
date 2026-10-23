use crate::accessibility::{handle_accessibility, normalize_to_crlf, to_wide, to_wide_normalized};
use crate::editor_manager::{self, get_edit_text};
use crate::i18n;
use crate::settings::Language;
use crate::treccani;
use crate::{WM_FOCUS_EDITOR, get_active_edit, show_error, with_state};
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::Controls::RichEdit::{CHARRANGE, EM_EXSETSEL};
use windows::Win32::UI::Controls::{
    EM_LIMITTEXT, EM_SCROLLCARET, EM_SETSEL, WC_COMBOBOXW, WC_LISTBOXW, WC_STATIC,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus, VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_RESETCONTENT, CB_SETCURSEL, CBS_DROPDOWNLIST,
    CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, ES_AUTOHSCROLL, GW_CHILD,
    GWLP_USERDATA, GetWindow, GetWindowLongPtrW, HMENU, IDC_ARROW, IsWindow, LB_ADDSTRING,
    LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL, LBN_DBLCLK, LBS_HASSTRINGS, LBS_NOINTEGRALHEIGHT,
    LBS_NOTIFY, LoadCursorW, MSG, PostMessageW, RegisterClassW, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowTextW, WINDOW_STYLE, WM_APP, WM_COMMAND, WM_CREATE, WM_DESTROY,
    WM_KEYDOWN, WM_NCDESTROY, WM_SETFOCUS, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE,
    WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
    WS_VSCROLL,
};
use windows::core::{PCWSTR, w};

const TRECCANI_CLASS_NAME: &str = "SonarpadTreccani";
const TRECCANI_INPUT_ID: usize = 9501;
const TRECCANI_SEARCH_ID: usize = 9502;
const TRECCANI_RESULTS_ID: usize = 9503;
const TRECCANI_STATUS_ID: usize = 9504;
const TRECCANI_CLOSE_ID: usize = 9505;
const TRECCANI_SECTIONS_ID: usize = 9506;
const TRECCANI_IMPORT_ID: usize = 9507;
const TRECCANI_LANGUAGE_ID: usize = 9508;

const WM_TRECCANI_SEARCH_DONE: u32 = WM_APP + 110;
const WM_TRECCANI_IMPORT_DONE: u32 = WM_APP + 111;
static SEARCH_GENERATION: AtomicUsize = AtomicUsize::new(0);
static IMPORT_GENERATION: AtomicUsize = AtomicUsize::new(0);

struct TreccaniWindowState {
    parent: HWND,
    input: HWND,
    language_combo: HWND,
    search: HWND,
    results: HWND,
    sections: HWND,
    import_button: HWND,
    status: HWND,
    close: HWND,
    results_data: Vec<treccani::SearchResult>,
    current_extract: Option<treccani::ExtractResult>,
}

struct TreccaniLabels {
    title: String,
    search_label: String,
    language_label: String,
    language_auto: String,
    search_button: String,
    results_label: String,
    sections_label: String,
    import_all: String,
    import_button: String,
    status_loading: String,
    status_no_results: String,
    status_no_query: String,
    status_importing: String,
    status_search_error: String,
    status_import_error: String,
    close: String,
}

struct SearchPayload {
    results: Vec<treccani::SearchResult>,
    error: Option<String>,
}

struct ImportPayload {
    extract: Option<treccani::ExtractResult>,
    error: Option<String>,
}

fn labels(_language: Language) -> TreccaniLabels {
    TreccaniLabels {
        title: i18n::tr_treccani("treccani.title"),
        search_label: i18n::tr_treccani("treccani.search_label"),
        language_label: i18n::tr_treccani("treccani.language_label"),
        language_auto: i18n::tr_treccani("treccani.language_italian"),
        search_button: i18n::tr_treccani("treccani.search_button"),
        results_label: i18n::tr_treccani("treccani.results_label"),
        sections_label: i18n::tr_treccani("treccani.sections_label"),
        import_all: i18n::tr_treccani("treccani.import_all_article"),
        import_button: i18n::tr_treccani("treccani.import_button"),
        status_loading: i18n::tr_treccani("treccani.loading"),
        status_no_results: i18n::tr_treccani("treccani.no_results"),
        status_no_query: i18n::tr_treccani("treccani.no_query"),
        status_importing: i18n::tr_treccani("treccani.importing"),
        status_search_error: i18n::tr_treccani("treccani.search_error"),
        status_import_error: i18n::tr_treccani("treccani.import_error"),
        close: i18n::tr_treccani("treccani.close"),
    }
}

pub fn handle_navigation(hwnd: HWND, msg: &MSG) -> bool {
    if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN {
        if msg.wParam.0 as u32 == VK_ESCAPE.0 as u32 {
            if focus_search_after_section_import(hwnd) {
                return true;
            }
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            return true;
        }
        if msg.wParam.0 as u32 == VK_RETURN.0 as u32 {
            let focus = crate::get_focus_safe();
            if let Some((input, search, results, sections, import_button, close)) =
                with_window_state(hwnd, |state| {
                    (
                        state.input,
                        state.search,
                        state.results,
                        state.sections,
                        state.import_button,
                        state.close,
                    )
                })
            {
                if focus == close {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return true;
                }
                if focus == sections || focus == import_button {
                    import_selected_section(hwnd);
                    return true;
                }
                if focus == input || focus == search {
                    run_search(hwnd);
                    return true;
                }
                if focus == results {
                    start_article_load(hwnd);
                    return true;
                }
            }
        }
    }
    handle_accessibility(hwnd, msg)
}

pub fn focus_import_choice(hwnd: HWND) -> bool {
    let Some((sections, import_button)) =
        with_window_state(hwnd, |state| (state.sections, state.import_button))
    else {
        return false;
    };
    let target = if sections.0 != 0 {
        sections
    } else {
        import_button
    };
    if target.0 == 0 {
        return false;
    }
    unsafe {
        SetForegroundWindow(hwnd);
        SetFocus(target);
    }
    true
}

fn focus_search_after_section_import(hwnd: HWND) -> bool {
    let Some((sections, import_button, results, input, has_extract)) =
        with_window_state(hwnd, |state| {
            (
                state.sections,
                state.import_button,
                state.results,
                state.input,
                state.current_extract.is_some(),
            )
        })
    else {
        return false;
    };
    if !has_extract {
        return false;
    }
    let focus = crate::get_focus_safe();
    if focus != sections && focus != import_button {
        return false;
    }
    let target = if results.0 != 0 { results } else { input };
    if target.0 == 0 {
        return false;
    }
    unsafe {
        SetFocus(target);
    }
    true
}

pub fn open(parent: HWND) {
    unsafe {
        let existing = with_state(parent, |state| state.treccani_window).unwrap_or(HWND(0));
        if existing.0 != 0 {
            SetForegroundWindow(existing);
            return;
        }

        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(TRECCANI_CLASS_NAME);
        let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
        let label_set = labels(language);
        let title = to_wide(&label_set.title);

        let wc = windows::Win32::UI::WindowsAndMessaging::WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
            ),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(treccani_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let state = Box::new(TreccaniWindowState {
            parent,
            input: HWND(0),
            language_combo: HWND(0),
            search: HWND(0),
            results: HWND(0),
            status: HWND(0),
            sections: HWND(0),
            import_button: HWND(0),
            close: HWND(0),
            results_data: Vec::new(),
            current_extract: None,
        });
        let state_ptr = Box::into_raw(state);
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            540,
            455,
            parent,
            HMENU(0),
            hinstance,
            Some(state_ptr as *const _),
        );
        if hwnd.0 == 0 {
            let _unused_box = Box::from_raw(state_ptr);
            return;
        }
        with_state(parent, |state| state.treccani_window = hwnd);
    }
}

unsafe extern "system" fn treccani_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "treccani_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || treccani_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn treccani_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const CREATESTRUCTW;
                let init_ptr = (*cs).lpCreateParams as *mut TreccaniWindowState;
                if init_ptr.is_null() {
                    return LRESULT(0);
                }
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, init_ptr as isize);

                let parent = (*init_ptr).parent;
                let language =
                    with_state(parent, |state| state.settings.language).unwrap_or_default();
                let label_set = labels(language);

                let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&label_set.search_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    12,
                    80,
                    20,
                    hwnd,
                    HMENU(0),
                    hinstance,
                    None,
                );
                let input = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    90,
                    10,
                    310,
                    24,
                    hwnd,
                    HMENU(TRECCANI_INPUT_ID as isize),
                    hinstance,
                    None,
                );
                let search = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&label_set.search_button).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    410,
                    10,
                    100,
                    24,
                    hwnd,
                    HMENU(TRECCANI_SEARCH_ID as isize),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&label_set.results_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    78,
                    120,
                    20,
                    hwnd,
                    HMENU(0),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&label_set.language_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    44,
                    120,
                    20,
                    hwnd,
                    HMENU(0),
                    hinstance,
                    None,
                );
                let language_combo = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    90,
                    40,
                    220,
                    240,
                    hwnd,
                    HMENU(TRECCANI_LANGUAGE_ID as isize),
                    hinstance,
                    None,
                );
                populate_language_combo(language_combo, &label_set);
                let results = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_LISTBOXW,
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WS_VSCROLL
                        | WINDOW_STYLE((LBS_NOTIFY | LBS_HASSTRINGS | LBS_NOINTEGRALHEIGHT) as u32),
                    12,
                    100,
                    498,
                    196,
                    hwnd,
                    HMENU(TRECCANI_RESULTS_ID as isize),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&label_set.sections_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    306,
                    80,
                    20,
                    hwnd,
                    HMENU(0),
                    hinstance,
                    None,
                );
                let sections = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    90,
                    302,
                    310,
                    220,
                    hwnd,
                    HMENU(TRECCANI_SECTIONS_ID as isize),
                    hinstance,
                    None,
                );
                let status = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    334,
                    360,
                    20,
                    hwnd,
                    HMENU(TRECCANI_STATUS_ID as isize),
                    hinstance,
                    None,
                );
                let import_button = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&label_set.import_button).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    410,
                    300,
                    100,
                    26,
                    hwnd,
                    HMENU(TRECCANI_IMPORT_ID as isize),
                    hinstance,
                    None,
                );
                let close = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&label_set.close).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    410,
                    330,
                    100,
                    26,
                    hwnd,
                    HMENU(TRECCANI_CLOSE_ID as isize),
                    hinstance,
                    None,
                );

                (*init_ptr).input = input;
                (*init_ptr).language_combo = language_combo;
                (*init_ptr).search = search;
                (*init_ptr).results = results;
                (*init_ptr).sections = sections;
                (*init_ptr).import_button = import_button;
                (*init_ptr).status = status;
                (*init_ptr).close = close;
                let proc_ptr = tab_subclass_proc as *const () as usize;
                for control in [
                    input,
                    language_combo,
                    search,
                    results,
                    sections,
                    import_button,
                    close,
                ] {
                    let prev = SetWindowLongPtrW(
                        control,
                        windows::Win32::UI::WindowsAndMessaging::GWLP_WNDPROC,
                        proc_ptr as isize,
                    );
                    SetWindowLongPtrW(control, GWLP_USERDATA, prev);
                }
                let input_edit = GetWindow(input, GW_CHILD);
                if input_edit.0 != 0 {
                    let prev = SetWindowLongPtrW(
                        input_edit,
                        windows::Win32::UI::WindowsAndMessaging::GWLP_WNDPROC,
                        proc_ptr as isize,
                    );
                    SetWindowLongPtrW(input_edit, GWLP_USERDATA, prev);
                }
                SetFocus(input);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TreccaniWindowState;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                SetFocus((*ptr).input);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 as u32 == VK_ESCAPE.0 as u32 {
                    if focus_search_after_section_import(hwnd) {
                        return LRESULT(0);
                    }
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                if wparam.0 as u32 == VK_RETURN.0 as u32 {
                    let focus = GetFocus();
                    let Some((search, close, results, sections, import_button)) =
                        with_window_state(hwnd, |state| {
                            (
                                state.search,
                                state.close,
                                state.results,
                                state.sections,
                                state.import_button,
                            )
                        })
                    else {
                        return DefWindowProcW(hwnd, msg, wparam, lparam);
                    };
                    if focus == close {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        return LRESULT(0);
                    }
                    if focus == search {
                        run_search(hwnd);
                        return LRESULT(0);
                    }
                    if focus == sections || focus == import_button {
                        import_selected_section(hwnd);
                        return LRESULT(0);
                    }
                    if focus == with_window_state(hwnd, |state| state.input).unwrap_or(HWND(0)) {
                        run_search(hwnd);
                        return LRESULT(0);
                    }
                    if focus == results {
                        start_article_load(hwnd);
                        return LRESULT(0);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                if id == TRECCANI_CLOSE_ID {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                if id == TRECCANI_SEARCH_ID {
                    run_search(hwnd);
                    return LRESULT(0);
                }
                if id == TRECCANI_IMPORT_ID {
                    import_selected_section(hwnd);
                    return LRESULT(0);
                }
                if id == TRECCANI_RESULTS_ID && ((wparam.0 >> 16) & 0xffff) == LBN_DBLCLK as usize {
                    start_article_load(hwnd);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_TRECCANI_SEARCH_DONE => {
                let generation = wparam.0;
                if generation != SEARCH_GENERATION.load(Ordering::SeqCst) {
                    let payload_ptr = lparam.0 as *mut SearchPayload;
                    if !payload_ptr.is_null() {
                        let _unused_box = Box::from_raw(payload_ptr);
                    }
                    return LRESULT(0);
                }
                let payload_ptr = lparam.0 as *mut SearchPayload;
                if payload_ptr.is_null() {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(payload_ptr);
                let language = with_window_state(hwnd, |state| state.parent)
                    .and_then(|parent| with_state(parent, |s| s.settings.language))
                    .unwrap_or_default();
                let label_set = labels(language);
                let (results_hwnd, status_hwnd) =
                    with_window_state(hwnd, |state| (state.results, state.status))
                        .unwrap_or((HWND(0), HWND(0)));
                let results = payload.results;
                let has_error = payload.error.is_some();
                let is_empty = results.is_empty();
                if results_hwnd.0 != 0 {
                    SendMessageW(results_hwnd, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
                    for display in crate::result_labels::accessible_result_labels(
                        results.iter().map(result_display_label).collect(),
                    ) {
                        SendMessageW(
                            results_hwnd,
                            LB_ADDSTRING,
                            WPARAM(0),
                            LPARAM(to_wide(&display).as_ptr() as isize),
                        );
                    }
                    if !results.is_empty() {
                        SendMessageW(results_hwnd, LB_SETCURSEL, WPARAM(0), LPARAM(0));
                        SetFocus(results_hwnd);
                    }
                }
                with_window_state(hwnd, |state| state.results_data = results);
                let status_text = if has_error {
                    label_set.status_search_error
                } else if is_empty {
                    label_set.status_no_results
                } else {
                    String::new()
                };
                if status_hwnd.0 != 0
                    && let Err(e) =
                        SetWindowTextW(status_hwnd, PCWSTR(to_wide(&status_text).as_ptr()))
                {
                    crate::log_debug(&format!("SetWindowTextW failed: {}", e));
                }
                LRESULT(0)
            }
            WM_TRECCANI_IMPORT_DONE => {
                let generation = wparam.0;
                if generation != IMPORT_GENERATION.load(Ordering::SeqCst) {
                    let payload_ptr = lparam.0 as *mut ImportPayload;
                    if !payload_ptr.is_null() {
                        let _unused_box = Box::from_raw(payload_ptr);
                    }
                    return LRESULT(0);
                }
                let payload_ptr = lparam.0 as *mut ImportPayload;
                if payload_ptr.is_null() {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(payload_ptr);
                let parent = with_window_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
                let language =
                    with_state(parent, |state| state.settings.language).unwrap_or_default();
                let label_set = labels(language);
                if let Some(error) = payload.error {
                    show_error(
                        parent,
                        language,
                        &format!("{} {error}", label_set.status_import_error),
                    );
                    return LRESULT(0);
                }
                let extract = if let Some(extract) = payload.extract {
                    extract
                } else {
                    show_error(parent, language, &label_set.status_import_error);
                    return LRESULT(0);
                };
                populate_sections(hwnd, extract, &label_set);
                LRESULT(0)
            }
            WM_DESTROY => LRESULT(0),
            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TreccaniWindowState;
                if !ptr.is_null() {
                    let state = Box::from_raw(ptr);
                    with_state(state.parent, |s| s.treccani_window = HWND(0));
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn handle_enter_key(hwnd: HWND) -> bool {
    let parent = crate::get_parent_safe(hwnd);
    if parent.0 == 0 {
        return false;
    }
    let id = crate::get_dlg_ctrl_id_safe(hwnd);
    if id == TRECCANI_CLOSE_ID {
        crate::log_if_err!(crate::destroy_window_safe(parent));
        return true;
    }
    if id == TRECCANI_SEARCH_ID {
        run_search(parent);
        return true;
    }
    if id == TRECCANI_INPUT_ID {
        run_search(parent);
        return true;
    }
    if id == TRECCANI_LANGUAGE_ID {
        run_search(parent);
        return true;
    }
    if id == TRECCANI_RESULTS_ID {
        start_article_load(parent);
        return true;
    }
    if id == TRECCANI_SECTIONS_ID || id == TRECCANI_IMPORT_ID {
        import_selected_section(parent);
        return true;
    }
    false
}

unsafe extern "system" fn tab_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "tab_subclass_proc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || tab_subclass_proc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn tab_subclass_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN {
        let key = wparam.0 as u16;
        if key == windows::Win32::UI::Input::KeyboardAndMouse::VK_TAB.0 {
            let shift_down = unsafe {
                windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(
                    windows::Win32::UI::Input::KeyboardAndMouse::VK_SHIFT.0 as i32,
                )
            } & 0x8000u16 as i16
                != 0;
            let parent = crate::get_parent_safe(hwnd);
            if parent.0 != 0 {
                crate::log_debug(&format!(
                    "treccani_tab_keydown: hwnd={:?} id={} shift={} focus_before={:?}",
                    hwnd,
                    crate::get_dlg_ctrl_id_safe(hwnd),
                    shift_down,
                    crate::get_focus_safe()
                ));
                log_treccani_tab(parent, hwnd, shift_down);
                focus_next_control(parent, hwnd, shift_down);
                return LRESULT(0);
            }
        }
        if key == windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN.0 && handle_enter_key(hwnd)
        {
            return LRESULT(0);
        }
    }
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_CHAR
        && wparam.0 == 13
        && handle_enter_key(hwnd)
    {
        return LRESULT(0);
    }
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_CHAR && wparam.0 == 9 {
        crate::log_debug(&format!(
            "treccani_tab_char: hwnd={:?} id={} focus={:?}",
            hwnd,
            crate::get_dlg_ctrl_id_safe(hwnd),
            crate::get_focus_safe()
        ));
        return LRESULT(0);
    }
    if msg == WM_SETFOCUS {
        crate::log_debug(&format!(
            "treccani_control_focus: hwnd={:?} id={} previous={:?}",
            hwnd,
            crate::get_dlg_ctrl_id_safe(hwnd),
            HWND(wparam.0 as isize)
        ));
    }
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_GETDLGCODE {
        let id = crate::get_dlg_ctrl_id_safe(hwnd);
        if matches!(
            id,
            TRECCANI_INPUT_ID
                | TRECCANI_LANGUAGE_ID
                | TRECCANI_CLOSE_ID
                | TRECCANI_SEARCH_ID
                | TRECCANI_RESULTS_ID
                | TRECCANI_IMPORT_ID
        ) {
            return LRESULT(windows::Win32::UI::WindowsAndMessaging::DLGC_WANTALLKEYS as isize);
        }
    }
    let prev = crate::get_window_long_ptr_w_safe(
        hwnd,
        windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
    );
    if prev == 0 {
        return crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam);
    }
    crate::call_window_proc_w_safe(
        crate::isize_to_wndproc_safe(prev),
        hwnd,
        msg,
        wparam,
        lparam,
    )
}

fn focus_next_control(parent: HWND, current: HWND, shift_down: bool) {
    let order = with_window_state(parent, |state| {
        vec![
            state.input,
            state.language_combo,
            state.search,
            state.results,
            state.sections,
            state.import_button,
            state.close,
        ]
    })
    .unwrap_or_default();
    if order.is_empty() {
        return;
    }
    let mut current_ctrl = current;
    let mut pos = order.iter().position(|hwnd| *hwnd == current_ctrl);
    if pos.is_none() {
        let maybe_parent = crate::get_parent_safe(current_ctrl);
        if maybe_parent.0 != 0 {
            current_ctrl = maybe_parent;
            pos = order.iter().position(|hwnd| *hwnd == current_ctrl);
        }
    }
    let Some(pos) = pos else {
        return;
    };
    let next_index = if shift_down {
        if pos == 0 { order.len() - 1 } else { pos - 1 }
    } else if pos + 1 >= order.len() {
        0
    } else {
        pos + 1
    };
    let target = order[next_index];
    if target.0 != 0 {
        unsafe {
            SetFocus(target);
        }
        crate::log_debug(&format!(
            "treccani_tab_focus: shift={} current={:?} target={:?}",
            shift_down, current, target
        ));
    }
}

fn log_treccani_tab(parent: HWND, current: HWND, shift_down: bool) {
    let current_focus = crate::get_focus_safe();
    let Some((input, language_combo, search, results, sections, import_button, close)) =
        with_window_state(parent, |state| {
            (
                state.input,
                state.language_combo,
                state.search,
                state.results,
                state.sections,
                state.import_button,
                state.close,
            )
        })
    else {
        crate::log_debug(&format!(
            "treccani_tab: missing_state shift={} current={:?} focus={:?}",
            shift_down, current, current_focus
        ));
        return;
    };
    crate::log_debug(&format!(
        "treccani_tab: shift={} current={:?} focus={:?} input={:?} language={:?} search={:?} results={:?} sections={:?} import={:?} close={:?}",
        shift_down,
        current,
        current_focus,
        input,
        language_combo,
        search,
        results,
        sections,
        import_button,
        close
    ));
}

fn with_window_state<F, R>(hwnd: HWND, f: F) -> Option<R>
where
    F: FnOnce(&mut TreccaniWindowState) -> R,
{
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut TreccaniWindowState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn populate_language_combo(combo: HWND, label_set: &TreccaniLabels) {
    if combo.0 == 0 {
        return;
    }
    crate::send_message_w_safe(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    crate::send_message_w_safe(
        combo,
        CB_ADDSTRING,
        WPARAM(0),
        LPARAM(to_wide(&label_set.language_auto).as_ptr() as isize),
    );
    crate::send_message_w_safe(combo, CB_SETCURSEL, WPARAM(0), LPARAM(0));
}

fn run_search(hwnd: HWND) {
    let Some((input, results, status)) =
        with_window_state(hwnd, |state| (state.input, state.results, state.status))
    else {
        return;
    };
    let parent = with_window_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    let label_set = labels(language);

    let len = crate::get_window_text_length_w_safe(input);
    if len <= 0 {
        if status.0 != 0 {
            crate::log_if_err!(crate::set_window_text_w_safe(
                status,
                PCWSTR(to_wide(&label_set.status_no_query).as_ptr()),
            ));
        }
        if results.0 != 0 {
            crate::send_message_w_safe(results, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
        }
        return;
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let _read = crate::get_window_text_w_safe(input, &mut buf);
    let query = String::from_utf16_lossy(&buf[..len as usize]);
    let trimmed = query.trim().to_string();
    if trimmed.is_empty() {
        if status.0 != 0 {
            crate::log_if_err!(crate::set_window_text_w_safe(
                status,
                PCWSTR(to_wide(&label_set.status_no_query).as_ptr()),
            ));
        }
        if results.0 != 0 {
            crate::send_message_w_safe(results, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
        }
        return;
    }
    if status.0 != 0 {
        crate::log_if_err!(crate::set_window_text_w_safe(
            status,
            PCWSTR(to_wide(&label_set.status_loading).as_ptr()),
        ));
    }
    if results.0 != 0 {
        crate::send_message_w_safe(results, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    }
    reset_sections(hwnd, &label_set.import_all);

    let generation = SEARCH_GENERATION
        .fetch_add(1, Ordering::SeqCst)
        .wrapping_add(1);
    SEARCH_GENERATION.store(generation, Ordering::SeqCst);

    let hwnd_val = hwnd.0;
    std::thread::spawn(move || {
        let results = treccani::search_articles(&trimmed, 20);
        let payload = match results {
            Ok(list) => SearchPayload {
                results: list,
                error: None,
            },
            Err(err) => SearchPayload {
                results: Vec::new(),
                error: Some(err.to_string()),
            },
        };
        let payload_ptr = Box::into_raw(Box::new(payload));
        let hwnd = HWND(hwnd_val);
        unsafe {
            if IsWindow(hwnd).as_bool() {
                if let Err(e) = PostMessageW(
                    hwnd,
                    WM_TRECCANI_SEARCH_DONE,
                    WPARAM(generation),
                    LPARAM(payload_ptr as isize),
                ) {
                    crate::log_debug(&format!("Failed to post WM_TRECCANI_SEARCH_DONE: {e}"));
                    let _unused_box = Box::from_raw(payload_ptr);
                }
            } else {
                let _unused_box = Box::from_raw(payload_ptr);
            }
        }
    });
}

fn start_article_load(hwnd: HWND) {
    let Some((parent, results_hwnd)) =
        with_window_state(hwnd, |state| (state.parent, state.results))
    else {
        return;
    };
    let sel = crate::send_message_w_safe(results_hwnd, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
    if sel < 0 {
        return;
    }
    let selection =
        with_window_state(hwnd, |state| state.results_data.get(sel as usize).cloned()).flatten();
    let Some(selection) = selection else {
        return;
    };
    crate::log_debug(&format!(
        "Treccani: selected result url={} title={}",
        selection.url, selection.title
    ));
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    let label_set = labels(language);
    if let Some(status) = with_window_state(hwnd, |state| state.status) {
        crate::log_if_err!(crate::set_window_text_w_safe(
            status,
            PCWSTR(to_wide(&label_set.status_importing).as_ptr()),
        ));
    }
    let generation = IMPORT_GENERATION
        .fetch_add(1, Ordering::SeqCst)
        .wrapping_add(1);
    IMPORT_GENERATION.store(generation, Ordering::SeqCst);

    let hwnd_val = hwnd.0;
    std::thread::spawn(move || {
        let result = treccani::fetch_extract(&selection.url);
        let payload = match result {
            Ok(extract) => ImportPayload {
                extract: Some(extract),
                error: None,
            },
            Err(err) => ImportPayload {
                extract: None,
                error: Some(err.to_string()),
            },
        };
        let payload_ptr = Box::into_raw(Box::new(payload));
        let hwnd = HWND(hwnd_val);
        unsafe {
            if IsWindow(hwnd).as_bool() {
                if let Err(e) = PostMessageW(
                    hwnd,
                    WM_TRECCANI_IMPORT_DONE,
                    WPARAM(generation),
                    LPARAM(payload_ptr as isize),
                ) {
                    crate::log_debug(&format!("Failed to post WM_TRECCANI_IMPORT_DONE: {e}"));
                    let _unused_box = Box::from_raw(payload_ptr);
                }
            } else {
                let _unused_box = Box::from_raw(payload_ptr);
            }
        }
    });
}

fn result_display_label(result: &treccani::SearchResult) -> String {
    let description = result.description.trim();
    if description.is_empty() {
        result.title.clone()
    } else {
        format!("{} — {}", result.title, description)
    }
}

fn reset_sections(hwnd: HWND, import_all_label: &str) {
    let Some(sections) = with_window_state(hwnd, |state| state.sections) else {
        return;
    };
    with_window_state(hwnd, |state| state.current_extract = None);
    if sections.0 == 0 {
        return;
    }
    crate::send_message_w_safe(sections, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    crate::send_message_w_safe(
        sections,
        CB_ADDSTRING,
        WPARAM(0),
        LPARAM(to_wide(import_all_label).as_ptr() as isize),
    );
    crate::send_message_w_safe(sections, CB_SETCURSEL, WPARAM(0), LPARAM(0));
}

fn populate_sections(hwnd: HWND, extract: treccani::ExtractResult, label_set: &TreccaniLabels) {
    let Some((sections, status)) = with_window_state(hwnd, |state| (state.sections, state.status))
    else {
        return;
    };
    crate::log_debug(&format!(
        "Treccani: populate combo section_count={}",
        extract.sections.len()
    ));
    if sections.0 != 0 {
        crate::send_message_w_safe(sections, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        crate::send_message_w_safe(
            sections,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(to_wide(&label_set.import_all).as_ptr() as isize),
        );
        for label in crate::result_labels::accessible_result_labels(
            extract.sections.iter().map(section_combo_label).collect(),
        ) {
            crate::send_message_w_safe(
                sections,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&label).as_ptr() as isize),
            );
        }
        crate::send_message_w_safe(sections, CB_SETCURSEL, WPARAM(0), LPARAM(0));
        unsafe {
            SetFocus(sections);
        }
    }
    with_window_state(hwnd, |state| state.current_extract = Some(extract));
    if status.0 != 0
        && let Err(e) = crate::set_window_text_w_safe(status, PCWSTR(to_wide("").as_ptr()))
    {
        crate::log_debug(&format!("SetWindowTextW failed: {}", e));
    }
}

fn section_combo_label(section: &treccani::ArticleSection) -> String {
    if section.level <= 2 {
        section.title.clone()
    } else {
        format!("{}{}", "  ".repeat(section.level - 2), section.title)
    }
}

fn import_selected_section(hwnd: HWND) {
    let Some((parent, sections, extract)) = with_window_state(hwnd, |state| {
        (state.parent, state.sections, state.current_extract.clone())
    }) else {
        return;
    };
    let Some(extract) = extract else {
        start_article_load(hwnd);
        return;
    };
    let sel = crate::send_message_w_safe(sections, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
    let body = if sel > 0 {
        crate::log_debug(&format!("Treccani: importing section index={sel}"));
        extract
            .sections
            .get((sel - 1) as usize)
            .map(|section| section.text.as_str())
            .unwrap_or(extract.extract.as_str())
    } else {
        crate::log_debug("Treccani: importing whole article");
        extract.extract.as_str()
    };
    let mut text = body.trim_end().to_string();
    text.push_str("\n\nFonte: Enciclopedia Treccani\n");
    text.push_str(&extract.url);

    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    let label_set = labels(language);
    if !apply_import_text(parent, &text) {
        show_error(parent, language, &label_set.status_import_error);
        return;
    }
    editor_manager::mark_current_document_from_treccani(parent, true);
}

fn force_focus_editor_on_parent(parent: HWND) {
    if parent.0 == 0 {
        return;
    }
    unsafe {
        SetForegroundWindow(parent);
        SendMessageW(
            parent,
            windows::Win32::UI::WindowsAndMessaging::WM_SETFOCUS,
            WPARAM(0),
            LPARAM(0),
        );
    }
    if get_active_edit(parent).is_none() {
        crate::send_message_w_safe(
            parent,
            WM_COMMAND,
            WPARAM(crate::menu::IDM_FILE_NEW),
            LPARAM(0),
        );
    }
    if let Some(hwnd_edit) = get_active_edit(parent) {
        unsafe {
            SetFocus(hwnd_edit);
            SendMessageW(hwnd_edit, EM_SETSEL, WPARAM(0), LPARAM(0));
            SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
            NotifyWinEvent(
                windows::Win32::UI::WindowsAndMessaging::EVENT_OBJECT_FOCUS,
                hwnd_edit,
                windows::Win32::UI::WindowsAndMessaging::OBJID_CLIENT.0,
                windows::Win32::UI::WindowsAndMessaging::CHILDID_SELF as i32,
            );
        }
    }
    crate::log_if_err!(crate::post_message_w_safe(
        parent,
        WM_FOCUS_EDITOR,
        WPARAM(0),
        LPARAM(0),
    ));
}

fn apply_import_text(parent: HWND, text: &str) -> bool {
    force_focus_editor_on_parent(parent);
    let Some(hwnd_edit) = get_active_edit(parent) else {
        return false;
    };
    // Large Treccani extracts can exceed the default Win32 edit text limit.
    unsafe {
        SendMessageW(hwnd_edit, EM_LIMITTEXT, WPARAM(0x7FFF_FFFEusize), LPARAM(0));
    }
    let cleaned = normalize_to_crlf(text);
    let existing = get_edit_text(hwnd_edit);
    let replace_existing = editor_manager::current_document_is_from_treccani(parent);
    if replace_existing {
        crate::log_debug("Treccani: replacing existing Treccani document content");
    }
    let combined = compose_import_text(cleaned, existing, replace_existing);
    let wide = to_wide_normalized(&combined);
    unsafe {
        SendMessageW(hwnd_edit, EM_SETSEL, WPARAM(0), LPARAM(-1));
        SendMessageW(
            hwnd_edit,
            crate::accessibility::EM_REPLACESEL,
            WPARAM(1),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    let cr = CHARRANGE { cpMin: 0, cpMax: 0 };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&cr as *const _ as isize),
        );
        SendMessageW(hwnd_edit, EM_SETSEL, WPARAM(0), LPARAM(0));
        SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
        NotifyWinEvent(
            windows::Win32::UI::WindowsAndMessaging::EVENT_OBJECT_VALUECHANGE,
            hwnd_edit,
            windows::Win32::UI::WindowsAndMessaging::OBJID_CLIENT.0,
            windows::Win32::UI::WindowsAndMessaging::CHILDID_SELF as i32,
        );
        NotifyWinEvent(
            windows::Win32::UI::WindowsAndMessaging::EVENT_OBJECT_FOCUS,
            hwnd_edit,
            windows::Win32::UI::WindowsAndMessaging::OBJID_CLIENT.0,
            windows::Win32::UI::WindowsAndMessaging::CHILDID_SELF as i32,
        );
    }
    true
}

fn compose_import_text(cleaned: String, existing: String, replace_existing: bool) -> String {
    if replace_existing || existing.is_empty() {
        cleaned
    } else {
        format!("{cleaned}\r\n\r\n{existing}")
    }
}

#[cfg(test)]
mod tests {
    use super::compose_import_text;

    #[test]
    fn treccani_import_replaces_content_in_a_treccani_document() {
        let result = compose_import_text(
            "Nuovo articolo".to_string(),
            "Articolo precedente".to_string(),
            true,
        );
        assert_eq!(result, "Nuovo articolo");
    }

    #[test]
    fn treccani_import_preserves_a_non_treccani_document() {
        let result = compose_import_text(
            "Articolo Treccani".to_string(),
            "Testo dell'utente".to_string(),
            false,
        );
        assert_eq!(result, "Articolo Treccani\r\n\r\nTesto dell'utente");
    }

    #[test]
    fn treccani_import_uses_an_empty_document_directly() {
        let result = compose_import_text("Articolo Treccani".to_string(), String::new(), false);
        assert_eq!(result, "Articolo Treccani");
    }
}
