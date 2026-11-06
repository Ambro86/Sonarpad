#[path = "places.rs"]
mod places;

use base64::Engine;
use quick_xml::{Reader, events::Event};
use std::collections::HashSet;
use url::{Url, form_urlencoded::Serializer};

const ITALIAONLINE_BASE_URL_B64: &str = "BT9NUUx/HRUjWkodMRoTOlYsGzZeHTVfCiMeAT4c";
const ITALIAONLINE_SEARCH_PB_B64: &str = "Hi5YUxU4Qho=";
const ITALIAONLINE_SEARCH_PG_B64: &str = "Hi5YUxU4Qh8=";
const ITALIAONLINE_DETAIL_PB_B64: &str = "CS5NQB88Qho=";
const ITALIAONLINE_DETAIL_PG_B64: &str = "CS5NQB88Qh8=";
const ITALIAONLINE_CLIENT_B64: &str = "HSlUThQ5Xh0=";
const ITALIAONLINE_VERSION_B64: &str = "XmUAD0M=";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectoryKind {
    PagineBianche,
    PagineGialle,
}

impl DirectoryKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::PagineBianche => "Ricerca nominativi",
            Self::PagineGialle => "Luoghi e attività",
        }
    }

    fn search_endpoint(self) -> Result<String, String> {
        match self {
            Self::PagineBianche => decode_italiaonline_url(ITALIAONLINE_SEARCH_PB_B64),
            Self::PagineGialle => decode_italiaonline_url(ITALIAONLINE_SEARCH_PG_B64),
        }
    }

    fn detail_endpoint(self) -> Result<String, String> {
        match self {
            Self::PagineBianche => decode_italiaonline_url(ITALIAONLINE_DETAIL_PB_B64),
            Self::PagineGialle => decode_italiaonline_url(ITALIAONLINE_DETAIL_PG_B64),
        }
    }

    pub(crate) fn primary_field_label(self) -> &'static str {
        match self {
            Self::PagineBianche => "Inserisci nome o cognome",
            Self::PagineGialle => "Inserisci luogo o attività",
        }
    }

    pub(crate) fn primary_field_name(self) -> &'static str {
        match self {
            Self::PagineBianche => "nome o cognome",
            Self::PagineGialle => "attività",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SearchQuery {
    pub(crate) kind: DirectoryKind,
    pub(crate) what: String,
    pub(crate) where_: String,
    pub(crate) country: String,
    pub(crate) language: String,
    pub(crate) page: usize,
    pub(crate) reverse: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct SearchResult {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) address: Option<String>,
    pub(crate) city: Option<String>,
    pub(crate) province: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) phones: Vec<String>,
    pub(crate) detail: Option<DetailResponse>,
}

#[derive(Clone, Debug)]
pub(crate) struct SearchResponse {
    pub(crate) display_where: Option<String>,
    pub(crate) current_page: usize,
    pub(crate) is_last_page: bool,
    pub(crate) results: Vec<SearchResult>,
}

#[derive(Clone, Debug)]
pub(crate) struct AmbiguousPlaceResponse {
    pub(crate) places: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) enum SearchOutcome {
    Results(SearchResponse),
    AmbiguousAddress(AmbiguousPlaceResponse),
}

#[derive(Clone, Debug)]
pub(crate) struct DetailResponse {
    pub(crate) title: String,
    pub(crate) body: String,
}

#[derive(Default)]
struct DetailAccumulator {
    status: Option<String>,
    title: Option<String>,
    description: Option<String>,
    category: Option<String>,
    address: Option<String>,
    city: Option<String>,
    province: Option<String>,
    phones: Vec<String>,
    websites: Vec<String>,
    emails: Vec<String>,
    public_url: Option<String>,
}

#[derive(Default)]
struct SearchAccumulator {
    status: Option<String>,
    display_where: Option<String>,
    current_page: usize,
    is_last_page: Option<bool>,
    result_count: Option<usize>,
    max_results: Option<usize>,
    places: Vec<String>,
}

