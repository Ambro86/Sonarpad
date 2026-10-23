use std::thread;

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Timelike};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, DEFAULT_GUI_FONT, HBRUSH, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    IsWindowEnabled, VK_ESCAPE, VK_RETURN, VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, CBS_DROPDOWNLIST, CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW,
    DefWindowProcW, DestroyWindow, DispatchMessageW, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY,
    GWLP_USERDATA, GetClientRect, GetDlgItem, GetMessageW, GetWindowLongPtrW, HMENU, IDC_ARROW,
    IsDialogMessageW, IsWindow, LoadCursorW, MSG, MoveWindow, RegisterClassW, SW_SHOW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowTextW, ShowWindow, TranslateMessage, WM_APP,
    WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_SETFONT, WM_SIZE,
    WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_POPUP,
    WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, w};

use crate::accessibility::to_wide;
use crate::tools::tv::{self, TvChannel, TvProgram};

const CLASS_NAME: &str = "SonarpadTvGuideWindow";
const ID_LABEL_DAY: usize = 4101;
const ID_COMBO_DAY: usize = 4102;
const ID_LABEL_PROGRAMS: usize = 4103;
const ID_COMBO_PROGRAMS: usize = 4104;
const ID_LABEL_PLOT: usize = 4105;
const ID_BUTTON_CLOSE: usize = 4106;
const ID_EDIT_PLOT: usize = 4107;
const CB_ADDSTRING: u32 = 0x0143;
const CB_GETCURSEL: u32 = 0x0147;
const CB_RESETCONTENT: u32 = 0x014B;
const CB_SETCURSEL: u32 = 0x014E;
const CBN_SELCHANGE: u16 = 1;
const WM_GUIDE_LOADED: u32 = WM_APP + 173;

struct TvGuideState {
    parent: HWND,
    channel: TvChannel,
    combo_day: HWND,
    combo_programs: HWND,
    edit_plot: HWND,
    programs: Vec<TvProgram>,
    dates: Vec<NaiveDate>,
    generation: u64,
}

struct TvGuideLoaded {
    generation: u64,
    result: Result<Vec<TvProgram>, String>,
}

