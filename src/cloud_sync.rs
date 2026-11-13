//! Shared v1 protocol with the mobile client. Never transmit complete settings.
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const ENDPOINT: &str = "https://sonarpad.com/synk/index.php";

/// One scheduler per app. A short local poll batches edits; remote checks and
/// retries are spaced out. No account is created by the scheduler.
#[derive(Default)]
pub struct AutoSync {
    next_attempt: u64,
    failures: u32,
    started: bool,
}
impl AutoSync {
    pub fn due(&mut self, meta: &Value, dirty: bool, now: u64) -> bool {
        if !meta["group_id"].is_string()
            || (meta["sync_preferences"] == false && meta["sync_collections"] == false)
        {
            *self = Self::default();
            return false;
        }
        if now < self.next_attempt {
            return false;
        }
        let last = meta["last_sync"].as_u64().unwrap_or(0);
        if !self.started || dirty || now.saturating_sub(last) >= 180 {
            self.started = true;
            self.next_attempt = now + 30;
            return true;
        }
        false
    }
    pub fn completed(&mut self, success: bool, now: u64) {
        self.started = true;
        self.failures = if success {
            0
        } else {
            (self.failures + 1).min(5)
        };
        self.next_attempt = now
            + if success {
                30
            } else {
                (30u64 << (self.failures - 1)).min(300)
            };
    }
    pub fn failed(&self) -> bool {
        self.failures != 0
    }
}
pub fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn selected(meta: &Value, state: &Value) -> Value {
    let mut out = json!({});
    if let Some(values) = state.as_object() {
        for (key, value) in values {
            if meta[if key == "preferences" {
                "sync_preferences"
            } else {
                "sync_collections"
            }] != false
            {
                out[key] = value.clone();
            }
        }
    }
    out
}
pub fn local_changes(meta: &Value, settings: &Value, mac: bool) -> Result<bool, String> {
    if meta.get("pending").is_some() {
        return Ok(true);
    }
    let local = selected(meta, &export(settings, mac));
    let remote = selected(meta, &project(&meta["baseline"], settings, mac)?);
    // Round-trip the baseline through the existing adapter to account for
    // fields/defaults which this platform cannot represent.
    let mut expected = settings.clone();
    apply(&mut expected, &remote, mac)?;
    Ok(local != selected(meta, &export(&expected, mac)))
}
pub fn status(meta: &Value, dirty: bool, busy: bool, failed: bool, italian: bool) -> String {
    let message = if meta.get("deletion_review").is_some() {
        if italian {
            "Rimozione inattesa bloccata: i dati sono protetti."
        } else {
            "Unexpected removal blocked: data is protected."
        }
    } else if busy {
        if italian {
            "Sincronizzazione in corso…"
        } else {
            "Syncing…"
        }
    } else if meta["sync_preferences"] == false && meta["sync_collections"] == false {
        if italian {
            "Sincronizzazione disattivata nelle opzioni."
        } else {
            "Sync disabled in options."
        }
    } else if failed {
        if italian {
            "Sincronizzazione in attesa. Riproverò automaticamente."
        } else {
            "Sync pending. Will retry automatically."
        }
    } else if dirty || meta.get("pending").is_some() {
        if italian {
            "Modifiche in attesa di sincronizzazione."
        } else {
            "Changes waiting to sync."
        }
    } else if meta["last_sync"].is_u64() {
        if italian {
            "Tutto sincronizzato."
        } else {
            "Everything synced."
        }
    } else {
        if italian {
            "In attesa della prima sincronizzazione."
        } else {
            "Waiting for the first sync."
        }
    };
    let last = meta["last_sync"]
        .as_u64()
        .and_then(|t| chrono::DateTime::from_timestamp(t as i64, 0));
    if let Some(last) = last {
        format!(
            "{message} {}: {}",
            if italian {
                "Ultima sincronizzazione"
            } else {
                "Last sync"
            },
            last.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M")
        )
    } else {
        message.into()
    }
}
pub fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn list(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}
fn item(id: &str, data: Value) -> Value {
    json!({"id":id,"data":data})
}
pub fn language(s: &Value, mac: bool) -> String {
    let v = text(
        s,
        if mac {
            "news_language"
        } else {
            "rss_news_language"
        },
    );
    if v.is_empty() { "it".into() } else { v.into() }
}
fn source_field(mac: bool) -> &'static str {
    if mac {
        "article_sources"
    } else {
        "rss_sources"
    }
}
fn is_youtube(v: &Value) -> bool {
    reqwest::Url::parse(text(v, "url"))
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .is_some_and(|h| {
            [
                "youtube.com",
                "www.youtube.com",
                "m.youtube.com",
                "youtu.be",
            ]
            .contains(&h.as_str())
        })
}
// Virtual navigation entry, never an RSS feed. Older clients uploaded it;
// ignore it on download too so their pending requests can be replayed safely.
fn virtual_news_source(item: &Value) -> bool {
    text(item, "url") == "sonarpad://rss/favorites"
}
fn personal(s: &Value, item: &Value, mac: bool) -> bool {
    if virtual_news_source(item) {
        return false;
    }
    if mac {
        !list(&s["_cloud_default_urls"])
            .iter()
            .any(|v| v.as_str() == item["url"].as_str())
    } else {
        item["user_title"].as_bool().unwrap_or(false)
    }
}
pub fn youtube_url(input: &str) -> String {
    let Ok(u) = reqwest::Url::parse(input.trim()) else {
        return input.trim().into();
    };
    let host = u.host_str().unwrap_or("");
    if host == "youtu.be" {
        return format!(
            "https://www.youtube.com/watch?v={}",
            u.path().trim_matches('/')
        );
    }
    if !["youtube.com", "www.youtube.com", "m.youtube.com"].contains(&host) {
        return input.trim().into();
    }
    let segments = u
        .path_segments()
        .map(|s| s.collect::<Vec<_>>())
        .unwrap_or_default();
    if segments.len() >= 2 && ["shorts", "live", "embed"].contains(&segments[0]) {
        return format!("https://www.youtube.com/watch?v={}", segments[1]);
    }
    let pairs: BTreeMap<_, _> = u.query_pairs().collect();
    if let Some(v) = pairs.get("v") {
        return format!("https://www.youtube.com/watch?v={v}");
    }
    if let Some(v) = pairs.get("list") {
        return format!("https://www.youtube.com/playlist?list={v}");
    }
    if let Some(first) = segments.first()
        && first.starts_with('@')
    {
        return format!("https://www.youtube.com/{first}");
    }
    if segments.len() >= 2 && segments[0] == "channel" {
        return format!("https://www.youtube.com/channel/{}", segments[1]);
    }
    format!("https://www.youtube.com{}", u.path().trim_end_matches('/'))
}
const MUSIC_FIELDS: &[&str] = &[
    "kind",
    "id",
    "title",
    "video_id",
    "browse_id",
    "browse_params",
    "artist",
    "duration",
    "subtitle",
    "available",
];
fn music_id(data: &Value) -> String {
    let params = text(data, "browse_params");
    let base = format!("{}:{}", text(data, "kind"), text(data, "id"));
    if params.is_empty() {
        base
    } else {
        format!("{base}:{params}")
    }
}
fn music_data(value: &Value) -> Value {
    let mut data = json!({});
    for key in MUSIC_FIELDS {
        data[*key] = if *key == "available" {
            json!(value[*key] != false)
        } else {
            json!(text(value, key))
        };
    }
    data
}
pub fn export(s: &Value, mac: bool) -> Value {
    let lang = language(s, mac);
    let mut result = json!({
        "podcast_bookmarks":list(&s["podcast_bookmarks"]).iter().map(|v| item(text(v,"url"), v.clone())).collect::<Vec<_>>(),
        "sonarmusic":list(&s["sonarmusic_favorites"]).iter().filter(|v| !text(v,"id").is_empty()).map(|v| {
            let data = music_data(v); item(&music_id(&data), data)
        }).collect::<Vec<_>>(),
        "radio":list(&s["radio_favorites"]).iter().map(|v| item(text(v,"stream_url"),json!({"name":v["name"],"stream_url":v["stream_url"],"language_code":v["language_code"]}))).collect::<Vec<_>>(),
        "tv":list(&s["tv_favorites"]).iter().map(|v| item(&text(v,"name").trim().to_lowercase(),json!({"name":v["name"]}))).collect::<Vec<_>>(),
        "podcasts":list(&s["podcast_sources"]).iter().map(|v| item(text(v,"url"),json!({"title":v["title"],"url":v["url"]}))).collect::<Vec<_>>(),
        "sonartube":list(&s[if mac {"youtube_favorites"} else {"stream_favorites"}]).iter().filter(|v|is_youtube(v)).map(|v| {
            let url=youtube_url(text(v,"url")); item(&url,json!({"title":v[if mac {"title"} else {"label"}],"url":url}))
        }).collect::<Vec<_>>()
    });
    result["news_sources"] = list(&s[source_field(mac)])
        .iter()
        .filter(|v| personal(s, v, mac))
        .map(|v| {
            let path = if mac {
                let p = text(v, "folder_path");
                if p.is_empty() {
                    json!([])
                } else {
                    json!(p.split('/').map(str::trim).collect::<Vec<_>>())
                }
            } else {
                v.get("folder_path").cloned().unwrap_or(json!([]))
            };
            item(
                &format!("{lang}:{}", text(v, "url")),
                json!({"language":lang,"title":v["title"],"url":v["url"],"folder_path":path}),
            )
        })
        .collect::<Vec<_>>()
        .into();
    let folders = if mac {
        list(&s["article_folders"])
            .iter()
            .map(|v| {
                json!(
                    v.as_str()
                        .unwrap_or("")
                        .split('/')
                        .map(str::trim)
                        .collect::<Vec<_>>()
                )
            })
            .collect()
    } else {
        list(&s["rss_folders_by_language"][&lang])
    };
    result["news_folders"] = folders
        .iter()
        .map(|path| {
            item(
                &format!("{lang}:{path}"),
                json!({"language":lang,"path":path}),
            )
        })
        .collect::<Vec<_>>()
        .into();
    if !mac {
        result["news_articles"] = list(&s["rss_favorite_articles"])
            .iter()
            .map(|v| {
                item(
                    text(v, "link"),
                    json!({"title":v["title"],"link":v["link"],"description":v["description"]}),
                )
            })
            .collect::<Vec<_>>()
            .into();
    }
    result["preferences"] = crate::cloud_sync_preferences::export(s, mac);
    result
}

