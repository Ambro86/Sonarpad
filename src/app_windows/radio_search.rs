use super::{
    RADIO_RESULTS_PAGE_SIZE, RadioStation, canonical_radio_name, favorite_from_station,
    fetch_community_radio_stations, fetch_radio_browser_stations, locale_display_names,
    normalize_stream_url, radio_browser_language_name, radio_search_rank,
};
use crate::settings::{Language, RadioFavorite};
use serde::Deserialize;
use std::collections::HashSet;
use std::time::Duration;
use url::Url;

const RADIO_BOSS_URL: &str = "https://sonarpad.com/api/radioboss.php";
const MAX_SOURCE_PAGES: usize = 101;

#[derive(Default)]
pub(super) struct SourcePage {
    pub stations: Vec<RadioStation>,
    pub has_more: bool,
}

pub(super) struct SearchSession {
    query: SearchQuery,
    cursors: [usize; 3],
    pending: [bool; 3],
    seen_names: HashSet<String>,
    seen_urls: HashSet<String>,
    errors: Vec<String>,
    used_genre_fallback: bool,
    pub results: Vec<RadioFavorite>,
}

struct SearchQuery {
    code: String,
    name: Option<String>,
    genre: Option<String>,
}

impl SearchSession {
    pub fn new(code: String, name: Option<String>, genre: Option<String>) -> Self {
        Self {
            query: SearchQuery { code, name, genre },
            cursors: [0; 3],
            pending: [true; 3],
            seen_names: HashSet::new(),
            seen_urls: HashSet::new(),
            errors: Vec::new(),
            used_genre_fallback: false,
            results: Vec::new(),
        }
    }

    pub fn has_more(&self) -> bool {
        self.pending.iter().any(|pending| *pending)
    }

    pub fn load_page(&mut self, page: usize) -> Result<(), String> {
        self.load_page_with(page, fetch_batch)
    }

    fn load_page_with(
        &mut self,
        page: usize,
        mut fetch: impl FnMut(&SearchQuery, [Option<usize>; 3]) -> [Result<SourcePage, String>; 3],
    ) -> Result<(), String> {
        // One result beyond the requested page is enough to enable Next. Each
        // source keeps its own cursor; filtering/deduplication never advances it.
        let target = (page + 1) * RADIO_RESULTS_PAGE_SIZE + 1;
        loop {
            while self.results.len() < target && self.has_more() {
                let requests = std::array::from_fn(|i| self.pending[i].then_some(self.cursors[i]));
                let batch = fetch(&self.query, requests);
                let mut unique = Vec::new();
                for (i, result) in batch.into_iter().enumerate() {
                    if !self.pending[i] {
                        continue;
                    }
                    match result {
                        Ok(page) => {
                            self.cursors[i] += 1;
                            self.pending[i] = page.has_more && self.cursors[i] < MAX_SOURCE_PAGES;
                            for station in page.stations {
                                let name = canonical_radio_name(
                                    station
                                        .name
                                        .split_once(" - ")
                                        .map_or(station.name.as_str(), |(name, _)| name),
                                );
                                let url = normalize_stream_url(&station.stream_url);
                                if name.is_empty()
                                    || station.stream_url.trim().is_empty()
                                    || self.seen_names.contains(&name)
                                    || self.seen_urls.contains(&url)
                                {
                                    continue;
                                }
                                self.seen_names.insert(name);
                                self.seen_urls.insert(url);
                                unique.push(favorite_from_station(&self.query.code, &station));
                            }
                        }
                        Err(error) => {
                            crate::log_debug(&format!(
                                "Radio source {i} page {} failed: {error}",
                                self.cursors[i]
                            ));
                            self.errors.push(error);
                            self.pending[i] = false;
                        }
                    }
                }
                let keyword = self.query.name.as_deref().unwrap_or_default();
                unique.sort_by_key(|station| radio_search_rank(&station.name, keyword));
                // Never sort old results again: back/next must preserve page boundaries.
                self.results.extend(unique);
            }
            if self.results.is_empty()
                && !self.used_genre_fallback
                && self.query.genre.is_some()
                && self.query.name.is_some()
            {
                self.used_genre_fallback = true;
                self.query.genre = None;
                self.cursors = [0; 3];
                self.pending = [true; 3];
                self.errors.clear();
                continue;
            }
            break;
        }
        if self.results.is_empty() && !self.errors.is_empty() {
            return Err(self.errors.join(" | "));
        }
        Ok(())
    }
}

