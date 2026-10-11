use base64::Engine;
use quick_xml::{Reader, events::Event};
use serde_json::Value;
use std::collections::HashSet;

const RAIPLAY_BASE_URL_B64: &str = "BT9NUQVqHVc7T1RfJlUTPlshC3lYBw==";
const RAIPLAY_MENU_URL_B64: &str = "BT9NUQVqHVc7T1RfJlUTPlshC3lYB3ZbAShFRiBAGiw=";
const RAIPLAY_SEARCH_URL_B64: &str =
    "BT9NUQVqHVc7T1RfJlUTPlshC3lYB3ZXECldCT5aFm0INBs8XSMQXz4lHS4OIxRSEyJEES9dDBAkXVU4Bm8fJFQSK1UM";
const RAIPLAY_MENU_SECTION_SOURCE_PREFIX: &str = "raiplay-menu-section:";
const RAIPLAY_SEARCH_SOURCE_PREFIX: &str = "raiplay-search:";
const RAIPLAY_ROOT_SOURCE: &str = "raiplay-root";
const RAIPLAY_SEARCH_TEMPLATE_IN: &str = "6470a982e4e0301afe1f81f1";
const RAIPLAY_SEARCH_TEMPLATE_OUT: &str = "6516ac5d40da6c377b151642";
const RAIPLAY_SEARCH_PAGE_SIZE: u32 = 12;
pub(crate) const DRM_NOT_SUPPORTED_ERROR: &str = "raiplay.drm_not_supported";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BrowseItemKind {
    Page,
    Media,
}

#[derive(Clone, Debug)]
pub(crate) struct BrowseItem {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) program_title: Option<String>,
    pub(crate) path_id: Option<String>,
    pub(crate) media_url: Option<String>,
    pub(crate) kind: BrowseItemKind,
}

#[derive(Clone, Debug)]
pub(crate) struct BrowsePage {
    pub(crate) source: String,
    pub(crate) title: String,
    pub(crate) items: Vec<BrowseItem>,
}

#[derive(Clone, Debug)]
pub(crate) struct LiveAudioTrack {
    pub(crate) info: crate::ffmpeg_source::AudioStreamInfo,
    pub(crate) url: String,
}

#[derive(Clone, Debug)]
pub(crate) enum PlaybackTarget {
    DirectStream {
        url: String,
        media_url: String,
        is_live: bool,
        live_audio_tracks: Vec<LiveAudioTrack>,
    },
    Download(String),
}

pub(crate) fn load_root_page() -> Result<BrowsePage, String> {
    let root = fetch_json(&raiplay_menu_url()?)?;
    let sections = root
        .get("menuv4")
        .and_then(Value::as_array)
        .or_else(|| root.get("menuv3").and_then(Value::as_array))
        .ok_or_else(|| "Menu RaiPlay non disponibile.".to_string())?;

    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for section in sections {
        if let Some(item) = parse_root_section(section)?
            && seen.insert(item.id.clone())
        {
            items.push(item);
        }
    }

    Ok(BrowsePage {
        source: RAIPLAY_ROOT_SOURCE.to_string(),
        title: "RaiPlay".to_string(),
        items,
    })
}

pub(crate) fn load_page(path_or_url: &str) -> Result<BrowsePage, String> {
    if path_or_url == RAIPLAY_ROOT_SOURCE {
        return load_root_page();
    }
    if let Some(query) = path_or_url.strip_prefix(RAIPLAY_SEARCH_SOURCE_PREFIX) {
        return search(query);
    }
    if let Some(section_name) = path_or_url.strip_prefix(RAIPLAY_MENU_SECTION_SOURCE_PREFIX) {
        return load_menu_section_page(section_name);
    }
    load_page_from_url(&absolute_url(path_or_url)?)
}

