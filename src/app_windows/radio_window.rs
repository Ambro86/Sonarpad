#[path = "radio_search.rs"]
mod radio_search;
use radio_search::{SearchSession, SourcePage};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::accessibility::{PlayerCommand, handle_player_keyboard, to_wide};
use crate::app_windows::{locale_display_names, scheduled_recording_window};
use crate::i18n;
use crate::launch_stream_url_in_mpv;
use crate::settings::{Language, RadioFavorite, load_settings, save_settings};
use crate::stream_recording::{self, StreamRecordingKind};
use serde::{Deserialize, Deserializer};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use url::Url;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, DEFAULT_GUI_FONT, HBRUSH, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, BS_DEFPUSHBUTTON, CREATESTRUCTW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, ES_AUTOHSCROLL, GWLP_USERDATA,
    GetCursorPos, GetMessageW, GetWindowLongPtrW, HMENU, IDC_ARROW, IsDialogMessageW, IsWindow,
    LoadCursorW, MB_ICONINFORMATION, MB_OK, MF_STRING, MoveWindow, RegisterClassW, SW_HIDE,
    SW_SHOW, SetWindowLongPtrW, ShowWindow, TPM_NONOTIFY, TPM_RETURNCMD, TrackPopupMenu,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU,
    WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_NEXTDLGCTL, WM_SETFOCUS, WM_SETFONT,
    WM_SIZE, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME,
    WS_OVERLAPPED, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

const CLASS_NAME: &str = "SonarpadRadioWindow";
const RADIO_RESULTS_PAGE_SIZE: usize = 25;
const RADIO_BROWSER_LIMIT: &str = "100";
const COMMUNITY_RADIOS_URL: &str = "https://sonarpad.com/api/get_community_radios.php";

const ID_EDIT_SEARCH: usize = 1001;
const ID_COMBO_LANGUAGE: usize = 1002;
const ID_LIST_RESULTS: usize = 1003;
const ID_COMBO_GENRE: usize = 1004;
const ID_BUTTON_SEARCH: usize = 1005;
const ID_LIST_FAVORITES: usize = 1008;
const ID_BUTTON_CLOSE: usize = 1011;
const ID_BUTTON_ADD_COMMUNITY: usize = 1017;
const ID_COMBO_BROWSE_MODE: usize = 1018;
const ID_COMBO_COUNTRY: usize = 1019;
const ID_EDIT_CITY: usize = 1020;
const ID_BUTTON_RECORDINGS: usize = 1021;
const ID_BUTTON_RESET_FILTERS: usize = 1022;
const ID_LABEL_BROWSE_MODE: usize = 1023;
const ID_LABEL_COUNTRY: usize = 1024;
const ID_LABEL_CITY: usize = 1025;
const ID_LABEL_RESULTS: usize = 1026;

const ADD_RADIO_CLASS_NAME: &str = "SonarpadAddCommunityRadioWindow";
const ID_ADD_NAME: usize = 2101;
const ID_ADD_URL: usize = 2102;
const ID_ADD_LANGUAGE: usize = 2103;
const ID_ADD_GENRE: usize = 2104;
const ID_ADD_SUBMIT: usize = 2105;
const ID_ADD_CANCEL: usize = 2106;
const ID_ADD_LABEL_NAME: usize = 2107;
const ID_ADD_LABEL_URL: usize = 2108;
const ID_ADD_LABEL_LANGUAGE: usize = 2109;
const ID_ADD_LABEL_GENRE: usize = 2110;
const ID_LABEL_PAGE: usize = 1012;
const ID_LABEL_SEARCH: usize = 1013;
const ID_LABEL_LANGUAGE: usize = 1014;
const ID_LABEL_GENRE: usize = 1015;
const ID_LABEL_FAVORITES: usize = 1016;

const CB_ADDSTRING: u32 = 0x0143;
const CB_RESETCONTENT: u32 = 0x014B;
const CB_SETCURSEL: u32 = 0x014E;
const CB_GETCURSEL: u32 = 0x0147;
const LB_ADDSTRING: u32 = 0x0180;
const LB_RESETCONTENT: u32 = 0x0184;
const LB_SETCURSEL: u32 = 0x0186;
const LB_GETCURSEL: u32 = 0x0188;
const WM_RADIO_FOCUS_RESULTS: u32 = WM_APP + 77;
const WM_RADIO_FOCUS_FAVORITES: u32 = WM_APP + 79;
const WM_RADIO_SEARCH_COMPLETE: u32 = WM_APP + 78;
const WM_RADIO_DIRECTORY_COMPLETE: u32 = WM_APP + 80;
const RADIO_REFOCUS_DELAYS_MS: &[u64] = &[150, 500];
const ID_CONTEXT_ADD_FAVORITE: usize = 1;
const ID_CONTEXT_REMOVE_FAVORITE: usize = 2;
const ID_CONTEXT_COPY_STREAM_URL: usize = 3;
const ID_CONTEXT_RECORD_AND_PLAY: usize = 4;
const ID_CONTEXT_SCHEDULE_RECORDING: usize = 5;

#[derive(Clone, Deserialize)]
struct RadioStation {
    name: String,
    stream_url: String,
}

#[derive(Deserialize)]
struct CommunityRadioApiResponse {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    message_code: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    error_code: String,
    #[serde(default)]
    error: String,
}

#[derive(Deserialize)]
struct CommunityRadioStation {
    #[serde(default)]
    name: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    language: String,
    #[serde(default)]
    genre: String,
    #[serde(default)]
    genre_label: String,
}

#[derive(Deserialize)]
struct RadioBrowserStation {
    #[serde(default)]
    name: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    url_resolved: String,
    #[serde(default)]
    tags: String,
    #[serde(default)]
    lastcheckok: i32,
}

struct AddRadioDialogState {
    parent: HWND,
    language: Language,
    edit_name: HWND,
    edit_url: HWND,
    combo_language: HWND,
    combo_genre: HWND,
}

struct RadioDialogState {
    parent: HWND,
    language: Language,
    edit_search: HWND,
    combo_browse_mode: HWND,
    combo_language: HWND,
    combo_country: HWND,
    edit_city: HWND,
    combo_genre: HWND,
    list_favorites: HWND,
    list_results: HWND,
    label_page: HWND,
    favorite_results: Vec<RadioFavorite>,
    all_results: Vec<RadioFavorite>,
    page: usize,
    search_session: Option<SearchSession>,
    search_id: u64,
    search_loading: bool,
    languages: Vec<(String, String)>,
    countries: Vec<(String, String)>,
    player_return_focus: Option<RadioReturnFocus>,
}

struct RadioSearchComplete {
    language: Language,
    session: SearchSession,
    page: usize,
    search_id: u64,
    error: Option<String>,
}

struct RadioDirectoryComplete {
    languages: Vec<(String, String)>,
    countries: Vec<(String, String)>,
}

#[derive(Clone, Deserialize)]
struct RadioBrowserDirectoryEntry {
    #[serde(default)]
    name: String,
    #[serde(default)]
    iso_639: Option<String>,
    #[serde(default, deserialize_with = "deserialize_u32_from_any")]
    stationcount: u32,
}

#[derive(Clone, Deserialize)]
struct RadioBrowserCountryEntry {
    #[serde(default)]
    name: String,
    #[serde(default, deserialize_with = "deserialize_u32_from_any")]
    stationcount: u32,
}

#[derive(Clone)]
struct RadioDirectoryOption {
    code: String,
    label: String,
    station_count: u32,
}

fn deserialize_u32_from_any<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Number(number) => number.as_u64().unwrap_or_default() as u32,
        serde_json::Value::String(text) => text.trim().parse::<u32>().unwrap_or_default(),
        _ => 0,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RadioBrowseMode {
    Language,
    Country,
    City,
    Genre,
}

#[derive(Clone, Copy)]
struct GenreOption {
    key: &'static str,
    label: &'static str,
    value: &'static str,
    tag: Option<&'static str>,
}

const GENRE_OPTIONS: &[GenreOption] = &[
    GenreOption {
        key: "radio.genre.all",
        label: "Tutti i generi",
        value: "all",
        tag: None,
    },
    GenreOption {
        key: "radio.genre.news",
        label: "Notizie",
        value: "news",
        tag: Some("news"),
    },
    GenreOption {
        key: "radio.genre.music",
        label: "Musica",
        value: "music",
        tag: Some("music"),
    },
    GenreOption {
        key: "radio.genre.sport",
        label: "Sport",
        value: "sport",
        tag: Some("sport"),
    },
    GenreOption {
        key: "radio.genre.talk",
        label: "Talk e approfondimenti",
        value: "talk",
        tag: Some("talk"),
    },
    GenreOption {
        key: "radio.genre.pop",
        label: "Pop",
        value: "pop",
        tag: Some("pop"),
    },
    GenreOption {
        key: "radio.genre.rock",
        label: "Rock",
        value: "rock",
        tag: Some("rock"),
    },
    GenreOption {
        key: "radio.genre.classical",
        label: "Classica",
        value: "classical",
        tag: Some("classical"),
    },
    GenreOption {
        key: "radio.genre.jazz",
        label: "Jazz",
        value: "jazz",
        tag: Some("jazz"),
    },
    GenreOption {
        key: "radio.genre.dance",
        label: "Dance",
        value: "dance",
        tag: Some("dance"),
    },
    GenreOption {
        key: "radio.genre.blues",
        label: "Blues",
        value: "blues",
        tag: Some("blues"),
    },
    GenreOption {
        key: "radio.genre.country",
        label: "Country",
        value: "country",
        tag: Some("country"),
    },
    GenreOption {
        key: "radio.genre.hiphop",
        label: "Hip hop",
        value: "hiphop",
        tag: Some("hiphop"),
    },
    GenreOption {
        key: "radio.genre.electronic",
        label: "Elettronica",
        value: "electronic",
        tag: Some("electronic"),
    },
    GenreOption {
        key: "radio.genre.latin",
        label: "Latina",
        value: "latin",
        tag: Some("latin"),
    },
    GenreOption {
        key: "radio.genre.reggae",
        label: "Reggae",
        value: "reggae",
        tag: Some("reggae"),
    },
    GenreOption {
        key: "radio.genre.metal",
        label: "Metal",
        value: "metal",
        tag: Some("metal"),
    },
    GenreOption {
        key: "radio.genre.folk",
        label: "Folk",
        value: "folk",
        tag: Some("folk"),
    },
    GenreOption {
        key: "radio.genre.religion",
        label: "Religione",
        value: "religion",
        tag: Some("religion"),
    },
    GenreOption {
        key: "radio.genre.local",
        label: "Locale",
        value: "local",
        tag: Some("local"),
    },
    GenreOption {
        key: "radio.genre.culture",
        label: "Cultura",
        value: "culture",
        tag: Some("culture"),
    },
    GenreOption {
        key: "radio.genre.oldies",
        label: "Anni 70 / 80 / 90",
        value: "oldies",
        tag: Some("oldies"),
    },
    GenreOption {
        key: "radio.genre.kids",
        label: "Bambini",
        value: "kids",
        tag: Some("kids"),
    },
    GenreOption {
        key: "radio.genre.ambient",
        label: "Ambient",
        value: "ambient",
        tag: Some("ambient"),
    },
    GenreOption {
        key: "radio.genre.custom",
        label: "Altro genere...",
        value: "custom",
        tag: None,
    },
];

#[derive(Clone, Copy)]
struct CommunityLanguageOption {
    key: &'static str,
    label: &'static str,
    value: &'static str,
}

const COMMUNITY_LANGUAGE_OPTIONS: &[CommunityLanguageOption] = &[
    CommunityLanguageOption {
        key: "radio.community_lang.it",
        label: "Italiano",
        value: "italian",
    },
    CommunityLanguageOption {
        key: "radio.community_lang.en",
        label: "Inglese",
        value: "english",
    },
    CommunityLanguageOption {
        key: "radio.lang.tr",
        label: "Turco",
        value: "turkish",
    },
    CommunityLanguageOption {
        key: "radio.community_lang.es",
        label: "Spagnolo",
        value: "spanish",
    },
    CommunityLanguageOption {
        key: "radio.community_lang.fr",
        label: "Francese",
        value: "french",
    },
    CommunityLanguageOption {
        key: "radio.community_lang.de",
        label: "Tedesco",
        value: "german",
    },
    CommunityLanguageOption {
        key: "radio.community_lang.pt",
        label: "Portoghese",
        value: "portuguese",
    },
    CommunityLanguageOption {
        key: "radio.lang.sv",
        label: "Svedese",
        value: "swedish",
    },
    CommunityLanguageOption {
        key: "radio.lang.vi",
        label: "Vietnamita",
        value: "vietnamese",
    },
    CommunityLanguageOption {
        key: "radio.lang.cs",
        label: "Ceco",
        value: "czech",
    },
    CommunityLanguageOption {
        key: "radio.lang.pl",
        label: "Polacco",
        value: "polish",
    },
    CommunityLanguageOption {
        key: "radio.lang.sr",
        label: "Serbo",
        value: "serbian",
    },
    CommunityLanguageOption {
        key: "radio.lang.uk",
        label: "Ucraino",
        value: "ukrainian",
    },
    CommunityLanguageOption {
        key: "radio.lang.lt",
        label: "Lituano",
        value: "lithuanian",
    },
    CommunityLanguageOption {
        key: "radio.lang.ru",
        label: "Russo",
        value: "russian",
    },
    CommunityLanguageOption {
        key: "radio.lang.zh",
        label: "Cinese",
        value: "chinese",
    },
    CommunityLanguageOption {
        key: "radio.community_lang.hi",
        label: "Hindi",
        value: "hindi",
    },
];

