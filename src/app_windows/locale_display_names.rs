use crate::settings::Language;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

// Generated from Unicode CLDR locale display-name data. The JSON is embedded in
// the executable so language and territory names never depend on an online
// translation service.
const DISPLAY_NAMES_JSON: &str = include_str!("locale_display_names.json");
// Indonesian display names: Unicode CLDR 47.0.0, cldr-localenames-full/main/id.
const LANGUAGE_CODE_ALIASES_JSON: &str = include_str!("language_code_aliases.json");
const LANGUAGE_NAME_ALIASES_JSON: &str = include_str!("language_name_aliases.json");

#[derive(Debug, Deserialize)]
struct LocaleNames {
    languages: HashMap<String, String>,
    territories: HashMap<String, String>,
}

type DisplayNames = HashMap<String, LocaleNames>;

fn display_names() -> &'static DisplayNames {
    static DATA: OnceLock<DisplayNames> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(DISPLAY_NAMES_JSON).unwrap_or_default())
}

fn language_code_aliases() -> &'static HashMap<String, String> {
    static DATA: OnceLock<HashMap<String, String>> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(LANGUAGE_CODE_ALIASES_JSON).unwrap_or_default())
}

fn language_name_aliases() -> &'static HashMap<String, String> {
    static DATA: OnceLock<HashMap<String, String>> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(LANGUAGE_NAME_ALIASES_JSON).unwrap_or_default())
}

pub fn app_locale(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::German => "de",
        Language::English => "en",
        Language::Spanish => "es",
        Language::Portuguese => "pt",
        Language::PortugueseBrazilian => "pt",
        Language::Swedish => "sv",
        Language::Vietnamese => "vi",
        Language::Czech => "cs",
        Language::Polish => "pl",
        Language::French => "fr",
        Language::Serbian => "sr",
        Language::Ukrainian => "uk",
        Language::Lithuanian => "lt",
        Language::Russian => "ru",
        Language::Chinese => "zh",
        Language::Hindi => "hi",
        Language::Indonesian => "id",
    }
}

pub fn language_name(language: Language, code: &str) -> Option<String> {
    language_name_for_locale(app_locale(language), code)
}

pub fn language_name_for_locale(display_locale: &str, code: &str) -> Option<String> {
    let code = canonical_language_code(code);
    if code.is_empty() {
        return None;
    }
    let base = code.split('-').next().unwrap_or(&code);
    lookup_language(display_locale, &code)
        .or_else(|| {
            (base != code)
                .then(|| lookup_language(display_locale, base))
                .flatten()
        })
        .or_else(|| lookup_language("en", &code))
        .or_else(|| {
            (base != code)
                .then(|| lookup_language("en", base))
                .flatten()
        })
        .cloned()
}

pub fn territory_name(language: Language, code: &str) -> Option<String> {
    let code = code.trim().to_ascii_uppercase();
    if code.is_empty() {
        return None;
    }
    let locale = app_locale(language);
    display_names()
        .get(locale)
        .and_then(|names| names.territories.get(&code))
        .or_else(|| {
            display_names()
                .get("en")
                .and_then(|names| names.territories.get(&code))
        })
        .cloned()
}

pub fn language_code_from_name(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let direct = canonical_language_code(value);
    if is_known_language_code(&direct) {
        return Some(direct);
    }

    let phrase_key = normalized_phrase_key(value);
    if let Some(code) = language_name_aliases().get(&phrase_key) {
        return Some(canonical_language_code(code));
    }

    let aliases = language_aliases();
    aliases
        .get(&normalized_lookup_key(value))
        .or_else(|| {
            let ascii = ascii_lookup_key(value);
            (!ascii.is_empty()).then(|| aliases.get(&ascii)).flatten()
        })
        .cloned()
}

pub fn language_codes_from_catalog_label(value: &str) -> Vec<String> {
    let cleaned = clean_catalog_language_label(value);
    if cleaned.is_empty() || is_generic_catalog_label(&cleaned) {
        return Vec::new();
    }

    if let Some(code) = language_code_from_name(&cleaned) {
        return vec![code];
    }

    let tokens = cleaned
        .split_whitespace()
        .filter(|token| !is_language_connector(token))
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return Vec::new();
    }

    segment_language_tokens(&tokens).unwrap_or_default()
}

