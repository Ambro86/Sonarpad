use windows::Win32::Foundation::HWND;

use crate::app_windows::prompt_window;
use crate::app_windows::youtube_transcript_window::{
    self, MultilineSelectionItem, MultilineSelectionResult, choose_combo_option_dialog,
};
use crate::editor_manager;
use crate::settings::Language;
use crate::tools::italiaonline::{
    self, DetailResponse, DirectoryKind, SearchOutcome, SearchQuery, SearchResponse,
};
use crate::{show_error, with_state};

const ITALIAONLINE_PREVIOUS_PAGE_ID: &str = "__italiaonline_previous_page__";
const ITALIAONLINE_NEXT_PAGE_ID: &str = "__italiaonline_next_page__";

pub fn open(parent: HWND) {
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();

    let initial_query = SearchQuery {
        kind: DirectoryKind::PagineGialle,
        what: String::new(),
        where_: String::new(),
        page: 1,
        reverse: false,
        country: "it".into(),
        language: "it".into(),
    };
    run_search_flow(parent, language, initial_query);
}

pub fn reopen_last(parent: HWND) {
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    let Some((query, selected_id)) = with_state(parent, |state| {
        Some((
            state.last_italiaonline_query.clone()?,
            state.last_italiaonline_result_id.clone(),
        ))
    })
    .flatten() else {
        open(parent);
        return;
    };
    crate::screen_reader_speak(&format!(
        "Ricerca {} in corso",
        if query.reverse {
            "numero di telefono"
        } else {
            query.kind.label()
        }
    ));
    let resolved = match resolve_search_outcome(parent, language, query.clone()) {
        Ok(Some(resolved)) => resolved,
        Ok(None) => return,
        Err(err) => {
            show_error(parent, language, &err);
            return;
        }
    };
    if resolved.response.results.is_empty() {
        show_error(parent, language, "Nessun risultato trovato.");
        return;
    }
    match browse_results(
        parent,
        language,
        &resolved.query,
        &resolved.response,
        selected_id,
    ) {
        BrowseResultsOutcome::OpenedDocument => {}
        BrowseResultsOutcome::NewSearch => {
            editor_manager::mark_current_document_from_italiaonline(parent, false);
            run_search_flow(parent, language, resolved.query);
        }
        BrowseResultsOutcome::Closed => {
            editor_manager::mark_current_document_from_italiaonline(parent, false);
        }
    }
}

fn run_search_flow(parent: HWND, language: Language, mut initial_query: SearchQuery) {
    loop {
        let Some(query) = prompt_search_query(parent, language, &initial_query) else {
            return;
        };
        initial_query = query.clone();
        crate::screen_reader_speak(&format!(
            "Ricerca {} in corso",
            if query.reverse {
                "numero di telefono"
            } else {
                query.kind.label()
            }
        ));
        let resolved = match resolve_search_outcome(parent, language, query.clone()) {
            Ok(Some(resolved)) => resolved,
            Ok(None) => continue,
            Err(err) => {
                show_error(parent, language, &err);
                continue;
            }
        };
        if resolved.response.results.is_empty() {
            show_error(parent, language, "Nessun risultato trovato.");
            continue;
        }
        match browse_results(parent, language, &resolved.query, &resolved.response, None) {
            BrowseResultsOutcome::OpenedDocument => return,
            BrowseResultsOutcome::NewSearch => continue,
            BrowseResultsOutcome::Closed => return,
        }
    }
}

enum BrowseResultsOutcome {
    Closed,
    OpenedDocument,
    NewSearch,
}

struct ResolvedSearch {
    query: SearchQuery,
    response: SearchResponse,
}

