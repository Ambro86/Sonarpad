use windows::Win32::Foundation::HWND;

use crate::app_windows::podcasts_window;
use crate::app_windows::prompt_window::{self, PromptDirectoryOptions};
use crate::app_windows::route_service::{
    GeocodeCandidate, RouteAvoid, RouteClient, RouteOptions, RoutePath, RoutePreference,
    RouteProfile, RouteRequestResult, RouteResult, format_distance_for_language,
    format_duration_for_language,
};
use crate::app_windows::youtube_transcript_window::{
    self, MultilineSearchOptions, MultilineSelectionItem, MultilineSelectionResult,
};
use crate::editor_manager;
use crate::settings::Language;
use crate::{i18n, show_error, with_state};

pub fn open(parent: HWND) {
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    run_route_flow(parent, language);
}

fn run_route_flow(parent: HWND, language: Language) {
    let client = RouteClient::default();

    // 1. Chiedi partenza, arrivo e profilo
    let Some(params) = prompt_route_params(parent, language) else {
        return;
    };

    crate::screen_reader_speak(&i18n::tr(language, "route.searching"));

    // 2. Esegui la ricerca
    match client.route_from_addresses(&params.from, &params.to, params.plan.clone()) {
        Ok(RouteRequestResult::Ready(route)) => {
            display_route(parent, language, route);
        }
        Ok(RouteRequestResult::NeedsSelection {
            from_candidates,
            to_candidates,
            profile,
            preference,
            avoid,
            include_municipalities,
        }) => {
            let plan = RouteOptions {
                profile,
                preference,
                avoid,
                include_municipalities,
                language,
                country: params.plan.country.clone(),
            };
            handle_selection_and_route(
                parent,
                language,
                &client,
                from_candidates,
                to_candidates,
                plan,
            );
        }
        Err(err) => {
            show_error(parent, language, &err.to_string());
        }
    }
}

struct RouteParams {
    from: String,
    to: String,
    plan: RouteOptions,
}

fn prompt_route_params(parent: HWND, language: Language) -> Option<RouteParams> {
    let profile_options = [
        RouteProfile::Walking,
        RouteProfile::Cycling,
        RouteProfile::Driving,
        RouteProfile::Wheelchair,
    ];
    let preference_options = [RoutePreference::Fastest, RoutePreference::Shortest];
    let avoid_options = [
        RouteAvoid::None,
        RouteAvoid::Highways,
        RouteAvoid::Tollways,
        RouteAvoid::HighwaysAndTollways,
    ];
    let country_options = podcasts_window::podcast_directory_country_options();
    let saved_country =
        with_state(parent, |state| state.settings.route_country.clone()).unwrap_or_default();
    let route_country =
        normalize_route_country(&saved_country).unwrap_or_else(|| default_route_country(language));

    let options_list = profile_options
        .iter()
        .map(|profile| profile.label(language))
        .collect();
    let preference_list = preference_options
        .iter()
        .map(|preference| preference.label(language))
        .collect();
    let avoid_list = avoid_options
        .iter()
        .map(|avoid| {
            if *avoid == RouteAvoid::None {
                i18n::tr(language, "route.avoid.none")
            } else {
                avoid.label(language)
            }
        })
        .collect();
    let country_list = country_options
        .iter()
        .map(|(code, fallback)| route_country_label(language, code, fallback))
        .collect();
    let country_default_selection = country_options
        .iter()
        .position(|(code, _)| *code == route_country)
        .unwrap_or_else(|| {
            country_options
                .iter()
                .position(|(code, _)| *code == default_route_country(language))
                .unwrap_or(0)
        });

    let options = PromptDirectoryOptions {
        title: i18n::tr(language, "route.title"),
        type_label: i18n::tr(language, "route.profile_label"),
        options: options_list,
        default_selection: 0,
        secondary_type_label: i18n::tr(language, "route.preference_label"),
        secondary_options: preference_list,
        secondary_default_selection: 0,
        tertiary_type_label: i18n::tr(language, "route.avoid_label"),
        tertiary_options: avoid_list,
        tertiary_default_selection: 0,
        tertiary_options_primary_index_only: Some(2),
        quaternary_type_label: i18n::tr(language, "route.country_label"),
        quaternary_options: country_list,
        quaternary_default_selection: country_default_selection,
        focus_primary_field: false,
        primary_label: i18n::tr(language, "route.from_label"),
        primary_labels: Vec::new(),
        primary_default: String::new(),
        secondary_label: i18n::tr(language, "route.to_label"),
        secondary_default: String::new(),
        tertiary_label: String::new(),
        tertiary_default: String::new(),
        checkbox_label: i18n::tr(language, "route.include_municipalities"),
        checkbox_default: false,
    };

    let result = prompt_window::prompt_directory_search(parent, options, language)?;

    let profile = profile_options
        .get(result.selected_index)
        .copied()
        .unwrap_or(RouteProfile::Walking);
    let preference = preference_options
        .get(result.secondary_selected_index)
        .copied()
        .unwrap_or(RoutePreference::Fastest);
    let avoid = if profile == RouteProfile::Driving {
        avoid_options
            .get(result.tertiary_selected_index)
            .copied()
            .unwrap_or(RouteAvoid::None)
    } else {
        RouteAvoid::None
    };
    let country = country_options
        .get(result.quaternary_selected_index)
        .map(|(code, _)| (*code).to_string())
        .unwrap_or_else(|| default_route_country(language).to_string());
    if let Some(settings_snapshot) = with_state(parent, |state| {
        if state.settings.route_country != country {
            state.settings.route_country = country.clone();
            Some(state.settings.clone())
        } else {
            None
        }
    })
    .flatten()
    {
        crate::settings::save_settings(settings_snapshot);
    }

    Some(RouteParams {
        from: result.primary_value,
        to: result.secondary_value,
        plan: RouteOptions {
            profile,
            preference,
            avoid,
            include_municipalities: result.checkbox_checked,
            language,
            country,
        },
    })
}