pub fn project(remote: &Value, s: &Value, mac: bool) -> Result<Value, String> {
    let fields: &[(&str, &[&str])] = &[
        ("radio", &["name", "stream_url", "language_code"]),
        ("tv", &["name"]),
        ("podcasts", &["title", "url"]),
        ("sonartube", &["title", "url"]),
        ("sonarmusic", MUSIC_FIELDS),
        ("podcast_bookmarks", crate::podcast_bookmarks::FIELDS),
        ("news_sources", &["language", "title", "url", "folder_path"]),
        ("news_folders", &["language", "path"]),
        ("news_articles", &["title", "link", "description"]),
    ];
    if !remote.is_object() {
        return Err("invalid_response".into());
    }
    let mut out = json!({});
    for &(name, keys) in fields {
        if mac && name == "news_articles" {
            continue;
        }
        let mut rows = Vec::new();
        let mut seen = std::collections::HashSet::new();
        if let Some(value) = remote.get(name) {
            let values = value.as_array().ok_or("invalid_response")?;
            for v in values {
                if name == "news_sources" && virtual_news_source(&v["data"]) {
                    continue;
                }
                if (name == "news_sources" || name == "news_folders")
                    && text(&v["data"], "language") != language(s, mac)
                {
                    continue;
                }
                let id = v["id"].as_str().ok_or("invalid_response")?;
                if id.is_empty() || !seen.insert(id) {
                    return Err("invalid_response".into());
                }
                let mut data = json!({});
                for key in keys {
                    let value = v["data"].get(*key).ok_or("invalid_response")?;
                    if *key == "path" || *key == "folder_path" {
                        if !value
                            .as_array()
                            .is_some_and(|p| p.iter().all(Value::is_string))
                        {
                            return Err("invalid_response".into());
                        }
                    } else if name == "sonarmusic" && *key == "available" {
                        if !value.is_boolean() {
                            return Err("invalid_response".into());
                        }
                    } else if name == "podcast_bookmarks"
                        && ["position_seconds", "updated_at"].contains(key)
                    {
                        if value.as_u64().is_none() {
                            return Err("invalid_response".into());
                        }
                    } else if !value.is_string() {
                        return Err("invalid_response".into());
                    }
                    if ["url", "stream_url", "link"].contains(key) {
                        let url = reqwest::Url::parse(value.as_str().unwrap_or(""))
                            .map_err(|_| "invalid_response")?;
                        if !["http", "https"].contains(&url.scheme()) || url.host_str().is_none() {
                            return Err("invalid_response".into());
                        }
                    }
                    data[*key] = value.clone();
                }
                if name == "sonarmusic"
                    && (text(&data, "id").is_empty()
                        || text(&data, "kind").is_empty()
                        || music_id(&data) != id)
                {
                    return Err("invalid_response".into());
                }
                if name == "podcast_bookmarks" && !crate::podcast_bookmarks::valid(&data, id) {
                    return Err("invalid_response".into());
                }
                rows.push(item(id, data));
            }
        }
        out[name] = rows.into();
    }
    out["preferences"] = crate::cloud_sync_preferences::project(&remote["preferences"], mac)?;
    Ok(out)
}

