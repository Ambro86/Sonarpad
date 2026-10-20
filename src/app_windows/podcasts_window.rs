use crate::accessibility::{handle_accessibility, nvda_speak, to_wide};
use crate::app_windows::help_window;
use crate::editor_manager;
use crate::i18n;
use crate::settings::{
    self, Language, ListDateDisplayMode, ListTimeDisplayMode, PodcastSearchProvider, confirm_title,
};
use crate::tools::rss::{self, PodcastEpisode, RssSource, RssSourceType};
use crate::{log_debug, with_state};
use chrono::{Local, NaiveDate, TimeZone};
use quick_xml::{Reader, events::Event};
use sha1::{Digest as Sha1Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::Controls::Dialogs::{
    OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};
use windows::Win32::UI::Controls::{
    HTREEITEM, NM_RETURN, NMTVKEYDOWN, TVGN_CARET, TVGN_CHILD, TVGN_NEXT, TVGN_PARENT, TVGN_ROOT,
    TVIF_CHILDREN, TVIF_PARAM, TVIF_TEXT, TVINSERTSTRUCTW, TVITEMEXW_CHILDREN, TVITEMW,
    TVM_DELETEITEM, TVM_ENSUREVISIBLE, TVM_EXPAND, TVM_GETITEMW, TVM_GETNEXTITEM, TVM_INSERTITEMW,
    TVM_SELECTITEM, TVM_SETITEMW, TVM_SORTCHILDRENCB, TVN_ITEMEXPANDINGW, TVN_KEYDOWN,
    TVN_SELCHANGEDW, TVSORTCB,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, SetActiveWindow, SetFocus, VK_APPS, VK_CONTROL, VK_DELETE,
    VK_ESCAPE, VK_F10, VK_LEFT, VK_MENU, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBS_DROPDOWNLIST, CHILDID_SELF,
    CallWindowProcW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, ES_AUTOHSCROLL,
    EVENT_OBJECT_FOCUS, GetClientRect, GetDlgItem, GetParent, GetWindowLongPtrW, GetWindowRect,
    HMENU, IDC_ARROW, IDYES, LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL, LBN_DBLCLK,
    LBS_NOTIFY, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, MB_YESNOCANCEL, MF_GRAYED,
    MF_POPUP, MF_SEPARATOR, MF_STRING, MSG, MessageBoxW, OBJID_CLIENT, PostMessageW,
    RegisterClassW, SendMessageW, SetForegroundWindow, SetWindowLongPtrW, SetWindowTextW,
    TrackPopupMenu, WINDOW_STYLE, WM_CHAR, WM_COMMAND, WM_CONTEXTMENU, WM_COPYDATA, WM_CREATE,
    WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_NEXTDLGCTL, WM_NOTIFY, WM_SETFOCUS, WM_SETFONT,
    WM_SIZE, WNDCLASSW, WNDPROC, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT,
    WS_EX_DLGMODALFRAME, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{PCWSTR, PWSTR, w};

const PODCASTS_WINDOW_CLASS: &str = "SonarpadPodcasts";
const PODCASTS_REORDER_CLASS: &str = "SonarpadPodcastsReorder";
const PODCASTS_ADD_CLASS: &str = "SonarpadPodcastsAdd";
const PODCASTS_CATEGORIES_CLASS: &str = "SonarpadPodcastsCategories";

const ID_TREE: usize = 12001;
const ID_SEARCH_LABEL: usize = 12005;
const ID_SEARCH_EDIT: usize = 12002;
const ID_SEARCH_PROVIDER: usize = 12011;
const ID_SEARCH_BUTTON: usize = 12006;
const ID_SEARCH_CATEGORIES_BUTTON: usize = 12012;
const ID_RESULTS: usize = 12003;
const ID_ADD_BUTTON: usize = 12004;
const ID_IMPORT_BUTTON: usize = 12009;
const ID_EXPORT_BUTTON: usize = 12010;
const ID_DELETE_BUTTON: usize = 12008;
const ID_CLOSE_BUTTON: usize = 12007;

const REORDER_EDIT_ID: usize = 12101;
const REORDER_OK_ID: usize = 12102;
const REORDER_CANCEL_ID: usize = 12103;

const ADD_URL_EDIT_ID: usize = 12201;
const ADD_OK_ID: usize = 12202;
const ADD_CANCEL_ID: usize = 12203;

const WM_PODCAST_FETCH_COMPLETE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 310;
const WM_PODCAST_SEARCH_COMPLETE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 311;

const PODCAST_DIRECTORY_COUNTRIES: &[(&str, &str)] = &[
    ("ae", "United Arab Emirates"),
    ("al", "Albania"),
    ("am", "Armenia"),
    ("at", "Austria"),
    ("au", "Australia"),
    ("az", "Azerbaijan"),
    ("bd", "Bangladesh"),
    ("be", "Belgium"),
    ("bg", "Bulgaria"),
    ("bo", "Bolivia"),
    ("bn", "Brunei"),
    ("br", "Brazil"),
    ("bt", "Bhutan"),
    ("bw", "Botswana"),
    ("ca", "Canada"),
    ("ch", "Switzerland"),
    ("ci", "Ivory Coast"),
    ("cl", "Chile"),
    ("cm", "Cameroon"),
    ("cn", "China"),
    ("co", "Colombia"),
    ("cr", "Costa Rica"),
    ("cy", "Cyprus"),
    ("cz", "Czech Republic"),
    ("dk", "Denmark"),
    ("do", "Dominican Republic"),
    ("dz", "Algeria"),
    ("ee", "Estonia"),
    ("ec", "Ecuador"),
    ("eg", "Egypt"),
    ("et", "Ethiopia"),
    ("fi", "Finland"),
    ("fr", "France"),
    ("de", "Germany"),
    ("ge", "Georgia"),
    ("gh", "Ghana"),
    ("gr", "Greece"),
    ("gt", "Guatemala"),
    ("hr", "Croatia"),
    ("hk", "Hong Kong"),
    ("hn", "Honduras"),
    ("hu", "Hungary"),
    ("id", "Indonesia"),
    ("in", "India"),
    ("ie", "Ireland"),
    ("is", "Iceland"),
    ("il", "Israel"),
    ("it", "Italy"),
    ("jp", "Japan"),
    ("jm", "Jamaica"),
    ("jo", "Jordan"),
    ("ke", "Kenya"),
    ("kh", "Cambodia"),
    ("kg", "Kyrgyzstan"),
    ("kr", "South Korea"),
    ("kz", "Kazakhstan"),
    ("kw", "Kuwait"),
    ("la", "Laos"),
    ("lk", "Sri Lanka"),
    ("lt", "Lithuania"),
    ("lb", "Lebanon"),
    ("lu", "Luxembourg"),
    ("lv", "Latvia"),
    ("ma", "Morocco"),
    ("mg", "Madagascar"),
    ("ml", "Mali"),
    ("mk", "North Macedonia"),
    ("mn", "Mongolia"),
    ("mt", "Malta"),
    ("mv", "Maldives"),
    ("mx", "Mexico"),
    ("mu", "Mauritius"),
    ("my", "Malaysia"),
    ("na", "Namibia"),
    ("ng", "Nigeria"),
    ("ni", "Nicaragua"),
    ("nl", "Netherlands"),
    ("np", "Nepal"),
    ("nz", "New Zealand"),
    ("no", "Norway"),
    ("pe", "Peru"),
    ("pa", "Panama"),
    ("ph", "Philippines"),
    ("pk", "Pakistan"),
    ("pl", "Poland"),
    ("pt", "Portugal"),
    ("py", "Paraguay"),
    ("qa", "Qatar"),
    ("ro", "Romania"),
    ("rs", "Serbia"),
    ("sa", "Saudi Arabia"),
    ("sg", "Singapore"),
    ("si", "Slovenia"),
    ("sk", "Slovakia"),
    ("es", "Spain"),
    ("se", "Sweden"),
    ("sn", "Senegal"),
    ("sv", "El Salvador"),
    ("th", "Thailand"),
    ("tn", "Tunisia"),
    ("tr", "Turkey"),
    ("tz", "Tanzania"),
    ("tw", "Taiwan"),
    ("ua", "Ukraine"),
    ("ug", "Uganda"),
    ("uz", "Uzbekistan"),
    ("gb", "United Kingdom"),
    ("us", "United States"),
    ("uy", "Uruguay"),
    ("ve", "Venezuela"),
    ("vn", "Vietnam"),
    ("zm", "Zambia"),
    ("zw", "Zimbabwe"),
    ("ar", "Argentina"),
];
const WM_PODCAST_PLAY_READY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 312;
const WM_PODCAST_PLAY_FAILED: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 313;
const WM_PODCAST_DOWNLOAD_PROGRESS: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 314;
const WM_PODCAST_CATEGORIES_READY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 315;
const WM_PODCAST_BACKGROUND_CHECK_COMPLETE: u32 =
    windows::Win32::UI::WindowsAndMessaging::WM_USER + 316;
const WM_PODCAST_MARK_EPISODE_PLAYED_UI: u32 =
    windows::Win32::UI::WindowsAndMessaging::WM_USER + 317;
const WM_PODCAST_DOWNLOAD_HEARTBEAT: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 318;

const EM_SETSEL: u32 = 0x00B1;

const ID_CTX_UPDATE: usize = 13001;
const ID_CTX_REMOVE: usize = 13002;
const ID_CTX_COPY_URL: usize = 13003;
const ID_CTX_OPEN_FEED: usize = 13004;
const ID_CTX_REORDER_UP: usize = 13005;
const ID_CTX_REORDER_DOWN: usize = 13006;
const ID_CTX_REORDER_TOP: usize = 13007;
const ID_CTX_REORDER_BOTTOM: usize = 13008;
const ID_CTX_REORDER_POSITION: usize = 13009;
const ID_CTX_SORT_ASC: usize = 13010;
const ID_CTX_SORT_DESC: usize = 13011;
const ID_CTX_SORT_NEWEST: usize = 13012;
const ID_CTX_SORT_OLDEST: usize = 13013;
const ID_CTX_UNDO_DELETE: usize = 13014;

const ID_CTX_PLAY: usize = 13101;
const ID_CTX_OPEN_EPISODE: usize = 13102;
const ID_CTX_COPY_AUDIO: usize = 13103;
const ID_CTX_COPY_TITLE: usize = 13104;
const ID_CTX_DOWNLOAD_EPISODE: usize = 13105;
const ID_CTX_VIEW_DESCRIPTION: usize = 13106;
const ID_CTX_REMOVE_EPISODE: usize = 13107;
const ID_CTX_PROPERTIES: usize = 13108;

const ID_CTX_SUBSCRIBE: usize = 13201;
const ID_CTX_SEARCH_INFO: usize = 13202;
const ID_CTX_SEARCH_COPY_URL: usize = 13203;
const ID_CTX_SEARCH_SHOW_EPISODES: usize = 13204;
const PODCAST_ADD_COPYDATA: usize = 0x504F4443;

const CATEGORIES_SOURCE_COMBO_ID: usize = 14101;
const CATEGORIES_MODE_COMBO_ID: usize = 14102;
const CATEGORIES_LIST_ID: usize = 14103;
const CATEGORIES_TERM_EDIT_ID: usize = 14104;
const CATEGORIES_OPEN_ID: usize = 14105;
const CATEGORIES_CANCEL_ID: usize = 14106;
const CATEGORIES_STATUS_ID: usize = 14107;
const CATEGORY_RESULTS_PAGE_SIZE: usize = 50;
const APPLE_MAX_LIMIT: u32 = 200;

#[derive(Clone)]
struct PodcastSearchResult {
    title: String,
    artist: String,
    feed_url: String,
}

#[derive(Clone, Copy)]
enum SearchProvider {
    Itunes,
    PodcastIndex,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Category {
    id: u32,
    name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Apple,
    PodcastIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Top,
    SearchInCategory,
}

struct PodcastWindowState {
    parent: HWND,
    language: Language,
    hwnd_tree: HWND,
    hwnd_search_label: HWND,
    hwnd_search: HWND,
    hwnd_search_provider: HWND,
    hwnd_search_button: HWND,
    hwnd_search_categories: HWND,
    hwnd_results: HWND,
    hwnd_add: HWND,
    hwnd_import: HWND,
    hwnd_export: HWND,
    hwnd_delete: HWND,
    hwnd_close: HWND,
    node_data: HashMap<isize, NodeData>,
    source_items: HashMap<isize, SourceItemsState>,
    pending_fetches: HashMap<String, isize>,
    search_results: Vec<PodcastSearchResult>,
    category_results: Option<CategoryResultsState>,
    tree_proc: WNDPROC,
    search_proc: WNDPROC,
    reorder_dialog: HWND,
    last_selected: isize,
    pending_play: Option<String>,
    download_in_progress: bool,
    last_download_progress_pct: u32,
    last_download_progress_at: Option<Instant>,
    preview_sources: Vec<crate::tools::rss::RssSource>,
    removed_history: Vec<PodcastLastRemoved>,
    suppress_tree_selection_events: bool,
}

#[derive(Clone)]
enum PodcastLastRemoved {
    Source {
        index: usize,
        source: RssSource,
    },
    Episode {
        source_index: usize,
        episode: PodcastEpisode,
        key: String,
        position: usize,
    },
}

#[derive(Clone)]
enum NodeData {
    Source(usize),
    PreviewSource(usize),
    Episode(Box<PodcastEpisode>),
}

struct SourceItemsState {
    items: Vec<PodcastEpisode>,
    read_item_keys: HashSet<String>,
}

fn parse_opml_sources(text: &str) -> Vec<(String, String)> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if !e.name().as_ref().eq_ignore_ascii_case(b"outline") {
                    buf.clear();
                    continue;
                }
                let mut url = String::new();
                let mut title = String::new();
                for attr in e.attributes().flatten() {
                    let key = attr.key.as_ref();
                    let value = attr
                        .decode_and_unescape_value(reader.decoder())
                        .unwrap_or_default()
                        .to_string();
                    if key.eq_ignore_ascii_case(b"xmlUrl") {
                        url = value;
                    } else if title.is_empty()
                        && (key.eq_ignore_ascii_case(b"title") || key.eq_ignore_ascii_case(b"text"))
                    {
                        title = value;
                    }
                }
                if !url.trim().is_empty() {
                    out.push((title, url));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn parse_single_path(buffer: &[u16]) -> Option<PathBuf> {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    if end == 0 {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(&buffer[..end])))
}

fn ensure_opml_extension(path: PathBuf) -> PathBuf {
    if path.extension().is_some() {
        path
    } else {
        path.with_extension("opml")
    }
}

fn open_opml_file_dialog(hwnd: HWND, language: Language, for_import: bool) -> Option<PathBuf> {
    let raw_filter = i18n::tr(language, "rss.import_filter");
    let filter = to_wide(&raw_filter.replace("\\0", "\0"));
    let mut buffer = vec![0u16; 4096];
    let default_name = if for_import {
        None
    } else {
        Some(to_wide("Sonarpad Podcast.opml"))
    };
    if let Some(name) = &default_name {
        let copy_len = name.len().min(buffer.len().saturating_sub(1));
        buffer[..copy_len].copy_from_slice(&name[..copy_len]);
    }
    let default_ext = to_wide("opml");
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrDefExt: PCWSTR(default_ext.as_ptr()),
        Flags: OFN_EXPLORER
            | OFN_HIDEREADONLY
            | OFN_PATHMUSTEXIST
            | if for_import {
                OFN_FILEMUSTEXIST
            } else {
                OFN_OVERWRITEPROMPT
            },
        ..Default::default()
    };
    let success = if for_import {
        crate::get_open_file_name_w_safe(&mut ofn).as_bool()
    } else {
        crate::get_save_file_name_w_safe(&mut ofn).as_bool()
    };
    if !success {
        return None;
    }
    let path = parse_single_path(&buffer)?;
    if for_import {
        Some(path)
    } else {
        Some(ensure_opml_extension(path))
    }
}

fn normalize_podcast_key(url: &str) -> String {
    rss::normalize_url(url).to_ascii_lowercase()
}

fn podcast_source_display_title(
    source: &RssSource,
    language: crate::settings::Language,
    announce_unread: bool,
    unread_label_position: crate::settings::RssPodcastUnreadLabelPosition,
) -> String {
    let base_title = if source.title.trim().is_empty() {
        source.url.clone()
    } else {
        source.title.clone()
    };
    if announce_unread && source.unread {
        match unread_label_position {
            crate::settings::RssPodcastUnreadLabelPosition::Before => format!(
                "{}{}",
                i18n::tr(language, "podcasts.unheard_prefix"),
                base_title
            ),
            crate::settings::RssPodcastUnreadLabelPosition::After => {
                format!(
                    "{base_title}{}",
                    i18n::tr(language, "podcasts.unheard_suffix")
                )
            }
        }
    } else {
        base_title
    }
}

#[derive(Clone, Copy)]
struct PodcastEpisodeTitleContext {
    language: crate::settings::Language,
    announce_unread: bool,
    unread_label_position: crate::settings::RssPodcastUnreadLabelPosition,
    date_mode: ListDateDisplayMode,
    time_mode: ListTimeDisplayMode,
}

fn podcast_episode_display_title(
    title: &str,
    item_unplayed: bool,
    pub_date: Option<i64>,
    has_multiple_items_same_day: bool,
    ctx: PodcastEpisodeTitleContext,
) -> String {
    let ts_suffix = format_timestamp_for_list(
        pub_date,
        ctx.language,
        ctx.date_mode,
        ctx.time_mode,
        has_multiple_items_same_day,
    )
    .map(|ts| format!(". {ts}"))
    .unwrap_or_default();
    if ctx.announce_unread && item_unplayed {
        match ctx.unread_label_position {
            crate::settings::RssPodcastUnreadLabelPosition::Before => format!(
                "{}{}{}",
                i18n::tr(ctx.language, "podcasts.item_unplayed_prefix"),
                title,
                ts_suffix
            ),
            crate::settings::RssPodcastUnreadLabelPosition::After => format!(
                "{title}{ts_suffix}{}",
                i18n::tr(ctx.language, "podcasts.item_unplayed_suffix")
            ),
        }
    } else {
        format!("{title}{ts_suffix}")
    }
}

fn day_from_timestamp(timestamp: Option<i64>) -> Option<NaiveDate> {
    let ts = timestamp?;
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.date_naive())
}

fn build_day_counts(items: &[PodcastEpisode]) -> HashMap<NaiveDate, usize> {
    let mut counts: HashMap<NaiveDate, usize> = HashMap::new();
    for item in items {
        if let Some(day) = day_from_timestamp(item.pub_date) {
            *counts.entry(day).or_insert(0) += 1;
        }
    }
    counts
}

fn has_multiple_items_same_day(
    pub_date: Option<i64>,
    day_counts: &HashMap<NaiveDate, usize>,
) -> bool {
    day_from_timestamp(pub_date)
        .and_then(|d| day_counts.get(&d).copied())
        .is_some_and(|count| count > 1)
}

fn format_timestamp_for_list(
    timestamp: Option<i64>,
    language: crate::settings::Language,
    date_mode: ListDateDisplayMode,
    time_mode: ListTimeDisplayMode,
    has_multiple_same_day: bool,
) -> Option<String> {
    let ts = timestamp?;
    let dt = Local.timestamp_opt(ts, 0).single()?;
    let (date_pattern, time_pattern) = match language {
        crate::settings::Language::German => ("%d.%m.%Y", "%H:%M"),
        crate::settings::Language::English
        | crate::settings::Language::Lithuanian
        | crate::settings::Language::Chinese => ("%m/%d/%Y", "%I:%M %p"),
        crate::settings::Language::Italian => ("%d/%m/%Y", "%H:%M"),
        crate::settings::Language::Spanish => ("%d/%m/%Y", "%H:%M"),
        crate::settings::Language::Portuguese | crate::settings::Language::PortugueseBrazilian => {
            ("%d/%m/%Y", "%H:%M")
        }
        crate::settings::Language::Swedish => ("%Y-%m-%d", "%H:%M"),
        crate::settings::Language::Vietnamese => ("%d/%m/%Y", "%H:%M"),
        crate::settings::Language::Czech => ("%d.%m.%Y", "%H:%M"),
        crate::settings::Language::Polish => ("%d.%m.%Y", "%H:%M"),
        crate::settings::Language::French => ("%d/%m/%Y", "%H:%M"),
        crate::settings::Language::Serbian => ("%d.%m.%Y", "%H:%M"),
        crate::settings::Language::Ukrainian => ("%d.%m.%Y", "%H:%M"),
        crate::settings::Language::Russian => ("%d.%m.%Y", "%H:%M"),
        crate::settings::Language::Hindi => ("%d/%m/%Y", "%H:%M"),
    };
    let show_date = matches!(date_mode, ListDateDisplayMode::Always);
    let show_time = match time_mode {
        ListTimeDisplayMode::Always => true,
        ListTimeDisplayMode::Never => false,
        ListTimeDisplayMode::OnlyIfMultipleSameDay => has_multiple_same_day,
    };
    if !show_date && !show_time {
        return None;
    }
    if show_date && !show_time {
        let now = Local::now().date_naive();
        let item_day = dt.date_naive();
        let day_diff = (now - item_day).num_days();
        if day_diff == 0 {
            return Some(i18n::tr(language, "rss.date.today"));
        }
        if day_diff == 1 {
            return Some(i18n::tr(language, "rss.date.yesterday"));
        }
        if day_diff == 2 {
            return Some(i18n::tr(language, "rss.date.day_before_yesterday"));
        }
        return Some(dt.format(date_pattern).to_string());
    }
    if !show_date && show_time {
        return Some(dt.format(time_pattern).to_string());
    }
    format_timestamp_for_language(timestamp, language)
}

fn format_timestamp_for_language(
    timestamp: Option<i64>,
    language: crate::settings::Language,
) -> Option<String> {
    let ts = timestamp?;
    let dt = Local.timestamp_opt(ts, 0).single()?;
    let (date_pattern, time_pattern) = match language {
        crate::settings::Language::German => ("%d.%m.%Y", "%H:%M"),
        crate::settings::Language::English
        | crate::settings::Language::Lithuanian
        | crate::settings::Language::Chinese => ("%m/%d/%Y", "%I:%M %p"),
        crate::settings::Language::Italian => ("%d/%m/%Y", "%H:%M"),
        crate::settings::Language::Spanish => ("%d/%m/%Y", "%H:%M"),
        crate::settings::Language::Portuguese | crate::settings::Language::PortugueseBrazilian => {
            ("%d/%m/%Y", "%H:%M")
        }
        crate::settings::Language::Swedish => ("%Y-%m-%d", "%H:%M"),
        crate::settings::Language::Vietnamese => ("%d/%m/%Y", "%H:%M"),
        crate::settings::Language::Czech => ("%d.%m.%Y", "%H:%M"),
        crate::settings::Language::Polish => ("%d.%m.%Y", "%H:%M"),
        crate::settings::Language::French => ("%d/%m/%Y", "%H:%M"),
        crate::settings::Language::Serbian => ("%d.%m.%Y", "%H:%M"),
        crate::settings::Language::Ukrainian => ("%d.%m.%Y", "%H:%M"),
        crate::settings::Language::Russian => ("%d.%m.%Y", "%H:%M"),
        crate::settings::Language::Hindi => ("%d/%m/%Y", "%H:%M"),
    };
    let now = Local::now().date_naive();
    let item_day = dt.date_naive();
    let day_diff = (now - item_day).num_days();
    let time_text = dt.format(time_pattern).to_string();
    if day_diff == 0 {
        return Some(format!(
            "{} {time_text}",
            i18n::tr(language, "rss.date.today")
        ));
    }
    if day_diff == 1 {
        return Some(format!(
            "{} {time_text}",
            i18n::tr(language, "rss.date.yesterday")
        ));
    }
    if day_diff == 2 {
        return Some(format!(
            "{} {time_text}",
            i18n::tr(language, "rss.date.day_before_yesterday")
        ));
    }
    let full_pattern = format!("{date_pattern} {time_pattern}");
    Some(dt.format(&full_pattern).to_string())
}

struct MarkEpisodePlayedUiMessage {
    hitem: isize,
    item_key: String,
    retries_left: u8,
}

fn post_mark_episode_played_ui_after_delay(
    hwnd: HWND,
    payload: MarkEpisodePlayedUiMessage,
    delay_ms: u64,
) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        let payload_ptr = Box::into_raw(Box::new(payload));
        if let Err(e) = crate::post_message_w_safe(
            hwnd,
            WM_PODCAST_MARK_EPISODE_PLAYED_UI,
            WPARAM(0),
            LPARAM(payload_ptr as isize),
        ) {
            let _payload_owner = crate::box_from_raw_safe(payload_ptr);
            log_debug(&format!(
                "Failed to post WM_PODCAST_MARK_EPISODE_PLAYED_UI: {}",
                e
            ));
        }
    });
}

fn import_podcast_sources_from_file(hwnd: HWND, path: &Path) -> Option<usize> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let is_opml = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("opml"))
        .unwrap_or(false)
        || text.to_ascii_lowercase().contains("<opml");
    let sources = if is_opml {
        parse_opml_sources(&text)
    } else {
        Vec::new()
    };
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return None;
    }
    let mut added = 0usize;
    {
        if with_state(parent, |state| {
            let mut existing: HashSet<String> = state
                .settings
                .podcast_sources
                .iter()
                .map(|src| normalize_podcast_key(&src.url))
                .filter(|k| !k.is_empty())
                .collect();
            for (mut title, url_raw) in sources {
                let url = rss::normalize_url(&url_raw);
                if url.is_empty() {
                    continue;
                }
                let key = normalize_podcast_key(&url);
                if key.is_empty() || existing.contains(&key) {
                    continue;
                }
                if title.trim().is_empty() {
                    title = url.clone();
                }
                state.settings.podcast_sources.push(rss::RssSource {
                    title: title.clone(),
                    url: url.clone(),
                    kind: rss::RssSourceType::Feed,
                    folder_path: Vec::new(),
                    user_title: title.trim() != url.trim(),
                    unread: true,
                    cache: rss::RssFeedCache::default(),
                    last_seen_guid: None,
                    last_updated: None,
                    removed_item_keys: Vec::new(),
                    read_item_keys: Vec::new(),
                });
                existing.insert(key);
                added += 1;
            }
            if added > 0 {
                crate::settings::save_settings(state.settings.clone());
            }
        })
        .is_none()
        {
            crate::log_debug("Failed to access state in import_opml");
        }
    }
    if added > 0 {
        reload_tree(hwnd);
    }
    Some(added)
}

fn escape_opml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn export_podcast_sources_to_file(hwnd: HWND, path: &Path) -> Result<usize, String> {
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return Err("missing parent".to_string());
    }
    let sources =
        { with_state(parent, |state| state.settings.podcast_sources.clone()) }.unwrap_or_default();
    if sources.is_empty() {
        return Ok(0);
    }
    let mut file = File::create(path).map_err(|e| e.to_string())?;
    writeln!(
        file,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"1.0\">\n<head>\n<title>Sonarpad Podcasts</title>\n</head>\n<body>"
    )
    .map_err(|e| e.to_string())?;
    for src in sources.iter() {
        let title = if src.title.trim().is_empty() {
            src.url.clone()
        } else {
            src.title.clone()
        };
        writeln!(
            file,
            "  <outline text=\"{}\" title=\"{}\" type=\"rss\" xmlUrl=\"{}\" />",
            escape_opml_attr(&title),
            escape_opml_attr(&title),
            escape_opml_attr(&src.url)
        )
        .map_err(|e| e.to_string())?;
    }
    writeln!(file, "</body>\n</opml>").map_err(|e| e.to_string())?;
    Ok(sources.len())
}

fn handle_import_opml(hwnd: HWND) {
    let language = with_podcast_state(hwnd, |s| s.language).unwrap_or_default();
    if let Some(path) = open_opml_file_dialog(hwnd, language, true) {
        if let Some(count) = import_podcast_sources_from_file(hwnd, &path) {
            if count > 0 {
                announce_status(&i18n::tr(language, "podcasts.imported"));
                let title = i18n::tr(language, "podcasts.window.title");
                let count_text = count.to_string();
                let path_text = path.display().to_string();
                let message = i18n::tr_f(
                    language,
                    "podcasts.import_success",
                    &[("count", &count_text), ("path", &path_text)],
                );
                unsafe {
                    MessageBoxW(
                        hwnd,
                        PCWSTR(to_wide(&message).as_ptr()),
                        PCWSTR(to_wide(&title).as_ptr()),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
                focus_library(hwnd);
            }
        } else {
            let title = i18n::tr(language, "podcasts.window.title");
            let message = i18n::tr(language, "podcasts.import_failed");
            unsafe {
                MessageBoxW(
                    hwnd,
                    PCWSTR(to_wide(&message).as_ptr()),
                    PCWSTR(to_wide(&title).as_ptr()),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
        }
    }
}

fn handle_export_opml(hwnd: HWND) {
    let language = with_podcast_state(hwnd, |s| s.language).unwrap_or_default();
    if let Some(path) = open_opml_file_dialog(hwnd, language, false) {
        match export_podcast_sources_to_file(hwnd, &path) {
            Ok(count) => {
                if count > 0 {
                    announce_status(&i18n::tr(language, "podcasts.exported"));
                    let title = i18n::tr(language, "podcasts.window.title");
                    let count_text = count.to_string();
                    let path_text = path.display().to_string();
                    let message = i18n::tr_f(
                        language,
                        "podcasts.export_success",
                        &[("count", &count_text), ("path", &path_text)],
                    );
                    unsafe {
                        MessageBoxW(
                            hwnd,
                            PCWSTR(to_wide(&message).as_ptr()),
                            PCWSTR(to_wide(&title).as_ptr()),
                            MB_OK | MB_ICONINFORMATION,
                        );
                    }
                    focus_library(hwnd);
                }
            }
            Err(err) => {
                let title = i18n::tr(language, "podcasts.window.title");
                let message = format!("{}: {}", i18n::tr(language, "podcasts.export_failed"), err);
                unsafe {
                    MessageBoxW(
                        hwnd,
                        PCWSTR(to_wide(&message).as_ptr()),
                        PCWSTR(to_wide(&title).as_ptr()),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ReorderAction {
    Up,
    Down,
    Top,
    Bottom,
    Position,
}

#[derive(Clone, Copy)]
struct ReorderDialogInit {
    parent: HWND,
    source_index: usize,
    total: usize,
}

struct AddDialogInit {
    parent: HWND,
}

struct CategoryDialogInit {
    parent: HWND,
    initial_source: Source,
    initial_term: String,
}

struct CategoryDialogState {
    parent: HWND,
    language: Language,
    hwnd_source_label: HWND,
    hwnd_source: HWND,
    hwnd_mode_label: HWND,
    hwnd_mode: HWND,
    hwnd_list_label: HWND,
    hwnd_list: HWND,
    list_proc: WNDPROC,
    hwnd_term_label: HWND,
    hwnd_term_edit: HWND,
    hwnd_open: HWND,
    hwnd_cancel: HWND,
    hwnd_status: HWND,
    source: Source,
    mode: Mode,
    categories: Vec<Category>,
}

struct CategoryListMsg {
    categories: Vec<Category>,
    error: Option<String>,
}

struct FetchResult {
    hitem: isize,
    node: NodeData,
    result: Result<rss::PodcastFetchOutcome, rss::FeedFetchError>,
}

struct BackgroundCheckResult {
    source_idx: usize,
    newest_item_key: Option<String>,
}

struct SearchResultMsg {
    results: Vec<PodcastSearchResult>,
    status: Option<String>,
    error: Option<String>,
    category_results: Option<CategoryResultsMeta>,
}

#[derive(Clone)]
struct CategoryResultsMeta {
    source: Source,
    mode: Mode,
    category: Category,
    search_term: String,
    visible_start: usize,
    can_load_more: bool,
}

struct CategoryResultsState {
    source: Source,
    mode: Mode,
    category: Category,
    search_term: String,
    loaded_results: Vec<PodcastSearchResult>,
    visible_start: usize,
    can_load_more: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SearchResultsNavAction {
    PreviousPage,
    NextPage,
    LoadMore,
}

struct PlayReadyMsg {
    path: PathBuf,
    enclosure_url: String,
    podcast_title: Option<String>,
    title: String,
    item_key: String,
    chapters_prefetch_started: bool,
}

pub fn handle_navigation(hwnd: HWND, msg: &MSG) -> bool {
    if msg.hwnd != hwnd && !crate::is_child_safe(hwnd, msg.hwnd) {
        return false;
    }
    if msg.message == WM_CHAR {
        return false;
    }
    if msg.message == WM_KEYDOWN {
        let key = msg.wParam.0 as u32;
        if key == VK_ESCAPE.0 as u32 {
            crate::send_message_w_safe(hwnd, WM_COMMAND, WPARAM(2), LPARAM(0));
            return true;
        }
        if key == VK_RETURN.0 as u32 {
            let (hwnd_tree, hwnd_results) =
                with_podcast_state(hwnd, |s| (s.hwnd_tree, s.hwnd_results))
                    .unwrap_or((HWND(0), HWND(0)));
            let focus = crate::get_focus_safe();

            // Handle Enter on search results list
            if hwnd_results.0 != 0 && focus == hwnd_results {
                subscribe_selected_result(hwnd);
                return true;
            }

            // Handle Enter on tree view
            if hwnd_tree.0 != 0
                && focus == hwnd_tree
                && let Some(item) = selected_episode(hwnd)
            {
                let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                open_episode_in_player(hwnd, parent, &item);
                return true;
            }
        }
    }
    handle_accessibility(hwnd, msg)
}

fn announce_status(message: &str) {
    log_debug(&format!("podcasts_status {}", message));
    if !nvda_speak(message) {
        crate::log_debug("NVDA speak failed");
    }
}

fn ensure_rss_http(parent: HWND) {
    let config = {
        with_state(parent, |s| rss::config_from_settings(&s.settings))
            .unwrap_or_else(rss::RssHttpConfig::default)
    };
    if let Err(err) = rss::init_http(config) {
        log_debug(&format!("rss_http_init_error: {}", err));
    }
}

fn rss_fetch_config(parent: HWND) -> rss::RssFetchConfig {
    {
        with_state(parent, |s| rss::fetch_config_from_settings(&s.settings))
            .unwrap_or_else(rss::RssFetchConfig::default)
    }
}

fn copy_text_to_clipboard(hwnd: HWND, text: &str) {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Memory::GMEM_MOVEABLE;

    const CF_UNICODETEXT: u32 = 13;

    let content = to_wide(text);
    if content.is_empty() {
        return;
    }
    if crate::open_clipboard_safe(hwnd).is_err() {
        return;
    }
    if let Err(e) = crate::empty_clipboard_safe() {
        crate::log_debug(&format!("EmptyClipboard failed: {}", e));
    }
    let size = content.len() * std::mem::size_of::<u16>();
    let handle = match crate::global_alloc_safe(GMEM_MOVEABLE, size) {
        Ok(handle) => handle,
        Err(_) => {
            if let Err(e) = crate::close_clipboard_safe() {
                crate::log_debug(&format!("CloseClipboard failed: {}", e));
            }
            return;
        }
    };
    if handle.0.is_null() {
        if let Err(e) = crate::close_clipboard_safe() {
            crate::log_debug(&format!("CloseClipboard failed: {}", e));
        }
        return;
    }
    let ptr = crate::global_lock_as_safe(handle) as *mut u16;
    if ptr.is_null() {
        if let Err(e) = crate::close_clipboard_safe() {
            crate::log_debug(&format!("CloseClipboard failed: {}", e));
        }
        return;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(content.as_ptr(), ptr, content.len());
    }
    crate::log_if_err!(crate::global_unlock_safe(handle));
    if let Err(e) = crate::set_clipboard_data_safe(CF_UNICODETEXT, HANDLE(handle.0 as isize)) {
        crate::log_debug(&format!("SetClipboardData failed: {}", e));
    }
    if let Err(e) = crate::close_clipboard_safe() {
        crate::log_debug(&format!("CloseClipboard failed: {}", e));
    }
}

fn move_vec_to_index<T>(items: &mut Vec<T>, from: usize, to: usize) -> bool {
    if from >= items.len() {
        return false;
    }
    let target = to.min(items.len().saturating_sub(1));
    if from == target {
        return false;
    }
    let item = items.remove(from);
    items.insert(target, item);
    true
}

unsafe extern "system" fn podcast_tree_compare(
    lparam1: LPARAM,
    lparam2: LPARAM,
    _lparam_sort: LPARAM,
) -> i32 {
    crate::panic_guard::guard(
        "podcast_tree_compare",
        || 0,
        || {
            let a = lparam1.0;
            let b = lparam2.0;
            a.cmp(&b) as i32
        },
    )
}

fn collect_root_items(hwnd_tree: HWND) -> Vec<HTREEITEM> {
    let mut items = Vec::new();
    let mut current = HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_ROOT as usize),
            LPARAM(0),
        )
        .0,
    );
    while current.0 != 0 {
        items.push(current);
        current = HTREEITEM(
            crate::send_message_w_safe(
                hwnd_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_NEXT as usize),
                LPARAM(current.0),
            )
            .0,
        );
    }
    items
}

fn apply_root_order(hwnd: HWND, hwnd_tree: HWND, ordered_items: &[HTREEITEM]) {
    for (i, hitem) in ordered_items.iter().enumerate() {
        let mut item = TVITEMW {
            mask: TVIF_PARAM,
            lParam: LPARAM(i as isize),
            ..Default::default()
        };
        item.hItem = *hitem;
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_SETITEMW,
            WPARAM(0),
            LPARAM(&mut item as *mut _ as isize),
        );
    }
    with_podcast_state(hwnd, |s| {
        for (i, hitem) in ordered_items.iter().enumerate() {
            s.node_data.insert(hitem.0, NodeData::Source(i));
        }
    });
    let mut sort_cb = TVSORTCB {
        hParent: windows::Win32::UI::Controls::TVI_ROOT,
        lpfnCompare: Some(podcast_tree_compare),
        lParam: LPARAM(0),
    };
    crate::send_message_w_safe(
        hwnd_tree,
        TVM_SORTCHILDRENCB,
        WPARAM(0),
        LPARAM(&mut sort_cb as *mut _ as isize),
    );
}

fn selected_tree_item(hwnd: HWND) -> HTREEITEM {
    let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return HTREEITEM(0);
    }
    HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    )
}

fn selected_source_index(hwnd: HWND) -> Option<usize> {
    let hitem = selected_tree_item(hwnd);
    selected_source_index_for_item(hwnd, hitem)
}

fn selected_source_index_for_item(hwnd: HWND, hitem: HTREEITEM) -> Option<usize> {
    if hitem.0 == 0 {
        return None;
    }
    let direct = with_podcast_state(hwnd, |s| match s.node_data.get(&hitem.0) {
        Some(NodeData::Source(idx)) => Some(*idx),
        _ => None,
    })
    .flatten();
    if direct.is_some() {
        return direct;
    }
    let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return None;
    }
    let parent_hitem = HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_PARENT as usize),
            LPARAM(hitem.0),
        )
        .0,
    );
    if parent_hitem.0 == 0 {
        return None;
    }
    with_podcast_state(hwnd, |s| match s.node_data.get(&parent_hitem.0) {
        Some(NodeData::Source(idx)) => Some(*idx),
        _ => None,
    })
    .flatten()
}

fn selected_node_data(hwnd: HWND) -> Option<NodeData> {
    let hitem = selected_tree_item(hwnd);
    if hitem.0 == 0 {
        return None;
    }
    with_podcast_state(hwnd, |s| s.node_data.get(&hitem.0).cloned()).flatten()
}

fn selected_source_name(hwnd: HWND) -> Option<String> {
    let index = selected_source_index(hwnd)?;
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return None;
    }
    {
        with_state(parent, |ps| {
            ps.settings.podcast_sources.get(index).map(|src| {
                if src.title.trim().is_empty() {
                    src.url.clone()
                } else {
                    src.title.clone()
                }
            })
        })
        .unwrap_or(None)
    }
}

fn update_delete_button_state(hwnd: HWND) {
    let enabled = selected_source_name(hwnd).is_some();
    unsafe {
        with_podcast_state(hwnd, |state| {
            if state.hwnd_delete.0 != 0 {
                EnableWindow(state.hwnd_delete, enabled);
            }
        });
    }
}

fn selected_episode(hwnd: HWND) -> Option<PodcastEpisode> {
    let hitem = selected_tree_item(hwnd);
    if hitem.0 == 0 {
        return None;
    }
    with_podcast_state(hwnd, |s| match s.node_data.get(&hitem.0) {
        Some(NodeData::Episode(item)) => Some((**item).clone()),
        _ => None,
    })
    .flatten()
}

fn show_selected_properties(hwnd: HWND) {
    let hitem = selected_tree_item(hwnd);
    if hitem.0 == 0 {
        return;
    }
    let language = with_podcast_state(hwnd, |s| s.language).unwrap_or_default();
    let parent_hwnd = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    let node = selected_node_data(hwnd);

    let mut lines: Vec<String> = Vec::new();
    match node {
        Some(NodeData::Source(index)) => {
            let source = {
                with_state(parent_hwnd, |ps| {
                    ps.settings.podcast_sources.get(index).cloned()
                })
            }
            .flatten();
            if let Some(src) = source {
                let title_value = if src.title.trim().is_empty() {
                    src.url.clone()
                } else {
                    src.title
                };
                let status_value = if src.unread {
                    i18n::tr(language, "properties.new_items")
                } else {
                    i18n::tr(language, "properties.no_new_items")
                };
                lines.push(format!(
                    "{}: {}",
                    i18n::tr(language, "properties.type"),
                    i18n::tr(language, "properties.podcast")
                ));
                lines.push(format!(
                    "{}: {}",
                    i18n::tr(language, "properties.title"),
                    title_value
                ));
                lines.push(format!(
                    "{}: {}",
                    i18n::tr(language, "properties.url"),
                    src.url
                ));
                lines.push(format!(
                    "{}: {}",
                    i18n::tr(language, "properties.status"),
                    status_value
                ));
            }
        }
        Some(NodeData::PreviewSource(index)) => {
            let source =
                with_podcast_state(hwnd, |s| s.preview_sources.get(index).cloned()).flatten();
            if let Some(src) = source {
                let title_value = if src.title.trim().is_empty() {
                    src.url.clone()
                } else {
                    src.title
                };
                lines.push(format!(
                    "{}: {}",
                    i18n::tr(language, "properties.type"),
                    i18n::tr(language, "properties.podcast")
                ));
                lines.push(format!(
                    "{}: {}",
                    i18n::tr(language, "properties.title"),
                    title_value
                ));
                lines.push(format!(
                    "{}: {}",
                    i18n::tr(language, "properties.url"),
                    src.url
                ));
            }
        }
        Some(NodeData::Episode(item)) => {
            let item = *item;
            let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
            let parent_item = HTREEITEM(
                crate::send_message_w_safe(
                    hwnd_tree,
                    TVM_GETNEXTITEM,
                    WPARAM(TVGN_PARENT as usize),
                    LPARAM(hitem.0),
                )
                .0,
            );
            let key = episode_key(&item);
            let unplayed = with_podcast_state(hwnd, |s| {
                s.source_items
                    .get(&parent_item.0)
                    .map(|state| !state.read_item_keys.contains(&key))
                    .unwrap_or(true)
            })
            .unwrap_or(true);
            let status_value = if unplayed {
                i18n::tr(language, "properties.unplayed")
            } else {
                i18n::tr(language, "properties.played")
            };
            let date_value = format_timestamp_for_language(item.pub_date, language)
                .unwrap_or_else(|| i18n::tr(language, "properties.not_available"));
            lines.push(format!(
                "{}: {}",
                i18n::tr(language, "properties.type"),
                i18n::tr(language, "properties.episode")
            ));
            lines.push(format!(
                "{}: {}",
                i18n::tr(language, "properties.title"),
                item.title
            ));
            lines.push(format!(
                "{}: {}",
                i18n::tr(language, "properties.url"),
                item.link
            ));
            lines.push(format!(
                "{}: {}",
                i18n::tr(language, "properties.date"),
                date_value
            ));
            lines.push(format!(
                "{}: {}",
                i18n::tr(language, "properties.status"),
                status_value
            ));
        }
        None => return,
    }

    let body = lines.join("\r\n");
    help_window::open_readonly_text(hwnd, &i18n::tr(language, "properties.title_window"), &body);
}

fn episode_key(item: &PodcastEpisode) -> String {
    if !item.guid.trim().is_empty() {
        return item.guid.trim().to_string();
    }
    if !item.link.trim().is_empty() {
        return item.link.trim().to_string();
    }
    item.title.trim().to_string()
}

fn source_removed_episode_keys(hwnd: HWND, hitem: HTREEITEM) -> HashSet<String> {
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return HashSet::new();
    }
    let source_index = with_podcast_state(hwnd, |s| match s.node_data.get(&hitem.0) {
        Some(NodeData::Source(idx)) => Some(*idx),
        _ => None,
    })
    .flatten();
    let Some(idx) = source_index else {
        return HashSet::new();
    };
    {
        with_state(parent, |ps| {
            ps.settings
                .podcast_sources
                .get(idx)
                .map(|src| src.removed_item_keys.iter().cloned().collect())
        })
        .flatten()
        .unwrap_or_default()
    }
}

fn source_read_episode_keys(hwnd: HWND, hitem: HTREEITEM) -> HashSet<String> {
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return HashSet::new();
    }
    let source_index = with_podcast_state(hwnd, |s| match s.node_data.get(&hitem.0) {
        Some(NodeData::Source(idx)) => Some(*idx),
        _ => None,
    })
    .flatten();
    let Some(idx) = source_index else {
        return HashSet::new();
    };
    {
        with_state(parent, |ps| {
            ps.settings
                .podcast_sources
                .get(idx)
                .map(|src| src.read_item_keys.iter().cloned().collect())
        })
        .flatten()
        .unwrap_or_default()
    }
}

fn prune_persisted_played_keys_for_source(hwnd: HWND, hitem: HTREEITEM) {
    let source_index = with_podcast_state(hwnd, |s| match s.node_data.get(&hitem.0) {
        Some(NodeData::Source(idx)) => Some(*idx),
        _ => None,
    })
    .flatten();
    let Some(source_index) = source_index else {
        return;
    };

    let current_item_keys: HashSet<String> = with_podcast_state(hwnd, |s| {
        s.source_items
            .get(&hitem.0)
            .map(|state| state.items.iter().map(episode_key).collect())
            .unwrap_or_default()
    })
    .unwrap_or_default();

    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }
    {
        with_state(parent, |ps| {
            if let Some(src) = ps.settings.podcast_sources.get_mut(source_index) {
                let before = src.read_item_keys.len();
                src.read_item_keys.retain(|k| current_item_keys.contains(k));
                if src.read_item_keys.len() != before {
                    settings::save_settings(ps.settings.clone());
                }
            }
        });
    }
}

fn load_episode_children(hwnd: HWND, hitem: HTREEITEM, node: NodeData, force: bool) {
    let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return;
    }
    let (parent, url, mut cache, force_uncached) = with_podcast_state(hwnd, |s| {
        let parent = s.parent;
        let empty_items = s
            .source_items
            .get(&hitem.0)
            .map(|state| state.items.is_empty())
            .unwrap_or(true);
        let (url, cache) = match node {
            NodeData::Source(idx) => {
                with_state(parent, |ps| {
                    ps.settings
                        .podcast_sources
                        .get(idx)
                        .map(|src| (src.url.clone(), src.cache.clone()))
                })
            }
            .flatten(),
            NodeData::PreviewSource(idx) => s
                .preview_sources
                .get(idx)
                .map(|src| (src.url.clone(), src.cache.clone())),
            _ => None,
        }
        .unwrap_or((String::new(), rss::RssFeedCache::default()));
        (parent, url, cache, empty_items)
    })
    .unwrap_or((HWND(0), String::new(), rss::RssFeedCache::default(), true));
    if parent.0 != 0 {
        ensure_rss_http(parent);
    }
    if url.trim().is_empty() {
        return;
    }
    if force_uncached {
        cache.etag = None;
        cache.last_modified = None;
    }

    let should_fetch = with_podcast_state(hwnd, |s| {
        if s.pending_fetches.contains_key(&url) {
            return false;
        }
        let state = s.source_items.get(&hitem.0);
        if state.is_none() {
            return true;
        }
        if force {
            return true;
        }
        state.map(|s| s.items.is_empty()).unwrap_or(true)
    })
    .unwrap_or(true);

    if !should_fetch {
        return;
    }

    with_podcast_state(hwnd, |s| {
        s.pending_fetches.insert(url.clone(), hitem.0);
    });

    let language = with_podcast_state(hwnd, |s| s.language).unwrap_or_default();
    let loading_txt = to_wide(&i18n::tr(language, "podcasts.loading"));

    let child = HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CHILD as usize),
            LPARAM(hitem.0),
        )
        .0,
    );
    if child.0 == 0 {
        let mut tvis_loading = TVINSERTSTRUCTW {
            hParent: hitem,
            hInsertAfter: windows::Win32::UI::Controls::TVI_LAST,
            Anonymous: windows::Win32::UI::Controls::TVINSERTSTRUCTW_0 {
                item: TVITEMW {
                    mask: TVIF_TEXT,
                    pszText: windows::core::PWSTR(loading_txt.as_ptr() as *mut _),
                    cchTextMax: loading_txt.len() as i32,
                    ..Default::default()
                },
            },
        };
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_INSERTITEMW,
            WPARAM(0),
            LPARAM(&mut tvis_loading as *mut _ as isize),
        );
    }

    unsafe {
        SendMessageW(
            hwnd_tree,
            TVM_EXPAND,
            WPARAM(windows::Win32::UI::Controls::TVE_EXPAND.0 as usize),
            LPARAM(hitem.0),
        );
        SendMessageW(hwnd_tree, TVM_ENSUREVISIBLE, WPARAM(0), LPARAM(hitem.0));
    }

    let hwnd_copy = hwnd;
    let node_copy = node.clone();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                crate::log_debug(&format!("Failed to build tokio runtime: {}", e));
                return;
            }
        };
        let res = rt.block_on(crate::tools::rss::fetch_podcast_feed(
            &url,
            cache,
            rss_fetch_config(parent),
            false,
            language,
        ));
        let msg = Box::new(FetchResult {
            hitem: hitem.0,
            node: node_copy,
            result: res,
        });
        if let Err(_e) = crate::post_message_w_safe(
            hwnd_copy,
            WM_PODCAST_FETCH_COMPLETE,
            WPARAM(0),
            LPARAM(Box::into_raw(msg) as isize),
        ) {}
    });
}

fn update_source_cache(
    parent: HWND,
    source_index: usize,
    cache: rss::RssFeedCache,
    last_updated: Option<i64>,
) {
    if {
        with_state(parent, |ps| {
            if let Some(src) = ps.settings.podcast_sources.get_mut(source_index) {
                src.cache = cache;
                if let Some(ts) = last_updated {
                    src.last_updated = Some(ts);
                }
                settings::save_settings(ps.settings.clone());
            }
        })
    }
    .is_none()
    {
        crate::log_debug("Failed to update source cache state");
    }
}

fn update_source_tree_title(hwnd_tree: HWND, hitem: HTREEITEM, title: &str) {
    if hwnd_tree.0 == 0 || hitem.0 == 0 {
        return;
    }
    let title_wide = to_wide(title);
    let mut tvi = TVITEMW {
        mask: TVIF_TEXT,
        hItem: hitem,
        pszText: windows::core::PWSTR(title_wide.as_ptr() as *mut _),
        ..Default::default()
    };
    crate::send_message_w_safe(
        hwnd_tree,
        TVM_SETITEMW,
        WPARAM(0),
        LPARAM(&mut tvi as *mut _ as isize),
    );
}

/// Launch background check for all podcast feeds to detect new episodes
fn start_background_unheard_check(hwnd: HWND) {
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }

    let sources: Vec<(usize, String, rss::RssFeedCache)> = {
        with_state(parent, |ps| {
            ps.settings
                .podcast_sources
                .iter()
                .enumerate()
                .map(|(i, src)| (i, src.url.clone(), src.cache.clone()))
                .collect()
        })
    }
    .unwrap_or_default();

    if sources.is_empty() {
        return;
    }

    let fetch_config = rss_fetch_config(parent);
    let language = { with_state(parent, |ps| ps.settings.language) }.unwrap_or_default();
    ensure_rss_http(parent);

    let hwnd_raw = hwnd.0;
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                crate::log_debug(&format!("Failed to build tokio runtime: {}", e));
                return;
            }
        };

        rt.block_on(async {
            let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
            let mut handles = Vec::new();

            for (idx, url, cache) in sources {
                let sem = semaphore.clone();
                let cfg = fetch_config;
                let hwnd_val = hwnd_raw;

                let handle = tokio::spawn(async move {
                    let _permit = sem.acquire().await.ok()?;
                    let result = rss::fetch_podcast_feed(&url, cache, cfg, false, language).await;
                    if let Ok(outcome) = result {
                        let newest_key = outcome
                            .items
                            .first()
                            .map(episode_key)
                            .filter(|key| !key.trim().is_empty());
                        if newest_key.is_none() {
                            return Some(());
                        }
                        let msg = Box::new(BackgroundCheckResult {
                            source_idx: idx,
                            newest_item_key: newest_key,
                        });
                        if let Err(e) = crate::post_message_w_safe(
                            HWND(hwnd_val),
                            WM_PODCAST_BACKGROUND_CHECK_COMPLETE,
                            WPARAM(0),
                            LPARAM(Box::into_raw(msg) as isize),
                        ) {
                            crate::log_debug(&format!(
                                "Failed to post WM_PODCAST_BACKGROUND_CHECK_COMPLETE: {}",
                                e
                            ));
                        }
                    }
                    Some(())
                });
                handles.push(handle);
            }

            for h in handles {
                if let Err(e) = h.await {
                    crate::log_debug(&format!("Background check handle await failed: {}", e));
                }
            }
        });
    });
}

/// Process background check result - update unheard state if new episodes detected
fn process_background_check_result(hwnd: HWND, res: BackgroundCheckResult) {
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }

    let Some(newest_key) = res.newest_item_key else {
        return;
    };

    let should_mark_unheard = {
        with_state(parent, |ps| {
            ps.settings
                .podcast_sources
                .get(res.source_idx)
                .map(|src| match &src.last_seen_guid {
                    Some(last_seen) => last_seen != &newest_key,
                    None => true,
                })
                .unwrap_or(false)
        })
    }
    .unwrap_or(false);

    if should_mark_unheard {
        let hitem_opt = with_podcast_state(hwnd, |s| {
            for (&h, node) in &s.node_data {
                if let NodeData::Source(idx) = node
                    && *idx == res.source_idx
                {
                    return Some(HTREEITEM(h));
                }
            }
            None
        })
        .flatten();

        if let Some(hitem) = hitem_opt {
            set_source_unheard(hwnd, hitem, true);
        }
    }
}

fn update_source_title(hwnd: HWND, hitem: HTREEITEM, source_index: usize, feed_title: &str) {
    let title = feed_title.trim();
    if title.is_empty() {
        return;
    }
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }
    let language = { with_state(parent, |ps| ps.settings.language) }.unwrap_or_default();
    let mut updated = None;
    let did_access = {
        with_state(parent, |ps| {
            if let Some(src) = ps.settings.podcast_sources.get_mut(source_index) {
                let looks_auto = src.title.trim().is_empty() || src.title == src.url;
                if !src.user_title && looks_auto {
                    src.title = title.to_string();
                    let display = podcast_source_display_title(
                        src,
                        language,
                        ps.settings.announce_unread_rss_podcast_items,
                        ps.settings.rss_podcast_unread_label_position,
                    );
                    settings::save_settings(ps.settings.clone());
                    updated = Some(display);
                }
            }
        })
    }
    .is_some();
    if !did_access {
        crate::log_debug("Failed to update source title state");
        return;
    }
    let Some(updated) = updated else { return };
    let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    update_source_tree_title(hwnd_tree, hitem, &updated);
}

fn apply_episode_results(hwnd: HWND, hitem: HTREEITEM, items: Vec<PodcastEpisode>) -> usize {
    let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return 0;
    }

    let removed_keys = source_removed_episode_keys(hwnd, hitem);
    let existing_keys: HashSet<String> = with_podcast_state(hwnd, |s| {
        s.source_items
            .get(&hitem.0)
            .map(|state| state.items.iter().map(episode_key).collect())
            .unwrap_or_default()
    })
    .unwrap_or_default();

    let mut new_items = Vec::new();
    for item in items {
        let key = episode_key(&item);
        if removed_keys.contains(&key) {
            continue;
        }
        if !existing_keys.contains(&key) {
            new_items.push(item);
        }
    }

    let child = HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CHILD as usize),
            LPARAM(hitem.0),
        )
        .0,
    );
    if child.0 != 0 {
        let mut item = TVITEMW {
            mask: TVIF_TEXT,
            hItem: child,
            pszText: windows::core::PWSTR::null(),
            cchTextMax: 0,
            ..Default::default()
        };
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETITEMW,
            WPARAM(0),
            LPARAM(&mut item as *mut _ as isize),
        );
        let mut buf = vec![0u16; 128];
        item.pszText = windows::core::PWSTR(buf.as_mut_ptr());
        item.cchTextMax = buf.len() as i32;
        if crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETITEMW,
            WPARAM(0),
            LPARAM(&mut item as *mut _ as isize),
        )
        .0 != 0
        {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let text = String::from_utf16_lossy(&buf[..len]);
            if text.trim()
                == i18n::tr(
                    with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
                    "podcasts.loading",
                )
            {
                crate::send_message_w_safe(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(child.0));
            }
        }
    }

    let (language, announce_unread, unread_label_position, podcast_date_mode, podcast_time_mode) =
        with_podcast_state(hwnd, |s| {
            {
                with_state(s.parent, |ps| {
                    (
                        ps.settings.language,
                        ps.settings.announce_unread_rss_podcast_items,
                        ps.settings.rss_podcast_unread_label_position,
                        ps.settings.podcast_episodes_date_display,
                        ps.settings.podcast_episodes_time_display,
                    )
                })
            }
            .unwrap_or((
                s.language,
                true,
                crate::settings::RssPodcastUnreadLabelPosition::Before,
                ListDateDisplayMode::Always,
                ListTimeDisplayMode::OnlyIfMultipleSameDay,
            ))
        })
        .unwrap_or((
            Language::English,
            true,
            crate::settings::RssPodcastUnreadLabelPosition::Before,
            ListDateDisplayMode::Always,
            ListTimeDisplayMode::OnlyIfMultipleSameDay,
        ));

    let read_keys = with_podcast_state(hwnd, |s| {
        s.source_items
            .get(&hitem.0)
            .map(|state| state.read_item_keys.clone())
            .unwrap_or_else(|| source_read_episode_keys(hwnd, hitem))
    })
    .unwrap_or_default();

    let day_counts = build_day_counts(&new_items);
    let title_ctx = PodcastEpisodeTitleContext {
        language,
        announce_unread,
        unread_label_position,
        date_mode: podcast_date_mode,
        time_mode: podcast_time_mode,
    };
    for item in new_items.iter() {
        let item_unplayed = !read_keys.contains(&episode_key(item));
        let display_title = podcast_episode_display_title(
            &item.title,
            item_unplayed,
            item.pub_date,
            has_multiple_items_same_day(item.pub_date, &day_counts),
            title_ctx,
        );
        let title = to_wide(&display_title);
        let mut tvis = TVINSERTSTRUCTW {
            hParent: hitem,
            hInsertAfter: windows::Win32::UI::Controls::TVI_LAST,
            Anonymous: windows::Win32::UI::Controls::TVINSERTSTRUCTW_0 {
                item: TVITEMW {
                    mask: TVIF_TEXT,
                    pszText: windows::core::PWSTR(title.as_ptr() as *mut _),
                    cchTextMax: title.len() as i32,
                    ..Default::default()
                },
            },
        };
        let inserted = HTREEITEM(
            crate::send_message_w_safe(
                hwnd_tree,
                TVM_INSERTITEMW,
                WPARAM(0),
                LPARAM(&mut tvis as *mut _ as isize),
            )
            .0,
        );
        if inserted.0 != 0 {
            with_podcast_state(hwnd, |s| {
                s.node_data
                    .insert(inserted.0, NodeData::Episode(Box::new(item.clone())));
            });
        }
    }

    let added = new_items.len();
    with_podcast_state(hwnd, |s| {
        let persisted_read = source_read_episode_keys(hwnd, hitem);
        let state = s.source_items.entry(hitem.0).or_insert(SourceItemsState {
            items: Vec::new(),
            read_item_keys: persisted_read,
        });
        state.items.extend(new_items);
    });
    added
}

fn create_tree_item(hwnd_tree: HWND, title: &str, index: usize) -> HTREEITEM {
    let title_w = to_wide(title);
    let mut tvis = TVINSERTSTRUCTW {
        hParent: HTREEITEM(0),
        hInsertAfter: windows::Win32::UI::Controls::TVI_LAST,
        Anonymous: windows::Win32::UI::Controls::TVINSERTSTRUCTW_0 {
            item: TVITEMW {
                mask: TVIF_TEXT | TVIF_PARAM | TVIF_CHILDREN,
                pszText: windows::core::PWSTR(title_w.as_ptr() as *mut _),
                cchTextMax: title_w.len() as i32,
                cChildren: TVITEMEXW_CHILDREN(1),
                lParam: LPARAM(index as isize),
                ..Default::default()
            },
        },
    };
    HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_INSERTITEMW,
            WPARAM(0),
            LPARAM(&mut tvis as *mut _ as isize),
        )
        .0,
    )
}

fn reload_tree(hwnd: HWND) {
    let (hwnd_tree, sources, language, announce_unread, unread_label_position) =
        with_podcast_state(hwnd, |s| {
            let (sources, language, announce_unread, unread_label_position) = {
                with_state(s.parent, |ps| {
                    (
                        ps.settings.podcast_sources.clone(),
                        ps.settings.language,
                        ps.settings.announce_unread_rss_podcast_items,
                        ps.settings.rss_podcast_unread_label_position,
                    )
                })
            }
            .unwrap_or((
                Vec::new(),
                crate::settings::Language::English,
                true,
                crate::settings::RssPodcastUnreadLabelPosition::Before,
            ));
            (
                s.hwnd_tree,
                sources,
                language,
                announce_unread,
                unread_label_position,
            )
        })
        .unwrap_or((
            HWND(0),
            Vec::new(),
            crate::settings::Language::English,
            true,
            crate::settings::RssPodcastUnreadLabelPosition::Before,
        ));
    if hwnd_tree.0 == 0 {
        return;
    }
    crate::send_message_w_safe(
        hwnd_tree,
        TVM_DELETEITEM,
        WPARAM(0),
        LPARAM(windows::Win32::UI::Controls::TVI_ROOT.0),
    );
    with_podcast_state(hwnd, |s| {
        s.node_data.clear();
        s.source_items.clear();
    });

    for (i, src) in sources.iter().enumerate() {
        let title =
            podcast_source_display_title(src, language, announce_unread, unread_label_position);
        let hitem = create_tree_item(hwnd_tree, &title, i);
        if hitem.0 != 0 {
            with_podcast_state(hwnd, |s| {
                s.node_data.insert(hitem.0, NodeData::Source(i));
            });
        }
    }

    let first = HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_ROOT as usize),
            LPARAM(0),
        )
        .0,
    );
    if first.0 != 0 {
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_SELECTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(first.0),
        );
    }
}

fn mark_episode_played_with_delayed_ui(hwnd: HWND, parent: HWND, episode_key_value: String) {
    mark_episode_played_with_ui_delay(hwnd, parent, episode_key_value, 2000, 8);
}

fn mark_episode_played_with_ui_delay(
    hwnd: HWND,
    parent: HWND,
    episode_key_value: String,
    delay_ms: u64,
    retries_left: u8,
) {
    let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return;
    }
    let mut episode_hitem = HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    let selected_matches = with_podcast_state(hwnd, |s| match s.node_data.get(&episode_hitem.0) {
        Some(NodeData::Episode(item)) => episode_key(item) == episode_key_value,
        _ => false,
    })
    .unwrap_or(false);
    if !selected_matches {
        episode_hitem = with_podcast_state(hwnd, |s| {
            s.node_data.iter().find_map(|(h, node)| match node {
                NodeData::Episode(item) if episode_key(item) == episode_key_value => {
                    Some(HTREEITEM(*h))
                }
                _ => None,
            })
        })
        .flatten()
        .unwrap_or(HTREEITEM(0));
    }
    if episode_hitem.0 == 0 {
        return;
    }

    let source_hitem = HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_PARENT as usize),
            LPARAM(episode_hitem.0),
        )
        .0,
    );
    if source_hitem.0 != 0 {
        with_podcast_state(hwnd, |s| {
            if let Some(state) = s.source_items.get_mut(&source_hitem.0) {
                state.read_item_keys.insert(episode_key_value.clone());
            }
            if let Some(NodeData::Source(source_index)) = s.node_data.get(&source_hitem.0)
                && parent.0 != 0
            {
                {
                    with_state(parent, |ps| {
                        if let Some(src) = ps.settings.podcast_sources.get_mut(*source_index)
                            && !src.read_item_keys.iter().any(|k| k == &episode_key_value)
                        {
                            src.read_item_keys.push(episode_key_value.clone());
                            const MAX_PERSISTED_READ_KEYS: usize = 5000;
                            if src.read_item_keys.len() > MAX_PERSISTED_READ_KEYS {
                                let overflow = src.read_item_keys.len() - MAX_PERSISTED_READ_KEYS;
                                src.read_item_keys.drain(0..overflow);
                            }
                            settings::save_settings(ps.settings.clone());
                        }
                    });
                }
            }
        });
    }

    post_mark_episode_played_ui_after_delay(
        hwnd,
        MarkEpisodePlayedUiMessage {
            hitem: episode_hitem.0,
            item_key: episode_key_value,
            retries_left,
        },
        delay_ms,
    );
}

fn open_episode_in_player(hwnd: HWND, parent: HWND, episode: &PodcastEpisode) {
    let Some(url) = episode.enclosure_url.as_ref() else {
        let language = { with_state(parent, |s| s.settings.language) }.unwrap_or_default();
        announce_status(&i18n::tr(language, "podcasts.no_audio_url"));
        return;
    };
    let play_key = episode_key(episode);
    let should_start = with_podcast_state(hwnd, |s| {
        if s.pending_play.as_deref() == Some(play_key.as_str()) {
            return false;
        }
        s.pending_play = Some(play_key.clone());
        true
    })
    .unwrap_or(true);
    if !should_start {
        return;
    }
    mark_episode_played_with_delayed_ui(hwnd, parent, play_key.clone());
    let mut chapters_prefetch_started = false;
    if parent.0 != 0 {
        crate::set_pending_podcast_chapters_key(parent, Some(play_key.clone()));
        if !episode.podlove_chapters.is_empty() {
            crate::cache_podcast_chapters(
                parent,
                play_key.clone(),
                episode.podlove_chapters.clone(),
            );
        } else if let Some(chapters_url) = episode.chapters_url.clone() {
            let chapters_type = episode.chapters_type.clone();
            let should_fetch = match chapters_type
                .as_deref()
                .map(|t| t.trim().to_ascii_lowercase())
            {
                None => true,
                Some(kind) => kind == "application/json" || kind == "application/json+chapters",
            };
            if should_fetch {
                crate::log_debug(&format!(
                    "podcast_chapters_prefetch_feed key={} url={}",
                    play_key, chapters_url
                ));
                crate::prefetch_podcast_chapters(parent, play_key.clone(), chapters_url);
                chapters_prefetch_started = true;
            }
        }
        if !chapters_prefetch_started {
            if let Some(fallback_url) = crate::extract_embedded_chapters_url(url)
                .or_else(|| crate::extract_buzzsprout_chapters_url(url))
            {
                crate::log_debug(&format!(
                    "podcast_chapters_prefetch_fallback key={} url={}",
                    play_key, fallback_url
                ));
                crate::prefetch_podcast_chapters(parent, play_key.clone(), fallback_url);
            } else {
                crate::log_debug(&format!(
                    "podcast_chapters_prefetch_none key={} source_url={}",
                    play_key, url
                ));
            }
        }
    }

    // Show loading message immediately so user knows action was triggered
    let language = { with_state(parent, |s| s.settings.language) }.unwrap_or_default();
    announce_status(&i18n::tr(language, "podcasts.loading"));

    if parent.0 != 0 {
        ensure_rss_http(parent);
    }
    let url = url.clone();
    let episode_title = episode.title.clone();
    let podcast_title = selected_source_name(hwnd);
    let enclosure_type = episode.enclosure_type.clone();
    let cached_path = podcast_cache_path(&url, enclosure_type.as_deref());
    let cached_ok = cached_path
        .metadata()
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false);
    if cached_ok {
        let msg = Box::new(PlayReadyMsg {
            path: cached_path,
            enclosure_url: url.clone(),
            podcast_title: podcast_title.clone(),
            title: episode_title.clone(),
            item_key: play_key.clone(),
            chapters_prefetch_started,
        });
        if let Err(e) = crate::post_message_w_safe(
            hwnd,
            WM_PODCAST_PLAY_READY,
            WPARAM(0),
            LPARAM(Box::into_raw(msg) as isize),
        ) {
            log_debug(&format!(
                "Failed to post WM_PODCAST_PLAY_READY from cache: {}",
                e
            ));
        }
        return;
    }

    let hwnd_copy = hwnd;
    let cache_limit_mb =
        { with_state(parent, |s| s.settings.podcast_cache_limit_mb) }.unwrap_or(500);
    let cache_dir = podcast_cache_dir();
    let file_path = podcast_cache_path(&url, enclosure_type.as_deref());
    let partial_file_path = podcast_partial_cache_path(&file_path);
    if let Some(parent_dir) = file_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent_dir)
    {
        crate::log_debug(&format!(
            "Failed to create podcast directory {}: {}",
            parent_dir.display(),
            e
        ));
    }
    with_podcast_state(hwnd, |s| {
        s.download_in_progress = true;
        s.last_download_progress_pct = 0;
        s.last_download_progress_at = None;
    });

    std::thread::spawn(move || {
        log_debug(&format!("Podcast thread: starting download for {}", url));
        let mut last_reported_pct = 0u32;
        let mut attempt: u32 = 0;
        let downloaded_len: Result<u64, String> = loop {
            attempt += 1;
            let resume_from = std::fs::metadata(&partial_file_path)
                .map(|meta| meta.len())
                .unwrap_or(0);
            if attempt == 1 && resume_from == 0 {
                unsafe {
                    if windows::Win32::UI::WindowsAndMessaging::IsWindow(hwnd_copy).as_bool() {
                        PostMessageW(
                            hwnd_copy,
                            WM_PODCAST_DOWNLOAD_PROGRESS,
                            WPARAM(0),
                            LPARAM(0),
                        )
                        .ok();
                    }
                }
            }
            if resume_from > 0 {
                log_debug(&format!(
                    "Podcast thread: resuming partial download for {} from {} bytes",
                    url, resume_from
                ));
            }
            let result = rss::download_url_to_file_with_progress(
                &url,
                &partial_file_path,
                resume_from,
                |pct| {
                    // Avoid announcing "100%" before file write/finalization is truly done.
                    // Final completion is announced via "podcasts.download_completed".
                    let announced_pct = pct.min(90);
                    if announced_pct >= last_reported_pct + 10 {
                        last_reported_pct = (announced_pct / 10) * 10;
                        unsafe {
                            if windows::Win32::UI::WindowsAndMessaging::IsWindow(hwnd_copy)
                                .as_bool()
                            {
                                PostMessageW(
                                    hwnd_copy,
                                    WM_PODCAST_DOWNLOAD_PROGRESS,
                                    WPARAM(last_reported_pct as usize),
                                    LPARAM(0),
                                )
                                .ok();
                            }
                        }
                    }
                },
            );
            match result {
                Ok(len) => break Ok(len),
                Err(err) => {
                    crate::log_debug(&format!(
                        "podcasts_download_attempt_failed attempt={} url={} partial_bytes={} err={}",
                        attempt, url, resume_from, err
                    ));
                    if resume_from == 0 {
                        last_reported_pct = 0;
                        unsafe {
                            if windows::Win32::UI::WindowsAndMessaging::IsWindow(hwnd_copy)
                                .as_bool()
                            {
                                PostMessageW(
                                    hwnd_copy,
                                    WM_PODCAST_DOWNLOAD_PROGRESS,
                                    WPARAM(0),
                                    LPARAM(0),
                                )
                                .ok();
                            }
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(500u64 * attempt as u64));
                    continue;
                }
            }
        };
        let downloaded_len = match downloaded_len {
            Ok(len) => {
                log_debug(&format!("podcasts_download_ok len={} url={}", len, url));
                len
            }
            Err(err) => {
                log_debug(&format!("podcasts_download_error {}: {}", url, err));
                if let Err(remove_err) = std::fs::remove_file(&partial_file_path)
                    && remove_err.kind() != std::io::ErrorKind::NotFound
                {
                    log_debug(&format!(
                        "Failed to remove partial podcast file {}: {}",
                        partial_file_path.display(),
                        remove_err
                    ));
                }
                unsafe {
                    if windows::Win32::UI::WindowsAndMessaging::IsWindow(hwnd_copy).as_bool()
                        && let Err(e) =
                            PostMessageW(hwnd_copy, WM_PODCAST_PLAY_FAILED, WPARAM(0), LPARAM(0))
                    {
                        log_debug(&format!("Failed to post WM_PODCAST_PLAY_FAILED: {}", e));
                    }
                }
                return;
            }
        };
        log_debug(&format!("podcasts_cache_path: {}", file_path.display()));
        match std::fs::rename(&partial_file_path, &file_path) {
            Ok(_) => {
                log_debug(&format!(
                    "podcasts_write_ok: {} len={}",
                    file_path.display(),
                    downloaded_len
                ));
                let limit_bytes = cache_limit_mb as u64 * 1024 * 1024;
                enforce_podcast_cache_limit(&cache_dir, limit_bytes, Some(&file_path));
                let msg = Box::new(PlayReadyMsg {
                    path: file_path,
                    enclosure_url: url.clone(),
                    podcast_title: podcast_title.clone(),
                    title: episode_title.clone(),
                    item_key: play_key.clone(),
                    chapters_prefetch_started,
                });
                unsafe {
                    if windows::Win32::UI::WindowsAndMessaging::IsWindow(hwnd_copy).as_bool() {
                        log_debug(&format!(
                            "Posting WM_PODCAST_PLAY_READY to HWND({:?})",
                            hwnd_copy.0
                        ));
                        if let Err(e) = PostMessageW(
                            hwnd_copy,
                            WM_PODCAST_PLAY_READY,
                            WPARAM(0),
                            LPARAM(Box::into_raw(msg) as isize),
                        ) {
                            log_debug(&format!("Failed to post WM_PODCAST_PLAY_READY: {}", e));
                        }
                    }
                }
            }
            Err(err) => {
                log_debug(&format!(
                    "Failed to finalize podcast cache file {} from {}: {}",
                    file_path.display(),
                    partial_file_path.display(),
                    err
                ));
                if let Err(remove_err) = std::fs::remove_file(&partial_file_path)
                    && remove_err.kind() != std::io::ErrorKind::NotFound
                {
                    log_debug(&format!(
                        "Failed to remove partial podcast file after finalize error {}: {}",
                        partial_file_path.display(),
                        remove_err
                    ));
                }
                unsafe {
                    if windows::Win32::UI::WindowsAndMessaging::IsWindow(hwnd_copy).as_bool()
                        && let Err(e) =
                            PostMessageW(hwnd_copy, WM_PODCAST_PLAY_FAILED, WPARAM(0), LPARAM(0))
                    {
                        log_debug(&format!(
                            "Failed to post WM_PODCAST_PLAY_FAILED after write error: {}",
                            e
                        ));
                    }
                }
            }
        }
    });

    // Keep accessibility feedback alive during slow/idle network phases.
    let heartbeat_hwnd = hwnd;
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(4));
            let keep_running = unsafe {
                if !windows::Win32::UI::WindowsAndMessaging::IsWindow(heartbeat_hwnd).as_bool() {
                    false
                } else {
                    with_podcast_state(heartbeat_hwnd, |s| s.download_in_progress).unwrap_or(false)
                }
            };
            if !keep_running {
                break;
            }
            unsafe {
                if let Err(err) = PostMessageW(
                    heartbeat_hwnd,
                    WM_PODCAST_DOWNLOAD_HEARTBEAT,
                    WPARAM(0),
                    LPARAM(0),
                ) {
                    log_debug(&format!(
                        "Failed to post WM_PODCAST_DOWNLOAD_HEARTBEAT: {}",
                        err
                    ));
                }
            }
        }
    });
}

fn podcast_cache_path(url: &str, mime: Option<&str>) -> PathBuf {
    let mut hasher = sha2::Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hex::encode(hasher.finalize());

    let mut ext = match mime.map(|m| m.to_ascii_lowercase()) {
        Some(m) if m.contains("mpeg") || m.contains("mp3") => "mp3",
        Some(m) if m.contains("mp4") || m.contains("m4a") || m.contains("aac") => "m4a",
        Some(m) if m.contains("ogg") || m.contains("vorbis") => "ogg",
        Some(m) if m.contains("opus") => "opus",
        Some(m) if m.contains("wav") => "wav",
        Some(m) if m.contains("flac") => "flac",
        _ => "",
    };

    let url_ext_owned;
    if ext.is_empty() {
        let url_ext = url
            .split('?')
            .next()
            .unwrap_or(url)
            .split('/')
            .next_back()
            .unwrap_or("")
            .split('.')
            .next_back()
            .unwrap_or("mp3")
            .to_ascii_lowercase();

        if url_ext == "mp4" {
            ext = "m4a";
        } else {
            url_ext_owned = url_ext;
            ext = &url_ext_owned;
        }
    }

    // Limit extension length to avoid issues with weird URLs
    if ext.len() > 5 || ext.is_empty() {
        ext = "mp3";
    }

    let filename = format!("podcast_{}.{}", &hash[..16], ext);
    podcast_cache_dir().join(filename)
}

fn podcast_cache_dir() -> PathBuf {
    settings::settings_dir().join("podcast cache")
}

fn podcast_partial_cache_path(file_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.part", file_path.to_string_lossy()))
}

fn cache_embedded_podcast_chapters_if_missing(
    parent: HWND,
    key: &str,
    media_path: &Path,
    skip_when_prefetch_started: bool,
) {
    if skip_when_prefetch_started {
        return;
    }

    let has_cached = with_state(parent, |state| {
        state
            .podcast_chapters_cache
            .get(key)
            .and_then(|v| v.as_ref())
            .is_some_and(|items| !items.is_empty())
    })
    .unwrap_or(false);
    if has_cached {
        return;
    }

    crate::prefetch_podcast_chapters_from_file(parent, key.to_string(), media_path.to_path_buf());
}

fn podcast_cache_marker_path(path: &Path) -> PathBuf {
    let mut marker = path.to_path_buf();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        marker.set_extension(format!("{}.played", ext));
    } else {
        marker.set_extension("played");
    }
    marker
}

fn system_time_secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn enforce_podcast_cache_limit(
    cache_dir: &Path,
    limit_bytes: u64,
    protected: Option<&Path>,
) {
    if limit_bytes == 0 {
        return;
    }
    let entries = match std::fs::read_dir(cache_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    let mut markers: HashMap<PathBuf, u64> = HashMap::new();
    let mut files: Vec<(PathBuf, u64, u64)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if !metadata.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("played") {
            let base = path.with_extension("");
            let modified = metadata.modified().map(system_time_secs).unwrap_or(0);
            markers.insert(base, modified);
            continue;
        }
        let modified = metadata.modified().map(system_time_secs).unwrap_or(0);
        files.push((path, metadata.len(), modified));
    }

    let mut total: u64 = files.iter().map(|(_, size, _)| *size).sum();
    if total <= limit_bytes {
        return;
    }

    let protected = protected.map(|path| path.to_path_buf());
    let mut played_entries: Vec<(PathBuf, u64, u64)> = Vec::new();
    let mut unplayed_entries: Vec<(PathBuf, u64, u64)> = Vec::new();
    for (path, size, modified) in files {
        if let Some(marked) = markers.get(&path).copied() {
            played_entries.push((path, size, marked));
        } else {
            unplayed_entries.push((path, size, modified));
        }
    }
    played_entries.sort_by_key(|entry| entry.2);
    unplayed_entries.sort_by_key(|entry| entry.2);

    for (path, size, _) in played_entries {
        if total <= limit_bytes {
            break;
        }
        remove_cache_entry(&path, size, &protected, &mut total);
    }
    for (path, size, _) in unplayed_entries {
        if total <= limit_bytes {
            break;
        }
        remove_cache_entry(&path, size, &protected, &mut total);
    }
}

fn remove_cache_entry(path: &Path, size: u64, protected: &Option<PathBuf>, total: &mut u64) {
    if protected.as_ref().map(|p| p == path).unwrap_or(false) {
        return;
    }
    if std::fs::remove_file(path).is_ok() {
        let marker = podcast_cache_marker_path(path);
        if let Err(e) = std::fs::remove_file(marker) {
            crate::log_debug(&format!("Failed to remove marker file: {}", e));
        }
        *total = total.saturating_sub(size);
    } else {
        log_debug(&format!(
            "podcast_cache_delete_failed {}",
            path.to_string_lossy()
        ));
    }
}

fn mark_podcast_episode_played(path: &Path) {
    let marker = podcast_cache_marker_path(path);
    if let Err(e) = std::fs::write(marker, b"") {
        crate::log_debug(&format!("Failed to write marker file: {}", e));
    }
}

fn set_source_unheard(hwnd: HWND, hitem: HTREEITEM, unheard: bool) {
    let first_item_key: Option<String> = if !unheard {
        with_podcast_state(hwnd, |s| {
            s.source_items
                .get(&hitem.0)
                .and_then(|state| state.items.first().map(episode_key))
        })
        .flatten()
    } else {
        None
    };

    let (hwnd_tree, title_opt) = with_podcast_state(hwnd, |s| {
        let hwnd_tree = s.hwnd_tree;
        let parent = s.parent;
        let source_idx = s.node_data.get(&hitem.0).and_then(|node| match node {
            NodeData::Source(idx) => Some(*idx),
            _ => None,
        });
        let Some(idx) = source_idx else {
            return (hwnd_tree, None);
        };
        let language = { with_state(parent, |ps| ps.settings.language) }.unwrap_or_default();
        let title_opt = {
            with_state(parent, |ps| {
                if let Some(src) = ps.settings.podcast_sources.get_mut(idx) {
                    let mut changed = false;
                    if src.unread != unheard {
                        src.unread = unheard;
                        changed = true;
                    }
                    if let Some(ref key) = first_item_key
                        && src.last_seen_guid.as_ref() != Some(key)
                    {
                        src.last_seen_guid = Some(key.clone());
                        changed = true;
                    }
                    if changed {
                        let title = podcast_source_display_title(
                            src,
                            language,
                            ps.settings.announce_unread_rss_podcast_items,
                            ps.settings.rss_podcast_unread_label_position,
                        );
                        settings::save_settings(ps.settings.clone());
                        return Some(title);
                    }
                }
                None
            })
        }
        .flatten();
        (hwnd_tree, title_opt)
    })
    .unwrap_or((HWND(0), None));
    if let Some(title) = title_opt {
        update_source_tree_title(hwnd_tree, hitem, &title);
    }
}

pub(crate) fn add_podcast_source(parent: HWND, feed_url: &str, title: &str) -> Option<usize> {
    let normalized = rss::normalize_url(feed_url);
    if normalized.is_empty() {
        return None;
    }
    {
        with_state(parent, |ps| {
            if ps
                .settings
                .podcast_sources
                .iter()
                .any(|src| rss::normalize_url(&src.url) == normalized)
            {
                return None;
            }
            let final_title = if title.trim().is_empty() {
                normalized.clone()
            } else {
                title.trim().to_string()
            };
            ps.settings.podcast_sources.push(RssSource {
                title: final_title,
                url: normalized,
                kind: RssSourceType::Feed,
                folder_path: Vec::new(),
                user_title: !title.trim().is_empty(),
                unread: true,
                cache: rss::RssFeedCache::default(),
                last_seen_guid: None,
                last_updated: None,
                removed_item_keys: Vec::new(),
                read_item_keys: Vec::new(),
            });
            settings::save_settings(ps.settings.clone());
            Some(ps.settings.podcast_sources.len() - 1)
        })
    }
    .flatten()
}

pub(crate) fn show_subscription_confirmation(hwnd: HWND, language: Language) {
    let title = i18n::tr(language, "podcasts.subscribed_title");
    let message = i18n::tr(language, "podcasts.subscribed_message");
    unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(to_wide(&message).as_ptr()),
            PCWSTR(to_wide(&title).as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

fn render_search_results_list(
    hwnd: HWND,
    visible_results: &[PodcastSearchResult],
    show_previous: bool,
    trailing_action: Option<SearchResultsNavAction>,
    announce_page: Option<String>,
) {
    let hwnd_results = with_podcast_state(hwnd, |s| s.hwnd_results).unwrap_or(HWND(0));
    if hwnd_results.0 == 0 {
        return;
    }
    crate::send_message_w_safe(hwnd_results, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    if visible_results.is_empty() {
        let text = to_wide(&i18n::tr(
            with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
            "podcasts.search.no_results",
        ));
        crate::send_message_w_safe(
            hwnd_results,
            LB_ADDSTRING,
            WPARAM(0),
            LPARAM(text.as_ptr() as isize),
        );
        with_podcast_state(hwnd, |s| s.search_results = Vec::new());
        unsafe {
            SendMessageW(hwnd_results, LB_SETCURSEL, WPARAM(0), LPARAM(0));
            SetFocus(hwnd_results);
        }
        return;
    }
    if show_previous {
        let previous = to_wide(&i18n::tr(
            with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
            "podcasts.categories.previous_results",
        ));
        crate::send_message_w_safe(
            hwnd_results,
            LB_ADDSTRING,
            WPARAM(0),
            LPARAM(previous.as_ptr() as isize),
        );
    }
    for item in visible_results {
        let label = if item.artist.trim().is_empty() {
            item.title.clone()
        } else {
            format!("{} - {}", item.title, item.artist)
        };
        let wide = to_wide(&label);
        crate::send_message_w_safe(
            hwnd_results,
            LB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    if let Some(action) = trailing_action {
        let key = match action {
            SearchResultsNavAction::NextPage => "podcasts.categories.next_results",
            SearchResultsNavAction::LoadMore => "podcasts.categories.load_more_results",
            SearchResultsNavAction::PreviousPage => "podcasts.categories.previous_results",
        };
        let trailing = to_wide(&i18n::tr(
            with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
            key,
        ));
        crate::send_message_w_safe(
            hwnd_results,
            LB_ADDSTRING,
            WPARAM(0),
            LPARAM(trailing.as_ptr() as isize),
        );
    }
    with_podcast_state(hwnd, |s| s.search_results = visible_results.to_vec());
    if let Some(page_label) = announce_page
        && !page_label.trim().is_empty()
    {
        announce_status(&page_label);
    }
    unsafe {
        SendMessageW(hwnd_results, LB_SETCURSEL, WPARAM(0), LPARAM(0));
        SetFocus(hwnd_results);
    }
}

fn update_search_results(
    hwnd: HWND,
    results: Vec<PodcastSearchResult>,
    status: Option<&str>,
    category_meta: Option<CategoryResultsMeta>,
) {
    if let Some(status) = status
        && !status.trim().is_empty()
    {
        announce_status(status);
    }
    if let Some(meta) = category_meta {
        let page_label = if results.len() > CATEGORY_RESULTS_PAGE_SIZE {
            let total_pages = results.len().div_ceil(CATEGORY_RESULTS_PAGE_SIZE);
            let current_page = (meta.visible_start / CATEGORY_RESULTS_PAGE_SIZE) + 1;
            Some(i18n::tr_f(
                with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
                "podcasts.categories.results_page",
                &[
                    ("current", &current_page.to_string()),
                    ("total", &total_pages.to_string()),
                ],
            ))
        } else {
            None
        };
        let show_previous = meta.visible_start > 0;
        let trailing_action = if meta.visible_start + CATEGORY_RESULTS_PAGE_SIZE < results.len() {
            Some(SearchResultsNavAction::NextPage)
        } else if meta.can_load_more {
            Some(SearchResultsNavAction::LoadMore)
        } else {
            None
        };
        let start = meta.visible_start.min(results.len());
        let end = (start + CATEGORY_RESULTS_PAGE_SIZE).min(results.len());
        let visible_results = results[start..end].to_vec();
        with_podcast_state(hwnd, |s| {
            s.category_results = Some(CategoryResultsState {
                source: meta.source,
                mode: meta.mode,
                category: meta.category,
                search_term: meta.search_term,
                loaded_results: results,
                visible_start: start,
                can_load_more: meta.can_load_more,
            });
        });
        render_search_results_list(
            hwnd,
            &visible_results,
            show_previous,
            trailing_action,
            page_label,
        );
        return;
    }
    with_podcast_state(hwnd, |s| s.category_results = None);
    render_search_results_list(hwnd, &results, false, None, None);
}

fn show_search_loading(hwnd: HWND) {
    let hwnd_results = with_podcast_state(hwnd, |s| s.hwnd_results).unwrap_or(HWND(0));
    if hwnd_results.0 == 0 {
        return;
    }
    crate::send_message_w_safe(hwnd_results, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    with_podcast_state(hwnd, |s| {
        s.search_results.clear();
        s.category_results = None;
    });
    let text = to_wide(&i18n::tr(
        with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
        "podcasts.loading",
    ));
    unsafe {
        SendMessageW(
            hwnd_results,
            LB_ADDSTRING,
            WPARAM(0),
            LPARAM(text.as_ptr() as isize),
        );
        SendMessageW(hwnd_results, LB_SETCURSEL, WPARAM(0), LPARAM(0));
        SetFocus(hwnd_results);
    }
}

fn selected_search_provider(hwnd: HWND) -> SearchProvider {
    let combo = with_podcast_state(hwnd, |s| s.hwnd_search_provider).unwrap_or(HWND(0));
    if combo.0 == 0 {
        return SearchProvider::Itunes;
    }
    let sel = crate::send_message_w_safe(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel == 1 {
        SearchProvider::PodcastIndex
    } else {
        SearchProvider::Itunes
    }
}

fn podcastindex_credentials_available(parent: HWND) -> bool {
    with_state(parent, |ps| {
        let key = ps.settings.podcast_index_api_key.trim();
        let secret = settings::decrypt_podcast_index_secret(&ps.settings.podcast_index_api_secret)
            .unwrap_or_default();
        !key.is_empty() && !secret.trim().is_empty()
    })
    .unwrap_or(false)
}

fn persisted_search_provider(parent: HWND) -> SearchProvider {
    with_state(parent, |ps| match ps.settings.podcast_search_provider {
        PodcastSearchProvider::PodcastIndex => SearchProvider::PodcastIndex,
        PodcastSearchProvider::Itunes => SearchProvider::Itunes,
    })
    .unwrap_or(SearchProvider::Itunes)
}

fn save_search_provider(parent: HWND, provider: SearchProvider) {
    let mut changed = false;
    with_state(parent, |ps| {
        let next = match provider {
            SearchProvider::Itunes => PodcastSearchProvider::Itunes,
            SearchProvider::PodcastIndex => PodcastSearchProvider::PodcastIndex,
        };
        if ps.settings.podcast_search_provider != next {
            ps.settings.podcast_search_provider = next;
            changed = true;
        }
    });
    if changed && let Some(settings) = with_state(parent, |ps| ps.settings.clone()) {
        settings::save_settings(settings);
    }
}

fn perform_search(hwnd: HWND, query: &str) {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return;
    }
    show_search_loading(hwnd);
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 != 0 {
        ensure_rss_http(parent);
    }
    let provider = selected_search_provider(hwnd);
    let podcastindex_auth = if matches!(provider, SearchProvider::PodcastIndex) {
        podcastindex_credentials_or_prompt(hwnd, parent)
    } else {
        None
    };
    if matches!(provider, SearchProvider::PodcastIndex) && podcastindex_auth.is_none() {
        return;
    }
    let exact_key = podcast_title_key(trimmed);
    let query = percent_encode(trimmed);
    let hwnd_copy = hwnd;
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                crate::log_debug(&format!("Failed to build tokio runtime: {}", e));
                return;
            }
        };
        let mut results = Vec::new();
        match provider {
            SearchProvider::Itunes => {
                let fetch_config = rss_fetch_config(parent);
                let country = apple_country_for_parent(parent);
                let url = format!(
                    "https://itunes.apple.com/search?media=podcast&entity=podcast&term={}&country={}&limit=20",
                    query, country
                );
                match rt.block_on(rss::fetch_url_bytes(&url, fetch_config)) {
                    Ok(bytes) => {
                        if let Ok(parsed) = serde_json::from_slice::<ItunesSearchResponse>(&bytes) {
                            results.extend(itunes_items_to_results(parsed.results));
                        }
                    }
                    Err(err) => {
                        log_debug(&format!("itunes_search_failed: {}", err));
                    }
                }

                let spreaker_url = format!(
                    "https://api.spreaker.com/v2/search?q={}&type=shows&limit=20",
                    query
                );
                match rt.block_on(rss::fetch_url_bytes(&spreaker_url, fetch_config)) {
                    Ok(bytes) => {
                        if let Ok(parsed) = serde_json::from_slice::<SpreakerSearchResponse>(&bytes)
                        {
                            results.extend(spreaker_items_to_results(parsed.response.items));
                        }
                    }
                    Err(err) => {
                        log_debug(&format!("spreaker_search_failed: {}", err));
                    }
                }

                if !exact_key.is_empty() {
                    let url = format!(
                        "https://api.spreaker.com/v2/shows/{}",
                        percent_encode(&exact_key)
                    );
                    match rt.block_on(rss::fetch_url_bytes(&url, fetch_config)) {
                        Ok(bytes) => match parse_spreaker_exact(&bytes, &exact_key) {
                            Ok(mut exact) => {
                                log_debug(&format!(
                                    "spreaker_exact_search: {} results",
                                    exact.len()
                                ));
                                exact.extend(results);
                                results = exact;
                            }
                            Err(err) => {
                                log_debug(&format!("spreaker_exact_search_unavailable: {err}"))
                            }
                        },
                        Err(err) => log_debug(&format!("spreaker_exact_search_unavailable: {err}")),
                    }
                }
                dedup_podcast_search_results(&mut results);
                results.sort_by_key(|item| podcast_title_key(&item.title) != exact_key);
            }
            SearchProvider::PodcastIndex => {
                let Some((key, secret)) = podcastindex_auth else {
                    log_debug("PodcastIndex search skipped: missing API keys");
                    let msg = Box::new(SearchResultMsg {
                        results,
                        status: None,
                        error: None,
                        category_results: None,
                    });
                    if let Err(e) = crate::post_message_w_safe(
                        hwnd_copy,
                        WM_PODCAST_SEARCH_COMPLETE,
                        WPARAM(0),
                        LPARAM(Box::into_raw(msg) as isize),
                    ) {
                        crate::log_debug(&format!("PostMessageW failed: {:?}", e));
                    }
                    return;
                };
                let url = format!(
                    "https://api.podcastindex.org/api/1.0/search/byterm?q={}&max=20",
                    query
                );
                let result = rt.block_on(async {
                    let client = reqwest::Client::new();
                    let resp = add_podcastindex_auth_headers(client.get(url), &key, &secret)
                        .send()
                        .await
                        .map_err(|e| e.to_string())?;
                    if !resp.status().is_success() {
                        return Err(format!("HTTP {}", resp.status().as_u16()));
                    }
                    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
                    let parsed: PodcastIndexSearchResponse =
                        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                    Ok(parsed.feeds.unwrap_or_default())
                });
                match result {
                    Ok(feeds) => {
                        for feed in feeds {
                            let feed_url = feed.feed_url.or(feed.url);
                            if let Some(feed_url) = feed_url {
                                results.push(PodcastSearchResult {
                                    title: feed.title.unwrap_or_default(),
                                    artist: feed.author.or(feed.owner_name).unwrap_or_default(),
                                    feed_url,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        log_debug(&format!("podcastindex_search_failed: {}", e));
                    }
                }
            }
        }
        let msg = Box::new(SearchResultMsg {
            results,
            status: None,
            error: None,
            category_results: None,
        });
        if let Err(e) = crate::post_message_w_safe(
            hwnd_copy,
            WM_PODCAST_SEARCH_COMPLETE,
            WPARAM(0),
            LPARAM(Box::into_raw(msg) as isize),
        ) {
            crate::log_debug(&format!("PostMessageW failed: {:?}", e));
        }
    });
}

fn parse_apple_top_ids(bytes: &[u8]) -> Vec<u64> {
    let mut ids = Vec::new();
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return ids;
    };
    let entries = value
        .get("feed")
        .and_then(|feed| feed.get("entry"))
        .cloned();
    let Some(entries) = entries else {
        return ids;
    };
    let list = match entries {
        serde_json::Value::Array(items) => items,
        other => vec![other],
    };
    for entry in list {
        if let Some(id_val) = entry
            .get("id")
            .and_then(|id| id.get("attributes"))
            .and_then(|attrs| attrs.get("im:id"))
            .and_then(|v| v.as_str())
            && let Ok(id) = id_val.parse::<u64>()
        {
            ids.push(id);
        }
    }
    ids
}

fn itunes_items_to_results(items: Vec<ItunesSearchItem>) -> Vec<PodcastSearchResult> {
    let mut results = Vec::new();
    for item in items {
        if let Some(feed_url) = item.feed_url {
            results.push(PodcastSearchResult {
                title: item.collection_name.unwrap_or_default(),
                artist: item.artist_name.unwrap_or_default(),
                feed_url,
            });
        }
    }
    results
}

fn podcast_title_key(title: &str) -> String {
    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn parse_spreaker_exact(
    bytes: &[u8],
    key: &str,
) -> Result<Vec<PodcastSearchResult>, serde_json::Error> {
    let response: SpreakerShowResponse = serde_json::from_slice(bytes)?;
    let item = response.response.show;
    if podcast_title_key(&item.title) != key {
        return Ok(Vec::new());
    }
    Ok(spreaker_items_to_results(vec![item]))
}

fn spreaker_items_to_results(items: Vec<SpreakerShowItem>) -> Vec<PodcastSearchResult> {
    let mut results = Vec::new();
    for item in items {
        if item.title.trim().is_empty() {
            continue;
        }
        results.push(PodcastSearchResult {
            title: item.title,
            artist: item
                .author_name
                .filter(|name| !name.trim().is_empty())
                .or_else(|| item.author.and_then(|author| author.fullname))
                .unwrap_or_default(),
            feed_url: format!(
                "https://www.spreaker.com/show/{}/episodes/feed",
                item.show_id
            ),
        });
    }
    results
}

fn dedup_podcast_search_results(results: &mut Vec<PodcastSearchResult>) {
    let mut seen_feed_urls = HashSet::new();
    results.retain(|result| {
        let feed_key = normalize_podcast_key(&result.feed_url);
        feed_key.is_empty() || seen_feed_urls.insert(feed_key)
    });
}

#[cfg(debug_assertions)]
fn log_itunes_items(context: &str, genre_id: u32, items: &[ItunesSearchItem]) {
    let mut lines = Vec::new();
    for item in items.iter().take(50) {
        let title = item.collection_name.clone().unwrap_or_default();
        let artist = item.artist_name.clone().unwrap_or_default();
        let primary = item
            .primary_genre_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "-".to_string());
        let genre_ids = item
            .genre_ids
            .as_ref()
            .map(|ids| ids.join(","))
            .unwrap_or_else(|| "-".to_string());
        let matches = itunes_item_matches_genre(item, genre_id);
        lines.push(format!(
            "title=\"{}\" artist=\"{}\" primary={} genreIds={} match={}",
            title, artist, primary, genre_ids, matches
        ));
    }
    log_debug(&format!(
        "itunes_category_debug context={} genre_id={} count={} items=[{}]",
        context,
        genre_id,
        items.len(),
        lines.join(" | ")
    ));
}

#[cfg(not(debug_assertions))]
fn log_itunes_items(_context: &str, _genre_id: u32, _items: &[ItunesSearchItem]) {}

fn itunes_items_to_results_slice(items: &[ItunesSearchItem]) -> Vec<PodcastSearchResult> {
    let mut results = Vec::new();
    for item in items {
        if let Some(feed_url) = item.feed_url.as_ref() {
            results.push(PodcastSearchResult {
                title: item.collection_name.clone().unwrap_or_default(),
                artist: item.artist_name.clone().unwrap_or_default(),
                feed_url: feed_url.clone(),
            });
        }
    }
    results
}

fn itunes_item_matches_genre(item: &ItunesSearchItem, genre_id: u32) -> bool {
    if let Some(ids) = item.genre_ids.as_ref() {
        return ids.iter().any(|id| id == &genre_id.to_string());
    }
    if let Some(primary) = item.primary_genre_id {
        return primary == genre_id;
    }
    false
}

fn itunes_filter_by_genre(
    items: &[ItunesSearchItem],
    genre_id: u32,
    language: Language,
) -> (Vec<PodcastSearchResult>, Option<String>) {
    let mut filtered = Vec::new();
    for item in items {
        if itunes_item_matches_genre(item, genre_id)
            && let Some(feed_url) = item.feed_url.as_ref()
        {
            filtered.push(PodcastSearchResult {
                title: item.collection_name.clone().unwrap_or_default(),
                artist: item.artist_name.clone().unwrap_or_default(),
                feed_url: feed_url.clone(),
            });
        }
    }
    if !filtered.is_empty() {
        return (filtered, None);
    }
    let status = i18n::tr(language, "podcasts.categories.apple_unfiltered_notice");
    (Vec::new(), Some(status))
}

fn normalize_category_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn podcastindex_feeds_to_results(
    feeds: Vec<PodcastIndexFeed>,
    category: Option<&Category>,
    language: Language,
) -> (Vec<PodcastSearchResult>, bool) {
    let mut results = Vec::new();
    let (filter_key, filter_name) = category.map_or((None, None), |c| {
        (
            Some(c.id.to_string()),
            Some(normalize_category_name(&c.name)),
        )
    });
    let has_categories = filter_key.is_some() && feeds.iter().any(|f| f.categories.is_some());
    let lang_filter = podcastindex_language_code(language);
    let has_languages = !lang_filter.is_empty()
        && feeds
            .iter()
            .any(|f| f.language.as_deref().unwrap_or_default().trim().len() >= 2);
    for feed in feeds {
        if !lang_filter.is_empty() {
            let feed_lang = feed.language.as_deref().unwrap_or_default().trim();
            let matches_lang = feed_lang.eq_ignore_ascii_case(lang_filter)
                || feed_lang
                    .split(&['-', '_'][..])
                    .next()
                    .map(|short| short.eq_ignore_ascii_case(lang_filter))
                    .unwrap_or(false);
            if has_languages {
                if !matches_lang {
                    continue;
                }
            } else if !feed_lang.is_empty() && !matches_lang {
                continue;
            }
        }
        if let Some(filter_key) = filter_key.as_deref() {
            match feed.categories.as_ref() {
                Some(categories) => {
                    let mut matched = categories.contains_key(filter_key);
                    if !matched && let Some(filter_name) = filter_name.as_deref() {
                        matched = categories
                            .values()
                            .any(|name| normalize_category_name(name) == filter_name);
                    }
                    if !matched {
                        continue;
                    }
                }
                None => {
                    if has_categories {
                        continue;
                    }
                }
            }
        }
        let feed_url = feed.feed_url.or(feed.url);
        if let Some(feed_url) = feed_url {
            results.push(PodcastSearchResult {
                title: feed.title.unwrap_or_default(),
                artist: feed.author.or(feed.owner_name).unwrap_or_default(),
                feed_url,
            });
        }
    }
    (results, has_categories)
}

async fn load_by_category(
    source: Source,
    mode: Mode,
    category: Category,
    search_term: &str,
    env: &CategoryLoadEnv,
) -> Result<(Vec<PodcastSearchResult>, Option<String>), String> {
    match source {
        Source::Apple => {
            log_debug(&format!(
                "podcasts_categories apple load start: mode={:?} category_id={} category_name={} term={}",
                mode,
                category.id,
                category.name,
                search_term.trim()
            ));
            let mut status = None;
            let results = match mode {
                Mode::Top => {
                    let url = apple_top_podcasts_by_genre(
                        category.id,
                        &env.apple_country,
                        env.apple_limit,
                    );
                    log_debug(&format!(
                        "podcasts_categories apple top fetch ids start: category_id={} country={} url={}",
                        category.id, env.apple_country, url
                    ));
                    let ids = match rss::fetch_url_bytes(&url, env.fetch_config).await {
                        Ok(bytes) => parse_apple_top_ids(&bytes),
                        Err(err) => {
                            log_debug(&format!("apple_top_fetch_failed: {}", err));
                            Vec::new()
                        }
                    };
                    log_debug(&format!(
                        "podcasts_categories apple top fetch ids done: category_id={} id_count={}",
                        category.id,
                        ids.len()
                    ));
                    let lookup_url = apple_lookup_by_ids(&ids, &env.apple_country);
                    if let Some(lookup_url) = lookup_url
                        && {
                            log_debug(&format!(
                                "podcasts_categories apple top lookup start: category_id={} url={}",
                                category.id, lookup_url
                            ));
                            true
                        }
                        && let Ok(bytes) = rss::fetch_url_bytes(&lookup_url, env.fetch_config).await
                        && let Ok(parsed) = serde_json::from_slice::<ItunesSearchResponse>(&bytes)
                    {
                        log_debug(&format!(
                            "podcasts_categories apple top lookup done: category_id={} result_count={}",
                            category.id,
                            parsed.results.len()
                        ));
                        log_itunes_items("apple_top_lookup", category.id, &parsed.results);
                        let mut map = HashMap::new();
                        for item in parsed.results {
                            if let (Some(id), Some(feed_url)) =
                                (item.collection_id, item.feed_url.clone())
                                && itunes_item_matches_genre(&item, category.id)
                            {
                                map.insert(
                                    id,
                                    PodcastSearchResult {
                                        title: item.collection_name.unwrap_or_default(),
                                        artist: item.artist_name.unwrap_or_default(),
                                        feed_url,
                                    },
                                );
                            }
                        }
                        let mut ordered = Vec::new();
                        for id in ids {
                            if let Some(item) = map.get(&id) {
                                ordered.push(item.clone());
                            }
                        }
                        if !ordered.is_empty() {
                            return Ok((ordered, None));
                        }
                    }
                    status = Some(i18n::tr(env.language, "podcasts.categories.top_fallback"));
                    Vec::new()
                }
                Mode::SearchInCategory => {
                    if search_term.trim().is_empty() {
                        return Err(i18n::tr(env.language, "podcasts.categories.term_required"));
                    }
                    Vec::new()
                }
            };
            if !results.is_empty() {
                return Ok((results, status));
            }
            let url = if matches!(mode, Mode::SearchInCategory) {
                apple_search_in_category(
                    search_term,
                    category.id,
                    &env.apple_country,
                    env.apple_limit,
                )
            } else {
                apple_search_by_category(category.id, &env.apple_country, env.apple_limit)
            };
            log_debug(&format!(
                "podcasts_categories apple search fetch start: mode={:?} category_id={} url={}",
                mode, category.id, url
            ));
            let bytes = rss::fetch_url_bytes(&url, env.fetch_config)
                .await
                .map_err(|e| e.to_string())?;
            log_debug(&format!(
                "podcasts_categories apple search fetch done: mode={:?} category_id={} bytes={}",
                mode,
                category.id,
                bytes.len()
            ));
            let parsed: ItunesSearchResponse =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            log_debug(&format!(
                "podcasts_categories apple parse done: mode={:?} category_id={} result_count={}",
                mode,
                category.id,
                parsed.results.len()
            ));
            let items = parsed.results;
            log_itunes_items("apple_search", category.id, &items);
            let (filtered, fallback_status) =
                itunes_filter_by_genre(&items, category.id, env.language);
            let results = if filtered.is_empty() && fallback_status.is_some() {
                itunes_items_to_results_slice(&items)
            } else {
                filtered
            };
            if status.is_none() {
                status = fallback_status;
            }
            Ok((results, status))
        }
        Source::PodcastIndex => {
            let (key, secret) = env
                .podcastindex_auth
                .clone()
                .ok_or_else(|| i18n::tr(env.language, "podcasts.categories.missing_credentials"))?;
            let client = podcastindex_client()?;
            let mut status = None;
            let lang_code = podcastindex_language_code(env.language);
            let feeds = match mode {
                Mode::Top => {
                    let url = format!(
                        "https://api.podcastindex.org/api/1.0/podcasts/trending?max=50&cat={}&lang={}",
                        category.id, lang_code
                    );
                    let resp = add_podcastindex_auth_headers(client.get(url), &key, &secret)
                        .send()
                        .await
                        .map_err(|e| e.to_string())?;
                    if !resp.status().is_success() {
                        return Err(format!("HTTP {}", resp.status().as_u16()));
                    }
                    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
                    let parsed: PodcastIndexTrendingResponse =
                        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                    parsed.feeds.unwrap_or_default()
                }
                Mode::SearchInCategory => {
                    if search_term.trim().is_empty() {
                        return Err(i18n::tr(env.language, "podcasts.categories.term_required"));
                    }
                    let url = format!(
                        "https://api.podcastindex.org/api/1.0/search/byterm?q={}&max=50&cat={}",
                        percent_encode(search_term),
                        category.id
                    );
                    let resp = add_podcastindex_auth_headers(client.get(url), &key, &secret)
                        .send()
                        .await
                        .map_err(|e| e.to_string())?;
                    if !resp.status().is_success() {
                        return Err(format!("HTTP {}", resp.status().as_u16()));
                    }
                    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
                    let parsed: PodcastIndexSearchResponse =
                        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                    parsed.feeds.unwrap_or_default()
                }
            };
            let (results, has_categories) =
                podcastindex_feeds_to_results(feeds, Some(&category), env.language);
            if !has_categories {
                status = Some(i18n::tr(
                    env.language,
                    "podcasts.categories.unfiltered_notice",
                ));
            }
            Ok((results, status))
        }
    }
}

struct CategoryLoadEnv {
    language: Language,
    apple_country: String,
    apple_limit: u32,
    fetch_config: rss::RssFetchConfig,
    podcastindex_auth: Option<(String, String)>,
}

fn update_category_status(hwnd: HWND, message: Option<&str>) {
    let hwnd_status = with_category_dialog_state(hwnd, |s| s.hwnd_status).unwrap_or(HWND(0));
    if hwnd_status.0 == 0 {
        return;
    }
    let text = message.unwrap_or_default();
    let wide = to_wide(text);
    if let Err(e) = crate::set_window_text_w_safe(hwnd_status, PCWSTR(wide.as_ptr())) {
        crate::log_debug(&format!("SetWindowTextW failed: {:?}", e));
    }
}

fn update_category_list(hwnd: HWND, categories: Vec<Category>) {
    let hwnd_list = with_category_dialog_state(hwnd, |s| s.hwnd_list).unwrap_or(HWND(0));
    if hwnd_list.0 == 0 {
        return;
    }
    crate::send_message_w_safe(hwnd_list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    for category in &categories {
        let wide = to_wide(&category.name);
        crate::send_message_w_safe(
            hwnd_list,
            LB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    with_category_dialog_state(hwnd, |s| s.categories = categories);
    crate::send_message_w_safe(hwnd_list, LB_SETCURSEL, WPARAM(0), LPARAM(0));
}

fn load_categories_for_source(hwnd: HWND, source: Source) {
    let language = with_category_dialog_state(hwnd, |s| s.language).unwrap_or_default();
    crate::log_debug(&format!(
        "podcasts_categories load source: hwnd={:?} source={:?}",
        hwnd, source
    ));
    with_category_dialog_state(hwnd, |s| s.source = source);
    match source {
        Source::Apple => {
            update_category_status(hwnd, None);
            update_category_list(hwnd, apple_categories(language));
        }
        Source::PodcastIndex => {
            update_category_status(hwnd, None);
            update_category_list(hwnd, podcastindex_categories(language));
        }
    }
}

fn set_category_mode(hwnd: HWND, mode: Mode) {
    with_category_dialog_state(hwnd, |s| s.mode = mode);
    let (label, edit) = with_category_dialog_state(hwnd, |s| (s.hwnd_term_label, s.hwnd_term_edit))
        .unwrap_or((HWND(0), HWND(0)));
    let visible = matches!(mode, Mode::SearchInCategory);
    let show_flag = if visible {
        windows::Win32::UI::WindowsAndMessaging::SW_SHOW
    } else {
        windows::Win32::UI::WindowsAndMessaging::SW_HIDE
    };
    if label.0 != 0 {
        crate::show_window_safe(label, show_flag);
    }
    if edit.0 != 0 {
        crate::show_window_safe(edit, show_flag);
    }
}

fn read_window_text(hwnd: HWND) -> String {
    if hwnd.0 == 0 {
        return String::new();
    }
    let len = crate::send_message_w_safe(
        hwnd,
        windows::Win32::UI::WindowsAndMessaging::WM_GETTEXTLENGTH,
        WPARAM(0),
        LPARAM(0),
    )
    .0;
    if len == 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len as usize + 1];
    crate::send_message_w_safe(
        hwnd,
        windows::Win32::UI::WindowsAndMessaging::WM_GETTEXT,
        WPARAM(buf.len()),
        LPARAM(buf.as_mut_ptr() as isize),
    );
    String::from_utf16_lossy(&buf[..len as usize])
}

fn trigger_category_load(
    hwnd: HWND,
    source: Source,
    mode: Mode,
    category: Category,
    search_term: String,
) {
    trigger_category_load_with_limit(hwnd, source, mode, category, search_term, APPLE_LIMIT, 0);
}

fn trigger_category_load_with_limit(
    hwnd: HWND,
    source: Source,
    mode: Mode,
    category: Category,
    search_term: String,
    apple_limit: u32,
    visible_start: usize,
) {
    crate::log_debug(&format!(
        "podcasts_categories trigger load: hwnd={:?} source={:?} mode={:?} category_id={} category_name={} term={} apple_limit={} visible_start={}",
        hwnd,
        source,
        mode,
        category.id,
        category.name,
        search_term.trim(),
        apple_limit,
        visible_start
    ));
    show_search_loading(hwnd);
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 != 0 {
        ensure_rss_http(parent);
    }
    let language = with_podcast_state(hwnd, |s| s.language).unwrap_or_default();
    let env = CategoryLoadEnv {
        language,
        apple_country: if matches!(source, Source::Apple) {
            apple_country_for_parent(parent)
        } else {
            String::new()
        },
        apple_limit,
        fetch_config: rss_fetch_config(parent),
        podcastindex_auth: if matches!(source, Source::PodcastIndex) {
            podcastindex_credentials_or_prompt(hwnd, parent)
        } else {
            None
        },
    };
    if matches!(source, Source::PodcastIndex) && env.podcastindex_auth.is_none() {
        crate::log_debug(
            "podcasts_categories trigger load aborted: missing PodcastIndex credentials",
        );
        return;
    }
    let hwnd_copy = hwnd;
    std::thread::spawn(move || {
        crate::log_debug(&format!(
            "podcasts_categories worker start: hwnd={:?} source={:?} mode={:?} category_id={} category_name={} term={}",
            hwnd_copy,
            source,
            mode,
            category.id,
            category.name,
            search_term.trim()
        ));
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => {
                crate::log_debug(&format!(
                    "podcasts_categories worker runtime ready: source={:?} mode={:?} category_id={}",
                    source, mode, category.id
                ));
                rt
            }
            Err(e) => {
                crate::log_debug(&format!("Failed to build tokio runtime: {}", e));
                return;
            }
        };
        crate::log_debug(&format!(
            "podcasts_categories worker before load_by_category: source={:?} mode={:?} category_id={} category_name={}",
            source, mode, category.id, category.name
        ));
        let result = rt.block_on(load_by_category(
            source,
            mode,
            category.clone(),
            &search_term,
            &env,
        ));
        crate::log_debug(&format!(
            "podcasts_categories worker after load_by_category: source={:?} mode={:?} category_id={} ok={}",
            source,
            mode,
            category.id,
            result.is_ok()
        ));
        crate::log_debug(&format!(
            "podcasts_categories load result: source={:?} mode={:?} category_id={} ok={}",
            source,
            mode,
            category.id,
            result.is_ok()
        ));
        let msg = match result {
            Ok((results, status)) => {
                let can_load_more =
                    apple_limit < APPLE_MAX_LIMIT && results.len() >= apple_limit as usize;
                SearchResultMsg {
                    results,
                    status,
                    error: None,
                    category_results: matches!(source, Source::Apple).then_some(
                        CategoryResultsMeta {
                            source,
                            mode,
                            category: category.clone(),
                            search_term: search_term.clone(),
                            visible_start,
                            can_load_more,
                        },
                    ),
                }
            }
            Err(err) => SearchResultMsg {
                results: Vec::new(),
                status: None,
                error: Some(err),
                category_results: None,
            },
        };
        crate::log_debug(&format!(
            "podcasts_categories worker posting search complete: hwnd={:?} source={:?} mode={:?} category_id={} error_present={} result_count={}",
            hwnd_copy,
            source,
            mode,
            category.id,
            msg.error.is_some(),
            msg.results.len()
        ));
        if let Err(e) = crate::post_message_w_safe(
            hwnd_copy,
            WM_PODCAST_SEARCH_COMPLETE,
            WPARAM(0),
            LPARAM(Box::into_raw(Box::new(msg)) as isize),
        ) {
            crate::log_debug(&format!("PostMessageW failed: {:?}", e));
        } else {
            crate::log_debug(&format!(
                "podcasts_categories worker posted search complete: hwnd={:?} category_id={}",
                hwnd_copy, category.id
            ));
        }
    });
}

fn apply_category_selection(hwnd: HWND) {
    let (list, mode, term_edit, parent) =
        with_category_dialog_state(hwnd, |s| (s.hwnd_list, s.mode, s.hwnd_term_edit, s.parent))
            .unwrap_or((HWND(0), Mode::Top, HWND(0), HWND(0)));
    crate::log_debug(&format!(
        "podcasts_categories apply selection: hwnd={:?} mode={:?} parent={:?}",
        hwnd, mode, parent
    ));
    if list.0 == 0 {
        return;
    }
    let idx = crate::send_message_w_safe(list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
    if idx < 0 {
        let language = with_category_dialog_state(hwnd, |s| s.language).unwrap_or_default();
        let message = i18n::tr(language, "podcasts.categories.no_selection");
        unsafe {
            MessageBoxW(
                hwnd,
                PCWSTR(to_wide(&message).as_ptr()),
                PCWSTR(to_wide(&i18n::tr(language, "podcasts.window.title")).as_ptr()),
                MB_OK | MB_ICONINFORMATION,
            );
        }
        return;
    }
    let category =
        with_category_dialog_state(hwnd, |s| s.categories.get(idx as usize).cloned()).flatten();
    let Some(category) = category else {
        return;
    };
    let source = with_category_dialog_state(hwnd, |s| s.source).unwrap_or(Source::Apple);
    let term = if matches!(mode, Mode::SearchInCategory) {
        read_window_text(term_edit)
    } else {
        String::new()
    };
    crate::log_debug(&format!(
        "podcasts_categories selected: source={:?} mode={:?} category_id={} category_name={} term={}",
        source,
        mode,
        category.id,
        category.name,
        term.trim()
    ));
    if matches!(mode, Mode::SearchInCategory) && term.trim().is_empty() {
        let language = with_category_dialog_state(hwnd, |s| s.language).unwrap_or_default();
        let message = i18n::tr(language, "podcasts.categories.term_required");
        unsafe {
            MessageBoxW(
                hwnd,
                PCWSTR(to_wide(&message).as_ptr()),
                PCWSTR(to_wide(&i18n::tr(language, "podcasts.window.title")).as_ptr()),
                MB_OK | MB_ICONINFORMATION,
            );
        }
        if term_edit.0 != 0 {
            crate::set_focus_safe(term_edit);
        }
        return;
    }
    if parent.0 != 0 {
        trigger_category_load(parent, source, mode, category, term);
    }
    crate::log_if_err!(crate::destroy_window_safe(hwnd));
}

fn show_categories_dialog(parent_hwnd: HWND) {
    let main_hwnd = with_podcast_state(parent_hwnd, |s| s.parent).unwrap_or(HWND(0));
    let existing = { with_state(main_hwnd, |s| s.podcasts_categories_dialog) }.unwrap_or(HWND(0));
    if existing.0 != 0 {
        crate::set_foreground_window_safe(existing);
        return;
    }
    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let class_name = to_wide(PODCASTS_CATEGORIES_CLASS);
    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
            crate::load_cursor_w_safe(HINSTANCE(0), IDC_ARROW)
                .unwrap_or_default()
                .0,
        ),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(categories_wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    crate::register_class_w_safe(&wc);

    let language = with_podcast_state(parent_hwnd, |s| s.language).unwrap_or_default();
    let title = i18n::tr(language, "podcasts.categories.dialog.title");
    let title_wide = to_wide(&title);
    let initial_source = match selected_search_provider(parent_hwnd) {
        SearchProvider::PodcastIndex => Source::PodcastIndex,
        SearchProvider::Itunes => Source::Apple,
    };
    let search_edit = with_podcast_state(parent_hwnd, |s| s.hwnd_search).unwrap_or(HWND(0));
    let initial_term = read_window_text(search_edit);
    crate::log_debug(&format!(
        "podcasts_categories open dialog: parent_hwnd={:?} main_hwnd={:?} initial_source={:?} initial_term={}",
        parent_hwnd,
        main_hwnd,
        initial_source,
        initial_term.trim()
    ));
    let init_ptr = Box::into_raw(Box::new(CategoryDialogInit {
        parent: parent_hwnd,
        initial_source,
        initial_term,
    }));
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE | WS_POPUP,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            520,
            420,
            parent_hwnd,
            None,
            hinstance,
            Some(init_ptr as *const _),
        )
    };
    if hwnd.0 == 0 {
        unsafe {
            let _unused_box = Box::from_raw(init_ptr);
        }
        return;
    }
    if main_hwnd.0 != 0 {
        with_state(main_hwnd, |s| s.podcasts_categories_dialog = hwnd);
    }
}

unsafe extern "system" fn categories_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "categories_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || categories_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn categories_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
                let init_ptr = (*cs).lpCreateParams as *mut CategoryDialogInit;
                let init = if init_ptr.is_null() {
                    return LRESULT(0);
                } else {
                    Box::from_raw(init_ptr)
                };
                crate::log_debug(&format!(
                    "podcasts_categories WM_CREATE: hwnd={:?} parent={:?} initial_source={:?} initial_term={}",
                    hwnd,
                    init.parent,
                    init.initial_source,
                    init.initial_term.trim()
                ));
                let language = with_podcast_state(init.parent, |s| s.language).unwrap_or_default();
                let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);

                let label_source = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(
                        to_wide(&i18n::tr(language, "podcasts.categories.source.label")).as_ptr(),
                    ),
                    WS_CHILD | WS_VISIBLE,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(1),
                    hinstance,
                    None,
                );
                let combo_source = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("COMBOBOX"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(CATEGORIES_SOURCE_COMBO_ID as isize),
                    hinstance,
                    None,
                );

                let label_mode = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&i18n::tr(language, "podcasts.categories.mode.label")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(2),
                    hinstance,
                    None,
                );
                let combo_mode = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("COMBOBOX"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(CATEGORIES_MODE_COMBO_ID as isize),
                    hinstance,
                    None,
                );

                let label_list = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&i18n::tr(language, "podcasts.categories.list.label")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(3),
                    hinstance,
                    None,
                );
                let list = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("LISTBOX"),
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WINDOW_STYLE(
                            (LBS_NOTIFY as u32)
                                | windows::Win32::UI::WindowsAndMessaging::WS_VSCROLL.0,
                        ),
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(CATEGORIES_LIST_ID as isize),
                    hinstance,
                    None,
                );
                let list_proc = if list.0 != 0 {
                    let proc_ptr = category_list_wndproc as *const () as usize;
                    let old = SetWindowLongPtrW(
                        list,
                        windows::Win32::UI::WindowsAndMessaging::GWLP_WNDPROC,
                        proc_ptr as isize,
                    );
                    crate::isize_to_wndproc_safe(old)
                } else {
                    None
                };

                let term_label = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&i18n::tr(language, "podcasts.categories.term.label")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(4),
                    hinstance,
                    None,
                );
                let term_edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(CATEGORIES_TERM_EDIT_ID as isize),
                    hinstance,
                    None,
                );
                if term_edit.0 != 0 && !init.initial_term.trim().is_empty() {
                    let wide = to_wide(&init.initial_term);
                    if let Err(e) = SetWindowTextW(term_edit, PCWSTR(wide.as_ptr())) {
                        crate::log_debug(&format!("SetWindowTextW failed: {:?}", e));
                    }
                }

                let status = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(CATEGORIES_STATUS_ID as isize),
                    hinstance,
                    None,
                );

                let open_btn = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&i18n::tr(language, "podcasts.categories.open")).as_ptr()),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WINDOW_STYLE(
                            windows::Win32::UI::WindowsAndMessaging::BS_DEFPUSHBUTTON as u32,
                        ),
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(CATEGORIES_OPEN_ID as isize),
                    hinstance,
                    None,
                );
                let cancel_btn = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&i18n::tr(language, "podcasts.categories.cancel")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(CATEGORIES_CANCEL_ID as isize),
                    hinstance,
                    None,
                );

                if combo_source.0 != 0 {
                    let apple_label = i18n::tr(language, "podcasts.categories.source.apple");
                    let podcastindex_label =
                        i18n::tr(language, "podcasts.categories.source.podcastindex");
                    let apple_wide = to_wide(&apple_label);
                    let podcastindex_wide = to_wide(&podcastindex_label);
                    SendMessageW(
                        combo_source,
                        CB_ADDSTRING,
                        WPARAM(0),
                        LPARAM(apple_wide.as_ptr() as isize),
                    );
                    SendMessageW(
                        combo_source,
                        CB_ADDSTRING,
                        WPARAM(0),
                        LPARAM(podcastindex_wide.as_ptr() as isize),
                    );
                    let source_index = if matches!(init.initial_source, Source::PodcastIndex) {
                        1
                    } else {
                        0
                    };
                    SendMessageW(combo_source, CB_SETCURSEL, WPARAM(source_index), LPARAM(0));
                }
                if combo_mode.0 != 0 {
                    let top_label = i18n::tr(language, "podcasts.categories.mode.top");
                    let search_label = i18n::tr(language, "podcasts.categories.mode.search");
                    let top_wide = to_wide(&top_label);
                    let search_wide = to_wide(&search_label);
                    SendMessageW(
                        combo_mode,
                        CB_ADDSTRING,
                        WPARAM(0),
                        LPARAM(top_wide.as_ptr() as isize),
                    );
                    SendMessageW(
                        combo_mode,
                        CB_ADDSTRING,
                        WPARAM(0),
                        LPARAM(search_wide.as_ptr() as isize),
                    );
                    SendMessageW(combo_mode, CB_SETCURSEL, WPARAM(0), LPARAM(0));
                }

                let state = Box::new(CategoryDialogState {
                    parent: init.parent,
                    language,
                    hwnd_source_label: label_source,
                    hwnd_source: combo_source,
                    hwnd_mode_label: label_mode,
                    hwnd_mode: combo_mode,
                    hwnd_list_label: label_list,
                    hwnd_list: list,
                    list_proc,
                    hwnd_term_label: term_label,
                    hwnd_term_edit: term_edit,
                    hwnd_open: open_btn,
                    hwnd_cancel: cancel_btn,
                    hwnd_status: status,
                    source: init.initial_source,
                    mode: Mode::Top,
                    categories: Vec::new(),
                });
                SetWindowLongPtrW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                    Box::into_raw(state) as isize,
                );

                let hfont = HFONT(
                    windows::Win32::Graphics::Gdi::GetStockObject(
                        windows::Win32::Graphics::Gdi::DEFAULT_GUI_FONT,
                    )
                    .0,
                );
                for ctrl in [
                    label_source,
                    combo_source,
                    label_mode,
                    combo_mode,
                    label_list,
                    list,
                    term_label,
                    term_edit,
                    status,
                    open_btn,
                    cancel_btn,
                ] {
                    if ctrl.0 != 0 {
                        SendMessageW(ctrl, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                }

                load_categories_for_source(hwnd, init.initial_source);
                set_category_mode(hwnd, Mode::Top);
                SetFocus(list);
                LRESULT(0)
            }
            WM_SIZE => {
                let mut rect = windows::Win32::Foundation::RECT::default();
                crate::log_if_err!(GetClientRect(hwnd, &mut rect));
                let width = (rect.right - rect.left).max(0);
                let height = (rect.bottom - rect.top).max(0);
                let margin = 10;
                let spacing = 6;
                let label_h = 16;
                let row_h = 24;
                let button_h = 26;
                let status_h = 18;

                let (
                    label_source,
                    combo_source,
                    label_mode,
                    combo_mode,
                    label_list,
                    list,
                    term_label,
                    term_edit,
                    open_btn,
                    cancel_btn,
                    status,
                ) = with_category_dialog_state(hwnd, |s| {
                    (
                        s.hwnd_source_label,
                        s.hwnd_source,
                        s.hwnd_mode_label,
                        s.hwnd_mode,
                        s.hwnd_list_label,
                        s.hwnd_list,
                        s.hwnd_term_label,
                        s.hwnd_term_edit,
                        s.hwnd_open,
                        s.hwnd_cancel,
                        s.hwnd_status,
                    )
                })
                .unwrap_or((
                    HWND(0),
                    HWND(0),
                    HWND(0),
                    HWND(0),
                    HWND(0),
                    HWND(0),
                    HWND(0),
                    HWND(0),
                    HWND(0),
                    HWND(0),
                    HWND(0),
                ));

                let mut y = margin;
                if label_source.0 != 0 {
                    crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                        label_source,
                        margin,
                        y,
                        width - margin * 2,
                        label_h,
                        true,
                    ));
                }
                y += label_h + spacing;
                if combo_source.0 != 0 {
                    crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                        combo_source,
                        margin,
                        y,
                        width - margin * 2,
                        row_h,
                        true,
                    ));
                }
                y += row_h + spacing;
                if label_mode.0 != 0 {
                    crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                        label_mode,
                        margin,
                        y,
                        width - margin * 2,
                        label_h,
                        true,
                    ));
                }
                y += label_h + spacing;
                if combo_mode.0 != 0 {
                    crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                        combo_mode,
                        margin,
                        y,
                        width - margin * 2,
                        row_h,
                        true,
                    ));
                }
                y += row_h + spacing;
                if label_list.0 != 0 {
                    crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                        label_list,
                        margin,
                        y,
                        width - margin * 2,
                        label_h,
                        true,
                    ));
                }
                y += label_h + spacing;

                let term_visible = with_category_dialog_state(hwnd, |s| s.mode)
                    .map(|m| matches!(m, Mode::SearchInCategory))
                    .unwrap_or(false);
                let term_block = if term_visible {
                    label_h + spacing + row_h + spacing
                } else {
                    0
                };
                let list_h = (height - y - term_block - status_h - button_h - margin * 2).max(80);
                if list.0 != 0 {
                    crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                        list,
                        margin,
                        y,
                        width - margin * 2,
                        list_h,
                        true,
                    ));
                }
                y += list_h + spacing;
                if term_visible {
                    if term_label.0 != 0 {
                        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                            term_label,
                            margin,
                            y,
                            width - margin * 2,
                            label_h,
                            true,
                        ));
                    }
                    y += label_h + spacing;
                    if term_edit.0 != 0 {
                        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                            term_edit,
                            margin,
                            y,
                            width - margin * 2,
                            row_h,
                            true,
                        ));
                    }
                    y += row_h + spacing;
                }
                if status.0 != 0 {
                    crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                        status,
                        margin,
                        y,
                        width - margin * 2,
                        status_h,
                        true,
                    ));
                }
                y += status_h + spacing;

                let button_w = 100;
                let cancel_x = (width - margin - button_w).max(margin);
                let open_x = (cancel_x - spacing - button_w).max(margin);
                if open_btn.0 != 0 {
                    crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                        open_btn, open_x, y, button_w, button_h, true,
                    ));
                }
                if cancel_btn.0 != 0 {
                    crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                        cancel_btn, cancel_x, y, button_w, button_h, true,
                    ));
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let code = ((wparam.0 >> 16) & 0xffff) as u16;
                match id {
                    CATEGORIES_OPEN_ID => {
                        apply_category_selection(hwnd);
                        LRESULT(0)
                    }
                    CATEGORIES_CANCEL_ID | 2 => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    CATEGORIES_SOURCE_COMBO_ID => {
                        if code == windows::Win32::UI::WindowsAndMessaging::CBN_SELCHANGE as u16 {
                            let sel = SendMessageW(
                                with_category_dialog_state(hwnd, |s| s.hwnd_source)
                                    .unwrap_or(HWND(0)),
                                CB_GETCURSEL,
                                WPARAM(0),
                                LPARAM(0),
                            )
                            .0;
                            let source = if sel == 1 {
                                Source::PodcastIndex
                            } else {
                                Source::Apple
                            };
                            crate::log_debug(&format!(
                                "podcasts_categories source changed: hwnd={:?} sel={} source={:?}",
                                hwnd, sel, source
                            ));
                            load_categories_for_source(hwnd, source);
                        }
                        LRESULT(0)
                    }
                    CATEGORIES_MODE_COMBO_ID => {
                        if code == windows::Win32::UI::WindowsAndMessaging::CBN_SELCHANGE as u16 {
                            let sel = SendMessageW(
                                with_category_dialog_state(hwnd, |s| s.hwnd_mode)
                                    .unwrap_or(HWND(0)),
                                CB_GETCURSEL,
                                WPARAM(0),
                                LPARAM(0),
                            )
                            .0;
                            let mode = if sel == 1 {
                                Mode::SearchInCategory
                            } else {
                                Mode::Top
                            };
                            crate::log_debug(&format!(
                                "podcasts_categories mode changed: hwnd={:?} sel={} mode={:?}",
                                hwnd, sel, mode
                            ));
                            set_category_mode(hwnd, mode);
                            SendMessageW(hwnd, WM_SIZE, WPARAM(0), LPARAM(0));
                        }
                        LRESULT(0)
                    }
                    CATEGORIES_LIST_ID => {
                        if code == LBN_DBLCLK as u16 {
                            apply_category_selection(hwnd);
                        }
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_KEYDOWN => {
                let key = wparam.0 as u16;
                if key == VK_ESCAPE.0 {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                if key == VK_RETURN.0 {
                    apply_category_selection(hwnd);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_PODCAST_CATEGORIES_READY => {
                let ptr = lparam.0 as *mut CategoryListMsg;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let msg = Box::from_raw(ptr);
                if let Some(error) = msg.error.as_deref() {
                    let language =
                        with_category_dialog_state(hwnd, |s| s.language).unwrap_or_default();
                    update_category_status(hwnd, Some(error));
                    let title = i18n::tr(language, "app.error_title");
                    MessageBoxW(
                        hwnd,
                        PCWSTR(to_wide(error).as_ptr()),
                        PCWSTR(to_wide(&title).as_ptr()),
                        MB_OK | MB_ICONINFORMATION,
                    );
                    announce_status(error);
                } else {
                    update_category_status(hwnd, None);
                    update_category_list(hwnd, msg.categories);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                let parent = GetParent(hwnd);
                if parent.0 != 0 {
                    let main_hwnd = with_podcast_state(parent, |s| s.parent).unwrap_or(HWND(0));
                    if main_hwnd.0 != 0 {
                        with_state(main_hwnd, |s| s.podcasts_categories_dialog = HWND(0));
                    }
                    let hwnd_results =
                        with_podcast_state(parent, |s| s.hwnd_results).unwrap_or(HWND(0));
                    if hwnd_results.0 != 0 {
                        SetFocus(hwnd_results);
                    }
                }
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut CategoryDialogState;
                if !ptr.is_null() {
                    let _unused_box = Box::from_raw(ptr);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe extern "system" fn category_list_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "category_list_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || category_list_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn category_list_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{DLGC_WANTALLKEYS, WM_GETDLGCODE};

    unsafe {
        // Tell the dialog we want to handle all keys including Enter
        if msg == WM_GETDLGCODE {
            return LRESULT(DLGC_WANTALLKEYS as isize);
        }

        // Handle Enter key - check both WM_KEYDOWN and WM_CHAR
        if (msg == WM_KEYDOWN || msg == WM_CHAR) && wparam.0 as u16 == VK_RETURN.0 {
            let parent = GetParent(hwnd);
            if parent.0 != 0 {
                apply_category_selection(parent);
            }
            return LRESULT(0);
        }
        let parent = GetParent(hwnd);
        let prev_proc = if parent.0 != 0 {
            with_category_dialog_state(parent, |s| s.list_proc).unwrap_or(None)
        } else {
            None
        };
        if let Some(proc) = prev_proc {
            CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
        } else {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

fn subscribe_selected_result(hwnd: HWND) {
    if let Some(action) = selected_search_nav_action(hwnd) {
        navigate_category_results_page(hwnd, action);
        return;
    }
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    let Some(result) = selected_search_result(hwnd) else {
        return;
    };
    let new_index = add_podcast_source(parent, &result.feed_url, &result.title);
    if let Some(index) = new_index {
        let language = { with_state(parent, |s| s.settings.language) }.unwrap_or_default();
        announce_status(&i18n::tr(language, "podcasts.added"));
        show_subscription_confirmation(hwnd, language);

        let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
        if hwnd_tree.0 != 0 {
            let display = {
                with_state(parent, |ps| {
                    ps.settings.podcast_sources.get(index).map(|src| {
                        podcast_source_display_title(
                            src,
                            language,
                            ps.settings.announce_unread_rss_podcast_items,
                            ps.settings.rss_podcast_unread_label_position,
                        )
                    })
                })
            }
            .flatten()
            .unwrap_or_else(|| {
                if result.title.trim().is_empty() {
                    result.feed_url.clone()
                } else {
                    result.title.clone()
                }
            });
            let hitem = create_tree_item(hwnd_tree, &display, index);
            if hitem.0 != 0 {
                with_podcast_state(hwnd, |s| {
                    s.node_data.insert(hitem.0, NodeData::Source(index));
                });
                unsafe {
                    SendMessageW(
                        hwnd_tree,
                        TVM_SELECTITEM,
                        WPARAM(TVGN_CARET as usize),
                        LPARAM(hitem.0),
                    );
                    SendMessageW(hwnd_tree, TVM_ENSUREVISIBLE, WPARAM(0), LPARAM(hitem.0));
                    SetForegroundWindow(hwnd);
                    SetFocus(hwnd_tree);
                    SendMessageW(hwnd_tree, WM_SETFOCUS, WPARAM(0), LPARAM(0));
                    load_episode_children(hwnd, hitem, NodeData::Source(index), false);
                }
            }
        }
    }
}

fn show_add_dialog(parent_hwnd: HWND) {
    let main_hwnd = with_podcast_state(parent_hwnd, |s| s.parent).unwrap_or(HWND(0));
    let existing = { with_state(main_hwnd, |s| s.podcasts_add_dialog) }.unwrap_or(HWND(0));
    if existing.0 != 0 {
        crate::set_foreground_window_safe(existing);
        return;
    }
    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let class_name = to_wide(PODCASTS_ADD_CLASS);
    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
            crate::load_cursor_w_safe(HINSTANCE(0), IDC_ARROW)
                .unwrap_or_default()
                .0,
        ),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(add_wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    crate::register_class_w_safe(&wc);

    let language = with_podcast_state(parent_hwnd, |s| s.language).unwrap_or_default();
    let init_ptr = Box::into_raw(Box::new(AddDialogInit {
        parent: parent_hwnd,
    }));
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide(&i18n::tr(language, "podcasts.add_dialog.title")).as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE | WS_POPUP,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            360,
            140,
            parent_hwnd,
            None,
            hinstance,
            Some(init_ptr as *const _),
        )
    };
    if hwnd.0 == 0 {
        unsafe {
            let _unused_box = Box::from_raw(init_ptr);
        }
        return;
    }
    with_state(main_hwnd, |s| s.podcasts_add_dialog = hwnd);
}

unsafe extern "system" fn add_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "add_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || add_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn add_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
                let init_ptr = (*cs).lpCreateParams as *mut AddDialogInit;
                let parent = if init_ptr.is_null() {
                    HWND(0)
                } else {
                    let init = Box::from_raw(init_ptr);
                    init.parent
                };
                let language = with_podcast_state(parent, |s| s.language).unwrap_or_default();
                let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&i18n::tr(language, "podcasts.add_dialog.url_label")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    10,
                    10,
                    320,
                    16,
                    hwnd,
                    HMENU(1),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    10,
                    28,
                    320,
                    24,
                    hwnd,
                    HMENU(ADD_URL_EDIT_ID as isize),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&i18n::tr(language, "podcasts.add_dialog.ok")).as_ptr()),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WINDOW_STYLE(
                            windows::Win32::UI::WindowsAndMessaging::BS_DEFPUSHBUTTON as u32,
                        ),
                    150,
                    70,
                    80,
                    24,
                    hwnd,
                    HMENU(ADD_OK_ID as isize),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&i18n::tr(language, "podcasts.add_dialog.cancel")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    250,
                    70,
                    80,
                    24,
                    hwnd,
                    HMENU(ADD_CANCEL_ID as isize),
                    hinstance,
                    None,
                );
                SetFocus(GetDlgItem(hwnd, ADD_URL_EDIT_ID as i32));
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                match id {
                    1 => {
                        SendMessageW(hwnd, WM_COMMAND, WPARAM(ADD_OK_ID), LPARAM(0));
                        LRESULT(0)
                    }
                    ADD_OK_ID => {
                        let h_edit_url = GetDlgItem(hwnd, ADD_URL_EDIT_ID as i32);
                        let len = SendMessageW(
                            h_edit_url,
                            windows::Win32::UI::WindowsAndMessaging::WM_GETTEXTLENGTH,
                            WPARAM(0),
                            LPARAM(0),
                        )
                        .0;
                        let mut buf = vec![0u16; len as usize + 1];
                        SendMessageW(
                            h_edit_url,
                            windows::Win32::UI::WindowsAndMessaging::WM_GETTEXT,
                            WPARAM(buf.len()),
                            LPARAM(buf.as_mut_ptr() as isize),
                        );
                        let url = String::from_utf16_lossy(&buf[..len as usize]);
                        let parent = GetParent(hwnd);
                        if !url.trim().is_empty() {
                            let payload = url.trim().to_string();
                            let url_wide = to_wide(&payload);
                            let cds = COPYDATASTRUCT {
                                dwData: PODCAST_ADD_COPYDATA,
                                cbData: (url_wide.len() * 2) as u32,
                                lpData: url_wide.as_ptr() as *mut _,
                            };
                            SendMessageW(
                                parent,
                                WM_COPYDATA,
                                WPARAM(hwnd.0 as usize),
                                LPARAM(&cds as *const _ as isize),
                            );
                        }
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    ADD_CANCEL_ID | 2 => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_DESTROY => {
                let parent = GetParent(hwnd);
                let main_hwnd = with_podcast_state(parent, |s| s.parent).unwrap_or(HWND(0));
                with_state(main_hwnd, |s| s.podcasts_add_dialog = HWND(0));
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

#[derive(serde::Deserialize)]
struct ItunesSearchResponse {
    #[serde(default)]
    results: Vec<ItunesSearchItem>,
}

#[derive(serde::Deserialize)]
struct ItunesSearchItem {
    #[serde(rename = "collectionId")]
    collection_id: Option<u64>,
    #[serde(rename = "collectionName")]
    collection_name: Option<String>,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    #[serde(rename = "feedUrl")]
    feed_url: Option<String>,
    #[serde(rename = "primaryGenreId")]
    primary_genre_id: Option<u32>,
    #[serde(rename = "genreIds")]
    genre_ids: Option<Vec<String>>,
}

#[derive(serde::Deserialize)]
struct SpreakerSearchResponse {
    response: SpreakerSearchPayload,
}

#[derive(serde::Deserialize)]
struct SpreakerSearchPayload {
    #[serde(default)]
    items: Vec<SpreakerShowItem>,
}

#[derive(serde::Deserialize)]
struct SpreakerShowItem {
    show_id: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    author_name: Option<String>,
    #[serde(default)]
    author: Option<SpreakerAuthor>,
}

#[derive(serde::Deserialize)]
struct SpreakerAuthor {
    fullname: Option<String>,
}

#[derive(serde::Deserialize)]
struct SpreakerShowResponse {
    response: SpreakerShowPayload,
}

#[derive(serde::Deserialize)]
struct SpreakerShowPayload {
    show: SpreakerShowItem,
}

#[derive(serde::Deserialize)]
struct PodcastIndexSearchResponse {
    feeds: Option<Vec<PodcastIndexFeed>>,
}

#[derive(serde::Deserialize)]
struct PodcastIndexFeed {
    title: Option<String>,
    author: Option<String>,
    #[serde(rename = "ownerName")]
    owner_name: Option<String>,
    url: Option<String>,
    #[serde(rename = "feedUrl")]
    feed_url: Option<String>,
    language: Option<String>,
    categories: Option<HashMap<String, String>>,
}

#[derive(serde::Deserialize)]
struct PodcastIndexTrendingResponse {
    feeds: Option<Vec<PodcastIndexFeed>>,
}

pub fn show_context_menu_from_keyboard(hwnd: HWND) {
    unsafe {
        let mut pt = windows::Win32::Foundation::POINT::default();
        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::GetCursorPos(
            &mut pt
        ));
        show_context_menu(hwnd, pt.x, pt.y, false);
    }
}

pub fn focus_library(hwnd: HWND) {
    let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 != 0 {
        crate::set_focus_safe(hwnd_tree);
    }
}

fn force_focus_editor_on_parent(parent: HWND) {
    if parent.0 == 0 {
        return;
    }
    unsafe {
        SetForegroundWindow(parent);
        SetActiveWindow(parent);
        SendMessageW(parent, WM_SETFOCUS, WPARAM(0), LPARAM(0));
    }
    if crate::get_active_edit(parent).is_none() {
        crate::send_message_w_safe(
            parent,
            WM_COMMAND,
            WPARAM(crate::menu::IDM_FILE_NEW),
            LPARAM(0),
        );
    }
    if let Some(hwnd_edit) = crate::get_active_edit(parent) {
        unsafe {
            SetFocus(hwnd_edit);
            SendMessageW(hwnd_edit, WM_SETFOCUS, WPARAM(0), LPARAM(0));
            SendMessageW(
                parent,
                WM_NEXTDLGCTL,
                WPARAM(hwnd_edit.0 as usize),
                LPARAM(1),
            );
        }
        // Re-assert focus after dialog navigation to help screen readers settle on the edit control.
        unsafe {
            SetFocus(hwnd_edit);
            SendMessageW(hwnd_edit, WM_SETFOCUS, WPARAM(0), LPARAM(0));
            NotifyWinEvent(
                EVENT_OBJECT_FOCUS,
                hwnd_edit,
                OBJID_CLIENT.0,
                CHILDID_SELF as i32,
            );
        }
    }
    crate::send_message_w_safe(parent, WM_SETFOCUS, WPARAM(0), LPARAM(0));
    if let Err(_e) =
        crate::post_message_w_safe(parent, crate::WM_FOCUS_EDITOR, WPARAM(0), LPARAM(0))
    {
        crate::log_debug(&format!("Error: {:?}", _e));
    }
}

fn show_context_menu(hwnd: HWND, x: i32, y: i32, use_hit_test: bool) {
    let (hwnd_tree, hwnd_results) =
        with_podcast_state(hwnd, |s| (s.hwnd_tree, s.hwnd_results)).unwrap_or((HWND(0), HWND(0)));
    if hwnd_tree.0 == 0 {
        return;
    }
    let focus = crate::get_focus_safe();
    let target_list = focus == hwnd_results;
    if target_list {
        show_search_context_menu(hwnd, x, y, use_hit_test);
    } else {
        show_tree_context_menu(hwnd, x, y, use_hit_test);
    }
}

fn selected_search_result(hwnd: HWND) -> Option<PodcastSearchResult> {
    let (results, idx, result_offset) = with_podcast_state(hwnd, |s| {
        let idx =
            crate::send_message_w_safe(s.hwnd_results, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
        let result_offset = usize::from(
            s.category_results
                .as_ref()
                .map(|pager| pager.visible_start > 0)
                .unwrap_or(false),
        );
        (s.search_results.clone(), idx, result_offset)
    })
    .unwrap_or((Vec::new(), -1, 0));
    if idx < 0 {
        return None;
    }
    let idx = idx as usize;
    if idx < result_offset {
        return None;
    }
    let result_idx = idx - result_offset;
    if result_idx >= results.len() {
        return None;
    }
    Some(results[result_idx].clone())
}

fn selected_search_nav_action(hwnd: HWND) -> Option<SearchResultsNavAction> {
    with_podcast_state(hwnd, |s| {
        let pager = s.category_results.as_ref()?;
        let idx =
            crate::send_message_w_safe(s.hwnd_results, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
        if idx < 0 {
            return None;
        }
        let idx = idx as usize;
        let show_previous = pager.visible_start > 0;
        if show_previous && idx == 0 {
            return Some(SearchResultsNavAction::PreviousPage);
        }
        let visible_len = s.search_results.len();
        let result_offset = usize::from(show_previous);
        if idx < result_offset + visible_len {
            return None;
        }
        if pager.visible_start + visible_len < pager.loaded_results.len() {
            return Some(SearchResultsNavAction::NextPage);
        }
        if pager.can_load_more {
            return Some(SearchResultsNavAction::LoadMore);
        }
        None
    })
    .flatten()
}

fn navigate_category_results_page(hwnd: HWND, action: SearchResultsNavAction) {
    match action {
        SearchResultsNavAction::LoadMore => {
            let Some((source, mode, category, search_term)) = with_podcast_state(hwnd, |s| {
                s.category_results.as_ref().map(|pager| {
                    (
                        pager.source,
                        pager.mode,
                        pager.category.clone(),
                        pager.search_term.clone(),
                    )
                })
            })
            .flatten() else {
                return;
            };
            trigger_category_load_with_limit(
                hwnd,
                source,
                mode,
                category,
                search_term,
                APPLE_MAX_LIMIT,
                CATEGORY_RESULTS_PAGE_SIZE,
            );
        }
        SearchResultsNavAction::PreviousPage | SearchResultsNavAction::NextPage => {
            let Some((results, status, meta)) = with_podcast_state(hwnd, |s| {
                let pager = s.category_results.as_ref()?;
                let next_start = match action {
                    SearchResultsNavAction::PreviousPage => pager
                        .visible_start
                        .saturating_sub(CATEGORY_RESULTS_PAGE_SIZE),
                    SearchResultsNavAction::NextPage => {
                        let candidate = pager.visible_start + CATEGORY_RESULTS_PAGE_SIZE;
                        candidate.min(pager.loaded_results.len().saturating_sub(1))
                    }
                    SearchResultsNavAction::LoadMore => pager.visible_start,
                };
                Some((
                    pager.loaded_results.clone(),
                    None::<String>,
                    CategoryResultsMeta {
                        source: pager.source,
                        mode: pager.mode,
                        category: pager.category.clone(),
                        search_term: pager.search_term.clone(),
                        visible_start: next_start,
                        can_load_more: pager.can_load_more,
                    },
                ))
            })
            .flatten() else {
                return;
            };
            update_search_results(hwnd, results, status.as_deref(), Some(meta));
        }
    }
}

fn trigger_search_from_edit(hwnd: HWND) {
    let (hwnd_search, hwnd_results) =
        with_podcast_state(hwnd, |s| (s.hwnd_search, s.hwnd_results)).unwrap_or((HWND(0), HWND(0)));
    if hwnd_search.0 == 0 {
        return;
    }
    let len = crate::send_message_w_safe(
        hwnd_search,
        windows::Win32::UI::WindowsAndMessaging::WM_GETTEXTLENGTH,
        WPARAM(0),
        LPARAM(0),
    )
    .0;
    let mut buf = vec![0u16; len as usize + 1];
    crate::send_message_w_safe(
        hwnd_search,
        windows::Win32::UI::WindowsAndMessaging::WM_GETTEXT,
        WPARAM(buf.len()),
        LPARAM(buf.as_mut_ptr() as isize),
    );
    let query = String::from_utf16_lossy(&buf[..len as usize]);
    perform_search(hwnd, &query);
    if hwnd_results.0 != 0 {
        unsafe {
            SetFocus(hwnd_results);
        }
    }
}

fn show_search_result_info(hwnd: HWND) {
    let result = match selected_search_result(hwnd) {
        Some(result) => result,
        None => return,
    };
    let language = with_podcast_state(hwnd, |s| s.language).unwrap_or_default();
    let title = i18n::tr(language, "podcasts.search.info_title");
    let body = i18n::tr_f(
        language,
        "podcasts.search.info_body",
        &[
            ("title", &result.title),
            ("artist", &result.artist),
            ("feed", &result.feed_url),
        ],
    );
    unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(to_wide(&body).as_ptr()),
            PCWSTR(to_wide(&title).as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::MB_OK,
        );
    }
}

fn show_selected_result_episodes(hwnd: HWND) {
    if let Some(res) = selected_search_result(hwnd) {
        let preview_idx = with_podcast_state(hwnd, |s| {
            s.preview_sources.push(crate::tools::rss::RssSource {
                title: format!("{} [Preview]", res.title),
                url: res.feed_url.clone(),
                kind: crate::tools::rss::RssSourceType::Feed,
                folder_path: Vec::new(),
                user_title: true,
                unread: false,
                cache: crate::tools::rss::RssFeedCache::default(),
                last_seen_guid: None,
                last_updated: None,
                removed_item_keys: Vec::new(),
                read_item_keys: Vec::new(),
            });
            s.preview_sources.len() - 1
        })
        .unwrap_or(0);

        let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
        if hwnd_tree.0 != 0 {
            let title = format!("{} [Preview]", res.title);
            let hitem = create_tree_item(hwnd_tree, &title, preview_idx);
            if hitem.0 != 0 {
                with_podcast_state(hwnd, |s| {
                    s.node_data
                        .insert(hitem.0, NodeData::PreviewSource(preview_idx));
                });
                unsafe {
                    SendMessageW(
                        hwnd_tree,
                        TVM_SELECTITEM,
                        WPARAM(TVGN_CARET as usize),
                        LPARAM(hitem.0),
                    );
                    SetFocus(hwnd_tree);
                    load_episode_children(hwnd, hitem, NodeData::PreviewSource(preview_idx), false);
                }
            }
        }
    }
}

fn show_search_context_menu(hwnd: HWND, x: i32, y: i32, use_hit_test: bool) {
    let hwnd_results = with_podcast_state(hwnd, |s| s.hwnd_results).unwrap_or(HWND(0));
    if hwnd_results.0 == 0 {
        return;
    }
    let mut rect = windows::Win32::Foundation::RECT::default();
    if use_hit_test
        && crate::get_window_rect_safe(hwnd_results, &mut rect).is_ok()
        && (x < rect.left || x > rect.right || y < rect.top || y > rect.bottom)
    {
        return;
    }
    let language = with_podcast_state(hwnd, |s| s.language).unwrap_or_default();
    let label = i18n::tr(language, "podcasts.context.subscribe");
    let show_episodes_label = i18n::tr(language, "podcasts.context.show_episodes");
    let info_label = i18n::tr(language, "podcasts.context.info");
    let copy_label = i18n::tr(language, "podcasts.context.copy_url");
    let menu = crate::create_menu_safe().unwrap_or(HMENU(0));
    if let Err(_e) = crate::append_menu_w_safe(
        menu,
        MF_STRING,
        ID_CTX_SUBSCRIBE,
        PCWSTR(to_wide(&label).as_ptr()),
    ) {}
    if let Err(_e) = crate::append_menu_w_safe(
        menu,
        MF_STRING,
        ID_CTX_SEARCH_SHOW_EPISODES,
        PCWSTR(to_wide(&show_episodes_label).as_ptr()),
    ) {}
    if let Err(_e) = crate::append_menu_w_safe(
        menu,
        MF_STRING,
        ID_CTX_SEARCH_INFO,
        PCWSTR(to_wide(&info_label).as_ptr()),
    ) {}
    if let Err(_e) = crate::append_menu_w_safe(
        menu,
        MF_STRING,
        ID_CTX_SEARCH_COPY_URL,
        PCWSTR(to_wide(&copy_label).as_ptr()),
    ) {}
    let cmd = crate::track_popup_menu_safe(
        menu,
        windows::Win32::UI::WindowsAndMessaging::TPM_RETURNCMD,
        x,
        y,
        0,
        hwnd,
        None,
    )
    .0 as usize;
    match cmd {
        ID_CTX_SUBSCRIBE => subscribe_selected_result(hwnd),
        ID_CTX_SEARCH_SHOW_EPISODES => show_selected_result_episodes(hwnd),
        ID_CTX_SEARCH_INFO => show_search_result_info(hwnd),
        ID_CTX_SEARCH_COPY_URL => {
            if let Some(result) = selected_search_result(hwnd) {
                copy_text_to_clipboard(hwnd, &result.feed_url);
            }
        }
        _ => {}
    }
    crate::log_if_err!(crate::destroy_menu_safe(menu));
}

fn show_tree_context_menu(hwnd: HWND, x: i32, y: i32, use_hit_test: bool) {
    unsafe {
        let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
        if hwnd_tree.0 == 0 {
            return;
        }
        let mut rect = windows::Win32::Foundation::RECT::default();
        if use_hit_test
            && GetWindowRect(hwnd_tree, &mut rect).is_ok()
            && (x < rect.left || x > rect.right || y < rect.top || y > rect.bottom)
        {
            return;
        }
        let hitem = selected_tree_item(hwnd);
        if hitem.0 == 0 {
            return;
        }
        let node = with_podcast_state(hwnd, |s| s.node_data.get(&hitem.0).cloned()).flatten();
        let language = with_podcast_state(hwnd, |s| s.language).unwrap_or_default();
        let properties_label = i18n::tr(language, "context.properties");
        let undo_label = i18n::tr(language, "edit.undo")
            .split('\t')
            .next()
            .unwrap_or_default()
            .to_string();
        let undo_flags =
            if with_podcast_state(hwnd, |s| !s.removed_history.is_empty()).unwrap_or(false) {
                MF_STRING
            } else {
                MF_STRING | MF_GRAYED
            };
        let menu = CreatePopupMenu().unwrap_or(HMENU(0));
        if menu.0 == 0 {
            return;
        }
        match node {
            Some(NodeData::Source(idx)) => {
                let update_label = i18n::tr(language, "podcasts.context.update");
                let remove_label = i18n::tr(language, "podcasts.context.remove");
                let reorder_label = i18n::tr(language, "podcasts.context.reorder");
                let reorder_up = i18n::tr(language, "rss.reorder.move_up");
                let reorder_down = i18n::tr(language, "rss.reorder.move_down");
                let reorder_top = i18n::tr(language, "rss.reorder.move_top");
                let reorder_bottom = i18n::tr(language, "rss.reorder.move_bottom");
                let reorder_position = i18n::tr(language, "rss.reorder.move_to_position");
                let sort_asc = i18n::tr(language, "rss.reorder.title_asc");
                let sort_desc = i18n::tr(language, "rss.reorder.title_desc");
                let sort_newest = i18n::tr(language, "rss.reorder.date_newest");
                let sort_oldest = i18n::tr(language, "rss.reorder.date_oldest");
                let copy_url = i18n::tr(language, "podcasts.context.copy_url");
                let open_feed = i18n::tr(language, "podcasts.context.open_feed");
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_UPDATE,
                    PCWSTR(to_wide(&update_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_REMOVE,
                    PCWSTR(to_wide(&remove_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    undo_flags,
                    ID_CTX_UNDO_DELETE,
                    PCWSTR(to_wide(&undo_label).as_ptr()),
                ) {}
                let total = with_podcast_state(hwnd, |s| {
                    with_state(s.parent, |ps| ps.settings.podcast_sources.len()).unwrap_or(0)
                })
                .unwrap_or(0);
                let at_top = idx == 0;
                let at_bottom = total == 0 || idx + 1 >= total;
                if let Ok(submenu) = CreatePopupMenu()
                    && submenu.0 != 0
                {
                    let up_flags = if at_top {
                        MF_STRING | MF_GRAYED
                    } else {
                        MF_STRING
                    };
                    let down_flags = if at_bottom {
                        MF_STRING | MF_GRAYED
                    } else {
                        MF_STRING
                    };
                    if let Err(_e) = AppendMenuW(
                        submenu,
                        up_flags,
                        ID_CTX_REORDER_UP,
                        PCWSTR(to_wide(&reorder_up).as_ptr()),
                    ) {}
                    if let Err(_e) = AppendMenuW(
                        submenu,
                        down_flags,
                        ID_CTX_REORDER_DOWN,
                        PCWSTR(to_wide(&reorder_down).as_ptr()),
                    ) {}
                    if let Err(_e) = AppendMenuW(
                        submenu,
                        up_flags,
                        ID_CTX_REORDER_TOP,
                        PCWSTR(to_wide(&reorder_top).as_ptr()),
                    ) {}
                    if let Err(_e) = AppendMenuW(
                        submenu,
                        down_flags,
                        ID_CTX_REORDER_BOTTOM,
                        PCWSTR(to_wide(&reorder_bottom).as_ptr()),
                    ) {}
                    if let Err(_e) = AppendMenuW(
                        submenu,
                        MF_STRING,
                        ID_CTX_REORDER_POSITION,
                        PCWSTR(to_wide(&reorder_position).as_ptr()),
                    ) {}
                    if let Err(_e) = AppendMenuW(submenu, MF_SEPARATOR, 0, PCWSTR::null()) {}
                    if let Err(_e) = AppendMenuW(
                        submenu,
                        MF_STRING,
                        ID_CTX_SORT_ASC,
                        PCWSTR(to_wide(&sort_asc).as_ptr()),
                    ) {}
                    if let Err(_e) = AppendMenuW(
                        submenu,
                        MF_STRING,
                        ID_CTX_SORT_DESC,
                        PCWSTR(to_wide(&sort_desc).as_ptr()),
                    ) {}
                    if let Err(_e) = AppendMenuW(
                        submenu,
                        MF_STRING,
                        ID_CTX_SORT_NEWEST,
                        PCWSTR(to_wide(&sort_newest).as_ptr()),
                    ) {}
                    if let Err(_e) = AppendMenuW(
                        submenu,
                        MF_STRING,
                        ID_CTX_SORT_OLDEST,
                        PCWSTR(to_wide(&sort_oldest).as_ptr()),
                    ) {}
                    if let Err(_e) = AppendMenuW(
                        menu,
                        MF_POPUP,
                        submenu.0 as usize,
                        PCWSTR(to_wide(&reorder_label).as_ptr()),
                    ) {}
                }
                if let Err(_e) = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) {
                    crate::log_debug(&format!("Error: {:?}", _e));
                }
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_COPY_URL,
                    PCWSTR(to_wide(&copy_url).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_OPEN_FEED,
                    PCWSTR(to_wide(&open_feed).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_PROPERTIES,
                    PCWSTR(to_wide(&properties_label).as_ptr()),
                ) {}
            }
            Some(NodeData::PreviewSource(_)) => {
                let update_label = i18n::tr(language, "podcasts.context.update");
                let remove_label = i18n::tr(language, "podcasts.context.remove");
                let copy_url = i18n::tr(language, "podcasts.context.copy_url");
                let open_feed = i18n::tr(language, "podcasts.context.open_feed");
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_UPDATE,
                    PCWSTR(to_wide(&update_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_REMOVE,
                    PCWSTR(to_wide(&remove_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    undo_flags,
                    ID_CTX_UNDO_DELETE,
                    PCWSTR(to_wide(&undo_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_COPY_URL,
                    PCWSTR(to_wide(&copy_url).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_OPEN_FEED,
                    PCWSTR(to_wide(&open_feed).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_PROPERTIES,
                    PCWSTR(to_wide(&properties_label).as_ptr()),
                ) {}
            }
            Some(NodeData::Episode(_item)) => {
                let play_label = i18n::tr(language, "podcasts.context.play");
                let open_label = i18n::tr(language, "podcasts.context.open_episode");
                let copy_audio = i18n::tr(language, "podcasts.context.copy_audio");
                let copy_title = i18n::tr(language, "podcasts.context.copy_title");
                let download_label = i18n::tr(language, "podcasts.context.download_episode");
                let view_description = i18n::tr(language, "podcasts.context.view_description");
                let remove_label = i18n::tr(language, "dictionary.remove");
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_PLAY,
                    PCWSTR(to_wide(&play_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_VIEW_DESCRIPTION,
                    PCWSTR(to_wide(&view_description).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_OPEN_EPISODE,
                    PCWSTR(to_wide(&open_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_COPY_AUDIO,
                    PCWSTR(to_wide(&copy_audio).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_COPY_TITLE,
                    PCWSTR(to_wide(&copy_title).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_DOWNLOAD_EPISODE,
                    PCWSTR(to_wide(&download_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_PROPERTIES,
                    PCWSTR(to_wide(&properties_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_REMOVE_EPISODE,
                    PCWSTR(to_wide(&remove_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    undo_flags,
                    ID_CTX_UNDO_DELETE,
                    PCWSTR(to_wide(&undo_label).as_ptr()),
                ) {}
            }
            None => {}
        }

        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            windows::Win32::UI::WindowsAndMessaging::TPM_RETURNCMD,
            x,
            y,
            0,
            hwnd,
            None,
        )
        .0 as usize;
        if let Err(_e) = PostMessageW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::WM_NULL,
            WPARAM(0),
            LPARAM(0),
        ) {}
        crate::log_if_err!(DestroyMenu(menu));
        match cmd {
            ID_CTX_UPDATE => handle_source_action(hwnd, SourceAction::Update),
            ID_CTX_REMOVE => handle_source_action(hwnd, SourceAction::Remove),
            ID_CTX_COPY_URL => handle_source_action(hwnd, SourceAction::CopyUrl),
            ID_CTX_OPEN_FEED => handle_source_action(hwnd, SourceAction::OpenFeed),
            ID_CTX_REORDER_UP => handle_reorder_action(hwnd, ReorderAction::Up),
            ID_CTX_REORDER_DOWN => handle_reorder_action(hwnd, ReorderAction::Down),
            ID_CTX_REORDER_TOP => handle_reorder_action(hwnd, ReorderAction::Top),
            ID_CTX_REORDER_BOTTOM => handle_reorder_action(hwnd, ReorderAction::Bottom),
            ID_CTX_REORDER_POSITION => handle_reorder_action(hwnd, ReorderAction::Position),
            ID_CTX_SORT_ASC => handle_sort_action(hwnd, crate::settings::SortOrder::TitleAsc),
            ID_CTX_SORT_DESC => handle_sort_action(hwnd, crate::settings::SortOrder::TitleDesc),
            ID_CTX_SORT_NEWEST => handle_sort_action(hwnd, crate::settings::SortOrder::DateNewest),
            ID_CTX_SORT_OLDEST => handle_sort_action(hwnd, crate::settings::SortOrder::DateOldest),
            ID_CTX_UNDO_DELETE => undo_last_delete(hwnd),
            ID_CTX_PLAY => handle_episode_action(hwnd, EpisodeAction::Play),
            ID_CTX_OPEN_EPISODE => handle_episode_action(hwnd, EpisodeAction::OpenEpisode),
            ID_CTX_COPY_AUDIO => handle_episode_action(hwnd, EpisodeAction::CopyAudio),
            ID_CTX_COPY_TITLE => handle_episode_action(hwnd, EpisodeAction::CopyTitle),
            ID_CTX_DOWNLOAD_EPISODE => handle_episode_action(hwnd, EpisodeAction::Download),
            ID_CTX_VIEW_DESCRIPTION => handle_episode_action(hwnd, EpisodeAction::ViewDescription),
            ID_CTX_REMOVE_EPISODE => handle_episode_action(hwnd, EpisodeAction::Remove),
            ID_CTX_PROPERTIES => show_selected_properties(hwnd),
            ID_CTX_SUBSCRIBE => subscribe_selected_result(hwnd),
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
enum SourceAction {
    Update,
    Remove,
    CopyUrl,
    OpenFeed,
}

fn handle_source_action(hwnd: HWND, verb: SourceAction) {
    let Some(source_index) = selected_source_index(hwnd) else {
        return;
    };
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    match verb {
        SourceAction::Update => {
            let hitem = selected_tree_item(hwnd);
            if hitem.0 != 0 {
                load_episode_children(hwnd, hitem, NodeData::Source(source_index), true);
                if parent.0 != 0 {
                    let language =
                        { with_state(parent, |s| s.settings.language) }.unwrap_or_default();
                    announce_status(&i18n::tr(language, "podcasts.updated"));
                }
            }
        }
        SourceAction::Remove => {
            let confirm = if parent.0 != 0 {
                let (language, require_confirm) = {
                    with_state(parent, |s| {
                        (
                            s.settings.language,
                            matches!(
                                s.settings.podcast_delete_confirm_mode,
                                crate::settings::PodcastDeleteConfirmMode::Podcast
                                    | crate::settings::PodcastDeleteConfirmMode::Both
                            ),
                        )
                    })
                }
                .unwrap_or((Language::default(), true));
                if require_confirm {
                    let title = confirm_title(language);
                    let msg = i18n::tr(language, "podcasts.remove_confirm");
                    unsafe {
                        MessageBoxW(
                            hwnd,
                            PCWSTR(to_wide(&msg).as_ptr()),
                            PCWSTR(to_wide(&title).as_ptr()),
                            MB_YESNOCANCEL,
                        ) == IDYES
                    }
                } else {
                    true
                }
            } else {
                true
            };
            if !confirm {
                let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                if hwnd_tree.0 != 0 {
                    crate::set_focus_safe(hwnd_tree);
                }
                update_delete_button_state(hwnd);
                return;
            }
            let removed_source = {
                with_state(parent, |ps| {
                    ps.settings.podcast_sources.get(source_index).cloned()
                })
            }
            .flatten();
            let removed = {
                with_state(parent, |ps| {
                    if source_index < ps.settings.podcast_sources.len() {
                        ps.settings.podcast_sources.remove(source_index);
                        settings::save_settings(ps.settings.clone());
                        true
                    } else {
                        false
                    }
                })
            }
            .unwrap_or(false);
            if removed {
                if let Some(source) = removed_source {
                    with_podcast_state(hwnd, |s| {
                        s.removed_history.push(PodcastLastRemoved::Source {
                            index: source_index,
                            source,
                        });
                    });
                }
                let language = { with_state(parent, |s| s.settings.language) }.unwrap_or_default();
                announce_status(&i18n::tr(language, "podcasts.removed"));
                reload_tree(hwnd);
                update_delete_button_state(hwnd);
                let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                if hwnd_tree.0 != 0 {
                    crate::set_focus_safe(hwnd_tree);
                    let first = HTREEITEM(
                        crate::send_message_w_safe(
                            hwnd_tree,
                            TVM_GETNEXTITEM,
                            WPARAM(TVGN_ROOT as usize),
                            LPARAM(0),
                        )
                        .0,
                    );
                    if first.0 != 0 {
                        crate::send_message_w_safe(
                            hwnd_tree,
                            TVM_SELECTITEM,
                            WPARAM(TVGN_CARET as usize),
                            LPARAM(first.0),
                        );
                    }
                }
            }
            {
                let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                if hwnd_tree.0 != 0 {
                    crate::set_focus_safe(hwnd_tree);
                }
                update_delete_button_state(hwnd);
            }
        }
        SourceAction::CopyUrl => {
            let url = {
                with_state(parent, |ps| {
                    ps.settings
                        .podcast_sources
                        .get(source_index)
                        .map(|s| s.url.clone())
                })
            }
            .unwrap_or(None)
            .unwrap_or_default();
            if !url.is_empty() {
                copy_text_to_clipboard(hwnd, &url);
            }
        }
        SourceAction::OpenFeed => {
            let url = {
                with_state(parent, |ps| {
                    ps.settings
                        .podcast_sources
                        .get(source_index)
                        .map(|s| s.url.clone())
                })
            }
            .unwrap_or(None)
            .unwrap_or_default();
            if !url.is_empty()
                && let Err(_e) = crate::audio_utils::open_url_in_browser(&url)
            {
                crate::log_debug(&format!("Error: {:?}", _e));
            }
        }
    }
}

#[derive(Clone, Copy)]
enum EpisodeAction {
    Play,
    OpenEpisode,
    CopyAudio,
    CopyTitle,
    Download,
    ViewDescription,
    Remove,
}

fn handle_episode_action(hwnd: HWND, action: EpisodeAction) {
    let Some(item) = selected_episode(hwnd) else {
        return;
    };
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    match action {
        EpisodeAction::Play => open_episode_in_player(hwnd, parent, &item),
        EpisodeAction::OpenEpisode => {
            if !item.link.trim().is_empty()
                && let Err(_e) = crate::audio_utils::open_url_in_browser(&item.link)
            {
                crate::log_debug(&format!("Error: {:?}", _e));
            }
        }
        EpisodeAction::CopyAudio => {
            if let Some(url) = item.enclosure_url {
                copy_text_to_clipboard(hwnd, &url);
            }
        }
        EpisodeAction::CopyTitle => copy_text_to_clipboard(hwnd, &item.title),
        EpisodeAction::Download => {
            if parent.0 == 0 {
                return;
            }
            let Some(url) = item.enclosure_url.clone() else {
                return;
            };
            let cache_path = podcast_cache_path(&url, item.enclosure_type.as_deref());
            crate::download_podcast_episode(
                parent,
                Some(url),
                selected_source_name(hwnd),
                Some(item.title.clone()),
                Some(cache_path),
                { with_state(parent, |s| s.settings.language) }.unwrap_or_default(),
            );
        }
        EpisodeAction::ViewDescription => {
            let content = crate::tools::reader::clean_text(&item.description);
            let final_content = crate::tools::reader::collapse_blank_lines(&content);
            show_description_dialog(hwnd, &item.title, &final_content);
        }
        EpisodeAction::Remove => {
            let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
            let selected_hitem = selected_tree_item(hwnd);
            let (language, require_confirm) = {
                with_state(parent, |s| {
                    (
                        s.settings.language,
                        matches!(
                            s.settings.podcast_delete_confirm_mode,
                            crate::settings::PodcastDeleteConfirmMode::Episode
                                | crate::settings::PodcastDeleteConfirmMode::Both
                        ),
                    )
                })
            }
            .unwrap_or((Language::default(), true));
            let remove_label = i18n::tr(language, "dictionary.remove");
            let title = if item.title.trim().is_empty() {
                item.link.clone()
            } else {
                item.title.clone()
            };
            let msg = format!("{remove_label}: \"{title}\"?");
            let confirmed = if require_confirm {
                let caption = confirm_title(language);
                unsafe {
                    MessageBoxW(
                        hwnd,
                        PCWSTR(to_wide(&msg).as_ptr()),
                        PCWSTR(to_wide(&caption).as_ptr()),
                        MB_YESNOCANCEL | MB_ICONQUESTION,
                    ) == IDYES
                }
            } else {
                true
            };
            if !confirmed {
                if hwnd_tree.0 != 0 {
                    if selected_hitem.0 != 0 {
                        unsafe {
                            SendMessageW(
                                hwnd_tree,
                                TVM_SELECTITEM,
                                WPARAM(TVGN_CARET as usize),
                                LPARAM(selected_hitem.0),
                            );
                            SendMessageW(
                                hwnd_tree,
                                TVM_ENSUREVISIBLE,
                                WPARAM(0),
                                LPARAM(selected_hitem.0),
                            );
                        }
                    }
                    if crate::get_focus_safe() != hwnd_tree {
                        crate::set_focus_safe(hwnd_tree);
                    }
                }
                return;
            }

            let hitem = selected_hitem;
            if hwnd_tree.0 == 0 || hitem.0 == 0 {
                return;
            }
            let parent_item = HTREEITEM(
                crate::send_message_w_safe(
                    hwnd_tree,
                    TVM_GETNEXTITEM,
                    WPARAM(TVGN_PARENT as usize),
                    LPARAM(hitem.0),
                )
                .0,
            );
            let key = episode_key(&item);
            let mut source_idx_for_undo: Option<usize> = None;
            if parent.0 != 0 {
                let source_index =
                    with_podcast_state(hwnd, |s| match s.node_data.get(&parent_item.0) {
                        Some(NodeData::Source(idx)) => Some(*idx),
                        _ => None,
                    })
                    .flatten();
                if let Some(source_idx) = source_index {
                    source_idx_for_undo = Some(source_idx);
                    {
                        with_state(parent, |ps| {
                            if let Some(src) = ps.settings.podcast_sources.get_mut(source_idx)
                                && !src.removed_item_keys.iter().any(|k| k == &key)
                            {
                                src.removed_item_keys.push(key.clone());
                                settings::save_settings(ps.settings.clone());
                            }
                        });
                    }
                }
            }
            let mut removed_position: Option<usize> = None;
            let mut focus_child_index: Option<usize> = None;
            with_podcast_state(hwnd, |s| {
                s.node_data.remove(&hitem.0);
                if parent_item.0 != 0
                    && let Some(state) = s.source_items.get_mut(&parent_item.0)
                    && let Some(pos) = state.items.iter().position(|x| episode_key(x) == key)
                {
                    removed_position = Some(pos);
                    state.items.remove(pos);
                    if state.items.is_empty() {
                        focus_child_index = None;
                    } else {
                        focus_child_index = Some(pos.min(state.items.len().saturating_sub(1)));
                    }
                }
            });
            crate::send_message_w_safe(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(hitem.0));
            let mut target = parent_item;
            if parent_item.0 != 0
                && let Some(target_index) = focus_child_index
            {
                let mut child = HTREEITEM(
                    crate::send_message_w_safe(
                        hwnd_tree,
                        TVM_GETNEXTITEM,
                        WPARAM(TVGN_CHILD as usize),
                        LPARAM(parent_item.0),
                    )
                    .0,
                );
                let mut idx = 0usize;
                while child.0 != 0 && idx < target_index {
                    child = HTREEITEM(
                        crate::send_message_w_safe(
                            hwnd_tree,
                            TVM_GETNEXTITEM,
                            WPARAM(TVGN_NEXT as usize),
                            LPARAM(child.0),
                        )
                        .0,
                    );
                    idx += 1;
                }
                if child.0 != 0 {
                    target = child;
                }
            }
            if target.0 != 0 {
                unsafe {
                    SendMessageW(
                        hwnd_tree,
                        TVM_SELECTITEM,
                        WPARAM(TVGN_CARET as usize),
                        LPARAM(target.0),
                    );
                    SendMessageW(hwnd_tree, TVM_ENSUREVISIBLE, WPARAM(0), LPARAM(target.0));
                }
            }
            if let (Some(source_index), Some(position)) = (source_idx_for_undo, removed_position) {
                with_podcast_state(hwnd, |s| {
                    s.removed_history.push(PodcastLastRemoved::Episode {
                        source_index,
                        episode: item.clone(),
                        key,
                        position,
                    });
                });
            }
            announce_status(&i18n::tr(language, "podcasts.episode_removed"));
            if hwnd_tree.0 != 0 && crate::get_focus_safe() != hwnd_tree {
                crate::set_focus_safe(hwnd_tree);
            }
        }
    }
}

fn undo_last_delete(hwnd: HWND) {
    unsafe {
        let Some(last_removed) = with_podcast_state(hwnd, |s| s.removed_history.pop()).flatten()
        else {
            return;
        };

        match last_removed {
            PodcastLastRemoved::Source { index, source } => {
                let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                if parent.0 == 0 {
                    return;
                }
                let restored_index = with_state(parent, |ps| {
                    let insert_at = index.min(ps.settings.podcast_sources.len());
                    ps.settings.podcast_sources.insert(insert_at, source);
                    settings::save_settings(ps.settings.clone());
                    insert_at
                })
                .unwrap_or(index);

                with_podcast_state(hwnd, |s| s.suppress_tree_selection_events = true);
                reload_tree(hwnd);
                with_podcast_state(hwnd, |s| s.suppress_tree_selection_events = false);
                let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                if hwnd_tree.0 != 0 {
                    let target_hitem = with_podcast_state(hwnd, |s| {
                        s.node_data.iter().find_map(|(&h, node)| match node {
                            NodeData::Source(i) if *i == restored_index => Some(HTREEITEM(h)),
                            _ => None,
                        })
                    })
                    .flatten();
                    if let Some(target) = target_hitem {
                        SendMessageW(
                            hwnd_tree,
                            TVM_SELECTITEM,
                            WPARAM(TVGN_CARET as usize),
                            LPARAM(target.0),
                        );
                        SendMessageW(hwnd_tree, TVM_ENSUREVISIBLE, WPARAM(0), LPARAM(target.0));
                    }
                    SetFocus(hwnd_tree);
                }
            }
            PodcastLastRemoved::Episode {
                source_index,
                episode,
                key,
                position,
            } => {
                let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                if parent.0 != 0 {
                    with_state(parent, |ps| {
                        if let Some(src) = ps.settings.podcast_sources.get_mut(source_index) {
                            src.removed_item_keys.retain(|k| k != &key);
                            settings::save_settings(ps.settings.clone());
                        }
                    });
                }

                let source_hitem = with_podcast_state(hwnd, |s| {
                    s.node_data.iter().find_map(|(&h, node)| match node {
                        NodeData::Source(i) if *i == source_index => Some(HTREEITEM(h)),
                        _ => None,
                    })
                })
                .flatten();

                let Some(source_hitem) = source_hitem else {
                    return;
                };

                let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                let mut show_in_tree = false;
                with_podcast_state(hwnd, |s| {
                    if let Some(state) = s.source_items.get_mut(&source_hitem.0) {
                        let insert_at = position.min(state.items.len());
                        state.items.insert(insert_at, episode.clone());
                        show_in_tree = true;
                    }
                });

                if show_in_tree && hwnd_tree.0 != 0 {
                    with_podcast_state(hwnd, |s| s.suppress_tree_selection_events = true);
                    // Rebuild source children to preserve original order after undo.
                    SendMessageW(
                        hwnd_tree,
                        windows::Win32::UI::WindowsAndMessaging::WM_SETREDRAW,
                        WPARAM(0),
                        LPARAM(0),
                    );
                    SendMessageW(
                        hwnd_tree,
                        TVM_SELECTITEM,
                        WPARAM(TVGN_CARET as usize),
                        LPARAM(source_hitem.0),
                    );
                    loop {
                        let child = HTREEITEM(
                            SendMessageW(
                                hwnd_tree,
                                TVM_GETNEXTITEM,
                                WPARAM(TVGN_CHILD as usize),
                                LPARAM(source_hitem.0),
                            )
                            .0,
                        );
                        if child.0 == 0 {
                            break;
                        }
                        with_podcast_state(hwnd, |s| {
                            s.node_data.remove(&child.0);
                        });
                        SendMessageW(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(child.0));
                    }

                    let mut restored_hitem = HTREEITEM(0);
                    let (
                        language,
                        announce_unread,
                        unread_label_position,
                        podcast_date_mode,
                        podcast_time_mode,
                    ) = with_podcast_state(hwnd, |s| {
                        with_state(s.parent, |ps| {
                            (
                                ps.settings.language,
                                ps.settings.announce_unread_rss_podcast_items,
                                ps.settings.rss_podcast_unread_label_position,
                                ps.settings.podcast_episodes_date_display,
                                ps.settings.podcast_episodes_time_display,
                            )
                        })
                        .unwrap_or((
                            s.language,
                            true,
                            crate::settings::RssPodcastUnreadLabelPosition::Before,
                            ListDateDisplayMode::Always,
                            ListTimeDisplayMode::OnlyIfMultipleSameDay,
                        ))
                    })
                    .unwrap_or((
                        Language::English,
                        true,
                        crate::settings::RssPodcastUnreadLabelPosition::Before,
                        ListDateDisplayMode::Always,
                        ListTimeDisplayMode::OnlyIfMultipleSameDay,
                    ));
                    with_podcast_state(hwnd, |s| {
                        if let Some(state) = s.source_items.get(&source_hitem.0) {
                            let day_counts = build_day_counts(&state.items);
                            let title_ctx = PodcastEpisodeTitleContext {
                                language,
                                announce_unread,
                                unread_label_position,
                                date_mode: podcast_date_mode,
                                time_mode: podcast_time_mode,
                            };
                            for entry in &state.items {
                                let item_unplayed =
                                    !state.read_item_keys.contains(&episode_key(entry));
                                let display_title = podcast_episode_display_title(
                                    &entry.title,
                                    item_unplayed,
                                    entry.pub_date,
                                    has_multiple_items_same_day(entry.pub_date, &day_counts),
                                    title_ctx,
                                );
                                let text = to_wide(&display_title);
                                let mut tvis = TVINSERTSTRUCTW {
                                    hParent: source_hitem,
                                    hInsertAfter: windows::Win32::UI::Controls::TVI_LAST,
                                    Anonymous: windows::Win32::UI::Controls::TVINSERTSTRUCTW_0 {
                                        item: TVITEMW {
                                            mask: TVIF_TEXT,
                                            pszText: windows::core::PWSTR(text.as_ptr() as *mut _),
                                            ..Default::default()
                                        },
                                    },
                                };
                                let hchild = HTREEITEM(
                                    SendMessageW(
                                        hwnd_tree,
                                        TVM_INSERTITEMW,
                                        WPARAM(0),
                                        LPARAM(&mut tvis as *mut _ as isize),
                                    )
                                    .0,
                                );
                                if hchild.0 != 0 {
                                    s.node_data.insert(
                                        hchild.0,
                                        NodeData::Episode(Box::new(entry.clone())),
                                    );
                                    if episode_key(entry) == key {
                                        restored_hitem = hchild;
                                    }
                                }
                            }
                        }
                    });
                    SendMessageW(
                        hwnd_tree,
                        windows::Win32::UI::WindowsAndMessaging::WM_SETREDRAW,
                        WPARAM(1),
                        LPARAM(0),
                    );
                    with_podcast_state(hwnd, |s| s.suppress_tree_selection_events = false);
                    if restored_hitem.0 != 0 {
                        SendMessageW(
                            hwnd_tree,
                            TVM_SELECTITEM,
                            WPARAM(TVGN_CARET as usize),
                            LPARAM(restored_hitem.0),
                        );
                        SendMessageW(
                            hwnd_tree,
                            TVM_ENSUREVISIBLE,
                            WPARAM(0),
                            LPARAM(restored_hitem.0),
                        );
                    } else {
                        SendMessageW(
                            hwnd_tree,
                            TVM_SELECTITEM,
                            WPARAM(TVGN_CARET as usize),
                            LPARAM(source_hitem.0),
                        );
                    }
                    SetFocus(hwnd_tree);
                } else if hwnd_tree.0 != 0 {
                    SendMessageW(
                        hwnd_tree,
                        TVM_SELECTITEM,
                        WPARAM(TVGN_CARET as usize),
                        LPARAM(source_hitem.0),
                    );
                    SetFocus(hwnd_tree);
                }
            }
        }
    }
}

struct DescriptionDialogInit {
    title: String,
    content: String,
}

const DESCRIPTION_DIALOG_CLASS: &str = "SonarpadPodcastDescription";
const ID_DESCRIPTION_EDIT: usize = 14001;
const ID_DESCRIPTION_OK: usize = 14002;

fn show_description_dialog(parent: HWND, title: &str, content: &str) {
    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let class_name = to_wide(DESCRIPTION_DIALOG_CLASS);

    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
            crate::load_cursor_w_safe(HINSTANCE(0), IDC_ARROW)
                .unwrap_or_default()
                .0,
        ),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(description_wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    let _atom = crate::register_class_w_safe(&wc);

    let window_title = i18n::tr(
        with_podcast_state(parent, |s| s.language).unwrap_or_default(),
        "podcasts.description_title",
    );

    let init_ptr = Box::into_raw(Box::new(DescriptionDialogInit {
        title: title.to_string(),
        content: content.to_string(),
    }));

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide(&window_title).as_ptr()),
            WS_CAPTION
                | WS_SYSMENU
                | WS_VISIBLE
                | WS_POPUP
                | windows::Win32::UI::WindowsAndMessaging::WS_THICKFRAME
                | windows::Win32::UI::WindowsAndMessaging::WS_MAXIMIZEBOX,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            600,
            450,
            parent,
            None,
            hinstance,
            Some(init_ptr as *const _),
        )
    };

    if hwnd.0 != 0 {
        let main_hwnd = with_podcast_state(parent, |s| s.parent).unwrap_or(HWND(0));
        if main_hwnd.0 != 0 {
            with_state(main_hwnd, |s| s.podcasts_description_dialog = hwnd);
        }
    } else {
        unsafe {
            let _unused_box = Box::from_raw(init_ptr);
        }
    }
}

unsafe extern "system" fn description_control_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "description_control_subclass_proc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || description_control_subclass_proc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn description_control_subclass_proc_inner(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CHAR && wparam.0 as u16 == VK_TAB.0 {
        return LRESULT(0);
    }
    if msg == WM_KEYDOWN {
        let id = crate::get_dlg_ctrl_id_safe(hwnd);
        let parent = crate::get_parent_safe(hwnd);
        let edit = crate::get_dlg_item_safe(parent, ID_DESCRIPTION_EDIT as i32);
        let ok = crate::get_dlg_item_safe(parent, ID_DESCRIPTION_OK as i32);

        if wparam.0 as u16 == VK_TAB.0 {
            let next = if id == ID_DESCRIPTION_EDIT { ok } else { edit };
            if next.0 != 0 {
                crate::set_focus_safe(next);
            }
            return LRESULT(0);
        }
        if wparam.0 as u16 == VK_RETURN.0 && id == ID_DESCRIPTION_OK {
            crate::log_if_err!(crate::destroy_window_safe(parent));
            return LRESULT(0);
        }
        if wparam.0 as u16 == VK_ESCAPE.0 {
            crate::log_if_err!(crate::destroy_window_safe(parent));
            return LRESULT(0);
        }
        // Allow Ctrl+A in edit
        if id == ID_DESCRIPTION_EDIT
            && wparam.0 as u16 == 'A' as u16
            && crate::get_key_state_safe(VK_CONTROL.0 as i32) < 0
        {
            crate::send_message_w_safe(edit, EM_SETSEL, WPARAM(0), LPARAM(-1));
            return LRESULT(0);
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

unsafe extern "system" fn description_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "description_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || description_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn description_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
                let init_ptr = (*cs).lpCreateParams as *mut DescriptionDialogInit;
                if !init_ptr.is_null() {
                    let init = Box::from_raw(init_ptr);
                    let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);

                    let full_text = format!("{}\r\n\r\n{}", init.title, init.content);

                    let edit = CreateWindowExW(
                        WS_EX_CLIENTEDGE,
                        w!("EDIT"),
                        PCWSTR::null(),
                        WS_CHILD
                            | WS_VISIBLE
                            | windows::Win32::UI::WindowsAndMessaging::WS_VSCROLL
                            | WS_TABSTOP
                            | WINDOW_STYLE(
                                (windows::Win32::UI::WindowsAndMessaging::ES_MULTILINE
                                    | windows::Win32::UI::WindowsAndMessaging::ES_READONLY
                                    | windows::Win32::UI::WindowsAndMessaging::ES_AUTOVSCROLL)
                                    as u32,
                            ),
                        0,
                        0,
                        0,
                        0,
                        hwnd,
                        HMENU(ID_DESCRIPTION_EDIT as isize),
                        hinstance,
                        None,
                    );

                    let hfont = HFONT(
                        windows::Win32::Graphics::Gdi::GetStockObject(
                            windows::Win32::Graphics::Gdi::DEFAULT_GUI_FONT,
                        )
                        .0,
                    );
                    SendMessageW(edit, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));

                    let wide_text = to_wide(&full_text);
                    SendMessageW(
                        edit,
                        windows::Win32::UI::WindowsAndMessaging::WM_SETTEXT,
                        WPARAM(0),
                        LPARAM(wide_text.as_ptr() as isize),
                    );

                    let ok_button = CreateWindowExW(
                        Default::default(),
                        w!("BUTTON"),
                        w!("OK"),
                        WS_CHILD
                            | WS_VISIBLE
                            | WS_TABSTOP
                            | WINDOW_STYLE(
                                windows::Win32::UI::WindowsAndMessaging::BS_DEFPUSHBUTTON as u32,
                            ),
                        0,
                        0,
                        0,
                        0,
                        hwnd,
                        HMENU(ID_DESCRIPTION_OK as isize),
                        hinstance,
                        None,
                    );
                    SendMessageW(ok_button, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));

                    // Subclass controls for Tab navigation
                    let proc_ptr = description_control_subclass_proc as *const () as usize;

                    let prev_edit = SetWindowLongPtrW(
                        edit,
                        windows::Win32::UI::WindowsAndMessaging::GWLP_WNDPROC,
                        proc_ptr as isize,
                    );
                    SetWindowLongPtrW(
                        edit,
                        windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                        prev_edit,
                    );

                    let prev_ok = SetWindowLongPtrW(
                        ok_button,
                        windows::Win32::UI::WindowsAndMessaging::GWLP_WNDPROC,
                        proc_ptr as isize,
                    );
                    SetWindowLongPtrW(
                        ok_button,
                        windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                        prev_ok,
                    );

                    SetFocus(edit);
                }
                LRESULT(0)
            }
            WM_SIZE => {
                let mut rect = windows::Win32::Foundation::RECT::default();
                crate::log_if_err!(GetClientRect(hwnd, &mut rect));
                let width = rect.right;
                let height = rect.bottom;
                let button_height = 30;
                let margin = 10;

                let edit_height = if height > (button_height + 2 * margin) {
                    height - button_height - 2 * margin
                } else {
                    height
                };

                let edit = GetDlgItem(hwnd, ID_DESCRIPTION_EDIT as i32);
                if edit.0 != 0 {
                    crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                        edit,
                        0,
                        0,
                        width,
                        edit_height,
                        true
                    ));
                }

                let ok_button = GetDlgItem(hwnd, ID_DESCRIPTION_OK as i32);
                if ok_button.0 != 0 {
                    let btn_width = 80;
                    let btn_x = (width - btn_width) / 2;
                    let btn_y = edit_height + margin;
                    crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                        ok_button,
                        btn_x,
                        btn_y,
                        btn_width,
                        button_height,
                        true
                    ));
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                if id == ID_DESCRIPTION_OK || id == 2 {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_KEYDOWN => {
                if wparam.0 as u16 == VK_ESCAPE.0 {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_DESTROY => {
                let parent = GetParent(hwnd);
                if parent.0 != 0 {
                    let main_hwnd = with_podcast_state(parent, |s| s.parent).unwrap_or(HWND(0));
                    if main_hwnd.0 != 0 {
                        with_state(main_hwnd, |s| s.podcasts_description_dialog = HWND(0));
                    }
                    focus_library(parent);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn apply_reorder_action(
    hwnd: HWND,
    source_index: usize,
    action: ReorderAction,
    target_index: usize,
) -> Option<usize> {
    let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 || parent.0 == 0 {
        return None;
    }
    let mut root_items = collect_root_items(hwnd_tree);
    if source_index >= root_items.len() {
        return None;
    }
    let new_index = {
        with_state(parent, |ps| {
            let moved = match action {
                ReorderAction::Up => settings::move_podcast_feed_up(&mut ps.settings, source_index),
                ReorderAction::Down => {
                    settings::move_podcast_feed_down(&mut ps.settings, source_index)
                }
                ReorderAction::Top => {
                    settings::move_podcast_feed_to_top(&mut ps.settings, source_index)
                }
                ReorderAction::Bottom => {
                    settings::move_podcast_feed_to_bottom(&mut ps.settings, source_index)
                }
                ReorderAction::Position => settings::move_podcast_feed_to_index(
                    &mut ps.settings,
                    source_index,
                    target_index,
                ),
            };
            if moved.is_some() {
                settings::save_settings(ps.settings.clone());
            }
            moved
        })
    }
    .flatten();
    let new_index = new_index?;
    if move_vec_to_index(&mut root_items, source_index, new_index) {
        apply_root_order(hwnd, hwnd_tree, &root_items);
    }
    Some(new_index)
}

fn handle_reorder_action(hwnd: HWND, action: ReorderAction) {
    let Some(source_index) = selected_source_index(hwnd) else {
        return;
    };
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    let language = { with_state(parent, |ps| ps.settings.language) }.unwrap_or_default();
    let total = { with_state(parent, |ps| ps.settings.podcast_sources.len()) }.unwrap_or(0);
    if total == 0 {
        return;
    }
    if matches!(action, ReorderAction::Position) {
        show_reorder_dialog(hwnd, source_index, total);
        return;
    }
    let new_index = match action {
        ReorderAction::Up => apply_reorder_action(hwnd, source_index, action, 0),
        ReorderAction::Down => apply_reorder_action(hwnd, source_index, action, 0),
        ReorderAction::Top => apply_reorder_action(hwnd, source_index, action, 0),
        ReorderAction::Bottom => apply_reorder_action(hwnd, source_index, action, 0),
        ReorderAction::Position => None,
    };
    if let Some(new_index) = new_index
        && new_index != source_index
    {
        let template = i18n::tr(language, "rss.reorder.moved_position");
        let message = template.replace("{x}", &(new_index + 1).to_string());
        announce_status(&message);
    }
}

fn handle_sort_action(hwnd: HWND, order: crate::settings::SortOrder) {
    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    {
        with_state(parent, |ps| {
            crate::settings::sort_podcast_sources(&mut ps.settings, order);
            crate::settings::save_settings(ps.settings.clone());
        });
        reload_tree(hwnd);
    }
}

fn show_reorder_dialog(parent_hwnd: HWND, source_index: usize, total: usize) {
    let existing = with_podcast_state(parent_hwnd, |s| s.reorder_dialog).unwrap_or(HWND(0));
    if existing.0 != 0 {
        crate::set_foreground_window_safe(existing);
        return;
    }
    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let class_name = to_wide(PODCASTS_REORDER_CLASS);
    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
            crate::load_cursor_w_safe(HINSTANCE(0), IDC_ARROW)
                .unwrap_or_default()
                .0,
        ),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(reorder_wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    crate::register_class_w_safe(&wc);

    let language = with_podcast_state(parent_hwnd, |s| s.language).unwrap_or_default();
    let title = i18n::tr(language, "podcasts.context.reorder");
    let init_ptr = Box::into_raw(Box::new(ReorderDialogInit {
        parent: parent_hwnd,
        source_index,
        total,
    }));
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide(&title).as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE | WS_POPUP,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            320,
            140,
            parent_hwnd,
            None,
            hinstance,
            Some(init_ptr as *const _),
        )
    };
    if hwnd.0 == 0 {
        unsafe {
            let _unused_box = Box::from_raw(init_ptr);
        }
        return;
    }
    with_podcast_state(parent_hwnd, |s| s.reorder_dialog = hwnd);
}

unsafe extern "system" fn reorder_control_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "reorder_control_subclass_proc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || reorder_control_subclass_proc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn reorder_control_subclass_proc_inner(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CHAR && wparam.0 as u16 == VK_TAB.0 {
        return LRESULT(0);
    }
    if msg == WM_KEYDOWN {
        let id = crate::get_dlg_ctrl_id_safe(hwnd);
        let parent = crate::get_parent_safe(hwnd);
        let edit = crate::get_dlg_item_safe(parent, REORDER_EDIT_ID as i32);
        let ok = crate::get_dlg_item_safe(parent, REORDER_OK_ID as i32);
        let cancel = crate::get_dlg_item_safe(parent, REORDER_CANCEL_ID as i32);
        if wparam.0 as u16 == VK_TAB.0 {
            let shift = (crate::get_key_state_safe(VK_SHIFT.0 as i32) & 0x8000u16 as i16) != 0;
            let next = if shift {
                if id == REORDER_EDIT_ID {
                    cancel
                } else if id == REORDER_CANCEL_ID {
                    ok
                } else {
                    edit
                }
            } else if id == REORDER_EDIT_ID {
                ok
            } else if id == REORDER_OK_ID {
                cancel
            } else {
                edit
            };
            crate::set_focus_safe(next);
            return LRESULT(0);
        }
        if wparam.0 as u16 == VK_RETURN.0 {
            let target = if id == REORDER_CANCEL_ID {
                REORDER_CANCEL_ID
            } else {
                REORDER_OK_ID
            };
            crate::send_message_w_safe(parent, WM_COMMAND, WPARAM(target), LPARAM(0));
            return LRESULT(0);
        }
        if wparam.0 as u16 == VK_ESCAPE.0 {
            crate::send_message_w_safe(parent, WM_COMMAND, WPARAM(REORDER_CANCEL_ID), LPARAM(0));
            return LRESULT(0);
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

unsafe extern "system" fn reorder_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "reorder_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || reorder_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn reorder_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
                let init_ptr = (*cs).lpCreateParams as *mut ReorderDialogInit;
                if init_ptr.is_null() {
                    return LRESULT(0);
                }
                let init = &*init_ptr;
                let parent = init.parent;
                let source_index = init.source_index;
                let total = init.total;
                SetWindowLongPtrW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                    init_ptr as isize,
                );
                let language = with_podcast_state(parent, |s| s.language).unwrap_or_default();
                let position_template = i18n::tr(language, "rss.reorder.position_of");
                let position_text = position_template
                    .replace("{x}", &(source_index + 1).to_string())
                    .replace("{n}", &total.to_string());
                let move_label = i18n::tr(language, "rss.reorder.move_to_position");
                let ok_label = i18n::tr(language, "rss.dialog.ok");
                let cancel_label = i18n::tr(language, "rss.dialog.cancel");
                let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&position_text).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    10,
                    10,
                    280,
                    16,
                    hwnd,
                    HMENU(1),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&move_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    10,
                    32,
                    280,
                    16,
                    hwnd,
                    HMENU(2),
                    hinstance,
                    None,
                );
                let edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    10,
                    54,
                    280,
                    24,
                    hwnd,
                    HMENU(REORDER_EDIT_ID as isize),
                    hinstance,
                    None,
                );
                let ok = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&ok_label).as_ptr()),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WINDOW_STYLE(
                            windows::Win32::UI::WindowsAndMessaging::BS_DEFPUSHBUTTON as u32,
                        ),
                    130,
                    92,
                    70,
                    24,
                    hwnd,
                    HMENU(REORDER_OK_ID as isize),
                    hinstance,
                    None,
                );
                let cancel = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&cancel_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    210,
                    92,
                    70,
                    24,
                    hwnd,
                    HMENU(REORDER_CANCEL_ID as isize),
                    hinstance,
                    None,
                );
                let proc_ptr = reorder_control_subclass_proc as *const () as usize;
                let prev = SetWindowLongPtrW(
                    edit,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_WNDPROC,
                    proc_ptr as isize,
                );
                SetWindowLongPtrW(
                    edit,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                    prev,
                );
                let proc_ptr = reorder_control_subclass_proc as *const () as usize;
                let prev_ok = SetWindowLongPtrW(
                    ok,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_WNDPROC,
                    proc_ptr as isize,
                );
                SetWindowLongPtrW(
                    ok,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                    prev_ok,
                );
                let proc_ptr = reorder_control_subclass_proc as *const () as usize;
                let prev_cancel = SetWindowLongPtrW(
                    cancel,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_WNDPROC,
                    proc_ptr as isize,
                );
                SetWindowLongPtrW(
                    cancel,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                    prev_cancel,
                );
                let text = format!("{}", source_index + 1);
                if let Err(_e) = SetWindowTextW(edit, PCWSTR(to_wide(&text).as_ptr())) {
                    crate::log_debug(&format!("Error: {:?}", _e));
                }
                SetFocus(edit);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                match id {
                    REORDER_OK_ID | 1 => {
                        let ptr = GetWindowLongPtrW(
                            hwnd,
                            windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                        ) as *mut ReorderDialogInit;
                        if ptr.is_null() {
                            return LRESULT(0);
                        }
                        let init = &*ptr;
                        let edit = GetDlgItem(hwnd, REORDER_EDIT_ID as i32);
                        let len = SendMessageW(
                            edit,
                            windows::Win32::UI::WindowsAndMessaging::WM_GETTEXTLENGTH,
                            WPARAM(0),
                            LPARAM(0),
                        )
                        .0;
                        let mut buf = vec![0u16; len as usize + 1];
                        SendMessageW(
                            edit,
                            windows::Win32::UI::WindowsAndMessaging::WM_GETTEXT,
                            WPARAM(buf.len()),
                            LPARAM(buf.as_mut_ptr() as isize),
                        );
                        let text = String::from_utf16_lossy(&buf[..len as usize]);
                        let language =
                            with_podcast_state(init.parent, |s| s.language).unwrap_or_default();
                        let pos = match text.trim().parse::<usize>() {
                            Ok(v) if v > 0 => v,
                            _ => {
                                let message = i18n::tr(language, "rss.reorder.invalid_position");
                                announce_status(&message);
                                SetFocus(edit);
                                return LRESULT(0);
                            }
                        };
                        let target = pos.clamp(1, init.total) - 1;
                        if let Some(new_index) = apply_reorder_action(
                            init.parent,
                            init.source_index,
                            ReorderAction::Position,
                            target,
                        ) && new_index != init.source_index
                        {
                            let template = i18n::tr(language, "rss.reorder.moved_position");
                            let message = template.replace("{x}", &(new_index + 1).to_string());
                            announce_status(&message);
                        }
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        focus_library(init.parent);
                        LRESULT(0)
                    }
                    REORDER_CANCEL_ID | 2 => {
                        let ptr = GetWindowLongPtrW(
                            hwnd,
                            windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                        ) as *mut ReorderDialogInit;
                        let parent = if ptr.is_null() {
                            HWND(0)
                        } else {
                            (*ptr).parent
                        };
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        if parent.0 != 0 {
                            focus_library(parent);
                        }
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_KEYDOWN => {
                if wparam.0 as u16 == VK_ESCAPE.0 {
                    SendMessageW(hwnd, WM_COMMAND, WPARAM(REORDER_CANCEL_ID), LPARAM(0));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_NCDESTROY => {
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut ReorderDialogInit;
                if !ptr.is_null() {
                    let init = Box::from_raw(ptr);
                    with_podcast_state(init.parent, |s| s.reorder_dialog = HWND(0));
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe extern "system" fn podcast_tree_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "podcast_tree_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || podcast_tree_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn podcast_tree_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_CHAR && wparam.0 as u32 == 26 && crate::get_key_state_safe(VK_CONTROL.0 as i32) < 0
    {
        return LRESULT(0);
    }
    if msg == WM_KEYDOWN
        || msg == windows::Win32::UI::WindowsAndMessaging::WM_SYSKEYDOWN
        || msg == WM_CHAR
    {
        let key = wparam.0 as u32;
        if msg == WM_CHAR && key == VK_RETURN.0 as u32 {
            let parent = crate::get_parent_safe(hwnd);
            if parent.0 != 0
                && let Some(item) = selected_episode(parent)
            {
                let main_hwnd = with_podcast_state(parent, |s| s.parent).unwrap_or(HWND(0));
                open_episode_in_player(parent, main_hwnd, &item);
                return LRESULT(0);
            }
        }
        if key == VK_DELETE.0 as u32 {
            let parent = crate::get_parent_safe(hwnd);
            if parent.0 != 0 {
                if selected_episode(parent).is_some() {
                    handle_episode_action(parent, EpisodeAction::Remove);
                } else {
                    handle_source_action(parent, SourceAction::Remove);
                }
                return LRESULT(0);
            }
        }
        if key == 'Z' as u32 && crate::get_key_state_safe(VK_CONTROL.0 as i32) < 0 {
            let parent = crate::get_parent_safe(hwnd);
            if parent.0 != 0 {
                undo_last_delete(parent);
                return LRESULT(0);
            }
        }
        if key == VK_RIGHT.0 as u32 {
            let parent = crate::get_parent_safe(hwnd);
            if parent.0 != 0
                && let Some(node) = selected_node_data(parent)
                && matches!(node, NodeData::Source(_) | NodeData::PreviewSource(_))
            {
                let hitem = selected_tree_item(parent);
                if hitem.0 != 0 {
                    load_episode_children(parent, hitem, node, false);
                    crate::send_message_w_safe(
                        hwnd,
                        TVM_EXPAND,
                        WPARAM(windows::Win32::UI::Controls::TVE_EXPAND.0 as usize),
                        LPARAM(hitem.0),
                    );
                    return LRESULT(0);
                }
            }
        }
        if key == VK_LEFT.0 as u32 {
            let parent = crate::get_parent_safe(hwnd);
            if parent.0 != 0 {
                let hitem = selected_tree_item(parent);
                if hitem.0 != 0 {
                    let parent_item = HTREEITEM(
                        crate::send_message_w_safe(
                            hwnd,
                            TVM_GETNEXTITEM,
                            WPARAM(TVGN_PARENT as usize),
                            LPARAM(hitem.0),
                        )
                        .0,
                    );
                    if parent_item.0 != 0 {
                        crate::send_message_w_safe(
                            hwnd,
                            TVM_SELECTITEM,
                            WPARAM(TVGN_CARET as usize),
                            LPARAM(parent_item.0),
                        );
                        return LRESULT(0);
                    }
                    if selected_source_index(parent).is_some() {
                        crate::send_message_w_safe(
                            hwnd,
                            TVM_EXPAND,
                            WPARAM(windows::Win32::UI::Controls::TVE_COLLAPSE.0 as usize),
                            LPARAM(hitem.0),
                        );
                        return LRESULT(0);
                    }
                }
            }
        }
        if key == VK_RETURN.0 as u32 {
            let parent = crate::get_parent_safe(hwnd);
            if parent.0 != 0 {
                if crate::get_key_state_safe(VK_MENU.0 as i32) < 0 {
                    show_selected_properties(parent);
                    return LRESULT(0);
                }
                if let Some(item) = selected_episode(parent) {
                    let main_hwnd = with_podcast_state(parent, |s| s.parent).unwrap_or(HWND(0));
                    open_episode_in_player(parent, main_hwnd, &item);
                    return LRESULT(0);
                }
                if let Some(node) = selected_node_data(parent)
                    && matches!(node, NodeData::Source(_) | NodeData::PreviewSource(_))
                {
                    let hitem = selected_tree_item(parent);
                    if hitem.0 != 0 {
                        load_episode_children(parent, hitem, node, false);
                    }
                    return LRESULT(0);
                }
            }
        }
        if key == u32::from(VK_APPS.0)
            || (key == u32::from(VK_F10.0) && crate::get_key_state_safe(VK_SHIFT.0 as i32) < 0)
        {
            let parent = crate::get_parent_safe(hwnd);
            if parent.0 != 0 {
                if let Err(_e) = crate::post_message_w_safe(
                    parent,
                    WM_CONTEXTMENU,
                    WPARAM(hwnd.0 as usize),
                    LPARAM(-1),
                ) {
                    crate::log_debug(&format!("Error: {:?}", _e));
                }
                return LRESULT(0);
            }
        }
    }

    let parent = crate::get_parent_safe(hwnd);
    let prev_proc = if parent.0 != 0 {
        with_podcast_state(parent, |s| s.tree_proc).unwrap_or(None)
    } else {
        None
    };
    if let Some(proc) = prev_proc {
        crate::call_window_proc_w_safe(Some(proc), hwnd, msg, wparam, lparam)
    } else {
        crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
    }
}

unsafe extern "system" fn podcast_search_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "podcast_search_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || podcast_search_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn podcast_search_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_KEYDOWN || msg == windows::Win32::UI::WindowsAndMessaging::WM_SYSKEYDOWN {
        let key = wparam.0 as u32;
        if key == VK_TAB.0 as u32 {
            let parent = crate::get_parent_safe(hwnd);
            if parent.0 != 0 {
                let (
                    hwnd_tree,
                    hwnd_search_button,
                    hwnd_results,
                    hwnd_add,
                    hwnd_import,
                    hwnd_export,
                    hwnd_close,
                ) = with_podcast_state(parent, |s| {
                    (
                        s.hwnd_tree,
                        s.hwnd_search_button,
                        s.hwnd_results,
                        s.hwnd_add,
                        s.hwnd_import,
                        s.hwnd_export,
                        s.hwnd_close,
                    )
                })
                .unwrap_or((
                    HWND(0),
                    HWND(0),
                    HWND(0),
                    HWND(0),
                    HWND(0),
                    HWND(0),
                    HWND(0),
                ));
                let prev = crate::get_key_state_safe(VK_SHIFT.0 as i32) < 0;
                let target = if prev {
                    hwnd_tree
                } else if hwnd_search_button.0 != 0 {
                    hwnd_search_button
                } else if hwnd_results.0 != 0 {
                    hwnd_results
                } else if hwnd_add.0 != 0 {
                    hwnd_add
                } else if hwnd_import.0 != 0 {
                    hwnd_import
                } else if hwnd_export.0 != 0 {
                    hwnd_export
                } else {
                    hwnd_close
                };
                if target.0 != 0 {
                    crate::set_focus_safe(target);
                    return LRESULT(0);
                }
            }
        }
        if key == VK_RETURN.0 as u32 {
            let parent = crate::get_parent_safe(hwnd);
            if parent.0 != 0 {
                trigger_search_from_edit(parent);
            }
            return LRESULT(0);
        }
    }
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_KEYUP
        || msg == windows::Win32::UI::WindowsAndMessaging::WM_SYSKEYUP
    {
        let key = wparam.0 as u32;
        if key == VK_RETURN.0 as u32 {
            let parent = crate::get_parent_safe(hwnd);
            if parent.0 != 0 {
                trigger_search_from_edit(parent);
            }
            return LRESULT(0);
        }
    }
    if msg == WM_CHAR && wparam.0 as u32 == 13 {
        let parent = crate::get_parent_safe(hwnd);
        if parent.0 != 0 {
            trigger_search_from_edit(parent);
        }
        return LRESULT(0);
    }
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_SYSCHAR && wparam.0 as u32 == 13 {
        let parent = crate::get_parent_safe(hwnd);
        if parent.0 != 0 {
            trigger_search_from_edit(parent);
        }
        return LRESULT(0);
    }
    let parent = crate::get_parent_safe(hwnd);
    let prev_proc = if parent.0 != 0 {
        with_podcast_state(parent, |s| s.search_proc).unwrap_or(None)
    } else {
        None
    };
    if let Some(proc) = prev_proc {
        crate::call_window_proc_w_safe(Some(proc), hwnd, msg, wparam, lparam)
    } else {
        crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
    }
}

fn create_controls(hwnd: HWND) {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let hwnd_tree = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("SysTreeView32"),
            PCWSTR::null(),
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | WINDOW_STYLE(
                    windows::Win32::UI::Controls::TVS_HASLINES
                        | windows::Win32::UI::Controls::TVS_HASBUTTONS
                        | windows::Win32::UI::Controls::TVS_LINESATROOT
                        | windows::Win32::UI::Controls::TVS_SHOWSELALWAYS,
                ),
            10,
            10,
            460,
            280,
            hwnd,
            HMENU(ID_TREE as isize),
            hinstance,
            None,
        );
        if hwnd_tree.0 != 0 {
            let proc_ptr = podcast_tree_wndproc as *const () as usize;
            let old = SetWindowLongPtrW(
                hwnd_tree,
                windows::Win32::UI::WindowsAndMessaging::GWLP_WNDPROC,
                proc_ptr as isize,
            );
            with_podcast_state(hwnd, |s| s.tree_proc = crate::isize_to_wndproc_safe(old));
        }

        let hwnd_delete = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            PCWSTR(
                to_wide(&i18n::tr(
                    with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
                    "podcasts.delete_button",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            220,
            300,
            200,
            26,
            hwnd,
            HMENU(ID_DELETE_BUTTON as isize),
            hinstance,
            None,
        );

        let hwnd_search_label = CreateWindowExW(
            Default::default(),
            w!("STATIC"),
            PCWSTR(
                to_wide(&i18n::tr(
                    with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
                    "podcasts.search.label",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE,
            10,
            310,
            460,
            16,
            hwnd,
            HMENU(ID_SEARCH_LABEL as isize),
            hinstance,
            None,
        );

        let hwnd_search = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            10,
            330,
            460,
            24,
            hwnd,
            HMENU(ID_SEARCH_EDIT as isize),
            hinstance,
            None,
        );
        if hwnd_search.0 != 0 {
            let proc_ptr = podcast_search_wndproc as *const () as usize;
            let old = SetWindowLongPtrW(
                hwnd_search,
                windows::Win32::UI::WindowsAndMessaging::GWLP_WNDPROC,
                proc_ptr as isize,
            );
            with_podcast_state(hwnd, |s| s.search_proc = crate::isize_to_wndproc_safe(old));
        }

        let provider_itunes = i18n::tr(
            with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
            "podcasts.search.provider.itunes",
        );
        let provider_podcastindex = i18n::tr(
            with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
            "podcasts.search.provider.podcastindex",
        );
        let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
        let show_search_provider = parent.0 != 0 && podcastindex_credentials_available(parent);
        let hwnd_search_provider = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("COMBOBOX"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            10,
            364,
            220,
            200,
            hwnd,
            HMENU(ID_SEARCH_PROVIDER as isize),
            hinstance,
            None,
        );
        if hwnd_search_provider.0 != 0 {
            let itunes_wide = to_wide(&provider_itunes);
            SendMessageW(
                hwnd_search_provider,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(itunes_wide.as_ptr() as isize),
            );
            if show_search_provider {
                let podcastindex_wide = to_wide(&provider_podcastindex);
                SendMessageW(
                    hwnd_search_provider,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(podcastindex_wide.as_ptr() as isize),
                );
                let provider_index = if matches!(
                    persisted_search_provider(parent),
                    SearchProvider::PodcastIndex
                ) {
                    1
                } else {
                    0
                };
                SendMessageW(
                    hwnd_search_provider,
                    CB_SETCURSEL,
                    WPARAM(provider_index),
                    LPARAM(0),
                );
            } else {
                SendMessageW(hwnd_search_provider, CB_SETCURSEL, WPARAM(0), LPARAM(0));
                EnableWindow(hwnd_search_provider, false);
                crate::show_window_safe(
                    hwnd_search_provider,
                    windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
                );
            }
        }

        let hwnd_search_button = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            PCWSTR(
                to_wide(&i18n::tr(
                    with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
                    "podcasts.search.button",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            10,
            396,
            140,
            26,
            hwnd,
            HMENU(ID_SEARCH_BUTTON as isize),
            hinstance,
            None,
        );

        let hwnd_search_categories = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            PCWSTR(
                to_wide(&i18n::tr(
                    with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
                    "podcasts.categories.browse_button",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            160,
            396,
            200,
            26,
            hwnd,
            HMENU(ID_SEARCH_CATEGORIES_BUTTON as isize),
            hinstance,
            None,
        );

        let hwnd_results = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("LISTBOX"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(LBS_NOTIFY as u32),
            10,
            430,
            460,
            140,
            hwnd,
            HMENU(ID_RESULTS as isize),
            hinstance,
            None,
        );

        let hwnd_add = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            PCWSTR(
                to_wide(&i18n::tr(
                    with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
                    "podcasts.add_button",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            10,
            490,
            200,
            26,
            hwnd,
            HMENU(ID_ADD_BUTTON as isize),
            hinstance,
            None,
        );

        let hwnd_import = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            PCWSTR(
                to_wide(&i18n::tr(
                    with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
                    "podcasts.import_button",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            10,
            526,
            200,
            26,
            hwnd,
            HMENU(ID_IMPORT_BUTTON as isize),
            hinstance,
            None,
        );

        let hwnd_export = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            PCWSTR(
                to_wide(&i18n::tr(
                    with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
                    "podcasts.export_button",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            10,
            562,
            200,
            26,
            hwnd,
            HMENU(ID_EXPORT_BUTTON as isize),
            hinstance,
            None,
        );

        let hwnd_close = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            PCWSTR(
                to_wide(&i18n::tr(
                    with_podcast_state(hwnd, |s| s.language).unwrap_or_default(),
                    "podcasts.close",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            430,
            490,
            200,
            26,
            hwnd,
            HMENU(ID_CLOSE_BUTTON as isize),
            hinstance,
            None,
        );

        with_podcast_state(hwnd, |s| {
            s.hwnd_tree = hwnd_tree;
            s.hwnd_search_label = hwnd_search_label;
            s.hwnd_search = hwnd_search;
            s.hwnd_search_provider = hwnd_search_provider;
            s.hwnd_search_button = hwnd_search_button;
            s.hwnd_search_categories = hwnd_search_categories;
            s.hwnd_results = hwnd_results;
            s.hwnd_add = hwnd_add;
            s.hwnd_import = hwnd_import;
            s.hwnd_export = hwnd_export;
            s.hwnd_delete = hwnd_delete;
            s.hwnd_close = hwnd_close;
        });

        let hfont = HFONT(
            windows::Win32::Graphics::Gdi::GetStockObject(
                windows::Win32::Graphics::Gdi::DEFAULT_GUI_FONT,
            )
            .0,
        );
        for ctrl in [
            hwnd_tree,
            hwnd_search_label,
            hwnd_search,
            hwnd_search_provider,
            hwnd_search_button,
            hwnd_search_categories,
            hwnd_results,
            hwnd_add,
            hwnd_import,
            hwnd_export,
            hwnd_close,
        ] {
            if ctrl.0 != 0 {
                SendMessageW(ctrl, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
            }
        }
    }
}

fn resize_controls(hwnd: HWND) {
    let mut rect = windows::Win32::Foundation::RECT::default();
    if crate::get_client_rect_safe(hwnd, &mut rect).is_err() {
        return;
    }
    let width = (rect.right - rect.left).max(0);
    let height = (rect.bottom - rect.top).max(0);
    let margin = 10;
    let spacing = 8;
    let label_h = 16;
    let search_h = 24;
    let search_button_h = 26;
    let results_h = 140;
    let button_h = 26;
    let button_rows = 3;
    let show_search_provider = {
        let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
        parent.0 != 0 && podcastindex_credentials_available(parent)
    };
    let provider_rows = if show_search_provider { 1 } else { 0 };
    let tree_h = (height
        - margin * 2
        - spacing * 8
        - label_h
        - search_h
        - (search_h * provider_rows)
        - (spacing * provider_rows)
        - search_button_h
        - results_h
        - button_h * button_rows)
        .max(120);
    let controls = with_podcast_state(hwnd, |s| {
        (
            s.hwnd_tree,
            s.hwnd_search_label,
            s.hwnd_search,
            s.hwnd_search_provider,
            s.hwnd_search_button,
            s.hwnd_search_categories,
            s.hwnd_results,
            s.hwnd_add,
            s.hwnd_import,
            s.hwnd_export,
            s.hwnd_close,
        )
    })
    .unwrap_or((
        HWND(0),
        HWND(0),
        HWND(0),
        HWND(0),
        HWND(0),
        HWND(0),
        HWND(0),
        HWND(0),
        HWND(0),
        HWND(0),
        HWND(0),
    ));
    if controls.0 != HWND(0) {
        unsafe {
            crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                controls.0,
                margin,
                margin,
                width - margin * 2,
                tree_h,
                true,
            ));
            let mut y = margin + tree_h + spacing;
            crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                controls.1,
                margin,
                y,
                width - margin * 2,
                label_h,
                true,
            ));
            y += label_h + spacing;
            crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                controls.2,
                margin,
                y,
                width - margin * 2,
                search_h,
                true,
            ));
            y += search_h + spacing;
            if show_search_provider {
                crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                    controls.3,
                    margin,
                    y,
                    width - margin * 2,
                    search_h,
                    true,
                ));
                y += search_h + spacing;
            }
            let button_total_w = (width - margin * 2).max(0);
            let button_w = ((button_total_w - spacing) / 2).max(120);
            crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                controls.4,
                margin,
                y,
                button_w,
                search_button_h,
                true,
            ));
            crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                controls.5,
                margin + button_w + spacing,
                y,
                button_w,
                search_button_h,
                true,
            ));
            y += search_button_h + spacing;
            crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                controls.6,
                margin,
                y,
                width - margin * 2,
                results_h,
                true,
            ));
            y += results_h + spacing;
            crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                controls.7, margin, y, 200, button_h, true,
            ));
            crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                controls.10,
                (width - margin - 200).max(margin),
                y,
                200,
                button_h,
                true,
            ));
            y += button_h + spacing;
            crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                controls.8, margin, y, 200, button_h, true,
            ));
            y += button_h + spacing;
            crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
                controls.9, margin, y, 200, button_h, true,
            ));
        }
    }
}

pub fn open(parent: HWND) {
    unsafe {
        let exists = with_state(parent, |s| s.podcasts_window).unwrap_or(HWND(0));
        if exists.0 != 0 {
            SetForegroundWindow(exists);
            return;
        }

        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(PODCASTS_WINDOW_CLASS);

        let wc = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                windows::Win32::UI::WindowsAndMessaging::LoadCursorW(None, IDC_ARROW)
                    .unwrap_or_default()
                    .0,
            ),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(podcast_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let language = with_state(parent, |s| s.settings.language).unwrap_or_default();
        let title = to_wide(&i18n::tr(language, "podcasts.window.title"));

        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            windows::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT,
            520,
            560,
            parent,
            None,
            hinstance,
            Some(parent.0 as *const _),
        );

        if hwnd.0 != 0 {
            if with_state(parent, |s| s.podcasts_window = hwnd).is_none() {
                crate::log_debug("Failed to set podcasts_window state");
            }
            SetForegroundWindow(hwnd);
        }
    }
}

unsafe extern "system" fn podcast_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "podcast_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || podcast_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn podcast_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
                let parent = HWND((*cs).lpCreateParams as isize);
                let language = with_state(parent, |s| s.settings.language).unwrap_or_default();
                let state = Box::new(PodcastWindowState {
                    parent,
                    language,
                    hwnd_tree: HWND(0),
                    hwnd_search_label: HWND(0),
                    hwnd_search: HWND(0),
                    hwnd_search_provider: HWND(0),
                    hwnd_search_button: HWND(0),
                    hwnd_search_categories: HWND(0),
                    hwnd_results: HWND(0),
                    hwnd_add: HWND(0),
                    hwnd_import: HWND(0),
                    hwnd_export: HWND(0),
                    hwnd_delete: HWND(0),
                    hwnd_close: HWND(0),
                    node_data: HashMap::new(),
                    source_items: HashMap::new(),
                    pending_fetches: HashMap::new(),
                    search_results: Vec::new(),
                    category_results: None,
                    tree_proc: None,
                    search_proc: None,
                    reorder_dialog: HWND(0),
                    last_selected: 0,
                    pending_play: None,
                    download_in_progress: false,
                    last_download_progress_pct: 0,
                    last_download_progress_at: None,
                    preview_sources: Vec::new(),
                    removed_history: Vec::new(),
                    suppress_tree_selection_events: false,
                });
                SetWindowLongPtrW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                    Box::into_raw(state) as isize,
                );
                create_controls(hwnd);
                reload_tree(hwnd);
                update_delete_button_state(hwnd);
                let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                if hwnd_tree.0 != 0 {
                    SetFocus(hwnd_tree);
                }
                start_background_unheard_check(hwnd);
                LRESULT(0)
            }
            WM_SIZE => {
                resize_controls(hwnd);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                focus_library(hwnd);
                LRESULT(0)
            }
            WM_NOTIFY => {
                let nmhdr = &*(lparam.0 as *const windows::Win32::UI::Controls::NMHDR);
                if nmhdr.idFrom == ID_TREE {
                    if nmhdr.code == NM_RETURN {
                        let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                        if let Some(item) = selected_episode(hwnd) {
                            open_episode_in_player(hwnd, parent, &item);
                            return LRESULT(0);
                        }
                    }
                    if nmhdr.code == TVN_KEYDOWN {
                        let key = (lparam.0 as *const NMTVKEYDOWN).as_ref();
                        if let Some(key) = key
                            && key.wVKey == VK_RETURN.0
                        {
                            let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                            if let Some(item) = selected_episode(hwnd) {
                                open_episode_in_player(hwnd, parent, &item);
                                return LRESULT(0);
                            }
                        }
                    }
                    if nmhdr.code == TVN_ITEMEXPANDINGW {
                        let info = &*(lparam.0 as *const windows::Win32::UI::Controls::NMTREEVIEWW);
                        let hitem = info.itemNew.hItem;
                        if let Some(node) =
                            with_podcast_state(hwnd, |s| s.node_data.get(&hitem.0).cloned())
                                .flatten()
                        {
                            match node {
                                NodeData::Source(idx) => {
                                    let has_loaded_items = with_podcast_state(hwnd, |s| {
                                        s.source_items
                                            .get(&hitem.0)
                                            .map(|state| !state.items.is_empty())
                                            .unwrap_or(false)
                                    })
                                    .unwrap_or(false);
                                    if has_loaded_items {
                                        set_source_unheard(hwnd, hitem, false);
                                    }
                                    load_episode_children(hwnd, hitem, NodeData::Source(idx), false)
                                }
                                NodeData::PreviewSource(idx) => load_episode_children(
                                    hwnd,
                                    hitem,
                                    NodeData::PreviewSource(idx),
                                    false,
                                ),
                                _ => {}
                            }
                        }
                    }
                    if nmhdr.code == TVN_SELCHANGEDW {
                        if with_podcast_state(hwnd, |s| s.suppress_tree_selection_events)
                            .unwrap_or(false)
                        {
                            return LRESULT(0);
                        }
                        let info = &*(lparam.0 as *const windows::Win32::UI::Controls::NMTREEVIEWW);
                        with_podcast_state(hwnd, |s| s.last_selected = info.itemNew.hItem.0);
                        update_delete_button_state(hwnd);
                    }
                }
                LRESULT(0)
            }
            WM_CONTEXTMENU => {
                let x = (lparam.0 & 0xffff) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
                show_context_menu(hwnd, x, y, false);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let code = ((wparam.0 >> 16) & 0xffff) as u16;
                match id {
                    ID_ADD_BUTTON => {
                        show_add_dialog(hwnd);
                        LRESULT(0)
                    }
                    ID_IMPORT_BUTTON => {
                        handle_import_opml(hwnd);
                        LRESULT(0)
                    }
                    ID_EXPORT_BUTTON => {
                        handle_export_opml(hwnd);
                        LRESULT(0)
                    }
                    ID_CLOSE_BUTTON | 2 => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    ID_SEARCH_BUTTON => {
                        if code == windows::Win32::UI::WindowsAndMessaging::BN_CLICKED as u16 {
                            trigger_search_from_edit(hwnd);
                            return LRESULT(0);
                        }
                        LRESULT(0)
                    }
                    ID_SEARCH_PROVIDER => {
                        if code == windows::Win32::UI::WindowsAndMessaging::CBN_SETFOCUS as u16 {
                            let language =
                                with_podcast_state(hwnd, |s| s.language).unwrap_or_default();
                            announce_status(&i18n::tr(
                                language,
                                "podcasts.categories.source.label",
                            ));
                            return LRESULT(0);
                        }
                        if code == windows::Win32::UI::WindowsAndMessaging::CBN_SELCHANGE as u16 {
                            let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                            if parent.0 != 0 {
                                save_search_provider(parent, selected_search_provider(hwnd));
                            }
                            return LRESULT(0);
                        }
                        LRESULT(0)
                    }
                    ID_SEARCH_CATEGORIES_BUTTON => {
                        if code == windows::Win32::UI::WindowsAndMessaging::BN_CLICKED as u16 {
                            show_categories_dialog(hwnd);
                            return LRESULT(0);
                        }
                        LRESULT(0)
                    }
                    ID_RESULTS => {
                        if code == LBN_DBLCLK as u16 {
                            subscribe_selected_result(hwnd);
                            return LRESULT(0);
                        }
                        LRESULT(0)
                    }
                    ID_DELETE_BUTTON => {
                        handle_source_action(hwnd, SourceAction::Remove);
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_KEYDOWN => {
                let focus = GetFocus();
                let (
                    hwnd_tree,
                    hwnd_search,
                    hwnd_search_provider,
                    hwnd_results,
                    hwnd_search_button,
                ) = with_podcast_state(hwnd, |s| {
                    (
                        s.hwnd_tree,
                        s.hwnd_search,
                        s.hwnd_search_provider,
                        s.hwnd_results,
                        s.hwnd_search_button,
                    )
                })
                .unwrap_or((HWND(0), HWND(0), HWND(0), HWND(0), HWND(0)));
                let key = wparam.0 as u32;
                if (focus == hwnd_search || focus == hwnd_search_provider)
                    && key == VK_RETURN.0 as u32
                {
                    if hwnd_search_button.0 != 0 {
                        SendMessageW(
                            hwnd_search_button,
                            windows::Win32::UI::WindowsAndMessaging::BM_CLICK,
                            WPARAM(0),
                            LPARAM(0),
                        );
                    } else {
                        trigger_search_from_edit(hwnd);
                    }
                    return LRESULT(0);
                }
                if focus == hwnd_results && key == VK_RETURN.0 as u32 {
                    subscribe_selected_result(hwnd);
                    return LRESULT(0);
                }
                if focus == hwnd_tree && key == 'Z' as u32 && GetKeyState(VK_CONTROL.0 as i32) < 0 {
                    undo_last_delete(hwnd);
                    return LRESULT(0);
                }
                if focus == hwnd_tree
                    && key == VK_RETURN.0 as u32
                    && GetKeyState(VK_MENU.0 as i32) < 0
                {
                    show_selected_properties(hwnd);
                    return LRESULT(0);
                }
                if focus == hwnd_tree && key == VK_RETURN.0 as u32 {
                    if let Some(item) = selected_episode(hwnd) {
                        let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                        open_episode_in_player(hwnd, parent, &item);
                        return LRESULT(0);
                    }
                    if let Some(node) = selected_node_data(hwnd) {
                        match node {
                            NodeData::Source(_) | NodeData::PreviewSource(_) => {
                                let hitem = selected_tree_item(hwnd);
                                if hitem.0 != 0 {
                                    load_episode_children(hwnd, hitem, node, false);
                                }
                                return LRESULT(0);
                            }
                            _ => {}
                        }
                    }
                }
                if focus == hwnd_tree
                    && key == VK_RIGHT.0 as u32
                    && let Some(node) = selected_node_data(hwnd)
                {
                    match node {
                        NodeData::Source(_) | NodeData::PreviewSource(_) => {
                            let hitem = selected_tree_item(hwnd);
                            if hitem.0 != 0 {
                                load_episode_children(hwnd, hitem, node, false);
                                SendMessageW(
                                    hwnd_tree,
                                    TVM_EXPAND,
                                    WPARAM(windows::Win32::UI::Controls::TVE_EXPAND.0 as usize),
                                    LPARAM(hitem.0),
                                );
                            }
                        }
                        _ => {}
                    }
                    return LRESULT(0);
                }
                if focus == hwnd_tree && key == VK_LEFT.0 as u32 {
                    let hitem = selected_tree_item(hwnd);
                    if hitem.0 != 0 {
                        let parent_item = HTREEITEM(
                            SendMessageW(
                                hwnd_tree,
                                TVM_GETNEXTITEM,
                                WPARAM(TVGN_PARENT as usize),
                                LPARAM(hitem.0),
                            )
                            .0,
                        );
                        if parent_item.0 != 0 {
                            SendMessageW(
                                hwnd_tree,
                                TVM_SELECTITEM,
                                WPARAM(TVGN_CARET as usize),
                                LPARAM(parent_item.0),
                            );
                            return LRESULT(0);
                        }
                        if selected_source_index(hwnd).is_some() {
                            SendMessageW(
                                hwnd_tree,
                                TVM_EXPAND,
                                WPARAM(windows::Win32::UI::Controls::TVE_COLLAPSE.0 as usize),
                                LPARAM(hitem.0),
                            );
                            return LRESULT(0);
                        }
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_COPYDATA => {
                let cds_ptr = lparam.0 as *const COPYDATASTRUCT;
                if !cds_ptr.is_null() && (*cds_ptr).dwData == PODCAST_ADD_COPYDATA {
                    let Some(url) =
                        crate::copydata_utf16_payload(cds_ptr, "podcasts WM_COPYDATA add source")
                    else {
                        return LRESULT(0);
                    };
                    let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                    if let Some(index) = add_podcast_source(parent, &url, "") {
                        let language =
                            with_state(parent, |s| s.settings.language).unwrap_or_default();
                        announce_status(&i18n::tr(language, "podcasts.added"));
                        let hwnd_tree =
                            with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                        if hwnd_tree.0 != 0 {
                            let display = with_state(parent, |ps| {
                                ps.settings.podcast_sources.get(index).map(|src| {
                                    podcast_source_display_title(
                                        src,
                                        language,
                                        ps.settings.announce_unread_rss_podcast_items,
                                        ps.settings.rss_podcast_unread_label_position,
                                    )
                                })
                            })
                            .flatten()
                            .unwrap_or_else(|| url.clone());
                            let hitem = create_tree_item(hwnd_tree, &display, index);
                            if hitem.0 != 0 {
                                with_podcast_state(hwnd, |s| {
                                    s.node_data.insert(hitem.0, NodeData::Source(index));
                                });
                                SendMessageW(
                                    hwnd_tree,
                                    TVM_SELECTITEM,
                                    WPARAM(TVGN_CARET as usize),
                                    LPARAM(hitem.0),
                                );
                                SendMessageW(
                                    hwnd_tree,
                                    TVM_ENSUREVISIBLE,
                                    WPARAM(0),
                                    LPARAM(hitem.0),
                                );
                                SetForegroundWindow(hwnd);
                                SetFocus(hwnd_tree);
                                SendMessageW(hwnd_tree, WM_SETFOCUS, WPARAM(0), LPARAM(0));
                                load_episode_children(hwnd, hitem, NodeData::Source(index), false);
                            }
                        }
                    } else {
                        let normalized = crate::tools::rss::normalize_url(&url);
                        let existing_idx = with_state(parent, |s| {
                            s.settings.podcast_sources.iter().position(|src| {
                                crate::tools::rss::normalize_url(&src.url) == normalized
                            })
                        })
                        .flatten();

                        if let Some(idx) = existing_idx {
                            let hitem = with_podcast_state(hwnd, |s| {
                                s.node_data.iter().find_map(|(h, data)| {
                                    if let NodeData::Source(i) = data {
                                        if *i == idx { Some(*h) } else { None }
                                    } else {
                                        None
                                    }
                                })
                            })
                            .flatten();

                            if let Some(hitem) = hitem {
                                let hwnd_tree =
                                    with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                                if hwnd_tree.0 != 0 {
                                    SendMessageW(
                                        hwnd_tree,
                                        TVM_SELECTITEM,
                                        WPARAM(TVGN_CARET as usize),
                                        LPARAM(hitem),
                                    );
                                    load_episode_children(
                                        hwnd,
                                        HTREEITEM(hitem),
                                        NodeData::Source(idx),
                                        true,
                                    );
                                }
                            }
                        }
                    }
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_PODCAST_FETCH_COMPLETE => {
                let msg_ptr = lparam.0 as *mut FetchResult;
                let msg = Box::from_raw(msg_ptr);
                let had_loaded_items = matches!(msg.node, NodeData::Source(_))
                    && with_podcast_state(hwnd, |s| {
                        s.source_items
                            .get(&msg.hitem)
                            .map(|state| !state.items.is_empty())
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                with_podcast_state(hwnd, |s| {
                    if let Some(src) = match msg.node {
                        NodeData::Source(idx) => with_state(s.parent, |ps| {
                            ps.settings.podcast_sources.get(idx).map(|s| s.url.clone())
                        })
                        .flatten(),
                        NodeData::PreviewSource(idx) => {
                            s.preview_sources.get(idx).map(|s| s.url.clone())
                        }
                        _ => None,
                    } {
                        s.pending_fetches.remove(&src);
                    }
                });
                match msg.result {
                    Ok(outcome) => {
                        match msg.node {
                            NodeData::Source(idx) => {
                                let parent =
                                    with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                                if parent.0 != 0 {
                                    // Compute the most recent episode pub_date for sorting
                                    let max_pub_date =
                                        outcome.items.iter().filter_map(|ep| ep.pub_date).max();
                                    update_source_cache(parent, idx, outcome.cache, max_pub_date);
                                    if !outcome.title.trim().is_empty() {
                                        update_source_title(
                                            hwnd,
                                            HTREEITEM(msg.hitem),
                                            idx,
                                            &outcome.title,
                                        );
                                    }
                                }
                            }
                            NodeData::PreviewSource(idx) => {
                                let title = outcome.title.clone();
                                with_podcast_state(hwnd, |s| {
                                    if let Some(src) = s.preview_sources.get_mut(idx) {
                                        src.cache = outcome.cache;
                                        if !title.trim().is_empty() {
                                            src.title = title.clone();
                                        }
                                    }
                                });
                                if !title.trim().is_empty() {
                                    let title_wide = to_wide(&title);
                                    let mut tvi = TVITEMW {
                                        mask: TVIF_TEXT,
                                        hItem: HTREEITEM(msg.hitem),
                                        pszText: windows::core::PWSTR(title_wide.as_ptr() as *mut _),
                                        ..Default::default()
                                    };
                                    SendMessageW(
                                        hwnd,
                                        TVM_SETITEMW,
                                        WPARAM(0),
                                        LPARAM(&mut tvi as *mut _ as isize),
                                    );
                                }
                            }
                            _ => {}
                        }
                        let has_items = !outcome.items.is_empty();
                        let appended =
                            apply_episode_results(hwnd, HTREEITEM(msg.hitem), outcome.items);
                        if matches!(msg.node, NodeData::Source(_)) {
                            if had_loaded_items {
                                if appended > 0 {
                                    set_source_unheard(hwnd, HTREEITEM(msg.hitem), true);
                                }
                            } else if has_items {
                                // First load: user expanded the feed, mark as heard
                                set_source_unheard(hwnd, HTREEITEM(msg.hitem), false);
                            }
                        }
                        prune_persisted_played_keys_for_source(hwnd, HTREEITEM(msg.hitem));
                    }
                    Err(_e) => {
                        let hwnd_tree =
                            with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                        if hwnd_tree.0 != 0 {
                            let child = HTREEITEM(
                                SendMessageW(
                                    hwnd_tree,
                                    TVM_GETNEXTITEM,
                                    WPARAM(TVGN_CHILD as usize),
                                    LPARAM(msg.hitem),
                                )
                                .0,
                            );
                            if child.0 != 0 {
                                SendMessageW(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(child.0));
                            }
                        }
                    }
                }
                LRESULT(0)
            }
            WM_PODCAST_BACKGROUND_CHECK_COMPLETE => {
                let ptr = lparam.0 as *mut BackgroundCheckResult;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let msg = Box::from_raw(ptr);
                process_background_check_result(hwnd, *msg);
                LRESULT(0)
            }
            WM_PODCAST_MARK_EPISODE_PLAYED_UI => {
                let ptr = lparam.0 as *mut MarkEpisodePlayedUiMessage;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let msg = Box::from_raw(ptr);
                let hitem = HTREEITEM(msg.hitem);
                let episode = with_podcast_state(hwnd, |s| match s.node_data.get(&hitem.0) {
                    Some(NodeData::Episode(item)) => Some((**item).clone()),
                    _ => None,
                })
                .flatten();
                if let Some(episode) = episode
                    && episode_key(&episode) == msg.item_key
                {
                    let should_skip_ui_update = with_podcast_state(hwnd, |s| {
                        s.download_in_progress
                            || s.pending_play.as_deref() == Some(msg.item_key.as_str())
                    })
                    .unwrap_or(false);
                    if should_skip_ui_update {
                        if msg.retries_left > 0 {
                            log_debug(&format!(
                                "podcasts_mark_played_ui_retry key={} retries_left={}",
                                msg.item_key, msg.retries_left
                            ));
                            post_mark_episode_played_ui_after_delay(
                                hwnd,
                                MarkEpisodePlayedUiMessage {
                                    hitem: msg.hitem,
                                    item_key: msg.item_key.clone(),
                                    retries_left: msg.retries_left.saturating_sub(1),
                                },
                                700,
                            );
                        } else {
                            log_debug(&format!(
                                "podcasts_mark_played_ui_giveup key={}",
                                msg.item_key
                            ));
                        }
                        return LRESULT(0);
                    }
                    let (
                        language,
                        announce_unread,
                        unread_label_position,
                        podcast_date_mode,
                        podcast_time_mode,
                    ) = with_podcast_state(hwnd, |s| {
                        with_state(s.parent, |ps| {
                            (
                                ps.settings.language,
                                ps.settings.announce_unread_rss_podcast_items,
                                ps.settings.rss_podcast_unread_label_position,
                                ps.settings.podcast_episodes_date_display,
                                ps.settings.podcast_episodes_time_display,
                            )
                        })
                        .unwrap_or((
                            Language::English,
                            true,
                            crate::settings::RssPodcastUnreadLabelPosition::Before,
                            ListDateDisplayMode::Always,
                            ListTimeDisplayMode::OnlyIfMultipleSameDay,
                        ))
                    })
                    .unwrap_or((
                        Language::English,
                        true,
                        crate::settings::RssPodcastUnreadLabelPosition::Before,
                        ListDateDisplayMode::Always,
                        ListTimeDisplayMode::OnlyIfMultipleSameDay,
                    ));
                    let day_counts = with_podcast_state(hwnd, |s| {
                        s.source_items
                            .values()
                            .find(|state| {
                                state.items.iter().any(|x| episode_key(x) == msg.item_key)
                            })
                            .map(|state| build_day_counts(&state.items))
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                    let title_ctx = PodcastEpisodeTitleContext {
                        language,
                        announce_unread,
                        unread_label_position,
                        date_mode: podcast_date_mode,
                        time_mode: podcast_time_mode,
                    };
                    let display_title = podcast_episode_display_title(
                        &episode.title,
                        false,
                        episode.pub_date,
                        has_multiple_items_same_day(episode.pub_date, &day_counts),
                        title_ctx,
                    );
                    let text = to_wide(&display_title);
                    let mut tvis = TVITEMW {
                        mask: TVIF_TEXT,
                        hItem: hitem,
                        pszText: windows::core::PWSTR(text.as_ptr() as *mut _),
                        cchTextMax: text.len() as i32,
                        ..Default::default()
                    };
                    let hwnd_tree = with_podcast_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                    if hwnd_tree.0 != 0 {
                        SendMessageW(
                            hwnd_tree,
                            TVM_SETITEMW,
                            WPARAM(0),
                            LPARAM(&mut tvis as *mut _ as isize),
                        );
                    }
                }
                LRESULT(0)
            }
            WM_PODCAST_SEARCH_COMPLETE => {
                let ptr = lparam.0 as *mut SearchResultMsg;
                if ptr.is_null() {
                    crate::log_debug(
                        "podcasts_categories WM_PODCAST_SEARCH_COMPLETE received with null payload",
                    );
                    return LRESULT(0);
                }
                let msg = Box::from_raw(ptr);
                crate::log_debug(&format!(
                    "podcasts_categories WM_PODCAST_SEARCH_COMPLETE: error_present={} result_count={} status_present={}",
                    msg.error.is_some(),
                    msg.results.len(),
                    msg.status.is_some()
                ));
                if let Some(error) = msg.error.as_deref() {
                    let language = with_podcast_state(hwnd, |s| s.language).unwrap_or_default();
                    let title = i18n::tr(language, "app.error_title");
                    MessageBoxW(
                        hwnd,
                        PCWSTR(to_wide(error).as_ptr()),
                        PCWSTR(to_wide(&title).as_ptr()),
                        MB_OK | MB_ICONINFORMATION,
                    );
                    announce_status(error);
                }
                update_search_results(
                    hwnd,
                    msg.results,
                    msg.status.as_deref(),
                    msg.category_results,
                );
                LRESULT(0)
            }
            WM_PODCAST_PLAY_READY => {
                let ptr = lparam.0 as *mut PlayReadyMsg;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let msg = Box::from_raw(ptr);
                let (language, announce_download_completed) = with_podcast_state(hwnd, |s| {
                    let language = s.language;
                    let announce = s.download_in_progress;
                    s.download_in_progress = false;
                    s.last_download_progress_pct = 0;
                    s.last_download_progress_at = None;
                    s.pending_play = None;
                    (language, announce)
                })
                .unwrap_or((Language::default(), false));
                if announce_download_completed {
                    announce_status(&i18n::tr(language, "podcasts.download_completed"));
                }
                let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                mark_podcast_episode_played(&msg.path);
                if parent.0 != 0 && !msg.item_key.trim().is_empty() {
                    // Ensure UI "played" state is applied when playback is actually ready.
                    mark_episode_played_with_ui_delay(hwnd, parent, msg.item_key.clone(), 150, 8);
                }
                editor_manager::open_document(parent, &msg.path);
                if let Some(title) = crate::podcast_episode_display_title(
                    msg.podcast_title.as_deref(),
                    Some(msg.title.as_str()),
                ) {
                    editor_manager::set_current_document_title(parent, &title);
                }
                if parent.0 != 0 {
                    if !msg.item_key.trim().is_empty() {
                        cache_embedded_podcast_chapters_if_missing(
                            parent,
                            &msg.item_key,
                            &msg.path,
                            msg.chapters_prefetch_started,
                        );
                    }
                    editor_manager::mark_current_document_from_rss(parent, true);
                    crate::set_active_podcast_episode_info(
                        parent,
                        Some(msg.enclosure_url.clone()),
                        None,
                        msg.podcast_title.clone(),
                        Some(msg.title.clone()),
                        Some(msg.path.clone()),
                    );
                    crate::menu::update_playback_menu(parent, true);
                    crate::activate_pending_podcast_chapters(parent);
                }
                if parent.0 != 0 {
                    SetForegroundWindow(parent);
                    if let Some(hwnd_tab) = with_state(parent, |s| s.hwnd_tab)
                        && hwnd_tab.0 != 0
                    {
                        SetFocus(hwnd_tab);
                    }
                }
                LRESULT(0)
            }
            WM_PODCAST_PLAY_FAILED => {
                with_podcast_state(hwnd, |s| {
                    s.pending_play = None;
                    s.download_in_progress = false;
                    s.last_download_progress_pct = 0;
                    s.last_download_progress_at = None;
                });
                let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                if parent.0 != 0 {
                    crate::set_pending_podcast_chapters_key(parent, None);
                }
                LRESULT(0)
            }
            WM_PODCAST_DOWNLOAD_PROGRESS => {
                let pct = wparam.0 as u32;
                let now = Instant::now();
                let (language, should_announce) = with_podcast_state(hwnd, |s| {
                    let language = s.language;
                    let should = if pct == 0 {
                        s.last_download_progress_pct == 0 && s.last_download_progress_at.is_none()
                    } else if pct >= 100 {
                        s.last_download_progress_pct < 100
                    } else if pct > s.last_download_progress_pct {
                        s.last_download_progress_at
                            .map(|t| now.saturating_duration_since(t) >= Duration::from_millis(600))
                            .unwrap_or(true)
                    } else {
                        false
                    };
                    if should {
                        s.last_download_progress_pct = pct.min(100);
                        s.last_download_progress_at = Some(now);
                    }
                    (language, should)
                })
                .unwrap_or((Language::default(), true));
                if !should_announce {
                    return LRESULT(0);
                }
                let message = crate::i18n::tr_f(
                    language,
                    "podcasts.download_progress",
                    &[("pct", &pct.to_string())],
                );
                announce_status(&message);
                LRESULT(0)
            }
            WM_PODCAST_DOWNLOAD_HEARTBEAT => {
                let now = Instant::now();
                let (language, should_announce) = with_podcast_state(hwnd, |s| {
                    if !s.download_in_progress {
                        return (s.language, false);
                    }
                    let should = s
                        .last_download_progress_at
                        .map(|t| now.saturating_duration_since(t) >= Duration::from_secs(4))
                        .unwrap_or(true);
                    if should {
                        s.last_download_progress_at = Some(now);
                    }
                    (s.language, should)
                })
                .unwrap_or((Language::default(), false));
                if !should_announce {
                    return LRESULT(0);
                }
                let mut message = crate::i18n::tr(language, "podcasts.download_progress")
                    .replace("{pct}%", "")
                    .replace("{pct}", "")
                    .trim()
                    .trim_end_matches([':', '-', '.', '…', ' '])
                    .to_string();
                if message.is_empty() {
                    message = crate::i18n::tr(language, "podcasts.loading");
                }
                announce_status(&message);
                LRESULT(0)
            }
            WM_DESTROY => {
                let parent = with_podcast_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                if parent.0 != 0 {
                    if with_state(parent, |s| s.podcasts_window = HWND(0)).is_none() {
                        crate::log_debug("Failed to reset podcasts_window state");
                    }
                    // Only focus editor if not in player mode (audiobook)
                    if !crate::editor_manager::is_current_audiobook(parent) {
                        force_focus_editor_on_parent(parent);
                    }
                }
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut PodcastWindowState;
                if !ptr.is_null() {
                    let _unused_box = Box::from_raw(ptr);
                }
                LRESULT(0)
            }
            WM_NCDESTROY => DefWindowProcW(hwnd, msg, wparam, lparam),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn with_podcast_state<R>(hwnd: HWND, f: impl FnOnce(&mut PodcastWindowState) -> R) -> Option<R> {
    let ptr = crate::get_window_long_ptr_w_safe(
        hwnd,
        windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
    ) as *mut PodcastWindowState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn with_category_dialog_state<R>(
    hwnd: HWND,
    f: impl FnOnce(&mut CategoryDialogState) -> R,
) -> Option<R> {
    let ptr = crate::get_window_long_ptr_w_safe(
        hwnd,
        windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
    ) as *mut CategoryDialogState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn percent_encode(input: &str) -> String {
    use url::form_urlencoded::byte_serialize;
    byte_serialize(input.as_bytes()).collect()
}

const APPLE_LIMIT: u32 = 50;

pub(crate) fn podcast_directory_country_options() -> &'static [(&'static str, &'static str)] {
    PODCAST_DIRECTORY_COUNTRIES
}

fn system_podcast_country() -> String {
    let mut buffer = [0u16; 85];
    let len = crate::get_user_default_locale_name_safe(&mut buffer);
    if len > 1 {
        let locale = String::from_utf16_lossy(&buffer[..len.saturating_sub(1) as usize]);
        for segment in locale.split(['-', '_']).rev() {
            let trimmed = segment.trim();
            if trimmed.len() == 2 && trimmed.chars().all(|ch| ch.is_ascii_alphabetic()) {
                return trimmed.to_ascii_lowercase();
            }
        }
    }
    "us".to_string()
}

fn normalize_podcast_country_code(country: &str) -> Option<&'static str> {
    let normalized = country.trim().to_ascii_lowercase();
    PODCAST_DIRECTORY_COUNTRIES
        .iter()
        .find_map(|(code, _)| (*code == normalized).then_some(*code))
}

fn apple_country_from_settings(settings: &settings::AppSettings) -> String {
    if let Some(country) = normalize_podcast_country_code(&settings.podcast_directory_country) {
        return country.to_string();
    }
    system_podcast_country()
}

fn apple_country_for_parent(parent: HWND) -> String {
    let settings = with_state(parent, |state| state.settings.clone()).unwrap_or_default();
    apple_country_from_settings(&settings)
}

fn podcastindex_language_code(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::Ukrainian
        | Language::Lithuanian
        | Language::Russian
        | Language::Chinese
        | Language::English => "en",
        Language::German => "de",
        Language::Hindi => "hi",
        Language::Spanish => "es",
        Language::Portuguese | Language::PortugueseBrazilian => "pt",
        Language::Swedish => "sv",
        Language::Vietnamese => "vi",
        Language::Czech => "cs",
        Language::Polish => "pl",
        Language::French => "fr",
        Language::Serbian => "sr",
    }
}

fn apple_categories(language: Language) -> Vec<Category> {
    // All 19 official Apple Podcasts categories with verified genre IDs
    // Source: https://podcasters.apple.com/support/1691-apple-podcasts-categories
    let (
        arts,
        business,
        comedy,
        education,
        fiction,
        government,
        health_fitness,
        history,
        kids_family,
        leisure,
        music,
        news,
        religion_spirituality,
        science,
        society_culture,
        sports,
        technology,
        true_crime,
        tv_film,
    ) = match language {
        Language::Italian => (
            "Arti",
            "Affari",
            "Commedia",
            "Istruzione",
            "Narrativa",
            "Governo",
            "Salute e fitness",
            "Storia",
            "Bambini e famiglia",
            "Tempo libero",
            "Musica",
            "Notizie",
            "Religione e spiritualita",
            "Scienza",
            "Societa e cultura",
            "Sport",
            "Tecnologia",
            "True crime",
            "TV e film",
        ),
        Language::German => (
            "Kunst",
            "Wirtschaft",
            "Comedy",
            "Bildung",
            "Fiktion",
            "Regierung",
            "Gesundheit und Fitness",
            "Geschichte",
            "Kinder und Familie",
            "Freizeit",
            "Musik",
            "Nachrichten",
            "Religion und Spiritualität",
            "Wissenschaft",
            "Gesellschaft und Kultur",
            "Sport",
            "Technologie",
            "True Crime",
            "TV und Film",
        ),
        Language::Ukrainian
        | Language::Lithuanian
        | Language::Russian
        | Language::Chinese
        | Language::English
        | Language::Hindi => (
            "Arts",
            "Business",
            "Comedy",
            "Education",
            "Fiction",
            "Government",
            "Health & Fitness",
            "History",
            "Kids & Family",
            "Leisure",
            "Music",
            "News",
            "Religion & Spirituality",
            "Science",
            "Society & Culture",
            "Sports",
            "Technology",
            "True Crime",
            "TV & Film",
        ),
        Language::Serbian => (
            "Umetnost",
            "Biznis",
            "Komedija",
            "Obrazovanje",
            "Fikcija",
            "Vlada",
            "Zdravlje i fitnes",
            "Istorija",
            "Deca i porodica",
            "Slobodno vreme",
            "Muzika",
            "Vesti",
            "Religija i duhovnost",
            "Nauka",
            "Drustvo i kultura",
            "Sport",
            "Tehnologija",
            "True Crime",
            "TV i film",
        ),
        Language::Spanish => (
            "Arte",
            "Negocios",
            "Comedia",
            "Educacion",
            "Ficcion",
            "Gobierno",
            "Salud y fitness",
            "Historia",
            "Ninos y familia",
            "Ocio",
            "Musica",
            "Noticias",
            "Religion y espiritualidad",
            "Ciencia",
            "Sociedad y cultura",
            "Deportes",
            "Tecnologia",
            "True crime",
            "TV y cine",
        ),
        Language::Portuguese => (
            "Artes",
            "Negocios",
            "Comedia",
            "Educacao",
            "Ficcao",
            "Governo",
            "Saude e fitness",
            "Historia",
            "Criancas e familia",
            "Lazer",
            "Musica",
            "Noticias",
            "Religiao e espiritualidade",
            "Ciencia",
            "Sociedade e cultura",
            "Desporto",
            "Tecnologia",
            "True crime",
            "TV e cinema",
        ),
        Language::PortugueseBrazilian => (
            "Artes",
            "Negócios",
            "Comédia",
            "Educação",
            "Ficção",
            "Governo",
            "Saúde e fitness",
            "História",
            "Crianças e família",
            "Lazer",
            "Música",
            "Notícias",
            "Religião e espiritualidade",
            "Ciência",
            "Sociedade e cultura",
            "Esportes",
            "Tecnologia",
            "Crimes reais",
            "TV e filmes",
        ),
        Language::Swedish => (
            "Konst",
            "Foretag",
            "Komedi",
            "Utbildning",
            "Fiktion",
            "Regering",
            "Halsa och fitness",
            "Historia",
            "Barn och familj",
            "Fritid",
            "Musik",
            "Nyheter",
            "Religion och andlighet",
            "Vetenskap",
            "Samhalle och kultur",
            "Sport",
            "Teknik",
            "True crime",
            "TV och film",
        ),
        Language::Vietnamese => (
            "Nghe thuat",
            "Kinh doanh",
            "Hai",
            "Giao duc",
            "Hu cau",
            "Chinh phu",
            "Suc khoe va the hinh",
            "Lich su",
            "Tre em va gia dinh",
            "Giai tri",
            "Am nhac",
            "Tin tuc",
            "Ton giao va tam linh",
            "Khoa hoc",
            "Xa hoi va van hoa",
            "The thao",
            "Cong nghe",
            "True crime",
            "TV va phim",
        ),
        Language::Czech => (
            "Umeni",
            "Byznys",
            "Komedie",
            "Vzdelavani",
            "Beletrie",
            "Vlada",
            "Zdravi a fitness",
            "Historie",
            "Deti a rodina",
            "Volny cas",
            "Hudba",
            "Zpravy",
            "Nabozenstvi a duchovno",
            "Veda",
            "Spolecnost a kultura",
            "Sport",
            "Technologie",
            "True crime",
            "TV a film",
        ),
        Language::Polish => (
            "Sztuka",
            "Biznes",
            "Komedia",
            "Edukacja",
            "Fikcja",
            "Rzad",
            "Zdrowie i fitness",
            "Historia",
            "Dzieci i rodzina",
            "Rozrywka",
            "Muzyka",
            "Wiadomosci",
            "Religia i duchownosc",
            "Nauka",
            "Spoleczenstwo i kultura",
            "Sport",
            "Technologia",
            "True crime",
            "TV i film",
        ),
        Language::French => (
            "Arts",
            "Affaires",
            "Comedie",
            "Education",
            "Fiction",
            "Gouvernement",
            "Sante et fitness",
            "Histoire",
            "Enfants et famille",
            "Loisirs",
            "Musique",
            "Actualites",
            "Religion et spiritualite",
            "Science",
            "Societe et culture",
            "Sports",
            "Technologie",
            "True crime",
            "TV et cinema",
        ),
    };
    let mut categories = vec![
        Category {
            id: 1301,
            name: arts.to_string(),
        },
        Category {
            id: 1321,
            name: business.to_string(),
        },
        Category {
            id: 1303,
            name: comedy.to_string(),
        },
        Category {
            id: 1304,
            name: education.to_string(),
        },
        Category {
            id: 1483,
            name: fiction.to_string(),
        },
        Category {
            id: 1511,
            name: government.to_string(),
        },
        Category {
            id: 1512,
            name: health_fitness.to_string(),
        },
        Category {
            id: 1487,
            name: history.to_string(),
        },
        Category {
            id: 1305,
            name: kids_family.to_string(),
        },
        Category {
            id: 1502,
            name: leisure.to_string(),
        },
        Category {
            id: 1310,
            name: music.to_string(),
        },
        Category {
            id: 1489,
            name: news.to_string(),
        },
        Category {
            id: 1314,
            name: religion_spirituality.to_string(),
        },
        Category {
            id: 1533,
            name: science.to_string(),
        },
        Category {
            id: 1324,
            name: society_culture.to_string(),
        },
        Category {
            id: 1545,
            name: sports.to_string(),
        },
        Category {
            id: 1318,
            name: technology.to_string(),
        },
        Category {
            id: 1488,
            name: true_crime.to_string(),
        },
        Category {
            id: 1309,
            name: tv_film.to_string(),
        },
    ];
    categories.extend(apple_subcategories(language));
    categories
}

fn apple_subcategories(language: Language) -> Vec<Category> {
    // Apple genre IDs from the official Podcasts genre tree.
    // Subcategory names are localized per app language (ASCII only).
    let subcategories: &[(u32, &str)] = match language {
        Language::Italian => &[
            // Arts
            (1482, "Libri"),
            (1402, "Design"),
            (1459, "Moda e bellezza"),
            (1306, "Cibo"),
            (1405, "Arti performative"),
            (1406, "Arti visive"),
            // Business
            (1410, "Carriere"),
            (1493, "Imprenditoria"),
            (1412, "Investimenti"),
            (1491, "Management"),
            (1492, "Marketing"),
            (1494, "Non profit"),
            // Comedy
            (1496, "Interviste comiche"),
            (1495, "Improvvisazione"),
            (1497, "Stand-up"),
            // Education
            (1501, "Corsi"),
            (1499, "Come fare"),
            (1498, "Apprendimento lingue"),
            (1500, "Crescita personale"),
            // Fiction
            (1486, "Narrativa comica"),
            (1484, "Dramma"),
            (1485, "Fantascienza"),
            // Health & Fitness
            (1513, "Salute alternativa"),
            (1514, "Fitness"),
            (1518, "Medicina"),
            (1517, "Salute mentale"),
            (1515, "Nutrizione"),
            (1516, "Sessualita"),
            // Kids & Family
            (1519, "Educazione per bambini"),
            (1521, "Genitorialita"),
            (1522, "Animali domestici"),
            (1520, "Storie per bambini"),
            // Leisure
            (1510, "Animazione e manga"),
            (1503, "Automobili"),
            (1504, "Aviazione"),
            (1506, "Artigianato"),
            (1507, "Giochi"),
            (1505, "Hobby"),
            (1508, "Casa e giardino"),
            (1509, "Videogiochi"),
            // Music
            (1523, "Commenti musicali"),
            (1524, "Storia della musica"),
            (1525, "Interviste musicali"),
            // News
            (1490, "Notizie di economia"),
            (1526, "Notizie quotidiane"),
            (1531, "Notizie di spettacolo"),
            (1530, "Commenti alle notizie"),
            (1527, "Politica"),
            (1529, "Notizie sportive"),
            (1528, "Notizie tecnologiche"),
            // Religion & Spirituality
            (1438, "Buddismo"),
            (1439, "Cristianesimo"),
            (1463, "Induismo"),
            (1440, "Islam"),
            (1441, "Ebraismo"),
            (1532, "Religione"),
            (1444, "Spiritualita"),
            // Science
            (1538, "Astronomia"),
            (1539, "Chimica"),
            (1540, "Scienze della terra"),
            (1541, "Scienze della vita"),
            (1536, "Matematica"),
            (1534, "Scienze naturali"),
            (1537, "Natura"),
            (1542, "Fisica"),
            (1535, "Scienze sociali"),
            // Society & Culture
            (1543, "Documentari"),
            (1302, "Diari personali"),
            (1443, "Filosofia"),
            (1320, "Luoghi e viaggi"),
            (1544, "Relazioni"),
            // Sports
            (1549, "Baseball"),
            (1548, "Pallacanestro"),
            (1554, "Cricket"),
            (1560, "Fantasy sport"),
            (1547, "Football americano"),
            (1553, "Golf"),
            (1550, "Hockey"),
            (1552, "Rugby"),
            (1551, "Corsa"),
            (1546, "Calcio"),
            (1558, "Nuoto"),
            (1556, "Tennis"),
            (1557, "Pallavolo"),
            (1559, "Natura selvaggia"),
            (1555, "Lotta"),
            // TV & Film
            (1562, "Dopo show"),
            (1564, "Storia del cinema"),
            (1565, "Interviste sul cinema"),
            (1563, "Recensioni di film"),
            (1561, "Recensioni TV"),
        ],
        Language::German => &[
            (1482, "Bücher"),
            (1402, "Design"),
            (1459, "Mode und Schönheit"),
            (1306, "Essen"),
            (1405, "Darstellende Kunst"),
            (1406, "Bildende Kunst"),
            (1410, "Karriere"),
            (1493, "Unternehmertum"),
            (1412, "Investieren"),
            (1491, "Management"),
            (1492, "Marketing"),
            (1494, "Gemeinnützig"),
            (1496, "Comedy-Interviews"),
            (1495, "Improvisation"),
            (1497, "Stand-up"),
            (1501, "Kurse"),
            (1499, "Anleitungen"),
            (1498, "Sprachenlernen"),
            (1500, "Persönliche Entwicklung"),
            (1486, "Humoristische Fiktion"),
            (1484, "Drama"),
            (1485, "Science-Fiction"),
            (1513, "Alternative Gesundheit"),
            (1514, "Fitness"),
            (1518, "Medizin"),
            (1517, "Psychische Gesundheit"),
            (1515, "Ernährung"),
            (1516, "Sexualität"),
            (1519, "Bildung für Kinder"),
            (1521, "Elternschaft"),
            (1522, "Haustiere und Tiere"),
            (1520, "Geschichten für Kinder"),
            (1510, "Animation und Manga"),
            (1503, "Automobile"),
            (1504, "Luftfahrt"),
            (1506, "Handwerk"),
            (1507, "Spiele"),
            (1505, "Hobbys"),
            (1508, "Haus und Garten"),
            (1509, "Videospiele"),
            (1523, "Musikkommentare"),
            (1524, "Musikgeschichte"),
            (1525, "Musikinterviews"),
            (1490, "Wirtschaftsnachrichten"),
            (1526, "Tägliche Nachrichten"),
            (1531, "Unterhaltungsnachrichten"),
            (1530, "Nachrichtenkommentare"),
            (1527, "Politik"),
            (1529, "Sportnachrichten"),
            (1528, "Technologienachrichten"),
            (1438, "Buddhismus"),
            (1439, "Christentum"),
            (1463, "Hinduismus"),
            (1440, "Islam"),
            (1441, "Judentum"),
            (1532, "Religion"),
            (1444, "Spiritualität"),
            (1538, "Astronomie"),
            (1539, "Chemie"),
            (1540, "Geowissenschaften"),
            (1541, "Biowissenschaften"),
            (1536, "Mathematik"),
            (1534, "Naturwissenschaften"),
            (1537, "Natur"),
            (1542, "Physik"),
            (1535, "Sozialwissenschaften"),
            (1543, "Dokumentation"),
            (1302, "Persönliche Tagebücher"),
            (1443, "Philosophie"),
            (1320, "Orte und Reisen"),
            (1544, "Beziehungen"),
            (1549, "Baseball"),
            (1548, "Basketball"),
            (1554, "Cricket"),
            (1560, "Fantasysport"),
            (1547, "American Football"),
            (1553, "Golf"),
            (1550, "Hockey"),
            (1552, "Rugby"),
            (1551, "Laufen"),
            (1546, "Fußball"),
            (1558, "Schwimmen"),
            (1556, "Tennis"),
            (1557, "Volleyball"),
            (1559, "Outdoor"),
            (1555, "Ringen"),
            (1562, "Begleitshows"),
            (1564, "Filmgeschichte"),
            (1565, "Filminterviews"),
            (1563, "Filmkritiken"),
            (1561, "TV-Kritiken"),
        ],
        Language::Ukrainian
        | Language::Lithuanian
        | Language::Russian
        | Language::Chinese
        | Language::English
        | Language::Hindi => &[
            // Arts
            (1482, "Books"),
            (1402, "Design"),
            (1459, "Fashion & Beauty"),
            (1306, "Food"),
            (1405, "Performing Arts"),
            (1406, "Visual Arts"),
            // Business
            (1410, "Careers"),
            (1493, "Entrepreneurship"),
            (1412, "Investing"),
            (1491, "Management"),
            (1492, "Marketing"),
            (1494, "Non-Profit"),
            // Comedy
            (1496, "Comedy Interviews"),
            (1495, "Improv"),
            (1497, "Stand-Up"),
            // Education
            (1501, "Courses"),
            (1499, "How To"),
            (1498, "Language Learning"),
            (1500, "Self-Improvement"),
            // Fiction
            (1486, "Comedy Fiction"),
            (1484, "Drama"),
            (1485, "Science Fiction"),
            // Health & Fitness
            (1513, "Alternative Health"),
            (1514, "Fitness"),
            (1518, "Medicine"),
            (1517, "Mental Health"),
            (1515, "Nutrition"),
            (1516, "Sexuality"),
            // Kids & Family
            (1519, "Education for Kids"),
            (1521, "Parenting"),
            (1522, "Pets & Animals"),
            (1520, "Stories for Kids"),
            // Leisure
            (1510, "Animation & Manga"),
            (1503, "Automotive"),
            (1504, "Aviation"),
            (1506, "Crafts"),
            (1507, "Games"),
            (1505, "Hobbies"),
            (1508, "Home & Garden"),
            (1509, "Video Games"),
            // Music
            (1523, "Music Commentary"),
            (1524, "Music History"),
            (1525, "Music Interviews"),
            // News
            (1490, "Business News"),
            (1526, "Daily News"),
            (1531, "Entertainment News"),
            (1530, "News Commentary"),
            (1527, "Politics"),
            (1529, "Sports News"),
            (1528, "Tech News"),
            // Religion & Spirituality
            (1438, "Buddhism"),
            (1439, "Christianity"),
            (1463, "Hinduism"),
            (1440, "Islam"),
            (1441, "Judaism"),
            (1532, "Religion"),
            (1444, "Spirituality"),
            // Science
            (1538, "Astronomy"),
            (1539, "Chemistry"),
            (1540, "Earth Sciences"),
            (1541, "Life Sciences"),
            (1536, "Mathematics"),
            (1534, "Natural Sciences"),
            (1537, "Nature"),
            (1542, "Physics"),
            (1535, "Social Sciences"),
            // Society & Culture
            (1543, "Documentary"),
            (1302, "Personal Journals"),
            (1443, "Philosophy"),
            (1320, "Places & Travel"),
            (1544, "Relationships"),
            // Sports
            (1549, "Baseball"),
            (1548, "Basketball"),
            (1554, "Cricket"),
            (1560, "Fantasy Sports"),
            (1547, "Football"),
            (1553, "Golf"),
            (1550, "Hockey"),
            (1552, "Rugby"),
            (1551, "Running"),
            (1546, "Soccer"),
            (1558, "Swimming"),
            (1556, "Tennis"),
            (1557, "Volleyball"),
            (1559, "Wilderness"),
            (1555, "Wrestling"),
            // TV & Film
            (1562, "After Shows"),
            (1564, "Film History"),
            (1565, "Film Interviews"),
            (1563, "Film Reviews"),
            (1561, "TV Reviews"),
        ],
        Language::Serbian => &[
            // Arts
            (1482, "Knjige"),
            (1402, "Dizajn"),
            (1459, "Moda i lepota"),
            (1306, "Hrana"),
            (1405, "Izvodjacke umetnosti"),
            (1406, "Vizuelne umetnosti"),
            // Business
            (1410, "Karijera"),
            (1493, "Preduzetnistvo"),
            (1412, "Investiranje"),
            (1491, "Menadzment"),
            (1492, "Marketing"),
            (1494, "Neprofitno"),
            // Comedy
            (1496, "Komedijski intervjui"),
            (1495, "Improvizacija"),
            (1497, "Stand-Up"),
            // Education
            (1501, "Kursevi"),
            (1499, "Kako da"),
            (1498, "Ucenje jezika"),
            (1500, "Licni razvoj"),
            // Fiction
            (1486, "Komedijska fikcija"),
            (1484, "Drama"),
            (1485, "Naucna fantastika"),
            // Health & Fitness
            (1513, "Alternativno zdravlje"),
            (1514, "Fitnes"),
            (1518, "Medicina"),
            (1517, "Mentalno zdravlje"),
            (1515, "Ishrana"),
            (1516, "Seksualnost"),
            // Kids & Family
            (1519, "Obrazovanje za decu"),
            (1521, "Roditeljstvo"),
            (1522, "Kucni ljubimci i zivotinje"),
            (1520, "Price za decu"),
            // Leisure
            (1510, "Animacija i manga"),
            (1503, "Automobili"),
            (1504, "Avijacija"),
            (1506, "Rucni rad"),
            (1507, "Igre"),
            (1505, "Hobi"),
            (1508, "Kuca i basta"),
            (1509, "Video igre"),
            // Music
            (1523, "Komentari o muzici"),
            (1524, "Istorija muzike"),
            (1525, "Intervjui o muzici"),
            // News
            (1490, "Poslovne vesti"),
            (1526, "Dnevne vesti"),
            (1531, "Vesti iz zabave"),
            (1530, "Komentari vesti"),
            (1527, "Politika"),
            (1529, "Sportske vesti"),
            (1528, "Tehnoloske vesti"),
            // Religion & Spirituality
            (1438, "Budizam"),
            (1439, "Hriscanstvo"),
            (1463, "Hinduizam"),
            (1440, "Islam"),
            (1441, "Judaizam"),
            (1532, "Religija"),
            (1444, "Duhovnost"),
            // Science
            (1538, "Astronomija"),
            (1539, "Hemija"),
            (1540, "Nauke o Zemlji"),
            (1541, "Nauke o zivotu"),
            (1536, "Matematika"),
            (1534, "Prirodne nauke"),
            (1537, "Priroda"),
            (1542, "Fizika"),
            (1535, "Drustvene nauke"),
            // Society & Culture
            (1543, "Dokumentarni"),
            (1302, "Licni dnevnici"),
            (1443, "Filozofija"),
            (1320, "Mesta i putovanja"),
            (1544, "Odnosi"),
            // Sports
            (1549, "Bejzbol"),
            (1548, "Kosarka"),
            (1554, "Kriket"),
            (1560, "Fantazi sport"),
            (1547, "Americki fudbal"),
            (1553, "Golf"),
            (1550, "Hokej"),
            (1552, "Ragbi"),
            (1551, "Trcanje"),
            (1546, "Fudbal"),
            (1558, "Plivanje"),
            (1556, "Tenis"),
            (1557, "Odbojka"),
            (1559, "Divljina"),
            (1555, "Rvanje"),
            // TV & Film
            (1562, "After show"),
            (1564, "Istorija filma"),
            (1565, "Intervjui o filmu"),
            (1563, "Filmske recenzije"),
            (1561, "TV recenzije"),
        ],
        Language::Spanish => &[
            // Arts
            (1482, "Libros"),
            (1402, "Diseno"),
            (1459, "Moda y belleza"),
            (1306, "Comida"),
            (1405, "Artes escenicas"),
            (1406, "Artes visuales"),
            // Business
            (1410, "Carreras"),
            (1493, "Emprendimiento"),
            (1412, "Inversion"),
            (1491, "Gestion"),
            (1492, "Marketing"),
            (1494, "Sin fines de lucro"),
            // Comedy
            (1496, "Entrevistas de comedia"),
            (1495, "Improvisacion"),
            (1497, "Stand-up"),
            // Education
            (1501, "Cursos"),
            (1499, "Como hacer"),
            (1498, "Aprendizaje de idiomas"),
            (1500, "Desarrollo personal"),
            // Fiction
            (1486, "Ficcion comica"),
            (1484, "Drama"),
            (1485, "Ciencia ficcion"),
            // Health & Fitness
            (1513, "Salud alternativa"),
            (1514, "Fitness"),
            (1518, "Medicina"),
            (1517, "Salud mental"),
            (1515, "Nutricion"),
            (1516, "Sexualidad"),
            // Kids & Family
            (1519, "Educacion para ninos"),
            (1521, "Crianza"),
            (1522, "Mascotas y animales"),
            (1520, "Cuentos para ninos"),
            // Leisure
            (1510, "Animacion y manga"),
            (1503, "Automocion"),
            (1504, "Aviacion"),
            (1506, "Manualidades"),
            (1507, "Juegos"),
            (1505, "Pasatiempos"),
            (1508, "Hogar y jardin"),
            (1509, "Videojuegos"),
            // Music
            (1523, "Comentarios musicales"),
            (1524, "Historia de la musica"),
            (1525, "Entrevistas musicales"),
            // News
            (1490, "Noticias de negocios"),
            (1526, "Noticias diarias"),
            (1531, "Noticias de entretenimiento"),
            (1530, "Comentario de noticias"),
            (1527, "Politica"),
            (1529, "Noticias deportivas"),
            (1528, "Noticias de tecnologia"),
            // Religion & Spirituality
            (1438, "Budismo"),
            (1439, "Cristianismo"),
            (1463, "Hinduismo"),
            (1440, "Islam"),
            (1441, "Judaismo"),
            (1532, "Religion"),
            (1444, "Espiritualidad"),
            // Science
            (1538, "Astronomia"),
            (1539, "Quimica"),
            (1540, "Ciencias de la tierra"),
            (1541, "Ciencias de la vida"),
            (1536, "Matematicas"),
            (1534, "Ciencias naturales"),
            (1537, "Naturaleza"),
            (1542, "Fisica"),
            (1535, "Ciencias sociales"),
            // Society & Culture
            (1543, "Documentales"),
            (1302, "Diarios personales"),
            (1443, "Filosofia"),
            (1320, "Lugares y viajes"),
            (1544, "Relaciones"),
            // Sports
            (1549, "Beisbol"),
            (1548, "Baloncesto"),
            (1554, "Cricket"),
            (1560, "Deportes fantasy"),
            (1547, "Futbol americano"),
            (1553, "Golf"),
            (1550, "Hockey"),
            (1552, "Rugby"),
            (1551, "Correr"),
            (1546, "Futbol"),
            (1558, "Natacion"),
            (1556, "Tenis"),
            (1557, "Voleibol"),
            (1559, "Naturaleza salvaje"),
            (1555, "Lucha"),
            // TV & Film
            (1562, "After show"),
            (1564, "Historia del cine"),
            (1565, "Entrevistas de cine"),
            (1563, "Criticas de cine"),
            (1561, "Criticas de TV"),
        ],
        Language::Portuguese => &[
            // Arts
            (1482, "Livros"),
            (1402, "Design"),
            (1459, "Moda e beleza"),
            (1306, "Comida"),
            (1405, "Artes performaticas"),
            (1406, "Artes visuais"),
            // Business
            (1410, "Carreiras"),
            (1493, "Empreendedorismo"),
            (1412, "Investimentos"),
            (1491, "Gestao"),
            (1492, "Marketing"),
            (1494, "Sem fins lucrativos"),
            // Comedy
            (1496, "Entrevistas de comedia"),
            (1495, "Improvisacao"),
            (1497, "Stand-up"),
            // Education
            (1501, "Cursos"),
            (1499, "Como fazer"),
            (1498, "Aprendizagem de idiomas"),
            (1500, "Desenvolvimento pessoal"),
            // Fiction
            (1486, "Ficcao comica"),
            (1484, "Drama"),
            (1485, "Ficcao cientifica"),
            // Health & Fitness
            (1513, "Saude alternativa"),
            (1514, "Fitness"),
            (1518, "Medicina"),
            (1517, "Saude mental"),
            (1515, "Nutricao"),
            (1516, "Sexualidade"),
            // Kids & Family
            (1519, "Educacao para criancas"),
            (1521, "Parentalidade"),
            (1522, "Animais de estimacao"),
            (1520, "Historias para criancas"),
            // Leisure
            (1510, "Animacao e manga"),
            (1503, "Automotivo"),
            (1504, "Aviacao"),
            (1506, "Artesanato"),
            (1507, "Jogos"),
            (1505, "Hobbies"),
            (1508, "Casa e jardim"),
            (1509, "Jogos eletronicos"),
            // Music
            (1523, "Comentarios musicais"),
            (1524, "Historia da musica"),
            (1525, "Entrevistas musicais"),
            // News
            (1490, "Noticias de negocios"),
            (1526, "Noticias diarias"),
            (1531, "Noticias de entretenimento"),
            (1530, "Comentarios de noticias"),
            (1527, "Politica"),
            (1529, "Noticias esportivas"),
            (1528, "Noticias de tecnologia"),
            // Religion & Spirituality
            (1438, "Budismo"),
            (1439, "Cristianismo"),
            (1463, "Hinduismo"),
            (1440, "Islam"),
            (1441, "Judaismo"),
            (1532, "Religiao"),
            (1444, "Espiritualidade"),
            // Science
            (1538, "Astronomia"),
            (1539, "Quimica"),
            (1540, "Ciencias da terra"),
            (1541, "Ciencias da vida"),
            (1536, "Matematica"),
            (1534, "Ciencias naturais"),
            (1537, "Natureza"),
            (1542, "Fisica"),
            (1535, "Ciencias sociais"),
            // Society & Culture
            (1543, "Documentarios"),
            (1302, "Diarios pessoais"),
            (1443, "Filosofia"),
            (1320, "Lugares e viagens"),
            (1544, "Relacionamentos"),
            // Sports
            (1549, "Beisebol"),
            (1548, "Basquete"),
            (1554, "Cricket"),
            (1560, "Esportes fantasy"),
            (1547, "Futebol americano"),
            (1553, "Golfe"),
            (1550, "Hoquei"),
            (1552, "Rugby"),
            (1551, "Corrida"),
            (1546, "Futebol"),
            (1558, "Natacao"),
            (1556, "Tenis"),
            (1557, "Volei"),
            (1559, "Natureza selvagem"),
            (1555, "Luta"),
            // TV & Film
            (1562, "After show"),
            (1564, "Historia do cinema"),
            (1565, "Entrevistas de cinema"),
            (1563, "Criticas de cinema"),
            (1561, "Criticas de TV"),
        ],
        Language::PortugueseBrazilian => &[
            // Arts
            (1482, "Livros"),
            (1402, "Design"),
            (1459, "Moda e beleza"),
            (1306, "Gastronomia"),
            (1405, "Artes cênicas"),
            (1406, "Artes visuais"),
            // Business
            (1410, "Carreiras"),
            (1493, "Empreendedorismo"),
            (1412, "Investimentos"),
            (1491, "Gestão"),
            (1492, "Marketing"),
            (1494, "Organizações sem fins lucrativos"),
            // Comedy
            (1496, "Entrevistas de comédia"),
            (1495, "Improvisação"),
            (1497, "Stand-up"),
            // Education
            (1501, "Cursos"),
            (1499, "Como fazer"),
            (1498, "Aprendizagem de idiomas"),
            (1500, "Desenvolvimento pessoal"),
            // Fiction
            (1486, "Ficção cômica"),
            (1484, "Drama"),
            (1485, "Ficção científica"),
            // Health & Fitness
            (1513, "Saúde alternativa"),
            (1514, "Fitness"),
            (1518, "Medicina"),
            (1517, "Saúde mental"),
            (1515, "Nutrição"),
            (1516, "Sexualidade"),
            // Kids & Family
            (1519, "Educação para crianças"),
            (1521, "Maternidade e paternidade"),
            (1522, "Animais de estimação"),
            (1520, "Histórias para crianças"),
            // Leisure
            (1510, "Animação e mangá"),
            (1503, "Automotivo"),
            (1504, "Aviação"),
            (1506, "Artesanato"),
            (1507, "Jogos"),
            (1505, "Hobbies"),
            (1508, "Casa e jardim"),
            (1509, "Jogos eletrônicos"),
            // Music
            (1523, "Comentários musicais"),
            (1524, "História da música"),
            (1525, "Entrevistas musicais"),
            // News
            (1490, "Notícias de negócios"),
            (1526, "Notícias diárias"),
            (1531, "Notícias de entretenimento"),
            (1530, "Comentários de notícias"),
            (1527, "Política"),
            (1529, "Notícias esportivas"),
            (1528, "Notícias de tecnologia"),
            // Religion & Spirituality
            (1438, "Budismo"),
            (1439, "Cristianismo"),
            (1463, "Hinduismo"),
            (1440, "Islã"),
            (1441, "Judaísmo"),
            (1532, "Religião"),
            (1444, "Espiritualidade"),
            // Science
            (1538, "Astronomia"),
            (1539, "Química"),
            (1540, "Ciências da terra"),
            (1541, "Ciências da vida"),
            (1536, "Matemática"),
            (1534, "Ciências naturais"),
            (1537, "Natureza"),
            (1542, "Física"),
            (1535, "Ciências sociais"),
            // Society & Culture
            (1543, "Documentários"),
            (1302, "Diários pessoais"),
            (1443, "Filosofia"),
            (1320, "Lugares e viagens"),
            (1544, "Relacionamentos"),
            // Sports
            (1549, "Beisebol"),
            (1548, "Basquete"),
            (1554, "Críquete"),
            (1560, "Esportes fantasy"),
            (1547, "Futebol americano"),
            (1553, "Golfe"),
            (1550, "Hóquei"),
            (1552, "Rugby"),
            (1551, "Corrida"),
            (1546, "Futebol"),
            (1558, "Natação"),
            (1556, "Tênis"),
            (1557, "Vôlei"),
            (1559, "Natureza selvagem"),
            (1555, "Luta"),
            // TV & Film
            (1562, "Pós-programa"),
            (1564, "História do cinema"),
            (1565, "Entrevistas de cinema"),
            (1563, "Críticas de cinema"),
            (1561, "Críticas de TV"),
        ],
        Language::Swedish => &[
            // Arts
            (1482, "Bocker"),
            (1402, "Design"),
            (1459, "Mode och skonhet"),
            (1306, "Mat"),
            (1405, "Scenkonst"),
            (1406, "Visuell konst"),
            // Business
            (1410, "Karriar"),
            (1493, "Entreprenorskap"),
            (1412, "Investeringar"),
            (1491, "Ledning"),
            (1492, "Marknadsforing"),
            (1494, "Ideell verksamhet"),
            // Comedy
            (1496, "Komedintervjuer"),
            (1495, "Improvisation"),
            (1497, "Stand-up"),
            // Education
            (1501, "Kurser"),
            (1499, "Sa gor du"),
            (1498, "Sprakinlarning"),
            (1500, "Sjalvutveckling"),
            // Fiction
            (1486, "Komedifiktion"),
            (1484, "Drama"),
            (1485, "Science fiction"),
            // Health & Fitness
            (1513, "Alternativ halsa"),
            (1514, "Fitness"),
            (1518, "Medicin"),
            (1517, "Mental halsa"),
            (1515, "Naringslara"),
            (1516, "Sexualitet"),
            // Kids & Family
            (1519, "Utbildning for barn"),
            (1521, "Foraldraskap"),
            (1522, "Husdjur och djur"),
            (1520, "Berattelser for barn"),
            // Leisure
            (1510, "Animation och manga"),
            (1503, "Bil"),
            (1504, "Flyg"),
            (1506, "Hantverk"),
            (1507, "Spel"),
            (1505, "Hobbyer"),
            (1508, "Hem och tradgard"),
            (1509, "Datorspel"),
            // Music
            (1523, "Musikkommentarer"),
            (1524, "Musikhistoria"),
            (1525, "Musikintervjuer"),
            // News
            (1490, "Ekonominyheter"),
            (1526, "Dagens nyheter"),
            (1531, "Nojesnyheter"),
            (1530, "Nyhetskommentarer"),
            (1527, "Politik"),
            (1529, "Sportnyheter"),
            (1528, "Tekniknyheter"),
            // Religion & Spirituality
            (1438, "Buddhism"),
            (1439, "Kristendom"),
            (1463, "Hinduism"),
            (1440, "Islam"),
            (1441, "Judendom"),
            (1532, "Religion"),
            (1444, "Andlighet"),
            // Science
            (1538, "Astronomi"),
            (1539, "Kemi"),
            (1540, "Geovetenskaper"),
            (1541, "Livsvetenskaper"),
            (1536, "Matematik"),
            (1534, "Naturvetenskap"),
            (1537, "Natur"),
            (1542, "Fysik"),
            (1535, "Samhallsvetenskaper"),
            // Society & Culture
            (1543, "Dokumentar"),
            (1302, "Personliga dagbocker"),
            (1443, "Filosofi"),
            (1320, "Platser och resor"),
            (1544, "Relationer"),
            // Sports
            (1549, "Baseboll"),
            (1548, "Basket"),
            (1554, "Cricket"),
            (1560, "Fantasisport"),
            (1547, "Amerikansk fotboll"),
            (1553, "Golf"),
            (1550, "Hockey"),
            (1552, "Rugby"),
            (1551, "Lopning"),
            (1546, "Fotboll"),
            (1558, "Simning"),
            (1556, "Tennis"),
            (1557, "Volleyboll"),
            (1559, "Vildmark"),
            (1555, "Brotning"),
            // TV & Film
            (1562, "Eftershow"),
            (1564, "Filmhistoria"),
            (1565, "Filmintervjuer"),
            (1563, "Filmrecensioner"),
            (1561, "TV-recensioner"),
        ],
        Language::Vietnamese => &[
            // Arts
            (1482, "Sach"),
            (1402, "Thiet ke"),
            (1459, "Thoi trang va lam dep"),
            (1306, "Am thuc"),
            (1405, "Nghe thuat bieu dien"),
            (1406, "Nghe thuat thi giac"),
            // Business
            (1410, "Nghe nghiep"),
            (1493, "Khoi nghiep"),
            (1412, "Dau tu"),
            (1491, "Quan ly"),
            (1492, "Tiep thi"),
            (1494, "Phi loi nhuan"),
            // Comedy
            (1496, "Phong van hai"),
            (1495, "Ung tac"),
            (1497, "Hai doc thoai"),
            // Education
            (1501, "Khoa hoc"),
            (1499, "Cach lam"),
            (1498, "Hoc ngon ngu"),
            (1500, "Phat trien ban than"),
            // Fiction
            (1486, "Tieu thuyet hai"),
            (1484, "Drama"),
            (1485, "Khoa hoc vien tuong"),
            // Health & Fitness
            (1513, "Suc khoe thay the"),
            (1514, "The hinh"),
            (1518, "Y hoc"),
            (1517, "Suc khoe tam than"),
            (1515, "Dinh duong"),
            (1516, "Tinh duc"),
            // Kids & Family
            (1519, "Giao duc tre em"),
            (1521, "Nuoi day con"),
            (1522, "Thu cung va dong vat"),
            (1520, "Truyen cho tre"),
            // Leisure
            (1510, "Hoat hinh va manga"),
            (1503, "O to"),
            (1504, "Hang khong"),
            (1506, "Thu cong"),
            (1507, "Tro choi"),
            (1505, "So thich"),
            (1508, "Nha va vuon"),
            (1509, "Tro choi dien tu"),
            // Music
            (1523, "Binh luan am nhac"),
            (1524, "Lich su am nhac"),
            (1525, "Phong van am nhac"),
            // News
            (1490, "Tin kinh doanh"),
            (1526, "Tin hang ngay"),
            (1531, "Tin giai tri"),
            (1530, "Binh luan thoi su"),
            (1527, "Chinh tri"),
            (1529, "Tin the thao"),
            (1528, "Tin cong nghe"),
            // Religion & Spirituality
            (1438, "Phat giao"),
            (1439, "Kito giao"),
            (1463, "An do giao"),
            (1440, "Hoi giao"),
            (1441, "Do thai"),
            (1532, "Ton giao"),
            (1444, "Tam linh"),
            // Science
            (1538, "Thien van hoc"),
            (1539, "Hoa hoc"),
            (1540, "Khoa hoc trai dat"),
            (1541, "Khoa hoc doi song"),
            (1536, "Toan hoc"),
            (1534, "Khoa hoc tu nhien"),
            (1537, "Thien nhien"),
            (1542, "Vat ly"),
            (1535, "Khoa hoc xa hoi"),
            // Society & Culture
            (1543, "Tai lieu"),
            (1302, "Nhat ky ca nhan"),
            (1443, "Triet hoc"),
            (1320, "Dia diem va du lich"),
            (1544, "Moi quan he"),
            // Sports
            (1549, "Bong chay"),
            (1548, "Bong ro"),
            (1554, "Cricket"),
            (1560, "The thao ao"),
            (1547, "Bong bau duc"),
            (1553, "Golf"),
            (1550, "Hockey"),
            (1552, "Rugby"),
            (1551, "Chay bo"),
            (1546, "Bong da"),
            (1558, "Boi"),
            (1556, "Quan vot"),
            (1557, "Bong chuyen"),
            (1559, "Hoang da"),
            (1555, "Vat"),
            // TV & Film
            (1562, "After show"),
            (1564, "Lich su dien anh"),
            (1565, "Phong van dien anh"),
            (1563, "Danh gia phim"),
            (1561, "Danh gia TV"),
        ],
        Language::Czech => &[
            // Arts
            (1482, "Knihy"),
            (1402, "Design"),
            (1459, "Moda a krasa"),
            (1306, "Jidlo"),
            (1405, "Scenicka umeni"),
            (1406, "Vytvarne umeni"),
            // Business
            (1410, "Kariera"),
            (1493, "Podnikani"),
            (1412, "Investovani"),
            (1491, "Rizeni"),
            (1492, "Marketing"),
            (1494, "Neziskove organizace"),
            // Comedy
            (1496, "Komedialni rozhovory"),
            (1495, "Improvizace"),
            (1497, "Stand-up"),
            // Education
            (1501, "Kurzy"),
            (1499, "Jak na to"),
            (1498, "Vyuka jazyku"),
            (1500, "Sebezlepseni"),
            // Fiction
            (1486, "Komedialni fikce"),
            (1484, "Drama"),
            (1485, "Sci-fi"),
            // Health & Fitness
            (1513, "Alternativni zdravi"),
            (1514, "Fitness"),
            (1518, "Medicina"),
            (1517, "Du sevni zdravi"),
            (1515, "Vyziva"),
            (1516, "Sexualita"),
            // Kids & Family
            (1519, "Vzdelavani pro deti"),
            (1521, "Rodicovstvi"),
            (1522, "Domaci mazlicci a zvirata"),
            (1520, "Pribehy pro deti"),
            // Leisure
            (1510, "Animace a manga"),
            (1503, "Automobily"),
            (1504, "Letectvi"),
            (1506, "Remesla"),
            (1507, "Hry"),
            (1505, "Zajmy"),
            (1508, "Dum a zahrada"),
            (1509, "Videohry"),
            // Music
            (1523, "Hudebni komentar"),
            (1524, "Dejiny hudby"),
            (1525, "Hudebni rozhovory"),
            // News
            (1490, "Byznysove zpravy"),
            (1526, "Denni zpravy"),
            (1531, "Zpravy ze zabavy"),
            (1530, "Komentare ke zpravam"),
            (1527, "Politika"),
            (1529, "Sportovni zpravy"),
            (1528, "Technologicke zpravy"),
            // Religion & Spirituality
            (1438, "Buddhismus"),
            (1439, "Krestanstvi"),
            (1463, "Hinduismus"),
            (1440, "Islam"),
            (1441, "Judaismus"),
            (1532, "Nabozenstvi"),
            (1444, "Spiritualita"),
            // Science
            (1538, "Astronomie"),
            (1539, "Chemie"),
            (1540, "Vedy o Zemi"),
            (1541, "Zivotni vedy"),
            (1536, "Matematika"),
            (1534, "Prirodni vedy"),
            (1537, "Priroda"),
            (1542, "Fyzika"),
            (1535, "Spolecenske vedy"),
            // Society & Culture
            (1543, "Dokument"),
            (1302, "Osobni deniky"),
            (1443, "Filozofie"),
            (1320, "Mista a cestovani"),
            (1544, "Vztahy"),
            // Sports
            (1549, "Baseball"),
            (1548, "Basketbal"),
            (1554, "Kriket"),
            (1560, "Fantasy sporty"),
            (1547, "Americky fotbal"),
            (1553, "Golf"),
            (1550, "Hokej"),
            (1552, "Ragby"),
            (1551, "Beh"),
            (1546, "Fotbal"),
            (1558, "Plavani"),
            (1556, "Tenis"),
            (1557, "Volejbal"),
            (1559, "Divocina"),
            (1555, "Zapas"),
            // TV & Film
            (1562, "After show"),
            (1564, "Dejiny filmu"),
            (1565, "Filmove rozhovory"),
            (1563, "Filmove recenze"),
            (1561, "Recenze TV"),
        ],
        Language::Polish => &[
            // Arts
            (1482, "Ksiazki"),
            (1402, "Design"),
            (1459, "Moda i uroda"),
            (1306, "Jedzenie"),
            (1405, "Sztuki widowiskowe"),
            (1406, "Sztuki wizualne"),
            // Business
            (1410, "Kariera"),
            (1493, "Przedsiebiorczosc"),
            (1412, "Inwestowanie"),
            (1491, "Zarzadzanie"),
            (1492, "Marketing"),
            (1494, "Non-profit"),
            // Comedy
            (1496, "Wywiady komediowe"),
            (1495, "Improwizacja"),
            (1497, "Stand-up"),
            // Education
            (1501, "Kursy"),
            (1499, "Jak to zrobic"),
            (1498, "Nauka jezykow"),
            (1500, "Rozwoj osobisty"),
            // Fiction
            (1486, "Fikcja komediowa"),
            (1484, "Dramat"),
            (1485, "Science fiction"),
            // Health & Fitness
            (1513, "Zdrowie alternatywne"),
            (1514, "Fitness"),
            (1518, "Medycyna"),
            (1517, "Zdrowie psychiczne"),
            (1515, "Zywienie"),
            (1516, "Seksualnosc"),
            // Kids & Family
            (1519, "Edukacja dla dzieci"),
            (1521, "Rodzicielstwo"),
            (1522, "Zwierzeta domowe"),
            (1520, "Bajki dla dzieci"),
            // Leisure
            (1510, "Animacja i manga"),
            (1503, "Motoryzacja"),
            (1504, "Lotnictwo"),
            (1506, "Rekodzielo"),
            (1507, "Gry"),
            (1505, "Hobby"),
            (1508, "Dom i ogrod"),
            (1509, "Gry wideo"),
            // Music
            (1523, "Komentarze muzyczne"),
            (1524, "Historia muzyki"),
            (1525, "Wywiady muzyczne"),
            // News
            (1490, "Wiadomosci biznesowe"),
            (1526, "Wiadomosci dnia"),
            (1531, "Wiadomosci rozrywkowe"),
            (1530, "Komentarze do wiadomosci"),
            (1527, "Polityka"),
            (1529, "Wiadomosci sportowe"),
            (1528, "Wiadomosci technologiczne"),
            // Religion & Spirituality
            (1438, "Buddyzm"),
            (1439, "Chrzescijanstwo"),
            (1463, "Hinduizm"),
            (1440, "Islam"),
            (1441, "Judaizm"),
            (1532, "Religia"),
            (1444, "Duchowosc"),
            // Science
            (1538, "Astronomia"),
            (1539, "Chemia"),
            (1540, "Nauki o Ziemi"),
            (1541, "Nauki o zyciu"),
            (1536, "Matematyka"),
            (1534, "Nauki przyrodnicze"),
            (1537, "Natura"),
            (1542, "Fizyka"),
            (1535, "Nauki spoleczne"),
            // Society & Culture
            (1543, "Dokument"),
            (1302, "Dzienniki osobiste"),
            (1443, "Filozofia"),
            (1320, "Miejsca i podroze"),
            (1544, "Relacje"),
            // Sports
            (1549, "Baseball"),
            (1548, "Koszykowka"),
            (1554, "Krykiet"),
            (1560, "Fantasy sport"),
            (1547, "Futbol amerykanski"),
            (1553, "Golf"),
            (1550, "Hokej"),
            (1552, "Rugby"),
            (1551, "Bieganie"),
            (1546, "Pilka nozna"),
            (1558, "Plywanie"),
            (1556, "Tenis"),
            (1557, "Siatkowka"),
            (1559, "Dzicz"),
            (1555, "Zapasy"),
            // TV & Film
            (1562, "After show"),
            (1564, "Historia filmu"),
            (1565, "Wywiady filmowe"),
            (1563, "Recenzje filmowe"),
            (1561, "Recenzje TV"),
        ],
        Language::French => &[
            // Arts
            (1482, "Livres"),
            (1402, "Design"),
            (1459, "Mode et beaute"),
            (1306, "Cuisine"),
            (1405, "Arts du spectacle"),
            (1406, "Arts visuels"),
            // Business
            (1410, "Carrieres"),
            (1493, "Entrepreneuriat"),
            (1412, "Investissement"),
            (1491, "Gestion"),
            (1492, "Marketing"),
            (1494, "Sans but lucratif"),
            // Comedy
            (1496, "Interviews humoristiques"),
            (1495, "Improvisation"),
            (1497, "Stand-up"),
            // Education
            (1501, "Cours"),
            (1499, "Comment faire"),
            (1498, "Apprentissage des langues"),
            (1500, "Developpement personnel"),
            // Fiction
            (1486, "Fiction comique"),
            (1484, "Drame"),
            (1485, "Science-fiction"),
            // Health & Fitness
            (1513, "Sante alternative"),
            (1514, "Fitness"),
            (1518, "Medecine"),
            (1517, "Sante mentale"),
            (1515, "Nutrition"),
            (1516, "Sexualite"),
            // Kids & Family
            (1519, "Education pour enfants"),
            (1521, "Parentalite"),
            (1522, "Animaux de compagnie"),
            (1520, "Histoires pour enfants"),
            // Leisure
            (1510, "Animation et manga"),
            (1503, "Automobile"),
            (1504, "Aviation"),
            (1506, "Artisanat"),
            (1507, "Jeux"),
            (1505, "Loisirs"),
            (1508, "Maison et jardin"),
            (1509, "Jeux video"),
            // Music
            (1523, "Commentaires musicaux"),
            (1524, "Histoire de la musique"),
            (1525, "Interviews musicales"),
            // News
            (1490, "Actualites business"),
            (1526, "Actualites quotidiennes"),
            (1531, "Actualites divertissement"),
            (1530, "Commentaires sur l'actualite"),
            (1527, "Politique"),
            (1529, "Actualites sportives"),
            (1528, "Actualites tech"),
            // Religion & Spirituality
            (1438, "Bouddhisme"),
            (1439, "Christianisme"),
            (1463, "Hindouisme"),
            (1440, "Islam"),
            (1441, "Judaisme"),
            (1532, "Religion"),
            (1444, "Spiritualite"),
            // Science
            (1538, "Astronomie"),
            (1539, "Chimie"),
            (1540, "Sciences de la Terre"),
            (1541, "Sciences de la vie"),
            (1536, "Mathematiques"),
            (1534, "Sciences naturelles"),
            (1537, "Nature"),
            (1542, "Physique"),
            (1535, "Sciences sociales"),
            // Society & Culture
            (1543, "Documentaire"),
            (1302, "Journaux personnels"),
            (1443, "Philosophie"),
            (1320, "Lieux et voyages"),
            (1544, "Relations"),
            // Sports
            (1549, "Baseball"),
            (1548, "Basket-ball"),
            (1554, "Cricket"),
            (1560, "Sports fantasy"),
            (1547, "Football americain"),
            (1553, "Golf"),
            (1550, "Hockey"),
            (1552, "Rugby"),
            (1551, "Course a pied"),
            (1546, "Football"),
            (1558, "Natation"),
            (1556, "Tennis"),
            (1557, "Volleyball"),
            (1559, "Nature sauvage"),
            (1555, "Lutte"),
            // TV & Film
            (1562, "After show"),
            (1564, "Histoire du cinema"),
            (1565, "Interviews de cinema"),
            (1563, "Critiques de films"),
            (1561, "Critiques TV"),
        ],
    };
    subcategories
        .iter()
        .map(|(id, name)| Category {
            id: *id,
            name: (*name).to_string(),
        })
        .collect()
}

fn podcastindex_categories(language: Language) -> Vec<Category> {
    // All 112 PodcastIndex categories with localized names (ASCII only).
    // Source: https://podcastindex-org.github.io/docs-api/#get-/categories/list
    let entries: &[(u32, &str)] = match language {
        Language::Italian => &[
            (1, "Arti"),
            (2, "Libri"),
            (3, "Design"),
            (4, "Moda"),
            (5, "Bellezza"),
            (6, "Cibo"),
            (7, "Arti performative"),
            (8, "Arti visive"),
            (9, "Affari"),
            (10, "Carriere"),
            (11, "Imprenditoria"),
            (12, "Investimenti"),
            (13, "Management"),
            (14, "Marketing"),
            (15, "Non profit"),
            (16, "Commedia"),
            (17, "Interviste"),
            (18, "Improvvisazione"),
            (19, "Stand-Up"),
            (20, "Istruzione"),
            (21, "Corsi"),
            (22, "Come fare"),
            (23, "Lingua"),
            (24, "Apprendimento"),
            (25, "Crescita personale"),
            (26, "Narrativa"),
            (27, "Dramma"),
            (28, "Storia"),
            (29, "Salute"),
            (30, "Fitness"),
            (31, "Alternativa"),
            (32, "Medicina"),
            (33, "Salute mentale"),
            (34, "Nutrizione"),
            (35, "Sessualita"),
            (36, "Bambini"),
            (37, "Famiglia"),
            (38, "Genitorialita"),
            (39, "Animali domestici"),
            (40, "Animali"),
            (41, "Storie"),
            (42, "Tempo libero"),
            (43, "Animazione"),
            (44, "Manga"),
            (45, "Automobili"),
            (46, "Aviazione"),
            (47, "Artigianato"),
            (48, "Giochi"),
            (49, "Hobby"),
            (50, "Casa"),
            (51, "Giardino"),
            (52, "Videogiochi"),
            (53, "Musica"),
            (54, "Commenti"),
            (55, "Notizie"),
            (56, "Quotidiano"),
            (57, "Intrattenimento"),
            (58, "Governo"),
            (59, "Politica"),
            (60, "Buddismo"),
            (61, "Cristianesimo"),
            (62, "Induismo"),
            (63, "Islam"),
            (64, "Ebraismo"),
            (65, "Religione"),
            (66, "Spiritualita"),
            (67, "Scienza"),
            (68, "Astronomia"),
            (69, "Chimica"),
            (70, "Scienze della terra"),
            (71, "Scienze della vita"),
            (72, "Matematica"),
            (73, "Scienze naturali"),
            (74, "Natura"),
            (75, "Fisica"),
            (76, "Scienze sociali"),
            (77, "Societa"),
            (78, "Cultura"),
            (79, "Documentario"),
            (80, "Personale"),
            (81, "Diari"),
            (82, "Filosofia"),
            (83, "Luoghi"),
            (84, "Viaggi"),
            (85, "Relazioni"),
            (86, "Sport"),
            (87, "Baseball"),
            (88, "Basketball"),
            (89, "Cricket"),
            (90, "Fantasy"),
            (91, "Football"),
            (92, "Golf"),
            (93, "Hockey"),
            (94, "Rugby"),
            (95, "Corsa"),
            (96, "Calcio"),
            (97, "Nuoto"),
            (98, "Tennis"),
            (99, "Volleyball"),
            (100, "Natura selvaggia"),
            (101, "Wrestling"),
            (102, "Tecnologia"),
            (103, "True Crime"),
            (104, "TV"),
            (105, "Film"),
            (106, "After-show"),
            (107, "Recensioni"),
            (108, "Clima"),
            (109, "Meteo"),
            (110, "Giochi da tavolo"),
            (111, "Gioco di ruolo"),
            (112, "Cryptocurrency"),
        ],
        Language::German => &[
            (1, "Kunst"),
            (2, "Bücher"),
            (3, "Design"),
            (4, "Mode"),
            (5, "Schönheit"),
            (6, "Essen"),
            (7, "Darstellende Kunst"),
            (8, "Bildende Kunst"),
            (9, "Wirtschaft"),
            (10, "Karriere"),
            (11, "Unternehmertum"),
            (12, "Investieren"),
            (13, "Management"),
            (14, "Marketing"),
            (15, "Gemeinnützig"),
            (16, "Comedy"),
            (17, "Interviews"),
            (18, "Improvisation"),
            (19, "Stand-up"),
            (20, "Bildung"),
            (21, "Kurse"),
            (22, "Anleitungen"),
            (23, "Sprache"),
            (24, "Lernen"),
            (25, "Persönliche Entwicklung"),
            (26, "Fiktion"),
            (27, "Drama"),
            (28, "Geschichte"),
            (29, "Gesundheit"),
            (30, "Fitness"),
            (31, "Alternative Gesundheit"),
            (32, "Medizin"),
            (33, "Psychische Gesundheit"),
            (34, "Ernährung"),
            (35, "Sexualität"),
            (36, "Kinder"),
            (37, "Familie"),
            (38, "Elternschaft"),
            (39, "Haustiere"),
            (40, "Tiere"),
            (41, "Geschichten"),
            (42, "Freizeit"),
            (43, "Animation"),
            (44, "Manga"),
            (45, "Automobile"),
            (46, "Luftfahrt"),
            (47, "Handwerk"),
            (48, "Spiele"),
            (49, "Hobbys"),
            (50, "Haus"),
            (51, "Garten"),
            (52, "Videospiele"),
            (53, "Musik"),
            (54, "Kommentare"),
            (55, "Nachrichten"),
            (56, "Täglich"),
            (57, "Unterhaltung"),
            (58, "Regierung"),
            (59, "Politik"),
            (60, "Buddhismus"),
            (61, "Christentum"),
            (62, "Hinduismus"),
            (63, "Islam"),
            (64, "Judentum"),
            (65, "Religion"),
            (66, "Spiritualität"),
            (67, "Wissenschaft"),
            (68, "Astronomie"),
            (69, "Chemie"),
            (70, "Geowissenschaften"),
            (71, "Biowissenschaften"),
            (72, "Mathematik"),
            (73, "Naturwissenschaften"),
            (74, "Natur"),
            (75, "Physik"),
            (76, "Sozialwissenschaften"),
            (77, "Gesellschaft"),
            (78, "Kultur"),
            (79, "Dokumentation"),
            (80, "Persönlich"),
            (81, "Tagebücher"),
            (82, "Philosophie"),
            (83, "Orte"),
            (84, "Reisen"),
            (85, "Beziehungen"),
            (86, "Sport"),
            (87, "Baseball"),
            (88, "Basketball"),
            (89, "Cricket"),
            (90, "Fantasysport"),
            (91, "American Football"),
            (92, "Golf"),
            (93, "Hockey"),
            (94, "Rugby"),
            (95, "Laufen"),
            (96, "Fußball"),
            (97, "Schwimmen"),
            (98, "Tennis"),
            (99, "Volleyball"),
            (100, "Outdoor"),
            (101, "Ringen"),
            (102, "Technologie"),
            (103, "True Crime"),
            (104, "TV"),
            (105, "Film"),
            (106, "Begleitshows"),
            (107, "Rezensionen"),
            (108, "Klima"),
            (109, "Wetter"),
            (110, "Brettspiele"),
            (111, "Rollenspiele"),
            (112, "Kryptowährungen"),
        ],
        Language::Ukrainian
        | Language::Lithuanian
        | Language::Russian
        | Language::Chinese
        | Language::English
        | Language::Hindi => &[
            (1, "Arts"),
            (2, "Books"),
            (3, "Design"),
            (4, "Fashion"),
            (5, "Beauty"),
            (6, "Food"),
            (7, "Performing"),
            (8, "Visual"),
            (9, "Business"),
            (10, "Careers"),
            (11, "Entrepreneurship"),
            (12, "Investing"),
            (13, "Management"),
            (14, "Marketing"),
            (15, "Non-Profit"),
            (16, "Comedy"),
            (17, "Interviews"),
            (18, "Improv"),
            (19, "Stand-Up"),
            (20, "Education"),
            (21, "Courses"),
            (22, "How-To"),
            (23, "Language"),
            (24, "Learning"),
            (25, "Self-Improvement"),
            (26, "Fiction"),
            (27, "Drama"),
            (28, "History"),
            (29, "Health"),
            (30, "Fitness"),
            (31, "Alternative"),
            (32, "Medicine"),
            (33, "Mental"),
            (34, "Nutrition"),
            (35, "Sexuality"),
            (36, "Kids"),
            (37, "Family"),
            (38, "Parenting"),
            (39, "Pets"),
            (40, "Animals"),
            (41, "Stories"),
            (42, "Leisure"),
            (43, "Animation"),
            (44, "Manga"),
            (45, "Automotive"),
            (46, "Aviation"),
            (47, "Crafts"),
            (48, "Games"),
            (49, "Hobbies"),
            (50, "Home"),
            (51, "Garden"),
            (52, "Video-Games"),
            (53, "Music"),
            (54, "Commentary"),
            (55, "News"),
            (56, "Daily"),
            (57, "Entertainment"),
            (58, "Government"),
            (59, "Politics"),
            (60, "Buddhism"),
            (61, "Christianity"),
            (62, "Hinduism"),
            (63, "Islam"),
            (64, "Judaism"),
            (65, "Religion"),
            (66, "Spirituality"),
            (67, "Science"),
            (68, "Astronomy"),
            (69, "Chemistry"),
            (70, "Earth"),
            (71, "Life"),
            (72, "Mathematics"),
            (73, "Natural"),
            (74, "Nature"),
            (75, "Physics"),
            (76, "Social"),
            (77, "Society"),
            (78, "Culture"),
            (79, "Documentary"),
            (80, "Personal"),
            (81, "Journals"),
            (82, "Philosophy"),
            (83, "Places"),
            (84, "Travel"),
            (85, "Relationships"),
            (86, "Sports"),
            (87, "Baseball"),
            (88, "Basketball"),
            (89, "Cricket"),
            (90, "Fantasy"),
            (91, "Football"),
            (92, "Golf"),
            (93, "Hockey"),
            (94, "Rugby"),
            (95, "Running"),
            (96, "Soccer"),
            (97, "Swimming"),
            (98, "Tennis"),
            (99, "Volleyball"),
            (100, "Wilderness"),
            (101, "Wrestling"),
            (102, "Technology"),
            (103, "True Crime"),
            (104, "TV"),
            (105, "Film"),
            (106, "After-Shows"),
            (107, "Reviews"),
            (108, "Climate"),
            (109, "Weather"),
            (110, "Tabletop"),
            (111, "Role-Playing"),
            (112, "Cryptocurrency"),
        ],
        Language::Serbian => &[
            (1, "Umetnost"),
            (2, "Knjige"),
            (3, "Dizajn"),
            (4, "Moda"),
            (5, "Lepota"),
            (6, "Hrana"),
            (7, "Izvodjacke"),
            (8, "Vizuelne"),
            (9, "Biznis"),
            (10, "Karijere"),
            (11, "Preduzetnistvo"),
            (12, "Investiranje"),
            (13, "Menadzment"),
            (14, "Marketing"),
            (15, "Neprofitno"),
            (16, "Komedija"),
            (17, "Intervjui"),
            (18, "Improvizacija"),
            (19, "Stand-Up"),
            (20, "Obrazovanje"),
            (21, "Kursevi"),
            (22, "Kako da"),
            (23, "Jezik"),
            (24, "Ucenje"),
            (25, "Licni razvoj"),
            (26, "Fikcija"),
            (27, "Drama"),
            (28, "Istorija"),
            (29, "Zdravlje"),
            (30, "Fitnes"),
            (31, "Alternativno"),
            (32, "Medicina"),
            (33, "Mentalno"),
            (34, "Ishrana"),
            (35, "Seksualnost"),
            (36, "Deca"),
            (37, "Porodica"),
            (38, "Roditeljstvo"),
            (39, "Ljubimci"),
            (40, "Zivotinje"),
            (41, "Price"),
            (42, "Slobodno vreme"),
            (43, "Animacija"),
            (44, "Manga"),
            (45, "Automobili"),
            (46, "Avijacija"),
            (47, "Rucni rad"),
            (48, "Igre"),
            (49, "Hobi"),
            (50, "Kuca"),
            (51, "Basta"),
            (52, "Video igre"),
            (53, "Muzika"),
            (54, "Komentari"),
            (55, "Vesti"),
            (56, "Dnevno"),
            (57, "Zabava"),
            (58, "Vlada"),
            (59, "Politika"),
            (60, "Budizam"),
            (61, "Hriscanstvo"),
            (62, "Hinduizam"),
            (63, "Islam"),
            (64, "Judaizam"),
            (65, "Religija"),
            (66, "Duhovnost"),
            (67, "Nauka"),
            (68, "Astronomija"),
            (69, "Hemija"),
            (70, "Zemlja"),
            (71, "Zivot"),
            (72, "Matematika"),
            (73, "Prirodno"),
            (74, "Priroda"),
            (75, "Fizika"),
            (76, "Drustveno"),
            (77, "Drustvo"),
            (78, "Kultura"),
            (79, "Dokumentarni"),
            (80, "Licno"),
            (81, "Dnevnici"),
            (82, "Filozofija"),
            (83, "Mesta"),
            (84, "Putovanja"),
            (85, "Odnosi"),
            (86, "Sport"),
            (87, "Bejzbol"),
            (88, "Kosarka"),
            (89, "Kriket"),
            (90, "Fantazi"),
            (91, "Americki fudbal"),
            (92, "Golf"),
            (93, "Hokej"),
            (94, "Ragbi"),
            (95, "Trcanje"),
            (96, "Fudbal"),
            (97, "Plivanje"),
            (98, "Tenis"),
            (99, "Odbojka"),
            (100, "Divljina"),
            (101, "Rvanje"),
            (102, "Tehnologija"),
            (103, "True Crime"),
            (104, "TV"),
            (105, "Film"),
            (106, "After-Shows"),
            (107, "Recenzije"),
            (108, "Klima"),
            (109, "Vreme"),
            (110, "Drustvene igre"),
            (111, "Igranje uloga"),
            (112, "Kriptovalute"),
        ],
        Language::Spanish => &[
            (1, "Arte"),
            (2, "Libros"),
            (3, "Diseno"),
            (4, "Moda"),
            (5, "Belleza"),
            (6, "Comida"),
            (7, "Artes escenicas"),
            (8, "Artes visuales"),
            (9, "Negocios"),
            (10, "Carreras"),
            (11, "Emprendimiento"),
            (12, "Inversion"),
            (13, "Gestion"),
            (14, "Marketing"),
            (15, "Sin fines de lucro"),
            (16, "Comedia"),
            (17, "Entrevistas"),
            (18, "Improvisacion"),
            (19, "Stand-Up"),
            (20, "Educacion"),
            (21, "Cursos"),
            (22, "Como hacer"),
            (23, "Idiomas"),
            (24, "Aprendizaje"),
            (25, "Desarrollo personal"),
            (26, "Ficcion"),
            (27, "Drama"),
            (28, "Historia"),
            (29, "Salud"),
            (30, "Fitness"),
            (31, "Salud alternativa"),
            (32, "Medicina"),
            (33, "Salud mental"),
            (34, "Nutricion"),
            (35, "Sexualidad"),
            (36, "Ninos"),
            (37, "Familia"),
            (38, "Crianza"),
            (39, "Mascotas"),
            (40, "Animales"),
            (41, "Cuentos"),
            (42, "Ocio"),
            (43, "Animacion"),
            (44, "Manga"),
            (45, "Automocion"),
            (46, "Aviacion"),
            (47, "Manualidades"),
            (48, "Juegos"),
            (49, "Pasatiempos"),
            (50, "Hogar"),
            (51, "Jardin"),
            (52, "Videojuegos"),
            (53, "Musica"),
            (54, "Comentarios"),
            (55, "Noticias"),
            (56, "Diario"),
            (57, "Entretenimiento"),
            (58, "Gobierno"),
            (59, "Politica"),
            (60, "Budismo"),
            (61, "Cristianismo"),
            (62, "Hinduismo"),
            (63, "Islam"),
            (64, "Judaismo"),
            (65, "Religion"),
            (66, "Espiritualidad"),
            (67, "Ciencia"),
            (68, "Astronomia"),
            (69, "Quimica"),
            (70, "Ciencias de la tierra"),
            (71, "Ciencias de la vida"),
            (72, "Matematicas"),
            (73, "Ciencias naturales"),
            (74, "Naturaleza"),
            (75, "Fisica"),
            (76, "Ciencias sociales"),
            (77, "Sociedad"),
            (78, "Cultura"),
            (79, "Documentales"),
            (80, "Personal"),
            (81, "Diarios personales"),
            (82, "Filosofia"),
            (83, "Lugares"),
            (84, "Viajes"),
            (85, "Relaciones"),
            (86, "Deportes"),
            (87, "Beisbol"),
            (88, "Baloncesto"),
            (89, "Cricket"),
            (90, "Fantasy"),
            (91, "Futbol americano"),
            (92, "Golf"),
            (93, "Hockey"),
            (94, "Rugby"),
            (95, "Correr"),
            (96, "Futbol"),
            (97, "Natacion"),
            (98, "Tenis"),
            (99, "Voleibol"),
            (100, "Naturaleza salvaje"),
            (101, "Lucha"),
            (102, "Tecnologia"),
            (103, "True Crime"),
            (104, "TV"),
            (105, "Cine"),
            (106, "After-show"),
            (107, "Criticas"),
            (108, "Clima"),
            (109, "Meteorologia"),
            (110, "Juegos de mesa"),
            (111, "Juego de roles"),
            (112, "Cryptocurrency"),
        ],
        Language::Portuguese => &[
            (1, "Artes"),
            (2, "Livros"),
            (3, "Design"),
            (4, "Moda"),
            (5, "Beleza"),
            (6, "Comida"),
            (7, "Artes performaticas"),
            (8, "Artes visuais"),
            (9, "Negocios"),
            (10, "Carreiras"),
            (11, "Empreendedorismo"),
            (12, "Investimentos"),
            (13, "Gestao"),
            (14, "Marketing"),
            (15, "Sem fins lucrativos"),
            (16, "Comedia"),
            (17, "Entrevistas"),
            (18, "Improvisacao"),
            (19, "Stand-Up"),
            (20, "Educacao"),
            (21, "Cursos"),
            (22, "Como fazer"),
            (23, "Idiomas"),
            (24, "Aprendizagem"),
            (25, "Desenvolvimento pessoal"),
            (26, "Ficcao"),
            (27, "Drama"),
            (28, "Historia"),
            (29, "Saude"),
            (30, "Fitness"),
            (31, "Saude alternativa"),
            (32, "Medicina"),
            (33, "Saude mental"),
            (34, "Nutricao"),
            (35, "Sexualidade"),
            (36, "Criancas"),
            (37, "Familia"),
            (38, "Parentalidade"),
            (39, "Animais de estimacao"),
            (40, "Animais"),
            (41, "Historias"),
            (42, "Lazer"),
            (43, "Animacao"),
            (44, "Manga"),
            (45, "Automotivo"),
            (46, "Aviacao"),
            (47, "Artesanato"),
            (48, "Jogos"),
            (49, "Hobbies"),
            (50, "Casa"),
            (51, "Jardim"),
            (52, "Jogos eletronicos"),
            (53, "Musica"),
            (54, "Comentarios"),
            (55, "Noticias"),
            (56, "Diario"),
            (57, "Entretenimento"),
            (58, "Governo"),
            (59, "Politica"),
            (60, "Budismo"),
            (61, "Cristianismo"),
            (62, "Hinduismo"),
            (63, "Islam"),
            (64, "Judaismo"),
            (65, "Religiao"),
            (66, "Espiritualidade"),
            (67, "Ciencia"),
            (68, "Astronomia"),
            (69, "Quimica"),
            (70, "Ciencias da terra"),
            (71, "Ciencias da vida"),
            (72, "Matematica"),
            (73, "Ciencias naturais"),
            (74, "Natureza"),
            (75, "Fisica"),
            (76, "Ciencias sociais"),
            (77, "Sociedade"),
            (78, "Cultura"),
            (79, "Documentarios"),
            (80, "Pessoal"),
            (81, "Diarios pessoais"),
            (82, "Filosofia"),
            (83, "Lugares"),
            (84, "Viagens"),
            (85, "Relacionamentos"),
            (86, "Desporto"),
            (87, "Beisebol"),
            (88, "Basquete"),
            (89, "Cricket"),
            (90, "Fantasy"),
            (91, "Futebol americano"),
            (92, "Golfe"),
            (93, "Hoquei"),
            (94, "Rugby"),
            (95, "Corrida"),
            (96, "Futebol"),
            (97, "Natacao"),
            (98, "Tenis"),
            (99, "Volei"),
            (100, "Natureza selvagem"),
            (101, "Luta"),
            (102, "Tecnologia"),
            (103, "True Crime"),
            (104, "TV"),
            (105, "Cinema"),
            (106, "After-show"),
            (107, "Criticas"),
            (108, "Clima"),
            (109, "Meteorologia"),
            (110, "Jogos de tabuleiro"),
            (111, "RPG"),
            (112, "Cryptocurrency"),
        ],
        Language::PortugueseBrazilian => &[
            (1, "Artes"),
            (2, "Livros"),
            (3, "Design"),
            (4, "Moda"),
            (5, "Beleza"),
            (6, "Gastronomia"),
            (7, "Artes cênicas"),
            (8, "Artes visuais"),
            (9, "Negócios"),
            (10, "Carreiras"),
            (11, "Empreendedorismo"),
            (12, "Investimentos"),
            (13, "Gestão"),
            (14, "Marketing"),
            (15, "Organizações sem fins lucrativos"),
            (16, "Comédia"),
            (17, "Entrevistas"),
            (18, "Improvisação"),
            (19, "Stand-Up"),
            (20, "Educação"),
            (21, "Cursos"),
            (22, "Como fazer"),
            (23, "Idiomas"),
            (24, "Aprendizagem"),
            (25, "Desenvolvimento pessoal"),
            (26, "Ficção"),
            (27, "Drama"),
            (28, "História"),
            (29, "Saúde"),
            (30, "Fitness"),
            (31, "Saúde alternativa"),
            (32, "Medicina"),
            (33, "Saúde mental"),
            (34, "Nutrição"),
            (35, "Sexualidade"),
            (36, "Crianças"),
            (37, "Família"),
            (38, "Maternidade e paternidade"),
            (39, "Animais de estimação"),
            (40, "Animais"),
            (41, "Histórias"),
            (42, "Lazer"),
            (43, "Animação"),
            (44, "Mangá"),
            (45, "Automotivo"),
            (46, "Aviação"),
            (47, "Artesanato"),
            (48, "Jogos"),
            (49, "Hobbies"),
            (50, "Casa"),
            (51, "Jardim"),
            (52, "Jogos eletrônicos"),
            (53, "Música"),
            (54, "Comentários"),
            (55, "Notícias"),
            (56, "Diário"),
            (57, "Entretenimento"),
            (58, "Governo"),
            (59, "Política"),
            (60, "Budismo"),
            (61, "Cristianismo"),
            (62, "Hinduismo"),
            (63, "Islã"),
            (64, "Judaísmo"),
            (65, "Religião"),
            (66, "Espiritualidade"),
            (67, "Ciência"),
            (68, "Astronomia"),
            (69, "Química"),
            (70, "Ciências da terra"),
            (71, "Ciências da vida"),
            (72, "Matemática"),
            (73, "Ciências naturais"),
            (74, "Natureza"),
            (75, "Física"),
            (76, "Ciências sociais"),
            (77, "Sociedade"),
            (78, "Cultura"),
            (79, "Documentários"),
            (80, "Pessoal"),
            (81, "Diários pessoais"),
            (82, "Filosofia"),
            (83, "Lugares"),
            (84, "Viagens"),
            (85, "Relacionamentos"),
            (86, "Esportes"),
            (87, "Beisebol"),
            (88, "Basquete"),
            (89, "Críquete"),
            (90, "Fantasia"),
            (91, "Futebol americano"),
            (92, "Golfe"),
            (93, "Hóquei"),
            (94, "Rugby"),
            (95, "Corrida"),
            (96, "Futebol"),
            (97, "Natação"),
            (98, "Tênis"),
            (99, "Vôlei"),
            (100, "Natureza selvagem"),
            (101, "Luta"),
            (102, "Tecnologia"),
            (103, "Crimes reais"),
            (104, "TV"),
            (105, "Cinema"),
            (106, "Pós-programa"),
            (107, "Críticas"),
            (108, "Clima"),
            (109, "Meteorologia"),
            (110, "Jogos de tabuleiro"),
            (111, "RPG"),
            (112, "Criptomoedas"),
        ],
        Language::Swedish => &[
            (1, "Konst"),
            (2, "Bocker"),
            (3, "Design"),
            (4, "Mode"),
            (5, "Skonhet"),
            (6, "Mat"),
            (7, "Scenkonst"),
            (8, "Visuell konst"),
            (9, "Foretag"),
            (10, "Karriar"),
            (11, "Entreprenorskap"),
            (12, "Investeringar"),
            (13, "Ledning"),
            (14, "Marknadsforing"),
            (15, "Ideell verksamhet"),
            (16, "Komedi"),
            (17, "Intervjuer"),
            (18, "Improvisation"),
            (19, "Stand-Up"),
            (20, "Utbildning"),
            (21, "Kurser"),
            (22, "Sa gor du"),
            (23, "Sprak"),
            (24, "Larande"),
            (25, "Sjalvutveckling"),
            (26, "Fiktion"),
            (27, "Drama"),
            (28, "Historia"),
            (29, "Halsa"),
            (30, "Fitness"),
            (31, "Alternativ halsa"),
            (32, "Medicin"),
            (33, "Mental halsa"),
            (34, "Naringslara"),
            (35, "Sexualitet"),
            (36, "Barn"),
            (37, "Familj"),
            (38, "Foraldraskap"),
            (39, "Husdjur"),
            (40, "Djur"),
            (41, "Berattelser"),
            (42, "Fritid"),
            (43, "Animation"),
            (44, "Manga"),
            (45, "Bil"),
            (46, "Flyg"),
            (47, "Hantverk"),
            (48, "Spel"),
            (49, "Hobbyer"),
            (50, "Hem"),
            (51, "Tradgard"),
            (52, "Datorspel"),
            (53, "Musik"),
            (54, "Kommentarer"),
            (55, "Nyheter"),
            (56, "Dagliga nyheter"),
            (57, "Underhallning"),
            (58, "Regering"),
            (59, "Politik"),
            (60, "Buddhism"),
            (61, "Kristendom"),
            (62, "Hinduism"),
            (63, "Islam"),
            (64, "Judendom"),
            (65, "Religion"),
            (66, "Andlighet"),
            (67, "Vetenskap"),
            (68, "Astronomi"),
            (69, "Kemi"),
            (70, "Geovetenskaper"),
            (71, "Livsvetenskaper"),
            (72, "Matematik"),
            (73, "Naturvetenskap"),
            (74, "Natur"),
            (75, "Fysik"),
            (76, "Samhallsvetenskaper"),
            (77, "Samhalle"),
            (78, "Kultur"),
            (79, "Dokumentar"),
            (80, "Personligt"),
            (81, "Dagbocker"),
            (82, "Filosofi"),
            (83, "Platser"),
            (84, "Resor"),
            (85, "Relationer"),
            (86, "Sport"),
            (87, "Baseboll"),
            (88, "Basket"),
            (89, "Cricket"),
            (90, "Fantasy"),
            (91, "Amerikansk fotboll"),
            (92, "Golf"),
            (93, "Hockey"),
            (94, "Rugby"),
            (95, "Lopning"),
            (96, "Fotboll"),
            (97, "Simning"),
            (98, "Tennis"),
            (99, "Volleyboll"),
            (100, "Vildmark"),
            (101, "Brottning"),
            (102, "Teknik"),
            (103, "True Crime"),
            (104, "TV"),
            (105, "Film"),
            (106, "Eftershow"),
            (107, "Recensioner"),
            (108, "Klimat"),
            (109, "Vader"),
            (110, "Bradspel"),
            (111, "Rollspel"),
            (112, "Cryptocurrency"),
        ],
        Language::Vietnamese => &[
            (1, "Nghe thuat"),
            (2, "Sach"),
            (3, "Thiet ke"),
            (4, "Thoi trang"),
            (5, "Lam dep"),
            (6, "Am thuc"),
            (7, "Nghe thuat bieu dien"),
            (8, "Nghe thuat thi giac"),
            (9, "Kinh doanh"),
            (10, "Nghe nghiep"),
            (11, "Khoi nghiep"),
            (12, "Dau tu"),
            (13, "Quan ly"),
            (14, "Tiep thi"),
            (15, "Phi loi nhuan"),
            (16, "Hai"),
            (17, "Phong van"),
            (18, "Ung tac"),
            (19, "Stand-Up"),
            (20, "Giao duc"),
            (21, "Khoa hoc truc tuyen"),
            (22, "Cach lam"),
            (23, "Ngon ngu"),
            (24, "Hoc tap"),
            (25, "Phat trien ban than"),
            (26, "Hu cau"),
            (27, "Drama"),
            (28, "Lich su"),
            (29, "Suc khoe"),
            (30, "The hinh"),
            (31, "Suc khoe thay the"),
            (32, "Y hoc"),
            (33, "Suc khoe tam than"),
            (34, "Dinh duong"),
            (35, "Tinh duc"),
            (36, "Tre em"),
            (37, "Gia dinh"),
            (38, "Nuoi day con"),
            (39, "Thu cung"),
            (40, "Dong vat"),
            (41, "Truyen"),
            (42, "Giai tri"),
            (43, "Hoat hinh"),
            (44, "Manga"),
            (45, "O to"),
            (46, "Hang khong"),
            (47, "Thu cong"),
            (48, "Tro choi"),
            (49, "So thich"),
            (50, "Nha"),
            (51, "Vuon"),
            (52, "Tro choi dien tu"),
            (53, "Am nhac"),
            (54, "Binh luan"),
            (55, "Tin tuc"),
            (56, "Tin hang ngay"),
            (57, "Giai tri"),
            (58, "Chinh phu"),
            (59, "Chinh tri"),
            (60, "Phat giao"),
            (61, "Kito giao"),
            (62, "An do giao"),
            (63, "Hoi giao"),
            (64, "Do thai"),
            (65, "Ton giao"),
            (66, "Tam linh"),
            (67, "Khoa hoc"),
            (68, "Thien van hoc"),
            (69, "Hoa hoc"),
            (70, "Khoa hoc trai dat"),
            (71, "Khoa hoc doi song"),
            (72, "Toan hoc"),
            (73, "Khoa hoc tu nhien"),
            (74, "Thien nhien"),
            (75, "Vat ly"),
            (76, "Khoa hoc xa hoi"),
            (77, "Xa hoi"),
            (78, "Van hoa"),
            (79, "Tai lieu"),
            (80, "Ca nhan"),
            (81, "Nhat ky"),
            (82, "Triet hoc"),
            (83, "Dia diem"),
            (84, "Du lich"),
            (85, "Moi quan he"),
            (86, "The thao"),
            (87, "Bong chay"),
            (88, "Bong ro"),
            (89, "Cricket"),
            (90, "Fantasy"),
            (91, "Bong bau duc"),
            (92, "Golf"),
            (93, "Hockey"),
            (94, "Rugby"),
            (95, "Chay bo"),
            (96, "Bong da"),
            (97, "Boi"),
            (98, "Quan vot"),
            (99, "Bong chuyen"),
            (100, "Hoang da"),
            (101, "Vat"),
            (102, "Cong nghe"),
            (103, "True Crime"),
            (104, "TV"),
            (105, "Phim"),
            (106, "After-show"),
            (107, "Danh gia"),
            (108, "Khi hau"),
            (109, "Thoi tiet"),
            (110, "Tro choi ban"),
            (111, "Nhap vai"),
            (112, "Cryptocurrency"),
        ],
        Language::Czech => &[
            (1, "Umeni"),
            (2, "Knihy"),
            (3, "Design"),
            (4, "Moda"),
            (5, "Krasa"),
            (6, "Jidlo"),
            (7, "Scenicka umeni"),
            (8, "Vytvarne umeni"),
            (9, "Byznys"),
            (10, "Kariera"),
            (11, "Podnikani"),
            (12, "Investovani"),
            (13, "Rizeni"),
            (14, "Marketing"),
            (15, "Neziskove organizace"),
            (16, "Komedie"),
            (17, "Rozhovory"),
            (18, "Improvizace"),
            (19, "Stand-Up"),
            (20, "Vzdelavani"),
            (21, "Kurzy"),
            (22, "Jak na to"),
            (23, "Jazyky"),
            (24, "Uceni"),
            (25, "Sebezlepseni"),
            (26, "Beletrie"),
            (27, "Drama"),
            (28, "Historie"),
            (29, "Zdravi"),
            (30, "Fitness"),
            (31, "Alternativni zdravi"),
            (32, "Medicina"),
            (33, "Dusevni zdravi"),
            (34, "Vyziva"),
            (35, "Sexualita"),
            (36, "Deti"),
            (37, "Rodina"),
            (38, "Rodicovstvi"),
            (39, "Domaci mazlicci"),
            (40, "Zvirata"),
            (41, "Pribehy"),
            (42, "Volny cas"),
            (43, "Animace"),
            (44, "Manga"),
            (45, "Automobily"),
            (46, "Letectvi"),
            (47, "Remesla"),
            (48, "Hry"),
            (49, "Zajmy"),
            (50, "Dum"),
            (51, "Zahrada"),
            (52, "Videohry"),
            (53, "Hudba"),
            (54, "Komentare"),
            (55, "Zpravy"),
            (56, "Denni zpravy"),
            (57, "Zabava"),
            (58, "Vlada"),
            (59, "Politika"),
            (60, "Buddhismus"),
            (61, "Krestanstvi"),
            (62, "Hinduismus"),
            (63, "Islam"),
            (64, "Judaismus"),
            (65, "Nabozenstvi"),
            (66, "Spiritualita"),
            (67, "Veda"),
            (68, "Astronomie"),
            (69, "Chemie"),
            (70, "Vedy o Zemi"),
            (71, "Zivotni vedy"),
            (72, "Matematika"),
            (73, "Prirodni vedy"),
            (74, "Priroda"),
            (75, "Fyzika"),
            (76, "Spolecenske vedy"),
            (77, "Spolecnost"),
            (78, "Kultura"),
            (79, "Dokument"),
            (80, "Osobni"),
            (81, "Deniky"),
            (82, "Filozofie"),
            (83, "Mista"),
            (84, "Cestovani"),
            (85, "Vztahy"),
            (86, "Sport"),
            (87, "Baseball"),
            (88, "Basketbal"),
            (89, "Kriket"),
            (90, "Fantasy"),
            (91, "Americky fotbal"),
            (92, "Golf"),
            (93, "Hokej"),
            (94, "Ragby"),
            (95, "Beh"),
            (96, "Fotbal"),
            (97, "Plavani"),
            (98, "Tenis"),
            (99, "Volejbal"),
            (100, "Divocina"),
            (101, "Zapas"),
            (102, "Technologie"),
            (103, "True Crime"),
            (104, "TV"),
            (105, "Film"),
            (106, "After-show"),
            (107, "Recenze"),
            (108, "Klima"),
            (109, "Pocasi"),
            (110, "Deskove hry"),
            (111, "Hrani roli"),
            (112, "Cryptocurrency"),
        ],
        Language::Polish => &[
            (1, "Sztuka"),
            (2, "Ksiazki"),
            (3, "Design"),
            (4, "Moda"),
            (5, "Uroda"),
            (6, "Jedzenie"),
            (7, "Sztuki widowiskowe"),
            (8, "Sztuki wizualne"),
            (9, "Biznes"),
            (10, "Kariera"),
            (11, "Przedsiebiorczosc"),
            (12, "Inwestowanie"),
            (13, "Zarzadzanie"),
            (14, "Marketing"),
            (15, "Non-profit"),
            (16, "Komedia"),
            (17, "Wywiady"),
            (18, "Improwizacja"),
            (19, "Stand-Up"),
            (20, "Edukacja"),
            (21, "Kursy"),
            (22, "Jak to zrobic"),
            (23, "Jezyki"),
            (24, "Nauka"),
            (25, "Rozwoj osobisty"),
            (26, "Fikcja"),
            (27, "Dramat"),
            (28, "Historia"),
            (29, "Zdrowie"),
            (30, "Fitness"),
            (31, "Zdrowie alternatywne"),
            (32, "Medycyna"),
            (33, "Zdrowie psychiczne"),
            (34, "Zywienie"),
            (35, "Seksualnosc"),
            (36, "Dzieci"),
            (37, "Rodzina"),
            (38, "Rodzicielstwo"),
            (39, "Zwierzeta domowe"),
            (40, "Zwierzeta"),
            (41, "Bajki"),
            (42, "Rozrywka"),
            (43, "Animacja"),
            (44, "Manga"),
            (45, "Motoryzacja"),
            (46, "Lotnictwo"),
            (47, "Rekodzielo"),
            (48, "Gry"),
            (49, "Hobby"),
            (50, "Dom"),
            (51, "Ogrod"),
            (52, "Gry wideo"),
            (53, "Muzyka"),
            (54, "Komentarze"),
            (55, "Wiadomosci"),
            (56, "Wiadomosci dnia"),
            (57, "Rozrywka"),
            (58, "Rzad"),
            (59, "Polityka"),
            (60, "Buddyzm"),
            (61, "Chrzescijanstwo"),
            (62, "Hinduizm"),
            (63, "Islam"),
            (64, "Judaizm"),
            (65, "Religia"),
            (66, "Duchowosc"),
            (67, "Nauka"),
            (68, "Astronomia"),
            (69, "Chemia"),
            (70, "Nauki o Ziemi"),
            (71, "Nauki o zyciu"),
            (72, "Matematyka"),
            (73, "Nauki przyrodnicze"),
            (74, "Natura"),
            (75, "Fizyka"),
            (76, "Nauki spoleczne"),
            (77, "Spoleczenstwo"),
            (78, "Kultura"),
            (79, "Dokument"),
            (80, "Osobiste"),
            (81, "Dzienniki"),
            (82, "Filozofia"),
            (83, "Miejsca"),
            (84, "Podroze"),
            (85, "Relacje"),
            (86, "Sport"),
            (87, "Baseball"),
            (88, "Koszykowka"),
            (89, "Krykiet"),
            (90, "Fantasy"),
            (91, "Futbol amerykanski"),
            (92, "Golf"),
            (93, "Hokej"),
            (94, "Rugby"),
            (95, "Bieganie"),
            (96, "Pilka nozna"),
            (97, "Plywanie"),
            (98, "Tenis"),
            (99, "Siatkowka"),
            (100, "Dzicz"),
            (101, "Zapasy"),
            (102, "Technologia"),
            (103, "True Crime"),
            (104, "TV"),
            (105, "Film"),
            (106, "After-show"),
            (107, "Recenzje"),
            (108, "Klimat"),
            (109, "Pogoda"),
            (110, "Gry planszowe"),
            (111, "Gry fabularne"),
            (112, "Cryptocurrency"),
        ],
        Language::French => &[
            (1, "Arts"),
            (2, "Livres"),
            (3, "Design"),
            (4, "Mode"),
            (5, "Beaute"),
            (6, "Cuisine"),
            (7, "Arts du spectacle"),
            (8, "Arts visuels"),
            (9, "Affaires"),
            (10, "Carrieres"),
            (11, "Entrepreneuriat"),
            (12, "Investissement"),
            (13, "Gestion"),
            (14, "Marketing"),
            (15, "Sans but lucratif"),
            (16, "Comedie"),
            (17, "Interviews"),
            (18, "Improvisation"),
            (19, "Stand-Up"),
            (20, "Education"),
            (21, "Cours"),
            (22, "Comment faire"),
            (23, "Langues"),
            (24, "Apprentissage"),
            (25, "Developpement personnel"),
            (26, "Fiction"),
            (27, "Drame"),
            (28, "Histoire"),
            (29, "Sante"),
            (30, "Fitness"),
            (31, "Sante alternative"),
            (32, "Medecine"),
            (33, "Sante mentale"),
            (34, "Nutrition"),
            (35, "Sexualite"),
            (36, "Enfants"),
            (37, "Famille"),
            (38, "Parentalite"),
            (39, "Animaux de compagnie"),
            (40, "Animaux"),
            (41, "Histoires"),
            (42, "Loisirs"),
            (43, "Animation"),
            (44, "Manga"),
            (45, "Automobile"),
            (46, "Aviation"),
            (47, "Artisanat"),
            (48, "Jeux"),
            (49, "Loisirs"),
            (50, "Maison"),
            (51, "Jardin"),
            (52, "Jeux video"),
            (53, "Musique"),
            (54, "Commentaires"),
            (55, "Actualites"),
            (56, "Actualites quotidiennes"),
            (57, "Divertissement"),
            (58, "Gouvernement"),
            (59, "Politique"),
            (60, "Bouddhisme"),
            (61, "Christianisme"),
            (62, "Hindouisme"),
            (63, "Islam"),
            (64, "Judaisme"),
            (65, "Religion"),
            (66, "Spiritualite"),
            (67, "Science"),
            (68, "Astronomie"),
            (69, "Chimie"),
            (70, "Sciences de la Terre"),
            (71, "Sciences de la vie"),
            (72, "Mathematiques"),
            (73, "Sciences naturelles"),
            (74, "Nature"),
            (75, "Physique"),
            (76, "Sciences sociales"),
            (77, "Societe"),
            (78, "Culture"),
            (79, "Documentaire"),
            (80, "Personnel"),
            (81, "Journaux"),
            (82, "Philosophie"),
            (83, "Lieux"),
            (84, "Voyages"),
            (85, "Relations"),
            (86, "Sports"),
            (87, "Baseball"),
            (88, "Basket-ball"),
            (89, "Cricket"),
            (90, "Fantasy"),
            (91, "Football americain"),
            (92, "Golf"),
            (93, "Hockey"),
            (94, "Rugby"),
            (95, "Course a pied"),
            (96, "Football"),
            (97, "Natation"),
            (98, "Tennis"),
            (99, "Volleyball"),
            (100, "Nature sauvage"),
            (101, "Lutte"),
            (102, "Technologie"),
            (103, "True Crime"),
            (104, "TV"),
            (105, "Cinema"),
            (106, "After-show"),
            (107, "Critiques"),
            (108, "Climat"),
            (109, "Meteo"),
            (110, "Jeux de plateau"),
            (111, "Jeu de role"),
            (112, "Cryptocurrency"),
        ],
    };
    entries
        .iter()
        .map(|&(id, name)| Category {
            id,
            name: name.to_string(),
        })
        .collect()
}

fn apple_search_by_category(genre_id: u32, country: &str, limit: u32) -> String {
    format!(
        "https://itunes.apple.com/search?media=podcast&entity=podcast&genreId={}&country={}&limit={}",
        genre_id, country, limit
    )
}

fn apple_search_in_category(term: &str, genre_id: u32, country: &str, limit: u32) -> String {
    format!(
        "https://itunes.apple.com/search?media=podcast&entity=podcast&term={}&genreId={}&country={}&limit={}",
        percent_encode(term),
        genre_id,
        country,
        limit
    )
}

fn apple_top_podcasts_by_genre(genre_id: u32, country: &str, limit: u32) -> String {
    format!(
        "https://itunes.apple.com/{}/rss/toppodcasts/limit={}/genre={}/json",
        country, limit, genre_id
    )
}

fn apple_lookup_by_ids(ids: &[u64], country: &str) -> Option<String> {
    if ids.is_empty() {
        return None;
    }
    let joined = ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<String>>()
        .join(",");
    Some(format!(
        "https://itunes.apple.com/lookup?id={}&country={}",
        joined, country
    ))
}

fn podcastindex_credentials_or_prompt(hwnd: HWND, parent: HWND) -> Option<(String, String)> {
    let (user_key, user_secret) = {
        with_state(parent, |ps| {
            (
                ps.settings.podcast_index_api_key.clone(),
                settings::decrypt_podcast_index_secret(&ps.settings.podcast_index_api_secret),
            )
        })
    }
    .unwrap_or((String::new(), None));

    let key = user_key.trim().to_string();
    let secret = user_secret.unwrap_or_default();
    let missing = key.trim().is_empty() || secret.trim().is_empty();
    if missing {
        let language = { with_state(parent, |ps| ps.settings.language) }.unwrap_or_default();
        let title = i18n::tr(language, "podcasts.podcastindex.missing_title");
        let body = i18n::tr(language, "podcasts.podcastindex.missing_body");
        let response = crate::message_box_w_safe(
            hwnd,
            PCWSTR(to_wide(&body).as_ptr()),
            PCWSTR(to_wide(&title).as_ptr()),
            MB_YESNO | MB_ICONINFORMATION,
        );
        if response == IDYES
            && let Err(e) =
                crate::audio_utils::open_url_in_browser("https://api.podcastindex.org/signup")
        {
            crate::log_debug(&format!("Error: {:?}", e));
        }
        return None;
    }
    Some((key, secret))
}

fn podcastindex_client() -> Result<reqwest::Client, String> {
    use std::time::Duration;

    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

fn add_podcastindex_auth_headers(
    rb: reqwest::RequestBuilder,
    api_key: &str,
    api_secret: &str,
) -> reqwest::RequestBuilder {
    let auth_date = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();
    let mut hasher = Sha1::new();
    hasher.update(format!("{api_key}{api_secret}{auth_date}").as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    rb.header("User-Agent", "Sonarpad")
        .header("X-Auth-Date", auth_date)
        .header("X-Auth-Key", api_key)
        .header("Authorization", hash)
}

#[cfg(test)]
mod tests {
    use super::{ensure_opml_extension, parse_opml_sources};
    use std::path::PathBuf;

    #[test]
    fn parse_opml_sources_accepts_type_rss_outline() {
        let text = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="1.0">
<head>
<title>Podcasts</title>
</head>
<body>
  <outline text="Example Podcast" title="Example Podcast" type="rss" xmlUrl="https://example.com/feed.xml" />
</body>
</opml>"#;

        let sources = parse_opml_sources(text);

        assert_eq!(
            sources,
            vec![(
                "Example Podcast".to_string(),
                "https://example.com/feed.xml".to_string()
            )]
        );
    }

    #[test]
    fn ensure_opml_extension_adds_missing_extension() {
        assert_eq!(
            ensure_opml_extension(PathBuf::from("Sonarpad Podcast")),
            PathBuf::from("Sonarpad Podcast.opml")
        );
    }

    #[test]
    fn ensure_opml_extension_keeps_existing_extension() {
        assert_eq!(
            ensure_opml_extension(PathBuf::from("Sonarpad Podcast.opml")),
            PathBuf::from("Sonarpad Podcast.opml")
        );
    }
}

#[cfg(test)]
mod spreaker_search_tests {
    use super::*;

    #[test]
    fn exact_match_handles_null_author_and_rejects_renamed_show() {
        let bytes = br#"{"response":{"show":{"show_id":5587296,"title":"Almanacco",
            "author_name":null,"author":{"fullname":"Radio Taormina"}}}}"#;
        let found = parse_spreaker_exact(bytes, "almanacco").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].artist, "Radio Taormina");
        assert!(found[0].feed_url.contains("/5587296/"));
        assert!(
            parse_spreaker_exact(bytes, "different-title")
                .unwrap()
                .is_empty()
        );
        assert!(parse_spreaker_exact(b"not json", "almanacco").is_err());
    }

    #[test]
    fn duplicate_feeds_removed_but_distinct_homonyms_kept() {
        let mut results = vec![];
        for id in [1, 2, 1] {
            results.extend(spreaker_items_to_results(vec![SpreakerShowItem {
                show_id: id,
                title: "Almanacco".into(),
                author_name: None,
                author: None,
            }]));
        }
        dedup_podcast_search_results(&mut results);
        assert_eq!(results.len(), 2);
        assert_eq!(
            podcast_title_key(" L'Almanacco del GIORNO "),
            "l-almanacco-del-giorno"
        );
    }

    #[test]
    #[ignore = "requires live Spreaker service"]
    fn live_almanacco_exact_lookup() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let key = podcast_title_key("Almanacco");
        let url = format!("https://api.spreaker.com/v2/shows/{}", percent_encode(&key));
        let bytes = rt
            .block_on(rss::fetch_url_bytes(&url, Default::default()))
            .unwrap();
        let found = parse_spreaker_exact(&bytes, &key).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].artist, "Radio Taormina");
        let feed = rt
            .block_on(rss::fetch_url_bytes(&found[0].feed_url, Default::default()))
            .unwrap();
        assert!(String::from_utf8_lossy(&feed).contains("<enclosure"));
    }
}
