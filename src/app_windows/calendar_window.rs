use crate::accessibility::{screen_reader_speak, to_wide};
use crate::i18n;
use crate::settings::{Language, load_settings, settings_dir};
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, Timelike, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, DEFAULT_GUI_FONT, HBRUSH, HFONT};
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::GMEM_MOVEABLE;
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, VK_ESCAPE, VK_LEFT, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL,
    CB_SETCURSEL, CBS_DROPDOWNLIST, CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    DestroyWindow, DispatchMessageW, ES_AUTOHSCROLL, EVENT_OBJECT_FOCUS, GWLP_USERDATA,
    GetClientRect, GetDlgItem, GetForegroundWindow, GetMessageW, GetWindowTextLengthW,
    GetWindowTextW, HMENU, IDC_ARROW, IDYES, IsChild, IsDialogMessageW, IsWindow, LB_ADDSTRING,
    LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL, LBN_DBLCLK, LBN_SELCHANGE, LBS_HASSTRINGS,
    LBS_NOTIFY, LoadCursorW, MB_ICONERROR, MB_ICONQUESTION, MB_ICONWARNING, MB_OK, MB_YESNO, MSG,
    MessageBoxW, MoveWindow, OBJID_CLIENT, RegisterClassW, SW_HIDE, SW_SHOW, SetForegroundWindow,
    SetTimer, SetWindowTextW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_SETFOCUS, WM_SETFONT,
    WM_SIZE, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT,
    WS_EX_DLGMODALFRAME, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, w};

const CLASS_NAME: &str = "SonarpadCalendarWindow";
const REMINDER_CLASS_NAME: &str = "SonarpadCalendarReminderWindow";
const ALERT_CLASS_NAME: &str = "SonarpadCalendarAlertWindow";
pub(crate) const REMINDER_TIMER_ID: usize = 0xCA11;
const REMINDER_CHECK_INTERVAL_MS: u32 = 30_000;
const CREATE_NO_WINDOW_FLAG: u32 = 0x0800_0000;
const MISSED_REMINDER_GRACE_HOURS: i64 = 24;

const ID_LABEL_LIST: usize = 3601;
const ID_LIST: usize = 3602;
const ID_BUTTON_TODAY: usize = 3603;
const ID_BUTTON_BACK: usize = 3604;
const ID_BUTTON_ADD: usize = 3605;
const ID_BUTTON_DELETE: usize = 3606;
const ID_BUTTON_COPY: usize = 3608;
const ID_BUTTON_LISTEN: usize = 3609;
const ID_BUTTON_CLOSE: usize = 3610;

const ID_REMINDER_LABEL_DATE: usize = 3651;
const ID_REMINDER_LABEL_TEXT: usize = 3652;
const ID_REMINDER_EDIT_TEXT: usize = 3653;
const ID_REMINDER_CHECK_TIME: usize = 3654;
const ID_REMINDER_LABEL_HOUR: usize = 3655;
const ID_REMINDER_COMBO_HOUR: usize = 3656;
const ID_REMINDER_LABEL_MINUTE: usize = 3657;
const ID_REMINDER_COMBO_MINUTE: usize = 3658;
const ID_REMINDER_LABEL_ALERT: usize = 3659;
const ID_REMINDER_COMBO_ALERT: usize = 3660;
const ID_REMINDER_INFO: usize = 3661;
const ID_REMINDER_SAVE: usize = 3662;
const ID_REMINDER_CANCEL: usize = 3663;

const ID_ALERT_LABEL: usize = 3701;
const ID_ALERT_LIST: usize = 3702;
const ID_ALERT_SNOOZE_COMBO: usize = 3703;
const ID_ALERT_COMPLETE: usize = 3704;
const ID_ALERT_SNOOZE: usize = 3705;
const ID_ALERT_CLOSE: usize = 3706;

const DAYS_INITIAL_RADIUS: i64 = 183;
const DAYS_EXTEND_BY: i64 = 365;
const DAYS_EXTEND_THRESHOLD: usize = 12;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CalendarView {
    Days,
    DayDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CalendarReminder {
    id: String,
    date: String,
    text: String,
    #[serde(default)]
    has_time: bool,
    #[serde(default)]
    hour: u32,
    #[serde(default)]
    minute: u32,
    #[serde(default)]
    alert_minutes: u32,
    #[serde(default)]
    completed: bool,
    #[serde(default)]
    alerted: bool,
    #[serde(default)]
    snoozed_until: Option<String>,
}

#[derive(Deserialize)]
struct CalendarData {
    saints: HashMap<String, HashMap<String, String>>,
    quotes: HashMap<String, Vec<String>>,
}

struct CalendarWindowState {
    parent: HWND,
    language: Language,
    list_label: HWND,
    list: HWND,
    button_today: HWND,
    button_back: HWND,
    button_add: HWND,
    button_delete: HWND,
    button_copy: HWND,
    button_listen: HWND,
    button_close: HWND,
    view: CalendarView,
    dates: Vec<NaiveDate>,
    detail_date: Option<NaiveDate>,
    detail_reminder_ids: Vec<Option<String>>,
    reminders: Vec<CalendarReminder>,
    rebuilding: bool,
}

#[derive(Clone)]
struct NewReminder {
    text: String,
    has_time: bool,
    hour: u32,
    minute: u32,
    alert_minutes: u32,
}

struct ReminderAlertWindowState {
    parent: HWND,
    language: Language,
    list: HWND,
    combo_snooze: HWND,
    reminder_ids: Vec<String>,
    previous_focus: HWND,
}

struct ReminderDialogData {
    language: Language,
    date: NaiveDate,
    result: Option<NewReminder>,
    edit_text: HWND,
    check_time: HWND,
    combo_hour: HWND,
    combo_minute: HWND,
    combo_alert: HWND,
}

pub fn open(parent: HWND) {
    unsafe {
        let language = load_settings().language;
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(CLASS_NAME);
        let window_class = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(calendar_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&window_class);

        let title = to_wide(&tr(language, "calendar.title"));
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1000,
            700,
            parent,
            HMENU(0),
            hinstance,
            Some(parent.0 as *const std::ffi::c_void),
        );
        if hwnd.0 == 0 {
            return;
        }
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);

        let mut message = MSG::default();
        while IsWindow(hwnd).as_bool() && GetMessageW(&mut message, HWND(0), 0, 0).as_bool() {
            if handle_reminder_alert_message(&message) {
                continue;
            }
            if handle_calendar_tab(hwnd, &message)
                || handle_calendar_enter(hwnd, &message)
                || handle_calendar_escape(hwnd, &message)
            {
                continue;
            }
            if crate::handle_focused_edit_shortcut(&message) {
                continue;
            }
            if !IsDialogMessageW(hwnd, &message).as_bool() {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

unsafe extern "system" fn calendar_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "calendar_window_wndproc",
            || DefWindowProcW(hwnd, message, wparam, lparam),
            || calendar_wndproc_inner(hwnd, message, wparam, lparam),
        )
    }
}

