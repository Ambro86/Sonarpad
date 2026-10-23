use crate::accessibility::to_wide;
use crate::app_windows::weather_service::{WeatherClient, WeatherForecast};
use crate::app_windows::youtube_transcript_window::choose_combo_option_dialog;
use crate::i18n;
use crate::settings::{
    Language, WeatherCity, WeatherTemperatureUnit, load_settings, save_settings,
};
use chrono::NaiveDate;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, DEFAULT_GUI_FONT, HBRUSH, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_RETURN, VK_SHIFT, VK_TAB};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, BS_DEFPUSHBUTTON, CREATESTRUCTW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, ES_AUTOHSCROLL, ES_AUTOVSCROLL,
    ES_MULTILINE, ES_READONLY, GWLP_USERDATA, GetCursorPos, GetDlgItem, GetMessageW,
    GetWindowTextLengthW, GetWindowTextW, HMENU, IDC_ARROW, IDYES, IsDialogMessageW, IsWindow,
    LoadCursorW, MB_ICONQUESTION, MB_YESNO, MF_STRING, MSG, MoveWindow, RegisterClassW, SW_SHOW,
    SetForegroundWindow, ShowWindow, TPM_NONOTIFY, TPM_RETURNCMD, TrackPopupMenu, TranslateMessage,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_CREATE,
    WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_SETFOCUS, WM_SETFONT, WM_SIZE, WNDCLASSW, WS_BORDER,
    WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_POPUP,
    WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, w};

const CLASS_NAME: &str = "SonarpadWeatherWindow";
const ID_LABEL_CITY: usize = 3101;
const ID_EDIT_CITY: usize = 3102;
const ID_BUTTON_SEARCH: usize = 3103;
const ID_LABEL_RECENT: usize = 3104;
const ID_LIST_RECENT: usize = 3105;
const ID_LABEL_DAY: usize = 3106;
const ID_COMBO_DAY: usize = 3107;
const ID_LABEL_UNIT: usize = 3108;
const ID_COMBO_UNIT: usize = 3109;
const ID_LABEL_FORECAST: usize = 3110;
const ID_EDIT_FORECAST: usize = 3111;
const ID_BUTTON_CLOSE: usize = 3112;

const ID_RECENT_REMOVE: usize = 1;
const ID_RECENT_CLEAR: usize = 2;

const CB_ADDSTRING: u32 = 0x0143;
const CB_RESETCONTENT: u32 = 0x014B;
const CB_GETCURSEL: u32 = 0x0147;
const CB_SETCURSEL: u32 = 0x014E;
const LB_ADDSTRING: u32 = 0x0180;
const LB_RESETCONTENT: u32 = 0x0184;
const LB_GETCURSEL: u32 = 0x0188;
const CBN_SELCHANGE: u16 = 1;
const LBN_DBLCLK: u16 = 2;

const WM_WEATHER_SEARCH_COMPLETE: u32 = WM_APP + 121;
const WM_WEATHER_FORECAST_COMPLETE: u32 = WM_APP + 122;

struct WeatherDialogState {
    parent: HWND,
    language: Language,
    edit_city: HWND,
    list_recent: HWND,
    combo_day: HWND,
    combo_unit: HWND,
    edit_forecast: HWND,
    recent_cities: Vec<WeatherCity>,
    selected_city: Option<WeatherCity>,
    forecast: Option<WeatherForecast>,
}

struct WeatherSearchComplete {
    result: Result<Vec<WeatherCity>, String>,
}

struct WeatherForecastComplete {
    city: WeatherCity,
    result: Result<WeatherForecast, String>,
}

