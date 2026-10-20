use crate::i18n;
use crate::settings::Language;
use scraper::{Html, Selector};

#[derive(Debug, Clone)]
pub struct ArticleContent {
    pub title: String,
    pub content: String,
}

fn decode_unicode(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'u') {
            chars.next();
            let mut hex = String::new();
            for _ in 0..4 {
                if let Some(h) = chars.next() {
                    hex.push(h);
                }
            }
            if let Ok(code) = u32::from_str_radix(&hex, 16)
                && let Some(decoded_char) = std::char::from_u32(code)
            {
                result.push(decoded_char);
                continue;
            }
            result.push_str("\\u");
            result.push_str(&hex);
        } else {
            result.push(c);
        }
    }
    result
}

/// Estrae una stringa JSON gestendo correttamente gli escape (\" \\ \n ecc.)
/// Ritorna la stringa decodificata e la posizione dopo la virgoletta di chiusura
fn extract_json_string(s: &str) -> Option<(String, usize)> {
    let mut result = String::new();
    let mut chars = s.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            // Carattere escaped
            if let Some((_, next_c)) = chars.next() {
                match next_c {
                    '"' => result.push('"'),
                    '\\' => result.push('\\'),
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    'u' => {
                        // Unicode escape \uXXXX
                        let mut hex = String::new();
                        for _ in 0..4 {
                            if let Some((_, h)) = chars.next() {
                                hex.push(h);
                            }
                        }
                        if let Ok(code) = u32::from_str_radix(&hex, 16)
                            && let Some(decoded_char) = std::char::from_u32(code)
                        {
                            result.push(decoded_char);
                        }
                    }
                    _ => {
                        result.push('\\');
                        result.push(next_c);
                    }
                }
            }
        } else if c == '"' {
            // Fine della stringa
            return Some((result, i + 1));
        } else {
            result.push(c);
        }
    }

    // Stringa non chiusa
    if !result.is_empty() {
        Some((result, s.len()))
    } else {
        None
    }
}

fn decode_html_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut entity = String::new();
            while let Some(&next) = chars.peek() {
                chars.next();
                if next == ';' {
                    break;
                }
                if entity.len() >= 16 {
                    entity.push(next);
                    break;
                }
                entity.push(next);
            }
            if entity.starts_with("#x") || entity.starts_with("#X") {
                if let Ok(code) = u32::from_str_radix(&entity[2..], 16)
                    && let Some(decoded) = std::char::from_u32(code)
                {
                    out.push(decoded);
                    continue;
                }
            } else if let Some(num) = entity.strip_prefix('#')
                && let Ok(code) = num.parse::<u32>()
                && let Some(decoded) = std::char::from_u32(code)
            {
                out.push(decoded);
                continue;
            } else {
                match entity.as_str() {
                    "nbsp" => out.push(' '),
                    "amp" => out.push('&'),
                    "quot" => out.push('"'),
                    "apos" => out.push('\''),
                    "hellip" => out.push('…'),
                    "ndash" => out.push('–'),
                    "mdash" => out.push('—'),
                    "rsquo" => out.push('’'),
                    "lsquo" => out.push('‘'),
                    "rdquo" => out.push('”'),
                    "ldquo" => out.push('“'),
                    _ => {
                        out.push('&');
                        out.push_str(&entity);
                        out.push(';');
                    }
                }
                continue;
            }
            out.push('&');
            out.push_str(&entity);
            out.push(';');
        } else {
            out.push(c);
        }
    }
    out
}

fn clamp_to_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn looks_like_teaser(value: &str) -> bool {
    let v = value.trim();
    if v.len() < 120 {
        return true;
    }
    v.contains("&hellip;")
        || v.contains("&#8230;")
        || v.contains("[&hellip;]")
        || v.contains("[…]")
        || v.contains("[...]")
        || v.ends_with("…")
        || v.ends_with("...")
}

fn looks_like_ui_chrome(value: &str) -> bool {
    let v = value.to_lowercase();
    v.contains("cookie")
        || v.contains("privacy policy")
        || v.contains("terms and conditions")
        || v.contains("sign up")
        || v.contains("log in")
        || v.contains("subscribe")
        || v.contains("newsletter")
        || v.contains("all rights reserved")
        || v.contains("enable js")
        || v.contains("advert")
        || v.contains("sponsored")
        || v.contains("consent")
}