fn calendar_wndproc_inner(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_CREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                let parent = HWND((*create).lpCreateParams as isize);
                create_calendar_controls(hwnd, parent);
                LRESULT(0)
            }
            WM_SIZE => {
                layout_calendar(hwnd);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                if let Some(list) = with_calendar_state(hwnd, |state| state.list) {
                    crate::set_focus_safe(list);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let notification = (wparam.0 >> 16) as u16;
                match id {
                    ID_LIST if notification == LBN_SELCHANGE as u16 => {
                        on_calendar_selection_changed(hwnd);
                        LRESULT(0)
                    }
                    ID_LIST if notification == LBN_DBLCLK as u16 => {
                        if current_view(hwnd) == Some(CalendarView::Days) {
                            open_selected_day(hwnd);
                        }
                        LRESULT(0)
                    }
                    ID_BUTTON_TODAY => {
                        select_today(hwnd);
                        LRESULT(0)
                    }
                    ID_BUTTON_BACK => {
                        show_days_view(hwnd);
                        LRESULT(0)
                    }
                    ID_BUTTON_ADD => {
                        add_reminder(hwnd);
                        LRESULT(0)
                    }
                    ID_BUTTON_DELETE => {
                        delete_selected_reminder(hwnd);
                        LRESULT(0)
                    }
                    ID_BUTTON_COPY => {
                        copy_selected_day(hwnd);
                        LRESULT(0)
                    }
                    ID_BUTTON_LISTEN => {
                        listen_selected_day(hwnd);
                        LRESULT(0)
                    }
                    ID_BUTTON_CLOSE => {
                        crate::log_if_err!(DestroyWindow(hwnd));
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
                }
            }
            WM_CLOSE => {
                if current_view(hwnd) == Some(CalendarView::DayDetails) {
                    show_days_view(hwnd);
                } else {
                    crate::log_if_err!(DestroyWindow(hwnd));
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                let parent = with_calendar_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
                if parent.0 != 0 {
                    crate::log_if_err!(crate::post_message_w_safe(
                        parent,
                        crate::WM_FOCUS_EDITOR,
                        WPARAM(0),
                        LPARAM(0)
                    ));
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let pointer = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA)
                    as *mut CalendarWindowState;
                if !pointer.is_null() {
                    let _state = Box::from_raw(pointer);
                    crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, 0);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

fn create_calendar_controls(hwnd: HWND, parent: HWND) {
    let language = load_settings().language;
    let list_label = create_static(hwnd, &tr(language, "calendar.instructions"), ID_LABEL_LIST);
    let list = create_listbox(hwnd, ID_LIST);
    let button_today = create_button(
        hwnd,
        &tr(language, "calendar.back_to_today"),
        ID_BUTTON_TODAY,
        false,
    );
    let button_back = create_button(
        hwnd,
        &tr(language, "calendar.back_to_days"),
        ID_BUTTON_BACK,
        false,
    );
    let button_add = create_button(
        hwnd,
        &tr(language, "calendar.add_reminder"),
        ID_BUTTON_ADD,
        false,
    );
    let button_delete = create_button(
        hwnd,
        &tr(language, "calendar.delete_reminder"),
        ID_BUTTON_DELETE,
        false,
    );
    let button_copy = create_button(
        hwnd,
        &tr(language, "calendar.copy_day"),
        ID_BUTTON_COPY,
        false,
    );
    let button_listen = create_button(
        hwnd,
        &tr(language, "calendar.listen_all"),
        ID_BUTTON_LISTEN,
        false,
    );
    let button_close = create_button(
        hwnd,
        &tr(language, "calendar.close"),
        ID_BUTTON_CLOSE,
        false,
    );

    let font = HFONT(crate::get_stock_object_safe(DEFAULT_GUI_FONT).0);
    for control in [
        list_label,
        list,
        button_today,
        button_back,
        button_add,
        button_delete,
        button_copy,
        button_listen,
        button_close,
    ] {
        crate::send_message_w_safe(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    }

    let today = Local::now().date_naive();
    let start = today - Duration::days(DAYS_INITIAL_RADIUS);
    let end = today + Duration::days(DAYS_INITIAL_RADIUS);
    let dates = dates_between(start, end);
    let state = Box::new(CalendarWindowState {
        parent,
        language,
        list_label,
        list,
        button_today,
        button_back,
        button_add,
        button_delete,
        button_copy,
        button_listen,
        button_close,
        view: CalendarView::Days,
        dates,
        detail_date: None,
        detail_reminder_ids: Vec::new(),
        reminders: load_reminders(),
        rebuilding: false,
    });
    crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
    refresh_calendar_controls(hwnd);
    populate_days(hwnd, today);
    layout_calendar(hwnd);
    crate::set_focus_safe(list);
    notify_focus(list);
}

fn handle_calendar_tab(hwnd: HWND, message: &MSG) -> bool {
    if message.message != WM_KEYDOWN || message.wParam.0 as u32 != VK_TAB.0 as u32 {
        return false;
    }
    let Some((view, list, today, back, add, delete, copy, listen, close)) =
        with_calendar_state(hwnd, |state| {
            (
                state.view,
                state.list,
                state.button_today,
                state.button_back,
                state.button_add,
                state.button_delete,
                state.button_copy,
                state.button_listen,
                state.button_close,
            )
        })
    else {
        return false;
    };

    let mut controls = Vec::new();
    controls.push(list);
    match view {
        CalendarView::Days => controls.push(today),
        CalendarView::DayDetails => {
            controls.push(add);
            if selected_detail_reminder_id(hwnd).is_some() {
                controls.push(delete);
            }
            controls.push(copy);
            controls.push(listen);
            controls.push(back);
        }
    }
    controls.push(close);

    let current = crate::get_focus_safe();
    let backwards = (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
    let current_index = controls.iter().position(|control| *control == current);
    let next_index = match (current_index, backwards) {
        (Some(0), true) | (None, true) => controls.len().saturating_sub(1),
        (Some(index), true) => index.saturating_sub(1),
        (Some(index), false) if index + 1 < controls.len() => index + 1,
        _ => 0,
    };
    if let Some(target) = controls.get(next_index).copied() {
        crate::set_focus_safe(target);
        return true;
    }
    false
}

fn handle_calendar_enter(hwnd: HWND, message: &MSG) -> bool {
    if message.message != WM_KEYDOWN || message.wParam.0 as u32 != VK_RETURN.0 as u32 {
        return false;
    }
    let Some((view, list)) = with_calendar_state(hwnd, |state| (state.view, state.list)) else {
        return false;
    };
    if crate::get_focus_safe() != list {
        return false;
    }
    match view {
        CalendarView::Days => open_selected_day(hwnd),
        CalendarView::DayDetails => update_reminder_action_buttons(hwnd),
    }
    true
}

fn handle_calendar_escape(hwnd: HWND, message: &MSG) -> bool {
    if message.message != WM_KEYDOWN || message.wParam.0 as u32 != VK_ESCAPE.0 as u32 {
        return false;
    }
    if current_view(hwnd) == Some(CalendarView::DayDetails) {
        show_days_view(hwnd);
    } else {
        crate::log_if_err!(crate::post_message_w_safe(
            hwnd,
            WM_CLOSE,
            WPARAM(0),
            LPARAM(0)
        ));
    }
    true
}

fn current_view(hwnd: HWND) -> Option<CalendarView> {
    with_calendar_state(hwnd, |state| state.view)
}

fn on_calendar_selection_changed(hwnd: HWND) {
    match current_view(hwnd) {
        Some(CalendarView::Days) => ensure_day_range(hwnd),
        Some(CalendarView::DayDetails) => update_reminder_action_buttons(hwnd),
        None => {}
    }
}

fn populate_days(hwnd: HWND, selected_date: NaiveDate) {
    let Some((language, list, dates, reminders)) = with_calendar_state(hwnd, |state| {
        state.rebuilding = true;
        (
            state.language,
            state.list,
            state.dates.clone(),
            state.reminders.clone(),
        )
    }) else {
        return;
    };
    crate::send_message_w_safe(list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    let today = Local::now().date_naive();
    for date in &dates {
        let row = format_day_row(language, *date, today, &reminders);
        add_list_string(list, &row);
    }
    let selected_index = dates
        .iter()
        .position(|date| *date == selected_date)
        .unwrap_or(0);
    crate::send_message_w_safe(list, LB_SETCURSEL, WPARAM(selected_index), LPARAM(0));
    let _state_updated = with_calendar_state(hwnd, |state| state.rebuilding = false);
}

fn ensure_day_range(hwnd: HWND) {
    let Some((rebuilding, list, dates)) = with_calendar_state(hwnd, |state| {
        (state.rebuilding, state.list, state.dates.clone())
    }) else {
        return;
    };
    if rebuilding || dates.is_empty() {
        return;
    }
    let Some(index) = selected_list_index(list) else {
        return;
    };
    let Some(selected_date) = dates.get(index).copied() else {
        return;
    };

    let mut new_dates = dates;
    let mut changed = false;
    if index <= DAYS_EXTEND_THRESHOLD
        && let Some(first) = new_dates.first().copied()
    {
        let start = first - Duration::days(DAYS_EXTEND_BY);
        let end = first - Duration::days(1);
        let mut prefix = dates_between(start, end);
        prefix.extend(new_dates);
        new_dates = prefix;
        changed = true;
    } else if index + DAYS_EXTEND_THRESHOLD >= new_dates.len()
        && let Some(last) = new_dates.last().copied()
    {
        let start = last + Duration::days(1);
        let end = last + Duration::days(DAYS_EXTEND_BY);
        new_dates.extend(dates_between(start, end));
        changed = true;
    }
    if changed {
        let _state_updated = with_calendar_state(hwnd, |state| state.dates = new_dates);
        populate_days(hwnd, selected_date);
    }
}

fn select_today(hwnd: HWND) {
    let today = Local::now().date_naive();
    let needs_rebuild =
        with_calendar_state(hwnd, |state| !state.dates.contains(&today)).unwrap_or(false);
    if needs_rebuild {
        let start = today - Duration::days(DAYS_INITIAL_RADIUS);
        let end = today + Duration::days(DAYS_INITIAL_RADIUS);
        let _state_updated =
            with_calendar_state(hwnd, |state| state.dates = dates_between(start, end));
    }
    populate_days(hwnd, today);
    if let Some(list) = with_calendar_state(hwnd, |state| state.list) {
        crate::set_focus_safe(list);
        notify_focus(list);
    }
}

fn open_selected_day(hwnd: HWND) {
    let Some((list, dates)) = with_calendar_state(hwnd, |state| (state.list, state.dates.clone()))
    else {
        return;
    };
    let Some(index) = selected_list_index(list) else {
        return;
    };
    let Some(date) = dates.get(index).copied() else {
        return;
    };
    show_day_details(hwnd, date);
}

fn show_day_details(hwnd: HWND, date: NaiveDate) {
    let _state_updated = with_calendar_state(hwnd, |state| {
        state.view = CalendarView::DayDetails;
        state.detail_date = Some(date);
        state.reminders = load_reminders();
    });
    refresh_calendar_controls(hwnd);
    populate_day_details(hwnd);
    let language = with_calendar_state(hwnd, |state| state.language).unwrap_or_default();
    let title = format!(
        "{} - {}",
        tr(language, "calendar.title"),
        localized_date(language, date)
    );
    set_window_text(hwnd, &title);
    if let Some(list) = with_calendar_state(hwnd, |state| state.list) {
        crate::set_focus_safe(list);
        notify_focus(list);
    }
}

fn show_days_view(hwnd: HWND) {
    let selected_date = with_calendar_state(hwnd, |state| state.detail_date)
        .flatten()
        .unwrap_or_else(|| Local::now().date_naive());
    let _state_updated = with_calendar_state(hwnd, |state| {
        state.view = CalendarView::Days;
        state.detail_date = None;
        state.detail_reminder_ids.clear();
        state.reminders = load_reminders();
    });
    let language = with_calendar_state(hwnd, |state| state.language).unwrap_or_default();
    set_window_text(hwnd, &tr(language, "calendar.title"));
    refresh_calendar_controls(hwnd);
    populate_days(hwnd, selected_date);
    if let Some(list) = with_calendar_state(hwnd, |state| state.list) {
        crate::set_focus_safe(list);
        notify_focus(list);
    }
}

fn populate_day_details(hwnd: HWND) {
    let Some((language, list, date, reminders)) = with_calendar_state(hwnd, |state| {
        (
            state.language,
            state.list,
            state.detail_date,
            state.reminders.clone(),
        )
    }) else {
        return;
    };
    let Some(date) = date else {
        return;
    };
    crate::send_message_w_safe(list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    let mut ids = Vec::new();

    add_detail_item(list, &mut ids, localized_date(language, date), None);
    if let Some(holiday) = holiday_for_date(language, date) {
        add_detail_item(
            list,
            &mut ids,
            format!("{}: {}", tr(language, "calendar.holiday"), holiday),
            None,
        );
    }
    let saint =
        saint_for_date(language, date).unwrap_or_else(|| tr(language, "calendar.not_available"));
    add_detail_item(
        list,
        &mut ids,
        format!("{}: {}", tr(language, "calendar.saint_of_the_day"), saint),
        None,
    );
    let quote = quote_for_date(language, date);
    add_detail_item(
        list,
        &mut ids,
        format!("{}: {}", tr(language, "calendar.quote_of_the_day"), quote),
        None,
    );

    let day_reminders = reminders_for_date(&reminders, date);
    if day_reminders.is_empty() {
        add_detail_item(list, &mut ids, tr(language, "calendar.no_reminders"), None);
    } else {
        add_detail_item(
            list,
            &mut ids,
            format!(
                "{}: {}",
                tr(language, "calendar.reminders"),
                day_reminders.len()
            ),
            None,
        );
        for reminder in day_reminders {
            add_detail_item(
                list,
                &mut ids,
                format_reminder(language, &reminder),
                Some(reminder.id.clone()),
            );
        }
    }
    let _state_updated = with_calendar_state(hwnd, |state| state.detail_reminder_ids = ids);
    crate::send_message_w_safe(list, LB_SETCURSEL, WPARAM(0), LPARAM(0));
    update_reminder_action_buttons(hwnd);
}

fn add_detail_item(
    list: HWND,
    ids: &mut Vec<Option<String>>,
    text: String,
    reminder_id: Option<String>,
) {
    add_list_string(list, &text);
    ids.push(reminder_id);
}

fn refresh_calendar_controls(hwnd: HWND) {
    let Some((language, view, label, today, back, add, delete, copy, listen)) =
        with_calendar_state(hwnd, |state| {
            (
                state.language,
                state.view,
                state.list_label,
                state.button_today,
                state.button_back,
                state.button_add,
                state.button_delete,
                state.button_copy,
                state.button_listen,
            )
        })
    else {
        return;
    };
    match view {
        CalendarView::Days => {
            set_window_text(label, &tr(language, "calendar.instructions"));
            show_control(today, true);
            for control in [back, add, delete, copy, listen] {
                show_control(control, false);
            }
        }
        CalendarView::DayDetails => {
            set_window_text(label, &tr(language, "calendar.day_details"));
            show_control(today, false);
            for control in [back, add, delete, copy, listen] {
                show_control(control, true);
            }
        }
    }
    update_reminder_action_buttons(hwnd);
    layout_calendar(hwnd);
}

fn update_reminder_action_buttons(hwnd: HWND) {
    let has_selected = selected_detail_reminder_id(hwnd).is_some();
    if let Some((view, delete)) =
        with_calendar_state(hwnd, |state| (state.view, state.button_delete))
    {
        let enabled = view == CalendarView::DayDetails && has_selected;
        unsafe {
            EnableWindow(delete, enabled);
        }
    }
}

fn selected_detail_reminder_id(hwnd: HWND) -> Option<String> {
    let (view, list, ids) = with_calendar_state(hwnd, |state| {
        (state.view, state.list, state.detail_reminder_ids.clone())
    })?;
    if view != CalendarView::DayDetails {
        return None;
    }
    let index = selected_list_index(list)?;
    ids.get(index).cloned().flatten()
}

fn add_reminder(hwnd: HWND) {
    let Some((parent, language, date)) = with_calendar_state(hwnd, |state| {
        (state.parent, state.language, state.detail_date)
    }) else {
        return;
    };
    let Some(date) = date else {
        return;
    };
    let Some(new_reminder) = show_reminder_dialog(hwnd, language, date) else {
        focus_calendar_list(hwnd);
        return;
    };
    let reminder = CalendarReminder {
        id: Uuid::new_v4().to_string(),
        date: date.format("%Y-%m-%d").to_string(),
        text: new_reminder.text,
        has_time: new_reminder.has_time,
        hour: new_reminder.hour,
        minute: new_reminder.minute,
        alert_minutes: new_reminder.alert_minutes,
        completed: false,
        alerted: false,
        snoozed_until: None,
    };
    let mut reminders = load_reminders();
    reminders.push(reminder.clone());
    if let Err(error) = save_reminders(&reminders) {
        show_error_message(hwnd, language, &error);
        focus_calendar_list(hwnd);
        return;
    }
    if reminder.has_time
        && let Err(error) = schedule_reminder_task(&reminder)
    {
        crate::log_debug(&format!(
            "Calendar reminder task scheduling failed id={} error={error}",
            reminder.id
        ));
        show_warning_message(
            hwnd,
            language,
            &tr(language, "calendar.task_schedule_failed"),
        );
    }
    let _state_updated = with_calendar_state(hwnd, |state| state.reminders = reminders);
    populate_day_details(hwnd);
    screen_reader_speak(&tr(language, "calendar.reminder_saved"));
    focus_calendar_list(hwnd);
    check_due_reminders(parent, None);
}

fn delete_selected_reminder(hwnd: HWND) {
    let Some(id) = selected_detail_reminder_id(hwnd) else {
        return;
    };
    let language = with_calendar_state(hwnd, |state| state.language).unwrap_or_default();
    let result = show_question(
        hwnd,
        &tr(language, "calendar.delete_reminder"),
        &tr(language, "calendar.delete_confirm"),
    );
    if !result {
        focus_calendar_list(hwnd);
        return;
    }
    let mut reminders = load_reminders();
    reminders.retain(|reminder| reminder.id != id);
    if let Err(error) = save_reminders(&reminders) {
        show_error_message(hwnd, language, &error);
        focus_calendar_list(hwnd);
        return;
    }
    delete_reminder_task(&id);
    let _state_updated = with_calendar_state(hwnd, |state| state.reminders = reminders);
    populate_day_details(hwnd);
    screen_reader_speak(&tr(language, "calendar.reminder_deleted"));
    focus_calendar_list(hwnd);
}

fn copy_selected_day(hwnd: HWND) {
    let Some((language, date, reminders)) = with_calendar_state(hwnd, |state| {
        (state.language, state.detail_date, state.reminders.clone())
    }) else {
        return;
    };
    let Some(date) = date else {
        return;
    };
    let day_reminders = reminders_for_date(&reminders, date);
    let include_reminders = if day_reminders.is_empty() {
        false
    } else {
        show_question(
            hwnd,
            &tr(language, "calendar.copy_day"),
            &tr(language, "calendar.include_reminders"),
        )
    };
    let text = build_day_text(language, date, &reminders, include_reminders);
    copy_text_to_clipboard(hwnd, &text);
    screen_reader_speak(&tr(language, "calendar.copied"));
    focus_calendar_list(hwnd);
}

fn listen_selected_day(hwnd: HWND) {
    let Some((parent, language, date, reminders)) = with_calendar_state(hwnd, |state| {
        (
            state.parent,
            state.language,
            state.detail_date,
            state.reminders.clone(),
        )
    }) else {
        return;
    };
    let Some(date) = date else {
        return;
    };
    let text = build_day_text(language, date, &reminders, true);
    crate::tts_engine::speak_text_once(parent, text);
    focus_calendar_list(hwnd);
}

fn build_day_text(
    language: Language,
    date: NaiveDate,
    reminders: &[CalendarReminder],
    include_reminders: bool,
) -> String {
    let mut lines = Vec::new();
    lines.push(localized_date(language, date));
    if let Some(holiday) = holiday_for_date(language, date) {
        lines.push(format!("{}: {}", tr(language, "calendar.holiday"), holiday));
    }
    let saint =
        saint_for_date(language, date).unwrap_or_else(|| tr(language, "calendar.not_available"));
    lines.push(format!(
        "{}: {}",
        tr(language, "calendar.saint_of_the_day"),
        saint
    ));
    let day_reminders = reminders_for_date(reminders, date);
    if include_reminders && !day_reminders.is_empty() {
        lines.push(tr(language, "calendar.reminders"));
        for reminder in day_reminders {
            lines.push(format_reminder(language, &reminder));
        }
    }
    lines.push(tr(language, "calendar.quote_of_the_day"));
    lines.push(quote_for_date(language, date));
    lines.join("\r\n")
}

fn show_reminder_dialog(parent: HWND, language: Language, date: NaiveDate) -> Option<NewReminder> {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(REMINDER_CLASS_NAME);
        let window_class = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(reminder_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&window_class);

        let mut data = Box::new(ReminderDialogData {
            language,
            date,
            result: None,
            edit_text: HWND(0),
            check_time: HWND(0),
            combo_hour: HWND(0),
            combo_minute: HWND(0),
            combo_alert: HWND(0),
        });
        let data_pointer = data.as_mut() as *mut ReminderDialogData;
        let title = to_wide(&tr(language, "calendar.add_reminder"));
        EnableWindow(parent, false);
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            720,
            500,
            parent,
            HMENU(0),
            hinstance,
            Some(data_pointer.cast()),
        );
        if hwnd.0 == 0 {
            EnableWindow(parent, true);
            return None;
        }
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);

        let mut message = MSG::default();
        while IsWindow(hwnd).as_bool() && GetMessageW(&mut message, HWND(0), 0, 0).as_bool() {
            if handle_reminder_alert_message(&message) {
                continue;
            }
            let key = message.wParam.0 as u16;
            if message.message == WM_KEYDOWN
                && message.hwnd == data.combo_minute
                && (key == VK_LEFT.0 || key == VK_RIGHT.0)
            {
                adjust_minute_combo_by_five(data.combo_minute, key == VK_RIGHT.0);
                continue;
            }
            if message.message == WM_KEYDOWN && message.wParam.0 as u32 == VK_ESCAPE.0 as u32 {
                crate::log_if_err!(DestroyWindow(hwnd));
                continue;
            }
            if crate::handle_focused_edit_shortcut(&message) {
                continue;
            }
            if !IsDialogMessageW(hwnd, &message).as_bool() {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        EnableWindow(parent, true);
        SetForegroundWindow(parent);
        data.result.take()
    }
}

unsafe extern "system" fn reminder_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "calendar_reminder_wndproc",
            || DefWindowProcW(hwnd, message, wparam, lparam),
            || reminder_wndproc_inner(hwnd, message, wparam, lparam),
        )
    }
}

fn reminder_wndproc_inner(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_CREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                let data = (*create).lpCreateParams as *mut ReminderDialogData;
                crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, data as isize);
                create_reminder_controls(hwnd);
                LRESULT(0)
            }
            WM_SIZE => {
                layout_reminder(hwnd);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                if let Some(edit) = with_reminder_data(hwnd, |data| data.edit_text) {
                    crate::set_focus_safe(edit);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                match id {
                    ID_REMINDER_CHECK_TIME => {
                        update_reminder_time_controls(hwnd);
                        LRESULT(0)
                    }
                    ID_REMINDER_SAVE => {
                        save_reminder_dialog(hwnd);
                        LRESULT(0)
                    }
                    ID_REMINDER_CANCEL => {
                        crate::log_if_err!(DestroyWindow(hwnd));
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
                }
            }
            WM_CLOSE => {
                crate::log_if_err!(DestroyWindow(hwnd));
                LRESULT(0)
            }
            WM_NCDESTROY => {
                crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, 0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

fn create_reminder_controls(hwnd: HWND) {
    let Some((language, date)) = with_reminder_data(hwnd, |data| (data.language, data.date)) else {
        return;
    };
    let label_date = create_static(
        hwnd,
        &localized_date(language, date),
        ID_REMINDER_LABEL_DATE,
    );
    let label_text = create_static(
        hwnd,
        &tr(language, "calendar.reminder_text"),
        ID_REMINDER_LABEL_TEXT,
    );
    let edit_text = create_edit(hwnd, ID_REMINDER_EDIT_TEXT);
    let check_time = create_checkbox(
        hwnd,
        &tr(language, "calendar.set_time"),
        ID_REMINDER_CHECK_TIME,
    );
    let label_hour = create_static(hwnd, &tr(language, "calendar.hour"), ID_REMINDER_LABEL_HOUR);
    let combo_hour = create_combo(hwnd, ID_REMINDER_COMBO_HOUR);
    let label_minute = create_static(
        hwnd,
        &tr(language, "calendar.minute"),
        ID_REMINDER_LABEL_MINUTE,
    );
    let combo_minute = create_combo(hwnd, ID_REMINDER_COMBO_MINUTE);
    let label_alert = create_static(
        hwnd,
        &tr(language, "calendar.alert"),
        ID_REMINDER_LABEL_ALERT,
    );
    let combo_alert = create_combo(hwnd, ID_REMINDER_COMBO_ALERT);
    let info = create_static(
        hwnd,
        &tr(language, "calendar.internal_alert_info"),
        ID_REMINDER_INFO,
    );
    let save = create_button(hwnd, &tr(language, "calendar.save"), ID_REMINDER_SAVE, true);
    let cancel = create_button(
        hwnd,
        &tr(language, "calendar.cancel"),
        ID_REMINDER_CANCEL,
        false,
    );

    let font = HFONT(crate::get_stock_object_safe(DEFAULT_GUI_FONT).0);
    for control in [
        label_date,
        label_text,
        edit_text,
        check_time,
        label_hour,
        combo_hour,
        label_minute,
        combo_minute,
        label_alert,
        combo_alert,
        info,
        save,
        cancel,
    ] {
        crate::send_message_w_safe(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    }

    for hour in 0..24 {
        add_combo_string(combo_hour, &format!("{hour:02}"));
    }
    for minute in 0..60 {
        add_combo_string(combo_minute, &format!("{minute:02}"));
    }
    for key in [
        "calendar.alert_at_time",
        "calendar.alert_5_minutes",
        "calendar.alert_15_minutes",
        "calendar.alert_30_minutes",
        "calendar.alert_1_hour",
        "calendar.alert_1_day",
    ] {
        add_combo_string(combo_alert, &tr(language, key));
    }
    let now = Local::now();
    let default_hour = if date == now.date_naive() {
        now.hour().saturating_add(1).min(23)
    } else {
        9
    };
    crate::send_message_w_safe(
        combo_hour,
        CB_SETCURSEL,
        WPARAM(default_hour as usize),
        LPARAM(0),
    );
    crate::send_message_w_safe(combo_minute, CB_SETCURSEL, WPARAM(0), LPARAM(0));
    crate::send_message_w_safe(combo_alert, CB_SETCURSEL, WPARAM(2), LPARAM(0));
    crate::send_message_w_safe(
        check_time,
        BM_SETCHECK,
        WPARAM(BST_UNCHECKED.0 as usize),
        LPARAM(0),
    );
    let _dialog_updated = with_reminder_data(hwnd, |data| {
        data.edit_text = edit_text;
        data.check_time = check_time;
        data.combo_hour = combo_hour;
        data.combo_minute = combo_minute;
        data.combo_alert = combo_alert;
    });
    update_reminder_time_controls(hwnd);
    layout_reminder(hwnd);
    crate::set_focus_safe(edit_text);
}

fn update_reminder_time_controls(hwnd: HWND) {
    let Some((check_time, combo_hour, combo_minute, combo_alert)) =
        with_reminder_data(hwnd, |data| {
            (
                data.check_time,
                data.combo_hour,
                data.combo_minute,
                data.combo_alert,
            )
        })
    else {
        return;
    };
    let enabled = is_checked(check_time);
    unsafe {
        for id in [
            ID_REMINDER_LABEL_HOUR,
            ID_REMINDER_LABEL_MINUTE,
            ID_REMINDER_LABEL_ALERT,
        ] {
            EnableWindow(GetDlgItem(hwnd, id as i32), enabled);
        }
        EnableWindow(combo_hour, enabled);
        EnableWindow(combo_minute, enabled);
        EnableWindow(combo_alert, enabled);
    }
}

pub(crate) fn adjust_minute_combo_by_five(combo: HWND, increase: bool) {
    let current = selected_combo_index(combo).unwrap_or(0).min(59);
    let adjusted = if increase {
        current.saturating_add(5).min(59)
    } else {
        current.saturating_sub(5)
    };
    crate::send_message_w_safe(combo, CB_SETCURSEL, WPARAM(adjusted), LPARAM(0));
    screen_reader_speak(&format!("{adjusted:02}"));
}

fn save_reminder_dialog(hwnd: HWND) {
    let Some((language, edit_text, check_time, combo_hour, combo_minute, combo_alert)) =
        with_reminder_data(hwnd, |data| {
            (
                data.language,
                data.edit_text,
                data.check_time,
                data.combo_hour,
                data.combo_minute,
                data.combo_alert,
            )
        })
    else {
        return;
    };
    let text = get_control_text(edit_text).trim().to_string();
    if text.is_empty() {
        show_warning_message(hwnd, language, &tr(language, "calendar.enter_reminder"));
        crate::set_focus_safe(edit_text);
        return;
    }
    let has_time = is_checked(check_time);
    let hour = selected_combo_index(combo_hour).unwrap_or(9) as u32;
    let minute = selected_combo_index(combo_minute).unwrap_or(0) as u32;
    let alert_values = [0u32, 5, 15, 30, 60, 1440];
    let alert_index = selected_combo_index(combo_alert).unwrap_or(2);
    let alert_minutes = alert_values.get(alert_index).copied().unwrap_or(15);
    let _dialog_updated = with_reminder_data(hwnd, |data| {
        data.result = Some(NewReminder {
            text,
            has_time,
            hour,
            minute,
            alert_minutes,
        });
    });
    unsafe {
        crate::log_if_err!(DestroyWindow(hwnd));
    }
}

fn layout_calendar(hwnd: HWND) {
    unsafe {
        let mut rect = RECT::default();
        if GetClientRect(hwnd, &mut rect).is_err() {
            return;
        }
        let width = rect.right.saturating_sub(rect.left).max(640);
        let height = rect.bottom.saturating_sub(rect.top).max(480);
        let margin = 16;
        let label_height = 32;
        let Some((view, label, list, today, back, add, delete, copy, listen, close)) =
            with_calendar_state(hwnd, |state| {
                (
                    state.view,
                    state.list_label,
                    state.list,
                    state.button_today,
                    state.button_back,
                    state.button_add,
                    state.button_delete,
                    state.button_copy,
                    state.button_listen,
                    state.button_close,
                )
            })
        else {
            return;
        };
        crate::log_if_err!(MoveWindow(
            label,
            margin,
            margin,
            width - margin * 2,
            label_height,
            true
        ));
        let list_bottom_space = if view == CalendarView::DayDetails {
            126
        } else {
            70
        };
        crate::log_if_err!(MoveWindow(
            list,
            margin,
            margin + label_height,
            width - margin * 2,
            height - margin * 2 - label_height - list_bottom_space,
            true
        ));
        if view == CalendarView::Days {
            crate::log_if_err!(MoveWindow(today, margin, height - 48, 190, 32, true));
            crate::log_if_err!(MoveWindow(
                close,
                width - margin - 130,
                height - 48,
                130,
                32,
                true
            ));
        } else {
            let y_actions = height - 92;
            let mut x = margin;
            for (control, control_width) in [(add, 170), (delete, 170), (copy, 140), (listen, 140)]
            {
                crate::log_if_err!(MoveWindow(control, x, y_actions, control_width, 32, true));
                x += control_width + 8;
            }
            crate::log_if_err!(MoveWindow(back, margin, height - 48, 170, 32, true));
            crate::log_if_err!(MoveWindow(
                close,
                width - margin - 130,
                height - 48,
                130,
                32,
                true
            ));
        }
    }
}

fn layout_reminder(hwnd: HWND) {
    unsafe {
        let mut rect = RECT::default();
        if GetClientRect(hwnd, &mut rect).is_err() {
            return;
        }
        let width = rect.right.saturating_sub(rect.left).max(600);
        let height = rect.bottom.saturating_sub(rect.top).max(420);
        let margin = 18;
        let content_width = width - margin * 2;
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_LABEL_DATE as i32),
            margin,
            16,
            content_width,
            28,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_LABEL_TEXT as i32),
            margin,
            52,
            content_width,
            24,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_EDIT_TEXT as i32),
            margin,
            78,
            content_width,
            34,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_CHECK_TIME as i32),
            margin,
            128,
            content_width,
            28,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_LABEL_HOUR as i32),
            margin,
            174,
            90,
            24,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_COMBO_HOUR as i32),
            margin,
            200,
            100,
            260,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_LABEL_MINUTE as i32),
            margin + 120,
            174,
            90,
            24,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_COMBO_MINUTE as i32),
            margin + 120,
            200,
            100,
            260,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_LABEL_ALERT as i32),
            margin + 240,
            174,
            220,
            24,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_COMBO_ALERT as i32),
            margin + 240,
            200,
            content_width - 240,
            260,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_INFO as i32),
            margin,
            270,
            content_width,
            48,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_SAVE as i32),
            width - margin - 280,
            height - 52,
            130,
            34,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_REMINDER_CANCEL as i32),
            width - margin - 140,
            height - 52,
            130,
            34,
            true
        ));
    }
}

