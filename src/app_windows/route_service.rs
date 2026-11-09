use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::fmt;
use url::Url;

use crate::i18n;
use crate::settings::Language;

const DEFAULT_BASE_URL: &str = "https://sonarpad.com/api";
const USER_AGENT: &str = concat!("Sonarpad/", env!("CARGO_PKG_VERSION"));
const ROUTE_API_CLIENT_TOKEN: &str = match option_env!("SONARPAD_ROUTE_CLIENT_TOKEN") {
    Some(token) => token,
    None => "",
};
const ROUTE_API_TOKEN_HEADER: &str = "X-Sonarpad-Route-Token";
const SERVER_UNAUTHORIZED_CLIENT_ERROR: &str = "Client non autorizzato.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteProfile {
    Walking,
    Cycling,
    Driving,
    Wheelchair,
    Transit,
}

impl RouteProfile {
    pub fn api_value(self) -> &'static str {
        match self {
            RouteProfile::Walking => "foot-walking",
            RouteProfile::Cycling => "cycling-regular",
            RouteProfile::Driving => "driving-car",
            RouteProfile::Wheelchair => "wheelchair",
            RouteProfile::Transit => "transit",
        }
    }

    pub fn label(self, language: Language) -> String {
        let key = match self {
            RouteProfile::Walking => "route.profile.walking",
            RouteProfile::Cycling => "route.profile.cycling",
            RouteProfile::Driving => "route.profile.driving",
            RouteProfile::Wheelchair => "route.profile.wheelchair",
            RouteProfile::Transit => "route.profile.transit",
        };
        i18n::tr(language, key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutePreference {
    Fastest,
    Shortest,
}

impl RoutePreference {
    pub fn api_value(self) -> &'static str {
        match self {
            RoutePreference::Fastest => "fastest",
            RoutePreference::Shortest => "shortest",
        }
    }

    pub fn label(self, language: Language) -> String {
        let key = match self {
            RoutePreference::Fastest => "route.preference.fastest",
            RoutePreference::Shortest => "route.preference.shortest",
        };
        i18n::tr(language, key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAvoid {
    None,
    Highways,
    Tollways,
    HighwaysAndTollways,
}

impl RouteAvoid {
    pub fn api_value(self) -> &'static str {
        match self {
            RouteAvoid::None => "",
            RouteAvoid::Highways => "highways",
            RouteAvoid::Tollways => "tollways",
            RouteAvoid::HighwaysAndTollways => "highways,tollways",
        }
    }

    pub fn label(self, language: Language) -> String {
        let key = match self {
            RouteAvoid::None => "route.avoid.none",
            RouteAvoid::Highways => "route.avoid.highways",
            RouteAvoid::Tollways => "route.avoid.tollways",
            RouteAvoid::HighwaysAndTollways => "route.avoid.highways_tollways",
        };
        i18n::tr(language, key)
    }
}

#[derive(Clone)]
pub struct RouteDeparture {
    pub time: String,
    pub time_zone: String,
}

pub fn default_departure_zone(country: &str) -> &'static str {
    match country.to_lowercase().as_str() {
        "it" => "Europe/Rome",
        "fr" => "Europe/Paris",
        "es" => "Europe/Madrid",
        "pt" => "Europe/Lisbon",
        "br" => "America/Sao_Paulo",
        "pl" => "Europe/Warsaw",
        "cz" => "Europe/Prague",
        "de" => "Europe/Berlin",
        "nl" => "Europe/Amsterdam",
        "gb" | "uk" => "Europe/London",
        "ua" => "Europe/Kyiv",
        "ro" => "Europe/Bucharest",
        "tr" => "Europe/Istanbul",
        "cn" => "Asia/Shanghai",
        "au" => "Australia/Sydney",
        "ca" => "America/Toronto",
        "us" => "America/New_York",
        _ => "UTC",
    }
}

#[derive(Clone)]
pub struct RouteOptions {
    pub departure: Option<RouteDeparture>,
    pub profile: RouteProfile,
    pub preference: RoutePreference,
    pub avoid: RouteAvoid,
    pub include_municipalities: bool,
    pub language: Language,
    pub country: String,
}

#[derive(Debug, Clone)]
pub struct RouteClient {
    client: Client,
    base_url: String,
}

impl Default for RouteClient {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

impl RouteClient {
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(40))
            .build()
            .unwrap_or_else(|error| {
                crate::log_debug(&format!(
                    "Route HTTP client user-agent setup failed: {error}"
                ));
                Client::new()
            });

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub fn geocode(
        &self,
        address: &str,
        language: Language,
        country: &str,
    ) -> Result<Vec<GeocodeCandidate>, RouteError> {
        let address = address.trim();

        if address.is_empty() {
            return Err(RouteError::InvalidInput(i18n::tr(
                language,
                "route.error.address_search",
            )));
        }

        match self.fetch_primary_geocode(address, language, country) {
            Ok(results) if !results.is_empty() => {
                crate::log_debug(&format!(
                    "route_geocode: provider=google candidates={}",
                    results.len()
                ));
                return Ok(results);
            }
            Ok(_) => crate::log_debug(
                "route_geocode: provider=ors fallback=true reason=no_usable_google_suggestions",
            ),
            Err(error) => crate::log_debug(&format!(
                "route_geocode: provider=ors fallback=true reason={error}"
            )),
        }

        // Preserve the existing ORS lookup and query simplification as fallback.
        let results = self.fetch_geocode(address, language, country)?;

        // Se i risultati sono solo la città (fallback del geocoder), prova a semplificare la query
        if is_all_city_fallback(&results, address) {
            if let Some(simplified) = simplify_query(address)
                && let Ok(fallback_results) = self.fetch_geocode(&simplified, language, country)
                && !fallback_results.is_empty()
                && !is_all_city_fallback(&fallback_results, &simplified)
            {
                return Ok(fallback_results);
            }

            // Prova un secondo livello di semplificazione (rimuovendo un'altra parola)
            if let Some(more_simplified) = simplify_query_more(address)
                && let Ok(fallback_results) =
                    self.fetch_geocode(&more_simplified, language, country)
                && !fallback_results.is_empty()
                && !is_all_city_fallback(&fallback_results, &more_simplified)
            {
                return Ok(fallback_results);
            }
        }

        Ok(results)
    }

    fn fetch_primary_geocode(
        &self,
        address: &str,
        language: Language,
        country: &str,
    ) -> Result<Vec<GeocodeCandidate>, RouteError> {
        let response: PrimaryGeocodeResponse = self
            .client
            .post(format!("{}/percorsi.php", self.base_url))
            .timeout(std::time::Duration::from_secs(10))
            .header(ROUTE_API_TOKEN_HEADER, ROUTE_API_CLIENT_TOKEN)
            .json(&serde_json::json!({
                "azione": "indirizzi", "q": address,
                "paese": country.to_ascii_lowercase(),
                "lingua": primary_route_language(language),
            }))
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| route_network_error(language, error.to_string()))?
            .json()
            .map_err(|error| route_invalid_response_error(language, error.to_string()))?;
        let data = response
            .data
            .filter(|_| response.ok)
            .ok_or_else(|| RouteError::Server(i18n::tr(language, "route.error.address_search")))?;
        // A number added to a street label is not a verified house coordinate.
        if data
            .alternative
            .iter()
            .any(|item| item.civico_da_verificare == Some(true))
        {
            return Ok(Vec::new());
        }
        Ok(data
            .alternative
            .into_iter()
            .filter_map(|item| {
                let latitude = item.latitude?;
                let longitude = item.longitude?;
                let label = item.indirizzo.trim().to_string();
                if label.is_empty()
                    || !latitude.is_finite()
                    || !longitude.is_finite()
                    || latitude.abs() > 90.0
                    || longitude.abs() > 180.0
                {
                    return None;
                }
                Some(GeocodeCandidate {
                    name: item.nome.unwrap_or_else(|| label.clone()),
                    label,
                    country: country.to_string(),
                    region: String::new(),
                    county: String::new(),
                    locality: String::new(),
                    postalcode: String::new(),
                    latitude,
                    longitude,
                    requires_confirmation: item.tipo.as_deref() != Some("indirizzo"),
                })
            })
            .collect())
    }

    fn fetch_geocode(
        &self,
        q: &str,
        language: Language,
        country: &str,
    ) -> Result<Vec<GeocodeCandidate>, RouteError> {
        let url = format!("{}/ors_geocode.php", self.base_url);
        let boundary_country = route_country_alpha3(country);

        let response: GeocodeApiResponse = self
            .client
            .get(url)
            .header(ROUTE_API_TOKEN_HEADER, ROUTE_API_CLIENT_TOKEN)
            .query(&[
                ("q", q),
                ("size", "20"),
                ("layers", "address,street,venue"),
                ("sources", "osm,oa"),
                ("boundary.country", boundary_country),
                ("language", route_backend_language(language)),
            ])
            .send()
            .map_err(|error| route_network_error(language, error.to_string()))?
            .json()
            .map_err(|error| route_invalid_response_error(language, error.to_string()))?;

        if !response.ok {
            return Err(RouteError::Server(route_server_error(
                language,
                response.error,
                "route.error.address_search",
            )));
        }

        Ok(response.results)
    }

    pub fn route_between_coordinates(
        &self,
        from: &GeocodeCandidate,
        to: &GeocodeCandidate,
        options: RouteOptions,
    ) -> Result<RouteResult, RouteError> {
        if options.profile != RouteProfile::Wheelchair {
            match self.primary_route(from, to, &options) {
                Ok(route) => {
                    crate::log_debug(&format!(
                        "route_result: provider=google fallback=false profile={} paths={}",
                        options.profile.api_value(),
                        route.paths.len()
                    ));
                    return Ok(route);
                }
                Err(error) if options.profile == RouteProfile::Transit => return Err(error),
                Err(error) => crate::log_debug(&format!(
                    "route_fallback: from=google to=ors reason={error}"
                )),
            }
        }
        let boundary_country = route_country_alpha3(&options.country);
        let url = route_endpoint_url(
            &self.base_url,
            &[
                ("from_lat", from.latitude.to_string()),
                ("from_lon", from.longitude.to_string()),
                ("to_lat", to.latitude.to_string()),
                ("to_lon", to.longitude.to_string()),
                ("profile", options.profile.api_value().to_string()),
                ("preference", options.preference.api_value().to_string()),
                ("avoid", options.avoid.api_value().to_string()),
                (
                    "include_municipalities",
                    if options.include_municipalities {
                        "1"
                    } else {
                        "0"
                    }
                    .to_string(),
                ),
                (
                    "language",
                    route_backend_language(options.language).to_string(),
                ),
                ("boundary.country", boundary_country.to_string()),
            ],
        )
        .map_err(|error| route_network_error(options.language, error.to_string()))?;

        let response: RouteApiResponse = self
            .client
            .get(url)
            .header(ROUTE_API_TOKEN_HEADER, ROUTE_API_CLIENT_TOKEN)
            .send()
            .map_err(|error| route_network_error(options.language, error.to_string()))?
            .json()
            .map_err(|error| route_invalid_response_error(options.language, error.to_string()))?;

        if !response.ok {
            return Err(RouteError::Server(route_server_error(
                options.language,
                response.error,
                "route.error.calculation",
            )));
        }

        let paths = response.into_paths();
        crate::log_debug(&format!(
            "route_result: provider=ors fallback={} profile={} paths={}",
            options.profile != RouteProfile::Wheelchair,
            options.profile.api_value(),
            paths.len()
        ));
        Ok(RouteResult {
            profile: options.profile,
            preference: options.preference,
            avoid: options.avoid,
            from_label: from.label.clone(),
            to_label: to.label.clone(),
            paths,
        })
    }

    fn primary_route(
        &self,
        from: &GeocodeCandidate,
        to: &GeocodeCandidate,
        options: &RouteOptions,
    ) -> Result<RouteResult, RouteError> {
        let mode = match options.profile {
            RouteProfile::Driving => "auto",
            RouteProfile::Walking => "piedi",
            RouteProfile::Cycling => "bici",
            RouteProfile::Transit => "pullman",
            _ => unreachable!(),
        };
        let preference = if options.profile == RouteProfile::Transit {
            "fastest"
        } else {
            options.preference.api_value()
        };
        let response: PrimaryRouteResponse = self
            .client
            .post(format!("{}/percorsi.php", self.base_url))
            .timeout(std::time::Duration::from_secs(60))
            .header(ROUTE_API_TOKEN_HEADER, ROUTE_API_CLIENT_TOKEN)
            .json(&serde_json::json!({
                "da": format!("{},{}", from.latitude, from.longitude),
                "a": format!("{},{}", to.latitude, to.longitude),
                "mezzo": mode,
                "partenza": if options.profile == RouteProfile::Transit { options.departure.as_ref().map(|d| d.time.as_str()).unwrap_or("adesso") } else { "adesso" },
                "fuso": if options.profile == RouteProfile::Transit { options.departure.as_ref().map(|d| d.time_zone.as_str()).unwrap_or("") } else { "" },
                "preferenza": preference, "evita": options.avoid.api_value(),
                "includi_comuni": options.include_municipalities,
                "paese": options.country, "lingua": primary_route_language(options.language)
            }))
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| route_network_error(options.language, e.to_string()))?
            .json()
            .map_err(|e| route_invalid_response_error(options.language, e.to_string()))?;
        let data = response
            .data
            .filter(|data| {
                response.ok
                    && !data.percorsi.is_empty()
                    && data.preferenza == preference
                    && data.evita == options.avoid.api_value()
                    && (!options.include_municipalities || data.includi_comuni == Some(true))
            })
            .ok_or_else(|| {
                RouteError::Server(i18n::tr(options.language, "route.error.calculation"))
            })?;
        if data
            .percorsi
            .iter()
            .any(|p| p.istruzioni.is_empty() || p.istruzioni.iter().any(|s| s.trim().is_empty()))
        {
            return Err(RouteError::Server(i18n::tr(
                options.language,
                "route.error.calculation",
            )));
        }
        Ok(RouteResult {
            profile: options.profile,
            preference: options.preference,
            avoid: options.avoid,
            from_label: from.display_label(),
            to_label: to.display_label(),
            paths: data
                .percorsi
                .into_iter()
                .map(|path| RoutePath {
                    notices: path.avvisi,
                    transit_details: path.mezzi,
                    distance_meters: path.distanza_metri,
                    duration_seconds: path.durata_secondi,
                    distance_text: Some(path.distanza),
                    duration_text: Some(path.durata),
                    steps: path
                        .istruzioni
                        .into_iter()
                        .map(|instruction| RouteStep {
                            instruction,
                            distance_meters: 0.0,
                            duration_seconds: 0.0,
                        })
                        .collect(),
                    geometry: Vec::new(),
                    municipality_changes: Vec::new(),
                })
                .collect(),
        })
    }

    pub fn route_from_addresses(
        &self,
        from_address: &str,
        to_address: &str,
        options: RouteOptions,
    ) -> Result<RouteRequestResult, RouteError> {
        let from_candidates = self.geocode(from_address, options.language, &options.country)?;
        let to_candidates = self.geocode(to_address, options.language, &options.country)?;

        if from_candidates.is_empty() {
            return Err(RouteError::NotFound(i18n::tr(
                options.language,
                "route.error.from_not_found",
            )));
        }

        if to_candidates.is_empty() {
            return Err(RouteError::NotFound(i18n::tr(
                options.language,
                "route.error.to_not_found",
            )));
        }

        if candidates_need_selection(&from_candidates) || candidates_need_selection(&to_candidates)
        {
            return Ok(RouteRequestResult::NeedsSelection {
                from_candidates,
                to_candidates,
                profile: options.profile,
                preference: options.preference,
                avoid: options.avoid,
                include_municipalities: options.include_municipalities,
            });
        }

        let route =
            self.route_between_coordinates(&from_candidates[0], &to_candidates[0], options)?;

        Ok(RouteRequestResult::Ready(route))
    }
}

