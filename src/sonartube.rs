//! SonarTube endpoint adapter. Navigation keeps opaque continuations and cached
//! previous pages; extraction/download/authentication fallback stays with yt-dlp.
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Credentials and non-YouTube sites always use the existing authenticated path.
pub fn direct_watch_url(input: &str, has_credentials: bool) -> Option<String> {
    if has_credentials {
        return None;
    }
    let u = reqwest::Url::parse(input).ok()?;
    if !u.username().is_empty() || u.password().is_some() {
        return None;
    }
    let id = match u.host_str()? {
        "youtu.be" => u.path().trim_matches('/').to_owned(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com" => {
            if u.path() == "/watch" {
                u.query_pairs().find(|(k, _)| k == "v")?.1.into_owned()
            } else {
                let mut parts = u.path().trim_matches('/').split('/');
                if !matches!(parts.next(), Some("shorts" | "live" | "embed")) {
                    return None;
                }
                parts.next()?.to_owned()
            }
        }
        _ => return None,
    };
    (id.len() == 11 && safe_id(&id)).then(|| format!("https://www.youtube.com/watch?v={id}"))
}

const ENDPOINT: &str = "https://sonarpad.com/api/youtube_resolve.php";
#[derive(Clone, Debug, Default)]
pub struct Page {
    pub entries: Vec<Value>,
    pub next: Option<String>,
}
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("").trim()
}
fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn parse_page(data: &Value, collection: bool) -> Result<Page, String> {
    if data["ok"] != true {
        return Err("SonarTube service unavailable".into());
    }
    let items = data["items"]
        .as_array()
        .ok_or("SonarTube invalid results")?;
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        let id = text(item, "id");
        let kind = text(item, "kind");
        if !safe_id(id) || (collection && kind != "video") {
            continue;
        }
        let url = match kind {
            "video" if id.len() == 11 && (collection || !text(item, "views").is_empty()) => {
                format!("https://www.youtube.com/watch?v={id}")
            }
            "channel" if id.starts_with("UC") => format!("https://www.youtube.com/channel/{id}"),
            "playlist" => {
                // Preserve a valid seed for YouTube mixes.
                let seed = reqwest::Url::parse(text(item, "url")).ok().and_then(|u| {
                    u.query_pairs()
                        .find(|(k, _)| k == "v")
                        .map(|(_, v)| v.into_owned())
                });
                match seed.filter(|seed| safe_id(seed) && seed.len() == 11) {
                    Some(seed) => format!("https://www.youtube.com/watch?v={seed}&list={id}"),
                    None => format!("https://www.youtube.com/playlist?list={id}"),
                }
            }
            _ => continue,
        };
        if !seen.insert(url.clone()) {
            continue;
        }
        entries.push(json!({"id":id,"kind":kind,"url":url,"title":text(item,"title"),"channel":text(item,"channel"),
            "duration_string":text(item,"duration"),"view_count":text(item,"views"),"subscriber_count":text(item,"subscribers")}));
    }
    let next = data["next_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    Ok(Page { entries, next })
}
fn parameters(input: &str, collection: bool) -> Option<Vec<(String, String)>> {
    if !collection {
        return Some(vec![
            ("q".into(), input.trim().into()),
            ("type".into(), "all".into()),
        ]);
    }
    let url = reqwest::Url::parse(input).ok()?;
    if !matches!(
        url.host_str(),
        Some("www.youtube.com" | "youtube.com" | "m.youtube.com")
    ) {
        return None;
    }
    if let Some((_, id)) = url.query_pairs().find(|(k, _)| k == "list") {
        // The service currently advertises unusable continuations for RD mixes.
        // Keep the complete existing yt-dlp navigation for these collections.
        if !safe_id(&id) || id.starts_with("RD") {
            return None;
        }
        let mut p = vec![
            ("browse".into(), id.into_owned()),
            ("kind".into(), "playlist".into()),
        ];
        if let Some((_, seed)) = url.query_pairs().find(|(k, _)| k == "v") {
            p.push(("seed".into(), seed.into_owned()));
        }
        return Some(p);
    }
    let id = url.path().strip_prefix("/channel/")?.split('/').next()?;
    if !id.starts_with("UC") || !safe_id(id) {
        return None;
    }
    Some(vec![
        ("browse".into(), id.into()),
        ("kind".into(), "channel".into()),
        ("sort".into(), "newest".into()),
    ])
}
#[derive(Default)]
struct Pages {
    pages: Vec<Page>,
    fallback: bool,
}
impl Pages {
    fn get(
        &mut self,
        index: usize,
        mut fetch: impl FnMut(Option<&str>, usize) -> Result<Page, String>,
    ) -> Result<Option<Page>, String> {
        if let Some(cached) = self.pages.get(index) {
            return Ok(Some(cached.clone()));
        }
        if self.fallback {
            return Ok(None);
        }
        while self.pages.len() <= index {
            let token = self.pages.last().and_then(|p| p.next.clone());
            if !self.pages.is_empty() && token.is_none() {
                return Ok(Some(Page::default()));
            }
            let mut page = match fetch(token.as_deref(), self.pages.len() + 1) {
                Ok(page) => page,
                Err(_) => {
                    // The caller uses numeric yt-dlp pagination, never a native/PHP cursor.
                    // Previously visited pages remain cached for Back/Escape.
                    self.fallback = true;
                    return Ok(None);
                }
            };
            if page.entries.is_empty() && page.next.is_some() {
                self.fallback = true;
                return Ok(None);
            }
            if page.next.is_some()
                && (page.next == token || self.pages.iter().any(|old| old.next == page.next))
            {
                page.next = None;
            }
            self.pages.push(page);
        }
        Ok(self.pages.get(index).cloned())
    }
}
struct Session {
    updated: Instant,
    pages: Pages,
}
pub fn page(
    input: &str,
    collection: bool,
    locale: &str,
    index: usize,
) -> Result<Option<Page>, String> {
    let Some(base) = parameters(input, collection) else {
        return Ok(None);
    };
    let token = option_env!("SONARPAD_ROUTE_CLIENT_TOKEN").unwrap_or("");
    page_with_token(input, collection, locale, index, base, token)
}
fn page_with_token(
    input: &str,
    collection: bool,
    locale: &str,
    index: usize,
    base: Vec<(String, String)>,
    token: &str,
) -> Result<Option<Page>, String> {
    static CACHE: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "SonarTube cache unavailable")?;
    let key = format!("{locale}:{collection}:{input}");
    cache.retain(|_, s| s.updated.elapsed() < Duration::from_secs(900));
    if cache.len() >= 32 && !cache.contains_key(&key) {
        cache.clear();
    }
    let session = cache.entry(key).or_insert_with(|| Session {
        updated: Instant::now(),
        pages: Pages::default(),
    });
    session.updated = Instant::now();
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 Sonarpad/1.0")
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "SonarTube HTTP client unavailable")?;
    let (hl, gl) = crate::sonarmusic::locale_pair(locale);
    session.pages.get(index, |continuation, page| {
        if continuation.is_none() || continuation.is_some_and(|t| t.starts_with("native:")) {
            let raw = continuation.and_then(|t| t.strip_prefix("native:"));
            if let Ok(data) = crate::direct_catalog::youtube(&base, raw, locale)
                && let Ok(mut result) = parse_page(&data, collection)
                && !result.entries.is_empty()
            {
                result.next = result.next.map(|t| format!("native:{t}"));
                return Ok(result);
            }
        }
        let replay = continuation.is_some_and(|t| t.starts_with("native:"));
        let mut php_token = continuation
            .and_then(|t| t.strip_prefix("php:"))
            .map(str::to_owned);
        for current in (if replay { 1 } else { page })..=page {
            let mut args = base.clone();
            args.extend([
                ("format".into(), "json".into()),
                ("hl".into(), hl.into()),
                ("gl".into(), gl.into()),
                ("page".into(), current.to_string()),
            ]);
            if let Some(t) = &php_token {
                args.push(("token".into(), t.clone()));
            }
            let data: Value = client
                .get(ENDPOINT)
                .query(&args)
                .header("X-Sonarpad-Route-Token", token)
                .send()
                .map_err(|_| "SonarTube connection failed")?
                .error_for_status()
                .map_err(|_| "SonarTube HTTP failure")?
                .json()
                .map_err(|_| "SonarTube invalid JSON")?;
            let mut result = parse_page(&data, collection)?;
            if current == page {
                result.next = result.next.map(|t| format!("php:{t}"));
                return Ok(result);
            }
            php_token = Some(result.next.ok_or("PHP pagination exhausted")?);
        }
        Err("PHP pagination unavailable".into())
    })
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn video() -> Value {
        json!({"kind":"video","id":"4K6cvc27_CE","title":"Wish","views":"12 views","duration":"5:34"})
    }
    #[test]
    fn protected_video_bypasses_direct() {
        assert!(direct_watch_url("https://www.youtube.com/watch?v=4K6cvc27_CE", true).is_none());
    }
    #[test]
    fn other_site_bypasses_direct() {
        assert!(direct_watch_url("https://vimeo.com/123", false).is_none());
    }
    #[test]
    fn short_link_normalized() {
        assert_eq!(
            direct_watch_url("https://youtu.be/4K6cvc27_CE", false).unwrap(),
            "https://www.youtube.com/watch?v=4K6cvc27_CE"
        );
    }
    #[test]
    fn spoofed_host_rejected() {
        assert!(
            direct_watch_url("https://youtube.com.evil.test/watch?v=4K6cvc27_CE", false).is_none()
        );
    }
    #[test]
    fn parses_video() {
        let p = parse_page(&json!({"ok":true,"items":[video()]}), false).unwrap();
        assert_eq!(p.entries.len(), 1);
        assert_eq!(p.entries[0]["duration_string"], "5:34");
    }
    #[test]
    fn missing_views_filtered() {
        let mut v = video();
        v["views"] = Value::Null;
        assert!(
            parse_page(&json!({"ok":true,"items":[v]}), false)
                .unwrap()
                .entries
                .is_empty()
        );
    }
    #[test]
    fn collection_videos_do_not_require_search_view_counts() {
        let mut item = video();
        item["views"] = Value::Null;
        assert_eq!(
            parse_page(&json!({"ok":true,"items":[item]}), true)
                .unwrap()
                .entries
                .len(),
            1
        );
    }
    #[test]
    fn channels_preserved() {
        assert_eq!(
            parse_page(
                &json!({"ok":true,"items":[{"kind":"channel","id":"UCabc","title":"Channel"}]}),
                false
            )
            .unwrap()
            .entries
            .len(),
            1
        );
    }
    #[test]
    fn collection_filters_channels() {
        assert!(
            parse_page(
                &json!({"ok":true,"items":[{"kind":"channel","id":"UCabc"}]}),
                true
            )
            .unwrap()
            .entries
            .is_empty()
        );
    }
    #[test]
    fn rejects_server_error() {
        assert!(parse_page(&json!({"ok":false}), false).is_err());
    }
    #[test]
    fn rejects_missing_items() {
        assert!(parse_page(&json!({"ok":true}), false).is_err());
    }
    #[test]
    fn deduplicates() {
        assert_eq!(
            parse_page(&json!({"ok":true,"items":[video(),video()]}), false)
                .unwrap()
                .entries
                .len(),
            1
        );
    }
    #[test]
    fn rejects_malicious_id() {
        let mut v = video();
        v["id"] = json!("../bad");
        assert!(
            parse_page(&json!({"ok":true,"items":[v]}), false)
                .unwrap()
                .entries
                .is_empty()
        );
    }
    #[test]
    fn automatic_mix_uses_existing_navigation() {
        assert!(
            parameters(
                "https://www.youtube.com/watch?v=4K6cvc27_CE&list=RD4K6cvc27_CE",
                true
            )
            .is_none()
        );
    }
    #[test]
    fn playlist_query() {
        assert!(
            parameters("https://www.youtube.com/playlist?list=PLabc", true)
                .unwrap()
                .contains(&("browse".into(), "PLabc".into()))
        );
    }
    #[test]
    fn channel_query() {
        assert!(parameters("https://www.youtube.com/channel/UCabc/videos", true).is_some());
    }
    #[test]
    fn handle_falls_back() {
        assert!(parameters("https://www.youtube.com/@abc", true).is_none());
    }
    #[test]
    fn other_site_falls_back() {
        assert!(parameters("https://vimeo.com/123", true).is_none());
    }
    #[test]
    fn search_keeps_query() {
        assert_eq!(parameters("Pink Floyd", false).unwrap()[0].1, "Pink Floyd");
    }
    #[test]
    fn cached_previous_page() {
        let mut p = Pages::default();
        p.get(0, |_, _| {
            Ok(Page {
                entries: vec![video()],
                next: Some("opaque+/=".into()),
            })
        })
        .unwrap();
        p.get(1, |t, n| {
            assert_eq!(t, Some("opaque+/="));
            assert_eq!(n, 2);
            Ok(Page::default())
        })
        .unwrap();
        assert_eq!(
            p.get(0, |_, _| panic!("must use cache"))
                .unwrap()
                .unwrap()
                .entries
                .len(),
            1
        );
    }
    #[test]
    fn repeated_token_stops() {
        let mut p = Pages::default();
        for n in 0..2 {
            p.get(n, |_, _| {
                Ok(Page {
                    entries: vec![video()],
                    next: Some("same".into()),
                })
            })
            .unwrap();
        }
        assert!(p.pages[1].next.is_none());
    }
    #[test]
    fn empty_initial_continuation_uses_fallback() {
        let mut p = Pages::default();
        assert!(
            p.get(0, |_, _| Ok(Page {
                entries: vec![],
                next: Some("broken".into())
            }))
            .unwrap()
            .is_none()
        );
    }
    #[test]
    fn initial_failure_falls_back_once() {
        let mut p = Pages::default();
        assert!(p.get(0, |_, _| Err("offline".into())).unwrap().is_none());
        assert!(
            p.get(1, |_, _| panic!("fallback sticks"))
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn later_failure_uses_numeric_fallback_and_retains_previous_page() {
        let mut p = Pages::default();
        p.get(0, |_, _| {
            Ok(Page {
                entries: vec![video()],
                next: Some("next".into()),
            })
        })
        .unwrap();
        assert!(p.get(1, |_, _| Err("offline".into())).unwrap().is_none());
        assert_eq!(
            p.get(0, |_, _| panic!("previous page must be cached"))
                .unwrap()
                .unwrap()
                .entries
                .len(),
            1
        );
        assert_eq!(p.pages.len(), 1);
    }
    #[test]
    fn last_page_no_more_requests() {
        let mut p = Pages::default();
        p.get(0, |_, _| Ok(Page::default())).unwrap();
        assert!(
            p.get(1, |_, _| panic!())
                .unwrap()
                .unwrap()
                .entries
                .is_empty()
        );
    }
    #[test]
    #[ignore = "Network and SONARMUSIC_LIVE_TOKEN required"]
    fn live_search_and_pagination() {
        let token = std::env::var("SONARMUSIC_LIVE_TOKEN").unwrap();
        let collections = [
            "https://www.youtube.com/channel/UCY2qt3dw2TQJxvBrDiYGHdQ",
            "https://www.youtube.com/playlist?list=PL1tiBqitg38_Rsqb2qiTvm3hKX2Y2qUgg",
            "https://www.youtube.com/playlist?list=PL0SqtI95X6o2In080Fh5QE3mBjwOjm34y",
        ];
        for query in [
            "Pink Floyd",
            "Gigi D'Alessio",
            "Raf",
            "Grateful Dead",
            "Cristina D'Avena",
        ] {
            let p = page_with_token(
                query,
                false,
                "it",
                0,
                parameters(query, false).unwrap(),
                &token,
            )
            .unwrap()
            .expect("primary service");
            assert!(!p.entries.is_empty(), "{query}");
            println!(
                "search {query}: {} entries, next={}",
                p.entries.len(),
                p.next.is_some()
            );
            if p.next.is_some() {
                let next = page_with_token(
                    query,
                    false,
                    "it",
                    1,
                    parameters(query, false).unwrap(),
                    &token,
                )
                .unwrap()
                .unwrap();
                assert!(!next.entries.is_empty(), "next {query}");
            }
        }
        for url in collections.iter().take(4) {
            let p = page_with_token(url, true, "it", 0, parameters(url, true).unwrap(), &token)
                .unwrap()
                .expect("primary collection service");
            println!(
                "collection {}: {} entries, next={}",
                url,
                p.entries.len(),
                p.next.is_some()
            );
            assert!(!p.entries.is_empty(), "empty collection {url}");
            if p.next.is_some() {
                let next =
                    page_with_token(url, true, "it", 1, parameters(url, true).unwrap(), &token)
                        .unwrap()
                        .unwrap();
                assert!(
                    !next.entries.is_empty(),
                    "empty collection continuation {url}"
                );
            }
            let previous =
                page_with_token(url, true, "it", 0, parameters(url, true).unwrap(), &token)
                    .unwrap()
                    .unwrap();
            assert_eq!(p.entries, previous.entries);
        }
    }
}

#[cfg(test)]
mod subscriber_tests {
    use super::*;
    #[test]
    fn subscriber_text_reaches_windows_result_metadata() {
        let page = parse_page(&json!({"ok":true,"items":[{"kind":"channel","id":"UCexample","title":"Example","subscribers":"15 mila iscritti"}]}), false).unwrap();
        assert_eq!(page.entries[0]["subscriber_count"], "15 mila iscritti");
        assert_eq!(page.entries[0]["kind"], "channel");
    }
}
