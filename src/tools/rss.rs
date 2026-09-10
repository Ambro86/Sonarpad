use crate::log_debug;
use crate::tools::reader;
use crate::{i18n, settings::Language};
use encoding_rs::{Encoding, WINDOWS_1252};
use feed_rs::parser;
use reqwest::{self, StatusCode, header};
use scraper::{Html, Selector};

use header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED, REFERER};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::Cursor;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::time::sleep;
use url::Url;

type HttpClient = reqwest::Client;
type HttpError = reqwest::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RssSourceType {
    Feed,
    Article,
    Site,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RssFeedCache {
    #[serde(default)]
    pub feed_url: Option<String>,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub last_modified: Option<String>,
    #[serde(default)]
    pub last_fetch: Option<i64>,
    #[serde(default)]
    pub last_status: Option<u16>,
    #[serde(default)]
    pub consecutive_failures: u32,
    #[serde(default)]
    pub blocked_until_epoch_secs: Option<i64>,
    #[serde(default)]
    pub last_error_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RssSource {
    pub title: String,
    pub url: String,
    pub kind: RssSourceType,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub folder_path: Vec<String>,
    #[serde(default)]
    pub user_title: bool,
    #[serde(default)]
    pub unread: bool,
    #[serde(default)]
    pub cache: RssFeedCache,
    #[serde(default)]
    pub last_seen_guid: Option<String>,
    #[serde(default)]
    pub last_updated: Option<i64>,
    #[serde(default)]
    pub removed_item_keys: Vec<String>,
    #[serde(default)]
    pub read_item_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RssItem {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub link: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_folder: bool,
    #[serde(default)]
    pub guid: String,
    #[serde(default)]
    pub pub_date: Option<i64>,
    #[serde(default)]
    pub related_items: Vec<RssItem>,
}

#[derive(Debug, Clone)]
pub struct PodcastEpisode {
    pub title: String,
    pub link: String,
    pub description: String,
    pub guid: String,
    pub enclosure_url: Option<String>,
    pub enclosure_type: Option<String>,
    pub chapters_url: Option<String>,
    pub chapters_type: Option<String>,
    pub podlove_chapters: Vec<crate::podcast::chapters::Chapter>,
    pub pub_date: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
pub struct RssFetchConfig {
    pub max_items_per_feed: usize,
    pub max_excerpt_chars: usize,
}

impl Default for RssFetchConfig {
    fn default() -> Self {
        Self {
            max_items_per_feed: 5000,
            max_excerpt_chars: 512,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FeedFetchError {
    HttpStatus {
        status: u16,
        kind: String,
        cache: Box<RssFeedCache>,
    },
    Network {
        message: String,
        cache: Box<RssFeedCache>,
    },
}

impl std::fmt::Display for FeedFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedFetchError::HttpStatus { status, kind, .. } => {
                write!(f, "HTTP {status} ({kind})")
            }
            FeedFetchError::Network { message, .. } => write!(f, "{message}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RssHttpConfig {
    pub global_max_concurrency: usize,
    pub per_host_max_concurrency: usize,
    pub max_retries: usize,
    pub backoff_max_secs: u64,
}

impl Default for RssHttpConfig {
    fn default() -> Self {
        Self {
            global_max_concurrency: 8,
            per_host_max_concurrency: 2,
            max_retries: 4,
            backoff_max_secs: 120,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RssFetchOutcome {
    pub kind: RssSourceType,
    pub title: String,
    pub items: Vec<RssItem>,
    pub cache: RssFeedCache,
    pub not_modified: bool,
}

#[derive(Debug, Clone)]
pub struct PodcastFetchOutcome {
    pub title: String,
    pub items: Vec<PodcastEpisode>,
    pub cache: RssFeedCache,
}

struct RssHttp {
    client: HttpClient,
    global_sem: Arc<Semaphore>,
    per_host_sem: Mutex<HashMap<String, Arc<Semaphore>>>,
    config: RssHttpConfig,
}

impl RssHttp {
    fn new(config: RssHttpConfig) -> Result<Self, String> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            ),
        );
        headers.insert(
            REFERER,
            header::HeaderValue::from_static("https://news.google.com/"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::limited(10))
            .gzip(true)
            .brotli(true)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            client,
            global_sem: Arc::new(Semaphore::new(config.global_max_concurrency.max(1))),
            per_host_sem: Mutex::new(HashMap::new()),
            config,
        })
    }

    async fn acquire_permits(&self, host: &str) -> Result<RequestPermits, String> {
        let global = self
            .global_sem
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "Global concurrency limiter closed".to_string())?;

        let host_sem = {
            let mut map = self.per_host_sem.lock().await;
            map.entry(host.to_string())
                .or_insert_with(|| {
                    Arc::new(Semaphore::new(self.config.per_host_max_concurrency.max(1)))
                })
                .clone()
        };
        let host = host_sem
            .acquire_owned()
            .await
            .map_err(|_| "Per-host concurrency limiter closed".to_string())?;

        Ok(RequestPermits {
            _global: global,
            _host: host,
        })
    }
}

struct RequestPermits {
    _global: OwnedSemaphorePermit,
    _host: OwnedSemaphorePermit,
}

static RSS_HTTP: OnceLock<Result<RssHttp, String>> = OnceLock::new();

pub fn init_http(config: RssHttpConfig) -> Result<(), String> {
    let res = RSS_HTTP.get_or_init(|| RssHttp::new(config));
    res.as_ref().map(|_| ()).map_err(|e| e.clone())
}

fn shared_http() -> Result<&'static RssHttp, String> {
    let res = RSS_HTTP.get_or_init(|| RssHttp::new(RssHttpConfig::default()));
    res.as_ref().map_err(|e| e.clone())
}

pub fn normalize_url(input: &str) -> String {
    let s = input.trim();
    if s.is_empty() {
        return String::new();
    }
    if s.starts_with("//") {
        return format!("https:{s}");
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        let mut out = s.to_string();
        if out.starts_with("http:////") {
            out = out.replacen("http:////", "http://", 1);
        } else if out.starts_with("https:////") {
            out = out.replacen("https:////", "https://", 1);
        } else if out.starts_with("http:///") {
            out = out.replacen("http:///", "http://", 1);
        } else if out.starts_with("https:///") {
            out = out.replacen("https:///", "https://", 1);
        }
        return out;
    }
    format!("https://{s}")
}

fn canonicalize_url(u: &str) -> String {
    let normalized = normalize_url(u);
    if let Ok(mut url) = Url::parse(&normalized) {
        url.set_fragment(None);
        if url.query().is_some() {
            let pairs: Vec<(String, String)> = url
                .query_pairs()
                .filter(|(k, _)| {
                    let k = k.to_ascii_lowercase();
                    !(k.starts_with("utm_")
                        || k == "gclid"
                        || k == "fbclid"
                        || k == "yclid"
                        || k == "mc_cid"
                        || k == "mc_eid")
                })
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            url.query_pairs_mut().clear();
            if pairs.is_empty() {
                url.set_query(None);
            } else {
                for (k, v) in pairs {
                    url.query_pairs_mut().append_pair(&k, &v);
                }
            }
        }
        crate::log_if_err!(url.set_port(None));
        let mut s = url.to_string();
        if let Some(rest) = s.strip_prefix("https://") {
            s = rest.to_string();
        } else if let Some(rest) = s.strip_prefix("http://") {
            s = rest.to_string();
        }
        while s.ends_with('/') && s.len() > 1 {
            s.pop();
        }
        return s;
    }
    let mut s = normalized;
    if let Some(rest) = s.strip_prefix("https://") {
        s = rest.to_string();
    } else if let Some(rest) = s.strip_prefix("http://") {
        s = rest.to_string();
    }
    if let Some((left, _)) = s.split_once('#') {
        s = left.to_string();
    }
    if let Some((left, _)) = s.split_once('?') {
        s = left.to_string();
    }
    while s.ends_with('/') && s.len() > 1 {
        s.pop();
    }
    s
}

const IL_GIORNALE_FEED_URL: &str =
    "https://news.google.com/rss/search?q=site%3Ailgiornale.it&hl=it&gl=IT&ceid=IT:it";

fn is_legacy_il_giornale_feed_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && matches!(url.host_str(), Some("ilgiornale.it" | "www.ilgiornale.it"))
        && url.path() == "/rss.xml"
        && url.query().is_none()
}

const CORRIERE_HOME_FEED_URL: &str =
    "https://xml2.corriereobjects.it/feed-hp/homepage-restyle-2025.xml";

fn is_corriere_home_feed_url(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    let path = parsed.path().trim_end_matches('/').to_ascii_lowercase();

    match host.as_str() {
        "corriere.it" | "www.corriere.it" => {
            path.is_empty() || path == "/rss" || path == "/rss/homepage.xml"
        }
        "xml2.corriereobjects.it" => {
            path == "/feed-hp/homepage.xml" || path == "/feed-hp/homepage-restyle-2025.xml"
        }
        _ => false,
    }
}

fn format_error_chain(e: &HttpError) -> String {
    let mut msg = e.to_string();
    let mut cur: Option<&(dyn Error + 'static)> = e.source();
    while let Some(err) = cur {
        msg.push_str(" | caused by: ");
        msg.push_str(&err.to_string());
        cur = err.source();
    }
    msg
}

fn host_from_url(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
}

fn detect_charset_label_from_html(bytes: &[u8]) -> Option<String> {
    let probe_len = bytes.len().min(16 * 1024);
    let probe = String::from_utf8_lossy(&bytes[..probe_len]).to_ascii_lowercase();
    let charset_pos = probe.find("charset=")?;
    let after = &probe[charset_pos + "charset=".len()..];
    let mut out = String::new();
    let mut started = false;
    for ch in after.chars() {
        if !started && (ch == '"' || ch == '\'' || ch.is_whitespace()) {
            continue;
        }
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            started = true;
            out.push(ch);
            continue;
        }
        if started {
            break;
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn decode_html_bytes(bytes: &[u8]) -> String {
    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
        return text;
    }

    if let Some(label) = detect_charset_label_from_html(bytes)
        && let Some(encoding) = Encoding::for_label(label.as_bytes())
    {
        let (decoded, _, _) = encoding.decode(bytes);
        return decoded.into_owned();
    }

    let (decoded, _, _) = WINDOWS_1252.decode(bytes);
    decoded.into_owned()
}

fn decode_basic_html_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '&' {
            out.push(c);
            continue;
        }

        let mut entity = String::new();
        let mut ended_with_semicolon = false;
        while let Some(&next) = chars.peek() {
            chars.next();
            if next == ';' {
                ended_with_semicolon = true;
                break;
            }
            if entity.len() >= 16 {
                entity.push(next);
                break;
            }
            entity.push(next);
        }

        let decoded = if entity.starts_with("#x") || entity.starts_with("#X") {
            u32::from_str_radix(&entity[2..], 16)
                .ok()
                .and_then(char::from_u32)
        } else if let Some(num) = entity.strip_prefix('#') {
            num.parse::<u32>().ok().and_then(char::from_u32)
        } else {
            match entity.as_str() {
                "nbsp" => Some(' '),
                "amp" => Some('&'),
                "quot" | "quote" => Some('"'),
                "apos" => Some('\''),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "laquo" => Some('«'),
                "raquo" => Some('»'),
                "hellip" => Some('…'),
                "ndash" => Some('–'),
                "mdash" => Some('—'),
                "rsquo" => Some('’'),
                "lsquo" => Some('‘'),
                "rdquo" => Some('”'),
                "ldquo" => Some('“'),
                "agrave" => Some('à'),
                "egrave" => Some('è'),
                "igrave" => Some('ì'),
                "ograve" => Some('ò'),
                "ugrave" => Some('ù'),
                "aacute" => Some('á'),
                "eacute" => Some('é'),
                "iacute" => Some('í'),
                "oacute" => Some('ó'),
                "uacute" => Some('ú'),
                "Agrave" => Some('À'),
                "Egrave" => Some('È'),
                "Igrave" => Some('Ì'),
                "Ograve" => Some('Ò'),
                "Ugrave" => Some('Ù'),
                "Aacute" => Some('Á'),
                "Eacute" => Some('É'),
                "Iacute" => Some('Í'),
                "Oacute" => Some('Ó'),
                "Uacute" => Some('Ú'),
                _ => None,
            }
        };

        if let Some(ch) = decoded {
            out.push(ch);
        } else {
            out.push('&');
            out.push_str(&entity);
            if ended_with_semicolon {
                out.push(';');
            }
        }
    }
    out
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn should_retry_status(status: StatusCode) -> bool {
    matches!(status.as_u16(), 429 | 502 | 503 | 504 | 508)
}

fn compute_backoff(attempt: usize, max_secs: u64) -> Duration {
    let secs = 1u64
        .checked_shl(attempt as u32)
        .unwrap_or(u64::MAX)
        .min(max_secs);
    Duration::from_secs(secs)
}

fn normalize_related_google_news_url(href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') || href.starts_with("javascript:") {
        return None;
    }
    if let Ok(url) = Url::parse(href) {
        return matches!(url.scheme(), "http" | "https").then(|| url.to_string());
    }
    let base = Url::parse("https://news.google.com/").ok()?;
    let joined = base.join(href).ok()?;
    matches!(joined.scheme(), "http" | "https").then(|| joined.to_string())
}

fn compact_element_text<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    parts
        .flat_map(str::split_whitespace)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_google_news_related_items(
    description_html: &str,
    parent_title: &str,
    parent_link: &str,
    pub_date: Option<i64>,
) -> Vec<RssItem> {
    if !is_google_news_article_url(parent_link)
        && !description_html
            .to_ascii_lowercase()
            .contains("news.google.com")
    {
        return Vec::new();
    }

    let document = Html::parse_fragment(description_html);
    let Ok(li_selector) = Selector::parse("li") else {
        return Vec::new();
    };
    let Ok(anchor_selector) = Selector::parse("a[href]") else {
        return Vec::new();
    };
    let mut related = Vec::new();
    let mut seen = HashSet::new();

    for li in document.select(&li_selector) {
        let Some(anchor) = li.select(&anchor_selector).next() else {
            continue;
        };
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let Some(link) = normalize_related_google_news_url(href) else {
            continue;
        };
        let key = canonicalize_url(&link);
        if key.is_empty() || !seen.insert(key.clone()) {
            continue;
        }

        let article_title = compact_element_text(anchor.text());
        if article_title.is_empty() {
            continue;
        }
        let whole_line = compact_element_text(li.text());
        let source = whole_line
            .strip_prefix(&article_title)
            .unwrap_or_default()
            .trim_matches(|ch: char| ch.is_whitespace() || matches!(ch, '-' | '–' | '—' | '·'))
            .trim();
        let display_title = if source.is_empty() || article_title.ends_with(source) {
            article_title
        } else {
            format!("{article_title} — {source}")
        };
        related.push(RssItem {
            title: display_title,
            link,
            description: String::new(),
            is_folder: false,
            guid: format!("related:{key}"),
            pub_date,
            related_items: Vec::new(),
        });
    }

    // Some Google News variants omit the primary link from the HTML list.
    let parent_key = canonicalize_url(parent_link);
    if !parent_link.trim().is_empty()
        && !parent_title.trim().is_empty()
        && !seen.contains(&parent_key)
    {
        related.insert(
            0,
            RssItem {
                title: parent_title.to_string(),
                link: parent_link.to_string(),
                description: String::new(),
                is_folder: false,
                guid: format!("related:{parent_key}"),
                pub_date,
                related_items: Vec::new(),
            },
        );
    }

    // A single link does not add useful hierarchy.
    if related.len() < 2 {
        Vec::new()
    } else {
        related
    }
}

fn parse_feed_bytes(
    bytes: Vec<u8>,
    fallback_title: &str,
    max_excerpt_chars: usize,
    language: Language,
) -> Option<(String, Vec<RssItem>)> {
    let cursor = Cursor::new(bytes);
    let feed = parser::parse(cursor).ok()?;
    let title = feed
        .title
        .map(|t| t.content)
        .unwrap_or_else(|| fallback_title.to_string());
    let title = decode_basic_html_entities(&title);
    let items = feed
        .entries
        .into_iter()
        .map(|entry| {
            let title = entry
                .title
                .as_ref()
                .map(|t| t.content.clone())
                .unwrap_or_else(|| i18n::tr(language, "reader.no_title_fallback"));
            let title = decode_basic_html_entities(&title);
            let link = select_entry_link(&entry);
            let guid = if let Some(stable_guid) = stable_google_news_guid(&entry.id, &link) {
                stable_guid
            } else if !entry.id.trim().is_empty() {
                entry.id.clone()
            } else if !link.trim().is_empty() {
                link.clone()
            } else {
                title.clone()
            };
            let raw_description = entry
                .summary
                .as_ref()
                .map(|s| s.content.clone())
                .unwrap_or_default();
            let pub_date = entry.published.or(entry.updated).map(|d| d.timestamp());
            let related_items =
                extract_google_news_related_items(&raw_description, &title, &link, pub_date);
            let description = decode_basic_html_entities(&raw_description);
            let description = truncate_excerpt(&description, max_excerpt_chars);
            RssItem {
                title,
                link,
                description,
                is_folder: false,
                guid,
                pub_date,
                related_items,
            }
        })
        .collect();
    Some((title, items))
}

pub(crate) fn is_google_news_article_url(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url.trim()) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or("");
    if !host.eq_ignore_ascii_case("news.google.com") {
        return false;
    }
    let path = parsed.path().to_ascii_lowercase();
    // Google News article links can appear in different path variants.
    path.contains("/rss/articles/")
        || path.contains("/articles/")
        || path.contains("/read/")
        || path.contains("/__i/rss/rd/articles/")
}

fn stable_google_news_guid(entry_id: &str, link: &str) -> Option<String> {
    let id_from_link = if is_google_news_article_url(link) {
        extract_google_news_article_id(link)
    } else {
        None
    };
    if let Some(id) = id_from_link {
        return Some(format!("google-news:{id}"));
    }

    if is_google_news_article_url(entry_id)
        && let Some(id) = extract_google_news_article_id(entry_id)
    {
        return Some(format!("google-news:{id}"));
    }
    None
}

fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = s.find(start)? + start.len();
    let rest = &s[from..];
    let to = rest.find(end)?;
    Some(&rest[..to])
}

fn extract_google_news_article_id(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let mut segments = parsed.path_segments()?;
    let segments: Vec<&str> = segments.by_ref().collect();
    let pos = segments
        .iter()
        .position(|seg| seg.eq_ignore_ascii_case("articles"))?;
    let id = segments.get(pos + 1)?.trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn extract_google_news_tokens(html: &str) -> Option<(String, String)> {
    let signature = extract_between(html, "data-n-a-sg=\"", "\"")
        .or_else(|| extract_between(html, "data-n-a-sg='", "'"))?
        .trim()
        .to_string();
    let timestamp = extract_between(html, "data-n-a-ts=\"", "\"")
        .or_else(|| extract_between(html, "data-n-a-ts='", "'"))?
        .trim()
        .to_string();
    if signature.is_empty() || timestamp.is_empty() {
        None
    } else {
        Some((signature, timestamp))
    }
}

fn extract_google_news_direct_url_from_article_html(html: &str) -> Option<String> {
    let candidate = extract_between(html, "data-n-au=\"", "\"")
        .or_else(|| extract_between(html, "data-n-au='", "'"))
        .map(str::trim)
        .filter(|v| !v.is_empty())?;
    let parsed = Url::parse(candidate).ok()?;
    match parsed.scheme() {
        "http" | "https" => {
            if is_google_news_article_url(candidate) {
                None
            } else {
                Some(candidate.to_string())
            }
        }
        _ => None,
    }
}

fn fetch_google_news_article_page_html(url: &str) -> Result<String, String> {
    let html = String::from_utf8_lossy(
        &crate::curl_client::CurlClient::fetch_url_impersonated(url).map_err(|e| e.to_string())?,
    )
    .to_string();
    if extract_google_news_tokens(&html).is_some() {
        return Ok(html);
    }

    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        )
        .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .text()
        .map_err(|e| e.to_string())
}

fn encode_form_value(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn extract_decoded_google_news_url(response: &str) -> Option<String> {
    let normalized = response.replace("\\\"", "\"").replace("\\/", "/");
    let url = extract_between(&normalized, "[\"garturlres\",\"", "\",")?.trim();
    let parsed = Url::parse(url).ok()?;
    match parsed.scheme() {
        "http" | "https" => Some(url.to_string()),
        _ => None,
    }
}

fn post_google_news_batchexecute_reqwest(body: &str) -> Result<String, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?
        .post("https://news.google.com/_/DotsSplashUi/data/batchexecute?rpcids=Fbv4je")
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded;charset=UTF-8",
        )
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        )
        .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .header(reqwest::header::REFERER, "https://news.google.com/")
        .header(reqwest::header::ORIGIN, "https://news.google.com")
        .header("X-Same-Domain", "1")
        .body(body.to_string())
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .text()
        .map_err(|e| e.to_string())
}

