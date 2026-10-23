use crate::WM_FOCUS_EDITOR;
use crate::accessibility::{handle_accessibility, to_wide};
use crate::i18n;
use crate::settings::{self, Language, save_settings};
use crate::tools::bdciechi;
use crate::with_state;
use chrono::Utc;
use encoding_rs::WINDOWS_1252;
use sha2::Digest;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{
    OFN_EXPLORER, OFN_HIDEREADONLY, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Controls::WC_COMBOBOXW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, SetFocus, VK_ESCAPE, VK_RETURN, VK_SPACE, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_RESETCONTENT, CB_SETCURSEL, CBS_DROPDOWNLIST,
    CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, ES_AUTOHSCROLL, ES_AUTOVSCROLL,
    ES_MULTILINE, ES_PASSWORD, ES_READONLY, GWLP_USERDATA, GetWindowLongPtrW, HMENU, IDC_ARROW,
    IDYES, LoadCursorW, MB_ICONQUESTION, MB_YESNO, MESSAGEBOX_STYLE, RegisterClassW, SC_CLOSE,
    SW_HIDE, SW_SHOW, SetForegroundWindow, SetWindowLongPtrW, WINDOW_STYLE, WM_APP, WM_CLOSE,
    WM_COMMAND, WM_CREATE, WM_DESTROY, WM_KEYDOWN, WM_NCDESTROY, WM_SETFOCUS, WM_SYSCOMMAND,
    WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_POPUP,
    WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{PCWSTR, PWSTR, w};

const BDC_CLASS_NAME: &str = "SonarpadBdCiechiWindow";

const IDC_USER: usize = 9941;
const IDC_PASS: usize = 9942;
const IDC_LOGIN: usize = 9943;
const IDC_SEARCH: usize = 9944;
const IDC_SEARCH_BTN: usize = 9945;
const IDC_LATEST_BTN: usize = 9946;
const IDC_RESULTS_COMBO: usize = 9947;
const IDC_DOWNLOAD_BTN: usize = 9948;
const IDC_CLOSE_BTN: usize = 9949;
const IDC_STATUS: usize = 9950;
const IDC_SAMPLE_BTN: usize = 9951;
const IDC_SAMPLE_EDIT: usize = 9952;
const IDC_SAMPLE_CLOSE_BTN: usize = 9953;
const IDC_LOGOUT_BTN: usize = 9954;
const WM_BDC_LOGIN_DONE: u32 = WM_APP + 240;
const BDCIECHI_CREDENTIAL_MAX_AGE_SECS: i64 = 30 * 24 * 60 * 60;

struct LoginDonePayload {
    username: String,
    password: String,
    nprov: String,
    quota: Option<bdciechi::BdcQuota>,
    catalog_raw: String,
    auto_login: bool,
    error: Option<String>,
}

struct BdState {
    parent: HWND,
    user_label: HWND,
    pass_label: HWND,
    user: HWND,
    pass: HWND,
    login: HWND,
    search: HWND,
    search_btn: HWND,
    latest_btn: HWND,
    results_label: HWND,
    results_combo: HWND,
    download_btn: HWND,
    sample_btn: HWND,
    logout_btn: HWND,
    sample_edit: HWND,
    sample_close_btn: HWND,
    close_btn: HWND,
    status: HWND,
    username: String,
    password: String,
    nprov: String,
    authenticated: bool,
    remember_credentials: bool,
    focus_editor_on_close: bool,
    catalog_rows: Vec<String>,
    visible_indices: Vec<usize>,
    login_announce_stop: Option<Arc<AtomicBool>>,
}

fn with_window_state<F, R>(hwnd: HWND, f: F) -> Option<R>
where
    F: FnOnce(&mut BdState) -> R,
{
    let ptr = crate::get_window_long_ptr_w_safe(hwnd, GWLP_USERDATA) as *mut BdState;
    crate::with_raw_mut_ptr_safe(ptr, f)
}

fn app_language(parent: HWND) -> Language {
    with_state(parent, |state| state.settings.language).unwrap_or_default()
}

fn tr(key: &str) -> String {
    i18n::tr(
        Language::Italian,
        &format!("excluded_from_testing.bdciechi.{key}"),
    )
}

fn set_status(state: &BdState, text: &str) {
    crate::log_if_err!(crate::set_window_text_w_safe(
        state.status,
        PCWSTR(to_wide(text).as_ptr())
    ));
}

fn set_sample_text(state: &BdState, text: &str) {
    crate::log_if_err!(crate::set_window_text_w_safe(
        state.sample_edit,
        PCWSTR(to_wide(text).as_ptr())
    ));
}

fn format_quota_message(quota: &bdciechi::BdcQuota) -> String {
    format!(
        "Libri ancora disponibili questo mese: {} su {}.",
        quota.remaining, quota.monthly_total
    )
}

fn speak_bdc_message_delayed(message: String) {
    match std::thread::Builder::new()
        .name("bdciechi-quota-speak".to_string())
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            if !crate::accessibility::screen_reader_speak(&message) {
                crate::log_debug("bdciechi: delayed screen reader speak failed");
            }
        }) {
        Ok(_handle) => {}
        Err(err) => crate::log_debug(&format!(
            "bdciechi: failed to spawn delayed speak thread: {err}"
        )),
    }
}

fn set_sample_visible(state: &BdState, visible: bool) {
    let cmd = if visible { SW_SHOW } else { SW_HIDE };
    crate::show_window_safe(state.sample_edit, cmd);
    crate::show_window_safe(state.sample_close_btn, cmd);
    crate::enable_window_safe(state.sample_edit, visible);
    crate::enable_window_safe(state.sample_close_btn, visible);
}

fn close_sample_and_focus_search(state: &BdState) {
    set_sample_visible(state, false);
    crate::set_focus_safe(state.search);
}

fn close_bdc_window_and_focus_editor(hwnd: HWND) {
    let parent = with_window_state(hwnd, |state| {
        state.focus_editor_on_close = true;
        state.parent
    });
    if let Some(parent) = parent {
        crate::show_window_safe(hwnd, SW_HIDE);
        with_state(parent, |state| state.bdciechi_window = HWND(0));
        crate::restore_editor_focus(parent);
    }
    crate::log_if_err!(crate::destroy_window_safe(hwnd));
}

fn stop_login_announcer(state: &mut BdState) {
    if let Some(flag) = state.login_announce_stop.take() {
        flag.store(true, Ordering::Relaxed);
    }
}

fn enable_logged_controls(state: &BdState, enabled: bool) {
    unsafe {
        EnableWindow(state.search, enabled);
        EnableWindow(state.search_btn, enabled);
        EnableWindow(state.latest_btn, enabled);
        EnableWindow(state.results_combo, enabled);
        EnableWindow(state.logout_btn, enabled);
    }
}

fn set_login_controls_visible(state: &BdState, visible: bool) {
    let cmd = if visible { SW_SHOW } else { SW_HIDE };
    for control in [
        state.user_label,
        state.pass_label,
        state.user,
        state.pass,
        state.login,
    ] {
        crate::show_window_safe(control, cmd);
        crate::enable_window_safe(control, visible);
    }
}

fn set_logout_button_visible(state: &BdState, visible: bool) {
    let cmd = if visible { SW_SHOW } else { SW_HIDE };
    crate::show_window_safe(state.logout_btn, cmd);
    crate::enable_window_safe(state.logout_btn, visible);
}

