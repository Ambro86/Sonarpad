use crate::accessibility::{handle_accessibility, normalize_to_crlf, to_wide, to_wide_normalized};
use crate::editor_manager::{self, get_edit_text};
use crate::i18n;
use crate::settings::{Language, save_settings};
use crate::wikipedia;
use crate::{WM_FOCUS_EDITOR, get_active_edit, show_error, with_state};
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::Controls::RichEdit::{CHARRANGE, EM_EXGETSEL, EM_EXSETSEL};
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

const WIKIPEDIA_CLASS_NAME: &str = "SonarpadWikipediaImport";
const WIKIPEDIA_INPUT_ID: usize = 9501;
const WIKIPEDIA_SEARCH_ID: usize = 9502;
const WIKIPEDIA_RESULTS_ID: usize = 9503;
const WIKIPEDIA_STATUS_ID: usize = 9504;
const WIKIPEDIA_CLOSE_ID: usize = 9505;
const WIKIPEDIA_SECTIONS_ID: usize = 9506;
const WIKIPEDIA_IMPORT_ID: usize = 9507;
const WIKIPEDIA_LANGUAGE_ID: usize = 9508;

const WM_WIKI_SEARCH_DONE: u32 = WM_APP + 110;
const WM_WIKI_IMPORT_DONE: u32 = WM_APP + 111;
static SEARCH_GENERATION: AtomicUsize = AtomicUsize::new(0);
static IMPORT_GENERATION: AtomicUsize = AtomicUsize::new(0);

struct WikipediaWindowState {
    parent: HWND,
    input: HWND,
    language_combo: HWND,
    search: HWND,
    results: HWND,
    sections: HWND,
    import_button: HWND,
    status: HWND,
    close: HWND,
    results_data: Vec<wikipedia::SearchResult>,
    current_extract: Option<wikipedia::ExtractResult>,
}

struct WikipediaLabels {
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
    results: Vec<wikipedia::SearchResult>,
    error: Option<String>,
}

struct ImportPayload {
    extract: Option<wikipedia::ExtractResult>,
    error: Option<String>,
}

fn labels(language: Language) -> WikipediaLabels {
    WikipediaLabels {
        title: i18n::tr(language, "wikipedia.title"),
        search_label: i18n::tr(language, "wikipedia.search_label"),
        language_label: i18n::tr(language, "options.label.wikipedia_language"),
        language_auto: i18n::tr(language, "dictionary.lookup.language.auto"),
        search_button: i18n::tr(language, "wikipedia.search_button"),
        results_label: i18n::tr(language, "wikipedia.results_label"),
        sections_label: tr_or(language, "wikipedia.sections_label", "Import:"),
        import_all: tr_or(
            language,
            "wikipedia.import_all_article",
            "Import whole article",
        ),
        import_button: tr_or(language, "wikipedia.import_button", "Import"),
        status_loading: i18n::tr(language, "wikipedia.loading"),
        status_no_results: i18n::tr(language, "wikipedia.no_results"),
        status_no_query: i18n::tr(language, "wikipedia.no_query"),
        status_importing: i18n::tr(language, "wikipedia.importing"),
        status_search_error: i18n::tr(language, "wikipedia.search_error"),
        status_import_error: i18n::tr(language, "wikipedia.import_error"),
        close: i18n::tr(language, "wikipedia.close"),
    }
}

