use std::fs;
use std::path::Path;

mod settings {
    #[derive(Clone, Copy)]
    pub enum Language {
        English,
        Italian,
        French,
        German,
        Spanish,
        Portuguese,
        PortugueseBrazilian,
        Swedish,
        Czech,
        Polish,
        Vietnamese,
        Serbian,
        Ukrainian,
        Lithuanian,
        Russian,
        Chinese,
        Hindi,
        Indonesian,
    }
}

mod i18n {
    use crate::settings::Language;

    pub fn tr(language: Language, key: &str) -> String {
        match key {
            "reader.no_title" => match language {
                Language::Italian => "Nessun titolo".to_string(),
                Language::French => "Sans titre".to_string(),
                Language::German => "Kein Titel".to_string(),
                Language::Spanish => "Sin título".to_string(),
                Language::Portuguese | Language::PortugueseBrazilian => "Sem título".to_string(),
                Language::Swedish => "Ingen titel".to_string(),
                Language::Vietnamese => "Không có tiêu đề".to_string(),
                Language::Czech => "Bez názvu".to_string(),
                Language::Polish => "Brak tytułu".to_string(),
                Language::Serbian => "Bez naslova".to_string(),
                Language::Lithuanian => "Be pavadinimo".to_string(),
                Language::Russian => "Без названия".to_string(),
                Language::Chinese => "无标题".to_string(),
                Language::Hindi => "कोई शीर्षक नहीं".to_string(),
                Language::Indonesian | Language::Ukrainian | Language::English => {
                    "No Title".to_string()
                }
            },
            "reader.external_link" => match language {
                Language::Italian => "Link esterno:\n{url}".to_string(),
                Language::French => "Lien externe :\n{url}".to_string(),
                Language::German => "Externer Link:\n{url}".to_string(),
                Language::Spanish => "Enlace externo:\n{url}".to_string(),
                Language::Portuguese | Language::PortugueseBrazilian => {
                    "Link externo:\n{url}".to_string()
                }
                Language::Swedish => "Extern länk:\n{url}".to_string(),
                Language::Vietnamese => "Liên kết ngoài:\n{url}".to_string(),
                Language::Czech => "Externí odkaz:\n{url}".to_string(),
                Language::Polish => "Link zewnętrzny:\n{url}".to_string(),
                Language::Serbian => "Spoljašnji link:\n{url}".to_string(),
                Language::Lithuanian => "Išorinė nuoroda:\n{url}".to_string(),
                Language::Russian => "Внешняя ссылка:\n{url}".to_string(),
                Language::Chinese => "外部链接：\n{url}".to_string(),
                Language::Hindi => "बाहरी लिंक:\n{url}".to_string(),
                Language::Indonesian | Language::Ukrainian | Language::English => {
                    "External link:\n{url}".to_string()
                }
            },
            _ => key.to_string(),
        }
    }
}

#[path = "../src/tools/reader.rs"]
mod reader;

struct Expectation {
    file: String,
    min_chars: usize,
    min_words: usize,
    require_title: bool,
}

fn load_expectations() -> Vec<Expectation> {
    let data =
        fs::read_to_string("tests/fixtures/article_expectations.csv").expect("read expectations");
    let mut out = Vec::new();
    for (i, line) in data.lines().enumerate() {
        if i == 0 {
            continue;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<_> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 4 {
            continue;
        }
        let file = parts[0].to_string();
        let min_chars = parts[1].parse::<usize>().expect("min_chars");
        let min_words = parts[2].parse::<usize>().expect("min_words");
        let require_title = !parts[3].is_empty();
        out.push(Expectation {
            file,
            min_chars,
            min_words,
            require_title,
        });
    }
    out
}

#[test]
fn reader_fixtures_meet_minimums() {
    let expectations = load_expectations();
    assert!(!expectations.is_empty(), "no expectations loaded");

    for exp in expectations {
        let path = Path::new(&exp.file);
        let html = fs::read_to_string(path).expect("read fixture html");
        let article = reader::reader_mode_extract(&html, settings::Language::Italian)
            .unwrap_or_else(|| panic!("no article extracted from {}", exp.file));

        let title = article.title.trim();
        if exp.require_title {
            assert!(!title.is_empty(), "empty title for {}", exp.file);
            assert!(title != "No Title", "missing title for {}", exp.file);
        }

        let content = article.content.trim();
        let chars = content.chars().count();
        let words = content.split_whitespace().count();
        assert!(
            chars >= exp.min_chars,
            "{} chars too short (got {}, expected >= {})",
            exp.file,
            chars,
            exp.min_chars
        );
        assert!(
            words >= exp.min_words,
            "{} words too short (got {}, expected >= {})",
            exp.file,
            words,
            exp.min_words
        );
    }
}

#[test]
fn ukrainian_language_variant_is_exercised() {
    let label = i18n::tr(settings::Language::Ukrainian, "reader.no_title");
    assert_eq!(label, "No Title");
}

#[test]
fn corriere_podcast_body_article_content_is_extracted() {
    let html = r#"
        <html>
            <head>
                <meta property="og:title" content="Podcast title">
                <meta name="description" content="Short feed preview that should not replace the article body.">
            </head>
            <body>
                <main>
                    <section class="body-article">
                        <div class="content">
                            <p>Il monitoraggio della diffusione in Italia apre il testo completo del podcast. Questa parte contiene il secondo tema, poi il terzo tema, e prosegue con molti dettagli utili per superare la soglia del parser. Il testo deve rimanere quello del corpo pagina, non la descrizione breve nei meta tag.</p>
                        </div>
                    </section>
                </main>
            </body>
        </html>
    "#;

    let article = reader::reader_mode_extract(html, settings::Language::Italian)
        .expect("expected Corriere podcast body article extraction");

    assert!(article.content.contains("Il monitoraggio della diffusione"));
    assert!(article.content.contains("non la descrizione breve"));
    assert!(!article.content.contains("Short feed preview"));
}
