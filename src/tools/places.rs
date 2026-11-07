//! Client for the Sonarpad places endpoint. No provider credentials in the client.
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone, Debug, Deserialize)]
pub struct Place {
    pub id: String,
    pub nome: String,
    pub indirizzo: Option<String>,
    pub telefoni: Vec<String>,
    pub categorie: Vec<String>,
    pub descrizione: Option<String>,
    pub stelle: Option<f64>,
    pub numero_recensioni: Option<u64>,
    pub sito_web: Option<String>,
    pub stato_apertura: Option<String>,
    #[serde(default)]
    pub orari_apertura: Vec<OpeningHours>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OpeningHours {
    pub giorno: String,
    pub data: Option<String>,
    pub fasce: Vec<String>,
}

pub struct PlaceText {
    pub category: String,
    pub address: String,
    pub phone: String,
    pub website: String,
    pub rating: String,
    pub reviews: String,
    pub unavailable: String,
    pub status: String,
    pub hours: String,
    pub hours_missing: String,
    pub today: String,
    pub tomorrow: String,
    pub range: String,
    pub next_day: String,
}
impl PlaceText {
    pub fn new(t: impl Fn(&str) -> String) -> Self {
        Self {
            category: t("category"),
            address: t("address"),
            phone: t("phone"),
            website: t("website"),
            rating: t("rating"),
            reviews: t("reviews"),
            unavailable: t("unavailable"),
            status: t("status"),
            hours: t("hours"),
            hours_missing: t("hours_missing"),
            today: t("today"),
            tomorrow: t("tomorrow"),
            range: t("range"),
            next_day: t("next_day"),
        }
    }
}

fn format_hours_range(value: &str, text: &PlaceText) -> String {
    fn minutes(value: &str) -> Option<u32> {
        let mut parts = value.trim().split(':');
        let hour: u32 = parts.next()?.parse().ok()?;
        let minute: u32 = parts.next().unwrap_or("0").parse().ok()?;
        if parts.next().is_some() || hour > 24 || minute > 59 || (hour == 24 && minute != 0) {
            return None;
        }
        Some(hour * 60 + minute)
    }
    let Some((start, end)) = value.split_once(['–', '-']) else {
        return value.into();
    };
    let (Some(start), Some(end)) = (minutes(start), minutes(end)) else {
        return value.into();
    };
    let suffix = if end == 0 || end == 1440 {
        ""
    } else if end < start {
        text.next_day.as_str()
    } else {
        ""
    };
    text.range
        .replace("%start%", &format!("{:02}:{:02}", start / 60, start % 60))
        .replace("%end%", &format!("{:02}:{:02}", (end / 60) % 24, end % 60))
        + suffix
}

fn opening_day_label<'a>(
    day: &'a OpeningHours,
    today: chrono::NaiveDate,
    text: &'a PlaceText,
) -> &'a str {
    let date = day
        .data
        .as_deref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    match date {
        Some(date) if date == today => &text.today,
        Some(date) if Some(date) == today.succ_opt() => &text.tomorrow,
        _ => &day.giorno,
    }
}

#[derive(Deserialize)]
struct Response {
    ok: bool,
    data: Option<Data>,
}

#[derive(Deserialize)]
struct Data {
    risultati: Vec<Place>,
}

pub fn search(
    what: &str,
    location: &str,
    country: &str,
    language: &str,
) -> Result<Vec<Place>, String> {
    search_request(
        serde_json::json!({"q": what, "citta": location, "paese": country, "lingua": language}),
    )
}

pub fn normalize_phone(input: &str) -> Option<String> {
    let input = input.trim();
    if input.chars().enumerate().any(|(i, c)| {
        !c.is_ascii_digit() && !c.is_whitespace() && !"()-./".contains(c) && !(c == '+' && i == 0)
    }) {
        return None;
    }
    let compact: String = input
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();
    let compact = compact
        .strip_prefix("00")
        .map(|s| format!("+{s}"))
        .unwrap_or(compact);
    (5..=15)
        .contains(&compact.trim_start_matches('+').len())
        .then_some(compact)
}

pub fn search_phone(number: &str, country: &str, language: &str) -> Result<Vec<Place>, String> {
    let number = normalize_phone(number).ok_or("Invalid phone number")?;
    search_request(
        serde_json::json!({"q": number, "telefono": number, "paese": country, "lingua": language}),
    )
}

fn search_request(params: serde_json::Value) -> Result<Vec<Place>, String> {
    let token = option_env!("SONARPAD_ROUTE_CLIENT_TOKEN").unwrap_or("");
    if token.is_empty() {
        return Err("Ricerca luoghi non configurata in questa versione.".into());
    }
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(40))
        .build()
        .map_err(|_| "Impossibile avviare la ricerca luoghi.")?
        .post("https://sonarpad.com/api/luoghi.php")
        .header("X-Sonarpad-Route-Token", token)
        .header(
            "User-Agent",
            concat!("Sonarpad/", env!("CARGO_PKG_VERSION")),
        )
        .json(&params)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|_| "Servizio di ricerca luoghi temporaneamente non disponibile.")?
        .json::<Response>()
        .map_err(|_| "Risposta della ricerca luoghi non valida.")?;
    if !response.ok {
        return Err("Ricerca luoghi non riuscita.".into());
    }
    let data = response
        .data
        .ok_or("Risultati della ricerca luoghi mancanti.")?;
    Ok(data
        .risultati
        .into_iter()
        .filter(|p| !p.id.is_empty() && !p.nome.trim().is_empty())
        .collect())
}

