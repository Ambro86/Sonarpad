use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use windows::Win32::Foundation::HWND;

use crate::app_windows::interpreter_select_window;
use crate::app_windows::interpreter_select_window::{
    InterpreterContextAction, InterpreterSecondaryActionOptions, InterpreterSelectionResult,
};
use crate::settings::Language;
use crate::tools::sonarpad_audiodescrizioni::{self, CatalogItem};
use crate::{show_error, with_state};

#[derive(Clone)]
enum SonarpadCatalogView {
    Recent,
    Folder { path: String, title: String },
}

#[derive(Clone)]
struct SonarpadPlayerReturnContext {
    view: SonarpadCatalogView,
    selected_label: String,
    stream_url: String,
}

static SONARPAD_PLAYER_RETURN_CONTEXT: OnceLock<Mutex<Option<SonarpadPlayerReturnContext>>> =
    OnceLock::new();

fn player_return_context() -> &'static Mutex<Option<SonarpadPlayerReturnContext>> {
    SONARPAD_PLAYER_RETURN_CONTEXT.get_or_init(|| Mutex::new(None))
}

fn remember_player_return_context(context: SonarpadPlayerReturnContext) {
    if let Ok(mut stored) = player_return_context().lock() {
        *stored = Some(context);
    }
}

fn clear_player_return_context() {
    if let Ok(mut stored) = player_return_context().lock() {
        *stored = None;
    }
}

pub(crate) fn restore_after_player_stop(parent: HWND, stopped_url: Option<&str>) -> bool {
    let Some(stopped_url) = stopped_url else {
        return false;
    };
    let context = player_return_context()
        .lock()
        .ok()
        .and_then(|stored| stored.clone())
        .filter(|context| context.stream_url == stopped_url);
    let Some(context) = context else {
        return false;
    };

    clear_player_return_context();
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    crate::enable_window_safe(parent, true);
    crate::set_foreground_window_safe(parent);
    crate::log_debug(&format!(
        "Sonarpad audiodescrizioni: restoring list after mpv stop view={} selected={}",
        match &context.view {
            SonarpadCatalogView::Recent => "recent",
            SonarpadCatalogView::Folder { .. } => "folder",
        },
        context.selected_label
    ));
    match context.view {
        SonarpadCatalogView::Recent => {
            open_recent_catalog(parent, language, Some(context.selected_label));
        }
        SonarpadCatalogView::Folder { path, title } => {
            open_folder_catalog(parent, language, path, title, Some(context.selected_label));
        }
    }
    true
}

pub fn open(parent: HWND) {
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    if language != Language::Italian {
        return;
    }
    if crate::settings::load_saved_rai_luce_code().is_none() {
        return;
    }
    open_recent_catalog(parent, language, None);
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
                .map(|item| {
                    !item.download_url.trim().is_empty()
                        || item
                            .stream_url
                            .as_deref()
                            .is_some_and(|url| !url.trim().is_empty())
                })
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
            let download_url = if !item.download_url.trim().is_empty() {
                item.download_url.trim().to_string()
            } else {
                item.stream_url
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            };
            if download_url.is_empty() {
                show_error(
                    parent,
                    language,
                    "Il contenuto selezionato non ha un URL di download disponibile.",
                );
                return;
            }
            crate::save_remote_media_url_direct(
                parent,
                language,
                download_url,
                item.suggested_download_name(),
            );
        }),
        children: Vec::new(),
    };

    let copy_items = Arc::clone(&items);
    let copy_action = InterpreterContextAction {
        label: "Copia URL audio (Ctrl+C)".to_string(),
        ctrl_c_shortcut: true,
        delete_shortcut: false,
        enabled: enabled_for(items),
        handler: Arc::new(move |selected_key: String| {
            let Some(item) = copy_items.get(&selected_key) else {
                return;
            };
            let url = item
                .stream_url
                .as_deref()
                .filter(|url| !url.trim().is_empty())
                .unwrap_or(item.download_url.as_str());
            if !url.trim().is_empty() {
                crate::app_windows::rai_audiodescrizioni_window::copy_text_to_clipboard(
                    parent, url,
                );
            }
        }),
        children: Vec::new(),
    };

    vec![save_action, copy_action]
}

