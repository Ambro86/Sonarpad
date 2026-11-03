use fancy_regex::Regex;
use reqwest::blocking::Client;
use scraper::{ElementRef, Html, Selector};
use std::collections::HashSet;
use std::time::Duration;
use url::Url;

const BASE: &str = "https://www.la7play.it";
const RIVEDI: &str = "https://www.la7play.it/guida-tv-rivedi/la7";
const PROGRAMMI: &str = "https://www.la7play.it/programmi";
const TUTTI_PROGRAMMI: &str = "https://www.la7.it/tutti-i-programmi";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";
const PROGRAM_EXCLUSIONS: &[&str] = &[
    "/meteola7",
    "/meteo-della-sera",
    "/tgla7",
    "/film",
    "/film-e-fiction",
];
const PROGRAM_MAPPINGS: &[(&str, &str)] = &[
    ("/facciaafaccia", "/faccia-a-faccia"),
    ("/il-boss-dei-comici", "/boss-dei-comici"),
    ("/lariadestate", "/laria-destate"),
    ("/taga-doc", "/tagada-doc"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ItemKind {
    Page,
    Media,
    Live,
}

#[derive(Clone, Debug)]
pub(crate) struct BrowseItem {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) kind: ItemKind,
    pub(crate) target: String,
}

#[derive(Clone, Debug)]
pub(crate) struct BrowsePage {
    pub(crate) title: String,
    pub(crate) source: String,
    pub(crate) items: Vec<BrowseItem>,
}

pub(crate) fn root_page() -> BrowsePage {
    BrowsePage {
        title: tr("la7.root"),
        source: "root".into(),
        items: vec![
            page("live", &tr("la7.live"), "live"),
            page("catchup", &tr("la7.catchup"), "rivedi"),
            page("programs", &tr("la7.programs"), "programs"),
        ],
    }
}

pub(crate) fn load_page(source: &str) -> Result<BrowsePage, String> {
    if source == "root" {
        return Ok(root_page());
    }
    if source == "live" {
        return Ok(BrowsePage {
            title: tr("la7.live"),
            source: source.into(),
            items: vec![
                live("live-la7", &tr("la7.live_la7"), "la7"),
                live("live-la7-cinema", &tr("la7.live_cinema"), "la7 cinema"),
            ],
        });
    }
    if source == "rivedi" {
        return rivedi_days();
    }
    if source == "programs" {
        return programs_page(None);
    }
    if let Some(url) = source.strip_prefix("day:") {
        return rivedi_day(url);
    }
    if let Some(url) = source.strip_prefix("program:") {
        return program_episodes(url);
    }
    if let Some(query) = source.strip_prefix("search:") {
        return programs_page(Some(query));
    }
    Err(tr("la7.page_error"))
}

pub(crate) fn search(query: &str) -> Result<BrowsePage, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err(tr("la7.search_prompt"));
    }
    programs_page(Some(query))
}