pub fn open(parent: HWND) {
    crate::log_debug(&format!(
        "Weather window open requested parent={:?}",
        parent
    ));
    unsafe {
        let language = load_settings().language;
        let hinstance = HINSTANCE(GetModuleHandleW(None).map(|module| module.0).unwrap_or(0));
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

        let title = to_wide(&tr(language, "weather.title"));
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            820,
            650,
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
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&message) {
                continue;
            }
            if handle_tab(hwnd, &message)
                || handle_enter(hwnd, &message)
                || handle_escape(hwnd, &message)
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

fn handle_tab(hwnd: HWND, message: &MSG) -> bool {
    if message.message != WM_KEYDOWN || message.wParam.0 as u32 != VK_TAB.0 as u32 {
        return false;
    }

    let controls = [
        ID_EDIT_CITY,
        ID_BUTTON_SEARCH,
        ID_LIST_RECENT,
        ID_COMBO_DAY,
        ID_COMBO_UNIT,
        ID_EDIT_FORECAST,
        ID_BUTTON_CLOSE,
    ]
    .map(|id| unsafe { GetDlgItem(hwnd, id as i32) });

    let current = crate::get_focus_safe();
    let backwards = (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
    let current_index = controls.iter().position(|control| *control == current);
    let next_index = match (current_index, backwards) {
        (Some(0), true) | (None, true) => controls.len() - 1,
        (Some(index), true) => index - 1,
        (Some(index), false) if index + 1 < controls.len() => index + 1,
        _ => 0,
    };

    let target = controls[next_index];
    if target.0 != 0 {
        crate::log_debug(&format!(
            "Weather Tab navigation current={:?} target={:?} backwards={}",
            current, target, backwards
        ));
        crate::set_focus_safe(target);
        return true;
    }

    false
}

fn handle_enter(hwnd: HWND, message: &MSG) -> bool {
    if message.message != WM_KEYDOWN || message.wParam.0 as u32 != VK_RETURN.0 as u32 {
        return false;
    }

    let close_button = unsafe { GetDlgItem(hwnd, ID_BUTTON_CLOSE as i32) };
    if close_button.0 != 0 && crate::get_focus_safe() == close_button {
        crate::log_debug("Weather close button activated with Enter");
        crate::log_if_err!(crate::post_message_w_safe(
            hwnd,
            WM_CLOSE,
            WPARAM(0),
            LPARAM(0)
        ));
        return true;
    }

    let target = with_state(hwnd, |state| {
        if message.hwnd == state.edit_city {
            1
        } else if message.hwnd == state.list_recent {
            2
        } else {
            0
        }
    })
    .unwrap_or(0);
    match target {
        1 => search_city(hwnd),
        2 => load_selected_recent_city(hwnd),
        _ => return false,
    }
    true
}

fn handle_escape(hwnd: HWND, message: &MSG) -> bool {
    if message.message != WM_KEYDOWN || message.wParam.0 as u32 != VK_ESCAPE.0 as u32 {
        return false;
    }
    crate::log_if_err!(crate::post_message_w_safe(
        hwnd,
        WM_CLOSE,
        WPARAM(0),
        LPARAM(0)
    ));
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
            "weather_window_wndproc",
            || DefWindowProcW(hwnd, message, wparam, lparam),
            || weather_wndproc_inner(hwnd, message, wparam, lparam),
        )
    }
}