fn set_result_action_buttons_visible(state: &BdState, visible: bool) {
    let cmd = if visible { SW_SHOW } else { SW_HIDE };
    for control in [state.download_btn, state.sample_btn] {
        crate::show_window_safe(control, cmd);
        crate::enable_window_safe(control, visible);
    }
}

fn set_results_visible(state: &BdState, visible: bool) {
    let cmd = if visible { SW_SHOW } else { SW_HIDE };
    for control in [state.results_label, state.results_combo] {
        crate::show_window_safe(control, cmd);
        crate::enable_window_safe(control, visible);
    }
}

fn focus_primary_bdc_control(state: &BdState) {
    if state.authenticated {
        crate::set_focus_safe(state.search);
    } else {
        crate::set_focus_safe(state.user);
    }
}

fn read_edit_text(hwnd: HWND) -> String {
    let len = crate::get_window_text_length_w_safe(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let read = crate::get_window_text_w_safe(hwnd, &mut buf);
    if read <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..read as usize])
}

fn sync_bdc_session(
    parent: HWND,
    username: &str,
    password: &str,
    nprov: &str,
    catalog_rows: &[String],
    authenticated: bool,
) {
    let _updated = with_state(parent, |state| {
        state.bdciechi_session_username = username.to_string();
        state.bdciechi_session_password = password.to_string();
        state.bdciechi_session_nprov = nprov.to_string();
        state.bdciechi_session_catalog_rows = catalog_rows.to_vec();
        state.bdciechi_session_authenticated = authenticated;
    });
}

fn persist_bdc_credentials(
    parent: HWND,
    remember: bool,
    username: &str,
    password: &str,
    last_successful_login_unix: i64,
) {
    let snapshot = with_state(parent, |state| {
        state.settings.remember_bdciechi_credentials = remember;
        if remember {
            state.settings.bdciechi_username = username.to_string();
            state.settings.bdciechi_password = password.to_string();
            state.settings.bdciechi_last_successful_login_unix = last_successful_login_unix;
        } else {
            state.settings.bdciechi_username.clear();
            state.settings.bdciechi_password.clear();
            state.settings.bdciechi_last_successful_login_unix = 0;
        }
        state.settings.clone()
    });
    if let Some(settings) = snapshot {
        save_settings(settings);
    }
}

fn sanitize_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "opera.txt".to_string()
    } else {
        trimmed.to_string()
    }
}

fn strip_known_extension<'a>(name: &'a str, extension: &str) -> &'a str {
    let suffix = format!(".{extension}");
    if name.len() > suffix.len() && name.ends_with(&suffix) {
        &name[..name.len() - suffix.len()]
    } else {
        name
    }
}

fn format_bdc_catalog_filename_base(name: &str) -> String {
    let mut formatted = String::with_capacity(name.len());
    let mut chars = name.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '_' {
            let prev_is_space = formatted.ends_with(' ');
            let next_is_space = chars.peek().copied() == Some(' ');
            if !prev_is_space {
                formatted.push(' ');
            }
            formatted.push('-');
            if !next_is_space {
                formatted.push(' ');
            }
        } else {
            formatted.push(ch);
        }
    }
    formatted.trim().to_string()
}

fn decode_book_text(bytes: &[u8]) -> String {
    let decoded = if let Ok(text) = std::str::from_utf8(bytes) {
        text.to_string()
    } else {
        let (decoded, _, _) = WINDOWS_1252.decode(bytes);
        decoded.to_string()
    };
    repair_utf8_mojibake(&decoded)
}

fn utf8_mojibake_score(text: &str) -> usize {
    text.chars()
        .filter(|ch| matches!(ch, 'Ã' | 'Â' | 'â' | '€' | '™' | 'œ' | 'ž'))
        .count()
}

fn repair_utf8_mojibake(text: &str) -> String {
    let original_score = utf8_mojibake_score(text);
    if original_score == 0 {
        return text.to_string();
    }

    let (encoded, _, had_unmappables) = WINDOWS_1252.encode(text);
    if !had_unmappables
        && let Ok(repaired) = String::from_utf8(encoded.into_owned())
        && utf8_mojibake_score(&repaired) < original_score
    {
        return repaired;
    }

    repair_common_mojibake_sequences(text)
}

fn repair_common_mojibake_sequences(text: &str) -> String {
    text.replace("â€™", "’")
        .replace("â€˜", "‘")
        .replace("â€œ", "“")
        .replace("â€", "”")
        .replace("â€“", "–")
        .replace("â€”", "—")
        .replace("â€¦", "…")
        .replace("Â ", " ")
        .replace("Ã ", "à")
        .replace("Ã¨", "è")
        .replace("Ã©", "é")
        .replace("Ã¬", "ì")
        .replace("Ã²", "ò")
        .replace("Ã¹", "ù")
        .replace("Ã€", "À")
        .replace("Ãˆ", "È")
        .replace("Ã‰", "É")
        .replace("ÃŒ", "Ì")
        .replace("Ã’", "Ò")
        .replace("Ã™", "Ù")
}

fn fold_search_char(ch: char) -> char {
    match ch {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' | 'đ' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' | 'ħ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' | 'ŉ' | 'ŋ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' | 'ŧ' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        'æ' => 'a',
        'œ' => 'o',
        _ => ch,
    }
}

fn normalize_search_text(text: &str) -> String {
    text.chars()
        .flat_map(|ch| ch.to_lowercase())
        .map(fold_search_char)
        .collect()
}

