use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;

use crate::settings::Language;

const BASE_URL: &str = "https://sonarpad.com/api/tmdb.php";
const USER_AGENT: &str = concat!("Sonarpad/", env!("CARGO_PKG_VERSION"));
const CLIENT_TOKEN: &str = match option_env!("SONARPAD_ROUTE_CLIENT_TOKEN") {
    Some(token) => token,
    None => "",
};
const TOKEN_HEADER: &str = "X-Sonarpad-Route-Token";

#[derive(Debug, Clone, Deserialize)]
pub struct CinemaMovie {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub release_date: String,
}

#[derive(Debug, Deserialize)]
struct MovieListResponse {
    #[serde(default)]
    results: Vec<CinemaMovie>,
}

#[derive(Debug, Deserialize)]
struct TrailerListResponse {
    #[serde(default)]
    results: Vec<TrailerEntry>,
}

#[derive(Debug, Deserialize)]
struct TrailerEntry {
    #[serde(default)]
    key: String,
    #[serde(default)]
    site: String,
    #[serde(default, rename = "type")]
    video_type: String,
}

#[derive(Clone)]
pub struct CinemaClient {
    client: Client,
}

impl Default for CinemaClient {
    fn default() -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(25))
            .build()
            .unwrap_or_else(|error| {
                crate::log_debug(&format!("Cinema HTTP client setup failed: {error}"));
                Client::new()
            });
        Self { client }
    }
}

impl CinemaClient {
    pub fn now_playing(&self, language: Language) -> Result<Vec<CinemaMovie>, String> {
        self.movie_list("now_playing", language)
    }

    pub fn upcoming(&self, language: Language) -> Result<Vec<CinemaMovie>, String> {
        self.movie_list("upcoming", language)
    }

    pub fn trailer_url(&self, movie_id: i64, language: Language) -> Result<Option<String>, String> {
        let localized = self.fetch_trailers(movie_id, tmdb_language(language))?;
        if let Some(url) = first_youtube_trailer(localized) {
            return Ok(Some(url));
        }

        if language != Language::English {
            let english = self.fetch_trailers(movie_id, "en-US")?;
            return Ok(first_youtube_trailer(english));
        }

        Ok(None)
    }

    fn movie_list(&self, action: &str, language: Language) -> Result<Vec<CinemaMovie>, String> {
        let response = self
            .client
            .get(BASE_URL)
            .header(TOKEN_HEADER, CLIENT_TOKEN)
            .header("Accept", "application/json")
            .query(&[("action", action), ("language", tmdb_language(language))])
            .send()
            .map_err(|error| format!("Cinema network error: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Cinema server error: {error}"))?;

        let mut movies = response
            .json::<MovieListResponse>()
            .map_err(|error| format!("Invalid cinema response: {error}"))?
            .results;
        movies.retain(|movie| movie.id > 0 && !movie.title.trim().is_empty());
        Ok(movies)
    }

    fn fetch_trailers(
        &self,
        movie_id: i64,
        language: &'static str,
    ) -> Result<Vec<TrailerEntry>, String> {
        let movie_id = movie_id.to_string();
        let response = self
            .client
            .get(BASE_URL)
            .header(TOKEN_HEADER, CLIENT_TOKEN)
            .header("Accept", "application/json")
            .query(&[
                ("action", "trailer"),
                ("movie_id", movie_id.as_str()),
                ("language", language),
            ])
            .send()
            .map_err(|error| format!("Cinema trailer network error: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Cinema trailer server error: {error}"))?;

        response
            .json::<TrailerListResponse>()
            .map(|value| value.results)
            .map_err(|error| format!("Invalid cinema trailer response: {error}"))
    }
}

fn first_youtube_trailer(entries: Vec<TrailerEntry>) -> Option<String> {
    entries
        .into_iter()
        .find(|entry| {
            entry.site.eq_ignore_ascii_case("youtube")
                && entry.video_type.eq_ignore_ascii_case("trailer")
                && !entry.key.trim().is_empty()
        })
        .map(|entry| format!("https://www.youtube.com/watch?v={}", entry.key.trim()))
}

fn tmdb_language(language: Language) -> &'static str {
    match language {
        Language::Italian => "it-IT",
        Language::German => "de-DE",
        Language::English => "en-US",
        Language::Spanish => "es-ES",
        Language::Portuguese => "pt-PT",
        Language::PortugueseBrazilian => "pt-BR",
        Language::Swedish => "sv-SE",
        Language::Vietnamese => "vi-VN",
        Language::Czech => "cs-CZ",
        Language::Polish => "pl-PL",
        Language::French => "fr-FR",
        Language::Serbian => "sr-RS",
        Language::Ukrainian => "uk-UA",
        Language::Lithuanian => "lt-LT",
        Language::Russian => "ru-RU",
        Language::Chinese => "zh-CN",
        Language::Hindi => "hi-IN",
        Language::Indonesian => "id-ID",
    }
}