pub(crate) fn normalize_phone(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.chars().enumerate().any(|(i, c)| {
        !c.is_ascii_digit() && !c.is_whitespace() && !"()-./".contains(c) && !(c == '+' && i == 0)
    }) {
        return Err("Inserisci un numero di telefono italiano valido.".into());
    }
    let compact: String = input
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();
    let number = if let Some(v) = compact
        .strip_prefix("+39")
        .or_else(|| compact.strip_prefix("0039"))
    {
        v
    } else if compact.starts_with('+') || compact.starts_with("00") {
        return Err("Inserisci un numero italiano, con prefisso +39 facoltativo.".into());
    } else {
        &compact
    };
    if !(5..=15).contains(&number.len()) {
        return Err("Inserisci un numero di telefono completo di prefisso.".into());
    }
    Ok(number.into())
}

pub(crate) fn search_places_only(query: &SearchQuery) -> Result<SearchOutcome, String> {
    let primary = places::search(&query.what, &query.where_, &query.country, &query.language)?;
    merge_activities(
        query,
        Ok(primary),
        Ok(SearchOutcome::Results(SearchResponse {
            display_where: Some(query.where_.clone()),
            current_page: 1,
            is_last_page: true,
            results: Vec::new(),
        })),
    )
}

pub(crate) fn search_activities(query: &SearchQuery) -> Result<SearchOutcome, String> {
    if !query.reverse && query.kind == DirectoryKind::PagineGialle && query.country != "it" {
        return search_places_only(query);
    }
    if query.reverse || query.kind != DirectoryKind::PagineGialle || query.page > 1 {
        return search(query);
    }
    let (primary, legacy) = std::thread::scope(|scope| {
        let worker = scope
            .spawn(|| places::search(&query.what, &query.where_, &query.country, &query.language));
        let legacy = search(query);
        (
            worker
                .join()
                .unwrap_or_else(|_| Err("Ricerca luoghi interrotta.".into())),
            legacy,
        )
    });
    merge_activities(query, primary, legacy)
}

fn merge_activities(
    query: &SearchQuery,
    primary: Result<Vec<places::Place>, String>,
    legacy: Result<SearchOutcome, String>,
) -> Result<SearchOutcome, String> {
    let primary = match primary {
        Ok(items) if !items.is_empty() => items,
        _ => return legacy,
    };
    let mut response = match legacy {
        Ok(SearchOutcome::Results(response)) => response,
        Ok(ambiguous) => return Ok(ambiguous),
        Err(_) => SearchResponse {
            display_where: Some(query.where_.clone()),
            current_page: 1,
            is_last_page: true,
            results: Vec::new(),
        },
    };
    response.results.retain(|item| {
        !primary.iter().any(|place| {
            places::same_place(
                place,
                &item.name,
                item.address.as_deref(),
                item.city.as_deref(),
                &item.phones,
            )
        })
    });
    let mut results: Vec<_> = primary
        .into_iter()
        .map(|place| {
            let detail = DetailResponse {
                title: place.nome.clone(),
                body: place.body(&places::PlaceText::new(|key| {
                    let language =
                        serde_json::from_value(serde_json::Value::String(query.language.clone()))
                            .unwrap_or(crate::settings::Language::English);
                    crate::i18n::tr(language, &format!("places.detail.{key}"))
                })),
            };
            SearchResult {
                id: format!("place:{}", place.id),
                name: place.nome,
                address: place.indirizzo,
                city: None,
                province: None,
                category: (!place.categorie.is_empty()).then(|| place.categorie.join(", ")),
                phones: place.telefoni,
                detail: Some(detail),
            }
        })
        .collect();
    results.extend(response.results);
    response.results = results;
    Ok(SearchOutcome::Results(response))
}