pub fn apply(s: &mut Value, state: &Value, mac: bool) -> Result<(), String> {
    let mut complete = export(s, mac);
    if let Some(incoming) = state.as_object() {
        for (key, value) in incoming {
            complete[key] = value.clone();
        }
    }
    let state = project(&complete, s, mac)?;
    for (collection, field, key) in [
        ("radio", "radio_favorites", "stream_url"),
        ("tv", "tv_favorites", "name"),
        ("podcasts", "podcast_sources", "url"),
        (
            "sonartube",
            if mac {
                "youtube_favorites"
            } else {
                "stream_favorites"
            },
            "url",
        ),
        ("news_articles", "rss_favorite_articles", "link"),
    ] {
        if mac && collection == "news_articles" {
            continue;
        }
        let old = list(&s[field]);
        let mut next = if collection == "sonartube" {
            old.iter().filter(|v| !is_youtube(v)).cloned().collect()
        } else {
            Vec::new()
        };
        for row in list(&state[collection]) {
            let data = &row["data"];
            let mut v = old
                .iter()
                .find(|v| text(v, key) == text(data, key))
                .cloned()
                .unwrap_or_else(|| json!({}));
            for (k, value) in data.as_object().ok_or("invalid_response")? {
                v[if collection == "sonartube" && !mac && k == "title" {
                    "label"
                } else {
                    k
                }] = value.clone();
            }
            if collection == "tv" && v.get("url").is_none() {
                v["url"] = json!("");
            }
            if collection == "tv" && !mac && v.get("category").is_none() {
                v["category"] = json!("");
            }
            if collection == "podcasts" && !mac && v.get("kind").is_none() {
                v["kind"] = json!("Feed");
            }
            next.push(v);
        }
        s[field] = next.into();
    }
    s["podcast_bookmarks"] = list(&state["podcast_bookmarks"])
        .iter()
        .map(|r| r["data"].clone())
        .collect::<Vec<_>>()
        .into();
    let old_music = list(&s["sonarmusic_favorites"]);
    s["sonarmusic_favorites"] = list(&state["sonarmusic"])
        .iter()
        .map(|row| {
            let mut value = old_music
                .iter()
                .find(|v| music_id(v) == text(row, "id"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            for key in MUSIC_FIELDS {
                value[*key] = row["data"][*key].clone();
            }
            value
        })
        .collect::<Vec<_>>()
        .into();
    let lang = language(s, mac);
    let old = list(&s[source_field(mac)]);
    let mut next: Vec<Value> = old
        .iter()
        .filter(|v| !personal(s, v, mac))
        .cloned()
        .collect();
    for row in list(&state["news_sources"]) {
        let d = &row["data"];
        let mut source = old
            .iter()
            .find(|v| v["url"] == d["url"])
            .cloned()
            .unwrap_or_else(|| json!({"kind":"Feed","user_title":true}));
        source["title"] = d["title"].clone();
        source["url"] = d["url"].clone();
        source["folder_path"] = if mac {
            json!(
                list(&d["folder_path"])
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("/")
            )
        } else {
            d["folder_path"].clone()
        };
        next.retain(|v| v["url"] != source["url"]);
        next.push(source);
    }
    s[source_field(mac)] = json!(next);
    if mac {
        s["article_folders"] = list(&state["news_folders"])
            .iter()
            .map(|v| {
                json!(
                    list(&v["data"]["path"])
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("/")
                )
            })
            .collect::<Vec<_>>()
            .into();
    } else {
        s["rss_sources_by_language"][&lang] = json!(next);
        s["rss_folders_by_language"][&lang] = list(&state["news_folders"])
            .iter()
            .map(|v| v["data"]["path"].clone())
            .collect::<Vec<_>>()
            .into();
    }
    crate::cloud_sync_preferences::apply(s, &state["preferences"], mac)?;
    let next_language = language(s, mac);
    if !mac && next_language != lang {
        // The old language's sources were already checkpointed above. Reuse
        // the existing per-language cache instead of relabeling them.
        s["rss_sources"] = s["rss_sources_by_language"]
            .get(&next_language)
            .cloned()
            .unwrap_or(json!([]));
    }
    Ok(())
}

pub fn reconcile_news_language(
    session: &Session,
    settings: &mut Value,
    remote: &mut Value,
    mac: bool,
) -> Result<(), String> {
    let next = project(&session.meta["response"]["state"], settings, mac)?;
    let base = project(&session.meta["baseline"], settings, mac)?;
    let local = export(settings, mac);
    let keys = ["news_sources", "news_folders"];
    let pick = |state: &Value| -> Value {
        keys.iter()
            .map(|key| ((*key).to_string(), state[*key].clone()))
            .collect()
    };
    apply(
        settings,
        &merge(&pick(&next), &pick(&base), &pick(&local)),
        mac,
    )?;
    for key in keys {
        remote[key] = next[key].clone();
    }
    Ok(())
}

/// Compare identities, not only counts: a replacement can hide a deletion.
pub fn removed_items(before: &Value, after: &Value) -> Value {
    let mut removed = json!({});
    if let Some(collections) = after.as_object() {
        for (name, rows) in collections {
            let remaining = list(rows);
            let missing: Vec<Value> = list(&before[name])
                .into_iter()
                .filter(|row| !remaining.iter().any(|v| v["id"] == row["id"]))
                .collect();
            if !missing.is_empty() {
                removed[name] = json!(missing);
            }
        }
    }
    removed
}

// Separate from the retry journal: background sync must never invent user intent.
static DELETION_EVENTS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn deletion_events(dir: &std::path::Path) -> Result<Value, String> {
    match std::fs::read_to_string(dir.join("cloud_deletion_events.json")) {
        Ok(raw) => serde_json::from_str(&raw).map_err(|_| "storage_error".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!([])),
        Err(_) => Err("storage_error".into()),
    }
}
fn write_deletion_events(dir: &std::path::Path, events: &Value) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|_| "storage_error")?;
    let path = dir.join("cloud_deletion_events.json");
    let temp = path.with_extension("tmp");
    let mut file = std::fs::File::create(&temp).map_err(|_| "storage_error")?;
    file.write_all(events.to_string().as_bytes())
        .map_err(|_| "storage_error")?;
    file.sync_all().map_err(|_| "storage_error")?;
    std::fs::rename(temp, path).map_err(|_| "storage_error".into())
}
pub fn record_explicit_deletions(
    dir: &std::path::Path,
    before: &Value,
    after: &Value,
) -> Result<(), String> {
    let removed = removed_items(before, after);
    if removed.as_object().is_none_or(|v| v.is_empty()) {
        return Ok(());
    }
    let _lock = DELETION_EVENTS_LOCK.lock().map_err(|_| "storage_error")?;
    let meta: Value = match std::fs::read_to_string(dir.join("cloud_sync.json")) {
        Ok(raw) => serde_json::from_str(&raw).map_err(|_| "storage_error")?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("storage_error".into()),
    };
    if !meta["group_id"].is_string() {
        return Ok(());
    }
    let mut events = list(&deletion_events(dir)?);
    for (collection, rows) in removed.as_object().ok_or("storage_error")? {
        for row in list(rows) {
            events.push(json!({"event":format!("{:016x}",rand::random::<u64>()),"group_id":meta["group_id"],"collection":collection,"id":row["id"]}));
        }
    }
    write_deletion_events(dir, &json!(events))?;
    log_collection_reductions(before, after, "explicit_user_removal", crate::log_debug);
    Ok(())
}
fn covered_deletions(removed: &Value, allowed: &Value) -> bool {
    removed.as_object().is_some_and(|collections| {
        collections.iter().all(|(name, rows)| {
            list(rows)
                .iter()
                .all(|row| list(&allowed[name]).iter().any(|id| *id == row["id"]))
        })
    })
}

fn explicit_deletions(events: &[Value]) -> Value {
    let mut allowed = json!({});
    for event in events {
        let key = text(event, "collection");
        if !allowed[key].is_array() {
            allowed[key] = json!([]);
        }
        if let Some(ids) = allowed[key].as_array_mut() {
            ids.push(event["id"].clone());
        }
    }
    allowed
}

pub fn preserve_collection_write(dir: &Path, before: &Value, after: &Value) -> Result<(), String> {
    if removed_items(before, after)
        .as_object()
        .is_none_or(|v| v.is_empty())
    {
        return Ok(());
    }
    let path = dir.join("cloud_collections_recovery.json");
    let mut recovery: Vec<Value> = match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).map_err(|_| "recovery_storage_error")?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(_) => return Err("recovery_storage_error".into()),
    };
    let collections: Value = before
        .as_object()
        .into_iter()
        .flatten()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if recovery
        .last()
        .is_some_and(|row| row["state"] == collections)
    {
        return Ok(());
    }
    recovery.push(json!({"time":now_seconds(),"state":collections}));
    if recovery.len() > 10 {
        recovery.remove(0);
    }
    std::fs::create_dir_all(dir).map_err(|_| "recovery_storage_error")?;
    let temp = path.with_extension(format!("{}.tmp", rand::random::<u64>()));
    let result = (|| -> Result<(), String> {
        let mut file = std::fs::File::create(&temp).map_err(|_| "recovery_storage_error")?;
        file.write_all(
            serde_json::to_string(&recovery)
                .map_err(|_| "recovery_storage_error")?
                .as_bytes(),
        )
        .map_err(|_| "recovery_storage_error")?;
        file.sync_all().map_err(|_| "recovery_storage_error")?;
        std::fs::rename(&temp, path).map_err(|_| "recovery_storage_error".into())
    })();
    if result.is_err() && temp.exists() {
        let _cleanup = std::fs::remove_file(temp);
    }
    result
}

pub fn log_collection_reductions(before: &Value, after: &Value, stage: &str, log: fn(&str)) {
    use sha2::{Digest, Sha256};
    for row in list(&after["preferences"]) {
        let id = text(&row, "id");
        if id.is_empty()
            || id.len() > 80
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'.')
        {
            continue;
        }
        if list(&before["preferences"])
            .iter()
            .any(|old| old["id"] == row["id"] && old["data"] != row["data"])
        {
            // Preference values can contain credentials: log the key only.
            log(&format!(
                "CloudSync preference_changed stage={stage} preference_id={id}"
            ));
        }
    }
    if let Some(collections) = removed_items(before, after).as_object() {
        for (name, rows) in collections {
            log(&format!(
                "CloudSync reduction stage={stage} collection={name} before={} after={} removed={}",
                list(&before[name]).len(),
                list(&after[name]).len(),
                list(rows).len()
            ));
            for row in list(rows) {
                let hash = format!("{:x}", Sha256::digest(text(&row, "id").as_bytes()));
                log(&format!(
                    "CloudSync removed_item stage={stage} collection={name} id_sha256={hash}"
                ));
            }
        }
    }
}

