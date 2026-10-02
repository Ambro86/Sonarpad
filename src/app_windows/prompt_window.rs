use crate::accessibility::{EM_GETSEL, EM_REPLACESEL, EM_SCROLLCARET, to_wide};
use crate::conpty::{ConPtySession, ConPtySpawn};
use crate::settings::{Language, confirm_title, save_settings};
use crate::{i18n, log_debug, show_error, with_state};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetDC, GetTextMetricsW, HFONT, ReleaseDC, TEXTMETRICW};
use windows::Win32::Storage::FileSystem::ReadFile;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::UI::Controls::{BST_CHECKED, WC_BUTTON, WC_COMBOBOXW, WC_EDIT, WC_STATIC};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, SetFocus, VK_CONTROL, VK_ESCAPE, VK_RETURN, VK_SHIFT,
    VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL,
    CB_SETCURSEL, CBS_DROPDOWNLIST, CreateWindowExW, DefWindowProcW, DispatchMessageW,
    ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_PASSWORD, ES_READONLY, GWLP_USERDATA,
    GetClientRect, GetMessageW, GetParent, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW,
    HMENU, IDC_ARROW, IsDialogMessageW, IsWindow, KillTimer, LoadCursorW, MB_ICONQUESTION,
    MB_OKCANCEL, MESSAGEBOX_STYLE, MSG, MessageBoxW, PostMessageW, RegisterClassW, SW_HIDE,
    SW_SHOW, SendMessageW, SetForegroundWindow, SetTimer, SetWindowLongPtrW, ShowWindow,
    TranslateMessage, WINDOW_STYLE, WM_ACTIVATE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CREATE,
    WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_SETFOCUS, WM_SETFONT, WM_SIZE, WM_SYSKEYDOWN,
    WM_TIMER, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT,
    WS_EX_DLGMODALFRAME, WS_SIZEBOX, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::PCWSTR;

const PROMPT_CLASS_NAME: &str = "SonarpadPrompt";

const PROMPT_ID_INPUT: usize = 9301;
const PROMPT_ID_OUTPUT: usize = 9302;
const PROMPT_ID_AUTOSCROLL: usize = 9303;
const PROMPT_ID_STRIP_ANSI: usize = 9304;
const PROMPT_ID_ANNOUNCE_LINES: usize = 9305;
const PROMPT_ID_BEEP_ON_IDLE: usize = 9306;
const PROMPT_ID_PREVENT_SLEEP: usize = 9307;

const WM_PROMPT_OUTPUT: u32 = WM_APP + 60;
const WM_CREDENTIALS_PROMPT_REFOCUS: u32 = WM_APP + 61;
const EM_SETSEL: u32 = 0x00B1;
const EM_LIMITTEXT: u32 = 0x00C5;
const EM_SETREADONLY: u32 = 0x00CF;
const PROMPT_OUTPUT_LIMIT: usize = 40_000;
const PROMPT_OUTPUT_KEEP: usize = 10_000;
const PROMPT_OUTPUT_TIMER_ID: usize = 3;
const PROMPT_OUTPUT_FLUSH_CHARS: usize = 2048;

fn read_file_ok(handle: HANDLE, buffer: &mut [u8], read: &mut u32) -> bool {
    unsafe { ReadFile(handle, Some(buffer), Some(read), None).is_ok() }
}

struct PromptWindowInit {
    parent: HWND,
    initial_command: Option<String>,
    working_dir: Option<PathBuf>,
}

struct PromptLabels {
    title: String,
    input: String,
    output: String,
    autoscroll: String,
    strip_ansi: String,
    announce_lines: String,
    beep_on_idle: String,
    prevent_sleep: String,
    clear_confirm: String,
}

#[derive(Clone, Copy, Default)]
pub struct PromptUserOptions {
    pub masked: bool,
}

pub struct PromptCredentialsResult {
    pub username: String,
    pub password: String,
    pub save_credentials: bool,
}

pub struct PromptDirectoryResult {
    pub selected_index: usize,
    pub secondary_selected_index: usize,
    pub tertiary_selected_index: usize,
    pub quaternary_selected_index: usize,
    pub primary_value: String,
    pub secondary_value: String,
    pub tertiary_value: String,
    pub checkbox_checked: bool,
}

pub struct PromptDirectoryOptions {
    pub title: String,
    pub type_label: String,
    pub options: Vec<String>,
    pub default_selection: usize,
    pub secondary_type_label: String,
    pub secondary_options: Vec<String>,
    pub secondary_default_selection: usize,
    pub tertiary_type_label: String,
    pub tertiary_options: Vec<String>,
    pub tertiary_default_selection: usize,
    pub tertiary_options_primary_index_only: Option<usize>,
    pub quaternary_type_label: String,
    pub quaternary_options: Vec<String>,
    pub quaternary_default_selection: usize,
    pub focus_primary_field: bool,
    pub primary_label: String,
    pub primary_labels: Vec<String>,
    pub primary_default: String,
    pub secondary_label: String,
    pub secondary_default: String,
    pub tertiary_label: String,
    pub tertiary_default: String,
    pub checkbox_label: String,
    pub checkbox_default: bool,
}

struct AnsiStripper {
    state: AnsiState,
    chars_consumed: usize,
}

enum AnsiState {
    Normal,
    Esc,
    Csi,
    Osc,
    OscEsc, // Met ESC while in OSC, waiting for backslash
}

impl AnsiStripper {
    fn new() -> Self {
        Self {
            state: AnsiState::Normal,
            chars_consumed: 0,
        }
    }

    fn process(&mut self, input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for ch in input.chars() {
            // Safety valve: prevent getting stuck in a state for too long
            if !matches!(self.state, AnsiState::Normal) {
                self.chars_consumed += 1;
                if self.chars_consumed > 1000 {
                    self.state = AnsiState::Normal;
                    self.chars_consumed = 0;
                }
            }

            match self.state {
                AnsiState::Normal => {
                    if ch == '\x1B' {
                        self.state = AnsiState::Esc;
                        self.chars_consumed = 0;
                    } else {
                        out.push(ch);
                    }
                }
                AnsiState::Esc => {
                    match ch {
                        '[' => self.state = AnsiState::Csi,
                        ']' => self.state = AnsiState::Osc,
                        '(' | ')' | '>' | '=' | '7' | '8' | 'M' | 'E' | 'D' | 'H' | 'Z' => {
                            // Simple sequences, just consume and return to normal
                            self.state = AnsiState::Normal;
                        }
                        _ => {
                            // Unknown sequence or simple ESC
                            self.state = AnsiState::Normal;
                            out.push(ch);
                        }
                    }
                }
                AnsiState::Csi => {
                    // CSI sequences MUST be ASCII.
                    // Parameter bytes: 0x30–0x3F (0-9:;<=>?)
                    // Intermediate bytes: 0x20–0x2F (space !"#$%&'()*+,-./)
                    // Final bytes: 0x40–0x7E (@A-Z[\]^_`a-z{|}~)
                    if ch.is_ascii() {
                        let b = ch as u8;
                        if (0x40..=0x7E).contains(&b) {
                            // Valid terminator
                            self.state = AnsiState::Normal;
                        } else if (0x20..=0x3F).contains(&b) {
                            // Valid parameter/intermediate, consume
                        } else {
                            // Invalid ASCII char for CSI (e.g. control char < 0x20)
                            // Abort sequence and output character
                            self.state = AnsiState::Normal;
                            out.push(ch);
                        }
                    } else {
                        // Non-ASCII character (e.g. UTF-8 box drawing).
                        // Definitely not part of a standard CSI sequence.
                        // Abort sequence and output character.
                        self.state = AnsiState::Normal;
                        out.push(ch);
                    }
                }
                AnsiState::Osc => {
                    if ch == '\x07' {
                        // BEL terminates OSC
                        self.state = AnsiState::Normal;
                    } else if ch == '\x1B' {
                        // Check for ST (ESC \)
                        self.state = AnsiState::OscEsc;
                    }
                    // Otherwise consume content of OSC
                }
                AnsiState::OscEsc => {
                    if ch == '\\' {
                        self.state = AnsiState::Normal;
                    } else {
                        // Not a backslash, so it wasn't an ST terminator.
                        // We are still in OSC mode.
                        self.state = AnsiState::Osc;
                    }
                }
            }
        }
        out
    }
}

struct PromptState {
    parent: HWND,
    label_input: HWND,
    input: HWND,
    label_output: HWND,
    output: HWND,

    checkbox_autoscroll: HWND,
    checkbox_strip_ansi: HWND,
    checkbox_announce_lines: HWND,
    checkbox_beep_on_idle: HWND,
    checkbox_prevent_sleep: HWND,
    auto_scroll: bool,
    strip_ansi: bool,
    announce_lines: bool,
    beep_on_idle: bool,
    prevent_sleep: bool,
    buffer: String,
    buffer_utf16_len: usize,
    line_start_byte: usize,
    line_start_utf16: usize,
    line_has_content: bool,
    blank_line_streak: u8,
    pending_ws: String,
    program_is_codex: bool,
    last_announced_line: String,
    beep_state: Arc<PromptBeepState>,
    session: Option<ConPtySession>,
    reader_cancel: Arc<AtomicBool>,
    ansi_stripper: AnsiStripper,
    output_queue: VecDeque<String>,
    output_flush_active: bool,
}

fn prompt_labels(language: Language) -> PromptLabels {
    PromptLabels {
        title: i18n::tr(language, "prompt.title"),
        input: i18n::tr(language, "prompt.input"),
        output: i18n::tr(language, "prompt.output"),
        autoscroll: i18n::tr(language, "prompt.autoscroll"),
        strip_ansi: i18n::tr(language, "prompt.strip_ansi"),
        announce_lines: i18n::tr(language, "prompt.announce_lines"),
        beep_on_idle: i18n::tr(language, "prompt.beep_on_idle"),
        prevent_sleep: i18n::tr(language, "prompt.prevent_sleep"),
        clear_confirm: i18n::tr(language, "prompt.clear_confirm"),
    }
}

pub fn prompt_user(
    parent: HWND,
    title: &str,
    body: &str,
    default_val: &str,
    language: Language,
) -> Option<String> {
    prompt_user_with_options(
        parent,
        title,
        body,
        default_val,
        language,
        PromptUserOptions::default(),
    )
}

pub fn prompt_user_with_options(
    parent: HWND,
    title: &str,
    body: &str,
    default_val: &str,
    language: Language,
    options: PromptUserOptions,
) -> Option<String> {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide("SonarpadSimplePrompt");

        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let wc = WNDCLASSW {
                hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                    LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
                ),
                hInstance: hinstance,
                lpszClassName: PCWSTR(class_name.as_ptr()),
                lpfnWndProc: Some(simple_prompt_wndproc),
                ..Default::default()
            };
            RegisterClassW(&wc);
        });

        let mut data = SimplePromptData {
            body: body.to_string(),
            value: default_val.to_string(),
            confirmed: false,
            language,
            masked: options.masked,
        };

        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide(title).as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            200,
            200,
            440,
            205,
            parent,
            HMENU(0),
            hinstance,
            Some(&mut data as *mut _ as *const std::ffi::c_void),
        );

        if hwnd.0 == 0 {
            return None;
        }

        windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(parent, false);

        let mut msg = MSG::default();
        while IsWindow(hwnd).as_bool() && GetMessageW(&mut msg, HWND(0), 0, 0).into() {
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&msg) {
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_TAB.0 as u32 {
                SendMessageW(hwnd, WM_KEYDOWN, msg.wParam, msg.lParam);
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

        windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(parent, true);
        if parent.0 != 0 {
            let mut class_buf = [0u16; 64];
            let class_len = crate::get_class_name_w_safe(parent, &mut class_buf);
            let parent_class = if class_len > 0 {
                String::from_utf16_lossy(&class_buf[..class_len as usize])
            } else {
                String::new()
            };
            if parent_class == "SonarpadWin32" {
                crate::bring_window_to_foreground(parent);
                crate::log_if_err!(crate::post_message_w_safe(
                    parent,
                    crate::WM_FOCUS_EDITOR,
                    WPARAM(0),
                    LPARAM(0)
                ));
            } else {
                crate::set_foreground_window_safe(parent);
                crate::set_focus_safe(parent);
            }
        }

        if data.confirmed {
            Some(data.value)
        } else {
            None
        }
    }
}

pub fn prompt_user_choice(
    parent: HWND,
    title: &str,
    body: &str,
    choices: &[String],
    preferred: &str,
    language: Language,
) -> Option<String> {
    if choices.is_empty() {
        return None;
    }

    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide("SonarpadChoicePrompt");

        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let wc = WNDCLASSW {
                hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                    LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
                ),
                hInstance: hinstance,
                lpszClassName: PCWSTR(class_name.as_ptr()),
                lpfnWndProc: Some(choice_prompt_wndproc),
                ..Default::default()
            };
            RegisterClassW(&wc);
        });

        let selected_index = choices
            .iter()
            .position(|choice| choice.eq_ignore_ascii_case(preferred))
            .unwrap_or(0);
        let mut data = ChoicePromptData {
            body: body.to_string(),
            choices: choices.to_vec(),
            selected_index,
            confirmed: false,
            language,
        };

        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide(title).as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            200,
            200,
            430,
            220,
            parent,
            HMENU(0),
            hinstance,
            Some(&mut data as *mut _ as *const std::ffi::c_void),
        );

        if hwnd.0 == 0 {
            return None;
        }

        EnableWindow(parent, false);

        let mut msg = MSG::default();
        while IsWindow(hwnd).as_bool() && GetMessageW(&mut msg, HWND(0), 0, 0).into() {
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&msg) {
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_TAB.0 as u32 {
                SendMessageW(hwnd, WM_KEYDOWN, msg.wParam, msg.lParam);
                continue;
            }
            if !IsDialogMessageW(hwnd, &msg).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        EnableWindow(parent, true);
        if parent.0 != 0 {
            let mut class_buf = [0u16; 64];
            let class_len = crate::get_class_name_w_safe(parent, &mut class_buf);
            let parent_class = if class_len > 0 {
                String::from_utf16_lossy(&class_buf[..class_len as usize])
            } else {
                String::new()
            };
            if parent_class == "SonarpadWin32" {
                crate::bring_window_to_foreground(parent);
                crate::log_if_err!(crate::post_message_w_safe(
                    parent,
                    crate::WM_FOCUS_EDITOR,
                    WPARAM(0),
                    LPARAM(0)
                ));
            } else {
                crate::set_foreground_window_safe(parent);
                crate::set_focus_safe(parent);
            }
        }

        if data.confirmed {
            data.choices.get(data.selected_index).cloned()
        } else {
            None
        }
    }
}

