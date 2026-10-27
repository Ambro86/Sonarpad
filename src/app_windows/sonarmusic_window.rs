use crate::accessibility::{handle_accessibility, to_wide};
use crate::sonarmusic::{self as music, Client, Page, Pagination, Request, ResultRow, TYPES};
use crate::{i18n, with_state};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, IsWindowEnabled, SetFocus, VK_APPS, VK_CONTROL, VK_ESCAPE,
    VK_F10, VK_RETURN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const WM_RESTORE_MUSIC_FOCUS: u32 = WM_APP + 211;
const SEARCH: usize = 101;
const SUGGEST: usize = 102;
const QUERY: usize = 103;
const KIND: usize = 104;
const RESULTS: usize = 105;
const OPEN: usize = 106;
const MORE: usize = 107;
const PREVIOUS: usize = 113;
const BACK: usize = 108;
const CLOSE: usize = 109;
const SAVE: usize = 110;
const TRANSCRIBE: usize = 111;
const AD: usize = 112;
const FAVORITES: usize = 114;
const OPEN_FAVORITE: usize = 115;
const ADD_FAVORITE: usize = 116;
const REMOVE_FAVORITE: usize = 117;
const REMOVE_SAVED: usize = 118;
const COPY_LINK_RESULT: usize = 119;
const COPY_LINK_FAVORITE: usize = 120;
#[derive(Clone, Default)]
struct SearchSnapshot {
    query: String,
    kind: usize,
    page: Page,
    request: Option<Request>,
    page_number: usize,
    pagination: Pagination,
    history: Vec<(Option<Request>, Page, usize, usize, Pagination)>,
    selection: usize,
}
struct PlayerReturn {
    parent: isize,
    url: String,
    search: SearchSnapshot,
}
static PLAYER_RETURN: Mutex<Option<PlayerReturn>> = Mutex::new(None);

fn take_return_context(
    stored: &mut Option<PlayerReturn>,
    parent: isize,
    url: Option<&str>,
) -> Option<SearchSnapshot> {
    if stored
        .as_ref()
        .is_some_and(|v| v.parent == parent && Some(v.url.as_str()) == url)
    {
        stored.take().map(|v| v.search)
    } else {
        None
    }
}

pub(crate) fn restore_after_player_stop(parent: HWND, url: Option<&str>) -> bool {
    let snapshot = take_return_context(
        &mut PLAYER_RETURN.lock().unwrap_or_else(|e| e.into_inner()),
        parent.0,
        url,
    );
    if let Some(snapshot) = snapshot {
        crate::log_debug(
            "SonarMusic: restoring cached results and selected track after player stop",
        );
        open_with_snapshot(parent, Some(snapshot));
        true
    } else {
        false
    }
}

enum Reply {
    Page(Request, Page, bool),
    Suggestions(Vec<String>),
}
struct State {
    parent: HWND,
    client: Client,
    locale: String,
    query: HWND,
    kind: HWND,
    results: HWND,
    status: HWND,
    favorites_combo: HWND,
    last_focus: HWND,
    favorites: Vec<music::Item>,
    suggest: HWND,
    controls: Vec<HWND>,
    page: Page,
    request: Option<Request>,
    page_number: usize,
    pagination: Pagination,
    history: Vec<(Option<Request>, Page, usize, usize, Pagination)>,
    suggestions: Option<Vec<String>>,
    receiver: Option<Receiver<Result<Reply, String>>>,
    selected_action: Option<(String, String, usize)>,
    playback_snapshot: Option<SearchSnapshot>,
}
fn remember_focus(hwnd: HWND, state: &mut State) {
    unsafe {
        let mut focus = GetFocus();
        // Remember the combo itself, not its transient internal edit/list window.
        while focus.0 != 0 && IsChild(hwnd, focus).as_bool() {
            if GetWindowLongW(focus, GWL_STYLE) as u32 & WS_TABSTOP.0 != 0 {
                state.last_focus = focus;
                break;
            }
            focus = GetParent(focus);
        }
    }
}

