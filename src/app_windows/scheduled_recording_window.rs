use std::fs;
use std::io::Write;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, DEFAULT_GUI_FONT, HBRUSH, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_LEFT, VK_RIGHT};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, CBS_DROPDOWNLIST, CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW,
    DefWindowProcW, DestroyWindow, DispatchMessageW, ES_AUTOHSCROLL, ES_NUMBER, GWLP_USERDATA,
    GetClientRect, GetDlgItem, GetMessageW, GetWindowLongPtrW, HMENU, IDC_ARROW, IsDialogMessageW,
    IsWindow, LoadCursorW, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK,
    MESSAGEBOX_STYLE, MSG, MoveWindow, RegisterClassW, SW_SHOW, SetForegroundWindow,
    SetWindowLongPtrW, ShowWindow, TranslateMessage, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY,
    WM_KEYDOWN, WM_NCDESTROY, WM_SETFONT, WM_SIZE, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD,
    WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_POPUP, WS_SYSMENU, WS_TABSTOP,
    WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

use crate::accessibility::to_wide;
use crate::i18n;
use crate::settings::{Language, RadioFavorite, load_settings, settings_dir};
use crate::stream_recording::{self, StreamRecordingKind};
use crate::tools::tv::{self, TvChannel};

const CLASS_NAME: &str = "SonarpadScheduledRecordingWindow";
const CREATE_NO_WINDOW_FLAG: u32 = 0x0800_0000;

const ID_LABEL_SOURCE: usize = 5101;
const ID_LABEL_DAY: usize = 5102;
const ID_COMBO_DAY: usize = 5103;
const ID_LABEL_HOUR: usize = 5104;
const ID_COMBO_HOUR: usize = 5105;
const ID_LABEL_MINUTE: usize = 5106;
const ID_COMBO_MINUTE: usize = 5107;
const ID_LABEL_RECURRENCE: usize = 5108;
const ID_COMBO_RECURRENCE: usize = 5109;
const ID_LABEL_DURATION: usize = 5110;
const ID_COMBO_DURATION: usize = 5111;
const ID_LABEL_CUSTOM_DURATION: usize = 5112;
const ID_EDIT_CUSTOM_DURATION: usize = 5113;
const ID_BUTTON_SAVE: usize = 5114;
const ID_BUTTON_CANCEL: usize = 5115;

const CB_ADDSTRING: u32 = 0x0143;
const CB_GETCURSEL: u32 = 0x0147;
const CB_SETCURSEL: u32 = 0x014E;
const CBN_SELCHANGE: u16 = 1;

const DEFAULT_DURATION_MINUTES: u32 = 60;
const DURATION_VALUES: [u32; 14] = [1, 5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 360, 480];

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
enum RecordingRecurrence {
    Once,
    Daily,
    Weekly,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum ScheduledRecordingSource {
    Radio { name: String, url: String },
    Tv { channel: Box<TvChannel> },
}

#[derive(Clone, Serialize, Deserialize)]
struct ScheduledRecording {
    id: String,
    source: ScheduledRecordingSource,
    language: Language,
    start_date: String,
    hour: u32,
    minute: u32,
    duration_minutes: u32,
    recurrence: RecordingRecurrence,
}

#[derive(Clone, Debug)]
pub(crate) struct ScheduledRecordingListItem {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) start_at: NaiveDateTime,
    pub(crate) duration_minutes: u32,
}

struct ScheduleDialogState {
    parent: HWND,
    language: Language,
    source: ScheduledRecordingSource,
    source_name: String,
    dates: Vec<NaiveDate>,
    combo_day: HWND,
    combo_hour: HWND,
    combo_minute: HWND,
    combo_recurrence: HWND,
    combo_duration: HWND,
    edit_custom_duration: HWND,
}

pub(crate) fn open_for_tv(parent: HWND, channel: TvChannel) {
    show_dialog(
        parent,
        Language::Italian,
        channel.name.clone(),
        ScheduledRecordingSource::Tv {
            channel: Box::new(channel),
        },
    );
}

pub(crate) fn open_for_radio(parent: HWND, language: Language, station: RadioFavorite) {
    show_dialog(
        parent,
        language,
        station.name.clone(),
        ScheduledRecordingSource::Radio {
            name: station.name,
            url: station.stream_url,
        },
    );
}