pub fn prompt_credentials(
    parent: HWND,
    title: &str,
    body: &str,
    username_default: &str,
    save_credentials_default: bool,
    language: Language,
) -> Option<PromptCredentialsResult> {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide("SonarpadCredentialsPrompt");

        static ONCE_CREDENTIALS: std::sync::Once = std::sync::Once::new();
        ONCE_CREDENTIALS.call_once(|| {
            let wc = WNDCLASSW {
                hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                    LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
                ),
                hInstance: hinstance,
                lpszClassName: PCWSTR(class_name.as_ptr()),
                lpfnWndProc: Some(credentials_prompt_wndproc),
                ..Default::default()
            };
            RegisterClassW(&wc);
        });

        let mut data = CredentialsPromptData {
            body: body.to_string(),
            username: username_default.to_string(),
            password: String::new(),
            tertiary: String::new(),
            save_credentials: save_credentials_default,
            directory_selected_index: 0,
            secondary_directory_selected_index: 0,
            tertiary_directory_selected_index: 0,
            quaternary_directory_selected_index: 0,
            confirmed: false,
            language,
            mode: CredentialsPromptMode::Credentials,
        };

        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide(title).as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            200,
            200,
            430,
            272,
            parent,
            HMENU(0),
            hinstance,
            Some(&mut data as *mut _ as *const std::ffi::c_void),
        );

        if hwnd.0 == 0 {
            return None;
        }

        windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(parent, false);

        let mut msg = MSG::default();
        while IsWindow(hwnd).as_bool() && GetMessageW(&mut msg, HWND(0), 0, 0).into() {
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&msg) {
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_TAB.0 as u32 {
                SendMessageW(hwnd, WM_KEYDOWN, msg.wParam, msg.lParam);
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

        windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(parent, true);
        if parent.0 != 0 {
            crate::bring_window_to_foreground(parent);
            crate::log_if_err!(crate::post_message_w_safe(
                parent,
                crate::WM_FOCUS_EDITOR,
                WPARAM(0),
                LPARAM(0)
            ));
        }

        if data.confirmed {
            Some(PromptCredentialsResult {
                username: data.username,
                password: data.password,
                save_credentials: data.save_credentials,
            })
        } else {
            None
        }
    }
}

pub fn prompt_directory_search(
    parent: HWND,
    options: PromptDirectoryOptions,
    language: Language,
) -> Option<PromptDirectoryResult> {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide("SonarpadCredentialsPrompt");

        static ONCE_CREDENTIALS: std::sync::Once = std::sync::Once::new();
        ONCE_CREDENTIALS.call_once(|| {
            let wc = WNDCLASSW {
                hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                    LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
                ),
                hInstance: hinstance,
                lpszClassName: PCWSTR(class_name.as_ptr()),
                lpfnWndProc: Some(credentials_prompt_wndproc),
                ..Default::default()
            };
            RegisterClassW(&wc);
        });

        let extra_combo_count = usize::from(!options.secondary_options.is_empty())
            + usize::from(!options.tertiary_options.is_empty())
            + usize::from(!options.quaternary_options.is_empty());
        let checkbox_extra_height = if options.checkbox_label.trim().is_empty() {
            0
        } else {
            34
        };
        let window_height = 304 + (extra_combo_count as i32 * 36) + checkbox_extra_height;

        let mut data = CredentialsPromptData {
            body: options.type_label,
            username: options.primary_default,
            password: options.secondary_default,
            tertiary: options.tertiary_default,
            save_credentials: options.checkbox_default,
            directory_selected_index: options
                .default_selection
                .min(options.options.len().saturating_sub(1)),
            secondary_directory_selected_index: options
                .secondary_default_selection
                .min(options.secondary_options.len().saturating_sub(1)),
            tertiary_directory_selected_index: options
                .tertiary_default_selection
                .min(options.tertiary_options.len().saturating_sub(1)),
            quaternary_directory_selected_index: options
                .quaternary_default_selection
                .min(options.quaternary_options.len().saturating_sub(1)),
            confirmed: false,
            language,
            mode: CredentialsPromptMode::DirectorySearch(Box::new(DirectorySearchPromptMode {
                options: options.options,
                selected_index: options.default_selection,
                secondary_type_label: options.secondary_type_label,
                secondary_options: options.secondary_options,
                secondary_selected_index: options.secondary_default_selection,
                tertiary_type_label: options.tertiary_type_label,
                tertiary_options: options.tertiary_options,
                tertiary_selected_index: options.tertiary_default_selection,
                tertiary_options_primary_index_only: options.tertiary_options_primary_index_only,
                quaternary_type_label: options.quaternary_type_label,
                quaternary_options: options.quaternary_options,
                quaternary_selected_index: options.quaternary_default_selection,
                focus_primary_field: options.focus_primary_field,
                primary_label: options.primary_label,
                primary_labels: options.primary_labels,
                secondary_label: options.secondary_label,
                tertiary_label: options.tertiary_label,
                checkbox_label: options.checkbox_label,
            })),
        };

        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(to_wide(&options.title).as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            200,
            200,
            520,
            window_height,
            parent,
            HMENU(0),
            hinstance,
            Some(&mut data as *mut _ as *const std::ffi::c_void),
        );

        if hwnd.0 == 0 {
            return None;
        }

        windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(parent, false);

        let mut msg = MSG::default();
        while IsWindow(hwnd).as_bool() && GetMessageW(&mut msg, HWND(0), 0, 0).into() {
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&msg) {
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_TAB.0 as u32 {
                SendMessageW(hwnd, WM_KEYDOWN, msg.wParam, msg.lParam);
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

        windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(parent, true);
        if !data.confirmed && parent.0 != 0 {
            crate::log_debug(&format!(
                "prompt_directory_search close restore: parent={:?} foreground_before={:?} focus_before={:?}",
                parent,
                crate::get_foreground_window_safe(),
                crate::get_focus_safe()
            ));
            crate::bring_window_to_foreground(parent);
            crate::log_if_err!(crate::post_message_w_safe(
                parent,
                crate::WM_FOCUS_EDITOR,
                WPARAM(0),
                LPARAM(0)
            ));
        }

        if data.confirmed {
            Some(PromptDirectoryResult {
                selected_index: data.directory_selected_index,
                secondary_selected_index: data.secondary_directory_selected_index,
                tertiary_selected_index: data.tertiary_directory_selected_index,
                quaternary_selected_index: data.quaternary_directory_selected_index,
                primary_value: data.username,
                secondary_value: data.password,
                tertiary_value: data.tertiary,
                checkbox_checked: data.save_credentials,
            })
        } else {
            None
        }
    }
}

struct SimplePromptData {
    body: String,
    value: String,
    confirmed: bool,
    language: Language,
    masked: bool,
}

struct ChoicePromptData {
    body: String,
    choices: Vec<String>,
    selected_index: usize,
    confirmed: bool,
    language: Language,
}

struct CredentialsPromptData {
    body: String,
    username: String,
    password: String,
    tertiary: String,
    save_credentials: bool,
    directory_selected_index: usize,
    secondary_directory_selected_index: usize,
    tertiary_directory_selected_index: usize,
    quaternary_directory_selected_index: usize,
    confirmed: bool,
    language: Language,
    mode: CredentialsPromptMode,
}

#[derive(Clone)]
enum CredentialsPromptMode {
    Credentials,
    DirectorySearch(Box<DirectorySearchPromptMode>),
}

#[derive(Clone)]
struct DirectorySearchPromptMode {
    options: Vec<String>,
    selected_index: usize,
    secondary_type_label: String,
    secondary_options: Vec<String>,
    secondary_selected_index: usize,
    tertiary_type_label: String,
    tertiary_options: Vec<String>,
    tertiary_selected_index: usize,
    tertiary_options_primary_index_only: Option<usize>,
    quaternary_type_label: String,
    quaternary_options: Vec<String>,
    quaternary_selected_index: usize,
    focus_primary_field: bool,
    primary_label: String,
    primary_labels: Vec<String>,
    secondary_label: String,
    tertiary_label: String,
    checkbox_label: String,
}