fn fetch_batch(query: &SearchQuery, pages: [Option<usize>; 3]) -> [Result<SourcePage, String>; 3] {
    std::thread::scope(|scope| {
        let tasks = pages;
        let handles: Vec<_> = tasks
            .into_iter()
            .enumerate()
            .map(|(source, page)| {
                scope.spawn(move || {
                    let Some(page) = page else {
                        return Ok(SourcePage::default());
                    };
                    let name = query.name.as_deref();
                    let genre = query.genre.as_deref();
                    match source {
                        0 => fetch_radio_browser_stations(&query.code, name, genre, page),
                        1 => fetch_community_radio_stations(&query.code, name, genre).map(
                            |stations| SourcePage {
                                stations,
                                has_more: false,
                            },
                        ),
                        _ => fetch_radio_boss_page(query, page),
                    }
                })
            })
            .collect();
        let mut results = handles.into_iter().map(|handle| {
            handle
                .join()
                .unwrap_or_else(|_| Err("Radio search worker panicked".to_string()))
        });
        std::array::from_fn(|_| {
            results
                .next()
                .unwrap_or_else(|| Err("Missing radio source result".to_string()))
        })
    })
}

fn radio_boss_url(query: &SearchQuery, page: usize) -> Result<Url, String> {
    let mut url = Url::parse(RADIO_BOSS_URL).map_err(|error| error.to_string())?;
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("page", &page.to_string());
        if let Some(genre) = query
            .genre
            .as_deref()
            .filter(|genre| !genre.trim().is_empty())
        {
            params.append_pair("genre", genre);
        }
        if let Some(name) = query.name.as_deref().filter(|name| !name.trim().is_empty()) {
            params.append_pair("q", name);
        } else if let Some(city) = query.code.strip_prefix("city:") {
            params.append_pair("city", city);
        } else if let Some(country) = query.code.strip_prefix("country:") {
            let country = match country.to_uppercase().as_str() {
                "IT" => "italia".to_string(),
                "US" => "usa".to_string(),
                "GB" => "britain".to_string(),
                "TR" => "turkey".to_string(),
                _ => {
                    locale_display_names::territory_name(Language::English, &country.to_uppercase())
                        .unwrap_or_else(|| country.to_string())
                        .to_lowercase()
                }
            };
            params.append_pair("country", &country);
        } else if query.code != "all" {
            params.append_pair("language", &radio_browser_language_name(&query.code));
        }
    }
    Ok(url)
}

#[derive(Deserialize)]
struct RadioBossResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    stations: Option<Vec<RadioBossStation>>,
    #[serde(default)]
    has_more: bool,
}

#[derive(Deserialize)]
struct RadioBossStation {
    #[serde(default)]
    name: String,
    stream: Option<String>,
    stream_direct: Option<String>,
}

fn parse_radio_boss_response(response: RadioBossResponse) -> Result<SourcePage, String> {
    if !response.ok {
        return Err(response
            .error
            .unwrap_or_else(|| "RadioBOSS search failed".to_string()));
    }
    let stations = response
        .stations
        .ok_or("RadioBOSS response is missing stations")?
        .into_iter()
        .filter_map(|station| {
            let name = station.name.trim().replace('&', "");
            let stream = station
                .stream
                .filter(|stream| !stream.trim().is_empty())
                .or(station.stream_direct)?;
            let stream_url = stream.trim().to_string();
            if name.is_empty() || stream_url.is_empty() {
                return None;
            }
            Some(RadioStation { name, stream_url })
        })
        .collect();
    Ok(SourcePage {
        stations,
        has_more: response.has_more,
    })
}

