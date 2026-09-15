use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::{BOOL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT, InvalidateRect};
use windows::Win32::UI::Controls::{
    HTREEITEM, TVE_EXPAND, TVGN_CARET, TVGN_PARENT, TVIF_PARAM, TVIF_TEXT, TVINSERTSTRUCTW,
    TVINSERTSTRUCTW_0, TVITEMW, TVM_ENSUREVISIBLE, TVM_EXPAND, TVM_GETITEMW, TVM_GETNEXTITEM,
    TVM_INSERTITEMW, TVM_SELECTITEM, TVS_HASBUTTONS, TVS_HASLINES, TVS_LINESATROOT,
    TVS_SHOWSELALWAYS, WC_BUTTON, WC_EDIT, WC_STATIC,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, SetFocus, VK_APPS, VK_CONTROL, VK_DOWN, VK_END, VK_ESCAPE, VK_F10, VK_HOME,
    VK_LEFT, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, BS_DEFPUSHBUTTON, CREATESTRUCTW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyMenu, DispatchMessageW, EN_CHANGE, ES_AUTOHSCROLL, GWLP_USERDATA,
    GetCursorPos, GetWindowTextLengthW, GetWindowTextW, HMENU, HWND_TOPMOST, IDC_ARROW,
    IsDialogMessageW, LB_ADDSTRING, LB_GETCURSEL, LB_GETTEXT, LB_GETTEXTLEN, LB_RESETCONTENT,
    LB_SETCARETINDEX, LB_SETCURSEL, LBS_NOTIFY, LoadCursorW, MF_POPUP, MF_STRING, MSG,
    PostMessageW, SW_HIDE, SW_SHOW, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SendMessageW,
    SetForegroundWindow, SetWindowPos, SetWindowTextW, ShowWindow, TPM_NONOTIFY, TPM_RETURNCMD,
    TrackPopupMenu, TranslateMessage, WINDOW_STYLE, WM_ACTIVATE, WM_APP, WM_CLOSE, WM_COMMAND,
    WM_CONTEXTMENU, WM_CREATE, WM_KEYDOWN, WM_NCDESTROY, WM_SETFONT, WM_SETREDRAW, WNDCLASSW,
    WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_SYSMENU,
    WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, w};

use crate::accessibility::to_wide;
use crate::i18n;
use crate::settings::Language;
use crate::with_state;

const INTERPRETER_SELECT_CLASS_NAME: &str = "SonarpadInterpreterSelect";
const ID_LIST: usize = 9201;
const ID_OK: usize = 9202;
const ID_CANCEL: usize = 9203;
const ID_SECONDARY: usize = 9204;
const ID_FILTER_EDIT: usize = 9205;
const ID_FLAT_LIST: usize = 9206;
const WM_RESTORE_LIST_FOCUS: u32 = WM_APP + 1;

// Keep track of open interpreter selectors so a media-save action launched from
// a context menu can return focus to the exact list that initiated it. Store
// raw HWND values to avoid imposing Send/Sync requirements on HWND itself.
static ACTIVE_INTERPRETER_SELECT_WINDOWS: Mutex<Vec<(isize, isize)>> = Mutex::new(Vec::new());

fn register_active_interpreter_select(hwnd: HWND, parent: HWND) {
    let mut windows = ACTIVE_INTERPRETER_SELECT_WINDOWS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    windows.retain(|(dialog, _)| *dialog != hwnd.0);
    windows.push((hwnd.0, parent.0));
}

fn unregister_active_interpreter_select(hwnd: HWND) {
    let mut windows = ACTIVE_INTERPRETER_SELECT_WINDOWS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    windows.retain(|(dialog, _)| *dialog != hwnd.0);
}

fn active_interpreter_select_for_parent(parent: HWND) -> Option<HWND> {
    let mut windows = ACTIVE_INTERPRETER_SELECT_WINDOWS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    windows.retain(|(dialog, _)| crate::is_window_handle_valid(HWND(*dialog)));
    windows
        .iter()
        .rev()
        .find(|(_, registered_parent)| *registered_parent == parent.0)
        .map(|(dialog, _)| HWND(*dialog))
}

pub fn has_active_interpreter_select_for_parent(parent: HWND) -> bool {
    active_interpreter_select_for_parent(parent).is_some()
}

pub fn restore_active_interpreter_select_for_parent(parent: HWND) -> bool {
    let Some(dialog) = active_interpreter_select_for_parent(parent) else {
        return false;
    };
    crate::log_debug(&format!(
        "Restoring active interpreter selector after media action dialog={:?} parent={:?}",
        dialog, parent
    ));
    crate::enable_window_safe(parent, false);
    crate::set_foreground_window_safe(dialog);
    let restored = restore_interpreter_select_focus(dialog);
    if restored {
        // Context-menu media actions run inside a nested native menu/modal loop.
        // The immediate SetFocus above can be overwritten while that loop unwinds,
        // leaving the disabled main window in front after an error. Queue one more
        // focus restore for the selector's normal message loop so the results list
        // is reliably foreground and keyboard-active once the action has finished.
        if let Err(err) =
            crate::post_message_w_safe(dialog, WM_RESTORE_LIST_FOCUS, WPARAM(0), LPARAM(0))
        {
            crate::log_debug(&format!(
                "Failed to queue deferred interpreter selector focus restore: {}",
                err
            ));
        }
    }
    restored
}

pub fn request_close_active_interpreter_selects_for_parent(parent: HWND) -> bool {
    let dialogs = {
        let mut windows = ACTIVE_INTERPRETER_SELECT_WINDOWS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        windows.retain(|(dialog, _)| crate::is_window_handle_valid(HWND(*dialog)));
        windows
            .iter()
            .filter(|(_, registered_parent)| *registered_parent == parent.0)
            .map(|(dialog, _)| HWND(*dialog))
            .collect::<Vec<_>>()
    };
    if dialogs.is_empty() {
        return false;
    }
    for dialog in dialogs.into_iter().rev() {
        crate::log_debug(&format!(
            "Requesting interpreter selector close before context transcription dialog={:?} parent={:?}",
            dialog, parent
        ));
        crate::log_if_err!(unsafe { PostMessageW(dialog, WM_CLOSE, WPARAM(0), LPARAM(0)) });
    }
    true
}