fn show_dialog(
    parent: HWND,
    language: Language,
    source_name: String,
    source: ScheduledRecordingSource,
) {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(CLASS_NAME);
        let window_class = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&window_class);

        let mut state = Box::new(ScheduleDialogState {
            parent,
            language,
            source,
            source_name,
            dates: Vec::new(),
            combo_day: HWND(0),
            combo_hour: HWND(0),
            combo_minute: HWND(0),
            combo_recurrence: HWND(0),
            combo_duration: HWND(0),
            edit_custom_duration: HWND(0),
        });
        let pointer = state.as_mut() as *mut ScheduleDialogState;
        let title = to_wide(&tr(language, "scheduled_recording.title"));
        crate::enable_window_safe(parent, false);
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            720,
            570,
            parent,
            HMENU(0),
            hinstance,
            Some(pointer.cast()),
        );
        if hwnd.0 == 0 {
            crate::enable_window_safe(parent, true);
            return;
        }
        let _owned_by_window = Box::into_raw(state);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);

        let mut message = MSG::default();
        while IsWindow(hwnd).as_bool() && GetMessageW(&mut message, HWND(0), 0, 0).as_bool() {
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&message) {
                continue;
            }
            let key = message.wParam.0 as u16;
            if message.message == WM_KEYDOWN
                && message.hwnd == with_state(hwnd, |state| state.combo_minute).unwrap_or(HWND(0))
                && (key == VK_LEFT.0 || key == VK_RIGHT.0)
            {
                crate::app_windows::calendar_window::adjust_minute_combo_by_five(
                    with_state(hwnd, |state| state.combo_minute).unwrap_or(HWND(0)),
                    key == VK_RIGHT.0,
                );
                continue;
            }
            if message.message == WM_KEYDOWN && key == VK_ESCAPE.0 {
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
    }
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "scheduled_recording_window_wndproc",
            || DefWindowProcW(hwnd, message, wparam, lparam),
            || wndproc_inner(hwnd, message, wparam, lparam),
        )
    }
}