fn route_endpoint_url(base_url: &str, params: &[(&str, String)]) -> Result<Url, url::ParseError> {
    let mut url = Url::parse(&format!("{}/ors_route.php", base_url.trim_end_matches('/')))?;
    url.query_pairs_mut()
        .extend_pairs(params.iter().map(|(key, value)| (*key, value)));
    Ok(url)
}

#[derive(Debug, Clone)]
pub enum RouteRequestResult {
    Ready(RouteResult),
    NeedsSelection {
        from_candidates: Vec<GeocodeCandidate>,
        to_candidates: Vec<GeocodeCandidate>,
        profile: RouteProfile,
        preference: RoutePreference,
        avoid: RouteAvoid,
        include_municipalities: bool,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeocodeCandidate {
    pub label: String,
    pub name: String,
    pub country: String,
    #[allow(dead_code)]
    pub region: String,
    #[allow(dead_code)]
    pub county: String,
    pub locality: String,
    #[allow(dead_code)]
    pub postalcode: String,
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default)]
    pub requires_confirmation: bool,
}

pub fn candidates_need_selection(candidates: &[GeocodeCandidate]) -> bool {
    candidates.len() > 1
        || candidates
            .first()
            .is_some_and(|item| item.requires_confirmation)
}

pub fn prefer_nearby_candidates(candidates: &mut [GeocodeCandidate], from: &GeocodeCandidate) {
    let distance = |candidate: &GeocodeCandidate| {
        let latitude = ((candidate.latitude - from.latitude).to_radians() / 2.0).sin();
        let longitude = ((candidate.longitude - from.longitude).to_radians() / 2.0).sin();
        latitude * latitude
            + from.latitude.to_radians().cos()
                * candidate.latitude.to_radians().cos()
                * longitude
                * longitude
    };
    // Stable ordering keeps all alternatives, including more distant namesakes.
    candidates.sort_by(|left, right| distance(left).total_cmp(&distance(right)));
}

