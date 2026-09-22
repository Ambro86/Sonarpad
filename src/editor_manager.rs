use crate::accessibility::{EM_GETSEL, EM_REPLACESEL, EM_SCROLLCARET, to_wide, to_wide_normalized};
use crate::app_windows::route_service::RouteMapData;
use crate::ebook_formats::{
    is_daisy_path, is_kindle_path, read_daisy_document, read_kindle_document,
};
use crate::file_handler::decode_text_with_encoding;
use crate::file_handler::*;
use crate::settings::{
    AppSettings, FileFormat, IndentationMode, Language, ModifiedMarkerPosition, TextEncoding,
    TtsEngine, confirm_save_message, confirm_title, untitled_title,
};
use crate::{EM_LINEFROMCHAR, EM_LINEINDEX, get_active_edit, log_debug, with_state};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{BOOL, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{HFONT, InvalidateRect};
use windows::Win32::System::Memory::GMEM_MOVEABLE;
use windows::Win32::UI::Controls::RichEdit::{
    CFM_COLOR, CFM_SIZE, CHARFORMAT2W, CHARRANGE, EM_EXGETSEL, EM_EXSETSEL, EM_GETTEXTRANGE,
    EM_SETCHARFORMAT, EM_SETEVENTMASK, ENM_CHANGE, ENM_SELCHANGE, MSFTEDIT_CLASS, SCF_ALL,
    TEXTRANGEW,
};
use windows::Win32::UI::Controls::{
    EM_GETMODIFY, EM_SETMODIFY, EM_SETREADONLY, TCIF_TEXT, TCITEMW, TCM_ADJUSTRECT,
    TCM_INSERTITEMW, TCM_SETCURSEL, TCM_SETITEMW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_CONTROL, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_MENU,
    VK_NEXT, VK_PRIOR, VK_RIGHT, VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_WANTRETURN,
    GWLP_USERDATA, GWLP_WNDPROC, GetClientRect, GetParent, GetWindowLongPtrW, HMENU, IDNO, IDYES,
    MB_ICONWARNING, MB_YESNOCANCEL, MoveWindow, PostMessageW, SW_HIDE, SW_SHOW, SendMessageW,
    SetWindowLongPtrW, SetWindowTextW, ShowWindow, WM_CHAR, WM_CONTEXTMENU, WM_GETTEXTLENGTH,
    WM_KEYDOWN, WM_LBUTTONUP, WM_SETFONT, WM_UNDO, WS_CHILD, WS_CLIPCHILDREN, WS_EX_CLIENTEDGE,
    WS_GROUP, WS_HSCROLL, WS_VSCROLL,
};
use windows::core::{PCWSTR, PWSTR};

const EM_LIMITTEXT: u32 = 0x00C5;
const EM_CANUNDO: u32 = 0x00C6;
const EM_EMPTYUNDOBUFFER: u32 = 0x00CD;
const EM_SETSEL: u32 = 0x00B1;
const EM_BEGINUNDOACTION: u32 = 0x0459;
const EM_ENDUNDOACTION: u32 = 0x045A;
const EM_STOPGROUPTYPING: u32 = 0x0477;
const EM_SETTEXTEX: u32 = 0x0461;
const EM_GETTEXTLENGTHEX: u32 = 0x045F;
const EM_SETTABSTOPS: u32 = 0x00CB;
const EM_SETTARGETDEVICE: u32 = 0x0448;
const ST_KEEPUNDO: u32 = 0x0001;
const ST_SELECTION: u32 = 0x0002;

pub(crate) struct LoadedDocument {
    pub(crate) content: String,
    pub(crate) format: FileFormat,
    pub(crate) opened_text_encoding: Option<TextEncoding>,
    pub(crate) epub_index: Vec<EpubIndexEntry>,
    pub(crate) epub_original_text: Option<String>,
}

pub(crate) struct DocumentLoadResult {
    pub(crate) path: PathBuf,
    pub(crate) result: Result<Option<LoadedDocument>, String>,
}
const GTL_NUMCHARS: u32 = 0x0008;
const CP_UNICODE: u32 = 1200;
const VOICE_PANEL_PADDING: i32 = 10;
const VOICE_PANEL_ROW_HEIGHT: i32 = 24;
const VOICE_PANEL_SPACING: i32 = 8;
const VOICE_PANEL_LABEL_WIDTH: i32 = 150;
const VOICE_PANEL_COMBO_HEIGHT: i32 = 140;
const VOICE_PANEL_BUTTON_WIDTH: i32 = 104;

fn should_use_opening_quote(hwnd_edit: HWND) -> bool {
    unsafe {
        let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
        if selection.cpMin <= 0 {
            return true;
        }

        let prev_index = selection.cpMin - 1;
        let mut buf = [0u16; 2];
        let mut range = TEXTRANGEW {
            chrg: CHARRANGE {
                cpMin: prev_index,
                cpMax: selection.cpMin,
            },
            lpstrText: PWSTR(buf.as_mut_ptr()),
        };
        SendMessageW(
            hwnd_edit,
            EM_GETTEXTRANGE,
            WPARAM(0),
            LPARAM(&mut range as *mut _ as isize),
        );
        let prev_char = char::from_u32(buf[0] as u32).unwrap_or('\0');
        matches!(
            prev_char,
            '\0' | ' '
                | '\n'
                | '\r'
                | '\t'
                | '('
                | '['
                | '{'
                | '<'
                | '—'
                | '–'
                | '«'
                | '“'
                | '‘'
                | '/'
                | '\\'
                | '‒'
                | '―'
        )
    }
}

unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "edit_subclass_proc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || edit_subclass_proc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn edit_subclass_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let call_prev = || {
            let prev = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if prev != 0 {
                CallWindowProcW(
                    Some(std::mem::transmute::<
                        isize,
                        unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
                    >(prev)),
                    hwnd,
                    msg,
                    wparam,
                    lparam,
                )
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        };

        if msg == WM_KEYDOWN && wparam.0 as u32 == VK_TAB.0 as u32 {
            let ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
            let shift_down = (GetKeyState(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
            let alt_down = (GetKeyState(VK_MENU.0 as i32) & (0x8000u16 as i16)) != 0;
            if !ctrl_down && !alt_down {
                let parent = GetParent(hwnd);
                let (mode, space_width, tab_width) = with_state(parent, |state| {
                    (
                        state.settings.indentation_mode,
                        state.settings.indent_space_width,
                        state.settings.indent_tab_width,
                    )
                })
                .unwrap_or((IndentationMode::Default, 4, 4));
                if handle_indent_tab_key(hwnd, mode, space_width, tab_width, shift_down) {
                    return LRESULT(0);
                }
            }
        }
        if msg == WM_KEYDOWN {
            let vk = wparam.0 as u32;
            let ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
            let shift_down = (GetKeyState(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
            let alt_down = (GetKeyState(VK_MENU.0 as i32) & (0x8000u16 as i16)) != 0;
            if vk == VK_ESCAPE.0 as u32 {
                let parent = GetParent(hwnd);
                if with_state(parent, |state| state.settings.editor_escape_closes_window)
                    .unwrap_or(false)
                {
                    try_close_app(parent);
                    return LRESULT(0);
                }
            }
            if !ctrl_down
                && !shift_down
                && !alt_down
                && (vk == VK_UP.0 as u32 || vk == VK_DOWN.0 as u32)
            {
                let parent = GetParent(hwnd);
                let move_to_line_start = with_state(parent, |state| {
                    state.settings.editor_up_down_moves_to_line_start
                })
                .unwrap_or(false);
                if move_to_line_start {
                    if with_state(parent, |state| {
                        state.spellcheck_typing_in_progress = false;
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access editor state");
                    }
                    let result = call_prev();
                    let mut caret_pos: u32 = 0;
                    SendMessageW(
                        hwnd,
                        EM_GETSEL,
                        WPARAM(&mut caret_pos as *mut u32 as usize),
                        LPARAM(0),
                    );
                    let line = crate::send_message_w_safe(
                        hwnd,
                        EM_LINEFROMCHAR,
                        WPARAM(caret_pos as usize),
                        LPARAM(0),
                    )
                    .0 as i32;
                    let line_start = crate::send_message_w_safe(
                        hwnd,
                        EM_LINEINDEX,
                        WPARAM(line as usize),
                        LPARAM(0),
                    )
                    .0 as i32;
                    if line_start >= 0 {
                        SendMessageW(
                            hwnd,
                            EM_SETSEL,
                            WPARAM(line_start as usize),
                            LPARAM(line_start as isize),
                        );
                    }
                    return result;
                }
            }
            if matches!(
                vk,
                v if v == VK_LEFT.0 as u32
                    || v == VK_RIGHT.0 as u32
                    || v == VK_UP.0 as u32
                    || v == VK_DOWN.0 as u32
                    || v == VK_HOME.0 as u32
                    || v == VK_END.0 as u32
                    || v == VK_PRIOR.0 as u32
                    || v == VK_NEXT.0 as u32
            ) {
                let parent = GetParent(hwnd);
                if with_state(parent, |state| {
                    state.spellcheck_typing_in_progress = false;
                })
                .is_none()
                {
                    crate::log_debug("Failed to access editor state");
                }
            }
        }
        if msg == WM_CHAR {
            let ch = wparam.0 as u32;
            if ch == VK_TAB.0 as u32 {
                let ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
                let alt_down = (GetKeyState(VK_MENU.0 as i32) & (0x8000u16 as i16)) != 0;
                if !ctrl_down && !alt_down {
                    return LRESULT(0);
                }
            }
            if matches!(
                ch,
                9 | 13 | 32 | 44 | 46 | 58 | 59 | 33 | 63 | 41 | 93 | 125
            ) {
                let parent = GetParent(hwnd);
                if with_state(parent, |state| {
                    state.spellcheck_space_trigger = Some(hwnd);
                    state.spellcheck_typing_in_progress = false;
                })
                .is_none()
                {
                    crate::log_debug("Failed to access editor state");
                }
            } else if ch >= 32 {
                let parent = GetParent(hwnd);
                if with_state(parent, |state| {
                    state.spellcheck_typing_in_progress = true;
                })
                .is_none()
                {
                    crate::log_debug("Failed to access editor state");
                }
            }
            if ch == '\'' as u32 || ch == '\"' as u32 {
                let parent = GetParent(hwnd);
                let enabled =
                    with_state(parent, |state| state.settings.smart_quotes).unwrap_or(false);
                if enabled {
                    let opening = should_use_opening_quote(hwnd);
                    let replacement = match (ch, opening) {
                        (34, true) => "“",
                        (34, false) => "”",
                        (_, true) => "‘",
                        _ => "’",
                    };
                    let wide = to_wide(replacement);
                    SendMessageW(
                        hwnd,
                        EM_REPLACESEL,
                        WPARAM(1),
                        LPARAM(wide.as_ptr() as isize),
                    );
                    return LRESULT(0);
                }
            }
        }
        if msg == WM_LBUTTONUP {
            let parent = GetParent(hwnd);
            if with_state(parent, |state| {
                state.spellcheck_typing_in_progress = false;
            })
            .is_none()
            {
                crate::log_debug("Failed to access editor state");
            }
        }
        if msg == WM_CONTEXTMENU {
            let parent = GetParent(hwnd);
            crate::show_editor_context_menu(parent, hwnd, lparam);
            return LRESULT(0);
        }

        call_prev()
    }
}

fn normalize_indent_width(width: u32) -> u32 {
    match width {
        2 | 4 | 6 | 8 => width,
        _ => 4,
    }
}

fn handle_indent_tab_key(
    hwnd: HWND,
    mode: IndentationMode,
    space_width: u32,
    tab_width: u32,
    shift_down: bool,
) -> bool {
    let mut sel = CHARRANGE::default();
    unsafe {
        SendMessageW(
            hwnd,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut sel as *mut _ as isize),
        );
    }
    let mut start = sel.cpMin;
    let mut end = sel.cpMax;
    if end < start {
        std::mem::swap(&mut start, &mut end);
    }

    if start == end {
        if shift_down {
            outdent_single_line(hwnd, start, mode, space_width, tab_width);
            return true;
        }
        let text = match mode {
            IndentationMode::Spaces => " ".repeat(normalize_indent_width(space_width) as usize),
            IndentationMode::Tabs => "\t".repeat(normalize_indent_width(tab_width) as usize),
            IndentationMode::Default => "\t".to_string(),
        };
        let wide = to_wide(&text);
        unsafe {
            SendMessageW(
                hwnd,
                EM_REPLACESEL,
                WPARAM(1),
                LPARAM(wide.as_ptr() as isize),
            );
        }
        return true;
    }

    indent_selection(hwnd, start, end, mode, space_width, tab_width, shift_down);
    true
}

pub fn indent_active_edit(hwnd: HWND, shift_down: bool) -> bool {
    {
        let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
            return false;
        };
        let (mode, space_width, tab_width) = with_state(hwnd, |state| {
            (
                state.settings.indentation_mode,
                state.settings.indent_space_width,
                state.settings.indent_tab_width,
            )
        })
        .unwrap_or((IndentationMode::Default, 4, 4));
        handle_indent_tab_key(hwnd_edit, mode, space_width, tab_width, shift_down)
    }
}

fn outdent_single_line(
    hwnd: HWND,
    caret_pos: i32,
    mode: IndentationMode,
    space_width: u32,
    tab_width: u32,
) {
    let line =
        crate::send_message_w_safe(hwnd, EM_LINEFROMCHAR, WPARAM(caret_pos as usize), LPARAM(0)).0
            as i32;
    let line_start =
        crate::send_message_w_safe(hwnd, EM_LINEINDEX, WPARAM(line as usize), LPARAM(0)).0 as i32;
    if line_start < 0 {
        return;
    }
    let indent_width = normalize_indent_width(space_width);
    let tab_width = normalize_indent_width(tab_width);
    let remove_len = detect_indent_removal(hwnd, line_start, mode, indent_width, tab_width);
    if remove_len <= 0 {
        return;
    }
    unsafe {
        let mut range = CHARRANGE {
            cpMin: line_start,
            cpMax: line_start + remove_len,
        };
        SendMessageW(
            hwnd,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut range as *mut _ as isize),
        );
        SendMessageW(hwnd, EM_REPLACESEL, WPARAM(1), LPARAM(0));
        let new_pos = (caret_pos - remove_len).max(line_start);
        let mut caret = CHARRANGE {
            cpMin: new_pos,
            cpMax: new_pos,
        };
        SendMessageW(
            hwnd,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut caret as *mut _ as isize),
        );
    }
}

fn indent_selection(
    hwnd: HWND,
    start: i32,
    end: i32,
    mode: IndentationMode,
    space_width: u32,
    tab_width: u32,
    shift_down: bool,
) {
    let start_line =
        crate::send_message_w_safe(hwnd, EM_LINEFROMCHAR, WPARAM(start as usize), LPARAM(0)).0
            as i32;
    let mut end_line =
        crate::send_message_w_safe(hwnd, EM_LINEFROMCHAR, WPARAM(end as usize), LPARAM(0)).0 as i32;
    let end_line_start =
        crate::send_message_w_safe(hwnd, EM_LINEINDEX, WPARAM(end_line as usize), LPARAM(0)).0
            as i32;
    if end > start && end == end_line_start {
        end_line -= 1;
    }
    if end_line < start_line {
        return;
    }

    let indent_width = normalize_indent_width(space_width);
    let tab_width = normalize_indent_width(tab_width);
    let indent_text = match mode {
        IndentationMode::Spaces => " ".repeat(indent_width as usize),
        IndentationMode::Tabs => "\t".repeat(tab_width as usize),
        IndentationMode::Default => "\t".to_string(),
    };
    let indent_len = indent_text.chars().count() as i32;

    let mut start_delta = 0i32;
    let mut end_delta = 0i32;

    unsafe {
        SendMessageW(hwnd, EM_BEGINUNDOACTION, WPARAM(0), LPARAM(0));
    }
    for line in (start_line..=end_line).rev() {
        let line_start =
            crate::send_message_w_safe(hwnd, EM_LINEINDEX, WPARAM(line as usize), LPARAM(0)).0
                as i32;
        if line_start < 0 {
            continue;
        }
        if shift_down {
            let remove_len = detect_indent_removal(hwnd, line_start, mode, indent_width, tab_width);
            if remove_len > 0 {
                unsafe {
                    let mut range = CHARRANGE {
                        cpMin: line_start,
                        cpMax: line_start + remove_len,
                    };
                    SendMessageW(
                        hwnd,
                        EM_EXSETSEL,
                        WPARAM(0),
                        LPARAM(&mut range as *mut _ as isize),
                    );
                    SendMessageW(hwnd, EM_REPLACESEL, WPARAM(1), LPARAM(0));
                }
                if line == start_line && line_start < start {
                    start_delta -= remove_len;
                }
                if line_start < end {
                    end_delta -= remove_len;
                }
            }
        } else {
            let wide = to_wide(&indent_text);
            unsafe {
                let mut range = CHARRANGE {
                    cpMin: line_start,
                    cpMax: line_start,
                };
                SendMessageW(
                    hwnd,
                    EM_EXSETSEL,
                    WPARAM(0),
                    LPARAM(&mut range as *mut _ as isize),
                );
                SendMessageW(
                    hwnd,
                    EM_REPLACESEL,
                    WPARAM(1),
                    LPARAM(wide.as_ptr() as isize),
                );
            }
            if line == start_line && line_start < start {
                start_delta += indent_len;
            }
            if line_start < end {
                end_delta += indent_len;
            }
        }
    }
    unsafe {
        SendMessageW(hwnd, EM_ENDUNDOACTION, WPARAM(0), LPARAM(0));
    }

    let new_start = start + start_delta;
    let new_end = end + end_delta;
    unsafe {
        let mut range = CHARRANGE {
            cpMin: new_start,
            cpMax: new_end,
        };
        SendMessageW(
            hwnd,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut range as *mut _ as isize),
        );
    }
}

fn detect_indent_removal(
    hwnd: HWND,
    line_start: i32,
    mode: IndentationMode,
    indent_width: u32,
    tab_width: u32,
) -> i32 {
    let max_len = match mode {
        IndentationMode::Tabs => tab_width.max(1),
        IndentationMode::Spaces => indent_width.max(1),
        IndentationMode::Default => 1,
    } as i32;
    let text = get_text_range_simple(hwnd, line_start, line_start + max_len);
    if text.is_empty() {
        return 0;
    }

    let tab_first = text.starts_with('\t');
    let spaces = text.chars().take_while(|c| *c == ' ').count() as i32;
    match mode {
        IndentationMode::Spaces => {
            if spaces > 0 {
                spaces.min(indent_width as i32)
            } else {
                0
            }
        }
        IndentationMode::Tabs => {
            if tab_first {
                text.chars()
                    .take_while(|c| *c == '\t')
                    .count()
                    .min(tab_width as usize) as i32
            } else {
                0
            }
        }
        _ => {
            if tab_first || spaces > 0 {
                1
            } else {
                0
            }
        }
    }
}