fn wndproc_inner(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_CREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                if create.is_null() {
                    return LRESULT(-1);
                }
                let state = (*create).lpCreateParams as *mut ScheduleDialogState;
                if state.is_null() {
                    return LRESULT(-1);
                }
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
                create_controls(hwnd, &mut *state);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let notification = (wparam.0 >> 16) as u16;
                if id == ID_COMBO_DURATION && notification == CBN_SELCHANGE {
                    update_custom_duration_enabled(hwnd);
                    return LRESULT(0);
                }
                if id == ID_BUTTON_SAVE {
                    save_from_dialog(hwnd);
                    return LRESULT(0);
                }
                if id == ID_BUTTON_CANCEL {
                    crate::log_if_err!(DestroyWindow(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_SIZE => {
                layout(hwnd);
                LRESULT(0)
            }
            WM_CLOSE => {
                crate::log_if_err!(DestroyWindow(hwnd));
                LRESULT(0)
            }
            WM_DESTROY => {
                if let Some(parent) = with_state(hwnd, |state| state.parent) {
                    crate::enable_window_safe(parent, true);
                    SetForegroundWindow(parent);
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ScheduleDialogState;
                if !pointer.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    let _state = Box::from_raw(pointer);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

unsafe fn create_controls(hwnd: HWND, state: &mut ScheduleDialogState) {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let font = HFONT(crate::get_stock_object_safe(DEFAULT_GUI_FONT).0);
        create_control(
            hwnd,
            hinstance,
            w!("STATIC"),
            &format!(
                "{}: {}",
                tr(state.language, "scheduled_recording.source"),
                state.source_name
            ),
            (WS_CHILD | WS_VISIBLE, 0),
            ID_LABEL_SOURCE,
            font,
        );
        create_control(
            hwnd,
            hinstance,
            w!("STATIC"),
            &tr(state.language, "scheduled_recording.day"),
            (WS_CHILD | WS_VISIBLE, 0),
            ID_LABEL_DAY,
            font,
        );
        state.combo_day = create_control(
            hwnd,
            hinstance,
            w!("COMBOBOX"),
            "",
            (
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_BORDER
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                        CBS_DROPDOWNLIST as u32,
                    ),
                0,
            ),
            ID_COMBO_DAY,
            font,
        );
        create_control(
            hwnd,
            hinstance,
            w!("STATIC"),
            &tr(state.language, "scheduled_recording.hour"),
            (WS_CHILD | WS_VISIBLE, 0),
            ID_LABEL_HOUR,
            font,
        );
        state.combo_hour = create_control(
            hwnd,
            hinstance,
            w!("COMBOBOX"),
            "",
            (
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_BORDER
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                        CBS_DROPDOWNLIST as u32,
                    ),
                0,
            ),
            ID_COMBO_HOUR,
            font,
        );
        create_control(
            hwnd,
            hinstance,
            w!("STATIC"),
            &tr(state.language, "scheduled_recording.minute"),
            (WS_CHILD | WS_VISIBLE, 0),
            ID_LABEL_MINUTE,
            font,
        );
        state.combo_minute = create_control(
            hwnd,
            hinstance,
            w!("COMBOBOX"),
            "",
            (
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_BORDER
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                        CBS_DROPDOWNLIST as u32,
                    ),
                0,
            ),
            ID_COMBO_MINUTE,
            font,
        );
        create_control(
            hwnd,
            hinstance,
            w!("STATIC"),
            &tr(state.language, "scheduled_recording.recurrence"),
            (WS_CHILD | WS_VISIBLE, 0),
            ID_LABEL_RECURRENCE,
            font,
        );
        state.combo_recurrence = create_control(
            hwnd,
            hinstance,
            w!("COMBOBOX"),
            "",
            (
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_BORDER
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                        CBS_DROPDOWNLIST as u32,
                    ),
                0,
            ),
            ID_COMBO_RECURRENCE,
            font,
        );
        create_control(
            hwnd,
            hinstance,
            w!("STATIC"),
            &tr(state.language, "scheduled_recording.duration"),
            (WS_CHILD | WS_VISIBLE, 0),
            ID_LABEL_DURATION,
            font,
        );
        state.combo_duration = create_control(
            hwnd,
            hinstance,
            w!("COMBOBOX"),
            "",
            (
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_BORDER
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                        CBS_DROPDOWNLIST as u32,
                    ),
                0,
            ),
            ID_COMBO_DURATION,
            font,
        );
        create_control(
            hwnd,
            hinstance,
            w!("STATIC"),
            &tr(state.language, "scheduled_recording.custom_duration"),
            (WS_CHILD | WS_VISIBLE, 0),
            ID_LABEL_CUSTOM_DURATION,
            font,
        );
        state.edit_custom_duration = create_control(
            hwnd,
            hinstance,
            w!("EDIT"),
            "60",
            (
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_BORDER
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(ES_AUTOHSCROLL as u32)
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(ES_NUMBER as u32),
                WS_EX_CLIENTEDGE.0,
            ),
            ID_EDIT_CUSTOM_DURATION,
            font,
        );
        create_control(
            hwnd,
            hinstance,
            w!("BUTTON"),
            &tr(state.language, "scheduled_recording.save"),
            (
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                        BS_DEFPUSHBUTTON as u32,
                    ),
                0,
            ),
            ID_BUTTON_SAVE,
            font,
        );
        create_control(
            hwnd,
            hinstance,
            w!("BUTTON"),
            &tr(state.language, "scheduled_recording.cancel"),
            (WS_CHILD | WS_VISIBLE | WS_TABSTOP, 0),
            ID_BUTTON_CANCEL,
            font,
        );

        let now = Local::now();
        let today = now.date_naive();
        let minutes_to_next_slot = 5 - (now.minute() % 5);
        let next_slot = now + Duration::minutes(i64::from(minutes_to_next_slot));
        state.dates = (0..61)
            .map(|offset| today + Duration::days(offset))
            .collect();
        for date in &state.dates {
            combo_add(
                state.combo_day,
                &localized_date(state.language, *date, today),
            );
        }
        let default_day_index = state
            .dates
            .iter()
            .position(|date| *date == next_slot.date_naive())
            .unwrap_or(0);
        crate::send_message_w_safe(
            state.combo_day,
            CB_SETCURSEL,
            WPARAM(default_day_index),
            LPARAM(0),
        );
        for hour in 0..24 {
            combo_add(state.combo_hour, &format!("{hour:02}"));
        }
        crate::send_message_w_safe(
            state.combo_hour,
            CB_SETCURSEL,
            WPARAM(next_slot.hour() as usize),
            LPARAM(0),
        );
        for minute in 0..60 {
            combo_add(state.combo_minute, &format!("{minute:02}"));
        }
        let minute_index = next_slot.minute() as usize;
        crate::send_message_w_safe(
            state.combo_minute,
            CB_SETCURSEL,
            WPARAM(minute_index),
            LPARAM(0),
        );
        for key in [
            "scheduled_recording.once",
            "scheduled_recording.daily",
            "scheduled_recording.weekly",
        ] {
            combo_add(state.combo_recurrence, &tr(state.language, key));
        }
        crate::send_message_w_safe(state.combo_recurrence, CB_SETCURSEL, WPARAM(0), LPARAM(0));
        for minutes in DURATION_VALUES {
            combo_add(
                state.combo_duration,
                &i18n::tr_f(
                    state.language,
                    "scheduled_recording.minutes",
                    &[("minutes", &minutes.to_string())],
                ),
            );
        }
        combo_add(
            state.combo_duration,
            &tr(state.language, "scheduled_recording.custom"),
        );
        let default_duration_index = DURATION_VALUES
            .iter()
            .position(|&value| value == DEFAULT_DURATION_MINUTES)
            .unwrap_or(0);
        crate::send_message_w_safe(
            state.combo_duration,
            CB_SETCURSEL,
            WPARAM(default_duration_index),
            LPARAM(0),
        );
        update_custom_duration_enabled(hwnd);
        layout(hwnd);
        crate::set_focus_safe(state.combo_day);
    }
}