pub fn deletion_review_text(review: &Value, italian: bool) -> String {
    let mut message = if italian {
        "Rimozione inattesa bloccata. La sincronizzazione non propaga eliminazioni senza un comando esplicito."
    } else { "Unexpected removal blocked. Sync does not propagate deletions without an explicit removal command." }.to_string();
    if let Some(collections) = removed_items(&review["before"], &review["after"]).as_object() {
        for (name, rows) in collections {
            let label = match name.as_str() {
                "preferences" => {
                    if italian {
                        "Preferenze"
                    } else {
                        "Preferences"
                    }
                }
                "radio" => "Radio",
                "tv" => "TV",
                "podcasts" => "Podcast",
                "sonartube" => "SonarTube",
                "sonarmusic" => "SonarMusic",
                "podcast_bookmarks" => {
                    if italian {
                        "Segnalibri podcast"
                    } else {
                        "Podcast bookmarks"
                    }
                }
                "news_sources" => {
                    if italian {
                        "Fonti notizie"
                    } else {
                        "News sources"
                    }
                }
                "news_folders" => {
                    if italian {
                        "Cartelle notizie"
                    } else {
                        "News folders"
                    }
                }
                "news_articles" => {
                    if italian {
                        "Articoli preferiti"
                    } else {
                        "Favorite articles"
                    }
                }
                _ => name,
            };
            message.push_str(&format!(
                "\n{label}: {} → {}; {} {}",
                list(&review["before"][name]).len(),
                list(&review["after"][name]).len(),
                list(rows).len(),
                if italian { "eliminazioni" } else { "deletions" }
            ));
        }
    }
    message.push_str(if italian {
        "\n\nQueste eliminazioni non sono state propagate. Controlla i dati e il log diagnostico."
    } else {
        "\n\nThese deletions were not propagated. Check your data and the diagnostic log."
    });
    message
}

pub fn merge(remote: &Value, base: &Value, local: &Value) -> Value {
    let mut result = remote.clone();
    if let Some(collections) = local.as_object() {
        for (name, rows) in collections {
            let before = list(&base[name]);
            let after = list(rows);
            let mut current = list(&remote[name]);
            current.retain(|v| {
                !before.iter().any(|b| b["id"] == v["id"])
                    || after.iter().any(|a| a["id"] == v["id"])
            });
            for row in &after {
                let old = before.iter().find(|b| b["id"] == row["id"]);
                let pos = current.iter().position(|v| v["id"] == row["id"]);
                // A resume point is atomic; delayed/offline devices must not rewind it.
                if name == "podcast_bookmarks" {
                    if let Some(i) = pos {
                        if old != Some(row)
                            && row["data"]["updated_at"].as_u64().unwrap_or(0)
                                > current[i]["data"]["updated_at"].as_u64().unwrap_or(0)
                        {
                            current[i] = row.clone();
                        }
                    } else if old.is_none() {
                        current.push(row.clone());
                    }
                    continue;
                }
                match (old, pos) {
                    (Some(b), Some(i)) => {
                        if let Some(fields) = row["data"].as_object() {
                            for (k, v) in fields {
                                if b["data"].get(k) != Some(v) {
                                    current[i]["data"][k] = v.clone();
                                }
                            }
                        }
                        if let Some(fields) = b["data"].as_object() {
                            for k in fields.keys() {
                                if row["data"].get(k).is_none()
                                    && let Some(d) = current[i]["data"].as_object_mut()
                                {
                                    d.remove(k);
                                }
                            }
                        }
                    }
                    (None, Some(i)) => {
                        if let Some(fields) = row["data"].as_object() {
                            for (k, v) in fields {
                                current[i]["data"][k] = v.clone();
                            }
                        }
                    }
                    (None, None) => current.push(row.clone()),
                    (Some(_), None) => {}
                }
            }
            let ids = |v: &Vec<Value>| v.iter().map(|r| r["id"].clone()).collect::<Vec<_>>();
            if ids(&before) != ids(&after) {
                let mut ordered = Vec::new();
                for row in &after {
                    if let Some(i) = current.iter().position(|v| v["id"] == row["id"]) {
                        ordered.push(current.remove(i));
                    }
                }
                ordered.extend(current);
                current = ordered;
            }
            result[name] = current.into();
        }
    }
    result
}

