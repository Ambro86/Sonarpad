use crate::i18n;
use crate::settings::Language;
use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::Value;
use std::fmt;
use std::time::Duration;

const MAX_PAGE_LENGTH: usize = 30000;
const LARGE_PAGE_MAX_DEFS: usize = 5;
const LARGE_PAGE_MAX_LINES: usize = 200;
const MAX_CHARS_PER_DEF: usize = 500;

#[derive(Debug, Clone)]
pub struct LookupOutput {
    pub lang: String,
    pub word: String,
    pub definitions: Vec<String>,
    pub synonyms: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DictionaryAndTranslation {
    pub dictionary: LookupOutput,
    pub translation: Option<LookupOutput>,
}

#[derive(Debug)]
pub enum LookupError {
    NotFound { lang: String, word: String },
    Api { code: String, info: String },
    Other(String),
}

impl fmt::Display for LookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LookupError::NotFound { lang, word } => {
                write!(f, "Word not found: {word} (lang={lang})")
            }
            LookupError::Api { code, info } => {
                write!(f, "MediaWiki API error ({code}): {info}")
            }
            LookupError::Other(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for LookupError {}

#[derive(Debug, Deserialize)]
struct MwParseResponse {
    parse: MwParse,
}

#[derive(Debug, Deserialize)]
struct MwParse {
    wikitext: String,
}

#[derive(Debug, Deserialize)]
struct MwInfoResponse {
    query: MwInfoQuery,
}

#[derive(Debug, Deserialize)]
struct MwInfoQuery {
    pages: Vec<MwInfoPage>,
}

#[derive(Debug, Deserialize)]
struct MwInfoPage {
    pageid: Option<i64>,
    missing: Option<bool>,
    length: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct MwErrorEnvelope {
    error: MwError,
}

#[derive(Debug, Deserialize)]
struct MwError {
    code: String,
    info: String,
}

fn validate_lang_subdomain(lang: &str) -> Result<(), LookupError> {
    if lang.is_empty() || !lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(LookupError::Other(format!(
            "Invalid Wiktionary language code: {lang}"
        )));
    }
    Ok(())
}

fn build_parse_wikitext_url(lang: &str, word: &str) -> Result<Url, LookupError> {
    validate_lang_subdomain(lang)?;
    let base = format!("https://{lang}.wiktionary.org/w/api.php");
    let mut url = Url::parse(&base).map_err(|err| LookupError::Other(err.to_string()))?;
    url.query_pairs_mut()
        .append_pair("action", "parse")
        .append_pair("page", word)
        .append_pair("prop", "wikitext")
        .append_pair("section", "1")
        .append_pair("format", "json")
        .append_pair("formatversion", "2");
    Ok(url)
}

fn build_page_info_url(lang: &str, word: &str) -> Result<Url, LookupError> {
    validate_lang_subdomain(lang)?;
    let base = format!("https://{lang}.wiktionary.org/w/api.php");
    let mut url = Url::parse(&base).map_err(|err| LookupError::Other(err.to_string()))?;
    url.query_pairs_mut()
        .append_pair("action", "query")
        .append_pair("prop", "info")
        .append_pair("titles", word)
        .append_pair("format", "json")
        .append_pair("formatversion", "2");
    Ok(url)
}
fn strip_links(mut s: String) -> String {
    while let Some(start) = s.find("[[") {
        let Some(end) = s[start + 2..].find("]]").map(|i| i + start + 2) else {
            break;
        };
        let inner = &s[start + 2..end];
        let replacement = inner.split('|').next_back().unwrap_or(inner).to_string();
        s.replace_range(start..end + 2, &replacement);
    }
    s
}

fn strip_external_links(mut s: String) -> String {
    while let Some(start) = s.find('[') {
        if s[start..].starts_with("[[") {
            let Some(end) = s[start + 2..].find("]]").map(|i| i + start + 2) else {
                break;
            };
            let inner = &s[start..end + 2];
            let rest = &s[end + 2..];
            let mut out = String::with_capacity(s.len());
            out.push_str(&s[..start]);
            out.push_str(inner);
            out.push_str(rest);
            s = out;
            continue;
        }
        let Some(end) = s[start + 1..].find(']').map(|i| i + start + 1) else {
            break;
        };
        let inner = &s[start + 1..end];
        let is_url = inner.starts_with("http://")
            || inner.starts_with("https://")
            || inner.starts_with("www.");
        let replacement = if is_url {
            inner
                .split_whitespace()
                .skip(1)
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            inner.to_string()
        };
        s.replace_range(start..end + 1, &replacement);
    }
    s
}

fn strip_html_comments(mut s: String) -> String {
    while let Some(start) = s.find("<!--") {
        let Some(end) = s[start + 4..].find("-->").map(|i| i + start + 4) else {
            break;
        };
        s.replace_range(start..end + 3, "");
    }
    s
}

fn strip_ref_blocks(mut s: String) -> String {
    loop {
        let lower = s.to_ascii_lowercase();
        let Some(start) = lower.find("<ref") else {
            break;
        };
        let Some(open_end) = lower[start..].find('>').map(|offset| start + offset) else {
            s.truncate(start);
            break;
        };
        if lower[start..=open_end].trim_end().ends_with("/>") {
            s.replace_range(start..=open_end, " ");
            continue;
        }
        let Some(close_start) = lower[open_end + 1..]
            .find("</ref>")
            .map(|offset| open_end + 1 + offset)
        else {
            s.truncate(start);
            break;
        };
        s.replace_range(start..close_start + "</ref>".len(), " ");
    }
    s
}

fn split_template_parts(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut link_depth = 0usize;
    let mut template_depth = 0usize;
    let mut index = 0usize;
    while index < inner.len() {
        if inner[index..].starts_with("[[") {
            link_depth += 1;
            index += 2;
            continue;
        }
        if inner[index..].starts_with("]]") && link_depth > 0 {
            link_depth -= 1;
            index += 2;
            continue;
        }
        if inner[index..].starts_with("{{") {
            template_depth += 1;
            index += 2;
            continue;
        }
        if inner[index..].starts_with("}}") && template_depth > 0 {
            template_depth -= 1;
            index += 2;
            continue;
        }
        let character = inner[index..].chars().next().unwrap_or('\0');
        if character == '|' && link_depth == 0 && template_depth == 0 {
            parts.push(&inner[start..index]);
            start = index + 1;
        }
        index += character.len_utf8().max(1);
    }
    parts.push(&inner[start..]);
    parts
}

fn strip_templates(s: String) -> String {
    if !s.contains("{{") {
        return s;
    }
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    let mut i = 0usize;
    while i < s.len() {
        if s[i..].starts_with("{{") {
            depth += 1;
            i += 2;
            continue;
        }
        if s[i..].starts_with("}}") && depth > 0 {
            depth -= 1;
            i += 2;
            continue;
        }
        let ch = s[i..].chars().next().unwrap_or('\0');
        if depth == 0 && ch != '\0' {
            out.push(ch);
        }
        i += ch.len_utf8().max(1);
    }
    out
}

fn simplify_template_inner(inner: &str) -> String {
    let parts = split_template_parts(inner);
    if parts.len() <= 1 {
        return String::new();
    }

    let template_name = parts[0].trim().to_ascii_lowercase();
    // Grammar/meta templates like {{w|f|sing|case}} can leak noisy fragments
    // into definitions; they are not semantic definition content.
    if matches!(template_name.as_str(), "w") {
        return String::new();
    }

    let mut last_nonempty = "";
    let mut last_with_letters = "";
    for arg in parts.iter().skip(1) {
        let trimmed = arg.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((name, value)) = trimmed.split_once('=') {
            let value = value.trim();
            if name.trim().eq_ignore_ascii_case("ft")
                && value.chars().any(|character| character.is_alphabetic())
            {
                last_nonempty = value;
                last_with_letters = value;
            }
            continue;
        }
        last_nonempty = trimmed;
        if trimmed.chars().any(|c| c.is_alphabetic()) {
            last_with_letters = trimmed;
        }
    }

    if !last_with_letters.is_empty() {
        last_with_letters.to_string()
    } else {
        last_nonempty.to_string()
    }
}

fn simplify_templates(mut s: String) -> String {
    while let Some(start) = s.find("{{") {
        let mut depth = 0usize;
        let mut end: Option<usize> = None;
        let mut i = start;
        while i + 1 < s.len() {
            if s[i..].starts_with("{{") {
                depth += 1;
                i += 2;
                continue;
            }
            if s[i..].starts_with("}}") {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                i += 2;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
                continue;
            }
            let ch = s[i..].chars().next().unwrap_or('\0');
            i += ch.len_utf8().max(1);
        }

        let Some(end_idx) = end else {
            break;
        };

        let inner = &s[start + 2..end_idx - 2];
        let replacement = simplify_template_inner(inner);
        s.replace_range(start..end_idx, &replacement);
    }
    s
}

fn strip_html_tags(s: String) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            continue;
        }
        if !in_tag {
            out.push(ch);
        }
    }
    out
}