fn route_country_label(language: Language, code: &str, fallback: &str) -> String {
    let key = format!("options.podcast_country.{code}");
    let label = i18n::tr(language, &key);
    if label == key {
        fallback.to_string()
    } else {
        label
    }
}

fn normalize_route_country(country: &str) -> Option<&'static str> {
    let normalized = country.trim().to_ascii_lowercase();
    podcasts_window::podcast_directory_country_options()
        .iter()
        .find_map(|(code, _)| (*code == normalized).then_some(*code))
}

fn default_route_country(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::German => "de",
        Language::English => "us",
        Language::Spanish => "es",
        Language::Portuguese | Language::PortugueseBrazilian => "pt",
        Language::Swedish => "se",
        Language::Vietnamese => "vn",
        Language::Czech => "cz",
        Language::Polish => "pl",
        Language::French => "fr",
        Language::Serbian => "rs",
        Language::Ukrainian => "ua",
        Language::Lithuanian => "lt",
        Language::Russian => "ru",
        Language::Chinese => "cn",
        Language::Hindi => "in",
        Language::Indonesian => "id",
    }
}

fn handle_selection_and_route(
    parent: HWND,
    language: Language,
    client: &RouteClient,
    from_candidates: Vec<GeocodeCandidate>,
    to_candidates: Vec<GeocodeCandidate>,
    plan: RouteOptions,
) {
    let from_selected = if from_candidates.len() > 1 {
        let items = from_candidates
            .iter()
            .enumerate()
            .map(|(idx, c)| MultilineSelectionItem {
                id: idx.to_string(),
                title: c.display_label(),
                description: None,
            })
            .collect();

        match youtube_transcript_window::select_multiline_items_with_search(
            parent,
            language,
            i18n::tr(language, "route.choose_from"),
            items,
            None,
            MultilineSearchOptions {
                initial_query: String::new(),
                search_button_label: String::new(),
                show_search_edit: false,
                secondary_action_label: None,
                context_actions: Vec::new(),
                right_arrow_accepts_selection: true,
                left_arrow_closes: true,
                escape_stops_active_player: false,
                refresh: None,
            },
        ) {
            MultilineSelectionResult::Selected(id_str) => {
                let idx = id_str.parse::<usize>().unwrap_or(0);
                from_candidates[idx].clone()
            }
            _ => return,
        }
    } else {
        from_candidates[0].clone()
    };

    let to_selected = if to_candidates.len() > 1 {
        let items = to_candidates
            .iter()
            .enumerate()
            .map(|(idx, c)| MultilineSelectionItem {
                id: idx.to_string(),
                title: c.display_label(),
                description: None,
            })
            .collect();

        match youtube_transcript_window::select_multiline_items_with_search(
            parent,
            language,
            i18n::tr(language, "route.choose_to"),
            items,
            None,
            MultilineSearchOptions {
                initial_query: String::new(),
                search_button_label: String::new(),
                show_search_edit: false,
                secondary_action_label: None,
                context_actions: Vec::new(),
                right_arrow_accepts_selection: true,
                left_arrow_closes: true,
                escape_stops_active_player: false,
                refresh: None,
            },
        ) {
            MultilineSelectionResult::Selected(id_str) => {
                let idx = id_str.parse::<usize>().unwrap_or(0);
                to_candidates[idx].clone()
            }
            _ => return,
        }
    } else {
        to_candidates[0].clone()
    };

    crate::screen_reader_speak(&i18n::tr(language, "route.calculating"));
    match client.route_between_coordinates(&from_selected, &to_selected, plan) {
        Ok(route) => display_route(parent, language, route),
        Err(err) => show_error(parent, language, &err.to_string()),
    }
}

