mod windows_spellcheck;

pub use windows_spellcheck::{Misspelling, WindowsSpellChecker, utf16_offset_to_utf8_byte_offset};

use crate::settings::{AppSettings, Language, SpellcheckLanguageMode};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LineCacheKey {
    doc_id: isize,
    line_index: i32,
    line_hash: u64,
    language: String,
}

#[derive(Clone, Debug)]
pub struct SpellcheckLanguageResolution {
    pub requested: String,
    pub effective: String,
    pub announce_fallback: bool,
}

#[derive(Default)]
pub struct SpellcheckManager {
    checker: WindowsSpellChecker,
    line_cache: HashMap<LineCacheKey, Vec<Misspelling>>,
    ignore_once: HashSet<String>,
    last_fallback: Option<(String, String)>,
}

impl SpellcheckManager {
    pub fn resolve_language(
        &mut self,
        settings: &AppSettings,
    ) -> Option<SpellcheckLanguageResolution> {
        if !settings.spellcheck_enabled {
            self.last_fallback = None;
            return None;
        }

        let requested = match settings.spellcheck_language_mode {
            SpellcheckLanguageMode::FollowEditorLanguage => {
                editor_language_tag(settings.language).to_string()
            }
            SpellcheckLanguageMode::FixedLanguage => {
                settings.spellcheck_fixed_language.trim().to_string()
            }
        };
        let requested = if requested.is_empty() {
            "en-US".to_string()
        } else {
            requested
        };

        let mut effective = requested.clone();
        let mut used_fallback = false;
        if !self.checker.is_language_supported(&effective) {
            used_fallback = true;
            if let Some(system) =
                system_language_tag().filter(|tag| self.checker.is_language_supported(tag))
            {
                effective = system;
            } else if self.checker.is_language_supported("en-US") {
                effective = "en-US".to_string();
            } else {
                let mut languages = self.checker.supported_languages();
                let first = languages.pop()?;
                effective = first;
            }
        }

        if !self.checker.set_language(&effective) {
            return None;
        }

        let announce_fallback = if used_fallback {
            let key = (requested.clone(), effective.clone());
            if self.last_fallback.as_ref() != Some(&key) {
                self.last_fallback = Some(key);
                true
            } else {
                false
            }
        } else {
            self.last_fallback = None;
            false
        };

        Some(SpellcheckLanguageResolution {
            requested,
            effective,
            announce_fallback,
        })
    }

    pub fn clear_cache(&mut self) {
        self.line_cache.clear();
    }

    pub fn check_line(
        &mut self,
        doc_id: isize,
        line_index: i32,
        line_text: &str,
        language: &str,
    ) -> Vec<Misspelling> {
        let line_hash = hash_line(line_text);
        let key = LineCacheKey {
            doc_id,
            line_index,
            line_hash,
            language: language.to_string(),
        };
        if let Some(hit) = self.line_cache.get(&key) {
            return hit.clone();
        }

        self.line_cache.retain(|k, _| {
            !(k.doc_id == doc_id
                && k.line_index == line_index
                && k.language == language
                && k.line_hash != line_hash)
        });

        let misspellings = self.checker.check_text(line_text);
        self.line_cache.insert(key, misspellings.clone());
        misspellings
    }

    pub fn is_word_misspelled(
        &mut self,
        doc_id: isize,
        line_index: i32,
        line_text: &str,
        word_range: (usize, usize),
        language: &str,
    ) -> Option<Misspelling> {
        let word = line_text
            .get(word_range.0..word_range.1)
            .unwrap_or("")
            .to_string();
        if word.is_empty() {
            return None;
        }
        if self.ignore_once.contains(&normalize_word(&word)) {
            return None;
        }

        let misspellings = self.check_line(doc_id, line_index, line_text, language);
        misspellings
            .into_iter()
            .find(|m| m.start <= word_range.0 && m.end >= word_range.1)
    }