fn clean_wikitext_line(s: String) -> String {
    let mut x = strip_html_comments(s);
    x = strip_ref_blocks(x);
    x = simplify_templates(x);
    x = strip_html_tags(x);
    x = strip_links(x);
    x = strip_external_links(x);
    x = x.replace("''", "");
    x.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn is_language_code_token(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "en" | "it" | "es" | "pt" | "sv" | "vi" | "cs" | "pl" | "fr" | "sr" | "de"
    )
}

fn is_grammar_meta_token(token: &str) -> bool {
    let normalized = token
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "w" | "m"
            | "f"
            | "n"
            | "mf"
            | "fm"
            | "sg"
            | "pl"
            | "sing"
            | "plural"
            | "case"
            | "nom"
            | "gen"
            | "dat"
            | "acc"
            | "voc"
    )
}

fn is_pure_grammar_marker_definition(definition: &str) -> bool {
    let tokens: Vec<&str> = definition
        .split_whitespace()
        .filter(|t| !t.trim().is_empty())
        .collect();
    if tokens.is_empty() {
        return false;
    }
    tokens.iter().all(|t| is_grammar_meta_token(t))
}

fn is_wikidata_qid_token(token: &str) -> bool {
    let t = token.trim();
    if t.len() < 2 {
        return false;
    }
    let mut chars = t.chars();
    if !matches!(chars.next(), Some('Q' | 'q')) {
        return false;
    }
    let rest: Vec<char> = chars.collect();
    if rest.is_empty() {
        return false;
    }
    let digit_count = rest.iter().take_while(|c| c.is_ascii_digit()).count();
    if digit_count == 0 {
        return false;
    }
    let tail = &rest[digit_count..];
    tail.is_empty() || (tail.len() == 1 && tail[0].is_ascii_alphabetic())
}

fn normalize_definition_noise(s: &str) -> String {
    let mut tokens: Vec<&str> = s.split_whitespace().collect();
    if tokens.is_empty() {
        return String::new();
    }

    // Some Wiktionary templates can leak one or more language codes at start (e.g. "it it ...").
    while let Some(first) = tokens.first() {
        if is_language_code_token(first) || is_wikidata_qid_token(first) {
            tokens.remove(0);
        } else {
            break;
        }
    }

    // Some entries leak raw grammar markers (e.g. "w f sing case").
    // Remove only clear leading marker sequences, keeping normal prose intact.
    let mut grammar_prefix_len = 0usize;
    for token in &tokens {
        if is_grammar_meta_token(token) {
            grammar_prefix_len += 1;
        } else {
            break;
        }
    }
    if grammar_prefix_len >= 2 && grammar_prefix_len < tokens.len() {
        tokens.drain(0..grammar_prefix_len);
    }

    // Drop trailing single-letter lowercase noise fragments (e.g. "... i")
    // and leaked Wikidata IDs (e.g. "... Q289").
    while let Some(last) = tokens.last() {
        if (last.chars().count() == 1 && last.chars().all(|c| c.is_ascii_lowercase()))
            || is_wikidata_qid_token(last)
            || is_hex_color_token(last)
        {
            tokens.pop();
        } else {
            break;
        }
    }

    // Drop trailing date-like revision noise (e.g. "... 2012-9-05")
    if let Some(last) = tokens.last() {
        let parts: Vec<&str> = last.split('-').collect();
        let looks_like_date = parts.len() == 3
            && parts[0].len() == 4
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
        if looks_like_date {
            tokens.pop();
        }
    }

    let mut out = tokens.join(" ").trim().to_string();
    if let Some(stripped) = strip_trailing_revision_date(&out) {
        out = stripped;
    }
    out = strip_compact_leading_qid(&out);
    if is_grammar_noise_definition(&out) {
        return String::new();
    }
    if out.is_empty() || is_language_code_token(out.trim()) || is_wikidata_qid_token(out.trim()) {
        String::new()
    } else {
        out
    }
}

fn is_hex_color_token(token: &str) -> bool {
    let t = token.trim();
    (t.len() == 6 || t.len() == 8) && t.chars().all(|c| c.is_ascii_hexdigit())
}

fn strip_compact_leading_qid(s: &str) -> String {
    let trimmed = s.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() < 3 || !(bytes[0] == b'Q' || bytes[0] == b'q') {
        return trimmed.to_string();
    }
    let mut i = 1usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i <= 1 {
        return trimmed.to_string();
    }
    if i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        // Optional Wikidata variant suffix (e.g. Q42569A), only when it is
        // a standalone suffix and not the beginning of a normal word.
        let next = i + 1;
        let suffix_is_standalone = next >= bytes.len()
            || bytes[next].is_ascii_whitespace()
            || bytes[next].is_ascii_punctuation();
        if suffix_is_standalone {
            i += 1;
        }
    }
    if i >= bytes.len() {
        return String::new();
    }
    trimmed[i..].trim().to_string()
}

