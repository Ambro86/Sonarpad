use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::{AtomicIsize, Ordering},
};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, EN_CHANGE,
    ES_AUTOHSCROLL, GWLP_USERDATA, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, HMENU,
    IDC_ARROW, IDYES, IsDialogMessageW, IsWindow, LoadCursorW, MB_ICONQUESTION, MB_YESNO, MSG,
    RegisterClassW, SetForegroundWindow, SetWindowLongPtrW, WINDOW_STYLE, WM_CLOSE, WM_COMMAND,
    WM_CREATE, WM_KEYDOWN, WM_NCDESTROY, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE,
    WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::PCWSTR;

use crate::app_windows::interpreter_select_window;
use crate::app_windows::interpreter_select_window::{
    GroupedSelectGroup, GroupedSelectItem, InterpreterContextAction,
    InterpreterSecondaryActionOptions, InterpreterSelectionResult,
};
use crate::settings::Language;
use crate::tools::rai_audiodescrizioni::{self, CatalogGroup, CatalogItem};
use crate::{show_error, with_state};

const AUTHOR_EMAIL: &str = "ambro86@gmail.com";
const REQUEST_FORM_CLASS: &str = "SonarpadRaiCodeRequest";
const REQUEST_ID_NAME: usize = 8201;
const REQUEST_ID_SURNAME: usize = 8202;
const REQUEST_ID_EMAIL: usize = 8203;
const REQUEST_ID_OK: usize = 8204;
const REQUEST_ID_CANCEL: usize = 8205;

static RAI_AUDIO_CONTEXT_TRANSCRIPTION_PARENT: AtomicIsize = AtomicIsize::new(0);
static RAI_AUDIO_CONTEXT_SAVE_PARENT: AtomicIsize = AtomicIsize::new(0);

pub(crate) fn mark_context_transcription_started(parent: HWND) {
    RAI_AUDIO_CONTEXT_TRANSCRIPTION_PARENT.store(parent.0, Ordering::SeqCst);
    crate::log_debug(&format!(
        "Rai audiodescrizioni context transcription started parent={:?}",
        parent
    ));
}

pub(crate) fn finish_context_transcription(parent: HWND) {
    if RAI_AUDIO_CONTEXT_TRANSCRIPTION_PARENT
        .compare_exchange(parent.0, 0, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        crate::log_debug(&format!(
            "Rai audiodescrizioni context transcription focus protection cleared parent={:?}",
            parent
        ));
    }
}

pub(crate) fn context_transcription_keeps_progress_foreground(parent: HWND) -> bool {
    RAI_AUDIO_CONTEXT_TRANSCRIPTION_PARENT.load(Ordering::SeqCst) == parent.0
}

pub(crate) fn mark_context_save_started(parent: HWND) {
    RAI_AUDIO_CONTEXT_SAVE_PARENT.store(parent.0, Ordering::SeqCst);
    crate::log_debug(&format!(
        "Rai audiodescrizioni context save started parent={:?}",
        parent
    ));
}

pub(crate) fn finish_context_save(parent: HWND) {
    if RAI_AUDIO_CONTEXT_SAVE_PARENT
        .compare_exchange(parent.0, 0, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        crate::log_debug(&format!(
            "Rai audiodescrizioni context save focus protection cleared parent={:?}",
            parent
        ));
    }
}

pub(crate) fn context_save_keeps_progress_foreground(parent: HWND) -> bool {
    RAI_AUDIO_CONTEXT_SAVE_PARENT.load(Ordering::SeqCst) == parent.0
}

struct RequestCodeState {
    parent: HWND,
    language: Language,
    name_edit: HWND,
    surname_edit: HWND,
    email_edit: HWND,
    error_label: HWND,
    ok_button: HWND,
    result: Option<(String, String, String)>,
    opened_at: Instant,
}

pub fn open(parent: HWND) {
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    if language != Language::Italian {
        return;
    }
    let initial_item_id =
        with_state(parent, |state| state.last_rai_recent_item_id.clone()).unwrap_or(None);
    open_recent_catalog(parent, language, initial_item_id);
}

pub(crate) fn ensure_rai_luce_access(parent: HWND, language: Language) -> bool {
    ensure_rai_luce_access_with_title(parent, language, None)
}

