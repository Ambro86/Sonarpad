use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Controls::Dialogs::{FR_DOWN, FR_MATCHCASE};
use windows::Win32::UI::Controls::RichEdit::{CHARRANGE, EM_EXSETSEL, EM_FINDTEXTEXW, FINDTEXTEXW};
use windows::Win32::UI::Controls::{
    NM_RETURN, NMHDR, NMTREEVIEWW, NMTVKEYDOWN, PBM_SETPOS, PBM_SETRANGE, PROGRESS_CLASSW,
    TVE_EXPAND, TVGN_CARET, TVGN_CHILD, TVGN_NEXT, TVI_ROOT, TVIF_CHILDREN, TVIF_PARAM, TVIF_TEXT,
    TVINSERTSTRUCTW, TVINSERTSTRUCTW_0, TVITEMEXW_CHILDREN, TVITEMW, TVM_DELETEITEM, TVM_GETITEMW,
    TVM_GETNEXTITEM, TVM_INSERTITEMW, TVM_SELECTITEM, TVN_ITEMEXPANDINGW, TVN_KEYDOWN,
    TVN_SELCHANGEDW, TVS_HASBUTTONS, TVS_HASLINES, TVS_LINESATROOT, TVS_SHOWSELALWAYS, WC_BUTTON,
    WC_EDIT, WC_STATIC,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, SetFocus, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::Shell::{
    BFFM_INITIALIZED, BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW, SHBrowseForFolderW,
    SHGetPathFromIDListW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetWindowTextLengthW,
    GetWindowTextW, HMENU, IDC_ARROW, IsDialogMessageW, LoadCursorW, MSG, PostMessageW,
    SendMessageW, SetForegroundWindow, SetWindowTextW, TranslateMessage, WINDOW_STYLE, WM_APP,
    WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_NEXTDLGCTL,
    WM_NOTIFY, WM_SETFOCUS, WM_SETFONT, WM_SETREDRAW, WNDCLASSW, WS_CAPTION, WS_CHILD,
    WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
    WS_VSCROLL,
};
use windows::core::{PCWSTR, PWSTR, w};

use crate::accessibility::{EM_SCROLLCARET, normalize_to_crlf, to_wide};
use crate::file_handler::{
    decode_text, is_audio_path, is_doc_path, is_docx_path, is_epub_path, is_html_path, is_pdf_path,
    is_ppt_path, is_pptx_path, is_spreadsheet_path, read_doc_text, read_docx_text, read_epub_text,
    read_html_text, read_ppt_text, read_spreadsheet_text,
};
use crate::i18n;
use crate::settings::Language;
use crate::{WM_FOCUS_EDITOR, with_state};

const FIND_IN_FILES_CLASS_NAME: &str = "SonarpadFindInFiles";
const FIND_IN_FILES_ID_TERM: usize = 9201;
const FIND_IN_FILES_ID_FOLDER: usize = 9202;
const FIND_IN_FILES_ID_BROWSE: usize = 9203;
const FIND_IN_FILES_ID_SEARCH: usize = 9204;
const FIND_IN_FILES_ID_RESULTS: usize = 9205;
const FIND_IN_FILES_ID_GO: usize = 9206;
const FIND_IN_FILES_ID_PROGRESS: usize = 9207;

const SNIPPET_MAX_CHARS: usize = 40;

const WM_FIND_IN_FILES_PROGRESS: u32 = WM_APP + 40;
const WM_FIND_IN_FILES_DONE: u32 = WM_APP + 41;
const WM_FIND_IN_FILES_FOCUS_RESULTS: u32 = WM_APP + 42;

struct FindInFilesInit {
    parent: HWND,
    language: Language,
    focus_results_on_open: bool,
}

struct FindInFilesState {
    hwnd: HWND,
    parent: HWND,
    term_edit: HWND,
    folder_edit: HWND,
    browse_button: HWND,
    search_button: HWND,
    go_button: HWND,
    progress_bar: HWND,
    progress_text: HWND,
    results_tree: HWND,
    results: Vec<SearchResult>,
    results_groups: Vec<ResultsGroup>,
    selected_result: Option<usize>,
    language: Language,
    searching: bool,
    cancel_flag: Option<Arc<AtomicBool>>,
    preserve_editor_return: bool,
}

struct FindInFilesLabels {
    title: String,
    term_label: String,
    folder_label: String,
    browse: String,
    search: String,
    go: String,
    results: String,
    empty_term: String,
    empty_folder: String,
    invalid_folder: String,
    progress: String,
}

#[derive(Clone)]
pub(crate) struct SearchResult {
    path: PathBuf,
    start_utf16: i32,
    len_utf16: i32,
    line: usize,
    snippet: String,
}

#[derive(Clone)]
pub(crate) struct FindInFilesCache {
    pub term: String,
    pub folder: String,
    pub results: Vec<SearchResult>,
    pub selected_result: Option<usize>,
}

struct ResultsGroup {
    path: PathBuf,
    indices: Vec<usize>,
}

fn labels(language: Language) -> FindInFilesLabels {
    FindInFilesLabels {
        title: i18n::tr(language, "find_in_files.title"),
        term_label: i18n::tr(language, "find_in_files.term_label"),
        folder_label: i18n::tr(language, "find_in_files.folder_label"),
        browse: i18n::tr(language, "find_in_files.browse"),
        search: i18n::tr(language, "find_in_files.search"),
        go: i18n::tr(language, "find_in_files.go"),
        results: i18n::tr(language, "find_in_files.results"),
        empty_term: i18n::tr(language, "find_in_files.empty_term"),
        empty_folder: i18n::tr(language, "find_in_files.empty_folder"),
        invalid_folder: i18n::tr(language, "find_in_files.invalid_folder"),
        progress: i18n::tr(language, "find_in_files.progress"),
    }
}

fn open_find_in_files_dialog_internal(parent: HWND, focus_results_on_open: bool) {
    let class_name = to_wide(FIND_IN_FILES_CLASS_NAME);
    let existing = crate::find_window_w_safe(PCWSTR(class_name.as_ptr()), PCWSTR::null());
    if existing.0 != 0 {
        unsafe {
            SetForegroundWindow(existing);
        }
        return;
    }

    let language = { with_state(parent, |state| state.settings.language) }.unwrap_or_default();
    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(unsafe {
            LoadCursorW(None, IDC_ARROW).unwrap_or_default().0
        }),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(find_in_files_wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    crate::register_class_w_safe(&wc);

    let init = Box::new(FindInFilesInit {
        parent,
        language,
        focus_results_on_open,
    });
    let labels = labels(language);
    let title = to_wide(&labels.title);

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            120,
            120,
            620,
            470,
            parent,
            HMENU(0),
            hinstance,
            Some(Box::into_raw(init) as *const _),
        )
    };

    if hwnd.0 == 0 {
        return;
    }

    unsafe {
        SetForegroundWindow(hwnd);
    }

    let mut msg = MSG::default();
    loop {
        if !crate::is_window_handle_valid(hwnd) {
            break;
        }
        let res = crate::get_message_w_safe(&mut msg, HWND(0), 0, 0);
        if res.0 == 0 {
            break;
        }
        unsafe {
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_ESCAPE.0 as u32 {
                if let Some(hwnd_edit) = crate::get_active_edit(parent)
                    && GetFocus() == hwnd_edit
                {
                    if with_find_state(hwnd, |state| {
                        SetFocus(state.results_tree);
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access find state");
                    }
                } else if let Err(_e) = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)) {
                    crate::log_debug(&format!("Error: {:?}", _e));
                }
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_RETURN.0 as u32 {
                if with_find_state(hwnd, |state| {
                    let focus = GetFocus();
                    let cmd = if focus == state.results_tree {
                        FIND_IN_FILES_ID_GO
                    } else if focus == state.term_edit || focus == state.folder_edit {
                        FIND_IN_FILES_ID_SEARCH
                    } else if focus == state.browse_button {
                        FIND_IN_FILES_ID_BROWSE
                    } else {
                        FIND_IN_FILES_ID_SEARCH
                    };
                    if let Err(_e) = PostMessageW(hwnd, WM_COMMAND, WPARAM(cmd), LPARAM(0)) {
                        crate::log_debug(&format!("Error: {:?}", _e));
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access find state");
                }
                continue;
            }
            if crate::handle_focused_edit_shortcut(&msg) {
                continue;
            }
            if IsDialogMessageW(hwnd, &msg).as_bool() {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    unsafe {
        SetForegroundWindow(parent);
    }
}

pub fn open_find_in_files_dialog(parent: HWND) {
    open_find_in_files_dialog_internal(parent, false);
}

unsafe extern "system" fn find_in_files_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "find_in_files_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || find_in_files_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

pub(crate) fn focus_find_in_files_results() -> bool {
    let class_name = to_wide(FIND_IN_FILES_CLASS_NAME);
    let hwnd = crate::find_window_w_safe(PCWSTR(class_name.as_ptr()), PCWSTR::null());
    if hwnd.0 == 0 {
        return false;
    }
    unsafe {
        SetForegroundWindow(hwnd);
        if let Err(e) = PostMessageW(hwnd, WM_FIND_IN_FILES_FOCUS_RESULTS, WPARAM(0), LPARAM(0)) {
            crate::log_debug(&format!(
                "Failed to post WM_FIND_IN_FILES_FOCUS_RESULTS: {}",
                e
            ));
        }
    }
    true
}

fn find_in_files_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create_struct =
                lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            let init_ptr = unsafe { (*create_struct).lpCreateParams as *mut FindInFilesInit };
            if init_ptr.is_null() {
                return LRESULT(0);
            }
            let init = crate::box_from_raw_safe(init_ptr);
            let labels = labels(init.language);
            let hfont = { with_state(init.parent, |state| state.hfont) }.unwrap_or(HFONT(0));

            let term_label = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.term_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    14,
                    160,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                )
            };
            let term_edit = unsafe {
                CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    16,
                    36,
                    560,
                    24,
                    hwnd,
                    HMENU(FIND_IN_FILES_ID_TERM as isize),
                    HINSTANCE(0),
                    None,
                )
            };

            let folder_label = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.folder_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    70,
                    160,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                )
            };
            let folder_edit = unsafe {
                CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    16,
                    92,
                    440,
                    24,
                    hwnd,
                    HMENU(FIND_IN_FILES_ID_FOLDER as isize),
                    HINSTANCE(0),
                    None,
                )
            };
            let browse_button = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.browse).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    464,
                    92,
                    112,
                    24,
                    hwnd,
                    HMENU(FIND_IN_FILES_ID_BROWSE as isize),
                    HINSTANCE(0),
                    None,
                )
            };

            let search_button = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.search).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    464,
                    124,
                    112,
                    26,
                    hwnd,
                    HMENU(FIND_IN_FILES_ID_SEARCH as isize),
                    HINSTANCE(0),
                    None,
                )
            };

            let progress_label = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&format!("{} 0%", labels.progress)).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    126,
                    220,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                )
            };

            let progress_bar = unsafe {
                CreateWindowExW(
                    Default::default(),
                    PROGRESS_CLASSW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    148,
                    560,
                    18,
                    hwnd,
                    HMENU(FIND_IN_FILES_ID_PROGRESS as isize),
                    HINSTANCE(0),
                    None,
                )
            };
            crate::send_message_w_safe(
                progress_bar,
                PBM_SETRANGE,
                WPARAM(0),
                LPARAM(((0u16 as u32) | ((100u16 as u32) << 16)) as isize),
            );

            let results_label = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.results).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    176,
                    160,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                )
            };
            let results_tree = unsafe {
                CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("SysTreeView32"),
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WS_VSCROLL
                        | WINDOW_STYLE(
                            TVS_HASBUTTONS | TVS_HASLINES | TVS_LINESATROOT | TVS_SHOWSELALWAYS,
                        ),
                    16,
                    198,
                    560,
                    190,
                    hwnd,
                    HMENU(FIND_IN_FILES_ID_RESULTS as isize),
                    HINSTANCE(0),
                    None,
                )
            };

            let go_button = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.go).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    464,
                    396,
                    112,
                    28,
                    hwnd,
                    HMENU(FIND_IN_FILES_ID_GO as isize),
                    HINSTANCE(0),
                    None,
                )
            };

            for control in [
                term_label,
                term_edit,
                folder_label,
                folder_edit,
                browse_button,
                search_button,
                progress_label,
                progress_bar,
                results_label,
                results_tree,
                go_button,
            ] {
                if control.0 != 0 && hfont.0 != 0 {
                    crate::send_message_w_safe(
                        control,
                        WM_SETFONT,
                        WPARAM(hfont.0 as usize),
                        LPARAM(1),
                    );
                }
            }

            crate::set_focus_safe(term_edit);

            let state = Box::new(FindInFilesState {
                hwnd,
                parent: init.parent,
                term_edit,
                folder_edit,
                browse_button,
                search_button,
                go_button,
                progress_bar,
                progress_text: progress_label,
                results_tree,
                results: Vec::new(),
                results_groups: Vec::new(),
                selected_result: None,
                language: init.language,
                searching: false,
                cancel_flag: None,
                preserve_editor_return: false,
            });
            crate::set_window_long_ptr_w_safe(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                Box::into_raw(state) as isize,
            );

            if with_find_state(hwnd, |state| {
                apply_cache(state);
                if init.focus_results_on_open && !state.results.is_empty() {
                    crate::log_if_err!(crate::post_message_w_safe(
                        hwnd,
                        WM_FIND_IN_FILES_FOCUS_RESULTS,
                        WPARAM(0),
                        LPARAM(0),
                    ));
                }
            })
            .is_none()
            {
                crate::log_debug("Failed to access find state");
            }

            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd_id = wparam.0 & 0xffff;
            if cmd_id == FIND_IN_FILES_ID_BROWSE {
                if with_find_state(hwnd, |state| {
                    if let Some(folder) = browse_for_folder(hwnd, state.language) {
                        let wide = to_wide(folder.to_string_lossy().as_ref());
                        if let Err(_e) =
                            crate::set_window_text_w_safe(state.folder_edit, PCWSTR(wide.as_ptr()))
                        {
                            crate::log_debug(&format!("Error: {:?}", _e));
                        }
                        crate::set_focus_safe(state.term_edit);
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access find state");
                }
                LRESULT(0)
            } else if cmd_id == FIND_IN_FILES_ID_SEARCH {
                if with_find_state(hwnd, |state| {
                    start_search(state);
                })
                .is_none()
                {
                    crate::log_debug("Failed to access find state");
                }
                LRESULT(0)
            } else if cmd_id == FIND_IN_FILES_ID_GO {
                if with_find_state(hwnd, |state| {
                    open_selected_result(state);
                })
                .is_none()
                {
                    crate::log_debug("Failed to access find state");
                }
                LRESULT(0)
            } else {
                crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
            }
        }
        WM_KEYDOWN => {
            if wparam.0 as u32 == VK_ESCAPE.0 as u32 {
                if let Err(_e) = crate::post_message_w_safe(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)) {
                    crate::log_debug(&format!("Error: {:?}", _e));
                }
                return LRESULT(0);
            }
            if wparam.0 as u32 == VK_RETURN.0 as u32 {
                if with_find_state(hwnd, |state| {
                    let focus = crate::get_focus_safe();
                    if focus == state.results_tree {
                        open_selected_result(state);
                    } else {
                        start_search(state);
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access find state");
                }
                return LRESULT(0);
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_NOTIFY => {
            let hdr = lparam.0 as *const NMHDR;
            if !hdr.is_null() {
                unsafe {
                    if (*hdr).code == NM_RETURN && (*hdr).idFrom == FIND_IN_FILES_ID_RESULTS {
                        if with_find_state(hwnd, |state| {
                            open_selected_result(state);
                        })
                        .is_none()
                        {
                            crate::log_debug("Failed to access find state");
                        }
                        return LRESULT(0);
                    }
                    if (*hdr).code == TVN_KEYDOWN && (*hdr).idFrom == FIND_IN_FILES_ID_RESULTS {
                        let key = (lparam.0 as *const NMTVKEYDOWN).as_ref();
                        if let Some(key) = key
                            && key.wVKey == VK_RETURN.0
                        {
                            if with_find_state(hwnd, |state| {
                                open_selected_result(state);
                            })
                            .is_none()
                            {
                                crate::log_debug("Failed to access find state");
                            }
                            return LRESULT(0);
                        }
                    }
                    if (*hdr).code == TVN_ITEMEXPANDINGW
                        && (*hdr).idFrom == FIND_IN_FILES_ID_RESULTS
                    {
                        let info = (lparam.0 as *const NMTREEVIEWW).as_ref();
                        if let Some(info) = info
                            && info.action == TVE_EXPAND
                        {
                            if with_find_state(hwnd, |state| {
                                ensure_children_loaded(state, info.itemNew.hItem);
                            })
                            .is_none()
                            {
                                crate::log_debug("Failed to access find state");
                            }
                            return LRESULT(0);
                        }
                    }
                    if (*hdr).code == TVN_SELCHANGEDW && (*hdr).idFrom == FIND_IN_FILES_ID_RESULTS {
                        let info = (lparam.0 as *const NMTREEVIEWW).as_ref();
                        if let Some(info) = info {
                            let failed = with_find_state(hwnd, |state| {
                                let idx = tree_item_param(state.results_tree, info.itemNew.hItem);
                                state.selected_result = idx.and_then(|value| {
                                    if value < 0 {
                                        None
                                    } else {
                                        Some(value as usize)
                                    }
                                });
                            })
                            .is_none();
                            if failed {
                                crate::log_debug("Failed to access find state");
                            }
                            return LRESULT(0);
                        }
                        return LRESULT(0);
                    }
                }
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_FIND_IN_FILES_PROGRESS => {
            let percent = wparam.0 as u32;
            if with_find_state(hwnd, |state| {
                set_progress(state, percent);
            })
            .is_none()
            {
                crate::log_debug("Failed to access find state");
            }
            LRESULT(0)
        }
        WM_FIND_IN_FILES_DONE => {
            let results_ptr = lparam.0 as *mut Vec<SearchResult>;
            if !results_ptr.is_null() {
                let results = crate::box_from_raw_safe(results_ptr);
                let res = with_find_state(hwnd, |state| {
                    state.results = *results;
                    state.searching = false;
                    state.cancel_flag = None;
                    set_search_enabled(state, true);
                    set_progress(state, 100);
                    populate_results_tree(state);
                    store_cache(state);
                });
                if res.is_none() {
                    crate::log_debug("Failed to access find state");
                }
            }
            LRESULT(0)
        }
        WM_FIND_IN_FILES_FOCUS_RESULTS => {
            if with_find_state(hwnd, |state| unsafe {
                SetFocus(state.results_tree);
            })
            .is_none()
            {
                crate::log_debug("Failed to access find state");
            }
            LRESULT(0)
        }
        WM_SETFOCUS => {
            if with_find_state(hwnd, |state| {
                if state.results.is_empty() {
                    crate::set_focus_safe(state.term_edit);
                } else {
                    crate::set_focus_safe(state.results_tree);
                }
            })
            .is_none()
            {
                crate::log_debug("Failed to access find state");
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if with_find_state(hwnd, |state| {
                if let Some(flag) = &state.cancel_flag {
                    flag.store(true, Ordering::SeqCst);
                }
                store_cache(state);
                if !state.preserve_editor_return
                    && with_state(state.parent, |main_state| {
                        if let Some(doc) = main_state.docs.get_mut(main_state.current) {
                            doc.from_find_in_files = false;
                        }
                    })
                    .is_none()
                {
                    crate::log_debug("Failed to clear find-in-files return marker");
                }
            })
            .is_none()
            {
                crate::log_debug("Failed to access find state");
            }
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            LRESULT(0)
        }
        WM_DESTROY => {
            if with_find_state(hwnd, |state| {
                crate::set_foreground_window_safe(state.parent);
            })
            .is_none()
            {
                crate::log_debug("Failed to access find state");
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let ptr = crate::get_window_long_ptr_w_safe(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
            ) as *mut FindInFilesState;
            if !ptr.is_null() {
                let _unused_box = crate::box_from_raw_safe(ptr);
            }
            LRESULT(0)
        }
        _ => crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
    }
}

fn with_find_state<F, R>(hwnd: HWND, f: F) -> Option<R>
where
    F: FnOnce(&mut FindInFilesState) -> R,
{
    let ptr = crate::get_window_long_ptr_w_safe(
        hwnd,
        windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
    ) as *mut FindInFilesState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn read_control_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..len as usize])
    }
}

fn load_cache(parent: HWND) -> Option<FindInFilesCache> {
    { with_state(parent, |state| state.find_in_files_cache.clone()) }.unwrap_or(None)
}

fn save_cache(parent: HWND, cache: FindInFilesCache) {
    {
        if with_state(parent, |state| {
            state.find_in_files_cache = Some(cache);
        })
        .is_none()
        {
            crate::log_debug("Failed to update find_in_files_cache state");
        }
    }
}

fn apply_cache(state: &mut FindInFilesState) {
    let Some(cache) = load_cache(state.parent) else {
        return;
    };
    crate::log_debug(&format!(
        "find_in_files.apply_cache: results={} selected_result={:?}",
        cache.results.len(),
        cache.selected_result
    ));
    if !cache.term.is_empty() {
        let wide = to_wide(&cache.term);
        unsafe {
            if let Err(e) = SetWindowTextW(state.term_edit, PCWSTR(wide.as_ptr())) {
                crate::log_debug(&format!("Failed to set term_edit text: {}", e));
            }
        }
    }
    if !cache.folder.is_empty() {
        let wide = to_wide(&cache.folder);
        unsafe {
            if let Err(e) = SetWindowTextW(state.folder_edit, PCWSTR(wide.as_ptr())) {
                crate::log_debug(&format!("Failed to set folder_edit text: {}", e));
            }
        }
    }
    if !cache.results.is_empty() {
        state.results = cache.results;
        state.selected_result = cache.selected_result;
        set_progress(state, 100);
        populate_results_tree(state);
    }
}

fn store_cache(state: &FindInFilesState) {
    let term = read_control_text(state.term_edit).trim().to_string();
    let folder = read_control_text(state.folder_edit).trim().to_string();
    let selected_result = state
        .selected_result
        .or_else(|| selected_result_index(state.results_tree));
    crate::log_debug(&format!(
        "find_in_files.store_cache: results={} selected_result={:?}",
        state.results.len(),
        selected_result
    ));
    let cache = FindInFilesCache {
        term,
        folder,
        results: state.results.clone(),
        selected_result,
    };
    save_cache(state.parent, cache);
}

fn start_search(state: &mut FindInFilesState) {
    if state.searching {
        return;
    }
    let term = read_control_text(state.term_edit).trim().to_string();
    let folder = read_control_text(state.folder_edit).trim().to_string();
    let labels = labels(state.language);

    if term.is_empty() {
        crate::show_error(state.parent, state.language, &labels.empty_term);
        return;
    }
    if folder.is_empty() {
        crate::show_error(state.parent, state.language, &labels.empty_folder);
        return;
    }

    let folder_path = PathBuf::from(folder);
    if !folder_path.is_dir() {
        crate::show_error(state.parent, state.language, &labels.invalid_folder);
        return;
    }

    state.searching = true;
    state.results.clear();
    state.results_groups.clear();
    set_search_enabled(state, false);
    set_progress(state, 0);
    clear_results_tree(state.results_tree);

    let hwnd = state.hwnd;
    let language = state.language;
    let term_norm = normalize_to_crlf(&term);
    let term_has_newline = term.contains('\n') || term.contains('\r');
    let folder_path = folder_path.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    state.cancel_flag = Some(cancel.clone());

    std::thread::spawn(move || {
        let files = collect_search_files(&folder_path);
        let total = files.len().max(1);
        let mut results = Vec::new();
        for (idx, path) in files.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            if let Some(text) = read_text_for_search(path, language) {
                if term_has_newline {
                    let normalized = normalize_to_crlf(&text);
                    let term_len_utf16 = term_norm.encode_utf16().count() as i32;
                    collect_matches(
                        &normalized,
                        &term_norm,
                        term_len_utf16,
                        true,
                        path,
                        &mut results,
                    );
                } else {
                    let term_len_utf16 = term.encode_utf16().count() as i32;
                    collect_matches(&text, &term, term_len_utf16, false, path, &mut results);
                }
            }
            let percent = ((idx + 1) * 100 / total) as u32;
            unsafe {
                if let Err(e) = PostMessageW(
                    hwnd,
                    WM_FIND_IN_FILES_PROGRESS,
                    WPARAM(percent as usize),
                    LPARAM(0),
                ) {
                    crate::log_debug(&format!("Failed to post WM_FIND_IN_FILES_PROGRESS: {}", e));
                }
            };
        }

        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let boxed = Box::new(results);
        unsafe {
            if let Err(e) = PostMessageW(
                hwnd,
                WM_FIND_IN_FILES_DONE,
                WPARAM(0),
                LPARAM(Box::into_raw(boxed) as isize),
            ) {
                crate::log_debug(&format!("Failed to post WM_FIND_IN_FILES_DONE: {}", e));
            }
        };
    });
}

fn open_selected_result(state: &mut FindInFilesState) {
    let idx = state
        .selected_result
        .or_else(|| selected_result_index(state.results_tree));
    let Some(idx) = idx else {
        return;
    };
    let Some(result) = state.results.get(idx).cloned() else {
        return;
    };
    let term = read_control_text(state.term_edit).trim().to_string();
    let pending = crate::PendingFindInFilesSelection {
        path: result.path.clone(),
        snippet: result.snippet.clone(),
        term,
        start_utf16: result.start_utf16,
        len_utf16: result.len_utf16,
    };
    unsafe {
        if with_state(state.parent, |state| {
            state.pending_find_in_files = Some(pending);
        })
        .is_none()
        {
            crate::log_debug("Failed to set pending find-in-files selection");
        }
        crate::editor_manager::open_document(state.parent, &result.path);
        if let Err(e) = PostMessageW(state.parent, WM_FOCUS_EDITOR, WPARAM(0), LPARAM(0)) {
            crate::log_debug(&format!("Failed to post WM_FOCUS_EDITOR: {}", e));
        }
        state.preserve_editor_return = true;
        if let Err(e) = PostMessageW(state.hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)) {
            crate::log_debug(&format!("Failed to close Find in Files dialog: {}", e));
        }
    }
}

pub(crate) fn apply_find_in_files_selection(
    hwnd_edit: HWND,
    snippet: &str,
    term: &str,
    start_utf16: i32,
    len_utf16: i32,
) {
    if select_snippet_exact(hwnd_edit, snippet) {
        return;
    }
    let term = normalize_to_crlf(term);
    if term.is_empty() {
        return;
    }
    select_term_at(hwnd_edit, &term, start_utf16, len_utf16);
}

fn set_search_enabled(state: &mut FindInFilesState, enabled: bool) {
    unsafe {
        EnableWindow(state.term_edit, enabled);
        EnableWindow(state.folder_edit, enabled);
        EnableWindow(state.browse_button, enabled);
        EnableWindow(state.search_button, enabled);
        EnableWindow(state.results_tree, enabled);
        EnableWindow(state.go_button, enabled);
    }
}

fn set_progress(state: &mut FindInFilesState, percent: u32) {
    let percent = percent.min(100);
    unsafe {
        SendMessageW(
            state.progress_bar,
            PBM_SETPOS,
            WPARAM(percent as usize),
            LPARAM(0),
        );
        let label = format!("{} {}%", labels(state.language).progress, percent);
        let wide = to_wide(&label);
        if let Err(e) = SetWindowTextW(state.progress_text, PCWSTR(wide.as_ptr())) {
            crate::log_debug(&format!("Failed to set progress text: {}", e));
        }
    }
}

fn clear_results_tree(tree: HWND) {
    crate::send_message_w_safe(tree, TVM_DELETEITEM, WPARAM(0), LPARAM(TVI_ROOT.0));
}

fn set_results_redraw(tree: HWND, enabled: bool) {
    crate::send_message_w_safe(tree, WM_SETREDRAW, WPARAM(enabled as usize), LPARAM(0));
}

fn populate_results_tree(state: &mut FindInFilesState) {
    set_results_redraw(state.results_tree, false);
    clear_results_tree(state.results_tree);
    if state.results.is_empty() {
        unsafe {
            SetFocus(state.results_tree);
        }
        set_results_redraw(state.results_tree, true);
        return;
    }
    let target_selected_result = state.selected_result;

    let mut grouped: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    for (idx, result) in state.results.iter().enumerate() {
        grouped.entry(result.path.clone()).or_default().push(idx);
    }
    state.results_groups = grouped
        .into_iter()
        .map(|(path, indices)| ResultsGroup { path, indices })
        .collect();

    let mut first_parent: Option<windows::Win32::UI::Controls::HTREEITEM> = None;
    for (group_idx, group) in state.results_groups.iter().enumerate() {
        let parent_text = format!("{} ({})", group.path.display(), group.indices.len());
        let parent_param = -1 - (group_idx as isize);
        let parent_item = insert_tree_item(
            state.results_tree,
            TVI_ROOT,
            &parent_text,
            parent_param,
            true,
        );
        if first_parent.is_none() {
            first_parent = Some(parent_item);
        }
    }

    if let Some(parent_item) = first_parent {
        unsafe {
            SendMessageW(
                state.results_tree,
                TVM_SELECTITEM,
                WPARAM(TVGN_CARET as usize),
                LPARAM(parent_item.0),
            );
            SetFocus(state.results_tree);
        }
    }
    state.selected_result = target_selected_result;
    restore_selected_result(state);
    set_results_redraw(state.results_tree, true);
}

fn restore_selected_result(state: &mut FindInFilesState) {
    let Some(selected_idx) = state.selected_result else {
        crate::log_debug("find_in_files.restore_selected_result: no selected_result");
        return;
    };
    crate::log_debug(&format!(
        "find_in_files.restore_selected_result: target={}",
        selected_idx
    ));
    let Some((group_idx, child_idx)) =
        state
            .results_groups
            .iter()
            .enumerate()
            .find_map(|(group_idx, group)| {
                group
                    .indices
                    .iter()
                    .position(|idx| *idx == selected_idx)
                    .map(|child_idx| (group_idx, child_idx))
            })
    else {
        crate::log_debug("find_in_files.restore_selected_result: target not found in groups");
        return;
    };

    let mut parent_item = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            state.results_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CHILD as usize),
            LPARAM(TVI_ROOT.0),
        )
        .0,
    );
    for _ in 0..group_idx {
        if parent_item.0 == 0 {
            crate::log_debug(
                "find_in_files.restore_selected_result: parent_item missing during walk",
            );
            return;
        }
        parent_item = windows::Win32::UI::Controls::HTREEITEM(
            crate::send_message_w_safe(
                state.results_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_NEXT as usize),
                LPARAM(parent_item.0),
            )
            .0,
        );
    }
    if parent_item.0 == 0 {
        crate::log_debug("find_in_files.restore_selected_result: parent_item not found");
        return;
    }

    ensure_children_loaded(state, parent_item);
    let mut child_item = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            state.results_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CHILD as usize),
            LPARAM(parent_item.0),
        )
        .0,
    );
    for _ in 0..child_idx {
        if child_item.0 == 0 {
            crate::log_debug(
                "find_in_files.restore_selected_result: child_item missing during walk",
            );
            return;
        }
        child_item = windows::Win32::UI::Controls::HTREEITEM(
            crate::send_message_w_safe(
                state.results_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_NEXT as usize),
                LPARAM(child_item.0),
            )
            .0,
        );
    }
    if child_item.0 == 0 {
        crate::log_debug("find_in_files.restore_selected_result: child_item not found");
        return;
    }

    crate::send_message_w_safe(
        state.results_tree,
        TVM_SELECTITEM,
        WPARAM(TVGN_CARET as usize),
        LPARAM(child_item.0),
    );
    crate::set_focus_safe(state.results_tree);
    crate::log_debug(&format!(
        "find_in_files.restore_selected_result: selected group={} child={} item={:?}",
        group_idx, child_idx, child_item
    ));
}