fn restore_focus(hwnd: HWND, state: &State) {
    unsafe {
        if GetForegroundWindow() != hwnd || !IsWindowEnabled(hwnd).as_bool() {
            return;
        }
        let target = [
            state.last_focus,
            state.query,
            GetDlgItem(hwnd, CLOSE as i32),
        ]
        .into_iter()
        .find(|&control| {
            control.0 != 0
                && IsChild(hwnd, control).as_bool()
                && IsWindowEnabled(control).as_bool()
                && IsWindowVisible(control).as_bool()
        });
        if let Some(target) = target {
            SetFocus(target);
            crate::log_debug(&format!(
                "SonarMusic focus restored: control={}",
                GetDlgCtrlID(target)
            ));
        }
    }
}

fn text(hwnd: HWND) -> String {
    unsafe {
        let mut b = vec![0u16; GetWindowTextLengthW(hwnd) as usize + 1];
        let n = GetWindowTextW(hwnd, &mut b);
        String::from_utf16_lossy(&b[..n as usize])
    }
}
fn set(hwnd: HWND, value: &str) {
    unsafe {
        let _ignored = SetWindowTextW(hwnd, PCWSTR(to_wide(value).as_ptr()));
    }
}
fn selected(hwnd: HWND) -> usize {
    unsafe {
        SendMessageW(hwnd, CB_GETCURSEL, WPARAM(0), LPARAM(0))
            .0
            .max(0) as usize
    }
}
fn fill(hwnd: HWND, values: impl Iterator<Item = String>) {
    unsafe {
        SendMessageW(hwnd, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        for value in values {
            SendMessageW(
                hwnd,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&value).as_ptr() as isize),
            );
        }
        SendMessageW(hwnd, CB_SETCURSEL, WPARAM(0), LPARAM(0));
    }
}
fn label(s: &State, key: &str) -> String {
    music::label(&s.locale, key)
}
fn render(hwnd: HWND, s: &State, index: usize) {
    fill(
        s.results,
        crate::result_labels::accessible_result_labels(s.pagination.labels(&s.page, &s.locale))
            .into_iter(),
    );
    unsafe {
        SendMessageW(s.results, CB_SETCURSEL, WPARAM(index), LPARAM(0));
    }
    update(hwnd, s);
}
fn focus_results(s: &State) {
    unsafe {
        SendMessageW(s.results, CB_SHOWDROPDOWN, WPARAM(0), LPARAM(0));
        SetFocus(s.results);
        // Reusing the same combo after an album load also needs a value/focus
        // notification: SetFocus alone may leave the screen reader on the old row.
        NotifyWinEvent(0x800E, s.results, -4, 0);
        NotifyWinEvent(0x8005, s.results, -4, 0);
    }
    crate::log_debug(&format!(
        "SonarMusic results focused: page={} index={} label={}",
        s.page_number,
        selected(s.results),
        text(s.results)
    ));
}
fn update(hwnd: HWND, s: &State) {
    unsafe {
        let busy = s.receiver.is_some();
        for h in &s.controls {
            EnableWindow(*h, !busy);
        }
        EnableWindow(
            GetDlgItem(hwnd, BACK as i32),
            !busy
                && s.pagination
                    .can_return(s.suggestions.is_some(), !s.history.is_empty()),
        );
    }
}
fn refresh_favorites(s: &State, selected_index: usize) {
    let labels = crate::result_labels::accessible_result_labels(
        s.favorites
            .iter()
            .map(|item| item.display(&s.locale))
            .collect(),
    );
    fill(s.favorites_combo, labels.into_iter());
    if !s.favorites.is_empty() {
        unsafe {
            SendMessageW(
                s.favorites_combo,
                CB_SETCURSEL,
                WPARAM(selected_index.min(s.favorites.len() - 1)),
                LPARAM(0),
            );
        }
    }
}
fn persist_favorites(s: &State) {
    if let Some(settings) = with_state(s.parent, |state| {
        state.settings.sonarmusic_favorites =
            s.favorites.iter().map(music::favorite_value).collect();
        state.settings.clone()
    }) {
        crate::settings::save_settings(settings);
    }
}
fn selected_item(s: &State) -> Option<&music::Item> {
    if s.suggestions.is_some() {
        return None;
    }
    match s.pagination.row(&s.page, selected(s.results)) {
        Some(ResultRow::Item(i)) => s.page.items.get(i),
        _ => None,
    }
}
fn selected_share_url(s: &State, favorite: bool) -> Option<String> {
    if s.receiver.is_some() {
        return None;
    }
    if favorite {
        s.favorites.get(selected(s.favorites_combo))?.share_url()
    } else {
        selected_item(s)?.share_url()
    }
}

