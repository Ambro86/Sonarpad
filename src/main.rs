#![deny(warnings)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(let_underscore_drop)]
#![cfg_attr(not(test), windows_subsystem = "windows")]

mod accessibility;
mod audio_description;
mod com_guard;
mod curl_client;
mod embedded_deps;
mod google_tts;
mod macros;
use accessibility::*;
mod conpty;
mod diagnostics;
mod sentry_integration;
mod settings;
mod telemetry;
mod theme;
mod translator;
mod watchdog;
use editor_manager::Document;
use settings::*;
mod bookmarks;
use bookmarks::*;
mod tts_engine;
use tts_engine::*;
mod daisy_player;
mod ebook_formats;
mod epub_editor;
mod file_handler;
mod mf_encoder;
mod panic_guard;

mod sapi4_engine;
mod sapi5_engine;

use file_handler::*;
mod menu;
use menu::*;
mod search;
use search::*;
mod audio_player;
use audio_player::*;
mod bass_ffmpeg_stream;
mod bass_output;
mod bass_sys;
mod editor_manager;
mod ffmpeg_dyn;
mod ffmpeg_export;
mod ffmpeg_source;
mod stream_recording;
mod subtitle_wasapi;
mod subtitles;
use editor_manager::*;
mod app_windows;
mod audio_monitor;
mod audio_utils;
mod dialogue_voice;
mod i18n;
mod podcast;
mod podcast_recorder;
mod spellcheck;
mod text_ops;
mod tools;
mod treccani;
mod updater;
mod wikipedia;
mod wiktionary;
mod win_ocr;

use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::io::{Read, Write};
use std::mem::size_of;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::Local;
use serde::{Deserialize, Serialize};

use windows::Win32::Foundation::{
    BOOL, ERROR_INVALID_PARAMETER, ERROR_INVALID_WINDOW_HANDLE, ERROR_MENU_ITEM_NOT_FOUND,
    GetLastError, GlobalFree, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SetLastError,
    WIN32_ERROR, WPARAM,
};
use windows::Win32::Globalization::GetUserDefaultLocaleName;
use windows::Win32::Graphics::Gdi::{
    COLOR_WINDOW, DEFAULT_GUI_FONT, DeleteDC, DeleteObject, GET_STOCK_OBJECT_FLAGS, GetDeviceCaps,
    GetObjectW, GetStockObject, HBRUSH, HDC, HFONT, HGDIOBJ, HORZRES, InvalidateRect, LOGFONTW,
    LOGPIXELSX, LOGPIXELSY, PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY, PHYSICALWIDTH,
    ScreenToClient,
};
use windows::Win32::Graphics::Printing::GetDefaultPrinterW;
use windows::Win32::Storage::Xps::{DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
use windows::Win32::System::DataExchange::{
    COPYDATASTRUCT, CloseClipboard, EmptyClipboard, IsClipboardFormatAvailable, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
use windows::Win32::System::Memory::{GLOBAL_ALLOC_FLAGS, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId, WaitForSingleObject,
};
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::Controls::Dialogs::{
    FINDREPLACE_FLAGS, FINDREPLACEW, GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER,
    OFN_FILEMUSTEXIST, OFN_HIDEREADONLY, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    PD_NOPAGENUMS, PD_NOSELECTION, PD_RETURNDC, PD_USEDEVMODECOPIESANDCOLLATE, PRINTDLGW,
    PrintDlgW,
};
use windows::Win32::UI::Controls::RichEdit::{
    CFE_AUTOBACKCOLOR, CFM_BACKCOLOR, CHARFORMAT2W, CHARRANGE, EM_EXGETSEL, EM_EXSETSEL,
    EM_GETTEXTRANGE, EM_SETCHARFORMAT, EM_SETEVENTMASK, EN_SELCHANGE, ENM_CHANGE, ENM_SELCHANGE,
    SCF_SELECTION, TEXTRANGEW,
};
use windows::Win32::UI::Controls::{
    BST_CHECKED, EM_GETMODIFY, EM_SETMODIFY, ICC_BAR_CLASSES, ICC_LISTVIEW_CLASSES,
    ICC_TAB_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx, NMHDR, SB_SETTEXTW,
    STATUSCLASSNAMEW, TCM_ADJUSTRECT, TCM_GETCURSEL, TCN_SELCHANGE, WC_BUTTON, WC_COMBOBOXW,
    WC_STATIC, WC_TABCONTROLW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, IsWindowEnabled, SetActiveWindow, SetFocus, VK_APPS,
    VK_CONTROL, VK_ESCAPE, VK_F1, VK_F2, VK_F3, VK_F4, VK_F7, VK_F8, VK_F9, VK_F10,
    VK_MEDIA_PLAY_PAUSE, VK_MENU, VK_NEXT, VK_OEM_COMMA, VK_OEM_PERIOD, VK_PRIOR, VK_RETURN,
    VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    DragAcceptFiles, DragFinish, DragQueryFileW, FileSaveDialog, HDROP, IFileDialog,
    IFileDialogControlEvents, IFileDialogControlEvents_Impl, IFileDialogCustomize,
    IFileDialogEvents, IFileDialogEvents_Impl, IFileSaveDialog, IShellItem,
    SHCreateItemFromParsingName, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ACCEL, AllowSetForegroundWindow, AppendMenuW, BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX,
    CB_ADDSTRING, CB_GETCOUNT, CB_GETCURSEL, CB_GETDROPPEDSTATE, CB_GETITEMDATA, CB_RESETCONTENT,
    CB_SETCURSEL, CB_SETITEMDATA, CBN_SELCHANGE, CBS_DROPDOWNLIST, CHILDID_SELF, CREATESTRUCTW,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CallWindowProcW, CheckMenuItem, CreateAcceleratorTableW,
    CreatePopupMenu, CreateWindowExW, DefWindowProcW, DeleteMenu, DestroyWindow, DispatchMessageW,
    DrawMenuBar, EN_CHANGE, EN_KILLFOCUS, ES_AUTOHSCROLL, EVENT_OBJECT_FOCUS, EnableMenuItem,
    EnumWindows, FALT, FCONTROL, FSHIFT, FVIRTKEY, FindWindowW, GWLP_USERDATA, GWLP_WNDPROC,
    GetClassNameW, GetCursorPos, GetDlgCtrlID, GetDlgItem, GetForegroundWindow, GetLastActivePopup,
    GetMenu, GetMenuItemCount, GetMessageW, GetNextDlgTabItem, GetParent, GetSubMenu,
    GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, HACCEL,
    HCURSOR, HICON, HMENU, HWND_NOTOPMOST, HWND_TOPMOST, IDC_ARROW, IDI_APPLICATION, IDYES,
    InsertMenuW, IsChild, IsDialogMessageW, IsIconic, IsWindow, IsWindowVisible, KillTimer,
    LoadCursorW, LoadIconW, MB_ICONASTERISK, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONWARNING,
    MB_OK, MB_YESNO, MENU_ITEM_FLAGS, MESSAGEBOX_RESULT, MESSAGEBOX_STYLE, MF_BYCOMMAND,
    MF_BYPOSITION, MF_CHECKED, MF_ENABLED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING,
    MF_UNCHECKED, MSG, MessageBoxW, ModifyMenuW, MoveWindow, OBJID_CLIENT, PostMessageW,
    PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SC_KEYMENU, SW_HIDE, SW_RESTORE,
    SW_SHOW, SW_SHOWMAXIMIZED, SW_SHOWNORMAL, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SendMessageW,
    SetForegroundWindow, SetMenu, SetTimer, SetWindowLongPtrW, SetWindowPos, SetWindowTextW,
    ShowWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateAcceleratorW,
    TranslateMessage, WINDOW_STYLE, WM_ACTIVATE, WM_APP, WM_APPCOMMAND, WM_CANCELMODE, WM_CHAR,
    WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_COPY, WM_COPYDATA, WM_CREATE, WM_CUT, WM_DESTROY,
    WM_DROPFILES, WM_GETFONT, WM_GETTEXTLENGTH, WM_INITMENUPOPUP, WM_KEYDOWN, WM_KEYUP,
    WM_MOUSEMOVE, WM_NCDESTROY, WM_NEXTDLGCTL, WM_NOTIFY, WM_NULL, WM_PASTE, WM_SETFOCUS,
    WM_SETFONT, WM_SETREDRAW, WM_SIZE, WM_SYSCHAR, WM_SYSCOMMAND, WM_SYSKEYDOWN, WM_SYSKEYUP,
    WM_TIMER, WNDCLASSW, WNDPROC, WS_CHILD, WS_CLIPCHILDREN, WS_EX_CLIENTEDGE, WS_OVERLAPPEDWINDOW,
    WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{Interface, PCWSTR, PWSTR, implement, w};

const EVENT_OBJECT_LOCATIONCHANGE_ID: u32 = 0x800B;
const EVENT_OBJECT_TEXTSELECTIONCHANGED_ID: u32 = 0x8014;
const OBJID_CARET_ID: i32 = -8;
const EM_SCROLLCARET: u32 = 0x00B7;
const EM_CHARFROMPOS: u32 = 0x00D7;
const EM_LINEFROMCHAR: u32 = 0x00C9;
const EM_LINEINDEX: u32 = 0x00BB;
const EM_LINELENGTH: u32 = 0x00C1;
const EM_SETSEL: u32 = 0x00B1;
const EM_REDO: u32 = 0x0454;
const EM_CANUNDO: u32 = 0x00C6;
const EM_FORMATRANGE: u32 = 0x0439;
const WM_UNDO_MSG: u32 = 0x0304;
const TWIPS_PER_INCH: i32 = 1440;
const PRINT_MARGIN_TWIPS: i32 = 720;
const APPCOMMAND_MEDIA_PLAY_PAUSE: usize = 14;

use crate::app_windows::find_in_files_window::{
    FindInFilesCache, apply_find_in_files_selection, focus_find_in_files_results,
};
use crate::podcast::chapters::Chapter;
use crate::tools::faster_whisper_bridge::BridgeModel;

pub(crate) fn app_release_tag() -> &'static str {
    option_env!("SONARPAD_RELEASE_TAG").unwrap_or(concat!("v", env!("CARGO_PKG_VERSION")))
}

pub(crate) fn app_display_version() -> &'static str {
    app_release_tag()
}

fn update_epub_index_menu_item(hwnd: HWND, view_menu: HMENU) {
    delete_menu_best_effort(
        view_menu,
        IDM_VIEW_SHOW_EPUB_INDEX as u32,
        MF_BYCOMMAND,
        "Delete EPUB index menu item",
    );
    if !editor_manager::current_document_has_epub_index(hwnd) {
        return;
    }

    let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
    let label = i18n::tr(language, "view.show_epub_index");
    unsafe {
        crate::log_if_err!(InsertMenuW(
            view_menu,
            2,
            MF_BYPOSITION | MF_STRING | MF_ENABLED,
            IDM_VIEW_SHOW_EPUB_INDEX,
            PCWSTR(to_wide(&label).as_ptr()),
        ));
    }
}

fn update_route_image_menu_item(hwnd: HWND, file_menu: HMENU) {
    delete_menu_best_effort(
        file_menu,
        IDM_FILE_SAVE_IMAGE as u32,
        MF_BYCOMMAND,
        "Delete route image menu item",
    );
    unsafe {
        if !editor_manager::current_document_has_route_map(hwnd) {
            return;
        }

        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
        let label = i18n::tr(language, "file.save_image");
        let item_count = GetMenuItemCount(file_menu);
        let insert_pos = if item_count > 6 {
            6
        } else {
            item_count.max(0) as u32
        };
        crate::log_if_err!(InsertMenuW(
            file_menu,
            insert_pos,
            MF_BYPOSITION | MF_STRING | MF_ENABLED,
            IDM_FILE_SAVE_IMAGE,
            PCWSTR(to_wide(&label).as_ptr()),
        ));
    }
}

const WM_PDF_LOADED: u32 = WM_APP + 1;
const WM_TTS_VOICES_LOADED: u32 = WM_APP + 2;
pub(crate) const WM_DOCUMENT_LOADED: u32 = WM_APP + 9;
const WM_TTS_AUDIOBOOK_DONE: u32 = WM_APP + 4;
const WM_TTS_PLAYBACK_ERROR: u32 = WM_APP + 5;
const WM_UPDATE_PROGRESS: u32 = WM_APP + 6;
const WM_TTS_CHUNK_START: u32 = WM_APP + 7;
const WM_TTS_SAPI_VOICES_LOADED: u32 = WM_APP + 8;
const WM_TTS_START: u32 = WM_APP + 10;

pub const WM_FOCUS_EDITOR: u32 = WM_APP + 30;
pub const WM_UPDATE_DIALOG: u32 = WM_APP + 80;
pub const WM_UPDATE_PROGRESS_OPEN: u32 = WM_APP + 84;
pub const WM_UPDATE_PROGRESS_SET: u32 = WM_APP + 85;
pub const WM_UPDATE_PROGRESS_CLOSE: u32 = WM_APP + 86;
const WM_AUTO_UPDATE_CHECK: u32 = WM_APP + 81;
const WM_CHECK_PENDING_UPDATE: u32 = WM_APP + 82;
const WM_SHOW_CHANGELOG: u32 = WM_APP + 83;
const WM_SHOW_UPDATE_CHANGELOG: u32 = WM_APP + 87;
const WM_PODCAST_CHAPTERS_READY: u32 = WM_APP + 31;
const WM_DICTIONARY_LOADED: u32 = WM_APP + 32;
const WM_PODCAST_EPISODE_SAVE_RESULT: u32 = WM_APP + 33;
const WM_WHISPER_TRANSCRIPTION_DONE: u32 = WM_APP + 34;
const WM_WHISPER_TRANSCRIPTION_PROGRESS: u32 = WM_APP + 35;
const WM_WHISPER_TRANSCRIPTION_STATUS_TEXT: u32 = WM_APP + 39;
const WM_DICTATION_DONE: u32 = WM_APP + 36;
const WM_PODCAST_EPISODE_PLAY_READY: u32 = WM_APP + 37;
const WM_PODCAST_EPISODE_PLAY_FAILED: u32 = WM_APP + 38;
const WM_LOCAL_MPV_VIDEO_MODE: u32 = WM_APP + 40;
const WM_LOCAL_MPV_MENU_VISIBLE: u32 = WM_APP + 41;
const WM_EDITOR_TRANSLATION_DONE: u32 = WM_APP + 42;
const WM_EDITOR_SUMMARY_DONE: u32 = WM_APP + 43;
const WM_REACTIVATE_MODAL_AFTER_EXTERNAL_OPEN: u32 = WM_APP + 44;
const WM_YOUTUBE_MPV_PLAYBACK_FAILED: u32 = WM_APP + 45;
const WM_OPEN_AUDIO_DESCRIPTION_FOR_TV_RECORDING: u32 = WM_APP + 46;
const WM_START_CONTEXT_WHISPER_TRANSCRIPTION: u32 = WM_APP + 47;
const WM_START_YOUTUBE_CONTEXT_AUDIO_DESCRIPTION: u32 = WM_APP + 48;
const WM_START_RAIPLAY_CONTEXT_AUDIO_DESCRIPTION: u32 = WM_APP + 49;
const WM_START_LA7_CONTEXT_AUDIO_DESCRIPTION: u32 = WM_APP + 50;
const FOCUS_EDITOR_TIMER_ID: usize = 1;
const FOCUS_EDITOR_TIMER_ID2: usize = 2;
const FOCUS_EDITOR_TIMER_ID3: usize = 3;
const FOCUS_EDITOR_TIMER_ID4: usize = 4;
const YOUTUBE_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE_TIMER_ID: usize = 55;
const RAIPLAY_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE_TIMER_ID: usize = 56;
const LA7_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE_TIMER_ID: usize = 57;
const MPV_BASS_FOCUS_DEBUG_TIMER_ID1: usize = 8;
const MPV_BASS_FOCUS_DEBUG_TIMER_ID2: usize = 9;
const MPV_BASS_FOCUS_DEBUG_TIMER_ID3: usize = 10;
const MPV_BASS_FOCUS_DEBUG_TIMER_ID4: usize = 11;
const ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID1: usize = 12;
const ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID2: usize = 13;
const ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID3: usize = 14;

struct EditorTranslateLanguage {
    key: &'static str,
    target: &'static str,
}

const EDITOR_TRANSLATE_LANGUAGES: [EditorTranslateLanguage; menu::IDM_EDIT_TRANSLATE_TARGET_MAX] = [
    EditorTranslateLanguage {
        key: "it",
        target: "it",
    },
    EditorTranslateLanguage {
        key: "en",
        target: "en",
    },
    EditorTranslateLanguage {
        key: "es",
        target: "es",
    },
    EditorTranslateLanguage {
        key: "fr",
        target: "fr",
    },
    EditorTranslateLanguage {
        key: "pt",
        target: "pt",
    },
    EditorTranslateLanguage {
        key: "pt_br",
        target: "pt-BR",
    },
    EditorTranslateLanguage {
        key: "de",
        target: "de",
    },
    EditorTranslateLanguage {
        key: "cs",
        target: "cs",
    },
    EditorTranslateLanguage {
        key: "pl",
        target: "pl",
    },
    EditorTranslateLanguage {
        key: "ru",
        target: "ru",
    },
    EditorTranslateLanguage {
        key: "sv",
        target: "sv",
    },
    EditorTranslateLanguage {
        key: "uk",
        target: "uk",
    },
    EditorTranslateLanguage {
        key: "lt",
        target: "lt",
    },
    EditorTranslateLanguage {
        key: "zh",
        target: "zh",
    },
];

struct EditorTranslationResult {
    language: Language,
    result: Result<EditorTranslationSuccess, String>,
}

struct EditorTranslationSuccess {
    text: String,
    provider: &'static str,
    fallback_from: Option<&'static str>,
}

struct EditorSummaryResult {
    language: Language,
    result: Result<EditorSummarySuccess, String>,
}

struct EditorSummarySuccess {
    text: String,
    warning: Option<String>,
}
const ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID4: usize = 15;
const MPV_ESC_FOCUS_DEBUG_TIMER_ID1: usize = 16;
const MPV_ESC_FOCUS_DEBUG_TIMER_ID2: usize = 17;
const MPV_ESC_FOCUS_DEBUG_TIMER_ID3: usize = 18;
const MPV_ESC_FOCUS_DEBUG_TIMER_ID4: usize = 19;
const MPV_ESC_FOCUS_DEBUG_TIMER_ID5: usize = 20;
const MPV_ESC_FOCUS_DEBUG_TIMER_ID6: usize = 21;
const EDITOR_TRANSLATION_SPEAK_TIMER_ID: usize = 22;
const EPUB_INDEX_ANNOUNCE_TIMER_ID: usize = 23;
const DEFERRED_MODAL_COPYDATA_OPEN_TIMER_ID: usize = 0xD0F1;
const DEFERRED_MODAL_COPYDATA_OPEN_TIMER_INTERVAL_MS: u32 = 150;
const STARTUP_UPDATE_CHANGELOG_TIMER_ID: usize = 0xD0F2;
const STARTUP_UPDATE_CHANGELOG_INITIAL_DELAY_MS: u32 = 1000;
const STARTUP_UPDATE_CHANGELOG_RETRY_INTERVAL_MS: u32 = 250;
const GEMINI_TRANSLATION_FALLBACK_MODEL: &str = "gemini-2.5-flash";

fn gemini_model_uses_compat_fallback(model: &str) -> bool {
    let model = model.trim();
    model.starts_with("gemini-3")
        || model == crate::settings::DEFAULT_GEMINI_MODEL
        || model == crate::settings::GEMINI_FLASH_LITE_LATEST_MODEL
}

const CHAPTER_ANNOUNCE_TIMER_ID: usize = 5;
const SPELLCHECK_HIGHLIGHT_TIMER_ID: usize = 6;
const AUDIO_PLAYLIST_TIMER_ID: usize = 7;
const SPELLCHECK_HIGHLIGHT_DEBOUNCE_MS: u32 = 100;
const PDF_OCR_PROMPT_TIMEOUT_COPYDATA_SECS: u64 = 30;
const COPYDATA_OPEN_FILE: usize = 1;
const COPYDATA_CALENDAR_REMINDER: usize = 2;
const COPYDATA_RESULT_HANDLED: isize = 1;
const COPYDATA_RESULT_DEFERRED_MODAL: isize = 2;
const VOICE_PANEL_ID_ENGINE: usize = 21001;
const VOICE_PANEL_ID_LANGUAGE: usize = 21012;
const VOICE_PANEL_ID_VOICE: usize = 21002;
const VOICE_PANEL_ID_MULTILINGUAL: usize = 21003;
const VOICE_PANEL_ID_FAVORITES: usize = 21004;
const VOICE_PANEL_ID_SPEED: usize = 21005;
const VOICE_PANEL_ID_PITCH: usize = 21006;
const VOICE_PANEL_ID_VOLUME: usize = 21007;
const VOICE_PANEL_ID_SPEED_EDIT: usize = 21008;
const VOICE_PANEL_ID_PITCH_EDIT: usize = 21009;
const VOICE_PANEL_ID_VOLUME_EDIT: usize = 21010;
const VOICE_PANEL_ID_INSERT_TAG: usize = 21011;
const VOICE_PANEL_ID_INSERT_PAUSE: usize = 21015;
const VOICE_PANEL_ID_MANAGE_GOOGLE_VOICES: usize = 21016;
const PAUSE_TAG_MENU_250MS: usize = 21101;
const PAUSE_TAG_MENU_500MS: usize = 21102;
const PAUSE_TAG_MENU_1S: usize = 21103;
const PAUSE_TAG_MENU_2S: usize = 21104;
const PAUSE_TAG_MENU_CUSTOM: usize = 21105;
const MAIN_STATUS_ID: usize = 22001;
const VOICE_MENU_ID_ADD_FAVORITE: u32 = 9001;
const VOICE_MENU_ID_REMOVE_FAVORITE: u32 = 9002;
const WINDOW_MENU_INDEX: i32 = 6;
const WINDOW_DOC_MENU_BASE: usize = 11_000;
const WINDOW_DOC_MENU_MAX: usize = 200;
const WINDOW_DOC_MENU_SEPARATOR_ID: usize = 10_999;
const CREATE_NO_WINDOW_FLAGS: u32 = 0x0800_0000;

pub(crate) fn bring_window_to_foreground(hwnd: HWND) {
    unsafe {
        let foreground = GetForegroundWindow();
        let current_thread = GetCurrentThreadId();
        let mut attached_thread = None;
        log_debug(&format!(
            "bring_window_to_foreground: target={:?} initial_foreground={:?}",
            hwnd, foreground
        ));
        if foreground.0 != 0 {
            let foreground_thread = GetWindowThreadProcessId(foreground, None);
            if foreground_thread != 0 && foreground_thread != current_thread {
                if AttachThreadInput(foreground_thread, current_thread, true).as_bool() {
                    attached_thread = Some(foreground_thread);
                } else {
                    log_debug("AttachThreadInput (attach) failed");
                }
            }
        }

        if IsIconic(hwnd).as_bool() {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            ShowWindow(hwnd, SW_SHOW);
        }
        if !SetForegroundWindow(hwnd).as_bool() {
            log_debug("SetForegroundWindow failed");
        }
        if GetForegroundWindow() != hwnd {
            log_debug(&format!(
                "bring_window_to_foreground: foreground after first attempt is {:?}, applying SetWindowPos fallback",
                GetForegroundWindow()
            ));
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW;
            if let Err(err) = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, flags) {
                log_debug(&format!("SetWindowPos(HWND_TOPMOST) failed: {}", err));
            }
            if let Err(err) = SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, flags) {
                log_debug(&format!("SetWindowPos(HWND_NOTOPMOST) failed: {}", err));
            }
            if !SetForegroundWindow(hwnd).as_bool() {
                log_debug("SetForegroundWindow retry failed");
            }
        }
        SetActiveWindow(hwnd);
        notify_active_editor_focus(hwnd, false);
        log_foreground_snapshot("bring_window_to_foreground.final");

        if let Some(foreground_thread) = attached_thread
            && !AttachThreadInput(foreground_thread, current_thread, false).as_bool()
        {
            log_debug("AttachThreadInput (detach) failed");
        }
    }
}

pub(crate) fn bring_modal_window_to_foreground(hwnd: HWND) {
    unsafe {
        if !is_window_handle_valid(hwnd) {
            log_debug(&format!(
                "bring_modal_window_to_foreground: invalid target={hwnd:?}"
            ));
            return;
        }

        let foreground = GetForegroundWindow();
        let current_thread = GetCurrentThreadId();
        let mut attached_thread = None;
        let previous_focus = GetFocus();
        let focus_to_restore = if previous_focus.0 != 0
            && is_window_handle_valid(previous_focus)
            && (previous_focus == hwnd || IsChild(hwnd, previous_focus).as_bool())
        {
            previous_focus
        } else {
            HWND(0)
        };

        log_debug(&format!(
            "bring_modal_window_to_foreground: target={:?} initial_foreground={:?} focus_to_restore={:?}",
            hwnd, foreground, focus_to_restore
        ));

        if foreground.0 != 0 {
            let foreground_thread = GetWindowThreadProcessId(foreground, None);
            if foreground_thread != 0 && foreground_thread != current_thread {
                if AttachThreadInput(foreground_thread, current_thread, true).as_bool() {
                    attached_thread = Some(foreground_thread);
                } else {
                    log_debug("Modal AttachThreadInput (attach) failed");
                }
            }
        }

        if IsIconic(hwnd).as_bool() {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            ShowWindow(hwnd, SW_SHOW);
        }
        if !SetForegroundWindow(hwnd).as_bool() {
            log_debug("Modal SetForegroundWindow failed");
        }
        if GetForegroundWindow() != hwnd {
            log_debug(&format!(
                "bring_modal_window_to_foreground: foreground after first attempt is {:?}, applying SetWindowPos fallback",
                GetForegroundWindow()
            ));
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW;
            if let Err(err) = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, flags) {
                log_debug(&format!("Modal SetWindowPos(HWND_TOPMOST) failed: {}", err));
            }
            if let Err(err) = SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, flags) {
                log_debug(&format!(
                    "Modal SetWindowPos(HWND_NOTOPMOST) failed: {}",
                    err
                ));
            }
            if !SetForegroundWindow(hwnd).as_bool() {
                log_debug("Modal SetForegroundWindow retry failed");
            }
        }
        SetActiveWindow(hwnd);
        if focus_to_restore.0 != 0 && is_window_handle_valid(focus_to_restore) {
            SetFocus(focus_to_restore);
        }
        log_foreground_snapshot("bring_modal_window_to_foreground.final");

        if let Some(foreground_thread) = attached_thread
            && !AttachThreadInput(foreground_thread, current_thread, false).as_bool()
        {
            log_debug("Modal AttachThreadInput (detach) failed");
        }
    }
}

fn notify_active_editor_focus(hwnd: HWND, notify_when_audiobook: bool) {
    let is_audiobook = {
        with_state(hwnd, |state| {
            state
                .docs
                .get(state.current)
                .map(|doc| matches!(doc.format, FileFormat::Audiobook))
                .unwrap_or(false)
        })
    }
    .unwrap_or(false);

    if is_audiobook {
        if !notify_when_audiobook {
            return;
        }
        let tab_hwnd = { with_state(hwnd, |state| state.hwnd_tab) }.unwrap_or(HWND(0));
        if tab_hwnd.0 != 0 {
            // SAFETY: `tab_hwnd` is owned by this process and used only for an accessibility focus event.
            unsafe {
                NotifyWinEvent(
                    EVENT_OBJECT_FOCUS,
                    tab_hwnd,
                    OBJID_CLIENT.0,
                    CHILDID_SELF as i32,
                );
            }
        }
        return;
    }

    if let Some(hwnd_edit) = get_active_edit(hwnd) {
        // SAFETY: `hwnd_edit` is the current editor handle managed by app state.
        unsafe {
            NotifyWinEvent(
                EVENT_OBJECT_FOCUS,
                hwnd_edit,
                OBJID_CLIENT.0,
                CHILDID_SELF as i32,
            );
            notify_editor_caret_changed(hwnd_edit);
        }
    }
}

fn reactivate_bdciechi_window(hwnd: HWND) -> bool {
    let bdc_window = with_state(hwnd, |state| state.bdciechi_window).unwrap_or(HWND(0));
    if bdc_window.0 == 0 || !is_window_handle_valid(bdc_window) {
        return false;
    }
    show_window_safe(bdc_window, SW_SHOW);
    set_foreground_window_safe(bdc_window);
    send_message_w_safe(bdc_window, WM_SETFOCUS, WPARAM(0), LPARAM(0));
    true
}

fn reactivate_batch_audiobooks_window(hwnd: HWND) -> bool {
    let batch_window = with_state(hwnd, |state| state.batch_audiobooks_window).unwrap_or(HWND(0));
    if batch_window.0 == 0 || !is_window_handle_valid(batch_window) {
        return false;
    }
    show_window_safe(batch_window, SW_SHOW);
    set_foreground_window_safe(batch_window);
    app_windows::batch_audiobooks_window::restore_batch_focus(batch_window)
}

pub(crate) fn notify_editor_caret_changed(hwnd_edit: HWND) {
    unsafe {
        if hwnd_edit.0 == 0 || GetFocus() != hwnd_edit {
            return;
        }
        NotifyWinEvent(
            EVENT_OBJECT_LOCATIONCHANGE_ID,
            hwnd_edit,
            OBJID_CARET_ID,
            CHILDID_SELF as i32,
        );
        NotifyWinEvent(
            EVENT_OBJECT_TEXTSELECTIONCHANGED_ID,
            hwnd_edit,
            OBJID_CLIENT.0,
            CHILDID_SELF as i32,
        );
    }
}

pub(crate) fn focus_editor(hwnd: HWND) {
    if has_secondary_window_open(hwnd) {
        return;
    }
    bring_window_to_foreground(hwnd);
    unsafe {
        let result = with_state(hwnd, |state| {
            state
                .docs
                .get(state.current)
                .map(|doc| (doc.hwnd_edit, matches!(doc.format, FileFormat::Audiobook)))
        })
        .flatten();

        if let Some((hwnd_edit, is_audiobook)) = result {
            if is_audiobook {
                let tab_hwnd = with_state(hwnd, |state| state.hwnd_tab).unwrap_or(HWND(0));
                if tab_hwnd.0 != 0 {
                    set_focus_safe(tab_hwnd);
                }
                return;
            }
            set_focus_safe(hwnd_edit);
            SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
            SendMessageW(hwnd_edit, WM_SETFOCUS, WPARAM(0), LPARAM(0));
            crate::log_if_err!(PostMessageW(
                hwnd,
                WM_NEXTDLGCTL,
                WPARAM(hwnd_edit.0 as usize),
                LPARAM(1)
            ));
            NotifyWinEvent(
                EVENT_OBJECT_FOCUS,
                hwnd_edit,
                OBJID_CLIENT.0,
                CHILDID_SELF as i32,
            );
            notify_editor_caret_changed(hwnd_edit);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockingModalKind {
    // Add new blocking completion dialogs here when they must:
    // 1. block editor/file-open focus handoff,
    // 2. be reactivated if hidden behind the main window,
    // 3. optionally defer WM_COPYDATA file opens until the user closes the modal.
    AudiobookDone,
    UpdateDialog,
    InfoDialog,
    RaiLuceMissingKey,
    DocumentSaveConfirm,
}

#[derive(Default)]
struct BlockingModalState {
    active: Option<BlockingModalKind>,
    deferred_copydata_open_paths: Vec<PathBuf>,
}

fn has_pending_blocking_modal(hwnd: HWND) -> bool {
    with_state(hwnd, |state| state.blocking_modal.active.is_some()).unwrap_or(false)
}

fn reactivate_pending_blocking_modal(hwnd: HWND) -> bool {
    if !has_pending_blocking_modal(hwnd) {
        return false;
    }
    let popup = unsafe { GetLastActivePopup(hwnd) };
    if popup.0 != 0 && popup != hwnd && is_window_handle_valid(popup) {
        bring_modal_window_to_foreground(popup);
    } else {
        bring_window_to_foreground(hwnd);
    }
    true
}

fn defer_copydata_paths_for_pending_blocking_modal(hwnd: HWND, paths: &[PathBuf]) -> bool {
    if !has_pending_blocking_modal(hwnd) {
        return false;
    }
    with_state(hwnd, |state| {
        state
            .blocking_modal
            .deferred_copydata_open_paths
            .extend(paths.iter().cloned());
    });
    crate::log_if_err!(unsafe {
        PostMessageW(
            hwnd,
            WM_REACTIVATE_MODAL_AFTER_EXTERNAL_OPEN,
            WPARAM(0),
            LPARAM(0),
        )
    });
    true
}

fn should_defer_external_file_open(main_window_enabled: bool) -> bool {
    !main_window_enabled
}

fn should_focus_existing_window_after_copydata(result: isize) -> bool {
    result != COPYDATA_RESULT_DEFERRED_MODAL
}

fn tracked_progress_modal_while_main_disabled(hwnd: HWND) -> HWND {
    with_state(hwnd, |state| {
        [
            state.podcast_save_window,
            state.update_progress_window,
            state.transcription_progress_window,
            state.replace_progress_window,
            state.editor_translation_progress_window,
        ]
        .into_iter()
        .find(|candidate| {
            candidate.0 != 0
                && is_window_handle_valid(*candidate)
                && unsafe { IsWindowVisible(*candidate).as_bool() }
        })
        .unwrap_or(HWND(0))
    })
    .unwrap_or(HWND(0))
}

pub(crate) fn media_action_progress_is_active(hwnd: HWND) -> bool {
    tracked_progress_modal_while_main_disabled(hwnd).0 != 0
}

fn reactivate_modal_child_while_main_disabled(hwnd: HWND) -> bool {
    if unsafe { IsWindowEnabled(hwnd).as_bool() } {
        return false;
    }

    // A context-list popup can remain the main window's last-active popup after it
    // launches an asynchronous save. Prefer the progress dialog tracked by AppState
    // so the list cannot immediately steal foreground/focus back from the download.
    let tracked = tracked_progress_modal_while_main_disabled(hwnd);
    if tracked.0 != 0 {
        let nested = unsafe { GetLastActivePopup(tracked) };
        let target = if nested.0 != 0 && nested != tracked && is_window_handle_valid(nested) {
            nested
        } else {
            tracked
        };
        log_debug(&format!(
            "Main window disabled: restoring tracked progress modal target={target:?} tracked={tracked:?}"
        ));
        bring_modal_window_to_foreground(target);
        return true;
    }

    let popup = unsafe { GetLastActivePopup(hwnd) };
    if popup.0 != 0 && popup != hwnd && is_window_handle_valid(popup) {
        log_debug(&format!(
            "Reactivating modal popup while main window is disabled: popup={popup:?}"
        ));
        bring_modal_window_to_foreground(popup);
        return true;
    }

    log_debug("Main window is disabled, but no valid modal popup was found");
    false
}

pub(crate) fn recover_main_window_after_audio_description(hwnd: HWND, stage: &str) {
    if hwnd.0 == 0 || !is_window_handle_valid(hwnd) {
        log_debug(&format!(
            "Audio description: cannot recover main window at {stage}; invalid parent={hwnd:?}"
        ));
        return;
    }
    sync_voice_panel_visibility_for_current_document(hwnd);
    if unsafe { IsWindowEnabled(hwnd).as_bool() } {
        return;
    }

    let popup = unsafe { GetLastActivePopup(hwnd) };
    if popup.0 != 0 && popup != hwnd && is_window_handle_valid(popup) {
        log_debug(&format!(
            "Audio description: main still disabled at {stage}, preserving valid modal popup={popup:?}"
        ));
        bring_modal_window_to_foreground(popup);
        return;
    }

    let tracked_progress = tracked_progress_modal_while_main_disabled(hwnd);
    if tracked_progress.0 != 0 {
        log_debug(&format!(
            "Audio description: main still disabled at {stage}, preserving tracked progress modal={tracked_progress:?}"
        ));
        bring_modal_window_to_foreground(tracked_progress);
        return;
    }

    if has_pending_blocking_modal(hwnd) {
        log_debug(&format!(
            "Audio description: main still disabled at {stage}, preserving tracked blocking modal"
        ));
        reactivate_pending_blocking_modal(hwnd);
        return;
    }

    log_debug(&format!(
        "Audio description: recovering stale disabled main window at {stage}"
    ));
    enable_window_safe(hwnd, true);
    send_message_w_safe(hwnd, WM_CANCELMODE, WPARAM(0), LPARAM(0));
    unsafe {
        crate::log_if_err!(DrawMenuBar(hwnd));
    }
}

fn defer_copydata_paths_while_main_window_disabled(hwnd: HWND, paths: &[PathBuf]) -> bool {
    let main_window_enabled = unsafe { IsWindowEnabled(hwnd).as_bool() };
    if !should_defer_external_file_open(main_window_enabled) {
        return false;
    }

    with_state(hwnd, |state| {
        state
            .deferred_modal_copydata_open_paths
            .extend(paths.iter().cloned());
    });
    crate::log_if_err!(unsafe {
        PostMessageW(
            hwnd,
            WM_REACTIVATE_MODAL_AFTER_EXTERNAL_OPEN,
            WPARAM(0),
            LPARAM(0),
        )
    });

    unsafe {
        if SetTimer(
            hwnd,
            DEFERRED_MODAL_COPYDATA_OPEN_TIMER_ID,
            DEFERRED_MODAL_COPYDATA_OPEN_TIMER_INTERVAL_MS,
            None,
        ) == 0
        {
            log_debug("Failed to set DEFERRED_MODAL_COPYDATA_OPEN_TIMER_ID");
        }
    }
    true
}

fn process_deferred_modal_copydata_paths_if_ready(hwnd: HWND) {
    if has_pending_blocking_modal(hwnd) || unsafe { !IsWindowEnabled(hwnd).as_bool() } {
        return;
    }

    kill_timer_best_effort(
        hwnd,
        DEFERRED_MODAL_COPYDATA_OPEN_TIMER_ID,
        "KillTimer DEFERRED_MODAL_COPYDATA_OPEN",
    );
    let pending_paths = with_state(hwnd, |state| {
        std::mem::take(&mut state.deferred_modal_copydata_open_paths)
    })
    .unwrap_or_default();
    if pending_paths.is_empty() {
        return;
    }

    log_debug(&format!(
        "Opening {} Explorer file(s) deferred until modal windows closed",
        pending_paths.len()
    ));
    open_copydata_paths(hwnd, pending_paths);
}

fn take_deferred_copydata_paths_for_blocking_modal(hwnd: HWND) -> Vec<PathBuf> {
    with_state(hwnd, |state| {
        std::mem::take(&mut state.blocking_modal.deferred_copydata_open_paths)
    })
    .unwrap_or_default()
}

fn set_blocking_modal_active(hwnd: HWND, kind: Option<BlockingModalKind>) {
    with_state(hwnd, |state| {
        state.blocking_modal.active = kind;
    });
}

pub(crate) fn copydata_utf16_payload(
    cds_ptr: *const COPYDATASTRUCT,
    log_tag: &str,
) -> Option<String> {
    if cds_ptr.is_null() {
        log_debug(&format!("{log_tag}: null COPYDATASTRUCT"));
        return None;
    }
    let cds = unsafe { &*cds_ptr };
    if cds.cbData == 0 {
        log_debug(&format!("{log_tag}: empty WM_COPYDATA payload"));
        return None;
    }
    if cds.cbData % 2 != 0 {
        log_debug(&format!(
            "{log_tag}: invalid WM_COPYDATA payload size {}",
            cds.cbData
        ));
        return None;
    }
    if cds.lpData.is_null() {
        log_debug(&format!(
            "{log_tag}: null WM_COPYDATA payload pointer ({} bytes)",
            cds.cbData
        ));
        return None;
    }

    let len_u16 = (cds.cbData as usize) / 2;
    let slice = unsafe { std::slice::from_raw_parts(cds.lpData as *const u16, len_u16) };
    let len = if slice.last().copied() == Some(0) {
        len_u16 - 1
    } else {
        len_u16
    };
    Some(String::from_utf16_lossy(&slice[..len]))
}

fn show_blocking_modal_message_box_owned(
    app_hwnd: HWND,
    owner_hwnd: HWND,
    kind: BlockingModalKind,
    message: PCWSTR,
    title: PCWSTR,
    flags: MESSAGEBOX_STYLE,
) -> MESSAGEBOX_RESULT {
    watchdog::enter_modal_dialog();
    set_blocking_modal_active(app_hwnd, Some(kind));

    let disable_app = owner_hwnd != app_hwnd && unsafe { IsWindowEnabled(app_hwnd).as_bool() };
    if disable_app {
        unsafe {
            EnableWindow(app_hwnd, false);
        }
    }

    let result = unsafe { MessageBoxW(owner_hwnd, message, title, flags) };

    if disable_app {
        unsafe {
            EnableWindow(app_hwnd, true);
        }
    }

    set_blocking_modal_active(app_hwnd, None);
    let pending_paths = take_deferred_copydata_paths_for_blocking_modal(app_hwnd);
    if !pending_paths.is_empty() {
        open_copydata_paths(app_hwnd, pending_paths);
    } else if unsafe { IsWindowEnabled(app_hwnd).as_bool() } {
        restore_editor_focus(app_hwnd);
    } else {
        log_debug(
            "Blocking modal closed while its owner is still disabled; deferring editor focus restoration",
        );
        reactivate_modal_child_while_main_disabled(app_hwnd);
    }
    watchdog::exit_modal_dialog();
    result
}

pub(crate) fn show_blocking_modal_message_box(
    hwnd: HWND,
    kind: BlockingModalKind,
    message: PCWSTR,
    title: PCWSTR,
    flags: MESSAGEBOX_STYLE,
) -> MESSAGEBOX_RESULT {
    show_blocking_modal_message_box_owned(hwnd, hwnd, kind, message, title, flags)
}

fn open_copydata_paths(hwnd: HWND, paths: Vec<PathBuf>) {
    if paths.iter().all(|path| is_audio_path(path)) {
        queue_audio_files_and_play(hwnd, paths);
    } else {
        for path in paths {
            editor_manager::open_document_from_copydata(hwnd, &path);
        }
    }
    show_window_safe(hwnd, SW_SHOWMAXIMIZED);
    restore_editor_focus(hwnd);
}

pub(crate) fn set_focus_safe(hwnd: HWND) {
    unsafe {
        SetFocus(hwnd);
    }
}

pub(crate) fn is_window_handle_valid(hwnd: HWND) -> bool {
    unsafe { IsWindow(hwnd).as_bool() }
}

pub(crate) fn hwnd_from_create_struct_lparam_safe(create_struct: *const CREATESTRUCTW) -> HWND {
    unsafe { HWND((*create_struct).lpCreateParams as isize) }
}

pub(crate) fn wave_format_extensible_ref_safe(
    fmt: &windows::Win32::Media::Audio::WAVEFORMATEX,
) -> &windows::Win32::Media::Audio::WAVEFORMATEXTENSIBLE {
    unsafe { &*(fmt as *const _ as *const windows::Win32::Media::Audio::WAVEFORMATEXTENSIBLE) }
}

pub(crate) fn get_focus_safe() -> HWND {
    unsafe { GetFocus() }
}

pub(crate) fn set_foreground_window_safe(hwnd: HWND) {
    unsafe {
        SetForegroundWindow(hwnd);
    }
}

pub(crate) fn get_foreground_window_safe() -> HWND {
    unsafe { GetForegroundWindow() }
}

pub(crate) fn send_message_w_safe(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { SendMessageW(hwnd, msg, wparam, lparam) }
}

pub(crate) fn def_window_proc_w_safe(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

pub(crate) fn get_window_long_ptr_w_safe(
    hwnd: HWND,
    index: windows::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX,
) -> isize {
    unsafe { GetWindowLongPtrW(hwnd, index) }
}

pub(crate) fn set_window_long_ptr_w_safe(
    hwnd: HWND,
    index: windows::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX,
    new_long: isize,
) -> isize {
    unsafe { SetWindowLongPtrW(hwnd, index, new_long) }
}

pub(crate) fn enable_window_safe(hwnd: HWND, enable: bool) -> BOOL {
    unsafe { EnableWindow(hwnd, enable) }
}

pub(crate) fn show_window_safe(
    hwnd: HWND,
    cmdshow: windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD,
) -> BOOL {
    unsafe { ShowWindow(hwnd, cmdshow) }
}

pub(crate) fn load_cursor_w_safe(
    instance: HINSTANCE,
    cursor_name: PCWSTR,
) -> windows::core::Result<HCURSOR> {
    unsafe { LoadCursorW(instance, cursor_name) }
}

pub(crate) fn get_window_rect_safe(hwnd: HWND, rect: &mut RECT) -> windows::core::Result<()> {
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, rect) }
}

pub(crate) fn get_client_rect_safe(hwnd: HWND, rect: &mut RECT) -> windows::core::Result<()> {
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, rect) }
}

pub(crate) fn create_menu_safe() -> windows::core::Result<HMENU> {
    unsafe { windows::Win32::UI::WindowsAndMessaging::CreateMenu() }
}

pub(crate) fn destroy_menu_safe(menu: HMENU) -> windows::core::Result<()> {
    unsafe { windows::Win32::UI::WindowsAndMessaging::DestroyMenu(menu) }
}

pub(crate) fn kill_timer_safe(hwnd: HWND, id_event: usize) -> windows::core::Result<()> {
    unsafe { KillTimer(hwnd, id_event) }
}

pub(crate) fn co_create_instance_safe<T: Interface>(
    clsid: *const windows::core::GUID,
    outer: Option<&windows::core::IUnknown>,
    clsctx: windows::Win32::System::Com::CLSCTX,
) -> windows::core::Result<T> {
    unsafe { CoCreateInstance(clsid, outer, clsctx) }
}

pub(crate) fn get_last_error_safe() -> WIN32_ERROR {
    unsafe { GetLastError() }
}

pub(crate) fn get_message_w_safe(
    lpmsg: *mut MSG,
    hwnd: HWND,
    wmsgfiltermin: u32,
    wmsgfiltermax: u32,
) -> BOOL {
    unsafe { GetMessageW(lpmsg, hwnd, wmsgfiltermin, wmsgfiltermax) }
}

pub(crate) fn is_dialog_message_w_safe(hwnd: HWND, msg: &MSG) -> BOOL {
    unsafe { IsDialogMessageW(hwnd, msg) }
}

pub(crate) fn get_next_dlg_tab_item_safe(hwnd: HWND, hwndctrl: HWND, bprevious: bool) -> HWND {
    unsafe { GetNextDlgTabItem(hwnd, hwndctrl, bprevious) }
}

pub(crate) fn wait_for_single_object_safe(
    handle: HANDLE,
    milliseconds: u32,
) -> windows::Win32::Foundation::WAIT_EVENT {
    unsafe { WaitForSingleObject(handle, milliseconds) }
}

pub(crate) fn find_window_w_safe(class_name: PCWSTR, window_name: PCWSTR) -> HWND {
    unsafe { FindWindowW(class_name, window_name) }
}

pub(crate) fn call_window_proc_w_safe(
    prev_wnd_func: WNDPROC,
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { CallWindowProcW(prev_wnd_func, hwnd, msg, wparam, lparam) }
}

pub(crate) fn isize_to_wndproc_safe(value: isize) -> WNDPROC {
    unsafe {
        Some(std::mem::transmute::<
            isize,
            unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
        >(value))
    }
}

pub(crate) fn destroy_window_safe(hwnd: HWND) -> windows::core::Result<()> {
    unsafe { DestroyWindow(hwnd) }
}

pub(crate) fn close_clipboard_safe() -> windows::core::Result<()> {
    unsafe { CloseClipboard() }
}

pub(crate) fn open_clipboard_safe(hwnd_new_owner: HWND) -> windows::core::Result<()> {
    unsafe { OpenClipboard(hwnd_new_owner) }
}

pub(crate) fn empty_clipboard_safe() -> windows::core::Result<()> {
    unsafe { EmptyClipboard() }
}

pub(crate) fn set_clipboard_data_safe(format: u32, hmem: HANDLE) -> windows::core::Result<HANDLE> {
    unsafe { SetClipboardData(format, hmem) }
}

pub(crate) fn global_alloc_safe(
    flags: GLOBAL_ALLOC_FLAGS,
    bytes: usize,
) -> windows::core::Result<windows::Win32::Foundation::HGLOBAL> {
    unsafe { GlobalAlloc(flags, bytes) }
}

pub(crate) fn global_lock_as_safe(
    hmem: windows::Win32::Foundation::HGLOBAL,
) -> *mut core::ffi::c_void {
    unsafe { GlobalLock(hmem) }
}

pub(crate) fn global_unlock_safe(
    hmem: windows::Win32::Foundation::HGLOBAL,
) -> windows::core::Result<()> {
    unsafe { GlobalUnlock(hmem) }
}

pub(crate) fn is_clipboard_format_available_safe(format: u32) -> bool {
    unsafe { IsClipboardFormatAvailable(format).is_ok() }
}

pub(crate) fn register_class_w_safe(
    class: &windows::Win32::UI::WindowsAndMessaging::WNDCLASSW,
) -> u16 {
    unsafe { RegisterClassW(class) }
}

pub(crate) fn set_window_text_w_safe(hwnd: HWND, text: PCWSTR) -> windows::core::Result<()> {
    unsafe { SetWindowTextW(hwnd, text) }
}

pub(crate) fn get_cursor_pos_safe(point: *mut POINT) -> windows::core::Result<()> {
    unsafe { GetCursorPos(point) }
}

pub(crate) fn get_save_file_name_w_safe(ofn: *mut OPENFILENAMEW) -> BOOL {
    unsafe { GetSaveFileNameW(ofn) }
}

pub(crate) fn get_open_file_name_w_safe(ofn: *mut OPENFILENAMEW) -> BOOL {
    unsafe { GetOpenFileNameW(ofn) }
}

pub(crate) fn drag_query_file_w_safe(
    hdrop: HDROP,
    ifile: u32,
    lpszfile: Option<&mut [u16]>,
) -> u32 {
    unsafe { DragQueryFileW(hdrop, ifile, lpszfile) }
}

pub(crate) fn get_user_default_locale_name_safe(locale_name: &mut [u16]) -> i32 {
    unsafe { GetUserDefaultLocaleName(locale_name) }
}

pub(crate) fn post_message_w_safe(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::core::Result<()> {
    unsafe { PostMessageW(hwnd, msg, wparam, lparam) }
}

pub(crate) fn message_box_w_safe(
    hwnd: HWND,
    text: PCWSTR,
    caption: PCWSTR,
    typ: MESSAGEBOX_STYLE,
) -> MESSAGEBOX_RESULT {
    unsafe { MessageBoxW(hwnd, text, caption, typ) }
}

pub(crate) fn get_module_handle_raw_default() -> isize {
    unsafe { GetModuleHandleW(None).unwrap_or_default().0 }
}

pub(crate) fn get_class_name_w_safe(hwnd: HWND, class_name: &mut [u16]) -> i32 {
    unsafe { GetClassNameW(hwnd, class_name) }
}

pub(crate) fn get_window_text_length_w_safe(hwnd: HWND) -> i32 {
    unsafe { GetWindowTextLengthW(hwnd) }
}

pub(crate) fn get_window_text_w_safe(hwnd: HWND, string: &mut [u16]) -> i32 {
    unsafe { GetWindowTextW(hwnd, string) }
}

fn log_foreground_snapshot(tag: &str) {
    const WINDOW_TEXT_LOG_PREVIEW_CHARS: usize = 180;

    fn redact_prefixed_secret(
        text: &str,
        prefix: &str,
        min_secret_chars: usize,
        replacement: &str,
    ) -> String {
        let mut output = String::with_capacity(text.len());
        let mut cursor = 0usize;

        while let Some(relative_start) = text[cursor..].find(prefix) {
            let start = cursor + relative_start;
            output.push_str(&text[cursor..start]);

            let secret_start = start + prefix.len();
            let mut secret_end = secret_start;
            let mut secret_chars = 0usize;
            for (relative_index, ch) in text[secret_start..].char_indices() {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
                    secret_end = secret_start + relative_index + ch.len_utf8();
                    secret_chars += 1;
                } else {
                    break;
                }
            }

            if secret_chars >= min_secret_chars {
                output.push_str(replacement);
                cursor = secret_end;
            } else {
                output.push_str(prefix);
                cursor = secret_start;
            }
        }

        output.push_str(&text[cursor..]);
        output
    }

    fn redact_sensitive_log_text(text: &str) -> String {
        // Keep the diagnostic text intact, but never persist credentials used by
        // Gemini/Sonarpad AI if an edit control happens to contain them.
        let redacted = redact_prefixed_secret(text, "AIza", 20, "[REDACTED_GEMINI_API_KEY]");
        let redacted = redact_prefixed_secret(&redacted, "sp_", 12, "[REDACTED_SONARPAD_CODE]");
        let redacted = redact_prefixed_secret(&redacted, "sst_", 12, "[REDACTED_SONARPAD_SESSION]");
        redact_prefixed_secret(&redacted, "Bearer ", 12, "Bearer [REDACTED_TOKEN]")
    }

    fn preview_window_text(text: &str) -> String {
        let safe_text = redact_sensitive_log_text(text);
        let mut preview = safe_text
            .chars()
            .take(WINDOW_TEXT_LOG_PREVIEW_CHARS)
            .collect::<String>();
        preview = preview.replace('\r', "\\r").replace('\n', "\\n");
        if safe_text.chars().count() > WINDOW_TEXT_LOG_PREVIEW_CHARS {
            preview.push_str("...");
        }
        preview
    }

    let foreground = unsafe { GetForegroundWindow() };
    let focus = unsafe { GetFocus() };
    let describe_hwnd = |hwnd: HWND| {
        let mut class_buf = [0u16; 128];
        let class_len = get_class_name_w_safe(hwnd, &mut class_buf);
        let class_name = if class_len > 0 {
            String::from_utf16_lossy(&class_buf[..class_len as usize])
        } else {
            String::new()
        };
        let text_len = get_window_text_length_w_safe(hwnd);
        let window_text = if text_len > 0 {
            let mut text_buf = vec![0u16; text_len as usize + 1];
            let read = get_window_text_w_safe(hwnd, &mut text_buf);
            String::from_utf16_lossy(&text_buf[..read.max(0) as usize])
        } else {
            String::new()
        };
        let text_chars = window_text.chars().count();
        (class_name, preview_window_text(&window_text), text_chars)
    };
    let (foreground_class, foreground_text, foreground_text_chars) = describe_hwnd(foreground);
    let (focus_class, focus_text, focus_text_chars) = describe_hwnd(focus);
    log_debug(&format!(
        "{}: foreground={:?} class='{}' text='{}' text_chars={} focus={:?} focus_class='{}' focus_text='{}' focus_text_chars={}",
        tag,
        foreground,
        foreground_class,
        foreground_text,
        foreground_text_chars,
        focus,
        focus_class,
        focus_text,
        focus_text_chars
    ));
}

fn log_mpv_focus_snapshot(hwnd: HWND, tag: &str) {
    let session = with_state(hwnd, |state| state.active_mpv_session.clone()).flatten();
    if let Some(session) = session {
        log_debug(&format!(
            "{}: active_mpv_pid={} active_url={:?}",
            tag,
            session.process_id,
            with_state(hwnd, |state| state.active_podcast_episode_url.clone()).flatten()
        ));
        log_foreground_snapshot(tag);
    }
}

pub(crate) fn get_key_state_safe(vkey: i32) -> i16 {
    unsafe { GetKeyState(vkey) }
}

pub(crate) fn get_menu_safe(hwnd: HWND) -> HMENU {
    unsafe { GetMenu(hwnd) }
}

pub(crate) fn is_child_safe(parent: HWND, child: HWND) -> bool {
    unsafe { IsChild(parent, child).as_bool() }
}

pub(crate) fn create_popup_menu_safe() -> HMENU {
    unsafe { CreatePopupMenu().unwrap_or(HMENU(0)) }
}

pub(crate) fn check_menu_item_safe(hmenu: HMENU, id_check_item: u32, u_check: u32) -> u32 {
    unsafe { CheckMenuItem(hmenu, id_check_item, u_check) }
}

pub(crate) fn track_popup_menu_safe(
    menu: HMENU,
    flags: windows::Win32::UI::WindowsAndMessaging::TRACK_POPUP_MENU_FLAGS,
    x: i32,
    y: i32,
    reserved: i32,
    hwnd: HWND,
    rect: Option<*const RECT>,
) -> BOOL {
    unsafe { TrackPopupMenu(menu, flags, x, y, reserved, hwnd, rect) }
}

pub(crate) fn append_menu_w_safe(
    menu: HMENU,
    flags: windows::Win32::UI::WindowsAndMessaging::MENU_ITEM_FLAGS,
    new_item_id: usize,
    text: PCWSTR,
) -> windows::core::Result<()> {
    unsafe { AppendMenuW(menu, flags, new_item_id, text) }
}

pub(crate) fn get_parent_safe(hwnd: HWND) -> HWND {
    unsafe { GetParent(hwnd) }
}

pub(crate) fn get_dlg_ctrl_id_safe(hwnd: HWND) -> usize {
    unsafe { GetDlgCtrlID(hwnd) as usize }
}

pub(crate) fn get_dlg_item_safe(parent: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(parent, id) }
}

pub(crate) fn with_raw_mut_ptr_safe<T, F, R>(ptr: *mut T, f: F) -> Option<R>
where
    F: FnOnce(&mut T) -> R,
{
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { f(&mut *ptr) })
    }
}

pub(crate) fn box_from_raw_safe<T>(ptr: *mut T) -> Box<T> {
    // Safety: caller guarantees `ptr` was allocated by `Box::into_raw`
    // and is consumed exactly once.
    unsafe { Box::from_raw(ptr) }
}

pub(crate) fn cstr_ptr_to_lossy_string_safe(ptr: *const i8) -> String {
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

pub(crate) fn read_unaligned_safe<T: Copy>(src: *const T) -> T {
    unsafe { std::ptr::read_unaligned(src) }
}

pub(crate) fn get_stock_object_safe(obj: GET_STOCK_OBJECT_FLAGS) -> HGDIOBJ {
    unsafe { GetStockObject(obj) }
}

pub(crate) fn zeroed_safe<T>() -> T {
    unsafe { std::mem::zeroed() }
}

pub(crate) fn reset_spellcheck_state(hwnd: HWND) {
    {
        if with_state(hwnd, |state| {
            state.spellcheck_manager.clear_cache();
            state.spellcheck_last_announce = None;
            state.spellcheck_context = None;
            state.spellcheck_typing_in_progress = false;
        })
        .is_none()
        {
            crate::log_debug("Failed to reset spellcheck state");
        }
    }
}

struct PdfLoadResult {
    hwnd_edit: HWND,
    path: PathBuf,
    result: Result<PdfTextResult, String>,
    from_copydata: bool,
}

pub struct UpdateDialogRequest {
    pub text: String,
    pub title: String,
    pub flags: MESSAGEBOX_STYLE,
    pub response_tx: mpsc::Sender<i32>,
}

pub struct UpdateProgressOpenRequest {
    pub language: Language,
    pub response_tx: mpsc::Sender<isize>,
}

struct PdfLoadingState {
    hwnd_edit: HWND,
    timer_id: usize,
    frame: usize,
    start_time: Instant,
    ocr_timeout_secs: u64,
}

struct PodcastEpisodeSaveResult {
    language: Language,
    target_path: PathBuf,
    error: Option<String>,
    open_audio_description: bool,
}

struct PodcastEpisodePlayReady {
    url: String,
    podcast_title: Option<String>,
    title: Option<String>,
    cache_path: PathBuf,
    prefer_title_for_document: bool,
    rai_origin: RaiAudioOrigin,
}

#[derive(Clone, Debug)]
pub(crate) struct RaiPlayLiveAudioVariant {
    pub(crate) track: crate::ffmpeg_source::AudioStreamInfo,
    pub(crate) url: String,
}

#[derive(Clone)]
struct MpvPlaybackSession {
    ipc_path: PathBuf,
    process_id: u32,
}

#[derive(Clone)]
struct MpvPlaybackStatus {
    volume: f32,
    speed: f32,
    pitch: f32,
}

struct YoutubeMpvPlaybackFailure {
    process_id: u32,
    detail: String,
}

fn media_playback_volume_from_settings(hwnd: HWND) -> f32 {
    with_state(hwnd, |state| state.settings.audiobook_playback_volume)
        .unwrap_or(1.0)
        .clamp(0.0, 3.0)
}

fn media_playback_volume_to_mpv(volume: f32) -> f32 {
    (volume * 100.0).clamp(0.0, 300.0)
}

fn persist_media_playback_volume(hwnd: HWND, volume: f32) {
    let volume = volume.clamp(0.0, 3.0);
    if with_state(hwnd, |state| {
        state.settings.audiobook_playback_volume = volume;
        crate::settings::save_settings(state.settings.clone());
    })
    .is_none()
    {
        log_debug("Failed to persist media playback volume setting");
    }
}

fn is_edit_control(hwnd: HWND) -> bool {
    if hwnd.0 == 0 {
        log_debug("edit shortcut: focus hwnd is null");
        return false;
    }
    let mut class_name = [0u16; 64];
    let len = get_class_name_w_safe(hwnd, &mut class_name);
    if len <= 0 {
        log_debug(&format!(
            "edit shortcut: GetClassName failed for focus {:?}",
            hwnd
        ));
        return false;
    }
    let class_name = String::from_utf16_lossy(&class_name[..len as usize]);
    let class_name = class_name.to_ascii_lowercase();
    let is_edit = class_name == "edit" || class_name.contains("richedit");
    log_debug(&format!(
        "edit shortcut: focus={:?} class={} is_edit={}",
        hwnd, class_name, is_edit
    ));
    is_edit
}

pub(crate) fn handle_focused_edit_shortcut(msg: &MSG) -> bool {
    if msg.message != WM_KEYDOWN {
        return false;
    }
    let ctrl_down = (get_key_state_safe(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
    let shift_down = (get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
    let alt_down = (get_key_state_safe(VK_MENU.0 as i32) & (0x8000u16 as i16)) != 0;
    if !ctrl_down || shift_down || alt_down {
        return false;
    }

    let key = msg.wParam.0 as u32;
    let is_standard_edit_shortcut = matches!(
        key,
        key if key == 'A' as u32
            || key == 'C' as u32
            || key == 'X' as u32
            || key == 'V' as u32
            || key == 'Z' as u32
            || key == 'Y' as u32
    );
    if !is_standard_edit_shortcut {
        return false;
    }

    let focus = get_focus_safe();
    log_debug(&format!(
        "edit shortcut: candidate key={} msg_hwnd={:?} focus={:?}",
        key as u8 as char, msg.hwnd, focus
    ));
    if !is_edit_control(focus) {
        return false;
    }

    match key {
        key if key == 'A' as u32 => {
            log_debug("edit shortcut: Ctrl+A -> focused edit");
            send_message_w_safe(focus, EM_SETSEL, WPARAM(0), LPARAM(-1));
            true
        }
        key if key == 'C' as u32 => {
            log_debug("edit shortcut: Ctrl+C -> focused edit");
            send_message_w_safe(focus, WM_COPY, WPARAM(0), LPARAM(0));
            true
        }
        key if key == 'X' as u32 => {
            log_debug("edit shortcut: Ctrl+X -> focused edit");
            send_message_w_safe(focus, WM_CUT, WPARAM(0), LPARAM(0));
            true
        }
        key if key == 'V' as u32 => {
            log_debug("edit shortcut: Ctrl+V -> focused edit");
            send_message_w_safe(focus, WM_PASTE, WPARAM(0), LPARAM(0));
            true
        }
        key if key == 'Z' as u32 => {
            log_debug("edit shortcut: Ctrl+Z -> focused edit");
            send_message_w_safe(focus, WM_UNDO_MSG, WPARAM(0), LPARAM(0));
            true
        }
        key if key == 'Y' as u32 => {
            log_debug("edit shortcut: Ctrl+Y -> focused edit");
            send_message_w_safe(focus, EM_REDO, WPARAM(0), LPARAM(0));
            true
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RaiAudioOrigin {
    #[default]
    None,
    Recenti,
    Tutte,
    RaiPlay,
    La7Play,
    RaiPlaySound,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct YouTubeReturnContext {
    pub input: Option<String>,
    pub collection_page: Option<usize>,
    pub selected_label: Option<String>,
    pub previous_input: Option<String>,
    pub previous_collection_page: Option<usize>,
    pub previous_selected_label: Option<String>,
}

struct PodcastEpisodePlayFailed {
    language: Language,
    error: String,
}

struct PodcastChaptersReady {
    key: String,
    chapters: Option<Vec<Chapter>>,
}

#[derive(Clone, PartialEq, Eq)]
struct SpellcheckAnnounceKey {
    doc_id: isize,
    line_index: i32,
    start_utf8: usize,
    end_utf8: usize,
    line_hash: u64,
    language: String,
}

#[derive(Clone)]
struct SpellcheckContextMenuState {
    hwnd_edit: HWND,
    line_start: i32,
    language: String,
    word_range: (usize, usize),
    word: String,
    line_text: String,
    suggestions: Vec<String>,
}

struct SpellcheckWordContext {
    doc_id: isize,
    line_index: i32,
    line_start: i32,
    line_text: String,
    line_hash: u64,
    word_range: (usize, usize),
    word: String,
}

fn log_path() -> Option<PathBuf> {
    let mut path = settings::settings_dir();
    path.push("Sonarpad.log");
    Some(path)
}

const MAX_LOG_SIZE: u64 = 1024 * 1024;
static LOG_WRITE_LOCK: Mutex<()> = Mutex::new(());

fn log_lock_path(log_path: &Path) -> Option<PathBuf> {
    let parent = log_path.parent()?;
    Some(parent.join("Sonarpad.log.lock"))
}

fn rotated_log_path(log_path: &Path) -> Option<PathBuf> {
    let parent = log_path.parent()?;
    Some(parent.join("Sonarpad.log.1"))
}

fn rotate_log_if_needed(path: &Path) {
    static LOG_INIT: Once = Once::new();
    LOG_INIT.call_once(|| {
        let Some(lock_path) = log_lock_path(path) else {
            return;
        };
        let start = Instant::now();
        let mut lock_acquired = false;
        while start.elapsed() < Duration::from_millis(200) {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(mut file) => {
                    lock_acquired = true;
                    let _write_lock_pid_result = writeln!(file, "{}", std::process::id());
                    break;
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => {
                    break;
                }
            }
        }
        if !lock_acquired {
            return;
        }

        let needs_rotation = path.metadata().ok().map(|m| m.len() > MAX_LOG_SIZE) == Some(true);
        if needs_rotation && let Some(rotated_path) = rotated_log_path(path) {
            match std::fs::remove_file(&rotated_path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    let _remove_lock_result = std::fs::remove_file(&lock_path);
                    return;
                }
            }
            if std::fs::rename(path, &rotated_path).is_err() {
                let _remove_lock_result = std::fs::remove_file(&lock_path);
                return;
            }
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)
            {
                let _write_rotation_marker_result = writeln!(
                    file,
                    "[INFO] log rotated (previous log saved as Sonarpad.log.1 after exceeding 1 MB)"
                );
            }
        }

        let _remove_lock_result = std::fs::remove_file(&lock_path);
    });
}

pub(crate) fn log_debug(message: &str) {
    // Isolated SAPI5 workers are monitored by the parent diagnostic log.
    // Avoid concurrent writes and log rotation races from many child processes.
    if std::env::var_os("SONARPAD_SAPI5_WORKER").is_some() {
        return;
    }

    // Push to telemetry ring buffer for hang diagnostics
    telemetry::push_log_line(message);

    let Some(path) = log_path() else {
        return;
    };
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    append_debug_log(&path, message);
}

fn append_debug_log(path: &Path, message: &str) {
    let _write_guard = match LOG_WRITE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    rotate_log_if_needed(path);
    if let Ok(mut log) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        if writeln!(log, "[{timestamp}] {message}").is_err() {}
    }
}

fn kill_timer_best_effort(hwnd: HWND, timer_id: usize, context: &str) {
    unsafe {
        SetLastError(WIN32_ERROR(0));
        if let Err(err) = KillTimer(hwnd, timer_id) {
            let last_error = GetLastError();
            if last_error == WIN32_ERROR(0)
                || last_error == ERROR_INVALID_PARAMETER
                || last_error == ERROR_INVALID_WINDOW_HANDLE
            {
                return;
            }
            log_debug(&format!(
                "{} failed: {:?} (win32={})",
                context, err, last_error.0
            ));
        }
    }
}

fn delete_menu_best_effort(menu: HMENU, position: u32, flags: MENU_ITEM_FLAGS, context: &str) {
    unsafe {
        SetLastError(WIN32_ERROR(0));
        if let Err(err) = DeleteMenu(menu, position, flags) {
            let last_error = GetLastError();
            if last_error == WIN32_ERROR(0) || last_error == ERROR_MENU_ITEM_NOT_FOUND {
                return;
            }
            log_debug(&format!(
                "{} failed: {:?} (win32={})",
                context, err, last_error.0
            ));
        }
    }
}

fn clean_menu_label(label: &str) -> String {
    let main = label.split('\t').next().unwrap_or(label);
    let mut cleaned = String::with_capacity(main.len());
    for ch in main.chars() {
        if ch != '&' {
            cleaned.push(ch);
        }
    }
    // Remove accelerator patterns like "(&X)" or "(X)" at the end
    let trimmed = cleaned.trim();
    if let Some(pos) = trimmed.rfind(" (") {
        let suffix = &trimmed[pos..];
        // Check if it matches pattern " (&X)" or " (X)" where X is a single char
        if suffix.len() <= 5 && suffix.ends_with(')') {
            return trimmed[..pos].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn confirm_menu_action(hwnd: HWND, key: &str) {
    let language = { with_state(hwnd, |state| state.settings.language).unwrap_or_default() };
    let label = i18n::tr(language, key);
    let cleaned = clean_menu_label(&label);
    if !cleaned.is_empty() {
        if key.starts_with("edit.") {
            {
                with_state(hwnd, |state| {
                    state.undo_action_label = Some(cleaned.clone())
                });
            }
        }
        let message = i18n::tr_f(language, "app.action_completed", &[("action", &cleaned)]);
        show_info(hwnd, language, &message);
    }
}

fn dictionary_cache_key(language: Language, pref: &str, word: &str) -> String {
    let lang = match language {
        Language::Italian => "it",
        Language::German => "de",
        Language::English => "en",
        Language::Spanish => "es",
        Language::Portuguese => "pt",
        Language::PortugueseBrazilian => "pt-BR",
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
    };
    format!(
        "{}|{}|{}",
        lang,
        pref.trim().to_ascii_lowercase(),
        word.trim().to_ascii_lowercase()
    )
}

fn dictionary_cache_path() -> std::path::PathBuf {
    settings::settings_dir().join("dictionary_cache.json")
}

fn load_dictionary_cache() -> HashMap<String, Vec<String>> {
    let path = dictionary_cache_path();
    if !path.exists() {
        return HashMap::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save_dictionary_cache(cache: &HashMap<String, Vec<String>>) {
    let path = dictionary_cache_path();
    if let Ok(content) = serde_json::to_string(cache) {
        crate::log_if_err!(std::fs::write(path, content));
    }
}

pub(crate) fn is_dictionary_not_found_cache_entry(language: Language, lines: &[String]) -> bool {
    lines.len() == 1 && lines[0] == i18n::tr(language, "dictionary.not_found")
}

pub(crate) fn update_dictionary_cache(hwnd: HWND, key: String, lines: Vec<String>) {
    {
        if with_state(hwnd, |state| {
            state.dictionary_cache.insert(key, lines);
            save_dictionary_cache(&state.dictionary_cache);
        })
        .is_none()
        {
            crate::log_debug("Failed to update dictionary cache state");
        }
    }
}

pub(crate) fn remove_dictionary_cache(hwnd: HWND, key: &str) {
    {
        if with_state(hwnd, |state| {
            let removed = state.dictionary_cache.remove(key).is_some();
            if removed {
                save_dictionary_cache(&state.dictionary_cache);
            }
        })
        .is_none()
        {
            crate::log_debug("Failed to remove dictionary cache state");
        }
    }
}

struct DictionaryLookupResult {
    key: String,
    lines: Vec<String>,
    generation: usize,
    cacheable: bool,
}

fn start_dictionary_lookup(
    hwnd_val: isize,
    word: String,
    language: Language,
    pref: String,
    key: String,
    generation: usize,
) {
    std::thread::spawn(move || {
        let (lines, cacheable) = match wiktionary::lookup_for_language(&word, language, &pref) {
            Ok(entry) => (wiktionary::format_menu_lines(language, &entry), true),
            Err(wiktionary::LookupError::NotFound { .. }) => {
                (vec![i18n::tr(language, "dictionary.not_found")], false)
            }
            Err(err) => {
                log_debug(&format!("Dictionary lookup failed: {err}"));
                (vec![i18n::tr(language, "dictionary.not_found")], false)
            }
        };
        let result = Box::new(DictionaryLookupResult {
            key,
            lines,
            generation,
            cacheable,
        });
        let hwnd = HWND(hwnd_val);
        if crate::is_window_handle_valid(hwnd) {
            unsafe {
                crate::log_if_err!(PostMessageW(
                    hwnd,
                    WM_DICTIONARY_LOADED,
                    WPARAM(0),
                    LPARAM(Box::into_raw(result) as isize),
                ));
            }
        }
    });
}

fn prefetch_dictionary_for_selection(hwnd: HWND, hwnd_edit: HWND) {
    let mut range = CHARRANGE::default();
    // SAFETY: `hwnd_edit` is an edit control and `range` is valid writable memory.
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut range as *mut _ as isize),
        );
    }
    let start = range.cpMin;
    let end = range.cpMax;
    if start >= end || (end - start) > 50 {
        return;
    }
    let len = (end - start) as usize;
    let mut buf = vec![0u16; len + 1];
    let mut tr = TEXTRANGEW {
        chrg: CHARRANGE {
            cpMin: start,
            cpMax: end,
        },
        lpstrText: windows::core::PWSTR(buf.as_mut_ptr()),
    };
    // SAFETY: `tr` points to `buf`, which is allocated for the requested range plus terminator.
    let copied = crate::send_message_w_safe(
        hwnd_edit,
        EM_GETTEXTRANGE,
        WPARAM(0),
        LPARAM(&mut tr as *mut _ as isize),
    )
    .0 as usize;
    if copied == 0 {
        return;
    }
    let selected = String::from_utf16_lossy(&buf[..copied]);
    let trimmed = selected.trim();
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
        return;
    }
    let word = trimmed.to_string();

    let prefetch_info = {
        with_state(hwnd, |state| {
            let language = state.settings.language;
            let pref = state.settings.dictionary_translation_language.clone();
            let key = dictionary_cache_key(language, &pref, &word);
            if let Some(lines) = state.dictionary_cache.get(&key).cloned() {
                if is_dictionary_not_found_cache_entry(language, &lines) {
                    state.dictionary_cache.remove(&key);
                    save_dictionary_cache(&state.dictionary_cache);
                } else {
                    return None;
                }
            }
            if state.dictionary_pending_lookup.as_ref() == Some(&key) {
                return None;
            }
            state.dictionary_pending_lookup = Some(key.clone());
            let generation = state.dictionary_prefetch_generation;
            Some((word.clone(), language, pref, key, generation))
        })
    }
    .flatten();

    if let Some((word, language, pref, key, generation)) = prefetch_info {
        start_dictionary_lookup(hwnd.0, word, language, pref, key, generation);
    }
}

fn format_time_hms(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    }
}

fn audiobook_position_ms_from_state(state: &AppState) -> Option<u64> {
    let player = state.active_audiobook.as_ref()?;
    if let Some(pos_secs) = player.position_secs() {
        return Some((pos_secs * 1000.0).max(0.0) as u64);
    }
    let accumulated_ms = player.accumulated_seconds.saturating_mul(1000);
    if player.is_paused {
        return Some(accumulated_ms);
    }
    let elapsed_real_ms = player.start_instant.elapsed().as_millis() as f64;
    let elapsed_audio_ms = (elapsed_real_ms * player.speed as f64) as u64;
    Some(accumulated_ms.saturating_add(elapsed_audio_ms))
}

fn update_chapter_announcement(hwnd: HWND) {
    let (current_pos_ms, chapters, last_idx, language) = {
        with_state(hwnd, |state| {
            (
                audiobook_position_ms_from_state(state),
                state.active_podcast_chapters.clone(),
                state.last_announced_chapter_index,
                state.settings.language,
            )
        })
    }
    .unwrap_or((None, Vec::new(), None, Language::default()));
    let Some(current_pos_ms) = current_pos_ms else {
        return;
    };
    if chapters.is_empty() {
        if { with_state(hwnd, |state| state.last_announced_chapter_index = None) }.is_none() {
            crate::log_debug("Failed to clear last announced chapter index");
        }
        return;
    }
    let current_idx = crate::podcast::chapters::current_chapter_index(current_pos_ms, &chapters);
    if current_idx == last_idx {
        return;
    }
    if {
        with_state(hwnd, |state| {
            state.last_announced_chapter_index = current_idx
        })
    }
    .is_none()
    {
        crate::log_debug("Failed to update last announced chapter index");
    }
    let Some(idx) = current_idx else {
        return;
    };
    if let Some(chapter) = chapters.get(idx) {
        let message = i18n::tr_f(
            language,
            "playback.chapter_announce",
            &[("title", &chapter.title)],
        );
        nvda_speak(&message);
    }
}

fn announce_current_chapter_on_start(
    hwnd: HWND,
    chapters: &[Chapter],
    current_pos_ms: Option<u64>,
    language: Language,
) {
    if chapters.is_empty() {
        return;
    }
    let current_idx = current_pos_ms
        .and_then(|pos| crate::podcast::chapters::current_chapter_index(pos, chapters))
        .or(Some(0));
    if with_state(hwnd, |state| {
        state.last_announced_chapter_index = current_idx
    })
    .is_none()
    {
        crate::log_debug("Failed to update last announced chapter index");
    };
    let Some(idx) = current_idx else {
        return;
    };
    if let Some(chapter) = chapters.get(idx) {
        let message = i18n::tr_f(
            language,
            "playback.chapter_announce",
            &[("title", &chapter.title)],
        );
        nvda_speak(&message);
    }
}

pub(crate) fn clear_active_podcast_chapters(hwnd: HWND) {
    {
        if with_state(hwnd, |state| {
            state.active_podcast_chapters_key = None;
            state.active_podcast_chapters.clear();
            state.last_announced_chapter_index = None;
            state.active_podcast_episode_url = None;
            state.active_podcast_episode_media_url = None;
            state.active_podcast_title = None;
            state.active_podcast_episode_title = None;
            state.active_podcast_episode_cache = None;
            state.active_podcast_episode_from_rai = RaiAudioOrigin::None;
            state.raiplay_live_audio_variants.clear();
            state.active_mpv_session = None;
            state.active_mpv_status = None;
            state.active_youtube_return_context = YouTubeReturnContext::default();
        })
        .is_none()
        {
            crate::log_debug("Failed to clear active podcast chapters");
        }
        kill_timer_best_effort(
            hwnd,
            CHAPTER_ANNOUNCE_TIMER_ID,
            "KillTimer CHAPTER_ANNOUNCE",
        );
    }
}

pub(crate) fn reset_active_podcast_chapters_for_playback(hwnd: HWND) {
    let (has_pending, has_active) = {
        with_state(hwnd, |state| {
            (
                state.pending_podcast_chapters_key.is_some(),
                state.active_podcast_chapters_key.is_some(),
            )
        })
        .unwrap_or((false, false))
    };
    if has_pending {
        {
            with_state(hwnd, |state| {
                state.active_podcast_chapters_key = None;
                state.active_podcast_chapters.clear();
                state.last_announced_chapter_index = None;
                state.active_podcast_episode_url = None;
                state.active_podcast_episode_media_url = None;
                state.active_podcast_title = None;
                state.active_podcast_episode_title = None;
                state.active_podcast_episode_cache = None;
                state.active_podcast_episode_from_rai = RaiAudioOrigin::None;
                state.raiplay_live_audio_variants.clear();
                state.active_mpv_session = None;
                state.active_mpv_status = None;
                state.active_youtube_return_context = YouTubeReturnContext::default();
            });
            kill_timer_best_effort(
                hwnd,
                CHAPTER_ANNOUNCE_TIMER_ID,
                "KillTimer CHAPTER_ANNOUNCE",
            );
        }
        return;
    }
    if !has_active {
        clear_active_podcast_chapters(hwnd);
    }
}

pub(crate) fn set_pending_podcast_chapters_key(hwnd: HWND, key: Option<String>) {
    with_state(hwnd, |state| state.pending_podcast_chapters_key = key);
}

pub(crate) fn activate_pending_podcast_chapters(hwnd: HWND) {
    let (chapters, language, current_pos_ms) = {
        with_state(hwnd, |state| {
            let key = state.pending_podcast_chapters_key.take();
            state.active_podcast_chapters_key = key.clone();
            state.last_announced_chapter_index = None;
            if let Some(key) = key.as_ref()
                && let Some(cached) = state.podcast_chapters_cache.get(key)
            {
                match cached {
                    Some(list) => {
                        state.active_podcast_chapters = list.clone();
                        return (
                            list.clone(),
                            state.settings.language,
                            audiobook_position_ms_from_state(state),
                        );
                    }
                    None => {
                        state.active_podcast_chapters.clear();
                        return (
                            Vec::new(),
                            state.settings.language,
                            audiobook_position_ms_from_state(state),
                        );
                    }
                }
            }
            state.active_podcast_chapters.clear();
            (
                Vec::new(),
                state.settings.language,
                audiobook_position_ms_from_state(state),
            )
        })
        .unwrap_or((Vec::new(), Language::default(), None))
    };
    unsafe {
        if !chapters.is_empty() {
            if SetTimer(hwnd, CHAPTER_ANNOUNCE_TIMER_ID, 500, None) == 0 {
                crate::log_debug("Failed to set CHAPTER_ANNOUNCE_TIMER");
            }
            announce_current_chapter_on_start(hwnd, &chapters, current_pos_ms, language);
        } else {
            kill_timer_best_effort(
                hwnd,
                CHAPTER_ANNOUNCE_TIMER_ID,
                "KillTimer CHAPTER_ANNOUNCE",
            );
        }
        crate::menu::update_playback_menu(hwnd, true);
    }
}

pub(crate) fn set_active_podcast_episode_info(
    hwnd: HWND,
    url: Option<String>,
    media_url: Option<String>,
    podcast_title: Option<String>,
    title: Option<String>,
    cache_path: Option<PathBuf>,
) {
    if let Some(url_value) = url {
        {
            let has_pending_chapters =
                with_state(hwnd, |state| state.pending_podcast_chapters_key.is_some())
                    .unwrap_or(false);
            if with_state(hwnd, |state| {
                state.active_podcast_episode_url = Some(url_value.clone());
                state.active_podcast_episode_media_url = media_url.clone();
                state.active_podcast_title = podcast_title;
                state.active_podcast_episode_title = title;
                state.active_podcast_episode_cache = cache_path;
            })
            .is_none()
            {
                crate::log_debug("Failed to set active podcast episode info");
            }

            if !has_pending_chapters {
                let chapters_url = extract_embedded_chapters_url(&url_value)
                    .or_else(|| extract_buzzsprout_chapters_url(&url_value));
                if let Some(chapters_url) = chapters_url {
                    let key = format!("url_chapters:{url_value}");
                    set_pending_podcast_chapters_key(hwnd, Some(key.clone()));
                    prefetch_podcast_chapters(hwnd, key, chapters_url);
                    activate_pending_podcast_chapters(hwnd);
                }
            }
        }
    }
}

pub(crate) fn set_active_youtube_return_context(
    hwnd: HWND,
    input: Option<String>,
    collection_page: Option<usize>,
) {
    crate::log_debug(&format!(
        "set_active_youtube_return_context: input={} page={:?}",
        input.as_deref().unwrap_or(""),
        collection_page
    ));
    if with_state(hwnd, |state| {
        state.active_youtube_return_context.input = input;
        state.active_youtube_return_context.collection_page = collection_page;
        state.active_youtube_return_context.selected_label = None;
        state.active_youtube_return_context.previous_input = None;
        state.active_youtube_return_context.previous_collection_page = None;
        state.active_youtube_return_context.previous_selected_label = None;
    })
    .is_none()
    {
        crate::log_debug("Failed to set active YouTube return context");
    }
}

pub(crate) fn clear_active_youtube_return_context(hwnd: HWND) {
    crate::log_debug("clear_active_youtube_return_context");
    if with_state(hwnd, |state| {
        state.active_youtube_return_context = YouTubeReturnContext::default();
    })
    .is_none()
    {
        crate::log_debug("Failed to clear active YouTube return context");
    }
}

pub(crate) fn download_active_podcast_episode(hwnd: HWND) {
    let (
        url,
        media_url,
        podcast_title,
        title,
        cache_path,
        language,
        rai_origin,
        uses_video_save_dialog,
    ) = {
        with_state(hwnd, |state| {
            let rai_origin = state.active_podcast_episode_from_rai;
            (
                state.active_podcast_episode_url.clone(),
                state.active_podcast_episode_media_url.clone(),
                state.active_podcast_title.clone(),
                state.active_podcast_episode_title.clone(),
                state.active_podcast_episode_cache.clone(),
                state.settings.language,
                rai_origin,
                matches!(
                    rai_origin,
                    RaiAudioOrigin::RaiPlay | RaiAudioOrigin::La7Play
                ) && state.raiplay_live_audio_variants.is_empty(),
            )
        })
        .unwrap_or((
            None,
            None,
            None,
            None,
            None,
            Language::default(),
            RaiAudioOrigin::None,
            false,
        ))
    };
    // I contenuti RaiPlay e La Sette Play devono passare dal selettore MP3/MP4.
    // Un HLS La7, altrimenti, verrebbe intercettato come streaming audio generico
    // e proporrebbe soltanto il salvataggio audio.
    if !uses_video_save_dialog
        && let Some(active_url) = url.as_deref()
        && app_windows::youtube_transcript_window::download_active_streaming_audio_media(
            hwnd, active_url, language,
        )
    {
        return;
    }
    let fallback_cache_path =
        current_playback_media_path(hwnd).filter(|path| is_local_cached_media_path(path));
    if uses_video_save_dialog {
        download_podcast_episode_with_progress(PodcastProgressDownloadRequest {
            hwnd,
            url,
            media_url,
            podcast_title,
            title,
            cache_path: cache_path.or(fallback_cache_path),
            language,
            rai_origin,
            open_audio_description: false,
            raiplay_context_audio_description: false,
            la7_context_audio_description: false,
            rai_audio_context_save: false,
            forced_save_mode: None,
        });
        return;
    }
    download_podcast_episode(
        hwnd,
        url,
        podcast_title,
        title,
        cache_path.or(fallback_cache_path),
        language,
    );
}

fn download_active_rai_la7_for_audio_description(hwnd: HWND) -> bool {
    let (url, media_url, podcast_title, title, cache_path, language, rai_origin, is_live) =
        with_state(hwnd, |state| {
            (
                state.active_podcast_episode_url.clone(),
                state.active_podcast_episode_media_url.clone(),
                state.active_podcast_title.clone(),
                state.active_podcast_episode_title.clone(),
                state.active_podcast_episode_cache.clone(),
                state.settings.language,
                state.active_podcast_episode_from_rai,
                state.active_mpv_is_live_tv || !state.raiplay_live_audio_variants.is_empty(),
            )
        })
        .unwrap_or((
            None,
            None,
            None,
            None,
            None,
            Language::default(),
            RaiAudioOrigin::None,
            false,
        ));

    if is_live
        || !matches!(
            rai_origin,
            RaiAudioOrigin::RaiPlay | RaiAudioOrigin::La7Play
        )
    {
        return false;
    }

    pause_active_playback_for_audio_description(hwnd);
    log_debug(&format!(
        "Audio description shortcut: saving {:?} video with existing media exporter",
        rai_origin
    ));
    download_podcast_episode_with_progress(PodcastProgressDownloadRequest {
        hwnd,
        url,
        media_url,
        podcast_title,
        title,
        cache_path,
        language,
        rai_origin,
        open_audio_description: true,
        raiplay_context_audio_description: false,
        la7_context_audio_description: false,
        rai_audio_context_save: false,
        forced_save_mode: None,
    });
    true
}

#[derive(Clone, Copy)]
enum RaiPlaySaveMode {
    Mp3,
    Mp4,
    Mp4Described,
}

struct PodcastProgressDownloadRequest {
    hwnd: HWND,
    url: Option<String>,
    media_url: Option<String>,
    podcast_title: Option<String>,
    title: Option<String>,
    cache_path: Option<PathBuf>,
    language: Language,
    rai_origin: RaiAudioOrigin,
    open_audio_description: bool,
    raiplay_context_audio_description: bool,
    la7_context_audio_description: bool,
    rai_audio_context_save: bool,
    forced_save_mode: Option<RaiPlaySaveMode>,
}

fn run_raiplay_context_media_action(
    hwnd: HWND,
    language: Language,
    playback_target: crate::tools::raiplay::PlaybackTarget,
    podcast_title: Option<String>,
    title: Option<String>,
    open_audio_description: bool,
) {
    let (url, media_url) = match playback_target {
        crate::tools::raiplay::PlaybackTarget::DirectStream {
            url,
            media_url,
            is_live,
            ..
        } => {
            if is_live {
                return;
            }
            (Some(url), Some(media_url))
        }
        crate::tools::raiplay::PlaybackTarget::Download(url) => (Some(url), None),
    };
    download_podcast_episode_with_progress(PodcastProgressDownloadRequest {
        hwnd,
        url,
        media_url,
        podcast_title,
        title,
        cache_path: None,
        language,
        rai_origin: RaiAudioOrigin::RaiPlay,
        open_audio_description,
        raiplay_context_audio_description: open_audio_description,
        la7_context_audio_description: false,
        rai_audio_context_save: false,
        forced_save_mode: None,
    });
}

pub(crate) fn save_raiplay_context_media(
    hwnd: HWND,
    language: Language,
    playback_target: crate::tools::raiplay::PlaybackTarget,
    podcast_title: Option<String>,
    title: Option<String>,
) {
    run_raiplay_context_media_action(hwnd, language, playback_target, podcast_title, title, false);
}

pub(crate) fn create_audio_description_from_raiplay_context(
    hwnd: HWND,
    language: Language,
    playback_target: crate::tools::raiplay::PlaybackTarget,
    podcast_title: Option<String>,
    title: Option<String>,
) {
    run_raiplay_context_media_action(hwnd, language, playback_target, podcast_title, title, true);
}

fn run_la7_context_media_action(
    hwnd: HWND,
    language: Language,
    media_url: String,
    podcast_title: Option<String>,
    title: Option<String>,
    open_audio_description: bool,
) {
    download_podcast_episode_with_progress(PodcastProgressDownloadRequest {
        hwnd,
        url: Some(media_url.clone()),
        media_url: Some(media_url),
        podcast_title,
        title,
        cache_path: None,
        language,
        rai_origin: RaiAudioOrigin::La7Play,
        open_audio_description,
        raiplay_context_audio_description: false,
        la7_context_audio_description: open_audio_description,
        rai_audio_context_save: false,
        forced_save_mode: None,
    });
}

pub(crate) fn save_la7_context_media(
    hwnd: HWND,
    language: Language,
    media_url: String,
    podcast_title: Option<String>,
    title: Option<String>,
) {
    run_la7_context_media_action(hwnd, language, media_url, podcast_title, title, false);
}

pub(crate) fn create_audio_description_from_la7_context(
    hwnd: HWND,
    language: Language,
    media_url: String,
    podcast_title: Option<String>,
    title: Option<String>,
) {
    run_la7_context_media_action(hwnd, language, media_url, podcast_title, title, true);
}

pub(crate) fn save_rai_audio_description_context_media(
    hwnd: HWND,
    language: Language,
    media_url: String,
    title: Option<String>,
) {
    download_podcast_episode_with_progress(PodcastProgressDownloadRequest {
        hwnd,
        url: Some(media_url),
        media_url: None,
        podcast_title: None,
        title,
        cache_path: None,
        language,
        rai_origin: RaiAudioOrigin::Recenti,
        open_audio_description: false,
        raiplay_context_audio_description: false,
        la7_context_audio_description: false,
        rai_audio_context_save: true,
        forced_save_mode: Some(RaiPlaySaveMode::Mp3),
    });
}

fn download_podcast_episode_with_progress(request: PodcastProgressDownloadRequest) {
    let PodcastProgressDownloadRequest {
        hwnd,
        url,
        media_url,
        podcast_title,
        title,
        cache_path,
        language,
        rai_origin,
        open_audio_description,
        raiplay_context_audio_description,
        la7_context_audio_description,
        rai_audio_context_save,
        forced_save_mode,
    } = request;
    let suggested_name =
        suggested_podcast_episode_filename(podcast_title.as_deref(), title.as_deref())
            .or_else(|| {
                cache_path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .and_then(|s| s.to_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "podcast_episode".to_string());
    let described_audio_url = url.as_deref().and_then(|value| {
        crate::tools::raiplay::resolve_playback_target(value)
            .ok()
            .and_then(|target| match target {
                crate::tools::raiplay::PlaybackTarget::DirectStream {
                    url: audio_only_url,
                    media_url,
                    ..
                } => (audio_only_url != media_url).then_some(audio_only_url),
                crate::tools::raiplay::PlaybackTarget::Download(_) => None,
            })
    });
    let save_mode = if let Some(save_mode) = forced_save_mode {
        save_mode
    } else if open_audio_description {
        RaiPlaySaveMode::Mp4
    } else {
        let Some(save_mode) =
            choose_rai_episode_save_mode(hwnd, language, rai_origin, described_audio_url.is_some())
        else {
            return;
        };
        save_mode
    };
    let ext = match save_mode {
        RaiPlaySaveMode::Mp3 => "mp3",
        RaiPlaySaveMode::Mp4 | RaiPlaySaveMode::Mp4Described => "mp4",
    };
    let target = if open_audio_description {
        let target_dir = with_state(hwnd, |state| state.settings.media_save_folder.clone())
            .map(|folder| folder.trim().to_string())
            .filter(|folder| !folder.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(settings::default_media_save_folder()));
        if let Err(err) = std::fs::create_dir_all(&target_dir) {
            show_error(
                hwnd,
                language,
                &i18n::tr_f(
                    language,
                    "stream_audio.download_failed",
                    &[("err", &err.to_string())],
                ),
            );
            return;
        }
        app_windows::youtube_transcript_window::unique_stream_media_path(
            &target_dir,
            &suggested_name,
            ext,
        )
    } else {
        let suggested_full = format!("{}.{}", suggested_name, ext);
        let Some(target) = save_podcast_episode_dialog(hwnd, language, &suggested_full) else {
            return;
        };
        replace_path_extension(target, ext)
    };
    let stream_source_url = match save_mode {
        RaiPlaySaveMode::Mp3 => described_audio_url.clone().or_else(|| url.clone()),
        RaiPlaySaveMode::Mp4 | RaiPlaySaveMode::Mp4Described => media_url.or(url.clone()),
    };
    let Some(stream_url) = stream_source_url
        .as_deref()
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
        .map(ToOwned::to_owned)
    else {
        if open_audio_description {
            show_error(
                hwnd,
                language,
                &i18n::tr(language, "stream_audio.no_output"),
            );
            return;
        }
        download_podcast_episode(hwnd, url, podcast_title, title, cache_path, language);
        return;
    };
    let convert_settings = match convert_settings_for_save_target(&target) {
        Ok(settings) => settings,
        Err(err) => {
            let body = i18n::tr_f(language, "podcasts.save_error_body", &[("err", &err)]);
            let title = i18n::tr(language, "podcasts.save_error_title");
            let body_w = to_wide(&body);
            let title_w = to_wide(&title);
            message_box_modal(
                hwnd,
                PCWSTR(body_w.as_ptr()),
                PCWSTR(title_w.as_ptr()),
                MB_OK | MB_ICONERROR,
            );
            return;
        }
    };
    let selected_audio_track = with_state(hwnd, |state| state.selected_audio_track).flatten();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    with_state(hwnd, |state| {
        state.podcast_save_cancel_token = Some(cancel_flag.clone());
    });
    if raiplay_context_audio_description {
        app_windows::raiplay_window::mark_context_audio_description_started(hwnd);
    }
    if la7_context_audio_description {
        app_windows::la7_play_window::mark_context_audio_description_started(hwnd);
    }
    if rai_audio_context_save {
        app_windows::rai_audiodescrizioni_window::mark_context_save_started(hwnd);
    }
    open_podcast_save_progress_window(hwnd, language);
    if (!open_audio_description
        || raiplay_context_audio_description
        || la7_context_audio_description)
        && app_windows::youtube_transcript_window::has_active_media_context_list(hwnd)
    {
        let progress_dialog =
            with_state(hwnd, |state| state.podcast_save_window).unwrap_or(HWND(0));
        app_windows::podcast_save_window::suppress_parent_restore_on_close(progress_dialog);
    }
    update_podcast_save_progress_window(hwnd, 0);
    let hwnd_copy = hwnd;
    std::thread::spawn(move || {
        let input_path = PathBuf::from(&stream_url);
        let mut progress_callback = |pct: u32| {
            update_podcast_save_progress_window(hwnd_copy, normalize_ffmpeg_progress_pct(pct));
        };
        let result = match save_mode {
            RaiPlaySaveMode::Mp4 => {
                crate::ffmpeg_export::remux_media_file_to_mp4_with_preferred_audio_stream(
                    &input_path,
                    &target,
                    selected_audio_track,
                    Some(cancel_flag.clone()),
                    Some(&mut progress_callback),
                )
            }
            RaiPlaySaveMode::Mp4Described => {
                if let Some(audio_url) = described_audio_url {
                    log_debug(&format!(
                        "RaiPlay save: muxing MP4 with described audio video={} audio={}",
                        input_path.display(),
                        audio_url
                    ));
                    crate::ffmpeg_export::remux_media_file_to_mp4_with_external_audio_stream(
                        &input_path,
                        &PathBuf::from(audio_url),
                        &target,
                        Some(cancel_flag.clone()),
                        Some(&mut progress_callback),
                    )
                } else {
                    Err("RaiPlay described audio stream not available.".to_string())
                }
            }
            RaiPlaySaveMode::Mp3 => crate::ffmpeg_export::convert_audio_file_with_preferred_stream(
                &input_path,
                &target,
                &convert_settings,
                Some(cancel_flag.clone()),
                Some(&mut progress_callback),
                selected_audio_track,
            ),
        };
        match result {
            Ok(()) => {
                if matches!(
                    save_mode,
                    RaiPlaySaveMode::Mp4 | RaiPlaySaveMode::Mp4Described
                ) {
                    update_podcast_save_progress_window(hwnd_copy, 100);
                }
                post_podcast_episode_save_result(
                    hwnd_copy,
                    PodcastEpisodeSaveResult {
                        language,
                        target_path: target,
                        error: None,
                        open_audio_description,
                    },
                )
            }
            Err(err) => post_podcast_episode_save_result(
                hwnd_copy,
                PodcastEpisodeSaveResult {
                    language,
                    target_path: target.clone(),
                    error: Some(format!("stream export failed: {err}")),
                    open_audio_description,
                },
            ),
        }
    });
}

fn post_podcast_episode_save_result(hwnd: HWND, payload: PodcastEpisodeSaveResult) {
    let ptr = Box::into_raw(Box::new(payload));
    unsafe {
        if let Err(err) = PostMessageW(
            hwnd,
            WM_PODCAST_EPISODE_SAVE_RESULT,
            WPARAM(0),
            LPARAM(ptr as isize),
        ) {
            log_debug(&format!(
                "Failed to post WM_PODCAST_EPISODE_SAVE_RESULT: {}",
                err
            ));
            let _drop_payload = Box::from_raw(ptr);
        }
    }
}

pub(crate) fn post_media_save_result(
    hwnd: HWND,
    language: Language,
    target_path: PathBuf,
    error: Option<String>,
) {
    post_podcast_episode_save_result(
        hwnd,
        PodcastEpisodeSaveResult {
            language,
            target_path,
            error,
            open_audio_description: false,
        },
    );
}

fn post_podcast_episode_play_ready(hwnd: HWND, payload: PodcastEpisodePlayReady) {
    let ptr = Box::into_raw(Box::new(payload));
    unsafe {
        if let Err(err) = PostMessageW(
            hwnd,
            WM_PODCAST_EPISODE_PLAY_READY,
            WPARAM(0),
            LPARAM(ptr as isize),
        ) {
            log_debug(&format!(
                "Failed to post WM_PODCAST_EPISODE_PLAY_READY: {}",
                err
            ));
            let _drop_payload = Box::from_raw(ptr);
        }
    }
}

fn post_podcast_episode_play_failed(hwnd: HWND, payload: PodcastEpisodePlayFailed) {
    let ptr = Box::into_raw(Box::new(payload));
    unsafe {
        if let Err(err) = PostMessageW(
            hwnd,
            WM_PODCAST_EPISODE_PLAY_FAILED,
            WPARAM(0),
            LPARAM(ptr as isize),
        ) {
            log_debug(&format!(
                "Failed to post WM_PODCAST_EPISODE_PLAY_FAILED: {}",
                err
            ));
            let _drop_payload = Box::from_raw(ptr);
        }
    }
}

fn podcast_partial_cache_path(file_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.part", file_path.to_string_lossy()))
}

fn is_usable_podcast_cache_file(path: &Path) -> bool {
    const MIN_MEDIA_BYTES: u64 = 1024;
    let Ok(meta) = path.metadata() else {
        return false;
    };
    if !meta.is_file() || meta.len() < MIN_MEDIA_BYTES {
        return false;
    }

    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 512];
    let read = std::io::Read::read(&mut file, &mut header).unwrap_or(0);
    if read == 0 {
        return false;
    }
    let prefix = String::from_utf8_lossy(&header[..read]);
    let prefix = prefix.trim_start().to_ascii_lowercase();
    !prefix.starts_with("<!doctype")
        && !prefix.starts_with("<html")
        && !prefix.starts_with("<?xml")
        && !prefix.starts_with("{\"error\"")
}

fn podcast_cache_path_for_url(url: &str, mime: Option<&str>, title: Option<&str>) -> PathBuf {
    use sha2::Digest;

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

    if ext.len() > 5 || ext.is_empty() {
        ext = "mp3";
    }

    if let Some(title) = title.and_then(suggested_filename_from_text) {
        return settings::settings_dir()
            .join("podcast cache")
            .join(format!("{title}.{ext}"));
    }

    settings::settings_dir()
        .join("podcast cache")
        .join(format!("podcast_{}.{}", &hash[..16], ext))
}

pub(crate) fn play_named_remote_audio_from_url_with_rai_origin(
    hwnd: HWND,
    url: String,
    title: Option<String>,
    mime: Option<&str>,
    rai_origin: RaiAudioOrigin,
) {
    with_state(hwnd, |state| {
        state.raiplay_live_audio_variants.clear();
    });
    play_podcast_episode_from_url_internal(hwnd, url, None, title, mime, true, rai_origin);
}

pub(crate) fn play_live_stream_audio_from_url_with_rai_origin(
    hwnd: HWND,
    url: String,
    podcast_title: Option<String>,
    title: Option<String>,
    live_audio_variants: Vec<crate::tools::raiplay::LiveAudioTrack>,
    rai_origin: RaiAudioOrigin,
) {
    let stream_path = PathBuf::from(&url);
    queue_audio_files_and_play(hwnd, vec![stream_path.clone()]);
    if let Some(display_title) =
        podcast_episode_display_title(podcast_title.as_deref(), title.as_deref())
    {
        editor_manager::set_current_document_title(hwnd, &display_title);
    }
    if with_state(hwnd, |state| {
        state.active_podcast_episode_from_rai = rai_origin;
        state.raiplay_live_audio_variants = live_audio_variants
            .iter()
            .map(|variant| RaiPlayLiveAudioVariant {
                track: variant.info.clone(),
                url: variant.url.clone(),
            })
            .collect();
        state.available_audio_tracks = live_audio_variants
            .iter()
            .map(|variant| variant.info.clone())
            .collect();
        state.selected_audio_track = live_audio_variants
            .iter()
            .find(|variant| variant.url == url)
            .map(|variant| variant.info.index);
    })
    .is_none()
    {
        log_debug("Failed to set active podcast Rai origin flag for live stream");
    }
    editor_manager::mark_current_document_from_rss(hwnd, true);
    set_active_podcast_episode_info(hwnd, Some(url), None, podcast_title, title, None);
    menu::update_playback_menu(hwnd, true);
    activate_pending_podcast_chapters(hwnd);
}

const MPV_RUNTIME_URL: &str =
    "https://github.com/Ambro86/Sonarpad-Tools/releases/download/0.7/mpv.zip";
const WINDOWS_ERROR_BAD_EXE_FORMAT: i32 = 193;

fn mpv_runtime_dir() -> PathBuf {
    settings::settings_dir().join("mpv")
}

fn mpv_runtime_executable_path() -> PathBuf {
    mpv_runtime_dir().join("mpv.exe")
}

fn find_mpv_executable_in_tree(root: &Path) -> Option<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir).ok()?;
        for entry in entries {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("mpv.exe"))
            {
                return Some(path);
            }
        }
    }
    None
}

fn download_and_extract_mpv_runtime(
    hwnd: HWND,
    language: Language,
    target_dir: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(target_dir)
        .map_err(|err| format!("Impossibile creare la cartella di mpv: {err}"))?;
    let zip_path = settings::settings_dir().join("mpv.zip.download");
    log_debug(&format!(
        "Downloading mpv runtime from {} to {}",
        MPV_RUNTIME_URL,
        zip_path.display()
    ));
    let client = reqwest::blocking::Client::builder()
        .build()
        .map_err(|err| format!("Impossibile inizializzare il download di mpv: {err}"))?;
    let mut response = client
        .get(MPV_RUNTIME_URL)
        .send()
        .map_err(|err| format!("Impossibile scaricare mpv: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Download di mpv non riuscito: {err}"))?;
    let total_bytes = response.content_length();
    open_podcast_save_progress_window(hwnd, language);
    update_podcast_save_progress_window(hwnd, 0);
    {
        let mut file = std::fs::File::create(&zip_path)
            .map_err(|err| format!("Impossibile creare l'archivio temporaneo di mpv: {err}"))?;
        let mut buffer = [0u8; 64 * 1024];
        let mut downloaded = 0u64;
        loop {
            let read = response
                .read(&mut buffer)
                .map_err(|err| format!("Impossibile salvare l'archivio di mpv: {err}"))?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read])
                .map_err(|err| format!("Impossibile salvare l'archivio di mpv: {err}"))?;
            downloaded = downloaded.saturating_add(read as u64);
            if let Some(total_bytes) = total_bytes.filter(|value| *value > 0) {
                let pct = ((downloaded.saturating_mul(90)) / total_bytes).min(90) as u32;
                update_podcast_save_progress_window(hwnd, pct);
            }
        }
        file.flush()
            .map_err(|err| format!("Impossibile finalizzare l'archivio di mpv: {err}"))?;
    }
    update_podcast_save_progress_window(hwnd, 92);

    let extract_result = (|| -> Result<(), String> {
        let file = std::fs::File::open(&zip_path)
            .map_err(|err| format!("Impossibile aprire l'archivio di mpv: {err}"))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|err| format!("Archivio mpv non valido: {err}"))?;
        let entry_count = archive.len().max(1);
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|err| format!("Impossibile leggere l'archivio di mpv: {err}"))?;
            let relative_path = entry
                .enclosed_name()
                .map(Path::to_path_buf)
                .ok_or_else(|| {
                    format!(
                        "Archivio mpv contiene un percorso non valido: {}",
                        entry.name()
                    )
                })?;
            let output_path = target_dir.join(relative_path);
            if entry.is_dir() {
                std::fs::create_dir_all(&output_path).map_err(|err| {
                    format!(
                        "Impossibile creare la cartella estratta di mpv {}: {err}",
                        output_path.display()
                    )
                })?;
                continue;
            }
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(|err| {
                    format!(
                        "Impossibile creare la cartella estratta di mpv {}: {err}",
                        parent.display()
                    )
                })?;
            }
            let mut output = std::fs::File::create(&output_path).map_err(|err| {
                format!(
                    "Impossibile creare il file estratto di mpv {}: {err}",
                    output_path.display()
                )
            })?;
            std::io::copy(&mut entry, &mut output).map_err(|err| {
                format!(
                    "Impossibile estrarre il file di mpv {}: {err}",
                    output_path.display()
                )
            })?;
            output.flush().map_err(|err| {
                format!(
                    "Impossibile finalizzare il file estratto di mpv {}: {err}",
                    output_path.display()
                )
            })?;
            let extract_pct = 92 + (((index + 1) * 8) / entry_count) as u32;
            update_podcast_save_progress_window(hwnd, extract_pct.min(100));
        }
        Ok(())
    })();

    if let Err(err) = std::fs::remove_file(&zip_path) {
        log_debug(&format!(
            "Temporary mpv archive cleanup failed for {}: {}",
            zip_path.display(),
            err
        ));
    }

    extract_result
}

pub(crate) fn installed_mpv_runtime_executable() -> Result<PathBuf, String> {
    let preferred_path = mpv_runtime_executable_path();
    if preferred_path.is_file() {
        return Ok(preferred_path);
    }

    let runtime_dir = mpv_runtime_dir();
    find_mpv_executable_in_tree(&runtime_dir).ok_or_else(|| {
        format!(
            "mpv non è installato nella cartella {}.",
            runtime_dir.display()
        )
    })
}

pub(crate) fn ensure_mpv_runtime_available(hwnd: HWND) -> Result<PathBuf, String> {
    if let Ok(path) = installed_mpv_runtime_executable() {
        return Ok(path);
    }

    let runtime_dir = mpv_runtime_dir();
    let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
    let result = download_and_extract_mpv_runtime(hwnd, language, &runtime_dir);
    close_podcast_save_progress_window(hwnd);
    result?;

    installed_mpv_runtime_executable().map_err(|_| {
        format!(
            "mpv scaricato ma mpv.exe non trovato in {}.",
            runtime_dir.display()
        )
    })
}

fn is_bad_executable_format_error(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(WINDOWS_ERROR_BAD_EXE_FORMAT)
}

fn reinstall_managed_mpv_runtime(hwnd: HWND, failed_executable: &Path) -> Result<PathBuf, String> {
    let runtime_dir = mpv_runtime_dir();
    if !failed_executable.starts_with(&runtime_dir) {
        return Err(format!(
            "Il file mpv non valido non appartiene alla cartella gestita da Sonarpad: {}.",
            failed_executable.display()
        ));
    }

    log_debug(&format!(
        "mpv executable has invalid Win32 format; reinstalling managed runtime from {} (path={})",
        MPV_RUNTIME_URL,
        failed_executable.display()
    ));
    if runtime_dir.exists() {
        std::fs::remove_dir_all(&runtime_dir).map_err(|err| {
            format!(
                "Impossibile rimuovere la copia danneggiata di mpv da {}: {err}",
                runtime_dir.display()
            )
        })?;
    }

    let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
    let result = download_and_extract_mpv_runtime(hwnd, language, &runtime_dir);
    close_podcast_save_progress_window(hwnd);
    result?;
    installed_mpv_runtime_executable().map_err(|_| {
        format!(
            "mpv riscaricato ma mpv.exe non trovato in {}.",
            runtime_dir.display()
        )
    })
}

fn spawn_mpv_with_runtime_repair(
    hwnd: HWND,
    mpv_executable: &Path,
    command: &mut std::process::Command,
) -> Result<std::process::Child, String> {
    match command.spawn() {
        Ok(child) => Ok(child),
        Err(error) if is_bad_executable_format_error(&error) => {
            log_debug(&format!(
                "mpv launch failed with Windows error {}: {}; starting automatic runtime repair",
                WINDOWS_ERROR_BAD_EXE_FORMAT, error
            ));
            reinstall_managed_mpv_runtime(hwnd, mpv_executable)?;
            command.spawn().map_err(|retry_error| {
                format!("Impossibile avviare mpv dopo il ripristino automatico: {retry_error}")
            })
        }
        Err(error) => Err(format!("Impossibile avviare mpv: {error}")),
    }
}

fn youtube_mpv_log_has_failure(log: &str) -> Option<String> {
    let lower = log.to_ascii_lowercase();
    let last_success = ["starting playback", "playback restart complete"]
        .into_iter()
        .filter_map(|marker| lower.rfind(marker))
        .max();
    let semantic_groups: &[(&str, &[&str])] = &[
        (
            "members-only",
            &[
                "members-only",
                "members only",
                "join this channel",
                "available to this channel's members",
                "available to members of this channel",
            ],
        ),
        (
            "widevine drm",
            &[
                "widevine",
                "this video is drm protected",
                "uses drm protection",
                "known to use drm protection",
            ],
        ),
        (
            "login required",
            &[
                "sign in to confirm",
                "login required",
                "use --username and --password",
            ],
        ),
    ];
    // A later transport error (often 403) must not hide YouTube's explicit
    // access reason. These semantic errors therefore outrank generic failures.
    for (detail, markers) in semantic_groups {
        let position = markers
            .iter()
            .filter_map(|marker| lower.rfind(marker))
            .max();
        if position.is_some_and(|position| last_success.is_none_or(|success| position > success)) {
            return Some((*detail).to_string());
        }
    }
    let groups: &[(&str, &[&str])] = &[
        ("youtube-dl failed", &["youtube-dl failed"]),
        ("no video formats found", &["no video formats found"]),
        (
            "requested format is not available",
            &["requested format is not available"],
        ),
        ("this video is unavailable", &["this video is unavailable"]),
        ("http error 403", &["http error 403"]),
        (
            "failed to recognize file format",
            &[
                "failed to recognize file format",
                "unable to recognize file format",
            ],
        ),
    ];
    let mut last_failure: Option<(usize, &str)> = None;
    for (detail, markers) in groups {
        for marker in *markers {
            if let Some(position) = lower.rfind(marker)
                && last_failure.is_none_or(|(current, _)| position > current)
            {
                last_failure = Some((position, detail));
            }
        }
    }
    last_failure
        .filter(|(position, _)| last_success.is_none_or(|success| *position > success))
        .map(|(_, detail)| detail.to_string())
}

fn monitor_youtube_mpv_startup(hwnd: HWND, process_id: u32, log_path: PathBuf) {
    std::thread::spawn(move || {
        // IPC has already confirmed an audio track. Keep a short asynchronous
        // watch for an immediate end-file/HTTP failure after file-loaded.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let log = std::fs::read_to_string(&log_path).unwrap_or_default();
            if let Some(detail) = youtube_mpv_log_has_failure(&log) {
                log_debug(&format!(
                    "Managed mpv YouTube end-file failure: pid={process_id} detail={detail}"
                ));
                let payload = Box::new(YoutubeMpvPlaybackFailure { process_id, detail });
                let raw = Box::into_raw(payload);
                unsafe {
                    if PostMessageW(
                        hwnd,
                        WM_YOUTUBE_MPV_PLAYBACK_FAILED,
                        WPARAM(0),
                        LPARAM(raw as isize),
                    )
                    .is_err()
                    {
                        let _reclaimed_payload = Box::from_raw(raw);
                    }
                }
                if let Err(error) = std::fs::remove_file(&log_path) {
                    log_debug(&format!(
                        "Managed mpv YouTube log cleanup failed: path={} error={error}",
                        log_path.display()
                    ));
                }
                return;
            }
            if Instant::now() >= deadline {
                log_debug(&format!(
                    "Managed mpv YouTube startup remained stable: pid={process_id}"
                ));
                if let Err(error) = std::fs::remove_file(&log_path) {
                    log_debug(&format!(
                        "Managed mpv YouTube log cleanup failed: path={} error={error}",
                        log_path.display()
                    ));
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
}

fn wait_for_youtube_mpv_file_loaded(
    child: &mut std::process::Child,
    ipc_path: &Path,
    log_path: Option<&Path>,
) -> Result<(), String> {
    for attempt in 0..150_u64 {
        if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
            let detail = log_path
                .and_then(|path| std::fs::read_to_string(path).ok())
                .and_then(|log| youtube_mpv_log_has_failure(&log))
                .unwrap_or_else(|| format!("mpv terminato prima di file-loaded: {status}"));
            return Err(detail);
        }
        if let Ok(mut pipe) = open_mpv_ipc_pipe(ipc_path)
            && let Ok(response) = send_mpv_ipc_request_with_id(
                ipc_path,
                &mut pipe,
                r#"{"command":["get_property","track-list"]}"#,
                700_000 + attempt,
            )
        {
            let has_audio = response
                .get("data")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|tracks| {
                    tracks.iter().any(|track| {
                        track.get("type").and_then(serde_json::Value::as_str) == Some("audio")
                    })
                });
            if has_audio {
                log_debug(&format!(
                    "Managed mpv YouTube IPC file-loaded confirmed: pid={} attempt={}",
                    child.id(),
                    attempt + 1
                ));
                return Ok(());
            }
        }
        if let Some(detail) = log_path
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|log| youtube_mpv_log_has_failure(&log))
        {
            return Err(detail);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("file-loaded timeout: mpv non ha caricato alcuna traccia audio".to_string())
}

fn clear_managed_mpv_state(hwnd: HWND) {
    if with_state(hwnd, |state| {
        state.active_mpv_session = None;
        state.active_mpv_is_live_tv = false;
        state.active_mpv_ipc = None;
        state.active_mpv_subtitle_generation = state.active_mpv_subtitle_generation.wrapping_add(1);
        state.next_mpv_request_id = 1;
        state.active_podcast_episode_url = None;
        state.active_podcast_episode_media_url = None;
        state.active_podcast_title = None;
        state.active_podcast_episode_title = None;
        state.active_podcast_episode_cache = None;
        state.active_podcast_episode_from_rai = RaiAudioOrigin::None;
        state.raiplay_live_audio_variants.clear();
        state.active_mpv_status = None;
        state.available_audio_tracks.clear();
        state.selected_audio_track = None;
    })
    .is_none()
    {
        log_debug("Failed to clear managed mpv state");
    }
    menu::update_playback_menu(hwnd, false);
}

pub(crate) fn open_mpv_ipc_pipe(ipc_path: &Path) -> Result<std::fs::File, String> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(ipc_path)
        .map_err(|err| format!("Impossibile aprire il canale IPC di mpv: {err}"))
}

fn next_mpv_request_id(hwnd: HWND) -> Result<u64, String> {
    with_state(hwnd, |state| {
        let request_id = state.next_mpv_request_id;
        state.next_mpv_request_id = state.next_mpv_request_id.saturating_add(1);
        request_id
    })
    .ok_or_else(|| "Stato interno di Sonarpad non disponibile.".to_string())
}

fn build_mpv_ipc_message(command_json: &str, request_id: u64) -> Result<String, String> {
    let mut value: serde_json::Value = serde_json::from_str(command_json)
        .map_err(|err| format!("Comando IPC di mpv non valido: {err}"))?;
    let Some(object) = value.as_object_mut() else {
        return Err("Comando IPC di mpv non valido.".to_string());
    };
    object.insert(
        "request_id".to_string(),
        serde_json::Value::Number(serde_json::Number::from(request_id)),
    );
    serde_json::to_string(&value).map_err(|err| format!("Comando IPC di mpv non valido: {err}"))
}

fn read_mpv_ipc_response_with_pipe(
    ipc_path: &Path,
    pipe: &mut std::fs::File,
    request_id: u64,
) -> Result<serde_json::Value, String> {
    use std::io::{BufRead as _, BufReader};

    loop {
        let mut reader = BufReader::new(&mut *pipe);
        let mut response = String::new();
        reader
            .read_line(&mut response)
            .map_err(|err| format!("Impossibile leggere la risposta da mpv: {err}"))?;
        let trimmed = response.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|err| format!("Risposta IPC di mpv non valida: {err}"))?;
        let received_request_id = parsed
            .get("request_id")
            .and_then(|value| value.as_u64())
            .unwrap_or_default();
        if received_request_id != request_id {
            log_debug(&format!(
                "Managed mpv IPC response skipped: pipe={} expected_request_id={} actual_request_id={} response={}",
                ipc_path.display(),
                request_id,
                received_request_id,
                trimmed
            ));
            continue;
        }
        return Ok(parsed);
    }
}

fn send_mpv_ipc_command_with_pipe(
    hwnd: HWND,
    ipc_path: &Path,
    pipe: &mut std::fs::File,
    command_json: &str,
) -> Result<(), String> {
    let request_id = next_mpv_request_id(hwnd)?;
    let message = build_mpv_ipc_message(command_json, request_id)?;
    log_debug(&format!(
        "Managed mpv IPC command send: pipe={} command={}",
        ipc_path.display(),
        message
    ));
    use std::io::Write as _;
    pipe.write_all(message.as_bytes())
        .map_err(|err| format!("Impossibile inviare il comando a mpv: {err}"))?;
    pipe.write_all(b"\n")
        .map_err(|err| format!("Impossibile inviare il comando a mpv: {err}"))?;
    pipe.flush()
        .map_err(|err| format!("Impossibile inviare il comando a mpv: {err}"))?;
    let response = read_mpv_ipc_response_with_pipe(ipc_path, pipe, request_id)?;
    log_debug(&format!(
        "Managed mpv IPC command ok: pipe={} command={} response={}",
        ipc_path.display(),
        message,
        response
    ));
    Ok(())
}

pub(crate) fn send_mpv_ipc_command(ipc_path: &Path, command_json: &str) -> Result<(), String> {
    let mut pipe = open_mpv_ipc_pipe(ipc_path)?;
    let request_id = 1;
    let message = build_mpv_ipc_message(command_json, request_id)?;
    log_debug(&format!(
        "Managed mpv IPC command send: pipe={} command={}",
        ipc_path.display(),
        message
    ));
    use std::io::Write as _;
    pipe.write_all(message.as_bytes())
        .map_err(|err| format!("Impossibile inviare il comando a mpv: {err}"))?;
    pipe.write_all(b"\n")
        .map_err(|err| format!("Impossibile inviare il comando a mpv: {err}"))?;
    pipe.flush()
        .map_err(|err| format!("Impossibile inviare il comando a mpv: {err}"))?;
    let response = read_mpv_ipc_response_with_pipe(ipc_path, &mut pipe, request_id)?;
    log_debug(&format!(
        "Managed mpv IPC command ok: pipe={} command={} response={}",
        ipc_path.display(),
        message,
        response
    ));
    Ok(())
}

fn send_mpv_ipc_request_with_pipe(
    hwnd: HWND,
    ipc_path: &Path,
    pipe: &mut std::fs::File,
    command_json: &str,
) -> Result<serde_json::Value, String> {
    let request_id = next_mpv_request_id(hwnd)?;
    send_mpv_ipc_request_with_id(ipc_path, pipe, command_json, request_id)
}

pub(crate) fn send_mpv_ipc_request_with_id(
    ipc_path: &Path,
    pipe: &mut std::fs::File,
    command_json: &str,
    request_id: u64,
) -> Result<serde_json::Value, String> {
    use std::io::Write as _;

    let message = build_mpv_ipc_message(command_json, request_id)?;
    log_debug(&format!(
        "Managed mpv IPC request send: pipe={} command={}",
        ipc_path.display(),
        message
    ));
    pipe.write_all(message.as_bytes())
        .map_err(|err| format!("Impossibile inviare il comando a mpv: {err}"))?;
    pipe.write_all(b"\n")
        .map_err(|err| format!("Impossibile inviare il comando a mpv: {err}"))?;
    pipe.flush()
        .map_err(|err| format!("Impossibile inviare il comando a mpv: {err}"))?;
    let response = read_mpv_ipc_response_with_pipe(ipc_path, pipe, request_id)?;
    log_debug(&format!(
        "Managed mpv IPC request response: pipe={} command={} response={}",
        ipc_path.display(),
        message,
        response
    ));
    Ok(response)
}

fn is_mpv_ipc_unavailable_error(err: &str) -> bool {
    err.contains("Impossibile aprire il canale IPC di mpv")
        || err.contains("Impossibile trovare il file specificato. (os error 2)")
        || err.contains("Tutte le istanze della pipe sono impegnate. (os error 231)")
}

fn taskkill_mpv_process(process_id: u32, context: &str) {
    if let Err(err) = std::process::Command::new("taskkill")
        .args(["/PID", &process_id.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW_FLAGS)
        .spawn()
    {
        log_debug(&format!("Managed mpv taskkill {} failed: {}", context, err));
    }
}

fn invalidate_managed_mpv_session(hwnd: HWND) {
    let session = with_state(hwnd, |state| state.active_mpv_session.clone()).flatten();
    let (active_url, active_origin) = with_state(hwnd, |state| {
        (
            state.active_podcast_episode_url.clone(),
            state.active_podcast_episode_from_rai,
        )
    })
    .unwrap_or((None, RaiAudioOrigin::None));
    if with_state(hwnd, |state| {
        state.last_stopped_mpv_origin = if active_url.is_some() {
            active_origin
        } else {
            RaiAudioOrigin::None
        };
        state.last_stopped_mpv_url = active_url;
    })
    .is_none()
    {
        log_debug("Failed to persist last stopped mpv url");
    }
    prevent_sleep(false);
    if let Some(session) = session {
        taskkill_mpv_process(session.process_id, "after IPC failure");
    }
    clear_managed_mpv_state(hwnd);
}

fn ensure_managed_mpv_ipc_connected(hwnd: HWND, ipc_path: &Path) -> Result<(), String> {
    if with_state(hwnd, |state| state.active_mpv_ipc.is_some()).unwrap_or(false) {
        return Ok(());
    }
    let pipe = open_mpv_ipc_pipe(ipc_path)?;
    if with_state(hwnd, |state| {
        state.active_mpv_ipc = Some(pipe);
    })
    .is_none()
    {
        return Err("Impossibile salvare la connessione IPC di mpv.".to_string());
    }
    Ok(())
}

fn try_send_command_to_managed_mpv(hwnd: HWND, command_json: &str) -> Result<(), String> {
    let session = with_state(hwnd, |state| state.active_mpv_session.clone())
        .flatten()
        .ok_or_else(|| "Nessuna riproduzione mpv attiva.".to_string())?;
    let result = ensure_managed_mpv_ipc_connected(hwnd, &session.ipc_path).and_then(|()| {
        with_state(hwnd, |state| {
            state
                .active_mpv_ipc
                .as_mut()
                .ok_or_else(|| "Connessione IPC di mpv non disponibile.".to_string())
                .and_then(|pipe| {
                    send_mpv_ipc_command_with_pipe(hwnd, &session.ipc_path, pipe, command_json)
                })
        })
        .unwrap_or_else(|| Err("Stato interno di Sonarpad non disponibile.".to_string()))
    });
    match result {
        Ok(()) => Ok(()),
        Err(err) => {
            log_debug(&format!(
                "Managed mpv IPC command failed: pipe={} command={} err={}",
                session.ipc_path.display(),
                command_json,
                err
            ));
            if is_mpv_ipc_unavailable_error(&err) {
                invalidate_managed_mpv_session(hwnd);
            }
            Err(err)
        }
    }
}

fn query_managed_mpv_property(hwnd: HWND, property: &str) -> Result<serde_json::Value, String> {
    let session = with_state(hwnd, |state| state.active_mpv_session.clone())
        .flatten()
        .ok_or_else(|| "Nessuna riproduzione mpv attiva.".to_string())?;
    let request = format!(r#"{{"command":["get_property","{}"]}}"#, property);
    let response = match ensure_managed_mpv_ipc_connected(hwnd, &session.ipc_path).and_then(|()| {
        with_state(hwnd, |state| {
            state
                .active_mpv_ipc
                .as_mut()
                .ok_or_else(|| "Connessione IPC di mpv non disponibile.".to_string())
                .and_then(|pipe| {
                    send_mpv_ipc_request_with_pipe(hwnd, &session.ipc_path, pipe, &request)
                })
        })
        .unwrap_or_else(|| Err("Stato interno di Sonarpad non disponibile.".to_string()))
    }) {
        Ok(response) => response,
        Err(err) => {
            log_debug(&format!(
                "Managed mpv IPC request failed: pipe={} property={} err={}",
                session.ipc_path.display(),
                property,
                err
            ));
            if is_mpv_ipc_unavailable_error(&err) {
                invalidate_managed_mpv_session(hwnd);
            }
            return Err(err);
        }
    };
    Ok(response
        .get("data")
        .cloned()
        .unwrap_or(serde_json::Value::Null))
}

fn query_managed_mpv_property_transient(
    hwnd: HWND,
    property: &str,
) -> Result<serde_json::Value, String> {
    let session = with_state(hwnd, |state| state.active_mpv_session.clone())
        .flatten()
        .ok_or_else(|| "Nessuna riproduzione mpv attiva.".to_string())?;
    let request = format!(r#"{{"command":["get_property","{}"]}}"#, property);
    let mut pipe = open_mpv_ipc_pipe(&session.ipc_path)?;
    let response = send_mpv_ipc_request_with_id(&session.ipc_path, &mut pipe, &request, 1)?;
    Ok(response
        .get("data")
        .cloned()
        .unwrap_or(serde_json::Value::Null))
}

fn is_raiplay_audiodescription_track(language: Option<&str>, title: Option<&str>) -> bool {
    language
        .map(|value| value.eq_ignore_ascii_case("des"))
        .unwrap_or(false)
        || title
            .map(|value| value.eq_ignore_ascii_case("Audiodescrizione"))
            .unwrap_or(false)
}

fn mpv_track_is_audiodescription(track: &serde_json::Value) -> bool {
    let is_audio = track
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(|value| value.eq_ignore_ascii_case("audio"))
        .unwrap_or(false);
    if !is_audio {
        return false;
    }

    let language = track.get("lang").and_then(serde_json::Value::as_str);
    let title = track
        .get("title")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            track
                .get("metadata")
                .and_then(|metadata| metadata.get("comment"))
                .and_then(serde_json::Value::as_str)
        });
    is_raiplay_audiodescription_track(language, title)
        || track
            .get("visual-impaired")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
}

pub(crate) fn audiodescription_mpv_audio_track_id(track_list: &serde_json::Value) -> Option<i64> {
    track_list.as_array()?.iter().find_map(|track| {
        mpv_track_is_audiodescription(track)
            .then(|| track.get("id").and_then(serde_json::Value::as_i64))
            .flatten()
    })
}

pub(crate) fn preferred_mpv_audio_track_id(track_list: &serde_json::Value) -> Option<i64> {
    let tracks = track_list.as_array()?;

    let find_track_id = |predicate: &dyn Fn(Option<&str>, Option<&str>) -> bool| {
        tracks.iter().find_map(|track| {
            let is_audio = track
                .get("type")
                .and_then(serde_json::Value::as_str)
                .map(|value| value.eq_ignore_ascii_case("audio"))
                .unwrap_or(false);
            if !is_audio {
                return None;
            }
            let language = track.get("lang").and_then(serde_json::Value::as_str);
            let title = track.get("title").and_then(serde_json::Value::as_str);
            predicate(language, title)
                .then(|| track.get("id").and_then(serde_json::Value::as_i64))
                .flatten()
        })
    };

    audiodescription_mpv_audio_track_id(track_list).or_else(|| {
        find_track_id(&|language, _| {
            language
                .map(|value| value.eq_ignore_ascii_case("ita"))
                .unwrap_or(false)
        })
    })
}

pub(crate) fn select_raiplay_mpv_audio_track(hwnd: HWND) {
    for _ in 0..10 {
        let Ok(track_list) = query_managed_mpv_property_transient(hwnd, "track-list") else {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        };
        let Some(track_id) = preferred_mpv_audio_track_id(&track_list) else {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        };
        let command = format!(r#"{{"command":["set_property","aid",{}]}}"#, track_id);
        if let Err(err) = try_send_command_to_managed_mpv_transient(hwnd, &command) {
            log_debug(&format!(
                "Managed mpv: failed to select RaiPlay audio track {}: {}",
                track_id, err
            ));
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }
        log_debug(&format!(
            "Managed mpv: selected RaiPlay preferred audio track {}",
            track_id
        ));
        return;
    }
    log_debug("Managed mpv: preferred RaiPlay audio track not available");
}

fn log_managed_mpv_raiplay_video_snapshot(hwnd: HWND, label: &str) {
    for property in [
        "track-list",
        "vid",
        "video-codec",
        "demuxer-cache-state",
        "stream-open-filename",
    ] {
        match query_managed_mpv_property_transient(hwnd, property) {
            Ok(value) => log_debug(&format!(
                "Managed mpv Rai video snapshot [{label}] {property}={value}"
            )),
            Err(err) => log_debug(&format!(
                "Managed mpv Rai video snapshot [{label}] {property} error: {err}"
            )),
        }
    }
}

fn try_send_command_to_managed_mpv_transient(hwnd: HWND, command_json: &str) -> Result<(), String> {
    let session = with_state(hwnd, |state| state.active_mpv_session.clone())
        .flatten()
        .ok_or_else(|| "Nessuna riproduzione mpv attiva.".to_string())?;
    let mut pipe = open_mpv_ipc_pipe(&session.ipc_path)?;
    send_mpv_ipc_command_with_pipe(hwnd, &session.ipc_path, &mut pipe, command_json)
}

pub(crate) fn is_mpv_playback_active(hwnd: HWND) -> bool {
    with_state(hwnd, |state| state.active_mpv_session.is_some()).unwrap_or(false)
}

pub(crate) fn is_local_mpv_video_mode_active(hwnd: HWND) -> bool {
    with_state(hwnd, |state| state.local_mpv_video_mode_active).unwrap_or(false)
}

fn active_local_mpv_media(hwnd: HWND) -> Option<(PathBuf, u32)> {
    with_state(hwnd, |state| {
        let session = state.active_mpv_session.as_ref()?;
        let active_url = state.active_podcast_episode_url.as_ref()?;
        Some((PathBuf::from(active_url), session.process_id))
    })
    .flatten()
}

fn is_local_mpv_playback_active(hwnd: HWND) -> bool {
    active_local_mpv_media(hwnd).is_some()
}

pub(crate) fn set_local_mpv_video_mode(hwnd: HWND, active: bool) {
    if with_state(hwnd, |state| {
        state.local_mpv_video_mode_active = active;
    })
    .is_none()
    {
        log_debug("Failed to update local mpv video mode state");
    }
    crate::send_message_w_safe(
        hwnd,
        WM_LOCAL_MPV_VIDEO_MODE,
        WPARAM(if active { 1 } else { 0 }),
        LPARAM(0),
    );
}

fn set_local_mpv_video_menu_visible(hwnd: HWND, visible: bool) {
    let should_post = with_state(hwnd, |state| {
        if !state.local_mpv_video_mode_active || state.local_mpv_hidden_menu.0 == 0 {
            return false;
        }
        if state.local_mpv_menu_visible == visible {
            return false;
        }
        state.local_mpv_menu_visible = visible;
        true
    })
    .unwrap_or(false);
    log_debug(&format!(
        "local_mpv_menu_visible request: visible={} should_apply={} attached_menu={:?} hidden_menu={:?}",
        visible,
        should_post,
        crate::get_menu_safe(hwnd),
        with_state(hwnd, |state| state.local_mpv_hidden_menu).unwrap_or(HMENU(0))
    ));
    if should_post {
        if visible {
            crate::set_foreground_window_safe(hwnd);
            crate::set_focus_safe(hwnd);
        }
        crate::send_message_w_safe(
            hwnd,
            WM_LOCAL_MPV_MENU_VISIBLE,
            WPARAM(if visible { 1 } else { 0 }),
            LPARAM(0),
        );
    }
}

fn local_mpv_position_secs_for_path(hwnd: HWND, path: &Path) -> Option<f64> {
    let (active_path, _) = active_local_mpv_media(hwnd)?;
    if active_path != path {
        return None;
    }
    query_managed_mpv_property_transient(hwnd, "time-pos")
        .ok()
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite())
}

fn local_mpv_duration_secs_for_path(hwnd: HWND, path: &Path) -> Option<f64> {
    let (active_path, _) = active_local_mpv_media(hwnd)?;
    if active_path != path {
        return None;
    }
    query_managed_mpv_property_transient(hwnd, "duration")
        .ok()
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite() && *value > 0.0)
}

pub(crate) fn seek_local_mpv_to_seconds(
    hwnd: HWND,
    path: &Path,
    seconds: u64,
) -> Result<(), String> {
    let (active_path, _) = active_local_mpv_media(hwnd).ok_or_else(|| "no_media".to_string())?;
    if active_path != path {
        return Err("no_media".to_string());
    }
    try_send_command_to_managed_mpv_transient(
        hwnd,
        &format!(r#"{{"command":["seek",{},"absolute"]}}"#, seconds),
    )
}

fn set_active_audiobook_bookmark(hwnd: HWND, path: &Path, position: i32) {
    let key = path.to_string_lossy().to_string();
    if with_state(hwnd, |state| {
        state.active_audiobook_bookmark = Some((key, position));
    })
    .is_none()
    {
        log_debug("Failed to persist active audiobook bookmark");
    }
}

fn clear_active_audiobook_bookmark(hwnd: HWND) {
    if with_state(hwnd, |state| {
        state.active_audiobook_bookmark = None;
    })
    .is_none()
    {
        log_debug("Failed to clear active audiobook bookmark");
    }
}

pub(crate) fn jump_audiobook_to_position(hwnd: HWND, path: &Path, seconds: u64) {
    set_active_audiobook_bookmark(hwnd, path, seconds as i32);
    if seek_local_mpv_to_seconds(hwnd, path, seconds).is_err() {
        crate::audio_player::start_audiobook_at(hwnd, path, seconds);
        set_active_audiobook_bookmark(hwnd, path, seconds as i32);
    } else {
        stop_mpv_subtitle_speech(hwnd, "bookmark_seek");
    }
}

fn saved_audio_bookmark_position_secs(hwnd: HWND, path: &Path, title: Option<&str>) -> u64 {
    with_state(hwnd, |state| {
        let path_key = path.to_string_lossy().to_string();
        let title_key = title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(|title| format!("{STREAM_BOOKMARK_PREFIX}{title}"));
        state
            .active_audiobook_bookmark
            .as_ref()
            .and_then(|(stored_key, position)| {
                (stored_key == &path_key).then_some((*position).max(0) as u64)
            })
            .or_else(|| {
                title_key.as_ref().and_then(|title_key| {
                    state
                        .bookmarks
                        .files
                        .get(title_key)
                        .and_then(|list| {
                            list.iter().find(|bookmark| {
                                bookmark.is_visible(state.settings.automatic_bookmark)
                            })
                        })
                        .map(|bookmark| bookmark.position.max(0) as u64)
                })
            })
            .or_else(|| {
                state
                    .bookmarks
                    .files
                    .get(&path_key)
                    .and_then(|list| {
                        list.iter()
                            .find(|bookmark| bookmark.is_visible(state.settings.automatic_bookmark))
                    })
                    .map(|bookmark| bookmark.position.max(0) as u64)
            })
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

fn next_mpv_playback_generation(hwnd: HWND, reason: &str) -> u64 {
    with_state(hwnd, |state| {
        state.active_mpv_playback_generation = state.active_mpv_playback_generation.wrapping_add(1);
        let generation = state.active_mpv_playback_generation;
        log_debug(&format!(
            "Managed mpv: playback generation {} ({})",
            generation, reason
        ));
        generation
    })
    .unwrap_or(0)
}

fn is_current_mpv_playback_generation(hwnd: HWND, generation: u64) -> bool {
    with_state(hwnd, |state| {
        state.active_mpv_playback_generation == generation
    })
    .unwrap_or(false)
}

fn stop_obsolete_mpv_child(
    hwnd: HWND,
    generation: u64,
    child: &mut std::process::Child,
    context: &str,
) -> bool {
    if is_current_mpv_playback_generation(hwnd, generation) {
        return false;
    }
    log_debug(&format!(
        "Managed mpv: obsolete playback generation {} at {}; killing pid {} before activation",
        generation,
        context,
        child.id()
    ));
    if let Err(err) = child.kill() {
        log_debug(&format!(
            "Managed mpv: failed to kill obsolete playback generation {} pid {} at {}: {}",
            generation,
            child.id(),
            context,
            err
        ));
    }
    if let Err(err) = child.wait() {
        log_debug(&format!(
            "Managed mpv: failed to wait obsolete playback generation {} pid {} at {}: {}",
            generation,
            child.id(),
            context,
            err
        ));
    }
    prevent_sleep(false);
    true
}

fn sync_mpv_sleep_prevention(hwnd: HWND) {
    let paused = query_managed_mpv_property(hwnd, "pause")
        .ok()
        .and_then(|value| value.as_bool());
    if let Some(paused) = paused {
        prevent_sleep(!paused);
    }
}

pub(crate) fn stop_managed_mpv_playback(hwnd: HWND) {
    next_mpv_playback_generation(hwnd, "stop");
    clear_active_audiobook_bookmark(hwnd);
    set_local_mpv_video_mode(hwnd, false);
    let session = with_state(hwnd, |state| state.active_mpv_session.clone()).flatten();
    let (active_url, active_origin) = with_state(hwnd, |state| {
        (
            state.active_podcast_episode_url.clone(),
            state.active_podcast_episode_from_rai,
        )
    })
    .unwrap_or((None, RaiAudioOrigin::None));
    log_debug(&format!(
        "stop_managed_mpv_playback: foreground_before={:?} focus_before={:?} has_session={}",
        unsafe { GetForegroundWindow() },
        unsafe { GetFocus() },
        session.is_some()
    ));
    stop_mpv_subtitle_speech(hwnd, "stop");
    let resume_position_secs = query_managed_mpv_property(hwnd, "time-pos")
        .ok()
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.floor() as u64);
    if with_state(hwnd, |state| {
        state.last_stopped_mpv_url = active_url.clone();
        state.last_stopped_mpv_position_secs = resume_position_secs;
        state.last_stopped_mpv_origin = if active_url.is_some() {
            active_origin
        } else {
            RaiAudioOrigin::None
        };
    })
    .is_none()
    {
        log_debug("Failed to persist last stopped mpv position");
    }
    prevent_sleep(false);
    if stop_active_mpv_stream_recording(hwnd) {
        // Concediamo a libavformat il tempo di chiudere il contenitore prima
        // di terminare il processo mpv.
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    if let Some(session) = session
        && let Err(err) = send_mpv_ipc_command(&session.ipc_path, r#"{"command":["quit"]}"#)
    {
        log_debug(&format!("Managed mpv quit command failed: {}", err));
        taskkill_mpv_process(session.process_id, "after quit failure");
    }
    clear_managed_mpv_state(hwnd);
    let restored_return_list =
        app_windows::youtube_transcript_window::restore_active_player_return_list(hwnd);
    log_debug(&format!(
        "stop_managed_mpv_playback: restored_return_list={} foreground_after={:?} focus_after={:?}",
        restored_return_list,
        unsafe { GetForegroundWindow() },
        unsafe { GetFocus() }
    ));
}

pub(crate) fn launch_raiplay_in_mpv(
    hwnd: HWND,
    url: &str,
    podcast_title: Option<&str>,
    title: Option<&str>,
    rai_origin: RaiAudioOrigin,
) -> Result<(), String> {
    launch_raiplay_in_mpv_with_resume(hwnd, url, podcast_title, title, rai_origin, None)
}

pub(crate) fn launch_raiplay_in_mpv_with_resume(
    hwnd: HWND,
    url: &str,
    podcast_title: Option<&str>,
    title: Option<&str>,
    rai_origin: RaiAudioOrigin,
    resume_seconds: Option<u64>,
) -> Result<(), String> {
    let mpv_exe = ensure_mpv_runtime_available(hwnd)?;
    let mpv_dir = mpv_exe
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Cartella di mpv non valida.".to_string())?;
    let ipc_name = format!(
        "sonarpad-mpv-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let ipc_path = PathBuf::from(format!(r"\\.\pipe\{ipc_name}"));

    if is_mpv_playback_active(hwnd) {
        stop_managed_mpv_playback(hwnd);
    }
    if with_state(hwnd, |state| state.active_audiobook.is_some()).unwrap_or(false) {
        crate::audio_player::stop_audiobook_playback(hwnd);
    }
    let mpv_generation = next_mpv_playback_generation(hwnd, "start rai");

    let show_video =
        with_state(hwnd, |state| state.settings.show_video_during_playback).unwrap_or(false);
    let (
        playback_url,
        playback_media_url,
        render_video,
        select_raiplay_audio_track,
        external_audio_url,
    ) = match crate::tools::raiplay::resolve_playback_target(url)? {
        crate::tools::raiplay::PlaybackTarget::DirectStream {
            url: audio_only_url,
            media_url,
            is_live,
            ..
        } => {
            let render_video = show_video && !is_live;
            let has_external_audio = audio_only_url != media_url;
            let external_audio_url =
                (render_video && rai_origin == RaiAudioOrigin::RaiPlay && has_external_audio)
                    .then(|| audio_only_url.clone());
            let playback_url = if render_video {
                media_url.clone()
            } else {
                audio_only_url
            };
            (
                playback_url,
                media_url,
                render_video,
                render_video && rai_origin == RaiAudioOrigin::RaiPlay,
                external_audio_url,
            )
        }
        crate::tools::raiplay::PlaybackTarget::Download(resolved_url) => {
            (resolved_url.clone(), resolved_url, show_video, false, None)
        }
    };
    log_debug(&format!(
        "Managed mpv Rai launch: origin={:?} show_video={} render_video={} playback_url={} media_url={} external_audio={} select_raiplay_audio_track={}",
        rai_origin,
        show_video,
        render_video,
        playback_url,
        playback_media_url,
        external_audio_url.as_deref().unwrap_or(""),
        select_raiplay_audio_track
    ));
    let bookmark_start_secs = saved_audio_bookmark_position_secs(
        hwnd,
        &PathBuf::from(url),
        podcast_episode_display_title(podcast_title, title).as_deref(),
    );
    let start_seconds = resume_seconds
        .filter(|value| *value > 0)
        .or((bookmark_start_secs > 0).then_some(bookmark_start_secs));
    let hwnd_video = with_state(hwnd, |state| {
        if render_video {
            state.local_mpv_video_hwnd
        } else {
            HWND(0)
        }
    })
    .unwrap_or(HWND(0));
    log_debug(&format!(
        "Managed mpv Rai launch video host: render_video={} hwnd_video={:?}",
        render_video, hwnd_video
    ));
    if hwnd_video.0 != 0 {
        set_local_mpv_video_mode(hwnd, true);
    }
    let initial_mpv_volume =
        media_playback_volume_to_mpv(media_playback_volume_from_settings(hwnd));

    let mut command = std::process::Command::new(&mpv_exe);
    command
        .current_dir(&mpv_dir)
        .arg("--no-terminal")
        .arg("--volume-max=300")
        .arg(format!("--volume={initial_mpv_volume:.3}"))
        .arg("--pause=yes")
        .arg(&playback_url)
        .arg(format!("--input-ipc-server={}", ipc_path.display()));
    if let Some(audio_url) = external_audio_url.as_deref() {
        log_debug(&format!(
            "Managed mpv: attaching RaiPlay external audio track {}",
            audio_url
        ));
        command.arg(format!("--audio-file={audio_url}"));
    }
    if hwnd_video.0 != 0 {
        log_debug("Managed mpv Rai launch: enabling embedded video output");
        command
            .arg(format!("--wid={}", hwnd_video.0))
            .arg("--force-window=yes")
            .arg("--vid=auto")
            .arg("--vo=gpu")
            .arg("--gpu-context=win");
    } else {
        log_debug("Managed mpv Rai launch: disabling video output");
        command.arg("--no-video");
    }
    if let Some(start_seconds) = start_seconds {
        command.arg(format!("--start={start_seconds}"));
    }
    if let Some(title) = title.filter(|value| !value.trim().is_empty()) {
        command.arg(format!("--title={title}"));
    }
    let mut child =
        spawn_mpv_with_runtime_repair(hwnd, &mpv_exe, &mut command).inspect_err(|_| {
            set_local_mpv_video_mode(hwnd, false);
        })?;
    for _ in 0..40 {
        if send_mpv_ipc_command(&ipc_path, r#"{"command":["get_property","pause"]}"#).is_ok() {
            if stop_obsolete_mpv_child(hwnd, mpv_generation, &mut child, "rai ipc ready") {
                set_local_mpv_video_mode(hwnd, false);
                return Err("Riproduzione mpv annullata da una richiesta più recente.".to_string());
            }
            let playback_path = PathBuf::from(url);
            if let Some(index) = editor_manager::ensure_audio_document_tab(hwnd, &playback_path) {
                editor_manager::select_tab(hwnd, index);
            }
            if let Some(display_title) = podcast_episode_display_title(podcast_title, title) {
                editor_manager::set_current_document_title(hwnd, &display_title);
            }
            editor_manager::mark_current_document_from_rss(hwnd, true);
            editor_manager::mark_current_document_prefer_mpv_playback(hwnd, true);
            let persistent_pipe = open_mpv_ipc_pipe(&ipc_path).ok();
            if with_state(hwnd, |state| {
                state.active_mpv_session = Some(MpvPlaybackSession {
                    ipc_path: ipc_path.clone(),
                    process_id: child.id(),
                });
                state.active_mpv_is_live_tv = false;
                state.active_mpv_ipc = persistent_pipe;
                state.active_mpv_status = Some(MpvPlaybackStatus {
                    volume: initial_mpv_volume,
                    speed: 1.0,
                    pitch: 0.0,
                });
                state.active_podcast_episode_from_rai = rai_origin;
                state.raiplay_live_audio_variants.clear();
                state.available_audio_tracks.clear();
                state.selected_audio_track = None;
                state.last_stopped_mpv_url = None;
                state.last_stopped_mpv_position_secs = None;
                state.last_stopped_mpv_origin = RaiAudioOrigin::None;
            })
            .is_none()
            {
                log_debug("Failed to persist managed mpv state");
            }
            if select_raiplay_audio_track {
                select_raiplay_mpv_audio_track(hwnd);
            }
            if render_video && rai_origin == RaiAudioOrigin::RaiPlay {
                std::thread::sleep(std::time::Duration::from_millis(1200));
                log_managed_mpv_raiplay_video_snapshot(hwnd, "after_start");
            }
            set_active_podcast_episode_info(
                hwnd,
                Some(url.to_string()),
                Some(playback_media_url),
                podcast_title.map(ToOwned::to_owned),
                title.map(ToOwned::to_owned),
                None,
            );
            if stop_obsolete_mpv_child(hwnd, mpv_generation, &mut child, "before unpause") {
                set_local_mpv_video_mode(hwnd, false);
                return Err("Riproduzione mpv annullata da una richiesta più recente.".to_string());
            }
            if let Err(err) = try_send_command_to_managed_mpv(
                hwnd,
                r#"{"command":["set_property","pause",false]}"#,
            ) {
                log_debug(&format!(
                    "Managed mpv: failed to unpause generation {}: {}",
                    mpv_generation, err
                ));
            }
            prevent_sleep(true);
            menu::update_playback_menu(hwnd, true);
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    set_local_mpv_video_mode(hwnd, false);
    Err("Impossibile inizializzare il controllo di mpv.".to_string())
}

pub(crate) fn active_mpv_process_id(hwnd: HWND) -> Option<u32> {
    with_state(hwnd, |state| {
        state
            .active_mpv_session
            .as_ref()
            .map(|session| session.process_id)
    })
    .flatten()
}

/// Avvia la registrazione sullo stesso demuxer già aperto da mpv.
/// In questo modo non viene creata una seconda connessione HTTP/HLS, che può
/// essere rifiutata da CDN dinamiche anche quando la riproduzione è attiva.
pub(crate) fn start_active_mpv_stream_recording(
    hwnd: HWND,
    output_path: &Path,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!("Impossibile creare la cartella temporanea della registrazione mpv: {err}")
        })?;
    }
    if let Err(err) = std::fs::remove_file(output_path)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        return Err(format!(
            "Impossibile preparare il file temporaneo della registrazione mpv: {err}"
        ));
    }

    let path_text = output_path.to_string_lossy().to_string();
    let command = serde_json::json!({
        "command": ["set_property", "stream-record", path_text]
    })
    .to_string();
    try_send_command_to_managed_mpv(hwnd, &command)?;

    let active_value = query_managed_mpv_property(hwnd, "stream-record")?;
    let active_path = active_value.as_str().unwrap_or_default();
    if active_path.trim().is_empty() {
        return Err("mpv non ha attivato la registrazione del flusso corrente.".to_string());
    }
    log_debug(&format!(
        "Managed mpv stream recording enabled: {}",
        output_path.display()
    ));
    Ok(())
}

fn stop_active_mpv_stream_recording(hwnd: HWND) -> bool {
    let Ok(value) = query_managed_mpv_property(hwnd, "stream-record") else {
        return false;
    };
    let Some(path) = value.as_str().filter(|value| !value.trim().is_empty()) else {
        return false;
    };
    let command = serde_json::json!({
        "command": ["set_property", "stream-record", ""]
    })
    .to_string();
    match try_send_command_to_managed_mpv(hwnd, &command) {
        Ok(()) => {
            log_debug(&format!("Managed mpv stream recording closed: {path}"));
            true
        }
        Err(err) => {
            log_debug(&format!(
                "Managed mpv stream recording close failed path={path}: {err}"
            ));
            false
        }
    }
}

/// Attende che il player mpv abbia realmente aperto il flusso e pubblicato
/// almeno una traccia. Il processo e il pipe IPC possono esistere alcuni
/// secondi prima che un HLS dinamico (in particolare RAI) sia pronto.
pub(crate) fn wait_for_active_mpv_tracks(hwnd: HWND, timeout: std::time::Duration) -> bool {
    let started = std::time::Instant::now();
    let mut attempt = 0u32;
    while started.elapsed() < timeout {
        attempt = attempt.saturating_add(1);
        if active_mpv_process_id(hwnd).is_none() {
            log_debug("wait_for_active_mpv_tracks: player process is no longer active");
            return false;
        }
        if let Ok(track_list) = query_managed_mpv_property_transient(hwnd, "track-list")
            && track_list
                .as_array()
                .map(|tracks| !tracks.is_empty())
                .unwrap_or(false)
        {
            log_debug(&format!(
                "wait_for_active_mpv_tracks: media ready after {} ms (attempt={attempt})",
                started.elapsed().as_millis()
            ));
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    log_debug(&format!(
        "wait_for_active_mpv_tracks: timed out after {} ms",
        started.elapsed().as_millis()
    ));
    false
}

pub(crate) fn launch_stream_url_in_mpv(
    hwnd: HWND,
    url: &str,
    title: Option<&str>,
    ytdlp_path: Option<&Path>,
    ytdl_format: Option<&str>,
    ytdlp_credentials: Option<(&str, &str)>,
) -> Result<(), String> {
    launch_stream_url_in_mpv_with_options(
        hwnd,
        url,
        StreamMpvOptions {
            title,
            ytdlp_path,
            ytdl_format,
            ytdlp_credentials,
            user_agent: None,
            prefer_audio_description: false,
            allow_bookmark_resume: true,
            force_video: false,
            live_tv: false,
            configure_tv_tracks_async: false,
        },
    )
}

pub(crate) fn launch_tv_stream_in_mpv(
    hwnd: HWND,
    url: &str,
    title: &str,
    user_agent: &str,
    prefer_audio_description: bool,
) -> Result<(), String> {
    launch_stream_url_in_mpv_with_options(
        hwnd,
        url,
        StreamMpvOptions {
            title: Some(title),
            ytdlp_path: None,
            ytdl_format: None,
            ytdlp_credentials: None,
            user_agent: Some(user_agent),
            prefer_audio_description,
            allow_bookmark_resume: false,
            force_video: true,
            live_tv: true,
            configure_tv_tracks_async: true,
        },
    )
}

pub(crate) fn launch_tv_stream_for_recording_in_mpv(
    hwnd: HWND,
    url: &str,
    title: &str,
    user_agent: &str,
    prefer_audio_description: bool,
) -> Result<(), String> {
    launch_stream_url_in_mpv_with_options(
        hwnd,
        url,
        StreamMpvOptions {
            title: Some(title),
            ytdlp_path: None,
            ytdl_format: None,
            ytdlp_credentials: None,
            user_agent: Some(user_agent),
            prefer_audio_description,
            allow_bookmark_resume: false,
            force_video: true,
            live_tv: true,
            // La registrazione configura e stabilizza le tracce in modo
            // sincrono prima di attivare stream-record.
            configure_tv_tracks_async: false,
        },
    )
}

pub(crate) fn launch_video_stream_in_mpv(
    hwnd: HWND,
    url: &str,
    title: &str,
    ytdlp_path: Option<&Path>,
    ytdl_format: Option<&str>,
    ytdlp_credentials: Option<(&str, &str)>,
) -> Result<(), String> {
    launch_stream_url_in_mpv_with_options(
        hwnd,
        url,
        StreamMpvOptions {
            title: Some(title),
            ytdlp_path,
            ytdl_format,
            ytdlp_credentials,
            user_agent: None,
            prefer_audio_description: false,
            allow_bookmark_resume: false,
            force_video: true,
            live_tv: false,
            configure_tv_tracks_async: true,
        },
    )
}

fn is_mediaset_cdn_stream(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("mediaset.net") || lower.contains("mediaset.it") || lower.contains("msf.cdn")
}

pub(crate) fn preferred_tv_video_track_id(
    tracks: &[serde_json::Value],
    preferred_audio_id: Option<i64>,
) -> Option<i64> {
    let is_track_type = |track: &&serde_json::Value, kind: &str| {
        track.get("type").and_then(serde_json::Value::as_str) == Some(kind)
    };
    let video_tracks = tracks
        .iter()
        .filter(|track| is_track_type(track, "video"))
        .collect::<Vec<_>>();
    let audio_tracks = tracks
        .iter()
        .filter(|track| is_track_type(track, "audio"))
        .collect::<Vec<_>>();
    let reference_audio = preferred_audio_id
        .and_then(|preferred_id| {
            audio_tracks.iter().copied().find(|track| {
                track.get("id").and_then(serde_json::Value::as_i64) == Some(preferred_id)
            })
        })
        .or_else(|| {
            audio_tracks.iter().copied().find(|track| {
                track
                    .get("selected")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            })
        })
        .or_else(|| audio_tracks.first().copied());

    let matching_video = reference_audio.and_then(|audio| {
        audio
            .get("program-id")
            .and_then(serde_json::Value::as_i64)
            .and_then(|program_id| {
                video_tracks.iter().copied().find(|video| {
                    video.get("program-id").and_then(serde_json::Value::as_i64) == Some(program_id)
                })
            })
            .or_else(|| {
                audio
                    .get("hls-bitrate")
                    .and_then(serde_json::Value::as_i64)
                    .and_then(|bitrate| {
                        video_tracks.iter().copied().find(|video| {
                            video.get("hls-bitrate").and_then(serde_json::Value::as_i64)
                                == Some(bitrate)
                        })
                    })
            })
    });

    matching_video
        .or_else(|| video_tracks.first().copied())
        .and_then(|track| track.get("id"))
        .and_then(serde_json::Value::as_i64)
}

pub(crate) fn stabilize_active_mpv_audiodescription_tracks_for_recording(
    hwnd: HWND,
    timeout: std::time::Duration,
) -> Result<bool, String> {
    let track_list = query_managed_mpv_property_transient(hwnd, "track-list")?;
    let Some(tracks) = track_list.as_array() else {
        return Err("mpv ha restituito un elenco tracce TV non valido.".to_string());
    };
    let Some(audio_id) = audiodescription_mpv_audio_track_id(&track_list) else {
        log_debug("TV recording track stabilization skipped: no audiodescription track available");
        return Ok(false);
    };
    let video_id = preferred_tv_video_track_id(tracks, Some(audio_id));

    let audio_command = format!(r#"{{"command":["set_property","aid",{}]}}"#, audio_id);
    try_send_command_to_managed_mpv_transient(hwnd, &audio_command)?;
    if let Some(video_id) = video_id {
        let video_command = format!(r#"{{"command":["set_property","vid",{}]}}"#, video_id);
        try_send_command_to_managed_mpv_transient(hwnd, &video_command)?;
    }

    let started = std::time::Instant::now();
    let mut stable_since = None;
    while started.elapsed() < timeout {
        let current = query_managed_mpv_property_transient(hwnd, "track-list")?;
        let selected = current.as_array().is_some_and(|tracks| {
            let audio_ready = tracks.iter().any(|track| {
                track.get("type").and_then(serde_json::Value::as_str) == Some("audio")
                    && track.get("id").and_then(serde_json::Value::as_i64) == Some(audio_id)
                    && track
                        .get("selected")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
            });
            let video_ready = video_id.is_none_or(|video_id| {
                tracks.iter().any(|track| {
                    track.get("type").and_then(serde_json::Value::as_str) == Some("video")
                        && track.get("id").and_then(serde_json::Value::as_i64) == Some(video_id)
                        && track
                            .get("selected")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false)
                })
            });
            audio_ready && video_ready
        });

        if selected {
            let since = stable_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= std::time::Duration::from_millis(750) {
                log_debug(&format!(
                    "TV recording tracks stabilized: audiodescription_id={audio_id} video_id={}",
                    video_id
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "none".to_string())
                ));
                return Ok(true);
            }
        } else {
            stable_since = None;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Err(format!(
        "Le tracce TV per la registrazione non si sono stabilizzate (audio={audio_id}, video={}).",
        video_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "nessuno".to_string())
    ))
}

fn configure_tv_tracks_after_load(
    hwnd: HWND,
    url: &str,
    generation: u64,
    prefer_audio_description: bool,
) {
    let mediaset = is_mediaset_cdn_stream(url);

    // Il pipe IPC è disponibile prima che mpv abbia realmente caricato il file.
    // Alcuni HLS (in particolare Mediaset) impiegano diversi secondi: attendiamo
    // la comparsa effettiva delle tracce, poi selezioniamo esplicitamente video
    // e audio se mpv non lo ha già fatto.
    for attempt in 1..=60 {
        if !is_current_mpv_playback_generation(hwnd, generation) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));

        let Ok(track_list) = query_managed_mpv_property_transient(hwnd, "track-list") else {
            continue;
        };
        let Some(tracks) = track_list.as_array() else {
            continue;
        };
        if tracks.is_empty() {
            if mediaset && attempt % 8 == 0 {
                log_debug(&format!(
                    "Mediaset mpv waiting for file-loaded attempt={attempt} tracks=0"
                ));
            }
            continue;
        }

        let summarize = |track: &serde_json::Value| {
            let kind = track
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?");
            let id = track
                .get("id")
                .and_then(serde_json::Value::as_i64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_string());
            let codec = track
                .get("codec")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?");
            let selected = track
                .get("selected")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            format!("type={kind},id={id},codec={codec},selected={selected}")
        };
        let summary = tracks.iter().map(summarize).collect::<Vec<_>>().join("; ");
        log_debug(&format!(
            "TV mpv tracks ready attempt={attempt} mediaset={mediaset}: {summary}"
        ));

        let preferred_audio_id = prefer_audio_description
            .then(|| preferred_mpv_audio_track_id(&track_list))
            .flatten();
        let audio_tracks = tracks
            .iter()
            .filter(|track| track.get("type").and_then(serde_json::Value::as_str) == Some("audio"))
            .collect::<Vec<_>>();
        let selected_audio_id = audio_tracks.iter().find_map(|track| {
            track
                .get("selected")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
                .then(|| track.get("id").and_then(serde_json::Value::as_i64))
                .flatten()
        });
        let target_audio_id = preferred_audio_id.or(selected_audio_id).or_else(|| {
            audio_tracks
                .first()
                .and_then(|track| track.get("id"))
                .and_then(serde_json::Value::as_i64)
        });

        if let Some(audio_id) = target_audio_id
            && selected_audio_id != Some(audio_id)
        {
            let command = format!(r#"{{"command":["set_property","aid",{}]}}"#, audio_id);
            match try_send_command_to_managed_mpv_transient(hwnd, &command) {
                Ok(()) if prefer_audio_description => log_debug(&format!(
                    "TV mpv selected preferred Rai audiodescription audio track id={audio_id}"
                )),
                Ok(()) => log_debug(&format!("TV mpv selected first audio track id={audio_id}")),
                Err(err) => log_debug(&format!(
                    "TV mpv failed to select audio track id={audio_id}: {err}"
                )),
            }
        }

        let video_tracks = tracks
            .iter()
            .filter(|track| track.get("type").and_then(serde_json::Value::as_str) == Some("video"))
            .collect::<Vec<_>>();
        let selected_video_id = video_tracks.iter().find_map(|track| {
            track
                .get("selected")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
                .then(|| track.get("id").and_then(serde_json::Value::as_i64))
                .flatten()
        });
        if let Some(video_id) = preferred_tv_video_track_id(tracks, target_audio_id)
            && selected_video_id != Some(video_id)
        {
            let command = format!(r#"{{"command":["set_property","vid",{}]}}"#, video_id);
            match try_send_command_to_managed_mpv_transient(hwnd, &command) {
                Ok(()) => log_debug(&format!(
                    "TV mpv selected video track matching audio variant id={video_id}"
                )),
                Err(err) => log_debug(&format!(
                    "TV mpv failed to select video track id={video_id}: {err}"
                )),
            }
        }

        match query_managed_mpv_property_transient(hwnd, "video-params") {
            Ok(params) => log_debug(&format!("TV mpv video-params ready: {params}")),
            Err(err) => log_debug(&format!("TV mpv video-params unavailable: {err}")),
        }
        return;
    }

    log_debug(&format!(
        "TV mpv track discovery timed out mediaset={mediaset} url_host={}",
        url::Url::parse(url)
            .ok()
            .and_then(|parsed| parsed.host_str().map(ToOwned::to_owned))
            .unwrap_or_else(|| "local-or-invalid".to_string())
    ));
}

fn schedule_tv_track_configuration(
    hwnd: HWND,
    url: &str,
    generation: u64,
    prefer_audio_description: bool,
) {
    // Passiamo al thread soltanto il valore numerico dell'handle: in alcune
    // versioni del crate windows HWND contiene un puntatore grezzo non Send.
    let hwnd_value = hwnd.0;
    let url = url.to_string();
    std::thread::spawn(move || {
        configure_tv_tracks_after_load(
            HWND(hwnd_value),
            &url,
            generation,
            prefer_audio_description,
        );
    });
}

struct StreamMpvOptions<'a> {
    title: Option<&'a str>,
    ytdlp_path: Option<&'a Path>,
    ytdl_format: Option<&'a str>,
    ytdlp_credentials: Option<(&'a str, &'a str)>,
    user_agent: Option<&'a str>,
    prefer_audio_description: bool,
    allow_bookmark_resume: bool,
    force_video: bool,
    live_tv: bool,
    configure_tv_tracks_async: bool,
}

fn launch_stream_url_in_mpv_with_options(
    hwnd: HWND,
    url: &str,
    options: StreamMpvOptions<'_>,
) -> Result<(), String> {
    let StreamMpvOptions {
        title,
        ytdlp_path,
        ytdl_format,
        ytdlp_credentials,
        user_agent,
        prefer_audio_description,
        allow_bookmark_resume,
        force_video,
        live_tv,
        configure_tv_tracks_async,
    } = options;
    let mpv_exe = ensure_mpv_runtime_available(hwnd)?;
    let mpv_dir = mpv_exe
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Cartella di mpv non valida.".to_string())?;
    let ipc_name = format!(
        "sonarpad-mpv-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let ipc_path = PathBuf::from(format!(r"\\.\pipe\{ipc_name}"));

    if is_mpv_playback_active(hwnd) {
        stop_managed_mpv_playback(hwnd);
    }
    if with_state(hwnd, |state| state.active_audiobook.is_some()).unwrap_or(false) {
        crate::audio_player::stop_audiobook_playback(hwnd);
    }
    focus_editor(hwnd);
    let mpv_generation = next_mpv_playback_generation(hwnd, "start stream");

    let bookmark_start_secs = if allow_bookmark_resume {
        saved_audio_bookmark_position_secs(hwnd, &PathBuf::from(url), title)
    } else {
        0
    };
    let hwnd_video = with_state(hwnd, |state| {
        if force_video || state.settings.show_video_during_playback {
            state.local_mpv_video_hwnd
        } else {
            HWND(0)
        }
    })
    .unwrap_or(HWND(0));
    if hwnd_video.0 != 0 {
        set_local_mpv_video_mode(hwnd, true);
    }
    let initial_mpv_volume =
        media_playback_volume_to_mpv(media_playback_volume_from_settings(hwnd));

    // YouTube playback is deliberately handed to mpv's ytdl hook.  Resolving a
    // googlevideo URL in Sonarpad made playback depend on the extractor client
    // selected by a separate yt-dlp invocation (notably android_vr), and those
    // short-lived URLs can fail even though the normal watch URL still works.
    // Sonarpad continues to store and download the original watch URL through
    // the independent download pipeline; this pseudo-URL is playback-only.
    let is_youtube = url.to_ascii_lowercase().contains("youtube.com/")
        || url.to_ascii_lowercase().contains("youtu.be/");
    let playback_input = if is_youtube {
        format!("ytdl://{url}")
    } else {
        url.to_string()
    };
    // mpv logs its full command line. Never create this diagnostic file when
    // username/password options are present; IPC still performs the load check.
    let youtube_log_path = (is_youtube && ytdlp_credentials.is_none()).then(|| {
        std::env::temp_dir().join(format!(
            "sonarpad-youtube-mpv-{}-{}.log",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ))
    });

    let mut command = std::process::Command::new(&mpv_exe);
    // Regression guard: .arg(if is_youtube { "--pause=no" } else { "--pause=yes" })
    command
        .current_dir(&mpv_dir)
        .arg(&playback_input)
        .arg("--no-terminal")
        .arg("--volume-max=300")
        .arg(format!("--volume={initial_mpv_volume:.3}"))
        .arg(if is_youtube {
            "--pause=no"
        } else {
            "--pause=yes"
        })
        .arg(format!("--input-ipc-server={}", ipc_path.display()));
    if is_youtube {
        command.arg("--ytdl=yes");
        if let Some(log_path) = youtube_log_path.as_ref() {
            command
                .arg(format!("--log-file={}", log_path.display()))
                .arg("--msg-level=ytdl_hook=debug,cplayer=debug");
        }
        log_debug(&format!(
            "Managed mpv YouTube direct playback: url={} log={}",
            url,
            youtube_log_path
                .as_deref()
                .map(Path::display)
                .map(|value| value.to_string())
                .unwrap_or_default()
        ));
    }
    if let Some(user_agent) = user_agent.map(str::trim).filter(|value| !value.is_empty()) {
        command.arg(format!("--user-agent={user_agent}"));
    }
    if force_video {
        command.arg("--aid=auto").arg("--audio-channels=stereo");
    }
    if live_tv && !url.to_ascii_lowercase().contains(".mpd") {
        // I resolver TV possono nascondere la playlist HLS dietro un redirect
        // PHP. Applichiamo quindi il limite a tutti i canali live non DASH.
        command.arg("--hls-bitrate=3000000");
        log_debug("Managed mpv HLS bitrate cap enabled: 3000000");
    }
    if live_tv && url.to_ascii_lowercase().contains(".mpd") {
        // I manifest DASH live descrivono gia le tracce disponibili. Evitiamo
        // il buffering aggiuntivo durante l'analisi iniziale di FFmpeg, senza
        // disattivare la cache di mpv usata durante la riproduzione.
        command.arg("--demuxer-lavf-o-add=fflags=+nobuffer");
        log_debug("Managed mpv fast DASH startup enabled");
    }
    if hwnd_video.0 != 0 {
        command
            .arg(format!("--wid={}", hwnd_video.0))
            .arg("--force-window=yes")
            .arg("--vid=auto")
            .arg("--vo=gpu")
            .arg("--gpu-context=win");
    } else {
        command.arg("--no-video").arg("--force-window=no");
    }
    if bookmark_start_secs > 0 {
        command.arg(format!("--start={bookmark_start_secs}"));
    }
    if let Some(ytdlp_path) = ytdlp_path {
        command.arg(format!(
            "--script-opts=ytdl_hook-ytdl_path={}",
            ytdlp_path.to_string_lossy()
        ));
    }
    if let Some(ytdl_format) = ytdl_format.filter(|value| !value.trim().is_empty()) {
        command.arg(format!("--ytdl-format={ytdl_format}"));
    }
    let mut ytdl_raw_options = Vec::new();
    if is_youtube {
        // Avoid yt-dlp's automatic android_vr choice: its signed media URLs
        // currently return 403 for otherwise public videos on Windows.
        ytdl_raw_options.push("extractor-args=youtube:player_client=android".to_string());
    }
    if let Some((username, password)) = ytdlp_credentials
        .filter(|(username, password)| !username.trim().is_empty() && !password.trim().is_empty())
    {
        log_debug(&format!(
            "Managed mpv yt-dlp auth args enabled username={} password_arg=true",
            username
        ));
        ytdl_raw_options.push(format!("username={username}"));
        ytdl_raw_options.push(format!("password={password}"));
    }
    if !ytdl_raw_options.is_empty() {
        command.arg(format!("--ytdl-raw-options={}", ytdl_raw_options.join(",")));
    }
    if let Some(title) = title.filter(|value| !value.trim().is_empty()) {
        command.arg(format!("--title={title}"));
    }
    command.creation_flags(CREATE_NO_WINDOW_FLAGS);

    let mut child =
        spawn_mpv_with_runtime_repair(hwnd, &mpv_exe, &mut command).inspect_err(|_| {
            set_local_mpv_video_mode(hwnd, false);
        })?;
    for _ in 0..40 {
        if send_mpv_ipc_command(&ipc_path, r#"{"command":["get_property","pause"]}"#).is_ok() {
            if stop_obsolete_mpv_child(hwnd, mpv_generation, &mut child, "stream ipc ready") {
                set_local_mpv_video_mode(hwnd, false);
                return Err("Riproduzione mpv annullata da una richiesta più recente.".to_string());
            }
            if is_youtube
                && let Err(detail) = wait_for_youtube_mpv_file_loaded(
                    &mut child,
                    &ipc_path,
                    youtube_log_path.as_deref(),
                )
            {
                log_debug(&format!(
                    "Managed mpv YouTube IPC end-file/load failure: pid={} detail={}",
                    child.id(),
                    detail
                ));
                if let Err(error) = child.kill() {
                    log_debug(&format!(
                        "Managed mpv YouTube failed to stop after load error: {error}"
                    ));
                }
                if let Err(error) = child.wait() {
                    log_debug(&format!(
                        "Managed mpv YouTube failed to wait after load error: {error}"
                    ));
                }
                if let Some(log_path) = youtube_log_path.as_deref()
                    && let Err(error) = std::fs::remove_file(log_path)
                {
                    log_debug(&format!(
                        "Managed mpv YouTube log cleanup failed: path={} error={error}",
                        log_path.display()
                    ));
                }
                set_local_mpv_video_mode(hwnd, false);
                return Err(detail);
            }
            let playback_path = PathBuf::from(url);
            if let Some(index) = editor_manager::ensure_audio_document_tab(hwnd, &playback_path) {
                editor_manager::select_tab(hwnd, index);
            }
            if let Some(title) = title {
                editor_manager::set_current_document_title(hwnd, title);
            }
            editor_manager::mark_current_document_from_rss(hwnd, true);
            editor_manager::mark_current_document_prefer_mpv_playback(hwnd, true);
            let persistent_pipe = open_mpv_ipc_pipe(&ipc_path).ok();
            if with_state(hwnd, |state| {
                state.active_mpv_session = Some(MpvPlaybackSession {
                    ipc_path: ipc_path.clone(),
                    process_id: child.id(),
                });
                state.active_mpv_is_live_tv = live_tv;
                state.active_mpv_ipc = persistent_pipe;
                state.active_mpv_status = Some(MpvPlaybackStatus {
                    volume: initial_mpv_volume,
                    speed: 1.0,
                    pitch: 0.0,
                });
                state.active_podcast_episode_url = Some(url.to_string());
                state.active_podcast_episode_media_url = Some(url.to_string());
                state.active_podcast_title = None;
                state.active_podcast_episode_title = title.map(ToOwned::to_owned);
                state.active_podcast_episode_cache = None;
                state.active_podcast_episode_from_rai = RaiAudioOrigin::None;
                state.raiplay_live_audio_variants.clear();
                state.available_audio_tracks.clear();
                state.selected_audio_track = None;
                state.last_stopped_mpv_url = None;
                state.last_stopped_mpv_position_secs = None;
                state.last_stopped_mpv_origin = RaiAudioOrigin::None;
            })
            .is_none()
            {
                log_debug("Failed to persist stream mpv state");
            }
            if let Some(log_path) = youtube_log_path.clone() {
                monitor_youtube_mpv_startup(hwnd, child.id(), log_path);
            }
            if prefer_audio_description {
                select_raiplay_mpv_audio_track(hwnd);
            }
            if stop_obsolete_mpv_child(hwnd, mpv_generation, &mut child, "before unpause") {
                set_local_mpv_video_mode(hwnd, false);
                return Err("Riproduzione mpv annullata da una richiesta più recente.".to_string());
            }
            if !is_youtube
                && let Err(err) = try_send_command_to_managed_mpv(
                    hwnd,
                    r#"{"command":["set_property","pause",false]}"#,
                )
            {
                log_debug(&format!(
                    "Managed mpv: failed to unpause generation {}: {}",
                    mpv_generation, err
                ));
            }
            if force_video && configure_tv_tracks_async {
                schedule_tv_track_configuration(
                    hwnd,
                    url,
                    mpv_generation,
                    prefer_audio_description,
                );
            }
            prevent_sleep(true);
            menu::update_playback_menu(hwnd, true);
            focus_editor(hwnd);
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    set_local_mpv_video_mode(hwnd, false);
    Err("Impossibile inizializzare il controllo di mpv.".to_string())
}

pub(crate) fn launch_local_tv_recording_in_mpv(hwnd: HWND, path: &Path) -> Result<(), String> {
    let fallback_title = i18n::tr_tv("tv.local_recording_title");
    let title = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(fallback_title.as_str());
    let path_text = path.to_string_lossy().to_string();
    launch_stream_url_in_mpv_with_options(
        hwnd,
        &path_text,
        StreamMpvOptions {
            title: Some(title),
            ytdlp_path: None,
            ytdl_format: None,
            ytdlp_credentials: None,
            user_agent: None,
            prefer_audio_description: false,
            allow_bookmark_resume: false,
            force_video: true,
            live_tv: false,
            configure_tv_tracks_async: true,
        },
    )
}

pub(crate) fn launch_local_video_in_mpv(hwnd: HWND, path: &Path) -> Result<(), String> {
    let mpv_exe = ensure_mpv_runtime_available(hwnd)?;
    let mpv_dir = mpv_exe
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Cartella di mpv non valida.".to_string())?;
    let ipc_name = format!(
        "sonarpad-mpv-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let ipc_path = PathBuf::from(format!(r"\\.\pipe\{ipc_name}"));

    if is_mpv_playback_active(hwnd) {
        stop_managed_mpv_playback(hwnd);
    }
    if with_state(hwnd, |state| state.active_audiobook.is_some()).unwrap_or(false) {
        crate::audio_player::stop_audiobook_playback(hwnd);
    }
    let mpv_generation = next_mpv_playback_generation(hwnd, "start local");

    let title = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Video");
    let bookmark_start_secs = saved_audio_bookmark_position_secs(hwnd, path, None);
    let hwnd_video = with_state(hwnd, |state| {
        if state.settings.show_video_during_playback {
            state.local_mpv_video_hwnd
        } else {
            HWND(0)
        }
    })
    .unwrap_or(HWND(0));
    if hwnd_video.0 != 0 {
        set_local_mpv_video_mode(hwnd, true);
    }
    let initial_mpv_volume =
        media_playback_volume_to_mpv(media_playback_volume_from_settings(hwnd));
    let mut command = std::process::Command::new(&mpv_exe);
    command
        .current_dir(&mpv_dir)
        .arg(path)
        .arg("--no-terminal")
        .arg("--volume-max=300")
        .arg(format!("--volume={initial_mpv_volume:.3}"))
        .arg("--pause=yes")
        .arg(format!("--input-ipc-server={}", ipc_path.display()))
        .arg(format!("--title={title}"));
    if hwnd_video.0 != 0 {
        command
            .arg(format!("--wid={}", hwnd_video.0))
            .arg("--force-window=yes")
            .arg("--vid=auto")
            .arg("--vo=gpu")
            .arg("--gpu-context=win");
    } else {
        command.arg("--no-video");
    }
    if bookmark_start_secs > 0 {
        command.arg(format!("--start={bookmark_start_secs}"));
    }

    let mut child = match spawn_mpv_with_runtime_repair(hwnd, &mpv_exe, &mut command) {
        Ok(child) => child,
        Err(err) => {
            set_local_mpv_video_mode(hwnd, false);
            return Err(err);
        }
    };
    for _ in 0..40 {
        if send_mpv_ipc_command(&ipc_path, r#"{"command":["get_property","pause"]}"#).is_ok() {
            if stop_obsolete_mpv_child(hwnd, mpv_generation, &mut child, "local ipc ready") {
                set_local_mpv_video_mode(hwnd, false);
                return Err("Riproduzione mpv annullata da una richiesta più recente.".to_string());
            }
            if let Some(index) = editor_manager::ensure_audio_document_tab(hwnd, path) {
                editor_manager::select_tab(hwnd, index);
            }
            editor_manager::mark_current_document_prefer_mpv_playback(hwnd, true);
            let path_text = path.to_string_lossy().to_string();
            let persistent_pipe = open_mpv_ipc_pipe(&ipc_path).ok();
            if with_state(hwnd, |state| {
                state.active_mpv_session = Some(MpvPlaybackSession {
                    ipc_path: ipc_path.clone(),
                    process_id: child.id(),
                });
                state.active_mpv_is_live_tv = false;
                state.active_mpv_ipc = persistent_pipe;
                state.active_mpv_status = Some(MpvPlaybackStatus {
                    volume: initial_mpv_volume,
                    speed: 1.0,
                    pitch: 0.0,
                });
                state.active_podcast_episode_url = Some(path_text.clone());
                state.active_podcast_episode_media_url = Some(path_text);
                state.active_podcast_title = None;
                state.active_podcast_episode_title = Some(title.to_string());
                state.active_podcast_episode_cache = None;
                state.active_podcast_episode_from_rai = RaiAudioOrigin::None;
                state.raiplay_live_audio_variants.clear();
                state.available_audio_tracks.clear();
                state.selected_audio_track = None;
                state.last_stopped_mpv_url = None;
                state.last_stopped_mpv_position_secs = None;
                state.last_stopped_mpv_origin = RaiAudioOrigin::None;
            })
            .is_none()
            {
                log_debug("Failed to persist local mpv state");
            }
            if stop_obsolete_mpv_child(hwnd, mpv_generation, &mut child, "before unpause") {
                set_local_mpv_video_mode(hwnd, false);
                return Err("Riproduzione mpv annullata da una richiesta più recente.".to_string());
            }
            if let Err(err) = try_send_command_to_managed_mpv(
                hwnd,
                r#"{"command":["set_property","pause",false]}"#,
            ) {
                log_debug(&format!(
                    "Managed mpv: failed to unpause generation {}: {}",
                    mpv_generation, err
                ));
            }
            prevent_sleep(true);
            menu::update_playback_menu(hwnd, true);
            if let Err(err) = apply_local_mpv_subtitle_offset(hwnd) {
                log_debug(&format!("Local mpv subtitle offset failed: {}", err));
            }
            if let Some(subtitle_path) = crate::subtitles::find_subtitle_for_media(path) {
                log_debug(&format!(
                    "Local mpv subtitle auto-load: {}",
                    subtitle_path.display()
                ));
                if let Err(err) = send_local_mpv_subtitle_file(hwnd, &subtitle_path) {
                    log_debug(&format!("Local mpv subtitle auto-load failed: {}", err));
                }
            }
            start_local_mpv_subtitle_reader(hwnd, path.to_path_buf(), child.id());
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    set_local_mpv_video_mode(hwnd, false);
    Err("Impossibile inizializzare il controllo di mpv.".to_string())
}

fn normalize_mpv_subtitle_text(text: &str) -> String {
    text.replace("\\N", "\n")
        .replace('\u{2028}', "\n")
        .trim()
        .to_string()
}

fn speak_mpv_subtitle_text(hwnd: HWND, text: String) {
    let mode = with_state(hwnd, |state| state.settings.subtitle_read_mode)
        .unwrap_or(SubtitleReadMode::Off);
    match mode {
        SubtitleReadMode::Off | SubtitleReadMode::Record => {}
        SubtitleReadMode::Nvda => {
            if !nvda_speak(&text) {
                log_debug("mpv subtitle: NVDA speak failed");
            }
        }
        SubtitleReadMode::User
        | SubtitleReadMode::Sapi5
        | SubtitleReadMode::Sapi4
        | SubtitleReadMode::Edge => {
            tts_engine::speak_text_once(hwnd, text);
        }
    }
}

fn stop_mpv_subtitle_speech(hwnd: HWND, reason: &str) {
    let mode = with_state(hwnd, |state| state.settings.subtitle_read_mode)
        .unwrap_or(SubtitleReadMode::Off);
    match mode {
        SubtitleReadMode::User
        | SubtitleReadMode::Sapi5
        | SubtitleReadMode::Sapi4
        | SubtitleReadMode::Edge => {
            log_debug(&format!("mpv subtitle speech stop: {}", reason));
            tts_engine::stop_tts_playback(hwnd);
        }
        SubtitleReadMode::Nvda => {
            log_debug(&format!(
                "mpv subtitle speech stop skipped for NVDA mode: {}",
                reason
            ));
        }
        SubtitleReadMode::Off | SubtitleReadMode::Record => {}
    }
}

fn apply_local_mpv_subtitle_offset(hwnd: HWND) -> Result<(), String> {
    let offset_secs = with_state(hwnd, |state| state.settings.subtitle_offset_ms)
        .unwrap_or_default() as f64
        / 1000.0;
    let command = serde_json::json!({
        "command": [
            "set_property",
            "sub-delay",
            offset_secs
        ]
    })
    .to_string();
    try_send_command_to_managed_mpv(hwnd, &command)
}

fn send_local_mpv_subtitle_file(hwnd: HWND, subtitle_path: &Path) -> Result<(), String> {
    let command = serde_json::json!({
        "command": [
            "sub-add",
            subtitle_path.to_string_lossy().to_string(),
            "select"
        ]
    })
    .to_string();
    try_send_command_to_managed_mpv(hwnd, &command)
}

pub(crate) fn set_local_mpv_subtitle_override(
    hwnd: HWND,
    subtitle_path: &Path,
) -> Result<(), String> {
    let (media_path, process_id) =
        active_local_mpv_media(hwnd).ok_or_else(|| "no_media".to_string())?;
    crate::subtitles::set_subtitle_override(&media_path, subtitle_path.to_path_buf());
    apply_local_mpv_subtitle_offset(hwnd)?;
    send_local_mpv_subtitle_file(hwnd, subtitle_path)?;
    start_local_mpv_subtitle_reader(hwnd, media_path, process_id);
    Ok(())
}

pub(crate) fn clear_local_mpv_subtitle_override(hwnd: HWND) -> Result<(), String> {
    let (media_path, _) = active_local_mpv_media(hwnd).ok_or_else(|| "no_media".to_string())?;
    crate::subtitles::clear_subtitle_override(&media_path);
    try_send_command_to_managed_mpv(hwnd, r#"{"command":["set_property","sid","no"]}"#)?;
    if with_state(hwnd, |state| {
        state.active_mpv_subtitle_generation = state.active_mpv_subtitle_generation.wrapping_add(1);
    })
    .is_none()
    {
        log_debug("Failed to stop local mpv subtitle reader");
    }
    Ok(())
}

fn start_local_mpv_subtitle_reader(hwnd: HWND, path: PathBuf, process_id: u32) {
    let mode = with_state(hwnd, |state| state.settings.subtitle_read_mode)
        .unwrap_or(SubtitleReadMode::Off);
    if mode == SubtitleReadMode::Off || mode == SubtitleReadMode::Record {
        return;
    }
    let generation = with_state(hwnd, |state| {
        state.active_mpv_subtitle_generation = state.active_mpv_subtitle_generation.wrapping_add(1);
        state.active_mpv_subtitle_generation
    })
    .unwrap_or_default();

    std::thread::spawn(move || {
        let path_text = path.to_string_lossy().to_string();
        let mut last_text = String::new();
        loop {
            let active = with_state(hwnd, |state| {
                state
                    .active_mpv_session
                    .as_ref()
                    .is_some_and(|session| session.process_id == process_id)
                    && state.active_mpv_subtitle_generation == generation
                    && state
                        .active_podcast_episode_url
                        .as_ref()
                        .is_some_and(|active_url| active_url == &path_text)
            })
            .unwrap_or(false);
            if !active {
                break;
            }

            let paused = query_managed_mpv_property_transient(hwnd, "pause")
                .ok()
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            if paused {
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }

            let text = query_managed_mpv_property_transient(hwnd, "sub-text")
                .ok()
                .and_then(|value| value.as_str().map(normalize_mpv_subtitle_text))
                .unwrap_or_default();
            if text.is_empty() {
                last_text.clear();
            } else if text != last_text {
                log_debug(&format!("mpv subtitle: {}", text.replace('\n', " ")));
                speak_mpv_subtitle_text(hwnd, text.clone());
                last_text = text;
            }
            std::thread::sleep(Duration::from_millis(60));
        }
    });
}

fn open_podcast_play_progress_window(hwnd: HWND, language: Language) {
    let labels = app_windows::podcast_save_window::SaveDialogLabels {
        title: i18n::tr(language, "podcast.save.title"),
        in_progress: i18n::tr(language, "podcasts.loading"),
        cancel: i18n::tr(language, "podcast.save.cancel"),
        cancel_confirm: i18n::tr(language, "podcast.cancel_confirm"),
    };
    let dialog = app_windows::podcast_save_window::open_with_labels(hwnd, language, labels, false);
    with_state(hwnd, |state| {
        state.podcast_save_window = dialog;
    });
}

fn open_podcast_save_progress_window(hwnd: HWND, language: Language) {
    let labels = app_windows::podcast_save_window::SaveDialogLabels {
        title: i18n::tr(language, "podcast.save.title"),
        in_progress: i18n::tr(language, "podcast.save.in_progress"),
        cancel: i18n::tr(language, "podcast.save.cancel"),
        cancel_confirm: i18n::tr(language, "podcast.cancel_confirm"),
    };
    let dialog = app_windows::podcast_save_window::open_with_labels(hwnd, language, labels, true);
    with_state(hwnd, |state| {
        state.podcast_save_window = dialog;
    });
}

fn update_podcast_save_progress_window(hwnd: HWND, pct: u32) {
    let dialog = with_state(hwnd, |state| state.podcast_save_window).unwrap_or(HWND(0));
    if dialog.0 != 0 {
        crate::send_message_w_safe(
            dialog,
            app_windows::podcast_save_window::WM_PODCAST_SAVE_PROGRESS,
            WPARAM(pct.min(100) as usize),
            LPARAM(0),
        );
    }
}

fn normalize_ffmpeg_progress_pct(raw_pct: u32) -> u32 {
    if raw_pct <= 100 {
        raw_pct
    } else {
        (raw_pct / 100).min(100)
    }
}

fn close_podcast_play_progress_window(hwnd: HWND) {
    let dialog = with_state(hwnd, |state| state.podcast_save_window).unwrap_or(HWND(0));
    if dialog.0 != 0 {
        crate::send_message_w_safe(
            dialog,
            app_windows::podcast_save_window::WM_PODCAST_SAVE_DONE,
            WPARAM(0),
            LPARAM(0),
        );
    }
    with_state(hwnd, |state| {
        state.podcast_save_window = HWND(0);
        state.podcast_save_cancel_token = None;
    });
}

fn close_podcast_save_progress_window(hwnd: HWND) {
    close_podcast_play_progress_window(hwnd);
}

fn play_podcast_episode_from_url_internal(
    hwnd: HWND,
    url: String,
    podcast_title: Option<String>,
    title: Option<String>,
    mime: Option<&str>,
    prefer_title_for_document: bool,
    rai_origin: RaiAudioOrigin,
) {
    let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
    let cache_path = podcast_cache_path_for_url(&url, mime, title.as_deref());
    screen_reader_speak(&i18n::tr(language, "podcasts.loading"));
    open_podcast_play_progress_window(hwnd, language);
    std::thread::spawn(move || {
        let cache_ok = is_usable_podcast_cache_file(&cache_path);
        if cache_path.exists() && !cache_ok {
            log_debug(&format!(
                "podcast cache invalid, removing before download: {}",
                cache_path.display()
            ));
            if let Err(err) = std::fs::remove_file(&cache_path) {
                log_debug(&format!(
                    "failed to remove invalid podcast cache {}: {}",
                    cache_path.display(),
                    err
                ));
            }
        }
        let result = if cache_ok {
            Ok(())
        } else {
            download_podcast_episode_cache_with_resume(&url, &cache_path, language)
        };

        match result {
            Ok(()) => post_podcast_episode_play_ready(
                hwnd,
                PodcastEpisodePlayReady {
                    url,
                    podcast_title,
                    title,
                    cache_path,
                    prefer_title_for_document,
                    rai_origin,
                },
            ),
            Err(error) => {
                post_podcast_episode_play_failed(hwnd, PodcastEpisodePlayFailed { language, error })
            }
        }
    });
}

fn download_podcast_episode_cache_with_resume(
    url: &str,
    cache_path: &Path,
    language: Language,
) -> Result<(), String> {
    const MAX_INVALID_MEDIA_ATTEMPTS: u32 = 3;

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let partial_file_path = podcast_partial_cache_path(cache_path);
    let mut last_reported_pct = 0u32;
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        let resume_from = std::fs::metadata(&partial_file_path)
            .map(|meta| meta.len())
            .unwrap_or(0);
        if resume_from > 0 {
            log_debug(&format!(
                "podcast_episode_save: resuming partial download from {} bytes for {}",
                resume_from, url
            ));
        }

        let result = crate::tools::rss::download_url_to_file_with_progress(
            url,
            &partial_file_path,
            resume_from,
            |pct| {
                let announced_pct = pct.min(90);
                if announced_pct >= last_reported_pct + 10 {
                    last_reported_pct = (announced_pct / 10) * 10;
                    let msg = i18n::tr_f(
                        language,
                        "podcasts.download_progress",
                        &[("pct", &last_reported_pct.to_string())],
                    );
                    screen_reader_speak(&msg);
                }
            },
        );

        match result {
            Ok(_) => {
                if !is_usable_podcast_cache_file(&partial_file_path) {
                    let invalid_size = partial_file_path
                        .metadata()
                        .map(|meta| meta.len())
                        .unwrap_or(0);
                    if let Err(err) = std::fs::remove_file(&partial_file_path) {
                        log_debug(&format!(
                            "failed to remove invalid partial podcast cache {}: {}",
                            partial_file_path.display(),
                            err
                        ));
                    }
                    if attempt < MAX_INVALID_MEDIA_ATTEMPTS {
                        log_debug(&format!(
                            "podcast_episode_save_invalid_media attempt={} url={} bytes={}; retrying from zero",
                            attempt, url, invalid_size
                        ));
                        last_reported_pct = 0;
                        std::thread::sleep(std::time::Duration::from_millis(
                            500u64 * attempt as u64,
                        ));
                        continue;
                    }
                    return Err(format!(
                        "downloaded response is not valid media after {} attempts ({} bytes)",
                        attempt, invalid_size
                    ));
                }
                std::fs::rename(&partial_file_path, cache_path).map_err(|err| {
                    format!(
                        "failed to finalize cache file {} from {}: {}",
                        cache_path.display(),
                        partial_file_path.display(),
                        err
                    )
                })?;
                if let Some(cache_dir) = cache_path.parent() {
                    let cache_limit_bytes = crate::settings::load_settings().podcast_cache_limit_mb
                        as u64
                        * 1024
                        * 1024;
                    crate::app_windows::podcasts_window::enforce_podcast_cache_limit(
                        cache_dir,
                        cache_limit_bytes,
                        Some(cache_path),
                    );
                }
                return Ok(());
            }
            Err(err) => {
                log_debug(&format!(
                    "podcast_episode_save_attempt_failed attempt={} url={} partial_bytes={} err={}",
                    attempt, url, resume_from, err
                ));
                if resume_from == 0 {
                    last_reported_pct = 0;
                }
                std::thread::sleep(std::time::Duration::from_millis(500u64 * attempt as u64));
                continue;
            }
        }
    }
}

pub(crate) fn download_podcast_episode(
    hwnd: HWND,
    url: Option<String>,
    podcast_title: Option<String>,
    title: Option<String>,
    cache_path: Option<PathBuf>,
    language: Language,
) {
    let suggested_name =
        suggested_podcast_episode_filename(podcast_title.as_deref(), title.as_deref())
            .or_else(|| {
                cache_path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .and_then(|s| s.to_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "podcast_episode".to_string());
    let mut ext = cache_path
        .as_ref()
        .and_then(|p| p.extension().and_then(|e| e.to_str()))
        .map(|e| e.to_string());
    if ext.is_none()
        && let Some(url) = url.as_deref()
    {
        ext = audio_extension_from_url(url);
    }
    if ext
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("m3u8"))
        .unwrap_or(false)
    {
        ext = Some("mp3".to_string());
    }
    let ext = ext.unwrap_or_else(|| "mp3".to_string());
    let suggested_full = format!("{}.{}", suggested_name, ext);
    let target = save_podcast_episode_dialog(hwnd, language, &suggested_full);
    let Some(target) = target else {
        return;
    };
    let target = ensure_path_extension(target, &ext);
    let cache_path = cache_path.clone();
    let url = url.clone();
    let selected_audio_track = with_state(hwnd, |state| state.selected_audio_track).flatten();
    let hwnd_copy = hwnd;
    std::thread::spawn(move || {
        let stream_url = url
            .as_deref()
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"));
        let can_export_stream_directly = cache_path
            .as_ref()
            .map(|path| is_direct_stream_url_path(path))
            .unwrap_or(false)
            || cache_path.is_none() && stream_url.is_some();

        if can_export_stream_directly {
            let Some(stream_url) = stream_url else {
                let err = "no source URL for direct stream export".to_string();
                log_debug(&format!("podcast_episode_save_failed {}", err));
                post_podcast_episode_save_result(
                    hwnd_copy,
                    PodcastEpisodeSaveResult {
                        language,
                        target_path: target.clone(),
                        error: Some(err),
                        open_audio_description: false,
                    },
                );
                return;
            };

            log_debug(&format!(
                "podcast_episode_save: exporting direct stream from {} to {}",
                stream_url,
                target.to_string_lossy()
            ));
            screen_reader_speak(&i18n::tr(language, "podcasts.loading"));
            let input_path = PathBuf::from(stream_url);
            let convert_settings = match convert_settings_for_save_target(&target) {
                Ok(settings) => settings,
                Err(err) => {
                    log_debug(&format!("podcast_episode_save_failed {}", err));
                    post_podcast_episode_save_result(
                        hwnd_copy,
                        PodcastEpisodeSaveResult {
                            language,
                            target_path: target.clone(),
                            error: Some(err),
                            open_audio_description: false,
                        },
                    );
                    return;
                }
            };
            let result = crate::ffmpeg_export::convert_audio_file_with_preferred_stream(
                &input_path,
                &target,
                &convert_settings,
                None,
                None,
                selected_audio_track,
            );
            match result {
                Ok(()) => {
                    log_debug(&format!(
                        "podcast_episode_saved src=stream dst={}",
                        target.to_string_lossy()
                    ));
                    post_podcast_episode_save_result(
                        hwnd_copy,
                        PodcastEpisodeSaveResult {
                            language,
                            target_path: target,
                            error: None,
                            open_audio_description: false,
                        },
                    );
                }
                Err(err) => {
                    let error = format!("stream export failed: {err}");
                    log_debug(&format!("podcast_episode_save_failed {}", error));
                    post_podcast_episode_save_result(
                        hwnd_copy,
                        PodcastEpisodeSaveResult {
                            language,
                            target_path: target.clone(),
                            error: Some(error),
                            open_audio_description: false,
                        },
                    );
                }
            }
            return;
        }

        let Some(cache_path) = cache_path.as_ref() else {
            let err = "no cache path".to_string();
            log_debug(&format!("podcast_episode_save_failed {}", err));
            post_podcast_episode_save_result(
                hwnd_copy,
                PodcastEpisodeSaveResult {
                    language,
                    target_path: target.clone(),
                    error: Some(err),
                    open_audio_description: false,
                },
            );
            return;
        };

        if !cache_path.exists() {
            let Some(url) = url.as_ref() else {
                let err = "no cache and no source URL".to_string();
                log_debug(&format!("podcast_episode_save_failed {}", err));
                post_podcast_episode_save_result(
                    hwnd_copy,
                    PodcastEpisodeSaveResult {
                        language,
                        target_path: target.clone(),
                        error: Some(err),
                        open_audio_description: false,
                    },
                );
                return;
            };
            log_debug(&format!(
                "podcast_episode_save: cache missing, downloading from {}",
                url
            ));
            screen_reader_speak(&i18n::tr(language, "podcasts.loading"));
            match download_podcast_episode_cache_with_resume(url, cache_path, language) {
                Ok(()) => {}
                Err(e) => {
                    let err = format!("download failed: {}", e);
                    log_debug(&format!("podcast_episode_save: {}", err));
                    post_podcast_episode_save_result(
                        hwnd_copy,
                        PodcastEpisodeSaveResult {
                            language,
                            target_path: target.clone(),
                            error: Some(err),
                            open_audio_description: false,
                        },
                    );
                    return;
                }
            }
        }

        if std::fs::copy(cache_path, &target).is_ok() {
            log_debug(&format!(
                "podcast_episode_saved src=cache dst={}",
                target.to_string_lossy()
            ));
            post_podcast_episode_save_result(
                hwnd_copy,
                PodcastEpisodeSaveResult {
                    language,
                    target_path: target,
                    error: None,
                    open_audio_description: false,
                },
            );
        } else {
            let err = format!("copy failed to dst={}", target.to_string_lossy());
            log_debug(&format!(
                "podcast_episode_save_failed copy dst={}",
                target.to_string_lossy()
            ));
            post_podcast_episode_save_result(
                hwnd_copy,
                PodcastEpisodeSaveResult {
                    language,
                    target_path: target,
                    error: Some(err),
                    open_audio_description: false,
                },
            );
        }
    });
}

fn convert_settings_for_save_target(
    target: &Path,
) -> Result<crate::ffmpeg_export::ConvertAudioSettings, String> {
    use crate::ffmpeg_export::{ConvertAudioFormat, ConvertAudioQuality, ConvertAudioSettings};

    let extension = target
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "m4a".to_string());

    let settings = match extension.as_str() {
        "mp3" => ConvertAudioSettings {
            format: ConvertAudioFormat::Mp3,
            quality: ConvertAudioQuality::BitrateKbps(192),
        },
        "m4a" | "aac" | "mp4" => ConvertAudioSettings {
            format: ConvertAudioFormat::Aac,
            quality: ConvertAudioQuality::BitrateKbps(192),
        },
        "opus" => ConvertAudioSettings {
            format: ConvertAudioFormat::Opus,
            quality: ConvertAudioQuality::BitrateKbps(160),
        },
        "ogg" => ConvertAudioSettings {
            format: ConvertAudioFormat::Ogg,
            quality: ConvertAudioQuality::OggQuality(5),
        },
        "flac" => ConvertAudioSettings {
            format: ConvertAudioFormat::Flac,
            quality: ConvertAudioQuality::FlacCompression(5),
        },
        "wav" => ConvertAudioSettings {
            format: ConvertAudioFormat::Wav,
            quality: ConvertAudioQuality::None,
        },
        "aiff" => ConvertAudioSettings {
            format: ConvertAudioFormat::Aiff,
            quality: ConvertAudioQuality::None,
        },
        other => {
            return Err(format!(
                "Formato di salvataggio non supportato per lo stream: {other}"
            ));
        }
    };

    Ok(settings)
}

fn ensure_path_extension(mut path: PathBuf, desired_ext: &str) -> PathBuf {
    let has_nonempty_extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| !e.trim().is_empty())
        .unwrap_or(false);
    if !has_nonempty_extension {
        path.set_extension(desired_ext);
    }
    path
}

fn replace_path_extension(mut path: PathBuf, desired_ext: &str) -> PathBuf {
    path.set_extension(desired_ext);
    path
}

fn choose_rai_episode_save_mode(
    hwnd: HWND,
    language: Language,
    rai_origin: RaiAudioOrigin,
    has_described_audio: bool,
) -> Option<RaiPlaySaveMode> {
    let (title, label) = if rai_origin == RaiAudioOrigin::La7Play {
        (
            crate::i18n::tr_la7_play("la7.save_format_title"),
            crate::i18n::tr_la7_play("la7.save_format_label"),
        )
    } else {
        match language {
            Language::Italian => (
                "Formato salvataggio RaiPlay".to_string(),
                "Seleziona il formato".to_string(),
            ),
            _ => (
                "RaiPlay Save Format".to_string(),
                "Select format".to_string(),
            ),
        }
    };
    let mut options = if rai_origin == RaiAudioOrigin::La7Play {
        vec![
            crate::i18n::tr_la7_play("la7.save_mp3"),
            crate::i18n::tr_la7_play("la7.save_mp4"),
        ]
    } else {
        vec!["MP3".to_string(), "MP4".to_string()]
    };
    if has_described_audio && rai_origin == RaiAudioOrigin::RaiPlay {
        options.push(match language {
            Language::Italian => "MP4 con audiodescrizione".to_string(),
            _ => "MP4 with described audio".to_string(),
        });
    }
    let selected = app_windows::youtube_transcript_window::choose_combo_option_dialog(
        hwnd, language, title, label, options, 0,
    )?;
    match selected {
        0 => Some(RaiPlaySaveMode::Mp3),
        1 => Some(RaiPlaySaveMode::Mp4),
        2 if has_described_audio && rai_origin == RaiAudioOrigin::RaiPlay => {
            Some(RaiPlaySaveMode::Mp4Described)
        }
        _ => None,
    }
}

fn suggested_podcast_episode_filename(
    podcast_title: Option<&str>,
    episode_title: Option<&str>,
) -> Option<String> {
    let podcast_title = podcast_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let episode_title = episode_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    match (podcast_title, episode_title) {
        (Some(podcast_title), Some(episode_title)) => {
            suggested_filename_from_text(&format!("{podcast_title} - {episode_title}"))
        }
        (_, Some(episode_title)) => suggested_filename_from_text(episode_title),
        (Some(podcast_title), None) => suggested_filename_from_text(podcast_title),
        (None, None) => None,
    }
}

fn audio_extension_from_url(url: &str) -> Option<String> {
    let clean_url = url.split('?').next().unwrap_or(url);
    Path::new(clean_url)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_string())
}

pub(crate) fn podcast_episode_display_title(
    podcast_title: Option<&str>,
    episode_title: Option<&str>,
) -> Option<String> {
    let podcast_title = podcast_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let episode_title = episode_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    match (podcast_title, episode_title) {
        (Some(podcast_title), Some(episode_title)) => {
            Some(format!("{podcast_title} - {episode_title}"))
        }
        (_, Some(episode_title)) => Some(episode_title.to_string()),
        (Some(podcast_title), None) => Some(podcast_title.to_string()),
        (None, None) => None,
    }
}

pub(crate) fn save_podcast_episode_dialog(
    hwnd: HWND,
    language: Language,
    suggested_name: &str,
) -> Option<PathBuf> {
    let initial_dir = with_state(hwnd, |state| state.settings.media_save_folder.clone())
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(settings::default_media_save_folder()));
    crate::log_if_err!(std::fs::create_dir_all(&initial_dir));
    unsafe {
        let pfd: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_ALL).ok()?;

        let filter_pairs = i18n::dialog_filter_pairs(language, "podcasts.download_filter");
        let name_wides: Vec<Vec<u16>> =
            filter_pairs.iter().map(|(name, _)| to_wide(name)).collect();
        let pattern_wides: Vec<Vec<u16>> = filter_pairs
            .iter()
            .map(|(_, pattern)| to_wide(pattern))
            .collect();
        let spec: Vec<COMDLG_FILTERSPEC> = name_wides
            .iter()
            .zip(pattern_wides.iter())
            .map(|(name, pattern)| COMDLG_FILTERSPEC {
                pszName: PCWSTR(name.as_ptr()),
                pszSpec: PCWSTR(pattern.as_ptr()),
            })
            .collect();
        pfd.SetFileTypes(&spec).ok()?;
        pfd.SetFileTypeIndex(1).ok()?;

        if let Some(default_ext) = Path::new(suggested_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .filter(|ext| !ext.trim().is_empty())
        {
            let default_ext_wide = to_wide(default_ext);
            pfd.SetDefaultExtension(PCWSTR(default_ext_wide.as_ptr()))
                .ok()?;
        }

        let initial_dir_wide = to_wide(&initial_dir.to_string_lossy());
        if let Ok(shell_folder) =
            SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(initial_dir_wide.as_ptr()), None)
        {
            let _unused = pfd.SetDefaultFolder(&shell_folder);
            let _unused = pfd.SetFolder(&shell_folder);
        }

        let default_name = Path::new(suggested_name)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(suggested_name);
        pfd.SetFileName(PCWSTR(to_wide(default_name).as_ptr()))
            .ok()?;

        pfd.Show(hwnd).ok()?;
        let result = pfd.GetResult().ok()?;
        let path = result
            .GetDisplayName(windows::Win32::UI::Shell::SIGDN_FILESYSPATH)
            .ok()?;
        Some(PathBuf::from(path.to_string().ok()?))
    }
}
pub(crate) fn prefetch_podcast_chapters(hwnd: HWND, key: String, url: String) {
    let should_fetch = {
        with_state(hwnd, |state| {
            !state.podcast_chapters_cache.contains_key(&key)
        })
        .unwrap_or(false)
    };
    if !should_fetch {
        return;
    }
    let config = {
        with_state(hwnd, |state| {
            crate::tools::rss::config_from_settings(&state.settings)
        })
        .unwrap_or_else(crate::tools::rss::RssHttpConfig::default)
    };
    if let Err(err) = crate::tools::rss::init_http(config) {
        log_debug(&format!("rss_http_init_error: {}", err));
    }
    let fetch_config = {
        with_state(hwnd, |state| {
            crate::tools::rss::fetch_config_from_settings(&state.settings)
        })
        .unwrap_or_else(crate::tools::rss::RssFetchConfig::default)
    };
    let fallback_url = extract_embedded_chapters_url(&url);
    let hwnd_copy = hwnd;
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                log_debug(&format!("Failed to build tokio runtime: {}", e));
                return;
            }
        };
        let chapters = fetch_chapters_with_fallback(&rt, &url, &fallback_url, fetch_config);
        let msg = Box::new(PodcastChaptersReady { key, chapters });
        unsafe {
            crate::log_if_err!(PostMessageW(
                hwnd_copy,
                WM_PODCAST_CHAPTERS_READY,
                WPARAM(0),
                LPARAM(Box::into_raw(msg) as isize),
            ));
        }
    });
}

pub(crate) fn prefetch_podcast_chapters_from_file(hwnd: HWND, key: String, path: PathBuf) {
    let should_fetch = with_state(hwnd, |state| {
        !state.podcast_chapters_cache.contains_key(&key)
    })
    .unwrap_or(false);
    if !should_fetch {
        return;
    }

    let hwnd_copy = hwnd;
    std::thread::spawn(move || {
        let parsed = crate::podcast::chapters::parse_embedded_chapters_from_media(&path);
        let chapters = if parsed.is_empty() {
            None
        } else {
            Some(parsed)
        };
        if let Some(ref list) = chapters {
            log_debug(&format!(
                "podcast_chapters_local_ok key={} path={} count={}",
                key,
                path.display(),
                list.len()
            ));
        }
        let msg = Box::new(PodcastChaptersReady { key, chapters });
        crate::log_if_err!(crate::post_message_w_safe(
            hwnd_copy,
            WM_PODCAST_CHAPTERS_READY,
            WPARAM(0),
            LPARAM(Box::into_raw(msg) as isize),
        ));
    });
}

pub(crate) fn local_media_chapters_key(path: &Path) -> String {
    format!("file_chapters:{}", path.to_string_lossy())
}

pub(crate) fn cache_podcast_chapters(hwnd: HWND, key: String, chapters: Vec<Chapter>) {
    with_state(hwnd, |state| {
        state.podcast_chapters_cache.insert(key, Some(chapters));
    });
}

fn fetch_chapters_with_fallback(
    rt: &tokio::runtime::Runtime,
    url: &str,
    fallback_url: &Option<String>,
    fetch_config: crate::tools::rss::RssFetchConfig,
) -> Option<Vec<Chapter>> {
    match rt.block_on(crate::tools::rss::fetch_url_bytes(url, fetch_config)) {
        Ok(bytes) => {
            let parsed = crate::podcast::chapters::parse_chapters_json(&bytes);
            if !parsed.is_empty() {
                log_debug(&format!(
                    "podcast_chapters_ok url={} count={}",
                    url,
                    parsed.len()
                ));
                return Some(parsed);
            }
            log_debug(&format!("podcast_chapters_empty url={}", url));
        }
        Err(err) => {
            log_debug(&format!("podcast_chapters_fetch_error {}", err));
        }
    }
    let fallback_url = fallback_url.as_ref()?;
    match rt.block_on(crate::tools::rss::fetch_url_bytes(
        fallback_url,
        fetch_config,
    )) {
        Ok(bytes) => {
            let parsed = crate::podcast::chapters::parse_chapters_json(&bytes);
            if parsed.is_empty() {
                log_debug(&format!("podcast_chapters_empty url={}", fallback_url));
                None
            } else {
                log_debug(&format!(
                    "podcast_chapters_ok url={} count={}",
                    fallback_url,
                    parsed.len()
                ));
                Some(parsed)
            }
        }
        Err(err) => {
            log_debug(&format!("podcast_chapters_fetch_error {}", err));
            None
        }
    }
}

pub(crate) fn extract_embedded_chapters_url(url: &str) -> Option<String> {
    let marker = "/chapters/";
    let idx = url.rfind(marker)?;
    let tail = &url[idx + marker.len()..];
    if tail.starts_with("http://") || tail.starts_with("https://") {
        Some(tail.to_string())
    } else {
        None
    }
}

pub(crate) fn extract_buzzsprout_chapters_url(url: &str) -> Option<String> {
    // Supports direct Buzzsprout URLs and wrapped URLs (e.g. op3.dev/.../www.buzzsprout.com/...).
    // Expected pattern: ".../<show_id>/episodes/<episode_id>-...".
    let marker = "/episodes/";
    let episode_idx = url.find(marker)?;
    let before = &url[..episode_idx];
    let show_id = before
        .rsplit('/')
        .find(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_digit()))?;
    let after = &url[episode_idx + marker.len()..];
    let episode_id: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    if episode_id.is_empty() {
        return None;
    }
    Some(format!(
        "https://www.buzzsprout.com/{show_id}/{episode_id}/chapters.json"
    ))
}

fn announce_player_time(hwnd: HWND) {
    const END_STOP_TOLERANCE_SECS: u64 = 1;

    let (current_raw, path, fallback_total, fallback_is_stopped_same_path, language) = {
        with_state(hwnd, |state| {
            let current = state
                .active_audiobook
                .as_ref()
                .map(|player| audiobook_position_secs(player).max(0.0).floor() as u64);
            let active_path = state
                .active_audiobook
                .as_ref()
                .map(|player| player.path.clone());
            let doc_path = if active_path.is_none() {
                state.docs.get(state.current).and_then(|doc| {
                    if matches!(doc.format, FileFormat::Audiobook) {
                        doc.path.clone()
                    } else {
                        None
                    }
                })
            } else {
                None
            };
            let path = active_path.or(doc_path);
            let fallback_total = state.active_audiobook.as_ref().and_then(|player| {
                player
                    .duration_secs()
                    .map(|secs| secs.max(0.0).floor() as u64)
            });
            let fallback_is_stopped_same_path = path
                .as_ref()
                .zip(state.last_stopped_audiobook.as_ref())
                .map(|(p, stopped)| p == stopped)
                .unwrap_or(false);
            (
                current,
                path,
                fallback_total,
                fallback_is_stopped_same_path,
                state.settings.language,
            )
        })
    }
    .unwrap_or((None, None, None, false, Language::default()));
    let Some(path) = path else {
        return;
    };

    let total = audiobook_duration_secs(&path).or(fallback_total);
    let Some(current_raw) = current_raw.or_else(|| {
        if fallback_is_stopped_same_path {
            // After reaching EOF and stopping, treat time as restarted from 0:00.
            Some(0)
        } else {
            total.map(|_| 0)
        }
    }) else {
        return;
    };
    let (current, should_stop) = if let Some(total) = total {
        let overrun = current_raw > total;
        if overrun {
            log_debug(&format!(
                "Audio player: position beyond duration (current={} total={}), clamping",
                current_raw, total
            ));
        }
        (
            current_raw.min(total),
            overrun && current_raw >= total.saturating_add(END_STOP_TOLERANCE_SECS),
        )
    } else {
        (current_raw, false)
    };

    let current_str = format_time_hms(current);
    let message = if let Some(total) = total {
        let total_str = format_time_hms(total);
        i18n::tr_f(
            language,
            "player.time_announce",
            &[("current", &current_str), ("total", &total_str)],
        )
    } else {
        i18n::tr_f(
            language,
            "player.time_announce_no_total",
            &[("current", &current_str)],
        )
    };
    nvda_speak(&message);
    if should_stop {
        stop_audiobook_playback(hwnd);
    }
}

fn announce_player_pitch(language: Language, pitch: f32) {
    let pitch_text = format!("{:+.0}", pitch);
    let message = i18n::tr_f(language, "player.pitch_announce", &[("pitch", &pitch_text)]);
    crate::accessibility::nvda_speak(&message);
}

fn announce_player_volume(hwnd: HWND) {
    let volume = crate::audio_player::audiobook_volume_level(hwnd);
    let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
    let Some(volume) = volume else {
        return;
    };
    let percent = (volume * 100.0).round().clamp(0.0, 300.0) as u32;
    let message = i18n::tr_f(
        language,
        "player.volume_announce",
        &[("pct", &percent.to_string())],
    );
    nvda_speak(&message);
}

fn announce_mpv_volume(hwnd: HWND) {
    let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
    let is_muted = query_managed_mpv_property(hwnd, "mute")
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let volume = match query_managed_mpv_property(hwnd, "volume")
        .ok()
        .and_then(|value| value.as_f64())
    {
        Some(volume) => {
            let volume = volume as f32;
            if with_state(hwnd, |state| {
                if let Some(status) = state.active_mpv_status.as_mut() {
                    status.volume = volume;
                }
            })
            .is_none()
            {
                log_debug("Failed to persist managed mpv volume state");
            }
            if !is_muted {
                persist_media_playback_volume(hwnd, volume / 100.0);
            }
            volume
        }
        None => with_state(hwnd, |state| {
            state.active_mpv_status.as_ref().map(|s| s.volume)
        })
        .flatten()
        .unwrap_or(100.0),
    };
    let percent = if is_muted {
        0
    } else {
        volume.round().clamp(0.0, 300.0) as u32
    };
    let message = i18n::tr_f(
        language,
        "player.volume_announce",
        &[("pct", &percent.to_string())],
    );
    nvda_speak(&message);
}

fn announce_mpv_time(hwnd: HWND) -> Result<(), String> {
    let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
    let current = query_managed_mpv_property(hwnd, "time-pos")?;
    let total = query_managed_mpv_property(hwnd, "duration").ok();
    let current_secs = current.as_f64().unwrap_or(0.0).max(0.0).floor() as u64;
    let current_str = format_time_hms(current_secs);
    let message = if let Some(total_secs) = total
        .and_then(|value| value.as_f64())
        .map(|value| value.max(0.0).floor() as u64)
    {
        let total_str = format_time_hms(total_secs);
        i18n::tr_f(
            language,
            "player.time_announce",
            &[("current", &current_str), ("total", &total_str)],
        )
    } else {
        i18n::tr_f(
            language,
            "player.time_announce_no_total",
            &[("current", &current_str)],
        )
    };
    nvda_speak(&message);
    Ok(())
}

fn sync_mpv_speed_status(hwnd: HWND) -> Option<f32> {
    let speed = query_managed_mpv_property(hwnd, "speed")
        .ok()
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)?;
    if with_state(hwnd, |state| {
        if let Some(status) = state.active_mpv_status.as_mut() {
            status.speed = speed;
        }
    })
    .is_none()
    {
        log_debug("Failed to persist managed mpv speed state");
    }
    Some(speed)
}

fn extract_mpv_rubberband_pitch_scale(value: &serde_json::Value) -> Option<f32> {
    match value {
        serde_json::Value::String(text) => {
            let marker = "rubberband=pitch=";
            let start = text.find(marker)? + marker.len();
            let tail = &text[start..];
            let number: String = tail
                .chars()
                .take_while(|ch| ch.is_ascii_digit() || matches!(ch, '+' | '-' | '.' | 'e' | 'E'))
                .collect();
            number.parse::<f32>().ok()
        }
        serde_json::Value::Array(items) => {
            items.iter().find_map(extract_mpv_rubberband_pitch_scale)
        }
        serde_json::Value::Object(map) => map.values().find_map(extract_mpv_rubberband_pitch_scale),
        _ => None,
    }
}

fn sync_mpv_pitch_status(hwnd: HWND) -> Option<f32> {
    let filters = query_managed_mpv_property(hwnd, "af").ok()?;
    let pitch = extract_mpv_rubberband_pitch_scale(&filters)
        .map(|scale| 12.0 * scale.log2())
        .unwrap_or(0.0);
    if with_state(hwnd, |state| {
        if let Some(status) = state.active_mpv_status.as_mut() {
            status.pitch = pitch;
        }
    })
    .is_none()
    {
        log_debug("Failed to persist managed mpv pitch state");
    }
    Some(pitch)
}

fn apply_mpv_pitch(hwnd: HWND, pitch: f32) -> Result<(), String> {
    let scale = 2f32.powf(pitch / 12.0);
    if pitch.abs() < f32::EPSILON {
        try_send_command_to_managed_mpv(hwnd, r#"{"command":["af","clr",""]}"#)
    } else {
        try_send_command_to_managed_mpv(
            hwnd,
            &format!(
                r#"{{"command":["af","set","lavfi=[rubberband=pitch={:.6}]"]}}"#,
                scale
            ),
        )
    }
}

fn announce_player_speed(language: Language, speed: f32) {
    let scaled = (speed * 10.0).round() / 10.0;
    let speed_text = if (scaled.fract() - 0.0).abs() < f32::EPSILON {
        format!("{:.0}", scaled)
    } else {
        format!("{:.1}", scaled)
    };
    let message = i18n::tr_f(language, "player.speed_announce", &[("speed", &speed_text)]);
    nvda_speak(&message);
}

fn announce_chapters_unavailable(language: Language) {
    let message = i18n::tr(language, "playback.chapters_unavailable");
    nvda_speak(&message);
}

fn seek_to_chapter_index(hwnd: HWND, chapters: &[Chapter], index: usize) {
    let Some(chapter) = chapters.get(index) else {
        return;
    };
    log_debug(&format!(
        "podcast_chapter_seek index={} start_ms={} title={}",
        index, chapter.start_ms, chapter.title
    ));
    let target_secs = chapter.start_ms.saturating_add(999) / 1000;
    {
        match seek_audiobook_to(hwnd, target_secs) {
            Ok(()) => {
                let language = if let Some(lang) = with_state(hwnd, |state| {
                    state.last_announced_chapter_index = Some(index);
                    state.settings.language
                }) {
                    lang
                } else {
                    crate::log_debug("Failed to update last announced chapter index after seek");
                    Language::default()
                };
                // Announce target chapter immediately; avoid recomputing from position too early
                // because some backends can report the old position for a short time after seek.
                let message = i18n::tr_f(
                    language,
                    "playback.chapter_announce",
                    &[("title", &chapter.title)],
                );
                nvda_speak(&message);
            }
            Err(err) => crate::log_debug(&format!("podcast_chapter_seek_error {}", err)),
        }
    }
}

fn handle_chapter_navigation(hwnd: HWND, direction: i32) {
    let (chapters, language, current_pos_ms, last_idx) = {
        with_state(hwnd, |state| {
            (
                state.active_podcast_chapters.clone(),
                state.settings.language,
                audiobook_position_ms_from_state(state),
                state.last_announced_chapter_index,
            )
        })
    }
    .unwrap_or((Vec::new(), Language::default(), None, None));
    if chapters.is_empty() {
        announce_chapters_unavailable(language);
        return;
    }
    let current_idx = match current_pos_ms {
        Some(pos)
            if chapters
                .first()
                .is_some_and(|chapter| pos < chapter.start_ms) =>
        {
            None
        }
        Some(pos) => crate::podcast::chapters::current_chapter_index(pos, &chapters).or(last_idx),
        None => last_idx,
    };
    if direction < 0 && current_idx.is_none_or(|idx| idx == 0) {
        {
            match seek_audiobook_to(hwnd, 0) {
                Ok(()) => {
                    if with_state(hwnd, |state| {
                        // Intro segment before first chapter: no active chapter index.
                        state.last_announced_chapter_index = None;
                    })
                    .is_none()
                    {
                        crate::log_debug(
                            "Failed to clear last announced chapter index after intro seek",
                        );
                    }
                }
                Err(err) => crate::log_debug(&format!("podcast_chapter_intro_seek_error {}", err)),
            }
        }
        return;
    }

    let target = if direction > 0 {
        match current_idx {
            Some(idx) if idx + 1 < chapters.len() => Some(idx + 1),
            None => Some(0),
            _ => None,
        }
    } else {
        match current_idx {
            Some(idx) if idx > 0 => Some(idx - 1),
            _ => Some(0),
        }
    };
    if let Some(index) = target {
        seek_to_chapter_index(hwnd, &chapters, index);
    }
}

fn handle_chapter_list(hwnd: HWND) {
    let (chapters, language) = {
        with_state(hwnd, |state| {
            (
                state.active_podcast_chapters.clone(),
                state.settings.language,
            )
        })
    }
    .unwrap_or((Vec::new(), Language::default()));
    if chapters.is_empty() {
        announce_chapters_unavailable(language);
        return;
    }
    if let Some(index) =
        app_windows::podcast_chapters_window::select_chapter(hwnd, &chapters, language)
    {
        seek_to_chapter_index(hwnd, &chapters, index);
    }
}

fn is_direct_stream_url_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.starts_with("http://") || s.starts_with("https://")
}

pub(crate) fn is_local_cached_media_path(path: &Path) -> bool {
    if is_direct_stream_url_path(path) || !path.starts_with(settings::settings_dir()) {
        return false;
    }

    path.ancestors().any(|ancestor| {
        ancestor
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_ascii_lowercase().contains("cache"))
            .unwrap_or(false)
    })
}

pub(crate) fn current_playback_media_path(hwnd: HWND) -> Option<PathBuf> {
    with_state(hwnd, |state| {
        if let Some(player) = state.active_audiobook.as_ref() {
            return Some(player.path.clone());
        }
        state.docs.get(state.current).and_then(|doc| {
            if matches!(doc.format, crate::settings::FileFormat::Audiobook) {
                doc.path.clone()
            } else {
                None
            }
        })
    })
    .flatten()
}

pub(crate) fn current_local_playback_media_path(hwnd: HWND) -> Option<PathBuf> {
    let path = current_playback_media_path(hwnd)?;
    if is_direct_stream_url_path(&path) || is_local_cached_media_path(&path) || !path.is_file() {
        return None;
    }
    Some(path)
}

pub(crate) fn finish_audio_description_after_output_preview(
    hwnd: HWND,
    source_player_path: Option<&Path>,
) {
    log_debug(&format!(
        "Audio description: finishing output preview workflow source_player_path={}",
        source_player_path
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));

    if is_mpv_playback_active(hwnd) {
        stop_managed_mpv_playback(hwnd);
    }

    let source_index = source_player_path.and_then(|source_path| {
        with_state(hwnd, |state| {
            state.docs.iter().position(|doc| {
                matches!(doc.format, FileFormat::Audiobook)
                    && doc.path.as_deref() == Some(source_path)
            })
        })
        .flatten()
    });

    if let Some(index) = source_index {
        log_debug(&format!(
            "Audio description: closing source player document at index {index}"
        ));
        if !editor_manager::close_document_at(hwnd, index) {
            log_debug("Audio description: failed to close source player document");
        }
    }

    clear_active_podcast_chapters(hwnd);
    clear_active_youtube_return_context(hwnd);

    // The audio-description window is still inside WM_DESTROY when this helper runs.
    // Do not hand focus back synchronously: Windows/NVDA can otherwise keep a stale
    // modal/accessibility state even though the edit control already owns focus.
    recover_main_window_after_audio_description(hwnd, "output_preview_cleanup");
    if with_state(hwnd, |state| {
        state.alt_menu_suppressed = false;
        state.alt_menu_used_with_key = false;
        state.local_mpv_alt_menu_pending = false;
    })
    .is_none()
    {
        log_debug("Audio description: app state unavailable while resetting Alt/menu state");
    }
    unsafe {
        crate::log_if_err!(DrawMenuBar(hwnd));
    }
    log_debug("Audio description: scheduling deferred editor focus after preview cleanup");
    crate::log_if_err!(post_message_w_safe(
        hwnd,
        WM_FOCUS_EDITOR,
        WPARAM(0),
        LPARAM(0)
    ));
}

fn pause_active_playback_for_audio_description(hwnd: HWND) {
    if is_mpv_playback_active(hwnd) {
        match try_send_command_to_managed_mpv(hwnd, r#"{"command":["set_property","pause",true]}"#)
        {
            Ok(()) => {
                log_debug("Audio description shortcut: paused active mpv playback");
                stop_mpv_subtitle_speech(hwnd, "audio_description_start");
                sync_mpv_sleep_prevention(hwnd);
            }
            Err(err) => {
                log_debug(&format!(
                    "Audio description shortcut: failed to pause active mpv playback: {}",
                    err
                ));
            }
        }
        return;
    }

    if crate::audio_player::pause_audiobook_if_playing(hwnd) {
        log_debug("Audio description shortcut: paused active BASS playback");
    }
}

pub(crate) fn post_audio_description_for_completed_tv_recording(hwnd: HWND, path: &Path) -> bool {
    let payload = Box::new(path.to_path_buf());
    let raw = Box::into_raw(payload);
    let posted = unsafe {
        PostMessageW(
            hwnd,
            WM_OPEN_AUDIO_DESCRIPTION_FOR_TV_RECORDING,
            WPARAM(0),
            LPARAM(raw as isize),
        )
    };
    if let Err(error) = posted {
        let reclaimed_payload = unsafe { Box::from_raw(raw) };
        log_debug(&format!(
            "Unable to post completed TV recording {} to audio-description window: {}",
            reclaimed_payload.display(),
            error
        ));
        false
    } else {
        true
    }
}

pub(crate) fn queue_youtube_context_audio_description(hwnd: HWND, input_path: PathBuf) {
    app_windows::youtube_transcript_window::mark_context_audio_description_started(hwnd);
    let selector_close_requested =
        app_windows::interpreter_select_window::request_close_active_interpreter_selects_for_parent(
            hwnd,
        );
    if !selector_close_requested {
        log_debug(
            "YouTube context audio description: no active interpreter selector needed closing",
        );
    }

    let payload = DeferredYoutubeContextAudioDescription { input_path };
    let ptr = Box::into_raw(Box::new(payload));
    if let Err(err) = post_message_w_safe(
        hwnd,
        WM_START_YOUTUBE_CONTEXT_AUDIO_DESCRIPTION,
        WPARAM(0),
        LPARAM(ptr as isize),
    ) {
        log_debug(&format!(
            "Failed to post WM_START_YOUTUBE_CONTEXT_AUDIO_DESCRIPTION: {err}"
        ));
        let DeferredYoutubeContextAudioDescription { input_path } = *unsafe { Box::from_raw(ptr) };
        enable_window_safe(hwnd, true);
        app_windows::audio_description_window::open_with_input(hwnd, input_path);
        protect_youtube_context_audio_description_focus(hwnd);
    }
}

fn restore_youtube_context_audio_description_focus(hwnd: HWND) -> bool {
    if !app_windows::youtube_transcript_window::context_audio_description_keeps_window_foreground(
        hwnd,
    ) {
        return false;
    }

    let window = app_windows::audio_description_window::visible_window(hwnd);
    if window.0 != 0 && is_window_handle_valid(window) {
        show_window_safe(window, SW_SHOW);
        set_foreground_window_safe(window);
        send_message_w_safe(window, WM_SETFOCUS, WPARAM(0), LPARAM(0));
        log_debug(&format!(
            "YouTube context audio description: keeping creation window in foreground window={:?}",
            window
        ));
    }
    true
}

fn protect_youtube_context_audio_description_focus(hwnd: HWND) {
    restore_youtube_context_audio_description_focus(hwnd);
    let timer = unsafe {
        SetTimer(
            hwnd,
            YOUTUBE_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE_TIMER_ID,
            750,
            None,
        )
    };
    if timer == 0 {
        log_debug("Failed to set YouTube context audio-description focus-release timer");
        app_windows::youtube_transcript_window::finish_context_audio_description(hwnd);
    }
}

pub(crate) fn queue_raiplay_context_audio_description(hwnd: HWND, input_path: PathBuf) {
    let payload = DeferredRaiplayContextAudioDescription { input_path };
    let ptr = Box::into_raw(Box::new(payload));
    if let Err(err) = post_message_w_safe(
        hwnd,
        WM_START_RAIPLAY_CONTEXT_AUDIO_DESCRIPTION,
        WPARAM(0),
        LPARAM(ptr as isize),
    ) {
        log_debug(&format!(
            "Failed to post WM_START_RAIPLAY_CONTEXT_AUDIO_DESCRIPTION: {err}"
        ));
        let DeferredRaiplayContextAudioDescription { input_path } = *unsafe { Box::from_raw(ptr) };
        enable_window_safe(hwnd, true);
        app_windows::audio_description_window::open_with_input(hwnd, input_path);
        protect_raiplay_context_audio_description_focus(hwnd);
    }
}

fn restore_raiplay_context_audio_description_focus(hwnd: HWND) -> bool {
    if !app_windows::raiplay_window::context_audio_description_keeps_window_foreground(hwnd) {
        return false;
    }
    let window = app_windows::audio_description_window::visible_window(hwnd);
    if window.0 != 0 && is_window_handle_valid(window) {
        show_window_safe(window, SW_SHOW);
        set_foreground_window_safe(window);
        send_message_w_safe(window, WM_SETFOCUS, WPARAM(0), LPARAM(0));
        log_debug(&format!(
            "RaiPlay context audio description: keeping creation window in foreground window={:?}",
            window
        ));
    }
    true
}

fn protect_raiplay_context_audio_description_focus(hwnd: HWND) {
    restore_raiplay_context_audio_description_focus(hwnd);
    let timer = unsafe {
        SetTimer(
            hwnd,
            RAIPLAY_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE_TIMER_ID,
            750,
            None,
        )
    };
    if timer == 0 {
        log_debug("Failed to set RaiPlay context audio-description focus-release timer");
        app_windows::raiplay_window::finish_context_audio_description_focus(hwnd);
    }
}

pub(crate) fn queue_la7_context_audio_description(hwnd: HWND, input_path: PathBuf) {
    let payload = DeferredLa7ContextAudioDescription { input_path };
    let ptr = Box::into_raw(Box::new(payload));
    if let Err(err) = post_message_w_safe(
        hwnd,
        WM_START_LA7_CONTEXT_AUDIO_DESCRIPTION,
        WPARAM(0),
        LPARAM(ptr as isize),
    ) {
        log_debug(&format!(
            "Failed to post WM_START_LA7_CONTEXT_AUDIO_DESCRIPTION: {err}"
        ));
        let DeferredLa7ContextAudioDescription { input_path } = *unsafe { Box::from_raw(ptr) };
        enable_window_safe(hwnd, true);
        app_windows::audio_description_window::open_with_input(hwnd, input_path);
        protect_la7_context_audio_description_focus(hwnd);
    }
}

fn restore_la7_context_audio_description_focus(hwnd: HWND) -> bool {
    if !app_windows::la7_play_window::context_audio_description_keeps_window_foreground(hwnd) {
        return false;
    }
    let window = app_windows::audio_description_window::visible_window(hwnd);
    if window.0 != 0 && is_window_handle_valid(window) {
        show_window_safe(window, SW_SHOW);
        set_foreground_window_safe(window);
        send_message_w_safe(window, WM_SETFOCUS, WPARAM(0), LPARAM(0));
        log_debug(&format!(
            "La7 Play context audio description: keeping creation window in foreground window={:?}",
            window
        ));
    }
    true
}

fn protect_la7_context_audio_description_focus(hwnd: HWND) {
    restore_la7_context_audio_description_focus(hwnd);
    let timer = unsafe {
        SetTimer(
            hwnd,
            LA7_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE_TIMER_ID,
            750,
            None,
        )
    };
    if timer == 0 {
        log_debug("Failed to set La7 Play context audio-description focus-release timer");
        app_windows::la7_play_window::finish_context_audio_description_focus(hwnd);
    }
}

fn open_audio_description_from_current_context(hwnd: HWND) {
    if let Some(path) = current_local_playback_media_path(hwnd)
        .filter(|path| file_handler::is_video_path(path.as_path()))
    {
        log_debug(&format!(
            "Audio description shortcut: opening local video {}",
            path.display()
        ));
        pause_active_playback_for_audio_description(hwnd);
        app_windows::audio_description_window::open_with_input(hwnd, path);
        return;
    }

    if download_active_rai_la7_for_audio_description(hwnd) {
        return;
    }

    let (active_url, language) = with_state(hwnd, |state| {
        (
            state.active_podcast_episode_url.clone(),
            state.settings.language,
        )
    })
    .unwrap_or((None, Language::default()));

    if let Some(active_url) = active_url.as_deref()
        && app_windows::youtube_transcript_window::can_create_audio_description_from_active_stream(
            active_url,
        )
    {
        log_debug("Audio description shortcut: downloading active yt-dlp video stream");
        pause_active_playback_for_audio_description(hwnd);
        app_windows::youtube_transcript_window::download_active_streaming_video_for_audio_description(
            hwnd,
            active_url,
            language,
        );
        return;
    }

    if let Some(active_url) = active_url.filter(|url| {
        app_windows::youtube_transcript_window::can_create_audio_description_from_active_youtube(
            url,
        )
    }) {
        log_debug("Audio description shortcut: downloading active YouTube video");
        pause_active_playback_for_audio_description(hwnd);
        app_windows::youtube_transcript_window::download_active_youtube_for_audio_description(
            hwnd,
            &active_url,
            language,
        );
        return;
    }

    log_debug("Audio description shortcut: opening empty window");
    app_windows::audio_description_window::open(hwnd);
}

fn restore_rai_audio_context_save_progress_focus(hwnd: HWND) -> bool {
    if !app_windows::rai_audiodescrizioni_window::context_save_keeps_progress_foreground(hwnd) {
        return false;
    }
    let progress_hwnd = with_state(hwnd, |state| state.podcast_save_window).unwrap_or(HWND(0));
    if progress_hwnd.0 != 0 && is_window_handle_valid(progress_hwnd) {
        show_window_safe(progress_hwnd, SW_SHOW);
        set_foreground_window_safe(progress_hwnd);
        app_windows::podcast_save_window::focus_cancel_button(progress_hwnd);
        log_debug(&format!(
            "Rai audiodescrizioni context save: keeping progress in foreground dialog={:?}",
            progress_hwnd
        ));
    }
    true
}

fn restore_context_transcription_progress_focus(hwnd: HWND) -> bool {
    let youtube_context =
        app_windows::youtube_transcript_window::context_transcription_keeps_progress_foreground(
            hwnd,
        );
    let la7_context =
        app_windows::la7_play_window::context_transcription_keeps_progress_foreground(hwnd);
    let rai_audio_context =
        app_windows::rai_audiodescrizioni_window::context_transcription_keeps_progress_foreground(
            hwnd,
        );
    if !youtube_context && !la7_context && !rai_audio_context {
        return false;
    }

    let (progress_hwnd, transcription_in_progress) = with_state(hwnd, |state| {
        (
            state.transcription_progress_window,
            state.transcription_in_progress,
        )
    })
    .unwrap_or((HWND(0), false));

    // The YouTube selector is closed before the deferred Whisper start message is
    // handled. During that short handoff, suppress any stale editor-focus retry
    // rather than flashing the empty editor in front of the upcoming progress dialog.
    if !transcription_in_progress {
        return true;
    }

    if progress_hwnd.0 != 0 && is_window_handle_valid(progress_hwnd) {
        show_window_safe(progress_hwnd, SW_SHOW);
        set_foreground_window_safe(progress_hwnd);
        app_windows::podcast_save_window::focus_cancel_button(progress_hwnd);
        log_debug(&format!(
            "Context transcription: keeping Whisper progress in foreground dialog={:?}",
            progress_hwnd
        ));
    }
    true
}

pub(crate) fn restore_transcription_progress_focus_for_current_document(hwnd: HWND) -> bool {
    let (
        progress_hwnd,
        transcription_in_progress,
        transcription_media_path,
        current_doc_path,
        active_podcast_episode_url,
        mpv_active,
        tab_hwnd,
        focused_hwnd,
    ) = with_state(hwnd, |state| {
        let current_doc_path = state.docs.get(state.current).and_then(|doc| {
            if matches!(doc.format, FileFormat::Audiobook) {
                doc.path.clone()
            } else {
                None
            }
        });
        (
            state.transcription_progress_window,
            state.transcription_in_progress,
            state.transcription_media_path.clone(),
            current_doc_path,
            state.active_podcast_episode_url.clone(),
            state.active_mpv_session.is_some(),
            state.hwnd_tab,
            crate::get_focus_safe(),
        )
    })
    .unwrap_or((HWND(0), false, None, None, None, false, HWND(0), HWND(0)));

    if !transcription_in_progress || progress_hwnd.0 == 0 || !is_window_handle_valid(progress_hwnd)
    {
        return false;
    }

    if focused_hwnd.0 != 0
        && is_window_handle_valid(focused_hwnd)
        && focused_hwnd != hwnd
        && focused_hwnd != tab_hwnd
    {
        return false;
    }

    let Some(transcription_media_path) = transcription_media_path else {
        return false;
    };
    let Some(current_doc_path) = current_doc_path else {
        return false;
    };

    let current_matches_transcription = current_doc_path == transcription_media_path;
    let current_matches_active_stream = mpv_active
        && is_stream_cache_media(&transcription_media_path)
        && active_podcast_episode_url
            .as_ref()
            .is_some_and(|url| current_doc_path.to_string_lossy() == url.as_str());

    if !current_matches_transcription && !current_matches_active_stream {
        return false;
    }

    show_window_safe(progress_hwnd, SW_SHOW);
    set_foreground_window_safe(progress_hwnd);
    app_windows::podcast_save_window::focus_cancel_button(progress_hwnd);
    true
}

fn is_stream_cache_media(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name.starts_with("stream_") {
        return true;
    }
    // Streaming downloads can now be renamed to media title, so they may not keep
    // the old "stream_*" prefix. Treat WebM files in podcast cache as stream media.
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("webm"))
    {
        let full = path.to_string_lossy().to_ascii_lowercase();
        return full.contains("\\podcast cache\\");
    }
    false
}

fn supports_direct_whisper_input(path: &Path, stream_index: Option<i32>) -> bool {
    if is_direct_stream_url_path(path) {
        return false;
    }
    // If the user selected a non-default track, keep ffmpeg conversion path to preserve track choice.
    if stream_index.is_some_and(|idx| idx > 0) {
        return false;
    }
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "wav"
            | "mp3"
            | "flac"
            | "ogg"
            | "opus"
            | "m4a"
            | "aac"
            | "wma"
            | "webm"
            | "mp4"
            | "mkv"
            | "mov"
    )
}

fn is_direct_stream_playback_active(hwnd: HWND) -> bool {
    {
        with_state(hwnd, |state| {
            if let Some(player) = state.active_audiobook.as_ref()
                && is_direct_stream_url_path(&player.path)
            {
                return true;
            }
            state
                .docs
                .get(state.current)
                .and_then(|doc| {
                    if matches!(doc.format, FileFormat::Audiobook) {
                        doc.path.as_ref()
                    } else {
                        None
                    }
                })
                .is_some_and(|path| is_direct_stream_url_path(path))
        })
        .unwrap_or(false)
    }
}

fn is_raiplay_stream_playback_active(hwnd: HWND) -> bool {
    with_state(hwnd, |state| {
        state.active_podcast_episode_from_rai == RaiAudioOrigin::RaiPlay
            && state
                .active_audiobook
                .as_ref()
                .map(|player| is_direct_stream_url_path(&player.path))
                .unwrap_or(false)
    })
    .unwrap_or(false)
}

fn is_raiplay_live_stream_playback_active(hwnd: HWND) -> bool {
    with_state(hwnd, |state| {
        state.active_podcast_episode_from_rai == RaiAudioOrigin::RaiPlay
            && !state.raiplay_live_audio_variants.is_empty()
            && state
                .active_audiobook
                .as_ref()
                .map(|player| is_direct_stream_url_path(&player.path))
                .unwrap_or(false)
    })
    .unwrap_or(false)
}

pub(crate) fn is_live_tv_playback_active(hwnd: HWND) -> bool {
    with_state(hwnd, |state| {
        state.active_mpv_session.is_some() && state.active_mpv_is_live_tv
    })
    .unwrap_or(false)
}

fn should_route_player_command_to_mpv(hwnd: HWND) -> bool {
    with_state(hwnd, |state| {
        if state.active_mpv_session.is_none() {
            return false;
        }
        let current_doc_path = state.docs.get(state.current).and_then(|doc| {
            if matches!(doc.format, FileFormat::Audiobook) {
                doc.path.as_ref()
            } else {
                None
            }
        });
        match (current_doc_path, state.active_podcast_episode_url.as_ref()) {
            (Some(path), Some(active_url)) => path.to_string_lossy() == active_url.as_str(),
            (Some(_), None) => false,
            (None, _) => false,
        }
    })
    .unwrap_or(false)
}

#[derive(Clone, Copy)]
enum TranscriptionProfile {
    Small,
    Medium,
    Large,
}

fn map_profile_to_bridge_model(profile: TranscriptionProfile) -> BridgeModel {
    match profile {
        TranscriptionProfile::Small => BridgeModel::Small,
        TranscriptionProfile::Medium => BridgeModel::Medium,
        TranscriptionProfile::Large => BridgeModel::LargeV3,
    }
}

fn profile_from_setting(value: &str) -> Option<TranscriptionProfile> {
    match value {
        "tiny_q5_1" | "base_q5_1" | "small_q5_1" => Some(TranscriptionProfile::Small),
        "medium_q5_0" => Some(TranscriptionProfile::Medium),
        "large_v3_turbo_q5_0" => Some(TranscriptionProfile::Large),
        _ => None,
    }
}

fn profile_to_setting(profile: TranscriptionProfile) -> &'static str {
    match profile {
        TranscriptionProfile::Small => "small_q5_1",
        TranscriptionProfile::Medium => "medium_q5_0",
        TranscriptionProfile::Large => "large_v3_turbo_q5_0",
    }
}

fn choose_whisper_profile_if_needed(
    hwnd: HWND,
    language: Language,
) -> Option<TranscriptionProfile> {
    if let Some(saved) = with_state(hwnd, |state| {
        profile_from_setting(&state.settings.whisper_model_profile)
    })
    .flatten()
    {
        return Some(saved);
    }

    let selected = crate::app_windows::whisper_model_window::choose_whisper_model(hwnd, language)
        .and_then(|s| profile_from_setting(&s));

    if let Some(profile) = selected {
        let to_store = profile_to_setting(profile).to_string();
        if let Some(snapshot) = with_state(hwnd, |state| {
            state.settings.whisper_model_profile = to_store;
            state.settings.clone()
        }) {
            settings::save_settings(snapshot);
        }
    }

    selected
}

fn cancel_whisper_transcription(hwnd: HWND) {
    let language = with_state(hwnd, |state| {
        if let Some(cancel) = state.transcription_cancel.as_ref() {
            cancel.store(true, Ordering::Relaxed);
        }
        state.transcription_media_path = None;
        state.settings.language
    })
    .unwrap_or_default();
    let msg = i18n::tr(language, "whisper.status.cancel_requested");
    if !msg.is_empty() {
        screen_reader_speak(&msg);
    }
    // Close the progress dialog immediately and return focus to Sonarpad.
    app_windows::la7_play_window::finish_context_transcription(hwnd, false);
    app_windows::youtube_transcript_window::finish_context_transcription(hwnd);
    close_whisper_progress_window(hwnd);
    crate::set_foreground_window_safe(hwnd);
    crate::set_focus_safe(hwnd);
}

pub(crate) fn open_whisper_progress_window(hwnd: HWND, language: Language) {
    let labels = app_windows::podcast_save_window::SaveDialogLabels {
        title: i18n::tr(language, "whisper.progress_title"),
        in_progress: i18n::tr(language, "whisper.status.transcribing"),
        cancel: i18n::tr(language, "playback.transcribe_cancel"),
        cancel_confirm: i18n::tr(language, "whisper.cancel_confirm"),
    };
    open_whisper_progress_window_with_labels(hwnd, language, labels, false);
}

pub(crate) fn open_whisper_progress_window_with_labels(
    hwnd: HWND,
    language: Language,
    labels: app_windows::podcast_save_window::SaveDialogLabels,
    show_status_field: bool,
) {
    let dialog = app_windows::podcast_save_window::open_with_labels_and_status_field_parent_mode(
        hwnd,
        language,
        labels,
        true,
        show_status_field,
        false,
    );
    with_state(hwnd, |state| {
        state.transcription_progress_window = dialog;
    });
}

pub(crate) fn update_whisper_progress_window(hwnd: HWND, pct: usize) {
    let dialog = with_state(hwnd, |state| state.transcription_progress_window).unwrap_or(HWND(0));
    if dialog.0 != 0 {
        crate::send_message_w_safe(
            dialog,
            app_windows::podcast_save_window::WM_PODCAST_SAVE_PROGRESS,
            WPARAM(pct.min(100)),
            LPARAM(0),
        );
    }
}

pub(crate) fn update_whisper_progress_status(hwnd: HWND, text: &str) {
    let dialog = with_state(hwnd, |state| state.transcription_progress_window).unwrap_or(HWND(0));
    if dialog.0 != 0 {
        app_windows::podcast_save_window::set_status_text(dialog, text);
    }
}

pub(crate) fn close_whisper_progress_window(hwnd: HWND) {
    let dialog = with_state(hwnd, |state| state.transcription_progress_window).unwrap_or(HWND(0));
    if dialog.0 != 0 {
        crate::send_message_w_safe(
            dialog,
            app_windows::podcast_save_window::WM_PODCAST_SAVE_DONE,
            WPARAM(0),
            LPARAM(0),
        );
    }
    with_state(hwnd, |state| {
        state.transcription_progress_window = HWND(0);
    });
}

fn post_whisper_progress_status(hwnd: HWND, text: String) {
    let ptr = Box::into_raw(Box::new(text));
    if let Err(err) = post_message_w_safe(
        hwnd,
        WM_WHISPER_TRANSCRIPTION_STATUS_TEXT,
        WPARAM(0),
        LPARAM(ptr as isize),
    ) {
        log_debug(&format!(
            "Failed to post WM_WHISPER_TRANSCRIPTION_STATUS_TEXT: {err}"
        ));
        let _unused_box = box_from_raw_safe(ptr);
    }
}

fn is_whisper_transcribable_audio_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "wav"
            | "mp3"
            | "flac"
            | "ogg"
            | "opus"
            | "m4a"
            | "aac"
            | "wma"
            | "webm"
            | "mp4"
            | "mkv"
            | "mov"
    )
}

fn whisper_audio_language_from_setting(code: &str, fallback: Language) -> Language {
    match code.trim() {
        "it" => Language::Italian,
        "en" => Language::English,
        "de" => Language::German,
        "es" => Language::Spanish,
        "pt" => Language::Portuguese,
        "pt-BR" | "pt-br" => Language::PortugueseBrazilian,
        "sv" => Language::Swedish,
        "vi" => Language::Vietnamese,
        "cs" => Language::Czech,
        "pl" => Language::Polish,
        "fr" => Language::French,
        "sr" => Language::Serbian,
        "uk" => Language::Ukrainian,
        "lt" => Language::Lithuanian,
        "ru" => Language::Russian,
        "zh" => Language::Chinese,
        "hi" => Language::Hindi,
        _ => fallback,
    }
}

fn collect_transcribable_audio_files_in_folder(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let read_dir = match std::fs::read_dir(folder) {
        Ok(read_dir) => read_dir,
        Err(err) => {
            log_debug(&format!(
                "Folder transcription: failed to read dir {}: {}",
                folder.display(),
                err
            ));
            return files;
        }
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() || !is_whisper_transcribable_audio_file(&path) {
            continue;
        }
        files.push(path);
    }
    files.sort_by_key(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_ascii_lowercase())
            .unwrap_or_default()
    });
    files
}

fn start_whisper_transcription(hwnd: HWND) {
    start_whisper_transcription_with_media(hwnd, None, true);
}

pub(crate) fn start_whisper_transcription_for_media(
    hwnd: HWND,
    media_path: PathBuf,
    media_title: Option<String>,
) {
    start_whisper_transcription_with_media(hwnd, Some((media_path, None, media_title)), false);
}

fn queue_context_whisper_transcription(
    hwnd: HWND,
    media_path: PathBuf,
    media_title: Option<String>,
    close_raiplay_on_success: bool,
    close_la7_on_success: bool,
) {
    // Context-menu transcription is launched from a modal media selector. Close that
    // selector first, then start Whisper from a posted main-window message. This avoids
    // destroying the selector from inside the transcription-completion handler, which
    // could unwind the old streaming flow after the result document had been created.
    let selector_close_requested =
        app_windows::interpreter_select_window::request_close_active_interpreter_selects_for_parent(
            hwnd,
        );
    if !selector_close_requested {
        log_debug("Context transcription: no active interpreter selector needed closing");
    }
    let payload = DeferredWhisperTranscription {
        media_path,
        media_title,
        close_raiplay_on_success,
        close_la7_on_success,
    };
    let ptr = Box::into_raw(Box::new(payload));
    if let Err(err) = post_message_w_safe(
        hwnd,
        WM_START_CONTEXT_WHISPER_TRANSCRIPTION,
        WPARAM(0),
        LPARAM(ptr as isize),
    ) {
        log_debug(&format!(
            "Failed to post WM_START_CONTEXT_WHISPER_TRANSCRIPTION: {err}"
        ));
        let DeferredWhisperTranscription {
            media_path,
            media_title,
            close_raiplay_on_success,
            close_la7_on_success,
        } = *unsafe { Box::from_raw(ptr) };
        start_whisper_transcription_for_media(hwnd, media_path, media_title);
        let started = with_state(hwnd, |state| state.transcription_in_progress).unwrap_or(false);
        if close_raiplay_on_success && started {
            app_windows::raiplay_window::mark_context_transcription_started(hwnd);
        }
        if close_la7_on_success && started {
            app_windows::la7_play_window::mark_context_transcription_started(hwnd);
        }
        if !started {
            app_windows::rai_audiodescrizioni_window::finish_context_transcription(hwnd);
            app_windows::youtube_transcript_window::finish_context_transcription(hwnd);
        }
    }
}

pub(crate) fn start_whisper_transcription_for_media_context(
    hwnd: HWND,
    media_path: PathBuf,
    media_title: Option<String>,
) {
    queue_context_whisper_transcription(hwnd, media_path, media_title, false, false);
}

pub(crate) fn start_whisper_transcription_for_remote_media_context(
    hwnd: HWND,
    media_url: String,
    media_title: Option<String>,
) {
    queue_context_whisper_transcription(hwnd, PathBuf::from(media_url), media_title, false, false);
}

pub(crate) fn start_whisper_transcription_for_raiplay_context(
    hwnd: HWND,
    media_url: String,
    media_title: Option<String>,
) {
    queue_context_whisper_transcription(hwnd, PathBuf::from(media_url), media_title, true, false);
}

pub(crate) fn start_whisper_transcription_for_la7_context(
    hwnd: HWND,
    media_url: String,
    media_title: Option<String>,
) {
    queue_context_whisper_transcription(hwnd, PathBuf::from(media_url), media_title, false, true);
}

fn start_whisper_transcription_with_media(
    hwnd: HWND,
    explicit_media: Option<(PathBuf, Option<i32>, Option<String>)>,
    download_direct_stream_with_ytdlp: bool,
) {
    let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
    let whisper_audio_language =
        with_state(hwnd, |state| state.settings.whisper_audio_language.clone()).unwrap_or_default();
    let whisper_include_timestamps =
        with_state(hwnd, |state| state.settings.whisper_include_timestamps).unwrap_or(false);
    let whisper_cuda_enabled =
        with_state(hwnd, |state| state.settings.whisper_cuda_enabled).unwrap_or(false);
    let Some(whisper_profile) = choose_whisper_profile_if_needed(hwnd, language) else {
        return;
    };
    let media_info = explicit_media.or_else(|| {
        with_state(hwnd, |state| {
            let active_stream_title_for_path = |path: &Path| {
                state
                    .active_podcast_episode_url
                    .as_ref()
                    .filter(|url| path.to_string_lossy() == url.as_str())
                    .and(state.active_podcast_episode_title.clone())
            };
            if let Some(player) = state.active_audiobook.as_ref() {
                return Some((
                    player.path.clone(),
                    state.selected_audio_track,
                    active_stream_title_for_path(&player.path),
                ));
            }
            state.docs.get(state.current).and_then(|doc| {
                if matches!(doc.format, FileFormat::Audiobook) {
                    doc.path.as_ref().map(|path| {
                        (
                            path.clone(),
                            state.selected_audio_track,
                            active_stream_title_for_path(path),
                        )
                    })
                } else {
                    None
                }
            })
        })
        .flatten()
    });

    let Some((mut media_path, mut stream_index, media_title)) = media_info else {
        screen_reader_speak(&i18n::tr(language, "whisper.error.no_active_media"));
        return;
    };

    if download_direct_stream_with_ytdlp && is_direct_stream_url_path(&media_path) {
        let active_url = media_path.to_string_lossy().to_string();
        let Some(downloaded_path) = app_windows::youtube_transcript_window::download_active_streaming_audio_media_for_transcription(
            hwnd,
            &active_url,
            language,
        ) else {
            return;
        };
        media_path = downloaded_path;
        stream_index = None;
    }

    if should_route_player_command_to_mpv(hwnd) {
        if try_send_command_to_managed_mpv(hwnd, r#"{"command":["set_property","pause",true]}"#)
            .is_ok()
        {
            stop_mpv_subtitle_speech(hwnd, "transcription_start");
            sync_mpv_sleep_prevention(hwnd);
        }
    } else {
        crate::audio_player::pause_audiobook_if_playing(hwnd);
    }
    prevent_sleep(true);

    let cancel_flag = Arc::new(AtomicBool::new(false));
    with_state(hwnd, |state| {
        if let Some(prev) = state.transcription_cancel.take() {
            prev.store(true, Ordering::Relaxed);
        }
        state.transcription_cancel = Some(cancel_flag.clone());
        state.transcription_in_progress = true;
        state.transcription_media_path = Some(media_path.clone());
    });
    open_whisper_progress_window(hwnd, language);
    update_whisper_progress_window(hwnd, 0);
    crate::menu::update_playback_menu(hwnd, true);

    let start_msg = i18n::tr(language, "whisper.status.starting");
    if !start_msg.is_empty() {
        screen_reader_speak(&start_msg);
    }

    std::thread::spawn(move || {
        let result = (|| -> Result<WhisperTranscriptionResult, String> {
            let input_path = if supports_direct_whisper_input(&media_path, stream_index) {
                crate::log_debug(&format!(
                    "Transcription: using direct media input {}",
                    media_path.display()
                ));
                media_path.clone()
            } else {
                crate::log_debug("Transcription: preparing WAV for faster-whisper bridge");
                screen_reader_speak(&i18n::tr(language, "whisper.status.preparing_audio"));
                let _unused = post_message_w_safe(
                    hwnd,
                    WM_WHISPER_TRANSCRIPTION_PROGRESS,
                    WPARAM(10),
                    LPARAM(0),
                );
                crate::audio_player::prepare_media_wav_for_transcription(&media_path, stream_index)?
            };

            let model = map_profile_to_bridge_model(whisper_profile);
            crate::log_debug(&format!(
                "Transcription: invoking faster-whisper bridge model={}",
                match model {
                    BridgeModel::Small => "small",
                    BridgeModel::Medium => "medium",
                    BridgeModel::LargeV3 => "large-v3",
                }
            ));
            let _unused = post_message_w_safe(
                hwnd,
                WM_WHISPER_TRANSCRIPTION_PROGRESS,
                WPARAM(20),
                LPARAM(0),
            );
            screen_reader_speak(&i18n::tr(language, "whisper.status.transcribing"));
            let forced_language = Some(whisper_audio_language_from_setting(
                &whisper_audio_language,
                language,
            ));
            let mut download_progress_last = -1;
            let bridge_download_progress_callback: Box<dyn FnMut(i32) + Send> =
                Box::new(move |pct| {
                    let clamped = pct.clamp(0, 100);
                    // Bridge download occupies 0..19% of overall progress.
                    let mapped = ((clamped * 19 + 99) / 100).clamp(0, 19);
                    if mapped > download_progress_last {
                        download_progress_last = mapped;
                        let _unused = post_message_w_safe(
                            hwnd,
                            WM_WHISPER_TRANSCRIPTION_PROGRESS,
                            WPARAM(mapped as usize),
                            LPARAM(0),
                        );
                    }
                });
            let mut progress_last = -1;
            let progress_callback: Box<dyn FnMut(i32) + Send> = Box::new(move |pct| {
                let clamped = pct.clamp(0, 100);
                // Keep UI progress monotonic from 20% (bridge start) to 99% (bridge end).
                // Bridge emits 0..99 based on decoded audio position.
                let mapped = if clamped <= 0 {
                    20
                } else if clamped >= 100 {
                    99
                } else {
                    20 + ((clamped * 79 + 99) / 100)
                };
                if mapped > progress_last {
                    progress_last = mapped;
                    let _unused = post_message_w_safe(
                        hwnd,
                        WM_WHISPER_TRANSCRIPTION_PROGRESS,
                        WPARAM(mapped as usize),
                        LPARAM(0),
                    );
                }
            });

            let text = crate::tools::faster_whisper_bridge::transcribe_wav_with_shared_worker(
                &input_path,
                model,
                forced_language,
                whisper_include_timestamps,
                whisper_cuda_enabled,
                &cancel_flag,
                crate::tools::faster_whisper_bridge::BridgeProgressCallbacks {
                    download: Some(bridge_download_progress_callback),
                    transcription: Some(progress_callback),
                },
            )?;
            crate::log_debug(&format!(
                "Transcription: bridge completed text_len={}",
                text.len()
            ));
            if cancel_flag.load(Ordering::Relaxed) {
                return Ok(WhisperTranscriptionResult {
                    title: String::new(),
                    text: String::new(),
                    error_message: None,
                    completed_message: i18n::tr(language, "whisper.status.completed"),
                    cancelled: true,
                });
            }
            let _unused = post_message_w_safe(
                hwnd,
                WM_WHISPER_TRANSCRIPTION_PROGRESS,
                WPARAM(100),
                LPARAM(0),
            );

            let fallback_name = media_path
                .file_stem()
                .and_then(|s| s.to_str())
                .or_else(|| media_path.file_name().and_then(|s| s.to_str()))
                .unwrap_or("audio");
            let base_name = media_title
                .as_deref()
                .filter(|title| !title.trim().is_empty())
                .unwrap_or(fallback_name);
            let mut title_path = PathBuf::from(i18n::tr_f(
                language,
                "whisper.output_title",
                &[("name", base_name)],
            ));
            title_path.set_extension("txt");
            let title = title_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("transcription.txt")
                .to_string();
            Ok(WhisperTranscriptionResult {
                title,
                text,
                error_message: None,
                completed_message: i18n::tr(language, "whisper.status.completed"),
                cancelled: false,
            })
        })();

        let payload = match result {
            Ok(done) => done,
            Err(err) => WhisperTranscriptionResult {
                title: String::new(),
                text: String::new(),
                error_message: Some(i18n::tr_f(
                    language,
                    "whisper.error.failed",
                    &[("err", &err)],
                )),
                completed_message: i18n::tr(language, "whisper.status.completed"),
                cancelled: cancel_flag.load(Ordering::Relaxed),
            },
        };
        if let Some(err) = payload.error_message.as_ref() {
            crate::log_debug(&format!("Transcription: failed: {}", err));
        } else if payload.cancelled {
            crate::log_debug("Transcription: cancelled");
        } else {
            crate::log_debug("Transcription: posting result to UI thread");
        }
        let ptr = Box::into_raw(Box::new(payload));
        if let Err(err) = post_message_w_safe(
            hwnd,
            WM_WHISPER_TRANSCRIPTION_DONE,
            WPARAM(0),
            LPARAM(ptr as isize),
        ) {
            log_debug(&format!(
                "Failed to post WM_WHISPER_TRANSCRIPTION_DONE: {err}"
            ));
            let _unused_box = box_from_raw_safe(ptr);
        }
    });
}

fn start_whisper_folder_transcription(hwnd: HWND) {
    let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
    let whisper_audio_language =
        with_state(hwnd, |state| state.settings.whisper_audio_language.clone()).unwrap_or_default();
    let whisper_include_timestamps =
        with_state(hwnd, |state| state.settings.whisper_include_timestamps).unwrap_or(false);
    let whisper_cuda_enabled =
        with_state(hwnd, |state| state.settings.whisper_cuda_enabled).unwrap_or(false);
    let Some(whisper_profile) = choose_whisper_profile_if_needed(hwnd, language) else {
        return;
    };

    let media_path = with_state(hwnd, |state| {
        state
            .active_audiobook
            .as_ref()
            .map(|player| player.path.clone())
            .or_else(|| {
                state.docs.get(state.current).and_then(|doc| {
                    if matches!(doc.format, FileFormat::Audiobook) {
                        doc.path.clone()
                    } else {
                        None
                    }
                })
            })
    })
    .flatten();

    let Some(media_path) = media_path else {
        screen_reader_speak(&i18n::tr(language, "whisper.error.no_active_media"));
        return;
    };
    let Some(folder) = media_path.parent() else {
        screen_reader_speak(&i18n::tr(language, "whisper.folder.error.no_audio_files"));
        return;
    };
    let folder = folder.to_path_buf();
    let files = collect_transcribable_audio_files_in_folder(&folder);
    if files.is_empty() {
        screen_reader_speak(&i18n::tr(language, "whisper.folder.error.no_audio_files"));
        return;
    }

    crate::audio_player::pause_audiobook_if_playing(hwnd);
    prevent_sleep(true);

    let cancel_flag = Arc::new(AtomicBool::new(false));
    with_state(hwnd, |state| {
        if let Some(prev) = state.transcription_cancel.take() {
            prev.store(true, Ordering::Relaxed);
        }
        state.transcription_cancel = Some(cancel_flag.clone());
        state.transcription_in_progress = true;
        state.transcription_media_path = Some(media_path.clone());
    });

    let labels = app_windows::podcast_save_window::SaveDialogLabels {
        title: i18n::tr(language, "whisper.folder.progress_title"),
        in_progress: i18n::tr(language, "whisper.folder.status.starting"),
        cancel: i18n::tr(language, "playback.transcribe_cancel"),
        cancel_confirm: i18n::tr(language, "whisper.folder.cancel_confirm"),
    };
    open_whisper_progress_window_with_labels(hwnd, language, labels, true);
    update_whisper_progress_window(hwnd, 0);
    update_whisper_progress_status(hwnd, &i18n::tr(language, "whisper.folder.status.starting"));
    crate::menu::update_playback_menu(hwnd, true);
    screen_reader_speak(&i18n::tr(language, "whisper.folder.status.starting"));

    std::thread::spawn(move || {
        let result = (|| -> Result<WhisperTranscriptionResult, String> {
            let model = map_profile_to_bridge_model(whisper_profile);
            let forced_language = Some(whisper_audio_language_from_setting(
                &whisper_audio_language,
                language,
            ));
            let total = files.len();
            let mut full_text = String::new();

            for (index, path) in files.iter().enumerate() {
                if cancel_flag.load(Ordering::Relaxed) {
                    return Ok(WhisperTranscriptionResult {
                        title: String::new(),
                        text: String::new(),
                        error_message: None,
                        completed_message: i18n::tr(language, "whisper.folder.status.completed"),
                        cancelled: true,
                    });
                }

                let file_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("audio")
                    .to_string();
                post_whisper_progress_status(
                    hwnd,
                    i18n::tr_f(
                        language,
                        "whisper.folder.status.progress",
                        &[
                            ("current", &(index + 1).to_string()),
                            ("total", &total.to_string()),
                            ("name", &file_name),
                        ],
                    ),
                );

                let input_path = if supports_direct_whisper_input(path, None) {
                    path.clone()
                } else {
                    crate::audio_player::prepare_media_wav_for_transcription(path, None)?
                };

                let mut progress_last = -1;
                let hwnd_progress = hwnd;
                let total_files = total;
                let current_index = index;
                let progress_callback: Box<dyn FnMut(i32) + Send> = Box::new(move |pct| {
                    let clamped = pct.clamp(0, 100) as usize;
                    let mapped = (((current_index * 100) + clamped) / total_files).min(99);
                    if mapped as i32 > progress_last {
                        progress_last = mapped as i32;
                        let _unused = post_message_w_safe(
                            hwnd_progress,
                            WM_WHISPER_TRANSCRIPTION_PROGRESS,
                            WPARAM(mapped),
                            LPARAM(0),
                        );
                    }
                });

                let text = crate::tools::faster_whisper_bridge::transcribe_wav_with_shared_worker(
                    &input_path,
                    model,
                    forced_language,
                    whisper_include_timestamps,
                    whisper_cuda_enabled,
                    &cancel_flag,
                    crate::tools::faster_whisper_bridge::BridgeProgressCallbacks {
                        download: None,
                        transcription: Some(progress_callback),
                    },
                )?;

                if !text.trim().is_empty() {
                    if !full_text.is_empty() {
                        full_text.push_str("\r\n\r\n");
                    }
                    full_text.push_str(text.trim());
                }
            }

            let _unused = post_message_w_safe(
                hwnd,
                WM_WHISPER_TRANSCRIPTION_PROGRESS,
                WPARAM(100),
                LPARAM(0),
            );

            let folder_name = folder
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("audio");
            let mut title_path = PathBuf::from(i18n::tr_f(
                language,
                "whisper.folder.output_title",
                &[("name", folder_name)],
            ));
            title_path.set_extension("txt");
            let title = title_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("transcription.txt")
                .to_string();

            Ok(WhisperTranscriptionResult {
                title,
                text: full_text,
                error_message: None,
                completed_message: i18n::tr(language, "whisper.folder.status.completed"),
                cancelled: false,
            })
        })();

        let payload = match result {
            Ok(done) => done,
            Err(err) => WhisperTranscriptionResult {
                title: String::new(),
                text: String::new(),
                error_message: Some(i18n::tr_f(
                    language,
                    "whisper.error.failed",
                    &[("err", &err)],
                )),
                completed_message: i18n::tr(language, "whisper.folder.status.completed"),
                cancelled: cancel_flag.load(Ordering::Relaxed),
            },
        };
        let ptr = Box::into_raw(Box::new(payload));
        if let Err(err) = post_message_w_safe(
            hwnd,
            WM_WHISPER_TRANSCRIPTION_DONE,
            WPARAM(0),
            LPARAM(ptr as isize),
        ) {
            log_debug(&format!(
                "Failed to post WM_WHISPER_TRANSCRIPTION_DONE: {err}"
            ));
            let _unused_box = box_from_raw_safe(ptr);
        }
    });
}

fn apply_whisper_transcription_result(hwnd: HWND, result: WhisperTranscriptionResult) {
    let (language, documents_save_folder) = with_state(hwnd, |state| {
        state.transcription_in_progress = false;
        state.transcription_cancel = None;
        state.transcription_media_path = None;
        (
            state.settings.language,
            state.settings.documents_save_folder.clone(),
        )
    })
    .unwrap_or_else(|| {
        (
            Language::default(),
            settings::default_documents_save_folder(),
        )
    });
    prevent_sleep(false);
    let transcription_succeeded = !result.cancelled && result.error_message.is_none();
    app_windows::raiplay_window::finish_context_transcription(hwnd, transcription_succeeded);
    app_windows::la7_play_window::finish_context_transcription(hwnd, transcription_succeeded);
    app_windows::rai_audiodescrizioni_window::finish_context_transcription(hwnd);
    app_windows::youtube_transcript_window::finish_context_transcription(hwnd);
    let dismissed_media_context = transcription_succeeded
        && app_windows::youtube_transcript_window::dismiss_active_media_context_lists_for_editor(
            hwnd,
        );
    close_whisper_progress_window(hwnd);
    crate::menu::update_playback_menu(hwnd, true);

    if result.cancelled {
        screen_reader_speak(&i18n::tr(language, "whisper.status.cancelled"));
        return;
    }
    if let Some(err) = result.error_message {
        screen_reader_speak(&err);
        return;
    }

    // A transcription launched from a media selector replaces that selector with the
    // result document.  Re-enable the editor parent only for that workflow; do not
    // disturb unrelated modal windows that the user may have opened independently.
    if dismissed_media_context {
        crate::enable_window_safe(hwnd, true);
    }

    let default_prefix = i18n::tr(language, "whisper.default_filename");
    let auto_save_result = auto_save_whisper_transcription(
        Path::new(&documents_save_folder),
        &result.title,
        &result.text,
        &default_prefix,
    );
    let (saved_path, save_error) = match auto_save_result {
        Ok(path) => (Some(path), None),
        Err(err) => (None, Some(err)),
    };

    editor_manager::new_document(hwnd);
    with_state(hwnd, |state| {
        let idx = state.current;
        let hwnd_tab = state.hwnd_tab;
        if let Some(doc) = state.docs.get_mut(idx) {
            doc.title = saved_path
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| result.title.clone());
            doc.path = saved_path.clone();
            doc.format = FileFormat::Text(TextEncoding::Utf8);
            editor_manager::set_edit_text(doc.hwnd_edit, &result.text);
            doc.dirty = saved_path.is_none();
            doc.confirm_close_when_clean = true;
            doc.prefer_title_for_save_suggestion = saved_path.is_none();
            editor_manager::update_tab_title(hwnd_tab, idx, &doc.title, doc.dirty);
        }
    });
    if let Some(path) = saved_path.as_ref() {
        crate::push_recent_file(hwnd, path);
    }
    editor_manager::update_window_title(hwnd);
    crate::log_if_err!(post_message_w_safe(
        hwnd,
        WM_FOCUS_EDITOR,
        WPARAM(0),
        LPARAM(0)
    ));
    if let Some(err) = save_error {
        let message = i18n::tr_f(
            language,
            "whisper.status.save_failed",
            &[("completed", &result.completed_message), ("error", &err)],
        );
        crate::show_error(hwnd, language, &message);
        screen_reader_speak(&message);
    } else if let Some(path) = saved_path {
        let message = i18n::tr_f(
            language,
            "whisper.status.saved_in",
            &[
                ("completed", &result.completed_message),
                ("path", &path.display().to_string()),
            ],
        );
        screen_reader_speak(&message);
    } else {
        screen_reader_speak(&result.completed_message);
    }
}

fn auto_save_whisper_transcription(
    documents_save_folder: &Path,
    suggested_title: &str,
    text: &str,
    default_prefix: &str,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(documents_save_folder).map_err(|err| err.to_string())?;
    let path =
        unique_whisper_transcription_path(documents_save_folder, suggested_title, default_prefix);
    std::fs::write(&path, encode_text(text, TextEncoding::Utf8)).map_err(|err| err.to_string())?;
    Ok(path)
}

fn unique_whisper_transcription_path(
    documents_save_folder: &Path,
    suggested_title: &str,
    default_prefix: &str,
) -> PathBuf {
    let mut candidate_name = sanitize_filename(suggested_title);
    if candidate_name.is_empty() {
        candidate_name = format!(
            "{}_{}.txt",
            default_prefix,
            Local::now().format("%Y-%m-%d_%H-%M-%S")
        );
    }
    if Path::new(&candidate_name).extension().is_none() {
        candidate_name.push_str(".txt");
    }

    let candidate_path = PathBuf::from(&candidate_name);
    let stem = candidate_path
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(default_prefix)
        .to_string();
    let extension = candidate_path
        .extension()
        .and_then(|ext| ext.to_str())
        .filter(|ext| !ext.trim().is_empty())
        .unwrap_or("txt")
        .to_string();

    let first_path = documents_save_folder.join(format!("{stem}.{extension}"));
    if !first_path.exists() {
        return first_path;
    }

    for index in 2..10_000 {
        let path = documents_save_folder.join(format!("{stem}_{index}.{extension}"));
        if !path.exists() {
            return path;
        }
    }

    documents_save_folder.join(format!(
        "{}_{}.{}",
        stem,
        Local::now().format("%Y-%m-%d_%H-%M-%S"),
        extension
    ))
}

fn insert_text_into_edit(hwnd_edit: HWND, text: &str) -> bool {
    if hwnd_edit.0 == 0 || text.is_empty() || !is_window_handle_valid(hwnd_edit) {
        return false;
    }
    let wide = to_wide(text);
    send_message_w_safe(
        hwnd_edit,
        EM_REPLACESEL,
        WPARAM(1),
        LPARAM(wide.as_ptr() as isize),
    );
    send_message_w_safe(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
    true
}

fn apply_dictation_result(hwnd: HWND, result: DictationResult) {
    let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
    log_debug(&format!(
        "Dictation: UI result session={} cancelled={} error_present={} text_chars={} target_edit={:?}",
        result.session_id,
        result.cancelled,
        result.error.is_some(),
        result.text.chars().count(),
        result.target_edit
    ));
    let current_session_id = with_state(hwnd, |state| state.dictation_session_id).unwrap_or(0);
    if result.session_id != current_session_id {
        log_debug("Dictation: ignoring stale result from previous session");
        return;
    }

    with_state(hwnd, |state| {
        state.dictation_transcribing = false;
        state.dictation_cancel = None;
        state.dictation_target_edit = HWND(0);
    });
    close_whisper_progress_window(hwnd);

    if result.cancelled {
        screen_reader_speak(&i18n::tr(language, "dictation.status.cancelled"));
        return;
    }
    if let Some(err) = result.error {
        screen_reader_speak(&i18n::tr_f(
            language,
            "dictation.error.failed",
            &[("err", &err)],
        ));
        return;
    }
    let trimmed = result.text.trim();
    if trimmed.is_empty() {
        screen_reader_speak(&i18n::tr(language, "dictation.error.no_speech"));
        return;
    }

    let preferred_edit = if is_window_handle_valid(result.target_edit) {
        result.target_edit
    } else {
        HWND(0)
    };
    let active_edit = get_active_edit(hwnd).unwrap_or(HWND(0));
    let target_edit = if preferred_edit.0 != 0 {
        preferred_edit
    } else {
        active_edit
    };
    if target_edit.0 == 0 {
        screen_reader_speak(&i18n::tr(language, "dictation.error.no_active_editor"));
        return;
    }

    let mut text = trimmed.to_string();
    if !text.ends_with([' ', '\n', '\t']) {
        text.push(' ');
    }
    if insert_text_into_edit(target_edit, &text) {
        bring_window_to_foreground(hwnd);
        set_focus_safe(target_edit);
        send_message_w_safe(target_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
        send_message_w_safe(target_edit, WM_SETFOCUS, WPARAM(0), LPARAM(0));
        crate::log_if_err!(post_message_w_safe(
            hwnd,
            WM_NEXTDLGCTL,
            WPARAM(target_edit.0 as usize),
            LPARAM(1)
        ));
        restore_editor_focus(hwnd);
        screen_reader_speak(&i18n::tr(language, "dictation.status.inserted"));
    } else {
        screen_reader_speak(&i18n::tr(language, "dictation.error.insert_failed"));
    }
}

fn dictation_language_name(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::German => "de",
        Language::English => "en",
        Language::Spanish => "es",
        Language::Portuguese => "pt",
        Language::PortugueseBrazilian => "pt-BR",
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

fn start_dictation_recorder_from_state(
    hwnd: HWND,
) -> Result<podcast_recorder::RecorderHandle, String> {
    let config = with_state(hwnd, |state| {
        crate::tools::dictation::DictationRecordingConfig {
            mic_device_id: state.settings.dictation_microphone_device_id.clone(),
            mic_gain: state.settings.podcast_microphone_gain,
        }
    })
    .ok_or_else(|| "Dictation settings unavailable.".to_string())?;
    crate::tools::dictation::start_recording(&config)
}

fn toggle_voice_dictation(hwnd: HWND) {
    let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
    let recording = with_state(hwnd, |state| state.dictation_recorder.is_some()).unwrap_or(false);
    if recording {
        let (
            recorder,
            target_edit,
            whisper_model,
            whisper_cuda_enabled,
            forced_language,
            cancel,
            session_id,
        ) = with_state(hwnd, |state| {
            let recorder = state.dictation_recorder.take();
            state.dictation_transcribing = recorder.is_some();
            let cancel = Arc::new(AtomicBool::new(false));
            state.dictation_cancel = if recorder.is_some() {
                Some(cancel.clone())
            } else {
                None
            };
            (
                recorder,
                state.dictation_target_edit,
                profile_from_setting(&state.settings.whisper_model_profile)
                    .map(map_profile_to_bridge_model)
                    .unwrap_or(BridgeModel::Small),
                state.settings.whisper_cuda_enabled,
                Some(whisper_audio_language_from_setting(
                    &state.settings.whisper_audio_language,
                    state.settings.language,
                )),
                cancel,
                state.dictation_session_id,
            )
        })
        .unwrap_or((
            None,
            HWND(0),
            BridgeModel::Small,
            false,
            Some(language),
            Arc::new(AtomicBool::new(false)),
            0,
        ));
        let Some(recorder) = recorder else {
            return;
        };
        open_whisper_progress_window(hwnd, language);
        update_whisper_progress_window(hwnd, 0);
        screen_reader_speak(&i18n::tr(language, "dictation.status.transcribing"));
        std::thread::spawn(move || {
            let started = Instant::now();
            let result = (|| -> Result<DictationResult, String> {
                let path = recorder.stop()?;
                let file_size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
                match crate::tools::dictation::wav_duration_seconds(&path) {
                    Ok(seconds) => log_debug(&format!(
                        "Dictation: bridge start session={} path={} size={} duration_secs={:.2} model={} cuda={} forced_language={}",
                        session_id,
                        path.display(),
                        file_size,
                        seconds,
                        match whisper_model {
                            BridgeModel::Small => "small",
                            BridgeModel::Medium => "medium",
                            BridgeModel::LargeV3 => "large-v3",
                        },
                        whisper_cuda_enabled,
                        forced_language
                            .map(dictation_language_name)
                            .unwrap_or("auto")
                    )),
                    Err(err) => log_debug(&format!(
                        "Dictation: bridge start session={} path={} size={} duration_unknown err={} model={} cuda={} forced_language={}",
                        session_id,
                        path.display(),
                        file_size,
                        err,
                        match whisper_model {
                            BridgeModel::Small => "small",
                            BridgeModel::Medium => "medium",
                            BridgeModel::LargeV3 => "large-v3",
                        },
                        whisper_cuda_enabled,
                        forced_language
                            .map(dictation_language_name)
                            .unwrap_or("auto")
                    )),
                }
                let mut progress_last = -1;
                let progress_callback: Box<dyn FnMut(i32) + Send> = Box::new(move |pct| {
                    let clamped = pct.clamp(0, 100);
                    let mapped = if clamped <= 0 {
                        20
                    } else if clamped >= 100 {
                        99
                    } else {
                        20 + ((clamped * 79 + 99) / 100)
                    };
                    if mapped > progress_last {
                        progress_last = mapped;
                        let _unused = post_message_w_safe(
                            hwnd,
                            WM_WHISPER_TRANSCRIPTION_PROGRESS,
                            WPARAM(mapped as usize),
                            LPARAM(0),
                        );
                    }
                });
                let _unused = post_message_w_safe(
                    hwnd,
                    WM_WHISPER_TRANSCRIPTION_PROGRESS,
                    WPARAM(20),
                    LPARAM(0),
                );
                let text = crate::tools::faster_whisper_bridge::transcribe_wav_with_shared_worker(
                    &path,
                    whisper_model,
                    forced_language,
                    false,
                    whisper_cuda_enabled,
                    &cancel,
                    crate::tools::faster_whisper_bridge::BridgeProgressCallbacks {
                        download: None,
                        transcription: Some(progress_callback),
                    },
                )?;
                if let Err(err) = std::fs::remove_file(&path) {
                    log_debug(&format!("Dictation temp cleanup failed: {err}"));
                }
                let _unused = post_message_w_safe(
                    hwnd,
                    WM_WHISPER_TRANSCRIPTION_PROGRESS,
                    WPARAM(100),
                    LPARAM(0),
                );
                log_debug(&format!(
                    "Dictation: single transcription completed session={} in {:?}, chars={}",
                    session_id,
                    started.elapsed(),
                    text.chars().count()
                ));
                Ok(DictationResult {
                    session_id,
                    text,
                    error: None,
                    cancelled: cancel.load(Ordering::Relaxed),
                    target_edit,
                })
            })();
            let payload = match result {
                Ok(done) => done,
                Err(err) => DictationResult {
                    session_id,
                    text: String::new(),
                    error: Some(err),
                    cancelled: cancel.load(Ordering::Relaxed),
                    target_edit,
                },
            };
            let ptr = Box::into_raw(Box::new(payload));
            if let Err(err) =
                post_message_w_safe(hwnd, WM_DICTATION_DONE, WPARAM(0), LPARAM(ptr as isize))
            {
                log_debug(&format!("Failed to post WM_DICTATION_DONE: {err}"));
                let _unused_box = unsafe { Box::from_raw(ptr) };
            }
        });
        return;
    }

    if with_state(hwnd, |state| state.dictation_transcribing).unwrap_or(false) {
        log_debug("Dictation: toggle ignored because transcription is still running");
        screen_reader_speak(&i18n::tr(language, "dictation.status.processing"));
        return;
    }
    let Some(target_edit) = get_active_edit(hwnd) else {
        screen_reader_speak(&i18n::tr(language, "dictation.error.no_active_editor"));
        return;
    };

    let session_id = with_state(hwnd, |state| {
        state.dictation_session_id = state.dictation_session_id.saturating_add(1);
        state.dictation_session_id
    })
    .unwrap_or(1);

    match start_dictation_recorder_from_state(hwnd) {
        Ok(recorder) => {
            let whisper_model = with_state(hwnd, |state| {
                profile_from_setting(&state.settings.whisper_model_profile)
                    .map(map_profile_to_bridge_model)
                    .unwrap_or(BridgeModel::Small)
            })
            .unwrap_or(BridgeModel::Small);
            let whisper_cuda_enabled =
                with_state(hwnd, |state| state.settings.whisper_cuda_enabled).unwrap_or(false);
            with_state(hwnd, |state| {
                state.dictation_recorder = Some(recorder);
                state.dictation_target_edit = target_edit;
            });
            crate::tools::faster_whisper_bridge::prewarm_shared_worker(
                whisper_model,
                whisper_cuda_enabled,
            );
            log_debug(&format!(
                "Dictation: recording started session={} target_edit={:?}",
                session_id, target_edit
            ));
            screen_reader_speak(&i18n::tr(language, "dictation.status.started"));
        }
        Err(err) => {
            screen_reader_speak(&i18n::tr_f(
                language,
                "dictation.error.start_failed",
                &[("err", &err)],
            ));
        }
    }
}

pub(crate) fn handle_player_command(hwnd: HWND, command: PlayerCommand) {
    if daisy_player::handle_player_command(hwnd, &command) {
        return;
    }
    if should_route_player_command_to_mpv(hwnd) {
        let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
        let result = match command {
            PlayerCommand::TogglePause => {
                let result =
                    try_send_command_to_managed_mpv(hwnd, r#"{"command":["cycle","pause"]}"#);
                if result.is_ok() {
                    stop_mpv_subtitle_speech(hwnd, "pause_toggle");
                    sync_mpv_sleep_prevention(hwnd);
                }
                result
            }
            PlayerCommand::Stop | PlayerCommand::StopOnly => {
                save_automatic_bookmark_for_active_raiplaysound_episode(hwnd);
                stop_managed_mpv_playback(hwnd);
                return;
            }
            PlayerCommand::Seek(amount) => {
                let result = try_send_command_to_managed_mpv(
                    hwnd,
                    &format!(r#"{{"command":["seek",{},"relative"]}}"#, amount),
                );
                if result.is_ok() {
                    stop_mpv_subtitle_speech(hwnd, "seek_relative");
                }
                result
            }
            PlayerCommand::SeekToStart => {
                let result =
                    try_send_command_to_managed_mpv(hwnd, r#"{"command":["seek",0,"absolute"]}"#);
                if result.is_ok() {
                    stop_mpv_subtitle_speech(hwnd, "seek_start");
                }
                result
            }
            PlayerCommand::SeekToEnd => {
                let result = query_managed_mpv_property(hwnd, "duration")
                    .ok()
                    .and_then(|value| value.as_f64())
                    .filter(|value| value.is_finite() && *value > 0.0)
                    .map(|value| value.floor().max(1.0) as u64)
                    .map(|duration| duration.saturating_sub(2))
                    .map_or_else(
                        || {
                            try_send_command_to_managed_mpv(
                                hwnd,
                                r#"{"command":["seek",100,"absolute-percent"]}"#,
                            )
                        },
                        |target| {
                            try_send_command_to_managed_mpv(
                                hwnd,
                                &format!(r#"{{"command":["seek",{},"absolute"]}}"#, target),
                            )
                        },
                    );
                if result.is_ok() {
                    stop_mpv_subtitle_speech(hwnd, "seek_end");
                }
                result
            }
            PlayerCommand::Volume(delta) => {
                let volume_delta = if delta > 0.0 { 10 } else { -10 };
                let result = try_send_command_to_managed_mpv(
                    hwnd,
                    &format!(r#"{{"command":["add","volume",{}]}}"#, volume_delta),
                );
                if result.is_ok() {
                    announce_mpv_volume(hwnd);
                }
                result
            }
            PlayerCommand::VolumeReset => {
                let result = try_send_command_to_managed_mpv(
                    hwnd,
                    r#"{"command":["set_property","volume",100]}"#,
                );
                if result.is_ok() {
                    announce_mpv_volume(hwnd);
                }
                result
            }
            PlayerCommand::MuteToggle => {
                let result =
                    try_send_command_to_managed_mpv(hwnd, r#"{"command":["cycle","mute"]}"#);
                if result.is_ok() {
                    announce_mpv_volume(hwnd);
                }
                result
            }
            PlayerCommand::Speed(delta) => {
                let base_speed = sync_mpv_speed_status(hwnd).unwrap_or_else(|| {
                    with_state(hwnd, |state| {
                        state.active_mpv_status.as_ref().map(|s| s.speed)
                    })
                    .flatten()
                    .unwrap_or(1.0)
                });
                let new_speed = (base_speed + delta).clamp(0.5, 3.0);
                let result = try_send_command_to_managed_mpv(
                    hwnd,
                    &format!(r#"{{"command":["set_property","speed",{}]}}"#, new_speed),
                );
                if result.is_ok() {
                    let announced_speed = sync_mpv_speed_status(hwnd).unwrap_or(new_speed);
                    announce_player_speed(language, announced_speed);
                }
                result
            }
            PlayerCommand::SpeedReset => {
                let result = try_send_command_to_managed_mpv(
                    hwnd,
                    r#"{"command":["set_property","speed",1.0]}"#,
                );
                if result.is_ok() {
                    let announced_speed = sync_mpv_speed_status(hwnd).unwrap_or(1.0);
                    announce_player_speed(language, announced_speed);
                }
                result
            }
            PlayerCommand::Pitch(delta) => {
                let base_pitch = sync_mpv_pitch_status(hwnd).unwrap_or_else(|| {
                    with_state(hwnd, |state| {
                        state.active_mpv_status.as_ref().map(|s| s.pitch)
                    })
                    .flatten()
                    .unwrap_or(0.0)
                });
                let new_pitch = (base_pitch + delta).clamp(-12.0, 12.0);
                let result = apply_mpv_pitch(hwnd, new_pitch);
                if result.is_ok() {
                    let announced_pitch = sync_mpv_pitch_status(hwnd).unwrap_or(new_pitch);
                    announce_player_pitch(language, announced_pitch);
                }
                result
            }
            PlayerCommand::PitchReset => {
                let result = apply_mpv_pitch(hwnd, 0.0);
                if result.is_ok() {
                    let announced_pitch = sync_mpv_pitch_status(hwnd).unwrap_or(0.0);
                    announce_player_pitch(language, announced_pitch);
                }
                result
            }
            PlayerCommand::AnnounceTime => announce_mpv_time(hwnd),
            _ => {
                let message = i18n::tr(language, "playback.direct_stream_command_disabled");
                if !message.is_empty() {
                    crate::accessibility::screen_reader_speak(&message);
                }
                return;
            }
        };
        if let Err(err) = result {
            log_debug(&format!("Managed mpv command failed: {}", err));
            if !err.is_empty() {
                crate::accessibility::screen_reader_speak(&err);
            }
        }
        return;
    }
    let disable_seek_for_live_raiplay = matches!(
        command,
        PlayerCommand::Seek(_)
            | PlayerCommand::SeekToStart
            | PlayerCommand::SeekToEnd
            | PlayerCommand::GoToTime
    ) && is_raiplay_live_stream_playback_active(hwnd);
    let disable_seek_rate_pitch = matches!(
        command,
        PlayerCommand::Seek(_)
            | PlayerCommand::SeekToStart
            | PlayerCommand::SeekToEnd
            | PlayerCommand::GoToTime
            | PlayerCommand::Speed(_)
            | PlayerCommand::SpeedReset
            | PlayerCommand::Pitch(_)
            | PlayerCommand::PitchReset
    ) && is_direct_stream_playback_active(hwnd)
        && !is_raiplay_stream_playback_active(hwnd);
    if disable_seek_for_live_raiplay || disable_seek_rate_pitch {
        let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
        let message = i18n::tr(language, "playback.direct_stream_command_disabled");
        if !message.is_empty() {
            crate::accessibility::screen_reader_speak(&message);
        }
        return;
    }
    match command {
        PlayerCommand::TogglePause => {
            toggle_audiobook_pause(hwnd);
        }
        PlayerCommand::Stop => {
            let stopped_url =
                with_state(hwnd, |state| state.active_podcast_episode_url.clone()).flatten();
            save_automatic_bookmark_for_active_raiplaysound_episode(hwnd);
            stop_audiobook_playback(hwnd);
            let restored_archive =
                app_windows::internet_archive_window::restore_episode_list_after_stop(
                    hwnd,
                    stopped_url.as_deref(),
                );
            if !restored_archive {
                app_windows::librivox_window::restore_chapter_list_after_stop(
                    hwnd,
                    stopped_url.as_deref(),
                );
            }
        }
        PlayerCommand::StopOnly => {
            save_automatic_bookmark_for_active_raiplaysound_episode(hwnd);
            stop_audiobook_playback(hwnd);
        }
        PlayerCommand::Seek(amount) => {
            seek_audiobook(hwnd, amount);
        }
        PlayerCommand::SeekToStart => {
            if let Err(err) = seek_audiobook_to(hwnd, 0) {
                if err == "No active audiobook" {
                    let restart_path = with_state(hwnd, |state| {
                        let doc = state.docs.get(state.current)?;
                        if matches!(doc.format, FileFormat::Audiobook) {
                            doc.path.clone()
                        } else {
                            None
                        }
                    })
                    .flatten();
                    if let Some(path) = restart_path {
                        start_audiobook_at(hwnd, &path, 0);
                    }
                } else {
                    log_debug(&format!("Seek to start failed: {}", err));
                }
            }
        }
        PlayerCommand::SeekToEnd => {
            if let Some(result) = with_state(hwnd, |state| {
                let player = state.active_audiobook.as_ref()?;
                let live_total = player.duration_secs().map(|s| s.max(0.0).floor() as u64);
                let file_total = crate::audio_player::audiobook_duration_secs(&player.path);
                let total = match (file_total, live_total) {
                    // Prefer the larger duration to avoid BASS underestimation on long MP3.
                    (Some(file), Some(live)) => Some(file.max(live)),
                    (Some(file), None) => Some(file),
                    (None, Some(live)) => Some(live),
                    (None, None) => None,
                }?;
                // Seek just before the exact end to avoid immediate stop logic.
                let target = total.saturating_sub(1);
                Some(seek_audiobook_to(hwnd, target))
            })
            .flatten()
                && let Err(err) = result
            {
                log_debug(&format!("Seek to end failed: {}", err));
            }
        }
        PlayerCommand::Volume(delta) => {
            change_audiobook_volume(hwnd, delta);
            announce_player_volume(hwnd);
        }
        PlayerCommand::VolumeReset => {
            reset_audiobook_volume(hwnd);
            announce_player_volume(hwnd);
        }
        PlayerCommand::Speed(delta) => {
            let language =
                { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
            let speed = change_audiobook_speed(hwnd, delta);
            if let Some(speed) = speed {
                announce_player_speed(language, speed);
            }
        }
        PlayerCommand::Pitch(delta) => {
            let language =
                { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
            let pitch = change_audiobook_pitch(hwnd, delta);
            if let Some(pitch) = pitch {
                announce_player_pitch(language, pitch);
            }
        }
        PlayerCommand::SpeedReset => {
            let language =
                { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
            let speed = reset_audiobook_speed(hwnd);
            if let Some(speed) = speed {
                announce_player_speed(language, speed);
            }
        }
        PlayerCommand::PitchReset => {
            let language =
                { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
            let pitch = reset_audiobook_pitch(hwnd);
            if let Some(pitch) = pitch {
                announce_player_pitch(language, pitch);
            }
        }
        PlayerCommand::MuteToggle => {
            toggle_audiobook_mute(hwnd);
        }
        PlayerCommand::GoToTime => {
            app_windows::go_to_time_window::open(hwnd);
        }
        PlayerCommand::AnnounceTime => {
            announce_player_time(hwnd);
        }
        PlayerCommand::ChapterPrev => {
            handle_chapter_navigation(hwnd, -1);
        }
        PlayerCommand::ChapterNext => {
            handle_chapter_navigation(hwnd, 1);
        }
        PlayerCommand::ChapterList => {
            handle_chapter_list(hwnd);
        }
        PlayerCommand::TrackPrev => {
            if !switch_audio_playlist_relative(hwnd, -1) {
                crate::log_debug("Audio player: no previous track available in playlist");
            }
        }
        PlayerCommand::TrackNext => {
            if !switch_audio_playlist_relative(hwnd, 1) {
                crate::log_debug("Audio player: no next track available in playlist");
            }
        }
        PlayerCommand::BlockNavigation | PlayerCommand::None => {}
    }
}

fn has_secondary_window_open(hwnd: HWND) -> bool {
    if app_windows::calendar_window::has_active_reminder_alert() {
        return true;
    }
    {
        with_state(hwnd, |state| {
            state.blocking_modal.active.is_some()
                || state.find_dialog.0 != 0
                || state.replace_dialog.0 != 0
                || state.options_dialog.0 != 0
                || state.help_window.0 != 0
                || state.changelog_window.0 != 0
                || state.donations_window.0 != 0
                || state.feedback_window.0 != 0
                || state.bookmarks_window.0 != 0
                || state.dictionary_window.0 != 0
                || state.dictionary_entry_dialog.0 != 0
                || state.wiktionary_window.0 != 0
                || state.wikipedia_window.0 != 0
                || state.treccani_window.0 != 0
                || state.bdciechi_window.0 != 0
                || state.prompt_window.0 != 0
                || state.podcast_window.0 != 0
                || state.podcast_save_window.0 != 0
                || state.replace_progress_window.0 != 0
                || state.update_progress_window.0 != 0
                || state.transcription_progress_window.0 != 0
                || state.batch_audiobooks_window.0 != 0
                || state.convert_audio_window.0 != 0
                || app_windows::audio_description_window::blocks_parent_focus(
                    hwnd,
                    state.audio_description_window,
                )
                || (state.audio_description_project_window.0 != 0
                    && unsafe { IsWindowVisible(state.audio_description_project_window).as_bool() })
                || state.media_split_window.0 != 0
                || state.radio_window.0 != 0
                || state.podcasts_window.0 != 0
                || state.podcasts_add_dialog.0 != 0
                || state.podcasts_categories_dialog.0 != 0
                || state.podcasts_description_dialog.0 != 0
                || state.rss_window.0 != 0
                || state.rss_add_dialog.0 != 0
                || state.go_to_time_dialog.0 != 0
        })
        .unwrap_or(false)
    }
}

fn should_force_editor_focus_on_foreground(hwnd: HWND) -> bool {
    unsafe {
        let foreground = GetForegroundWindow();
        with_state(hwnd, |state| {
            let current_doc_path = state
                .docs
                .get(state.current)
                .and_then(|doc| doc.path.clone());
            let is_reader_mode = state
                .docs
                .get(state.current)
                .map(|doc| matches!(doc.format, FileFormat::Audiobook))
                .unwrap_or(false);
            let blocking_modal_open = state.blocking_modal.active.is_some();
            let audiobook_progress_in_foreground =
                state.audiobook_progress.0 != 0 && foreground == state.audiobook_progress;
            let transcription_blocks_focus = state.transcription_progress_window.0 != 0
                && state.transcription_in_progress
                && state
                    .transcription_media_path
                    .as_ref()
                    .is_some_and(|media_path| {
                        state.transcription_media_path == current_doc_path
                            || (state.active_mpv_session.is_some()
                                && is_stream_cache_media(media_path)
                                && state
                                    .active_podcast_episode_url
                                    .as_ref()
                                    .zip(current_doc_path.as_ref())
                                    .is_some_and(|(url, doc_path)| {
                                        doc_path.to_string_lossy() == url.as_str()
                                    }))
                    });
            !blocking_modal_open
                && state.update_progress_window.0 == 0
                && !transcription_blocks_focus
                && state.bdciechi_window.0 == 0
                && state.replace_progress_window.0 == 0
                && state.radio_window.0 == 0
                && !app_windows::audio_description_window::blocks_parent_focus(
                    hwnd,
                    state.audio_description_window,
                )
                && (state.audio_description_project_window.0 == 0
                    || !IsWindowVisible(state.audio_description_project_window).as_bool())
                && !audiobook_progress_in_foreground
                && !is_reader_mode
        })
        .unwrap_or(false)
    }
}

fn force_active_editor_focus(hwnd: HWND) {
    if !should_force_editor_focus_on_foreground(hwnd) {
        return;
    }
    unsafe {
        if let Some(hwnd_edit) = get_active_edit(hwnd) {
            set_focus_safe(hwnd_edit);
            SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
            SendMessageW(hwnd_edit, WM_SETFOCUS, WPARAM(0), LPARAM(0));
            crate::log_if_err!(PostMessageW(
                hwnd,
                WM_NEXTDLGCTL,
                WPARAM(hwnd_edit.0 as usize),
                LPARAM(1)
            ));
            NotifyWinEvent(
                EVENT_OBJECT_FOCUS,
                hwnd_edit,
                OBJID_CLIENT.0,
                CHILDID_SELF as i32,
            );
            notify_editor_caret_changed(hwnd_edit);
        }
    }
}

fn schedule_editor_focus_retry(hwnd: HWND) {
    unsafe {
        if SetTimer(hwnd, FOCUS_EDITOR_TIMER_ID, 80, None) == 0 {
            crate::log_debug("Failed to set FOCUS_EDITOR_TIMER_ID");
        }
        if SetTimer(hwnd, FOCUS_EDITOR_TIMER_ID2, 200, None) == 0 {
            crate::log_debug("Failed to set FOCUS_EDITOR_TIMER_ID2");
        }
        if SetTimer(hwnd, FOCUS_EDITOR_TIMER_ID3, 350, None) == 0 {
            crate::log_debug("Failed to set FOCUS_EDITOR_TIMER_ID3");
        }
        if SetTimer(hwnd, FOCUS_EDITOR_TIMER_ID4, 600, None) == 0 {
            crate::log_debug("Failed to set FOCUS_EDITOR_TIMER_ID4");
        }
    }
}

pub(crate) fn schedule_mpv_bass_focus_debug_snapshots(hwnd: HWND) {
    unsafe {
        if SetTimer(hwnd, MPV_BASS_FOCUS_DEBUG_TIMER_ID1, 100, None) == 0 {
            crate::log_debug("Failed to set MPV_BASS_FOCUS_DEBUG_TIMER_ID1");
        }
        if SetTimer(hwnd, MPV_BASS_FOCUS_DEBUG_TIMER_ID2, 300, None) == 0 {
            crate::log_debug("Failed to set MPV_BASS_FOCUS_DEBUG_TIMER_ID2");
        }
        if SetTimer(hwnd, MPV_BASS_FOCUS_DEBUG_TIMER_ID3, 700, None) == 0 {
            crate::log_debug("Failed to set MPV_BASS_FOCUS_DEBUG_TIMER_ID3");
        }
        if SetTimer(hwnd, MPV_BASS_FOCUS_DEBUG_TIMER_ID4, 1200, None) == 0 {
            crate::log_debug("Failed to set MPV_BASS_FOCUS_DEBUG_TIMER_ID4");
        }
    }
}

fn schedule_mpv_esc_focus_debug_snapshots(hwnd: HWND) {
    unsafe {
        if SetTimer(hwnd, MPV_ESC_FOCUS_DEBUG_TIMER_ID1, 10, None) == 0 {
            crate::log_debug("Failed to set MPV_ESC_FOCUS_DEBUG_TIMER_ID1");
        }
        if SetTimer(hwnd, MPV_ESC_FOCUS_DEBUG_TIMER_ID2, 25, None) == 0 {
            crate::log_debug("Failed to set MPV_ESC_FOCUS_DEBUG_TIMER_ID2");
        }
        if SetTimer(hwnd, MPV_ESC_FOCUS_DEBUG_TIMER_ID3, 50, None) == 0 {
            crate::log_debug("Failed to set MPV_ESC_FOCUS_DEBUG_TIMER_ID3");
        }
        if SetTimer(hwnd, MPV_ESC_FOCUS_DEBUG_TIMER_ID4, 100, None) == 0 {
            crate::log_debug("Failed to set MPV_ESC_FOCUS_DEBUG_TIMER_ID4");
        }
        if SetTimer(hwnd, MPV_ESC_FOCUS_DEBUG_TIMER_ID5, 200, None) == 0 {
            crate::log_debug("Failed to set MPV_ESC_FOCUS_DEBUG_TIMER_ID5");
        }
        if SetTimer(hwnd, MPV_ESC_FOCUS_DEBUG_TIMER_ID6, 500, None) == 0 {
            crate::log_debug("Failed to set MPV_ESC_FOCUS_DEBUG_TIMER_ID6");
        }
    }
}

pub(crate) fn schedule_italiaonline_close_focus_debug_snapshots(hwnd: HWND) {
    unsafe {
        if SetTimer(hwnd, ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID1, 100, None) == 0 {
            crate::log_debug("Failed to set ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID1");
        }
        if SetTimer(hwnd, ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID2, 300, None) == 0 {
            crate::log_debug("Failed to set ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID2");
        }
        if SetTimer(hwnd, ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID3, 700, None) == 0 {
            crate::log_debug("Failed to set ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID3");
        }
        if SetTimer(hwnd, ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID4, 1200, None) == 0 {
            crate::log_debug("Failed to set ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID4");
        }
    }
}

pub(crate) fn restore_editor_focus(hwnd: HWND) {
    bring_window_to_foreground(hwnd);
    if should_force_editor_focus_on_foreground(hwnd) {
        force_active_editor_focus(hwnd);
        schedule_editor_focus_retry(hwnd);
    } else {
        focus_editor(hwnd);
    }
}

#[derive(Default)]
pub(crate) struct AppState {
    hwnd_tab: HWND,
    hwnd_status: HWND,
    local_mpv_video_hwnd: HWND,
    local_mpv_hidden_menu: HMENU,
    local_mpv_video_mode_active: bool,
    local_mpv_alt_menu_pending: bool,
    local_mpv_menu_visible: bool,
    alt_menu_suppressed: bool,
    alt_menu_used_with_key: bool,
    editor_translation_pending_speak: Option<String>,
    docs: Vec<Document>,
    current: usize,
    untitled_count: usize,
    hfont: HFONT,
    hfont_custom: bool,
    hmenu_recent: HMENU,
    recent_files: Vec<PathBuf>,
    settings: AppSettings,
    bookmarks: BookmarkStore,
    find_dialog: HWND,
    replace_dialog: HWND,
    options_dialog: HWND,
    help_window: HWND,
    changelog_window: HWND,
    donations_window: HWND,
    feedback_window: HWND,
    bookmarks_window: HWND,
    dictionary_window: HWND,
    dictionary_entry_dialog: HWND,
    wiktionary_window: HWND,
    wikipedia_window: HWND,
    treccani_window: HWND,
    bdciechi_window: HWND,
    bdciechi_session_username: String,
    bdciechi_session_password: String,
    bdciechi_session_nprov: String,
    bdciechi_session_catalog_rows: Vec<String>,
    bdciechi_session_authenticated: bool,
    prompt_window: HWND,
    podcast_window: HWND,
    podcast_save_window: HWND,
    replace_progress_window: HWND,
    update_progress_window: HWND,
    transcription_progress_window: HWND,
    batch_audiobooks_window: HWND,
    convert_audio_window: HWND,
    audio_description_window: HWND,
    audio_description_project_window: HWND,
    media_split_window: HWND,
    radio_window: HWND,
    podcasts_window: HWND,
    podcasts_add_dialog: HWND,
    podcasts_categories_dialog: HWND,
    podcasts_description_dialog: HWND,
    rss_window: HWND,
    rss_add_dialog: HWND, // Input dialog for RSS
    go_to_time_dialog: HWND,
    playback_menu: HMENU,
    find_msg: u32,
    find_text: Vec<u16>,
    replace_text: Vec<u16>,
    find_replace: Option<FINDREPLACEW>,
    replace_replace: Option<FINDREPLACEW>,
    last_find_flags: FINDREPLACE_FLAGS,
    find_use_regex: bool,
    find_dot_matches_newline: bool,
    find_wrap_around: bool,
    find_match_case: bool,
    find_whole_word: bool,
    find_replace_in_selection: bool,
    find_replace_in_all_docs: bool,
    replace_cancel_requested: bool,
    replace_cancel_token: Option<Arc<AtomicBool>>,
    podcast_save_cancel_token: Option<Arc<AtomicBool>>,
    editor_translation_progress_window: HWND,
    editor_translation_cancel_token: Option<Arc<AtomicBool>>,
    editor_progress_canceling_message: String,
    pdf_loading: Vec<PdfLoadingState>,
    next_timer_id: usize,
    tts_session: Option<TtsSession>,
    tts_next_session_id: u64,
    tts_last_offset: i32,
    tts_pending_start_pos: Option<i32>,
    tts_automatic_bookmark_position: Option<(HWND, String, i32)>,
    tts_sentence_nav_anchor: Option<(HWND, i32)>,
    edge_voices: Vec<VoiceInfo>,
    sapi_voices: Vec<VoiceInfo>,

    audiobook_progress: HWND,
    audiobook_cancel: Option<Arc<AtomicBool>>,
    blocking_modal: BlockingModalState,
    deferred_modal_copydata_open_paths: Vec<PathBuf>,
    active_audiobook: Option<AudiobookPlayer>,
    active_audiobook_bookmark: Option<(String, i32)>,
    audiobook_session_id: u64,
    audiobook_playback_generation: u64,
    last_stopped_audiobook: Option<std::path::PathBuf>,
    last_stopped_audiobook_position_secs: Option<u64>,
    stopped_audiobook_positions: HashMap<PathBuf, u64>,
    active_podcast_episode_url: Option<String>,
    active_podcast_episode_media_url: Option<String>,
    active_podcast_title: Option<String>,
    active_podcast_episode_title: Option<String>,
    active_podcast_episode_cache: Option<PathBuf>,
    active_podcast_episode_from_rai: RaiAudioOrigin,
    raiplay_live_audio_variants: Vec<RaiPlayLiveAudioVariant>,
    active_mpv_session: Option<MpvPlaybackSession>,
    active_mpv_is_live_tv: bool,
    active_mpv_playback_generation: u64,
    active_mpv_ipc: Option<std::fs::File>,
    active_mpv_subtitle_generation: u64,
    next_mpv_request_id: u64,
    active_mpv_status: Option<MpvPlaybackStatus>,
    last_stopped_mpv_url: Option<String>,
    last_stopped_mpv_position_secs: Option<u64>,
    last_stopped_mpv_origin: RaiAudioOrigin,
    active_youtube_return_context: YouTubeReturnContext,
    last_italiaonline_query: Option<crate::tools::italiaonline::SearchQuery>,
    last_italiaonline_result_id: Option<String>,
    last_rai_recent_item_id: Option<String>,
    last_rai_grouped_item_id: Option<String>,
    raiplay_navigation_stack: Vec<(String, Option<String>)>,
    last_raiplay_page_path: Option<String>,
    last_raiplay_item_id: Option<String>,
    la7_play_navigation_stack: Vec<(String, Option<String>)>,
    last_la7_play_page_path: Option<String>,
    last_la7_play_item_id: Option<String>,
    last_la7_play_search_query: String,
    raiplaysound_navigation_stack: Vec<(String, Option<String>)>,
    last_raiplaysound_page_path: Option<String>,
    last_raiplaysound_item_id: Option<String>,
    podcast_chapters_cache: HashMap<String, Option<Vec<Chapter>>>,
    pending_podcast_chapters_key: Option<String>,
    active_podcast_chapters_key: Option<String>,
    active_podcast_chapters: Vec<Chapter>,
    last_announced_chapter_index: Option<usize>,
    available_audio_tracks: Vec<crate::ffmpeg_source::AudioStreamInfo>,
    selected_audio_track: Option<i32>,
    audio_playlist: Vec<PathBuf>,
    audio_playlist_index: Option<usize>,
    audio_ffmpeg_retry_for: Option<PathBuf>,
    audio_unexpected_stop_retry_for: Option<PathBuf>,
    transcription_cancel: Option<Arc<AtomicBool>>,
    transcription_in_progress: bool,
    transcription_media_path: Option<PathBuf>,
    dictation_recorder: Option<podcast_recorder::RecorderHandle>,
    dictation_target_edit: HWND,
    dictation_cancel: Option<Arc<AtomicBool>>,
    dictation_transcribing: bool,
    dictation_session_id: u64,
    voice_panel_visible: bool,
    voice_label_engine: HWND,
    voice_combo_engine: HWND,
    voice_label_language: HWND,
    voice_combo_language: HWND,
    voice_language_codes: Vec<String>,
    voice_label_voice: HWND,
    voice_combo_voice: HWND,
    voice_button_insert_tag: HWND,
    voice_button_manage_google_voices: HWND,
    voice_button_insert_pause: HWND,
    voice_label_speed: HWND,
    voice_combo_speed: HWND,
    voice_edit_speed: HWND,
    voice_label_pitch: HWND,
    voice_combo_pitch: HWND,
    voice_edit_pitch: HWND,
    voice_label_volume: HWND,
    voice_combo_volume: HWND,
    voice_edit_volume: HWND,
    voice_checkbox_multilingual: HWND,
    voice_favorites_visible: bool,
    voice_label_favorites: HWND,
    voice_combo_favorites: HWND,
    voice_combo_voice_proc: WNDPROC,
    voice_combo_favorites_proc: WNDPROC,
    voice_context_voice: Option<FavoriteVoice>,
    find_in_files_cache: Option<FindInFilesCache>,
    pending_find_in_files: Option<PendingFindInFilesSelection>,
    normalize_undo: Option<NormalizeUndo>,
    undo_action_label: Option<String>,
    normalize_skip_change: bool,
    spellcheck_manager: spellcheck::SpellcheckManager,
    spellcheck_last_announce: Option<SpellcheckAnnounceKey>,
    spellcheck_context: Option<SpellcheckContextMenuState>,
    spellcheck_space_trigger: Option<HWND>,
    spellcheck_typing_in_progress: bool,
    spellcheck_highlight_pending: Option<HWND>,
    spellcheck_last_highlighted_line: Option<(isize, i32)>, // (doc_id, line_index)
    dictionary_context_menu: HMENU,
    dictionary_context_word: String,
    dictionary_context_language: Language,
    dictionary_context_pref: String,
    dictionary_context_loaded: bool,
    dictionary_context_expanded: bool,
    dictionary_cache: HashMap<String, Vec<String>>,
    dictionary_pending_lookup: Option<String>,
    dictionary_prefetch_generation: usize,
    large_text_editors: HashSet<isize>,
}

#[derive(Clone)]
pub(crate) struct PendingFindInFilesSelection {
    pub(crate) path: PathBuf,
    pub(crate) snippet: String,
    pub(crate) term: String,
    pub(crate) start_utf16: i32,
    pub(crate) len_utf16: i32,
}

#[derive(Default, Serialize, Deserialize)]
struct RecentFileStore {
    files: Vec<String>,
}

struct WhisperTranscriptionResult {
    title: String,
    text: String,
    error_message: Option<String>,
    completed_message: String,
    cancelled: bool,
}

struct DeferredWhisperTranscription {
    media_path: PathBuf,
    media_title: Option<String>,
    close_raiplay_on_success: bool,
    close_la7_on_success: bool,
}

struct DeferredYoutubeContextAudioDescription {
    input_path: PathBuf,
}

struct DeferredRaiplayContextAudioDescription {
    input_path: PathBuf,
}

struct DeferredLa7ContextAudioDescription {
    input_path: PathBuf,
}

struct DictationResult {
    session_id: u64,
    text: String,
    error: Option<String>,
    cancelled: bool,
    target_edit: HWND,
}

fn main() -> windows::core::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = tts_engine::run_sapi5_audiobook_worker_from_args(&args) {
        std::process::exit(code);
    }

    // Initialize COM for the main UI thread (STA)
    let _com = com_guard::ComGuard::new_sta().ok();

    // Estrai le dipendenze embedded (DLL, certificati, ecc.)
    if let Err(e) = embedded_deps::extract_all() {
        log_debug(&format!("Warning: Failed to extract embedded deps: {}", e));
    }
    log_debug("Application started.");

    if let Some(index) = args.iter().position(|arg| arg == "--scheduled-recording") {
        let code = args
            .get(index + 1)
            .filter(|value| !value.trim().is_empty())
            .map(|id| app_windows::scheduled_recording_window::run_scheduled_recording(id.trim()))
            .unwrap_or(2);
        std::process::exit(code);
    }
    if args.iter().any(|arg| arg == "--self-update") {
        match updater::run_self_update(&args) {
            Ok(code) => std::process::exit(code),
            Err(err) => {
                log_debug(&format!("Self-update failed: {err}"));
                std::process::exit(2);
            }
        }
    }
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_cli_help();
        std::process::exit(0);
    }
    if args.iter().any(|arg| arg == "--version") {
        println!("Sonarpad {}", app_display_version());
        std::process::exit(0);
    }
    let show_update_completed = args.iter().any(|arg| arg == "--after-update-completed");
    let force_new_window = args.iter().any(|arg| arg == "--new-window");
    let mut calendar_reminder_id = None;
    let mut audio_description_input = None;
    let mut filtered_args = Vec::with_capacity(args.len());
    let mut index = 0usize;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--after-update-completed" || argument == "--new-window" {
            index += 1;
            continue;
        }
        if argument == "--calendar-reminder" {
            if let Some(value) = args.get(index + 1) {
                calendar_reminder_id = Some(value.clone());
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if argument == "--audio-description-input" {
            if let Some(value) = args.get(index + 1) {
                audio_description_input = Some(value.clone());
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        filtered_args.push(argument.clone());
        index += 1;
    }
    updater::cleanup_backup_on_start();
    updater::cleanup_update_lock_on_start();
    updater::cleanup_update_temp_on_start();

    // Inizializza Sentry per crash reporting (opt-in)
    {
        let settings = load_settings();
        sentry_integration::init(settings.send_crash_reports, option_env!("SENTRY_DSN"));
    }

    // Inizializza telemetry per hang diagnostics
    telemetry::init();

    // Installa panic hook (logga + invia a Sentry)
    sentry_integration::install_panic_hook();

    // Error boundary: cattura errori fatali
    if let Err(e) = run_app(
        &filtered_args,
        show_update_completed,
        calendar_reminder_id.as_deref(),
        audio_description_input.as_deref(),
        force_new_window,
    ) {
        sentry_integration::capture_fatal_windows_error("run_app", &e);
        sentry_integration::flush(2);
        return Err(e);
    }

    Ok(())
}

fn print_cli_help() {
    println!("Sonarpad {}", app_display_version());
    println!("Usage:");
    println!("  sonarpad.exe [OPTIONS] [FILES...]");
    println!();
    println!("Options:");
    println!("  -h, --help         Show this help message and exit");
    println!("  --version          Show version and exit");
    println!("  --self-update      Internal updater mode (do not use manually)");
    println!("  --after-update-completed  Internal updater handoff (do not use manually)");
    println!();
    println!("Arguments:");
    println!("  FILES...           One or more files to open");
}

fn is_large_text_editor(hwnd: HWND, hwnd_edit: HWND) -> bool {
    {
        with_state(hwnd, |state| {
            state.large_text_editors.contains(&hwnd_edit.0)
        })
        .unwrap_or(false)
    }
}

/// Core dell'applicazione - separato per error boundary
fn run_app(
    args: &[String],
    show_update_completed: bool,
    calendar_reminder_id: Option<&str>,
    audio_description_input: Option<&str>,
    force_new_window: bool,
) -> windows::core::Result<()> {
    unsafe {
        crate::log_if_err!(LoadLibraryW(w!("Msftedit.dll")));
        let hinstance = HINSTANCE(GetModuleHandleW(None)?.0);
        let mut settings = load_settings();
        theme::initialize(settings.dark_mode);
        let class_name = w!("SonarpadWin32");

        let wc = WNDCLASSW {
            hCursor: HCURSOR(LoadCursorW(None, IDC_ARROW)?.0),
            hIcon: HICON(LoadIconW(None, IDI_APPLICATION)?.0),
            hInstance: hinstance,
            lpszClassName: class_name,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let extra_paths: Vec<String> = if args.len() > 1 {
            args[1..].to_vec()
        } else {
            Vec::new()
        };
        let current_version = env!("CARGO_PKG_VERSION");
        if settings.last_seen_changelog_version != current_version
            && app_windows::rss_window::sync_default_sources_for_settings(&mut settings)
        {
            save_settings(settings.clone());
        }
        crate::settings::sync_start_menu_shortcuts(&settings);
        if let Some(reminder_id) = calendar_reminder_id.filter(|value| !value.trim().is_empty()) {
            let existing = FindWindowW(class_name, PCWSTR::null());
            if existing.0 != 0 {
                let wide = to_wide(reminder_id.trim());
                let mut cds = COPYDATASTRUCT {
                    dwData: COPYDATA_CALENDAR_REMINDER,
                    cbData: (wide.len() * 2) as u32,
                    lpData: wide.as_ptr() as *mut std::ffi::c_void,
                };
                let mut existing_pid = 0u32;
                let existing_thread = GetWindowThreadProcessId(existing, Some(&mut existing_pid));
                if existing_thread == 0 {
                    log_debug(
                        "GetWindowThreadProcessId failed for existing calendar reminder window",
                    );
                } else if existing_pid != 0 {
                    crate::log_if_err!(AllowSetForegroundWindow(existing_pid));
                }
                SendMessageW(
                    existing,
                    WM_COPYDATA,
                    WPARAM(0),
                    LPARAM(&mut cds as *mut _ as isize),
                );
                return Ok(());
            }
        }
        if !force_new_window
            && !extra_paths.is_empty()
            && settings.open_behavior == OpenBehavior::NewTab
        {
            let existing = FindWindowW(class_name, PCWSTR::null());
            if existing.0 != 0 {
                // Send paths to existing window via WM_COPYDATA
                let joined = extra_paths.join("|");
                let wide = to_wide(&joined);
                let mut cds = COPYDATASTRUCT {
                    dwData: 1, // 1 = open files
                    cbData: (wide.len() * 2) as u32,
                    lpData: wide.as_ptr() as *mut std::ffi::c_void,
                };
                let mut existing_pid = 0u32;
                let existing_thread = GetWindowThreadProcessId(existing, Some(&mut existing_pid));
                if existing_thread == 0 {
                    log_debug("GetWindowThreadProcessId failed for existing window");
                } else if existing_pid != 0 {
                    crate::log_if_err!(AllowSetForegroundWindow(existing_pid));
                }
                let copydata_result = SendMessageW(
                    existing,
                    WM_COPYDATA,
                    WPARAM(0),
                    LPARAM(&mut cds as *mut _ as isize),
                );
                if should_focus_existing_window_after_copydata(copydata_result.0) {
                    bring_window_to_foreground(existing);
                } else {
                    log_debug(
                        "Existing Sonarpad instance deferred the file open because a modal dialog is active; leaving that dialog in the foreground",
                    );
                }
                return Ok(());
            }
        }
        let lp_param = &extra_paths as *const Vec<String> as *const std::ffi::c_void;

        let title_wide = to_wide(crate::settings::app_display_name(&settings));
        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            PCWSTR(title_wide.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            900,
            700,
            None,
            None,
            hinstance,
            Some(lp_param),
        );

        if hwnd.0 == 0 {
            return Ok(());
        }
        theme::apply_to_window(hwnd);
        refresh_voice_panel(hwnd);
        app_windows::calendar_window::initialize_reminder_system(hwnd, calendar_reminder_id);
        if let Some(path) = audio_description_input
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let input = PathBuf::from(path);
            if input.is_file() {
                log_debug(&format!(
                    "Opening audio-description window for completed TV recording: {}",
                    input.display()
                ));
                app_windows::audio_description_window::open_with_input(hwnd, input);
            } else {
                log_debug(&format!(
                    "Completed TV recording could not be opened for audio description because the file is unavailable: {}",
                    input.display()
                ));
            }
        }
        crate::log_if_err!(PostMessageW(
            hwnd,
            WM_CHECK_PENDING_UPDATE,
            WPARAM(0),
            LPARAM(0)
        ));
        let mut show_changelog = false;
        let mut cleanup_legacy_context_menu = false;
        with_state(hwnd, |state| {
            let last_seen = state.settings.last_seen_changelog_version.clone();
            if last_seen.is_empty() {
                state.settings.last_seen_changelog_version = current_version.to_string();
                save_settings(state.settings.clone());
                return;
            }
            if last_seen != current_version {
                state.settings.last_seen_changelog_version = current_version.to_string();
                save_settings(state.settings.clone());
                show_changelog = true;
                cleanup_legacy_context_menu = true;
            }
        });
        if cleanup_legacy_context_menu {
            crate::settings::cleanup_legacy_context_menu_entries();
        }
        if show_update_completed {
            if SetTimer(
                hwnd,
                STARTUP_UPDATE_CHANGELOG_TIMER_ID,
                STARTUP_UPDATE_CHANGELOG_INITIAL_DELAY_MS,
                None,
            ) == 0
            {
                log_debug(
                    "Failed to schedule the delayed update changelog; opening it immediately",
                );
                if let Err(e) = PostMessageW(hwnd, WM_SHOW_UPDATE_CHANGELOG, WPARAM(0), LPARAM(0)) {
                    crate::log_debug(&format!(
                        "Failed to post startup update changelog message: {}",
                        e
                    ));
                }
            } else {
                log_debug(&format!(
                    "Update changelog scheduled after startup focus restoration ({} ms)",
                    STARTUP_UPDATE_CHANGELOG_INITIAL_DELAY_MS
                ));
            }
        } else if show_changelog
            && let Err(e) = PostMessageW(hwnd, WM_SHOW_CHANGELOG, WPARAM(0), LPARAM(0))
        {
            crate::log_debug(&format!("Failed to post show changelog message: {}", e));
        }

        let check_updates = !show_update_completed
            && with_state(hwnd, |state| state.settings.check_updates_on_startup).unwrap_or(true);
        if check_updates {
            let hwnd_val = hwnd.0;
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if let Err(e) =
                    PostMessageW(HWND(hwnd_val), WM_AUTO_UPDATE_CHECK, WPARAM(0), LPARAM(0))
                {
                    crate::log_debug(&format!("Failed to post startup update check: {}", e));
                }
            });
        }

        let accel = create_accelerators();
        let mut msg = MSG::default();

        // Avvia watchdog per rilevare freeze
        let watchdog = watchdog::start_watchdog(watchdog::WatchdogConfig::default());

        while GetMessageW(&mut msg, HWND(0), 0, 0).into() {
            // Keep the watchdog aligned with UI message-loop activity.
            watchdog.heartbeat();
            if app_windows::calendar_window::handle_reminder_alert_message(&msg) {
                continue;
            }
            log_alt_raw_key_probe(hwnd, &msg);
            // Priority 1: Global navigation keys (Ctrl+Tab)
            if msg.message == WM_KEYDOWN
                && msg.wParam.0 as u32 == VK_TAB.0 as u32
                && (GetKeyState(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0
            {
                let options_hwnd =
                    with_state(hwnd, |state| state.options_dialog).unwrap_or(HWND(0));
                if options_hwnd.0 != 0 {
                    if app_windows::options_window::handle_navigation(options_hwnd, &msg) {
                        continue;
                    }
                } else {
                    // Switch tabs in main window
                    let tab_hwnd = with_state(hwnd, |state| state.hwnd_tab).unwrap_or(HWND(0));
                    if tab_hwnd.0 != 0 {
                        let count = SendMessageW(
                            tab_hwnd,
                            windows::Win32::UI::Controls::TCM_GETITEMCOUNT,
                            WPARAM(0),
                            LPARAM(0),
                        )
                        .0;
                        if count > 1 {
                            let cur = SendMessageW(
                                tab_hwnd,
                                windows::Win32::UI::Controls::TCM_GETCURSEL,
                                WPARAM(0),
                                LPARAM(0),
                            )
                            .0;
                            let shift_down =
                                (GetKeyState(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
                            let next = if shift_down {
                                if cur == 0 { count - 1 } else { cur - 1 }
                            } else if cur == count - 1 {
                                0
                            } else {
                                cur + 1
                            };
                            editor_manager::select_tab(hwnd, next as usize);
                        }
                    }
                    continue;
                }
            }
            if msg.message == WM_CONTEXTMENU && msg.lParam.0 == -1 {
                let rss_hwnd = with_state(hwnd, |state| state.rss_window).unwrap_or(HWND(0));
                if rss_hwnd.0 != 0 {
                    let mut cur = msg.hwnd;
                    let mut rss_target = false;
                    while cur.0 != 0 {
                        if cur == rss_hwnd {
                            app_windows::rss_window::show_context_menu_from_keyboard(rss_hwnd);
                            rss_target = true;
                            break;
                        }
                        cur = GetParent(cur);
                    }
                    if rss_target {
                        continue;
                    }
                }
                let podcasts_hwnd =
                    with_state(hwnd, |state| state.podcasts_window).unwrap_or(HWND(0));
                if podcasts_hwnd.0 != 0 {
                    let mut cur = msg.hwnd;
                    let mut podcasts_target = false;
                    while cur.0 != 0 {
                        if cur == podcasts_hwnd {
                            app_windows::podcasts_window::show_context_menu_from_keyboard(
                                podcasts_hwnd,
                            );
                            podcasts_target = true;
                            break;
                        }
                        cur = GetParent(cur);
                    }
                    if podcasts_target {
                        continue;
                    }
                }
            }
            if msg.message == WM_KEYDOWN || msg.message == WM_SYSKEYDOWN {
                let key = msg.wParam.0 as u32;
                if key != u32::from(VK_MENU.0) {
                    with_state(hwnd, |state| {
                        if state.alt_menu_suppressed {
                            state.alt_menu_used_with_key = true;
                        }
                    });
                }
                if msg.message == WM_SYSKEYDOWN && key == u32::from(VK_MENU.0) {
                    let alt_down =
                        (crate::get_key_state_safe(VK_MENU.0 as i32) & (0x8000u16 as i16)) != 0;
                    let shift_down =
                        (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
                    let ctrl_down =
                        (crate::get_key_state_safe(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
                    let main_window_enabled = IsWindowEnabled(hwnd).as_bool();
                    let should_show_menu = with_state(hwnd, |state| {
                        state.local_mpv_alt_menu_pending = false;
                        alt_down
                            && main_window_enabled
                            && state.local_mpv_video_mode_active
                            && state.local_mpv_hidden_menu.0 != 0
                    })
                    .unwrap_or(false);
                    log_debug(&format!(
                        "local_mpv_alt_down: alt_down={} should_show_menu={} main_enabled={} video_mode={} attached_menu={:?} hidden_menu={:?}",
                        alt_down,
                        should_show_menu,
                        main_window_enabled,
                        with_state(hwnd, |state| state.local_mpv_video_mode_active)
                            .unwrap_or(false),
                        crate::get_menu_safe(hwnd),
                        with_state(hwnd, |state| state.local_mpv_hidden_menu).unwrap_or(HMENU(0))
                    ));
                    if should_show_menu {
                        set_local_mpv_video_menu_visible(hwnd, true);
                    }
                    if alt_down && !should_show_menu {
                        with_state(hwnd, |state| {
                            state.alt_menu_suppressed = true;
                            state.alt_menu_used_with_key = shift_down || ctrl_down;
                        });
                        if !main_window_enabled {
                            log_debug(
                                "shortcut probe: suppress Alt menu while a modal progress window disables the main window",
                            );
                            reactivate_modal_child_while_main_disabled(hwnd);
                        } else {
                            log_debug(&format!(
                                "shortcut probe: suppress Alt menu activation shift_down={} ctrl_down={}",
                                shift_down, ctrl_down
                            ));
                        }
                        continue;
                    }
                }
                let is_context_key = key == u32::from(VK_APPS.0)
                    || (key == u32::from(VK_F10.0) && GetKeyState(VK_SHIFT.0 as i32) < 0);
                if is_context_key {
                    let rss_hwnd = with_state(hwnd, |state| state.rss_window).unwrap_or(HWND(0));
                    if rss_hwnd.0 != 0 {
                        let mut cur = msg.hwnd;
                        let mut rss_target = false;
                        while cur.0 != 0 {
                            if cur == rss_hwnd {
                                app_windows::rss_window::show_context_menu_from_keyboard(rss_hwnd);
                                rss_target = true;
                                break;
                            }
                            cur = GetParent(cur);
                        }
                        if rss_target {
                            continue;
                        }
                    }
                    let podcasts_hwnd =
                        with_state(hwnd, |state| state.podcasts_window).unwrap_or(HWND(0));
                    if podcasts_hwnd.0 != 0 {
                        let mut cur = msg.hwnd;
                        let mut podcasts_target = false;
                        while cur.0 != 0 {
                            if cur == podcasts_hwnd {
                                app_windows::podcasts_window::show_context_menu_from_keyboard(
                                    podcasts_hwnd,
                                );
                                podcasts_target = true;
                                break;
                            }
                            cur = GetParent(cur);
                        }
                        if podcasts_target {
                            continue;
                        }
                    }
                }
            }
            if msg.message == WM_SYSKEYUP && msg.wParam.0 as u32 == 'R' as u32 {
                let ctrl_down =
                    (crate::get_key_state_safe(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
                let shift_down =
                    (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
                let alt_down =
                    (crate::get_key_state_safe(VK_MENU.0 as i32) & (0x8000u16 as i16)) != 0;
                let should_dispatch = with_state(hwnd, |state| {
                    let should_dispatch =
                        state.alt_menu_suppressed && !ctrl_down && shift_down && alt_down;
                    if should_dispatch {
                        state.alt_menu_suppressed = false;
                        state.alt_menu_used_with_key = true;
                    }
                    should_dispatch
                })
                .unwrap_or(false);
                if should_dispatch {
                    log_debug("shortcut probe: WM_SYSKEYUP Alt+Shift+R -> radio");
                    dispatch_shortcut_command(hwnd, IDM_TOOLS_RADIO);
                    continue;
                }
            }
            if msg.message == WM_SYSKEYUP && msg.wParam.0 as u32 == u32::from(VK_MENU.0) {
                let show_alt_menu = with_state(hwnd, |state| {
                    let show = state.alt_menu_suppressed && !state.alt_menu_used_with_key;
                    state.alt_menu_suppressed = false;
                    state.alt_menu_used_with_key = false;
                    state.local_mpv_alt_menu_pending = false;
                    show
                })
                .unwrap_or(false);
                if show_alt_menu {
                    log_debug("shortcut probe: Alt released alone -> open menu");
                    crate::log_if_err!(PostMessageW(
                        hwnd,
                        WM_SYSCOMMAND,
                        WPARAM(SC_KEYMENU as usize),
                        LPARAM(0)
                    ));
                    continue;
                }
            }
            if msg.message == WM_MOUSEMOVE {
                let video_hwnd =
                    with_state(hwnd, |state| state.local_mpv_video_hwnd).unwrap_or(HWND(0));
                if msg.hwnd == hwnd || msg.hwnd == video_hwnd {
                    let mouse_y = ((msg.lParam.0 >> 16) & 0xFFFF) as i16 as i32;
                    let should_show_menu = mouse_y <= 32;
                    if with_state(hwnd, |state| state.local_mpv_video_mode_active).unwrap_or(false)
                    {
                        set_local_mpv_video_menu_visible(hwnd, should_show_menu);
                    }
                }
            }
            if (msg.message == WM_KEYDOWN || msg.message == WM_SYSKEYDOWN)
                && msg.wParam.0 as u32 == VK_ESCAPE.0 as u32
            {
                log_mpv_focus_snapshot(hwnd, "mpv_escape.pretranslate");
                if is_mpv_playback_active(hwnd) {
                    schedule_mpv_esc_focus_debug_snapshots(hwnd);
                }
                let rss_hwnd = with_state(hwnd, |state| state.rss_window).unwrap_or(HWND(0));
                if rss_hwnd.0 != 0
                    && let Some(hwnd_edit) = get_active_edit(hwnd)
                    && GetFocus() == hwnd_edit
                    && editor_manager::current_document_is_from_rss(hwnd)
                {
                    app_windows::rss_window::focus_library(rss_hwnd);
                    continue;
                }
                if let Some(hwnd_edit) = get_active_edit(hwnd)
                    && GetFocus() == hwnd_edit
                    && editor_manager::current_document_is_from_italiaonline(hwnd)
                {
                    app_windows::italiaonline_window::reopen_last(hwnd);
                    continue;
                }
                if let Some(hwnd_edit) = get_active_edit(hwnd)
                    && GetFocus() == hwnd_edit
                    && editor_manager::current_document_is_from_find_in_files(hwnd)
                {
                    app_windows::find_in_files_window::reopen_results(hwnd);
                    continue;
                }
                if let Some(hwnd_edit) = get_active_edit(hwnd)
                    && GetFocus() == hwnd_edit
                    && app_windows::gutenberg_window::current_document_has_return_context(hwnd)
                {
                    app_windows::gutenberg_window::reopen_results(hwnd);
                    continue;
                }
                let wikipedia_hwnd =
                    with_state(hwnd, |state| state.wikipedia_window).unwrap_or(HWND(0));
                if wikipedia_hwnd.0 != 0
                    && let Some(hwnd_edit) = get_active_edit(hwnd)
                    && GetFocus() == hwnd_edit
                    && editor_manager::current_document_is_from_wikipedia(hwnd)
                    && app_windows::wikipedia_window::focus_import_choice(wikipedia_hwnd)
                {
                    continue;
                }
                let treccani_hwnd =
                    with_state(hwnd, |state| state.treccani_window).unwrap_or(HWND(0));
                if treccani_hwnd.0 != 0
                    && let Some(hwnd_edit) = get_active_edit(hwnd)
                    && GetFocus() == hwnd_edit
                    && editor_manager::current_document_is_from_treccani(hwnd)
                    && app_windows::treccani_window::focus_import_choice(treccani_hwnd)
                {
                    continue;
                }
                let save_hwnd =
                    with_state(hwnd, |state| state.podcast_save_window).unwrap_or(HWND(0));
                if save_hwnd.0 != 0 {
                    crate::log_if_err!(PostMessageW(save_hwnd, WM_COMMAND, WPARAM(2), LPARAM(0)));
                    continue;
                }
                let update_progress_hwnd =
                    with_state(hwnd, |state| state.update_progress_window).unwrap_or(HWND(0));
                if update_progress_hwnd.0 != 0 {
                    crate::log_if_err!(PostMessageW(
                        update_progress_hwnd,
                        WM_COMMAND,
                        WPARAM(2),
                        LPARAM(0)
                    ));
                    continue;
                }
                let whisper_progress_hwnd =
                    with_state(hwnd, |state| state.transcription_progress_window)
                        .unwrap_or(HWND(0));
                if whisper_progress_hwnd.0 != 0 {
                    crate::log_if_err!(PostMessageW(
                        whisper_progress_hwnd,
                        WM_COMMAND,
                        WPARAM(2),
                        LPARAM(0)
                    ));
                    continue;
                }
                if let Some(hwnd_edit) = get_active_edit(hwnd)
                    && GetFocus() == hwnd_edit
                    && focus_find_in_files_results()
                {
                    continue;
                }
            }
            if (msg.message == WM_KEYDOWN
                && msg.wParam.0 as u32 == u32::from(VK_MEDIA_PLAY_PAUSE.0))
                || (msg.message == WM_APPCOMMAND
                    && appcommand_from_lparam(msg.lParam) == APPCOMMAND_MEDIA_PLAY_PAUSE)
            {
                let has_player = with_state(hwnd, |state| {
                    state.active_audiobook.is_some() || state.active_mpv_session.is_some()
                })
                .unwrap_or(false);
                if has_player {
                    handle_player_command(hwnd, PlayerCommand::TogglePause);
                    continue;
                }
                if is_tts_active(hwnd) {
                    tts_engine::toggle_tts_pause(hwnd);
                    continue;
                }
            }
            if msg.message == WM_SYSKEYDOWN && msg.wParam.0 as u32 == u32::from(VK_F4.0) {
                let (prompt_hwnd, prompt_open, podcast_open) = with_state(hwnd, |state| {
                    (
                        state.prompt_window,
                        state.prompt_window.0 != 0,
                        state.podcast_window.0 != 0
                            || state.podcast_save_window.0 != 0
                            || state.replace_progress_window.0 != 0
                            || state.update_progress_window.0 != 0
                            || state.transcription_progress_window.0 != 0,
                    )
                })
                .unwrap_or((HWND(0), false, false));
                let target = msg.hwnd;
                let target_parent = GetParent(target);
                let prompt_target = target == prompt_hwnd || target_parent == prompt_hwnd;
                let main_target = target == hwnd || target_parent == hwnd;
                if main_target && !prompt_target && (prompt_open || podcast_open) {
                    editor_manager::close_current_document(hwnd);
                    continue;
                }
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == 'Z' as u32 {
                let ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
                let shift_down = (GetKeyState(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
                let alt_down = (GetKeyState(VK_MENU.0 as i32) & (0x8000u16 as i16)) != 0;
                if ctrl_down
                    && !shift_down
                    && !alt_down
                    && let Some(hwnd_edit) = get_active_edit(hwnd)
                    && GetFocus() == hwnd_edit
                {
                    if !editor_manager::try_normalize_undo(hwnd) {
                        editor_manager::undo_active_edit_skip_navigation(hwnd);
                    }
                    continue;
                }
            }
            // The prompt/terminal window has special Ctrl+C/Ctrl+V handling. In particular,
            // multiline clipboard text must be written directly to the PTY instead of being
            // pasted into the single-line EDIT control. Give those shortcuts a chance before
            // the generic EDIT shortcut handler consumes them.
            let prompt_window = with_state(hwnd, |state| state.prompt_window).unwrap_or(HWND(0));
            let prompt_focus = GetFocus();
            let prompt_has_focus = prompt_window.0 != 0
                && prompt_focus.0 != 0
                && (prompt_focus == prompt_window || GetParent(prompt_focus) == prompt_window);
            let plain_prompt_ctrl_c_or_v = if msg.message == WM_KEYDOWN {
                let ctrl_down = (GetKeyState(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
                let shift_down = (GetKeyState(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
                let alt_down = (GetKeyState(VK_MENU.0 as i32) & (0x8000u16 as i16)) != 0;
                let key = msg.wParam.0 as u32;
                ctrl_down && !shift_down && !alt_down && (key == 'C' as u32 || key == 'V' as u32)
            } else {
                false
            };
            if prompt_has_focus
                && plain_prompt_ctrl_c_or_v
                && app_windows::prompt_window::handle_navigation(prompt_window, &msg)
            {
                continue;
            }

            let main_editor_focused = get_active_edit(hwnd).is_some_and(|edit| GetFocus() == edit);
            if !main_editor_focused && handle_focused_edit_shortcut(&msg) {
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == u32::from(VK_F1.0) {
                app_windows::help_window::open(hwnd);
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == u32::from(VK_F2.0) {
                updater::check_for_update(hwnd, true);
                continue;
            }
            if msg.message == WM_KEYDOWN
                && msg.wParam.0 as u32 == u32::from(VK_F9.0)
                && is_tts_active(hwnd)
            {
                cycle_favorite_voice(hwnd, -1);
                continue;
            }
            if (msg.message == WM_KEYDOWN || msg.message == WM_SYSKEYDOWN)
                && msg.wParam.0 as u32 == u32::from(VK_F10.0)
            {
                // F10 is normally used for menu, so only use it for voice cycling during TTS
                if is_tts_active(hwnd) {
                    cycle_favorite_voice(hwnd, 1);
                    continue;
                }
            }
            // F7/F8 for spelling navigation
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == u32::from(VK_F7.0) {
                go_to_spelling_error(hwnd, false);
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == u32::from(VK_F8.0) {
                go_to_spelling_error(hwnd, true);
                continue;
            }
            if msg.message == WM_KEYDOWN
                && msg.wParam.0 as u32 == VK_TAB.0 as u32
                && (GetKeyState(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) == 0
                && handle_voice_panel_tab(hwnd)
            {
                continue;
            }

            // Enter on the voice panel "Insert Tag" button: behave like the options dialog
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_RETURN.0 as u32 {
                let focus = GetFocus();
                if is_voice_panel_tuning_edit(hwnd, focus) {
                    handle_voice_panel_tuning_edit_change(hwnd);
                    continue;
                }
                let is_insert_tag = focus.0 != 0
                    && with_state(hwnd, |state| focus == state.voice_button_insert_tag)
                        .unwrap_or(false);
                if is_insert_tag {
                    insert_voice_tag_from_voice_panel(hwnd);
                    if let Some(hwnd_edit) = get_active_edit(hwnd) {
                        set_focus_safe(hwnd_edit);
                    }
                    continue;
                }
                let is_insert_pause = focus.0 != 0
                    && with_state(hwnd, |state| focus == state.voice_button_insert_pause)
                        .unwrap_or(false);
                if is_insert_pause {
                    show_pause_tag_menu_from_voice_panel(hwnd);
                    if let Some(hwnd_edit) = get_active_edit(hwnd) {
                        set_focus_safe(hwnd_edit);
                    }
                    continue;
                }
            }

            let mut handled = false;
            with_state(hwnd, |state| {
                // Audiobook keyboard controls (ONLY if no secondary window is open)
                if msg.message == WM_KEYDOWN {
                    let is_current_audiobook_doc = state
                        .docs
                        .get(state.current)
                        .map(|d| matches!(d.format, FileFormat::Audiobook))
                        .unwrap_or(false);
                    let is_audiobook =
                        is_current_audiobook_doc || should_route_player_command_to_mpv(hwnd);
                    let secondary_open = state.bookmarks_window.0 != 0
                        || state.options_dialog.0 != 0
                        || state.help_window.0 != 0
                        || state.changelog_window.0 != 0
                        || state.donations_window.0 != 0
                        || state.feedback_window.0 != 0
                        || state.dictionary_window.0 != 0;
                    let secondary_open = secondary_open
                        || state.dictionary_entry_dialog.0 != 0
                        || state.go_to_time_dialog.0 != 0
                        || state.podcasts_add_dialog.0 != 0
                        || state.podcasts_categories_dialog.0 != 0
                        || state.podcasts_description_dialog.0 != 0
                        || app_windows::audio_description_window::blocks_parent_focus(
                            hwnd,
                            state.audio_description_window,
                        )
                        || (state.audio_description_project_window.0 != 0
                            && IsWindowVisible(state.audio_description_project_window).as_bool());

                    // Exclude voice panel controls from player keyboard handling
                    let is_voice_panel_control = is_focus_in_voice_panel(hwnd);

                    let is_main_target = msg.hwnd == hwnd || IsChild(hwnd, msg.hwnd).as_bool();
                    if is_audiobook && !secondary_open && is_main_target && !is_voice_panel_control
                    {
                        let command =
                            handle_player_keyboard(&msg, state.settings.audiobook_skip_seconds);
                        if !matches!(command, PlayerCommand::None) {
                            if is_local_mpv_playback_active(hwnd)
                                && matches!(
                                    command,
                                    PlayerCommand::TrackPrev | PlayerCommand::TrackNext
                                )
                            {
                                let delta = if matches!(command, PlayerCommand::TrackPrev) {
                                    -1
                                } else {
                                    1
                                };
                                if !switch_audio_playlist_relative(hwnd, delta) {
                                    log_debug(
                                        "Audio player: no adjacent track available in playlist",
                                    );
                                }
                                handled = true;
                                return;
                            }
                            if matches!(command, PlayerCommand::BlockNavigation) {
                                handled = true;
                                return;
                            }
                            let is_stop = matches!(command, PlayerCommand::Stop);
                            let podcasts_window = state.podcasts_window;
                            let from_rai = state.active_podcast_episode_from_rai;
                            let youtube_return_context =
                                state.active_youtube_return_context.clone();
                            let stopped_url = state.active_podcast_episode_url.clone();
                            let stopped_audio_path = state
                                .active_audiobook
                                .as_ref()
                                .map(|player| player.path.clone());
                            let is_mpv = state.active_mpv_session.is_some();
                            if is_stop {
                                // close_current_document() already stops audiobook playback
                                // for audiobook tabs, so avoid duplicate stop work here.
                                if is_mpv {
                                    log_mpv_focus_snapshot(hwnd, "mpv_stop.before_stop");
                                    stop_managed_mpv_playback(hwnd);
                                    log_mpv_focus_snapshot(hwnd, "mpv_stop.after_stop");
                                    editor_manager::close_current_document(hwnd);
                                    log_foreground_snapshot("mpv_stop.after_close_document");
                                    schedule_mpv_esc_focus_debug_snapshots(hwnd);
                                } else {
                                    editor_manager::close_current_document(hwnd);
                                }
                                if daisy_player::restore_index_after_stop(
                                    hwnd,
                                    stopped_audio_path.as_deref(),
                                ) {
                                    handled = true;
                                    return;
                                }
                                if app_windows::audio_description_window::restore_after_player_stop(
                                    hwnd,
                                    stopped_audio_path.as_deref(),
                                ) {
                                    handled = true;
                                    return;
                                }
                                if from_rai == RaiAudioOrigin::Recenti {
                                    app_windows::rai_audiodescrizioni_window::open(hwnd);
                                } else if from_rai == RaiAudioOrigin::Tutte {
                                    app_windows::rai_audiodescrizioni_window::open_grouped(hwnd);
                                } else if from_rai == RaiAudioOrigin::RaiPlay {
                                    app_windows::raiplay_window::reopen_last(hwnd);
                                } else if from_rai == RaiAudioOrigin::La7Play {
                                    app_windows::la7_play_window::reopen_last(hwnd);
                                } else if from_rai == RaiAudioOrigin::RaiPlaySound {
                                    app_windows::raiplaysound_window::reopen_last(hwnd);
                                } else if youtube_return_context.input.is_some() {
                                    app_windows::youtube_transcript_window::reopen_stream_selection(
                                        hwnd,
                                        youtube_return_context,
                                    );
                                } else {
                                    let restored_catalog = app_windows::internet_archive_window::restore_episode_list_after_stop(
                                        hwnd,
                                        stopped_url.as_deref(),
                                    ) || app_windows::librivox_window::restore_chapter_list_after_stop(
                                        hwnd,
                                        stopped_url.as_deref(),
                                    );
                                    if !restored_catalog && podcasts_window.0 != 0 {
                                        SetForegroundWindow(podcasts_window);
                                        app_windows::podcasts_window::focus_library(
                                            podcasts_window,
                                        );
                                    }
                                }
                            } else {
                                handle_player_command(hwnd, command);
                            }
                            handled = true;
                            return;
                        }
                    }
                }

                if state.find_dialog.0 != 0 && handle_accessibility(state.find_dialog, &msg) {
                    handled = true;
                    return;
                }
                if state.replace_dialog.0 != 0 && handle_accessibility(state.replace_dialog, &msg) {
                    handled = true;
                    return;
                }
                if state.go_to_time_dialog.0 != 0
                    && app_windows::go_to_time_window::handle_navigation(
                        state.go_to_time_dialog,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.feedback_window.0 != 0
                    && app_windows::feedback_window::handle_navigation(state.feedback_window, &msg)
                {
                    handled = true;
                    return;
                }
                if app_windows::help_window::handle_readonly_navigation(&msg) {
                    handled = true;
                    return;
                }

                if state.help_window.0 != 0 {
                    // Manual TAB handling for Help window
                    if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_TAB.0 as u32 {
                        app_windows::help_window::handle_tab(state.help_window);
                        handled = true;
                        return;
                    }

                    if handle_accessibility(state.help_window, &msg) {
                        handled = true;
                        return;
                    }
                }
                if state.changelog_window.0 != 0 {
                    if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_TAB.0 as u32 {
                        app_windows::help_window::handle_tab(state.changelog_window);
                        handled = true;
                        return;
                    }

                    if handle_accessibility(state.changelog_window, &msg) {
                        handled = true;
                        return;
                    }
                }
                if state.donations_window.0 != 0 {
                    if msg.message == WM_KEYDOWN && msg.wParam.0 as u32 == VK_TAB.0 as u32 {
                        app_windows::help_window::handle_tab(state.donations_window);
                        handled = true;
                        return;
                    }

                    if handle_accessibility(state.donations_window, &msg) {
                        handled = true;
                        return;
                    }
                }

                if state.options_dialog.0 != 0
                    && app_windows::options_window::handle_navigation(state.options_dialog, &msg)
                {
                    handled = true;
                    return;
                }

                if state.podcast_window.0 != 0
                    && (msg.hwnd == state.podcast_window
                        || crate::is_child_safe(state.podcast_window, msg.hwnd))
                    && app_windows::podcast_window::handle_navigation(state.podcast_window, &msg)
                {
                    handled = true;
                    return;
                }

                if state.podcast_save_window.0 != 0
                    && (msg.hwnd == state.podcast_save_window
                        || crate::is_child_safe(state.podcast_save_window, msg.hwnd))
                    && app_windows::podcast_save_window::handle_navigation(
                        state.podcast_save_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.replace_progress_window.0 != 0
                    && (msg.hwnd == state.replace_progress_window
                        || crate::is_child_safe(state.replace_progress_window, msg.hwnd))
                    && app_windows::podcast_save_window::handle_navigation(
                        state.replace_progress_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.update_progress_window.0 != 0
                    && (msg.hwnd == state.update_progress_window
                        || crate::is_child_safe(state.update_progress_window, msg.hwnd))
                    && app_windows::podcast_save_window::handle_navigation(
                        state.update_progress_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.transcription_progress_window.0 != 0
                    && (msg.hwnd == state.transcription_progress_window
                        || crate::is_child_safe(state.transcription_progress_window, msg.hwnd))
                    && app_windows::podcast_save_window::handle_navigation(
                        state.transcription_progress_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }

                if state.audiobook_progress.0 != 0
                    && app_windows::audiobook_window::handle_navigation(
                        state.audiobook_progress,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }

                if state.bookmarks_window.0 != 0
                    && app_windows::bookmarks_window::handle_navigation(
                        state.bookmarks_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }

                if state.dictionary_window.0 != 0
                    && app_windows::dictionary_window::handle_navigation(
                        state.dictionary_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }

                if state.wiktionary_window.0 != 0
                    && app_windows::wiktionary_window::handle_navigation(
                        state.wiktionary_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.wikipedia_window.0 != 0
                    && app_windows::wikipedia_window::handle_navigation(
                        state.wikipedia_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.treccani_window.0 != 0
                    && app_windows::treccani_window::handle_navigation(state.treccani_window, &msg)
                {
                    handled = true;
                    return;
                }
                if state.bdciechi_window.0 != 0
                    && app_windows::bdciechi_window::handle_navigation(state.bdciechi_window, &msg)
                {
                    handled = true;
                    return;
                }

                if state.dictionary_entry_dialog.0 != 0
                    && handle_accessibility(state.dictionary_entry_dialog, &msg)
                {
                    handled = true;
                    return;
                }

                if state.batch_audiobooks_window.0 != 0
                    && app_windows::batch_audiobooks_window::handle_navigation(
                        state.batch_audiobooks_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.batch_audiobooks_window.0 != 0
                    && handle_accessibility(state.batch_audiobooks_window, &msg)
                {
                    handled = true;
                    return;
                }
                if state.convert_audio_window.0 != 0
                    && app_windows::convert_audio_window::handle_navigation(
                        state.convert_audio_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.convert_audio_window.0 != 0
                    && handle_accessibility(state.convert_audio_window, &msg)
                {
                    handled = true;
                    return;
                }
                // The project editor can be opened from the creation window, so route
                // keyboard and dialog messages to the topmost editor first.
                if state.audio_description_project_window.0 != 0
                    && IsWindowVisible(state.audio_description_project_window).as_bool()
                    && app_windows::audio_description_project_window::handle_navigation(
                        state.audio_description_project_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.audio_description_project_window.0 != 0
                    && IsWindowVisible(state.audio_description_project_window).as_bool()
                    && handle_accessibility(state.audio_description_project_window, &msg)
                {
                    handled = true;
                    return;
                }
                if state.audio_description_window.0 != 0
                    && IsWindowVisible(state.audio_description_window).as_bool()
                    && app_windows::audio_description_window::handle_navigation(
                        state.audio_description_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.audio_description_window.0 != 0
                    && IsWindowVisible(state.audio_description_window).as_bool()
                    && handle_accessibility(state.audio_description_window, &msg)
                {
                    handled = true;
                    return;
                }
                if state.media_split_window.0 != 0
                    && app_windows::media_split_window::handle_navigation(
                        state.media_split_window,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.media_split_window.0 != 0
                    && handle_accessibility(state.media_split_window, &msg)
                {
                    handled = true;
                    return;
                }

                if state.prompt_window.0 != 0
                    && app_windows::prompt_window::handle_navigation(state.prompt_window, &msg)
                {
                    handled = true;
                    return;
                }

                if state.rss_window.0 != 0 && handle_accessibility(state.rss_window, &msg) {
                    handled = true;
                    return;
                }

                if state.rss_add_dialog.0 != 0 && handle_accessibility(state.rss_add_dialog, &msg) {
                    handled = true;
                    return;
                }
                if state.podcasts_description_dialog.0 != 0
                    && app_windows::podcasts_window::handle_navigation(
                        state.podcasts_description_dialog,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.podcasts_categories_dialog.0 != 0
                    && app_windows::podcasts_window::handle_navigation(
                        state.podcasts_categories_dialog,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.podcasts_add_dialog.0 != 0
                    && app_windows::podcasts_window::handle_navigation(
                        state.podcasts_add_dialog,
                        &msg,
                    )
                {
                    handled = true;
                    return;
                }
                if state.podcasts_window.0 != 0
                    && app_windows::podcasts_window::handle_navigation(state.podcasts_window, &msg)
                {
                    handled = true;
                }
            });
            if handled {
                continue;
            }
            if handle_custom_shortcuts(hwnd, &msg) {
                continue;
            }
            if accel.0 != 0 && TranslateAcceleratorW(hwnd, accel, &msg) != 0 {
                if msg.message == WM_KEYDOWN {
                    let key = msg.wParam.0 as u32;
                    let ctrl_down =
                        (get_key_state_safe(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
                    if ctrl_down
                        && matches!(
                            key,
                            key if key == 'A' as u32
                                || key == 'C' as u32
                                || key == 'H' as u32
                                || key == 'V' as u32
                                || key == 'X' as u32
                                || key == 'Y' as u32
                                || key == 'Z' as u32
                        )
                    {
                        log_debug(&format!(
                            "edit shortcut: accelerator consumed Ctrl+{} msg_hwnd={:?} focus={:?}",
                            key as u8 as char,
                            msg.hwnd,
                            get_focus_safe()
                        ));
                    }
                }
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Ferma watchdog prima di uscire
        log_debug("Application shutdown: message loop exited");
        watchdog.stop();
        log_debug("Application shutdown: watchdog stop requested");

        Ok(())
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

// Keep inner `unsafe { ... }` blocks untouched here to avoid behavioral refactors in message handling.
fn wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if let Some(find_msg) = with_state(hwnd, |state| state.find_msg)
            && msg == find_msg
        {
            handle_find_message(hwnd, lparam);
            return LRESULT(0);
        }

        match msg {
            WM_CREATE => {
                let icc = INITCOMMONCONTROLSEX {
                    dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
                    dwICC: ICC_TAB_CLASSES | ICC_BAR_CLASSES | ICC_LISTVIEW_CLASSES,
                };
                InitCommonControlsEx(&icc);

                let hwnd_tab = CreateWindowExW(
                    Default::default(),
                    WC_TABCONTROLW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let hwnd_status = CreateWindowExW(
                    Default::default(),
                    STATUSCLASSNAMEW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(MAIN_STATUS_ID as isize),
                    HINSTANCE(0),
                    None,
                );
                let local_mpv_video_hwnd = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR::null(),
                    WS_CHILD | WS_CLIPCHILDREN,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );

                let find_msg = RegisterWindowMessageW(w!("commdlg_FindReplace"));
                let settings = load_settings();
                let hfont = create_ui_font(&settings.editor_font_face, None, settings.text_size)
                    .unwrap_or_else(|| HFONT(GetStockObject(DEFAULT_GUI_FONT).0));
                let bookmarks = load_bookmarks();
                let (_, recent_menu) = create_menus(hwnd, settings.language);
                let recent_files = load_recent_files();
                let panel_labels = voice_panel_labels(settings.language);
                let _panel_labels = panel_labels;
                let empty_label = to_wide("");
                let label_engine = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(empty_label.as_ptr()),
                    WS_CHILD,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let combo_engine = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    0,
                    0,
                    0,
                    140,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_ENGINE as isize),
                    HINSTANCE(0),
                    None,
                );
                let label_language = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(empty_label.as_ptr()),
                    WS_CHILD,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let combo_language = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    0,
                    0,
                    0,
                    140,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_LANGUAGE as isize),
                    HINSTANCE(0),
                    None,
                );
                let label_voice = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(empty_label.as_ptr()),
                    WS_CHILD,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let combo_voice = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    0,
                    0,
                    0,
                    160,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_VOICE as isize),
                    HINSTANCE(0),
                    None,
                );
                let button_insert_tag = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(empty_label.as_ptr()),
                    WS_CHILD | WS_TABSTOP,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_INSERT_TAG as isize),
                    HINSTANCE(0),
                    None,
                );
                let button_manage_google_voices = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(empty_label.as_ptr()),
                    WS_CHILD | WS_TABSTOP,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_MANAGE_GOOGLE_VOICES as isize),
                    HINSTANCE(0),
                    None,
                );
                let button_insert_pause = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(empty_label.as_ptr()),
                    WS_CHILD | WS_TABSTOP,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_INSERT_PAUSE as isize),
                    HINSTANCE(0),
                    None,
                );
                let label_speed = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(empty_label.as_ptr()),
                    WS_CHILD,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let combo_speed = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    0,
                    0,
                    0,
                    140,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_SPEED as isize),
                    HINSTANCE(0),
                    None,
                );
                let edit_speed = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_SPEED_EDIT as isize),
                    HINSTANCE(0),
                    None,
                );
                let label_pitch = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(empty_label.as_ptr()),
                    WS_CHILD,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let combo_pitch = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    0,
                    0,
                    0,
                    140,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_PITCH as isize),
                    HINSTANCE(0),
                    None,
                );
                let edit_pitch = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_PITCH_EDIT as isize),
                    HINSTANCE(0),
                    None,
                );
                let label_volume = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(empty_label.as_ptr()),
                    WS_CHILD,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let combo_volume = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    0,
                    0,
                    0,
                    140,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_VOLUME as isize),
                    HINSTANCE(0),
                    None,
                );
                let edit_volume = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_VOLUME_EDIT as isize),
                    HINSTANCE(0),
                    None,
                );
                let checkbox_multilingual = CreateWindowExW(
                    Default::default(),
                    WC_BUTTON,
                    PCWSTR(empty_label.as_ptr()),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_MULTILINGUAL as isize),
                    HINSTANCE(0),
                    None,
                );
                let label_favorites = CreateWindowExW(
                    Default::default(),
                    WC_STATIC,
                    PCWSTR(empty_label.as_ptr()),
                    WS_CHILD,
                    0,
                    0,
                    0,
                    0,
                    hwnd,
                    HMENU(0),
                    HINSTANCE(0),
                    None,
                );
                let combo_favorites = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    0,
                    0,
                    0,
                    160,
                    hwnd,
                    HMENU(VOICE_PANEL_ID_FAVORITES as isize),
                    HINSTANCE(0),
                    None,
                );
                let combo_voice_proc = if combo_voice.0 != 0 {
                    let proc_ptr = voice_combo_subclass_proc as *const () as usize;
                    let old = SetWindowLongPtrW(combo_voice, GWLP_WNDPROC, proc_ptr as isize);
                    crate::isize_to_wndproc_safe(old)
                } else {
                    None
                };
                let combo_favorites_proc = if combo_favorites.0 != 0 {
                    let proc_ptr = voice_combo_subclass_proc as *const () as usize;
                    let old = SetWindowLongPtrW(combo_favorites, GWLP_WNDPROC, proc_ptr as isize);
                    crate::isize_to_wndproc_safe(old)
                } else {
                    None
                };
                for control in [
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
                ] {
                    if control.0 != 0 && hfont.0 != 0 {
                        SendMessageW(control, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
                    }
                    ShowWindow(control, SW_HIDE);
                }
                let state = Box::new(AppState {
                    hwnd_tab,
                    hwnd_status,
                    local_mpv_video_hwnd,
                    local_mpv_hidden_menu: HMENU(0),
                    local_mpv_video_mode_active: false,
                    local_mpv_alt_menu_pending: false,
                    local_mpv_menu_visible: false,
                    alt_menu_suppressed: false,
                    alt_menu_used_with_key: false,
                    editor_translation_pending_speak: None,
                    docs: Vec::new(),
                    current: 0,
                    untitled_count: 0,
                    hfont,
                    hfont_custom: !settings.editor_font_face.trim().is_empty() && hfont.0 != 0,
                    hmenu_recent: recent_menu,
                    recent_files,
                    settings: settings.clone(),
                    bookmarks,
                    find_dialog: HWND(0),
                    replace_dialog: HWND(0),
                    options_dialog: HWND(0),
                    help_window: HWND(0),
                    changelog_window: HWND(0),
                    donations_window: HWND(0),
                    feedback_window: HWND(0),
                    bookmarks_window: HWND(0),
                    dictionary_window: HWND(0),
                    dictionary_entry_dialog: HWND(0),
                    wiktionary_window: HWND(0),
                    wikipedia_window: HWND(0),
                    treccani_window: HWND(0),
                    bdciechi_window: HWND(0),
                    bdciechi_session_username: String::new(),
                    bdciechi_session_password: String::new(),
                    bdciechi_session_nprov: String::new(),
                    bdciechi_session_catalog_rows: Vec::new(),
                    bdciechi_session_authenticated: false,
                    prompt_window: HWND(0),
                    podcast_window: HWND(0),
                    rss_window: HWND(0),
                    podcasts_window: HWND(0),
                    podcasts_add_dialog: HWND(0),
                    podcasts_categories_dialog: HWND(0),
                    podcasts_description_dialog: HWND(0),
                    rss_add_dialog: HWND(0),
                    go_to_time_dialog: HWND(0),
                    playback_menu: HMENU(0),
                    podcast_save_window: HWND(0),
                    replace_progress_window: HWND(0),
                    update_progress_window: HWND(0),
                    transcription_progress_window: HWND(0),
                    batch_audiobooks_window: HWND(0),
                    convert_audio_window: HWND(0),
                    audio_description_window: HWND(0),
                    audio_description_project_window: HWND(0),
                    media_split_window: HWND(0),
                    radio_window: HWND(0),

                    find_msg,
                    find_text: vec![0u16; 256],
                    replace_text: vec![0u16; 256],
                    find_replace: None,
                    replace_replace: None,
                    last_find_flags: FINDREPLACE_FLAGS(0),
                    find_use_regex: false,
                    find_dot_matches_newline: false,
                    find_wrap_around: true,
                    find_match_case: false,
                    find_whole_word: false,
                    find_replace_in_selection: false,
                    find_replace_in_all_docs: false,
                    replace_cancel_requested: false,
                    replace_cancel_token: None,
                    podcast_save_cancel_token: None,
                    editor_translation_progress_window: HWND(0),
                    editor_translation_cancel_token: None,
                    editor_progress_canceling_message: String::new(),
                    pdf_loading: Vec::new(),
                    next_timer_id: 1,
                    tts_session: None,
                    tts_next_session_id: 1,
                    tts_last_offset: 0,
                    tts_pending_start_pos: None,
                    tts_automatic_bookmark_position: None,
                    tts_sentence_nav_anchor: None,
                    edge_voices: Vec::new(),
                    sapi_voices: Vec::new(),

                    audiobook_progress: HWND(0),
                    audiobook_cancel: None,
                    blocking_modal: BlockingModalState::default(),
                    deferred_modal_copydata_open_paths: Vec::new(),
                    active_audiobook: None,
                    active_audiobook_bookmark: None,
                    audiobook_session_id: 0,
                    audiobook_playback_generation: 0,
                    last_stopped_audiobook: None,
                    last_stopped_audiobook_position_secs: None,
                    stopped_audiobook_positions: HashMap::new(),
                    active_podcast_episode_url: None,
                    active_podcast_episode_media_url: None,
                    active_podcast_title: None,
                    active_podcast_episode_title: None,
                    active_podcast_episode_cache: None,
                    active_podcast_episode_from_rai: RaiAudioOrigin::None,
                    raiplay_live_audio_variants: Vec::new(),
                    active_mpv_session: None,
                    active_mpv_is_live_tv: false,
                    active_mpv_playback_generation: 0,
                    active_mpv_ipc: None,
                    active_mpv_subtitle_generation: 0,
                    next_mpv_request_id: 1,
                    active_mpv_status: None,
                    last_stopped_mpv_url: None,
                    last_stopped_mpv_position_secs: None,
                    last_stopped_mpv_origin: RaiAudioOrigin::None,
                    active_youtube_return_context: YouTubeReturnContext::default(),
                    last_italiaonline_query: None,
                    last_italiaonline_result_id: None,
                    last_rai_recent_item_id: None,
                    last_rai_grouped_item_id: None,
                    raiplay_navigation_stack: Vec::new(),
                    last_raiplay_page_path: None,
                    last_raiplay_item_id: None,
                    la7_play_navigation_stack: Vec::new(),
                    last_la7_play_page_path: None,
                    last_la7_play_item_id: None,
                    last_la7_play_search_query: String::new(),
                    raiplaysound_navigation_stack: Vec::new(),
                    last_raiplaysound_page_path: None,
                    last_raiplaysound_item_id: None,
                    podcast_chapters_cache: HashMap::new(),
                    pending_podcast_chapters_key: None,
                    active_podcast_chapters_key: None,
                    active_podcast_chapters: Vec::new(),
                    last_announced_chapter_index: None,
                    available_audio_tracks: Vec::new(),
                    selected_audio_track: None,
                    audio_playlist: Vec::new(),
                    audio_playlist_index: None,
                    audio_ffmpeg_retry_for: None,
                    audio_unexpected_stop_retry_for: None,
                    transcription_cancel: None,
                    transcription_in_progress: false,
                    transcription_media_path: None,
                    dictation_recorder: None,
                    dictation_target_edit: HWND(0),
                    dictation_cancel: None,
                    dictation_transcribing: false,
                    dictation_session_id: 0,
                    voice_panel_visible: false,
                    voice_label_engine: label_engine,
                    voice_combo_engine: combo_engine,
                    voice_label_language: label_language,
                    voice_combo_language: combo_language,
                    voice_language_codes: Vec::new(),
                    voice_label_voice: label_voice,
                    voice_combo_voice: combo_voice,
                    voice_button_insert_tag: button_insert_tag,
                    voice_button_manage_google_voices: button_manage_google_voices,
                    voice_button_insert_pause: button_insert_pause,
                    voice_label_speed: label_speed,
                    voice_combo_speed: combo_speed,
                    voice_edit_speed: edit_speed,
                    voice_label_pitch: label_pitch,
                    voice_combo_pitch: combo_pitch,
                    voice_edit_pitch: edit_pitch,
                    voice_label_volume: label_volume,
                    voice_combo_volume: combo_volume,
                    voice_edit_volume: edit_volume,
                    voice_checkbox_multilingual: checkbox_multilingual,
                    voice_favorites_visible: false,
                    voice_label_favorites: label_favorites,
                    voice_combo_favorites: combo_favorites,
                    voice_combo_voice_proc: combo_voice_proc,
                    voice_combo_favorites_proc: combo_favorites_proc,
                    voice_context_voice: None,
                    find_in_files_cache: None,
                    pending_find_in_files: None,
                    normalize_undo: None,
                    undo_action_label: None,
                    normalize_skip_change: false,
                    spellcheck_manager: spellcheck::SpellcheckManager::default(),
                    spellcheck_last_announce: None,
                    spellcheck_context: None,
                    spellcheck_space_trigger: None,
                    spellcheck_typing_in_progress: false,
                    spellcheck_highlight_pending: None,
                    spellcheck_last_highlighted_line: None,
                    dictionary_context_menu: HMENU(0),
                    dictionary_context_word: String::new(),
                    dictionary_context_language: Language::default(),
                    dictionary_context_pref: String::new(),
                    dictionary_context_loaded: false,
                    dictionary_context_expanded: false,
                    dictionary_cache: load_dictionary_cache(),
                    dictionary_pending_lookup: None,
                    dictionary_prefetch_generation: 0,
                    large_text_editors: HashSet::new(),
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
                if SetTimer(hwnd, AUDIO_PLAYLIST_TIMER_ID, 700, None) == 0 {
                    log_debug("Failed to set AUDIO_PLAYLIST_TIMER");
                }

                update_recent_menu(hwnd, recent_menu);
                if settings.show_voice_panel {
                    set_voice_panel_visible_internal(hwnd, true, false);
                }
                if settings.show_favorite_panel {
                    set_favorites_panel_visible_internal(hwnd, true, false);
                }
                update_voice_panel_menu_check(hwnd);

                let create_struct = lparam.0 as *const CREATESTRUCTW;
                let lp_create_params = (*create_struct).lpCreateParams as *const Vec<String>;
                let file_paths: Vec<PathBuf> = if !lp_create_params.is_null() {
                    (*lp_create_params).iter().map(PathBuf::from).collect()
                } else {
                    Vec::new()
                };

                if file_paths.len() > 1 && file_paths.iter().all(|path| is_audio_path(path)) {
                    queue_audio_files_and_play(hwnd, file_paths);
                    ShowWindow(hwnd, SW_SHOWMAXIMIZED);
                    bring_window_to_foreground(hwnd);

                    notify_active_editor_focus(hwnd, true);
                    crate::log_if_err!(PostMessageW(hwnd, WM_FOCUS_EDITOR, WPARAM(0), LPARAM(0)));
                } else if let Some(path) = file_paths.first() {
                    editor_manager::open_document(hwnd, path);
                    ShowWindow(hwnd, SW_SHOWMAXIMIZED);
                    bring_window_to_foreground(hwnd);

                    notify_active_editor_focus(hwnd, true);
                    crate::log_if_err!(PostMessageW(hwnd, WM_FOCUS_EDITOR, WPARAM(0), LPARAM(0)));
                } else {
                    editor_manager::new_document(hwnd);
                    crate::log_if_err!(PostMessageW(hwnd, WM_FOCUS_EDITOR, WPARAM(0), LPARAM(0)));
                }

                editor_manager::layout_children(hwnd);
                editor_manager::apply_text_limit_to_all_edits(hwnd);
                update_main_status_bar(hwnd);
                DragAcceptFiles(hwnd, true);
                LRESULT(0)
            }
            WM_SIZE => {
                let (video_active, video_hwnd, hwnd_tab, chrome_visible) =
                    with_state(hwnd, |state| {
                        (
                            state.local_mpv_video_mode_active,
                            state.local_mpv_video_hwnd,
                            state.hwnd_tab,
                            state.local_mpv_menu_visible,
                        )
                    })
                    .unwrap_or((false, HWND(0), HWND(0), false));
                if video_active && video_hwnd.0 != 0 {
                    let mut rc = RECT::default();
                    if get_client_rect_safe(hwnd, &mut rc).is_ok() {
                        let mut video_top = 0;
                        if chrome_visible && hwnd_tab.0 != 0 {
                            let width = rc.right - rc.left;
                            let height = rc.bottom - rc.top;
                            crate::log_if_err!(MoveWindow(
                                hwnd_tab,
                                0,
                                0,
                                width,
                                height.max(0),
                                true
                            ));
                            let mut tab_rc = rc;
                            crate::send_message_w_safe(
                                hwnd_tab,
                                TCM_ADJUSTRECT,
                                WPARAM(0),
                                LPARAM(&mut tab_rc as *mut _ as isize),
                            );
                            video_top = tab_rc.top.max(0);
                        }
                        crate::log_if_err!(MoveWindow(
                            video_hwnd,
                            0,
                            video_top,
                            rc.right - rc.left,
                            (rc.bottom - rc.top - video_top).max(0),
                            true
                        ));
                    }
                } else {
                    editor_manager::layout_children(hwnd);
                }
                LRESULT(0)
            }
            WM_OPEN_AUDIO_DESCRIPTION_FOR_TV_RECORDING => {
                let path = Box::from_raw(lparam.0 as *mut PathBuf);
                if path.is_file() {
                    log_debug(&format!(
                        "Opening audio-description window after TV recording: {}",
                        path.display()
                    ));
                    app_windows::audio_description_window::open_with_input(hwnd, *path);
                } else {
                    log_debug(&format!(
                        "TV recording audio-description handoff ignored because the file is unavailable: {}",
                        path.display()
                    ));
                }
                LRESULT(0)
            }
            WM_YOUTUBE_MPV_PLAYBACK_FAILED => {
                let payload = Box::from_raw(lparam.0 as *mut YoutubeMpvPlaybackFailure);
                let is_current = with_state(hwnd, |state| {
                    state
                        .active_mpv_session
                        .as_ref()
                        .is_some_and(|session| session.process_id == payload.process_id)
                })
                .unwrap_or(false);
                if is_current {
                    let language =
                        with_state(hwnd, |state| state.settings.language).unwrap_or_default();
                    let return_context =
                        with_state(hwnd, |state| state.active_youtube_return_context.clone())
                            .unwrap_or_default();
                    log_debug(&format!(
                        "Managed mpv YouTube playback failure handled: pid={} detail={}",
                        payload.process_id, payload.detail
                    ));
                    stop_managed_mpv_playback(hwnd);
                    editor_manager::close_current_document(hwnd);
                    let message =
                        app_windows::youtube_transcript_window::youtube_mpv_playback_error_message(
                            language,
                            &payload.detail,
                        );
                    show_error(hwnd, language, &message);
                    if return_context.input.is_some() {
                        app_windows::youtube_transcript_window::reopen_stream_selection(
                            hwnd,
                            return_context,
                        );
                    }
                } else {
                    log_debug(&format!(
                        "Ignoring stale mpv YouTube failure: pid={} detail={}",
                        payload.process_id, payload.detail
                    ));
                }
                LRESULT(0)
            }
            WM_LOCAL_MPV_MENU_VISIBLE => {
                let show_menu = wparam.0 != 0;
                let (video_hwnd, hidden_menu, hwnd_tab) = with_state(hwnd, |state| {
                    (
                        state.local_mpv_video_hwnd,
                        state.local_mpv_hidden_menu,
                        state.hwnd_tab,
                    )
                })
                .unwrap_or((HWND(0), HMENU(0), HWND(0)));
                log_debug(&format!(
                    "local_mpv_menu_visible apply: show_menu={} attached_before={:?} hidden_menu={:?}",
                    show_menu,
                    GetMenu(hwnd),
                    hidden_menu
                ));
                if hidden_menu.0 != 0 {
                    crate::log_if_err!(SetMenu(
                        hwnd,
                        if show_menu { hidden_menu } else { HMENU(0) }
                    ));
                    show_window_safe(hwnd_tab, if show_menu { SW_SHOW } else { SW_HIDE });
                    crate::log_if_err!(DrawMenuBar(hwnd));
                    let mut rc = RECT::default();
                    if video_hwnd.0 != 0 && get_client_rect_safe(hwnd, &mut rc).is_ok() {
                        let mut video_top = 0;
                        if show_menu && hwnd_tab.0 != 0 {
                            let width = rc.right - rc.left;
                            let height = rc.bottom - rc.top;
                            crate::log_if_err!(MoveWindow(
                                hwnd_tab,
                                0,
                                0,
                                width,
                                height.max(0),
                                true
                            ));
                            let mut tab_rc = rc;
                            crate::send_message_w_safe(
                                hwnd_tab,
                                TCM_ADJUSTRECT,
                                WPARAM(0),
                                LPARAM(&mut tab_rc as *mut _ as isize),
                            );
                            video_top = tab_rc.top.max(0);
                        }
                        crate::log_if_err!(MoveWindow(
                            video_hwnd,
                            0,
                            video_top,
                            rc.right - rc.left,
                            (rc.bottom - rc.top - video_top).max(0),
                            true
                        ));
                    }
                }
                LRESULT(0)
            }
            WM_LOCAL_MPV_VIDEO_MODE => {
                let entering = wparam.0 != 0;
                let (hwnd_tab, hwnd_status, video_hwnd, hidden_menu) = with_state(hwnd, |state| {
                    (
                        state.hwnd_tab,
                        state.hwnd_status,
                        state.local_mpv_video_hwnd,
                        state.local_mpv_hidden_menu,
                    )
                })
                .unwrap_or((HWND(0), HWND(0), HWND(0), HMENU(0)));
                if entering {
                    with_state(hwnd, |state| {
                        state.local_mpv_alt_menu_pending = false;
                        state.local_mpv_menu_visible = false;
                    });
                    show_window_safe(hwnd_tab, SW_HIDE);
                    show_window_safe(hwnd_status, SW_HIDE);
                    show_window_safe(video_hwnd, SW_SHOW);
                    if hidden_menu.0 == 0 {
                        let current_menu = GetMenu(hwnd);
                        if current_menu.0 != 0 {
                            crate::log_if_err!(SetMenu(hwnd, HMENU(0)));
                            with_state(hwnd, |state| {
                                state.local_mpv_hidden_menu = current_menu;
                                state.local_mpv_menu_visible = false;
                            });
                        }
                    }
                    let mut rc = RECT::default();
                    if video_hwnd.0 != 0 && get_client_rect_safe(hwnd, &mut rc).is_ok() {
                        crate::log_if_err!(MoveWindow(
                            video_hwnd,
                            0,
                            0,
                            rc.right - rc.left,
                            rc.bottom - rc.top,
                            true
                        ));
                    }
                    crate::log_if_err!(DrawMenuBar(hwnd));
                } else {
                    crate::send_message_w_safe(hwnd, WM_CANCELMODE, WPARAM(0), LPARAM(0));
                    with_state(hwnd, |state| {
                        state.local_mpv_alt_menu_pending = false;
                        state.local_mpv_menu_visible = false;
                    });
                    if hidden_menu.0 != 0 {
                        crate::log_if_err!(SetMenu(hwnd, hidden_menu));
                        with_state(hwnd, |state| {
                            state.local_mpv_hidden_menu = HMENU(0);
                        });
                    }
                    show_window_safe(video_hwnd, SW_HIDE);
                    show_window_safe(hwnd_tab, SW_SHOW);
                    show_window_safe(hwnd_status, SW_SHOW);
                    editor_manager::layout_children(hwnd);
                    update_main_status_bar(hwnd);
                    crate::log_if_err!(DrawMenuBar(hwnd));
                    crate::log_if_err!(PostMessageW(hwnd, WM_FOCUS_EDITOR, WPARAM(0), LPARAM(0)));
                }
                LRESULT(0)
            }
            WM_SETFOCUS => {
                log_mpv_focus_snapshot(hwnd, "mpv_wm_setfocus.before");
                if with_state(hwnd, |state| state.local_mpv_video_mode_active).unwrap_or(false) {
                    log_mpv_focus_snapshot(hwnd, "mpv_wm_setfocus.video_mode");
                    return LRESULT(0);
                }
                if restore_transcription_progress_focus_for_current_document(hwnd) {
                    return LRESULT(0);
                }
                if app_windows::audio_description_window::restore_on_parent_activation(hwnd) {
                    return LRESULT(0);
                }
                force_active_editor_focus(hwnd);
                log_mpv_focus_snapshot(hwnd, "mpv_wm_setfocus.after");
                LRESULT(0)
            }
            WM_ACTIVATE => {
                let is_activating = (wparam.0 & 0xFFFF) != 0;
                if is_activating {
                    log_mpv_focus_snapshot(hwnd, "mpv_wm_activate.activate.before");
                } else {
                    log_mpv_focus_snapshot(hwnd, "mpv_wm_activate.deactivate");
                }
                if is_activating {
                    if reactivate_modal_child_while_main_disabled(hwnd) {
                        log_debug(
                            "WM_ACTIVATE redirected to the active modal popup because the main window is disabled",
                        );
                        return LRESULT(0);
                    }
                    if reactivate_batch_audiobooks_window(hwnd) {
                        return LRESULT(0);
                    }
                    if reactivate_bdciechi_window(hwnd) {
                        return LRESULT(0);
                    }
                    if restore_context_transcription_progress_focus(hwnd) {
                        return LRESULT(0);
                    }
                    if restore_transcription_progress_focus_for_current_document(hwnd) {
                        return LRESULT(0);
                    }
                    if app_windows::audio_description_window::restore_on_parent_activation(hwnd) {
                        log_debug(
                            "WM_ACTIVATE redirected to the audio-description window after owner reactivation",
                        );
                        return LRESULT(0);
                    }
                    if with_state(hwnd, |state| state.local_mpv_video_mode_active).unwrap_or(false)
                    {
                        log_mpv_focus_snapshot(hwnd, "mpv_wm_activate.video_mode");
                        return LRESULT(0);
                    }
                    if should_force_editor_focus_on_foreground(hwnd) {
                        force_active_editor_focus(hwnd);
                        schedule_editor_focus_retry(hwnd);
                    }
                    log_mpv_focus_snapshot(hwnd, "mpv_wm_activate.activate.after");
                }
                LRESULT(0)
            }
            WM_NOTIFY => {
                let hdr = &*(lparam.0 as *const NMHDR);
                if hdr.code == TCN_SELCHANGE && hdr.hwndFrom == editor_manager::get_tab(hwnd) {
                    // Cancel pending spellcheck highlight when switching tabs
                    kill_timer_best_effort(
                        hwnd,
                        SPELLCHECK_HIGHLIGHT_TIMER_ID,
                        "KillTimer SPELLCHECK_HIGHLIGHT",
                    );
                    with_state(hwnd, |state| {
                        state.spellcheck_highlight_pending = None;
                        state.spellcheck_last_highlighted_line = None;
                    });
                    attempt_switch_to_selected_tab(hwnd);
                    return LRESULT(0);
                }
                if hdr.code == EN_CHANGE {
                    editor_manager::mark_dirty_from_edit(hwnd, hdr.hwndFrom);
                    let active_edit = get_active_edit(hwnd).unwrap_or(HWND(0));
                    if active_edit == hdr.hwndFrom {
                        let language =
                            with_state(hwnd, |state| state.settings.language).unwrap_or_default();
                        let label = i18n::tr(language, "undo.action.text");
                        with_state(hwnd, |state| state.undo_action_label = Some(label));
                        update_main_status_bar(hwnd);
                    }
                    return LRESULT(0);
                }
                if hdr.code == EN_SELCHANGE {
                    // Only process if editor has focus to avoid focus issues during file open etc.
                    if GetFocus() == hdr.hwndFrom {
                        clear_stale_tts_automatic_bookmark_for_edit(hwnd, hdr.hwndFrom);
                        let is_large_editor = is_large_text_editor(hwnd, hdr.hwndFrom);
                        if !is_large_editor {
                            handle_spellcheck_selection_change(hwnd, hdr.hwndFrom);
                            prefetch_dictionary_for_selection(hwnd, hdr.hwndFrom);
                            trigger_spellcheck_highlight(hwnd, hdr.hwndFrom);
                        }
                    }
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_REACTIVATE_MODAL_AFTER_EXTERNAL_OPEN => {
                if !reactivate_pending_blocking_modal(hwnd) {
                    reactivate_modal_child_while_main_disabled(hwnd);
                }
                LRESULT(0)
            }
            WM_TIMER => {
                if wparam.0 == EPUB_INDEX_ANNOUNCE_TIMER_ID {
                    kill_timer_best_effort(
                        hwnd,
                        EPUB_INDEX_ANNOUNCE_TIMER_ID,
                        "KillTimer EPUB_INDEX_ANNOUNCE",
                    );
                    if editor_manager::current_document_has_epub_index(hwnd) {
                        let language =
                            with_state(hwnd, |state| state.settings.language).unwrap_or_default();
                        screen_reader_speak(&i18n::tr(language, "epub_index.announcement"));
                    }
                    return LRESULT(0);
                }
                if wparam.0 == DEFERRED_MODAL_COPYDATA_OPEN_TIMER_ID {
                    process_deferred_modal_copydata_paths_if_ready(hwnd);
                    return LRESULT(0);
                }
                if wparam.0 == STARTUP_UPDATE_CHANGELOG_TIMER_ID {
                    kill_timer_best_effort(
                        hwnd,
                        STARTUP_UPDATE_CHANGELOG_TIMER_ID,
                        "KillTimer STARTUP_UPDATE_CHANGELOG",
                    );
                    if let Err(err) =
                        PostMessageW(hwnd, WM_SHOW_UPDATE_CHANGELOG, WPARAM(0), LPARAM(0))
                    {
                        log_debug(&format!(
                            "Failed to repost startup update changelog message: {}",
                            err
                        ));
                    }
                    return LRESULT(0);
                }
                if wparam.0 == LA7_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE_TIMER_ID {
                    kill_timer_best_effort(
                        hwnd,
                        LA7_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE_TIMER_ID,
                        "KillTimer LA7_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE",
                    );
                    restore_la7_context_audio_description_focus(hwnd);
                    app_windows::la7_play_window::finish_context_audio_description_focus(hwnd);
                    return LRESULT(0);
                }
                if wparam.0 == RAIPLAY_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE_TIMER_ID {
                    kill_timer_best_effort(
                        hwnd,
                        RAIPLAY_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE_TIMER_ID,
                        "KillTimer RAIPLAY_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE",
                    );
                    restore_raiplay_context_audio_description_focus(hwnd);
                    app_windows::raiplay_window::finish_context_audio_description_focus(hwnd);
                    return LRESULT(0);
                }
                if wparam.0 == YOUTUBE_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE_TIMER_ID {
                    kill_timer_best_effort(
                        hwnd,
                        YOUTUBE_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE_TIMER_ID,
                        "KillTimer YOUTUBE_CONTEXT_AUDIO_DESCRIPTION_FOCUS_RELEASE",
                    );
                    restore_youtube_context_audio_description_focus(hwnd);
                    app_windows::youtube_transcript_window::finish_context_audio_description(hwnd);
                    return LRESULT(0);
                }
                if wparam.0 == FOCUS_EDITOR_TIMER_ID
                    || wparam.0 == FOCUS_EDITOR_TIMER_ID2
                    || wparam.0 == FOCUS_EDITOR_TIMER_ID3
                    || wparam.0 == FOCUS_EDITOR_TIMER_ID4
                {
                    log_foreground_snapshot(&format!("focus_timer.before.{}", wparam.0));
                    kill_timer_best_effort(hwnd, wparam.0, "KillTimer FOCUS_EDITOR");
                    let changelog_window =
                        with_state(hwnd, |state| state.changelog_window).unwrap_or(HWND(0));
                    if changelog_window.0 != 0 {
                        log_debug(&format!(
                            "focus timer {} suppressed because the changelog window is open",
                            wparam.0
                        ));
                        crate::set_foreground_window_safe(changelog_window);
                        log_foreground_snapshot(&format!(
                            "focus_timer.after_changelog.{}",
                            wparam.0
                        ));
                        return LRESULT(0);
                    }
                    if restore_la7_context_audio_description_focus(hwnd) {
                        log_debug(&format!(
                            "focus timer {} suppressed because La7 Play context audio description is opening",
                            wparam.0
                        ));
                        log_foreground_snapshot(&format!(
                            "focus_timer.after_la7_context_audio_description.{}",
                            wparam.0
                        ));
                        return LRESULT(0);
                    }
                    if restore_raiplay_context_audio_description_focus(hwnd) {
                        log_debug(&format!(
                            "focus timer {} suppressed because RaiPlay context audio description is opening",
                            wparam.0
                        ));
                        log_foreground_snapshot(&format!(
                            "focus_timer.after_raiplay_context_audio_description.{}",
                            wparam.0
                        ));
                        return LRESULT(0);
                    }
                    if restore_youtube_context_audio_description_focus(hwnd) {
                        log_debug(&format!(
                            "focus timer {} suppressed because YouTube context audio description is opening",
                            wparam.0
                        ));
                        log_foreground_snapshot(&format!(
                            "focus_timer.after_context_audio_description.{}",
                            wparam.0
                        ));
                        return LRESULT(0);
                    }
                    if restore_rai_audio_context_save_progress_focus(hwnd) {
                        log_debug(&format!(
                            "focus timer {} suppressed because Rai audiodescrizioni context save is active",
                            wparam.0
                        ));
                        log_foreground_snapshot(&format!(
                            "focus_timer.after_rai_audio_context_save.{}",
                            wparam.0
                        ));
                        return LRESULT(0);
                    }
                    if restore_context_transcription_progress_focus(hwnd) {
                        log_debug(&format!(
                            "focus timer {} suppressed because YouTube context transcription is active",
                            wparam.0
                        ));
                        log_foreground_snapshot(&format!(
                            "focus_timer.after_context_transcription.{}",
                            wparam.0
                        ));
                        return LRESULT(0);
                    }
                    if reactivate_modal_child_while_main_disabled(hwnd) {
                        log_debug(&format!(
                            "focus timer {} suppressed because a modal popup is active",
                            wparam.0
                        ));
                        log_foreground_snapshot(&format!(
                            "focus_timer.after_modal_popup.{}",
                            wparam.0
                        ));
                        return LRESULT(0);
                    }
                    if app_windows::audio_description_window::restore_on_parent_activation(hwnd) {
                        log_debug(&format!(
                            "focus timer {} suppressed because the audio-description window is active",
                            wparam.0
                        ));
                        return LRESULT(0);
                    }
                    if !is_mpv_playback_active(hwnd)
                        && app_windows::youtube_transcript_window::has_active_player_return_list(
                            hwnd,
                        )
                        && app_windows::youtube_transcript_window::restore_active_player_return_list(
                            hwnd,
                        )
                    {
                        log_debug(&format!(
                            "focus timer {} suppressed because a TV/recordings return list is active",
                            wparam.0
                        ));
                        log_foreground_snapshot(&format!(
                            "focus_timer.after_return_list.{}",
                            wparam.0
                        ));
                        return LRESULT(0);
                    }
                    force_active_editor_focus(hwnd);
                    log_foreground_snapshot(&format!("focus_timer.after.{}", wparam.0));
                    return LRESULT(0);
                }
                if wparam.0 == MPV_BASS_FOCUS_DEBUG_TIMER_ID1
                    || wparam.0 == MPV_BASS_FOCUS_DEBUG_TIMER_ID2
                    || wparam.0 == MPV_BASS_FOCUS_DEBUG_TIMER_ID3
                    || wparam.0 == MPV_BASS_FOCUS_DEBUG_TIMER_ID4
                {
                    log_foreground_snapshot(&format!("mpv_bass_focus_debug.{}", wparam.0));
                    kill_timer_best_effort(hwnd, wparam.0, "KillTimer MPV_BASS_FOCUS_DEBUG");
                    if wparam.0 == MPV_BASS_FOCUS_DEBUG_TIMER_ID1
                        || wparam.0 == MPV_BASS_FOCUS_DEBUG_TIMER_ID2
                    {
                        bring_window_to_foreground(hwnd);
                        if let Some(hwnd_tab) = with_state(hwnd, |state| state.hwnd_tab) {
                            set_focus_safe(hwnd_tab);
                        }
                        log_foreground_snapshot(&format!(
                            "mpv_bass_focus_debug.refocus.{}",
                            wparam.0
                        ));
                    }
                    return LRESULT(0);
                }
                if wparam.0 == MPV_ESC_FOCUS_DEBUG_TIMER_ID1
                    || wparam.0 == MPV_ESC_FOCUS_DEBUG_TIMER_ID2
                    || wparam.0 == MPV_ESC_FOCUS_DEBUG_TIMER_ID3
                    || wparam.0 == MPV_ESC_FOCUS_DEBUG_TIMER_ID4
                    || wparam.0 == MPV_ESC_FOCUS_DEBUG_TIMER_ID5
                    || wparam.0 == MPV_ESC_FOCUS_DEBUG_TIMER_ID6
                {
                    log_foreground_snapshot(&format!("mpv_esc_focus_debug.{}", wparam.0));
                    kill_timer_best_effort(hwnd, wparam.0, "KillTimer MPV_ESC_FOCUS_DEBUG");
                    return LRESULT(0);
                }
                if wparam.0 == ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID1
                    || wparam.0 == ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID2
                    || wparam.0 == ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID3
                    || wparam.0 == ITALIAONLINE_CLOSE_FOCUS_DEBUG_TIMER_ID4
                {
                    log_foreground_snapshot(&format!(
                        "italiaonline_close_focus_debug.{}",
                        wparam.0
                    ));
                    kill_timer_best_effort(
                        hwnd,
                        wparam.0,
                        "KillTimer ITALIAONLINE_CLOSE_FOCUS_DEBUG",
                    );
                    return LRESULT(0);
                }
                if wparam.0 == CHAPTER_ANNOUNCE_TIMER_ID {
                    update_chapter_announcement(hwnd);
                    return LRESULT(0);
                }
                if wparam.0 == EDITOR_TRANSLATION_SPEAK_TIMER_ID {
                    kill_timer_best_effort(
                        hwnd,
                        EDITOR_TRANSLATION_SPEAK_TIMER_ID,
                        "KillTimer EDITOR_TRANSLATION_SPEAK",
                    );
                    speak_pending_editor_translation_message(hwnd);
                    return LRESULT(0);
                }
                if wparam.0 == SPELLCHECK_HIGHLIGHT_TIMER_ID {
                    kill_timer_best_effort(
                        hwnd,
                        SPELLCHECK_HIGHLIGHT_TIMER_ID,
                        "KillTimer SPELLCHECK_HIGHLIGHT",
                    );
                    handle_spellcheck_highlight_timer(hwnd);
                    return LRESULT(0);
                }
                if wparam.0 == AUDIO_PLAYLIST_TIMER_ID {
                    handle_audio_playlist_timer(hwnd);
                    return LRESULT(0);
                }
                if app_windows::calendar_window::handle_reminder_timer(hwnd, wparam.0) {
                    return LRESULT(0);
                }
                handle_pdf_loading_timer(hwnd, wparam.0);
                LRESULT(0)
            }
            WM_PODCAST_CHAPTERS_READY => {
                let ptr = lparam.0 as *mut PodcastChaptersReady;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let msg = Box::from_raw(ptr);
                let (apply_now, chapters, language, announce_unavailable, current_pos_ms) =
                    with_state(hwnd, |state| {
                        let incoming = msg.chapters.clone();
                        let chapters = if incoming.is_none() {
                            state
                                .podcast_chapters_cache
                                .get(&msg.key)
                                .and_then(|cached| cached.clone())
                        } else {
                            incoming
                        };
                        state
                            .podcast_chapters_cache
                            .insert(msg.key.clone(), chapters.clone());
                        let apply_now = state
                            .active_podcast_chapters_key
                            .as_deref()
                            .map(|k| k == msg.key.as_str())
                            .unwrap_or(false);
                        if apply_now {
                            state.last_announced_chapter_index = None;
                            if let Some(list) = chapters.clone() {
                                state.active_podcast_chapters = list.clone();
                                return (
                                    true,
                                    list,
                                    state.settings.language,
                                    false,
                                    audiobook_position_ms_from_state(state),
                                );
                            }
                            state.active_podcast_chapters.clear();
                            let announce_unavailable = !msg.key.starts_with("file_chapters:");
                            return (
                                true,
                                Vec::new(),
                                state.settings.language,
                                announce_unavailable,
                                audiobook_position_ms_from_state(state),
                            );
                        }
                        (
                            false,
                            Vec::new(),
                            state.settings.language,
                            false,
                            audiobook_position_ms_from_state(state),
                        )
                    })
                    .unwrap_or((
                        false,
                        Vec::new(),
                        Language::default(),
                        false,
                        None,
                    ));
                if apply_now {
                    if !chapters.is_empty() {
                        if SetTimer(hwnd, CHAPTER_ANNOUNCE_TIMER_ID, 500, None) == 0 {
                            crate::log_debug("Failed to set CHAPTER_ANNOUNCE_TIMER");
                        }
                        announce_current_chapter_on_start(
                            hwnd,
                            &chapters,
                            current_pos_ms,
                            language,
                        );
                    } else {
                        kill_timer_best_effort(
                            hwnd,
                            CHAPTER_ANNOUNCE_TIMER_ID,
                            "KillTimer CHAPTER_ANNOUNCE",
                        );
                    }
                    if announce_unavailable {
                        let message = i18n::tr(language, "playback.chapters_unavailable");
                        nvda_speak(&message);
                    }
                    crate::menu::update_playback_menu(hwnd, true);
                }
                LRESULT(0)
            }
            WM_PODCAST_EPISODE_SAVE_RESULT => {
                let ptr = lparam.0 as *mut PodcastEpisodeSaveResult;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(ptr);
                close_podcast_save_progress_window(hwnd);
                app_windows::rai_audiodescrizioni_window::finish_context_save(hwnd);
                if let Some(err) = payload.error {
                    app_windows::raiplay_window::finish_context_audio_description_download(
                        hwnd, false,
                    );
                    app_windows::la7_play_window::finish_context_audio_description_download(
                        hwnd, false,
                    );
                    if err == "Saving canceled." {
                        screen_reader_speak(&i18n::tr(payload.language, "podcast.save.canceled"));
                        app_windows::youtube_transcript_window::restore_active_media_context_list(
                            hwnd,
                        );
                        return LRESULT(0);
                    }
                    let body = i18n::tr_f(
                        payload.language,
                        "podcasts.save_error_body",
                        &[("err", &err)],
                    );
                    let title = i18n::tr(payload.language, "podcasts.save_error_title");
                    let body_w = to_wide(&body);
                    let title_w = to_wide(&title);
                    message_box_modal(
                        hwnd,
                        PCWSTR(body_w.as_ptr()),
                        PCWSTR(title_w.as_ptr()),
                        MB_OK | MB_ICONERROR,
                    );
                    app_windows::youtube_transcript_window::restore_active_media_context_list(hwnd);
                    return LRESULT(0);
                }

                if payload.open_audio_description {
                    let raiplay_context =
                        app_windows::raiplay_window::finish_context_audio_description_download(
                            hwnd, true,
                        );
                    let la7_context =
                        app_windows::la7_play_window::finish_context_audio_description_download(
                            hwnd, true,
                        );
                    if raiplay_context || la7_context {
                        app_windows::youtube_transcript_window::dismiss_active_media_context_lists_for_editor(
                            hwnd,
                        );
                        if la7_context {
                            queue_la7_context_audio_description(hwnd, payload.target_path);
                        } else {
                            queue_raiplay_context_audio_description(hwnd, payload.target_path);
                        }
                    } else {
                        app_windows::audio_description_window::open_with_input(
                            hwnd,
                            payload.target_path,
                        );
                    }
                    return LRESULT(0);
                }

                let show_confirmation =
                    with_state(hwnd, |state| state.settings.show_media_save_confirmation)
                        .unwrap_or(true);
                if !show_confirmation {
                    app_windows::youtube_transcript_window::restore_active_media_context_list(hwnd);
                    return LRESULT(0);
                }

                let path_text = payload.target_path.to_string_lossy().to_string();
                let saved_line = i18n::tr_f(
                    payload.language,
                    "podcasts.save_confirm_body",
                    &[("path", &path_text)],
                );
                let open_label = i18n::tr(payload.language, "podcasts.save_confirm_open_folder");
                let body = format!("{saved_line}\n\n{open_label}?");
                let title = i18n::tr(payload.language, "podcasts.save_confirm_title");
                let body_w = to_wide(&body);
                let title_w = to_wide(&title);
                let response = message_box_modal(
                    hwnd,
                    PCWSTR(body_w.as_ptr()),
                    PCWSTR(title_w.as_ptr()),
                    MB_YESNO | MB_ICONINFORMATION,
                );
                let mut opened_folder = false;
                if response == IDYES {
                    let folder = payload
                        .target_path
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| payload.target_path.clone());
                    let folder_wide = to_wide(&folder.to_string_lossy());
                    let open_res = ShellExecuteW(
                        hwnd,
                        w!("open"),
                        PCWSTR(folder_wide.as_ptr()),
                        PCWSTR::null(),
                        PCWSTR::null(),
                        SW_SHOWNORMAL,
                    );
                    if open_res.0 as isize <= 32 {
                        log_debug(&format!(
                            "podcast_episode_save_open_folder_failed path={} code={}",
                            folder.to_string_lossy(),
                            open_res.0
                        ));
                    } else {
                        opened_folder = true;
                    }
                }
                if !opened_folder {
                    app_windows::youtube_transcript_window::restore_active_media_context_list(hwnd);
                }
                LRESULT(0)
            }
            WM_EDITOR_TRANSLATION_DONE => {
                let ptr = lparam.0 as *mut EditorTranslationResult;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let payload = box_from_raw_safe(ptr);
                apply_editor_translation_result(hwnd, *payload);
                LRESULT(0)
            }
            WM_EDITOR_SUMMARY_DONE => {
                let ptr = lparam.0 as *mut EditorSummaryResult;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let payload = box_from_raw_safe(ptr);
                apply_editor_summary_result(hwnd, *payload);
                LRESULT(0)
            }
            WM_PODCAST_EPISODE_PLAY_READY => {
                let ptr = lparam.0 as *mut PodcastEpisodePlayReady;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(ptr);
                close_podcast_play_progress_window(hwnd);
                editor_manager::open_document(hwnd, &payload.cache_path);
                if payload.prefer_title_for_document
                    && let Some(title) = podcast_episode_display_title(
                        payload.podcast_title.as_deref(),
                        payload.title.as_deref(),
                    )
                {
                    editor_manager::set_current_document_title(hwnd, &title);
                }
                if with_state(hwnd, |state| {
                    state.active_podcast_episode_from_rai = payload.rai_origin;
                })
                .is_none()
                {
                    log_debug("Failed to set active podcast Rai origin flag");
                }
                editor_manager::mark_current_document_from_rss(hwnd, true);
                set_active_podcast_episode_info(
                    hwnd,
                    Some(payload.url),
                    None,
                    payload.podcast_title,
                    payload.title,
                    Some(payload.cache_path),
                );
                menu::update_playback_menu(hwnd, true);
                activate_pending_podcast_chapters(hwnd);
                LRESULT(0)
            }
            WM_PODCAST_EPISODE_PLAY_FAILED => {
                let ptr = lparam.0 as *mut PodcastEpisodePlayFailed;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(ptr);
                close_podcast_play_progress_window(hwnd);
                show_error(hwnd, payload.language, &payload.error);
                LRESULT(0)
            }
            WM_START_LA7_CONTEXT_AUDIO_DESCRIPTION => {
                let ptr = lparam.0 as *mut DeferredLa7ContextAudioDescription;
                if ptr.is_null() {
                    app_windows::la7_play_window::finish_context_audio_description_focus(hwnd);
                    return LRESULT(0);
                }
                let DeferredLa7ContextAudioDescription { input_path } = *Box::from_raw(ptr);
                enable_window_safe(hwnd, true);
                app_windows::audio_description_window::open_with_input(hwnd, input_path);
                protect_la7_context_audio_description_focus(hwnd);
                LRESULT(0)
            }
            WM_START_RAIPLAY_CONTEXT_AUDIO_DESCRIPTION => {
                let ptr = lparam.0 as *mut DeferredRaiplayContextAudioDescription;
                if ptr.is_null() {
                    app_windows::raiplay_window::finish_context_audio_description_focus(hwnd);
                    return LRESULT(0);
                }
                let DeferredRaiplayContextAudioDescription { input_path } = *Box::from_raw(ptr);
                enable_window_safe(hwnd, true);
                app_windows::audio_description_window::open_with_input(hwnd, input_path);
                protect_raiplay_context_audio_description_focus(hwnd);
                LRESULT(0)
            }
            WM_START_YOUTUBE_CONTEXT_AUDIO_DESCRIPTION => {
                let ptr = lparam.0 as *mut DeferredYoutubeContextAudioDescription;
                if ptr.is_null() {
                    app_windows::youtube_transcript_window::finish_context_audio_description(hwnd);
                    return LRESULT(0);
                }
                let DeferredYoutubeContextAudioDescription { input_path } = *Box::from_raw(ptr);
                enable_window_safe(hwnd, true);
                app_windows::audio_description_window::open_with_input(hwnd, input_path);
                protect_youtube_context_audio_description_focus(hwnd);
                LRESULT(0)
            }
            WM_START_CONTEXT_WHISPER_TRANSCRIPTION => {
                let ptr = lparam.0 as *mut DeferredWhisperTranscription;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let DeferredWhisperTranscription {
                    media_path,
                    media_title,
                    close_raiplay_on_success,
                    close_la7_on_success,
                } = *Box::from_raw(ptr);
                start_whisper_transcription_for_media(hwnd, media_path, media_title);
                let started =
                    with_state(hwnd, |state| state.transcription_in_progress).unwrap_or(false);
                if close_raiplay_on_success && started {
                    app_windows::raiplay_window::mark_context_transcription_started(hwnd);
                }
                if close_la7_on_success && started {
                    app_windows::la7_play_window::mark_context_transcription_started(hwnd);
                }
                if !started {
                    app_windows::rai_audiodescrizioni_window::finish_context_transcription(hwnd);
                    app_windows::youtube_transcript_window::finish_context_transcription(hwnd);
                }
                LRESULT(0)
            }
            WM_WHISPER_TRANSCRIPTION_DONE => {
                let ptr = lparam.0 as *mut WhisperTranscriptionResult;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(ptr);
                apply_whisper_transcription_result(hwnd, *payload);
                LRESULT(0)
            }
            WM_WHISPER_TRANSCRIPTION_PROGRESS => {
                update_whisper_progress_window(hwnd, wparam.0.min(100));
                LRESULT(0)
            }
            WM_WHISPER_TRANSCRIPTION_STATUS_TEXT => {
                let ptr = lparam.0 as *mut String;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let payload = box_from_raw_safe(ptr);
                update_whisper_progress_status(hwnd, &payload);
                LRESULT(0)
            }
            WM_DICTATION_DONE => {
                let ptr = lparam.0 as *mut DictationResult;
                if ptr.is_null() {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(ptr);
                apply_dictation_result(hwnd, *payload);
                LRESULT(0)
            }
            WM_DICTIONARY_LOADED => {
                if lparam.0 == 0 {
                    return LRESULT(0);
                }
                let result = Box::from_raw(lparam.0 as *mut DictionaryLookupResult);
                let updated = with_state(hwnd, |state| {
                    let current_gen = state.dictionary_prefetch_generation;
                    if result.generation != current_gen {
                        return false;
                    }
                    if result.cacheable {
                        state
                            .dictionary_cache
                            .insert(result.key.clone(), result.lines.clone());
                    } else {
                        state.dictionary_cache.remove(&result.key);
                    }
                    state.dictionary_pending_lookup = None;
                    save_dictionary_cache(&state.dictionary_cache);

                    if state.dictionary_context_menu.0 != 0 && !state.dictionary_context_loaded {
                        let key = dictionary_cache_key(
                            state.dictionary_context_language,
                            &state.dictionary_context_pref,
                            &state.dictionary_context_word,
                        );
                        if key == result.key {
                            let hmenu = state.dictionary_context_menu;
                            let count = GetMenuItemCount(hmenu);
                            if count > 0 {
                                for _ in 0..count {
                                    crate::log_if_err!(DeleteMenu(hmenu, 0, MF_BYPOSITION));
                                }
                            }
                            for line in &result.lines {
                                let display = format!(" {}", line);
                                crate::log_if_err!(AppendMenuW(
                                    hmenu,
                                    MF_STRING | MF_GRAYED,
                                    0,
                                    PCWSTR(to_wide(&display).as_ptr()),
                                ));
                            }
                            state.dictionary_context_loaded = true;
                            return true;
                        }
                    }
                    false
                })
                .unwrap_or(false);
                if updated {
                    let (hmenu, expanded) = with_state(hwnd, |state| {
                        (
                            state.dictionary_context_menu,
                            state.dictionary_context_expanded,
                        )
                    })
                    .unwrap_or((HMENU(0), false));
                    if hmenu.0 != 0 && expanded {
                        crate::log_if_err!(DrawMenuBar(hwnd));
                        use windows::Win32::UI::Input::KeyboardAndMouse::{
                            KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_LEFT, VK_RIGHT, keybd_event,
                        };
                        keybd_event(VK_LEFT.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
                        keybd_event(VK_LEFT.0 as u8, 0, KEYEVENTF_KEYUP, 0);
                        keybd_event(VK_RIGHT.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
                        keybd_event(VK_RIGHT.0 as u8, 0, KEYEVENTF_KEYUP, 0);
                    }
                }
                LRESULT(0)
            }
            WM_UPDATE_DIALOG => {
                if lparam.0 == 0 {
                    return LRESULT(0);
                }
                crate::log_debug("UI: Received WM_UPDATE_DIALOG");
                let req = Box::from_raw(lparam.0 as *mut UpdateDialogRequest);
                let text = to_wide(&req.text);
                let title = to_wide(&req.title);
                set_blocking_modal_active(hwnd, Some(BlockingModalKind::UpdateDialog));
                let result = MessageBoxW(
                    hwnd,
                    PCWSTR(text.as_ptr()),
                    PCWSTR(title.as_ptr()),
                    req.flags,
                );
                set_blocking_modal_active(hwnd, None);
                let pending_paths = take_deferred_copydata_paths_for_blocking_modal(hwnd);
                if !pending_paths.is_empty() {
                    open_copydata_paths(hwnd, pending_paths);
                } else {
                    restore_editor_focus(hwnd);
                }
                crate::log_debug(&format!("UI: Update dialog result: {:?}", result));
                if let Err(e) = req.response_tx.send(result.0) {
                    crate::log_debug(&format!("UI: Failed to send response to channel: {}", e));
                } else {
                    crate::log_debug("UI: Response sent to channel successfully");
                }
                LRESULT(0)
            }
            WM_UPDATE_PROGRESS_OPEN => {
                if lparam.0 == 0 {
                    return LRESULT(0);
                }
                let req = Box::from_raw(lparam.0 as *mut UpdateProgressOpenRequest);
                let labels = app_windows::podcast_save_window::SaveDialogLabels {
                    title: i18n::tr(req.language, "updater.title"),
                    in_progress: i18n::tr(req.language, "podcast.save.in_progress"),
                    cancel: i18n::tr(req.language, "podcast.save.cancel"),
                    cancel_confirm: i18n::tr(req.language, "podcast.cancel_confirm"),
                };
                let dialog = app_windows::podcast_save_window::open_with_labels(
                    hwnd,
                    req.language,
                    labels,
                    false,
                );
                with_state(hwnd, |state| {
                    state.update_progress_window = dialog;
                });
                if let Err(e) = req.response_tx.send(dialog.0) {
                    crate::log_debug(&format!(
                        "UI: Failed to send update progress dialog handle: {}",
                        e
                    ));
                }
                LRESULT(0)
            }
            WM_UPDATE_PROGRESS_SET => {
                let pct = wparam.0.min(100);
                let dialog =
                    with_state(hwnd, |state| state.update_progress_window).unwrap_or(HWND(0));
                if dialog.0 != 0 {
                    crate::log_if_err!(PostMessageW(
                        dialog,
                        app_windows::podcast_save_window::WM_PODCAST_SAVE_PROGRESS,
                        WPARAM(pct),
                        LPARAM(0)
                    ));
                }
                LRESULT(0)
            }
            WM_UPDATE_PROGRESS_CLOSE => {
                let dialog =
                    with_state(hwnd, |state| state.update_progress_window).unwrap_or(HWND(0));
                if dialog.0 != 0 {
                    crate::log_if_err!(PostMessageW(
                        dialog,
                        app_windows::podcast_save_window::WM_PODCAST_SAVE_DONE,
                        WPARAM(0),
                        LPARAM(0)
                    ));
                    with_state(hwnd, |state| {
                        state.update_progress_window = HWND(0);
                    });
                }
                LRESULT(0)
            }
            app_windows::podcast_save_window::WM_PODCAST_SAVE_CLOSED => {
                let closed_hwnd = HWND(lparam.0);
                with_state(hwnd, |state| {
                    if state.podcast_save_window == closed_hwnd {
                        state.podcast_save_window = HWND(0);
                    }
                    if state.update_progress_window == closed_hwnd {
                        state.update_progress_window = HWND(0);
                    }
                    if state.transcription_progress_window == closed_hwnd {
                        state.transcription_progress_window = HWND(0);
                    }
                    if state.replace_progress_window == closed_hwnd {
                        state.replace_progress_window = HWND(0);
                        state.replace_cancel_requested = false;
                        state.replace_cancel_token = None;
                    }
                    if state.editor_translation_progress_window == closed_hwnd {
                        state.editor_translation_progress_window = HWND(0);
                        state.editor_translation_cancel_token = None;
                        state.editor_progress_canceling_message.clear();
                    }
                });
                LRESULT(0)
            }
            app_windows::podcast_save_window::WM_PODCAST_SAVE_CANCEL => {
                let source_hwnd = HWND(lparam.0);
                let is_podcast_save = with_state(hwnd, |state| {
                    source_hwnd.0 != 0 && state.podcast_save_window == source_hwnd
                })
                .unwrap_or(false);
                let is_replace = with_state(hwnd, |state| {
                    source_hwnd.0 != 0 && state.replace_progress_window == source_hwnd
                })
                .unwrap_or(false);
                let is_whisper = with_state(hwnd, |state| {
                    state.transcription_in_progress
                        && source_hwnd.0 != 0
                        && state.transcription_progress_window == source_hwnd
                })
                .unwrap_or(false);
                let is_editor_translation = with_state(hwnd, |state| {
                    source_hwnd.0 != 0 && state.editor_translation_progress_window == source_hwnd
                })
                .unwrap_or(false);
                if is_replace {
                    with_state(hwnd, |state| {
                        state.replace_cancel_requested = true;
                        if let Some(token) = state.replace_cancel_token.as_ref() {
                            token.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                    });
                }
                if is_whisper {
                    cancel_whisper_transcription(hwnd);
                }
                if is_podcast_save {
                    with_state(hwnd, |state| {
                        if let Some(token) = state.podcast_save_cancel_token.as_ref() {
                            token.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                    });
                }
                if is_editor_translation {
                    with_state(hwnd, |state| {
                        if let Some(token) = state.editor_translation_cancel_token.as_ref() {
                            token.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                        if state.editor_translation_progress_window.0 != 0 {
                            let canceling_message = state.editor_progress_canceling_message.clone();
                            app_windows::podcast_save_window::set_status_text(
                                state.editor_translation_progress_window,
                                &canceling_message,
                            );
                        }
                    });
                }
                LRESULT(0)
            }
            search::WM_REPLACE_ALL_PROGRESS => {
                search::handle_replace_all_progress(hwnd, wparam);
                LRESULT(0)
            }
            WM_AUTO_UPDATE_CHECK => {
                if !has_secondary_window_open(hwnd) {
                    updater::check_for_update(hwnd, false);
                }
                LRESULT(0)
            }
            WM_CHECK_PENDING_UPDATE => {
                if !has_secondary_window_open(hwnd) {
                    updater::check_pending_update(hwnd, false);
                }
                LRESULT(0)
            }
            WM_SHOW_CHANGELOG => {
                if !has_secondary_window_open(hwnd) {
                    app_windows::help_window::open_changelog(hwnd);
                }
                LRESULT(0)
            }
            WM_SHOW_UPDATE_CHANGELOG => {
                if has_secondary_window_open(hwnd) {
                    if SetTimer(
                        hwnd,
                        STARTUP_UPDATE_CHANGELOG_TIMER_ID,
                        STARTUP_UPDATE_CHANGELOG_RETRY_INTERVAL_MS,
                        None,
                    ) == 0
                    {
                        log_debug(
                            "Failed to schedule the update changelog while another window is open",
                        );
                    } else {
                        log_debug(
                            "Update changelog deferred until the current secondary window closes",
                        );
                    }
                    return LRESULT(0);
                }
                kill_timer_best_effort(
                    hwnd,
                    STARTUP_UPDATE_CHANGELOG_TIMER_ID,
                    "KillTimer STARTUP_UPDATE_CHANGELOG",
                );
                log_debug("Opening the combined update completion and changelog window");
                bring_window_to_foreground(hwnd);
                app_windows::help_window::open_update_changelog(hwnd);
                LRESULT(0)
            }
            search::WM_REPLACE_ALL_DONE => {
                search::handle_replace_all_done(hwnd, lparam);
                LRESULT(0)
            }
            WM_PDF_LOADED => {
                if lparam.0 == 0 {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(lparam.0 as *mut PdfLoadResult);
                handle_pdf_loaded(hwnd, *payload);
                LRESULT(0)
            }
            WM_DOCUMENT_LOADED => {
                if lparam.0 == 0 {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(lparam.0 as *mut editor_manager::DocumentLoadResult);
                handle_document_loaded(hwnd, *payload);
                LRESULT(0)
            }
            WM_TTS_VOICES_LOADED => {
                if lparam.0 == 0 {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(lparam.0 as *mut Vec<VoiceInfo>);
                let voices: Vec<VoiceInfo> = *payload;
                with_state(hwnd, |state| {
                    state.edge_voices = voices.clone();
                });
                if let Some(dialog) = with_state(hwnd, |state| state.options_dialog)
                    && dialog.0 != 0
                {
                    app_windows::options_window::refresh_voices(dialog);
                }
                refresh_voice_panel(hwnd);
                LRESULT(0)
            }
            WM_TTS_SAPI_VOICES_LOADED => {
                if lparam.0 == 0 {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(lparam.0 as *mut Vec<VoiceInfo>);
                let voices: Vec<VoiceInfo> = *payload;
                with_state(hwnd, |state| {
                    state.sapi_voices = voices.clone();
                });
                if let Some(dialog) = with_state(hwnd, |state| state.options_dialog)
                    && dialog.0 != 0
                {
                    app_windows::options_window::refresh_voices(dialog);
                }
                refresh_voice_panel(hwnd);
                LRESULT(0)
            }
            WM_TTS_START => {
                if lparam.0 == 0 {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(lparam.0 as *mut tts_engine::TtsPlaybackOptions);
                tts_engine::start_tts_playback_with_chunks(*payload);
                LRESULT(0)
            }

            WM_TTS_PLAYBACK_DONE => {
                let session_id = wparam.0 as u64;
                with_state(hwnd, |state| {
                    if let Some(current) = &state.tts_session
                        && current.id == session_id
                    {
                        state.tts_session = None;
                        state.tts_last_offset = 0;
                        state.tts_pending_start_pos = None;
                        prevent_sleep(false);
                    }
                });
                LRESULT(0)
            }
            WM_TTS_CHUNK_START => {
                let session_id = wparam.0 as u64;
                let offset = lparam.0 as i32;
                with_state(hwnd, |state| {
                    let Some((initial_caret_pos, source_edit)) = state
                        .tts_session
                        .as_ref()
                        .filter(|current| current.id == session_id)
                        .map(|current| (current.initial_caret_pos, current.source_edit))
                    else {
                        return;
                    };

                    let safe_offset = clamp_tts_chunk_offset(state.tts_last_offset, offset);
                    if safe_offset != offset {
                        log_debug(&format!(
                            "TTS: normalized non-monotonic offset session={} prev={} new={} safe={}",
                            session_id, state.tts_last_offset, offset, safe_offset
                        ));
                    }
                    state.tts_last_offset = safe_offset;
                    state.tts_pending_start_pos = None;

                    if source_edit.0 == 0 || !is_window_handle_valid(source_edit) {
                        return;
                    }

                    let current_pos = (initial_caret_pos + safe_offset).max(0);
                    if let Some(doc) = state.docs.iter().find(|doc| doc.hwnd_edit == source_edit) {
                        let title = doc.title.clone();
                        let path = doc.path.clone();
                        let format = doc.format;
                        let (storage_key, _) = runtime_bookmark_storage_key(
                            path.as_deref(),
                            source_edit,
                            &title,
                            format,
                        );
                        state.tts_automatic_bookmark_position =
                            Some((source_edit, storage_key, current_pos));
                    }
                    state.tts_sentence_nav_anchor = Some((source_edit, current_pos));

                    if state.settings.move_cursor_during_reading {
                        let mut cr = CHARRANGE {
                            cpMin: current_pos,
                            cpMax: current_pos,
                        };
                        SendMessageW(
                            source_edit,
                            EM_EXSETSEL,
                            WPARAM(0),
                            LPARAM(&mut cr as *mut _ as isize),
                        );
                        SendMessageW(source_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
                        notify_editor_caret_changed(source_edit);
                    }
                });
                LRESULT(0)
            }
            WM_TTS_PLAYBACK_ERROR => {
                if lparam.0 == 0 {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(lparam.0 as *mut String);
                let message: String = *payload;
                let session_id = wparam.0 as u64;
                let mut should_show = false;
                with_state(hwnd, |state| {
                    if let Some(current) = &state.tts_session
                        && current.id == session_id
                    {
                        state.tts_session = None;
                        state.tts_last_offset = 0;
                        state.tts_pending_start_pos = None;
                        prevent_sleep(false);
                        should_show = true;
                    }
                });
                if should_show {
                    let language =
                        with_state(hwnd, |state| state.settings.language).unwrap_or_default();
                    show_error(hwnd, language, &message);
                } else {
                    log_debug(&format!(
                        "TTS error ignored for session {session_id}: {message}"
                    ));
                }
                LRESULT(0)
            }
            WM_TTS_AUDIOBOOK_DONE => {
                if lparam.0 == 0 {
                    return LRESULT(0);
                }

                with_state(hwnd, |state| {
                    if state.audiobook_progress.0 != 0 {
                        crate::log_if_err!(DestroyWindow(state.audiobook_progress));
                        state.audiobook_progress = HWND(0);
                        state.audiobook_cancel = None;
                    }
                    if let Some(doc) = state.docs.get(state.current) {
                        set_focus_safe(doc.hwnd_edit);
                    }
                });

                let payload = Box::from_raw(lparam.0 as *mut AudiobookResult);
                let language =
                    with_state(hwnd, |state| state.settings.language).unwrap_or_default();
                let title = if payload.success {
                    audiobook_done_title(language)
                } else {
                    error_title(language)
                };
                let title = to_wide(&title);
                let message = to_wide(&payload.message);
                let flags = if payload.success {
                    MB_OK | MB_ICONINFORMATION
                } else {
                    MB_OK | MB_ICONERROR
                };
                // Pattern for future blocking completion dialogs:
                // set the modal kind before MessageBoxW, clear it right after,
                // then drain any deferred external file opens before restoring editor focus.
                set_blocking_modal_active(hwnd, Some(BlockingModalKind::AudiobookDone));
                MessageBoxW(
                    hwnd,
                    PCWSTR(message.as_ptr()),
                    PCWSTR(title.as_ptr()),
                    flags,
                );
                set_blocking_modal_active(hwnd, None);
                let pending_paths = take_deferred_copydata_paths_for_blocking_modal(hwnd);
                if !pending_paths.is_empty() {
                    open_copydata_paths(hwnd, pending_paths);
                } else {
                    restore_editor_focus(hwnd);
                }
                LRESULT(0)
            }
            WM_FOCUS_EDITOR => {
                if !is_mpv_playback_active(hwnd)
                    && app_windows::youtube_transcript_window::has_active_player_return_list(hwnd)
                    && app_windows::youtube_transcript_window::restore_active_player_return_list(
                        hwnd,
                    )
                {
                    log_debug(
                        "WM_FOCUS_EDITOR suppressed because a TV/recordings return list is active",
                    );
                    return LRESULT(0);
                }
                log_debug(&format!(
                    "WM_FOCUS_EDITOR received: hwnd={:?} foreground_before={:?} focus_before={:?} should_force={} has_secondary={}",
                    hwnd,
                    get_foreground_window_safe(),
                    get_focus_safe(),
                    should_force_editor_focus_on_foreground(hwnd),
                    has_secondary_window_open(hwnd)
                ));
                log_foreground_snapshot("wm_focus_editor.before");
                if restore_context_transcription_progress_focus(hwnd) {
                    log_foreground_snapshot("wm_focus_editor.after_context_transcription_restore");
                    return LRESULT(0);
                }
                if restore_transcription_progress_focus_for_current_document(hwnd) {
                    log_foreground_snapshot("wm_focus_editor.after_transcription_restore");
                    return LRESULT(0);
                }
                if should_force_editor_focus_on_foreground(hwnd) {
                    force_active_editor_focus(hwnd);
                    schedule_editor_focus_retry(hwnd);
                } else if !has_secondary_window_open(hwnd) {
                    focus_editor(hwnd);
                }
                log_foreground_snapshot("wm_focus_editor.after");
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wparam.0 as u32 == u32::from(VK_F9.0) {
                    cycle_favorite_voice(hwnd, -1);
                    return LRESULT(0);
                }
                if wparam.0 as u32 == u32::from(VK_F10.0) {
                    cycle_favorite_voice(hwnd, 1);
                    return LRESULT(0);
                }
                if wparam.0 as u32 == u32::from(VK_TAB.0)
                    && (GetKeyState(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0
                {
                    next_tab_with_prompt(hwnd);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SYSCOMMAND => {
                let command = (wparam.0 & 0xFFF0) as u32;
                if command == SC_KEYMENU
                    && with_state(hwnd, |state| state.local_mpv_video_mode_active).unwrap_or(false)
                {
                    log_debug(&format!(
                        "WM_SYSCOMMAND SC_KEYMENU intercepted: attached_menu_before={:?} hidden_menu={:?}",
                        GetMenu(hwnd),
                        with_state(hwnd, |state| state.local_mpv_hidden_menu).unwrap_or(HMENU(0))
                    ));
                    set_local_mpv_video_menu_visible(hwnd, true);
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_INITMENUPOPUP => {
                let hmenu = HMENU(wparam.0 as isize);
                let main_menu = GetMenu(hwnd);
                log_debug(&format!(
                    "WM_INITMENUPOPUP: popup={:?} main_menu={:?} focus={:?} video_mode={} menu_visible={} hidden_menu={:?}",
                    hmenu,
                    main_menu,
                    crate::get_focus_safe(),
                    with_state(hwnd, |state| state.local_mpv_video_mode_active).unwrap_or(false),
                    with_state(hwnd, |state| state.local_mpv_menu_visible).unwrap_or(false),
                    with_state(hwnd, |state| state.local_mpv_hidden_menu).unwrap_or(HMENU(0))
                ));
                if main_menu.0 != 0 {
                    update_voice_panel_menu_check(hwnd);
                    let file_menu = GetSubMenu(main_menu, 0);
                    if file_menu == hmenu {
                        update_route_image_menu_item(hwnd, hmenu);
                    }
                    let view_menu = GetSubMenu(main_menu, 2);
                    if view_menu == hmenu {
                        update_epub_index_menu_item(hwnd, hmenu);
                    }
                    let edit_menu = GetSubMenu(main_menu, 1);
                    if edit_menu == hmenu {
                        let can_undo = can_undo_now(hwnd);
                        let flags = if can_undo {
                            MF_BYCOMMAND | MF_ENABLED
                        } else {
                            MF_BYCOMMAND | MF_GRAYED
                        };
                        let _enabled = EnableMenuItem(hmenu, IDM_EDIT_UNDO as u32, flags);
                        let language =
                            with_state(hwnd, |state| state.settings.language).unwrap_or_default();
                        let undo_label = build_undo_menu_label(hwnd, language);
                        let menu_flags = if can_undo {
                            MF_BYCOMMAND | MF_STRING | MF_ENABLED
                        } else {
                            MF_BYCOMMAND | MF_STRING | MF_GRAYED
                        };
                        let _modified = ModifyMenuW(
                            hmenu,
                            IDM_EDIT_UNDO as u32,
                            menu_flags,
                            IDM_EDIT_UNDO,
                            PCWSTR(to_wide(&undo_label).as_ptr()),
                        );
                        let can_cut_copy = has_active_text_selection(hwnd);
                        let cut_copy_flags = if can_cut_copy {
                            MF_BYCOMMAND | MF_ENABLED
                        } else {
                            MF_BYCOMMAND | MF_GRAYED
                        };
                        let _enabled = EnableMenuItem(hmenu, IDM_EDIT_CUT as u32, cut_copy_flags);
                        let _enabled = EnableMenuItem(hmenu, IDM_EDIT_COPY as u32, cut_copy_flags);
                        let paste_flags = if can_paste_now(hwnd) {
                            MF_BYCOMMAND | MF_ENABLED
                        } else {
                            MF_BYCOMMAND | MF_GRAYED
                        };
                        let _enabled = EnableMenuItem(hmenu, IDM_EDIT_PASTE as u32, paste_flags);
                    }
                    let window_menu = GetSubMenu(main_menu, WINDOW_MENU_INDEX);
                    if window_menu == hmenu {
                        refresh_window_open_documents_menu(hwnd, hmenu);
                    }
                }
                let ctx = with_state(hwnd, |state| {
                    if state.dictionary_context_menu != hmenu || state.dictionary_context_loaded {
                        return None;
                    }
                    state.dictionary_context_expanded = true;
                    let key = dictionary_cache_key(
                        state.dictionary_context_language,
                        &state.dictionary_context_pref,
                        &state.dictionary_context_word,
                    );
                    let not_found =
                        i18n::tr(state.dictionary_context_language, "dictionary.not_found");
                    let cached = match state.dictionary_cache.get(&key) {
                        Some(lines) if lines.len() == 1 && lines[0] == not_found => {
                            state.dictionary_cache.remove(&key);
                            save_dictionary_cache(&state.dictionary_cache);
                            None
                        }
                        Some(lines) => Some(lines.clone()),
                        None => None,
                    };
                    let pending = state.dictionary_pending_lookup.as_ref() == Some(&key);
                    Some((
                        state.dictionary_context_word.clone(),
                        state.dictionary_context_language,
                        state.dictionary_context_pref.clone(),
                        key,
                        cached,
                        pending,
                        state.dictionary_prefetch_generation,
                    ))
                })
                .flatten();
                let Some((word, language, pref, key, cached, pending, generation)) = ctx else {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                };

                let count = GetMenuItemCount(hmenu);
                if count > 0 {
                    for _ in 0..count {
                        crate::log_if_err!(DeleteMenu(hmenu, 0, MF_BYPOSITION));
                    }
                }

                match cached {
                    Some(lines) => {
                        for line in lines {
                            let display = line.replace('&', "");
                            crate::log_if_err!(AppendMenuW(
                                hmenu,
                                MF_STRING | MF_GRAYED,
                                0,
                                PCWSTR(to_wide(&display).as_ptr()),
                            ));
                        }
                        with_state(hwnd, |state| {
                            state.dictionary_context_loaded = true;
                        });
                    }
                    None => {
                        let loading_msg = i18n::tr(language, "dictionary.loading");
                        crate::log_if_err!(AppendMenuW(
                            hmenu,
                            MF_STRING | MF_GRAYED,
                            0,
                            PCWSTR(to_wide(&loading_msg).as_ptr()),
                        ));
                        if !pending {
                            with_state(hwnd, |state| {
                                state.dictionary_pending_lookup = Some(key.clone());
                            });
                            start_dictionary_lookup(hwnd.0, word, language, pref, key, generation);
                        }
                    }
                }
                LRESULT(0)
            }
            WM_CONTEXTMENU => {
                let target = HWND(wparam.0 as isize);
                let (combo_voice, combo_favorites) = with_state(hwnd, |state| {
                    (state.voice_combo_voice, state.voice_combo_favorites)
                })
                .unwrap_or((HWND(0), HWND(0)));
                if (target == combo_voice && combo_voice.0 != 0)
                    || (target == combo_favorites && combo_favorites.0 != 0)
                {
                    show_voice_context_menu(hwnd, target, lparam);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_COMMAND => {
                let cmd_id = wparam.0 & 0xffff;
                let notification = (wparam.0 >> 16) as u16;
                if cmd_id == IDM_TOOLS_RADIO
                    || cmd_id == IDM_TOOLS_PATHS_NAVIGATION
                    || cmd_id == IDM_FILE_PRINT
                {
                    log_debug(&format!(
                        "shortcut probe: WM_COMMAND cmd={} notification={} lparam={:?} focus={:?}",
                        cmd_id,
                        notification,
                        lparam,
                        crate::get_focus_safe()
                    ));
                }
                if u32::from(notification) == EN_KILLFOCUS {
                    log_mpv_focus_snapshot(hwnd, &format!("mpv_en_killfocus.cmd_{}", cmd_id));
                }
                if u32::from(notification) == EN_CHANGE {
                    if is_voice_panel_tuning_edit(hwnd, HWND(lparam.0)) {
                        return LRESULT(0);
                    }
                    editor_manager::handle_normalize_edit_change(hwnd, HWND(lparam.0));
                    mark_dirty_from_edit(hwnd, HWND(lparam.0));
                    let active_edit = get_active_edit(hwnd).unwrap_or(HWND(0));
                    if active_edit == HWND(lparam.0) {
                        let language =
                            with_state(hwnd, |state| state.settings.language).unwrap_or_default();
                        let label = i18n::tr(language, "undo.action.text");
                        with_state(hwnd, |state| state.undo_action_label = Some(label));
                        update_main_status_bar(hwnd);
                    }
                    return LRESULT(0);
                }
                if cmd_id == VOICE_PANEL_ID_ENGINE && u32::from(notification) == CBN_SELCHANGE {
                    handle_voice_panel_engine_change(hwnd);
                    return LRESULT(0);
                }
                if cmd_id == VOICE_PANEL_ID_LANGUAGE && u32::from(notification) == CBN_SELCHANGE {
                    refresh_voice_panel_voice_list(hwnd);
                    return LRESULT(0);
                }
                if cmd_id == VOICE_PANEL_ID_VOICE && u32::from(notification) == CBN_SELCHANGE {
                    handle_voice_panel_voice_change(hwnd);
                    return LRESULT(0);
                }
                if cmd_id == VOICE_PANEL_ID_FAVORITES && u32::from(notification) == CBN_SELCHANGE {
                    handle_voice_panel_favorite_change(hwnd);
                    return LRESULT(0);
                }
                if cmd_id == VOICE_PANEL_ID_MULTILINGUAL {
                    handle_voice_panel_multilingual_toggle(hwnd);
                    return LRESULT(0);
                }
                if cmd_id == VOICE_PANEL_ID_INSERT_TAG {
                    insert_voice_tag_from_voice_panel(hwnd);
                    return LRESULT(0);
                }
                if cmd_id == VOICE_PANEL_ID_MANAGE_GOOGLE_VOICES {
                    handle_voice_panel_manage_google_voices(hwnd);
                    return LRESULT(0);
                }
                if cmd_id == VOICE_PANEL_ID_INSERT_PAUSE {
                    show_pause_tag_menu_from_voice_panel(hwnd);
                    return LRESULT(0);
                }
                if (cmd_id == VOICE_PANEL_ID_SPEED
                    || cmd_id == VOICE_PANEL_ID_PITCH
                    || cmd_id == VOICE_PANEL_ID_VOLUME)
                    && u32::from(notification) == CBN_SELCHANGE
                {
                    handle_voice_panel_tuning_combo_change(hwnd);
                    return LRESULT(0);
                }
                if (cmd_id == VOICE_PANEL_ID_SPEED_EDIT
                    || cmd_id == VOICE_PANEL_ID_PITCH_EDIT
                    || cmd_id == VOICE_PANEL_ID_VOLUME_EDIT)
                    && u32::from(notification) == EN_KILLFOCUS
                {
                    handle_voice_panel_tuning_edit_change(hwnd);
                    return LRESULT(0);
                }
                if cmd_id == VOICE_MENU_ID_ADD_FAVORITE as usize {
                    handle_voice_context_favorite(hwnd, true);
                    return LRESULT(0);
                }
                if cmd_id == VOICE_MENU_ID_REMOVE_FAVORITE as usize {
                    handle_voice_context_favorite(hwnd, false);
                    return LRESULT(0);
                }
                if (IDM_SPELLCHECK_SUGGESTION_BASE
                    ..IDM_SPELLCHECK_SUGGESTION_BASE + IDM_SPELLCHECK_SUGGESTION_MAX)
                    .contains(&cmd_id)
                {
                    let index = cmd_id - IDM_SPELLCHECK_SUGGESTION_BASE;
                    handle_spellcheck_suggestion(hwnd, index);
                    return LRESULT(0);
                }
                if cmd_id == IDM_SPELLCHECK_ADD_TO_DICTIONARY {
                    handle_spellcheck_add_to_dictionary(hwnd);
                    return LRESULT(0);
                }
                if cmd_id == IDM_SPELLCHECK_IGNORE_ONCE {
                    handle_spellcheck_ignore_once(hwnd);
                    return LRESULT(0);
                }

                if (IDM_FILE_RECENT_BASE..IDM_FILE_RECENT_BASE + MAX_RECENT).contains(&cmd_id) {
                    let index = cmd_id - IDM_FILE_RECENT_BASE;
                    if let Some(path) =
                        with_state(hwnd, |state| state.recent_files.get(index).cloned()).flatten()
                    {
                        editor_manager::open_document(hwnd, &path);
                    }
                    return LRESULT(0);
                }
                if cmd_id == IDM_FILE_RECENT_CLEAR {
                    clear_recent_files(hwnd);
                    return LRESULT(0);
                }

                match cmd_id {
                    IDM_FILE_NEW => {
                        log_debug("Menu: New document");
                        editor_manager::new_document(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_OPEN => {
                        if with_state(hwnd, |_| {}).is_none() {
                            log_debug("Menu: Open document ignored (not initialized)");
                            return LRESULT(0);
                        }
                        if reactivate_pending_blocking_modal(hwnd) {
                            log_debug(
                                "Menu: Open document deferred while blocking modal dialog is pending",
                            );
                            return LRESULT(0);
                        }
                        log_debug("Menu: Open document");
                        // Cancel spellcheck highlight to avoid focus issues
                        kill_timer_best_effort(
                            hwnd,
                            SPELLCHECK_HIGHLIGHT_TIMER_ID,
                            "KillTimer SPELLCHECK_HIGHLIGHT",
                        );
                        with_state(hwnd, |state| {
                            state.spellcheck_highlight_pending = None;
                            state.spellcheck_last_highlighted_line = None;
                        });
                        if let Some(selected) = open_file_dialog_with_encoding(hwnd) {
                            if selected.iter().all(|(path, _)| is_audio_path(path)) {
                                let audio_paths = selected
                                    .into_iter()
                                    .map(|(path, _)| path)
                                    .collect::<Vec<_>>();
                                queue_audio_files_and_play(hwnd, audio_paths);
                            } else {
                                for (path, encoding) in selected {
                                    open_document_with_encoding(hwnd, &path, encoding);
                                }
                            }
                            if with_state(hwnd, |state| state.prompt_window.0 != 0).unwrap_or(false)
                            {
                                focus_editor(hwnd);
                            }
                        }
                        LRESULT(0)
                    }
                    IDM_FILE_SAVE => {
                        log_debug("Menu: Save document");
                        editor_manager::save_current_document(hwnd);
                        editor_manager::refresh_current_editor_visual(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_SAVE_AS => {
                        log_debug("Menu: Save document as");
                        editor_manager::save_current_document_as(hwnd);
                        editor_manager::refresh_current_editor_visual(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_SAVE_ALL => {
                        log_debug("Menu: Save all documents");
                        editor_manager::save_all_documents(hwnd);
                        editor_manager::refresh_current_editor_visual(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_PRINT => {
                        log_debug("Menu: Print document");
                        print_current_document(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_SAVE_IMAGE => {
                        log_debug("Menu: Save route image");
                        app_windows::route_map_export::export_current_route_map_image(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_CLOSE => {
                        log_debug("Menu: Close document");
                        editor_manager::close_current_document(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_CLOSE_OTHERS => {
                        log_debug("Menu: Close other files");
                        if editor_manager::close_other_documents(hwnd) {
                            close_other_windows(hwnd);
                        }
                        LRESULT(0)
                    }
                    IDM_FILE_EXIT => {
                        log_debug("Menu: Exit");
                        editor_manager::try_close_app(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_READ_START => {
                        log_debug("Menu: Start reading");
                        let mut should_restart_tts = false;
                        with_state(hwnd, |state| {
                            state.tts_pending_start_pos = None;
                            if let Some(doc) = state.docs.get(state.current)
                                && !matches!(doc.format, FileFormat::Audiobook)
                            {
                                should_restart_tts = true;
                            }
                        });
                        if should_restart_tts {
                            tts_engine::stop_tts_playback(hwnd);
                            tts_engine::start_tts_from_caret(hwnd);
                        } else {
                            tts_engine::start_tts_from_caret(hwnd);
                        }
                        LRESULT(0)
                    }
                    IDM_FILE_READ_PREVIOUS_SENTENCE => {
                        log_debug("Menu: Read previous sentence");
                        jump_tts_sentence(hwnd, SentenceNavigationDirection::Previous);
                        LRESULT(0)
                    }
                    IDM_FILE_READ_NEXT_SENTENCE => {
                        log_debug("Menu: Read next sentence");
                        jump_tts_sentence(hwnd, SentenceNavigationDirection::Next);
                        LRESULT(0)
                    }
                    IDM_FILE_EXECUTE => {
                        log_debug("Menu: Execute file");
                        execute_current_file(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_READ_PAUSE => {
                        log_debug("Menu: Pause/resume reading");
                        tts_engine::toggle_tts_pause(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_READ_STOP => {
                        log_debug("Menu: Stop reading");
                        tts_engine::stop_tts_playback(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_AUDIOBOOK => {
                        log_debug("Menu: Record audiobook");
                        telemetry::record_action("audiobook_record", "document");
                        tts_engine::start_audiobook(hwnd);
                        LRESULT(0)
                    }
                    IDM_EDIT_AUDIOBOOK_SELECTION => {
                        log_debug("Menu: Record audiobook from selection");
                        telemetry::record_action("audiobook_record", "selection");
                        tts_engine::start_audiobook_from_selection(hwnd);
                        LRESULT(0)
                    }
                    cmd if editor_translate_language_for_menu_id(cmd).is_some() => {
                        log_debug("Menu: Translate selection");
                        handle_editor_translate_menu_command(hwnd, cmd);
                        LRESULT(0)
                    }
                    menu::IDM_EDIT_SUMMARIZE_TEXT => {
                        log_debug("Menu: Summarize text");
                        handle_editor_summarize_menu_command(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_BATCH_AUDIOBOOK => {
                        log_debug("Menu: Batch audiobooks");
                        app_windows::batch_audiobooks_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_PODCAST => {
                        log_debug("Menu: Record podcast");
                        app_windows::podcast_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_TOGGLE_DICTATION => {
                        log_debug("Tools: Toggle dictation");
                        toggle_voice_dictation(hwnd);
                        LRESULT(0)
                    }
                    IDM_FILE_CONVERT_AUDIO => {
                        log_debug("Menu: Convert audio");
                        app_windows::convert_audio_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_EDIT_UNDO => {
                        log_debug("Menu: Undo");
                        if !editor_manager::try_normalize_undo(hwnd) {
                            editor_manager::undo_active_edit_skip_navigation(hwnd);
                        }
                        with_state(hwnd, |state| state.undo_action_label = None);
                        LRESULT(0)
                    }
                    IDM_EDIT_CUT => {
                        log_debug("Menu: Cut");
                        editor_manager::send_to_active_edit(hwnd, WM_CUT);
                        LRESULT(0)
                    }
                    IDM_EDIT_COPY => {
                        log_debug("Menu: Copy");
                        editor_manager::send_to_active_edit(hwnd, WM_COPY);
                        LRESULT(0)
                    }
                    IDM_EDIT_PASTE => {
                        log_debug("Menu: Paste");
                        editor_manager::send_to_active_edit(hwnd, WM_PASTE);
                        LRESULT(0)
                    }
                    IDM_EDIT_SELECT_ALL => {
                        log_debug("Menu: Select All");
                        editor_manager::select_all_active_edit(hwnd);
                        // announce_menu_action_screen_reader(hwnd, "edit.select_all");
                        LRESULT(0)
                    }
                    IDM_EDIT_FIND => {
                        log_debug("Menu: Find");
                        search::open_find_dialog(hwnd);
                        LRESULT(0)
                    }
                    IDM_EDIT_FIND_IN_FILES => {
                        log_debug("Menu: Find in files");
                        app_windows::find_in_files_window::open_find_in_files_dialog(hwnd);
                        LRESULT(0)
                    }
                    IDM_EDIT_FIND_NEXT => {
                        log_debug("Menu: Find next");
                        search::find_next_from_state(hwnd);
                        LRESULT(0)
                    }
                    IDM_EDIT_FIND_PREVIOUS => {
                        log_debug("Menu: Find previous");
                        search::find_previous_from_state(hwnd);
                        LRESULT(0)
                    }
                    IDM_EDIT_REPLACE => {
                        log_debug("Menu: Replace");
                        search::open_replace_dialog(hwnd);
                        LRESULT(0)
                    }
                    IDM_EDIT_GO_TO_LINE => {
                        log_debug("Menu: Go to Line");
                        if let Some(hwnd_edit) = get_active_edit(hwnd) {
                            let language = with_state(hwnd, |state| state.settings.language)
                                .unwrap_or_default();
                            let title = i18n::tr(language, "goto_line.prompt_title");
                            let body = i18n::tr(language, "goto_line.prompt_body");
                            let current_line = (SendMessageW(
                                hwnd_edit,
                                EM_LINEFROMCHAR,
                                WPARAM(usize::MAX),
                                LPARAM(0),
                            )
                            .0 as usize)
                                + 1;
                            let current_line_str = current_line.to_string();
                            if let Some(res) = app_windows::prompt_window::prompt_user(
                                hwnd,
                                &title,
                                &body,
                                &current_line_str,
                                language,
                            ) && let Ok(line) = res.trim().parse::<usize>()
                                && line > 0
                            {
                                let line_idx = line - 1;
                                let char_idx = SendMessageW(
                                    hwnd_edit,
                                    EM_LINEINDEX,
                                    WPARAM(line_idx),
                                    LPARAM(0),
                                )
                                .0;
                                if char_idx != -1 {
                                    SendMessageW(
                                        hwnd_edit,
                                        EM_SETSEL,
                                        WPARAM(char_idx as usize),
                                        LPARAM(char_idx as isize),
                                    );
                                    SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
                                }
                            }
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_PREV_SPELLING_ERROR => {
                        log_debug("Menu: Previous spelling error");
                        go_to_spelling_error(hwnd, false);
                        LRESULT(0)
                    }
                    IDM_EDIT_NEXT_SPELLING_ERROR => {
                        log_debug("Menu: Next spelling error");
                        go_to_spelling_error(hwnd, true);
                        LRESULT(0)
                    }
                    IDM_EDIT_STRIP_MARKDOWN => {
                        log_debug("Menu: Strip Markdown");
                        if editor_manager::strip_markdown_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.strip_markdown");
                            if let Some(hwnd_edit) = get_active_edit(hwnd) {
                                set_focus_safe(hwnd_edit);
                            }
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_AUTO_FORMAT_TTS => {
                        log_debug("Menu: Auto format TTS");
                        if editor_manager::auto_format_tts_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.auto_format_tts");
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_NORMALIZE_WHITESPACE => {
                        log_debug("Menu: Normalize whitespace");
                        if editor_manager::normalize_whitespace_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.normalize_whitespace");
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_HARD_LINE_BREAK => {
                        log_debug("Menu: Hard line break");
                        if editor_manager::hard_line_break_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.hard_line_break");
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_ORDER_ITEMS => {
                        log_debug("Menu: Order items");
                        if editor_manager::order_items_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.order_items");
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_KEEP_UNIQUE_ITEMS => {
                        log_debug("Menu: Keep unique items");
                        if editor_manager::keep_unique_items_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.keep_unique_items");
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_REVERSE_ITEMS => {
                        log_debug("Menu: Reverse items");
                        if editor_manager::reverse_items_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.reverse_items");
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_QUOTE_LINES => {
                        log_debug("Menu: Quote lines");
                        if editor_manager::quote_lines_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.quote_lines");
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_UNQUOTE_LINES => {
                        log_debug("Menu: Unquote lines");
                        if editor_manager::unquote_lines_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.unquote_lines");
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_INDENT => {
                        log_debug("Menu: Indent");
                        editor_manager::indent_active_edit(hwnd, false);
                        LRESULT(0)
                    }
                    IDM_EDIT_OUTDENT => {
                        log_debug("Menu: Outdent");
                        editor_manager::indent_active_edit(hwnd, true);
                        LRESULT(0)
                    }
                    IDM_EDIT_INSERT_ELLIPSIS => {
                        log_debug("Menu: Insert ellipsis");
                        if !editor_manager::is_current_audiobook(hwnd)
                            && let Some(hwnd_edit) = get_active_edit(hwnd)
                        {
                            let text = to_wide("…");
                            SendMessageW(
                                hwnd_edit,
                                EM_REPLACESEL,
                                WPARAM(1),
                                LPARAM(text.as_ptr() as isize),
                            );
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_TEXT_STATS => {
                        log_debug("Menu: Text stats");
                        editor_manager::text_stats_active_edit(hwnd);
                        LRESULT(0)
                    }
                    IDM_EDIT_JOIN_LINES => {
                        log_debug("Menu: Join lines");
                        if editor_manager::join_lines_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.join_lines");
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_CLEAN_EOL_HYPHENS => {
                        log_debug("Menu: Clean EOL hyphens");
                        if editor_manager::clean_end_of_line_hyphens_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.clean_eol_hyphens");
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_REMOVE_DUPLICATE_LINES => {
                        log_debug("Menu: Remove duplicate lines");
                        if editor_manager::remove_duplicate_lines_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.remove_duplicate_lines");
                        }
                        LRESULT(0)
                    }
                    IDM_EDIT_REMOVE_DUPLICATE_CONSECUTIVE_LINES => {
                        log_debug("Menu: Remove duplicate consecutive lines");
                        if editor_manager::remove_duplicate_consecutive_lines_active_edit(hwnd) {
                            confirm_menu_action(hwnd, "edit.remove_duplicate_consecutive_lines");
                        }
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_PLAY_PAUSE => {
                        handle_player_command(hwnd, PlayerCommand::TogglePause);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_STOP => {
                        handle_player_command(hwnd, PlayerCommand::Stop);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_SEEK_FORWARD => {
                        let skip_seconds =
                            with_state(hwnd, |state| state.settings.audiobook_skip_seconds)
                                .unwrap_or(0);
                        handle_player_command(hwnd, PlayerCommand::Seek(skip_seconds as i64));
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_SEEK_BACKWARD => {
                        let skip_seconds =
                            with_state(hwnd, |state| state.settings.audiobook_skip_seconds)
                                .unwrap_or(0);
                        handle_player_command(hwnd, PlayerCommand::Seek(-(skip_seconds as i64)));
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_SEEK_TO_START => {
                        handle_player_command(hwnd, PlayerCommand::SeekToStart);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_SEEK_TO_END => {
                        handle_player_command(hwnd, PlayerCommand::SeekToEnd);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_TRACK_PREV => {
                        if is_local_mpv_playback_active(hwnd)
                            && switch_audio_playlist_relative(hwnd, -1)
                        {
                            LRESULT(0)
                        } else {
                            handle_player_command(hwnd, PlayerCommand::TrackPrev);
                            LRESULT(0)
                        }
                    }
                    IDM_PLAYBACK_TRACK_NEXT => {
                        if is_local_mpv_playback_active(hwnd)
                            && switch_audio_playlist_relative(hwnd, 1)
                        {
                            LRESULT(0)
                        } else {
                            handle_player_command(hwnd, PlayerCommand::TrackNext);
                            LRESULT(0)
                        }
                    }
                    IDM_PLAYBACK_CHAPTER_PREV => {
                        handle_chapter_navigation(hwnd, -1);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_CHAPTER_NEXT => {
                        handle_chapter_navigation(hwnd, 1);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_CHAPTER_LIST => {
                        handle_chapter_list(hwnd);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_DOWNLOAD_EPISODE => {
                        if !is_live_tv_playback_active(hwnd) {
                            download_active_podcast_episode(hwnd);
                        }
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_CREATE_AUDIO_DESCRIPTION => {
                        open_audio_description_from_current_context(hwnd);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_TRANSCRIBE_CURRENT => {
                        start_whisper_transcription(hwnd);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_TRANSCRIBE_CURRENT_FOLDER => {
                        start_whisper_folder_transcription(hwnd);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_TRANSCRIBE_CANCEL => {
                        cancel_whisper_transcription(hwnd);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_GO_TO_TIME => {
                        handle_player_command(hwnd, PlayerCommand::GoToTime);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_SPLIT_PARTS => {
                        app_windows::media_split_window::open(
                            hwnd,
                            app_windows::media_split_window::MediaSplitMode::Parts,
                        );
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_SPLIT_TIME => {
                        app_windows::media_split_window::open(
                            hwnd,
                            app_windows::media_split_window::MediaSplitMode::Time,
                        );
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_ANNOUNCE_TIME => {
                        handle_player_command(hwnd, PlayerCommand::AnnounceTime);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_ADD_SUBTITLES => {
                        if let Some(path) = open_subtitle_file_dialog(hwnd) {
                            let language = with_state(hwnd, |state| state.settings.language)
                                .unwrap_or_default();
                            let result = audio_player::set_audiobook_subtitle_override(hwnd, &path)
                                .and_then(|()| {
                                    if is_mpv_playback_active(hwnd) {
                                        set_local_mpv_subtitle_override(hwnd, &path)?;
                                    }
                                    Ok(())
                                });
                            match result {
                                Ok(()) => {}
                                Err(code) => {
                                    let key = match code.as_str() {
                                        "invalid_subtitle" => "playback.add_subtitles_invalid",
                                        "no_media" => "playback.add_subtitles_no_media",
                                        _ => "playback.add_subtitles_state",
                                    };
                                    let message = i18n::tr(language, key);
                                    show_error(hwnd, language, &message);
                                }
                            }
                        }
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_REMOVE_SUBTITLES => {
                        let language =
                            with_state(hwnd, |state| state.settings.language).unwrap_or_default();
                        let result = audio_player::clear_audiobook_subtitle_override(hwnd)
                            .and_then(|()| {
                                if is_mpv_playback_active(hwnd) {
                                    clear_local_mpv_subtitle_override(hwnd)?;
                                }
                                Ok(())
                            });
                        match result {
                            Ok(()) => {}
                            Err(code) => {
                                let key = match code.as_str() {
                                    "no_media" => "playback.remove_subtitles_no_media",
                                    _ => "playback.remove_subtitles_state",
                                };
                                let message = i18n::tr(language, key);
                                show_error(hwnd, language, &message);
                            }
                        }
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_VOLUME_UP => {
                        handle_player_command(hwnd, PlayerCommand::Volume(0.1));
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_VOLUME_DOWN => {
                        handle_player_command(hwnd, PlayerCommand::Volume(-0.1));
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_VOLUME_RESET => {
                        handle_player_command(hwnd, PlayerCommand::VolumeReset);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_SPEED_UP => {
                        handle_player_command(hwnd, PlayerCommand::Speed(0.1));
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_SPEED_DOWN => {
                        handle_player_command(hwnd, PlayerCommand::Speed(-0.1));
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_SPEED_RESET => {
                        handle_player_command(hwnd, PlayerCommand::SpeedReset);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_PITCH_UP => {
                        handle_player_command(hwnd, PlayerCommand::Pitch(1.0));
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_PITCH_DOWN => {
                        handle_player_command(hwnd, PlayerCommand::Pitch(-1.0));
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_PITCH_RESET => {
                        handle_player_command(hwnd, PlayerCommand::PitchReset);
                        LRESULT(0)
                    }
                    IDM_PLAYBACK_MUTE_TOGGLE => {
                        handle_player_command(hwnd, PlayerCommand::MuteToggle);
                        LRESULT(0)
                    }
                    cmd_id
                        if (IDM_PLAYBACK_AUDIO_TRACK_BASE
                            ..IDM_PLAYBACK_AUDIO_TRACK_BASE + IDM_PLAYBACK_AUDIO_TRACK_MAX)
                            .contains(&cmd_id) =>
                    {
                        let track_menu_index = cmd_id - IDM_PLAYBACK_AUDIO_TRACK_BASE;
                        let live_variant = with_state(hwnd, |state| {
                            state
                                .raiplay_live_audio_variants
                                .get(track_menu_index)
                                .cloned()
                        })
                        .flatten();
                        if let Some(variant) = live_variant {
                            audio_player::switch_to_live_stream_url(
                                hwnd,
                                variant.url,
                                variant.track.index,
                            );
                        } else if let Some(idx) = with_state(hwnd, |state| {
                            state
                                .available_audio_tracks
                                .get(track_menu_index)
                                .map(|t| t.index)
                        })
                        .flatten()
                        {
                            audio_player::switch_audio_track(hwnd, idx);
                        }
                        LRESULT(0)
                    }
                    IDM_VIEW_SHOW_VOICES => {
                        log_debug("Menu: Toggle voice panel");
                        toggle_voice_panel(hwnd);
                        LRESULT(0)
                    }
                    IDM_VIEW_SHOW_FAVORITES => {
                        log_debug("Menu: Toggle favorite voices panel");
                        toggle_favorites_panel(hwnd);
                        LRESULT(0)
                    }
                    IDM_VIEW_SHOW_EPUB_INDEX => {
                        if editor_manager::current_daisy_document_path(hwnd).is_some() {
                            daisy_player::open_index_for_current_document(hwnd);
                            return LRESULT(0);
                        }
                        let language =
                            with_state(hwnd, |state| state.settings.language).unwrap_or_default();
                        let index = editor_manager::current_epub_index(hwnd);
                        if index.is_empty() {
                            show_info(hwnd, language, &i18n::tr(language, "epub_index.no_index"));
                            return LRESULT(0);
                        }
                        if let Some(target_utf16) =
                            app_windows::epub_index_window::select_epub_index_entry(
                                hwnd, &index, language,
                            )
                        {
                            editor_manager::go_to_current_document_utf16_offset(hwnd, target_utf16);
                        }
                        LRESULT(0)
                    }
                    IDM_VIEW_READ_ONLY => {
                        let new_read_only = with_state(hwnd, |state| {
                            state.settings.editor_read_only = !state.settings.editor_read_only;
                            state.settings.editor_read_only
                        })
                        .unwrap_or(false);
                        editor_manager::apply_read_only_to_all_edits(hwnd, new_read_only);
                        if let Some(settings) = with_state(hwnd, |state| state.settings.clone()) {
                            save_settings(settings);
                        }
                        update_voice_panel_menu_check(hwnd);
                        LRESULT(0)
                    }
                    IDM_VIEW_WORD_WRAP => {
                        let new_word_wrap = with_state(hwnd, |state| {
                            state.settings.word_wrap = !state.settings.word_wrap;
                            state.settings.word_wrap
                        })
                        .unwrap_or(true);
                        editor_manager::apply_word_wrap_to_all_edits(hwnd, new_word_wrap);
                        if let Some(settings) = with_state(hwnd, |state| state.settings.clone()) {
                            save_settings(settings);
                        }
                        update_voice_panel_menu_check(hwnd);
                        LRESULT(0)
                    }
                    IDM_VIEW_DARK_MODE => {
                        let (dark_mode, text_color, text_size) = with_state(hwnd, |state| {
                            state.settings.dark_mode = !state.settings.dark_mode;
                            (
                                state.settings.dark_mode,
                                state.settings.text_color,
                                state.settings.text_size,
                            )
                        })
                        .unwrap_or((false, 0x000000, 12));
                        if let Some(settings) = with_state(hwnd, |state| state.settings.clone()) {
                            save_settings(settings);
                        }
                        theme::set_dark_mode(dark_mode);
                        editor_manager::apply_text_appearance_to_all_edits(
                            hwnd, text_color, text_size,
                        );
                        update_voice_panel_menu_check(hwnd);
                        focus_editor(hwnd);
                        LRESULT(0)
                    }
                    IDM_VIEW_SHOW_VIDEO_DURING_PLAYBACK => {
                        let show_video = with_state(hwnd, |state| {
                            state.settings.show_video_during_playback =
                                !state.settings.show_video_during_playback;
                            state.settings.show_video_during_playback
                        })
                        .unwrap_or(true);
                        if !show_video
                            && with_state(hwnd, |state| state.local_mpv_video_mode_active)
                                .unwrap_or(false)
                        {
                            if let Err(err) = try_send_command_to_managed_mpv(
                                hwnd,
                                r#"{"command":["set_property","vid","no"]}"#,
                            ) {
                                log_debug(&format!("Local mpv video disable failed: {}", err));
                            }
                            set_local_mpv_video_mode(hwnd, false);
                        }
                        if let Some(settings) = with_state(hwnd, |state| state.settings.clone()) {
                            save_settings(settings);
                        }
                        update_voice_panel_menu_check(hwnd);
                        LRESULT(0)
                    }
                    cmd_id if font_face_from_menu_id(cmd_id).is_some() => {
                        let face = font_face_from_menu_id(cmd_id).unwrap_or("Segoe UI");
                        apply_ui_font(hwnd, face.to_string());
                        LRESULT(0)
                    }
                    cmd_id if text_color_from_menu_id(cmd_id).is_some() => {
                        let color = text_color_from_menu_id(cmd_id);
                        update_text_preferences(hwnd, color, None);
                        LRESULT(0)
                    }
                    cmd_id if text_size_from_menu_id(cmd_id).is_some() => {
                        let size = text_size_from_menu_id(cmd_id);
                        update_text_preferences(hwnd, None, size);
                        LRESULT(0)
                    }
                    IDM_INSERT_BOOKMARK => {
                        log_debug("Menu: Insert Bookmark");
                        insert_bookmark(hwnd);
                        LRESULT(0)
                    }
                    IDM_AUTOMATIC_BOOKMARK => {
                        with_state(hwnd, |state| {
                            state.settings.automatic_bookmark = !state.settings.automatic_bookmark;
                        });
                        if let Some(settings) = with_state(hwnd, |state| state.settings.clone()) {
                            save_settings(settings);
                        }
                        update_voice_panel_menu_check(hwnd);
                        LRESULT(0)
                    }
                    IDM_GOTO_NEXT_BOOKMARK => {
                        log_debug("Menu: Go to next bookmark");
                        goto_relative_bookmark(hwnd, true);
                        LRESULT(0)
                    }
                    IDM_GOTO_PREV_BOOKMARK => {
                        log_debug("Menu: Go to previous bookmark");
                        goto_relative_bookmark(hwnd, false);
                        LRESULT(0)
                    }
                    IDM_INSERT_CLEAR_BOOKMARKS => {
                        log_debug("Menu: Clear Current Bookmarks");
                        if clear_current_bookmarks(hwnd) {
                            confirm_menu_action(hwnd, "insert.clear_bookmarks");
                        }
                        LRESULT(0)
                    }
                    IDM_MANAGE_BOOKMARKS => {
                        log_debug("Menu: Manage Bookmarks");
                        app_windows::bookmarks_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_NEXT_TAB => {
                        next_tab_with_prompt(hwnd);
                        LRESULT(0)
                    }
                    IDM_WINDOW_OPEN_DOCUMENTS => {
                        open_documents_popup(hwnd);
                        LRESULT(0)
                    }
                    cmd_id if window_doc_menu_index_from_command(cmd_id).is_some() => {
                        if let Some(index) = window_doc_menu_index_from_command(cmd_id) {
                            select_tab(hwnd, index);
                        }
                        LRESULT(0)
                    }
                    IDM_WINDOW_CLOSE_ALL => {
                        log_debug("Menu: Close all documents");
                        editor_manager::close_all_documents(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_OPTIONS => {
                        log_debug("Menu: Options");
                        app_windows::options_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_DICTIONARY => {
                        log_debug("Menu: Dictionary");
                        app_windows::dictionary_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_DICTIONARY_LOOKUP => {
                        log_debug("Menu: Dictionary lookup");
                        open_dictionary_lookup(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_WIKIPEDIA_IMPORT => {
                        log_debug("Menu: Wikipedia import");
                        app_windows::wikipedia_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_GUTENBERG => {
                        log_debug("Menu: Project Gutenberg");
                        app_windows::gutenberg_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_INTERNET_ARCHIVE => {
                        log_debug("Menu: Internet Archive");
                        app_windows::internet_archive_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_LIBRIVOX => {
                        log_debug("Menu: LibriVox");
                        app_windows::librivox_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_TRECCANI => {
                        log_debug("Menu: Enciclopedia Treccani");
                        app_windows::treccani_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_IMPORT_YOUTUBE => {
                        log_debug("Menu: Import YouTube transcript");
                        app_windows::youtube_transcript_window::import_youtube_transcript(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_STREAM_AUDIO => {
                        log_debug("Menu: Stream audio from URL");
                        app_windows::youtube_transcript_window::play_streaming_audio_from_url(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_CREATE_AUDIO_DESCRIPTION => {
                        log_debug("Menu: Create audio description");
                        open_audio_description_from_current_context(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_RADIO => {
                        log_debug("Menu: Radio");
                        app_windows::radio_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_PROMPT => {
                        log_debug("Menu: Prompt");
                        app_windows::prompt_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_RSS => {
                        log_debug("Menu: RSS");
                        app_windows::rss_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_PODCASTS => {
                        log_debug("Menu: Podcasts");
                        app_windows::podcasts_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_BDCIECHI => {
                        log_debug("Menu: bdCiechi");
                        app_windows::bdciechi_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_RAI_AUDIODESCRIZIONI => {
                        log_debug("Menu: Rai audiodescrizioni");
                        app_windows::rai_audiodescrizioni_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_RAIPLAYSOUND => {
                        log_debug("Menu: RaiPlay Sound");
                        app_windows::raiplaysound_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_RAIPLAY => {
                        log_debug("Menu: RaiPlay");
                        app_windows::raiplay_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_LA7_PLAY => {
                        log_debug("Menu: La Sette Play");
                        app_windows::la7_play_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_TV => {
                        log_debug("Menu: TV");
                        app_windows::tv_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_ITALIAONLINE => {
                        log_debug("Menu: Italiaonline directories");
                        app_windows::italiaonline_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_CALENDAR => {
                        log_debug("Menu: Calendar");
                        app_windows::calendar_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_WEATHER => {
                        log_debug("Menu: Weather");
                        app_windows::weather_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_CINEMA => {
                        log_debug("Menu: Cinema");
                        app_windows::cinema_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_TOOLS_PATHS_NAVIGATION => {
                        log_debug("Menu: Paths and navigation");
                        app_windows::route_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_HELP_GUIDE => {
                        log_debug("Menu: Guide");
                        app_windows::help_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_HELP_CHANGELOG => {
                        log_debug("Menu: Changelog");
                        app_windows::help_window::open_changelog(hwnd);
                        LRESULT(0)
                    }
                    IDM_HELP_VISIT_SONARPAD => {
                        log_debug("Menu: Visit sonarpad.com");
                        let url = to_wide("https://sonarpad.com");
                        let res = ShellExecuteW(
                            hwnd,
                            w!("open"),
                            PCWSTR(url.as_ptr()),
                            PCWSTR::null(),
                            PCWSTR::null(),
                            SW_SHOWNORMAL,
                        );
                        if res.0 as isize <= 32 {
                            log_debug(&format!(
                                "ShellExecuteW failed to open sonarpad.com: {}",
                                res.0
                            ));
                        }
                        LRESULT(0)
                    }
                    IDM_HELP_FEEDBACK => {
                        log_debug("Menu: Feedback");
                        app_windows::feedback_window::open(hwnd);
                        LRESULT(0)
                    }
                    IDM_HELP_DONATIONS => {
                        log_debug("Menu: Donations");
                        app_windows::help_window::open_donations(hwnd);
                        LRESULT(0)
                    }
                    IDM_HELP_CHECK_UPDATES => {
                        log_debug("Menu: Check updates");
                        updater::check_for_update(hwnd, true);
                        LRESULT(0)
                    }
                    IDM_HELP_ABOUT => {
                        log_debug("Menu: About");
                        app_windows::about_window::show(hwnd);
                        LRESULT(0)
                    }
                    IDM_HELP_EXPORT_DIAGNOSTICS => {
                        log_debug("Menu: Export diagnostics");
                        export_diagnostics_dialog(hwnd);
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_SYSCHAR => {
                let ctrl_down =
                    (crate::get_key_state_safe(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
                let shift_down =
                    (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
                let alt_down =
                    (crate::get_key_state_safe(VK_MENU.0 as i32) & (0x8000u16 as i16)) != 0;
                let sys_char = wparam.0 as u32;
                if !ctrl_down && shift_down && alt_down {
                    match sys_char {
                        value if value == u32::from(b'a') || value == u32::from(b'A') => {
                            dispatch_shortcut_command(hwnd, IDM_TOOLS_RAI_AUDIODESCRIZIONI);
                            return LRESULT(0);
                        }
                        value if value == u32::from(b'r') || value == u32::from(b'R') => {
                            log_debug("shortcut probe: WM_SYSCHAR Alt+Shift+R -> radio");
                            dispatch_shortcut_command(hwnd, IDM_TOOLS_RADIO);
                            return LRESULT(0);
                        }
                        value if value == u32::from(b'n') || value == u32::from(b'N') => {
                            log_debug("shortcut probe: WM_SYSCHAR Alt+Shift+N -> paths navigation");
                            dispatch_shortcut_command(hwnd, IDM_TOOLS_PATHS_NAVIGATION);
                            return LRESULT(0);
                        }
                        _ => {}
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                log_debug(&format!(
                    "Application shutdown: WM_CLOSE received hwnd={:?}",
                    hwnd
                ));
                let audio_description_window =
                    with_state(hwnd, |state| state.audio_description_window).unwrap_or(HWND(0));
                if app_windows::audio_description_window::blocks_main_window_close(
                    hwnd,
                    audio_description_window,
                ) {
                    log_debug(
                        "Main window close requested while independent audio-description window is open; closing current document instead",
                    );
                    editor_manager::close_current_document(hwnd);
                } else {
                    if audio_description_window.0 != 0 {
                        if is_window_handle_valid(audio_description_window) {
                            log_debug(
                                "Main window close requested while audio-description window is hidden for output preview; closing application",
                            );
                        } else {
                            if with_state(hwnd, |state| {
                                if state.audio_description_window == audio_description_window {
                                    state.audio_description_window = HWND(0);
                                }
                            })
                            .is_none()
                            {
                                log_debug(
                                    "Audio description: app state unavailable while clearing stale window handle during main close",
                                );
                            }
                            log_debug(
                                "Main window close found a stale audio-description window handle; cleared it and closing application",
                            );
                        }
                    }
                    try_close_app(hwnd);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                let has_other = has_other_main_windows(hwnd);
                log_debug(&format!(
                    "Application shutdown: WM_DESTROY hwnd={:?} same_process_other_main_windows={}",
                    hwnd, has_other
                ));
                if !has_other {
                    log_debug("Application shutdown: stopping Google TTS runtime");
                    google_tts::shutdown();
                    log_debug("Application shutdown: posting WM_QUIT");
                    PostQuitMessage(0);
                }
                LRESULT(0)
            }
            WM_DROPFILES => {
                handle_drop_files(hwnd, HDROP(wparam.0 as isize));
                focus_editor(hwnd);
                LRESULT(0)
            }
            WM_COPYDATA => {
                let cds_ptr = lparam.0 as *const COPYDATASTRUCT;
                if !cds_ptr.is_null() && (*cds_ptr).dwData == COPYDATA_CALENDAR_REMINDER {
                    let Some(reminder_id) =
                        copydata_utf16_payload(cds_ptr, "WM_COPYDATA calendar reminder")
                    else {
                        return LRESULT(1);
                    };
                    let reminder_id = reminder_id.trim();
                    if !reminder_id.is_empty() {
                        app_windows::calendar_window::show_reminder_by_id(hwnd, reminder_id);
                    }
                    return LRESULT(1);
                }
                if !cds_ptr.is_null() && (*cds_ptr).dwData == COPYDATA_OPEN_FILE {
                    let Some(joined_paths) =
                        copydata_utf16_payload(cds_ptr, "WM_COPYDATA open files")
                    else {
                        return LRESULT(1);
                    };
                    if !joined_paths.is_empty() {
                        let paths: Vec<PathBuf> = joined_paths
                            .split('|')
                            .filter(|path_str| !path_str.is_empty())
                            .map(PathBuf::from)
                            .collect();
                        if defer_copydata_paths_for_pending_blocking_modal(hwnd, &paths) {
                            log_debug(
                                "WM_COPYDATA open deferred while blocking modal dialog is pending",
                            );
                            return LRESULT(COPYDATA_RESULT_DEFERRED_MODAL);
                        }
                        if defer_copydata_paths_while_main_window_disabled(hwnd, &paths) {
                            log_debug(
                                "WM_COPYDATA open deferred while the main window is disabled by a modal child",
                            );
                            return LRESULT(COPYDATA_RESULT_DEFERRED_MODAL);
                        }
                        open_copydata_paths(hwnd, paths);
                    }
                    return LRESULT(COPYDATA_RESULT_HANDLED);
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppState;
                if !ptr.is_null() {
                    let state_box = Box::from_raw(ptr);
                    if state_box.hfont_custom
                        && state_box.hfont.0 != 0
                        && !DeleteObject(state_box.hfont).as_bool()
                    {
                        log_debug("DeleteObject failed for AppState font on destroy");
                    }
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn cycle_favorite_voice(hwnd: HWND, direction: i32) {
    let (favorites, current_engine, current_voice) = {
        with_state(hwnd, |state| {
            (
                state.settings.favorite_voices.clone(),
                state.settings.tts_engine,
                state.settings.tts_voice.clone(),
            )
        })
    }
    .unwrap_or((Vec::new(), TtsEngine::Edge, String::new()));
    if favorites.is_empty() {
        return;
    }
    let mut current_idx = favorites
        .iter()
        .position(|fav| fav.engine == current_engine && fav.short_name == current_voice);
    if current_idx.is_none() {
        current_idx = Some(if direction >= 0 {
            0
        } else {
            favorites.len().saturating_sub(1)
        });
    }
    let idx = current_idx.unwrap_or(0);
    let len = favorites.len() as i32;
    let mut next_idx = idx as i32 + direction;
    if next_idx < 0 {
        next_idx = len - 1;
    } else if next_idx >= len {
        next_idx = 0;
    }
    let Some(next_fav) = favorites.get(next_idx as usize).cloned() else {
        return;
    };
    if next_fav.engine == current_engine && next_fav.short_name == current_voice {
        return;
    }
    {
        with_state(hwnd, |state| {
            state.settings.tts_engine = next_fav.engine;
            state.settings.tts_voice = next_fav.short_name.clone();
        });
    }
    let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
    app_windows::options_window::ensure_voice_lists_loaded(hwnd, language);
    refresh_voice_panel(hwnd);
    save_settings_with_synced_active_voice_profile(hwnd);
    restart_tts_from_current_offset(hwnd);
}

fn is_tts_active(hwnd: HWND) -> bool {
    with_state(hwnd, |state| state.tts_session.is_some()).unwrap_or(false)
}

struct VoicePanelLabels {
    label_engine: String,
    label_language: String,
    label_voice: String,
    label_speed: String,
    label_pitch: String,
    label_volume: String,
    label_favorites: String,
    label_multilingual: String,
    button_insert_tag: String,
    button_manage_google_voices: String,
    button_insert_pause: String,
    engine_edge: String,
    engine_sapi: String,
    engine_sapi4: String,
    engine_google: String,
    voices_empty: String,
    favorites_empty: String,
    add_favorite: String,
    remove_favorite: String,
}

fn voice_panel_labels(language: Language) -> VoicePanelLabels {
    VoicePanelLabels {
        label_engine: i18n::tr(language, "voice_panel.label_engine"),
        label_language: i18n::tr(language, "voice_panel.label_language"),
        label_voice: i18n::tr(language, "voice_panel.label_voice"),
        label_speed: i18n::tr(language, "tts_tuning.label_speed"),
        label_pitch: i18n::tr(language, "tts_tuning.label_pitch"),
        label_volume: i18n::tr(language, "tts_tuning.label_volume"),
        label_favorites: i18n::tr(language, "voice_panel.label_favorites"),
        label_multilingual: i18n::tr(language, "voice_panel.label_multilingual"),
        button_insert_tag: i18n::tr(language, "voice_panel.insert_tag"),
        button_manage_google_voices: i18n::tr(language, "menu.google_voices"),
        button_insert_pause: i18n::tr(language, "options.label.insert_pause_tag"),
        engine_edge: i18n::tr(language, "voice_panel.engine_edge"),
        engine_sapi: i18n::tr(language, "voice_panel.engine_sapi"),
        engine_sapi4: i18n::tr(language, "voice_panel.engine_sapi4"),
        engine_google: i18n::tr(language, "options.engine.google"),
        voices_empty: i18n::tr(language, "voice_panel.voices_empty"),
        favorites_empty: i18n::tr(language, "voice_panel.favorites_empty"),
        add_favorite: i18n::tr(language, "voice_panel.add_favorite"),
        remove_favorite: i18n::tr(language, "voice_panel.remove_favorite"),
    }
}

fn voice_locale_language_code(locale: &str) -> Option<String> {
    let base = locale.split(['-', '_']).next()?.trim();
    if base.is_empty() {
        return None;
    }
    Some(base.to_ascii_lowercase())
}

fn localized_voice_language_name(language: Language, code: &str) -> String {
    let key = format!("voice.lang.{}", code);
    let localized = i18n::tr(language, &key);
    if localized != key {
        return localized;
    }
    let from_i18n = |key: &str| i18n::tr(language, key);
    match code {
        "it" => from_i18n("options.lang.it"),
        "en" => from_i18n("options.lang.en"),
        "es" => from_i18n("options.lang.es"),
        "pt" => from_i18n("options.lang.pt"),
        "sv" => from_i18n("options.lang.sv"),
        "vi" => from_i18n("options.lang.vi"),
        "cs" => from_i18n("options.lang.cs"),
        "pl" => from_i18n("options.lang.pl"),
        "fr" => from_i18n("options.lang.fr"),
        "sr" => {
            let value = from_i18n("options.lang.sr");
            if value == "options.lang.sr" {
                "Serbian".to_string()
            } else {
                value
            }
        }
        "uk" => {
            let value = from_i18n("options.lang.uk");
            if value == "options.lang.uk" {
                "Ukrainian".to_string()
            } else {
                value
            }
        }
        "lt" => {
            let value = from_i18n("options.lang.lt");
            if value == "options.lang.lt" {
                "Lithuanian".to_string()
            } else {
                value
            }
        }
        "ru" => {
            let value = from_i18n("options.lang.ru");
            if value == "options.lang.ru" {
                "Russian".to_string()
            } else {
                value
            }
        }
        "zh" => {
            let value = from_i18n("options.lang.zh");
            if value == "options.lang.zh" {
                "Chinese".to_string()
            } else {
                value
            }
        }
        "de" => match language {
            Language::Italian => "Tedesco".to_string(),
            Language::Spanish => "Aleman".to_string(),
            Language::Portuguese => "Alemao".to_string(),
            Language::PortugueseBrazilian => "Alemão".to_string(),
            Language::French => "Allemand".to_string(),
            Language::German => "Deutsch".to_string(),
            _ => "German".to_string(),
        },
        _ => code.to_ascii_uppercase(),
    }
}

fn collect_voice_language_codes(voices: &[VoiceInfo]) -> Vec<String> {
    let mut codes: Vec<String> = voices
        .iter()
        .filter_map(|v| voice_locale_language_code(&v.locale))
        .collect();
    codes.sort();
    codes.dedup();
    codes
}

const TTS_RATE_MIN: i32 = -100;
const TTS_RATE_MAX: i32 = 100;
const TTS_PITCH_MIN: i32 = -12;
const TTS_PITCH_MAX: i32 = 12;
const EDGE_TTS_VOLUME_MIN: i32 = 25;
const EDGE_TTS_VOLUME_MAX: i32 = 200;
const SAPI_TTS_VOLUME_MIN: i32 = 0;
const SAPI_TTS_VOLUME_MAX: i32 = 100;
const TTS_UI_OFFSET: i32 = 100;

#[derive(Clone, Copy)]
struct TtsTuningLimits {
    rate_min: i32,
    rate_max: i32,
    pitch_min: i32,
    pitch_max: i32,
    volume_min: i32,
    volume_max: i32,
}

fn tts_tuning_limits_for_engine(engine: TtsEngine) -> TtsTuningLimits {
    match engine {
        TtsEngine::Edge => TtsTuningLimits {
            rate_min: TTS_RATE_MIN,
            rate_max: TTS_RATE_MAX,
            pitch_min: TTS_PITCH_MIN,
            pitch_max: TTS_PITCH_MAX,
            volume_min: EDGE_TTS_VOLUME_MIN,
            volume_max: EDGE_TTS_VOLUME_MAX,
        },
        TtsEngine::Google => TtsTuningLimits {
            rate_min: TTS_RATE_MIN,
            rate_max: TTS_RATE_MAX,
            pitch_min: crate::google_tts::GOOGLE_PITCH_ENCODE_BASE,
            pitch_max: crate::google_tts::GOOGLE_PITCH_ENCODE_BASE + 100,
            volume_min: SAPI_TTS_VOLUME_MIN,
            volume_max: SAPI_TTS_VOLUME_MAX,
        },
        TtsEngine::Sapi5 | TtsEngine::Sapi4 => TtsTuningLimits {
            rate_min: TTS_RATE_MIN,
            rate_max: TTS_RATE_MAX,
            pitch_min: TTS_PITCH_MIN,
            pitch_max: TTS_PITCH_MAX,
            volume_min: SAPI_TTS_VOLUME_MIN,
            volume_max: SAPI_TTS_VOLUME_MAX,
        },
    }
}

fn clamp_tts_tuning_for_engine(engine: TtsEngine, tuning: TtsTuning) -> TtsTuning {
    let limits = tts_tuning_limits_for_engine(engine);
    let pitch = if engine == TtsEngine::Google {
        crate::google_tts::normalize_google_pitch_internal(tuning.pitch)
    } else {
        tuning.pitch.clamp(limits.pitch_min, limits.pitch_max)
    };
    TtsTuning::new(
        tuning.rate.clamp(limits.rate_min, limits.rate_max),
        pitch,
        tuning.volume.clamp(limits.volume_min, limits.volume_max),
    )
}

fn set_active_tts_tuning(settings: &mut AppSettings, engine: TtsEngine, tuning: TtsTuning) {
    let tuning = clamp_tts_tuning_for_engine(engine, tuning);
    set_tts_tuning_for_engine(settings, engine, tuning);
    if settings.tts_engine == engine {
        settings.tts_rate = tuning.rate;
        settings.tts_pitch = tuning.pitch;
        settings.tts_volume = tuning.volume;
    }
}

fn save_settings_with_synced_active_voice_profile(hwnd: HWND) {
    if let Some(settings) = with_state(hwnd, |state| {
        sync_active_voice_profile_from_settings_fields(&mut state.settings);
        state.settings.clone()
    }) {
        save_settings(settings);
    }
}

fn clamp_tts_chunk_offset(previous: i32, incoming: i32) -> i32 {
    previous.max(incoming.max(0))
}

fn init_tts_panel_combo(hwnd: HWND, items: &[(String, i32)]) {
    unsafe {
        SendMessageW(hwnd, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        for (label, value) in items {
            let idx = SendMessageW(
                hwnd,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(label).as_ptr() as isize),
            )
            .0 as usize;
            SendMessageW(hwnd, CB_SETITEMDATA, WPARAM(idx), LPARAM(*value as isize));
        }
    }
}

fn combo_value(hwnd: HWND) -> i32 {
    unsafe {
        let sel = SendMessageW(hwnd, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        if sel < 0 {
            return 0;
        }
        SendMessageW(hwnd, CB_GETITEMDATA, WPARAM(sel as usize), LPARAM(0)).0 as i32
    }
}

fn select_combo_nearest_value(hwnd: HWND, value: i32) {
    unsafe {
        let count = SendMessageW(hwnd, CB_GETCOUNT, WPARAM(0), LPARAM(0)).0;
        if count <= 0 {
            return;
        }
        let mut best_idx = 0;
        let mut best_diff = i32::MAX;
        for i in 0..count {
            let data = SendMessageW(hwnd, CB_GETITEMDATA, WPARAM(i as usize), LPARAM(0)).0 as i32;
            let diff = (data - value).abs();
            if diff < best_diff {
                best_diff = diff;
                best_idx = i;
            }
        }
        SendMessageW(hwnd, CB_SETCURSEL, WPARAM(best_idx as usize), LPARAM(0));
    }
}

fn voice_panel_pitch_value_from_combo(combo: HWND, engine: TtsEngine) -> i32 {
    let raw = combo_value(combo);
    if engine == TtsEngine::Google {
        crate::google_tts::google_pitch_preset_internal(raw)
    } else {
        raw
    }
}

fn select_voice_panel_pitch_combo(combo: HWND, engine: TtsEngine, value: i32) {
    unsafe {
        let count = SendMessageW(combo, CB_GETCOUNT, WPARAM(0), LPARAM(0)).0;
        if count <= 0 {
            return;
        }
        let target = if engine == TtsEngine::Google {
            crate::google_tts::google_pitch_percent_from_internal(value)
        } else {
            value
        };
        let mut best_idx = 0;
        let mut best_diff = i32::MAX;
        for i in 0..count {
            let raw = SendMessageW(combo, CB_GETITEMDATA, WPARAM(i as usize), LPARAM(0)).0 as i32;
            let candidate = if engine == TtsEngine::Google {
                crate::google_tts::google_pitch_percent_from_internal(raw)
            } else {
                raw
            };
            let diff = (candidate - target).abs();
            if diff < best_diff {
                best_diff = diff;
                best_idx = i;
            }
        }
        SendMessageW(combo, CB_SETCURSEL, WPARAM(best_idx as usize), LPARAM(0));
    }
}

fn voice_panel_pitch_ui_value(engine: TtsEngine, value: i32) -> i32 {
    if engine == TtsEngine::Google {
        crate::google_tts::google_pitch_percent_from_internal(value)
    } else {
        tts_ui_value_from_internal(value)
    }
}

fn read_voice_panel_pitch_with_clamp(
    edit: HWND,
    engine: TtsEngine,
    fallback_internal: i32,
) -> (i32, Option<i32>) {
    if engine == TtsEngine::Google {
        let fallback = crate::google_tts::google_pitch_percent_from_internal(fallback_internal);
        let (percent, adjusted) = read_tts_edit_value_with_clamp(edit, fallback, 0, 100);
        (
            crate::google_tts::google_pitch_percent_to_internal(percent),
            adjusted,
        )
    } else {
        let limits = tts_tuning_limits_for_engine(engine);
        read_tts_tuning_edit_value_with_clamp(
            edit,
            fallback_internal,
            limits.pitch_min,
            limits.pitch_max,
        )
    }
}

fn set_voice_panel_pitch_edit_text(edit: HWND, engine: TtsEngine, value: i32) {
    let display_value = voice_panel_pitch_ui_value(engine, value);
    if let Err(e) =
        crate::set_window_text_w_safe(edit, PCWSTR(to_wide(&display_value.to_string()).as_ptr()))
    {
        log_debug(&format!("Failed to update Google pitch edit: {e}"));
    }
}

fn read_tts_edit_value_with_clamp(
    edit: HWND,
    fallback: i32,
    min: i32,
    max: i32,
) -> (i32, Option<i32>) {
    unsafe {
        let len = GetWindowTextLengthW(edit);
        if len <= 0 {
            return (fallback, None);
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        let read = GetWindowTextW(edit, &mut buf);
        let text = String::from_utf16_lossy(&buf[..read as usize]);
        match text.trim().parse::<i32>() {
            Ok(parsed) => {
                let clamped = parsed.clamp(min, max);
                if clamped != parsed {
                    (clamped, Some(clamped))
                } else {
                    (clamped, None)
                }
            }
            Err(_) => (fallback, Some(fallback)),
        }
    }
}

fn tts_ui_value_from_internal(value: i32) -> i32 {
    value + TTS_UI_OFFSET
}

fn read_tts_tuning_edit_value_with_clamp(
    edit: HWND,
    fallback_internal: i32,
    min: i32,
    max: i32,
) -> (i32, Option<i32>) {
    let ui_min = min + TTS_UI_OFFSET;
    let ui_max = max + TTS_UI_OFFSET;
    let ui_fallback = tts_ui_value_from_internal(fallback_internal).clamp(ui_min, ui_max);
    let (ui_value, adjusted) = read_tts_edit_value_with_clamp(edit, ui_fallback, ui_min, ui_max);
    ((ui_value - TTS_UI_OFFSET).clamp(min, max), adjusted)
}

fn set_tts_tuning_edit_text(edit: HWND, value: i32, is_tuning_value: bool) {
    let display_value = if is_tuning_value {
        tts_ui_value_from_internal(value)
    } else {
        value
    };
    if let Err(e) =
        crate::set_window_text_w_safe(edit, PCWSTR(to_wide(&display_value.to_string()).as_ptr()))
    {
        log_debug(&format!("Failed to update voice panel tuning edit: {e}"));
    }
}

fn text_color_menu_id(text_color: u32) -> usize {
    match text_color {
        0x000000 => IDM_VIEW_TEXT_COLOR_BLACK,
        0x800000 => IDM_VIEW_TEXT_COLOR_DARK_BLUE,
        0x006400 => IDM_VIEW_TEXT_COLOR_DARK_GREEN,
        0x002850 => IDM_VIEW_TEXT_COLOR_DARK_BROWN,
        0x404040 => IDM_VIEW_TEXT_COLOR_DARK_GRAY,
        0xFFCC99 => IDM_VIEW_TEXT_COLOR_LIGHT_BLUE,
        0x99CC99 => IDM_VIEW_TEXT_COLOR_LIGHT_GREEN,
        0x99B2CC => IDM_VIEW_TEXT_COLOR_LIGHT_BROWN,
        0xC0C0C0 => IDM_VIEW_TEXT_COLOR_LIGHT_GRAY,
        _ => IDM_VIEW_TEXT_COLOR_BLACK,
    }
}

fn text_color_from_menu_id(cmd_id: usize) -> Option<u32> {
    match cmd_id {
        IDM_VIEW_TEXT_COLOR_BLACK => Some(0x000000),
        IDM_VIEW_TEXT_COLOR_DARK_BLUE => Some(0x800000),
        IDM_VIEW_TEXT_COLOR_DARK_GREEN => Some(0x006400),
        IDM_VIEW_TEXT_COLOR_DARK_BROWN => Some(0x002850),
        IDM_VIEW_TEXT_COLOR_DARK_GRAY => Some(0x404040),
        IDM_VIEW_TEXT_COLOR_LIGHT_BLUE => Some(0xFFCC99),
        IDM_VIEW_TEXT_COLOR_LIGHT_GREEN => Some(0x99CC99),
        IDM_VIEW_TEXT_COLOR_LIGHT_BROWN => Some(0x99B2CC),
        IDM_VIEW_TEXT_COLOR_LIGHT_GRAY => Some(0xC0C0C0),
        _ => None,
    }
}

fn text_size_menu_id(text_size: i32) -> usize {
    match text_size {
        10 => IDM_VIEW_TEXT_SIZE_SMALL,
        12 => IDM_VIEW_TEXT_SIZE_NORMAL,
        16 => IDM_VIEW_TEXT_SIZE_LARGE,
        20 => IDM_VIEW_TEXT_SIZE_XLARGE,
        24 => IDM_VIEW_TEXT_SIZE_XXLARGE,
        _ => IDM_VIEW_TEXT_SIZE_NORMAL,
    }
}

fn text_size_from_menu_id(cmd_id: usize) -> Option<i32> {
    match cmd_id {
        IDM_VIEW_TEXT_SIZE_SMALL => Some(10),
        IDM_VIEW_TEXT_SIZE_NORMAL => Some(12),
        IDM_VIEW_TEXT_SIZE_LARGE => Some(16),
        IDM_VIEW_TEXT_SIZE_XLARGE => Some(20),
        IDM_VIEW_TEXT_SIZE_XXLARGE => Some(24),
        _ => None,
    }
}

fn font_face_from_menu_id(cmd_id: usize) -> Option<&'static str> {
    match cmd_id {
        IDM_VIEW_FONT_ARIAL => Some("Arial"),
        IDM_VIEW_FONT_CALIBRI => Some("Calibri"),
        IDM_VIEW_FONT_CONSOLAS => Some("Consolas"),
        IDM_VIEW_FONT_SEGOE_UI => Some("Segoe UI"),
        IDM_VIEW_FONT_TAHOMA => Some("Tahoma"),
        IDM_VIEW_FONT_VERDANA => Some("Verdana"),
        IDM_VIEW_FONT_TIMES_NEW_ROMAN => Some("Times New Roman"),
        IDM_VIEW_FONT_GEORGIA => Some("Georgia"),
        _ => None,
    }
}

fn create_ui_font(
    face_name: &str,
    base_font: Option<HFONT>,
    fallback_text_size: i32,
) -> Option<HFONT> {
    if face_name.trim().is_empty() {
        return None;
    }
    let mut logfont = LOGFONTW::default();
    if let Some(font) = base_font
        && font.0 != 0
    {
        let copied = unsafe {
            GetObjectW(
                font,
                size_of::<LOGFONTW>() as i32,
                Some((&mut logfont as *mut LOGFONTW).cast()),
            )
        };
        if copied == 0 {
            log_debug("GetObjectW failed for base font; using fallback LOGFONT");
        }
    }
    if logfont.lfHeight == 0 {
        let points = fallback_text_size.max(1);
        // LOGFONT height is in pixels; negative means character height.
        logfont.lfHeight = -((points * 96 + 36) / 72);
    }
    logfont.lfFaceName.fill(0);
    let face_wide = to_wide(face_name);
    let mut i = 0usize;
    while i + 1 < face_wide.len() && i < logfont.lfFaceName.len() {
        logfont.lfFaceName[i] = face_wide[i];
        i += 1;
    }
    let hfont = unsafe { windows::Win32::Graphics::Gdi::CreateFontIndirectW(&logfont) };
    if hfont.0 == 0 { None } else { Some(hfont) }
}

fn apply_ui_font(hwnd: HWND, face_name: String) {
    let (base_font, text_size) =
        { with_state(hwnd, |state| (state.hfont, state.settings.text_size)) }
            .unwrap_or((HFONT(0), 12));
    let custom_font = create_ui_font(
        &face_name,
        if base_font.0 != 0 {
            Some(base_font)
        } else {
            None
        },
        text_size,
    );
    let is_custom = custom_font.is_some() && !face_name.trim().is_empty();
    let new_font_resolved =
        custom_font.unwrap_or_else(|| HFONT(crate::get_stock_object_safe(DEFAULT_GUI_FONT).0));
    let Some((new_font, old_font, old_custom)) = {
        with_state(hwnd, |state| {
            let old_font = state.hfont;
            let old_custom = state.hfont_custom;
            state.hfont = new_font_resolved;
            state.hfont_custom = is_custom;
            state.settings.editor_font_face = face_name.clone();
            Some((new_font_resolved, old_font, old_custom))
        })
    }
    .flatten() else {
        return;
    };

    if old_custom
        && old_font.0 != 0
        && old_font != new_font
        && !unsafe { DeleteObject(old_font) }.as_bool()
    {
        log_debug("DeleteObject failed for previous UI font");
    }

    let controls = {
        with_state(hwnd, |state| {
            vec![
                state.hwnd_tab,
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
            ]
        })
    }
    .unwrap_or_default();
    for control in controls {
        if control.0 != 0 {
            unsafe {
                SendMessageW(control, WM_SETFONT, WPARAM(new_font.0 as usize), LPARAM(1));
            }
        }
    }
    editor_manager::apply_font_to_all_edits(hwnd, new_font);
    if let Some(settings) = { with_state(hwnd, |state| state.settings.clone()) } {
        save_settings(settings);
    }
}

fn update_text_preferences(hwnd: HWND, text_color: Option<u32>, text_size: Option<i32>) {
    let mut changed = false;
    let mut next_color = None;
    let mut next_size = None;
    {
        with_state(hwnd, |state| {
            if let Some(color) = text_color {
                if state.settings.text_color != color {
                    state.settings.text_color = color;
                    changed = true;
                }
                next_color = Some(state.settings.text_color);
            } else {
                next_color = Some(state.settings.text_color);
            }
            if let Some(size) = text_size {
                if state.settings.text_size != size {
                    state.settings.text_size = size;
                    changed = true;
                }
                next_size = Some(state.settings.text_size);
            } else {
                next_size = Some(state.settings.text_size);
            }
        });
    }

    let (color, size) = match (next_color, next_size) {
        (Some(c), Some(s)) => (c, s),
        _ => return,
    };
    if changed && let Some(settings) = { with_state(hwnd, |state| state.settings.clone()) } {
        save_settings(settings);
    }
    editor_manager::apply_text_appearance_to_all_edits(hwnd, color, size);
    update_voice_panel_menu_check(hwnd);
}

pub(crate) fn update_voice_panel_menu_check(hwnd: HWND) {
    let (
        visible,
        favorites_visible,
        text_color,
        text_size,
        read_only,
        word_wrap,
        show_video,
        dark_mode,
        automatic_bookmark,
    ) = {
        with_state(hwnd, |state| {
            (
                state.voice_panel_visible,
                state.voice_favorites_visible,
                state.settings.text_color,
                state.settings.text_size,
                state.settings.editor_read_only,
                state.settings.word_wrap,
                state.settings.show_video_during_playback,
                state.settings.dark_mode,
                state.settings.automatic_bookmark,
            )
        })
    }
    .unwrap_or((false, false, 0x000000, 12, false, true, true, false, false));
    let hmenu = crate::get_menu_safe(hwnd);
    if hmenu.0 == 0 {
        return;
    }
    let flags = if visible { MF_CHECKED } else { MF_UNCHECKED };
    if crate::check_menu_item_safe(hmenu, IDM_VIEW_SHOW_VOICES as u32, (MF_BYCOMMAND | flags).0)
        == 0xFFFFFFFF
    {
        crate::log_debug("CheckMenuItem failed for IDM_VIEW_SHOW_VOICES");
    }
    let fav_flags = if favorites_visible {
        MF_CHECKED
    } else {
        MF_UNCHECKED
    };
    unsafe {
        CheckMenuItem(
            hmenu,
            IDM_VIEW_SHOW_FAVORITES as u32,
            (MF_BYCOMMAND | fav_flags).0,
        );
    }
    let read_only_flags = if read_only { MF_CHECKED } else { MF_UNCHECKED };
    unsafe {
        CheckMenuItem(
            hmenu,
            IDM_VIEW_READ_ONLY as u32,
            (MF_BYCOMMAND | read_only_flags).0,
        );
    }
    let wrap_flags = if word_wrap { MF_CHECKED } else { MF_UNCHECKED };
    unsafe {
        CheckMenuItem(
            hmenu,
            IDM_VIEW_WORD_WRAP as u32,
            (MF_BYCOMMAND | wrap_flags).0,
        );
    }
    let video_flags = if show_video { MF_CHECKED } else { MF_UNCHECKED };
    if crate::check_menu_item_safe(
        hmenu,
        IDM_VIEW_SHOW_VIDEO_DURING_PLAYBACK as u32,
        (MF_BYCOMMAND | video_flags).0,
    ) == 0xFFFFFFFF
    {
        crate::log_debug("CheckMenuItem failed for IDM_VIEW_SHOW_VIDEO_DURING_PLAYBACK");
    }
    let dark_mode_flags = if dark_mode { MF_CHECKED } else { MF_UNCHECKED };
    if crate::check_menu_item_safe(
        hmenu,
        IDM_VIEW_DARK_MODE as u32,
        (MF_BYCOMMAND | dark_mode_flags).0,
    ) == 0xFFFFFFFF
    {
        crate::log_debug("CheckMenuItem failed for IDM_VIEW_DARK_MODE");
    }
    let automatic_bookmark_flags = if automatic_bookmark {
        MF_CHECKED
    } else {
        MF_UNCHECKED
    };
    if crate::check_menu_item_safe(
        hmenu,
        IDM_AUTOMATIC_BOOKMARK as u32,
        (MF_BYCOMMAND | automatic_bookmark_flags).0,
    ) == 0xFFFFFFFF
    {
        crate::log_debug("CheckMenuItem failed for IDM_AUTOMATIC_BOOKMARK");
    }

    let color_items = [
        IDM_VIEW_TEXT_COLOR_BLACK,
        IDM_VIEW_TEXT_COLOR_DARK_BLUE,
        IDM_VIEW_TEXT_COLOR_DARK_GREEN,
        IDM_VIEW_TEXT_COLOR_DARK_BROWN,
        IDM_VIEW_TEXT_COLOR_DARK_GRAY,
        IDM_VIEW_TEXT_COLOR_LIGHT_BLUE,
        IDM_VIEW_TEXT_COLOR_LIGHT_GREEN,
        IDM_VIEW_TEXT_COLOR_LIGHT_BROWN,
        IDM_VIEW_TEXT_COLOR_LIGHT_GRAY,
    ];
    let selected_color = text_color_menu_id(text_color);
    for item in color_items {
        let item_flags = if item == selected_color {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        if crate::check_menu_item_safe(hmenu, item as u32, (MF_BYCOMMAND | item_flags).0)
            == 0xFFFFFFFF
        {
            crate::log_debug("CheckMenuItem failed for view item");
        }
    }

    let size_items = [
        IDM_VIEW_TEXT_SIZE_SMALL,
        IDM_VIEW_TEXT_SIZE_NORMAL,
        IDM_VIEW_TEXT_SIZE_LARGE,
        IDM_VIEW_TEXT_SIZE_XLARGE,
        IDM_VIEW_TEXT_SIZE_XXLARGE,
    ];
    let selected_size = text_size_menu_id(text_size);
    for item in size_items {
        let item_flags = if item == selected_size {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        if crate::check_menu_item_safe(hmenu, item as u32, (MF_BYCOMMAND | item_flags).0)
            == 0xFFFFFFFF
        {
            crate::log_debug("CheckMenuItem failed for color item");
        }
    }
}

fn toggle_voice_panel(hwnd: HWND) {
    let visible = { with_state(hwnd, |state| state.voice_panel_visible) }.unwrap_or(false);
    set_voice_panel_visible(hwnd, !visible);
}

fn set_voice_panel_visible(hwnd: HWND, visible: bool) {
    set_voice_panel_visible_internal(hwnd, visible, true);
}

fn set_voice_panel_visible_internal(hwnd: HWND, visible: bool, persist: bool) {
    let (
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
        changed,
    ) = match with_state(hwnd, |state| {
        let changed = state.settings.show_voice_panel != visible;
        state.voice_panel_visible = visible;
        state.settings.show_voice_panel = visible;
        (
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
            changed,
        )
    }) {
        Some(values) => values,
        None => return,
    };

    let show = if visible { SW_SHOW } else { SW_HIDE };
    for control in [
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
    ] {
        if control.0 != 0 {
            unsafe {
                ShowWindow(control, show);
            }
        }
    }
    let show_manage_google = visible
        && with_state(hwnd, |state| state.settings.tts_engine == TtsEngine::Google)
            .unwrap_or(false);
    if button_manage_google_voices.0 != 0 {
        unsafe {
            ShowWindow(
                button_manage_google_voices,
                if show_manage_google { SW_SHOW } else { SW_HIDE },
            );
            EnableWindow(button_manage_google_voices, show_manage_google);
        }
    }
    update_voice_panel_menu_check(hwnd);
    if visible {
        let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
        app_windows::options_window::ensure_voice_lists_loaded(hwnd, language);
        refresh_voice_panel(hwnd);
    }
    if persist
        && changed
        && let Some(settings) = { with_state(hwnd, |state| state.settings.clone()) }
    {
        save_settings(settings);
    }
    clear_voice_labels_if_hidden(hwnd);
    editor_manager::layout_children(hwnd);
    sync_voice_panel_visibility_for_current_document(hwnd);
}

fn toggle_favorites_panel(hwnd: HWND) {
    let visible = { with_state(hwnd, |state| state.voice_favorites_visible) }.unwrap_or(false);
    set_favorites_panel_visible(hwnd, !visible);
}

fn set_favorites_panel_visible(hwnd: HWND, visible: bool) {
    set_favorites_panel_visible_internal(hwnd, visible, true);
}

fn set_favorites_panel_visible_internal(hwnd: HWND, visible: bool, persist: bool) {
    let (label_favorites, combo_favorites, changed) = match with_state(hwnd, |state| {
        let changed = state.settings.show_favorite_panel != visible;
        state.voice_favorites_visible = visible;
        state.settings.show_favorite_panel = visible;
        (
            state.voice_label_favorites,
            state.voice_combo_favorites,
            changed,
        )
    }) {
        Some(values) => values,
        None => return,
    };
    let show = if visible { SW_SHOW } else { SW_HIDE };
    for control in [label_favorites, combo_favorites] {
        if control.0 != 0 {
            unsafe {
                ShowWindow(control, show);
            }
        }
    }
    update_voice_panel_menu_check(hwnd);
    if visible {
        let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
        app_windows::options_window::ensure_voice_lists_loaded(hwnd, language);
        refresh_voice_panel(hwnd);
    }
    if persist
        && changed
        && let Some(settings) = { with_state(hwnd, |state| state.settings.clone()) }
    {
        save_settings(settings);
    }
    clear_voice_labels_if_hidden(hwnd);
    editor_manager::layout_children(hwnd);
    sync_voice_panel_visibility_for_current_document(hwnd);
}

pub(crate) fn sync_voice_panel_visibility_for_current_document(hwnd: HWND) {
    unsafe {
        let data = with_state(hwnd, |state| {
            let is_audiobook = state
                .docs
                .get(state.current)
                .map(|doc| matches!(doc.format, FileFormat::Audiobook))
                .unwrap_or(false);
            let player_active = is_audiobook || state.active_mpv_session.is_some();
            (
                player_active,
                state.voice_panel_visible,
                state.voice_favorites_visible,
                state.settings.tts_engine,
                state.settings.tts_only_multilingual,
                state.settings.tts_manual_tuning,
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
        let Some((
            player_active,
            voice_requested,
            favorites_requested,
            tts_engine,
            tts_only_multilingual,
            manual_tuning,
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
        )) = data
        else {
            return;
        };

        let voice_visible = voice_requested && !player_active;
        let favorites_visible = favorites_requested && !player_active;
        let show = |control: HWND, visible: bool| {
            if control.0 != 0 {
                ShowWindow(control, if visible { SW_SHOW } else { SW_HIDE });
            }
        };

        for control in [
            label_engine,
            combo_engine,
            label_voice,
            combo_voice,
            button_insert_tag,
            button_insert_pause,
            label_speed,
            label_pitch,
            label_volume,
        ] {
            show(control, voice_visible);
        }
        let is_edge = matches!(tts_engine, TtsEngine::Edge);
        let is_google = matches!(tts_engine, TtsEngine::Google);
        show(
            label_language,
            voice_visible && is_edge && !tts_only_multilingual,
        );
        show(
            combo_language,
            voice_visible && is_edge && !tts_only_multilingual,
        );
        show(checkbox_multilingual, voice_visible && is_edge);
        show(button_manage_google_voices, voice_visible && is_google);
        show(combo_speed, voice_visible && !manual_tuning);
        show(combo_pitch, voice_visible && !manual_tuning);
        show(combo_volume, voice_visible && !manual_tuning);
        show(edit_speed, voice_visible && manual_tuning);
        show(edit_pitch, voice_visible && manual_tuning);
        show(edit_volume, voice_visible && manual_tuning);
        show(label_favorites, favorites_visible);
        show(combo_favorites, favorites_visible);

        if player_active {
            log_debug("Voice panels suppressed while player playback is active");
            return;
        }
        if voice_visible || favorites_visible {
            refresh_voice_panel(hwnd);
        }
    }
}

pub(crate) fn refresh_voice_panel(hwnd: HWND) {
    let player_active = with_state(hwnd, |state| {
        state
            .docs
            .get(state.current)
            .map(|doc| matches!(doc.format, FileFormat::Audiobook))
            .unwrap_or(false)
            || state.active_mpv_session.is_some()
    })
    .unwrap_or(false);
    if player_active {
        sync_voice_panel_visibility_for_current_document(hwnd);
        return;
    }

    unsafe {
        let (
            voice_visible,
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
            favorites_visible,
            label_favorites,
            combo_favorites,
        ) = match with_state(hwnd, |state| {
            (
                state.voice_panel_visible,
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
                state.voice_favorites_visible,
                state.voice_label_favorites,
                state.voice_combo_favorites,
            )
        }) {
            Some(values) => values,
            None => return,
        };
        if !voice_visible && !favorites_visible {
            return;
        }

        let settings = with_state(hwnd, |state| state.settings.clone()).unwrap_or_default();
        let labels = voice_panel_labels(settings.language);
        if voice_visible {
            let label_engine_wide = to_wide(&labels.label_engine);
            let label_language_wide = to_wide(&labels.label_language);
            let label_voice_wide = to_wide(&labels.label_voice);
            let label_speed_wide = to_wide(&labels.label_speed);
            let label_pitch_wide = to_wide(&labels.label_pitch);
            let label_volume_wide = to_wide(&labels.label_volume);
            crate::log_if_err!(SetWindowTextW(
                label_engine,
                PCWSTR(label_engine_wide.as_ptr())
            ));
            crate::log_if_err!(SetWindowTextW(
                label_language,
                PCWSTR(label_language_wide.as_ptr())
            ));
            crate::log_if_err!(SetWindowTextW(
                label_voice,
                PCWSTR(label_voice_wide.as_ptr())
            ));
            let button_insert_wide = to_wide(&labels.button_insert_tag);
            let button_manage_google_wide = to_wide(&labels.button_manage_google_voices);
            let button_pause_wide = to_wide(&labels.button_insert_pause);
            crate::log_if_err!(SetWindowTextW(
                button_insert_tag,
                PCWSTR(button_insert_wide.as_ptr())
            ));
            crate::log_if_err!(SetWindowTextW(
                button_manage_google_voices,
                PCWSTR(button_manage_google_wide.as_ptr())
            ));
            crate::log_if_err!(SetWindowTextW(
                button_insert_pause,
                PCWSTR(button_pause_wide.as_ptr())
            ));
            crate::log_if_err!(SetWindowTextW(
                label_speed,
                PCWSTR(label_speed_wide.as_ptr())
            ));
            crate::log_if_err!(SetWindowTextW(
                label_pitch,
                PCWSTR(label_pitch_wide.as_ptr())
            ));
            crate::log_if_err!(SetWindowTextW(
                label_volume,
                PCWSTR(label_volume_wide.as_ptr())
            ));
            let label_multi_wide = to_wide(&labels.label_multilingual);
            crate::log_if_err!(SetWindowTextW(
                checkbox_multilingual,
                PCWSTR(label_multi_wide.as_ptr())
            ));
        }
        if favorites_visible && label_favorites.0 != 0 {
            let label_fav_wide = to_wide(&labels.label_favorites);
            crate::log_if_err!(SetWindowTextW(
                label_favorites,
                PCWSTR(label_fav_wide.as_ptr())
            ));
        }

        if voice_visible && combo_engine.0 != 0 && combo_voice.0 != 0 {
            SendMessageW(combo_engine, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
            SendMessageW(
                combo_engine,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&labels.engine_edge).as_ptr() as isize),
            );
            SendMessageW(
                combo_engine,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&labels.engine_sapi).as_ptr() as isize),
            );
            SendMessageW(
                combo_engine,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&labels.engine_sapi4).as_ptr() as isize),
            );
            SendMessageW(
                combo_engine,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&labels.engine_google).as_ptr() as isize),
            );
            let engine_index = match settings.tts_engine {
                TtsEngine::Edge => 0,
                TtsEngine::Sapi5 => 1,
                TtsEngine::Sapi4 => 2,
                TtsEngine::Google => 3,
            };
            SendMessageW(combo_engine, CB_SETCURSEL, WPARAM(engine_index), LPARAM(0));
            let is_google = matches!(settings.tts_engine, TtsEngine::Google);
            ShowWindow(
                button_manage_google_voices,
                if is_google { SW_SHOW } else { SW_HIDE },
            );
            EnableWindow(button_manage_google_voices, is_google);
            let is_edge = matches!(settings.tts_engine, TtsEngine::Edge);
            SendMessageW(
                checkbox_multilingual,
                BM_SETCHECK,
                WPARAM(if settings.tts_only_multilingual {
                    BST_CHECKED.0 as usize
                } else {
                    0
                }),
                LPARAM(0),
            );
            EnableWindow(checkbox_multilingual, is_edge);
            let multi_show = if is_edge { SW_SHOW } else { SW_HIDE };
            ShowWindow(checkbox_multilingual, multi_show);
            let show_language_combo = is_edge && !settings.tts_only_multilingual;
            ShowWindow(
                label_language,
                if show_language_combo {
                    SW_SHOW
                } else {
                    SW_HIDE
                },
            );
            ShowWindow(
                combo_language,
                if show_language_combo {
                    SW_SHOW
                } else {
                    SW_HIDE
                },
            );
            EnableWindow(combo_language, show_language_combo);
        }

        if voice_visible {
            let speed_items = [
                (
                    i18n::tr(settings.language, "tts_tuning.speed.extremely_slow"),
                    -100,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.speed.very_slow"),
                    -60,
                ),
                (i18n::tr(settings.language, "tts_tuning.speed.slow"), -35),
                (
                    i18n::tr(settings.language, "tts_tuning.speed.a_bit_slow"),
                    -20,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.speed.slightly_slow"),
                    -10,
                ),
                (i18n::tr(settings.language, "tts_tuning.speed.normal"), 0),
                (
                    i18n::tr(settings.language, "tts_tuning.speed.slightly_fast"),
                    10,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.speed.a_bit_fast"),
                    20,
                ),
                (i18n::tr(settings.language, "tts_tuning.speed.fast"), 35),
                (
                    i18n::tr(settings.language, "tts_tuning.speed.very_fast"),
                    50,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.speed.super_fast"),
                    100,
                ),
            ];
            let pitch_items = [
                (
                    i18n::tr(settings.language, "tts_tuning.pitch.very_low"),
                    -12,
                ),
                (i18n::tr(settings.language, "tts_tuning.pitch.low"), -10),
                (
                    i18n::tr(settings.language, "tts_tuning.pitch.a_bit_low"),
                    -7,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.pitch.slightly_low"),
                    -5,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.pitch.a_little_lower"),
                    -2,
                ),
                (i18n::tr(settings.language, "tts_tuning.pitch.normal"), 0),
                (
                    i18n::tr(settings.language, "tts_tuning.pitch.a_little_higher"),
                    2,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.pitch.slightly_high"),
                    5,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.pitch.a_bit_high"),
                    7,
                ),
                (i18n::tr(settings.language, "tts_tuning.pitch.high"), 9),
                (
                    i18n::tr(settings.language, "tts_tuning.pitch.very_high"),
                    12,
                ),
            ];
            let volume_items = [
                (
                    i18n::tr(settings.language, "tts_tuning.volume.very_low"),
                    25,
                ),
                (i18n::tr(settings.language, "tts_tuning.volume.low"), 40),
                (
                    i18n::tr(settings.language, "tts_tuning.volume.a_bit_low"),
                    55,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.volume.medium_low"),
                    70,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.volume.slightly_low"),
                    85,
                ),
                (i18n::tr(settings.language, "tts_tuning.volume.normal"), 100),
                (
                    i18n::tr(settings.language, "tts_tuning.volume.slightly_high"),
                    115,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.volume.medium_high"),
                    130,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.volume.a_bit_high"),
                    145,
                ),
                (i18n::tr(settings.language, "tts_tuning.volume.high"), 160),
                (
                    i18n::tr(settings.language, "tts_tuning.volume.very_high"),
                    180,
                ),
                (
                    i18n::tr(settings.language, "tts_tuning.volume.maximum"),
                    200,
                ),
            ];
            init_tts_panel_combo(combo_speed, &speed_items);
            init_tts_panel_combo(combo_pitch, &pitch_items);
            init_tts_panel_combo(combo_volume, &volume_items);
            select_combo_nearest_value(combo_speed, settings.tts_rate);
            select_voice_panel_pitch_combo(combo_pitch, settings.tts_engine, settings.tts_pitch);
            select_combo_nearest_value(combo_volume, settings.tts_volume);
            crate::log_if_err!(SetWindowTextW(
                edit_speed,
                PCWSTR(
                    to_wide(&tts_ui_value_from_internal(settings.tts_rate).to_string()).as_ptr()
                ),
            ));
            crate::log_if_err!(SetWindowTextW(
                edit_pitch,
                PCWSTR(
                    to_wide(
                        &voice_panel_pitch_ui_value(settings.tts_engine, settings.tts_pitch)
                            .to_string(),
                    )
                    .as_ptr()
                ),
            ));
            crate::log_if_err!(SetWindowTextW(
                edit_volume,
                PCWSTR(to_wide(&settings.tts_volume.to_string()).as_ptr()),
            ));
            let manual = settings.tts_manual_tuning;
            ShowWindow(combo_speed, if manual { SW_HIDE } else { SW_SHOW });
            ShowWindow(combo_pitch, if manual { SW_HIDE } else { SW_SHOW });
            ShowWindow(combo_volume, if manual { SW_HIDE } else { SW_SHOW });
            ShowWindow(edit_speed, if manual { SW_SHOW } else { SW_HIDE });
            ShowWindow(edit_pitch, if manual { SW_SHOW } else { SW_HIDE });
            ShowWindow(edit_volume, if manual { SW_SHOW } else { SW_HIDE });
            EnableWindow(combo_speed, !manual);
            EnableWindow(combo_pitch, !manual);
            EnableWindow(combo_volume, !manual);
            EnableWindow(edit_speed, manual);
            EnableWindow(edit_pitch, manual);
            EnableWindow(edit_volume, manual);
            let voices: Vec<crate::settings::VoiceInfo> =
                with_state(hwnd, |state| match settings.tts_engine {
                    TtsEngine::Edge => state.edge_voices.clone(),
                    TtsEngine::Google => crate::google_tts::installed_voices(),
                    TtsEngine::Sapi5 => state.sapi_voices.clone(),
                    TtsEngine::Sapi4 => crate::sapi4_engine::get_voices(),
                })
                .unwrap_or_default();
            let mut language_filter: Option<String> = None;
            let show_language_combo =
                matches!(settings.tts_engine, TtsEngine::Edge) && !settings.tts_only_multilingual;
            if show_language_combo {
                let previous_selection = with_state(hwnd, |state| {
                    let sel = SendMessageW(combo_language, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
                    if sel >= 0 {
                        state.voice_language_codes.get(sel as usize).cloned()
                    } else {
                        None
                    }
                })
                .flatten();
                let mut codes = collect_voice_language_codes(&voices);
                if !codes.is_empty() {
                    let selected_from_voice = voices
                        .iter()
                        .find(|v| v.short_name == settings.tts_voice)
                        .and_then(|v| voice_locale_language_code(&v.locale));
                    let selected_code = previous_selection
                        .filter(|code| codes.contains(code))
                        .or(selected_from_voice.filter(|code| codes.contains(code)))
                        .unwrap_or_else(|| codes[0].clone());
                    SendMessageW(combo_language, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
                    let mut selected_index: Option<usize> = None;
                    for (idx, code) in codes.iter().enumerate() {
                        let label = localized_voice_language_name(settings.language, code);
                        let added = SendMessageW(
                            combo_language,
                            CB_ADDSTRING,
                            WPARAM(0),
                            LPARAM(to_wide(&label).as_ptr() as isize),
                        )
                        .0;
                        if added >= 0 && *code == selected_code {
                            selected_index = Some(idx);
                        }
                    }
                    SendMessageW(
                        combo_language,
                        CB_SETCURSEL,
                        WPARAM(selected_index.unwrap_or(0)),
                        LPARAM(0),
                    );
                    language_filter = Some(selected_code);
                }
                if with_state(hwnd, |state| {
                    state.voice_language_codes = std::mem::take(&mut codes);
                })
                .is_none()
                {
                    log_debug("Failed to update voice language codes");
                }
            } else {
                SendMessageW(combo_language, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
                if with_state(hwnd, |state| state.voice_language_codes.clear()).is_none() {
                    log_debug("Failed to clear voice language codes");
                }
            }
            populate_voice_panel_combo(
                combo_voice,
                settings.tts_engine,
                &voices,
                &settings.tts_voice,
                settings.tts_engine == TtsEngine::Edge && settings.tts_only_multilingual,
                language_filter.as_deref(),
                &labels.voices_empty,
            );
        }
        if favorites_visible {
            populate_favorites_combo(
                combo_favorites,
                &settings.favorite_voices,
                settings.tts_engine,
                &settings.tts_voice,
                &labels,
            );
        }
    }
}

fn refresh_voice_panel_voice_list(hwnd: HWND) {
    unsafe {
        let (voice_visible, combo_voice, checkbox_multilingual, label_language, combo_language) =
            match with_state(hwnd, |state| {
                (
                    state.voice_panel_visible,
                    state.voice_combo_voice,
                    state.voice_checkbox_multilingual,
                    state.voice_label_language,
                    state.voice_combo_language,
                )
            }) {
                Some(values) => values,
                None => return,
            };
        if !voice_visible || combo_voice.0 == 0 {
            return;
        }

        let settings = with_state(hwnd, |state| state.settings.clone()).unwrap_or_default();
        let labels = voice_panel_labels(settings.language);
        let is_edge = matches!(settings.tts_engine, TtsEngine::Edge);
        SendMessageW(
            checkbox_multilingual,
            BM_SETCHECK,
            WPARAM(if settings.tts_only_multilingual {
                BST_CHECKED.0 as usize
            } else {
                0
            }),
            LPARAM(0),
        );
        EnableWindow(checkbox_multilingual, is_edge);
        let multi_show = if is_edge { SW_SHOW } else { SW_HIDE };
        ShowWindow(checkbox_multilingual, multi_show);
        let show_language_combo = is_edge && !settings.tts_only_multilingual;
        ShowWindow(
            label_language,
            if show_language_combo {
                SW_SHOW
            } else {
                SW_HIDE
            },
        );
        ShowWindow(
            combo_language,
            if show_language_combo {
                SW_SHOW
            } else {
                SW_HIDE
            },
        );
        EnableWindow(combo_language, show_language_combo);

        let voices: Vec<crate::settings::VoiceInfo> =
            with_state(hwnd, |state| match settings.tts_engine {
                TtsEngine::Edge => state.edge_voices.clone(),
                TtsEngine::Google => crate::google_tts::installed_voices(),
                TtsEngine::Sapi5 => state.sapi_voices.clone(),
                TtsEngine::Sapi4 => crate::sapi4_engine::get_voices(),
            })
            .unwrap_or_default();
        let mut language_filter: Option<String> = None;
        if show_language_combo {
            let previous_selection = with_state(hwnd, |state| {
                let sel = SendMessageW(combo_language, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
                if sel >= 0 {
                    state.voice_language_codes.get(sel as usize).cloned()
                } else {
                    None
                }
            })
            .flatten();
            let mut codes = collect_voice_language_codes(&voices);
            if !codes.is_empty() {
                let selected_from_voice = voices
                    .iter()
                    .find(|v| v.short_name == settings.tts_voice)
                    .and_then(|v| voice_locale_language_code(&v.locale));
                let selected_code = previous_selection
                    .filter(|code| codes.contains(code))
                    .or(selected_from_voice.filter(|code| codes.contains(code)))
                    .unwrap_or_else(|| codes[0].clone());
                SendMessageW(combo_language, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
                let mut selected_index: Option<usize> = None;
                for (idx, code) in codes.iter().enumerate() {
                    let label = localized_voice_language_name(settings.language, code);
                    let added = SendMessageW(
                        combo_language,
                        CB_ADDSTRING,
                        WPARAM(0),
                        LPARAM(to_wide(&label).as_ptr() as isize),
                    )
                    .0;
                    if added >= 0 && *code == selected_code {
                        selected_index = Some(idx);
                    }
                }
                SendMessageW(
                    combo_language,
                    CB_SETCURSEL,
                    WPARAM(selected_index.unwrap_or(0)),
                    LPARAM(0),
                );
                language_filter = Some(selected_code);
            }
            if with_state(hwnd, |state| {
                state.voice_language_codes = std::mem::take(&mut codes);
            })
            .is_none()
            {
                log_debug("Failed to update voice language codes");
            }
        } else {
            SendMessageW(combo_language, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
            if with_state(hwnd, |state| state.voice_language_codes.clear()).is_none() {
                log_debug("Failed to clear voice language codes");
            }
        }
        populate_voice_panel_combo(
            combo_voice,
            settings.tts_engine,
            &voices,
            &settings.tts_voice,
            settings.tts_engine == TtsEngine::Edge && settings.tts_only_multilingual,
            language_filter.as_deref(),
            &labels.voices_empty,
        );
    }
}

fn clear_voice_labels_if_hidden(hwnd: HWND) {
    unsafe {
        let (
            voice_visible,
            favorites_visible,
            label_engine,
            label_language,
            label_voice,
            label_speed,
            label_pitch,
            label_volume,
            checkbox_multilingual,
            label_favorites,
        ) = match with_state(hwnd, |state| {
            (
                state.voice_panel_visible,
                state.voice_favorites_visible,
                state.voice_label_engine,
                state.voice_label_language,
                state.voice_label_voice,
                state.voice_label_speed,
                state.voice_label_pitch,
                state.voice_label_volume,
                state.voice_checkbox_multilingual,
                state.voice_label_favorites,
            )
        }) {
            Some(values) => values,
            None => return,
        };
        if voice_visible || favorites_visible {
            return;
        }
        let empty = to_wide("");
        crate::log_if_err!(SetWindowTextW(label_engine, PCWSTR(empty.as_ptr())));
        crate::log_if_err!(SetWindowTextW(label_language, PCWSTR(empty.as_ptr())));
        crate::log_if_err!(SetWindowTextW(label_voice, PCWSTR(empty.as_ptr())));
        crate::log_if_err!(SetWindowTextW(label_speed, PCWSTR(empty.as_ptr())));
        crate::log_if_err!(SetWindowTextW(label_pitch, PCWSTR(empty.as_ptr())));
        crate::log_if_err!(SetWindowTextW(label_volume, PCWSTR(empty.as_ptr())));
        crate::log_if_err!(SetWindowTextW(
            checkbox_multilingual,
            PCWSTR(empty.as_ptr())
        ));
        crate::log_if_err!(SetWindowTextW(label_favorites, PCWSTR(empty.as_ptr())));
    }
}

fn populate_voice_panel_combo(
    combo_voice: HWND,
    engine: TtsEngine,
    voices: &[VoiceInfo],
    selected: &str,
    only_multilingual: bool,
    language_filter: Option<&str>,
    empty_label: &str,
) {
    unsafe {
        SendMessageW(combo_voice, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        if voices.is_empty() {
            SendMessageW(
                combo_voice,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(empty_label).as_ptr() as isize),
            );
            SendMessageW(combo_voice, CB_SETCURSEL, WPARAM(0), LPARAM(0));
            return;
        }
        let mut selected_index: Option<usize> = None;
        let mut combo_index = 0usize;

        for (voice_index, voice) in voices.iter().enumerate() {
            if only_multilingual && !voice.is_multilingual {
                continue;
            }
            if let Some(filter) = language_filter {
                let Some(code) = voice_locale_language_code(&voice.locale) else {
                    continue;
                };
                if code != filter {
                    continue;
                }
            }
            let voice_name = if engine == TtsEngine::Google {
                crate::google_tts::voice_display_name(&voice.short_name)
            } else {
                voice.short_name.clone()
            };
            let label = format!("{} ({})", voice_name, voice.locale);
            let idx = SendMessageW(
                combo_voice,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&label).as_ptr() as isize),
            )
            .0;
            if idx >= 0 {
                SendMessageW(
                    combo_voice,
                    CB_SETITEMDATA,
                    WPARAM(idx as usize),
                    LPARAM(voice_index as isize),
                );
                if voice.short_name == selected {
                    selected_index = Some(combo_index);
                }
                combo_index += 1;
            }
        }

        if let Some(idx) = selected_index {
            SendMessageW(combo_voice, CB_SETCURSEL, WPARAM(idx), LPARAM(0));
        } else if combo_index > 0 {
            SendMessageW(combo_voice, CB_SETCURSEL, WPARAM(0), LPARAM(0));
        }
    }
}

fn populate_favorites_combo(
    combo_favorites: HWND,
    favorites: &[FavoriteVoice],
    selected_engine: TtsEngine,
    selected_voice: &str,
    labels: &VoicePanelLabels,
) {
    unsafe {
        SendMessageW(combo_favorites, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        if favorites.is_empty() {
            SendMessageW(
                combo_favorites,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&labels.favorites_empty).as_ptr() as isize),
            );
            SendMessageW(combo_favorites, CB_SETCURSEL, WPARAM(0), LPARAM(0));
            return;
        }
        let mut selected_index: Option<usize> = None;
        for (idx, fav) in favorites.iter().enumerate() {
            let engine_label = match fav.engine {
                TtsEngine::Edge => &labels.engine_edge,
                TtsEngine::Sapi5 => &labels.engine_sapi,
                TtsEngine::Sapi4 => &labels.engine_sapi,
                TtsEngine::Google => &labels.engine_google,
            };
            let favorite_name = if fav.engine == TtsEngine::Google {
                crate::google_tts::voice_display_name(&fav.short_name)
            } else {
                fav.short_name.clone()
            };
            let label = format!("{} ({})", favorite_name, engine_label);
            let cb_idx = SendMessageW(
                combo_favorites,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(&label).as_ptr() as isize),
            )
            .0;
            if cb_idx >= 0 {
                SendMessageW(
                    combo_favorites,
                    CB_SETITEMDATA,
                    WPARAM(cb_idx as usize),
                    LPARAM(idx as isize),
                );
                if fav.short_name == selected_voice && fav.engine == selected_engine {
                    selected_index = Some(cb_idx as usize);
                }
            }
        }
        if let Some(idx) = selected_index {
            SendMessageW(combo_favorites, CB_SETCURSEL, WPARAM(idx), LPARAM(0));
        } else {
            SendMessageW(combo_favorites, CB_SETCURSEL, WPARAM(0), LPARAM(0));
        }
    }
}

fn handle_voice_panel_engine_change(hwnd: HWND) {
    unsafe {
        let (combo_engine, language) = match with_state(hwnd, |state| {
            (state.voice_combo_engine, state.settings.language)
        }) {
            Some(values) => values,
            None => return,
        };
        let sel = SendMessageW(combo_engine, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        let new_engine = match sel {
            1 => TtsEngine::Sapi5,
            2 => TtsEngine::Sapi4,
            3 => TtsEngine::Google,
            _ => TtsEngine::Edge,
        };
        let (old_engine, old_voice) = with_state(hwnd, |state| {
            (state.settings.tts_engine, state.settings.tts_voice.clone())
        })
        .unwrap_or((TtsEngine::Edge, String::new()));
        with_state(hwnd, |state| {
            let old_tuning = TtsTuning::new(
                state.settings.tts_rate,
                state.settings.tts_pitch,
                state.settings.tts_volume,
            );
            set_tts_tuning_for_engine(&mut state.settings, old_engine, old_tuning);
            state.settings.tts_engine = new_engine;
            let new_tuning = clamp_tts_tuning_for_engine(
                new_engine,
                tts_tuning_for_engine(&state.settings, new_engine),
            );
            set_tts_tuning_for_engine(&mut state.settings, new_engine, new_tuning);
            state.settings.tts_rate = new_tuning.rate;
            state.settings.tts_pitch = new_tuning.pitch;
            state.settings.tts_volume = new_tuning.volume;
        });
        app_windows::options_window::ensure_voice_lists_loaded(hwnd, language);
        refresh_voice_panel(hwnd);
        let mut new_voice = old_voice.clone();
        if let Some(voice_name) = current_voice_selection(hwnd, new_engine) {
            with_state(hwnd, |state| {
                state.settings.tts_voice = voice_name.clone();
            });
            new_voice = voice_name;
        }
        let changed = new_engine != old_engine || new_voice != old_voice;
        if changed {
            save_settings_with_synced_active_voice_profile(hwnd);
            restart_tts_from_current_offset(hwnd);
        }
    }
}

fn handle_voice_panel_manage_google_voices(hwnd: HWND) {
    unsafe {
        let (engine, language, combo_voice) = match with_state(hwnd, |state| {
            (
                state.settings.tts_engine,
                state.settings.language,
                state.voice_combo_voice,
            )
        }) {
            Some(values) => values,
            None => return,
        };
        if engine != TtsEngine::Google || combo_voice.0 == 0 {
            return;
        }

        let old_voice =
            with_state(hwnd, |state| state.settings.tts_voice.clone()).unwrap_or_default();
        let manager_font = HFONT(SendMessageW(combo_voice, WM_GETFONT, WPARAM(0), LPARAM(0)).0);
        log_debug("Voice panel: opening Google voice manager");
        crate::app_windows::google_voice_manager_window::open_with_language(
            hwnd,
            language,
            manager_font,
        );
        refresh_google_voice_settings(hwnd);
        let new_voice =
            with_state(hwnd, |state| state.settings.tts_voice.clone()).unwrap_or_default();
        if new_voice != old_voice {
            restart_tts_from_current_offset(hwnd);
        }
        set_focus_safe(combo_voice);
        log_debug("Voice panel: Google voice manager closed; installed voice list refreshed");
    }
}

fn handle_voice_panel_voice_change(hwnd: HWND) {
    {
        let engine = with_state(hwnd, |state| state.settings.tts_engine).unwrap_or_default();
        if let Some(voice_name) = current_voice_selection(hwnd, engine) {
            let old_voice =
                with_state(hwnd, |state| state.settings.tts_voice.clone()).unwrap_or_default();
            if voice_name != old_voice {
                with_state(hwnd, |state| {
                    state.settings.tts_voice = voice_name;
                });
                save_settings_with_synced_active_voice_profile(hwnd);
                restart_tts_from_current_offset(hwnd);
            }
        }
    }
}

fn handle_voice_panel_multilingual_toggle(hwnd: HWND) {
    unsafe {
        let (checkbox, is_edge) = with_state(hwnd, |state| {
            (
                state.voice_checkbox_multilingual,
                matches!(state.settings.tts_engine, TtsEngine::Edge),
            )
        })
        .unwrap_or((HWND(0), false));
        if checkbox.0 == 0 {
            return;
        }
        if !is_edge {
            return;
        }
        let checked =
            SendMessageW(checkbox, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 as u32 == BST_CHECKED.0;
        with_state(hwnd, |state| {
            state.settings.tts_only_multilingual = checked;
        });
        save_settings_with_synced_active_voice_profile(hwnd);
        refresh_voice_panel_voice_list(hwnd);
    }
}

fn insert_voice_tag_from_voice_panel(hwnd: HWND) {
    {
        let (engine, rate, pitch, volume) = with_state(hwnd, |state| {
            (
                state.settings.tts_engine,
                state.settings.tts_rate,
                state.settings.tts_pitch,
                state.settings.tts_volume,
            )
        })
        .unwrap_or_default();
        let voice_name = current_voice_selection(hwnd, engine)
            .or_else(|| with_state(hwnd, |state| Some(state.settings.tts_voice.clone())).flatten())
            .unwrap_or_default();
        if voice_name.trim().is_empty() {
            return;
        }
        crate::editor_manager::insert_voice_tag_at_caret(
            hwnd,
            engine,
            &voice_name,
            rate,
            pitch,
            volume,
        );
        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
        let label = clean_menu_label(&i18n::tr(language, "options.label.insert_voice_tag"));
        if !label.is_empty() {
            with_state(hwnd, |state| state.undo_action_label = Some(label));
        }
    }
}

fn show_pause_tag_menu_from_voice_panel(hwnd: HWND) {
    let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
    if let Some(ms) = choose_pause_tag_milliseconds(hwnd, language) {
        crate::editor_manager::insert_pause_tag_at_caret(hwnd, ms);
        let label = clean_menu_label(&i18n::tr(language, "options.label.insert_pause_tag"));
        if !label.is_empty() {
            with_state(hwnd, |state| state.undo_action_label = Some(label));
        }
    }
}

fn choose_pause_tag_milliseconds(hwnd: HWND, language: Language) -> Option<u32> {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or(HMENU(0));
        if menu.0 == 0 {
            return None;
        }
        for (id, key) in [
            (PAUSE_TAG_MENU_250MS, "pause_tag.menu.250ms"),
            (PAUSE_TAG_MENU_500MS, "pause_tag.menu.500ms"),
            (PAUSE_TAG_MENU_1S, "pause_tag.menu.1s"),
            (PAUSE_TAG_MENU_2S, "pause_tag.menu.2s"),
            (PAUSE_TAG_MENU_CUSTOM, "pause_tag.menu.custom"),
        ] {
            let label = i18n::tr(language, key);
            crate::log_if_err!(AppendMenuW(
                menu,
                MF_STRING,
                id,
                PCWSTR(to_wide(&label).as_ptr()),
            ));
        }

        let mut pt = windows::Win32::Foundation::POINT::default();
        crate::log_if_err!(GetCursorPos(&mut pt));
        SetForegroundWindow(hwnd);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        )
        .0 as usize;
        crate::log_if_err!(PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0)));
        match selected {
            PAUSE_TAG_MENU_250MS => Some(250),
            PAUSE_TAG_MENU_500MS => Some(500),
            PAUSE_TAG_MENU_1S => Some(1000),
            PAUSE_TAG_MENU_2S => Some(2000),
            PAUSE_TAG_MENU_CUSTOM => prompt_custom_pause_milliseconds(hwnd, language),
            _ => None,
        }
    }
}

fn prompt_custom_pause_milliseconds(hwnd: HWND, language: Language) -> Option<u32> {
    let title = i18n::tr(language, "pause_tag.custom.title");
    let prompt = i18n::tr(language, "pause_tag.custom.prompt");
    let value = app_windows::prompt_window::prompt_user(hwnd, &title, &prompt, "1000", language)?;
    let parsed = value.trim().parse::<u32>().ok();
    if let Some(ms) = parsed
        && (tts_engine::PAUSE_TAG_MIN_MS..=tts_engine::PAUSE_TAG_MAX_MS).contains(&ms)
    {
        return Some(ms);
    }
    let min = tts_engine::PAUSE_TAG_MIN_MS.to_string();
    let max = tts_engine::PAUSE_TAG_MAX_MS.to_string();
    let message = i18n::tr_f(
        language,
        "pause_tag.custom.invalid",
        &[("min", &min), ("max", &max)],
    );
    show_error(hwnd, language, &message);
    None
}

fn is_voice_panel_tuning_edit(hwnd: HWND, target: HWND) -> bool {
    if target.0 == 0 {
        return false;
    }
    {
        with_state(hwnd, |state| {
            target == state.voice_edit_speed
                || target == state.voice_edit_pitch
                || target == state.voice_edit_volume
        })
    }
    .unwrap_or(false)
}

fn handle_voice_panel_tuning_combo_change(hwnd: HWND) {
    {
        let (
            combo_speed,
            combo_pitch,
            combo_volume,
            was_active,
            engine,
            old_rate,
            old_pitch,
            old_volume,
        ) = with_state(hwnd, |state| {
            (
                state.voice_combo_speed,
                state.voice_combo_pitch,
                state.voice_combo_volume,
                state.tts_session.is_some(),
                state.settings.tts_engine,
                state.settings.tts_rate,
                state.settings.tts_pitch,
                state.settings.tts_volume,
            )
        })
        .unwrap_or((HWND(0), HWND(0), HWND(0), false, TtsEngine::Edge, 0, 0, 100));
        if combo_speed.0 == 0 || combo_pitch.0 == 0 || combo_volume.0 == 0 {
            return;
        }
        let limits = tts_tuning_limits_for_engine(engine);
        let rate = combo_value(combo_speed).clamp(limits.rate_min, limits.rate_max);
        let pitch = voice_panel_pitch_value_from_combo(combo_pitch, engine);
        let volume = combo_value(combo_volume).clamp(limits.volume_min, limits.volume_max);
        let changed = with_state(hwnd, |state| {
            if state.settings.tts_rate != rate
                || state.settings.tts_pitch != pitch
                || state.settings.tts_volume != volume
            {
                set_active_tts_tuning(
                    &mut state.settings,
                    engine,
                    TtsTuning::new(rate, pitch, volume),
                );
                true
            } else {
                false
            }
        })
        .unwrap_or(false);
        if changed {
            save_settings_with_synced_active_voice_profile(hwnd);
            if was_active && (old_rate != rate || old_pitch != pitch || old_volume != volume) {
                restart_tts_from_current_offset(hwnd);
            }
        }
    }
}

fn handle_voice_panel_tuning_edit_change(hwnd: HWND) {
    {
        let (
            edit_speed,
            edit_pitch,
            edit_volume,
            was_active,
            engine,
            language,
            old_rate,
            old_pitch,
            old_volume,
        ) = with_state(hwnd, |state| {
            (
                state.voice_edit_speed,
                state.voice_edit_pitch,
                state.voice_edit_volume,
                state.tts_session.is_some(),
                state.settings.tts_engine,
                state.settings.language,
                state.settings.tts_rate,
                state.settings.tts_pitch,
                state.settings.tts_volume,
            )
        })
        .unwrap_or((
            HWND(0),
            HWND(0),
            HWND(0),
            false,
            TtsEngine::Edge,
            Language::English,
            0,
            0,
            100,
        ));
        if edit_speed.0 == 0 || edit_pitch.0 == 0 || edit_volume.0 == 0 {
            return;
        }
        let limits = tts_tuning_limits_for_engine(engine);
        let (rate, adjusted_rate) = read_tts_tuning_edit_value_with_clamp(
            edit_speed,
            old_rate,
            limits.rate_min,
            limits.rate_max,
        );
        let (pitch, adjusted_pitch) =
            read_voice_panel_pitch_with_clamp(edit_pitch, engine, old_pitch);
        let (volume, adjusted_volume) = read_tts_edit_value_with_clamp(
            edit_volume,
            old_volume,
            limits.volume_min,
            limits.volume_max,
        );
        if adjusted_rate.is_some() {
            set_tts_tuning_edit_text(edit_speed, rate, true);
        }
        if adjusted_pitch.is_some() {
            set_voice_panel_pitch_edit_text(edit_pitch, engine, pitch);
        }
        if adjusted_volume.is_some() {
            set_tts_tuning_edit_text(edit_volume, volume, false);
        }
        if let Some(value) = adjusted_rate.or(adjusted_pitch).or(adjusted_volume) {
            let message = i18n::tr_f(
                language,
                "tts_tuning.value_clamped",
                &[("value", &value.to_string())],
            );
            log_debug(&format!(
                "Voice panel TTS manual tuning value clamped: {}",
                value
            ));
            let title = "Sonarpad";
            unsafe {
                MessageBoxW(
                    hwnd,
                    PCWSTR(to_wide(&message).as_ptr()),
                    PCWSTR(to_wide(title).as_ptr()),
                    MB_OK | MB_ICONWARNING,
                );
            }
        }
        let changed = with_state(hwnd, |state| {
            if state.settings.tts_rate != rate
                || state.settings.tts_pitch != pitch
                || state.settings.tts_volume != volume
            {
                set_active_tts_tuning(
                    &mut state.settings,
                    engine,
                    TtsTuning::new(rate, pitch, volume),
                );
                true
            } else {
                false
            }
        })
        .unwrap_or(false);
        if changed {
            save_settings_with_synced_active_voice_profile(hwnd);
            if was_active && (old_rate != rate || old_pitch != pitch || old_volume != volume) {
                restart_tts_from_current_offset(hwnd);
            }
        }
    }
}

fn handle_voice_panel_favorite_change(hwnd: HWND) {
    unsafe {
        let (combo_favorites, favorites) = with_state(hwnd, |state| {
            (
                state.voice_combo_favorites,
                state.settings.favorite_voices.clone(),
            )
        })
        .unwrap_or((HWND(0), Vec::new()));
        if combo_favorites.0 == 0 || favorites.is_empty() {
            return;
        }
        let sel = SendMessageW(combo_favorites, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        if sel < 0 {
            return;
        }
        let fav_idx = SendMessageW(
            combo_favorites,
            CB_GETITEMDATA,
            WPARAM(sel as usize),
            LPARAM(0),
        )
        .0 as usize;
        let Some(fav) = favorites.get(fav_idx).cloned() else {
            return;
        };
        let (old_engine, old_voice) = with_state(hwnd, |state| {
            (state.settings.tts_engine, state.settings.tts_voice.clone())
        })
        .unwrap_or((TtsEngine::Edge, String::new()));
        if fav.engine == old_engine && fav.short_name == old_voice {
            return;
        }
        with_state(hwnd, |state| {
            let old_tuning = TtsTuning::new(
                state.settings.tts_rate,
                state.settings.tts_pitch,
                state.settings.tts_volume,
            );
            set_tts_tuning_for_engine(&mut state.settings, old_engine, old_tuning);
            state.settings.tts_engine = fav.engine;
            state.settings.tts_voice = fav.short_name.clone();
            let new_tuning = clamp_tts_tuning_for_engine(
                fav.engine,
                tts_tuning_for_engine(&state.settings, fav.engine),
            );
            set_tts_tuning_for_engine(&mut state.settings, fav.engine, new_tuning);
            state.settings.tts_rate = new_tuning.rate;
            state.settings.tts_pitch = new_tuning.pitch;
            state.settings.tts_volume = new_tuning.volume;
        });
        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
        app_windows::options_window::ensure_voice_lists_loaded(hwnd, language);
        refresh_voice_panel(hwnd);
        save_settings_with_synced_active_voice_profile(hwnd);
        restart_tts_from_current_offset(hwnd);
    }
}

pub(crate) fn refresh_google_voice_settings(hwnd: HWND) {
    let voices = crate::google_tts::installed_voices();
    let first_voice = voices.first().map(|voice| voice.short_name.clone());
    let changed = with_state(hwnd, |state| {
        let mut changed = false;
        let mut ensure_voice = |engine: TtsEngine, selected: &mut String| {
            if engine != TtsEngine::Google {
                return;
            }
            let selected_id = crate::google_tts::voice_id_from_stored(selected);
            let replacement = voices
                .iter()
                .find(|voice| {
                    crate::google_tts::voice_id_from_stored(&voice.short_name) == selected_id
                })
                .map(|voice| voice.short_name.clone())
                .or_else(|| first_voice.clone())
                .unwrap_or_default();
            if *selected != replacement {
                *selected = replacement;
                changed = true;
            }
        };
        ensure_voice(state.settings.tts_engine, &mut state.settings.tts_voice);
        ensure_voice(
            state.settings.dialogue_tts_engine,
            &mut state.settings.dialogue_voice,
        );
        ensure_voice(
            state.settings.dialogue_secondary_tts_engine,
            &mut state.settings.dialogue_secondary_voice,
        );
        changed
    })
    .unwrap_or(false);
    refresh_voice_panel(hwnd);
    if changed {
        save_settings_with_synced_active_voice_profile(hwnd);
    }
}

fn current_voice_selection(hwnd: HWND, engine: TtsEngine) -> Option<String> {
    let (combo_voice, voices) = {
        with_state(hwnd, |state| {
            let list = match engine {
                TtsEngine::Edge => state.edge_voices.clone(),
                TtsEngine::Google => crate::google_tts::installed_voices(),
                TtsEngine::Sapi5 => state.sapi_voices.clone(),
                TtsEngine::Sapi4 => crate::sapi4_engine::get_voices(),
            };
            (state.voice_combo_voice, list)
        })
    }?;
    if voices.is_empty() || combo_voice.0 == 0 {
        return None;
    }
    let sel = crate::send_message_w_safe(combo_voice, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel < 0 {
        return None;
    }
    let voice_index = unsafe {
        SendMessageW(combo_voice, CB_GETITEMDATA, WPARAM(sel as usize), LPARAM(0)).0 as usize
    };
    voices.get(voice_index).map(|v| v.short_name.clone())
}

fn current_favorite_selection(hwnd: HWND) -> Option<FavoriteVoice> {
    let (combo_favorites, favorites) = {
        with_state(hwnd, |state| {
            (
                state.voice_combo_favorites,
                state.settings.favorite_voices.clone(),
            )
        })
    }?;
    if combo_favorites.0 == 0 || favorites.is_empty() {
        return None;
    }
    let sel = crate::send_message_w_safe(combo_favorites, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel < 0 {
        return None;
    }
    let fav_idx = unsafe {
        SendMessageW(
            combo_favorites,
            CB_GETITEMDATA,
            WPARAM(sel as usize),
            LPARAM(0),
        )
        .0 as usize
    };
    favorites.get(fav_idx).cloned()
}

unsafe extern "system" fn voice_combo_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "voice_combo_subclass_proc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || {
                if msg == WM_CONTEXTMENU {
                    let parent = GetParent(hwnd);
                    if parent.0 != 0 {
                        show_voice_context_menu(parent, hwnd, lparam);
                        return LRESULT(0);
                    }
                }
                if msg == WM_KEYDOWN
                    && wparam.0 as u32 == u32::from(VK_F10.0)
                    && GetKeyState(VK_SHIFT.0 as i32) < 0
                {
                    let parent = GetParent(hwnd);
                    if parent.0 != 0 {
                        show_voice_context_menu(parent, hwnd, LPARAM(-1));
                        return LRESULT(0);
                    }
                }

                let parent = GetParent(hwnd);
                let prev_proc = if parent.0 != 0 {
                    with_state(parent, |s| {
                        if hwnd == s.voice_combo_voice {
                            s.voice_combo_voice_proc
                        } else if hwnd == s.voice_combo_favorites {
                            s.voice_combo_favorites_proc
                        } else {
                            None
                        }
                    })
                    .unwrap_or(None)
                } else {
                    None
                };
                if let Some(proc) = prev_proc {
                    CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            },
        )
    }
}

pub(crate) fn restart_tts_from_current_offset(hwnd: HWND) {
    let mut restart = None;
    with_state(hwnd, |state| {
        if let Some(session) = &state.tts_session
            && let Some(doc) = state.docs.get(state.current)
        {
            if matches!(doc.format, FileFormat::Audiobook) {
                return;
            }
            let pos = (session.initial_caret_pos + state.tts_last_offset).max(0);
            restart = Some((doc.hwnd_edit, pos));
        }
    });
    let Some((hwnd_edit, pos)) = restart else {
        return;
    };
    tts_engine::stop_tts_playback(hwnd);
    let pos = adjust_tts_restart_pos(hwnd_edit, pos);
    restart_tts_from_position(hwnd, hwnd_edit, pos);
}

#[derive(Clone, Copy, Debug)]
enum SentenceNavigationDirection {
    Previous,
    Next,
}

fn jump_tts_sentence(hwnd: HWND, direction: SentenceNavigationDirection) {
    let mut navigation = None;
    with_state(hwnd, |state| {
        let Some(doc) = state.docs.get(state.current) else {
            return;
        };
        if matches!(doc.format, FileFormat::Audiobook) {
            return;
        }
        let current_pos = if let Some((anchor_hwnd, anchor_pos)) = state.tts_sentence_nav_anchor {
            if anchor_hwnd == doc.hwnd_edit {
                anchor_pos.max(0)
            } else if let Some(pending) = state.tts_pending_start_pos {
                pending.max(0)
            } else if let Some(session) = &state.tts_session {
                (session.initial_caret_pos + state.tts_last_offset).max(0)
            } else {
                spellcheck_caret_char_index(doc.hwnd_edit).unwrap_or(0)
            }
        } else if let Some(pending) = state.tts_pending_start_pos {
            pending.max(0)
        } else if let Some(session) = &state.tts_session {
            (session.initial_caret_pos + state.tts_last_offset).max(0)
        } else {
            spellcheck_caret_char_index(doc.hwnd_edit).unwrap_or(0)
        };
        navigation = Some((doc.hwnd_edit, current_pos));
    });
    let Some((hwnd_edit, current_pos)) = navigation else {
        return;
    };
    let text = editor_text_for_offsets(hwnd_edit);
    let Some(target_pos) = sentence_navigation_target(&text, current_pos, direction) else {
        with_state(hwnd, |state| {
            state.tts_sentence_nav_anchor = Some((hwnd_edit, current_pos));
        });
        log_debug(&format!(
            "TTS sentence jump: no target direction={:?} current_pos={}",
            direction, current_pos
        ));
        return;
    };
    let current_preview =
        sentence_preview_at_pos(&text, current_pos).unwrap_or_else(|| "(none)".to_string());
    let target_preview =
        sentence_preview_at_pos(&text, target_pos).unwrap_or_else(|| "(none)".to_string());
    tts_engine::stop_tts_playback(hwnd);
    with_state(hwnd, |state| {
        state.tts_last_offset = 0;
        state.tts_pending_start_pos = Some(target_pos);
        state.tts_sentence_nav_anchor = Some((hwnd_edit, target_pos));
    });
    log_debug(&format!(
        "TTS sentence jump: direction={:?} current_pos={} target_pos={} current_preview=\"{}\" target_preview=\"{}\"",
        direction, current_pos, target_pos, current_preview, target_preview
    ));
    restart_tts_from_position(hwnd, hwnd_edit, target_pos);
}

fn sentence_navigation_target(
    text: &str,
    current_pos: i32,
    direction: SentenceNavigationDirection,
) -> Option<i32> {
    const SENTENCE_NAVIGATION_TOLERANCE_UTF16: i32 = 4;

    let starts = sentence_start_offsets_utf16(text);
    if starts.is_empty() {
        return None;
    }
    let current = current_pos.max(0);
    match direction {
        SentenceNavigationDirection::Previous => {
            let effective_current = current.saturating_add(SENTENCE_NAVIGATION_TOLERANCE_UTF16);
            let current_index = starts
                .partition_point(|start| *start <= effective_current)
                .saturating_sub(1);
            if current_index == 0 {
                None
            } else {
                starts.get(current_index - 1).copied()
            }
        }
        SentenceNavigationDirection::Next => {
            let effective_current = current.saturating_add(SENTENCE_NAVIGATION_TOLERANCE_UTF16);
            let current_index = starts.partition_point(|start| *start <= effective_current);
            starts.get(current_index).copied()
        }
    }
}

fn sentence_start_offsets_utf16(text: &str) -> Vec<i32> {
    let mut starts = Vec::new();
    let mut offset = 0usize;
    let mut start_of_sentence = true;
    let chars: Vec<char> = text.chars().collect();
    for (index, ch) in chars.iter().copied().enumerate() {
        let start = offset;
        if start_of_sentence && !ch.is_whitespace() {
            starts.push(start as i32);
            start_of_sentence = false;
        }
        offset += ch.len_utf16();
        let prev = index.checked_sub(1).and_then(|i| chars.get(i)).copied();
        let next = chars.get(index + 1).copied();
        if is_sentence_terminator(ch, prev, next) {
            start_of_sentence = true;
        }
    }
    starts
}

fn is_sentence_terminator(ch: char, prev: Option<char>, next: Option<char>) -> bool {
    match ch {
        '!' | '?' => true,
        '.' => {
            if prev.is_some_and(|c| c.is_ascii_digit()) && next.is_some_and(|c| c.is_ascii_digit())
            {
                return false;
            }
            true
        }
        _ => false,
    }
}

fn sentence_preview_at_pos(text: &str, pos: i32) -> Option<String> {
    let starts = sentence_start_offsets_utf16(text);
    if starts.is_empty() {
        return None;
    }
    let position = pos.max(0);
    let effective_position = position.saturating_add(4);
    let index = starts.partition_point(|start| *start <= effective_position);
    let sentence_index = index.saturating_sub(1);
    let start = *starts.get(sentence_index)? as usize;
    let end = starts
        .get(sentence_index + 1)
        .copied()
        .unwrap_or(text.encode_utf16().count() as i32)
        .max(0) as usize;
    let snippet = utf16_slice_to_string(text, start, end)
        .trim()
        .replace('\n', " ");
    if snippet.is_empty() {
        None
    } else {
        Some(snippet.chars().take(120).collect())
    }
}

fn utf16_slice_to_string(text: &str, start_utf16: usize, end_utf16: usize) -> String {
    let mut utf16_index = 0usize;
    let mut out = String::new();
    for ch in text.chars() {
        let ch_len = ch.len_utf16();
        let ch_start = utf16_index;
        let ch_end = utf16_index + ch_len;
        if ch_end <= start_utf16 {
            utf16_index = ch_end;
            continue;
        }
        if ch_start >= end_utf16 {
            break;
        }
        out.push(ch);
        utf16_index = ch_end;
    }
    out
}

fn restart_tts_from_position(hwnd: HWND, hwnd_edit: HWND, pos: i32) {
    let mut original_selection = CHARRANGE { cpMin: 0, cpMax: 0 };
    let mut cr = CHARRANGE {
        cpMin: pos,
        cpMax: pos,
    };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut original_selection as *mut _ as isize),
        );
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut cr as *mut _ as isize),
        );
    }
    tts_engine::start_tts_from_caret(hwnd);
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut original_selection as *mut _ as isize),
        );
    }
}

#[repr(C)]
struct RichEditFormatRange {
    hdc: HDC,
    hdc_target: HDC,
    rc: RECT,
    rc_page: RECT,
    chrg: CHARRANGE,
}

fn print_current_document(hwnd: HWND) {
    let (path, hwnd_edit, dirty, language) = with_state(hwnd, |state| {
        state.docs.get(state.current).map(|doc| {
            (
                doc.path.clone(),
                doc.hwnd_edit,
                doc.dirty,
                state.settings.language,
            )
        })
    })
    .flatten()
    .unwrap_or((None, HWND(0), false, Language::default()));
    log_debug(&format!(
        "Print: start hwnd={:?} hwnd_edit={:?} has_path={} dirty={}",
        hwnd,
        hwnd_edit,
        path.is_some(),
        dirty
    ));

    if should_print_with_editor(hwnd_edit, path.as_deref()) {
        match print_current_editor_text(hwnd, hwnd_edit, language) {
            Ok(()) => {
                log_debug("Print: internal RichEdit print completed");
                show_print_message(
                    hwnd,
                    language,
                    &i18n::tr(language, "print.completed"),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
            Err(err) => {
                log_debug(&format!("Print: internal RichEdit print failed err={err}"));
                show_print_message(
                    hwnd,
                    language,
                    &i18n::tr_f(language, "print.document_text_error", &[("error", &err)]),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
        return;
    }

    let Some(path) = path else {
        log_debug("Print: abort, no active document path and editor is not printable");
        show_print_message(
            hwnd,
            language,
            &i18n::tr(language, "print.no_active_document"),
            MB_OK | MB_ICONWARNING,
        );
        return;
    };

    if dirty {
        log_debug("Print: non-text document dirty, printing saved file path only");
        screen_reader_speak(&i18n::tr(language, "print.saved_file_warning"));
    }

    match try_print_external_document(hwnd, &path) {
        PrintLaunchResult::Started => {
            log_debug("Print: external launch result started");
            show_print_message(
                hwnd,
                language,
                &i18n::tr(language, "print.completed"),
                MB_OK | MB_ICONINFORMATION,
            );
        }
        PrintLaunchResult::Failed { print_error } => {
            log_debug(&format!(
                "Print: external print failed print_error={print_error}"
            ));
            if hwnd_edit.0 != 0 {
                log_debug("Print: external print failed, trying loaded editor text fallback");
                match print_current_editor_text(hwnd, hwnd_edit, language) {
                    Ok(()) => {
                        log_debug("Print: editor text fallback print completed");
                        show_print_message(
                            hwnd,
                            language,
                            &i18n::tr(language, "print.completed"),
                            MB_OK | MB_ICONINFORMATION,
                        );
                    }
                    Err(err) => {
                        log_debug(&format!("Print: editor text fallback failed err={err}"));
                        show_print_message(
                            hwnd,
                            language,
                            &i18n::tr_f(language, "print.document_text_error", &[("error", &err)]),
                            MB_OK | MB_ICONERROR,
                        );
                    }
                }
            } else {
                show_print_message(
                    hwnd,
                    language,
                    &i18n::tr_f(
                        language,
                        "print.external_direct_error",
                        &[("code", &print_error.to_string())],
                    ),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
    }
}

fn should_print_with_editor(hwnd_edit: HWND, path: Option<&Path>) -> bool {
    if hwnd_edit.0 == 0 {
        return false;
    }

    match path
        .and_then(|p| p.extension())
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
    {
        None => true,
        Some(ext) => matches!(
            ext.as_str(),
            "txt"
                | "log"
                | "md"
                | "markdown"
                | "csv"
                | "json"
                | "xml"
                | "html"
                | "htm"
                | "rs"
                | "py"
                | "js"
                | "ts"
                | "css"
                | "c"
                | "cpp"
                | "h"
                | "hpp"
                | "java"
                | "cs"
                | "php"
                | "ini"
                | "toml"
                | "yaml"
                | "yml"
        ),
    }
}

fn print_current_editor_text(
    hwnd: HWND,
    hwnd_edit: HWND,
    language: Language,
) -> Result<(), String> {
    if hwnd_edit.0 == 0 {
        return Err(i18n::tr(language, "print.no_active_editor"));
    }

    let window_text_len =
        send_message_w_safe(hwnd_edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as i32;
    let text = editor_manager::get_text_range(
        hwnd_edit,
        CHARRANGE {
            cpMin: 0,
            cpMax: window_text_len,
        },
    );
    let text_len = text.encode_utf16().count() as i32;
    log_debug(&format!(
        "Print: RichEdit text_len={text_len} window_text_len={window_text_len}"
    ));
    if text_len <= 0 {
        return Err(i18n::tr(language, "print.empty_document"));
    }

    let mut print_dialog = PRINTDLGW {
        lStructSize: size_of::<PRINTDLGW>() as u32,
        hwndOwner: hwnd,
        Flags: PD_RETURNDC | PD_NOSELECTION | PD_NOPAGENUMS | PD_USEDEVMODECOPIESANDCOLLATE,
        ..Default::default()
    };

    // SAFETY: PRINTDLGW is initialized with its documented size and owner hwnd.
    // The dialog fills the structure synchronously before returning.
    let dialog_ok = unsafe { PrintDlgW(&mut print_dialog).as_bool() };
    log_debug(&format!("Print: PrintDlgW ok={dialog_ok}"));
    if !dialog_ok {
        return Err(i18n::tr(language, "print.cancelled_or_no_printer"));
    }

    let hdc = print_dialog.hDC;
    if hdc.0 == 0 {
        free_print_dialog_handles(&print_dialog);
        return Err(i18n::tr(language, "print.no_printer_context"));
    }

    let result = print_rich_edit_to_hdc(hwnd_edit, hdc, text_len, language);

    // SAFETY: hdc comes from PrintDlgW with PD_RETURNDC and is owned by this function.
    let deleted_dc = unsafe { DeleteDC(hdc).as_bool() };
    if !deleted_dc {
        log_debug(&format!(
            "Print: DeleteDC failed after print dialog: {}",
            unsafe { GetLastError().0 }
        ));
    }
    free_print_dialog_handles(&print_dialog);

    result
}

fn print_rich_edit_to_hdc(
    hwnd_edit: HWND,
    hdc: HDC,
    text_len: i32,
    language: Language,
) -> Result<(), String> {
    // SAFETY: hdc is a printer DC returned by PrintDlgW and remains valid for this print job.
    let log_pixels_x = unsafe { GetDeviceCaps(hdc, LOGPIXELSX) };
    // SAFETY: hdc is a printer DC returned by PrintDlgW and remains valid for this print job.
    let log_pixels_y = unsafe { GetDeviceCaps(hdc, LOGPIXELSY) };
    // SAFETY: hdc is a printer DC returned by PrintDlgW and remains valid for this print job.
    let physical_width = unsafe { GetDeviceCaps(hdc, PHYSICALWIDTH) };
    // SAFETY: hdc is a printer DC returned by PrintDlgW and remains valid for this print job.
    let physical_height = unsafe { GetDeviceCaps(hdc, PHYSICALHEIGHT) };
    // SAFETY: hdc is a printer DC returned by PrintDlgW and remains valid for this print job.
    let offset_x = unsafe { GetDeviceCaps(hdc, PHYSICALOFFSETX) };
    // SAFETY: hdc is a printer DC returned by PrintDlgW and remains valid for this print job.
    let offset_y = unsafe { GetDeviceCaps(hdc, PHYSICALOFFSETY) };
    // SAFETY: hdc is a printer DC returned by PrintDlgW and remains valid for this print job.
    let horz_res = unsafe { GetDeviceCaps(hdc, HORZRES) };
    // SAFETY: hdc is a printer DC returned by PrintDlgW and remains valid for this print job.
    let vert_res = unsafe { GetDeviceCaps(hdc, windows::Win32::Graphics::Gdi::VERTRES) };

    log_debug(&format!(
        "Print: device caps dpi=({log_pixels_x},{log_pixels_y}) physical=({physical_width},{physical_height}) offset=({offset_x},{offset_y}) res=({horz_res},{vert_res})"
    ));

    if log_pixels_x <= 0 || log_pixels_y <= 0 || physical_width <= 0 || physical_height <= 0 {
        return Err(i18n::tr(language, "print.invalid_printer_parameters"));
    }

    let to_twips_x = |value: i32| value.saturating_mul(TWIPS_PER_INCH) / log_pixels_x;
    let to_twips_y = |value: i32| value.saturating_mul(TWIPS_PER_INCH) / log_pixels_y;

    let rc_page = RECT {
        left: 0,
        top: 0,
        right: to_twips_x(physical_width),
        bottom: to_twips_y(physical_height),
    };

    let mut rc = RECT {
        left: to_twips_x(offset_x).saturating_add(PRINT_MARGIN_TWIPS),
        top: to_twips_y(offset_y).saturating_add(PRINT_MARGIN_TWIPS),
        right: to_twips_x(offset_x.saturating_add(horz_res)).saturating_sub(PRINT_MARGIN_TWIPS),
        bottom: to_twips_y(offset_y.saturating_add(vert_res)).saturating_sub(PRINT_MARGIN_TWIPS),
    };

    if rc.right <= rc.left || rc.bottom <= rc.top {
        log_debug("Print: margins too large, using printable area without extra margins");
        rc = RECT {
            left: to_twips_x(offset_x),
            top: to_twips_y(offset_y),
            right: to_twips_x(offset_x.saturating_add(horz_res)),
            bottom: to_twips_y(offset_y.saturating_add(vert_res)),
        };
    }
    log_debug(&format!(
        "Print: format rect page=({},{}-{},{}), content=({},{}-{},{}), text_len={}",
        rc_page.left,
        rc_page.top,
        rc_page.right,
        rc_page.bottom,
        rc.left,
        rc.top,
        rc.right,
        rc.bottom,
        text_len
    ));

    let doc_name = to_wide("Sonarpad");
    let doc_info = DOCINFOW {
        cbSize: size_of::<DOCINFOW>() as i32,
        lpszDocName: PCWSTR(doc_name.as_ptr()),
        ..Default::default()
    };

    // SAFETY: hdc is a valid printer DC and doc_info points to live UTF-16 data for the call.
    let start_doc = unsafe { StartDocW(hdc, &doc_info) };
    log_debug(&format!("Print: StartDocW result={start_doc}"));
    if start_doc <= 0 {
        let error = unsafe { GetLastError().0 }.to_string();
        return Err(i18n::tr_f(
            language,
            "print.start_doc_failed",
            &[("error", &error)],
        ));
    }

    let mut next_char = 0i32;
    let mut page = 0i32;
    let mut ok = true;
    let mut err = String::new();

    while next_char < text_len {
        page += 1;
        // SAFETY: hdc is inside an active StartDocW/EndDoc print job.
        let start_page = unsafe { StartPage(hdc) };
        log_debug(&format!(
            "Print: StartPage page={page} result={start_page} next_char={next_char}"
        ));
        if start_page <= 0 {
            ok = false;
            let error = unsafe { GetLastError().0 }.to_string();
            err = i18n::tr_f(language, "print.start_page_failed", &[("error", &error)]);
            break;
        }

        let mut format_range = RichEditFormatRange {
            hdc,
            hdc_target: hdc,
            rc,
            rc_page,
            chrg: CHARRANGE {
                cpMin: next_char,
                cpMax: text_len,
            },
        };

        let formatted_until = send_message_w_safe(
            hwnd_edit,
            EM_FORMATRANGE,
            WPARAM(1),
            LPARAM(&mut format_range as *mut _ as isize),
        )
        .0 as i32;
        log_debug(&format!(
            "Print: EM_FORMATRANGE page={page} formatted_until={formatted_until}"
        ));

        // SAFETY: hdc is inside an active StartPage/EndPage pair.
        let end_page = unsafe { EndPage(hdc) };
        log_debug(&format!("Print: EndPage page={page} result={end_page}"));
        if end_page <= 0 {
            ok = false;
            let error = unsafe { GetLastError().0 }.to_string();
            err = i18n::tr_f(language, "print.end_page_failed", &[("error", &error)]);
            break;
        }

        if formatted_until >= text_len.saturating_sub(1) {
            next_char = text_len;
            continue;
        }

        if formatted_until <= next_char {
            ok = false;
            err = i18n::tr(language, "print.richedit_no_progress");
            break;
        }
        next_char = formatted_until;
    }

    send_message_w_safe(hwnd_edit, EM_FORMATRANGE, WPARAM(0), LPARAM(0));
    // SAFETY: hdc is inside an active StartDocW/EndDoc print job.
    let end_doc = unsafe { EndDoc(hdc) };
    log_debug(&format!("Print: EndDoc result={end_doc} ok={ok}"));

    if !ok {
        return Err(err);
    }
    if end_doc <= 0 {
        let error = unsafe { GetLastError().0 }.to_string();
        return Err(i18n::tr_f(
            language,
            "print.end_doc_failed",
            &[("error", &error)],
        ));
    }
    Ok(())
}

fn free_print_dialog_handles(print_dialog: &PRINTDLGW) {
    unsafe {
        if !print_dialog.hDevMode.0.is_null()
            && let Err(err) = GlobalFree(print_dialog.hDevMode)
        {
            log_debug(&format!("Print: GlobalFree hDevMode failed: {err}"));
        }
        if !print_dialog.hDevNames.0.is_null()
            && let Err(err) = GlobalFree(print_dialog.hDevNames)
        {
            log_debug(&format!("Print: GlobalFree hDevNames failed: {err}"));
        }
    }
}

enum PrintLaunchResult {
    Started,
    Failed { print_error: isize },
}

fn try_print_external_document(hwnd: HWND, path: &Path) -> PrintLaunchResult {
    log_debug(&format!(
        "Print: try external shell print path={}",
        path.display()
    ));
    let print_result = shell_execute_path(hwnd, w!("print"), path, None, SW_HIDE);
    log_debug(&format!(
        "Print: ShellExecute verb=print result={print_result}"
    ));
    if print_result > 32 {
        return PrintLaunchResult::Started;
    }

    if let Some(printer_name) = get_default_printer_name() {
        log_debug(&format!("Print: default printer found name={printer_name}"));
        let printer_param = format!("\"{printer_name}\"");
        let printto_result =
            shell_execute_path(hwnd, w!("printto"), path, Some(&printer_param), SW_HIDE);
        log_debug(&format!(
            "Print: ShellExecute verb=printto result={printto_result}"
        ));
        if printto_result > 32 {
            return PrintLaunchResult::Started;
        }
    } else {
        log_debug("Print: no default printer name available");
    }

    PrintLaunchResult::Failed {
        print_error: print_result,
    }
}

fn shell_execute_path(
    hwnd: HWND,
    verb: PCWSTR,
    path: &Path,
    parameters: Option<&str>,
    show: windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD,
) -> isize {
    let path_wide = to_wide(&path.to_string_lossy());
    let parameters_wide = parameters.map(to_wide);
    let parameters_ptr = parameters_wide
        .as_ref()
        .map(|wide| PCWSTR(wide.as_ptr()))
        .unwrap_or_else(PCWSTR::null);

    unsafe {
        ShellExecuteW(
            hwnd,
            verb,
            PCWSTR(path_wide.as_ptr()),
            parameters_ptr,
            PCWSTR::null(),
            show,
        )
        .0 as isize
    }
}

fn get_default_printer_name() -> Option<String> {
    let mut needed = 0u32;
    let first_probe_ok = unsafe { GetDefaultPrinterW(PWSTR::null(), &mut needed).as_bool() };
    log_debug(&format!(
        "Print: GetDefaultPrinter probe ok={} needed={}",
        first_probe_ok, needed
    ));
    if needed == 0 {
        log_debug("Print: GetDefaultPrinter probe returned zero buffer size");
        return None;
    }

    let mut buffer = vec![0u16; needed as usize];
    let ok = unsafe { GetDefaultPrinterW(PWSTR(buffer.as_mut_ptr()), &mut needed).as_bool() };
    log_debug(&format!(
        "Print: GetDefaultPrinter read ok={} final_needed={}",
        ok, needed
    ));
    if !ok {
        return None;
    }

    let end = buffer
        .iter()
        .position(|ch| *ch == 0)
        .unwrap_or(buffer.len());
    if end == 0 {
        log_debug("Print: default printer buffer was empty");
        None
    } else {
        let printer_name = String::from_utf16_lossy(&buffer[..end]);
        log_debug(&format!(
            "Print: default printer decoded chars={}",
            printer_name.chars().count()
        ));
        Some(printer_name)
    }
}

fn show_print_message(hwnd: HWND, language: Language, message: &str, flags: MESSAGEBOX_STYLE) {
    log_debug(&format!(
        "Print: showing message flags={:#x} chars={}",
        flags.0,
        message.chars().count()
    ));
    let message_wide = to_wide(message);
    let title_wide = to_wide(&i18n::tr(language, "print.title"));
    message_box_modal(
        hwnd,
        PCWSTR(message_wide.as_ptr()),
        PCWSTR(title_wide.as_ptr()),
        flags,
    );
}

fn adjust_tts_restart_pos(hwnd_edit: HWND, pos: i32) -> i32 {
    if pos <= 0 {
        return 0;
    }
    let text = editor_text_for_offsets(hwnd_edit);
    if text.is_empty() {
        return pos;
    }
    let mut items: Vec<(usize, usize, bool)> = Vec::new();
    let mut offset = 0usize;
    for ch in text.chars() {
        let start = offset;
        let len = ch.len_utf16();
        let end = start + len;
        let is_word = ch.is_alphanumeric() || ch == '_';
        items.push((start, end, is_word));
        offset = end;
    }
    if offset == 0 {
        return pos;
    }
    let mut pos_usize = pos as usize;
    if pos_usize > offset {
        pos_usize = offset;
    }

    let mut prev: Option<usize> = None;
    let mut next: Option<usize> = None;
    for (idx, (start, end, _)) in items.iter().enumerate() {
        if *end <= pos_usize {
            prev = Some(idx);
            continue;
        }
        if *start >= pos_usize {
            next = Some(idx);
            break;
        }
        next = Some(idx);
        break;
    }

    let prev_is_word = prev
        .and_then(|idx| items.get(idx))
        .map(|v| v.2)
        .unwrap_or(false);
    let next_is_word = next
        .and_then(|idx| items.get(idx))
        .map(|v| v.2)
        .unwrap_or(false);
    if prev_is_word
        && next_is_word
        && let Some(mut idx) = prev
    {
        while idx > 0 && items[idx - 1].2 {
            idx -= 1;
        }
        return items[idx].0 as i32;
    }
    pos
}

fn editor_text_for_offsets(hwnd_edit: HWND) -> String {
    let total_len =
        unsafe { SendMessageW(hwnd_edit, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 } as i32;
    if total_len <= 0 {
        return String::new();
    }
    editor_manager::get_text_range(
        hwnd_edit,
        CHARRANGE {
            cpMin: 0,
            cpMax: total_len,
        },
    )
}

fn spellcheck_caret_char_index(hwnd_edit: HWND) -> Option<i32> {
    let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
    }
    if selection.cpMin < 0 {
        None
    } else {
        Some(selection.cpMin)
    }
}

fn spellcheck_char_index_from_lparam(hwnd_edit: HWND, lparam: LPARAM) -> Option<i32> {
    if lparam.0 == -1 {
        return spellcheck_caret_char_index(hwnd_edit);
    }
    let x = (lparam.0 & 0xffff) as i32;
    let y = ((lparam.0 >> 16) & 0xffff) as i32;
    if x == -1 && y == -1 {
        return spellcheck_caret_char_index(hwnd_edit);
    }
    let mut pt = POINT { x, y };
    if !unsafe { ScreenToClient(hwnd_edit, &mut pt).as_bool() } {
        crate::log_debug("ScreenToClient failed");
    }
    let res = unsafe {
        SendMessageW(
            hwnd_edit,
            EM_CHARFROMPOS,
            WPARAM(0),
            LPARAM(&pt as *const _ as isize),
        )
        .0 as i32
    };
    if res < 0 { None } else { Some(res) }
}

fn spellcheck_line_info(hwnd_edit: HWND, char_index: i32) -> Option<(i32, i32, String)> {
    if char_index < 0 {
        return None;
    }
    let line_index = unsafe {
        SendMessageW(
            hwnd_edit,
            EM_LINEFROMCHAR,
            WPARAM(char_index as usize),
            LPARAM(0),
        )
        .0 as i32
    };
    if line_index < 0 {
        return None;
    }
    let line_start = unsafe {
        SendMessageW(
            hwnd_edit,
            EM_LINEINDEX,
            WPARAM(line_index as usize),
            LPARAM(0),
        )
        .0 as i32
    };
    if line_start < 0 {
        return None;
    }
    let line_len = unsafe {
        SendMessageW(
            hwnd_edit,
            EM_LINELENGTH,
            WPARAM(line_start as usize),
            LPARAM(0),
        )
        .0 as i32
    };
    if line_len <= 0 {
        return Some((line_index, line_start, String::new()));
    }
    let mut buf = vec![0u16; (line_len + 1) as usize];
    let mut range = TEXTRANGEW {
        chrg: CHARRANGE {
            cpMin: line_start,
            cpMax: line_start + line_len,
        },
        lpstrText: PWSTR(buf.as_mut_ptr()),
    };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_GETTEXTRANGE,
            WPARAM(0),
            LPARAM(&mut range as *mut _ as isize),
        );
    }
    let line_len = line_len.max(0) as usize;
    let line_text = String::from_utf16_lossy(&buf[..line_len]);
    Some((line_index, line_start, line_text))
}

fn spellcheck_word_context_from_char_index(
    hwnd_edit: HWND,
    char_index: i32,
) -> Option<SpellcheckWordContext> {
    let (line_index, line_start, line_text) = spellcheck_line_info(hwnd_edit, char_index)?;
    if line_text.is_empty() {
        return None;
    }
    let offset_utf16 = (char_index - line_start).max(0) as u32;
    let caret_byte = spellcheck::utf16_offset_to_utf8_byte_offset(&line_text, offset_utf16);
    let word_range = spellcheck::word_range_at(&line_text, caret_byte)?;
    let word = line_text
        .get(word_range.0..word_range.1)
        .unwrap_or("")
        .to_string();
    if word.is_empty() {
        return None;
    }
    let line_hash = spellcheck::hash_line(&line_text);
    Some(SpellcheckWordContext {
        doc_id: hwnd_edit.0,
        line_index,
        line_start,
        line_text,
        line_hash,
        word_range,
        word,
    })
}

fn spellcheck_word_context_from_lparam(
    hwnd_edit: HWND,
    lparam: LPARAM,
) -> Option<SpellcheckWordContext> {
    let char_index = spellcheck_char_index_from_lparam(hwnd_edit, lparam)?;
    spellcheck_word_context_from_char_index(hwnd_edit, char_index)
}

fn handle_spellcheck_selection_change(hwnd: HWND, hwnd_edit: HWND) {
    let announce_allowed = {
        with_state(hwnd, |state| {
            if state.spellcheck_space_trigger == Some(hwnd_edit) {
                // Force re-highlight of current line when space/punctuation is pressed.
                state.spellcheck_last_highlighted_line = None;
                state.spellcheck_space_trigger = None;
                state.spellcheck_typing_in_progress = false;
                return true;
            }
            !state.spellcheck_typing_in_progress
        })
    }
    .unwrap_or(false);
    let Some(caret_index) = spellcheck_caret_char_index(hwnd_edit) else {
        {
            with_state(hwnd, |state| state.spellcheck_last_announce = None);
        }
        return;
    };
    let Some(word_ctx) = spellcheck_word_context_from_char_index(hwnd_edit, caret_index) else {
        {
            with_state(hwnd, |state| state.spellcheck_last_announce = None);
        }
        return;
    };

    let (announce_msg, fallback_msg) = {
        with_state(hwnd, |state| {
            let settings = &state.settings;
            let Some(resolution) = state.spellcheck_manager.resolve_language(settings) else {
                state.spellcheck_last_announce = None;
                return (None, None);
            };
            let language_ui = settings.language;
            let fallback_msg = if resolution.announce_fallback {
                Some(i18n::tr_f(
                    language_ui,
                    "spellcheck.language_fallback",
                    &[
                        ("requested", &resolution.requested),
                        ("language", &resolution.effective),
                    ],
                ))
            } else {
                None
            };

            let miss = state.spellcheck_manager.is_word_misspelled(
                word_ctx.doc_id,
                word_ctx.line_index,
                &word_ctx.line_text,
                word_ctx.word_range,
                &resolution.effective,
            );
            if let Some(miss) = miss {
                let key = SpellcheckAnnounceKey {
                    doc_id: word_ctx.doc_id,
                    line_index: word_ctx.line_index,
                    start_utf8: miss.start,
                    end_utf8: miss.end,
                    line_hash: word_ctx.line_hash,
                    language: resolution.effective.clone(),
                };
                if announce_allowed && state.spellcheck_last_announce.as_ref() != Some(&key) {
                    state.spellcheck_last_announce = Some(key);
                    let msg = i18n::tr_f(
                        language_ui,
                        "spellcheck.announce_misspelled",
                        &[("word", &word_ctx.word)],
                    );
                    return (Some(msg), fallback_msg);
                }
                return (None, fallback_msg);
            }
            state.spellcheck_last_announce = None;
            (None, fallback_msg)
        })
    }
    .unwrap_or((None, None));

    if let Some(message) = fallback_msg {
        log_debug(&format!("Spellcheck: {message}"));
        nvda_speak(&message);
    }
    if let Some(message) = announce_msg {
        nvda_speak(&message);
    }
}

/// Triggers the debounced spellcheck highlight timer
fn trigger_spellcheck_highlight(hwnd: HWND, hwnd_edit: HWND) {
    let should_start_timer = {
        with_state(hwnd, |state| {
            if !state.settings.spellcheck_enabled {
                return false;
            }
            state.spellcheck_highlight_pending = Some(hwnd_edit);
            true
        })
    }
    .unwrap_or(false);

    if should_start_timer {
        // Reset/start the debounce timer only if spellcheck is enabled
        unsafe {
            SetTimer(
                hwnd,
                SPELLCHECK_HIGHLIGHT_TIMER_ID,
                SPELLCHECK_HIGHLIGHT_DEBOUNCE_MS,
                None,
            );
        }
    }
}

/// Called when the debounce timer fires - highlights misspellings on current line
fn handle_spellcheck_highlight_timer(hwnd: HWND) {
    let Some(hwnd_edit) =
        ({ with_state(hwnd, |state| state.spellcheck_highlight_pending.take()).flatten() })
    else {
        return;
    };

    // Don't do anything if editor doesn't have focus
    if crate::get_focus_safe() != hwnd_edit {
        return;
    }

    let Some(caret_index) = spellcheck_caret_char_index(hwnd_edit) else {
        return;
    };

    let Some((line_index, line_start, line_text)) = spellcheck_line_info(hwnd_edit, caret_index)
    else {
        return;
    };

    let doc_id = hwnd_edit.0;

    // Check if we're on the same line as before - no need to re-highlight
    let should_highlight = {
        with_state(hwnd, |state| {
            let last = state.spellcheck_last_highlighted_line;
            if last == Some((doc_id, line_index)) {
                return false;
            }
            state.spellcheck_last_highlighted_line = Some((doc_id, line_index));
            true
        })
    }
    .unwrap_or(false);

    if !should_highlight {
        return;
    }

    // Get misspellings for this line
    let misspellings = {
        with_state(hwnd, |state| {
            let settings = &state.settings;
            let resolution = state.spellcheck_manager.resolve_language(settings)?;
            let misses = state.spellcheck_manager.check_line(
                doc_id,
                line_index,
                &line_text,
                &resolution.effective,
            );
            Some((misses, settings.text_color))
        })
    }
    .flatten();

    let Some((misspellings, text_color)) = misspellings else {
        return;
    };

    // First, reset the entire line to normal formatting
    reset_line_formatting(hwnd_edit, line_start, line_text.len(), text_color);

    // Then highlight each misspelled word with red background
    for miss in misspellings {
        let start_utf16 = spellcheck::utf8_byte_offset_to_utf16_units(&line_text, miss.start);
        let end_utf16 = spellcheck::utf8_byte_offset_to_utf16_units(&line_text, miss.end);
        let abs_start = line_start + start_utf16;
        let abs_end = line_start + end_utf16;
        highlight_misspelled_word(hwnd_edit, abs_start, abs_end);
    }
}

/// Resets line formatting to normal (removes red background)
fn reset_line_formatting(hwnd_edit: HWND, line_start: i32, line_len: usize, _text_color: u32) {
    unsafe {
        if line_len == 0 {
            return;
        }

        // Check if this editor still has focus - don't mess with selection if not
        if GetFocus() != hwnd_edit {
            return;
        }

        // Save modified state - formatting changes should not mark document as dirty
        let was_modified = SendMessageW(hwnd_edit, EM_GETMODIFY, WPARAM(0), LPARAM(0)).0 != 0;

        // Disable change notifications during formatting
        SendMessageW(hwnd_edit, EM_SETEVENTMASK, WPARAM(0), LPARAM(0));

        // Lock the control to prevent redraws/scrolling
        SendMessageW(hwnd_edit, WM_SETREDRAW, WPARAM(0), LPARAM(0));

        // Save current selection
        let mut old_sel = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut old_sel as *mut _ as isize),
        );

        // Select the line
        let line_end = line_start + line_len as i32;
        let mut sel = CHARRANGE {
            cpMin: line_start,
            cpMax: line_end,
        };
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut sel as *mut _ as isize),
        );

        // Apply normal formatting (remove background color)
        let mut format = CHARFORMAT2W::default();
        format.Base.cbSize = std::mem::size_of::<CHARFORMAT2W>() as u32;
        format.Base.dwMask = CFM_BACKCOLOR;
        format.Base.dwEffects = CFE_AUTOBACKCOLOR; // Use default background
        SendMessageW(
            hwnd_edit,
            EM_SETCHARFORMAT,
            WPARAM(SCF_SELECTION as usize),
            LPARAM(&mut format as *mut _ as isize),
        );

        // Restore selection
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut old_sel as *mut _ as isize),
        );

        // Unlock redraw
        SendMessageW(hwnd_edit, WM_SETREDRAW, WPARAM(1), LPARAM(0));
        // Force repaint to avoid stale/blank regions after formatting changes.
        if !InvalidateRect(hwnd_edit, None, BOOL(1)).as_bool() {
            crate::log_debug("InvalidateRect failed in reset_line_formatting");
        }

        // Re-enable change notifications
        SendMessageW(
            hwnd_edit,
            EM_SETEVENTMASK,
            WPARAM(0),
            LPARAM((ENM_CHANGE | ENM_SELCHANGE) as isize),
        );

        // Restore modified state
        SendMessageW(
            hwnd_edit,
            EM_SETMODIFY,
            WPARAM(if was_modified { 1 } else { 0 }),
            LPARAM(0),
        );
    }
}

/// Highlights a misspelled word with red background
fn highlight_misspelled_word(hwnd_edit: HWND, start: i32, end: i32) {
    unsafe {
        // Check if this editor still has focus - don't mess with selection if not
        if GetFocus() != hwnd_edit {
            return;
        }

        // Save modified state - formatting changes should not mark document as dirty
        let was_modified = SendMessageW(hwnd_edit, EM_GETMODIFY, WPARAM(0), LPARAM(0)).0 != 0;

        // Disable change notifications during formatting
        SendMessageW(hwnd_edit, EM_SETEVENTMASK, WPARAM(0), LPARAM(0));

        // Lock the control to prevent redraws/scrolling
        SendMessageW(hwnd_edit, WM_SETREDRAW, WPARAM(0), LPARAM(0));

        // Save current selection
        let mut old_sel = CHARRANGE { cpMin: 0, cpMax: 0 };
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut old_sel as *mut _ as isize),
        );

        // Select the misspelled word
        let mut sel = CHARRANGE {
            cpMin: start,
            cpMax: end,
        };
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut sel as *mut _ as isize),
        );

        // Apply red background color
        let mut format = CHARFORMAT2W::default();
        format.Base.cbSize = std::mem::size_of::<CHARFORMAT2W>() as u32;
        format.Base.dwMask = CFM_BACKCOLOR;
        format.crBackColor = windows::Win32::Foundation::COLORREF(0x0000FF); // Red in BGR format
        SendMessageW(
            hwnd_edit,
            EM_SETCHARFORMAT,
            WPARAM(SCF_SELECTION as usize),
            LPARAM(&mut format as *mut _ as isize),
        );

        // Restore selection
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut old_sel as *mut _ as isize),
        );

        // Unlock redraw
        SendMessageW(hwnd_edit, WM_SETREDRAW, WPARAM(1), LPARAM(0));
        // Force repaint to avoid stale/blank regions after formatting changes.
        if !InvalidateRect(hwnd_edit, None, BOOL(1)).as_bool() {
            crate::log_debug("InvalidateRect failed in highlight_misspelled_word");
        }

        // Re-enable change notifications
        SendMessageW(
            hwnd_edit,
            EM_SETEVENTMASK,
            WPARAM(0),
            LPARAM((ENM_CHANGE | ENM_SELCHANGE) as isize),
        );

        // Restore modified state
        SendMessageW(
            hwnd_edit,
            EM_SETMODIFY,
            WPARAM(if was_modified { 1 } else { 0 }),
            LPARAM(0),
        );
    }
}

pub(crate) fn show_editor_context_menu(hwnd: HWND, hwnd_edit: HWND, lparam: LPARAM) {
    unsafe {
        let language_ui = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
        let labels = menu_labels(language_ui);
        let dictionary_pref = with_state(hwnd, |state| {
            state.settings.dictionary_translation_language.clone()
        })
        .unwrap_or_else(|| "auto".to_string());

        let mut spell_status = None;
        let mut spell_context = None;
        let mut fallback_msg = None;

        if let Some(word_ctx) = spellcheck_word_context_from_lparam(hwnd_edit, lparam) {
            let (status, suggestions, language, fallback) = with_state(hwnd, |state| {
                let settings = &state.settings;
                let Some(resolution) = state.spellcheck_manager.resolve_language(settings) else {
                    return (None, Vec::new(), None, None);
                };
                let fallback_msg = if resolution.announce_fallback {
                    Some(i18n::tr_f(
                        settings.language,
                        "spellcheck.language_fallback",
                        &[
                            ("requested", &resolution.requested),
                            ("language", &resolution.effective),
                        ],
                    ))
                } else {
                    None
                };
                let miss = state.spellcheck_manager.is_word_misspelled(
                    word_ctx.doc_id,
                    word_ctx.line_index,
                    &word_ctx.line_text,
                    word_ctx.word_range,
                    &resolution.effective,
                );
                if miss.is_some() {
                    let suggestions = state
                        .spellcheck_manager
                        .suggestions(&word_ctx.word, &resolution.effective);
                    (
                        Some(true),
                        suggestions,
                        Some(resolution.effective.clone()),
                        fallback_msg,
                    )
                } else {
                    (
                        Some(false),
                        Vec::new(),
                        Some(resolution.effective.clone()),
                        fallback_msg,
                    )
                }
            })
            .unwrap_or((None, Vec::new(), None, None));

            spell_status = status;
            fallback_msg = fallback;
            if status == Some(true) {
                let suggestions = suggestions
                    .into_iter()
                    .take(menu::IDM_SPELLCHECK_SUGGESTION_MAX)
                    .collect::<Vec<_>>();
                if let Some(language) = language {
                    spell_context = Some(SpellcheckContextMenuState {
                        hwnd_edit,
                        line_start: word_ctx.line_start,
                        language,
                        word_range: word_ctx.word_range,
                        word: word_ctx.word,
                        line_text: word_ctx.line_text,
                        suggestions,
                    });
                }
            }
        }

        with_state(hwnd, |state| {
            state.spellcheck_context = spell_context.clone();
        });

        if let Some(message) = fallback_msg {
            log_debug(&format!("Spellcheck: {message}"));
            nvda_speak(&message);
        }

        let menu = CreatePopupMenu().unwrap_or(HMENU(0));
        if menu.0 == 0 {
            return;
        }

        if let Some(word_ctx) = spellcheck_word_context_from_lparam(hwnd_edit, lparam)
            && let Ok(submenu) = CreatePopupMenu()
            && submenu.0 != 0
        {
            let placeholder = format!(" {}", i18n::tr(language_ui, "dictionary.menu_expand"));
            crate::log_if_err!(AppendMenuW(
                submenu,
                MF_STRING | MF_GRAYED,
                0,
                PCWSTR(to_wide(&placeholder).as_ptr()),
            ));
            let label = i18n::tr(language_ui, "context_menu.dictionary");
            crate::log_if_err!(AppendMenuW(
                menu,
                MF_POPUP,
                submenu.0 as usize,
                PCWSTR(to_wide(&label).as_ptr()),
            ));
            crate::log_if_err!(AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()));
            let prefetch_info = with_state(hwnd, |state| {
                state.dictionary_context_menu = submenu;
                state.dictionary_context_word = word_ctx.word.clone();
                state.dictionary_context_language = language_ui;
                state.dictionary_context_pref = dictionary_pref.clone();
                state.dictionary_context_loaded = false;
                state.dictionary_prefetch_generation =
                    state.dictionary_prefetch_generation.wrapping_add(1);
                let generation = state.dictionary_prefetch_generation;

                let key = dictionary_cache_key(language_ui, &dictionary_pref, &word_ctx.word);
                if let Some(lines) = state.dictionary_cache.get(&key).cloned() {
                    if is_dictionary_not_found_cache_entry(language_ui, &lines) {
                        state.dictionary_cache.remove(&key);
                        save_dictionary_cache(&state.dictionary_cache);
                    } else {
                        return None;
                    }
                }
                if state.dictionary_pending_lookup.as_ref() == Some(&key) {
                    return None;
                }
                state.dictionary_pending_lookup = Some(key.clone());
                Some((word_ctx.word.clone(), key, generation))
            })
            .flatten();
            if let Some((word, key, generation)) = prefetch_info {
                start_dictionary_lookup(
                    hwnd.0,
                    word,
                    language_ui,
                    dictionary_pref.clone(),
                    key,
                    generation,
                );
            }
        }

        if let Some(status) = spell_status {
            if status {
                let label = i18n::tr(language_ui, "context_menu.spelling_misspelled");
                crate::log_if_err!(AppendMenuW(
                    menu,
                    MF_STRING | MF_GRAYED,
                    0,
                    PCWSTR(to_wide(&label).as_ptr()),
                ));
                if let Ok(submenu) = CreatePopupMenu()
                    && submenu.0 != 0
                {
                    let suggestions = spell_context
                        .as_ref()
                        .map(|ctx| ctx.suggestions.as_slice())
                        .unwrap_or(&[]);
                    if suggestions.is_empty() {
                        let none_label =
                            i18n::tr(language_ui, "context_menu.spelling_no_suggestions");
                        crate::log_if_err!(AppendMenuW(
                            submenu,
                            MF_STRING | MF_GRAYED,
                            0,
                            PCWSTR(to_wide(&none_label).as_ptr()),
                        ));
                    } else {
                        for (idx, suggestion) in suggestions.iter().enumerate() {
                            let id = menu::IDM_SPELLCHECK_SUGGESTION_BASE + idx;
                            crate::log_if_err!(AppendMenuW(
                                submenu,
                                MF_STRING,
                                id,
                                PCWSTR(to_wide(suggestion).as_ptr()),
                            ));
                        }
                    }
                    crate::log_if_err!(AppendMenuW(submenu, MF_SEPARATOR, 0, PCWSTR::null()));
                    let add_label =
                        i18n::tr(language_ui, "context_menu.spelling_add_to_dictionary");
                    let ignore_label = i18n::tr(language_ui, "context_menu.spelling_ignore_once");
                    crate::log_if_err!(AppendMenuW(
                        submenu,
                        MF_STRING,
                        menu::IDM_SPELLCHECK_ADD_TO_DICTIONARY,
                        PCWSTR(to_wide(&add_label).as_ptr()),
                    ));
                    crate::log_if_err!(AppendMenuW(
                        submenu,
                        MF_STRING,
                        menu::IDM_SPELLCHECK_IGNORE_ONCE,
                        PCWSTR(to_wide(&ignore_label).as_ptr()),
                    ));
                    let suggestions_label =
                        i18n::tr(language_ui, "context_menu.spelling_suggestions");
                    crate::log_if_err!(AppendMenuW(
                        menu,
                        MF_POPUP,
                        submenu.0 as usize,
                        PCWSTR(to_wide(&suggestions_label).as_ptr()),
                    ));
                }
            } else {
                let label = i18n::tr(language_ui, "context_menu.spelling_ok");
                crate::log_if_err!(AppendMenuW(
                    menu,
                    MF_STRING | MF_GRAYED,
                    0,
                    PCWSTR(to_wide(&label).as_ptr()),
                ));
            }
            crate::log_if_err!(AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()));
        }

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
        let has_selection = selection.cpMin != selection.cpMax;
        if let Ok(submenu) = CreatePopupMenu()
            && submenu.0 != 0
        {
            let (saved_target, recent_languages) = with_state(hwnd, |state| {
                (
                    state.settings.editor_translate_target_language.clone(),
                    state.settings.editor_translate_recent_languages.clone(),
                )
            })
            .unwrap_or_else(|| (EDITOR_TRANSLATE_LANGUAGES[0].key.to_string(), Vec::new()));
            let saved_target = normalized_editor_translate_target_language(&saved_target);
            for idx in editor_translate_ordered_language_indices(&recent_languages) {
                let translate_language = &EDITOR_TRANSLATE_LANGUAGES[idx];
                let key = format!("options.lang.{}", translate_language.key);
                let label = i18n::tr(language_ui, &key);
                let mut flags = MF_STRING;
                if translate_language.key == saved_target {
                    flags |= MF_CHECKED;
                }
                crate::log_if_err!(AppendMenuW(
                    submenu,
                    flags,
                    menu::IDM_EDIT_TRANSLATE_TARGET_BASE + idx,
                    PCWSTR(to_wide(&label).as_ptr()),
                ));
            }
            let label = i18n::tr(language_ui, "context_menu.translate");
            crate::log_if_err!(AppendMenuW(
                menu,
                MF_POPUP,
                submenu.0 as usize,
                PCWSTR(to_wide(&label).as_ptr()),
            ));
        }
        let has_gemini_key = with_state(hwnd, |state| {
            !state.settings.gemini_api_key.trim().is_empty()
        })
        .unwrap_or(false);
        if has_gemini_key {
            let label = i18n::tr(language_ui, "context_menu.summarize");
            crate::log_if_err!(AppendMenuW(
                menu,
                MF_STRING,
                menu::IDM_EDIT_SUMMARIZE_TEXT,
                PCWSTR(to_wide(&label).as_ptr()),
            ));
        }
        if has_selection {
            let label = i18n::tr(language_ui, "context_menu.audiobook_selection");
            crate::log_if_err!(AppendMenuW(
                menu,
                MF_STRING,
                menu::IDM_EDIT_AUDIOBOOK_SELECTION,
                PCWSTR(to_wide(&label).as_ptr()),
            ));
        }
        crate::log_if_err!(AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()));

        let undo_flags = if can_undo_now(hwnd) {
            MF_STRING
        } else {
            MF_STRING | MF_GRAYED
        };
        let cut_copy_flags = if has_selection {
            MF_STRING
        } else {
            MF_STRING | MF_GRAYED
        };
        let paste_flags = if can_paste_now(hwnd) {
            MF_STRING
        } else {
            MF_STRING | MF_GRAYED
        };
        let undo_label = build_undo_menu_label(hwnd, language_ui);
        crate::log_if_err!(AppendMenuW(
            menu,
            undo_flags,
            IDM_EDIT_UNDO,
            PCWSTR(to_wide(&undo_label).as_ptr()),
        ));
        crate::log_if_err!(AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()));
        crate::log_if_err!(AppendMenuW(
            menu,
            cut_copy_flags,
            IDM_EDIT_CUT,
            PCWSTR(to_wide(&labels.edit_cut).as_ptr()),
        ));
        crate::log_if_err!(AppendMenuW(
            menu,
            cut_copy_flags,
            IDM_EDIT_COPY,
            PCWSTR(to_wide(&labels.edit_copy).as_ptr()),
        ));
        crate::log_if_err!(AppendMenuW(
            menu,
            paste_flags,
            IDM_EDIT_PASTE,
            PCWSTR(to_wide(&labels.edit_paste).as_ptr()),
        ));
        crate::log_if_err!(AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()));
        crate::log_if_err!(AppendMenuW(
            menu,
            MF_STRING,
            IDM_EDIT_SELECT_ALL,
            PCWSTR(to_wide(&labels.edit_select_all).as_ptr()),
        ));

        let mut x = (lparam.0 & 0xffff) as i32;
        let mut y = ((lparam.0 >> 16) & 0xffff) as i32;
        if x == -1 && y == -1 {
            let mut pt = POINT::default();
            crate::log_if_err!(GetCursorPos(&mut pt));
            x = pt.x;
            y = pt.y;
        }
        SetForegroundWindow(hwnd);
        if !TrackPopupMenu(menu, TPM_RIGHTBUTTON, x, y, 0, hwnd, None).as_bool() {
            crate::log_debug("TrackPopupMenu failed");
        }
        crate::log_if_err!(PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0)));
        with_state(hwnd, |state| {
            state.dictionary_context_menu = HMENU(0);
            state.dictionary_context_word.clear();
            state.dictionary_context_pref.clear();
            state.dictionary_context_loaded = false;
            state.dictionary_context_expanded = false;
        });
    }
}

fn editor_translate_language_for_menu_id(
    cmd_id: usize,
) -> Option<&'static EditorTranslateLanguage> {
    let idx = cmd_id.checked_sub(menu::IDM_EDIT_TRANSLATE_TARGET_BASE)?;
    EDITOR_TRANSLATE_LANGUAGES.get(idx)
}

fn editor_translate_language_index_by_key(value: &str) -> Option<usize> {
    let value = value.trim();
    EDITOR_TRANSLATE_LANGUAGES
        .iter()
        .position(|language| language.key.eq_ignore_ascii_case(value))
}

fn editor_translate_ordered_language_indices(recent_languages: &[String]) -> Vec<usize> {
    let mut indices = Vec::with_capacity(EDITOR_TRANSLATE_LANGUAGES.len());
    for language in recent_languages {
        let Some(idx) = editor_translate_language_index_by_key(language) else {
            continue;
        };
        if !indices.contains(&idx) {
            indices.push(idx);
        }
    }
    for idx in 0..EDITOR_TRANSLATE_LANGUAGES.len() {
        if !indices.contains(&idx) {
            indices.push(idx);
        }
    }
    indices
}

fn normalized_editor_translate_target_language(value: &str) -> &'static str {
    let value = value.trim();
    EDITOR_TRANSLATE_LANGUAGES
        .iter()
        .find(|language| language.key.eq_ignore_ascii_case(value))
        .map(|language| language.key)
        .unwrap_or(EDITOR_TRANSLATE_LANGUAGES[0].key)
}

fn record_editor_translate_recent_language(settings: &mut settings::AppSettings, key: &str) {
    let Some(idx) = editor_translate_language_index_by_key(key) else {
        return;
    };
    let key = EDITOR_TRANSLATE_LANGUAGES[idx].key.to_string();
    settings
        .editor_translate_recent_languages
        .retain(|language| !language.eq_ignore_ascii_case(&key));
    settings.editor_translate_recent_languages.insert(0, key);
    settings
        .editor_translate_recent_languages
        .truncate(EDITOR_TRANSLATE_LANGUAGES.len());
}

const DEEPL_FREE_COOLDOWN_SECONDS: u64 = 10 * 60;
static DEEPL_FREE_COOLDOWN_UNTIL: AtomicU64 = AtomicU64::new(0);

fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn deepl_free_cooldown_remaining_seconds() -> u64 {
    let now = unix_timestamp_seconds();
    let until = DEEPL_FREE_COOLDOWN_UNTIL.load(Ordering::Relaxed);
    until.saturating_sub(now)
}

fn mark_deepl_free_cooldown() {
    let until = unix_timestamp_seconds().saturating_add(DEEPL_FREE_COOLDOWN_SECONDS);
    DEEPL_FREE_COOLDOWN_UNTIL.store(until, Ordering::Relaxed);
}

fn is_deepl_too_many_requests_error(err: &translator::TranslatorError) -> bool {
    match err {
        translator::TranslatorError::HttpStatus { status, body } => {
            *status == 429 || body.to_ascii_lowercase().contains("too many requests")
        }
        _ => false,
    }
}

fn line_starts_with_uppercase_letter(line: &str) -> bool {
    line.trim_start()
        .chars()
        .find(|ch| ch.is_alphabetic())
        .is_some_and(|ch| ch.is_uppercase())
}

fn line_looks_like_list_or_heading(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed
        .chars()
        .next()
        .is_some_and(|ch| matches!(ch, '-' | '*' | '•' | '–' | '—'))
    {
        return true;
    }
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return true;
    };
    first.is_ascii_digit() && chars.take(3).any(|ch| matches!(ch, '.' | ')' | ':' | '-'))
}

fn should_join_soft_line_break(previous_line: &str, next_line: &str) -> bool {
    let previous = previous_line.trim_end();
    let next = next_line.trim_start();
    if previous.is_empty() || next.is_empty() {
        return false;
    }
    if line_looks_like_list_or_heading(next) {
        return false;
    }

    if previous.ends_with('-') {
        return true;
    }

    let sentence_ended = previous
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace() && !matches!(ch, '"' | '\'' | ')' | ']' | '}' | '”' | '’'))
        .is_some_and(|ch| matches!(ch, '.' | '!' | '?' | ':' | ';'));
    if !sentence_ended && previous.chars().count() >= 40 {
        return true;
    }

    // TXT/PDF exports often wrap a sentence on a plain newline.
    // The most reliable signal here is whether the next visible line starts
    // with a lowercase continuation. This also fixes lines ending with
    // ellipsis or quotes where punctuation alone is misleading.
    !line_starts_with_uppercase_letter(next)
}

fn normalize_soft_line_breaks_for_translation(text: &str) -> String {
    if !text.contains('\n') && !text.contains('\r') {
        return text.to_string();
    }

    let normalized_newlines = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = normalized_newlines.split('\n').peekable();
    let mut output = String::with_capacity(text.len());

    while let Some(line) = lines.next() {
        output.push_str(line.trim_end());
        let Some(next_line) = lines.peek().copied() else {
            break;
        };

        if should_join_soft_line_break(line, next_line) {
            if line.trim_end().ends_with('-') {
                output.pop();
            } else if !output.chars().last().is_some_and(|ch| ch.is_whitespace()) {
                output.push(' ');
            }
        } else {
            output.push('\n');
        }
    }

    output
}

fn editor_translation_progress_labels(
    language: Language,
) -> app_windows::podcast_save_window::SaveDialogLabels {
    let (title, in_progress, cancel_confirm) = match language {
        Language::Italian => (
            "Traduzione in corso".to_string(),
            "Traduzione in corso...".to_string(),
            "Vuoi annullare la traduzione?".to_string(),
        ),
        _ => (
            "Translation in progress".to_string(),
            "Translation in progress...".to_string(),
            "Do you want to cancel the translation?".to_string(),
        ),
    };

    app_windows::podcast_save_window::SaveDialogLabels {
        title,
        in_progress,
        cancel: i18n::tr(language, "podcast.save.cancel"),
        cancel_confirm,
    }
}

fn editor_summary_progress_labels(
    language: Language,
) -> app_windows::podcast_save_window::SaveDialogLabels {
    let in_progress = i18n::tr(language, "summarize.status.starting");
    let cancel_confirm = match language {
        Language::Italian => "Vuoi annullare il riassunto?".to_string(),
        _ => "Do you want to cancel the summary?".to_string(),
    };

    app_windows::podcast_save_window::SaveDialogLabels {
        title: in_progress.clone(),
        in_progress,
        cancel: i18n::tr(language, "podcast.save.cancel"),
        cancel_confirm,
    }
}

fn open_editor_translation_progress_window(
    hwnd: HWND,
    language: Language,
    cancel_token: Arc<AtomicBool>,
) {
    let labels = editor_translation_progress_labels(language);
    let canceling_message = i18n::tr(language, "editor.translation.canceling");
    open_editor_progress_window(hwnd, language, labels, cancel_token, canceling_message);
}

fn open_editor_summary_progress_window(
    hwnd: HWND,
    language: Language,
    cancel_token: Arc<AtomicBool>,
) {
    let labels = editor_summary_progress_labels(language);
    let canceling_message = i18n::tr(language, "editor.summary.canceling");
    open_editor_progress_window(hwnd, language, labels, cancel_token, canceling_message);
}

fn open_editor_progress_window(
    hwnd: HWND,
    language: Language,
    labels: app_windows::podcast_save_window::SaveDialogLabels,
    cancel_token: Arc<AtomicBool>,
    canceling_message: String,
) {
    let dialog = app_windows::podcast_save_window::open_with_labels(hwnd, language, labels, true);
    if dialog.0 != 0 {
        app_windows::podcast_save_window::disable_fake_progress(dialog);
        app_windows::podcast_save_window::focus_cancel_button(dialog);
    }
    with_state(hwnd, |state| {
        state.editor_translation_progress_window = dialog;
        state.editor_translation_cancel_token = Some(cancel_token);
        state.editor_progress_canceling_message = canceling_message;
    });
}

fn close_editor_translation_progress_window(hwnd: HWND) {
    let dialog =
        with_state(hwnd, |state| state.editor_translation_progress_window).unwrap_or(HWND(0));
    if dialog.0 != 0 {
        crate::send_message_w_safe(
            dialog,
            app_windows::podcast_save_window::WM_PODCAST_SAVE_DONE,
            WPARAM(0),
            LPARAM(0),
        );
    }
    with_state(hwnd, |state| {
        state.editor_translation_progress_window = HWND(0);
        state.editor_translation_cancel_token = None;
    });
}

fn post_editor_progress_percent(dialog_value: isize, pct: usize) {
    if dialog_value == 0 {
        return;
    }
    if let Err(err) = post_message_w_safe(
        HWND(dialog_value),
        app_windows::podcast_save_window::WM_PODCAST_SAVE_PROGRESS,
        WPARAM(pct.min(100)),
        LPARAM(0),
    ) {
        log_debug(&format!("Editor progress: failed to post progress: {err}"));
    }
}

struct EditorProgressTracker {
    dialog_value: isize,
    last_pct: usize,
}

impl EditorProgressTracker {
    fn new(dialog_value: isize) -> Self {
        Self {
            dialog_value,
            last_pct: 0,
        }
    }

    fn post_chunk(&mut self, completed: usize, total: usize) {
        if total == 0 {
            return;
        }

        let pct = ((completed.saturating_mul(100)) / total).min(100);
        if pct < self.last_pct {
            return;
        }

        self.last_pct = pct;
        post_editor_progress_percent(self.dialog_value, pct);
    }
}

fn selected_text_from_edit(hwnd_edit: HWND, selection: CHARRANGE) -> Option<String> {
    let start = selection.cpMin.min(selection.cpMax).max(0) as usize;
    let end = selection.cpMin.max(selection.cpMax).max(0) as usize;
    if start >= end {
        log_debug(&format!(
            "Editor translation: empty range start={start} end={end}"
        ));
        return None;
    }

    let len = end.checked_sub(start)?;
    let mut buffer = vec![0u16; len + 1];
    let range = TEXTRANGEW {
        chrg: CHARRANGE {
            cpMin: start.min(i32::MAX as usize) as i32,
            cpMax: end.min(i32::MAX as usize) as i32,
        },
        lpstrText: PWSTR(buffer.as_mut_ptr()),
    };
    let copied = send_message_w_safe(
        hwnd_edit,
        EM_GETTEXTRANGE,
        WPARAM(0),
        LPARAM(&range as *const _ as isize),
    )
    .0
    .max(0) as usize;
    if copied == 0 {
        log_debug(&format!(
            "Editor translation: EM_GETTEXTRANGE copied 0 chars start={start} end={end}"
        ));
        return None;
    }

    log_debug(&format!(
        "Editor translation: extracted text chars={} range={}..{}",
        copied, start, end
    ));
    Some(String::from_utf16_lossy(&buffer[..copied]))
}

fn start_editor_selection_translation(
    hwnd: HWND,
    hwnd_edit: HWND,
    selection: CHARRANGE,
    target_lang: &'static str,
    language: Language,
) {
    let Some(text) = selected_text_from_edit(hwnd_edit, selection) else {
        log_debug("Editor translation: no text extracted, translation not started");
        return;
    };
    start_editor_translation_text(
        hwnd,
        hwnd_edit,
        selection.cpMin.min(selection.cpMax),
        selection.cpMin.max(selection.cpMax),
        text,
        target_lang,
        language,
    );
}

fn start_editor_translation_text(
    hwnd: HWND,
    _hwnd_edit: HWND,
    cp_min: i32,
    cp_max: i32,
    text: String,
    target_lang: &'static str,
    language: Language,
) {
    let hwnd_value = hwnd.0;
    let target = target_lang.to_string();
    let source = "auto".to_string();
    let original_chars = text.chars().count();
    let original_cr = text.matches('\r').count();
    let original_lf = text.matches('\n').count();
    let text = normalize_soft_line_breaks_for_translation(&text);
    let normalized_chars = text.chars().count();
    let normalized_lf = text.matches('\n').count();
    if normalized_chars != original_chars {
        log_debug(&format!(
            "Editor translation: normalized soft line breaks chars_before={} chars_after={} cr_before={} lf_before={} lf_after={}",
            original_chars, normalized_chars, original_cr, original_lf, normalized_lf
        ));
    }
    log_debug(&format!(
        "Editor translation: starting source={} target={} range={}..{} chars={}",
        source, target, cp_min, cp_max, normalized_chars
    ));

    let cancel_token = Arc::new(AtomicBool::new(false));
    open_editor_translation_progress_window(hwnd, language, cancel_token.clone());
    let progress_dialog_value = with_state(hwnd, |state| state.editor_translation_progress_window)
        .unwrap_or(HWND(0))
        .0;

    let spawn_result = std::thread::Builder::new()
        .name("editor-selection-translation".to_string())
        .spawn(move || {
            log_debug(&format!(
                "Editor translation worker: request begin source={} target={} chars={}",
                source,
                target,
                text.chars().count()
            ));
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|err| format!("Runtime traduzione non disponibile: {err}"))
                .and_then(|runtime| {
                    runtime.block_on(async {
                        let mut progress_tracker =
                            EditorProgressTracker::new(progress_dialog_value);
                        let mut gemini_failed = false;
                        let settings = crate::load_settings();
                        if !settings.gemini_api_key.trim().is_empty() {
                            let mut progress_callback = |completed: usize, total: usize| {
                                progress_tracker.post_chunk(completed, total);
                            };
                            log_debug(&format!(
                                "Editor translation worker: using Gemini (model={})",
                                settings.gemini_model
                            ));
                            let gemini_translator = translator::TranslatorGemini::new(
                                settings.gemini_api_key.clone(),
                                settings.gemini_model.clone(),
                                target.clone(),
                                if source == "auto" { None } else { Some(source.clone()) },
                            )
                            .map_err(|err| err.to_string())?;

                            match gemini_translator
                                .translate_chunked_cancellable_with_progress(
                                    &text,
                                    cancel_token.as_ref(),
                                    &mut progress_callback,
                                )
                                .await
                            {
                                Ok(translated) => {
                                    return Ok(EditorTranslationSuccess {
                                        text: translated,
                                        provider: "Gemini",
                                        fallback_from: None,
                                    });
                                }
                                Err(err) => {
                                    if gemini_model_uses_compat_fallback(&settings.gemini_model)
                                        && !cancel_token.load(Ordering::Relaxed)
                                    {
                                        log_debug(&format!(
                                            "Editor translation worker: Gemini model {} failed: {err}, retrying {}",
                                            settings.gemini_model,
                                            GEMINI_TRANSLATION_FALLBACK_MODEL
                                        ));
                                        let fallback_gemini = translator::TranslatorGemini::new(
                                            settings.gemini_api_key.clone(),
                                            GEMINI_TRANSLATION_FALLBACK_MODEL.to_string(),
                                            target.clone(),
                                            if source == "auto" {
                                                None
                                            } else {
                                                Some(source.clone())
                                            },
                                        )
                                        .map_err(|err| err.to_string())?;
                                        match fallback_gemini
                                            .translate_chunked_cancellable_with_progress(
                                                &text,
                                                cancel_token.as_ref(),
                                                &mut progress_callback,
                                            )
                                            .await
                                        {
                                            Ok(translated) => {
                                                return Ok(EditorTranslationSuccess {
                                                    text: translated,
                                                    provider: "Gemini 2.5 Flash",
                                                    fallback_from: Some("Gemini 3.0"),
                                                });
                                            }
                                            Err(fallback_err) => {
                                                log_debug(&format!(
                                                    "Editor translation worker: Gemini fallback model {} failed: {fallback_err}",
                                                    GEMINI_TRANSLATION_FALLBACK_MODEL
                                                ));
                                            }
                                        }
                                    }
                                    log_debug(&format!(
                                        "Editor translation worker: Gemini failed: {err}, falling back to DeepL/Google"
                                    ));
                                    gemini_failed = true;
                                }
                            }
                        }

                        let google_source = if source == target {
                            "auto".to_string()
                        } else {
                            source.clone()
                        };

                        let translate_with_google = |deepl_status: String,
                                                     google_source: String,
                                                     target: String,
                                                     text: String,
                                                     fallback_from: &'static str,
                                                     cancel_token: Arc<AtomicBool>,
                                                     progress_dialog_value: isize,
                                                     inherited_progress_pct: usize| async move {
                            log_debug(&format!(
                                "Editor translation worker: Google fallback source={} target={}",
                                google_source, target
                            ));
                            let google_translator =
                                translator::TranslatorGoogleFree::new(target.clone(), google_source)
                                    .map_err(|err| err.to_string())?;
                            let mut progress_tracker =
                                EditorProgressTracker::new(progress_dialog_value);
                            progress_tracker.last_pct = inherited_progress_pct;
                            let mut progress_callback = |completed: usize, total: usize| {
                                progress_tracker.post_chunk(completed, total);
                            };
                            google_translator
                                .translate_chunked_cancellable_with_progress(
                                    &text,
                                    Some(cancel_token.as_ref()),
                                    &mut progress_callback,
                                )
                                .await
                                .map(|translated| EditorTranslationSuccess {
                                    text: translated,
                                    provider: "Google Translate",
                                    fallback_from: Some(fallback_from),
                                })
                                .map_err(|google_err| {
                                    format!(
                                        "{deepl_status}; Google fallback failed: {google_err}"
                                    )
                                })
                        };

                        let cooldown_remaining = deepl_free_cooldown_remaining_seconds();
                        if cooldown_remaining > 0 {
                            let deepl_status =
                                format!("DeepL cooldown active for {cooldown_remaining}s");
                            let fallback_prov = if gemini_failed {
                                "Gemini"
                            } else {
                                "DeepL"
                            };
                            log_debug(&format!(
                                "Editor translation worker: {deepl_status}, using Google fallback directly"
                            ));
                            return translate_with_google(
                                deepl_status,
                                google_source,
                                target,
                                text,
                                fallback_prov,
                                cancel_token.clone(),
                                progress_dialog_value,
                                progress_tracker.last_pct,
                            )
                            .await;
                        }

                        let deepl_translator = if source == "auto" {
                            translator::TranslatorDeepLFree::new(target.clone())
                        } else {
                            translator::TranslatorDeepLFree::with_source_lang(
                                target.clone(),
                                source.clone(),
                            )
                        }
                        .map_err(|err| err.to_string())?;

                        if cancel_token.load(Ordering::Relaxed) {
                            return Err(translator::TranslatorError::Cancelled.to_string());
                        }

                        let deepl_result = {
                            let mut progress_callback = |completed: usize, total: usize| {
                                progress_tracker.post_chunk(completed, total);
                            };
                            deepl_translator
                                .translate_chunked_cancellable_with_progress(
                                &text,
                                Some(cancel_token.as_ref()),
                                &mut progress_callback,
                            )
                            .await
                        };

                        match deepl_result
                        {
                            Ok(translated) => Ok(EditorTranslationSuccess {
                                text: translated,
                                provider: "DeepL",
                                fallback_from: if !settings.gemini_api_key.trim().is_empty() {
                                    Some("Gemini")
                                } else {
                                    None
                                },
                            }),
                            Err(deepl_err) => {
                                if is_deepl_too_many_requests_error(&deepl_err) {
                                    mark_deepl_free_cooldown();
                                    log_debug(&format!(
                                        "Editor translation worker: DeepL too many requests, cooldown set for {}s",
                                        DEEPL_FREE_COOLDOWN_SECONDS
                                    ));
                                }
                                let deepl_status = format!("DeepL failed: {deepl_err}");
                                log_debug(&format!(
                                    "Editor translation worker: {deepl_status}, trying Google fallback"
                                ));
                                translate_with_google(
                                    deepl_status,
                                    google_source,
                                    target,
                                    text,
                                    if !settings.gemini_api_key.trim().is_empty() {
                                        "Gemini"
                                    } else {
                                        "DeepL"
                                    },
                                    cancel_token.clone(),
                                    progress_dialog_value,
                                    progress_tracker.last_pct,
                                )
                                .await
                            }
                        }
                    })
                });
            match &result {
                Ok(translated) => log_debug(&format!(
                    "Editor translation worker: request ok provider={} fallback_from={:?} translated_chars={}",
                    translated.provider,
                    translated.fallback_from,
                    translated.text.chars().count()
                )),
                Err(err) => log_debug(&format!("Editor translation worker: request failed: {err}")),
            }
            let payload = Box::new(EditorTranslationResult {
                language,
                result,
            });
            let ptr = Box::into_raw(payload);
            if let Err(err) = post_message_w_safe(
                HWND(hwnd_value),
                WM_EDITOR_TRANSLATION_DONE,
                WPARAM(0),
                LPARAM(ptr as isize),
            ) {
                log_debug(&format!("Failed to post WM_EDITOR_TRANSLATION_DONE: {err}"));
                let _unused_box = box_from_raw_safe(ptr);
            }
        });

    if let Err(err) = spawn_result {
        close_editor_translation_progress_window(hwnd);
        show_error(
            hwnd,
            language,
            &format!("Impossibile avviare la traduzione: {err}"),
        );
    }
}

fn handle_editor_translate_menu_command(hwnd: HWND, cmd_id: usize) {
    let Some(target_language) = editor_translate_language_for_menu_id(cmd_id) else {
        log_debug(&format!(
            "Editor translation: command ignored, unknown cmd_id={cmd_id}"
        ));
        return;
    };
    let Some(hwnd_edit) = get_active_edit(hwnd) else {
        log_debug("Editor translation: command ignored, no active editor");
        return;
    };
    log_debug(&format!(
        "Editor translation: command cmd_id={} key={} target={}",
        cmd_id, target_language.key, target_language.target
    ));

    let language = with_state(hwnd, |state| {
        state.settings.editor_translate_target_language = target_language.key.to_string();
        record_editor_translate_recent_language(&mut state.settings, target_language.key);
        let language = state.settings.language;
        save_settings(state.settings.clone());
        language
    })
    .unwrap_or_default();

    let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
    send_message_w_safe(
        hwnd_edit,
        EM_EXGETSEL,
        WPARAM(0),
        LPARAM(&mut selection as *mut _ as isize),
    );
    if selection.cpMin == selection.cpMax {
        let text = editor_manager::get_edit_text(hwnd_edit);
        if text.is_empty() {
            log_debug("Editor translation: document empty, translation not started");
            return;
        }
        log_debug(&format!(
            "Editor translation: no selection, using full document chars={}",
            text.chars().count()
        ));
        start_editor_translation_text(
            hwnd,
            hwnd_edit,
            0,
            -1,
            text,
            target_language.target,
            language,
        );
        return;
    } else {
        log_debug(&format!(
            "Editor translation: using selection range={}..{}",
            selection.cpMin, selection.cpMax
        ));
    }

    start_editor_selection_translation(
        hwnd,
        hwnd_edit,
        selection,
        target_language.target,
        language,
    );
}

fn handle_editor_summarize_menu_command(hwnd: HWND) {
    let Some(hwnd_edit) = get_active_edit(hwnd) else {
        log_debug("Editor summary: command ignored, no active editor");
        return;
    };
    let (language, api_key, model) = with_state(hwnd, |state| {
        (
            state.settings.language,
            state.settings.gemini_api_key.clone(),
            state.settings.gemini_model.clone(),
        )
    })
    .unwrap_or_default();
    if api_key.trim().is_empty() {
        show_error(
            hwnd,
            language,
            &i18n::tr(language, "summarize.error.no_gemini_key"),
        );
        return;
    }

    let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
    send_message_w_safe(
        hwnd_edit,
        EM_EXGETSEL,
        WPARAM(0),
        LPARAM(&mut selection as *mut _ as isize),
    );
    let text = if selection.cpMin != selection.cpMax {
        selected_text_from_edit(hwnd_edit, selection).unwrap_or_default()
    } else {
        editor_manager::get_edit_text(hwnd_edit)
    };
    if text.trim().is_empty() {
        log_debug("Editor summary: empty text, summary not started");
        return;
    }

    let starting = i18n::tr(language, "summarize.status.starting");
    set_main_status_message(hwnd, &starting);
    screen_reader_speak(&starting);
    let cancel_token = Arc::new(AtomicBool::new(false));
    open_editor_summary_progress_window(hwnd, language, cancel_token.clone());
    start_editor_summary_text(hwnd, text, language, api_key, model, cancel_token);
}

fn start_editor_summary_text(
    hwnd: HWND,
    text: String,
    language: Language,
    api_key: String,
    model: String,
    cancel_token: Arc<AtomicBool>,
) {
    let hwnd_value = hwnd.0;
    let progress_dialog_value = with_state(hwnd, |state| state.editor_translation_progress_window)
        .unwrap_or(HWND(0))
        .0;
    let spawn_result = std::thread::Builder::new()
        .name("editor-summary".to_string())
        .spawn(move || {
            log_debug(&format!(
                "Editor summary worker: request begin chars={}",
                text.chars().count()
            ));
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|err| format!("Runtime riassunto non disponibile: {err}"))
                .and_then(|runtime| {
                    runtime.block_on(async {
                        let mut progress_tracker =
                            EditorProgressTracker::new(progress_dialog_value);
                        let mut progress_callback = |completed: usize, total: usize| {
                            progress_tracker.post_chunk(completed, total);
                        };
                        let gemini = translator::TranslatorGemini::new(
                            api_key.clone(),
                            model.clone(),
                            String::new(),
                            None,
                        )
                        .map_err(|err| err.to_string())?;
                        match gemini
                            .summarize_same_language_chunked_cancellable_with_progress(
                                &text,
                                cancel_token.as_ref(),
                                &mut progress_callback,
                            )
                            .await
                        {
                            Ok(summary) => {
                                if cancel_token.load(Ordering::Relaxed) {
                                    return Err(
                                        translator::TranslatorError::Cancelled.to_string()
                                    );
                                }
                                Ok(EditorSummarySuccess {
                                    text: summary,
                                    warning: None,
                                })
                            }
                            Err(translator::TranslatorError::PartialSummary {
                                summary,
                                error,
                            }) if gemini_model_uses_compat_fallback(&model)
                                && !cancel_token.load(Ordering::Relaxed) =>
                            {
                                log_debug(&format!(
                                    "Editor summary worker: Gemini model {} returned partial summary: {error}, retrying {}",
                                    model, GEMINI_TRANSLATION_FALLBACK_MODEL
                                ));
                                let fallback_gemini = translator::TranslatorGemini::new(
                                    api_key,
                                    GEMINI_TRANSLATION_FALLBACK_MODEL.to_string(),
                                    String::new(),
                                    None,
                                )
                                .map_err(|err| err.to_string())?;
                                match fallback_gemini
                                    .summarize_same_language_chunked_cancellable_with_progress(
                                        &text,
                                        cancel_token.as_ref(),
                                        &mut progress_callback,
                                    )
                                    .await
                                {
                                    Ok(fallback_summary) => {
                                        if cancel_token.load(Ordering::Relaxed) {
                                            return Err(
                                                translator::TranslatorError::Cancelled.to_string()
                                            );
                                        }
                                        log_debug(&format!(
                                            "Editor summary worker: fallback model {} completed",
                                            GEMINI_TRANSLATION_FALLBACK_MODEL
                                        ));
                                        Ok(EditorSummarySuccess {
                                            text: fallback_summary,
                                            warning: None,
                                        })
                                    }
                                    Err(translator::TranslatorError::PartialSummary {
                                        summary: fallback_partial,
                                        error: fallback_error,
                                    }) => {
                                        log_debug(&format!(
                                            "Editor summary worker: fallback model {} returned partial summary: {fallback_error}",
                                            GEMINI_TRANSLATION_FALLBACK_MODEL
                                        ));
                                        Ok(EditorSummarySuccess {
                                            text: fallback_partial,
                                            warning: Some(fallback_error),
                                        })
                                    }
                                    Err(fallback_err) => {
                                        log_debug(&format!(
                                            "Editor summary worker: Gemini fallback model {} failed: {fallback_err}; keeping primary partial summary",
                                            GEMINI_TRANSLATION_FALLBACK_MODEL
                                        ));
                                        Ok(EditorSummarySuccess {
                                            text: summary,
                                            warning: Some(error),
                                        })
                                    }
                                }
                            }
                            Err(err)
                                if gemini_model_uses_compat_fallback(&model)
                                    && !cancel_token.load(Ordering::Relaxed) =>
                            {
                                log_debug(&format!(
                                    "Editor summary worker: Gemini model {} failed: {err}, retrying {}",
                                    model, GEMINI_TRANSLATION_FALLBACK_MODEL
                                ));
                                let fallback_gemini = translator::TranslatorGemini::new(
                                    api_key,
                                    GEMINI_TRANSLATION_FALLBACK_MODEL.to_string(),
                                    String::new(),
                                    None,
                                )
                                .map_err(|err| err.to_string())?;
                                match fallback_gemini
                                    .summarize_same_language_chunked_cancellable_with_progress(
                                        &text,
                                        cancel_token.as_ref(),
                                        &mut progress_callback,
                                    )
                                    .await
                                {
                                    Ok(summary) => {
                                        if cancel_token.load(Ordering::Relaxed) {
                                            return Err(
                                                translator::TranslatorError::Cancelled.to_string()
                                            );
                                        }
                                        log_debug(&format!(
                                            "Editor summary worker: fallback model {} completed",
                                            GEMINI_TRANSLATION_FALLBACK_MODEL
                                        ));
                                        Ok(EditorSummarySuccess {
                                            text: summary,
                                            warning: None,
                                        })
                                    }
                                    Err(translator::TranslatorError::PartialSummary {
                                        summary,
                                        error,
                                    }) => {
                                        log_debug(&format!(
                                            "Editor summary worker: fallback model {} returned partial summary: {error}",
                                            GEMINI_TRANSLATION_FALLBACK_MODEL
                                        ));
                                        Ok(EditorSummarySuccess {
                                            text: summary,
                                            warning: Some(error),
                                        })
                                    }
                                    Err(fallback_err) => {
                                        log_debug(&format!(
                                            "Editor summary worker: Gemini fallback model {} failed: {fallback_err}",
                                            GEMINI_TRANSLATION_FALLBACK_MODEL
                                        ));
                                        Err(fallback_err.to_string())
                                    }
                                }
                            }
                            Err(translator::TranslatorError::PartialSummary { summary, error }) => {
                                Ok(EditorSummarySuccess {
                                    text: summary,
                                    warning: Some(error),
                                })
                            }
                            Err(err) => Err(err.to_string()),
                        }
                    })
                });
            match &result {
                Ok(summary) => log_debug(&format!(
                    "Editor summary worker: request ok summary_chars={}",
                    summary.text.chars().count()
                )),
                Err(err) => log_debug(&format!("Editor summary worker: request failed: {err}")),
            }
            let payload = Box::new(EditorSummaryResult { language, result });
            let ptr = Box::into_raw(payload);
            if let Err(err) = post_message_w_safe(
                HWND(hwnd_value),
                WM_EDITOR_SUMMARY_DONE,
                WPARAM(0),
                LPARAM(ptr as isize),
            ) {
                log_debug(&format!("Failed to post WM_EDITOR_SUMMARY_DONE: {err}"));
                let _unused_box = box_from_raw_safe(ptr);
            }
        });

    if let Err(err) = spawn_result {
        close_editor_translation_progress_window(hwnd);
        let message = i18n::tr_f(
            language,
            "summarize.error.failed",
            &[("err", &err.to_string())],
        );
        show_error(hwnd, language, &message);
    }
}

fn apply_editor_summary_result(hwnd: HWND, payload: EditorSummaryResult) {
    close_editor_translation_progress_window(hwnd);
    let success = match payload.result {
        Ok(summary) => summary,
        Err(err) => {
            if err.contains("canceled") || err.contains("cancelled") {
                return;
            }
            let message = i18n::tr_f(payload.language, "summarize.error.failed", &[("err", &err)]);
            show_error(hwnd, payload.language, &message);
            return;
        }
    };
    if let Some(warning) = success.warning.as_ref() {
        log_debug(&format!(
            "Editor summary: partial summary warning: {warning}"
        ));
        let message = i18n::tr(payload.language, "summary.partial_created_error");
        show_info(hwnd, payload.language, &message);
    }
    editor_manager::new_document(hwnd);
    let Some(hwnd_edit) = get_active_edit(hwnd) else {
        log_debug("Editor summary: no active editor for summary result");
        return;
    };
    editor_manager::set_current_document_title(
        hwnd,
        &i18n::tr(payload.language, "context_menu.summarize"),
    );
    editor_manager::set_edit_text(hwnd_edit, &success.text);
    editor_manager::mark_dirty_from_edit(hwnd, hwnd_edit);
    let completed = i18n::tr(payload.language, "summarize.status.completed");
    set_main_status_message(hwnd, &completed);
    set_focus_safe(hwnd_edit);
    schedule_editor_translation_speak(hwnd, completed);
}

fn apply_editor_translation_result(hwnd: HWND, payload: EditorTranslationResult) {
    close_editor_translation_progress_window(hwnd);
    let success = match payload.result {
        Ok(success) => success,
        Err(err) => {
            log_debug(&format!(
                "Editor translation: applying failed result: {err}"
            ));
            if err.contains("Translation canceled") {
                return;
            }
            let message = i18n::tr_f(
                payload.language,
                "editor.translation.error",
                &[("err", &err)],
            );
            show_error(hwnd, payload.language, &message);
            return;
        }
    };
    let translated_text = success.text;
    editor_manager::new_document(hwnd);
    let Some(hwnd_edit) = get_active_edit(hwnd) else {
        log_debug("Editor translation: no active editor for translation result");
        return;
    };
    editor_manager::set_current_document_title(
        hwnd,
        &i18n::tr(payload.language, "document.translation.title"),
    );
    editor_manager::set_edit_text(hwnd_edit, &translated_text);
    editor_manager::mark_dirty_from_edit(hwnd, hwnd_edit);
    set_focus_safe(hwnd_edit);
    announce_editor_translation_completed(
        hwnd,
        payload.language,
        success.provider,
        success.fallback_from,
    );
}

fn announce_editor_translation_completed(
    hwnd: HWND,
    language: Language,
    provider: &str,
    fallback_from: Option<&str>,
) {
    let message = if let Some(primary) = fallback_from {
        i18n::tr_f(
            language,
            "translation.completed_with_fallback",
            &[("primary", primary), ("provider", provider)],
        )
    } else {
        i18n::tr_f(
            language,
            "translation.completed_with_provider",
            &[("provider", provider)],
        )
    };
    set_main_status_message(hwnd, &message);
    schedule_editor_translation_speak(hwnd, message);
}

fn schedule_editor_translation_speak(hwnd: HWND, message: String) {
    let scheduled = with_state(hwnd, |state| {
        state.editor_translation_pending_speak = Some(message);
        unsafe { SetTimer(hwnd, EDITOR_TRANSLATION_SPEAK_TIMER_ID, 300, None) != 0 }
    })
    .unwrap_or(false);
    if !scheduled {
        log_debug("Failed to set EDITOR_TRANSLATION_SPEAK_TIMER_ID");
        speak_pending_editor_translation_message(hwnd);
    }
}

fn speak_pending_editor_translation_message(hwnd: HWND) {
    let message = with_state(hwnd, |state| state.editor_translation_pending_speak.take()).flatten();
    if let Some(message) = message {
        screen_reader_speak(&message);
    }
}

fn set_main_status_message(hwnd: HWND, message: &str) {
    let hwnd_status = with_state(hwnd, |state| state.hwnd_status).unwrap_or(HWND(0));
    if hwnd_status.0 == 0 {
        return;
    }
    let message_wide = to_wide(message);
    send_message_w_safe(
        hwnd_status,
        SB_SETTEXTW,
        WPARAM(0),
        LPARAM(message_wide.as_ptr() as isize),
    );
}

fn open_dictionary_lookup(hwnd: HWND) {
    app_windows::wiktionary_window::open(hwnd);
}

fn can_undo_now(hwnd: HWND) -> bool {
    if with_state(hwnd, |state| state.normalize_undo.is_some()).unwrap_or(false) {
        return true;
    }
    let Some(hwnd_edit) = get_active_edit(hwnd) else {
        return false;
    };
    // SAFETY: querying EM_CANUNDO on the active edit control is side-effect free.
    send_message_w_safe(hwnd_edit, EM_CANUNDO, WPARAM(0), LPARAM(0)).0 != 0
}

pub(crate) fn update_main_status_bar(hwnd: HWND) {
    let (hwnd_status, language) =
        { with_state(hwnd, |state| (state.hwnd_status, state.settings.language)) }
            .unwrap_or((HWND(0), Language::default()));
    if hwnd_status.0 == 0 {
        return;
    }
    let (chars, words, line, col) = if let Some(hwnd_edit) = get_active_edit(hwnd) {
        let text = editor_manager::get_edit_text(hwnd_edit);
        let chars = text.chars().count();
        let words = text.split_whitespace().count();
        let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
        unsafe {
            SendMessageW(
                hwnd_edit,
                EM_EXGETSEL,
                WPARAM(0),
                LPARAM(&mut selection as *mut _ as isize),
            );
        }
        let caret = selection.cpMax.max(0);
        let line_idx = unsafe {
            SendMessageW(
                hwnd_edit,
                EM_LINEFROMCHAR,
                WPARAM(caret as usize),
                LPARAM(0),
            )
            .0 as i32
        };
        let line_start = unsafe {
            SendMessageW(
                hwnd_edit,
                EM_LINEINDEX,
                WPARAM(line_idx.max(0) as usize),
                LPARAM(0),
            )
            .0 as i32
        };
        let col = (caret - line_start).max(0) + 1;
        (chars, words, line_idx.max(0) + 1, col)
    } else {
        (0, 0, 1, 1)
    };
    let chars_str = chars.to_string();
    let words_str = words.to_string();
    let chars_label = i18n::tr_f(
        language,
        "text_stats.characters_with_spaces",
        &[("count", &chars_str)],
    );
    let words_label = i18n::tr_f(language, "text_stats.words", &[("count", &words_str)]);
    let chars_label = chars_label.trim_end_matches('.');
    let words_label = words_label.trim_end_matches('.');
    let label = format!(
        "{}. | {}. | Ln {}, Col {}",
        chars_label, words_label, line, col
    );
    let label_wide = to_wide(&label);
    unsafe {
        SendMessageW(
            hwnd_status,
            SB_SETTEXTW,
            WPARAM(0),
            LPARAM(label_wide.as_ptr() as isize),
        );
    }
}

fn build_undo_menu_label(hwnd: HWND, language: Language) -> String {
    let base = i18n::tr(language, "edit.undo");
    let action = { with_state(hwnd, |state| state.undo_action_label.clone()).flatten() };
    let Some(action) = action else {
        return base;
    };
    let action = action.trim();
    if action.is_empty() {
        return base;
    }
    let mut split = base.splitn(2, '\t');
    let caption = split.next().unwrap_or("").trim();
    let accel = split.next();
    if caption.is_empty() {
        return base;
    }
    match accel {
        Some(accel) if !accel.is_empty() => format!("{}: {}\t{}", caption, action, accel),
        _ => format!("{}: {}", caption, action),
    }
}

fn has_active_text_selection(hwnd: HWND) -> bool {
    let Some(hwnd_edit) = get_active_edit(hwnd) else {
        return false;
    };
    let mut selection = CHARRANGE { cpMin: 0, cpMax: 0 };
    // SAFETY: `selection` is valid writable memory and `hwnd_edit` is the active edit control.
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut selection as *mut _ as isize),
        );
    }
    selection.cpMin != selection.cpMax
}

fn can_paste_now(hwnd: HWND) -> bool {
    if get_active_edit(hwnd).is_none() {
        return false;
    }
    // CF_UNICODETEXT = 13
    is_clipboard_format_available_safe(13)
}

fn show_voice_context_menu(hwnd: HWND, target: HWND, lparam: LPARAM) {
    let (combo_voice, combo_favorites, engine, language) = {
        with_state(hwnd, |state| {
            (
                state.voice_combo_voice,
                state.voice_combo_favorites,
                state.settings.tts_engine,
                state.settings.language,
            )
        })
    }
    .unwrap_or((HWND(0), HWND(0), TtsEngine::Edge, Language::Italian));
    let labels = voice_panel_labels(language);

    let mut action_id = VOICE_MENU_ID_ADD_FAVORITE;
    let mut action_label = labels.add_favorite;
    let mut ctx_voice: Option<FavoriteVoice> = None;

    if target == combo_favorites {
        if let Some(fav) = current_favorite_selection(hwnd) {
            action_id = VOICE_MENU_ID_REMOVE_FAVORITE;
            action_label = labels.remove_favorite;
            ctx_voice = Some(fav);
        }
    } else if target == combo_voice {
        let Some(voice_name) = current_voice_selection(hwnd, engine) else {
            return;
        };
        let is_favorite = {
            with_state(hwnd, |state| {
                state
                    .settings
                    .favorite_voices
                    .iter()
                    .any(|fav| fav.engine == engine && fav.short_name == voice_name)
            })
        }
        .unwrap_or(false);
        if is_favorite {
            action_id = VOICE_MENU_ID_REMOVE_FAVORITE;
            action_label = labels.remove_favorite;
        }
        ctx_voice = Some(FavoriteVoice {
            engine,
            short_name: voice_name,
        });
    } else {
        return;
    }

    let Some(ctx) = ctx_voice else {
        return;
    };
    unsafe {
        let menu = CreatePopupMenu().unwrap_or(HMENU(0));
        if menu.0 == 0 {
            return;
        }
        crate::log_if_err!(AppendMenuW(
            menu,
            MF_STRING,
            action_id as usize,
            PCWSTR(to_wide(&action_label).as_ptr()),
        ));
        with_state(hwnd, |state| {
            state.voice_context_voice = Some(ctx);
        });

        let mut x = (lparam.0 & 0xffff) as i32;
        let mut y = ((lparam.0 >> 16) & 0xffff) as i32;
        if x == -1 && y == -1 {
            let mut pt = windows::Win32::Foundation::POINT::default();
            crate::log_if_err!(GetCursorPos(&mut pt));
            x = pt.x;
            y = pt.y;
        }

        SetForegroundWindow(hwnd);
        if !TrackPopupMenu(menu, TPM_RIGHTBUTTON, x, y, 0, hwnd, None).as_bool() {
            crate::log_debug("TrackPopupMenu failed");
        }
        crate::log_if_err!(PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0)));
    }
}

fn replace_spellcheck_word(hwnd_edit: HWND, ctx: &SpellcheckContextMenuState, replacement: &str) {
    let start_utf16 = ctx.line_start
        + spellcheck::utf8_byte_offset_to_utf16_units(&ctx.line_text, ctx.word_range.0);
    let end_utf16 = ctx.line_start
        + spellcheck::utf8_byte_offset_to_utf16_units(&ctx.line_text, ctx.word_range.1);
    let mut range = CHARRANGE {
        cpMin: start_utf16,
        cpMax: end_utf16,
    };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut range as *mut _ as isize),
        );
    }
    let wide = to_wide(replacement);
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    let new_end =
        start_utf16 + spellcheck::utf8_byte_offset_to_utf16_units(replacement, replacement.len());
    let mut new_sel = CHARRANGE {
        cpMin: new_end,
        cpMax: new_end,
    };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut new_sel as *mut _ as isize),
        );
    }
}

fn handle_spellcheck_suggestion(hwnd: HWND, index: usize) {
    let ctx = { with_state(hwnd, |state| state.spellcheck_context.clone()) }.unwrap_or(None);
    let Some(ctx) = ctx else {
        return;
    };
    let Some(replacement) = ctx.suggestions.get(index).cloned() else {
        return;
    };
    if ctx.hwnd_edit.0 != 0 {
        replace_spellcheck_word(ctx.hwnd_edit, &ctx, &replacement);
    }
    {
        with_state(hwnd, |state| {
            state.spellcheck_manager.clear_cache();
            state.spellcheck_last_announce = None;
            state.spellcheck_context = None;
        });
    }
}

fn handle_spellcheck_add_to_dictionary(hwnd: HWND) {
    let ctx = { with_state(hwnd, |state| state.spellcheck_context.clone()) }.unwrap_or(None);
    let Some(ctx) = ctx else {
        return;
    };
    {
        with_state(hwnd, |state| {
            state
                .spellcheck_manager
                .add_to_dictionary(&ctx.word, &ctx.language);
            state.spellcheck_last_announce = None;
            state.spellcheck_context = None;
        });
    }
}

fn handle_spellcheck_ignore_once(hwnd: HWND) {
    let ctx = { with_state(hwnd, |state| state.spellcheck_context.clone()) }.unwrap_or(None);
    let Some(ctx) = ctx else {
        return;
    };
    {
        with_state(hwnd, |state| {
            state
                .spellcheck_manager
                .ignore_once(&ctx.word, &ctx.language);
            state.spellcheck_last_announce = None;
            state.spellcheck_context = None;
        });
    }
}

/// Navigate to next (forward=true) or previous (forward=false) spelling error
fn go_to_spelling_error(hwnd: HWND, forward: bool) {
    use windows::Win32::UI::Controls::RichEdit::{CHARRANGE, EM_EXGETSEL, EM_EXSETSEL};

    let Some(hwnd_edit) = get_active_edit(hwnd) else {
        return;
    };

    // Get spellcheck language
    let resolution = {
        with_state(hwnd, |state| {
            state.spellcheck_manager.resolve_language(&state.settings)
        })
    }
    .flatten();
    let Some(resolution) = resolution else {
        // Spellcheck disabled or no language available
        return;
    };

    // Get current cursor position
    let mut cr = CHARRANGE { cpMin: 0, cpMax: 0 };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut cr as *mut _ as isize),
        );
    }
    let current_pos = if forward { cr.cpMax } else { cr.cpMin };

    // Get document info
    let doc_id = {
        with_state(hwnd, |state| {
            state
                .docs
                .iter()
                .find(|d| d.hwnd_edit == hwnd_edit)
                .map(|d| d.hwnd_edit.0)
        })
    }
    .flatten()
    .unwrap_or(0);

    let text = editor_manager::get_edit_text(hwnd_edit);
    if text.is_empty() {
        return;
    }

    // Collect all misspellings from all lines
    let mut all_errors: Vec<(i32, i32)> = Vec::new(); // (start_utf16, end_utf16)

    let mut line_start_utf16 = 0i32;
    for (line_idx, line) in text.lines().enumerate() {
        let misspellings = {
            with_state(hwnd, |state| {
                state.spellcheck_manager.check_line(
                    doc_id,
                    line_idx as i32,
                    line,
                    &resolution.effective,
                )
            })
        }
        .unwrap_or_default();

        for m in misspellings {
            // Convert byte offsets to UTF-16 offsets
            let start_byte = m.start;
            let end_byte = m.end;
            let prefix = &line[..start_byte.min(line.len())];
            let word_part = &line[start_byte.min(line.len())..end_byte.min(line.len())];
            let start_utf16_in_line: i32 = prefix.encode_utf16().count() as i32;
            let word_utf16_len: i32 = word_part.encode_utf16().count() as i32;

            let abs_start = line_start_utf16 + start_utf16_in_line;
            let abs_end = abs_start + word_utf16_len;
            all_errors.push((abs_start, abs_end));
        }

        // Account for line ending (could be \r\n or \n)
        let line_utf16_len: i32 = line.encode_utf16().count() as i32;
        line_start_utf16 += line_utf16_len + 1; // +1 for \n (simplified)
    }

    if all_errors.is_empty() {
        return;
    }

    let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();

    // Find the next/previous error relative to current position (no wrap-around)
    let target = if forward {
        // Find first error after current position
        all_errors.iter().find(|(start, _)| *start > current_pos)
    } else {
        // Find last error before current position
        all_errors.iter().rev().find(|(_, end)| *end < current_pos)
    };

    if let Some(&(start, end)) = target {
        // Select the misspelled word
        let mut new_range = CHARRANGE {
            cpMin: start,
            cpMax: end,
        };
        unsafe {
            SendMessageW(
                hwnd_edit,
                EM_EXSETSEL,
                WPARAM(0),
                LPARAM(&mut new_range as *mut _ as isize),
            );
            // Scroll to make visible
            SendMessageW(
                hwnd_edit,
                crate::accessibility::EM_SCROLLCARET,
                WPARAM(0),
                LPARAM(0),
            );
        }
    } else {
        let key = if forward {
            "spellcheck.no_next_error"
        } else {
            "spellcheck.no_previous_error"
        };
        screen_reader_speak(&i18n::tr(language, key));
    }
}

fn handle_voice_context_favorite(hwnd: HWND, add: bool) {
    let ctx = { with_state(hwnd, |state| state.voice_context_voice.clone()) }.unwrap_or(None);
    let Some(fav) = ctx else {
        return;
    };
    if add {
        add_favorite_voice(hwnd, fav.engine, &fav.short_name);
    } else {
        remove_favorite_voice(hwnd, fav.engine, &fav.short_name);
    }
    {
        with_state(hwnd, |state| {
            state.voice_context_voice = None;
        });
    }
}

fn add_favorite_voice(hwnd: HWND, engine: TtsEngine, voice_name: &str) {
    {
        with_state(hwnd, |state| {
            if state
                .settings
                .favorite_voices
                .iter()
                .any(|fav| fav.engine == engine && fav.short_name == voice_name)
            {
                return;
            }
            state.settings.favorite_voices.push(FavoriteVoice {
                engine,
                short_name: voice_name.to_string(),
            });
        });
    }
    if let Some(settings) = { with_state(hwnd, |state| state.settings.clone()) } {
        save_settings(settings);
    }
    refresh_voice_panel(hwnd);
}

fn remove_favorite_voice(hwnd: HWND, engine: TtsEngine, voice_name: &str) {
    {
        with_state(hwnd, |state| {
            state
                .settings
                .favorite_voices
                .retain(|fav| !(fav.engine == engine && fav.short_name == voice_name));
        });
    }
    if let Some(settings) = { with_state(hwnd, |state| state.settings.clone()) } {
        save_settings(settings);
    }
    refresh_voice_panel(hwnd);
}

fn is_focus_in_voice_panel(hwnd: HWND) -> bool {
    let focus = crate::get_focus_safe();
    if focus.0 == 0 {
        return false;
    }

    let mut class_buf = [0u16; 64];
    let len = crate::get_class_name_w_safe(focus, &mut class_buf);
    let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
    if class_name == "ComboLBox" {
        return true;
    }

    unsafe {
        with_state(hwnd, |state| {
            if !state.voice_panel_visible && !state.voice_favorites_visible {
                return false;
            }
            let is_match =
                |ctrl: HWND| ctrl.0 != 0 && (focus == ctrl || IsChild(ctrl, focus).as_bool());
            is_match(state.voice_combo_engine)
                || is_match(state.voice_combo_language)
                || is_match(state.voice_combo_voice)
                || is_match(state.voice_button_insert_tag)
                || is_match(state.voice_button_manage_google_voices)
                || is_match(state.voice_button_insert_pause)
                || is_match(state.voice_combo_speed)
                || is_match(state.voice_combo_pitch)
                || is_match(state.voice_combo_volume)
                || is_match(state.voice_edit_speed)
                || is_match(state.voice_edit_pitch)
                || is_match(state.voice_edit_volume)
                || is_match(state.voice_checkbox_multilingual)
                || is_match(state.voice_combo_favorites)
        })
    }
    .unwrap_or(false)
}

fn handle_voice_panel_tab(hwnd: HWND) -> bool {
    if is_mpv_playback_active(hwnd) {
        return false;
    }
    unsafe {
        let (
            visible,
            combo_engine,
            combo_language,
            combo_voice,
            button_insert_tag,
            button_manage_google_voices,
            button_insert_pause,
            combo_speed,
            combo_pitch,
            combo_volume,
            edit_speed,
            edit_pitch,
            edit_volume,
            checkbox_multilingual,
            combo_favorites,
            favorites_visible,
            is_edge,
            is_google,
            only_multilingual,
            manual_tuning,
            hwnd_tab,
        ) = match with_state(hwnd, |state| {
            (
                state.voice_panel_visible,
                state.voice_combo_engine,
                state.voice_combo_language,
                state.voice_combo_voice,
                state.voice_button_insert_tag,
                state.voice_button_manage_google_voices,
                state.voice_button_insert_pause,
                state.voice_combo_speed,
                state.voice_combo_pitch,
                state.voice_combo_volume,
                state.voice_edit_speed,
                state.voice_edit_pitch,
                state.voice_edit_volume,
                state.voice_checkbox_multilingual,
                state.voice_combo_favorites,
                state.voice_favorites_visible,
                matches!(state.settings.tts_engine, TtsEngine::Edge),
                matches!(state.settings.tts_engine, TtsEngine::Google),
                state.settings.tts_only_multilingual,
                state.settings.tts_manual_tuning,
                state.hwnd_tab,
            )
        }) {
            Some(values) => values,
            None => return false,
        };
        if !visible && !favorites_visible {
            return false;
        }
        let raw_focus = GetFocus();
        if raw_focus.0 == 0 {
            return false;
        }
        let focus = if raw_focus == combo_engine || IsChild(combo_engine, raw_focus).as_bool() {
            combo_engine
        } else if raw_focus == combo_language || IsChild(combo_language, raw_focus).as_bool() {
            combo_language
        } else if raw_focus == combo_voice || IsChild(combo_voice, raw_focus).as_bool() {
            combo_voice
        } else if raw_focus == button_insert_tag || IsChild(button_insert_tag, raw_focus).as_bool()
        {
            button_insert_tag
        } else if raw_focus == button_manage_google_voices
            || IsChild(button_manage_google_voices, raw_focus).as_bool()
        {
            button_manage_google_voices
        } else if raw_focus == button_insert_pause
            || IsChild(button_insert_pause, raw_focus).as_bool()
        {
            button_insert_pause
        } else if raw_focus == combo_speed || IsChild(combo_speed, raw_focus).as_bool() {
            combo_speed
        } else if raw_focus == combo_pitch || IsChild(combo_pitch, raw_focus).as_bool() {
            combo_pitch
        } else if raw_focus == combo_volume || IsChild(combo_volume, raw_focus).as_bool() {
            combo_volume
        } else if raw_focus == combo_favorites || IsChild(combo_favorites, raw_focus).as_bool() {
            combo_favorites
        } else {
            raw_focus
        };
        let is_combo_focus = focus == combo_engine
            || (is_edge && !only_multilingual && focus == combo_language)
            || focus == combo_voice
            || focus == button_insert_tag
            || (is_google && focus == button_manage_google_voices)
            || focus == button_insert_pause
            || (!manual_tuning && focus == combo_speed)
            || (!manual_tuning && focus == combo_pitch)
            || (!manual_tuning && focus == combo_volume)
            || (favorites_visible && focus == combo_favorites);
        if is_combo_focus {
            let dropped = SendMessageW(focus, CB_GETDROPPEDSTATE, WPARAM(0), LPARAM(0)).0 != 0;
            if dropped {
                return false;
            }
        }
        let (mut hwnd_edit, is_audiobook) = with_state(hwnd, |state| {
            let doc = state.docs.get(state.current);
            let hwnd_edit = doc.map(|d| d.hwnd_edit).unwrap_or(HWND(0));
            let is_audiobook = doc
                .map(|d| matches!(d.format, FileFormat::Audiobook))
                .unwrap_or(false);
            (hwnd_edit, is_audiobook)
        })
        .unwrap_or((HWND(0), false));
        if is_audiobook {
            hwnd_edit = hwnd_tab;
        }
        let speed_control = if manual_tuning {
            edit_speed
        } else {
            combo_speed
        };
        let pitch_control = if manual_tuning {
            edit_pitch
        } else {
            combo_pitch
        };
        let volume_control = if manual_tuning {
            edit_volume
        } else {
            combo_volume
        };
        if focus != hwnd_edit
            && focus != combo_engine
            && focus != combo_language
            && focus != combo_voice
            && focus != button_insert_tag
            && !(is_google && focus == button_manage_google_voices)
            && focus != button_insert_pause
            && focus != speed_control
            && focus != pitch_control
            && focus != volume_control
            && focus != hwnd_tab
            && !(is_edge && focus == checkbox_multilingual)
            && !(favorites_visible && focus == combo_favorites)
        {
            return false;
        }
        let shift_down = (GetKeyState(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
        if focus == hwnd_edit || focus == hwnd_tab {
            if visible {
                set_focus_safe(combo_engine);
            } else if favorites_visible {
                set_focus_safe(combo_favorites);
            }
            return true;
        }
        let fallback_edit = if hwnd_edit.0 != 0 {
            hwnd_edit
        } else {
            hwnd_tab
        };
        let mut order = Vec::new();
        if visible {
            order.push(combo_engine);
            if is_edge && !only_multilingual {
                order.push(combo_language);
            }
            order.push(combo_voice);
            order.push(button_insert_tag);
            if is_google {
                order.push(button_manage_google_voices);
            }
            order.push(button_insert_pause);
            order.push(speed_control);
            order.push(pitch_control);
            order.push(volume_control);
            if is_edge {
                order.push(checkbox_multilingual);
            }
        }
        if favorites_visible {
            order.push(combo_favorites);
        }
        let Some(idx) = order.iter().position(|item| *item == focus) else {
            return false;
        };
        if shift_down {
            if idx == 0 {
                if fallback_edit.0 != 0 {
                    set_focus_safe(fallback_edit);
                    return true;
                }
                return false;
            }
            let target = order[idx - 1];
            if target.0 != 0 {
                set_focus_safe(target);
                return true;
            }
        } else {
            if idx + 1 >= order.len() {
                if fallback_edit.0 != 0 {
                    set_focus_safe(fallback_edit);
                    return true;
                }
                return false;
            }
            let target = order[idx + 1];
            if target.0 != 0 {
                set_focus_safe(target);
                return true;
            }
        }
        false
    }
}

fn is_modifier_vk(key: u16) -> bool {
    matches!(key, 0x10 | 0x11 | 0x12 | 0xA0..=0xA5)
}

fn appcommand_from_lparam(lparam: LPARAM) -> usize {
    ((lparam.0 as usize) >> 16) & 0x7ff
}

fn shortcut_matches_message(binding: ShortcutBinding, msg: &MSG) -> bool {
    if msg.message != WM_KEYDOWN && msg.message != WM_SYSKEYDOWN {
        return false;
    }
    let key = msg.wParam.0 as u16;
    if is_modifier_vk(key) {
        return false;
    }
    // SAFETY: key-state reads are thread-local Win32 queries with no aliasing requirements.
    let ctrl_down = (crate::get_key_state_safe(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
    // SAFETY: key-state reads are thread-local Win32 queries with no aliasing requirements.
    let shift_down = (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
    // SAFETY: key-state reads are thread-local Win32 queries with no aliasing requirements.
    let alt_down = (crate::get_key_state_safe(VK_MENU.0 as i32) & (0x8000u16 as i16)) != 0;
    key == binding.key
        && ctrl_down == binding.ctrl
        && shift_down == binding.shift
        && alt_down == binding.alt
}

fn dispatch_shortcut_command(hwnd: HWND, cmd: usize) {
    unsafe {
        crate::log_if_err!(PostMessageW(hwnd, WM_COMMAND, WPARAM(cmd), LPARAM(0)));
    }
}

fn log_alt_raw_key_probe(hwnd: HWND, msg: &MSG) {
    let is_key_message = matches!(
        msg.message,
        WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP | WM_CHAR | WM_SYSCHAR
    );
    if !is_key_message {
        return;
    }
    let ctrl_down = (crate::get_key_state_safe(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
    let shift_down = (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
    let alt_down = (crate::get_key_state_safe(VK_MENU.0 as i32) & (0x8000u16 as i16)) != 0;
    let (suppressed, used_with_key) = with_state(hwnd, |state| {
        (state.alt_menu_suppressed, state.alt_menu_used_with_key)
    })
    .unwrap_or((false, false));
    let key = msg.wParam.0 as u32;
    if alt_down || suppressed || key == u32::from(b'R') || key == u32::from(b'r') {
        crate::log_debug(&format!(
            "shortcut raw: msg={} key={} ctrl={} shift={} alt={} suppressed={} used={} msg_hwnd={:?} focus={:?}",
            msg.message,
            key,
            ctrl_down,
            shift_down,
            alt_down,
            suppressed,
            used_with_key,
            msg.hwnd,
            crate::get_focus_safe()
        ));
    }
}

fn log_alt_shift_shortcut_probe(
    msg: &MSG,
    key: u16,
    ctrl_down: bool,
    shift_down: bool,
    alt_down: bool,
    shortcuts: &ShortcutSettings,
) {
    if !ctrl_down && shift_down && alt_down && (key == 'R' as u16 || key == 'N' as u16) {
        crate::log_debug(&format!(
            "shortcut probe: key={} msg={} msg_hwnd={:?} focus={:?} radio_binding=ctrl:{} shift:{} alt:{} key:{} paths_binding=ctrl:{} shift:{} alt:{} key:{} radio_match={} paths_match={}",
            key as u8 as char,
            msg.message,
            msg.hwnd,
            crate::get_focus_safe(),
            shortcuts.open_radio.ctrl,
            shortcuts.open_radio.shift,
            shortcuts.open_radio.alt,
            shortcuts.open_radio.key,
            shortcuts.open_paths_navigation.ctrl,
            shortcuts.open_paths_navigation.shift,
            shortcuts.open_paths_navigation.alt,
            shortcuts.open_paths_navigation.key,
            shortcut_matches_message(shortcuts.open_radio, msg),
            shortcut_matches_message(shortcuts.open_paths_navigation, msg)
        ));
    }
}

fn handle_custom_shortcuts(hwnd: HWND, msg: &MSG) -> bool {
    if msg.message != WM_KEYDOWN && msg.message != WM_SYSKEYDOWN {
        return false;
    }
    let options_hwnd = { with_state(hwnd, |state| state.options_dialog) }.unwrap_or(HWND(0));
    if options_hwnd.0 != 0
        && (msg.hwnd == options_hwnd || crate::is_child_safe(options_hwnd, msg.hwnd))
    {
        return false;
    }

    let shortcuts = { with_state(hwnd, |state| state.settings.shortcuts.clone()) }
        .unwrap_or_else(ShortcutSettings::default);

    // Dedicated shortcut requested for streaming audio.
    // NOTE: this intentionally takes precedence over the accelerator table.
    let key = msg.wParam.0 as u16;
    let ctrl_down = (crate::get_key_state_safe(VK_CONTROL.0 as i32) & (0x8000u16 as i16)) != 0;
    let shift_down = (crate::get_key_state_safe(VK_SHIFT.0 as i32) & (0x8000u16 as i16)) != 0;
    let alt_down = (crate::get_key_state_safe(VK_MENU.0 as i32) & (0x8000u16 as i16)) != 0;
    log_alt_shift_shortcut_probe(msg, key, ctrl_down, shift_down, alt_down, &shortcuts);
    let has_active_player = with_state(hwnd, |state| {
        state.active_audiobook.is_some() || state.active_mpv_session.is_some()
    })
    .unwrap_or(false);
    if key == 'I' as u16 && ctrl_down && shift_down && !alt_down {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_CREATE_AUDIO_DESCRIPTION);
        return true;
    }
    if key == 'B' as u16 && !ctrl_down && shift_down && alt_down {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_BDCIECHI);
        return true;
    }
    if key == 'U' as u16 && !ctrl_down && shift_down && alt_down {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_GUTENBERG);
        return true;
    }
    if key == 'I' as u16 && !ctrl_down && shift_down && alt_down {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_INTERNET_ARCHIVE);
        return true;
    }
    if key == 'V' as u16 && ctrl_down && shift_down && !alt_down {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_LIBRIVOX);
        return true;
    }
    if key == 'S' as u16 && !ctrl_down && shift_down && alt_down {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_STREAM_AUDIO);
        return true;
    }
    if key == 'P' as u16 && !ctrl_down && shift_down && alt_down {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_RAIPLAY);
        return true;
    }
    if key == 'L' as u16 && ctrl_down && !shift_down && !alt_down {
        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
        if language == Language::Italian {
            dispatch_shortcut_command(hwnd, IDM_TOOLS_LA7_PLAY);
            return true;
        }
        return false;
    }
    if key == 'V' as u16 && !ctrl_down && shift_down && alt_down {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_TV);
        return true;
    }
    if key == 'T' as u16 && !ctrl_down && shift_down && alt_down {
        dispatch_shortcut_command(hwnd, IDM_PLAYBACK_TRANSCRIBE_CURRENT);
        return true;
    }
    if key == 'C' as u16 && !ctrl_down && shift_down && alt_down {
        dispatch_shortcut_command(hwnd, IDM_PLAYBACK_TRANSCRIBE_CURRENT_FOLDER);
        return true;
    }
    if key == 'E' as u16 && !ctrl_down && shift_down && alt_down {
        if !is_live_tv_playback_active(hwnd) {
            dispatch_shortcut_command(hwnd, IDM_PLAYBACK_DOWNLOAD_EPISODE);
        }
        return true;
    }
    if key == 'L' as u16 && !ctrl_down && shift_down && alt_down {
        if has_active_player {
            dispatch_shortcut_command(hwnd, IDM_PLAYBACK_CHAPTER_LIST);
            return true;
        }
        return false;
    }
    if key == 'A' as u16 && !ctrl_down && shift_down && alt_down {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_RAI_AUDIODESCRIZIONI);
        return true;
    }
    if key == 'G' as u16 && !ctrl_down && shift_down && alt_down {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_ITALIAONLINE);
        return true;
    }
    if key == 'E' as u16 && ctrl_down && shift_down && !alt_down {
        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
        if language == Language::Italian {
            dispatch_shortcut_command(hwnd, IDM_TOOLS_TRECCANI);
            return true;
        }
    }
    if key == 'S' as u16 && ctrl_down && shift_down && !alt_down {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_RAIPLAYSOUND);
        return true;
    }
    if shortcut_matches_message(shortcuts.read_pause_resume, msg) {
        dispatch_shortcut_command(hwnd, IDM_FILE_READ_PAUSE);
        return true;
    }
    if shortcut_matches_message(shortcuts.read_start, msg) {
        dispatch_shortcut_command(hwnd, IDM_FILE_READ_START);
        return true;
    }
    if shortcut_matches_message(shortcuts.read_previous_sentence, msg) {
        dispatch_shortcut_command(hwnd, IDM_FILE_READ_PREVIOUS_SENTENCE);
        return true;
    }
    if shortcut_matches_message(shortcuts.read_next_sentence, msg) {
        dispatch_shortcut_command(hwnd, IDM_FILE_READ_NEXT_SENTENCE);
        return true;
    }
    if shortcut_matches_message(shortcuts.read_stop, msg) {
        dispatch_shortcut_command(hwnd, IDM_FILE_READ_STOP);
        return true;
    }
    if shortcut_matches_message(shortcuts.execute_file, msg) {
        dispatch_shortcut_command(hwnd, IDM_FILE_EXECUTE);
        return true;
    }
    if shortcut_matches_message(shortcuts.audiobook, msg) {
        dispatch_shortcut_command(hwnd, IDM_FILE_AUDIOBOOK);
        return true;
    }
    if shortcut_matches_message(shortcuts.batch_audiobooks, msg) {
        dispatch_shortcut_command(hwnd, IDM_FILE_BATCH_AUDIOBOOK);
        return true;
    }
    if shortcut_matches_message(shortcuts.record_podcast, msg) {
        dispatch_shortcut_command(hwnd, IDM_FILE_PODCAST);
        return true;
    }
    if shortcut_matches_message(shortcuts.dictation, msg) {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_TOGGLE_DICTATION);
        return true;
    }
    if shortcut_matches_message(shortcuts.convert_audio, msg) {
        dispatch_shortcut_command(hwnd, IDM_FILE_CONVERT_AUDIO);
        return true;
    }
    if shortcut_matches_message(shortcuts.open_rss, msg) {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_RSS);
        return true;
    }
    if shortcut_matches_message(shortcuts.open_podcasts, msg) {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_PODCASTS);
        return true;
    }
    if shortcut_matches_message(shortcuts.open_paths_navigation, msg) {
        if key == 'N' as u16 && !ctrl_down && shift_down && alt_down {
            crate::log_debug("shortcut probe: dispatch Alt+Shift+N -> paths navigation");
        }
        dispatch_shortcut_command(hwnd, IDM_TOOLS_PATHS_NAVIGATION);
        return true;
    }
    if shortcut_matches_message(shortcuts.open_radio, msg) {
        if key == 'R' as u16 && !ctrl_down && shift_down && alt_down {
            crate::log_debug("shortcut probe: dispatch Alt+Shift+R -> radio");
        }
        dispatch_shortcut_command(hwnd, IDM_TOOLS_RADIO);
        return true;
    }
    if shortcut_matches_message(shortcuts.open_calendar, msg) {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_CALENDAR);
        return true;
    }
    if shortcut_matches_message(shortcuts.open_weather, msg) {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_WEATHER);
        return true;
    }
    if shortcut_matches_message(shortcuts.open_cinema, msg) {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_CINEMA);
        return true;
    }
    if shortcut_matches_message(shortcuts.open_dictionary, msg) {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_DICTIONARY);
        return true;
    }
    if shortcut_matches_message(shortcuts.open_options, msg) {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_OPTIONS);
        return true;
    }
    if shortcut_matches_message(shortcuts.open_terminal, msg) {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_PROMPT);
        return true;
    }
    if shortcut_matches_message(shortcuts.import_wikipedia, msg) {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_WIKIPEDIA_IMPORT);
        return true;
    }
    if shortcut_matches_message(shortcuts.import_youtube, msg) {
        dispatch_shortcut_command(hwnd, IDM_TOOLS_IMPORT_YOUTUBE);
        return true;
    }
    if shortcut_matches_message(shortcuts.find, msg) {
        dispatch_shortcut_command(hwnd, IDM_EDIT_FIND);
        return true;
    }
    if shortcut_matches_message(shortcuts.quote_lines, msg) {
        dispatch_shortcut_command(hwnd, IDM_EDIT_QUOTE_LINES);
        return true;
    }
    if shortcut_matches_message(shortcuts.unquote_lines, msg) {
        dispatch_shortcut_command(hwnd, IDM_EDIT_UNQUOTE_LINES);
        return true;
    }
    if shortcut_matches_message(shortcuts.media_prev, msg) {
        dispatch_shortcut_command(hwnd, IDM_PLAYBACK_TRACK_PREV);
        return true;
    }
    if shortcut_matches_message(shortcuts.media_next, msg) {
        dispatch_shortcut_command(hwnd, IDM_PLAYBACK_TRACK_NEXT);
        return true;
    }
    if shortcut_matches_message(shortcuts.chapter_prev, msg) {
        dispatch_shortcut_command(hwnd, IDM_PLAYBACK_CHAPTER_PREV);
        return true;
    }
    if shortcut_matches_message(shortcuts.chapter_next, msg) {
        dispatch_shortcut_command(hwnd, IDM_PLAYBACK_CHAPTER_NEXT);
        return true;
    }
    if !ctrl_down && shift_down && alt_down && (key == 'R' as u16 || key == 'N' as u16) {
        crate::log_debug(
            "shortcut probe: no custom shortcut matched, passing to accelerator/dispatch",
        );
    }
    false
}

fn create_accelerators() -> HACCEL {
    unsafe {
        let virt = FCONTROL | FVIRTKEY;
        let virt_shift = FCONTROL | FSHIFT | FVIRTKEY;
        let virt_shift_only = FSHIFT | FVIRTKEY;
        let virt_alt = FALT | FVIRTKEY;
        let virt_alt_shift = FALT | FSHIFT | FVIRTKEY;
        let accels = [
            ACCEL {
                fVirt: virt,
                key: 'N' as u16,
                cmd: IDM_FILE_NEW as u16,
            },
            ACCEL {
                fVirt: virt,
                key: 'O' as u16,
                cmd: IDM_FILE_OPEN as u16,
            },
            ACCEL {
                fVirt: virt,
                key: 'S' as u16,
                cmd: IDM_FILE_SAVE as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: 'S' as u16,
                cmd: IDM_FILE_SAVE_ALL as u16,
            },
            ACCEL {
                fVirt: virt,
                key: 'P' as u16,
                cmd: IDM_FILE_PRINT as u16,
            },
            ACCEL {
                fVirt: virt,
                key: 'W' as u16,
                cmd: IDM_FILE_CLOSE as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: 'W' as u16,
                cmd: IDM_FILE_CLOSE_OTHERS as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: 'F' as u16,
                cmd: IDM_EDIT_FIND_IN_FILES as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: 'M' as u16,
                cmd: IDM_EDIT_STRIP_MARKDOWN as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: 'I' as u16,
                cmd: IDM_TOOLS_CREATE_AUDIO_DESCRIPTION as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: 'H' as u16,
                cmd: IDM_EDIT_HARD_LINE_BREAK as u16,
            },
            ACCEL {
                fVirt: virt_alt_shift,
                key: 'O' as u16,
                cmd: IDM_EDIT_ORDER_ITEMS as u16,
            },
            ACCEL {
                fVirt: virt_alt_shift,
                key: 'K' as u16,
                cmd: IDM_EDIT_KEEP_UNIQUE_ITEMS as u16,
            },
            ACCEL {
                fVirt: virt_alt_shift,
                key: 'Z' as u16,
                cmd: IDM_EDIT_REVERSE_ITEMS as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: VK_RETURN.0,
                cmd: IDM_EDIT_NORMALIZE_WHITESPACE as u16,
            },
            ACCEL {
                fVirt: FVIRTKEY,
                key: VK_F3.0,
                cmd: IDM_EDIT_FIND_NEXT as u16,
            },
            ACCEL {
                fVirt: virt_shift_only,
                key: VK_F3.0,
                cmd: IDM_EDIT_FIND_PREVIOUS as u16,
            },
            ACCEL {
                fVirt: virt,
                key: 'H' as u16,
                cmd: IDM_EDIT_REPLACE as u16,
            },
            ACCEL {
                fVirt: virt,
                key: 'J' as u16,
                cmd: IDM_EDIT_GO_TO_LINE as u16,
            },
            ACCEL {
                fVirt: virt,
                key: 'A' as u16,
                cmd: IDM_EDIT_SELECT_ALL as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: VK_OEM_PERIOD.0,
                cmd: IDM_EDIT_INDENT as u16,
            },
            ACCEL {
                fVirt: virt,
                key: VK_OEM_PERIOD.0,
                cmd: IDM_EDIT_INSERT_ELLIPSIS as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: VK_OEM_COMMA.0,
                cmd: IDM_EDIT_OUTDENT as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: 'J' as u16,
                cmd: IDM_EDIT_JOIN_LINES as u16,
            },
            ACCEL {
                fVirt: virt_alt,
                key: 'Y' as u16,
                cmd: IDM_EDIT_TEXT_STATS as u16,
            },
            ACCEL {
                fVirt: virt,
                key: 'D' as u16,
                cmd: IDM_EDIT_REMOVE_DUPLICATE_LINES as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: 'C' as u16,
                cmd: IDM_EDIT_REMOVE_DUPLICATE_CONSECUTIVE_LINES as u16,
            },
            ACCEL {
                fVirt: virt_alt_shift,
                key: 'H' as u16,
                cmd: IDM_EDIT_CLEAN_EOL_HYPHENS as u16,
            },
            ACCEL {
                fVirt: virt_alt_shift,
                key: 'D' as u16,
                cmd: IDM_TOOLS_DICTIONARY_LOOKUP as u16,
            },
            ACCEL {
                fVirt: virt_alt_shift,
                key: 'A' as u16,
                cmd: IDM_TOOLS_RAI_AUDIODESCRIZIONI as u16,
            },
            ACCEL {
                fVirt: virt,
                key: VK_TAB.0,
                cmd: IDM_NEXT_TAB as u16,
            },
            ACCEL {
                fVirt: virt_shift_only,
                key: VK_NEXT.0,
                cmd: IDM_GOTO_NEXT_BOOKMARK as u16,
            },
            ACCEL {
                fVirt: virt_shift_only,
                key: VK_PRIOR.0,
                cmd: IDM_GOTO_PREV_BOOKMARK as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: 'G' as u16,
                cmd: IDM_MANAGE_BOOKMARKS as u16,
            },
            ACCEL {
                fVirt: virt_shift,
                key: 'L' as u16,
                cmd: IDM_INSERT_CLEAR_BOOKMARKS as u16,
            },
            ACCEL {
                fVirt: virt,
                key: 'B' as u16,
                cmd: IDM_INSERT_BOOKMARK as u16,
            },
        ];
        CreateAcceleratorTableW(&accels).unwrap_or(HACCEL(0))
    }
}

enum EnumWindowsRequest {
    Idle,
    CloseOthers { current: isize },
    FindProcessTitle { pid: u32, best: Option<String> },
    CountMainWindows { exclude: isize, count: usize },
}

static ENUM_WINDOWS_REQUEST: LazyLock<Mutex<EnumWindowsRequest>> =
    LazyLock::new(|| Mutex::new(EnumWindowsRequest::Idle));

unsafe extern "system" fn enum_close_other_windows(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    unsafe {
        crate::panic_guard::guard(
            "enum_close_other_windows",
            || BOOL(1),
            || {
                let mut request = ENUM_WINDOWS_REQUEST
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                match &mut *request {
                    EnumWindowsRequest::Idle => BOOL(1),
                    EnumWindowsRequest::CloseOthers { current } => {
                        if hwnd == HWND(*current) {
                            return BOOL(1);
                        }
                        let mut buf = [0u16; 64];
                        let len = GetClassNameW(hwnd, &mut buf);
                        if len == 0 {
                            return BOOL(1);
                        }
                        let name = String::from_utf16_lossy(&buf[..len as usize]);
                        if name == "SonarpadWin32" {
                            crate::log_if_err!(PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)));
                        }
                        BOOL(1)
                    }
                    EnumWindowsRequest::FindProcessTitle { pid, best } => {
                        if !IsWindowVisible(hwnd).as_bool() {
                            return BOOL(1);
                        }
                        let mut window_pid = 0u32;
                        let _thread = GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
                        if window_pid != *pid {
                            return BOOL(1);
                        }
                        let text_len = get_window_text_length_w_safe(hwnd);
                        if text_len <= 0 {
                            return BOOL(1);
                        }
                        let mut text_buf = vec![0u16; text_len as usize + 1];
                        let read = get_window_text_w_safe(hwnd, &mut text_buf);
                        if read <= 0 {
                            return BOOL(1);
                        }
                        let title = String::from_utf16_lossy(&text_buf[..read as usize])
                            .trim()
                            .to_string();
                        if title.is_empty() {
                            return BOOL(1);
                        }
                        let should_replace =
                            best.as_ref().map(|current| title.len() > current.len()) != Some(false);
                        if should_replace {
                            *best = Some(title);
                        }
                        BOOL(1)
                    }
                    EnumWindowsRequest::CountMainWindows { exclude, count } => {
                        if hwnd == HWND(*exclude) || !IsWindowVisible(hwnd).as_bool() {
                            return BOOL(1);
                        }
                        let mut window_pid = 0u32;
                        let _thread = GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
                        if window_pid == 0 || window_pid != GetCurrentProcessId() {
                            return BOOL(1);
                        }
                        let mut buf = [0u16; 64];
                        let len = GetClassNameW(hwnd, &mut buf);
                        if len == 0 {
                            return BOOL(1);
                        }
                        let name = String::from_utf16_lossy(&buf[..len as usize]);
                        if name == "SonarpadWin32" {
                            *count += 1;
                        }
                        BOOL(1)
                    }
                }
            },
        )
    }
}

fn close_other_windows(hwnd: HWND) {
    {
        let mut request = ENUM_WINDOWS_REQUEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *request = EnumWindowsRequest::CloseOthers { current: hwnd.0 };
    }
    unsafe {
        crate::log_if_err!(EnumWindows(Some(enum_close_other_windows), LPARAM(0)));
    }
    let mut request = ENUM_WINDOWS_REQUEST
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *request = EnumWindowsRequest::Idle;
}

pub(crate) fn find_process_window_title(process_id: u32) -> Option<String> {
    {
        let mut request = ENUM_WINDOWS_REQUEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *request = EnumWindowsRequest::FindProcessTitle {
            pid: process_id,
            best: None,
        };
    }
    unsafe {
        crate::log_if_err!(EnumWindows(Some(enum_close_other_windows), LPARAM(0)));
    }
    let mut request = ENUM_WINDOWS_REQUEST
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    match std::mem::replace(&mut *request, EnumWindowsRequest::Idle) {
        EnumWindowsRequest::FindProcessTitle { best, .. } => best,
        _ => None,
    }
}

fn has_other_main_windows(current: HWND) -> bool {
    {
        let mut request = ENUM_WINDOWS_REQUEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *request = EnumWindowsRequest::CountMainWindows {
            exclude: current.0,
            count: 0,
        };
    }
    unsafe {
        crate::log_if_err!(EnumWindows(Some(enum_close_other_windows), LPARAM(0)));
    }
    let mut request = ENUM_WINDOWS_REQUEST
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    match std::mem::replace(&mut *request, EnumWindowsRequest::Idle) {
        EnumWindowsRequest::CountMainWindows { count, .. } => count > 0,
        _ => false,
    }
}

pub(crate) fn get_active_edit(hwnd: HWND) -> Option<HWND> {
    {
        with_state(hwnd, |state| {
            state.docs.get(state.current).map(|doc| doc.hwnd_edit)
        })
        .flatten()
    }
}

const UNSAVED_BOOKMARK_PREFIX: &str = "__unsaved__:";
const STREAM_BOOKMARK_PREFIX: &str = "__stream_title__:";

fn bookmark_storage_key(path: Option<&Path>, hwnd_edit: HWND) -> (String, bool) {
    if let Some(path) = path {
        (path.to_string_lossy().to_string(), true)
    } else {
        (format!("{UNSAVED_BOOKMARK_PREFIX}{}", hwnd_edit.0), false)
    }
}

pub(crate) fn runtime_bookmark_storage_key(
    path: Option<&Path>,
    hwnd_edit: HWND,
    title: &str,
    format: FileFormat,
) -> (String, bool) {
    if matches!(format, FileFormat::Audiobook)
        && let Some(path) = path
        && !path.is_file()
        && !title.trim().is_empty()
    {
        return (format!("{STREAM_BOOKMARK_PREFIX}{}", title.trim()), true);
    }
    bookmark_storage_key(path, hwnd_edit)
}

fn current_edit_caret_position(hwnd_edit: HWND) -> i32 {
    let mut cr = CHARRANGE { cpMin: 0, cpMax: 0 };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXGETSEL,
            WPARAM(0),
            LPARAM(&mut cr as *mut _ as isize),
        );
    }
    cr.cpMax.max(0)
}

fn text_bookmark_snippet(hwnd_edit: HWND, pos: i32) -> String {
    let mut buffer = vec![0u16; 62];
    let mut tr = TEXTRANGEW {
        chrg: CHARRANGE {
            cpMin: pos,
            cpMax: pos + 60,
        },
        lpstrText: PWSTR(buffer.as_mut_ptr()),
    };
    let copied = unsafe {
        SendMessageW(
            hwnd_edit,
            EM_GETTEXTRANGE,
            WPARAM(0),
            LPARAM(&mut tr as *mut _ as isize),
        )
        .0 as usize
    };
    let mut snippet = String::from_utf16_lossy(&buffer[..copied]);
    if let Some(idx) = snippet.find(['\r', '\n']) {
        snippet.truncate(idx);
    }
    if snippet.trim().is_empty() && pos > 0 {
        let start_pre = (pos - 60).max(0);
        let mut buffer_pre = vec![0u16; 62];
        let mut tr_pre = TEXTRANGEW {
            chrg: CHARRANGE {
                cpMin: start_pre,
                cpMax: pos,
            },
            lpstrText: PWSTR(buffer_pre.as_mut_ptr()),
        };
        let copied_pre = unsafe {
            SendMessageW(
                hwnd_edit,
                EM_GETTEXTRANGE,
                WPARAM(0),
                LPARAM(&mut tr_pre as *mut _ as isize),
            )
            .0 as usize
        };
        let mut snippet_pre = String::from_utf16_lossy(&buffer_pre[..copied_pre]);
        if let Some(idx) = snippet_pre.rfind(['\r', '\n']) {
            snippet_pre = snippet_pre[idx + 1..].to_string();
        }
        snippet = snippet_pre;
    }
    snippet.trim().to_string()
}

fn text_bookmark_at_position(hwnd_edit: HWND, pos: i32) -> Bookmark {
    Bookmark::manual(
        pos.max(0),
        text_bookmark_snippet(hwnd_edit, pos.max(0)),
        Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    )
}

pub(crate) fn automatic_media_bookmark_reached_end(
    position_secs: u64,
    duration_secs: Option<u64>,
) -> bool {
    const END_TOLERANCE_SECS: u64 = 2;
    duration_secs
        .filter(|duration| *duration > 0)
        .is_some_and(|duration| position_secs.saturating_add(END_TOLERANCE_SECS) >= duration)
}

fn current_text_bookmark_position(hwnd: HWND, doc_index: usize, hwnd_edit: HWND) -> i32 {
    with_state(hwnd, |state| {
        if state.current == doc_index
            && let Some(doc) = state.docs.get(doc_index)
            && doc.hwnd_edit == hwnd_edit
        {
            let (storage_key, _) = runtime_bookmark_storage_key(
                doc.path.as_deref(),
                doc.hwnd_edit,
                &doc.title,
                doc.format,
            );
            if let Some((bookmark_hwnd, bookmark_key, bookmark_pos)) =
                &state.tts_automatic_bookmark_position
                && *bookmark_hwnd == hwnd_edit
                && *bookmark_key == storage_key
            {
                return (*bookmark_pos).max(0);
            }
            if let Some(pending) = state.tts_pending_start_pos {
                return pending.max(0);
            }
            if let Some(session) = &state.tts_session {
                return (session.initial_caret_pos + state.tts_last_offset).max(0);
            }
        }
        spellcheck_caret_char_index(hwnd_edit)
            .unwrap_or_else(|| current_edit_caret_position(hwnd_edit))
    })
    .unwrap_or_else(|| current_edit_caret_position(hwnd_edit))
}

fn clear_stale_tts_automatic_bookmark_for_edit(hwnd: HWND, hwnd_edit: HWND) {
    let caret_pos = spellcheck_caret_char_index(hwnd_edit)
        .unwrap_or_else(|| current_edit_caret_position(hwnd_edit));
    with_state(hwnd, |state| {
        let should_clear = state.tts_session.is_none()
            && state.tts_pending_start_pos.is_none()
            && state.tts_automatic_bookmark_position.as_ref().is_some_and(
                |(bookmark_hwnd, _, bookmark_pos)| {
                    *bookmark_hwnd == hwnd_edit && caret_pos != (*bookmark_pos).max(0)
                },
            );
        if should_clear {
            state.tts_automatic_bookmark_position = None;
        }
    });
}

fn clear_automatic_bookmark_for_storage_key(
    hwnd: HWND,
    storage_key: &str,
    hwnd_edit: HWND,
) -> bool {
    let (bookmarks_window, removed) = with_state(hwnd, |state| {
        let mut removed = false;
        let remove_empty = if let Some(list) = state.bookmarks.files.get_mut(storage_key) {
            let before = list.len();
            list.retain(|bookmark| !bookmark.automatic);
            removed = list.len() != before;
            list.is_empty()
        } else {
            false
        };
        if remove_empty {
            state.bookmarks.files.remove(storage_key);
        }
        if removed {
            save_bookmarks(&state.bookmarks);
            let should_clear_tts_bookmark = state
                .tts_automatic_bookmark_position
                .as_ref()
                .is_some_and(|(bookmark_hwnd, bookmark_key, _)| {
                    *bookmark_hwnd == hwnd_edit && bookmark_key == storage_key
                });
            if should_clear_tts_bookmark {
                state.tts_automatic_bookmark_position = None;
            }
        }
        (state.bookmarks_window, removed)
    })
    .unwrap_or((HWND(0), false));

    if removed && bookmarks_window.0 != 0 {
        app_windows::bookmarks_window::refresh_bookmarks_list(bookmarks_window);
    }
    removed
}

pub(crate) fn save_automatic_bookmark_for_document(hwnd: HWND, doc_index: usize) -> bool {
    let Some((hwnd_edit, path, format, title, automatic_enabled)) = with_state(hwnd, |state| {
        state.docs.get(doc_index).map(|doc| {
            (
                doc.hwnd_edit,
                doc.path.clone(),
                doc.format,
                doc.title.clone(),
                state.settings.automatic_bookmark,
            )
        })
    })
    .flatten() else {
        return false;
    };
    if !automatic_enabled || hwnd_edit.0 == 0 {
        return false;
    }
    if matches!(format, FileFormat::Audiobook)
        && daisy_player::is_active_audio_document_path(path.as_deref())
    {
        return false;
    }

    let (storage_key, persist_to_disk) =
        runtime_bookmark_storage_key(path.as_deref(), hwnd_edit, &title, format);
    if !persist_to_disk {
        return false;
    }

    let bookmark = if matches!(format, FileFormat::Audiobook) {
        let mpv_position_secs = path
            .as_deref()
            .and_then(|bookmark_path| local_mpv_position_secs_for_path(hwnd, bookmark_path));
        let mpv_duration_secs = path
            .as_deref()
            .and_then(|bookmark_path| local_mpv_duration_secs_for_path(hwnd, bookmark_path));
        let Some((pos, snippet)) = with_state(hwnd, |state| {
            if let Some(position_secs) = mpv_position_secs {
                return Some(audio_bookmark_position_and_snippet(position_secs));
            }
            if let (Some(player), Some(bookmark_path)) =
                (&mut state.active_audiobook, path.as_ref())
                && player.path == *bookmark_path
            {
                let position_secs = crate::audio_player::audiobook_position_secs(player);
                return Some(audio_bookmark_position_and_snippet(position_secs));
            }
            if let Some(bookmark_path) = path.as_ref() {
                let path_key = bookmark_path.to_string_lossy();
                if state.last_stopped_mpv_url.as_deref() == Some(path_key.as_ref())
                    && let Some(position_secs) = state.last_stopped_mpv_position_secs
                {
                    return Some(audio_bookmark_position_and_snippet(position_secs as f64));
                }
            }
            state
                .active_audiobook_bookmark
                .as_ref()
                .and_then(|(stored_key, position)| {
                    (stored_key == &storage_key)
                        .then(|| audio_bookmark_position_and_snippet(*position as f64))
                })
        })
        .flatten() else {
            return false;
        };
        let duration_secs = mpv_duration_secs
            .map(|duration| duration.max(0.0).floor() as u64)
            .or_else(|| {
                path.as_ref()
                    .and_then(|bookmark_path| audiobook_duration_secs(bookmark_path))
            })
            .or_else(|| {
                with_state(hwnd, |state| {
                    let bookmark_path = path.as_ref()?;
                    let player = state.active_audiobook.as_ref()?;
                    (player.path == *bookmark_path).then(|| {
                        player
                            .duration_secs()
                            .map(|secs| secs.max(0.0).floor() as u64)
                    })?
                })
                .flatten()
            });
        if automatic_media_bookmark_reached_end(pos.max(0) as u64, duration_secs) {
            crate::log_debug(&format!(
                "Automatic media bookmark reached end, clearing: key={} pos={} duration={:?}",
                storage_key, pos, duration_secs
            ));
            return clear_automatic_bookmark_for_storage_key(hwnd, &storage_key, hwnd_edit);
        }
        Bookmark::automatic(
            pos,
            snippet,
            Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        )
    } else {
        let pos = current_text_bookmark_position(hwnd, doc_index, hwnd_edit);
        Bookmark::automatic(
            pos.max(0),
            text_bookmark_snippet(hwnd_edit, pos.max(0)),
            Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        )
    };

    let bookmarks_window = with_state(hwnd, |state| {
        state
            .bookmarks
            .files
            .insert(storage_key.clone(), vec![bookmark]);
        save_bookmarks(&state.bookmarks);
        let should_clear_tts_bookmark = state.tts_automatic_bookmark_position.as_ref().is_some_and(
            |(bookmark_hwnd, bookmark_key, _)| {
                *bookmark_hwnd == hwnd_edit && bookmark_key == &storage_key
            },
        );
        if should_clear_tts_bookmark {
            state.tts_automatic_bookmark_position = None;
        }
        state.bookmarks_window
    })
    .unwrap_or(HWND(0));

    if bookmarks_window.0 != 0 {
        app_windows::bookmarks_window::refresh_bookmarks_list(bookmarks_window);
    }
    true
}

fn save_automatic_bookmark_for_active_raiplaysound_episode(hwnd: HWND) -> bool {
    let doc_index = with_state(hwnd, |state| {
        if !state.settings.automatic_bookmark
            || state.active_podcast_episode_from_rai != RaiAudioOrigin::RaiPlaySound
        {
            return None;
        }
        let active_path = state
            .active_audiobook
            .as_ref()
            .map(|player| player.path.clone())
            .or_else(|| state.active_podcast_episode_url.as_ref().map(PathBuf::from))
            .or_else(|| state.active_podcast_episode_cache.clone())?;
        state.docs.iter().position(|doc| {
            matches!(doc.format, FileFormat::Audiobook)
                && doc.path.as_ref().is_some_and(|path| path == &active_path)
        })
    })
    .flatten();

    doc_index
        .map(|index| save_automatic_bookmark_for_document(hwnd, index))
        .unwrap_or(false)
}

fn insert_bookmark(hwnd: HWND) {
    let (hwnd_edit, path, format, title): (HWND, Option<std::path::PathBuf>, FileFormat, String) =
        with_state(hwnd, |state| {
            state.docs.get(state.current).map(|doc| {
                (
                    doc.hwnd_edit,
                    doc.path.clone(),
                    doc.format,
                    doc.title.clone(),
                )
            })
        })
        .flatten()
        .unwrap_or((HWND(0), None, FileFormat::default(), String::new()));
    if hwnd_edit.0 == 0 {
        return;
    }
    let (storage_key, persist_to_disk) =
        runtime_bookmark_storage_key(path.as_deref(), hwnd_edit, &title, format);

    if matches!(format, FileFormat::Audiobook) {
        let mpv_position_secs = path
            .as_deref()
            .and_then(|path| local_mpv_position_secs_for_path(hwnd, path));
        let (pos, snippet) = with_state(hwnd, |state| {
            if let Some(position_secs) = mpv_position_secs {
                audio_bookmark_position_and_snippet(position_secs)
            } else if let Some(player) = &mut state.active_audiobook {
                let position_secs = crate::audio_player::audiobook_position_secs(player);
                audio_bookmark_position_and_snippet(position_secs)
            } else {
                (0, "Audio non in riproduzione".to_string())
            }
        })
        .unwrap_or((0, String::new()));

        let bookmark = Bookmark::manual(
            pos,
            snippet,
            Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        );

        let (bookmarks_window, inserted) = with_state(hwnd, |state| {
            let automatic_bookmark_enabled = state.settings.automatic_bookmark;
            let list = state
                .bookmarks
                .files
                .entry(storage_key.clone())
                .or_default();
            if list.iter().any(|existing| {
                existing.position == bookmark.position
                    && existing.is_visible(automatic_bookmark_enabled)
            }) {
                return (state.bookmarks_window, false);
            }
            list.push(bookmark);
            crate::bookmarks::sort_bookmarks(list);
            if persist_to_disk {
                save_bookmarks(&state.bookmarks);
            }
            (state.bookmarks_window, true)
        })
        .unwrap_or((HWND(0), false));

        if inserted && bookmarks_window.0 != 0 {
            app_windows::bookmarks_window::refresh_bookmarks_list(bookmarks_window);
        }
        if inserted {
            confirm_menu_action(hwnd, "insert.bookmark");
        } else {
            crate::accessibility::screen_reader_speak(
                "Segnalibro già presente in questa posizione.",
            );
        }
        return;
    }

    let pos = current_edit_caret_position(hwnd_edit);
    let bookmark = text_bookmark_at_position(hwnd_edit, pos);

    let (bookmarks_window, inserted) = with_state(hwnd, |state| {
        let automatic_bookmark_enabled = state.settings.automatic_bookmark;
        let list = state
            .bookmarks
            .files
            .entry(storage_key.clone())
            .or_default();
        if list.iter().any(|existing| {
            existing.position == bookmark.position
                && existing.is_visible(automatic_bookmark_enabled)
        }) {
            return (state.bookmarks_window, false);
        }
        list.push(bookmark);
        crate::bookmarks::sort_bookmarks(list);
        if persist_to_disk {
            save_bookmarks(&state.bookmarks);
        }
        (state.bookmarks_window, true)
    })
    .unwrap_or((HWND(0), false));

    if inserted && bookmarks_window.0 != 0 {
        app_windows::bookmarks_window::refresh_bookmarks_list(bookmarks_window);
    }
    if inserted {
        confirm_menu_action(hwnd, "insert.bookmark");
    } else {
        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
        crate::accessibility::screen_reader_speak(&crate::i18n::tr(
            language,
            "bookmark.already_present",
        ));
    }
}

fn audio_bookmark_position_and_snippet(position_secs: f64) -> (i32, String) {
    let current_total = position_secs.max(0.0).floor() as u64;
    let mins = current_total / 60;
    let secs = current_total % 60;
    (
        current_total as i32,
        format!("Posizione audio: {:02}:{:02}", mins, secs),
    )
}

fn relative_audiobook_bookmark(
    bookmarks: &[Bookmark],
    anchor_position: Option<i32>,
    current_position_secs: f64,
    forward: bool,
) -> Option<&Bookmark> {
    if bookmarks.is_empty() {
        return None;
    }
    let current_index = anchor_position
        .and_then(|position| {
            bookmarks
                .iter()
                .position(|bookmark| bookmark.position == position)
        })
        .or_else(|| {
            let threshold = current_position_secs.floor() as i32;
            bookmarks
                .iter()
                .rposition(|bookmark| bookmark.position <= threshold)
        });
    match (current_index, forward) {
        (Some(index), true) => bookmarks.get(index + 1),
        (Some(index), false) => index.checked_sub(1).and_then(|prev| bookmarks.get(prev)),
        (None, true) => bookmarks.first(),
        (None, false) => None,
    }
}

fn goto_relative_bookmark(hwnd: HWND, forward: bool) -> bool {
    let (path, hwnd_edit, format, title): (Option<std::path::PathBuf>, HWND, FileFormat, String) =
        {
            with_state(hwnd, |state| {
                state.docs.get(state.current).map(|doc| {
                    (
                        doc.path.clone(),
                        doc.hwnd_edit,
                        doc.format,
                        doc.title.clone(),
                    )
                })
            })
        }
        .flatten()
        .unwrap_or((None, HWND(0), FileFormat::default(), String::new()));
    if hwnd_edit.0 == 0 {
        return false;
    }

    let (storage_key, _) = runtime_bookmark_storage_key(path.as_deref(), hwnd_edit, &title, format);
    let Some(bookmarks) = {
        with_state(hwnd, |state| {
            state.bookmarks.files.get(&storage_key).map(|list| {
                list.iter()
                    .filter(|bookmark| bookmark.is_visible(state.settings.automatic_bookmark))
                    .cloned()
                    .collect::<Vec<_>>()
            })
        })
    }
    .flatten() else {
        return false;
    };
    if bookmarks.is_empty() {
        return false;
    }

    let target = if matches!(format, FileFormat::Audiobook) {
        let mpv_position_secs = path
            .as_deref()
            .and_then(|path| local_mpv_position_secs_for_path(hwnd, path));
        let anchor_position = path.as_deref().and_then(|path| {
            let key = path.to_string_lossy().to_string();
            with_state(hwnd, |state| {
                state
                    .active_audiobook_bookmark
                    .as_ref()
                    .and_then(|(stored_key, position)| (stored_key == &key).then_some(*position))
            })
            .flatten()
        });
        let current_position_secs = {
            with_state(hwnd, |state| {
                if let Some(position_secs) = mpv_position_secs {
                    return Some(position_secs);
                }
                let current_path = path.as_ref()?;
                let active = state.active_audiobook.as_ref()?;
                if active.path != *current_path {
                    return Some(0.0);
                }
                Some(crate::audio_player::audiobook_position_secs(active))
            })
        }
        .flatten()
        .unwrap_or(0.0);
        relative_audiobook_bookmark(&bookmarks, anchor_position, current_position_secs, forward)
    } else {
        let mut cr = CHARRANGE { cpMin: 0, cpMax: 0 };
        unsafe {
            SendMessageW(
                hwnd_edit,
                EM_EXGETSEL,
                WPARAM(0),
                LPARAM(&mut cr as *mut _ as isize),
            );
        }
        let current_pos = if forward { cr.cpMax } else { cr.cpMin };
        if forward {
            bookmarks.iter().find(|bm| bm.position > current_pos)
        } else {
            bookmarks.iter().rev().find(|bm| bm.position < current_pos)
        }
    };
    let Some(target) = target else {
        return false;
    };
    let target_position = target.position;
    let target_snippet = target.snippet.trim().to_string();

    if matches!(format, FileFormat::Audiobook) {
        let Some(path) = path.as_ref() else {
            return false;
        };
        let target_seconds = target_position.max(0) as u64;
        jump_audiobook_to_position(hwnd, path, target_seconds);
        if !target_snippet.is_empty() {
            crate::accessibility::screen_reader_speak(&target_snippet);
        }
        return true;
    }

    let mut cr = CHARRANGE {
        cpMin: target_position,
        cpMax: target_position,
    };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_EXSETSEL,
            WPARAM(0),
            LPARAM(&mut cr as *mut _ as isize),
        );
        SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
        set_focus_safe(hwnd_edit);
    }
    announce_bookmark_target_line(hwnd_edit, target_position, &target_snippet);
    true
}

fn announce_bookmark_target_line(hwnd_edit: HWND, position: i32, fallback: &str) {
    let line = unsafe {
        SendMessageW(
            hwnd_edit,
            EM_LINEFROMCHAR,
            WPARAM(position.max(0) as usize),
            LPARAM(0),
        )
        .0 as i32
    };
    if line < 0 {
        if !fallback.is_empty() {
            crate::accessibility::screen_reader_speak(fallback);
        }
        return;
    }
    let line_start =
        send_message_w_safe(hwnd_edit, EM_LINEINDEX, WPARAM(line as usize), LPARAM(0)).0 as i32;
    if line_start < 0 {
        if !fallback.is_empty() {
            crate::accessibility::screen_reader_speak(fallback);
        }
        return;
    }
    let line_len = unsafe {
        SendMessageW(
            hwnd_edit,
            EM_LINELENGTH,
            WPARAM(line_start.max(0) as usize),
            LPARAM(0),
        )
        .0 as i32
    };
    if line_len <= 0 {
        if !fallback.is_empty() {
            crate::accessibility::screen_reader_speak(fallback);
        }
        return;
    }
    let mut buf = vec![0u16; (line_len + 1) as usize];
    let mut range = TEXTRANGEW {
        chrg: CHARRANGE {
            cpMin: line_start,
            cpMax: line_start + line_len,
        },
        lpstrText: PWSTR(buf.as_mut_ptr()),
    };
    unsafe {
        SendMessageW(
            hwnd_edit,
            EM_GETTEXTRANGE,
            WPARAM(0),
            LPARAM(&mut range as *mut _ as isize),
        );
    }
    let text = String::from_utf16_lossy(&buf[..line_len as usize]);
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        crate::accessibility::screen_reader_speak(trimmed);
    } else if !fallback.is_empty() {
        crate::accessibility::screen_reader_speak(fallback);
    }
}

fn clear_current_bookmarks(hwnd: HWND) -> bool {
    let (path, hwnd_edit, format, title) = {
        with_state(hwnd, |state| {
            state.docs.get(state.current).map(|doc| {
                (
                    doc.path.clone(),
                    doc.hwnd_edit,
                    doc.format,
                    doc.title.clone(),
                )
            })
        })
    }
    .flatten()
    .unwrap_or((None, HWND(0), FileFormat::default(), String::new()));
    if hwnd_edit.0 == 0 {
        return false;
    }
    let (storage_key, persist_to_disk) =
        runtime_bookmark_storage_key(path.as_deref(), hwnd_edit, &title, format);

    let (removed, bookmarks_window) = {
        with_state(hwnd, |state| {
            let removed = state.bookmarks.files.remove(&storage_key).is_some();
            if removed && persist_to_disk {
                save_bookmarks(&state.bookmarks);
            }
            (removed, state.bookmarks_window)
        })
    }
    .unwrap_or((false, HWND(0)));

    if bookmarks_window.0 != 0 {
        app_windows::bookmarks_window::refresh_bookmarks_list(bookmarks_window);
    }
    removed
}

pub(crate) fn goto_first_bookmark(
    hwnd_edit: HWND,
    path: &Path,
    bookmarks: &BookmarkStore,
    format: FileFormat,
    automatic_bookmark_enabled: bool,
) {
    let path_str = path.to_string_lossy().to_string();
    if let Some(list) = bookmarks.files.get(&path_str)
        && let Some(bm) = list
            .iter()
            .find(|bookmark| bookmark.is_visible(automatic_bookmark_enabled))
    {
        if matches!(format, FileFormat::Audiobook) {
            // Audiobook position is handled by playback start
        } else {
            let mut cr = CHARRANGE {
                cpMin: bm.position,
                cpMax: bm.position,
            };
            unsafe {
                SendMessageW(
                    hwnd_edit,
                    EM_EXSETSEL,
                    WPARAM(0),
                    LPARAM(&mut cr as *mut _ as isize),
                );
                SendMessageW(hwnd_edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
            }
        }
    }
}

pub(crate) fn rebuild_menus(hwnd: HWND) {
    let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
    let had_playback_menu = with_state(hwnd, |state| state.playback_menu.0 != 0).unwrap_or(false);
    let (_, recent_menu) = create_menus(hwnd, language);
    {
        with_state(hwnd, |state| {
            state.hmenu_recent = recent_menu;
        });
    }
    update_recent_menu(hwnd, recent_menu);
    if had_playback_menu {
        crate::menu::update_playback_menu(hwnd, true);
    }
    update_voice_panel_menu_check(hwnd);
}

pub(crate) fn push_recent_file(hwnd: HWND, path: &Path) {
    let (hmenu_recent, files) = match with_state(hwnd, |state| {
        state.recent_files.retain(|p| p != path);
        state.recent_files.insert(0, path.to_path_buf());
        if state.recent_files.len() > MAX_RECENT {
            state.recent_files.truncate(MAX_RECENT);
        }
        (state.hmenu_recent, state.recent_files.clone())
    }) {
        Some(values) => values,
        None => return,
    };
    update_recent_menu(hwnd, hmenu_recent);
    save_recent_files(&files);
}

pub(crate) fn clear_recent_files(hwnd: HWND) {
    let hmenu_recent = {
        with_state(hwnd, |state| {
            state.recent_files.clear();
            state.hmenu_recent
        })
    }
    .unwrap_or(HMENU(0));
    if hmenu_recent.0 != 0 {
        update_recent_menu(hwnd, hmenu_recent);
    }
    save_recent_files(&[]);
}

fn spawn_new_window_with_paths(paths: &[PathBuf]) -> bool {
    if paths.is_empty() {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let mut command = std::process::Command::new(exe);
    command.arg("--new-window");
    command.args(paths);
    command.spawn().is_ok()
}

fn spawn_new_window_with_path(path: &Path) -> bool {
    spawn_new_window_with_paths(&[path.to_path_buf()])
}

#[cfg(test)]
mod tests {
    use super::{
        COPYDATA_RESULT_DEFERRED_MODAL, COPYDATA_RESULT_HANDLED, SentenceNavigationDirection,
        append_debug_log, apply_selected_save_filter_extension,
        audio_bookmark_position_and_snippet, audiodescription_mpv_audio_track_id,
        clamp_tts_chunk_offset, editor_translate_language_for_menu_id,
        editor_translate_language_index_by_key, is_bad_executable_format_error,
        normalize_soft_line_breaks_for_translation, preferred_mpv_audio_track_id,
        preferred_tv_video_track_id, relative_audiobook_bookmark, sentence_navigation_target,
        sentence_start_offsets_utf16, should_defer_external_file_open,
        should_focus_existing_window_after_copydata,
    };
    use crate::bookmarks::Bookmark;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn save_dialog_replaces_stale_epub_extension_with_selected_txt_filter() {
        let mut path = PathBuf::from("book.epub");
        apply_selected_save_filter_extension(&mut path, "*.txt");
        assert_eq!(path, PathBuf::from("book.txt"));

        let mut text_path = PathBuf::from("document.txt");
        apply_selected_save_filter_extension(&mut text_path, "*.pdf");
        assert_eq!(text_path, PathBuf::from("document.pdf"));
    }

    #[test]
    fn save_dialog_preserves_matching_or_all_files_extensions() {
        let mut epub_path = PathBuf::from("book.epub");
        apply_selected_save_filter_extension(&mut epub_path, "*.epub");
        assert_eq!(epub_path, PathBuf::from("book.epub"));

        let mut custom_path = PathBuf::from("book.custom");
        apply_selected_save_filter_extension(&mut custom_path, "*.*");
        assert_eq!(custom_path, PathBuf::from("book.custom"));
    }

    #[test]
    fn mpv_runtime_repair_only_matches_bad_executable_format() {
        assert!(is_bad_executable_format_error(
            &std::io::Error::from_raw_os_error(193)
        ));
        assert!(!is_bad_executable_format_error(
            &std::io::Error::from_raw_os_error(2)
        ));
    }

    #[test]
    fn editor_translation_includes_german_and_brazilian_portuguese() {
        for (key, target) in [("de", "de"), ("pt_br", "pt-BR")] {
            let index = editor_translate_language_index_by_key(key)
                .expect("translation language should be available");
            let language = editor_translate_language_for_menu_id(
                crate::menu::IDM_EDIT_TRANSLATE_TARGET_BASE + index,
            )
            .expect("translation menu id should resolve");
            assert_eq!(language.key, key);
            assert_eq!(language.target, target);
        }
    }

    #[test]
    fn tv_video_track_matches_selected_audio_hls_program() {
        let tracks = serde_json::json!([
            {"type": "video", "id": 1, "program-id": 0, "hls-bitrate": 5689632},
            {"type": "video", "id": 3, "program-id": 2, "hls-bitrate": 2793078},
            {"type": "audio", "id": 4, "program-id": 0, "selected": false},
            {"type": "audio", "id": 6, "program-id": 2, "selected": true}
        ]);

        assert_eq!(
            preferred_tv_video_track_id(tracks.as_array().unwrap(), None),
            Some(3)
        );
    }

    #[test]
    fn tv_rai_prefers_audiodescription_and_its_video_variant() {
        let tracks = serde_json::json!([
            {"type": "video", "id": 1, "program-id": 0, "selected": true},
            {"type": "video", "id": 3, "program-id": 2, "selected": false},
            {"type": "audio", "id": 4, "program-id": 0, "lang": "ita", "selected": true},
            {"type": "audio", "id": 6, "program-id": 2, "lang": "des", "title": "Audiodescrizione", "selected": false}
        ]);

        let audio_id = preferred_mpv_audio_track_id(&tracks);
        assert_eq!(audio_id, Some(6));
        assert_eq!(
            preferred_tv_video_track_id(tracks.as_array().unwrap(), audio_id),
            Some(3)
        );
    }

    #[test]
    fn tv_audiodescription_detection_supports_generic_mpv_metadata() {
        let tracks = serde_json::json!([
            {"type": "audio", "id": 1, "lang": "ita"},
            {"type": "audio", "id": 2, "metadata": {"comment": "Audiodescrizione"}},
            {"type": "audio", "id": 3, "visual-impaired": true}
        ]);

        assert_eq!(audiodescription_mpv_audio_track_id(&tracks), Some(2));
    }

    #[test]
    fn tv_video_track_keeps_first_video_as_safe_fallback() {
        let tracks = serde_json::json!([
            {"type": "audio", "id": 1, "selected": true},
            {"type": "video", "id": 7},
            {"type": "video", "id": 8}
        ]);

        assert_eq!(
            preferred_tv_video_track_id(tracks.as_array().unwrap(), None),
            Some(7)
        );
    }

    #[test]
    fn concurrent_debug_log_writes_keep_lines_intact() {
        const THREADS: usize = 8;
        const LINES_PER_THREAD: usize = 50;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = Arc::new(std::env::temp_dir().join(format!(
            "sonarpad-concurrent-log-{}-{nonce}.log",
            std::process::id()
        )));
        let barrier = Arc::new(Barrier::new(THREADS));

        std::thread::scope(|scope| {
            for thread_index in 0..THREADS {
                let path = Arc::clone(&path);
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    barrier.wait();
                    for line_index in 0..LINES_PER_THREAD {
                        append_debug_log(
                            &path,
                            &format!("concurrent-log-{thread_index}-{line_index}"),
                        );
                    }
                });
            }
        });

        let content = std::fs::read_to_string(path.as_ref()).unwrap_or_default();
        let lines = content.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), THREADS * LINES_PER_THREAD);
        let messages = lines
            .iter()
            .filter_map(|line| line.split_once("] ").map(|(_, message)| message))
            .collect::<HashSet<_>>();
        assert_eq!(messages.len(), THREADS * LINES_PER_THREAD);
        for thread_index in 0..THREADS {
            for line_index in 0..LINES_PER_THREAD {
                let expected = format!("concurrent-log-{thread_index}-{line_index}");
                assert!(messages.contains(expected.as_str()));
            }
        }
        assert!(std::fs::remove_file(path.as_ref()).is_ok());
    }

    #[test]
    fn external_file_open_is_deferred_when_main_window_is_disabled() {
        assert!(should_defer_external_file_open(false));
    }

    #[test]
    fn external_file_open_is_immediate_when_main_window_is_enabled() {
        assert!(!should_defer_external_file_open(true));
    }

    #[test]
    fn sender_does_not_focus_disabled_main_window_when_open_is_deferred() {
        assert!(!should_focus_existing_window_after_copydata(
            COPYDATA_RESULT_DEFERRED_MODAL,
        ));
    }

    #[test]
    fn sender_focuses_existing_window_after_immediate_open() {
        assert!(should_focus_existing_window_after_copydata(
            COPYDATA_RESULT_HANDLED,
        ));
    }

    #[test]
    fn audio_bookmark_position_rounds_down_and_formats() {
        let (pos, snippet) = audio_bookmark_position_and_snippet(73.9);
        assert_eq!(pos, 73);
        assert_eq!(snippet, "Posizione audio: 01:13");
    }

    #[test]
    fn audio_bookmark_position_clamps_negative() {
        let (pos, snippet) = audio_bookmark_position_and_snippet(-2.0);
        assert_eq!(pos, 0);
        assert_eq!(snippet, "Posizione audio: 00:00");
    }

    #[test]
    fn audio_bookmark_position_formats_over_an_hour() {
        let (pos, snippet) = audio_bookmark_position_and_snippet(3723.0);
        assert_eq!(pos, 3723);
        assert_eq!(snippet, "Posizione audio: 62:03");
    }

    #[test]
    fn tts_chunk_offset_stays_monotonic() {
        assert_eq!(clamp_tts_chunk_offset(120, 140), 140);
        assert_eq!(clamp_tts_chunk_offset(120, 120), 120);
        assert_eq!(clamp_tts_chunk_offset(120, 90), 120);
    }

    #[test]
    fn tts_chunk_offset_clamps_negative_input() {
        assert_eq!(clamp_tts_chunk_offset(0, -7), 0);
        assert_eq!(clamp_tts_chunk_offset(25, -1), 25);
    }

    #[test]
    fn sentence_navigation_does_not_split_on_thousands_separators() {
        let text = "Sono chiamati alle urne 51.424.729 cittadini, tra cui 5.477.619 residenti all’estero. Si vota.";
        assert_eq!(sentence_start_offsets_utf16(text), vec![0, 86]);
    }

    #[test]
    fn previous_sentence_moves_to_real_previous_sentence() {
        let text = "Prima frase. Seconda frase. Terza frase.";
        let starts = sentence_start_offsets_utf16(text);
        assert_eq!(starts, vec![0, 13, 28]);
        assert_eq!(
            sentence_navigation_target(text, 30, SentenceNavigationDirection::Previous),
            Some(13)
        );
        assert_eq!(
            sentence_navigation_target(text, 15, SentenceNavigationDirection::Previous),
            Some(0)
        );
    }

    #[test]
    fn audiobook_previous_skips_current_bookmark_after_seek() {
        let bookmarks = vec![
            Bookmark::manual(2, String::new(), String::new()),
            Bookmark::manual(4, String::new(), String::new()),
            Bookmark::manual(5, String::new(), String::new()),
        ];
        let target = relative_audiobook_bookmark(&bookmarks, None, 5.1, false)
            .map(|bookmark| bookmark.position);
        assert_eq!(target, Some(4));
    }

    #[test]
    fn audiobook_next_advances_from_current_bookmark() {
        let bookmarks = vec![
            Bookmark::manual(2, String::new(), String::new()),
            Bookmark::manual(4, String::new(), String::new()),
            Bookmark::manual(5, String::new(), String::new()),
        ];
        let target = relative_audiobook_bookmark(&bookmarks, None, 4.1, true)
            .map(|bookmark| bookmark.position);
        assert_eq!(target, Some(5));
    }

    #[test]
    fn audiobook_previous_uses_anchor_not_playback_drift() {
        let bookmarks = vec![
            Bookmark::manual(2, String::new(), String::new()),
            Bookmark::manual(4, String::new(), String::new()),
            Bookmark::manual(5, String::new(), String::new()),
        ];
        let target = relative_audiobook_bookmark(&bookmarks, Some(4), 4.9, false)
            .map(|bookmark| bookmark.position);
        assert_eq!(target, Some(2));
    }

    #[test]
    fn translation_normalization_joins_wrapped_book_lines() {
        let text = "what it takes the three of us two days to \ndo.\" My college roommate";

        assert_eq!(
            normalize_soft_line_breaks_for_translation(text),
            "what it takes the three of us two days to do.\" My college roommate"
        );
    }

    #[test]
    fn translation_normalization_joins_rich_edit_cr_line_breaks() {
        let text =
            "My college roommate was working at a retail electronics store in \rthe early 2000s";

        assert_eq!(
            normalize_soft_line_breaks_for_translation(text),
            "My college roommate was working at a retail electronics store in the early 2000s"
        );
    }

    #[test]
    fn translation_normalization_preserves_sentence_breaks() {
        let text = "First complete sentence.\nSecond complete sentence.";

        assert_eq!(normalize_soft_line_breaks_for_translation(text), text);
    }

    #[test]
    fn translation_normalization_preserves_short_heading_breaks() {
        let text = "Chapter One\nA Beginning";

        assert_eq!(normalize_soft_line_breaks_for_translation(text), text);
    }
}

fn open_document_with_encoding(hwnd: HWND, path: &Path, encoding: Option<TextEncoding>) {
    let behavior =
        { with_state(hwnd, |state| state.settings.open_behavior) }.unwrap_or(OpenBehavior::NewTab);
    if behavior == OpenBehavior::NewWindow && spawn_new_window_with_path(path) {
        return;
    }
    editor_manager::open_document_with_encoding(hwnd, path, encoding);
}

fn play_audio_playlist_item(hwnd: HWND, index: usize) {
    let path = {
        with_state(hwnd, |state| {
            if index >= state.audio_playlist.len() {
                return None;
            }
            state.audio_playlist_index = Some(index);
            state.audio_ffmpeg_retry_for = None;
            Some(state.audio_playlist[index].clone())
        })
        .flatten()
    };
    let Some(path) = path else {
        return;
    };

    let tab_index = editor_manager::ensure_audio_document_tab(hwnd, &path);
    if let Some(tab_index) = tab_index {
        editor_manager::select_tab(hwnd, tab_index);
    }
    if is_video_path(&path) {
        if let Err(err) = launch_local_video_in_mpv(hwnd, &path) {
            log_debug(&format!(
                "Audio player: failed to launch local video in mpv: {}",
                err
            ));
            screen_reader_speak(&err);
        }
        return;
    }
    let mpv_was_active = is_mpv_playback_active(hwnd);
    if mpv_was_active {
        log_debug(&format!(
            "Audio player: mpv->audio handoff start target={} foreground_before_stop={:?} focus_before_stop={:?}",
            path.display(),
            unsafe { GetForegroundWindow() },
            unsafe { GetFocus() }
        ));
    }
    audio_player::stop_audiobook_playback(hwnd);
    if mpv_was_active {
        log_debug(&format!(
            "Audio player: mpv->audio handoff after stop foreground={:?} focus={:?}",
            unsafe { GetForegroundWindow() },
            unsafe { GetFocus() }
        ));
        bring_window_to_foreground(hwnd);
        if let Some(hwnd_tab) = with_state(hwnd, |state| state.hwnd_tab) {
            set_focus_safe(hwnd_tab);
        }
        schedule_mpv_bass_focus_debug_snapshots(hwnd);
        log_debug(&format!(
            "Audio player: mpv->audio handoff after player focus foreground={:?} focus={:?}",
            unsafe { GetForegroundWindow() },
            unsafe { GetFocus() }
        ));
    }
    audio_player::start_audiobook_playback(hwnd, &path);
}

pub(crate) fn queue_audio_files_and_play(hwnd: HWND, paths: Vec<PathBuf>) {
    {
        if paths.is_empty() {
            return;
        }
        for path in &paths {
            if editor_manager::ensure_audio_document_tab(hwnd, path).is_none() {
                crate::log_debug(&format!(
                    "Audio player: failed to ensure audio tab for {}",
                    path.display()
                ));
            }
            push_recent_file(hwnd, path);
        }
        let target_index = with_state(hwnd, |state| {
            let play_path = paths[0].clone();
            if state.audio_playlist.is_empty() {
                for path in &paths {
                    if !state.audio_playlist.iter().any(|p| p == path) {
                        state.audio_playlist.push(path.clone());
                    }
                }
                let idx = state
                    .audio_playlist
                    .iter()
                    .position(|p| p == &play_path)
                    .unwrap_or(0);
                state.audio_playlist_index = Some(idx);
                return idx;
            }
            let current = state
                .audio_playlist_index
                .filter(|idx| *idx < state.audio_playlist.len())
                .unwrap_or(0);
            let mut insert_at = current.saturating_add(1);
            for path in &paths {
                if state.audio_playlist.iter().any(|p| p == path) {
                    continue;
                }
                state.audio_playlist.insert(insert_at, path.clone());
                insert_at = insert_at.saturating_add(1);
            }
            let target = state
                .audio_playlist
                .iter()
                .position(|p| p == &play_path)
                .unwrap_or(current);
            state.audio_playlist_index = Some(target);
            target
        })
        .unwrap_or(0);

        play_audio_playlist_item(hwnd, target_index);
        show_window_safe(hwnd, SW_SHOWMAXIMIZED);
        restore_editor_focus(hwnd);
    }
}

fn switch_audio_playlist_relative(hwnd: HWND, delta: i32) -> bool {
    let target = {
        with_state(hwnd, |state| {
            if state.audio_playlist.is_empty() {
                return None;
            }
            let current = state.audio_playlist_index?;
            let next = if delta > 0 {
                current.checked_add(delta as usize)?
            } else {
                current.checked_sub(delta.unsigned_abs() as usize)?
            };
            if next >= state.audio_playlist.len() {
                return None;
            }
            Some(next)
        })
        .flatten()
    };
    let Some(target) = target else {
        return false;
    };
    play_audio_playlist_item(hwnd, target);
    true
}

fn handle_audio_playlist_timer(hwnd: HWND) {
    if daisy_player::handle_playback_timer(hwnd) {
        return;
    }
    let (is_paused, should_advance, current_seconds, elapsed_since_start, total_seconds) = {
        with_state(hwnd, |state| {
            let player = state.active_audiobook.as_ref()?;
            if player.is_paused {
                return Some((true, false, 0_u64, std::time::Duration::from_secs(0), None));
            }
            let current = audio_player::audiobook_position_secs(player)
                .max(0.0)
                .floor() as u64;
            let total_player = player.duration_secs().map(|d| d.max(0.0).floor() as u64);
            let total_file = audio_player::audiobook_duration_secs(&player.path);
            let near_end_player = total_player
                .map(|duration| current.saturating_add(1) >= duration)
                .unwrap_or(false);
            let near_end_file = total_file
                .map(|duration| current.saturating_add(1) >= duration)
                .unwrap_or(false);
            // Treat as ended if either live player duration or file duration confirms near-end.
            let should_advance = near_end_player || near_end_file;
            let total_seconds = total_file.or(total_player);
            Some((
                false,
                should_advance,
                current,
                player.start_instant.elapsed(),
                total_seconds,
            ))
        })
        .flatten()
        .unwrap_or((false, false, 0_u64, std::time::Duration::from_secs(0), None))
    };
    if is_paused {
        return;
    }
    let output_stopped = audio_player::audiobook_output_stopped(hwnd).unwrap_or(false);
    if !should_advance && !output_stopped {
        return;
    }
    // Retry with forced FFmpeg streaming only in the startup window.
    // This avoids collisions with manual seeks/skips during normal playback.
    if !should_advance
        && output_stopped
        && current_seconds == 0
        && elapsed_since_start.as_secs() <= 5
        && audio_player::retry_current_with_ffmpeg_stream(hwnd)
    {
        return;
    }
    // In the first seconds after startup/resume FFmpeg streaming may transiently report
    // "stopped" before stable output; avoid stopping the playlist too early.
    if !should_advance && output_stopped && elapsed_since_start.as_secs() <= 2 {
        return;
    }
    // If output stops mid-file (e.g. transient backend failure), recover by restarting
    // from the current position instead of stopping playback entirely.
    if !should_advance && output_stopped {
        let is_stream_cache = {
            with_state(hwnd, |state| {
                state
                    .active_audiobook
                    .as_ref()
                    .map(|player| is_stream_cache_media(&player.path))
                    .unwrap_or(false)
            })
            .unwrap_or(false)
        };
        if is_stream_cache {
            let stopped_far_from_end = total_seconds
                .map(|total| current_seconds.saturating_add(5) < total)
                .unwrap_or(current_seconds >= 15);
            if elapsed_since_start.as_secs() >= 5
                && current_seconds > 0
                && stopped_far_from_end
                && audio_player::retry_current_after_unexpected_stop(hwnd)
            {
                return;
            }
            log_debug("Audio player: stream cache output stopped, skipping mid-file auto-restart");
        } else {
            let restart = {
                with_state(hwnd, |state| {
                    let player = state.active_audiobook.as_ref()?;
                    let position_secs = audio_player::audiobook_position_secs(player)
                        .max(0.0)
                        .floor() as u64;
                    Some((player.path.clone(), position_secs))
                })
                .flatten()
            };
            if let Some((path, position_secs)) = restart {
                audio_player::start_audiobook_at(hwnd, &path, position_secs);
                return;
            }
        }
    }
    if !switch_audio_playlist_relative(hwnd, 1) {
        // Keep the player instance alive when no next track is available.
        // This matches non-hard-stop behavior (seek back and resume works).
    }
}

fn open_path_with_behavior(hwnd: HWND, path: &Path) {
    if is_audio_path(path) {
        queue_audio_files_and_play(hwnd, vec![path.to_path_buf()]);
        return;
    }
    open_document_with_encoding(hwnd, path, None);
}

pub(crate) fn with_state<F, R>(hwnd: HWND, f: F) -> Option<R>
where
    F: FnOnce(&mut AppState) -> R,
{
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut AppState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

pub(crate) fn open_pdf_document_async(hwnd: HWND, path: &Path, from_copydata: bool) {
    unsafe {
        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
        let path_buf = path.to_path_buf();
        let title = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("File")
            .to_string();
        let (hwnd_edit, new_index) = with_state(hwnd, |state| {
            let hwnd_edit = create_edit(
                hwnd,
                state.hfont,
                state.settings.word_wrap,
                state.settings.text_color,
                state.settings.text_size,
            );
            editor_manager::set_edit_text(hwnd_edit, &pdf_loading_placeholder(0, language));
            let doc = Document {
                title: title.clone(),
                path: Some(path_buf.clone()),
                hwnd_edit,
                dirty: false,
                format: FileFormat::Pdf,
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
                pdf_is_imported_source: true,
            };
            state.docs.push(doc);
            insert_tab(state.hwnd_tab, &title, (state.docs.len() - 1) as i32);
            (hwnd_edit, state.docs.len() - 1)
        })
        .unwrap_or((HWND(0), 0));

        if hwnd_edit.0 == 0 {
            return;
        }
        select_tab(hwnd, new_index);

        let ocr_timeout_secs = if from_copydata {
            PDF_OCR_PROMPT_TIMEOUT_COPYDATA_SECS
        } else {
            0
        };
        start_pdf_loading_animation(hwnd, hwnd_edit, ocr_timeout_secs);

        let hwnd_main = hwnd;
        std::thread::spawn(move || {
            let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                read_pdf_text_with_status(&path_buf, language)
            })) {
                Ok(result) => result,
                Err(panic) => {
                    let panic_msg = if let Some(s) = panic.downcast_ref::<&str>() {
                        (*s).to_string()
                    } else if let Some(s) = panic.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "unknown panic payload".to_string()
                    };
                    crate::log_debug(&format!("PDF load thread panic caught: {}", panic_msg));
                    Err(crate::i18n::tr_f(
                        language,
                        "file_handler.pdf_read_error",
                        &[("err", "PDF extraction crashed unexpectedly")],
                    ))
                }
            };
            let payload = Box::new(PdfLoadResult {
                hwnd_edit,
                path: path_buf,
                result,
                from_copydata,
            });
            let payload_ptr = Box::into_raw(payload);
            if let Err(e) = PostMessageW(
                hwnd_main,
                WM_PDF_LOADED,
                WPARAM(0),
                LPARAM(payload_ptr as isize),
            ) {
                crate::log_debug(&format!("Failed to post WM_PDF_LOADED: {}", e));
                let _unused_box = Box::from_raw(payload_ptr);
            }
        });
    }
}

fn start_ocr_for_pdf(
    hwnd: HWND,
    hwnd_edit: HWND,
    path: PathBuf,
    from_copydata: bool,
    language: Language,
) {
    start_pdf_loading_animation(hwnd, hwnd_edit, 0);
    let hwnd_main = hwnd;
    std::thread::spawn(move || {
        let final_result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            win_ocr::recognize_text_from_pdf(&path, language)
        })) {
            Ok(ocr_result) => match ocr_result {
                Ok(text) => Ok(PdfTextResult::Text(text)),
                Err(e) => Err(e),
            },
            Err(panic) => {
                let panic_msg = if let Some(s) = panic.downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = panic.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic payload".to_string()
                };
                crate::log_debug(&format!("PDF OCR thread panic caught: {}", panic_msg));
                Err(crate::i18n::tr_f(
                    language,
                    "file_handler.pdf_read_error",
                    &[("err", "PDF OCR crashed unexpectedly")],
                ))
            }
        };
        let payload = Box::new(PdfLoadResult {
            hwnd_edit,
            path,
            result: final_result,
            from_copydata,
        });
        unsafe {
            let payload_ptr = Box::into_raw(payload);
            if let Err(e) = PostMessageW(
                hwnd_main,
                WM_PDF_LOADED,
                WPARAM(0),
                LPARAM(payload_ptr as isize),
            ) {
                crate::log_debug(&format!("Failed to post WM_PDF_LOADED (OCR): {}", e));
                let _unused_box = Box::from_raw(payload_ptr);
            }
        }
    });
}

fn handle_pdf_loaded(hwnd: HWND, payload: PdfLoadResult) {
    unsafe {
        let PdfLoadResult {
            hwnd_edit,
            path,
            result,
            from_copydata,
        } = payload;
        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();

        stop_pdf_loading_animation(hwnd, hwnd_edit);

        let doc_index = with_state(hwnd, |state| {
            state
                .docs
                .iter()
                .enumerate()
                .find_map(|(i, doc)| (doc.hwnd_edit == hwnd_edit).then_some(i))
        })
        .flatten();

        let Some(index) = doc_index else {
            return;
        };

        match result {
            Ok(PdfTextResult::Text(text)) => {
                editor_manager::set_edit_text(hwnd_edit, &text);
                with_state(hwnd, |state| {
                    goto_first_bookmark(
                        hwnd_edit,
                        &path,
                        &state.bookmarks,
                        FileFormat::Pdf,
                        state.settings.automatic_bookmark,
                    );
                });
                let msg = pdf_loaded_message(language);
                crate::log_debug(&format!("Info (speech): {msg}"));
                crate::accessibility::nvda_speak(&msg);
                crate::log_if_err!(MessageBeep(MB_ICONASTERISK));
                let mut update_title = false;
                with_state(hwnd, |state| {
                    if let Some(doc) = state.docs.get_mut(index) {
                        doc.dirty = false;
                        update_tab_title(state.hwnd_tab, index, &doc.title, false);
                        update_title = state.current == index;
                    }
                });
                if update_title {
                    update_window_title(hwnd);
                }
                push_recent_file(hwnd, &path);
            }
            Ok(PdfTextResult::NoText) => {
                start_ocr_for_pdf(hwnd, hwnd_edit, path, from_copydata, language);
            }
            Err(message) => {
                // Instead of closing the document, show error message as placeholder text
                let error_placeholder = format!(
                    "{}\n\n{}",
                    message,
                    i18n::tr(language, "app.pdf_error_hint")
                );
                editor_manager::set_edit_text(hwnd_edit, &error_placeholder);
                show_error(hwnd, language, &message);
                let mut update_title = false;
                with_state(hwnd, |state| {
                    if let Some(doc) = state.docs.get_mut(index) {
                        doc.dirty = false;
                        update_tab_title(state.hwnd_tab, index, &doc.title, false);
                        update_title = state.current == index;
                    }
                });
                if update_title {
                    update_window_title(hwnd);
                }
            }
        }
    }
}

fn handle_document_loaded(hwnd: HWND, payload: editor_manager::DocumentLoadResult) {
    {
        let editor_manager::DocumentLoadResult { path, result } = payload;
        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
        const LARGE_FILE_NO_WRAP_THRESHOLD_BYTES: u64 = 15 * 1024 * 1024;

        let loaded = match result {
            Ok(loaded) => loaded,
            Err(message) => {
                show_error(hwnd, language, &message);
                return;
            }
        };
        let Some(loaded) = loaded else {
            return;
        };
        let large_file_no_wrap = std::fs::metadata(&path)
            .map(|meta| meta.len() >= LARGE_FILE_NO_WRAP_THRESHOLD_BYTES)
            .unwrap_or(false);

        let title = path.file_name().and_then(|s| s.to_str()).unwrap_or("File");
        let is_daisy = matches!(loaded.format, FileFormat::Daisy);
        let has_epub_index = !loaded.epub_index.is_empty() && !is_daisy;
        let (hwnd_edit, new_index) = with_state(hwnd, |state| {
            let use_word_wrap = state.settings.word_wrap && !large_file_no_wrap;
            let hwnd_edit = editor_manager::create_edit(
                hwnd,
                state.hfont,
                use_word_wrap,
                state.settings.text_color,
                state.settings.text_size,
            );
            editor_manager::set_edit_text(hwnd_edit, &loaded.content);

            let doc = Document {
                title: title.to_string(),
                path: Some(path.clone()),
                hwnd_edit,
                dirty: false,
                format: loaded.format,
                opened_text_encoding: loaded.opened_text_encoding,
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
                epub_index: loaded.epub_index,
                epub_original_text: loaded.epub_original_text,
                pdf_is_imported_source: false,
            };
            state.docs.push(doc);
            if large_file_no_wrap {
                state.large_text_editors.insert(hwnd_edit.0);
            } else {
                state.large_text_editors.remove(&hwnd_edit.0);
            }
            insert_tab(state.hwnd_tab, title, (state.docs.len() - 1) as i32);
            goto_first_bookmark(
                hwnd_edit,
                &path,
                &state.bookmarks,
                loaded.format,
                state.settings.automatic_bookmark,
            );
            (hwnd_edit, state.docs.len() - 1)
        })
        .unwrap_or((HWND(0), 0));

        if hwnd_edit.0 == 0 {
            return;
        }
        if large_file_no_wrap {
            log_debug(&format!(
                "Large file mode: disabled word wrap for '{}' (>= {} bytes)",
                path.display(),
                LARGE_FILE_NO_WRAP_THRESHOLD_BYTES
            ));
        }

        editor_manager::select_tab(hwnd, new_index);
        if has_epub_index {
            unsafe {
                kill_timer_best_effort(
                    hwnd,
                    EPUB_INDEX_ANNOUNCE_TIMER_ID,
                    "KillTimer previous EPUB_INDEX_ANNOUNCE",
                );
                if SetTimer(hwnd, EPUB_INDEX_ANNOUNCE_TIMER_ID, 1000, None) == 0 {
                    log_debug("Failed to set EPUB index announcement timer");
                }
            }
        }
        push_recent_file(hwnd, &path);

        let pending = with_state(hwnd, |state| state.pending_find_in_files.clone()).unwrap_or(None);
        if let Some(pending) = pending
            && pending.path == path
        {
            apply_find_in_files_selection(
                hwnd_edit,
                &pending.snippet,
                &pending.term,
                pending.start_utf16,
                pending.len_utf16,
            );
            with_state(hwnd, |state| {
                if let Some(doc) = state.docs.get_mut(state.current) {
                    doc.from_find_in_files = true;
                }
                state.pending_find_in_files = None;
            });
        }
        if is_daisy {
            daisy_player::open_index_for_document(hwnd, &path);
        }
    }
}

fn start_pdf_loading_animation(hwnd: HWND, hwnd_edit: HWND, ocr_timeout_secs: u64) {
    unsafe {
        let timer_id = with_state(hwnd, |state| {
            let timer_id = state.next_timer_id;
            state.next_timer_id = state.next_timer_id.saturating_add(1);
            state.pdf_loading.push(PdfLoadingState {
                hwnd_edit,
                timer_id,
                frame: 0,
                start_time: Instant::now(),
                ocr_timeout_secs,
            });
            timer_id
        })
        .unwrap_or(0);

        if timer_id == 0 {
            return;
        }

        if SetTimer(hwnd, timer_id, 120, None) == 0 {
            stop_pdf_loading_animation(hwnd, hwnd_edit);
        }
    }
}

fn stop_pdf_loading_animation(hwnd: HWND, hwnd_edit: HWND) {
    {
        let mut timer_id = None;
        with_state(hwnd, |state| {
            if let Some(pos) = state
                .pdf_loading
                .iter()
                .position(|entry| entry.hwnd_edit == hwnd_edit)
            {
                timer_id = Some(state.pdf_loading[pos].timer_id);
                state.pdf_loading.swap_remove(pos);
            }
        });
        if let Some(timer_id) = timer_id {
            kill_timer_best_effort(hwnd, timer_id, "KillTimer PDF_LOADING");
        }
    }
}

fn handle_pdf_loading_timer(hwnd: HWND, timer_id: usize) {
    {
        let mut target = None;
        let mut should_timeout = false;
        with_state(hwnd, |state| {
            if let Some(entry) = state
                .pdf_loading
                .iter_mut()
                .find(|entry| entry.timer_id == timer_id)
            {
                entry.frame = entry.frame.wrapping_add(1);
                let timeout_secs = entry.ocr_timeout_secs;
                if timeout_secs == 0 || entry.start_time.elapsed().as_secs() >= timeout_secs {
                    should_timeout = true;
                }
                target = Some((entry.hwnd_edit, entry.frame, timeout_secs));
            }
        });

        if should_timeout && let Some((hwnd_edit, _, timeout_secs)) = target {
            stop_pdf_loading_animation(hwnd, hwnd_edit);
            let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
            let from_copydata = timeout_secs != 0;
            let path = with_state(hwnd, |state| {
                state
                    .docs
                    .iter()
                    .find(|d| d.hwnd_edit == hwnd_edit)
                    .and_then(|d| d.path.clone())
            })
            .flatten();

            if let Some(path) = path {
                start_ocr_for_pdf(hwnd, hwnd_edit, path, from_copydata, language);
            } else {
                let err = i18n::tr(language, "app.pdf_error_hint");
                editor_manager::set_edit_text(hwnd_edit, &err);
            }
            return;
        }

        if let Some((hwnd_edit, frame, _)) = target {
            let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
            editor_manager::set_edit_text(hwnd_edit, &pdf_loading_placeholder(frame, language));
        }
    }
}

pub(crate) fn pdf_loading_placeholder(frame: usize, language: crate::settings::Language) -> String {
    let spinner = ['|', '/', '-', '\\'][frame % 4];
    let bar_width = 24;
    let filled = frame % (bar_width + 1);
    let bar = format!(
        "{}{}",
        "#".repeat(filled),
        "-".repeat(bar_width.saturating_sub(filled))
    );
    let loading = i18n::tr(language, "app.pdf_loading");
    let analyzing = i18n::tr(language, "app.pdf_analyzing");
    format!("{loading}\r\n\r\n[{bar}]\r\n{analyzing} {spinner}")
}

fn handle_drop_files(hwnd: HWND, hdrop: HDROP) {
    let count = drag_query_file_w_safe(hdrop, 0xFFFFFFFF, None);
    let mut dropped_paths = Vec::new();
    for index in 0..count {
        let mut buffer = [0u16; 260];
        let len = drag_query_file_w_safe(hdrop, index, Some(&mut buffer));
        if len == 0 {
            continue;
        }
        let path = PathBuf::from(String::from_utf16_lossy(&buffer[..len as usize]));
        if path.as_os_str().is_empty() {
            continue;
        }
        dropped_paths.push(path);
    }
    if !dropped_paths.is_empty() && dropped_paths.iter().all(|path| is_audio_path(path)) {
        queue_audio_files_and_play(hwnd, dropped_paths);
    } else {
        for path in dropped_paths {
            open_path_with_behavior(hwnd, &path);
        }
    }
    unsafe {
        DragFinish(hdrop);
    }
}

fn next_tab_with_prompt(hwnd: HWND) {
    select_relative_tab_with_prompt(hwnd, 1);
}

fn select_relative_tab_with_prompt(hwnd: HWND, delta: isize) {
    let (current, count) = match with_state(hwnd, |state| {
        if state.docs.is_empty() {
            return None;
        }
        let current = state.current;
        Some((current, state.docs.len()))
    }) {
        Some(Some(values)) => values,
        _ => return,
    };
    if count <= 1 {
        return;
    }
    let next = if delta < 0 {
        if current == 0 { count - 1 } else { current - 1 }
    } else {
        (current + 1) % count
    };
    select_tab(hwnd, next);
}

fn open_documents_popup(hwnd: HWND) {
    let docs = {
        with_state(hwnd, |state| {
            state
                .docs
                .iter()
                .enumerate()
                .map(|(idx, doc)| (idx, doc.title.clone()))
                .collect::<Vec<_>>()
        })
    }
    .unwrap_or_default();
    if docs.len() <= 1 {
        return;
    }

    let menu = crate::create_popup_menu_safe();
    if menu.0 == 0 {
        return;
    }

    for (idx, title) in docs.iter().take(200) {
        let id = (WINDOW_DOC_MENU_BASE + *idx) as u32;
        let display = if title.trim().is_empty() {
            format!("Documento {}", idx + 1)
        } else {
            title.clone()
        };
        let label = format!("&{} {}", idx + 1, display);
        unsafe {
            crate::log_if_err!(AppendMenuW(
                menu,
                MF_STRING,
                id as usize,
                PCWSTR(to_wide(&label).as_ptr()),
            ));
        }
    }

    let mut pt = POINT::default();
    if crate::get_cursor_pos_safe(&mut pt).is_err() {
        return;
    }
    let command = crate::track_popup_menu_safe(
        menu,
        TPM_RIGHTBUTTON | TPM_RETURNCMD,
        pt.x,
        pt.y,
        0,
        hwnd,
        None,
    );
    if let Some(index) = window_doc_menu_index_from_command(command.0 as usize) {
        select_tab(hwnd, index);
    }
}

fn window_doc_menu_index_from_command(cmd_id: usize) -> Option<usize> {
    if (WINDOW_DOC_MENU_BASE..(WINDOW_DOC_MENU_BASE + WINDOW_DOC_MENU_MAX)).contains(&cmd_id) {
        Some(cmd_id - WINDOW_DOC_MENU_BASE)
    } else {
        None
    }
}

fn refresh_window_open_documents_menu(hwnd: HWND, window_menu: HMENU) {
    unsafe {
        crate::log_if_err!(DeleteMenu(
            window_menu,
            WINDOW_DOC_MENU_SEPARATOR_ID as u32,
            MF_BYCOMMAND
        ));
    }
    for idx in 0..WINDOW_DOC_MENU_MAX {
        let cmd_id = (WINDOW_DOC_MENU_BASE + idx) as u32;
        delete_menu_best_effort(
            window_menu,
            cmd_id,
            MF_BYCOMMAND,
            &format!("DeleteMenu(window open docs, cmd_id={cmd_id})"),
        );
    }

    let docs = {
        with_state(hwnd, |state| {
            state
                .docs
                .iter()
                .enumerate()
                .map(|(idx, doc)| (idx, doc.title.clone()))
                .collect::<Vec<_>>()
        })
    }
    .unwrap_or_default();
    if docs.len() <= 1 {
        return;
    }

    unsafe {
        crate::log_if_err!(AppendMenuW(
            window_menu,
            MF_SEPARATOR,
            WINDOW_DOC_MENU_SEPARATOR_ID,
            PCWSTR::null()
        ));
    }
    for (idx, title) in docs.iter().take(WINDOW_DOC_MENU_MAX) {
        let display = if title.trim().is_empty() {
            format!("Documento {}", idx + 1)
        } else {
            title.clone()
        };
        let label = format!("&{} {}", idx + 1, display);
        unsafe {
            crate::log_if_err!(AppendMenuW(
                window_menu,
                MF_STRING,
                WINDOW_DOC_MENU_BASE + idx,
                PCWSTR(to_wide(&label).as_ptr()),
            ));
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn suggest_extension_for_interpreter(interpreter: &str) -> &'static str {
    let lower = interpreter.to_lowercase();
    if lower.contains("python") || lower.ends_with("py.exe") || lower.ends_with("py") {
        "py"
    } else if lower.contains("java") {
        "java"
    } else if lower.contains("node") {
        "js"
    } else if lower.contains("ruby") {
        "rb"
    } else if lower.contains("perl") {
        "pl"
    } else if lower.contains("php") {
        "php"
    } else if lower.contains("lua") {
        "lua"
    } else if lower.contains("htm") || lower.contains("html") {
        "html"
    } else if lower.contains("powershell") || lower.contains("pwsh") {
        "ps1"
    } else if lower.contains("bash") || lower.contains("sh.exe") || lower == "sh" {
        "sh"
    } else {
        "txt"
    }
}

fn execute_current_file(hwnd: HWND) {
    let (path, content, interpreter) = match with_state(hwnd, |state| {
        let doc = state.docs.get(state.current)?;
        let path = doc.path.clone();
        let hwnd_edit = doc.hwnd_edit;
        let content = editor_manager::get_edit_text(hwnd_edit);
        let interpreter = state.settings.interpreter_path.clone();
        Some((path, content, interpreter))
    }) {
        Some(Some(v)) => v,
        _ => return,
    };

    let (exec_path, working_dir) = if let Some(p) = path {
        let dir = p.parent().map(|d| d.to_path_buf());
        (p, dir)
    } else {
        // Not saved: create temp file
        let ext = suggest_extension_for_interpreter(&interpreter);
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("sonarpad_exec_{}.{}", now_ms(), ext));
        if let Err(e) = std::fs::write(&temp_file, content) {
            log_debug(&format!("Failed to write temp file for execution: {}", e));
            return;
        }
        (temp_file, Some(temp_dir))
    };

    let extension = exec_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if extension == "html" || extension == "htm" {
        let res = unsafe {
            ShellExecuteW(
                hwnd,
                w!("open"),
                PCWSTR(to_wide(&exec_path.to_string_lossy()).as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if res.0 as isize <= 32 {
            crate::log_debug(&format!("ShellExecuteW failed to open HTML: {}", res.0));
        }
        return;
    }

    let command = format!("\"{}\" \"{}\"", interpreter, exec_path.to_string_lossy());
    app_windows::prompt_window::open_with_command(hwnd, Some(command), working_dir);
}

fn attempt_switch_to_selected_tab(hwnd: HWND) {
    let (current, hwnd_tab, count) = match with_state(hwnd, |state| {
        if state.docs.is_empty() {
            return None;
        }
        let current = state.current;
        Some((current, state.hwnd_tab, state.docs.len()))
    }) {
        Some(Some(values)) => values,
        _ => return,
    };
    let sel = send_message_w_safe(hwnd_tab, TCM_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
    if sel < 0 {
        return;
    }
    let sel = sel as usize;
    if sel >= count || sel == current {
        return;
    }
    select_tab(hwnd, sel);
}

fn suggested_filename_from_text(text: &str) -> Option<String> {
    let first_line = text.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return None;
    }
    let sanitized = sanitize_filename(first_line);
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

pub(crate) fn sanitize_filename(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_control() {
            continue;
        }
        match ch {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => out.push(' '),
            _ => out.push(ch),
        }
    }
    let mut cleaned = out.trim().trim_end_matches(['.', ' ']).to_string();
    if cleaned.is_empty() {
        return cleaned;
    }
    if cleaned.len() > 120 {
        let mut idx = 120;
        while idx > 0 && !cleaned.is_char_boundary(idx) {
            idx -= 1;
        }
        cleaned.truncate(idx);
    }
    if is_reserved_filename(&cleaned) {
        cleaned.push('_');
    }
    cleaned
}

fn is_reserved_filename(name: &str) -> bool {
    let upper = name.trim_end_matches(['.', ' ']).to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

pub(crate) struct SaveAudioDialogResult {
    pub path: PathBuf,
    pub create_parts_folder: bool,
}

pub(crate) fn save_audio_dialog(
    hwnd: HWND,
    suggested_name: Option<&str>,
    show_split_folder_option: bool,
) -> Option<SaveAudioDialogResult> {
    unsafe {
        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();
        let initial_dir_setting =
            with_state(hwnd, |state| state.settings.audiobook_save_folder.clone())
                .unwrap_or_default();
        let initial_dir = initial_dir_setting.trim().to_string();
        if !initial_dir.is_empty() {
            crate::log_if_err!(std::fs::create_dir_all(&initial_dir));
        }
        let pfd: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_ALL).ok()?;

        let filter_pairs = i18n::dialog_filter_pairs(language, "dialog.save_audio_filter");
        let name_wides: Vec<Vec<u16>> =
            filter_pairs.iter().map(|(name, _)| to_wide(name)).collect();
        let pattern_wides: Vec<Vec<u16>> = filter_pairs
            .iter()
            .map(|(_, pattern)| to_wide(pattern))
            .collect();
        let spec: Vec<COMDLG_FILTERSPEC> = name_wides
            .iter()
            .zip(pattern_wides.iter())
            .map(|(name, pattern)| COMDLG_FILTERSPEC {
                pszName: PCWSTR(name.as_ptr()),
                pszSpec: PCWSTR(pattern.as_ptr()),
            })
            .collect();
        pfd.SetFileTypes(&spec).ok()?;
        pfd.SetFileTypeIndex(1).ok()?;
        pfd.SetDefaultExtension(w!("mp3")).ok()?;
        pfd.SetTitle(PCWSTR(
            to_wide(&i18n::tr(language, "dialog.save_audio_title")).as_ptr(),
        ))
        .ok()?;

        if !initial_dir.is_empty() {
            let initial_dir_w = to_wide(&initial_dir);
            if let Ok(shell_folder) = SHCreateItemFromParsingName::<_, _, IShellItem>(
                PCWSTR(initial_dir_w.as_ptr()),
                None,
            ) {
                let _unused = pfd.SetDefaultFolder(&shell_folder);
                let _unused = pfd.SetFolder(&shell_folder);
            }
        }

        if let Some(name) = suggested_name {
            let default_name = Path::new(name)
                .file_name()
                .and_then(|n| n.to_str())
                .filter(|n| !n.trim().is_empty())
                .unwrap_or(name);
            pfd.SetFileName(PCWSTR(to_wide(default_name).as_ptr()))
                .ok()?;
        }

        let current_bitrate =
            with_state(hwnd, |state| state.settings.audiobook_m4b_bitrate).unwrap_or(128);
        let bitrate_options = [64u32, 80, 96, 128, 160, 192, 256, 320];
        let initial_bitrate = if bitrate_options.contains(&current_bitrate) {
            current_bitrate
        } else {
            128
        };
        let selected_bitrate = Arc::new(Mutex::new(initial_bitrate));

        const AUDIO_SAVE_BITRATE_BUTTON_ID: u32 = 201;
        const AUDIO_SAVE_SPLIT_FOLDER_CHECKBOX_ID: u32 = 202;
        let pfdc: IFileDialogCustomize = pfd.cast().ok()?;
        pfdc.AddPushButton(
            AUDIO_SAVE_BITRATE_BUTTON_ID,
            PCWSTR(to_wide(&audiobook_bitrate_button_label(language, initial_bitrate)).as_ptr()),
        )
        .ok()?;
        if show_split_folder_option {
            pfdc.AddCheckButton(
                AUDIO_SAVE_SPLIT_FOLDER_CHECKBOX_ID,
                PCWSTR(to_wide(&i18n::tr(language, "dialog.save_audio_split_folder")).as_ptr()),
                BOOL(1),
            )
            .ok()?;
        }

        let handler: IFileDialogEvents = AudiobookBitrateDialogHandler {
            parent: hwnd,
            language,
            selected_bitrate: selected_bitrate.clone(),
            allowed_bitrates: bitrate_options.to_vec(),
        }
        .into();
        let cookie = pfd.Advise(&handler).ok()?;

        watchdog::enter_modal_dialog();
        let show_ok = pfd.Show(hwnd).is_ok();
        watchdog::exit_modal_dialog();
        pfd.Unadvise(cookie).ok()?;

        if show_ok {
            let item = pfd.GetResult().ok()?;
            let path_ptr = item
                .GetDisplayName(windows::Win32::UI::Shell::SIGDN_FILESYSPATH)
                .ok()?;
            let path_str = path_ptr.to_string().unwrap_or_default();
            CoTaskMemFree(Some(path_ptr.0 as *const _));

            let mut path = PathBuf::from(path_str);
            let has_valid_ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::trim)
                .is_some_and(|e| !e.is_empty());
            if !has_valid_ext {
                let filter_index = pfd.GetFileTypeIndex().ok().unwrap_or(1);
                if filter_index == 2 {
                    path.set_extension("m4b");
                } else {
                    path.set_extension("mp3");
                }
            }

            let selected_bitrate = selected_bitrate
                .lock()
                .map(|v| *v)
                .unwrap_or(current_bitrate.clamp(64, 320));
            log_debug(&format!(
                "Save audio dialog: selected bitrate {} kbps (current {} kbps)",
                selected_bitrate, current_bitrate
            ));
            if let Some(settings) = with_state(hwnd, |state| {
                state.settings.audiobook_m4b_bitrate = selected_bitrate;
                state.settings.clone()
            }) {
                save_settings(settings);
            } else {
                log_debug("Failed to access settings for audiobook bitrate");
            }

            let create_parts_folder = if show_split_folder_option {
                pfdc.GetCheckButtonState(AUDIO_SAVE_SPLIT_FOLDER_CHECKBOX_ID)
                    .ok()
                    .is_none_or(|state| state.as_bool())
            } else {
                false
            };

            Some(SaveAudioDialogResult {
                path,
                create_parts_folder,
            })
        } else {
            None
        }
    }
}

fn audiobook_bitrate_button_label(language: Language, bitrate_kbps: u32) -> String {
    let bitrate_label = i18n::tr(language, "podcast.bitrate");
    format!("{bitrate_label} ({bitrate_kbps} kbps)")
}

#[implement(IFileDialogEvents, IFileDialogControlEvents)]
struct AudiobookBitrateDialogHandler {
    parent: HWND,
    language: Language,
    selected_bitrate: Arc<Mutex<u32>>,
    allowed_bitrates: Vec<u32>,
}

impl IFileDialogEvents_Impl for AudiobookBitrateDialogHandler {
    fn OnFileOk(&self, _pfd: Option<&IFileDialog>) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnFolderChange(&self, _pfd: Option<&IFileDialog>) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnFolderChanging(
        &self,
        _pfd: Option<&IFileDialog>,
        _psi: Option<&IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnSelectionChange(&self, _pfd: Option<&IFileDialog>) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnShareViolation(
        &self,
        _pfd: Option<&IFileDialog>,
        _psi: Option<&IShellItem>,
    ) -> windows::core::Result<windows::Win32::UI::Shell::FDE_SHAREVIOLATION_RESPONSE> {
        Ok(windows::Win32::UI::Shell::FDESVR_DEFAULT)
    }
    fn OnTypeChange(&self, _pfd: Option<&IFileDialog>) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnOverwrite(
        &self,
        _pfd: Option<&IFileDialog>,
        _psi: Option<&IShellItem>,
    ) -> windows::core::Result<windows::Win32::UI::Shell::FDE_OVERWRITE_RESPONSE> {
        Ok(windows::Win32::UI::Shell::FDEOR_DEFAULT)
    }
}

impl IFileDialogControlEvents_Impl for AudiobookBitrateDialogHandler {
    fn OnItemSelected(
        &self,
        _pfdc: Option<&IFileDialogCustomize>,
        _dwidctl: u32,
        _dwiditem: u32,
    ) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnButtonClicked(
        &self,
        pfdc: Option<&IFileDialogCustomize>,
        dwidctl: u32,
    ) -> windows::core::Result<()> {
        const AUDIO_SAVE_BITRATE_BUTTON_ID: u32 = 201;
        if dwidctl != AUDIO_SAVE_BITRATE_BUTTON_ID {
            return Ok(());
        }
        let current = self
            .selected_bitrate
            .lock()
            .map(|v| *v)
            .unwrap_or(128)
            .clamp(64, 256);
        let menu = crate::create_popup_menu_safe();
        if menu.0 == 0 {
            return Ok(());
        }
        let mut ids = Vec::new();
        for (i, bitrate) in self.allowed_bitrates.iter().enumerate() {
            let id = 10_000u32 + i as u32;
            ids.push((id, *bitrate));
            let text = if *bitrate == current {
                format!("* {bitrate} kbps")
            } else {
                format!("{bitrate} kbps")
            };
            unsafe {
                crate::log_if_err!(AppendMenuW(
                    menu,
                    MF_STRING,
                    id as usize,
                    PCWSTR(to_wide(&text).as_ptr()),
                ));
            }
        }
        let mut pt = POINT::default();
        if crate::get_cursor_pos_safe(&mut pt).is_err() {
            return Ok(());
        }
        let owner = {
            let fg = crate::get_foreground_window_safe();
            if fg.0 != 0 { fg } else { self.parent }
        };
        let command = crate::track_popup_menu_safe(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            pt.x,
            pt.y,
            0,
            owner,
            None,
        );
        let chosen = ids
            .iter()
            .find(|(id, _)| *id == command.0 as u32)
            .map(|(_, bitrate)| *bitrate);
        if let Some(next) = chosen {
            if let Ok(mut guard) = self.selected_bitrate.lock() {
                *guard = next;
            }
            if let Some(pfdc) = pfdc {
                unsafe {
                    let label = audiobook_bitrate_button_label(self.language, next);
                    pfdc.SetControlLabel(dwidctl, PCWSTR(to_wide(&label).as_ptr()))
                        .ok();
                }
            }
        }
        Ok(())
    }

    fn OnCheckButtonToggled(
        &self,
        _pfdc: Option<&IFileDialogCustomize>,
        _dwidctl: u32,
        _pbchecked: windows::Win32::Foundation::BOOL,
    ) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnControlActivating(
        &self,
        _pfdc: Option<&IFileDialogCustomize>,
        _dwidctl: u32,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}
/// Mostra dialog per salvare file diagnostici zip.
fn export_diagnostics_dialog(hwnd: HWND) {
    unsafe {
        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();

        // Genera nome file con timestamp
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let default_name = format!("sonarpad_diagnostics_{}.zip", timestamp);
        let mut default_wide = to_wide(&default_name);
        default_wide.resize(260, 0);

        let zip_archive_label = i18n::tr(language, "dialog.zip_archive");
        let all_files_label = i18n::tr(language, "dialog.all_files");
        let filter = to_wide(&format!(
            "{} (*.zip)\0*.zip\0{} (*.*)\0*.*\0\0",
            zip_archive_label, all_files_label
        ));
        let title = to_wide(&i18n::tr(language, "dialog.export_diagnostics_title"));

        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: hwnd,
            lpstrFile: PWSTR(default_wide.as_mut_ptr()),
            nMaxFile: default_wide.len() as u32,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrTitle: PCWSTR(title.as_ptr()),
            lpstrDefExt: PCWSTR(to_wide("zip").as_ptr()),
            nFilterIndex: 1,
            Flags: OFN_EXPLORER | OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST,
            ..Default::default()
        };

        if !GetSaveFileNameW(&mut ofn).as_bool() {
            return;
        }

        let len = default_wide
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(default_wide.len());
        let path = PathBuf::from(String::from_utf16_lossy(&default_wide[..len]));

        match diagnostics::export_diagnostics_zip(&path) {
            Ok(()) => {
                let message = i18n::tr(language, "dialog.export_diagnostics_success")
                    .replace("{path}", &path.to_string_lossy());
                show_info(hwnd, language, &message);
            }
            Err(e) => {
                let message = format!(
                    "{}: {}",
                    i18n::tr(language, "dialog.export_diagnostics_error"),
                    e
                );
                show_error(hwnd, language, &message);
            }
        }
    }
}

pub(crate) fn show_error(hwnd: HWND, language: Language, message: &str) {
    show_error_with_id(hwnd, language, message, None);
}

pub(crate) fn show_error_with_id(
    hwnd: HWND,
    language: Language,
    message: &str,
    event_id: Option<&sentry_integration::EventId>,
) {
    let full_message = format!(
        "{}{}",
        message,
        sentry_integration::format_event_id(event_id)
    );
    log_debug(&format!("Error shown: {full_message}"));
    let wide = to_wide(&full_message);
    let title = to_wide(&error_title(language));
    show_blocking_modal_message_box(
        hwnd,
        BlockingModalKind::InfoDialog,
        PCWSTR(wide.as_ptr()),
        PCWSTR(title.as_ptr()),
        MB_OK | MB_ICONERROR,
    );
}

pub(crate) fn show_info(hwnd: HWND, language: Language, message: &str) {
    log_debug(&format!("Info shown: {message}"));
    let wide = to_wide(message);
    let title = to_wide(&info_title(language));
    show_blocking_modal_message_box(
        hwnd,
        BlockingModalKind::InfoDialog,
        PCWSTR(wide.as_ptr()),
        PCWSTR(title.as_ptr()),
        MB_OK | MB_ICONINFORMATION,
    );
}

/// Show an informational dialog owned by a secondary Sonarpad window while
/// keeping blocking-modal bookkeeping attached to the main application window.
pub(crate) fn show_info_owned_by(
    owner_hwnd: HWND,
    app_hwnd: HWND,
    language: Language,
    message: &str,
) {
    log_debug(&format!("Info shown: {message}"));
    let effective_owner = if owner_hwnd.0 != 0 && is_window_handle_valid(owner_hwnd) {
        owner_hwnd
    } else {
        log_debug(&format!(
            "Info dialog secondary owner invalid; falling back to main window: owner={owner_hwnd:?} main={app_hwnd:?}"
        ));
        app_hwnd
    };
    let wide = to_wide(message);
    let title = to_wide(&info_title(language));
    show_blocking_modal_message_box_owned(
        app_hwnd,
        effective_owner,
        BlockingModalKind::InfoDialog,
        PCWSTR(wide.as_ptr()),
        PCWSTR(title.as_ptr()),
        MB_OK | MB_ICONINFORMATION,
    );
}

/// Mostra un MessageBox generico sospendendo il watchdog durante la visualizzazione.
/// Usare per i MessageBox diretti che non passano da show_error/show_info.
pub(crate) fn message_box_modal(
    hwnd: HWND,
    message: PCWSTR,
    title: PCWSTR,
    flags: MESSAGEBOX_STYLE,
) -> MESSAGEBOX_RESULT {
    watchdog::enter_modal_dialog();
    let result = crate::message_box_w_safe(hwnd, message, title, flags);
    watchdog::exit_modal_dialog();
    result
}

pub(crate) fn recent_store_path() -> Option<PathBuf> {
    let mut path = settings::settings_dir();
    path.push("recent.json");
    Some(path)
}

fn load_recent_files() -> Vec<PathBuf> {
    let Some(path) = recent_store_path() else {
        return Vec::new();
    };
    let data = std::fs::read_to_string(path).ok();
    let Some(data) = data else {
        return Vec::new();
    };
    let store: RecentFileStore = serde_json::from_str(&data).unwrap_or_default();
    store
        .files
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .collect()
}

fn save_recent_files(files: &[PathBuf]) {
    let Some(path) = recent_store_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        crate::log_if_err!(std::fs::create_dir_all(parent));
    }
    let store = RecentFileStore {
        files: files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&store) {
        crate::log_if_err!(std::fs::write(path, json));
    }
}

#[implement(IFileDialogEvents, IFileDialogControlEvents)]
struct CustomFileDialogEventHandler {
    _encoding_label: String,
    _encodings: Vec<String>,
    _initial_encoding: TextEncoding,
    _txt_filter_index: u32,
    _filter_patterns: Vec<String>,
}

impl IFileDialogEvents_Impl for CustomFileDialogEventHandler {
    fn OnFileOk(&self, _pfd: Option<&IFileDialog>) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnFolderChange(&self, _pfd: Option<&IFileDialog>) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnFolderChanging(
        &self,
        _pfd: Option<&IFileDialog>,
        _psi: Option<&IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnSelectionChange(&self, _pfd: Option<&IFileDialog>) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnShareViolation(
        &self,
        _pfd: Option<&IFileDialog>,
        _psi: Option<&IShellItem>,
    ) -> windows::core::Result<windows::Win32::UI::Shell::FDE_SHAREVIOLATION_RESPONSE> {
        Ok(windows::Win32::UI::Shell::FDESVR_DEFAULT)
    }
    fn OnTypeChange(&self, pfd: Option<&IFileDialog>) -> windows::core::Result<()> {
        unsafe {
            let Some(pfd) = pfd else {
                return Ok(());
            };
            let filter_index = pfd.GetFileTypeIndex()?;
            crate::log_debug(&format!("OnTypeChange: filter_index = {}", filter_index));
            if let Some(pattern) = self
                ._filter_patterns
                .get(filter_index.saturating_sub(1) as usize)
            {
                let extension = default_extension_from_filter_pattern(pattern).unwrap_or_default();
                let extension_wide = to_wide(&extension);
                crate::log_if_err!(pfd.SetDefaultExtension(PCWSTR(extension_wide.as_ptr())));

                if let Ok(file_name_ptr) = pfd.GetFileName() {
                    let current_name = file_name_ptr.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(file_name_ptr.0 as *const _));
                    if !current_name.trim().is_empty() {
                        let mut updated_path = PathBuf::from(&current_name);
                        apply_selected_save_filter_extension(&mut updated_path, pattern);
                        let updated_name = updated_path.to_string_lossy();
                        if updated_name.as_ref() != current_name.as_str() {
                            let updated_name_wide = to_wide(updated_name.as_ref());
                            crate::log_if_err!(
                                pfd.SetFileName(PCWSTR(updated_name_wide.as_ptr())),
                                "Save dialog: update visible filename extension failed"
                            );
                        }
                    }
                }
            }
            let pfdc: IFileDialogCustomize = pfd.cast()?;
            let is_txt = filter_index == self._txt_filter_index;
            if is_txt {
                crate::log_debug("OnTypeChange: showing encoding combobox");
                // Show the ComboBox (101)
                pfdc.SetControlState(
                    101,
                    windows::Win32::UI::Shell::CDCS_VISIBLE
                        | windows::Win32::UI::Shell::CDCS_ENABLED,
                )?;
            } else {
                crate::log_debug("OnTypeChange: hiding encoding combobox");
                // Hide the ComboBox (101)
                pfdc.SetControlState(101, windows::Win32::UI::Shell::CDCS_INACTIVE)?;
            }
        }
        Ok(())
    }
    fn OnOverwrite(
        &self,
        _pfd: Option<&IFileDialog>,
        _psi: Option<&IShellItem>,
    ) -> windows::core::Result<windows::Win32::UI::Shell::FDE_OVERWRITE_RESPONSE> {
        Ok(windows::Win32::UI::Shell::FDEOR_DEFAULT)
    }
}

impl IFileDialogControlEvents_Impl for CustomFileDialogEventHandler {
    fn OnItemSelected(
        &self,
        _pfdc: Option<&IFileDialogCustomize>,
        _dwidctl: u32,
        _dwiditem: u32,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnButtonClicked(
        &self,
        _pfdc: Option<&IFileDialogCustomize>,
        _dwidctl: u32,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnCheckButtonToggled(
        &self,
        _pfdc: Option<&IFileDialogCustomize>,
        _dwidctl: u32,
        _pbchecked: windows::Win32::Foundation::BOOL,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnControlActivating(
        &self,
        _pfdc: Option<&IFileDialogCustomize>,
        _dwidctl: u32,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

fn encoding_to_index(enc: TextEncoding) -> u32 {
    match enc {
        TextEncoding::Ansi => 0,
        TextEncoding::Utf8 => 1,
        TextEncoding::Utf8Bom => 2,
        TextEncoding::Utf16Le => 3,
        TextEncoding::Utf16Be => 4,
    }
}

fn index_to_encoding(index: u32) -> TextEncoding {
    match index {
        0 => TextEncoding::Ansi,
        1 => TextEncoding::Utf8,
        2 => TextEncoding::Utf8Bom,
        3 => TextEncoding::Utf16Le,
        4 => TextEncoding::Utf16Be,
        _ => TextEncoding::Utf8,
    }
}

pub(crate) fn open_file_dialog_with_encoding(
    hwnd: HWND,
) -> Option<Vec<(PathBuf, Option<TextEncoding>)>> {
    unsafe {
        log_debug("open_file_dialog_with_encoding called");
        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();

        use windows::Win32::UI::Shell::FileOpenDialog;
        use windows::Win32::UI::Shell::IFileOpenDialog;
        use windows::Win32::UI::Shell::{FILEOPENDIALOGOPTIONS, FOS_ALLOWMULTISELECT};

        let pfd: IFileOpenDialog = match CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL) {
            Ok(dialog) => {
                log_debug("FileOpenDialog created successfully");
                dialog
            }
            Err(e) => {
                log_debug(&format!("Failed to create FileOpenDialog: {:?}", e));
                return None;
            }
        };

        let filter_pairs = i18n::dialog_filter_pairs(language, "dialog.open_filter");
        let name_wides: Vec<Vec<u16>> =
            filter_pairs.iter().map(|(name, _)| to_wide(name)).collect();
        let pattern_wides: Vec<Vec<u16>> = filter_pairs
            .iter()
            .map(|(_, pattern)| to_wide(pattern))
            .collect();
        let spec: Vec<COMDLG_FILTERSPEC> = name_wides
            .iter()
            .zip(pattern_wides.iter())
            .map(|(name, pattern)| COMDLG_FILTERSPEC {
                pszName: PCWSTR(name.as_ptr()),
                pszSpec: PCWSTR(pattern.as_ptr()),
            })
            .collect();
        pfd.SetFileTypes(&spec).ok()?;
        pfd.SetFileTypeIndex(1).ok()?; // Default to "All supported formats"
        let mut options = pfd.GetOptions().ok()?;
        options |= FILEOPENDIALOGOPTIONS(FOS_ALLOWMULTISELECT.0);
        pfd.SetOptions(options).ok()?;

        let pfdc: IFileDialogCustomize = pfd.cast().ok()?;
        let encoding_label = i18n::tr(language, "dialog.encoding_label");
        let encodings = vec![
            i18n::tr(language, "encoding.ansi"),
            i18n::tr(language, "encoding.utf8"),
            i18n::tr(language, "encoding.utf8bom"),
            i18n::tr(language, "encoding.utf16le"),
            i18n::tr(language, "encoding.utf16be"),
        ];

        log_debug("Adding encoding controls to open dialog");

        // Use ComboBox with "Codifica: " prefix in each item for NVDA
        pfdc.AddComboBox(101).ok()?;

        for (i, enc_name) in encodings.iter().enumerate() {
            let item_text = format!("{} {}", encoding_label, enc_name);
            pfdc.AddControlItem(101, i as u32, PCWSTR(to_wide(&item_text).as_ptr()))
                .ok()?;
        }
        pfdc.SetSelectedControlItem(101, encoding_to_index(TextEncoding::Utf8))
            .ok()?;

        let handler: IFileDialogEvents = CustomFileDialogEventHandler {
            _encoding_label: encoding_label,
            _encodings: encodings,
            _initial_encoding: TextEncoding::Utf8,
            _txt_filter_index: 2,
            _filter_patterns: Vec::new(),
        }
        .into();
        let cookie = pfd.Advise(&handler).ok()?;
        log_debug(&format!(
            "Event handler registered with cookie: {:?}",
            cookie
        ));

        // Trigger OnTypeChange to set initial visibility
        // Default index 1 = "All supported formats", encoding will be hidden
        log_debug("Triggering initial OnTypeChange");
        crate::log_if_err!(pfd.SetFileTypeIndex(1));

        log_debug("Showing open dialog");
        if pfd.Show(hwnd).is_ok() {
            log_debug("Dialog closed with OK");
            let selected_encoding_idx = pfdc.GetSelectedControlItem(101).ok()?;
            let filter_index = pfd.GetFileTypeIndex().ok()?;
            let manual_encoding = if filter_index == 2 {
                Some(index_to_encoding(selected_encoding_idx))
            } else {
                None
            };
            let mut out = Vec::new();
            if let Ok(items) = pfd.GetResults()
                && let Ok(count) = items.GetCount()
            {
                for i in 0..count {
                    if let Ok(item) = items.GetItemAt(i)
                        && let Ok(path_ptr) =
                            item.GetDisplayName(windows::Win32::UI::Shell::SIGDN_FILESYSPATH)
                    {
                        let path_str = path_ptr.to_string().unwrap_or_default();
                        CoTaskMemFree(Some(path_ptr.0 as *const _));
                        if !path_str.is_empty() {
                            out.push((PathBuf::from(path_str), None));
                        }
                    }
                }
            }

            if out.is_empty() {
                let item = pfd.GetResult().ok()?;
                let path_ptr = item
                    .GetDisplayName(windows::Win32::UI::Shell::SIGDN_FILESYSPATH)
                    .ok()?;
                let path_str = path_ptr.to_string().unwrap_or_default();
                CoTaskMemFree(Some(path_ptr.0 as *const _));
                if path_str.is_empty() {
                    pfd.Unadvise(cookie).ok()?;
                    return None;
                }
                out.push((PathBuf::from(path_str), manual_encoding));
            } else if out.len() == 1 {
                out[0].1 = manual_encoding;
            }

            pfd.Unadvise(cookie).ok()?;
            Some(out)
        } else {
            pfd.Unadvise(cookie).ok()?;
            None
        }
    }
}

pub(crate) fn open_subtitle_file_dialog(hwnd: HWND) -> Option<PathBuf> {
    let language = { with_state(hwnd, |state| state.settings.language) }.unwrap_or_default();
    let title = i18n::tr(language, "dialog.open_subtitles_title");
    let filter_label = i18n::tr(language, "dialog.subtitles_filter");
    let all_files_label = i18n::tr(language, "dialog.all_files");

    let pattern = crate::subtitles::SUBTITLE_EXTENSIONS
        .iter()
        .map(|ext| format!("*.{ext}"))
        .collect::<Vec<_>>()
        .join(";");

    let filter = format!(
        "{} ({})\0{}\0{} (*.*)\0*.*\0",
        filter_label, pattern, pattern, all_files_label
    );
    let mut filter_wide: Vec<u16> = filter.encode_utf16().collect();
    filter_wide.push(0);

    let mut buffer = [0u16; 1024];
    let title_wide = to_wide(&title);
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd,
        lpstrFilter: PCWSTR(filter_wide.as_ptr()),
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: PCWSTR(title_wide.as_ptr()),
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY,
        ..Default::default()
    };

    if !crate::get_open_file_name_w_safe(&mut ofn).as_bool() {
        return None;
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    if len == 0 {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(&buffer[..len])))
}

fn filter_pattern_contains_extension(pattern: &str, extension: &str) -> bool {
    let wanted = extension.trim_start_matches('.');
    pattern.split(';').any(|part| {
        part.trim()
            .strip_prefix("*.")
            .is_some_and(|value| value.eq_ignore_ascii_case(wanted))
    })
}

fn default_extension_from_filter_pattern(pattern: &str) -> Option<String> {
    pattern.split(';').find_map(|part| {
        let extension = part.trim().strip_prefix("*.")?;
        (!extension.is_empty() && extension != "*").then(|| extension.to_string())
    })
}

fn apply_selected_save_filter_extension(path: &mut PathBuf, pattern: &str) {
    if pattern
        .split(';')
        .any(|part| part.trim() == "*.*" || part.trim() == "*")
    {
        return;
    }

    let Some(default_extension) = default_extension_from_filter_pattern(pattern) else {
        return;
    };
    let current_extension = path.extension().and_then(|value| value.to_str());
    if current_extension
        .is_none_or(|extension| !filter_pattern_contains_extension(pattern, extension))
    {
        path.set_extension(default_extension);
    }
}

pub(crate) fn save_file_dialog_with_encoding(
    hwnd: HWND,
    suggested_name: Option<&str>,
    initial_encoding: TextEncoding,
    allow_epub: bool,
    preferred_extension: Option<&str>,
) -> Option<(PathBuf, TextEncoding)> {
    unsafe {
        let language = with_state(hwnd, |state| state.settings.language).unwrap_or_default();

        let pfd: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_ALL).ok()?;

        let mut filter_pairs = i18n::dialog_filter_pairs(language, "dialog.save_filter");
        if allow_epub
            && !filter_pairs
                .iter()
                .any(|(_, pattern)| filter_pattern_contains_extension(pattern, "epub"))
        {
            let insertion_index = filter_pairs
                .iter()
                .position(|(_, pattern)| pattern.trim() == "*.*")
                .unwrap_or(filter_pairs.len());
            filter_pairs.insert(
                insertion_index,
                ("EPUB (*.epub)".to_string(), "*.epub".to_string()),
            );
        }

        let wanted_extension = preferred_extension.unwrap_or("txt").trim_start_matches('.');
        let preferred_filter_index = filter_pairs
            .iter()
            .position(|(_, pattern)| filter_pattern_contains_extension(pattern, wanted_extension))
            .map(|index| index as u32 + 1)
            .unwrap_or(1);
        let txt_filter_index = filter_pairs
            .iter()
            .position(|(_, pattern)| filter_pattern_contains_extension(pattern, "txt"))
            .map(|index| index as u32 + 1)
            .unwrap_or(1);
        let filter_patterns: Vec<String> = filter_pairs
            .iter()
            .map(|(_, pattern)| pattern.clone())
            .collect();

        let name_wides: Vec<Vec<u16>> =
            filter_pairs.iter().map(|(name, _)| to_wide(name)).collect();
        let pattern_wides: Vec<Vec<u16>> = filter_pairs
            .iter()
            .map(|(_, pattern)| to_wide(pattern))
            .collect();
        let specs: Vec<COMDLG_FILTERSPEC> = name_wides
            .iter()
            .zip(pattern_wides.iter())
            .map(|(name, pattern)| COMDLG_FILTERSPEC {
                pszName: PCWSTR(name.as_ptr()),
                pszSpec: PCWSTR(pattern.as_ptr()),
            })
            .collect();
        pfd.SetFileTypes(&specs).ok()?;
        pfd.SetFileTypeIndex(preferred_filter_index).ok()?;
        let default_extension_wide = to_wide(wanted_extension);
        pfd.SetDefaultExtension(PCWSTR(default_extension_wide.as_ptr()))
            .ok()?;

        let route_document = with_state(hwnd, |state| {
            state
                .docs
                .get(state.current)
                .and_then(|doc| doc.route_map.as_ref())
                .is_some()
        })
        .unwrap_or(false);
        let initial_dir = if route_document {
            settings::default_documents_save_folder()
        } else {
            with_state(hwnd, |state| state.settings.documents_save_folder.clone())
                .map(|path| path.trim().to_string())
                .filter(|path| !path.is_empty())
                .unwrap_or_else(settings::default_documents_save_folder)
        };
        crate::log_if_err!(std::fs::create_dir_all(&initial_dir));
        let initial_dir_wide = to_wide(&initial_dir);
        if let Ok(shell_folder) =
            SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(initial_dir_wide.as_ptr()), None)
        {
            let _set_default_folder_result = pfd.SetDefaultFolder(&shell_folder);
            let _set_folder_result = pfd.SetFolder(&shell_folder);
        }

        if let Some(name) = suggested_name {
            let suggested_name_wide = to_wide(name);
            pfd.SetFileName(PCWSTR(suggested_name_wide.as_ptr())).ok()?;
        }

        let pfdc: IFileDialogCustomize = pfd.cast().ok()?;
        let encoding_label = i18n::tr(language, "dialog.encoding_label");
        let encodings = vec![
            i18n::tr(language, "encoding.ansi"),
            i18n::tr(language, "encoding.utf8"),
            i18n::tr(language, "encoding.utf8bom"),
            i18n::tr(language, "encoding.utf16le"),
            i18n::tr(language, "encoding.utf16be"),
        ];

        pfdc.AddComboBox(101).ok()?;
        for (index, encoding_name) in encodings.iter().enumerate() {
            let item_text = format!("{} {}", encoding_label, encoding_name);
            let item_text_wide = to_wide(&item_text);
            pfdc.AddControlItem(101, index as u32, PCWSTR(item_text_wide.as_ptr()))
                .ok()?;
        }
        pfdc.SetSelectedControlItem(101, encoding_to_index(initial_encoding))
            .ok()?;

        let handler: IFileDialogEvents = CustomFileDialogEventHandler {
            _encoding_label: encoding_label,
            _encodings: encodings,
            _initial_encoding: initial_encoding,
            _txt_filter_index: txt_filter_index,
            _filter_patterns: filter_patterns,
        }
        .into();
        let cookie = pfd.Advise(&handler).ok()?;
        crate::log_if_err!(pfd.SetFileTypeIndex(preferred_filter_index));

        if pfd.Show(hwnd).is_ok() {
            let item = pfd.GetResult().ok()?;
            let path_ptr = item
                .GetDisplayName(windows::Win32::UI::Shell::SIGDN_FILESYSPATH)
                .ok()?;
            let path_string = path_ptr.to_string().unwrap_or_default();
            CoTaskMemFree(Some(path_ptr.0 as *const _));

            let selected_encoding_index = pfdc.GetSelectedControlItem(101).ok()?;
            let filter_index = pfd.GetFileTypeIndex().ok()?;
            let mut path = PathBuf::from(path_string);
            if let Some((_, pattern)) = filter_pairs.get(filter_index.saturating_sub(1) as usize) {
                apply_selected_save_filter_extension(&mut path, pattern);
            }

            pfd.Unadvise(cookie).ok()?;
            Some((path, index_to_encoding(selected_encoding_index)))
        } else {
            pfd.Unadvise(cookie).ok()?;
            None
        }
    }
}