pub fn reopen_results(parent: HWND) {
    open_find_in_files_dialog_internal(parent, true);
}

fn insert_tree_item(
    tree: HWND,
    parent: windows::Win32::UI::Controls::HTREEITEM,
    text: &str,
    param: isize,
    has_children: bool,
) -> windows::Win32::UI::Controls::HTREEITEM {
    let mut wide = to_wide(text);
    let item = TVITEMW {
        mask: if has_children {
            TVIF_TEXT | TVIF_PARAM | TVIF_CHILDREN
        } else {
            TVIF_TEXT | TVIF_PARAM
        },
        hItem: windows::Win32::UI::Controls::HTREEITEM(0),
        state: Default::default(),
        stateMask: Default::default(),
        pszText: PWSTR(wide.as_mut_ptr()),
        cchTextMax: (wide.len().saturating_sub(1)) as i32,
        iImage: 0,
        iSelectedImage: 0,
        cChildren: if has_children {
            TVITEMEXW_CHILDREN(1)
        } else {
            TVITEMEXW_CHILDREN(0)
        },
        lParam: LPARAM(param),
    };
    let insert = TVINSERTSTRUCTW {
        hParent: parent,
        hInsertAfter: windows::Win32::UI::Controls::HTREEITEM(0),
        Anonymous: TVINSERTSTRUCTW_0 { item },
    };
    let res = crate::send_message_w_safe(
        tree,
        TVM_INSERTITEMW,
        WPARAM(0),
        LPARAM(&insert as *const _ as isize),
    );
    windows::Win32::UI::Controls::HTREEITEM(res.0)
}

