//! Global bookmark manager: unlike bookmarks_window, this displays every saved file.
//! All list entries retain their original file key and bookmark index so filtering
//! and sorting can never delete a different bookmark.
use super::youtube_transcript_window::{
    playlist_download_insert_entry, playlist_download_selected_indices,
    playlist_download_set_entry_checked,
};
use crate::accessibility::{handle_accessibility, to_wide};
use crate::bookmarks::{BookmarkStore, remove_selected_bookmarks};
use crate::i18n;
use crate::settings::Language;
use crate::with_state;
use chrono::{Duration, Local, NaiveDateTime};
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    LIST_VIEW_ITEM_STATE_FLAGS, LVCF_WIDTH, LVCOLUMNW, LVIS_FOCUSED, LVIS_SELECTED, LVITEMW,
    LVM_DELETEALLITEMS, LVM_ENSUREVISIBLE, LVM_GETNEXTITEM, LVM_INSERTCOLUMNW,
    LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETITEMSTATE, LVNI_FOCUSED, NM_DBLCLK, NMHDR, WC_BUTTON,
    WC_COMBOBOX, WC_EDIT, WC_LISTVIEWW, WC_STATIC,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_DELETE, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL,
    CBN_SELCHANGE, CBS_DROPDOWNLIST, CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    EN_CHANGE, GWLP_USERDATA, GetWindowTextLengthW, GetWindowTextW, HMENU, IDC_ARROW, IDCANCEL,
    IDYES, LoadCursorW, MB_ICONWARNING, MB_YESNO, MSG, RegisterClassW, SetForegroundWindow,
    SetWindowTextW, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_KEYDOWN,
    WM_NCDESTROY, WM_NOTIFY, WM_SETFONT, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_CONTROLPARENT,
    WS_EX_DLGMODALFRAME, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::PCWSTR;

const CLASS: &str = "SonarpadAllBookmarks";
const SEARCH: usize = 9401;
const LIST: usize = 9402;
const SORT: usize = 9403;
const AGE: usize = 9404;
const AUTOMATIC: usize = 9405;
const OPEN: usize = 9406;
const DELETE: usize = 9407;
const CLOSE: usize = 9408;
const COUNT: usize = 9409;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    key: String,
    index: usize,
    position: i32,
    updated: String,
    opened: Option<String>,
    automatic: bool,
    snippet: String,
    missing: bool,
}

struct State {
    parent: HWND,
    language: Language,
    list: HWND,
    search: HWND,
    sort: HWND,
    age: HWND,
    automatic: HWND,
    count: HWND,
    rows: Vec<Row>,
}

fn tr(language: Language, key: &str) -> String {
    i18n::tr(language, key)
}

fn is_local_file(key: &str) -> bool {
    // Titles of remote streams and legacy unsaved buffers must not be mistaken for files.
    !key.starts_with("__") && !key.contains("://") && Path::new(key).is_absolute()
}

fn display_name(key: &str) -> String {
    Path::new(key)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(key)
        .to_string()
}

fn rows_for_store(store: &BookmarkStore) -> Vec<Row> {
    let mut rows = Vec::new();
    for (key, bookmarks) in &store.files {
        let missing = is_local_file(key) && !Path::new(key).is_file();
        for (index, bm) in bookmarks.iter().enumerate() {
            rows.push(Row {
                key: key.clone(),
                index,
                position: bm.position,
                updated: bm.timestamp.clone(),
                opened: bm.last_opened.clone(),
                automatic: bm.automatic,
                snippet: bm.snippet.clone(),
                missing,
            });
        }
    }
    rows
}

fn parse_date(date: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S").ok()
}

fn visible_rows(
    store: &BookmarkStore,
    query: &str,
    sort: usize,
    age: usize,
    automatic: bool,
) -> Vec<Row> {
    let query = query.to_lowercase();
    let now = Local::now().naive_local();
    let days = match age {
        1 => Some(30),
        2 => Some(60),
        3 => Some(90),
        4 => Some(180),
        5 => Some(365),
        _ => None,
    };
    let mut rows: Vec<_> = rows_for_store(store)
        .into_iter()
        .filter(|row| {
            if automatic && !row.automatic {
                return false;
            }
            if !query.is_empty()
                && !row.key.to_lowercase().contains(&query)
                && !row.snippet.to_lowercase().contains(&query)
            {
                return false;
            }
            if age == 6 {
                return row.opened.as_deref().and_then(parse_date).is_none();
            }
            if age == 7 {
                return row.missing;
            }
            if let Some(days) = days {
                return row
                    .opened
                    .as_deref()
                    .and_then(parse_date)
                    .is_some_and(|date| date <= now - Duration::days(days));
            }
            true
        })
        .collect();
    match sort {
        1 => rows.sort_by(|a, b| {
            a.key
                .to_lowercase()
                .cmp(&b.key.to_lowercase())
                .then(a.position.cmp(&b.position))
        }),
        2 => rows.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.key.cmp(&b.key))),
        _ => rows.sort_by(|a, b| b.opened.cmp(&a.opened).then(a.key.cmp(&b.key))),
    }
    rows
}