pub fn english_language_name(code: &str) -> Option<String> {
    language_name_for_locale("en", code)
}

pub fn is_known_language_code(code: &str) -> bool {
    let code = canonical_language_code(code);
    !code.is_empty()
        && display_names()
            .get("en")
            .is_some_and(|names| names.languages.contains_key(&code))
}

fn lookup_language(display_locale: &str, code: &str) -> Option<&'static String> {
    display_names()
        .get(display_locale)
        .and_then(|names| names.languages.get(code))
}

fn normalize_language_code(code: &str) -> String {
    code.trim().replace('_', "-").to_lowercase()
}

fn canonical_language_code(code: &str) -> String {
    let normalized = normalize_language_code(code);
    if normalized.is_empty() {
        return normalized;
    }
    let mut parts = normalized.splitn(2, '-');
    let base = parts.next().unwrap_or_default();
    let canonical_base = language_code_aliases()
        .get(base)
        .map(String::as_str)
        .unwrap_or(base);
    match parts.next() {
        Some(suffix) if !suffix.is_empty() => format!("{canonical_base}-{suffix}"),
        _ => canonical_base.to_string(),
    }
}

fn clean_catalog_language_label(value: &str) -> String {
    let normalized = normalized_phrase_key(value);
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    let mut start = 0;

    while tokens
        .get(start)
        .is_some_and(|token| token.chars().all(|ch| ch.is_ascii_digit()))
    {
        start += 1;
    }

    while tokens
        .get(start)
        .is_some_and(|token| is_language_prefix(token))
    {
        start += 1;
    }

    tokens[start..].join(" ")
}

fn segment_language_tokens(tokens: &[&str]) -> Option<Vec<String>> {
    let mut paths = vec![None::<Vec<String>>; tokens.len() + 1];
    paths[0] = Some(Vec::new());

    for start in 0..tokens.len() {
        let Some(current) = paths[start].clone() else {
            continue;
        };

        if is_language_connector(tokens[start]) {
            if paths[start + 1].is_none() {
                paths[start + 1] = Some(current);
            }
            continue;
        }

        for end in (start + 1..=tokens.len()).rev() {
            let candidate = tokens[start..end].join(" ");
            let Some(code) = language_code_from_name(&candidate) else {
                continue;
            };
            let mut next = current.clone();
            if !next.contains(&code) {
                next.push(code);
            }
            if paths[end].is_none() {
                paths[end] = Some(next);
            }
        }
    }

    paths.pop().flatten().filter(|codes| !codes.is_empty())
}

fn is_language_prefix(value: &str) -> bool {
    matches!(
        value,
        "language"
            | "languages"
            | "lingua"
            | "lingue"
            | "idioma"
            | "idiomas"
            | "langue"
            | "langues"
            | "sprache"
            | "sprachen"
            | "язык"
            | "языки"
            | "język"
            | "jezyk"
            | "jazyky"
            | "jazyk"
            | "linguagem"
            | "língua"
            | "línguas"
            | "linguas"
            | "dil"
            | "diller"
            | "لغة"
            | "اللغة"
            | "زبان"
            | "भाषा"
    )
}

fn is_language_connector(value: &str) -> bool {
    matches!(
        value,
        "and"
            | "or"
            | "e"
            | "o"
            | "y"
            | "u"
            | "et"
            | "ou"
            | "und"
            | "oder"
            | "и"
            | "или"
            | "a"
            | "i"
            | "và"
            | "hoặc"
            | "و"
            | "और"
    )
}

fn is_generic_catalog_label(value: &str) -> bool {
    matches!(
        value,
        "unknown"
            | "unknown language"
            | "unknown languages"
            | "other language"
            | "other languages"
            | "additional language"
            | "additional languages"
            | "addtional language"
            | "addtional languages"
            | "various languages"
            | "multiple languages"
            | "multilingual"
            | "неизвестен"
            | "неизвестный"
            | "неизвестно"
    )
}