fn ensure_children_loaded(
    state: &mut FindInFilesState,
    parent: windows::Win32::UI::Controls::HTREEITEM,
) {
    let has_child = crate::send_message_w_safe(
        state.results_tree,
        TVM_GETNEXTITEM,
        WPARAM(TVGN_CHILD as usize),
        LPARAM(parent.0),
    );
    if has_child.0 != 0 {
        return;
    }
    let mut item = TVITEMW {
        mask: TVIF_PARAM,
        hItem: parent,
        ..Default::default()
    };
    let ok = crate::send_message_w_safe(
        state.results_tree,
        TVM_GETITEMW,
        WPARAM(0),
        LPARAM(&mut item as *mut _ as isize),
    );
    if ok.0 == 0 {
        return;
    }
    let group_idx = decode_group_index(item.lParam.0);
    let Some(group_idx) = group_idx else {
        return;
    };
    let Some(group) = state.results_groups.get(group_idx) else {
        return;
    };
    for idx in &group.indices {
        let result = &state.results[*idx];
        let line_prefix = i18n::tr_f(
            state.language,
            "find_in_files.line_prefix",
            &[("line", &result.line.to_string())],
        );
        let label = format!("{line_prefix} {}", result.snippet);
        insert_tree_item(state.results_tree, parent, &label, *idx as isize, false);
    }
}