pub(crate) fn open(parent: HWND, channel: TvChannel) {
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

        let mut state = Box::new(TvGuideState {
            parent,
            channel,
            combo_day: HWND(0),
            combo_programs: HWND(0),
            edit_plot: HWND(0),
            programs: Vec::new(),
            dates: Vec::new(),
            generation: 0,
        });
        let state_pointer = state.as_mut() as *mut TvGuideState;
        let title = to_wide(&crate::i18n::tr_tv_f(
            "tv.guide.title",
            &[("channel", &state.channel.name)],
        ));
        crate::enable_window_safe(parent, false);
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            780,
            620,
            parent,
            HMENU(0),
            hinstance,
            Some(state_pointer.cast()),
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
            if handle_keyboard(hwnd, &message) {
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

fn handle_keyboard(hwnd: HWND, message: &MSG) -> bool {
    if message.message != WM_KEYDOWN {
        return false;
    }
    let key = message.wParam.0 as u32;
    if key == VK_ESCAPE.0 as u32 {
        crate::log_if_err!(crate::post_message_w_safe(
            hwnd,
            WM_CLOSE,
            WPARAM(0),
            LPARAM(0)
        ));
        return true;
    }
    if key == VK_RETURN.0 as u32
        && crate::get_focus_safe() == unsafe { GetDlgItem(hwnd, ID_BUTTON_CLOSE as i32) }
    {
        crate::log_if_err!(crate::post_message_w_safe(
            hwnd,
            WM_CLOSE,
            WPARAM(0),
            LPARAM(0)
        ));
        return true;
    }
    if key != VK_TAB.0 as u32 {
        return false;
    }
    let controls = [
        ID_COMBO_DAY,
        ID_COMBO_PROGRAMS,
        ID_EDIT_PLOT,
        ID_BUTTON_CLOSE,
    ]
    .map(|id| unsafe { GetDlgItem(hwnd, id as i32) })
    .into_iter()
    .filter(|control| control.0 != 0 && unsafe { IsWindowEnabled(*control).as_bool() })
    .collect::<Vec<_>>();
    if controls.is_empty() {
        return false;
    }
    let current = crate::get_focus_safe();
    let backwards = (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
    let index = controls.iter().position(|control| *control == current);
    let next = match (index, backwards) {
        (Some(0), true) | (None, true) => controls.len() - 1,
        (Some(value), true) => value - 1,
        (Some(value), false) if value + 1 < controls.len() => value + 1,
        _ => 0,
    };
    crate::set_focus_safe(controls[next]);
    true
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "tv_guide_window_wndproc",
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
                let state = (*create).lpCreateParams as *mut TvGuideState;
                if state.is_null() {
                    return LRESULT(-1);
                }
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
                create_controls(hwnd, &mut *state);
                start_load(hwnd);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let notification = (wparam.0 >> 16) as u16;
                if id == ID_COMBO_DAY && notification == CBN_SELCHANGE {
                    start_load(hwnd);
                    return LRESULT(0);
                }
                if id == ID_COMBO_PROGRAMS && notification == CBN_SELCHANGE {
                    update_selected_program_plot(hwnd);
                    return LRESULT(0);
                }
                if id == ID_BUTTON_CLOSE {
                    crate::log_if_err!(DestroyWindow(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_GUIDE_LOADED => {
                let payload = Box::from_raw(lparam.0 as *mut TvGuideLoaded);
                let current_generation = with_state(hwnd, |state| state.generation).unwrap_or(0);
                if payload.generation == current_generation {
                    apply_loaded_programs(hwnd, payload.result);
                }
                LRESULT(0)
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
                let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TvGuideState;
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

unsafe fn create_controls(hwnd: HWND, state: &mut TvGuideState) {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let font = HFONT(crate::get_stock_object_safe(DEFAULT_GUI_FONT).0);
        create_control(
            hwnd,
            hinstance,
            w!("STATIC"),
            &crate::i18n::tr_tv("tv.guide.day_label"),
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
            &crate::i18n::tr_tv("tv.guide.programs_label"),
            (WS_CHILD | WS_VISIBLE, 0),
            ID_LABEL_PROGRAMS,
            font,
        );
        state.combo_programs = create_control(
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
            ID_COMBO_PROGRAMS,
            font,
        );
        create_control(
            hwnd,
            hinstance,
            w!("STATIC"),
            &crate::i18n::tr_tv("tv.guide.plot_label"),
            (WS_CHILD | WS_VISIBLE, 0),
            ID_LABEL_PLOT,
            font,
        );
        state.edit_plot = create_control(
            hwnd,
            hinstance,
            w!("EDIT"),
            "",
            (
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_BORDER
                    | WS_VSCROLL
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                        (ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY) as u32,
                    ),
                0,
            ),
            ID_EDIT_PLOT,
            font,
        );
        create_control(
            hwnd,
            hinstance,
            w!("BUTTON"),
            &crate::i18n::tr_tv("tv.guide.close"),
            (
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                        BS_DEFPUSHBUTTON as u32,
                    ),
                0,
            ),
            ID_BUTTON_CLOSE,
            font,
        );

        let today = Local::now().date_naive();
        state.dates = (0..8)
            .map(|offset| today + Duration::days(offset))
            .collect();
        for date in &state.dates {
            let label = to_wide(&date_label(*date, today));
            crate::send_message_w_safe(
                state.combo_day,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(label.as_ptr() as isize),
            );
        }
        crate::send_message_w_safe(state.combo_day, CB_SETCURSEL, WPARAM(0), LPARAM(0));
        set_program_controls_loading(state);
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

fn start_load(hwnd: HWND) {
    let Some((channel, date, generation)) = with_state(hwnd, |state| {
        let selected =
            crate::send_message_w_safe(state.combo_day, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        let index = if selected < 0 { 0 } else { selected as usize };
        let date = state.dates.get(index).copied()?;
        state.generation = state.generation.wrapping_add(1);
        Some((state.channel.clone(), date, state.generation))
    })
    .flatten() else {
        return;
    };
    if with_state(hwnd, set_program_controls_loading).is_none() {
        crate::log_debug("TV guide: state unavailable while entering loading state");
        return;
    }
    let hwnd_value = hwnd.0;
    thread::spawn(move || {
        let result = load_guide_programs(&channel, date);
        let payload = Box::new(TvGuideLoaded { generation, result });
        let payload_pointer = Box::into_raw(payload);
        if let Err(error) = crate::post_message_w_safe(
            HWND(hwnd_value),
            WM_GUIDE_LOADED,
            WPARAM(0),
            LPARAM(payload_pointer as isize),
        ) {
            let _payload = unsafe { Box::from_raw(payload_pointer) };
            crate::log_debug(&format!("TV guide result post failed: {error}"));
        }
    });
}

fn load_guide_programs(channel: &TvChannel, date: NaiveDate) -> Result<Vec<TvProgram>, String> {
    let now = Local::now();
    let today = now.date_naive();
    let now_seconds = now.timestamp();
    let mut programs = tv::load_channel_guide(channel, date)?
        .into_iter()
        .filter(|program| {
            let start_date = Local
                .timestamp_opt(program.start_time, 0)
                .single()
                .map(|value| value.date_naive());
            let end_date = Local
                .timestamp_opt(program.end_time, 0)
                .single()
                .map(|value| value.date_naive());
            let belongs_to_day = start_date == Some(date) || end_date == Some(date);
            belongs_to_day && (date != today || program.end_time > now_seconds)
        })
        .collect::<Vec<_>>();
    programs.sort_by_key(|program| program.start_time);
    crate::log_debug(&format!(
        "TV guide display: channel={:?} date={} today={} visible_programs={}",
        channel.name,
        date,
        date == today,
        programs.len()
    ));
    Ok(programs)
}

fn program_label(program: &TvProgram) -> String {
    let start = Local.timestamp_opt(program.start_time, 0).single();
    let end = Local.timestamp_opt(program.end_time, 0).single();
    match (start, end) {
        (Some(start), Some(end)) => format!(
            "{:02}:{:02} - {:02}:{:02}  {}",
            start.hour(),
            start.minute(),
            end.hour(),
            end.minute(),
            program.title
        ),
        _ => program.title.clone(),
    }
}

fn reset_program_combo(combo: HWND, message: Option<&str>) {
    crate::send_message_w_safe(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    if let Some(message) = message {
        let wide = to_wide(message);
        crate::send_message_w_safe(
            combo,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
        crate::send_message_w_safe(combo, CB_SETCURSEL, WPARAM(0), LPARAM(0));
    }
}

fn set_program_controls_loading(state: &mut TvGuideState) {
    state.programs.clear();
    reset_program_combo(
        state.combo_programs,
        Some(&crate::i18n::tr_tv("tv.guide.loading")),
    );
    crate::enable_window_safe(state.combo_programs, false);
    set_plot_text(state.edit_plot, &crate::i18n::tr_tv("tv.guide.loading"));
}

fn apply_loaded_programs(hwnd: HWND, result: Result<Vec<TvProgram>, String>) {
    if with_state(hwnd, |state| {
        state.programs.clear();
        match result {
            Ok(programs) if !programs.is_empty() => {
                reset_program_combo(state.combo_programs, None);
                for display in crate::result_labels::accessible_result_labels(
                    programs.iter().map(program_label).collect(),
                ) {
                    let label = to_wide(&display);
                    crate::send_message_w_safe(
                        state.combo_programs,
                        CB_ADDSTRING,
                        WPARAM(0),
                        LPARAM(label.as_ptr() as isize),
                    );
                }
                state.programs = programs;
                crate::send_message_w_safe(
                    state.combo_programs,
                    CB_SETCURSEL,
                    WPARAM(0),
                    LPARAM(0),
                );
                crate::enable_window_safe(state.combo_programs, true);
                update_plot_for_state(state);
            }
            Ok(_) => {
                reset_program_combo(
                    state.combo_programs,
                    Some(&crate::i18n::tr_tv("tv.guide.no_programs")),
                );
                crate::enable_window_safe(state.combo_programs, false);
                set_plot_text(state.edit_plot, &crate::i18n::tr_tv("tv.guide.no_programs"));
            }
            Err(error) => {
                reset_program_combo(state.combo_programs, Some(&error));
                crate::enable_window_safe(state.combo_programs, false);
                set_plot_text(state.edit_plot, &error);
            }
        }
    })
    .is_none()
    {
        crate::log_debug("TV guide: state unavailable while applying loaded programs");
    }
}

fn set_plot_text(edit: HWND, text: &str) {
    if edit.0 == 0 {
        return;
    }
    let wide = to_wide(text);
    unsafe {
        crate::log_if_err!(SetWindowTextW(edit, PCWSTR(wide.as_ptr())));
    }
}

fn update_plot_for_state(state: &mut TvGuideState) {
    let selection =
        crate::send_message_w_safe(state.combo_programs, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if selection < 0 {
        set_plot_text(state.edit_plot, &crate::i18n::tr_tv("tv.guide.no_plot"));
        return;
    }
    let Some(program) = state.programs.get(selection as usize) else {
        set_plot_text(state.edit_plot, &crate::i18n::tr_tv("tv.guide.no_plot"));
        return;
    };
    let description = program.description.trim();
    if description.is_empty() {
        set_plot_text(state.edit_plot, &crate::i18n::tr_tv("tv.guide.no_plot"));
    } else {
        set_plot_text(state.edit_plot, description);
    }
}

fn update_selected_program_plot(hwnd: HWND) {
    if with_state(hwnd, update_plot_for_state).is_none() {
        crate::log_debug("TV guide: state unavailable while updating selected program plot");
    }
}

fn date_label(date: NaiveDate, today: NaiveDate) -> String {
    const WEEKDAY_KEYS: [&str; 7] = [
        "tv.guide.weekday.monday",
        "tv.guide.weekday.tuesday",
        "tv.guide.weekday.wednesday",
        "tv.guide.weekday.thursday",
        "tv.guide.weekday.friday",
        "tv.guide.weekday.saturday",
        "tv.guide.weekday.sunday",
    ];
    const MONTH_KEYS: [&str; 12] = [
        "tv.guide.month.january",
        "tv.guide.month.february",
        "tv.guide.month.march",
        "tv.guide.month.april",
        "tv.guide.month.may",
        "tv.guide.month.june",
        "tv.guide.month.july",
        "tv.guide.month.august",
        "tv.guide.month.september",
        "tv.guide.month.october",
        "tv.guide.month.november",
        "tv.guide.month.december",
    ];
    let prefix = if date == today {
        crate::i18n::tr_tv("tv.guide.today_prefix")
    } else if date == today + Duration::days(1) {
        crate::i18n::tr_tv("tv.guide.tomorrow_prefix")
    } else {
        String::new()
    };
    let weekday = crate::i18n::tr_tv(WEEKDAY_KEYS[date.weekday().num_days_from_monday() as usize]);
    let month = crate::i18n::tr_tv(MONTH_KEYS[date.month0() as usize]);
    format!(
        "{}{} {} {} {}",
        prefix,
        weekday,
        date.day(),
        month,
        date.year()
    )
}

fn layout(hwnd: HWND) {
    unsafe {
        let mut rect = windows::Win32::Foundation::RECT::default();
        if GetClientRect(hwnd, &mut rect).is_err() {
            return;
        }
        let width = (rect.right - rect.left).max(620);
        let height = (rect.bottom - rect.top).max(480);
        let margin = 18;
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_LABEL_DAY as i32),
            margin,
            16,
            width - margin * 2,
            24,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_COMBO_DAY as i32),
            margin,
            42,
            width - margin * 2,
            260,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_LABEL_PROGRAMS as i32),
            margin,
            82,
            width - margin * 2,
            24,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_COMBO_PROGRAMS as i32),
            margin,
            108,
            width - margin * 2,
            260,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_LABEL_PLOT as i32),
            margin,
            150,
            width - margin * 2,
            24,
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_EDIT_PLOT as i32),
            margin,
            176,
            width - margin * 2,
            (height - 246).max(180),
            true
        ));
        crate::log_if_err!(MoveWindow(
            GetDlgItem(hwnd, ID_BUTTON_CLOSE as i32),
            width - margin - 130,
            height - 52,
            130,
            34,
            true
        ));
    }
}

fn with_state<T>(hwnd: HWND, callback: impl FnOnce(&mut TvGuideState) -> T) -> Option<T> {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TvGuideState };
    if pointer.is_null() {
        None
    } else {
        Some(callback(unsafe { &mut *pointer }))
    }
}