pub(crate) fn ensure_rai_luce_access_with_title(
    parent: HWND,
    language: Language,
    title_override: Option<&str>,
) -> bool {
    if crate::settings::load_saved_rai_luce_code().is_some() {
        return true;
    }
    !handle_missing_luce_key(
        parent,
        language,
        "Chiave Luce mancante: inserisci il codice nelle impostazioni RSS/Podcast.",
        title_override,
    )
}

fn context_menu_label(language: Language, key: &str) -> String {
    crate::i18n::tr(language, key)
        .replace('&', "")
        .split('\t')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn optional_media_title(title: &str) -> Option<String> {
    let title = title.trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

fn catalog_context_actions(
    parent: HWND,
    language: Language,
    items_by_key: HashMap<String, CatalogItem>,
) -> Vec<InterpreterContextAction> {
    let items = Arc::new(items_by_key);
    let enabled_for = |items: Arc<HashMap<String, CatalogItem>>| {
        Arc::new(move |selected_key: &str| {
            items
                .get(selected_key)
                .map(|item| !item.audio_url.trim().is_empty())
                .unwrap_or(false)
        }) as Arc<dyn Fn(&str) -> bool + Send + Sync>
    };

    let save_items = Arc::clone(&items);
    let save_action = InterpreterContextAction {
        label: context_menu_label(language, "playback.download_episode"),
        ctrl_c_shortcut: false,
        delete_shortcut: false,
        enabled: enabled_for(Arc::clone(&items)),
        handler: Arc::new(move |selected_key: String| {
            let Some(item) = save_items.get(&selected_key) else {
                return;
            };
            let resolved_url = match rai_audiodescrizioni::resolve_audio_url(&item.audio_url) {
                Ok(url) => url,
                Err(err) => {
                    show_error(parent, language, &err);
                    return;
                }
            };
            crate::save_rai_audio_description_context_media(
                parent,
                language,
                resolved_url,
                optional_media_title(&item.title),
            );
        }),
        children: Vec::new(),
    };

    let transcribe_items = Arc::clone(&items);
    let transcribe_action = InterpreterContextAction {
        label: context_menu_label(language, "playback.transcribe_current"),
        ctrl_c_shortcut: false,
        delete_shortcut: false,
        enabled: enabled_for(Arc::clone(&items)),
        handler: Arc::new(move |selected_key: String| {
            let Some(item) = transcribe_items.get(&selected_key) else {
                return;
            };
            let resolved_url = match rai_audiodescrizioni::resolve_audio_url(&item.audio_url) {
                Ok(url) => url,
                Err(err) => {
                    show_error(parent, language, &err);
                    return;
                }
            };
            mark_context_transcription_started(parent);
            crate::start_whisper_transcription_for_remote_media_context(
                parent,
                resolved_url,
                optional_media_title(&item.title),
            );
        }),
        children: Vec::new(),
    };

    let copy_items = Arc::clone(&items);
    let copy_action = InterpreterContextAction {
        label: format!(
            "{} (Ctrl+C)",
            crate::i18n::tr(language, "rai_audiodescrizioni.copy_audio_url")
        ),
        ctrl_c_shortcut: true,
        delete_shortcut: false,
        enabled: enabled_for(items),
        handler: Arc::new(move |selected_key: String| {
            if let Some(item) = copy_items.get(&selected_key) {
                copy_text_to_clipboard(
                    parent,
                    &format_resolved_audio_url_clipboard_text(
                        language,
                        &item.title,
                        &item.audio_url,
                    ),
                );
            }
        }),
        children: Vec::new(),
    };

    vec![save_action, transcribe_action, copy_action]
}

fn open_recent_catalog(parent: HWND, language: Language, initial_item_id: Option<String>) {
    crate::screen_reader_speak(&crate::i18n::tr(
        language,
        "rai_audiodescrizioni.loading_recent",
    ));
    let catalog = match rai_audiodescrizioni::load_catalog() {
        Ok(catalog) => catalog,
        Err(err) => {
            if handle_missing_luce_key(parent, language, &err, None) {
                return;
            }
            show_error(parent, language, &err);
            return;
        }
    };

    if catalog.items.is_empty() {
        show_error(
            parent,
            language,
            &crate::i18n::tr(language, "rai_audiodescrizioni.error.empty_recent_catalog"),
        );
        return;
    }

    crate::screen_reader_speak(&crate::i18n::tr(
        language,
        "rai_audiodescrizioni.loading_recent",
    ));
    let (display_items, labels) = build_display_items(&catalog.items, language);
    let initial_label = initial_item_id.as_deref().and_then(|item_id| {
        display_items
            .iter()
            .find(|(_, item)| item.item_id == item_id)
            .map(|(label, _)| label.clone())
    });
    let context_items = display_items
        .iter()
        .cloned()
        .collect::<HashMap<String, CatalogItem>>();
    let filter_label = crate::i18n::tr(language, "wikipedia.search_label");
    let selection = interpreter_select_window::select_interpreter_with_secondary_action_and_context_actions_and_initial_without_parent_restore(
        parent,
        labels,
        language,
        crate::i18n::tr(language, "rai_audiodescrizioni.window.recent_title"),
        InterpreterSecondaryActionOptions {
            label: crate::i18n::tr(language, "rai_audiodescrizioni.action.show_all"),
            filter_label: Some(filter_label),
        },
        initial_label,
        catalog_context_actions(parent, language, context_items),
    );

    match selection {
        Some(InterpreterSelectionResult::Item(selected_label)) => {
            let Some((_, selected_item)) = display_items
                .into_iter()
                .find(|(label, _)| label == &selected_label)
            else {
                show_error(
                    parent,
                    language,
                    &crate::i18n::tr(language, "rai_audiodescrizioni.error.open_selected"),
                );
                return;
            };
            with_state(parent, |state| {
                state.last_rai_recent_item_id = Some(selected_item.item_id.clone());
            });
            crate::enable_window_safe(parent, true);
            crate::set_foreground_window_safe(parent);
            open_item(
                parent,
                language,
                &selected_item,
                crate::RaiAudioOrigin::Recenti,
            );
        }
        Some(InterpreterSelectionResult::SecondaryAction) => {
            crate::enable_window_safe(parent, true);
            crate::set_foreground_window_safe(parent);
            open_grouped(parent);
        }
        Some(InterpreterSelectionResult::BackNavigation) | None => {}
    }
}

pub fn open_grouped(parent: HWND) {
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    if language != Language::Italian {
        return;
    }
    let initial_item_id =
        with_state(parent, |state| state.last_rai_grouped_item_id.clone()).unwrap_or(None);
    open_grouped_catalog(parent, language, initial_item_id);
}

fn open_grouped_catalog(parent: HWND, language: Language, initial_item_id: Option<String>) {
    crate::screen_reader_speak(&crate::i18n::tr(
        language,
        "rai_audiodescrizioni.loading_full_catalog",
    ));
    let groups = match rai_audiodescrizioni::load_grouped_catalog() {
        Ok(groups) => groups,
        Err(err) => {
            if handle_missing_luce_key(parent, language, &err, None) {
                return;
            }
            show_error(parent, language, &err);
            return;
        }
    };

    if groups.is_empty() {
        show_error(
            parent,
            language,
            &crate::i18n::tr(language, "rai_audiodescrizioni.error.empty_full_catalog"),
        );
        return;
    }

    match rai_audiodescrizioni::write_grouped_catalog_dump(&groups) {
        Ok(path) => crate::log_debug(&format!(
            "Rai grouped catalog dump written to {}",
            path.display()
        )),
        Err(err) => crate::log_debug(&format!(
            "Failed to write Rai grouped catalog dump: {}",
            err
        )),
    }

    let film_items = groups
        .iter()
        .find(|group| is_film_group(&group.title))
        .map(|group| group.items.clone())
        .unwrap_or_default();
    let grouped_items = build_grouped_items(&groups);
    let item_by_id: std::collections::HashMap<String, CatalogItem> = groups
        .iter()
        .flat_map(|group| group.items.iter().cloned())
        .map(|item| (item.item_id.clone(), item))
        .collect();
    let filter_label = crate::i18n::tr(language, "wikipedia.search_label");
    let selection = if film_items.is_empty() {
        interpreter_select_window::select_grouped_interpreter_with_context_actions_without_parent_restore_on_accept(
            parent,
            grouped_items,
            language,
            crate::i18n::tr(language, "rai_audiodescrizioni.window.full_title"),
            Some(filter_label),
            initial_item_id,
            catalog_context_actions(parent, language, item_by_id.clone()),
        )
        .map(InterpreterSelectionResult::Item)
    } else {
        interpreter_select_window::select_grouped_interpreter_with_secondary_action_and_context_actions_without_parent_restore_on_accept(
            parent,
            grouped_items,
            language,
            crate::i18n::tr(language, "rai_audiodescrizioni.window.full_title"),
            InterpreterSecondaryActionOptions {
                label: "Film".to_string(),
                filter_label: Some(filter_label),
            },
            initial_item_id,
            catalog_context_actions(parent, language, item_by_id.clone()),
        )
    };
    let Some(selected_value) = selection else {
        let recent_item_id =
            with_state(parent, |state| state.last_rai_recent_item_id.clone()).unwrap_or(None);
        crate::enable_window_safe(parent, true);
        crate::set_foreground_window_safe(parent);
        open_recent_catalog(parent, language, recent_item_id);
        return;
    };

    if matches!(selected_value, InterpreterSelectionResult::SecondaryAction) {
        crate::enable_window_safe(parent, true);
        crate::set_foreground_window_safe(parent);
        open_film_catalog(parent, language, &film_items);
        return;
    }

    let InterpreterSelectionResult::Item(selected_value) = selected_value else {
        return;
    };

    for group in groups {
        for item in group.items {
            if item.item_id == selected_value {
                with_state(parent, |state| {
                    state.last_rai_grouped_item_id = Some(item.item_id.clone());
                });
                crate::enable_window_safe(parent, true);
                crate::set_foreground_window_safe(parent);
                open_item(parent, language, &item, crate::RaiAudioOrigin::Tutte);
                return;
            }
        }
    }

    show_error(
        parent,
        language,
        &crate::i18n::tr(language, "rai_audiodescrizioni.error.open_selected"),
    );
}

pub(crate) fn format_audio_url_clipboard_text(
    language: Language,
    title: &str,
    audio_url: &str,
) -> String {
    let title_label = crate::i18n::tr(language, "properties.title");
    let url_label = crate::i18n::tr(language, "properties.url");
    format!(
        "{title_label}: {}\r\n{url_label}: {}",
        title.trim(),
        audio_url
    )
}

pub(crate) fn format_resolved_audio_url_clipboard_text(
    language: Language,
    title: &str,
    audio_url: &str,
) -> String {
    let resolved_audio_url = match rai_audiodescrizioni::resolve_audio_url_for_clipboard(audio_url)
    {
        Ok(url) => url,
        Err(err) => {
            crate::log_debug(&format!(
                "Rai audio URL copy fallback to original URL: {}",
                err
            ));
            audio_url.trim().to_string()
        }
    };
    format_audio_url_clipboard_text(language, title, &resolved_audio_url)
}

pub(crate) fn copy_text_to_clipboard(hwnd: HWND, text: &str) {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Memory::GMEM_MOVEABLE;

    const CF_UNICODETEXT: u32 = 13;

    let content = crate::to_wide(text);
    if content.is_empty() {
        return;
    }
    if crate::open_clipboard_safe(hwnd).is_err() {
        return;
    }
    if let Err(err) = crate::empty_clipboard_safe() {
        crate::log_debug(&format!("EmptyClipboard failed: {}", err));
    }
    let size = content.len() * std::mem::size_of::<u16>();
    let handle = match crate::global_alloc_safe(GMEM_MOVEABLE, size) {
        Ok(handle) => handle,
        Err(err) => {
            crate::log_debug(&format!("GlobalAlloc failed: {}", err));
            crate::log_if_err!(crate::close_clipboard_safe());
            return;
        }
    };
    if handle.0.is_null() {
        crate::log_if_err!(crate::close_clipboard_safe());
        return;
    }
    let ptr = crate::global_lock_as_safe(handle) as *mut u16;
    if ptr.is_null() {
        crate::log_if_err!(crate::close_clipboard_safe());
        return;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(content.as_ptr(), ptr, content.len());
    }
    crate::log_if_err!(crate::global_unlock_safe(handle));
    if let Err(err) = crate::set_clipboard_data_safe(CF_UNICODETEXT, HANDLE(handle.0 as isize)) {
        crate::log_debug(&format!("SetClipboardData failed: {}", err));
    }
    crate::log_if_err!(crate::close_clipboard_safe());
}

fn open_item(
    parent: HWND,
    language: Language,
    selected_item: &CatalogItem,
    origin: crate::RaiAudioOrigin,
) {
    let resolved_url = match rai_audiodescrizioni::resolve_audio_url(&selected_item.audio_url) {
        Ok(url) => url,
        Err(err) => {
            show_error(parent, language, &err);
            return;
        }
    };

    let title = selected_item.title.trim();
    let title = (!title.is_empty()).then_some(title);
    if let Err(err) =
        crate::launch_stream_url_in_mpv(parent, &resolved_url, title, None, None, None)
    {
        show_error(parent, language, &err);
        return;
    }
    with_state(parent, |state| {
        state.active_podcast_episode_from_rai = origin;
    });
    crate::focus_editor(parent);
}

fn build_grouped_items(groups: &[CatalogGroup]) -> Vec<GroupedSelectGroup> {
    groups
        .iter()
        .map(|group| GroupedSelectGroup {
            label: group.title.clone(),
            items: group
                .items
                .iter()
                .map(|item| GroupedSelectItem {
                    label: item.title.clone(),
                    value: item.item_id.clone(),
                })
                .collect(),
            hidden_in_tree: is_film_group(&group.title),
        })
        .collect()
}

fn is_film_group(title: &str) -> bool {
    title.trim().eq_ignore_ascii_case("Film")
}

fn open_film_catalog(parent: HWND, language: Language, items: &[CatalogItem]) {
    if items.is_empty() {
        return;
    }
    let (display_items, labels) = build_display_items(items, language);
    let context_items = display_items
        .iter()
        .cloned()
        .collect::<HashMap<String, CatalogItem>>();
    let selection =
        interpreter_select_window::select_interpreter_with_context_actions_without_parent_restore_on_accept(
            parent,
            labels,
            language,
            "Film".to_string(),
            None,
            catalog_context_actions(parent, language, context_items),
        );
    let Some(selected_label) = selection else {
        let initial_item_id =
            with_state(parent, |state| state.last_rai_grouped_item_id.clone()).unwrap_or(None);
        crate::enable_window_safe(parent, true);
        crate::set_foreground_window_safe(parent);
        crate::set_focus_safe(parent);
        open_grouped_catalog(parent, language, initial_item_id);
        return;
    };
    let Some((_, selected_item)) = display_items
        .into_iter()
        .find(|(label, _)| label == &selected_label)
    else {
        show_error(
            parent,
            language,
            &crate::i18n::tr(language, "rai_audiodescrizioni.error.open_selected"),
        );
        return;
    };
    with_state(parent, |state| {
        state.last_rai_grouped_item_id = Some(selected_item.item_id.clone());
    });
    crate::enable_window_safe(parent, true);
    crate::set_foreground_window_safe(parent);
    open_item(
        parent,
        language,
        &selected_item,
        crate::RaiAudioOrigin::Tutte,
    );
}

fn build_display_items(
    items: &[CatalogItem],
    language: Language,
) -> (Vec<(String, CatalogItem)>, Vec<String>) {
    let mut used = HashSet::new();
    let mut display_items = Vec::with_capacity(items.len());
    let mut labels = Vec::with_capacity(items.len());

    for item in items {
        let base_label = format_item_label(item, language);
        let unique_label = ensure_unique_label(base_label, item, &mut used);
        labels.push(unique_label.clone());
        display_items.push((unique_label, item.clone()));
    }

    (display_items, labels)
}

fn format_item_label(item: &CatalogItem, language: Language) -> String {
    let mut parts = Vec::new();
    let title = item.title.trim();
    if !title.is_empty() {
        parts.push(title.to_string());
    }
    let display_date = item.date.trim().to_string();
    if !display_date.is_empty() {
        parts.push(display_date);
    } else {
        let gen_date = item.gen_date.as_deref().unwrap_or("").trim();
        if !gen_date.is_empty() {
            parts.push(gen_date.to_string());
        }
    }
    let description = item.description.trim();
    if !description.is_empty() {
        parts.push(description.to_string());
    } else {
        let set_name = item.set_name.trim();
        if !set_name.is_empty() {
            parts.push(set_name.to_string());
        }
    }

    if parts.is_empty() {
        crate::i18n::tr(language, "rai_audiodescrizioni.item.default_title")
    } else {
        parts.join(" - ")
    }
}

fn ensure_unique_label(
    base_label: String,
    item: &CatalogItem,
    used: &mut HashSet<String>,
) -> String {
    if used.insert(base_label.clone()) {
        return base_label;
    }

    let date = item.date.trim();
    if !date.is_empty() {
        let dated = format!("{base_label} - {date}");
        if used.insert(dated.clone()) {
            return dated;
        }
    }

    let mut index = 2usize;
    loop {
        let candidate = format!("{base_label} ({index})");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}

fn handle_missing_luce_key(
    parent: HWND,
    language: Language,
    err: &str,
    title_override: Option<&str>,
) -> bool {
    if !rai_audiodescrizioni::is_luce_key_missing_error(err) {
        return false;
    }
    crate::log_debug(&format!("Rai Luce missing key handler invoked: {}", err));

    let ask = crate::show_blocking_modal_message_box(
        parent,
        crate::BlockingModalKind::RaiLuceMissingKey,
        PCWSTR(
            crate::to_wide(&crate::i18n::tr(
                language,
                "rai_audiodescrizioni.missing_key_message",
            ))
            .as_ptr(),
        ),
        PCWSTR(
            crate::to_wide(title_override.unwrap_or(&crate::i18n::tr(
                language,
                "rai_audiodescrizioni.missing_key_title",
            )))
            .as_ptr(),
        ),
        MB_YESNO | MB_ICONQUESTION,
    );
    if ask != IDYES {
        return true;
    }

    let Some((nome, cognome, mail)) = request_code_contact(parent, language) else {
        return true;
    };

    let nome = nome.trim();
    let cognome = cognome.trim();
    let mail = mail.trim();
    if nome.is_empty() || cognome.is_empty() || mail.is_empty() {
        return true;
    }

    let body = format!(
        "Richiesta da: {nome} {cognome}\r\nEmail: {mail}\r\nSistema operativo: Windows\r\nLingua: {}",
        app_language_label(language)
    );
    let uri = format!(
        "mailto:{AUTHOR_EMAIL}?subject={}&body={}",
        mailto_encode_component(&crate::i18n::tr(
            language,
            "rai_audiodescrizioni.request_code.subject",
        )),
        mailto_encode_component(&body)
    );
    if let Err(open_err) = crate::audio_utils::open_url_in_browser(&uri) {
        show_error(parent, language, &open_err);
    }
    true
}

fn app_language_label(language: Language) -> &'static str {
    match language {
        Language::Italian => "Italiano",
        Language::German => "Deutsch",
        Language::English => "English",
        Language::Spanish => "Español",
        Language::Portuguese | Language::PortugueseBrazilian => "Português",
        Language::Swedish => "Svenska",
        Language::Vietnamese => "Tiếng Việt",
        Language::Czech => "Čeština",
        Language::Polish => "Polski",
        Language::French => "Français",
        Language::Serbian => "Srpski",
        Language::Ukrainian => "Українська",
        Language::Lithuanian => "Lietuvių",
        Language::Russian => "Русский",
        Language::Chinese => "中文",
        Language::Hindi => "हिंदी",
    }
}

fn mailto_encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            b' ' => encoded.push_str("%20"),
            b'\r' => encoded.push_str("%0D"),
            b'\n' => encoded.push_str("%0A"),
            _ => {
                encoded.push('%');
                encoded.push_str(&format!("{byte:02X}"));
            }
        }
    }
    encoded
}