fn post_google_news_batchexecute_curl(body: &str) -> Result<String, String> {
    let headers = [
        "Content-Type: application/x-www-form-urlencoded;charset=UTF-8",
        "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        "Accept-Language: en-US,en;q=0.9",
        "Referer: https://news.google.com/",
        "Origin: https://news.google.com",
        "X-Same-Domain: 1",
    ];
    let bytes = crate::curl_client::CurlClient::post_form_impersonated(
        "https://news.google.com/_/DotsSplashUi/data/batchexecute?rpcids=Fbv4je",
        body,
        &headers,
    )
    .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

pub(crate) fn resolve_google_news_article_url_blocking(
    url: &str,
) -> Result<Option<String>, String> {
    if !is_google_news_article_url(url) {
        return Ok(None);
    }
    let article_id = match extract_google_news_article_id(url) {
        Some(id) => id,
        None => {
            log_debug(&format!(
                "google_news_decode skip reason=article_id_missing url=\"{}\"",
                url
            ));
            return Ok(None);
        }
    };

    let html = fetch_google_news_article_page_html(url)?;
    if let Some(decoded) = extract_google_news_direct_url_from_article_html(&html) {
        log_debug(&format!(
            "google_news_decode ok(from_html) from=\"{}\" to=\"{}\"",
            url, decoded
        ));
        return Ok(Some(decoded));
    }
    let (signature, timestamp) = match extract_google_news_tokens(&html) {
        Some(tokens) => tokens,
        None => {
            log_debug(&format!(
                "google_news_decode skip reason=tokens_missing url=\"{}\"",
                url
            ));
            return Ok(None);
        }
    };

    let req_inner = format!(
        r#"["garturlreq",[["en-US","US",["WEB_TEST_1_0_0"],null,null,1,1,"US:en",null,180,null,null,null,null,null,0,null,null,[1608992183,723341000]],"en-US","US",1,[2,3,4,8],1,0,"655000234",0,0,null,0],"{article_id}",{timestamp},"{signature}"]"#
    );
    let req_inner_json = serde_json::to_string(&req_inner).map_err(|e| e.to_string())?;
    let f_req = format!(r#"[[["Fbv4je",{}]]]"#, req_inner_json);
    let body = format!("f.req={}", encode_form_value(&f_req));

    let mut last_err = String::new();
    let mut response: Option<String> = None;
    for _attempt in 0..2 {
        match post_google_news_batchexecute_reqwest(&body) {
            Ok(text) => {
                response = Some(text);
                break;
            }
            Err(e) => {
                last_err = e;
            }
        }
        std::thread::sleep(Duration::from_millis(350));
    }
    if response.is_none() {
        match post_google_news_batchexecute_curl(&body) {
            Ok(text) => {
                log_debug("google_news_decode: batchexecute resolved via curl fallback");
                response = Some(text);
            }
            Err(err) => {
                if !last_err.is_empty() {
                    last_err = format!("{last_err} | curl_fallback: {err}");
                } else {
                    last_err = err;
                }
            }
        }
    }
    let response = response.ok_or(last_err)?;

    let decoded = match extract_decoded_google_news_url(&response) {
        Some(v) => v,
        None => {
            log_debug(&format!(
                "google_news_decode skip reason=decoded_url_missing url=\"{}\"",
                url
            ));
            return Ok(None);
        }
    };
    if decoded == url {
        log_debug(&format!(
            "google_news_decode skip reason=same_url url=\"{}\"",
            url
        ));
        return Ok(None);
    }
    log_debug(&format!(
        "google_news_decode ok from=\"{}\" to=\"{}\"",
        url, decoded
    ));
    Ok(Some(decoded))
}

fn parse_podcast_feed_bytes(
    bytes: Vec<u8>,
    fallback_title: &str,
    language: Language,
) -> Option<(String, Vec<PodcastEpisode>)> {
    let cursor = Cursor::new(bytes);
    let feed = match parser::parse(cursor) {
        Ok(f) => f,
        Err(e) => {
            crate::log_debug(&format!("DEBUG: parser::parse failed: {:?}", e));
            return None;
        }
    };
    let title = feed
        .title
        .map(|t| t.content)
        .unwrap_or_else(|| fallback_title.to_string());
    let title = decode_basic_html_entities(&title);
    let items = feed
        .entries
        .into_iter()
        .map(|entry| {
            let title = entry
                .title
                .as_ref()
                .map(|t| t.content.clone())
                .unwrap_or_else(|| i18n::tr(language, "reader.no_title_fallback"));
            let title = decode_basic_html_entities(&title);
            let link = select_entry_link(&entry);
            let guid = if !entry.id.trim().is_empty() {
                entry.id.clone()
            } else if !link.trim().is_empty() {
                link.clone()
            } else {
                title.clone()
            };
            let description = entry
                .summary
                .as_ref()
                .map(|s| s.content.clone())
                .or_else(|| entry.content.as_ref().and_then(|c| c.body.clone()))
                .unwrap_or_default();
            let description = decode_basic_html_entities(&description);

            let (enclosure_url, enclosure_type) = select_podcast_enclosure(&entry);
            let (chapters_url, chapters_type) = select_podcast_chapters_link(&entry);
            let pub_date = entry.published.or(entry.updated).map(|d| d.timestamp());
            PodcastEpisode {
                title,
                link,
                description,
                guid,
                enclosure_url,
                enclosure_type,
                chapters_url,
                chapters_type,
                podlove_chapters: Vec::new(),
                pub_date,
            }
        })
        .collect();
    Some((title, items))
}

fn select_podcast_enclosure(entry: &feed_rs::model::Entry) -> (Option<String>, Option<String>) {
    if let Some(content) = entry.content.as_ref()
        && let Some(src) = content.src.as_ref()
    {
        return (
            Some(src.href.clone()),
            Some(content.content_type.to_string()),
        );
    }
    for link in &entry.links {
        if let Some(rel) = link.rel.as_deref()
            && rel.eq_ignore_ascii_case("enclosure")
        {
            return (Some(link.href.clone()), link.media_type.clone());
        }
    }
    for link in &entry.links {
        if let Some(media_type) = link.media_type.as_deref()
            && (media_type.starts_with("audio/") || media_type.starts_with("video/"))
        {
            return (Some(link.href.clone()), link.media_type.clone());
        }
    }
    // First pass: prefer URLs with audio file extensions (.mp3, .m4a, etc.) or audio path
    for media in &entry.media {
        for content in &media.content {
            if let Some(url) = content.url.as_ref() {
                let url_str = url.as_str().to_lowercase();
                let has_audio_ext = url_str.ends_with(".mp3")
                    || url_str.ends_with(".m4a")
                    || url_str.ends_with(".aac")
                    || url_str.ends_with(".ogg")
                    || url_str.ends_with(".opus")
                    || url_str.ends_with(".wav")
                    || url_str.contains("/audio.mp3")
                    || url_str.contains("/audio.");
                let is_embed = url_str.contains("/embed");
                if has_audio_ext && !is_embed {
                    let media_type = content.content_type.as_ref().map(|m| m.to_string());
                    return (Some(url.to_string()), media_type);
                }
            }
        }
    }
    // Second pass: take any audio/video content that's not an embed page
    for media in &entry.media {
        for content in &media.content {
            if let Some(url) = content.url.as_ref() {
                let url_str = url.as_str().to_lowercase();
                let is_embed = url_str.contains("/embed");
                if let Some(media_type) = content.content_type.as_ref() {
                    let mt = media_type.to_string().to_lowercase();
                    if (mt.starts_with("audio/") || mt.starts_with("video/")) && !is_embed {
                        return (Some(url.to_string()), Some(media_type.to_string()));
                    }
                }
            }
        }
    }
    // Fallback: take first media content but skip embed URLs
    for media in &entry.media {
        for content in &media.content {
            if let Some(url) = content.url.as_ref() {
                let url_str = url.as_str().to_lowercase();
                if !url_str.contains("/embed") {
                    let media_type = content.content_type.as_ref().map(|m| m.to_string());
                    return (Some(url.to_string()), media_type);
                }
            }
        }
    }
    (None, None)
}

fn select_podcast_chapters_link(entry: &feed_rs::model::Entry) -> (Option<String>, Option<String>) {
    for link in &entry.links {
        let rel = link.rel.as_deref().unwrap_or("").to_lowercase();
        let href = link.href.to_lowercase();
        if rel.contains("chapters") || href.contains("/chapters/") {
            return (Some(link.href.clone()), link.media_type.clone());
        }
        if let Some(media_type) = link.media_type.as_deref()
            && media_type.eq_ignore_ascii_case("application/json")
            && (rel.contains("podcast") || href.contains("chapters"))
        {
            return (Some(link.href.clone()), link.media_type.clone());
        }
    }
    (None, None)
}

fn select_entry_link(entry: &feed_rs::model::Entry) -> String {
    for link in &entry.links {
        let href = link.href.trim();
        if href.is_empty() {
            continue;
        }
        let rel = link.rel.as_deref().unwrap_or("");
        if rel.is_empty() || rel.eq_ignore_ascii_case("alternate") {
            return href.to_string();
        }
    }
    if let Some(link) = entry.links.iter().find(|l| !l.href.trim().is_empty()) {
        return link.href.clone();
    }
    String::new()
}

fn truncate_excerpt(input: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in input.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(ch);
    }
    out
}

fn dedup_items(items: Vec<RssItem>, max_items: usize) -> Vec<RssItem> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let key = if !item.guid.trim().is_empty() {
            format!("guid:{}", item.guid.trim())
        } else {
            format!("link:{}", canonicalize_url(&item.link))
        };
        if seen.insert(key) {
            out.push(item);
            if out.len() >= max_items {
                break;
            }
        }
    }
    out
}