unsafe fn create_control(
    parent: HWND,
    hinstance: HINSTANCE,
    class_name: PCWSTR,
    text: &str,
    styles: (windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE, u32),
    id: usize,
    font: HFONT,
) -> HWND {
    unsafe {
        let wide = to_wide(text);
        let (style, ex_style) = styles;
        let control = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(ex_style),
            class_name,
            PCWSTR(wide.as_ptr()),
            style,
            0,
            0,
            0,
            0,
            parent,
            HMENU(id as isize),
            hinstance,
            None,
        );
        crate::send_message_w_safe(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        control
    }
}

fn combo_add(combo: HWND, text: &str) {
    let wide = to_wide(text);
    crate::send_message_w_safe(
        combo,
        CB_ADDSTRING,
        WPARAM(0),
        LPARAM(wide.as_ptr() as isize),
    );
}

fn update_custom_duration_enabled(hwnd: HWND) {
    let Some((combo, edit)) = with_state(hwnd, |state| {
        (state.combo_duration, state.edit_custom_duration)
    }) else {
        return;
    };
    let selected = crate::send_message_w_safe(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    crate::enable_window_safe(edit, selected == DURATION_VALUES.len() as isize);
}

fn save_from_dialog(hwnd: HWND) {
    let Some(data) = with_state(hwnd, |state| {
        let date_index = combo_index(state.combo_day).unwrap_or(0);
        let date = state.dates.get(date_index).copied()?;
        let hour = combo_index(state.combo_hour).unwrap_or(0) as u32;
        let minute = combo_index(state.combo_minute).unwrap_or(0) as u32;
        let recurrence = match combo_index(state.combo_recurrence).unwrap_or(0) {
            1 => RecordingRecurrence::Daily,
            2 => RecordingRecurrence::Weekly,
            _ => RecordingRecurrence::Once,
        };
        let default_duration_index = DURATION_VALUES
            .iter()
            .position(|&value| value == DEFAULT_DURATION_MINUTES)
            .unwrap_or(0);
        let duration_index = combo_index(state.combo_duration).unwrap_or(default_duration_index);
        let duration = if let Some(value) = DURATION_VALUES.get(duration_index) {
            *value
        } else {
            get_control_text(state.edit_custom_duration)
                .trim()
                .parse::<u32>()
                .ok()?
        };
        Some((
            state.parent,
            state.language,
            state.source.clone(),
            date,
            hour,
            minute,
            recurrence,
            duration,
        ))
    })
    .flatten() else {
        let language = with_state(hwnd, |state| state.language).unwrap_or_default();
        show_message(
            hwnd,
            language,
            "scheduled_recording.invalid_duration",
            MB_ICONWARNING,
        );
        return;
    };
    let (parent, language, source, date, hour, minute, recurrence, duration) = data;
    if duration == 0 || duration > 1440 {
        show_message(
            hwnd,
            language,
            "scheduled_recording.invalid_duration",
            MB_ICONWARNING,
        );
        return;
    }
    let Some(mut start_at) = date.and_hms_opt(hour, minute, 0) else {
        show_message(
            hwnd,
            language,
            "scheduled_recording.invalid_time",
            MB_ICONWARNING,
        );
        return;
    };
    let now = Local::now().naive_local();
    match recurrence {
        RecordingRecurrence::Once if start_at <= now => {
            show_message(
                hwnd,
                language,
                "scheduled_recording.past_time",
                MB_ICONWARNING,
            );
            return;
        }
        RecordingRecurrence::Daily => {
            while start_at <= now {
                start_at += Duration::days(1);
            }
        }
        RecordingRecurrence::Weekly => {
            while start_at <= now {
                start_at += Duration::weeks(1);
            }
        }
        RecordingRecurrence::Once => {}
    }
    let scheduled_date = start_at.date();

    let schedule = ScheduledRecording {
        id: Uuid::new_v4().to_string(),
        source,
        language,
        start_date: scheduled_date.format("%Y-%m-%d").to_string(),
        hour,
        minute,
        duration_minutes: duration,
        recurrence,
    };
    let creation_result = (|| -> Result<(), String> {
        if matches!(&schedule.source, ScheduledRecordingSource::Tv { .. }) {
            let runtime_owner = if parent.0 != 0 { parent } else { hwnd };
            crate::ensure_mpv_runtime_available(runtime_owner)?;
        }
        save_schedule(&schedule)?;
        create_scheduled_task(&schedule, start_at)
    })();
    match creation_result {
        Ok(()) => {
            show_message(
                hwnd,
                language,
                "scheduled_recording.saved",
                MB_ICONINFORMATION,
            );
            crate::log_if_err!(unsafe { DestroyWindow(hwnd) });
            if parent.0 != 0 {
                crate::set_focus_safe(parent);
            }
        }
        Err(error) => {
            delete_task(&schedule.id);
            if let Err(remove_error) = fs::remove_file(schedule_path(&schedule.id))
                && remove_error.kind() != std::io::ErrorKind::NotFound
            {
                crate::log_debug(&format!(
                    "Scheduled recording rollback failed id={} error={remove_error}",
                    schedule.id
                ));
            }
            crate::log_debug(&format!("Scheduled recording creation failed: {error}"));
            let text = i18n::tr_f(
                language,
                "scheduled_recording.save_failed",
                &[("error", error.as_str())],
            );
            show_text(
                hwnd,
                &tr(language, "scheduled_recording.title"),
                &text,
                MB_ICONERROR,
            );
        }
    }
}

fn combo_index(combo: HWND) -> Option<usize> {
    let selected = crate::send_message_w_safe(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    (selected >= 0).then_some(selected as usize)
}

fn get_control_text(control: HWND) -> String {
    let length = crate::send_message_w_safe(
        control,
        windows::Win32::UI::WindowsAndMessaging::WM_GETTEXTLENGTH,
        WPARAM(0),
        LPARAM(0),
    )
    .0
    .max(0) as usize;
    let mut buffer = vec![0u16; length + 1];
    crate::send_message_w_safe(
        control,
        windows::Win32::UI::WindowsAndMessaging::WM_GETTEXT,
        WPARAM(buffer.len()),
        LPARAM(buffer.as_mut_ptr() as isize),
    );
    String::from_utf16_lossy(&buffer[..length])
}

fn save_schedule(schedule: &ScheduledRecording) -> Result<(), String> {
    let directory = schedules_dir();
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let path = schedule_path(&schedule.id);
    let payload = serde_json::to_vec_pretty(schedule).map_err(|error| error.to_string())?;
    fs::write(path, payload).map_err(|error| error.to_string())
}

fn load_schedule(id: &str) -> Result<ScheduledRecording, String> {
    let payload = fs::read(schedule_path(id)).map_err(|error| error.to_string())?;
    serde_json::from_slice(&payload).map_err(|error| error.to_string())
}

fn schedules_dir() -> PathBuf {
    settings_dir().join("ScheduledRecordings")
}

fn schedule_path(id: &str) -> PathBuf {
    schedules_dir().join(format!("{}.json", safe_task_component(id)))
}

pub(crate) fn list_scheduled_recordings(
    kind: StreamRecordingKind,
) -> Vec<ScheduledRecordingListItem> {
    let now = Local::now().naive_local();
    let Ok(entries) = fs::read_dir(schedules_dir()) else {
        return Vec::new();
    };
    let mut result = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                return None;
            }
            let payload = fs::read(path).ok()?;
            let schedule = serde_json::from_slice::<ScheduledRecording>(&payload).ok()?;
            let (source_kind, title) = match &schedule.source {
                ScheduledRecordingSource::Radio { name, .. } => {
                    (StreamRecordingKind::Radio, name.clone())
                }
                ScheduledRecordingSource::Tv { channel } => {
                    (StreamRecordingKind::Tv, channel.name.clone())
                }
            };
            if source_kind != kind {
                return None;
            }
            let start_at = next_occurrence(&schedule, now)?;
            Some(ScheduledRecordingListItem {
                id: schedule.id,
                title,
                start_at,
                duration_minutes: schedule.duration_minutes,
            })
        })
        .collect::<Vec<_>>();
    result.sort_by_key(|item| item.start_at);
    result
}