fn format_day_row(
    language: Language,
    date: NaiveDate,
    today: NaiveDate,
    reminders: &[CalendarReminder],
) -> String {
    let prefix = if date == today {
        Some(tr(language, "calendar.today"))
    } else if date == today + Duration::days(1) {
        Some(tr(language, "calendar.tomorrow"))
    } else if date == today - Duration::days(1) {
        Some(tr(language, "calendar.yesterday"))
    } else {
        None
    };
    let mut pieces = Vec::new();
    if let Some(prefix) = prefix {
        pieces.push(prefix);
    }
    pieces.push(localized_date(language, date));
    if let Some(holiday) = holiday_for_date(language, date) {
        pieces.push(holiday);
    }
    if let Some(saint) = saint_for_date(language, date) {
        pieces.push(format!(
            "{}: {}",
            tr(language, "calendar.saint_of_the_day"),
            saint
        ));
    }
    let count = reminders_for_date(reminders, date).len();
    if count > 0 {
        pieces.push(format!("{}: {}", tr(language, "calendar.reminders"), count));
    }
    pieces.join(" - ")
}

fn format_reminder(language: Language, reminder: &CalendarReminder) -> String {
    if reminder.has_time {
        format!(
            "{} {:02}:{:02}: {}",
            tr(language, "calendar.reminder"),
            reminder.hour,
            reminder.minute,
            reminder.text
        )
    } else {
        format!("{}: {}", tr(language, "calendar.reminder"), reminder.text)
    }
}