fn dedup_podcast_items(items: Vec<PodcastEpisode>, max_items: usize) -> Vec<PodcastEpisode> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let key = if !item.guid.trim().is_empty() {
            format!("guid:{}", item.guid.trim())
        } else {
            format!("link:{}", canonicalize_url(&item.link))
        };
        if seen.insert(key) {
            out.push(item);
            if out.len() >= max_items {
                break;
            }
        }
    }
    out
}

async fn fetch_bytes_with_retries(
    http: &RssHttp,
    url: &str,
    is_feed: bool,
    language: Language,
    mut cache: Option<&mut RssFeedCache>,
) -> Result<FetchBytesOutcome, FeedFetchError> {
    let host = host_from_url(url).unwrap_or_else(|| "unknown".to_string());
    let max_attempts = http.config.max_retries + 1;

    for attempt in 1..=max_attempts {
        let response = {
            let _permits = http.acquire_permits(&host).await.map_err(|e| {
                let cache = cache.as_deref().cloned().unwrap_or_default();
                FeedFetchError::Network {
                    message: e,
                    cache: Box::new(cache),
                }
            })?;
            let mut req = http.client.get(url);
            if is_feed && let Some(c) = cache.as_ref() {
                if let Some(etag) = c.etag.as_deref() {
                    req = req.header(IF_NONE_MATCH, etag);
                }
                if let Some(m) = c.last_modified.as_deref() {
                    req = req.header(IF_MODIFIED_SINCE, m);
                }
            }
            req.send().await
        };

        match response {
            Ok(resp) => {
                let status = resp.status();
                let headers = resp.headers().clone();
                if let Some(c) = cache.as_deref_mut() {
                    c.last_fetch = Some(now_unix());
                    c.last_status = Some(status.as_u16());
                }
                if status == StatusCode::NOT_MODIFIED && is_feed {
                    return Ok(FetchBytesOutcome {
                        bytes: Vec::new(),
                        not_modified: true,
                    });
                }
                if !status.is_success() {
                    if should_retry_status(status) && attempt < max_attempts {
                        sleep(compute_backoff(attempt - 1, http.config.backoff_max_secs)).await;
                        continue;
                    }
                    let cache = cache.as_deref().cloned().unwrap_or_default();
                    return Err(FeedFetchError::HttpStatus {
                        status: status.as_u16(),
                        kind: "http_error".to_string(),
                        cache: Box::new(cache),
                    });
                }
                let bytes = resp
                    .bytes()
                    .await
                    .map_err(|e| {
                        let cache = cache.as_deref().cloned().unwrap_or_default();
                        FeedFetchError::Network {
                            message: e.to_string(),
                            cache: Box::new(cache),
                        }
                    })?
                    .to_vec();
                if let Some(c) = cache.as_deref_mut() {
                    c.consecutive_failures = 0;
                    if let Some(etag) = headers.get(ETAG).and_then(|v| v.to_str().ok()) {
                        c.etag = Some(etag.to_string());
                    }
                    if let Some(m) = headers.get(LAST_MODIFIED).and_then(|v| v.to_str().ok()) {
                        c.last_modified = Some(m.to_string());
                    }
                }
                return Ok(FetchBytesOutcome {
                    bytes,
                    not_modified: false,
                });
            }
            Err(err) => {
                if attempt < max_attempts {
                    sleep(compute_backoff(attempt - 1, http.config.backoff_max_secs)).await;
                    continue;
                }
                let cache = cache.as_deref().cloned().unwrap_or_default();
                return Err(FeedFetchError::Network {
                    message: format_error_chain(&err),
                    cache: Box::new(cache),
                });
            }
        }
    }
    let cache = cache.as_deref().cloned().unwrap_or_default();
    Err(FeedFetchError::Network {
        message: i18n::tr(language, "rss.error.retries_exhausted"),
        cache: Box::new(cache),
    })
}