impl GeocodeCandidate {
    pub fn display_label(&self) -> String {
        if !self.label.trim().is_empty() {
            return self.label.clone();
        }

        let mut parts = Vec::new();

        if !self.name.trim().is_empty() {
            parts.push(self.name.as_str());
        }

        if !self.locality.trim().is_empty() {
            parts.push(self.locality.as_str());
        }

        if !self.country.trim().is_empty() {
            parts.push(self.country.as_str());
        }

        parts.join(", ")
    }
}

#[derive(Debug, Clone)]
pub struct RouteResult {
    pub profile: RouteProfile,
    pub preference: RoutePreference,
    pub avoid: RouteAvoid,
    pub from_label: String,
    pub to_label: String,
    pub paths: Vec<RoutePath>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoutePath {
    #[serde(default)]
    pub notices: Vec<String>,
    #[serde(default)]
    pub transit_details: Vec<String>,
    #[serde(default)]
    pub distance_text: Option<String>,
    #[serde(default)]
    pub duration_text: Option<String>,
    pub distance_meters: f64,
    pub duration_seconds: f64,
    #[serde(default)]
    pub steps: Vec<RouteStep>,
    #[serde(default)]
    pub geometry: Vec<[f64; 2]>,
    #[serde(default)]
    pub municipality_changes: Vec<MunicipalityChange>,
}

impl RoutePath {
    pub fn distance_label(&self, language: Language) -> String {
        self.distance_text
            .clone()
            .unwrap_or_else(|| format_distance_for_language(self.distance_meters, language))
    }
    pub fn duration_label(&self, language: Language) -> String {
        self.duration_text
            .clone()
            .unwrap_or_else(|| format_duration_for_language(self.duration_seconds, language))
    }
}

#[derive(Deserialize)]
struct PrimaryGeocodeResponse {
    ok: bool,
    data: Option<PrimaryGeocodeData>,
}

#[derive(Deserialize)]
struct PrimaryGeocodeData {
    #[serde(default)]
    alternative: Vec<PrimaryGeocodeCandidate>,
}

#[derive(Deserialize)]
struct PrimaryGeocodeCandidate {
    #[serde(default)]
    indirizzo: String,
    nome: Option<String>,
    tipo: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    civico_da_verificare: Option<bool>,
}

#[derive(Deserialize)]
struct PrimaryRouteResponse {
    ok: bool,
    data: Option<PrimaryRouteData>,
}
#[derive(Deserialize)]
struct PrimaryRouteData {
    includi_comuni: Option<bool>,
    preferenza: String,
    evita: String,
    percorsi: Vec<PrimaryRoutePath>,
}
#[derive(Deserialize)]
struct PrimaryRoutePath {
    distanza_metri: f64,
    durata_secondi: f64,
    distanza: String,
    durata: String,
    istruzioni: Vec<String>,
    #[serde(default)]
    avvisi: Vec<String>,
    #[serde(default)]
    mezzi: Vec<String>,
}

fn primary_route_language(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::English => "en",
        Language::German => "de",
        Language::Spanish => "es",
        Language::Portuguese => "pt",
        Language::PortugueseBrazilian => "pt-BR",
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

#[derive(Debug, Clone, Serialize)]
pub struct RouteMapData {
    pub from_label: String,
    pub to_label: String,
    pub suggested_filename: String,
    pub geometry: Vec<[f64; 2]>,
}

impl RouteResult {
    pub fn suggested_filename(&self, language: Language) -> String {
        route_filename(language, &self.from_label, &self.to_label)
    }

    pub fn map_data(&self, language: Language) -> Option<RouteMapData> {
        let path = self.paths.first()?;
        if path.geometry.len() < 2 {
            return None;
        }

        Some(RouteMapData {
            from_label: self.from_label.clone(),
            to_label: self.to_label.clone(),
            suggested_filename: self.suggested_filename(language),
            geometry: path.geometry.clone(),
        })
    }

    pub fn format_for_speech_or_text(&self, language: Language) -> String {
        let mut output = String::new();

        output.push_str(&i18n::tr(language, "route.found_prefix"));
        output.push(' ');
        output.push_str(&self.profile.label(language));
        output.push(' ');
        if self
            .paths
            .first()
            .is_none_or(|path| path.duration_text.is_none())
        {
            output.push_str(&self.preference.label(language));
        }
        if self.avoid != RouteAvoid::None {
            output.push_str(", ");
            output.push_str(&self.avoid.label(language));
        }
        output.push(' ');
        output.push_str(&i18n::tr(language, "route.found_suffix"));
        output.push_str("\n\n");

        output.push_str(&i18n::tr(language, "route.from_result"));
        output.push(' ');
        output.push_str(&self.from_label);
        output.push('\n');

        output.push_str(&i18n::tr(language, "route.to_result"));
        output.push(' ');
        output.push_str(&self.to_label);
        output.push('\n');

        if self.paths.is_empty() {
            output.push('\n');
            output.push_str(&i18n::tr(language, "route.no_route_available"));
            return output;
        }

        if self
            .paths
            .first()
            .map(|path| path.geometry.len() >= 2)
            .unwrap_or(false)
        {
            output.push_str(&i18n::tr(language, "route.map.available_hint"));
            output.push('\n');
        }

        if self.paths.len() > 1 {
            output.push('\n');
            output.push_str(&i18n::tr(language, "route.alternative_routes_found"));
            output.push(' ');
            output.push_str(&self.paths.len().to_string());
            output.push_str(".\n");
        }

        for (path_index, path) in self.paths.iter().enumerate() {
            output.push('\n');
            if path_index == 0 {
                output.push_str(&i18n::tr(language, "route.main_route"));
            } else {
                output.push_str(&i18n::tr(language, "route.alternative_route"));
                output.push(' ');
                output.push_str(&path_index.to_string());
            }
            output.push_str(".\n");

            output.push_str(&i18n::tr(language, "route.distance"));
            output.push(' ');
            output.push_str(&path.distance_label(language));
            output.push_str(".\n");

            output.push_str(&i18n::tr(language, "route.estimated_duration"));
            output.push(' ');
            output.push_str(&path.duration_label(language));
            output.push_str(".\n");

            output.push('\n');
            output.push_str(&i18n::tr(language, "route.directions"));
            output.push('\n');

            append_route_steps_with_municipality_changes(&mut output, path, language);
            if !path.notices.is_empty() {
                output.push('\n');
                for notice in &path.notices {
                    output.push_str(notice);
                    output.push('\n');
                }
            }
        }

        output
    }
}

pub fn route_filename(language: Language, from_label: &str, to_label: &str) -> String {
    let template = i18n::tr_f(
        language,
        "route.filename_template",
        &[("from", from_label), ("to", to_label)],
    );
    sanitize_route_filename(&template)
}

fn sanitize_route_filename(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len());

    for ch in name.chars() {
        if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || ch.is_control() {
            sanitized.push(' ');
        } else {
            sanitized.push(ch);
        }
    }

    let sanitized = sanitized
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches('.')
        .trim()
        .to_string();

    if sanitized.is_empty() {
        return "route".to_string();
    }

    sanitized.chars().take(150).collect()
}

#[derive(Debug, Clone, Deserialize)]
pub struct MunicipalityChange {
    pub name: String,
    #[serde(default)]
    pub distance_meters: f64,
    #[allow(dead_code)]
    pub coordinate: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RouteStep {
    pub instruction: String,
    pub distance_meters: f64,
    #[allow(dead_code)]
    pub duration_seconds: f64,
}

#[derive(Debug, Deserialize)]
struct GeocodeApiResponse {
    ok: bool,
    error: Option<String>,
    #[serde(default)]
    results: Vec<GeocodeCandidate>,
}

#[derive(Debug, Deserialize)]
struct RouteApiResponse {
    ok: bool,
    error: Option<String>,
    distance_meters: Option<f64>,
    duration_seconds: Option<f64>,
    #[serde(default)]
    steps: Vec<RouteStep>,
    #[serde(default)]
    routes: Vec<RoutePath>,
}

impl RouteApiResponse {
    fn into_paths(self) -> Vec<RoutePath> {
        if !self.routes.is_empty() {
            return self.routes;
        }

        vec![RoutePath {
            notices: Vec::new(),
            transit_details: Vec::new(),
            distance_text: None,
            duration_text: None,
            distance_meters: self.distance_meters.unwrap_or(0.0),
            duration_seconds: self.duration_seconds.unwrap_or(0.0),
            steps: self.steps,
            geometry: Vec::new(),
            municipality_changes: Vec::new(),
        }]
    }
}

fn route_backend_language(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::German => "de",
        Language::English => "en",
        Language::Spanish => "es",
        Language::Portuguese | Language::PortugueseBrazilian => "pt",
        Language::French => "fr",
        Language::Russian => "ru",
        Language::Chinese => "zh",
        _ => "en",
    }
}

fn route_country_alpha3(country: &str) -> &'static str {
    match country.trim().to_ascii_lowercase().as_str() {
        "ae" => "ARE",
        "al" => "ALB",
        "am" => "ARM",
        "ar" => "ARG",
        "at" => "AUT",
        "au" => "AUS",
        "az" => "AZE",
        "bd" => "BGD",
        "be" => "BEL",
        "bg" => "BGR",
        "bn" => "BRN",
        "bo" => "BOL",
        "br" => "BRA",
        "bt" => "BTN",
        "bw" => "BWA",
        "ca" => "CAN",
        "ch" => "CHE",
        "ci" => "CIV",
        "cl" => "CHL",
        "cm" => "CMR",
        "cn" => "CHN",
        "co" => "COL",
        "cr" => "CRI",
        "cy" => "CYP",
        "cz" => "CZE",
        "de" => "DEU",
        "dk" => "DNK",
        "do" => "DOM",
        "dz" => "DZA",
        "ec" => "ECU",
        "ee" => "EST",
        "eg" => "EGY",
        "es" => "ESP",
        "et" => "ETH",
        "fi" => "FIN",
        "fr" => "FRA",
        "gb" => "GBR",
        "ge" => "GEO",
        "gh" => "GHA",
        "gr" => "GRC",
        "gt" => "GTM",
        "hk" => "HKG",
        "hn" => "HND",
        "hr" => "HRV",
        "hu" => "HUN",
        "id" => "IDN",
        "ie" => "IRL",
        "il" => "ISR",
        "in" => "IND",
        "is" => "ISL",
        "it" => "ITA",
        "jm" => "JAM",
        "jo" => "JOR",
        "jp" => "JPN",
        "ke" => "KEN",
        "kg" => "KGZ",
        "kh" => "KHM",
        "kr" => "KOR",
        "kw" => "KWT",
        "kz" => "KAZ",
        "la" => "LAO",
        "lb" => "LBN",
        "lk" => "LKA",
        "lt" => "LTU",
        "lu" => "LUX",
        "lv" => "LVA",
        "ma" => "MAR",
        "mg" => "MDG",
        "mk" => "MKD",
        "ml" => "MLI",
        "mn" => "MNG",
        "mt" => "MLT",
        "mu" => "MUS",
        "mv" => "MDV",
        "mx" => "MEX",
        "my" => "MYS",
        "na" => "NAM",
        "ng" => "NGA",
        "ni" => "NIC",
        "nl" => "NLD",
        "no" => "NOR",
        "np" => "NPL",
        "nz" => "NZL",
        "pa" => "PAN",
        "pe" => "PER",
        "ph" => "PHL",
        "pk" => "PAK",
        "pl" => "POL",
        "pt" => "PRT",
        "py" => "PRY",
        "qa" => "QAT",
        "ro" => "ROU",
        "rs" => "SRB",
        "sa" => "SAU",
        "se" => "SWE",
        "sg" => "SGP",
        "si" => "SVN",
        "sk" => "SVK",
        "sn" => "SEN",
        "sv" => "SLV",
        "th" => "THA",
        "tn" => "TUN",
        "tr" => "TUR",
        "tw" => "TWN",
        "tz" => "TZA",
        "ua" => "UKR",
        "ug" => "UGA",
        "us" => "USA",
        "uy" => "URY",
        "uz" => "UZB",
        "ve" => "VEN",
        "vn" => "VNM",
        "zm" => "ZMB",
        "zw" => "ZWE",
        _ => "ITA",
    }
}