unsafe extern "system" fn simple_prompt_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "simple_prompt_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || simple_prompt_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn simple_prompt_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create_struct =
                lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            let data_ptr = unsafe { (*create_struct).lpCreateParams as *mut SimplePromptData };
            crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, data_ptr as isize);
            let Some((body_text, value_text, masked)) =
                crate::with_raw_mut_ptr_safe(data_ptr, |data| {
                    (data.body.clone(), data.value.clone(), data.masked)
                })
            else {
                crate::log_debug("Prompt create params pointer unavailable");
                return LRESULT(0);
            };
            let hfont = HFONT(
                crate::get_stock_object_safe(windows::Win32::Graphics::Gdi::DEFAULT_GUI_FONT).0,
            );

            let language = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA)
                .try_into()
                .ok()
                .and_then(|ptr: usize| {
                    crate::with_raw_mut_ptr_safe(ptr as *mut SimplePromptData, |data| data.language)
                })
                .unwrap_or_default();

            let _label = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&body_text).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    20,
                    18,
                    400,
                    46,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                )
            };

            let edit = unsafe {
                let edit_style = if masked {
                    ES_AUTOHSCROLL | ES_PASSWORD
                } else {
                    ES_AUTOHSCROLL
                };
                CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR(to_wide(&value_text).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(edit_style as u32),
                    20,
                    72,
                    395,
                    26,
                    hwnd,
                    HMENU(101),
                    HINSTANCE(0),
                    None,
                )
            };

            let ok = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "options.ok")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    212,
                    116,
                    90,
                    30,
                    hwnd,
                    HMENU(1),
                    HINSTANCE(0),
                    None,
                )
            };

            let cancel = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "options.cancel")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    312,
                    116,
                    103,
                    30,
                    hwnd,
                    HMENU(2),
                    HINSTANCE(0),
                    None,
                )
            };

            if hfont.0 != 0 {
                unsafe {
                    SendMessageW(_label, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    SendMessageW(edit, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    SendMessageW(ok, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    SendMessageW(cancel, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                }
            }

            unsafe {
                SetFocus(edit);
                SendMessageW(edit, 0x00B1, WPARAM(0), LPARAM(-1)); // EM_SETSEL: select all
            }

            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wparam.0 & 0xffff;
            if id == 1 {
                // OK
                let ptr =
                    crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut SimplePromptData;
                if !ptr.is_null() {
                    let edit = crate::get_dlg_item_safe(hwnd, 101);
                    let len = crate::get_window_text_length_w_safe(edit);
                    let mut buf = vec![0u16; (len + 1) as usize];
                    let read = crate::get_window_text_w_safe(edit, &mut buf);
                    let value = String::from_utf16_lossy(&buf[..read as usize]);
                    if crate::with_raw_mut_ptr_safe(ptr, |data| {
                        data.value = value;
                        data.confirmed = true;
                    })
                    .is_none()
                    {
                        crate::log_debug("Prompt state pointer unavailable during OK handling");
                    }
                }
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
            } else if id == 2 {
                // Cancel
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let key = wparam.0 as u32;
            if key == VK_TAB.0 as u32 {
                let shift_down =
                    (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
                let current_focus = crate::get_focus_safe();
                let edit = crate::get_dlg_item_safe(hwnd, 101);
                let ok = crate::get_dlg_item_safe(hwnd, 1);
                let cancel = crate::get_dlg_item_safe(hwnd, 2);

                let order = [edit, ok, cancel];
                let mut idx = order.iter().position(|&h| h == current_focus).unwrap_or(0);

                if shift_down {
                    idx = if idx == 0 { order.len() - 1 } else { idx - 1 };
                } else {
                    idx = (idx + 1) % order.len();
                }
                crate::set_focus_safe(order[idx]);
                return LRESULT(0);
            }
            if key == VK_RETURN.0 as u32 {
                crate::send_message_w_safe(hwnd, WM_COMMAND, WPARAM(1), LPARAM(0));
                return LRESULT(0);
            }
            if key == VK_ESCAPE.0 as u32 {
                crate::send_message_w_safe(hwnd, WM_COMMAND, WPARAM(2), LPARAM(0));
                return LRESULT(0);
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            LRESULT(0)
        }
        _ => crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn choice_prompt_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "choice_prompt_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || choice_prompt_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn choice_prompt_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    const IDC_CHOICE: isize = 101;
    match msg {
        WM_CREATE => {
            let create_struct =
                lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            let data_ptr = unsafe { (*create_struct).lpCreateParams as *mut ChoicePromptData };
            crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, data_ptr as isize);
            let Some((body_text, choices, selected_index, language)) =
                crate::with_raw_mut_ptr_safe(data_ptr, |data| {
                    (
                        data.body.clone(),
                        data.choices.clone(),
                        data.selected_index,
                        data.language,
                    )
                })
            else {
                crate::log_debug("Choice prompt create params pointer unavailable");
                return LRESULT(0);
            };

            let hfont = HFONT(
                crate::get_stock_object_safe(windows::Win32::Graphics::Gdi::DEFAULT_GUI_FONT).0,
            );
            let label = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&body_text).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    20,
                    20,
                    380,
                    55,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                )
            };
            let combo = unsafe {
                CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    20,
                    82,
                    380,
                    220,
                    hwnd,
                    HMENU(IDC_CHOICE),
                    HINSTANCE(0),
                    None,
                )
            };
            for choice in &choices {
                unsafe {
                    SendMessageW(
                        combo,
                        CB_ADDSTRING,
                        WPARAM(0),
                        LPARAM(to_wide(choice).as_ptr() as isize),
                    );
                }
            }
            unsafe {
                SendMessageW(
                    combo,
                    CB_SETCURSEL,
                    WPARAM(selected_index.min(choices.len().saturating_sub(1))),
                    LPARAM(0),
                );
            }
            let ok = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "options.ok")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    215,
                    132,
                    80,
                    28,
                    hwnd,
                    HMENU(1),
                    HINSTANCE(0),
                    None,
                )
            };
            let cancel = unsafe {
                CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&i18n::tr(language, "options.cancel")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    305,
                    132,
                    95,
                    28,
                    hwnd,
                    HMENU(2),
                    HINSTANCE(0),
                    None,
                )
            };

            if hfont.0 != 0 {
                unsafe {
                    for control in [label, combo, ok, cancel] {
                        SendMessageW(control, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                }
            }
            crate::set_focus_safe(combo);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wparam.0 & 0xffff;
            if id == 1 {
                let ptr =
                    crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut ChoicePromptData;
                if !ptr.is_null() {
                    let combo = crate::get_dlg_item_safe(hwnd, IDC_CHOICE as i32);
                    let selected =
                        unsafe { SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 };
                    if selected >= 0
                        && crate::with_raw_mut_ptr_safe(ptr, |data| {
                            data.selected_index = selected as usize;
                            data.confirmed = true;
                        })
                        .is_none()
                    {
                        crate::log_debug(
                            "Choice prompt: state pointer unavailable while confirming selection",
                        );
                    }
                }
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
            } else if id == 2 {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let key = wparam.0 as u32;
            if key == VK_TAB.0 as u32 {
                let shift_down =
                    (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
                let current_focus = crate::get_focus_safe();
                let combo = crate::get_dlg_item_safe(hwnd, IDC_CHOICE as i32);
                let ok = crate::get_dlg_item_safe(hwnd, 1);
                let cancel = crate::get_dlg_item_safe(hwnd, 2);
                let order = [combo, ok, cancel];
                let mut idx = order.iter().position(|&h| h == current_focus).unwrap_or(0);
                if shift_down {
                    idx = if idx == 0 { order.len() - 1 } else { idx - 1 };
                } else {
                    idx = (idx + 1) % order.len();
                }
                crate::set_focus_safe(order[idx]);
                return LRESULT(0);
            }
            if key == VK_RETURN.0 as u32 {
                crate::send_message_w_safe(hwnd, WM_COMMAND, WPARAM(1), LPARAM(0));
                return LRESULT(0);
            }
            if key == VK_ESCAPE.0 as u32 {
                crate::send_message_w_safe(hwnd, WM_COMMAND, WPARAM(2), LPARAM(0));
                return LRESULT(0);
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            LRESULT(0)
        }
        _ => crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn credentials_prompt_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "credentials_prompt_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || credentials_prompt_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn credentials_prompt_wndproc_inner(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    const IDC_CREDENTIALS_USER: isize = 201;
    const IDC_CREDENTIALS_PASS: isize = 202;
    const IDC_CREDENTIALS_SAVE: isize = 203;
    const IDC_CREDENTIALS_KIND: i32 = 204;
    const IDC_CREDENTIALS_USER_LABEL: isize = 205;
    const IDC_CREDENTIALS_TERTIARY: i32 = 206;
    const IDC_CREDENTIALS_SECONDARY_KIND: i32 = 207;
    const IDC_CREDENTIALS_TERTIARY_KIND: i32 = 208;
    const IDC_CREDENTIALS_TERTIARY_KIND_LABEL: i32 = 209;
    const IDC_CREDENTIALS_QUATERNARY_KIND: i32 = 210;
    match msg {
        WM_CREATE => {
            let create_struct =
                lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            let data_ptr = unsafe { (*create_struct).lpCreateParams as *mut CredentialsPromptData };
            crate::set_window_long_ptr_w_safe(hwnd, GWLP_USERDATA, data_ptr as isize);
            let Some((
                body_text,
                username_value,
                password_value,
                tertiary_value,
                save_credentials,
                language,
                mode,
            )) = crate::with_raw_mut_ptr_safe(data_ptr, |data| {
                (
                    data.body.clone(),
                    data.username.clone(),
                    data.password.clone(),
                    data.tertiary.clone(),
                    data.save_credentials,
                    data.language,
                    data.mode.clone(),
                )
            })
            else {
                crate::log_debug("Credentials prompt create params pointer unavailable");
                return LRESULT(0);
            };
            let hfont = HFONT(
                crate::get_stock_object_safe(windows::Win32::Graphics::Gdi::DEFAULT_GUI_FONT).0,
            );
            match mode {
                CredentialsPromptMode::Credentials => {
                    let username_label_text = i18n::tr(language, "stream_audio.auth_username");
                    let password_label_text = i18n::tr(language, "stream_audio.auth_password");
                    let save_credentials_text = i18n::tr(language, "stream_audio.save_credentials");

                    let body_label = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_STATIC,
                            PCWSTR(to_wide(&body_text).as_ptr()),
                            WS_CHILD | WS_VISIBLE,
                            20,
                            18,
                            380,
                            40,
                            hwnd,
                            HMENU(0),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let user_label = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_STATIC,
                            PCWSTR(to_wide(&username_label_text).as_ptr()),
                            WS_CHILD | WS_VISIBLE,
                            20,
                            76,
                            100,
                            20,
                            hwnd,
                            HMENU(0),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let user_edit = unsafe {
                        CreateWindowExW(
                            WS_EX_CLIENTEDGE,
                            WC_EDIT,
                            PCWSTR(to_wide(&username_value).as_ptr()),
                            WS_CHILD
                                | WS_VISIBLE
                                | WS_TABSTOP
                                | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                            128,
                            72,
                            250,
                            24,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_USER),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let pass_label = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_STATIC,
                            PCWSTR(to_wide(&password_label_text).as_ptr()),
                            WS_CHILD | WS_VISIBLE,
                            20,
                            112,
                            100,
                            20,
                            hwnd,
                            HMENU(0),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let pass_edit = unsafe {
                        CreateWindowExW(
                            WS_EX_CLIENTEDGE,
                            WC_EDIT,
                            PCWSTR::null(),
                            WS_CHILD
                                | WS_VISIBLE
                                | WS_TABSTOP
                                | WINDOW_STYLE((ES_AUTOHSCROLL | ES_PASSWORD) as u32),
                            128,
                            108,
                            250,
                            24,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_PASS),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let save_checkbox = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_BUTTON,
                            PCWSTR(to_wide(&save_credentials_text).as_ptr()),
                            WS_CHILD
                                | WS_VISIBLE
                                | WS_TABSTOP
                                | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                            20,
                            150,
                            358,
                            24,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_SAVE),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let ok = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_BUTTON,
                            PCWSTR(to_wide(&i18n::tr(language, "options.ok")).as_ptr()),
                            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                            193,
                            192,
                            80,
                            28,
                            hwnd,
                            HMENU(1),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let cancel = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_BUTTON,
                            PCWSTR(to_wide(&i18n::tr(language, "options.cancel")).as_ptr()),
                            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                            283,
                            192,
                            95,
                            28,
                            hwnd,
                            HMENU(2),
                            HINSTANCE(0),
                            None,
                        )
                    };

                    if hfont.0 != 0 {
                        unsafe {
                            for control in [
                                body_label,
                                user_label,
                                user_edit,
                                pass_label,
                                pass_edit,
                                save_checkbox,
                                ok,
                                cancel,
                            ] {
                                SendMessageW(
                                    control,
                                    WM_SETFONT,
                                    WPARAM(hfont.0 as usize),
                                    LPARAM(1),
                                );
                            }
                        }
                    }

                    if save_credentials {
                        unsafe {
                            SendMessageW(
                                save_checkbox,
                                BM_SETCHECK,
                                WPARAM(BST_CHECKED.0 as usize),
                                LPARAM(0),
                            );
                        }
                    }

                    unsafe {
                        SetFocus(user_edit);
                        SendMessageW(user_edit, 0x00B1, WPARAM(0), LPARAM(-1));
                    }
                }
                CredentialsPromptMode::DirectorySearch(config) => {
                    let options = &config.options;
                    let selected_index = config.selected_index;
                    let secondary_type_label = &config.secondary_type_label;
                    let secondary_options = &config.secondary_options;
                    let secondary_selected_index = config.secondary_selected_index;
                    let tertiary_type_label = &config.tertiary_type_label;
                    let tertiary_options = &config.tertiary_options;
                    let tertiary_selected_index = config.tertiary_selected_index;
                    let quaternary_type_label = &config.quaternary_type_label;
                    let quaternary_options = &config.quaternary_options;
                    let quaternary_selected_index = config.quaternary_selected_index;
                    let focus_primary_field = config.focus_primary_field;
                    let primary_label = &config.primary_label;
                    let primary_labels = &config.primary_labels;
                    let secondary_label = &config.secondary_label;
                    let tertiary_label = &config.tertiary_label;
                    let show_secondary =
                        !secondary_label.trim().is_empty() || !password_value.is_empty();
                    let show_tertiary =
                        !tertiary_label.trim().is_empty() || !tertiary_value.is_empty();
                    let show_secondary_kind = !secondary_options.is_empty();
                    let show_tertiary_kind = !tertiary_options.is_empty();
                    let show_quaternary_kind = !quaternary_options.is_empty();
                    let tertiary_primary_index_only = config.tertiary_options_primary_index_only;
                    let show_tertiary_kind_initial = show_tertiary_kind
                        && tertiary_primary_index_only.is_none_or(|idx| selected_index == idx);
                    let mut y = 20;
                    let kind_label = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_STATIC,
                            PCWSTR(to_wide(&body_text).as_ptr()),
                            WS_CHILD | WS_VISIBLE,
                            20,
                            y + 4,
                            154,
                            20,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_TERTIARY_KIND_LABEL as isize),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let kind_combo = unsafe {
                        CreateWindowExW(
                            WS_EX_CLIENTEDGE,
                            WC_COMBOBOXW,
                            PCWSTR::null(),
                            WS_CHILD
                                | WS_VISIBLE
                                | WS_TABSTOP
                                | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                            182,
                            y,
                            300,
                            180,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_KIND as isize),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    y += 36;
                    let secondary_kind_label = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_STATIC,
                            PCWSTR(to_wide(secondary_type_label).as_ptr()),
                            if show_secondary_kind {
                                WS_CHILD | WS_VISIBLE
                            } else {
                                WS_CHILD
                            },
                            20,
                            y + 4,
                            154,
                            20,
                            hwnd,
                            HMENU(0),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let secondary_kind_combo = unsafe {
                        CreateWindowExW(
                            WS_EX_CLIENTEDGE,
                            WC_COMBOBOXW,
                            PCWSTR::null(),
                            if show_secondary_kind {
                                WS_CHILD
                                    | WS_VISIBLE
                                    | WS_TABSTOP
                                    | WINDOW_STYLE(CBS_DROPDOWNLIST as u32)
                            } else {
                                WS_CHILD | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32)
                            },
                            182,
                            y,
                            300,
                            180,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_SECONDARY_KIND as isize),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    if show_secondary_kind {
                        y += 36;
                    }
                    let tertiary_kind_label = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_STATIC,
                            PCWSTR(to_wide(tertiary_type_label).as_ptr()),
                            if show_tertiary_kind_initial {
                                WS_CHILD | WS_VISIBLE
                            } else {
                                WS_CHILD
                            },
                            20,
                            y + 4,
                            154,
                            20,
                            hwnd,
                            HMENU(0),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let tertiary_kind_combo = unsafe {
                        CreateWindowExW(
                            WS_EX_CLIENTEDGE,
                            WC_COMBOBOXW,
                            PCWSTR::null(),
                            if show_tertiary_kind_initial {
                                WS_CHILD
                                    | WS_VISIBLE
                                    | WS_TABSTOP
                                    | WINDOW_STYLE(CBS_DROPDOWNLIST as u32)
                            } else {
                                WS_CHILD | WINDOW_STYLE(CBS_DROPDOWNLIST as u32)
                            },
                            182,
                            y,
                            300,
                            180,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_TERTIARY_KIND as isize),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    if show_tertiary_kind {
                        y += 36;
                    }
                    let quaternary_kind_label = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_STATIC,
                            PCWSTR(to_wide(quaternary_type_label).as_ptr()),
                            if show_quaternary_kind {
                                WS_CHILD | WS_VISIBLE
                            } else {
                                WS_CHILD
                            },
                            20,
                            y + 4,
                            154,
                            20,
                            hwnd,
                            HMENU(0),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let quaternary_kind_combo = unsafe {
                        CreateWindowExW(
                            WS_EX_CLIENTEDGE,
                            WC_COMBOBOXW,
                            PCWSTR::null(),
                            if show_quaternary_kind {
                                WS_CHILD
                                    | WS_VISIBLE
                                    | WS_TABSTOP
                                    | WINDOW_STYLE(CBS_DROPDOWNLIST as u32)
                            } else {
                                WS_CHILD | WINDOW_STYLE(CBS_DROPDOWNLIST as u32)
                            },
                            182,
                            y,
                            300,
                            180,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_QUATERNARY_KIND as isize),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    if show_quaternary_kind {
                        y += 36;
                    }
                    let user_label = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_STATIC,
                            PCWSTR(to_wide(primary_label).as_ptr()),
                            WS_CHILD | WS_VISIBLE,
                            20,
                            y,
                            154,
                            32,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_USER_LABEL),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let user_edit = unsafe {
                        CreateWindowExW(
                            WS_EX_CLIENTEDGE,
                            WC_EDIT,
                            PCWSTR(to_wide(&username_value).as_ptr()),
                            WS_CHILD
                                | WS_VISIBLE
                                | WS_TABSTOP
                                | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                            182,
                            y,
                            300,
                            24,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_USER),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    y += 36;
                    let pass_label = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_STATIC,
                            PCWSTR(to_wide(secondary_label).as_ptr()),
                            if show_secondary {
                                WS_CHILD | WS_VISIBLE
                            } else {
                                WS_CHILD
                            },
                            20,
                            y + 4,
                            154,
                            20,
                            hwnd,
                            HMENU(0),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let pass_edit = unsafe {
                        CreateWindowExW(
                            WS_EX_CLIENTEDGE,
                            WC_EDIT,
                            PCWSTR(to_wide(&password_value).as_ptr()),
                            if show_secondary {
                                WS_CHILD
                                    | WS_VISIBLE
                                    | WS_TABSTOP
                                    | WINDOW_STYLE(ES_AUTOHSCROLL as u32)
                            } else {
                                WS_CHILD | WINDOW_STYLE(ES_AUTOHSCROLL as u32)
                            },
                            182,
                            y,
                            300,
                            24,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_PASS),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    if show_secondary {
                        y += 36;
                    }
                    let tertiary_label_hwnd = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_STATIC,
                            PCWSTR(to_wide(tertiary_label).as_ptr()),
                            if show_tertiary {
                                WS_CHILD | WS_VISIBLE
                            } else {
                                WS_CHILD
                            },
                            20,
                            y + 4,
                            154,
                            20,
                            hwnd,
                            HMENU(0),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let tertiary_edit = unsafe {
                        CreateWindowExW(
                            WS_EX_CLIENTEDGE,
                            WC_EDIT,
                            PCWSTR(to_wide(&tertiary_value).as_ptr()),
                            if show_tertiary {
                                WS_CHILD
                                    | WS_VISIBLE
                                    | WS_TABSTOP
                                    | WINDOW_STYLE(ES_AUTOHSCROLL as u32)
                            } else {
                                WS_CHILD | WINDOW_STYLE(ES_AUTOHSCROLL as u32)
                            },
                            182,
                            y,
                            300,
                            24,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_TERTIARY as isize),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    if show_tertiary {
                        y += 36;
                    }
                    let show_checkbox = !config.checkbox_label.trim().is_empty();
                    let checkbox = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_BUTTON,
                            PCWSTR(to_wide(&config.checkbox_label).as_ptr()),
                            if show_checkbox {
                                WS_CHILD
                                    | WS_VISIBLE
                                    | WS_TABSTOP
                                    | WINDOW_STYLE(BS_AUTOCHECKBOX as u32)
                            } else {
                                WS_CHILD | WINDOW_STYLE(BS_AUTOCHECKBOX as u32)
                            },
                            20,
                            y + 4,
                            462,
                            24,
                            hwnd,
                            HMENU(IDC_CREDENTIALS_SAVE),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    if save_credentials {
                        unsafe {
                            SendMessageW(
                                checkbox,
                                BM_SETCHECK,
                                WPARAM(BST_CHECKED.0 as usize),
                                LPARAM(0),
                            );
                        }
                    }
                    if show_checkbox {
                        y += 34;
                    }
                    let ok = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_BUTTON,
                            PCWSTR(to_wide(&i18n::tr(language, "options.ok")).as_ptr()),
                            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                            292,
                            y + 20,
                            90,
                            30,
                            hwnd,
                            HMENU(1),
                            HINSTANCE(0),
                            None,
                        )
                    };
                    let cancel = unsafe {
                        CreateWindowExW(
                            Default::default(),
                            WC_BUTTON,
                            PCWSTR(to_wide(&i18n::tr(language, "options.cancel")).as_ptr()),
                            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                            392,
                            y + 20,
                            90,
                            30,
                            hwnd,
                            HMENU(2),
                            HINSTANCE(0),
                            None,
                        )
                    };

                    for option in options {
                        let option_wide = to_wide(option);
                        unsafe {
                            SendMessageW(
                                kind_combo,
                                CB_ADDSTRING,
                                WPARAM(0),
                                LPARAM(option_wide.as_ptr() as isize),
                            );
                        }
                    }
                    for option in secondary_options {
                        let option_wide = to_wide(option);
                        unsafe {
                            SendMessageW(
                                secondary_kind_combo,
                                CB_ADDSTRING,
                                WPARAM(0),
                                LPARAM(option_wide.as_ptr() as isize),
                            );
                        }
                    }
                    for option in tertiary_options {
                        let option_wide = to_wide(option);
                        unsafe {
                            SendMessageW(
                                tertiary_kind_combo,
                                CB_ADDSTRING,
                                WPARAM(0),
                                LPARAM(option_wide.as_ptr() as isize),
                            );
                        }
                    }
                    for option in quaternary_options {
                        let option_wide = to_wide(option);
                        unsafe {
                            SendMessageW(
                                quaternary_kind_combo,
                                CB_ADDSTRING,
                                WPARAM(0),
                                LPARAM(option_wide.as_ptr() as isize),
                            );
                        }
                    }
                    unsafe {
                        SendMessageW(
                            kind_combo,
                            CB_SETCURSEL,
                            WPARAM(selected_index.min(options.len().saturating_sub(1))),
                            LPARAM(0),
                        );
                        if show_secondary_kind {
                            SendMessageW(
                                secondary_kind_combo,
                                CB_SETCURSEL,
                                WPARAM(
                                    secondary_selected_index
                                        .min(secondary_options.len().saturating_sub(1)),
                                ),
                                LPARAM(0),
                            );
                        }
                        if show_tertiary_kind {
                            SendMessageW(
                                tertiary_kind_combo,
                                CB_SETCURSEL,
                                WPARAM(
                                    tertiary_selected_index
                                        .min(tertiary_options.len().saturating_sub(1)),
                                ),
                                LPARAM(0),
                            );
                        }
                        if show_quaternary_kind {
                            SendMessageW(
                                quaternary_kind_combo,
                                CB_SETCURSEL,
                                WPARAM(
                                    quaternary_selected_index
                                        .min(quaternary_options.len().saturating_sub(1)),
                                ),
                                LPARAM(0),
                            );
                        }
                    }
                    if let Some(selected_primary_label) = primary_labels.get(selected_index) {
                        crate::log_if_err!(crate::set_window_text_w_safe(
                            user_label,
                            PCWSTR(to_wide(selected_primary_label).as_ptr())
                        ));
                    }

                    if let Some(primary_index) = tertiary_primary_index_only {
                        let show = selected_index == primary_index;
                        let command = if show { SW_SHOW } else { SW_HIDE };
                        unsafe {
                            ShowWindow(tertiary_kind_label, command);
                            ShowWindow(tertiary_kind_combo, command);
                            EnableWindow(tertiary_kind_combo, show);
                        }
                        crate::log_debug(&format!(
                            "route_prompt_init_avoid_combo: selected_index={} show={} label={:?} combo={:?}",
                            selected_index, show, tertiary_kind_label, tertiary_kind_combo
                        ));
                    }

                    if hfont.0 != 0 {
                        unsafe {
                            for control in [
                                kind_label,
                                kind_combo,
                                secondary_kind_label,
                                secondary_kind_combo,
                                tertiary_kind_label,
                                tertiary_kind_combo,
                                quaternary_kind_label,
                                quaternary_kind_combo,
                                user_label,
                                user_edit,
                                pass_label,
                                pass_edit,
                                tertiary_label_hwnd,
                                tertiary_edit,
                                checkbox,
                                ok,
                                cancel,
                            ] {
                                SendMessageW(
                                    control,
                                    WM_SETFONT,
                                    WPARAM(hfont.0 as usize),
                                    LPARAM(1),
                                );
                            }
                        }
                    }

                    unsafe {
                        if focus_primary_field {
                            SetFocus(user_edit);
                        } else {
                            SetFocus(kind_combo);
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wparam.0 & 0xffff;
            let notify = ((wparam.0 >> 16) & 0xffff) as u16;
            if id == IDC_CREDENTIALS_KIND as usize
                && notify == windows::Win32::UI::WindowsAndMessaging::CBN_SELCHANGE as u16
            {
                let combo_value = crate::send_message_w_safe(
                    crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_KIND),
                    CB_GETCURSEL,
                    WPARAM(0),
                    LPARAM(0),
                )
                .0;
                let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA)
                    as *mut CredentialsPromptData;
                if combo_value >= 0 {
                    let _updated = crate::with_raw_mut_ptr_safe(ptr, |data| {
                        data.directory_selected_index = combo_value as usize;
                        if let CredentialsPromptMode::DirectorySearch(config) = &data.mode
                            && let Some(label) = config.primary_labels.get(combo_value as usize)
                        {
                            let user_label =
                                crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_USER_LABEL as i32);
                            crate::log_if_err!(crate::set_window_text_w_safe(
                                user_label,
                                PCWSTR(to_wide(label).as_ptr())
                            ));
                        }
                        if let CredentialsPromptMode::DirectorySearch(config) = &data.mode
                            && !config.tertiary_options.is_empty()
                            && config.tertiary_options_primary_index_only.is_some()
                        {
                            let show = config
                                .tertiary_options_primary_index_only
                                .is_some_and(|idx| combo_value as usize == idx);
                            let command = if show { SW_SHOW } else { SW_HIDE };
                            let label =
                                crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_TERTIARY_KIND_LABEL);
                            let combo =
                                crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_TERTIARY_KIND);

                            unsafe {
                                ShowWindow(label, command);
                                ShowWindow(combo, command);
                                EnableWindow(combo, show);
                            }
                            crate::log_debug(&format!(
                                "route_prompt_change_avoid_combo: selected_index={} show={} label={:?} combo={:?} focus={:?}",
                                combo_value,
                                show,
                                label,
                                combo,
                                crate::get_focus_safe()
                            ));

                            if !show {
                                data.tertiary_directory_selected_index = 0;
                                crate::send_message_w_safe(
                                    combo,
                                    CB_SETCURSEL,
                                    WPARAM(0),
                                    LPARAM(0),
                                );
                            }
                        }
                    });
                }
                return LRESULT(0);
            } else if id == IDC_CREDENTIALS_SECONDARY_KIND as usize
                && notify == windows::Win32::UI::WindowsAndMessaging::CBN_SELCHANGE as u16
            {
                let combo_value = crate::send_message_w_safe(
                    crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_SECONDARY_KIND),
                    CB_GETCURSEL,
                    WPARAM(0),
                    LPARAM(0),
                )
                .0;
                let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA)
                    as *mut CredentialsPromptData;
                if combo_value >= 0 {
                    let _updated = crate::with_raw_mut_ptr_safe(ptr, |data| {
                        data.secondary_directory_selected_index = combo_value as usize;
                    });
                }
                return LRESULT(0);
            } else if id == IDC_CREDENTIALS_TERTIARY_KIND as usize
                && notify == windows::Win32::UI::WindowsAndMessaging::CBN_SELCHANGE as u16
            {
                let combo_value = crate::send_message_w_safe(
                    crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_TERTIARY_KIND),
                    CB_GETCURSEL,
                    WPARAM(0),
                    LPARAM(0),
                )
                .0;
                let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA)
                    as *mut CredentialsPromptData;
                if combo_value >= 0 {
                    let _updated = crate::with_raw_mut_ptr_safe(ptr, |data| {
                        data.tertiary_directory_selected_index = combo_value as usize;
                    });
                }
                return LRESULT(0);
            } else if id == IDC_CREDENTIALS_QUATERNARY_KIND as usize
                && notify == windows::Win32::UI::WindowsAndMessaging::CBN_SELCHANGE as u16
            {
                let combo_value = crate::send_message_w_safe(
                    crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_QUATERNARY_KIND),
                    CB_GETCURSEL,
                    WPARAM(0),
                    LPARAM(0),
                )
                .0;
                let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA)
                    as *mut CredentialsPromptData;
                if combo_value >= 0 {
                    let _updated = crate::with_raw_mut_ptr_safe(ptr, |data| {
                        data.quaternary_directory_selected_index = combo_value as usize;
                    });
                }
                return LRESULT(0);
            } else if id == 1 {
                let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA)
                    as *mut CredentialsPromptData;
                if !ptr.is_null() {
                    let user_edit = crate::get_dlg_item_safe(hwnd, 201);
                    let pass_edit = crate::get_dlg_item_safe(hwnd, 202);
                    let tertiary_edit = crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_TERTIARY);
                    let save_checkbox = crate::get_dlg_item_safe(hwnd, 203);
                    let secondary_kind_combo =
                        crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_SECONDARY_KIND);
                    let tertiary_kind_combo =
                        crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_TERTIARY_KIND);
                    let quaternary_kind_combo =
                        crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_QUATERNARY_KIND);
                    let user_len = crate::get_window_text_length_w_safe(user_edit);
                    let pass_len = crate::get_window_text_length_w_safe(pass_edit);
                    let tertiary_len = crate::get_window_text_length_w_safe(tertiary_edit);
                    let mut user_buf = vec![0u16; (user_len + 1) as usize];
                    let mut pass_buf = vec![0u16; (pass_len + 1) as usize];
                    let mut tertiary_buf = vec![0u16; (tertiary_len + 1) as usize];
                    let user_read = crate::get_window_text_w_safe(user_edit, &mut user_buf);
                    let pass_read = crate::get_window_text_w_safe(pass_edit, &mut pass_buf);
                    let tertiary_read =
                        crate::get_window_text_w_safe(tertiary_edit, &mut tertiary_buf);
                    let username = String::from_utf16_lossy(&user_buf[..user_read as usize]);
                    let password = String::from_utf16_lossy(&pass_buf[..pass_read as usize]);
                    let tertiary =
                        String::from_utf16_lossy(&tertiary_buf[..tertiary_read as usize]);
                    let save_credentials = crate::send_message_w_safe(
                        save_checkbox,
                        BM_GETCHECK,
                        WPARAM(0),
                        LPARAM(0),
                    )
                    .0 as u32
                        == BST_CHECKED.0;
                    let combo_value = crate::send_message_w_safe(
                        crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_KIND),
                        CB_GETCURSEL,
                        WPARAM(0),
                        LPARAM(0),
                    )
                    .0;
                    let secondary_combo_value = crate::send_message_w_safe(
                        secondary_kind_combo,
                        CB_GETCURSEL,
                        WPARAM(0),
                        LPARAM(0),
                    )
                    .0;
                    let tertiary_combo_value = crate::send_message_w_safe(
                        tertiary_kind_combo,
                        CB_GETCURSEL,
                        WPARAM(0),
                        LPARAM(0),
                    )
                    .0;
                    let quaternary_combo_value = crate::send_message_w_safe(
                        quaternary_kind_combo,
                        CB_GETCURSEL,
                        WPARAM(0),
                        LPARAM(0),
                    )
                    .0;
                    if crate::with_raw_mut_ptr_safe(ptr, |data| {
                        data.username = username;
                        data.password = password;
                        data.tertiary = tertiary;
                        data.save_credentials = save_credentials;
                        if combo_value >= 0 {
                            data.directory_selected_index = combo_value as usize;
                        }
                        if secondary_combo_value >= 0 {
                            data.secondary_directory_selected_index =
                                secondary_combo_value as usize;
                        }
                        if tertiary_combo_value >= 0 {
                            data.tertiary_directory_selected_index = tertiary_combo_value as usize;
                        }
                        if quaternary_combo_value >= 0 {
                            data.quaternary_directory_selected_index =
                                quaternary_combo_value as usize;
                        }
                        data.confirmed = true;
                    })
                    .is_none()
                    {
                        crate::log_debug(
                            "Credentials prompt state pointer unavailable during OK handling",
                        );
                    }
                }
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
            } else if id == 2 {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let key = wparam.0 as u32;
            if key == VK_TAB.0 as u32 {
                let shift_down =
                    (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
                let current_focus = crate::get_focus_safe();
                let profile_combo = crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_KIND);
                let preference_combo =
                    crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_SECONDARY_KIND);
                let avoid_combo = crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_TERTIARY_KIND);
                let route_auto_selected =
                    crate::send_message_w_safe(profile_combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0
                        == 2;
                crate::log_debug(&format!(
                    "route_prompt_tab: shift={} focus={:?} profile_combo={:?} preference_combo={:?} avoid_combo={:?} auto={}",
                    shift_down,
                    current_focus,
                    profile_combo,
                    preference_combo,
                    avoid_combo,
                    route_auto_selected
                ));

                if !shift_down && current_focus == preference_combo && route_auto_selected {
                    unsafe {
                        ShowWindow(avoid_combo, SW_SHOW);
                        EnableWindow(avoid_combo, true);
                    }
                    crate::set_focus_safe(avoid_combo);
                    crate::log_debug(&format!(
                        "route_prompt_tab_forced_avoid: after_focus={:?}",
                        crate::get_focus_safe()
                    ));
                    return LRESULT(0);
                }

                let order = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA)
                    .try_into()
                    .ok()
                    .and_then(|ptr: usize| {
                        crate::with_raw_mut_ptr_safe(ptr as *mut CredentialsPromptData, |data| {
                            match data.mode {
                                CredentialsPromptMode::Credentials => vec![
                                    crate::get_dlg_item_safe(hwnd, 201),
                                    crate::get_dlg_item_safe(hwnd, 202),
                                    crate::get_dlg_item_safe(hwnd, 203),
                                    crate::get_dlg_item_safe(hwnd, 1),
                                    crate::get_dlg_item_safe(hwnd, 2),
                                ],
                                CredentialsPromptMode::DirectorySearch(_) => vec![
                                    crate::get_dlg_item_safe(hwnd, 204),
                                    crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_SECONDARY_KIND),
                                    crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_TERTIARY_KIND),
                                    crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_QUATERNARY_KIND),
                                    crate::get_dlg_item_safe(hwnd, 201),
                                    crate::get_dlg_item_safe(hwnd, 202),
                                    crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_TERTIARY),
                                    crate::get_dlg_item_safe(hwnd, 203),
                                    crate::get_dlg_item_safe(hwnd, 1),
                                    crate::get_dlg_item_safe(hwnd, 2),
                                ],
                            }
                        })
                    })
                    .unwrap_or_else(|| {
                        vec![
                            crate::get_dlg_item_safe(hwnd, 201),
                            crate::get_dlg_item_safe(hwnd, 202),
                            crate::get_dlg_item_safe(hwnd, 1),
                            crate::get_dlg_item_safe(hwnd, 2),
                        ]
                    });
                let route_auto_selected = order
                    .first()
                    .map(|profile_combo| {
                        crate::send_message_w_safe(
                            *profile_combo,
                            CB_GETCURSEL,
                            WPARAM(0),
                            LPARAM(0),
                        )
                        .0 == 2
                    })
                    .unwrap_or(false);
                let tertiary_combo = crate::get_dlg_item_safe(hwnd, IDC_CREDENTIALS_TERTIARY_KIND);
                let order: Vec<HWND> = order
                    .into_iter()
                    .filter(|hwnd| {
                        prompt_tab_stop_visible(hwnd, current_focus)
                            || (route_auto_selected && *hwnd == tertiary_combo)
                    })
                    .collect();
                if order.is_empty() {
                    return LRESULT(0);
                }
                let mut idx = order.iter().position(|&h| h == current_focus).unwrap_or(0);
                if shift_down {
                    idx = if idx == 0 { order.len() - 1 } else { idx - 1 };
                } else {
                    idx = (idx + 1) % order.len();
                }
                crate::set_focus_safe(order[idx]);
                return LRESULT(0);
            }
            if key == VK_RETURN.0 as u32 {
                crate::send_message_w_safe(hwnd, WM_COMMAND, WPARAM(1), LPARAM(0));
                return LRESULT(0);
            }
            if key == VK_ESCAPE.0 as u32 {
                crate::send_message_w_safe(hwnd, WM_COMMAND, WPARAM(2), LPARAM(0));
                return LRESULT(0);
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_ACTIVATE => {
            if (wparam.0 & 0xffff) != 0 {
                focus_credentials_prompt_initial_control(hwnd, "activate.immediate");
                if credentials_prompt_needs_posted_refocus(hwnd) {
                    crate::log_if_err!(crate::post_message_w_safe(
                        hwnd,
                        WM_CREDENTIALS_PROMPT_REFOCUS,
                        WPARAM(0),
                        LPARAM(0),
                    ));
                }
            }
            crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam)
        }
        WM_CREDENTIALS_PROMPT_REFOCUS => {
            focus_credentials_prompt_initial_control(hwnd, "activate.posted");
            LRESULT(0)
        }
        WM_CLOSE => {
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            LRESULT(0)
        }
        _ => crate::def_window_proc_w_safe(hwnd, msg, wparam, lparam),
    }
}