fn fetch_radio_boss_page(query: &SearchQuery, page: usize) -> Result<SourcePage, String> {
    let url = radio_boss_url(query, page)?;
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("Sonarpad/1.0 (https://sonarpad.com)")
        .build()
        .map_err(|error| error.to_string())?
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<RadioBossResponse>())
        .map_err(|error| error.to_string())?;
    let result = parse_radio_boss_response(response)?;
    crate::log_debug(&format!(
        "RadioBOSS page {page}: {} stations, has_more={}",
        result.stations.len(),
        result.has_more
    ));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stations(start: usize, count: usize, has_more: bool) -> SourcePage {
        SourcePage {
            stations: (start..start + count)
                .map(|i| RadioStation {
                    name: format!("Station {i:04}"),
                    stream_url: format!("https://example.com/{i}"),
                })
                .collect(),
            has_more,
        }
    }

    fn session() -> SearchSession {
        SearchSession::new("all".into(), None, None)
    }

    #[test]
    fn independent_cursors_cross_100_results_without_reordering_or_refetching() {
        let mut session = session();
        let mut calls = Vec::new();
        let mut fetch = |_: &SearchQuery, pages: [Option<usize>; 3]| {
            calls.push(pages);
            [
                Ok(pages[0].map_or_else(SourcePage::default, |page| {
                    stations(page * 100, 100, page == 0)
                })),
                Ok(SourcePage::default()),
                Ok(pages[2].map_or_else(SourcePage::default, |page| {
                    stations(1000 + page * 10, 10, page < 2)
                })),
            ]
        };
        session.load_page_with(0, &mut fetch).unwrap();
        let first: Vec<_> = session
            .results
            .iter()
            .map(|station| station.stream_url.clone())
            .collect();
        session.load_page_with(4, &mut fetch).unwrap();
        assert!(session.results.len() > 125);
        assert_eq!(
            first,
            session.results[..first.len()]
                .iter()
                .map(|station| station.stream_url.clone())
                .collect::<Vec<_>>()
        );
        session.load_page_with(0, &mut fetch).unwrap();
        session.load_page_with(8, &mut fetch).unwrap();
        assert_eq!(session.results.len(), 230);
        assert!(!session.has_more());
        assert_eq!(
            calls,
            vec![
                [Some(0), Some(0), Some(0)],
                [Some(1), None, Some(1)],
                [None, None, Some(2)]
            ]
        );
    }

    #[test]
    fn duplicate_and_filtered_pages_do_not_hide_later_stations() {
        let mut session = session();
        let mut calls = 0;
        session
            .load_page_with(0, |_, pages| {
                calls += 1;
                let page = pages[2].unwrap();
                let boss = match page {
                    0 => stations(0, 1, true), // Duplicates Radio Browser.
                    1 => SourcePage {
                        stations: vec![],
                        has_more: true,
                    },
                    _ => stations(1, 26, false),
                };
                [
                    Ok(stations(0, 1, false)),
                    Ok(SourcePage::default()),
                    Ok(boss),
                ]
            })
            .unwrap();
        assert_eq!(calls, 3);
        assert_eq!(session.results.len(), 27);
        assert!(!session.has_more());
    }

    #[test]
    fn deduplicates_names_and_urls_across_sources_and_pages() {
        let mut session = session();
        session
            .load_page_with(0, |_, _| {
                let mut boss = stations(40, 3, false);
                boss.stations[0].name = "Station 0000".into();
                boss.stations[1].stream_url = "http://example.com/0/".into();
                [
                    Ok(stations(0, 1, false)),
                    Ok(SourcePage::default()),
                    Ok(boss),
                ]
            })
            .unwrap();
        assert_eq!(session.results.len(), 2);
        assert_eq!(session.results[0].name, "Station 0000");
    }

    #[test]
    fn failed_source_preserves_other_results_and_exact_final_page_has_no_next() {
        let mut session = session();
        session
            .load_page_with(0, |_, _| {
                [
                    Err("Browser unavailable".into()),
                    Ok(SourcePage::default()),
                    Ok(stations(0, 25, false)),
                ]
            })
            .unwrap();
        assert_eq!(session.results.len(), 25);
        assert!(!session.has_more());
        session
            .load_page_with(0, |_, _| panic!("Cached pages must not fetch again"))
            .unwrap();
        let mut failed = SearchSession::new("all".into(), None, None);
        assert!(
            failed
                .load_page_with(0, |_, _| std::array::from_fn(|_| Err("Offline".into())))
                .is_err()
        );
    }

    #[test]
    fn genre_fallback_resets_all_cursors_once() {
        let mut session = SearchSession::new("it".into(), Some("test".into()), Some("jazz".into()));
        let mut calls = 0;
        session
            .load_page_with(0, |query, pages| {
                calls += 1;
                assert_eq!(pages, [Some(0); 3]);
                [
                    Ok(SourcePage::default()),
                    Ok(SourcePage::default()),
                    Ok(if query.genre.is_some() {
                        SourcePage::default()
                    } else {
                        stations(0, 1, false)
                    }),
                ]
            })
            .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(session.results.len(), 1);
    }

    #[test]
    fn server_repeating_pages_cannot_loop_forever() {
        let mut session = session();
        let mut calls = 0;
        session
            .load_page_with(0, |_, _| {
                calls += 1;
                [
                    Ok(SourcePage::default()),
                    Ok(SourcePage::default()),
                    Ok(stations(0, 1, true)),
                ]
            })
            .unwrap();
        assert_eq!(calls, MAX_SOURCE_PAGES);
        assert_eq!(session.results.len(), 1);
        assert!(!session.has_more());
    }

    #[test]
    fn boss_filters_and_pagination_match_mobile() {
        for (code, key, value) in [
            ("it", "language", "Italian"),
            ("country:it", "country", "italia"),
            ("country:gb", "country", "britain"),
            ("country:de", "country", "germany"),
            ("city:New York", "city", "New York"),
        ] {
            let query = SearchQuery {
                code: code.into(),
                name: None,
                genre: Some("rock & pop".into()),
            };
            let url = radio_boss_url(&query, 3).unwrap();
            let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
            assert!(params[key].eq_ignore_ascii_case(value));
            assert_eq!(params["page"], "3");
            assert_eq!(params["genre"], "rock & pop");
        }
        let query = SearchQuery {
            code: "country:it".into(),
            name: Some("radio & jazz".into()),
            genre: None,
        };
        let url = radio_boss_url(&query, 0).unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params["q"], "radio & jazz");
        assert!(!params.contains_key("country"));
    }

    #[test]
    fn boss_response_keeps_has_more_even_when_stations_are_invalid() {
        let response = serde_json::from_str(
            r#"{"ok":true,"has_more":true,"stations":[
            {"name":"Example","stream":"","stream_direct":"https://example.com/live"},
            {"name":"Missing stream"},{"name":"","stream":"https://example.com/invalid"}
        ]}"#,
        )
        .unwrap();
        let page = parse_radio_boss_response(response).unwrap();
        assert!(page.has_more);
        assert_eq!(page.stations.len(), 1);
        assert_eq!(page.stations[0].stream_url, "https://example.com/live");
        assert!(
            parse_radio_boss_response(
                serde_json::from_str(r#"{"ok":false,"error":"Unavailable"}"#).unwrap()
            )
            .is_err()
        );
        assert!(
            parse_radio_boss_response(serde_json::from_str(r#"{"ok":true}"#).unwrap()).is_err()
        );
    }

    #[test]
    #[ignore = "Requires live radio catalog APIs"]
    fn live_catalog_pagination() {
        let query = SearchQuery {
            code: "all".into(),
            name: None,
            genre: None,
        };
        let first = fetch_radio_boss_page(&query, 0).unwrap();
        assert!(!first.stations.is_empty());
        if first.has_more {
            let second = fetch_radio_boss_page(&query, 1).unwrap();
            assert!(!second.stations.is_empty());
            assert_ne!(first.stations[0].stream_url, second.stations[0].stream_url);
        }
        let first = fetch_radio_browser_stations("all", None, None, 0).unwrap();
        let second = fetch_radio_browser_stations("all", None, None, 1).unwrap();
        assert!(first.has_more);
        assert!(!second.stations.is_empty());
        assert_ne!(first.stations[0].stream_url, second.stations[0].stream_url);
    }
}