fn open_recent_catalog(parent: HWND, language: Language, initial_label: Option<String>) {
    crate::screen_reader_speak("Caricamento audiodescrizioni Sonarpad");
    let items = match sonarpad_audiodescrizioni::load_recent_catalog() {
        Ok(items) => items,
        Err(err) => {
            show_error(parent, language, &err);
            return;
        }
    };
    if items.is_empty() {
        show_error(
            parent,
            language,
            "Nessuna audiodescrizione Sonarpad disponibile.",
        );
        return;
    }

    let (display_items, labels) = build_display_items(&items, true);
    let context_items = display_items
        .iter()
        .cloned()
        .collect::<HashMap<String, CatalogItem>>();
    let selection = interpreter_select_window::select_interpreter_with_secondary_action_and_context_actions_and_initial_without_parent_restore_and_right_navigation(
        parent,
        labels,
        language,
        "Audiodescrizioni Sonarpad".to_string(),
        InterpreterSecondaryActionOptions {
            label: "Tutte le audiodescrizioni Sonarpad".to_string(),
            filter_label: Some(crate::i18n::tr(language, "wikipedia.search_label")),
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
                    "Impossibile aprire l'audiodescrizione selezionata.",
                );
                return;
            };
            crate::enable_window_safe(parent, true);
            crate::set_foreground_window_safe(parent);
            open_item(
                parent,
                language,
                &selected_item,
                SonarpadCatalogView::Recent,
                selected_label,
            );
        }
        Some(InterpreterSelectionResult::SecondaryAction) => {
            crate::enable_window_safe(parent, true);
            crate::set_foreground_window_safe(parent);
            open_all_catalog(parent, language, None);
        }
        Some(InterpreterSelectionResult::BackNavigation) | None => {
            crate::enable_window_safe(parent, true);
            crate::set_foreground_window_safe(parent);
            crate::focus_editor(parent);
        }
    }
}

fn open_all_catalog(parent: HWND, language: Language, initial_label: Option<String>) {
    open_folder_catalog(
        parent,
        language,
        String::new(),
        "Tutte le audiodescrizioni Sonarpad".to_string(),
        initial_label,
    );
}

fn open_folder_catalog(
    parent: HWND,
    language: Language,
    start_path: String,
    start_title: String,
    initial_label: Option<String>,
) {
    let mut current_path = start_path.trim_matches('/').to_string();
    let mut current_title = if start_title.trim().is_empty() {
        folder_title_from_path(&current_path)
    } else {
        start_title
    };
    let mut initial_label = initial_label;
    let mut initial_item_path: Option<String> = None;

    loop {
        crate::screen_reader_speak(if current_path.is_empty() {
            "Caricamento catalogo completo audiodescrizioni Sonarpad"
        } else {
            "Caricamento cartella audiodescrizioni Sonarpad"
        });

        let items = match sonarpad_audiodescrizioni::load_folder_catalog(&current_path) {
            Ok(items) => items,
            Err(err) => {
                show_error(parent, language, &err);
                return;
            }
        };
        if items.is_empty() {
            show_error(
                parent,
                language,
                "Nessun contenuto disponibile in questa cartella.",
            );
            return;
        }

        let (display_items, labels) = build_display_items(&items, false);
        let context_items = display_items
            .iter()
            .cloned()
            .collect::<HashMap<String, CatalogItem>>();

        let selector_initial = initial_item_path
            .take()
            .and_then(|path| label_for_path(&display_items, &path))
            .or_else(|| initial_label.take());

        let selection =
            interpreter_select_window::select_interpreter_with_context_actions_and_back_navigation_without_parent_restore_on_accept(
                parent,
                labels,
                language,
                current_title.clone(),
                selector_initial,
                catalog_context_actions(parent, language, context_items),
            );

        let selected_label = match selection {
            Some(InterpreterSelectionResult::Item(selected_label)) => selected_label,
            Some(InterpreterSelectionResult::BackNavigation) => {
                crate::enable_window_safe(parent, true);
                crate::set_foreground_window_safe(parent);
                if current_path.is_empty() {
                    open_recent_catalog(parent, language, None);
                    return;
                }

                let child_path = current_path.clone();
                current_path = parent_folder_path(&current_path);
                current_title = if current_path.is_empty() {
                    "Tutte le audiodescrizioni Sonarpad".to_string()
                } else {
                    folder_title_from_path(&current_path)
                };
                initial_item_path = Some(child_path);
                continue;
            }
            Some(InterpreterSelectionResult::SecondaryAction) | None => {
                crate::enable_window_safe(parent, true);
                crate::set_foreground_window_safe(parent);
                crate::focus_editor(parent);
                return;
            }
        };

        let Some((_, selected_item)) = display_items
            .into_iter()
            .find(|(label, _)| label == &selected_label)
        else {
            show_error(
                parent,
                language,
                "Impossibile aprire l'elemento selezionato.",
            );
            return;
        };

        crate::enable_window_safe(parent, true);
        crate::set_foreground_window_safe(parent);

        if selected_item.is_folder() {
            current_path = selected_item.path.trim_matches('/').to_string();
            current_title = if selected_item.title.trim().is_empty() {
                folder_title_from_path(&current_path)
            } else {
                selected_item.title.trim().to_string()
            };
            initial_label = None;
            initial_item_path = None;
            continue;
        }

        open_item(
            parent,
            language,
            &selected_item,
            SonarpadCatalogView::Folder {
                path: current_path.clone(),
                title: current_title.clone(),
            },
            selected_label,
        );
        return;
    }
}

