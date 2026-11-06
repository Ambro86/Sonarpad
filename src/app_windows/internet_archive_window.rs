use reqwest::blocking::Client;
use serde_json::Value;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use windows::Win32::Foundation::HWND;

use crate::app_windows::prompt_window::{self, PromptDirectoryOptions};
use crate::app_windows::youtube_transcript_window::{
    self, MultilineSearchOptions, MultilineSelectionItem, MultilineSelectionResult,
};
use crate::settings::Language;
use crate::{RaiAudioOrigin, i18n, show_error, with_state};

const SEARCH_URL: &str = "https://archive.org/advancedsearch.php";
const USER_AGENT: &str = concat!("Sonarpad/", env!("CARGO_PKG_VERSION"));
const PAGE_ROWS: usize = 50;
const PREVIOUS_RESULTS_ID: &str = "__sonarpad_internet_archive_previous_results__";
const NEXT_RESULTS_ID: &str = "__sonarpad_internet_archive_next_results__";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ArchiveSource {
    OldTimeRadio,
    Speeches,
    LiveMusic,
}

#[derive(Clone)]
struct ArchiveSearch {
    query: String,
    source: ArchiveSource,
}

#[derive(Clone)]
struct ArchiveItem {
    identifier: String,
    title: String,
    creator: String,
    description: String,
}

#[derive(Clone)]
struct ArchiveTrack {
    title: String,
    file_name: String,
    audio_url: String,
    format: String,
    length: String,
}

#[derive(Clone)]
struct ArchiveParentListContext {
    search: ArchiveSearch,
    items: Vec<ArchiveItem>,
    has_more: bool,
    current_page: usize,
    selected_id: String,
}

#[derive(Clone)]
struct ArchivePlayerReturnContext {
    item: ArchiveItem,
    tracks: Vec<ArchiveTrack>,
    selected_url: String,
    parent_list: ArchiveParentListContext,
}

static ARCHIVE_PLAYER_RETURN_CONTEXT: OnceLock<Mutex<Option<ArchivePlayerReturnContext>>> =
    OnceLock::new();

fn player_return_context() -> &'static Mutex<Option<ArchivePlayerReturnContext>> {
    ARCHIVE_PLAYER_RETURN_CONTEXT.get_or_init(|| Mutex::new(None))
}

fn remember_player_return_context(context: ArchivePlayerReturnContext) {
    if let Ok(mut stored) = player_return_context().lock() {
        *stored = Some(context);
    }
}

fn clear_player_return_context() {
    if let Ok(mut stored) = player_return_context().lock() {
        *stored = None;
    }
}

pub(crate) fn restore_episode_list_after_stop(parent: HWND, stopped_url: Option<&str>) -> bool {
    let Some(stopped_url) = stopped_url else {
        return false;
    };
    let context = player_return_context()
        .lock()
        .ok()
        .and_then(|stored| stored.clone())
        .filter(|context| context.selected_url == stopped_url);
    let Some(context) = context else {
        return false;
    };

    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    clear_player_return_context();
    let parent_list = context.parent_list.clone();
    let started_playback = browse_loaded_tracks(
        parent,
        language,
        &context.item,
        context.tracks,
        Some(context.selected_url),
        parent_list.clone(),
    );
    if !started_playback {
        browse_parent_list(parent, language, parent_list);
    }
    true
}

struct ArchivePage {
    items: Vec<ArchiveItem>,
    has_more: bool,
}

#[derive(Clone)]
struct ArchiveClient {
    client: Client,
}

impl Default for ArchiveClient {
    fn default() -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(35))
            .build()
            .unwrap_or_else(|error| {
                crate::log_debug(&format!(
                    "Internet Archive HTTP client setup failed: {error}"
                ));
                Client::new()
            });
        Self { client }
    }
}