fn reminders_for_date(reminders: &[CalendarReminder], date: NaiveDate) -> Vec<CalendarReminder> {
    let date_key = date.format("%Y-%m-%d").to_string();
    let mut result = reminders
        .iter()
        .filter(|reminder| reminder.date == date_key && !reminder.completed)
        .cloned()
        .collect::<Vec<_>>();
    result.sort_by(|left, right| {
        (
            left.has_time,
            left.hour,
            left.minute,
            left.text.to_lowercase(),
        )
            .cmp(&(
                right.has_time,
                right.hour,
                right.minute,
                right.text.to_lowercase(),
            ))
    });
    result
}

fn reminders_path() -> PathBuf {
    settings_dir().join("calendar_reminders.json")
}

fn load_reminders() -> Vec<CalendarReminder> {
    let path = reminders_path();
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<CalendarReminder>>(&content) {
        Ok(mut reminders) => {
            reminders.sort_by(|left, right| {
                (&left.date, left.hour, left.minute, left.text.to_lowercase()).cmp(&(
                    &right.date,
                    right.hour,
                    right.minute,
                    right.text.to_lowercase(),
                ))
            });
            reminders
        }
        Err(error) => {
            crate::log_debug(&format!(
                "Calendar reminders parse failed path={} error={error}",
                path.display()
            ));
            Vec::new()
        }
    }
}