fn result_actions(s: &State) -> Vec<usize> {
    if s.receiver.is_some() {
        return Vec::new();
    }
    if let Some(suggestions) = &s.suggestions {
        return if suggestions.get(selected(s.results)).is_some() {
            vec![OPEN]
        } else {
            Vec::new()
        };
    }
    let index = match s.pagination.row(&s.page, selected(s.results)) {
        Some(ResultRow::Previous) => return vec![PREVIOUS],
        Some(ResultRow::Next) => return vec![MORE],
        Some(ResultRow::Item(index)) => index,
        None => return Vec::new(),
    };
    match s.page.items.get(index) {
        Some(item) if item.playable() => vec![
            OPEN,
            SAVE,
            TRANSCRIBE,
            AD,
            COPY_LINK_RESULT,
            if s.favorites.iter().any(|f| music::same_favorite(f, item)) {
                REMOVE_FAVORITE
            } else {
                ADD_FAVORITE
            },
        ],
        Some(item) if item.browse_request().is_some() => vec![
            OPEN,
            COPY_LINK_RESULT,
            if s.favorites.iter().any(|f| music::same_favorite(f, item)) {
                REMOVE_FAVORITE
            } else {
                ADD_FAVORITE
            },
        ],
        _ => Vec::new(),
    }
}
fn show_result_menu(hwnd: HWND, s: &State, position: LPARAM, favorites: bool) {
    let actions = if favorites {
        if s.receiver.is_none() && s.favorites.get(selected(s.favorites_combo)).is_some() {
            vec![OPEN_FAVORITE, COPY_LINK_FAVORITE, REMOVE_SAVED]
        } else {
            Vec::new()
        }
    } else {
        result_actions(s)
    };
    if actions.is_empty() {
        return;
    }
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        // This is an owned top-level window, not a child: GetParent can be null.
        let language = with_state(s.parent, |state| state.settings.language).unwrap_or_default();
        for action in actions {
            let title = match action {
                ADD_FAVORITE => label(s, "add_favorite"),
                REMOVE_FAVORITE | REMOVE_SAVED => label(s, "remove_favorite"),
                OPEN_FAVORITE => label(s, "open_favorite"),
                MORE => label(s, "next_results"),
                PREVIOUS => label(s, "previous_results"),
                SAVE => i18n::tr(language, "playback.download_episode"),
                TRANSCRIBE => i18n::tr(language, "playback.transcribe_current"),
                AD => i18n::tr(language, "menu.create_audio_description"),
                COPY_LINK_RESULT | COPY_LINK_FAVORITE => {
                    format!("{} (Ctrl+C)", i18n::tr(language, "stream_audio.copy_link"))
                }
                _ => label(s, "open"),
            };
            crate::log_if_err!(AppendMenuW(
                menu,
                MF_STRING,
                action,
                PCWSTR(to_wide(&title).as_ptr())
            ));
        }
        let (x, y) = if position.0 == -1 {
            let mut rect = RECT::default();
            crate::log_if_err!(GetWindowRect(
                if favorites {
                    s.favorites_combo
                } else {
                    s.results
                },
                &mut rect
            ));
            (rect.left, rect.bottom)
        } else {
            (position.0 as i16 as i32, (position.0 >> 16) as i16 as i32)
        };
        SendMessageW(s.results, CB_SHOWDROPDOWN, WPARAM(0), LPARAM(0));
        if favorites {
            SetFocus(s.favorites_combo);
        } else {
            focus_results(s);
        }
        let command =
            TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, x, y, 0, hwnd, None).0 as usize;
        crate::log_if_err!(DestroyMenu(menu));
        if command != 0 {
            crate::log_if_err!(PostMessageW(hwnd, WM_COMMAND, WPARAM(command), LPARAM(0)));
        }
    }
}
fn load(hwnd: HWND, s: &mut State, request: Request, more: bool) {
    unsafe {
        SendMessageW(s.results, CB_SHOWDROPDOWN, WPARAM(0), LPARAM(0));
    }
    let client = s.client.clone();
    let token = if more { s.page.next.clone() } else { None };
    let page = if more { s.page_number + 1 } else { 1 };
    let (tx, rx) = mpsc::channel();
    s.receiver = Some(rx);
    set(s.status, &label(s, "loading"));
    update(hwnd, s);
    std::thread::spawn(move || {
        let result = client
            .load(&request, token.as_deref(), page)
            .map(|p| Reply::Page(request, p, more));
        let _ignored = tx.send(result);
    });
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    crate::panic_guard::guard(
        "sonarmusic",
        || crate::def_window_proc_w_safe(hwnd, msg, w, l),
        || unsafe {
            if msg == WM_CLOSE {
                let _ignored = DestroyWindow(hwnd);
                return LRESULT(0);
            }
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
            if ptr.is_null() {
                return DefWindowProcW(hwnd, msg, w, l);
            }
            let s = &mut *ptr;
            match msg {
                WM_ACTIVATE => {
                    if w.0 & 0xffff == WA_INACTIVE as usize {
                        remember_focus(hwnd, s);
                    } else {
                        let _ignored =
                            PostMessageW(hwnd, WM_RESTORE_MUSIC_FOCUS, WPARAM(0), LPARAM(0));
                    }
                    DefWindowProcW(hwnd, msg, w, l)
                }
                WM_SETFOCUS => {
                    let _ignored = PostMessageW(hwnd, WM_RESTORE_MUSIC_FOCUS, WPARAM(0), LPARAM(0));
                    LRESULT(0)
                }
                WM_RESTORE_MUSIC_FOCUS => {
                    restore_focus(hwnd, s);
                    LRESULT(0)
                }
                WM_CONTEXTMENU => {
                    if HWND(w.0 as isize) == s.results || HWND(w.0 as isize) == s.favorites_combo {
                        show_result_menu(hwnd, s, l, HWND(w.0 as isize) == s.favorites_combo);
                        return LRESULT(0);
                    }
                    DefWindowProcW(hwnd, msg, w, l)
                }
                WM_COMMAND => {
                    let id = w.0 & 0xffff;
                    let notification = (w.0 >> 16) as u32;
                    if id == QUERY && notification == EN_CHANGE {
                        let visible = !text(s.query).trim().is_empty();
                        if IsWindowVisible(s.suggest).as_bool() != visible {
                            ShowWindow(s.suggest, if visible { SW_SHOW } else { SW_HIDE });
                        }
                    }
                    if id == KIND && notification == CBN_SELCHANGE {
                        let kind = TYPES[selected(s.kind).min(TYPES.len() - 1)].to_owned();
                        if let Some(settings) = with_state(s.parent, |state| {
                            state.settings.sonarmusic_search_type = kind;
                            state.settings.clone()
                        }) {
                            crate::log_debug(&format!(
                                "SonarMusic search type saved: {}",
                                settings.sonarmusic_search_type
                            ));
                            crate::settings::save_settings(settings);
                        }
                    }
                    if id == RESULTS && notification == CBN_SELCHANGE {
                        update(hwnd, s);
                    }
                    if notification != BN_CLICKED {
                        return LRESULT(0);
                    }
                    if id == CLOSE {
                        let _ignored = DestroyWindow(hwnd);
                        return LRESULT(0);
                    }
                    if s.receiver.is_some() {
                        return LRESULT(0);
                    }
                    match id {
                        COPY_LINK_RESULT | COPY_LINK_FAVORITE => {
                            if let Some(url) = selected_share_url(s, id == COPY_LINK_FAVORITE) {
                                crate::app_windows::rai_audiodescrizioni_window::copy_text_to_clipboard(hwnd, &url);
                                let language =
                                    with_state(s.parent, |state| state.settings.language)
                                        .unwrap_or_default();
                                set(
                                    s.status,
                                    &format!(
                                        "{}: {}",
                                        i18n::tr(language, "stream_audio.copy_link"),
                                        url
                                    ),
                                );
                            }
                        }
                        ADD_FAVORITE | REMOVE_FAVORITE | REMOVE_SAVED => {
                            let item = if id == REMOVE_SAVED {
                                s.favorites.get(selected(s.favorites_combo)).cloned()
                            } else {
                                selected_item(s).cloned()
                            };
                            if let Some(item) = item {
                                let index = selected(s.favorites_combo);
                                let mut values: Vec<_> =
                                    s.favorites.iter().map(music::favorite_value).collect();
                                if id == ADD_FAVORITE {
                                    music::add_favorite(&mut values, &item);
                                } else {
                                    music::remove_favorite(&mut values, &item);
                                }
                                s.favorites = music::favorite_items(&values);
                                persist_favorites(s);
                                refresh_favorites(s, index);
                                set(
                                    s.status,
                                    &format!("{}: {}", label(s, "favorites"), s.favorites.len()),
                                );
                            }
                        }
                        OPEN_FAVORITE => {
                            let index = selected(s.favorites_combo);
                            if index < s.favorites.len() {
                                s.history.push((
                                    s.request.clone(),
                                    s.page.clone(),
                                    s.page_number,
                                    selected(s.results),
                                    s.pagination.clone(),
                                ));
                                s.page = Page {
                                    items: s.favorites.clone(),
                                    next: None,
                                };
                                s.request = None;
                                s.page_number = 1;
                                s.pagination = Pagination::default();
                                s.suggestions = None;
                                render(hwnd, s, index);
                                let kind = TYPES[selected(s.kind).min(TYPES.len() - 1)];
                                if let Some(request) = s.favorites[index].favorite_search(kind) {
                                    set(s.query, &s.favorites[index].title);
                                    load(hwnd, s, request, false);
                                } else {
                                    PostMessageW(hwnd, WM_COMMAND, WPARAM(OPEN), LPARAM(0)).ok();
                                }
                            }
                        }
                        SEARCH => {
                            if let Ok(r) = Request::search(
                                &text(s.query),
                                TYPES[selected(s.kind).min(TYPES.len() - 1)],
                            ) {
                                s.history.clear();
                                s.suggestions = None;
                                load(hwnd, s, r, false);
                            }
                        }
                        SUGGEST => {
                            let query = text(s.query);
                            if !query.trim().is_empty() {
                                let client = s.client.clone();
                                let (tx, rx) = mpsc::channel();
                                s.receiver = Some(rx);
                                set(s.status, &label(s, "loading"));
                                update(hwnd, s);
                                std::thread::spawn(move || {
                                    let _ignored =
                                        tx.send(client.suggestions(&query).map(Reply::Suggestions));
                                });
                            }
                        }
                        PREVIOUS => {
                            if s.pagination.back(&mut s.page) {
                                s.page_number = s.page_number.saturating_sub(1);
                                render(hwnd, s, 0);
                                focus_results(s);
                            }
                        }
                        MORE => {
                            if let Some(r) = s.request.clone() {
                                load(hwnd, s, r, true);
                            }
                        }
                        BACK => {
                            if s.suggestions.take().is_some() {
                                render(hwnd, s, 0);
                            } else if s.pagination.back(&mut s.page) {
                                s.page_number = s.page_number.saturating_sub(1);
                                render(hwnd, s, 0);
                            } else if let Some((r, p, n, index, pagination)) = s.history.pop() {
                                s.request = r;
                                s.page = p;
                                s.page_number = n;
                                s.pagination = pagination;
                                render(hwnd, s, index);
                            }
                            focus_results(s);
                        }
                        OPEN | SAVE | TRANSCRIBE | AD => {
                            let index = selected(s.results);
                            if let Some(suggestions) = s.suggestions.as_ref() {
                                if let Some(value) = suggestions.get(index).cloned() {
                                    set(s.query, &value);
                                    SetFocus(s.query);
                                    s.suggestions = None;
                                    render(hwnd, s, 0);
                                }
                            } else {
                                let item_index = match s.pagination.row(&s.page, index) {
                                    Some(ResultRow::Previous) => {
                                        crate::log_if_err!(PostMessageW(
                                            hwnd,
                                            WM_COMMAND,
                                            WPARAM(PREVIOUS),
                                            LPARAM(0)
                                        ));
                                        return LRESULT(0);
                                    }
                                    Some(ResultRow::Next) => {
                                        crate::log_if_err!(PostMessageW(
                                            hwnd,
                                            WM_COMMAND,
                                            WPARAM(MORE),
                                            LPARAM(0)
                                        ));
                                        return LRESULT(0);
                                    }
                                    Some(ResultRow::Item(i)) => i,
                                    None => return LRESULT(0),
                                };
                                if let Some(item) = s.page.items.get(item_index).cloned() {
                                    if let Some(url) = item.watch_url() {
                                        let action = match id {
                                            SAVE => 1,
                                            TRANSCRIBE => 2,
                                            AD => 3,
                                            _ => 0,
                                        };
                                        if action == 0 {
                                            s.playback_snapshot = Some(SearchSnapshot {
                                                query: text(s.query),
                                                kind: selected(s.kind),
                                                page: s.page.clone(),
                                                request: s.request.clone(),
                                                page_number: s.page_number,
                                                pagination: s.pagination.clone(),
                                                history: s.history.clone(),
                                                selection: index,
                                            });
                                        }
                                        s.selected_action = Some((url, item.title, action));
                                        let _ignored = DestroyWindow(hwnd);
                                    } else if let Some(r) = item.browse_request() {
                                        {
                                            s.history.push((
                                                s.request.clone(),
                                                s.page.clone(),
                                                s.page_number,
                                                index,
                                                s.pagination.clone(),
                                            ));
                                        }
                                        load(hwnd, s, r, false);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    LRESULT(0)
                }
                WM_TIMER => {
                    let reply = s.receiver.as_ref().and_then(|r| r.try_recv().ok());
                    if let Some(reply) = reply {
                        s.receiver = None;
                        match reply {
                            Ok(Reply::Page(r, p, more)) => {
                                s.request = Some(r);
                                s.suggestions = None;
                                if more {
                                    s.pagination.advance(&mut s.page, p);
                                    s.page_number += 1;
                                } else {
                                    s.page = p;
                                    s.pagination = Pagination::default();
                                    s.page_number = 1;
                                }
                                render(hwnd, s, 0);
                                set(
                                    s.status,
                                    &if s.page.items.is_empty() {
                                        label(s, "empty")
                                    } else {
                                        format!("{}: {}", label(s, "results"), s.page.items.len())
                                    },
                                );
                                focus_results(s);
                            }
                            Ok(Reply::Suggestions(v)) => {
                                fill(s.results, v.iter().cloned());
                                set(
                                    s.status,
                                    &if v.is_empty() {
                                        label(s, "empty")
                                    } else {
                                        label(s, "suggest")
                                    },
                                );
                                s.suggestions = Some(v);
                                update(hwnd, s);
                                focus_results(s);
                            }
                            Err(e) => {
                                crate::log_debug(&e);
                                set(s.status, &e);
                                update(hwnd, s);
                                SetFocus(s.query);
                            }
                        }
                    }
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, w, l),
            }
        },
    )
}
pub fn open(parent: HWND) {
    open_with_snapshot(parent, None);
}
fn open_with_snapshot(parent: HWND, snapshot: Option<SearchSnapshot>) {
    unsafe {
        let language = with_state(parent, |s| s.settings.language).unwrap_or_default();
        let locale = serde_json::to_value(language)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "en".into());
        let client = match Client::new(
            &locale,
            option_env!("SONARPAD_ROUTE_CLIENT_TOKEN").unwrap_or(""),
        ) {
            Ok(c) => c,
            Err(e) => {
                crate::show_error(parent, language, &e);
                return;
            }
        };
        let instance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class = w!("SonarpadMusic");
        let wc = WNDCLASSW {
            hInstance: instance,
            lpszClassName: class,
            lpfnWndProc: Some(wndproc),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
            class,
            w!("SonarMusic"),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            200,
            180,
            760,
            485,
            parent,
            HMENU(0),
            instance,
            None,
        );
        if hwnd.0 == 0 {
            return;
        }
        let make =
            |class: PCWSTR, value: &str, id: usize, x, y, width, height, style: WINDOW_STYLE| {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    class,
                    PCWSTR(to_wide(value).as_ptr()),
                    WS_CHILD | WS_VISIBLE | style,
                    x,
                    y,
                    width,
                    height,
                    hwnd,
                    HMENU(id as isize),
                    instance,
                    None,
                )
            };
        let t = |key| music::label(&locale, key);
        make(
            w!("STATIC"),
            &t("query"),
            0,
            15,
            15,
            680,
            22,
            WINDOW_STYLE(0),
        );
        let query = make(
            w!("EDIT"),
            "",
            QUERY,
            15,
            40,
            450,
            25,
            WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
        );
        make(
            w!("STATIC"),
            &t("favorites"),
            0,
            15,
            245,
            710,
            22,
            WINDOW_STYLE(0),
        );
        let favorites_combo = make(
            w!("COMBOBOX"),
            "",
            FAVORITES,
            15,
            270,
            710,
            220,
            WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
        );
        let suggest = make(
            w!("BUTTON"),
            &t("suggest"),
            SUGGEST,
            480,
            40,
            245,
            28,
            WS_TABSTOP,
        );
        ShowWindow(suggest, SW_HIDE);
        make(
            w!("STATIC"),
            &t("type"),
            0,
            15,
            80,
            680,
            22,
            WINDOW_STYLE(0),
        );
        let kind = make(
            w!("COMBOBOX"),
            "",
            KIND,
            15,
            105,
            710,
            220,
            WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
        );
        fill(kind, TYPES.iter().map(|k| t(k)));
        let saved_kind = with_state(parent, |state| {
            state.settings.sonarmusic_search_type.clone()
        })
        .unwrap_or_default();
        crate::log_debug(&format!("SonarMusic search type restored: {saved_kind}"));
        SendMessageW(
            kind,
            CB_SETCURSEL,
            WPARAM(music::search_type_index(&saved_kind)),
            LPARAM(0),
        );
        let search = make(
            w!("BUTTON"),
            &t("search"),
            SEARCH,
            15,
            140,
            115,
            28,
            WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
        );
        make(
            w!("STATIC"),
            &t("results"),
            0,
            15,
            180,
            680,
            22,
            WINDOW_STYLE(0),
        );
        let results = make(
            w!("COMBOBOX"),
            "",
            RESULTS,
            15,
            205,
            710,
            240,
            WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
        );
        let favorites = with_state(parent, |state| {
            music::favorite_items(&state.settings.sonarmusic_favorites)
        })
        .unwrap_or_default();
        let status = make(w!("STATIC"), "", 0, 15, 310, 710, 40, WINDOW_STYLE(0));
        let mut controls = vec![query, search, suggest, kind, results, favorites_combo];
        for (id, key, x) in [(BACK, "back", 15), (CLOSE, "close", 535)] {
            let h = make(w!("BUTTON"), &t(key), id, x, 365, 190, 35, WS_TABSTOP);
            if id != CLOSE {
                controls.push(h);
            }
        }
        let mut state = State {
            parent,
            client,
            locale,
            query,
            kind,
            results,
            status,
            favorites_combo,
            last_focus: query,
            favorites,
            suggest,
            controls,
            page: Page::default(),
            request: None,
            page_number: 0,
            pagination: Pagination::default(),
            history: Vec::new(),
            suggestions: None,
            receiver: None,
            selected_action: None,
            playback_snapshot: None,
        };
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut state as *mut _ as isize);
        let restoring = snapshot.is_some();
        if let Some(snapshot) = snapshot {
            set(query, &snapshot.query);
            SendMessageW(kind, CB_SETCURSEL, WPARAM(snapshot.kind), LPARAM(0));
            state.page = snapshot.page;
            state.request = snapshot.request;
            state.page_number = snapshot.page_number;
            state.pagination = snapshot.pagination;
            state.history = snapshot.history;
            render(hwnd, &state, snapshot.selection);
            set(
                state.status,
                &format!("{}: {}", label(&state, "results"), state.page.items.len()),
            );
        }
        refresh_favorites(&state, 0);
        update(hwnd, &state);
        SetTimer(hwnd, 1, 100, None);
        EnableWindow(parent, false);
        SetForegroundWindow(hwnd);
        if restoring {
            focus_results(&state);
        } else {
            SetFocus(query);
        }
        remember_focus(hwnd, &mut state);
        let mut msg = MSG::default();
        while IsWindow(hwnd).as_bool() && GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
            remember_focus(hwnd, &mut state);
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&msg) {
                continue;
            }
            if msg.message == WM_KEYDOWN
                && msg.wParam.0 == b'C' as usize
                && GetKeyState(VK_CONTROL.0 as i32) < 0
                && (msg.hwnd == results || msg.hwnd == favorites_combo)
            {
                let id = if msg.hwnd == favorites_combo {
                    COPY_LINK_FAVORITE
                } else {
                    COPY_LINK_RESULT
                };
                SendMessageW(hwnd, WM_COMMAND, WPARAM(id), LPARAM(0));
                continue;
            }
            if msg.message == WM_KEYDOWN
                && (GetFocus() == results || GetFocus() == favorites_combo)
                && (msg.wParam.0 == VK_APPS.0 as usize
                    || (msg.wParam.0 == VK_F10.0 as usize && GetKeyState(VK_SHIFT.0 as i32) < 0))
            {
                SendMessageW(
                    hwnd,
                    WM_CONTEXTMENU,
                    WPARAM(GetFocus().0 as usize),
                    LPARAM(-1),
                );
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 == VK_ESCAPE.0 as usize {
                if state
                    .pagination
                    .can_return(state.suggestions.is_some(), !state.history.is_empty())
                {
                    // Discard a pending album response if Escape returns before it finishes.
                    state.receiver = None;
                    SendMessageW(hwnd, WM_COMMAND, WPARAM(BACK), LPARAM(0));
                } else {
                    let _ignored = DestroyWindow(hwnd);
                }
                continue;
            }
            if msg.message == WM_KEYDOWN
                && msg.wParam.0 == VK_RETURN.0 as usize
                && (msg.hwnd == query || msg.hwnd == results || msg.hwnd == favorites_combo)
            {
                let id = if msg.hwnd == query {
                    SEARCH
                } else if msg.hwnd == favorites_combo {
                    OPEN_FAVORITE
                } else {
                    OPEN
                };
                SendMessageW(hwnd, WM_COMMAND, WPARAM(id), LPARAM(0));
                continue;
            }
            if crate::handle_focused_edit_shortcut(&msg) {
                continue;
            }
            if !handle_accessibility(hwnd, &msg) && !IsDialogMessageW(hwnd, &msg).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        EnableWindow(parent, true);
        crate::restore_editor_focus(parent);
        if let Some((url, title, action)) = state.selected_action {
            match super::youtube_transcript_window::sonarmusic_action(parent, &url, &title, action)
            {
                Ok(()) => {
                    if let Some(search) = state.playback_snapshot {
                        *PLAYER_RETURN.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(PlayerReturn {
                                parent: parent.0,
                                url,
                                search,
                            });
                    }
                }
                Err(e) => crate::show_error(parent, language, &e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn return_context_preserves_search_page_and_selection_once() {
        let mut stored = Some(PlayerReturn {
            parent: 42,
            url: "track".into(),
            search: SearchSnapshot {
                query: "Pink Floyd".into(),
                kind: 2,
                page_number: 3,
                selection: 7,
                ..SearchSnapshot::default()
            },
        });
        assert!(take_return_context(&mut stored, 41, Some("track")).is_none());
        assert!(take_return_context(&mut stored, 42, Some("other")).is_none());
        assert!(take_return_context(&mut stored, 42, None).is_none());
        let Some(search) = take_return_context(&mut stored, 42, Some("track")) else {
            panic!("missing context")
        };
        assert_eq!(
            (
                search.query.as_str(),
                search.kind,
                search.page_number,
                search.selection
            ),
            ("Pink Floyd", 2, 3, 7)
        );
        assert!(take_return_context(&mut stored, 42, Some("track")).is_none());
    }
}