pub(crate) fn load_result_detail(
    query: &SearchQuery,
    result: &SearchResult,
) -> Result<DetailResponse, String> {
    match &result.detail {
        Some(detail) => Ok(detail.clone()),
        None => load_detail(query, &result.id),
    }
}

pub(crate) fn search(query: &SearchQuery) -> Result<SearchOutcome, String> {
    let trimmed_what = query.what.trim();
    if trimmed_what.is_empty() {
        return Err(format!(
            "Il campo {} è vuoto.",
            query.kind.primary_field_name()
        ));
    }

    let url = build_search_url(query)?;
    let bytes = crate::curl_client::CurlClient::fetch_url_impersonated(&url)
        .map_err(|err| format!("Impossibile cercare in {}: {err}", query.kind.label()))?;
    let xml = String::from_utf8(bytes)
        .map_err(|err| format!("Risposta XML {} non valida: {err}", query.kind.label()))?;
    let mut outcome = parse_search_response(&xml, query.kind)?;
    if query.reverse {
        let number = normalize_phone(&query.what)?;
        if matches!(outcome, SearchOutcome::AmbiguousAddress(_)) {
            return Err("Ricerca per numero non disponibile. Verifica il numero e riprova.".into());
        }
        if let SearchOutcome::Results(ref mut response) = outcome {
            response.results.retain(|r| {
                r.phones
                    .iter()
                    .any(|p| normalize_phone(p).ok().as_deref() == Some(number.as_str()))
            });
        }
    }
    Ok(outcome)
}

// Keep the source in combined result IDs so details use the matching directory.
pub(crate) fn search_phone(query: &SearchQuery) -> Result<SearchOutcome, String> {
    let mut combined = SearchResponse {
        display_where: None,
        current_page: query.page,
        is_last_page: true,
        results: Vec::new(),
    };
    let mut succeeded = false;
    let mut errors = Vec::new();
    for (kind, prefix) in [
        (DirectoryKind::PagineBianche, "pb:"),
        (DirectoryKind::PagineGialle, "pg:"),
    ] {
        match search(&SearchQuery {
            kind,
            reverse: true,
            where_: String::new(),
            ..query.clone()
        }) {
            Ok(SearchOutcome::Results(response)) => {
                succeeded = true;
                combined.is_last_page &= response.is_last_page;
                combined
                    .results
                    .extend(response.results.into_iter().map(|mut item| {
                        item.id = format!("{prefix}{}", item.id);
                        item
                    }));
            }
            Ok(_) => errors.push("Ricerca per numero non disponibile.".to_string()),
            Err(error) => errors.push(error),
        }
    }
    if !succeeded || (combined.results.is_empty() && !errors.is_empty()) {
        return Err(errors.join("\n"));
    }
    Ok(SearchOutcome::Results(combined))
}

pub(crate) fn load_detail(query: &SearchQuery, id: &str) -> Result<DetailResponse, String> {
    if query.reverse {
        for (prefix, kind) in [
            ("pb:", DirectoryKind::PagineBianche),
            ("pg:", DirectoryKind::PagineGialle),
        ] {
            if let Some(id) = id.strip_prefix(prefix) {
                return load_detail(
                    &SearchQuery {
                        kind,
                        ..query.clone()
                    },
                    id,
                );
            }
        }
    }
    let trimmed_id = id.trim();
    if trimmed_id.is_empty() {
        return Err("Risultato non valido: identificativo mancante.".to_string());
    }

    let url = build_detail_url(query, trimmed_id)?;
    let bytes = crate::curl_client::CurlClient::fetch_url_impersonated(&url).map_err(|err| {
        format!(
            "Impossibile caricare il dettaglio di {}: {err}",
            query.kind.label()
        )
    })?;
    let xml = String::from_utf8(bytes).map_err(|err| {
        format!(
            "Risposta dettaglio {} non valida: {err}",
            query.kind.label()
        )
    })?;
    parse_detail_response(&xml, query.kind)
}

