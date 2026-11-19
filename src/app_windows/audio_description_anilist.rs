//! Optional metadata for a single TMDB-selected animated production.
use crate::tools::audio_description_bridge::{AniListReference, BridgeCharacter};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const SEARCH: &str = "query($q:String!){Page(perPage:20){media(type:ANIME,search:$q,sort:[SEARCH_MATCH]){id title{native english romaji} synonyms format startDate{year}}}}";
const CAST: &str = "query($id:Int!,$p:Int!){Media(id:$id,type:ANIME){characters(page:$p,perPage:50,sort:[ROLE,ID]){pageInfo{hasNextPage} edges{role node{id name{full native alternative} description(asHtml:false) image{large medium}}}}}}";
static COOLDOWN: Mutex<Option<Instant>> = Mutex::new(None);

// Keep only recent successful lookups for titles the user actually opened.
type CachedPage = (String, Instant, Value);
static PAGES: Mutex<Vec<CachedPage>> = Mutex::new(Vec::new());

fn query(client: &Client, query: &str, variables: Value) -> Result<Value, String> {
    let key = format!("{query}:{variables}");
    {
        let mut pages = PAGES.lock().unwrap_or_else(|e| e.into_inner());
        pages.retain(|(_, time, _)| time.elapsed() < Duration::from_secs(900));
        if let Some((_, _, data)) = pages.iter().find(|(k, _, _)| k == &key) {
            return Ok(data.clone());
        }
    }
    if COOLDOWN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some_and(|t| t > Instant::now())
    {
        return Err("rate limit".into());
    }
    let response = client
        .post("https://graphql.anilist.co")
        .json(&json!({"query":query,"variables":variables}))
        .send()
        .map_err(|e| format!("{}: {e:?}", if query == SEARCH { "search" } else { "cast" }))?;
    if response.status().as_u16() == 429 {
        let delay = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60);
        *COOLDOWN.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(Instant::now() + Duration::from_secs(delay.clamp(1, 86400)));
        return Err("rate limit".into());
    }
    let body: Value = response
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;
    if body["errors"].is_array() {
        return Err("GraphQL error".into());
    }
    let data = body["data"].clone();
    let mut pages = PAGES.lock().unwrap_or_else(|e| e.into_inner());
    if pages.len() >= 16 {
        pages.remove(0);
    }
    pages.push((key, Instant::now(), data.clone()));
    Ok(data)
}
fn normalized(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn matches(m: &Value, titles: &[&str], year: &str, kind: &str) -> bool {
    let format = m["format"].as_str().unwrap_or("");
    let compatible = if kind == "movie" {
        matches!(format, "MOVIE" | "OVA" | "SPECIAL" | "ONA")
    } else {
        matches!(format, "TV" | "TV_SHORT" | "OVA" | "ONA" | "SPECIAL")
    };
    compatible
        && (year.is_empty()
            || m["startDate"]["year"]
                .as_i64()
                .is_some_and(|y| y.to_string() == year))
        && m["title"]
            .as_object()
            .into_iter()
            .flat_map(|o| o.values())
            .chain(m["synonyms"].as_array().into_iter().flatten())
            .filter_map(Value::as_str)
            .any(|alias| {
                let key = normalized(alias);
                !key.is_empty() && titles.iter().any(|t| normalized(t) == key)
            })
}
fn title(m: &Value) -> &str {
    ["english", "romaji", "native"]
        .into_iter()
        .find_map(|k| m["title"][k].as_str())
        .unwrap_or("")
}

pub(super) fn lookup(
    titles: &[&str],
    year: &str,
    kind: &str,
    choose: &mut impl FnMut(&str, Vec<String>) -> Result<usize, String>,
) -> Result<Vec<BridgeCharacter>, String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(14))
        .http1_only()
        .pool_max_idle_per_host(0)
        .user_agent(concat!("Sonarpad/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let mut candidates = Vec::new();
    let mut searched = std::collections::HashSet::new();
    for t in titles.iter().filter(|t| !t.is_empty()) {
        if !searched.insert(normalized(t)) {
            continue;
        }
        let data = query(&client, SEARCH, json!({"q":t}))?;
        for m in data["Page"]["media"].as_array().into_iter().flatten() {
            if matches(m, titles, year, kind)
                && !candidates.iter().any(|v: &Value| v["id"] == m["id"])
            {
                candidates.push(m.clone())
            }
        }
    }
    let index = match candidates.len() {
        0 => return Ok(Vec::new()),
        1 => 0,
        _ => choose(
            "production",
            candidates
                .iter()
                .map(|m| {
                    format!(
                        "AniList — {} ({}, {}) [{}]",
                        title(m),
                        m["startDate"]["year"],
                        m["format"].as_str().unwrap_or(""),
                        m["id"]
                    )
                })
                .collect(),
        )?,
    };
    let media = candidates.get(index).ok_or("No selection")?;
    let media_id = media["id"].as_i64().ok_or("Missing media ID")?;
    let mut rows = Vec::new();
    for page in 1..=4 {
        let data = query(&client, CAST, json!({"id":media_id,"p":page}))?;
        let connection = &data["Media"]["characters"];
        for edge in connection["edges"].as_array().into_iter().flatten() {
            let node = &edge["node"];
            let Some(id) = node["id"].as_i64() else {
                continue;
            };
            let Some(name) = node["name"]["full"].as_str() else {
                continue;
            };
            let Some(url) = ["large", "medium"]
                .into_iter()
                .filter_map(|k| node["image"][k].as_str())
                .find(|u| {
                    u.starts_with("https://s4.anilist.co/file/anilistcdn/character/")
                        && !u.contains("/default")
                })
            else {
                continue;
            };
            rows.push(BridgeCharacter {
                id: format!("anilist_{media_id}_{id}"),
                name: name.into(),
                description: node["description"]
                    .as_str()
                    .filter(|v| !v.is_empty())
                    .unwrap_or("Animated character candidate")
                    .chars()
                    .take(600)
                    .collect(),
                tmdb: None,
                provisional: true,
                anilist: Some(AniListReference {
                    media_id,
                    media_title: title(media).into(),
                    source_character_id: id,
                    role: edge["role"].as_str().unwrap_or("").into(),
                    aliases: node["name"]["alternative"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .chain(std::iter::once(&node["name"]["native"]))
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect(),
                    image_urls: vec![url.into()],
                }),
            });
            if rows.len() == 96 {
                return Ok(rows);
            }
        }
        if connection["pageInfo"]["hasNextPage"] != true {
            break;
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_other_franchise_installments_years_and_formats() {
        let z = json!({"id":813,"title":{"romaji":"Dragon Ball Z"},"synonyms":["ドラゴンボールZ"],"startDate":{"year":1989},"format":"TV"});
        assert!(matches(&z, &["ドラゴンボールZ"], "1989", "tv"));
        for title in [
            "Dragon Ball",
            "Dragon Ball GT",
            "Dragon Ball Super",
            "The Simpsons",
        ] {
            assert!(!matches(&z, &[title], "", "tv"));
        }
        assert!(!matches(&z, &["Dragon Ball Z"], "1996", "tv"));
        assert!(!matches(&z, &["Dragon Ball Z"], "1989", "movie"));
    }

    #[test]
    #[ignore = "live provider smoke test"]
    fn selected_titles_have_the_expected_cast() {
        for (title, year, kind, id, name) in [
            ("Death Note", "2006", "tv", 1535, "Light Yagami"),
            ("Dragon Ball Z", "1989", "tv", 813, "Gokuu Son"),
            ("Dragon Ball GT", "1996", "tv", 225, "Trunks"),
            ("Spirited Away", "2001", "movie", 199, "Chihiro Ogino"),
            ("The Simpsons", "1989", "tv", 0, ""),
        ] {
            let cast = lookup(&[title], year, kind, &mut |_, _| {
                Err("unexpected ambiguity".into())
            })
            .unwrap();
            if id == 0 {
                assert!(cast.is_empty());
                continue;
            }
            assert!(cast.iter().any(|c| c.name == name), "{title}");
            assert!(cast.iter().all(|c| {
                c.anilist
                    .as_ref()
                    .is_some_and(|r| r.media_id == id && !r.image_urls.is_empty())
            }));
        }
    }
}
