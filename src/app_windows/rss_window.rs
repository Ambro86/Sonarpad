use crate::accessibility::{nvda_speak, to_wide};
use crate::app_windows::{help_window, youtube_transcript_window};
use crate::settings::{ListDateDisplayMode, ListTimeDisplayMode};

use crate::editor_manager;
use crate::i18n;
use crate::log_debug;
use crate::tools::rss::{self, RssFeedCache, RssItem, RssSource, RssSourceType};
use crate::with_state;
use chrono::{Local, NaiveDate, TimeZone};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::mem;
use std::path::{Path, PathBuf};
use std::time::Duration;
use url::Url;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::Controls::Dialogs::{
    OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};
use windows::Win32::UI::Controls::RichEdit::MSFTEDIT_CLASS;
use windows::Win32::UI::Controls::{
    EM_SETREADONLY, NM_RCLICK, NMHDR, NMTREEVIEWW, NMTVKEYDOWN, TVE_EXPAND, TVGN_CARET, TVGN_CHILD,
    TVGN_NEXT, TVGN_PARENT, TVGN_ROOT, TVHITTESTINFO, TVI_FIRST, TVI_LAST, TVI_ROOT, TVIF_PARAM,
    TVIF_TEXT, TVINSERTSTRUCTW, TVINSERTSTRUCTW_0, TVITEMEXW_CHILDREN, TVITEMW, TVM_DELETEITEM,
    TVM_ENSUREVISIBLE, TVM_EXPAND, TVM_GETITEMW, TVM_GETNEXTITEM, TVM_HITTEST, TVM_INSERTITEMW,
    TVM_SELECTITEM, TVM_SETITEMW, TVN_ITEMEXPANDINGW, TVN_KEYDOWN, TVN_SELCHANGEDW, WC_BUTTON,
    WC_COMBOBOXW, WC_LISTBOXW, WC_STATIC,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, GetKeyState, IsWindowEnabled, SetActiveWindow, SetFocus, VK_APPS, VK_CONTROL,
    VK_DOWN, VK_ESCAPE, VK_F10, VK_MENU, VK_RETURN, VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, BS_DEFPUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBN_SELCHANGE,
    CHILDID_SELF, CREATESTRUCTW, CW_USEDEFAULT, CallWindowProcW, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyMenu, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, EVENT_OBJECT_FOCUS,
    GWLP_USERDATA, GWLP_WNDPROC, GetCursorPos, GetDlgItem, GetParent, GetWindowLongPtrW,
    GetWindowRect, GetWindowTextLengthW, GetWindowTextW, HMENU, IDYES, KillTimer, LB_ADDSTRING,
    LB_GETCURSEL, LB_RESETCONTENT, LBN_DBLCLK, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK,
    MB_YESNOCANCEL, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, MessageBoxW, MoveWindow,
    OBJID_CLIENT, PostMessageW, RegisterClassW, SB_TOP, SW_HIDE, SW_SHOW, SendMessageW,
    SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowTextW, ShowWindow, TrackPopupMenu,
    WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_CREATE, WM_DESTROY, WM_KEYDOWN,
    WM_NCDESTROY, WM_NEXTDLGCTL, WM_NOTIFY, WM_NULL, WM_SETFOCUS, WM_SETFONT, WM_SETREDRAW,
    WM_SYSKEYDOWN, WM_TIMER, WM_USER, WM_VSCROLL, WNDCLASSW, WNDPROC, WS_CAPTION, WS_CHILD,
    WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
    WS_VSCROLL,
};
use windows::core::{PCWSTR, PWSTR, w};

const RSS_WINDOW_CLASS: &str = "SonarpadRssWindow";
const RSS_CITY_WINDOW_CLASS: &str = "SonarpadRssCityWindow";
const RSS_FOLDER_WINDOW_CLASS: &str = "SonarpadRssFolderWindow";
const RSS_COMMUNITY_ADD_WINDOW_CLASS: &str = "SonarpadRssCommunityAddWindow";
const RSS_COMMUNITY_LIST_WINDOW_CLASS: &str = "SonarpadRssCommunityListWindow";
const ID_CITY_EDIT: usize = 1601;
const ID_CITY_OK: usize = 1602;
const ID_CITY_CANCEL: usize = 1603;
const ID_COMMUNITY_ADD_NAME: usize = 1701;
const ID_COMMUNITY_ADD_URL: usize = 1702;
const ID_COMMUNITY_ADD_SUBMIT: usize = 1703;
const ID_COMMUNITY_ADD_CANCEL: usize = 1704;
const ID_COMMUNITY_LIST: usize = 1801;
const ID_COMMUNITY_LIST_ADD: usize = 1802;
const ID_COMMUNITY_LIST_CLOSE: usize = 1803;
const ID_FOLDER_EDIT: usize = 1901;
const ID_FOLDER_OK: usize = 1902;
const ID_FOLDER_CANCEL: usize = 1903;

#[inline]
fn ignore_bool(_value: bool) {}
const ID_TREE: usize = 1001;
const ID_BTN_ADD: usize = 1002;
const ID_BTN_CLOSE: usize = 1003;
const ID_BTN_IMPORT: usize = 1004;
const ID_BTN_EXPORT: usize = 1005;
const ID_BTN_SEARCH: usize = 1006;
const ID_EDIT_ARTICLE_PREVIEW: usize = 1007;
const ID_COMBO_NEWS_LANGUAGE: usize = 1008;
const ID_BTN_COMMUNITY_ADD: usize = 1009;
const ID_BTN_COMMUNITY_BROWSE: usize = 1010;
const ID_CTX_EDIT: usize = 1101;
const ID_CTX_DELETE: usize = 1102;
const ID_CTX_RETRY: usize = 1103;
const ID_CTX_UNDO_DELETE: usize = 1104;
const ID_CTX_REORDER_UP: usize = 1301;
const ID_CTX_REORDER_DOWN: usize = 1302;
const ID_CTX_REORDER_TOP: usize = 1303;
const ID_CTX_REORDER_BOTTOM: usize = 1304;
const ID_CTX_REORDER_POSITION: usize = 1305;
const ID_CTX_SORT_ASC: usize = 1306;
const ID_CTX_SORT_DESC: usize = 1307;
const ID_CTX_SORT_NEWEST: usize = 1308;
const ID_CTX_SORT_OLDEST: usize = 1309;
const ID_CTX_MOVE_FOLDER_BASE: usize = 2000;
const ID_CTX_MOVE_FOLDER_LIMIT: usize = 1000;
const ID_CTX_MOVE_FOLDER_END: usize = ID_CTX_MOVE_FOLDER_BASE + ID_CTX_MOVE_FOLDER_LIMIT - 1;
const ID_CTX_OPEN_BROWSER: usize = 1201;
const ID_CTX_SHARE_FACEBOOK: usize = 1202;
const ID_CTX_SHARE_TWITTER: usize = 1203;
const ID_CTX_SHARE_WHATSAPP: usize = 1204;
const ID_CTX_SHARE_EMAIL: usize = 1205;
const ID_CTX_PROPERTIES: usize = 1206;
const ID_CTX_ADD_TO_FAVORITES: usize = 1207;
const ID_CTX_CHANGE_CITY: usize = 1208;
const ID_CTX_SELECT_ARTICLES: usize = 1209;
const ID_CTX_CREATE_FOLDER: usize = 1210;

const WM_RSS_FETCH_COMPLETE: u32 = WM_USER + 200;
const WM_RSS_IMPORT_COMPLETE: u32 = WM_USER + 201;
const WM_SHOW_ADD_DIALOG: u32 = WM_USER + 202;
const WM_CLEAR_ENTER_GUARD: u32 = WM_USER + 203;
const WM_CLEAR_ADD_GUARD: u32 = WM_USER + 204;
pub(crate) const WM_RSS_SHOW_CONTEXT: u32 = WM_USER + 205;
const WM_RSS_BACKGROUND_CHECK_COMPLETE: u32 = WM_USER + 206;
const WM_RSS_MARK_ITEM_READ_UI: u32 = WM_USER + 207;
const WM_RSS_SELECT_SOURCE_DELAYED: u32 = WM_USER + 208;
const WM_RSS_PREVIEW_COMPLETE: u32 = WM_USER + 209;
const WM_RSS_CITY_CHANGED: u32 = WM_USER + 210;
const WM_RSS_COMMUNITY_SUBMIT_COMPLETE: u32 = WM_USER + 211;
const WM_RSS_COMMUNITY_LIST_COMPLETE: u32 = WM_USER + 212;
const ADD_GUARD_TIMER_ID: usize = 1;
const RSS_LANGUAGE_FOCUS_TIMER_ID: usize = 2;
const RSS_LANGUAGE_FOCUS_DELAY_MS: u32 = 2_000;
const EM_REPLACESEL: u32 = 0x00C2;
const REORDER_EDIT_ID: usize = 1401;
const REORDER_OK_ID: usize = 1402;
const REORDER_CANCEL_ID: usize = 1403;
const EM_SCROLLCARET: u32 = 0x00B7;
const SEARCH_EDIT_ID: usize = 1501;
const SEARCH_OK_ID: usize = 1502;
const SEARCH_CANCEL_ID: usize = 1503;

const FEED_EN_DATA: &str = include_str!("../../i18n/feed_en.txt");
const FEED_DE_DATA: &str = include_str!("../../i18n/feed_de.txt");
const FEED_UK_DATA: &str = include_str!("../../i18n/feed_uk.txt");
const FEED_IT_DATA: &str = include_str!("../../i18n/feed_it.txt");
const FEED_ES_DATA: &str = include_str!("../../i18n/feed_es.txt");
const FEED_PT_DATA: &str = include_str!("../../i18n/feed_pt.txt");
const FEED_PT_BR_DATA: &str = include_str!("../../i18n/feed_pt_BR.txt");
const FEED_VI_DATA: &str = include_str!("../../i18n/feed_vi.txt");
const FEED_CS_DATA: &str = include_str!("../../i18n/feed_cs.txt");
const FEED_PL_DATA: &str = include_str!("../../i18n/feed_pl.txt");
const FEED_FR_DATA: &str = include_str!("../../i18n/feed_fr.txt");
const FEED_SR_DATA: &str = include_str!("../../i18n/feed_sr HR.txt");
const FEED_RU_DATA: &str = include_str!("../../i18n/feed_ru.txt");
const FEED_HI_DATA: &str = include_str!("../../i18n/feed_hi.txt");
const EM_SETSEL: u32 = 0x00B1;
const EM_LIMITTEXT: u32 = 0x00C5;
const INITIAL_LOAD_COUNT: usize = 5;
const LOAD_MORE_COUNT: usize = 5;
const RSS_FAVORITES_SOURCE_URL: &str = "sonarpad://rss/favorites";
const COMMUNITY_NEWS_SOURCES_URL: &str = "https://sonarpad.com/api/get_community_news_sources.php";
const ADD_COMMUNITY_NEWS_SOURCE_URL: &str =
    "https://sonarpad.com/api/add_community_news_source.php";
const COMMUNITY_USER_AGENT: &str = "SonarpadWindows/0.7 (https://sonarpad.com)";
const NEWS_LANGUAGE_CODES: [&str; 9] = ["it", "en", "de", "fr", "es", "pt", "pt-br", "pl", "cs"];

fn default_news_language_code(language: crate::settings::Language) -> &'static str {
    match language {
        crate::settings::Language::German => "de",
        crate::settings::Language::English => "en",
        crate::settings::Language::French => "fr",
        crate::settings::Language::Spanish => "es",
        crate::settings::Language::Portuguese => "pt",
        crate::settings::Language::PortugueseBrazilian => "pt-br",
        crate::settings::Language::Polish => "pl",
        crate::settings::Language::Czech => "cs",
        _ => "it",
    }
}

fn normalize_news_language_code(code: &str) -> Option<&'static str> {
    NEWS_LANGUAGE_CODES
        .iter()
        .copied()
        .find(|candidate| candidate.eq_ignore_ascii_case(code.trim()))
}

fn news_language_as_app_language(code: &str) -> crate::settings::Language {
    match normalize_news_language_code(code).unwrap_or("it") {
        "en" => crate::settings::Language::English,
        "de" => crate::settings::Language::German,
        "fr" => crate::settings::Language::French,
        "es" => crate::settings::Language::Spanish,
        "pt" => crate::settings::Language::Portuguese,
        "pt-br" => crate::settings::Language::PortugueseBrazilian,
        "pl" => crate::settings::Language::Polish,
        "cs" => crate::settings::Language::Czech,
        _ => crate::settings::Language::Italian,
    }
}

fn news_language_label(ui_language: crate::settings::Language, code: &str) -> String {
    i18n::tr(
        ui_language,
        &format!("options.lang.{}", code.replace('-', "_")),
    )
}

fn active_news_language_code_from_state(state: &crate::AppState) -> String {
    normalize_news_language_code(&state.settings.rss_news_language)
        .unwrap_or_else(|| default_news_language_code(state.settings.language))
        .to_string()
}

fn sync_active_rss_sources(state: &mut crate::AppState) {
    let code = active_news_language_code_from_state(state);
    state
        .settings
        .rss_sources_by_language
        .insert(code, state.settings.rss_sources.clone());
}

fn normalized_folder_path(path: &[String]) -> Vec<String> {
    path.iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn add_folder_path_with_parents(paths: &mut Vec<Vec<String>>, path: &[String]) {
    let normalized = normalized_folder_path(path);
    for depth in 1..=normalized.len() {
        let prefix = normalized[..depth].to_vec();
        if !paths.contains(&prefix) {
            paths.push(prefix);
        }
    }
}

fn rss_folder_paths_for_settings(settings: &crate::settings::AppSettings) -> Vec<Vec<String>> {
    let code = normalize_news_language_code(&settings.rss_news_language)
        .unwrap_or_else(|| default_news_language_code(settings.language));
    let mut paths = settings
        .rss_folders_by_language
        .get(code)
        .cloned()
        .unwrap_or_default();
    for source in &settings.rss_sources {
        add_folder_path_with_parents(&mut paths, &source.folder_path);
    }
    paths
}

fn sync_active_rss_folders(state: &mut crate::AppState) {
    let code = active_news_language_code_from_state(state);
    let mut paths = state
        .settings
        .rss_folders_by_language
        .get(&code)
        .cloned()
        .unwrap_or_default();
    for source in &state.settings.rss_sources {
        add_folder_path_with_parents(&mut paths, &source.folder_path);
    }
    state.settings.rss_folders_by_language.insert(code, paths);
}

fn load_or_migrate_active_rss_sources(
    settings: &mut crate::settings::AppSettings,
    code: &str,
) -> bool {
    if let Some(active) = settings.rss_sources_by_language.get(code).cloned() {
        settings.rss_sources = active;
        false
    } else {
        settings
            .rss_sources_by_language
            .insert(code.to_string(), settings.rss_sources.clone());
        true
    }
}

fn save_rss_settings(state: &mut crate::AppState) {
    sync_active_rss_sources(state);
    sync_active_rss_folders(state);
    crate::settings::save_settings(state.settings.clone());
}

fn prepare_rss_language_state(parent: HWND) {
    with_state(parent, |state| {
        let code = active_news_language_code_from_state(state);
        let mut changed = false;
        if state.settings.rss_news_language != code {
            state.settings.rss_news_language = code.clone();
            changed = true;
        }
        if load_or_migrate_active_rss_sources(&mut state.settings, &code) {
            changed = true;
        }
        if changed {
            save_rss_settings(state);
        }
    });
}

fn active_news_language_code(parent: HWND) -> String {
    with_state(parent, |state| active_news_language_code_from_state(state))
        .unwrap_or_else(|| "it".to_string())
}

fn switch_news_language(hwnd: HWND, code: &str) {
    let Some(code) = normalize_news_language_code(code) else {
        return;
    };
    let parent = with_rss_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }
    let changed = with_state(parent, |state| {
        let old_code = active_news_language_code_from_state(state);
        if old_code == code {
            return false;
        }
        state
            .settings
            .rss_sources_by_language
            .insert(old_code, state.settings.rss_sources.clone());
        state.settings.rss_news_language = code.to_string();
        state.settings.rss_sources = state
            .settings
            .rss_sources_by_language
            .get(code)
            .cloned()
            .unwrap_or_default();
        save_rss_settings(state);
        true
    })
    .unwrap_or(false);
    if !changed {
        return;
    }
    ensure_default_sources(parent);
    ensure_favorites_source(parent);
    reload_tree(hwnd);
    let hwnd_tree = with_rss_state(hwnd, |state| state.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 != 0 {
        select_first_root_if_needed(hwnd, hwnd_tree);
    }

    let hwnd_language_combo =
        with_rss_state(hwnd, |state| state.hwnd_language_combo).unwrap_or(HWND(0));
    if hwnd_language_combo.0 != 0 {
        crate::set_focus_safe(hwnd_language_combo);
    }

    if unsafe {
        SetTimer(
            hwnd,
            RSS_LANGUAGE_FOCUS_TIMER_ID,
            RSS_LANGUAGE_FOCUS_DELAY_MS,
            None,
        )
    } == 0
    {
        crate::log_debug("Failed to set RSS language focus timer");
        if hwnd_tree.0 != 0 {
            crate::set_focus_safe(hwnd_tree);
        }
    }
    start_background_unread_check(hwnd);
}

// Normalize article text before sending it to the editor:
// - collapse multiple blank lines to a single blank line
// - replace embedded NULs (which would truncate Win32 edit text)
fn normalize_article_text(s: &str) -> String {
    let decoded = decode_basic_html_entities(s);
    let no_nul: String = decoded
        .chars()
        .map(|c| if c == '\0' { ' ' } else { c })
        .collect();
    collapse_blank_lines(&no_nul)
}

fn remove_favorite_article_by_key(items: &mut Vec<RssItem>, key: &str) -> bool {
    let before = items.len();
    items.retain(|item| rss_item_key(item) != key);
    items.len() != before
}

fn restore_favorite_article_at(
    items: &mut Vec<RssItem>,
    item: RssItem,
    key: &str,
    position: usize,
) -> bool {
    if items.iter().any(|entry| rss_item_key(entry) == key) {
        return false;
    }
    let insert_at = position.min(items.len());
    items.insert(insert_at, item);
    true
}

#[cfg(test)]
mod tests {
    use super::{
        ReorderAction, add_folder_path_with_parents, build_google_news_rss_url,
        decode_basic_html_entities, ensure_opml_extension, format_google_news_source_title,
        load_default_feeds, load_or_migrate_active_rss_sources, merge_imported_opml_sources,
        move_folder_destinations, move_rss_source_to_folder, normalize_rss_url_key,
        parse_opml_folder_paths, parse_opml_sources, remove_favorite_article_by_key,
        reorder_rss_source_within_folder, restore_favorite_article_at,
        rss_folder_paths_for_settings, write_sources_to_opml, write_sources_to_opml_with_folders,
    };
    use crate::tools::rss::{RssFeedCache, RssItem, RssSource, RssSourceType};
    use std::path::PathBuf;

    fn test_rss_source(title: &str, url: &str) -> RssSource {
        RssSource {
            title: title.to_string(),
            url: url.to_string(),
            kind: RssSourceType::Feed,
            folder_path: Vec::new(),
            user_title: true,
            unread: false,
            cache: RssFeedCache::default(),
            last_seen_guid: None,
            last_updated: None,
            removed_item_keys: Vec::new(),
            read_item_keys: Vec::new(),
        }
    }

    fn test_rss_item(title: &str, guid: &str) -> RssItem {
        RssItem {
            title: title.to_string(),
            guid: guid.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn deleting_rss_favorite_removes_it_from_persistent_collection() {
        let first = test_rss_item("Primo", "favorite-1");
        let second = test_rss_item("Secondo", "favorite-2");
        let mut items = vec![first, second.clone()];

        assert!(remove_favorite_article_by_key(&mut items, "favorite-1"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].guid, second.guid);
    }

    #[test]
    fn undo_rss_favorite_delete_restores_it_without_duplicates() {
        let first = test_rss_item("Primo", "favorite-1");
        let second = test_rss_item("Secondo", "favorite-2");
        let mut items = vec![second.clone()];

        assert!(restore_favorite_article_at(
            &mut items,
            first.clone(),
            "favorite-1",
            0
        ));
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].guid, first.guid);
        assert_eq!(items[1].guid, second.guid);
        assert!(!restore_favorite_article_at(
            &mut items,
            first,
            "favorite-1",
            0
        ));
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn rss_language_migration_preserves_current_sources_when_bucket_is_missing() {
        let custom = test_rss_source("Fonte pessoal", "https://example.com/pessoal.xml");
        let english = test_rss_source("English", "https://example.com/en.xml");
        let mut settings = crate::settings::AppSettings {
            rss_sources: vec![custom.clone()],
            rss_sources_by_language: [("en".to_string(), vec![english])].into_iter().collect(),
            ..Default::default()
        };

        let changed = load_or_migrate_active_rss_sources(&mut settings, "pt");

        assert!(changed);
        assert_eq!(settings.rss_sources, vec![custom]);
        assert_eq!(
            settings.rss_sources_by_language.get("pt"),
            Some(&settings.rss_sources)
        );
    }

    #[test]
    fn rss_language_migration_loads_existing_language_bucket() {
        let legacy = test_rss_source("Legacy", "https://example.com/legacy.xml");
        let portuguese = test_rss_source("Português", "https://example.com/pt.xml");
        let mut settings = crate::settings::AppSettings {
            rss_sources: vec![legacy],
            rss_sources_by_language: [("pt".to_string(), vec![portuguese.clone()])]
                .into_iter()
                .collect(),
            ..Default::default()
        };

        let changed = load_or_migrate_active_rss_sources(&mut settings, "pt");

        assert!(!changed);
        assert_eq!(settings.rss_sources, vec![portuguese]);
    }

    #[test]
    fn normalize_rss_url_key_keeps_query_parameters() {
        let key = normalize_rss_url_key("https://servis.idnes.cz/rss.aspx?c=kultura");
        assert_eq!(key, "servis.idnes.cz/rss.aspx?c=kultura");
    }

    #[test]
    fn normalize_rss_url_key_removes_fragment_only() {
        let key = normalize_rss_url_key("https://servis.idnes.cz/rss.aspx?c=technet#section");
        assert_eq!(key, "servis.idnes.cz/rss.aspx?c=technet");
    }

    #[test]
    fn german_default_feeds_include_requested_german_language_newspapers() {
        let feeds = load_default_feeds(crate::settings::Language::German);
        let expected = [
            (
                "Deutsche Welle Deutsch",
                "https://rss.dw.com/xml/rss-de-all",
            ),
            (
                "Süddeutsche Zeitung",
                "https://rss.sueddeutsche.de/rss/Topthemen",
            ),
            (
                "Frankfurter Allgemeine Zeitung",
                "https://www.faz.net/rss/aktuell/",
            ),
            ("Neue Zürcher Zeitung", "https://www.nzz.ch/recent.rss"),
            ("DER STANDARD", "https://www.derstandard.at/rss"),
        ];

        for (expected_title, expected_url) in expected {
            assert!(
                feeds
                    .iter()
                    .any(|(title, url)| title == expected_title && url == expected_url),
                "missing German RSS source: {expected_title} ({expected_url})"
            );
        }
    }

    #[test]
    fn google_news_search_uses_selected_english_rss_language() {
        let url = build_google_news_rss_url("technology", "en");
        assert!(url.contains("hl=en"));
        assert!(url.contains("gl=US"));
        assert!(url.contains("ceid=US:en"));
    }

    #[test]
    fn google_news_search_uses_selected_italian_rss_language() {
        let url = build_google_news_rss_url("tecnologia", "it");
        assert!(url.contains("hl=it"));
        assert!(url.contains("gl=IT"));
        assert!(url.contains("ceid=IT:it"));
    }

    #[test]
    fn google_news_search_uses_selected_german_rss_language() {
        let url = build_google_news_rss_url("technologie", "de");
        assert!(url.contains("hl=de"));
        assert!(url.contains("gl=DE"));
        assert!(url.contains("ceid=DE:de"));
    }

    #[test]
    fn google_news_search_uses_brazilian_portuguese_locale() {
        let url = build_google_news_rss_url("tecnologia", "pt-br");
        assert!(url.contains("hl=pt-BR"));
        assert!(url.contains("gl=BR"));
        assert!(url.contains("ceid=BR:pt-419"));
    }

    #[test]
    fn normalize_rss_url_key_normalizes_scheme_case_and_trailing_slash() {
        let key = normalize_rss_url_key("HTTP://EXAMPLE.COM/Feed/");
        assert_eq!(key, "example.com/feed");
    }

    #[test]
    fn format_google_news_source_title_capitalizes_each_word() {
        let title = format_google_news_source_title("elon musk");
        assert_eq!(title, "Elon Musk");
    }

    #[test]
    fn decode_basic_html_entities_decodes_common_italian_entities() {
        let text = "cos&igrave; si &egrave; visto, &laquo;ok&raquo;";
        let decoded = decode_basic_html_entities(text);
        assert_eq!(decoded, "così si è visto, «ok»");
    }

    #[test]
    fn ensure_opml_extension_adds_missing_extension() {
        assert_eq!(
            ensure_opml_extension(PathBuf::from("Sonarpad Rss")),
            PathBuf::from("Sonarpad Rss.opml")
        );
    }

    #[test]
    fn ensure_opml_extension_keeps_existing_extension() {
        assert_eq!(
            ensure_opml_extension(PathBuf::from("Sonarpad Rss.opml")),
            PathBuf::from("Sonarpad Rss.opml")
        );
    }

    #[test]
    fn arturo_style_opml_preserves_outline_folders() {
        let opml = r#"<?xml version="1.0" encoding="utf-8"?>
<opml version="2.0">
  <head><title>Medios españoles — OPML para lector RSS</title></head>
  <body>
    <outline text="01 — Actualidad y generalistas" title="01 — Actualidad y generalistas">
      <outline type="rss" text="EL PAÍS" title="EL PAÍS" xmlUrl="https://feeds.elpais.com/mrss-s/pages/ep/site/elpais.com/portada"/>
      <outline type="rss" text="El Confidencial — España" title="El Confidencial — España" xmlUrl="https://rss.elconfidencial.com/espana/"/>
    </outline>
    <outline text="05 — Tecnología, IA e internet" title="05 — Tecnología, IA e internet">
      <outline type="rss" text="Xataka" title="Xataka" xmlUrl="https://www.xataka.com/index.xml"/>
    </outline>
  </body>
</opml>"#;

        let sources = parse_opml_sources(opml);

        assert_eq!(sources.len(), 3);
        assert_eq!(
            sources[0].folder_path,
            vec!["01 — Actualidad y generalistas".to_string()]
        );
        assert_eq!(sources[0].title, "EL PAÍS");
        assert_eq!(sources[1].folder_path, sources[0].folder_path);
        assert_eq!(
            sources[2].folder_path,
            vec!["05 — Tecnología, IA e internet".to_string()]
        );
        assert_eq!(sources[2].title, "Xataka");
    }

    #[test]
    fn opml_parser_keeps_flat_feeds_at_root() {
        let opml = r#"<opml version="1.0"><body>
<outline type="rss" text="Root feed" xmlUrl="https://example.com/feed.xml"/>
</body></opml>"#;

        let sources = parse_opml_sources(opml);

        assert_eq!(sources.len(), 1);
        assert!(sources[0].folder_path.is_empty());
        assert_eq!(sources[0].title, "Root feed");
    }

    #[test]
    fn reimporting_foldered_opml_organizes_existing_feed_without_duplicate() {
        let mut existing = vec![test_rss_source(
            "Xataka",
            "https://www.xataka.com/index.xml",
        )];
        let imported = parse_opml_sources(
            r#"<opml version="2.0"><body>
<outline text="05 — Tecnología, IA e internet">
  <outline type="rss" text="Xataka" xmlUrl="https://www.xataka.com/index.xml"/>
</outline>
</body></opml>"#,
        );

        let (added, organized) = merge_imported_opml_sources(&mut existing, imported);

        assert_eq!(added, 0);
        assert_eq!(organized, 1);
        assert_eq!(existing.len(), 1);
        assert_eq!(
            existing[0].folder_path,
            vec!["05 — Tecnología, IA e internet".to_string()]
        );
    }

    #[test]
    fn legacy_rss_source_json_defaults_to_root_folder() {
        let source: RssSource = serde_json::from_str(
            r#"{"title":"Legacy","url":"https://example.com/feed.xml","kind":"Feed"}"#,
        )
        .expect("legacy RSS source should deserialize");

        assert!(source.folder_path.is_empty());
    }

    #[test]
    fn folder_registry_keeps_empty_folder_and_parent_paths() {
        let mut settings = crate::settings::AppSettings {
            rss_news_language: "es".to_string(),
            ..Default::default()
        };
        let paths = settings
            .rss_folders_by_language
            .entry("es".to_string())
            .or_default();
        add_folder_path_with_parents(paths, &["Tecnología".to_string(), "IA".to_string()]);

        let active = rss_folder_paths_for_settings(&settings);

        assert_eq!(
            active,
            vec![
                vec!["Tecnología".to_string()],
                vec!["Tecnología".to_string(), "IA".to_string()]
            ]
        );
    }

    #[test]
    fn reordering_feed_inside_folder_only_changes_sibling_order() {
        let mut a1 = test_rss_source("A1", "https://example.com/a1.xml");
        a1.folder_path = vec!["Cartella A".to_string()];
        let mut b1 = test_rss_source("B1", "https://example.com/b1.xml");
        b1.folder_path = vec!["Cartella B".to_string()];
        let mut a2 = test_rss_source("A2", "https://example.com/a2.xml");
        a2.folder_path = vec!["Cartella A".to_string()];
        let mut settings = crate::settings::AppSettings {
            rss_sources: vec![a1, b1, a2],
            ..Default::default()
        };

        let new_index = reorder_rss_source_within_folder(&mut settings, 2, ReorderAction::Up, 0);

        assert_eq!(new_index, Some(0));
        assert_eq!(settings.rss_sources[0].title, "A2");
        assert_eq!(settings.rss_sources[1].title, "B1");
        assert_eq!(settings.rss_sources[2].title, "A1");
        assert_eq!(
            settings.rss_sources[0].folder_path,
            vec!["Cartella A".to_string()]
        );
        assert_eq!(
            settings.rss_sources[1].folder_path,
            vec!["Cartella B".to_string()]
        );
    }

    #[test]
    fn move_to_folder_destinations_from_root_omit_main_folder() {
        let root = test_rss_source("Root", "https://example.com/root.xml");
        let mut in_a = test_rss_source("A", "https://example.com/a.xml");
        in_a.folder_path = vec!["Cartella A".to_string()];
        let mut in_b = test_rss_source("B", "https://example.com/b.xml");
        in_b.folder_path = vec!["Cartella B".to_string()];
        let settings = crate::settings::AppSettings {
            rss_sources: vec![root, in_a, in_b],
            ..Default::default()
        };

        let destinations = move_folder_destinations(&settings, 0);

        assert_eq!(
            destinations,
            vec![
                vec!["Cartella A".to_string()],
                vec!["Cartella B".to_string()]
            ]
        );
    }

    #[test]
    fn move_to_folder_destinations_inside_folder_include_root_and_omit_current() {
        let mut in_a = test_rss_source("A", "https://example.com/a.xml");
        in_a.folder_path = vec!["Cartella A".to_string()];
        let mut in_b = test_rss_source("B", "https://example.com/b.xml");
        in_b.folder_path = vec!["Cartella B".to_string()];
        let settings = crate::settings::AppSettings {
            rss_sources: vec![in_a, in_b],
            ..Default::default()
        };

        let destinations = move_folder_destinations(&settings, 0);

        assert_eq!(
            destinations,
            vec![Vec::<String>::new(), vec!["Cartella B".to_string()]]
        );
    }

    #[test]
    fn moving_feed_to_folder_places_it_after_existing_destination_siblings() {
        let mut source = test_rss_source("Da spostare", "https://example.com/source.xml");
        source.folder_path = vec!["Cartella A".to_string()];
        let mut b1 = test_rss_source("B1", "https://example.com/b1.xml");
        b1.folder_path = vec!["Cartella B".to_string()];
        let mut b2 = test_rss_source("B2", "https://example.com/b2.xml");
        b2.folder_path = vec!["Cartella B".to_string()];
        let mut settings = crate::settings::AppSettings {
            rss_sources: vec![source, b1, b2],
            ..Default::default()
        };

        let new_index = move_rss_source_to_folder(&mut settings, 0, &["Cartella B".to_string()]);

        assert_eq!(new_index, Some(2));
        assert_eq!(settings.rss_sources[2].title, "Da spostare");
        assert_eq!(
            settings.rss_sources[2].folder_path,
            vec!["Cartella B".to_string()]
        );
    }

    #[test]
    fn opml_empty_folders_are_preserved_by_export_and_parser() {
        let folders = vec![
            vec!["Cartella vuota".to_string()],
            vec!["Cartella vuota".to_string(), "Sottocartella".to_string()],
        ];
        let mut bytes = Vec::new();

        write_sources_to_opml_with_folders(&mut bytes, &[], &folders)
            .expect("empty-folder OPML export should succeed");
        let exported = String::from_utf8(bytes).expect("exported OPML should be UTF-8");
        let parsed_folders = parse_opml_folder_paths(&exported);

        assert_eq!(parsed_folders, folders);
    }

    #[test]
    fn opml_export_round_trip_preserves_folder_paths() {
        let mut first = test_rss_source("EL PAÍS", "https://example.com/elpais.xml");
        first.folder_path = vec!["01 — Actualidad y generalistas".to_string()];
        let mut second = test_rss_source("Xataka", "https://example.com/xataka.xml");
        second.folder_path = vec!["05 — Tecnología, IA e internet".to_string()];
        let mut bytes = Vec::new();

        write_sources_to_opml(&mut bytes, &[first, second]).expect("OPML export should succeed");
        let exported = String::from_utf8(bytes).expect("exported OPML should be UTF-8");
        let parsed = parse_opml_sources(&exported);

        assert!(exported.contains("<opml version=\"2.0\">"));
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed[0].folder_path,
            vec!["01 — Actualidad y generalistas".to_string()]
        );
        assert_eq!(
            parsed[1].folder_path,
            vec!["05 — Tecnología, IA e internet".to_string()]
        );
    }
}

fn decode_basic_html_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '&' {
            out.push(c);
            continue;
        }

        let mut entity = String::new();
        let mut ended_with_semicolon = false;
        while let Some(&next) = chars.peek() {
            chars.next();
            if next == ';' {
                ended_with_semicolon = true;
                break;
            }
            // Keep entity names bounded to avoid eating large text chunks.
            if entity.len() >= 16 {
                entity.push(next);
                break;
            }
            entity.push(next);
        }

        let decoded = if entity.starts_with("#x") || entity.starts_with("#X") {
            u32::from_str_radix(&entity[2..], 16)
                .ok()
                .and_then(char::from_u32)
        } else if let Some(num) = entity.strip_prefix('#') {
            num.parse::<u32>().ok().and_then(char::from_u32)
        } else {
            match entity.as_str() {
                "nbsp" => Some(' '),
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "laquo" => Some('«'),
                "raquo" => Some('»'),
                "hellip" => Some('…'),
                "ndash" => Some('–'),
                "mdash" => Some('—'),
                "rsquo" => Some('’'),
                "lsquo" => Some('‘'),
                "rdquo" => Some('”'),
                "ldquo" => Some('“'),
                // Latin entities commonly seen in Italian and other EU feeds.
                "agrave" => Some('à'),
                "egrave" => Some('è'),
                "igrave" => Some('ì'),
                "ograve" => Some('ò'),
                "ugrave" => Some('ù'),
                "aacute" => Some('á'),
                "eacute" => Some('é'),
                "iacute" => Some('í'),
                "oacute" => Some('ó'),
                "uacute" => Some('ú'),
                "Agrave" => Some('À'),
                "Egrave" => Some('È'),
                "Igrave" => Some('Ì'),
                "Ograve" => Some('Ò'),
                "Ugrave" => Some('Ù'),
                "Aacute" => Some('Á'),
                "Eacute" => Some('É'),
                "Iacute" => Some('Í'),
                "Oacute" => Some('Ó'),
                "Uacute" => Some('Ú'),
                _ => None,
            }
        };

        if let Some(ch) = decoded {
            out.push(ch);
        } else {
            out.push('&');
            out.push_str(&entity);
            if ended_with_semicolon {
                out.push(';');
            }
        }
    }
    out
}

fn normalize_rss_url_key(url: &str) -> String {
    let mut s = url.trim().to_string();
    if s.is_empty() {
        return s;
    }
    if s.get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
    {
        s = s[8..].to_string();
    } else if s
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
    {
        s = s[7..].to_string();
    }
    if let Some((left, _)) = s.split_once('#') {
        s = left.to_string();
    }
    while s.ends_with('/') && s.len() > 1 {
        s.pop();
    }
    s.to_ascii_lowercase()
}

fn rss_source_display_title(
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
            crate::settings::RssPodcastUnreadLabelPosition::Before => {
                format!("{}{}", i18n::tr(language, "rss.unread_prefix"), base_title)
            }
            crate::settings::RssPodcastUnreadLabelPosition::After => {
                format!("{base_title}{}", i18n::tr(language, "rss.unread_suffix"))
            }
        }
    } else {
        base_title
    }
}

fn handle_move_shortcut(hwnd: HWND, move_up: bool) -> bool {
    if selected_source_index(hwnd).is_some() {
        let action = if move_up {
            ReorderAction::Up
        } else {
            ReorderAction::Down
        };
        handle_reorder_action(hwnd, action);
        return true;
    }
    move_selected_article_by_one(hwnd, move_up)
}

#[derive(Clone, Copy)]
struct RssItemTitleContext {
    language: crate::settings::Language,
    announce_unread: bool,
    unread_label_position: crate::settings::RssPodcastUnreadLabelPosition,
    date_mode: ListDateDisplayMode,
    time_mode: ListTimeDisplayMode,
}

fn rss_item_display_title(
    title: &str,
    item_unread: bool,
    pub_date: Option<i64>,
    has_multiple_items_same_day: bool,
    ctx: RssItemTitleContext,
) -> String {
    let base = format!(
        "{title}{}",
        format_timestamp_for_list(
            pub_date,
            ctx.language,
            ctx.date_mode,
            ctx.time_mode,
            has_multiple_items_same_day,
        )
        .map(|ts| format!(". {ts}"))
        .unwrap_or_default()
    );
    if ctx.announce_unread && item_unread {
        return match ctx.unread_label_position {
            crate::settings::RssPodcastUnreadLabelPosition::Before => {
                format!(
                    "{}{}",
                    i18n::tr(ctx.language, "rss.item_unread_prefix"),
                    base
                )
            }
            crate::settings::RssPodcastUnreadLabelPosition::After => {
                format!("{base}{}", i18n::tr(ctx.language, "rss.item_unread_suffix"))
            }
        };
    }
    base
}

fn rss_item_tree_display_title(
    item: &RssItem,
    item_unread: bool,
    has_multiple_items_same_day: bool,
    ctx: RssItemTitleContext,
) -> String {
    let mut display_title = rss_item_display_title(
        &item.title,
        item_unread,
        item.pub_date,
        has_multiple_items_same_day,
        ctx,
    );
    if !item.related_items.is_empty() {
        let related_label = i18n::tr(ctx.language, "rss.related_sources_count")
            .replace("{count}", &item.related_items.len().to_string());
        display_title.push_str(", ");
        display_title.push_str(&related_label);
    }
    display_title
}

fn day_from_timestamp(timestamp: Option<i64>) -> Option<NaiveDate> {
    let ts = timestamp?;
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.date_naive())
}

fn build_day_counts(items: &[RssItem]) -> HashMap<NaiveDate, usize> {
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

fn show_selected_properties(hwnd: HWND) {
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return;
    }
    let hitem = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    if hitem.0 == 0 {
        return;
    }

    let language = with_rss_state(hwnd, |s| {
        { with_state(s.parent, |ps| ps.settings.language) }.unwrap_or_default()
    })
    .unwrap_or_default();

    let (node, source, item_unread) = with_rss_state(hwnd, |s| {
        let node = s.node_data.get(&hitem.0).cloned();
        let source = match &node {
            Some(NodeData::Source(idx)) => {
                { with_state(s.parent, |ps| ps.settings.rss_sources.get(*idx).cloned()) }.flatten()
            }
            _ => None,
        };
        let item_unread = match &node {
            Some(NodeData::Item(item)) => {
                let key = rss_item_key(item);
                let unread = source_ancestor_hitem(s, hitem)
                    .and_then(|source_hitem| s.source_items.get(&source_hitem.0))
                    .map(|state| !state.read_item_keys.contains(&key))
                    .unwrap_or(true);
                Some(unread)
            }
            _ => None,
        };
        (node, source, item_unread)
    })
    .unwrap_or((None, None, None));

    let mut lines: Vec<String> = Vec::new();
    match node {
        Some(NodeData::Source(_)) => {
            if let Some(src) = source {
                let source_type = match src.kind {
                    RssSourceType::Feed => i18n::tr(language, "properties.feed"),
                    RssSourceType::Site => i18n::tr(language, "properties.site"),
                    RssSourceType::Article => i18n::tr(language, "properties.article"),
                };
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
                    i18n::tr(language, "properties.source")
                ));
                lines.push(format!(
                    "{}: {}",
                    i18n::tr(language, "properties.title"),
                    title_value
                ));
                lines.push(format!(
                    "{}: {}",
                    i18n::tr(language, "properties.kind"),
                    source_type
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
        Some(NodeData::LoadMore) => {
            lines.push(format!(
                "{}: {}",
                i18n::tr(language, "properties.type"),
                rss_load_more_label(language)
            ));
        }
        Some(NodeData::Item(item)) => {
            let status_value = if item_unread.unwrap_or(true) {
                i18n::tr(language, "properties.unread")
            } else {
                i18n::tr(language, "properties.read")
            };
            let date_value = format_timestamp_for_language(item.pub_date, language)
                .unwrap_or_else(|| i18n::tr(language, "properties.not_available"));
            lines.push(format!(
                "{}: {}",
                i18n::tr(language, "properties.type"),
                i18n::tr(language, "properties.article")
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
        _ => return,
    }

    let body = lines.join("\r\n");
    help_window::open_readonly_text(hwnd, &i18n::tr(language, "properties.title_window"), &body);
}

fn percent_encode(input: &str) -> String {
    url::form_urlencoded::byte_serialize(input.as_bytes()).collect()
}

fn mailto_encode_component(input: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(input.len() * 3);
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0x0F) as usize] as char);
            }
        }
    }
    out
}

fn decode_mail_text_component(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = bytes[i + 1];
            let h2 = bytes[i + 2];
            let v1 = (h1 as char).to_digit(16);
            let v2 = (h2 as char).to_digit(16);
            if let (Some(a), Some(b)) = (v1, v2) {
                out.push(((a << 4) | b) as u8);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    decode_basic_html_entities(&String::from_utf8_lossy(&out))
        .trim()
        .to_string()
}

fn build_google_news_rss_url(keyword: &str, news_language_code: &str) -> String {
    let locale = google_news_locale(news_language_code);
    let query = percent_encode(keyword.trim());
    format!(
        "https://news.google.com/rss/search?q={}&hl={}&gl={}&ceid={}",
        query, locale.hl, locale.gl, locale.ceid
    )
}

fn format_google_news_source_title(keyword: &str) -> String {
    keyword
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                let mut out = String::new();
                out.extend(first.to_uppercase());
                for ch in chars {
                    out.extend(ch.to_lowercase());
                }
                out
            } else {
                String::new()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportedOpmlSource {
    title: String,
    url: String,
    folder_path: Vec<String>,
}

fn parse_opml_sources(text: &str) -> Vec<ImportedOpmlSource> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let mut folder_path = Vec::new();
    let mut outline_folder_stack = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
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
                    out.push(ImportedOpmlSource {
                        title,
                        url,
                        folder_path: folder_path.clone(),
                    });
                    outline_folder_stack.push(false);
                } else if !title.trim().is_empty() {
                    folder_path.push(title.trim().to_string());
                    outline_folder_stack.push(true);
                } else {
                    outline_folder_stack.push(false);
                }
            }
            Ok(Event::Empty(e)) => {
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
                    out.push(ImportedOpmlSource {
                        title,
                        url,
                        folder_path: folder_path.clone(),
                    });
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref().eq_ignore_ascii_case(b"outline")
                    && let Some(was_folder) = outline_folder_stack.pop()
                    && was_folder
                {
                    folder_path.truncate(folder_path.len().saturating_sub(1));
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

fn parse_opml_folder_paths(text: &str) -> Vec<Vec<String>> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut folders = Vec::new();
    let mut current_path = Vec::new();
    let mut outline_folder_stack = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if !e.name().as_ref().eq_ignore_ascii_case(b"outline") {
                    buf.clear();
                    continue;
                }
                let mut has_url = false;
                let mut title = String::new();
                for attr in e.attributes().flatten() {
                    let key = attr.key.as_ref();
                    let value = attr
                        .decode_and_unescape_value(reader.decoder())
                        .unwrap_or_default()
                        .to_string();
                    if key.eq_ignore_ascii_case(b"xmlUrl") && !value.trim().is_empty() {
                        has_url = true;
                    } else if title.is_empty()
                        && (key.eq_ignore_ascii_case(b"title") || key.eq_ignore_ascii_case(b"text"))
                    {
                        title = value;
                    }
                }
                if !has_url && !title.trim().is_empty() {
                    current_path.push(title.trim().to_string());
                    if !folders.contains(&current_path) {
                        folders.push(current_path.clone());
                    }
                    outline_folder_stack.push(true);
                } else {
                    outline_folder_stack.push(false);
                }
            }
            Ok(Event::Empty(e)) => {
                if !e.name().as_ref().eq_ignore_ascii_case(b"outline") {
                    buf.clear();
                    continue;
                }
                let mut has_url = false;
                let mut title = String::new();
                for attr in e.attributes().flatten() {
                    let key = attr.key.as_ref();
                    let value = attr
                        .decode_and_unescape_value(reader.decoder())
                        .unwrap_or_default()
                        .to_string();
                    if key.eq_ignore_ascii_case(b"xmlUrl") && !value.trim().is_empty() {
                        has_url = true;
                    } else if title.is_empty()
                        && (key.eq_ignore_ascii_case(b"title") || key.eq_ignore_ascii_case(b"text"))
                    {
                        title = value;
                    }
                }
                if !has_url && !title.trim().is_empty() {
                    let mut path = current_path.clone();
                    path.push(title.trim().to_string());
                    if !folders.contains(&path) {
                        folders.push(path);
                    }
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref().eq_ignore_ascii_case(b"outline")
                    && let Some(was_folder) = outline_folder_stack.pop()
                    && was_folder
                {
                    current_path.truncate(current_path.len().saturating_sub(1));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    folders
}

fn merge_imported_opml_sources(
    rss_sources: &mut Vec<RssSource>,
    imported_sources: Vec<ImportedOpmlSource>,
) -> (usize, usize) {
    let mut existing: HashMap<String, usize> = rss_sources
        .iter()
        .enumerate()
        .map(|(index, source)| (normalize_rss_url_key(&source.url), index))
        .collect();
    let mut added = 0;
    let mut organized = 0;

    for imported in imported_sources {
        let ImportedOpmlSource {
            title,
            url,
            folder_path,
        } = imported;
        let key = normalize_rss_url_key(&url);
        if let Some(existing_index) = existing.get(&key).copied() {
            if !folder_path.is_empty()
                && let Some(source) = rss_sources.get_mut(existing_index)
                && source.folder_path != folder_path
            {
                source.folder_path = folder_path;
                organized += 1;
            }
            continue;
        }

        let user_title = title.trim() != url.trim();
        rss_sources.push(RssSource {
            title,
            url,
            kind: RssSourceType::Feed,
            folder_path,
            user_title,
            unread: false,
            cache: RssFeedCache::default(),
            last_seen_guid: None,
            last_updated: None,
            removed_item_keys: Vec::new(),
            read_item_keys: Vec::new(),
        });
        existing.insert(key, rss_sources.len() - 1);
        added += 1;
    }

    (added, organized)
}

fn open_import_txt_dialog(hwnd: HWND, language: crate::settings::Language) -> Option<PathBuf> {
    let filter_raw = i18n::tr(language, "rss.import_filter");
    let filter = to_wide(&filter_raw.replace("\\0", "\0"));
    let mut buffer = vec![0u16; 4096];
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY,
        ..Default::default()
    };
    if !crate::get_open_file_name_w_safe(&mut ofn).as_bool() {
        return None;
    }
    parse_single_path(&buffer)
}

fn open_export_opml_dialog(hwnd: HWND, language: crate::settings::Language) -> Option<PathBuf> {
    let filter_raw = i18n::tr(language, "rss.import_filter");
    let filter = to_wide(&filter_raw.replace("\\0", "\0"));
    let mut buffer = vec![0u16; 4096];
    let default_name = to_wide("Sonarpad Rss.opml");
    let copy_len = default_name.len().min(buffer.len().saturating_sub(1));
    buffer[..copy_len].copy_from_slice(&default_name[..copy_len]);
    let default_ext = to_wide("opml");
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrDefExt: PCWSTR(default_ext.as_ptr()),
        Flags: OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT | OFN_HIDEREADONLY,
        ..Default::default()
    };
    if !crate::get_save_file_name_w_safe(&mut ofn).as_bool() {
        return None;
    }
    parse_single_path(&buffer).map(ensure_opml_extension)
}

fn escape_opml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Default)]
struct OpmlFolderTree {
    name: String,
    source_indices: Vec<usize>,
    children: Vec<OpmlFolderTree>,
}

fn opml_folder_node_mut<'a>(
    children: &'a mut Vec<OpmlFolderTree>,
    path: &[String],
) -> Option<&'a mut OpmlFolderTree> {
    let (first, rest) = path.split_first()?;
    let position = children
        .iter()
        .position(|child| child.name == *first)
        .unwrap_or_else(|| {
            children.push(OpmlFolderTree {
                name: first.clone(),
                ..Default::default()
            });
            children.len() - 1
        });
    if rest.is_empty() {
        children.get_mut(position)
    } else {
        let child = children.get_mut(position)?;
        opml_folder_node_mut(&mut child.children, rest)
    }
}

fn write_opml_source<W: Write>(
    writer: &mut W,
    source: &RssSource,
    depth: usize,
) -> Result<(), String> {
    let title = if source.title.trim().is_empty() {
        source.url.clone()
    } else {
        source.title.clone()
    };
    let indent = "  ".repeat(depth);
    writeln!(
        writer,
        "{indent}<outline type=\"rss\" text=\"{}\" title=\"{}\" xmlUrl=\"{}\" />",
        escape_opml_attr(&title),
        escape_opml_attr(&title),
        escape_opml_attr(&source.url)
    )
    .map_err(|e| e.to_string())
}

fn write_opml_folder<W: Write>(
    writer: &mut W,
    folder: &OpmlFolderTree,
    sources: &[RssSource],
    depth: usize,
) -> Result<(), String> {
    let indent = "  ".repeat(depth);
    writeln!(
        writer,
        "{indent}<outline text=\"{}\" title=\"{}\">",
        escape_opml_attr(&folder.name),
        escape_opml_attr(&folder.name)
    )
    .map_err(|e| e.to_string())?;
    for source_index in &folder.source_indices {
        if let Some(source) = sources.get(*source_index) {
            write_opml_source(writer, source, depth + 1)?;
        }
    }
    for child in &folder.children {
        write_opml_folder(writer, child, sources, depth + 1)?;
    }
    writeln!(writer, "{indent}</outline>").map_err(|e| e.to_string())
}

fn write_sources_to_opml_with_folders<W: Write>(
    writer: &mut W,
    sources: &[RssSource],
    folder_paths: &[Vec<String>],
) -> Result<(), String> {
    writeln!(
        writer,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\">\n<head>\n<title>Sonarpad RSS</title>\n</head>\n<body>"
    )
    .map_err(|e| e.to_string())?;

    let mut folders = Vec::new();
    for path in folder_paths {
        let normalized = normalized_folder_path(path);
        if !normalized.is_empty() {
            let folder_node = opml_folder_node_mut(&mut folders, &normalized);
            if folder_node.is_none() {
                return Err("invalid OPML folder path".to_string());
            }
        }
    }

    let mut root_sources = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let normalized = normalized_folder_path(&source.folder_path);
        if normalized.is_empty() {
            root_sources.push(index);
        } else if let Some(folder) = opml_folder_node_mut(&mut folders, &normalized) {
            folder.source_indices.push(index);
        }
    }

    for source_index in root_sources {
        if let Some(source) = sources.get(source_index) {
            write_opml_source(writer, source, 1)?;
        }
    }
    for folder in &folders {
        write_opml_folder(writer, folder, sources, 1)?;
    }
    writeln!(writer, "</body>\n</opml>").map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
fn write_sources_to_opml<W: Write>(writer: &mut W, sources: &[RssSource]) -> Result<(), String> {
    let mut folders = Vec::new();
    for source in sources {
        add_folder_path_with_parents(&mut folders, &source.folder_path);
    }
    write_sources_to_opml_with_folders(writer, sources, &folders)
}

fn export_sources_to_opml_file(hwnd: HWND, path: &Path) -> Result<usize, String> {
    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return Err("missing parent".to_string());
    }
    let (sources, folders) = with_state(parent, |state| {
        (
            state.settings.rss_sources.clone(),
            rss_folder_paths_for_settings(&state.settings),
        )
    })
    .unwrap_or_default();
    if sources.is_empty() && folders.is_empty() {
        return Ok(0);
    }

    let mut file = File::create(path).map_err(|e| e.to_string())?;
    write_sources_to_opml_with_folders(&mut file, &sources, &folders)?;
    Ok(sources.len())
}

fn import_sources_from_file(hwnd: HWND, path: &Path) -> usize {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            log_debug(&format!(
                "rss_import_file_error path=\"{}\" error=\"{}\"",
                path.to_string_lossy(),
                err
            ));
            return 0;
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let is_opml = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("opml"))
        .unwrap_or(false)
        || text.to_ascii_lowercase().contains("<opml");
    let opml_sources = if is_opml {
        parse_opml_sources(&text)
    } else {
        Vec::new()
    };
    let opml_folders = if is_opml {
        parse_opml_folder_paths(&text)
    } else {
        Vec::new()
    };
    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return 0;
    }
    let import_result = with_state(parent, |state| {
        let (added, organized) =
            merge_imported_opml_sources(&mut state.settings.rss_sources, opml_sources);
        let code = active_news_language_code_from_state(state);
        let folders = state
            .settings
            .rss_folders_by_language
            .entry(code)
            .or_default();
        let before = folders.len();
        for folder in opml_folders {
            add_folder_path_with_parents(folders, &folder);
        }
        let folder_added = folders.len().saturating_sub(before);
        if added > 0 || organized > 0 || folder_added > 0 {
            save_rss_settings(state);
        }
        (added, organized, folder_added)
    });
    let Some((added, organized, folder_added)) = import_result else {
        crate::log_debug("Failed to access state in import_opml_file");
        return 0;
    };
    let source_changes = added + organized;
    if source_changes > 0 || folder_added > 0 {
        log_debug(&format!(
            "rss_import_file_changed path=\"{}\" added={} organized={} folders={}",
            path.to_string_lossy(),
            added,
            organized,
            folder_added
        ));
        reload_tree(hwnd);
    }
    source_changes
}

fn is_valid_article_url(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    matches!(parsed.scheme(), "http" | "https")
}

fn rss_item_key(item: &RssItem) -> String {
    if !item.guid.trim().is_empty() {
        return item.guid.trim().to_string();
    }
    if !item.link.trim().is_empty() {
        return item.link.trim().to_string();
    }
    item.title.trim().to_string()
}

fn collect_rss_item_keys(item: &RssItem, keys: &mut HashSet<String>) {
    keys.insert(rss_item_key(item));
    for related in &item.related_items {
        collect_rss_item_keys(related, keys);
    }
}

fn source_ancestor_hitem(
    state: &RssWindowState,
    mut hitem: windows::Win32::UI::Controls::HTREEITEM,
) -> Option<windows::Win32::UI::Controls::HTREEITEM> {
    while hitem.0 != 0 {
        if matches!(state.node_data.get(&hitem.0), Some(NodeData::Source(_))) {
            return Some(hitem);
        }
        hitem = windows::Win32::UI::Controls::HTREEITEM(
            crate::send_message_w_safe(
                state.hwnd_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_PARENT as usize),
                LPARAM(hitem.0),
            )
            .0,
        );
    }
    None
}

fn select_newest_item_key(items: &[RssItem], removed_keys: &HashSet<String>) -> Option<String> {
    let mut best: Option<(i64, usize, String)> = None;
    for (idx, item) in items.iter().enumerate() {
        let key = rss_item_key(item);
        if removed_keys.contains(&key) {
            continue;
        }
        let ts = item.pub_date.unwrap_or(i64::MIN);
        match &best {
            Some((best_ts, best_idx, _))
                if ts < *best_ts || (ts == *best_ts && idx >= *best_idx) => {}
            _ => best = Some((ts, idx, key)),
        }
    }
    best.map(|(_, _, key)| key)
}

fn is_favorites_source_url(url: &str) -> bool {
    url.trim().eq_ignore_ascii_case(RSS_FAVORITES_SOURCE_URL)
}

fn is_favorites_source(source: &RssSource) -> bool {
    is_favorites_source_url(&source.url)
}

fn favorites_source_title(language: crate::settings::Language) -> String {
    i18n::tr(language, "rss.favorites.title")
}

fn ensure_favorites_source(parent: HWND) -> usize {
    with_state(parent, |ps| {
        if let Some(idx) = ps.settings.rss_sources.iter().position(is_favorites_source) {
            if let Some(src) = ps.settings.rss_sources.get_mut(idx) {
                let expected_title = favorites_source_title(ps.settings.language);
                if src.title != expected_title {
                    src.title = expected_title;
                    src.user_title = true;
                    save_rss_settings(ps);
                }
            }
            return idx;
        }

        let favorites = RssSource {
            title: favorites_source_title(ps.settings.language),
            url: RSS_FAVORITES_SOURCE_URL.to_string(),
            kind: RssSourceType::Feed,
            folder_path: Vec::new(),
            user_title: true,
            unread: false,
            cache: RssFeedCache::default(),
            last_seen_guid: None,
            last_updated: None,
            removed_item_keys: Vec::new(),
            read_item_keys: Vec::new(),
        };
        ps.settings.rss_sources.insert(0, favorites);
        save_rss_settings(ps);
        0
    })
    .unwrap_or(0)
}

fn load_favorites_source_items(
    hwnd: HWND,
    hitem: windows::Win32::UI::Controls::HTREEITEM,
    source_index: usize,
) {
    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }
    let (items, saved_read_item_keys) = with_state(parent, |ps| {
        let legacy_removed_keys: HashSet<String> = ps
            .settings
            .rss_sources
            .get(source_index)
            .map(|src| src.removed_item_keys.iter().cloned().collect())
            .unwrap_or_default();
        if !legacy_removed_keys.is_empty() {
            ps.settings
                .rss_favorite_articles
                .retain(|item| !legacy_removed_keys.contains(&rss_item_key(item)));
            if let Some(src) = ps.settings.rss_sources.get_mut(source_index) {
                src.removed_item_keys.clear();
            }
            save_rss_settings(ps);
        }

        let mut items = ps.settings.rss_favorite_articles.clone();
        sort_items_by_date_desc(&mut items);
        let read_keys = ps
            .settings
            .rss_sources
            .get(source_index)
            .map(|src| src.read_item_keys.iter().cloned().collect())
            .unwrap_or_default();
        (items, read_keys)
    })
    .unwrap_or_default();

    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 != 0 {
        loop {
            let child = crate::send_message_w_safe(
                hwnd_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_CHILD as usize),
                LPARAM(hitem.0),
            );
            if child.0 == 0 {
                break;
            }
            crate::send_message_w_safe(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(child.0));
        }
    }

    with_rss_state(hwnd, |s| {
        s.source_items.insert(
            hitem.0,
            SourceItemsState {
                items,
                loaded: 0,
                read_item_keys: saved_read_item_keys,
            },
        );
    });

    let (initial_count, _next_count) = rss_page_sizes(parent);
    let (_inserted, _first_inserted) = load_more_items(hwnd, hitem, initial_count);
}
fn source_removed_keys_for_tree_item(
    hwnd: HWND,
    hitem: windows::Win32::UI::Controls::HTREEITEM,
) -> HashSet<String> {
    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return HashSet::new();
    }
    let source_index = with_rss_state(hwnd, |s| match s.node_data.get(&hitem.0) {
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
                .rss_sources
                .get(idx)
                .map(|src| src.removed_item_keys.iter().cloned().collect())
        })
        .flatten()
        .unwrap_or_default()
    }
}

fn sort_items_by_date_desc(items: &mut [RssItem]) {
    let mut indexed: Vec<(usize, RssItem)> = items.iter().cloned().enumerate().collect();
    indexed.sort_by(|(ia, a), (ib, b)| {
        let at = a.pub_date.unwrap_or(i64::MIN);
        let bt = b.pub_date.unwrap_or(i64::MIN);
        bt.cmp(&at).then_with(|| ia.cmp(ib))
    });
    for (dst, (_, item)) in indexed.into_iter().enumerate() {
        items[dst] = item;
    }
}

fn prune_persisted_read_keys_for_source(
    hwnd: HWND,
    hitem: windows::Win32::UI::Controls::HTREEITEM,
) {
    let source_index = with_rss_state(hwnd, |s| match s.node_data.get(&hitem.0) {
        Some(NodeData::Source(idx)) => Some(*idx),
        _ => None,
    })
    .flatten();
    let Some(source_index) = source_index else {
        return;
    };

    let current_item_keys: HashSet<String> = with_rss_state(hwnd, |s| {
        let mut keys = HashSet::new();
        if let Some(state) = s.source_items.get(&hitem.0) {
            for item in &state.items {
                collect_rss_item_keys(item, &mut keys);
            }
        }
        keys
    })
    .unwrap_or_default();

    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }

    {
        with_state(parent, |ps| {
            if let Some(src) = ps.settings.rss_sources.get_mut(source_index) {
                let before = src.read_item_keys.len();
                src.read_item_keys.retain(|k| current_item_keys.contains(k));
                if src.read_item_keys.len() != before {
                    save_rss_settings(ps);
                }
            }
        });
    }
}

fn select_first_root_if_needed(hwnd: HWND, hwnd_tree: HWND) {
    if hwnd_tree.0 == 0 {
        return;
    }
    let current = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    if current.0 != 0 {
        return;
    }
    let first = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_ROOT as usize),
            LPARAM(0),
        )
        .0,
    );
    if first.0 != 0 {
        unsafe {
            SendMessageW(
                hwnd_tree,
                TVM_SELECTITEM,
                WPARAM(TVGN_CARET as usize),
                LPARAM(first.0),
            );
            SendMessageW(hwnd_tree, TVM_ENSUREVISIBLE, WPARAM(0), LPARAM(first.0));
        }
        with_rss_state(hwnd, |s| s.last_selected = first.0);
    }
}

fn announce_rss_status(message: &str) {
    log_debug(&format!("rss_status {}", message));
    if !nvda_speak(message) {
        crate::log_debug("NVDA speak failed");
    }
}

fn rss_page_sizes(parent: HWND) -> (usize, usize) {
    {
        with_state(parent, |s| {
            (
                s.settings.rss_initial_page_size,
                s.settings.rss_next_page_size,
            )
        })
        .unwrap_or((INITIAL_LOAD_COUNT, LOAD_MORE_COUNT))
    }
}

fn rss_load_more_label(language: crate::settings::Language) -> String {
    i18n::tr(language, "rss.load_more_news")
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

fn delete_load_more_nodes(hwnd: HWND, source_hitem: windows::Win32::UI::Controls::HTREEITEM) {
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 || source_hitem.0 == 0 {
        return;
    }
    let mut child = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CHILD as usize),
            LPARAM(source_hitem.0),
        )
        .0,
    );
    while child.0 != 0 {
        let next = windows::Win32::UI::Controls::HTREEITEM(
            crate::send_message_w_safe(
                hwnd_tree,
                TVM_GETNEXTITEM,
                WPARAM(windows::Win32::UI::Controls::TVGN_NEXT as usize),
                LPARAM(child.0),
            )
            .0,
        );
        let is_load_more = with_rss_state(hwnd, |s| {
            matches!(s.node_data.get(&child.0), Some(NodeData::LoadMore))
        })
        .unwrap_or(false);
        if is_load_more {
            with_rss_state(hwnd, |s| {
                s.node_data.remove(&child.0);
            });
            crate::send_message_w_safe(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(child.0));
        }
        child = next;
    }
}

fn append_load_more_node(
    hwnd: HWND,
    source_hitem: windows::Win32::UI::Controls::HTREEITEM,
    language: crate::settings::Language,
) {
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 || source_hitem.0 == 0 {
        return;
    }
    let text = to_wide(&rss_load_more_label(language));
    let mut tvis = TVINSERTSTRUCTW {
        hParent: source_hitem,
        hInsertAfter: TVI_LAST,
        Anonymous: TVINSERTSTRUCTW_0 {
            item: TVITEMW {
                mask: TVIF_TEXT | TVIF_PARAM,
                pszText: windows::core::PWSTR(text.as_ptr() as *mut _),
                lParam: LPARAM(0),
                ..Default::default()
            },
        },
    };
    let hchild = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_INSERTITEMW,
            WPARAM(0),
            LPARAM(&mut tvis as *mut _ as isize),
        )
        .0,
    );
    if hchild.0 != 0 {
        with_rss_state(hwnd, |s| {
            s.node_data.insert(hchild.0, NodeData::LoadMore);
        });
    }
}

fn default_feed_path(language: crate::settings::Language) -> Option<PathBuf> {
    let file_name = match language {
        crate::settings::Language::German => "feed_de.txt",
        crate::settings::Language::Ukrainian => "feed_uk.txt",
        crate::settings::Language::English
        | crate::settings::Language::Lithuanian
        | crate::settings::Language::Chinese => "feed_en.txt",
        crate::settings::Language::Italian => "feed_it.txt",
        crate::settings::Language::Spanish => "feed_es.txt",
        crate::settings::Language::Portuguese => "feed_pt.txt",
        crate::settings::Language::PortugueseBrazilian => "feed_pt_BR.txt",
        crate::settings::Language::Swedish => "feed_en.txt",
        crate::settings::Language::Vietnamese => "feed_vi.txt",
        crate::settings::Language::Czech => "feed_cs.txt",
        crate::settings::Language::Polish => "feed_pl.txt",
        crate::settings::Language::French => "feed_fr.txt",
        crate::settings::Language::Serbian => "feed_sr HR.txt",
        crate::settings::Language::Russian => "feed_ru.txt",
        crate::settings::Language::Hindi => "feed_hi.txt",
    };
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));
    let mut candidates = Vec::new();
    if let Some(dir) = exe_dir {
        candidates.push(dir.join("i18n").join(file_name));
    }
    if let Ok(dir) = std::env::current_dir() {
        candidates.push(dir.join("i18n").join(file_name));
    }
    candidates.into_iter().find(|path| path.exists())
}

fn embedded_default_feeds(language: crate::settings::Language) -> &'static str {
    match language {
        crate::settings::Language::German => FEED_DE_DATA,
        crate::settings::Language::Ukrainian => FEED_UK_DATA,
        crate::settings::Language::English
        | crate::settings::Language::Lithuanian
        | crate::settings::Language::Chinese => FEED_EN_DATA,
        crate::settings::Language::Italian => FEED_IT_DATA,
        crate::settings::Language::Spanish => FEED_ES_DATA,
        crate::settings::Language::Portuguese => FEED_PT_DATA,
        crate::settings::Language::PortugueseBrazilian => FEED_PT_BR_DATA,
        crate::settings::Language::Swedish => FEED_EN_DATA,
        crate::settings::Language::Vietnamese => FEED_VI_DATA,
        crate::settings::Language::Czech => FEED_CS_DATA,
        crate::settings::Language::Polish => FEED_PL_DATA,
        crate::settings::Language::French => FEED_FR_DATA,
        crate::settings::Language::Serbian => FEED_SR_DATA,
        crate::settings::Language::Russian => FEED_RU_DATA,
        crate::settings::Language::Hindi => FEED_HI_DATA,
    }
}

fn load_default_feeds(language: crate::settings::Language) -> Vec<(String, String)> {
    let data = default_feed_path(language).and_then(|path| std::fs::read_to_string(path).ok());
    let data = data
        .as_deref()
        .unwrap_or_else(|| embedded_default_feeds(language));
    data.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|line| {
            if let Some((left, right)) = line.split_once('|') {
                let title = left.trim();
                let url = right.trim();
                let title = if title.is_empty() { url } else { title };
                (title.to_string(), url.to_string())
            } else {
                (line.to_string(), line.to_string())
            }
        })
        .filter(|(_, url)| !url.is_empty())
        .collect()
}

fn is_default_key(
    language: crate::settings::Language,
    settings: &crate::settings::AppSettings,
    key: &str,
) -> bool {
    match language {
        crate::settings::Language::German
        | crate::settings::Language::Ukrainian
        | crate::settings::Language::English
        | crate::settings::Language::Lithuanian
        | crate::settings::Language::Chinese
        | crate::settings::Language::Russian => settings
            .rss_default_en_keys
            .iter()
            .any(|k| normalize_rss_url_key(k) == key),
        crate::settings::Language::Swedish => settings
            .rss_default_en_keys
            .iter()
            .any(|k| normalize_rss_url_key(k) == key),
        crate::settings::Language::Italian => settings
            .rss_default_it_keys
            .iter()
            .any(|k| normalize_rss_url_key(k) == key),
        crate::settings::Language::Spanish => settings
            .rss_default_es_keys
            .iter()
            .any(|k| normalize_rss_url_key(k) == key),
        crate::settings::Language::Portuguese => settings
            .rss_default_pt_keys
            .iter()
            .any(|k| normalize_rss_url_key(k) == key),
        crate::settings::Language::PortugueseBrazilian => settings
            .rss_default_pt_br_keys
            .iter()
            .any(|k| normalize_rss_url_key(k) == key),
        crate::settings::Language::Vietnamese => settings
            .rss_default_vi_keys
            .iter()
            .any(|k| normalize_rss_url_key(k) == key),
        crate::settings::Language::Czech => settings
            .rss_default_cs_keys
            .iter()
            .any(|k| normalize_rss_url_key(k) == key),
        crate::settings::Language::Polish => settings
            .rss_default_pl_keys
            .iter()
            .any(|k| normalize_rss_url_key(k) == key),
        crate::settings::Language::French => settings
            .rss_default_fr_keys
            .iter()
            .any(|k| normalize_rss_url_key(k) == key),
        crate::settings::Language::Serbian => settings
            .rss_default_sr_keys
            .iter()
            .any(|k| normalize_rss_url_key(k) == key),
        crate::settings::Language::Hindi => settings
            .rss_default_hi_keys
            .iter()
            .any(|k| normalize_rss_url_key(k) == key),
    }
}

fn apply_default_sources(
    rss_sources: &mut Vec<RssSource>,
    removed_list: &[String],
    keys_list: &mut Vec<String>,
    defaults: &[(String, String)],
) -> bool {
    let mut default_items: Vec<(String, String, String)> = Vec::new();
    let mut default_by_key: HashMap<String, (String, String)> = HashMap::new();
    for (title, url) in defaults {
        let key = normalize_rss_url_key(url);
        if key.is_empty() {
            continue;
        }
        if default_by_key.contains_key(&key) {
            continue;
        }
        default_by_key.insert(key.clone(), (title.clone(), url.clone()));
        default_items.push((key, title.clone(), url.clone()));
    }
    if default_items.is_empty() {
        return false;
    }
    let current_default_keys: HashSet<String> =
        default_items.iter().map(|(k, _, _)| k.clone()).collect();

    let mut removed = HashSet::new();
    for url in removed_list.iter() {
        let key = normalize_rss_url_key(url);
        if !key.is_empty() {
            removed.insert(key);
        }
    }
    let mut existing = HashSet::new();
    for src in rss_sources.iter() {
        let key = normalize_rss_url_key(&src.url);
        if !key.is_empty() {
            existing.insert(key);
        }
    }
    let mut changed = false;
    let stored_keys: HashSet<String> = keys_list
        .iter()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect();
    if !stored_keys.is_empty() {
        let before_len = rss_sources.len();
        rss_sources.retain(|src| {
            let key = normalize_rss_url_key(&src.url);
            if key.is_empty() {
                return true;
            }
            if stored_keys.contains(&key) && !current_default_keys.contains(&key) {
                return false;
            }
            true
        });
        if rss_sources.len() != before_len {
            changed = true;
        }
    }

    let mut seen_keys = HashSet::new();
    let before_len = rss_sources.len();
    rss_sources.retain(|src| {
        let key = normalize_rss_url_key(&src.url);
        if key.is_empty() {
            return true;
        }
        if (current_default_keys.contains(&key) || stored_keys.contains(&key))
            && !seen_keys.insert(key)
        {
            return false;
        }
        true
    });
    if rss_sources.len() != before_len {
        changed = true;
    }

    for src in rss_sources.iter_mut() {
        let key = normalize_rss_url_key(&src.url);
        let Some((title, _url)) = default_by_key.get(&key) else {
            continue;
        };
        if removed.contains(&key) {
            continue;
        }
        if !title.trim().is_empty() && src.title != *title {
            src.title = title.clone();
            changed = true;
        }
        if src.user_title != (title.trim() != src.url.trim()) {
            src.user_title = title.trim() != src.url.trim();
            changed = true;
        }
        if !matches!(src.kind, RssSourceType::Feed) {
            src.kind = RssSourceType::Feed;
            changed = true;
        }
    }

    for (key, title, url) in &default_items {
        if removed.contains(key) || existing.contains(key) {
            continue;
        }
        rss_sources.push(RssSource {
            title: title.clone(),
            url: url.clone(),
            kind: RssSourceType::Feed,
            folder_path: Vec::new(),
            user_title: title.trim() != url.trim(),
            unread: false,
            cache: rss::RssFeedCache::default(),
            last_seen_guid: None,
            last_updated: None,
            removed_item_keys: Vec::new(),
            read_item_keys: Vec::new(),
        });
        existing.insert(key.clone());
        changed = true;
    }

    let mut new_keys: Vec<String> = current_default_keys.into_iter().collect();
    new_keys.sort();
    let mut old_keys: Vec<String> = keys_list.clone();
    old_keys.sort();
    if new_keys != old_keys {
        *keys_list = new_keys;
        changed = true;
    }
    changed
}

fn apply_defaults_for_news_language(
    settings: &mut crate::settings::AppSettings,
    language: crate::settings::Language,
    defaults: &[(String, String)],
) -> bool {
    match language {
        crate::settings::Language::German => apply_default_sources(
            &mut settings.rss_sources,
            &settings.rss_removed_default_de,
            &mut settings.rss_default_de_keys,
            defaults,
        ),
        crate::settings::Language::Italian => apply_default_sources(
            &mut settings.rss_sources,
            &settings.rss_removed_default_it,
            &mut settings.rss_default_it_keys,
            defaults,
        ),
        crate::settings::Language::Spanish => apply_default_sources(
            &mut settings.rss_sources,
            &settings.rss_removed_default_es,
            &mut settings.rss_default_es_keys,
            defaults,
        ),
        crate::settings::Language::Portuguese => apply_default_sources(
            &mut settings.rss_sources,
            &settings.rss_removed_default_pt,
            &mut settings.rss_default_pt_keys,
            defaults,
        ),
        crate::settings::Language::PortugueseBrazilian => apply_default_sources(
            &mut settings.rss_sources,
            &settings.rss_removed_default_pt_br,
            &mut settings.rss_default_pt_br_keys,
            defaults,
        ),
        crate::settings::Language::Czech => apply_default_sources(
            &mut settings.rss_sources,
            &settings.rss_removed_default_cs,
            &mut settings.rss_default_cs_keys,
            defaults,
        ),
        crate::settings::Language::Polish => apply_default_sources(
            &mut settings.rss_sources,
            &settings.rss_removed_default_pl,
            &mut settings.rss_default_pl_keys,
            defaults,
        ),
        crate::settings::Language::French => apply_default_sources(
            &mut settings.rss_sources,
            &settings.rss_removed_default_fr,
            &mut settings.rss_default_fr_keys,
            defaults,
        ),
        _ => apply_default_sources(
            &mut settings.rss_sources,
            &settings.rss_removed_default_en,
            &mut settings.rss_default_en_keys,
            defaults,
        ),
    }
}

fn ensure_default_sources(parent: HWND) {
    let language = news_language_as_app_language(&active_news_language_code(parent));
    let defaults = load_default_feeds(language);
    if defaults.is_empty() {
        return;
    }
    with_state(parent, |state| {
        if apply_defaults_for_news_language(&mut state.settings, language, &defaults) {
            save_rss_settings(state);
        }
    });
}

pub(crate) fn sync_default_sources_for_settings(
    settings: &mut crate::settings::AppSettings,
) -> bool {
    let code = normalize_news_language_code(&settings.rss_news_language)
        .unwrap_or_else(|| default_news_language_code(settings.language))
        .to_string();
    settings.rss_news_language = code.clone();

    let mut changed = load_or_migrate_active_rss_sources(settings, &code);

    let language = news_language_as_app_language(&code);
    let defaults = load_default_feeds(language);
    if apply_defaults_for_news_language(settings, language, &defaults) {
        changed = true;
    }
    let previous = settings
        .rss_sources_by_language
        .insert(code, settings.rss_sources.clone());
    if previous.as_ref() != Some(&settings.rss_sources) {
        changed = true;
    }
    changed
}

struct RssWindowState {
    parent: HWND,
    hwnd_tree: HWND,
    hwnd_preview: HWND,
    hwnd_language_combo: HWND,
    hwnd_import: HWND,
    hwnd_export: HWND,
    node_data: HashMap<isize, NodeData>,
    pending_fetches: HashMap<String, isize>, // URL -> hItem
    source_items: HashMap<isize, SourceItemsState>,
    enter_guard: bool,
    add_guard: bool,
    reorder_dialog: HWND,
    search_dialog: HWND,
    pending_edit: Option<usize>,
    tree_proc: WNDPROC,
    last_selected: isize,
    removed_history: Vec<RssLastRemoved>,
    suppress_tree_selection_events: bool,
    suppress_focus_restore_once: bool,
    preview_proc: WNDPROC,
    preview_request_seq: u64,
    community_add_dialog: HWND,
    community_list_dialog: HWND,
    city_dialog: HWND,
    folder_dialog: HWND,
    pending_local_category: isize,
}

#[derive(Clone)]
enum RssLastRemoved {
    Source {
        index: usize,
        source: RssSource,
        language: crate::settings::Language,
        default_removed_key_added: Option<String>,
    },
    Item {
        source_index: usize,
        item: RssItem,
        key: String,
        position: usize,
    },
    Folder {
        path: Vec<String>,
        sources: Vec<(usize, RssSource, Option<String>)>,
        folders: Vec<Vec<String>>,
        language: crate::settings::Language,
        news_code: String,
    },
}

#[derive(Clone)]
struct GoogleNewsCategory {
    title: String,
    url: String,
    is_local: bool,
}

struct GoogleNewsLocale {
    root_title: &'static str,
    top_title: &'static str,
    local_title: &'static str,
    nation_title: &'static str,
    world_title: &'static str,
    business_title: &'static str,
    technology_title: &'static str,
    entertainment_title: &'static str,
    sports_title: &'static str,
    health_title: &'static str,
    hl: &'static str,
    gl: &'static str,
    ceid: &'static str,
}

fn google_news_locale(code: &str) -> GoogleNewsLocale {
    match normalize_news_language_code(code).unwrap_or("it") {
        "en" => GoogleNewsLocale {
            root_title: "Google News English",
            top_title: "Top stories",
            local_title: "My City",
            nation_title: "US",
            world_title: "World",
            business_title: "Business",
            technology_title: "Science & Tech",
            entertainment_title: "Entertainment",
            sports_title: "Sports",
            health_title: "Health",
            hl: "en",
            gl: "US",
            ceid: "US:en",
        },
        "de" => GoogleNewsLocale {
            root_title: "Google News Deutschland",
            top_title: "Schlagzeilen",
            local_title: "Meine Stadt",
            nation_title: "Deutschland",
            world_title: "Welt",
            business_title: "Wirtschaft",
            technology_title: "Wissenschaft und Technik",
            entertainment_title: "Unterhaltung",
            sports_title: "Sport",
            health_title: "Gesundheit",
            hl: "de",
            gl: "DE",
            ceid: "DE:de",
        },
        "fr" => GoogleNewsLocale {
            root_title: "Google News France",
            top_title: "À la une",
            local_title: "Ma ville",
            nation_title: "France",
            world_title: "Monde",
            business_title: "Économie",
            technology_title: "Science et technologie",
            entertainment_title: "Divertissement",
            sports_title: "Sports",
            health_title: "Santé",
            hl: "fr",
            gl: "FR",
            ceid: "FR:fr",
        },
        "es" => GoogleNewsLocale {
            root_title: "Google News España",
            top_title: "Noticias principales",
            local_title: "Mi ciudad",
            nation_title: "España",
            world_title: "Mundo",
            business_title: "Negocios",
            technology_title: "Ciencia y Tecnología",
            entertainment_title: "Entretenimiento",
            sports_title: "Deportes",
            health_title: "Salud",
            hl: "es",
            gl: "ES",
            ceid: "ES:es",
        },
        "pt" => GoogleNewsLocale {
            root_title: "Google News Portugal",
            top_title: "Principais notícias",
            local_title: "A minha cidade",
            nation_title: "Portugal",
            world_title: "Mundo",
            business_title: "Negócios",
            technology_title: "Ciência e tecnologia",
            entertainment_title: "Entretenimento",
            sports_title: "Desporto",
            health_title: "Saúde",
            hl: "pt-PT",
            gl: "PT",
            ceid: "PT:pt-150",
        },
        "pt-br" => GoogleNewsLocale {
            root_title: "Google Notícias Brasil",
            top_title: "Principais notícias",
            local_title: "Minha cidade",
            nation_title: "Brasil",
            world_title: "Mundo",
            business_title: "Negócios",
            technology_title: "Ciência e tecnologia",
            entertainment_title: "Entretenimento",
            sports_title: "Esportes",
            health_title: "Saúde",
            hl: "pt-BR",
            gl: "BR",
            ceid: "BR:pt-419",
        },
        "pl" => GoogleNewsLocale {
            root_title: "Google News Polska",
            top_title: "Najważniejsze wiadomości",
            local_title: "Moje miasto",
            nation_title: "Polska",
            world_title: "Świat",
            business_title: "Biznes",
            technology_title: "Nauka i technologia",
            entertainment_title: "Rozrywka",
            sports_title: "Sport",
            health_title: "Zdrowie",
            hl: "pl",
            gl: "PL",
            ceid: "PL:pl",
        },
        "cs" => GoogleNewsLocale {
            root_title: "Google News Česko",
            top_title: "Hlavní zprávy",
            local_title: "Moje město",
            nation_title: "Česko",
            world_title: "Svět",
            business_title: "Byznys",
            technology_title: "Věda a technologie",
            entertainment_title: "Zábava",
            sports_title: "Sport",
            health_title: "Zdraví",
            hl: "cs",
            gl: "CZ",
            ceid: "CZ:cs",
        },
        _ => GoogleNewsLocale {
            root_title: "Google News Italia",
            top_title: "Notizie principali",
            local_title: "La mia città",
            nation_title: "Italia",
            world_title: "Dal mondo",
            business_title: "Affari",
            technology_title: "Scienza e tecnologia",
            entertainment_title: "Intrattenimento",
            sports_title: "Sport",
            health_title: "Salute",
            hl: "it",
            gl: "IT",
            ceid: "IT:it",
        },
    }
}

fn google_news_root_url(locale: &GoogleNewsLocale) -> String {
    format!(
        "https://news.google.com/rss?hl={}&gl={}&ceid={}",
        locale.hl, locale.gl, locale.ceid
    )
}

fn google_news_topic_url(locale: &GoogleNewsLocale, topic: &str) -> String {
    format!(
        "https://news.google.com/news/rss/headlines/section/topic/{}?hl={}&gl={}&ceid={}",
        topic, locale.hl, locale.gl, locale.ceid
    )
}

fn google_news_categories(code: &str) -> Vec<GoogleNewsCategory> {
    let locale = google_news_locale(code);
    vec![
        GoogleNewsCategory {
            title: locale.top_title.to_string(),
            url: google_news_root_url(&locale),
            is_local: false,
        },
        GoogleNewsCategory {
            title: locale.local_title.to_string(),
            url: String::new(),
            is_local: true,
        },
        GoogleNewsCategory {
            title: locale.nation_title.to_string(),
            url: google_news_topic_url(&locale, "NATION"),
            is_local: false,
        },
        GoogleNewsCategory {
            title: locale.world_title.to_string(),
            url: google_news_topic_url(&locale, "WORLD"),
            is_local: false,
        },
        GoogleNewsCategory {
            title: locale.business_title.to_string(),
            url: google_news_topic_url(&locale, "BUSINESS"),
            is_local: false,
        },
        GoogleNewsCategory {
            title: locale.technology_title.to_string(),
            url: google_news_topic_url(&locale, "TECHNOLOGY"),
            is_local: false,
        },
        GoogleNewsCategory {
            title: locale.entertainment_title.to_string(),
            url: google_news_topic_url(&locale, "ENTERTAINMENT"),
            is_local: false,
        },
        GoogleNewsCategory {
            title: locale.sports_title.to_string(),
            url: google_news_topic_url(&locale, "SPORTS"),
            is_local: false,
        },
        GoogleNewsCategory {
            title: locale.health_title.to_string(),
            url: google_news_topic_url(&locale, "HEALTH"),
            is_local: false,
        },
    ]
}

fn google_news_local_url(code: &str, city: &str) -> String {
    let locale = google_news_locale(code);
    format!(
        "https://news.google.com/rss/search?q={}&hl={}&gl={}&ceid={}",
        percent_encode(city.trim()),
        locale.hl,
        locale.gl,
        locale.ceid
    )
}

#[derive(Clone)]
enum NodeData {
    GoogleNewsRoot,
    GoogleNewsCategory(GoogleNewsCategory),
    Folder(Vec<String>),
    Source(usize), // Index in settings
    Item(RssItem),
    LoadMore,
}

struct SourceItemsState {
    items: Vec<RssItem>,
    loaded: usize,
    read_item_keys: HashSet<String>,
}

struct RssCityDialogState {
    parent: HWND,
    category_hitem: isize,
    edit: HWND,
}

struct RssFolderDialogState {
    parent: HWND,
    base_path: Vec<String>,
    edit: HWND,
}

fn read_control_text(hwnd: HWND) -> String {
    if hwnd.0 == 0 {
        return String::new();
    }
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let copied = GetWindowTextW(hwnd, &mut buffer);
        String::from_utf16_lossy(&buffer[..copied.max(0) as usize])
    }
}

fn with_rss_city_state<R>(
    hwnd: HWND,
    callback: impl FnOnce(&mut RssCityDialogState) -> R,
) -> Option<R> {
    let pointer = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut RssCityDialogState;
    crate::with_raw_mut_ptr_safe(pointer, callback)
}

fn with_rss_folder_state<R>(
    hwnd: HWND,
    callback: impl FnOnce(&mut RssFolderDialogState) -> R,
) -> Option<R> {
    let pointer =
        crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut RssFolderDialogState;
    crate::with_raw_mut_ptr_safe(pointer, callback)
}

fn show_rss_city_dialog(
    rss_hwnd: HWND,
    category_hitem: windows::Win32::UI::Controls::HTREEITEM,
    prefill: &str,
) {
    let existing = with_rss_state(rss_hwnd, |state| state.city_dialog).unwrap_or(HWND(0));
    if existing.0 != 0 {
        crate::set_foreground_window_safe(existing);
        let edit = with_rss_city_state(existing, |state| state.edit).unwrap_or(HWND(0));
        if edit.0 != 0 {
            crate::set_focus_safe(edit);
        }
        return;
    }

    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(RSS_CITY_WINDOW_CLASS);
        let window_class = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::LoadCursorW(
                None,
                windows::Win32::UI::WindowsAndMessaging::IDC_ARROW,
            )
            .unwrap_or_default(),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(rss_city_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&window_class);

        let language = with_rss_state(rss_hwnd, |state| {
            with_state(state.parent, |parent_state| parent_state.settings.language)
                .unwrap_or_default()
        })
        .unwrap_or_default();
        let title = to_wide(&i18n::tr(language, "rss.city.title"));
        let init = Box::new((rss_hwnd, category_hitem.0, prefill.to_string()));
        let init_ptr = Box::into_raw(init);
        let dialog = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            500,
            190,
            rss_hwnd,
            None,
            hinstance,
            Some(init_ptr as *const _),
        );
        if dialog.0 == 0 {
            let _unused = Box::from_raw(init_ptr);
            return;
        }
        with_rss_state(rss_hwnd, |state| {
            state.city_dialog = dialog;
            state.pending_local_category = category_hitem.0;
        });
        crate::enable_window_safe(rss_hwnd, false);
        crate::set_foreground_window_safe(dialog);
    }
}

unsafe extern "system" fn rss_city_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                let init_ptr = (*create).lpCreateParams as *mut (HWND, isize, String);
                if init_ptr.is_null() {
                    return LRESULT(-1);
                }
                let (parent, category_hitem, prefill) = *Box::from_raw(init_ptr);
                let language = with_rss_state(parent, |state| {
                    with_state(state.parent, |parent_state| parent_state.settings.language)
                        .unwrap_or_default()
                })
                .unwrap_or_default();
                let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
                let label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&i18n::tr(language, "rss.city.label")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    15,
                    450,
                    24,
                    hwnd,
                    None,
                    hinstance,
                    None,
                );
                let edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR(to_wide(&prefill).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    12,
                    45,
                    450,
                    28,
                    hwnd,
                    HMENU(ID_CITY_EDIT as isize),
                    hinstance,
                    None,
                );
                let ok = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "common.ok")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    252,
                    92,
                    100,
                    30,
                    hwnd,
                    HMENU(ID_CITY_OK as isize),
                    hinstance,
                    None,
                );
                let cancel = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "common.cancel")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    362,
                    92,
                    100,
                    30,
                    hwnd,
                    HMENU(ID_CITY_CANCEL as isize),
                    hinstance,
                    None,
                );
                for control in [edit, ok, cancel] {
                    subclass_auxiliary_dialog_control(control);
                }
                let font = with_rss_state(parent, |state| {
                    with_state(state.parent, |parent_state| parent_state.hfont).unwrap_or(HFONT(0))
                })
                .unwrap_or(HFONT(0));
                if font.0 != 0 {
                    for control in [label, edit, ok, cancel] {
                        SendMessageW(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
                    }
                }
                let state = Box::new(RssCityDialogState {
                    parent,
                    category_hitem,
                    edit,
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
                SetFocus(edit);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                match id {
                    ID_CITY_OK | 1 => {
                        let Some((parent, category_hitem, edit)) =
                            with_rss_city_state(hwnd, |state| {
                                (state.parent, state.category_hitem, state.edit)
                            })
                        else {
                            return LRESULT(0);
                        };
                        let city = read_control_text(edit).trim().to_string();
                        if city.is_empty() {
                            let language = with_rss_state(parent, |state| {
                                with_state(state.parent, |parent_state| {
                                    parent_state.settings.language
                                })
                                .unwrap_or_default()
                            })
                            .unwrap_or_default();
                            MessageBoxW(
                                hwnd,
                                PCWSTR(to_wide(&i18n::tr(language, "rss.city.required")).as_ptr()),
                                PCWSTR(to_wide(&i18n::tr(language, "rss.city.title")).as_ptr()),
                                MB_OK | MB_ICONINFORMATION,
                            );
                            SetFocus(edit);
                            return LRESULT(0);
                        }
                        let payload = Box::new(city);
                        let pointer = Box::into_raw(payload);
                        if let Err(error) = crate::post_message_w_safe(
                            parent,
                            WM_RSS_CITY_CHANGED,
                            WPARAM(category_hitem as usize),
                            LPARAM(pointer as isize),
                        ) {
                            let _payload_owner = Box::from_raw(pointer);
                            crate::log_debug(&format!(
                                "Failed to post RSS city change message: {}",
                                error
                            ));
                            return LRESULT(0);
                        }
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    ID_CITY_CANCEL | 2 => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_KEYDOWN => {
                if wparam.0 as u16 == VK_ESCAPE.0 {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RssCityDialogState;
                if !pointer.is_null() {
                    let state = Box::from_raw(pointer);
                    let parent = state.parent;
                    with_rss_state(parent, |rss_state| {
                        rss_state.city_dialog = HWND(0);
                        if rss_state.pending_local_category == state.category_hitem {
                            rss_state.pending_local_category = 0;
                        }
                    });
                    crate::enable_window_safe(parent, true);
                    crate::set_foreground_window_safe(parent);
                    let tree =
                        with_rss_state(parent, |rss_state| rss_state.hwnd_tree).unwrap_or(HWND(0));
                    if tree.0 != 0 {
                        crate::set_focus_safe(tree);
                    }
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn selected_folder_creation_base(hwnd: HWND) -> Vec<String> {
    let tree = with_rss_state(hwnd, |state| state.hwnd_tree).unwrap_or(HWND(0));
    if tree.0 == 0 {
        return Vec::new();
    }
    let selected = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    if selected.0 == 0 {
        return Vec::new();
    }
    with_rss_state(hwnd, |state| match state.node_data.get(&selected.0) {
        Some(NodeData::Folder(path)) => path.clone(),
        Some(NodeData::Source(index)) => with_state(state.parent, |parent_state| {
            parent_state
                .settings
                .rss_sources
                .get(*index)
                .map(|source| source.folder_path.clone())
                .unwrap_or_default()
        })
        .unwrap_or_default(),
        _ => Vec::new(),
    })
    .unwrap_or_default()
}

fn select_folder_path(hwnd: HWND, path: &[String]) {
    let target = with_rss_state(hwnd, |state| {
        state
            .node_data
            .iter()
            .find_map(|(handle, node)| match node {
                NodeData::Folder(candidate) if candidate == path => Some(*handle),
                _ => None,
            })
    })
    .flatten();
    let Some(handle) = target else {
        return;
    };
    let tree = with_rss_state(hwnd, |state| state.hwnd_tree).unwrap_or(HWND(0));
    if tree.0 == 0 {
        return;
    }
    crate::send_message_w_safe(
        tree,
        TVM_SELECTITEM,
        WPARAM(TVGN_CARET as usize),
        LPARAM(handle),
    );
    crate::send_message_w_safe(tree, TVM_ENSUREVISIBLE, WPARAM(0), LPARAM(handle));
    crate::set_focus_safe(tree);
}

fn show_create_rss_folder_dialog(rss_hwnd: HWND) {
    let existing = with_rss_state(rss_hwnd, |state| state.folder_dialog).unwrap_or(HWND(0));
    if existing.0 != 0 {
        crate::set_foreground_window_safe(existing);
        let edit = with_rss_folder_state(existing, |state| state.edit).unwrap_or(HWND(0));
        if edit.0 != 0 {
            crate::set_focus_safe(edit);
        }
        return;
    }

    let base_path = selected_folder_creation_base(rss_hwnd);
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(RSS_FOLDER_WINDOW_CLASS);
        let window_class = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::LoadCursorW(
                None,
                windows::Win32::UI::WindowsAndMessaging::IDC_ARROW,
            )
            .unwrap_or_default(),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(rss_folder_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&window_class);

        let language = with_rss_state(rss_hwnd, |state| {
            with_state(state.parent, |parent_state| parent_state.settings.language)
                .unwrap_or_default()
        })
        .unwrap_or_default();
        let title = to_wide(&i18n::tr(language, "rss.folder.title"));
        let init = Box::new((rss_hwnd, base_path));
        let init_ptr = Box::into_raw(init);
        let dialog = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            500,
            190,
            rss_hwnd,
            None,
            hinstance,
            Some(init_ptr as *const _),
        );
        if dialog.0 == 0 {
            let _unused_init = Box::from_raw(init_ptr);
            return;
        }
        with_rss_state(rss_hwnd, |state| state.folder_dialog = dialog);
        crate::enable_window_safe(rss_hwnd, false);
        crate::set_foreground_window_safe(dialog);
    }
}

unsafe extern "system" fn rss_folder_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "rss_folder_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || rss_folder_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn rss_folder_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let create = lparam.0 as *const CREATESTRUCTW;
                let init_ptr = (*create).lpCreateParams as *mut (HWND, Vec<String>);
                if init_ptr.is_null() {
                    return LRESULT(-1);
                }
                let (parent, base_path) = *Box::from_raw(init_ptr);
                let language = with_rss_state(parent, |state| {
                    with_state(state.parent, |parent_state| parent_state.settings.language)
                        .unwrap_or_default()
                })
                .unwrap_or_default();
                let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
                let label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&i18n::tr(language, "rss.folder.name")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    15,
                    450,
                    24,
                    hwnd,
                    None,
                    hinstance,
                    None,
                );
                let edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    12,
                    45,
                    450,
                    28,
                    hwnd,
                    HMENU(ID_FOLDER_EDIT as isize),
                    hinstance,
                    None,
                );
                let ok = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "common.ok")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    252,
                    92,
                    100,
                    30,
                    hwnd,
                    HMENU(ID_FOLDER_OK as isize),
                    hinstance,
                    None,
                );
                let cancel = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "common.cancel")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    362,
                    92,
                    100,
                    30,
                    hwnd,
                    HMENU(ID_FOLDER_CANCEL as isize),
                    hinstance,
                    None,
                );
                for control in [edit, ok, cancel] {
                    subclass_auxiliary_dialog_control(control);
                }
                let font = with_rss_state(parent, |state| {
                    with_state(state.parent, |parent_state| parent_state.hfont).unwrap_or(HFONT(0))
                })
                .unwrap_or(HFONT(0));
                if font.0 != 0 {
                    for control in [label, edit, ok, cancel] {
                        SendMessageW(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
                    }
                }
                let state = Box::new(RssFolderDialogState {
                    parent,
                    base_path,
                    edit,
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
                SetFocus(edit);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                match id {
                    ID_FOLDER_OK | 1 => {
                        let Some((parent, base_path, edit)) =
                            with_rss_folder_state(hwnd, |state| {
                                (state.parent, state.base_path.clone(), state.edit)
                            })
                        else {
                            return LRESULT(0);
                        };
                        let name = read_control_text(edit).trim().to_string();
                        let language = with_rss_state(parent, |state| {
                            with_state(state.parent, |parent_state| parent_state.settings.language)
                                .unwrap_or_default()
                        })
                        .unwrap_or_default();
                        if name.is_empty() {
                            MessageBoxW(
                                hwnd,
                                PCWSTR(
                                    to_wide(&i18n::tr(language, "rss.folder.required")).as_ptr(),
                                ),
                                PCWSTR(to_wide(&i18n::tr(language, "rss.folder.title")).as_ptr()),
                                MB_OK | MB_ICONINFORMATION,
                            );
                            SetFocus(edit);
                            return LRESULT(0);
                        }
                        let mut new_path = base_path;
                        new_path.push(name);
                        new_path = normalized_folder_path(&new_path);
                        let app_parent =
                            with_rss_state(parent, |state| state.parent).unwrap_or(HWND(0));
                        let created = with_state(app_parent, |app_state| {
                            let existing = rss_folder_paths_for_settings(&app_state.settings);
                            if existing.contains(&new_path) {
                                return false;
                            }
                            let code = active_news_language_code_from_state(app_state);
                            let folders = app_state
                                .settings
                                .rss_folders_by_language
                                .entry(code)
                                .or_default();
                            add_folder_path_with_parents(folders, &new_path);
                            save_rss_settings(app_state);
                            true
                        })
                        .unwrap_or(false);
                        if !created {
                            MessageBoxW(
                                hwnd,
                                PCWSTR(to_wide(&i18n::tr(language, "rss.folder.exists")).as_ptr()),
                                PCWSTR(to_wide(&i18n::tr(language, "rss.folder.title")).as_ptr()),
                                MB_OK | MB_ICONINFORMATION,
                            );
                            SetFocus(edit);
                            return LRESULT(0);
                        }
                        reload_tree(parent);
                        select_folder_path(parent, &new_path);
                        announce_rss_status(&i18n::tr(language, "rss.folder.created"));
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    ID_FOLDER_CANCEL | 2 => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_KEYDOWN => {
                if wparam.0 as u16 == VK_ESCAPE.0 {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RssFolderDialogState;
                if !pointer.is_null() {
                    let state = Box::from_raw(pointer);
                    let parent = state.parent;
                    with_rss_state(parent, |rss_state| rss_state.folder_dialog = HWND(0));
                    crate::enable_window_safe(parent, true);
                    crate::set_foreground_window_safe(parent);
                    let tree =
                        with_rss_state(parent, |rss_state| rss_state.hwnd_tree).unwrap_or(HWND(0));
                    if tree.0 != 0 {
                        crate::set_focus_safe(tree);
                    }
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn show_change_city_for_selected_category(hwnd: HWND) {
    let tree = with_rss_state(hwnd, |state| state.hwnd_tree).unwrap_or(HWND(0));
    if tree.0 == 0 {
        return;
    }
    let selected = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    let is_local = with_rss_state(hwnd, |state| {
        matches!(
            state.node_data.get(&selected.0),
            Some(NodeData::GoogleNewsCategory(category)) if category.is_local
        )
    })
    .unwrap_or(false);
    if !is_local {
        return;
    }
    let parent = with_rss_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
    let city =
        with_state(parent, |state| state.settings.rss_local_city.clone()).unwrap_or_default();
    show_rss_city_dialog(hwnd, selected, &city);
}

#[derive(Clone)]
struct CommunityNewsSource {
    name: String,
    url: String,
}

struct CommunityAddDialogState {
    parent: HWND,
    language: crate::settings::Language,
    edit_name: HWND,
    edit_url: HWND,
    submit: HWND,
    busy: bool,
}

struct CommunityListDialogState {
    parent: HWND,
    language: crate::settings::Language,
    list: HWND,
    add_button: HWND,
    sources: Vec<CommunityNewsSource>,
}

struct CommunitySubmitResult {
    result: Result<String, String>,
}

struct CommunityListResult {
    result: Result<Vec<CommunityNewsSource>, String>,
}

static CITY_DIALOG_TAB_ORDER: [usize; 3] = [ID_CITY_EDIT, ID_CITY_OK, ID_CITY_CANCEL];
static COMMUNITY_ADD_TAB_ORDER: [usize; 4] = [
    ID_COMMUNITY_ADD_NAME,
    ID_COMMUNITY_ADD_URL,
    ID_COMMUNITY_ADD_SUBMIT,
    ID_COMMUNITY_ADD_CANCEL,
];
static COMMUNITY_LIST_TAB_ORDER: [usize; 3] = [
    ID_COMMUNITY_LIST,
    ID_COMMUNITY_LIST_ADD,
    ID_COMMUNITY_LIST_CLOSE,
];
static FOLDER_DIALOG_TAB_ORDER: [usize; 3] = [ID_FOLDER_EDIT, ID_FOLDER_OK, ID_FOLDER_CANCEL];
static RSS_MAIN_BUTTON_TAB_ORDER: [usize; 7] = [
    ID_BTN_ADD,
    ID_BTN_COMMUNITY_ADD,
    ID_BTN_COMMUNITY_BROWSE,
    ID_BTN_IMPORT,
    ID_BTN_EXPORT,
    ID_BTN_SEARCH,
    ID_BTN_CLOSE,
];

fn auxiliary_dialog_navigation(id: usize) -> Option<(&'static [usize], usize, usize)> {
    if CITY_DIALOG_TAB_ORDER.contains(&id) {
        Some((&CITY_DIALOG_TAB_ORDER, ID_CITY_OK, ID_CITY_CANCEL))
    } else if COMMUNITY_ADD_TAB_ORDER.contains(&id) {
        Some((
            &COMMUNITY_ADD_TAB_ORDER,
            ID_COMMUNITY_ADD_SUBMIT,
            ID_COMMUNITY_ADD_CANCEL,
        ))
    } else if COMMUNITY_LIST_TAB_ORDER.contains(&id) {
        Some((
            &COMMUNITY_LIST_TAB_ORDER,
            ID_COMMUNITY_LIST_ADD,
            ID_COMMUNITY_LIST_CLOSE,
        ))
    } else if FOLDER_DIALOG_TAB_ORDER.contains(&id) {
        Some((&FOLDER_DIALOG_TAB_ORDER, ID_FOLDER_OK, ID_FOLDER_CANCEL))
    } else {
        None
    }
}

fn subclass_auxiliary_dialog_control(control: HWND) {
    if control.0 == 0 {
        return;
    }
    unsafe {
        let procedure = rss_auxiliary_control_wndproc as *const () as usize;
        let previous = SetWindowLongPtrW(control, GWLP_WNDPROC, procedure as isize);
        SetWindowLongPtrW(control, GWLP_USERDATA, previous);
    }
}

unsafe extern "system" fn rss_auxiliary_control_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "rss_auxiliary_control_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || rss_auxiliary_control_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn rss_auxiliary_control_wndproc_inner(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_CHAR && wparam.0 as u16 == VK_TAB.0 {
        return LRESULT(0);
    }
    if msg == WM_KEYDOWN {
        let id = crate::get_dlg_ctrl_id_safe(hwnd);
        if wparam.0 as u16 == VK_TAB.0
            && (id == ID_COMBO_NEWS_LANGUAGE || RSS_MAIN_BUTTON_TAB_ORDER.contains(&id))
        {
            let parent = crate::get_parent_safe(hwnd);
            let backwards =
                (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
            let target = if id == ID_COMBO_NEWS_LANGUAGE {
                if backwards {
                    if rss_article_preview_enabled(parent)
                        && selected_article_item(parent).is_some()
                    {
                        with_rss_state(parent, |state| state.hwnd_preview).unwrap_or(HWND(0))
                    } else {
                        with_rss_state(parent, |state| state.hwnd_tree).unwrap_or(HWND(0))
                    }
                } else {
                    crate::get_dlg_item_safe(parent, RSS_MAIN_BUTTON_TAB_ORDER[0] as i32)
                }
            } else if let Some(position) = RSS_MAIN_BUTTON_TAB_ORDER
                .iter()
                .position(|candidate| *candidate == id)
            {
                if backwards {
                    if position == 0 {
                        with_rss_state(parent, |state| state.hwnd_language_combo).unwrap_or(HWND(0))
                    } else {
                        crate::get_dlg_item_safe(
                            parent,
                            RSS_MAIN_BUTTON_TAB_ORDER[position - 1] as i32,
                        )
                    }
                } else if position + 1 == RSS_MAIN_BUTTON_TAB_ORDER.len() {
                    with_rss_state(parent, |state| state.hwnd_tree).unwrap_or(HWND(0))
                } else {
                    crate::get_dlg_item_safe(parent, RSS_MAIN_BUTTON_TAB_ORDER[position + 1] as i32)
                }
            } else {
                HWND(0)
            };
            if target.0 != 0 {
                crate::set_focus_safe(target);
                return LRESULT(0);
            }
        }
        if let Some((order, accept_id, cancel_id)) = auxiliary_dialog_navigation(id) {
            let parent = crate::get_parent_safe(hwnd);
            if wparam.0 as u16 == VK_TAB.0 {
                let backwards =
                    (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
                if let Some(position) = order.iter().position(|candidate| *candidate == id) {
                    for offset in 1..=order.len() {
                        let next_position = if backwards {
                            (position + order.len() - (offset % order.len())) % order.len()
                        } else {
                            (position + offset) % order.len()
                        };
                        let target = crate::get_dlg_item_safe(parent, order[next_position] as i32);
                        if target.0 != 0 && unsafe { IsWindowEnabled(target).as_bool() } {
                            crate::set_focus_safe(target);
                            return LRESULT(0);
                        }
                    }
                }
            }
            if wparam.0 as u16 == VK_RETURN.0 {
                let command = if id == cancel_id {
                    cancel_id
                } else {
                    accept_id
                };
                crate::send_message_w_safe(parent, WM_COMMAND, WPARAM(command), LPARAM(0));
                return LRESULT(0);
            }
            if wparam.0 as u16 == VK_ESCAPE.0 {
                crate::send_message_w_safe(parent, WM_COMMAND, WPARAM(cancel_id), LPARAM(0));
                return LRESULT(0);
            }
        }
    }
    let previous = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA);
    if previous == 0 {
        return crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam);
    }
    crate::call_window_proc_w_safe(
        crate::isize_to_wndproc_safe(previous),
        hwnd,
        msg,
        wparam,
        lparam,
    )
}

fn community_language_key(code: &str) -> &'static str {
    match normalize_news_language_code(code).unwrap_or("it") {
        "en" => "english",
        "fr" => "french",
        "es" => "spanish",
        "pt" => "portuguese",
        "pl" => "polish",
        "cs" => "czech",
        _ => "italian",
    }
}

fn normalize_community_language_key(value: &str) -> Option<&'static str> {
    let normalized = value.trim().to_lowercase().replace('_', "-");
    let primary = normalized.split('-').next().unwrap_or_default();
    match normalized.as_str() {
        "italian" | "italiano" => Some("italian"),
        "english" | "inglese" => Some("english"),
        "french" | "francese" | "français" | "francais" => Some("french"),
        "spanish" | "spagnolo" | "español" | "espanol" => Some("spanish"),
        "portuguese" | "portoghese" | "português" | "portugues" => Some("portuguese"),
        "polish" | "polacco" | "polski" => Some("polish"),
        "czech" | "ceco" | "čeština" | "cestina" => Some("czech"),
        "german" | "tedesco" | "deutsch" => Some("german"),
        _ => match primary {
            "it" => Some("italian"),
            "en" => Some("english"),
            "fr" => Some("french"),
            "es" => Some("spanish"),
            "pt" => Some("portuguese"),
            "pl" => Some("polish"),
            "cs" | "cz" => Some("czech"),
            "de" => Some("german"),
            _ => None,
        },
    }
}

// Keep the same duplicate comparison used by Sonarpad Mobile for community
// sources. In particular, do not merge http/https or slash variants here:
// they can resolve to different feeds on some sites.
fn community_source_url_key(url: &str) -> String {
    url.trim().to_lowercase()
}

fn app_language_code(language: crate::settings::Language) -> &'static str {
    match language {
        crate::settings::Language::Italian => "it",
        crate::settings::Language::German => "de",
        crate::settings::Language::English => "en",
        crate::settings::Language::Spanish => "es",
        crate::settings::Language::Portuguese => "pt",
        crate::settings::Language::PortugueseBrazilian => "pt-br",
        crate::settings::Language::Swedish => "sv",
        crate::settings::Language::Vietnamese => "vi",
        crate::settings::Language::Czech => "cs",
        crate::settings::Language::Polish => "pl",
        crate::settings::Language::French => "fr",
        crate::settings::Language::Serbian => "sr",
        crate::settings::Language::Ukrainian => "uk",
        crate::settings::Language::Lithuanian => "lt",
        crate::settings::Language::Russian => "ru",
        crate::settings::Language::Chinese => "zh",
        crate::settings::Language::Hindi => "hi",
    }
}

fn post_community_news_source(
    name: &str,
    url: &str,
    news_language: &str,
    ui_language: crate::settings::Language,
) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(COMMUNITY_USER_AGENT)
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .post(ADD_COMMUNITY_NEWS_SOURCE_URL)
        .header("Accept", "application/json")
        .form(&[
            ("name", name),
            ("url", url),
            ("language", community_language_key(news_language)),
            ("ui_language", app_language_code(ui_language)),
        ])
        .send()
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let body = response.text().map_err(|error| error.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|error| {
        if body.trim().is_empty() {
            format!("HTTP {}: {}", status, error)
        } else {
            format!("HTTP {}: {}", status, body.trim())
        }
    })?;
    let ok = json
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let message = json
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let error = json
        .get("error")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if !status.is_success() || !ok {
        Err(if error.is_empty() {
            format!("HTTP {}", status)
        } else {
            error
        })
    } else {
        Ok(message)
    }
}

fn unique_community_source_name(base_name: &str, known_names: &mut HashSet<String>) -> String {
    let base_name = base_name.trim();
    let mut candidate = base_name.to_string();
    let mut suffix = 2usize;
    while known_names.contains(&candidate.to_lowercase()) {
        candidate = format!("{} ({})", base_name, suffix);
        suffix += 1;
    }
    known_names.insert(candidate.to_lowercase());
    candidate
}

fn fetch_community_news_sources(
    news_language: &str,
    known_urls: HashSet<String>,
    mut known_names: HashSet<String>,
) -> Result<Vec<CommunityNewsSource>, String> {
    let expected_language = community_language_key(news_language);
    crate::log_debug(&format!(
        "RSS community list: request language={} code={} known_urls={}",
        expected_language,
        news_language,
        known_urls.len()
    ));
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent(COMMUNITY_USER_AGENT)
        .build()
        .map_err(|error| error.to_string())?;
    let request_bases = [
        COMMUNITY_NEWS_SOURCES_URL,
        "https://www.sonarpad.com/api/get_community_news_sources.php",
    ];
    let mut selected_json = None;
    let mut last_empty_json = None;
    let mut last_error = None;
    for (attempt, base_url) in request_bases.iter().enumerate() {
        let mut request_url = Url::parse(base_url).map_err(|error| error.to_string())?;
        let cache_buster = format!("{}-{}", Local::now().timestamp_millis(), attempt);
        request_url
            .query_pairs_mut()
            .append_pair("language", expected_language)
            .append_pair("lang", news_language)
            .append_pair("_ts", &cache_buster);
        crate::log_debug(&format!(
            "RSS community list: attempt={} endpoint={} cache_buster={}",
            attempt + 1,
            base_url,
            cache_buster
        ));
        let response = match client
            .get(request_url)
            .header("Accept", "application/json")
            .header(
                reqwest::header::CACHE_CONTROL,
                "no-cache, no-store, max-age=0",
            )
            .header(reqwest::header::PRAGMA, "no-cache")
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                crate::log_debug(&format!(
                    "RSS community list: attempt={} request failed: {}",
                    attempt + 1,
                    error
                ));
                last_error = Some(error.to_string());
                continue;
            }
        };
        let status = response.status();
        let effective_url = response.url().to_string();
        let body = match response.text() {
            Ok(body) => body,
            Err(error) => {
                crate::log_debug(&format!(
                    "RSS community list: attempt={} body read failed: {}",
                    attempt + 1,
                    error
                ));
                last_error = Some(error.to_string());
                continue;
            }
        };
        crate::log_debug(&format!(
            "RSS community list: attempt={} HTTP {} effective_url={} body_bytes={}",
            attempt + 1,
            status,
            effective_url,
            body.len()
        ));
        if !status.is_success() {
            last_error = Some(format!("HTTP {}", status));
            continue;
        }
        let json: serde_json::Value = match serde_json::from_str(&body) {
            Ok(json) => json,
            Err(error) => {
                crate::log_debug(&format!(
                    "RSS community list: attempt={} invalid JSON: {}",
                    attempt + 1,
                    error
                ));
                last_error = Some(error.to_string());
                continue;
            }
        };
        let item_count = if let Some(array) = json.as_array() {
            Some(array.len())
        } else {
            ["items", "sources", "data"]
                .iter()
                .find_map(|key| json.get(*key).and_then(serde_json::Value::as_array))
                .map(Vec::len)
        };
        let Some(item_count) = item_count else {
            crate::log_debug(&format!(
                "RSS community list: attempt={} response has no supported source array",
                attempt + 1
            ));
            last_error = Some("Invalid community source response".to_string());
            continue;
        };
        crate::log_debug(&format!(
            "RSS community list: attempt={} server_items={}",
            attempt + 1,
            item_count
        ));
        if item_count > 0 {
            selected_json = Some(json);
            break;
        }
        last_empty_json = Some(json);
    }
    let json = selected_json.or(last_empty_json).ok_or_else(|| {
        last_error.unwrap_or_else(|| "Unable to load community sources".to_string())
    })?;
    let items = if let Some(array) = json.as_array() {
        array
    } else {
        ["items", "sources", "data"]
            .iter()
            .find_map(|key| json.get(*key).and_then(serde_json::Value::as_array))
            .ok_or_else(|| "Invalid community source response".to_string())?
    };
    let mut results = Vec::new();
    let mut result_urls = HashSet::new();
    let mut skipped_invalid = 0usize;
    let mut skipped_language = 0usize;
    let mut skipped_known = 0usize;
    let mut skipped_duplicate = 0usize;
    for item in items {
        let name = item
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim();
        let url = item
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim();
        let language = item
            .get("language")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim();
        if name.is_empty() || url.is_empty() {
            skipped_invalid += 1;
            continue;
        }
        if !language.is_empty()
            && normalize_community_language_key(language) != Some(expected_language)
        {
            skipped_language += 1;
            continue;
        }
        let valid_url = matches!(
            Url::parse(url),
            Ok(parsed) if matches!(parsed.scheme(), "http" | "https") && parsed.host().is_some()
        );
        if !valid_url {
            skipped_invalid += 1;
            continue;
        }
        let normalized_url = community_source_url_key(url);
        if known_urls.contains(&normalized_url) {
            skipped_known += 1;
            continue;
        }
        if !result_urls.insert(normalized_url) {
            skipped_duplicate += 1;
            continue;
        }
        results.push(CommunityNewsSource {
            name: unique_community_source_name(name, &mut known_names),
            url: url.to_string(),
        });
    }
    results.sort_by_key(|source| source.name.to_lowercase());
    crate::log_debug(&format!(
        "RSS community list: selected_server_items={} available={} skipped_invalid={} skipped_language={} skipped_known={} skipped_duplicate={}",
        items.len(),
        results.len(),
        skipped_invalid,
        skipped_language,
        skipped_known,
        skipped_duplicate
    ));
    Ok(results)
}

fn with_community_add_state<R>(
    hwnd: HWND,
    callback: impl FnOnce(&mut CommunityAddDialogState) -> R,
) -> Option<R> {
    let pointer =
        crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut CommunityAddDialogState;
    crate::with_raw_mut_ptr_safe(pointer, callback)
}

fn with_community_list_state<R>(
    hwnd: HWND,
    callback: impl FnOnce(&mut CommunityListDialogState) -> R,
) -> Option<R> {
    let pointer =
        crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut CommunityListDialogState;
    crate::with_raw_mut_ptr_safe(pointer, callback)
}

fn show_community_add_dialog(rss_hwnd: HWND) {
    let existing = with_rss_state(rss_hwnd, |state| state.community_add_dialog).unwrap_or(HWND(0));
    if existing.0 != 0 {
        crate::set_foreground_window_safe(existing);
        return;
    }
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(RSS_COMMUNITY_ADD_WINDOW_CLASS);
        let window_class = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::LoadCursorW(
                None,
                windows::Win32::UI::WindowsAndMessaging::IDC_ARROW,
            )
            .unwrap_or_default(),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(community_add_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&window_class);
        let language = with_rss_state(rss_hwnd, |state| {
            with_state(state.parent, |parent_state| parent_state.settings.language)
                .unwrap_or_default()
        })
        .unwrap_or_default();
        let title = to_wide(&i18n::tr(language, "rss.community.add_title"));
        let dialog = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            650,
            330,
            rss_hwnd,
            None,
            hinstance,
            Some(rss_hwnd.0 as *const _),
        );
        if dialog.0 != 0 {
            with_rss_state(rss_hwnd, |state| state.community_add_dialog = dialog);
            crate::set_foreground_window_safe(dialog);
        }
    }
}

unsafe extern "system" fn community_add_wndproc(
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
                let language = with_rss_state(parent, |state| {
                    with_state(state.parent, |parent_state| parent_state.settings.language)
                        .unwrap_or_default()
                })
                .unwrap_or_default();
                let news_code = active_news_language_code(
                    with_rss_state(parent, |state| state.parent).unwrap_or(HWND(0)),
                );
                let selected_language = news_language_label(language, &news_code);
                let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
                let instructions = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&i18n::tr(language, "rss.community.instructions")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    12,
                    600,
                    48,
                    hwnd,
                    None,
                    hinstance,
                    None,
                );
                let name_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&i18n::tr(language, "rss.community.name_label")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    75,
                    175,
                    24,
                    hwnd,
                    None,
                    hinstance,
                    None,
                );
                let edit_name = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    190,
                    70,
                    420,
                    28,
                    hwnd,
                    HMENU(ID_COMMUNITY_ADD_NAME as isize),
                    hinstance,
                    None,
                );
                let url_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&i18n::tr(language, "rss.community.url_label")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    115,
                    175,
                    24,
                    hwnd,
                    None,
                    hinstance,
                    None,
                );
                let edit_url = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    190,
                    110,
                    420,
                    28,
                    hwnd,
                    HMENU(ID_COMMUNITY_ADD_URL as isize),
                    hinstance,
                    None,
                );
                let language_label_text = i18n::tr(language, "rss.community.selected_language")
                    .replace("{language}", &selected_language);
                let language_label = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&language_label_text).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    155,
                    598,
                    24,
                    hwnd,
                    None,
                    hinstance,
                    None,
                );
                let submit = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "rss.community.submit")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    350,
                    205,
                    150,
                    32,
                    hwnd,
                    HMENU(ID_COMMUNITY_ADD_SUBMIT as isize),
                    hinstance,
                    None,
                );
                let cancel = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "common.cancel")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    510,
                    205,
                    100,
                    32,
                    hwnd,
                    HMENU(ID_COMMUNITY_ADD_CANCEL as isize),
                    hinstance,
                    None,
                );
                for control in [edit_name, edit_url, submit, cancel] {
                    subclass_auxiliary_dialog_control(control);
                }
                let font = with_rss_state(parent, |state| {
                    with_state(state.parent, |parent_state| parent_state.hfont).unwrap_or(HFONT(0))
                })
                .unwrap_or(HFONT(0));
                if font.0 != 0 {
                    for control in [
                        instructions,
                        name_label,
                        edit_name,
                        url_label,
                        edit_url,
                        language_label,
                        submit,
                        cancel,
                    ] {
                        SendMessageW(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
                    }
                }
                let state = Box::new(CommunityAddDialogState {
                    parent,
                    language,
                    edit_name,
                    edit_url,
                    submit,
                    busy: false,
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
                SetFocus(edit_name);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                match id {
                    ID_COMMUNITY_ADD_SUBMIT | 1 => {
                        submit_community_news_source(hwnd);
                        LRESULT(0)
                    }
                    ID_COMMUNITY_ADD_CANCEL | 2 => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_RSS_COMMUNITY_SUBMIT_COMPLETE => {
                let pointer = lparam.0 as *mut CommunitySubmitResult;
                if pointer.is_null() {
                    return LRESULT(0);
                }
                let result = *Box::from_raw(pointer);
                let Some((parent, language, submit)) = with_community_add_state(hwnd, |state| {
                    state.busy = false;
                    (state.parent, state.language, state.submit)
                }) else {
                    return LRESULT(0);
                };
                crate::enable_window_safe(submit, true);
                crate::log_if_err!(crate::set_window_text_w_safe(
                    submit,
                    PCWSTR(to_wide(&i18n::tr(language, "rss.community.submit")).as_ptr()),
                ));
                match result.result {
                    Ok(message) => {
                        let message = if message.trim().is_empty() {
                            i18n::tr(language, "rss.community.added")
                        } else {
                            message
                        };
                        MessageBoxW(
                            hwnd,
                            PCWSTR(to_wide(&message).as_ptr()),
                            PCWSTR(
                                to_wide(&i18n::tr(language, "rss.community.add_title")).as_ptr(),
                            ),
                            MB_OK | MB_ICONINFORMATION,
                        );
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        focus_library(parent);
                    }
                    Err(error) => {
                        let message = i18n::tr(language, "rss.community.add_error")
                            .replace("{error}", &error);
                        MessageBoxW(
                            hwnd,
                            PCWSTR(to_wide(&message).as_ptr()),
                            PCWSTR(
                                to_wide(&i18n::tr(language, "rss.community.add_title")).as_ptr(),
                            ),
                            MB_OK | MB_ICONINFORMATION,
                        );
                    }
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 as u16 == VK_ESCAPE.0 {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let pointer =
                    GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut CommunityAddDialogState;
                if !pointer.is_null() {
                    let state = Box::from_raw(pointer);
                    with_rss_state(state.parent, |rss_state| {
                        rss_state.community_add_dialog = HWND(0)
                    });
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn submit_community_news_source(hwnd: HWND) {
    let Some((parent, language, name, url, already_busy, submit)) =
        with_community_add_state(hwnd, |state| {
            (
                state.parent,
                state.language,
                read_control_text(state.edit_name).trim().to_string(),
                read_control_text(state.edit_url).trim().to_string(),
                state.busy,
                state.submit,
            )
        })
    else {
        return;
    };
    if already_busy {
        return;
    }
    if name.is_empty() || url.is_empty() {
        unsafe {
            MessageBoxW(
                hwnd,
                PCWSTR(to_wide(&i18n::tr(language, "rss.community.missing_fields")).as_ptr()),
                PCWSTR(to_wide(&i18n::tr(language, "rss.community.add_title")).as_ptr()),
                MB_OK | MB_ICONINFORMATION,
            );
        }
        return;
    }
    with_community_add_state(hwnd, |state| state.busy = true);
    crate::enable_window_safe(submit, false);
    crate::log_if_err!(crate::set_window_text_w_safe(
        submit,
        PCWSTR(to_wide(&i18n::tr(language, "rss.community.checking")).as_ptr()),
    ));
    let news_language =
        active_news_language_code(with_rss_state(parent, |state| state.parent).unwrap_or(HWND(0)));
    let dialog_raw = hwnd.0;
    std::thread::spawn(move || {
        let result = post_community_news_source(&name, &url, &news_language, language);
        let payload = Box::new(CommunitySubmitResult { result });
        let pointer = Box::into_raw(payload);
        if crate::post_message_w_safe(
            HWND(dialog_raw),
            WM_RSS_COMMUNITY_SUBMIT_COMPLETE,
            WPARAM(0),
            LPARAM(pointer as isize),
        )
        .is_err()
        {
            let _unused = unsafe { Box::from_raw(pointer) };
        }
    });
}

fn show_community_sources_dialog(rss_hwnd: HWND) {
    let existing = with_rss_state(rss_hwnd, |state| state.community_list_dialog).unwrap_or(HWND(0));
    if existing.0 != 0 {
        crate::set_foreground_window_safe(existing);
        return;
    }
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(RSS_COMMUNITY_LIST_WINDOW_CLASS);
        let window_class = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::LoadCursorW(
                None,
                windows::Win32::UI::WindowsAndMessaging::IDC_ARROW,
            )
            .unwrap_or_default(),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(community_list_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&window_class);
        let language = with_rss_state(rss_hwnd, |state| {
            with_state(state.parent, |parent_state| parent_state.settings.language)
                .unwrap_or_default()
        })
        .unwrap_or_default();
        let title = to_wide(&i18n::tr(language, "rss.community.sources_title"));
        let dialog = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            650,
            500,
            rss_hwnd,
            None,
            hinstance,
            Some(rss_hwnd.0 as *const _),
        );
        if dialog.0 != 0 {
            with_rss_state(rss_hwnd, |state| state.community_list_dialog = dialog);
            crate::set_foreground_window_safe(dialog);
        }
    }
}

unsafe extern "system" fn community_list_wndproc(
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
                let language = with_rss_state(parent, |state| {
                    with_state(state.parent, |parent_state| parent_state.settings.language)
                        .unwrap_or_default()
                })
                .unwrap_or_default();
                let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
                let list = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_LISTBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(0x0001),
                    12,
                    12,
                    600,
                    370,
                    hwnd,
                    HMENU(ID_COMMUNITY_LIST as isize),
                    hinstance,
                    None,
                );
                let add_button = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "rss.community.add_to_library")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    350,
                    395,
                    150,
                    32,
                    hwnd,
                    HMENU(ID_COMMUNITY_LIST_ADD as isize),
                    hinstance,
                    None,
                );
                let close = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "common.ok")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    510,
                    395,
                    100,
                    32,
                    hwnd,
                    HMENU(ID_COMMUNITY_LIST_CLOSE as isize),
                    hinstance,
                    None,
                );
                for control in [list, add_button, close] {
                    subclass_auxiliary_dialog_control(control);
                }
                let loading = to_wide(&i18n::tr(language, "rss.community.loading"));
                SendMessageW(
                    list,
                    LB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(loading.as_ptr() as isize),
                );
                crate::enable_window_safe(add_button, false);
                let font = with_rss_state(parent, |state| {
                    with_state(state.parent, |parent_state| parent_state.hfont).unwrap_or(HFONT(0))
                })
                .unwrap_or(HFONT(0));
                if font.0 != 0 {
                    for control in [list, add_button, close] {
                        SendMessageW(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
                    }
                }
                let state = Box::new(CommunityListDialogState {
                    parent,
                    language,
                    list,
                    add_button,
                    sources: Vec::new(),
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
                SetFocus(list);
                start_community_sources_fetch(hwnd, parent);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let notification = (wparam.0 >> 16) & 0xffff;
                if id == ID_COMMUNITY_LIST && notification == LBN_DBLCLK as usize {
                    add_selected_community_source(hwnd);
                    return LRESULT(0);
                }
                match id {
                    ID_COMMUNITY_LIST_ADD | 1 => {
                        add_selected_community_source(hwnd);
                        LRESULT(0)
                    }
                    ID_COMMUNITY_LIST_CLOSE | 2 => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_RSS_COMMUNITY_LIST_COMPLETE => {
                let pointer = lparam.0 as *mut CommunityListResult;
                if pointer.is_null() {
                    return LRESULT(0);
                }
                let result = *Box::from_raw(pointer);
                let Some((language, list, add_button)) = with_community_list_state(hwnd, |state| {
                    (state.language, state.list, state.add_button)
                }) else {
                    return LRESULT(0);
                };
                SendMessageW(list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
                match result.result {
                    Ok(sources) => {
                        if sources.is_empty() {
                            let empty = to_wide(&i18n::tr(language, "rss.community.empty"));
                            SendMessageW(
                                list,
                                LB_ADDSTRING,
                                WPARAM(0),
                                LPARAM(empty.as_ptr() as isize),
                            );
                            crate::enable_window_safe(add_button, false);
                        } else {
                            for source in &sources {
                                let text = to_wide(&source.name);
                                SendMessageW(
                                    list,
                                    LB_ADDSTRING,
                                    WPARAM(0),
                                    LPARAM(text.as_ptr() as isize),
                                );
                            }
                            with_community_list_state(hwnd, |state| state.sources = sources);
                            SendMessageW(
                                list,
                                windows::Win32::UI::WindowsAndMessaging::LB_SETCURSEL,
                                WPARAM(0),
                                LPARAM(0),
                            );
                            crate::enable_window_safe(add_button, true);
                        }
                    }
                    Err(error) => {
                        let message = i18n::tr(language, "rss.community.fetch_error")
                            .replace("{error}", &error);
                        SendMessageW(
                            list,
                            LB_ADDSTRING,
                            WPARAM(0),
                            LPARAM(to_wide(&message).as_ptr() as isize),
                        );
                        crate::enable_window_safe(add_button, false);
                    }
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 as u16 == VK_ESCAPE.0 {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                if wparam.0 as u16 == VK_RETURN.0
                    && GetFocus() == GetDlgItem(hwnd, ID_COMMUNITY_LIST as i32)
                {
                    add_selected_community_source(hwnd);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let pointer =
                    GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut CommunityListDialogState;
                if !pointer.is_null() {
                    let state = Box::from_raw(pointer);
                    with_rss_state(state.parent, |rss_state| {
                        rss_state.community_list_dialog = HWND(0)
                    });
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn start_community_sources_fetch(dialog: HWND, rss_hwnd: HWND) {
    let parent = with_rss_state(rss_hwnd, |state| state.parent).unwrap_or(HWND(0));
    let news_language = active_news_language_code(parent);
    let (known_urls, known_names) = with_state(parent, |state| {
        let urls = state
            .settings
            .rss_sources
            .iter()
            .map(|source| community_source_url_key(&source.url))
            .filter(|url| !url.is_empty())
            .collect();
        let names = state
            .settings
            .rss_sources
            .iter()
            .map(|source| source.title.trim().to_lowercase())
            .filter(|name| !name.is_empty())
            .collect();
        (urls, names)
    })
    .unwrap_or_default();
    let dialog_raw = dialog.0;
    std::thread::spawn(move || {
        let result = fetch_community_news_sources(&news_language, known_urls, known_names);
        let payload = Box::new(CommunityListResult { result });
        let pointer = Box::into_raw(payload);
        if crate::post_message_w_safe(
            HWND(dialog_raw),
            WM_RSS_COMMUNITY_LIST_COMPLETE,
            WPARAM(0),
            LPARAM(pointer as isize),
        )
        .is_err()
        {
            let _unused = unsafe { Box::from_raw(pointer) };
        }
    });
}

fn add_selected_community_source(hwnd: HWND) {
    let Some((parent, language, source)) = with_community_list_state(hwnd, |state| {
        let selected = crate::send_message_w_safe(state.list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        let source = if selected >= 0 {
            state.sources.get(selected as usize).cloned()
        } else {
            None
        };
        (state.parent, state.language, source)
    }) else {
        return;
    };
    let Some(source) = source else {
        return;
    };
    let app_parent = with_rss_state(parent, |state| state.parent).unwrap_or(HWND(0));
    let added = with_state(app_parent, |state| {
        let key = community_source_url_key(&source.url);
        if state
            .settings
            .rss_sources
            .iter()
            .any(|existing| community_source_url_key(&existing.url) == key)
        {
            return false;
        }
        state.settings.rss_sources.push(RssSource {
            title: source.name.clone(),
            url: source.url.clone(),
            kind: RssSourceType::Feed,
            folder_path: Vec::new(),
            user_title: true,
            unread: false,
            cache: rss::RssFeedCache::default(),
            last_seen_guid: None,
            last_updated: None,
            removed_item_keys: Vec::new(),
            read_item_keys: Vec::new(),
        });
        save_rss_settings(state);
        true
    })
    .unwrap_or(false);
    if added {
        reload_tree(parent);
        let message =
            i18n::tr(language, "rss.community.added_to_library").replace("{name}", &source.name);
        announce_rss_status(&message);
        unsafe {
            MessageBoxW(
                hwnd,
                PCWSTR(to_wide(&message).as_ptr()),
                PCWSTR(to_wide(&i18n::tr(language, "rss.community.sources_title")).as_ptr()),
                MB_OK | MB_ICONINFORMATION,
            );
        }
        crate::log_if_err!(crate::destroy_window_safe(hwnd));
        focus_library(parent);
    }
}

struct AddDialogInit {
    parent: HWND,
    prefill_title: String,
    prefill_url: String,
    hide_url_field: bool,
}

struct SearchDialogInit {
    parent: HWND,
}

struct PreviewResult {
    request_seq: u64,
    text: String,
}

fn rss_article_preview_enabled(hwnd: HWND) -> bool {
    with_rss_state(hwnd, |s| {
        with_state(s.parent, |ps| ps.settings.rss_show_article_preview).unwrap_or(true)
    })
    .unwrap_or(true)
}

fn set_preview_text(hwnd_preview: HWND, text: &str) {
    if hwnd_preview.0 == 0 {
        return;
    }
    crate::log_if_err!(crate::set_window_text_w_safe(
        hwnd_preview,
        PCWSTR(to_wide(text).as_ptr())
    ));
}

fn article_preview_fallback_text(item: &RssItem) -> String {
    let mut parts = Vec::new();
    if !item.title.trim().is_empty() {
        parts.push(item.title.trim().to_string());
    }
    if !item.link.trim().is_empty() {
        parts.push(item.link.trim().to_string());
    }
    normalize_article_text(&parts.join("\n\n"))
}

fn update_preview_layout(hwnd: HWND) {
    let (hwnd_tree, hwnd_preview) =
        with_rss_state(hwnd, |s| (s.hwnd_tree, s.hwnd_preview)).unwrap_or((HWND(0), HWND(0)));
    if hwnd_tree.0 == 0 || hwnd_preview.0 == 0 {
        return;
    }

    let show_preview = rss_article_preview_enabled(hwnd);
    let (tree_height, preview_cmd) = if show_preview {
        (345, SW_SHOW)
    } else {
        (525, SW_HIDE)
    };

    unsafe {
        crate::log_if_err!(MoveWindow(hwnd_tree, 10, 10, 780, tree_height, true));
    }
    crate::show_window_safe(hwnd_preview, preview_cmd);
    crate::enable_window_safe(hwnd_preview, show_preview);
    if show_preview {
        unsafe {
            crate::log_if_err!(MoveWindow(hwnd_preview, 10, 365, 780, 170, true));
        }
    }
}

fn request_article_preview(hwnd: HWND, item: RssItem) {
    let Some((parent, request_seq, hwnd_preview)) = with_rss_state(hwnd, |s| {
        s.preview_request_seq = s.preview_request_seq.wrapping_add(1);
        (s.parent, s.preview_request_seq, s.hwnd_preview)
    }) else {
        return;
    };

    let fallback = article_preview_fallback_text(&item);
    set_preview_text(hwnd_preview, &fallback);

    if parent.0 != 0 {
        ensure_rss_http(parent);
    }

    let url = item.link.clone();
    let title = item.title.clone();
    let description = item.description.clone();
    let language = if parent.0 != 0 {
        news_language_as_app_language(&active_news_language_code(parent))
    } else {
        crate::settings::Language::default()
    };

    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                crate::log_debug(&format!("Failed to build tokio runtime: {}", err));
                return;
            }
        };

        let text = match rt.block_on(crate::tools::rss::fetch_article_text(
            &url,
            &title,
            &description,
            language,
        )) {
            Ok(text) => normalize_article_text(&text),
            Err(err) => {
                crate::log_debug(&format!(
                    "rss_preview_fallback url=\"{}\" error=\"{}\"",
                    url, err
                ));
                fallback
            }
        };

        let payload = Box::new(PreviewResult { request_seq, text });
        let _unused = crate::post_message_w_safe(
            hwnd,
            WM_RSS_PREVIEW_COMPLETE,
            WPARAM(0),
            LPARAM(Box::into_raw(payload) as isize),
        );
    });
}

fn refresh_article_preview_for_selection(hwnd: HWND) {
    let hwnd_preview = with_rss_state(hwnd, |s| s.hwnd_preview).unwrap_or(HWND(0));
    if hwnd_preview.0 == 0 {
        return;
    }
    if !rss_article_preview_enabled(hwnd) {
        set_preview_text(hwnd_preview, "");
        return;
    }
    let Some(item) = selected_article_item(hwnd) else {
        set_preview_text(hwnd_preview, "");
        return;
    };
    request_article_preview(hwnd, item);
}

pub fn open(parent: HWND) {
    unsafe {
        let exists = with_state(parent, |s| s.rss_window).unwrap_or(HWND(0));
        if exists.0 != 0 {
            SetForegroundWindow(exists);
            return;
        }

        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(RSS_WINDOW_CLASS);

        let wc = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                windows::Win32::UI::WindowsAndMessaging::LoadCursorW(
                    None,
                    windows::Win32::UI::WindowsAndMessaging::IDC_ARROW,
                )
                .unwrap_or_default()
                .0,
            ),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(rss_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let language = with_state(parent, |s| s.settings.language).unwrap_or_default();
        let title = to_wide(&i18n::tr(language, "rss.window.title"));

        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            820,
            760,
            parent,
            None,
            hinstance,
            Some(parent.0 as *const _),
        );

        if hwnd.0 != 0 && with_state(parent, |s| s.rss_window = hwnd).is_none() {
            crate::log_debug("Failed to set rss_window state");
            // IMPORTANT: do NOT disable the parent window.
            // If the parent is disabled, Windows (and NVDA) treat the editor as unavailable,
            // and SetFocus/SetForegroundWindow will not behave reliably.
        }
    }
}

pub fn show_context_menu_from_keyboard(hwnd: HWND) {
    let mut pt = POINT::default();
    crate::log_if_err!(crate::get_cursor_pos_safe(&mut pt));
    show_rss_context_menu(hwnd, pt.x, pt.y, false);
}

pub fn focus_library(hwnd: HWND) {
    if hwnd.0 == 0 {
        return;
    }
    crate::set_foreground_window_safe(hwnd);
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 != 0 {
        select_first_root_if_needed(hwnd, hwnd_tree);
        crate::set_focus_safe(hwnd_tree);
    }
}

fn show_rss_context_menu(hwnd: HWND, x: i32, y: i32, use_hit_test: bool) {
    unsafe {
        let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
        if hwnd_tree.0 == 0 {
            return;
        }

        let mut rect = RECT::default();
        if use_hit_test
            && GetWindowRect(hwnd_tree, &mut rect).is_ok()
            && (x < rect.left || x > rect.right || y < rect.top || y > rect.bottom)
        {
            return;
        }

        let hitem = if use_hit_test {
            let mut pt = POINT { x, y };
            if rect.right != 0 || rect.bottom != 0 {
                pt.x -= rect.left;
                pt.y -= rect.top;
            }
            let mut hit = TVHITTESTINFO {
                pt,
                ..Default::default()
            };
            let hitem = windows::Win32::UI::Controls::HTREEITEM(
                SendMessageW(
                    hwnd_tree,
                    TVM_HITTEST,
                    WPARAM(0),
                    LPARAM(&mut hit as *mut _ as isize),
                )
                .0,
            );
            if hitem.0 != 0 {
                SendMessageW(
                    hwnd_tree,
                    TVM_SELECTITEM,
                    WPARAM(TVGN_CARET as usize),
                    LPARAM(hitem.0),
                );
                SendMessageW(hwnd_tree, TVM_ENSUREVISIBLE, WPARAM(0), LPARAM(hitem.0));
            }
            hitem
        } else {
            windows::Win32::UI::Controls::HTREEITEM(
                SendMessageW(
                    hwnd_tree,
                    TVM_GETNEXTITEM,
                    WPARAM(TVGN_CARET as usize),
                    LPARAM(0),
                )
                .0,
            )
        };

        if hitem.0 == 0 {
            return;
        }

        let (is_source, source_index, folder_path, article_item, local_google_category) =
            with_rss_state(hwnd, |state| match state.node_data.get(&hitem.0) {
                Some(NodeData::Source(index)) => (true, Some(*index), None, None, false),
                Some(NodeData::Folder(path)) => (false, None, Some(path.clone()), None, false),
                Some(NodeData::Item(item)) => (false, None, None, Some(item.clone()), false),
                Some(NodeData::GoogleNewsCategory(category)) if category.is_local => {
                    (false, None, None, None, true)
                }
                _ => (false, None, None, None, false),
            })
            .unwrap_or((false, None, None, None, false));
        if !is_source && folder_path.is_none() && article_item.is_none() && !local_google_category {
            return;
        }

        let language = with_rss_state(hwnd, |s| {
            with_state(s.parent, |ps| ps.settings.language).unwrap_or_default()
        })
        .unwrap_or_default();
        let edit_label = i18n::tr(language, "rss.context.edit");
        let delete_label = i18n::tr(language, "rss.context.delete");
        let remove_entry_label = i18n::tr(language, "dictionary.remove");
        let retry_label = i18n::tr(language, "rss.context.retry_now");
        let reorder_label = i18n::tr(language, "rss.context.reorder");
        let reorder_up = i18n::tr(language, "rss.reorder.move_up");
        let reorder_down = i18n::tr(language, "rss.reorder.move_down");
        let reorder_top = i18n::tr(language, "rss.reorder.move_top");
        let reorder_bottom = i18n::tr(language, "rss.reorder.move_bottom");
        let reorder_position = i18n::tr(language, "rss.reorder.move_to_position");
        let move_to_folder_label = i18n::tr(language, "rss.reorder.move_to_folder");
        let main_folder_label = i18n::tr(language, "rss.folder.main");
        let sort_asc = i18n::tr(language, "rss.reorder.title_asc");
        let sort_desc = i18n::tr(language, "rss.reorder.title_desc");
        let sort_newest = i18n::tr(language, "rss.reorder.date_newest");
        let sort_oldest = i18n::tr(language, "rss.reorder.date_oldest");
        let open_label = i18n::tr(language, "rss.context.open_browser");
        let facebook_label = i18n::tr(language, "rss.context.share_facebook");
        let twitter_label = i18n::tr(language, "rss.context.share_twitter");
        let whatsapp_label = i18n::tr(language, "rss.context.share_whatsapp");
        let email_label = i18n::tr(language, "rss.context.share_email");
        let add_to_favorites_label = i18n::tr(language, "rss.context.add_to_favorites");
        let select_articles_label = i18n::tr(language, "rss.context.select_articles");
        let properties_label = i18n::tr(language, "context.properties");
        let change_city_label = i18n::tr(language, "rss.city.change");
        let create_folder_label = i18n::tr(language, "rss.context.create_folder");
        let undo_label = i18n::tr(language, "edit.undo")
            .split('\t')
            .next()
            .unwrap_or_default()
            .to_string();
        let has_undo = with_rss_state(hwnd, |s| !s.removed_history.is_empty()).unwrap_or(false);
        let is_favorites_source_node = source_index
            .and_then(|idx| {
                with_rss_state(hwnd, |s| {
                    with_state(s.parent, |ps| {
                        ps.settings
                            .rss_sources
                            .get(idx)
                            .map(is_favorites_source)
                            .unwrap_or(false)
                    })
                })
                .flatten()
            })
            .unwrap_or(false);

        if let Ok(menu) = CreatePopupMenu()
            && menu.0 != 0
        {
            if local_google_category {
                if let Err(_error) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_CHANGE_CITY,
                    PCWSTR(to_wide(&change_city_label).as_ptr()),
                ) {}
            } else if folder_path.is_some() {
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_CREATE_FOLDER,
                    PCWSTR(to_wide(&create_folder_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_DELETE,
                    PCWSTR(to_wide(&i18n::tr(language, "rss.folder.delete_title")).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) {}
                let undo_flags = if has_undo {
                    MF_STRING
                } else {
                    MF_STRING | MF_GRAYED
                };
                if let Err(_e) = AppendMenuW(
                    menu,
                    undo_flags,
                    ID_CTX_UNDO_DELETE,
                    PCWSTR(to_wide(&undo_label).as_ptr()),
                ) {}
            } else if is_source {
                let source_action_flags = if is_favorites_source_node {
                    MF_STRING | MF_GRAYED
                } else {
                    MF_STRING
                };
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_CREATE_FOLDER,
                    PCWSTR(to_wide(&create_folder_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    source_action_flags,
                    ID_CTX_EDIT,
                    PCWSTR(to_wide(&edit_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_DELETE,
                    PCWSTR(to_wide(&delete_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    source_action_flags,
                    ID_CTX_RETRY,
                    PCWSTR(to_wide(&retry_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_PROPERTIES,
                    PCWSTR(to_wide(&properties_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) {}
                let undo_flags = if has_undo {
                    MF_STRING
                } else {
                    MF_STRING | MF_GRAYED
                };
                if let Err(_e) = AppendMenuW(
                    menu,
                    undo_flags,
                    ID_CTX_UNDO_DELETE,
                    PCWSTR(to_wide(&undo_label).as_ptr()),
                ) {}
                if let Some(idx) = source_index {
                    let (position, total) = with_rss_state(hwnd, |s| {
                        with_state(s.parent, |ps| {
                            let siblings = source_sibling_indices(&ps.settings, idx);
                            let position = siblings
                                .iter()
                                .position(|candidate| *candidate == idx)
                                .unwrap_or(0);
                            (position, siblings.len())
                        })
                    })
                    .flatten()
                    .unwrap_or((0, 0));
                    let at_top = position == 0;
                    let at_bottom = total == 0 || position + 1 >= total;
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
                        if !is_favorites_source_node {
                            let destinations = with_rss_state(hwnd, |state| {
                                with_state(state.parent, |parent_state| {
                                    move_folder_destinations(&parent_state.settings, idx)
                                })
                                .unwrap_or_default()
                            })
                            .unwrap_or_default();
                            if !destinations.is_empty()
                                && let Ok(folder_menu) = CreatePopupMenu()
                                && folder_menu.0 != 0
                            {
                                for (destination_index, destination) in destinations
                                    .iter()
                                    .take(ID_CTX_MOVE_FOLDER_LIMIT)
                                    .enumerate()
                                {
                                    let label =
                                        folder_destination_label(destination, &main_folder_label);
                                    if let Err(_e) = AppendMenuW(
                                        folder_menu,
                                        MF_STRING,
                                        ID_CTX_MOVE_FOLDER_BASE + destination_index,
                                        PCWSTR(to_wide(&label).as_ptr()),
                                    ) {}
                                }
                                if let Err(_e) = AppendMenuW(
                                    submenu,
                                    MF_POPUP,
                                    folder_menu.0 as usize,
                                    PCWSTR(to_wide(&move_to_folder_label).as_ptr()),
                                ) {}
                            }
                        }
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
                }
            } else if let Some(item) = article_item {
                let url = item.link.trim();
                let valid_url = !url.is_empty() && is_valid_article_url(url);
                let flags = if valid_url {
                    MF_STRING
                } else {
                    MF_STRING | MF_GRAYED
                };
                if let Err(_e) = AppendMenuW(
                    menu,
                    flags,
                    ID_CTX_OPEN_BROWSER,
                    PCWSTR(to_wide(&open_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    flags,
                    ID_CTX_SHARE_FACEBOOK,
                    PCWSTR(to_wide(&facebook_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    flags,
                    ID_CTX_SHARE_TWITTER,
                    PCWSTR(to_wide(&twitter_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    flags,
                    ID_CTX_SHARE_WHATSAPP,
                    PCWSTR(to_wide(&whatsapp_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    flags,
                    ID_CTX_SHARE_EMAIL,
                    PCWSTR(to_wide(&email_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_ADD_TO_FAVORITES,
                    PCWSTR(to_wide(&add_to_favorites_label).as_ptr()),
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
                    ID_CTX_SELECT_ARTICLES,
                    PCWSTR(to_wide(&select_articles_label).as_ptr()),
                ) {}
                if let Err(_e) = AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_CTX_DELETE,
                    PCWSTR(to_wide(&remove_entry_label).as_ptr()),
                ) {}
                let undo_flags = if has_undo {
                    MF_STRING
                } else {
                    MF_STRING | MF_GRAYED
                };
                if let Err(_e) = AppendMenuW(
                    menu,
                    undo_flags,
                    ID_CTX_UNDO_DELETE,
                    PCWSTR(to_wide(&undo_label).as_ptr()),
                ) {}
            }
            SetForegroundWindow(hwnd);
            if !TrackPopupMenu(
                menu,
                windows::Win32::UI::WindowsAndMessaging::TPM_RIGHTBUTTON,
                x,
                y,
                0,
                hwnd,
                None,
            )
            .as_bool()
            {
                crate::log_debug("TrackPopupMenu failed");
            }
            if let Err(e) = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0)) {
                crate::log_debug(&format!("Failed to post WM_NULL: {}", e));
            }
            crate::log_if_err!(DestroyMenu(menu));
        }
    }
}

unsafe extern "system" fn rss_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "rss_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || rss_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn rss_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const CREATESTRUCTW;
                let parent = HWND((*cs).lpCreateParams as isize);

                let state = Box::new(RssWindowState {
                    parent,
                    hwnd_tree: HWND(0),
                    hwnd_preview: HWND(0),
                    hwnd_language_combo: HWND(0),
                    hwnd_import: HWND(0),
                    hwnd_export: HWND(0),
                    node_data: HashMap::new(),
                    pending_fetches: HashMap::new(),
                    source_items: HashMap::new(),
                    enter_guard: false,
                    add_guard: false,
                    reorder_dialog: HWND(0),
                    search_dialog: HWND(0),
                    pending_edit: None,
                    tree_proc: None,
                    last_selected: 0,
                    removed_history: Vec::new(),
                    suppress_tree_selection_events: false,
                    suppress_focus_restore_once: false,
                    preview_proc: None,
                    preview_request_seq: 0,
                    community_add_dialog: HWND(0),
                    community_list_dialog: HWND(0),
                    city_dialog: HWND(0),
                    folder_dialog: HWND(0),
                    pending_local_category: 0,
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

                prepare_rss_language_state(parent);
                create_controls(hwnd);
                ensure_default_sources(parent);
                reload_tree(hwnd);
                let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                if hwnd_tree.0 != 0 {
                    select_first_root_if_needed(hwnd, hwnd_tree);
                    SetFocus(hwnd_tree);
                }

                // Start background check for new articles on all feeds
                start_background_unread_check(hwnd);

                LRESULT(0)
            }
            WM_DESTROY => {
                let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                if parent.0 != 0 {
                    if with_state(parent, |s| s.rss_window = HWND(0)).is_none() {
                        crate::log_debug("Failed to reset rss_window state");
                    }
                    // Parent was never disabled; just bring it to front as a convenience.
                    // Only focus editor if not in player mode (audiobook)
                    if !crate::editor_manager::is_current_audiobook(parent) {
                        force_focus_editor_on_parent(parent);
                    }
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RssWindowState;
                if !ptr.is_null() {
                    let parent = (*ptr).parent;
                    let hwnd_tree = (*ptr).hwnd_tree;
                    let hwnd_preview = (*ptr).hwnd_preview;
                    if hwnd_tree.0 != 0
                        && let Some(proc) = (*ptr).tree_proc
                    {
                        let proc_ptr = proc as usize;
                        SetWindowLongPtrW(hwnd_tree, GWLP_WNDPROC, proc_ptr as isize);
                    }
                    if hwnd_preview.0 != 0
                        && let Some(proc) = (*ptr).preview_proc
                    {
                        let proc_ptr = proc as usize;
                        SetWindowLongPtrW(hwnd_preview, GWLP_WNDPROC, proc_ptr as isize);
                    }
                    if parent.0 != 0 {
                        force_focus_editor_on_parent(parent);
                    }
                    let _unused_box = Box::from_raw(ptr);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let notification = (wparam.0 >> 16) & 0xffff;
                if id == ID_COMBO_NEWS_LANGUAGE && notification == CBN_SELCHANGE as usize {
                    let combo =
                        with_rss_state(hwnd, |state| state.hwnd_language_combo).unwrap_or(HWND(0));
                    if combo.0 != 0 {
                        let selected = SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
                        if selected >= 0
                            && let Some(code) = NEWS_LANGUAGE_CODES.get(selected as usize)
                        {
                            switch_news_language(hwnd, code);
                        }
                    }
                    return LRESULT(0);
                }
                match id {
                    ID_BTN_CLOSE | 2 => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    ID_BTN_ADD => {
                        // Direct activation (mouse click, or Space/Enter if the button sends BN_CLICKED).
                        // Guard against the common case where Enter also generates IDOK (1),
                        // which would otherwise open the dialog twice and crash.
                        let already = with_rss_state(hwnd, |s| s.add_guard).unwrap_or(false);
                        if !already {
                            with_rss_state(hwnd, |s| s.add_guard = true);
                            if let Err(_e) =
                                PostMessageW(hwnd, WM_SHOW_ADD_DIALOG, WPARAM(0), LPARAM(0))
                            {
                                crate::log_debug(&format!("Error: {:?}", _e));
                            }
                        }
                        LRESULT(0)
                    }
                    ID_BTN_COMMUNITY_ADD => {
                        show_community_add_dialog(hwnd);
                        LRESULT(0)
                    }
                    ID_BTN_COMMUNITY_BROWSE => {
                        show_community_sources_dialog(hwnd);
                        LRESULT(0)
                    }
                    ID_BTN_IMPORT => {
                        let language = with_rss_state(hwnd, |s| {
                            with_state(s.parent, |ps| ps.settings.language).unwrap_or_default()
                        })
                        .unwrap_or_default();
                        if let Some(path) = open_import_txt_dialog(hwnd, language) {
                            let count = import_sources_from_file(hwnd, &path);
                            if count > 0 {
                                let title = i18n::tr(language, "rss.window.title");
                                let count_text = count.to_string();
                                let path_text = path.display().to_string();
                                let message = i18n::tr_f(
                                    language,
                                    "rss.import_success",
                                    &[("count", &count_text), ("path", &path_text)],
                                );
                                MessageBoxW(
                                    hwnd,
                                    PCWSTR(to_wide(&message).as_ptr()),
                                    PCWSTR(to_wide(&title).as_ptr()),
                                    MB_OK | MB_ICONINFORMATION,
                                );
                                focus_library(hwnd);
                            }
                        }
                        LRESULT(0)
                    }
                    ID_BTN_SEARCH => {
                        show_rss_search_dialog(hwnd);
                        LRESULT(0)
                    }
                    ID_BTN_EXPORT => {
                        let language = with_rss_state(hwnd, |s| {
                            with_state(s.parent, |ps| ps.settings.language).unwrap_or_default()
                        })
                        .unwrap_or_default();
                        if let Some(path) = open_export_opml_dialog(hwnd, language) {
                            match export_sources_to_opml_file(hwnd, &path) {
                                Ok(count) => {
                                    if count > 0 {
                                        announce_rss_status(&i18n::tr(language, "rss.exported"));
                                        let title = i18n::tr(language, "rss.window.title");
                                        let count_text = count.to_string();
                                        let path_text = path.display().to_string();
                                        let message = i18n::tr_f(
                                            language,
                                            "rss.export_success",
                                            &[("count", &count_text), ("path", &path_text)],
                                        );
                                        MessageBoxW(
                                            hwnd,
                                            PCWSTR(to_wide(&message).as_ptr()),
                                            PCWSTR(to_wide(&title).as_ptr()),
                                            MB_OK | MB_ICONINFORMATION,
                                        );
                                    }
                                }
                                Err(err) => {
                                    let title = i18n::tr(language, "rss.window.title");
                                    let message = format!(
                                        "{}: {}",
                                        i18n::tr(language, "rss.export_failed"),
                                        err
                                    );
                                    MessageBoxW(
                                        hwnd,
                                        PCWSTR(to_wide(&message).as_ptr()),
                                        PCWSTR(to_wide(&title).as_ptr()),
                                        MB_OK | MB_ICONINFORMATION,
                                    );
                                }
                            }
                        }
                        LRESULT(0)
                    }
                    1 => {
                        // IDOK (Enter key often triggers this generic command)
                        let focus = GetFocus();
                        let btn_add = GetDlgItem(hwnd, ID_BTN_ADD as i32);
                        let btn_search = GetDlgItem(hwnd, ID_BTN_SEARCH as i32);
                        let btn_community_add = GetDlgItem(hwnd, ID_BTN_COMMUNITY_ADD as i32);
                        let btn_community_browse = GetDlgItem(hwnd, ID_BTN_COMMUNITY_BROWSE as i32);
                        let btn_import = GetDlgItem(hwnd, ID_BTN_IMPORT as i32);
                        let btn_export = GetDlgItem(hwnd, ID_BTN_EXPORT as i32);
                        let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));

                        if focus == btn_add {
                            let already = with_rss_state(hwnd, |s| s.add_guard).unwrap_or(false);
                            if !already {
                                with_rss_state(hwnd, |s| s.add_guard = true);
                                if let Err(_e) =
                                    PostMessageW(hwnd, WM_SHOW_ADD_DIALOG, WPARAM(0), LPARAM(0))
                                {
                                    crate::log_debug(&format!("Error: {:?}", _e));
                                }
                            }
                            return LRESULT(0);
                        }

                        if focus == btn_community_add {
                            show_community_add_dialog(hwnd);
                            return LRESULT(0);
                        }

                        if focus == btn_community_browse {
                            show_community_sources_dialog(hwnd);
                            return LRESULT(0);
                        }

                        if focus == btn_import {
                            let language = with_rss_state(hwnd, |s| {
                                with_state(s.parent, |ps| ps.settings.language).unwrap_or_default()
                            })
                            .unwrap_or_default();
                            if let Some(path) = open_import_txt_dialog(hwnd, language) {
                                let count = import_sources_from_file(hwnd, &path);
                                if count > 0 {
                                    let title = i18n::tr(language, "rss.window.title");
                                    let count_text = count.to_string();
                                    let path_text = path.display().to_string();
                                    let message = i18n::tr_f(
                                        language,
                                        "rss.import_success",
                                        &[("count", &count_text), ("path", &path_text)],
                                    );
                                    MessageBoxW(
                                        hwnd,
                                        PCWSTR(to_wide(&message).as_ptr()),
                                        PCWSTR(to_wide(&title).as_ptr()),
                                        MB_OK | MB_ICONINFORMATION,
                                    );
                                    focus_library(hwnd);
                                }
                            }
                            return LRESULT(0);
                        }

                        if focus == btn_search {
                            show_rss_search_dialog(hwnd);
                            return LRESULT(0);
                        }

                        if focus == btn_export {
                            let language = with_rss_state(hwnd, |s| {
                                with_state(s.parent, |ps| ps.settings.language).unwrap_or_default()
                            })
                            .unwrap_or_default();
                            if let Some(path) = open_export_opml_dialog(hwnd, language) {
                                match export_sources_to_opml_file(hwnd, &path) {
                                    Ok(count) => {
                                        if count > 0 {
                                            announce_rss_status(&i18n::tr(
                                                language,
                                                "rss.exported",
                                            ));
                                            let title = i18n::tr(language, "rss.window.title");
                                            let count_text = count.to_string();
                                            let path_text = path.display().to_string();
                                            let message = i18n::tr_f(
                                                language,
                                                "rss.export_success",
                                                &[("count", &count_text), ("path", &path_text)],
                                            );
                                            MessageBoxW(
                                                hwnd,
                                                PCWSTR(to_wide(&message).as_ptr()),
                                                PCWSTR(to_wide(&title).as_ptr()),
                                                MB_OK | MB_ICONINFORMATION,
                                            );
                                        }
                                    }
                                    Err(err) => {
                                        let title = i18n::tr(language, "rss.window.title");
                                        let message = format!(
                                            "{}: {}",
                                            i18n::tr(language, "rss.export_failed"),
                                            err
                                        );
                                        MessageBoxW(
                                            hwnd,
                                            PCWSTR(to_wide(&message).as_ptr()),
                                            PCWSTR(to_wide(&title).as_ptr()),
                                            MB_OK | MB_ICONINFORMATION,
                                        );
                                    }
                                }
                            }
                            return LRESULT(0);
                        }

                        if focus == hwnd_tree {
                            let already = with_rss_state(hwnd, |s| s.enter_guard).unwrap_or(false);
                            if !already {
                                let shift_down = GetKeyState(VK_SHIFT.0 as i32) < 0;
                                with_rss_state(hwnd, |s| s.enter_guard = true);
                                if let Err(_e) =
                                    PostMessageW(hwnd, WM_CLEAR_ENTER_GUARD, WPARAM(0), LPARAM(0))
                                {
                                    crate::log_debug(&format!("Error: {:?}", _e));
                                }
                                handle_enter_action(hwnd, shift_down);
                            }
                            return LRESULT(0);
                        }

                        LRESULT(0)
                    }
                    id if (ID_CTX_MOVE_FOLDER_BASE..=ID_CTX_MOVE_FOLDER_END).contains(&id) => {
                        handle_move_source_to_folder(hwnd, id - ID_CTX_MOVE_FOLDER_BASE);
                        LRESULT(0)
                    }
                    ID_CTX_CREATE_FOLDER => {
                        show_create_rss_folder_dialog(hwnd);
                        LRESULT(0)
                    }
                    ID_CTX_EDIT => {
                        handle_edit_source(hwnd);
                        LRESULT(0)
                    }
                    ID_CTX_DELETE => {
                        handle_delete(hwnd);
                        LRESULT(0)
                    }
                    ID_CTX_UNDO_DELETE => {
                        undo_last_delete(hwnd);
                        LRESULT(0)
                    }
                    ID_CTX_RETRY => {
                        handle_retry_now(hwnd);
                        LRESULT(0)
                    }
                    ID_CTX_REORDER_UP => {
                        handle_reorder_action(hwnd, ReorderAction::Up);
                        LRESULT(0)
                    }
                    ID_CTX_REORDER_DOWN => {
                        handle_reorder_action(hwnd, ReorderAction::Down);
                        LRESULT(0)
                    }
                    ID_CTX_REORDER_TOP => {
                        handle_reorder_action(hwnd, ReorderAction::Top);
                        LRESULT(0)
                    }
                    ID_CTX_REORDER_BOTTOM => {
                        handle_reorder_action(hwnd, ReorderAction::Bottom);
                        LRESULT(0)
                    }
                    ID_CTX_REORDER_POSITION => {
                        handle_reorder_action(hwnd, ReorderAction::Position);
                        LRESULT(0)
                    }
                    ID_CTX_SORT_ASC => {
                        handle_sort_action(hwnd, crate::settings::SortOrder::TitleAsc);
                        LRESULT(0)
                    }
                    ID_CTX_SORT_DESC => {
                        handle_sort_action(hwnd, crate::settings::SortOrder::TitleDesc);
                        LRESULT(0)
                    }
                    ID_CTX_SORT_NEWEST => {
                        handle_sort_action(hwnd, crate::settings::SortOrder::DateNewest);
                        LRESULT(0)
                    }
                    ID_CTX_SORT_OLDEST => {
                        handle_sort_action(hwnd, crate::settings::SortOrder::DateOldest);
                        LRESULT(0)
                    }
                    ID_CTX_OPEN_BROWSER => {
                        handle_article_action(hwnd, ArticleAction::OpenInBrowser);
                        LRESULT(0)
                    }
                    ID_CTX_SHARE_FACEBOOK => {
                        handle_article_action(hwnd, ArticleAction::ShareFacebook);
                        LRESULT(0)
                    }
                    ID_CTX_SHARE_TWITTER => {
                        handle_article_action(hwnd, ArticleAction::ShareTwitter);
                        LRESULT(0)
                    }
                    ID_CTX_SHARE_WHATSAPP => {
                        handle_article_action(hwnd, ArticleAction::ShareWhatsApp);
                        LRESULT(0)
                    }
                    ID_CTX_SHARE_EMAIL => {
                        handle_article_action(hwnd, ArticleAction::ShareEmail);
                        LRESULT(0)
                    }
                    ID_CTX_ADD_TO_FAVORITES => {
                        handle_add_article_to_favorites(hwnd);
                        LRESULT(0)
                    }
                    ID_CTX_SELECT_ARTICLES => {
                        handle_select_articles(hwnd);
                        LRESULT(0)
                    }
                    ID_CTX_PROPERTIES => {
                        show_selected_properties(hwnd);
                        LRESULT(0)
                    }
                    ID_CTX_CHANGE_CITY => {
                        show_change_city_for_selected_category(hwnd);
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_RSS_CITY_CHANGED => {
                let pointer = lparam.0 as *mut String;
                if pointer.is_null() {
                    return LRESULT(0);
                }
                let city = *Box::from_raw(pointer);
                let category_hitem = windows::Win32::UI::Controls::HTREEITEM(wparam.0 as isize);
                let parent = with_rss_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
                with_state(parent, |state| {
                    state.settings.rss_local_city = city;
                    save_rss_settings(state);
                });
                let tree = with_rss_state(hwnd, |state| state.hwnd_tree).unwrap_or(HWND(0));
                clear_tree_children(hwnd, tree, category_hitem);
                with_rss_state(hwnd, |state| {
                    state.source_items.remove(&category_hitem.0);
                    state.pending_local_category = 0;
                });
                handle_expand(hwnd, category_hitem);
                if tree.0 != 0 {
                    SendMessageW(
                        tree,
                        TVM_EXPAND,
                        WPARAM(TVE_EXPAND.0 as usize),
                        LPARAM(category_hitem.0),
                    );
                    SendMessageW(
                        tree,
                        TVM_SELECTITEM,
                        WPARAM(TVGN_CARET as usize),
                        LPARAM(category_hitem.0),
                    );
                }
                LRESULT(0)
            }
            windows::Win32::UI::WindowsAndMessaging::WM_COPYDATA => {
                let cds = lparam.0 as *const COPYDATASTRUCT;
                if !cds.is_null() && (*cds).dwData == 0x52535331 {
                    let Some(payload) =
                        crate::copydata_utf16_payload(cds, "rss WM_COPYDATA add source")
                    else {
                        return LRESULT(0);
                    };
                    let mut lines = payload.lines();
                    let first = lines.next().unwrap_or("");
                    let second = lines.next();
                    let (mut title, url) = if let Some(url_line) = second {
                        (first.trim().to_string(), url_line.trim().to_string())
                    } else {
                        (String::new(), first.trim().to_string())
                    };
                    if url.is_empty() {
                        return LRESULT(0);
                    }
                    if title.trim().is_empty() {
                        title = url.clone();
                    }

                    let edit_idx = with_rss_state(hwnd, |s| s.pending_edit.take()).unwrap_or(None);
                    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                    if let Some(idx) = edit_idx {
                        with_state(parent, |state| {
                            if let Some(src) = state.settings.rss_sources.get_mut(idx) {
                                src.title = title.clone();
                                src.url = url.clone();
                                src.user_title = title.trim() != url.trim();
                                src.cache = rss::RssFeedCache::default();
                            }
                            save_rss_settings(state);
                        });
                        reload_tree(hwnd);
                    } else {
                        with_state(parent, |state| {
                            state.settings.rss_sources.push(RssSource {
                                title: title.clone(),
                                url: url.clone(),
                                kind: RssSourceType::Site,
                                folder_path: Vec::new(),
                                user_title: title.trim() != url.trim(),
                                unread: false,
                                cache: rss::RssFeedCache::default(),
                                last_seen_guid: None,
                                last_updated: None,
                                removed_item_keys: Vec::new(),
                                read_item_keys: Vec::new(),
                            });
                            save_rss_settings(state);
                        });
                        reload_tree(hwnd);

                        // Auto-expand the new item to trigger fetch (and title update)
                        let idx = with_rss_state(hwnd, |s| {
                            with_state(s.parent, |ps| ps.settings.rss_sources.len()).unwrap_or(0)
                        })
                        .unwrap_or(0);
                        if idx > 0 {
                            let last_idx = idx - 1;
                            let hitem = with_rss_state(hwnd, |s| {
                                s.node_data.iter().find_map(|(k, v)| {
                                    if let NodeData::Source(i) = v {
                                        if *i == last_idx { Some(*k) } else { None }
                                    } else {
                                        None
                                    }
                                })
                            })
                            .flatten();

                            if let Some(h) = hitem {
                                let hwnd_tree =
                                    with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                                SendMessageW(
                                    hwnd_tree,
                                    TVM_EXPAND,
                                    WPARAM(TVE_EXPAND.0 as usize),
                                    LPARAM(h),
                                );
                            }
                        }
                    }
                }
                LRESULT(0)
            }
            WM_NOTIFY => {
                let nmhdr = lparam.0 as *const NMHDR;
                if (*nmhdr).idFrom == ID_TREE {
                    match (*nmhdr).code {
                        TVN_ITEMEXPANDINGW => {
                            let pnmtv = lparam.0 as *const NMTREEVIEWW;
                            // action contains TVE_EXPAND (2) when expanding
                            // Use bitwise AND to check for the expand flag
                            let action_val = (*pnmtv).action.0;
                            if (action_val & TVE_EXPAND.0) != 0 {
                                let hitem = (*pnmtv).itemNew.hItem;
                                handle_expand(hwnd, hitem);
                            }
                            LRESULT(0)
                        }
                        TVN_SELCHANGEDW => {
                            if with_rss_state(hwnd, |s| s.suppress_tree_selection_events)
                                .unwrap_or(false)
                            {
                                return LRESULT(0);
                            }
                            let pnmtv = lparam.0 as *const NMTREEVIEWW;
                            let hitem = (*pnmtv).itemNew.hItem;
                            handle_selection_changed(hwnd, hitem);
                            refresh_article_preview_for_selection(hwnd);
                            LRESULT(0)
                        }
                        TVN_KEYDOWN => {
                            let ptvkd = lparam.0 as *const NMTVKEYDOWN;
                            let ctrl_down = GetKeyState(VK_CONTROL.0 as i32) < 0;
                            let shift_down = GetKeyState(VK_SHIFT.0 as i32) < 0;
                            if ctrl_down && shift_down && (*ptvkd).wVKey == VK_UP.0 {
                                ignore_bool(handle_move_shortcut(hwnd, true));
                                return LRESULT(1);
                            }
                            if ctrl_down && shift_down && (*ptvkd).wVKey == VK_DOWN.0 {
                                ignore_bool(handle_move_shortcut(hwnd, false));
                                return LRESULT(1);
                            }
                            if (*ptvkd).wVKey
                                == windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN.0
                            {
                                if GetKeyState(VK_MENU.0 as i32) < 0 {
                                    show_selected_properties(hwnd);
                                    return LRESULT(1);
                                }
                                let shift_down = GetKeyState(VK_SHIFT.0 as i32) < 0;
                                with_rss_state(hwnd, |s| s.enter_guard = true);
                                if let Err(_e) =
                                    PostMessageW(hwnd, WM_CLEAR_ENTER_GUARD, WPARAM(0), LPARAM(0))
                                {
                                    crate::log_debug(&format!("Error: {:?}", _e));
                                }
                                handle_enter_action(hwnd, shift_down);
                                LRESULT(1)
                            } else if (*ptvkd).wVKey
                                == windows::Win32::UI::Input::KeyboardAndMouse::VK_DELETE.0
                            {
                                handle_delete(hwnd);
                                LRESULT(1)
                            } else if (*ptvkd).wVKey == 'Z' as u16
                                && GetKeyState(VK_CONTROL.0 as i32) < 0
                            {
                                undo_last_delete(hwnd);
                                LRESULT(1)
                            } else if (*ptvkd).wVKey == VK_F10.0
                                && GetKeyState(VK_SHIFT.0 as i32) < 0
                            {
                                let hwnd_tree =
                                    with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                                if hwnd_tree.0 != 0
                                    && let Err(_e) = PostMessageW(
                                        hwnd,
                                        WM_CONTEXTMENU,
                                        WPARAM(hwnd_tree.0 as usize),
                                        LPARAM(-1),
                                    )
                                {}
                                LRESULT(1)
                            } else if (*ptvkd).wVKey == VK_APPS.0 {
                                let hwnd_tree =
                                    with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                                if hwnd_tree.0 != 0
                                    && let Err(_e) = PostMessageW(
                                        hwnd,
                                        WM_CONTEXTMENU,
                                        WPARAM(hwnd_tree.0 as usize),
                                        LPARAM(-1),
                                    )
                                {}
                                LRESULT(1)
                            } else if (*ptvkd).wVKey == VK_ESCAPE.0 {
                                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                                LRESULT(1)
                            } else {
                                LRESULT(0)
                            }
                        }
                        NM_RCLICK => {
                            let mut pt = POINT::default();
                            crate::log_if_err!(GetCursorPos(&mut pt));
                            show_rss_context_menu(hwnd, pt.x, pt.y, true);
                            LRESULT(1)
                        }
                        _ => LRESULT(0),
                    }
                } else {
                    LRESULT(0)
                }
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let key = wparam.0 as u32;
                if key == u32::from(VK_ESCAPE.0) {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                if hwnd_tree.0 == 0 {
                    return LRESULT(0);
                }
                if GetFocus() != hwnd_tree {
                    return LRESULT(0);
                }
                if key == u32::from(VK_APPS.0)
                    || (key == u32::from(VK_F10.0) && GetKeyState(VK_SHIFT.0 as i32) < 0)
                {
                    if let Err(_e) = PostMessageW(
                        hwnd,
                        WM_CONTEXTMENU,
                        WPARAM(hwnd_tree.0 as usize),
                        LPARAM(-1),
                    ) {}
                    return LRESULT(0);
                }
                if GetKeyState(VK_CONTROL.0 as i32) < 0
                    && GetKeyState(VK_SHIFT.0 as i32) < 0
                    && key == u32::from(VK_UP.0)
                {
                    ignore_bool(handle_move_shortcut(hwnd, true));
                    return LRESULT(0);
                }
                if GetKeyState(VK_CONTROL.0 as i32) < 0
                    && GetKeyState(VK_SHIFT.0 as i32) < 0
                    && key == u32::from(VK_DOWN.0)
                {
                    ignore_bool(handle_move_shortcut(hwnd, false));
                    return LRESULT(0);
                }
                if key == 'Z' as u32 && GetKeyState(VK_CONTROL.0 as i32) < 0 {
                    undo_last_delete(hwnd);
                    return LRESULT(0);
                }
                if key == 'C' as u32 && GetKeyState(VK_CONTROL.0 as i32) < 0 {
                    ignore_bool(handle_rss_quick_copy(hwnd));
                    return LRESULT(0);
                }
                if key == u32::from(VK_TAB.0)
                    && GetKeyState(VK_SHIFT.0 as i32) >= 0
                    && rss_article_preview_enabled(hwnd)
                    && selected_article_item(hwnd).is_some()
                {
                    let hwnd_preview = with_rss_state(hwnd, |s| s.hwnd_preview).unwrap_or(HWND(0));
                    if hwnd_preview.0 != 0 {
                        SetFocus(hwnd_preview);
                        SendMessageW(hwnd_preview, EM_SETSEL, WPARAM(0), LPARAM(0));
                        SendMessageW(
                            hwnd_preview,
                            WM_VSCROLL,
                            WPARAM(SB_TOP.0 as usize),
                            LPARAM(0),
                        );
                        SendMessageW(hwnd_preview, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
                        return LRESULT(0);
                    }
                }
                if key == u32::from(VK_RETURN.0) && GetKeyState(VK_MENU.0 as i32) < 0 {
                    show_selected_properties(hwnd);
                    return LRESULT(0);
                }
                if key == u32::from(VK_ESCAPE.0) {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CONTEXTMENU => {
                let mut x = (lparam.0 & 0xffff) as i32;
                let mut y = ((lparam.0 >> 16) & 0xffff) as i32;
                let use_hit_test = !(x == -1 && y == -1);
                if x == -1 && y == -1 {
                    let mut pt = POINT::default();
                    crate::log_if_err!(GetCursorPos(&mut pt));
                    x = pt.x;
                    y = pt.y;
                }
                show_rss_context_menu(hwnd, x, y, use_hit_test);
                LRESULT(0)
            }
            WM_RSS_FETCH_COMPLETE => {
                let ptr = lparam.0 as *mut FetchResult;
                let res = *Box::from_raw(ptr);
                process_fetch_result(hwnd, res);
                LRESULT(0)
            }
            WM_RSS_PREVIEW_COMPLETE => {
                let ptr = lparam.0 as *mut PreviewResult;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let res = *Box::from_raw(ptr);
                with_rss_state(hwnd, |s| {
                    if res.request_seq != s.preview_request_seq {
                        return;
                    }
                    set_preview_text(s.hwnd_preview, &res.text);
                });
                LRESULT(0)
            }
            WM_RSS_BACKGROUND_CHECK_COMPLETE => {
                let ptr = lparam.0 as *mut BackgroundCheckResult;
                let res = *Box::from_raw(ptr);
                process_background_check_result(hwnd, res);
                LRESULT(0)
            }
            WM_RSS_MARK_ITEM_READ_UI => {
                let ptr = lparam.0 as *mut MarkItemReadUiMessage;
                let msg_data = *Box::from_raw(ptr);
                let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                if hwnd_tree.0 == 0 {
                    return LRESULT(0);
                }
                let hitem = windows::Win32::UI::Controls::HTREEITEM(msg_data.hitem);
                let item = with_rss_state(hwnd, |s| match s.node_data.get(&hitem.0) {
                    Some(NodeData::Item(item)) => Some(item.clone()),
                    _ => None,
                })
                .flatten();
                if let Some(item) = item
                    && rss_item_key(&item) == msg_data.item_key
                {
                    let (
                        language,
                        announce_unread,
                        unread_label_position,
                        rss_date_mode,
                        rss_time_mode,
                    ) = with_rss_state(hwnd, |s| {
                        with_state(s.parent, |ps| {
                            (
                                ps.settings.language,
                                ps.settings.announce_unread_rss_podcast_items,
                                ps.settings.rss_podcast_unread_label_position,
                                ps.settings.rss_articles_date_display,
                                ps.settings.rss_articles_time_display,
                            )
                        })
                        .unwrap_or((
                            crate::settings::Language::English,
                            true,
                            crate::settings::RssPodcastUnreadLabelPosition::Before,
                            ListDateDisplayMode::Always,
                            ListTimeDisplayMode::Always,
                        ))
                    })
                    .unwrap_or((
                        crate::settings::Language::English,
                        true,
                        crate::settings::RssPodcastUnreadLabelPosition::Before,
                        ListDateDisplayMode::Always,
                        ListTimeDisplayMode::Always,
                    ));
                    let day_counts = with_rss_state(hwnd, |s| {
                        let parent_hitem = windows::Win32::UI::Controls::HTREEITEM(
                            crate::send_message_w_safe(
                                s.hwnd_tree,
                                TVM_GETNEXTITEM,
                                WPARAM(TVGN_PARENT as usize),
                                LPARAM(hitem.0),
                            )
                            .0,
                        );
                        if let Some(NodeData::Item(parent_item)) = s.node_data.get(&parent_hitem.0)
                        {
                            return build_day_counts(&parent_item.related_items);
                        }
                        source_ancestor_hitem(s, hitem)
                            .and_then(|source_hitem| s.source_items.get(&source_hitem.0))
                            .map(|state| build_day_counts(&state.items))
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                    let same_day = has_multiple_items_same_day(item.pub_date, &day_counts);
                    let title_ctx = RssItemTitleContext {
                        language,
                        announce_unread,
                        unread_label_position,
                        date_mode: rss_date_mode,
                        time_mode: rss_time_mode,
                    };
                    let updated = rss_item_tree_display_title(&item, false, same_day, title_ctx);
                    let text = to_wide(&updated);
                    let mut tv_item = TVITEMW {
                        mask: TVIF_TEXT,
                        hItem: hitem,
                        pszText: windows::core::PWSTR(text.as_ptr() as *mut _),
                        cchTextMax: text.len() as i32,
                        ..Default::default()
                    };
                    SendMessageW(
                        hwnd_tree,
                        TVM_SETITEMW,
                        WPARAM(0),
                        LPARAM(&mut tv_item as *mut _ as isize),
                    );
                }
                LRESULT(0)
            }
            WM_RSS_SELECT_SOURCE_DELAYED => {
                select_source_by_index(hwnd, wparam.0);
                LRESULT(0)
            }
            WM_CLEAR_ENTER_GUARD => {
                with_rss_state(hwnd, |s| s.enter_guard = false);
                LRESULT(0)
            }
            WM_CLEAR_ADD_GUARD => {
                with_rss_state(hwnd, |s| s.add_guard = false);
                LRESULT(0)
            }

            WM_TIMER => {
                // Clear short-lived guards (prevents double-open on Enter on the Add button)
                if wparam.0 == ADD_GUARD_TIMER_ID {
                    with_rss_state(hwnd, |s| s.add_guard = false);
                    if let Err(e) = KillTimer(hwnd, ADD_GUARD_TIMER_ID) {
                        crate::log_debug(&format!("Failed to kill ADD_GUARD_TIMER: {}", e));
                    }
                    return LRESULT(0);
                }
                if wparam.0 == RSS_LANGUAGE_FOCUS_TIMER_ID {
                    if let Err(e) = KillTimer(hwnd, RSS_LANGUAGE_FOCUS_TIMER_ID) {
                        crate::log_debug(&format!(
                            "Failed to kill RSS language focus timer: {}",
                            e
                        ));
                    }
                    let hwnd_tree =
                        with_rss_state(hwnd, |state| state.hwnd_tree).unwrap_or(HWND(0));
                    if hwnd_tree.0 != 0 {
                        crate::set_focus_safe(hwnd_tree);
                    }
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }

            WM_RSS_IMPORT_COMPLETE => {
                let ptr = lparam.0 as *mut ImportResult;
                let res = Box::from_raw(ptr);
                crate::log_debug("WM_RSS_IMPORT_COMPLETE received");

                let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                let language =
                    with_state(parent, |state| state.settings.language).unwrap_or_default();
                let rss_title = i18n::tr(language, "rss.temp_title");
                let hwnd_edit = editor_manager::get_or_create_rss_document(parent, &rss_title);

                if let Some(h_edit) = hwnd_edit {
                    // Bring the main window to the front *before* moving focus.
                    // Doing it the other way around is frequently ignored by Windows,
                    // especially when focus changes originate from posted messages.
                    let main_window = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                    if main_window.0 != 0 {
                        SetForegroundWindow(main_window);
                    }
                    // Ensure large articles are not truncated by the default edit limit.
                    SendMessageW(h_edit, EM_LIMITTEXT, WPARAM(0x7FFF_FFFEusize), LPARAM(0));

                    // Replace the entire editor contents, then move caret to the start.
                    // We normalize text to avoid embedded NULs (which truncate Win32 edit text)
                    // and to reduce multiple blank lines.
                    let cleaned = normalize_article_text(&res.text);
                    let wide = to_wide(&cleaned);
                    SendMessageW(h_edit, EM_SETSEL, WPARAM(0), LPARAM(-1));
                    SendMessageW(
                        h_edit,
                        EM_REPLACESEL,
                        WPARAM(1),
                        LPARAM(wide.as_ptr() as isize),
                    );
                    SendMessageW(h_edit, EM_SETSEL, WPARAM(0), LPARAM(0));

                    SetFocus(h_edit);
                    if parent.0 != 0 {
                        editor_manager::mark_current_document_from_rss(parent, true);
                    }
                }
                LRESULT(0)
            }
            WM_SHOW_ADD_DIALOG => {
                // If the add dialog is already open, just bring it to the front.
                let main_hwnd = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                let existing = with_state(main_hwnd, |s| s.rss_add_dialog).unwrap_or(HWND(0));
                if existing.0 != 0 {
                    SetForegroundWindow(existing);
                } else {
                    show_add_dialog(hwnd);
                }
                if let Err(_e) = PostMessageW(hwnd, WM_CLEAR_ADD_GUARD, WPARAM(0), LPARAM(0)) {
                    crate::log_debug(&format!("Error: {:?}", _e));
                }
                LRESULT(0)
            }
            WM_RSS_SHOW_CONTEXT => {
                show_context_menu_from_keyboard(hwnd);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                if hwnd_tree.0 != 0 {
                    SetFocus(hwnd_tree);
                    let suppress_restore =
                        with_rss_state(hwnd, |s| mem::take(&mut s.suppress_focus_restore_once))
                            .unwrap_or(false);
                    if suppress_restore {
                        return LRESULT(0);
                    }
                    let last = with_rss_state(hwnd, |s| s.last_selected).unwrap_or(0);
                    if last != 0 {
                        SendMessageW(
                            hwnd_tree,
                            TVM_SELECTITEM,
                            WPARAM(TVGN_CARET as usize),
                            LPARAM(last),
                        );
                        SendMessageW(hwnd_tree, TVM_ENSUREVISIBLE, WPARAM(0), LPARAM(last));
                    }
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
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
                let cs = lparam.0 as *const CREATESTRUCTW;
                let init_ptr = (*cs).lpCreateParams as *mut ReorderDialogInit;
                if init_ptr.is_null() {
                    return LRESULT(0);
                }
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, init_ptr as isize);
                let init = &*init_ptr;

                let language = with_rss_state(init.parent, |s| {
                    with_state(s.parent, |ps| ps.settings.language).unwrap_or_default()
                })
                .unwrap_or_default();
                let position_template = i18n::tr(language, "rss.reorder.position_of");
                let position_text = position_template
                    .replace("{x}", &(init.source_index + 1).to_string())
                    .replace("{n}", &init.total.to_string());
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
                    330,
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
                    36,
                    330,
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
                    120,
                    24,
                    hwnd,
                    HMENU(REORDER_EDIT_ID as isize),
                    hinstance,
                    None,
                );
                SendMessageW(edit, EM_LIMITTEXT, WPARAM(6), LPARAM(0));
                let ok = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&ok_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    170,
                    96,
                    80,
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
                    260,
                    96,
                    80,
                    24,
                    hwnd,
                    HMENU(REORDER_CANCEL_ID as isize),
                    hinstance,
                    None,
                );
                let proc_ptr = reorder_control_subclass_proc as *const () as usize;
                for control in [edit, ok, cancel] {
                    let prev = SetWindowLongPtrW(control, GWLP_WNDPROC, proc_ptr as isize);
                    SetWindowLongPtrW(control, GWLP_USERDATA, prev);
                }
                SetFocus(edit);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                match id {
                    1 => {
                        SendMessageW(hwnd, WM_COMMAND, WPARAM(REORDER_OK_ID), LPARAM(0));
                        LRESULT(0)
                    }
                    REORDER_OK_ID => {
                        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ReorderDialogInit;
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
                        let language = with_rss_state(init.parent, |s| {
                            with_state(s.parent, |ps| ps.settings.language).unwrap_or_default()
                        })
                        .unwrap_or_default();
                        let pos = match text.trim().parse::<usize>() {
                            Ok(v) if v > 0 => v,
                            _ => {
                                let message = i18n::tr(language, "rss.reorder.invalid_position");
                                announce_rss_status(&message);
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
                            announce_rss_status(&message);
                        }
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    REORDER_CANCEL_ID | 2 => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
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
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ReorderDialogInit;
                if !ptr.is_null() {
                    let init = Box::from_raw(ptr);
                    with_rss_state(init.parent, |s| s.reorder_dialog = HWND(0));
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn with_rss_state<F, R>(hwnd: HWND, f: F) -> Option<R>
where
    F: FnOnce(&mut RssWindowState) -> R,
{
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut RssWindowState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

unsafe extern "system" fn rss_tree_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "rss_tree_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || rss_tree_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn rss_tree_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == windows::Win32::UI::WindowsAndMessaging::WM_CHAR
            && wparam.0 as u32 == 3
            && GetKeyState(VK_CONTROL.0 as i32) < 0
        {
            return LRESULT(0);
        }
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let key = wparam.0 as u32;
            if GetKeyState(VK_CONTROL.0 as i32) < 0
                && GetKeyState(VK_SHIFT.0 as i32) < 0
                && (key == u32::from(VK_UP.0) || key == u32::from(VK_DOWN.0))
            {
                let parent = GetParent(hwnd);
                if parent.0 != 0 {
                    ignore_bool(handle_move_shortcut(parent, key == u32::from(VK_UP.0)));
                    return LRESULT(0);
                }
            }
            if key == 'C' as u32 && GetKeyState(VK_CONTROL.0 as i32) < 0 {
                let parent = GetParent(hwnd);
                if parent.0 != 0 {
                    ignore_bool(handle_rss_quick_copy(parent));
                    return LRESULT(0);
                }
            }
            if key == u32::from(VK_TAB.0) {
                let parent = GetParent(hwnd);
                if parent.0 != 0 {
                    let target = if GetKeyState(VK_SHIFT.0 as i32) < 0 {
                        GetDlgItem(parent, ID_BTN_CLOSE as i32)
                    } else if rss_article_preview_enabled(parent)
                        && selected_article_item(parent).is_some()
                    {
                        with_rss_state(parent, |state| state.hwnd_preview).unwrap_or(HWND(0))
                    } else {
                        with_rss_state(parent, |state| state.hwnd_language_combo).unwrap_or(HWND(0))
                    };
                    if target.0 != 0 {
                        SetFocus(target);
                        return LRESULT(0);
                    }
                }
            }
            if key == u32::from(VK_RETURN.0) && GetKeyState(VK_MENU.0 as i32) < 0 {
                let parent = GetParent(hwnd);
                if parent.0 != 0 {
                    show_selected_properties(parent);
                    return LRESULT(0);
                }
            }
            if key == u32::from(VK_APPS.0)
                || (key == u32::from(VK_F10.0) && GetKeyState(VK_SHIFT.0 as i32) < 0)
            {
                let parent = GetParent(hwnd);
                if parent.0 != 0 {
                    if let Err(_e) =
                        PostMessageW(parent, WM_CONTEXTMENU, WPARAM(hwnd.0 as usize), LPARAM(-1))
                    {
                        crate::log_debug(&format!("Error: {:?}", _e));
                    }
                    return LRESULT(0);
                }
            }
        }

        let parent = GetParent(hwnd);
        let prev_proc = if parent.0 != 0 {
            with_rss_state(parent, |s| s.tree_proc).unwrap_or(None)
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

unsafe extern "system" fn rss_preview_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "rss_preview_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || rss_preview_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn rss_preview_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let key = wparam.0 as u32;
            if key == u32::from(VK_TAB.0) {
                let parent = GetParent(hwnd);
                if parent.0 != 0 {
                    let target = if GetKeyState(VK_SHIFT.0 as i32) < 0 {
                        with_rss_state(parent, |s| s.hwnd_tree).unwrap_or(HWND(0))
                    } else {
                        with_rss_state(parent, |s| s.hwnd_language_combo).unwrap_or(HWND(0))
                    };
                    if target.0 != 0 {
                        SetFocus(target);
                        return LRESULT(0);
                    }
                }
            }
        }

        let parent = GetParent(hwnd);
        let prev_proc = if parent.0 != 0 {
            with_rss_state(parent, |s| s.preview_proc).unwrap_or(None)
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
                | WS_VSCROLL
                | WINDOW_STYLE(
                    windows::Win32::UI::Controls::TVS_HASLINES
                        | windows::Win32::UI::Controls::TVS_HASBUTTONS
                        | windows::Win32::UI::Controls::TVS_LINESATROOT
                        | windows::Win32::UI::Controls::TVS_SHOWSELALWAYS,
                ),
            10,
            10,
            780,
            525,
            hwnd,
            HMENU(ID_TREE as isize),
            hinstance,
            None,
        );
        if hwnd_tree.0 != 0 {
            let proc_ptr = rss_tree_wndproc as *const () as usize;
            let old = SetWindowLongPtrW(hwnd_tree, GWLP_WNDPROC, proc_ptr as isize);
            with_rss_state(hwnd, |s| {
                s.tree_proc = mem::transmute::<isize, WNDPROC>(old)
            });
        }

        let hwnd_preview = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            MSFTEDIT_CLASS,
            PCWSTR::null(),
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | WS_VSCROLL
                | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL) as u32),
            10,
            365,
            780,
            170,
            hwnd,
            HMENU(ID_EDIT_ARTICLE_PREVIEW as isize),
            hinstance,
            None,
        );
        if hwnd_preview.0 != 0 {
            let proc_ptr = rss_preview_wndproc as *const () as usize;
            let old = SetWindowLongPtrW(hwnd_preview, GWLP_WNDPROC, proc_ptr as isize);
            with_rss_state(hwnd, |s| {
                s.preview_proc = mem::transmute::<isize, WNDPROC>(old)
            });
            SendMessageW(
                hwnd_preview,
                EM_LIMITTEXT,
                WPARAM(0x7FFF_FFFEusize),
                LPARAM(0),
            );
            SendMessageW(hwnd_preview, EM_SETREADONLY, WPARAM(1), LPARAM(0));
        }

        let (language, selected_news_language) = with_rss_state(hwnd, |s| {
            with_state(s.parent, |ps| {
                (
                    ps.settings.language,
                    active_news_language_code_from_state(ps),
                )
            })
            .unwrap_or((crate::settings::Language::default(), "it".to_string()))
        })
        .unwrap_or((crate::settings::Language::default(), "it".to_string()));

        let hwnd_language_label = CreateWindowExW(
            Default::default(),
            WC_STATIC,
            PCWSTR(to_wide(&i18n::tr(language, "rss.news_language")).as_ptr()),
            WS_CHILD | WS_VISIBLE,
            10,
            550,
            150,
            25,
            hwnd,
            None,
            hinstance,
            None,
        );
        let hwnd_language_combo = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            WC_COMBOBOXW,
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(0x0003),
            180,
            545,
            330,
            220,
            hwnd,
            HMENU(ID_COMBO_NEWS_LANGUAGE as isize),
            hinstance,
            None,
        );
        for code in NEWS_LANGUAGE_CODES {
            let label = news_language_label(language, code);
            SendMessageW(
                hwnd_language_combo,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&label).as_ptr() as isize),
            );
        }
        let selected_index = NEWS_LANGUAGE_CODES
            .iter()
            .position(|code| *code == selected_news_language)
            .unwrap_or(0);
        SendMessageW(
            hwnd_language_combo,
            CB_SETCURSEL,
            WPARAM(selected_index),
            LPARAM(0),
        );

        let hwnd_add = CreateWindowExW(
            Default::default(),
            WC_BUTTON,
            PCWSTR(to_wide(&i18n::tr(language, "rss.tree.add_source")).as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            10,
            585,
            160,
            32,
            hwnd,
            HMENU(ID_BTN_ADD as isize),
            hinstance,
            None,
        );
        let hwnd_community_add = CreateWindowExW(
            Default::default(),
            WC_BUTTON,
            PCWSTR(to_wide(&i18n::tr(language, "rss.community.add_button")).as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            180,
            585,
            300,
            32,
            hwnd,
            HMENU(ID_BTN_COMMUNITY_ADD as isize),
            hinstance,
            None,
        );
        let hwnd_community_browse = CreateWindowExW(
            Default::default(),
            WC_BUTTON,
            PCWSTR(to_wide(&i18n::tr(language, "rss.community.browse_button")).as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            490,
            585,
            300,
            32,
            hwnd,
            HMENU(ID_BTN_COMMUNITY_BROWSE as isize),
            hinstance,
            None,
        );

        let hwnd_import = CreateWindowExW(
            Default::default(),
            WC_BUTTON,
            PCWSTR(to_wide(&i18n::tr(language, "rss.tree.import_txt")).as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            10,
            627,
            180,
            32,
            hwnd,
            HMENU(ID_BTN_IMPORT as isize),
            hinstance,
            None,
        );
        let export_label = i18n::tr(language, "rss.tree.export_opml");
        let export_label = if export_label == "rss.tree.export_opml" {
            "Export OPML...".to_string()
        } else {
            export_label
        };
        let hwnd_export = CreateWindowExW(
            Default::default(),
            WC_BUTTON,
            PCWSTR(to_wide(&export_label).as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            200,
            627,
            180,
            32,
            hwnd,
            HMENU(ID_BTN_EXPORT as isize),
            hinstance,
            None,
        );
        let hwnd_search = CreateWindowExW(
            Default::default(),
            WC_BUTTON,
            PCWSTR(to_wide(&i18n::tr(language, "rss.search.button")).as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            390,
            627,
            180,
            32,
            hwnd,
            HMENU(ID_BTN_SEARCH as isize),
            hinstance,
            None,
        );
        let hwnd_close = CreateWindowExW(
            Default::default(),
            WC_BUTTON,
            PCWSTR(to_wide(&i18n::tr(language, "rss.tree.close")).as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            580,
            627,
            180,
            32,
            hwnd,
            HMENU(ID_BTN_CLOSE as isize),
            hinstance,
            None,
        );

        for control in [
            hwnd_language_combo,
            hwnd_add,
            hwnd_community_add,
            hwnd_community_browse,
            hwnd_import,
            hwnd_export,
            hwnd_search,
            hwnd_close,
        ] {
            subclass_auxiliary_dialog_control(control);
        }

        with_rss_state(hwnd, |s| {
            s.hwnd_tree = hwnd_tree;
            s.hwnd_preview = hwnd_preview;
            s.hwnd_language_combo = hwnd_language_combo;
            s.hwnd_import = hwnd_import;
            s.hwnd_export = hwnd_export;
        });

        let hfont = with_rss_state(hwnd, |s| {
            with_state(s.parent, |ps| ps.hfont).unwrap_or(HFONT(0))
        })
        .unwrap_or(HFONT(0));
        if hfont.0 != 0 {
            for control in [
                hwnd_tree,
                hwnd_preview,
                hwnd_language_label,
                hwnd_language_combo,
                hwnd_add,
                hwnd_community_add,
                hwnd_community_browse,
                hwnd_import,
                hwnd_export,
                hwnd_search,
                hwnd_close,
            ] {
                SendMessageW(control, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
            }
        }

        update_preview_layout(hwnd);
        SetFocus(hwnd_tree);
    }
}

fn ensure_folder_tree_path(
    hwnd: HWND,
    hwnd_tree: HWND,
    folder_handles: &mut HashMap<Vec<String>, windows::Win32::UI::Controls::HTREEITEM>,
    path: &[String],
) -> windows::Win32::UI::Controls::HTREEITEM {
    let mut parent_hitem = TVI_ROOT;
    let mut current_path = Vec::new();
    for part in path {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        current_path.push(part.to_string());
        if let Some(existing) = folder_handles.get(&current_path) {
            parent_hitem = *existing;
            continue;
        }
        let text = to_wide(part);
        let mut insert = TVINSERTSTRUCTW {
            hParent: parent_hitem,
            hInsertAfter: TVI_LAST,
            Anonymous: TVINSERTSTRUCTW_0 {
                item: TVITEMW {
                    mask: TVIF_TEXT | TVIF_PARAM,
                    pszText: windows::core::PWSTR(text.as_ptr() as *mut _),
                    lParam: LPARAM(-2),
                    ..Default::default()
                },
            },
        };
        let folder_hitem = windows::Win32::UI::Controls::HTREEITEM(
            crate::send_message_w_safe(
                hwnd_tree,
                TVM_INSERTITEMW,
                WPARAM(0),
                LPARAM(&mut insert as *mut _ as isize),
            )
            .0,
        );
        if folder_hitem.0 == 0 {
            continue;
        }
        let stored_path = current_path.clone();
        with_rss_state(hwnd, |state| {
            state
                .node_data
                .insert(folder_hitem.0, NodeData::Folder(stored_path));
        });
        folder_handles.insert(current_path.clone(), folder_hitem);
        parent_hitem = folder_hitem;
    }
    parent_hitem
}

fn reload_tree(hwnd: HWND) {
    let (hwnd_tree, sources, folders, language, announce_unread, unread_label_position) =
        match with_rss_state(hwnd, |s| {
            (
                s.hwnd_tree,
                { with_state(s.parent, |ps| ps.settings.rss_sources.clone()) },
                { with_state(s.parent, |ps| rss_folder_paths_for_settings(&ps.settings)) },
                { with_state(s.parent, |ps| ps.settings.language) },
                { with_state(s.parent, |ps| ps.settings.announce_unread_rss_podcast_items) },
                { with_state(s.parent, |ps| ps.settings.rss_podcast_unread_label_position) },
            )
        }) {
            Some((
                t,
                Some(src),
                Some(folders),
                Some(language),
                Some(announce_unread),
                Some(unread_label_position),
            )) => (
                t,
                src,
                folders,
                language,
                announce_unread,
                unread_label_position,
            ),
            _ => return,
        };

    crate::send_message_w_safe(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(TVI_ROOT.0));

    with_rss_state(hwnd, |s| {
        s.node_data.clear();
        s.source_items.clear();
        s.pending_fetches.clear();
    });

    let news_code =
        active_news_language_code(with_rss_state(hwnd, |state| state.parent).unwrap_or(HWND(0)));
    let google_title = to_wide(google_news_locale(&news_code).root_title);
    let mut google_insert = TVINSERTSTRUCTW {
        hParent: TVI_ROOT,
        hInsertAfter: TVI_FIRST,
        Anonymous: TVINSERTSTRUCTW_0 {
            item: TVITEMW {
                mask: TVIF_TEXT | TVIF_PARAM | windows::Win32::UI::Controls::TVIF_CHILDREN,
                pszText: windows::core::PWSTR(google_title.as_ptr() as *mut _),
                cChildren: TVITEMEXW_CHILDREN(1),
                lParam: LPARAM(-1),
                ..Default::default()
            },
        },
    };
    let google_hitem = crate::send_message_w_safe(
        hwnd_tree,
        TVM_INSERTITEMW,
        WPARAM(0),
        LPARAM(&mut google_insert as *mut _ as isize),
    );
    with_rss_state(hwnd, |state| {
        state
            .node_data
            .insert(google_hitem.0, NodeData::GoogleNewsRoot);
    });

    let mut folder_handles: HashMap<Vec<String>, windows::Win32::UI::Controls::HTREEITEM> =
        HashMap::new();
    for folder_path in &folders {
        ensure_folder_tree_path(hwnd, hwnd_tree, &mut folder_handles, folder_path);
    }
    for (i, source) in sources.into_iter().enumerate() {
        let parent_hitem =
            ensure_folder_tree_path(hwnd, hwnd_tree, &mut folder_handles, &source.folder_path);

        let title = to_wide(&rss_source_display_title(
            &source,
            language,
            announce_unread,
            unread_label_position,
        ));
        let mut tvis = TVINSERTSTRUCTW {
            hParent: parent_hitem,
            hInsertAfter: TVI_LAST,
            Anonymous: TVINSERTSTRUCTW_0 {
                item: TVITEMW {
                    mask: TVIF_TEXT | TVIF_PARAM | windows::Win32::UI::Controls::TVIF_CHILDREN,
                    pszText: windows::core::PWSTR(title.as_ptr() as *mut _),
                    cChildren: TVITEMEXW_CHILDREN(1),
                    lParam: LPARAM(i as isize),
                    ..Default::default()
                },
            },
        };
        let hitem = crate::send_message_w_safe(
            hwnd_tree,
            TVM_INSERTITEMW,
            WPARAM(0),
            LPARAM(&mut tvis as *mut _ as isize),
        );

        with_rss_state(hwnd, |s| {
            s.node_data.insert(hitem.0, NodeData::Source(i));
        });
    }
}

fn collect_tree_subtree_handles(
    hwnd_tree: HWND,
    item: windows::Win32::UI::Controls::HTREEITEM,
    handles: &mut Vec<isize>,
) {
    handles.push(item.0);
    let mut child = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CHILD as usize),
            LPARAM(item.0),
        )
        .0,
    );
    while child.0 != 0 {
        collect_tree_subtree_handles(hwnd_tree, child, handles);
        child = windows::Win32::UI::Controls::HTREEITEM(
            crate::send_message_w_safe(
                hwnd_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_NEXT as usize),
                LPARAM(child.0),
            )
            .0,
        );
    }
}

fn clear_tree_children(
    hwnd: HWND,
    hwnd_tree: HWND,
    parent_item: windows::Win32::UI::Controls::HTREEITEM,
) {
    let mut removed_handles = Vec::new();
    loop {
        let child = windows::Win32::UI::Controls::HTREEITEM(
            crate::send_message_w_safe(
                hwnd_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_CHILD as usize),
                LPARAM(parent_item.0),
            )
            .0,
        );
        if child.0 == 0 {
            break;
        }
        collect_tree_subtree_handles(hwnd_tree, child, &mut removed_handles);
        crate::send_message_w_safe(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(child.0));
    }
    with_rss_state(hwnd, |state| {
        for handle in removed_handles {
            state.node_data.remove(&handle);
            state.source_items.remove(&handle);
        }
    });
}

fn populate_google_news_categories(hwnd: HWND, root_item: windows::Win32::UI::Controls::HTREEITEM) {
    let hwnd_tree = with_rss_state(hwnd, |state| state.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return;
    }
    let code =
        active_news_language_code(with_rss_state(hwnd, |state| state.parent).unwrap_or(HWND(0)));
    let categories = google_news_categories(&code);
    clear_tree_children(hwnd, hwnd_tree, root_item);
    with_rss_state(hwnd, |state| {
        state.source_items.remove(&root_item.0);
    });

    for category in categories {
        let title = to_wide(&category.title);
        let mut insert = TVINSERTSTRUCTW {
            hParent: root_item,
            hInsertAfter: TVI_LAST,
            Anonymous: TVINSERTSTRUCTW_0 {
                item: TVITEMW {
                    mask: TVIF_TEXT | TVIF_PARAM | windows::Win32::UI::Controls::TVIF_CHILDREN,
                    pszText: windows::core::PWSTR(title.as_ptr() as *mut _),
                    cChildren: TVITEMEXW_CHILDREN(1),
                    lParam: LPARAM(0),
                    ..Default::default()
                },
            },
        };
        let hitem = crate::send_message_w_safe(
            hwnd_tree,
            TVM_INSERTITEMW,
            WPARAM(0),
            LPARAM(&mut insert as *mut _ as isize),
        );
        with_rss_state(hwnd, |state| {
            state
                .node_data
                .insert(hitem.0, NodeData::GoogleNewsCategory(category.clone()));
        });
    }
}

fn schedule_delayed_source_select(hwnd: HWND, source_index: usize) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        if let Err(e) = crate::post_message_w_safe(
            hwnd,
            WM_RSS_SELECT_SOURCE_DELAYED,
            WPARAM(source_index),
            LPARAM(0),
        ) {
            crate::log_debug(&format!(
                "Failed to post delayed RSS source selection: {}",
                e
            ));
        }
    });
}

fn select_source_by_index(hwnd: HWND, source_index: usize) {
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return;
    }
    let target_hitem = with_rss_state(hwnd, |s| {
        s.node_data.iter().find_map(|(&h, node)| match node {
            NodeData::Source(i) if *i == source_index => {
                Some(windows::Win32::UI::Controls::HTREEITEM(h))
            }
            _ => None,
        })
    })
    .flatten();
    if let Some(target) = target_hitem {
        unsafe {
            SendMessageW(
                hwnd_tree,
                TVM_SELECTITEM,
                WPARAM(TVGN_CARET as usize),
                LPARAM(target.0),
            );
            SendMessageW(hwnd_tree, TVM_ENSUREVISIBLE, WPARAM(0), LPARAM(target.0));
        }
        if crate::get_focus_safe() != hwnd_tree {
            with_rss_state(hwnd, |s| s.suppress_focus_restore_once = true);
            crate::set_focus_safe(hwnd_tree);
        }
    }
}

fn update_source_tree_title(
    hwnd_tree: HWND,
    hitem: windows::Win32::UI::Controls::HTREEITEM,
    title: &str,
) {
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

fn set_source_unread(hwnd: HWND, hitem: windows::Win32::UI::Controls::HTREEITEM, unread: bool) {
    // When marking as read (!unread), also update last_seen_guid from the first item
    let first_item_key: Option<String> = if !unread {
        with_rss_state(hwnd, |s| {
            s.source_items
                .get(&hitem.0)
                .and_then(|state| state.items.first().map(rss_item_key))
        })
        .flatten()
    } else {
        None
    };

    let (hwnd_tree, title_opt) = with_rss_state(hwnd, |s| {
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
                if let Some(src) = ps.settings.rss_sources.get_mut(idx) {
                    let mut changed = false;
                    if src.unread != unread {
                        src.unread = unread;
                        changed = true;
                    }
                    // Update last_seen_guid when marking as read
                    if let Some(ref key) = first_item_key
                        && src.last_seen_guid.as_ref() != Some(key)
                    {
                        src.last_seen_guid = Some(key.clone());
                        changed = true;
                    }
                    if changed {
                        let title = rss_source_display_title(
                            src,
                            language,
                            ps.settings.announce_unread_rss_podcast_items,
                            ps.settings.rss_podcast_unread_label_position,
                        );
                        save_rss_settings(ps);
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

fn populate_related_article_nodes(
    hwnd: HWND,
    parent_hitem: windows::Win32::UI::Controls::HTREEITEM,
    parent_item: &RssItem,
) {
    if parent_item.related_items.is_empty() {
        return;
    }
    let hwnd_tree = with_rss_state(hwnd, |state| state.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return;
    }
    let first_child = crate::send_message_w_safe(
        hwnd_tree,
        TVM_GETNEXTITEM,
        WPARAM(TVGN_CHILD as usize),
        LPARAM(parent_hitem.0),
    );
    if first_child.0 != 0 {
        return;
    }

    let (language, announce_unread, unread_label_position, rss_date_mode, rss_time_mode) =
        with_rss_state(hwnd, |state| {
            with_state(state.parent, |parent_state| {
                (
                    parent_state.settings.language,
                    parent_state.settings.announce_unread_rss_podcast_items,
                    parent_state.settings.rss_podcast_unread_label_position,
                    parent_state.settings.rss_articles_date_display,
                    parent_state.settings.rss_articles_time_display,
                )
            })
            .unwrap_or((
                crate::settings::Language::English,
                true,
                crate::settings::RssPodcastUnreadLabelPosition::Before,
                ListDateDisplayMode::Always,
                ListTimeDisplayMode::Always,
            ))
        })
        .unwrap_or((
            crate::settings::Language::English,
            true,
            crate::settings::RssPodcastUnreadLabelPosition::Before,
            ListDateDisplayMode::Always,
            ListTimeDisplayMode::Always,
        ));
    let read_item_keys = with_rss_state(hwnd, |state| {
        source_ancestor_hitem(state, parent_hitem)
            .and_then(|source_hitem| state.source_items.get(&source_hitem.0))
            .map(|source_state| source_state.read_item_keys.clone())
            .unwrap_or_default()
    })
    .unwrap_or_default();
    let day_counts = build_day_counts(&parent_item.related_items);
    let title_ctx = RssItemTitleContext {
        language,
        announce_unread,
        unread_label_position,
        date_mode: rss_date_mode,
        time_mode: rss_time_mode,
    };

    for related in &parent_item.related_items {
        let item_unread = !read_item_keys.contains(&rss_item_key(related));
        let display_title = rss_item_tree_display_title(
            related,
            item_unread,
            has_multiple_items_same_day(related.pub_date, &day_counts),
            title_ctx,
        );
        let text = to_wide(&display_title);
        let mut insert = TVINSERTSTRUCTW {
            hParent: parent_hitem,
            hInsertAfter: TVI_LAST,
            Anonymous: TVINSERTSTRUCTW_0 {
                item: TVITEMW {
                    mask: TVIF_TEXT | TVIF_PARAM,
                    pszText: windows::core::PWSTR(text.as_ptr() as *mut _),
                    lParam: LPARAM(0),
                    ..Default::default()
                },
            },
        };
        let child = windows::Win32::UI::Controls::HTREEITEM(
            crate::send_message_w_safe(
                hwnd_tree,
                TVM_INSERTITEMW,
                WPARAM(0),
                LPARAM(&mut insert as *mut _ as isize),
            )
            .0,
        );
        if child.0 != 0 {
            with_rss_state(hwnd, |state| {
                state
                    .node_data
                    .insert(child.0, NodeData::Item(related.clone()));
            });
        }
    }
}

fn handle_expand(hwnd: HWND, hitem: windows::Win32::UI::Controls::HTREEITEM) {
    let node = with_rss_state(hwnd, |state| state.node_data.get(&hitem.0).cloned()).flatten();
    match node.as_ref() {
        Some(NodeData::GoogleNewsRoot) => {
            populate_google_news_categories(hwnd, hitem);
            return;
        }
        Some(NodeData::Folder(_)) => return,
        Some(NodeData::Item(item)) if !item.related_items.is_empty() => {
            populate_related_article_nodes(hwnd, hitem, item);
            return;
        }
        Some(NodeData::GoogleNewsCategory(category)) if category.is_local => {
            let parent = with_rss_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
            let city = with_state(parent, |state| state.settings.rss_local_city.clone())
                .unwrap_or_default();
            if city.trim().is_empty() {
                with_rss_state(hwnd, |state| state.pending_local_category = hitem.0);
                show_rss_city_dialog(hwnd, hitem, "");
                return;
            }
        }
        _ => {}
    }

    let has_loaded_items = with_rss_state(hwnd, |state| {
        matches!(
            state.node_data.get(&hitem.0),
            Some(NodeData::Source(_)) | Some(NodeData::GoogleNewsCategory(_))
        ) && state
            .source_items
            .get(&hitem.0)
            .map(|items| !items.items.is_empty())
            .unwrap_or(false)
    })
    .unwrap_or(false);

    let favorites_source_idx = with_rss_state(hwnd, |state| {
        if let Some(NodeData::Source(index)) = state.node_data.get(&hitem.0) {
            with_state(state.parent, |parent_state| {
                parent_state
                    .settings
                    .rss_sources
                    .get(*index)
                    .filter(|source| is_favorites_source(source))
                    .map(|_| *index)
            })
            .flatten()
        } else {
            None
        }
    })
    .flatten();
    if let Some(source_index) = favorites_source_idx {
        load_favorites_source_items(hwnd, hitem, source_index);
        set_source_unread(hwnd, hitem, false);
        return;
    }

    if has_loaded_items {
        set_source_unread(hwnd, hitem, false);
    }

    let item_info_opt = with_rss_state(hwnd, |state| match state.node_data.get(&hitem.0) {
        Some(NodeData::Source(index)) => with_state(state.parent, |parent_state| {
            parent_state.settings.rss_sources.get(*index).map(|source| {
                (
                    source.url.clone(),
                    source.kind.clone(),
                    source.cache.clone(),
                    true,
                )
            })
        })
        .flatten(),
        Some(NodeData::GoogleNewsCategory(category)) => {
            let url = if category.is_local {
                let city = with_state(state.parent, |parent_state| {
                    parent_state.settings.rss_local_city.clone()
                })
                .unwrap_or_default();
                google_news_local_url(&active_news_language_code(state.parent), &city)
            } else {
                category.url.clone()
            };
            Some((
                url,
                RssSourceType::Feed,
                rss::RssFeedCache::default(),
                false,
            ))
        }
        Some(NodeData::Item(item)) if item.is_folder => Some((
            item.link.clone(),
            RssSourceType::Site,
            rss::RssFeedCache::default(),
            false,
        )),
        _ => None,
    });

    let (url, source_kind, mut cache, _is_source) = if let Some(info) = item_info_opt.flatten() {
        info
    } else {
        return;
    };
    let empty_items = with_rss_state(hwnd, |s| {
        s.source_items
            .get(&hitem.0)
            .map(|state| state.items.is_empty())
            .unwrap_or(true)
    })
    .unwrap_or(true);
    if empty_items {
        cache.etag = None;
        cache.last_modified = None;
    }

    with_rss_state(hwnd, |s| {
        s.pending_fetches.insert(url.clone(), hitem.0);
    });

    let url_clone = url.clone();

    // Ensure the node expands immediately for keyboard users (Right Arrow),
    // even when children are populated asynchronously.
    // If there are no children yet, insert a temporary "Loading…" child.
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 != 0 {
        let first_child = crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CHILD as usize),
            LPARAM(hitem.0),
        );
        if first_child.0 == 0 {
            let mut loading_label = "Loading...".to_string();
            if loading_label.trim().is_empty() {
                loading_label = "Loading...".to_string();
            }
            let loading_txt = to_wide(&loading_label);
            let mut tvis_loading = TVINSERTSTRUCTW {
                hParent: hitem,
                hInsertAfter: TVI_LAST,
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
        // Force visual expansion now.
        unsafe {
            SendMessageW(
                hwnd_tree,
                TVM_EXPAND,
                WPARAM(TVE_EXPAND.0 as usize),
                LPARAM(hitem.0),
            );
            SendMessageW(hwnd_tree, TVM_ENSUREVISIBLE, WPARAM(0), LPARAM(hitem.0));
        }
    }

    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    let fetch_config = if parent.0 != 0 {
        rss_fetch_config(parent)
    } else {
        rss::RssFetchConfig::default()
    };
    let language = news_language_as_app_language(&active_news_language_code(parent));
    if parent.0 != 0 {
        ensure_rss_http(parent);
    }

    // UI: "Refresh feeds" should trigger this fetch for the selected source.
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
        let res = rt.block_on(rss::fetch_and_parse(
            &url_clone,
            source_kind,
            cache,
            fetch_config,
            false,
            language,
        ));
        let msg = Box::new(FetchResult {
            hitem: hitem.0,
            result: res,
        });
        if let Err(_e) = crate::post_message_w_safe(
            hwnd,
            WM_RSS_FETCH_COMPLETE,
            WPARAM(0),
            LPARAM(Box::into_raw(msg) as isize),
        ) {}
    });
}

struct FetchResult {
    hitem: isize,
    result: Result<rss::RssFetchOutcome, rss::FeedFetchError>,
}

/// Result of a background check for new articles (lightweight, no UI update needed)
struct BackgroundCheckResult {
    source_idx: usize,
    source_url: String,
    news_language: String,
    newest_item_key: Option<String>,
}

/// Launch background check for all feeds to detect new articles without blocking UI
fn start_background_unread_check(hwnd: HWND) {
    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }

    let sources: Vec<(
        usize,
        String,
        rss::RssSourceType,
        rss::RssFeedCache,
        HashSet<String>,
    )> = {
        with_state(parent, |ps| {
            ps.settings
                .rss_sources
                .iter()
                .enumerate()
                .filter(|(_, src)| !is_favorites_source(src))
                .map(|(i, src)| {
                    (
                        i,
                        src.url.clone(),
                        src.kind.clone(),
                        src.cache.clone(),
                        src.removed_item_keys.iter().cloned().collect(),
                    )
                })
                .collect()
        })
    }
    .unwrap_or_default();

    if sources.is_empty() {
        return;
    }

    let fetch_config = rss_fetch_config(parent);
    let news_language = active_news_language_code(parent);
    let language = news_language_as_app_language(&news_language);
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
            // Process feeds concurrently but not all at once (limit concurrency)
            let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
            let mut handles = Vec::new();

            for (idx, url, kind, cache, removed_keys) in sources {
                let sem = semaphore.clone();
                let cfg = fetch_config;
                let hwnd_val = hwnd_raw;
                let result_language = news_language.clone();
                let result_url = url.clone();

                let handle = tokio::spawn(async move {
                    let _permit = sem.acquire().await.ok()?;
                    let result =
                        rss::fetch_and_parse(&url, kind, cache, cfg, false, language).await;
                    if let Ok(outcome) = result {
                        let newest_key = select_newest_item_key(&outcome.items, &removed_keys);
                        let msg = Box::new(BackgroundCheckResult {
                            source_idx: idx,
                            source_url: result_url,
                            news_language: result_language,
                            newest_item_key: newest_key,
                        });
                        let pointer = Box::into_raw(msg);
                        if let Err(e) = crate::post_message_w_safe(
                            HWND(hwnd_val),
                            WM_RSS_BACKGROUND_CHECK_COMPLETE,
                            WPARAM(0),
                            LPARAM(pointer as isize),
                        ) {
                            let _message_owner = unsafe { Box::from_raw(pointer) };
                            crate::log_debug(&format!(
                                "Failed to post WM_RSS_BACKGROUND_CHECK_COMPLETE: {}",
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

/// Process background check result - update unread state if new articles detected
fn process_background_check_result(hwnd: HWND, res: BackgroundCheckResult) {
    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }

    let Some(newest_key) = res.newest_item_key else {
        return;
    };

    // Ignore results started for a language or source list that is no longer active.
    let should_mark_unread = with_state(parent, |state| {
        if active_news_language_code_from_state(state) != res.news_language {
            return false;
        }
        state
            .settings
            .rss_sources
            .get(res.source_idx)
            .filter(|source| {
                normalize_rss_url_key(&source.url) == normalize_rss_url_key(&res.source_url)
            })
            .map(|source| match &source.last_seen_guid {
                Some(last_seen) => last_seen != &newest_key,
                None => true,
            })
            .unwrap_or(false)
    })
    .unwrap_or(false);

    if should_mark_unread {
        // Find the tree item for this source and mark it unread
        let hitem_opt = with_rss_state(hwnd, |s| {
            for (&h, node) in &s.node_data {
                if let NodeData::Source(idx) = node
                    && *idx == res.source_idx
                {
                    return Some(windows::Win32::UI::Controls::HTREEITEM(h));
                }
            }
            None
        })
        .flatten();

        if let Some(hitem) = hitem_opt {
            set_source_unread(hwnd, hitem, true);
        }
    }
}

fn process_fetch_result(hwnd: HWND, res: FetchResult) {
    unsafe {
        let hitem = windows::Win32::UI::Controls::HTREEITEM(res.hitem);
        let still_current =
            with_rss_state(hwnd, |state| state.node_data.contains_key(&hitem.0)).unwrap_or(false);
        if !still_current {
            return;
        }
        let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
        let caret = windows::Win32::UI::Controls::HTREEITEM(
            SendMessageW(
                hwnd_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_CARET as usize),
                LPARAM(0),
            )
            .0,
        );

        match res.result {
            Ok(outcome) => {
                // Update source title if applicable
                let is_source_node = with_rss_state(hwnd, |s| {
                    s.node_data.contains_key(&hitem.0)
                        && matches!(s.node_data[&hitem.0], NodeData::Source(_))
                })
                .unwrap_or(false);
                if is_source_node {
                    let idx = with_rss_state(hwnd, |s| {
                        if let NodeData::Source(i) = s.node_data[&hitem.0] {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .flatten();
                    if let Some(i) = idx {
                        let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                        let mut final_title = outcome.title.clone();
                        let allow_title_update = caret != hitem;
                        with_state(parent, |ps| {
                            let ui_language = ps.settings.language;
                            let default_language = news_language_as_app_language(
                                &active_news_language_code_from_state(ps),
                            );
                            let (_key, keep_default_title) = ps
                                .settings
                                .rss_sources
                                .get(i)
                                .map(|src| {
                                    let key = normalize_rss_url_key(&src.url);
                                    let keep = is_default_key(default_language, &ps.settings, &key);
                                    (key, keep)
                                })
                                .unwrap_or_default();
                            if let Some(src) = ps.settings.rss_sources.get_mut(i) {
                                let looks_auto =
                                    src.title.trim().is_empty() || src.title == src.url;
                                if !src.user_title
                                    && !keep_default_title
                                    && looks_auto
                                    && !outcome.title.is_empty()
                                {
                                    src.title = outcome.title.clone();
                                }
                                final_title = rss_source_display_title(
                                    src,
                                    ui_language,
                                    ps.settings.announce_unread_rss_podcast_items,
                                    ps.settings.rss_podcast_unread_label_position,
                                );
                                if src.kind != outcome.kind {
                                    src.kind = outcome.kind;
                                }
                                src.cache = outcome.cache.clone();
                                let max_ts = outcome.items.iter().filter_map(|i| i.pub_date).max();
                                if let Some(ts) = max_ts {
                                    src.last_updated = Some(ts);
                                }
                            }
                            save_rss_settings(ps);
                        });

                        if allow_title_update {
                            let title_wide = to_wide(&final_title);
                            let mut tvi = TVITEMW {
                                mask: TVIF_TEXT,
                                hItem: hitem,
                                pszText: windows::core::PWSTR(title_wide.as_ptr() as *mut _),
                                ..Default::default()
                            };
                            SendMessageW(
                                hwnd_tree,
                                TVM_SETITEMW,
                                WPARAM(0),
                                LPARAM(&mut tvi as *mut _ as isize),
                            );
                        }
                    }
                }

                if outcome.not_modified {
                    let has_items = with_rss_state(hwnd, |s| {
                        s.source_items
                            .get(&hitem.0)
                            .map(|state| !state.items.is_empty())
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                    if !has_items {
                        let child = SendMessageW(
                            hwnd_tree,
                            TVM_GETNEXTITEM,
                            WPARAM(TVGN_CHILD as usize),
                            LPARAM(hitem.0),
                        );
                        if child.0 != 0 {
                            let mut item = TVITEMW {
                                mask: TVIF_TEXT,
                                hItem: windows::Win32::UI::Controls::HTREEITEM(child.0),
                                pszText: windows::core::PWSTR::null(),
                                cchTextMax: 0,
                                ..Default::default()
                            };
                            SendMessageW(
                                hwnd_tree,
                                TVM_GETITEMW,
                                WPARAM(0),
                                LPARAM(&mut item as *mut _ as isize),
                            );
                            let mut buf = vec![0u16; 64];
                            item.pszText = windows::core::PWSTR(buf.as_mut_ptr());
                            item.cchTextMax = buf.len() as i32;
                            if SendMessageW(
                                hwnd_tree,
                                TVM_GETITEMW,
                                WPARAM(0),
                                LPARAM(&mut item as *mut _ as isize),
                            )
                            .0 != 0
                            {
                                let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                                let text = String::from_utf16_lossy(&buf[..len]);
                                if text.trim() == "Loading..." {
                                    SendMessageW(
                                        hwnd_tree,
                                        TVM_DELETEITEM,
                                        WPARAM(0),
                                        LPARAM(child.0),
                                    );
                                }
                            }
                        }
                    }
                    return;
                }

                let mut appended = 0usize;
                let mut loaded_before = 0usize;
                let mut total_before = 0usize;
                let existing = with_rss_state(hwnd, |s| s.source_items.contains_key(&hitem.0))
                    .unwrap_or(false);

                let removed_keys = source_removed_keys_for_tree_item(hwnd, hitem);

                if existing {
                    with_rss_state(hwnd, |s| {
                        let Some(state) = s.source_items.get_mut(&hitem.0) else {
                            return;
                        };
                        loaded_before = state.loaded;
                        total_before = state.items.len();
                        let mut seen: HashSet<String> =
                            state.items.iter().map(rss_item_key).collect();
                        for item in outcome.items {
                            let key = rss_item_key(&item);
                            if removed_keys.contains(&key) {
                                continue;
                            }
                            if seen.insert(key) {
                                state.items.push(item);
                                appended += 1;
                            }
                        }
                        // Keep a consistent chronological order for all RSS feeds.
                        // Google News often requires this, but standard feeds can also arrive unsorted.
                        sort_items_by_date_desc(&mut state.items);
                    });
                    if appended > 0 {
                        log_debug(&format!(
                            "rss_ui_batch start source={} append_count={} loaded_before={} total_before={}",
                            hitem.0, appended, loaded_before, total_before
                        ));
                        if loaded_before >= total_before {
                            let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                            let (_initial_count, next_count) = rss_page_sizes(parent);
                            let (_inserted, _first_inserted) =
                                load_more_items(hwnd, hitem, next_count);
                        }
                        log_debug(&format!(
                            "rss_ui_batch end source={} appended={}",
                            hitem.0, appended
                        ));
                        set_source_unread(hwnd, hitem, true);
                    } else {
                        // Feed opened/refreshed with no newly appended items: clear "new items" flag.
                        // This keeps source-level status aligned with real incoming updates.
                        set_source_unread(hwnd, hitem, false);
                    }
                } else {
                    loop {
                        let child = SendMessageW(
                            hwnd_tree,
                            TVM_GETNEXTITEM,
                            WPARAM(TVGN_CHILD as usize),
                            LPARAM(hitem.0),
                        );
                        if child.0 == 0 {
                            break;
                        }
                        SendMessageW(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(child.0));
                    }

                    let saved_read_item_keys: HashSet<String> = with_rss_state(hwnd, |s| {
                        let source_index = match s.node_data.get(&hitem.0) {
                            Some(NodeData::Source(index)) => Some(*index),
                            _ => None,
                        };
                        with_state(s.parent, |ps| {
                            source_index
                                .and_then(|index| ps.settings.rss_sources.get(index))
                                .map(|src| src.read_item_keys.iter().cloned().collect())
                        })
                        .flatten()
                        .unwrap_or_default()
                    })
                    .unwrap_or_default();

                    with_rss_state(hwnd, |s| {
                        let mut items: Vec<RssItem> = outcome
                            .items
                            .into_iter()
                            .filter(|item| !removed_keys.contains(&rss_item_key(item)))
                            .collect();
                        // Keep a consistent chronological order for all RSS feeds.
                        sort_items_by_date_desc(&mut items);
                        s.source_items.insert(
                            hitem.0,
                            SourceItemsState {
                                items,
                                loaded: 0,
                                read_item_keys: saved_read_item_keys,
                            },
                        );
                    });
                    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                    let (initial_count, _next_count) = rss_page_sizes(parent);
                    log_debug(&format!(
                        "rss_ui_batch start source={} append_count={} loaded_before=0 total_before=0",
                        hitem.0, initial_count
                    ));
                    let (inserted, _) = load_more_items(hwnd, hitem, initial_count);
                    log_debug(&format!(
                        "rss_ui_batch end source={} appended={}",
                        hitem.0, inserted
                    ));

                    // User expanded the feed and items are now loaded - mark as read
                    // This updates last_seen_guid to the newest item
                    set_source_unread(hwnd, hitem, false);
                }
                prune_persisted_read_keys_for_source(hwnd, hitem);
            }
            Err(e) => {
                let (message, cache) = match e {
                    rss::FeedFetchError::HttpStatus {
                        status,
                        kind,
                        cache,
                    } => (format!("Feed error {status} ({kind})."), Some(*cache)),
                    rss::FeedFetchError::Network { message, cache } => {
                        (format!("Error: {message}"), Some(*cache))
                    }
                };

                let is_source_node = with_rss_state(hwnd, |s| {
                    s.node_data.contains_key(&hitem.0)
                        && matches!(s.node_data[&hitem.0], NodeData::Source(_))
                })
                .unwrap_or(false);
                if is_source_node {
                    let idx = with_rss_state(hwnd, |s| {
                        if let NodeData::Source(i) = s.node_data[&hitem.0] {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .flatten();
                    if let (Some(i), Some(cache)) = (idx, cache) {
                        let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                        with_state(parent, |ps| {
                            if let Some(src) = ps.settings.rss_sources.get_mut(i) {
                                src.cache = cache;
                            }
                            save_rss_settings(ps);
                        });
                    }
                }

                let has_items = with_rss_state(hwnd, |s| s.source_items.contains_key(&hitem.0))
                    .unwrap_or(false);
                if has_items {
                    return;
                }
                loop {
                    let child = SendMessageW(
                        hwnd_tree,
                        TVM_GETNEXTITEM,
                        WPARAM(TVGN_CHILD as usize),
                        LPARAM(hitem.0),
                    );
                    if child.0 == 0 {
                        break;
                    }
                    SendMessageW(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(child.0));
                }
                with_rss_state(hwnd, |s| {
                    s.source_items.remove(&hitem.0);
                });
                let text = to_wide(&message);
                let mut tvis = TVINSERTSTRUCTW {
                    hParent: hitem,
                    hInsertAfter: TVI_LAST,
                    Anonymous: TVINSERTSTRUCTW_0 {
                        item: TVITEMW {
                            mask: TVIF_TEXT,
                            pszText: windows::core::PWSTR(text.as_ptr() as *mut _),
                            ..Default::default()
                        },
                    },
                };
                SendMessageW(
                    hwnd_tree,
                    TVM_INSERTITEMW,
                    WPARAM(0),
                    LPARAM(&mut tvis as *mut _ as isize),
                );
            }
        }
    }
}

fn handle_selection_changed(hwnd: HWND, hitem: windows::Win32::UI::Controls::HTREEITEM) {
    if hitem.0 == 0 {
        return;
    }
    with_rss_state(hwnd, |s| s.last_selected = hitem.0);
}

fn load_more_items(
    hwnd: HWND,
    hitem: windows::Win32::UI::Controls::HTREEITEM,
    batch: usize,
) -> (usize, windows::Win32::UI::Controls::HTREEITEM) {
    // UI: "Load more titles" can call this to append the next page locally.
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return (0, windows::Win32::UI::Controls::HTREEITEM(0));
    }
    let (language, announce_unread, unread_label_position, rss_date_mode, rss_time_mode) =
        with_rss_state(hwnd, |s| {
            {
                with_state(s.parent, |ps| {
                    (
                        ps.settings.language,
                        ps.settings.announce_unread_rss_podcast_items,
                        ps.settings.rss_podcast_unread_label_position,
                        ps.settings.rss_articles_date_display,
                        ps.settings.rss_articles_time_display,
                    )
                })
            }
            .unwrap_or((
                crate::settings::Language::English,
                true,
                crate::settings::RssPodcastUnreadLabelPosition::Before,
                ListDateDisplayMode::Always,
                ListTimeDisplayMode::Always,
            ))
        })
        .unwrap_or((
            crate::settings::Language::English,
            true,
            crate::settings::RssPodcastUnreadLabelPosition::Before,
            ListDateDisplayMode::Always,
            ListTimeDisplayMode::Always,
        ));

    crate::send_message_w_safe(hwnd_tree, WM_SETREDRAW, WPARAM(0), LPARAM(0));
    delete_load_more_nodes(hwnd, hitem);
    let (inserted, loaded_after, total_after, first_inserted) = with_rss_state(hwnd, |s| {
        let Some(state) = s.source_items.get_mut(&hitem.0) else {
            return (
                0usize,
                0usize,
                0usize,
                windows::Win32::UI::Controls::HTREEITEM(0),
            );
        };
        if state.loaded >= state.items.len() {
            return (
                0usize,
                state.loaded,
                state.items.len(),
                windows::Win32::UI::Controls::HTREEITEM(0),
            );
        }
        let mut inserted = 0usize;
        let mut idx = state.loaded;
        let mut first_inserted = windows::Win32::UI::Controls::HTREEITEM(0);
        let day_counts = build_day_counts(&state.items);
        while idx < state.items.len() && inserted < batch {
            let item = &state.items[idx];
            idx += 1;
            if item.title.trim().is_empty() {
                continue;
            }
            let item_unread = !state.read_item_keys.contains(&rss_item_key(item));
            let title_ctx = RssItemTitleContext {
                language,
                announce_unread,
                unread_label_position,
                date_mode: rss_date_mode,
                time_mode: rss_time_mode,
            };
            let display_title = rss_item_tree_display_title(
                item,
                item_unread,
                has_multiple_items_same_day(item.pub_date, &day_counts),
                title_ctx,
            );
            let text = to_wide(&display_title);
            let c_children = if item.is_folder || !item.related_items.is_empty() {
                1
            } else {
                0
            };
            let mut tvis = TVINSERTSTRUCTW {
                hParent: hitem,
                hInsertAfter: TVI_LAST,
                Anonymous: TVINSERTSTRUCTW_0 {
                    item: TVITEMW {
                        mask: TVIF_TEXT | TVIF_PARAM | windows::Win32::UI::Controls::TVIF_CHILDREN,
                        pszText: windows::core::PWSTR(text.as_ptr() as *mut _),
                        cChildren: TVITEMEXW_CHILDREN(c_children),
                        lParam: LPARAM(0),
                        ..Default::default()
                    },
                },
            };
            let hchild = crate::send_message_w_safe(
                hwnd_tree,
                TVM_INSERTITEMW,
                WPARAM(0),
                LPARAM(&mut tvis as *mut _ as isize),
            );
            s.node_data.insert(hchild.0, NodeData::Item(item.clone()));
            if inserted == 0 && hchild.0 != 0 {
                first_inserted = windows::Win32::UI::Controls::HTREEITEM(hchild.0);
            }
            inserted += 1;
        }
        state.loaded = idx;
        (inserted, state.loaded, state.items.len(), first_inserted)
    })
    .unwrap_or((
        0usize,
        0usize,
        0usize,
        windows::Win32::UI::Controls::HTREEITEM(0),
    ));
    if loaded_after < total_after {
        append_load_more_node(hwnd, hitem, language);
    }
    crate::send_message_w_safe(hwnd_tree, WM_SETREDRAW, WPARAM(1), LPARAM(0));
    if inserted > 0 {
        log_debug(&format!(
            "rss_ui_batch append source={} inserted={} loaded={} total={}",
            hitem.0, inserted, loaded_after, total_after
        ));
    }
    (inserted, first_inserted)
}

fn handle_enter_action(hwnd: HWND, open_in_browser: bool) {
    // UI: Enter imports the article, Shift+Enter opens it in the browser.
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    let hitem = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    if hitem.0 == 0 {
        return;
    }

    let load_more_parent = with_rss_state(hwnd, |s| match s.node_data.get(&hitem.0) {
        Some(NodeData::LoadMore) => Some(windows::Win32::UI::Controls::HTREEITEM(
            crate::send_message_w_safe(
                s.hwnd_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_PARENT as usize),
                LPARAM(hitem.0),
            )
            .0,
        )),
        _ => None,
    })
    .flatten();
    if let Some(parent) = load_more_parent {
        let parent_hwnd = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
        let (_initial_count, next_count) = rss_page_sizes(parent_hwnd);
        let (inserted, first_inserted) = load_more_items(hwnd, parent, next_count);
        if inserted > 0 && first_inserted.0 != 0 {
            crate::send_message_w_safe(
                hwnd_tree,
                TVM_SELECTITEM,
                WPARAM(TVGN_CARET as usize),
                LPARAM(first_inserted.0),
            );
            crate::send_message_w_safe(
                hwnd_tree,
                TVM_ENSUREVISIBLE,
                WPARAM(0),
                LPARAM(first_inserted.0),
            );
        }
        return;
    }

    let selected_node = with_rss_state(hwnd, |s| s.node_data.get(&hitem.0).cloned()).flatten();
    let item_opt = match selected_node {
        Some(NodeData::Item(item)) if !item.is_folder => Some(item),
        _ => None,
    };

    if let Some(item) = item_opt {
        let item_key = rss_item_key(&item);
        with_rss_state(hwnd, |s| {
            let source_hitem = source_ancestor_hitem(s, hitem);
            if let Some(source_hitem) = source_hitem {
                if let Some(state) = s.source_items.get_mut(&source_hitem.0) {
                    state.read_item_keys.insert(item_key.clone());
                }
                if let Some(NodeData::Source(source_index)) = s.node_data.get(&source_hitem.0) {
                    with_state(s.parent, |ps| {
                        if let Some(src) = ps.settings.rss_sources.get_mut(*source_index)
                            && !src.read_item_keys.iter().any(|k| k == &item_key)
                        {
                            src.read_item_keys.push(item_key.clone());
                            const MAX_PERSISTED_READ_KEYS: usize = 5000;
                            if src.read_item_keys.len() > MAX_PERSISTED_READ_KEYS {
                                let overflow = src.read_item_keys.len() - MAX_PERSISTED_READ_KEYS;
                                src.read_item_keys.drain(0..overflow);
                            }
                            save_rss_settings(ps);
                        }
                    });
                }
            }
        });
        let delayed_key = rss_item_key(&item);
        let delayed_hitem = hitem.0;
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let payload = Box::new(MarkItemReadUiMessage {
                hitem: delayed_hitem,
                item_key: delayed_key,
            });
            let payload_ptr = Box::into_raw(payload);
            if let Err(e) = crate::post_message_w_safe(
                hwnd,
                WM_RSS_MARK_ITEM_READ_UI,
                WPARAM(0),
                LPARAM(payload_ptr as isize),
            ) {
                let _payload_owner = crate::box_from_raw_safe(payload_ptr);
                crate::log_debug(&format!("Failed to post WM_RSS_MARK_ITEM_READ_UI: {}", e));
            }
        });
        if open_in_browser {
            handle_article_action(hwnd, ArticleAction::OpenInBrowser);
        } else {
            import_item(hwnd, item);
        }
    } else {
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_EXPAND,
            WPARAM(TVE_EXPAND.0 as usize),
            LPARAM(hitem.0),
        );
    }
}

fn handle_select_articles(hwnd: HWND) {
    unsafe {
        let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
        if hwnd_tree.0 == 0 {
            return;
        }
        let selected_hitem = windows::Win32::UI::Controls::HTREEITEM(
            SendMessageW(
                hwnd_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_CARET as usize),
                LPARAM(0),
            )
            .0,
        );
        if selected_hitem.0 == 0 {
            return;
        }
        let source_hitem = windows::Win32::UI::Controls::HTREEITEM(
            SendMessageW(
                hwnd_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_PARENT as usize),
                LPARAM(selected_hitem.0),
            )
            .0,
        );
        if source_hitem.0 == 0 {
            return;
        }
        let Some((source_index, candidates)) = with_rss_state(hwnd, |s| {
            let source_index = match s.node_data.get(&source_hitem.0) {
                Some(NodeData::Source(index)) => *index,
                _ => return None,
            };
            let items = s.source_items.get(&source_hitem.0)?;
            let candidates = items
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| !item.is_folder)
                .map(|(position, item)| (position, item.clone()))
                .collect::<Vec<_>>();
            Some((source_index, candidates))
        })
        .flatten() else {
            return;
        };
        if candidates.is_empty() {
            return;
        }

        let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
        let (language, require_confirm) = with_state(parent, |ps| {
            (
                ps.settings.language,
                matches!(
                    ps.settings.rss_delete_confirm_mode,
                    crate::settings::RssDeleteConfirmMode::Article
                        | crate::settings::RssDeleteConfirmMode::Both
                ),
            )
        })
        .unwrap_or((crate::settings::Language::default(), true));

        let labels = candidates
            .iter()
            .map(|(_, item)| {
                if item.title.trim().is_empty() {
                    item.link.clone()
                } else {
                    item.title.clone()
                }
            })
            .collect::<Vec<_>>();
        let Some(mut selected_indices) =
            youtube_transcript_window::choose_checkbox_selection_entries(
                hwnd,
                language,
                youtube_transcript_window::CheckboxSelectionDialogText {
                    title: i18n::tr(language, "rss.multi_select.title"),
                    instructions: i18n::tr(language, "rss.multi_select.instructions"),
                    accept_label: i18n::tr(language, "rss.multi_select.delete"),
                    none_selected_message: i18n::tr(language, "rss.multi_select.none_selected"),
                    selected_count_template: i18n::tr(
                        language,
                        "stream_audio.playlist_download_selected_count",
                    ),
                },
                labels,
            )
        else {
            SetFocus(hwnd_tree);
            return;
        };
        selected_indices.sort_unstable();
        selected_indices.dedup();
        let selected = selected_indices
            .into_iter()
            .filter_map(|index| candidates.get(index).cloned())
            .collect::<Vec<_>>();
        if selected.is_empty() {
            SetFocus(hwnd_tree);
            return;
        }

        if require_confirm {
            let count = selected.len().to_string();
            let message = i18n::tr(language, "rss.multi_select.confirm").replace("{count}", &count);
            let caption = i18n::tr(language, "rss.delete_title");
            if MessageBoxW(
                hwnd,
                PCWSTR(to_wide(&message).as_ptr()),
                PCWSTR(to_wide(&caption).as_ptr()),
                MB_YESNOCANCEL | MB_ICONQUESTION,
            ) != IDYES
            {
                SetFocus(hwnd_tree);
                return;
            }
        }

        let selected_keys = selected
            .iter()
            .map(|(_, item)| rss_item_key(item))
            .collect::<HashSet<_>>();
        let source_is_favorites = with_state(parent, |ps| {
            ps.settings
                .rss_sources
                .get(source_index)
                .is_some_and(is_favorites_source)
        })
        .unwrap_or(false);

        with_state(parent, |ps| {
            if source_is_favorites {
                ps.settings
                    .rss_favorite_articles
                    .retain(|item| !selected_keys.contains(&rss_item_key(item)));
                if let Some(source) = ps.settings.rss_sources.get_mut(source_index) {
                    source
                        .removed_item_keys
                        .retain(|key| !selected_keys.contains(key));
                }
            } else if let Some(source) = ps.settings.rss_sources.get_mut(source_index) {
                for key in &selected_keys {
                    if !source
                        .removed_item_keys
                        .iter()
                        .any(|existing| existing == key)
                    {
                        source.removed_item_keys.push(key.clone());
                    }
                }
            }
            save_rss_settings(ps);
        });

        let first_remaining_key = with_rss_state(hwnd, |s| {
            let state = s.source_items.get_mut(&source_hitem.0)?;
            let removed_loaded = selected
                .iter()
                .filter(|(position, _)| *position < state.loaded)
                .count();
            state
                .items
                .retain(|item| !selected_keys.contains(&rss_item_key(item)));
            state.loaded = state
                .loaded
                .saturating_sub(removed_loaded)
                .min(state.items.len());
            state.items.first().map(rss_item_key)
        })
        .flatten()
        .unwrap_or_default();

        // Keep normal Undo semantics: each selected article can be restored with Ctrl+Z,
        // in reverse deletion order, while preserving its original source position.
        with_rss_state(hwnd, |s| {
            for (position, item) in &selected {
                s.removed_history.push(RssLastRemoved::Item {
                    source_index,
                    item: item.clone(),
                    key: rss_item_key(item),
                    position: *position,
                });
            }
        });

        with_rss_state(hwnd, |s| s.suppress_tree_selection_events = true);
        let rebuilt = rebuild_source_children_from_state(hwnd, source_hitem, &first_remaining_key);
        with_rss_state(hwnd, |s| s.suppress_tree_selection_events = false);
        if let Some(target) = rebuilt.filter(|item| item.0 != 0) {
            SendMessageW(
                hwnd_tree,
                TVM_SELECTITEM,
                WPARAM(TVGN_CARET as usize),
                LPARAM(target.0),
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
        let count = selected.len().to_string();
        announce_rss_status(
            &i18n::tr(language, "rss.multi_select.deleted").replace("{count}", &count),
        );
    }
}

fn removed_default_list_mut(
    settings: &mut crate::settings::AppSettings,
    language: crate::settings::Language,
) -> &mut Vec<String> {
    match language {
        crate::settings::Language::German => &mut settings.rss_removed_default_de,
        crate::settings::Language::Italian => &mut settings.rss_removed_default_it,
        crate::settings::Language::Spanish => &mut settings.rss_removed_default_es,
        crate::settings::Language::Portuguese => &mut settings.rss_removed_default_pt,
        crate::settings::Language::PortugueseBrazilian => &mut settings.rss_removed_default_pt_br,
        crate::settings::Language::Vietnamese => &mut settings.rss_removed_default_vi,
        crate::settings::Language::Czech => &mut settings.rss_removed_default_cs,
        crate::settings::Language::Polish => &mut settings.rss_removed_default_pl,
        crate::settings::Language::French => &mut settings.rss_removed_default_fr,
        crate::settings::Language::Serbian => &mut settings.rss_removed_default_sr,
        crate::settings::Language::Hindi => &mut settings.rss_removed_default_hi,
        crate::settings::Language::English
        | crate::settings::Language::Swedish
        | crate::settings::Language::Ukrainian
        | crate::settings::Language::Lithuanian
        | crate::settings::Language::Russian
        | crate::settings::Language::Chinese => &mut settings.rss_removed_default_en,
    }
}

fn mark_default_rss_source_removed(
    settings: &mut crate::settings::AppSettings,
    language: crate::settings::Language,
    url: &str,
) -> Option<String> {
    let key = normalize_rss_url_key(url);
    if key.is_empty() {
        return None;
    }
    let is_default = load_default_feeds(language)
        .into_iter()
        .any(|(_title, default_url)| normalize_rss_url_key(&default_url) == key);
    if !is_default {
        return None;
    }
    let removed = removed_default_list_mut(settings, language);
    if removed
        .iter()
        .any(|existing| normalize_rss_url_key(existing) == key)
    {
        return None;
    }
    removed.push(key.clone());
    Some(key)
}

fn unmark_default_rss_source_removed(
    settings: &mut crate::settings::AppSettings,
    language: crate::settings::Language,
    key: &str,
) {
    let removed = removed_default_list_mut(settings, language);
    removed.retain(|url| normalize_rss_url_key(url) != key);
}

fn handle_delete_folder(hwnd: HWND, folder_path: Vec<String>) {
    let folder_path = normalized_folder_path(&folder_path);
    if folder_path.is_empty() {
        return;
    }
    let parent = with_rss_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }
    let (language, news_code, news_language, source_count) = with_state(parent, |state| {
        let code = active_news_language_code_from_state(state);
        let count = state
            .settings
            .rss_sources
            .iter()
            .filter(|source| source.folder_path.starts_with(folder_path.as_slice()))
            .count();
        (
            state.settings.language,
            code.clone(),
            news_language_as_app_language(&code),
            count,
        )
    })
    .unwrap_or((
        crate::settings::Language::default(),
        String::new(),
        crate::settings::Language::default(),
        0,
    ));
    let folder_name = folder_path.last().cloned().unwrap_or_default();
    let message = i18n::tr(language, "rss.folder.delete_confirm")
        .replace("{title}", &folder_name)
        .replace("{count}", &source_count.to_string());
    let caption = i18n::tr(language, "rss.folder.delete_title");
    let confirmed = unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(to_wide(&message).as_ptr()),
            PCWSTR(to_wide(&caption).as_ptr()),
            MB_YESNOCANCEL | MB_ICONQUESTION,
        ) == IDYES
    };
    if !confirmed {
        return;
    }

    let removed = with_state(parent, |state| {
        let stored_folders = state
            .settings
            .rss_folders_by_language
            .get(&news_code)
            .cloned()
            .unwrap_or_default();
        let folders = stored_folders
            .into_iter()
            .filter(|path| path.starts_with(folder_path.as_slice()))
            .collect::<Vec<_>>();
        let candidates = state
            .settings
            .rss_sources
            .iter()
            .enumerate()
            .filter(|(_index, source)| source.folder_path.starts_with(folder_path.as_slice()))
            .map(|(index, source)| (index, source.clone()))
            .collect::<Vec<_>>();
        let mut sources = Vec::with_capacity(candidates.len());
        for (index, source) in candidates {
            let default_key =
                mark_default_rss_source_removed(&mut state.settings, news_language, &source.url);
            sources.push((index, source, default_key));
        }
        state
            .settings
            .rss_sources
            .retain(|source| !source.folder_path.starts_with(folder_path.as_slice()));
        if let Some(active_folders) = state.settings.rss_folders_by_language.get_mut(&news_code) {
            active_folders.retain(|path| !path.starts_with(folder_path.as_slice()));
        }
        save_rss_settings(state);
        (sources, folders)
    });
    let Some((sources, folders)) = removed else {
        return;
    };

    with_rss_state(hwnd, |state| {
        state.removed_history.push(RssLastRemoved::Folder {
            path: folder_path,
            sources,
            folders,
            language: news_language,
            news_code,
        });
        state.suppress_tree_selection_events = true;
    });
    let tree = with_rss_state(hwnd, |state| state.hwnd_tree).unwrap_or(HWND(0));
    if tree.0 != 0 {
        crate::send_message_w_safe(tree, WM_SETREDRAW, WPARAM(0), LPARAM(0));
    }
    reload_tree(hwnd);
    if tree.0 != 0 {
        crate::send_message_w_safe(tree, WM_SETREDRAW, WPARAM(1), LPARAM(0));
        with_rss_state(hwnd, |state| state.suppress_tree_selection_events = false);
        select_first_root_if_needed(hwnd, tree);
        crate::set_focus_safe(tree);
    } else {
        with_rss_state(hwnd, |state| state.suppress_tree_selection_events = false);
    }
    announce_rss_status(&i18n::tr(language, "rss.folder.deleted"));
}

fn handle_delete(hwnd: HWND) {
    unsafe {
        let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
        let hitem = windows::Win32::UI::Controls::HTREEITEM(
            SendMessageW(
                hwnd_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_CARET as usize),
                LPARAM(0),
            )
            .0,
        );
        if hitem.0 == 0 {
            return;
        }

        let selected_node = with_rss_state(hwnd, |s| s.node_data.get(&hitem.0).cloned()).flatten();

        if let Some(NodeData::Folder(folder_path)) = selected_node.as_ref() {
            handle_delete_folder(hwnd, folder_path.clone());
            return;
        }

        if let Some(NodeData::Source(idx)) = selected_node {
            let source_info = with_rss_state(hwnd, |s| {
                with_state(s.parent, |ps| ps.settings.rss_sources.get(idx).cloned()).flatten()
            })
            .flatten();
            let (title, url) = source_info
                .as_ref()
                .map(|src| (src.title.clone(), src.url.clone()))
                .unwrap_or_default();

            // Localize message and title
            let (language, require_confirm) = with_rss_state(hwnd, |s| {
                with_state(s.parent, |ps| {
                    (
                        ps.settings.language,
                        matches!(
                            ps.settings.rss_delete_confirm_mode,
                            crate::settings::RssDeleteConfirmMode::Feed
                                | crate::settings::RssDeleteConfirmMode::Both
                        ),
                    )
                })
                .unwrap_or((crate::settings::Language::default(), true))
            })
            .unwrap_or((crate::settings::Language::default(), true));
            let news_language = news_language_as_app_language(&active_news_language_code(
                with_rss_state(hwnd, |state| state.parent).unwrap_or(HWND(0)),
            ));
            let msg_template = i18n::tr(language, "rss.delete_confirm");
            let msg_text = msg_template.replace("{title}", &title);
            let caption = i18n::tr(language, "rss.delete_title");

            let confirmed = if require_confirm {
                MessageBoxW(
                    hwnd,
                    PCWSTR(to_wide(&msg_text).as_ptr()),
                    PCWSTR(to_wide(&caption).as_ptr()),
                    MB_YESNOCANCEL | MB_ICONQUESTION,
                ) == IDYES
            } else {
                true
            };
            if confirmed {
                let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                let mut default_removed_key_added: Option<String> = None;
                let mut delayed_target_source_idx: Option<usize> = None;
                with_state(parent, |ps| {
                    if matches!(
                        news_language,
                        crate::settings::Language::English
                            | crate::settings::Language::German
                            | crate::settings::Language::Swedish
                            | crate::settings::Language::Italian
                            | crate::settings::Language::Spanish
                            | crate::settings::Language::Portuguese
                            | crate::settings::Language::PortugueseBrazilian
                            | crate::settings::Language::Vietnamese
                            | crate::settings::Language::Czech
                            | crate::settings::Language::Polish
                            | crate::settings::Language::French
                            | crate::settings::Language::Serbian
                    ) {
                        let defaults = load_default_feeds(news_language);
                        if !defaults.is_empty() {
                            let mut default_keys = HashSet::new();
                            for (_title, url) in defaults {
                                let key = normalize_rss_url_key(&url);
                                if !key.is_empty() {
                                    default_keys.insert(key);
                                }
                            }
                            let key = normalize_rss_url_key(&url);
                            if !key.is_empty() && default_keys.contains(&key) {
                                let removed_list = match news_language {
                                    crate::settings::Language::Ukrainian
                                    | crate::settings::Language::English
                                    | crate::settings::Language::Lithuanian
                                    | crate::settings::Language::Chinese
                                    | crate::settings::Language::Russian => {
                                        &mut ps.settings.rss_removed_default_en
                                    }
                                    crate::settings::Language::German => {
                                        &mut ps.settings.rss_removed_default_de
                                    }
                                    crate::settings::Language::Swedish => {
                                        &mut ps.settings.rss_removed_default_en
                                    }
                                    crate::settings::Language::Italian => {
                                        &mut ps.settings.rss_removed_default_it
                                    }
                                    crate::settings::Language::Spanish => {
                                        &mut ps.settings.rss_removed_default_es
                                    }
                                    crate::settings::Language::Portuguese => {
                                        &mut ps.settings.rss_removed_default_pt
                                    }
                                    crate::settings::Language::PortugueseBrazilian => {
                                        &mut ps.settings.rss_removed_default_pt_br
                                    }
                                    crate::settings::Language::Vietnamese => {
                                        &mut ps.settings.rss_removed_default_vi
                                    }
                                    crate::settings::Language::Czech => {
                                        &mut ps.settings.rss_removed_default_cs
                                    }
                                    crate::settings::Language::Polish => {
                                        &mut ps.settings.rss_removed_default_pl
                                    }
                                    crate::settings::Language::French => {
                                        &mut ps.settings.rss_removed_default_fr
                                    }
                                    crate::settings::Language::Serbian => {
                                        &mut ps.settings.rss_removed_default_sr
                                    }
                                    crate::settings::Language::Hindi => {
                                        &mut ps.settings.rss_removed_default_hi
                                    }
                                };
                                let already =
                                    removed_list.iter().any(|u| normalize_rss_url_key(u) == key);
                                if !already {
                                    removed_list.push(key.clone());
                                    default_removed_key_added = Some(key);
                                }
                            }
                        }
                    }
                    if is_favorites_source_url(&url) {
                        ps.settings.rss_favorite_articles.clear();
                    }
                    ps.settings.rss_sources.remove(idx);
                    delayed_target_source_idx = if ps.settings.rss_sources.is_empty() {
                        None
                    } else if idx > 0 {
                        Some(idx - 1)
                    } else {
                        Some(0)
                    };
                    save_rss_settings(ps);
                });
                with_rss_state(hwnd, |s| s.suppress_tree_selection_events = true);
                SendMessageW(hwnd_tree, WM_SETREDRAW, WPARAM(0), LPARAM(0));
                reload_tree(hwnd);
                SendMessageW(hwnd_tree, WM_SETREDRAW, WPARAM(1), LPARAM(0));
                with_rss_state(hwnd, |s| s.suppress_tree_selection_events = false);
                if let Some(source) = source_info {
                    with_rss_state(hwnd, |s| {
                        s.removed_history.push(RssLastRemoved::Source {
                            index: idx,
                            source,
                            language: news_language,
                            default_removed_key_added,
                        });
                    });
                }
                announce_rss_status(&i18n::tr(language, "rss.removed"));
                if let Some(target_idx) = delayed_target_source_idx {
                    schedule_delayed_source_select(hwnd, target_idx);
                }
            }
        } else if let Some(NodeData::Item(item)) = selected_node {
            let (language, require_confirm) = with_rss_state(hwnd, |s| {
                with_state(s.parent, |ps| {
                    (
                        ps.settings.language,
                        matches!(
                            ps.settings.rss_delete_confirm_mode,
                            crate::settings::RssDeleteConfirmMode::Article
                                | crate::settings::RssDeleteConfirmMode::Both
                        ),
                    )
                })
                .unwrap_or((crate::settings::Language::default(), true))
            })
            .unwrap_or((crate::settings::Language::default(), true));
            let title = if item.title.trim().is_empty() {
                item.link.clone()
            } else {
                item.title.clone()
            };
            let msg_template = i18n::tr(language, "rss.delete_confirm");
            let msg_text = msg_template.replace("{title}", &title);
            let caption = i18n::tr(language, "rss.delete_title");
            let confirmed = if require_confirm {
                MessageBoxW(
                    hwnd,
                    PCWSTR(to_wide(&msg_text).as_ptr()),
                    PCWSTR(to_wide(&caption).as_ptr()),
                    MB_YESNOCANCEL | MB_ICONQUESTION,
                ) == IDYES
            } else {
                true
            };
            if confirmed {
                let parent_item = windows::Win32::UI::Controls::HTREEITEM(
                    SendMessageW(
                        hwnd_tree,
                        TVM_GETNEXTITEM,
                        WPARAM(TVGN_PARENT as usize),
                        LPARAM(hitem.0),
                    )
                    .0,
                );
                let next_sibling = windows::Win32::UI::Controls::HTREEITEM(
                    SendMessageW(
                        hwnd_tree,
                        TVM_GETNEXTITEM,
                        WPARAM(windows::Win32::UI::Controls::TVGN_NEXT as usize),
                        LPARAM(hitem.0),
                    )
                    .0,
                );
                let prev_sibling = windows::Win32::UI::Controls::HTREEITEM(
                    SendMessageW(
                        hwnd_tree,
                        TVM_GETNEXTITEM,
                        WPARAM(windows::Win32::UI::Controls::TVGN_PREVIOUS as usize),
                        LPARAM(hitem.0),
                    )
                    .0,
                );
                let key = rss_item_key(&item);
                let mut source_idx_for_undo: Option<usize> = None;
                let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                if parent.0 != 0 {
                    let source_index =
                        with_rss_state(hwnd, |s| match s.node_data.get(&parent_item.0) {
                            Some(NodeData::Source(idx)) => Some(*idx),
                            _ => None,
                        })
                        .flatten();
                    if let Some(source_idx) = source_index {
                        source_idx_for_undo = Some(source_idx);
                        with_state(parent, |ps| {
                            let is_favorites = ps
                                .settings
                                .rss_sources
                                .get(source_idx)
                                .is_some_and(is_favorites_source);
                            if is_favorites {
                                let removed = remove_favorite_article_by_key(
                                    &mut ps.settings.rss_favorite_articles,
                                    &key,
                                );
                                if let Some(src) = ps.settings.rss_sources.get_mut(source_idx) {
                                    // Old builds could have recorded these keys even though the
                                    // favorites loader never consumed them. Keep the special
                                    // source clean now that deletion is persisted directly.
                                    src.removed_item_keys.retain(|k| k != &key);
                                }
                                if removed {
                                    save_rss_settings(ps);
                                }
                            } else if let Some(src) = ps.settings.rss_sources.get_mut(source_idx)
                                && !src.removed_item_keys.iter().any(|k| k == &key)
                            {
                                src.removed_item_keys.push(key.clone());
                                save_rss_settings(ps);
                            }
                        });
                    }
                }
                let mut removed_position: Option<usize> = None;
                with_rss_state(hwnd, |s| {
                    s.node_data.remove(&hitem.0);
                    if parent_item.0 != 0
                        && let Some(state) = s.source_items.get_mut(&parent_item.0)
                        && let Some(pos) = state.items.iter().position(|x| rss_item_key(x) == key)
                    {
                        removed_position = Some(pos);
                        state.items.remove(pos);
                        state.loaded = state.loaded.saturating_sub(1);
                    }
                });
                SendMessageW(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(hitem.0));
                let next_exists = next_sibling.0 != 0
                    && with_rss_state(hwnd, |s| s.node_data.contains_key(&next_sibling.0))
                        .unwrap_or(false);
                let prev_exists = prev_sibling.0 != 0
                    && with_rss_state(hwnd, |s| s.node_data.contains_key(&prev_sibling.0))
                        .unwrap_or(false);
                if next_exists {
                    SendMessageW(
                        hwnd_tree,
                        TVM_SELECTITEM,
                        WPARAM(TVGN_CARET as usize),
                        LPARAM(next_sibling.0),
                    );
                } else if prev_exists {
                    SendMessageW(
                        hwnd_tree,
                        TVM_SELECTITEM,
                        WPARAM(TVGN_CARET as usize),
                        LPARAM(prev_sibling.0),
                    );
                } else if parent_item.0 != 0 {
                    SendMessageW(
                        hwnd_tree,
                        TVM_SELECTITEM,
                        WPARAM(TVGN_CARET as usize),
                        LPARAM(parent_item.0),
                    );
                }
                if let (Some(source_index), Some(position)) =
                    (source_idx_for_undo, removed_position)
                {
                    with_rss_state(hwnd, |s| {
                        s.removed_history.push(RssLastRemoved::Item {
                            source_index,
                            item,
                            key,
                            position,
                        });
                    });
                }
                announce_rss_status(&i18n::tr(language, "rss.article_removed"));
            }
        }
        if hwnd_tree.0 != 0 && GetFocus() != hwnd_tree {
            with_rss_state(hwnd, |s| s.suppress_focus_restore_once = true);
            SetFocus(hwnd_tree);
        }
    }
}

fn undo_last_delete(hwnd: HWND) {
    unsafe {
        let Some(last_removed) = with_rss_state(hwnd, |s| s.removed_history.pop()).flatten() else {
            return;
        };

        match last_removed {
            RssLastRemoved::Source {
                index,
                source: removed_source,
                language,
                default_removed_key_added,
            } => {
                let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                if parent.0 == 0 {
                    return;
                }
                let mut restored_index = index;
                with_state(parent, |ps| {
                    let insert_at = index.min(ps.settings.rss_sources.len());
                    restored_index = insert_at;
                    ps.settings.rss_sources.insert(insert_at, removed_source);
                    if let Some(key) = default_removed_key_added {
                        let removed_list = match language {
                            crate::settings::Language::Ukrainian
                            | crate::settings::Language::English
                            | crate::settings::Language::Lithuanian
                            | crate::settings::Language::Chinese
                            | crate::settings::Language::Russian => {
                                &mut ps.settings.rss_removed_default_en
                            }
                            crate::settings::Language::German => {
                                &mut ps.settings.rss_removed_default_de
                            }
                            crate::settings::Language::Swedish => {
                                &mut ps.settings.rss_removed_default_en
                            }
                            crate::settings::Language::Italian => {
                                &mut ps.settings.rss_removed_default_it
                            }
                            crate::settings::Language::Spanish => {
                                &mut ps.settings.rss_removed_default_es
                            }
                            crate::settings::Language::Portuguese => {
                                &mut ps.settings.rss_removed_default_pt
                            }
                            crate::settings::Language::PortugueseBrazilian => {
                                &mut ps.settings.rss_removed_default_pt_br
                            }
                            crate::settings::Language::Vietnamese => {
                                &mut ps.settings.rss_removed_default_vi
                            }
                            crate::settings::Language::Czech => {
                                &mut ps.settings.rss_removed_default_cs
                            }
                            crate::settings::Language::Polish => {
                                &mut ps.settings.rss_removed_default_pl
                            }
                            crate::settings::Language::French => {
                                &mut ps.settings.rss_removed_default_fr
                            }
                            crate::settings::Language::Serbian => {
                                &mut ps.settings.rss_removed_default_sr
                            }
                            crate::settings::Language::Hindi => {
                                &mut ps.settings.rss_removed_default_hi
                            }
                        };
                        removed_list.retain(|u| normalize_rss_url_key(u) != key);
                    }
                    save_rss_settings(ps);
                })
                .unwrap_or(());

                let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                with_rss_state(hwnd, |s| s.suppress_tree_selection_events = true);
                if hwnd_tree.0 != 0 {
                    SendMessageW(hwnd_tree, WM_SETREDRAW, WPARAM(0), LPARAM(0));
                }
                reload_tree(hwnd);
                if hwnd_tree.0 != 0 {
                    SendMessageW(hwnd_tree, WM_SETREDRAW, WPARAM(1), LPARAM(0));
                    with_rss_state(hwnd, |s| s.suppress_tree_selection_events = false);
                    if GetFocus() != hwnd_tree {
                        with_rss_state(hwnd, |s| s.suppress_focus_restore_once = true);
                        SetFocus(hwnd_tree);
                    }
                } else {
                    with_rss_state(hwnd, |s| s.suppress_tree_selection_events = false);
                }
                schedule_delayed_source_select(hwnd, restored_index);
            }
            RssLastRemoved::Folder {
                path,
                mut sources,
                folders,
                language,
                news_code,
            } => {
                let parent = with_rss_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
                if parent.0 == 0 {
                    return;
                }
                sources.sort_by_key(|(index, _source, _key)| *index);
                let default_keys = sources
                    .iter()
                    .filter_map(|(_index, _source, key)| key.clone())
                    .collect::<Vec<_>>();
                let restored_to_active = with_state(parent, |state| {
                    let active_code = active_news_language_code_from_state(state);
                    if active_code == news_code {
                        for (index, source, _default_key) in sources {
                            let insert_at = index.min(state.settings.rss_sources.len());
                            state.settings.rss_sources.insert(insert_at, source);
                        }
                    } else {
                        let bucket = state
                            .settings
                            .rss_sources_by_language
                            .entry(news_code.clone())
                            .or_default();
                        for (index, source, _default_key) in sources {
                            let insert_at = index.min(bucket.len());
                            bucket.insert(insert_at, source);
                        }
                    }
                    for key in default_keys {
                        unmark_default_rss_source_removed(&mut state.settings, language, &key);
                    }
                    let folder_bucket = state
                        .settings
                        .rss_folders_by_language
                        .entry(news_code.clone())
                        .or_default();
                    for folder in folders {
                        add_folder_path_with_parents(folder_bucket, &folder);
                    }
                    add_folder_path_with_parents(folder_bucket, &path);
                    save_rss_settings(state);
                    active_code == news_code
                })
                .unwrap_or(false);

                if restored_to_active {
                    let tree = with_rss_state(hwnd, |state| state.hwnd_tree).unwrap_or(HWND(0));
                    with_rss_state(hwnd, |state| state.suppress_tree_selection_events = true);
                    if tree.0 != 0 {
                        SendMessageW(tree, WM_SETREDRAW, WPARAM(0), LPARAM(0));
                    }
                    reload_tree(hwnd);
                    if tree.0 != 0 {
                        SendMessageW(tree, WM_SETREDRAW, WPARAM(1), LPARAM(0));
                        with_rss_state(hwnd, |state| state.suppress_tree_selection_events = false);
                        select_folder_path(hwnd, &path);
                    } else {
                        with_rss_state(hwnd, |state| state.suppress_tree_selection_events = false);
                    }
                }
                let ui_language =
                    with_state(parent, |state| state.settings.language).unwrap_or_default();
                announce_rss_status(&i18n::tr(ui_language, "rss.folder.restored"));
            }
            RssLastRemoved::Item {
                source_index,
                item,
                key,
                position,
            } => {
                let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
                if parent.0 != 0 {
                    with_state(parent, |ps| {
                        let is_favorites = ps
                            .settings
                            .rss_sources
                            .get(source_index)
                            .is_some_and(is_favorites_source);
                        if is_favorites {
                            let restored = restore_favorite_article_at(
                                &mut ps.settings.rss_favorite_articles,
                                item.clone(),
                                &key,
                                position,
                            );
                            if let Some(src) = ps.settings.rss_sources.get_mut(source_index) {
                                src.removed_item_keys.retain(|k| k != &key);
                            }
                            if restored {
                                save_rss_settings(ps);
                            }
                        } else if let Some(src) = ps.settings.rss_sources.get_mut(source_index) {
                            src.removed_item_keys.retain(|k| k != &key);
                            save_rss_settings(ps);
                        }
                    });
                }

                let source_hitem = with_rss_state(hwnd, |s| {
                    s.node_data.iter().find_map(|(&h, node)| match node {
                        NodeData::Source(i) if *i == source_index => {
                            Some(windows::Win32::UI::Controls::HTREEITEM(h))
                        }
                        _ => None,
                    })
                })
                .flatten();

                let Some(source_hitem) = source_hitem else {
                    return;
                };

                let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
                let mut show_in_tree = false;
                with_rss_state(hwnd, |s| {
                    if let Some(state) = s.source_items.get_mut(&source_hitem.0) {
                        let insert_at = position.min(state.items.len());
                        if state.loaded > 0 && insert_at <= state.loaded {
                            state.loaded += 1;
                            show_in_tree = true;
                        }
                        state.items.insert(insert_at, item.clone());
                    }
                });

                if show_in_tree && hwnd_tree.0 != 0 {
                    with_rss_state(hwnd, |s| s.suppress_tree_selection_events = true);
                    // Rebuild loaded children to preserve exact order after undo.
                    SendMessageW(hwnd_tree, WM_SETREDRAW, WPARAM(0), LPARAM(0));
                    SendMessageW(
                        hwnd_tree,
                        TVM_SELECTITEM,
                        WPARAM(TVGN_CARET as usize),
                        LPARAM(source_hitem.0),
                    );
                    loop {
                        let child = windows::Win32::UI::Controls::HTREEITEM(
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
                        with_rss_state(hwnd, |s| {
                            s.node_data.remove(&child.0);
                        });
                        SendMessageW(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(child.0));
                    }

                    let mut restored_hitem = windows::Win32::UI::Controls::HTREEITEM(0);
                    let (
                        language,
                        announce_unread,
                        unread_label_position,
                        rss_date_mode,
                        rss_time_mode,
                    ) = with_rss_state(hwnd, |s| {
                        with_state(s.parent, |ps| {
                            (
                                ps.settings.language,
                                ps.settings.announce_unread_rss_podcast_items,
                                ps.settings.rss_podcast_unread_label_position,
                                ps.settings.rss_articles_date_display,
                                ps.settings.rss_articles_time_display,
                            )
                        })
                        .unwrap_or((
                            crate::settings::Language::English,
                            true,
                            crate::settings::RssPodcastUnreadLabelPosition::Before,
                            ListDateDisplayMode::Always,
                            ListTimeDisplayMode::Always,
                        ))
                    })
                    .unwrap_or((
                        crate::settings::Language::English,
                        true,
                        crate::settings::RssPodcastUnreadLabelPosition::Before,
                        ListDateDisplayMode::Always,
                        ListTimeDisplayMode::Always,
                    ));
                    with_rss_state(hwnd, |s| {
                        if let Some(state) = s.source_items.get(&source_hitem.0) {
                            let day_counts = build_day_counts(&state.items);
                            for entry in state.items.iter().take(state.loaded) {
                                if entry.title.trim().is_empty() {
                                    continue;
                                }
                                let item_unread =
                                    !state.read_item_keys.contains(&rss_item_key(entry));
                                let title_ctx = RssItemTitleContext {
                                    language,
                                    announce_unread,
                                    unread_label_position,
                                    date_mode: rss_date_mode,
                                    time_mode: rss_time_mode,
                                };
                                let display_title = rss_item_tree_display_title(
                                    entry,
                                    item_unread,
                                    has_multiple_items_same_day(entry.pub_date, &day_counts),
                                    title_ctx,
                                );
                                let text = to_wide(&display_title);
                                let mut tvis = TVINSERTSTRUCTW {
                                    hParent: source_hitem,
                                    hInsertAfter: TVI_LAST,
                                    Anonymous: TVINSERTSTRUCTW_0 {
                                        item: TVITEMW {
                                            mask: TVIF_TEXT
                                                | TVIF_PARAM
                                                | windows::Win32::UI::Controls::TVIF_CHILDREN,
                                            pszText: windows::core::PWSTR(text.as_ptr() as *mut _),
                                            cChildren: TVITEMEXW_CHILDREN(if entry.is_folder {
                                                1
                                            } else {
                                                0
                                            }),
                                            lParam: LPARAM(0),
                                            ..Default::default()
                                        },
                                    },
                                };
                                let hchild = windows::Win32::UI::Controls::HTREEITEM(
                                    SendMessageW(
                                        hwnd_tree,
                                        TVM_INSERTITEMW,
                                        WPARAM(0),
                                        LPARAM(&mut tvis as *mut _ as isize),
                                    )
                                    .0,
                                );
                                if hchild.0 != 0 {
                                    s.node_data.insert(hchild.0, NodeData::Item(entry.clone()));
                                    if rss_item_key(entry) == key {
                                        restored_hitem = hchild;
                                    }
                                }
                            }
                        }
                    });
                    SendMessageW(hwnd_tree, WM_SETREDRAW, WPARAM(1), LPARAM(0));
                    with_rss_state(hwnd, |s| s.suppress_tree_selection_events = false);
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
                    if GetFocus() != hwnd_tree {
                        with_rss_state(hwnd, |s| s.suppress_focus_restore_once = true);
                        SetFocus(hwnd_tree);
                    }
                } else if hwnd_tree.0 != 0 {
                    SendMessageW(
                        hwnd_tree,
                        TVM_SELECTITEM,
                        WPARAM(TVGN_CARET as usize),
                        LPARAM(source_hitem.0),
                    );
                    if GetFocus() != hwnd_tree {
                        with_rss_state(hwnd, |s| s.suppress_focus_restore_once = true);
                        SetFocus(hwnd_tree);
                    }
                }
            }
        }
    }
}

fn handle_edit_source(hwnd: HWND) {
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    let hitem = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    if hitem.0 == 0 {
        return;
    }

    let source_idx = with_rss_state(hwnd, |s| match s.node_data.get(&hitem.0) {
        Some(NodeData::Source(idx)) => Some(*idx),
        _ => None,
    })
    .flatten();

    let Some(idx) = source_idx else {
        return;
    };

    let source_is_favorites = with_rss_state(hwnd, |s| {
        with_state(s.parent, |ps| {
            ps.settings
                .rss_sources
                .get(idx)
                .map(is_favorites_source)
                .unwrap_or(false)
        })
    })
    .flatten()
    .unwrap_or(false);
    if source_is_favorites {
        return;
    }

    let source_info = with_rss_state(hwnd, |s| {
        {
            with_state(s.parent, |ps| {
                ps.settings
                    .rss_sources
                    .get(idx)
                    .map(|src| (src.title.clone(), src.url.clone()))
            })
        }
        .flatten()
    })
    .flatten();
    let (title, url) = source_info.unwrap_or_default();
    if url.trim().is_empty() {
        return;
    }

    let main_hwnd = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    let existing = { with_state(main_hwnd, |s| s.rss_add_dialog) }.unwrap_or(HWND(0));
    if existing.0 != 0 {
        crate::set_foreground_window_safe(existing);
        return;
    }

    with_rss_state(hwnd, |s| s.pending_edit = Some(idx));
    show_add_dialog_with_prefill(hwnd, title, url);
}

fn handle_retry_now(hwnd: HWND) {
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    let hitem = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    if hitem.0 == 0 {
        return;
    }

    let source_info = with_rss_state(hwnd, |s| match s.node_data.get(&hitem.0) {
        Some(NodeData::Source(idx)) => {
            with_state(s.parent, |ps| {
                ps.settings.rss_sources.get(*idx).map(|src| {
                    (
                        src.url.clone(),
                        src.kind.clone(),
                        src.cache.clone(),
                        is_favorites_source(src),
                    )
                })
            })
        }
        .flatten(),
        _ => None,
    })
    .flatten();

    let Some((url, source_kind, cache, is_favorites)) = source_info else {
        return;
    };
    if is_favorites || url.trim().is_empty() {
        return;
    }

    let host = Url::parse(&url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_default();
    log_debug(&format!(
        "rss_action kind=feed action=retry_now override=true host=\"{}\"",
        host
    ));

    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    let fetch_config = if parent.0 != 0 {
        rss_fetch_config(parent)
    } else {
        rss::RssFetchConfig::default()
    };
    let language = news_language_as_app_language(&active_news_language_code(parent));
    if parent.0 != 0 {
        ensure_rss_http(parent);
    }

    let url_clone = url.clone();
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
        let res = rt.block_on(rss::fetch_and_parse(
            &url_clone,
            source_kind,
            cache,
            fetch_config,
            true,
            language,
        ));
        let msg = Box::new(FetchResult {
            hitem: hitem.0,
            result: res,
        });
        if let Err(_e) = crate::post_message_w_safe(
            hwnd,
            WM_RSS_FETCH_COMPLETE,
            WPARAM(0),
            LPARAM(Box::into_raw(msg) as isize),
        ) {}
    });
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

fn selected_source_index(hwnd: HWND) -> Option<usize> {
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return None;
    }
    let hitem = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    if hitem.0 == 0 {
        return None;
    }
    with_rss_state(hwnd, |s| match s.node_data.get(&hitem.0) {
        Some(NodeData::Source(idx)) => Some(*idx),
        _ => None,
    })
    .flatten()
}

fn source_sibling_indices(
    settings: &crate::settings::AppSettings,
    source_index: usize,
) -> Vec<usize> {
    let Some(source) = settings.rss_sources.get(source_index) else {
        return Vec::new();
    };
    let folder_path = &source.folder_path;
    settings
        .rss_sources
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            if candidate.folder_path == *folder_path {
                Some(index)
            } else {
                None
            }
        })
        .collect()
}

fn move_folder_destinations(
    settings: &crate::settings::AppSettings,
    source_index: usize,
) -> Vec<Vec<String>> {
    let Some(source) = settings.rss_sources.get(source_index) else {
        return Vec::new();
    };
    let current_path = normalized_folder_path(&source.folder_path);
    let mut destinations = Vec::new();
    if !current_path.is_empty() {
        destinations.push(Vec::new());
    }

    let mut folders = rss_folder_paths_for_settings(settings)
        .into_iter()
        .map(|path| normalized_folder_path(&path))
        .filter(|path| !path.is_empty() && *path != current_path)
        .collect::<Vec<_>>();
    folders.sort_by_key(|path| {
        path.iter()
            .map(|part| part.to_lowercase())
            .collect::<Vec<_>>()
            .join("\u{0}")
    });
    folders.dedup();
    destinations.extend(folders);
    destinations
}

fn move_rss_source_to_folder(
    settings: &mut crate::settings::AppSettings,
    source_index: usize,
    destination_path: &[String],
) -> Option<usize> {
    let destination_path = normalized_folder_path(destination_path);
    let current_path = settings
        .rss_sources
        .get(source_index)
        .map(|source| normalized_folder_path(&source.folder_path))?;
    if current_path == destination_path {
        return Some(source_index);
    }

    let mut moved_source = settings.rss_sources.remove(source_index);
    moved_source.folder_path = destination_path.clone();
    let insert_at = settings
        .rss_sources
        .iter()
        .rposition(|source| normalized_folder_path(&source.folder_path) == destination_path)
        .map(|index| index + 1)
        .unwrap_or(settings.rss_sources.len());
    settings.rss_sources.insert(insert_at, moved_source);
    Some(insert_at)
}

fn folder_destination_label(path: &[String], main_folder_label: &str) -> String {
    if path.is_empty() {
        main_folder_label.to_string()
    } else {
        path.join(" > ")
    }
}

fn reorder_rss_source_within_folder(
    settings: &mut crate::settings::AppSettings,
    source_index: usize,
    action: ReorderAction,
    target_position: usize,
) -> Option<usize> {
    let sibling_indices = source_sibling_indices(settings, source_index);
    let current_position = sibling_indices
        .iter()
        .position(|index| *index == source_index)?;
    if sibling_indices.is_empty() {
        return None;
    }
    let destination_position = match action {
        ReorderAction::Up => current_position.saturating_sub(1),
        ReorderAction::Down => (current_position + 1).min(sibling_indices.len() - 1),
        ReorderAction::Top => 0,
        ReorderAction::Bottom => sibling_indices.len() - 1,
        ReorderAction::Position => target_position.min(sibling_indices.len() - 1),
    };
    if destination_position == current_position {
        return Some(source_index);
    }

    let mut siblings = sibling_indices
        .iter()
        .filter_map(|index| settings.rss_sources.get(*index).cloned())
        .collect::<Vec<_>>();
    if siblings.len() != sibling_indices.len() {
        return None;
    }
    let moved_source = siblings.remove(current_position);
    siblings.insert(destination_position, moved_source);
    for (slot, source) in sibling_indices.iter().zip(siblings) {
        settings.rss_sources[*slot] = source;
    }
    Some(sibling_indices[destination_position])
}

fn apply_reorder_action(
    hwnd: HWND,
    source_index: usize,
    action: ReorderAction,
    target_position: usize,
) -> Option<usize> {
    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return None;
    }
    let new_index = with_state(parent, |ps| {
        let new_index = reorder_rss_source_within_folder(
            &mut ps.settings,
            source_index,
            action,
            target_position,
        );
        if new_index.is_some() {
            save_rss_settings(ps);
        }
        new_index
    })
    .flatten()?;

    reload_tree(hwnd);
    schedule_delayed_source_select(hwnd, new_index);
    Some(new_index)
}

fn handle_move_source_to_folder(hwnd: HWND, destination_index: usize) {
    let Some(source_index) = selected_source_index(hwnd) else {
        return;
    };
    let parent = with_rss_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }
    let destination = with_state(parent, |state| {
        move_folder_destinations(&state.settings, source_index)
            .get(destination_index)
            .cloned()
    })
    .flatten();
    let Some(destination) = destination else {
        return;
    };

    let new_index = with_state(parent, |state| {
        let new_index = move_rss_source_to_folder(&mut state.settings, source_index, &destination);
        if new_index.is_some() {
            save_rss_settings(state);
        }
        new_index
    })
    .flatten();
    let Some(new_index) = new_index else {
        return;
    };

    reload_tree(hwnd);
    select_source_by_index(hwnd, new_index);
}

fn handle_reorder_action(hwnd: HWND, action: ReorderAction) {
    let Some(source_index) = selected_source_index(hwnd) else {
        return;
    };
    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    let language = { with_state(parent, |ps| ps.settings.language) }.unwrap_or_default();
    let total = with_state(parent, |ps| {
        source_sibling_indices(&ps.settings, source_index).len()
    })
    .unwrap_or(0);
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
        let position = with_state(parent, |ps| {
            source_sibling_indices(&ps.settings, new_index)
                .iter()
                .position(|index| *index == new_index)
                .map(|value| value + 1)
        })
        .flatten()
        .unwrap_or(1);
        let template = i18n::tr(language, "rss.reorder.moved_position");
        let message = template.replace("{x}", &position.to_string());
        announce_rss_status(&message);
    }
}

fn handle_sort_action(hwnd: HWND, order: crate::settings::SortOrder) {
    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    {
        with_state(parent, |ps| {
            crate::settings::sort_rss_sources(&mut ps.settings, order);
            save_rss_settings(ps);
        });
        reload_tree(hwnd);
    }
}

fn rebuild_source_children_from_state(
    hwnd: HWND,
    source_hitem: windows::Win32::UI::Controls::HTREEITEM,
    select_key: &str,
) -> Option<windows::Win32::UI::Controls::HTREEITEM> {
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 || source_hitem.0 == 0 {
        return None;
    }
    let (language, announce_unread, unread_label_position, rss_date_mode, rss_time_mode) =
        with_rss_state(hwnd, |s| {
            with_state(s.parent, |ps| {
                (
                    ps.settings.language,
                    ps.settings.announce_unread_rss_podcast_items,
                    ps.settings.rss_podcast_unread_label_position,
                    ps.settings.rss_articles_date_display,
                    ps.settings.rss_articles_time_display,
                )
            })
            .unwrap_or((
                crate::settings::Language::English,
                true,
                crate::settings::RssPodcastUnreadLabelPosition::Before,
                ListDateDisplayMode::Always,
                ListTimeDisplayMode::Always,
            ))
        })
        .unwrap_or((
            crate::settings::Language::English,
            true,
            crate::settings::RssPodcastUnreadLabelPosition::Before,
            ListDateDisplayMode::Always,
            ListTimeDisplayMode::Always,
        ));

    crate::send_message_w_safe(hwnd_tree, WM_SETREDRAW, WPARAM(0), LPARAM(0));
    loop {
        let child = crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CHILD as usize),
            LPARAM(source_hitem.0),
        );
        if child.0 == 0 {
            break;
        }
        with_rss_state(hwnd, |s| {
            s.node_data.remove(&child.0);
        });
        crate::send_message_w_safe(hwnd_tree, TVM_DELETEITEM, WPARAM(0), LPARAM(child.0));
    }

    let selected_hitem = with_rss_state(hwnd, |s| {
        let mut selected_hitem = windows::Win32::UI::Controls::HTREEITEM(0);
        if let Some(state) = s.source_items.get(&source_hitem.0) {
            let day_counts = build_day_counts(&state.items);
            for entry in state.items.iter().take(state.loaded) {
                if entry.title.trim().is_empty() {
                    continue;
                }
                let item_unread = !state.read_item_keys.contains(&rss_item_key(entry));
                let title_ctx = RssItemTitleContext {
                    language,
                    announce_unread,
                    unread_label_position,
                    date_mode: rss_date_mode,
                    time_mode: rss_time_mode,
                };
                let display_title = rss_item_tree_display_title(
                    entry,
                    item_unread,
                    has_multiple_items_same_day(entry.pub_date, &day_counts),
                    title_ctx,
                );
                let text = to_wide(&display_title);
                let mut tvis = TVINSERTSTRUCTW {
                    hParent: source_hitem,
                    hInsertAfter: TVI_LAST,
                    Anonymous: TVINSERTSTRUCTW_0 {
                        item: TVITEMW {
                            mask: TVIF_TEXT
                                | TVIF_PARAM
                                | windows::Win32::UI::Controls::TVIF_CHILDREN,
                            pszText: windows::core::PWSTR(text.as_ptr() as *mut _),
                            cChildren: TVITEMEXW_CHILDREN(if entry.is_folder { 1 } else { 0 }),
                            lParam: LPARAM(0),
                            ..Default::default()
                        },
                    },
                };
                let hchild = windows::Win32::UI::Controls::HTREEITEM(
                    crate::send_message_w_safe(
                        hwnd_tree,
                        TVM_INSERTITEMW,
                        WPARAM(0),
                        LPARAM(&mut tvis as *mut _ as isize),
                    )
                    .0,
                );
                if hchild.0 != 0 {
                    s.node_data.insert(hchild.0, NodeData::Item(entry.clone()));
                    if rss_item_key(entry) == select_key {
                        selected_hitem = hchild;
                    }
                }
            }
            if state.loaded < state.items.len() {
                let text = to_wide(&rss_load_more_label(language));
                let mut tvis = TVINSERTSTRUCTW {
                    hParent: source_hitem,
                    hInsertAfter: TVI_LAST,
                    Anonymous: TVINSERTSTRUCTW_0 {
                        item: TVITEMW {
                            mask: TVIF_TEXT | TVIF_PARAM,
                            pszText: windows::core::PWSTR(text.as_ptr() as *mut _),
                            lParam: LPARAM(0),
                            ..Default::default()
                        },
                    },
                };
                let hchild = windows::Win32::UI::Controls::HTREEITEM(
                    crate::send_message_w_safe(
                        hwnd_tree,
                        TVM_INSERTITEMW,
                        WPARAM(0),
                        LPARAM(&mut tvis as *mut _ as isize),
                    )
                    .0,
                );
                if hchild.0 != 0 {
                    s.node_data.insert(hchild.0, NodeData::LoadMore);
                }
            }
        }
        selected_hitem
    });
    crate::send_message_w_safe(hwnd_tree, WM_SETREDRAW, WPARAM(1), LPARAM(0));
    selected_hitem
}

fn move_selected_article_by_one(hwnd: HWND, move_up: bool) -> bool {
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return false;
    }
    let hitem = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    if hitem.0 == 0 {
        return false;
    }
    let Some(current_item) = with_rss_state(hwnd, |s| match s.node_data.get(&hitem.0) {
        Some(NodeData::Item(item)) if !item.is_folder => Some(item.clone()),
        _ => None,
    })
    .flatten() else {
        return false;
    };
    let parent_hitem = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_PARENT as usize),
            LPARAM(hitem.0),
        )
        .0,
    );
    if parent_hitem.0 == 0 {
        return false;
    }

    let moved_key = rss_item_key(&current_item);
    if moved_key.trim().is_empty() {
        return false;
    }
    let (moved, source_index) = with_rss_state(hwnd, |s| {
        let source_index = match s.node_data.get(&parent_hitem.0) {
            Some(NodeData::Source(idx)) => Some(*idx),
            _ => None,
        };
        let Some(state) = s.source_items.get_mut(&parent_hitem.0) else {
            return (false, source_index);
        };
        let Some(cur_pos) = state
            .items
            .iter()
            .position(|it| rss_item_key(it) == moved_key)
        else {
            return (false, source_index);
        };
        if cur_pos >= state.loaded {
            return (false, source_index);
        }
        let target_pos = if move_up {
            cur_pos.saturating_sub(1)
        } else {
            cur_pos + 1
        };
        if target_pos >= state.loaded || target_pos == cur_pos {
            return (false, source_index);
        }
        state.items.swap(cur_pos, target_pos);
        (true, source_index)
    })
    .unwrap_or((false, None));
    if !moved {
        return false;
    }

    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 != 0
        && let Some(source_idx) = source_index
    {
        with_state(parent, |ps| {
            if ps
                .settings
                .rss_sources
                .get(source_idx)
                .is_some_and(is_favorites_source)
            {
                let updated = with_rss_state(hwnd, |s| {
                    s.source_items
                        .get(&parent_hitem.0)
                        .map(|state| state.items.clone())
                })
                .flatten()
                .unwrap_or_default();
                ps.settings.rss_favorite_articles = updated;
                save_rss_settings(ps);
            }
        });
    }

    if let Some(selected_hitem) = rebuild_source_children_from_state(hwnd, parent_hitem, &moved_key)
    {
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_SELECTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(selected_hitem.0),
        );
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_ENSUREVISIBLE,
            WPARAM(0),
            LPARAM(selected_hitem.0),
        );
    }
    true
}

#[derive(Clone, Copy)]
enum ArticleAction {
    OpenInBrowser,
    ShareFacebook,
    ShareTwitter,
    ShareWhatsApp,
    ShareEmail,
}

fn selected_article_item(hwnd: HWND) -> Option<RssItem> {
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return None;
    }
    let hitem = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(0),
        )
        .0,
    );
    if hitem.0 == 0 {
        return None;
    }
    with_rss_state(hwnd, |s| match s.node_data.get(&hitem.0) {
        Some(NodeData::Item(item)) => Some(item.clone()),
        _ => None,
    })
    .flatten()
}

fn restore_article_tree_focus(hwnd: HWND, hitem: windows::Win32::UI::Controls::HTREEITEM) {
    if hitem.0 == 0 {
        return;
    }
    let hwnd_tree = with_rss_state(hwnd, |s| s.hwnd_tree).unwrap_or(HWND(0));
    if hwnd_tree.0 == 0 {
        return;
    }
    let exists = with_rss_state(hwnd, |s| s.node_data.contains_key(&hitem.0)).unwrap_or(false);
    if !exists {
        return;
    }
    let parent_hitem = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_GETNEXTITEM,
            WPARAM(windows::Win32::UI::Controls::TVGN_PARENT as usize),
            LPARAM(hitem.0),
        )
        .0,
    );
    unsafe {
        // Force a real selection transition so screen readers (NVDA) announce the article again.
        if parent_hitem.0 != 0 {
            SendMessageW(
                hwnd_tree,
                TVM_SELECTITEM,
                WPARAM(TVGN_CARET as usize),
                LPARAM(parent_hitem.0),
            );
        }
        SendMessageW(
            hwnd_tree,
            TVM_SELECTITEM,
            WPARAM(TVGN_CARET as usize),
            LPARAM(hitem.0),
        );
        SendMessageW(hwnd_tree, TVM_ENSUREVISIBLE, WPARAM(0), LPARAM(hitem.0));
        if GetFocus() != hwnd_tree {
            with_rss_state(hwnd, |s| s.suppress_focus_restore_once = true);
            SetFocus(hwnd_tree);
        }
    }
}
fn insert_favorites_source_node_without_reload(
    hwnd: HWND,
) -> Option<windows::Win32::UI::Controls::HTREEITEM> {
    let (hwnd_tree, parent) =
        with_rss_state(hwnd, |s| (s.hwnd_tree, s.parent)).unwrap_or((HWND(0), HWND(0)));
    if hwnd_tree.0 == 0 || parent.0 == 0 {
        return None;
    }
    let (favorites_source, language, announce_unread, unread_label_position) =
        with_state(parent, |ps| {
            (
                ps.settings.rss_sources.first().cloned(),
                ps.settings.language,
                ps.settings.announce_unread_rss_podcast_items,
                ps.settings.rss_podcast_unread_label_position,
            )
        })
        .unwrap_or((
            None,
            crate::settings::Language::default(),
            true,
            crate::settings::RssPodcastUnreadLabelPosition::default(),
        ));
    let favorites_source = favorites_source.filter(is_favorites_source)?;

    with_rss_state(hwnd, |s| {
        for node in s.node_data.values_mut() {
            if let NodeData::Source(idx) = node {
                *idx += 1;
            }
        }
        if let Some(pending_edit) = s.pending_edit.as_mut() {
            *pending_edit += 1;
        }
        for removed in &mut s.removed_history {
            match removed {
                RssLastRemoved::Source { index, .. } => *index += 1,
                RssLastRemoved::Item { source_index, .. } => *source_index += 1,
                RssLastRemoved::Folder { sources, .. } => {
                    for (index, _source, _key) in sources {
                        *index += 1;
                    }
                }
            }
        }
    });

    let title = to_wide(&rss_source_display_title(
        &favorites_source,
        language,
        announce_unread,
        unread_label_position,
    ));
    let mut tvis = TVINSERTSTRUCTW {
        hParent: TVI_ROOT,
        hInsertAfter: TVI_FIRST,
        Anonymous: TVINSERTSTRUCTW_0 {
            item: TVITEMW {
                mask: TVIF_TEXT | TVIF_PARAM | windows::Win32::UI::Controls::TVIF_CHILDREN,
                pszText: windows::core::PWSTR(title.as_ptr() as *mut _),
                cChildren: TVITEMEXW_CHILDREN(1),
                lParam: LPARAM(0),
                ..Default::default()
            },
        },
    };
    let hitem = windows::Win32::UI::Controls::HTREEITEM(
        crate::send_message_w_safe(
            hwnd_tree,
            TVM_INSERTITEMW,
            WPARAM(0),
            LPARAM(&mut tvis as *mut _ as isize),
        )
        .0,
    );
    if hitem.0 == 0 {
        return None;
    }
    with_rss_state(hwnd, |s| {
        s.node_data.insert(hitem.0, NodeData::Source(0));
    });
    Some(hitem)
}

fn handle_add_article_to_favorites(hwnd: HWND) {
    let selected_article_hitem = with_rss_state(hwnd, |s| {
        let hwnd_tree = s.hwnd_tree;
        if hwnd_tree.0 == 0 {
            return None;
        }
        let hitem = windows::Win32::UI::Controls::HTREEITEM(
            crate::send_message_w_safe(
                hwnd_tree,
                TVM_GETNEXTITEM,
                WPARAM(TVGN_CARET as usize),
                LPARAM(0),
            )
            .0,
        );
        if hitem.0 == 0 {
            return None;
        }
        if matches!(s.node_data.get(&hitem.0), Some(NodeData::Item(_))) {
            Some(hitem)
        } else {
            None
        }
    })
    .flatten();
    let Some(mut item) = selected_article_item(hwnd) else {
        return;
    };
    let key = rss_item_key(&item);
    if key.trim().is_empty() {
        return;
    }
    item.is_folder = false;
    if item.guid.trim().is_empty() {
        item.guid = key.clone();
    }

    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    if parent.0 == 0 {
        return;
    }
    let language = with_state(parent, |ps| ps.settings.language).unwrap_or_default();

    let existed_before = with_state(parent, |ps| {
        ps.settings.rss_sources.iter().any(is_favorites_source)
    })
    .unwrap_or(false);
    let favorites_index = ensure_favorites_source(parent);
    if !existed_before && insert_favorites_source_node_without_reload(hwnd).is_none() {
        crate::log_debug("Failed to insert favorites RSS node in tree");
    }
    let mut added = false;
    let mut already_exists = false;
    with_state(parent, |ps| {
        if ps
            .settings
            .rss_favorite_articles
            .iter()
            .any(|entry| rss_item_key(entry) == key)
        {
            already_exists = true;
            return;
        }

        ps.settings.rss_favorite_articles.push(item.clone());
        sort_items_by_date_desc(&mut ps.settings.rss_favorite_articles);
        if let Some(src) = ps.settings.rss_sources.get_mut(favorites_index) {
            src.unread = true;
        }
        save_rss_settings(ps);
        added = true;
    });

    if added {
        let favorites_hitem = with_rss_state(hwnd, |s| {
            s.node_data.iter().find_map(|(h, node)| {
                if let NodeData::Source(i) = node
                    && *i == favorites_index
                {
                    Some(windows::Win32::UI::Controls::HTREEITEM(*h))
                } else {
                    None
                }
            })
        })
        .flatten();

        if let Some(hitem) = favorites_hitem {
            with_rss_state(hwnd, |s| {
                s.source_items.remove(&hitem.0);
            });
            set_source_unread(hwnd, hitem, true);
        }
        announce_rss_status(&i18n::tr(language, "rss.favorite_added"));
    } else if already_exists {
        announce_rss_status(&i18n::tr(language, "rss.favorite_already_exists"));
    }
    if let Some(hitem) = selected_article_hitem {
        restore_article_tree_focus(hwnd, hitem);
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

fn fetch_full_article_text_for_quick_copy(parent: HWND, item: &RssItem) -> String {
    if parent.0 == 0 {
        return clean_html_for_quick_copy(&item.description);
    }
    ensure_rss_http(parent);
    let language = { with_state(parent, |ps| ps.settings.language) }.unwrap_or_default();
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return clean_html_for_quick_copy(&item.description),
    };
    let text = match rt.block_on(crate::tools::rss::fetch_article_text(
        &item.link,
        &item.title,
        &item.description,
        language,
    )) {
        Ok(res) => res,
        Err(_) => item.description.clone(),
    };
    let normalized = normalize_article_text(&text);
    if normalized.trim().is_empty() {
        clean_html_for_quick_copy(&item.description)
    } else {
        normalized
    }
}

fn rss_quick_copy_text(
    parent: HWND,
    item: &RssItem,
    mode: crate::settings::RssQuickCopyMode,
) -> String {
    let title = item.title.trim();
    let url = item.link.trim();
    match mode {
        crate::settings::RssQuickCopyMode::Title => title.to_string(),
        crate::settings::RssQuickCopyMode::Url => url.to_string(),
        crate::settings::RssQuickCopyMode::Content => {
            fetch_full_article_text_for_quick_copy(parent, item)
        }
        crate::settings::RssQuickCopyMode::All => {
            let content = fetch_full_article_text_for_quick_copy(parent, item);
            let mut parts = Vec::new();
            if !title.is_empty() {
                parts.push(title.to_string());
            }
            if !url.is_empty() {
                parts.push(url.to_string());
            }
            if !content.is_empty() {
                parts.push(content);
            }
            parts.join("\r\n")
        }
    }
}

fn clean_html_for_quick_copy(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    let mut prev_space = false;

    for ch in input.chars() {
        match ch {
            '<' => {
                in_tag = true;
            }
            '>' => {
                in_tag = false;
                if !prev_space {
                    out.push(' ');
                    prev_space = true;
                }
            }
            _ if in_tag => {}
            '\r' | '\n' | '\t' => {
                if !prev_space {
                    out.push(' ');
                    prev_space = true;
                }
            }
            _ => {
                out.push(ch);
                prev_space = ch.is_whitespace();
            }
        }
    }

    normalize_article_text(out.trim())
}

fn handle_rss_quick_copy(hwnd: HWND) -> bool {
    let Some(item) = selected_article_item(hwnd) else {
        return false;
    };
    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    let mode = { with_state(parent, |ps| ps.settings.rss_quick_copy_mode) }.unwrap_or_default();
    let language = { with_state(parent, |ps| ps.settings.language) }.unwrap_or_default();
    let text = rss_quick_copy_text(parent, &item, mode);
    if text.trim().is_empty() {
        return false;
    }
    copy_text_to_clipboard(hwnd, &text);
    announce_rss_status(&i18n::tr(language, "rss.copied"));
    true
}

fn handle_article_action(hwnd: HWND, action: ArticleAction) {
    let Some(item) = selected_article_item(hwnd) else {
        log_debug("rss_action kind=article action=unavailable reason=no_article");
        return;
    };
    let url = item.link.trim().to_string();
    if !is_valid_article_url(&url) {
        log_debug("rss_action kind=article action=unavailable reason=invalid_url");
        return;
    }
    if matches!(action, ArticleAction::OpenInBrowser)
        && crate::tools::rss::is_google_news_article_url(&url)
    {
        std::thread::spawn(move || {
            let final_url = match crate::tools::rss::resolve_google_news_article_url_blocking(&url)
            {
                Ok(Some(decoded)) => decoded,
                Ok(None) => url.clone(),
                Err(err) => {
                    log_debug(&format!(
                        "rss_action kind=article action=google_news_resolve_failed error=\"{}\"",
                        err
                    ));
                    url.clone()
                }
            };
            if let Err(err) = crate::audio_utils::open_url_in_browser(&final_url) {
                log_debug(&format!(
                    "rss_action kind=article action=browser_error error=\"{}\"",
                    err
                ));
            }
        });
        return;
    }
    let title = item.title.trim().to_string();
    let language = with_rss_state(hwnd, |s| {
        { with_state(s.parent, |ps| ps.settings.language) }.unwrap_or_default()
    })
    .unwrap_or_default();
    let share_url = match action {
        ArticleAction::OpenInBrowser => {
            log_debug(&format!(
                "rss_action kind=article action=open_in_browser url=\"{}\"",
                url
            ));
            url.clone()
        }
        ArticleAction::ShareFacebook => {
            log_debug(&format!(
                "rss_action kind=article action=share_facebook url=\"{}\"",
                url
            ));
            format!(
                "https://www.facebook.com/sharer/sharer.php?u={}",
                percent_encode(&url)
            )
        }
        ArticleAction::ShareTwitter => {
            log_debug(&format!(
                "rss_action kind=article action=share_twitter url=\"{}\"",
                url
            ));
            let mut share = format!(
                "https://twitter.com/intent/tweet?url={}",
                percent_encode(&url)
            );
            if !title.is_empty() {
                share.push_str("&text=");
                share.push_str(&percent_encode(&title));
            }
            share
        }
        ArticleAction::ShareWhatsApp => {
            log_debug(&format!(
                "rss_action kind=article action=share_whatsapp url=\"{}\"",
                url
            ));
            let text = if title.is_empty() {
                url.clone()
            } else {
                format!("{}\n{}", title, url)
            };
            format!("https://wa.me/?text={}", percent_encode(&text))
        }
        ArticleAction::ShareEmail => {
            log_debug(&format!(
                "rss_action kind=article action=share_email url=\"{}\"",
                url
            ));
            let subject_raw = if title.is_empty() {
                i18n::tr(language, "rss.email.default_subject")
            } else {
                title.clone()
            };
            let subject = decode_mail_text_component(&subject_raw);
            let intro = decode_mail_text_component(&i18n::tr(language, "rss.email.body_intro"));
            let body = format!("\r\n{}\r\n{}\r\n", intro, url);
            format!(
                "mailto:?subject={}&body={}",
                mailto_encode_component(&subject),
                mailto_encode_component(&body)
            )
        }
    };
    if let Err(err) = crate::audio_utils::open_url_in_browser(&share_url) {
        log_debug(&format!(
            "rss_action kind=article action=browser_error error=\"{}\"",
            err
        ));
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
        // Re-assert focus after dialog navigation to help NVDA settle on the edit control.
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

fn import_item(hwnd: HWND, item: RssItem) {
    let url = item.link.clone();

    let parent = with_rss_state(hwnd, |s| s.parent).unwrap_or(HWND(0));
    let language = if parent.0 != 0 {
        { with_state(parent, |state| state.settings.language) }.unwrap_or_default()
    } else {
        crate::settings::Language::default()
    };
    if parent.0 != 0 {
        ensure_rss_http(parent);
    }

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
        let content_res = rt.block_on(crate::tools::rss::fetch_article_text(
            &url,
            &item.title,
            &item.description,
            language,
        ));

        let text = match content_res {
            Ok(text) => text,
            Err(err) => {
                log_debug(&format!(
                    "rss_import_fallback url=\"{}\" error=\"{}\"",
                    url, err
                ));
                format!("{}\n\n{}", item.title, url)
            }
        };
        let msg = Box::new(ImportResult { text });
        if let Err(_e) = crate::post_message_w_safe(
            hwnd,
            WM_RSS_IMPORT_COMPLETE,
            WPARAM(0),
            LPARAM(Box::into_raw(msg) as isize),
        ) {}
    });
}

struct ImportResult {
    text: String,
}

struct MarkItemReadUiMessage {
    hitem: isize,
    item_key: String,
}

/// Collapse multiple consecutive blank (or whitespace-only) lines into a single blank line.
/// This improves readability for screen-reader users and keeps the editor content compact.
fn collapse_blank_lines(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_blank = false;

    for line in input.lines() {
        let is_blank = line.trim().is_empty();

        if is_blank {
            if prev_blank {
                continue;
            }
            prev_blank = true;
            out.push('\n');
        } else {
            prev_blank = false;
            out.push_str(line);
            out.push('\n');
        }
    }

    // If the input ended without a newline, `lines()` won't tell us that.
    // Keep behavior stable by not forcing an extra newline when the output is empty.
    if out.is_empty() { String::new() } else { out }
}

fn show_reorder_dialog(parent_hwnd: HWND, source_index: usize, total: usize) {
    let existing = with_rss_state(parent_hwnd, |s| s.reorder_dialog).unwrap_or(HWND(0));
    if existing.0 != 0 {
        crate::set_foreground_window_safe(existing);
        return;
    }
    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let class_name = to_wide("SonarpadRssReorder");
    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::LoadCursorW(
                    None,
                    windows::Win32::UI::WindowsAndMessaging::IDC_ARROW,
                )
            }
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

    let language = with_rss_state(parent_hwnd, |s| {
        { with_state(s.parent, |ps| ps.settings.language) }.unwrap_or_default()
    })
    .unwrap_or_default();
    let title = i18n::tr(language, "rss.context.reorder");
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
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            360,
            160,
            parent_hwnd,
            None,
            hinstance,
            Some(init_ptr as *const _),
        )
    };
    if hwnd.0 == 0 {
        let _unused_box = crate::box_from_raw_safe(init_ptr);
        return;
    }
    with_rss_state(parent_hwnd, |s| s.reorder_dialog = hwnd);
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
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_CHAR && wparam.0 as u16 == VK_TAB.0 {
        return LRESULT(0);
    }
    if msg == WM_KEYDOWN {
        let id = crate::get_dlg_ctrl_id_safe(hwnd);
        let parent = crate::get_parent_safe(hwnd);
        let (edit_id, ok_id, cancel_id) =
            if id == REORDER_EDIT_ID || id == REORDER_OK_ID || id == REORDER_CANCEL_ID {
                (REORDER_EDIT_ID, REORDER_OK_ID, REORDER_CANCEL_ID)
            } else if id == SEARCH_EDIT_ID || id == SEARCH_OK_ID || id == SEARCH_CANCEL_ID {
                (SEARCH_EDIT_ID, SEARCH_OK_ID, SEARCH_CANCEL_ID)
            } else {
                (0, 0, 0)
            };
        if edit_id == 0 {
            let prev = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA);
            if prev == 0 {
                return crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam);
            }
            return crate::call_window_proc_w_safe(
                crate::isize_to_wndproc_safe(prev),
                hwnd,
                msg,
                wparam,
                lparam,
            );
        }
        let edit = crate::get_dlg_item_safe(parent, edit_id as i32);
        let ok = crate::get_dlg_item_safe(parent, ok_id as i32);
        let cancel = crate::get_dlg_item_safe(parent, cancel_id as i32);
        if wparam.0 as u16 == VK_TAB.0 {
            let shift = (crate::get_key_state_safe(VK_SHIFT.0 as i32) & 0x8000u16 as i16) != 0;
            let next = if shift {
                if id == edit_id {
                    cancel
                } else if id == cancel_id {
                    ok
                } else {
                    edit
                }
            } else if id == edit_id {
                ok
            } else if id == ok_id {
                cancel
            } else {
                edit
            };
            crate::set_focus_safe(next);
            return LRESULT(0);
        }
        if wparam.0 as u16 == VK_RETURN.0 {
            let target = if id == cancel_id { cancel_id } else { ok_id };
            crate::send_message_w_safe(parent, WM_COMMAND, WPARAM(target), LPARAM(0));
            return LRESULT(0);
        }
        if wparam.0 as u16 == VK_ESCAPE.0 {
            crate::send_message_w_safe(parent, WM_COMMAND, WPARAM(cancel_id), LPARAM(0));
            return LRESULT(0);
        }
    }
    let prev = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA);
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

fn show_add_dialog(parent_hwnd: HWND) {
    show_add_dialog_with_prefill(parent_hwnd, String::new(), String::new());
}

fn show_rss_search_dialog(parent_hwnd: HWND) {
    let exists = with_rss_state(parent_hwnd, |s| s.search_dialog).unwrap_or(HWND(0));
    if exists.0 != 0 {
        crate::set_foreground_window_safe(exists);
        return;
    }

    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let class_name = to_wide("SonarpadRssSearchKeyword");
    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::LoadCursorW(
                    None,
                    windows::Win32::UI::WindowsAndMessaging::IDC_ARROW,
                )
            }
            .unwrap_or_default()
            .0,
        ),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(search_keyword_wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    crate::register_class_w_safe(&wc);

    let main_hwnd = with_rss_state(parent_hwnd, |s| s.parent).unwrap_or(HWND(0));
    let language = { with_state(main_hwnd, |s| s.settings.language) }.unwrap_or_default();
    let title = i18n::tr(language, "rss.search_dialog.title");
    let init_ptr = Box::into_raw(Box::new(SearchDialogInit {
        parent: parent_hwnd,
    }));
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide(&title).as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE | WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            420,
            170,
            parent_hwnd,
            None,
            hinstance,
            Some(init_ptr as *const _),
        )
    };
    if hwnd.0 == 0 {
        let _unused_box = crate::box_from_raw_safe(init_ptr);
        return;
    }
    with_rss_state(parent_hwnd, |s| s.search_dialog = hwnd);
}

unsafe extern "system" fn search_keyword_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "search_keyword_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || search_keyword_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn search_keyword_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const CREATESTRUCTW;
                let init_ptr = (*cs).lpCreateParams as *mut SearchDialogInit;
                let parent = if init_ptr.is_null() {
                    HWND(0)
                } else {
                    let init = Box::from_raw(init_ptr);
                    init.parent
                };
                let main_hwnd = with_rss_state(parent, |s| s.parent).unwrap_or(HWND(0));
                let language = with_state(main_hwnd, |s| s.settings.language).unwrap_or_default();
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, parent.0);

                let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
                let keyword_label = i18n::tr(language, "rss.search_dialog.keyword_label");
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&keyword_label).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    10,
                    12,
                    390,
                    18,
                    hwnd,
                    HMENU(1601),
                    hinstance,
                    None,
                );
                let edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    10,
                    34,
                    390,
                    24,
                    hwnd,
                    HMENU(SEARCH_EDIT_ID as isize),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "rss.dialog.ok")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    220,
                    80,
                    85,
                    28,
                    hwnd,
                    HMENU(SEARCH_OK_ID as isize),
                    hinstance,
                    None,
                );
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "rss.dialog.cancel")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    315,
                    80,
                    85,
                    28,
                    hwnd,
                    HMENU(SEARCH_CANCEL_ID as isize),
                    hinstance,
                    None,
                );
                let ok = GetDlgItem(hwnd, SEARCH_OK_ID as i32);
                let cancel = GetDlgItem(hwnd, SEARCH_CANCEL_ID as i32);
                let proc_ptr = reorder_control_subclass_proc as *const () as usize;
                for control in [edit, ok, cancel] {
                    let prev = SetWindowLongPtrW(control, GWLP_WNDPROC, proc_ptr as isize);
                    SetWindowLongPtrW(control, GWLP_USERDATA, prev);
                }
                SetFocus(edit);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                if id == SEARCH_CANCEL_ID || id == 2 {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                if id == SEARCH_OK_ID || id == 1 {
                    let parent = HWND(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
                    let main_hwnd = with_rss_state(parent, |s| s.parent).unwrap_or(HWND(0));
                    let language =
                        with_state(main_hwnd, |s| s.settings.language).unwrap_or_default();
                    let edit = GetDlgItem(hwnd, SEARCH_EDIT_ID as i32);
                    let mut buf = vec![0u16; 1024];
                    let len =
                        windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(edit, &mut buf)
                            as usize;
                    let keyword = String::from_utf16_lossy(&buf[..len]).trim().to_string();
                    if keyword.is_empty() {
                        let title = i18n::tr(language, "rss.search_dialog.title");
                        let message = i18n::tr(language, "rss.search_dialog.empty_keyword");
                        MessageBoxW(
                            hwnd,
                            PCWSTR(to_wide(&message).as_ptr()),
                            PCWSTR(to_wide(&title).as_ptr()),
                            MB_OK | MB_ICONINFORMATION,
                        );
                        return LRESULT(0);
                    }
                    let news_language = active_news_language_code(main_hwnd);
                    let url = build_google_news_rss_url(&keyword, &news_language);
                    let source_title = format_google_news_source_title(&keyword);
                    show_add_dialog_with_prefill_options(parent, source_title, url, true);
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let parent = HWND(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
                if parent.0 != 0 {
                    with_rss_state(parent, |s| s.search_dialog = HWND(0));
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn show_add_dialog_with_prefill(parent_hwnd: HWND, title: String, url: String) {
    show_add_dialog_with_prefill_options(parent_hwnd, title, url, false);
}

fn show_add_dialog_with_prefill_options(
    parent_hwnd: HWND,
    title: String,
    url: String,
    hide_url_field: bool,
) {
    let hinstance = HINSTANCE(crate::get_module_handle_raw_default());
    let class_name = to_wide("SonarpadInput");

    let wc = WNDCLASSW {
        hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::LoadCursorW(
                    None,
                    windows::Win32::UI::WindowsAndMessaging::IDC_ARROW,
                )
            }
            .unwrap_or_default()
            .0,
        ),
        hInstance: hinstance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(input_wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
        ..Default::default()
    };
    crate::register_class_w_safe(&wc);

    let main_hwnd = with_rss_state(parent_hwnd, |s| s.parent).unwrap_or(HWND(0));
    let language = { with_state(main_hwnd, |s| s.settings.language) }.unwrap_or_default();
    let init_ptr = Box::into_raw(Box::new(AddDialogInit {
        parent: parent_hwnd,
        prefill_title: title,
        prefill_url: url,
        hide_url_field,
    }));
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide(&i18n::tr(language, "rss.add_dialog.title")).as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE | WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            400,
            190,
            parent_hwnd,
            None,
            hinstance,
            Some(init_ptr as *const _),
        )
    };
    if hwnd.0 == 0 {
        let _unused_box = crate::box_from_raw_safe(init_ptr);
        return;
    }

    let main_window = with_rss_state(parent_hwnd, |s| s.parent).unwrap_or(HWND(0));
    with_state(main_window, |s| s.rss_add_dialog = hwnd);
}

unsafe extern "system" fn input_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    crate::panic_guard::guard(
        "input_wndproc",
        || crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
        || input_wndproc_inner(hwnd, msg, wparam, lparam),
    )
}

fn input_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const CREATESTRUCTW;
                let init_ptr = (*cs).lpCreateParams as *mut AddDialogInit;
                let (parent, prefill_title, prefill_url, hide_url_field): (
                    HWND,
                    String,
                    String,
                    bool,
                ) = if init_ptr.is_null() {
                    (HWND(0), String::new(), String::new(), false)
                } else {
                    let init = Box::from_raw(init_ptr);
                    (
                        init.parent,
                        init.prefill_title,
                        init.prefill_url,
                        init.hide_url_field,
                    )
                };
                // We need language. But we can't easily pass it.
                // We can get it from parent (rss_window) -> parent (main)
                let main_hwnd = with_rss_state(parent, |s| s.parent).unwrap_or(HWND(0));
                let language = with_state(main_hwnd, |s| s.settings.language).unwrap_or_default();

                let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
                // URL label
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&i18n::tr(language, "rss.dialog.url_label")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    10,
                    10,
                    360,
                    16,
                    hwnd,
                    HMENU(105),
                    hinstance,
                    None,
                );
                // URL edit
                CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    10,
                    28,
                    360,
                    24,
                    hwnd,
                    HMENU(101),
                    hinstance,
                    None,
                );
                // Title label
                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&i18n::tr(language, "rss.dialog.title_label")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    10,
                    58,
                    360,
                    16,
                    hwnd,
                    HMENU(106),
                    hinstance,
                    None,
                );
                // Title edit
                CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    10,
                    76,
                    360,
                    24,
                    hwnd,
                    HMENU(104),
                    hinstance,
                    None,
                );
                // OK
                CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&i18n::tr(language, "rss.dialog.ok")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    180,
                    120,
                    90,
                    24,
                    hwnd,
                    HMENU(102),
                    hinstance,
                    None,
                );
                // Cancel
                CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&i18n::tr(language, "rss.dialog.cancel")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    280,
                    120,
                    90,
                    24,
                    hwnd,
                    HMENU(103),
                    hinstance,
                    None,
                );
                if !prefill_url.trim().is_empty()
                    && let Err(_e) = SetWindowTextW(
                        GetDlgItem(hwnd, 101),
                        PCWSTR(to_wide(&prefill_url).as_ptr()),
                    )
                {}
                if !prefill_title.trim().is_empty()
                    && let Err(_e) = SetWindowTextW(
                        GetDlgItem(hwnd, 104),
                        PCWSTR(to_wide(&prefill_title).as_ptr()),
                    )
                {}
                if hide_url_field {
                    ShowWindow(GetDlgItem(hwnd, 105), SW_HIDE);
                    ShowWindow(GetDlgItem(hwnd, 101), SW_HIDE);
                    SetFocus(GetDlgItem(hwnd, 104));
                } else {
                    SetFocus(GetDlgItem(hwnd, 101));
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                match id {
                    1 => {
                        // IDOK (Enter). This window is not a real DialogBox, so we map Enter to our OK button.
                        // Re-dispatch as if OK (102) was pressed.
                        SendMessageW(hwnd, WM_COMMAND, WPARAM(102), LPARAM(0));
                        LRESULT(0)
                    }
                    102 => {
                        // OK
                        let h_edit_url = GetDlgItem(hwnd, 101);
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

                        let h_edit_title = GetDlgItem(hwnd, 104);
                        let tlen = SendMessageW(
                            h_edit_title,
                            windows::Win32::UI::WindowsAndMessaging::WM_GETTEXTLENGTH,
                            WPARAM(0),
                            LPARAM(0),
                        )
                        .0;
                        let mut tbuf = vec![0u16; tlen as usize + 1];
                        SendMessageW(
                            h_edit_title,
                            windows::Win32::UI::WindowsAndMessaging::WM_GETTEXT,
                            WPARAM(tbuf.len()),
                            LPARAM(tbuf.as_mut_ptr() as isize),
                        );
                        let title = String::from_utf16_lossy(&tbuf[..tlen as usize]);

                        if !url.trim().is_empty() {
                            let parent = windows::Win32::UI::WindowsAndMessaging::GetParent(hwnd);
                            let main_hwnd = with_rss_state(parent, |s| s.parent).unwrap_or(HWND(0));
                            let mut source_url = url.trim().to_string();
                            let mut source_title = title.trim().to_string();
                            if !is_valid_article_url(&source_url) {
                                let news_language = active_news_language_code(main_hwnd);
                                source_url = build_google_news_rss_url(&source_url, &news_language);
                                if source_title.is_empty() {
                                    source_title = format_google_news_source_title(url.trim());
                                }
                            }
                            if source_title.is_empty() {
                                source_title = source_url.clone();
                            }

                            let payload = format!("{}\n{}", source_title, source_url);
                            let url_wide = to_wide(&payload);
                            let cds = COPYDATASTRUCT {
                                dwData: 0x52535331,
                                cbData: (url_wide.len() * 2) as u32,
                                lpData: url_wide.as_ptr() as *mut _,
                            };
                            SendMessageW(
                                parent,
                                windows::Win32::UI::WindowsAndMessaging::WM_COPYDATA,
                                WPARAM(hwnd.0 as usize),
                                LPARAM(&cds as *const _ as isize),
                            );
                        }
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    103 | 2 => {
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_DESTROY => {
                // We can't access AppState directly from here easily without exact parent link.
                // But we know parent is rss_window.
                // However, rss_window state has parent.
                // Let's rely on GWL_USERDATA of parent if possible?
                // Actually, when show_add_dialog creates it, it passes parent_hwnd as parent in CreateWindowEx
                let desktop = windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow();
                let parent = windows::Win32::UI::WindowsAndMessaging::GetParent(hwnd);

                if parent != desktop && parent.0 != 0 {
                    // This parent is rss_window
                    // We need to reach main window to clear rss_add_dialog
                    // rss_window stores its state in GWLP_USERDATA
                    let main_hwnd = with_rss_state(parent, |s| s.parent).unwrap_or(HWND(0));
                    if main_hwnd.0 != 0 {
                        with_state(main_hwnd, |s| s.rss_add_dialog = HWND(0));
                    }
                    with_rss_state(parent, |s| s.pending_edit = None);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