fn focused_index(list: HWND) -> Option<usize> {
    let index = crate::send_message_w_safe(
        list,
        LVM_GETNEXTITEM,
        WPARAM(usize::MAX),
        LPARAM(LVNI_FOCUSED as isize),
    )
    .0;
    usize::try_from(index).ok()
}

fn focus_row(list: HWND, index: usize, count: usize) {
    if count == 0 {
        return;
    }
    let index = index.min(count - 1);
    let mut item = LVITEMW {
        state: LIST_VIEW_ITEM_STATE_FLAGS(LVIS_SELECTED.0 | LVIS_FOCUSED.0),
        stateMask: LIST_VIEW_ITEM_STATE_FLAGS(LVIS_SELECTED.0 | LVIS_FOCUSED.0),
        ..Default::default()
    };
    crate::send_message_w_safe(
        list,
        LVM_SETITEMSTATE,
        WPARAM(index),
        LPARAM((&mut item as *mut LVITEMW) as isize),
    );
    crate::send_message_w_safe(list, LVM_ENSUREVISIBLE, WPARAM(index), LPARAM(0));
}

fn control_text(hwnd: HWND) -> String {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return String::new();
    }
    let mut text = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut text) };
    String::from_utf16_lossy(&text[..copied.max(0) as usize])
}

fn choice(hwnd: HWND) -> usize {
    crate::send_message_w_safe(hwnd, CB_GETCURSEL, WPARAM(0), LPARAM(0))
        .0
        .max(0) as usize
}