fn credentials_prompt_needs_posted_refocus(hwnd: HWND) -> bool {
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut CredentialsPromptData;
    crate::with_raw_mut_ptr_safe(ptr, |data| match &data.mode {
        CredentialsPromptMode::Credentials => false,
        CredentialsPromptMode::DirectorySearch(config) => !config.focus_primary_field,
    })
    .unwrap_or(false)
}

fn focus_credentials_prompt_initial_control(hwnd: HWND, reason: &str) {
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut CredentialsPromptData;
    let target = crate::with_raw_mut_ptr_safe(ptr, |data| match &data.mode {
        CredentialsPromptMode::Credentials => crate::get_dlg_item_safe(hwnd, 201),
        CredentialsPromptMode::DirectorySearch(config) => {
            if config.focus_primary_field {
                crate::get_dlg_item_safe(hwnd, 201)
            } else {
                crate::get_dlg_item_safe(hwnd, 204)
            }
        }
    })
    .unwrap_or(HWND(0));
    if target.0 != 0 {
        let focus_before = crate::get_focus_safe();
        crate::set_focus_safe(target);
        crate::log_debug(&format!(
            "credentials_prompt_refocus: reason={} target={:?} focus_before={:?} focus_after={:?}",
            reason,
            target,
            focus_before,
            crate::get_focus_safe()
        ));
    }
}