fn route_network_error(language: Language, message: String) -> RouteError {
    RouteError::Network(format!(
        "{} {}",
        i18n::tr(language, "route.error.network"),
        message
    ))
}

fn route_invalid_response_error(language: Language, message: String) -> RouteError {
    RouteError::InvalidResponse(format!(
        "{} {}",
        i18n::tr(language, "route.error.invalid_response"),
        message
    ))
}

fn route_server_error(language: Language, message: Option<String>, fallback_key: &str) -> String {
    match message {
        Some(message) if message.trim() == SERVER_UNAUTHORIZED_CLIENT_ERROR => {
            i18n::tr(language, "route.error.unauthorized_client")
        }
        Some(message) => message,
        None => i18n::tr(language, fallback_key),
    }
}

#[derive(Debug, Clone)]
pub enum RouteError {
    InvalidInput(String),
    Network(String),
    InvalidResponse(String),
    Server(String),
    NotFound(String),
}

impl fmt::Display for RouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RouteError::InvalidInput(message) => write!(f, "{message}"),
            RouteError::Network(message) => write!(f, "{message}"),
            RouteError::InvalidResponse(message) => write!(f, "{message}"),
            RouteError::Server(message) => write!(f, "{message}"),
            RouteError::NotFound(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for RouteError {}

fn append_route_steps_with_municipality_changes(
    output: &mut String,
    path: &RoutePath,
    language: Language,
) {
    let changes = sorted_municipality_changes(path);

    if let Some(start_municipality) = changes
        .first()
        .filter(|change| change.distance_meters <= 1.0)
    {
        output.push_str(&i18n::tr(language, "route.start_municipality"));
        output.push(' ');
        output.push_str(start_municipality.name.trim());
        output.push_str(".\n");
    }

    let mut next_change = first_route_municipality_change_index(&changes);

    if path.steps.is_empty() {
        output.push_str(&i18n::tr(language, "route.no_instruction_available"));
        output.push('\n');
        append_remaining_municipality_changes(output, &changes, next_change, 1, language);
        return;
    }

    let mut instruction_number = 1usize;
    let mut cumulative_distance = 0.0f64;

    for step in &path.steps {
        let instruction = clean_route_instruction(&step.instruction);
        let distance = format_distance_for_language(step.distance_meters, language);

        output.push_str(&instruction_number.to_string());
        output.push_str(". ");
        output.push_str(&instruction);

        if step.distance_meters > 0.0 {
            output.push_str(", ");
            output.push_str(&i18n::tr(language, "route.for_distance"));
            output.push(' ');
            output.push_str(&distance);
        }

        output.push_str(".\n");
        instruction_number += 1;

        cumulative_distance += step.distance_meters.max(0.0);

        while next_change < changes.len()
            && changes[next_change].distance_meters <= cumulative_distance + 10.0
        {
            append_municipality_instruction(
                output,
                instruction_number,
                changes[next_change],
                language,
            );
            instruction_number += 1;
            next_change += 1;
        }
    }

    append_remaining_municipality_changes(
        output,
        &changes,
        next_change,
        instruction_number,
        language,
    );
}

fn sorted_municipality_changes(path: &RoutePath) -> Vec<&MunicipalityChange> {
    let mut changes: Vec<&MunicipalityChange> = path
        .municipality_changes
        .iter()
        .filter(|change| !change.name.trim().is_empty())
        .collect();

    changes.sort_by(|left, right| left.distance_meters.total_cmp(&right.distance_meters));

    let mut seen = std::collections::HashSet::new();
    changes
        .into_iter()
        .filter(|change| seen.insert(municipality_dedupe_key(&change.name)))
        .collect()
}

fn municipality_dedupe_key(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn first_route_municipality_change_index(changes: &[&MunicipalityChange]) -> usize {
    changes
        .iter()
        .position(|change| change.distance_meters > 1.0)
        .unwrap_or(changes.len())
}

fn append_municipality_instruction(
    output: &mut String,
    instruction_number: usize,
    change: &MunicipalityChange,
    language: Language,
) {
    output.push_str(&instruction_number.to_string());
    output.push_str(". ");
    output.push_str(&i18n::tr(language, "route.enter_municipality"));
    output.push(' ');
    output.push_str(change.name.trim());
    output.push_str(".\n");
}

fn append_remaining_municipality_changes(
    output: &mut String,
    changes: &[&MunicipalityChange],
    mut next_change: usize,
    mut instruction_number: usize,
    language: Language,
) {
    while next_change < changes.len() {
        append_municipality_instruction(output, instruction_number, changes[next_change], language);
        next_change += 1;
        instruction_number += 1;
    }
}

pub fn clean_route_instruction(instruction: &str) -> String {
    instruction
        .replace("Gira svolta sinistra", "Gira a sinistra")
        .replace("Gira svolta destra", "Gira a destra")
        .replace("Gira sulla sinistra", "Gira a sinistra")
        .replace("Gira sulla destra", "Gira a destra")
        .replace("  ", " ")
        .trim()
        .to_string()
}

pub fn format_distance_for_language(meters: f64, language: Language) -> String {
    if meters < 1.0 {
        return i18n::tr(language, "route.distance.few_meters");
    }

    if meters < 1000.0 {
        return format!(
            "{} {}",
            meters.round() as i64,
            i18n::tr(language, "route.distance.meters")
        );
    }

    let kilometers = meters / 1000.0;

    if kilometers < 10.0 {
        return format!("{kilometers:.1} km").replace('.', ",");
    }

    format!("{} km", kilometers.round() as i64)
}

pub fn format_duration_for_language(seconds: f64, language: Language) -> String {
    let total_seconds = seconds.round() as i64;

    if total_seconds < 60 {
        return format!(
            "{total_seconds} {}",
            i18n::tr(language, "route.duration.seconds")
        );
    }

    let total_minutes = ((total_seconds as f64) / 60.0).round() as i64;

    if total_minutes < 60 {
        if total_minutes == 1 {
            return i18n::tr(language, "route.duration.minute_one");
        }

        return format!(
            "{total_minutes} {}",
            i18n::tr(language, "route.duration.minutes")
        );
    }

    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;

    if minutes == 0 {
        if hours == 1 {
            return i18n::tr(language, "route.duration.hour_one");
        }

        return format!("{hours} {}", i18n::tr(language, "route.duration.hours"));
    }

    if hours == 1 {
        return i18n::tr_f(
            language,
            "route.duration.hour_and_minutes",
            &[("minutes", &minutes.to_string())],
        );
    }

    i18n::tr_f(
        language,
        "route.duration.hours_and_minutes",
        &[
            ("hours", &hours.to_string()),
            ("minutes", &minutes.to_string()),
        ],
    )
}

fn is_all_city_fallback(results: &[GeocodeCandidate], original_query: &str) -> bool {
    if results.is_empty() {
        return false;
    }

    // Se la query originale è corta, non consideriamolo un fallback
    if original_query.split_whitespace().count() <= 1 {
        return false;
    }

    // Se tutti i risultati hanno postalcode vuoto e il nome è uguale alla località, è probabilmente un fallback del geocoder alla città
    results.iter().all(|c| {
        c.postalcode.is_empty()
            && (c.name == c.locality || c.name == c.region || c.name == c.country)
    })
}

fn simplify_query(query: &str) -> Option<String> {
    let mut words: Vec<&str> = query.split_whitespace().collect();
    if words.len() <= 1 {
        return None;
    }

    // Rimuovi prefissi comuni (via, corso, etc.)
    let prefixes = [
        "via", "corso", "viale", "piazza", "vicolo", "largo", "strada", "v.", "c.so", "p.zza",
        "p.za",
    ];
    if prefixes.contains(&words[0].to_lowercase().as_str()) {
        words.remove(0);
    }

    if words.is_empty() {
        return None;
    }

    Some(words.join(" "))
}

fn simplify_query_more(query: &str) -> Option<String> {
    let mut words: Vec<&str> = query.split_whitespace().collect();

    // Rimuovi il prefisso se esiste
    let prefixes = [
        "via", "corso", "viale", "piazza", "vicolo", "largo", "strada", "v.", "c.so", "p.zza",
        "p.za",
    ];
    if !words.is_empty() && prefixes.contains(&words[0].to_lowercase().as_str()) {
        words.remove(0);
    }

    // Se rimangono più di 2 parole (es. "Pierluigi Palestrina 45 Torino"), prova a rimuovere la prima parola del nome
    // Spesso i geocoder falliscono sui nomi propri composti o con refusi
    if words.len() > 2 {
        words.remove(0);
        return Some(words.join(" "));
    }

    None
}

#[cfg(test)]
mod primary_route_tests {
    use super::*;
    use std::io::{BufRead, Read, Write};
    use std::net::TcpListener;

    fn server(
        responses: Vec<(u16, &'static str)>,
    ) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "Missing route request"
                            );
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                    .unwrap();
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap();
                    }
                    request.push_str(&line);
                }
                let mut bytes = vec![0; length];
                reader.read_exact(&mut bytes).unwrap();
                request.push_str(&String::from_utf8(bytes).unwrap());
                requests.push(request);
                write!(stream, "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
            requests
        });
        (base, handle)
    }
    fn candidate() -> GeocodeCandidate {
        GeocodeCandidate {
            label: "Oxford Circus".into(),
            name: String::new(),
            country: String::new(),
            region: String::new(),
            county: String::new(),
            locality: String::new(),
            postalcode: String::new(),
            latitude: 51.5152,
            longitude: -0.1419,
            requires_confirmation: false,
        }
    }
    fn options(profile: RouteProfile) -> RouteOptions {
        RouteOptions {
            departure: None,
            profile,
            preference: RoutePreference::Fastest,
            avoid: RouteAvoid::None,
            include_municipalities: false,
            language: Language::English,
            country: "gb".into(),
        }
    }
    const PRIMARY: &str = r#"{"ok":true,"data":{"percorsi":[{"distanza":"1.2 mi","durata":"20 min","istruzioni":["Take Bus 12 at 12:30","Arrive at destination"],"avvisi":["Notice: Bus diversion"],"mezzi":["Take Bus 12 at 12:30"],"distanza_metri":1900,"durata_secondi":1200},{"distanza":"1.4 mi","durata":"24 min","istruzioni":["Take Underground Central"],"distanza_metri":2300,"durata_secondi":1440}],"preferenza":"fastest","evita":""}}"#;
    const LOCALITIES: &str = r#"{"ok":true,"data":{"percorsi":[{"distanza":"1.2 mi","durata":"20 min","istruzioni":["Locality detected along the route: London.","Take Bus 12 at 12:30"],"distanza_metri":1900,"durata_secondi":1200},{"distanza":"1.4 mi","durata":"24 min","istruzioni":["Take Underground Central"],"distanza_metri":2300,"durata_secondi":1440}],"preferenza":"fastest","evita":"","includi_comuni":true}}"#;
    const SHORTEST: &str = r#"{"ok":true,"data":{"percorsi":[{"distanza":"1.2 mi","durata":"20 min","istruzioni":["Take Bus 12 at 12:30"],"distanza_metri":1900,"durata_secondi":1200},{"distanza":"1.4 mi","durata":"24 min","istruzioni":["Take Underground Central"],"distanza_metri":2300,"durata_secondi":1440}],"preferenza":"shortest","evita":""}}"#;
    const AVOID: &str = r#"{"ok":true,"data":{"percorsi":[{"distanza":"1.2 mi","durata":"20 min","istruzioni":["Take Bus 12 at 12:30"],"distanza_metri":1900,"durata_secondi":1200},{"distanza":"1.4 mi","durata":"24 min","istruzioni":["Take Underground Central"],"distanza_metri":2300,"durata_secondi":1440}],"preferenza":"fastest","evita":"highways,tollways"}}"#;

    const ORS: &str = r#"{"ok":true,"distance_meters":1000,"duration_seconds":600,"steps":[]}"#;

    const ADDRESS: &str = r#"{"ok":true,"data":{"alternative":[{"indirizzo":"Via Francesco Cilea, 12, Torino, TO","nome":"Via Francesco Cilea, 12","tipo":"indirizzo","latitude":45.0983965,"longitude":7.7052344}]}}"#;
    const PLACES: &str = r#"{"ok":true,"data":{"alternative":[{"indirizzo":"Zerbonia Autodemolizioni, Alessandria, AL","nome":"Zerbonia Autodemolizioni","tipo":"luogo","latitude":44.8455399,"longitude":8.6805169},{"indirizzo":"IIS Romolo Zerboni, Via Paolo della Cella, Torino, TO","nome":"IIS Romolo Zerboni","tipo":"luogo","latitude":45.104888,"longitude":7.6786255},{"indirizzo":"zerboni torino","latitude":null,"longitude":null}]}}"#;
    const SINGLE_PLACE: &str = r#"{"ok":true,"data":{"alternative":[{"indirizzo":"IIS Romolo Zerboni, Via Paolo della Cella, Torino, TO","tipo":"luogo","latitude":45.104888,"longitude":7.6786255}]}}"#;
    const ORS_GEOCODE: &str = r#"{"ok":true,"results":[{"label":"Via Francesco Cilea, Torino, Italia","name":"Via Francesco Cilea","country":"Italia","region":"Torino","county":"","locality":"Torino","postalcode":"10154","latitude":45.0983965,"longitude":7.7052344}]}"#;

    #[test]
    fn place_suggestions_require_selection_and_route_to_the_chosen_coordinates() {
        let (base, server) = server(vec![(200, ADDRESS), (200, PLACES), (200, PRIMARY)]);
        let client = RouteClient::new(&base);
        let mut plan = options(RouteProfile::Driving);
        plan.language = Language::Italian;
        plan.country = "it".into();
        let result = client
            .route_from_addresses("via francesco cilea 12", "zerboni", plan.clone())
            .unwrap();
        let RouteRequestResult::NeedsSelection {
            from_candidates,
            mut to_candidates,
            ..
        } = result
        else {
            panic!("A place must be chosen before calculating the route");
        };
        assert!(!candidates_need_selection(&from_candidates));
        assert!(candidates_need_selection(&to_candidates));
        assert_eq!(
            to_candidates.len(),
            2,
            "Generic searches have no coordinates"
        );
        assert_eq!(to_candidates[0].name, "Zerbonia Autodemolizioni");
        prefer_nearby_candidates(&mut to_candidates, &from_candidates[0]);
        assert_eq!(to_candidates[0].name, "IIS Romolo Zerboni");
        assert_eq!(to_candidates[1].name, "Zerbonia Autodemolizioni");
        let result = client
            .route_between_coordinates(&from_candidates[0], &to_candidates[0], plan)
            .unwrap();
        assert!(result.to_label.contains("Via Paolo della Cella"));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 3);
        for request in &requests[..2] {
            assert!(request.starts_with("POST /percorsi.php "));
            assert!(request.contains(r#""azione":"indirizzi""#));
            assert!(request.contains(r#""paese":"it""#));
            assert!(request.contains(r#""lingua":"it""#));
        }
        assert!(requests[1].contains(r#""q":"zerboni""#));
        assert!(requests[2].contains(r#""a":"45.104888,7.6786255""#));
    }

    #[test]
    fn even_one_place_requires_confirmation_but_verified_addresses_keep_the_existing_flow() {
        let (base, place_server) = server(vec![(200, ADDRESS), (200, SINGLE_PLACE)]);
        let result = RouteClient::new(&base)
            .route_from_addresses("via Cilea 12", "zerboni", options(RouteProfile::Driving))
            .unwrap();
        let RouteRequestResult::NeedsSelection { to_candidates, .. } = result else {
            panic!("A single place still needs confirmation");
        };
        assert_eq!(to_candidates.len(), 1);
        assert!(candidates_need_selection(&to_candidates));
        assert_eq!(place_server.join().unwrap().len(), 2);

        let (base, address_server) = server(vec![(200, ADDRESS), (200, ADDRESS), (200, PRIMARY)]);
        let result = RouteClient::new(&base)
            .route_from_addresses(
                "via Cilea 12",
                "via Cilea 12",
                options(RouteProfile::Driving),
            )
            .unwrap();
        assert!(matches!(result, RouteRequestResult::Ready(_)));
        assert_eq!(address_server.join().unwrap().len(), 3);
    }

    #[test]
    fn unusable_google_suggestions_keep_ors_address_lookup_as_fallback() {
        for failure in [
            (502, "unavailable"),
            (200, "not JSON"),
            (200, r#"{"ok":false}"#),
            (200, r#"{"ok":true,"data":{"alternative":[]}}"#),
            (
                200,
                r#"{"ok":true,"data":{"alternative":[{"indirizzo":"Old server without coordinates"}]}}"#,
            ),
            (
                200,
                r#"{"ok":true,"data":{"alternative":[{"indirizzo":"Invalid point","latitude":91,"longitude":7}]}}"#,
            ),
            (
                200,
                r#"{"ok":true,"data":{"alternative":[{"indirizzo":"Via Cilea, 12","civico_da_verificare":true,"latitude":45,"longitude":7}]}}"#,
            ),
        ] {
            let (base, server) = server(vec![failure, (200, ORS_GEOCODE)]);
            let results = RouteClient::new(&base)
                .geocode("via Cilea 12", Language::Italian, "it")
                .unwrap();
            assert_eq!(results[0].latitude, 45.0983965);
            assert!(!results[0].requires_confirmation);
            let requests = server.join().unwrap();
            assert!(requests[0].starts_with("POST /percorsi.php "));
            assert!(requests[1].starts_with("GET /ors_geocode.php?"));
            assert!(requests[1].contains("boundary.country=ITA"));
            assert!(requests[1].contains("q=via+Cilea+12"));
        }
    }

    #[test]
    fn geocode_suggestions_use_the_selected_international_country_and_language() {
        let (base, server) = server(vec![(200, ADDRESS)]);
        RouteClient::new(&base)
            .geocode("10 Downing Street", Language::English, "GB")
            .unwrap();
        let requests = server.join().unwrap();
        assert!(requests[0].contains(r#""paese":"gb""#));
        assert!(requests[0].contains(r#""lingua":"en""#));
        assert!(requests[0].contains(r#""q":"10 Downing Street""#));
    }

    #[test]
    fn primary_success_preserves_alternatives_text_and_region() {
        for (profile, mode) in [
            (RouteProfile::Walking, "piedi"),
            (RouteProfile::Driving, "auto"),
            (RouteProfile::Transit, "pullman"),
        ] {
            let (base, server) = server(vec![(200, PRIMARY)]);
            let result = RouteClient::new(&base)
                .route_between_coordinates(&candidate(), &candidate(), options(profile))
                .unwrap();
            assert_eq!(result.paths.len(), 2);
            assert_eq!(result.paths[0].transit_details, ["Take Bus 12 at 12:30"]);
            assert!(result.paths[1].transit_details.is_empty());
            let text = result.format_for_speech_or_text(Language::English);
            assert!(
                text.contains("1.2 mi")
                    && text.contains("20 min")
                    && text.contains("Take Bus 12 at 12:30")
            );
            assert!(text.contains("2. Arrive at destination.\n\nNotice: Bus diversion\n"));
            assert!(!text.contains("3. Notice:"));
            let requests = server.join().unwrap();
            assert!(requests[0].starts_with("POST /percorsi.php "));
            assert!(requests[0].contains(&format!(r#""mezzo":"{mode}""#)));
            assert!(
                requests[0].contains(r#""paese":"gb""#) && requests[0].contains(r#""lingua":"en""#)
            );
        }
    }
    #[test]
    fn primary_failures_fall_back_to_ors() {
        for failure in [
            (502, "down"),
            (200, "not JSON"),
            (200, r#"{"ok":true,"data":{"percorsi":[]}}"#),
        ] {
            for profile in [RouteProfile::Walking, RouteProfile::Driving] {
                let (base, server) = server(vec![failure, (200, ORS)]);
                let result = RouteClient::new(&base)
                    .route_between_coordinates(&candidate(), &candidate(), options(profile))
                    .unwrap();
                assert_eq!(result.paths[0].distance_meters, 1000.0);
                let requests = server.join().unwrap();
                assert!(requests[0].starts_with("POST /percorsi.php "));
                assert!(requests[1].starts_with("GET /ors_route.php?"));
                assert!(requests[1].contains(profile.api_value()));
            }
        }
    }
    #[test]
    fn wheelchair_stays_on_ors() {
        let (base, server) = server(vec![(200, ORS)]);
        RouteClient::new(&base)
            .route_between_coordinates(
                &candidate(),
                &candidate(),
                options(RouteProfile::Wheelchair),
            )
            .unwrap();
        assert!(server.join().unwrap()[0].starts_with("GET /ors_route.php?"));
    }
    #[test]
    fn localities_enrich_the_same_google_route() {
        let mut plan = options(RouteProfile::Driving);
        plan.include_municipalities = true;
        let (base, primary_server) = server(vec![(200, LOCALITIES)]);
        let result = RouteClient::new(&base)
            .route_between_coordinates(&candidate(), &candidate(), plan.clone())
            .unwrap();
        assert_eq!(result.paths[0].distance_meters, 1900.0);
        assert!(result.paths[0].steps[0].instruction.contains("London"));
        let requests = primary_server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(
            requests[0].starts_with("POST /percorsi.php ")
                && requests[0].contains(r#""includi_comuni":true"#)
        );
        let (base, server) = server(vec![(200, PRIMARY), (200, ORS)]);
        RouteClient::new(&base)
            .route_between_coordinates(&candidate(), &candidate(), plan)
            .unwrap();
        assert!(server.join().unwrap()[1].contains("include_municipalities=1"));
    }
    #[test]
    fn preferences_and_avoidance_use_primary_and_require_confirmation() {
        let mut shortest = options(RouteProfile::Driving);
        shortest.preference = RoutePreference::Shortest;
        let mut avoid = options(RouteProfile::Driving);
        avoid.avoid = RouteAvoid::HighwaysAndTollways;
        for (plan, response) in [
            (shortest.clone(), SHORTEST),
            (avoid.clone(), AVOID),
            (options(RouteProfile::Cycling), PRIMARY),
        ] {
            let (base, server) = server(vec![(200, response)]);
            let result = RouteClient::new(&base)
                .route_between_coordinates(&candidate(), &candidate(), plan.clone())
                .unwrap();
            assert_eq!(result.paths[0].distance_meters, 1900.0);
            let requests = server.join().unwrap();
            assert!(requests[0].contains(&format!(
                r#""preferenza":"{}""#,
                plan.preference.api_value()
            )));
            assert!(requests[0].contains(&format!(r#""evita":"{}""#, plan.avoid.api_value())));
        }
        // Old servers or ignored options must not silently discard the user's choice.
        for plan in [shortest, avoid] {
            let (base, server) = server(vec![(200, PRIMARY), (200, ORS)]);
            RouteClient::new(&base)
                .route_between_coordinates(&candidate(), &candidate(), plan)
                .unwrap();
            assert!(server.join().unwrap()[1].starts_with("GET /ors_route.php?"));
        }
    }
    #[test]
    fn scheduled_transit_passes_time_and_zone_and_other_modes_ignore_them() {
        for profile in [RouteProfile::Transit, RouteProfile::Driving] {
            let mut plan = options(profile);
            plan.departure = Some(RouteDeparture {
                time: "18:35".into(),
                time_zone: "Europe/London".into(),
            });
            let (base, server) = server(vec![(200, PRIMARY)]);
            RouteClient::new(&base)
                .route_between_coordinates(&candidate(), &candidate(), plan)
                .unwrap();
            let requests = server.join().unwrap();
            assert_eq!(requests.len(), 1);
            let expected = if profile == RouteProfile::Transit {
                "18:35"
            } else {
                "adesso"
            };
            assert!(requests[0].contains(&format!(r#""partenza":"{expected}""#)));
            assert_eq!(
                requests[0].contains(r#""fuso":"Europe/London""#),
                profile == RouteProfile::Transit
            );
        }
    }

    #[test]
    fn transit_failure_does_not_request_an_ors_walking_route() {
        let (base, server) = server(vec![(502, "unavailable")]);
        assert!(
            RouteClient::new(&base)
                .route_between_coordinates(
                    &candidate(),
                    &candidate(),
                    options(RouteProfile::Transit)
                )
                .is_err()
        );
        assert_eq!(server.join().unwrap().len(), 1);
    }
}