fn get_text_range_simple(hwnd: HWND, start: i32, end: i32) -> String {
    if end <= start {
        return String::new();
    }
    let mut buf = vec![0u16; (end - start) as usize + 1];
    let mut range = TEXTRANGEW {
        chrg: CHARRANGE {
            cpMin: start,
            cpMax: end,
        },
        lpstrText: PWSTR(buf.as_mut_ptr()),
    };
    unsafe {
        SendMessageW(
            hwnd,
            EM_GETTEXTRANGE,
            WPARAM(0),
            LPARAM(&mut range as *mut _ as isize),
        );
    }
    let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

fn selected_line_block_from_selection(
    hwnd_edit: HWND,
    _text: &str,
    mut selection: CHARRANGE,
) -> Option<(i32, i32, String, bool)> {
    unsafe {
        if selection.cpMin == selection.cpMax {
            return None;
        }
        if selection.cpMax < selection.cpMin {
            std::mem::swap(&mut selection.cpMin, &mut selection.cpMax);
        }

        let text_len = SendMessageW(hwnd_edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as i32;
        let mut start = selection.cpMin.clamp(0, text_len);
        let end = selection.cpMax.clamp(0, text_len);
        if start == end {
            return None;
        }

        // If selection starts exactly on a line break, normalize to next line start.
        if start < end {
            let next = (start + 1).min(text_len);
            let first = get_text_range_simple(hwnd_edit, start, next);
            if first == "\r" {
                let next2 = (start + 2).min(text_len);
                let second = get_text_range_simple(hwnd_edit, next, next2);
                start = if second == "\n" {
                    (start + 2).min(end)
                } else {
                    next
                };
            } else if first == "\n" {
                start = next;
            }
        }
        if start == end {
            return None;
        }

        let start_line = SendMessageW(
            hwnd_edit,
            EM_LINEFROMCHAR,
            WPARAM(start as usize),
            LPARAM(0),
        )
        .0 as i32;

        // Compute end line from cpMax and exclude the next line when cpMax is
        // exactly at that line start (classic RichEdit full-line selection case).
        let mut end_line =
            SendMessageW(hwnd_edit, EM_LINEFROMCHAR, WPARAM(end as usize), LPARAM(0)).0 as i32;
        let end_line_start = SendMessageW(
            hwnd_edit,
            EM_LINEINDEX,
            WPARAM(end_line as usize),
            LPARAM(0),
        )
        .0 as i32;
        if end > start && end == end_line_start {
            end_line -= 1;
        }
        if end_line < start_line {
            return None;
        }

        let range_start = SendMessageW(
            hwnd_edit,
            EM_LINEINDEX,
            WPARAM(start_line as usize),
            LPARAM(0),
        )
        .0 as i32;
        let mut range_end = SendMessageW(
            hwnd_edit,
            EM_LINEINDEX,
            WPARAM((end_line + 1) as usize),
            LPARAM(0),
        )
        .0 as i32;
        if range_end < 0 {
            range_end = SendMessageW(hwnd_edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as i32;
        }

        let selected = get_text_range_simple(hwnd_edit, range_start, range_end);
        let trailing = selected.ends_with('\n') || selected.ends_with('\r');
        Some((range_start, range_end, selected, trailing))
    }
}

fn current_line_block_from_caret(
    hwnd_edit: HWND,
    selection: CHARRANGE,
) -> Option<(i32, i32, String, bool)> {
    unsafe {
        let text_len = SendMessageW(hwnd_edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as i32;
        if text_len <= 0 {
            return None;
        }
        let caret = selection.cpMin.clamp(0, text_len);
        let line = SendMessageW(
            hwnd_edit,
            EM_LINEFROMCHAR,
            WPARAM(caret as usize),
            LPARAM(0),
        )
        .0 as i32;
        let range_start =
            SendMessageW(hwnd_edit, EM_LINEINDEX, WPARAM(line as usize), LPARAM(0)).0 as i32;
        if range_start < 0 {
            return None;
        }
        let mut range_end = SendMessageW(
            hwnd_edit,
            EM_LINEINDEX,
            WPARAM((line + 1) as usize),
            LPARAM(0),
        )
        .0 as i32;
        if range_end < 0 {
            range_end = text_len;
        }
        let selected = get_text_range_simple(hwnd_edit, range_start, range_end);
        let trailing = selected.ends_with('\n') || selected.ends_with('\r');
        Some((range_start, range_end, selected, trailing))
    }
}

fn restore_caret_after_line_op_if_no_selection(
    hwnd_edit: HWND,
    had_selection: bool,
    original_caret: i32,
    replace_start: i32,
    original_text: &str,
    replaced_text: &str,
) {
    unsafe {
        let new_caret = if had_selection {
            // Keep caret at the start of the last selected line.
            // If the block ends with a newline, ignore trailing line breaks first,
            // otherwise caret can land on the next line outside the selection.
            let trimmed = replaced_text.trim_end_matches(['\r', '\n']);
            let last_line_start = trimmed.rfind('\n').map(|idx| idx + 1).unwrap_or(0);
            let line_start_in_block = byte_index_to_utf16(trimmed, last_line_start);
            replace_start.saturating_add(line_start_in_block)
        } else {
            let new_len = byte_index_to_utf16(replaced_text, replaced_text.len());
            let relative = original_caret.saturating_sub(replace_start).max(0);
            // Preserve caret near the same logical content when line ops add/remove a leading prefix
            // (e.g. quote/unquote or similar commands that shift the line start).
            let first_old_line = original_text
                .split('\n')
                .next()
                .unwrap_or(original_text)
                .trim_end_matches('\r');
            let first_new_line = replaced_text
                .split('\n')
                .next()
                .unwrap_or(replaced_text)
                .trim_end_matches('\r');
            let leading_delta =
                if !first_old_line.is_empty() && first_new_line.ends_with(first_old_line) {
                    byte_index_to_utf16(
                        first_new_line,
                        first_new_line.len().saturating_sub(first_old_line.len()),
                    )
                } else if !first_new_line.is_empty() && first_old_line.ends_with(first_new_line) {
                    -byte_index_to_utf16(
                        first_old_line,
                        first_old_line.len().saturating_sub(first_new_line.len()),
                    )
                } else {
                    0
                };
            let shifted = if leading_delta >= 0 {
                relative.saturating_add(leading_delta)
            } else {
                relative.saturating_sub(leading_delta.saturating_abs())
            };
            replace_start.saturating_add(shifted.min(new_len))
        };
        let mut caret_range = CHARRANGE {
            cpMin: new_caret,
            cpMax: new_caret,
        };
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut caret_range as *mut _ as isize),
        );
    }
}

fn apply_indent_settings_to_edit(hwnd_edit: HWND, settings: &AppSettings) {
    let width = match settings.indentation_mode {
        IndentationMode::Spaces => settings.indent_space_width,
        _ => settings.indent_tab_width,
    };
    let width = normalize_indent_width(width);
    let tab_stop = (width * 4) as i32;
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_SETTABSTOPS,
            WPARAM(1),
            LPARAM(&tab_stop as *const _ as isize),
        );
    }
}

pub(crate) fn apply_indent_settings_to_all_edits(hwnd: HWND, settings: &AppSettings) {
    {
        with_state(hwnd, |state| {
            for doc in &state.docs {
                if doc.hwnd_edit.0 != 0 {
                    apply_indent_settings_to_edit(doc.hwnd_edit, settings);
                }
            }
        });
    }
}

fn apply_text_limit(hwnd_edit: HWND) {
    unsafe {
        if hwnd_edit.0 != 0 {
            SendMessageW(hwnd_edit, EM_LIMITTEXT, WPARAM(0x7FFFFFFE), LPARAM(0));
        }
    }
}

pub fn apply_text_limit_to_all_edits(hwnd: HWND) {
    let edits = {
        with_state(hwnd, |state| {
            state.docs.iter().map(|d| d.hwnd_edit).collect::<Vec<_>>()
        })
        .unwrap_or_default()
    };

    for hwnd_edit in edits {
        apply_text_limit(hwnd_edit);
    }
}

pub fn insert_voice_tag_at_caret(
    hwnd: HWND,
    engine: TtsEngine,
    voice: &str,
    rate: i32,
    pitch: i32,
    volume: i32,
) {
    let voice = voice.trim();
    if voice.is_empty() {
        return;
    }
    let Some(hwnd_edit) = get_active_edit(hwnd) else {
        log_debug("insert_voice_tag_at_caret: no active edit");
        return;
    };
    let engine_token = match engine {
        TtsEngine::Edge => "edge",
        TtsEngine::Sapi5 => "sapi5",
        TtsEngine::Sapi4 => "sapi4",
        TtsEngine::Google => "google",
    };
    let voice = if engine == TtsEngine::Google {
        crate::google_tts::voice_id_from_stored(voice)
    } else {
        voice
    };
    let mut extras = String::new();
    if rate != 0 {
        extras.push_str(&format!(" speed={rate}"));
    }
    if pitch != 0 {
        extras.push_str(&format!(" pitch={pitch}"));
    }
    if volume != 100 {
        extras.push_str(&format!(" volume={volume}"));
    }
    let open = format!("<voice {engine_token} {voice}{extras}>");
    let close = "</voice>";
    let insert = format!("{open}  {close}");
    let mut start: u32 = 0;
    let mut end: u32 = 0;
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_GETSEL,
            WPARAM(&mut start as *mut u32 as usize),
            LPARAM(&mut end as *mut u32 as isize),
        );
        let text_len = SendMessageW(hwnd_edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as u32;
        log_debug(&format!(
            "insert_voice_tag_at_caret: hwnd_edit={:?} engine={} voice_len={} sel_start={} sel_end={} text_len={}",
            hwnd_edit,
            engine_token,
            voice.len(),
            start,
            end,
            text_len
        ));
        if start != end {
            let full_text = get_edit_text(hwnd_edit);
            let selection = CHARRANGE {
                cpMin: start as i32,
                cpMax: end as i32,
            };
            if let Some((range_start, range_end, selected, _)) =
                selected_line_block_from_selection(hwnd_edit, &full_text, selection)
            {
                let wrapped = wrap_voice_tag_block(&selected, &open, close);
                let mut replace_range = CHARRANGE {
                    cpMin: range_start,
                    cpMax: range_end,
                };
                SendMessageW(
                    hwnd_edit,
                    EM_EXSETSEL,
                    WPARAM(0),
                    LPARAM(&mut replace_range as *mut _ as isize),
                );
                let wide = to_wide(&wrapped);
                SendMessageW(
                    hwnd_edit,
                    EM_REPLACESEL,
                    WPARAM(1),
                    LPARAM(wide.as_ptr() as isize),
                );
                copy_voice_tag_to_clipboard(hwnd, &insert);
                return;
            }
        }
        let full_text = get_edit_text(hwnd_edit);
        let lower = full_text.to_ascii_lowercase();
        let full_utf16: Vec<u16> = full_text.encode_utf16().collect();
        let lower_utf16: Vec<u16> = lower.encode_utf16().collect();
        let caret_pos = start as usize;
        let open_needle: Vec<u16> = "<voice".encode_utf16().collect();
        let close_needle: Vec<u16> = "</voice>".encode_utf16().collect();
        let gt: u16 = '>' as u16;

        // Check if the caret is inside a </voice> closing tag literal.
        // If so, move insertion point after that closing tag.
        let caret_pos = if let Some(ct_start) = find_next_utf16_at_or_after(
            &lower_utf16,
            &close_needle,
            caret_pos.saturating_sub(close_needle.len().saturating_sub(1)),
        ) {
            let ct_end = ct_start + close_needle.len();
            if caret_pos >= ct_start && caret_pos < ct_end {
                log_debug(
                    "insert_voice_tag_at_caret: caret inside closing tag literal, moving after it",
                );
                ct_end
            } else {
                caret_pos
            }
        } else {
            caret_pos
        };

        // Check if the caret is inside an opening <voice ...> tag literal
        // (between '<' and '>'). If so, move insertion point before that tag.
        let caret_pos = if let Some(open_start) = find_last_utf16_before(
            &lower_utf16,
            &open_needle,
            caret_pos.saturating_add(open_needle.len()),
        ) {
            // Make sure this is not a </voice match (slash before 'voice')
            let is_closing = open_start > 0
                && full_utf16.get(open_start.saturating_sub(1)).copied() == Some('/' as u16);
            if !is_closing && caret_pos >= open_start {
                // Find the closing '>' of this opening tag
                let tag_end = full_utf16[open_start..]
                    .iter()
                    .position(|&c| c == gt)
                    .map(|p| open_start + p + 1);
                if let Some(end) = tag_end {
                    if caret_pos < end {
                        log_debug(
                            "insert_voice_tag_at_caret: caret inside opening tag literal, moving before it",
                        );
                        open_start
                    } else {
                        caret_pos
                    }
                } else {
                    log_debug(
                        "insert_voice_tag_at_caret: caret inside malformed opening tag, moving before it",
                    );
                    open_start
                }
            } else {
                caret_pos
            }
        } else {
            caret_pos
        };

        let last_open = find_last_utf16_before(&lower_utf16, &open_needle, caret_pos);
        let last_close = find_last_utf16_before(&lower_utf16, &close_needle, caret_pos);
        let inside_voice =
            matches!(last_open, Some(open_pos) if last_close.map(|c| open_pos > c).unwrap_or(true));
        let mut insert_pos = caret_pos;
        let mut needs_newline = false;
        if let Some(close_pos) =
            find_last_utf16_before(&lower_utf16, &close_needle, caret_pos.saturating_add(1))
        {
            let close_end = close_pos.saturating_add(close_needle.len());
            if caret_pos < close_end {
                insert_pos = close_end;
                if insert_pos > 0 {
                    let prev = full_utf16
                        .get(insert_pos.saturating_sub(1))
                        .copied()
                        .unwrap_or(0);
                    needs_newline = prev != '\n' as u16 && prev != '\r' as u16;
                } else {
                    needs_newline = true;
                }
                if needs_newline {
                    log_debug(
                        "insert_voice_tag_at_caret: caret inside closing tag, moving to new line after close",
                    );
                } else {
                    log_debug(
                        "insert_voice_tag_at_caret: caret inside closing tag, moving after close",
                    );
                }
            }
        }
        if insert_pos == caret_pos
            && inside_voice
            && let Some(close_pos) = find_next_utf16_at_or_after(
                &lower_utf16,
                &close_needle,
                caret_pos.saturating_sub(close_needle.len().saturating_sub(1)),
            )
        {
            insert_pos = close_pos + close_needle.len();
            if insert_pos > 0 {
                let prev = full_utf16
                    .get(insert_pos.saturating_sub(1))
                    .copied()
                    .unwrap_or(0);
                needs_newline = prev != '\n' as u16 && prev != '\r' as u16;
            } else {
                needs_newline = true;
            }
            if needs_newline {
                log_debug(
                    "insert_voice_tag_at_caret: inside voice tag, moving to new line after close",
                );
            } else {
                log_debug("insert_voice_tag_at_caret: inside voice tag, moving after close");
            }
        }
        let actual_insert = if insert_pos != caret_pos {
            insert_pos
        } else {
            caret_pos
        };
        if actual_insert != start as usize {
            let pos_i32 = actual_insert as i32;
            SendMessageW(
                hwnd_edit,
                EM_SETSEL,
                WPARAM(pos_i32 as usize),
                LPARAM(pos_i32 as isize),
            );
        }
        let insert_text = if needs_newline {
            format!("\r\n{insert}")
        } else {
            insert.clone()
        };
        let wide = to_wide(&insert_text);
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(wide.as_ptr() as isize),
        );
        copy_voice_tag_to_clipboard(hwnd, &insert);
        // After EM_REPLACESEL the caret sits at the end of the inserted text.
        // Move it back by len(" </voice>") = close.len() + 1 to place it
        // between the two padding spaces: "<voice ...> | </voice>".
        let mut after_sel: u32 = 0;
        SendMessageW(
            hwnd_edit,
            EM_GETSEL,
            WPARAM(&mut after_sel as *mut u32 as usize),
            LPARAM(0),
        );
        let close_u16_len = close.encode_utf16().count() as i32;
        let caret = after_sel as i32 - close_u16_len - 1;
        log_debug(&format!(
            "insert_voice_tag_at_caret: after_sel={} close_u16_len={} caret={} actual_insert={} start={} needs_newline={}",
            after_sel, close_u16_len, caret, actual_insert, start, needs_newline,
        ));
        SendMessageW(
            hwnd_edit,
            EM_SETSEL,
            WPARAM(caret as usize),
            LPARAM(caret as isize),
        );
        SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
        let mut new_start: u32 = 0;
        let mut new_end: u32 = 0;
        SendMessageW(
            hwnd_edit,
            EM_GETSEL,
            WPARAM(&mut new_start as *mut u32 as usize),
            LPARAM(&mut new_end as *mut u32 as isize),
        );
        let new_text_len = SendMessageW(hwnd_edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as u32;
        log_debug(&format!(
            "insert_voice_tag_at_caret: after insert sel_start={} sel_end={} text_len={}",
            new_start, new_end, new_text_len
        ));
    }
}

pub fn insert_pause_tag_at_caret(hwnd: HWND, milliseconds: u32) {
    let Some(hwnd_edit) = get_active_edit(hwnd) else {
        log_debug("insert_pause_tag_at_caret: no active edit");
        return;
    };
    let tag = format!("<pause ms=\"{milliseconds}\"/>");
    unsafe {
        let mut start: u32 = 0;
        let mut end: u32 = 0;
        SendMessageW(
            hwnd_edit,
            EM_GETSEL,
            WPARAM(&mut start as *mut u32 as usize),
            LPARAM(&mut end as *mut u32 as isize),
        );

        if start == end {
            let full_text = get_edit_text(hwnd_edit);
            let lower = full_text.to_ascii_lowercase();
            let full_utf16: Vec<u16> = full_text.encode_utf16().collect();
            let lower_utf16: Vec<u16> = lower.encode_utf16().collect();
            let caret_pos = start as usize;
            let open_needle: Vec<u16> = "<voice".encode_utf16().collect();
            let close_needle: Vec<u16> = "</voice>".encode_utf16().collect();

            let last_open = find_last_utf16_before(&lower_utf16, &open_needle, caret_pos);
            let last_close = find_last_utf16_before(&lower_utf16, &close_needle, caret_pos);
            let inside_voice = matches!(
                last_open,
                Some(open_pos) if last_close.map(|close_pos| open_pos > close_pos).unwrap_or(true)
            );

            if inside_voice
                && let Some(close_pos) = find_next_utf16_at_or_after(
                    &lower_utf16,
                    &close_needle,
                    caret_pos.saturating_sub(close_needle.len().saturating_sub(1)),
                )
            {
                let insert_pos = close_pos + close_needle.len();
                let needs_newline = full_utf16
                    .get(insert_pos.saturating_sub(1))
                    .copied()
                    .map(|prev| prev != '\n' as u16 && prev != '\r' as u16)
                    .unwrap_or(true);
                let pos_i32 = insert_pos as i32;
                SendMessageW(
                    hwnd_edit,
                    EM_SETSEL,
                    WPARAM(pos_i32 as usize),
                    LPARAM(pos_i32 as isize),
                );
                let insert_text = if needs_newline {
                    format!("\r\n{tag}")
                } else {
                    tag.clone()
                };
                let wide = to_wide(&insert_text);
                SendMessageW(
                    hwnd_edit,
                    EM_REPLACESEL,
                    WPARAM(1),
                    LPARAM(wide.as_ptr() as isize),
                );
                SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
                copy_text_to_clipboard(hwnd, &tag, "pause tag");
                return;
            }
        }

        let wide = to_wide(&tag);
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(wide.as_ptr() as isize),
        );
        SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
    }
    copy_text_to_clipboard(hwnd, &tag, "pause tag");
}

fn copy_text_to_clipboard(hwnd: HWND, text: &str, context: &str) {
    const CF_UNICODETEXT: u32 = 13;

    let content = to_wide(text);
    if content.is_empty() {
        return;
    }
    if let Err(e) = crate::open_clipboard_safe(hwnd) {
        log_debug(&format!("OpenClipboard failed for {}: {}", context, e));
        return;
    }
    if let Err(e) = crate::empty_clipboard_safe() {
        log_debug(&format!("EmptyClipboard failed for {}: {}", context, e));
    }
    let size = content.len() * std::mem::size_of::<u16>();
    let handle = match crate::global_alloc_safe(GMEM_MOVEABLE, size) {
        Ok(handle) => handle,
        Err(e) => {
            log_debug(&format!("GlobalAlloc failed for {}: {}", context, e));
            crate::log_if_err!(crate::close_clipboard_safe());
            return;
        }
    };
    let ptr = crate::global_lock_as_safe(handle) as *mut u16;
    if ptr.is_null() {
        log_debug(&format!("GlobalLock failed for {}", context));
        crate::log_if_err!(crate::close_clipboard_safe());
        return;
    }
    // SAFETY: `ptr` points to a movable global memory block locked above with enough
    // bytes for `content`, and the source slice is valid for `content.len()` u16s.
    unsafe {
        std::ptr::copy_nonoverlapping(content.as_ptr(), ptr, content.len());
    }
    crate::log_if_err!(crate::global_unlock_safe(handle));
    if let Err(e) = crate::set_clipboard_data_safe(CF_UNICODETEXT, HANDLE(handle.0 as isize)) {
        log_debug(&format!("SetClipboardData failed for {}: {}", context, e));
    }
    crate::log_if_err!(crate::close_clipboard_safe());
}