fn decode_group_index(param: isize) -> Option<usize> {
    if param >= 0 {
        None
    } else {
        Some((-param - 1) as usize)
    }
}

fn tree_item_param(tree: HWND, item: windows::Win32::UI::Controls::HTREEITEM) -> Option<isize> {
    if item.0 == 0 {
        return None;
    }
    let mut tv_item = TVITEMW {
        mask: TVIF_PARAM,
        hItem: item,
        ..Default::default()
    };
    let ok = crate::send_message_w_safe(
        tree,
        TVM_GETITEMW,
        WPARAM(0),
        LPARAM(&mut tv_item as *mut _ as isize),
    );
    if ok.0 == 0 {
        return None;
    }
    Some(tv_item.lParam.0)
}

fn selected_result_index(tree: HWND) -> Option<usize> {
    let hitem = crate::send_message_w_safe(
        tree,
        TVM_GETNEXTITEM,
        WPARAM(TVGN_CARET as usize),
        LPARAM(0),
    );
    if hitem.0 == 0 {
        return None;
    }
    let mut item = TVITEMW {
        mask: TVIF_PARAM,
        hItem: windows::Win32::UI::Controls::HTREEITEM(hitem.0),
        ..Default::default()
    };
    let ok = crate::send_message_w_safe(
        tree,
        TVM_GETITEMW,
        WPARAM(0),
        LPARAM(&mut item as *mut _ as isize),
    );
    if ok.0 == 0 {
        return None;
    }
    let idx = item.lParam.0;
    if idx < 0 { None } else { Some(idx as usize) }
}

