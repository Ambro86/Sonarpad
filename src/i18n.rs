use crate::settings::Language;
use std::collections::HashMap;
use std::sync::OnceLock;

const EN_JSON: &str = include_str!("../i18n/en.json");
const DE_JSON: &str = include_str!("../i18n/de.json");
const IT_JSON: &str = include_str!("../i18n/it.json");
const ES_JSON: &str = include_str!("../i18n/es.json");
const PT_JSON: &str = include_str!("../i18n/pt.json");
const PT_BR_JSON: &str = include_str!("../i18n/pt-BR.json");
const SV_JSON: &str = include_str!("../i18n/sv.json");
const VI_JSON: &str = include_str!("../i18n/vi.json");
const CS_JSON: &str = include_str!("../i18n/cs.json");
const PL_JSON: &str = include_str!("../i18n/pl.json");
const FR_JSON: &str = include_str!("../i18n/fr.json");
const SR_JSON: &str = include_str!("../i18n/sr.json");
const UK_JSON: &str = include_str!("../i18n/uk.json");
const LT_JSON: &str = include_str!("../i18n/lt.json");
const RU_JSON: &str = include_str!("../i18n/ru.json");
const ZH_JSON: &str = include_str!("../i18n/zh.json");
const ID_JSON: &str = include_str!("../i18n/id.json");
const HI_JSON: &str = include_str!("../i18n/hi.json");
const TRECCANI_IT_JSON: &str = include_str!("../i18n/features/treccani_it.json");
const TV_IT_JSON: &str = include_str!("../i18n/features/tv/it.json");
const LA7_PLAY_IT_JSON: &str = include_str!("../i18n/features/la7_play/it.json");

fn load_map(raw: &str) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = serde_json::from_str(raw).unwrap_or_default();
    for value in map.values_mut() {
        if value.contains("\\n") {
            *value = value.replace("\\n", "\n");
        }
    }
    map
}

fn load_sv_map() -> HashMap<String, String> {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let override_path = dir.join("sv.json");
            if override_path.exists() {
                match std::fs::read_to_string(&override_path) {
                    Ok(content) => {
                        match serde_json::from_str::<HashMap<String, String>>(&content) {
                            Ok(mut map) => {
                                for value in map.values_mut() {
                                    if value.contains("\\n") {
                                        *value = value.replace("\\n", "\n");
                                    }
                                }
                                return map;
                            }
                            Err(err) => {
                                crate::log_debug(&format!(
                                    "Failed to parse Swedish translation override: {err}"
                                ));
                            }
                        }
                    }
                    Err(err) => {
                        crate::log_debug(&format!(
                            "Failed to read Swedish translation override: {err}"
                        ));
                    }
                }
            }
        }
    } else {
        crate::log_debug("Failed to resolve executable path for Swedish translation override.");
    }
    load_map(SV_JSON)
}

fn load_cs_map() -> HashMap<String, String> {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let override_path = dir.join("cs.json");
            if override_path.exists() {
                match std::fs::read_to_string(&override_path) {
                    Ok(content) => {
                        match serde_json::from_str::<HashMap<String, String>>(&content) {
                            Ok(mut map) => {
                                for value in map.values_mut() {
                                    if value.contains("\\n") {
                                        *value = value.replace("\\n", "\n");
                                    }
                                }
                                return map;
                            }
                            Err(err) => {
                                crate::log_debug(&format!(
                                    "Failed to parse Czech translation override: {err}"
                                ));
                            }
                        }
                    }
                    Err(err) => {
                        crate::log_debug(&format!(
                            "Failed to read Czech translation override: {err}"
                        ));
                    }
                }
            }
        }
    } else {
        crate::log_debug("Failed to resolve executable path for Czech translation override.");
    }
    load_map(CS_JSON)
}

fn load_pl_map() -> HashMap<String, String> {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let override_path = dir.join("pl.json");
            if override_path.exists() {
                match std::fs::read_to_string(&override_path) {
                    Ok(content) => {
                        match serde_json::from_str::<HashMap<String, String>>(&content) {
                            Ok(mut map) => {
                                for value in map.values_mut() {
                                    if value.contains("\\n") {
                                        *value = value.replace("\\n", "\n");
                                    }
                                }
                                return map;
                            }
                            Err(err) => {
                                crate::log_debug(&format!(
                                    "Failed to parse Polish translation override: {err}"
                                ));
                            }
                        }
                    }
                    Err(err) => {
                        crate::log_debug(&format!(
                            "Failed to read Polish translation override: {err}"
                        ));
                    }
                }
            }
        }
    } else {
        crate::log_debug("Failed to resolve executable path for Polish translation override.");
    }
    load_map(PL_JSON)
}

