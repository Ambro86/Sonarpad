//! Explicit portable preferences and validated Sonarpad activation only.
//! Other credentials and local paths remain excluded.
use serde_json::{Value, json};
const SEARCH: &[&str] = &[
    "all",
    "videos",
    "playlists",
    "channels",
    "courses",
    "podcasts",
];
const MUSIC: &[&str] = &["all", "song", "album", "artist", "playlist", "video"];
const NEWS: &[(&str, &str)] = &[
    ("italian", "it"),
    ("english", "en"),
    ("french", "fr"),
    ("spanish", "es"),
    ("portuguese", "pt"),
    ("portugueseBrazil", "pt-br"),
    ("polish", "pl"),
    ("czech", "cs"),
    ("german", "de"),
];
fn news_code(name: &str, mac: bool) -> Option<&'static str> {
    NEWS.iter()
        .find(|(n, c)| *n == name && (!mac || !["en", "pt-br"].contains(c)))
        .map(|(_, code)| *code)
}
fn field(id: &str, mac: bool) -> Option<&'static str> {
    Some(match id {
        "activation.code" => "rai_luce_code",
        "sonartube.search_type" => "sonartube_search_type",
        "sonarmusic.search_type" => "sonarmusic_search_type",
        "cutter.mode" => "media_cutter_advanced_mode",
        "cutter.seek_ms" => "media_cutter_seek_step_ms",
        "radio.language" => "radio_search_language",
        "radio.country" => "radio_search_country",
        "radio.genre" if !mac => "radio_search_genre",
        "converter.format" => "converter_format",
        "converter.bitrate" => "converter_bitrate",
        "converter.wav_depth" => "converter_wav_depth",
        "converter.ogg_quality" => "converter_ogg_quality",
        "converter.flac_compression" => "converter_flac_compression",
        "tv.audio_description" if mac => "prefer_raiplay_audio_description",
        "reader.sleep_minutes" if !mac => "sleep_timer_minutes",
        "playback.seek_seconds" if !mac => "audiobook_skip_seconds",
        "weather.city" => "weather_city_query",
        "weather.unit" => "weather_temperature_unit",
        "podcast.country" => "podcast_directory_country",
        "news.language" => {
            if mac {
                "news_language"
            } else {
                "rss_news_language"
            }
        }
        "news.local_city" if !mac => "rss_local_city",
        "reader.auto_bookmark" => {
            if mac {
                "auto_media_bookmark"
            } else {
                "automatic_bookmark"
            }
        }
        "playback.audiobook_speed" if !mac => "audiobook_playback_speed",
        "mac.playback.seek_seconds" if mac => "media_seek_seconds",
        _ => return None,
    })
}
fn valid(id: &str, v: &Value) -> bool {
    match id {
        "activation.code" => v
            .as_str()
            .is_some_and(crate::tools::tv::is_activation_code_valid),
        "sonartube.search_type" => v.as_str().is_some_and(|s| SEARCH.contains(&s)),
        "sonarmusic.search_type" => v.as_str().is_some_and(|s| MUSIC.contains(&s)),
        "cutter.mode" => v
            .as_str()
            .is_some_and(|s| ["guided", "advanced"].contains(&s)),
        "weather.unit" => v
            .as_str()
            .is_some_and(|s| ["celsius", "fahrenheit"].contains(&s)),
        "reader.auto_bookmark" | "tv.audio_description" => v.is_boolean(),
        "cutter.seek_ms" => v.as_u64().is_some_and(|n| {
            [1000, 5000, 10000, 30000, 60000, 120000, 300000, 600000].contains(&n)
        }),
        "reader.sleep_minutes" => v.as_u64().is_some_and(|n| n <= 120),
        "playback.seek_seconds" => v.as_u64().is_some_and(|n| (10..=300).contains(&n)),
        "converter.format" => v.as_str().is_some_and(|s| {
            [
                "mp3", "aac", "m4b", "mp4", "avi", "mov", "opus", "ogg", "flac", "wav", "aiff",
            ]
            .contains(&s)
        }),
        "converter.wav_depth" => v
            .as_str()
            .is_some_and(|s| ["pcm16", "pcm24", "pcm32", "float32"].contains(&s)),
        "converter.bitrate" => v.as_u64().is_some_and(|n| (64..=320).contains(&n)),
        "converter.ogg_quality" => v.as_u64().is_some_and(|n| n <= 10),
        "converter.flac_compression" => v.as_u64().is_some_and(|n| n <= 12),
        "playback.audiobook_speed" => v.as_f64().is_some_and(|n| (0.5..=2.0).contains(&n)),
        "mac.playback.seek_seconds" => v
            .as_u64()
            .is_some_and(|n| [5, 10, 20, 30, 40, 50, 60, 90, 120, 150, 180].contains(&n)),
        _ => v.as_str().is_some_and(|s| s.chars().count() <= 256),
    }
}
pub fn export(s: &Value, mac: bool) -> Value {
    let mut result = Vec::new();
    for id in [
        "activation.code",
        "sonartube.search_type",
        "sonarmusic.search_type",
        "cutter.mode",
        "cutter.seek_ms",
        "radio.language",
        "radio.country",
        "radio.genre",
        "converter.format",
        "converter.bitrate",
        "converter.wav_depth",
        "converter.ogg_quality",
        "converter.flac_compression",
        "tv.audio_description",
        "reader.sleep_minutes",
        "playback.seek_seconds",
        "weather.city",
        "weather.unit",
        "podcast.country",
        "news.language",
        "news.local_city",
        "reader.auto_bookmark",
        "playback.audiobook_speed",
        "mac.playback.seek_seconds",
    ] {
        let Some(key) = field(id, mac) else { continue };
        let Some(raw) = s.get(key) else { continue };
        let v = match id {
            "sonartube.search_type" => json!(
                SEARCH
                    .get(raw.as_u64().unwrap_or(0) as usize)
                    .unwrap_or(&"all")
            ),
            "sonarmusic.search_type" if raw.as_str() == Some("") => json!("all"),
            "weather.unit" if raw.as_str() == Some("") => json!("celsius"),
            "news.language" => json!(
                NEWS.iter()
                    .find(|(_, code)| raw.as_str() == Some(*code))
                    .map(|(name, _)| *name)
                    .unwrap_or("")
            ),
            "cutter.mode" => json!(if raw.as_bool().unwrap_or(false) {
                "advanced"
            } else {
                "guided"
            }),
            "weather.city" if !mac && s["weather_city"]["name"].is_string() => {
                s["weather_city"]["name"].clone()
            }
            _ => raw.clone(),
        };
        if valid(id, &v) && v.as_str() != Some("") {
            result.push(json!({"id":id,"data":{"value":v}}));
        }
    }
    json!(result)
}
pub fn project(rows: &Value, mac: bool) -> Result<Value, String> {
    if rows.is_null() {
        return Ok(json!([]));
    }
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for row in rows.as_array().ok_or("invalid_preference")? {
        let id = row["id"].as_str().ok_or("invalid_preference")?;
        if field(id, mac).is_none() {
            continue;
        }
        if id == "news.language"
            && row["data"]["value"]
                .as_str()
                .is_some_and(|v| news_code(v, mac).is_none())
        {
            continue;
        }
        // Invalid activation must never break favorites synchronization.
        if id == "activation.code" && !valid(id, &row["data"]["value"]) {
            continue;
        }
        if !valid(id, &row["data"]["value"]) || !seen.insert(id) {
            return Err("invalid_preference".into());
        }
        out.push(json!({"id":id,"data":{"value":row["data"]["value"]}}));
    }
    Ok(json!(out))
}
pub fn apply(s: &mut Value, rows: &Value, mac: bool) -> Result<(), String> {
    for row in project(rows, mac)?.as_array().ok_or("invalid_preference")? {
        let id = row["id"].as_str().ok_or("invalid_preference")?;
        let key = field(id, mac).ok_or("invalid_preference")?;
        let v = &row["data"]["value"];
        if id == "activation.code" && valid(id, &s[key]) {
            continue;
        }
        if id == "weather.city"
            && !mac
            && !s["weather_city"].is_null()
            && s["weather_city"]["name"] != *v
        {
            s["weather_city"] = Value::Null;
        }
        s[key] = match id {
            "sonartube.search_type" => json!(
                SEARCH
                    .iter()
                    .position(|k| Some(*k) == v.as_str())
                    .ok_or("invalid_preference")?
            ),
            "cutter.mode" => json!(v.as_str() == Some("advanced")),
            "news.language" => json!(
                news_code(v.as_str().ok_or("invalid_preference")?, mac)
                    .ok_or("invalid_preference")?
            ),
            _ => v.clone(),
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mobile_desktop_module_preferences_round_trip() {
        let rows = json!([
            {"id":"cutter.seek_ms","data":{"value":600000}},
            {"id":"radio.language","data":{"value":"fr"}},
            {"id":"radio.country","data":{"value":"fr"}},
            {"id":"converter.format","data":{"value":"wav"}},
            {"id":"converter.bitrate","data":{"value":256}},
            {"id":"converter.wav_depth","data":{"value":"pcm24"}},
            {"id":"converter.ogg_quality","data":{"value":10}},
            {"id":"converter.flac_compression","data":{"value":12}},
            {"id":"weather.unit","data":{"value":"fahrenheit"}},
            {"id":"podcast.country","data":{"value":"us"}},
            {"id":"reader.auto_bookmark","data":{"value":true}}
        ]);
        for mac in [false, true] {
            let mut settings =
                json!({"private_api_key":"secret","converter_output":"private/path"});
            apply(&mut settings, &rows, mac).unwrap();
            let exported = export(&settings, mac);
            for row in rows.as_array().unwrap() {
                assert!(exported.as_array().unwrap().contains(row), "{}", row["id"]);
            }
            assert_eq!(settings["private_api_key"], "secret");
            assert_eq!(settings["converter_output"], "private/path");
            for (id, value) in [
                ("cutter.seek_ms", json!(7000)),
                ("converter.wav_depth", json!("bad")),
                ("converter.bitrate", json!(999)),
                ("converter.ogg_quality", json!(11)),
            ] {
                assert!(project(&json!([{"id":id,"data":{"value":value}}]), mac).is_err());
            }
        }
    }
    #[test]
    fn desktop_settings_persist_and_default_without_new_fields() {
        // Uses serde only: does not read or modify the user's settings file.
        let mut settings = crate::settings::AppSettings::default();
        settings.media_cutter_seek_step_ms = 120000;
        settings.radio_search_language = "fr".into();
        settings.radio_search_country = "fr".into();
        settings.converter_format = "flac".into();
        settings.converter_flac_compression = 12;
        let encoded = serde_json::to_value(&settings).unwrap();
        let restored: crate::settings::AppSettings =
            serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(restored.media_cutter_seek_step_ms, 120000);
        assert_eq!(restored.converter_format, "flac");
        assert_eq!(restored.converter_flac_compression, 12);
        let mut old = encoded;
        for key in [
            "media_cutter_seek_step_ms",
            "converter_format",
            "converter_bitrate",
        ] {
            old.as_object_mut().unwrap().remove(key);
        }
        let restored: crate::settings::AppSettings = serde_json::from_value(old).unwrap();
        assert_eq!(restored.media_cutter_seek_step_ms, 5000);
        assert_eq!(restored.converter_format, "mp3");
        assert_eq!(restored.converter_bitrate, 192);
    }
    #[test]
    fn portable_preferences_and_private_settings() {
        let original = json!({"sonartube_search_type":1,"sonarmusic_search_type":"album","media_cutter_advanced_mode":true,"weather_city_query":"Roma","password":"secret","download_folder":"private"});
        for mac in [false, true] {
            let rows = export(&original, mac);
            assert!(!rows.to_string().contains("secret"));
            assert!(!rows.to_string().contains("private"));
            let mut next = original.clone();
            next["sonartube_search_type"] = json!(0);
            apply(&mut next, &rows, mac).unwrap();
            assert_eq!(next, original);
            assert!(project(&json!([{"id":"cutter.mode","data":{"value":"bad"}}]), mac).is_err());
            assert_eq!(
                project(
                    &json!([{"id":"home.hidden","data":{"value":["radio"]}}]),
                    mac
                )
                .unwrap(),
                json!([])
            );
        }
    }
}

#[cfg(test)]
mod activation_sync_tests {
    use super::*;
    #[test]
    fn invalid_or_absent_activation_never_clears_local_code() {
        for mac in [false, true] {
            let mut local = json!({"rai_luce_code":"local-sentinel"});
            apply(&mut local, &json!([]), mac).unwrap();
            assert!(local["rai_luce_code"].as_str() == Some("local-sentinel"));
            let invalid = json!([{"id":"activation.code","data":{"value":"invalid"}}]);
            assert!(
                project(&invalid, mac)
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            apply(&mut local, &invalid, mac).unwrap();
            assert!(local["rai_luce_code"].as_str() == Some("local-sentinel"));
            assert!(
                export(&local, mac)
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|r| r["id"] != "activation.code")
            );
        }
    }
}

#[cfg(test)]
mod activation_local_check {
    use super::*;
    #[test]
    #[ignore = "Uses the owner's already saved activation, never prints it"]
    fn saved_activation_round_trip() {
        let code = crate::settings::load_saved_rai_luce_code().expect("No saved activation");
        assert!(valid("activation.code", &json!(code)));
        let rows = export(&json!({"rai_luce_code":code}), false);
        for mac in [false, true] {
            let mut target = json!({"rai_luce_code":""});
            apply(&mut target, &rows, mac).unwrap();
            assert!(target["rai_luce_code"].as_str() == Some(code.as_str()));
        }
        if let Ok(path) = std::env::var("SONARPAD_ACTIVATION_TEST_FILE") {
            std::fs::write(path, serde_json::to_vec(&rows).unwrap()).unwrap();
        }
    }
}