pub struct Session {
    pub path: PathBuf,
    pub meta: Value,
    token: String,
}
impl Session {
    pub fn metadata(dir: &Path) -> Result<Value, String> {
        let path = dir.join("cloud_sync.json");
        match std::fs::read(&path) {
            Ok(bytes) => {
                let meta: Value = serde_json::from_slice(&bytes).map_err(|_| "storage_error")?;
                if !meta.is_object() {
                    return Err("storage_error".into());
                }
                Ok(meta)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
            Err(_) => Err("storage_error".into()),
        }
    }
    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("cloud_sync.json");
        let meta = Self::metadata(dir)?;
        let entry = keyring::Entry::new("com.sonarpad.cloudsync", &dir.to_string_lossy())
            .map_err(|_| "credential_error")?;
        let token = match entry.get_password() {
            Ok(v) => v,
            Err(keyring::Error::NoEntry) => {
                use rand::RngCore;
                let mut bytes = [0u8; 32];
                rand::rngs::OsRng.fill_bytes(&mut bytes);
                let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
                entry.set_password(&token).map_err(|_| "credential_error")?;
                token
            }
            Err(_) => return Err("credential_error".into()),
        };
        Ok(Self { path, meta, token })
    }
    pub fn save(&self) -> Result<(), String> {
        let temp = self.path.with_extension("json.tmp");
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|_| "storage_error")?;
        }
        {
            let mut f = std::fs::File::create(&temp).map_err(|_| "storage_error")?;
            f.write_all(
                serde_json::to_string(&self.meta)
                    .map_err(|_| "storage_error")?
                    .as_bytes(),
            )
            .map_err(|_| "storage_error")?;
            f.sync_all().map_err(|_| "storage_error")?;
        }
        std::fs::rename(temp, &self.path).map_err(|_| "storage_error".into())
    }
    pub fn select(&self, state: &Value) -> Value {
        let mut selected = json!({});
        if let Some(map) = state.as_object() {
            for (key, value) in map {
                let enabled = if let Some(pending) = self.meta.get("pending") {
                    pending["state"].get(key).is_some()
                } else {
                    self.meta[if key == "preferences" {
                        "sync_preferences"
                    } else {
                        "sync_collections"
                    }] != json!(false)
                };
                if enabled {
                    selected[key] = value.clone();
                }
            }
        }
        selected
    }
    pub fn toggle_scope(&mut self, preferences: bool) -> Result<bool, String> {
        if self.meta.get("pending").is_some() {
            return Err("pending_sync".into());
        }
        let key = if preferences {
            "sync_preferences"
        } else {
            "sync_collections"
        };
        let enabled = self.meta[key] == json!(false);
        self.meta[key] = json!(enabled);
        self.save()?;
        Ok(enabled)
    }
    pub fn connected(&self) -> bool {
        self.meta["group_id"].is_string()
    }
    pub fn request(&self, action: &str, body: &Value, log: fn(&str)) -> Result<Value, String> {
        let start = Instant::now();
        log(&format!("CloudSync action={action} started"));
        let result = (|| {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("SonarpadSync/1.0")
                .build()
                .map_err(|_| "network_error")?;
            let mut r = client
                .post(ENDPOINT)
                .query(&[("action", action)])
                .bearer_auth(&self.token)
                .json(body)
                .send()
                .map_err(|_| "network_error")?;
            let status = r.status().as_u16();
            let request_id = r
                .headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .filter(|s| s.len() == 16 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                .unwrap_or("-");
            log(&format!(
                "CloudSync action={action} status={status} duration_ms={} request_id={request_id}",
                start.elapsed().as_millis()
            ));
            let mut bytes = Vec::new();
            r.by_ref()
                .take(5 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "network_error")?;
            if bytes.len() > 5 * 1024 * 1024 {
                return Err("invalid_response".into());
            }
            let value: Value = serde_json::from_slice(&bytes).map_err(|_| "invalid_response")?;
            if status != 200 {
                return Err(match text(&value, "error") {
                    "unauthorized" => "unauthorized",
                    "invalid_or_expired_code" | "invalid_code" => "invalid_code",
                    "rate_limited" => "rate_limited",
                    "deletion_confirmation_required" => "deletion_confirmation_required",
                    _ => "server_error",
                }
                .into());
            }
            Ok(value)
        })();
        if let Err(ref safe) = result {
            log(&format!(
                "CloudSync action={action} error={safe} duration_ms={}",
                start.elapsed().as_millis()
            ));
        }
        result
    }
    pub fn connect(&mut self, code: Option<&str>, log: fn(&str)) -> Result<(), String> {
        if self.connected() {
            return Ok(());
        }
        let mut body = json!({"device_name":format!("Sonarpad {}",std::env::consts::OS)});
        if let Some(code) = code {
            body["code"] = json!(code.trim());
        }
        let response = self.request(if code.is_some() { "join" } else { "create" }, &body, log)?;
        if !response["group_id"].is_string() || !response["sequence"].is_u64() {
            return Err("invalid_response".into());
        }
        self.meta["group_id"] = response["group_id"].clone();
        self.meta["sequence"] = response["sequence"].clone();
        self.meta["baseline"] = json!({});
        self.save()
    }
    pub fn guard_deletions(
        &mut self,
        direction: &str,
        before: &Value,
        after: &Value,
    ) -> Result<Value, String> {
        log_collection_reductions(before, after, direction, crate::log_debug);
        if removed_items(before, after)
            .as_object()
            .is_none_or(|v| v.is_empty())
        {
            return Ok(after.clone());
        }
        let collections = |state: &Value| -> Value {
            state
                .as_object()
                .into_iter()
                .flatten()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        };
        let review =
            json!({"direction":direction,"before":collections(before),"after":collections(after)});
        // Keep ten pre-removal copies independently of the retry journal.
        let mut recovery = list(&self.meta["recovery"]);
        if recovery
            .last()
            .is_none_or(|v| v["state"] != review["before"])
        {
            recovery.push(json!({"time":now_seconds(),"state":review["before"]}));
            if recovery.len() > 10 {
                recovery.remove(0);
            }
            self.meta["recovery"] = json!(recovery);
        }
        let allowed = if direction == "local_during_sync" {
            let events: Vec<Value> = list(&deletion_events(
                self.path.parent().ok_or("storage_error")?,
            )?)
            .into_iter()
            .filter(|event| event["group_id"] == self.meta["group_id"])
            .collect();
            explicit_deletions(&events)
        } else if direction == "outgoing" {
            self.meta["pending"]["explicit_deletions"].clone()
        } else {
            self.meta["response"]["authorized_deletions"].clone()
        };
        if covered_deletions(&removed_items(before, after), &allowed) {
            self.save()?;
            return Ok(after.clone());
        }
        // Cancel only unproven removals, preserving additions, edits and explicit
        // user deletions in the same batch. The rest of sync can then proceed.
        let mut safe = after.clone();
        for (name, rows) in removed_items(before, after)
            .as_object()
            .ok_or("storage_error")?
        {
            let restored: Vec<Value> = list(rows)
                .into_iter()
                .filter(|row| !list(&allowed[name]).contains(&row["id"]))
                .collect();
            if restored.is_empty() {
                continue;
            }
            crate::log_debug(&format!(
                "CloudSync deletion_cancelled direction={direction} collection={name} restored={} sync_continues=true",
                restored.len()
            ));
            safe[name]
                .as_array_mut()
                .ok_or("storage_error")?
                .extend(restored);
        }
        log_collection_reductions(
            &safe,
            after,
            &format!("cancelled_{direction}"),
            crate::log_debug,
        );
        self.meta["last_cancelled_deletions"] = review;
        self.meta
            .as_object_mut()
            .ok_or("storage_error")?
            .remove("deletion_review");
        self.save()?;
        Ok(safe)
    }

    fn prepare_pending_state(&mut self) -> Result<Value, String> {
        // Keep the actual local snapshot separate from the repaired wire state.
        // Otherwise the merge would interpret the same missing items as fresh
        // local deletions and discard the just-recovered favorites again.
        if self.meta.get("pending_local_state").is_none() {
            self.meta["pending_local_state"] = self.meta["pending"]["state"].clone();
        }
        let before = self.meta["pending"]["baseline"].clone();
        let after = self.meta["pending"]["state"].clone();
        let safe = self.guard_deletions("outgoing", &before, &after)?;
        self.meta["pending"]["state"] = safe.clone();
        self.save()?;
        Ok(safe)
    }

    pub fn merge_response(&mut self, remote: &Value, current: &Value) -> Result<Value, String> {
        let base = self
            .meta
            .get("pending_local_state")
            .unwrap_or(&self.meta["pending"]["state"])
            .clone();
        let current = self.guard_deletions("local_during_sync", &base, current)?;
        let merged = merge(remote, &base, &current);
        // Protect recovered outgoing items too, even if the local snapshot was
        // empty and an unproven server reduction omits the same items.
        let expected = merge(&self.meta["pending"]["state"], &base, &current);
        self.guard_deletions("incoming", &expected, &merged)
    }
    pub fn sync(&mut self, settings: &Value, mac: bool, log: fn(&str)) -> Result<Value, String> {
        if !self.connected() {
            return Err("unauthorized".into());
        }
        if self.meta["blocked_before_send"] == true && self.meta.get("response").is_none() {
            self.meta
                .as_object_mut()
                .ok_or("storage_error")?
                .remove("pending");
            self.meta
                .as_object_mut()
                .ok_or("storage_error")?
                .remove("blocked_before_send");
            self.meta
                .as_object_mut()
                .ok_or("storage_error")?
                .remove("pending_local_state");
        }
        if self.meta.get("pending").is_none() {
            self.meta["pending"] = json!({"schema_version":1,"sequence":self.meta["sequence"].as_u64().unwrap_or(0)+1,
                "baseline":self.select(&project(&self.meta["baseline"],settings,mac)?),"state":self.select(&export(settings,mac))});
            let events = list(&deletion_events(
                self.path.parent().ok_or("storage_error")?,
            )?)
            .into_iter()
            .filter(|event| event["group_id"] == self.meta["group_id"])
            .collect::<Vec<_>>();
            self.meta["pending"]["explicit_deletions"] = explicit_deletions(&events);
            self.meta["pending"]["deletion_events"] = json!(events);
            self.save()?;
        }
        let before = self.meta["pending"]["baseline"].clone();
        let after = self.prepare_pending_state()?;
        if self.meta.get("response").is_none()
            && removed_items(&before, &after)
                .as_object()
                .is_some_and(|v| !v.is_empty())
        {
            let approval = self.request("confirm_deletions", &self.meta["pending"], log)?;
            if approval["confirmed"] != true {
                return Err("invalid_response".into());
            }
        }
        let response = if let Some(v) = self.meta.get("response") {
            v.clone()
        } else {
            self.request("sync", &self.meta["pending"], log)?
        };
        if response["group_id"] != self.meta["group_id"]
            || response["sequence"] != self.meta["pending"]["sequence"]
        {
            return Err("invalid_response".into());
        }
        // A missing collection is a malformed response, not an empty collection.
        if let Some(sent) = after.as_object() {
            for key in sent.keys() {
                if !response["state"][key].is_array() {
                    return Err("invalid_response".into());
                }
            }
        }
        let state = self.select(&project(&response["state"], settings, mac)?);
        self.meta["response"] = response;
        self.save()?;
        Ok(state)
    }
    pub fn finish(&mut self, baseline: Value) -> Result<(), String> {
        if !self.meta["baseline"].is_object() {
            self.meta["baseline"] = json!({});
        }
        if let Some(values) = baseline.as_object() {
            for (key, value) in values {
                self.meta["baseline"][key] = value.clone();
            }
        }
        self.meta["sequence"] = self.meta["response"]["sequence"].clone();
        self.meta["last_sync"] = json!(now_seconds());
        {
            let _lock = DELETION_EVENTS_LOCK.lock().map_err(|_| "storage_error")?;
            let dir = self.path.parent().ok_or("storage_error")?;
            let sent = list(&self.meta["pending"]["deletion_events"]);
            let remaining: Vec<Value> = list(&deletion_events(dir)?)
                .into_iter()
                .filter(|event| !sent.iter().any(|old| old["event"] == event["event"]))
                .collect();
            write_deletion_events(dir, &json!(remaining))?;
        }
        if let Some(m) = self.meta.as_object_mut() {
            m.remove("approved_outgoing");
            m.remove("approved_incoming");
            m.remove("deletion_review");
            m.remove("blocked_before_send");
            m.remove("pending_local_state");
            m.remove("pending");
            m.remove("response");
        }
        self.save()
    }
    pub fn disconnect(&mut self, log: fn(&str)) -> Result<(), String> {
        if let Err(e) = self.request("disconnect", &json!({}), log)
            && e != "unauthorized"
        {
            return Err(e);
        }
        let dir = self.path.parent().ok_or("storage_error")?;
        keyring::Entry::new("com.sonarpad.cloudsync", &dir.to_string_lossy())
            .map_err(|_| "credential_error")?
            .delete_credential()
            .map_err(|_| "credential_error")?;
        self.meta = json!({});
        self.save()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn podcast_resume_wire_round_trip_and_private_cache() {
        let wire: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/podcast_bookmarks_sync.json"
        ))
        .unwrap();
        for mac in [false, true] {
            let mut settings = json!({"podcast_cache_sources":{"C:\\cache\\random.mp3":"https://podcast.example/episode.mp3?episode=1"}});
            let private_cache = settings["podcast_cache_sources"].clone();
            apply(&mut settings, &wire, mac).unwrap();
            assert_eq!(
                export(&settings, mac)["podcast_bookmarks"],
                wire["podcast_bookmarks"]
            );
            assert!(
                export(&settings, mac)
                    .get("podcast_cache_sources")
                    .is_none()
            );
            assert_eq!(settings["podcast_cache_sources"], private_cache);
            let rows = list(&settings["podcast_bookmarks"]);
            assert_eq!(
                crate::podcast_bookmarks::position(
                    &rows,
                    "https://podcast.example/episode.mp3?episode=1"
                ),
                Some(123)
            );
            assert_eq!(
                crate::podcast_bookmarks::position(
                    &rows,
                    "https://podcast.example/episode.mp3?episode=2"
                ),
                Some(456)
            );
            apply(&mut settings, &json!({"radio":[]}), mac).unwrap();
            assert_eq!(list(&settings["podcast_bookmarks"]), rows);
            let before = settings.clone();
            let mut invalid = wire.clone();
            invalid["podcast_bookmarks"][0]["data"]["position_seconds"] = json!(-1);
            assert!(apply(&mut settings, &invalid, mac).is_err());
            assert_eq!(settings, before);
        }
    }

    #[test]
    fn podcast_resume_conflicts_keep_latest_atomic_point_including_reset() {
        let base: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/podcast_bookmarks_sync.json"
        ))
        .unwrap();
        let mut older = base.clone();
        older["podcast_bookmarks"][0]["data"]["position_seconds"] = json!(180);
        older["podcast_bookmarks"][0]["data"]["updated_at"] = json!(1794564200000u64);
        let mut latest = base.clone();
        latest["podcast_bookmarks"][0]["data"]["position_seconds"] = json!(0);
        latest["podcast_bookmarks"][0]["data"]["updated_at"] = json!(1794564300000u64);
        for initial in [&base, &json!({})] {
            assert_eq!(
                merge(&latest, initial, &older)["podcast_bookmarks"],
                latest["podcast_bookmarks"]
            );
            assert_eq!(
                merge(&older, initial, &latest)["podcast_bookmarks"],
                latest["podcast_bookmarks"]
            );
        }
        assert_eq!(
            merge(&latest, &base, &base)["podcast_bookmarks"],
            latest["podcast_bookmarks"]
        );
        assert_eq!(
            merge(&latest, &json!({}), &json!({"radio":[]}))["podcast_bookmarks"],
            latest["podcast_bookmarks"]
        );
    }

    use super::*;
    #[test]
    fn automatic_schedule_batches_retries_and_periodically_pulls() {
        let mut schedule = AutoSync::default();
        let mut meta = json!({"group_id":"g","last_sync":1000});
        assert!(!schedule.due(&json!({}), true, 1000));
        assert!(schedule.due(&meta, false, 1000));
        schedule.completed(true, 1000);
        assert!(!schedule.due(&meta, true, 1010));
        assert!(schedule.due(&meta, true, 1030));
        schedule.completed(false, 1030);
        assert!(!schedule.due(&meta, true, 1059));
        assert!(schedule.due(&meta, true, 1060));
        schedule.completed(false, 1060);
        assert!(!schedule.due(&meta, true, 1119));
        assert!(schedule.due(&meta, true, 1120));
        schedule.completed(true, 1120);
        meta["last_sync"] = json!(1120);
        assert!(!schedule.failed());
        assert!(!schedule.due(&meta, false, 1299));
        assert!(schedule.due(&meta, false, 1300));
        meta["sync_preferences"] = json!(false);
        meta["sync_collections"] = json!(false);
        assert!(!schedule.due(&meta, true, 2000));
    }
    #[test]
    fn automatic_change_detection_uses_portable_values_and_selected_scopes() {
        let mut settings = json!({"rss_news_language":"it","news_language":"it",
            "radio_favorites":[],"weather_city_query":"Roma"});
        for mac in [false, true] {
            let mut meta =
                json!({"group_id":"g", "baseline":export(&settings, mac), "last_sync":1000});
            assert!(!local_changes(&meta, &settings, mac).unwrap());
            settings["weather_city_query"] = json!("Milano");
            assert!(local_changes(&meta, &settings, mac).unwrap());
            meta["sync_preferences"] = json!(false);
            assert!(!local_changes(&meta, &settings, mac).unwrap());
            meta["pending"] = json!({});
            assert!(local_changes(&meta, &settings, mac).unwrap());
            settings["weather_city_query"] = json!("Roma");
        }
        let message = status(&json!({"last_sync":1000}), true, false, true, true);
        assert!(message.contains("Riproverò automaticamente"));
        assert!(message.contains("Ultima sincronizzazione"));
    }

    #[test]
    fn virtual_favorites_source_survives_locally_and_does_not_block_old_requests() {
        let virtual_source = json!({"url":"sonarpad://rss/favorites","title":"Favorites","user_title":true,"folder_path":[]});
        let feed = json!({"url":"https://example.org/feed","title":"Feed","user_title":true,"folder_path":[]});
        let mut settings = json!({"rss_news_language":"it","rss_sources":[virtual_source.clone(),feed.clone()],"rss_sources_by_language":{},"rss_folders_by_language":{}});
        let clean = export(&settings, false);
        assert_eq!(clean["news_sources"].as_array().unwrap().len(), 1);
        let mut old_response = clean.clone();
        old_response["news_sources"].as_array_mut().unwrap().push(json!({"id":"it:sonarpad://rss/favorites","data":{"language":"it","url":"sonarpad://rss/favorites","title":"Favorites","folder_path":[]}}));
        let projected = project(&old_response, &settings, false).unwrap();
        assert_eq!(projected, clean);
        apply(&mut settings, &projected, false).unwrap();
        assert!(
            settings["rss_sources"]
                .as_array()
                .unwrap()
                .contains(&virtual_source)
        );
        assert!(settings["rss_sources"].as_array().unwrap().contains(&feed));
        assert!(project(&old_response, &json!({"news_language":"it"}), true).is_ok());
        old_response["news_sources"][0]["data"]["url"] = json!("file:///private/document");
        assert!(project(&old_response, &settings, false).is_err());
    }
    #[test]
    #[ignore = "Read-only diagnostic of an explicitly supplied local pending request"]
    fn diagnose_pending_sync_read_only() {
        let dir = PathBuf::from(
            std::env::var("SONARPAD_SYNC_DIAGNOSTIC_DIR").expect("diagnostic directory required"),
        );
        let meta: Value =
            serde_json::from_slice(&std::fs::read(dir.join("cloud_sync.json")).unwrap()).unwrap();
        let mut settings: Value =
            serde_json::from_slice(&std::fs::read(dir.join("settings.json")).unwrap()).unwrap();
        let snapshot = meta
            .get("pending")
            .map(|p| &p["state"])
            .unwrap_or(&meta["baseline"]);
        let projected = project(snapshot, &settings, false).unwrap();
        let rebased = merge(&projected, snapshot, &export(&settings, false));
        apply(&mut settings, &rebased, false).unwrap();
        let _restored: crate::settings::AppSettings =
            serde_json::from_value(settings.clone()).unwrap();
        // All work above is in memory: no credentials, network or settings writes.
        assert!(
            export(&settings, false)["news_sources"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| !virtual_news_source(&r["data"]))
        );
    }
    #[test]
    fn language_change_preserves_old_sources_and_loads_new_cloud_sources() {
        let old = json!({"title":"Italian personal","url":"https://example.org/it","user_title":true,"folder_path":[]});
        let cached = json!({"title":"Cached English","url":"https://example.org/cached","user_title":true,"folder_path":[]});
        let mut settings = json!({"rss_news_language":"it","rss_sources":[old.clone()],"rss_sources_by_language":{"en":[cached.clone()]},"rss_folders_by_language":{}});
        let initial = export(&settings, false);
        let mut full = initial.clone();
        full["preferences"] = json!([{"id":"news.language","data":{"value":"english"}}]);
        full["news_sources"].as_array_mut().unwrap().push(json!({"id":"en:https://example.org/remote","data":{"language":"en","title":"Remote English","url":"https://example.org/remote","folder_path":[]}}));
        let session = Session {
            path: PathBuf::new(),
            token: String::new(),
            meta: json!({"baseline":initial,"response":{"state":full}}),
        };
        let mut remote = project(&full, &settings, false).unwrap();
        apply(&mut settings, &remote, false).unwrap();
        reconcile_news_language(&session, &mut settings, &mut remote, false).unwrap();
        assert_eq!(settings["rss_news_language"], "en");
        assert_eq!(
            settings["rss_sources_by_language"]["it"][0]["url"],
            old["url"]
        );
        assert_eq!(settings["rss_sources"].as_array().unwrap().len(), 2);
        assert!(
            settings["rss_sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["url"] == cached["url"])
        );
        assert!(
            remote["news_sources"]
                .as_array()
                .unwrap()
                .iter()
                .all(|v| v["data"]["language"] == "en")
        );
    }
    #[test]
    fn scopes_preserve_unselected_collections() {
        let session = Session {
            path: PathBuf::new(),
            token: String::new(),
            meta: json!({"sync_collections":false}),
        };
        let settings = json!({"radio_favorites":[{"name":"Radio","stream_url":"https://radio.example/stream","language_code":"it"}],"media_cutter_advanced_mode":false});
        let selected = session.select(&export(&settings, false));
        assert_eq!(selected.as_object().unwrap().len(), 1);
        let mut next = settings.clone();
        apply(
            &mut next,
            &json!({"preferences":[{"id":"cutter.mode","data":{"value":"advanced"}}]}),
            false,
        )
        .unwrap();
        assert_eq!(next["radio_favorites"], settings["radio_favorites"]);
        assert_eq!(next["media_cutter_advanced_mode"], json!(true));
        let frozen = Session {
            meta: json!({"sync_preferences":false,"pending":{"state":{"preferences":[]}}}),
            ..session
        };
        assert!(
            frozen
                .select(&export(&settings, false))
                .get("preferences")
                .is_some()
        );
    }
    #[test]
    #[ignore = "Uses isolated temporary groups on the production sync service"]
    fn live_backend_round_trip() {
        fn quiet(_: &str) {}
        let token = || {
            use rand::RngCore;
            let mut bytes = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        };
        let first = Session {
            path: PathBuf::new(),
            meta: json!({}),
            token: token(),
        };
        let second = Session {
            path: PathBuf::new(),
            meta: json!({}),
            token: token(),
        };
        let created = first
            .request(
                "create",
                &json!({"device_name":"Sonarpad native sync test"}),
                quiet,
            )
            .unwrap_or_else(|e| panic!("{e}"));
        struct Cleanup<'a>(&'a Session, Value);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let cleanup = self
                    .0
                    .request("delete", &json!({"confirm_group_id":self.1}), quiet);
                assert!(cleanup.is_ok(), "temporary group cleanup failed");
            }
        }
        let _cleanup = Cleanup(&first, created["group_id"].clone());
        let payload = json!({"radio":[item("https://example.invalid/test",json!({"name":"Synthetic test","stream_url":"https://example.invalid/test","language_code":"it"}))]});
        let sent = first
            .request(
                "sync",
                &json!({"sequence":1,"baseline":{},"state":payload}),
                quiet,
            )
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(sent["state"], payload);
        let code = first
            .request("code", &json!({}), quiet)
            .unwrap_or_else(|e| panic!("{e}"));
        let joined = second
            .request(
                "join",
                &json!({"code":code["code"],"device_name":"Sonarpad second native test"}),
                quiet,
            )
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(joined["state"], payload);
        let deleted = second
            .request(
                "sync",
                &json!({"sequence":1,"baseline":payload,"state":{"radio":[]}}),
                quiet,
            )
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(deleted["state"]["radio"], json!([]));
    }

    #[test]
    fn cross_platform_collections_preserve_private_settings_and_metadata() {
        let mut win = json!({"password":"DO_NOT_SYNC","rss_news_language":"it","rss_sources_by_language":{},"rss_folders_by_language":{},
            "radio_favorites":[{"name":"Radio","stream_url":"https://example.invalid/radio","language_code":"it"}],
            "tv_favorites":[{"name":"TV","url":"https://example.invalid/tv","category":"News"}],
            "podcast_sources":[{"title":"Podcast","url":"https://example.invalid/feed","kind":"Feed","cache":{"etag":"local"}}],
            "stream_favorites":[{"label":"Video","url":"https://youtu.be/abc"},{"label":"Other stream","url":"https://example.invalid/stream"}],
            "rss_sources":[{"title":"Personal","url":"https://example.invalid/news","kind":"Feed","user_title":true,"folder_path":["News"]}],
            "rss_favorite_articles":[{"title":"Article","link":"https://example.invalid/article","description":"Text"}]});
        let state = export(&win, false);
        assert!(!state.to_string().contains("DO_NOT_SYNC"));
        assert!(!state.to_string().contains("etag"));
        assert_eq!(state["sonartube"].as_array().map(Vec::len), Some(1));
        let mut mac = json!({"news_language":"it","_cloud_default_urls":[],"article_sources":[],"article_folders":[]});
        apply(&mut mac, &state, true).unwrap_or_else(|e| panic!("{e}"));
        let from_mac = export(&mac, true);
        assert_eq!(from_mac["radio"], state["radio"]);
        assert_eq!(from_mac["sonartube"], state["sonartube"]);
        assert_eq!(from_mac["podcasts"], state["podcasts"]);
        assert_eq!(from_mac["news_sources"], state["news_sources"]);
        assert!(from_mac.get("news_articles").is_none());
        let mut returned = from_mac;
        returned["news_articles"] = state["news_articles"].clone();
        apply(&mut win, &returned, false).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(win["password"], "DO_NOT_SYNC");
        assert_eq!(win["podcast_sources"][0]["cache"]["etag"], "local");
        assert_eq!(win["tv_favorites"][0]["url"], "https://example.invalid/tv");
        assert_eq!(win["stream_favorites"].as_array().map(Vec::len), Some(2));
    }
    #[test]
    fn sonarmusic_shared_wire_round_trip_and_omitted_scope() {
        let wire: Value =
            serde_json::from_str(include_str!("../tests/fixtures/sonarmusic_sync.json")).unwrap();
        for mac in [false, true] {
            let mut settings =
                json!({"sonarmusic_favorites":[], "sonarmusic_history":[{"id":"local-history"}]});
            apply(&mut settings, &wire, mac).unwrap();
            assert_eq!(export(&settings, mac)["sonarmusic"], wire["sonarmusic"]);
            assert_eq!(
                settings["sonarmusic_favorites"].as_array().unwrap().len(),
                4
            );
            assert_eq!(
                settings["sonarmusic_favorites"][3]["browse_params"],
                "mix-params"
            );
            assert_eq!(settings["sonarmusic_history"][0]["id"], "local-history");
            apply(&mut settings, &json!({"radio":[]}), mac).unwrap();
            assert_eq!(export(&settings, mac)["sonarmusic"], wire["sonarmusic"]);
            let before = settings.clone();
            let mut invalid = wire.clone();
            invalid["sonarmusic"][0]["id"] = json!("wrong");
            assert!(apply(&mut settings, &invalid, mac).is_err());
            assert_eq!(settings, before);
        }
    }
    #[test]
    fn offline_deletion_wins_and_new_item_survives() {
        let base = json!({"radio":[item("a",json!({"name":"A"}))]});
        let local =
            json!({"radio":[item("a",json!({"name":"Edited"})),item("b",json!({"name":"B"}))]});
        let result = merge(
            &json!({"radio":[],"tv":[item("tv",json!({"name":"TV"}))]}),
            &base,
            &local,
        );
        assert_eq!(result["radio"], json!([item("b", json!({"name":"B"}))]));
        assert_eq!(result["tv"].as_array().map(Vec::len), Some(1));
    }
    #[test]
    fn rejects_invalid_cloud_url_before_local_writes() {
        let mut settings = json!({"radio_favorites":[]});
        let original = settings.clone();
        let remote = json!({"radio":[item("r",json!({"name":"R","stream_url":"file:///private","language_code":"it"}))]});
        assert!(apply(&mut settings, &remote, false).is_err());
        assert_eq!(settings, original);
    }
}