impl Place {
    pub fn rating_text(&self, text: &PlaceText) -> Option<String> {
        self.stelle
            .filter(|v| v.is_finite() && (1.0..=5.0).contains(v))
            .map(|stars| {
                let mut rating = text.rating.replace("%stars%", &stars.to_string());
                if let Some(count) = self.numero_recensioni {
                    rating.push_str(&text.reviews.replace("%count%", &count.to_string()));
                }
                rating
            })
    }

    pub fn body(&self, text: &PlaceText) -> String {
        let mut lines = vec![self.nome.clone()];
        if !self.categorie.is_empty() {
            lines.push(format!("{}: {}", text.category, self.categorie.join(", ")));
        }
        lines.push(format!(
            "{}: {}",
            text.address,
            self.indirizzo
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(&text.unavailable)
        ));
        lines.push(format!(
            "{}: {}",
            text.phone,
            if self.telefoni.is_empty() {
                text.unavailable.clone()
            } else {
                self.telefoni.join(" - ")
            }
        ));
        if let Some(website) = self
            .sito_web
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            lines.push(format!("{}: {website}", text.website));
        }
        if let Some(description) = &self.descrizione {
            lines.push(description.clone());
        }
        if let Some(rating) = self.rating_text(text) {
            lines.push(rating);
        }
        if let Some(status) = self
            .stato_apertura
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            lines.push(format!("{}: {status}", text.status));
        }
        let today = chrono::Local::now().date_naive();
        let hours: Vec<String> = self
            .orari_apertura
            .iter()
            .filter_map(|day| {
                let ranges: Vec<String> = day
                    .fasce
                    .iter()
                    .map(String::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .map(|value| format_hours_range(value, text))
                    .collect();
                if day.giorno.trim().is_empty() || ranges.is_empty() {
                    return None;
                }
                Some(format!(
                    "{}: {}",
                    opening_day_label(day, today, text),
                    ranges.join("; ")
                ))
            })
            .collect();
        lines.push(if hours.is_empty() {
            text.hours_missing.clone()
        } else {
            format!("{}:\r\n{}", text.hours, hours.join("\r\n"))
        });
        lines.join("\r\n\r\n")
    }
}

