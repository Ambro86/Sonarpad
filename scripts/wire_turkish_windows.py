#!/usr/bin/env python3
"""Connect Turkish to the Windows Rust language framework (idempotent, assert anchors)."""
from pathlib import Path
import re
ROOT=Path(__file__).resolve().parents[1]
changes=[]
def load(path):
    return (ROOT/path).read_text(encoding="utf-8")
def save(path,old,new):
    if old!=new:
        (ROOT/path).write_text(new,encoding="utf-8")
        changes.append(path)
def once(s,old,new,path):
    if new in s:return s
    if old not in s:raise RuntimeError(f"Missing anchor in {path}: {old[:100]}")
    return s.replace(old,new,1)

p="src/settings.rs";s=load(p);o=s
s=once(s,'    Indonesian,\n}', '    Indonesian,\n    #[serde(rename = "tr")]\n    Turkish,\n}',p)
s=once(s,'        if lower.starts_with("id") {','        if lower.starts_with("tr") {\n            return Language::Turkish;\n        }\n        if lower.starts_with("id") {',p)
save(p,o,s)

p="src/i18n.rs";s=load(p);o=s
s=once(s,'const ID_JSON: &str = include_str!("../i18n/id.json");','const ID_JSON: &str = include_str!("../i18n/id.json");\nconst TR_JSON: &str = include_str!("../i18n/tr.json");',p)
s=once(s,'    static ID: OnceLock<HashMap<String, String>> = OnceLock::new();','    static ID: OnceLock<HashMap<String, String>> = OnceLock::new();\n    static TR: OnceLock<HashMap<String, String>> = OnceLock::new();',p)
s=once(s,'        Language::Indonesian => ID.get_or_init(|| load_map(ID_JSON)),','        Language::Indonesian => ID.get_or_init(|| load_map(ID_JSON)),\n        Language::Turkish => TR.get_or_init(|| load_map(TR_JSON)),',p)
# Add to language array tests, but avoid changing intentionally Indonesian-specific assertions.
s=once(s,'            Language::Indonesian,\n        ]','            Language::Indonesian,\n            Language::Turkish,\n        ]',p)
save(p,o,s)

# For each exhaustive Language match add Turkish, while retaining old branch behavior for
# cases that currently share a category or formatting with Indonesian.
for p0 in (ROOT/"src").rglob("*.rs"):
    p=p0.relative_to(ROOT).as_posix()
    if p in {"src/settings.rs","src/i18n.rs"}:continue
    s=load(p);original=s
    if 'Language::Turkish' in s:
        continue  # Already wired on an earlier partial execution.
    # Turkish belongs to the default group where Indonesian was grouped.
    lines=s.splitlines(keepends=True)
    out=[]
    for line in lines:
        out.append(line)
        if "Language::Indonesian" in line and re.match(r"^\s*\|\s*(?:crate::settings::)?Language::Indonesian\s*$",line.strip("\n")):
            indent=re.match(r"^\s*",line).group()
            prefix="crate::settings::" if "crate::settings::Language::" in line else ""
            out.append(f"{indent}| {prefix}Language::Turkish\n")
            continue
        m=re.match(r"^(\s*)((?:crate::settings::)?Language::Indonesian)\s*=>\s*(.+?)(\r?\n)?$",line)
        if m:
            indent,_,rhs,_=m.groups()
            if p=="src/app_windows/options_window.rs" and rhs.startswith("17"):
                rhs=rhs.replace("17","18",1)
            rhs=(rhs.replace("WEEKDAYS_ID","WEEKDAYS_TR").replace("MONTHS_ID","MONTHS_TR")
                 .replace('"id-ID"','"tr-TR"').replace('"id"','"tr"').replace('"ID"','"TR"')
                 .replace('"Indonesian"','"Turkish"').replace('"Bahasa Indonesia"','"Türkçe"')
                 .replace('"Oleh"','"Yazan"')
                 .replace('guida_id.txt','guida_tr.txt')
                 .replace('CHANGELOG_ID.md','CHANGELOG_TR.md')
                 .replace('donations_id.txt','donations_tr.txt'))
            prefix="crate::settings::" if "crate::settings::" in m.group(2) else ""
            out.append(f"{indent}{prefix}Language::Turkish => {rhs}\n")
    s="".join(out)
    if p in ("src/menu.rs",):
        s=s.replace('(Language::Indonesian, "id"),','(Language::Indonesian, "id"),\n            (Language::Turkish, "tr"),')
    if p=="src/settings.rs":pass
    if p=="src/main.rs":
        s=s.replace('"id" => Language::Indonesian,','"id" => Language::Indonesian,\n        "tr" => Language::Turkish,')
    if p=="src/app_windows/wiktionary_window.rs":
        s=s.replace('"id" => Language::Indonesian,','"id" => Language::Indonesian,\n        "tr" => Language::Turkish,')
    if p=="src/app_windows/options_window.rs":
        s=s.replace('17 => Some(Language::Indonesian),','17 => Some(Language::Indonesian),\n        18 => Some(Language::Turkish),')
        s=s.replace('17 => Language::Indonesian,','17 => Language::Indonesian,\n            18 => Language::Turkish,')
        s=s.replace('"id" => Some(Language::Indonesian),','"id" => Some(Language::Indonesian),\n        "tr" => Some(Language::Turkish),')
        s=once(s,'    lang_id: String,','    lang_id: String,\n    lang_tr: String,',p)
        s=once(s,'        lang_id: "Bahasa Indonesia".to_string(),',
            '        lang_id: "Bahasa Indonesia".to_string(),\n        lang_tr: {\n            let label = i18n::tr(language, "options.lang.tr");\n            if label == "options.lang.tr" { "Türkçe".to_string() } else { label }\n        },',p)
        s=once(s,'        "id" => labels.lang_id.clone(),','        "id" => labels.lang_id.clone(),\n        "tr" => labels.lang_tr.clone(),',p)
        s=s.replace('labels.lang_id.as_str(),','labels.lang_id.as_str(),\n                    labels.lang_tr.as_str(),')
        s=once(s,'            LPARAM(to_wide(&labels.lang_id).as_ptr() as isize),\n        );',
            '            LPARAM(to_wide(&labels.lang_id).as_ptr() as isize),\n        );\n        SendMessageW(\n            combo_lang,\n            CB_ADDSTRING,\n            WPARAM(0),\n            LPARAM(to_wide(&labels.lang_tr).as_ptr() as isize),\n        );',p)
        s=s.replace('(labels.lang_id.clone(), "id"),','(labels.lang_id.clone(), "id"),\n            (labels.lang_tr.clone(), "tr"),')
    if p=="src/app_windows/audio_description_window.rs":
        s=once(s,'        17 => Language::Indonesian,','        17 => Language::Indonesian,\n        18 => Language::Turkish,',p)
        s=once(s,'                add_combo_item(language_combo, "Bahasa Indonesia");',
           '                add_combo_item(language_combo, "Bahasa Indonesia");\n                add_combo_item(language_combo, "Türkçe");',p)
    if p=="src/app_windows/locale_display_names.rs":
        # Test array: for all languages, include tr. Don't touch Indonesian-specific tests.
        s=s.replace('            Language::Indonesian,\n        ] {','            Language::Indonesian,\n            Language::Turkish,\n        ] {')
    if p=="src/tools/reader.rs":
        s=s.replace('            Language::Indonesian,\n','            Language::Indonesian,\n            Language::Turkish,\n')
    if p=="src/app_windows/rai_audiodescrizioni_window.rs":
        # Already has Turkish via duplicate match arm.
        pass
    if s!=original:save(p,original,s)