impl ArchiveClient {
    fn search(&self, search: &ArchiveSearch, page: usize) -> Result<ArchivePage, String> {
        let page_value = page.to_string();
        let rows_value = PAGE_ROWS.to_string();
        let query = archive_query(search.source, &search.query);
        let response = self
            .client
            .get(SEARCH_URL)
            .query(&[
                ("q", query.as_str()),
                ("output", "json"),
                ("page", page_value.as_str()),
                ("rows", rows_value.as_str()),
                ("sort[]", "downloads desc"),
                ("fl[]", "identifier"),
                ("fl[]", "title"),
                ("fl[]", "creator"),
                ("fl[]", "description"),
            ])
            .header("Accept", "application/json")
            .send()
            .map_err(|error| format!("Internet Archive network error: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Internet Archive server error: {error}"))?;
        let root = response
            .json::<Value>()
            .map_err(|error| format!("Invalid Internet Archive response: {error}"))?;
        let empty_response = Value::Null;
        let response_value = root.get("response").unwrap_or(&empty_response);
        let total = value_as_usize(response_value.get("numFound"));
        let items = response_value
            .get("docs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(parse_archive_item)
            .collect::<Vec<_>>();
        Ok(ArchivePage {
            items,
            has_more: page.saturating_mul(PAGE_ROWS) < total,
        })
    }

    fn tracks(&self, identifier: &str) -> Result<Vec<ArchiveTrack>, String> {
        let url = format!("https://archive.org/metadata/{identifier}");
        let response = self
            .client
            .get(url)
            .header("Accept", "application/json")
            .send()
            .map_err(|error| format!("Internet Archive network error: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Internet Archive server error: {error}"))?;
        let root = response
            .json::<Value>()
            .map_err(|error| format!("Invalid Internet Archive metadata: {error}"))?;
        let mut seen_urls = std::collections::HashSet::new();
        let tracks = root
            .get("files")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|value| parse_archive_track(identifier, value))
            .filter(|track| seen_urls.insert(track.audio_url.clone()))
            .collect::<Vec<_>>();
        Ok(tracks)
    }
}

pub fn open(parent: HWND) {
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    let Some(mut search) = prompt_search(parent, language, None) else {
        return;
    };
    let client = ArchiveClient::default();

    loop {
        crate::screen_reader_speak(&i18n::tr(language, "internet_archive.loading"));
        let first_page = match client.search(&search, 1) {
            Ok(page) => page,
            Err(error) => {
                show_archive_error(parent, language, &error);
                return;
            }
        };
        let mut items = first_page.items;
        let mut has_more = first_page.has_more;
        let mut current_page = 1usize;
        let mut selected_id = None;

        if items.is_empty() {
            show_error(
                parent,
                language,
                &i18n::tr(language, "internet_archive.no_results"),
            );
            let Some(new_search) = prompt_search(parent, language, Some(&search)) else {
                return;
            };
            search = new_search;
            continue;
        }

        loop {
            let list = archive_result_items(&items, language, current_page > 1, has_more);
            let result = youtube_transcript_window::select_multiline_items_with_search_without_parent_restore_on_action(
                parent,
                language,
                i18n::tr(language, "internet_archive.title"),
                list,
                selected_id.clone(),
                MultilineSearchOptions {
                    initial_query: search.query.clone(),
                    search_button_label: i18n::tr(language, "podcasts.search.button"),
                    show_search_edit: true,
                    secondary_action_label: None,
                    context_actions: Vec::new(),
                    right_arrow_accepts_selection: true,
                    left_arrow_closes: true,
                    escape_stops_active_player: true,
                    refresh: None,
                },
            );

            match result {
                MultilineSelectionResult::Cancelled => return,
                MultilineSelectionResult::Search(query) => {
                    search.query = query.trim().to_string();
                    break;
                }
                MultilineSelectionResult::SecondaryAction => {}
                MultilineSelectionResult::Selected(identifier) => {
                    if identifier == PREVIOUS_RESULTS_ID || identifier == NEXT_RESULTS_ID {
                        let requested_page = if identifier == PREVIOUS_RESULTS_ID {
                            current_page.saturating_sub(1).max(1)
                        } else {
                            current_page.saturating_add(1)
                        };
                        crate::screen_reader_speak(&i18n::tr(language, "internet_archive.loading"));
                        match client.search(&search, requested_page) {
                            Ok(page) if !page.items.is_empty() => {
                                items = page.items;
                                has_more = page.has_more;
                                current_page = requested_page;
                                selected_id = Some(first_archive_result_id(
                                    &items,
                                    current_page > 1,
                                    has_more,
                                ));
                            }
                            Ok(_) => {
                                if identifier == NEXT_RESULTS_ID {
                                    has_more = false;
                                }
                            }
                            Err(error) => show_archive_error(parent, language, &error),
                        }
                        continue;
                    }
                    selected_id = Some(identifier.clone());
                    if let Some(item) = items.iter().find(|item| item.identifier == identifier) {
                        let parent_list = ArchiveParentListContext {
                            search: search.clone(),
                            items: items.clone(),
                            has_more,
                            current_page,
                            selected_id: identifier,
                        };
                        if browse_tracks(parent, language, &client, item, parent_list) {
                            return;
                        }
                    }
                }
            }
        }
    }
}