fn save_reminders(reminders: &[CalendarReminder]) -> Result<(), String> {
    let path = reminders_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let content = serde_json::to_string_pretty(reminders).map_err(|error| error.to_string())?;
    std::fs::write(&path, content).map_err(|error| error.to_string())
}

pub(crate) fn initialize_reminder_system(parent: HWND, forced_id: Option<&str>) {
    unsafe {
        if SetTimer(parent, REMINDER_TIMER_ID, REMINDER_CHECK_INTERVAL_MS, None) == 0 {
            crate::log_debug("Calendar reminder timer could not be created");
        }
    }
    if forced_id.is_none() {
        let pending = load_reminders()
            .into_iter()
            .filter(|reminder| reminder.has_time && !reminder.completed && !reminder.alerted)
            .collect::<Vec<_>>();
        let _task_sync_thread = std::thread::spawn(move || {
            for reminder in pending {
                if let Err(error) = schedule_reminder_task(&reminder) {
                    crate::log_debug(&format!(
                        "Calendar startup task synchronization failed id={} error={error}",
                        reminder.id
                    ));
                }
            }
        });
    }
    match forced_id {
        Some(id) if !id.trim().is_empty() => show_reminder_by_id(parent, id.trim()),
        _ => check_due_reminders(parent, None),
    }
}

pub(crate) fn handle_reminder_timer(parent: HWND, timer_id: usize) -> bool {
    if timer_id != REMINDER_TIMER_ID {
        return false;
    }
    check_due_reminders(parent, None);
    true
}

pub(crate) fn show_reminder_by_id(parent: HWND, reminder_id: &str) {
    check_due_reminders(parent, Some(reminder_id));
}

pub(crate) fn handle_reminder_alert_message(message: &MSG) -> bool {
    let alert = active_alert_window();
    if alert.0 == 0 || !unsafe { IsWindow(alert).as_bool() } {
        return false;
    }
    let belongs_to_alert = message.hwnd == alert
        || (message.hwnd.0 != 0 && unsafe { IsChild(alert, message.hwnd).as_bool() });
    if !belongs_to_alert {
        return false;
    }
    if message.message == WM_KEYDOWN && message.wParam.0 as u32 == VK_ESCAPE.0 as u32 {
        unsafe {
            crate::log_if_err!(DestroyWindow(alert));
        }
        return true;
    }
    unsafe { IsDialogMessageW(alert, message).as_bool() }
}

fn check_due_reminders(parent: HWND, forced_id: Option<&str>) {
    let now = Local::now().naive_local();
    let grace_start = now - Duration::hours(MISSED_REMINDER_GRACE_HOURS);
    let mut reminders = load_reminders();
    let mut due_ids = Vec::new();
    let forced_id = forced_id.map(str::trim).filter(|value| !value.is_empty());

    for reminder in &mut reminders {
        if reminder.completed || reminder.alerted || !reminder.has_time {
            continue;
        }
        let forced = forced_id == Some(reminder.id.as_str());
        let Some(alert_at) = reminder_alert_datetime(reminder) else {
            continue;
        };
        let Some(event_at) = reminder_event_datetime(reminder) else {
            continue;
        };
        let due_now = if reminder.snoozed_until.is_some() {
            alert_at <= now && alert_at >= grace_start
        } else {
            alert_at <= now && event_at >= grace_start
        };
        if forced || due_now {
            reminder.alerted = true;
            reminder.snoozed_until = None;
            due_ids.push(reminder.id.clone());
        }
    }

    if due_ids.is_empty() {
        if let Some(id) = forced_id {
            crate::log_debug(&format!(
                "Calendar reminder launch ignored because reminder is missing, untimed, completed, or already shown: {id}"
            ));
        }
        return;
    }

    if let Err(error) = save_reminders(&reminders) {
        crate::log_debug(&format!(
            "Calendar due reminder state could not be saved: {error}"
        ));
        return;
    }
    for id in &due_ids {
        delete_reminder_task(id);
    }
    show_reminder_alert_window(parent, due_ids);
}

fn reminder_event_datetime(reminder: &CalendarReminder) -> Option<NaiveDateTime> {
    if !reminder.has_time {
        return None;
    }
    let date = NaiveDate::parse_from_str(&reminder.date, "%Y-%m-%d").ok()?;
    date.and_hms_opt(reminder.hour, reminder.minute, 0)
}

fn reminder_alert_datetime(reminder: &CalendarReminder) -> Option<NaiveDateTime> {
    if let Some(value) = reminder.snoozed_until.as_deref()
        && let Ok(parsed) = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
    {
        return Some(parsed);
    }
    reminder_event_datetime(reminder)
        .map(|event_at| event_at - Duration::minutes(i64::from(reminder.alert_minutes)))
}

fn schedule_reminder_task(reminder: &CalendarReminder) -> Result<(), String> {
    if reminder.completed || reminder.alerted || !reminder.has_time {
        delete_reminder_task(&reminder.id);
        return Ok(());
    }
    let Some(alert_at) = reminder_alert_datetime(reminder) else {
        return Ok(());
    };
    if alert_at <= Local::now().naive_local() {
        delete_reminder_task(&reminder.id);
        return Ok(());
    }
    create_scheduled_task(&reminder.id, alert_at)
}

fn create_scheduled_task(reminder_id: &str, alert_at: NaiveDateTime) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let working_directory = executable.parent().unwrap_or(Path::new(""));
    let username = std::env::var("USERNAME").unwrap_or_default();
    if username.trim().is_empty() {
        return Err("Windows user name is unavailable".to_string());
    }
    let domain = std::env::var("USERDOMAIN").unwrap_or_default();
    let user_id = if domain.trim().is_empty() {
        username
    } else {
        format!("{domain}\\{username}")
    };
    let task_name = reminder_task_name(reminder_id);
    let executable_text = executable.to_string_lossy();
    let working_directory_text = working_directory.to_string_lossy();
    let xml = format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\n",
            "<Task version=\"1.4\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\r\n",
            "  <RegistrationInfo><Author>{author}</Author><Description>Sonarpad calendar reminder</Description></RegistrationInfo>\r\n",
            "  <Triggers><TimeTrigger><StartBoundary>{start}</StartBoundary><Enabled>true</Enabled></TimeTrigger></Triggers>\r\n",
            "  <Principals><Principal id=\"Author\"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>\r\n",
            "  <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><AllowHardTerminate>true</AllowHardTerminate><StartWhenAvailable>true</StartWhenAvailable><RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable><AllowStartOnDemand>true</AllowStartOnDemand><Enabled>true</Enabled><Hidden>false</Hidden><RunOnlyIfIdle>false</RunOnlyIfIdle><WakeToRun>false</WakeToRun><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><Priority>7</Priority></Settings>\r\n",
            "  <Actions Context=\"Author\"><Exec><Command>{command}</Command><Arguments>--calendar-reminder {argument}</Arguments><WorkingDirectory>{working}</WorkingDirectory></Exec></Actions>\r\n",
            "</Task>\r\n"
        ),
        author = xml_escape(&user_id),
        start = alert_at.format("%Y-%m-%dT%H:%M:%S"),
        user = xml_escape(&user_id),
        command = xml_escape(executable_text.as_ref()),
        argument = xml_escape(reminder_id),
        working = xml_escape(working_directory_text.as_ref()),
    );

    let task_dir = settings_dir().join("CalendarTasks");
    std::fs::create_dir_all(&task_dir).map_err(|error| error.to_string())?;
    let xml_path = task_dir.join(format!("{}.xml", safe_task_component(reminder_id)));
    write_utf16_xml(&xml_path, &xml)?;

    let output = Command::new("schtasks.exe")
        .arg("/Create")
        .arg("/TN")
        .arg(task_name)
        .arg("/XML")
        .arg(&xml_path)
        .arg("/F")
        .creation_flags(CREATE_NO_WINDOW_FLAG)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| error.to_string());
    if let Err(error) = std::fs::remove_file(&xml_path) {
        crate::log_debug(&format!(
            "Calendar temporary task XML removal failed path={} error={error}",
            xml_path.display()
        ));
    }
    let output = output?;
    if output.status.success() {
        crate::log_debug(&format!(
            "Calendar reminder task scheduled: id={reminder_id} at={alert_at}"
        ));
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(if stderr.is_empty() { stdout } else { stderr })
}

fn delete_reminder_task(reminder_id: &str) {
    let task_name = reminder_task_name(reminder_id);
    match Command::new("schtasks.exe")
        .arg("/Delete")
        .arg("/TN")
        .arg(task_name)
        .arg("/F")
        .creation_flags(CREATE_NO_WINDOW_FLAG)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {
            crate::log_debug(&format!("Calendar reminder task removed: id={reminder_id}"));
        }
        Ok(_) => {}
        Err(error) => crate::log_debug(&format!(
            "Calendar reminder task removal failed id={reminder_id} error={error}"
        )),
    }
}

fn reminder_task_name(reminder_id: &str) -> String {
    format!(
        "Sonarpad Calendar Reminder {}",
        safe_task_component(reminder_id)
    )
}

fn safe_task_component(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .collect()
}

fn write_utf16_xml(path: &Path, text: &str) -> Result<(), String> {
    let mut file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    file.write_all(&[0xff, 0xfe])
        .map_err(|error| error.to_string())?;
    for unit in text.encode_utf16() {
        file.write_all(&unit.to_le_bytes())
            .map_err(|error| error.to_string())?;
    }
    file.flush().map_err(|error| error.to_string())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn alert_window_slot() -> &'static Mutex<isize> {
    static SLOT: OnceLock<Mutex<isize>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(0))
}

fn active_alert_window() -> HWND {
    match alert_window_slot().lock() {
        Ok(value) => HWND(*value),
        Err(poisoned) => HWND(*poisoned.into_inner()),
    }
}

pub(crate) fn has_active_reminder_alert() -> bool {
    let hwnd = active_alert_window();
    hwnd.0 != 0 && unsafe { IsWindow(hwnd).as_bool() }
}

fn set_active_alert_window(hwnd: HWND) {
    match alert_window_slot().lock() {
        Ok(mut value) => *value = hwnd.0,
        Err(poisoned) => *poisoned.into_inner() = hwnd.0,
    }
}