fn build_search_url(query: &SearchQuery) -> Result<String, String> {
    let mut serializer = Serializer::new(String::new());
    serializer.append_pair("client", &italiaonline_client()?);
    serializer.append_pair("version", &italiaonline_version()?);
    append_directory_query(&mut serializer, query, None)?;
    Ok(format!(
        "{}{}?{}",
        italiaonline_base_url()?,
        query.kind.search_endpoint()?,
        serializer.finish()
    ))
}

fn build_detail_url(query: &SearchQuery, id: &str) -> Result<String, String> {
    let mut serializer = Serializer::new(String::new());
    serializer.append_pair("client", &italiaonline_client()?);
    serializer.append_pair("version", &italiaonline_version()?);
    append_directory_query(&mut serializer, query, Some(id))?;
    Ok(format!(
        "{}{}?{}",
        italiaonline_base_url()?,
        query.kind.detail_endpoint()?,
        serializer.finish()
    ))
}

// Query semantics are independent of the credentials used by the endpoint.
fn append_directory_query(
    serializer: &mut Serializer<'_, String>,
    query: &SearchQuery,
    detail_id: Option<&str>,
) -> Result<(), String> {
    if let Some(id) = detail_id {
        serializer.append_pair("id", id);
    }
    let what = if query.reverse {
        normalize_phone(&query.what)?
    } else {
        query.what.trim().to_string()
    };
    serializer.append_pair("what", &what);
    if !query.reverse && !query.where_.trim().is_empty() {
        serializer.append_pair("where", query.where_.trim());
    }
    if detail_id.is_none() && query.page > 1 {
        serializer.append_pair("page", &query.page.to_string());
    }
    Ok(())
}

fn parse_search_response(xml: &str, kind: DirectoryKind) -> Result<SearchOutcome, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path: Vec<Vec<u8>> = Vec::new();
    let mut search = SearchAccumulator {
        current_page: 1,
        ..SearchAccumulator::default()
    };
    let mut results = Vec::new();
    let mut current_result: Option<SearchResult> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = event.name().as_ref().to_vec();
                if name.as_slice() == b"result" {
                    current_result = Some(SearchResult {
                        id: String::new(),
                        name: String::new(),
                        address: None,
                        city: None,
                        province: None,
                        category: None,
                        phones: Vec::new(),
                        detail: None,
                    });
                }
                path.push(name);
            }
            Ok(Event::End(event)) => {
                if event.name().as_ref() == b"result"
                    && let Some(result) = current_result.take()
                    && !result.id.trim().is_empty()
                    && !result.name.trim().is_empty()
                {
                    results.push(result);
                }
                let _unused = path.pop();
            }
            Ok(Event::Text(text)) => {
                let decoded = text.decode().map_err(|err| {
                    format!("Risposta XML {} non decodificabile: {err}", kind.label())
                })?;
                assign_search_text(&path, decoded.trim(), &mut search, current_result.as_mut());
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => {
                return Err(format!("Risposta XML {} non valida: {err}", kind.label()));
            }
        }
    }

    if search.status.as_deref() == Some("302") {
        let places = search
            .places
            .into_iter()
            .map(|place| place.trim().to_string())
            .filter(|place| !place.is_empty())
            .collect::<Vec<_>>();
        return Ok(SearchOutcome::AmbiguousAddress(AmbiguousPlaceResponse {
            places,
        }));
    }

    if search.status.as_deref() != Some("200") {
        return Err(format!(
            "Ricerca {} non riuscita (status {}).",
            kind.label(),
            search.status.unwrap_or_else(|| "sconosciuto".to_string())
        ));
    }

    let parsed_result_count = search.result_count.unwrap_or(results.len());
    deduplicate_search_results(&mut results);
    let computed_is_last_page = search.is_last_page.unwrap_or_else(|| {
        if parsed_result_count == 0 {
            true
        } else if let Some(max_results) = search.max_results {
            search.current_page.saturating_mul(parsed_result_count) >= max_results
        } else {
            true
        }
    });

    Ok(SearchOutcome::Results(SearchResponse {
        display_where: search.display_where,
        current_page: search.current_page,
        is_last_page: computed_is_last_page,
        results,
    }))
}

