use crate::settings::{AppSettings, TtsEngine};
use std::path::PathBuf;

#[derive(Clone, PartialEq, Eq)]
pub struct DialogueVoiceConfig {
    pub engine: TtsEngine,
    pub voice: String,
    pub use_secondary_voice: bool,
    pub secondary_voice: String,
    pub secondary_engine: TtsEngine,
    pub secondary_rate: i32,
    pub secondary_pitch: i32,
    pub secondary_volume: i32,
    pub rate: i32,
    pub pitch: i32,
    pub volume: i32,
    pub opening_quote: String,
    pub closing_quote: String,
    pub allow_multiline: bool,
}

impl DialogueVoiceConfig {
    pub fn config_path() -> PathBuf {
        crate::settings::settings_dir().join("dialogue.ini")
    }
}

pub fn parse_engine_input(input: &str) -> Option<TtsEngine> {
    match input.trim().to_ascii_lowercase().as_str() {
        "edge" | "microsoft" | "microsoft voices" => Some(TtsEngine::Edge),
        "sapi5" | "sapi 5" => Some(TtsEngine::Sapi5),
        "sapi4" | "sapi 4" => Some(TtsEngine::Sapi4),
        "google" | "google tts" => Some(TtsEngine::Google),
        "gemini" => Some(TtsEngine::Gemini),
        _ => None,
    }
}

pub fn engine_to_key(engine: TtsEngine) -> &'static str {
    match engine {
        TtsEngine::Edge => "edge",
        TtsEngine::Sapi5 => "sapi5",
        TtsEngine::Sapi4 => "sapi4",
        TtsEngine::Google => "google",
        TtsEngine::Gemini => "gemini",
    }
}

pub fn save_dialogue_voice_config(cfg: &DialogueVoiceConfig) -> Result<(), String> {
    let config_path = DialogueVoiceConfig::config_path();
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create dialogue config folder {:?}: {}",
                parent, e
            )
        })?;
    }
    let body = format!(
        "engine={}\nvoice={}\nuse_secondary_voice={}\nsecondary_voice={}\nsecondary_engine={}\nsecondary_rate={}\nsecondary_pitch={}\nsecondary_volume={}\nrate={}\npitch={}\nvolume={}\nopen_quote={}\nclose_quote={}\nallow_multiline={}\n",
        engine_to_key(cfg.engine),
        cfg.voice,
        if cfg.use_secondary_voice {
            "true"
        } else {
            "false"
        },
        cfg.secondary_voice,
        engine_to_key(cfg.secondary_engine),
        cfg.secondary_rate,
        cfg.secondary_pitch,
        cfg.secondary_volume,
        cfg.rate,
        cfg.pitch,
        cfg.volume,
        cfg.opening_quote,
        cfg.closing_quote,
        if cfg.allow_multiline { "true" } else { "false" }
    );
    std::fs::write(&config_path, body)
        .map_err(|e| format!("Failed to save dialogue config {:?}: {}", config_path, e))
}

