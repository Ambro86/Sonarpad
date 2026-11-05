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
    let message = if busy {
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
pub fn export(s: &Value, mac: bool) -> Value {
    let lang = language(s, mac);
    let mut result = json!({
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
    pub fn sync(&mut self, settings: &Value, mac: bool, log: fn(&str)) -> Result<Value, String> {
        if !self.connected() {
            return Err("unauthorized".into());
        }
        if self.meta.get("pending").is_none() {
            self.meta["pending"] = json!({"schema_version":1,"sequence":self.meta["sequence"].as_u64().unwrap_or(0)+1,
                "baseline":self.select(&project(&self.meta["baseline"],settings,mac)?),"state":self.select(&export(settings,mac))});
            self.save()?;
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
        if let Some(m) = self.meta.as_object_mut() {
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