fn tokenize_search_terms(text: &str) -> Vec<String> {
    normalize_search_text(text)
        .split(|ch: char| ch.is_whitespace() || ch == ',')
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn row_matches_query(row: &str, query_terms: &[String]) -> bool {
    if query_terms.is_empty() {
        return false;
    }
    let normalized_row = normalize_search_text(row);
    query_terms.iter().all(|term| normalized_row.contains(term))
}

fn suggested_name_from_info(info: &str, row: &str, index: usize) -> String {
    let extension = info
        .split(';')
        .nth(3)
        .and_then(|path_field| {
            let trimmed = path_field.trim();
            if trimmed.is_empty() {
                None
            } else {
                trimmed
                    .rsplit('\\')
                    .next()
                    .and_then(|name| Path::new(name.trim()).extension())
                    .and_then(|ext| ext.to_str())
                    .filter(|ext| !ext.trim().is_empty())
                    .map(|ext| ext.trim().to_string())
            }
        })
        .unwrap_or_else(|| "txt".to_string());

    let row_name = format_bdc_catalog_filename_base(&sanitize_filename(
        strip_known_extension(row.trim(), &extension).trim(),
    ));
    if !row_name.is_empty() && row_name != "opera.txt" {
        return format!("{row_name}.{extension}");
    }

    if let Some(path_field) = info.split(';').nth(3) {
        let trimmed = path_field.trim();
        if !trimmed.is_empty()
            && let Some(name) = trimmed.rsplit('\\').next()
        {
            let name = sanitize_filename(name.trim());
            if !name.is_empty() {
                return name;
            }
        }
    }
    format!("opera_{index}.txt")
}

fn default_docs_folder(parent: HWND) -> PathBuf {
    let configured = with_state(parent, |state| state.settings.documents_save_folder.clone())
        .map(|p| p.trim().to_string())
        .unwrap_or_default();
    if configured.is_empty() {
        PathBuf::from(settings::default_documents_save_folder())
    } else {
        PathBuf::from(configured)
    }
}

fn bdciechi_fallback_download_dir() -> PathBuf {
    settings::settings_dir().join("bdciechi").join("downloads")
}

fn bdciechi_catalog_cache_path() -> PathBuf {
    settings::settings_dir()
        .join("bdciechi")
        .join("catalog_cache.txt")
}

fn load_bdciechi_catalog_cache() -> Option<String> {
    let path = bdciechi_catalog_cache_path();
    match fs::read_to_string(&path) {
        Ok(raw) if !raw.trim().is_empty() => Some(raw),
        Ok(_) => None,
        Err(err) => {
            crate::log_debug(&format!(
                "bdciechi: failed to read catalog cache {}: {}",
                path.display(),
                err
            ));
            None
        }
    }
}

fn save_bdciechi_catalog_cache(raw: &str) {
    let path = bdciechi_catalog_cache_path();
    if let Some(parent) = path.parent()
        && let Err(err) = fs::create_dir_all(parent)
    {
        crate::log_debug(&format!(
            "bdciechi: failed to create cache dir {}: {}",
            parent.display(),
            err
        ));
        return;
    }
    if let Err(err) = fs::write(&path, raw) {
        crate::log_debug(&format!(
            "bdciechi: failed to write catalog cache {}: {}",
            path.display(),
            err
        ));
    }
}

fn catalog_hash(raw: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn catalog_cache_date(raw: &str) -> Option<String> {
    raw.lines()
        .map(str::trim)
        .find(|line| line.starts_with('[') && line.ends_with(']') && line.contains('/'))
        .map(|line| line.trim_start_matches('[').trim_end_matches(']'))
        .and_then(|date| {
            let mut parts = date.split('/');
            let day = parts.next()?.trim();
            let month = parts.next()?.trim();
            let year = parts.next()?.trim();
            if parts.next().is_some() {
                return None;
            }
            let day_num = day.parse::<u32>().ok()?;
            let month_num = month.parse::<u32>().ok()?;
            let year_num = year.parse::<u32>().ok()?;
            Some(format!("{year_num:04}-{month_num:02}-{day_num:02}"))
        })
}

fn refresh_catalog_if_needed(state: &mut BdState) -> Result<bool, String> {
    let utc_info = bdciechi::fetch_catalog_utc(&state.nprov)?;
    crate::log_debug(&format!(
        "bdciechi: utc check server_utc={} catalog_date={} catalog_ubound={}",
        utc_info.server_utc, utc_info.catalog_date, utc_info.catalog_ubound
    ));

    let current_count = state.catalog_rows.len();
    let current_ubound = current_count.saturating_sub(1);
    let cached_catalog_date =
        load_bdciechi_catalog_cache().and_then(|raw| catalog_cache_date(&raw));
    let catalog_date_matches =
        cached_catalog_date.as_deref() == Some(utc_info.catalog_date.as_str());
    let catalog_count_matches = if current_count == 0 {
        false
    } else {
        current_ubound == utc_info.catalog_ubound
    };

    if catalog_date_matches && catalog_count_matches {
        return Ok(false);
    }

    let catalog_raw = bdciechi::fetch_catalog_list(&state.nprov)?;
    let cached_hash = load_bdciechi_catalog_cache()
        .map(|raw| catalog_hash(&raw))
        .unwrap_or_default();
    let remote_hash = catalog_hash(&catalog_raw);
    state.catalog_rows = bdciechi::parse_catalog_records(&catalog_raw);
    if cached_hash != remote_hash {
        save_bdciechi_catalog_cache(&catalog_raw);
    }
    sync_bdc_session(
        state.parent,
        &state.username,
        &state.password,
        &state.nprov,
        &state.catalog_rows,
        true,
    );
    Ok(true)
}

fn bdciechi_credentials_expired(last_successful_login_unix: i64) -> bool {
    if last_successful_login_unix <= 0 {
        return true;
    }
    let now = Utc::now().timestamp();
    now.saturating_sub(last_successful_login_unix) > BDCIECHI_CREDENTIAL_MAX_AGE_SECS
}

fn ensure_bdciechi_download_dir(parent: HWND) -> Result<PathBuf, String> {
    let docs_dir = default_docs_folder(parent);
    match fs::create_dir_all(&docs_dir) {
        Ok(()) => Ok(docs_dir),
        Err(primary_err) => {
            let fallback_dir = bdciechi_fallback_download_dir();
            match fs::create_dir_all(&fallback_dir) {
                Ok(()) => {
                    crate::log_debug(&format!(
                        "bdciechi: documents dir unavailable ({}), using fallback {}",
                        primary_err,
                        fallback_dir.display()
                    ));
                    Ok(fallback_dir)
                }
                Err(fallback_err) => Err(format!(
                    "{primary_err}; fallback {}: {fallback_err}",
                    fallback_dir.display()
                )),
            }
        }
    }
}

fn save_as_dialog(owner: HWND, initial_dir: &Path, suggested_name: &str) -> Option<PathBuf> {
    let filter = format!(
        "{}\0*.txt\0{}\0*.*\0\0",
        tr("save_filter_text"),
        tr("save_filter_all")
    );
    let mut filter_wide: Vec<u16> = filter.encode_utf16().collect();
    if filter_wide.last().copied() != Some(0) {
        filter_wide.push(0);
    }

    let mut file_buf = [0u16; 1024];
    let mut suggested_wide: Vec<u16> = suggested_name.encode_utf16().collect();
    if suggested_wide.len() >= file_buf.len() {
        suggested_wide.truncate(file_buf.len().saturating_sub(1));
    }
    for (i, ch) in suggested_wide.iter().enumerate() {
        file_buf[i] = *ch;
    }

    let mut dir_wide: Vec<u16> = initial_dir.to_string_lossy().encode_utf16().collect();
    if dir_wide.last().copied() != Some(0) {
        dir_wide.push(0);
    }

    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: PCWSTR(filter_wide.as_ptr()),
        lpstrFile: PWSTR(file_buf.as_mut_ptr()),
        nMaxFile: file_buf.len() as u32,
        lpstrInitialDir: PCWSTR(dir_wide.as_ptr()),
        Flags: OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT | OFN_HIDEREADONLY,
        ..Default::default()
    };

    if !crate::get_save_file_name_w_safe(&mut ofn).as_bool() {
        return None;
    }
    let len = file_buf
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(file_buf.len());
    if len == 0 {
        None
    } else {
        Some(PathBuf::from(String::from_utf16_lossy(&file_buf[..len])))
    }
}

fn show_results(state: &mut BdState, indices: Vec<usize>) {
    state.visible_indices = indices;
    crate::send_message_w_safe(state.results_combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));

    let display_labels = crate::result_labels::accessible_result_labels(
        state
            .visible_indices
            .iter()
            .filter_map(|idx| state.catalog_rows.get(*idx))
            .map(ToString::to_string)
            .collect(),
    );
    for text in display_labels {
        crate::send_message_w_safe(
            state.results_combo,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(to_wide(&text).as_ptr() as isize),
        );
    }

    if !state.visible_indices.is_empty() {
        set_results_visible(state, true);
        set_result_action_buttons_visible(state, true);
        crate::send_message_w_safe(state.results_combo, CB_SETCURSEL, WPARAM(0), LPARAM(0));
        unsafe {
            SetFocus(state.results_combo);
        }
    } else {
        set_results_visible(state, false);
        set_result_action_buttons_visible(state, false);
    }
}