fn prompt_search_query(
    parent: HWND,
    language: Language,
    initial: &SearchQuery,
) -> Option<SearchQuery> {
    if language != Language::Italian {
        return prompt_search_query_with_focus(
            parent,
            language,
            &SearchQuery {
                kind: DirectoryKind::PagineGialle,
                reverse: false,
                page: 1,
                ..initial.clone()
            },
            false,
        );
    }
    let selection = choose_combo_option_dialog(
        parent,
        language,
        "Cerca luoghi e attività".into(),
        "Tipo di ricerca".into(),
        vec![
            "Luoghi e attività".into(),
            "Ricerca nominativi".into(),
            "Cerca un numero di telefono".into(),
        ],
        if initial.reverse {
            2
        } else if initial.kind == DirectoryKind::PagineGialle {
            0
        } else {
            1
        },
    )?;
    if selection == 2 {
        let mut value = if initial.reverse {
            initial.what.clone()
        } else {
            String::new()
        };
        loop {
            value = prompt_window::prompt_user(
                parent,
                "Cerca un numero di telefono",
                "Inserisci il numero di telefono",
                &value,
                language,
            )?;
            match italiaonline::normalize_phone(&value) {
                Ok(what) => {
                    return Some(SearchQuery {
                        kind: DirectoryKind::PagineBianche,
                        what,
                        where_: String::new(),
                        page: 1,
                        reverse: true,
                        country: "it".into(),
                        language: "it".into(),
                    });
                }
                Err(error) => show_error(parent, language, &error),
            }
        }
    }
    let initial = SearchQuery {
        kind: if selection == 0 {
            DirectoryKind::PagineGialle
        } else {
            DirectoryKind::PagineBianche
        },
        reverse: false,
        ..initial.clone()
    };
    prompt_search_query_with_focus(parent, language, &initial, false)
}

fn prompt_search_query_with_focus(
    parent: HWND,
    language: Language,
    initial: &SearchQuery,
    focus_primary_field: bool,
) -> Option<SearchQuery> {
    let (initial_city, initial_address) = split_where_fields(&initial.where_);
    let countries = crate::app_windows::podcasts_window::podcast_directory_country_options();
    let saved = with_state(parent, |s| s.settings.places_country.clone()).unwrap_or_default();
    let country = crate::app_windows::route_window::normalize_route_country(&saved)
        .unwrap_or_else(|| crate::app_windows::route_window::default_route_country(language));
    let country_index = countries
        .iter()
        .position(|(code, _)| *code == country)
        .unwrap_or(0);
    let prompt = prompt_window::prompt_directory_search(
        parent,
        prompt_window::PromptDirectoryOptions {
            title: if language == Language::Italian {
                initial.kind.label().to_string()
            } else {
                crate::i18n::tr(language, "places.title")
            },
            type_label: String::new(),
            options: Vec::new(),
            default_selection: 0,
            secondary_type_label: crate::i18n::tr(language, "route.country_label"),
            secondary_options: if initial.kind == DirectoryKind::PagineGialle {
                countries
                    .iter()
                    .map(|(code, name)| {
                        crate::app_windows::route_window::route_country_label(language, code, name)
                    })
                    .collect()
            } else {
                Vec::new()
            },
            secondary_default_selection: country_index,
            tertiary_type_label: String::new(),
            tertiary_options: Vec::new(),
            tertiary_default_selection: 0,
            tertiary_options_primary_index_only: None,
            quaternary_type_label: String::new(),
            quaternary_options: Vec::new(),
            quaternary_default_selection: 0,
            focus_primary_field,
            primary_label: if language == Language::Italian {
                initial.kind.primary_field_label().to_string()
            } else {
                crate::i18n::tr(language, "places.query")
            },
            primary_labels: Vec::new(),
            primary_default: initial.what.clone(),
            secondary_label: crate::i18n::tr(language, "places.city"),
            secondary_default: initial_city,
            tertiary_label: crate::i18n::tr(language, "places.address"),
            tertiary_default: initial_address,
            checkbox_label: String::new(),
            checkbox_default: false,
        },
        language,
    )?;
    let kind = initial.kind;
    let country = countries
        .get(prompt.secondary_selected_index)
        .map(|(code, _)| *code)
        .unwrap_or(country)
        .to_string();
    if kind == DirectoryKind::PagineGialle
        && let Some(settings) = with_state(parent, |state| {
            state.settings.places_country = country.clone();
            state.settings.clone()
        })
    {
        crate::settings::save_settings(settings);
    }
    let trimmed_what = prompt.primary_value.trim();
    if trimmed_what.is_empty() {
        show_error(
            parent,
            language,
            &format!("Il campo {} è vuoto.", kind.primary_field_name()),
        );
        return prompt_search_query_with_focus(
            parent,
            language,
            &SearchQuery {
                kind,
                what: prompt.primary_value,
                where_: compose_where_value(&prompt.secondary_value, &prompt.tertiary_value),
                page: 1,
                reverse: false,
                country: country.clone(),
                language: crate::audio_description::language_code(language).into(),
            },
            true,
        );
    }

    let what = trimmed_what.to_string();
    Some(SearchQuery {
        kind,
        what,
        reverse: false,
        where_: compose_where_value(&prompt.secondary_value, &prompt.tertiary_value),
        page: 1,
        country: country.clone(),
        language: crate::audio_description::language_code(language).into(),
    })
}