fn copy_voice_tag_to_clipboard(hwnd: HWND, tag: &str) {
    copy_text_to_clipboard(hwnd, tag, "voice tag");
}

fn wrap_voice_tag_block(selected: &str, open: &str, close: &str) -> String {
    if let Some(content) = selected.strip_suffix("\r\n") {
        return format!("{open}{content}{close}\r\n");
    }
    if let Some(content) = selected.strip_suffix('\n') {
        return format!("{open}{content}{close}\n");
    }
    if let Some(content) = selected.strip_suffix('\r') {
        return format!("{open}{content}{close}\r");
    }
    format!("{open}{selected}{close}")
}

fn find_last_utf16_before(haystack: &[u16], needle: &[u16], before: usize) -> Option<usize> {
    if needle.is_empty() || haystack.is_empty() {
        return None;
    }
    let limit = before.min(haystack.len());
    if limit < needle.len() {
        return None;
    }
    let mut last = None;
    let end = limit - needle.len();
    for idx in 0..=end {
        if haystack[idx..idx + needle.len()] == *needle {
            last = Some(idx);
        }
    }
    last
}

fn find_next_utf16_at_or_after(haystack: &[u16], needle: &[u16], start: usize) -> Option<usize> {
    if needle.is_empty() || haystack.is_empty() || start >= haystack.len() {
        return None;
    }
    let mut idx = start;
    while idx + needle.len() <= haystack.len() {
        if haystack[idx..idx + needle.len()] == *needle {
            return Some(idx);
        }
        idx += 1;
    }
    None
}

pub struct Document {
    pub title: String,
    pub path: Option<PathBuf>,
    pub hwnd_edit: HWND,
    pub dirty: bool,
    pub format: FileFormat,
    pub opened_text_encoding: Option<TextEncoding>,
    pub current_save_text_encoding: Option<TextEncoding>,
    pub from_rss: bool,
    pub from_italiaonline: bool,
    pub from_find_in_files: bool,
    pub from_wikipedia: bool,
    pub from_treccani: bool,
    pub is_temporary: bool,
    pub confirm_close_when_clean: bool,
    pub prefer_title_for_save_suggestion: bool,
    pub prefer_mpv_playback: bool,
    pub route_map: Option<RouteMapData>,
    pub epub_index: Vec<EpubIndexEntry>,
    pub epub_original_text: Option<String>,
    pub pdf_is_imported_source: bool,
}

#[derive(Clone)]
pub struct NormalizeUndo {
    pub hwnd_edit: HWND,
    pub text: String,
    pub sel_start: i32,
    pub sel_end: i32,
    pub was_dirty: bool,
}

impl Default for Document {
    fn default() -> Self {
        Document {
            title: String::new(),
            path: None,
            hwnd_edit: HWND(0),
            dirty: false,
            format: FileFormat::Text(TextEncoding::Utf8),
            opened_text_encoding: None,
            current_save_text_encoding: None,
            from_rss: false,
            from_italiaonline: false,
            from_find_in_files: false,
            from_wikipedia: false,
            from_treccani: false,
            is_temporary: false,
            confirm_close_when_clean: false,
            prefer_title_for_save_suggestion: false,
            prefer_mpv_playback: false,
            route_map: None,
            epub_index: Vec::new(),
            epub_original_text: None,
            pdf_is_imported_source: false,
        }
    }
}

// --- Editor Helpers ---

pub fn set_edit_text(hwnd_edit: HWND, text: &str) {
    let wide = to_wide_normalized(text);
    if hwnd_edit.0 != 0 {
        // Prevent programmatic loads from marking the document as modified.
        crate::send_message_w_safe(hwnd_edit, EM_SETEVENTMASK, WPARAM(0), LPARAM(0));
    }
    if let Err(e) = crate::set_window_text_w_safe(hwnd_edit, PCWSTR(wide.as_ptr())) {
        crate::log_debug(&format!("Failed to set editor text: {}", e));
    }
    if hwnd_edit.0 != 0 {
        crate::send_message_w_safe(hwnd_edit, EM_SETMODIFY, WPARAM(0), LPARAM(0));
        // Programmatic loads must not leave stale undo history.
        crate::send_message_w_safe(hwnd_edit, EM_EMPTYUNDOBUFFER, WPARAM(0), LPARAM(0));
        unsafe {
            SendMessageW(
                hwnd_edit,
                EM_SETEVENTMASK,
                WPARAM(0),
                LPARAM((ENM_CHANGE | ENM_SELCHANGE) as isize),
            );
        }
    }
}

pub fn get_edit_text(hwnd_edit: HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::{WM_GETTEXT, WM_GETTEXTLENGTH};
    let len =
        crate::send_message_w_safe(hwnd_edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as usize;
    if len == 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len + 1];
    unsafe {
        SendMessageW(
            hwnd_edit,
            WM_GETTEXT,
            WPARAM(buf.len()),
            LPARAM(buf.as_mut_ptr() as isize),
        );
    }
    String::from_utf16_lossy(&buf[..len])
}

pub fn send_to_active_edit(hwnd: HWND, msg: u32) {
    if let Some(hwnd_edit) = crate::get_active_edit(hwnd) {
        crate::send_message_w_safe(hwnd_edit, msg, WPARAM(0), LPARAM(0));
    }
}

pub fn select_all_active_edit(hwnd: HWND) {
    if let Some(hwnd_edit) = crate::get_active_edit(hwnd) {
        let cr = CHARRANGE {
            cpMin: 0,
            cpMax: -1,
        };
        unsafe {
            SendMessageW(
                hwnd_edit,
                EM_EXSETSEL,
                WPARAM(0),
                LPARAM(&cr as *const _ as isize),
            );
        }
    }
}

pub fn remove_duplicate_lines_active_edit(hwnd: HWND) -> bool {
    apply_text_op_active_edit(hwnd, crate::text_ops::remove_duplicate_lines)
}

pub fn remove_duplicate_consecutive_lines_active_edit(hwnd: HWND) -> bool {
    apply_text_op_active_edit(hwnd, crate::text_ops::remove_duplicate_consecutive_lines)
}

pub fn auto_format_tts_active_edit(hwnd: HWND) -> bool {
    apply_text_op_active_edit(hwnd, auto_format_tts_block)
}

fn apply_text_op_active_edit<F>(hwnd: HWND, op: F) -> bool
where
    F: Fn(&str) -> String,
{
    unsafe {
        let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
            return false;
        };

        let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );

        if selection.cpMin > selection.cpMax {
            std::mem::swap(&mut selection.cpMin, &mut selection.cpMax);
        }

        let (affected, replace_range) = if selection.cpMin != selection.cpMax {
            let affected = get_text_range(hwnd_edit, selection);
            if affected.is_empty() {
                return false;
            }
            (affected, selection)
        } else {
            let text = get_edit_text(hwnd_edit);
            if text.is_empty() {
                return false;
            }
            let replace_range = CHARRANGE {
                cpMin: 0,
                cpMax: byte_index_to_utf16(&text, text.len()),
            };
            (text, replace_range)
        };

        let processed = op(&affected);

        if processed == affected {
            return false;
        }

        let mut replace_range = replace_range;

        // Select the range to be replaced
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut replace_range as *mut _ as isize),
        );

        // Single-undo guarantee.
        begin_single_undo_action(hwnd_edit);
        let replace_wide = to_wide(&processed);
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(replace_wide.as_ptr() as isize),
        );

        // According to specs:
        // "If operating on selection: replace the selection and re-select the replaced block (same start, new end)."
        // EM_REPLACESEL with 1 (fCanUndo) often handles caret, but let's ensure selection is set to the new block.
        // The previous selection started at `replace_range.cpMin`.
        // The new end is `replace_range.cpMin + processed_utf16_len`.
        let new_len_utf16 = processed.encode_utf16().count() as i32;
        let mut new_selection = CHARRANGE {
            cpMin: replace_range.cpMin,
            cpMax: replace_range.cpMin + new_len_utf16,
        };
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut new_selection as *mut _ as isize),
        );

        end_single_undo_action(hwnd_edit);
        mark_dirty_from_edit(hwnd, hwnd_edit);
        SetFocus(hwnd_edit);
        true
    }
}

pub(crate) fn get_text_range(hwnd_edit: HWND, range: CHARRANGE) -> String {
    let len = (range.cpMax - range.cpMin).max(0) as usize;
    if len == 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len + 1];
    let mut text_range = TEXTRANGEW {
        chrg: range,
        lpstrText: PWSTR(buf.as_mut_ptr()),
    };
    let copied = crate::send_message_w_safe(
        hwnd_edit,
        EM_GETTEXTRANGE,
        WPARAM(0),
        LPARAM(&mut text_range as *mut _ as isize),
    )
    .0 as usize;
    let used = copied.min(len);
    String::from_utf16_lossy(&buf[..used])
}

pub fn apply_word_wrap_to_all_edits(hwnd: HWND, word_wrap: bool) {
    let edits = {
        with_state(hwnd, |state| {
            state.docs.iter().map(|d| d.hwnd_edit).collect::<Vec<_>>()
        })
    }
    .unwrap_or_default();

    for hwnd_edit in edits {
        if hwnd_edit.0 == 0 {
            continue;
        }
        log_debug(&format!(
            "Word wrap toggle for {:?}: {}",
            hwnd_edit, word_wrap
        ));
        let line_width = if word_wrap { 0 } else { 1 };
        crate::send_message_w_safe(hwnd_edit, EM_SETTARGETDEVICE, WPARAM(0), LPARAM(line_width));
        unsafe {
            InvalidateRect(hwnd_edit, None, true);
        }
        apply_text_limit(hwnd_edit);
    }
}

pub fn apply_text_appearance_to_all_edits(hwnd: HWND, text_color: u32, text_size: i32) {
    let edits = {
        with_state(hwnd, |state| {
            state.docs.iter().map(|d| d.hwnd_edit).collect::<Vec<_>>()
        })
    }
    .unwrap_or_default();

    for hwnd_edit in edits {
        if hwnd_edit.0 == 0 {
            continue;
        }
        apply_text_appearance(hwnd_edit, text_color, text_size);
    }
}

pub fn apply_font_to_all_edits(hwnd: HWND, hfont: HFONT) {
    let edits = {
        with_state(hwnd, |state| {
            state.docs.iter().map(|d| d.hwnd_edit).collect::<Vec<_>>()
        })
    }
    .unwrap_or_default();
    for hwnd_edit in edits {
        if hwnd_edit.0 == 0 {
            continue;
        }
        crate::send_message_w_safe(hwnd_edit, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
    }
}

pub fn apply_read_only_to_all_edits(hwnd: HWND, read_only: bool) {
    let edit_targets = {
        with_state(hwnd, |state| {
            state
                .docs
                .iter()
                .map(|doc| {
                    let force_read_only = matches!(doc.format, FileFormat::Audiobook);
                    (doc.hwnd_edit, read_only || force_read_only)
                })
                .collect::<Vec<_>>()
        })
    }
    .unwrap_or_default();

    for (hwnd_edit, should_read_only) in edit_targets {
        if hwnd_edit.0 != 0 {
            unsafe {
                SendMessageW(
                    hwnd_edit,
                    EM_SETREADONLY,
                    WPARAM(if should_read_only { 1 } else { 0 }),
                    LPARAM(0),
                );
            }
        }
    }
}

fn apply_text_appearance(hwnd_edit: HWND, text_color: u32, text_size: i32) {
    let mut format = CHARFORMAT2W::default();
    format.Base.cbSize = std::mem::size_of::<CHARFORMAT2W>() as u32;
    format.Base.dwMask = CFM_COLOR | CFM_SIZE;
    let visible_text_color = crate::theme::effective_editor_text_color(text_color);
    format.Base.crTextColor = windows::Win32::Foundation::COLORREF(visible_text_color);
    if text_size > 0 {
        format.Base.yHeight = text_size.saturating_mul(20);
    }
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_SETCHARFORMAT,
            WPARAM(SCF_ALL as usize),
            LPARAM(&mut format as *mut _ as isize),
        );
    }
}

#[repr(C)]
struct SetTextEx {
    flags: u32,
    codepage: u32,
}

#[repr(C)]
struct GetTextLengthEx {
    flags: u32,
    codepage: u32,
}

fn begin_single_undo_action(hwnd_edit: HWND) {
    unsafe {
        SendMessageW(hwnd_edit, EM_STOPGROUPTYPING, WPARAM(0), LPARAM(0));
        SendMessageW(hwnd_edit, EM_BEGINUNDOACTION, WPARAM(0), LPARAM(0));
    }
}

fn end_single_undo_action(hwnd_edit: HWND) {
    unsafe {
        SendMessageW(hwnd_edit, EM_ENDUNDOACTION, WPARAM(0), LPARAM(0));
    }
}

pub fn try_normalize_undo(hwnd: HWND) -> bool {
    let mut undo = None;
    if {
        with_state(hwnd, |state| {
            undo = state.normalize_undo.clone();
        })
    }
    .is_none()
    {
        crate::log_debug("Failed to access editor state");
    }
    let Some(undo) = undo else {
        return false;
    };
    if undo.hwnd_edit.0 == 0 {
        if { with_state(hwnd, |state| state.normalize_undo = None) }.is_none() {
            crate::log_debug("Failed to access editor state");
        }
        return false;
    }
    let current_text = get_edit_text(undo.hwnd_edit);
    if current_text == undo.text {
        // Stale snapshot: do not consume Ctrl+Z, let normal editor undo run.
        if { with_state(hwnd, |state| state.normalize_undo = None) }.is_none() {
            crate::log_debug("Failed to access editor state");
        }
        return false;
    }

    if {
        with_state(hwnd, |state| {
            state.normalize_undo = None;
        })
    }
    .is_none()
    {
        crate::log_debug("Failed to access editor state");
    }

    set_edit_text(undo.hwnd_edit, &undo.text);
    let mut cr = CHARRANGE {
        cpMin: undo.sel_start,
        cpMax: undo.sel_end,
    };
    unsafe {
        SendMessageW(
            undo.hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut cr as *mut _ as isize),
        );
    }
    if {
        with_state(hwnd, |state| {
            for (idx, doc) in state.docs.iter_mut().enumerate() {
                if doc.hwnd_edit == undo.hwnd_edit {
                    doc.dirty = undo.was_dirty;
                    update_tab_title(state.hwnd_tab, idx, &doc.title, doc.dirty);
                    if state.current == idx {
                        update_window_title(hwnd);
                    }
                    break;
                }
            }
        })
    }
    .is_none()
    {
        crate::log_debug("Failed to access editor state");
    }
    crate::set_focus_safe(undo.hwnd_edit);
    true
}

pub fn undo_active_edit_skip_navigation(hwnd: HWND) -> bool {
    let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
        return false;
    };
    let mut before = get_edit_text(hwnd_edit);
    // Some operations can leave multiple non-text undo records (caret/selection moves).
    // Keep skipping those until we hit an actual text change.
    for _ in 0..32 {
        let can_undo =
            crate::send_message_w_safe(hwnd_edit, EM_CANUNDO, WPARAM(0), LPARAM(0)).0 != 0;
        if !can_undo {
            return false;
        }
        crate::send_message_w_safe(hwnd_edit, WM_UNDO, WPARAM(0), LPARAM(0));
        let after = get_edit_text(hwnd_edit);
        if after != before {
            return true;
        }
        before = after;
    }
    false
}

pub fn handle_normalize_edit_change(hwnd: HWND, hwnd_edit: HWND) {
    if {
        with_state(hwnd, |state| {
            if state.normalize_skip_change {
                state.normalize_skip_change = false;
                return;
            }
            if let Some(pending) = &state.normalize_undo
                && pending.hwnd_edit == hwnd_edit
            {
                state.normalize_undo = None;
            }
        })
    }
    .is_none()
    {
        crate::log_debug("Failed to access editor state");
    }
}

pub fn strip_markdown_active_edit(hwnd: HWND) -> bool {
    let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
        return false;
    };
    let text = get_edit_text(hwnd_edit);
    if text.is_empty() {
        return false;
    }
    let keep_bullets =
        { with_state(hwnd, |state| state.settings.strip_markdown_keep_bullets) }.unwrap_or(false);
    let cleaned = strip_markdown_text(&text, keep_bullets);
    if cleaned == text {
        return false;
    }
    let mut replace_range = CHARRANGE {
        cpMin: 0,
        cpMax: byte_index_to_utf16(&text, text.len()),
    };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut replace_range as *mut _ as isize),
        );
    }
    // Single-undo guarantee.
    begin_single_undo_action(hwnd_edit);
    let replace_wide = to_wide(&cleaned);
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(replace_wide.as_ptr() as isize),
        );
    }
    end_single_undo_action(hwnd_edit);
    mark_dirty_from_edit(hwnd, hwnd_edit);
    crate::set_focus_safe(hwnd_edit);
    true
}

pub fn normalize_whitespace_active_edit(hwnd: HWND) -> bool {
    let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
        return false;
    };
    let text = get_edit_text(hwnd_edit);
    if text.is_empty() {
        return false;
    }

    let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
    }

    let mut length_info = GetTextLengthEx {
        flags: GTL_NUMCHARS,
        codepage: CP_UNICODE,
    };
    let total_chars = crate::send_message_w_safe(
        hwnd_edit,
        EM_GETTEXTLENGTHEX,
        WPARAM(&mut length_info as *mut _ as usize),
        LPARAM(0),
    )
    .0 as i32;
    let mut sel_start = selection.cpMin;
    let mut sel_end = selection.cpMax;
    if sel_start < 0 {
        sel_start = 0;
    }
    if sel_end < 0 {
        sel_end = total_chars;
    }
    if sel_end > total_chars {
        sel_end = total_chars;
    }
    let near_end = sel_end >= total_chars.saturating_sub(1);
    if near_end {
        sel_end = total_chars;
    }

    let has_selection = sel_start != sel_end;
    let whole_doc_selected = has_selection && sel_start == 0 && near_end;
    let (start_byte, end_byte) = if has_selection {
        (
            utf16_index_to_byte(&text, sel_start),
            utf16_index_to_byte(&text, sel_end),
        )
    } else {
        (0, text.len())
    };

    let (affected_start, affected_end) = if has_selection {
        if whole_doc_selected {
            (0, text.len())
        } else {
            let mut effective_end = end_byte;
            if end_byte > start_byte && end_byte > 0 && text.as_bytes()[end_byte - 1] == b'\n' {
                effective_end = end_byte.saturating_sub(1);
            }
            let line_start = text[..start_byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line_end = text[effective_end..]
                .find('\n')
                .map(|i| effective_end + i + 1)
                .unwrap_or(text.len());
            (line_start.min(start_byte), line_end.max(end_byte))
        }
    } else {
        (0, text.len())
    };

    let affected = &text[affected_start..affected_end];
    let normalized = normalize_whitespace_block(affected, line_ending);
    if normalized == affected {
        return false;
    }
    let was_dirty = {
        with_state(hwnd, |state| {
            state
                .docs
                .iter()
                .find(|doc| doc.hwnd_edit == hwnd_edit)
                .map(|doc| doc.dirty)
                .unwrap_or(false)
        })
    }
    .unwrap_or(false);

    let mut replace_range = if whole_doc_selected {
        CHARRANGE {
            cpMin: 0,
            cpMax: -1,
        }
    } else {
        CHARRANGE {
            cpMin: byte_index_to_utf16(&text, affected_start),
            cpMax: byte_index_to_utf16(&text, affected_end),
        }
    };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut replace_range as *mut _ as isize),
        );
    }
    // Single-undo guarantee.
    crate::send_message_w_safe(hwnd_edit, EM_STOPGROUPTYPING, WPARAM(0), LPARAM(0));
    let result = {
        with_state(hwnd, |state| {
            state.normalize_undo = Some(NormalizeUndo {
                hwnd_edit,
                text: text.clone(),
                sel_start,
                sel_end,
                was_dirty,
            });
            state.normalize_skip_change = true;
        })
    };
    if result.is_none() {
        crate::log_debug("Failed to access editor state");
    }
    let mut set_text = SetTextEx {
        flags: ST_KEEPUNDO | ST_SELECTION,
        codepage: CP_UNICODE,
    };
    let replace_wide = to_wide(&normalized);
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_SETTEXTEX,
            WPARAM(&mut set_text as *mut _ as usize),
            LPARAM(replace_wide.as_ptr() as isize),
        );
    }
    mark_dirty_from_edit(hwnd, hwnd_edit);
    crate::set_focus_safe(hwnd_edit);
    true
}