pub(crate) fn resolve_vod(page_url: &str) -> Result<String, String> {
    let html = get(page_url)?;
    if let Some(result) = structured_vod(&html) {
        return result;
    }
    let content =
        Regex::new(r#"\.net/i/.*?content/(.*?)(?:\.mp4)"#).map_err(|err| err.to_string())?;
    if let Some(captures) = content.captures(&html).map_err(|err| err.to_string())?
        && let Some(value) = captures.get(1)
    {
        return Ok(format!(
            "https://awsvodpkg.iltrovatore.it/local/hls/,/content/{}.mp4.urlset/master.m3u8",
            value.as_str()
        ));
    }

    let m3u8 = Regex::new(r#"m3u8:\s*[\"'](.*?)[\"']"#).map_err(|err| err.to_string())?;
    if let Some(captures) = m3u8.captures(&html).map_err(|err| err.to_string())?
        && let Some(value) = captures.get(1)
    {
        return Ok(value.as_str().replace("\\/", "/"));
    }

    let lower = html.to_ascii_lowercase();
    if lower.contains("widevine") || lower.contains("license") && lower.contains("dash") {
        return Err(tr("la7.drm"));
    }
    Err(tr("la7.no_media"))
}

// The new site renders its catalogue and player configuration as JSON in HTML.
// Read that data directly, retaining the legacy HTML parsers below.
fn site_data(html: &str) -> Vec<serde_json::Value> {
    let document = Html::parse_document(html);
    let mut values = Vec::new();
    let mut flight = String::new();
    for script in document.select(&selector("script")) {
        let text = script.inner_html();
        if script.value().attr("type") == Some("application/ld+json") {
            if let Ok(value) = serde_json::from_str(&text) {
                values.push(value);
            }
        } else if let Some(body) = text
            .trim()
            .strip_prefix("self.__next_f.push(")
            .and_then(|s| s.trim_end_matches(';').strip_suffix(')'))
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(body)
            && let Some(chunk) = value.get(1).and_then(|v| v.as_str())
        {
            flight.push_str(chunk);
        }
    }
    // Chunks may split a record (including in the middle of a JSON string).
    for line in flight.lines() {
        if let Some((_, body)) = line.split_once(':')
            && let Ok(value) = serde_json::from_str(body)
        {
            values.push(value);
        }
    }
    values
}

fn find_field<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    match value {
        serde_json::Value::Object(map) => map
            .get(key)
            .or_else(|| map.values().find_map(|v| find_field(v, key))),
        serde_json::Value::Array(items) => items.iter().find_map(|v| find_field(v, key)),
        _ => None,
    }
}

fn structured_items(html: &str, types: &[&str]) -> Vec<(String, String)> {
    fn collect(value: &serde_json::Value, types: &[&str], out: &mut Vec<(String, String)>) {
        if value["@type"] == "ItemList" {
            if let Some(items) = value["itemListElement"].as_array() {
                for item in items {
                    let item = &item["item"];
                    if types.contains(&item["@type"].as_str().unwrap_or(""))
                        && let (Some(title), Some(url)) =
                            (item["name"].as_str(), item["url"].as_str())
                        && !title.is_empty()
                        && !url.is_empty()
                    {
                        out.push((title.to_string(), url.to_string()));
                    }
                }
            }
        } else if let Some(map) = value.as_object() {
            for v in map.values() {
                collect(v, types, out);
            }
        } else if let Some(items) = value.as_array() {
            for v in items {
                collect(v, types, out);
            }
        }
    }
    let mut out = Vec::new();
    for value in site_data(html) {
        collect(&value, types, &mut out);
    }
    let mut seen = HashSet::new();
    out.retain(|(_, url)| seen.insert(url.clone()));
    out
}

fn guide_days(html: &str) -> Vec<serde_json::Value> {
    site_data(html)
        .iter()
        .find_map(|value| {
            find_field(value, "giorno")
                .and_then(|v| v.as_array())
                .cloned()
        })
        .unwrap_or_default()
}

fn playable_guide_entry(entry: &serde_json::Value) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    entry["pathname"]
        .as_str()
        .is_some_and(|path| !path.is_empty())
        && entry["timestamp"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .is_some_and(|t| t <= now)
}

fn structured_vod(html: &str) -> Option<Result<String, String>> {
    let data = site_data(html);
    let video = data.iter().find_map(|v| find_field(v, "videoData"))?;
    let settings = data.iter().find_map(|v| find_field(v, "playerSettings"))?;
    if video["drm"] == true && settings["abilitaDrm"] == true {
        return Some(Err(tr("la7.drm")));
    }
    let remote = video["uploadVideo"]["cf_remote_url"].as_str()?;
    let host = settings["hostnameCdn"].as_str()?;
    if remote.is_empty() || !host.starts_with("https://") {
        return None;
    }
    let new_storage = video["uploadVideo"]["cf_old_entry_id"]
        .as_str()
        .is_none_or(|s| s.is_empty());
    let remote = format!("/{}", remote.trim_start_matches('/')).replace(".mp4", "");
    let paths = remote
        .split(',')
        .map(|part| {
            let part = part.trim();
            if new_storage && !part.is_empty() && !part.starts_with("/nk/") {
                format!("/nk/{}", part.trim_start_matches('/'))
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    Some(Ok(format!(
        "{}/local/hls/,{paths},.mp4.urlset/master.m3u8",
        host.trim_end_matches('/')
    )))
}

fn rivedi_days() -> Result<BrowsePage, String> {
    let html = get(RIVEDI)?;
    let days = guide_days(&html);
    if !days.is_empty() {
        let mut items = Vec::new();
        for (index, day) in days.iter().enumerate() {
            if day["programma"]
                .as_array()
                .is_none_or(|entries| !entries.iter().any(playable_guide_entry))
            {
                continue;
            }
            if let Some(date) = day["@attributes"]["data"].as_str() {
                items.push(page(
                    &format!("day-{index}"),
                    &date.replace('-', "/"),
                    &format!("day:{RIVEDI}#{date}"),
                ));
            }
        }
        items.reverse();
        return Ok(BrowsePage {
            title: tr("la7.catchup"),
            source: "rivedi".into(),
            items,
        });
    }

    let document = Html::parse_document(&html);
    let rows = selector("div.item--menu-guida-tv, div.item.item--menu-guida-tv");
    let links = selector("a");
    let number = selector(".giorno-numero");
    let month = selector(".giorno-mese");
    let name = selector(".giorno-text");
    let mut items = Vec::new();

    for (index, row) in document.select(&rows).enumerate() {
        let href = row
            .select(&links)
            .next()
            .and_then(|link| link.value().attr("href"))
            .unwrap_or("");
        if href.is_empty() {
            continue;
        }
        let title = format!(
            "{} {} {}",
            text(&row, &name),
            text(&row, &number),
            text(&row, &month)
        )
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
        items.push(page(
            &format!("day-{index}"),
            &title,
            &format!("day:{}", absolute(href)),
        ));
    }
    items.reverse();
    Ok(BrowsePage {
        title: tr("la7.catchup"),
        source: "rivedi".into(),
        items,
    })
}

fn rivedi_day(url: &str) -> Result<BrowsePage, String> {
    let html = get(url)?;
    let days = guide_days(&html);
    if !days.is_empty() {
        let date = url.split_once('#').map(|(_, date)| date).unwrap_or("");
        let mut items = Vec::new();
        for day in &days {
            if day["@attributes"]["data"].as_str() != Some(date) {
                continue;
            }
            if let Some(entries) = day["programma"].as_array() {
                for entry in entries.iter().filter(|e| playable_guide_entry(e)) {
                    let title = format!(
                        "{} — {}",
                        entry["ora"].as_str().unwrap_or(""),
                        entry["titolo"].as_str().unwrap_or("")
                    );
                    items.push(media(
                        &format!("episode-{}", items.len()),
                        &title,
                        &absolute(entry["pathname"].as_str().unwrap_or("")),
                        entry["sinossi"].as_str().map(str::to_string),
                    ));
                }
            }
        }
        return Ok(BrowsePage {
            title: tr("la7.catchup"),
            source: format!("day:{url}"),
            items,
        });
    }

    let document = Html::parse_document(&html);
    let rows = selector("#content_guida_tv_rivedi div.item--guida-tv");
    let links = selector("a");
    let time = selector(".orario");
    let property = selector(".property");
    let plot = selector(".occhiello");
    let mut items = Vec::new();

    for (index, row) in document.select(&rows).enumerate() {
        let href = row
            .select(&links)
            .next()
            .and_then(|link| link.value().attr("href"))
            .unwrap_or("");
        if href.is_empty() {
            continue;
        }
        let time_text = text(&row, &time);
        let name = text(&row, &property);
        let title = if time_text.is_empty() {
            name
        } else {
            format!("{time_text} — {name}")
        };
        items.push(media(
            &format!("episode-{index}"),
            &title,
            &absolute(href),
            opt_text(&row, &plot),
        ));
    }

    Ok(BrowsePage {
        title: tr("la7.catchup"),
        source: format!("day:{url}"),
        items,
    })
}

fn programs_page(query: Option<&str>) -> Result<BrowsePage, String> {
    let mut programs = Vec::new();
    let mut seen = HashSet::new();

    for url in [PROGRAMMI, TUTTI_PROGRAMMI] {
        let Ok(html) = get(url) else {
            continue;
        };
        let listed = structured_items(&html, &["TVSeries"]);
        if !listed.is_empty() {
            programs.extend(listed);
            break;
        }
        let document = Html::parse_document(&html);
        let rows = selector("#container-programmi-list div.list-item, div.list-item");
        let links = selector("a");
        let titles = selector(".titolo");

        for row in document.select(&rows) {
            let raw_href = row
                .select(&links)
                .next()
                .and_then(|link| link.value().attr("href"))
                .unwrap_or("")
                .trim();
            if raw_href.is_empty() {
                continue;
            }
            let href = map_program_href(raw_href);
            if is_excluded_program_href(&href) {
                continue;
            }
            let target = absolute(&href);
            let mut title = text(&row, &titles);
            if title.is_empty() {
                title = program_title_from_href(&href);
            }
            if title.is_empty() {
                continue;
            }

            let dedupe_key = format!(
                "{}|{}",
                normalize_search_text(&title),
                normalize_search_text(&target)
            );
            if !seen.insert(dedupe_key) {
                continue;
            }
            programs.push((title, target));
        }
    }

    if !programs.iter().any(|(_, url)| url.contains("la7play.it"))
        && !seen
            .iter()
            .any(|key| key.starts_with("la mala educaxxxion 2|"))
    {
        programs.push((
            "LA MALA EDUCAXXXION 2".to_string(),
            format!("{BASE}/la-mala-educaxxxion"),
        ));
    }

    let discovered_count = programs.len();
    let mut matched_programs: Vec<(String, String)> = programs
        .into_iter()
        .filter(|(title, target)| {
            query.is_none_or(|value| matches_program_query(title, target, value))
        })
        .collect();
    matched_programs.sort_by_key(|(title, _)| normalize_search_text(title));

    let mut items = Vec::new();
    let mut media_seen = HashSet::new();
    for (program_index, (title, target)) in matched_programs.into_iter().enumerate() {
        items.push(page(
            &format!("program-{program_index}"),
            &title,
            &format!("program:{target}"),
        ));

        // Nei risultati di ricerca le clip editoriali del programma vengono
        // mostrate come contenuti separati. La cartella del programma resta
        // riservata alle puntate complete e all'archivio Rivedi LA7.
        if query.is_some() {
            for (clip_index, clip) in program_search_clips(&target).into_iter().enumerate() {
                if !media_seen.insert(clip.target.clone()) {
                    continue;
                }
                items.push(BrowseItem {
                    id: format!("search-clip-{program_index}-{clip_index}"),
                    title: clip.title,
                    description: clip.description.or_else(|| Some(title.clone())),
                    kind: ItemKind::Media,
                    target: clip.target,
                });
            }
        }
    }

    crate::log_debug(&format!(
        "La Sette Play programs: query={:?} discovered={} results={}",
        query,
        discovered_count,
        items.len()
    ));

    let source = query
        .map(|value| format!("search:{value}"))
        .unwrap_or_else(|| "programs".into());
    Ok(BrowsePage {
        title: query
            .map(|value| format!("{}: {value}", tr("la7.search")))
            .unwrap_or_else(|| tr("la7.programs")),
        source,
        items,
    })
}

fn program_search_clips(program_url: &str) -> Vec<BrowseItem> {
    let Ok(html) = get(program_url) else {
        return Vec::new();
    };
    let listed = structured_items(&html, &["VideoObject", "TVEpisode", "TVClip"]);
    if !listed.is_empty() {
        return listed
            .into_iter()
            .filter(|(_, url)| !is_program_episode_url(url))
            .enumerate()
            .map(|(index, (title, url))| media(&format!("clip-{index}"), &title, &url, None))
            .collect();
    }
    let document = Html::parse_document(&html);
    let rows = selector(".home-block__content-inner div.item");
    let links = selector("a");
    let occhiello = selector(".occhiello");
    let titles = selector(".title, .title_puntata");
    let dates = selector(".data");
    let mut seen = HashSet::new();
    let mut clips = Vec::new();

    for row in document.select(&rows) {
        let href = row
            .select(&links)
            .next()
            .and_then(|link| link.value().attr("href"))
            .unwrap_or("")
            .trim();
        if href.is_empty() {
            continue;
        }
        let link = absolute(href);
        if is_program_episode_url(&link)
            || same_url_without_query(&link, program_url)
            || !seen.insert(link.clone())
        {
            continue;
        }

        let mut title = text(&row, &occhiello);
        if title.is_empty() {
            title = text(&row, &titles);
        }
        if title.is_empty() {
            continue;
        }
        let date = text(&row, &dates);
        if !date.is_empty() {
            title = format!("{title} ({date})");
        }
        clips.push(media(&format!("clip-{}", clips.len()), &title, &link, None));
    }
    clips
}

fn program_episodes(url: &str) -> Result<BrowsePage, String> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();

    // Dalla pagina principale prendiamo soltanto il riquadro "ultima puntata".
    // Le altre card sono clip editoriali e vengono mostrate separatamente nei
    // risultati della ricerca, non dentro l'archivio del programma.
    if let Ok(html) = get(url) {
        let listed = structured_items(&html, &["TVEpisode", "VideoObject"]);
        if !listed.is_empty() {
            let items = listed
                .into_iter()
                .filter(|(_, link)| is_program_episode_url(link))
                .enumerate()
                .map(|(index, (title, link))| media(&format!("media-{index}"), &title, &link, None))
                .collect();
            return Ok(BrowsePage {
                title: program_title_from_href(url),
                source: format!("program:{url}"),
                items,
            });
        }
        let document = Html::parse_document(&html);
        let latest = selector(".ultima_puntata");
        for row in document.select(&latest) {
            push_program_episode(&mut items, &mut seen, &row);
        }
    }

    // La pagina /rivedila7 contiene ultima replica, carosello settimanale e
    // archivio delle puntate complete.
    let rivedi_url = format!("{}/rivedila7", url.trim_end_matches('/'));
    if let Ok(html) = get(&rivedi_url) {
        let document = Html::parse_document(&html);
        let latest = selector(
            ".ultima_puntata, .contenitoreUltimaReplicaLa7d, .contenitoreUltimaReplicaNoLuminosa",
        );
        if let Some(row) = document.select(&latest).next() {
            push_program_episode(&mut items, &mut seen, &row);
        }

        let carousel = selector(".home-block__content-carousel.container-vetrina div.item");
        for row in document.select(&carousel) {
            push_program_episode(&mut items, &mut seen, &row);
        }

        let archive = selector(".view-content.clearfix .views-row, .view-content .views-row");
        for row in document.select(&archive) {
            push_program_episode(&mut items, &mut seen, &row);
        }
    }

    Ok(BrowsePage {
        title: program_title_from_href(url),
        source: format!("program:{url}"),
        items,
    })
}

fn push_program_episode(
    items: &mut Vec<BrowseItem>,
    seen: &mut HashSet<String>,
    row: &ElementRef<'_>,
) {
    let links = selector("a");
    let titles = selector(".title_puntata, .title");
    let dates = selector(".scritta_ultima, .data");
    let plots = selector(".occhiello");
    let href = row
        .select(&links)
        .next()
        .and_then(|link| link.value().attr("href"))
        .unwrap_or("")
        .trim();
    if href.is_empty() {
        return;
    }
    let link = absolute(href);
    if !is_program_episode_url(&link) || !seen.insert(link.clone()) {
        return;
    }

    let mut title = text(row, &titles);
    if title.is_empty() {
        title = tr("la7.episode");
    }
    let date = text(row, &dates);
    if !date.is_empty() {
        title = format!("{title} ({date})");
    }
    items.push(media(
        &format!("media-{}", items.len()),
        &title,
        &link,
        opt_text(row, &plots),
    ));
}

fn is_program_episode_url(url: &str) -> bool {
    Url::parse(url)
        .ok()
        .map(|parsed| {
            let path = parsed.path().to_ascii_lowercase();
            path.contains("/rivedila7/") || path.contains("/puntate/")
        })
        .unwrap_or_else(|| url.to_ascii_lowercase().contains("/rivedila7/"))
}

fn same_url_without_query(left: &str, right: &str) -> bool {
    normalized_url_path(left) == normalized_url_path(right)
}

fn normalized_url_path(value: &str) -> String {
    Url::parse(value)
        .ok()
        .map(|url| url.path().trim_end_matches('/').to_ascii_lowercase())
        .unwrap_or_else(|| {
            value
                .split(['?', '#'])
                .next()
                .unwrap_or(value)
                .trim_end_matches('/')
                .to_ascii_lowercase()
        })
}

fn get(url: &str) -> Result<String, String> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .user_agent(UA)
        .build()
        .map_err(|err| err.to_string())?
        .get(url)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("Accept-Language", "it-IT,it;q=0.9,en;q=0.8")
        .header("Referer", BASE)
        .header("Origin", BASE)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.text())
        .map_err(|err| trf("la7.load_error", &err.to_string()))
}

fn map_program_href(href: &str) -> String {
    let rule_path = program_rule_path(href);
    for (from, to) in PROGRAM_MAPPINGS {
        if rule_path.eq_ignore_ascii_case(from) {
            return (*to).to_string();
        }
    }
    href.to_string()
}

fn is_excluded_program_href(href: &str) -> bool {
    let path = program_rule_path(href);
    PROGRAM_EXCLUSIONS
        .iter()
        .any(|excluded| path.eq_ignore_ascii_case(excluded))
}

fn program_rule_path(href: &str) -> String {
    if let Ok(url) = Url::parse(href) {
        return url.path().trim_end_matches('/').to_string();
    }
    href.split(['?', '#'])
        .next()
        .unwrap_or(href)
        .trim_end_matches('/')
        .to_string()
}

fn program_title_from_href(href: &str) -> String {
    let path = program_rule_path(href);
    let slug = path.rsplit('/').find(|part| !part.is_empty()).unwrap_or("");
    slug.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(capitalize_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize_word(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first
        .to_uppercase()
        .chain(chars.flat_map(char::to_lowercase))
        .collect()
}

fn matches_program_query(title: &str, url: &str, query: &str) -> bool {
    let query = normalize_search_text(query);
    if query.is_empty() {
        return false;
    }
    let words: Vec<&str> = query.split_whitespace().collect();
    let title = normalize_search_text(title);
    let url = normalize_search_text(url);
    query_matches_text(&query, &words, &title) || query_matches_text(&query, &words, &url)
}

fn query_matches_text(query: &str, words: &[&str], text: &str) -> bool {
    text.contains(query) || !words.is_empty() && words.iter().all(|word| text.contains(word))
}

fn normalize_search_text(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_space = true;
    for character in value.trim().chars().flat_map(char::to_lowercase) {
        if matches!(character, '\'' | '’' | '`' | '´' | 'ʼ') {
            continue;
        }
        let folded = match character {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other if other.is_alphanumeric() => other,
            _ => ' ',
        };
        if folded == ' ' {
            if !previous_space {
                normalized.push(' ');
                previous_space = true;
            }
        } else {
            normalized.push(folded);
            previous_space = false;
        }
    }
    normalized.trim().to_string()
}

fn selector(value: &str) -> Selector {
    match Selector::parse(value) {
        Ok(selector) => selector,
        Err(error) => panic!("invalid LA7 selector {value:?}: {error:?}"),
    }
}

fn text(node: &ElementRef<'_>, selector: &Selector) -> String {
    node.select(selector)
        .next()
        .map(|element| {
            element
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

fn opt_text(node: &ElementRef<'_>, selector: &Selector) -> Option<String> {
    let value = text(node, selector);
    if value.is_empty() { None } else { Some(value) }
}

fn absolute(href: &str) -> String {
    Url::parse(BASE)
        .ok()
        .and_then(|base| base.join(href).ok())
        .map(|url| url.to_string())
        .unwrap_or_else(|| href.to_string())
}

fn page(id: &str, title: &str, target: &str) -> BrowseItem {
    BrowseItem {
        id: id.into(),
        title: title.into(),
        description: None,
        kind: ItemKind::Page,
        target: target.into(),
    }
}

fn live(id: &str, title: &str, target: &str) -> BrowseItem {
    BrowseItem {
        id: id.into(),
        title: title.into(),
        description: None,
        kind: ItemKind::Live,
        target: target.into(),
    }
}

fn media(id: &str, title: &str, target: &str, description: Option<String>) -> BrowseItem {
    BrowseItem {
        id: id.into(),
        title: title.into(),
        description,
        kind: ItemKind::Media,
        target: target.into(),
    }
}

fn tr(key: &str) -> String {
    crate::i18n::tr_la7_play(key)
}

fn trf(key: &str, error: &str) -> String {
    crate::i18n::tr_la7_play_f(key, &[("error", error)])
}

#[cfg(test)]
mod tests {
    use super::{
        is_program_episode_url, matches_program_query, normalize_search_text,
        program_title_from_href, same_url_without_query,
    };

    #[test]
    fn la7_search_ignores_apostrophes_accents_and_word_order() {
        assert!(matches_program_query(
            "L'Aria che tira",
            "https://www.la7.it/laria-che-tira",
            "l'aria che tira"
        ));
        assert!(matches_program_query(
            "Tagadà",
            "https://www.la7.it/tagada",
            "tagada"
        ));
        assert!(matches_program_query(
            "L'Aria che tira",
            "https://www.la7.it/laria-che-tira",
            "tira aria"
        ));
    }

    #[test]
    fn la7_search_can_match_program_slug() {
        assert!(matches_program_query(
            "Programma",
            "https://www.la7.it/laria-che-tira",
            "l'aria che tira"
        ));
    }

    #[test]
    fn la7_program_folder_accepts_only_rivedi_episode_urls() {
        assert!(is_program_episode_url(
            "https://www.la7.it/laria-che-tira/rivedila7/laria-che-tira-23-07-2026-123"
        ));
        assert!(!is_program_episode_url(
            "https://www.la7.it/laria-che-tira/video/sondaggi-intenzioni-di-voto"
        ));
    }

    #[test]
    fn la7_search_clip_does_not_repeat_program_page() {
        assert!(same_url_without_query(
            "https://www.la7.it/laria-che-tira/?ref=home",
            "https://www.la7.it/laria-che-tira"
        ));
    }

    #[test]
    fn la7_program_title_falls_back_to_slug() {
        assert_eq!(program_title_from_href("/laria-che-tira"), "Laria Che Tira");
        assert_eq!(normalize_search_text("L’Aria che Tira"), "laria che tira");
    }
}

#[cfg(test)]
mod new_site_tests {
    use super::*;
    const PROGRAMS: &str = include_str!("../../tests/fixtures/la7play/programs.html");
    const PROGRAM: &str = include_str!("../../tests/fixtures/la7play/prop.html");
    const GUIDE: &str = include_str!("../../tests/fixtures/la7play/guide.html");
    const FREE: &str = include_str!("../../tests/fixtures/la7play/free-episode.html");
    const DRM: &str = include_str!("../../tests/fixtures/la7play/episode.html");

    #[test]
    fn new_catalogue_lists_programs_and_separates_full_episodes() {
        let programs = structured_items(PROGRAMS, &["TVSeries"]);
        assert_eq!(programs.len(), 58);
        assert!(programs.iter().any(|(title, _)| title == "Otto e mezzo"));
        let episodes = structured_items(PROGRAM, &["TVEpisode", "VideoObject"]);
        assert!(!episodes.is_empty());
        assert_eq!(structured_items(PROGRAM, &["TVClip"]).len(), 20);
        assert!(episodes.iter().any(|(_, url)| is_program_episode_url(url)));
        assert!(!is_program_episode_url(
            "https://www.la7play.it/otto-e-mezzo/video/clip"
        ));
    }

    #[test]
    fn new_guide_decodes_split_chunks_and_keeps_playable_replays() {
        let days = guide_days(GUIDE);
        assert_eq!(days.len(), 2);
        assert_eq!(days[0]["@attributes"]["data"], "26-10-2026");
        assert!(playable_guide_entry(&days[0]["programma"][0]));
        assert!(!playable_guide_entry(
            &serde_json::json!({"timestamp":"1792994400"})
        ));
        assert!(!playable_guide_entry(
            &serde_json::json!({"timestamp":"9999999999", "pathname":"/future"})
        ));
    }

    #[test]
    fn new_player_uses_actual_storage_and_does_not_confuse_drm_settings_with_drm_content() {
        let url = structured_vod(FREE).unwrap().unwrap();
        assert!(url.starts_with("https://awsvodpkg.iltrovatore.it/local/hls/,/nk/content/"));
        assert!(url.ends_with(",.mp4.urlset/master.m3u8"));
        assert!(!url.contains("/nk/nk/"));
        assert!(structured_vod(DRM).unwrap().is_err());
        assert!(
            structured_vod("<script>var player={m3u8:'https://example.test/a.m3u8'}</script>")
                .is_none()
        );
    }

    #[test]
    #[ignore = "Requests the public LA7 Play catalogue and a free HLS manifest"]
    fn live_new_site_catalogue_and_free_manifest() {
        let programs = load_page("programs").unwrap();
        assert!(programs.items.len() > 10);
        let program = programs
            .items
            .iter()
            .find(|p| p.title == "Otto e mezzo")
            .unwrap();
        let episodes = load_page(&program.target).unwrap();
        assert!(!episodes.items.is_empty());
        let stream = resolve_vod(&episodes.items[0].target).unwrap();
        assert!(get(&stream).unwrap().starts_with("#EXTM3U"));
        let days = load_page("rivedi").unwrap();
        assert!(!days.items.is_empty());
        let day = load_page(&days.items[0].target).unwrap();
        assert!(!day.items.is_empty());
    }
}
