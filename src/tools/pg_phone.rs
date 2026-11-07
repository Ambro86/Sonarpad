//! Pagine Gialle's dedicated telephone search; the legacy search API requires a city.
use scraper::{Html, Selector};

#[derive(Debug)]
pub struct PhoneListing {
    pub id: String,
    pub name: String,
    pub address: Option<String>,
    pub category: Option<String>,
    pub phones: Vec<String>,
}

pub struct PhonePage {
    pub results: Vec<PhoneListing>,
    pub is_last_page: bool,
}

pub fn search_url(number: &str, page: usize) -> String {
    let mut url = format!("https://www.paginegialle.it/ricerca/telefono-{number}");
    if page > 1 {
        url.push_str(&format!("/p-{page}"));
    }
    url
}

pub fn parse(html: &str, number: &str) -> Result<PhonePage, String> {
    let document = Html::parse_document(html);
    let selector =
        |value| Selector::parse(value).map_err(|_| "Selettore Pagine Gialle non valido.");
    let input = selector("input#numero")?;
    let cards = selector(".search-itm[data-tr='listing-search-itm']")?;
    let heading = selector(".search-itm__rag")?;
    let links = selector(".search-itm__info a[href]")?;
    let address = selector(".search-itm__adr")?;
    let category = selector(".search-itm__category")?;
    let phones = selector(".search-itm__phone-item")?;
    let next = selector(".next-page-btn[data-pageurl]")?;
    let digits = |text: &str| {
        let value: String = text
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '+')
            .collect();
        value
            .strip_prefix("0039")
            .or_else(|| value.strip_prefix("+39"))
            .unwrap_or(&value)
            .to_string()
    };
    if !document.select(&input).any(|item| {
        item.value()
            .attr("value")
            .is_some_and(|value| digits(value) == number)
    }) {
        return Err("Risposta della ricerca telefonica Pagine Gialle non riconosciuta.".into());
    }
    let text = |item: scraper::ElementRef<'_>| {
        item.text()
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for card in document.select(&cards) {
        let numbers: Vec<_> = card
            .select(&phones)
            .map(text)
            .filter(|p| !p.is_empty())
            .collect();
        if !numbers.iter().any(|phone| digits(phone) == number) {
            continue; // The page may also contain sponsored, unrelated listings.
        }
        let name = card.select(&heading).next().map(text).unwrap_or_default();
        let id = card.select(&links).find_map(|link| {
            let url = url::Url::parse(link.value().attr("href")?).ok()?;
            if !matches!(
                url.host_str(),
                Some("www.paginegialle.it" | "paginegialle.it")
            ) {
                return None;
            }
            let id = url.path().trim_matches('/');
            (!id.is_empty() && !id.starts_with("ricerca/")).then(|| id.to_string())
        });
        let Some(id) = id.filter(|id| !name.is_empty() && seen.insert(id.clone())) else {
            continue;
        };
        results.push(PhoneListing {
            id,
            name,
            address: card
                .select(&address)
                .next()
                .map(text)
                .filter(|v| !v.is_empty()),
            category: card
                .select(&category)
                .next()
                .map(text)
                .filter(|v| !v.is_empty()),
            phones: numbers,
        });
    }
    Ok(PhonePage {
        results,
        is_last_page: !document.select(&next).any(|item| {
            item.value()
                .attr("data-pageurl")
                .is_some_and(|path| path.starts_with(&format!("/ricerca/telefono-{number}/p-")))
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<input id="numero" value="0114310032">
      <div class="search-itm" data-tr="listing-search-itm"><div class="search-itm__info">
        <a href="https://www.paginegialle.it/km5"><h2 class="search-itm__rag">Km5 <span>Torino &amp; bar</span></h2></a>
        <div class="search-itm__adr">Via San Domenico, 14 - Torino</div><div class="search-itm__category">Bar</div>
        <span class="search-itm__phone-item">+39 011 4310032</span></div></div>
      <div class="search-itm" data-tr="listing-search-itm"><div class="search-itm__info">
        <a href="https://www.paginegialle.it/sponsor"><h2 class="search-itm__rag">Sponsored</h2></a>
        <span class="search-itm__phone-item">011 9999999</span></div></div>
      <a class="next-page-btn" data-pageurl="/ricerca/telefono-0114310032/p-2"></a>"#;

    #[test]
    fn telephone_results_preserve_detail_ids_and_filter_sponsors() {
        let page = parse(PAGE, "0114310032").unwrap();
        assert_eq!(page.results.len(), 1);
        assert_eq!(page.results[0].id, "km5");
        assert_eq!(page.results[0].name, "Km5 Torino & bar");
        assert!(page.results[0].address.as_ref().unwrap().contains("Torino"));
        assert_eq!(page.results[0].category.as_deref(), Some("Bar"));
        assert!(!page.is_last_page);
        assert!(search_url("0114310032", 2).ends_with("/telefono-0114310032/p-2"));
    }

    #[test]
    fn telephone_empty_page_is_distinct_from_blocked_or_wrong_response() {
        assert!(parse("<html>Access denied</html>", "0114310032").is_err());
        assert!(parse(PAGE, "020202").is_err());
        let page = parse(r#"<input id="numero" value="0114310032">"#, "0114310032").unwrap();
        assert!(page.results.is_empty());
        assert!(page.is_last_page);
    }
}