fn tr_or(language: Language, key: &str, fallback: &str) -> String {
    let value = i18n::tr(language, key);
    if value == key {
        fallback.to_string()
    } else {
        value
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
        let existing = with_state(parent, |state| state.wikipedia_window).unwrap_or(HWND(0));
        if existing.0 != 0 {
            SetForegroundWindow(existing);
            return;
        }

        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(WIKIPEDIA_CLASS_NAME);
        let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
        let label_set = labels(language);
        let title = to_wide(&label_set.title);

        let wc = windows::Win32::UI::WindowsAndMessaging::WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
            ),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(wikipedia_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let state = Box::new(WikipediaWindowState {
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
            640,
            475,
            parent,
            HMENU(0),
            hinstance,
            Some(state_ptr as *const _),
        );
        if hwnd.0 == 0 {
            let _unused_box = Box::from_raw(state_ptr);
            return;
        }
        with_state(parent, |state| state.wikipedia_window = hwnd);
    }
}

unsafe extern "system" fn wikipedia_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "wikipedia_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || wikipedia_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn wikipedia_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const CREATESTRUCTW;
                let init_ptr = (*cs).lpCreateParams as *mut WikipediaWindowState;
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
                    16,
                    14,
                    130,
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
                    152,
                    10,
                    350,
                    26,
                    hwnd,
                    HMENU(WIKIPEDIA_INPUT_ID as isize),
                    hinstance,
                    None,
                );
                let search = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&label_set.search_button).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    512,
                    10,
                    100,
                    26,
                    hwnd,
                    HMENU(WIKIPEDIA_SEARCH_ID as isize),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&label_set.results_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    82,
                    130,
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
                    16,
                    46,
                    130,
                    32,
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
                    152,
                    42,
                    300,
                    240,
                    hwnd,
                    HMENU(WIKIPEDIA_LANGUAGE_ID as isize),
                    hinstance,
                    None,
                );
                populate_language_combo(parent, language_combo, language, &label_set);
                let results = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_LISTBOXW,
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WS_VSCROLL
                        | WINDOW_STYLE((LBS_NOTIFY | LBS_HASSTRINGS | LBS_NOINTEGRALHEIGHT) as u32),
                    16,
                    104,
                    596,
                    200,
                    hwnd,
                    HMENU(WIKIPEDIA_RESULTS_ID as isize),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&label_set.sections_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    318,
                    130,
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
                    152,
                    312,
                    350,
                    220,
                    hwnd,
                    HMENU(WIKIPEDIA_SECTIONS_ID as isize),
                    hinstance,
                    None,
                );
                let status = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    350,
                    460,
                    20,
                    hwnd,
                    HMENU(WIKIPEDIA_STATUS_ID as isize),
                    hinstance,
                    None,
                );
                let import_button = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&label_set.import_button).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    512,
                    310,
                    100,
                    28,
                    hwnd,
                    HMENU(WIKIPEDIA_IMPORT_ID as isize),
                    hinstance,
                    None,
                );
                let close = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&label_set.close).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    512,
                    346,
                    100,
                    28,
                    hwnd,
                    HMENU(WIKIPEDIA_CLOSE_ID as isize),
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
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WikipediaWindowState;
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
                if id == WIKIPEDIA_CLOSE_ID {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                if id == WIKIPEDIA_SEARCH_ID {
                    run_search(hwnd);
                    return LRESULT(0);
                }
                if id == WIKIPEDIA_IMPORT_ID {
                    import_selected_section(hwnd);
                    return LRESULT(0);
                }
                if id == WIKIPEDIA_RESULTS_ID && ((wparam.0 >> 16) & 0xffff) == LBN_DBLCLK as usize
                {
                    start_article_load(hwnd);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_WIKI_SEARCH_DONE => {
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
                        results.iter().map(|item| item.title.clone()).collect(),
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
            WM_WIKI_IMPORT_DONE => {
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
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WikipediaWindowState;
                if !ptr.is_null() {
                    let state = Box::from_raw(ptr);
                    with_state(state.parent, |s| s.wikipedia_window = HWND(0));
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
    if id == WIKIPEDIA_CLOSE_ID {
        crate::log_if_err!(crate::destroy_window_safe(parent));
        return true;
    }
    if id == WIKIPEDIA_SEARCH_ID {
        run_search(parent);
        return true;
    }
    if id == WIKIPEDIA_INPUT_ID {
        run_search(parent);
        return true;
    }
    if id == WIKIPEDIA_LANGUAGE_ID {
        run_search(parent);
        return true;
    }
    if id == WIKIPEDIA_RESULTS_ID {
        start_article_load(parent);
        return true;
    }
    if id == WIKIPEDIA_SECTIONS_ID || id == WIKIPEDIA_IMPORT_ID {
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
                    "wikipedia_tab_keydown: hwnd={:?} id={} shift={} focus_before={:?}",
                    hwnd,
                    crate::get_dlg_ctrl_id_safe(hwnd),
                    shift_down,
                    crate::get_focus_safe()
                ));
                log_wikipedia_tab(parent, hwnd, shift_down);
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
            "wikipedia_tab_char: hwnd={:?} id={} focus={:?}",
            hwnd,
            crate::get_dlg_ctrl_id_safe(hwnd),
            crate::get_focus_safe()
        ));
        return LRESULT(0);
    }
    if msg == WM_SETFOCUS {
        crate::log_debug(&format!(
            "wikipedia_control_focus: hwnd={:?} id={} previous={:?}",
            hwnd,
            crate::get_dlg_ctrl_id_safe(hwnd),
            HWND(wparam.0 as isize)
        ));
    }
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_GETDLGCODE {
        let id = crate::get_dlg_ctrl_id_safe(hwnd);
        if matches!(
            id,
            WIKIPEDIA_INPUT_ID
                | WIKIPEDIA_LANGUAGE_ID
                | WIKIPEDIA_CLOSE_ID
                | WIKIPEDIA_SEARCH_ID
                | WIKIPEDIA_RESULTS_ID
                | WIKIPEDIA_IMPORT_ID
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
            "wikipedia_tab_focus: shift={} current={:?} target={:?}",
            shift_down, current, target
        ));
    }
}