fn request_code_contact(parent: HWND, language: Language) -> Option<(String, String, String)> {
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = crate::to_wide(REQUEST_FORM_CLASS);
        let wc = WNDCLASSW {
            hCursor: windows::Win32::UI::WindowsAndMessaging::HCURSOR(
                LoadCursorW(None, IDC_ARROW).unwrap_or_default().0,
            ),
            hInstance: hinstance,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            lpfnWndProc: Some(request_code_wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let state = Box::new(RequestCodeState {
            parent,
            language,
            name_edit: HWND(0),
            surname_edit: HWND(0),
            email_edit: HWND(0),
            error_label: HWND(0),
            ok_button: HWND(0),
            result: None,
            opened_at: Instant::now(),
        });
        let state_ptr = Box::into_raw(state);
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(
                crate::to_wide(&crate::i18n::tr(
                    language,
                    "rai_audiodescrizioni.request_code.title",
                ))
                .as_ptr(),
            ),
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            420,
            270,
            parent,
            HMENU(0),
            hinstance,
            Some(state_ptr as *const _),
        );
        if hwnd.0 == 0 {
            let _unused_box = Box::from_raw(state_ptr);
            return None;
        }

        crate::enable_window_safe(parent, false);
        SetForegroundWindow(hwnd);
        let mut msg = MSG::default();
        while IsWindow(hwnd).as_bool() {
            if windows::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, HWND(0), 0, 0).0 == 0
            {
                break;
            }
            if crate::app_windows::calendar_window::handle_reminder_alert_message(&msg) {
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam.0 as u16 == VK_RETURN.0 {
                let handled_enter = request_code_state_mut(hwnd).is_some_and(|state| {
                    let focus = windows::Win32::UI::Input::KeyboardAndMouse::GetFocus();
                    if focus != state.ok_button {
                        return false;
                    }
                    windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                        hwnd,
                        WM_COMMAND,
                        WPARAM(REQUEST_ID_OK),
                        LPARAM(0),
                    );
                    true
                });
                if handled_enter {
                    continue;
                }
            }
            if crate::handle_focused_edit_shortcut(&msg) {
                continue;
            }
            if !IsDialogMessageW(hwnd, &msg).as_bool() {
                windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
            }
        }

        crate::enable_window_safe(parent, true);
        crate::set_foreground_window_safe(parent);
        let state = Box::from_raw(state_ptr);
        state.result
    }
}

