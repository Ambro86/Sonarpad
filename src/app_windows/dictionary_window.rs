use crate::accessibility::{handle_accessibility, to_wide};
use crate::i18n;
use crate::settings::{
    DictionaryChange, DictionaryEntry, Language, TtsEngine, VoiceInfo, save_dictionary_change,
};
use crate::{tts_engine, with_state};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::mpsc;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{BST_CHECKED, WC_BUTTON, WC_COMBOBOXW, WC_LISTBOXW, WC_STATIC};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus, VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL,
    CB_GETITEMDATA, CB_RESETCONTENT, CB_SETCURSEL, CB_SETITEMDATA, CBS_DROPDOWNLIST, CREATESTRUCTW,
    CreateWindowExW, DefWindowProcW, ES_AUTOHSCROLL, GWLP_USERDATA, GetWindowLongPtrW, HMENU,
    IDC_ARROW, IDCANCEL, IDOK, LB_ADDSTRING, LB_GETCOUNT, LB_GETCURSEL, LB_GETITEMDATA,
    LB_RESETCONTENT, LB_SETCURSEL, LB_SETITEMDATA, LBN_SELCHANGE, LBS_HASSTRINGS, LBS_NOTIFY,
    LoadCursorW, MSG, PostMessageW, RegisterClassW, SW_HIDE, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowTextW, ShowWindow, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_COMMAND,
    WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_SETFOCUS, WM_SETFONT, WNDCLASSW,
    WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_SYSMENU,
    WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, w};

const DICTIONARY_CLASS_NAME: &str = "SonarpadDictionary";
const DICTIONARY_ENTRY_CLASS_NAME: &str = "SonarpadDictionaryEntry";

const DICT_ID_LIST: usize = 9101;
const DICT_ID_ADD: usize = 9102;
const DICT_ID_EDIT: usize = 9103;
const DICT_ID_REMOVE: usize = 9104;
const DICT_ID_CLOSE: usize = 9105;

const DICT_ENTRY_ID_ORIGINAL: usize = 9201;
const DICT_ENTRY_ID_REPLACEMENT: usize = 9202;
const DICT_ENTRY_ID_OK: usize = 9203;
const DICT_ENTRY_ID_CANCEL: usize = 9204;
const DICT_ENTRY_ID_USE_VOICE: usize = 9205;
const DICT_ENTRY_ID_ENGINE: usize = 9206;
const DICT_ENTRY_ID_VOICE: usize = 9207;
const DICT_ENTRY_ID_PREVIEW: usize = 9208;
const DICT_ENTRY_ID_MATCH_CASE: usize = 9209;
const DICT_FOCUS_LIST_MSG: u32 = WM_APP + 9;

struct DictionaryWindowState {
    parent: HWND,
    hwnd_list: HWND,
    hwnd_edit: HWND,
    hwnd_remove: HWND,
}

struct DictionaryEntryState {
    parent: HWND,
    owner: HWND,
    edit_original: HWND,
    edit_replacement: HWND,
    checkbox_match_case: HWND,
    ok_button: HWND,
    index: Option<usize>,
    checkbox_use_voice: HWND,
    label_engine: HWND,
    combo_engine: HWND,
    label_voice: HWND,
    combo_voice: HWND,
    button_preview: HWND,
}

struct DictionaryLabels {
    title: String,
    add: String,
    edit: String,
    remove: String,
    close: String,
    entry_title_add: String,
    entry_title_edit: String,
    entry_original: String,
    entry_replacement: String,
    entry_match_case: String,
    entry_ok: String,
    entry_cancel: String,
    entry_use_voice: String,
    entry_engine: String,
    entry_voice: String,
    entry_preview: String,
    engine_edge: String,
    engine_sapi5: String,
    engine_sapi4: String,
    engine_google: String,
    voices_empty: String,
}