pub fn load_dialogue_voice_config() -> Option<DialogueVoiceConfig> {
    let config_path = DialogueVoiceConfig::config_path();
    let text = std::fs::read_to_string(config_path).ok()?;
    let mut engine = None;
    let mut voice = String::new();
    let mut use_secondary_voice = false;
    let mut secondary_voice = String::new();
    let mut secondary_engine = None;
    let mut secondary_rate = 0;
    let mut secondary_pitch = 0;
    let mut secondary_volume = 100;
    let mut rate = 0;
    let mut pitch = 0;
    let mut volume = 100;
    let mut opening_quote = "\"|\u{201C}|\u{00AB}|\u{201E}".to_string();
    let mut closing_quote = "\"|\u{201D}|\u{00BB}|\u{201C}".to_string();
    let mut allow_multiline = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim().to_ascii_lowercase();
        let val = v.trim();
        match key.as_str() {
            "engine" => engine = parse_engine_input(val),
            "voice" => voice = val.to_string(),
            "use_secondary_voice" => {
                use_secondary_voice =
                    matches!(val.to_ascii_lowercase().as_str(), "true" | "1" | "yes")
            }
            "secondary_voice" => secondary_voice = val.to_string(),
            "secondary_engine" => secondary_engine = parse_engine_input(val),
            "secondary_rate" => {
                if let Ok(parsed) = val.parse::<i32>() {
                    secondary_rate = parsed;
                }
            }
            "secondary_pitch" => {
                if let Ok(parsed) = val.parse::<i32>() {
                    secondary_pitch = parsed;
                }
            }
            "secondary_volume" => {
                if let Ok(parsed) = val.parse::<i32>() {
                    secondary_volume = parsed;
                }
            }
            "rate" => {
                if let Ok(parsed) = val.parse::<i32>() {
                    rate = parsed;
                }
            }
            "pitch" => {
                if let Ok(parsed) = val.parse::<i32>() {
                    pitch = parsed;
                }
            }
            "volume" => {
                if let Ok(parsed) = val.parse::<i32>() {
                    volume = parsed;
                }
            }
            "open_quote" => opening_quote = val.to_string(),
            "close_quote" => closing_quote = val.to_string(),
            "allow_multiline" => {
                allow_multiline = matches!(val.to_ascii_lowercase().as_str(), "true" | "1" | "yes")
            }
            _ => {}
        }
    }

    let engine = engine?;
    if voice.trim().is_empty() || opening_quote.is_empty() || closing_quote.is_empty() {
        return None;
    }
    if opening_quote == "\"" {
        opening_quote = "\"|\u{201C}|\u{00AB}|\u{201E}".to_string();
    }
    if closing_quote == "\"" || closing_quote == "\"|\u{201D}|\u{00BB}" {
        closing_quote = "\"|\u{201D}|\u{00BB}|\u{201C}".to_string();
    }
    Some(DialogueVoiceConfig {
        engine,
        voice,
        use_secondary_voice,
        secondary_voice,
        secondary_engine: secondary_engine.unwrap_or(engine),
        secondary_rate,
        secondary_pitch,
        secondary_volume,
        rate,
        pitch,
        volume,
        opening_quote,
        closing_quote,
        allow_multiline,
    })
}

fn xml_escape_attr(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;")
}

fn find_closing(
    text: &str,
    search_from: usize,
    closing_quotes: &[String],
    allow_multiline: bool,
) -> Option<(usize, usize)> {
    if closing_quotes.is_empty() {
        return None;
    }
    let search_end = if allow_multiline {
        text.len()
    } else {
        text[search_from..]
            .find('\n')
            .map(|p| search_from + p)
            .unwrap_or(text.len())
    };
    let mut best: Option<(usize, usize)> = None;
    for quote in closing_quotes {
        if quote.is_empty() {
            continue;
        }
        if let Some(rel) = text[search_from..search_end].find(quote) {
            let pos = search_from + rel;
            let len = quote.len();
            match best {
                None => best = Some((pos, len)),
                Some((best_pos, best_len)) => {
                    if pos < best_pos || (pos == best_pos && len > best_len) {
                        best = Some((pos, len));
                    }
                }
            }
        }
    }
    best
}

fn find_opening(text: &str, cursor: usize, opening_quotes: &[String]) -> Option<(usize, usize)> {
    if opening_quotes.is_empty() {
        return None;
    }
    let mut best: Option<(usize, usize)> = None;
    for quote in opening_quotes {
        if quote.is_empty() {
            continue;
        }
        if let Some(rel) = text[cursor..].find(quote) {
            let pos = cursor + rel;
            let len = quote.len();
            match best {
                None => best = Some((pos, len)),
                Some((best_pos, best_len)) => {
                    if pos < best_pos || (pos == best_pos && len > best_len) {
                        best = Some((pos, len));
                    }
                }
            }
        }
    }
    best
}