unsafe extern "system" fn request_code_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = lparam.0 as *const CREATESTRUCTW;
            let state_ptr = unsafe { (*cs).lpCreateParams as *mut RequestCodeState };
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize) };
            create_request_code_controls(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wparam.0 & 0xffff;
            let notify = (wparam.0 >> 16) & 0xffff;
            if notify == EN_CHANGE as usize {
                clear_request_code_error(hwnd);
                return LRESULT(0);
            }
            match id {
                REQUEST_ID_OK => {
                    let Some((language, name_edit, surname_edit, email_edit)) =
                        request_code_state_mut(hwnd).map(|state| {
                            (
                                state.language,
                                state.name_edit,
                                state.surname_edit,
                                state.email_edit,
                            )
                        })
                    else {
                        return LRESULT(0);
                    };

                    let name = read_edit_text(name_edit);
                    let surname = read_edit_text(surname_edit);
                    let email = read_edit_text(email_edit);
                    if !name.trim().is_empty()
                        && !surname.trim().is_empty()
                        && !email.trim().is_empty()
                    {
                        if let Some(state) = request_code_state_mut(hwnd) {
                            state.result = Some((name, surname, email));
                        }
                        crate::log_if_err!(crate::destroy_window_safe(hwnd));
                        return LRESULT(0);
                    }

                    let message = crate::i18n::tr(
                        language,
                        "rai_audiodescrizioni.request_code.fill_all_fields",
                    );
                    crate::log_debug(&format!("Request code validation failed: {message}"));
                    set_request_code_error(hwnd, &message);
                    crate::screen_reader_speak(&message);

                    let focus_target = if name.trim().is_empty() {
                        name_edit
                    } else if surname.trim().is_empty() {
                        surname_edit
                    } else {
                        email_edit
                    };
                    if focus_target.0 != 0 {
                        unsafe {
                            SetFocus(focus_target);
                        }
                    }
                    LRESULT(0)
                }
                REQUEST_ID_CANCEL => {
                    crate::log_if_err!(crate::destroy_window_safe(hwnd));
                    LRESULT(0)
                }
                _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            }
        }
        WM_KEYDOWN => match wparam.0 as u16 {
            key if key == VK_RETURN.0 => {
                if request_code_state_mut(hwnd)
                    .is_some_and(|state| state.opened_at.elapsed() < Duration::from_millis(300))
                {
                    return LRESULT(0);
                }
                unsafe {
                    windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                        hwnd,
                        WM_COMMAND,
                        WPARAM(REQUEST_ID_OK),
                        LPARAM(0),
                    );
                }
                LRESULT(0)
            }
            key if key == VK_ESCAPE.0 => {
                unsafe {
                    windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                        hwnd,
                        WM_COMMAND,
                        WPARAM(REQUEST_ID_CANCEL),
                        LPARAM(0),
                    );
                }
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        },
        WM_CLOSE => {
            crate::log_if_err!(crate::destroy_window_safe(hwnd));
            LRESULT(0)
        }
        WM_NCDESTROY => {
            if let Some(state) = request_code_state_mut(hwnd) {
                crate::enable_window_safe(state.parent, true);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn create_request_code_controls(hwnd: HWND) {
    let language = request_code_state_mut(hwnd)
        .map(|state| state.language)
        .unwrap_or_default();
    unsafe {
        let hinstance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        CreateWindowExW(
            Default::default(),
            windows::core::w!("STATIC"),
            PCWSTR(
                crate::to_wide(&crate::i18n::tr(
                    language,
                    "rai_audiodescrizioni.request_code.name",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE,
            12,
            16,
            180,
            18,
            hwnd,
            HMENU(0),
            hinstance,
            None,
        );
        let name_edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            windows::core::w!("EDIT"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            12,
            34,
            380,
            24,
            hwnd,
            HMENU(REQUEST_ID_NAME as isize),
            hinstance,
            None,
        );
        CreateWindowExW(
            Default::default(),
            windows::core::w!("STATIC"),
            PCWSTR(
                crate::to_wide(&crate::i18n::tr(
                    language,
                    "rai_audiodescrizioni.request_code.surname",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE,
            12,
            68,
            180,
            18,
            hwnd,
            HMENU(0),
            hinstance,
            None,
        );
        let surname_edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            windows::core::w!("EDIT"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            12,
            86,
            380,
            24,
            hwnd,
            HMENU(REQUEST_ID_SURNAME as isize),
            hinstance,
            None,
        );
        CreateWindowExW(
            Default::default(),
            windows::core::w!("STATIC"),
            PCWSTR(
                crate::to_wide(&crate::i18n::tr(
                    language,
                    "rai_audiodescrizioni.request_code.email",
                ))
                .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE,
            12,
            120,
            180,
            18,
            hwnd,
            HMENU(0),
            hinstance,
            None,
        );
        let email_edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            windows::core::w!("EDIT"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            12,
            138,
            380,
            24,
            hwnd,
            HMENU(REQUEST_ID_EMAIL as isize),
            hinstance,
            None,
        );
        let error_label = CreateWindowExW(
            Default::default(),
            windows::core::w!("STATIC"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE,
            12,
            170,
            380,
            18,
            hwnd,
            HMENU(0),
            hinstance,
            None,
        );
        let ok_button = CreateWindowExW(
            Default::default(),
            windows::core::w!("BUTTON"),
            PCWSTR(crate::to_wide(&crate::i18n::tr(language, "common.ok")).as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
            206,
            202,
            90,
            26,
            hwnd,
            HMENU(REQUEST_ID_OK as isize),
            hinstance,
            None,
        );
        CreateWindowExW(
            Default::default(),
            windows::core::w!("BUTTON"),
            PCWSTR(crate::to_wide(&crate::i18n::tr(language, "common.cancel")).as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            302,
            202,
            90,
            26,
            hwnd,
            HMENU(REQUEST_ID_CANCEL as isize),
            hinstance,
            None,
        );

        if let Some(state) = request_code_state_mut(hwnd) {
            state.name_edit = name_edit;
            state.surname_edit = surname_edit;
            state.email_edit = email_edit;
            state.error_label = error_label;
            state.ok_button = ok_button;
        }
        SetFocus(name_edit);
    }
}

fn request_code_state_mut(hwnd: HWND) -> Option<&'static mut RequestCodeState> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RequestCodeState };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &mut *ptr })
    }
}

fn read_edit_text(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len as usize + 1];
    let written = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..written as usize])
}

fn set_request_code_error(hwnd: HWND, message: &str) {
    let Some(error_label) = request_code_state_mut(hwnd).map(|state| state.error_label) else {
        return;
    };
    if error_label.0 == 0 {
        return;
    }
    let wide = crate::to_wide(message);
    crate::log_if_err!(crate::set_window_text_w_safe(
        error_label,
        PCWSTR(wide.as_ptr())
    ));
}

fn clear_request_code_error(hwnd: HWND) {
    set_request_code_error(hwnd, "");
}