fn normalized(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Conservative matching: equal name plus address/locality, or phone when an address is absent.
pub fn same_place(
    place: &Place,
    name: &str,
    address: Option<&str>,
    city: Option<&str>,
    phones: &[String],
) -> bool {
    if normalized(&place.nome) != normalized(name) {
        return false;
    }
    let primary = normalized(place.indirizzo.as_deref().unwrap_or(""));
    let street = normalized(address.unwrap_or(""));
    if !primary.is_empty() && !street.is_empty() {
        let city = normalized(city.unwrap_or(""));
        return (primary == street
            || primary
                .strip_prefix(&street)
                .is_some_and(|suffix| suffix.starts_with(' ')))
            && (city.is_empty() || primary.contains(&city));
    }
    phones.iter().any(|phone| {
        let phone: String = phone.chars().filter(char::is_ascii_digit).collect();
        !phone.is_empty()
            && place.telefoni.iter().any(|other| {
                other
                    .chars()
                    .filter(char::is_ascii_digit)
                    .collect::<String>()
                    == phone
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn international_phone_normalization_preserves_prefix_and_zeroes() {
        for (input, expected) in [
            ("02 0202", "020202"),
            ("+39 (02) 0202", "+39020202"),
            ("0033 1 58 59 22 22", "+33158592222"),
            ("01 58 59 22 22", "0158592222"),
            ("+1 (212) 123-4567", "+12121234567"),
        ] {
            assert_eq!(normalize_phone(input).as_deref(), Some(expected));
        }
        for input in ["", "123", "phone 020202", "02+0202", "+1234567890123456"] {
            assert_eq!(normalize_phone(input), None);
        }
    }

    fn italian() -> PlaceText {
        let values: serde_json::Value =
            serde_json::from_str(include_str!("../../i18n/it.json")).unwrap();
        PlaceText::new(|key| {
            values[format!("places.detail.{key}")]
                .as_str()
                .unwrap()
                .into()
        })
    }

    fn sample() -> Place {
        serde_json::from_value(serde_json::json!({"id":"1","nome":"Bar Uno","indirizzo":"Via Roma, 12, Torino",
            "telefoni":["011 12345"],"categorie":["Bar"],"descrizione":null,"stelle":4.5,"numero_recensioni":23})).unwrap()
    }

    #[test]
    fn english_details_preserve_content_and_localize_labels() {
        let values: serde_json::Value =
            serde_json::from_str(include_str!("../../i18n/en.json")).unwrap();
        let text = PlaceText::new(|key| {
            values[format!("places.detail.{key}")]
                .as_str()
                .unwrap()
                .into()
        });
        let mut place = sample();
        place.sito_web = Some("https://example.org".into());
        let body = place.body(&text);
        for expected in [
            "Category: Bar",
            "Address: Via Roma, 12, Torino",
            "Phone: 011 12345",
            "Website: https://example.org",
            "Rating: 4.5 out of 5 stars (23 reviews)",
            "Opening hours not available",
        ] {
            assert!(body.contains(expected), "{body}");
        }
        assert!(!body.contains("Categoria:"));
        assert_eq!(format_hours_range("05–00", &text), "from 05:00 to 00:00");
        assert_eq!(
            format_hours_range("19–02", &text),
            "from 19:00 to 02:00 the following day"
        );
        let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
        let day = OpeningHours {
            giorno: "Friday".into(),
            data: Some("2026-11-06".into()),
            fasce: vec![],
        };
        assert_eq!(opening_day_label(&day, today, &text), "Today");
    }

    #[test]
    fn website_is_shown_only_when_available() {
        let mut place = sample();
        assert!(!place.body(&italian()).contains("Sito web:"));
        place.sito_web = Some("https://example.org/ristorante/".into());
        assert!(
            place
                .body(&italian())
                .contains("Sito web: https://example.org/ristorante/")
        );
        place.sito_web = Some(" ".into());
        assert!(!place.body(&italian()).contains("Sito web:"));
    }

    #[test]
    fn readable_hours_and_relative_days() {
        assert_eq!(
            format_hours_range("05–00", &italian()),
            "dalle 05:00 alle 00:00"
        );
        assert_eq!(
            format_hours_range("05–22:30", &italian()),
            "dalle 05:00 alle 22:30"
        );
        assert_eq!(
            format_hours_range("19–02", &italian()),
            "dalle 19:00 alle 02:00 del giorno successivo"
        );
        assert_eq!(format_hours_range("Chiuso", &italian()), "Chiuso");
        assert_eq!(
            format_hours_range("Aperto 24 ore", &italian()),
            "Aperto 24 ore"
        );
        let today = chrono::NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        let mut day = OpeningHours {
            giorno: "giovedì".into(),
            data: Some("2026-12-31".into()),
            fasce: vec![],
        };
        assert_eq!(opening_day_label(&day, today, &italian()), "Oggi");
        day.data = Some("2027-01-01".into());
        assert_eq!(opening_day_label(&day, today, &italian()), "Domani");
        day.data = None;
        assert_eq!(opening_day_label(&day, today, &italian()), "giovedì");
    }

    #[test]
    fn opening_hours_preserve_split_shifts_and_closures() {
        let mut place = sample();
        assert!(place.body(&italian()).contains("Orari non disponibili"));
        place.stato_apertura = Some("Chiuso · Apre alle ore 12:30".into());
        place.orari_apertura = serde_json::from_value(serde_json::json!([
            {"giorno":"venerdì","data":"2026-11-06","fasce":["12:30–14:30","19:30–23"]},
            {"giorno":"lunedì","data":null,"fasce":["Chiuso"]}
        ]))
        .unwrap();
        let body = place.body(&italian());
        assert!(body.contains("Stato apertura: Chiuso · Apre alle ore 12:30"));
        assert!(body.contains("dalle 12:30 alle 14:30; dalle 19:30 alle 23:00"));
        assert!(body.contains(": Chiuso"));
        assert!(!body.contains("Orari non disponibili"));
    }

    #[test]
    fn same_branch_is_merged_but_another_address_is_not() {
        let place = sample();
        assert!(same_place(
            &place,
            "BAR UNO",
            Some("Via Roma 12"),
            Some("Torino"),
            &[]
        ));
        assert!(!same_place(
            &place,
            "Bar Uno",
            Some("Via Roma 1"),
            Some("Torino"),
            &place.telefoni
        ));
        assert!(!same_place(
            &place,
            "Bar Uno",
            Some("Via Roma 120"),
            Some("Torino"),
            &place.telefoni
        ));
        assert!(!same_place(
            &place,
            "Bar Uno",
            Some("Via Roma 12"),
            Some("Milano"),
            &place.telefoni
        ));
    }

    #[test]
    fn details_include_rating_without_inventing_review_text() {
        let mut place = sample();
        assert!(
            place
                .body(&italian())
                .contains("4.5 su 5 stelle (23 recensioni)")
        );
        place.stelle = None;
        place.telefoni.clear();
        assert!(!place.body(&italian()).contains("Valutazione:"));
        assert!(place.body(&italian()).contains("Telefono: non disponibile"));
    }
}