    pub fn suggestions(&mut self, word: &str, language: &str) -> Vec<String> {
        if word.is_empty() || !self.checker.set_language(language) {
            return Vec::new();
        }
        self.checker.suggestions(word)
    }

    pub fn add_to_dictionary(&mut self, word: &str, language: &str) -> bool {
        if word.is_empty() || !self.checker.set_language(language) {
            return false;
        }
        let added = self.checker.add_to_dictionary(word);
        if added {
            self.ignore_once.remove(&normalize_word(word));
            self.clear_cache();
        }
        added
    }

    pub fn ignore_once(&mut self, word: &str, language: &str) -> bool {
        if word.is_empty() || !self.checker.set_language(language) {
            return false;
        }
        let ok = self.checker.ignore_once(word);
        self.ignore_once.insert(normalize_word(word));
        if ok {
            self.clear_cache();
        }
        ok
    }
}

pub fn editor_language_tag(language: Language) -> &'static str {
    match language {
        Language::Italian => "it-IT",
        Language::German => "de-DE",
        Language::Ukrainian | Language::English => "en-US",
        Language::Spanish => "es-ES",
        Language::Portuguese => "pt-PT",
        Language::PortugueseBrazilian => "pt-BR",
        Language::Swedish => "sv-SE",
        Language::Vietnamese => "vi-VN",
        Language::Czech => "cs-CZ",
        Language::Polish => "pl-PL",
        Language::French => "fr-FR",
        Language::Serbian => "sr-RS",
        Language::Lithuanian => "lt-LT",
        Language::Russian => "ru-RU",
        Language::Chinese => "en-US",
        Language::Hindi => "hi-IN",
        Language::Indonesian => "id-ID",
    }
}

pub fn hash_line(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

pub fn word_range_at(text: &str, byte_pos: usize) -> Option<(usize, usize)> {
    if text.is_empty() {
        return None;
    }
    let byte_pos = byte_pos.min(text.len());
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    if chars.is_empty() {
        return None;
    }

    let mut idx = None;
    for (i, (offset, _)) in chars.iter().enumerate() {
        if *offset == byte_pos {
            idx = Some(i);
            break;
        }
        if *offset > byte_pos {
            idx = Some(i.saturating_sub(1));
            break;
        }
    }
    let mut idx = idx.unwrap_or(chars.len() - 1);
    if !is_word_char(chars[idx].1) {
        if idx > 0 && is_word_char(chars[idx - 1].1) {
            idx -= 1;
        } else {
            return None;
        }
    }

    let mut start_idx = idx;
    while start_idx > 0 && is_word_char(chars[start_idx - 1].1) {
        start_idx -= 1;
    }
    let mut end_idx = idx;
    while end_idx + 1 < chars.len() && is_word_char(chars[end_idx + 1].1) {
        end_idx += 1;
    }

    let start = chars[start_idx].0;
    let end = if end_idx + 1 < chars.len() {
        chars[end_idx + 1].0
    } else {
        text.len()
    };
    if start == end {
        None
    } else {
        Some((start, end))
    }
}

pub fn utf8_byte_offset_to_utf16_units(text: &str, byte_idx: usize) -> i32 {
    let mut utf16_count = 0usize;
    for (idx, ch) in text.char_indices() {
        if idx >= byte_idx {
            break;
        }
        utf16_count += ch.len_utf16();
    }
    utf16_count as i32
}

fn normalize_word(word: &str) -> String {
    word.to_lowercase()
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '\'' || ch == '’'
}

fn system_language_tag() -> Option<String> {
    let mut buffer = [0u16; 85];
    let len = crate::get_user_default_locale_name_safe(&mut buffer);
    if len == 0 {
        return None;
    }
    let len = len.saturating_sub(1) as usize;
    let locale = String::from_utf16_lossy(&buffer[..len]);
    if locale.is_empty() {
        None
    } else {
        Some(locale)
    }
}