fn assign_search_text(
    path: &[Vec<u8>],
    text: &str,
    search: &mut SearchAccumulator,
    current_result: Option<&mut SearchResult>,
) {
    if text.is_empty() {
        return;
    }
    match path {
        [response, status_tag]
            if response.as_slice() == b"response" && status_tag.as_slice() == b"status" =>
        {
            search.status = Some(text.to_string());
        }
        [response, where_tag]
            if response.as_slice() == b"response" && where_tag.as_slice() == b"where" =>
        {
            search.display_where = Some(text.to_string());
        }
        [response, current_page_tag]
            if response.as_slice() == b"response"
                && current_page_tag.as_slice() == b"current_page" =>
        {
            if let Ok(value) = text.parse::<usize>() {
                search.current_page = value.max(1);
            }
        }
        [response, result_count_tag]
            if response.as_slice() == b"response"
                && result_count_tag.as_slice() == b"result_count" =>
        {
            if let Ok(value) = text.parse::<usize>() {
                search.result_count = Some(value);
            }
        }
        [response, max_results_tag]
            if response.as_slice() == b"response"
                && max_results_tag.as_slice() == b"max_results" =>
        {
            if let Ok(value) = text.parse::<usize>() {
                search.max_results = Some(value);
            }
        }
        [response, is_last_page_tag]
            if response.as_slice() == b"response"
                && is_last_page_tag.as_slice() == b"isLastPage" =>
        {
            search.is_last_page = Some(text == "1");
        }
        [response, places_tag, place_tag, address_tag]
            if response.as_slice() == b"response"
                && places_tag.as_slice() == b"places"
                && place_tag.as_slice() == b"place"
                && address_tag.as_slice() == b"address" =>
        {
            search.places.push(text.to_string());
        }
        [response, results_tag, result_tag, field]
            if response.as_slice() == b"response"
                && results_tag.as_slice() == b"results"
                && result_tag.as_slice() == b"result" =>
        {
            if let Some(result) = current_result {
                match field.as_slice() {
                    b"id" => result.id = text.to_string(),
                    b"name" => result.name = text.to_string(),
                    b"address" => result.address = Some(text.to_string()),
                    b"city" => result.city = Some(text.to_string()),
                    b"province" => result.province = Some(text.to_string()),
                    b"category" => result.category = Some(text.to_string()),
                    _ => {}
                }
            }
        }
        [
            response,
            results_tag,
            result_tag,
            phones_tag,
            phone_tag,
            number_tag,
        ] if response.as_slice() == b"response"
            && results_tag.as_slice() == b"results"
            && result_tag.as_slice() == b"result"
            && phones_tag.as_slice() == b"phones"
            && phone_tag.as_slice() == b"phone"
            && number_tag.as_slice() == b"number" =>
        {
            if let Some(result) = current_result {
                result.phones.push(text.to_string());
            }
        }
        _ => {}
    }
}