pub(crate) fn cancel_scheduled_recording(id: &str) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("Identificatore della registrazione programmata non valido.".to_string());
    }

    // Remove the Windows scheduled task first so a future occurrence cannot start
    // while the Sonarpad definition is being removed.
    delete_task_checked(id)?;
    let path = schedule_path(id);
    match fs::remove_file(&path) {
        Ok(()) => {
            crate::log_debug(&format!(
                "Scheduled recording cancelled id={id} definition={}",
                path.display()
            ));
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            crate::log_debug(&format!(
                "Scheduled recording cancellation found no definition id={id}; task deletion was still requested"
            ));
            Ok(())
        }
        Err(error) => Err(format!(
            "Impossibile rimuovere la registrazione programmata: {error}"
        )),
    }
}

fn next_occurrence(schedule: &ScheduledRecording, now: NaiveDateTime) -> Option<NaiveDateTime> {
    let date = NaiveDate::parse_from_str(&schedule.start_date, "%Y-%m-%d").ok()?;
    let mut start = date.and_hms_opt(schedule.hour, schedule.minute, 0)?;
    match schedule.recurrence {
        RecordingRecurrence::Once => Some(start),
        RecordingRecurrence::Daily => {
            while start < now {
                start += Duration::days(1);
            }
            Some(start)
        }
        RecordingRecurrence::Weekly => {
            while start < now {
                start += Duration::weeks(1);
            }
            Some(start)
        }
    }
}