fn weather_wndproc_inner(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_CREATE => {
                crate::log_debug(&format!("Weather WM_CREATE hwnd={:?}", hwnd));
                let create = lparam.0 as *const CREATESTRUCTW;
                let parent = HWND((*create).lpCreateParams as isize);
                create_controls(hwnd, parent);
                LRESULT(0)
            }
            WM_SIZE => {
                layout(hwnd);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                if let Some(edit_city) = with_state(hwnd, |state| state.edit_city) {
                    crate::set_focus_safe(edit_city);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let notification = (wparam.0 >> 16) as u16;
                match id {
                    ID_BUTTON_SEARCH => {
                        search_city(hwnd);
                        LRESULT(0)
                    }
                    ID_LIST_RECENT if notification == LBN_DBLCLK => {
                        load_selected_recent_city(hwnd);
                        LRESULT(0)
                    }
                    ID_COMBO_DAY if notification == CBN_SELCHANGE => {
                        refresh_forecast_text(hwnd);
                        LRESULT(0)
                    }
                    ID_COMBO_UNIT if notification == CBN_SELCHANGE => {
                        save_selected_unit(hwnd);
                        refresh_forecast_text(hwnd);
                        LRESULT(0)
                    }
                    ID_BUTTON_CLOSE => {
                        crate::log_if_err!(DestroyWindow(hwnd));
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
                }
            }
            WM_CONTEXTMENU => {
                if show_recent_context_menu(hwnd, wparam, lparam) {
                    LRESULT(0)
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }
            WM_WEATHER_SEARCH_COMPLETE => {
                let result = Box::from_raw(lparam.0 as *mut WeatherSearchComplete);
                finish_city_search(hwnd, *result);
                LRESULT(0)
            }
            WM_WEATHER_FORECAST_COMPLETE => {
                let result = Box::from_raw(lparam.0 as *mut WeatherForecastComplete);
                finish_forecast(hwnd, *result);
                LRESULT(0)
            }
            WM_CLOSE => {
                crate::log_if_err!(DestroyWindow(hwnd));
                LRESULT(0)
            }
            WM_DESTROY => {
                let parent = with_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
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
                    as *mut WeatherDialogState;
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

fn create_controls(hwnd: HWND, parent: HWND) {
    let settings = load_settings();
    let language = settings.language;
    let city_label = create_static(hwnd, &tr(language, "weather.city"), ID_LABEL_CITY);
    let edit_city = create_edit(hwnd, ID_EDIT_CITY, false);
    let search = create_button(
        hwnd,
        &tr(language, "weather.search"),
        ID_BUTTON_SEARCH,
        true,
    );
    let recent_label = create_static(
        hwnd,
        &tr(language, "weather.recent_cities"),
        ID_LABEL_RECENT,
    );
    let list_recent = create_listbox(hwnd, ID_LIST_RECENT);
    let day_label = create_static(hwnd, &tr(language, "weather.choose_day"), ID_LABEL_DAY);
    let combo_day = create_combo(hwnd, ID_COMBO_DAY);
    let unit_label = create_static(
        hwnd,
        &tr(language, "weather.temperature_unit"),
        ID_LABEL_UNIT,
    );
    let combo_unit = create_combo(hwnd, ID_COMBO_UNIT);
    let forecast_label = create_static(hwnd, &tr(language, "weather.forecast"), ID_LABEL_FORECAST);
    let edit_forecast = create_edit(hwnd, ID_EDIT_FORECAST, true);
    let close = create_button(hwnd, &tr(language, "weather.close"), ID_BUTTON_CLOSE, false);

    let font = HFONT(crate::get_stock_object_safe(DEFAULT_GUI_FONT).0);
    for control in [
        city_label,
        edit_city,
        search,
        recent_label,
        list_recent,
        day_label,
        combo_day,
        unit_label,
        combo_unit,
        forecast_label,
        edit_forecast,
        close,
    ] {
        crate::send_message_w_safe(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    }

    for label in [
        tr(language, "weather.celsius"),
        tr(language, "weather.fahrenheit"),
    ] {
        let wide = to_wide(&label);
        crate::send_message_w_safe(
            combo_unit,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    let unit_index = match settings.weather_temperature_unit {
        WeatherTemperatureUnit::Celsius => 0,
        WeatherTemperatureUnit::Fahrenheit => 1,
    };
    crate::send_message_w_safe(combo_unit, CB_SETCURSEL, WPARAM(unit_index), LPARAM(0));

    let mut state = Box::new(WeatherDialogState {
        parent,
        language,
        edit_city,
        list_recent,
        combo_day,
        combo_unit,
        edit_forecast,
        recent_cities: settings.weather_recent_cities.clone(),
        selected_city: settings.weather_city.clone(),
        forecast: None,
    });
    normalize_recent_cities(&mut state.recent_cities);
    crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
    populate_recent_cities(hwnd);
    set_forecast_text(hwnd, &tr(language, "weather.enter_city"));
    layout(hwnd);

    if let Some(city) = settings.weather_city {
        set_control_text(edit_city, &city.name);
        fetch_forecast(hwnd, city);
    } else {
        crate::set_focus_safe(edit_city);
    }
}

fn search_city(hwnd: HWND) {
    let Some((language, edit_city, edit_forecast)) = with_state(hwnd, |state| {
        (state.language, state.edit_city, state.edit_forecast)
    }) else {
        return;
    };
    let query = get_control_text(edit_city).trim().to_string();
    crate::log_debug(&format!("Weather city search requested query={:?}", query));
    if query.is_empty() {
        set_control_text(edit_forecast, &tr(language, "weather.enter_city"));
        crate::set_focus_safe(edit_city);
        return;
    }
    set_control_text(edit_forecast, &tr(language, "weather.searching_city"));
    let hwnd_value = hwnd.0;
    std::thread::spawn(move || {
        let result = WeatherClient::new().search_city(&query, language);
        match &result {
            Ok(cities) => crate::log_debug(&format!(
                "Weather city search worker completed query={:?} count={}",
                query,
                cities.len()
            )),
            Err(error) => crate::log_debug(&format!(
                "Weather city search worker failed query={:?} error={}",
                query, error
            )),
        }
        let completion = Box::new(WeatherSearchComplete { result });
        let raw = Box::into_raw(completion);
        let posted = crate::post_message_w_safe(
            HWND(hwnd_value),
            WM_WEATHER_SEARCH_COMPLETE,
            WPARAM(0),
            LPARAM(raw as isize),
        );
        if posted.is_err() {
            unsafe {
                let _completion = Box::from_raw(raw);
            }
        }
    });
}

fn finish_city_search(hwnd: HWND, completion: WeatherSearchComplete) {
    crate::log_debug(&format!(
        "Weather finish_city_search entered hwnd={:?}",
        hwnd
    ));
    let Some((language, edit_forecast)) =
        with_state(hwnd, |state| (state.language, state.edit_forecast))
    else {
        return;
    };
    let cities = match completion.result {
        Ok(cities) if !cities.is_empty() => cities,
        Ok(_) => {
            set_control_text(edit_forecast, &tr(language, "weather.city_not_found"));
            return;
        }
        Err(error) => {
            crate::log_debug(&format!("Weather city search error: {error}"));
            set_control_text(edit_forecast, &tr(language, "weather.search_error"));
            return;
        }
    };

    crate::log_debug(&format!(
        "Weather city search UI received {} result(s)",
        cities.len()
    ));
    let city = if cities.len() == 1 {
        crate::log_debug(&format!(
            "Weather city auto-selected label={}",
            city_display_label(&cities[0])
        ));
        cities[0].clone()
    } else {
        let options = cities.iter().map(city_display_label).collect::<Vec<_>>();
        crate::log_debug(&format!(
            "Weather opening safe city combo selector with {} option(s)",
            options.len()
        ));
        let Some(index) = choose_combo_option_dialog(
            hwnd,
            language,
            tr(language, "weather.choose_city"),
            tr(language, "weather.city"),
            options,
            0,
        ) else {
            crate::log_debug("Weather city combo selector cancelled");
            return;
        };
        crate::log_debug(&format!(
            "Weather city combo selector returned index={index}"
        ));
        cities
            .get(index)
            .cloned()
            .unwrap_or_else(|| cities[0].clone())
    };

    crate::log_debug(&format!(
        "Weather city selected label={} latitude={} longitude={}",
        city_display_label(&city),
        city.latitude,
        city.longitude
    ));
    if let Some(edit_city) = with_state(hwnd, |state| state.edit_city) {
        set_control_text(edit_city, &city.name);
    }
    fetch_forecast(hwnd, city);
}

fn fetch_forecast(hwnd: HWND, city: WeatherCity) {
    crate::log_debug(&format!(
        "Weather forecast fetch requested hwnd={:?} city={} latitude={} longitude={}",
        hwnd,
        city_display_label(&city),
        city.latitude,
        city.longitude
    ));
    let Some((language, edit_forecast)) =
        with_state(hwnd, |state| (state.language, state.edit_forecast))
    else {
        return;
    };
    set_control_text(edit_forecast, &tr(language, "weather.loading"));
    let hwnd_value = hwnd.0;
    std::thread::spawn(move || {
        let result = WeatherClient::new().forecast(&city);
        match &result {
            Ok(forecast) => crate::log_debug(&format!(
                "Weather forecast worker completed city={} days={}",
                city_display_label(&city),
                forecast.days.len()
            )),
            Err(error) => crate::log_debug(&format!(
                "Weather forecast worker failed city={} error={}",
                city_display_label(&city),
                error
            )),
        }
        let completion = Box::new(WeatherForecastComplete { city, result });
        let raw = Box::into_raw(completion);
        let posted = crate::post_message_w_safe(
            HWND(hwnd_value),
            WM_WEATHER_FORECAST_COMPLETE,
            WPARAM(0),
            LPARAM(raw as isize),
        );
        if posted.is_err() {
            unsafe {
                let _completion = Box::from_raw(raw);
            }
        }
    });
}

fn finish_forecast(hwnd: HWND, completion: WeatherForecastComplete) {
    crate::log_debug(&format!("Weather finish_forecast entered hwnd={:?}", hwnd));
    let Some((language, edit_forecast)) =
        with_state(hwnd, |state| (state.language, state.edit_forecast))
    else {
        return;
    };
    let forecast = match completion.result {
        Ok(forecast) => forecast,
        Err(error) => {
            crate::log_debug(&format!("Weather forecast error: {error}"));
            set_control_text(edit_forecast, &tr(language, "weather.search_error"));
            return;
        }
    };

    let city = completion.city;
    crate::log_debug(&format!(
        "Weather forecast accepted city={} days={}",
        city_display_label(&city),
        forecast.days.len()
    ));
    with_state(hwnd, |state| {
        state.selected_city = Some(city.clone());
        state.forecast = Some(forecast);
    });
    crate::log_debug("Weather saving selected city and recent list");
    save_city_and_recent(&city);
    crate::log_debug("Weather selected city saved successfully");
    if let Some(edit_city) = with_state(hwnd, |state| state.edit_city) {
        set_control_text(edit_city, &city.name);
    }
    with_state(hwnd, |state| {
        state.recent_cities = load_settings().weather_recent_cities;
        normalize_recent_cities(&mut state.recent_cities);
    });
    crate::log_debug("Weather refreshing recent cities control");
    populate_recent_cities(hwnd);
    crate::log_debug("Weather populating forecast days control");
    populate_days(hwnd);
    crate::log_debug("Weather formatting forecast text");
    refresh_forecast_text(hwnd);
    crate::set_focus_safe(edit_forecast);
    crate::log_debug("Weather finish_forecast completed");
}

fn populate_days(hwnd: HWND) {
    let Some((language, combo_day, forecast)) = with_state(hwnd, |state| {
        (state.language, state.combo_day, state.forecast.clone())
    }) else {
        return;
    };
    crate::send_message_w_safe(combo_day, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    let Some(forecast) = forecast else {
        return;
    };
    for label in crate::result_labels::accessible_result_labels(
        forecast
            .days
            .iter()
            .enumerate()
            .map(|(index, day)| day_label(language, index, &day.date))
            .collect(),
    ) {
        let wide = to_wide(&label);
        crate::send_message_w_safe(
            combo_day,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    crate::send_message_w_safe(combo_day, CB_SETCURSEL, WPARAM(0), LPARAM(0));
}

fn refresh_forecast_text(hwnd: HWND) {
    let Some((language, city, forecast, combo_day, combo_unit, edit_forecast)) =
        with_state(hwnd, |state| {
            (
                state.language,
                state.selected_city.clone(),
                state.forecast.clone(),
                state.combo_day,
                state.combo_unit,
                state.edit_forecast,
            )
        })
    else {
        return;
    };
    let (Some(city), Some(forecast)) = (city, forecast) else {
        return;
    };
    let selected_day = selected_index(combo_day).unwrap_or(0);
    let unit = if selected_index(combo_unit).unwrap_or(0) == 1 {
        WeatherTemperatureUnit::Fahrenheit
    } else {
        WeatherTemperatureUnit::Celsius
    };
    let text = format_forecast(language, &city, &forecast, selected_day, unit);
    set_control_text(edit_forecast, &text);
}

fn format_forecast(
    language: Language,
    city: &WeatherCity,
    forecast: &WeatherForecast,
    selected_day: usize,
    unit: WeatherTemperatureUnit,
) -> String {
    let Some(day) = forecast.days.get(selected_day) else {
        return tr(language, "weather.search_error");
    };
    let mut lines = Vec::new();
    lines.push(format!(
        "{}: {}",
        tr(language, "weather.forecast_for"),
        city_display_label(city)
    ));
    lines.push(day_label(language, selected_day, &day.date));
    lines.push(String::new());

    if selected_day == 0 {
        let situation = forecast
            .current
            .weather_code
            .map(|code| weather_code_label(language, code))
            .unwrap_or_else(|| "-".to_string());
        lines.push(format!(
            "{}: {}",
            tr(language, "weather.current_situation"),
            situation
        ));
        lines.push(format!(
            "{}: {}",
            tr(language, "weather.current_temperature"),
            format_temperature(forecast.current.temperature_c, unit)
        ));
    }
    lines.push(format!(
        "{}: {}",
        tr(language, "weather.max_temperature"),
        format_temperature(day.max_temperature_c, unit)
    ));
    lines.push(format!(
        "{}: {}",
        tr(language, "weather.min_temperature"),
        format_temperature(day.min_temperature_c, unit)
    ));
    lines.push(format!(
        "{}: {}",
        tr(language, "weather.precipitation_probability"),
        format_measure(day.precipitation_probability, "%")
    ));
    lines.push(format!(
        "{}: {}",
        tr(language, "weather.precipitation"),
        format_measure(day.precipitation_mm, "mm")
    ));
    lines.push(format!(
        "{}: {}",
        tr(language, "weather.wind"),
        format_measure(day.wind_speed_kmh, "km/h")
    ));
    if selected_day == 0 {
        lines.push(format!(
            "{}: {}",
            tr(language, "weather.relative_humidity"),
            format_measure(forecast.current.relative_humidity, "%")
        ));
    }
    lines.join("\r\n")
}

fn format_temperature(value: Option<f64>, unit: WeatherTemperatureUnit) -> String {
    let Some(celsius) = value else {
        return "-".to_string();
    };
    match unit {
        WeatherTemperatureUnit::Celsius => format!("{} °C", format_number(celsius)),
        WeatherTemperatureUnit::Fahrenheit => {
            format!("{} °F", format_number((celsius * 9.0 / 5.0) + 32.0))
        }
    }
}

fn format_measure(value: Option<f64>, unit: &str) -> String {
    value
        .map(|value| format!("{} {unit}", format_number(value)))
        .unwrap_or_else(|| "-".to_string())
}

fn format_number(value: f64) -> String {
    if (value - value.round()).abs() < 0.05 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn day_label(language: Language, index: usize, date: &str) -> String {
    match index {
        0 => tr(language, "weather.today"),
        1 => tr(language, "weather.tomorrow"),
        _ => NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map(|date| localized_numeric_date(language, date))
            .unwrap_or_else(|_| date.to_string()),
    }
}

fn localized_numeric_date(language: Language, date: NaiveDate) -> String {
    use chrono::Datelike;
    match language {
        Language::German => format!("{:02}.{:02}.{:04}", date.day(), date.month(), date.year()),
        Language::English => format!("{:02}/{:02}/{:04}", date.month(), date.day(), date.year()),
        Language::Chinese => {
            format!(
                "{:04}年{:02}月{:02}日",
                date.year(),
                date.month(),
                date.day()
            )
        }
        Language::Hindi => format!("{:02}-{:02}-{:04}", date.day(), date.month(), date.year()),
        _ => format!("{:02}/{:02}/{:04}", date.day(), date.month(), date.year()),
    }
}

fn weather_code_label(language: Language, code: i32) -> String {
    let key = format!("weather.code.{code}");
    let value = i18n::tr(language, &key);
    if value == key {
        code.to_string()
    } else {
        value
    }
}

fn save_selected_unit(hwnd: HWND) {
    let Some(combo_unit) = with_state(hwnd, |state| state.combo_unit) else {
        return;
    };
    let mut settings = load_settings();
    settings.weather_temperature_unit = if selected_index(combo_unit).unwrap_or(0) == 1 {
        WeatherTemperatureUnit::Fahrenheit
    } else {
        WeatherTemperatureUnit::Celsius
    };
    save_settings(settings);
}

fn save_city_and_recent(city: &WeatherCity) {
    let mut settings = load_settings();
    settings.weather_city = Some(city.clone());
    settings
        .weather_recent_cities
        .retain(|existing| !same_city(existing, city));
    settings.weather_recent_cities.insert(0, city.clone());
    normalize_recent_cities(&mut settings.weather_recent_cities);
    save_settings(settings);
}

fn normalize_recent_cities(cities: &mut Vec<WeatherCity>) {
    let mut normalized: Vec<WeatherCity> = Vec::new();
    for mut city in cities.drain(..) {
        city.name = city.name.trim().to_string();
        city.admin1 = city.admin1.trim().to_string();
        city.country = city.country.trim().to_string();
        if city.name.is_empty() || normalized.iter().any(|existing| same_city(existing, &city)) {
            continue;
        }
        normalized.push(city);
        if normalized.len() >= 50 {
            break;
        }
    }
    *cities = normalized;
}

fn same_city(left: &WeatherCity, right: &WeatherCity) -> bool {
    (left.latitude - right.latitude).abs() < 0.000_001
        && (left.longitude - right.longitude).abs() < 0.000_001
}

fn populate_recent_cities(hwnd: HWND) {
    let Some((list_recent, cities)) = with_state(hwnd, |state| {
        (state.list_recent, state.recent_cities.clone())
    }) else {
        return;
    };
    crate::send_message_w_safe(list_recent, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    for label in crate::result_labels::accessible_result_labels(
        cities.iter().map(city_display_label).collect(),
    ) {
        let wide = to_wide(&label);
        crate::send_message_w_safe(
            list_recent,
            LB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
}

fn load_selected_recent_city(hwnd: HWND) {
    let Some((list_recent, cities, edit_city)) = with_state(hwnd, |state| {
        (
            state.list_recent,
            state.recent_cities.clone(),
            state.edit_city,
        )
    }) else {
        return;
    };
    let Some(index) = selected_list_index(list_recent) else {
        return;
    };
    let Some(city) = cities.get(index).cloned() else {
        return;
    };
    set_control_text(edit_city, &city.name);
    fetch_forecast(hwnd, city);
}

fn show_recent_context_menu(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> bool {
    let Some((language, list_recent)) =
        with_state(hwnd, |state| (state.language, state.list_recent))
    else {
        return false;
    };
    if HWND(wparam.0 as isize) != list_recent {
        return false;
    }
    let menu = unsafe { CreatePopupMenu().unwrap_or(HMENU(0)) };
    if menu.0 == 0 {
        return false;
    }
    let remove = to_wide(&tr(language, "weather.delete_city"));
    let clear = to_wide(&tr(language, "weather.clear_history"));
    crate::log_if_err!(unsafe {
        AppendMenuW(menu, MF_STRING, ID_RECENT_REMOVE, PCWSTR(remove.as_ptr()))
    });
    crate::log_if_err!(unsafe {
        AppendMenuW(menu, MF_STRING, ID_RECENT_CLEAR, PCWSTR(clear.as_ptr()))
    });

    let point = if lparam.0 == -1 {
        let mut point = POINT::default();
        if unsafe { GetCursorPos(&mut point) }.is_err() {
            crate::log_if_err!(unsafe { DestroyMenu(menu) });
            return false;
        }
        point
    } else {
        POINT {
            x: (lparam.0 as u32 & 0xffff) as i16 as i32,
            y: ((lparam.0 as u32 >> 16) & 0xffff) as i16 as i32,
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
    match command.0 as usize {
        ID_RECENT_REMOVE => remove_selected_recent_city(hwnd),
        ID_RECENT_CLEAR => clear_recent_cities(hwnd),
        _ => {}
    }
    true
}

fn remove_selected_recent_city(hwnd: HWND) {
    let Some((list_recent, cities)) = with_state(hwnd, |state| {
        (state.list_recent, state.recent_cities.clone())
    }) else {
        return;
    };
    let Some(index) = selected_list_index(list_recent) else {
        return;
    };
    let Some(city) = cities.get(index).cloned() else {
        return;
    };
    let mut settings = load_settings();
    settings
        .weather_recent_cities
        .retain(|existing| !same_city(existing, &city));
    save_settings(settings.clone());
    with_state(hwnd, |state| {
        state.recent_cities = settings.weather_recent_cities;
    });
    populate_recent_cities(hwnd);
}

fn clear_recent_cities(hwnd: HWND) {
    let Some(language) = with_state(hwnd, |state| state.language) else {
        return;
    };
    let text = to_wide(&tr(language, "weather.confirm_clear_history"));
    let title = to_wide(&tr(language, "weather.clear_history"));
    let result = crate::message_box_w_safe(
        hwnd,
        PCWSTR(text.as_ptr()),
        PCWSTR(title.as_ptr()),
        MB_YESNO | MB_ICONQUESTION,
    );
    if result != IDYES {
        return;
    }
    let mut settings = load_settings();
    settings.weather_recent_cities.clear();
    save_settings(settings);
    with_state(hwnd, |state| state.recent_cities.clear());
    populate_recent_cities(hwnd);
}

fn city_display_label(city: &WeatherCity) -> String {
    let location = city_location(city);
    if location.is_empty() {
        city.name.clone()
    } else {
        format!("{} - {}", city.name, location)
    }
}

fn city_location(city: &WeatherCity) -> String {
    [city.admin1.trim(), city.country.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn selected_index(combo: HWND) -> Option<usize> {
    let result = crate::send_message_w_safe(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0));
    (result.0 >= 0).then_some(result.0 as usize)
}

fn selected_list_index(list: HWND) -> Option<usize> {
    let result = crate::send_message_w_safe(list, LB_GETCURSEL, WPARAM(0), LPARAM(0));
    (result.0 >= 0).then_some(result.0 as usize)
}

fn with_state<R>(hwnd: HWND, callback: impl FnOnce(&mut WeatherDialogState) -> R) -> Option<R> {
    let pointer = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut WeatherDialogState;
    if pointer.is_null() {
        None
    } else {
        Some(callback(unsafe { &mut *pointer }))
    }
}

fn create_static(parent: HWND, text: &str, id: usize) -> HWND {
    let text = to_wide(text);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            PCWSTR(text.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            0,
            0,
            parent,
            HMENU(id as isize),
            HINSTANCE(0),
            None,
        )
    }
}

fn create_edit(parent: HWND, id: usize, multiline_read_only: bool) -> HWND {
    let mut style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER;
    if multiline_read_only {
        style |= WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY) as u32) | WS_VSCROLL;
    } else {
        style |= WINDOW_STYLE(ES_AUTOHSCROLL as u32);
    }
    unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR::null(),
            style,
            0,
            0,
            0,
            0,
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
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(0x0001 | 0x00200000 | 0x00100000),
            0,
            0,
            0,
            0,
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
            WS_EX_CLIENTEDGE,
            w!("COMBOBOX"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(0x0003),
            0,
            0,
            0,
            240,
            parent,
            HMENU(id as isize),
            HINSTANCE(0),
            None,
        )
    }
}

fn create_button(parent: HWND, text: &str, id: usize, default: bool) -> HWND {
    let text = to_wide(text);
    let mut style = WS_CHILD | WS_VISIBLE | WS_TABSTOP;
    if default {
        style |= WINDOW_STYLE(BS_DEFPUSHBUTTON as u32);
    }
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            PCWSTR(text.as_ptr()),
            style,
            0,
            0,
            0,
            0,
            parent,
            HMENU(id as isize),
            HINSTANCE(0),
            None,
        )
    }
}

fn layout(hwnd: HWND) {
    let mut rectangle = Default::default();
    crate::log_if_err!(crate::get_client_rect_safe(hwnd, &mut rectangle));
    let width = rectangle.right - rectangle.left;
    let height = rectangle.bottom - rectangle.top;
    let margin = 10;
    let label_width = 135;
    let row_height = 27;
    let button_width = 105;
    let field_x = margin + label_width;
    let field_width = width - (margin * 2) - label_width;
    let mut y = margin;

    move_id(hwnd, ID_LABEL_CITY, margin, y + 5, label_width, row_height);
    move_id(
        hwnd,
        ID_EDIT_CITY,
        field_x,
        y,
        field_width - button_width - 8,
        row_height,
    );
    move_id(
        hwnd,
        ID_BUTTON_SEARCH,
        width - margin - button_width,
        y,
        button_width,
        row_height,
    );
    y += row_height + 10;

    move_id(
        hwnd,
        ID_LABEL_RECENT,
        margin,
        y + 5,
        label_width,
        row_height,
    );
    move_id(hwnd, ID_LIST_RECENT, field_x, y, field_width, 92);
    y += 102;

    let half_width = (field_width - 10) / 2;
    move_id(hwnd, ID_LABEL_DAY, margin, y + 5, label_width, row_height);
    move_id(hwnd, ID_COMBO_DAY, field_x, y, half_width, row_height);
    move_id(
        hwnd,
        ID_LABEL_UNIT,
        field_x + half_width + 10,
        y + 5,
        105,
        row_height,
    );
    move_id(
        hwnd,
        ID_COMBO_UNIT,
        field_x + half_width + 115,
        y,
        (field_width - half_width - 115).max(110),
        row_height,
    );
    y += row_height + 10;

    move_id(
        hwnd,
        ID_LABEL_FORECAST,
        margin,
        y + 5,
        label_width,
        row_height,
    );
    let close_height = 30;
    let forecast_height = (height - y - margin - close_height - 10).max(120);
    move_id(
        hwnd,
        ID_EDIT_FORECAST,
        field_x,
        y,
        field_width,
        forecast_height,
    );
    move_id(
        hwnd,
        ID_BUTTON_CLOSE,
        width - margin - button_width,
        height - margin - close_height,
        button_width,
        close_height,
    );
}

fn move_id(hwnd: HWND, id: usize, x: i32, y: i32, width: i32, height: i32) {
    let control = unsafe { GetDlgItem(hwnd, id as i32) };
    if control.0 != 0 {
        crate::log_if_err!(unsafe { MoveWindow(control, x, y, width, height, true) });
    }
}

fn get_control_text(hwnd: HWND) -> String {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return String::new();
    }
    let mut buffer = vec![0u16; length as usize + 1];
    let written = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    String::from_utf16_lossy(&buffer[..written as usize])
}

fn set_control_text(hwnd: HWND, text: &str) {
    let text = to_wide(text);
    crate::log_if_err!(crate::set_window_text_w_safe(hwnd, PCWSTR(text.as_ptr())));
}

fn set_forecast_text(hwnd: HWND, text: &str) {
    if let Some(edit_forecast) = with_state(hwnd, |state| state.edit_forecast) {
        set_control_text(edit_forecast, text);
    }
}

fn tr(language: Language, key: &str) -> String {
    let value = i18n::tr(language, key);
    if value == key {
        match key {
            "weather.title" => "Weather".to_string(),
            "weather.close" => "Close".to_string(),
            _ => key.to_string(),
        }
    } else {
        value
    }
}