pub fn close_active_interpreter_selects_for_parent(parent: HWND) -> bool {
    let dialogs = {
        let mut windows = ACTIVE_INTERPRETER_SELECT_WINDOWS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        windows.retain(|(dialog, _)| crate::is_window_handle_valid(HWND(*dialog)));
        windows
            .iter()
            .filter(|(_, registered_parent)| *registered_parent == parent.0)
            .map(|(dialog, _)| HWND(*dialog))
            .collect::<Vec<_>>()
    };
    if dialogs.is_empty() {
        return false;
    }
    for dialog in dialogs.into_iter().rev() {
        crate::log_debug(&format!(
            "Closing active interpreter selector for transcription result dialog={:?} parent={:?}",
            dialog, parent
        ));
        crate::log_if_err!(crate::destroy_window_safe(dialog));
    }
    true
}

type ContextActionEnabled = Arc<dyn Fn(&str) -> bool + Send + Sync>;
type ContextActionHandler = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Clone)]
pub(crate) struct GroupedSelectItem {
    pub(crate) label: String,
    pub(crate) value: String,
}

#[derive(Clone)]
pub(crate) struct GroupedSelectGroup {
    pub(crate) label: String,
    pub(crate) items: Vec<GroupedSelectItem>,
    pub(crate) hidden_in_tree: bool,
}

#[derive(Clone)]
pub(crate) enum InterpreterSelectionResult {
    Item(String),
    SecondaryAction,
}

#[derive(Clone)]
enum InterpreterDialogInitMode {
    List(Vec<String>),
    Tree(Vec<GroupedSelectGroup>),
}

struct InterpreterSelectInit {
    parent: HWND,
    mode: InterpreterDialogInitMode,
    language: Language,
    secondary_action_label: Option<String>,
    initial_list_value: Option<String>,
    initial_tree_value: Option<String>,
    filter_label: Option<String>,
    context_actions: Vec<InterpreterContextAction>,
    result: Arc<Mutex<Option<InterpreterSelectionResult>>>,
}

#[derive(Default)]
struct InterpreterSelectOptions {
    suppress_parent_restore_on_accept: bool,
    suppress_parent_restore_on_secondary: bool,
    suppress_parent_restore_on_cancel: bool,
    pin_topmost: bool,
    secondary_action_label: Option<String>,
    initial_list_value: Option<String>,
    initial_tree_value: Option<String>,
    filter_label: Option<String>,
    context_actions: Vec<InterpreterContextAction>,
}

enum ControlKind {
    List(HWND),
    Tree(HWND),
}

struct InterpreterSelectState {
    control: ControlKind,
    original_mode: InterpreterDialogInitMode,
    language: Language,
    initial_list_value: Option<String>,
    secondary_button: Option<HWND>,
    filter_edit: Option<HWND>,
    flat_list: Option<HWND>,
    flat_list_values: Vec<FlatListValue>,
    tree_values: Vec<String>,
    context_actions: Vec<InterpreterContextAction>,
    result: Arc<Mutex<Option<InterpreterSelectionResult>>>,
}

#[derive(Clone)]
enum FlatListValue {
    Group(String),
    Item(String),
}

#[derive(Clone)]
pub struct InterpreterContextAction {
    pub label: String,
    pub ctrl_c_shortcut: bool,
    pub delete_shortcut: bool,
    pub enabled: ContextActionEnabled,
    pub handler: ContextActionHandler,
    pub children: Vec<InterpreterContextAction>,
}

pub(crate) fn append_context_actions_to_menu(
    menu: HMENU,
    actions: &[InterpreterContextAction],
    selected_value: &str,
) -> Result<Vec<InterpreterContextAction>, String> {
    fn append_recursive(
        menu: HMENU,
        actions: &[InterpreterContextAction],
        selected_value: &str,
        commands: &mut Vec<InterpreterContextAction>,
    ) -> Result<usize, String> {
        let mut appended = 0usize;
        for action in actions {
            if !(action.enabled)(selected_value) {
                continue;
            }
            if action.children.is_empty() {
                let command_id = commands.len() + 1;
                let label_w = to_wide(&action.label);
                unsafe {
                    AppendMenuW(menu, MF_STRING, command_id, PCWSTR(label_w.as_ptr()))
                        .map_err(|err| err.to_string())?;
                }
                commands.push(action.clone());
                appended += 1;
                continue;
            }

            let submenu = unsafe { CreatePopupMenu() }.map_err(|err| err.to_string())?;
            let child_count =
                match append_recursive(submenu, &action.children, selected_value, commands) {
                    Ok(count) => count,
                    Err(err) => {
                        crate::log_if_err!(unsafe { DestroyMenu(submenu) });
                        return Err(err);
                    }
                };
            if child_count == 0 {
                crate::log_if_err!(unsafe { DestroyMenu(submenu) });
                continue;
            }
            let label_w = to_wide(&action.label);
            if let Err(err) =
                unsafe { AppendMenuW(menu, MF_POPUP, submenu.0 as usize, PCWSTR(label_w.as_ptr())) }
            {
                crate::log_if_err!(unsafe { DestroyMenu(submenu) });
                return Err(err.to_string());
            }
            appended += 1;
        }
        Ok(appended)
    }

    let mut commands = Vec::new();
    append_recursive(menu, actions, selected_value, &mut commands)?;
    Ok(commands)
}

pub struct InterpreterSecondaryActionOptions {
    pub label: String,
    pub filter_label: Option<String>,
}

pub fn select_interpreter(
    parent: HWND,
    items: Vec<String>,
    language: Language,
    title: String,
) -> Option<String> {
    match select_interpreter_internal(
        parent,
        InterpreterDialogInitMode::List(items),
        language,
        title,
        InterpreterSelectOptions::default(),
    ) {
        Some(InterpreterSelectionResult::Item(value)) => Some(value),
        _ => None,
    }
}

pub fn select_interpreter_with_secondary_action_and_context_actions_and_initial_without_parent_restore(
    parent: HWND,
    items: Vec<String>,
    language: Language,
    title: String,
    secondary_action: InterpreterSecondaryActionOptions,
    initial_value: Option<String>,
    context_actions: Vec<InterpreterContextAction>,
) -> Option<InterpreterSelectionResult> {
    select_interpreter_internal(
        parent,
        InterpreterDialogInitMode::List(items),
        language,
        title,
        InterpreterSelectOptions {
            filter_label: secondary_action.filter_label,
            secondary_action_label: Some(secondary_action.label),
            initial_list_value: initial_value,
            context_actions,
            suppress_parent_restore_on_accept: true,
            suppress_parent_restore_on_secondary: true,
            ..Default::default()
        },
    )
}