fn parse_quote_delimiters(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let has_explicit_separator = trimmed
        .chars()
        .any(|ch| matches!(ch, '|' | ';' | ',' | '\n' | '\r'));
    let mut delimiters = if has_explicit_separator {
        trimmed
            .split(['|', ';', ',', '\n', '\r'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    } else {
        let space_split = trimmed
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        if space_split.len() > 1 {
            space_split.into_iter().map(ToOwned::to_owned).collect()
        } else {
            vec![trimmed.to_string()]
        }
    };
    delimiters.sort();
    delimiters.dedup();
    delimiters
}

fn quote_delimiters_log(delimiters: &[String]) -> String {
    delimiters
        .iter()
        .map(|d| format!("{:?}", d))
        .collect::<Vec<_>>()
        .join(", ")
}

fn preview_for_log(text: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(max_chars);
    for ch in text.chars().take(max_chars) {
        if ch == '\n' || ch == '\r' || ch == '\t' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

fn voice_tag_ranges(text: &str) -> Vec<(usize, usize)> {
    let lower = text.to_ascii_lowercase();
    let mut ranges = Vec::new();
    let mut cursor = 0usize;
    while let Some(rel_start) = lower[cursor..].find("<voice") {
        let start = cursor + rel_start;
        let Some(open_end_rel) = lower[start..].find('>') else {
            ranges.push((start, text.len()));
            break;
        };
        let open_end = start + open_end_rel + 1;
        let end = lower[open_end..]
            .find("</voice>")
            .map(|rel_end| open_end + rel_end + "</voice>".len())
            .unwrap_or(open_end);
        ranges.push((start, end));
        cursor = end;
    }
    ranges
}

fn containing_range(ranges: &[(usize, usize)], pos: usize) -> Option<(usize, usize)> {
    ranges
        .iter()
        .copied()
        .find(|(start, end)| *start <= pos && pos < *end)
}

fn intersects_range(ranges: &[(usize, usize)], start: usize, end: usize) -> bool {
    ranges
        .iter()
        .any(|(range_start, range_end)| start < *range_end && *range_start < end)
}

pub fn apply_dialogue_tags(text: &str, cfg: &DialogueVoiceConfig) -> String {
    if text.is_empty() || cfg.voice.trim().is_empty() {
        crate::log_debug("Dialogue tagging: skipped (empty text or empty primary voice)");
        return text.to_string();
    }
    if cfg.opening_quote.is_empty() || cfg.closing_quote.is_empty() {
        crate::log_debug("Dialogue tagging: skipped (empty opening/closing quote)");
        return text.to_string();
    }
    let opening_quotes = parse_quote_delimiters(&cfg.opening_quote);
    let closing_quotes = parse_quote_delimiters(&cfg.closing_quote);
    if opening_quotes.is_empty() || closing_quotes.is_empty() {
        crate::log_debug("Dialogue tagging: skipped (no valid opening/closing quote delimiters)");
        return text.to_string();
    }
    let explicit_voice_ranges = voice_tag_ranges(text);
    crate::log_debug(&format!(
        "Dialogue tagging: start open_quote=[{}] close_quote=[{}] allow_multiline={} primary_engine={} primary_voice={:?} secondary_enabled={} secondary_engine={} secondary_voice={:?}",
        quote_delimiters_log(&opening_quotes),
        quote_delimiters_log(&closing_quotes),
        cfg.allow_multiline,
        engine_to_key(cfg.engine),
        cfg.voice,
        cfg.use_secondary_voice,
        engine_to_key(cfg.secondary_engine),
        cfg.secondary_voice
    ));

    let build_open_tag = |engine: TtsEngine,
                          voice_name: &str,
                          rate: i32,
                          pitch: i32,
                          volume: i32| {
        format!(
            "<voice engine=\"{}\" voice=\"{}\" rate=\"{}\" pitch=\"{}\" volume=\"{}\" sonarpad-dialogue=\"1\">",
            engine_to_key(engine),
            xml_escape_attr(voice_name),
            rate,
            pitch,
            volume
        )
    };
    let close_tag = "</voice>";
    let use_secondary = cfg.use_secondary_voice && !cfg.secondary_voice.trim().is_empty();
    let secondary_voice = cfg.secondary_voice.trim();
    let mut out = String::with_capacity(text.len() + 128);
    let mut cursor = 0usize;
    let mut replaced_any = false;
    let mut use_secondary_next = false;
    let mut dialogue_idx = 0usize;

    while cursor < text.len() {
        let Some((open_pos, open_len)) = find_opening(text, cursor, &opening_quotes) else {
            out.push_str(&text[cursor..]);
            break;
        };
        if let Some((_, range_end)) = containing_range(&explicit_voice_ranges, open_pos) {
            out.push_str(&text[cursor..range_end]);
            cursor = range_end;
            continue;
        }
        out.push_str(&text[cursor..open_pos]);
        let search_from = open_pos + open_len;
        let Some((close_pos, close_len)) =
            find_closing(text, search_from, &closing_quotes, cfg.allow_multiline)
        else {
            crate::log_debug(&format!(
                "Dialogue tagging: unclosed quote at pos={} preview={:?}",
                open_pos,
                preview_for_log(&text[open_pos..], 120)
            ));
            out.push_str(&text[open_pos..]);
            break;
        };
        let end = close_pos + close_len;
        if intersects_range(&explicit_voice_ranges, open_pos, end) {
            crate::log_debug(&format!(
                "Dialogue tagging: skipped segment={} range=[{}..{}] because it overlaps explicit <voice> tag",
                dialogue_idx, open_pos, end
            ));
            out.push_str(&text[open_pos..end]);
            cursor = end;
            continue;
        }
        let before_toggle = use_secondary_next;
        let (engine, selected_voice, rate, pitch, volume) = if use_secondary && use_secondary_next {
            (
                cfg.secondary_engine,
                secondary_voice,
                cfg.secondary_rate,
                cfg.secondary_pitch,
                cfg.secondary_volume,
            )
        } else {
            (
                cfg.engine,
                cfg.voice.as_str(),
                cfg.rate,
                cfg.pitch,
                cfg.volume,
            )
        };
        let open_tag = build_open_tag(engine, selected_voice, rate, pitch, volume);
        out.push_str(&open_tag);
        out.push_str(&text[open_pos..end]);
        out.push_str(close_tag);
        crate::log_debug(&format!(
            "Dialogue tagging: segment={} range=[{}..{}] used_engine={} used_voice={:?} rate={} pitch={} volume={} secondary_before_toggle={} text_preview={:?}",
            dialogue_idx,
            open_pos,
            end,
            engine_to_key(engine),
            selected_voice,
            rate,
            pitch,
            volume,
            before_toggle,
            preview_for_log(&text[open_pos..end], 120)
        ));
        if use_secondary {
            use_secondary_next = !use_secondary_next;
        }
        cursor = end;
        replaced_any = true;
        dialogue_idx += 1;
    }

    if replaced_any {
        crate::log_debug(&format!(
            "Dialogue tagging: done segments_tagged={}",
            dialogue_idx
        ));
        out
    } else {
        crate::log_debug("Dialogue tagging: done (no matching quoted segments found)");
        text.to_string()
    }
}

pub fn apply_dialogue_tags_from_settings(text: &str, settings: &AppSettings) -> String {
    if !settings.use_dialogue_voice {
        crate::log_debug("Dialogue tagging: disabled in settings");
        return text.to_string();
    }
    let fallback = DialogueVoiceConfig {
        engine: settings.dialogue_tts_engine,
        voice: settings.dialogue_voice.clone(),
        use_secondary_voice: settings.dialogue_use_secondary_voice,
        secondary_voice: settings.dialogue_secondary_voice.clone(),
        secondary_engine: settings.dialogue_secondary_tts_engine,
        secondary_rate: settings.dialogue_secondary_voice_rate,
        secondary_pitch: settings.dialogue_secondary_voice_pitch,
        secondary_volume: settings.dialogue_secondary_voice_volume,
        rate: settings.dialogue_voice_rate,
        pitch: settings.dialogue_voice_pitch,
        volume: settings.dialogue_voice_volume,
        opening_quote: settings.dialogue_opening_quote.clone(),
        closing_quote: settings.dialogue_closing_quote.clone(),
        allow_multiline: settings.dialogue_allow_multiline,
    };
    let has_settings_dialogue_config = !fallback.voice.trim().is_empty()
        && !fallback.opening_quote.trim().is_empty()
        && !fallback.closing_quote.trim().is_empty();
    let cfg = if has_settings_dialogue_config {
        crate::log_debug("Dialogue tagging: using in-memory settings configuration");
        fallback
    } else if let Some(file_cfg) = load_dialogue_voice_config() {
        crate::log_debug("Dialogue tagging: using legacy dialogue.ini fallback");
        file_cfg
    } else {
        crate::log_debug("Dialogue tagging: no valid dialogue configuration found");
        return text.to_string();
    };
    apply_dialogue_tags(text, &cfg)
}

#[cfg(test)]
mod tests {
    use super::{DialogueVoiceConfig, apply_dialogue_tags};
    use crate::settings::TtsEngine;

    fn cfg(opening: &str, closing: &str, allow_multiline: bool) -> DialogueVoiceConfig {
        DialogueVoiceConfig {
            engine: TtsEngine::Edge,
            voice: "it-IT-ElsaNeural".to_string(),
            use_secondary_voice: false,
            secondary_voice: String::new(),
            secondary_engine: TtsEngine::Edge,
            secondary_rate: 0,
            secondary_pitch: 0,
            secondary_volume: 100,
            rate: 0,
            pitch: 0,
            volume: 100,
            opening_quote: opening.to_string(),
            closing_quote: closing.to_string(),
            allow_multiline,
        }
    }

    #[test]
    fn dialogue_tags_wrap_quoted_text_same_line() {
        let text = r#"Lui disse: "ciao"."#;
        let out = apply_dialogue_tags(text, &cfg("\"", "\"", false));
        assert!(out.contains("<voice"));
        assert!(out.contains("\"ciao\""));
    }

    #[test]
    fn dialogue_tags_do_not_cross_line_when_disabled() {
        let text = "\"ciao\nmondo\"";
        let out = apply_dialogue_tags(text, &cfg("\"", "\"", false));
        assert_eq!(out, text);
    }

    #[test]
    fn dialogue_tags_alternate_primary_and_secondary_voice() {
        let text = "\"ciao\" e \"come va\"";
        let mut settings = cfg("\"", "\"", false);
        settings.use_secondary_voice = true;
        settings.secondary_voice = "it-IT-DiegoNeural".to_string();
        settings.secondary_rate = -10;
        settings.secondary_pitch = 6;
        settings.secondary_volume = 130;
        let out = apply_dialogue_tags(text, &settings);
        assert!(out.contains("it-IT-ElsaNeural"));
        assert!(out.contains("it-IT-DiegoNeural"));
        assert!(
            out.contains("voice=\"it-IT-DiegoNeural\" rate=\"-10\" pitch=\"6\" volume=\"130\"")
        );
    }

    #[test]
    fn dialogue_tags_support_multiple_quote_delimiters() {
        let text = "Prima. \"Ciao mondo” Dopo.";
        let mut settings = cfg("\"|“|«", "\"|”|»", false);
        settings.use_secondary_voice = true;
        settings.secondary_voice = "it-IT-DiegoNeural".to_string();
        let out = apply_dialogue_tags(text, &settings);
        assert!(out.contains("<voice"));
        assert!(out.contains("\"Ciao mondo”"));
        assert!(out.contains("Prima. "));
        assert!(out.contains(" Dopo."));
    }

    #[test]
    fn dialogue_tags_support_czech_quotes_and_mixed_edge_sapi5_voices() {
        let text = "Řekl: „Ahoj.“ Odpověděla: „Dobrý den.“";
        let mut settings = cfg("\"|“|«|„", "\"|”|»|“", false);
        settings.engine = TtsEngine::Edge;
        settings.voice = "cs-CZ-AntoninNeural".to_string();
        settings.use_secondary_voice = true;
        settings.secondary_engine = TtsEngine::Sapi5;
        settings.secondary_voice = "Microsoft Zira Desktop".to_string();

        let out = apply_dialogue_tags(text, &settings);
        assert_eq!(out.matches("<voice ").count(), 2);
        assert!(out.contains("engine=\"edge\" voice=\"cs-CZ-AntoninNeural\""));
        assert!(out.contains("engine=\"sapi5\" voice=\"Microsoft Zira Desktop\""));
        assert!(out.contains("„Ahoj.“"));
        assert!(out.contains("„Dobrý den.“"));
    }

    #[test]
    fn dialogue_tags_keep_explicit_voice_tag_and_tag_later_dialogue() {
        let text = r#"<voice engine="edge" voice="it-IT-DiegoNeural">"tag"</voice> poi "dialogo""#;
        let out = apply_dialogue_tags(text, &cfg("\"", "\"", false));
        assert!(out.contains(r#"<voice engine="edge" voice="it-IT-DiegoNeural">"tag"</voice>"#));
        assert!(out.contains(r#"<voice engine="edge" voice="it-IT-ElsaNeural""#));
        assert!(out.contains(r#">"dialogo"</voice>"#));
    }

    #[test]
    fn dialogue_tags_do_not_wrap_quote_that_overlaps_explicit_voice_tag() {
        let text = r#""prima <voice engine="edge" voice="it-IT-DiegoNeural">tag</voice> dopo""#;
        let out = apply_dialogue_tags(text, &cfg("\"", "\"", false));
        assert_eq!(out, text);
    }
}