fn count_sentences(value: &str) -> usize {
    value
        .chars()
        .filter(|c| matches!(c, '.' | '!' | '?'))
        .count()
}

fn extract_json_values(json_text: &str, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search_pos = 0;
    while let Some(text_start) = json_text[search_pos..].find(key) {
        let abs_start = search_pos + text_start + key.len();
        if abs_start < json_text.len() {
            if let Some((val, end_pos)) = extract_json_string(&json_text[abs_start..]) {
                out.push(val);
                search_pos = abs_start + end_pos;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    out
}

fn extract_json_values_loose(json_text: &str, key_name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let token = format!("\"{key_name}\"");
    let mut search_pos = 0usize;
    while let Some(found) = json_text[search_pos..].find(&token) {
        let mut i = search_pos + found + token.len();
        while let Some(ch) = json_text[i..].chars().next() {
            if ch.is_whitespace() {
                i += ch.len_utf8();
            } else {
                break;
            }
        }
        if !json_text[i..].starts_with(':') {
            search_pos = search_pos + found + token.len();
            continue;
        }
        i += 1;
        while let Some(ch) = json_text[i..].chars().next() {
            if ch.is_whitespace() {
                i += ch.len_utf8();
            } else {
                break;
            }
        }
        if !json_text[i..].starts_with('"') {
            search_pos = search_pos + found + token.len();
            continue;
        }
        i += 1;
        if let Some((val, end_pos)) = extract_json_string(&json_text[i..]) {
            out.push(val);
            search_pos = i + end_pos;
        } else {
            break;
        }
    }
    out
}

fn pick_best_json_article_text(json_text: &str) -> Option<String> {
    let keys = [
        "articleBody",
        "body",
        "bodyHtml",
        "content",
        "contentHtml",
        "full_text",
        "text",
    ];
    let mut best = String::new();
    for key in keys {
        let strict = format!("\"{key}\":\"");
        for val in extract_json_values(json_text, &strict)
            .into_iter()
            .chain(extract_json_values_loose(json_text, key))
        {
            if val.len() < 80 {
                continue;
            }
            let cleaned = clean_text(&val);
            let cleaned = collapse_blank_lines(&cleaned);
            let trimmed = cleaned.trim();
            if trimmed.len() < 300 {
                continue;
            }
            if looks_like_teaser(trimmed) || looks_like_ui_chrome(trimmed) {
                continue;
            }
            if count_sentences(trimmed) < 2 {
                continue;
            }
            if trimmed.len() > best.len() {
                best = trimmed.to_string();
            }
        }
    }
    if best.is_empty() { None } else { Some(best) }
}

fn pick_teaser_json_article_text(json_text: &str) -> Option<String> {
    let mut best = String::new();
    let strict = "\"articleBody\":\"";
    for val in extract_json_values(json_text, strict)
        .into_iter()
        .chain(extract_json_values_loose(json_text, "articleBody"))
    {
        let cleaned = collapse_blank_lines(&clean_text(&val));
        let trimmed = cleaned.trim();
        if trimmed.len() >= 40 && trimmed.len() > best.len() {
            best = trimmed.to_string();
        }
    }
    if best.is_empty() { None } else { Some(best) }
}

fn trim_after_known_trailers(input: &str) -> String {
    let markers = [
        "ABOUT THE AUTHOR",
        "Related Stories",
        "CBC's Journalistic Standards and Practices",
        "Corrections and clarifications",
    ];
    let mut cut = input.len();
    for marker in markers {
        if let Some(idx) = input.find(marker)
            && idx < cut
        {
            cut = idx;
        }
    }
    input[..cut].trim().to_string()
}

fn extract_cbc_initial_state_article_text(html_content: &str) -> Option<String> {
    let mut best = String::new();
    for val in extract_json_values(html_content, "\"bodyHtml\":\"") {
        if val.len() < 300 {
            continue;
        }
        let cleaned = collapse_blank_lines(&clean_text(&val));
        let trimmed = trim_after_known_trailers(cleaned.trim());
        if trimmed.len() < 300 {
            continue;
        }
        if looks_like_ui_chrome(&trimmed) {
            continue;
        }
        if count_sentences(&trimmed) < 3 {
            continue;
        }
        if trimmed.len() > best.len() {
            best = trimmed;
        }
    }
    if best.is_empty() {
        let mut lines = Vec::new();
        for val in extract_json_values(html_content, "\"type\":\"text\",\"content\":\"") {
            let cleaned = collapse_blank_lines(&clean_text(&val));
            let trimmed = cleaned.trim();
            if trimmed.len() < 20 {
                continue;
            }
            if looks_like_ui_chrome(trimmed) {
                continue;
            }
            if trimmed.contains("ABOUT THE AUTHOR")
                || trimmed.contains("Related Stories")
                || trimmed.contains("Journalistic Standards")
                || trimmed.contains("Corrections and clarifications")
            {
                continue;
            }
            if lines.last().is_some_and(|last: &String| last == trimmed) {
                continue;
            }
            lines.push(trimmed.to_string());
        }
        if !lines.is_empty() {
            let joined = lines.join("\n\n");
            let trimmed = trim_after_known_trailers(joined.trim());
            if trimmed.len() >= 300 && count_sentences(&trimmed) >= 3 {
                best = trimmed;
            }
        }
    }
    if best.is_empty() { None } else { Some(best) }
}

fn extract_jina_markdown_fixture(raw_content: &str, language: Language) -> Option<ArticleContent> {
    if !raw_content.contains("URL Source:")
        || !raw_content.contains("Markdown Content:")
        || !raw_content.contains("Title:")
    {
        return None;
    }

    let title = raw_content
        .lines()
        .find_map(|line| line.strip_prefix("Title:").map(str::trim))
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| i18n::tr(language, "reader.no_title"));

    let marker = "Markdown Content:";
    let start = raw_content.find(marker)? + marker.len();
    let body_src = &raw_content[start..];

    let mut lines = Vec::new();
    for line in body_src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            lines.push(String::new());
            continue;
        }
        // r.jina.ai footer/call-to-action blocks (non-article).
        if trimmed.eq_ignore_ascii_case("[YOU MAKE OUR WORK POSSIBLE.](https://iowacapitaldispatch.com/donate/?oa_referrer=midstorybox)")
            || trimmed.starts_with("If you value")
            || trimmed.starts_with("Support")
        {
            break;
        }
        lines.push(trimmed.to_string());
    }

    let body = collapse_blank_lines(&clean_text(&lines.join("\n")));
    let content = body.trim().to_string();
    if content.len() < 300 || count_sentences(&content) < 3 {
        return None;
    }

    Some(ArticleContent { title, content })
}