pub fn get_selected_text(hwnd_edit: HWND) -> Option<String> {
    let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
    }
    if selection.cpMin > selection.cpMax {
        std::mem::swap(&mut selection.cpMin, &mut selection.cpMax);
    }
    if selection.cpMin == selection.cpMax {
        return None;
    }

    let selected = get_text_range(hwnd_edit, selection);
    if selected.trim().is_empty() {
        return None;
    }
    Some(selected)
}

pub fn hard_line_break_active_edit(hwnd: HWND) -> bool {
    unsafe {
        let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
            return false;
        };
        let text = get_edit_text(hwnd_edit);
        if text.is_empty() {
            return false;
        }
        let wrap_width = with_state(hwnd, |state| state.settings.wrap_width).unwrap_or(80);
        let wrap_width = wrap_width.max(1) as usize;
        let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };

        let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
        let had_selection = selection.cpMin != selection.cpMax;
        let original_caret = selection.cpMin;

        let (range_start, range_end, has_trailing_newline) = if selection.cpMin != selection.cpMax {
            let start = utf16_index_to_byte(&text, selection.cpMin);
            let end = utf16_index_to_byte(&text, selection.cpMax);
            let selected = &text[start..end];
            (start, end, selected.ends_with('\n'))
        } else {
            let Some((start_u16, end_u16, _selected, trailing)) =
                current_line_block_from_caret(hwnd_edit, selection)
            else {
                return false;
            };
            let start = utf16_index_to_byte(&text, start_u16);
            let end = utf16_index_to_byte(&text, end_u16);
            (start, end, trailing)
        };

        let target = &text[range_start..range_end];
        let reformatted = reflow_block_text(target, wrap_width, line_ending, has_trailing_newline);
        if reformatted == target {
            return false;
        }

        let mut replace_range = CHARRANGE {
            cpMin: byte_index_to_utf16(&text, range_start),
            cpMax: byte_index_to_utf16(&text, range_end),
        };
        // Single-undo guarantee.
        begin_single_undo_action(hwnd_edit);
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut replace_range as *mut _ as isize),
        );
        let replace_wide = to_wide(&reformatted);
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(replace_wide.as_ptr() as isize),
        );
        end_single_undo_action(hwnd_edit);
        restore_caret_after_line_op_if_no_selection(
            hwnd_edit,
            had_selection,
            original_caret,
            replace_range.cpMin,
            target,
            &reformatted,
        );
        mark_dirty_from_edit(hwnd, hwnd_edit);
        SetFocus(hwnd_edit);
        true
    }
}

pub fn order_items_active_edit(hwnd: HWND) -> bool {
    unsafe {
        let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
            return false;
        };
        let text = get_edit_text(hwnd_edit);
        if text.is_empty() {
            return false;
        }

        let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
        let had_selection = selection.cpMin != selection.cpMax;
        let original_caret = selection.cpMin;

        let (replace_start, replace_end, affected, has_trailing_newline) =
            if let Some((start, end, selected, trailing)) =
                selected_line_block_from_selection(hwnd_edit, &text, selection)
            {
                (start, end, selected, trailing)
            } else {
                let Some((start, end, selected, trailing)) =
                    current_line_block_from_caret(hwnd_edit, selection)
                else {
                    return false;
                };
                (start, end, selected, trailing)
            };

        let ordered = order_lines_block(&affected, line_ending, has_trailing_newline);
        if ordered == affected {
            return false;
        }

        let mut replace_range = CHARRANGE {
            cpMin: replace_start,
            cpMax: replace_end,
        };
        // Single-undo guarantee.
        begin_single_undo_action(hwnd_edit);
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut replace_range as *mut _ as isize),
        );
        let replace_wide = to_wide(&ordered);
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(replace_wide.as_ptr() as isize),
        );
        end_single_undo_action(hwnd_edit);
        restore_caret_after_line_op_if_no_selection(
            hwnd_edit,
            had_selection,
            original_caret,
            replace_start,
            &affected,
            &ordered,
        );
        mark_dirty_from_edit(hwnd, hwnd_edit);
        SetFocus(hwnd_edit);
        true
    }
}

pub fn keep_unique_items_active_edit(hwnd: HWND) -> bool {
    unsafe {
        let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
            return false;
        };
        let text = get_edit_text(hwnd_edit);
        if text.is_empty() {
            return false;
        }

        let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
        let had_selection = selection.cpMin != selection.cpMax;
        let original_caret = selection.cpMin;

        let (replace_start, replace_end, affected, has_trailing_newline) =
            if let Some((start, end, selected, trailing)) =
                selected_line_block_from_selection(hwnd_edit, &text, selection)
            {
                (start, end, selected, trailing)
            } else {
                let Some((start, end, selected, trailing)) =
                    current_line_block_from_caret(hwnd_edit, selection)
                else {
                    return false;
                };
                (start, end, selected, trailing)
            };

        let cleaned = keep_unique_lines_block(&affected, line_ending, has_trailing_newline);
        if cleaned == affected {
            return false;
        }

        let mut replace_range = CHARRANGE {
            cpMin: replace_start,
            cpMax: replace_end,
        };
        // Single-undo guarantee.
        begin_single_undo_action(hwnd_edit);
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut replace_range as *mut _ as isize),
        );
        let replace_wide = to_wide(&cleaned);
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(replace_wide.as_ptr() as isize),
        );
        end_single_undo_action(hwnd_edit);
        restore_caret_after_line_op_if_no_selection(
            hwnd_edit,
            had_selection,
            original_caret,
            replace_start,
            &affected,
            &cleaned,
        );
        mark_dirty_from_edit(hwnd, hwnd_edit);
        SetFocus(hwnd_edit);
        true
    }
}

pub fn reverse_items_active_edit(hwnd: HWND) -> bool {
    unsafe {
        let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
            return false;
        };
        let text = get_edit_text(hwnd_edit);
        if text.is_empty() {
            return false;
        }

        let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
        let had_selection = selection.cpMin != selection.cpMax;
        let original_caret = selection.cpMin;

        let (replace_start, replace_end, affected, has_trailing_newline) =
            if let Some((start, end, selected, trailing)) =
                selected_line_block_from_selection(hwnd_edit, &text, selection)
            {
                (start, end, selected, trailing)
            } else {
                let Some((start, end, selected, trailing)) =
                    current_line_block_from_caret(hwnd_edit, selection)
                else {
                    return false;
                };
                (start, end, selected, trailing)
            };

        let reversed = reverse_lines_block(&affected, line_ending, has_trailing_newline);
        if reversed == affected {
            return false;
        }

        let mut replace_range = CHARRANGE {
            cpMin: replace_start,
            cpMax: replace_end,
        };
        // Single-undo guarantee.
        begin_single_undo_action(hwnd_edit);
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut replace_range as *mut _ as isize),
        );
        let replace_wide = to_wide(&reversed);
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(replace_wide.as_ptr() as isize),
        );
        end_single_undo_action(hwnd_edit);
        restore_caret_after_line_op_if_no_selection(
            hwnd_edit,
            had_selection,
            original_caret,
            replace_start,
            &affected,
            &reversed,
        );
        mark_dirty_from_edit(hwnd, hwnd_edit);
        SetFocus(hwnd_edit);
        true
    }
}

pub fn quote_lines_active_edit(hwnd: HWND) -> bool {
    unsafe {
        let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
            return false;
        };
        let text = get_edit_text(hwnd_edit);
        if text.is_empty() {
            return false;
        }

        let quote_prefix = with_state(hwnd, |state| state.settings.quote_prefix.clone())
            .unwrap_or_else(|| "> ".to_string());
        if quote_prefix.is_empty() {
            return false;
        }

        let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
        let had_selection = selection.cpMin != selection.cpMax;
        let original_caret = selection.cpMin;

        let (replace_start, replace_end, affected, has_trailing_newline) =
            if let Some((start, end, selected, trailing)) =
                selected_line_block_from_selection(hwnd_edit, &text, selection)
            {
                (start, end, selected, trailing)
            } else {
                let Some((start, end, selected, trailing)) =
                    current_line_block_from_caret(hwnd_edit, selection)
                else {
                    return false;
                };
                (start, end, selected, trailing)
            };

        let quoted = quote_lines_block(&affected, line_ending, has_trailing_newline, &quote_prefix);
        if quoted == affected {
            return false;
        }

        let mut replace_range = CHARRANGE {
            cpMin: replace_start,
            cpMax: replace_end,
        };
        // Single-undo guarantee.
        begin_single_undo_action(hwnd_edit);
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut replace_range as *mut _ as isize),
        );
        let replace_wide = to_wide(&quoted);
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(replace_wide.as_ptr() as isize),
        );
        end_single_undo_action(hwnd_edit);
        restore_caret_after_line_op_if_no_selection(
            hwnd_edit,
            had_selection,
            original_caret,
            replace_start,
            &affected,
            &quoted,
        );
        mark_dirty_from_edit(hwnd, hwnd_edit);
        SetFocus(hwnd_edit);
        true
    }
}

pub fn unquote_lines_active_edit(hwnd: HWND) -> bool {
    unsafe {
        let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
            return false;
        };
        let text = get_edit_text(hwnd_edit);
        if text.is_empty() {
            return false;
        }

        let quote_prefix = with_state(hwnd, |state| state.settings.quote_prefix.clone())
            .unwrap_or_else(|| "> ".to_string());
        if quote_prefix.is_empty() {
            return false;
        }

        let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
        let had_selection = selection.cpMin != selection.cpMax;
        let original_caret = selection.cpMin;

        let (replace_start, replace_end, affected, has_trailing_newline) =
            if let Some((start, end, selected, trailing)) =
                selected_line_block_from_selection(hwnd_edit, &text, selection)
            {
                (start, end, selected, trailing)
            } else {
                let Some((start, end, selected, trailing)) =
                    current_line_block_from_caret(hwnd_edit, selection)
                else {
                    return false;
                };
                (start, end, selected, trailing)
            };

        let unquoted =
            unquote_lines_block(&affected, line_ending, has_trailing_newline, &quote_prefix);
        if unquoted == affected {
            return false;
        }

        let mut replace_range = CHARRANGE {
            cpMin: replace_start,
            cpMax: replace_end,
        };
        // Single-undo guarantee.
        begin_single_undo_action(hwnd_edit);
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut replace_range as *mut _ as isize),
        );
        let replace_wide = to_wide(&unquoted);
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(replace_wide.as_ptr() as isize),
        );
        end_single_undo_action(hwnd_edit);
        restore_caret_after_line_op_if_no_selection(
            hwnd_edit,
            had_selection,
            original_caret,
            replace_start,
            &affected,
            &unquoted,
        );
        mark_dirty_from_edit(hwnd, hwnd_edit);
        SetFocus(hwnd_edit);
        true
    }
}

pub fn join_lines_active_edit(hwnd: HWND) -> bool {
    unsafe {
        let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
            return false;
        };
        let text = get_edit_text(hwnd_edit);
        if text.is_empty() {
            return false;
        }

        let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
        let had_selection = selection.cpMin != selection.cpMax;
        let original_caret = selection.cpMin;

        let (replace_start, replace_end, affected, has_trailing_newline) =
            if let Some((start, end, selected, trailing)) =
                selected_line_block_from_selection(hwnd_edit, &text, selection)
            {
                (start, end, selected, trailing)
            } else {
                let Some((start, end, selected, trailing)) =
                    current_line_block_from_caret(hwnd_edit, selection)
                else {
                    return false;
                };
                (start, end, selected, trailing)
            };

        let joined = join_lines_block(&affected, line_ending, has_trailing_newline);
        if joined == affected {
            return false;
        }

        let mut replace_range = CHARRANGE {
            cpMin: replace_start,
            cpMax: replace_end,
        };
        // Single-undo guarantee.
        begin_single_undo_action(hwnd_edit);
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut replace_range as *mut _ as isize),
        );
        let replace_wide = to_wide(&joined);
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(replace_wide.as_ptr() as isize),
        );
        end_single_undo_action(hwnd_edit);
        restore_caret_after_line_op_if_no_selection(
            hwnd_edit,
            had_selection,
            original_caret,
            replace_start,
            &affected,
            &joined,
        );
        mark_dirty_from_edit(hwnd, hwnd_edit);
        SetFocus(hwnd_edit);
        true
    }
}

pub fn join_wrapped_lines_active_edit(hwnd: HWND) -> bool {
    unsafe {
        let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
            return false;
        };
        let text = get_edit_text(hwnd_edit);
        if text.is_empty() {
            return false;
        }

        let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
        let had_selection = selection.cpMin != selection.cpMax;
        let original_caret = selection.cpMin;

        // With a selection, operate on the complete selected lines. Without a
        // selection, process the whole document: this command is intended for
        // imported/OCR/transcribed text where hard line wraps affect many lines.
        let (replace_start, replace_end, affected, has_trailing_newline) = if had_selection {
            let Some((start, end, selected, trailing)) =
                selected_line_block_from_selection(hwnd_edit, &text, selection)
            else {
                return false;
            };
            (start, end, selected, trailing)
        } else {
            let text_len = SendMessageW(hwnd_edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as i32;
            (
                0,
                text_len,
                text.clone(),
                text.ends_with('\n') || text.ends_with('\r'),
            )
        };

        let joined = join_wrapped_lines_block(&affected, line_ending, has_trailing_newline);
        if joined == affected {
            return false;
        }

        let mut replace_range = CHARRANGE {
            cpMin: replace_start,
            cpMax: replace_end,
        };
        begin_single_undo_action(hwnd_edit);
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut replace_range as *mut _ as isize),
        );
        let replace_wide = to_wide(&joined);
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(replace_wide.as_ptr() as isize),
        );
        end_single_undo_action(hwnd_edit);
        restore_caret_after_line_op_if_no_selection(
            hwnd_edit,
            had_selection,
            original_caret,
            replace_start,
            &affected,
            &joined,
        );
        mark_dirty_from_edit(hwnd, hwnd_edit);
        SetFocus(hwnd_edit);
        true
    }
}

pub fn clean_end_of_line_hyphens_active_edit(hwnd: HWND) -> bool {
    unsafe {
        let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
            return false;
        };
        let text = get_edit_text(hwnd_edit);
        if text.is_empty() {
            return false;
        }

        let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );

        let (replace_start, replace_end, affected, has_trailing_newline) =
            if let Some((start, end, selected, trailing)) =
                selected_line_block_from_selection(hwnd_edit, &text, selection)
            {
                (start, end, selected, trailing)
            } else {
                (
                    0,
                    byte_index_to_utf16(&text, text.len()),
                    text.clone(),
                    text.ends_with('\n') || text.ends_with('\r'),
                )
            };

        let cleaned = clean_end_of_line_hyphens_block(&affected, line_ending, has_trailing_newline);
        if cleaned == affected {
            return false;
        }

        let mut replace_range = CHARRANGE {
            cpMin: replace_start,
            cpMax: replace_end,
        };
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut replace_range as *mut _ as isize),
        );
        // Single-undo guarantee.
        begin_single_undo_action(hwnd_edit);
        let replace_wide = to_wide(&cleaned);
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(replace_wide.as_ptr() as isize),
        );
        end_single_undo_action(hwnd_edit);
        mark_dirty_from_edit(hwnd, hwnd_edit);
        SetFocus(hwnd_edit);
        true
    }
}

pub fn text_stats_active_edit(hwnd: HWND) {
    let Some(hwnd_edit) = crate::get_active_edit(hwnd) else {
        return;
    };
    let text = get_edit_text(hwnd_edit);
    let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
    if text.is_empty() {
        let message = build_text_stats_message(language, 0, 0, 0, 0);
        crate::show_info(hwnd, language, &message);
        return;
    }

    let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
    }

    let target = if selection.cpMin != selection.cpMax {
        let start_byte = utf16_index_to_byte(&text, selection.cpMin);
        let end_byte = utf16_index_to_byte(&text, selection.cpMax);
        &text[start_byte..end_byte]
    } else {
        &text[..]
    };

    let chars_with_spaces = target.chars().count();
    let chars_without_spaces = target.chars().filter(|c| !c.is_whitespace()).count();
    let words = target.split_whitespace().count();
    let lines = if target.is_empty() {
        0
    } else {
        target.as_bytes().iter().filter(|b| **b == b'\n').count() + 1
    };
    let message = build_text_stats_message(
        language,
        chars_with_spaces,
        chars_without_spaces,
        words,
        lines,
    );
    crate::show_info(hwnd, language, &message);
    crate::set_focus_safe(hwnd_edit);
}

fn normalize_whitespace_block(text: &str, line_ending: &str) -> String {
    let mut out_lines = Vec::new();
    let mut blank_run = 0usize;
    for raw_line in text.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 2 {
                out_lines.push(String::new());
            }
        } else {
            blank_run = 0;
            out_lines.push(trimmed.to_string());
        }
    }
    out_lines.join(line_ending)
}

fn order_lines_block(text: &str, line_ending: &str, has_trailing_newline: bool) -> String {
    let (content, trailing_newline) = split_trailing_newline(text, has_trailing_newline);
    let mut lines = split_lines_any_newline(content);

    let mut nonblank_indices = Vec::new();
    let mut nonblank_lines = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if !line.trim().is_empty() {
            nonblank_indices.push(idx);
            nonblank_lines.push((line.clone(), idx));
        }
    }

    if nonblank_lines.len() > 1 {
        nonblank_lines.sort_by_key(|(line, idx)| (line.to_ascii_lowercase(), *idx));
    }

    for (slot, (line, _)) in nonblank_indices.into_iter().zip(nonblank_lines) {
        lines[slot] = line;
    }

    let mut out = lines.join(line_ending);
    if trailing_newline {
        out.push_str(line_ending);
    }
    out
}

fn keep_unique_lines_block(text: &str, line_ending: &str, has_trailing_newline: bool) -> String {
    let (content, trailing_newline) = split_trailing_newline(text, has_trailing_newline);
    let mut seen: HashSet<String> = HashSet::new();
    let mut out_lines: Vec<String> = Vec::new();

    for line in split_lines_any_newline(content) {
        if line.trim().is_empty() {
            out_lines.push(line);
            continue;
        }
        let key = line.to_ascii_lowercase();
        if seen.insert(key) {
            out_lines.push(line);
        }
    }

    let mut out = out_lines.join(line_ending);
    if trailing_newline {
        out.push_str(line_ending);
    }
    out
}

fn reverse_lines_block(text: &str, line_ending: &str, has_trailing_newline: bool) -> String {
    let (content, trailing_newline) = split_trailing_newline(text, has_trailing_newline);
    let mut lines = split_lines_any_newline(content);

    let mut nonblank_indices = Vec::new();
    let mut nonblank_lines = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if !line.trim().is_empty() {
            nonblank_indices.push(idx);
            nonblank_lines.push(line.clone());
        }
    }

    if nonblank_lines.len() > 1 {
        nonblank_lines.reverse();
    }

    for (slot, line) in nonblank_indices.into_iter().zip(nonblank_lines) {
        lines[slot] = line;
    }

    let mut out = lines.join(line_ending);
    if trailing_newline {
        out.push_str(line_ending);
    }
    out
}

fn quote_lines_block(
    text: &str,
    line_ending: &str,
    has_trailing_newline: bool,
    quote_prefix: &str,
) -> String {
    let (content, trailing_newline) = split_trailing_newline(text, has_trailing_newline);
    let mut out_lines: Vec<String> = Vec::new();

    for line in split_lines_any_newline(content) {
        if line.trim().is_empty() {
            out_lines.push(line);
        } else {
            let mut quoted = String::with_capacity(quote_prefix.len() + line.len());
            quoted.push_str(quote_prefix);
            quoted.push_str(&line);
            out_lines.push(quoted);
        }
    }

    let mut out = out_lines.join(line_ending);
    if trailing_newline {
        out.push_str(line_ending);
    }
    out
}