fn do_login_impl(hwnd: HWND, auto_login: bool) {
    with_window_state(hwnd, |state| {
        let username = read_edit_text(state.user).trim().to_string();
        let password = read_edit_text(state.pass).trim().to_string();
        if username.is_empty() || password.is_empty() {
            let message = tr("status.enter_credentials");
            set_status(state, &message);
            crate::accessibility::screen_reader_speak(&message);
            return;
        }

        let connecting = tr("status.connecting_catalog");
        let in_progress = tr("status.connecting");
        set_status(state, &connecting);
        crate::accessibility::screen_reader_speak(&connecting);
        crate::accessibility::screen_reader_speak(&in_progress);
        stop_login_announcer(state);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let running_speech = Arc::clone(&stop_flag);
        let _speech_thread = std::thread::spawn(move || {
            let mut say_connecting = true;
            while !running_speech.load(Ordering::Relaxed) {
                if say_connecting {
                    crate::accessibility::screen_reader_speak(&tr("status.connecting"));
                } else {
                    crate::accessibility::screen_reader_speak(&tr("status.please_wait"));
                }
                say_connecting = !say_connecting;
                std::thread::sleep(Duration::from_secs(2));
            }
        });
        state.login_announce_stop = Some(stop_flag);
        crate::enable_window_safe(state.user, false);
        crate::enable_window_safe(state.pass, false);
        crate::enable_window_safe(state.login, false);

        let hwnd_val = hwnd.0;
        std::thread::spawn(move || {
            let payload = match bdciechi::identify(&username, &password).and_then(|id| {
                let quota = id.quota.clone();
                bdciechi::fetch_catalog_list(&id.nprov).map(|c| (id.nprov, quota, c))
            }) {
                Ok((nprov, quota, catalog_raw)) => LoginDonePayload {
                    username,
                    password,
                    nprov,
                    quota,
                    catalog_raw,
                    auto_login,
                    error: None,
                },
                Err(err) => LoginDonePayload {
                    username,
                    password,
                    nprov: String::new(),
                    quota: None,
                    catalog_raw: String::new(),
                    auto_login,
                    error: Some(err),
                },
            };
            let payload_ptr = Box::into_raw(Box::new(payload));
            if let Err(err) = crate::post_message_w_safe(
                HWND(hwnd_val),
                WM_BDC_LOGIN_DONE,
                WPARAM(0),
                LPARAM(payload_ptr as isize),
            ) {
                crate::log_debug(&format!("bdciechi login post message failed: {}", err));
                let _unused_box = unsafe { Box::from_raw(payload_ptr) };
            }
        });
    });
}

fn do_login(hwnd: HWND) {
    do_login_impl(hwnd, false);
}

fn apply_authenticated_state(state: &mut BdState) {
    state.visible_indices.clear();
    state.authenticated = true;
    enable_logged_controls(state, true);
    set_results_visible(state, false);
    set_result_action_buttons_visible(state, false);
    set_login_controls_visible(state, false);
    set_logout_button_visible(state, true);
    set_status(
        state,
        &i18n::tr_f(
            Language::Italian,
            "excluded_from_testing.bdciechi.status.catalog_loaded",
            &[("count", &state.catalog_rows.len().to_string())],
        ),
    );
}

fn apply_logged_out_state(state: &mut BdState) {
    state.authenticated = false;
    state.username.clear();
    state.password.clear();
    state.nprov.clear();
    state.remember_credentials = false;
    state.visible_indices.clear();
    show_results(state, Vec::new());
    set_sample_text(state, "");
    set_sample_visible(state, false);
    enable_logged_controls(state, false);
    set_result_action_buttons_visible(state, false);
    set_logout_button_visible(state, false);
    set_login_controls_visible(state, true);
    crate::log_if_err!(crate::set_window_text_w_safe(state.user, PCWSTR::null()));
    crate::log_if_err!(crate::set_window_text_w_safe(state.pass, PCWSTR::null()));
    crate::log_if_err!(crate::set_window_text_w_safe(state.search, PCWSTR::null()));
    sync_bdc_session(state.parent, "", "", "", &[], false);
    persist_bdc_credentials(state.parent, false, "", "", 0);
    set_status(state, &tr("status.login_prompt"));
}

fn do_logout(hwnd: HWND) {
    let ask = crate::message_box_w_safe(
        hwnd,
        PCWSTR(to_wide(&tr("ask_logout")).as_ptr()),
        PCWSTR(to_wide(&tr("title")).as_ptr()),
        MESSAGEBOX_STYLE(MB_YESNO.0 | MB_ICONQUESTION.0),
    );
    if ask != IDYES {
        return;
    }
    with_window_state(hwnd, |state| {
        apply_logged_out_state(state);
        crate::set_focus_safe(state.user);
    });
}

fn do_search(hwnd: HWND) {
    with_window_state(hwnd, |state| {
        set_sample_visible(state, false);
        if !state.authenticated {
            set_status(state, &tr("status.login_first"));
            return;
        }
        if let Err(err) = refresh_catalog_if_needed(state) {
            set_status(
                state,
                &i18n::tr_f(
                    Language::Italian,
                    "excluded_from_testing.bdciechi.error.search",
                    &[("err", &err.to_string())],
                ),
            );
            return;
        }
        let query = read_edit_text(state.search);
        let query_terms = tokenize_search_terms(&query);
        if query_terms.is_empty() {
            set_status(state, &tr("status.enter_search"));
            return;
        }
        let mut indices = Vec::new();
        for (idx, row) in state.catalog_rows.iter().enumerate() {
            if row_matches_query(row, &query_terms) {
                indices.push(idx);
            }
        }
        if indices.is_empty() {
            let message = tr("status.no_results");
            set_status(state, &message);
            show_results(state, Vec::new());
            crate::message_box_w_safe(
                hwnd,
                PCWSTR(to_wide(&message).as_ptr()),
                PCWSTR(to_wide(&tr("title")).as_ptr()),
                Default::default(),
            );
            return;
        }
        let count = indices.len();
        show_results(state, indices);
        set_status(
            state,
            &i18n::tr_f(
                Language::Italian,
                "excluded_from_testing.bdciechi.status.results_found",
                &[("count", &count.to_string())],
            ),
        );
    });
}

fn do_latest(hwnd: HWND) {
    with_window_state(hwnd, |state| {
        set_sample_visible(state, false);
        if !state.authenticated {
            set_status(state, &tr("status.login_first"));
            return;
        }
        set_status(state, &tr("status.loading_latest"));
        if let Err(err) = refresh_catalog_if_needed(state) {
            set_status(
                state,
                &i18n::tr_f(
                    Language::Italian,
                    "excluded_from_testing.bdciechi.error.latest",
                    &[("err", &err.to_string())],
                ),
            );
            return;
        }
        match bdciechi::fetch_latest_list(&state.nprov) {
            Ok(raw) => {
                let latest_rows = bdciechi::parse_catalog_records(&raw);
                let mut indices = Vec::new();
                for row in latest_rows {
                    if let Some(idx) = state.catalog_rows.iter().position(|r| r == &row) {
                        indices.push(idx);
                    }
                }
                indices.sort_unstable();
                indices.dedup();
                if indices.is_empty() {
                    set_status(state, &tr("status.no_latest"));
                } else {
                    let count = indices.len();
                    show_results(state, indices);
                    set_status(
                        state,
                        &i18n::tr_f(
                            Language::Italian,
                            "excluded_from_testing.bdciechi.status.latest_results",
                            &[("count", &count.to_string())],
                        ),
                    );
                }
            }
            Err(err) => set_status(
                state,
                &i18n::tr_f(
                    Language::Italian,
                    "excluded_from_testing.bdciechi.error.latest",
                    &[("err", &err.to_string())],
                ),
            ),
        }
    });
}