fn collect_search_files(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if is_pdf_path(&path) || is_audio_path(&path) {
                continue;
            }
            files.push(path);
        }
    }

    files
}

fn read_text_for_search(path: &Path, language: Language) -> Option<String> {
    if is_docx_path(path) {
        return read_docx_text(path, language).ok();
    }
    if is_doc_path(path) {
        return read_doc_text(path, language).ok();
    }
    if is_epub_path(path) {
        return read_epub_text(path, language).ok();
    }
    if is_html_path(path) {
        return read_html_text(path, language).ok().map(|(text, _)| text);
    }
    if is_spreadsheet_path(path) {
        return read_spreadsheet_text(path, language).ok();
    }
    if is_pptx_path(path) || is_ppt_path(path) {
        return read_ppt_text(path, language).ok();
    }
    let bytes = std::fs::read(path).ok()?;
    let (text, _) = decode_text(&bytes, language).ok()?;
    Some(text)
}

fn collect_matches(
    text: &str,
    term: &str,
    term_len_utf16: i32,
    normalized_offsets: bool,
    path: &Path,
    out: &mut Vec<SearchResult>,
) {
    if term.is_empty() {
        return;
    }
    let mut start = 0usize;
    while let Some(found) = text[start..].find(term) {
        let byte_index = start + found;
        let utf16_index = if normalized_offsets {
            byte_index_to_utf16(text, byte_index)
        } else {
            byte_index_to_utf16_crlf(text, byte_index)
        };
        let line = count_line_number(text, byte_index);
        let (line_start, line_end) = line_bounds(text, byte_index);
        let line_text = text[line_start..line_end].trim_matches(['\r', '\n']);
        let match_start = byte_index.saturating_sub(line_start);
        let snippet = snippet_for_match(line_text, match_start, term.len(), SNIPPET_MAX_CHARS);

        out.push(SearchResult {
            path: path.to_path_buf(),
            start_utf16: utf16_index,
            len_utf16: term_len_utf16,
            line,
            snippet,
        });

        start = byte_index + term.len();
    }
}