fn unquote_lines_block(
    text: &str,
    line_ending: &str,
    has_trailing_newline: bool,
    quote_prefix: &str,
) -> String {
    let (content, trailing_newline) = split_trailing_newline(text, has_trailing_newline);
    let mut out_lines: Vec<String> = Vec::new();

    for line in split_lines_any_newline(content) {
        if line.trim().is_empty() {
            out_lines.push(line);
        } else if let Some(rest) = line.strip_prefix(quote_prefix) {
            out_lines.push(rest.to_string());
        } else {
            out_lines.push(line);
        }
    }

    let mut out = out_lines.join(line_ending);
    if trailing_newline {
        out.push_str(line_ending);
    }
    out
}

fn join_lines_block(text: &str, line_ending: &str, has_trailing_newline: bool) -> String {
    let (content, trailing_newline) = split_trailing_newline(text, has_trailing_newline);
    let mut out = String::new();
    let mut has_content = false;

    for line in split_lines_any_newline(content) {
        if line.trim().is_empty() {
            continue;
        }
        if !has_content {
            out.push_str(&line);
            has_content = true;
            continue;
        }

        let prev_ends_ws = out.chars().last().is_some_and(|c| c.is_whitespace());
        let next_starts_ws = line.chars().next().is_some_and(|c| c.is_whitespace());
        if !prev_ends_ws && !next_starts_ws {
            let prev_is_word = out
                .chars()
                .last()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
            let next_is_word = line
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
            if prev_is_word && next_is_word {
                out.push(' ');
            }
        }
        out.push_str(&line);
    }

    if trailing_newline {
        out.push_str(line_ending);
    }
    out
}

fn split_lines_any_newline(content: &str) -> Vec<String> {
    content
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(ToString::to_string)
        .collect()
}

fn line_looks_like_list_item(line: &str) -> bool {
    let s = line.trim_start();
    if s.is_empty() {
        return false;
    }
    if s.starts_with("- ") || s.starts_with("* ") || s.starts_with("• ") {
        return true;
    }

    let mut chars = s.chars().peekable();
    let mut saw_digit = false;
    while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
        saw_digit = true;
        chars.next();
    }
    if saw_digit && matches!(chars.next(), Some('.') | Some(')')) {
        return chars.next().is_some_and(|c| c.is_whitespace());
    }
    false
}

fn append_wrapped_line(target: &mut String, next: &str) {
    let next = next.trim_start();
    if next.is_empty() {
        return;
    }

    while target.chars().last().is_some_and(|c| c.is_whitespace()) {
        target.pop();
    }

    let previous = target.chars().last();
    let first = next.chars().next();
    let no_space_after_previous =
        previous.is_some_and(|c| matches!(c, '-' | '‐' | '‑' | '/' | '\'' | '’' | '(' | '[' | '{'));
    let no_space_before_next = first
        .is_some_and(|c| matches!(c, ',' | '.' | ';' | ':' | '!' | '?' | '…' | ')' | ']' | '}'));

    if !target.is_empty() && !no_space_after_previous && !no_space_before_next {
        target.push(' ');
    }
    target.push_str(next);
}

fn join_wrapped_lines_block(text: &str, line_ending: &str, has_trailing_newline: bool) -> String {
    let (content, trailing_newline) = split_trailing_newline(text, has_trailing_newline);
    let lines = split_lines_any_newline(content);
    let mut out_lines: Vec<String> = Vec::with_capacity(lines.len());
    let mut current: Option<String> = None;

    let flush_current = |out_lines: &mut Vec<String>, current: &mut Option<String>| {
        if let Some(line) = current.take() {
            out_lines.push(line);
        }
    };

    for line in lines {
        if line.trim().is_empty() {
            flush_current(&mut out_lines, &mut current);
            out_lines.push(line);
            continue;
        }

        if line_looks_like_list_item(&line) {
            flush_current(&mut out_lines, &mut current);
            current = Some(line);
            continue;
        }

        if let Some(existing) = current.as_mut() {
            append_wrapped_line(existing, &line);
        } else {
            current = Some(line);
        }
    }

    flush_current(&mut out_lines, &mut current);

    let mut out = out_lines.join(line_ending);
    if trailing_newline {
        out.push_str(line_ending);
    }
    out
}

fn clean_end_of_line_hyphens_block(
    text: &str,
    line_ending: &str,
    has_trailing_newline: bool,
) -> String {
    let (content, trailing_newline) = split_trailing_newline(text, has_trailing_newline);
    let mut out = String::with_capacity(content.len());
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '-' {
            // Check char before '-'
            let mut valid_before = false;
            if i > 0 {
                let prev = chars[i - 1];
                if prev.is_alphanumeric() && !prev.is_whitespace() {
                    valid_before = true;
                }
            }

            if valid_before {
                // Look ahead for line breaks (up to 3)
                let mut temp_j = i + 1;
                let mut line_breaks = 0;

                while line_breaks < 3 && temp_j < chars.len() {
                    if chars[temp_j] == '\r' {
                        temp_j += 1;
                        continue;
                    }
                    if chars[temp_j] == '\n' {
                        line_breaks += 1;
                        temp_j += 1;
                        // Skip any optional \r\n that might follow (if we allow more line breaks)
                        continue;
                    }
                    // If we reach here, it's not a line break char.
                    break;
                }

                if line_breaks > 0 && temp_j < chars.len() {
                    let next_char = chars[temp_j];
                    if next_char.is_alphabetic() {
                        // Join! Skip the hyphen and the line breaks.
                        i = temp_j;
                        continue;
                    }
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }

    if trailing_newline {
        out.push_str(line_ending);
    }
    out
}

fn build_text_stats_message(
    language: crate::settings::Language,
    chars_with_spaces: usize,
    chars_without_spaces: usize,
    words: usize,
    lines: usize,
) -> String {
    let with_spaces = crate::i18n::tr_f(
        language,
        "text_stats.characters_with_spaces",
        &[("count", &chars_with_spaces.to_string())],
    );
    let without_spaces = crate::i18n::tr_f(
        language,
        "text_stats.characters_without_spaces",
        &[("count", &chars_without_spaces.to_string())],
    );
    let words = crate::i18n::tr_f(
        language,
        "text_stats.words",
        &[("count", &words.to_string())],
    );
    let lines = crate::i18n::tr_f(
        language,
        "text_stats.lines",
        &[("count", &lines.to_string())],
    );
    format!("{with_spaces}.\n{without_spaces}.\n{words}.\n{lines}.")
}

fn reflow_block_text(
    text: &str,
    wrap_width: usize,
    line_ending: &str,
    has_trailing_newline: bool,
) -> String {
    let (content, trailing_newline) = split_trailing_newline(text, has_trailing_newline);
    let mut out_lines: Vec<String> = Vec::new();
    let mut current_words: Vec<&str> = Vec::new();

    for raw_line in content.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.trim().is_empty() {
            if !current_words.is_empty() {
                out_lines.extend(wrap_words(current_words.drain(..), wrap_width));
            }
            out_lines.push(String::new());
        } else {
            current_words.extend(line.split_whitespace());
        }
    }

    if !current_words.is_empty() {
        out_lines.extend(wrap_words(current_words.drain(..), wrap_width));
    }

    let mut out = out_lines.join(line_ending);
    if trailing_newline {
        out.push_str(line_ending);
    }
    out
}

fn split_trailing_newline(text: &str, prefer_trailing: bool) -> (&str, bool) {
    if prefer_trailing && text.ends_with("\r\n") {
        return (&text[..text.len().saturating_sub(2)], true);
    }
    if prefer_trailing && text.ends_with('\n') {
        return (&text[..text.len().saturating_sub(1)], true);
    }
    if prefer_trailing && text.ends_with('\r') {
        return (&text[..text.len().saturating_sub(1)], true);
    }
    (text, false)
}

fn auto_format_tts_block(text: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }

    let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    // 1) Remove markdown markers first.
    let stripped = strip_markdown_text(text, false);
    // 2) Join wrapped lines while preserving paragraph breaks.
    let mut out = reflow_block_text(
        &stripped,
        1000,
        line_ending,
        stripped.ends_with('\n') || stripped.ends_with('\r'),
    );
    // 3) Remove common quote characters that often degrade TTS flow.
    out.retain(|c| !matches!(c, '"' | '“' | '”' | '«' | '»' | '„' | '‟' | '‹' | '›'));
    // 4) Collapse repeated blank lines.
    let mut compact = String::with_capacity(out.len());
    let mut blank_run = 0usize;
    for line in out.lines() {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                compact.push_str(line_ending);
            }
            continue;
        }
        blank_run = 0;
        if !compact.is_empty() && !compact.ends_with('\n') && !compact.ends_with('\r') {
            compact.push_str(line_ending);
        }
        compact.push_str(line.trim());
    }
    compact.trim().to_string()
}

fn wrap_words<'a, I>(words: I, wrap_width: usize) -> Vec<String>
where
    I: Iterator<Item = &'a str>,
{
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;

    for word in words {
        let word_len = word.chars().count();
        if current_len == 0 {
            current.push_str(word);
            current_len = word_len;
            continue;
        }
        if current_len + 1 + word_len <= wrap_width {
            current.push(' ');
            current.push_str(word);
            current_len += 1 + word_len;
        } else {
            lines.push(current);
            current = word.to_string();
            current_len = word_len;
        }
    }

    if current_len > 0 {
        lines.push(current);
    }
    lines
}

fn utf16_index_to_byte(text: &str, target: i32) -> usize {
    if target <= 0 {
        return 0;
    }
    let target = target as usize;
    let mut utf16_count = 0usize;
    for (byte_idx, ch) in text.char_indices() {
        let units = ch.len_utf16();
        let next = utf16_count + units;
        if target <= next {
            if target == next {
                return byte_idx + ch.len_utf8();
            }
            return byte_idx;
        }
        utf16_count = next;
    }
    text.len()
}

fn byte_index_to_utf16(text: &str, byte_idx: usize) -> i32 {
    let mut utf16_count = 0usize;
    for (idx, ch) in text.char_indices() {
        if idx >= byte_idx {
            break;
        }
        utf16_count += ch.len_utf16();
    }
    utf16_count as i32
}

fn strip_markdown_text(text: &str, keep_bullets: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (content, line_end) = if let Some(pos) = line.find('\n') {
            (&line[..pos], &line[pos..])
        } else {
            (line, "")
        };
        let mut trimmed = content.trim_start();
        if is_markdown_horizontal_rule(trimmed) {
            continue;
        }
        if trimmed.starts_with("```") {
            trimmed = trimmed.trim_start_matches('`').trim_start();
        }
        if trimmed.starts_with('#') {
            trimmed = trimmed.trim_start_matches('#').trim_start();
        }
        if trimmed.starts_with('>') {
            trimmed = trimmed.trim_start_matches('>').trim_start();
        }
        let mut line: std::borrow::Cow<'_, str> = std::borrow::Cow::Borrowed(trimmed);
        if trimmed.starts_with("- ") || trimmed.starts_with("+ ") || trimmed.starts_with("* ") {
            if keep_bullets {
                let bullet = trimmed.chars().next().unwrap_or('-');
                let rest = trimmed[1..].trim_start();
                line = std::borrow::Cow::Owned(format!("{bullet} {rest}"));
            } else {
                line = std::borrow::Cow::Borrowed(trimmed[2..].trim_start());
            }
        }
        let mut cleaned = strip_markdown_inline(line.as_ref());
        cleaned.push_str(line_end);
        out.push_str(&cleaned);
    }
    out
}

fn is_markdown_horizontal_rule(text: &str) -> bool {
    let mut marker = None;
    let mut count = 0usize;
    for ch in text.chars() {
        if ch == ' ' || ch == '\t' {
            continue;
        }
        if ch != '-' && ch != '*' && ch != '_' {
            return false;
        }
        if let Some(prev) = marker {
            if prev != ch {
                return false;
            }
        } else {
            marker = Some(ch);
        }
        count += 1;
    }
    count >= 3
}

fn strip_markdown_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(&ch) = chars.peek() {
        chars.next();
        if ch == '`' {
            continue;
        }
        if (ch == '*' || ch == '_')
            && let Some(next) = chars.peek()
            && *next == ch
        {
            chars.next();
            continue;
        }
        if ch == '~'
            && let Some(next) = chars.peek()
            && *next == '~'
        {
            chars.next();
            continue;
        }
        if ch == '!' && chars.peek() == Some(&'[') {
            chars.next();
            let alt = collect_bracket_text(&mut chars, ']');
            if chars.peek() == Some(&'(') {
                chars.next();
                collect_bracket_text(&mut chars, ')');
            }
            out.push_str(&alt);
            continue;
        }
        if ch == '[' {
            let label = collect_bracket_text(&mut chars, ']');
            if chars.peek() == Some(&'(') {
                chars.next();
                collect_bracket_text(&mut chars, ')');
                out.push_str(&label);
                continue;
            }
            out.push('[');
            out.push_str(&label);
            continue;
        }
        out.push(ch);
    }
    out
}

fn collect_bracket_text<I: Iterator<Item = char>>(
    chars: &mut std::iter::Peekable<I>,
    end: char,
) -> String {
    let mut out = String::new();
    for ch in chars.by_ref() {
        if ch == end {
            break;
        }
        out.push(ch);
    }
    out
}

// --- Document Management ---

pub fn new_document(hwnd: HWND) {
    {
        let new_index = with_state(hwnd, |state| {
            state.untitled_count += 1;
            let language = state.settings.language;
            let title = untitled_title(language, state.untitled_count);
            let hwnd_edit = create_edit(
                hwnd,
                state.hfont,
                state.settings.word_wrap,
                state.settings.text_color,
                state.settings.text_size,
            );
            let doc = Document {
                title: title.clone(),
                path: None,
                hwnd_edit,
                dirty: false,
                format: FileFormat::Text(TextEncoding::Utf8),
                opened_text_encoding: None,
                current_save_text_encoding: None,
                from_rss: false,
                from_italiaonline: false,
                from_find_in_files: false,
                from_wikipedia: false,
                from_treccani: false,
                is_temporary: false,
                confirm_close_when_clean: false,
                prefer_title_for_save_suggestion: false,
                prefer_mpv_playback: false,
                route_map: None,
                epub_index: Vec::new(),
                epub_original_text: None,
                pdf_is_imported_source: false,
            };
            state.docs.push(doc);
            insert_tab(state.hwnd_tab, &title, (state.docs.len() - 1) as i32);
            state.docs.len() - 1
        })
        .unwrap_or(0);
        select_tab(hwnd, new_index);
    }
}

pub fn ensure_audio_document_tab(hwnd: HWND, path: &Path) -> Option<usize> {
    unsafe {
        with_state(hwnd, |state| {
            if let Some((index, _)) = state.docs.iter().enumerate().find(|(_, doc)| {
                matches!(doc.format, FileFormat::Audiobook)
                    && doc.path.as_deref().map(|p| p == path).unwrap_or(false)
            }) {
                return index;
            }

            let title = path.file_name().and_then(|s| s.to_str()).unwrap_or("File");
            let hwnd_edit = create_edit(
                hwnd,
                state.hfont,
                state.settings.word_wrap,
                state.settings.text_color,
                state.settings.text_size,
            );
            set_edit_text(hwnd_edit, "");

            let doc = Document {
                title: title.to_string(),
                path: Some(path.to_path_buf()),
                hwnd_edit,
                dirty: false,
                format: FileFormat::Audiobook,
                opened_text_encoding: None,
                current_save_text_encoding: None,
                from_rss: false,
                from_italiaonline: false,
                from_find_in_files: false,
                from_wikipedia: false,
                from_treccani: false,
                is_temporary: false,
                confirm_close_when_clean: false,
                prefer_title_for_save_suggestion: false,
                prefer_mpv_playback: false,
                route_map: None,
                epub_index: Vec::new(),
                epub_original_text: None,
                pdf_is_imported_source: false,
            };
            SendMessageW(hwnd_edit, EM_SETREADONLY, WPARAM(1), LPARAM(0));
            ShowWindow(hwnd_edit, SW_HIDE);
            state.docs.push(doc);
            insert_tab(state.hwnd_tab, title, (state.docs.len() - 1) as i32);
            state.docs.len() - 1
        })
    }
}

pub fn retarget_current_audiobook_document(hwnd: HWND, path: &Path, title: &str) -> bool {
    let changed = with_state(hwnd, |state| {
        let index = state.current;
        let document = state.docs.get_mut(index)?;
        if !matches!(document.format, FileFormat::Audiobook) {
            return None;
        }
        document.path = Some(path.to_path_buf());
        document.title = title.to_string();
        document.dirty = false;
        Some((state.hwnd_tab, index))
    })
    .flatten();
    let Some((hwnd_tab, index)) = changed else {
        return false;
    };
    update_tab_title(hwnd_tab, index, title, false);
    update_window_title(hwnd);
    true
}

pub fn current_daisy_document_path(hwnd: HWND) -> Option<PathBuf> {
    with_state(hwnd, |state| {
        state.docs.get(state.current).and_then(|document| {
            matches!(document.format, FileFormat::Daisy)
                .then(|| document.path.clone())
                .flatten()
        })
    })
    .flatten()
}

fn open_document_with_encoding_internal(
    hwnd: HWND,
    path: &Path,
    user_encoding: Option<TextEncoding>,
    from_copydata: bool,
) {
    unsafe {
        // Record telemetry for hang diagnostics
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown");
        crate::telemetry::record_action("file_open", ext);

        log_debug(&format!(
            "Open document: {} (encoding: {:?})",
            path.display(),
            user_encoding
        ));

        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
        if is_pdf_path(path) {
            crate::open_pdf_document_async(hwnd, path, from_copydata);
            return;
        }
        if is_audio_path(path) {
            crate::queue_audio_files_and_play(hwnd, vec![path.to_path_buf()]);
            return;
        }

        let path_buf = path.to_path_buf();
        let hwnd_main = hwnd;
        std::thread::spawn(move || {
            let result = load_document_content(&path_buf, user_encoding, language);
            let payload = Box::new(DocumentLoadResult {
                path: path_buf,
                result,
            });
            let payload_ptr = Box::into_raw(payload);
            if let Err(e) = PostMessageW(
                hwnd_main,
                crate::WM_DOCUMENT_LOADED,
                WPARAM(0),
                LPARAM(payload_ptr as isize),
            ) {
                crate::log_debug(&format!("Failed to post WM_DOCUMENT_LOADED: {}", e));
                let _unused_box = Box::from_raw(payload_ptr);
            }
        });
    }
}