fn create_scheduled_task(
    schedule: &ScheduledRecording,
    start_at: NaiveDateTime,
) -> Result<(), String> {
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
    let trigger = task_trigger_xml(schedule.recurrence, start_at);
    let executable_text = executable.to_string_lossy();
    let working_directory_text = working_directory.to_string_lossy();
    let xml = format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\n",
            "<Task version=\"1.4\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\r\n",
            "  <RegistrationInfo><Author>{author}</Author><Description>Sonarpad scheduled stream recording</Description></RegistrationInfo>\r\n",
            "  <Triggers>{trigger}</Triggers>\r\n",
            "  <Principals><Principal id=\"Author\"><UserId>{user}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>\r\n",
            "  <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><AllowHardTerminate>true</AllowHardTerminate><StartWhenAvailable>true</StartWhenAvailable><RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable><AllowStartOnDemand>true</AllowStartOnDemand><Enabled>true</Enabled><Hidden>false</Hidden><RunOnlyIfIdle>false</RunOnlyIfIdle><WakeToRun>false</WakeToRun><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><Priority>7</Priority></Settings>\r\n",
            "  <Actions Context=\"Author\"><Exec><Command>{command}</Command><Arguments>--scheduled-recording {argument}</Arguments><WorkingDirectory>{working}</WorkingDirectory></Exec></Actions>\r\n",
            "</Task>\r\n"
        ),
        author = xml_escape(&user_id),
        trigger = trigger,
        user = xml_escape(&user_id),
        command = xml_escape(executable_text.as_ref()),
        argument = xml_escape(&schedule.id),
        working = xml_escape(working_directory_text.as_ref()),
    );

    let task_dir = schedules_dir().join("Tasks");
    fs::create_dir_all(&task_dir).map_err(|error| error.to_string())?;
    let xml_path = task_dir.join(format!("{}.xml", safe_task_component(&schedule.id)));
    write_utf16_xml(&xml_path, &xml)?;
    let output = Command::new("schtasks.exe")
        .arg("/Create")
        .arg("/TN")
        .arg(task_name(&schedule.id))
        .arg("/XML")
        .arg(&xml_path)
        .arg("/F")
        .creation_flags(CREATE_NO_WINDOW_FLAG)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| error.to_string());
    if let Err(error) = fs::remove_file(&xml_path) {
        crate::log_debug(&format!(
            "Scheduled recording temporary XML removal failed path={} error={error}",
            xml_path.display()
        ));
    }
    let output = output?;
    if output.status.success() {
        crate::log_debug(&format!(
            "Scheduled recording task created id={} start={} recurrence={:?}",
            schedule.id, start_at, schedule.recurrence
        ));
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}