fn dictionary_labels(language: Language) -> DictionaryLabels {
    DictionaryLabels {
        title: i18n::tr(language, "dictionary.title"),
        add: i18n::tr(language, "dictionary.add"),
        edit: i18n::tr(language, "dictionary.edit"),
        remove: i18n::tr(language, "dictionary.remove"),
        close: i18n::tr(language, "dictionary.close"),
        entry_title_add: i18n::tr(language, "dictionary.entry_title_add"),
        entry_title_edit: i18n::tr(language, "dictionary.entry_title_edit"),
        entry_original: i18n::tr(language, "dictionary.entry_original"),
        entry_replacement: i18n::tr(language, "dictionary.entry_replacement"),
        entry_match_case: i18n::tr(language, "find.match_case"),
        entry_ok: i18n::tr(language, "dictionary.entry_ok"),
        entry_cancel: i18n::tr(language, "dictionary.entry_cancel"),
        entry_use_voice: i18n::tr(language, "dictionary.entry_use_voice"),
        entry_engine: i18n::tr(language, "dictionary.entry_engine"),
        entry_voice: i18n::tr(language, "dictionary.entry_voice"),
        entry_preview: i18n::tr(language, "dictionary.entry_preview"),
        engine_edge: i18n::tr(language, "options.engine.edge"),
        engine_sapi5: i18n::tr(language, "options.engine.sapi5"),
        engine_sapi4: i18n::tr(language, "options.engine.sapi4"),
        engine_google: i18n::tr(language, "options.engine.google"),
        voices_empty: i18n::tr(language, "voice_panel.voices_empty"),
    }
}

pub fn handle_navigation(hwnd: HWND, msg: &MSG) -> bool {
    if msg.message == WM_KEYDOWN {
        if msg.wParam.0 as u32 == VK_ESCAPE.0 as u32 {
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            return true;
        }
        if msg.wParam.0 as u32 == VK_RETURN.0 as u32 {
            return handle_accessibility(hwnd, msg);
        }
    }
    handle_accessibility(hwnd, msg)
}

pub fn open(parent: HWND) {
    unsafe {
        let existing = with_state(parent, |state| state.dictionary_window).unwrap_or(HWND(0));
        if existing.0 != 0 {
            SetForegroundWindow(existing);
            return;
        }

        // Another window or process may have edited the dictionary meanwhile.
        let latest = crate::settings::load_settings().dictionary;
        with_state(parent, |state| state.settings.dictionary = latest);

        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(DICTIONARY_CLASS_NAME);
        let wc = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
            ),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(dictionary_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
        let labels = dictionary_labels(language);
        let title = to_wide(&labels.title);

        let window = CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            0,
            0,
            520,
            430,
            parent,
            None,
            hinstance,
            Some(parent.0 as *const std::ffi::c_void),
        );

        if window.0 != 0 {
            if with_state(parent, |state| {
                state.dictionary_window = window;
            })
            .is_none()
            {
                crate::log_debug("Failed to access dictionary state");
            }
            EnableWindow(parent, false);
            SetForegroundWindow(window);
        }
    }
}

