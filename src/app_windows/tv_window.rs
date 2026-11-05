use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_RETURN, VK_RIGHT};
use windows::Win32::UI::WindowsAndMessaging::WM_CLOSE;

use crate::app_windows::interpreter_select_window::InterpreterContextAction;
use crate::app_windows::youtube_transcript_window::{
    self, MultilineSearchOptions, MultilineSelectionItem, MultilineSelectionResult,
};
use crate::app_windows::{scheduled_recording_window, tv_guide_window};
use crate::settings::{Language, TvFavorite, load_settings, save_settings};
use crate::stream_recording::{self, StreamRecordingKind};
use crate::tools::tv::{self, TvChannel, TvProgram};
use crate::{i18n, show_error, with_state};

#[derive(Clone, Debug, PartialEq, Eq)]
enum TvPage {
    Root,
    Category(String),
    Regions,
    Region(String),
    Favorites,
    Search(String),
}

fn program_page_cache_key(page: &TvPage) -> Option<String> {
    match page {
        TvPage::Category(category) => Some(format!("category:{category}")),
        TvPage::Region(region) => Some(format!("region:{region}")),
        TvPage::Favorites => Some("favorites".to_string()),
        TvPage::Search(query) => Some(format!("search:{}", query.trim().to_lowercase())),
        TvPage::Root | TvPage::Regions => None,
    }
}