fn download_selected(hwnd: HWND) {
    let open_after_download = with_window_state(hwnd, |state| {
        if !state.authenticated {
            set_status(state, &tr("status.login_first"));
            return None;
        }
        let sel =
            crate::send_message_w_safe(state.results_combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        if sel < 0 {
            set_status(state, &tr("status.select_result"));
            return None;
        }
        let sel_idx = sel as usize;
        if sel_idx >= state.visible_indices.len() {
            set_status(state, &tr("status.invalid_selection"));
            return None;
        }

        let selected_row = match state
            .visible_indices
            .get(sel_idx)
            .and_then(|idx| state.catalog_rows.get(*idx))
            .cloned()
        {
            Some(row) => row,
            None => {
                set_status(state, &tr("status.invalid_selection"));
                return None;
            }
        };

        let downloading = tr("status.downloading");
        set_status(state, &downloading);
        crate::accessibility::screen_reader_speak(&downloading);

        if let Err(err) = refresh_catalog_if_needed(state) {
            set_status(
                state,
                &i18n::tr_f(
                    Language::Italian,
                    "excluded_from_testing.bdciechi.error.download",
                    &[("err", &err.to_string())],
                ),
            );
            return None;
        }

        let catalog_index = match state
            .catalog_rows
            .iter()
            .position(|row| row == &selected_row)
        {
            Some(idx) => idx,
            None => {
                set_status(
                    state,
                    &i18n::tr_f(
                        Language::Italian,
                        "excluded_from_testing.bdciechi.error.download",
                        &[("err", "Opera non più disponibile nel catalogo aggiornato")],
                    ),
                );
                return None;
            }
        };

        let work = match bdciechi::download_work(
            &state.username,
            &state.password,
            &catalog_index.to_string(),
            false,
        ) {
            Ok(w) => w,
            Err(err) => {
                set_status(
                    state,
                    &i18n::tr_f(
                        Language::Italian,
                        "excluded_from_testing.bdciechi.error.download",
                        &[("err", &err.to_string())],
                    ),
                );
                return None;
            }
        };

        let mut docs_dir = match ensure_bdciechi_download_dir(state.parent) {
            Ok(path) => path,
            Err(err) => {
                set_status(
                    state,
                    &i18n::tr_f(
                        Language::Italian,
                        "excluded_from_testing.bdciechi.error.documents_dir",
                        &[("err", &err)],
                    ),
                );
                return None;
            }
        };

        let suggested = suggested_name_from_info(&work.info, &selected_row, catalog_index);
        let Some(path) = save_as_dialog(state.parent, &docs_dir, &suggested) else {
            set_status(state, &tr("status.save_cancelled"));
            return None;
        };

        if let Some(parent_dir) = path.parent() {
            docs_dir = parent_dir.to_path_buf();
            if let Err(err) = fs::create_dir_all(&docs_dir) {
                set_status(
                    state,
                    &i18n::tr_f(
                        Language::Italian,
                        "excluded_from_testing.bdciechi.error.destination_dir",
                        &[("err", &err.to_string())],
                    ),
                );
                return None;
            }
        }

        let decoded_text = decode_book_text(&work.text);
        if let Err(err) = fs::write(&path, decoded_text.as_bytes()) {
            set_status(
                state,
                &i18n::tr_f(
                    Language::Italian,
                    "excluded_from_testing.bdciechi.error.save",
                    &[("err", &err.to_string())],
                ),
            );
            return None;
        }

        set_status(
            state,
            &i18n::tr_f(
                Language::Italian,
                "excluded_from_testing.bdciechi.status.file_saved",
                &[("path", &path.display().to_string())],
            ),
        );
        let quota_message = bdciechi::parse_work_quota(&work.info).map(|quota| {
            crate::log_debug(&format!(
                "bdciechi: download quota remaining={} total={}",
                quota.remaining, quota.monthly_total
            ));
            format_quota_message(&quota)
        });

        let ask = crate::message_box_w_safe(
            state.parent,
            PCWSTR(to_wide(&tr("ask_open_saved")).as_ptr()),
            PCWSTR(to_wide(&tr("title")).as_ptr()),
            MESSAGEBOX_STYLE(MB_YESNO.0 | MB_ICONQUESTION.0),
        );
        if let Some(message) = quota_message {
            speak_bdc_message_delayed(message);
        }
        if ask == IDYES {
            Some((state.parent, path))
        } else {
            None
        }
    });

    if let Some(Some((parent, path))) = open_after_download {
        crate::editor_manager::open_document(parent, &path);
        close_bdc_window_and_focus_editor(hwnd);
    }
}

fn sample_selected(hwnd: HWND) {
    with_window_state(hwnd, |state| {
        if !state.authenticated {
            set_status(state, &tr("status.login_first"));
            return;
        }
        let sel =
            crate::send_message_w_safe(state.results_combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        if sel < 0 {
            set_status(state, &tr("status.select_result"));
            return;
        }
        let sel_idx = sel as usize;
        if sel_idx >= state.visible_indices.len() {
            set_status(state, &tr("status.invalid_selection"));
            return;
        }

        let selected_row = match state
            .visible_indices
            .get(sel_idx)
            .and_then(|idx| state.catalog_rows.get(*idx))
            .cloned()
        {
            Some(row) => row,
            None => {
                set_status(state, &tr("status.invalid_selection"));
                return;
            }
        };

        let sample_loading = tr("status.sample_loading");
        set_status(state, &sample_loading);
        crate::accessibility::screen_reader_speak(&sample_loading);

        if let Err(err) = refresh_catalog_if_needed(state) {
            set_status(
                state,
                &i18n::tr_f(
                    Language::Italian,
                    "excluded_from_testing.bdciechi.error.sample",
                    &[("err", &err.to_string())],
                ),
            );
            return;
        }

        let catalog_index = match state
            .catalog_rows
            .iter()
            .position(|row| row == &selected_row)
        {
            Some(idx) => idx,
            None => {
                set_status(
                    state,
                    &i18n::tr_f(
                        Language::Italian,
                        "excluded_from_testing.bdciechi.error.sample",
                        &[("err", "Opera non più disponibile nel catalogo aggiornato")],
                    ),
                );
                return;
            }
        };

        let work = match bdciechi::download_work(
            &state.username,
            &state.password,
            &catalog_index.to_string(),
            true,
        ) {
            Ok(w) => w,
            Err(err) => {
                set_status(
                    state,
                    &i18n::tr_f(
                        Language::Italian,
                        "excluded_from_testing.bdciechi.error.sample",
                        &[("err", &err.to_string())],
                    ),
                );
                return;
            }
        };

        let file_name = suggested_name_from_info(&work.info, &selected_row, catalog_index);
        let decoded = decode_book_text(&work.text);
        let sample: String = decoded.chars().take(8_000).collect();
        let composed = format!(
            "{} {}\r\n\r\n{}",
            tr("sample.file_label"),
            file_name,
            sample
        );
        set_sample_text(state, &composed);
        set_sample_visible(state, true);
        set_status(state, &tr("status.sample_ready"));
        crate::set_focus_safe(state.sample_edit);
    });
}

pub fn handle_navigation(hwnd: HWND, msg: &windows::Win32::UI::WindowsAndMessaging::MSG) -> bool {
    if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN {
        let key = msg.wParam.0 as u32;
        if key == VK_ESCAPE.0 as u32 {
            let mut handled = false;
            with_window_state(hwnd, |state| {
                if msg.hwnd == state.sample_edit || msg.hwnd == state.sample_close_btn {
                    close_sample_and_focus_search(state);
                    handled = true;
                }
            });
            if handled {
                return true;
            }
            close_bdc_window_and_focus_editor(hwnd);
            return true;
        }
        if key == VK_RETURN.0 as u32 {
            let mut handled = false;
            with_window_state(hwnd, |state| {
                let target = msg.hwnd;
                if target == state.user || target == state.pass || target == state.login {
                    do_login(hwnd);
                    handled = true;
                } else if target == state.search || target == state.search_btn {
                    do_search(hwnd);
                    handled = true;
                } else if target == state.latest_btn {
                    do_latest(hwnd);
                    handled = true;
                } else if target == state.results_combo || target == state.download_btn {
                    download_selected(hwnd);
                    handled = true;
                } else if target == state.logout_btn {
                    do_logout(hwnd);
                    handled = true;
                } else if target == state.sample_edit || target == state.sample_close_btn {
                    close_sample_and_focus_search(state);
                    handled = true;
                } else if target == state.close_btn {
                    close_bdc_window_and_focus_editor(hwnd);
                    handled = true;
                }
            });
            if handled {
                return true;
            }
        }
        if key == VK_TAB.0 as u32 {
            let mut handled = false;
            with_window_state(hwnd, |state| {
                if msg.hwnd == state.sample_edit {
                    crate::set_focus_safe(state.sample_close_btn);
                    handled = true;
                } else if msg.hwnd == state.sample_close_btn {
                    crate::set_focus_safe(state.sample_edit);
                    handled = true;
                }
            });
            if handled {
                return true;
            }
        }
        if key == VK_SPACE.0 as u32 {
            let mut handled = false;
            with_window_state(hwnd, |state| {
                if msg.hwnd == state.sample_close_btn {
                    close_sample_and_focus_search(state);
                    handled = true;
                }
            });
            if handled {
                return true;
            }
        }
    }
    handle_accessibility(hwnd, msg)
}

pub fn open(parent: HWND) {
    let language = app_language(parent);
    if language != Language::Italian {
        return;
    }

    unsafe {
        let existing = with_state(parent, |state| state.bdciechi_window).unwrap_or(HWND(0));
        if existing.0 != 0 {
            SetForegroundWindow(existing);
            return;
        }

        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = to_wide(BDC_CLASS_NAME);

        let wc = windows::Win32::UI::WindowsAndMessaging::WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
            ),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(bdc_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let cached_catalog_rows = load_bdciechi_catalog_cache()
            .map(|raw| bdciechi::parse_catalog_records(&raw))
            .unwrap_or_default();

        let (
            session_username,
            session_password,
            session_nprov,
            session_catalog_rows,
            session_authenticated,
            remembered,
            remembered_username,
            remembered_password,
            remembered_last_successful_login_unix,
        ) = with_state(parent, |state| {
            (
                state.bdciechi_session_username.clone(),
                state.bdciechi_session_password.clone(),
                state.bdciechi_session_nprov.clone(),
                state.bdciechi_session_catalog_rows.clone(),
                state.bdciechi_session_authenticated,
                state.settings.remember_bdciechi_credentials,
                state.settings.bdciechi_username.clone(),
                state.settings.bdciechi_password.clone(),
                state.settings.bdciechi_last_successful_login_unix,
            )
        })
        .unwrap_or_else(|| {
            (
                String::new(),
                String::new(),
                String::new(),
                Vec::new(),
                false,
                false,
                String::new(),
                String::new(),
                0,
            )
        });

        let remembered_credentials_valid = remembered
            && !bdciechi_credentials_expired(remembered_last_successful_login_unix)
            && !remembered_username.trim().is_empty()
            && !remembered_password.trim().is_empty();

        if remembered && !remembered_credentials_valid {
            persist_bdc_credentials(parent, true, "", "", 0);
        }

        let (username, password, nprov, authenticated, catalog_rows) =
            if session_authenticated && !session_catalog_rows.is_empty() {
                (
                    session_username,
                    session_password,
                    session_nprov,
                    true,
                    session_catalog_rows,
                )
            } else {
                (
                    if remembered_credentials_valid {
                        remembered_username
                    } else {
                        String::new()
                    },
                    if remembered_credentials_valid {
                        remembered_password
                    } else {
                        String::new()
                    },
                    String::new(),
                    false,
                    cached_catalog_rows,
                )
            };

        let init = Box::new(BdState {
            parent,
            user_label: HWND(0),
            pass_label: HWND(0),
            user: HWND(0),
            pass: HWND(0),
            login: HWND(0),
            search: HWND(0),
            search_btn: HWND(0),
            latest_btn: HWND(0),
            results_label: HWND(0),
            results_combo: HWND(0),
            download_btn: HWND(0),
            sample_btn: HWND(0),
            logout_btn: HWND(0),
            sample_edit: HWND(0),
            sample_close_btn: HWND(0),
            close_btn: HWND(0),
            status: HWND(0),
            username,
            password,
            nprov,
            authenticated,
            remember_credentials: remembered,
            focus_editor_on_close: false,
            catalog_rows,
            visible_indices: Vec::new(),
            login_announce_stop: None,
        });
        let init_ptr = Box::into_raw(init);

        let title_w = to_wide(&tr("title"));
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            760,
            540,
            parent,
            HMENU(0),
            hinstance,
            Some(init_ptr as *const _),
        );

        if hwnd.0 == 0 {
            let _unused_box = Box::from_raw(init_ptr);
            return;
        }

        with_state(parent, |state| state.bdciechi_window = hwnd);
    }
}

unsafe extern "system" fn bdc_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "bdc_wndproc",
            || DefWindowProcW(hwnd, msg, wparam, lparam),
            || bdc_wndproc_inner(hwnd, msg, wparam, lparam),
        )
    }
}