fn line_bounds(text: &str, byte_index: usize) -> (usize, usize) {
    let before = &text[..byte_index];
    let last_lf = before.rfind('\n');
    let last_cr = before.rfind('\r');
    let start = match (last_lf, last_cr) {
        (None, None) => 0,
        (Some(idx), None) | (None, Some(idx)) => idx + 1,
        (Some(lf), Some(cr)) => lf.max(cr) + 1,
    };

    let after = &text[byte_index..];
    let next_lf = after.find('\n');
    let next_cr = after.find('\r');
    let end = match (next_lf, next_cr) {
        (None, None) => text.len(),
        (Some(idx), None) | (None, Some(idx)) => byte_index + idx,
        (Some(lf), Some(cr)) => byte_index + lf.min(cr),
    };
    (start, end)
}

fn count_line_number(text: &str, byte_index: usize) -> usize {
    let mut line = 1usize;
    let mut prev_cr = false;
    for b in text[..byte_index].bytes() {
        if b == b'\r' {
            line += 1;
            prev_cr = true;
        } else if b == b'\n' {
            if !prev_cr {
                line += 1;
            }
            prev_cr = false;
        } else {
            prev_cr = false;
        }
    }
    line
}

fn byte_index_to_utf16(text: &str, byte_idx: usize) -> i32 {
    let mut utf16_count = 0usize;
    for (idx, ch) in text.char_indices() {
        if idx >= byte_idx {
            break;
        }
        utf16_count += ch.len_utf16();
    }
    utf16_count as i32
}