unsafe extern "system" fn dictionary_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "dictionary_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || dictionary_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn dictionary_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let create_struct = lparam.0 as *const CREATESTRUCTW;
                let parent = HWND((*create_struct).lpCreateParams as isize);
                let hfont = with_state(parent, |state| state.hfont).unwrap_or(HFONT(0));
                let language =
                    with_state(parent, |state| state.settings.language).unwrap_or_default();
                let labels = dictionary_labels(language);

                let hwnd_list = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_LISTBOXW,
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_VSCROLL
                        | WS_TABSTOP
                        | WINDOW_STYLE((LBS_NOTIFY | LBS_HASSTRINGS) as u32),
                    10,
                    10,
                    480,
                    270,
                    hwnd,
                    HMENU(DICT_ID_LIST as isize),
                    HINSTANCE(0),
                    None,
                );

                let hwnd_add = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.add).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    10,
                    290,
                    240,
                    30,
                    hwnd,
                    HMENU(DICT_ID_ADD as isize),
                    HINSTANCE(0),
                    None,
                );

                let hwnd_edit = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.edit).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    260,
                    290,
                    230,
                    30,
                    hwnd,
                    HMENU(DICT_ID_EDIT as isize),
                    HINSTANCE(0),
                    None,
                );

                let hwnd_remove = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.remove).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    10,
                    330,
                    240,
                    30,
                    hwnd,
                    HMENU(DICT_ID_REMOVE as isize),
                    HINSTANCE(0),
                    None,
                );

                let hwnd_close = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.close).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    260,
                    330,
                    230,
                    30,
                    hwnd,
                    HMENU(DICT_ID_CLOSE as isize),
                    HINSTANCE(0),
                    None,
                );

                for ctrl in [hwnd_list, hwnd_add, hwnd_edit, hwnd_remove, hwnd_close] {
                    if ctrl.0 != 0 && hfont.0 != 0 {
                        SendMessageW(ctrl, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                }

                let state = Box::new(DictionaryWindowState {
                    parent,
                    hwnd_list,
                    hwnd_edit,
                    hwnd_remove,
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

                refresh_dictionary_list(hwnd);
                update_button_states(hwnd);
                SetFocus(hwnd_list);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                let list = with_dictionary_state(hwnd, |s| s.hwnd_list).unwrap_or(HWND(0));
                if list.0 != 0 {
                    SetFocus(list);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let cmd_id = wparam.0 & 0xffff;
                let notify = (wparam.0 >> 16) as u16;
                match cmd_id {
                    DICT_ID_ADD => {
                        open_entry_dialog(hwnd, None);
                        LRESULT(0)
                    }
                    DICT_ID_EDIT => {
                        if let Some(index) = selected_dictionary_index(hwnd) {
                            open_entry_dialog(hwnd, Some(index));
                        }
                        LRESULT(0)
                    }
                    DICT_ID_REMOVE => {
                        remove_selected_entry(hwnd);
                        LRESULT(0)
                    }
                    DICT_ID_CLOSE => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    DICT_ID_LIST if notify == LBN_SELCHANGE as u16 => {
                        update_button_states(hwnd);
                        LRESULT(0)
                    }
                    cmd if cmd == IDCANCEL.0 as usize || cmd == 2 => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            DICT_FOCUS_LIST_MSG => {
                let list = with_dictionary_state(hwnd, |s| s.hwnd_list).unwrap_or(HWND(0));
                if list.0 != 0 {
                    SetForegroundWindow(hwnd);
                    SetFocus(list);
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 as u32 == VK_ESCAPE.0 as u32 {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                LRESULT(0)
            }
            WM_DESTROY => {
                let parent = with_dictionary_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                if parent.0 != 0 {
                    if with_state(parent, |state| {
                        state.dictionary_window = HWND(0);
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access dictionary state");
                    }
                    EnableWindow(parent, true);
                    SetForegroundWindow(parent);
                    let _unused =
                        PostMessageW(parent, crate::WM_FOCUS_EDITOR, WPARAM(0), LPARAM(0));
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DictionaryWindowState;
                if !ptr.is_null() {
                    let _unused_box = Box::from_raw(ptr);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn with_dictionary_state<F, R>(hwnd: HWND, f: F) -> Option<R>
where
    F: FnOnce(&mut DictionaryWindowState) -> R,
{
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut DictionaryWindowState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn update_button_states(hwnd: HWND) {
    let (hwnd_list, hwnd_edit, hwnd_remove) =
        match with_dictionary_state(hwnd, |s| (s.hwnd_list, s.hwnd_edit, s.hwnd_remove)) {
            Some(values) => values,
            None => return,
        };

    let count = crate::send_message_w_safe(hwnd_list, LB_GETCOUNT, WPARAM(0), LPARAM(0)).0;
    let sel = crate::send_message_w_safe(hwnd_list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    let has_selection = count > 0 && sel >= 0;
    unsafe {
        EnableWindow(hwnd_edit, has_selection);
        EnableWindow(hwnd_remove, has_selection);
    }
}

pub fn refresh_dictionary_list(hwnd: HWND) {
    let (parent, hwnd_list) = match with_dictionary_state(hwnd, |s| (s.parent, s.hwnd_list)) {
        Some(values) => values,
        None => return,
    };

    let selected = crate::send_message_w_safe(hwnd_list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    crate::send_message_w_safe(hwnd_list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));

    let entries =
        { with_state(parent, |state| state.settings.dictionary.clone()) }.unwrap_or_default();
    for (idx, entry) in entries.iter().enumerate() {
        let label = format!("{} -> {}", entry.original, entry.replacement);
        let lb_idx = crate::send_message_w_safe(
            hwnd_list,
            LB_ADDSTRING,
            WPARAM(0),
            LPARAM(to_wide(&label).as_ptr() as isize),
        )
        .0;
        if lb_idx >= 0 {
            crate::send_message_w_safe(
                hwnd_list,
                LB_SETITEMDATA,
                WPARAM(lb_idx as usize),
                LPARAM(idx as isize),
            );
        }
    }

    let count = crate::send_message_w_safe(hwnd_list, LB_GETCOUNT, WPARAM(0), LPARAM(0)).0;
    if count > 0 {
        let target = if selected >= 0 && selected < count {
            selected
        } else {
            0
        };
        crate::send_message_w_safe(hwnd_list, LB_SETCURSEL, WPARAM(target as usize), LPARAM(0));
    }
    update_button_states(hwnd);
}

fn selected_dictionary_index(hwnd: HWND) -> Option<usize> {
    let hwnd_list = with_dictionary_state(hwnd, |s| s.hwnd_list).unwrap_or(HWND(0));
    if hwnd_list.0 == 0 {
        return None;
    }
    let sel = crate::send_message_w_safe(hwnd_list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel < 0 {
        return None;
    }
    let idx =
        crate::send_message_w_safe(hwnd_list, LB_GETITEMDATA, WPARAM(sel as usize), LPARAM(0)).0;
    if idx < 0 {
        return None;
    }
    Some(idx as usize)
}

fn remove_selected_entry(hwnd: HWND) {
    let (parent, _list) = match with_dictionary_state(hwnd, |s| (s.parent, s.hwnd_list)) {
        Some(values) => values,
        None => return,
    };
    let Some(index) = selected_dictionary_index(hwnd) else {
        return;
    };
    let result = with_state(parent, |state| {
        let Some(entry) = state.settings.dictionary.get(index).cloned() else {
            return Ok(());
        };
        let entries =
            save_dictionary_change(state.settings.clone(), DictionaryChange::Remove(entry))?;
        state.settings.dictionary = entries;
        Ok::<(), String>(())
    });
    if let Some(Err(error)) = result {
        let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
        crate::show_error(hwnd, language, &error);
    }
    refresh_dictionary_list(hwnd);
    if let Err(_e) = crate::post_message_w_safe(hwnd, DICT_FOCUS_LIST_MSG, WPARAM(0), LPARAM(0)) {
        crate::log_debug(&format!("Error: {:?}", _e));
    }
}

fn open_entry_dialog(owner: HWND, index: Option<usize>) {
    let parent = with_dictionary_state(owner, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }

    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let class_name = to_wide(DICTIONARY_ENTRY_CLASS_NAME);
    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(unsafe {
            LoadCursorW(None, IDC_ARROW).unwrap_or_default().0
        }),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(dictionary_entry_wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    crate::register_class_w_safe(&wc);

    let language = { with_state(parent, |state| state.settings.language) }.unwrap_or_default();
    let labels = dictionary_labels(language);
    let title = if index.is_some() {
        &labels.entry_title_edit
    } else {
        &labels.entry_title_add
    };

    let params = Box::new(DictionaryEntryState {
        parent,
        owner,
        edit_original: HWND(0),
        edit_replacement: HWND(0),
        checkbox_match_case: HWND(0),
        ok_button: HWND(0),
        index,
        checkbox_use_voice: HWND(0),
        label_engine: HWND(0),
        combo_engine: HWND(0),
        label_voice: HWND(0),
        combo_voice: HWND(0),
        button_preview: HWND(0),
    });

    let dialog = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide(title).as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            0,
            0,
            420,
            380,
            owner,
            None,
            hinstance,
            Some(Box::into_raw(params) as *const std::ffi::c_void),
        )
    };

    if dialog.0 != 0 {
        if {
            with_state(parent, |state| {
                state.dictionary_entry_dialog = dialog;
            })
        }
        .is_none()
        {
            crate::log_debug("Failed to access dictionary state");
        }
        unsafe {
            EnableWindow(owner, false);
            SetForegroundWindow(dialog);
        }
    }
}

unsafe extern "system" fn dictionary_entry_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "dictionary_entry_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || dictionary_entry_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn dictionary_entry_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let create_struct = lparam.0 as *const CREATESTRUCTW;
                let state_ptr = (*create_struct).lpCreateParams as *mut DictionaryEntryState;
                if state_ptr.is_null() {
                    return LRESULT(0);
                }
                let mut state = Box::from_raw(state_ptr);
                let language =
                    with_state(state.parent, |s| s.settings.language).unwrap_or_default();
                let labels = dictionary_labels(language);
                let hfont = with_state(state.parent, |s| s.hfont).unwrap_or(HFONT(0));

                let label_original = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.entry_original).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    10,
                    10,
                    380,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let edit_original = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    10,
                    32,
                    380,
                    24,
                    hwnd,
                    HMENU(DICT_ENTRY_ID_ORIGINAL as isize),
                    HINSTANCE(0),
                    None,
                );

                let label_replacement = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.entry_replacement).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    10,
                    64,
                    380,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let edit_replacement = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    10,
                    86,
                    380,
                    24,
                    hwnd,
                    HMENU(DICT_ENTRY_ID_REPLACEMENT as isize),
                    HINSTANCE(0),
                    None,
                );
                let checkbox_match_case = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.entry_match_case).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    10,
                    118,
                    380,
                    20,
                    hwnd,
                    HMENU(DICT_ENTRY_ID_MATCH_CASE as isize),
                    HINSTANCE(0),
                    None,
                );

                // Voice controls
                let checkbox_use_voice = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.entry_use_voice).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    10,
                    150,
                    380,
                    20,
                    hwnd,
                    HMENU(DICT_ENTRY_ID_USE_VOICE as isize),
                    HINSTANCE(0),
                    None,
                );

                let label_engine = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.entry_engine).as_ptr()),
                    WS_CHILD,
                    10,
                    180,
                    120,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let combo_engine = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    140,
                    178,
                    250,
                    120,
                    hwnd,
                    HMENU(DICT_ENTRY_ID_ENGINE as isize),
                    HINSTANCE(0),
                    None,
                );

                let label_voice = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.entry_voice).as_ptr()),
                    WS_CHILD,
                    10,
                    215,
                    120,
                    20,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let combo_voice = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    140,
                    213,
                    250,
                    200,
                    hwnd,
                    HMENU(DICT_ENTRY_ID_VOICE as isize),
                    HINSTANCE(0),
                    None,
                );

                let button_preview = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.entry_preview).as_ptr()),
                    WS_CHILD | WS_TABSTOP,
                    140,
                    248,
                    250,
                    26,
                    hwnd,
                    HMENU(DICT_ENTRY_ID_PREVIEW as isize),
                    HINSTANCE(0),
                    None,
                );

                // Populate engine combo
                SendMessageW(
                    combo_engine,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&labels.engine_edge).as_ptr() as isize),
                );
                SendMessageW(
                    combo_engine,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&labels.engine_sapi5).as_ptr() as isize),
                );
                SendMessageW(
                    combo_engine,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&labels.engine_sapi4).as_ptr() as isize),
                );
                SendMessageW(
                    combo_engine,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&labels.engine_google).as_ptr() as isize),
                );
                SendMessageW(combo_engine, CB_SETCURSEL, WPARAM(0), LPARAM(0));

                let ok_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.entry_ok).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    200,
                    320,
                    90,
                    28,
                    hwnd,
                    HMENU(DICT_ENTRY_ID_OK as isize),
                    HINSTANCE(0),
                    None,
                );
                let cancel_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.entry_cancel).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    300,
                    320,
                    90,
                    28,
                    hwnd,
                    HMENU(DICT_ENTRY_ID_CANCEL as isize),
                    HINSTANCE(0),
                    None,
                );

                for ctrl in [
                    label_original,
                    edit_original,
                    label_replacement,
                    edit_replacement,
                    checkbox_match_case,
                    checkbox_use_voice,
                    label_engine,
                    combo_engine,
                    label_voice,
                    combo_voice,
                    button_preview,
                    ok_button,
                    cancel_button,
                ] {
                    if ctrl.0 != 0 && hfont.0 != 0 {
                        SendMessageW(ctrl, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                }

                // Load existing entry data if editing
                let saved_voice: Option<String> = if let Some(index) = state.index
                    && let Some(entry) =
                        with_state(state.parent, |s| s.settings.dictionary.get(index).cloned())
                            .unwrap_or(None)
                {
                    if let Err(_e) =
                        SetWindowTextW(edit_original, PCWSTR(to_wide(&entry.original).as_ptr()))
                    {
                        crate::log_debug(&format!("Failed to set edit_original text: {:?}", _e));
                    }
                    if let Err(_e) = SetWindowTextW(
                        edit_replacement,
                        PCWSTR(to_wide(&entry.replacement).as_ptr()),
                    ) {
                        crate::log_debug(&format!("Failed to set edit_replacement text: {:?}", _e));
                    }
                    SendMessageW(
                        checkbox_match_case,
                        BM_SETCHECK,
                        WPARAM(if entry.match_case {
                            BST_CHECKED.0 as usize
                        } else {
                            0
                        }),
                        LPARAM(0),
                    );
                    None
                } else {
                    SendMessageW(
                        checkbox_match_case,
                        BM_SETCHECK,
                        WPARAM(BST_CHECKED.0 as usize),
                        LPARAM(0),
                    );
                    None
                };

                state.edit_original = edit_original;
                state.edit_replacement = edit_replacement;
                state.checkbox_match_case = checkbox_match_case;
                state.ok_button = ok_button;
                state.checkbox_use_voice = checkbox_use_voice;
                state.label_engine = label_engine;
                state.combo_engine = combo_engine;
                state.label_voice = label_voice;
                state.combo_voice = combo_voice;
                state.button_preview = button_preview;

                let state_ptr = Box::into_raw(state);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);

                // Populate voice combo and update visibility
                populate_entry_voice_combo(hwnd, saved_voice.as_deref());
                update_voice_controls_visibility(hwnd);

                SetFocus(edit_original);
                LRESULT(0)
            }
            WM_COMMAND => {
                let cmd_id = wparam.0 & 0xffff;
                match cmd_id {
                    DICT_ENTRY_ID_OK => {
                        apply_entry_dialog(hwnd);
                        LRESULT(0)
                    }
                    cmd if cmd == IDOK.0 as usize => {
                        apply_entry_dialog(hwnd);
                        LRESULT(0)
                    }
                    DICT_ENTRY_ID_CANCEL | 2 => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    DICT_ENTRY_ID_USE_VOICE => {
                        update_voice_controls_visibility(hwnd);
                        LRESULT(0)
                    }
                    DICT_ENTRY_ID_ENGINE => {
                        populate_entry_voice_combo(hwnd, None);
                        LRESULT(0)
                    }
                    DICT_ENTRY_ID_PREVIEW => {
                        preview_entry_voice(hwnd);
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_KEYDOWN => {
                if wparam.0 as u32 == VK_ESCAPE.0 as u32 {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                LRESULT(0)
            }
            WM_DESTROY => {
                let (owner, parent) =
                    with_entry_state(hwnd, |s| (s.owner, s.parent)).unwrap_or((HWND(0), HWND(0)));
                if owner.0 != 0 {
                    EnableWindow(owner, true);
                    SetForegroundWindow(owner);
                    if let Err(_e) = PostMessageW(owner, DICT_FOCUS_LIST_MSG, WPARAM(0), LPARAM(0))
                    {
                        crate::log_debug(&format!("Error: {:?}", _e));
                    }
                }
                if parent.0 != 0
                    && with_state(parent, |state| {
                        state.dictionary_entry_dialog = HWND(0);
                    })
                    .is_none()
                {
                    crate::log_debug("Failed to access dictionary state");
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DictionaryEntryState;
                if !ptr.is_null() {
                    let _unused_box = Box::from_raw(ptr);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn with_entry_state<F, R>(hwnd: HWND, f: F) -> Option<R>
where
    F: FnOnce(&mut DictionaryEntryState) -> R,
{
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut DictionaryEntryState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn apply_entry_dialog(hwnd: HWND) {
    let (parent, owner, edit_original, edit_replacement, checkbox_match_case, index) =
        match with_entry_state(hwnd, |s| {
            (
                s.parent,
                s.owner,
                s.edit_original,
                s.edit_replacement,
                s.checkbox_match_case,
                s.index,
            )
        }) {
            Some(values) => values,
            None => return,
        };

    let original = get_window_text(edit_original);
    let replacement = get_window_text(edit_replacement);
    let match_case = unsafe {
        SendMessageW(checkbox_match_case, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 as u32
            == BST_CHECKED.0
    };
    if original.trim().is_empty() {
        return;
    }

    // Custom voices are now handled via explicit <voice> tags in the text.
    let use_voice = false;
    let custom_engine = None;
    let custom_voice = None;

    let result = with_state(parent, |state| {
        let entry = DictionaryEntry {
            original,
            replacement,
            match_case,
            use_custom_voice: use_voice,
            custom_voice_engine: custom_engine,
            custom_voice,
        };
        let change = match index {
            Some(idx) => {
                let Some(previous) = state.settings.dictionary.get(idx).cloned() else {
                    return Ok(());
                };
                DictionaryChange::Replace(previous, entry)
            }
            None => DictionaryChange::Add(entry),
        };
        let entries = save_dictionary_change(state.settings.clone(), change)?;
        state.settings.dictionary = entries;
        Ok::<(), String>(())
    });
    if let Some(Err(error)) = result {
        let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
        crate::show_error(hwnd, language, &error);
        return;
    }

    refresh_dictionary_list(owner);
    if let Err(_e) = crate::post_message_w_safe(owner, DICT_FOCUS_LIST_MSG, WPARAM(0), LPARAM(0)) {
        crate::log_debug(&format!("Error: {:?}", _e));
    }
    crate::log_if_err!(crate::destroy_window_safe(hwnd));
}

fn get_window_text(hwnd: HWND) -> String {
    let len = crate::get_window_text_length_w_safe(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let read = crate::get_window_text_w_safe(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..read as usize])
}

fn update_voice_controls_visibility(hwnd: HWND) {
    let (checkbox_use_voice, label_engine, combo_engine, label_voice, combo_voice, button_preview) =
        match with_entry_state(hwnd, |s| {
            (
                s.checkbox_use_voice,
                s.label_engine,
                s.combo_engine,
                s.label_voice,
                s.combo_voice,
                s.button_preview,
            )
        }) {
            Some(values) => values,
            None => return,
        };

    let controls = [
        checkbox_use_voice,
        label_engine,
        combo_engine,
        label_voice,
        combo_voice,
        button_preview,
    ];
    for control in controls {
        unsafe {
            ShowWindow(control, SW_HIDE);
            EnableWindow(control, false);
        }
    }
}

fn populate_entry_voice_combo(hwnd: HWND, selected_voice: Option<&str>) {
    let (parent, combo_engine, combo_voice) =
        match with_entry_state(hwnd, |s| (s.parent, s.combo_engine, s.combo_voice)) {
            Some(values) => values,
            None => return,
        };

    let engine_sel = crate::send_message_w_safe(combo_engine, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    let engine = match engine_sel {
        1 => TtsEngine::Sapi5,
        2 => TtsEngine::Sapi4,
        3 => TtsEngine::Google,
        _ => TtsEngine::Edge,
    };

    let voices: Vec<VoiceInfo> = {
        with_state(parent, |state| match engine {
            TtsEngine::Edge => state.edge_voices.clone(),
            TtsEngine::Google => crate::google_tts::installed_voices(),
            TtsEngine::Sapi5 => state.sapi_voices.clone(),
            TtsEngine::Sapi4 => crate::sapi4_engine::get_voices(),
        })
        .unwrap_or_default()
    };

    let language =
        { with_state(parent, |state| state.settings.language).unwrap_or(Language::Italian) };
    let labels = dictionary_labels(language);

    crate::send_message_w_safe(combo_voice, CB_RESETCONTENT, WPARAM(0), LPARAM(0));

    if voices.is_empty() {
        crate::send_message_w_safe(
            combo_voice,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(to_wide(&labels.voices_empty).as_ptr() as isize),
        );
        crate::send_message_w_safe(combo_voice, CB_SETCURSEL, WPARAM(0), LPARAM(0));
        return;
    }

    let mut selected_index: Option<usize> = None;
    let mut combo_index = 0usize;

    for (voice_index, voice) in voices.iter().enumerate() {
        let display_name = if engine == TtsEngine::Google {
            crate::google_tts::voice_display_name(&voice.short_name)
        } else {
            voice.short_name.clone()
        };
        let label = format!("{} ({})", display_name, voice.locale);
        let wide = to_wide(&label);
        let idx = crate::send_message_w_safe(
            combo_voice,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        )
        .0;
        if idx >= 0 {
            crate::send_message_w_safe(
                combo_voice,
                CB_SETITEMDATA,
                WPARAM(idx as usize),
                LPARAM(voice_index as isize),
            );
            if let Some(sel) = selected_voice
                && voice.short_name == sel
            {
                selected_index = Some(combo_index);
            }
            combo_index += 1;
        }
    }

    if let Some(idx) = selected_index {
        crate::send_message_w_safe(combo_voice, CB_SETCURSEL, WPARAM(idx), LPARAM(0));
    } else if combo_index > 0 {
        crate::send_message_w_safe(combo_voice, CB_SETCURSEL, WPARAM(0), LPARAM(0));
    }
}

fn preview_entry_voice(hwnd: HWND) {
    let (parent, edit_original, edit_replacement, combo_engine, combo_voice) =
        match with_entry_state(hwnd, |s| {
            (
                s.parent,
                s.edit_original,
                s.edit_replacement,
                s.combo_engine,
                s.combo_voice,
            )
        }) {
            Some(values) => values,
            None => return,
        };

    let language =
        { with_state(parent, |state| state.settings.language).unwrap_or(Language::Italian) };

    // Use the replacement text if available, otherwise use the original word
    let replacement = get_window_text(edit_replacement);
    let original = get_window_text(edit_original);
    let text = if replacement.trim().is_empty() {
        if original.trim().is_empty() {
            i18n::tr(language, "tts.preview_text")
        } else {
            original
        }
    } else {
        replacement
    };

    if text.trim().is_empty() {
        return;
    }

    let engine_sel = crate::send_message_w_safe(combo_engine, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    let engine = match engine_sel {
        1 => TtsEngine::Sapi5,
        2 => TtsEngine::Sapi4,
        3 => TtsEngine::Google,
        _ => TtsEngine::Edge,
    };

    let voices: Vec<VoiceInfo> = {
        with_state(parent, |state| match engine {
            TtsEngine::Edge => state.edge_voices.clone(),
            TtsEngine::Google => crate::google_tts::installed_voices(),
            TtsEngine::Sapi5 => state.sapi_voices.clone(),
            TtsEngine::Sapi4 => crate::sapi4_engine::get_voices(),
        })
        .unwrap_or_default()
    };

    let voice_sel = crate::send_message_w_safe(combo_voice, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if voice_sel < 0 {
        return;
    }
    let voice_index = crate::send_message_w_safe(
        combo_voice,
        CB_GETITEMDATA,
        WPARAM(voice_sel as usize),
        LPARAM(0),
    )
    .0 as usize;
    if voice_index >= voices.len() {
        return;
    }
    let voice = voices[voice_index].short_name.clone();

    // Use default rate/pitch/volume
    let rate = 0;
    let pitch = 0;
    let volume = 100;

    match engine {
        TtsEngine::Edge | TtsEngine::Google => {
            let chunks = tts_engine::split_into_tts_chunks(&text, false, &[], engine);
            let options = tts_engine::TtsPlaybackOptions {
                hwnd: parent,
                engine,
                cleaned: text,
                voice,
                chunks,
                initial_caret_pos: 0,
                source_edit: HWND(0),
                rate,
                pitch,
                volume,
            };
            tts_engine::start_tts_playback_with_chunks(options);
        }
        TtsEngine::Sapi4 => {
            tts_engine::stop_tts_playback(parent);
            let voice_idx = if let Some(hash_pos) = voice.find('#') {
                let rest = &voice[hash_pos + 1..];
                if let Some(pipe_pos) = rest.find('|') {
                    rest[..pipe_pos].parse::<i32>().unwrap_or(1)
                } else {
                    rest.parse::<i32>().unwrap_or(1)
                }
            } else {
                1
            };
            let cancel = Arc::new(AtomicBool::new(false));
            let (command_tx, command_rx) = mpsc::unbounded_channel();
            if {
                with_state(parent, |state| {
                    state.tts_session = Some(tts_engine::TtsSession {
                        id: state.tts_next_session_id,
                        command_tx,
                        cancel: cancel.clone(),
                        paused: false,
                        initial_caret_pos: 0,
                        source_edit: HWND(0),
                    });
                    state.tts_next_session_id += 1;
                })
            }
            .is_none()
            {
                crate::log_debug("Failed to access state in dictionary_window");
            }
            crate::sapi4_engine::play_sapi4(
                voice_idx, text, rate, pitch, volume, cancel, command_rx,
            );
        }
        TtsEngine::Sapi5 => {
            tts_engine::stop_tts_playback(parent);
            let cancel = Arc::new(AtomicBool::new(false));
            let (command_tx, command_rx) = mpsc::unbounded_channel();
            if {
                with_state(parent, |state| {
                    state.tts_session = Some(tts_engine::TtsSession {
                        id: state.tts_next_session_id,
                        command_tx,
                        cancel: cancel.clone(),
                        paused: false,
                        initial_caret_pos: 0,
                        source_edit: HWND(0),
                    });
                    state.tts_next_session_id += 1;
                })
            }
            .is_none()
            {
                crate::log_debug("Failed to access state in dictionary_window");
            }
            let chunks = vec![text];
            if let Err(e) = crate::sapi5_engine::play_sapi(
                chunks, voice, rate, pitch, volume, cancel, command_rx,
            ) {
                crate::log_debug(&format!("SAPI5 preview error: {}", e));
            }
        }
    }
}
