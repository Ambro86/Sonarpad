//! Native InnerTube catalog adapter. Tokens are scoped to their provider;
//! callers replay PHP pagination when a native continuation fails.
use serde_json::{Value, json};
use std::collections::HashSet;
use std::time::{Duration, Instant};

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn text(v: &Value) -> String {
    if let Some(t) = v["simpleText"].as_str().or(v.as_str()) {
        return t.into();
    }
    v["runs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| s(&r["text"]))
        .collect()
}
fn walk<'a>(v: &'a Value, depth: usize, f: &mut impl FnMut(&'a Value) -> bool) {
    if depth > 40 || !f(v) {
        return;
    }
    match v {
        Value::Array(a) => {
            for x in a {
                walk(x, depth + 1, f);
            }
        }
        Value::Object(o) => {
            for (k, x) in o {
                if !matches!(
                    k.as_str(),
                    "menu" | "navigationEndpoint" | "trackingParams" | "header"
                ) {
                    walk(x, depth + 1, f);
                }
            }
        }
        _ => {}
    }
}
fn find<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    fn visit<'a>(v: &'a Value, keys: &[&str], depth: usize) -> Option<&'a Value> {
        if depth > 40 {
            return None;
        }
        for key in keys {
            if v[*key].is_object() {
                return Some(&v[*key]);
            }
        }
        match v {
            Value::Array(a) => a.iter().find_map(|x| visit(x, keys, depth + 1)),
            Value::Object(o) => o
                .iter()
                .filter(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "musicCarouselShelfRenderer"
                            | "menu"
                            | "header"
                            | "musicResponsiveHeaderRenderer"
                            | "navigationEndpoint"
                    )
                })
                .find_map(|(_, x)| visit(x, keys, depth + 1)),
            _ => None,
        }
    }
    visit(v, keys, 0)
}
fn continuation(v: &Value) -> Option<String> {
    let mut result = None;
    walk(v, 0, &mut |n| {
        let t =
            n["continuationItemRenderer"]["continuationEndpoint"]["continuationCommand"]["token"]
                .as_str()
                .or(n["nextContinuationData"]["continuation"].as_str());
        if result.is_none() {
            result = t.filter(|t| !t.is_empty()).map(str::to_owned);
        }
        result.is_none()
    });
    result
}
fn post(music: bool, path: &str, mut body: Value, locale: &str) -> Result<Value, String> {
    let (hl, gl) = crate::sonarmusic::locale_pair(locale);
    let version = if music {
        format!("1.{}.01.00", chrono::Utc::now().format("%Y%m%d"))
    } else {
        "2.20260722.01.00".into()
    };
    body["context"] = json!({"client":{"clientName":if music {"WEB_REMIX"} else {"WEB"},"clientVersion":version,"hl":hl,"gl":gl,"platform":"DESKTOP"}});
    let host = if music {
        "music.youtube.com"
    } else {
        "www.youtube.com"
    };
    let key = if music {
        "AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30"
    } else {
        "AIzaSyA8eiZmM1FaDVjRy-df2KTyQ_vfUWA8808"
    };
    static HTTP: std::sync::OnceLock<Result<reqwest::blocking::Client, String>> =
        std::sync::OnceLock::new();
    HTTP.get_or_init(|| reqwest::blocking::Client::builder().timeout(Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130.0.0.0 Safari/537.36")
        .build().map_err(|e|e.to_string())).as_ref().map_err(Clone::clone)?
        .post(format!("https://{host}/youtubei/v1/{path}?prettyPrint=false&key={key}"))
        .header("Origin",format!("https://{host}" )).header("Referer",format!("https://{host}/"))
        .header("X-Youtube-Client-Name",if music {"67"} else {"1"})
        .header("X-Youtube-Client-Version",version).header("Cookie","SOCS=CAI")
        .json(&body).send().map_err(|_|"Internal catalog connection failed".to_string())?
        .error_for_status().map_err(|_|"Internal catalog HTTP failure".to_string())?
        .json().map_err(|_|"Invalid internal catalog JSON".into())
}
pub fn youtube(
    args: &[(String, String)],
    token: Option<&str>,
    locale: &str,
) -> Result<Value, String> {
    let get = |k: &str| {
        args.iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
            .unwrap_or("")
    };
    let browse = get("browse");
    let mut body = if browse.is_empty() {
        json!({"query":get("q")})
    } else {
        json!({"browseId":if get("kind")=="playlist" {format!("VL{browse}")} else {browse.into()},"params":if get("kind")=="channel" {"EgZ2aWRlb3PyBgQKAjoA"} else {""}})
    };
    if let Some(t) = token {
        body = json!({"continuation":t});
    }
    let data = post(
        false,
        if browse.is_empty() {
            "search"
        } else {
            "browse"
        },
        body,
        locale,
    )?;
    let mut items = Vec::new();
    walk(&data, 0, &mut |n| {
        for (key, kind, idkey) in [
            ("videoRenderer", "video", "videoId"),
            ("playlistVideoRenderer", "video", "videoId"),
            ("channelRenderer", "channel", "channelId"),
            ("playlistRenderer", "playlist", "playlistId"),
            ("radioRenderer", "playlist", "playlistId"),
        ] {
            let r = &n[key];
            if !r.is_object() {
                continue;
            }
            items.push(json!({"kind":kind,"id":r[idkey],"title":text(&r["title"]),"views":text(&r["viewCountText"]),"duration":text(&r["lengthText"]),"channel":text(&r["ownerText"]),"subscribers":if kind == "channel" { youtube_channel_subscribers(r) } else { String::new() }}));
            return false;
        }
        let r = &n["lockupViewModel"];
        if r.is_object() {
            let kind = match s(&r["contentType"]) {
                "LOCKUP_CONTENT_TYPE_PLAYLIST" => "playlist",
                "LOCKUP_CONTENT_TYPE_VIDEO" => "video",
                _ => return false,
            };
            let meta = &r["metadata"]["lockupMetadataViewModel"];
            let views = lockup_views(meta, !browse.is_empty());
            if kind == "video" && views.trim().is_empty() {
                return false;
            }
            items.push(json!({"kind":kind,"id":r["contentId"],"title":meta["title"]["content"],"views":views}));
            return false;
        }
        true
    });
    checked(items, continuation(&data))
}
// Same channelRenderer distinction as Mobile: the modern search layout puts
// the handle in subscriberCountText and the subscribers in videoCountText.
fn youtube_channel_subscribers(renderer: &Value) -> String {
    let subscribers = text(&renderer["subscriberCountText"]);
    if subscribers.trim_start().starts_with('@') {
        text(&renderer["videoCountText"])
    } else {
        subscribers
    }
}

// Match Mobile's metadata layout. Never mistake a publication age, duration
// or channel name containing digits for the video's view count.
fn lockup_views(metadata: &serde_json::Value, collection: bool) -> &str {
    let Some(rows) = metadata["metadata"]["contentMetadataViewModel"]["metadataRows"].as_array()
    else {
        return "";
    };
    if rows.iter().any(|row| {
        row["badges"].as_array().is_some_and(|badges| {
            badges
                .iter()
                .any(|badge| badge["badgeViewModel"]["badgeStyle"] == "BADGE_MEMBERS_ONLY")
        })
    }) {
        return "";
    }
    // Hype and other badge-only rows do not describe the channel or views.
    let rows: Vec<_> = rows
        .iter()
        .filter(|row| {
            row["metadataParts"]
                .as_array()
                .is_some_and(|parts| !parts.is_empty())
        })
        .collect();
    let single_row = rows.len() == 1
        && (collection || metadata["image"]["decoratedAvatarViewModel"].is_object());
    rows.get(if single_row { 0 } else { 1 })
        .and_then(|row| row["metadataParts"][0]["text"]["content"].as_str())
        .unwrap_or_default()
}
fn checked(items: Vec<Value>, next: Option<String>) -> Result<Value, String> {
    if items.is_empty() {
        return Err("Internal catalog has no usable results".into());
    }
    Ok(json!({"ok":true,"items":items,"next_token":next}))
}
fn browse_kind(b: &Value) -> &'static str {
    match s(
        &b["browseEndpointContextSupportedConfigs"]["browseEndpointContextMusicConfig"]["pageType"],
    ) {
        "MUSIC_PAGE_TYPE_ALBUM" => return "album",
        "MUSIC_PAGE_TYPE_ARTIST" | "MUSIC_PAGE_TYPE_USER_CHANNEL" => return "artist",
        "MUSIC_PAGE_TYPE_PLAYLIST" => return "playlist",
        "MUSIC_PAGE_TYPE_PODCAST_SHOW_DETAIL_PAGE" => return "podcast",
        "MUSIC_PAGE_TYPE_ARTIST_DISCOGRAPHY" => return "collection",
        _ => {}
    }
    let id = s(&b["browseId"]);
    for (prefix, kind) in [
        ("UC", "artist"),
        ("MPRE", "album"),
        ("MPAD", "collection"),
        ("MPSP", "podcast"),
        ("VL", "playlist"),
        ("PL", "playlist"),
        ("RD", "playlist"),
    ] {
        if id.starts_with(prefix) {
            return kind;
        }
    }
    ""
}
fn row(r: &Value, card: bool) -> Option<Value> {
    let title_node = if card {
        &r["title"]
    } else {
        &r["flexColumns"][0]["musicResponsiveListItemFlexColumnRenderer"]["text"]
    };
    let title = text(title_node);
    if title.is_empty() {
        return None;
    }
    let nav = [
        &r["navigationEndpoint"],
        &title_node["runs"][0]["navigationEndpoint"],
        &r["onTap"],
    ]
    .into_iter()
    .find(|n| n["browseEndpoint"].is_object() || n["watchEndpoint"].is_object())
    .unwrap_or(&Value::Null);
    let browse = &nav["browseEndpoint"];
    let play = &r["overlay"]["musicItemThumbnailOverlayRenderer"]["content"]["musicPlayButtonRenderer"]
        ["playNavigationEndpoint"]["watchEndpoint"];
    let watch = if nav["watchEndpoint"].is_object() {
        &nav["watchEndpoint"]
    } else {
        play
    };
    let video = r["playlistItemData"]["videoId"]
        .as_str()
        .unwrap_or(s(&watch["videoId"]));
    let vt = s(
        &watch["watchEndpointMusicSupportedConfigs"]["watchEndpointMusicConfig"]["musicVideoType"],
    );
    let collection = browse_kind(browse);
    let kind = if !collection.is_empty() && !nav["watchEndpoint"].is_object() {
        collection
    } else if vt.contains("PODCAST") {
        "episode"
    } else if vt.contains("OMV") || vt.contains("UGC") {
        "video"
    } else if !video.is_empty() {
        "song"
    } else {
        return None;
    };
    let is_collection = !matches!(kind, "song" | "video" | "episode");
    let id = if is_collection {
        s(&browse["browseId"])
    } else {
        video
    };
    if id.is_empty() {
        return None;
    }
    let mut artist = String::new();
    let mut duration = String::new();
    let metadata = if card {
        r["subtitle"].clone()
    } else {
        json!([
            r["flexColumns"]
                .as_array()
                .map(|a| a.iter().skip(1).cloned().collect::<Vec<_>>()),
            r["fixedColumns"]
        ])
    };
    // Metadata links identify artists, rather than guessing from translated labels.
    fn metadata_walk(v: &Value, artist: &mut String, duration: &mut String, depth: usize) {
        if depth > 20 {
            return;
        }
        if s(&v["navigationEndpoint"]["browseEndpoint"]["browseId"]).starts_with("UC")
            && artist.is_empty()
        {
            *artist = text(&v["text"]);
        }
        if let Some(t) = v.as_str() {
            for part in t.split(['•', '·']) {
                let p = part.trim();
                if p.contains(':') && p.chars().all(|c| c.is_ascii_digit() || c == ':') {
                    *duration = p.into();
                }
            }
        }
        match v {
            Value::Array(a) => {
                for x in a {
                    metadata_walk(x, artist, duration, depth + 1)
                }
            }
            Value::Object(o) => {
                for x in o.values() {
                    metadata_walk(x, artist, duration, depth + 1)
                }
            }
            _ => {}
        }
    }
    metadata_walk(&metadata, &mut artist, &mut duration, 0);
    let subtitle = if card {
        text(&r["subtitle"])
    } else {
        ["flexColumns", "fixedColumns"]
            .iter()
            .flat_map(|key| {
                r[*key]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .skip(if *key == "flexColumns" { 1 } else { 0 })
            })
            .map(|column| {
                text(&column["musicResponsiveListItemFlexColumnRenderer"]["text"]).to_owned()
                    + &text(&column["musicResponsiveListItemFixedColumnRenderer"]["text"])
            })
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" • ")
    };
    if kind == "artist" {
        artist = title.clone();
    }
    Some(
        json!({"kind":kind,"id":id,"title":title,"video_id":if is_collection{""}else{video},"browse_id":browse["browseId"],"browse_params":browse["params"],"artist":artist,"duration":duration,"subtitle":subtitle,"available":r["musicItemRendererDisplayPolicy"]!="MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT"}),
    )
}
fn music_items(data: &Value) -> Vec<Value> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    walk(data, 0, &mut |n| {
        let h = &n["musicCarouselShelfRenderer"]["header"]["musicCarouselShelfBasicHeaderRenderer"];
        let b = &h["moreContentButton"]["buttonRenderer"]["navigationEndpoint"]["browseEndpoint"];
        if s(&b["browseId"]).starts_with("MPAD") {
            let item = json!({"kind":"collection","id":b["browseId"],"browse_id":b["browseId"],"browse_params":b["params"],"title":text(&h["title"])});
            if seen.insert(format!(
                "{}:{}:{}",
                item["kind"], item["id"], item["browse_params"]
            )) {
                items.push(item);
            }
        }
        for (k, card) in [
            ("musicResponsiveListItemRenderer", false),
            ("musicTwoRowItemRenderer", true),
            ("musicCardShelfRenderer", true),
        ] {
            if n[k].is_object() {
                if let Some(item) = row(&n[k], card) {
                    let key = format!("{}:{}:{}", item["kind"], item["id"], item["browse_params"]);
                    if seen.insert(key) {
                        items.push(item);
                    }
                }
                return false;
            }
        }
        true
    });
    items
}
fn filter(kind: &str) -> &'static str {
    match kind {
        "song" => "EgWKAQIIAWoMEA4QChADEAQQCRAF",
        "video" => "EgWKAQIQAWoMEA4QChADEAQQCRAF",
        "album" => "EgWKAQIYAWoMEA4QChADEAQQCRAF",
        "artist" => "EgWKAQIgAWoMEA4QChADEAQQCRAF",
        "playlist" => "Eg-KAQwIABAAGAAgACgBMABqChAEEAMQCRAFEAo=",
        _ => "",
    }
}
fn music_raw(
    request: &crate::sonarmusic::Request,
    token: Option<&str>,
    locale: &str,
) -> Result<Value, String> {
    use crate::sonarmusic::Request;
    let (path, body) = match request {
        Request::Search { query, kind } => ("search", json!({"query":query,"params":filter(kind)})),
        Request::Browse { id, params } => ("browse", json!({"browseId":id,"params":params})),
    };
    post(
        true,
        path,
        token.map(|t| json!({"continuation":t})).unwrap_or(body),
        locale,
    )
}
fn normalized(s: &str) -> String {
    s.to_lowercase()
        .replace(['’', '‘'], "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn album_state(query: &str, locale: &str) -> Result<Option<Value>, String> {
    use crate::sonarmusic::Request;
    let data = music_raw(&Request::search(query, "artist")?, None, locale)?;
    let mut artists: Vec<_> = music_items(&data)
        .into_iter()
        .filter(|r| r["kind"] == "artist" && normalized(s(&r["title"])) == normalized(query))
        .collect();
    if artists.is_empty() {
        return Ok(None);
    }
    if artists.len() > 1 {
        let general = music_items(&music_raw(&Request::search(query, "all")?, None, locale)?);
        let Some(top) = general.first() else {
            return Ok(None);
        };
        artists.retain(|a| top["kind"] == "artist" && a["id"] == top["id"]);
        if artists.len() != 1 {
            return Ok(None);
        }
    }
    let artist = &artists[0];
    let data = music_raw(
        &Request::Browse {
            id: s(&artist["id"]).into(),
            params: String::new(),
        },
        None,
        locale,
    )?;
    let mut state = None;
    walk(&data, 0, &mut |n| {
        let h = &n["musicCarouselShelfRenderer"]["header"]["musicCarouselShelfBasicHeaderRenderer"];
        let title = text(&h["title"]).to_lowercase();
        if [
            "album",
            "albums",
            "álbumes",
            "álbuns",
            "alben",
            "alba",
            "albumy",
            "albume",
            "albümler",
            "альбоми",
            "альбомы",
            "албуми",
            "albumai",
            "专辑",
            "專輯",
            "アルバム",
            "앨범",
            "एल्बम",
        ]
        .contains(&title.as_str())
        {
            let b =
                &h["moreContentButton"]["buttonRenderer"]["navigationEndpoint"]["browseEndpoint"];
            if s(&b["browseId"]).starts_with("MPAD") {
                state = Some(
                    json!({"id":b["browseId"],"params":s(&b["params"]),"artist":artist["title"],"offset":0,"source":null}),
                );
                return false;
            }
        }
        true
    });
    Ok(state)
}
pub fn music(
    request: &crate::sonarmusic::Request,
    token: Option<&str>,
    locale: &str,
) -> Result<Value, String> {
    let mut state = if let Some(t) = token.and_then(|t| t.strip_prefix("albums:")) {
        Some(serde_json::from_str::<Value>(t).map_err(|_| "Invalid album cursor")?)
    } else {
        None
    };
    if token.is_none()
        && let crate::sonarmusic::Request::Search { query, kind } = request
        && kind == "album"
    {
        state = album_state(query, locale)?;
    }
    if let Some(mut state) = state {
        let browse = crate::sonarmusic::Request::Browse {
            id: s(&state["id"]).into(),
            params: s(&state["params"]).into(),
        };
        let data = music_raw(&browse, state["source"].as_str(), locale)?;
        let albums: Vec<_> = music_items(&data)
            .into_iter()
            .filter(|i| i["kind"] == "album")
            .map(|mut i| {
                if s(&i["artist"]).is_empty() {
                    i["artist"] = state["artist"].clone();
                }
                i
            })
            .collect();
        let offset = state["offset"].as_u64().unwrap_or(0) as usize;
        let items = albums.iter().skip(offset).take(20).cloned().collect();
        let next = if offset + 20 < albums.len() {
            state["offset"] = json!(offset + 20);
            Some(format!("albums:{state}"))
        } else if let Some(t) =
            continuation(&data).filter(|t| Some(t.as_str()) != state["source"].as_str())
        {
            state["offset"] = json!(0);
            state["source"] = json!(t);
            Some(format!("albums:{state}"))
        } else {
            None
        };
        return checked(items, next);
    }
    let data = music_raw(request, token, locale)?;
    let source = if request.collection() {
        find(
            &data,
            &[
                "musicPlaylistShelfRenderer",
                "musicShelfRenderer",
                "musicPlaylistShelfContinuation",
                "musicShelfContinuation",
                "appendContinuationItemsAction",
                "reloadContinuationItemsCommand",
            ],
        )
        .ok_or("Missing collection tracks")?
    } else {
        &data
    };
    checked(music_items(source), continuation(source))
}

/// yt-dlp is the last catalog provider, never a dependency of native search.
pub fn music_ytdlp(request: &crate::sonarmusic::Request, page: usize) -> Result<Value, String> {
    use std::process::{Command, Stdio};
    let url = match request {
        crate::sonarmusic::Request::Search { query, kind } => {
            let mut u = reqwest::Url::parse("https://music.youtube.com/search")
                .map_err(|e| e.to_string())?;
            u.query_pairs_mut()
                .append_pair("q", query)
                .append_pair("sp", filter(kind));
            u.to_string()
        }
        crate::sonarmusic::Request::Browse { id, .. } => {
            if let Some(id) = id.strip_prefix("VL") {
                format!("https://music.youtube.com/playlist?list={id}")
            } else {
                format!("https://music.youtube.com/browse/{id}")
            }
        }
    };
    let local = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .map(|p| p.join("Sonarpad/yt-dlp.exe"));
    let exe = local
        .filter(|p| p.is_file())
        .unwrap_or_else(|| "yt-dlp".into());
    let mut command = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .args([
            "--flat-playlist",
            "--dump-single-json",
            "--skip-download",
            "--no-warnings",
            "--encoding",
            "utf-8",
            "--socket-timeout",
            "15",
            "--playlist-start",
            &(page.max(1) * 20 - 19).to_string(),
            "--playlist-end",
            &(page.max(1) * 20 + 1).to_string(),
            "--",
            &url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(|_| "yt-dlp unavailable")?;
    let stdout = child.stdout.take().ok_or("yt-dlp output unavailable")?;
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut bytes = Vec::new();
        stdout
            .take(16 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    match reader.join() {
                        Ok(Ok(_)) => {}
                        Ok(Err(error)) => {
                            crate::log_debug(&format!("Catalog reader cleanup failed: {error}"))
                        }
                        Err(_) => crate::log_debug("Catalog reader thread panicked during cleanup"),
                    }
                    return Err("yt-dlp catalog failed".into());
                }
                break;
            }
            Ok(None) if start.elapsed() < Duration::from_secs(60) => {
                std::thread::sleep(Duration::from_millis(50))
            }
            _ => {
                if let Err(error) = child.kill() {
                    crate::log_debug(&format!("Catalog process termination failed: {error}"));
                }
                if let Err(error) = child.wait() {
                    crate::log_debug(&format!("Catalog process wait failed: {error}"));
                }
                match reader.join() {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        crate::log_debug(&format!("Catalog reader cleanup failed: {error}"))
                    }
                    Err(_) => crate::log_debug("Catalog reader thread panicked during cleanup"),
                }
                return Err("yt-dlp catalog timed out".into());
            }
        }
    }
    let bytes = reader
        .join()
        .map_err(|_| "yt-dlp reader failed")?
        .map_err(|_| "yt-dlp output failed")?;
    let data: Value = serde_json::from_slice(&bytes).map_err(|_| "yt-dlp invalid JSON")?;
    let entries = data["entries"].as_array().ok_or("yt-dlp missing entries")?;
    let kind = match request {
        crate::sonarmusic::Request::Search { kind, .. } => kind.as_str(),
        _ => "song",
    };
    let mut items = Vec::new();
    for e in entries.iter().take(20) {
        let id = s(&e["id"]);
        let playable = id.len() == 11;
        let url = s(&e["url"]);
        let browse = if playable {
            String::new()
        } else if let Ok(u) = reqwest::Url::parse(url) {
            u.path()
                .strip_prefix("/browse/")
                .or_else(|| u.path().strip_prefix("/channel/"))
                .map(str::to_owned)
                .or_else(|| {
                    u.query_pairs()
                        .find(|(k, _)| k == "list")
                        .map(|(_, v)| format!("VL{v}"))
                })
                .unwrap_or_else(|| id.into())
        } else {
            id.into()
        };
        let actual = if playable {
            if kind == "video" { "video" } else { "song" }
        } else if matches!(kind, "album" | "artist" | "playlist") {
            kind
        } else if browse.starts_with("UC") {
            "artist"
        } else if browse.starts_with("MPRE") {
            "album"
        } else {
            "playlist"
        };
        items.push(json!({"kind":actual,"id":if playable{id}else{&browse},"title":e["title"],"video_id":if playable{id}else{""},"browse_id":browse,"artist":e["artist"].as_str().or(e["uploader"].as_str()).unwrap_or(""),"duration":e["duration_string"]}));
    }
    checked(
        items,
        (entries.len() > 20).then(|| format!("ytdlp:{}", page + 1)),
    )
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn lockup_without_views_does_not_use_age_or_channel_digits() {
        let m = json!({"metadata":{"contentMetadataViewModel":{"metadataRows":[
            {"metadataParts":[{"text":{"content":"Channel 123"}}]},
            {"metadataParts":[{}, {"text":{"content":"2 days ago"}}]}
        ]}}});
        assert!(lockup_views(&m, false).is_empty());
    }
    #[test]
    fn lockup_reads_views_instead_of_publication_age() {
        let m = json!({"metadata":{"contentMetadataViewModel":{"metadataRows":[
            {"metadataParts":[{"text":{"content":"Channel"}}]},
            {"metadataParts":[{"text":{"content":"199 visualizzazioni"}},{"text":{"content":"55 minuti fa"}}]}
        ]}}});
        assert_eq!(lockup_views(&m, false), "199 visualizzazioni");
    }
    #[test]
    fn collection_shelf_excludes_recommendations() {
        let v = json!({"a":{"musicCarouselShelfRenderer":{"musicShelfRenderer":{"wrong":true}}},"b":{"musicPlaylistShelfRenderer":{"tracks":true}}});
        assert_eq!(
            find(&v, &["musicShelfRenderer", "musicPlaylistShelfRenderer"]).unwrap()["tracks"],
            true
        );
    }
    #[test]
    fn collection_overlay_is_not_a_song() {
        let r = json!({"title":{"runs":[{"text":"Album"}]},"navigationEndpoint":{"browseEndpoint":{"browseId":"MPREabc"}},"overlay":{"musicItemThumbnailOverlayRenderer":{"content":{"musicPlayButtonRenderer":{"playNavigationEndpoint":{"watchEndpoint":{"videoId":"4K6cvc27_CE"}}}}}}});
        let i = row(&r, true).unwrap();
        assert_eq!(i["kind"], "album");
        assert_eq!(i["video_id"], "");
    }
    #[test]
    fn ignores_sorting_continuations() {
        let v = json!({"header":{"continuationCommand":{"token":"sort"}},"contents":[{"continuationItemRenderer":{"continuationEndpoint":{"continuationCommand":{"token":"next"}}}}]});
        assert_eq!(continuation(&v).as_deref(), Some("next"));
    }
    #[test]
    fn parses_artist_and_duration() {
        let r = json!({"title":{"runs":[{"text":"Song"}]},"subtitle":{"runs":[{"text":"Artist","navigationEndpoint":{"browseEndpoint":{"browseId":"UCabc"}}},{"text":" • 5:34"}]},"navigationEndpoint":{"watchEndpoint":{"videoId":"4K6cvc27_CE"}}});
        let i = row(&r, true).unwrap();
        assert_eq!(i["artist"], "Artist");
        assert_eq!(i["duration"], "5:34");
        assert_eq!(i["subtitle"], "Artist • 5:34");
    }
    #[test]
    fn unavailable_item_stays_unavailable() {
        let r = json!({"title":{"simpleText":"Song"},"navigationEndpoint":{"watchEndpoint":{"videoId":"4K6cvc27_CE"}},"musicItemRendererDisplayPolicy":"MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT"});
        assert_eq!(row(&r, true).unwrap()["available"], false);
    }
    #[test]
    fn deduplicates_music_renderers() {
        let r = json!({"musicTwoRowItemRenderer":{"title":{"simpleText":"Album"},"navigationEndpoint":{"browseEndpoint":{"browseId":"MPREabc"}}}});
        assert_eq!(music_items(&json!([r, r])).len(), 1);
    }
    #[test]
    fn empty_internal_result_triggers_fallback() {
        assert!(checked(vec![], None).is_err());
    }
    #[test]
    fn artist_normalization_handles_apostrophes() {
        assert_eq!(normalized(" Gigi D’Alessio "), normalized("gigi d'alessio"));
    }
    #[test]
    #[ignore = "Live internal catalogs and yt-dlp"]
    fn live_internal_catalogs() {
        for query in [
            "Pink Floyd",
            "Grateful Dead",
            "Gigi D'Alessio",
            "Raf",
            "Cristina D'Avena",
        ] {
            let args = vec![("q".into(), query.into())];
            let first = youtube(&args, None, "it").unwrap();
            assert!(!first["items"].as_array().unwrap().is_empty());
            println!(
                "YouTube {query}: {}",
                first["items"].as_array().unwrap().len()
            );
            if let Some(t) = first["next_token"].as_str() {
                assert!(youtube(&args, Some(t), "it").is_ok());
            }
            for kind in ["song", "album"] {
                let request = crate::sonarmusic::Request::search(query, kind).unwrap();
                let first = music(&request, None, "it").unwrap();
                println!(
                    "Music {query}/{kind}: {}, next={}",
                    first["items"].as_array().unwrap().len(),
                    first["next_token"].is_string()
                );
                if kind == "album" {
                    assert!(
                        first["items"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .all(|i| i["kind"] == "album")
                    );
                }
                if let Some(t) = first["next_token"].as_str() {
                    assert!(music(&request, Some(t), "it").is_ok());
                }
                if let Some(item) = first["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|i| i["kind"] == "album")
                {
                    let request = crate::sonarmusic::Request::Browse {
                        id: s(&item["browse_id"]).into(),
                        params: s(&item["browse_params"]).into(),
                    };
                    let tracks = music(&request, None, "it").unwrap();
                    assert!(
                        tracks["items"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .all(|i| matches!(s(&i["kind"]), "song" | "video" | "episode"))
                    );
                }
            }
        }
        let r = crate::sonarmusic::Request::search("Pink Floyd", "album").unwrap();
        let d = music_ytdlp(&r, 1).unwrap();
        println!("yt-dlp albums: {}", d["items"].as_array().unwrap().len());
        assert!(
            d["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|i| i["kind"] == "album")
        );
    }
}

#[cfg(test)]
mod channel_badge_tests {
    use super::*;
    #[test]
    #[ignore = "Live YouTube channel request"]
    fn live_reported_channel() {
        let data = youtube(
            &[
                ("browse".into(), "UCnfd-Yur_dJD9vLvSA9h8rw".into()),
                ("kind".into(), "channel".into()),
            ],
            None,
            "it",
        )
        .unwrap();
        let items = data["items"].as_array().unwrap();
        assert_eq!(items[0]["id"], "1o9lnx4-VrQ");
        assert!(!items.iter().any(|item| item["id"] == "YWSvIPz82dY"));
    }
    #[test]
    fn public_hype_members_only_and_collaboration() {
        let data: Value =
            serde_json::from_str(include_str!("test_fixtures/sonartube_channel_hype.json"))
                .unwrap();
        let rows = data["contents"].as_array().unwrap();
        let views: Vec<_> = rows
            .iter()
            .map(|row| {
                lockup_views(
                    &row["lockupViewModel"]["metadata"]["lockupMetadataViewModel"],
                    true,
                )
            })
            .collect();
        assert_eq!(
            views,
            vec![
                "25K visualizzazioni",
                "",
                "6,7K visualizzazioni",
                "32K visualizzazioni"
            ]
        );
    }
}

#[cfg(test)]
mod subscriber_tests {
    use super::*;
    #[test]
    fn youtube_modern_search_reads_subscribers_instead_of_handle() {
        assert_eq!(
            youtube_channel_subscribers(&json!({
                "subscriberCountText":{"simpleText":" @PinkFloyd"},
                "videoCountText":{"runs":[{"text":"4,12 M di iscritti"}]}
            })),
            "4,12 M di iscritti"
        );
    }
    #[test]
    fn youtube_legacy_layout_never_confuses_videos_with_subscribers() {
        assert_eq!(
            youtube_channel_subscribers(&json!({
                "subscriberCountText":{"simpleText":"10 mila iscritti"},
                "videoCountText":{"simpleText":"200 video"}
            })),
            "10 mila iscritti"
        );
        assert_eq!(
            youtube_channel_subscribers(&json!({"videoCountText":{"simpleText":"200 video"}})),
            ""
        );
        assert_eq!(
            youtube_channel_subscribers(&json!({"subscriberCountText":{"simpleText":"@example"}})),
            ""
        );
    }
    #[test]
    fn sonarmusic_artist_subscribers_survive_both_native_layouts_and_favorites() {
        let endpoint = json!({"browseEndpoint":{"browseId":"UCexample"}});
        let card = json!({"title":{"simpleText":"Artist"},"subtitle":{"runs":[{"text":"Artista • 15 mila iscritti"}]},"navigationEndpoint":endpoint});
        let list = json!({"flexColumns":[
            {"musicResponsiveListItemFlexColumnRenderer":{"text":{"simpleText":"Artist"}}},
            {"musicResponsiveListItemFlexColumnRenderer":{"text":{"simpleText":"Artista • 15 mila iscritti"}}}
        ],"navigationEndpoint":endpoint});
        for (source, is_card) in [(card, true), (list, false)] {
            let value = row(&source, is_card).unwrap();
            let item = crate::sonarmusic::Item::parse(&value).unwrap();
            let label = item.display("it");
            assert_eq!(label.matches("15 mila iscritti").count(), 1);
            assert_eq!(
                crate::sonarmusic::Item::parse(&crate::sonarmusic::favorite_value(&item))
                    .unwrap()
                    .display("it"),
                label
            );
        }
    }
}