fn parent_folder_path(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    trimmed
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_default()
}

fn folder_title_from_path(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    trimmed
        .rsplit('/')
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Tutte le audiodescrizioni Sonarpad")
        .to_string()
}

fn label_for_path(display_items: &[(String, CatalogItem)], path: &str) -> Option<String> {
    display_items
        .iter()
        .find(|(_, item)| item.path.trim_matches('/') == path.trim_matches('/'))
        .map(|(label, _)| label.clone())
}

fn open_item(
    parent: HWND,
    language: Language,
    item: &CatalogItem,
    view: SonarpadCatalogView,
    selected_label: String,
) {
    let Some(stream_url) = item
        .stream_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    else {
        show_error(
            parent,
            language,
            "Il contenuto selezionato non ha un URL di riproduzione disponibile.",
        );
        return;
    };
    let title = item.title.trim();
    let title = (!title.is_empty()).then_some(title);
    remember_player_return_context(SonarpadPlayerReturnContext {
        view,
        selected_label,
        stream_url: stream_url.to_string(),
    });
    if let Err(err) = crate::launch_stream_url_in_mpv(parent, stream_url, title, None, None, None) {
        clear_player_return_context();
        show_error(parent, language, &err);
        return;
    }
    crate::focus_editor(parent);
}

fn build_display_items(
    items: &[CatalogItem],
    show_date: bool,
) -> (Vec<(String, CatalogItem)>, Vec<String>) {
    let mut used = HashSet::new();
    let mut display_items = Vec::with_capacity(items.len());
    let mut labels = Vec::with_capacity(items.len());

    for item in items {
        let base_label = format_item_label(item, show_date);
        let unique_label = ensure_unique_label(base_label, item, &mut used);
        labels.push(unique_label.clone());
        display_items.push((unique_label, item.clone()));
    }

    (display_items, labels)
}

fn format_item_label(item: &CatalogItem, show_date: bool) -> String {
    let title = item.title.trim();
    let mut parts = vec![if title.is_empty() {
        "Audiodescrizione Sonarpad".to_string()
    } else {
        title.to_string()
    }];
    if show_date && let Some(date) = display_date(item) {
        parts.push(date);
    }
    parts.join(" - ")
}

fn display_date(item: &CatalogItem) -> Option<String> {
    let raw = item.modified_at.as_deref()?.trim();
    let date = raw.split('T').next().unwrap_or(raw);
    let mut parts = date.split('-');
    let year = parts.next()?;
    let month = parts.next()?;
    let day = parts.next()?;
    if year.len() == 4 && month.len() == 2 && day.len() == 2 {
        Some(format!("{day}/{month}/{year}"))
    } else {
        None
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

    let parent = item.parent.trim();
    if !parent.is_empty() {
        let candidate = format!("{base_label} - {parent}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }

    let stable_key = item.stable_key();
    for suffix in 2.. {
        let candidate = format!("{base_label} ({suffix})");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        if suffix > 1000 {
            return format!("{base_label} - {stable_key}");
        }
    }
    unreachable!()
}