fn deduplicate_search_results(results: &mut Vec<SearchResult>) {
    let mut seen = HashSet::new();
    results.retain(|result| {
        let phones = result
            .phones
            .iter()
            .map(|phone| phone.trim().to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join("|");
        let key = format!(
            "content:{}|{}|{}|{}|{}|{}",
            result.name.trim().to_ascii_lowercase(),
            result
                .category
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase(),
            result
                .address
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase(),
            result
                .city
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase(),
            result
                .province
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase(),
            phones
        );
        seen.insert(key)
    });
}

fn parse_detail_response(xml: &str, kind: DirectoryKind) -> Result<DetailResponse, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path: Vec<Vec<u8>> = Vec::new();
    let mut detail = DetailAccumulator::default();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => path.push(event.name().as_ref().to_vec()),
            Ok(Event::End(_)) => {
                let _unused = path.pop();
            }
            Ok(Event::Text(text)) => {
                let decoded = text.decode().map_err(|err| {
                    format!("Dettaglio XML {} non decodificabile: {err}", kind.label())
                })?;
                assign_detail_text(&path, decoded.trim(), &mut detail);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => {
                return Err(format!("Dettaglio XML {} non valido: {err}", kind.label()));
            }
        }
    }

    if detail.status.as_deref() != Some("200") {
        return Err(format!(
            "Dettaglio {} non disponibile (status {}).",
            kind.label(),
            detail.status.unwrap_or_else(|| "sconosciuto".to_string())
        ));
    }

    let final_title = detail
        .title
        .clone()
        .unwrap_or_else(|| kind.label().to_string());
    Ok(DetailResponse {
        title: final_title.clone(),
        body: format_detail_body(&final_title, &detail),
    })
}

fn assign_detail_text(path: &[Vec<u8>], text: &str, detail: &mut DetailAccumulator) {
    if text.is_empty() {
        return;
    }
    match path {
        [response, status_tag]
            if response.as_slice() == b"response" && status_tag.as_slice() == b"status" =>
        {
            detail.status = Some(text.to_string());
        }
        [response, detail_tag, field]
            if response.as_slice() == b"response" && detail_tag.as_slice() == b"detail" =>
        {
            match field.as_slice() {
                b"name" => detail.title = Some(text.to_string()),
                b"description" => detail.description = Some(text.to_string()),
                b"category" => detail.category = Some(text.to_string()),
                b"address" => detail.address = Some(text.to_string()),
                b"city" => detail.city = Some(text.to_string()),
                b"province" => detail.province = Some(text.to_string()),
                b"email_address" => detail.emails.push(text.to_string()),
                b"web_address" | b"visualsite" | b"urldetail" => {
                    detail.websites.push(text.to_string())
                }
                _ => {}
            }
        }
        [response, detail_tag, phones_tag, phone_tag, number_tag]
            if response.as_slice() == b"response"
                && detail_tag.as_slice() == b"detail"
                && phones_tag.as_slice() == b"phones"
                && phone_tag.as_slice() == b"phone"
                && number_tag.as_slice() == b"number" =>
        {
            detail.phones.push(text.to_string());
        }
        [response, detail_tag, wwws_tag, _, url_tag]
            if response.as_slice() == b"response"
                && detail_tag.as_slice() == b"detail"
                && wwws_tag.as_slice() == b"wwws"
                && url_tag.as_slice() == b"url" =>
        {
            detail.websites.push(text.to_string());
        }
        [response, detail_tag, urldetail_tag]
            if response.as_slice() == b"response"
                && detail_tag.as_slice() == b"detail"
                && urldetail_tag.as_slice() == b"urldetail" =>
        {
            detail.public_url = Some(text.to_string());
        }
        _ => {}
    }
}

fn format_detail_body(title: &str, detail: &DetailAccumulator) -> String {
    let mut lines = vec![title.to_string()];

    if let Some(value) = detail
        .category
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(String::new());
        lines.push(format!("Categoria: {value}"));
    }

    if let Some(value) = detail
        .description
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(String::new());
        lines.push("Descrizione:".to_string());
        lines.push(value.to_string());
    }

    if detail.address.is_some() || detail.city.is_some() || detail.province.is_some() {
        lines.push(String::new());
        lines.push("Indirizzo:".to_string());
        if let Some(value) = detail
            .address
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            lines.push(value.to_string());
        }
        let locality = format_locality_line(detail.city.as_deref(), detail.province.as_deref());
        if !locality.is_empty() {
            lines.push(locality);
        }
    }

    if !detail.phones.is_empty() {
        lines.push(String::new());
        lines.push("Telefoni:".to_string());
        lines.extend(detail.phones.iter().cloned());
    }

    if !detail.emails.is_empty() {
        lines.push(String::new());
        lines.push("Email:".to_string());
        lines.extend(detail.emails.iter().cloned());
    }

    let unique_websites = detail
        .websites
        .iter()
        .filter_map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() || is_italiaonline_directory_url(trimmed) {
                return None;
            }
            Some(trimmed.to_string())
        })
        .collect::<Vec<_>>();
    let unique_websites = dedupe_trimmed(&unique_websites);
    if !unique_websites.is_empty() {
        lines.push(String::new());
        lines.push("Siti web:".to_string());
        lines.extend(unique_websites);
    }

    if let Some(value) = detail
        .public_url
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(String::new());
        lines.push("Scheda web:".to_string());
        lines.push(value.to_string());
    }

    lines.join("\r\n")
}