pub(crate) fn search(query: &str) -> Result<BrowsePage, String> {
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return Err("Inserisci un testo da cercare in RaiPlay.".to_string());
    }

    let body = serde_json::json!({
        "templateIn": RAIPLAY_SEARCH_TEMPLATE_IN,
        "templateOut": RAIPLAY_SEARCH_TEMPLATE_OUT,
        "params": {
            "param": normalize_search_text(trimmed_query),
            "from": 0,
            "sort": "relevance",
            "size": RAIPLAY_SEARCH_PAGE_SIZE,
            "additionalSize": RAIPLAY_SEARCH_PAGE_SIZE,
            "onlyVideoQuery": false,
            "onlyProgramsQuery": false,
        }
    });
    let body_text = serde_json::to_string(&body)
        .map_err(|err| format!("Richiesta ricerca RaiPlay non valida: {err}"))?;
    let bytes = crate::curl_client::CurlClient::post_form_impersonated(
        &raiplay_search_url()?,
        &body_text,
        &["Content-Type: application/json"],
    )
    .map_err(|err| format!("Impossibile cercare in RaiPlay: {err}"))?;
    let root: Value = serde_json::from_slice(&bytes)
        .map_err(|err| format!("Risposta ricerca RaiPlay non valida: {err}"))?;

    let mut items = Vec::new();
    let mut seen = HashSet::new();

    if let Some(cards) = root
        .get("agg")
        .and_then(|agg| agg.get("titoli"))
        .and_then(|titles| titles.get("cards"))
        .and_then(Value::as_array)
    {
        collect_cards(cards, &mut seen, &mut items)?;
    }
    if let Some(cards) = root
        .get("agg")
        .and_then(|agg| agg.get("video"))
        .and_then(|video| video.get("cards"))
        .and_then(Value::as_array)
    {
        collect_cards(cards, &mut seen, &mut items)?;
    }

    Ok(BrowsePage {
        source: format!("{RAIPLAY_SEARCH_SOURCE_PREFIX}{trimmed_query}"),
        title: format!("Risultati per {trimmed_query}"),
        items,
    })
}

pub(crate) fn resolve_playback_target(media_url: &str) -> Result<PlaybackTarget, String> {
    let trimmed = media_url.trim();
    if trimmed.is_empty() {
        return Err("Il contenuto selezionato non ha un URL media disponibile.".to_string());
    }

    let (resolved_url, is_live) = if trimmed.contains("/relinker/relinkerServlet") {
        resolve_relinker_content_url(trimmed)?
    } else {
        (trimmed.to_string(), false)
    };

    if is_drm_protected_raiplay_url(&resolved_url) {
        return Err(DRM_NOT_SUPPORTED_ERROR.to_string());
    }

    if is_hls_url(&resolved_url) {
        let live_audio_tracks = if is_live {
            resolve_hls_live_audio_tracks(&resolved_url)
        } else {
            Vec::new()
        };
        let audio_only_url = live_audio_tracks
            .iter()
            .find(|track| track.info.is_default)
            .map(|track| track.url.clone())
            .or_else(|| resolve_hls_audio_only_url(&resolved_url))
            .unwrap_or_else(|| resolved_url.clone());
        crate::log_debug(&format!(
            "RaiPlay playback target resolved: source={} resolved={} audio_only={} is_live={} live_tracks={}",
            trimmed,
            resolved_url,
            audio_only_url,
            is_live,
            live_audio_tracks.len()
        ));
        Ok(PlaybackTarget::DirectStream {
            url: audio_only_url,
            media_url: resolved_url,
            is_live,
            live_audio_tracks,
        })
    } else {
        crate::log_debug(&format!(
            "RaiPlay playback target resolved: source={} resolved={}",
            trimmed, resolved_url
        ));
        Ok(PlaybackTarget::Download(resolved_url))
    }
}