pub fn open(parent: HWND) {
    unsafe {
        let language = load_settings().language;
        let hinstance = HINSTANCE(GetModuleHandleW(None).map(|m| m.0).unwrap_or(0));
        let class_name = to_wide(CLASS_NAME);
        let wc = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let title = to_wide(&tr(language, "radio.title", "Radio da tutto il mondo"));
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            820,
            620,
            parent,
            HMENU(0),
            hinstance,
            Some(parent.0 as *const std::ffi::c_void),
        );
        if hwnd.0 == 0 {
            return;
        }
        ShowWindow(hwnd, SW_SHOW);

        let mut msg = Default::default();
        while IsWindow(hwnd).as_bool() && GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&msg) {
                continue;
            }
            match route_player_keyboard(hwnd, parent, &msg) {
                RadioLoopAction::NotHandled => {}
                RadioLoopAction::Handled => continue,
            }
            if handle_enter_key(hwnd, &msg) {
                continue;
            }
            if handle_escape_key(hwnd, &msg) {
                continue;
            }
            if crate::handle_focused_edit_shortcut(&msg) {
                continue;
            }
            if !IsDialogMessageW(hwnd, &msg).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

enum RadioLoopAction {
    NotHandled,
    Handled,
}

fn handle_enter_key(hwnd: HWND, msg: &windows::Win32::UI::WindowsAndMessaging::MSG) -> bool {
    if msg.message != WM_KEYDOWN || msg.wParam.0 as u32 != VK_RETURN.0 as u32 {
        return false;
    }
    let target = with_radio_state(hwnd, |state| {
        if msg.hwnd == state.list_favorites {
            Some(RadioEnterTarget::Favorites)
        } else if msg.hwnd == state.list_results {
            Some(RadioEnterTarget::Results)
        } else if msg.hwnd == state.edit_search || msg.hwnd == state.edit_city {
            Some(RadioEnterTarget::Search)
        } else if msg.hwnd == state.combo_browse_mode
            || crate::is_child_safe(state.combo_browse_mode, msg.hwnd)
            || msg.hwnd == state.combo_language
            || crate::is_child_safe(state.combo_language, msg.hwnd)
            || msg.hwnd == state.combo_country
            || crate::is_child_safe(state.combo_country, msg.hwnd)
            || msg.hwnd == state.combo_genre
            || crate::is_child_safe(state.combo_genre, msg.hwnd)
        {
            Some(RadioEnterTarget::Language)
        } else {
            None
        }
    })
    .flatten();
    match target {
        Some(RadioEnterTarget::Favorites) => open_selected_favorite(hwnd),
        Some(RadioEnterTarget::Results) => open_selected(hwnd),
        Some(RadioEnterTarget::Search) => search(hwnd),
        Some(RadioEnterTarget::Language) => show_all_and_focus(hwnd),
        None => return false,
    }
    true
}

fn handle_escape_key(hwnd: HWND, msg: &windows::Win32::UI::WindowsAndMessaging::MSG) -> bool {
    if msg.message != WM_KEYDOWN || msg.wParam.0 as u32 != VK_ESCAPE.0 as u32 {
        return false;
    }
    crate::log_debug(&format!("Radio: ESC closes dialog hwnd={:?}", hwnd));
    crate::log_if_err!(crate::post_message_w_safe(
        hwnd,
        WM_CLOSE,
        WPARAM(0),
        LPARAM(0)
    ));
    true
}

enum RadioEnterTarget {
    Favorites,
    Results,
    Search,
    Language,
}

fn route_player_keyboard(
    hwnd: HWND,
    parent: HWND,
    msg: &windows::Win32::UI::WindowsAndMessaging::MSG,
) -> RadioLoopAction {
    let is_escape = msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_ESCAPE.0 as u32;
    if is_escape {
        crate::log_debug(&format!(
            "Radio: ESC in loop hwnd={:?} msg_hwnd={:?}",
            hwnd, msg.hwnd
        ));
    }
    let Some((has_player, skip_seconds)) = crate::with_state(parent, |state| {
        (
            state.active_audiobook.is_some() || state.active_mpv_session.is_some(),
            state.settings.audiobook_skip_seconds,
        )
    }) else {
        if is_escape {
            crate::log_debug("Radio: ESC parent AppState unavailable");
        }
        return RadioLoopAction::NotHandled;
    };
    if !has_player {
        return RadioLoopAction::NotHandled;
    }
    let command = handle_player_keyboard(msg, skip_seconds);
    if is_escape {
        crate::log_debug(&format!("Radio: ESC player command {:?}", command));
    }
    if matches!(command, PlayerCommand::None) {
        return RadioLoopAction::NotHandled;
    }
    let stop = matches!(command, PlayerCommand::Stop);
    crate::handle_player_command(parent, command);
    if stop {
        crate::log_debug(
            "Radio: ESC stopped player, closing player document and restoring source list focus",
        );
        crate::editor_manager::close_current_document(parent);
        restore_player_return_focus(hwnd);
    }
    RadioLoopAction::Handled
}

fn schedule_results_refocus(hwnd: HWND) {
    let hwnd_value = hwnd.0;
    std::thread::spawn(move || {
        for delay in RADIO_REFOCUS_DELAYS_MS {
            std::thread::sleep(Duration::from_millis(*delay));
            let hwnd = HWND(hwnd_value);
            crate::log_if_err!(crate::post_message_w_safe(
                hwnd,
                WM_RADIO_FOCUS_RESULTS,
                WPARAM(0),
                LPARAM(0)
            ));
        }
    });
}

fn focus_search_edit(hwnd: HWND) {
    crate::set_foreground_window_safe(hwnd);
    if let Some(edit_search) = with_radio_state(hwnd, |state| state.edit_search) {
        crate::log_debug(&format!(
            "Radio: focus search field hwnd={:?} edit={:?} before={:?}",
            hwnd,
            edit_search,
            crate::get_focus_safe()
        ));
        crate::set_focus_safe(edit_search);
        crate::send_message_w_safe(
            hwnd,
            WM_NEXTDLGCTL,
            WPARAM(edit_search.0 as usize),
            LPARAM(1),
        );
        crate::log_debug(&format!(
            "Radio: focus search field after={:?}",
            crate::get_focus_safe()
        ));
    } else {
        crate::log_debug("Radio: focus search field failed, state unavailable");
    }
}

fn focus_results_list(hwnd: HWND) {
    crate::set_foreground_window_safe(hwnd);
    if let Some(list_results) = with_radio_state(hwnd, |state| state.list_results) {
        crate::log_debug(&format!(
            "Radio: focus results list hwnd={:?} list={:?} before={:?}",
            hwnd,
            list_results,
            crate::get_focus_safe()
        ));
        crate::set_focus_safe(list_results);
        crate::send_message_w_safe(
            hwnd,
            WM_NEXTDLGCTL,
            WPARAM(list_results.0 as usize),
            LPARAM(1),
        );
        crate::log_debug(&format!(
            "Radio: focus results list after={:?}",
            crate::get_focus_safe()
        ));
    } else {
        crate::log_debug("Radio: focus results list failed, state unavailable");
    }
}

fn schedule_favorites_refocus(hwnd: HWND) {
    let hwnd_value = hwnd.0;
    std::thread::spawn(move || {
        for delay in RADIO_REFOCUS_DELAYS_MS {
            std::thread::sleep(Duration::from_millis(*delay));
            let hwnd = HWND(hwnd_value);
            crate::log_if_err!(crate::post_message_w_safe(
                hwnd,
                WM_RADIO_FOCUS_FAVORITES,
                WPARAM(0),
                LPARAM(0)
            ));
        }
    });
}

fn focus_favorites_list(hwnd: HWND) {
    crate::set_foreground_window_safe(hwnd);
    if let Some(list_favorites) = with_radio_state(hwnd, |state| state.list_favorites) {
        crate::set_focus_safe(list_favorites);
        crate::send_message_w_safe(
            hwnd,
            WM_NEXTDLGCTL,
            WPARAM(list_favorites.0 as usize),
            LPARAM(1),
        );
    }
}