fn prompt_search(
    parent: HWND,
    language: Language,
    previous: Option<&ArchiveSearch>,
) -> Option<ArchiveSearch> {
    let sources = [
        ArchiveSource::OldTimeRadio,
        ArchiveSource::Speeches,
        ArchiveSource::LiveMusic,
    ];
    let default_selection = previous
        .and_then(|value| sources.iter().position(|source| *source == value.source))
        .unwrap_or(0);
    let result = prompt_window::prompt_directory_search(
        parent,
        PromptDirectoryOptions {
            title: i18n::tr(language, "internet_archive.title"),
            type_label: i18n::tr(language, "internet_archive.source"),
            options: sources
                .iter()
                .map(|source| source_label(*source, language))
                .collect(),
            default_selection,
            secondary_type_label: String::new(),
            secondary_options: Vec::new(),
            secondary_default_selection: 0,
            tertiary_type_label: String::new(),
            tertiary_options: Vec::new(),
            tertiary_default_selection: 0,
            tertiary_options_primary_index_only: None,
            secondary_controls_disabled_primary_index: None,
            quaternary_type_label: String::new(),
            quaternary_options: Vec::new(),
            quaternary_default_selection: 0,
            focus_primary_field: true,
            primary_label: i18n::tr(language, "internet_archive.search_prompt"),
            primary_labels: Vec::new(),
            primary_default: previous
                .map(|value| value.query.clone())
                .unwrap_or_default(),
            secondary_label: String::new(),
            secondary_default: String::new(),
            tertiary_label: String::new(),
            tertiary_default: String::new(),
            checkbox_label: String::new(),
            checkbox_default: false,
        },
        language,
    )?;
    Some(ArchiveSearch {
        query: result.primary_value.trim().to_string(),
        source: sources
            .get(result.selected_index)
            .copied()
            .unwrap_or(ArchiveSource::OldTimeRadio),
    })
}

fn browse_tracks(
    parent: HWND,
    language: Language,
    client: &ArchiveClient,
    item: &ArchiveItem,
    parent_list: ArchiveParentListContext,
) -> bool {
    crate::screen_reader_speak(&i18n::tr(language, "internet_archive.loading_tracks"));
    let tracks = match client.tracks(&item.identifier) {
        Ok(tracks) => tracks,
        Err(error) => {
            show_archive_error(parent, language, &error);
            return false;
        }
    };
    if tracks.is_empty() {
        show_error(
            parent,
            language,
            &i18n::tr(language, "internet_archive.no_tracks"),
        );
        return false;
    }
    browse_loaded_tracks(parent, language, item, tracks, None, parent_list)
}

fn browse_loaded_tracks(
    parent: HWND,
    language: Language,
    item: &ArchiveItem,
    tracks: Vec<ArchiveTrack>,
    mut selected_id: Option<String>,
    parent_list: ArchiveParentListContext,
) -> bool {
    loop {
        let list = tracks
            .iter()
            .map(|track| MultilineSelectionItem {
                id: track.audio_url.clone(),
                title: track.title.clone(),
                description: Some(track_description(track)),
            })
            .collect();
        match youtube_transcript_window::select_multiline_items_with_search(
            parent,
            language,
            item.title.clone(),
            list,
            selected_id.clone(),
            MultilineSearchOptions {
                initial_query: String::new(),
                search_button_label: String::new(),
                show_search_edit: false,
                secondary_action_label: None,
                context_actions: Vec::new(),
                right_arrow_accepts_selection: true,
                left_arrow_closes: true,
                escape_stops_active_player: true,
                refresh: None,
            },
        ) {
            MultilineSelectionResult::Cancelled => return false,
            MultilineSelectionResult::Selected(url) => {
                selected_id = Some(url.clone());
                if let Some(track) = tracks.iter().find(|track| track.audio_url == url) {
                    let title = format!("{} - {}", item.title, track.title);
                    remember_player_return_context(ArchivePlayerReturnContext {
                        item: item.clone(),
                        tracks: tracks.clone(),
                        selected_url: track.audio_url.clone(),
                        parent_list: parent_list.clone(),
                    });
                    crate::play_named_remote_audio_from_url_with_rai_origin(
                        parent,
                        track.audio_url.clone(),
                        Some(title),
                        Some(track_mime(track)),
                        RaiAudioOrigin::None,
                    );
                    return true;
                }
            }
            MultilineSelectionResult::Search(_) | MultilineSelectionResult::SecondaryAction => {}
        }
    }
}