fn is_grammar_noise_definition(definition: &str) -> bool {
    matches!(
        definition.trim().to_ascii_lowercase().as_str(),
        "ing-form" | "inflection of"
    )
}

fn strip_trailing_revision_date(s: &str) -> Option<String> {
    let mut end = s.len();
    for (idx, ch) in s.char_indices().rev() {
        if ch.is_ascii_digit() || ch == '-' {
            end = idx;
            continue;
        }
        break;
    }

    if end >= s.len() {
        return None;
    }

    let suffix = &s[end..];
    let parts: Vec<&str> = suffix.split('-').collect();
    let looks_like_date = parts.len() == 3
        && parts[0].len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    if !looks_like_date {
        return None;
    }

    let mut head = s[..end]
        .trim_end_matches(|c: char| c.is_whitespace() || c == '.' || c == ',' || c == ';')
        .to_string();
    if head.is_empty() {
        None
    } else {
        Some(std::mem::take(&mut head))
    }
}

fn is_probably_leaked_related_lemma(definition: &str, from_subpoint: bool) -> bool {
    if !from_subpoint {
        return false;
    }

    let trimmed = definition.trim();
    if !trimmed.ends_with('.') {
        return false;
    }

    let base = trimmed.trim_end_matches('.').trim();
    if base.is_empty() || base.split_whitespace().count() != 1 {
        return false;
    }

    let Some(first) = base.chars().next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }

    base.chars()
        .all(|c| c.is_alphabetic() || c == '-' || c == '\'')
}

fn is_probably_single_lemma_noise(definition: &str) -> bool {
    let trimmed = definition.trim();
    if !trimmed.ends_with('.') {
        return false;
    }
    let base = trimmed.trim_end_matches('.').trim();
    if base.is_empty() || base.split_whitespace().count() != 1 {
        return false;
    }
    let Some(first) = base.chars().next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    base.len() >= 3
        && base
            .chars()
            .all(|c| c.is_alphabetic() || c == '-' || c == '\'')
}

fn is_probably_bibliographic_noise(definition: &str, from_subpoint: bool) -> bool {
    if !from_subpoint {
        return false;
    }

    let normalized = definition
        .trim_end_matches('.')
        .split_whitespace()
        .collect::<Vec<_>>();
    if normalized.len() < 3 {
        return false;
    }

    let head = normalized[0];
    let marker = normalized[1].to_ascii_lowercase();
    let page = normalized[2];

    let head_is_simple_lemma = head
        .chars()
        .all(|c| c.is_alphabetic() || c == '-' || c == '\'')
        && head.chars().next().is_some_and(|c| c.is_ascii_lowercase());
    let marker_is_page = matches!(marker.as_str(), "pág." | "pág" | "pag." | "pag" | "p.");
    let page_is_number = page.chars().all(|c| c.is_ascii_digit());

    head_is_simple_lemma && marker_is_page && page_is_number
}

fn extract_definitions_with_subpoints(
    wikitext: &str,
    max_main_defs: usize,
    max_total_lines: usize,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut main_count = 0;

    for line in wikitext.lines() {
        if out.len() >= max_total_lines {
            break;
        }
        let l = line.trim_start();

        let mut candidate: Option<(&str, bool)> = None;
        if let Some(rest) = l.strip_prefix("# ") {
            candidate = Some((rest.trim(), false));
        } else if let Some(rest) = l.strip_prefix("#*") {
            // Italian Wiktionary often stores real senses as "#*" under a
            // grammar header line (e.g. "casa"), so we treat these as
            // definition-bearing subpoints.
            candidate = Some((rest.trim(), true));
        } else if let Some(rest) = l.strip_prefix(';') {
            let mut idx = 0usize;
            let chars: Vec<char> = rest.chars().collect();
            while idx < chars.len() && chars[idx].is_ascii_digit() {
                idx += 1;
            }
            if idx > 0 {
                let after_number = rest[idx..].trim_start();
                if let Some(colon_pos) = after_number.find(':') {
                    let after_colon = after_number[colon_pos + 1..].trim();
                    if !after_colon.is_empty() {
                        candidate = Some((after_colon, true));
                    }
                } else if !after_number.is_empty() {
                    candidate = Some((after_number, true));
                }
            }
        }

        if let Some((text, from_subpoint)) = candidate {
            if main_count >= max_main_defs {
                break;
            }
            let cleaned = clean_wikitext_line(text.to_string());
            let normalized = normalize_definition_noise(&cleaned);
            let truncated = normalized
                .chars()
                .take(MAX_CHARS_PER_DEF)
                .collect::<String>();
            if is_probably_leaked_related_lemma(&truncated, from_subpoint) {
                continue;
            }
            if is_probably_bibliographic_noise(&truncated, from_subpoint) {
                continue;
            }
            if !from_subpoint && !out.is_empty() && is_probably_single_lemma_noise(&truncated) {
                continue;
            }
            if is_pure_grammar_marker_definition(&truncated) {
                continue;
            }
            if !truncated.is_empty() && truncated.chars().any(|c| c.is_alphanumeric()) {
                out.push(truncated);
                main_count += 1;
            }
            if main_count >= max_main_defs {
                break;
            }
        }
    }

    out
}

fn parse_numbered_colon_item(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix(':')?.trim_start();
    if !rest.starts_with('[') {
        return None;
    }
    let closing = rest.find(']')?;
    let marker = &rest[1..closing];
    if marker.is_empty()
        || !marker.chars().all(|c| {
            c.is_ascii_digit()
                || c.is_whitespace()
                || matches!(c, ',' | '-' | '–' | '—' | '?' | '*')
        })
    {
        return None;
    }
    let text = rest[closing + 1..].trim();
    (!text.is_empty()).then_some(text)
}

fn extract_numbered_template_block(wikitext: &str, marker: &str, max_items: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block = false;

    for line in wikitext.lines() {
        let trimmed = line.trim_start();
        if let Some(remainder) = trimmed.strip_prefix(marker) {
            in_block = true;
            if let Some(item) = parse_numbered_colon_item(remainder.trim_start()) {
                let cleaned = normalize_definition_noise(&clean_wikitext_line(item.to_string()));
                let truncated = cleaned.chars().take(MAX_CHARS_PER_DEF).collect::<String>();
                if !truncated.is_empty() && truncated.chars().any(|c| c.is_alphanumeric()) {
                    out.push(truncated);
                    if out.len() >= max_items {
                        break;
                    }
                }
            }
            continue;
        }
        if !in_block {
            continue;
        }
        if trimmed.starts_with("{{") {
            in_block = false;
            continue;
        }
        let Some(item) = parse_numbered_colon_item(trimmed) else {
            continue;
        };
        let cleaned = normalize_definition_noise(&clean_wikitext_line(item.to_string()));
        let truncated = cleaned.chars().take(MAX_CHARS_PER_DEF).collect::<String>();
        if !truncated.is_empty() && truncated.chars().any(|c| c.is_alphanumeric()) {
            out.push(truncated);
            if out.len() >= max_items {
                break;
            }
        }
    }

    out
}