fn show_station_context_menu(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> bool {
    let target = HWND(wparam.0 as isize);
    let Some((list_favorites, list_results, language)) = with_radio_state(hwnd, |state| {
        (state.list_favorites, state.list_results, state.language)
    }) else {
        return false;
    };
    if target.0 != 0 && target != hwnd && target != list_favorites && target != list_results {
        return false;
    }
    let list_kind = if target == list_favorites {
        RadioListKind::Favorites
    } else if target == list_results {
        RadioListKind::Results
    } else {
        selected_radio_list_kind(hwnd).unwrap_or(RadioListKind::Results)
    };
    match list_kind {
        RadioListKind::Favorites => crate::set_focus_safe(list_favorites),
        RadioListKind::Results => crate::set_focus_safe(list_results),
    }
    let Some(item) = selected_result(hwnd, list_kind) else {
        return false;
    };
    let is_fav = load_settings()
        .radio_favorites
        .iter()
        .any(|f| f.stream_url == item.stream_url);

    let menu = match unsafe { CreatePopupMenu() } {
        Ok(menu) => menu,
        Err(err) => {
            crate::log_debug(&format!("Radio: CreatePopupMenu failed: {}", err));
            return false;
        }
    };

    if !is_fav {
        let add_label = to_wide(&tr(language, "radio.add_favorite", "Aggiungi ai preferiti"));
        if let Err(err) = unsafe {
            AppendMenuW(
                menu,
                MF_STRING,
                ID_CONTEXT_ADD_FAVORITE,
                PCWSTR(add_label.as_ptr()),
            )
        } {
            crate::log_debug(&format!("Radio: AppendMenuW add favorite failed: {}", err));
            crate::log_if_err!(unsafe { DestroyMenu(menu) });
            return false;
        }
    } else {
        let remove_label = to_wide(&tr(
            language,
            "radio.remove_favorite",
            "Rimuovi dai preferiti",
        ));
        if let Err(err) = unsafe {
            AppendMenuW(
                menu,
                MF_STRING,
                ID_CONTEXT_REMOVE_FAVORITE,
                PCWSTR(remove_label.as_ptr()),
            )
        } {
            crate::log_debug(&format!(
                "Radio: AppendMenuW remove favorite failed: {}",
                err
            ));
            crate::log_if_err!(unsafe { DestroyMenu(menu) });
            return false;
        }
    }

    let record_label = to_wide(&tr(
        language,
        "radio.record_and_play",
        "Registra e riproduci radio",
    ));
    if let Err(err) = unsafe {
        AppendMenuW(
            menu,
            MF_STRING,
            ID_CONTEXT_RECORD_AND_PLAY,
            PCWSTR(record_label.as_ptr()),
        )
    } {
        crate::log_debug(&format!("Radio: AppendMenuW record failed: {}", err));
        crate::log_if_err!(unsafe { DestroyMenu(menu) });
        return false;
    }

    let schedule_label = to_wide(&tr(
        language,
        "scheduled_recording.action",
        "Programma registrazione",
    ));
    if let Err(err) = unsafe {
        AppendMenuW(
            menu,
            MF_STRING,
            ID_CONTEXT_SCHEDULE_RECORDING,
            PCWSTR(schedule_label.as_ptr()),
        )
    } {
        crate::log_debug(&format!(
            "Radio: AppendMenuW schedule recording failed: {}",
            err
        ));
        crate::log_if_err!(unsafe { DestroyMenu(menu) });
        return false;
    }

    let copy_label = to_wide(&tr(language, "radio.copy_audio_url", "Copia URL audio"));
    if let Err(err) = unsafe {
        AppendMenuW(
            menu,
            MF_STRING,
            ID_CONTEXT_COPY_STREAM_URL,
            PCWSTR(copy_label.as_ptr()),
        )
    } {
        crate::log_debug(&format!(
            "Radio: AppendMenuW copy stream URL failed: {}",
            err
        ));
        crate::log_if_err!(unsafe { DestroyMenu(menu) });
        return false;
    }

    let point = if lparam.0 == -1 {
        let mut pt = POINT::default();
        if let Err(err) = unsafe { GetCursorPos(&mut pt) } {
            crate::log_debug(&format!("Radio: GetCursorPos failed: {}", err));
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

    match command.0 as usize {
        ID_CONTEXT_ADD_FAVORITE => add_selected_favorite(hwnd),
        ID_CONTEXT_REMOVE_FAVORITE => remove_selected_favorite(hwnd),
        ID_CONTEXT_COPY_STREAM_URL => copy_selected_stream_url(hwnd),
        ID_CONTEXT_RECORD_AND_PLAY => record_and_play_selected_radio(hwnd),
        ID_CONTEXT_SCHEDULE_RECORDING => schedule_selected_radio(hwnd),
        0 => {}
        other => crate::log_debug(&format!("Radio: unknown context menu command {}", other)),
    }
    true
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_RADIO_FOCUS_RESULTS => {
                crate::log_debug("Radio: deferred focus results message");
                focus_results_list(hwnd);
                LRESULT(0)
            }
            WM_RADIO_FOCUS_FAVORITES => {
                crate::log_debug("Radio: deferred focus favorites message");
                focus_favorites_list(hwnd);
                LRESULT(0)
            }
            WM_RADIO_SEARCH_COMPLETE => {
                let result = Box::from_raw(lparam.0 as *mut RadioSearchComplete);
                finish_radio_search(hwnd, *result);
                LRESULT(0)
            }
            WM_RADIO_DIRECTORY_COMPLETE => {
                let result = Box::from_raw(lparam.0 as *mut RadioDirectoryComplete);
                apply_radio_directory_options(hwnd, *result);
                LRESULT(0)
            }
            WM_CREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                let parent = HWND((*create).lpCreateParams as isize);
                if parent.0 != 0 {
                    crate::with_state(parent, |state| {
                        state.radio_window = hwnd;
                    });
                }
                create_controls(hwnd, parent);
                LRESULT(0)
            }
            WM_SIZE => {
                layout(hwnd);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                focus_favorites_list(hwnd);
                LRESULT(0)
            }
            WM_CONTEXTMENU => {
                if show_station_context_menu(hwnd, wparam, lparam) {
                    LRESULT(0)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let notification = (wparam.0 >> 16) & 0xffff;
                if id == ID_COMBO_BROWSE_MODE && notification == 1 {
                    update_browse_filter_visibility(hwnd);
                    layout(hwnd);
                    return LRESULT(0);
                }
                match id {
                    ID_BUTTON_SEARCH => {
                        search(hwnd);
                        LRESULT(0)
                    }
                    ID_BUTTON_ADD_COMMUNITY => {
                        open_add_community_radio_dialog(hwnd);
                        LRESULT(0)
                    }
                    ID_BUTTON_RECORDINGS => {
                        let Some(language) = with_radio_state(hwnd, |state| state.language) else {
                            return LRESULT(0);
                        };
                        let playback_parent =
                            with_radio_state(hwnd, |state| state.parent).unwrap_or(hwnd);
                        let recordings_result = stream_recording::open_recordings(
                            hwnd,
                            playback_parent,
                            language,
                            StreamRecordingKind::Radio,
                        );
                        if recordings_result
                            != stream_recording::OpenRecordingsResult::PlaybackStarted
                        {
                            focus_search_edit(hwnd);
                        }
                        LRESULT(0)
                    }
                    ID_BUTTON_RESET_FILTERS => {
                        reset_filters(hwnd);
                        LRESULT(0)
                    }
                    ID_BUTTON_CLOSE => {
                        crate::log_if_err!(DestroyWindow(hwnd));
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_KEYDOWN => {
                if wparam.0 as u32 == VK_ESCAPE.0 as u32 {
                    crate::log_debug(&format!("Radio: ESC in wndproc hwnd={:?}", hwnd));
                    crate::log_if_err!(DestroyWindow(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                crate::log_if_err!(DestroyWindow(hwnd));
                LRESULT(0)
            }
            WM_DESTROY => {
                let parent = with_radio_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
                if parent.0 != 0 {
                    crate::with_state(parent, |state| {
                        if state.radio_window == hwnd {
                            state.radio_window = HWND(0);
                        }
                    });
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
                let parent = with_radio_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
                if parent.0 != 0 {
                    crate::with_state(parent, |state| {
                        if state.radio_window == hwnd {
                            state.radio_window = HWND(0);
                        }
                    });
                }
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RadioDialogState;
                if !ptr.is_null() {
                    let _box = Box::from_raw(ptr);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn open_add_community_radio_dialog(parent: HWND) {
    unsafe {
        let language = load_settings().language;
        let hinstance = HINSTANCE(GetModuleHandleW(None).map(|m| m.0).unwrap_or(0));
        let class_name = to_wide(ADD_RADIO_CLASS_NAME);
        let wc = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(add_radio_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let title = to_wide(&tr(
            language,
            "radio.add_community_title",
            "Aggiungi radio alla comunità Sonarpad",
        ));
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            260,
            parent,
            HMENU(0),
            hinstance,
            Some(parent.0 as *const std::ffi::c_void),
        );
        if hwnd.0 == 0 {
            return;
        }
        ShowWindow(hwnd, SW_SHOW);

        let mut msg = Default::default();
        while IsWindow(hwnd).as_bool() && GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&msg) {
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_ESCAPE.0 as u32 {
                let parent = with_add_radio_state(hwnd, |s| s.parent);
                crate::log_if_err!(DestroyWindow(hwnd));
                if let Some(p) = parent {
                    schedule_favorites_refocus(p);
                }
                continue;
            }
            if crate::handle_focused_edit_shortcut(&msg) {
                continue;
            }
            if !IsDialogMessageW(hwnd, &msg).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

unsafe extern "system" fn add_radio_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                let parent = HWND((*create).lpCreateParams as isize);
                create_add_radio_controls(hwnd, parent);
                LRESULT(0)
            }
            WM_SIZE => {
                layout_add_radio_dialog(hwnd);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                match id {
                    ID_ADD_SUBMIT => {
                        submit_add_community_radio(hwnd);
                        LRESULT(0)
                    }
                    ID_ADD_CANCEL => {
                        let parent = with_add_radio_state(hwnd, |s| s.parent);
                        crate::log_if_err!(DestroyWindow(hwnd));
                        if let Some(p) = parent {
                            schedule_favorites_refocus(p);
                        }
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_KEYDOWN => {
                if wparam.0 as u32 == VK_ESCAPE.0 as u32 {
                    let parent = with_add_radio_state(hwnd, |s| s.parent);
                    crate::log_if_err!(DestroyWindow(hwnd));
                    if let Some(p) = parent {
                        schedule_favorites_refocus(p);
                    }
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                let parent = with_add_radio_state(hwnd, |s| s.parent);
                crate::log_if_err!(DestroyWindow(hwnd));
                if let Some(p) = parent {
                    schedule_favorites_refocus(p);
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AddRadioDialogState;
                if !ptr.is_null() {
                    let _box = Box::from_raw(ptr);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn create_add_radio_controls(hwnd: HWND, parent: HWND) {
    let language = load_settings().language;
    let font = HFONT(crate::get_stock_object_safe(DEFAULT_GUI_FONT).0);

    let label_name = create_static(
        hwnd,
        &tr(language, "radio.add_name", "Nome radio:"),
        ID_ADD_LABEL_NAME,
    );
    let edit_name = create_edit(hwnd, ID_ADD_NAME);
    let label_url = create_static(
        hwnd,
        &tr(language, "radio.add_url", "Indirizzo streaming:"),
        ID_ADD_LABEL_URL,
    );
    let edit_url = create_edit(hwnd, ID_ADD_URL);
    let label_language = create_static(
        hwnd,
        &tr(language, "radio.language", "Lingua:"),
        ID_ADD_LABEL_LANGUAGE,
    );
    let combo_language = create_combo(hwnd, ID_ADD_LANGUAGE);
    let label_genre = create_static(
        hwnd,
        &tr(language, "radio.genre", "Genere:"),
        ID_ADD_LABEL_GENRE,
    );
    let combo_genre = create_combo(hwnd, ID_ADD_GENRE);
    let btn_submit = create_button(
        hwnd,
        &tr(language, "radio.add_submit", "Verifica e aggiungi"),
        ID_ADD_SUBMIT,
        true,
    );
    let btn_cancel = create_button(
        hwnd,
        &tr(language, "radio.cancel", "Annulla"),
        ID_ADD_CANCEL,
        false,
    );

    for hwnd_ctrl in [
        label_name,
        edit_name,
        label_url,
        edit_url,
        label_language,
        combo_language,
        label_genre,
        combo_genre,
        btn_submit,
        btn_cancel,
    ] {
        crate::send_message_w_safe(hwnd_ctrl, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    }

    for option in COMMUNITY_LANGUAGE_OPTIONS {
        let w = to_wide(&tr(language, option.key, option.label));
        crate::send_message_w_safe(
            combo_language,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w.as_ptr() as isize),
        );
    }
    let current_code = match language {
        Language::Italian => "radio.community_lang.it",
        Language::German => "radio.community_lang.de",
        Language::English => "radio.community_lang.en",
        Language::Spanish => "radio.community_lang.es",
        Language::Portuguese | Language::PortugueseBrazilian => "radio.community_lang.pt",
        Language::French => "radio.community_lang.fr",
        Language::Hindi => "radio.community_lang.hi",
        _ => "radio.community_lang.en",
    };
    let default_idx = COMMUNITY_LANGUAGE_OPTIONS
        .iter()
        .position(|opt| opt.key == current_code)
        .unwrap_or(0);
    crate::send_message_w_safe(combo_language, CB_SETCURSEL, WPARAM(default_idx), LPARAM(0));

    for option in GENRE_OPTIONS.iter().filter(|option| option.tag.is_some()) {
        let w = to_wide(&tr(language, option.key, option.label));
        crate::send_message_w_safe(
            combo_genre,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w.as_ptr() as isize),
        );
    }
    crate::send_message_w_safe(combo_genre, CB_SETCURSEL, WPARAM(0), LPARAM(0));

    let state = Box::new(AddRadioDialogState {
        parent,
        language,
        edit_name,
        edit_url,
        combo_language,
        combo_genre,
    });
    crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
    layout_add_radio_dialog(hwnd);
    crate::set_focus_safe(edit_name);
}

fn layout_add_radio_dialog(hwnd: HWND) {
    let mut rc = Default::default();
    crate::log_if_err!(crate::get_client_rect_safe(hwnd, &mut rc));
    let w = rc.right - rc.left;
    let margin = 10;
    let label_w = 145;
    let row_h = 25;
    let mut y = margin;

    move_id(hwnd, ID_ADD_LABEL_NAME, margin, y + 4, label_w, row_h);
    move_id(
        hwnd,
        ID_ADD_NAME,
        margin + label_w,
        y,
        w - margin * 2 - label_w,
        row_h,
    );
    y += 35;
    move_id(hwnd, ID_ADD_LABEL_URL, margin, y + 4, label_w, row_h);
    move_id(
        hwnd,
        ID_ADD_URL,
        margin + label_w,
        y,
        w - margin * 2 - label_w,
        row_h,
    );
    y += 35;
    move_id(hwnd, ID_ADD_LABEL_LANGUAGE, margin, y + 4, label_w, row_h);
    move_id(
        hwnd,
        ID_ADD_LANGUAGE,
        margin + label_w,
        y,
        w - margin * 2 - label_w,
        140,
    );
    y += 38;
    move_id(hwnd, ID_ADD_LABEL_GENRE, margin, y + 4, label_w, row_h);
    move_id(
        hwnd,
        ID_ADD_GENRE,
        margin + label_w,
        y,
        w - margin * 2 - label_w,
        180,
    );
    y += 46;
    move_id(hwnd, ID_ADD_SUBMIT, w - margin - 260, y, 160, 30);
    move_id(hwnd, ID_ADD_CANCEL, w - margin - 90, y, 90, 30);
}

fn with_add_radio_state<R>(hwnd: HWND, f: impl FnOnce(&mut AddRadioDialogState) -> R) -> Option<R> {
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut AddRadioDialogState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn selected_community_language_value(state: &AddRadioDialogState) -> &'static str {
    let idx = crate::send_message_w_safe(state.combo_language, CB_GETCURSEL, WPARAM(0), LPARAM(0))
        .0
        .max(0) as usize;
    COMMUNITY_LANGUAGE_OPTIONS
        .get(idx)
        .map(|option| option.value)
        .unwrap_or("italian")
}

fn selected_community_genre_value(state: &AddRadioDialogState) -> &'static str {
    let idx = crate::send_message_w_safe(state.combo_genre, CB_GETCURSEL, WPARAM(0), LPARAM(0))
        .0
        .max(0) as usize;
    GENRE_OPTIONS
        .iter()
        .filter(|option| option.tag.is_some())
        .nth(idx)
        .map(|option| option.value)
        .unwrap_or("news")
}

fn submit_add_community_radio(hwnd: HWND) {
    let Some((parent, language, name, url, radio_language, genre)) =
        with_add_radio_state(hwnd, |state| {
            (
                state.parent,
                state.language,
                get_edit_text(state.edit_name).trim().to_string(),
                get_edit_text(state.edit_url).trim().to_string(),
                selected_community_language_value(state),
                selected_community_genre_value(state),
            )
        })
    else {
        return;
    };

    if name.is_empty() || url.is_empty() {
        message(
            hwnd,
            "Radio",
            &i18n::tr(language, "radio.add_missing_fields"),
        );
        return;
    }

    match post_community_radio(language, &name, &url, radio_language, genre) {
        Ok(text) => {
            message(hwnd, "Radio", &text);
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            schedule_favorites_refocus(parent);
        }
        Err(err) => message(hwnd, "Radio", &err),
    }
}

fn post_community_radio(
    app_language: Language,
    name: &str,
    url: &str,
    language: &str,
    genre: &str,
) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("Sonarpad/1.0 (https://sonarpad.com)")
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post("https://sonarpad.com/api/add_community_radio.php")
        .form(&[
            ("name", name),
            ("url", url),
            ("language", language),
            ("genre", genre),
            ("ui_language", app_language_code(app_language)),
        ])
        .send()
        .map_err(|e| e.to_string())?;

    let status = response.status();
    let body = response.text().map_err(|e| e.to_string())?;

    let response = serde_json::from_str::<CommunityRadioApiResponse>(&body).map_err(|e| {
        let status = status.to_string();
        if body.trim().is_empty() {
            let error = e.to_string();
            tr_fallback(
                app_language,
                "radio.server_empty_response",
                &[("status", status.as_str()), ("error", error.as_str())],
                "Il server ha risposto con stato {status}, ma senza un messaggio leggibile: {error}",
            )
        } else {
            tr_fallback(
                app_language,
                "radio.server_invalid_json",
                &[("status", status.as_str()), ("body", body.as_str())],
                "Il server ha risposto con stato {status}, ma la risposta non è JSON valida: {body}",
            )
        }
    })?;

    if !status.is_success() && response.error.trim().is_empty() {
        let status = status.to_string();
        return Err(tr_fallback(
            app_language,
            "radio.server_rejected",
            &[("status", status.as_str())],
            "Il server ha rifiutato la richiesta con stato {status}.",
        ));
    };

    if response.ok {
        if !response.message_code.trim().is_empty() {
            crate::log_debug(&format!(
                "Community radio API success code: {}",
                response.message_code.trim()
            ));
        }

        if response.message.trim().is_empty() {
            Ok(tr(
                app_language,
                "radio.community_added",
                "Radio aggiunta correttamente alla comunità Sonarpad.",
            ))
        } else {
            Ok(response.message)
        }
    } else if response.error.trim().is_empty() {
        Err(i18n::tr(app_language, "radio.community_add_error"))
    } else {
        if !response.error_code.trim().is_empty() {
            crate::log_debug(&format!(
                "Community radio API error code: {}",
                response.error_code.trim()
            ));
        }

        Err(response.error)
    }
}

fn app_language_code(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::German => "de",
        Language::English => "en",
        Language::Spanish => "es",
        Language::Portuguese | Language::PortugueseBrazilian => "pt",
        Language::Swedish => "sv",
        Language::Vietnamese => "vi",
        Language::Czech => "cs",
        Language::Polish => "pl",
        Language::French => "fr",
        Language::Serbian => "sr",
        Language::Ukrainian => "uk",
        Language::Lithuanian => "lt",
        Language::Russian => "ru",
        Language::Chinese => "zh",
        Language::Hindi => "hi",
    }
}

fn create_controls(hwnd: HWND, parent: HWND) {
    let language = load_settings().language;
    let font = HFONT(crate::get_stock_object_safe(DEFAULT_GUI_FONT).0);
    let languages = radio_menu_languages(language);
    let countries = radio_menu_countries(language);

    let label_favorites = create_static(
        hwnd,
        &tr(language, "radio.favorites", "Preferiti:"),
        ID_LABEL_FAVORITES,
    );
    let list_favorites = create_listbox(hwnd, ID_LIST_FAVORITES);
    let label_search = create_static(
        hwnd,
        &tr(language, "radio.search_text", "Cerca radio:"),
        ID_LABEL_SEARCH,
    );
    let edit_search = create_edit(hwnd, ID_EDIT_SEARCH);
    let label_mode = create_static(
        hwnd,
        &tr(language, "radio.browse_mode", "Cerca per:"),
        ID_LABEL_BROWSE_MODE,
    );
    let combo_browse_mode = create_combo(hwnd, ID_COMBO_BROWSE_MODE);
    let label_lang = create_static(
        hwnd,
        &tr(language, "radio.language", "Lingua:"),
        ID_LABEL_LANGUAGE,
    );
    let combo_language = create_combo(hwnd, ID_COMBO_LANGUAGE);
    let label_country = create_static(
        hwnd,
        &tr(language, "radio.country", "Nazione:"),
        ID_LABEL_COUNTRY,
    );
    let combo_country = create_combo(hwnd, ID_COMBO_COUNTRY);
    let label_city = create_static(hwnd, &tr(language, "radio.city", "Città:"), ID_LABEL_CITY);
    let edit_city = create_edit(hwnd, ID_EDIT_CITY);
    let label_genre = create_static(
        hwnd,
        &tr(language, "radio.genre", "Genere:"),
        ID_LABEL_GENRE,
    );
    let combo_genre = create_combo(hwnd, ID_COMBO_GENRE);
    let label_results = create_static(
        hwnd,
        &tr(language, "find_in_files.results", "Risultati:"),
        ID_LABEL_RESULTS,
    );
    let list_results = create_listbox(hwnd, ID_LIST_RESULTS);
    let label_page = create_static(hwnd, "", ID_LABEL_PAGE);
    let btn_search = create_button(
        hwnd,
        &tr(language, "radio.search", "Ricerca"),
        ID_BUTTON_SEARCH,
        true,
    );
    let btn_reset = create_button(
        hwnd,
        &tr(language, "radio.reset_filters", "Reimposta filtri"),
        ID_BUTTON_RESET_FILTERS,
        false,
    );
    let btn_recordings = create_button(
        hwnd,
        &tr(language, "radio.recordings", "Registrazioni radio"),
        ID_BUTTON_RECORDINGS,
        false,
    );
    let btn_add = create_button(
        hwnd,
        &tr(language, "radio.add_community", "Aggiungi radio"),
        ID_BUTTON_ADD_COMMUNITY,
        false,
    );
    let btn_close = create_button(
        hwnd,
        &tr(language, "radio.close", "Chiudi"),
        ID_BUTTON_CLOSE,
        false,
    );

    for hwnd_ctrl in [
        label_favorites,
        list_favorites,
        label_search,
        edit_search,
        label_mode,
        combo_browse_mode,
        label_lang,
        combo_language,
        label_country,
        combo_country,
        label_city,
        edit_city,
        label_genre,
        combo_genre,
        label_results,
        list_results,
        label_page,
        btn_search,
        btn_reset,
        btn_recordings,
        btn_add,
        btn_close,
    ] {
        crate::send_message_w_safe(hwnd_ctrl, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    }

    for label in [
        tr(language, "radio.browse_language", "Lingua"),
        tr(language, "radio.browse_country", "Nazione"),
        tr(language, "radio.browse_city", "Città"),
        tr(language, "radio.browse_genre", "Genere"),
    ] {
        let wide = to_wide(&label);
        crate::send_message_w_safe(
            combo_browse_mode,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    crate::send_message_w_safe(combo_browse_mode, CB_SETCURSEL, WPARAM(0), LPARAM(0));

    for (_, label) in &languages {
        let wide = to_wide(label);
        crate::send_message_w_safe(
            combo_language,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    let current_code = app_language_code(language);
    let default_language_index = languages
        .iter()
        .position(|(code, _)| code == current_code)
        .unwrap_or(0);
    crate::send_message_w_safe(
        combo_language,
        CB_SETCURSEL,
        WPARAM(default_language_index),
        LPARAM(0),
    );

    for (_, label) in &countries {
        let wide = to_wide(label);
        crate::send_message_w_safe(
            combo_country,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    let default_country = default_country_code(language);
    let default_country_index = countries
        .iter()
        .position(|(code, _)| code == default_country)
        .unwrap_or(0);
    crate::send_message_w_safe(
        combo_country,
        CB_SETCURSEL,
        WPARAM(default_country_index),
        LPARAM(0),
    );

    for genre in GENRE_OPTIONS {
        let wide = to_wide(&tr(language, genre.key, genre.label));
        crate::send_message_w_safe(
            combo_genre,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    crate::send_message_w_safe(combo_genre, CB_SETCURSEL, WPARAM(0), LPARAM(0));

    let mut state = Box::new(RadioDialogState {
        parent,
        language,
        edit_search,
        combo_browse_mode,
        combo_language,
        combo_country,
        edit_city,
        combo_genre,
        list_favorites,
        list_results,
        label_page,
        favorite_results: Vec::new(),
        all_results: Vec::new(),
        page: 0,
        search_session: None,
        search_id: 0,
        search_loading: false,
        languages,
        countries,
        player_return_focus: None,
    });
    state.favorite_results = initial_results();
    crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
    update_browse_filter_visibility(hwnd);
    populate_favorites(hwnd);
    populate_results(hwnd);
    layout(hwnd);
    crate::set_focus_safe(list_favorites);
    start_radio_directory_refresh(hwnd, language);
}

fn create_static(parent: HWND, text: &str, id: usize) -> HWND {
    let t = to_wide(text);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            PCWSTR(t.as_ptr()),
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
fn create_edit(parent: HWND, id: usize) -> HWND {
    unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
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
            200,
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
fn create_button(parent: HWND, text: &str, id: usize, default: bool) -> HWND {
    let t = to_wide(text);
    let mut style = WS_CHILD | WS_VISIBLE | WS_TABSTOP;
    if default {
        style |= WINDOW_STYLE(BS_DEFPUSHBUTTON as u32);
    }
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            PCWSTR(t.as_ptr()),
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
    let mut rc = Default::default();
    crate::log_if_err!(crate::get_client_rect_safe(hwnd, &mut rc));
    let width = rc.right - rc.left;
    let margin = 10;
    let label_width = 95;
    let row_height = 25;
    let field_x = margin + label_width;
    let field_width = width - margin * 2 - label_width;
    let mut y = margin;

    move_id(
        hwnd,
        ID_LABEL_FAVORITES,
        margin,
        y + 4,
        label_width,
        row_height,
    );
    move_id(hwnd, ID_LIST_FAVORITES, field_x, y, field_width, 82);
    y += 92;

    move_id(
        hwnd,
        ID_LABEL_SEARCH,
        margin,
        y + 4,
        label_width,
        row_height,
    );
    move_id(hwnd, ID_EDIT_SEARCH, field_x, y, field_width, row_height);
    y += 33;

    move_id(
        hwnd,
        ID_LABEL_BROWSE_MODE,
        margin,
        y + 4,
        label_width,
        row_height,
    );
    move_id(hwnd, ID_COMBO_BROWSE_MODE, field_x, y, field_width, 120);
    y += 34;

    let mode = with_radio_state(hwnd, |state| selected_browse_mode(state))
        .unwrap_or(RadioBrowseMode::Language);
    let (active_label, active_control, control_height) = match mode {
        RadioBrowseMode::Language => (ID_LABEL_LANGUAGE, ID_COMBO_LANGUAGE, 180),
        RadioBrowseMode::Country => (ID_LABEL_COUNTRY, ID_COMBO_COUNTRY, 220),
        RadioBrowseMode::City => (ID_LABEL_CITY, ID_EDIT_CITY, row_height),
        RadioBrowseMode::Genre => (ID_LABEL_GENRE, ID_COMBO_GENRE, 210),
    };
    move_id(hwnd, active_label, margin, y + 4, label_width, row_height);
    move_id(
        hwnd,
        active_control,
        field_x,
        y,
        field_width,
        control_height,
    );
    y += 38;

    let button_gap = 8;
    let search_width = 90;
    let reset_width = 125;
    let recordings_width = 145;
    let add_width = 125;
    let total_buttons = search_width + reset_width + recordings_width + add_width + button_gap * 3;
    let mut button_x = (width - margin - total_buttons).max(margin);
    move_id(hwnd, ID_BUTTON_SEARCH, button_x, y, search_width, 30);
    button_x += search_width + button_gap;
    move_id(hwnd, ID_BUTTON_RESET_FILTERS, button_x, y, reset_width, 30);
    button_x += reset_width + button_gap;
    move_id(
        hwnd,
        ID_BUTTON_RECORDINGS,
        button_x,
        y,
        recordings_width,
        30,
    );
    button_x += recordings_width + button_gap;
    move_id(hwnd, ID_BUTTON_ADD_COMMUNITY, button_x, y, add_width, 30);
    y += 42;

    move_id(
        hwnd,
        ID_LABEL_RESULTS,
        margin,
        y + 4,
        label_width,
        row_height,
    );
    y += 25;
    let list_height = (rc.bottom - y - 72).max(80);
    move_id(
        hwnd,
        ID_LIST_RESULTS,
        margin,
        y,
        width - margin * 2,
        list_height,
    );
    y += list_height + 6;
    move_id(hwnd, ID_LABEL_PAGE, margin, y + 4, width - margin * 2, 26);
    y += 32;
    move_id(hwnd, ID_BUTTON_CLOSE, width - margin - 90, y, 90, 30);
}

fn update_browse_filter_visibility(hwnd: HWND) {
    let mode = with_radio_state(hwnd, |state| selected_browse_mode(state))
        .unwrap_or(RadioBrowseMode::Language);
    for (label_id, control_id, visible) in [
        (
            ID_LABEL_LANGUAGE,
            ID_COMBO_LANGUAGE,
            mode == RadioBrowseMode::Language,
        ),
        (
            ID_LABEL_COUNTRY,
            ID_COMBO_COUNTRY,
            mode == RadioBrowseMode::Country,
        ),
        (ID_LABEL_CITY, ID_EDIT_CITY, mode == RadioBrowseMode::City),
        (
            ID_LABEL_GENRE,
            ID_COMBO_GENRE,
            mode == RadioBrowseMode::Genre,
        ),
    ] {
        set_control_visible(hwnd, label_id, visible);
        set_control_visible(hwnd, control_id, visible);
    }
}

fn set_control_visible(hwnd: HWND, id: usize, visible: bool) {
    let control = crate::get_dlg_item_safe(hwnd, id as i32);
    if control.0 != 0 {
        unsafe {
            ShowWindow(control, if visible { SW_SHOW } else { SW_HIDE });
        }
    }
}

fn move_id(hwnd: HWND, id: usize, x: i32, y: i32, w: i32, h: i32) {
    let child = crate::get_dlg_item_safe(hwnd, id as i32);
    if child.0 != 0 {
        crate::log_if_err!(unsafe { MoveWindow(child, x, y, w, h, true) });
    }
}

fn with_radio_state<R>(hwnd: HWND, f: impl FnOnce(&mut RadioDialogState) -> R) -> Option<R> {
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut RadioDialogState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn tr(language: Language, key: &str, fallback: &str) -> String {
    let value = i18n::tr(language, key);
    if value == key {
        fallback.to_string()
    } else {
        value
    }
}

fn tr_fallback(language: Language, key: &str, args: &[(&str, &str)], fallback: &str) -> String {
    let value = i18n::tr_f(language, key, args);
    if value == key {
        let mut out = fallback.to_string();
        for (name, value) in args {
            out = out.replace(&format!("{{{name}}}"), value);
        }
        out
    } else {
        value
    }
}

fn get_edit_text(hwnd: HWND) -> String {
    let len = crate::get_window_text_length_w_safe(hwnd);
    let mut buf = vec![0u16; len as usize + 1];
    crate::get_window_text_w_safe(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..len as usize])
}

fn radio_menu_languages(language: Language) -> Vec<(String, String)> {
    [
        ("it", "radio.lang.it", "Italiano"),
        ("en", "radio.lang.en", "Inglese"),
        ("tr", "radio.lang.tr", "Turco"),
        ("de", "radio.lang.de", "Tedesco"),
        ("es", "radio.lang.es", "Spagnolo"),
        ("pt", "radio.lang.pt", "Portoghese"),
        ("sv", "radio.lang.sv", "Svedese"),
        ("vi", "radio.lang.vi", "Vietnamita"),
        ("cs", "radio.lang.cs", "Ceco"),
        ("pl", "radio.lang.pl", "Polacco"),
        ("fr", "radio.lang.fr", "Francese"),
        ("sr", "radio.lang.sr", "Serbo"),
        ("uk", "radio.lang.uk", "Ucraino"),
        ("hi", "radio.lang.hi", "Hindi"),
        ("lt", "radio.lang.lt", "Lituano"),
        ("ru", "radio.lang.ru", "Russo"),
        ("zh", "radio.lang.zh", "Cinese"),
    ]
    .into_iter()
    .map(|(code, key, fallback)| (code.to_string(), tr(language, key, fallback)))
    .collect()
}

fn radio_menu_countries(language: Language) -> Vec<(String, String)> {
    [
        "it", "us", "gb", "tr", "fr", "es", "de", "ch", "at", "be", "nl", "pt", "br", "ar", "mx",
        "ca", "au", "ie", "se", "pl", "jp", "cn", "in", "cz", "ru", "lt", "ua",
    ]
    .into_iter()
    .map(|code| {
        let key = format!("options.podcast_country.{code}");
        let translated = i18n::tr(language, &key);
        let label = if translated == key {
            code.to_uppercase()
        } else {
            translated
        };
        (code.to_string(), label)
    })
    .collect()
}

fn start_radio_directory_refresh(hwnd: HWND, language: Language) {
    let hwnd_value = hwnd.0;
    std::thread::spawn(move || {
        let languages = fetch_radio_browser_language_options(language).unwrap_or_else(|error| {
            crate::log_debug(&format!(
                "Radio: language directory unavailable, using fallback list: {error}"
            ));
            radio_menu_languages(language)
        });
        let countries = fetch_radio_browser_country_options(language).unwrap_or_else(|error| {
            crate::log_debug(&format!(
                "Radio: country directory unavailable, using fallback list: {error}"
            ));
            radio_menu_countries(language)
        });

        let complete = Box::new(RadioDirectoryComplete {
            languages,
            countries,
        });
        let ptr = Box::into_raw(complete);
        let hwnd = HWND(hwnd_value);
        if let Err(error) = crate::post_message_w_safe(
            hwnd,
            WM_RADIO_DIRECTORY_COMPLETE,
            WPARAM(0),
            LPARAM(ptr as isize),
        ) {
            crate::log_debug(&format!("Radio: unable to post directory update: {error}"));
            // SAFETY: ownership was transferred by Box::into_raw above and the failed post
            // guarantees that the window procedure will not reclaim this allocation.
            let _reclaimed_complete = unsafe { Box::from_raw(ptr) };
        }
    });
}

fn apply_radio_directory_options(hwnd: HWND, complete: RadioDirectoryComplete) {
    with_radio_state(hwnd, |state| {
        let selected_language = selected_language_code(state);
        let selected_country = selected_country_code(state);

        if !complete.languages.is_empty() {
            state.languages = complete.languages;
            replace_language_combo_options(
                state.combo_language,
                &state.languages,
                &selected_language,
                app_language_code(state.language),
            );
        }
        if !complete.countries.is_empty() {
            state.countries = complete.countries;
            replace_combo_options(
                state.combo_country,
                &state.countries,
                &selected_country,
                default_country_code(state.language),
            );
        }
    });
}

fn replace_language_combo_options(
    combo: HWND,
    options: &[(String, String)],
    selected_code: &str,
    fallback_code: &str,
) {
    crate::send_message_w_safe(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    for (_, label) in options {
        let wide = to_wide(label);
        crate::send_message_w_safe(
            combo,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    let selected_canonical = canonical_language_option_code(selected_code);
    let fallback_canonical = canonical_language_option_code(fallback_code);
    let selected_index = options
        .iter()
        .position(|(code, _)| code == selected_code)
        .or_else(|| {
            options
                .iter()
                .position(|(code, _)| canonical_language_option_code(code) == selected_canonical)
        })
        .or_else(|| {
            options
                .iter()
                .position(|(code, _)| canonical_language_option_code(code) == fallback_canonical)
        })
        .unwrap_or(0);
    crate::send_message_w_safe(combo, CB_SETCURSEL, WPARAM(selected_index), LPARAM(0));
}

fn replace_combo_options(
    combo: HWND,
    options: &[(String, String)],
    selected_code: &str,
    fallback_code: &str,
) {
    crate::send_message_w_safe(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    for (_, label) in options {
        let wide = to_wide(label);
        crate::send_message_w_safe(
            combo,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    let selected_index = options
        .iter()
        .position(|(code, _)| code == selected_code)
        .or_else(|| options.iter().position(|(code, _)| code == fallback_code))
        .unwrap_or(0);
    crate::send_message_w_safe(combo, CB_SETCURSEL, WPARAM(selected_index), LPARAM(0));
}

fn fetch_radio_browser_language_options(
    language: Language,
) -> Result<Vec<(String, String)>, String> {
    let entries = fetch_radio_browser_language_directory()?;
    let dynamic = language_options_from_directory_entries(language, entries);
    Ok(merge_language_directory_options(language, dynamic))
}

fn language_options_from_directory_entries(
    language: Language,
    entries: Vec<RadioBrowserDirectoryEntry>,
) -> Vec<RadioDirectoryOption> {
    entries
        .into_iter()
        .filter(|entry| entry.stationcount > 0)
        .filter_map(|entry| {
            let raw_name = entry.name.trim();
            if raw_name.is_empty() {
                return None;
            }

            let mut codes = entry
                .iso_639
                .as_deref()
                .and_then(radio_code_from_browser_language)
                .map(|code| vec![code])
                .unwrap_or_else(|| {
                    locale_display_names::language_codes_from_catalog_label(raw_name)
                });
            codes.dedup();
            if codes.is_empty() {
                crate::log_debug(&format!(
                    "Radio: ignored invalid language-directory label: {raw_name}"
                ));
                return None;
            }

            let label = localized_language_codes_label(language, &codes);
            let canonical_code = (codes.len() == 1).then(|| codes[0].as_str());
            Some(RadioDirectoryOption {
                code: dynamic_language_code(canonical_code, raw_name),
                label,
                station_count: entry.stationcount,
            })
        })
        .collect()
}

fn localized_language_codes_label(language: Language, codes: &[String]) -> String {
    let mut labels = Vec::new();
    for code in codes {
        let label = locale_display_names::language_name(language, code)
            .unwrap_or_else(|| code.to_uppercase());
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    labels.join(" / ")
}

fn fetch_radio_browser_country_options(
    language: Language,
) -> Result<Vec<(String, String)>, String> {
    let entries = fetch_radio_browser_country_directory()?;
    let dynamic = entries
        .into_iter()
        .filter(|entry| entry.stationcount > 0)
        .filter_map(|entry| {
            let code = entry.name.trim().to_ascii_lowercase();
            if code.len() != 2 || !code.chars().all(|ch| ch.is_ascii_alphabetic()) {
                return None;
            }
            let fallback = code.to_ascii_uppercase();
            let name = locale_display_names::territory_name(language, &code)
                .or_else(|| translated_country_name(language, &code))
                .unwrap_or(fallback);
            Some(RadioDirectoryOption {
                label: country_name_with_code(&name, &code),
                code,
                station_count: entry.stationcount,
            })
        })
        .collect::<Vec<_>>();

    Ok(merge_country_directory_options(language, dynamic))
}

fn fetch_radio_browser_language_directory() -> Result<Vec<RadioBrowserDirectoryEntry>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("Sonarpad/0.8.1 (https://sonarpad.com)")
        .build()
        .map_err(|error| error.to_string())?;
    let mut last_error = None;
    for mirror in radio_browser_mirrors() {
        let url =
            format!("{mirror}/json/languages?hidebroken=true&order=stationcount&reverse=true");
        match client
            .get(&url)
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.json::<Vec<RadioBrowserDirectoryEntry>>())
        {
            Ok(entries) => return Ok(entries),
            Err(error) => {
                crate::log_debug(&format!(
                    "Radio: language directory request failed: url={url} error={error}"
                ));
                last_error = Some(error.to_string());
            }
        }
    }
    Err(last_error.unwrap_or_else(|| "Radio Browser language list unavailable".to_string()))
}

fn fetch_radio_browser_country_directory() -> Result<Vec<RadioBrowserCountryEntry>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("Sonarpad/0.8.1 (https://sonarpad.com)")
        .build()
        .map_err(|error| error.to_string())?;
    let mut last_error = None;
    for mirror in radio_browser_mirrors() {
        let url =
            format!("{mirror}/json/countrycodes?hidebroken=true&order=stationcount&reverse=true");
        match client
            .get(&url)
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.json::<Vec<RadioBrowserCountryEntry>>())
        {
            Ok(entries) => return Ok(entries),
            Err(error) => {
                crate::log_debug(&format!(
                    "Radio: country directory request failed: url={url} error={error}"
                ));
                last_error = Some(error.to_string());
            }
        }
    }
    Err(last_error.unwrap_or_else(|| "Radio Browser country list unavailable".to_string()))
}

fn radio_browser_mirrors() -> [&'static str; 4] {
    [
        "https://all.api.radio-browser.info",
        "https://de1.api.radio-browser.info",
        "https://fi1.api.radio-browser.info",
        "https://at1.api.radio-browser.info",
    ]
}

fn merge_language_directory_options(
    language: Language,
    dynamic: Vec<RadioDirectoryOption>,
) -> Vec<(String, String)> {
    let mut by_code = HashMap::<String, RadioDirectoryOption>::new();
    for (code, label) in radio_menu_languages(language) {
        by_code.insert(
            code.clone(),
            RadioDirectoryOption {
                code,
                label,
                station_count: 0,
            },
        );
    }
    for option in dynamic {
        let key = canonical_language_option_code(&option.code);
        let replace = by_code
            .get(&key)
            .is_none_or(|current| option.station_count > current.station_count);
        if replace {
            by_code.insert(key, option);
        }
    }

    // Radio Browser can expose the same language using several raw labels
    // (for example "english russian", "english/russian" and the equivalent
    // Cyrillic form). Once localized, those entries become identical and a
    // Win32 combo box still treats them as separate rows. Screen readers then
    // receive a selection change without any text change and may announce
    // nothing. Keep a single row for every visible label, choosing the
    // directory entry with the greatest station coverage.
    let mut by_label = HashMap::<String, RadioDirectoryOption>::new();
    for option in by_code.into_values() {
        let key = normalized_language_option_label(&option.label);
        let replace = by_label
            .get(&key)
            .is_none_or(|current| prefer_language_directory_option(&option, current));
        if replace {
            by_label.insert(key, option);
        }
    }

    let mut options = by_label.into_values().collect::<Vec<_>>();
    options.sort_by(|left, right| {
        preferred_language_priority(&canonical_language_option_code(&left.code))
            .cmp(&preferred_language_priority(
                &canonical_language_option_code(&right.code),
            ))
            .then_with(|| left.label.to_lowercase().cmp(&right.label.to_lowercase()))
    });
    options
        .into_iter()
        .map(|option| (option.code, option.label))
        .collect()
}

fn normalized_language_option_label(label: &str) -> String {
    label
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn prefer_language_directory_option(
    candidate: &RadioDirectoryOption,
    current: &RadioDirectoryOption,
) -> bool {
    candidate.station_count > current.station_count
        || (candidate.station_count == current.station_count
            && canonical_language_option_code(&candidate.code).len()
                < canonical_language_option_code(&current.code).len())
        || (candidate.station_count == current.station_count
            && canonical_language_option_code(&candidate.code).len()
                == canonical_language_option_code(&current.code).len()
            && candidate.code < current.code)
}

fn merge_country_directory_options(
    language: Language,
    dynamic: Vec<RadioDirectoryOption>,
) -> Vec<(String, String)> {
    let mut by_code = HashMap::<String, RadioDirectoryOption>::new();
    for (code, label) in radio_menu_countries(language) {
        by_code.insert(
            code.clone(),
            RadioDirectoryOption {
                code,
                label,
                station_count: 0,
            },
        );
    }
    for option in dynamic {
        let key = option.code.to_lowercase();
        let replace = by_code
            .get(&key)
            .is_none_or(|current| option.station_count > current.station_count);
        if replace {
            by_code.insert(key, option);
        }
    }
    let mut options = by_code.into_values().collect::<Vec<_>>();
    let default_code = default_country_code(language);
    options.sort_by(|left, right| {
        usize::from(left.code != default_code)
            .cmp(&usize::from(right.code != default_code))
            .then_with(|| left.label.to_lowercase().cmp(&right.label.to_lowercase()))
    });
    options
        .into_iter()
        .map(|option| (option.code, option.label))
        .collect()
}

fn preferred_language_priority(code: &str) -> usize {
    const PREFERRED: [&str; 8] = ["it", "en", "tr", "de", "es", "fr", "pt", "pl"];
    PREFERRED
        .iter()
        .position(|preferred| *preferred == code)
        .unwrap_or(1000)
}

fn radio_code_from_browser_language(value: &str) -> Option<String> {
    locale_display_names::language_code_from_name(value)
}

fn dynamic_language_code(iso_code: Option<&str>, name: &str) -> String {
    let iso_code = iso_code.unwrap_or_default().trim().to_lowercase();
    let name = name.trim().to_lowercase();
    format!("language:{iso_code}:{name}")
}

fn dynamic_language_parts(code: &str) -> Option<(Option<&str>, &str)> {
    let rest = code.strip_prefix("language:")?;
    if let Some((iso_code, name)) = rest.split_once(':') {
        let iso_code = (!iso_code.is_empty()).then_some(iso_code);
        Some((iso_code, name))
    } else {
        // Compatibility with favourites saved by the first dynamic-directory
        // implementation, which stored only the Radio Browser name.
        Some((None, rest))
    }
}

fn dynamic_language_name(code: &str) -> Option<&str> {
    dynamic_language_parts(code).map(|(_, name)| name)
}

fn canonical_language_option_code(code: &str) -> String {
    dynamic_language_parts(code)
        .and_then(|(iso_code, _)| iso_code)
        .unwrap_or(code)
        .trim()
        .replace('_', "-")
        .to_lowercase()
}

fn title_case_radio_directory_label(value: &str) -> String {
    value
        .split_whitespace()
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let first = first.to_uppercase().collect::<String>();
                    format!("{first}{}", chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn country_name_with_code(name: &str, code: &str) -> String {
    let code = code.trim().to_ascii_uppercase();
    let name = name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case(&code) {
        code
    } else {
        format!("{name} ({code})")
    }
}

fn translated_country_name(language: Language, code: &str) -> Option<String> {
    let key = format!("options.podcast_country.{}", code.to_ascii_lowercase());
    let translated = i18n::tr(language, &key);
    (translated != key && !translated.trim().is_empty()).then_some(translated)
}

fn default_country_code(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::German => "de",
        Language::English => "us",
        Language::Spanish => "es",
        Language::Portuguese | Language::PortugueseBrazilian => "pt",
        Language::Swedish => "se",
        Language::Polish => "pl",
        Language::French => "fr",
        Language::Czech => "cz",
        Language::Ukrainian => "ua",
        Language::Lithuanian => "lt",
        Language::Russian => "ru",
        Language::Chinese => "cn",
        Language::Hindi => "in",
        _ => "it",
    }
}

fn selected_browse_mode(state: &RadioDialogState) -> RadioBrowseMode {
    match crate::send_message_w_safe(state.combo_browse_mode, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0
    {
        1 => RadioBrowseMode::Country,
        2 => RadioBrowseMode::City,
        3 => RadioBrowseMode::Genre,
        _ => RadioBrowseMode::Language,
    }
}

fn selected_country_code(state: &RadioDialogState) -> String {
    let index = crate::send_message_w_safe(state.combo_country, CB_GETCURSEL, WPARAM(0), LPARAM(0))
        .0
        .max(0) as usize;
    state
        .countries
        .get(index)
        .map(|(code, _)| code.clone())
        .unwrap_or_else(|| "it".to_string())
}

fn selected_browse_code(state: &RadioDialogState) -> Result<String, String> {
    match selected_browse_mode(state) {
        RadioBrowseMode::Language => Ok(selected_language_code(state)),
        RadioBrowseMode::Country => Ok(format!("country:{}", selected_country_code(state))),
        RadioBrowseMode::City => {
            let city = get_edit_text(state.edit_city).trim().to_string();
            if city.is_empty() {
                Err(tr(
                    state.language,
                    "radio.city_required",
                    "Inserisci il nome della città.",
                ))
            } else {
                Ok(format!("city:{city}"))
            }
        }
        RadioBrowseMode::Genre => Ok("all".to_string()),
    }
}

fn initial_results() -> Vec<RadioFavorite> {
    normalize_favorites(load_settings().radio_favorites)
}

fn selected_language_code(state: &RadioDialogState) -> String {
    let idx = crate::send_message_w_safe(state.combo_language, CB_GETCURSEL, WPARAM(0), LPARAM(0))
        .0
        .max(0) as usize;
    state
        .languages
        .get(idx)
        .map(|(c, _)| c.clone())
        .unwrap_or_else(|| "it".into())
}

fn selected_genre_tag(state: &RadioDialogState) -> Option<String> {
    let idx = crate::send_message_w_safe(state.combo_genre, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if idx >= 0 {
        return GENRE_OPTIONS
            .get(idx as usize)
            .and_then(|genre| genre_tag_from_value(genre.value))
            .map(str::to_string);
    }
    let typed = get_edit_text(state.combo_genre);
    let typed = typed.trim();
    if typed.is_empty() {
        None
    } else {
        Some(typed.to_string())
    }
}

fn genre_tag_from_value(value: &str) -> Option<&'static str> {
    GENRE_OPTIONS
        .iter()
        .find(|genre| genre.value == value)
        .and_then(|genre| genre.tag)
}

fn show_all(hwnd: HWND) {
    start_radio_search(hwnd, None);
}

fn show_all_and_focus(hwnd: HWND) {
    show_all(hwnd);
}

fn search(hwnd: HWND) {
    let keyword = with_radio_state(hwnd, |state| {
        normalized_radio_name(&get_edit_text(state.edit_search))
    })
    .unwrap_or_default();
    let name_query = if keyword.is_empty() {
        None
    } else {
        Some(keyword)
    };
    start_radio_search(hwnd, name_query);
}

fn reset_filters(hwnd: HWND) {
    with_radio_state(hwnd, |state| {
        crate::send_message_w_safe(state.combo_browse_mode, CB_SETCURSEL, WPARAM(0), LPARAM(0));
        let language_index = state
            .languages
            .iter()
            .position(|(code, _)| code == app_language_code(state.language))
            .unwrap_or(0);
        crate::send_message_w_safe(
            state.combo_language,
            CB_SETCURSEL,
            WPARAM(language_index),
            LPARAM(0),
        );
        let country_index = state
            .countries
            .iter()
            .position(|(code, _)| code == default_country_code(state.language))
            .unwrap_or(0);
        crate::send_message_w_safe(
            state.combo_country,
            CB_SETCURSEL,
            WPARAM(country_index),
            LPARAM(0),
        );
        crate::send_message_w_safe(state.combo_genre, CB_SETCURSEL, WPARAM(0), LPARAM(0));
        let empty = to_wide("");
        crate::log_if_err!(crate::set_window_text_w_safe(
            state.edit_search,
            PCWSTR(empty.as_ptr())
        ));
        crate::log_if_err!(crate::set_window_text_w_safe(
            state.edit_city,
            PCWSTR(empty.as_ptr())
        ));
    });
    update_browse_filter_visibility(hwnd);
    layout(hwnd);
    show_all(hwnd);
}

fn start_radio_search(hwnd: HWND, name_query: Option<String>) {
    let Some((language, code_result, genre)) = with_radio_state(hwnd, |state| {
        let mode = selected_browse_mode(state);
        (
            state.language,
            selected_browse_code(state),
            if mode == RadioBrowseMode::Genre {
                selected_genre_tag(state)
            } else {
                None
            },
        )
    }) else {
        return;
    };
    let code = match code_result {
        Ok(code) => code,
        Err(message_text) => {
            message(hwnd, "Radio", &message_text);
            return;
        }
    };
    static NEXT_SEARCH_ID: AtomicU64 = AtomicU64::new(1);
    let search_id = NEXT_SEARCH_ID.fetch_add(1, Ordering::Relaxed);
    with_radio_state(hwnd, |state| {
        state.search_id = search_id;
        state.search_loading = true;
        state.search_session = None;
        show_loading_in_results(state);
    });
    load_radio_page(
        hwnd,
        language,
        search_id,
        SearchSession::new(code, name_query, genre),
        0,
    );
}

fn load_radio_page(
    hwnd: HWND,
    language: Language,
    search_id: u64,
    mut session: SearchSession,
    page: usize,
) {
    let hwnd_value = hwnd.0;
    std::thread::spawn(move || {
        let error = session.load_page(page).err();
        let complete = Box::new(RadioSearchComplete {
            language,
            session,
            page,
            search_id,
            error,
        });
        let ptr = Box::into_raw(complete);
        if let Err(err) = crate::post_message_w_safe(
            HWND(hwnd_value),
            WM_RADIO_SEARCH_COMPLETE,
            WPARAM(0),
            LPARAM(ptr as isize),
        ) {
            // Posting failed: ownership was never transferred to the window.
            let _complete = unsafe { Box::from_raw(ptr) };
            crate::log_debug(&format!("Radio: post search complete failed: {err}"));
        }
    });
}

fn show_loading_in_results(state: &mut RadioDialogState) {
    let message = i18n::tr(state.language, "radio.loading");
    show_status_in_results(state, &message);
}

fn show_status_in_results(state: &mut RadioDialogState, text: &str) {
    state.all_results.clear();
    state.page = 0;
    crate::send_message_w_safe(state.list_results, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    let text = to_wide(text);
    crate::send_message_w_safe(
        state.list_results,
        LB_ADDSTRING,
        WPARAM(0),
        LPARAM(text.as_ptr() as isize),
    );
    crate::send_message_w_safe(state.list_results, LB_SETCURSEL, WPARAM(0), LPARAM(0));
    let page = to_wide("");
    crate::log_if_err!(crate::set_window_text_w_safe(
        state.label_page,
        PCWSTR(page.as_ptr())
    ));
    focus_results_list_from_state(state);
}

fn finish_radio_search(hwnd: HWND, complete: RadioSearchComplete) {
    // Ignore late results from an earlier query (or an earlier window instance).
    let current =
        with_radio_state(hwnd, |state| state.search_id == complete.search_id).unwrap_or(false);
    if !current {
        return;
    }
    let empty = complete.session.results.is_empty();
    with_radio_state(hwnd, |state| {
        state.search_loading = false;
        state.all_results = complete.session.results.clone();
        state.search_session = Some(complete.session);
        state.page = complete
            .page
            .min(state.all_results.len().saturating_sub(1) / RADIO_RESULTS_PAGE_SIZE);
    });
    if let Some(error) = complete.error {
        crate::log_debug(&format!("Radio search finished with error: {error}"));
        with_radio_state(hwnd, |state| {
            let message = i18n::tr(complete.language, "radio.search_failed");
            show_status_in_results(state, &message);
        });
    } else {
        populate_results(hwnd);
        focus_results_list(hwnd);
        if empty {
            message(
                hwnd,
                "Radio",
                &tr(
                    complete.language,
                    "radio.none_found",
                    "Nessuna radio trovata.",
                ),
            );
        }
    }
}

fn has_next_page(state: &RadioDialogState) -> bool {
    (state.page + 1) * RADIO_RESULTS_PAGE_SIZE < state.all_results.len()
        || state
            .search_session
            .as_ref()
            .is_some_and(SearchSession::has_more)
}

fn focus_results_list_from_state(state: &RadioDialogState) {
    crate::set_focus_safe(state.list_results);
}

fn populate_favorites(hwnd: HWND) {
    with_radio_state(hwnd, |state| {
        crate::send_message_w_safe(state.list_favorites, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
        for item in &state.favorite_results {
            let w = to_wide(&radio_label(item, state.language));
            crate::send_message_w_safe(
                state.list_favorites,
                LB_ADDSTRING,
                WPARAM(0),
                LPARAM(w.as_ptr() as isize),
            );
        }
        if !state.favorite_results.is_empty() {
            crate::send_message_w_safe(state.list_favorites, LB_SETCURSEL, WPARAM(0), LPARAM(0));
        }
    });
}

fn populate_results(hwnd: HWND) {
    with_radio_state(hwnd, |state| {
        crate::send_message_w_safe(state.list_results, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
        let total_pages = state
            .all_results
            .len()
            .div_ceil(RADIO_RESULTS_PAGE_SIZE)
            .max(1);
        state.page = state.page.min(total_pages.saturating_sub(1));
        let start = state.page * RADIO_RESULTS_PAGE_SIZE;
        let end = (start + RADIO_RESULTS_PAGE_SIZE).min(state.all_results.len());
        if state.page > 0 {
            let previous = tr(state.language, "radio.previous", "Precedenti");
            let w = to_wide(&previous);
            crate::send_message_w_safe(
                state.list_results,
                LB_ADDSTRING,
                WPARAM(0),
                LPARAM(w.as_ptr() as isize),
            );
        }
        for item in &state.all_results[start..end] {
            let w = to_wide(&radio_label(item, state.language));
            crate::send_message_w_safe(
                state.list_results,
                LB_ADDSTRING,
                WPARAM(0),
                LPARAM(w.as_ptr() as isize),
            );
        }
        if has_next_page(state) {
            let next = tr(state.language, "radio.next", "Successivi");
            let w = to_wide(&next);
            crate::send_message_w_safe(
                state.list_results,
                LB_ADDSTRING,
                WPARAM(0),
                LPARAM(w.as_ptr() as isize),
            );
        }
        if end > start {
            let selected = if state.page > 0 { 1 } else { 0 };
            crate::send_message_w_safe(
                state.list_results,
                LB_SETCURSEL,
                WPARAM(selected),
                LPARAM(0),
            );
        }
        let page_key = if state
            .search_session
            .as_ref()
            .is_some_and(SearchSession::has_more)
        {
            "radio.page_loading_total"
        } else {
            "radio.page"
        };
        let page = i18n::tr_f(
            state.language,
            page_key,
            &[
                ("current", &(state.page + 1).to_string()),
                ("total", &total_pages.to_string()),
            ],
        );
        let text = if page == "radio.page" {
            format!("Pagina {} di {}", state.page + 1, total_pages)
        } else {
            page
        };
        let w = to_wide(&text);
        crate::log_if_err!(crate::set_window_text_w_safe(
            state.label_page,
            PCWSTR(w.as_ptr())
        ));
    });
}

fn change_page(hwnd: HWND, delta: isize) {
    let request = with_radio_state(hwnd, |state| {
        if state.search_loading {
            return None;
        }
        let page = state.page.saturating_add_signed(delta);
        if delta > 0 && !has_next_page(state) {
            return None;
        }
        let needs_more = (page + 1) * RADIO_RESULTS_PAGE_SIZE >= state.all_results.len()
            && state
                .search_session
                .as_ref()
                .is_some_and(SearchSession::has_more);
        if needs_more && let Some(session) = state.search_session.take() {
            state.search_loading = true;
            let text = to_wide(&i18n::tr(state.language, "radio.loading"));
            crate::log_if_err!(crate::set_window_text_w_safe(
                state.label_page,
                PCWSTR(text.as_ptr())
            ));
            return Some((state.language, state.search_id, session, page));
        }
        state.page = page.min(state.all_results.len().saturating_sub(1) / RADIO_RESULTS_PAGE_SIZE);
        None
    })
    .flatten();
    if let Some((language, search_id, session, page)) = request {
        load_radio_page(hwnd, language, search_id, session, page);
    } else if with_radio_state(hwnd, |state| !state.search_loading).unwrap_or(false) {
        populate_results(hwnd);
    }
}

#[derive(Clone, Copy, Debug)]
enum RadioListKind {
    Favorites,
    Results,
}

#[derive(Clone, Copy, Debug)]
struct RadioReturnFocus {
    kind: RadioListKind,
    selection: isize,
}

fn selected_radio_list_kind(hwnd: HWND) -> Option<RadioListKind> {
    let focus = crate::get_focus_safe();
    with_radio_state(hwnd, |state| {
        if focus == state.list_favorites {
            Some(RadioListKind::Favorites)
        } else if focus == state.list_results {
            Some(RadioListKind::Results)
        } else {
            None
        }
    })
    .flatten()
}

fn selected_result(hwnd: HWND, kind: RadioListKind) -> Option<RadioFavorite> {
    with_radio_state(hwnd, |state| selected_result_from_state(state, kind)).flatten()
}

fn remember_player_return_focus(hwnd: HWND, kind: RadioListKind) {
    with_radio_state(hwnd, |state| {
        let list = match kind {
            RadioListKind::Favorites => state.list_favorites,
            RadioListKind::Results => state.list_results,
        };
        let selection = crate::send_message_w_safe(list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        state.player_return_focus = Some(RadioReturnFocus { kind, selection });
        crate::log_debug(&format!(
            "Radio: remember player return focus kind={:?} selection={}",
            kind, selection
        ));
    });
}

fn restore_player_return_focus(hwnd: HWND) {
    let target = with_radio_state(hwnd, |state| state.player_return_focus).flatten();
    let Some(target) = target else {
        crate::log_debug("Radio: no remembered player return focus; falling back to results");
        focus_results_list(hwnd);
        schedule_results_refocus(hwnd);
        return;
    };

    if target.selection >= 0 {
        with_radio_state(hwnd, |state| {
            let list = match target.kind {
                RadioListKind::Favorites => state.list_favorites,
                RadioListKind::Results => state.list_results,
            };
            crate::send_message_w_safe(
                list,
                LB_SETCURSEL,
                WPARAM(target.selection as usize),
                LPARAM(0),
            );
        });
    }

    crate::log_debug(&format!(
        "Radio: restoring player return focus kind={:?} selection={}",
        target.kind, target.selection
    ));
    match target.kind {
        RadioListKind::Favorites => {
            focus_favorites_list(hwnd);
            crate::log_if_err!(crate::post_message_w_safe(
                hwnd,
                WM_RADIO_FOCUS_FAVORITES,
                WPARAM(0),
                LPARAM(0)
            ));
            schedule_favorites_refocus(hwnd);
        }
        RadioListKind::Results => {
            focus_results_list(hwnd);
            crate::log_if_err!(crate::post_message_w_safe(
                hwnd,
                WM_RADIO_FOCUS_RESULTS,
                WPARAM(0),
                LPARAM(0)
            ));
            schedule_results_refocus(hwnd);
        }
    }
}

enum RadioListAction {
    Open(RadioFavorite),
    PreviousPage,
    NextPage,
}

fn selected_list_action_from_state(state: &mut RadioDialogState) -> Option<RadioListAction> {
    if state.search_loading {
        return None;
    }
    let sel = crate::send_message_w_safe(state.list_results, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel < 0 {
        return None;
    }
    let start = state.page * RADIO_RESULTS_PAGE_SIZE;
    let end = (start + RADIO_RESULTS_PAGE_SIZE).min(state.all_results.len());
    if state.page > 0 && sel == 0 {
        return Some(RadioListAction::PreviousPage);
    }
    let radio_row = if state.page > 0 {
        (sel as usize).saturating_sub(1)
    } else {
        sel as usize
    };
    let index = start + radio_row;
    if index < end {
        return state
            .all_results
            .get(index)
            .cloned()
            .map(RadioListAction::Open);
    }
    if radio_row == end.saturating_sub(start) && has_next_page(state) {
        Some(RadioListAction::NextPage)
    } else {
        None
    }
}

fn selected_result_from_state(
    state: &mut RadioDialogState,
    kind: RadioListKind,
) -> Option<RadioFavorite> {
    match kind {
        RadioListKind::Favorites => {
            let sel = crate::send_message_w_safe(
                state.list_favorites,
                LB_GETCURSEL,
                WPARAM(0),
                LPARAM(0),
            )
            .0;
            if sel < 0 {
                return None;
            }
            state.favorite_results.get(sel as usize).cloned()
        }
        RadioListKind::Results => match selected_list_action_from_state(state)? {
            RadioListAction::Open(item) => Some(item),
            RadioListAction::PreviousPage => None,
            RadioListAction::NextPage => None,
        },
    }
}

fn open_selected_favorite(hwnd: HWND) {
    if let Some((parent, item)) = with_radio_state(hwnd, |state| {
        selected_result_from_state(state, RadioListKind::Favorites).map(|item| (state.parent, item))
    })
    .flatten()
    {
        remember_player_return_focus(hwnd, RadioListKind::Favorites);
        if let Err(err) =
            launch_stream_url_in_mpv(parent, &item.stream_url, Some(&item.name), None, None, None)
        {
            message(hwnd, "Radio", &err);
        }
    }
}

fn open_selected(hwnd: HWND) {
    match with_radio_state(hwnd, |state| {
        selected_list_action_from_state(state).map(|action| (state.parent, action))
    })
    .flatten()
    {
        Some((_, RadioListAction::PreviousPage)) => change_page(hwnd, -1),
        Some((_, RadioListAction::NextPage)) => change_page(hwnd, 1),
        Some((parent, RadioListAction::Open(item))) => {
            remember_player_return_focus(hwnd, RadioListKind::Results);
            if let Err(err) = launch_stream_url_in_mpv(
                parent,
                &item.stream_url,
                Some(&item.name),
                None,
                None,
                None,
            ) {
                message(hwnd, "Radio", &err);
            }
        }
        None => {}
    }
}
fn add_selected_favorite(hwnd: HWND) {
    let Some(kind) = selected_radio_list_kind(hwnd) else {
        return;
    };
    let Some(item) = selected_result(hwnd, kind) else {
        return;
    };
    let mut settings = load_settings();
    if settings
        .radio_favorites
        .iter()
        .any(|f| f.stream_url == item.stream_url)
    {
        message(
            hwnd,
            "Radio",
            &tr(
                settings.language,
                "radio.already_favorite",
                "La radio selezionata è già nei preferiti.",
            ),
        );
        return;
    }
    settings.radio_favorites.push(item.clone());
    settings.radio_favorites = normalize_favorites(settings.radio_favorites.clone());
    let favorites = settings.radio_favorites.clone();
    let lang = settings.language;
    save_settings(settings);
    with_radio_state(hwnd, |state| {
        state.favorite_results = favorites;
    });
    populate_favorites(hwnd);
    message(
        hwnd,
        "Radio",
        &i18n::tr_f(lang, "radio.favorite_added", &[("name", &item.name)]),
    );
    with_radio_state(hwnd, |state| {
        crate::set_focus_safe(state.list_favorites);
    });
}
fn remove_selected_favorite(hwnd: HWND) {
    let Some(kind) = selected_radio_list_kind(hwnd) else {
        return;
    };
    let Some(item) = selected_result(hwnd, kind) else {
        return;
    };
    let mut settings = load_settings();
    let before = settings.radio_favorites.len();
    settings
        .radio_favorites
        .retain(|f| f.stream_url != item.stream_url);
    if settings.radio_favorites.len() != before {
        settings.radio_favorites = normalize_favorites(settings.radio_favorites.clone());
        let favorites = settings.radio_favorites.clone();
        let lang = settings.language;
        save_settings(settings);
        with_radio_state(hwnd, |state| {
            state.favorite_results = favorites;
        });
        populate_favorites(hwnd);
        message(
            hwnd,
            "Radio",
            &i18n::tr_f(lang, "radio.favorite_removed", &[("name", &item.name)]),
        );
    }
}

fn record_and_play_selected_radio(hwnd: HWND) {
    let Some(kind) = selected_radio_list_kind(hwnd) else {
        return;
    };
    let Some(item) = selected_result(hwnd, kind) else {
        return;
    };
    let Some((parent, language)) = with_radio_state(hwnd, |state| (state.parent, state.language))
    else {
        return;
    };
    let recording_folder = stream_recording::recordings_folder(StreamRecordingKind::Radio);
    let recording_folder_text = recording_folder.to_string_lossy().to_string();
    let starting_announcement = tr_fallback(
        language,
        "radio.recording_starting",
        &[
            ("name", item.name.as_str()),
            ("path", recording_folder_text.as_str()),
        ],
        "Avvio registrazione di {name}. Il file sarà salvato in {path}.",
    );
    crate::screen_reader_speak(&starting_announcement);

    match stream_recording::start_radio_recording_and_playback(
        parent,
        &item.stream_url,
        &item.name,
        language,
    ) {
        Ok(path) => {
            let path_text = path.to_string_lossy().to_string();
            let text = tr_fallback(
                language,
                "radio.recording_started",
                &[("name", item.name.as_str()), ("path", path_text.as_str())],
                "Registrazione di {name} avviata. Il file sarà salvato in {path}. La registrazione terminerà chiudendo il player.",
            );
            crate::screen_reader_speak(&text);
        }
        Err(err) => message(hwnd, "Radio", &err),
    }
}

fn schedule_selected_radio(hwnd: HWND) {
    let Some(kind) = selected_radio_list_kind(hwnd) else {
        return;
    };
    let Some(item) = selected_result(hwnd, kind) else {
        return;
    };
    let language = with_radio_state(hwnd, |state| state.language).unwrap_or_default();
    scheduled_recording_window::open_for_radio(hwnd, language, item);
}

fn copy_selected_stream_url(hwnd: HWND) {
    let Some(kind) = selected_radio_list_kind(hwnd) else {
        return;
    };
    let Some(item) = selected_result(hwnd, kind) else {
        return;
    };
    crate::app_windows::rai_audiodescrizioni_window::copy_text_to_clipboard(hwnd, &item.stream_url);
}

fn message(hwnd: HWND, title: &str, body: &str) {
    let t = to_wide(title);
    let b = to_wide(body);
    crate::message_box_w_safe(
        hwnd,
        PCWSTR(b.as_ptr()),
        PCWSTR(t.as_ptr()),
        MB_OK | MB_ICONINFORMATION,
    );
}

fn favorite_from_station(language_code: &str, station: &RadioStation) -> RadioFavorite {
    RadioFavorite {
        language_code: language_code.to_string(),
        name: station.name.clone(),
        stream_url: station.stream_url.clone(),
    }
}
fn normalize_favorites(mut items: Vec<RadioFavorite>) -> Vec<RadioFavorite> {
    items.retain(|x| !x.name.trim().is_empty() && !x.stream_url.trim().is_empty());
    items.sort_by(|a, b| {
        canonical_radio_name(&a.name)
            .cmp(&canonical_radio_name(&b.name))
            .then_with(|| a.stream_url.cmp(&b.stream_url))
    });
    items.dedup_by(|a, b| {
        canonical_radio_name(&a.name) == canonical_radio_name(&b.name)
            || normalize_stream_url(&a.stream_url) == normalize_stream_url(&b.stream_url)
    });
    items
}
fn radio_label(f: &RadioFavorite, language: Language) -> String {
    if f.language_code == "custom"
        || f.language_code == "it"
        || f.language_code.starts_with("country:")
        || f.language_code.starts_with("city:")
    {
        f.name.clone()
    } else {
        format!(
            "{} ({})",
            f.name,
            language_label(language, &f.language_code)
        )
    }
}
fn language_label(language: Language, code: &str) -> String {
    if let Some((iso_code, raw_name)) = dynamic_language_parts(code) {
        if let Some(label) =
            iso_code.and_then(|iso| locale_display_names::language_name(language, iso))
        {
            return label;
        }
        return title_case_radio_directory_label(raw_name);
    }
    locale_display_names::language_name(language, code)
        .or_else(|| {
            radio_menu_languages(language)
                .into_iter()
                .find(|(candidate, _)| candidate == code)
                .map(|(_, label)| label)
        })
        .unwrap_or_else(|| code.to_string())
}

fn normalized_radio_name(value: &str) -> String {
    value
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn canonical_radio_name(value: &str) -> String {
    let mut n = normalized_radio_name(value);
    if let Some(rest) = n.strip_prefix("radio rai ") {
        n = format!("rai radio {rest}");
    }
    n.replace("rai radiouno", "rai radio 1")
        .replace("rai radiodue", "rai radio 2")
        .replace("rai radiotre", "rai radio 3")
        .replace("rai radio1", "rai radio 1")
        .replace("rai radio2", "rai radio 2")
        .replace("rai radio3", "rai radio 3")
        .replace("rai radio uno", "rai radio 1")
        .replace("rai radio due", "rai radio 2")
        .replace("rai radio tre", "rai radio 3")
}
fn normalize_radio_stations(mut stations: Vec<RadioStation>) -> Vec<RadioStation> {
    for s in &mut stations {
        s.name = s
            .name
            .replace('&', "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if canonical_radio_name(&s.name) == "rai radio tutta italiana" {
            s.name = "Rai Radio Tutta Italiana".into();
        }
    }
    stations.retain(|s| !s.name.trim().is_empty() && !s.stream_url.trim().is_empty());
    stations.sort_by(|a, b| {
        radio_name_priority(&a.name)
            .cmp(&radio_name_priority(&b.name))
            .then_with(|| a.name.len().cmp(&b.name.len()))
            .then_with(|| a.name.cmp(&b.name))
    });
    stations.dedup_by(|a, b| {
        canonical_radio_name(&a.name) == canonical_radio_name(&b.name)
            || normalize_stream_url(&a.stream_url) == normalize_stream_url(&b.stream_url)
    });
    stations
}
fn radio_name_priority(value: &str) -> (usize, usize, String) {
    let n = normalized_radio_name(value);
    let c = canonical_radio_name(value);
    (
        usize::from(!n.starts_with("rai radio ")),
        usize::from(!n.starts_with("rai ")),
        c,
    )
}
fn radio_search_rank(name: &str, keyword: &str) -> (usize, usize, usize, String) {
    let n = normalized_radio_name(name);
    let c = canonical_radio_name(name);
    let exact = n == keyword;
    let starts = n.starts_with(keyword);
    let word = n.contains(&format!(" {keyword}"));
    let pos = n.find(keyword).unwrap_or(usize::MAX);
    let tier = if exact {
        0
    } else if starts {
        1
    } else if word {
        2
    } else {
        3
    };
    (
        tier,
        if keyword == "rai" && c.starts_with("rai radio ") {
            0
        } else {
            1
        },
        pos,
        c,
    )
}
fn radio_name_matches_keyword(name: &str, keyword: &str) -> bool {
    let c = canonical_radio_name(name);
    c == keyword
        || c.starts_with(&format!("{keyword} "))
        || c.contains(&format!(" {keyword} "))
        || (keyword.len() >= 4 && c.split_whitespace().any(|w| w.starts_with(keyword)))
}
fn radio_browser_language_name(code: &str) -> String {
    if let Some(name) = dynamic_language_name(code) {
        return name.to_string();
    }
    locale_display_names::english_language_name(code).unwrap_or_else(|| code.to_string())
}

fn fetch_community_radio_stations(
    language_code: &str,
    name_query: Option<&str>,
    genre: Option<&str>,
) -> Result<Vec<RadioStation>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("Sonarpad/1.0 (https://sonarpad.com)")
        .build()
        .map_err(|e| e.to_string())?;

    let stations = client
        .get(COMMUNITY_RADIOS_URL)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json::<Vec<CommunityRadioStation>>())
        .map_err(|e| e.to_string())?;

    let global_search = name_query
        .map(str::trim)
        .is_some_and(|query| !query.is_empty());
    let wanted_language = community_language_from_radio_code(language_code);
    let wanted_genre = genre.unwrap_or_default().trim();
    if !global_search && wanted_language.is_none() && wanted_genre.is_empty() {
        return Ok(Vec::new());
    }
    let keyword = name_query.unwrap_or_default();

    let mut seen = HashSet::new();
    let results = stations
        .into_iter()
        .filter(|station| {
            global_search
                || wanted_language.is_none_or(|wanted_language| {
                    station
                        .language
                        .trim()
                        .eq_ignore_ascii_case(wanted_language)
                })
        })
        .filter(|station| {
            wanted_genre.is_empty() || station.genre.trim().eq_ignore_ascii_case(wanted_genre)
        })
        .filter_map(|station| {
            let url = station.url.trim().to_string();
            if url.is_empty() || !seen.insert(normalize_stream_url(&url)) {
                return None;
            }

            let mut name = station.name.trim().replace('&', "");
            if name.is_empty() {
                name = url.clone();
            } else if !station.genre_label.trim().is_empty() {
                name = format!("{} - {}", name, station.genre_label.trim());
            }

            if !keyword.is_empty() && !radio_name_matches_keyword(&name, keyword) {
                return None;
            }

            Some(RadioStation {
                name,
                stream_url: url,
            })
        })
        .collect();

    Ok(normalize_radio_stations(results))
}

fn community_language_from_radio_code(code: &str) -> Option<&'static str> {
    let canonical_code = canonical_language_option_code(code);
    match canonical_code.as_str() {
        "it" | "country:it" => Some("italian"),
        "en" | "country:us" | "country:gb" | "country:ca" | "country:au" | "country:ie" => {
            Some("english")
        }
        "es" | "country:es" | "country:mx" | "country:ar" => Some("spanish"),
        "fr" | "country:fr" | "country:be" | "country:ch" => Some("french"),
        "pt" | "country:pt" | "country:br" => Some("portuguese"),
        "sv" | "country:se" => Some("swedish"),
        "tr" | "country:tr" => Some("turkish"),
        "vi" => Some("vietnamese"),
        "cs" | "country:cz" => Some("czech"),
        "pl" | "country:pl" => Some("polish"),
        "sr" => Some("serbian"),
        "uk" | "country:ua" => Some("ukrainian"),
        "lt" | "country:lt" => Some("lithuanian"),
        "ru" | "country:ru" => Some("russian"),
        "zh" | "country:cn" => Some("chinese"),
        "hi" | "country:in" => Some("hindi"),
        "de" | "country:de" | "country:at" => Some("german"),
        _ => None,
    }
}

fn fetch_radio_browser_stations(
    language_code: &str,
    name_query: Option<&str>,
    genre: Option<&str>,
    page: usize,
) -> Result<SourcePage, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("Sonarpad/1.0 (https://sonarpad.com)")
        .build()
        .map_err(|e| e.to_string())?;
    let mut last = None;
    for mirror in radio_browser_mirrors() {
        let mut url =
            Url::parse(&format!("{mirror}/json/stations/search")).map_err(|e| e.to_string())?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("hidebroken", "true");
            query.append_pair("order", "votes");
            query.append_pair("reverse", "true");
            query.append_pair("limit", RADIO_BROWSER_LIMIT);
            query.append_pair("offset", &(page * 100).to_string());
            let global_search = name_query
                .map(str::trim)
                .is_some_and(|name| !name.is_empty());
            if let Some(name) = name_query.map(str::trim).filter(|s| !s.is_empty()) {
                query.append_pair("name", name);
            }
            if let Some(tag) = genre.map(str::trim).filter(|s| !s.is_empty()) {
                query.append_pair("tag", tag);
            }
            if !global_search {
                if let Some(country) = language_code.strip_prefix("country:") {
                    query.append_pair("countrycode", &country.to_uppercase());
                } else if let Some(city) = language_code.strip_prefix("city:") {
                    query.append_pair("state", city);
                } else if language_code != "all" {
                    let browser_language = radio_browser_language_name(language_code);
                    query.append_pair("language", &browser_language);
                    query.append_pair("languageExact", "true");
                }
            }
        }
        let request_url = url.to_string();
        crate::log_debug(&format!("Radio Browser search URL: {}", request_url));
        match client
            .get(url)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json::<Vec<RadioBrowserStation>>())
        {
            Ok(v) => {
                let has_more = v.len() >= 100;
                let mut seen = HashSet::new();
                let stations = v
                    .into_iter()
                    .filter(|s| s.lastcheckok == 1)
                    .filter_map(|s| {
                        let url = if s.url_resolved.trim().is_empty() {
                            s.url.trim().to_string()
                        } else {
                            s.url_resolved.trim().to_string()
                        };
                        if url.is_empty() || !seen.insert(normalize_stream_url(&url)) {
                            return None;
                        }
                        let name = if s.name.trim().is_empty() {
                            url.clone()
                        } else if s.tags.trim().is_empty() {
                            s.name.replace('&', "")
                        } else {
                            format!("{} - {}", s.name.replace('&', ""), s.tags)
                        };
                        Some(RadioStation {
                            name,
                            stream_url: url,
                        })
                    })
                    .collect();
                let stations = normalize_radio_stations(stations)
                    .into_iter()
                    .filter(|station| {
                        name_query.is_none_or(|keyword| {
                            keyword.is_empty() || radio_name_matches_keyword(&station.name, keyword)
                        })
                    })
                    .collect();
                return Ok(SourcePage { stations, has_more });
            }
            Err(e) => {
                crate::log_debug(&format!(
                    "Radio Browser search failed: url={} error={} debug={:?}",
                    request_url, e, e
                ));
                last = Some(format!("{e:?}; url={request_url}"));
            }
        }
    }
    Err(last.unwrap_or_else(|| "radio browser request failed".to_string()))
}

fn normalize_stream_url(raw_url: &str) -> String {
    let raw_url = raw_url.trim();
    match Url::parse(raw_url) {
        Ok(mut parsed) => {
            if let Err(err) = parsed.set_scheme("http") {
                crate::log_debug(&format!("Radio: normalize scheme failed: {:?}", err));
            }
            let host = parsed.host_str().unwrap_or("").to_lowercase();
            let path = parsed.path().trim_end_matches('/');
            let mut key = format!("{host}{path}");
            if let Some(query) = parsed.query() {
                key.push('?');
                key.push_str(query);
            }
            key
        }
        Err(_) => raw_url
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .trim_end_matches('/')
            .to_lowercase(),
    }
}

#[cfg(test)]
mod radio_directory_tests {
    use super::*;

    #[test]
    fn radio_browser_language_names_are_mapped_to_standard_codes() {
        assert_eq!(
            radio_code_from_browser_language("Italian").as_deref(),
            Some("it")
        );
        assert_eq!(
            radio_code_from_browser_language("en").as_deref(),
            Some("en")
        );
        assert_eq!(
            radio_code_from_browser_language("German").as_deref(),
            Some("de")
        );
        assert_eq!(
            radio_code_from_browser_language("Türkçe").as_deref(),
            Some("tr")
        );
        assert_eq!(
            radio_code_from_browser_language("Čeština").as_deref(),
            Some("cs")
        );
        assert_eq!(
            radio_code_from_browser_language("American English").as_deref(),
            Some("en-us")
        );
    }

    #[test]
    fn uncommon_standard_languages_are_fully_localized() {
        let code = dynamic_language_code(Some("ace"), "acehnese");
        assert_eq!(code, "language:ace:acehnese");
        assert_eq!(
            dynamic_language_parts(&code),
            Some((Some("ace"), "acehnese"))
        );
        assert_eq!(language_label(Language::Italian, &code), "accinese");
        assert_eq!(language_label(Language::French, &code), "aceh");
        assert_eq!(radio_browser_language_name(&code), "acehnese");
    }

    #[test]
    fn foreign_language_names_are_translated_and_combined_labels_are_preserved() {
        let entries = vec![
            RadioBrowserDirectoryEntry {
                name: "Английский Русский".to_string(),
                iso_639: None,
                stationcount: 12,
            },
            RadioBrowserDirectoryEntry {
                name: "Беларуская".to_string(),
                iso_639: None,
                stationcount: 7,
            },
            RadioBrowserDirectoryEntry {
                name: "ภาษาไทย".to_string(),
                iso_639: None,
                stationcount: 5,
            },
        ];
        let options = language_options_from_directory_entries(Language::Italian, entries);
        assert_eq!(options.len(), 3);
        assert_eq!(options[0].label, "inglese / russo");
        assert_eq!(options[1].label, "bielorusso");
        assert_eq!(options[2].label, "thailandese");
    }

    #[test]
    fn invalid_radio_browser_language_labels_are_filtered_out() {
        let entries = [
            "#japan",
            "+7 Languages",
            "10 Additional Languages",
            "111",
            "80s",
            "Aboriginal Languages",
            "Afghan",
            "音乐",
            "中国",
            "Неизвестен",
        ]
        .into_iter()
        .map(|name| RadioBrowserDirectoryEntry {
            name: name.to_string(),
            iso_639: None,
            stationcount: 1,
        })
        .collect();
        assert!(language_options_from_directory_entries(Language::Italian, entries).is_empty());
    }

    #[test]
    fn numbered_valid_language_labels_are_cleaned_and_translated() {
        let entries = vec![RadioBrowserDirectoryEntry {
            name: "128 Brazilian Portuguese".to_string(),
            iso_639: None,
            stationcount: 3,
        }];
        let options = language_options_from_directory_entries(Language::Italian, entries);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].label, "portoghese (Brasile)");
    }

    #[test]
    fn localized_language_options_do_not_contain_duplicate_visible_rows() {
        let dynamic = vec![
            RadioDirectoryOption {
                code: dynamic_language_code(None, "english russian"),
                label: "inglese / russo".to_string(),
                station_count: 10,
            },
            RadioDirectoryOption {
                code: dynamic_language_code(None, "english/russian"),
                label: "inglese / russo".to_string(),
                station_count: 25,
            },
            RadioDirectoryOption {
                code: dynamic_language_code(None, "английский русский"),
                label: "inglese / russo".to_string(),
                station_count: 15,
            },
            RadioDirectoryOption {
                code: dynamic_language_code(Some("sr-me"), "montenegrin"),
                label: "serbo".to_string(),
                station_count: 4,
            },
            RadioDirectoryOption {
                code: dynamic_language_code(Some("sr"), "serbian"),
                label: "serbo".to_string(),
                station_count: 30,
            },
        ];

        let options = merge_language_directory_options(Language::Italian, dynamic);
        assert_eq!(
            options
                .iter()
                .filter(|(_, label)| label == "inglese / russo")
                .count(),
            1
        );
        assert_eq!(
            options.iter().filter(|(_, label)| label == "serbo").count(),
            1
        );
        assert!(options.iter().any(|(code, label)| {
            label == "inglese / russo" && radio_browser_language_name(code) == "english/russian"
        }));
        assert!(options.iter().any(|(code, label)| {
            label == "serbo" && canonical_language_option_code(code) == "sr"
        }));
    }

    #[test]
    fn country_labels_replace_iso_codes_with_localized_names() {
        assert_eq!(country_name_with_code("Italia", "it"), "Italia (IT)");
        assert_eq!(country_name_with_code("IT", "it"), "IT");
        assert_eq!(
            locale_display_names::territory_name(Language::Italian, "DE").as_deref(),
            Some("Germania")
        );
        assert_eq!(
            locale_display_names::territory_name(Language::Hindi, "DE").as_deref(),
            Some("जर्मनी")
        );
    }

    #[test]
    fn radio_browser_language_directory_response_is_deserialized() {
        let entries: Vec<RadioBrowserDirectoryEntry> = serde_json::from_str(
            r#"[{"name":"italian","iso_639":"it","stationcount":"321"},{"name":"american english","iso_639":null,"stationcount":8},{"name":"afrikaans","stationcount":12}]"#,
        )
        .expect("valid Radio Browser language directory response");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].name, "italian");
        assert_eq!(entries[0].iso_639.as_deref(), Some("it"));
        assert_eq!(entries[0].stationcount, 321);
        assert_eq!(entries[1].iso_639, None);
    }

    #[test]
    fn radio_browser_country_directory_response_is_deserialized() {
        let entries: Vec<RadioBrowserCountryEntry> =
            serde_json::from_str(r#"[{"name":"IT","stationcount":"456"}]"#)
                .expect("valid Radio Browser country-code directory response");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "IT");
        assert_eq!(entries[0].stationcount, 456);
    }
}