fn show_reminder_alert_window(parent: HWND, reminder_ids: Vec<String>) {
    let existing = active_alert_window();
    if existing.0 != 0 && unsafe { IsWindow(existing).as_bool() } {
        let _alert_updated = with_alert_state(existing, |state| {
            for id in reminder_ids {
                if !state.reminder_ids.contains(&id) {
                    state.reminder_ids.push(id);
                }
            }
        });
        refresh_alert_window(existing);
        announce_alert_window(existing);
        unsafe {
            ShowWindow(existing, SW_SHOW);
            SetForegroundWindow(existing);
        }
        return;
    }

    unsafe {
        let language = load_settings().language;
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(ALERT_CLASS_NAME);
        let window_class = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(alert_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&window_class);

        let mut state = Box::new(ReminderAlertWindowState {
            parent,
            language,
            list: HWND(0),
            combo_snooze: HWND(0),
            reminder_ids,
            previous_focus: crate::get_focus_safe(),
        });
        let state_pointer = state.as_mut() as *mut ReminderAlertWindowState;
        let title = to_wide(&tr(language, "calendar.reminder_alert_title"));
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            780,
            520,
            HWND(0),
            HMENU(0),
            hinstance,
            Some(state_pointer.cast()),
        );
        if hwnd.0 == 0 {
            return;
        }
        let _owned_by_window = Box::into_raw(state);
        set_active_alert_window(hwnd);
        ShowWindow(hwnd, SW_SHOW);
        let foreground = GetForegroundWindow();
        if foreground.0 == 0 || foreground == parent || IsChild(parent, foreground).as_bool() {
            SetForegroundWindow(hwnd);
            if let Some(list) = with_alert_state(hwnd, |current| current.list) {
                crate::set_focus_safe(list);
                notify_focus(list);
            }
        }
        crate::log_if_err!(MessageBeep(MB_ICONWARNING));
        announce_alert_window(hwnd);
    }
}

unsafe extern "system" fn alert_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "calendar_alert_wndproc",
            || DefWindowProcW(hwnd, message, wparam, lparam),
            || alert_wndproc_inner(hwnd, message, wparam, lparam),
        )
    }
}

fn alert_wndproc_inner(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_CREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                let state = (*create).lpCreateParams as *mut ReminderAlertWindowState;
                crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, state as isize);
                create_alert_controls(hwnd);
                LRESULT(0)
            }
            WM_SIZE => {
                layout_alert_window(hwnd);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                if let Some(list) = with_alert_state(hwnd, |state| state.list) {
                    crate::set_focus_safe(list);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                match id {
                    ID_ALERT_COMPLETE => {
                        complete_selected_alert(hwnd);
                        LRESULT(0)
                    }
                    ID_ALERT_SNOOZE => {
                        snooze_selected_alert(hwnd);
                        LRESULT(0)
                    }
                    ID_ALERT_CLOSE => {
                        crate::log_if_err!(DestroyWindow(hwnd));
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
                }
            }
            WM_CLOSE => {
                crate::log_if_err!(DestroyWindow(hwnd));
                LRESULT(0)
            }
            WM_DESTROY => {
                let restore = GetForegroundWindow() == hwnd;
                if let Some((parent, previous_focus)) =
                    with_alert_state(hwnd, |state| (state.parent, state.previous_focus))
                {
                    set_active_alert_window(HWND(0));
                    if restore && parent.0 != 0 && IsWindow(parent).as_bool() {
                        SetForegroundWindow(parent);
                        if previous_focus.0 != 0 && IsWindow(previous_focus).as_bool() {
                            crate::set_focus_safe(previous_focus);
                        }
                    }
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let pointer = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA)
                    as *mut ReminderAlertWindowState;
                if !pointer.is_null() {
                    let _state = Box::from_raw(pointer);
                    crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, 0);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

fn create_alert_controls(hwnd: HWND) {
    let Some(language) = with_alert_state(hwnd, |state| state.language) else {
        return;
    };
    let label = create_static(
        hwnd,
        &tr(language, "calendar.pending_reminders"),
        ID_ALERT_LABEL,
    );
    let list = create_listbox(hwnd, ID_ALERT_LIST);
    let combo_snooze = create_combo(hwnd, ID_ALERT_SNOOZE_COMBO);
    for key in [
        "calendar.snooze_5_minutes",
        "calendar.snooze_10_minutes",
        "calendar.snooze_30_minutes",
        "calendar.snooze_1_hour",
        "calendar.snooze_tomorrow",
    ] {
        add_combo_string(combo_snooze, &tr(language, key));
    }
    crate::send_message_w_safe(combo_snooze, CB_SETCURSEL, WPARAM(1), LPARAM(0));
    let button_complete = create_button(
        hwnd,
        &tr(language, "calendar.complete"),
        ID_ALERT_COMPLETE,
        true,
    );
    let button_snooze = create_button(
        hwnd,
        &tr(language, "calendar.snooze"),
        ID_ALERT_SNOOZE,
        false,
    );
    let button_close = create_button(hwnd, &tr(language, "calendar.close"), ID_ALERT_CLOSE, false);
    let font = HFONT(crate::get_stock_object_safe(DEFAULT_GUI_FONT).0);
    for control in [
        label,
        list,
        combo_snooze,
        button_complete,
        button_snooze,
        button_close,
    ] {
        crate::send_message_w_safe(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    }
    let _alert_updated = with_alert_state(hwnd, |state| {
        state.list = list;
        state.combo_snooze = combo_snooze;
    });
    refresh_alert_window(hwnd);
    layout_alert_window(hwnd);
    crate::set_focus_safe(list);
}

fn refresh_alert_window(hwnd: HWND) {
    let reminders = load_reminders();
    let Some((language, list, current_ids)) = with_alert_state(hwnd, |state| {
        (state.language, state.list, state.reminder_ids.clone())
    }) else {
        return;
    };
    let mut retained_ids = Vec::new();
    crate::send_message_w_safe(list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    for id in current_ids {
        let Some(reminder) = reminders
            .iter()
            .find(|item| item.id == id && !item.completed)
        else {
            continue;
        };
        add_list_string(list, &format_alert_row(language, reminder));
        retained_ids.push(id);
    }
    let _alert_updated = with_alert_state(hwnd, |state| state.reminder_ids = retained_ids.clone());
    let label = unsafe { GetDlgItem(hwnd, ID_ALERT_LABEL as i32) };
    set_window_text(
        label,
        &format!(
            "{}: {}",
            tr(language, "calendar.pending_reminders"),
            retained_ids.len()
        ),
    );
    if retained_ids.is_empty() {
        unsafe {
            crate::log_if_err!(DestroyWindow(hwnd));
        }
    } else {
        crate::send_message_w_safe(list, LB_SETCURSEL, WPARAM(0), LPARAM(0));
    }
}

fn format_alert_row(language: Language, reminder: &CalendarReminder) -> String {
    let date = NaiveDate::parse_from_str(&reminder.date, "%Y-%m-%d").ok();
    let date_text = date
        .map(|value| localized_date(language, value))
        .unwrap_or_else(|| reminder.date.clone());
    format!(
        "{} — {:02}:{:02} — {}",
        date_text, reminder.hour, reminder.minute, reminder.text
    )
}

fn selected_alert_id(hwnd: HWND) -> Option<String> {
    let (list, ids) = with_alert_state(hwnd, |state| (state.list, state.reminder_ids.clone()))?;
    let index = selected_list_index(list)?;
    ids.get(index).cloned()
}

fn complete_selected_alert(hwnd: HWND) {
    let Some(id) = selected_alert_id(hwnd) else {
        return;
    };
    let language = with_alert_state(hwnd, |state| state.language).unwrap_or_default();
    let mut reminders = load_reminders();
    if let Some(reminder) = reminders.iter_mut().find(|item| item.id == id) {
        reminder.completed = true;
        reminder.alerted = true;
        reminder.snoozed_until = None;
    }
    if let Err(error) = save_reminders(&reminders) {
        show_error_message(hwnd, language, &error);
        return;
    }
    delete_reminder_task(&id);
    let _alert_updated = with_alert_state(hwnd, |state| {
        state.reminder_ids.retain(|current| current != &id)
    });
    screen_reader_speak(&tr(language, "calendar.reminder_completed"));
    refresh_alert_window(hwnd);
}

fn snooze_selected_alert(hwnd: HWND) {
    let Some(id) = selected_alert_id(hwnd) else {
        return;
    };
    let Some((language, combo)) =
        with_alert_state(hwnd, |state| (state.language, state.combo_snooze))
    else {
        return;
    };
    let minutes = match selected_combo_index(combo).unwrap_or(1) {
        0 => 5,
        1 => 10,
        2 => 30,
        3 => 60,
        _ => 1440,
    };
    let snoozed_until = Local::now().naive_local() + Duration::minutes(minutes);
    let mut reminders = load_reminders();
    let mut scheduled = None;
    if let Some(reminder) = reminders.iter_mut().find(|item| item.id == id) {
        reminder.completed = false;
        reminder.alerted = false;
        reminder.snoozed_until = Some(snoozed_until.format("%Y-%m-%dT%H:%M:%S").to_string());
        scheduled = Some(reminder.clone());
    }
    if let Err(error) = save_reminders(&reminders) {
        show_error_message(hwnd, language, &error);
        return;
    }
    if let Some(reminder) = scheduled
        && let Err(error) = schedule_reminder_task(&reminder)
    {
        crate::log_debug(&format!(
            "Calendar snoozed reminder task scheduling failed id={} error={error}",
            reminder.id
        ));
        show_warning_message(
            hwnd,
            language,
            &tr(language, "calendar.task_schedule_failed"),
        );
    }
    let _alert_updated = with_alert_state(hwnd, |state| {
        state.reminder_ids.retain(|current| current != &id)
    });
    screen_reader_speak(&tr(language, "calendar.reminder_snoozed"));
    refresh_alert_window(hwnd);
}

fn announce_alert_window(hwnd: HWND) {
    let reminders = load_reminders();
    let Some((language, ids)) =
        with_alert_state(hwnd, |state| (state.language, state.reminder_ids.clone()))
    else {
        return;
    };
    let first_text = ids
        .first()
        .and_then(|id| reminders.iter().find(|item| item.id == *id))
        .map(|reminder| reminder.text.as_str())
        .unwrap_or_default();
    let message = if ids.len() == 1 {
        format!(
            "{}. {}",
            tr(language, "calendar.reminder_alert_title"),
            first_text
        )
    } else {
        format!(
            "{}. {}: {}",
            tr(language, "calendar.reminder_alert_title"),
            tr(language, "calendar.pending_reminders"),
            ids.len()
        )
    };
    screen_reader_speak(&message);
}

fn layout_alert_window(hwnd: HWND) {
    unsafe {
        let mut rect = RECT::default();
        if GetClientRect(hwnd, &mut rect).is_err() {
            return;
        }
        let width = rect.right.saturating_sub(rect.left).max(620);
        let height = rect.bottom.saturating_sub(rect.top).max(420);
        let margin = 18;
        let content_width = width - margin * 2;
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_ALERT_LABEL as i32),
            margin,
            16,
            content_width,
            28,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_ALERT_LIST as i32),
            margin,
            48,
            content_width,
            height - 150,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_ALERT_SNOOZE_COMBO as i32),
            margin,
            height - 86,
            210,
            220,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_ALERT_COMPLETE as i32),
            margin + 228,
            height - 86,
            150,
            34,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_ALERT_SNOOZE as i32),
            margin + 386,
            height - 86,
            150,
            34,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_ALERT_CLOSE as i32),
            width - margin - 130,
            height - 44,
            130,
            32,
            true
        ));
    }
}

