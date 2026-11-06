#[path = "route_history.rs"]
mod route_history;

use windows::Win32::Foundation::HWND;

use crate::app_windows::podcasts_window;
use crate::app_windows::prompt_window::{self, PromptDirectoryOptions};
use crate::app_windows::route_service::{
    GeocodeCandidate, RouteAvoid, RouteClient, RouteDeparture, RouteOptions, RoutePath,
    RoutePreference, RouteProfile, RouteRequestResult, RouteResult,
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
            remember_route(&route, &params.plan);
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
                departure: params.plan.departure.clone(),
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

fn remember_route(route: &RouteResult, plan: &RouteOptions) {
    route_history::remember(route_history::RecentRoute {
        from: route.from_label.clone(),
        to: route.to_label.clone(),
        profile: plan.profile.api_value().into(),
        preference: plan.preference.api_value().into(),
        avoid: plan.avoid.api_value().into(),
        country: plan.country.clone(),
        municipalities: plan.include_municipalities,
    });
}

fn choose_recent_route(parent: HWND, language: Language) -> Option<RouteParams> {
    loop {
        let history = route_history::load();
        let mut items = Vec::new();
        for (index, item) in history.iter().enumerate() {
            items.push(MultilineSelectionItem {
                id: index.to_string(),
                title: format!("{} → {}", item.from, item.to),
                description: Some(recent_profile(&item.profile).label(language)),
            });
        }
        match youtube_transcript_window::select_multiline_items_with_search(
            parent,
            language,
            i18n::tr(language, "route.recent_routes"),
            items,
            None,
            MultilineSearchOptions {
                initial_query: String::new(),
                search_button_label: String::new(),
                show_search_edit: false,
                secondary_action_label: Some(i18n::tr(language, "route.clear_history")),
                context_actions: Vec::new(),
                right_arrow_accepts_selection: true,
                left_arrow_closes: true,
                escape_stops_active_player: false,
                refresh: None,
            },
        ) {
            MultilineSelectionResult::Selected(id) => {
                let item = history.get(id.parse::<usize>().ok()?)?;
                return Some(RouteParams {
                    from: item.from.clone(),
                    to: item.to.clone(),
                    plan: RouteOptions {
                        departure: None,
                        profile: recent_profile(&item.profile),
                        preference: if item.preference == "shortest" {
                            RoutePreference::Shortest
                        } else {
                            RoutePreference::Fastest
                        },
                        avoid: match item.avoid.as_str() {
                            "highways" => RouteAvoid::Highways,
                            "tollways" => RouteAvoid::Tollways,
                            "highways,tollways" => RouteAvoid::HighwaysAndTollways,
                            _ => RouteAvoid::None,
                        },
                        include_municipalities: item.municipalities,
                        language,
                        country: item.country.clone(),
                    },
                });
            }
            MultilineSelectionResult::SecondaryAction => route_history::save(&[]),
            _ => return None,
        }
    }
}

fn recent_route_fields(
    parent: HWND,
    language: Language,
) -> Option<prompt_window::PromptDirectoryResult> {
    let params = choose_recent_route(parent, language)?;
    Some(prompt_window::PromptDirectoryResult {
        departure: None,
        selected_index: match params.plan.profile {
            RouteProfile::Walking => 0,
            RouteProfile::Cycling => 1,
            RouteProfile::Driving => 2,
            RouteProfile::Wheelchair => 3,
            RouteProfile::Transit => 4,
        },
        secondary_selected_index: usize::from(params.plan.preference == RoutePreference::Shortest),
        tertiary_selected_index: match params.plan.avoid {
            RouteAvoid::None => 0,
            RouteAvoid::Highways => 1,
            RouteAvoid::Tollways => 2,
            RouteAvoid::HighwaysAndTollways => 3,
        },
        quaternary_selected_index: podcasts_window::podcast_directory_country_options()
            .iter()
            .position(|(code, _)| *code == params.plan.country)
            .unwrap_or(0),
        primary_value: params.from,
        secondary_value: params.to,
        tertiary_value: String::new(),
        checkbox_checked: params.plan.include_municipalities,
    })
}

fn recent_profile(value: &str) -> RouteProfile {
    match value {
        "driving-car" => RouteProfile::Driving,
        "cycling-regular" => RouteProfile::Cycling,
        "wheelchair" => RouteProfile::Wheelchair,
        "transit" => RouteProfile::Transit,
        _ => RouteProfile::Walking,
    }
}

fn prompt_route_params(parent: HWND, language: Language) -> Option<RouteParams> {
    let profile_options = [
        RouteProfile::Walking,
        RouteProfile::Cycling,
        RouteProfile::Driving,
        RouteProfile::Wheelchair,
        RouteProfile::Transit,
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
        secondary_controls_disabled_primary_index: Some(4),
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

    let result = prompt_window::prompt_directory_search_with_action(
        parent,
        options,
        language,
        Some((
            i18n::tr(language, "route.recent_routes"),
            recent_route_fields,
        )),
        Some(
            country_options
                .iter()
                .map(|(code, _)| {
                    crate::app_windows::route_service::default_departure_zone(code).to_string()
                })
                .collect(),
        ),
    )?;

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

    let departure = if profile == RouteProfile::Transit {
        result
            .departure
            .map(|(time, time_zone)| RouteDeparture { time, time_zone })
    } else {
        None
    };
    Some(RouteParams {
        from: result.primary_value,
        to: result.secondary_value,
        plan: RouteOptions {
            departure,
            profile,
            preference,
            avoid,
            include_municipalities: result.checkbox_checked,
            language,
            country,
        },
    })
}

pub(crate) fn route_country_label(language: Language, code: &str, fallback: &str) -> String {
    let key = format!("options.podcast_country.{code}");
    let label = i18n::tr(language, &key);
    if label == key {
        fallback.to_string()
    } else {
        label
    }
}

pub(crate) fn normalize_route_country(country: &str) -> Option<&'static str> {
    let normalized = country.trim().to_ascii_lowercase();
    podcasts_window::podcast_directory_country_options()
        .iter()
        .find_map(|(code, _)| (*code == normalized).then_some(*code))
}

pub(crate) fn default_route_country(language: Language) -> &'static str {
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
    match client.route_between_coordinates(&from_selected, &to_selected, plan.clone()) {
        Ok(route) => {
            remember_route(&route, &plan);
            display_route(parent, language, route);
        }
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
        path.distance_label(language),
        path.duration_label(language)
    );

    MultilineSelectionItem {
        id: index.to_string(),
        title,
        description: Some(description),
    }
}