fn extract_feed_links(html: &str, base_url: &str) -> Vec<String> {
    let mut links = Vec::new();
    let document = Html::parse_document(html);
    if let Ok(selector) = Selector::parse("link[rel~='alternate'][href]") {
        for element in document.select(&selector) {
            if let Some(type_attr) = element.value().attr("type") {
                let t = type_attr.to_lowercase();
                if (t.contains("rss") || t.contains("atom") || t.contains("xml"))
                    && let Some(href) = element.value().attr("href")
                {
                    if let Ok(u) = Url::parse(base_url)
                        && let Ok(joined) = u.join(href)
                    {
                        links.push(joined.to_string());
                    } else if href.starts_with("http") {
                        links.push(href.to_string());
                    }
                }
            }
        }
    }
    links
}

struct FetchBytesOutcome {
    bytes: Vec<u8>,
    not_modified: bool,
}

pub async fn fetch_and_parse(
    url: &str,
    _source_kind: RssSourceType,
    cache: RssFeedCache,
    fetch_config: RssFetchConfig,
    _override_cooldown: bool,
    language: Language,
) -> Result<RssFetchOutcome, FeedFetchError> {
    let mut url = normalize_url(url);
    let http = shared_http().map_err(|e| FeedFetchError::Network {
        message: e,
        cache: Box::new(cache.clone()),
    })?;
    let mut cache = cache;

    if is_corriere_home_feed_url(&url) && url != CORRIERE_HOME_FEED_URL {
        crate::log_debug(&format!(
            "rss_corriere_home_feed_override from=\"{}\" to=\"{}\"",
            url, CORRIERE_HOME_FEED_URL
        ));
        url = CORRIERE_HOME_FEED_URL.to_string();
        cache.etag = None;
        cache.last_modified = None;
    }

    // Match Mobile Starter and repair subscriptions saved with the defunct publisher feed.
    if is_legacy_il_giornale_feed_url(&url) {
        crate::log_debug(&format!(
            "rss_il_giornale_feed_override from={url} to={IL_GIORNALE_FEED_URL}"
        ));
        url = IL_GIORNALE_FEED_URL.to_string();
        cache.etag = None;
        cache.last_modified = None;
    }

    let out_result = fetch_bytes_with_retries(http, &url, true, language, Some(&mut cache)).await;

    let out = match out_result {
        Ok(o) => o,
        Err(e) => {
            let (returned_cache, msg) = match e {
                FeedFetchError::HttpStatus { cache, status, .. } => {
                    (cache, format!("HTTP {}", status))
                }
                FeedFetchError::Network { cache, message } => (cache, message),
            };
            cache = *returned_cache;
            crate::log_debug(&format!(
                "Standard fetch failed for {}, trying CurlClient. Error: {}",
                url, msg
            ));
            match fetch_url_bytes(&url, fetch_config).await {
                Ok(bytes) => {
                    cache.etag = None;
                    cache.last_modified = None;
                    cache.last_fetch = Some(now_unix());
                    cache.last_status = Some(200);
                    cache.consecutive_failures = 0;
                    FetchBytesOutcome {
                        bytes,
                        not_modified: false,
                    }
                }
                Err(curl_err) => {
                    let msg = match curl_err {
                        FeedFetchError::HttpStatus { status, .. } => format!("HTTP {}", status),
                        FeedFetchError::Network { message, .. } => message,
                    };
                    return Err(FeedFetchError::Network {
                        message: msg,
                        cache: Box::new(cache),
                    });
                }
            }
        }
    };

    if out.not_modified {
        return Ok(RssFetchOutcome {
            kind: RssSourceType::Feed,
            title: String::new(),
            items: Vec::new(),
            cache,
            not_modified: true,
        });
    }

    if let Some((title, items)) = parse_feed_bytes(
        out.bytes.clone(),
        &url,
        fetch_config.max_excerpt_chars,
        language,
    ) {
        return Ok(RssFetchOutcome {
            kind: RssSourceType::Feed,
            title,
            items: dedup_items(items, fetch_config.max_items_per_feed),
            cache,
            not_modified: false,
        });
    }

    // HTML Discovery
    let html = decode_html_bytes(&out.bytes);
    let feed_links = extract_feed_links(&html, &url);
    for feed_link in feed_links {
        crate::log_debug(&format!("Discovering feed at: {}", feed_link));
        let sub_out_result = fetch_bytes_with_retries(http, &feed_link, true, language, None).await;

        let sub_bytes = match sub_out_result {
            Ok(o) => o.bytes,
            Err(_) => match fetch_url_bytes(&feed_link, fetch_config).await {
                Ok(b) => b,
                Err(_) => continue,
            },
        };

        if let Some((title, items)) = parse_feed_bytes(
            sub_bytes,
            &feed_link,
            fetch_config.max_excerpt_chars,
            language,
        ) {
            cache.feed_url = Some(feed_link);
            return Ok(RssFetchOutcome {
                kind: RssSourceType::Feed,
                title,
                items: dedup_items(items, fetch_config.max_items_per_feed),
                cache,
                not_modified: false,
            });
        }
    }

    Err(FeedFetchError::Network {
        message: "Parsing failed".to_string(),
        cache: Box::new(cache),
    })
}

