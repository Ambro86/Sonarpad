//! Shared Windows/macOS SonarMusic catalog client. Keep both copies identical.
use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;

pub const ENDPOINT: &str = "https://sonarpad.com/api/youtube_music_resolve.php";
pub const TYPES: &[&str] = &["all", "song", "album", "artist", "playlist", "video"];

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
    pub fn display(&self, locale: &str) -> String {
        [
            self.title.clone(),
            self.artist.clone(),
            if TYPES.contains(&self.kind.as_str()) {
                label(locale, &self.kind)
            } else {
                String::new()
            },
            self.duration.clone(),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" — ")
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
        Page::parse(
            &self.get(request.parameters(token, page))?,
            request.collection(),
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
        assert!(label.contains("5:35"));
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