fn load_menu_section_page(section_name: &str) -> Result<BrowsePage, String> {
    let root = fetch_json(&raiplay_menu_url()?)?;
    let sections = root
        .get("menuv4")
        .and_then(Value::as_array)
        .or_else(|| root.get("menuv3").and_then(Value::as_array))
        .ok_or_else(|| "Menu RaiPlay non disponibile.".to_string())?;
    let section = sections
        .iter()
        .find(|entry| {
            string_field(entry, "name")
                .map(|name| name.eq_ignore_ascii_case(section_name))
                .unwrap_or(false)
        })
        .ok_or_else(|| "Sezione RaiPlay non trovata.".to_string())?;

    let title = string_field(section, "title")
        .or_else(|| string_field(section, "name"))
        .unwrap_or_else(|| "RaiPlay".to_string());
    let elements = section
        .get("elements")
        .and_then(Value::as_array)
        .ok_or_else(|| "Sezione RaiPlay non disponibile.".to_string())?;

    let mut items = Vec::new();
    let mut seen = HashSet::new();
    collect_cards(elements, &mut seen, &mut items)?;

    Ok(BrowsePage {
        source: format!("{RAIPLAY_MENU_SECTION_SOURCE_PREFIX}{section_name}"),
        title,
        items,
    })
}

fn load_page_from_url(url: &str) -> Result<BrowsePage, String> {
    let root = fetch_json(url)?;
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    collect_nested_items(&root, &mut seen, &mut items)?;
    if items.is_empty()
        && let Some(item) = parse_card(&root)?
    {
        items.push(item);
    }

    Ok(BrowsePage {
        source: url.to_string(),
        title: page_title(&root),
        items,
    })
}