pub fn select_interpreter_with_context_actions_without_parent_restore_on_accept(
    parent: HWND,
    items: Vec<String>,
    language: Language,
    title: String,
    initial_value: Option<String>,
    context_actions: Vec<InterpreterContextAction>,
) -> Option<String> {
    match select_interpreter_internal(
        parent,
        InterpreterDialogInitMode::List(items),
        language,
        title,
        InterpreterSelectOptions {
            initial_list_value: initial_value,
            context_actions,
            suppress_parent_restore_on_accept: true,
            suppress_parent_restore_on_cancel: true,
            ..Default::default()
        },
    ) {
        Some(InterpreterSelectionResult::Item(value)) => Some(value),
        _ => None,
    }
}

pub fn select_grouped_interpreter_with_context_actions_without_parent_restore_on_accept(
    parent: HWND,
    groups: Vec<GroupedSelectGroup>,
    language: Language,
    title: String,
    filter_label: Option<String>,
    initial_value: Option<String>,
    context_actions: Vec<InterpreterContextAction>,
) -> Option<String> {
    match select_interpreter_internal(
        parent,
        InterpreterDialogInitMode::Tree(groups),
        language,
        title,
        InterpreterSelectOptions {
            filter_label,
            suppress_parent_restore_on_accept: true,
            suppress_parent_restore_on_cancel: true,
            pin_topmost: true,
            initial_tree_value: initial_value,
            context_actions,
            ..Default::default()
        },
    ) {
        Some(InterpreterSelectionResult::Item(value)) => Some(value),
        _ => None,
    }
}

pub fn select_grouped_interpreter_with_secondary_action_and_context_actions_without_parent_restore_on_accept(
    parent: HWND,
    groups: Vec<GroupedSelectGroup>,
    language: Language,
    title: String,
    secondary_action: InterpreterSecondaryActionOptions,
    initial_value: Option<String>,
    context_actions: Vec<InterpreterContextAction>,
) -> Option<InterpreterSelectionResult> {
    select_interpreter_internal(
        parent,
        InterpreterDialogInitMode::Tree(groups),
        language,
        title,
        InterpreterSelectOptions {
            filter_label: secondary_action.filter_label,
            secondary_action_label: Some(secondary_action.label),
            suppress_parent_restore_on_accept: true,
            suppress_parent_restore_on_secondary: true,
            suppress_parent_restore_on_cancel: true,
            pin_topmost: true,
            initial_tree_value: initial_value,
            context_actions,
            ..Default::default()
        },
    )
}