fn language_aliases() -> &'static HashMap<String, String> {
    static ALIASES: OnceLock<HashMap<String, String>> = OnceLock::new();
    ALIASES.get_or_init(|| {
        let mut aliases = HashMap::new();

        // Use a fixed locale order so ambiguous names always resolve in the
        // same way. English comes first because Radio Browser most commonly
        // returns English display names; the remaining Sonarpad locales add
        // their translated forms without overwriting earlier matches.
        for locale_code in [
            "en", "it", "es", "pt", "sv", "vi", "cs", "pl", "fr", "sr", "uk", "lt", "ru", "zh",
            "hi",
        ] {
            let Some(locale) = display_names().get(locale_code) else {
                continue;
            };
            let mut entries = locale.languages.iter().collect::<Vec<_>>();
            entries.sort_by(|(left_code, _), (right_code, _)| {
                left_code
                    .contains('-')
                    .cmp(&right_code.contains('-'))
                    .then_with(|| left_code.len().cmp(&right_code.len()))
                    .then_with(|| left_code.cmp(right_code))
            });
            for (code, name) in entries {
                insert_alias(&mut aliases, code, code);
                insert_alias(&mut aliases, name, code);
            }
        }

        // Common catalogue spellings and historical names not always used as
        // the canonical CLDR display name.
        for (alias, code) in [
            ("farsi", "fa"),
            ("persian", "fa"),
            ("mandarin", "zh"),
            ("mandarin chinese", "zh"),
            ("simplified chinese", "zh-hans"),
            ("traditional chinese", "zh-hant"),
            ("brazilian portuguese", "pt-br"),
            ("portuguese brazil", "pt-br"),
            ("american english", "en-us"),
            ("british english", "en-gb"),
            ("castilian", "es"),
            ("serbo croatian", "sh"),
            ("burmese", "my"),
            ("moldavian", "ro-md"),
            ("norwegian bokmal", "nb"),
            ("norwegian nynorsk", "nn"),
            ("tagalog", "tl"),
        ] {
            insert_alias(&mut aliases, alias, code);
        }
        aliases
    })
}

fn insert_alias(aliases: &mut HashMap<String, String>, alias: &str, code: &str) {
    let code = canonical_language_code(code);
    let unicode_key = normalized_lookup_key(alias);
    if !unicode_key.is_empty() {
        aliases.entry(unicode_key).or_insert_with(|| code.clone());
    }
    let ascii_key = ascii_lookup_key(alias);
    if !ascii_key.is_empty() {
        aliases.entry(ascii_key).or_insert(code);
    }
}