fn load_document_content(
    path: &Path,
    user_encoding: Option<TextEncoding>,
    language: Language,
) -> Result<Option<LoadedDocument>, String> {
    if is_docx_path(path) {
        let text = read_docx_text(path, language)?;
        return Ok(Some(LoadedDocument {
            content: text,
            format: FileFormat::Docx,
            opened_text_encoding: None,
            epub_index: Vec::new(),
            epub_original_text: None,
        }));
    }
    if is_odt_path(path) {
        let text = read_odt_text(path, language)?;
        return Ok(Some(LoadedDocument {
            content: text,
            format: FileFormat::Odt,
            opened_text_encoding: None,
            epub_index: Vec::new(),
            epub_original_text: None,
        }));
    }
    if is_pptx_path(path) {
        let text = read_ppt_text(path, language)?;
        return Ok(Some(LoadedDocument {
            content: text,
            format: FileFormat::Pptx,
            opened_text_encoding: None,
            epub_index: Vec::new(),
            epub_original_text: None,
        }));
    }
    if is_ppt_path(path) {
        let text = read_ppt_text(path, language)?;
        return Ok(Some(LoadedDocument {
            content: text,
            format: FileFormat::Ppt,
            opened_text_encoding: None,
            epub_index: Vec::new(),
            epub_original_text: None,
        }));
    }
    if is_odp_path(path) {
        let text = read_odp_text(path, language)?;
        return Ok(Some(LoadedDocument {
            content: text,
            format: FileFormat::Odp,
            opened_text_encoding: None,
            epub_index: Vec::new(),
            epub_original_text: None,
        }));
    }
    if is_kindle_path(path) {
        let document = read_kindle_document(path, language)?;
        return Ok(Some(LoadedDocument {
            content: document.text,
            format: FileFormat::KindleEbook,
            opened_text_encoding: None,
            epub_index: document.index,
            epub_original_text: None,
        }));
    }
    if is_daisy_path(path) {
        let document = read_daisy_document(path, language)?;
        return Ok(Some(LoadedDocument {
            content: document.text,
            format: FileFormat::Daisy,
            opened_text_encoding: None,
            epub_index: document.index,
            epub_original_text: None,
        }));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        return Err(crate::settings::error_open_file_message(
            language,
            "The ZIP archive is not a recognized DAISY Digital Talking Book.",
        ));
    }
    if is_epub_path(path) {
        let document = read_epub_document(path, language)?;
        let original_text = document.text.clone();
        return Ok(Some(LoadedDocument {
            content: document.text,
            format: FileFormat::Epub,
            opened_text_encoding: None,
            epub_index: document.index,
            epub_original_text: Some(original_text),
        }));
    }
    if is_html_path(path) {
        let (text, _encoding) = read_html_text(path, language)?;
        return Ok(Some(LoadedDocument {
            content: text,
            format: FileFormat::Html,
            opened_text_encoding: None,
            epub_index: Vec::new(),
            epub_original_text: None,
        }));
    }
    let is_rtf = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("rtf"))
        .unwrap_or(false);
    if is_rtf {
        let bytes = std::fs::read(path)
            .map_err(|err| crate::settings::error_open_file_message(language, err))?;
        return Ok(Some(LoadedDocument {
            content: extract_rtf_text(&bytes),
            format: FileFormat::Text(TextEncoding::Utf8),
            opened_text_encoding: None,
            epub_index: Vec::new(),
            epub_original_text: None,
        }));
    }
    if is_gdoc_path(path) {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) => {
                crate::log_debug(&format!(
                    "Failed to read Google pointer file '{}': {}. Trying shell fallback.",
                    path.display(),
                    err
                ));
                match crate::audio_utils::open_path_with_default_app(path) {
                    Ok(()) => return Ok(None),
                    Err(shell_err) => {
                        crate::log_debug(&format!(
                            "Google pointer fallback launch failed for '{}': {}",
                            path.display(),
                            shell_err
                        ));
                        return Err(crate::settings::error_open_file_message(language, err));
                    }
                }
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        if let Some(url_pos) = text.find("\"url\"") {
            let after_url = &text[url_pos + 5..];
            if let Some(start_quote) = after_url.find('\"') {
                let from_quote = &after_url[start_quote + 1..];
                if let Some(end_quote) = from_quote.find('\"') {
                    let url = &from_quote[..end_quote];
                    if url.starts_with("http") {
                        if let Err(e) = crate::audio_utils::open_url_in_browser(url) {
                            crate::log_debug(&format!("Failed to open GDoc URL: {}", e));
                        }
                        return Ok(None);
                    }
                }
            }
        }
        let (text, encoding) = decode_text(&bytes, language)?;
        return Ok(Some(LoadedDocument {
            content: text,
            format: FileFormat::Text(encoding),
            opened_text_encoding: Some(encoding),
            epub_index: Vec::new(),
            epub_original_text: None,
        }));
    }
    if is_doc_path(path) {
        let text = read_doc_text(path, language)?;
        return Ok(Some(LoadedDocument {
            content: text,
            format: FileFormat::Doc,
            opened_text_encoding: None,
            epub_index: Vec::new(),
            epub_original_text: None,
        }));
    }
    if is_spreadsheet_path(path) {
        let text = read_spreadsheet_text(path, language)?;
        return Ok(Some(LoadedDocument {
            content: text,
            format: FileFormat::Spreadsheet,
            opened_text_encoding: None,
            epub_index: Vec::new(),
            epub_original_text: None,
        }));
    }

    let bytes = std::fs::read(path)
        .map_err(|err| crate::settings::error_open_file_message(language, err))?;
    if let Some(encoding) = user_encoding {
        let text = decode_text_with_encoding(&bytes, encoding, language)?;
        return Ok(Some(LoadedDocument {
            content: text,
            format: FileFormat::Text(encoding),
            opened_text_encoding: Some(encoding),
            epub_index: Vec::new(),
            epub_original_text: None,
        }));
    }
    let (text, encoding) = decode_text(&bytes, language)?;
    Ok(Some(LoadedDocument {
        content: text,
        format: FileFormat::Text(encoding),
        opened_text_encoding: Some(encoding),
        epub_index: Vec::new(),
        epub_original_text: None,
    }))
}

pub fn open_document_with_encoding(hwnd: HWND, path: &Path, user_encoding: Option<TextEncoding>) {
    open_document_with_encoding_internal(hwnd, path, user_encoding, false);
}

pub fn open_document_with_encoding_from_copydata(
    hwnd: HWND,
    path: &Path,
    user_encoding: Option<TextEncoding>,
) {
    open_document_with_encoding_internal(hwnd, path, user_encoding, true);
}

pub fn open_document(hwnd: HWND, path: &Path) {
    open_document_with_encoding(hwnd, path, None);
}

pub fn open_document_from_copydata(hwnd: HWND, path: &Path) {
    open_document_with_encoding_from_copydata(hwnd, path, None);
}