fn format_locality_line(city: Option<&str>, province: Option<&str>) -> String {
    match (
        city.map(str::trim).filter(|value| !value.is_empty()),
        province.map(str::trim).filter(|value| !value.is_empty()),
    ) {
        (Some(city), Some(province)) => format!("{city} ({province})"),
        (Some(city), None) => city.to_string(),
        (None, Some(province)) => province.to_string(),
        (None, None) => String::new(),
    }
}

fn dedupe_trimmed(values: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() || out.iter().any(|item: &String| item == trimmed) {
            continue;
        }
        out.push(trimmed.to_string());
    }
    out
}

fn is_italiaonline_directory_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    matches!(
        host.trim_start_matches("www.")
            .to_ascii_lowercase()
            .as_str(),
        "paginegialle.it" | "paginebianche.it"
    )
}

fn italiaonline_base_url() -> Result<String, String> {
    decode_italiaonline_url(ITALIAONLINE_BASE_URL_B64)
}

fn italiaonline_client() -> Result<String, String> {
    decode_italiaonline_url(ITALIAONLINE_CLIENT_B64)
}

fn italiaonline_version() -> Result<String, String> {
    decode_italiaonline_url(ITALIAONLINE_VERSION_B64)
}

fn decode_italiaonline_url(encoded: &str) -> Result<String, String> {
    let key = resolve_italiaonline_secret_key()?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|err| format!("URL Italiaonline offuscato non valido: {err}"))?;
    let decoded: Vec<u8> = bytes
        .into_iter()
        .enumerate()
        .map(|(index, byte)| byte ^ key[index % key.len()])
        .collect();
    String::from_utf8(decoded)
        .map_err(|err| format!("URL Italiaonline decodificato non valido: {err}"))
}

fn resolve_italiaonline_secret_key() -> Result<Vec<u8>, String> {
    if let Some(secret_key) = crate::settings::load_saved_rai_luce_code() {
        let trimmed = secret_key.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.as_bytes().to_vec());
        }
    }
    Err("Chiave Luce mancante: inserisci il codice nelle impostazioni RSS/Podcast.".to_string())
}

#[cfg(test)]
mod activities_tests {
    use super::*;

    fn query() -> SearchQuery {
        SearchQuery {
            kind: DirectoryKind::PagineGialle,
            what: "bar".into(),
            where_: "Torino".into(),
            page: 1,
            reverse: false,
            country: "it".into(),
            language: "it".into(),
        }
    }

    fn place() -> places::Place {
        serde_json::from_value(serde_json::json!({"id":"new","nome":"Bar Uno","indirizzo":"Via Roma, 12, Torino",
            "telefoni":["01112345"],"categorie":["Bar"],"descrizione":null,"stelle":4.5,"numero_recensioni":23})).unwrap()
    }

    fn legacy() -> SearchOutcome {
        SearchOutcome::Results(SearchResponse {
            display_where: Some("Torino".into()),
            current_page: 1,
            is_last_page: false,
            results: vec![
                SearchResult {
                    id: "old".into(),
                    name: "Bar Uno".into(),
                    address: Some("Via Roma 12".into()),
                    city: Some("Torino".into()),
                    province: None,
                    category: None,
                    phones: vec!["01112345".into()],
                    detail: None,
                },
                SearchResult {
                    id: "other".into(),
                    name: "Bar Due".into(),
                    address: None,
                    city: None,
                    province: None,
                    category: None,
                    phones: vec![],
                    detail: None,
                },
            ],
        })
    }