fn with_alert_state<T>(
    hwnd: HWND,
    callback: impl FnOnce(&mut ReminderAlertWindowState) -> T,
) -> Option<T> {
    let pointer =
        crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut ReminderAlertWindowState;
    if pointer.is_null() {
        return None;
    }
    unsafe { Some(callback(&mut *pointer)) }
}

fn calendar_data() -> &'static CalendarData {
    static DATA: OnceLock<CalendarData> = OnceLock::new();
    DATA.get_or_init(|| {
        let raw = include_str!("calendar_data.json");
        match serde_json::from_str::<CalendarData>(raw) {
            Ok(data) => data,
            Err(error) => {
                crate::log_debug(&format!("Calendar data parse failed: {error}"));
                CalendarData {
                    saints: HashMap::new(),
                    quotes: HashMap::new(),
                }
            }
        }
    })
}

fn saint_for_date(language: Language, date: NaiveDate) -> Option<String> {
    let key = format!("{}-{}", date.day(), date.month());
    let values = calendar_data().saints.get(&key)?;
    values.get(language_code(language)).cloned()
}

fn quote_for_date(language: Language, date: NaiveDate) -> String {
    let data = calendar_data();
    let list = data.quotes.get(language_code(language));
    let Some(list) = list else {
        return tr(language, "calendar.not_available");
    };
    if list.is_empty() {
        return tr(language, "calendar.not_available");
    }
    let Some(epoch) = NaiveDate::from_ymd_opt(1970, 1, 1) else {
        return list[0].clone();
    };
    let days = date.signed_duration_since(epoch).num_days();
    let index = days.rem_euclid(list.len() as i64) as usize;
    list.get(index)
        .cloned()
        .unwrap_or_else(|| tr(language, "calendar.not_available"))
}

fn holiday_for_date(language: Language, date: NaiveDate) -> Option<String> {
    let day = date.day();
    let month = date.month();
    let value = match language {
        Language::Italian => match (day, month) {
            (1, 1) => "Capodanno",
            (6, 1) => "Epifania",
            (25, 4) => "Festa della Liberazione",
            (1, 5) => "Festa dei Lavoratori",
            (2, 6) => "Festa della Repubblica",
            (15, 8) => "Ferragosto",
            (1, 11) => "Tutti i Santi",
            (8, 12) => "Immacolata Concezione",
            (25, 12) => "Natale",
            (26, 12) => "Santo Stefano",
            _ => return None,
        },
        Language::Portuguese => match (day, month) {
            (1, 1) => "Ano Novo",
            (6, 1) => "Epifania",
            (25, 4) => "Dia da Liberdade",
            (1, 5) => "Dia do Trabalhador",
            (10, 6) => "Dia de Portugal",
            (15, 8) => "Assunção de Nossa Senhora",
            (1, 11) => "Todos os Santos",
            (8, 12) => "Imaculada Conceição",
            (25, 12) => "Natal",
            _ => return None,
        },
        Language::PortugueseBrazilian => match (day, month) {
            (1, 1) => "Confraternização Universal",
            (21, 4) => "Tiradentes",
            (1, 5) => "Dia do Trabalho",
            (7, 9) => "Independência do Brasil",
            (12, 10) => "Nossa Senhora Aparecida",
            (2, 11) => "Finados",
            (15, 11) => "Proclamação da República",
            (20, 11) => "Dia Nacional de Zumbi e da Consciência Negra",
            (25, 12) => "Natal",
            _ => return None,
        },
        Language::Polish => match (day, month) {
            (1, 1) => "Nowy Rok",
            (6, 1) => "Święto Trzech Króli",
            (1, 5) => "Święto Pracy",
            (3, 5) => "Święto Konstytucji 3 Maja",
            (15, 8) => "Wniebowzięcie Najświętszej Maryi Panny",
            (1, 11) => "Wszystkich Świętych",
            (11, 11) => "Narodowe Święto Niepodległości",
            (25, 12) => "Boże Narodzenie",
            (26, 12) => "Drugi dzień Świąt Bożego Narodzenia",
            _ => return None,
        },
        Language::German => match (day, month) {
            (1, 1) => "Neujahr",
            (1, 5) => "Tag der Arbeit",
            (3, 10) => "Tag der Deutschen Einheit",
            (25, 12) => "Erster Weihnachtstag",
            (26, 12) => "Zweiter Weihnachtstag",
            _ => return None,
        },
        Language::Czech => match (day, month) {
            (1, 1) => "Nový rok",
            (1, 5) => "Svátek práce",
            (8, 5) => "Den vítězství",
            (5, 7) => "Den slovanských věrozvěstů Cyrila a Metoděje",
            (6, 7) => "Den upálení mistra Jana Husa",
            (28, 9) => "Den české státnosti",
            (28, 10) => "Den vzniku samostatného československého státu",
            (17, 11) => "Den boje za svobodu a demokracii",
            (24, 12) => "Štědrý den",
            (25, 12) => "1. svátek vánoční",
            (26, 12) => "2. svátek vánoční",
            _ => return None,
        },
        _ => return None,
    };
    Some(value.to_string())
}

fn localized_date(language: Language, date: NaiveDate) -> String {
    let (weekdays, months) = localized_date_names(language);
    let weekday_index = match date.weekday() {
        Weekday::Mon => 0,
        Weekday::Tue => 1,
        Weekday::Wed => 2,
        Weekday::Thu => 3,
        Weekday::Fri => 4,
        Weekday::Sat => 5,
        Weekday::Sun => 6,
    };
    let month_index = date.month0() as usize;
    format!(
        "{} {} {} {}",
        weekdays[weekday_index],
        date.day(),
        months[month_index],
        date.year()
    )
}

fn localized_date_names(
    language: Language,
) -> (&'static [&'static str; 7], &'static [&'static str; 12]) {
    match language {
        Language::Italian => (&WEEKDAYS_IT, &MONTHS_IT),
        Language::Spanish => (&WEEKDAYS_ES, &MONTHS_ES),
        Language::Portuguese | Language::PortugueseBrazilian => (&WEEKDAYS_PT, &MONTHS_PT),
        Language::Swedish => (&WEEKDAYS_SV, &MONTHS_SV),
        Language::Vietnamese => (&WEEKDAYS_VI, &MONTHS_VI),
        Language::Czech => (&WEEKDAYS_CS, &MONTHS_CS),
        Language::Polish => (&WEEKDAYS_PL, &MONTHS_PL),
        Language::French => (&WEEKDAYS_FR, &MONTHS_FR),
        Language::Serbian => (&WEEKDAYS_SR, &MONTHS_SR),
        Language::Ukrainian => (&WEEKDAYS_UK, &MONTHS_UK),
        Language::Lithuanian => (&WEEKDAYS_LT, &MONTHS_LT),
        Language::Russian => (&WEEKDAYS_RU, &MONTHS_RU),
        Language::Chinese => (&WEEKDAYS_ZH, &MONTHS_ZH),
        Language::Hindi => (&WEEKDAYS_HI, &MONTHS_HI),
        Language::Indonesian => (&WEEKDAYS_ID, &MONTHS_ID),
        Language::German => (&WEEKDAYS_DE, &MONTHS_DE),
        Language::English => (&WEEKDAYS_EN, &MONTHS_EN),
    }
}

fn language_code(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::Spanish => "es",
        Language::Portuguese => "pt",
        Language::PortugueseBrazilian => "pt-BR",
        Language::Czech => "cs",
        Language::Polish => "pl",
        Language::French => "fr",
        Language::German => "de",
        Language::English => "en",
        Language::Swedish => "sv",
        Language::Vietnamese => "vi",
        Language::Serbian => "sr",
        Language::Ukrainian => "uk",
        Language::Lithuanian => "lt",
        Language::Russian => "ru",
        Language::Chinese => "zh",
        Language::Hindi => "hi",
        Language::Indonesian => "id",
    }
}

fn dates_between(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    let mut dates = Vec::new();
    let mut current = start;
    while current <= end {
        dates.push(current);
        current += Duration::days(1);
    }
    dates
}

fn with_calendar_state<T>(
    hwnd: HWND,
    callback: impl FnOnce(&mut CalendarWindowState) -> T,
) -> Option<T> {
    let pointer =
        crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut CalendarWindowState;
    if pointer.is_null() {
        return None;
    }
    unsafe { Some(callback(&mut *pointer)) }
}

fn with_reminder_data<T>(
    hwnd: HWND,
    callback: impl FnOnce(&mut ReminderDialogData) -> T,
) -> Option<T> {
    let pointer = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut ReminderDialogData;
    if pointer.is_null() {
        return None;
    }
    unsafe { Some(callback(&mut *pointer)) }
}

fn create_static(parent: HWND, text: &str, id: usize) -> HWND {
    let wide = to_wide(text);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            PCWSTR(wide.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            100,
            24,
            parent,
            HMENU(id as isize),
            HINSTANCE(0),
            None,
        )
    }
}

fn create_listbox(parent: HWND, id: usize) -> HWND {
    unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("LISTBOX"),
            PCWSTR::null(),
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | WS_VSCROLL
                | WINDOW_STYLE(LBS_NOTIFY as u32 | LBS_HASSTRINGS as u32),
            0,
            0,
            100,
            100,
            parent,
            HMENU(id as isize),
            HINSTANCE(0),
            None,
        )
    }
}

fn create_button(parent: HWND, text: &str, id: usize, default: bool) -> HWND {
    let wide = to_wide(text);
    let style = if default {
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32)
    } else {
        WS_CHILD | WS_VISIBLE | WS_TABSTOP
    };
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            PCWSTR(wide.as_ptr()),
            style,
            0,
            0,
            100,
            30,
            parent,
            HMENU(id as isize),
            HINSTANCE(0),
            None,
        )
    }
}

fn create_checkbox(parent: HWND, text: &str, id: usize) -> HWND {
    let wide = to_wide(text);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            PCWSTR(wide.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            0,
            0,
            100,
            28,
            parent,
            HMENU(id as isize),
            HINSTANCE(0),
            None,
        )
    }
}

fn create_edit(parent: HWND, id: usize) -> HWND {
    unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            0,
            0,
            100,
            30,
            parent,
            HMENU(id as isize),
            HINSTANCE(0),
            None,
        )
    }
}

fn create_combo(parent: HWND, id: usize) -> HWND {
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("COMBOBOX"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            0,
            0,
            100,
            200,
            parent,
            HMENU(id as isize),
            HINSTANCE(0),
            None,
        )
    }
}

fn add_list_string(list: HWND, text: &str) {
    let wide = to_wide(text);
    crate::send_message_w_safe(
        list,
        LB_ADDSTRING,
        WPARAM(0),
        LPARAM(wide.as_ptr() as isize),
    );
}