pub async fn fetch_url_bytes(
    url: &str,
    _fetch_config: RssFetchConfig,
) -> Result<Vec<u8>, FeedFetchError> {
    let url_str = normalize_url(url);
    let bytes_res = tokio::task::spawn_blocking(move || {
        crate::curl_client::CurlClient::fetch_url_impersonated(&url_str).map_err(|e| e.to_string())
    })
    .await;

    match bytes_res {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(err)) => Err(FeedFetchError::Network {
            message: err,
            cache: Box::new(RssFeedCache::default()),
        }),
        Err(err) => Err(FeedFetchError::Network {
            message: err.to_string(),
            cache: Box::new(RssFeedCache::default()),
        }),
    }
}

pub fn download_url_to_file_with_progress<F: FnMut(u32)>(
    url: &str,
    destination: &std::path::Path,
    resume_from: u64,
    progress_cb: F,
) -> Result<u64, String> {
    let url_str = normalize_url(url);
    log_debug(&format!(
        "download_url_to_file_with_progress: calling impersonated for {} resume_from={} destination={}",
        url_str,
        resume_from,
        destination.display()
    ));
    crate::curl_client::CurlClient::fetch_url_to_file_with_progress(
        &url_str,
        destination,
        resume_from,
        progress_cb,
    )
    .map_err(|e| e.to_string())
}