fn prompt_tab_stop_visible(hwnd: &HWND, current_focus: HWND) -> bool {
    if hwnd.0 == 0 {
        return false;
    }

    *hwnd == current_focus
        || unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(*hwnd) }.as_bool()
}

pub fn open(parent: HWND) {
    open_with_command(parent, None, None);
}

pub fn open_with_command(
    parent: HWND,
    initial_command: Option<String>,
    working_dir: Option<PathBuf>,
) {
    unsafe {
        let existing = with_state(parent, |state| state.prompt_window).unwrap_or(HWND(0));
        if existing.0 != 0 {
            SetForegroundWindow(existing);
            if let Some(cmd) = initial_command {
                with_prompt_state(existing, |state| {
                    if let Some(session) = &state.session {
                        let newline = if state.program_is_codex { "\n" } else { "\r\n" };
                        let payload = format!("{}{}", cmd, newline);
                        session.write_input(&payload);
                    }
                });
            }
            return;
        }

        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(PROMPT_CLASS_NAME);
        let wc = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
            ),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(prompt_wndproc),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
        let labels = prompt_labels(language);
        let title = to_wide(&labels.title);

        let init = Box::new(PromptWindowInit {
            parent,
            initial_command,
            working_dir,
        });
        let init_ptr: *mut PromptWindowInit = Box::into_raw(init);

        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_SIZEBOX | WS_VISIBLE,
            140,
            140,
            720,
            520,
            None,
            HMENU(0),
            hinstance,
            Some(init_ptr as *const std::ffi::c_void),
        );

        if hwnd.0 == 0 {
            if !init_ptr.is_null() {
                let _unused_box = Box::from_raw(init_ptr);
            }
            return;
        }

        if with_state(parent, |state| {
            state.prompt_window = hwnd;
        })
        .is_none()
        {
            crate::log_debug("Failed to access prompt state");
        }
    }
}