fn char_index_to_byte(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    text.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn word_bounds(chars: &[char], match_start: usize, match_end: usize) -> (usize, usize) {
    let mut start = match_start.min(chars.len());
    while start > 0 && is_word_char(chars[start - 1]) {
        start = start.saturating_sub(1);
    }
    let mut end = match_end.min(chars.len());
    while end < chars.len() && is_word_char(chars[end]) {
        end += 1;
    }
    (start, end)
}

fn snippet_for_match(line: &str, match_start: usize, match_len: usize, max_chars: usize) -> String {
    if max_chars == 0 || line.is_empty() {
        return String::new();
    }

    let match_end = match_start.saturating_add(match_len).min(line.len());
    let match_start_char = line[..match_start].chars().count();
    let match_end_char = line[..match_end].chars().count();
    let chars: Vec<char> = line.chars().collect();
    let total_chars = chars.len();
    let (word_start_char, word_end_char) = word_bounds(&chars, match_start_char, match_end_char);

    let mut slice_start_char = match_start_char.saturating_sub(max_chars / 2);
    let mut slice_end_char = (slice_start_char + max_chars).min(total_chars);

    if match_end_char > slice_end_char {
        slice_end_char = match_end_char.min(total_chars);
        slice_start_char = slice_end_char.saturating_sub(max_chars);
    }

    if slice_start_char > word_start_char {
        slice_start_char = word_start_char;
    }
    if slice_end_char < word_end_char {
        slice_end_char = word_end_char;
    }

    if slice_end_char.saturating_sub(slice_start_char) > max_chars {
        let word_len = word_end_char.saturating_sub(word_start_char);
        if word_len >= max_chars {
            slice_start_char = word_start_char;
            slice_end_char = (word_start_char + max_chars).min(total_chars);
        } else {
            let extra = max_chars - word_len;
            let before = extra / 2;
            let after = extra - before;
            slice_start_char = word_start_char.saturating_sub(before);
            slice_end_char = (word_end_char + after).min(total_chars);
            let size = slice_end_char.saturating_sub(slice_start_char);
            if size < max_chars {
                let missing = max_chars - size;
                slice_start_char = slice_start_char.saturating_sub(missing);
            }
        }
    }

    if slice_start_char > 0
        && slice_start_char < word_start_char
        && is_word_char(chars[slice_start_char])
        && is_word_char(chars[slice_start_char - 1])
    {
        let mut adjust = slice_start_char;
        while adjust < word_start_char && is_word_char(chars[adjust]) {
            adjust += 1;
        }
        slice_start_char = adjust.min(word_start_char);
    }

    if slice_end_char < total_chars
        && slice_end_char > word_end_char
        && is_word_char(chars[slice_end_char - 1])
        && is_word_char(chars[slice_end_char])
    {
        let mut adjust = slice_end_char;
        while adjust > word_end_char && is_word_char(chars[adjust - 1]) {
            adjust = adjust.saturating_sub(1);
        }
        slice_end_char = adjust.max(word_end_char);
    }

    let slice_start_byte = char_index_to_byte(line, slice_start_char);
    let slice_end_byte = char_index_to_byte(line, slice_end_char);
    let mut snippet = line[slice_start_byte..slice_end_byte].trim().to_string();

    if slice_start_char > 0 {
        snippet.insert_str(0, "...");
    }
    if slice_end_char < total_chars {
        snippet.push_str("...");
    }

    snippet
}

fn select_term_at(hwnd_edit: HWND, term: &str, start: i32, _len: i32) {
    if term.is_empty() {
        return;
    }
    let wide = to_wide(term);
    let mut ft = FINDTEXTEXW {
        chrg: CHARRANGE {
            cpMin: start.max(0),
            cpMax: -1,
        },
        lpstrText: PCWSTR(wide.as_ptr()),
        chrgText: CHARRANGE { cpMin: 0, cpMax: 0 },
    };
    let found = unsafe {
        SendMessageW(
            hwnd_edit,
            EM_FINDTEXTEXW,
            WPARAM((FR_MATCHCASE | FR_DOWN).0 as usize),
            LPARAM(&mut ft as *mut _ as isize),
        )
        .0 != -1
    };

    if found {
        let start = ft.chrgText.cpMin.min(ft.chrgText.cpMax);
        set_caret_position(hwnd_edit, start);
    } else {
        let start = start.max(0);
        set_caret_position(hwnd_edit, start);
    }
}

fn byte_index_to_utf16_crlf(text: &str, byte_idx: usize) -> i32 {
    let mut extra = 0usize;
    let mut prev_cr = false;
    for b in text[..byte_idx].bytes() {
        if b == b'\r' {
            prev_cr = true;
        } else if b == b'\n' {
            if !prev_cr {
                extra += 1;
            }
            prev_cr = false;
        } else {
            prev_cr = false;
        }
    }
    byte_index_to_utf16(text, byte_idx) + extra as i32
}

fn strip_snippet_markers(snippet: &str) -> String {
    let mut out = snippet.trim().to_string();
    if let Some(rest) = out.strip_prefix("...") {
        out = rest.trim_start().to_string();
    }
    if let Some(rest) = out.strip_suffix("...") {
        out = rest.trim_end().to_string();
    }
    out
}

fn select_snippet_exact(hwnd_edit: HWND, snippet: &str) -> bool {
    let snippet = strip_snippet_markers(snippet);
    if snippet.is_empty() {
        return false;
    }
    let wide = to_wide(&snippet);
    let mut ft = FINDTEXTEXW {
        chrg: CHARRANGE {
            cpMin: 0,
            cpMax: -1,
        },
        lpstrText: PCWSTR(wide.as_ptr()),
        chrgText: CHARRANGE { cpMin: 0, cpMax: 0 },
    };
    let result = crate::send_message_w_safe(
        hwnd_edit,
        EM_FINDTEXTEXW,
        WPARAM((FR_MATCHCASE | FR_DOWN).0 as usize),
        LPARAM(&mut ft as *mut _ as isize),
    );
    if result.0 == -1 {
        return false;
    }
    let start = ft.chrgText.cpMin.min(ft.chrgText.cpMax);
    set_caret_position(hwnd_edit, start);
    true
}

fn set_caret_position(hwnd_edit: HWND, pos: i32) {
    let pos = pos.max(0);
    let mut cr = CHARRANGE {
        cpMin: pos,
        cpMax: pos,
    };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut cr as *mut _ as isize),
        );
        SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
        SetFocus(hwnd_edit);
    }
}