fn browse_parent_list(parent: HWND, language: Language, mut context: ArchiveParentListContext) {
    let client = ArchiveClient::default();
    loop {
        let list = archive_result_items(
            &context.items,
            language,
            context.current_page > 1,
            context.has_more,
        );
        match youtube_transcript_window::select_multiline_items_with_search_without_parent_restore_on_action(
            parent,
            language,
            i18n::tr(language, "internet_archive.title"),
            list,
            Some(context.selected_id.clone()),
            MultilineSearchOptions {
                initial_query: context.search.query.clone(),
                search_button_label: i18n::tr(language, "podcasts.search.button"),
                show_search_edit: true,
                secondary_action_label: None,
                context_actions: Vec::new(),
                right_arrow_accepts_selection: true,
                left_arrow_closes: true,
                escape_stops_active_player: true,
                refresh: None,
            },
        ) {
            MultilineSelectionResult::Cancelled => return,
            MultilineSelectionResult::Search(query) => {
                context.search.query = query.trim().to_string();
                crate::screen_reader_speak(&i18n::tr(language, "internet_archive.loading"));
                match client.search(&context.search, 1) {
                    Ok(page) if !page.items.is_empty() => {
                        context.items = page.items;
                        context.has_more = page.has_more;
                        context.current_page = 1;
                        context.selected_id = context.items[0].identifier.clone();
                    }
                    Ok(_) => show_error(
                        parent,
                        language,
                        &i18n::tr(language, "internet_archive.no_results"),
                    ),
                    Err(error) => show_archive_error(parent, language, &error),
                }
            }
            MultilineSelectionResult::SecondaryAction => {}
            MultilineSelectionResult::Selected(identifier) => {
                if identifier == PREVIOUS_RESULTS_ID || identifier == NEXT_RESULTS_ID {
                    let requested_page = if identifier == PREVIOUS_RESULTS_ID {
                        context.current_page.saturating_sub(1).max(1)
                    } else {
                        context.current_page.saturating_add(1)
                    };
                    crate::screen_reader_speak(&i18n::tr(language, "internet_archive.loading"));
                    match client.search(&context.search, requested_page) {
                        Ok(page) if !page.items.is_empty() => {
                            context.items = page.items;
                            context.has_more = page.has_more;
                            context.current_page = requested_page;
                            context.selected_id = first_archive_result_id(
                                &context.items,
                                context.current_page > 1,
                                context.has_more,
                            );
                        }
                        Ok(_) => {
                            if identifier == NEXT_RESULTS_ID {
                                context.has_more = false;
                            }
                        }
                        Err(error) => show_archive_error(parent, language, &error),
                    }
                    continue;
                }
                context.selected_id = identifier.clone();
                if let Some(item) = context
                    .items
                    .iter()
                    .find(|item| item.identifier == identifier)
                    .cloned()
                    && browse_tracks(parent, language, &client, &item, context.clone())
                {
                    return;
                }
            }
        }
    }
}

fn archive_result_items(
    items: &[ArchiveItem],
    language: Language,
    has_previous: bool,
    has_more: bool,
) -> Vec<MultilineSelectionItem> {
    let mut result = Vec::with_capacity(items.len().saturating_add(2));
    if has_previous {
        result.push(MultilineSelectionItem {
            id: PREVIOUS_RESULTS_ID.to_string(),
            title: i18n::tr(language, "podcasts.categories.previous_results"),
            description: None,
        });
    }
    result.extend(items.iter().map(|item| MultilineSelectionItem {
        id: item.identifier.clone(),
        title: item.title.clone(),
        description: Some(item_description(item)),
    }));
    if has_more {
        result.push(MultilineSelectionItem {
            id: NEXT_RESULTS_ID.to_string(),
            title: i18n::tr(language, "podcasts.categories.next_results"),
            description: None,
        });
    }
    result
}

fn first_archive_result_id(items: &[ArchiveItem], has_previous: bool, has_more: bool) -> String {
    if has_previous {
        PREVIOUS_RESULTS_ID.to_string()
    } else if let Some(item) = items.first() {
        item.identifier.clone()
    } else if has_more {
        NEXT_RESULTS_ID.to_string()
    } else {
        String::new()
    }
}

fn parse_archive_item(value: &Value) -> Option<ArchiveItem> {
    let identifier = value_as_string(value.get("identifier"));
    if identifier.is_empty() {
        return None;
    }
    let title = value_as_string(value.get("title"));
    Some(ArchiveItem {
        identifier: identifier.clone(),
        title: if title.is_empty() { identifier } else { title },
        creator: value_as_string(value.get("creator")),
        description: value_as_string(value.get("description")),
    })
}