# Calendar month and day names.
p="src/app_windows/calendar_window.rs";s=load(p);o=s
# Insert Turkish alongside Indonesian constants near other locales.
m=re.search(r'(const WEEKDAYS_ID: \[&str; 7\] = [^\n]+;\n)',s)
if m:
    s=s.replace(m.group(1),m.group(1)+'const WEEKDAYS_TR: [&str; 7] = ["Pazartesi", "Salı", "Çarşamba", "Perşembe", "Cuma", "Cumartesi", "Pazar"];\n')
else:
    # Some projects use static instead of const, insert before localized_date_names.
    s=s.replace('fn localized_date_names(', 'const WEEKDAYS_TR: [&str; 7] = ["Pazartesi", "Salı", "Çarşamba", "Perşembe", "Cuma", "Cumartesi", "Pazar"];\nconst MONTHS_TR: [&str; 12] = ["Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim", "Kasım", "Aralık"];\n\nfn localized_date_names(')
m=re.search(r'(const MONTHS_ID: \[&str; 12\] = [^\n]+;\n)',s)
if m:
    s=s.replace(m.group(1),m.group(1)+'const MONTHS_TR: [&str; 12] = ["Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim", "Kasım", "Aralık"];\n')
if s!=o:save(p,o,s)

# Ensure no missing source names.
p="src/app_windows/rss_window.rs";s=load(p);o=s
# All available RSS news languages (Turkish sources are supported by feed search).
s=s.replace('const NEWS_LANGUAGE_CODES: [&str; 9] = ["it", "en", "de", "fr", "es", "pt", "pt-br", "pl", "cs"];',
    'const NEWS_LANGUAGE_CODES: [&str; 10] = ["it", "en", "de", "fr", "es", "pt", "pt-br", "pl", "cs", "tr"];')
s=s.replace('crate::settings::Language::Czech => "cs",\n        _ => "it",','crate::settings::Language::Czech => "cs",\n        crate::settings::Language::Turkish => "tr",\n        _ => "it",')
s=s.replace('"cs" => crate::settings::Language::Czech,\n', '"cs" => crate::settings::Language::Czech,\n        "tr" => crate::settings::Language::Turkish,\n')
save(p,o,s)

# Add Turkish date names, visible as native Turkish, to multi-language names map.
p="src/app_windows/locale_display_names.json";s=load(p);o=s
# Unicode CLDR data is generated separately if available.
# Ensure Turkish UI filename is present at program compile time.
print("CHANGED",len(changes),flush=True)
for name in changes:print(name)