fn describe_hwnd(hwnd: HWND) -> String {
    if hwnd.0 == 0 {
        return "HWND(0)".to_string();
    }
    let mut class_buf = [0u16; 128];
    let class_len = crate::get_class_name_w_safe(hwnd, &mut class_buf);
    let class_name = if class_len > 0 {
        String::from_utf16_lossy(&class_buf[..class_len as usize])
    } else {
        String::new()
    };
    let text_len = crate::get_window_text_length_w_safe(hwnd);
    let window_text = if text_len > 0 {
        let mut text_buf = vec![0u16; text_len as usize + 1];
        let read = crate::get_window_text_w_safe(hwnd, &mut text_buf);
        String::from_utf16_lossy(&text_buf[..read.max(0) as usize])
    } else {
        String::new()
    };
    format!("{:?} class='{}' text='{}'", hwnd, class_name, window_text)
}

fn log_folder_picker_focus(tag: &str, hwnd: HWND) {
    let foreground = crate::get_foreground_window_safe();
    let focus = crate::get_focus_safe();
    crate::log_debug(&format!(
        "{}: dialog={} foreground={} focus={}",
        tag,
        describe_hwnd(hwnd),
        describe_hwnd(foreground),
        describe_hwnd(focus)
    ));
}

fn folder_picker_focus_target(dialog: HWND) -> HWND {
    let mut current = crate::get_next_dlg_tab_item_safe(dialog, HWND(0), false);
    let mut first_non_static = HWND(0);
    for _ in 0..16 {
        if current.0 == 0 {
            break;
        }
        let mut class_buf = [0u16; 128];
        let class_len = crate::get_class_name_w_safe(current, &mut class_buf);
        let class_name = if class_len > 0 {
            String::from_utf16_lossy(&class_buf[..class_len as usize])
        } else {
            String::new()
        };
        if class_name == "SysTreeView32" {
            return current;
        }
        if first_non_static.0 == 0 && class_name != "Static" {
            first_non_static = current;
        }
        current = crate::get_next_dlg_tab_item_safe(dialog, current, false);
    }
    first_non_static
}

extern "system" fn browse_folder_focus_callback(
    hwnd: HWND,
    msg: u32,
    _lparam: LPARAM,
    _lpdata: LPARAM,
) -> i32 {
    if msg == BFFM_INITIALIZED {
        log_folder_picker_focus("find_in_files.folder_picker.initialized.before", hwnd);
        let dialog = hwnd;
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(120));
            let target = folder_picker_focus_target(dialog);
            if target.0 != 0 {
                crate::send_message_w_safe(
                    dialog,
                    WM_NEXTDLGCTL,
                    WPARAM(target.0 as usize),
                    LPARAM(1),
                );
            } else {
                crate::log_debug("find_in_files.folder_picker: no dialog focus target found");
            }
        });
        log_folder_picker_focus("find_in_files.folder_picker.initialized.after", hwnd);
    }
    0
}

pub(crate) fn browse_for_folder(owner: HWND, language: Language) -> Option<PathBuf> {
    let labels = labels(language);
    browse_for_folder_with_title(owner, &labels.folder_label)
}

pub(crate) fn browse_for_folder_with_title(owner: HWND, title: &str) -> Option<PathBuf> {
    let title = to_wide(title);
    let bi = BROWSEINFOW {
        hwndOwner: owner,
        lpszTitle: PCWSTR(title.as_ptr()),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
        lpfn: Some(browse_folder_focus_callback),
        ..Default::default()
    };

    let pidl = unsafe { SHBrowseForFolderW(&bi) };
    if pidl.is_null() {
        return None;
    }
    let mut buffer = [0u16; 260];
    let ok = unsafe { SHGetPathFromIDListW(pidl, &mut buffer).as_bool() };
    unsafe { CoTaskMemFree(Some(pidl as *const _)) };
    if !ok {
        return None;
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    let path = String::from_utf16_lossy(&buffer[..len]);
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}