pub async fn fetch_article_text(
    url: &str,
    fallback_title: &str,
    fallback_description: &str,
    language: crate::settings::Language,
) -> Result<String, String> {
    let start_total = Instant::now();
    let mut url_str = normalize_url(url);
    if url_str.is_empty() {
        return Err(i18n::tr(language, "rss.error.empty_url"));
    }
    if is_google_news_article_url(&url_str) {
        log_debug(&format!(
            "rss_article_fetch google_news_resolve_attempt url=\"{}\"",
            url_str
        ));
        let original = url_str.clone();
        match tokio::task::spawn_blocking(move || {
            resolve_google_news_article_url_blocking(&original)
        })
        .await
        {
            Ok(Ok(Some(decoded))) => {
                log_debug(&format!(
                    "rss_article_fetch google_news_resolved from=\"{}\" to=\"{}\"",
                    url_str, decoded
                ));
                url_str = decoded;
            }
            Ok(Ok(None)) => {}
            Ok(Err(err)) => {
                log_debug(&format!(
                    "rss_article_fetch google_news_resolve_failed error=\"{err}\""
                ));
            }
            Err(err) => {
                log_debug(&format!(
                    "rss_article_fetch google_news_resolve_join_failed error=\"{}\"",
                    err
                ));
            }
        }
    }

    log_debug(&format!(
        "rss_article_fetch starting via curl-impersonate url=\"{url_str}\""
    ));
    let url_for_curl = url_str.clone();
    let bytes_res = tokio::task::spawn_blocking(move || {
        crate::curl_client::CurlClient::fetch_url_impersonated(&url_for_curl)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?;

    let html = match bytes_res {
        Ok(bytes) => {
            let s = decode_html_bytes(&bytes);
            // DEBUG: Salva l'HTML grezzo in un file vicino all'exe
            #[cfg(debug_assertions)]
            if let Ok(mut exe_path) = std::env::current_exe() {
                exe_path.set_file_name("debug_last_fetch.txt");
                crate::log_if_err!(std::fs::write(exe_path, &s));
            }
            s
        }
        Err(err) => {
            log_debug(&format!(
                "rss_article_fetch curl_failed url=\"{url_str}\" error=\"{err}\""
            ));
            return Err(err);
        }
    };

    let mut article =
        reader::reader_mode_extract(&html, language).unwrap_or(reader::ArticleContent {
            title: fallback_title.to_string(),
            content: fallback_description.to_string(),
        });

    let weak_extraction = article.content.trim().chars().count() < 80
        || article.content.trim() == fallback_description.trim()
        || article.content.trim().is_empty();
    if weak_extraction {
        log_debug(&format!(
            "rss_article_fetch weak_extraction_retry_iphone url=\"{url_str}\" content_len={}",
            article.content.trim().chars().count()
        ));
        let url_for_iphone = url_str.clone();
        let iphone_bytes_res = tokio::task::spawn_blocking(move || {
            crate::curl_client::CurlClient::fetch_url_iphone_impersonated(&url_for_iphone)
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?;

        if let Ok(iphone_bytes) = iphone_bytes_res {
            let iphone_html = decode_html_bytes(&iphone_bytes);
            let iphone_article = reader::reader_mode_extract(&iphone_html, language).unwrap_or(
                reader::ArticleContent {
                    title: fallback_title.to_string(),
                    content: fallback_description.to_string(),
                },
            );
            if iphone_article.content.trim().chars().count()
                > article.content.trim().chars().count()
            {
                article = iphone_article;
            }
        }
    }
    log_debug(&format!(
        "rss_article_fetch_done ms={} url=\"{url_str}\"",
        start_total.elapsed().as_millis()
    ));
    Ok(format!("{}\n\n{}", article.title, article.content))
}
pub fn config_from_settings(settings: &crate::settings::AppSettings) -> RssHttpConfig {
    RssHttpConfig {
        global_max_concurrency: settings.rss_global_max_concurrency,
        per_host_max_concurrency: settings.rss_per_host_max_concurrency,
        max_retries: settings.rss_max_retries,
        backoff_max_secs: settings.rss_backoff_max_secs,
    }
}

pub fn fetch_config_from_settings(settings: &crate::settings::AppSettings) -> RssFetchConfig {
    RssFetchConfig {
        max_items_per_feed: settings.rss_max_items_per_feed,
        max_excerpt_chars: settings.rss_max_excerpt_chars,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn il_giornale_legacy_feed_matches_only_the_retired_endpoint() {
        for url in [
            "https://www.ilgiornale.it/rss.xml",
            "http://ilgiornale.it/rss.xml",
        ] {
            assert!(super::is_legacy_il_giornale_feed_url(url));
        }
        for url in [
            super::IL_GIORNALE_FEED_URL,
            "https://www.ilgiornale.it/news/article.html",
            "https://blog.ilgiornale.it/rss.xml",
            "https://ilgiornale.it.example.org/rss.xml",
            "https://www.ilgiornale.it/rss.xml?section=sport",
        ] {
            assert!(!super::is_legacy_il_giornale_feed_url(url));
        }
        assert!(include_str!("../../i18n/feed_it.txt").contains(super::IL_GIORNALE_FEED_URL));
    }

    use super::*;

    #[test]
    fn decode_html_bytes_preserves_spanish_accents_from_elmundo_fixture() {
        let bytes = std::fs::read("tests/fixtures/rss/elmundo_first_article.html")
            .expect("failed to read elmundo article fixture");
        let decoded = decode_html_bytes(&bytes);
        assert!(
            decoded.chars().any(|c| matches!(
                c,
                'á' | 'é' | 'í' | 'ó' | 'ú' | 'ñ' | 'Á' | 'É' | 'Í' | 'Ó' | 'Ú' | 'Ñ'
            )),
            "decoded article does not contain expected Spanish accented characters"
        );
    }

    #[test]
    fn parse_elmundo_feed_fixture_extracts_items() {
        let bytes = std::fs::read("tests/fixtures/rss/elmundo_portada.xml")
            .expect("failed to read elmundo feed fixture");
        let parsed = parse_feed_bytes(
            bytes,
            "http://estaticos.elmundo.es/elmundo/rss/portada.xml",
            512,
            crate::settings::Language::English,
        );
        let Some((title, items)) = parsed else {
            panic!("failed to parse elmundo feed fixture");
        };
        assert!(!title.trim().is_empty(), "feed title is empty");
        assert!(!items.is_empty(), "feed has no items");
    }

    #[test]
    fn parse_feed_bytes_decodes_html_entities_in_titles_and_descriptions() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>marca &amp; portada</title>
    <item>
      <title>Test &quot;ok&quot;</title>
      <link>https://example.com/a</link>
      <description>Desc &amp; details</description>
      <guid>g1</guid>
    </item>
  </channel>
</rss>"#;
        let parsed = parse_feed_bytes(
            xml.as_bytes().to_vec(),
            "fallback",
            512,
            crate::settings::Language::English,
        )
        .expect("failed to parse inline rss fixture");
        let (feed_title, items) = parsed;
        assert_eq!(feed_title, "marca & portada");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Test \"ok\"");
        assert_eq!(items[0].description, "Desc & details");
    }

    #[test]
    fn decode_basic_html_entities_handles_quote_alias() {
        let decoded = decode_basic_html_entities("Test &quote;ok&quote; &amp; more");
        assert_eq!(decoded, "Test \"ok\" & more");
    }

    #[test]
    fn corriere_home_feed_detection_only_matches_home_variants() {
        assert!(is_corriere_home_feed_url("https://www.corriere.it/"));
        assert!(is_corriere_home_feed_url(
            "https://www.corriere.it/rss/homepage.xml"
        ));
        assert!(is_corriere_home_feed_url(
            "https://xml2.corriereobjects.it/feed-hp/homepage.xml"
        ));
        assert!(is_corriere_home_feed_url(CORRIERE_HOME_FEED_URL));
        assert!(!is_corriere_home_feed_url(
            "https://www.corriere.it/dynamic-feed/rss/section/Politica.xml"
        ));
        assert!(!is_corriere_home_feed_url(
            "https://www.corriere.it/cronache/"
        ));
    }

    #[test]
    fn reader_extract_from_elmundo_fixture_keeps_spanish_accents() {
        let bytes = std::fs::read("tests/fixtures/rss/elmundo_first_article.html")
            .expect("failed to read elmundo article fixture");
        let html = decode_html_bytes(&bytes);
        let article = reader::reader_mode_extract(&html, crate::settings::Language::Spanish)
            .unwrap_or_else(|| panic!("reader_mode_extract failed on elmundo article fixture"));

        let combined = format!("{} {}", article.title, article.content);
        assert!(
            combined.chars().any(|c| matches!(
                c,
                'á' | 'é' | 'í' | 'ó' | 'ú' | 'ñ' | 'Á' | 'É' | 'Í' | 'Ó' | 'Ú' | 'Ñ'
            )),
            "reader output does not contain expected Spanish accented characters"
        );
        assert!(
            article.content.trim().chars().count() >= 120,
            "reader output is unexpectedly short"
        );
    }

    #[test]
    fn reader_extract_from_ilriformista_fixture_keeps_full_body() {
        let html = std::fs::read_to_string("tests/fixtures/articles/084.html")
            .expect("failed to read il riformista article fixture");
        let article = reader::reader_mode_extract(&html, crate::settings::Language::Italian)
            .unwrap_or_else(|| panic!("reader_mode_extract failed on il riformista fixture"));

        assert!(
            article
                .title
                .contains("Bizzarro ma influente, la storia di Peter Thiel"),
            "unexpected il riformista title: {}",
            article.title
        );
        assert!(
            article
                .content
                .contains("La sua preparazione filosofica è impeccabile."),
            "extracted il riformista content is missing later body paragraphs: {}",
            article.content
        );
    }

    #[test]
    fn extract_google_news_article_id_parses_rss_articles_path() {
        let id = extract_google_news_article_id(
            "https://news.google.com/rss/articles/CBMiQGh0dHBzOi8vZXhhbXBsZS5jb20v?oc=5",
        );
        assert_eq!(id.as_deref(), Some("CBMiQGh0dHBzOi8vZXhhbXBsZS5jb20v"));
    }

    #[test]
    fn extract_decoded_google_news_url_parses_batchexecute_payload() {
        let payload = r#")]}'

[["wrb.fr","Fbv4je","[\"garturlres\",\"https://example.com/article\",1]",null,null,null,""]]"#;
        let decoded = extract_decoded_google_news_url(payload);
        assert_eq!(decoded.as_deref(), Some("https://example.com/article"));
    }

    #[test]
    fn extract_google_news_direct_url_from_html_parses_data_n_au() {
        let html = r#"<div data-n-au="https://www.limesonline.com/rubrica/dollaro"></div>"#;
        let decoded = extract_google_news_direct_url_from_article_html(html);
        assert_eq!(
            decoded.as_deref(),
            Some("https://www.limesonline.com/rubrica/dollaro")
        );
    }

    #[test]
    fn stable_google_news_guid_prefers_article_id_from_link() {
        let guid = stable_google_news_guid(
            "tag:news.google.com,2005:cluster=527802233",
            "https://news.google.com/rss/articles/CBMiQGh0dHBzOi8vZXhhbXBsZS5jb20v?hl=it&gl=IT&ceid=IT:it",
        );
        assert_eq!(
            guid.as_deref(),
            Some("google-news:CBMiQGh0dHBzOi8vZXhhbXBsZS5jb20v")
        );
    }

    #[test]
    fn stable_google_news_guid_uses_entry_id_when_it_is_google_link() {
        let guid = stable_google_news_guid(
            "https://news.google.com/rss/articles/CBMif2h0dHBzOi8vZXhhbXBsZS5vcmcvYXJ0aWNsZT9pZD0xMjPSAQA?oc=5",
            "",
        );
        assert_eq!(
            guid.as_deref(),
            Some("google-news:CBMif2h0dHBzOi8vZXhhbXBsZS5vcmcvYXJ0aWNsZT9pZD0xMjPSAQA")
        );
    }
}

// Stubs for missing podcast/itunes functions to keep file structure consistent
pub async fn fetch_podcast_feed(
    url: &str,
    cache: RssFeedCache,
    cfg: RssFetchConfig,
    _override_cooldown: bool,
    language: Language,
) -> Result<PodcastFetchOutcome, FeedFetchError> {
    let url = normalize_url(url);
    if crate::tools::raiplaysound::is_raiplaysound_url(&url) {
        return fetch_raiplaysound_podcast_page(&url, cache, cfg);
    }
    let http = shared_http().map_err(|e| FeedFetchError::Network {
        message: e,
        cache: Box::new(cache.clone()),
    })?;
    let mut cache = cache;

    let out_result = fetch_bytes_with_retries(http, &url, true, language, Some(&mut cache)).await;

    let out = match out_result {
        Ok(o) => o,
        Err(e) => {
            let (returned_cache, msg) = match e {
                FeedFetchError::HttpStatus { cache, status, .. } => {
                    (cache, format!("HTTP {}", status))
                }
                FeedFetchError::Network { cache, message } => (cache, message),
            };
            cache = *returned_cache;
            crate::log_debug(&format!(
                "Standard fetch failed for {}, trying CurlClient. Error: {}",
                url, msg
            ));
            match fetch_url_bytes(&url, cfg).await {
                Ok(bytes) => {
                    cache.etag = None;
                    cache.last_modified = None;
                    cache.last_fetch = Some(now_unix());
                    cache.last_status = Some(200);
                    cache.consecutive_failures = 0;
                    FetchBytesOutcome {
                        bytes,
                        not_modified: false,
                    }
                }
                Err(curl_err) => {
                    let msg = match curl_err {
                        FeedFetchError::HttpStatus { status, .. } => format!("HTTP {}", status),
                        FeedFetchError::Network { message, .. } => message,
                    };
                    return Err(FeedFetchError::Network {
                        message: msg,
                        cache: Box::new(cache),
                    });
                }
            }
        }
    };

    if out.not_modified {
        return Ok(PodcastFetchOutcome {
            title: String::new(),
            items: Vec::new(),
            cache,
        });
    }
    if let Some((title, items)) = parse_podcast_feed_bytes(out.bytes, &url, language) {
        return Ok(PodcastFetchOutcome {
            title,
            items: dedup_podcast_items(items, cfg.max_items_per_feed),
            cache,
        });
    }
    Err(FeedFetchError::Network {
        message: "Parsing failed".to_string(),
        cache: Box::new(cache),
    })
}

fn fetch_raiplaysound_podcast_page(
    url: &str,
    mut cache: RssFeedCache,
    cfg: RssFetchConfig,
) -> Result<PodcastFetchOutcome, FeedFetchError> {
    let page =
        crate::tools::raiplaysound::load_page(url).map_err(|message| FeedFetchError::Network {
            message,
            cache: Box::new(cache.clone()),
        })?;
    cache.etag = None;
    cache.last_modified = None;
    cache.last_fetch = Some(now_unix());
    cache.last_status = Some(200);
    cache.consecutive_failures = 0;

    let items = page
        .items
        .into_iter()
        .filter_map(|item| {
            let audio_url = item.audio_url?;
            let title = item.title.trim().to_string();
            if title.is_empty() || audio_url.trim().is_empty() {
                return None;
            }
            Some(PodcastEpisode {
                title,
                link: item.path_id.unwrap_or_else(|| audio_url.clone()),
                description: item.description.unwrap_or_default(),
                guid: item.id,
                enclosure_url: Some(audio_url),
                enclosure_type: Some("audio/mpeg".to_string()),
                chapters_url: None,
                chapters_type: None,
                podlove_chapters: Vec::new(),
                pub_date: None,
            })
        })
        .take(cfg.max_items_per_feed)
        .collect();

    Ok(PodcastFetchOutcome {
        title: page.title,
        items,
        cache,
    })
}

#[cfg(test)]
mod related_news_tests {
    use super::extract_google_news_related_items;

    #[test]
    fn extracts_google_news_related_sources() {
        let html = r#"<ol><li><a href="https://news.google.com/rss/articles/AAA">Título principal</a> Fonte A</li><li><a href="./rss/articles/BBB">Outro enfoque</a> Fonte B</li></ol>"#;
        let items = extract_google_news_related_items(
            html,
            "Título principal",
            "https://news.google.com/rss/articles/AAA",
            None,
        );
        assert_eq!(items.len(), 2);
        assert!(items[0].title.contains("Fonte A"));
        assert!(items[1].link.contains("news.google.com/rss/articles/BBB"));
    }

    #[test]
    fn ignores_single_google_news_link() {
        let html = r#"<ol><li><a href="https://news.google.com/rss/articles/AAA">Título</a> Fonte</li></ol>"#;
        let items = extract_google_news_related_items(
            html,
            "Título",
            "https://news.google.com/rss/articles/AAA",
            None,
        );
        assert!(items.is_empty());
    }
}
