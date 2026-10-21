use crate::accessibility::{handle_accessibility, to_wide};
use crate::sonarmusic::{self as music, Client, Page, Request, TYPES};
use crate::{i18n, with_state};
use std::sync::mpsc::{self, Receiver};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus, VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const SEARCH: usize = 101;
const SUGGEST: usize = 102;
const QUERY: usize = 103;
const KIND: usize = 104;
const RESULTS: usize = 105;
const OPEN: usize = 106;
const MORE: usize = 107;
const BACK: usize = 108;
const CLOSE: usize = 109;
const SAVE: usize = 110;
const TRANSCRIBE: usize = 111;
const AD: usize = 112;
enum Reply {
    Page(Request, Page, bool),
    Suggestions(Vec<String>),
}
struct State {
    client: Client,
    locale: String,
    query: HWND,
    kind: HWND,
    results: HWND,
    status: HWND,
    suggest: HWND,
    controls: Vec<HWND>,
    page: Page,
    request: Option<Request>,
    page_number: usize,
    history: Vec<(Request, Page, usize, usize)>,
    suggestions: Option<Vec<String>>,
    receiver: Option<Receiver<Result<Reply, String>>>,
    selected_action: Option<(String, String, usize)>,
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
    fill(s.results, s.page.items.iter().map(|i| i.display(&s.locale)));
    unsafe {
        SendMessageW(s.results, CB_SETCURSEL, WPARAM(index), LPARAM(0));
    }
    update(hwnd, s);
}
fn update(hwnd: HWND, s: &State) {
    unsafe {
        let busy = s.receiver.is_some();
        for h in &s.controls {
            EnableWindow(*h, !busy);
        }
        EnableWindow(
            GetDlgItem(hwnd, MORE as i32),
            !busy && s.page.next.is_some() && s.suggestions.is_none(),
        );
        EnableWindow(
            GetDlgItem(hwnd, BACK as i32),
            !busy && (!s.history.is_empty() || s.suggestions.is_some()),
        );
        let item = s.page.items.get(selected(s.results));
        let playable = s.suggestions.is_none() && item.is_some_and(|i| i.playable());
        for id in [SAVE, TRANSCRIBE, AD] {
            EnableWindow(GetDlgItem(hwnd, id as i32), !busy && playable);
        }
        EnableWindow(
            GetDlgItem(hwnd, OPEN as i32),
            !busy
                && (s.suggestions.as_ref().is_some_and(|v| !v.is_empty())
                    || item.is_some_and(|i| i.playable() || i.browse_request().is_some())),
        );
    }
}
fn load(hwnd: HWND, s: &mut State, request: Request, more: bool) {
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
                WM_COMMAND => {
                    let id = w.0 & 0xffff;
                    let notification = (w.0 >> 16) as u32;
                    if id == QUERY && notification == EN_CHANGE {
                        let visible = !text(s.query).trim().is_empty();
                        if IsWindowVisible(s.suggest).as_bool() != visible {
                            ShowWindow(s.suggest, if visible { SW_SHOW } else { SW_HIDE });
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
                        MORE => {
                            if let Some(r) = s.request.clone() {
                                load(hwnd, s, r, true);
                            }
                        }
                        BACK => {
                            if s.suggestions.take().is_some() {
                                render(hwnd, s, 0);
                            } else if let Some((r, p, n, index)) = s.history.pop() {
                                s.request = Some(r);
                                s.page = p;
                                s.page_number = n;
                                render(hwnd, s, index);
                            }
                            SetFocus(s.results);
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
                            } else if let Some(item) = s.page.items.get(index).cloned() {
                                if let Some(url) = item.watch_url() {
                                    let action = match id {
                                        SAVE => 1,
                                        TRANSCRIBE => 2,
                                        AD => 3,
                                        _ => 0,
                                    };
                                    s.selected_action = Some((url, item.title, action));
                                    let _ignored = DestroyWindow(hwnd);
                                } else if let Some(r) = item.browse_request() {
                                    if let Some(current) = s.request.clone() {
                                        s.history.push((
                                            current,
                                            s.page.clone(),
                                            s.page_number,
                                            index,
                                        ));
                                    }
                                    load(hwnd, s, r, false);
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
                                    s.page.append(p);
                                    s.page_number += 1;
                                } else {
                                    s.page = p;
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
                                SetFocus(s.results);
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
                                SetFocus(s.results);
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
            440,
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
        let search = make(
            w!("BUTTON"),
            &t("search"),
            SEARCH,
            480,
            40,
            115,
            28,
            WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
        );
        let suggest = make(
            w!("BUTTON"),
            &t("suggest"),
            SUGGEST,
            605,
            40,
            125,
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
        make(
            w!("STATIC"),
            &t("results"),
            0,
            15,
            145,
            680,
            22,
            WINDOW_STYLE(0),
        );
        let results = make(
            w!("COMBOBOX"),
            "",
            RESULTS,
            15,
            170,
            710,
            240,
            WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
        );
        let status = make(w!("STATIC"), "", 0, 15, 210, 710, 40, WINDOW_STYLE(0));
        let mut controls = vec![query, search, suggest, kind, results];
        for (id, key, x, width) in [
            (OPEN, t("open"), 15, 140),
            (
                SAVE,
                i18n::tr(language, "playback.download_episode"),
                165,
                155,
            ),
            (
                TRANSCRIBE,
                i18n::tr(language, "playback.transcribe_current"),
                330,
                155,
            ),
            (
                AD,
                i18n::tr(language, "menu.create_audio_description"),
                495,
                235,
            ),
        ] {
            controls.push(make(w!("BUTTON"), &key, id, x, 260, width, 35, WS_TABSTOP));
        }
        for (id, key, x) in [
            (BACK, "back", 15),
            (MORE, "more", 210),
            (CLOSE, "close", 535),
        ] {
            let h = make(w!("BUTTON"), &t(key), id, x, 315, 190, 35, WS_TABSTOP);
            if id != CLOSE {
                controls.push(h);
            }
        }
        let mut state = State {
            client,
            locale,
            query,
            kind,
            results,
            status,
            suggest,
            controls,
            page: Page::default(),
            request: None,
            page_number: 0,
            history: Vec::new(),
            suggestions: None,
            receiver: None,
            selected_action: None,
        };
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut state as *mut _ as isize);
        update(hwnd, &state);
        SetTimer(hwnd, 1, 100, None);
        EnableWindow(parent, false);
        SetFocus(query);
        let mut msg = MSG::default();
        while IsWindow(hwnd).as_bool() && GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&msg) {
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 == VK_ESCAPE.0 as usize {
                let _ignored = DestroyWindow(hwnd);
                continue;
            }
            if msg.message == WM_KEYDOWN
                && msg.wParam.0 == VK_RETURN.0 as usize
                && (msg.hwnd == query || msg.hwnd == results)
            {
                let id = if msg.hwnd == query { SEARCH } else { OPEN };
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
        if let Some((url, title, action)) = state.selected_action
            && let Err(e) =
                super::youtube_transcript_window::sonarmusic_action(parent, &url, &title, action)
        {
            crate::show_error(parent, language, &e);
        }
    }
}