fn wait_for_tv_navigation_accept_keys_release() {
    const PRESSED_MASK: i16 = 0x8000u16 as i16;
    for _ in 0..200 {
        let return_down = unsafe { GetAsyncKeyState(VK_RETURN.0 as i32) } & PRESSED_MASK != 0;
        let right_down = unsafe { GetAsyncKeyState(VK_RIGHT.0 as i32) } & PRESSED_MASK != 0;
        if !return_down && !right_down {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    crate::log_debug("TV: timed out waiting for navigation accept keys to be released");
}

#[derive(Clone, Debug)]
enum TvEntryKind {
    Page(TvPage),
    Channel(usize),
    Favorite(Box<TvFavorite>),
    Recordings,
}

#[derive(Clone, Debug)]
enum TvDeferredAction {
    Refresh,
    Record {
        channel: Box<TvChannel>,
        selected_id: String,
    },
    Guide {
        channel: Box<TvChannel>,
        selected_id: String,
    },
    Schedule {
        channel: Box<TvChannel>,
        selected_id: String,
    },
}

static TV_DEFERRED_ACTION: OnceLock<Mutex<Option<TvDeferredAction>>> = OnceLock::new();

fn set_tv_deferred_action(action: TvDeferredAction) {
    *TV_DEFERRED_ACTION
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|err| err.into_inner()) = Some(action);
}

fn take_tv_deferred_action() -> Option<TvDeferredAction> {
    TV_DEFERRED_ACTION
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .take()
}

fn close_tv_browser_dialog() {
    let dialog = crate::get_foreground_window_safe();
    if dialog.0 != 0
        && let Err(err) = crate::post_message_w_safe(dialog, WM_CLOSE, WPARAM(0), LPARAM(0))
    {
        crate::log_debug(&format!("TV: failed to close channel browser: {err}"));
    }
}

#[derive(Clone, Debug)]
struct TvEntry {
    id: String,
    title: String,
    description: Option<String>,
    kind: TvEntryKind,
}

#[derive(Debug)]
struct TvCatalog {
    channels: Vec<TvChannel>,
    categories: Vec<(String, Vec<usize>)>,
    regions: Vec<(String, Vec<usize>)>,
    current_programs: Mutex<HashMap<String, TvProgram>>,
    loaded_program_pages: Mutex<HashSet<String>>,
}

impl TvCatalog {
    fn new(channels: Vec<TvChannel>) -> Self {
        let mut categories = Vec::<(String, Vec<usize>)>::new();
        let mut regions = Vec::<(String, Vec<usize>)>::new();

        for (index, channel) in channels.iter().enumerate() {
            if channel.is_regional() {
                let fallback_region = i18n::tr_tv("tv.other_regions");
                let region = channel.regional_name().unwrap_or(fallback_region.as_str());
                push_group_index(&mut regions, region, index);
            } else {
                push_group_index(&mut categories, &channel.category, index);
            }
        }

        Self {
            channels,
            categories,
            regions,
            current_programs: Mutex::new(HashMap::new()),
            loaded_program_pages: Mutex::new(HashSet::new()),
        }
    }

    fn load_current_programs_for_page(&self, page: &TvPage) -> Result<bool, String> {
        let Some(page_key) = program_page_cache_key(page) else {
            return Ok(false);
        };
        {
            let mut loaded_pages = self
                .loaded_program_pages
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            if !loaded_pages.insert(page_key) {
                return Ok(false);
            }
        }

        let channels = self.channels_for_program_page(page);
        if channels.is_empty() {
            return Ok(false);
        }
        crate::screen_reader_speak(&i18n::tr_tv("tv.loading_programs"));
        let programs = tv::load_current_programs(&channels)?;
        self.current_programs
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .extend(programs);
        Ok(true)
    }

    fn channels_for_program_page(&self, page: &TvPage) -> Vec<TvChannel> {
        let indices = match page {
            TvPage::Category(category) => self
                .categories
                .iter()
                .find(|(name, _)| name == category)
                .map(|(_, indices)| indices.as_slice()),
            TvPage::Region(region) => self
                .regions
                .iter()
                .find(|(name, _)| name == region)
                .map(|(_, indices)| indices.as_slice()),
            _ => None,
        };
        if let Some(indices) = indices {
            return indices
                .iter()
                .filter_map(|index| self.channels.get(*index).cloned())
                .collect();
        }
        match page {
            TvPage::Favorites => load_settings()
                .tv_favorites
                .iter()
                .map(|favorite| channel_from_favorite(favorite, &self.channels))
                .collect(),
            TvPage::Search(query) => self
                .channels
                .iter()
                .filter(|channel| tv::matches_search(channel, query))
                .cloned()
                .collect(),
            _ => Vec::new(),
        }
    }

    fn entries_for_page(&self, page: &TvPage) -> Vec<TvEntry> {
        match page {
            TvPage::Root => self.root_entries(),
            TvPage::Category(category) => self.channel_entries(
                self.categories
                    .iter()
                    .find(|(name, _)| name == category)
                    .map(|(_, indices)| indices.as_slice())
                    .unwrap_or_default(),
            ),
            TvPage::Regions => self.region_entries(),
            TvPage::Region(region) => self.channel_entries(
                self.regions
                    .iter()
                    .find(|(name, _)| name == region)
                    .map(|(_, indices)| indices.as_slice())
                    .unwrap_or_default(),
            ),
            TvPage::Favorites => self.favorite_entries(),
            TvPage::Search(query) => {
                let indices = self
                    .channels
                    .iter()
                    .enumerate()
                    .filter_map(|(index, channel)| {
                        tv::matches_search(channel, query).then_some(index)
                    })
                    .collect::<Vec<_>>();
                self.channel_entries(&indices)
            }
        }
    }

    fn root_entries(&self) -> Vec<TvEntry> {
        let mut entries = Vec::new();
        let favorites = load_settings().tv_favorites;
        if !favorites.is_empty() {
            entries.push(TvEntry {
                id: "favorites".to_string(),
                title: i18n::tr_tv("tv.favorites"),
                description: Some(channel_count_description(favorites.len())),
                kind: TvEntryKind::Page(TvPage::Favorites),
            });
        }
        entries.extend(
            self.categories
                .iter()
                .enumerate()
                .map(|(index, (name, channels))| TvEntry {
                    id: format!("category:{index}"),
                    title: name.clone(),
                    description: Some(channel_count_description(channels.len())),
                    kind: TvEntryKind::Page(TvPage::Category(name.clone())),
                })
                .collect::<Vec<_>>(),
        );

        if !self.regions.is_empty() {
            let channel_count = self
                .regions
                .iter()
                .map(|(_, channels)| channels.len())
                .sum::<usize>();
            entries.push(TvEntry {
                id: "regions".to_string(),
                title: i18n::tr_tv("tv.regions"),
                description: Some({
                    let region_count = self.regions.len().to_string();
                    let channel_count = channel_count_description(channel_count);
                    i18n::tr_tv_f(
                        "tv.regions_summary",
                        &[("regions", &region_count), ("channels", &channel_count)],
                    )
                }),
                kind: TvEntryKind::Page(TvPage::Regions),
            });
        }
        entries.push(TvEntry {
            id: "recordings".to_string(),
            title: i18n::tr_tv("tv.recordings"),
            description: Some(i18n::tr_tv("tv.recordings_description")),
            kind: TvEntryKind::Recordings,
        });
        entries
    }

    fn region_entries(&self) -> Vec<TvEntry> {
        self.regions
            .iter()
            .enumerate()
            .map(|(index, (name, channels))| TvEntry {
                id: format!("region:{index}"),
                title: name.clone(),
                description: Some(channel_count_description(channels.len())),
                kind: TvEntryKind::Page(TvPage::Region(name.clone())),
            })
            .collect()
    }

    fn channel_entries(&self, indices: &[usize]) -> Vec<TvEntry> {
        indices
            .iter()
            .filter_map(|index| {
                let channel = self.channels.get(*index)?;
                let description = if channel.tvg_name.trim().is_empty()
                    || channel.tvg_name.trim().eq_ignore_ascii_case(&channel.name)
                {
                    Some(channel.category.clone())
                } else {
                    Some(format!("{}; {}", channel.tvg_name.trim(), channel.category))
                };
                Some(TvEntry {
                    id: format!("channel:{index}"),
                    title: self.channel_accessible_title(channel),
                    description,
                    kind: TvEntryKind::Channel(*index),
                })
            })
            .collect()
    }

    fn favorite_entries(&self) -> Vec<TvEntry> {
        load_settings()
            .tv_favorites
            .into_iter()
            .enumerate()
            .map(|(index, favorite)| {
                let channel = channel_from_favorite(&favorite, &self.channels);
                TvEntry {
                    id: format!("favorite:{index}"),
                    title: self.channel_accessible_title(&channel),
                    description: Some(favorite.category.clone()),
                    kind: TvEntryKind::Favorite(Box::new(favorite)),
                }
            })
            .collect()
    }

    fn channel_accessible_title(&self, channel: &TvChannel) -> String {
        let current_programs = self
            .current_programs
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        tv::current_program_for_channel(&current_programs, channel)
            .map(|program| {
                i18n::tr_tv_f(
                    "tv.now_playing",
                    &[("channel", &channel.name), ("program", &program.title)],
                )
            })
            .unwrap_or_else(|| channel.name.clone())
    }
}

pub fn open(parent: HWND) {
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    if language != Language::Italian {
        return;
    }
    let access_title = i18n::tr_tv("tv.access_title");
    if !crate::app_windows::rai_audiodescrizioni_window::ensure_rai_luce_access_with_title(
        parent,
        language,
        Some(access_title.as_str()),
    ) {
        return;
    }

    crate::screen_reader_speak(&i18n::tr_tv("tv.loading_channels"));
    let load_result = match tv::load_channels_with_cache() {
        Ok(result) => result,
        Err(err) => {
            show_error(parent, language, &err);
            return;
        }
    };
    if load_result.channels.is_empty() {
        show_error(parent, language, &i18n::tr_tv("tv.no_channels"));
        return;
    }
    if let Some(warning) = load_result.cache_warning.as_deref() {
        crate::screen_reader_speak(warning);
    }

    let catalog = Arc::new(TvCatalog::new(load_result.channels));
    browse_catalog(parent, language, catalog);
}

fn browse_catalog(parent: HWND, language: Language, catalog: Arc<TvCatalog>) {
    let mut page = TvPage::Root;
    let mut selected_id = None;
    let mut history = Vec::<(TvPage, Option<String>)>::new();
    let mut search_query = String::new();

    loop {
        if let Err(err) = catalog.load_current_programs_for_page(&page) {
            crate::log_debug(&format!(
                "TV: programmi correnti non disponibili per {page:?}, mostro comunque i canali: {err}"
            ));
        }
        let entries = catalog.entries_for_page(&page);
        if entries.is_empty() {
            match &page {
                TvPage::Search(query) => show_error(
                    parent,
                    language,
                    &i18n::tr_tv_f("tv.search_no_results", &[("query", query)]),
                ),
                _ => show_error(parent, language, &i18n::tr_tv("tv.empty_section")),
            }
            if let Some((previous_page, previous_selected_id)) = history.pop() {
                page = previous_page;
                selected_id = previous_selected_id;
                search_query.clear();
                continue;
            }
            return;
        }

        let selection_items = entries
            .iter()
            .map(|entry| MultilineSelectionItem {
                id: entry.id.clone(),
                title: entry.title.clone(),
                description: entry.description.clone(),
            })
            .collect::<Vec<_>>();

        let selection = youtube_transcript_window::select_multiline_items_with_search(
            parent,
            language,
            page_title(&page),
            selection_items,
            selected_id.clone(),
            MultilineSearchOptions {
                initial_query: search_query.clone(),
                search_button_label: i18n::tr_tv("tv.search_button"),
                show_search_edit: true,
                secondary_action_label: None,
                context_actions: tv_context_actions(Arc::clone(&catalog)),
                // Freccia destra apre sia le pagine sia il canale selezionato.
                // Dopo l'apertura di una pagina aspettiamo il rilascio del tasto
                // prima di mostrare la lista successiva, così la stessa pressione
                // non può propagarsi fino al primo canale.
                right_arrow_accepts_selection: true,
                // Nelle sottopagine Freccia sinistra torna alla pagina TV
                // precedente. Nella radice non deve mai cadere nell'editor:
                // per chiudere l'intera funzione resta disponibile Esc.
                left_arrow_closes: page != TvPage::Root,
                escape_stops_active_player: true,
                refresh: None,
            },
        );

        match take_tv_deferred_action() {
            Some(TvDeferredAction::Refresh) => {
                selected_id = None;
                search_query.clear();
                continue;
            }
            Some(TvDeferredAction::Record {
                channel,
                selected_id: recorded_id,
            }) => {
                // Keep the TV browser flow alive exactly like normal playback.
                // The next loop iteration recreates the same list, selects the
                // recorded channel and hides it behind mpv. Esc can therefore
                // stop playback and return directly to the channel list.
                selected_id = Some(recorded_id);
                record_tv_channel(parent, language, channel.as_ref());
                continue;
            }
            Some(TvDeferredAction::Guide {
                channel,
                selected_id: guide_id,
            }) => {
                selected_id = Some(guide_id);
                tv_guide_window::open(parent, *channel);
                continue;
            }
            Some(TvDeferredAction::Schedule {
                channel,
                selected_id: schedule_id,
            }) => {
                selected_id = Some(schedule_id);
                scheduled_recording_window::open_for_tv(parent, *channel);
                continue;
            }
            None => {}
        }

        let selected_entry_id = match selection {
            MultilineSelectionResult::Selected(id) => id,
            MultilineSelectionResult::Search(query) => {
                let query = query.trim();
                if query.is_empty() {
                    continue;
                }
                history.push((page.clone(), selected_id.clone()));
                page = TvPage::Search(query.to_string());
                selected_id = None;
                search_query = query.to_string();
                continue;
            }
            MultilineSelectionResult::SecondaryAction => {
                continue;
            }
            MultilineSelectionResult::Cancelled => {
                if let Some((previous_page, previous_selected_id)) = history.pop() {
                    page = previous_page;
                    selected_id = previous_selected_id;
                    search_query.clear();
                    continue;
                }
                return;
            }
        };

        let Some(entry) = entries
            .into_iter()
            .find(|entry| entry.id == selected_entry_id)
        else {
            show_error(parent, language, &i18n::tr_tv("tv.open_selected_error"));
            continue;
        };

        selected_id = Some(entry.id.clone());
        search_query.clear();
        match entry.kind {
            TvEntryKind::Page(next_page) => {
                history.push((page, selected_id.clone()));
                page = next_page;
                selected_id = None;
                wait_for_tv_navigation_accept_keys_release();
            }
            TvEntryKind::Channel(index) => {
                let Some(channel) = catalog.channels.get(index) else {
                    show_error(parent, language, &i18n::tr_tv("tv.channel_unavailable"));
                    continue;
                };
                play_channel(parent, language, channel);
            }
            TvEntryKind::Favorite(favorite) => {
                let channel = channel_from_favorite(favorite.as_ref(), &catalog.channels);
                play_channel(parent, language, &channel);
            }
            TvEntryKind::Recordings => {
                stream_recording::open_recordings(
                    parent,
                    parent,
                    language,
                    StreamRecordingKind::Tv,
                );
            }
        }
    }
}

fn favorite_from_channel(channel: &TvChannel) -> TvFavorite {
    TvFavorite {
        name: channel.name.clone(),
        url: channel.url.clone(),
        dash_url: channel.dash_url.clone(),
        category: channel.category.clone(),
        stream_resolver: channel.stream_resolver.clone(),
        resolver_endpoint: channel.resolver_endpoint.clone(),
        resolver_realm: channel.resolver_realm.clone(),
        resolver_channel_id: channel.resolver_channel_id.clone(),
        tvg_id: channel.tvg_id.clone(),
        tvg_name: channel.tvg_name.clone(),
        http_user_agent: channel.http_user_agent.clone(),
    }
}

fn channel_from_favorite(favorite: &TvFavorite, channels: &[TvChannel]) -> TvChannel {
    if favorite.url.trim().is_empty()
        && let Some(channel) = channels
            .iter()
            .find(|c| c.name.trim().eq_ignore_ascii_case(favorite.name.trim()))
    {
        return channel.clone();
    }
    TvChannel {
        name: favorite.name.clone(),
        url: favorite.url.clone(),
        dash_url: favorite.dash_url.clone(),
        category: favorite.category.clone(),
        stream_resolver: favorite.stream_resolver.clone(),
        resolver_endpoint: favorite.resolver_endpoint.clone(),
        resolver_realm: favorite.resolver_realm.clone(),
        resolver_channel_id: favorite.resolver_channel_id.clone(),
        tvg_id: favorite.tvg_id.clone(),
        tvg_name: favorite.tvg_name.clone(),
        http_user_agent: favorite.http_user_agent.clone(),
    }
}

fn selected_channel_from_id(catalog: &TvCatalog, id: &str) -> Option<TvChannel> {
    if let Some(raw_index) = id.strip_prefix("channel:")
        && let Ok(index) = raw_index.parse::<usize>()
    {
        return catalog.channels.get(index).cloned();
    }
    if let Some(raw_index) = id.strip_prefix("favorite:")
        && let Ok(index) = raw_index.parse::<usize>()
    {
        return load_settings()
            .tv_favorites
            .get(index)
            .map(|favorite| channel_from_favorite(favorite, &catalog.channels));
    }
    None
}

fn tv_favorite_matches_channel(favorite: &TvFavorite, channel: &TvChannel) -> bool {
    favorite.name.eq_ignore_ascii_case(&channel.name)
        && (favorite.url.trim().is_empty()
            || favorite.url == channel.url
            || (!favorite.tvg_id.trim().is_empty()
                && favorite.tvg_id.eq_ignore_ascii_case(&channel.tvg_id)))
}

fn tv_context_actions(catalog: Arc<TvCatalog>) -> Vec<InterpreterContextAction> {
    let add_catalog = Arc::clone(&catalog);
    let add_enabled = Arc::new(move |id: &str| {
        let Some(channel) = selected_channel_from_id(&add_catalog, id) else {
            return false;
        };
        !load_settings()
            .tv_favorites
            .iter()
            .any(|favorite| tv_favorite_matches_channel(favorite, &channel))
    });
    let add_catalog_handler = Arc::clone(&catalog);
    let add_handler = Arc::new(move |id: String| {
        let Some(channel) = selected_channel_from_id(&add_catalog_handler, &id) else {
            return;
        };
        let mut settings = load_settings();
        if !settings
            .tv_favorites
            .iter()
            .any(|favorite| tv_favorite_matches_channel(favorite, &channel))
        {
            settings.tv_favorites.push(favorite_from_channel(&channel));
            settings.tv_favorites.sort_by(|a, b| {
                a.name
                    .to_lowercase()
                    .cmp(&b.name.to_lowercase())
                    .then_with(|| a.url.cmp(&b.url))
            });
            save_settings(settings);
            crate::screen_reader_speak(&i18n::tr_tv_f(
                "tv.favorite_added",
                &[("channel", &channel.name)],
            ));
            set_tv_deferred_action(TvDeferredAction::Refresh);
            close_tv_browser_dialog();
        }
    });

    let remove_catalog = Arc::clone(&catalog);
    let remove_enabled = Arc::new(move |id: &str| {
        let Some(channel) = selected_channel_from_id(&remove_catalog, id) else {
            return false;
        };
        load_settings()
            .tv_favorites
            .iter()
            .any(|favorite| tv_favorite_matches_channel(favorite, &channel))
    });
    let remove_catalog_handler = Arc::clone(&catalog);
    let remove_handler = Arc::new(move |id: String| {
        let Some(channel) = selected_channel_from_id(&remove_catalog_handler, &id) else {
            return;
        };
        let mut settings = load_settings();
        let before = settings.tv_favorites.len();
        settings
            .tv_favorites
            .retain(|favorite| !tv_favorite_matches_channel(favorite, &channel));
        if settings.tv_favorites.len() != before {
            save_settings(settings);
            crate::screen_reader_speak(&i18n::tr_tv_f(
                "tv.favorite_removed",
                &[("channel", &channel.name)],
            ));
            set_tv_deferred_action(TvDeferredAction::Refresh);
            close_tv_browser_dialog();
        }
    });

    let record_catalog = Arc::clone(&catalog);
    let record_enabled =
        Arc::new(move |id: &str| selected_channel_from_id(&record_catalog, id).is_some());
    let record_catalog_handler = Arc::clone(&catalog);
    let record_handler = Arc::new(move |id: String| {
        let Some(channel) = selected_channel_from_id(&record_catalog_handler, &id) else {
            return;
        };
        set_tv_deferred_action(TvDeferredAction::Record {
            channel: Box::new(channel),
            selected_id: id,
        });
        close_tv_browser_dialog();
    });

    let guide_catalog = Arc::clone(&catalog);
    let guide_enabled = Arc::new(move |id: &str| {
        let Some(channel) = selected_channel_from_id(&guide_catalog, id) else {
            return false;
        };
        let current_programs = guide_catalog
            .current_programs
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        tv::current_program_for_channel(&current_programs, &channel).is_some()
    });
    let guide_catalog_handler = Arc::clone(&catalog);
    let guide_handler = Arc::new(move |id: String| {
        let Some(channel) = selected_channel_from_id(&guide_catalog_handler, &id) else {
            return;
        };
        set_tv_deferred_action(TvDeferredAction::Guide {
            channel: Box::new(channel),
            selected_id: id,
        });
        close_tv_browser_dialog();
    });

    let schedule_catalog = Arc::clone(&catalog);
    let schedule_enabled =
        Arc::new(move |id: &str| selected_channel_from_id(&schedule_catalog, id).is_some());
    let schedule_catalog_handler = Arc::clone(&catalog);
    let schedule_handler = Arc::new(move |id: String| {
        let Some(channel) = selected_channel_from_id(&schedule_catalog_handler, &id) else {
            return;
        };
        set_tv_deferred_action(TvDeferredAction::Schedule {
            channel: Box::new(channel),
            selected_id: id,
        });
        close_tv_browser_dialog();
    });

    vec![
        InterpreterContextAction {
            label: i18n::tr_tv("tv.action.add_favorite"),
            ctrl_c_shortcut: false,
            delete_shortcut: false,
            children: Vec::new(),
            enabled: add_enabled,
            handler: add_handler,
        },
        InterpreterContextAction {
            label: i18n::tr_tv("tv.action.remove_favorite"),
            ctrl_c_shortcut: false,
            delete_shortcut: false,
            children: Vec::new(),
            enabled: remove_enabled,
            handler: remove_handler,
        },
        InterpreterContextAction {
            label: i18n::tr_tv("tv.action.record_and_play"),
            ctrl_c_shortcut: false,
            delete_shortcut: false,
            children: Vec::new(),
            enabled: record_enabled,
            handler: record_handler,
        },
        InterpreterContextAction {
            label: i18n::tr_tv("tv.action.show_guide"),
            ctrl_c_shortcut: false,
            delete_shortcut: false,
            children: Vec::new(),
            enabled: guide_enabled,
            handler: guide_handler,
        },
        InterpreterContextAction {
            label: i18n::tr_tv("tv.action.schedule_recording"),
            ctrl_c_shortcut: false,
            delete_shortcut: false,
            children: Vec::new(),
            enabled: schedule_enabled,
            handler: schedule_handler,
        },
    ]
}

fn record_tv_channel(parent: HWND, language: Language, channel: &TvChannel) {
    crate::screen_reader_speak(&i18n::tr_tv_f(
        "tv.recording_starting",
        &[("channel", &channel.name)],
    ));
    let resolved_url = match tv::resolve_stream_url(channel) {
        Ok(url) if !url.trim().is_empty() => url,
        Ok(_) => {
            show_error(parent, language, &i18n::tr_tv("tv.stream_empty"));
            return;
        }
        Err(err) => {
            show_error(
                parent,
                language,
                &i18n::tr_tv_f(
                    "tv.record_error",
                    &[("channel", &channel.name), ("error", &err)],
                ),
            );
            return;
        }
    };

    match stream_recording::start_tv_recording_and_playback(
        parent,
        &resolved_url,
        &channel.name,
        channel.media_playback_user_agent(),
        tv::is_rai_audio_description_channel(channel),
    ) {
        Ok(path) => {
            let path_text = path.display().to_string();
            crate::screen_reader_speak(&i18n::tr_tv_f(
                "tv.recording_started",
                &[("channel", &channel.name), ("path", &path_text)],
            ));
        }
        Err(err) => show_error(parent, language, &err),
    }
}

fn play_channel(parent: HWND, language: Language, channel: &TvChannel) {
    crate::screen_reader_speak(&i18n::tr_tv_f(
        "tv.playback_starting",
        &[("channel", &channel.name)],
    ));
    let resolved_url = match tv::resolve_stream_url(channel) {
        Ok(url) if !url.trim().is_empty() => url,
        Ok(_) => {
            show_error(parent, language, &i18n::tr_tv("tv.stream_empty"));
            return;
        }
        Err(err) => {
            show_error(
                parent,
                language,
                &i18n::tr_tv_f(
                    "tv.start_error",
                    &[("channel", &channel.name), ("error", &err)],
                ),
            );
            return;
        }
    };

    crate::log_debug(&format!(
        "TV playback start: name={} category={} resolver={} user_agent={}",
        channel.name,
        channel.category,
        channel.stream_resolver.as_deref().unwrap_or("direct"),
        channel.playback_user_agent()
    ));
    if let Err(err) = crate::launch_tv_stream_in_mpv(
        parent,
        &resolved_url,
        &channel.name,
        channel.media_playback_user_agent(),
        tv::is_rai_audio_description_channel(channel),
    ) {
        show_error(
            parent,
            language,
            &i18n::tr_tv_f(
                "tv.play_error",
                &[("channel", &channel.name), ("error", &err)],
            ),
        );
    }
}

fn page_title(page: &TvPage) -> String {
    match page {
        TvPage::Root => i18n::tr_tv("tv.page.root"),
        TvPage::Category(category) => i18n::tr_tv_f("tv.page.category", &[("category", category)]),
        TvPage::Regions => i18n::tr_tv("tv.page.regions"),
        TvPage::Region(region) => i18n::tr_tv_f("tv.page.region", &[("region", region)]),
        TvPage::Favorites => i18n::tr_tv("tv.page.favorites"),
        TvPage::Search(query) => i18n::tr_tv_f("tv.page.search", &[("query", query)]),
    }
}

fn push_group_index(groups: &mut Vec<(String, Vec<usize>)>, name: &str, index: usize) {
    if let Some((_, indices)) = groups.iter_mut().find(|(existing, _)| existing == name) {
        indices.push(index);
    } else {
        groups.push((name.to_string(), vec![index]));
    }
}

fn channel_count_description(count: usize) -> String {
    if count == 1 {
        i18n::tr_tv("tv.channel_count.one")
    } else {
        let count_text = count.to_string();
        i18n::tr_tv_f("tv.channel_count.many", &[("count", &count_text)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(name: &str, category: &str) -> TvChannel {
        TvChannel {
            name: name.to_string(),
            url: "https://example.test/live.m3u8".to_string(),
            dash_url: None,
            category: category.to_string(),
            stream_resolver: None,
            resolver_endpoint: None,
            resolver_realm: None,
            resolver_channel_id: None,
            tvg_id: String::new(),
            tvg_name: String::new(),
            http_user_agent: String::new(),
        }
    }

    #[test]
    fn catalog_groups_regional_channels_separately() {
        let catalog = TvCatalog::new(vec![
            channel("Rai 1", "Rai"),
            channel("TGR Piemonte", "Regionali - Piemonte"),
        ]);
        assert_eq!(catalog.categories.len(), 1);
        assert_eq!(catalog.regions.len(), 1);
        assert_eq!(catalog.regions[0].0, "Piemonte");
    }
}