fn select_interpreter_internal(
    parent: HWND,
    mode: InterpreterDialogInitMode,
    language: Language,
    title: String,
    options: InterpreterSelectOptions,
) -> Option<InterpreterSelectionResult> {
    match &mode {
        InterpreterDialogInitMode::List(items) if items.is_empty() => return None,
        InterpreterDialogInitMode::Tree(groups) if groups.is_empty() => return None,
        _ => {}
    }

    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let class_name = to_wide(INTERPRETER_SELECT_CLASS_NAME);
    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(unsafe {
            LoadCursorW(None, IDC_ARROW).unwrap_or_default().0
        }),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(interpreter_select_wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    crate::register_class_w_safe(&wc);

    let result = Arc::new(Mutex::new(None));
    let init = Box::new(InterpreterSelectInit {
        parent,
        mode: mode.clone(),
        language,
        secondary_action_label: options.secondary_action_label,
        initial_list_value: options.initial_list_value,
        initial_tree_value: options.initial_tree_value,
        filter_label: options.filter_label,
        context_actions: options.context_actions,
        result: result.clone(),
    });
    let title = to_wide(&title);

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            620,
            360,
            parent,
            HMENU(0),
            hinstance,
            Some(Box::into_raw(init) as *const _),
        )
    };

    if hwnd.0 == 0 {
        return None;
    }

    register_active_interpreter_select(hwnd, parent);

    unsafe {
        EnableWindow(parent, false);
        SetForegroundWindow(hwnd);
        if options.pin_topmost
            && let Err(err) = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            )
        {
            crate::log_debug(&format!(
                "Failed to pin interpreter selection window {:?} as topmost: {}",
                hwnd, err
            ));
        }
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
            let focused = crate::get_focus_safe();
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_ESCAPE.0 as u32 {
                crate::log_if_err!(PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)));
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_RETURN.0 as u32 {
                let command_id = with_interpreter_state(hwnd, |state| {
                    if state
                        .secondary_button
                        .is_some_and(|button| focused == button)
                    {
                        ID_SECONDARY
                    } else {
                        ID_OK
                    }
                })
                .unwrap_or(ID_OK);
                crate::log_if_err!(PostMessageW(
                    hwnd,
                    WM_COMMAND,
                    WPARAM(command_id),
                    LPARAM(0)
                ));
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_RIGHT.0 as u32 {
                let is_flat_list_focused = with_interpreter_state(hwnd, |state| {
                    if let ControlKind::Tree(_) = state.control {
                        state
                            .flat_list
                            .map(|flat_list| focused == flat_list)
                            .unwrap_or(false)
                    } else {
                        false
                    }
                })
                .unwrap_or(false);
                if is_flat_list_focused {
                    crate::log_if_err!(PostMessageW(hwnd, WM_COMMAND, WPARAM(ID_OK), LPARAM(0)));
                    continue;
                }
            }
            if msg.message == WM_KEYDOWN
                && (msg.wParam.0 as u32 == VK_APPS.0 as u32
                    || (msg.wParam.0 as u32 == VK_F10.0 as u32
                        && crate::get_key_state_safe(VK_SHIFT.0 as i32) < 0))
            {
                let should_open_context_menu =
                    with_interpreter_state(hwnd, |state| match state.control {
                        ControlKind::List(list) => focused == list,
                        ControlKind::Tree(tree) => focused == tree,
                    })
                    .unwrap_or(false);
                if should_open_context_menu {
                    crate::log_if_err!(PostMessageW(
                        hwnd,
                        WM_CONTEXTMENU,
                        WPARAM(focused.0 as usize),
                        LPARAM(-1),
                    ));
                    continue;
                }
            }
            if msg.message == WM_KEYDOWN
                && msg.wParam.0 as u32 == 'C' as u32
                && crate::get_key_state_safe(VK_CONTROL.0 as i32) < 0
            {
                let handled = with_interpreter_state(hwnd, |state| {
                    let value = match state.control {
                        ControlKind::List(list) => {
                            if focused != list {
                                return false;
                            }
                            let Some(value) = selected_list_value(list) else {
                                return false;
                            };
                            value
                        }
                        ControlKind::Tree(tree) => {
                            if focused != tree {
                                return false;
                            }
                            if let Some(flat_list) = state.flat_list
                                && windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(
                                    flat_list,
                                )
                                .as_bool()
                            {
                                return false;
                            }
                            let Some(value) = selected_tree_value(tree, &state.tree_values) else {
                                return false;
                            };
                            value
                        }
                    };
                    let Some(action) = state
                        .context_actions
                        .iter()
                        .find(|action| action.ctrl_c_shortcut && (action.enabled)(&value))
                        .cloned()
                    else {
                        return false;
                    };
                    (action.handler)(value);
                    true
                })
                .unwrap_or(false);
                if handled {
                    continue;
                }
            }
            let control_has_navigation_key =
                with_interpreter_state(hwnd, |state| match state.control {
                    ControlKind::List(list) => {
                        focused == list && is_list_navigation_key(msg.wParam.0 as u32)
                    }
                    ControlKind::Tree(tree) => {
                        focused == tree && is_tree_navigation_key(msg.wParam.0 as u32)
                    }
                })
                .unwrap_or(false);
            if control_has_navigation_key {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
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

    let result_value = result.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let suppress_parent_restore = match &result_value {
        Some(InterpreterSelectionResult::Item(_)) => options.suppress_parent_restore_on_accept,
        Some(InterpreterSelectionResult::SecondaryAction) => {
            options.suppress_parent_restore_on_secondary
        }
        None => options.suppress_parent_restore_on_cancel,
    };
    unsafe {
        if !suppress_parent_restore {
            EnableWindow(parent, true);
        }
        if result_value.is_none() {
            SetForegroundWindow(parent);
        }
    }

    result_value
}

unsafe extern "system" fn interpreter_select_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "interpreter_select_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || interpreter_select_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn interpreter_select_wndproc_inner(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create_struct = lparam.0 as *const CREATESTRUCTW;
            let init_ptr = unsafe { (*create_struct).lpCreateParams as *mut InterpreterSelectInit };
            if init_ptr.is_null() {
                return LRESULT(0);
            }
            let init = crate::box_from_raw_safe(init_ptr);
            let parent = init.parent;
            let hfont = with_state(parent, |state| state.hfont).unwrap_or(HFONT(0));

            let (control, tree_values, flat_list) = match init.mode.clone() {
                InterpreterDialogInitMode::List(items) => {
                    let list = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            w!("LISTBOX"),
                            PCWSTR::null(),
                            WS_CHILD
                                | WS_VISIBLE
                                | WS_TABSTOP
                                | WS_VSCROLL
                                | WINDOW_STYLE(LBS_NOTIFY as u32),
                            10,
                            10,
                            580,
                            214,
                            hwnd,
                            HMENU(ID_LIST as isize),
                            HINSTANCE(0),
                            None,
                        )
                    };

                    let mut initial_index = 0usize;
                    for (index, item) in items.into_iter().enumerate() {
                        if init.initial_list_value.as_deref() == Some(item.as_str()) {
                            initial_index = index;
                        }
                        crate::send_message_w_safe(
                            list,
                            LB_ADDSTRING,
                            WPARAM(0),
                            LPARAM(to_wide(&item).as_ptr() as isize),
                        );
                    }
                    unsafe {
                        SendMessageW(list, LB_SETCURSEL, WPARAM(initial_index), LPARAM(0));
                        SetFocus(list);
                    }
                    (ControlKind::List(list), Vec::new(), None)
                }
                InterpreterDialogInitMode::Tree(groups) => {
                    let tree = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            w!("SysTreeView32"),
                            PCWSTR::null(),
                            WS_CHILD
                                | WS_VISIBLE
                                | WS_TABSTOP
                                | WS_VSCROLL
                                | WINDOW_STYLE(
                                    TVS_HASBUTTONS
                                        | TVS_HASLINES
                                        | TVS_LINESATROOT
                                        | TVS_SHOWSELALWAYS,
                                ),
                            10,
                            10,
                            580,
                            214,
                            hwnd,
                            HMENU(ID_LIST as isize),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let flat_list = unsafe {
                        let list = CreateWindowExW(
                            Default::default(),
                            w!("LISTBOX"),
                            PCWSTR::null(),
                            WS_CHILD | WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(LBS_NOTIFY as u32),
                            10,
                            10,
                            580,
                            214,
                            hwnd,
                            HMENU(ID_FLAT_LIST as isize),
                            HINSTANCE(0),
                            None,
                        );
                        ShowWindow(list, SW_HIDE);
                        list
                    };
                    let mut tree_values = Vec::new();
                    let mut first_group = HTREEITEM(0);
                    let mut initial_selection = HTREEITEM(0);
                    for group in groups {
                        if group.hidden_in_tree {
                            continue;
                        }
                        let parent_item = insert_tree_item(tree, HTREEITEM(0), &group.label, -1);
                        if parent_item.0 == 0 {
                            continue;
                        }
                        if first_group.0 == 0 {
                            first_group = parent_item;
                        }
                        for item in group.items {
                            let value_index = tree_values.len();
                            let item_value = item.value;
                            tree_values.push(item_value.clone());
                            let child_item = insert_tree_item(
                                tree,
                                parent_item,
                                &item.label,
                                value_index as isize,
                            );
                            if init.initial_tree_value.as_deref() == Some(item_value.as_str()) {
                                initial_selection = child_item;
                                crate::send_message_w_safe(
                                    tree,
                                    TVM_EXPAND,
                                    WPARAM(TVE_EXPAND.0 as usize),
                                    LPARAM(parent_item.0),
                                );
                            }
                        }
                    }
                    let target = if initial_selection.0 != 0 {
                        initial_selection
                    } else {
                        first_group
                    };
                    if target.0 != 0 {
                        crate::send_message_w_safe(
                            tree,
                            TVM_SELECTITEM,
                            WPARAM(TVGN_CARET as usize),
                            LPARAM(target.0),
                        );
                        crate::send_message_w_safe(
                            tree,
                            TVM_ENSUREVISIBLE,
                            WPARAM(0),
                            LPARAM(target.0),
                        );
                    }
                    unsafe {
                        SetFocus(tree);
                    }
                    (ControlKind::Tree(tree), tree_values, Some(flat_list))
                }
            };

            let mut filter_label_hwnd = None;
            let filter_edit = init.filter_label.as_ref().map(|label| unsafe {
                let label_hwnd = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    10,
                    230,
                    120,
                    18,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                filter_label_hwnd = Some(label_hwnd);
                CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    10,
                    248,
                    580,
                    24,
                    hwnd,
                    HMENU(ID_FILTER_EDIT as isize),
                    HINSTANCE(0),
                    None,
                )
            });

            let secondary_button = init.secondary_action_label.as_ref().map(|label| unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(label).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    10,
                    282,
                    240,
                    28,
                    hwnd,
                    HMENU(ID_SECONDARY as isize),
                    HINSTANCE(0),
                    None,
                )
            });

            let ok = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(init.language, "options.ok")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    390,
                    282,
                    90,
                    28,
                    hwnd,
                    HMENU(ID_OK as isize),
                    HINSTANCE(0),
                    None,
                )
            };

            let cancel = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(init.language, "options.cancel")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    490,
                    282,
                    90,
                    28,
                    hwnd,
                    HMENU(ID_CANCEL as isize),
                    HINSTANCE(0),
                    None,
                )
            };

            if hfont.0 != 0 {
                let control_hwnd = match control {
                    ControlKind::List(list) => list,
                    ControlKind::Tree(tree) => tree,
                };
                unsafe {
                    SendMessageW(
                        control_hwnd,
                        WM_SETFONT,
                        WPARAM(hfont.0 as usize),
                        LPARAM(1),
                    );
                    if let Some(button) = secondary_button {
                        SendMessageW(button, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                    if let Some(label) = filter_label_hwnd {
                        SendMessageW(label, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                    if let Some(edit) = filter_edit {
                        SendMessageW(edit, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                    if let Some(list) = flat_list {
                        SendMessageW(list, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                    SendMessageW(ok, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    SendMessageW(cancel, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                }
            }

            let state = Box::new(InterpreterSelectState {
                control,
                original_mode: init.mode,
                language: init.language,
                initial_list_value: init.initial_list_value,
                secondary_button,
                filter_edit,
                flat_list,
                flat_list_values: Vec::new(),
                tree_values,
                context_actions: init.context_actions,
                result: init.result.clone(),
            });
            crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
            LRESULT(0)
        }
        WM_ACTIVATE => {
            if wparam.0 & 0xFFFF != 0 {
                crate::log_if_err!(unsafe {
                    PostMessageW(hwnd, WM_RESTORE_LIST_FOCUS, WPARAM(0), LPARAM(0))
                });
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_RESTORE_LIST_FOCUS => {
            let parent = crate::get_parent_safe(hwnd);
            if crate::media_action_progress_is_active(parent) {
                crate::log_debug(&format!(
                    "interpreter_select restore focus suppressed for active progress modal hwnd={:?} parent={:?}",
                    hwnd, parent
                ));
            } else {
                restore_interpreter_select_focus(hwnd);
            }
            LRESULT(0)
        }
        WM_CONTEXTMENU => {
            let target = HWND(wparam.0 as isize);
            let handled = with_interpreter_state(hwnd, |state| {
                if state.context_actions.is_empty() {
                    return false;
                }
                let value = match state.control {
                    ControlKind::List(list) => {
                        if target.0 != 0 && target != list && target != hwnd {
                            return false;
                        }
                        let Some(value) = selected_list_value(list) else {
                            return false;
                        };
                        value
                    }
                    ControlKind::Tree(tree) => {
                        if let Some(flat_list) = state.flat_list
                            && unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(flat_list) }
                                .as_bool()
                        {
                            return false;
                        }
                        if target.0 != 0 && target != tree && target != hwnd {
                            return false;
                        }
                        let Some(value) = selected_tree_value(tree, &state.tree_values) else {
                            return false;
                        };
                        value
                    }
                };
                let menu = match unsafe { CreatePopupMenu() } {
                    Ok(menu) => menu,
                    Err(err) => {
                        crate::log_debug(&format!(
                            "Failed to create interpreter selection context menu: {}",
                            err
                        ));
                        return false;
                    }
                };
                let applicable_actions = match append_context_actions_to_menu(
                    menu,
                    &state.context_actions,
                    &value,
                ) {
                    Ok(actions) => actions,
                    Err(err) => {
                        crate::log_debug(&format!(
                            "Failed to append interpreter selection context menu item: {}",
                            err
                        ));
                        crate::log_if_err!(unsafe { DestroyMenu(menu) });
                        return false;
                    }
                };
                if applicable_actions.is_empty() {
                    crate::log_if_err!(unsafe { DestroyMenu(menu) });
                    return false;
                }
                let point = if lparam.0 == -1 {
                    let mut pt = POINT::default();
                    if let Err(err) = unsafe { GetCursorPos(&mut pt) } {
                        crate::log_debug(&format!(
                            "Failed to query cursor position for interpreter selection context menu: {}",
                            err
                        ));
                        crate::log_if_err!(unsafe { DestroyMenu(menu) });
                        return false;
                    }
                    pt
                } else {
                    POINT {
                        x: (lparam.0 as u32 & 0xFFFF) as i16 as i32,
                        y: ((lparam.0 as u32 >> 16) & 0xFFFF) as i16 as i32,
                    }
                };
                let command = unsafe {
                    TrackPopupMenu(
                        menu,
                        TPM_RETURNCMD | TPM_NONOTIFY,
                        point.x,
                        point.y,
                        0,
                        hwnd,
                        None,
                    )
                };
                crate::log_if_err!(unsafe { DestroyMenu(menu) });
                if command.0 == 0 {
                    return true;
                }
                let Some(action_index) = usize::try_from(command.0.saturating_sub(1)).ok() else {
                    return true;
                };
                let Some(action) = applicable_actions.get(action_index) else {
                    return true;
                };
                (action.handler)(value);
                true
            })
            .unwrap_or(false);
            if handled {
                return LRESULT(0);
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_COMMAND => {
            let id = wparam.0;
            match id {
                command
                    if command & 0xffff == ID_FILTER_EDIT
                        && ((command >> 16) & 0xffff) as u32 == EN_CHANGE =>
                {
                    refresh_filtered_control(hwnd);
                    LRESULT(0)
                }
                ID_OK => {
                    let activated_filtered_group = with_interpreter_state(hwnd, |state| {
                        if let ControlKind::Tree(tree) = &state.control
                            && is_tree_filter_active(state)
                        {
                            return activate_filtered_tree_group_selection(hwnd, state, *tree);
                        }
                        false
                    })
                    .unwrap_or(false);
                    if activated_filtered_group {
                        return LRESULT(0);
                    }
                    with_interpreter_state(hwnd, |state| match &state.control {
                        ControlKind::List(list) => {
                            if let Some(value) =
                                read_listbox_selected_text(*list, "ok_command.read_selected_text")
                            {
                                *state.result.lock().unwrap_or_else(|e| e.into_inner()) =
                                    Some(InterpreterSelectionResult::Item(value));
                            }
                        }
                        ControlKind::Tree(tree) => {
                            let caret = HTREEITEM(
                                crate::send_message_w_safe(
                                    *tree,
                                    TVM_GETNEXTITEM,
                                    WPARAM(TVGN_CARET as usize),
                                    LPARAM(0),
                                )
                                .0,
                            );
                            if caret.0 != 0 {
                                let mut item = TVITEMW {
                                    mask: TVIF_PARAM | TVIF_TEXT,
                                    hItem: caret,
                                    ..Default::default()
                                };
                                let mut text = vec![0u16; 512];
                                item.pszText = windows::core::PWSTR(text.as_mut_ptr());
                                item.cchTextMax = text.len() as i32;
                                if crate::send_message_w_safe(
                                    *tree,
                                    TVM_GETITEMW,
                                    WPARAM(0),
                                    LPARAM(&mut item as *mut _ as isize),
                                )
                                .0 != 0
                                {
                                    if item.lParam.0 >= 0 {
                                        let index = item.lParam.0 as usize;
                                        if let Some(value) = state.tree_values.get(index) {
                                            *state
                                                .result
                                                .lock()
                                                .unwrap_or_else(|e| e.into_inner()) = Some(
                                                InterpreterSelectionResult::Item(value.clone()),
                                            );
                                        }
                                    } else {
                                        crate::send_message_w_safe(
                                            *tree,
                                            TVM_EXPAND,
                                            WPARAM(TVE_EXPAND.0 as usize),
                                            LPARAM(caret.0),
                                        );
                                    }
                                }
                            }
                        }
                    });
                    if with_interpreter_state(hwnd, |state| {
                        state
                            .result
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .is_some()
                    })
                    .unwrap_or(false)
                    {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    }
                    LRESULT(0)
                }
                ID_SECONDARY => {
                    with_interpreter_state(hwnd, |state| {
                        *state.result.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(InterpreterSelectionResult::SecondaryAction);
                    });
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    LRESULT(0)
                }
                ID_CANCEL => {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    LRESULT(0)
                }
                _ => crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
            }
        }
        WM_CLOSE => {
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            LRESULT(0)
        }
        WM_NCDESTROY => {
            unregister_active_interpreter_select(hwnd);
            let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA)
                as *mut InterpreterSelectState;
            if !ptr.is_null() {
                let _unused_box = crate::box_from_raw_safe(ptr);
            }
            LRESULT(0)
        }
        _ => crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
    }
}

fn selected_list_value(list: HWND) -> Option<String> {
    read_listbox_selected_text(list, "selected_list_value")
}

fn selected_tree_value(tree: HWND, tree_values: &[String]) -> Option<String> {
    let caret = HTREEITEM(
        crate::send_message_w_safe(
            tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    if caret.0 == 0 {
        return None;
    }
    let mut item = TVITEMW {
        mask: TVIF_PARAM,
        hItem: caret,
        ..Default::default()
    };
    if crate::send_message_w_safe(
        tree,
        TVM_GETITEMW,
        WPARAM(0),
        LPARAM(&mut item as *mut _ as isize),
    )
    .0 == 0
    {
        return None;
    }
    if item.lParam.0 < 0 {
        return None;
    }
    tree_values.get(item.lParam.0 as usize).cloned()
}

fn refresh_filtered_control(hwnd: HWND) {
    with_interpreter_state(hwnd, |state| {
        let filter_text = state
            .filter_edit
            .map(read_window_text)
            .unwrap_or_default()
            .trim()
            .to_lowercase();

        let result_count = match (&state.control, &state.original_mode) {
            (ControlKind::List(list), InterpreterDialogInitMode::List(items)) => {
                let preferred =
                    selected_list_value(*list).or_else(|| state.initial_list_value.clone());
                repopulate_list(*list, items, preferred.as_deref(), &filter_text)
            }
            (ControlKind::Tree(tree), InterpreterDialogInitMode::Tree(groups)) => {
                repopulate_group_filter_list(
                    *tree,
                    state.flat_list,
                    &mut state.flat_list_values,
                    groups,
                    &filter_text,
                )
            }
            _ => 0,
        };

        if state.filter_edit.is_some() && !filter_text.is_empty() {
            let message = crate::i18n::tr_f(
                state.language,
                "interpreter_select.results_count",
                &[("count", &result_count.to_string())],
            );
            crate::screen_reader_speak(&message);
        }
    });
}

fn repopulate_list(
    list: HWND,
    items: &[String],
    preferred: Option<&str>,
    filter_text: &str,
) -> usize {
    set_control_redraw(list, false);
    crate::send_message_w_safe(list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    let mut selected_index = None;
    let mut inserted_count = 0usize;
    for item in items {
        if !matches_filter(item, filter_text) {
            continue;
        }
        crate::send_message_w_safe(
            list,
            LB_ADDSTRING,
            WPARAM(0),
            LPARAM(to_wide(item).as_ptr() as isize),
        );
        if preferred == Some(item.as_str()) {
            selected_index = Some(inserted_count);
        }
        inserted_count += 1;
    }
    if inserted_count == 0 {
        refresh_control_after_bulk_update(list);
        return 0;
    }
    crate::send_message_w_safe(
        list,
        LB_SETCURSEL,
        WPARAM(selected_index.unwrap_or(0)),
        LPARAM(0),
    );
    refresh_control_after_bulk_update(list);
    inserted_count
}

fn repopulate_group_filter_list(
    tree: HWND,
    flat_list: Option<HWND>,
    flat_list_values: &mut Vec<FlatListValue>,
    groups: &[GroupedSelectGroup],
    filter_text: &str,
) -> usize {
    let Some(flat_list) = flat_list else {
        return 0;
    };

    if filter_text.is_empty() {
        unsafe {
            ShowWindow(flat_list, SW_HIDE);
            ShowWindow(tree, SW_SHOW);
        }
        return 0;
    }

    unsafe {
        ShowWindow(tree, SW_HIDE);
        ShowWindow(flat_list, SW_SHOW);
    }

    set_control_redraw(flat_list, false);
    crate::send_message_w_safe(flat_list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    flat_list_values.clear();

    let mut inserted_count = 0usize;
    for group in groups {
        if !group.hidden_in_tree && matches_filter(&group.label, filter_text) {
            crate::send_message_w_safe(
                flat_list,
                LB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&group.label).as_ptr() as isize),
            );
            flat_list_values.push(FlatListValue::Group(group.label.clone()));
            inserted_count += 1;
        }
        for item in &group.items {
            if !matches_filter(&item.label, filter_text) {
                continue;
            }
            let display = if group.hidden_in_tree {
                format!("{} [{}]", item.label, group.label)
            } else {
                item.label.clone()
            };
            crate::send_message_w_safe(
                flat_list,
                LB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&display).as_ptr() as isize),
            );
            flat_list_values.push(FlatListValue::Item(item.value.clone()));
            inserted_count += 1;
        }
    }
    if inserted_count > 0 {
        crate::send_message_w_safe(flat_list, LB_SETCURSEL, WPARAM(0), LPARAM(0));
    }
    refresh_control_after_bulk_update(flat_list);
    inserted_count
}

fn is_tree_filter_active(state: &InterpreterSelectState) -> bool {
    state
        .filter_edit
        .map(read_window_text)
        .is_some_and(|text| !text.trim().is_empty())
}

fn activate_filtered_tree_group_selection(
    hwnd: HWND,
    state: &mut InterpreterSelectState,
    tree: HWND,
) -> bool {
    let Some(flat_list) = state.flat_list else {
        return false;
    };
    let selected_value = selected_flat_group_value(flat_list, &state.flat_list_values);
    let Some(selected_value) = selected_value else {
        return false;
    };
    match selected_value {
        FlatListValue::Group(selected_group) => {
            let Some(group_item) = find_tree_group_by_label(tree, &selected_group) else {
                return false;
            };

            if let Some(edit) = state.filter_edit {
                crate::log_if_err!(unsafe { SetWindowTextW(edit, PCWSTR::null()) });
            }
            unsafe {
                ShowWindow(flat_list, SW_HIDE);
                ShowWindow(tree, SW_SHOW);
                SetFocus(tree);
            }
            crate::send_message_w_safe(
                tree,
                TVM_SELECTITEM,
                WPARAM(TVGN_CARET as usize),
                LPARAM(group_item.0),
            );
            crate::send_message_w_safe(
                tree,
                TVM_EXPAND,
                WPARAM(TVE_EXPAND.0 as usize),
                LPARAM(group_item.0),
            );
            crate::send_message_w_safe(tree, TVM_ENSUREVISIBLE, WPARAM(0), LPARAM(group_item.0));
            true
        }
        FlatListValue::Item(selected_item) => {
            *state.result.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(InterpreterSelectionResult::Item(selected_item));
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            true
        }
    }
}

fn selected_flat_group_value(
    flat_list: HWND,
    flat_list_values: &[FlatListValue],
) -> Option<FlatListValue> {
    let sel = crate::send_message_w_safe(flat_list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel < 0 {
        return None;
    }
    flat_list_values.get(sel as usize).cloned()
}

fn find_tree_group_by_label(tree: HWND, target_label: &str) -> Option<HTREEITEM> {
    let mut current = HTREEITEM(
        crate::send_message_w_safe(
            tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    if current.0 != 0
        && let Some(parent) = parent_tree_item(tree, current)
    {
        current = parent;
    }
    while let Some(label) = tree_item_label(tree, current) {
        if label == target_label {
            return Some(current);
        }
        current = HTREEITEM(
            crate::send_message_w_safe(
                tree,
                TVM_GETNEXTITEM,
                WPARAM(windows::Win32::UI::Controls::TVGN_NEXT as usize),
                LPARAM(current.0),
            )
            .0,
        );
        if current.0 == 0 {
            break;
        }
    }

    let mut current = HTREEITEM(
        crate::send_message_w_safe(
            tree,
            TVM_GETNEXTITEM,
            WPARAM(windows::Win32::UI::Controls::TVGN_ROOT as usize),
            LPARAM(0),
        )
        .0,
    );
    while current.0 != 0 {
        if tree_item_label(tree, current).as_deref() == Some(target_label) {
            return Some(current);
        }
        current = HTREEITEM(
            crate::send_message_w_safe(
                tree,
                TVM_GETNEXTITEM,
                WPARAM(windows::Win32::UI::Controls::TVGN_NEXT as usize),
                LPARAM(current.0),
            )
            .0,
        );
    }
    None
}

fn parent_tree_item(tree: HWND, item: HTREEITEM) -> Option<HTREEITEM> {
    let parent = HTREEITEM(
        crate::send_message_w_safe(
            tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_PARENT as usize),
            LPARAM(item.0),
        )
        .0,
    );
    if parent.0 == 0 { None } else { Some(parent) }
}

fn tree_item_label(tree: HWND, item: HTREEITEM) -> Option<String> {
    if item.0 == 0 {
        return None;
    }
    let mut entry = TVITEMW {
        mask: TVIF_TEXT,
        hItem: item,
        ..Default::default()
    };
    let mut text = vec![0u16; 512];
    entry.pszText = windows::core::PWSTR(text.as_mut_ptr());
    entry.cchTextMax = text.len() as i32;
    if crate::send_message_w_safe(
        tree,
        TVM_GETITEMW,
        WPARAM(0),
        LPARAM(&mut entry as *mut _ as isize),
    )
    .0 == 0
    {
        return None;
    }
    let len = text
        .iter()
        .position(|&value| value == 0)
        .unwrap_or(text.len());
    Some(String::from_utf16_lossy(&text[..len]))
}

fn set_control_redraw(hwnd: HWND, enabled: bool) {
    unsafe {
        SendMessageW(hwnd, WM_SETREDRAW, WPARAM(usize::from(enabled)), LPARAM(0));
    }
}

fn refresh_control_after_bulk_update(hwnd: HWND) {
    set_control_redraw(hwnd, true);
    unsafe {
        if !InvalidateRect(hwnd, None, BOOL(1)).as_bool() {
            crate::log_debug("InvalidateRect failed after interpreter selection bulk update");
        }
    }
}

fn matches_filter(text: &str, filter_text: &str) -> bool {
    if filter_text.is_empty() {
        return true;
    }
    text.trim_start().to_lowercase().starts_with(filter_text)
}

fn read_window_text(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len as usize + 1];
    let written = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..written as usize])
}

fn log_interpreter_control_state(tag: &str, hwnd: HWND) {
    let mut class_buf = [0u16; 128];
    let class_len = if crate::is_window_handle_valid(hwnd) {
        crate::get_class_name_w_safe(hwnd, &mut class_buf)
    } else {
        0
    };
    let class_name = if class_len > 0 {
        String::from_utf16_lossy(&class_buf[..class_len as usize])
    } else {
        String::new()
    };
    crate::log_debug(&format!(
        "interpreter_select control [{}] hwnd={:?} valid={} class='{}' focus={:?}",
        tag,
        hwnd,
        crate::is_window_handle_valid(hwnd),
        class_name,
        crate::get_focus_safe()
    ));
}

fn read_listbox_selected_text(list: HWND, log_tag: &str) -> Option<String> {
    log_interpreter_control_state(log_tag, list);
    if !crate::is_window_handle_valid(list) {
        crate::log_debug(&format!(
            "interpreter_select {} aborted: invalid list hwnd={:?}",
            log_tag, list
        ));
        return None;
    }
    let sel = crate::send_message_w_safe(list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    crate::log_debug(&format!(
        "interpreter_select {} selection hwnd={:?} sel={}",
        log_tag, list, sel
    ));
    if sel < 0 {
        return None;
    }
    let len = crate::send_message_w_safe(list, LB_GETTEXTLEN, WPARAM(sel as usize), LPARAM(0)).0;
    crate::log_debug(&format!(
        "interpreter_select {} textlen hwnd={:?} sel={} len={}",
        log_tag, list, sel, len
    ));
    if len < 0 {
        return None;
    }
    const MAX_LISTBOX_TEXT_LEN: isize = 32_768;
    if len > MAX_LISTBOX_TEXT_LEN {
        crate::log_debug(&format!(
            "interpreter_select {} rejected suspicious textlen hwnd={:?} sel={} len={}",
            log_tag, list, sel, len
        ));
        return None;
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    crate::send_message_w_safe(
        list,
        LB_GETTEXT,
        WPARAM(sel as usize),
        LPARAM(buf.as_mut_ptr() as isize),
    );
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}

fn insert_tree_item(tree: HWND, parent: HTREEITEM, label: &str, value_index: isize) -> HTREEITEM {
    let text = to_wide(label);
    let mut insert = TVINSERTSTRUCTW {
        hParent: parent,
        hInsertAfter: HTREEITEM(0xffff0002u32 as isize),
        Anonymous: TVINSERTSTRUCTW_0 {
            item: TVITEMW {
                mask: TVIF_TEXT | TVIF_PARAM,
                pszText: windows::core::PWSTR(text.as_ptr() as *mut _),
                lParam: LPARAM(value_index),
                ..Default::default()
            },
        },
    };
    HTREEITEM(
        crate::send_message_w_safe(
            tree,
            TVM_INSERTITEMW,
            WPARAM(0),
            LPARAM(&mut insert as *mut _ as isize),
        )
        .0,
    )
}

fn with_interpreter_state<F, R>(hwnd: HWND, f: F) -> Option<R>
where
    F: FnOnce(&mut InterpreterSelectState) -> R,
{
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut InterpreterSelectState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

pub fn restore_interpreter_select_focus(hwnd: HWND) -> bool {
    if !crate::is_window_handle_valid(hwnd) {
        crate::log_debug(&format!(
            "interpreter_select restore focus aborted: invalid hwnd={:?}",
            hwnd
        ));
        return false;
    }
    let mut class_buf = [0u16; 128];
    let class_len = crate::get_class_name_w_safe(hwnd, &mut class_buf);
    let class_name = if class_len > 0 {
        String::from_utf16_lossy(&class_buf[..class_len as usize])
    } else {
        String::new()
    };
    if class_len <= 0 || class_name != INTERPRETER_SELECT_CLASS_NAME {
        crate::log_debug(&format!(
            "interpreter_select restore focus aborted: hwnd={:?} class='{}'",
            hwnd, class_name
        ));
        return false;
    }
    crate::log_debug(&format!(
        "interpreter_select restore focus start hwnd={:?} focus_before={:?}",
        hwnd,
        crate::get_focus_safe()
    ));
    with_interpreter_state(hwnd, |state| {
        let target = match state.control {
            ControlKind::List(list) => list,
            ControlKind::Tree(tree) => {
                if let Some(flat_list) = state.flat_list
                    && unsafe {
                        windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(flat_list)
                    }
                    .as_bool()
                {
                    flat_list
                } else {
                    tree
                }
            }
        };
        crate::log_debug(&format!(
            "interpreter_select restore focus state hwnd={:?} target={:?} filter_edit={:?} flat_list={:?} focus_before_target={:?}",
            hwnd,
            target,
            state.filter_edit,
            state.flat_list,
            crate::get_focus_safe()
        ));
        crate::log_debug(&format!(
            "interpreter_select restore focus hwnd={:?} target={:?}",
            hwnd, target
        ));
        log_interpreter_control_state("restore_focus.target_before", target);
        unsafe {
            SetFocus(target);
        }
        if matches!(state.control, ControlKind::List(_))
            || state.flat_list.is_some_and(|flat_list| flat_list == target)
        {
            let sel = crate::send_message_w_safe(target, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
            crate::log_debug(&format!(
                "interpreter_select restore focus listbox state hwnd={:?} target={:?} sel={}",
                hwnd, target, sel
            ));
            if sel >= 0 {
                crate::send_message_w_safe(target, LB_SETCURSEL, WPARAM(sel as usize), LPARAM(0));
                crate::send_message_w_safe(
                    target,
                    LB_SETCARETINDEX,
                    WPARAM(sel as usize),
                    LPARAM(0),
                );
                crate::log_debug(&format!(
                    "interpreter_select restore focus listbox reseated hwnd={:?} target={:?} sel={}",
                    hwnd, target, sel
                ));
            }
        }
        crate::log_debug(&format!(
            "interpreter_select restore focus after SetFocus hwnd={:?} target={:?} focus_after={:?}",
            hwnd,
            target,
            crate::get_focus_safe()
        ));
        true
    })
    .unwrap_or(false)
}

fn is_list_navigation_key(key: u32) -> bool {
    key == VK_UP.0 as u32
        || key == VK_DOWN.0 as u32
        || key == VK_PRIOR.0 as u32
        || key == VK_NEXT.0 as u32
        || key == VK_HOME.0 as u32
        || key == VK_END.0 as u32
}

fn is_tree_navigation_key(key: u32) -> bool {
    is_list_navigation_key(key) || key == VK_LEFT.0 as u32 || key == VK_RIGHT.0 as u32
}