pub fn clean_text(input: &str) -> String {
    let decoded = decode_html_entities(&decode_unicode(input));
    // Pulizia encoding Mediaset/TGCOM24
    let mut text = decoded
        .replace("ÃƒÂ¨", "Ã¨")
        .replace("ÃƒÂ ", "Ã ")
        .replace("ÃƒÂ¹", "Ã¹")
        .replace("ÃƒÂ²", "Ã²")
        .replace("ÃƒÂ¬", "Ã¬")
        .replace("Ã‚Â ", " ")
        .replace("ÃƒÂ©", "Ã©")
        .replace("Ã‚", "");

    text = text
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("\u{00a0}", " ");
    text = text
        .replace("\\\"", "\"")
        .replace("\\n", "\n")
        .replace("\\/", "/");

    let mut cleaned = String::new();
    let mut in_tag = false;
    for c in text.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
            cleaned.push(' ');
        } else if !in_tag {
            cleaned.push(c);
        }
    }
    cleaned
}

fn author_prefix(language: Language) -> &'static str {
    match language {
        Language::Ukrainian | Language::English => "By",
        Language::German => "Von",
        Language::Italian => "Di",
        Language::French => "Par",
        Language::Spanish => "Por",
        Language::Portuguese | Language::PortugueseBrazilian => "Por",
        Language::Swedish => "Av",
        Language::Czech => "Od",
        Language::Polish => "Autor",
        Language::Vietnamese => "Bởi",
        Language::Serbian => "Autor",
        Language::Lithuanian => "Autorius",
        Language::Russian => "Автор",
        Language::Chinese => "作者",
        Language::Hindi => "द्वारा",
        Language::Indonesian => "Oleh",
    }
}