/// Returns true if the current document is an audiobook (player mode).
/// Use this to avoid showing/focusing the editor when in player mode.
pub fn is_current_audiobook(hwnd: HWND) -> bool {
    {
        with_state(hwnd, |state| {
            state
                .docs
                .get(state.current)
                .map(|doc| matches!(doc.format, FileFormat::Audiobook))
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }
}

pub fn mark_current_document_from_rss(hwnd: HWND, from_rss: bool) {
    let result = {
        with_state(hwnd, |state| {
            if let Some(doc) = state.docs.get_mut(state.current) {
                doc.from_rss = from_rss;
            }
        })
    };
    if result.is_none() {
        crate::log_debug("Failed to access editor state");
    }
}

pub fn mark_current_document_from_italiaonline(hwnd: HWND, from_italiaonline: bool) {
    let result = {
        with_state(hwnd, |state| {
            if let Some(doc) = state.docs.get_mut(state.current) {
                doc.from_italiaonline = from_italiaonline;
            }
        })
    };
    if result.is_none() {
        crate::log_debug("Failed to access editor state");
    }
}

pub fn mark_current_document_from_wikipedia(hwnd: HWND, from_wikipedia: bool) {
    let result = {
        with_state(hwnd, |state| {
            if let Some(doc) = state.docs.get_mut(state.current) {
                doc.from_wikipedia = from_wikipedia;
                if from_wikipedia {
                    doc.from_treccani = false;
                }
            }
        })
    };
    if result.is_none() {
        crate::log_debug("Failed to access editor state");
    }
}

pub fn mark_current_document_from_treccani(hwnd: HWND, from_treccani: bool) {
    let result = with_state(hwnd, |state| {
        if let Some(doc) = state.docs.get_mut(state.current) {
            doc.from_treccani = from_treccani;
            if from_treccani {
                doc.from_wikipedia = false;
            }
        }
    });
    if result.is_none() {
        crate::log_debug("Failed to access editor state");
    }
}

pub fn mark_current_document_prefer_mpv_playback(hwnd: HWND, prefer_mpv_playback: bool) {
    let result = {
        with_state(hwnd, |state| {
            if let Some(doc) = state.docs.get_mut(state.current) {
                doc.prefer_mpv_playback = prefer_mpv_playback;
            }
        })
    };
    if result.is_none() {
        crate::log_debug("Failed to access editor state");
    }
}

pub fn mark_current_document_dirty_prefer_title(hwnd: HWND) {
    let updated = with_state(hwnd, |state| {
        let index = state.current;
        if index >= state.docs.len() {
            return false;
        }
        state.docs[index].dirty = true;
        state.docs[index].prefer_title_for_save_suggestion = true;
        update_tab_title(
            state.hwnd_tab,
            index,
            &state.docs[index].title,
            state.docs[index].dirty,
        );
        true
    })
    .unwrap_or(false);

    if updated {
        update_window_title(hwnd);
    } else {
        crate::log_debug("Failed to mark current document dirty");
    }
}

pub fn set_current_document_title(hwnd: HWND, title: &str) {
    let new_title = title.trim();
    if new_title.is_empty() {
        return;
    }

    let updated = with_state(hwnd, |state| {
        let index = state.current;
        if index >= state.docs.len() {
            return false;
        }
        state.docs[index].title = new_title.to_string();
        update_tab_title(
            state.hwnd_tab,
            index,
            &state.docs[index].title,
            state.docs[index].dirty,
        );
        true
    })
    .unwrap_or(false);

    if updated {
        update_window_title(hwnd);
    } else {
        crate::log_debug("Failed to set current document title");
    }
}

pub fn set_current_route_map(hwnd: HWND, route_map: RouteMapData) {
    let updated = with_state(hwnd, |state| {
        let index = state.current;
        if index >= state.docs.len() {
            return false;
        }
        state.docs[index].route_map = Some(route_map);
        true
    })
    .unwrap_or(false);

    if !updated {
        crate::log_debug("Failed to set current route map");
    }
}

pub fn current_route_map(hwnd: HWND) -> Option<RouteMapData> {
    with_state(hwnd, |state| {
        state
            .docs
            .get(state.current)
            .and_then(|doc| doc.route_map.clone())
    })
    .flatten()
}

pub fn current_document_has_route_map(hwnd: HWND) -> bool {
    with_state(hwnd, |state| {
        state
            .docs
            .get(state.current)
            .and_then(|doc| doc.route_map.as_ref())
            .is_some()
    })
    .unwrap_or(false)
}

pub fn current_document_is_from_rss(hwnd: HWND) -> bool {
    {
        with_state(hwnd, |state| {
            state
                .docs
                .get(state.current)
                .map(|doc| doc.from_rss)
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }
}

pub fn current_document_is_from_italiaonline(hwnd: HWND) -> bool {
    {
        with_state(hwnd, |state| {
            state
                .docs
                .get(state.current)
                .map(|doc| doc.from_italiaonline)
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }
}

pub fn current_document_is_from_find_in_files(hwnd: HWND) -> bool {
    {
        with_state(hwnd, |state| {
            state
                .docs
                .get(state.current)
                .map(|doc| doc.from_find_in_files)
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }
}

pub fn current_document_is_from_wikipedia(hwnd: HWND) -> bool {
    {
        with_state(hwnd, |state| {
            state
                .docs
                .get(state.current)
                .map(|doc| doc.from_wikipedia)
                .unwrap_or(false)
        })
        .unwrap_or(false)
    }
}

pub fn current_document_is_from_treccani(hwnd: HWND) -> bool {
    with_state(hwnd, |state| {
        state
            .docs
            .get(state.current)
            .map(|doc| doc.from_treccani)
            .unwrap_or(false)
    })
    .unwrap_or(false)
}

pub fn get_or_create_rss_document(hwnd: HWND, title: &str) -> Option<HWND> {
    {
        let (index, hwnd_edit) = with_state(hwnd, |state| {
            if let Some((idx, doc)) = state
                .docs
                .iter()
                .enumerate()
                .find(|(_i, doc)| doc.from_rss && doc.is_temporary)
            {
                return Some((idx, doc.hwnd_edit));
            }
            let hwnd_edit = create_edit(
                hwnd,
                state.hfont,
                state.settings.word_wrap,
                state.settings.text_color,
                state.settings.text_size,
            );
            let doc = Document {
                title: title.to_string(),
                path: None,
                hwnd_edit,
                dirty: false,
                format: FileFormat::Text(TextEncoding::Utf8),
                opened_text_encoding: None,
                current_save_text_encoding: None,
                from_rss: true,
                from_italiaonline: false,
                from_find_in_files: false,
                from_wikipedia: false,
                from_treccani: false,
                is_temporary: true,
                confirm_close_when_clean: false,
                prefer_title_for_save_suggestion: false,
                prefer_mpv_playback: false,
                route_map: None,
                epub_index: Vec::new(),
                epub_original_text: None,
                pdf_is_imported_source: false,
            };
            state.docs.push(doc);
            insert_tab(state.hwnd_tab, title, (state.docs.len() - 1) as i32);
            Some((state.docs.len() - 1, hwnd_edit))
        })
        .flatten()?;
        select_tab(hwnd, index);
        Some(hwnd_edit)
    }
}

pub fn current_epub_index(hwnd: HWND) -> Vec<EpubIndexEntry> {
    with_state(hwnd, |state| {
        state
            .docs
            .get(state.current)
            .map(|document| document.epub_index.clone())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

pub fn current_document_has_epub_index(hwnd: HWND) -> bool {
    with_state(hwnd, |state| {
        state
            .docs
            .get(state.current)
            .map(|document| {
                matches!(
                    document.format,
                    FileFormat::Epub | FileFormat::KindleEbook | FileFormat::Daisy
                ) && !document.epub_index.is_empty()
            })
            .unwrap_or(false)
    })
    .unwrap_or(false)
}

pub fn go_to_current_document_utf16_offset(hwnd: HWND, target_utf16: i32) -> bool {
    let hwnd_edit = with_state(hwnd, |state| {
        state
            .docs
            .get(state.current)
            .map(|document| document.hwnd_edit)
    })
    .flatten()
    .unwrap_or(HWND(0));
    if hwnd_edit.0 == 0 {
        return false;
    }

    unsafe {
        let target = target_utf16.max(0);
        let mut selection = CHARRANGE {
            cpMin: target,
            cpMax: target,
        };
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
        SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
        SetFocus(hwnd_edit);
    }
    crate::notify_editor_caret_changed(hwnd_edit);
    true
}

pub fn select_tab(hwnd: HWND, index: usize) {
    unsafe {
        crate::log_debug(&format!("Editor: select_tab called for index {}", index));
        let result = with_state(hwnd, |state| {
            if index >= state.docs.len() {
                return None;
            }
            let prev = state.current;
            let prev_edit = state.docs.get(prev).map(|doc| doc.hwnd_edit);
            let new_doc = state.docs.get(index);
            let new_edit = new_doc.map(|doc| doc.hwnd_edit);
            let is_audiobook = new_doc
                .map(|doc| matches!(doc.format, FileFormat::Audiobook))
                .unwrap_or(false);
            crate::log_debug(&format!(
                "Editor: Switching from index {} to {}. Is audiobook: {}",
                prev, index, is_audiobook
            ));
            state.current = index;
            Some((state.hwnd_tab, prev_edit, new_edit, is_audiobook))
        })
        .flatten();

        let Some((hwnd_tab, prev_edit, new_edit, is_audiobook)) = result else {
            return;
        };

        if !is_audiobook
            && with_state(hwnd, |state| state.local_mpv_video_mode_active).unwrap_or(false)
        {
            crate::set_local_mpv_video_mode(hwnd, false);
        }

        if let Some(hwnd_edit) = prev_edit {
            ShowWindow(hwnd_edit, SW_HIDE);
        }
        SendMessageW(hwnd_tab, TCM_SETCURSEL, WPARAM(index), LPARAM(0));
        if let Some(hwnd_edit) = new_edit {
            if is_audiobook {
                ShowWindow(hwnd_edit, SW_HIDE);
                SetFocus(hwnd_tab);
            } else {
                ShowWindow(hwnd_edit, SW_SHOW);
                SetFocus(hwnd_edit);
            }
        }
        update_window_title(hwnd);
        crate::menu::update_playback_menu(hwnd, is_audiobook);
        layout_children(hwnd);
        crate::sync_voice_panel_visibility_for_current_document(hwnd);
        crate::update_main_status_bar(hwnd);
        crate::restore_transcription_progress_focus_for_current_document(hwnd);
    }
}

pub fn insert_tab(hwnd_tab: HWND, title: &str, index: i32) {
    unsafe {
        let mut text = to_wide(title);
        let mut item = TCITEMW {
            mask: TCIF_TEXT,
            pszText: PWSTR(text.as_mut_ptr()),
            ..Default::default()
        };
        SendMessageW(
            hwnd_tab,
            TCM_INSERTITEMW,
            WPARAM(index as usize),
            LPARAM(&mut item as *mut _ as isize),
        );
    }
}

pub fn update_tab_title(hwnd_tab: HWND, index: usize, title: &str, dirty: bool) {
    unsafe {
        let label = if dirty {
            format!("{title}*")
        } else {
            title.to_string()
        };
        let mut text = to_wide(&label);
        let mut item = TCITEMW {
            mask: TCIF_TEXT,
            pszText: PWSTR(text.as_mut_ptr()),
            ..Default::default()
        };
        SendMessageW(
            hwnd_tab,
            TCM_SETITEMW,
            WPARAM(index),
            LPARAM(&mut item as *mut _ as isize),
        );
    }
}

pub fn mark_dirty_from_edit(hwnd: HWND, hwnd_edit: HWND) {
    {
        if with_state(hwnd, |state| {
            for (i, doc) in state.docs.iter_mut().enumerate() {
                if doc.hwnd_edit == hwnd_edit && !doc.dirty {
                    doc.dirty = true;
                    update_tab_title(state.hwnd_tab, i, &doc.title, true);
                    update_window_title(hwnd);
                    break;
                }
            }
        })
        .is_none()
        {
            crate::log_debug("Failed to access editor state");
        }
    }
}

pub fn update_window_title(hwnd: HWND) {
    unsafe {
        if with_state(hwnd, |state| {
            if let Some(doc) = state.docs.get(state.current) {
                let display_title = &doc.title;
                let app_name = crate::settings::app_display_name(&state.settings);
                let base_title = if display_title.trim().is_empty() {
                    app_name.to_string()
                } else {
                    format!("{display_title} - {app_name}")
                };
                let full_title = apply_modified_marker(
                    &base_title,
                    doc.dirty,
                    state.settings.modified_marker_position,
                );
                let wide = to_wide(&full_title);
                if let Err(e) = SetWindowTextW(hwnd, PCWSTR(wide.as_ptr())) {
                    crate::log_debug(&format!("Failed to set window title: {}", e));
                }
            }
        })
        .is_none()
        {
            crate::log_debug("Failed to access editor state");
        }
    }
}

fn apply_modified_marker(title: &str, dirty: bool, position: ModifiedMarkerPosition) -> String {
    if !dirty {
        return title.to_string();
    }
    match position {
        ModifiedMarkerPosition::Beginning => format!("* {title}"),
        _ => format!("{title} *"),
    }
}

pub fn layout_children(hwnd: HWND) {
    unsafe {
        let state_data = with_state(hwnd, |state| {
            (
                state.hwnd_tab,
                state.hwnd_status,
                state.docs.iter().map(|d| d.hwnd_edit).collect::<Vec<_>>(),
                state
                    .docs
                    .get(state.current)
                    .map(|doc| matches!(doc.format, FileFormat::Audiobook))
                    .unwrap_or(false),
                state.voice_panel_visible,
                state.voice_favorites_visible,
                state.settings.tts_engine,
                state.settings.tts_only_multilingual,
                state.voice_label_engine,
                state.voice_combo_engine,
                state.voice_label_language,
                state.voice_combo_language,
                state.voice_label_voice,
                state.voice_combo_voice,
                state.voice_button_insert_tag,
                state.voice_button_manage_google_voices,
                state.voice_button_insert_pause,
                state.voice_label_speed,
                state.voice_combo_speed,
                state.voice_edit_speed,
                state.voice_label_pitch,
                state.voice_combo_pitch,
                state.voice_edit_pitch,
                state.voice_label_volume,
                state.voice_combo_volume,
                state.voice_edit_volume,
                state.voice_checkbox_multilingual,
                state.voice_label_favorites,
                state.voice_combo_favorites,
            )
        });
        if state_data.is_none() {
            crate::log_debug("Failed to access editor state");
            return;
        }

        let Some((
            hwnd_tab,
            hwnd_status,
            edit_handles,
            current_is_audiobook,
            voice_panel_visible,
            favorites_visible,
            tts_engine,
            tts_only_multilingual,
            label_engine,
            combo_engine,
            label_language,
            combo_language,
            label_voice,
            combo_voice,
            button_insert_tag,
            button_manage_google_voices,
            button_insert_pause,
            label_speed,
            combo_speed,
            edit_speed,
            label_pitch,
            combo_pitch,
            edit_pitch,
            label_volume,
            combo_volume,
            edit_volume,
            checkbox_multilingual,
            label_favorites,
            combo_favorites,
        )) = state_data
        else {
            return;
        };

        let mut rc = RECT::default();
        if GetClientRect(hwnd, &mut rc).is_err() {
            return;
        }

        let width = rc.right - rc.left;
        let height = rc.bottom - rc.top;
        let status_height = if hwnd_status.0 != 0 { 22 } else { 0 };
        let tab_height = (height - status_height).max(0);

        crate::log_if_err!(MoveWindow(hwnd_tab, 0, 0, width, tab_height, true));
        if hwnd_status.0 != 0 {
            crate::log_if_err!(MoveWindow(
                hwnd_status,
                0,
                tab_height,
                width,
                status_height,
                true
            ));
        }

        let mut tab_rc = rc;
        SendMessageW(
            hwnd_tab,
            TCM_ADJUSTRECT,
            WPARAM(0),
            LPARAM(&mut tab_rc as *mut _ as isize),
        );

        let voice_panel_visible = voice_panel_visible && !current_is_audiobook;
        let favorites_visible = favorites_visible && !current_is_audiobook;
        let mut panel_height = 0;
        let panel_visible = voice_panel_visible || favorites_visible;
        if panel_visible {
            let show_multilingual =
                voice_panel_visible && matches!(tts_engine, crate::settings::TtsEngine::Edge);
            let show_language = voice_panel_visible
                && matches!(tts_engine, crate::settings::TtsEngine::Edge)
                && !tts_only_multilingual;
            let mut rows = 0;
            if voice_panel_visible {
                rows += 6;
                if show_multilingual {
                    rows += 1;
                }
                if show_language {
                    rows += 1;
                }
            }
            if favorites_visible {
                rows += 1;
            }
            panel_height = VOICE_PANEL_PADDING * 2
                + VOICE_PANEL_ROW_HEIGHT * rows
                + VOICE_PANEL_SPACING * (rows - 1);
            let label_x = tab_rc.left + VOICE_PANEL_PADDING;
            let combo_x = label_x + VOICE_PANEL_LABEL_WIDTH + VOICE_PANEL_PADDING;
            let combo_width = (tab_rc.right - VOICE_PANEL_PADDING) - combo_x;
            let combo_width = if combo_width < 120 { 120 } else { combo_width };
            let combo_voice_width =
                (combo_width - (VOICE_PANEL_BUTTON_WIDTH + VOICE_PANEL_PADDING)).max(120);
            let button_voice_x = combo_x + combo_voice_width + VOICE_PANEL_PADDING;
            let row1_top = tab_rc.top + VOICE_PANEL_PADDING;
            let row2_top = row1_top + VOICE_PANEL_ROW_HEIGHT + VOICE_PANEL_SPACING;
            let row3_top = row2_top + VOICE_PANEL_ROW_HEIGHT + VOICE_PANEL_SPACING;
            let row4_top = row3_top + VOICE_PANEL_ROW_HEIGHT + VOICE_PANEL_SPACING;
            let row5_top = row4_top + VOICE_PANEL_ROW_HEIGHT + VOICE_PANEL_SPACING;
            let row6_top = row5_top + VOICE_PANEL_ROW_HEIGHT + VOICE_PANEL_SPACING;
            let row7_top = row6_top + VOICE_PANEL_ROW_HEIGHT + VOICE_PANEL_SPACING;
            let row8_top = row7_top + VOICE_PANEL_ROW_HEIGHT + VOICE_PANEL_SPACING;

            if voice_panel_visible {
                crate::log_if_err!(MoveWindow(
                    label_engine,
                    label_x,
                    row1_top,
                    VOICE_PANEL_LABEL_WIDTH,
                    VOICE_PANEL_ROW_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    combo_engine,
                    combo_x,
                    row1_top - 2,
                    combo_width,
                    VOICE_PANEL_COMBO_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    label_language,
                    label_x,
                    row2_top,
                    VOICE_PANEL_LABEL_WIDTH,
                    VOICE_PANEL_ROW_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    combo_language,
                    combo_x,
                    row2_top - 2,
                    combo_width,
                    VOICE_PANEL_COMBO_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    label_voice,
                    label_x,
                    if show_language { row3_top } else { row2_top },
                    VOICE_PANEL_LABEL_WIDTH,
                    VOICE_PANEL_ROW_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    combo_voice,
                    combo_x,
                    (if show_language { row3_top } else { row2_top }) - 2,
                    combo_voice_width,
                    VOICE_PANEL_COMBO_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    button_insert_tag,
                    button_voice_x,
                    if show_language { row3_top } else { row2_top },
                    VOICE_PANEL_BUTTON_WIDTH,
                    VOICE_PANEL_ROW_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    button_manage_google_voices,
                    combo_x,
                    if show_language { row4_top } else { row3_top },
                    combo_voice_width,
                    VOICE_PANEL_ROW_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    button_insert_pause,
                    button_voice_x,
                    if show_language { row4_top } else { row3_top },
                    VOICE_PANEL_BUTTON_WIDTH,
                    VOICE_PANEL_ROW_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    label_speed,
                    label_x,
                    if show_language { row5_top } else { row4_top },
                    VOICE_PANEL_LABEL_WIDTH,
                    VOICE_PANEL_ROW_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    combo_speed,
                    combo_x,
                    (if show_language { row5_top } else { row4_top }) - 2,
                    combo_width,
                    VOICE_PANEL_COMBO_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    edit_speed,
                    combo_x,
                    (if show_language { row5_top } else { row4_top }) - 2,
                    combo_width,
                    VOICE_PANEL_COMBO_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    label_pitch,
                    label_x,
                    if show_language { row6_top } else { row5_top },
                    VOICE_PANEL_LABEL_WIDTH,
                    VOICE_PANEL_ROW_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    combo_pitch,
                    combo_x,
                    (if show_language { row6_top } else { row5_top }) - 2,
                    combo_width,
                    VOICE_PANEL_COMBO_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    edit_pitch,
                    combo_x,
                    (if show_language { row6_top } else { row5_top }) - 2,
                    combo_width,
                    VOICE_PANEL_COMBO_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    label_volume,
                    label_x,
                    if show_language { row7_top } else { row6_top },
                    VOICE_PANEL_LABEL_WIDTH,
                    VOICE_PANEL_ROW_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    combo_volume,
                    combo_x,
                    (if show_language { row7_top } else { row6_top }) - 2,
                    combo_width,
                    VOICE_PANEL_COMBO_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    edit_volume,
                    combo_x,
                    (if show_language { row7_top } else { row6_top }) - 2,
                    combo_width,
                    VOICE_PANEL_COMBO_HEIGHT,
                    true,
                ));
                if show_multilingual {
                    let checkbox_row = if show_language { row8_top } else { row7_top };
                    crate::log_if_err!(MoveWindow(
                        checkbox_multilingual,
                        label_x,
                        checkbox_row,
                        combo_width + VOICE_PANEL_LABEL_WIDTH + VOICE_PANEL_PADDING,
                        VOICE_PANEL_ROW_HEIGHT,
                        true,
                    ));
                    if favorites_visible {
                        let favorites_row = if show_language {
                            row8_top + VOICE_PANEL_ROW_HEIGHT + VOICE_PANEL_SPACING
                        } else {
                            row8_top
                        };
                        crate::log_if_err!(MoveWindow(
                            label_favorites,
                            label_x,
                            favorites_row,
                            VOICE_PANEL_LABEL_WIDTH,
                            VOICE_PANEL_ROW_HEIGHT,
                            true,
                        ));
                        crate::log_if_err!(MoveWindow(
                            combo_favorites,
                            combo_x,
                            favorites_row - 2,
                            combo_width,
                            VOICE_PANEL_COMBO_HEIGHT,
                            true,
                        ));
                    }
                } else if favorites_visible {
                    let favorites_row = if show_language { row8_top } else { row7_top };
                    crate::log_if_err!(MoveWindow(
                        label_favorites,
                        label_x,
                        favorites_row,
                        VOICE_PANEL_LABEL_WIDTH,
                        VOICE_PANEL_ROW_HEIGHT,
                        true,
                    ));
                    crate::log_if_err!(MoveWindow(
                        combo_favorites,
                        combo_x,
                        favorites_row - 2,
                        combo_width,
                        VOICE_PANEL_COMBO_HEIGHT,
                        true,
                    ));
                }
            } else if favorites_visible {
                crate::log_if_err!(MoveWindow(
                    label_favorites,
                    label_x,
                    row1_top,
                    VOICE_PANEL_LABEL_WIDTH,
                    VOICE_PANEL_ROW_HEIGHT,
                    true,
                ));
                crate::log_if_err!(MoveWindow(
                    combo_favorites,
                    combo_x,
                    row1_top - 2,
                    combo_width,
                    VOICE_PANEL_COMBO_HEIGHT,
                    true,
                ));
            }
        }

        let panel_offset = if panel_height > 0 {
            panel_height + 6
        } else {
            0
        };
        for hwnd_edit in edit_handles {
            if hwnd_edit.0 != 0 {
                crate::log_if_err!(MoveWindow(
                    hwnd_edit,
                    tab_rc.left,
                    tab_rc.top + panel_offset,
                    tab_rc.right - tab_rc.left,
                    tab_rc.bottom - tab_rc.top - panel_offset,
                    true,
                ));
            }
        }
    }
}

pub fn create_edit(
    parent: HWND,
    hfont: HFONT,
    word_wrap: bool,
    text_color: u32,
    text_size: i32,
) -> HWND {
    unsafe {
        let mut style = WS_CHILD
            | WS_CLIPCHILDREN
            | WS_VSCROLL
            | WS_GROUP
            | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(ES_MULTILINE as u32)
            | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(ES_AUTOVSCROLL as u32)
            | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(ES_WANTRETURN as u32);
        if !word_wrap {
            style |= WS_HSCROLL
                | windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(ES_AUTOHSCROLL as u32);
        }

        let hwnd_edit = windows::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            WS_EX_CLIENTEDGE,
            MSFTEDIT_CLASS,
            PCWSTR::null(),
            style,
            0,
            0,
            0,
            0,
            parent,
            HMENU(0),
            HINSTANCE(0),
            None,
        );

        if hwnd_edit.0 != 0 {
            if hfont.0 != 0 {
                SendMessageW(hwnd_edit, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
            }
            // Allow large pastes (default edit limit is ~32K).
            apply_text_limit(hwnd_edit);
            apply_text_appearance(hwnd_edit, text_color, text_size);
            if let Some(settings) = with_state(parent, |state| state.settings.clone()) {
                apply_indent_settings_to_edit(hwnd_edit, &settings);
                SendMessageW(
                    hwnd_edit,
                    EM_SETREADONLY,
                    WPARAM(if settings.editor_read_only { 1 } else { 0 }),
                    LPARAM(0),
                );
            }
            SendMessageW(hwnd_edit, EM_SETMODIFY, WPARAM(0), LPARAM(0));
            // Fresh editor instance starts with empty undo stack.
            SendMessageW(hwnd_edit, EM_EMPTYUNDOBUFFER, WPARAM(0), LPARAM(0));
            SendMessageW(
                hwnd_edit,
                EM_SETEVENTMASK,
                WPARAM(0),
                LPARAM((ENM_CHANGE | ENM_SELCHANGE) as isize),
            );
            // Install subclass for smart quotes
            let proc_ptr = edit_subclass_proc as *const () as usize;
            let prev = SetWindowLongPtrW(hwnd_edit, GWLP_WNDPROC, proc_ptr as isize);
            SetWindowLongPtrW(hwnd_edit, GWLP_USERDATA, prev);
            crate::theme::apply_to_window(hwnd_edit);
        }
        hwnd_edit
    }
}

fn should_export_epub_copy(source_is_epub: bool, force_dialog: bool, target_is_epub: bool) -> bool {
    source_is_epub && force_dialog && !target_is_epub
}

fn requires_save_as_for_lossy_source(format: FileFormat, pdf_is_imported_source: bool) -> bool {
    matches!(
        format,
        FileFormat::Docx
            | FileFormat::Odt
            | FileFormat::Doc
            | FileFormat::Spreadsheet
            | FileFormat::Html
            | FileFormat::Ppt
            | FileFormat::Pptx
            | FileFormat::Odp
            | FileFormat::KindleEbook
            | FileFormat::Daisy
    ) || matches!(format, FileFormat::Pdf) && pdf_is_imported_source
}

pub fn save_current_document(hwnd: HWND) -> bool {
    save_document_at(hwnd, get_current_index(hwnd), false)
}

pub fn save_current_document_as(hwnd: HWND) -> bool {
    save_document_at(hwnd, get_current_index(hwnd), true)
}

pub fn save_all_documents(hwnd: HWND) -> bool {
    let doc_count = { with_state(hwnd, |state| state.docs.len()) }.unwrap_or(0);
    let mut dirty_indices = Vec::new();
    for index in 0..doc_count {
        if sync_dirty_from_edit(hwnd, index) {
            dirty_indices.push(index);
        }
    }
    for index in dirty_indices {
        if !save_document_at(hwnd, index, false) {
            return false;
        }
    }
    true
}

pub fn save_document_at(hwnd: HWND, index: usize, force_dialog: bool) -> bool {
    unsafe {
        let result = with_state(hwnd, |state| {
            if state.docs.is_empty() || index >= state.docs.len() {
                return None;
            }
            // Prevent saving audio/video files which would corrupt them
            if matches!(state.docs[index].format, FileFormat::Audiobook) {
                return None;
            }
            if let Some(path) = state.docs[index].path.as_ref()
                && crate::file_handler::is_audio_path(path)
            {
                return None;
            }
            let language = state.settings.language;
            let text = get_edit_text(state.docs[index].hwnd_edit);
            let original_path = state.docs[index].path.clone();
            let epub_original_text = state.docs[index].epub_original_text.clone();
            let source_is_epub = matches!(state.docs[index].format, FileFormat::Epub);
            let allow_epub_save = source_is_epub
                && original_path
                    .as_deref()
                    .is_some_and(crate::file_handler::is_epub_path)
                && epub_original_text.is_some();
            let prefer_title_for_save_suggestion =
                state.docs[index].prefer_title_for_save_suggestion;
            let is_lossy_doc = requires_save_as_for_lossy_source(
                state.docs[index].format,
                state.docs[index].pdf_is_imported_source,
            );
            let mut suggested_name = state.docs[index]
                .path
                .as_ref()
                .and_then(|path| {
                    if source_is_epub {
                        path.file_stem()
                            .and_then(|name| name.to_str())
                            .map(ToString::to_string)
                    } else if path.exists() {
                        path.file_stem()
                            .and_then(|name| name.to_str())
                            .map(ToString::to_string)
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    if prefer_title_for_save_suggestion {
                        None
                    } else {
                        crate::suggested_filename_from_text(&text).filter(|name| !name.is_empty())
                    }
                })
                .unwrap_or_else(|| state.docs[index].title.clone());
            if is_lossy_doc {
                let mut base_name = Path::new(&suggested_name)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or("document")
                    .to_string();
                if !base_name.to_ascii_lowercase().ends_with(".txt") {
                    base_name.push_str(".txt");
                }
                suggested_name = base_name;
            }

            let path_info = if !force_dialog && !is_lossy_doc {
                state.docs[index].path.clone().map(|p| (p, None))
            } else {
                None
            };

            let (path, user_selected_encoding) = match path_info {
                Some((path, enc)) => (path, enc),
                None => {
                    let initial_encoding = state.docs[index]
                        .current_save_text_encoding
                        .or(state.docs[index].opened_text_encoding)
                        .unwrap_or_default();
                    let initial_directory = original_path
                        .as_deref()
                        .and_then(Path::parent)
                        .filter(|directory| directory.is_dir());
                    match crate::save_file_dialog_with_encoding(
                        hwnd,
                        Some(&suggested_name),
                        initial_directory,
                        initial_encoding,
                        allow_epub_save,
                        allow_epub_save.then_some("epub"),
                    ) {
                        Some((path, enc)) => (path, Some(enc)),
                        None => return None,
                    }
                }
            };
            if crate::file_handler::is_audio_path(&path) {
                return None;
            }

            let is_epub = crate::file_handler::is_epub_path(&path);
            let exporting_epub_copy =
                should_export_epub_copy(source_is_epub, force_dialog, is_epub);
            let is_pdf = crate::file_handler::is_pdf_path(&path);
            let is_docx = crate::file_handler::is_docx_path(&path);
            let is_doc = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("doc"))
                .unwrap_or(false);
            let is_rtf = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("rtf"))
                .unwrap_or(false);

            if is_epub {
                let Some(source_path) = original_path.as_deref() else {
                    crate::show_error(
                        hwnd,
                        language,
                        &crate::settings::error_save_file_message(
                            language,
                            "An EPUB can be saved only when the document was opened from an EPUB file.",
                        ),
                    );
                    return None;
                };
                let Some(original_epub_text) = epub_original_text.as_deref() else {
                    crate::show_error(
                        hwnd,
                        language,
                        &crate::settings::error_save_file_message(
                            language,
                            "The original EPUB text map is not available. Reopen the book before saving it.",
                        ),
                    );
                    return None;
                };
                if let Err(message) = crate::epub_editor::write_epub_preserving_structure(
                    source_path,
                    &path,
                    original_epub_text,
                    &text,
                    language,
                ) {
                    crate::show_error(hwnd, language, &message);
                    return None;
                }
                let saved_document = match crate::file_handler::read_epub_document(&path, language)
                {
                    Ok(document) => document,
                    Err(message) => {
                        crate::show_error(hwnd, language, &message);
                        return None;
                    }
                };
                let saved_text = saved_document.text;
                let saved_index = saved_document.index;
                if saved_text != text {
                    let hwnd_edit = state.docs[index].hwnd_edit;
                    let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
                    SendMessageW(
                        hwnd_edit,
                        EM_EXGETSEL,
                        WPARAM(0),
                        LPARAM(&mut selection as *mut _ as isize),
                    );
                    set_edit_text(hwnd_edit, &saved_text);
                    let maximum_position = saved_text.encode_utf16().count() as i32;
                    selection.cpMin = selection.cpMin.clamp(0, maximum_position);
                    selection.cpMax = selection.cpMax.clamp(0, maximum_position);
                    SendMessageW(
                        hwnd_edit,
                        EM_EXSETSEL,
                        WPARAM(0),
                        LPARAM(&mut selection as *mut _ as isize),
                    );
                    SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
                }
                state.docs[index].format = FileFormat::Epub;
                state.docs[index].opened_text_encoding = None;
                state.docs[index].current_save_text_encoding = None;
                state.docs[index].epub_original_text = Some(saved_text);
                state.docs[index].epub_index = saved_index;
            } else if is_pdf {
                let pdf_title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Sonarpad Document");
                if crate::file_handler::text_has_pdf_form_values(&text) {
                    let Some(source_path) = original_path.as_deref() else {
                        crate::show_error(
                            hwnd,
                            language,
                            &crate::settings::error_save_file_message(
                                language,
                                "Original PDF form not found",
                            ),
                        );
                        return None;
                    };
                    match crate::file_handler::write_pdf_form_values_from_text(
                        source_path,
                        &path,
                        &text,
                        language,
                    ) {
                        Ok(true) => {}
                        Ok(false) => {
                            crate::show_error(
                                hwnd,
                                language,
                                &crate::settings::error_save_file_message(
                                    language,
                                    "No matching PDF form fields were found",
                                ),
                            );
                            return None;
                        }
                        Err(message) => {
                            crate::show_error(hwnd, language, &message);
                            return None;
                        }
                    }
                } else if let Err(message) =
                    crate::file_handler::write_pdf_text(&path, pdf_title, &text, language)
                {
                    crate::show_error(hwnd, language, &message);
                    return None;
                }
                if !exporting_epub_copy {
                    state.docs[index].format = FileFormat::Pdf;
                }
            } else if is_docx || is_doc {
                if let Err(message) = crate::file_handler::write_docx_text(&path, &text, language) {
                    crate::show_error(hwnd, language, &message);
                    return None;
                }
                if !exporting_epub_copy {
                    state.docs[index].format = if is_docx {
                        FileFormat::Docx
                    } else {
                        FileFormat::Doc
                    };
                }
            } else if is_rtf {
                if let Err(message) =
                    crate::audio_utils::write_rtf_text(&path, state.docs[index].hwnd_edit)
                {
                    crate::show_error(hwnd, language, &message);
                    return None;
                }
                if !exporting_epub_copy {
                    state.docs[index].format = FileFormat::Doc; // Doc handles RTF as well
                }
            } else {
                let encoding = if let Some(enc) = user_selected_encoding {
                    if !exporting_epub_copy {
                        state.docs[index].current_save_text_encoding = Some(enc);
                    }
                    enc
                } else {
                    state.docs[index]
                        .current_save_text_encoding
                        .or(state.docs[index].opened_text_encoding)
                        .unwrap_or_default()
                };
                let bytes = encode_text(&text, encoding);
                if let Err(err) = std::fs::write(&path, bytes) {
                    crate::show_error(
                        hwnd,
                        language,
                        &crate::settings::error_save_file_message(language, err),
                    );
                    return None;
                }
                if !exporting_epub_copy {
                    state.docs[index].format = FileFormat::Text(encoding);
                }
            }

            if !exporting_epub_copy {
                state.docs[index].pdf_is_imported_source = false;
            }

            if exporting_epub_copy {
                crate::log_debug(&format!(
                    "EPUB export: wrote '{}' without changing the source document association",
                    path.display()
                ));
                return Some(path);
            }

            if !is_epub {
                state.docs[index].epub_original_text = None;
                state.docs[index].epub_index.clear();
            }

            let hwnd_edit = state.docs[index].hwnd_edit;
            let (old_bookmark_key, _) =
                crate::bookmark_storage_key(state.docs[index].path.as_deref(), hwnd_edit);
            let (new_bookmark_key, new_bookmark_persist) =
                crate::bookmark_storage_key(Some(path.as_path()), hwnd_edit);
            if old_bookmark_key != new_bookmark_key
                && let Some(mut moved) = state.bookmarks.files.remove(&old_bookmark_key)
            {
                state
                    .bookmarks
                    .files
                    .entry(new_bookmark_key.clone())
                    .or_default()
                    .append(&mut moved);
                if let Some(list) = state.bookmarks.files.get_mut(&new_bookmark_key) {
                    crate::bookmarks::sort_bookmarks(list);
                }
                if new_bookmark_persist {
                    crate::bookmarks::save_bookmarks(&state.bookmarks);
                }
            }
            state.docs[index].path = Some(path.clone());
            state.docs[index].dirty = false;
            if force_dialog {
                state.docs[index].is_temporary = false;
                state.docs[index].from_rss = false;
                state.docs[index].from_wikipedia = false;
                state.docs[index].from_treccani = false;
            }
            SendMessageW(hwnd_edit, EM_SETMODIFY, WPARAM(0), LPARAM(0));
            let title = path.file_name().and_then(|s| s.to_str()).unwrap_or("File");
            state.docs[index].title = title.to_string();
            update_tab_title(state.hwnd_tab, index, &state.docs[index].title, false);
            if index == state.current {
                update_window_title(hwnd);
            }
            Some(path)
        });
        if result.is_none() {
            crate::log_debug("Failed to access editor state");
        }

        if let Some(Some(path)) = result {
            crate::push_recent_file(hwnd, &path);
            true
        } else {
            false
        }
    }
}

pub fn refresh_current_editor_visual(hwnd: HWND) {
    unsafe {
        let hwnd_edit = with_state(hwnd, |state| {
            state.docs.get(state.current).map(|doc| doc.hwnd_edit)
        })
        .flatten();
        if let Some(hwnd_edit) = hwnd_edit
            && !InvalidateRect(hwnd_edit, None, BOOL(1)).as_bool()
        {
            crate::log_debug("InvalidateRect failed after save");
        }
    }
}

pub fn close_current_document(hwnd: HWND) {
    let index = match with_state(hwnd, |state| state.current) {
        Some(i) => i,
        None => return,
    };
    if !close_document_at(hwnd, index) {
        crate::log_debug("Failed to close document");
    }
}

pub fn close_other_documents(hwnd: HWND) -> bool {
    loop {
        let (current, total) = match with_state(hwnd, |state| (state.current, state.docs.len())) {
            Some(values) => values,
            None => return true,
        };
        if total <= 1 {
            return true;
        }
        let idx = if current == 0 { 1 } else { 0 };
        if !close_document_at(hwnd, idx) {
            return false;
        }
    }
}

pub fn close_all_documents(hwnd: HWND) -> bool {
    let initial_total = { with_state(hwnd, |state| state.docs.len()) }.unwrap_or(0);
    for _ in 0..initial_total {
        if !close_document_at(hwnd, 0) {
            return false;
        }
    }
    true
}

pub fn close_document_at(hwnd: HWND, index: usize) -> bool {
    unsafe {
        let result = with_state(hwnd, |state| {
            if index >= state.docs.len() {
                return None;
            }
            Some((
                state.current,
                state.hwnd_tab,
                state.docs.len(),
                state.docs[index].title.clone(),
            ))
        });
        if result.is_none() {
            crate::log_debug("Failed to access editor state");
        }

        let (_current, hwnd_tab, _count, title) = match result {
            Some(Some(values)) => values,
            _ => return true,
        };

        if !confirm_save_if_dirty_entry(hwnd, index, &title) {
            return false;
        }
        crate::save_automatic_bookmark_for_document(hwnd, index);

        let mut closing_hwnd_edit = HWND(0);
        let mut new_hwnd_edit = None;
        let mut was_current = false;
        let mut was_empty = false;
        let mut update_title = false;
        let mut was_audiobook = false;
        let mut should_stop_tts = false;
        let mut should_clear_tts_automatic_bookmark = false;
        let mut removed_path: Option<PathBuf> = None;
        let mut removed_from_rss = false;

        if with_state(hwnd, |state| {
            was_current = state.current == index;
            let doc = state.docs.remove(index);
            removed_path = doc.path.clone();
            removed_from_rss = doc.from_rss;
            closing_hwnd_edit = doc.hwnd_edit;
            state.large_text_editors.remove(&closing_hwnd_edit.0);
            was_audiobook = matches!(doc.format, FileFormat::Audiobook);
            should_stop_tts = !was_audiobook
                && was_current
                && (state.tts_session.is_some() || state.tts_pending_start_pos.is_some());
            should_clear_tts_automatic_bookmark = state
                .tts_automatic_bookmark_position
                .as_ref()
                .is_some_and(|(bookmark_hwnd, _, _)| *bookmark_hwnd == closing_hwnd_edit);
            SendMessageW(
                hwnd_tab,
                windows::Win32::UI::Controls::TCM_DELETEITEM,
                WPARAM(index),
                LPARAM(0),
            );

            if state.docs.is_empty() {
                state.untitled_count = 0;
                state.current = 0;
                was_empty = true;
            } else if was_current {
                let idx = if index >= state.docs.len() {
                    state.docs.len() - 1
                } else {
                    index
                };
                state.current = idx;
                SendMessageW(hwnd_tab, TCM_SETCURSEL, WPARAM(idx), LPARAM(0));
                new_hwnd_edit = state.docs.get(idx).map(|doc| doc.hwnd_edit);
                update_title = true;
            } else if index < state.current {
                state.current -= 1;
                SendMessageW(hwnd_tab, TCM_SETCURSEL, WPARAM(state.current), LPARAM(0));
            }
        })
        .is_none()
        {
            crate::log_debug("Failed to access editor state");
        }

        if closing_hwnd_edit.0 != 0 {
            crate::log_if_err!(crate::destroy_window_safe(closing_hwnd_edit));
        }
        if should_stop_tts {
            crate::tts_engine::stop_tts_playback(hwnd);
        } else if should_clear_tts_automatic_bookmark {
            with_state(hwnd, |state| {
                state.tts_automatic_bookmark_position = None;
            });
        }
        if was_audiobook {
            crate::audio_player::stop_audiobook_playback(hwnd);
            crate::clear_active_podcast_chapters(hwnd);
            if removed_from_rss {
                let removed_path_for_cleanup = removed_path.clone();
                if with_state(hwnd, |state| {
                    if let Some(path) = removed_path_for_cleanup.as_ref() {
                        if let Some(pos) = state.audio_playlist.iter().position(|p| p == path) {
                            state.audio_playlist.remove(pos);
                            state.audio_playlist_index = match state.audio_playlist_index {
                                Some(current) if current == pos => None,
                                Some(current) if current > pos => Some(current - 1),
                                other => other,
                            };
                        }
                        if state
                            .audio_ffmpeg_retry_for
                            .as_ref()
                            .is_some_and(|retry| retry == path)
                        {
                            state.audio_ffmpeg_retry_for = None;
                        }
                        if state
                            .audio_unexpected_stop_retry_for
                            .as_ref()
                            .is_some_and(|retry| retry == path)
                        {
                            state.audio_unexpected_stop_retry_for = None;
                        }
                        if state
                            .last_stopped_audiobook
                            .as_ref()
                            .is_some_and(|stopped| stopped == path)
                        {
                            state.last_stopped_audiobook = None;
                        }
                    }
                    state.selected_audio_track = None;
                    state.pending_podcast_chapters_key = None;
                })
                .is_none()
                {
                    crate::log_debug(
                        "Failed to clear streaming playback state after closing document",
                    );
                }
            }
        }

        if was_empty {
            new_document(hwnd);
        } else {
            if let Some(hwnd_edit) = new_hwnd_edit {
                let is_audiobook = with_state(hwnd, |state| {
                    state
                        .docs
                        .get(state.current)
                        .map(|d| matches!(d.format, FileFormat::Audiobook))
                        .unwrap_or(false)
                })
                .unwrap_or(false);
                if is_audiobook {
                    let hwnd_tab = with_state(hwnd, |state| state.hwnd_tab).unwrap_or(HWND(0));
                    if hwnd_tab.0 != 0 {
                        SetFocus(hwnd_tab);
                    }
                } else {
                    ShowWindow(hwnd_edit, SW_SHOW);
                    SetFocus(hwnd_edit);
                }
            }
            if update_title {
                update_window_title(hwnd);
            }
        }
        layout_children(hwnd);
        let is_audiobook = with_state(hwnd, |state| {
            state
                .docs
                .get(state.current)
                .map(|d| matches!(d.format, FileFormat::Audiobook))
                .unwrap_or(false)
        })
        .unwrap_or(false);
        crate::menu::update_playback_menu(hwnd, is_audiobook);
        crate::update_main_status_bar(hwnd);
        crate::restore_transcription_progress_focus_for_current_document(hwnd);
        true
    }
}

pub fn try_close_app(hwnd: HWND) -> bool {
    crate::log_debug(&format!(
        "Application shutdown: try_close_app start hwnd={:?}",
        hwnd
    ));
    let result = with_state(hwnd, |state| {
        state
            .docs
            .iter()
            .enumerate()
            .map(|(i, d)| (i, d.title.clone()))
            .collect::<Vec<_>>()
    });
    if result.is_none() {
        crate::log_debug("Failed to access editor state");
    }

    if let Some(entries) = result {
        for (index, title) in &entries {
            if !confirm_save_if_dirty_entry(hwnd, *index, title) {
                return false;
            }
        }
        for (index, _) in entries {
            crate::save_automatic_bookmark_for_document(hwnd, index);
        }
    }
    let has_active_audiobook = with_state(hwnd, |state| {
        state.active_audiobook.is_some() || state.active_mpv_session.is_some()
    })
    .unwrap_or(false);
    if has_active_audiobook {
        crate::audio_player::stop_audiobook_playback(hwnd);
    }
    crate::clear_active_podcast_chapters(hwnd);
    crate::log_debug("Application shutdown: cleaning temporary TTS artifacts");
    if let Err(e) = crate::ffmpeg_export::cleanup_tts_artifacts() {
        crate::log_debug(&e);
    }
    crate::log_debug("Application shutdown: stopping shared dictation worker");
    crate::tools::faster_whisper_bridge::shutdown_shared_worker();
    crate::log_debug("Application shutdown: destroying main window");
    crate::log_if_err!(crate::destroy_window_safe(hwnd));
    crate::log_debug("Application shutdown: main-window destroy returned");
    true
}

pub fn sync_dirty_from_edit(hwnd: HWND, index: usize) -> bool {
    unsafe {
        let mut hwnd_edit = HWND(0);
        let mut is_dirty = false;
        let mut is_current = false;
        if with_state(hwnd, |state| {
            if let Some(doc) = state.docs.get(index) {
                hwnd_edit = doc.hwnd_edit;
                is_dirty = doc.dirty;
                is_current = state.current == index;
            }
        })
        .is_none()
        {
            crate::log_debug("Failed to access editor state");
        }

        if hwnd_edit.0 == 0 {
            return is_dirty;
        }

        let modified = SendMessageW(hwnd_edit, EM_GETMODIFY, WPARAM(0), LPARAM(0)).0 != 0;
        if modified && !is_dirty {
            if with_state(hwnd, |state| {
                if let Some(doc) = state.docs.get_mut(index) {
                    doc.dirty = true;
                    update_tab_title(state.hwnd_tab, index, &doc.title, true);
                    if is_current {
                        update_window_title(hwnd);
                    }
                }
            })
            .is_none()
            {
                crate::log_debug("Failed to access editor state");
            }
            return true;
        }
        is_dirty
    }
}

pub fn confirm_save_if_dirty_entry(hwnd: HWND, index: usize, title: &str) -> bool {
    let is_dirty = sync_dirty_from_edit(hwnd, index);
    let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();

    if is_dirty {
        let msg = confirm_save_message(language, title);
        let title_w = confirm_title(language);

        let result = crate::show_blocking_modal_message_box(
            hwnd,
            crate::BlockingModalKind::DocumentSaveConfirm,
            PCWSTR(to_wide(&msg).as_ptr()),
            PCWSTR(to_wide(&title_w).as_ptr()),
            MB_YESNOCANCEL | MB_ICONWARNING,
        );

        return match result {
            IDYES => save_document_at(hwnd, index, false),
            IDNO => true,
            _ => false,
        };
    }

    let confirm_close_when_clean = with_state(hwnd, |state| {
        state
            .docs
            .get(index)
            .map(|doc| doc.confirm_close_when_clean)
            .unwrap_or(false)
    })
    .unwrap_or(false);
    if !confirm_close_when_clean {
        return true;
    }

    let msg = crate::i18n::tr(language, "whisper.close_confirm");
    let title_w = confirm_title(language);
    crate::show_blocking_modal_message_box(
        hwnd,
        crate::BlockingModalKind::DocumentSaveConfirm,
        PCWSTR(to_wide(&msg).as_ptr()),
        PCWSTR(to_wide(&title_w).as_ptr()),
        windows::Win32::UI::WindowsAndMessaging::MB_YESNO | MB_ICONWARNING,
    ) == IDYES
}

pub fn get_current_index(hwnd: HWND) -> usize {
    { with_state(hwnd, |state| state.current) }.unwrap_or(0)
}

pub fn get_tab(hwnd: HWND) -> HWND {
    { with_state(hwnd, |state| state.hwnd_tab) }.unwrap_or(HWND(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epub_cross_format_save_as_is_an_export_copy() {
        assert!(should_export_epub_copy(true, true, false));
        assert!(!should_export_epub_copy(true, false, false));
        assert!(!should_export_epub_copy(true, true, true));
        assert!(!should_export_epub_copy(false, true, false));
    }

    #[test]
    fn imported_pdf_requires_save_as_but_generated_pdf_does_not() {
        assert!(requires_save_as_for_lossy_source(FileFormat::Pdf, true));
        assert!(!requires_save_as_for_lossy_source(FileFormat::Pdf, false));
        assert!(requires_save_as_for_lossy_source(FileFormat::Docx, false));
        assert!(!requires_save_as_for_lossy_source(
            FileFormat::Text(TextEncoding::Utf8),
            false
        ));
    }

    #[test]
    fn join_wrapped_lines_reconstructs_paragraphs_across_sentence_boundaries() {
        let input = "Prima parte della frase,\nche continua qui.\nNuova frase.\nAltra riga senza punto\nche continua.";
        let expected = "Prima parte della frase, che continua qui. Nuova frase. Altra riga senza punto che continua.";
        assert_eq!(join_wrapped_lines_block(input, "\n", false), expected);
    }

    #[test]
    fn join_wrapped_lines_preserves_paragraphs_and_lists() {
        let input = "Frase spezzata\nin due righe.\n\n1. Primo elemento\n2. Secondo elemento\n\nTesto finale.";
        let expected = "Frase spezzata in due righe.\n\n1. Primo elemento\n2. Secondo elemento\n\nTesto finale.";
        assert_eq!(join_wrapped_lines_block(input, "\n", false), expected);
    }

    #[test]
    fn join_wrapped_lines_handles_hyphen_apostrophe_and_windows_eol() {
        let input = "cognitivo-\ncomportamentale e l'\nelaborazione.\r\nFrase nuova.";
        let expected = "cognitivo-comportamentale e l'elaborazione. Frase nuova.";
        assert_eq!(join_wrapped_lines_block(input, "\r\n", false), expected);
    }

    #[test]
    fn join_wrapped_lines_keeps_list_items_separate_and_joins_their_continuations() {
        let input = "1. Primo elemento\nche continua su una seconda riga.\n2. Secondo elemento\ncon altra continuazione.";
        let expected = "1. Primo elemento che continua su una seconda riga.\n2. Secondo elemento con altra continuazione.";
        assert_eq!(join_wrapped_lines_block(input, "\n", false), expected);
    }

    #[test]
    fn join_wrapped_lines_keeps_trailing_newline() {
        assert_eq!(
            join_wrapped_lines_block("riga spezzata\nqui\n", "\n", true),
            "riga spezzata qui\n"
        );
    }

    #[test]
    fn test_clean_end_of_line_hyphens() {
        // Simple join
        assert_eq!(
            clean_end_of_line_hyphens_block("inter-\nnational", "\n", false),
            "international"
        );
        // Windows EOL
        assert_eq!(
            clean_end_of_line_hyphens_block("inter-\r\nnational", "\r\n", false),
            "international"
        );
        // Page gap (2 line breaks)
        assert_eq!(
            clean_end_of_line_hyphens_block("inter-\n\nnational", "\n", false),
            "international"
        );
        // 3 line breaks
        assert_eq!(
            clean_end_of_line_hyphens_block("inter-\n\n\nnational", "\n", false),
            "international"
        );
        // 4 line breaks (too many, shouldn't join)
        assert_eq!(
            clean_end_of_line_hyphens_block("inter-\n\n\n\nnational", "\n", false),
            "inter-\n\n\n\nnational"
        );

        // Non-join for dashes (whitespace before hyphen)
        assert_eq!(
            clean_end_of_line_hyphens_block("word -\nnext", "\n", false),
            "word -\nnext"
        );

        // Non-join for hyphenated compounds (next char not alphabetic)
        assert_eq!(
            clean_end_of_line_hyphens_block("state-\n123", "\n", false),
            "state-\n123"
        );
        assert_eq!(
            clean_end_of_line_hyphens_block("state-\n. Next", "\n", false),
            "state-\n. Next"
        );

        // Digit before hyphen (should join)
        assert_eq!(
            clean_end_of_line_hyphens_block("Section3-\npart", "\n", false),
            "Section3part"
        );

        // Preserve paragraph structure
        let input = "This is a test-\ncase.\n\nNew paragraph.";
        let expected = "This is a testcase.\n\nNew paragraph.";
        assert_eq!(
            clean_end_of_line_hyphens_block(input, "\n", false),
            expected
        );

        // Case sensitivity (lowercase/uppercase after join)
        assert_eq!(
            clean_end_of_line_hyphens_block("Inter-\nNational", "\n", false),
            "InterNational"
        );
    }

    #[test]
    fn quote_lines_block_handles_cr_only_newlines() {
        let input = "linea1\rlinea2\rlinea3";
        let out = quote_lines_block(input, "\r\n", false, "> ");
        assert_eq!(out, "> linea1\r\n> linea2\r\n> linea3");
    }

    #[test]
    fn unquote_lines_block_handles_cr_only_newlines() {
        let input = "> linea1\r> linea2\r> linea3";
        let out = unquote_lines_block(input, "\r\n", false, "> ");
        assert_eq!(out, "linea1\r\nlinea2\r\nlinea3");
    }
}