fn log_wikipedia_tab(parent: HWND, current: HWND, shift_down: bool) {
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
            "wikipedia_tab: missing_state shift={} current={:?} focus={:?}",
            shift_down, current, current_focus
        ));
        return;
    };
    crate::log_debug(&format!(
        "wikipedia_tab: shift={} current={:?} focus={:?} input={:?} language={:?} search={:?} results={:?} sections={:?} import={:?} close={:?}",
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
    F: FnOnce(&mut WikipediaWindowState) -> R,
{
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut WikipediaWindowState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn populate_language_combo(
    parent: HWND,
    combo: HWND,
    language: Language,
    label_set: &WikipediaLabels,
) {
    if combo.0 == 0 {
        return;
    }
    let language_options = [
        (label_set.language_auto.clone(), "auto"),
        (i18n::tr(language, "options.lang.it"), "it"),
        (i18n::tr(language, "options.lang.en"), "en"),
        (i18n::tr(language, "options.lang.es"), "es"),
        (i18n::tr(language, "options.lang.pt"), "pt"),
        (i18n::tr(language, "options.lang.sv"), "sv"),
        (i18n::tr(language, "options.lang.vi"), "vi"),
        (i18n::tr(language, "options.lang.cs"), "cs"),
        (i18n::tr(language, "options.lang.pl"), "pl"),
        (i18n::tr(language, "options.lang.fr"), "fr"),
        (i18n::tr(language, "options.lang.sr"), "sr"),
        (i18n::tr(language, "options.lang.uk"), "uk"),
        (i18n::tr(language, "options.lang.lt"), "lt"),
        (i18n::tr(language, "options.lang.zh"), "zh"),
        (i18n::tr(language, "options.lang.hi"), "hi"),
    ];
    let saved_pref = with_state(parent, |state| state.settings.wikipedia_language.clone())
        .unwrap_or_else(|| "auto".to_string())
        .trim()
        .to_ascii_lowercase();
    let mut selected_index = 0usize;
    for (idx, (label, value)) in language_options.iter().enumerate() {
        crate::send_message_w_safe(
            combo,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(to_wide(label).as_ptr() as isize),
        );
        if saved_pref == *value {
            selected_index = idx;
        }
    }
    crate::send_message_w_safe(combo, CB_SETCURSEL, WPARAM(selected_index), LPARAM(0));
}

fn selected_language_pref(combo: HWND) -> String {
    let sel = crate::send_message_w_safe(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    match sel {
        1 => "it",
        2 => "en",
        3 => "es",
        4 => "pt",
        5 => "sv",
        6 => "vi",
        7 => "cs",
        8 => "pl",
        9 => "fr",
        10 => "sr",
        11 => "uk",
        12 => "lt",
        13 => "zh",
        14 => "hi",
        _ => "auto",
    }
    .to_string()
}

fn run_search(hwnd: HWND) {
    let Some((parent, input, language_combo, results, status)) = with_window_state(hwnd, |state| {
        (
            state.parent,
            state.input,
            state.language_combo,
            state.results,
            state.status,
        )
    }) else {
        return;
    };
    let language = { with_state(parent, |s| s.settings.language) }.unwrap_or_default();
    let pref = selected_language_pref(language_combo);
    let label_set = labels(language);

    let len = crate::get_window_text_length_w_safe(input);
    if len <= 0 {
        if status.0 != 0
            && let Err(e) = crate::set_window_text_w_safe(
                status,
                PCWSTR(to_wide(&label_set.status_no_query).as_ptr()),
            )
        {
            crate::log_debug(&format!("SetWindowTextW failed: {}", e));
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
        if status.0 != 0
            && let Err(e) = crate::set_window_text_w_safe(
                status,
                PCWSTR(to_wide(&label_set.status_no_query).as_ptr()),
            )
        {
            crate::log_debug(&format!("SetWindowTextW failed: {}", e));
        }
        if results.0 != 0 {
            crate::send_message_w_safe(results, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
        }
        return;
    }
    if status.0 != 0
        && let Err(e) = crate::set_window_text_w_safe(
            status,
            PCWSTR(to_wide(&label_set.status_loading).as_ptr()),
        )
    {
        crate::log_debug(&format!("SetWindowTextW failed: {}", e));
    }
    if results.0 != 0 {
        crate::send_message_w_safe(results, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    }
    reset_sections(hwnd, &label_set.import_all);
    if let Some(settings_clone) = with_state(parent, |state| {
        state.settings.wikipedia_language = pref.clone();
        state.settings.clone()
    }) {
        save_settings(settings_clone);
    }

    let generation = SEARCH_GENERATION
        .fetch_add(1, Ordering::SeqCst)
        .wrapping_add(1);
    SEARCH_GENERATION.store(generation, Ordering::SeqCst);

    let hwnd_val = hwnd.0;
    std::thread::spawn(move || {
        let lang_code = wikipedia::resolve_language_code(language, &pref);
        let results = wikipedia::search_articles(&lang_code, &trimmed, 20);
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
                    WM_WIKI_SEARCH_DONE,
                    WPARAM(generation),
                    LPARAM(payload_ptr as isize),
                ) {
                    crate::log_debug(&format!("Failed to post WM_WIKI_SEARCH_DONE: {}", e));
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
    let (language, pref, selection) = with_window_state(hwnd, |state| {
        (
            { with_state(parent, |s| s.settings.language) }.unwrap_or_default(),
            { with_state(parent, |s| s.settings.wikipedia_language.clone()) }
                .unwrap_or_else(|| "auto".to_string()),
            state.results_data.get(sel as usize).cloned(),
        )
    })
    .unwrap_or((Language::default(), "auto".to_string(), None));
    let Some(selection) = selection else {
        return;
    };
    crate::log_debug(&format!(
        "Wikipedia import: selected result pageid={} title={}",
        selection.pageid, selection.title
    ));
    let label_set = labels(language);
    if let Some(status) = with_window_state(hwnd, |state| state.status)
        && let Err(e) = crate::set_window_text_w_safe(
            status,
            PCWSTR(to_wide(&label_set.status_importing).as_ptr()),
        )
    {
        crate::log_debug(&format!("SetWindowTextW failed: {}", e));
    }
    let generation = IMPORT_GENERATION
        .fetch_add(1, Ordering::SeqCst)
        .wrapping_add(1);
    IMPORT_GENERATION.store(generation, Ordering::SeqCst);

    let hwnd_val = hwnd.0;
    std::thread::spawn(move || {
        let lang_code = wikipedia::resolve_language_code(language, &pref);
        let result = wikipedia::fetch_extract(&lang_code, selection.pageid);
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
                    WM_WIKI_IMPORT_DONE,
                    WPARAM(generation),
                    LPARAM(payload_ptr as isize),
                ) {
                    crate::log_debug(&format!("Failed to post WM_WIKI_IMPORT_DONE: {}", e));
                    let _unused_box = Box::from_raw(payload_ptr);
                }
            } else {
                let _unused_box = Box::from_raw(payload_ptr);
            }
        }
    });
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

fn populate_sections(hwnd: HWND, extract: wikipedia::ExtractResult, label_set: &WikipediaLabels) {
    let Some((sections, status)) = with_window_state(hwnd, |state| (state.sections, state.status))
    else {
        return;
    };
    crate::log_debug(&format!(
        "Wikipedia import: populate combo section_count={}",
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

fn section_combo_label(section: &wikipedia::ArticleSection) -> String {
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
        crate::log_debug(&format!("Wikipedia import: importing section index={sel}"));
        extract
            .sections
            .get((sel - 1) as usize)
            .map(|section| section.text.as_str())
            .unwrap_or(extract.extract.as_str())
    } else {
        crate::log_debug("Wikipedia import: importing whole article");
        extract.extract.as_str()
    };
    let mut text = body.trim_end().to_string();
    text.push_str("\n\nFonte: Wikipedia (CC BY-SA)\n");
    text.push_str(&extract.url);

    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    let label_set = labels(language);
    if !apply_import_text(parent, &text) {
        show_error(parent, language, &label_set.status_import_error);
        return;
    }
    editor_manager::mark_current_document_from_wikipedia(parent, true);
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
    // Large Wikipedia extracts can exceed the default Win32 edit text limit.
    unsafe {
        SendMessageW(hwnd_edit, EM_LIMITTEXT, WPARAM(0x7FFF_FFFEusize), LPARAM(0));
    }
    let cleaned = normalize_to_crlf(text);
    let existing = get_edit_text(hwnd_edit);
    let article_start = if existing.is_empty() {
        let wide = to_wide_normalized(&cleaned);
        unsafe {
            SendMessageW(hwnd_edit, EM_SETSEL, WPARAM(0), LPARAM(-1));
            SendMessageW(
                hwnd_edit,
                crate::accessibility::EM_REPLACESEL,
                WPARAM(1),
                LPARAM(wide.as_ptr() as isize),
            );
        }
        0
    } else {
        let separator = to_wide_normalized("\r\n\r\n");
        let article = to_wide_normalized(&cleaned);
        let mut insertion = CHARRANGE { cpMin: 0, cpMax: 0 };
        unsafe {
            SendMessageW(hwnd_edit, EM_SETSEL, WPARAM(usize::MAX), LPARAM(-1));
            SendMessageW(
                hwnd_edit,
                crate::accessibility::EM_REPLACESEL,
                WPARAM(1),
                LPARAM(separator.as_ptr() as isize),
            );
            SendMessageW(
                hwnd_edit,
                EM_EXGETSEL,
                WPARAM(0),
                LPARAM(&mut insertion as *mut _ as isize),
            );
            SendMessageW(
                hwnd_edit,
                crate::accessibility::EM_REPLACESEL,
                WPARAM(1),
                LPARAM(article.as_ptr() as isize),
            );
        }
        insertion.cpMin
    };
    let cr = CHARRANGE {
        cpMin: article_start,
        cpMax: article_start,
    };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&cr as *const _ as isize),
        );
        SendMessageW(
            hwnd_edit,
            EM_SETSEL,
            WPARAM(article_start as usize),
            LPARAM(article_start as isize),
        );
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