fn split_where_fields(value: &str) -> (String, String) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return (String::new(), String::new());
    }
    if let Some((city, address)) = trimmed.split_once(',') {
        return (city.trim().to_string(), address.trim().to_string());
    }
    (trimmed.to_string(), String::new())
}

fn compose_where_value(city: &str, address: &str) -> String {
    let trimmed_city = city.trim();
    let trimmed_address = address.trim();
    match (trimmed_city.is_empty(), trimmed_address.is_empty()) {
        (true, true) => String::new(),
        (false, true) => trimmed_city.to_string(),
        (true, false) => trimmed_address.to_string(),
        (false, false) => format!("{trimmed_city}, {trimmed_address}"),
    }
}

fn resolve_search_outcome(
    parent: HWND,
    language: Language,
    mut query: SearchQuery,
) -> Result<Option<ResolvedSearch>, String> {
    loop {
        let outcome = if language != Language::Italian {
            query.kind = DirectoryKind::PagineGialle;
            query.reverse = false;
            italiaonline::search_places_only(&query)?
        } else if query.reverse {
            italiaonline::search_phone(&query)?
        } else if query.kind == DirectoryKind::PagineGialle {
            italiaonline::search_activities(&query)?
        } else {
            italiaonline::search(&query)?
        };
        match outcome {
            SearchOutcome::Results(response) => {
                return Ok(Some(ResolvedSearch { query, response }));
            }
            SearchOutcome::AmbiguousAddress(ambiguous) => {
                let places =
                    if ambiguous.places.is_empty() && query.kind == DirectoryKind::PagineBianche {
                        match italiaonline::search(&SearchQuery {
                            kind: DirectoryKind::PagineGialle,
                            ..query.clone()
                        })? {
                            SearchOutcome::AmbiguousAddress(fallback) => fallback.places,
                            SearchOutcome::Results(_) => Vec::new(),
                        }
                    } else {
                        ambiguous.places
                    };
                if places.is_empty() {
                    return Err(format!(
                        "Ricerca {} non riuscita: indirizzo ambiguo. Specifica meglio la via o il numero civico.",
                        query.kind.label()
                    ));
                }
                let Some(selected_index) = choose_combo_option_dialog(
                    parent,
                    language,
                    "Seleziona via".to_string(),
                    "Sono state trovate più vie. Seleziona quella corretta".to_string(),
                    places.clone(),
                    0,
                ) else {
                    return Ok(None);
                };
                let Some(selected_place) = places.get(selected_index) else {
                    return Ok(None);
                };
                let (city, _) = split_where_fields(&query.where_);
                query.where_ = compose_where_value(&city, selected_place);
            }
        }
    }
}

