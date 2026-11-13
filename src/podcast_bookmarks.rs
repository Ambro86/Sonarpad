//! Portable podcast resume points. Never use a device's downloaded cache path.
use serde_json::{Value, json};

pub const FIELDS: &[&str] = &["url", "title", "position_seconds", "updated_at"];

pub fn valid(data: &Value, id: &str) -> bool {
    let Some(url) = data["url"].as_str() else {
        return false;
    };
    reqwest::Url::parse(url)
        .is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
        && url == id
        && url == url.trim()
        && data["title"].is_string()
        && data["position_seconds"]
            .as_u64()
            .is_some_and(|v| v <= i32::MAX as u64)
        && data["updated_at"]
            .as_u64()
            .is_some_and(|v| v <= 9007199254740991)
}

pub fn position(rows: &[Value], url: &str) -> Option<u64> {
    rows.iter()
        .find(|v| v["url"] == url.trim())
        .and_then(|v| v["position_seconds"].as_u64())
}

pub fn save(rows: &mut Vec<Value>, url: &str, title: &str, seconds: u64) -> bool {
    let url = url.trim();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let previous_time = rows
        .iter()
        .find(|v| v["url"] == url)
        .and_then(|v| v["updated_at"].as_u64())
        .unwrap_or(0);
    let now = now.max(previous_time.saturating_add(1));
    let data = json!({"url":url,"title":title,"position_seconds":seconds,"updated_at":now});
    if !valid(&data, url) || position(rows, url) == Some(seconds) {
        return false;
    }
    if let Some(row) = rows.iter_mut().find(|v| v["url"] == url) {
        *row = data;
    } else {
        rows.push(data);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_podcast_save_is_monotonic_and_ignores_unchanged_positions() {
        let url = "https://example.org/episode.mp3";
        let mut rows = vec![
            json!({"url":url,"title":"Episode","position_seconds":123,"updated_at":8000000000000u64}),
        ];
        assert!(save(&mut rows, url, "Episode", 0));
        assert_eq!(rows[0]["updated_at"], json!(8000000000001u64));
        let saved = rows.clone();
        assert!(!save(&mut rows, url, "Episode", 0));
        assert_eq!(rows, saved);
        assert!(!save(&mut rows, "C:\\cache\\episode.mp3", "Local", 42));
        assert_eq!(rows, saved);
    }
}