pub fn handle_navigation(hwnd: HWND, msg: &MSG) -> bool {
    unsafe {
        let focus = GetFocus();
        if focus.0 == 0 {
            return false;
        }
        let focus_parent = GetParent(focus);
        if focus != hwnd && focus_parent != hwnd {
            return false;
        }

        if msg.message == WM_SYSKEYDOWN {
            if msg.wParam.0 as u32 == 'I' as u32 {
                if with_prompt_state(hwnd, |state| {
                    SetFocus(state.input);
                })
                .is_none()
                {
                    crate::log_debug("Failed to access prompt state");
                }
                return true;
            }
            if msg.wParam.0 as u32 == 'O' as u32 {
                if with_prompt_state(hwnd, |state| {
                    SetFocus(state.output);
                })
                .is_none()
                {
                    crate::log_debug("Failed to access prompt state");
                }
                return true;
            }
            return false;
        }

        if msg.message != WM_KEYDOWN {
            return false;
        }

        if msg.wParam.0 as u32 == VK_TAB.0 as u32 {
            let shift_down = (GetKeyState(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
            if with_prompt_state(hwnd, |state| {
                let order = [
                    state.input,
                    state.output,
                    state.checkbox_autoscroll,
                    state.checkbox_strip_ansi,
                    state.checkbox_announce_lines,
                    state.checkbox_beep_on_idle,
                    state.checkbox_prevent_sleep,
                ];
                let mut idx = order.iter().position(|&h| h == focus).unwrap_or(0);
                if shift_down {
                    idx = if idx == 0 { order.len() - 1 } else { idx - 1 };
                } else {
                    idx = (idx + 1) % order.len();
                }
                SetFocus(order[idx]);
            })
            .is_none()
            {
                crate::log_debug("Failed to access prompt state");
            }
            return true;
        }

        if msg.wParam.0 as u32 == VK_RETURN.0 as u32 {
            if with_prompt_state(hwnd, |state| {
                if focus == state.input {
                    send_input_to_pty(state);
                }
            })
            .is_none()
            {
                crate::log_debug("Failed to access prompt state");
            }
            return true;
        }

        let ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
        if ctrl_down && msg.wParam.0 as u32 == 'V' as u32 {
            let mut handled = false;
            if with_prompt_state(hwnd, |state| {
                if focus != state.input {
                    return;
                }
                let Some(mut pasted) = read_clipboard_text(state.input) else {
                    return;
                };
                if !pasted.contains('\n') && !pasted.contains('\r') {
                    return;
                }
                pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
                let existing = {
                    let len = crate::get_window_text_length_w_safe(state.input);
                    if len <= 0 {
                        String::new()
                    } else {
                        let mut buffer = vec![0u16; (len + 1) as usize];
                        let read = crate::get_window_text_w_safe(state.input, &mut buffer);
                        String::from_utf16_lossy(&buffer[..read as usize])
                    }
                };
                let combined = if existing.is_empty() {
                    pasted
                } else if pasted.starts_with('\n') {
                    format!("{existing}{pasted}")
                } else {
                    format!("{existing}\n{pasted}")
                };
                let payload = if state.program_is_codex {
                    // Codex enables terminal bracketed-paste mode. Wrapping the clipboard
                    // payload makes embedded newlines paste into the composer instead of
                    // being interpreted as separate Enter/submit keypresses.
                    format!("\x1b[200~{combined}\x1b[201~")
                } else {
                    combined.replace('\n', "\r\n")
                };
                if let Some(session) = state.session.as_ref() {
                    if !session.write_input(&payload) {
                        crate::log_debug("Failed to write pasted multiline input");
                    } else {
                        if let Err(e) = crate::set_window_text_w_safe(state.input, PCWSTR::null()) {
                            crate::log_debug(&format!("Failed to clear prompt input: {e}"));
                        }
                        handled = true;
                    }
                }
            })
            .is_none()
            {
                crate::log_debug("Failed to access prompt state");
            }
            if handled {
                return true;
            }
        }
        if ctrl_down && msg.wParam.0 as u32 == 'C' as u32 {
            if with_prompt_state(hwnd, |state| {
                if focus == state.output {
                    copy_output_selection(state.output);
                } else if let Some(session) = state.session.as_ref()
                    && !session.send_ctrl_c()
                {
                    crate::log_debug("Failed to send Ctrl+C");
                }
            })
            .is_none()
            {
                crate::log_debug("Failed to access prompt state");
            }
            return true;
        }
        if ctrl_down && msg.wParam.0 as u32 == 'L' as u32 {
            if with_prompt_state(hwnd, |state| {
                if confirm_clear_output(hwnd, state.parent) {
                    clear_output(state);
                }
            })
            .is_none()
            {
                crate::log_debug("Failed to access prompt state");
            }
            return true;
        }

        false
    }
}

unsafe extern "system" fn prompt_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "prompt_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || prompt_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn prompt_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let create_struct =
                    lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
                let init_ptr = (*create_struct).lpCreateParams as *mut PromptWindowInit;
                if init_ptr.is_null() {
                    return LRESULT(0);
                }
                let init = Box::from_raw(init_ptr);
                let parent = init.parent;
                let language =
                    with_state(parent, |state| state.settings.language).unwrap_or_default();
                let labels = prompt_labels(language);
                let hfont = with_state(parent, |state| state.hfont).unwrap_or_default();
                let settings =
                    with_state(parent, |state| state.settings.clone()).unwrap_or_default();

                let label_input = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.input).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    16,
                    80,
                    18,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let input = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    100,
                    14,
                    580,
                    22,
                    hwnd,
                    HMENU(PROMPT_ID_INPUT as isize),
                    HINSTANCE(0),
                    None,
                );

                let label_output = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(to_wide(&labels.output).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    16,
                    50,
                    80,
                    18,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let output = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_EDIT,
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WS_VSCROLL
                        | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY) as u32),
                    16,
                    70,
                    664,
                    360,
                    hwnd,
                    HMENU(PROMPT_ID_OUTPUT as isize),
                    HINSTANCE(0),
                    None,
                );
                SendMessageW(output, EM_LIMITTEXT, WPARAM(0x7FFFFFFE), LPARAM(0));

                let checkbox_autoscroll = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.autoscroll).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    16,
                    440,
                    200,
                    20,
                    hwnd,
                    HMENU(PROMPT_ID_AUTOSCROLL as isize),
                    HINSTANCE(0),
                    None,
                );
                let checkbox_strip_ansi = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.strip_ansi).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    230,
                    440,
                    220,
                    20,
                    hwnd,
                    HMENU(PROMPT_ID_STRIP_ANSI as isize),
                    HINSTANCE(0),
                    None,
                );
                let checkbox_announce_lines = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.announce_lines).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    16,
                    464,
                    260,
                    20,
                    hwnd,
                    HMENU(PROMPT_ID_ANNOUNCE_LINES as isize),
                    HINSTANCE(0),
                    None,
                );
                let checkbox_beep_on_idle = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.beep_on_idle).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    290,
                    464,
                    240,
                    20,
                    hwnd,
                    HMENU(PROMPT_ID_BEEP_ON_IDLE as isize),
                    HINSTANCE(0),
                    None,
                );
                let checkbox_prevent_sleep = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(to_wide(&labels.prevent_sleep).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    16,
                    488,
                    320,
                    20,
                    hwnd,
                    HMENU(PROMPT_ID_PREVENT_SLEEP as isize),
                    HINSTANCE(0),
                    None,
                );

                for control in [
                    label_input,
                    input,
                    label_output,
                    output,
                    checkbox_autoscroll,
                    checkbox_strip_ansi,
                    checkbox_announce_lines,
                    checkbox_beep_on_idle,
                    checkbox_prevent_sleep,
                ] {
                    if control.0 != 0 && hfont.0 != 0 {
                        SendMessageW(control, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                }

                let auto_scroll = settings.prompt_auto_scroll;
                let strip_ansi = settings.prompt_strip_ansi;
                let announce_lines = settings.prompt_announce_lines;
                let beep_on_idle = settings.prompt_beep_on_idle;
                let prevent_sleep = settings.prompt_prevent_sleep;
                let program_lower = settings.prompt_program.to_ascii_lowercase();
                let program_is_codex =
                    program_lower.contains("codex") || program_lower.contains("claude");
                SendMessageW(
                    checkbox_autoscroll,
                    BM_SETCHECK,
                    WPARAM(if auto_scroll { 1 } else { 0 }),
                    LPARAM(0),
                );
                SendMessageW(
                    checkbox_strip_ansi,
                    BM_SETCHECK,
                    WPARAM(if strip_ansi { 1 } else { 0 }),
                    LPARAM(0),
                );
                SendMessageW(
                    checkbox_announce_lines,
                    BM_SETCHECK,
                    WPARAM(if announce_lines { 1 } else { 0 }),
                    LPARAM(0),
                );
                SendMessageW(
                    checkbox_beep_on_idle,
                    BM_SETCHECK,
                    WPARAM(if beep_on_idle { 1 } else { 0 }),
                    LPARAM(0),
                );
                SendMessageW(
                    checkbox_prevent_sleep,
                    BM_SETCHECK,
                    WPARAM(if prevent_sleep { 1 } else { 0 }),
                    LPARAM(0),
                );

                let reader_cancel = Arc::new(AtomicBool::new(false));
                let beep_state = Arc::new(PromptBeepState::new(beep_on_idle, prevent_sleep));
                let mut state = PromptState {
                    parent,
                    label_input,
                    input,
                    label_output,
                    output,

                    checkbox_autoscroll,
                    checkbox_strip_ansi,
                    checkbox_announce_lines,
                    checkbox_beep_on_idle,
                    checkbox_prevent_sleep,
                    auto_scroll,
                    strip_ansi,
                    announce_lines,
                    beep_on_idle,
                    prevent_sleep,
                    buffer: String::new(),
                    buffer_utf16_len: 0,
                    line_start_byte: 0,
                    line_start_utf16: 0,
                    line_has_content: false,
                    blank_line_streak: 0,
                    pending_ws: String::new(),
                    program_is_codex,
                    last_announced_line: String::new(),
                    beep_state: beep_state.clone(),
                    session: None,
                    reader_cancel: reader_cancel.clone(),
                    ansi_stripper: AnsiStripper::new(),
                    output_queue: VecDeque::new(),
                    output_flush_active: false,
                };

                layout_prompt(hwnd, &state);

                if let Some(spawn) = start_prompt_session(
                    hwnd,
                    &settings.prompt_program,
                    &state,
                    init.working_dir.as_deref(),
                ) {
                    state.session = Some(spawn.session);
                    start_output_reader(hwnd, spawn.output_read, reader_cancel, beep_state);

                    if let Some(cmd) = init.initial_command {
                        let newline = if state.program_is_codex { "\n" } else { "\r\n" };
                        let payload = format!("{}{}", cmd, newline);
                        if let Some(session) = state.session.as_ref() {
                            session.write_input(&payload);
                        }
                    }
                }

                SetWindowLongPtrW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                    Box::into_raw(Box::new(state)) as isize,
                );
                SetFocus(input);
                LRESULT(0)
            }
            WM_SIZE => {
                if with_prompt_state(hwnd, |state| {
                    layout_prompt(hwnd, state);
                    if let Some(session) = state.session.as_ref()
                        && let Some((cols, rows)) = output_cells(state.output)
                        && !session.resize(cols, rows)
                    {
                        crate::log_debug("Failed to resize session");
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access prompt state");
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let cmd_id = wparam.0 & 0xffff;
                match cmd_id {
                    PROMPT_ID_AUTOSCROLL => {
                        if with_prompt_state(hwnd, |state| {
                            let checked = SendMessageW(
                                state.checkbox_autoscroll,
                                BM_GETCHECK,
                                WPARAM(0),
                                LPARAM(0),
                            )
                            .0 != 0;
                            state.auto_scroll = checked;
                            update_prompt_settings(state.parent, |settings| {
                                settings.prompt_auto_scroll = checked;
                            });
                        })
                        .is_none()
                        {
                            crate::log_debug("Failed to access prompt state");
                        }
                        LRESULT(0)
                    }
                    PROMPT_ID_STRIP_ANSI => {
                        if with_prompt_state(hwnd, |state| {
                            let checked = SendMessageW(
                                state.checkbox_strip_ansi,
                                BM_GETCHECK,
                                WPARAM(0),
                                LPARAM(0),
                            )
                            .0 != 0;
                            state.strip_ansi = checked;
                            update_prompt_settings(state.parent, |settings| {
                                settings.prompt_strip_ansi = checked;
                            });
                        })
                        .is_none()
                        {
                            crate::log_debug("Failed to access prompt state");
                        }
                        LRESULT(0)
                    }
                    PROMPT_ID_ANNOUNCE_LINES => {
                        if with_prompt_state(hwnd, |state| {
                            let checked = SendMessageW(
                                state.checkbox_announce_lines,
                                BM_GETCHECK,
                                WPARAM(0),
                                LPARAM(0),
                            )
                            .0 != 0;
                            state.announce_lines = checked;
                            update_prompt_settings(state.parent, |settings| {
                                settings.prompt_announce_lines = checked;
                            });
                        })
                        .is_none()
                        {
                            crate::log_debug("Failed to access prompt state");
                        }
                        LRESULT(0)
                    }
                    PROMPT_ID_BEEP_ON_IDLE => {
                        if with_prompt_state(hwnd, |state| {
                            let checked = SendMessageW(
                                state.checkbox_beep_on_idle,
                                BM_GETCHECK,
                                WPARAM(0),
                                LPARAM(0),
                            )
                            .0 != 0;
                            state.beep_on_idle = checked;
                            state.beep_state.enabled.store(checked, Ordering::Relaxed);
                            update_prompt_settings(state.parent, |settings| {
                                settings.prompt_beep_on_idle = checked;
                            });
                        })
                        .is_none()
                        {
                            crate::log_debug("Failed to access prompt state");
                        }
                        LRESULT(0)
                    }
                    PROMPT_ID_PREVENT_SLEEP => {
                        if with_prompt_state(hwnd, |state| {
                            let checked = SendMessageW(
                                state.checkbox_prevent_sleep,
                                BM_GETCHECK,
                                WPARAM(0),
                                LPARAM(0),
                            )
                            .0 != 0;
                            state.prevent_sleep = checked;
                            state
                                .beep_state
                                .sleep_enabled
                                .store(checked, Ordering::Relaxed);
                            if !checked && state.beep_state.sleep_active.load(Ordering::Relaxed) {
                                apply_prevent_sleep(false);
                                state
                                    .beep_state
                                    .sleep_active
                                    .store(false, Ordering::Relaxed);
                            }
                            update_prompt_settings(state.parent, |settings| {
                                settings.prompt_prevent_sleep = checked;
                            });
                        })
                        .is_none()
                        {
                            crate::log_debug("Failed to access prompt state");
                        }
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_KEYDOWN => {
                if wparam.0 as u32 == VK_RETURN.0 as u32 {
                    let focus = GetFocus();
                    if with_prompt_state(hwnd, |state| {
                        if focus == state.input {
                            send_input_to_pty(state);
                        }
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access prompt state");
                    }
                    return LRESULT(0);
                }
                if wparam.0 as u32 == VK_ESCAPE.0 as u32 {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    return LRESULT(0);
                }
                let ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
                if ctrl_down && wparam.0 as u32 == 'C' as u32 {
                    if with_prompt_state(hwnd, |state| {
                        let focus = GetFocus();
                        if focus == state.output {
                            copy_output_selection(state.output);
                        } else if let Some(session) = state.session.as_ref()
                            && !session.send_ctrl_c()
                        {
                            crate::log_debug("Failed to send Ctrl+C");
                        }
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access prompt state");
                    }
                    return LRESULT(0);
                }
                if ctrl_down && wparam.0 as u32 == 'L' as u32 {
                    if with_prompt_state(hwnd, |state| {
                        if confirm_clear_output(hwnd, state.parent) {
                            clear_output(state);
                        }
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access prompt state");
                    }
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SYSKEYDOWN => {
                if wparam.0 as u32 == 'I' as u32 {
                    if with_prompt_state(hwnd, |state| {
                        SetFocus(state.input);
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access prompt state");
                    }
                    return LRESULT(0);
                }
                if wparam.0 as u32 == 'O' as u32 {
                    if with_prompt_state(hwnd, |state| {
                        SetFocus(state.output);
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access prompt state");
                    }
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SETFOCUS => {
                if with_prompt_state(hwnd, |state| {
                    if state.input.0 != 0 {
                        SetFocus(state.input);
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access prompt state");
                }
                LRESULT(0)
            }
            WM_PROMPT_OUTPUT => {
                if lparam.0 == 0 {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(lparam.0 as *mut String);
                if with_prompt_state(hwnd, |state| {
                    state.output_queue.push_back(*payload);
                    if !state.output_flush_active {
                        state.output_flush_active = true;
                        if SetTimer(hwnd, PROMPT_OUTPUT_TIMER_ID, 20, None) == 0 {
                            crate::log_debug("Failed to set PROMPT_OUTPUT_TIMER");
                        }
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access prompt state");
                }
                LRESULT(0)
            }
            WM_TIMER => {
                if wparam.0 == PROMPT_OUTPUT_TIMER_ID {
                    if with_prompt_state(hwnd, |state| {
                        let mut budget = PROMPT_OUTPUT_FLUSH_CHARS;
                        let mut merged = String::new();
                        while budget > 0 {
                            let Some(chunk) = state.output_queue.pop_front() else {
                                break;
                            };
                            if merged.is_empty() && chunk.len() > budget {
                                append_output(state, &chunk);
                                break;
                            }
                            budget = budget.saturating_sub(chunk.len());
                            merged.push_str(&chunk);
                        }
                        if !merged.is_empty() {
                            append_output(state, &merged);
                        }
                        if state.output_queue.is_empty() {
                            state.output_flush_active = false;
                            if let Err(e) = KillTimer(hwnd, PROMPT_OUTPUT_TIMER_ID) {
                                crate::log_debug(&format!(
                                    "Failed to kill PROMPT_OUTPUT_TIMER: {}",
                                    e
                                ));
                            }
                        }
                    })
                    .is_none()
                    {
                        crate::log_debug("Failed to access prompt state");
                    }
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_DESTROY => {
                if with_prompt_state(hwnd, |state| {
                    state.reader_cancel.store(true, Ordering::Relaxed);
                    state.output_queue.clear();
                    state.output_flush_active = false;
                    if let Err(e) = KillTimer(hwnd, PROMPT_OUTPUT_TIMER_ID) {
                        crate::log_debug(&format!("Failed to kill PROMPT_OUTPUT_TIMER: {}", e));
                    }
                    if state.beep_state.sleep_active.load(Ordering::Relaxed) {
                        apply_prevent_sleep(false);
                        state
                            .beep_state
                            .sleep_active
                            .store(false, Ordering::Relaxed);
                    }
                    if let Some(mut session) = state.session.take() {
                        session.close();
                    }
                })
                .is_none()
                {
                    crate::log_debug("Failed to access prompt state");
                }
                let parent = with_prompt_state(hwnd, |state| state.parent).unwrap_or(HWND(0));
                if with_state(parent, |state| {
                    state.prompt_window = HWND(0);
                })
                .is_none()
                {
                    crate::log_debug("Failed to access prompt state");
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let ptr =
                    GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA)
                        as *mut PromptState;
                if !ptr.is_null() {
                    let _unused_box = Box::from_raw(ptr);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                crate::log_if_err!(crate::destroy_window_safe(hwnd));
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn with_prompt_state<F, R>(hwnd: HWND, f: F) -> Option<R>
where
    F: FnOnce(&mut PromptState) -> R,
{
    let ptr = crate::get_window_long_ptr_w_safe(
        hwnd,
        windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
    ) as *mut PromptState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn read_clipboard_text(hwnd_owner: HWND) -> Option<String> {
    unsafe {
        const CF_UNICODETEXT: u32 = 13;
        if OpenClipboard(hwnd_owner).is_err() {
            return None;
        }
        let result = (|| {
            let handle = match GetClipboardData(CF_UNICODETEXT) {
                Ok(h) => h,
                Err(e) => {
                    crate::log_debug(&format!("GetClipboardData failed: {}", e));
                    return None;
                }
            };
            if handle.0 == 0 {
                return None;
            }
            let hglobal = HGLOBAL(handle.0 as *mut std::ffi::c_void);
            let ptr = GlobalLock(hglobal) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let size_bytes = GlobalSize(hglobal);
            if size_bytes < std::mem::size_of::<u16>() {
                if let Err(e) = GlobalUnlock(hglobal) {
                    crate::log_debug(&format!("GlobalUnlock failed: {}", e));
                }
                return None;
            }
            let max_len = size_bytes / std::mem::size_of::<u16>();
            let mut len = 0usize;
            while len < max_len && *ptr.add(len) != 0 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            if let Err(e) = GlobalUnlock(hglobal) {
                crate::log_debug(&format!("GlobalUnlock failed: {}", e));
            }
            Some(text)
        })();
        if let Err(e) = CloseClipboard() {
            crate::log_debug(&format!("CloseClipboard failed: {}", e));
        }
        result
    }
}

fn copy_output_selection(hwnd_output: HWND) {
    unsafe {
        const CF_UNICODETEXT: u32 = 13;
        let mut start: u32 = 0;
        let mut end: u32 = 0;
        SendMessageW(
            hwnd_output,
            EM_GETSEL,
            WPARAM(&mut start as *mut u32 as usize),
            LPARAM(&mut end as *mut u32 as isize),
        );
        if end <= start {
            return;
        }
        let len = GetWindowTextLengthW(hwnd_output);
        if len <= 0 {
            return;
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        let read = GetWindowTextW(hwnd_output, &mut buf) as usize;
        if read == 0 {
            return;
        }
        let start = (start as usize).min(read);
        let end = (end as usize).min(read);
        if end <= start {
            return;
        }
        let mut selection = buf[start..end].to_vec();
        selection.push(0);
        if OpenClipboard(hwnd_output).is_err() {
            return;
        }
        if let Err(e) = EmptyClipboard() {
            crate::log_debug(&format!("EmptyClipboard failed: {}", e));
        }
        let size = selection.len() * std::mem::size_of::<u16>();
        let handle = match GlobalAlloc(GMEM_MOVEABLE, size) {
            Ok(handle) => handle,
            Err(_) => {
                if let Err(e) = CloseClipboard() {
                    crate::log_debug(&format!("CloseClipboard failed: {}", e));
                }
                return;
            }
        };
        if handle.0.is_null() {
            if let Err(e) = CloseClipboard() {
                crate::log_debug(&format!("CloseClipboard failed: {}", e));
            }
            return;
        }
        let ptr = GlobalLock(handle) as *mut u16;
        if ptr.is_null() {
            if let Err(e) = CloseClipboard() {
                crate::log_debug(&format!("CloseClipboard failed: {}", e));
            }
            return;
        }
        std::ptr::copy_nonoverlapping(selection.as_ptr(), ptr, selection.len());
        if let Err(e) = GlobalUnlock(handle) {
            crate::log_debug(&format!("GlobalUnlock failed: {}", e));
        }
        if let Err(e) = SetClipboardData(CF_UNICODETEXT, HANDLE(handle.0 as isize)) {
            crate::log_debug(&format!("SetClipboardData failed: {}", e));
        }
        if let Err(e) = CloseClipboard() {
            crate::log_debug(&format!("CloseClipboard failed: {}", e));
        }
    }
}

fn start_prompt_session(
    hwnd: HWND,
    program: &str,
    state: &PromptState,
    working_dir: Option<&Path>,
) -> Option<ConPtySpawn> {
    let (cols, rows) = output_cells(state.output).unwrap_or((80, 24));
    match ConPtySession::spawn(program, cols, rows, working_dir) {
        Ok(spawn) => Some(spawn),
        Err(err) => {
            log_debug(&format!("Prompt spawn failed: {err}"));
            {
                let language =
                    with_state(state.parent, |state| state.settings.language).unwrap_or_default();
                let err_text = i18n::tr_f(language, "prompt.error", &[("err", &err.to_string())]);
                show_error(hwnd, language, &err_text);
            }
            None
        }
    }
}

fn start_output_reader(
    hwnd: HWND,
    output_read: windows::Win32::Foundation::HANDLE,
    cancel: Arc<AtomicBool>,
    beep_state: Arc<PromptBeepState>,
) {
    let beep_cancel = cancel.clone();
    let beep_state_clone = beep_state.clone();
    std::thread::spawn(move || {
        loop {
            if beep_cancel.load(Ordering::Relaxed) {
                break;
            }
            let last = beep_state_clone.last_output_ms.load(Ordering::Relaxed);
            if last != 0 {
                let now = now_ms();
                if now.saturating_sub(last) >= 1_000
                    && beep_state_clone.enabled.load(Ordering::Relaxed)
                    && !beep_state_clone.beeped.swap(true, Ordering::Relaxed)
                {
                    unsafe {
                        if let Err(e) = MessageBeep(MESSAGEBOX_STYLE(0)) {
                            crate::log_debug(&format!("MessageBeep failed: {}", e));
                        }
                    }
                }
                if now.saturating_sub(last) >= 1_000
                    && beep_state_clone.sleep_enabled.load(Ordering::Relaxed)
                    && beep_state_clone.sleep_active.load(Ordering::Relaxed)
                {
                    apply_prevent_sleep(false);
                    beep_state_clone
                        .sleep_active
                        .store(false, Ordering::Relaxed);
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    });
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        let mut total_read = 0usize;
        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let mut read = 0u32;
            log_debug("Prompt: Calling ReadFile...");
            let ok = read_file_ok(output_read, &mut buffer, &mut read);
            if !ok {
                let err = crate::get_last_error_safe();
                log_debug(&format!(
                    "Prompt: ReadFile returned false, error code: {:?}, total bytes read so far: {}",
                    err, total_read
                ));
                break;
            }
            if read == 0 {
                log_debug(&format!(
                    "Prompt: ReadFile read 0 bytes (EOF), total bytes read: {}",
                    total_read
                ));
                break;
            }
            total_read += read as usize;
            log_debug(&format!(
                "Prompt: ReadFile read {} bytes, total: {}",
                read, total_read
            ));
            beep_state.last_output_ms.store(now_ms(), Ordering::Relaxed);
            beep_state.beeped.store(false, Ordering::Relaxed);
            if beep_state.sleep_enabled.load(Ordering::Relaxed)
                && !beep_state.sleep_active.swap(true, Ordering::Relaxed)
            {
                apply_prevent_sleep(true);
            }
            let chunk = String::from_utf8_lossy(&buffer[..read as usize]).to_string();
            if chunk.trim().is_empty() {
                continue;
            }
            let payload = Box::new(chunk);
            unsafe {
                let payload_ptr = Box::into_raw(payload);
                if PostMessageW(
                    hwnd,
                    WM_PROMPT_OUTPUT,
                    WPARAM(0),
                    LPARAM(payload_ptr as isize),
                )
                .is_err()
                {
                    let _unused_box = Box::from_raw(payload_ptr);
                    break;
                }
            }
        }
        unsafe {
            if let Err(e) = windows::Win32::Foundation::CloseHandle(output_read) {
                crate::log_debug(&format!("Failed to close output_read: {}", e));
            }
        }
    });
}

fn update_prompt_settings<F>(parent: HWND, update: F)
where
    F: FnOnce(&mut crate::settings::AppSettings),
{
    let settings = {
        with_state(parent, |state| {
            update(&mut state.settings);
            state.settings.clone()
        })
    }
    .unwrap_or_default();
    save_settings(settings);
}

fn send_input_to_pty(state: &mut PromptState) {
    if state.input.0 == 0 {
        return;
    }
    let len = crate::get_window_text_length_w_safe(state.input);
    if len < 0 {
        return;
    }
    let mut buffer = vec![0u16; (len + 1) as usize];
    let read = crate::get_window_text_w_safe(state.input, &mut buffer);
    let text = String::from_utf16_lossy(&buffer[..read as usize]);
    if state.program_is_codex && is_codex_approvals_command(&text) {
        spawn_codex_approvals();
        if let Err(_e) = crate::set_window_text_w_safe(state.input, PCWSTR::null()) {
            crate::log_debug(&format!("Error: {:?}", _e));
        }
        return;
    }
    if let Some(session) = state.session.as_ref() {
        let newline = if state.program_is_codex { "\n" } else { "\r\n" };
        let payload = format!("{text}{newline}");
        if !session.write_input(&payload) {
            crate::log_debug("Failed to write input");
        }
    }
    if let Err(_e) = crate::set_window_text_w_safe(state.input, PCWSTR::null()) {
        crate::log_debug(&format!("Error: {:?}", _e));
    }
}

fn clear_output(state: &mut PromptState) {
    state.buffer.clear();
    state.buffer_utf16_len = 0;
    state.line_start_byte = 0;
    state.line_start_utf16 = 0;
    state.line_has_content = false;
    state.blank_line_streak = 0;
    state.pending_ws.clear();
    state.last_announced_line.clear();
    if let Err(_e) = crate::set_window_text_w_safe(state.output, PCWSTR::null()) {
        crate::log_debug(&format!("Error: {:?}", _e));
    }
}

fn trim_output_keep_last(state: &mut PromptState) {
    if state.buffer_utf16_len <= PROMPT_OUTPUT_KEEP {
        return;
    }
    let excess = state.buffer_utf16_len - PROMPT_OUTPUT_KEEP;
    let mut units_removed = 0usize;
    let mut cut_idx = 0usize;
    for (byte_idx, ch) in state.buffer.char_indices() {
        units_removed += ch.len_utf16();
        cut_idx = byte_idx + ch.len_utf8();
        if units_removed >= excess {
            break;
        }
    }
    if cut_idx == 0 {
        return;
    }
    state.buffer.drain(..cut_idx);
    state.buffer_utf16_len -= units_removed;
    state.line_start_byte = state.buffer.len();
    state.line_start_utf16 = state.buffer_utf16_len;
    state.line_has_content = false;
    state.blank_line_streak = 0;
    state.pending_ws.clear();
    state.last_announced_line.clear();
    let wide = to_wide(&state.buffer);
    crate::send_message_w_safe(state.output, EM_SETREADONLY, WPARAM(0), LPARAM(0));
    if let Err(_e) = crate::set_window_text_w_safe(state.output, PCWSTR(wide.as_ptr())) {
        crate::log_debug(&format!("Error: {:?}", _e));
    }
    unsafe {
        SendMessageW(state.output, EM_SETREADONLY, WPARAM(1), LPARAM(0));
        SendMessageW(
            state.output,
            EM_SETSEL,
            WPARAM(state.buffer_utf16_len),
            LPARAM(state.buffer_utf16_len as isize),
        );
        SendMessageW(state.output, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
    }
}

fn apply_prevent_sleep(enabled: bool) -> bool {
    crate::power_awake::set_required("prompt-window", enabled)
}

fn confirm_clear_output(hwnd: HWND, parent: HWND) -> bool {
    let language = { with_state(parent, |state| state.settings.language).unwrap_or_default() };
    let labels = prompt_labels(language);
    let title = to_wide(&confirm_title(language));
    let message = to_wide(&labels.clear_confirm);
    unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OKCANCEL | MB_ICONQUESTION,
        )
        .0 == 1
    }
}

fn append_output(state: &mut PromptState, text: &str) {
    let filtered = if state.strip_ansi {
        let stripped = state.ansi_stripper.process(text);
        filter_context_left_lines(&stripped)
    } else {
        text.to_string()
    };

    let filtered_units = filtered.encode_utf16().count();
    if state.buffer_utf16_len + filtered_units > PROMPT_OUTPUT_LIMIT {
        trim_output_keep_last(state);
    }

    let prev_len = state.buffer_utf16_len;
    let prev_line_start_utf16 = state.line_start_utf16;
    let prev_line_start_byte = state.line_start_byte;
    let mut had_cr = false;
    let mut delta = String::new();
    let mut newline_appended = false;
    let mut lines_to_announce: Vec<String> = Vec::new();
    let mut chars = filtered.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if matches!(chars.peek(), Some(&'\n')) {
                chars.next();
                if !state.line_has_content {
                    if state.blank_line_streak >= 1 {
                        continue;
                    }
                    state.blank_line_streak = 1;
                } else {
                    state.blank_line_streak = 0;
                }
                append_newline(
                    state,
                    &mut delta,
                    &mut newline_appended,
                    &mut lines_to_announce,
                );
                state.line_has_content = false;
                state.pending_ws.clear();
            } else {
                had_cr = true;
                state.buffer.truncate(state.line_start_byte);
                state.buffer_utf16_len = state.line_start_utf16;
                delta.clear();
                state.line_has_content = false;
                state.blank_line_streak = 0;
                state.pending_ws.clear();
            }
            continue;
        }
        if ch == '\n' {
            if !state.line_has_content {
                if state.blank_line_streak >= 1 {
                    continue;
                }
                state.blank_line_streak = 1;
            } else {
                state.blank_line_streak = 0;
            }
            append_newline(
                state,
                &mut delta,
                &mut newline_appended,
                &mut lines_to_announce,
            );
            state.line_has_content = false;
            state.pending_ws.clear();
            continue;
        }
        if matches!(ch, ' ' | '\t') && !state.line_has_content {
            state.pending_ws.push(ch);
            continue;
        }
        if !state.pending_ws.is_empty() {
            state.buffer.push_str(&state.pending_ws);
            state.buffer_utf16_len += state.pending_ws.encode_utf16().count();
            delta.push_str(&state.pending_ws);
            state.pending_ws.clear();
        }
        state.buffer.push(ch);
        state.buffer_utf16_len += ch.len_utf16();
        delta.push(ch);
        if !ch.is_whitespace() {
            state.line_has_content = true;
        }
        state.blank_line_streak = 0;
    }

    let output = state.output;
    let focus = crate::get_focus_safe();
    let mut sel_start = 0u32;
    let mut sel_end = 0u32;
    if focus == output {
        crate::send_message_w_safe(
            output,
            EM_GETSEL,
            WPARAM(&mut sel_start as *mut _ as usize),
            LPARAM(&mut sel_end as *mut _ as isize),
        );
    }
    let should_scroll = state.auto_scroll && (focus != output || sel_end as usize == prev_len);

    let replace_start = if had_cr {
        prev_line_start_utf16
    } else {
        prev_len
    };
    let replace_end = prev_len;
    let replace_text = if had_cr {
        state.buffer[prev_line_start_byte..].to_string()
    } else {
        delta.clone() // Clone for debug log
    };

    if replace_text.is_empty() {
        return;
    }
    if !replace_text.trim().is_empty() && replace_text.len() <= 200 {
        log_debug(&format!(
            "Prompt: appending output '{}'",
            replace_text.trim()
        ));
    }
    let wide = to_wide(&replace_text);
    unsafe {
        SendMessageW(
            output,
            EM_SETREADONLY,
            WPARAM(0), // False
            LPARAM(0),
        );
        SendMessageW(
            output,
            EM_SETSEL,
            WPARAM(replace_start),
            LPARAM(replace_end as isize),
        );
        SendMessageW(
            output,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(wide.as_ptr() as isize),
        );
        SendMessageW(
            output,
            EM_SETREADONLY,
            WPARAM(1), // True
            LPARAM(0),
        );
    }
    if state.announce_lines && newline_appended {
        for line in lines_to_announce {
            announce_line(&line);
            state.last_announced_line = line;
        }
    }
    if state.announce_lines {
        let current_line = state.buffer[state.line_start_byte..].to_string();
        if !current_line.is_empty()
            && current_line != state.last_announced_line
            && looks_like_prompt(&current_line)
        {
            announce_line(&current_line);
            state.last_announced_line = current_line;
        }
    }

    if should_scroll {
        unsafe {
            SendMessageW(
                output,
                EM_SETSEL,
                WPARAM(state.buffer_utf16_len),
                LPARAM(state.buffer_utf16_len as isize),
            );
            SendMessageW(output, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
        }
    } else if focus == output {
        let max = state.buffer_utf16_len as u32;
        let restore_start = sel_start.min(max);
        let restore_end = sel_end.min(max);
        crate::send_message_w_safe(
            output,
            EM_SETSEL,
            WPARAM(restore_start as usize),
            LPARAM(restore_end as isize),
        );
    }
}

fn append_newline(
    state: &mut PromptState,
    delta: &mut String,
    newline_appended: &mut bool,
    lines_to_announce: &mut Vec<String>,
) {
    let line = state.buffer[state.line_start_byte..].to_string();
    if !line.is_empty() {
        lines_to_announce.push(line);
    }
    state.buffer.push('\r');
    state.buffer.push('\n');
    state.buffer_utf16_len += 2;
    state.line_start_byte = state.buffer.len();
    state.line_start_utf16 = state.buffer_utf16_len;
    delta.push('\r');
    delta.push('\n');
    *newline_appended = true;
}

fn announce_line(line: &str) {
    if line.is_empty() {
        return;
    }
    crate::accessibility::nvda_speak(line);
}

fn looks_like_prompt(line: &str) -> bool {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return false;
    }
    let last = trimmed.chars().last().unwrap_or(' ');
    matches!(last, '>' | '$' | '#')
}

fn filter_context_left_lines(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let line_start = i;
        while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
            i += 1;
        }
        let line = &input[line_start..i];
        let mut line_end = "";
        if i < bytes.len() {
            if bytes[i] == b'\r' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    line_end = "\r\n";
                    i += 2;
                } else {
                    line_end = "\r";
                    i += 1;
                }
            } else {
                line_end = "\n";
                i += 1;
            }
        }
        let line = if is_whitespace_only_line(line) {
            ""
        } else {
            line
        };
        if !is_context_left_line(line) && !is_interrupt_hint_line(line) {
            out.push_str(line);
            out.push_str(line_end);
        }
    }
    out
}

fn is_codex_approvals_command(text: &str) -> bool {
    text.trim().eq_ignore_ascii_case("/approvals")
}

fn spawn_codex_approvals() {
    let spawn = Command::new("cmd")
        .args(["/c", "start", "", "codex", "/approvals"])
        .spawn();
    if let Err(err) = spawn {
        log_debug(&format!("Prompt approvals spawn failed: {err}"));
    }
}

fn is_whitespace_only_line(line: &str) -> bool {
    !line.is_empty() && line.chars().all(|ch| ch == ' ' || ch == '\t')
}

fn is_context_left_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("context left") && lower.contains("shortcuts") {
        return true;
    }
    let Some(before_suffix) = trimmed.strip_suffix("context left") else {
        return false;
    };
    let before = before_suffix.trim_end();
    let Some(num_part) = before.strip_suffix('%') else {
        return false;
    };
    !num_part.is_empty() && num_part.chars().all(|c| c.is_ascii_digit())
}

fn is_interrupt_hint_line(line: &str) -> bool {
    line.to_ascii_lowercase().contains("esc to interrupt")
}

fn output_cells(hwnd_output: HWND) -> Option<(i16, i16)> {
    let mut rect = windows::Win32::Foundation::RECT::default();
    unsafe {
        if GetClientRect(hwnd_output, &mut rect).is_err() {
            return None;
        }
    }
    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);
    let (char_w, char_h) = text_metrics(hwnd_output).unwrap_or((8, 16));
    let cols = (width / char_w).max(1) as i16;
    let rows = (height / char_h).max(1) as i16;
    Some((cols, rows))
}

fn text_metrics(hwnd: HWND) -> Option<(i32, i32)> {
    unsafe {
        let hdc = GetDC(hwnd);
        if hdc.0 == 0 {
            return None;
        }
        let mut tm = TEXTMETRICW::default();
        let ok = GetTextMetricsW(hdc, &mut tm).as_bool();
        ReleaseDC(hwnd, hdc);
        if ok {
            Some((tm.tmAveCharWidth.max(1), tm.tmHeight.max(1)))
        } else {
            None
        }
    }
}

fn client_size(hwnd: HWND) -> Option<(i32, i32)> {
    let mut rect = windows::Win32::Foundation::RECT::default();
    unsafe {
        if GetClientRect(hwnd, &mut rect).is_err() {
            return None;
        }
    }
    Some((rect.right - rect.left, rect.bottom - rect.top))
}

fn layout_prompt(hwnd: HWND, state: &PromptState) {
    let Some((width, height)) = client_size(hwnd) else {
        return;
    };
    let margin = 16;
    let label_width = 80;
    let input_height = 22;
    let label_height = 18;
    let checkbox_height = 20;
    let spacing = 8;

    let mut y = margin;
    unsafe {
        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
            state.label_input,
            margin,
            y,
            label_width,
            label_height,
            true,
        ));
        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
            state.input,
            margin + label_width + spacing,
            y - 2,
            (width - margin * 2 - label_width - spacing).max(120),
            input_height,
            true,
        ));
    }
    y += input_height + spacing;

    let output_height =
        (height - y - label_height - checkbox_height * 3 - spacing * 2 - margin).max(120);
    unsafe {
        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
            state.label_output,
            margin,
            y,
            label_width,
            label_height,
            true,
        ));
        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
            state.output,
            margin,
            y + label_height,
            (width - margin * 2).max(120),
            output_height,
            true,
        ));
    }
    let output_bottom = y + label_height + output_height;
    let checkbox_y = output_bottom + spacing;
    let checkbox_y2 = checkbox_y + checkbox_height + spacing;
    let checkbox_y3 = checkbox_y2 + checkbox_height + spacing;
    unsafe {
        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
            state.checkbox_autoscroll,
            margin,
            checkbox_y,
            200,
            checkbox_height,
            true,
        ));
        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
            state.checkbox_strip_ansi,
            margin + 210,
            checkbox_y,
            220,
            checkbox_height,
            true,
        ));
        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
            state.checkbox_announce_lines,
            margin,
            checkbox_y2,
            260,
            checkbox_height,
            true,
        ));
        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
            state.checkbox_beep_on_idle,
            margin + 270,
            checkbox_y2,
            240,
            checkbox_height,
            true,
        ));
        crate::log_if_err!(windows::Win32::UI::WindowsAndMessaging::MoveWindow(
            state.checkbox_prevent_sleep,
            margin,
            checkbox_y3,
            320,
            checkbox_height,
            true,
        ));
    }
}
struct PromptBeepState {
    last_output_ms: AtomicU64,
    beeped: AtomicBool,
    enabled: AtomicBool,
    sleep_enabled: AtomicBool,
    sleep_active: AtomicBool,
}

impl PromptBeepState {
    fn new(beep_enabled: bool, sleep_enabled: bool) -> Self {
        Self {
            last_output_ms: AtomicU64::new(0),
            beeped: AtomicBool::new(false),
            enabled: AtomicBool::new(beep_enabled),
            sleep_enabled: AtomicBool::new(sleep_enabled),
            sleep_active: AtomicBool::new(false),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