fn collect_nested_items(
    value: &Value,
    seen: &mut HashSet<String>,
    items: &mut Vec<BrowseItem>,
) -> Result<(), String> {
    match value {
        Value::Array(array) => {
            for entry in array {
                collect_entry(entry, seen, items)?;
            }
        }
        Value::Object(map) => {
            for key in ["items", "contents", "blocks", "sets", "elements"] {
                if let Some(array) = map.get(key).and_then(Value::as_array) {
                    for entry in array {
                        collect_entry(entry, seen, items)?;
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn collect_entry(
    entry: &Value,
    seen: &mut HashSet<String>,
    items: &mut Vec<BrowseItem>,
) -> Result<(), String> {
    if let Some(item) = parse_card(entry)?
        && seen.insert(item.id.clone())
    {
        items.push(item);
    }
    collect_nested_items(entry, seen, items)
}

fn collect_cards(
    cards: &[Value],
    seen: &mut HashSet<String>,
    items: &mut Vec<BrowseItem>,
) -> Result<(), String> {
    for card in cards {
        if let Some(item) = parse_card(card)?
            && seen.insert(item.id.clone())
        {
            items.push(item);
        }
    }
    Ok(())
}

// Nasconde solo la voce delle trasmissioni in diretta dal menu di RaiPlay.
// I contenuti on demand e il menu TV indipendente di Sonarpad restano invariati.
fn is_hidden_raiplay_live_section(label: &str) -> bool {
    matches!(
        label.trim().to_ascii_lowercase().as_str(),
        "dirette" | "diretta" | "dirette tv" | "tv in diretta" | "live"
    )
}

fn parse_root_section(section: &Value) -> Result<Option<BrowseItem>, String> {
    let raw_title = match string_field(section, "name") {
        Some(value) if !value.is_empty() => value,
        _ => return Ok(None),
    };
    if raw_title.eq_ignore_ascii_case("Altro")
        || is_hidden_raiplay_live_section(&raw_title)
        || string_field(section, "title")
            .as_deref()
            .is_some_and(is_hidden_raiplay_live_section)
    {
        return Ok(None);
    }
    let title = if raw_title.eq_ignore_ascii_case("Cerca") {
        "Esplora".to_string()
    } else {
        raw_title
    };
    if section
        .get("elements")
        .and_then(Value::as_array)
        .map(|elements| !elements.is_empty())
        .unwrap_or(false)
    {
        let section_source = format!("{RAIPLAY_MENU_SECTION_SOURCE_PREFIX}{}", title.trim());
        return Ok(Some(BrowseItem {
            id: format!("page|root|{title}"),
            title,
            description: string_field(section, "title")
                .or_else(|| string_field(section, "menu_type")),
            program_title: None,
            path_id: Some(section_source),
            media_url: None,
            kind: BrowseItemKind::Page,
        }));
    }
    if let Some(path_id) = string_field(section, "path_id")
        && is_supported_internal_target(&path_id)
    {
        return Ok(Some(BrowseItem {
            id: format!("page|{}", path_id.trim()),
            title,
            description: string_field(section, "menu_type"),
            program_title: None,
            path_id: Some(absolute_url(&path_id)?),
            media_url: None,
            kind: BrowseItemKind::Page,
        }));
    }
    Ok(None)
}

fn parse_card(card: &Value) -> Result<Option<BrowseItem>, String> {
    if card.get("action").is_some() {
        return Ok(None);
    }
    if card
        .get("type")
        .and_then(Value::as_str)
        .map(|value| matches!(value, "label" | "placeholder"))
        .unwrap_or(false)
    {
        return Ok(None);
    }
    if string_field(card, "menu_type")
        .map(|value| value.eq_ignore_ascii_case("RaiPlay Separatore Nav"))
        .unwrap_or(false)
    {
        return Ok(None);
    }

    let media_url = card
        .get("video")
        .and_then(|video| string_field(video, "content_url"))
        .or_else(|| string_field(card, "video_url"));
    let path_id = string_field(card, "path_id")
        .filter(|value| is_supported_internal_target(value))
        .or_else(|| string_field(card, "url").and_then(|value| html_url_to_json_path(&value)));

    let kind = if media_url.is_some() {
        BrowseItemKind::Media
    } else if path_id.is_some() {
        BrowseItemKind::Page
    } else {
        return Ok(None);
    };

    let title = preferred_title(card)?;
    let description = preferred_description(card);
    let program_title = preferred_program_title(card);
    let id = match kind {
        BrowseItemKind::Media => {
            format!(
                "media|{}|{}",
                media_url.clone().unwrap_or_default(),
                path_id.clone().unwrap_or_default()
            )
        }
        BrowseItemKind::Page => format!("page|{}", path_id.clone().unwrap_or_default()),
    };

    Ok(Some(BrowseItem {
        id,
        title,
        description,
        program_title,
        path_id: path_id.map(|value| absolute_url(&value)).transpose()?,
        media_url,
        kind,
    }))
}

fn preferred_title(card: &Value) -> Result<String, String> {
    for key in [
        "titolo",
        "episode_title",
        "toptitle",
        "title",
        "name",
        "label",
        "programma",
        "program_name",
    ] {
        if let Some(value) = string_field(card, key).filter(|value| !value.is_empty()) {
            return Ok(value);
        }
    }
    Err("Elemento RaiPlay senza titolo.".to_string())
}

fn preferred_description(card: &Value) -> Option<String> {
    for key in [
        "sommario",
        "description",
        "vanity",
        "caption",
        "subtitle",
        "duration_in_minutes",
        "menu_type",
    ] {
        if let Some(value) = string_field(card, key).filter(|value| !value.is_empty()) {
            return Some(value);
        }
    }
    None
}

fn preferred_program_title(card: &Value) -> Option<String> {
    for key in ["program_name", "programma"] {
        if let Some(value) = string_field(card, key).filter(|value| !value.is_empty()) {
            return Some(value);
        }
    }
    None
}

fn page_title(root: &Value) -> String {
    for key in ["name", "title", "label"] {
        if let Some(value) = string_field(root, key).filter(|value| !value.is_empty()) {
            return value;
        }
    }
    "RaiPlay".to_string()
}

fn fetch_json(url: &str) -> Result<Value, String> {
    let bytes = crate::curl_client::CurlClient::fetch_url_impersonated(url)
        .map_err(|err| format!("Impossibile caricare i dati di RaiPlay: {err}"))?;
    serde_json::from_slice(&bytes).map_err(|err| format!("Risposta JSON RaiPlay non valida: {err}"))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn is_supported_internal_target(path_or_url: &str) -> bool {
    let trimmed = path_or_url.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.contains("raiplay.it")
            && (trimmed.ends_with(".json") || trimmed.contains(".json?"));
    }
    trimmed.starts_with('/') && trimmed.ends_with(".json")
}

fn html_url_to_json_path(path_or_url: &str) -> Option<String> {
    let trimmed = path_or_url.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.ends_with(".json") {
        return Some(trimmed.to_string());
    }
    if let Some(prefix) = trimmed.strip_suffix(".html") {
        return Some(format!("{prefix}.json"));
    }
    if trimmed.starts_with('/') {
        return Some(format!("{trimmed}.json"));
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let replaced = trimmed.replace(".html", ".json");
        if replaced != trimmed {
            return Some(replaced);
        }
        return Some(format!("{trimmed}.json"));
    }
    None
}

fn absolute_url(path_or_url: &str) -> Result<String, String> {
    let trimmed = path_or_url.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("{}{}", raiplay_base_url()?, trimmed))
    }
}

fn normalize_search_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn resolve_relinker_content_url(relinker_url: &str) -> Result<(String, bool), String> {
    let separator = if relinker_url.contains('?') { '&' } else { '?' };
    let xml_url = format!("{relinker_url}{separator}output=45&pl=native");
    let bytes = crate::curl_client::CurlClient::fetch_url_iphone_impersonated(&xml_url)
        .map_err(|err| format!("Impossibile risolvere il contenuto RaiPlay: {err}"))?;
    let xml = String::from_utf8(bytes)
        .map_err(|err| format!("Risposta RaiPlay non decodificabile come UTF-8: {err}"))?;
    parse_relinker_content_url(&xml)
}

fn parse_relinker_content_url(xml: &str) -> Result<(String, bool), String> {
    if let Some(content_url) = extract_xml_tag_with_attribute(xml, "url", "type", "content") {
        let is_live = extract_xml_tag_text(xml, "is_live")
            .map(|value| value.eq_ignore_ascii_case("Y"))
            .unwrap_or(false);
        return Ok((content_url, is_live));
    }

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut current_tag = Vec::new();
    let mut content_url = None;
    let mut is_live = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                current_tag.clear();
                current_tag.extend_from_slice(event.name().as_ref());
                if current_tag.as_slice() == b"is_live" {
                    match reader.read_event() {
                        Ok(Event::Text(text)) => {
                            let decoded = text
                                .decode()
                                .map_err(|err| {
                                    format!("Flag live RaiPlay non decodificabile: {err}")
                                })?
                                .into_owned();
                            is_live = decoded.trim().eq_ignore_ascii_case("Y");
                        }
                        Ok(Event::End(_)) => {}
                        Ok(_) => {}
                        Err(err) => {
                            return Err(format!("XML contenuto RaiPlay non valido: {err}"));
                        }
                    }
                } else if current_tag.as_slice() == b"url" {
                    let mut is_content_url = false;
                    for attribute in event.attributes().flatten() {
                        if attribute.key.as_ref() == b"type"
                            && attribute
                                .decode_and_unescape_value(reader.decoder())
                                .map(|value| value == "content")
                                .unwrap_or(false)
                        {
                            is_content_url = true;
                            break;
                        }
                    }
                    if is_content_url {
                        match reader.read_event() {
                            Ok(Event::Text(text)) => {
                                let decoded = text
                                    .decode()
                                    .map_err(|err| {
                                        format!("URL contenuto RaiPlay non decodificabile: {err}")
                                    })?
                                    .into_owned();
                                if !decoded.trim().is_empty() {
                                    content_url = Some(decoded);
                                }
                            }
                            Ok(Event::End(_)) => {}
                            Ok(_) => {}
                            Err(err) => {
                                return Err(format!("XML contenuto RaiPlay non valido: {err}"));
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => return Err(format!("XML contenuto RaiPlay non valido: {err}")),
        }
    }

    let content_url =
        content_url.ok_or_else(|| "URL contenuto RaiPlay non disponibile.".to_string())?;
    Ok((content_url, is_live))
}

fn extract_xml_tag_text(xml: &str, tag_name: &str) -> Option<String> {
    let start_tag = format!("<{tag_name}>");
    let end_tag = format!("</{tag_name}>");
    let start = xml.find(&start_tag)? + start_tag.len();
    let end = xml[start..].find(&end_tag)? + start;
    let value = xml[start..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn extract_xml_tag_with_attribute(
    xml: &str,
    tag_name: &str,
    attribute_name: &str,
    attribute_value: &str,
) -> Option<String> {
    let start_tag_prefix = format!("<{tag_name}");
    let end_tag = format!("</{tag_name}>");
    let mut search_offset = 0usize;

    while let Some(relative_start) = xml[search_offset..].find(&start_tag_prefix) {
        let start = search_offset + relative_start;
        let tag_end = xml[start..].find('>')? + start;
        let tag_content = &xml[start..=tag_end];
        let expected_attribute = format!(r#"{attribute_name}="{attribute_value}""#);
        if tag_content.contains(&expected_attribute) {
            let value_start = tag_end + 1;
            let value_end = xml[value_start..].find(&end_tag)? + value_start;
            let value = xml[value_start..value_end].trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        search_offset = tag_end.saturating_add(1);
    }

    None
}

fn is_hls_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.contains(".m3u8")
}

fn is_drm_protected_raiplay_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.contains("/drm_root/")
        || lower.contains("drmnagra")
        || lower.contains(".mpd")
        || lower.contains("manifest_mvnumber.mpd")
}

fn resolve_hls_audio_only_url(master_url: &str) -> Option<String> {
    resolve_hls_live_audio_tracks(master_url)
        .into_iter()
        .find(|track| track.info.is_default)
        .map(|track| track.url)
        .or_else(|| {
            resolve_hls_audio_track_urls(master_url)
                .into_iter()
                .next()
                .map(|(_, url)| url)
        })
}

fn resolve_hls_live_audio_tracks(master_url: &str) -> Vec<LiveAudioTrack> {
    let mut tracks = Vec::new();
    for (index, (attrs, url)) in resolve_hls_audio_track_urls(master_url)
        .into_iter()
        .enumerate()
    {
        let language = attrs.get("LANGUAGE").cloned();
        let title = attrs.get("NAME").cloned();
        tracks.push(LiveAudioTrack {
            info: crate::ffmpeg_source::AudioStreamInfo {
                index: index as i32,
                language,
                title,
                codec: "aac".to_string(),
                channels: 0,
                sample_rate: 0,
                is_default: false,
            },
            url,
        });
    }

    let default_index = tracks
        .iter()
        .position(|track| {
            track
                .info
                .language
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case("des"))
                .unwrap_or(false)
        })
        .or_else(|| {
            tracks.iter().position(|track| {
                track
                    .info
                    .title
                    .as_deref()
                    .map(|value| value.eq_ignore_ascii_case("Audiodescrizione"))
                    .unwrap_or(false)
            })
        })
        .or_else(|| {
            tracks.iter().position(|track| {
                track
                    .info
                    .language
                    .as_deref()
                    .map(|value| value.eq_ignore_ascii_case("ita"))
                    .unwrap_or(false)
            })
        })
        .or_else(|| tracks.first().map(|_| 0));

    if let Some(default_index) = default_index
        && let Some(track) = tracks.get_mut(default_index)
    {
        track.info.is_default = true;
    }

    tracks
}

fn resolve_hls_audio_track_urls(
    master_url: &str,
) -> Vec<(std::collections::HashMap<String, String>, String)> {
    let Ok(bytes) = crate::curl_client::CurlClient::fetch_url_impersonated(master_url) else {
        return Vec::new();
    };
    let Ok(playlist) = String::from_utf8(bytes) else {
        return Vec::new();
    };
    if !playlist.contains("#EXT-X-MEDIA:TYPE=AUDIO") {
        return Vec::new();
    }

    let mut tracks = Vec::new();

    for line in playlist.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("#EXT-X-MEDIA:") || !trimmed.contains("TYPE=AUDIO") {
            continue;
        }

        if let Some(uri) = parse_hls_attribute(trimmed, "URI") {
            let mut attrs = std::collections::HashMap::new();
            for key in ["LANGUAGE", "NAME", "DEFAULT"] {
                if let Some(value) = parse_hls_attribute(trimmed, key) {
                    attrs.insert(key.to_string(), value);
                }
            }
            tracks.push((attrs, resolve_hls_child_url(master_url, &uri)));
        }
    }

    tracks
}

fn parse_hls_attribute(line: &str, key: &str) -> Option<String> {
    let pattern = format!("{key}=\"");
    let start = line.find(&pattern)? + pattern.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn resolve_hls_child_url(master_url: &str, child_uri: &str) -> String {
    let trimmed = child_uri.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }

    let (base_without_query, query_suffix) = master_url
        .split_once('?')
        .map(|(base, query)| (base, format!("?{query}")))
        .unwrap_or((master_url, String::new()));

    let mut base_parts = base_without_query.rsplitn(2, '/');
    let _file_name = base_parts.next();
    let parent = base_parts.next().unwrap_or(base_without_query);
    if trimmed.contains('?') {
        format!("{parent}/{trimmed}")
    } else {
        format!("{parent}/{trimmed}{query_suffix}")
    }
}

fn raiplay_base_url() -> Result<String, String> {
    decode_raiplay_url(RAIPLAY_BASE_URL_B64)
}

fn raiplay_menu_url() -> Result<String, String> {
    decode_raiplay_url(RAIPLAY_MENU_URL_B64)
}

fn raiplay_search_url() -> Result<String, String> {
    decode_raiplay_url(RAIPLAY_SEARCH_URL_B64)
}

fn decode_raiplay_url(encoded: &str) -> Result<String, String> {
    let key = raiplay_obfuscated_url_key()?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|err| format!("URL RaiPlay offuscato non valido: {err}"))?;
    let decoded: Vec<u8> = bytes
        .into_iter()
        .enumerate()
        .map(|(index, byte)| byte ^ key[index % key.len()])
        .collect();
    String::from_utf8(decoded).map_err(|err| format!("URL RaiPlay decodificato non valido: {err}"))
}

fn raiplay_obfuscated_url_key() -> Result<Vec<u8>, String> {
    Ok(resolve_raiplay_secret_key()?.into_bytes())
}

fn resolve_raiplay_secret_key() -> Result<String, String> {
    if let Some(secret_key) = crate::settings::load_saved_rai_luce_code() {
        let trimmed = secret_key.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    Err("Chiave Luce mancante: inserisci il codice nelle impostazioni RSS/Podcast.".to_string())
}

#[cfg(test)]
mod root_section_visibility_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn raiplay_live_section_is_hidden_from_root() {
        for title in [
            "Dirette",
            " dirette ",
            "DIRETTE",
            "Diretta",
            "Dirette TV",
            "TV in diretta",
            "Live",
        ] {
            let section = json!({"name": title, "elements": [{"title": "Rai 1"}]});
            assert!(parse_root_section(&section).unwrap().is_none(), "{title}");
        }
    }

    #[test]
    fn raiplay_live_section_is_hidden_when_api_uses_title() {
        let section =
            json!({"name": "stream", "title": "Dirette", "elements": [{"title": "Rai 1"}]});
        assert!(parse_root_section(&section).unwrap().is_none());
    }

    #[test]
    fn raiplay_on_demand_sections_remain_visible() {
        for title in [
            "Esplora",
            "Film",
            "Serie TV",
            "Documentari",
            "Bambini",
            "Cerca",
        ] {
            let section = json!({"name": title, "elements": [{"title": "Film on demand"}]});
            let result = parse_root_section(&section).unwrap().unwrap();
            assert_eq!(result.kind, BrowseItemKind::Page);
            assert_eq!(
                result.title,
                if title == "Cerca" { "Esplora" } else { title }
            );
        }
    }
}