fn task_trigger_xml(recurrence: RecordingRecurrence, start_at: NaiveDateTime) -> String {
    let start = start_at.format("%Y-%m-%dT%H:%M:%S");
    match recurrence {
        RecordingRecurrence::Once => format!(
            "<TimeTrigger><StartBoundary>{start}</StartBoundary><Enabled>true</Enabled></TimeTrigger>"
        ),
        RecordingRecurrence::Daily => format!(
            "<CalendarTrigger><StartBoundary>{start}</StartBoundary><Enabled>true</Enabled><ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay></CalendarTrigger>"
        ),
        RecordingRecurrence::Weekly => {
            let day = match start_at.date().weekday() {
                chrono::Weekday::Mon => "Monday",
                chrono::Weekday::Tue => "Tuesday",
                chrono::Weekday::Wed => "Wednesday",
                chrono::Weekday::Thu => "Thursday",
                chrono::Weekday::Fri => "Friday",
                chrono::Weekday::Sat => "Saturday",
                chrono::Weekday::Sun => "Sunday",
            };
            format!(
                "<CalendarTrigger><StartBoundary>{start}</StartBoundary><Enabled>true</Enabled><ScheduleByWeek><WeeksInterval>1</WeeksInterval><DaysOfWeek><{day}/></DaysOfWeek></ScheduleByWeek></CalendarTrigger>"
            )
        }
    }
}

fn delete_task_checked(id: &str) -> Result<(), String> {
    let output = Command::new("schtasks.exe")
        .arg("/Delete")
        .arg("/TN")
        .arg(task_name(id))
        .arg("/F")
        .creation_flags(CREATE_NO_WINDOW_FLAG)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        return Ok(());
    }

    // If the task is already gone, cancellation is complete from Windows' point
    // of view. Query it to distinguish that harmless case from a real delete
    // failure (for example an access/Task Scheduler error).
    let query = Command::new("schtasks.exe")
        .arg("/Query")
        .arg("/TN")
        .arg(task_name(id))
        .creation_flags(CREATE_NO_WINDOW_FLAG)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if query.is_ok_and(|status| !status.success()) {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(if stderr.is_empty() { stdout } else { stderr })
}

fn delete_task(id: &str) {
    if let Err(error) = delete_task_checked(id) {
        crate::log_debug(&format!(
            "Scheduled recording task deletion failed id={id} error={error}"
        ));
    }
}

fn task_name(id: &str) -> String {
    format!("Sonarpad Scheduled Recording {}", safe_task_component(id))
}

fn safe_task_component(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .collect()
}

