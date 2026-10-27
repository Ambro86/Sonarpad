//! SonarMusic catalog client and native/PHP/yt-dlp search routing.
use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;

pub const ENDPOINT: &str = "https://sonarpad.com/api/youtube_music_resolve.php";
pub const TYPES: &[&str] = &["all", "song", "album", "artist", "playlist", "video"];

pub fn search_type_index(saved: &str) -> usize {
    TYPES.iter().position(|kind| *kind == saved).unwrap_or(0)
}

pub fn label(locale: &str, key: &str) -> String {
    static LABELS: OnceLock<Value> = OnceLock::new();
    let data = LABELS.get_or_init(|| {
        serde_json::from_str(include_str!("sonarmusic_labels.json")).unwrap_or(Value::Null)
    });
    data[locale][key]
        .as_str()
        .or_else(|| data["en"][key].as_str())
        .unwrap_or(key)
        .to_owned()
}
fn string(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().trim().to_owned()
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub video_id: String,
    pub browse_id: String,
    pub params: String,
    pub artist: String,
    pub duration: String,
    pub subtitle: String,
    pub views: String,
    pub available: bool,
}
impl Item {
    pub fn parse(v: &Value) -> Option<Self> {
        let video_id = string(v, "video_id");
        let id = string(v, "id");
        let id = if id.is_empty() { video_id.clone() } else { id };
        if id.is_empty() {
            return None;
        }
        let title = string(v, "title");
        Some(Self {
            kind: string(v, "kind"),
            title: if title.is_empty() { id.clone() } else { title },
            id,
            video_id,
            browse_id: string(v, "browse_id"),
            params: string(v, "browse_params"),
            artist: string(v, "artist"),
            duration: string(v, "duration"),
            subtitle: string(v, "subtitle"),
            views: v
                .get("views")
                .or_else(|| v.get("view_count"))
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .or_else(|| v.as_u64().map(|n| n.to_string()))
                        .unwrap_or_default()
                })
                .unwrap_or_default(),
            available: v["available"] != false,
        })
    }
    pub fn playable(&self) -> bool {
        self.available
            && matches!(self.kind.as_str(), "song" | "video" | "episode")
            && valid_video_id(&self.video_id)
    }
    pub fn watch_url(&self) -> Option<String> {
        self.playable()
            .then(|| format!("https://www.youtube.com/watch?v={}", self.video_id))
    }
    /// Public share link for a song, album, artist or playlist.
    pub fn share_url(&self) -> Option<String> {
        if let Some(url) = self.watch_url() {
            return Some(url);
        }
        let Request::Browse { id, .. } = self.browse_request()? else {
            return None;
        };
        if id.is_empty()
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return None;
        }
        if let Some(list) = id.strip_prefix("VL") {
            if list.is_empty() {
                return None;
            }
            return Some(format!("https://music.youtube.com/playlist?list={list}"));
        }
        if id.starts_with("UC") {
            return Some(format!("https://music.youtube.com/channel/{id}"));
        }
        Some(format!("https://music.youtube.com/browse/{id}"))
    }

    pub fn browse_request(&self) -> Option<Request> {
        if !self.available
            || !matches!(
                self.kind.as_str(),
                "album" | "artist" | "playlist" | "collection" | "podcast"
            )
        {
            return None;
        }
        let id = if self.browse_id.is_empty() {
            &self.id
        } else {
            &self.browse_id
        };
        Some(Request::Browse {
            id: normalize_browse_id(id),
            params: self.params.clone(),
        })
    }
    /// Apply the selected search filter when opening a saved artist.
    /// Individual albums and tracks retain their normal open action.
    pub fn favorite_search(&self, kind: &str) -> Option<Request> {
        if self.available && self.kind == "artist" && kind != "all" {
            Request::search(&self.title, kind).ok()
        } else {
            None
        }
    }
    pub fn display(&self, locale: &str) -> String {
        let language =
            serde_json::from_value::<crate::settings::Language>(serde_json::json!(locale))
                .unwrap_or(crate::settings::Language::English);
        let kind = label(locale, &self.kind);
        let metadata: Vec<_> = self
            .subtitle
            .split(['•', '·'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let clock = if !self.duration.is_empty() {
            self.duration.as_str()
        } else {
            metadata
                .iter()
                .copied()
                .find(|s| crate::result_labels::media_duration(s, language) != *s)
                .unwrap_or_default()
        };
        let mut parts = Vec::new();
        let mut seen = HashSet::new();
        let mut append = |value: String| {
            let value = value.trim();
            if !value.is_empty() && seen.insert(value.to_lowercase()) {
                parts.push(value.to_owned());
            }
        };
        append(self.title.clone());
        if TYPES.contains(&self.kind.as_str()) {
            append(kind.clone());
        }
        append(self.artist.clone());
        if !clock.is_empty() {
            append(format!(
                "{} {}",
                crate::i18n::tr(language, "stream_audio.duration_label"),
                crate::result_labels::media_duration(clock, language)
            ));
        }
        for piece in metadata {
            if piece == clock
                || piece.eq_ignore_ascii_case(&kind)
                || music_kind_descriptor(&self.kind, piece)
            {
                continue;
            }
            append(piece.into());
        }
        if !self.views.trim().is_empty() {
            let views = if self.views.parse::<u64>().is_ok() {
                format!(
                    "{} {}",
                    crate::i18n::tr(language, "stream_audio.views_label"),
                    self.views
                )
            } else {
                self.views.clone()
            };
            append(views);
        }
        parts.join(" — ")
    }

    fn key(&self) -> String {
        format!("{}:{}:{}", self.kind, self.id, self.params)
    }
}
fn valid_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn normalize_browse_id(id: &str) -> String {
    if ["VL", "MPRE", "MPAD", "UC", "MPSP", "MPED"]
        .iter()
        .any(|p| id.starts_with(p))
    {
        id.into()
    } else {
        format!("VL{id}")
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Search { query: String, kind: String },
    Browse { id: String, params: String },
}
impl Request {
    pub fn search(query: &str, kind: &str) -> Result<Self, String> {
        if query.trim().is_empty() {
            return Err("Empty search".into());
        }
        if !TYPES.contains(&kind) {
            return Err("Invalid search type".into());
        }
        Ok(Self::Search {
            query: query.trim().into(),
            kind: kind.into(),
        })
    }
    pub fn collection(&self) -> bool {
        matches!(self,Self::Browse{id,..} if id.starts_with("MPRE") || id.starts_with("VL"))
    }
    pub fn parameters(&self, token: Option<&str>, page: usize) -> Vec<(String, String)> {
        let mut p = match self {
            Self::Search { query, kind } => {
                vec![("q".into(), query.clone()), ("type".into(), kind.clone())]
            }
            Self::Browse { id, params } => {
                let mut p = vec![("browse".into(), id.clone())];
                if !params.is_empty() {
                    p.push(("params".into(), params.clone()));
                }
                p
            }
        };
        p.push(("page".into(), page.max(1).to_string()));
        if let Some(token) = token.filter(|t| !t.is_empty()) {
            p.push(("token".into(), token.into()));
        }
        p
    }
}
#[derive(Clone, Debug, Default)]
pub struct Page {
    pub items: Vec<Item>,
    pub next: Option<String>,
}
impl Page {
    pub fn parse(data: &Value, collection: bool) -> Result<Self, String> {
        if data["ok"] != true {
            return Err(format!("SonarMusic: {}", string(data, "error")));
        }
        let mut items = Vec::new();
        let mut seen = HashSet::new();
        let mut add = |v: &Value| {
            if let Some(rows) = v.as_array() {
                for row in rows {
                    if let Some(item) = Item::parse(row)
                        && seen.insert(item.key())
                    {
                        items.push(item);
                    }
                }
            }
        };
        add(&data["items"]);
        // Album recommendations must never become the selected album's tracks.
        if !collection && let Some(sections) = data["sections"].as_array() {
            for section in sections {
                add(&section["items"]);
            }
        }
        let next = string(data, "next_token");
        Ok(Self {
            items,
            next: (!next.is_empty()).then_some(next),
        })
    }
    #[cfg(test)]
    pub fn append(&mut self, other: Self) {
        let mut seen: HashSet<_> = self.items.iter().map(Item::key).collect();
        self.items
            .extend(other.items.into_iter().filter(|i| seen.insert(i.key())));
        // A broken server returning the same continuation must not loop forever.
        self.next = if self.next == other.next {
            None
        } else {
            other.next
        };
    }
}
#[derive(Clone, Default)]
pub struct Pagination {
    previous: Vec<Page>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ResultRow {
    Previous,
    Item(usize),
    Next,
}
impl Pagination {
    pub fn can_return(&self, suggestions: bool, parent_list: bool) -> bool {
        suggestions || self.has_previous() || parent_list
    }
    pub fn has_previous(&self) -> bool {
        !self.previous.is_empty()
    }
    pub fn row(&self, page: &Page, index: usize) -> Option<ResultRow> {
        if !self.previous.is_empty() && index == 0 {
            return Some(ResultRow::Previous);
        }
        let index = index.checked_sub(usize::from(!self.previous.is_empty()))?;
        if index < page.items.len() {
            Some(ResultRow::Item(index))
        } else if index == page.items.len() && page.next.is_some() {
            Some(ResultRow::Next)
        } else {
            None
        }
    }
    pub fn labels(&self, page: &Page, locale: &str) -> Vec<String> {
        let mut rows = Vec::new();
        if !self.previous.is_empty() {
            rows.push(label(locale, "previous_results"));
        }
        rows.extend(page.items.iter().map(|i| i.display(locale)));
        if page.next.is_some() {
            rows.push(label(locale, "next_results"));
        }
        rows
    }
    pub fn advance(&mut self, current: &mut Page, mut next: Page) {
        if next.next == current.next {
            next.next = None;
        }
        self.previous.push(std::mem::replace(current, next));
    }
    pub fn back(&mut self, current: &mut Page) -> bool {
        if let Some(previous) = self.previous.pop() {
            *current = previous;
            true
        } else {
            false
        }
    }
}
#[derive(Clone)]
pub struct Client {
    http: reqwest::blocking::Client,
    endpoint: String,
    token: String,
    locale: String,
}
impl Client {
    pub fn new(locale: &str, token: &str) -> Result<Self, String> {
        Ok(Self {
            http: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(40))
                .user_agent("Mozilla/5.0 Sonarpad/1.0")
                .build()
                .map_err(|e| e.to_string())?,
            endpoint: ENDPOINT.into(),
            token: token.into(),
            locale: locale.into(),
        })
    }
    fn get(&self, mut params: Vec<(String, String)>) -> Result<Value, String> {
        let (hl, gl) = locale_pair(&self.locale);
        params.extend([
            ("format".into(), "json".into()),
            ("hl".into(), hl.into()),
            ("gl".into(), gl.into()),
        ]);
        let response = self
            .http
            .get(&self.endpoint)
            .query(&params)
            .header("Accept", "application/json")
            .header("X-Sonarpad-Route-Token", &self.token)
            .send()
            .map_err(|_| "SonarMusic: connection failed".to_string())?;
        if !response.status().is_success() {
            return Err(format!("SonarMusic HTTP {}", response.status().as_u16()));
        }
        let data: Value = response
            .json()
            .map_err(|_| "SonarMusic: invalid response".to_string())?;
        if data["ok"] != true {
            return Err(format!("SonarMusic: {}", string(&data, "error")));
        }
        Ok(data)
    }
    pub fn load(
        &self,
        request: &Request,
        token: Option<&str>,
        page: usize,
    ) -> Result<Page, String> {
        load_catalog(
            request,
            token,
            page,
            |cursor| crate::direct_catalog::music(request, cursor, &self.locale),
            |cursor, index| self.get(request.parameters(cursor, index)),
            |index| crate::direct_catalog::music_ytdlp(request, index),
        )
    }
    pub fn suggestions(&self, query: &str) -> Result<Vec<String>, String> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let data = self.get(vec![("suggest".into(), query.trim().into())])?;
        let mut seen = HashSet::new();
        Ok(data["suggestions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty() && seen.insert(s.to_lowercase()))
            .map(str::to_owned)
            .collect())
    }
}
fn load_catalog(
    request: &Request,
    token: Option<&str>,
    page: usize,
    mut native: impl FnMut(Option<&str>) -> Result<Value, String>,
    mut php: impl FnMut(Option<&str>, usize) -> Result<Value, String>,
    mut ytdlp: impl FnMut(usize) -> Result<Value, String>,
) -> Result<Page, String> {
    // A continuation belongs to its provider. When switching to PHP,
    // replay its earlier pages instead of submitting an internal cursor.
    if token.is_none() || token.is_some_and(|t| t.starts_with("native:")) {
        let raw = token.and_then(|t| t.strip_prefix("native:"));
        if let Ok(data) = native(raw) {
            let mut result = Page::parse(&data, request.collection())?;
            result.next = result.next.map(|t| format!("native:{t}"));
            return Ok(result);
        }
    }
    if !token.is_some_and(|t| t.starts_with("ytdlp:")) {
        let php_result = (|| {
            let mut previous = token
                .and_then(|t| t.strip_prefix("php:"))
                .map(str::to_owned);
            let replay = token.is_some_and(|t| t.starts_with("native:"));
            let start = if replay { 1 } else { page.max(1) };
            for index in start..=page.max(1) {
                let mut result =
                    Page::parse(&php(previous.as_deref(), index)?, request.collection())?;
                if index == page.max(1) {
                    result.next = result.next.map(|t| format!("php:{t}"));
                    return Ok(result);
                }
                previous = Some(
                    result
                        .next
                        .ok_or_else(|| "PHP pagination exhausted".to_string())?,
                );
            }
            Err("PHP pagination unavailable".to_string())
        })();
        if let Ok(result) = php_result {
            return Ok(result);
        }
    }
    Page::parse(&ytdlp(page)?, request.collection())
}
pub fn locale_pair(locale: &str) -> (&str, &str) {
    match locale {
        "it" => ("it", "IT"),
        "cs" => ("cs", "CZ"),
        "de" => ("de", "DE"),
        "es" => ("es", "ES"),
        "fr" => ("fr", "FR"),
        "pt" => ("pt", "PT"),
        "pt-BR" | "pt_br" => ("pt", "BR"),
        "pl" => ("pl", "PL"),
        "tr" => ("tr", "TR"),
        "ro" => ("ro", "RO"),
        "uk" => ("uk", "UA"),
        "zh" | "zh-CN" | "zh_cn" => ("zh-CN", "CN"),
        "id" => ("id", "ID"),
        "ru" => ("ru", "RU"),
        "hi" => ("hi", "IN"),
        "sv" => ("sv", "SE"),
        "vi" => ("vi", "VN"),
        "sr" => ("sr", "RS"),
        "lt" => ("lt", "LT"),
        _ => ("en", "US"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;
    fn song() -> Value {
        json!({"id":"LX5Z7R6rt_I","video_id":"LX5Z7R6rt_I","kind":"song","title":"Wish You Were Here","artist":"Pink Floyd","duration":"5:35"})
    }
    fn item() -> Item {
        Item::parse(&song()).unwrap()
    }
    fn catalog_page(title: &str, next: Option<&str>) -> Page {
        let mut track = item();
        track.title = title.into();
        Page {
            items: vec![track],
            next: next.map(str::to_owned),
        }
    }
    #[test]
    fn saved_search_type_restores_all_choices_and_defaults_unknown_values() {
        for (index, kind) in TYPES.iter().enumerate() {
            assert_eq!(search_type_index(kind), index);
        }
        assert_eq!(search_type_index(""), 0);
        assert_eq!(search_type_index("not-a-type"), 0);
    }
    #[test]
    fn escape_returns_from_second_search_page_without_an_album_parent() {
        let mut navigation = Pagination::default();
        let mut page = catalog_page("first", Some("one"));
        assert!(!navigation.can_return(false, false));
        navigation.advance(&mut page, catalog_page("second", None));
        assert!(navigation.can_return(false, false));
        assert!(navigation.back(&mut page));
        assert_eq!(page.items[0].title, "first");
        assert!(!navigation.can_return(false, false));
        assert!(navigation.can_return(false, true));
        assert!(navigation.can_return(true, false));
    }
    #[test]
    fn pagination_first_page_has_track_then_next() {
        let nav = Pagination::default();
        let page = catalog_page("first", Some("token"));
        assert_eq!(nav.row(&page, 0), Some(ResultRow::Item(0)));
        assert_eq!(nav.row(&page, 1), Some(ResultRow::Next));
        assert_eq!(nav.row(&page, 2), None);
    }
    #[test]
    fn pagination_second_page_replaces_tracks_and_offsets_selection() {
        let mut nav = Pagination::default();
        let mut page = catalog_page("first", Some("one"));
        nav.advance(&mut page, catalog_page("second", Some("two")));
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].title, "second");
        assert_eq!(nav.row(&page, 0), Some(ResultRow::Previous));
        assert_eq!(nav.row(&page, 1), Some(ResultRow::Item(0)));
        assert_eq!(nav.row(&page, 2), Some(ResultRow::Next));
    }
    #[test]
    fn pagination_last_page_has_no_next() {
        let mut nav = Pagination::default();
        let mut page = catalog_page("first", Some("one"));
        nav.advance(&mut page, catalog_page("last", None));
        assert_eq!(nav.row(&page, 0), Some(ResultRow::Previous));
        assert_eq!(nav.row(&page, 2), None);
    }
    #[test]
    fn pagination_back_restores_cached_page_and_continuation() {
        let mut nav = Pagination::default();
        let mut page = catalog_page("first", Some("one"));
        nav.advance(&mut page, catalog_page("second", Some("two")));
        nav.advance(&mut page, catalog_page("third", None));
        assert!(nav.back(&mut page));
        assert_eq!(page.items[0].title, "second");
        assert_eq!(page.next.as_deref(), Some("two"));
        assert!(nav.back(&mut page));
        assert_eq!(page.items[0].title, "first");
        assert_eq!(nav.row(&page, 0), Some(ResultRow::Item(0)));
        assert!(!nav.back(&mut page));
    }
    #[test]
    fn pagination_empty_page_still_allows_return() {
        let mut nav = Pagination::default();
        let mut page = catalog_page("first", Some("one"));
        nav.advance(&mut page, Page::default());
        assert_eq!(nav.row(&page, 0), Some(ResultRow::Previous));
        assert_eq!(nav.row(&page, 1), None);
    }
    #[test]
    fn pagination_repeated_token_does_not_offer_endless_next() {
        let mut nav = Pagination::default();
        let mut page = catalog_page("first", Some("one"));
        nav.advance(&mut page, catalog_page("second", Some("one")));
        assert!(page.next.is_none());
    }
    #[test]
    fn pagination_rows_use_localized_navigation_labels() {
        let mut nav = Pagination::default();
        let mut page = catalog_page("first", Some("one"));
        nav.advance(&mut page, catalog_page("second", Some("two")));
        let rows = nav.labels(&page, "it");
        assert_eq!(rows.first().unwrap(), "Vai ai risultati precedenti");
        assert_eq!(rows.last().unwrap(), "Vai ai risultati successivi");
        assert_eq!(rows.len(), 3);
    }
    #[test]
    #[ignore = "Requires network and SONARMUSIC_LIVE_TOKEN"]
    fn live_catalog_search_suggest_browse_and_next_page() {
        let token = std::env::var("SONARMUSIC_LIVE_TOKEN").unwrap();
        let client = Client::new("it", &token).unwrap();
        let suggestions = client.suggestions("pink floyd").unwrap();
        assert!(!suggestions.is_empty());
        println!("LIVE suggestions: {}", suggestions.len());
        let mut first_album = None;
        for kind in TYPES {
            let request = Request::search("Pink Floyd", kind).unwrap();
            let page = client.load(&request, None, 1).unwrap();
            assert!(!page.items.is_empty(), "No results for {kind}");
            println!("LIVE search {kind}: {}", page.items.len());
            if *kind == "album" {
                first_album = page.items.iter().find_map(Item::browse_request);
                if let Some(token) = &page.next {
                    let next = client.load(&request, Some(token), 2).unwrap();
                    println!("LIVE album page 2: {}", next.items.len());
                    assert!(!next.items.is_empty());
                }
            }
        }
        let album = first_album.unwrap();
        let tracks = client.load(&album, None, 1).unwrap();
        assert!(tracks.items.iter().any(Item::playable));
        assert!(
            tracks
                .items
                .iter()
                .all(|i| matches!(i.kind.as_str(), "song" | "video" | "episode"))
        );
        println!("LIVE album tracks: {}", tracks.items.len());
    }

    #[test]
    fn song_has_safe_playback_url() {
        assert_eq!(
            item().watch_url().unwrap(),
            "https://www.youtube.com/watch?v=LX5Z7R6rt_I"
        );
    }
    #[test]
    fn unavailable_song_cannot_play() {
        let mut v = song();
        v["available"] = json!(false);
        assert!(!Item::parse(&v).unwrap().playable());
    }
    #[test]
    fn malformed_video_id_is_not_a_url() {
        let mut v = song();
        v["video_id"] = json!("https://evil.test");
        assert!(Item::parse(&v).unwrap().watch_url().is_none());
    }
    #[test]
    fn missing_id_uses_video_id() {
        let mut v = song();
        v.as_object_mut().unwrap().remove("id");
        assert_eq!(Item::parse(&v).unwrap().id, "LX5Z7R6rt_I");
    }
    #[test]
    fn empty_rows_are_ignored() {
        assert!(Item::parse(&json!({})).is_none());
        assert!(Item::parse(&Value::Null).is_none());
    }
    #[test]
    fn missing_title_uses_id() {
        let mut v = song();
        v["title"] = json!(" ");
        assert_eq!(Item::parse(&v).unwrap().title, "LX5Z7R6rt_I");
    }
    #[test]
    fn album_is_browsable_not_playable() {
        let i = Item::parse(&json!({"id":"MPRE123","kind":"album"})).unwrap();
        assert!(!i.playable());
        assert!(i.browse_request().unwrap().collection());
    }
    #[test]
    fn sonarmusic_share_links_support_tracks_albums_and_playlists() {
        assert_eq!(
            item().share_url(),
            Some("https://www.youtube.com/watch?v=LX5Z7R6rt_I".into())
        );
        let album = Item::parse(&json!({"id":"MPRE123","kind":"album"})).unwrap();
        assert_eq!(
            album.share_url(),
            Some("https://music.youtube.com/browse/MPRE123".into())
        );
        let playlist = Item::parse(&json!({"id":"PLabc123","kind":"playlist"})).unwrap();
        assert_eq!(
            playlist.share_url(),
            Some("https://music.youtube.com/playlist?list=PLabc123".into())
        );
        let artist = Item::parse(&json!({"id":"UCabc123","kind":"artist"})).unwrap();
        assert_eq!(
            artist.share_url(),
            Some("https://music.youtube.com/channel/UCabc123".into())
        );
        let bad = Item::parse(&json!({"id":"PLbad?tag=1","kind":"playlist"})).unwrap();
        assert_eq!(bad.share_url(), None);
    }

    #[test]
    fn playlist_prefix_is_normalized() {
        assert_eq!(normalize_browse_id("PL123"), "VLPL123");
        assert_eq!(normalize_browse_id("VLPL123"), "VLPL123");
    }
    #[test]
    fn artist_keeps_browse_id() {
        assert_eq!(normalize_browse_id("UC123"), "UC123");
        assert!(
            !Request::Browse {
                id: "UC123".into(),
                params: String::new()
            }
            .collection()
        );
    }
    #[test]
    fn discography_params_are_retained() {
        let i = Item::parse(&json!({"id":"MPAD123","kind":"collection","browse_params":"albums"}))
            .unwrap();
        assert!(
            i.browse_request()
                .unwrap()
                .parameters(None, 1)
                .contains(&("params".into(), "albums".into()))
        );
    }
    #[test]
    fn request_search_trims_query() {
        assert_eq!(
            Request::search("  Pink Floyd  ", "album")
                .unwrap()
                .parameters(None, 1)[0],
            ("q".into(), "Pink Floyd".into())
        );
    }
    #[test]
    fn empty_search_rejected() {
        assert!(Request::search(" \n ", "all").is_err());
    }
    #[test]
    fn unknown_search_type_rejected() {
        assert!(Request::search("Pink", "invalid").is_err());
    }
    #[test]
    fn all_six_types_supported() {
        for t in TYPES {
            assert!(Request::search("music", t).is_ok());
        }
        assert_eq!(TYPES.len(), 6);
    }
    #[test]
    fn continuation_is_opaque_not_decoded() {
        let p = Request::search("a", "all")
            .unwrap()
            .parameters(Some("a%2Fb+=&"), 2);
        assert!(p.contains(&("token".into(), "a%2Fb+=&".into())));
        assert!(p.contains(&("page".into(), "2".into())));
    }
    #[test]
    fn initial_request_omits_empty_token() {
        let p = Request::search("a", "all").unwrap().parameters(Some(""), 0);
        assert!(!p.iter().any(|(k, _)| k == "token"));
        assert!(p.contains(&("page".into(), "1".into())));
    }
    #[test]
    fn server_errors_are_not_empty_success() {
        assert!(Page::parse(&json!({"ok":false,"error":"unauthorized"}), false).is_err());
    }
    #[test]
    fn top_results_deduplicate_sections() {
        let p = Page::parse(
            &json!({"ok":true,"items":[song()],"sections":[{"items":[song()]}]}),
            false,
        )
        .unwrap();
        assert_eq!(p.items.len(), 1);
    }
    #[test]
    fn album_never_imports_recommendations() {
        let p = Page::parse(
            &json!({"ok":true,"items":[],"sections":[{"items":[song()]}],"next_token":"next"}),
            true,
        )
        .unwrap();
        assert!(p.items.is_empty());
        assert_eq!(p.next.as_deref(), Some("next"));
    }
    #[test]
    fn search_includes_section_results() {
        let p = Page::parse(&json!({"ok":true,"sections":[{"items":[song()]}]}), false).unwrap();
        assert_eq!(p.items.len(), 1);
    }
    #[test]
    fn append_keeps_order_and_deduplicates() {
        let mut p = Page {
            items: vec![item()],
            next: Some("one".into()),
        };
        let mut second = item();
        second.id = "other".into();
        p.append(Page {
            items: vec![item(), second],
            next: Some("two".into()),
        });
        assert_eq!(p.items.len(), 2);
        assert_eq!(p.items[0].title, "Wish You Were Here");
        assert_eq!(p.next.as_deref(), Some("two"));
    }
    #[test]
    fn repeated_continuation_stops_pagination() {
        let mut p = Page {
            items: vec![],
            next: Some("one".into()),
        };
        p.append(Page {
            items: vec![],
            next: Some("one".into()),
        });
        assert!(p.next.is_none());
    }
    #[test]
    fn no_continuation_means_last_page() {
        assert!(
            Page::parse(&json!({"ok":true,"next_token":""}), false)
                .unwrap()
                .next
                .is_none()
        );
    }
    #[test]
    fn display_includes_artist_and_duration() {
        let label = item().display("it");
        assert!(label.contains("Pink Floyd"));
        assert!(label.contains("5 minuti e 35 secondi"));
        assert!(label.contains("Brani"));
    }
    #[test]
    fn locale_and_fallback_are_deterministic() {
        assert_eq!(locale_pair("pt-BR"), ("pt", "BR"));
        assert_eq!(locale_pair("it"), ("it", "IT"));
        assert_eq!(locale_pair("unknown"), ("en", "US"));
        assert_eq!(label("unknown", "song"), label("en", "song"));
    }
    #[test]
    fn every_bundled_language_has_complete_labels() {
        let labels: Value = serde_json::from_str(include_str!("sonarmusic_labels.json")).unwrap();
        for language in labels.as_object().unwrap().values() {
            for key in [
                "query", "suggest", "search", "type", "results", "more", "open", "back", "close",
                "loading", "empty",
            ] {
                assert!(!language[key].as_str().unwrap().is_empty());
            }
        }
    }
}

/// Resolve only progressive video+audio streams. Other formats keep the existing
/// yt-dlp path; never hand a video-only stream to the player.
pub fn resolve_playback(watch_url: &str) -> Result<String, String> {
    resolve_playback_with_title(watch_url).map(|(url, _)| url)
}

pub fn resolve_playback_with_title(watch_url: &str) -> Result<(String, String), String> {
    let url = reqwest::Url::parse(watch_url).map_err(|_| "Invalid watch URL")?;
    if url.host_str() != Some("www.youtube.com") || url.path() != "/watch" {
        return Err("Unsupported watch URL".into());
    }
    let id = url
        .query_pairs()
        .find(|(key, _)| key == "v")
        .map(|(_, value)| value.into_owned())
        .filter(|id| valid_video_id(id))
        .ok_or("Invalid video ID")?;
    static HTTP: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    let client = HTTP.get_or_init(reqwest::blocking::Client::new);
    let data: Value = client.post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false&key=AIzaSyA8eiZmM1FaDVjRy-df2KTyQ_vfUWA8808")
        .timeout(Duration::from_secs(10))
        .header("User-Agent", "com.google.android.youtube/21.29.366 (Linux; U; Android 16; en_US; SM-S908E Build/TP1A.220624.014) gzip")
        .header("X-YouTube-Client-Name", "3")
        .header("X-YouTube-Client-Version", "21.29.366")
        .json(&serde_json::json!({"context":{"client":{"hl":"en","gl":"US","clientName":"ANDROID","clientVersion":"21.29.366","androidSdkVersion":33,"osName":"Android","osVersion":"16","platform":"MOBILE"}},"videoId":id,"playbackContext":{"contentPlaybackContext":{"html5Preference":"HTML5_PREF_WANTS"}},"contentCheckOk":true,"racyCheckOk":true}))
        .send().map_err(|_| "Direct resolver network failure")?
        .error_for_status().map_err(|_| "Direct resolver HTTP failure")?
        .json().map_err(|_| "Direct resolver invalid response")?;
    Ok((playback_url(&data)?, string(&data["videoDetails"], "title")))
}

fn playback_url(data: &Value) -> Result<String, String> {
    if data["playabilityStatus"]["status"] != "OK" || data["videoDetails"]["isLiveContent"] == true
    {
        return Err("Direct playback unavailable".into());
    }
    let formats = data["streamingData"]["formats"]
        .as_array()
        .ok_or("No progressive formats")?;
    let valid = |f: &&Value| {
        f["audioQuality"].is_string()
            && f["mimeType"]
                .as_str()
                .is_some_and(|mime| mime.starts_with("video/"))
            && f["url"].as_str().is_some_and(|url| {
                reqwest::Url::parse(url).is_ok_and(|u| {
                    u.scheme() == "https"
                        && u.host_str()
                            .is_some_and(|host| host.ends_with(".googlevideo.com"))
                })
            })
    };
    formats
        .iter()
        .filter(valid)
        .find(|f| f["itag"] == 18)
        .or_else(|| {
            formats
                .iter()
                .filter(valid)
                .filter(|f| f["height"].as_u64().unwrap_or(u64::MAX) <= 480)
                .max_by_key(|f| f["height"].as_u64().unwrap_or(0))
        })
        .and_then(|f| f["url"].as_str())
        .map(str::to_owned)
        .ok_or("No compatible direct stream".into())
}

#[cfg(test)]
mod playback_tests {
    use super::*;
    use serde_json::json;
    fn data() -> Value {
        json!({"playabilityStatus":{"status":"OK"},"streamingData":{"formats":[{"itag":18,"height":360,"audioQuality":"AUDIO_QUALITY_MEDIUM","mimeType":"video/mp4","url":"https://rr1.googlevideo.com/videoplayback?test=1"}]}})
    }
    #[test]
    fn progressive_stream() {
        assert!(playback_url(&data()).is_ok());
    }
    #[test]
    fn unavailable() {
        let mut d = data();
        d["playabilityStatus"]["status"] = json!("ERROR");
        assert!(playback_url(&d).is_err());
    }
    #[test]
    fn live_uses_fallback() {
        let mut d = data();
        d["videoDetails"] = json!({"isLiveContent":true});
        assert!(playback_url(&d).is_err());
    }
    #[test]
    fn cipher_uses_fallback() {
        let mut d = data();
        d["streamingData"]["formats"][0]["url"] = Value::Null;
        assert!(playback_url(&d).is_err());
    }
    #[test]
    fn video_only_uses_fallback() {
        let mut d = data();
        d["streamingData"]["formats"][0]["audioQuality"] = Value::Null;
        assert!(playback_url(&d).is_err());
    }
    #[test]
    fn reject_untrusted_url() {
        let mut d = data();
        d["streamingData"]["formats"][0]["url"] = json!("https://googlevideo.com.evil.test/a");
        assert!(playback_url(&d).is_err());
    }
    #[test]
    fn reject_plain_http() {
        let mut d = data();
        d["streamingData"]["formats"][0]["url"] = json!("http://rr1.googlevideo.com/a");
        assert!(playback_url(&d).is_err());
    }
    #[test]
    fn adaptive_only_uses_fallback() {
        assert!(
            playback_url(
                &json!({"playabilityStatus":{"status":"OK"},"streamingData":{"adaptiveFormats":[]}})
            )
            .is_err()
        );
    }
    #[test]
    fn invalid_watch_url() {
        assert!(resolve_playback("https://example.org/watch?v=4K6cvc27_CE").is_err());
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod catalog_fallback_tests {
    use super::*;
    use serde_json::json;
    fn result() -> Value {
        json!({"ok":true,"items":[{"kind":"song","id":"4K6cvc27_CE","video_id":"4K6cvc27_CE"}],"next_token":"next"})
    }
    fn request() -> Request {
        Request::search("Pink Floyd", "song").unwrap()
    }
    #[test]
    fn native_success_does_not_contact_other_providers() {
        let p = load_catalog(
            &request(),
            None,
            1,
            |t| {
                assert!(t.is_none());
                Ok(result())
            },
            |_, _| panic!(),
            |_| panic!(),
        )
        .unwrap();
        assert_eq!(p.next.as_deref(), Some("native:next"));
    }
    #[test]
    fn native_failure_uses_php() {
        let p = load_catalog(
            &request(),
            None,
            1,
            |_| Err("offline".into()),
            |t, p| {
                assert!(t.is_none());
                assert_eq!(p, 1);
                Ok(result())
            },
            |_| panic!(),
        )
        .unwrap();
        assert_eq!(p.next.as_deref(), Some("php:next"));
    }
    #[test]
    fn both_failures_use_ytdlp() {
        let p = load_catalog(
            &request(),
            None,
            1,
            |_| Err("offline".into()),
            |_, _| Err("offline".into()),
            |p| {
                assert_eq!(p, 1);
                Ok(result())
            },
        )
        .unwrap();
        assert_eq!(p.items.len(), 1);
    }
    #[test]
    fn php_cursor_never_reaches_native() {
        load_catalog(
            &request(),
            Some("php:opaque"),
            2,
            |_| panic!(),
            |t, p| {
                assert_eq!(t, Some("opaque"));
                assert_eq!(p, 2);
                Ok(result())
            },
            |_| panic!(),
        )
        .unwrap();
    }
    #[test]
    fn native_failure_replays_php_without_native_cursor() {
        let mut calls = 0;
        load_catalog(
            &request(),
            Some("native:albums:private"),
            3,
            |t| {
                assert_eq!(t, Some("albums:private"));
                Err("offline".into())
            },
            |t, p| {
                calls += 1;
                assert_eq!(p, calls);
                assert_eq!(t, if p == 1 { None } else { Some("next") });
                Ok(result())
            },
            |_| panic!(),
        )
        .unwrap();
        assert_eq!(calls, 3);
    }
    #[test]
    fn ytdlp_cursor_stays_with_ytdlp() {
        load_catalog(
            &request(),
            Some("ytdlp:2"),
            2,
            |_| panic!(),
            |_, _| panic!(),
            |p| {
                assert_eq!(p, 2);
                Ok(result())
            },
        )
        .unwrap();
    }
    #[test]
    fn all_failed_returns_error() {
        assert!(
            load_catalog(
                &request(),
                None,
                1,
                |_| Err("offline".into()),
                |_, _| Err("offline".into()),
                |_| Err("offline".into())
            )
            .is_err()
        );
    }
}

/// Persist the original catalog fields, not the UI label or a temporary media URL.
pub fn favorite_value(item: &Item) -> Value {
    serde_json::json!({"kind":item.kind,"id":item.id,"title":item.title,
        "video_id":item.video_id,"browse_id":item.browse_id,"browse_params":item.params,
        "artist":item.artist,"duration":item.duration,"subtitle":item.subtitle,"views":item.views,"available":item.available})
}
pub fn favorite_items(values: &[Value]) -> Vec<Item> {
    values
        .iter()
        .filter_map(Item::parse)
        .filter(|item| item.playable() || item.browse_request().is_some())
        .collect()
}
pub fn same_favorite(left: &Item, right: &Item) -> bool {
    if let (Some(a), Some(b)) = (left.watch_url(), right.watch_url()) {
        return a == b;
    }
    left.kind == right.kind
        && left.browse_request().is_some()
        && left.browse_request() == right.browse_request()
}
pub fn add_favorite(values: &mut Vec<Value>, item: &Item) -> bool {
    if (!item.playable() && item.browse_request().is_none())
        || favorite_items(values)
            .iter()
            .any(|saved| same_favorite(saved, item))
    {
        return false;
    }
    values.push(favorite_value(item));
    true
}
pub fn remove_favorite(values: &mut Vec<Value>, item: &Item) {
    values.retain(|value| !Item::parse(value).is_some_and(|saved| same_favorite(&saved, item)));
}
#[cfg(test)]
mod favorite_tests {
    use super::*;
    #[test]
    fn saved_artist_uses_selected_filter_but_albums_and_songs_keep_their_action() {
        let mut item = Item::parse(&serde_json::json!({
            "kind": "artist", "id": "UCartist", "title": "Gigi D'Alessio"
        }))
        .unwrap();
        for kind in TYPES.iter().copied().filter(|kind| *kind != "all") {
            assert_eq!(
                item.favorite_search(kind),
                Request::search("Gigi D'Alessio", kind).ok()
            );
        }
        assert!(item.favorite_search("all").is_none());
        assert!(item.favorite_search("invalid").is_none());
        for kind in ["album", "song", "playlist", "video"] {
            item.kind = kind.into();
            assert!(item.favorite_search("album").is_none());
        }
    }

    #[test]
    fn track_round_trip_and_duplicate_identity() {
        let a = Item::parse(
            &serde_json::json!({"kind":"song","id":"a","video_id":"abcdefghijk","title":"Same"}),
        )
        .unwrap();
        let mut favorites = Vec::new();
        assert!(add_favorite(&mut favorites, &a));
        let saved = serde_json::to_string(&favorites).unwrap();
        let mut restored: Vec<Value> = serde_json::from_str(&saved).unwrap();
        assert_eq!(favorite_items(&restored), vec![a.clone()]);
        let mut renamed = a.clone();
        renamed.title = "New title".into();
        assert!(!add_favorite(&mut restored, &renamed));
        remove_favorite(&mut restored, &renamed);
        assert!(restored.is_empty());
    }
    #[test]
    fn albums_keep_browse_parameters_and_same_titles_stay_distinct() {
        let a=Item::parse(&serde_json::json!({"kind":"album","id":"MPREone","title":"Same","browse_params":"special"})).unwrap();
        let mut b = a.clone();
        b.id = "MPREtwo".into();
        let mut saved = Vec::new();
        assert!(add_favorite(&mut saved, &a));
        assert!(add_favorite(&mut saved, &b));
        let restored = favorite_items(&saved);
        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].browse_request(), a.browse_request());
        remove_favorite(&mut saved, &a);
        assert_eq!(favorite_items(&saved), vec![b]);
    }
    #[test]
    fn damaged_and_unavailable_entries_are_not_opened() {
        assert!(
            favorite_items(&[
                Value::Null,
                serde_json::json!({"id":"x","kind":"song","available":false})
            ])
            .is_empty()
        );
    }
}

fn music_kind_descriptor(kind: &str, text: &str) -> bool {
    let values: &[&str] = match kind {
        "song" => &[
            "brano",
            "brani",
            "canzone",
            "canzoni",
            "song",
            "songs",
            "track",
            "tracks",
            "titre",
            "titres",
            "canción",
            "canciones",
            "música",
            "musica",
            "músicas",
            "utwór",
            "utwory",
            "skladba",
            "skladby",
            "пісня",
            "пісні",
            "melodie",
            "melodii",
            "歌曲",
        ],
        "video" => &[
            "video",
            "videos",
            "vídeo",
            "vídeos",
            "video musicali",
            "music video",
            "music videos",
            "videoclip",
            "videoclipuri",
            "musikvideo",
            "musikvideos",
            "clip",
            "clips",
        ],
        "artist" => &[
            "artista",
            "artisti",
            "artist",
            "artists",
            "artiste",
            "artistes",
            "artistas",
            "künstler",
            "umělec",
            "umělci",
            "виконавець",
            "виконавці",
            "艺人",
        ],
        "album" => &[
            "album",
            "albums",
            "álbum",
            "álbumes",
            "alben",
            "álbuns",
            "albumy",
            "alba",
            "альбом",
            "альбоми",
            "专辑",
        ],
        "playlist" => &[
            "playlist",
            "playlists",
            "playlisty",
            "liste",
            "список відтворення",
            "плейлисти",
            "播放列表",
        ],
        _ => &[],
    };
    values.contains(&text.to_lowercase().as_str())
}

#[cfg(test)]
mod media_result_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn metadata_duration_views_and_kind_are_not_repeated() {
        let item = Item::parse(&json!({"id":"4K6cvc27_CE", "kind":"video", "title":"Example", "artist":"Singer", "subtitle":"Video • Singer • 5:34 • 12 mila visualizzazioni"})).unwrap();
        let text = item.display("it");
        assert_eq!(text.matches("Singer").count(), 1);
        assert_eq!(text.matches("Video").count(), 1);
        assert!(text.contains("5 minuti e 34 secondi"));
        assert!(!text.contains("5:34"));
        assert!(text.contains("12 mila visualizzazioni"));
        assert_eq!(Item::parse(&favorite_value(&item)).unwrap(), item);
    }
    #[test]
    fn album_playlist_artist_have_types_without_fake_durations() {
        for kind in ["album", "playlist", "artist"] {
            let item =
                Item::parse(&json!({"id":"collection", "kind":kind, "title":"Example"})).unwrap();
            assert!(item.display("it").contains(&label("it", kind)));
            assert!(!item.display("it").contains("0 secondi"));
        }
    }
}