fn browse_results(
    parent: HWND,
    language: Language,
    query: &SearchQuery,
    response: &SearchResponse,
    initial_selected_id: Option<String>,
) -> BrowseResultsOutcome {
    let mut current_query = query.clone();
    let mut current_response = response.clone();
    let mut current_selected_id = initial_selected_id;

    loop {
        let title = results_title(language, &current_query, &current_response);
        let mut items = Vec::new();
        if current_response.current_page > 1 {
            items.push(MultilineSelectionItem {
                id: ITALIAONLINE_PREVIOUS_PAGE_ID.to_string(),
                title: "Risultati precedenti".to_string(),
                description: Some(format!(
                    "Torna alla pagina {} dei risultati",
                    current_response.current_page.saturating_sub(1)
                )),
            });
        }
        items.extend(
            current_response
                .results
                .iter()
                .map(|result| MultilineSelectionItem {
                    id: result.id.clone(),
                    title: result.name.clone(),
                    description: Some(format_result_description(result)),
                }),
        );
        if !current_response.is_last_page {
            items.push(MultilineSelectionItem {
                id: ITALIAONLINE_NEXT_PAGE_ID.to_string(),
                title: "Risultati successivi".to_string(),
                description: Some(format!(
                    "Vai alla pagina {} dei risultati",
                    current_response.current_page + 1
                )),
            });
        }

        match youtube_transcript_window::select_multiline_items_with_search(
            parent,
            language,
            title,
            items,
            current_selected_id.clone(),
            youtube_transcript_window::MultilineSearchOptions {
                initial_query: String::new(),
                search_button_label: "Nuova ricerca".to_string(),
                show_search_edit: false,
                secondary_action_label: Some("Apri tutto".to_string()),
                context_actions: Vec::new(),
                right_arrow_accepts_selection: true,
                left_arrow_closes: false,
                escape_stops_active_player: false,
                refresh: None,
            },
        ) {
            MultilineSelectionResult::Selected(id) => {
                if id == ITALIAONLINE_PREVIOUS_PAGE_ID || id == ITALIAONLINE_NEXT_PAGE_ID {
                    let next_page = if id == ITALIAONLINE_PREVIOUS_PAGE_ID {
                        current_query.page.saturating_sub(1).max(1)
                    } else {
                        current_query.page + 1
                    };
                    crate::screen_reader_speak("Caricamento risultati");
                    let next_query = SearchQuery {
                        page: next_page,
                        ..current_query.clone()
                    };
                    match resolve_search_outcome(parent, language, next_query.clone()) {
                        Ok(Some(resolved)) => {
                            current_query = resolved.query;
                            current_response = resolved.response;
                            current_selected_id = None;
                            continue;
                        }
                        Ok(None) => continue,
                        Err(err) => {
                            show_error(parent, language, &err);
                            continue;
                        }
                    }
                }
                crate::screen_reader_speak("Caricamento dettaglio");
                match current_response
                    .results
                    .iter()
                    .find(|result| result.id == id)
                    .ok_or_else(|| "Risultato non più disponibile.".to_string())
                    .and_then(|result| italiaonline::load_result_detail(&current_query, result))
                {
                    Ok(detail) => {
                        with_state(parent, |state| {
                            state.last_italiaonline_query = Some(current_query.clone());
                            state.last_italiaonline_result_id = Some(id.clone());
                        });
                        open_detail_document(parent, detail);
                        return BrowseResultsOutcome::OpenedDocument;
                    }
                    Err(err) => {
                        show_error(parent, language, &err);
                        continue;
                    }
                }
            }
            MultilineSelectionResult::SecondaryAction => {
                crate::screen_reader_speak("Caricamento dettagli");
                match open_results_document(parent, &current_query, &current_response) {
                    Ok(()) => {
                        with_state(parent, |state| {
                            state.last_italiaonline_query = Some(current_query.clone());
                            state.last_italiaonline_result_id = None;
                        });
                        return BrowseResultsOutcome::OpenedDocument;
                    }
                    Err(err) => {
                        show_error(parent, language, &err);
                        continue;
                    }
                }
            }
            MultilineSelectionResult::Cancelled => return BrowseResultsOutcome::Closed,
            MultilineSelectionResult::Search(_) => return BrowseResultsOutcome::NewSearch,
        }
    }
}