fn extract_definitions_for_language(
    wikitext: &str,
    language: &str,
    max_main_defs: usize,
    max_total_lines: usize,
) -> Vec<String> {
    if language.eq_ignore_ascii_case("de") {
        extract_numbered_template_block(wikitext, "{{Bedeutungen}}", max_main_defs)
    } else {
        extract_definitions_with_subpoints(wikitext, max_main_defs, max_total_lines)
    }
}

fn extract_synonyms_for_language(wikitext: &str, language: &str, max_syns: usize) -> Vec<String> {
    if language.eq_ignore_ascii_case("de") {
        extract_numbered_template_block(wikitext, "{{Synonyme}}", max_syns)
    } else {
        extract_synonyms(wikitext, max_syns)
    }
}

fn extract_synonyms(wikitext: &str, max_syns: usize) -> Vec<String> {
    let mut out = Vec::new();
    let start_pos = match wikitext.find("{{-sin-}}") {
        Some(p) => p,
        None => return out,
    };
    let after_start = &wikitext[start_pos + "{{-sin-}}".len()..];
    let end_pos = after_start.find("\n{{-").unwrap_or(after_start.len());
    let block = &after_start[..end_pos];
    let block = strip_templates(block.to_string());

    for line in block.lines() {
        if line.trim_start().starts_with('*') {
            let cleaned = clean_wikitext_line(line.trim_start()[1..].trim().to_string());
            if !cleaned.is_empty() {
                out.push(cleaned);
                if out.len() >= max_syns {
                    break;
                }
            }
        }
    }

    out
}