fn bdc_wndproc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const CREATESTRUCTW;
                let init_ptr = (*cs).lpCreateParams as *mut BdState;
                if init_ptr.is_null() {
                    return LRESULT(0);
                }
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, init_ptr as isize);

                let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);

                let user_label = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&tr("label.username")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    14,
                    90,
                    20,
                    hwnd,
                    HMENU(0),
                    hinstance,
                    None,
                );
                let user = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    104,
                    12,
                    180,
                    24,
                    hwnd,
                    HMENU(IDC_USER as isize),
                    hinstance,
                    None,
                );

                let pass_label = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&tr("label.password")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    292,
                    14,
                    70,
                    20,
                    hwnd,
                    HMENU(0),
                    hinstance,
                    None,
                );
                let pass = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WINDOW_STYLE((ES_AUTOHSCROLL | ES_PASSWORD) as u32),
                    364,
                    12,
                    180,
                    24,
                    hwnd,
                    HMENU(IDC_PASS as isize),
                    hinstance,
                    None,
                );

                let login = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&tr("button.login")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                    552,
                    11,
                    90,
                    26,
                    hwnd,
                    HMENU(IDC_LOGIN as isize),
                    hinstance,
                    None,
                );

                CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&tr("label.search")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    52,
                    60,
                    20,
                    hwnd,
                    HMENU(0),
                    hinstance,
                    None,
                );
                let search = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    76,
                    50,
                    280,
                    24,
                    hwnd,
                    HMENU(IDC_SEARCH as isize),
                    hinstance,
                    None,
                );

                let search_btn = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&tr("button.search_catalog")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    364,
                    49,
                    130,
                    26,
                    hwnd,
                    HMENU(IDC_SEARCH_BTN as isize),
                    hinstance,
                    None,
                );

                let latest_btn = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&tr("button.latest")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    500,
                    49,
                    120,
                    26,
                    hwnd,
                    HMENU(IDC_LATEST_BTN as isize),
                    hinstance,
                    None,
                );

                let results_label = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&tr("label.results")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    90,
                    70,
                    20,
                    hwnd,
                    HMENU(0),
                    hinstance,
                    None,
                );
                let results_combo = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    WC_COMBOBOXW,
                    PCWSTR::null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                    76,
                    88,
                    544,
                    260,
                    hwnd,
                    HMENU(IDC_RESULTS_COMBO as isize),
                    hinstance,
                    None,
                );

                let download_btn = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&tr("button.download_selected")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    76,
                    128,
                    150,
                    26,
                    hwnd,
                    HMENU(IDC_DOWNLOAD_BTN as isize),
                    hinstance,
                    None,
                );
                let sample_btn = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&tr("button.sample_text")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    234,
                    128,
                    130,
                    26,
                    hwnd,
                    HMENU(IDC_SAMPLE_BTN as isize),
                    hinstance,
                    None,
                );
                let logout_btn = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&tr("button.logout")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    372,
                    128,
                    130,
                    26,
                    hwnd,
                    HMENU(IDC_LOGOUT_BTN as isize),
                    hinstance,
                    None,
                );

                let close_btn = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&tr("button.close")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    530,
                    128,
                    90,
                    26,
                    hwnd,
                    HMENU(IDC_CLOSE_BTN as isize),
                    hinstance,
                    None,
                );

                let status = CreateWindowExW(
                    Default::default(),
                    w!("STATIC"),
                    PCWSTR(to_wide(&tr("status.login_prompt")).as_ptr()),
                    WS_CHILD | WS_VISIBLE,
                    12,
                    170,
                    730,
                    24,
                    hwnd,
                    HMENU(IDC_STATUS as isize),
                    hinstance,
                    None,
                );
                let sample_edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    PCWSTR::null(),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY) as u32),
                    12,
                    200,
                    730,
                    300,
                    hwnd,
                    HMENU(IDC_SAMPLE_EDIT as isize),
                    hinstance,
                    None,
                );
                let sample_close_btn = CreateWindowExW(
                    Default::default(),
                    w!("BUTTON"),
                    PCWSTR(to_wide(&tr("button.close")).as_ptr()),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    652,
                    506,
                    90,
                    26,
                    hwnd,
                    HMENU(IDC_SAMPLE_CLOSE_BTN as isize),
                    hinstance,
                    None,
                );

                (*init_ptr).user = user;
                (*init_ptr).user_label = user_label;
                (*init_ptr).pass_label = pass_label;
                (*init_ptr).pass = pass;
                (*init_ptr).login = login;
                (*init_ptr).search = search;
                (*init_ptr).search_btn = search_btn;
                (*init_ptr).latest_btn = latest_btn;
                (*init_ptr).results_label = results_label;
                (*init_ptr).results_combo = results_combo;
                (*init_ptr).download_btn = download_btn;
                (*init_ptr).sample_btn = sample_btn;
                (*init_ptr).logout_btn = logout_btn;
                (*init_ptr).sample_edit = sample_edit;
                (*init_ptr).sample_close_btn = sample_close_btn;
                (*init_ptr).close_btn = close_btn;
                (*init_ptr).status = status;

                crate::log_if_err!(crate::set_window_text_w_safe(
                    user,
                    PCWSTR(to_wide(&(*init_ptr).username).as_ptr())
                ));
                crate::log_if_err!(crate::set_window_text_w_safe(
                    pass,
                    PCWSTR(to_wide(&(*init_ptr).password).as_ptr())
                ));

                set_results_visible(&*init_ptr, false);
                set_sample_visible(&*init_ptr, false);
                set_result_action_buttons_visible(&*init_ptr, false);
                if (*init_ptr).authenticated {
                    apply_authenticated_state(&mut *init_ptr);
                    SetFocus(search);
                } else {
                    enable_logged_controls(&*init_ptr, false);
                    set_logout_button_visible(&*init_ptr, false);
                    set_login_controls_visible(&*init_ptr, true);
                    if !(*init_ptr).catalog_rows.is_empty() {
                        set_status(&*init_ptr, &tr("status.cached_catalog_loaded"));
                    }
                    if (*init_ptr).remember_credentials
                        && !(*init_ptr).username.trim().is_empty()
                        && !(*init_ptr).password.trim().is_empty()
                    {
                        do_login_impl(hwnd, true);
                    } else {
                        SetFocus(user);
                    }
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                let key = wparam.0 as u32;
                if key == VK_ESCAPE.0 as u32 {
                    let focus = GetFocus();
                    with_window_state(hwnd, |state| {
                        if focus == state.sample_edit || focus == state.sample_close_btn {
                            close_sample_and_focus_search(state);
                        } else {
                            crate::send_message_w_safe(
                                hwnd,
                                WM_COMMAND,
                                WPARAM(IDC_CLOSE_BTN),
                                LPARAM(0),
                            );
                        }
                    });
                    return LRESULT(0);
                }
                if key == VK_RETURN.0 as u32 {
                    let focus = GetFocus();
                    with_window_state(hwnd, |state| {
                        if focus == state.user || focus == state.pass || focus == state.login {
                            do_login(hwnd);
                        } else if focus == state.search || focus == state.search_btn {
                            do_search(hwnd);
                        } else if focus == state.latest_btn {
                            do_latest(hwnd);
                        } else if focus == state.results_combo || focus == state.download_btn {
                            download_selected(hwnd);
                        } else if focus == state.logout_btn {
                            do_logout(hwnd);
                        } else if focus == state.sample_edit || focus == state.sample_close_btn {
                            close_sample_and_focus_search(state);
                        } else if focus == state.close_btn {
                            crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        }
                    });
                    return LRESULT(0);
                }
                if key == VK_SPACE.0 as u32 {
                    let focus = GetFocus();
                    with_window_state(hwnd, |state| {
                        if focus == state.sample_close_btn {
                            close_sample_and_focus_search(state);
                        }
                    });
                    return LRESULT(0);
                }
                if key == VK_TAB.0 as u32 {
                    let focus = GetFocus();
                    with_window_state(hwnd, |state| {
                        if focus == state.sample_edit {
                            crate::set_focus_safe(state.sample_close_btn);
                        } else if focus == state.sample_close_btn {
                            crate::set_focus_safe(state.sample_edit);
                        }
                    });
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SETFOCUS => {
                with_window_state(hwnd, |state| {
                    focus_primary_bdc_control(state);
                });
                LRESULT(0)
            }
            WM_CLOSE => {
                close_bdc_window_and_focus_editor(hwnd);
                LRESULT(0)
            }
            WM_SYSCOMMAND => {
                if (wparam.0 & 0xFFF0) == SC_CLOSE as usize {
                    close_bdc_window_and_focus_editor(hwnd);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                match id {
                    IDC_LOGIN => {
                        do_login(hwnd);
                        LRESULT(0)
                    }
                    IDC_SEARCH_BTN => {
                        do_search(hwnd);
                        LRESULT(0)
                    }
                    IDC_LATEST_BTN => {
                        do_latest(hwnd);
                        LRESULT(0)
                    }
                    IDC_DOWNLOAD_BTN => {
                        download_selected(hwnd);
                        LRESULT(0)
                    }
                    IDC_SAMPLE_BTN => {
                        sample_selected(hwnd);
                        LRESULT(0)
                    }
                    IDC_LOGOUT_BTN => {
                        do_logout(hwnd);
                        LRESULT(0)
                    }
                    IDC_SAMPLE_CLOSE_BTN => {
                        with_window_state(hwnd, |state| close_sample_and_focus_search(state));
                        LRESULT(0)
                    }
                    IDC_CLOSE_BTN => {
                        close_bdc_window_and_focus_editor(hwnd);
                        LRESULT(0)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_BDC_LOGIN_DONE => {
                let payload_ptr = lparam.0 as *mut LoginDonePayload;
                if payload_ptr.is_null() {
                    return LRESULT(0);
                }
                let payload = Box::from_raw(payload_ptr);
                with_window_state(hwnd, |state| {
                    stop_login_announcer(state);
                    crate::enable_window_safe(state.user, true);
                    crate::enable_window_safe(state.pass, true);
                    crate::enable_window_safe(state.login, true);
                    if let Some(err) = &payload.error {
                        sync_bdc_session(state.parent, "", "", "", &[], false);
                        set_status(
                            state,
                            &i18n::tr_f(
                                Language::Italian,
                                "excluded_from_testing.bdciechi.error.login_or_catalog",
                                &[("err", err)],
                            ),
                        );
                        crate::accessibility::screen_reader_speak(&tr("status.connection_error"));
                        crate::set_focus_safe(state.user);
                        return;
                    }
                    state.username = payload.username.clone();
                    state.password = payload.password.clone();
                    state.nprov = payload.nprov.clone();
                    let cached_hash = load_bdciechi_catalog_cache()
                        .map(|raw| catalog_hash(&raw))
                        .unwrap_or_default();
                    let remote_hash = catalog_hash(&payload.catalog_raw);
                    let catalog_changed = cached_hash != remote_hash;
                    state.catalog_rows = bdciechi::parse_catalog_records(&payload.catalog_raw);
                    if catalog_changed {
                        save_bdciechi_catalog_cache(&payload.catalog_raw);
                    }
                    apply_authenticated_state(state);
                    sync_bdc_session(
                        state.parent,
                        &state.username,
                        &state.password,
                        &state.nprov,
                        &state.catalog_rows,
                        true,
                    );
                    if state.remember_credentials {
                        let now = Utc::now().timestamp();
                        persist_bdc_credentials(
                            state.parent,
                            true,
                            &state.username,
                            &state.password,
                            now,
                        );
                    } else if !payload.auto_login {
                        let ask = crate::message_box_w_safe(
                            hwnd,
                            PCWSTR(to_wide(&tr("ask_remember_credentials")).as_ptr()),
                            PCWSTR(to_wide(&tr("title")).as_ptr()),
                            MESSAGEBOX_STYLE(MB_YESNO.0 | MB_ICONQUESTION.0),
                        );
                        if ask == IDYES {
                            state.remember_credentials = true;
                            let now = Utc::now().timestamp();
                            persist_bdc_credentials(
                                state.parent,
                                true,
                                &state.username,
                                &state.password,
                                now,
                            );
                        } else {
                            persist_bdc_credentials(state.parent, false, "", "", 0);
                        }
                    }
                    set_status(
                        state,
                        &i18n::tr_f(
                            Language::Italian,
                            if catalog_changed {
                                "excluded_from_testing.bdciechi.status.catalog_updated"
                            } else {
                                "excluded_from_testing.bdciechi.status.catalog_already_current"
                            },
                            &[("count", &state.catalog_rows.len().to_string())],
                        ),
                    );
                    let login_message = if let Some(quota) = &payload.quota {
                        crate::log_debug(&format!(
                            "bdciechi: login quota remaining={} total={}",
                            quota.remaining, quota.monthly_total
                        ));
                        format!(
                            "{} {}",
                            tr("status.login_completed"),
                            format_quota_message(quota)
                        )
                    } else {
                        tr("status.login_completed")
                    };
                    speak_bdc_message_delayed(login_message);
                    SetForegroundWindow(hwnd);
                    SetFocus(state.search);
                });
                LRESULT(0)
            }
            WM_DESTROY => LRESULT(0),
            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut BdState;
                if !ptr.is_null() {
                    let mut state = Box::from_raw(ptr);
                    stop_login_announcer(&mut state);
                    with_state(state.parent, |s| s.bdciechi_window = HWND(0));
                    if state.focus_editor_on_close {
                        let parent = state.parent;
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_millis(120));
                            crate::log_if_err!(crate::post_message_w_safe(
                                parent,
                                WM_FOCUS_EDITOR,
                                WPARAM(0),
                                LPARAM(0),
                            ));
                        });
                    }
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_bdc_catalog_filename_base, normalize_search_text, repair_utf8_mojibake,
        row_matches_query, strip_known_extension, suggested_name_from_info, tokenize_search_terms,
    };

    #[test]
    fn bdciechi_search_normalizes_accents_in_query() {
        assert_eq!(tokenize_search_terms("Giosuè città"), ["giosue", "citta"]);
    }

    #[test]
    fn bdciechi_search_matches_rows_without_diacritic_input() {
        let terms = tokenize_search_terms("giosue citta");
        assert!(row_matches_query(
            "Giosuè Carducci - Le città invisibili",
            &terms
        ));
    }

    #[test]
    fn bdciechi_search_treats_comma_as_term_separator() {
        let terms = tokenize_search_terms("Giosuè Carducci, Le città invisibili");
        assert_eq!(terms, ["giosue", "carducci", "le", "citta", "invisibili"]);
        assert!(row_matches_query(
            "Giosuè Carducci - Le città invisibili",
            &terms
        ));
    }

    #[test]
    fn bdciechi_search_normalization_keeps_plain_ascii() {
        assert_eq!(
            normalize_search_text("autore titolo 123"),
            "autore titolo 123"
        );
    }

    #[test]
    fn repair_utf8_mojibake_fixes_common_cp1252_utf8_mixups() {
        assert_eq!(
            repair_utf8_mojibake("Lâ€™EMIRATO DEL POSSIBILE OpportunitÃ "),
            "L’EMIRATO DEL POSSIBILE Opportunità"
        );
    }

    #[test]
    fn repair_utf8_mojibake_fixes_apostrophe_only_case() {
        assert_eq!(
            repair_utf8_mojibake("DUBAI\nLâ€™EMIRATO DEL POSSIBILE"),
            "DUBAI\nL’EMIRATO DEL POSSIBILE"
        );
    }

    #[test]
    fn suggested_name_from_info_prefers_catalog_row_with_original_extension() {
        assert_eq!(
            suggested_name_from_info(
                "x;y;z;C:\\\\books\\\\Titolo opera.txt",
                "Autore Nome - Titolo opera",
                12
            ),
            "Autore Nome - Titolo opera.txt"
        );
    }

    #[test]
    fn strip_known_extension_removes_duplicate_suffix_only_once() {
        assert_eq!(
            strip_known_extension("Titolo opera.txt", "txt"),
            "Titolo opera"
        );
        assert_eq!(
            strip_known_extension("Titolo opera.rtf", "txt"),
            "Titolo opera.rtf"
        );
    }

    #[test]
    fn format_bdc_catalog_filename_base_replaces_underscores_with_spaced_dash() {
        assert_eq!(
            format_bdc_catalog_filename_base("Carducci Giosue_ Conversazioni Critiche"),
            "Carducci Giosue - Conversazioni Critiche"
        );
    }
}