fn state<R>(hwnd: HWND, f: impl FnOnce(&mut State) -> R) -> Option<R> {
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut State;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn refresh(hwnd: HWND) {
    let Some((parent, list, query, sort, age, automatic, count_hwnd, language)) =
        state(hwnd, |s| {
            (
                s.parent,
                s.list,
                control_text(s.search),
                choice(s.sort),
                choice(s.age),
                crate::send_message_w_safe(s.automatic, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1,
                s.count,
                s.language,
            )
        })
    else {
        return;
    };
    let rows = with_state(parent, |app| {
        visible_rows(&app.bookmarks, &query, sort, age, automatic)
    })
    .unwrap_or_default();
    let previous_index = focused_index(list).unwrap_or(0);
    crate::send_message_w_safe(list, LVM_DELETEALLITEMS, WPARAM(0), LPARAM(0));
    for (index, row) in rows.iter().enumerate() {
        let opened = row
            .opened
            .clone()
            .unwrap_or_else(|| tr(language, "bookmark_manager.unknown"));
        let kind = if row.automatic {
            tr(language, "bookmark_manager.auto")
        } else {
            tr(language, "bookmark_manager.manual")
        };
        let status = if row.missing {
            tr(language, "bookmark_manager.missing")
        } else {
            String::new()
        };
        let label = format!(
            "{} | {} | {}: {} | {}: {} | {}: {} | {} | {} | {}",
            display_name(&row.key),
            row.key,
            tr(language, "bookmark_manager.position"),
            row.position,
            tr(language, "bookmark_manager.opened"),
            opened,
            tr(language, "bookmark_manager.updated"),
            row.updated,
            kind,
            status,
            row.snippet
        );
        playlist_download_insert_entry(list, index, &label);
    }
    let count_text = format!("{}: {}", tr(language, "bookmark_manager.count"), rows.len());
    unsafe {
        crate::log_if_err!(SetWindowTextW(
            count_hwnd,
            PCWSTR(to_wide(&count_text).as_ptr())
        ));
    }
    focus_row(list, previous_index, rows.len());
    state(hwnd, |s| s.rows = rows);
}

fn alert(hwnd: HWND, language: Language, key: &str) {
    let text = tr(language, key);
    let title = tr(language, "bookmark_manager.title");
    crate::message_box_w_safe(
        hwnd,
        PCWSTR(to_wide(&text).as_ptr()),
        PCWSTR(to_wide(&title).as_ptr()),
        windows::Win32::UI::WindowsAndMessaging::MB_OK,
    );
}

fn open_selected(hwnd: HWND) {
    let Some((parent, language, chosen)) = state(hwnd, |s| {
        let index = focused_index(s.list);
        (
            s.parent,
            s.language,
            index.and_then(|i| s.rows.get(i).cloned()),
        )
    }) else {
        return;
    };
    let Some(row) = chosen else {
        return;
    };
    if !is_local_file(&row.key) || row.missing {
        alert(hwnd, language, "bookmark_manager.cannot_open");
        return;
    }
    let path = PathBuf::from(&row.key);
    let existing = with_state(parent, |app| {
        app.docs.iter().enumerate().find_map(|(index, doc)| {
            doc.path
                .as_deref()
                .filter(|p| p.to_string_lossy().eq_ignore_ascii_case(&row.key))
                .map(|_| (index, doc.format))
        })
    })
    .flatten();
    if let Some((index, format)) = existing {
        crate::editor_manager::select_tab(parent, index);
        crate::record_bookmark_open(parent, &row.key);
        if matches!(format, crate::settings::FileFormat::Audiobook) {
            crate::jump_audiobook_to_position(parent, &path, row.position.max(0) as u64);
        } else {
            crate::editor_manager::go_to_current_document_utf16_offset(parent, row.position);
        }
    } else {
        if !crate::file_handler::is_audio_path(&path) {
            with_state(parent, |app| {
                app.pending_global_bookmark = Some((path.clone(), row.position))
            });
        }
        crate::editor_manager::open_document(parent, &path);
    }
    crate::log_if_err!(crate::destroy_window_safe(hwnd));
}

fn delete_selected(hwnd: HWND) {
    let Some((parent, language, rows)) = state(hwnd, |s| {
        let selected = playlist_download_selected_indices(s.list)
            .into_iter()
            .filter_map(|i| s.rows.get(i).cloned())
            .collect::<Vec<_>>();
        (s.parent, s.language, selected)
    }) else {
        return;
    };
    if rows.is_empty() {
        crate::accessibility::screen_reader_speak(&tr(
            language,
            "bookmark_manager.select_at_least_one",
        ));
        return;
    }
    let prompt = format!(
        "{} ({})",
        tr(language, "bookmark_manager.confirm_delete"),
        rows.len()
    );
    let title = tr(language, "bookmark_manager.title");
    if crate::message_box_w_safe(
        hwnd,
        PCWSTR(to_wide(&prompt).as_ptr()),
        PCWSTR(to_wide(&title).as_ptr()),
        MB_YESNO | MB_ICONWARNING,
    ) != IDYES
    {
        return;
    }
    let selected: Vec<_> = rows
        .iter()
        .map(|row| (row.key.clone(), row.index))
        .collect();
    let outcome = with_state(parent, |app| {
        let mut updated = BookmarkStore {
            files: app.bookmarks.files.clone(),
        };
        let removed = remove_selected_bookmarks(&mut updated, &selected);
        if removed == 0 {
            return Ok(0);
        }
        crate::bookmarks::backup_before_cleanup()?;
        crate::bookmarks::try_save_bookmarks(&updated)?;
        app.bookmarks = updated;
        // An already-open document must not recreate a bookmark when it closes.
        for row in &rows {
            if row.automatic
                && app.docs.iter().any(|doc| {
                    doc.path
                        .as_ref()
                        .is_some_and(|p| p.to_string_lossy().eq_ignore_ascii_case(&row.key))
                })
            {
                app.bookmark_cleanup_suppressed.insert(row.key.clone());
            }
        }
        Ok::<usize, std::io::Error>(removed)
    });
    match outcome {
        Some(Ok(_)) => {
            refresh(hwnd);
            if let Some((list, count)) = state(hwnd, |s| (s.list, s.rows.len())) {
                focus_row(list, 0, count);
                crate::set_focus_safe(list);
            }
        }
        _ => alert(hwnd, language, "bookmark_manager.save_failed"),
    }
}

struct ControlPlacement {
    id: usize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

fn add_control(
    parent: HWND,
    class: PCWSTR,
    label: &str,
    style: WINDOW_STYLE,
    placement: ControlPlacement,
    font: HFONT,
) -> HWND {
    let ControlPlacement {
        id,
        x,
        y,
        width,
        height,
    } = placement;
    let hwnd = unsafe {
        CreateWindowExW(
            Default::default(),
            class,
            PCWSTR(to_wide(label).as_ptr()),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            width,
            height,
            parent,
            HMENU(id as isize),
            HINSTANCE(0),
            None,
        )
    };
    if hwnd.0 != 0 && font.0 != 0 {
        crate::send_message_w_safe(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    }
    hwnd
}

fn window_proc_inner(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let parent = crate::hwnd_from_create_struct_lparam_safe(lp.0 as *const CREATESTRUCTW);
            let language = with_state(parent, |s| s.settings.language).unwrap_or_default();
            let font = with_state(parent, |s| s.hfont).unwrap_or(HFONT(0));
            add_control(
                hwnd,
                WC_STATIC,
                &tr(language, "bookmark_manager.search"),
                WINDOW_STYLE(0),
                ControlPlacement {
                    id: 0,
                    x: 12,
                    y: 12,
                    width: 470,
                    height: 20,
                },
                font,
            );
            let search = add_control(
                hwnd,
                WC_EDIT,
                "",
                WS_TABSTOP,
                ControlPlacement {
                    id: SEARCH,
                    x: 12,
                    y: 36,
                    width: 480,
                    height: 25,
                },
                font,
            );
            add_control(
                hwnd,
                WC_STATIC,
                &tr(language, "bookmark_manager.sort"),
                WINDOW_STYLE(0),
                ControlPlacement {
                    id: 0,
                    x: 512,
                    y: 12,
                    width: 240,
                    height: 20,
                },
                font,
            );
            let sort = add_control(
                hwnd,
                WC_COMBOBOX,
                "",
                WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                ControlPlacement {
                    id: SORT,
                    x: 512,
                    y: 36,
                    width: 278,
                    height: 180,
                },
                font,
            );
            for key in [
                "bookmark_manager.sort_opened",
                "bookmark_manager.sort_name",
                "bookmark_manager.sort_updated",
            ] {
                let label = to_wide(&tr(language, key));
                crate::send_message_w_safe(
                    sort,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(label.as_ptr() as isize),
                );
            }
            crate::send_message_w_safe(sort, CB_SETCURSEL, WPARAM(0), LPARAM(0));
            add_control(
                hwnd,
                WC_STATIC,
                &tr(language, "bookmark_manager.older"),
                WINDOW_STYLE(0),
                ControlPlacement {
                    id: 0,
                    x: 12,
                    y: 71,
                    width: 260,
                    height: 20,
                },
                font,
            );
            let age = add_control(
                hwnd,
                WC_COMBOBOX,
                "",
                WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                ControlPlacement {
                    id: AGE,
                    x: 12,
                    y: 93,
                    width: 260,
                    height: 180,
                },
                font,
            );
            for key in [
                "bookmark_manager.all",
                "bookmark_manager.days30",
                "bookmark_manager.days60",
                "bookmark_manager.days90",
                "bookmark_manager.days180",
                "bookmark_manager.days365",
                "bookmark_manager.unknown",
                "bookmark_manager.missing_only",
            ] {
                let label = to_wide(&tr(language, key));
                crate::send_message_w_safe(
                    age,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(label.as_ptr() as isize),
                );
            }
            crate::send_message_w_safe(age, CB_SETCURSEL, WPARAM(0), LPARAM(0));
            let automatic = add_control(
                hwnd,
                WC_BUTTON,
                &tr(language, "bookmark_manager.only_auto"),
                WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                ControlPlacement {
                    id: AUTOMATIC,
                    x: 298,
                    y: 94,
                    width: 470,
                    height: 27,
                },
                font,
            );
            let list = add_control(
                hwnd,
                WC_LISTVIEWW,
                "",
                WS_TABSTOP | WS_VSCROLL | WINDOW_STYLE(0x0001 | 0x0004 | 0x0008 | 0x4000),
                ControlPlacement {
                    id: LIST,
                    x: 12,
                    y: 140,
                    width: 778,
                    height: 270,
                },
                font,
            );
            crate::send_message_w_safe(
                list,
                LVM_SETEXTENDEDLISTVIEWSTYLE,
                WPARAM(0x0000_0004 | 0x0000_0020),
                LPARAM(0x0000_0004 | 0x0000_0020),
            );
            let mut column = LVCOLUMNW {
                mask: LVCF_WIDTH,
                cx: 748,
                ..Default::default()
            };
            crate::send_message_w_safe(
                list,
                LVM_INSERTCOLUMNW,
                WPARAM(0),
                LPARAM((&mut column as *mut LVCOLUMNW) as isize),
            );
            let count = add_control(
                hwnd,
                WC_STATIC,
                "",
                WINDOW_STYLE(0),
                ControlPlacement {
                    id: COUNT,
                    x: 12,
                    y: 419,
                    width: 770,
                    height: 22,
                },
                font,
            );
            add_control(
                hwnd,
                WC_BUTTON,
                &tr(language, "bookmarks.goto"),
                WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                ControlPlacement {
                    id: OPEN,
                    x: 12,
                    y: 448,
                    width: 130,
                    height: 30,
                },
                font,
            );
            add_control(
                hwnd,
                WC_BUTTON,
                &tr(language, "bookmarks.delete"),
                WS_TABSTOP,
                ControlPlacement {
                    id: DELETE,
                    x: 153,
                    y: 448,
                    width: 130,
                    height: 30,
                },
                font,
            );
            add_control(
                hwnd,
                WC_BUTTON,
                &tr(language, "bookmark_manager.close"),
                WS_TABSTOP,
                ControlPlacement {
                    id: CLOSE,
                    x: 660,
                    y: 448,
                    width: 130,
                    height: 30,
                },
                font,
            );
            let data = Box::new(State {
                parent,
                language,
                list,
                search,
                sort,
                age,
                automatic,
                count,
                rows: Vec::new(),
            });
            crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, Box::into_raw(data) as isize);
            refresh(hwnd);
            crate::set_focus_safe(list);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wp.0 & 0xffff;
            let notification = (wp.0 >> 16) as u16;
            match id {
                OPEN => {
                    open_selected(hwnd);
                    LRESULT(0)
                }
                DELETE => {
                    delete_selected(hwnd);
                    LRESULT(0)
                }
                CLOSE => {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    LRESULT(0)
                }
                SEARCH if notification == EN_CHANGE as u16 => {
                    refresh(hwnd);
                    LRESULT(0)
                }
                SORT | AGE if notification == CBN_SELCHANGE as u16 => {
                    refresh(hwnd);
                    LRESULT(0)
                }
                AUTOMATIC => {
                    refresh(hwnd);
                    LRESULT(0)
                }
                cmd if cmd == IDCANCEL.0 as usize => {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    LRESULT(0)
                }
                _ => crate::def_window_proc_w_safe(hwnd, msg, wp, lp),
            }
        }
        WM_NOTIFY => {
            if lp.0 != 0 {
                let notification = unsafe { &*(lp.0 as *const NMHDR) };
                if notification.idFrom == LIST && notification.code == NM_DBLCLK {
                    open_selected(hwnd);
                    return LRESULT(0);
                }
            }
            crate::def_window_proc_w_safe(hwnd, msg, wp, lp)
        }
        WM_CLOSE => {
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            LRESULT(0)
        }
        WM_DESTROY => {
            if let Some(parent) = state(hwnd, |s| s.parent) {
                crate::enable_window_safe(parent, true);
                with_state(parent, |s| s.bookmark_manager_window = HWND(0));
                crate::set_foreground_window_safe(parent);
                if let Some(edit) = crate::get_active_edit(parent) {
                    crate::set_focus_safe(edit);
                }
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut State;
            if !ptr.is_null() {
                let _dropped = crate::box_from_raw_safe(ptr);
            }
            LRESULT(0)
        }
        _ => crate::def_window_proc_w_safe(hwnd, msg, wp, lp),
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "bookmark_manager_window",
            || DefWindowProcW(hwnd, msg, wp, lp),
            || window_proc_inner(hwnd, msg, wp, lp),
        )
    }
}

pub fn handle_navigation(hwnd: HWND, msg: &MSG) -> bool {
    if msg.message == WM_KEYDOWN {
        let list = state(hwnd, |s| s.list).unwrap_or(HWND(0));
        if crate::get_focus_safe() == list {
            if msg.wParam.0 as u32 == VK_RETURN.0 as u32 {
                open_selected(hwnd);
                return true;
            }
            if msg.wParam.0 as u32 == VK_DELETE.0 as u32 {
                delete_selected(hwnd);
                return true;
            }
            if msg.wParam.0 as u32 == u32::from(b'A')
                && unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0
            {
                let count = state(hwnd, |s| s.rows.len()).unwrap_or(0);
                for index in 0..count {
                    playlist_download_set_entry_checked(list, index, true);
                }
                return true;
            }
        }
    }
    handle_accessibility(hwnd, msg)
}

pub fn open(parent: HWND) {
    unsafe {
        let existing = with_state(parent, |s| s.bookmark_manager_window).unwrap_or(HWND(0));
        if existing.0 != 0 {
            SetForegroundWindow(existing);
            return;
        }
        let instance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class = to_wide(CLASS);
        let wc = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
            ),
            hInstance: instance,
            lpszClassName: PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(window_proc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let language = with_state(parent, |s| s.settings.language).unwrap_or_default();
        let title = to_wide(&tr(language, "bookmark_manager.title"));
        let window = CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            820,
            525,
            parent,
            None,
            instance,
            Some(parent.0 as *const std::ffi::c_void),
        );
        if window.0 != 0 {
            with_state(parent, |s| s.bookmark_manager_window = window);
            crate::enable_window_safe(parent, false);
            SetForegroundWindow(window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_local_file, visible_rows};
    use crate::bookmarks::{Bookmark, BookmarkStore};
    #[test]
    fn native_checkboxes_and_arrow_navigation_after_rebuild() {
        use super::*;
        use windows::Win32::UI::Controls::{
            ICC_LISTVIEW_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
        };
        use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DOWN, VK_UP};
        unsafe {
            assert!(
                InitCommonControlsEx(&INITCOMMONCONTROLSEX {
                    dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                    dwICC: ICC_LISTVIEW_CLASSES,
                })
                .as_bool()
            );
        }
        let list = unsafe {
            CreateWindowExW(
                Default::default(),
                WC_LISTVIEWW,
                PCWSTR::null(),
                WINDOW_STYLE(0x0001 | 0x0004 | 0x0008 | 0x4000),
                0,
                0,
                400,
                200,
                HWND(0),
                None,
                HINSTANCE(0),
                None,
            )
        };
        assert_ne!(list.0, 0);
        struct Cleanup(HWND);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                crate::log_if_err!(crate::destroy_window_safe(self.0));
            }
        }
        let _cleanup = Cleanup(list);
        crate::send_message_w_safe(list, LVM_SETEXTENDEDLISTVIEWSTYLE, WPARAM(4), LPARAM(4));
        for index in 0..3 {
            playlist_download_insert_entry(list, index, "Bookmark");
        }
        focus_row(list, 0, 3);
        assert!(playlist_download_selected_indices(list).is_empty());
        playlist_download_set_entry_checked(list, 1, true);
        playlist_download_set_entry_checked(list, 2, true);
        assert_eq!(playlist_download_selected_indices(list), vec![1, 2]);
        // The highlighted row used by Open is independent of checked rows to delete.
        assert_eq!(focused_index(list), Some(0));
        crate::send_message_w_safe(list, LVM_DELETEALLITEMS, WPARAM(0), LPARAM(0));
        for index in 0..2 {
            playlist_download_insert_entry(list, index, "Remaining bookmark");
        }
        focus_row(list, 2, 2);
        assert_eq!(focused_index(list), Some(1));
        assert!(playlist_download_selected_indices(list).is_empty());
        crate::send_message_w_safe(list, WM_KEYDOWN, WPARAM(VK_UP.0 as usize), LPARAM(0));
        assert_eq!(focused_index(list), Some(0));
        crate::send_message_w_safe(list, WM_KEYDOWN, WPARAM(VK_DOWN.0 as usize), LPARAM(0));
        assert_eq!(focused_index(list), Some(1));
        crate::send_message_w_safe(list, LVM_DELETEALLITEMS, WPARAM(0), LPARAM(0));
        focus_row(list, 1, 0);
        assert_eq!(focused_index(list), None);
    }
    #[test]
    fn filter_preserves_original_indices_across_files() {
        let mut store = BookmarkStore::default();
        store.files.insert(
            "C:\\story.txt".into(),
            vec![
                Bookmark::manual(1, "Alice".into(), "2026-10-01 10:00:00".into()),
                Bookmark::automatic(9, "Bob".into(), "2026-10-10 10:00:00".into()),
            ],
        );
        store.files.insert(
            "C:\\other.txt".into(),
            vec![Bookmark::manual(
                4,
                "other".into(),
                "2026-10-12 10:00:00".into(),
            )],
        );
        let rows = visible_rows(&store, "bob", 0, 0, false);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].index, 1);
        assert!(rows[0].automatic);
    }
    #[test]
    fn remote_keys_are_not_mistaken_for_missing_files() {
        assert!(!is_local_file("__stream_title__:episode"));
        assert!(!is_local_file("https://example.com/file"));
    }
}