fn results_title(language: Language, query: &SearchQuery, response: &SearchResponse) -> String {
    let label = if language != Language::Italian {
        crate::i18n::tr(language, "places.title")
    } else if query.reverse {
        "Numero di telefono".into()
    } else {
        query.kind.label().into()
    };
    let mut title = format!("{} - {}", label, query.what.trim());
    let where_display = response
        .display_where
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let raw = query.where_.trim();
            if raw.is_empty() { None } else { Some(raw) }
        });
    if let Some(where_display) = where_display {
        title.push_str(" - ");
        title.push_str(where_display);
    }
    if response.current_page > 1 {
        title.push_str(&format!(" - Pagina {}", response.current_page));
    }
    title
}

fn format_result_description(result: &italiaonline::SearchResult) -> String {
    let mut parts = Vec::new();
    if let Some(category) = result
        .category
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(category.to_string());
    }
    if let Some(address) = result
        .address
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(address.to_string());
    }
    let locality = match (
        result
            .city
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        result
            .province
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    ) {
        (Some(city), Some(province)) => Some(format!("{city} ({province})")),
        (Some(city), None) => Some(city.to_string()),
        (None, Some(province)) => Some(province.to_string()),
        (None, None) => None,
    };
    if let Some(locality) = locality {
        parts.push(locality);
    }
    if !result.phones.is_empty() {
        parts.push(result.phones.join(" - "));
    }
    parts.join(" - ")
}

fn open_detail_document(parent: HWND, detail: DetailResponse) {
    editor_manager::new_document(parent);
    editor_manager::set_current_document_title(parent, &detail.title);
    editor_manager::mark_current_document_from_italiaonline(parent, true);
    if with_state(parent, |state| {
        state
            .docs
            .get(state.current)
            .map(|doc| editor_manager::set_edit_text(doc.hwnd_edit, &detail.body))
    })
    .flatten()
    .is_none()
    {
        crate::log_debug("Failed to populate Italiaonline detail document");
    }
}

fn open_results_document(
    parent: HWND,
    query: &SearchQuery,
    response: &SearchResponse,
) -> Result<(), String> {
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    let title = results_title(language, query, response);
    let mut entries = Vec::new();
    for result in &response.results {
        let detail = italiaonline::load_result_detail(query, result).map_err(|err| {
            format!(
                "Impossibile caricare il dettaglio di {}: {err}",
                result.name
            )
        })?;
        entries.push(detail.body);
    }
    let body = entries.join("\r\n\r\n");

    editor_manager::new_document(parent);
    editor_manager::set_current_document_title(parent, &title);
    editor_manager::mark_current_document_from_italiaonline(parent, true);
    if with_state(parent, |state| {
        state
            .docs
            .get(state.current)
            .map(|doc| editor_manager::set_edit_text(doc.hwnd_edit, &body))
    })
    .flatten()
    .is_none()
    {
        crate::log_debug("Failed to populate Italiaonline results document");
        return Err("Impossibile creare il documento con tutti i risultati.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod reverse_live_tests {
    use super::*;
    #[test]
    #[ignore = "Public directory network check, run explicitly"]
    fn reverse_public_institution_lookup() {
        for kind in [DirectoryKind::PagineBianche, DirectoryKind::PagineGialle] {
            let query = SearchQuery {
                kind,
                what: "+39 02 0202".into(),
                where_: String::new(),
                page: 1,
                reverse: true,
                country: "it".into(),
                language: "it".into(),
            };
            let resolved = resolve_search_outcome(HWND(0), Language::Italian, query)
                .unwrap()
                .unwrap();
            assert!(!resolved.response.results.is_empty());
            assert!(resolved.response.results.iter().all(|r| {
                r.phones
                    .iter()
                    .any(|p| italiaonline::normalize_phone(p).ok().as_deref() == Some("020202"))
            }));
        }
    }
}