fn parse_archive_track(identifier: &str, value: &Value) -> Option<ArchiveTrack> {
    let file_name = value_as_string(value.get("name"));
    if file_name.is_empty() {
        return None;
    }
    let format = value_as_string(value.get("format"));
    if !is_audio_file(&format, &file_name) {
        return None;
    }
    let title = value_as_string(value.get("title"));
    let display_title = if title.is_empty() {
        display_file_name(&file_name)
    } else {
        title
    };
    let audio_url = archive_download_url(identifier, &file_name)?;
    Some(ArchiveTrack {
        title: display_title,
        file_name,
        audio_url,
        format,
        length: value_as_string(value.get("length")),
    })
}

fn archive_download_url(identifier: &str, file_name: &str) -> Option<String> {
    let mut url = url::Url::parse("https://archive.org/download/").ok()?;
    {
        let mut segments = url.path_segments_mut().ok()?;
        segments.pop_if_empty().push(identifier);
        for segment in file_name.split('/') {
            segments.push(segment);
        }
    }
    Some(url.into())
}

fn archive_query(source: ArchiveSource, query: &str) -> String {
    let base = match source {
        ArchiveSource::OldTimeRadio => "collection:oldtimeradio AND mediatype:audio",
        ArchiveSource::LiveMusic => "collection:etree AND mediatype:audio",
        ArchiveSource::Speeches => {
            "mediatype:audio AND (subject:speech OR title:speech OR description:speech)"
        }
    };
    let escaped = query.trim().replace('"', "");
    if escaped.is_empty() {
        base.to_string()
    } else {
        format!(
            "{base} AND (title:\"{escaped}\" OR creator:\"{escaped}\" OR description:\"{escaped}\")"
        )
    }
}

fn source_label(source: ArchiveSource, language: Language) -> String {
    let key = match source {
        ArchiveSource::OldTimeRadio => "internet_archive.source.old_time_radio",
        ArchiveSource::Speeches => "internet_archive.source.speeches",
        ArchiveSource::LiveMusic => "internet_archive.source.live_music",
    };
    i18n::tr(language, key)
}

fn item_description(item: &ArchiveItem) -> String {
    [item.creator.as_str(), item.description.as_str()]
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn track_description(track: &ArchiveTrack) -> String {
    [track.format.as_str(), track.length.as_str()]
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" - ")
}

fn track_mime(track: &ArchiveTrack) -> &'static str {
    if track.file_name.to_lowercase().ends_with(".ogg")
        || track.format.to_lowercase().contains("ogg")
    {
        "audio/ogg"
    } else {
        "audio/mpeg"
    }
}

fn is_audio_file(format: &str, file_name: &str) -> bool {
    let format = format.to_lowercase();
    let file_name = file_name.to_lowercase();
    format.contains("vbr mp3")
        || format == "mp3"
        || format.contains("ogg")
        || file_name.ends_with(".mp3")
        || file_name.ends_with(".ogg")
}

fn display_file_name(value: &str) -> String {
    let base = value.rsplit('/').next().unwrap_or(value);
    base.rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(base)
        .to_string()
}

fn value_as_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Array(values)) => values
            .iter()
            .find_map(|value| match value {
                Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_string()),
                _ => None,
            })
            .unwrap_or_default(),
        Some(Value::Number(number)) => number.to_string(),
        _ => String::new(),
    }
}

fn value_as_usize(value: Option<&Value>) -> usize {
    match value {
        Some(Value::Number(number)) => number.as_u64().unwrap_or(0) as usize,
        Some(Value::String(text)) => text.parse().unwrap_or(0),
        _ => 0,
    }
}

fn show_archive_error(parent: HWND, language: Language, error: &str) {
    crate::log_debug(&format!("Internet Archive error: {error}"));
    let message = i18n::tr_f(language, "internet_archive.error", &[("err", error)]);
    show_error(parent, language, &message);
}

#[cfg(test)]
mod tests {
    use super::archive_download_url;

    #[test]
    fn archive_download_url_encodes_spaces_as_path_segments() {
        let url = archive_download_url(
            "OTRR_Gunsmoke_Singles",
            "Gunsmoke 52-04-26 (001) Billy the Kid.mp3",
        )
        .expect("valid Archive download URL");

        assert_eq!(
            url,
            "https://archive.org/download/OTRR_Gunsmoke_Singles/Gunsmoke%2052-04-26%20(001)%20Billy%20the%20Kid.mp3"
        );
        assert!(!url.contains('+'));
    }

    #[test]
    fn archive_download_url_preserves_nested_file_paths() {
        let url = archive_download_url("collection id", "disc 1/track #1.mp3")
            .expect("valid nested Archive download URL");

        assert_eq!(
            url,
            "https://archive.org/download/collection%20id/disc%201/track%20%231.mp3"
        );
    }
}