    #[test]
    fn merge_preserves_legacy_pagination_and_uses_inline_details() {
        let SearchOutcome::Results(response) =
            merge_activities(&query(), Ok(vec![place()]), Ok(legacy())).unwrap()
        else {
            panic!("unexpected ambiguity");
        };
        assert_eq!(
            response
                .results
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>(),
            ["place:new", "other"]
        );
        assert!(!response.is_last_page);
        let detail = load_result_detail(&query(), &response.results[0]).unwrap();
        assert!(detail.body.contains("4.5 su 5 stelle (23 recensioni)"));
        assert!(response.results[1].detail.is_none());
    }

    #[test]
    fn either_source_can_fail_without_losing_the_other() {
        let SearchOutcome::Results(old) =
            merge_activities(&query(), Err("unavailable".into()), Ok(legacy())).unwrap()
        else {
            panic!();
        };
        assert_eq!(old.results.len(), 2);
        let SearchOutcome::Results(new) =
            merge_activities(&query(), Ok(vec![place()]), Err("unavailable".into())).unwrap()
        else {
            panic!();
        };
        assert_eq!(new.results[0].id, "place:new");
        assert!(
            merge_activities(
                &query(),
                Err("unavailable".into()),
                Err("unavailable".into())
            )
            .is_err()
        );
    }

    #[test]
    fn ambiguous_legacy_address_still_requests_selection() {
        let outcome = merge_activities(
            &query(),
            Ok(vec![place()]),
            Ok(SearchOutcome::AmbiguousAddress(AmbiguousPlaceResponse {
                places: vec!["Via Roma".into()],
            })),
        )
        .unwrap();
        assert!(matches!(outcome, SearchOutcome::AmbiguousAddress(_)));
    }
}

#[cfg(test)]
mod reverse_tests {
    use super::*;

    fn query_url(query: &SearchQuery, detail_id: Option<&str>) -> Url {
        let mut serializer = Serializer::new(String::new());
        append_directory_query(&mut serializer, query, detail_id).unwrap();
        Url::parse(&format!("https://example.invalid/?{}", serializer.finish())).unwrap()
    }

    #[test]
    fn reverse_number_normalization_and_url() {
        for number in ["02 02 02", "+39 (02) 02-02", "0039 02.02.02"] {
            assert_eq!(normalize_phone(number).unwrap(), "020202");
        }
        for invalid in ["", "123", "Milano", "+33 123456789", "02+0202"] {
            assert!(normalize_phone(invalid).is_err());
        }
        let query = SearchQuery {
            kind: DirectoryKind::PagineBianche,
            what: "+39 (02) 0202".into(),
            where_: "Roma".into(),
            page: 2,
            reverse: true,
            country: "it".into(),
            language: "it".into(),
        };
        let url = query_url(&query, None);
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params.get("what").unwrap(), "020202");
        assert_eq!(params.get("page").unwrap(), "2");
        assert!(!params.contains_key("where"));
        let detail = query_url(&query, Some("sample"));
        assert!(
            !detail
                .query_pairs()
                .any(|(k, _)| k == "where" || k == "page")
        );
        assert!(
            detail
                .query_pairs()
                .any(|(k, v)| k == "id" && v == "sample")
        );
        assert!(
            detail
                .query_pairs()
                .any(|(k, v)| k == "what" && v == "020202")
        );
        let normal = SearchQuery {
            reverse: false,
            what: "Comune".into(),
            ..query
        };
        let url = query_url(&normal, None);
        assert!(url.query_pairs().any(|(k, v)| k == "where" && v == "Roma"));
    }
}