fn write_utf16_xml(path: &Path, text: &str) -> Result<(), String> {
    let mut file = fs::File::create(path).map_err(|error| error.to_string())?;
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

fn maybe_open_audio_description_after_scheduled_tv_recording(path: &Path) {
    let settings = load_settings();
    if settings.language != Language::Italian || !settings.audio_description_after_tv_recording {
        return;
    }
    let Ok(executable) = std::env::current_exe() else {
        crate::log_debug(
            "Scheduled TV recording: unable to resolve Sonarpad executable for audio-description handoff",
        );
        return;
    };
    match Command::new(executable)
        .arg("--new-window")
        .arg("--audio-description-input")
        .arg(path)
        .spawn()
    {
        Ok(_) => crate::log_debug(&format!(
            "Scheduled TV recording opened in audio-description window: {}",
            path.display()
        )),
        Err(error) => crate::log_debug(&format!(
            "Scheduled TV recording audio-description handoff failed: path={} error={}",
            path.display(),
            error
        )),
    }
}

pub(crate) fn run_scheduled_recording(id: &str) -> i32 {
    let schedule = match load_schedule(id) {
        Ok(schedule) => schedule,
        Err(error) => {
            crate::log_debug(&format!(
                "Scheduled recording could not be loaded id={id} error={error}"
            ));
            return 2;
        }
    };
    let result = match &schedule.source {
        ScheduledRecordingSource::Radio { name, url } => {
            stream_recording::record_stream_for_duration(
                url,
                name,
                None,
                StreamRecordingKind::Radio,
                Some(schedule.language),
                stream_recording::ScheduledRecordingOptions {
                    duration_minutes: schedule.duration_minutes,
                    scheduled_id: &schedule.id,
                    prefer_audio_description: false,
                },
            )
        }
        ScheduledRecordingSource::Tv { channel } => {
            tv::resolve_stream_url(channel).and_then(|url| {
                stream_recording::record_stream_for_duration(
                    &url,
                    &channel.name,
                    Some(channel.media_playback_user_agent()),
                    StreamRecordingKind::Tv,
                    None,
                    stream_recording::ScheduledRecordingOptions {
                        duration_minutes: schedule.duration_minutes,
                        scheduled_id: &schedule.id,
                        prefer_audio_description: tv::is_rai_audio_description_channel(channel),
                    },
                )
            })
        }
    };
    let exit_code = match result {
        Ok(path) => {
            crate::log_debug(&format!(
                "Scheduled recording completed id={} output={}",
                schedule.id,
                path.display()
            ));
            if matches!(&schedule.source, ScheduledRecordingSource::Tv { .. }) {
                maybe_open_audio_description_after_scheduled_tv_recording(&path);
            }
            0
        }
        Err(error) => {
            crate::log_debug(&format!(
                "Scheduled recording failed id={} error={error}",
                schedule.id
            ));
            3
        }
    };
    if schedule.recurrence == RecordingRecurrence::Once {
        delete_task(&schedule.id);
        if let Err(error) = fs::remove_file(schedule_path(&schedule.id))
            && error.kind() != std::io::ErrorKind::NotFound
        {
            crate::log_debug(&format!(
                "Scheduled recording definition removal failed id={} error={error}",
                schedule.id
            ));
        }
    }
    exit_code
}

fn localized_date(language: Language, date: NaiveDate, today: NaiveDate) -> String {
    let prefix = if date == today {
        format!("{}, ", tr(language, "calendar.today"))
    } else if date == today + Duration::days(1) {
        format!("{}, ", tr(language, "calendar.tomorrow"))
    } else {
        String::new()
    };
    format!("{}{date}", prefix)
}

fn layout(hwnd: HWND) {
    unsafe {
        let mut rect = windows::Win32::Foundation::RECT::default();
        if GetClientRect(hwnd, &mut rect).is_err() {
            return;
        }
        let width = (rect.right - rect.left).max(620);
        let height = (rect.bottom - rect.top).max(500);
        let margin = 18;
        let content = width - margin * 2;
        let rows = [
            (ID_LABEL_SOURCE, 16, 30),
            (ID_LABEL_DAY, 58, 24),
            (ID_COMBO_DAY, 84, 260),
            (ID_LABEL_HOUR, 132, 24),
            (ID_COMBO_HOUR, 158, 260),
            (ID_LABEL_MINUTE, 206, 24),
            (ID_COMBO_MINUTE, 232, 260),
            (ID_LABEL_RECURRENCE, 280, 24),
            (ID_COMBO_RECURRENCE, 306, 260),
            (ID_LABEL_DURATION, 354, 24),
            (ID_COMBO_DURATION, 380, 260),
            (ID_LABEL_CUSTOM_DURATION, 428, 24),
            (ID_EDIT_CUSTOM_DURATION, 454, 34),
        ];
        for (id, y, control_height) in rows {
            crate::log_if_err!(MoveWindow(
                GetDlgItem(hwnd, id as i32),
                margin,
                y,
                content,
                control_height,
                true
            ));
        }
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_BUTTON_SAVE as i32),
            width - margin - 280,
            height - 52,
            130,
            34,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_BUTTON_CANCEL as i32),
            width - margin - 140,
            height - 52,
            130,
            34,
            true
        ));
    }
}

fn tr(language: Language, key: &str) -> String {
    i18n::tr(language, key)
}

fn show_message(hwnd: HWND, language: Language, key: &str, icon: MESSAGEBOX_STYLE) {
    show_text(
        hwnd,
        &tr(language, "scheduled_recording.title"),
        &tr(language, key),
        icon,
    );
}

fn show_text(hwnd: HWND, title: &str, body: &str, icon: MESSAGEBOX_STYLE) {
    let title = to_wide(title);
    let body = to_wide(body);
    crate::message_box_w_safe(
        hwnd,
        PCWSTR(body.as_ptr()),
        PCWSTR(title.as_ptr()),
        MB_OK | icon,
    );
}

fn with_state<T>(hwnd: HWND, callback: impl FnOnce(&mut ScheduleDialogState) -> T) -> Option<T> {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ScheduleDialogState };
    if pointer.is_null() {
        None
    } else {
        Some(callback(unsafe { &mut *pointer }))
    }
}