fn is_spanish_infinitive(token: &str) -> bool {
    let t = token.trim();
    if t.len() < 3 || !t.chars().all(|c| c.is_alphabetic() || c == '-') {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    lower.ends_with("ar") || lower.ends_with("er") || lower.ends_with("ir")
}

fn strip_trailing_definition_punctuation(s: &str) -> &str {
    s.trim().trim_end_matches(|c: char| {
        c.is_whitespace() || c == '.' || c == ',' || c == ';' || c == ':'
    })
}

fn remove_spanish_infinitive_noise_when_contextual(defs: &mut Vec<String>) {
    if defs.len() <= 1 {
        return;
    }
    defs.retain(|d| {
        let base = strip_trailing_definition_punctuation(d);
        if base.is_empty() || base.split_whitespace().count() != 1 {
            return true;
        }
        !is_spanish_infinitive(base)
    });
}

pub struct WiktionaryService {
    client: Client,
}

impl WiktionaryService {
    pub fn new() -> Result<Self, LookupError> {
        let client = Client::builder()
            .user_agent("Sonarpad/0.5 (Wiktionary dictionary)")
            .timeout(Duration::from_secs(4))
            .build()
            .map_err(|err| LookupError::Other(err.to_string()))?;
        Ok(Self { client })
    }

    fn fetch_page_length(&self, lang: &str, word: &str) -> Result<usize, LookupError> {
        let url = build_page_info_url(lang, word)?;
        let resp = self
            .client
            .get(url)
            .send()
            .map_err(|err| LookupError::Other(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(LookupError::Other(format!(
                "Wiktionary HTTP error: {}",
                resp.status()
            )));
        }

        let v: Value = resp
            .json()
            .map_err(|err| LookupError::Other(err.to_string()))?;
        if v.get("error").is_some() {
            let env: MwErrorEnvelope =
                serde_json::from_value(v).map_err(|err| LookupError::Other(err.to_string()))?;
            if env.error.code == "missingtitle" {
                return Err(LookupError::NotFound {
                    lang: lang.to_string(),
                    word: word.to_string(),
                });
            }
            return Err(LookupError::Api {
                code: env.error.code,
                info: env.error.info,
            });
        }

        let parsed: MwInfoResponse =
            serde_json::from_value(v).map_err(|err| LookupError::Other(err.to_string()))?;
        let page = parsed.query.pages.first().ok_or_else(|| {
            LookupError::Other("Wiktionary response missing page info".to_string())
        })?;
        if page.missing.unwrap_or(false) || page.pageid == Some(-1) {
            return Err(LookupError::NotFound {
                lang: lang.to_string(),
                word: word.to_string(),
            });
        }
        Ok(page.length.unwrap_or(0))
    }

    fn fetch_section1_wikitext(&self, lang: &str, word: &str) -> Result<String, LookupError> {
        let url = build_parse_wikitext_url(lang, word)?;
        let resp = self
            .client
            .get(url)
            .send()
            .map_err(|err| LookupError::Other(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(LookupError::Other(format!(
                "Wiktionary HTTP error: {}",
                resp.status()
            )));
        }

        let v: Value = resp
            .json()
            .map_err(|err| LookupError::Other(err.to_string()))?;
        if v.get("error").is_some() {
            let env: MwErrorEnvelope =
                serde_json::from_value(v).map_err(|err| LookupError::Other(err.to_string()))?;
            if env.error.code == "missingtitle" {
                return Err(LookupError::NotFound {
                    lang: lang.to_string(),
                    word: word.to_string(),
                });
            }
            return Err(LookupError::Api {
                code: env.error.code,
                info: env.error.info,
            });
        }

        let parsed: MwParseResponse =
            serde_json::from_value(v).map_err(|err| LookupError::Other(err.to_string()))?;
        Ok(parsed.parse.wikitext)
    }

    fn fetch_section1_wikitext_with_timeout(
        &self,
        lang: &str,
        word: &str,
        timeout: Duration,
    ) -> Result<String, LookupError> {
        let url = build_parse_wikitext_url(lang, word)?;
        let client = Client::builder()
            .user_agent("Sonarpad/0.5 (Wiktionary dictionary)")
            .timeout(timeout)
            .build()
            .map_err(|err| LookupError::Other(err.to_string()))?;
        let resp = client
            .get(url)
            .send()
            .map_err(|err| LookupError::Other(err.to_string()))?;
        if !resp.status().is_success() {
            return Err(LookupError::Other(format!(
                "Wiktionary HTTP error: {}",
                resp.status()
            )));
        }

        let v: Value = resp
            .json()
            .map_err(|err| LookupError::Other(err.to_string()))?;
        if v.get("error").is_some() {
            let env: MwErrorEnvelope =
                serde_json::from_value(v).map_err(|err| LookupError::Other(err.to_string()))?;
            if env.error.code == "missingtitle" {
                return Err(LookupError::NotFound {
                    lang: lang.to_string(),
                    word: word.to_string(),
                });
            }
            return Err(LookupError::Api {
                code: env.error.code,
                info: env.error.info,
            });
        }

        let parsed: MwParseResponse =
            serde_json::from_value(v).map_err(|err| LookupError::Other(err.to_string()))?;
        Ok(parsed.parse.wikitext)
    }

    fn dictionary_lookup_with_length(
        &self,
        dictionary_lang: &str,
        word: &str,
    ) -> Result<(LookupOutput, usize), LookupError> {
        let length = self.fetch_page_length(dictionary_lang, word)?;
        let wikitext = if length > MAX_PAGE_LENGTH {
            self.fetch_section1_wikitext_with_timeout(
                dictionary_lang,
                word,
                Duration::from_secs(8),
            )?
        } else {
            self.fetch_section1_wikitext(dictionary_lang, word)?
        };
        let (max_defs, max_lines) = if length > MAX_PAGE_LENGTH {
            (LARGE_PAGE_MAX_DEFS, LARGE_PAGE_MAX_LINES)
        } else {
            (usize::MAX, usize::MAX)
        };
        let defs =
            extract_definitions_for_language(&wikitext, dictionary_lang, max_defs, max_lines);
        let syns = if length > MAX_PAGE_LENGTH {
            Vec::new()
        } else {
            extract_synonyms_for_language(&wikitext, dictionary_lang, usize::MAX)
        };

        if defs.is_empty() {
            return Err(LookupError::NotFound {
                lang: dictionary_lang.to_string(),
                word: word.to_string(),
            });
        }

        let output = LookupOutput {
            lang: dictionary_lang.to_string(),
            word: word.to_string(),
            definitions: defs,
            synonyms: syns,
        };
        Ok((output, length))
    }

    pub fn dictionary_lookup(
        &self,
        dictionary_lang: &str,
        word: &str,
    ) -> Result<LookupOutput, LookupError> {
        self.dictionary_lookup_with_length(dictionary_lang, word)
            .map(|(output, _)| output)
    }

    pub fn translate_word(
        &self,
        target_lang: &str,
        word: &str,
    ) -> Result<LookupOutput, LookupError> {
        let length = self.fetch_page_length(target_lang, word)?;
        let wikitext = if length > MAX_PAGE_LENGTH {
            self.fetch_section1_wikitext_with_timeout(target_lang, word, Duration::from_secs(8))?
        } else {
            self.fetch_section1_wikitext(target_lang, word)?
        };
        let (max_defs, max_lines) = if length > MAX_PAGE_LENGTH {
            (LARGE_PAGE_MAX_DEFS, LARGE_PAGE_MAX_LINES)
        } else {
            (usize::MAX, usize::MAX)
        };
        let defs = extract_definitions_for_language(&wikitext, target_lang, max_defs, max_lines);
        if defs.is_empty() {
            return Err(LookupError::NotFound {
                lang: target_lang.to_string(),
                word: word.to_string(),
            });
        }
        Ok(LookupOutput {
            lang: target_lang.to_string(),
            word: word.to_string(),
            definitions: defs,
            synonyms: Vec::new(),
        })
    }

    pub fn dictionary_and_translation(
        &self,
        dictionary_lang: &str,
        target_lang: Option<&str>,
        word: &str,
    ) -> Result<DictionaryAndTranslation, LookupError> {
        let mut dict = self.dictionary_lookup(dictionary_lang, word)?;
        if dictionary_lang.eq_ignore_ascii_case("es") {
            remove_spanish_infinitive_noise_when_contextual(&mut dict.definitions);
        }
        let mut translation = match target_lang {
            None => None,
            Some(t) if t.eq_ignore_ascii_case(dictionary_lang) => None,
            Some(t) => match self.translate_word(t, word) {
                Ok(x) => Some(x),
                Err(LookupError::NotFound { .. }) => None,
                Err(err) => return Err(err),
            },
        };
        if dictionary_lang.eq_ignore_ascii_case("es")
            && let Some(t) = translation.as_mut()
        {
            remove_spanish_infinitive_noise_when_contextual(&mut t.definitions);
            if t.definitions.is_empty() {
                translation = None;
            }
        }
        Ok(DictionaryAndTranslation {
            dictionary: dict,
            translation,
        })
    }

    pub fn dictionary_and_translation_with_meta(
        &self,
        dictionary_lang: &str,
        target_lang: Option<&str>,
        word: &str,
    ) -> Result<(DictionaryAndTranslation, bool), LookupError> {
        let (mut dict, length) = self.dictionary_lookup_with_length(dictionary_lang, word)?;
        if dictionary_lang.eq_ignore_ascii_case("es") {
            remove_spanish_infinitive_noise_when_contextual(&mut dict.definitions);
        }
        let mut translation = match target_lang {
            None => None,
            Some(t) if t.eq_ignore_ascii_case(dictionary_lang) => None,
            Some(t) => match self.translate_word(t, word) {
                Ok(x) => Some(x),
                Err(LookupError::NotFound { .. }) => None,
                Err(err) => return Err(err),
            },
        };
        if dictionary_lang.eq_ignore_ascii_case("es")
            && let Some(t) = translation.as_mut()
        {
            remove_spanish_infinitive_noise_when_contextual(&mut t.definitions);
            if t.definitions.is_empty() {
                translation = None;
            }
        }
        Ok((
            DictionaryAndTranslation {
                dictionary: dict,
                translation,
            },
            length > MAX_PAGE_LENGTH,
        ))
    }
}

pub(crate) fn language_to_code(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::Ukrainian | Language::English => "en",
        Language::German => "de",
        Language::Lithuanian => "lt",
        Language::Spanish => "es",
        Language::Portuguese | Language::PortugueseBrazilian => "pt",
        Language::Swedish => "sv",
        Language::Vietnamese => "vi",
        Language::Czech => "cs",
        Language::Polish => "pl",
        Language::French => "fr",
        Language::Serbian => "sr",
        Language::Russian => "ru",
        Language::Chinese => "zh",
        Language::Hindi => "hi",
        Language::Indonesian => "id",
    }
}

fn translation_target_for_code(dictionary_lang: &str, preference: &str) -> Option<String> {
    let pref = preference.trim().to_ascii_lowercase();
    if pref.is_empty() || pref == "auto" {
        return (!dictionary_lang.eq_ignore_ascii_case("en")).then(|| "en".to_string());
    }
    if pref == "none" {
        return None;
    }
    let code = match pref.as_str() {
        "it" | "en" | "de" | "es" | "pt" | "sv" | "vi" | "cs" | "pl" | "fr" | "lt" | "ru"
        | "zh" => pref.as_str(),
        _ => {
            return (!dictionary_lang.eq_ignore_ascii_case("en")).then(|| "en".to_string());
        }
    };
    if code.eq_ignore_ascii_case(dictionary_lang) {
        None
    } else {
        Some(code.to_string())
    }
}

fn translation_target(language: Language, preference: &str) -> Option<String> {
    translation_target_for_code(language_to_code(language), preference)
}

pub fn lookup_for_language(
    word: &str,
    language: Language,
    translation_preference: &str,
) -> Result<DictionaryAndTranslation, LookupError> {
    let trimmed = word.trim();
    if trimmed.is_empty() {
        return Err(LookupError::Other("Empty word".to_string()));
    }
    let svc = WiktionaryService::new()?;
    let dict_lang = language_to_code(language);
    let target_lang = translation_target(language, translation_preference);
    svc.dictionary_and_translation(dict_lang, target_lang.as_deref(), trimmed)
}

pub fn lookup_for_code_with_meta(
    word: &str,
    dictionary_lang: &str,
    translation_preference: &str,
) -> Result<(DictionaryAndTranslation, bool), LookupError> {
    let trimmed = word.trim();
    if trimmed.is_empty() {
        return Err(LookupError::Other("Empty word".to_string()));
    }
    validate_lang_subdomain(dictionary_lang)?;
    let svc = WiktionaryService::new()?;
    let target_lang = translation_target_for_code(dictionary_lang, translation_preference);
    svc.dictionary_and_translation_with_meta(dictionary_lang, target_lang.as_deref(), trimmed)
}

fn push_definitions_menu(lines: &mut Vec<String>, definitions: &[String]) {
    for def in definitions {
        if def.starts_with("- ") {
            let trimmed = def.trim_start_matches("- ").trim();
            lines.push(trimmed.to_string());
        } else {
            lines.push(def.to_string());
        }
    }
}

pub fn format_output_text(language: Language, entry: &DictionaryAndTranslation) -> String {
    let mut out = String::new();
    let title = i18n::tr_f(
        language,
        "dictionary.word_label",
        &[("word", &entry.dictionary.word)],
    );
    out.push_str(&title);
    out.push_str("\n\n");

    let visible_defs: Vec<String> = entry
        .dictionary
        .definitions
        .iter()
        .filter(|def| !is_pure_grammar_marker_definition(def))
        .cloned()
        .collect();
    if !visible_defs.is_empty() {
        out.push_str(&i18n::tr(language, "dictionary.definitions"));
        out.push_str(":\n");
        for line in format_definition_lines(&visible_defs) {
            out.push_str(&line);
            out.push('\n');
        }
    }

    if !entry.dictionary.synonyms.is_empty() {
        out.push('\n');
        out.push_str(&i18n::tr(language, "dictionary.synonyms"));
        out.push_str(":\n");
        for syn in &entry.dictionary.synonyms {
            out.push_str(syn);
            out.push('\n');
        }
    }

    if let Some(translation) = &entry.translation {
        out.push('\n');
        let label = i18n::tr_f(
            language,
            "dictionary.translation_label",
            &[("lang", &translation.lang)],
        );
        out.push_str(&label);
        out.push('\n');
        for line in format_definition_lines(&translation.definitions) {
            out.push_str(&line);
            out.push('\n');
        }
    }

    out
}

pub fn format_menu_lines(language: Language, entry: &DictionaryAndTranslation) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(i18n::tr_f(
        language,
        "dictionary.word_label",
        &[("word", &entry.dictionary.word)],
    ));
    let visible_defs: Vec<String> = entry
        .dictionary
        .definitions
        .iter()
        .filter(|def| !is_pure_grammar_marker_definition(def))
        .cloned()
        .collect();
    if !visible_defs.is_empty() {
        lines.push(i18n::tr(language, "dictionary.definitions"));
        push_definitions_menu(&mut lines, &visible_defs);
    }

    if !entry.dictionary.synonyms.is_empty() {
        lines.push(i18n::tr(language, "dictionary.synonyms"));
        for syn in &entry.dictionary.synonyms {
            lines.push(syn.clone());
        }
    }

    if let Some(translation) = &entry.translation {
        let label = i18n::tr_f(
            language,
            "dictionary.translation_label",
            &[("lang", &translation.lang)],
        );
        lines.push(label);
        push_definitions_menu(&mut lines, &translation.definitions);
    }

    lines
        .into_iter()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn format_definition_lines(definitions: &[String]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut main_index = 1;
    for def in definitions {
        if def.starts_with("- ") {
            let trimmed = def.trim_start_matches("- ").trim();
            lines.push(trimmed.to_string());
        } else {
            lines.push(format!("{main_index}. {def}"));
            main_index += 1;
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::extract_definitions_for_language;
    use super::extract_definitions_with_subpoints;
    use super::extract_synonyms_for_language;
    use super::is_pure_grammar_marker_definition;
    use super::translation_target_for_code;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn german_haus_extracts_numbered_meanings() {
        let wikitext = "== Haus ({{Sprache|Deutsch}}) ==\n{{Bedeutungen}}\n:[1] zu einem bestimmten Zweck erbautes [[Gebäude]]\n:[2] zum Wohnen dienendes und genutztes [[Gebäude]]\n{{Abkürzungen}}\n:[[H.]]";
        let defs = extract_definitions_for_language(wikitext, "de", usize::MAX, usize::MAX);
        assert_eq!(
            defs,
            vec![
                "zu einem bestimmten Zweck erbautes Gebäude".to_string(),
                "zum Wohnen dienendes und genutztes Gebäude".to_string(),
            ]
        );
    }

    #[test]
    fn german_haus_extracts_synonyms_only_from_synonym_block() {
        let wikitext = "{{Bedeutungen}}\n:[1] ein Gebäude\n{{Synonyme}}\n:[1, 2] [[Wohnhaus]], [[Wohngebäude]]\n:[3] [[Heim]], [[Wohnung]], [[Zuhause]]\n{{Sinnverwandte Wörter}}\n:[1] [[Bau]]";
        let synonyms = extract_synonyms_for_language(wikitext, "de", usize::MAX);
        assert_eq!(
            synonyms,
            vec![
                "Wohnhaus, Wohngebäude".to_string(),
                "Heim, Wohnung, Zuhause".to_string(),
            ]
        );
    }

    #[test]
    fn german_multiple_meaning_blocks_are_combined() {
        let wikitext = "{{Bedeutungen}}\n:[1] erste Bedeutung\n{{Herkunft}}\n:erste Herkunft\n{{Bedeutungen}}\n:[1] zweite Bedeutung\n{{Beispiele}}\n:[1] Beispiel";
        let defs = extract_definitions_for_language(wikitext, "de", usize::MAX, usize::MAX);
        assert_eq!(
            defs,
            vec![
                "erste Bedeutung".to_string(),
                "zweite Bedeutung".to_string(),
            ]
        );
    }

    #[test]
    fn german_tag_keeps_piped_link_inside_context_template() {
        let wikitext = "{{Bedeutungen}}\n:[5] {{K|ft=in den [[bergmännisch]]en [[Ausdruck|Ausdrücken]] „zu Tage“, „über Tage“ und „unter Tage“}} die [[Erdoberfläche]]";
        let definitions = extract_definitions_for_language(wikitext, "de", usize::MAX, usize::MAX);
        assert_eq!(
            definitions,
            [
                "in den bergmännischen Ausdrücken „zu Tage“, „über Tage“ und „unter Tage“ die Erdoberfläche"
            ]
        );
        assert!(!definitions[0].contains("]]"));
    }

    #[test]
    fn german_lernen_removes_reference_bibliography_from_meaning() {
        let wikitext = "{{Bedeutungen}}\n:[5] {{K|Bedva.|landsch.}} [[lehren]], auch in alter Zeit nicht häufig,<ref>{{Ref-Grimm|Lernen}}, Abschnitte I 1) und II 2)</ref> galt als veraltet,<ref>{{Literatur|Autor=[[w:Johannes Saß|Johannes Sass]]|Titel=Wörterbuch}}</ref> heute regional";
        let definitions = extract_definitions_for_language(wikitext, "de", usize::MAX, usize::MAX);
        assert_eq!(definitions.len(), 1);
        assert!(definitions[0].contains("lehren"));
        assert!(definitions[0].contains("heute regional"));
        assert!(!definitions[0].contains("Lernen, Abschnitte"));
        assert!(!definitions[0].contains("Johannes"));
    }

    #[test]
    fn german_dictionary_auto_translation_targets_english() {
        assert_eq!(
            translation_target_for_code("de", "auto"),
            Some("en".to_string())
        );
        assert_eq!(translation_target_for_code("de", "de"), None);
    }

    #[test]
    #[ignore = "requires 50 live German Wiktionary lookups"]
    fn german_wiktionary_live_50_words_return_clean_definitions() {
        let words = [
            "Haus",
            "Baum",
            "Wasser",
            "Feuer",
            "Erde",
            "Luft",
            "Mensch",
            "Kind",
            "Frau",
            "Mann",
            "Freund",
            "Familie",
            "Schule",
            "Buch",
            "Sprache",
            "Stadt",
            "Straße",
            "Arbeit",
            "Zeit",
            "Tag",
            "Nacht",
            "Liebe",
            "Glück",
            "Freiheit",
            "Gesundheit",
            "gehen",
            "kommen",
            "sehen",
            "hören",
            "sprechen",
            "lesen",
            "schreiben",
            "essen",
            "trinken",
            "schlafen",
            "arbeiten",
            "lernen",
            "denken",
            "wissen",
            "machen",
            "groß",
            "klein",
            "schnell",
            "langsam",
            "schön",
            "schwierig",
            "möglich",
            "glücklich",
            "freundlich",
            "Überraschung",
        ];
        let mut url = reqwest::Url::parse("https://de.wiktionary.org/w/api.php")
            .expect("German Wiktionary API URL");
        url.query_pairs_mut()
            .append_pair("action", "query")
            .append_pair("prop", "revisions")
            .append_pair("rvprop", "content")
            .append_pair("rvslots", "main")
            .append_pair("titles", &words.join("|"))
            .append_pair("redirects", "1")
            .append_pair("format", "json")
            .append_pair("formatversion", "2");
        let response: serde_json::Value = reqwest::blocking::Client::builder()
            .user_agent("Sonarpad/0.8.1 (German Wiktionary 50-word integration test)")
            .build()
            .expect("German Wiktionary test client")
            .get(url)
            .send()
            .expect("German Wiktionary batch request")
            .error_for_status()
            .expect("successful German Wiktionary batch response")
            .json()
            .expect("German Wiktionary batch JSON");
        let pages = response["query"]["pages"]
            .as_array()
            .expect("German Wiktionary pages array");
        assert_eq!(pages.len(), 50, "expected 50 German Wiktionary pages");
        let mut failures = Vec::new();
        let mut total_definitions = 0usize;

        for page in pages {
            let word = page["title"].as_str().unwrap_or("<missing title>");
            let Some(complete_wikitext) = page["revisions"][0]["slots"]["main"]["content"].as_str()
            else {
                failures.push(format!("{word}: missing live wikitext"));
                continue;
            };
            let mut section_wikitext = String::new();
            let mut in_first_section = false;
            for line in complete_wikitext.lines() {
                let trimmed = line.trim();
                let is_level_two_heading = trimmed.starts_with("== ")
                    && trimmed.ends_with(" ==")
                    && !trimmed.starts_with("===");
                if is_level_two_heading {
                    if in_first_section {
                        break;
                    }
                    in_first_section = true;
                }
                if in_first_section {
                    section_wikitext.push_str(line);
                    section_wikitext.push('\n');
                }
            }
            let definitions =
                extract_definitions_for_language(&section_wikitext, "de", usize::MAX, usize::MAX);
            let synonyms = extract_synonyms_for_language(&section_wikitext, "de", usize::MAX);
            if definitions.is_empty() {
                failures.push(format!("{word}: no definitions"));
                continue;
            }

            for definition in &definitions {
                let lower = definition.to_ascii_lowercase();
                if definition.trim().is_empty()
                    || is_pure_grammar_marker_definition(definition)
                    || definition.contains("{{")
                    || definition.contains("}}")
                    || definition.contains("[[")
                    || definition.contains("]]")
                    || lower.contains("<ref")
                    || lower.contains("wikidata")
                {
                    failures.push(format!("{word}: noisy definition: {definition}"));
                }
            }

            total_definitions += definitions.len();
            eprintln!(
                "{word} | {} definitions | {} synonyms | {}",
                definitions.len(),
                synonyms.len(),
                definitions[0]
            );
        }

        assert!(
            failures.is_empty(),
            "German Wiktionary failures:\n{}",
            failures.join("\n")
        );
        assert!(
            total_definitions >= 50,
            "expected at least one definition per German word"
        );
    }

    #[test]
    fn spanish_hola_keeps_definition_text_inside_template() {
        let wikitext = "==== {{interjección|es}} ====\n;1: {{impropia|Expresión de [[saludo]] utilizada entre dos o más personas}}.\n;2: {{impropia|Expresión de [[sorpresa]]}}.";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert!(defs.len() >= 2);
        assert!(defs[0].contains("Expresión de saludo"));
        assert!(defs[1].contains("Expresión de sorpresa"));
    }

    #[test]
    fn spanish_casa_removes_ref_tags() {
        let wikitext = ";5: {{plm|descendencia}} o [[linaje]] que tiene un mismo apellido.<ref name=\"dlc1914\"></ref>";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs.len(), 1);
        assert!(!defs[0].contains("dlc1914"));
        assert!(!defs[0].contains("<ref"));
    }

    #[test]
    fn spanish_agua_parses_semantic_label_before_colon() {
        let wikitext = ";10 {{csem|astrología}}: Elemento que incluye los signos de [[Cáncer]], [[Escorpio]] y [[Piscis]].";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs.len(), 1);
        assert!(defs[0].starts_with("Elemento que incluye los signos"));
    }

    #[test]
    fn strips_leading_language_code_noise_from_definition() {
        let wikitext = "# {{term|it}} Definizione di prova";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs, vec!["Definizione di prova".to_string()]);
    }

    #[test]
    fn filters_definition_that_is_only_language_code() {
        let wikitext = "# {{term|it}}";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert!(defs.is_empty());
    }

    #[test]
    fn strips_repeated_leading_language_codes() {
        let wikitext = "# it it liquido profumato";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs, vec!["liquido profumato".to_string()]);
    }

    #[test]
    fn strips_trailing_single_letter_noise() {
        let wikitext = "# Non disponibile i";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs, vec!["Non disponibile".to_string()]);
    }

    #[test]
    fn strips_leading_wikidata_qid_noise() {
        let wikitext = "# Q283 uncountable An inorganic compound found as a clear liquid.";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(
            defs,
            vec!["uncountable An inorganic compound found as a clear liquid.".to_string()]
        );
    }

    #[test]
    fn filters_definition_that_is_only_wikidata_qid() {
        let wikitext = "# Q289";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert!(defs.is_empty());
    }

    #[test]
    fn strips_trailing_wikidata_qid_noise() {
        let wikitext = "# Non disponibile Q289";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs, vec!["Non disponibile".to_string()]);
    }

    #[test]
    fn strips_leading_wikidata_qid_with_suffix_noise() {
        let wikitext = "# Q42569A South American mammal.";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs, vec!["South American mammal.".to_string()]);
    }

    #[test]
    fn filters_single_related_lemma_from_subpoint() {
        let wikitext = ";11: [[aguar]].";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert!(defs.is_empty());
    }

    #[test]
    fn filters_spanish_bibliographic_noise_from_subpoint() {
        let wikitext = ";3: salva Pág. 775";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert!(defs.is_empty());
    }

    #[test]
    fn strips_trailing_date_noise() {
        let wikitext = "# definición útil 2012-9-05";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs, vec!["definición útil".to_string()]);
    }

    #[test]
    fn strips_trailing_date_noise_without_space() {
        let wikitext = "# origen.2012-9-05";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs, vec!["origen".to_string()]);
    }

    #[test]
    fn filters_single_lemma_noise_after_valid_definition() {
        let wikitext = "# Definición válida extensa.\n# lama.";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs, vec!["Definición válida extensa.".to_string()]);
    }

    #[test]
    fn drops_grammar_template_noise_from_definitions() {
        let wikitext = "# {{w|f|sing|case}}\n# abitazione";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs, vec!["abitazione".to_string()]);
    }

    #[test]
    fn drops_leading_raw_grammar_markers_before_definition() {
        let wikitext = "# w f sing case abitazione";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs, vec!["abitazione".to_string()]);
    }

    #[test]
    fn filters_definition_when_only_raw_grammar_markers_are_available() {
        let wikitext = "# w f sing case";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert!(defs.is_empty());
    }

    #[test]
    fn recognizes_pure_grammar_marker_definition() {
        assert!(is_pure_grammar_marker_definition("w f sing case"));
    }

    #[test]
    fn does_not_treat_normal_definition_as_grammar_marker() {
        assert!(!is_pure_grammar_marker_definition("abitazione"));
    }

    #[test]
    fn italian_casa_extracts_real_definitions_from_hash_star_lines() {
        let wikitext = "# {{Pn|w}} ''f sing'' {{Linkp|case}}\n#* {{Term|architettura|it}} [[edificio]] [[costruito]] [[per]] [[essere]] utilizzato [[come]] [[abitazione]]\n#* [[dimora]] di una [[persona]]";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs.len(), 2);
        assert!(defs[0].contains("edificio"));
        assert!(defs[1].contains("dimora"));
    }

    #[test]
    fn real_wiktionary_fixtures_extract_definitions_word_by_word() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("wiktionary");
        let index = fs::read_to_string(root.join("index.csv"))
            .expect("Unable to read tests/fixtures/wiktionary/index.csv");
        let mut checked = 0usize;

        for line in index.lines().skip(1) {
            if line.trim().is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(',').collect();
            assert!(parts.len() == 3, "Malformed fixture index row: {line}");
            let lang = parts[0].trim();
            let word = parts[1].trim();
            let rel_file = parts[2].trim();
            let wikitext = fs::read_to_string(root.join(rel_file))
                .unwrap_or_else(|_| panic!("Missing fixture file for {lang}:{word} => {rel_file}"));
            let defs = extract_definitions_with_subpoints(&wikitext, usize::MAX, usize::MAX);
            assert!(
                !defs.is_empty(),
                "No definitions extracted for real fixture {lang}:{word}"
            );
            assert!(
                defs.iter().any(|d| !is_pure_grammar_marker_definition(d)),
                "Only grammar markers extracted for real fixture {lang}:{word}: {defs:?}"
            );
            checked += 1;
        }

        assert_eq!(checked, 60, "Expected 60 real dictionary fixtures");
    }

    #[test]
    fn strips_compact_leading_qid_noise() {
        let wikitext = "# Q235544The visible part of fire.";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(defs, vec!["The visible part of fire.".to_string()]);
    }

    #[test]
    fn strips_trailing_hex_color_noise() {
        let wikitext = "# A brilliant reddish orange-gold fiery colour. E82D14";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert_eq!(
            defs,
            vec!["A brilliant reddish orange-gold fiery colour.".to_string()]
        );
    }

    #[test]
    fn filters_grammar_noise_entry() {
        let wikitext = "# ing-form";
        let defs = extract_definitions_with_subpoints(wikitext, usize::MAX, usize::MAX);
        assert!(defs.is_empty());
    }

    #[test]
    fn removes_spanish_infinitive_noise_when_other_definitions_exist() {
        let mut defs = vec![
            "saludar".to_string(),
            "Fórmula social de comienzo de una conversación.".to_string(),
        ];
        super::remove_spanish_infinitive_noise_when_contextual(&mut defs);
        assert_eq!(
            defs,
            vec!["Fórmula social de comienzo de una conversación.".to_string()]
        );
    }
}