pub fn reader_mode_extract(html_content: &str, language: Language) -> Option<ArticleContent> {
    if !html_content.contains("<html")
        && let Some(article) = extract_jina_markdown_fixture(html_content, language)
    {
        return Some(article);
    }

    let document = Html::parse_document(html_content);
    let title = pick_title(&document, language);

    let mut body_acc = String::new();
    let mut author_info = String::new();
    let mut found_anything = false;

    // 1. ESTRAZIONE DA JSON-LD (Schema.org) - MOLTO RICCO SU TGCOM24
    if let Ok(s) = Selector::parse("script[type='application/ld+json']") {
        for element in document.select(&s) {
            let json = element.text().collect::<Vec<_>>().join("");
            let mut has_rich_json_body = false;

            // Cerchiamo Autore e Data
            if author_info.is_empty() {
                if let Some(author_idx) = json.find("\"author\"") {
                    let author_part = &json[author_idx..];
                    if let Some(name_idx) = author_part.find("\"name\":\"") {
                        let part = &author_part[name_idx + 8..];
                        if let Some((name, _)) = extract_json_string(part) {
                            let trimmed = name.trim();
                            if !trimmed.eq_ignore_ascii_case(&title)
                                && !trimmed.eq_ignore_ascii_case("home")
                                && trimmed.len() >= 3
                            {
                                author_info.push_str(trimmed);
                            }
                        }
                    }
                }
                if author_info.is_empty()
                    && let Some(a_idx) = json.find("\"name\":\"")
                {
                    let part = &json[a_idx + 8..];
                    if let Some((name, _)) = extract_json_string(part) {
                        let trimmed = name.trim();
                        if !trimmed.eq_ignore_ascii_case(&title)
                            && !trimmed.eq_ignore_ascii_case("home")
                            && trimmed.len() >= 3
                        {
                            author_info.push_str(trimmed);
                        }
                    }
                }
                if let Some(d_idx) = json.find("\"datePublished\":\"") {
                    let part = &json[d_idx + 17..];
                    if let Some((date_str, _)) = extract_json_string(part) {
                        let date = if date_str.len() >= 10 {
                            &date_str[..10]
                        } else {
                            &date_str
                        };
                        author_info.push_str(&format!(" ({})", date));
                    }
                }
            }

            // Cerchiamo description e articleBody
            for key in [
                "\"description\":\"",
                "\"articleBody\":\"",
                "\"subtitle\":\"",
            ] {
                let mut search_pos = 0;
                while let Some(key_pos) = json[search_pos..].find(key) {
                    let abs_start = search_pos + key_pos + key.len();
                    if abs_start < json.len() {
                        if let Some((val, end_pos)) = extract_json_string(&json[abs_start..]) {
                            if key == "\"description\":\"" {
                                let window_start =
                                    clamp_to_char_boundary(&json, key_pos.saturating_sub(400));
                                let window_end = clamp_to_char_boundary(&json, key_pos);
                                let window = &json[window_start..window_end];
                                let is_person_or_org = window.contains("\"@type\":\"Person\"")
                                    || window.contains("\"@type\":\"Organization\"");
                                let is_article = window.contains("\"@type\":\"Article\"")
                                    || window.contains("\"@type\":\"NewsArticle\"")
                                    || window.contains("\"@type\":\"TechArticle\"")
                                    || window.contains("\"@type\":\"BlogPosting\"");
                                if is_person_or_org || !is_article {
                                    search_pos = abs_start + end_pos;
                                    continue;
                                }
                            }
                            if val.len() > 40
                                && !val.contains("http")
                                && !body_acc.contains(&val)
                                && !looks_like_teaser(&val)
                            {
                                body_acc.push_str(&val);
                                body_acc.push_str("\n\n");
                                if key == "\"articleBody\":\"" {
                                    has_rich_json_body = true;
                                }
                            }
                            search_pos = abs_start + end_pos;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
            if has_rich_json_body {
                found_anything = true;
            }
        }
    }

    // 2. ESTRAZIONE DA NEXT_DATA (WSJ / Altri)
    if !found_anything
        && let Ok(next_selector) = Selector::parse("script#__NEXT_DATA__")
        && let Some(element) = document.select(&next_selector).next()
    {
        let json_text = element.text().collect::<Vec<_>>().join("");

        // WSJ: estrai testi dai blocchi "content":[...] dei paragrafi
        let mut seen_paragraphs = std::collections::HashSet::new();
        for content_block in json_text.split("\"type\":\"paragraph\"") {
            if let Some(content_start) = content_block.find("\"content\":[") {
                let after_content = &content_block[content_start..];
                // Estrai tutti i "text":"..." dal blocco content usando extract_json_string
                let mut para_text = String::new();
                let mut search_pos = 0;
                while let Some(text_start) = after_content[search_pos..].find("\"text\":\"") {
                    let abs_start = search_pos + text_start + 8; // dopo "text":"
                    if abs_start < after_content.len() {
                        if let Some((val, end_pos)) =
                            extract_json_string(&after_content[abs_start..])
                        {
                            if !val.is_empty() && !val.starts_with('{') {
                                para_text.push_str(&val);
                            }
                            search_pos = abs_start + end_pos;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                // Evita duplicati
                if para_text.len() > 20 && !seen_paragraphs.contains(&para_text) {
                    seen_paragraphs.insert(para_text.clone());
                    body_acc.push_str(&para_text);
                    body_acc.push_str("\n\n");
                    found_anything = true;
                }
            }
        }

        // Fallback: vecchio metodo per altri siti (con extract_json_string)
        if !found_anything {
            // Next.js payloads like Radioitalia store article paragraphs as
            // "__typename":"Text","content":"...".
            let mut seen_text_content = std::collections::HashSet::new();
            for val in extract_json_values(&json_text, "\"__typename\":\"Text\",\"content\":\"") {
                let cleaned = collapse_blank_lines(&clean_text(&val));
                let trimmed = cleaned.trim();
                if trimmed.len() < 30
                    || trimmed.contains("http")
                    || trimmed.contains('{')
                    || trimmed.contains("categoryName")
                    || looks_like_ui_chrome(trimmed)
                {
                    continue;
                }
                if seen_text_content.insert(trimmed.to_string()) {
                    body_acc.push_str(trimmed);
                    body_acc.push_str("\n\n");
                    found_anything = true;
                }
            }
        }

        if !found_anything {
            let mut search_pos = 0;
            while let Some(text_start) = json_text[search_pos..].find("\"text\":\"") {
                let abs_start = search_pos + text_start + 8;
                if abs_start < json_text.len() {
                    if let Some((val, end_pos)) = extract_json_string(&json_text[abs_start..]) {
                        if val.len() > 30 && !val.contains("http") && !val.contains("{") {
                            body_acc.push_str(&val);
                            body_acc.push_str("\n\n");
                            found_anything = true;
                        }
                        search_pos = abs_start + end_pos;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        if !found_anything && let Some(best) = pick_best_json_article_text(&json_text) {
            body_acc.push_str(&best);
            body_acc.push_str("\n\n");
            found_anything = true;
        }
    }

    // 2b. CBC / React state fallback: bodyHtml often contains full article paragraphs.
    if body_acc.len() < 300
        && html_content.contains("cbc.ca")
        && html_content.contains("__INITIAL_STATE__")
        && let Some(cbc_body) = extract_cbc_initial_state_article_text(html_content)
    {
        body_acc.push_str(&cbc_body);
        body_acc.push_str("\n\n");
        found_anything = true;
    }

    // 3. FALLBACK CSS
    if !found_anything || body_acc.len() < 300 {
        let content_selectors = [
            ".body-article .content h2, .body-article .content h3, .body-article .content p, .body-article .content li",
            ".blog-detail-wrapper .rich-text h2, .blog-detail-wrapper .rich-text h3, .blog-detail-wrapper .rich-text p, .blog-detail-wrapper .rich-text li",
            ".node-text .textarea-content-body",
            ".node-summary",
            ".section--content-news .left-content p",
            ".section--content-news .title-quote-text p",
            ".story__text p, .story__text h2, .story__text li",
            "#article-body .story__text",
            ".post-content > h2, .post-content > h3, .post-content > p, .post-content > li",
            ".entry-content h2, .entry-content h3, .entry-content p, .entry-content li",
            ".wp-block-post-content h2, .wp-block-post-content h3, .wp-block-post-content p, .wp-block-post-content li",
            ".ifq-post__content p",
            ".ifq-post__content",
            ".media-content.news-txt p, .media-content.news-txt figcaption, .media-content.news-txt .image-caption",
            ".col-md-8.pb-5 .mt-4 p",
            "p[data-type='paragraph']", // WSJ modern
            "article [data-testid='article-body'] p",
            "article [data-testid='paragraph']",
            "article [data-type='paragraph']",
            ".prose p",
            ".wsj-article-body p",
            "article p",
            ".atext",
            ".art-text",
            ".story-content p",
            ".article-body p",
            "#col-sx-interna p",
        ];
        let mut best_sel_acc = String::new();
        for sel_str in content_selectors {
            if let Ok(selector) = Selector::parse(sel_str) {
                let mut sel_acc = String::new();
                for element in document.select(&selector) {
                    let text = element.text().collect::<Vec<_>>().join(" ");
                    if text.to_lowercase().contains("enable js") {
                        continue;
                    }
                    sel_acc.push_str(&text);
                    sel_acc.push_str("\n\n");
                }
                if sel_acc.len() > best_sel_acc.len() {
                    best_sel_acc = sel_acc;
                }
            }
        }
        if best_sel_acc.len() > 200 {
            body_acc.push_str(&best_sel_acc);
        }
    }

    // 3b. Last-resort teaser fallback for paywalled pages where only JSON-LD
    // articleBody preview is present: prefer teaser over title-only output.
    if body_acc.trim().len() < 40
        && let Some(teaser) = pick_teaser_json_article_text(html_content)
    {
        body_acc.push_str(&teaser);
        body_acc.push_str("\n\n");
    }

    let mut final_text = String::new();
    if !author_info.is_empty() {
        let prefix = author_prefix(language);
        final_text.push_str(&format!("{prefix} {}\n\n", author_info));
    }
    final_text.push_str(&body_acc);

    let content = strip_post_extraction_noise(&clean_text(&final_text));
    let mut final_content = collapse_blank_lines(&content);
    if let Some(meta_desc) = pick_meta_description(&document) {
        let should_fallback = body_acc.trim().len() < 120
            || count_sentences(&final_content) < 2
            || looks_like_ui_chrome(&final_content);
        if should_fallback {
            let mut fallback = String::new();
            if !author_info.is_empty() {
                let prefix = author_prefix(language);
                fallback.push_str(&format!("{prefix} {}\n\n", author_info));
            }
            fallback.push_str(meta_desc.trim());
            let fallback_content =
                collapse_blank_lines(&strip_post_extraction_noise(&clean_text(&fallback)));
            if fallback_content.len() > final_content.len() {
                final_content = fallback_content;
            }
        }
    }
    if final_content.trim().len() < 10
        && let Some(url) = pick_reddit_link_post_url(&document)
    {
        final_content = i18n::tr(language, "reader.external_link").replace("{url}", &url);
    }
    Some(ArticleContent {
        title: title.trim().to_string(),
        content: final_content,
    })
}

fn strip_post_extraction_noise(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut skip_fatto_regwall_block = false;
    let mut fatto_regwall_brace_depth = 0i32;

    for line in input.lines() {
        let trimmed = line.trim();
        if skip_fatto_regwall_block {
            fatto_regwall_brace_depth += brace_delta(trimmed);
            if fatto_regwall_brace_depth < 0 {
                skip_fatto_regwall_block = false;
                fatto_regwall_brace_depth = 0;
            }
            continue;
        }
        if starts_fatto_regwall_noise_block(trimmed) {
            skip_fatto_regwall_block = true;
            fatto_regwall_brace_depth = brace_delta(trimmed);
            continue;
        }
        if is_known_js_noise_line(trimmed) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim_end_matches('\n').to_string()
}

fn brace_delta(line: &str) -> i32 {
    line.chars().fold(0i32, |acc, c| match c {
        '{' => acc + 1,
        '}' => acc - 1,
        _ => acc,
    })
}

fn starts_fatto_regwall_noise_block(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("softregwall")
        || lower.starts_with("if (softregwall")
        || lower.contains("ifq-post__label-soft-registration")
}

fn is_known_js_noise_line(line: &str) -> bool {
    if line.is_empty() {
        return false;
    }

    let lower = line.to_ascii_lowercase();
    lower.contains("window.datawrapper")
        || lower.contains("datawrapper-height")
        || lower.contains("addeventlistener(\"message\"")
        || lower.contains("addeventlistener('message'")
        || lower.contains("window.addeventlistener('load'")
        || lower.contains("window.addeventlistener(\"load\"")
        || lower.contains("window.datalayer.push({")
        || lower.contains("event: 'show_paywall'")
        || lower.contains("event: \"show_paywall\"")
        || lower.contains("category: 'regwall'")
        || lower.contains("category: \"regwall\"")
        || lower.contains("action: 'overlay'")
        || lower.contains("action: \"overlay\"")
        || lower.contains("ifq-post__label-soft-registration")
        || lower.contains("visualizzazione soft regwall")
        // Radioitalia pages can leak a raw nav JSON blob into extracted text.
        || (lower.starts_with("{\"type\":\"main\",\"entry\":[")
            && lower.contains("\"categoryname\":\"undefined\""))
}

#[cfg(test)]
mod tests {
    use super::{
        author_prefix, clamp_to_char_boundary, extract_json_values_loose, is_known_js_noise_line,
        pick_best_json_article_text, strip_post_extraction_noise,
    };
    use crate::settings::Language;

    #[test]
    fn author_prefix_covers_all_languages() {
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
            Language::Lithuanian,
            Language::Russian,
            Language::Chinese,
            Language::Hindi,
            Language::Indonesian,
        ];

        for language in languages {
            let prefix = author_prefix(language);
            let name = match language {
                Language::Italian => "Italian",
                Language::Ukrainian | Language::English => "English",
                Language::German => "German",
                Language::Spanish => "Spanish",
                Language::Portuguese => "Portuguese (Portugal)",
                Language::PortugueseBrazilian => "Portuguese (Brazil)",
                Language::Swedish => "Swedish",
                Language::Vietnamese => "Vietnamese",
                Language::Czech => "Czech",
                Language::Polish => "Polish",
                Language::French => "French",
                Language::Serbian => "Serbian",
                Language::Lithuanian => "Lithuanian",
                Language::Russian => "Russian",
                Language::Chinese => "Chinese",
                Language::Hindi => "Hindi",
                Language::Indonesian => "Indonesian",
            };
            assert!(
                !prefix.trim().is_empty(),
                "empty author prefix for language {name}"
            );
        }
    }

    #[test]
    fn known_js_noise_filters_radioitalia_nav_blob() {
        let noise = r#"{"type":"main","entry":[{"enabled":"true","categoryName":"undefined","label":"NEWS"}]}"#;
        assert!(is_known_js_noise_line(noise));
        assert!(!is_known_js_noise_line(
            "Gigi D'Alessio racconta le storie dietro le canzoni."
        ));
    }

    #[test]
    fn known_js_noise_filters_fatto_regwall_script_lines() {
        assert!(is_known_js_noise_line(
            "window.addEventListener('load', function () {"
        ));
        assert!(is_known_js_noise_line(
            "const softRegwall = document.querySelector('.ifq-post__label-soft-registration');"
        ));
        assert!(is_known_js_noise_line("window.dataLayer.push({"));
        assert!(is_known_js_noise_line("event: 'show_paywall',"));
        assert!(is_known_js_noise_line("category: 'regwall',"));
        assert!(is_known_js_noise_line("action: 'overlay',"));
        assert!(is_known_js_noise_line(
            "label: 'Visualizzazione soft regwall - test'"
        ));
    }

    #[test]
    fn strip_post_extraction_noise_removes_fatto_regwall_block() {
        let input = concat!(
            "Prima frase utile.\n",
            "if (softRegwall && !softRegwall.classList.contains('hidden')) {\n",
            "window.dataLayer = window.dataLayer || [];\n",
            "category: 'regwall',\n",
            "action: 'overlay',\n",
            "});\n",
            "}\n",
            "Seconda frase utile.\n"
        );
        let cleaned = strip_post_extraction_noise(input);
        assert!(cleaned.contains("Prima frase utile."));
        assert!(cleaned.contains("Seconda frase utile."));
        assert!(!cleaned.contains("softRegwall"));
        assert!(!cleaned.contains("regwall"));
        assert!(!cleaned.contains("overlay"));
    }

    #[test]
    fn clamp_to_char_boundary_handles_multibyte_indices() {
        let s = "abcědef";
        // byte index 4 is inside 'ě' (which starts at 3 and ends at 5)
        assert_eq!(clamp_to_char_boundary(s, 4), 3);
        assert_eq!(clamp_to_char_boundary(s, 5), 5);
        assert_eq!(clamp_to_char_boundary(s, 999), s.len());
    }

    #[test]
    fn extract_json_values_loose_supports_spaces_around_colon() {
        let json = r#"{"articleBody" : "Prima frase. Seconda frase."}"#;
        let vals = extract_json_values_loose(json, "articleBody");
        assert_eq!(vals.len(), 1);
        assert!(vals[0].contains("Seconda frase"));
    }

    #[test]
    fn pick_best_json_article_text_supports_spaced_article_body_key() {
        let json = r#"{"articleBody" : "Questa e una prima frase abbastanza lunga per il parser e contiene diversi dettagli utili alla lettura. Questa e una seconda frase abbastanza lunga per superare i filtri e mantenere una struttura naturale del testo estratto. Questa e una terza frase con ulteriore contesto, dati e spiegazioni per simulare un articolo reale con contenuto corposo e leggibile. Questa e una quarta frase che completa il paragrafo e porta la lunghezza totale oltre la soglia minima prevista dai controlli."}"#;
        let best = pick_best_json_article_text(json).expect("expected article text");
        assert!(best.contains("prima frase"));
        assert!(best.contains("seconda frase"));
    }
}

fn pick_title(document: &Html, language: Language) -> String {
    let title_selectors = ["meta[property='og:title']", "h1", "title"];
    for sel in title_selectors {
        if let Ok(s) = Selector::parse(sel)
            && let Some(el) = document.select(&s).next()
        {
            let t = if sel.contains("meta") {
                el.value().attr("content").unwrap_or("").to_string()
            } else {
                el.text().collect::<Vec<_>>().join(" ")
            };
            let clean_t = t.trim();
            if clean_t.len() > 5 && !clean_t.to_lowercase().ends_with(".com") {
                return decode_unicode(clean_t);
            }
        }
    }
    i18n::tr(language, "reader.no_title")
}

fn pick_meta_description(document: &Html) -> Option<String> {
    let selectors = [
        "meta[name='description']",
        "meta[property='og:description']",
        "meta[name='twitter:description']",
    ];
    let mut best = String::new();
    for sel in selectors {
        if let Ok(s) = Selector::parse(sel)
            && let Some(el) = document.select(&s).next()
            && let Some(content) = el.value().attr("content")
        {
            let clean = decode_unicode(content.trim());
            if clean.len() > best.len() {
                best = clean;
            }
        }
    }
    if best.len() >= 40 { Some(best) } else { None }
}

fn pick_reddit_link_post_url(document: &Html) -> Option<String> {
    let selector =
        Selector::parse("shreddit-post[post-type='link'] div[slot='post-media-container'] a[href]")
            .ok()?;
    for el in document.select(&selector) {
        if let Some(href) = el.value().attr("href") {
            let trimmed = href.trim();
            if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

pub fn collapse_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_run = 0usize;
    let mut seen_short = std::collections::HashSet::new();
    for line in s.lines() {
        let l = line.trim();
        if l.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push('\n');
            }
        } else {
            if let Some(prev) = out.lines().last()
                && prev.eq_ignore_ascii_case(l)
            {
                continue;
            }
            if l.len() <= 40 {
                let key = l.to_ascii_lowercase();
                if !seen_short.insert(key) {
                    continue;
                }
            }
            blank_run = 0;
            out.push_str(l);
            out.push('\n');
        }
    }
    out.trim_end_matches('\n').to_string()
}