fn display_route(parent: HWND, language: Language, route: RouteResult) {
    let Some(route) = select_route_path(parent, language, route) else {
        return;
    };
    let route_map = route.map_data(language);
    let text = route.format_for_speech_or_text(language);
    editor_manager::new_document(parent);
    editor_manager::set_current_document_title(parent, &route.suggested_filename(language));
    if let Some(route_map) = route_map {
        editor_manager::set_current_route_map(parent, route_map);
    }
    if let Some(hwnd_edit) = crate::get_active_edit(parent) {
        editor_manager::set_edit_text(hwnd_edit, &text);
        editor_manager::mark_current_document_dirty_prefer_title(parent);
    }
}

fn select_route_path(parent: HWND, language: Language, route: RouteResult) -> Option<RouteResult> {
    crate::log_debug(&format!("route_select_path: paths={}", route.paths.len()));
    if route.paths.len() <= 1 {
        return Some(route);
    }

    let items = route
        .paths
        .iter()
        .enumerate()
        .map(|(idx, path)| route_path_selection_item(language, idx, path))
        .collect();

    let selected = youtube_transcript_window::select_multiline_items_with_search(
        parent,
        language,
        i18n::tr(language, "route.choose_route"),
        items,
        None,
        MultilineSearchOptions {
            initial_query: String::new(),
            search_button_label: String::new(),
            show_search_edit: false,
            secondary_action_label: None,
            context_actions: Vec::new(),
            right_arrow_accepts_selection: true,
            left_arrow_closes: true,
            escape_stops_active_player: false,
            refresh: None,
        },
    );

    match selected {
        MultilineSelectionResult::Selected(id_str) => {
            let idx = id_str.parse::<usize>().unwrap_or(0);
            let selected_path = route.paths.get(idx)?.clone();
            Some(RouteResult {
                paths: vec![selected_path],
                ..route
            })
        }
        _ => None,
    }
}

fn route_path_selection_item(
    language: Language,
    index: usize,
    path: &RoutePath,
) -> MultilineSelectionItem {
    let title = if index == 0 {
        i18n::tr(language, "route.main_route")
    } else {
        format!("{} {index}", i18n::tr(language, "route.alternative_route"))
    };
    let description = format!(
        "{}; {}",
        format_distance_for_language(path.distance_meters, language),
        format_duration_for_language(path.duration_seconds, language)
    );

    MultilineSelectionItem {
        id: index.to_string(),
        title,
        description: Some(description),
    }
}