fn load_fr_map() -> HashMap<String, String> {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let override_path = dir.join("fr.json");
            if override_path.exists() {
                match std::fs::read_to_string(&override_path) {
                    Ok(content) => {
                        match serde_json::from_str::<HashMap<String, String>>(&content) {
                            Ok(mut map) => {
                                for value in map.values_mut() {
                                    if value.contains("\\n") {
                                        *value = value.replace("\\n", "\n");
                                    }
                                }
                                return map;
                            }
                            Err(err) => {
                                crate::log_debug(&format!(
                                    "Failed to parse French translation override: {err}"
                                ));
                            }
                        }
                    }
                    Err(err) => {
                        crate::log_debug(&format!(
                            "Failed to read French translation override: {err}"
                        ));
                    }
                }
            }
        }
    } else {
        crate::log_debug("Failed to resolve executable path for French translation override.");
    }
    load_map(FR_JSON)
}

fn map_for_language(language: Language) -> &'static HashMap<String, String> {
    static EN: OnceLock<HashMap<String, String>> = OnceLock::new();
    static DE: OnceLock<HashMap<String, String>> = OnceLock::new();
    static IT: OnceLock<HashMap<String, String>> = OnceLock::new();
    static ES: OnceLock<HashMap<String, String>> = OnceLock::new();
    static PT: OnceLock<HashMap<String, String>> = OnceLock::new();
    static PT_BR: OnceLock<HashMap<String, String>> = OnceLock::new();
    static SV: OnceLock<HashMap<String, String>> = OnceLock::new();
    static VI: OnceLock<HashMap<String, String>> = OnceLock::new();
    static CS: OnceLock<HashMap<String, String>> = OnceLock::new();
    static PL: OnceLock<HashMap<String, String>> = OnceLock::new();
    static FR: OnceLock<HashMap<String, String>> = OnceLock::new();
    static SR: OnceLock<HashMap<String, String>> = OnceLock::new();
    static UK: OnceLock<HashMap<String, String>> = OnceLock::new();
    static LT: OnceLock<HashMap<String, String>> = OnceLock::new();
    static RU: OnceLock<HashMap<String, String>> = OnceLock::new();
    static ZH: OnceLock<HashMap<String, String>> = OnceLock::new();
    static ID: OnceLock<HashMap<String, String>> = OnceLock::new();
    static HI: OnceLock<HashMap<String, String>> = OnceLock::new();
    match language {
        Language::Italian => IT.get_or_init(|| load_map(IT_JSON)),
        Language::Spanish => ES.get_or_init(|| load_map(ES_JSON)),
        Language::Portuguese => PT.get_or_init(|| load_map(PT_JSON)),
        Language::PortugueseBrazilian => PT_BR.get_or_init(|| load_map(PT_BR_JSON)),
        Language::Swedish => SV.get_or_init(load_sv_map),
        Language::Vietnamese => VI.get_or_init(|| load_map(VI_JSON)),
        Language::Czech => CS.get_or_init(load_cs_map),
        Language::Polish => PL.get_or_init(load_pl_map),
        Language::French => FR.get_or_init(load_fr_map),
        Language::Serbian => SR.get_or_init(|| load_map(SR_JSON)),
        Language::Ukrainian => UK.get_or_init(|| load_map(UK_JSON)),
        Language::Lithuanian => LT.get_or_init(|| load_map(LT_JSON)),
        Language::Russian => RU.get_or_init(|| load_map(RU_JSON)),
        Language::Chinese => ZH.get_or_init(|| load_map(ZH_JSON)),
        Language::Hindi => HI.get_or_init(|| load_map(HI_JSON)),
        Language::Indonesian => ID.get_or_init(|| load_map(ID_JSON)),
        Language::German => DE.get_or_init(|| load_map(DE_JSON)),
        Language::English => EN.get_or_init(|| load_map(EN_JSON)),
    }
}

pub fn tr(language: Language, key: &str) -> String {
    map_for_language(language)
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

fn split_dialog_filter_fields(raw: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\0' {
            fields.push(std::mem::take(&mut current));
            continue;
        }

        if ch == '\\' {
            let mut slash_count = 1usize;
            while chars.peek() == Some(&'\\') {
                let _slash = chars.next();
                slash_count += 1;
            }
            if chars.peek() == Some(&'0') {
                let _zero = chars.next();
                fields.push(std::mem::take(&mut current));
                continue;
            }
            current.extend(std::iter::repeat_n('\\', slash_count));
            continue;
        }

        current.push(ch);
    }
    fields.push(current);
    fields
}

pub(crate) fn parse_dialog_filter_pairs(raw: &str) -> Vec<(String, String)> {
    let fields = split_dialog_filter_fields(raw);
    let mut pairs = Vec::new();
    let mut fields = fields.into_iter();

    while let (Some(name), Some(pattern)) = (fields.next(), fields.next()) {
        if name.is_empty() || pattern.is_empty() {
            break;
        }
        pairs.push((name, pattern));
    }

    pairs
}