#[cfg(test)]
mod deletion_guard_tests {
    use super::*;
    #[test]
    fn explicit_events_are_durable_and_consumed_without_losing_new_actions() {
        let dir = std::env::temp_dir().join(format!(
            "sonarpad-intents-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut session = Session {
            path: dir.join("cloud_sync.json"),
            meta: json!({"group_id":"g"}),
            token: String::new(),
        };
        session.save().unwrap();
        let before = json!({"sonartube":[{"id":"a","data":{}},{"id":"b","data":{}}]});
        let after = json!({"sonartube":[{"id":"b","data":{}}]});
        record_explicit_deletions(&dir, &before, &after).unwrap();
        let events = deletion_events(&dir).unwrap();
        assert_eq!(events.as_array().unwrap().len(), 1);
        assert_eq!(events[0]["id"], "a");
        assert_eq!(events[0]["group_id"], "g");
        session.meta["pending"] = json!({"deletion_events":events});
        session.meta["response"] = json!({"sequence":2});
        record_explicit_deletions(&dir, &after, &json!({"sonartube":[]})).unwrap();
        session.finish(after).unwrap();
        let remaining = deletion_events(&dir).unwrap();
        assert_eq!(remaining.as_array().unwrap().len(), 1);
        assert_eq!(remaining[0]["id"], "b");
        std::fs::remove_file(dir.join("cloud_sync.json")).unwrap();
        std::fs::remove_file(dir.join("cloud_deletion_events.json")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn reductions_restore_only_unproven_removals_and_keep_recovery() {
        let path = std::env::temp_dir().join(format!(
            "sonarpad-sync-review-{}-{}.json",
            std::process::id(),
            rand::random::<u64>()
        ));
        let mut session = Session {
            path: path.clone(),
            meta: json!({}),
            token: String::new(),
        };
        for name in [
            "radio",
            "tv",
            "podcasts",
            "sonartube",
            "news_sources",
            "news_folders",
            "news_articles",
            "preferences",
        ] {
            let rows: Vec<Value> = (0..30)
                .map(|i| json!({"id":i.to_string(),"data":{"name":i.to_string()}}))
                .collect();
            let before = json!({name:rows});
            let after = json!({name:rows[..25]});
            assert_eq!(
                session.guard_deletions("outgoing", &before, &after),
                Ok(before.clone())
            );
            let review = session.meta["last_cancelled_deletions"].clone();
            let persisted: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(persisted["last_cancelled_deletions"], review);
            assert!(persisted.get("deletion_review").is_none());
            assert_eq!(
                persisted["recovery"].as_array().unwrap().last().unwrap()["state"],
                before
            );
            let removed = removed_items(&before, &after);
            let ids: Vec<Value> = list(&removed[name])
                .iter()
                .map(|row| row["id"].clone())
                .collect();
            session.meta["pending"] = json!({"explicit_deletions":{name:ids}});
            assert_eq!(
                session
                    .guard_deletions("outgoing", &before, &after)
                    .unwrap(),
                after
            );
            assert_eq!(
                session
                    .guard_deletions("incoming", &before, &after)
                    .unwrap(),
                before
            );
            assert_eq!(
                session
                    .guard_deletions("outgoing", &before, &json!({name:[]}))
                    .unwrap(),
                after
            );
            session.meta["response"] = json!({"authorized_deletions":{name:ids}});
            assert_eq!(
                session
                    .guard_deletions("incoming", &before, &after)
                    .unwrap(),
                after
            );
            assert_eq!(
                session
                    .guard_deletions("incoming", &before, &json!({name:[]}))
                    .unwrap(),
                after
            );
            session.meta.as_object_mut().unwrap().remove("pending");
            session.meta.as_object_mut().unwrap().remove("response");
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn recovered_outgoing_items_survive_retry_merge_and_unproven_server_loss() {
        let dir =
            std::env::temp_dir().join(format!("sonarpad-sync-restore-{}", rand::random::<u64>()));
        let before = json!({"sonartube":[
            {"id":"a","data":{"name":"A"}}, {"id":"b","data":{"name":"B"}}, {"id":"c","data":{"name":"C"}}
        ], "preferences":[{"id":"sonartube.search_type","data":{"value":"videos"}}]});
        let local = json!({"sonartube":[], "preferences":[{"id":"sonartube.search_type","data":{"value":"channels"}}]});
        let mut session = Session {
            path: dir.join("cloud_sync.json"),
            token: String::new(),
            meta: json!({"group_id":"g", "pending":{"baseline":before,"state":local}}),
        };
        let sent = session.prepare_pending_state().unwrap();
        assert_eq!(sent["sonartube"], before["sonartube"]);
        assert_eq!(sent["preferences"], local["preferences"]);
        session.meta = Session::metadata(&dir).unwrap();
        assert_eq!(session.prepare_pending_state().unwrap(), sent);
        assert_eq!(session.meta["pending_local_state"], local);
        let mut remote = sent.clone();
        remote["sonartube"][0]["data"]["name"] = json!("Server edit");
        let applied = session.merge_response(&remote, &local).unwrap();
        assert_eq!(applied, remote);
        remote["sonartube"] = json!([]);
        assert_eq!(session.merge_response(&remote, &local).unwrap(), sent);
        session.meta["response"] =
            json!({"sequence":2,"authorized_deletions":{"sonartube":["a","b","c"]}});
        assert_eq!(session.merge_response(&remote, &local).unwrap(), remote);
        session.finish(remote).unwrap();
        assert!(session.meta.get("pending_local_state").is_none());
        std::fs::remove_file(dir.join("cloud_sync.json")).unwrap();
        std::fs::remove_file(dir.join("cloud_deletion_events.json")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn concurrent_accidental_loss_is_restored_but_new_explicit_removal_survives() {
        let dir = std::env::temp_dir().join(format!(
            "sonarpad-sync-concurrent-{}",
            rand::random::<u64>()
        ));
        let before = json!({"sonartube":[{"id":"a","data":{}},{"id":"b","data":{}}]});
        let empty = json!({"sonartube":[]});
        let mut session = Session {
            path: dir.join("cloud_sync.json"),
            token: String::new(),
            meta: json!({"group_id":"g", "pending":{"baseline":before,"state":before}}),
        };
        session.prepare_pending_state().unwrap();
        assert_eq!(session.merge_response(&before, &empty).unwrap(), before);
        let keep_b = json!({"sonartube":[{"id":"b","data":{}}]});
        record_explicit_deletions(&dir, &before, &keep_b).unwrap();
        assert_eq!(session.merge_response(&before, &empty).unwrap(), keep_b);
        let additions = json!({"sonartube":[{"id":"new","data":{}}]});
        let merged = session.merge_response(&before, &additions).unwrap();
        let ids: Vec<Value> = list(&merged["sonartube"])
            .iter()
            .map(|row| row["id"].clone())
            .collect();
        assert_eq!(ids, vec![json!("new"), json!("b")]);
        std::fs::remove_file(dir.join("cloud_sync.json")).unwrap();
        std::fs::remove_file(dir.join("cloud_deletion_events.json")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn replacements_are_deletions_but_omitted_scope_and_additions_are_not() {
        let before = json!({"radio":[{"id":"a","data":{}}]});
        assert!(
            !removed_items(&before, &json!({"radio":[{"id":"b","data":{}}]}))["radio"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(removed_items(&before, &json!({"tv":[]})), json!({}));
        assert_eq!(removed_items(&json!({"radio":[]}), &before), json!({}));
    }
}
