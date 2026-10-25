use crate::settings::{Language, WeatherCity};
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::Duration;
use url::Url;

const GEOCODING_ENDPOINT: &str = "https://geocoding-api.open-meteo.com/v1/search";
const FORECAST_ENDPOINT: &str = "https://api.open-meteo.com/v1/forecast";
const USER_AGENT: &str = "Sonarpad/0.7 weather";

#[derive(Clone, Debug, Default)]
pub struct WeatherCurrent {
    pub temperature_c: Option<f64>,
    pub relative_humidity: Option<f64>,
    pub weather_code: Option<i32>,
}

#[derive(Clone, Debug, Default)]
pub struct WeatherDay {
    pub date: String,
    pub max_temperature_c: Option<f64>,
    pub min_temperature_c: Option<f64>,
    pub precipitation_probability: Option<f64>,
    pub precipitation_mm: Option<f64>,
    pub wind_speed_kmh: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct WeatherForecast {
    pub current: WeatherCurrent,
    pub days: Vec<WeatherDay>,
}

#[derive(Clone, Debug)]
pub struct WeatherClient {
    client: Client,
}

impl Default for WeatherClient {
    fn default() -> Self {
        Self::new()
    }
}

impl WeatherClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|error| {
                crate::log_debug(&format!("Weather HTTP client setup failed: {error}"));
                Client::new()
            });
        Self { client }
    }

    pub fn search_city(&self, query: &str, language: Language) -> Result<Vec<WeatherCity>, String> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let mut url = Url::parse(GEOCODING_ENDPOINT)
            .map_err(|error| format!("Invalid weather geocoding URL: {error}"))?;
        url.query_pairs_mut()
            .append_pair("name", query)
            .append_pair("count", "10")
            .append_pair("language", geocoding_language(language))
            .append_pair("format", "json");

        let response = self
            .client
            .get(url)
            .send()
            .map_err(|error| format!("Weather city search failed: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Weather city search failed: {error}"))?;
        let data: Value = response
            .json()
            .map_err(|error| format!("Invalid weather city response: {error}"))?;

        let mut results = Vec::new();
        if let Some(items) = data.get("results").and_then(Value::as_array) {
            for item in items {
                let Some(name) = item.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let Some(latitude) = item.get("latitude").and_then(Value::as_f64) else {
                    continue;
                };
                let Some(longitude) = item.get("longitude").and_then(Value::as_f64) else {
                    continue;
                };
                let city = WeatherCity {
                    name: name.trim().to_string(),
                    admin1: item
                        .get("admin1")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    admin2: item
                        .get("admin2")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    admin3: item
                        .get("admin3")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    admin4: item
                        .get("admin4")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    country: item
                        .get("country")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    latitude,
                    longitude,
                };
                if !city.name.is_empty()
                    && !results.iter().any(|existing: &WeatherCity| {
                        existing.name.eq_ignore_ascii_case(&city.name)
                            && (existing.latitude - city.latitude).abs() < 0.000_001
                            && (existing.longitude - city.longitude).abs() < 0.000_001
                    })
                {
                    results.push(city);
                }
            }
        }
        Ok(results)
    }

    pub fn forecast(&self, city: &WeatherCity) -> Result<WeatherForecast, String> {
        let mut url = Url::parse(FORECAST_ENDPOINT)
            .map_err(|error| format!("Invalid weather forecast URL: {error}"))?;
        url.query_pairs_mut()
            .append_pair("latitude", &city.latitude.to_string())
            .append_pair("longitude", &city.longitude.to_string())
            .append_pair(
                "current",
                "temperature_2m,relative_humidity_2m,weather_code",
            )
            .append_pair(
                "daily",
                "temperature_2m_max,temperature_2m_min,precipitation_probability_max,precipitation_sum,wind_speed_10m_max",
            )
            .append_pair("forecast_days", "7")
            .append_pair("timezone", "auto");

        let response = self
            .client
            .get(url)
            .send()
            .map_err(|error| format!("Weather forecast request failed: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Weather forecast request failed: {error}"))?;
        let data: Value = response
            .json()
            .map_err(|error| format!("Invalid weather forecast response: {error}"))?;

        let current_value = data.get("current").unwrap_or(&Value::Null);
        let current = WeatherCurrent {
            temperature_c: current_value.get("temperature_2m").and_then(Value::as_f64),
            relative_humidity: current_value
                .get("relative_humidity_2m")
                .and_then(Value::as_f64),
            weather_code: current_value
                .get("weather_code")
                .and_then(Value::as_i64)
                .and_then(|value| i32::try_from(value).ok()),
        };

        let daily = data.get("daily").unwrap_or(&Value::Null);
        let dates = string_array(daily.get("time"));
        let max_temperatures = number_array(daily.get("temperature_2m_max"));
        let min_temperatures = number_array(daily.get("temperature_2m_min"));
        let precipitation_probabilities = number_array(daily.get("precipitation_probability_max"));
        let precipitation_sums = number_array(daily.get("precipitation_sum"));
        let wind_speeds = number_array(daily.get("wind_speed_10m_max"));

        let mut days = Vec::with_capacity(dates.len());
        for (index, date) in dates.into_iter().enumerate() {
            days.push(WeatherDay {
                date,
                max_temperature_c: value_at(&max_temperatures, index),
                min_temperature_c: value_at(&min_temperatures, index),
                precipitation_probability: value_at(&precipitation_probabilities, index),
                precipitation_mm: value_at(&precipitation_sums, index),
                wind_speed_kmh: value_at(&wind_speeds, index),
            });
        }

        if days.is_empty() {
            return Err("Weather forecast did not contain daily data".to_string());
        }

        Ok(WeatherForecast { current, days })
    }
}

fn value_at(values: &[Option<f64>], index: usize) -> Option<f64> {
    values.get(index).copied().flatten()
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn number_array(value: Option<&Value>) -> Vec<Option<f64>> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().map(Value::as_f64).collect())
        .unwrap_or_default()
}

fn geocoding_language(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::German => "de",
        Language::English => "en",
        Language::Spanish => "es",
        Language::Portuguese | Language::PortugueseBrazilian => "pt",
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