pub(crate) fn dialog_filter_pairs(language: Language, key: &str) -> Vec<(String, String)> {
    let raw = tr(language, key);
    let pairs = parse_dialog_filter_pairs(&raw);
    if !pairs.is_empty() {
        return pairs;
    }

    crate::log_debug(&format!(
        "Invalid localized file-dialog filter for key={key}; using all-files fallback"
    ));
    vec![(tr(language, "dialog.all_files"), "*.*".to_string())]
}

pub fn tr_treccani(key: &str) -> String {
    static TRECCANI_IT: OnceLock<HashMap<String, String>> = OnceLock::new();
    TRECCANI_IT
        .get_or_init(|| load_map(TRECCANI_IT_JSON))
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

pub fn tr_tv(key: &str) -> String {
    static TV_IT: OnceLock<HashMap<String, String>> = OnceLock::new();
    TV_IT
        .get_or_init(|| load_map(TV_IT_JSON))
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

fn replace_named_args(mut text: String, args: &[(&str, &str)]) -> String {
    for (name, value) in args {
        let token = format!("{{{name}}}");
        text = text.replace(&token, value);
    }
    text
}

pub fn tr_tv_f(key: &str, args: &[(&str, &str)]) -> String {
    replace_named_args(tr_tv(key), args)
}

pub fn tr_la7_play(key: &str) -> String {
    static LA7_PLAY_IT: OnceLock<HashMap<String, String>> = OnceLock::new();
    LA7_PLAY_IT
        .get_or_init(|| load_map(LA7_PLAY_IT_JSON))
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

pub fn tr_la7_play_f(key: &str, args: &[(&str, &str)]) -> String {
    replace_named_args(tr_la7_play(key), args)
}

pub fn tr_f(language: Language, key: &str, args: &[(&str, &str)]) -> String {
    replace_named_args(tr(language, key), args)
}

#[cfg(test)]
mod tests {
    #[test]
    fn indonesian_settings_roundtrip_loads_embedded_translation() {
        let language: crate::settings::Language = serde_json::from_str("\"id\"").unwrap();
        assert_eq!(language, crate::settings::Language::Indonesian);
        assert_eq!(serde_json::to_string(&language).unwrap(), "\"id\"");
        let source: std::collections::HashMap<String, String> =
            serde_json::from_str(super::ID_JSON).unwrap();
        assert_eq!(
            super::tr(language, "options.cancel"),
            source["options.cancel"]
        );
        assert_ne!(
            super::tr(language, "options.cancel"),
            super::tr(crate::settings::Language::English, "options.cancel")
        );
    }

    use super::{dialog_filter_pairs, parse_dialog_filter_pairs};
    use crate::settings::Language;

    #[test]
    fn dialog_filters_accept_literal_backslash_zero_separators() {
        assert_eq!(
            parse_dialog_filter_pairs(r"Text (*.txt)\0*.txt\0All files (*.*)\0*.*\0\0"),
            vec![
                ("Text (*.txt)".to_string(), "*.txt".to_string()),
                ("All files (*.*)".to_string(), "*.*".to_string()),
            ]
        );
    }

    #[test]
    fn dialog_filters_accept_embedded_nul_separators() {
        assert_eq!(
            parse_dialog_filter_pairs("Text (*.txt)\0*.txt\0All files (*.*)\0*.*\0\0"),
            vec![
                ("Text (*.txt)".to_string(), "*.txt".to_string()),
                ("All files (*.*)".to_string(), "*.*".to_string()),
            ]
        );
    }

    #[test]
    fn dialog_filters_accept_overescaped_separators() {
        assert_eq!(
            parse_dialog_filter_pairs(
                r"Text (*.txt)\\\\\0*.txt\\\\\0All files (*.*)\\\\\0*.*\\\\\0\\\\\0"
            ),
            vec![
                ("Text (*.txt)".to_string(), "*.txt".to_string()),
                ("All files (*.*)".to_string(), "*.*".to_string()),
            ]
        );
    }

    #[test]
    fn open_filter_lists_kindle_and_daisy_in_every_interface_language() {
        let languages = [
            Language::Italian,
            Language::English,
            Language::German,
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
        ];
        for language in languages {
            let pairs = dialog_filter_pairs(language, "dialog.open_filter");
            assert_eq!(pairs.get(1).map(|pair| pair.1.as_str()), Some("*.txt"));
            assert!(pairs.iter().any(|pair| pair.1 == "*.mobi;*.azw;*.azw3"));
            assert!(
                pairs
                    .iter()
                    .any(|pair| pair.1 == "*.daisy;*.opf;*.ncx;*.smil;*.xml;*.zip")
            );
        }
    }

    #[test]
    fn dialog_filters_ignore_incomplete_trailing_pair() {
        assert_eq!(
            parse_dialog_filter_pairs(r"Text (*.txt)\0*.txt\0Incomplete"),
            vec![("Text (*.txt)".to_string(), "*.txt".to_string())]
        );
    }
}