fn normalized_phrase_key(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalized_lookup_key(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

fn ascii_lookup_key(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter_map(|ch| match ch {
            'a'..='z' | '0'..='9' => Some(ch),
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => Some('a'),
            'ç' | 'ć' | 'č' | 'ĉ' | 'ċ' => Some('c'),
            'ď' | 'đ' => Some('d'),
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => Some('e'),
            'ğ' | 'ĝ' | 'ġ' | 'ģ' => Some('g'),
            'ĥ' | 'ħ' => Some('h'),
            'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => Some('i'),
            'ĵ' => Some('j'),
            'ķ' => Some('k'),
            'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => Some('l'),
            'ñ' | 'ń' | 'ņ' | 'ň' | 'ŉ' | 'ŋ' => Some('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => Some('o'),
            'ŕ' | 'ŗ' | 'ř' => Some('r'),
            'ś' | 'ŝ' | 'ş' | 'š' | 'ș' => Some('s'),
            'ţ' | 'ť' | 'ŧ' | 'ț' => Some('t'),
            'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => Some('u'),
            'ŵ' => Some('w'),
            'ý' | 'ÿ' | 'ŷ' => Some('y'),
            'ź' | 'ż' | 'ž' => Some('z'),
            'æ' => Some('a'),
            'œ' => Some('o'),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localizes_common_and_uncommon_languages() {
        assert_eq!(
            language_name(Language::Italian, "de").as_deref(),
            Some("tedesco")
        );
        assert_eq!(
            language_name(Language::Italian, "ace").as_deref(),
            Some("accinese")
        );
        assert_eq!(
            language_name(Language::Hindi, "af").as_deref(),
            Some("अफ़्रीकी")
        );
        assert_eq!(
            language_name(Language::Chinese, "uk").as_deref(),
            Some("乌克兰语")
        );
        assert_eq!(
            language_name(Language::Italian, "en-US").as_deref(),
            Some("inglese (Stati Uniti)")
        );
    }

    #[test]
    fn resolves_language_names_and_variants_to_codes() {
        assert_eq!(language_code_from_name("Afrikaans").as_deref(), Some("af"));
        assert_eq!(language_code_from_name("Türkçe").as_deref(), Some("tr"));
        assert_eq!(
            language_code_from_name("American English").as_deref(),
            Some("en-us")
        );
        assert_eq!(language_code_from_name("mandarin").as_deref(), Some("zh"));
        assert_eq!(language_code_from_name("eng").as_deref(), Some("en"));
        assert_eq!(language_code_from_name("ger").as_deref(), Some("de"));
        assert_eq!(language_code_from_name("zho").as_deref(), Some("zh"));
    }

    #[test]
    fn recognizes_native_foreign_and_combined_catalogue_labels() {
        assert_eq!(
            language_codes_from_catalog_label("Английский Русский"),
            vec!["en".to_string(), "ru".to_string()]
        );
        assert_eq!(
            language_codes_from_catalog_label("Язык: Русский Английский"),
            vec!["ru".to_string(), "en".to_string()]
        );
        assert_eq!(
            language_codes_from_catalog_label("Беларуская"),
            vec!["be".to_string()]
        );
        assert_eq!(
            language_codes_from_catalog_label("ภาษาไทย"),
            vec!["th".to_string()]
        );
        assert_eq!(
            language_codes_from_catalog_label("عربي"),
            vec!["ar".to_string()]
        );
        assert_eq!(
            language_codes_from_catalog_label("128 Brazilian Portuguese"),
            vec!["pt-br".to_string()]
        );
        assert_eq!(
            language_codes_from_catalog_label("Молдавский"),
            vec!["ro-md".to_string()]
        );
    }

    #[test]
    fn rejects_catalogue_values_that_are_not_languages() {
        for value in [
            "#japan",
            "+7 Languages",
            "10 Additional Languages",
            "111",
            "80s",
            "Aboriginal Languages",
            "Afghan",
            "音乐",
            "中国",
            "Неизвестен",
        ] {
            assert!(
                language_codes_from_catalog_label(value).is_empty(),
                "{value}"
            );
        }
    }

    #[test]
    fn embedded_database_covers_every_sonarpad_interface_language() {
        for language in [
            Language::Italian,
            Language::English,
            Language::Spanish,
            Language::Portuguese,
            Language::PortugueseBrazilian,
            Language::Swedish,
            Language::Vietnamese,
            Language::Czech,
            Language::Polish,
            Language::French,
            Language::Serbian,
            Language::Ukrainian,
            Language::Lithuanian,
            Language::Russian,
            Language::Chinese,
            Language::Hindi,
            Language::Indonesian,
        ] {
            assert!(language_name(language, "ace").is_some());
            assert!(language_name(language, "en-us").is_some());
            assert!(language_name(language, "pt-br").is_some());
            assert!(territory_name(language, "IT").is_some());
            assert!(territory_name(language, "DE").is_some());
            assert!(territory_name(language, "US").is_some());
        }
    }

    #[test]
    fn localizes_territories_in_every_supported_script() {
        assert_eq!(
            territory_name(Language::Italian, "DE").as_deref(),
            Some("Germania")
        );
        assert_eq!(
            territory_name(Language::Serbian, "DE").as_deref(),
            Some("Немачка")
        );
        assert_eq!(
            territory_name(Language::Vietnamese, "IT").as_deref(),
            Some("Italy")
        );
    }
}

#[cfg(test)]
mod indonesian_tests {
    use super::*;

    #[test]
    fn indonesian_language_and_country_names_are_embedded() {
        assert_eq!(
            language_name(Language::Indonesian, "en").as_deref(),
            Some("Inggris")
        );
        assert_eq!(
            language_name(Language::Indonesian, "de").as_deref(),
            Some("Jerman")
        );
        assert_eq!(
            territory_name(Language::Indonesian, "US").as_deref(),
            Some("Amerika Serikat")
        );
    }
}