fn add_combo_string(combo: HWND, text: &str) {
    let wide = to_wide(text);
    crate::send_message_w_safe(
        combo,
        CB_ADDSTRING,
        WPARAM(0),
        LPARAM(wide.as_ptr() as isize),
    );
}

fn selected_list_index(list: HWND) -> Option<usize> {
    let value = crate::send_message_w_safe(list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if value < 0 {
        None
    } else {
        Some(value as usize)
    }
}

fn selected_combo_index(combo: HWND) -> Option<usize> {
    let value = crate::send_message_w_safe(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if value < 0 {
        None
    } else {
        Some(value as usize)
    }
}

fn get_control_text(control: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(control);
        if length <= 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let copied = GetWindowTextW(control, &mut buffer);
        String::from_utf16_lossy(&buffer[..copied as usize])
    }
}

fn set_window_text(hwnd: HWND, text: &str) {
    let wide = to_wide(text);
    unsafe {
        crate::log_if_err!(SetWindowTextW(hwnd, PCWSTR(wide.as_ptr())));
    }
}

fn show_control(control: HWND, visible: bool) {
    unsafe {
        ShowWindow(control, if visible { SW_SHOW } else { SW_HIDE });
    }
}

fn is_checked(control: HWND) -> bool {
    crate::send_message_w_safe(control, BM_GETCHECK, WPARAM(0), LPARAM(0)).0
        == BST_CHECKED.0 as isize
}

fn focus_calendar_list(hwnd: HWND) {
    if let Some(list) = with_calendar_state(hwnd, |state| state.list) {
        unsafe {
            SetForegroundWindow(hwnd);
        }
        crate::set_focus_safe(list);
        notify_focus(list);
    }
}

fn notify_focus(control: HWND) {
    unsafe {
        NotifyWinEvent(
            EVENT_OBJECT_FOCUS,
            control,
            OBJID_CLIENT.0,
            windows::Win32::UI::WindowsAndMessaging::CHILDID_SELF as i32,
        );
    }
}

fn show_question(owner: HWND, title: &str, message: &str) -> bool {
    let title_wide = to_wide(title);
    let message_wide = to_wide(message);
    unsafe {
        MessageBoxW(
            owner,
            PCWSTR(message_wide.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            MB_YESNO | MB_ICONQUESTION,
        ) == IDYES
    }
}

fn show_warning_message(owner: HWND, language: Language, message: &str) {
    let title = to_wide(&i18n::tr(language, "app.warning_title"));
    let message = to_wide(message);
    unsafe {
        MessageBoxW(
            owner,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONWARNING,
        );
    }
}

fn show_error_message(owner: HWND, language: Language, message: &str) {
    let title = to_wide(&i18n::tr(language, "app.error_title"));
    let message = to_wide(message);
    unsafe {
        MessageBoxW(
            owner,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn copy_text_to_clipboard(hwnd: HWND, text: &str) {
    const CF_UNICODETEXT: u32 = 13;
    let content = to_wide(text);
    if content.is_empty() || crate::open_clipboard_safe(hwnd).is_err() {
        return;
    }
    if let Err(error) = crate::empty_clipboard_safe() {
        crate::log_debug(&format!("Calendar EmptyClipboard failed: {error}"));
    }
    let size = content.len() * std::mem::size_of::<u16>();
    let handle = match crate::global_alloc_safe(GMEM_MOVEABLE, size) {
        Ok(handle) => handle,
        Err(error) => {
            crate::log_debug(&format!("Calendar GlobalAlloc failed: {error}"));
            crate::log_if_err!(crate::close_clipboard_safe());
            return;
        }
    };
    if handle.0.is_null() {
        crate::log_if_err!(crate::close_clipboard_safe());
        return;
    }
    let pointer = crate::global_lock_as_safe(handle) as *mut u16;
    if pointer.is_null() {
        crate::log_if_err!(crate::close_clipboard_safe());
        return;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(content.as_ptr(), pointer, content.len());
    }
    crate::log_if_err!(crate::global_unlock_safe(handle));
    if let Err(error) = crate::set_clipboard_data_safe(CF_UNICODETEXT, HANDLE(handle.0 as isize)) {
        crate::log_debug(&format!("Calendar SetClipboardData failed: {error}"));
    }
    crate::log_if_err!(crate::close_clipboard_safe());
}

fn tr(language: Language, key: &str) -> String {
    i18n::tr(language, key)
}

const WEEKDAYS_DE: [&str; 7] = [
    "Montag",
    "Dienstag",
    "Mittwoch",
    "Donnerstag",
    "Freitag",
    "Samstag",
    "Sonntag",
];
const MONTHS_DE: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];
const WEEKDAYS_ID: [&str; 7] = [
    "Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu", "Minggu",
];
const MONTHS_ID: [&str; 12] = [
    "Januari",
    "Februari",
    "Maret",
    "April",
    "Mei",
    "Juni",
    "Juli",
    "Agustus",
    "September",
    "Oktober",
    "November",
    "Desember",
];
const WEEKDAYS_EN: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS_IT: [&str; 7] = [
    "Lunedì",
    "Martedì",
    "Mercoledì",
    "Giovedì",
    "Venerdì",
    "Sabato",
    "Domenica",
];
const MONTHS_IT: [&str; 12] = [
    "gennaio",
    "febbraio",
    "marzo",
    "aprile",
    "maggio",
    "giugno",
    "luglio",
    "agosto",
    "settembre",
    "ottobre",
    "novembre",
    "dicembre",
];
const WEEKDAYS_ES: [&str; 7] = [
    "Lunes",
    "Martes",
    "Miércoles",
    "Jueves",
    "Viernes",
    "Sábado",
    "Domingo",
];
const MONTHS_ES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];
const WEEKDAYS_PT: [&str; 7] = [
    "Segunda-feira",
    "Terça-feira",
    "Quarta-feira",
    "Quinta-feira",
    "Sexta-feira",
    "Sábado",
    "Domingo",
];
const MONTHS_PT: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];
const WEEKDAYS_SV: [&str; 7] = [
    "Måndag", "Tisdag", "Onsdag", "Torsdag", "Fredag", "Lördag", "Söndag",
];
const MONTHS_SV: [&str; 12] = [
    "januari",
    "februari",
    "mars",
    "april",
    "maj",
    "juni",
    "juli",
    "augusti",
    "september",
    "oktober",
    "november",
    "december",
];
const WEEKDAYS_VI: [&str; 7] = [
    "Thứ Hai",
    "Thứ Ba",
    "Thứ Tư",
    "Thứ Năm",
    "Thứ Sáu",
    "Thứ Bảy",
    "Chủ Nhật",
];
const MONTHS_VI: [&str; 12] = [
    "tháng 1",
    "tháng 2",
    "tháng 3",
    "tháng 4",
    "tháng 5",
    "tháng 6",
    "tháng 7",
    "tháng 8",
    "tháng 9",
    "tháng 10",
    "tháng 11",
    "tháng 12",
];
const WEEKDAYS_CS: [&str; 7] = [
    "Pondělí",
    "Úterý",
    "Středa",
    "Čtvrtek",
    "Pátek",
    "Sobota",
    "Neděle",
];
const MONTHS_CS: [&str; 12] = [
    "ledna",
    "února",
    "března",
    "dubna",
    "května",
    "června",
    "července",
    "srpna",
    "září",
    "října",
    "listopadu",
    "prosince",
];
const WEEKDAYS_PL: [&str; 7] = [
    "Poniedziałek",
    "Wtorek",
    "Środa",
    "Czwartek",
    "Piątek",
    "Sobota",
    "Niedziela",
];
const MONTHS_PL: [&str; 12] = [
    "stycznia",
    "lutego",
    "marca",
    "kwietnia",
    "maja",
    "czerwca",
    "lipca",
    "sierpnia",
    "września",
    "października",
    "listopada",
    "grudnia",
];
const WEEKDAYS_FR: [&str; 7] = [
    "Lundi", "Mardi", "Mercredi", "Jeudi", "Vendredi", "Samedi", "Dimanche",
];
const MONTHS_FR: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];
const WEEKDAYS_SR: [&str; 7] = [
    "Ponedeljak",
    "Utorak",
    "Sreda",
    "Četvrtak",
    "Petak",
    "Subota",
    "Nedelja",
];
const MONTHS_SR: [&str; 12] = [
    "januar",
    "februar",
    "mart",
    "april",
    "maj",
    "jun",
    "jul",
    "avgust",
    "septembar",
    "oktobar",
    "novembar",
    "decembar",
];
const WEEKDAYS_UK: [&str; 7] = [
    "Понеділок",
    "Вівторок",
    "Середа",
    "Четвер",
    "П’ятниця",
    "Субота",
    "Неділя",
];
const MONTHS_UK: [&str; 12] = [
    "січня",
    "лютого",
    "березня",
    "квітня",
    "травня",
    "червня",
    "липня",
    "серпня",
    "вересня",
    "жовтня",
    "листопада",
    "грудня",
];
const WEEKDAYS_LT: [&str; 7] = [
    "Pirmadienis",
    "Antradienis",
    "Trečiadienis",
    "Ketvirtadienis",
    "Penktadienis",
    "Šeštadienis",
    "Sekmadienis",
];
const MONTHS_LT: [&str; 12] = [
    "sausio",
    "vasario",
    "kovo",
    "balandžio",
    "gegužės",
    "birželio",
    "liepos",
    "rugpjūčio",
    "rugsėjo",
    "spalio",
    "lapkričio",
    "gruodžio",
];
const WEEKDAYS_RU: [&str; 7] = [
    "Понедельник",
    "Вторник",
    "Среда",
    "Четверг",
    "Пятница",
    "Суббота",
    "Воскресенье",
];
const MONTHS_RU: [&str; 12] = [
    "января",
    "февраля",
    "марта",
    "апреля",
    "мая",
    "июня",
    "июля",
    "августа",
    "сентября",
    "октября",
    "ноября",
    "декабря",
];
const WEEKDAYS_ZH: [&str; 7] = [
    "星期一",
    "星期二",
    "星期三",
    "星期四",
    "星期五",
    "星期六",
    "星期日",
];
const MONTHS_ZH: [&str; 12] = [
    "一月",
    "二月",
    "三月",
    "四月",
    "五月",
    "六月",
    "七月",
    "八月",
    "九月",
    "十月",
    "十一月",
    "十二月",
];
const WEEKDAYS_HI: [&str; 7] = [
    "सोमवार",
    "मंगलवार",
    "बुधवार",
    "गुरुवार",
    "शुक्रवार",
    "शनिवार",
    "रविवार",
];
const MONTHS_HI: [&str; 12] = [
    "जनवरी",
    "फ़रवरी",
    "मार्च",
    "अप्रैल",
    "मई",
    "जून",
    "जुलाई",
    "अगस्त",
    "सितंबर",
    "अक्टूबर",
    "नवंबर",
    "दिसंबर",
];
