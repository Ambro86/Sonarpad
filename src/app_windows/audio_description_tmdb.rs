//! Optional cast lookup. TMDB uses the Sonarpad Cinema proxy; animation can use AniList.
#[path = "audio_description_anilist.rs"]
mod anilist;
use super::cinema_service::CinemaClient;
use crate::i18n;
use crate::settings::Language;
use crate::tools::audio_description_bridge::{BridgeCharacter, TmdbCharacterReference};
use serde_json::Value;

fn number(value: &Value, key: &str) -> i64 {
    value[key].as_i64().unwrap_or(0)
}
fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").trim().to_string()
}
fn entries(value: &Value, key: &str) -> Vec<Value> {
    value[key].as_array().cloned().unwrap_or_default()
}

pub(super) fn lookup(
    title: &str,
    year: &str,
    language: Language,
    mut choose: impl FnMut(&str, Vec<String>) -> Result<usize, String>,
    mut choose_tv: impl FnMut(i64, Vec<(i64, String)>) -> Result<(i64, Option<i64>), String>,
) -> Result<Vec<BridgeCharacter>, String> {
    let client = CinemaClient::default();
    let parameters = vec![("query", title.to_string()), ("year", year.to_string())];
    let mut productions = Vec::new();
    // A failed search must not silently omit potentially ambiguous matches.
    for kind in ["movie", "tv"] {
        let response = client.character_lookup(&format!("search_{kind}"), language, &parameters)?;
        for production in entries(&response, "results") {
            if number(&production, "id") > 0 {
                productions.push((kind, production));
            }
        }
    }
    if productions.is_empty() {
        return Err(i18n::tr(language, "audio_description.tmdb.no_results"));
    }
    let options = productions
        .iter()
        .map(|(kind, item)| {
            let (name, date) = if *kind == "movie" {
                ("title", "release_date")
            } else {
                ("name", "first_air_date")
            };
            format!(
                "{} ({}) — {}",
                text(item, name),
                text(item, date).chars().take(4).collect::<String>(),
                i18n::tr(language, &format!("audio_description.tmdb.{kind}"))
            )
        })
        .collect();
    let selected = choose("production", options)?;
    let (kind, production) = productions
        .get(selected)
        .ok_or("TMDB selection cancelled")?;
    let id = number(production, "id");
    let mut parameters = vec![(
        if *kind == "movie" {
            "movie_id"
        } else {
            "series_id"
        },
        id.to_string(),
    )];
    let details = client.character_lookup(&format!("{kind}_details"), language, &parameters)?;
    let animation = entries(&details, "genres")
        .iter()
        .any(|genre| number(genre, "id") == 16);
    let production_title = text(&details, if *kind == "movie" { "title" } else { "name" });
    if animation {
        let original = text(
            &details,
            if *kind == "movie" {
                "original_title"
            } else {
                "original_name"
            },
        );
        let date = text(
            &details,
            if *kind == "movie" {
                "release_date"
            } else {
                "first_air_date"
            },
        );
        return Ok(anilist::lookup(
            &[&production_title, &original],
            date.get(..4).unwrap_or(year),
            kind,
            &mut choose,
        )
        .unwrap_or_else(|e| {
            crate::log_debug(&format!("Optional AniList lookup: {e}"));
            Vec::new()
        }));
    }
    let mut season_number = None;
    let mut episode_number = None;
    let mut action = format!("{kind}_credits");
    if *kind == "tv" {
        let seasons = entries(&details, "seasons");
        if !seasons.is_empty() {
            let options = seasons
                .iter()
                .map(|season| {
                    (
                        number(season, "season_number"),
                        format!("{} ({})", text(season, "name"), text(season, "air_date")),
                    )
                })
                .collect();
            let (season, episode) = choose_tv(id, options)?;
            season_number = Some(season);
            episode_number = episode;
            parameters.push(("season_number", season.to_string()));
            action = "season_credits".into();
            if let Some(episode) = episode {
                parameters.push(("episode_number", episode.to_string()));
                action = "episode_credits".into();
            }
        }
    }
    let credits = client.character_lookup(&action, language, &parameters)?;
    let mut cast = entries(&credits, "cast");
    cast.extend(entries(&credits, "guest_stars"));
    let mut characters = Vec::new();
    for actor in cast {
        let mut roles = entries(&actor, "roles");
        if roles.is_empty() {
            roles.push(actor.clone());
        }
        for role in roles {
            let name = text(&role, "character");
            if name.is_empty() || number(&actor, "id") <= 0 {
                continue;
            }
            let credit_id = text(&role, "credit_id");
            let person_id = number(&actor, "id");
            let profile = text(&actor, "profile_path");
            let reference = TmdbCharacterReference {
                actor_name: text(&actor, "name"),
                person_id,
                credit_id: credit_id.clone(),
                production_id: id,
                production_title: production_title.clone(),
                production_year: text(
                    &details,
                    if *kind == "movie" {
                        "release_date"
                    } else {
                        "first_air_date"
                    },
                )
                .get(..4)
                .and_then(|year| year.parse().ok()),
                media_type: kind.to_string(),
                season_number,
                episode_number,
                animation,
                profile_image_url: (!animation
                    && profile.starts_with('/')
                    && !profile.contains(".."))
                .then(|| format!("https://image.tmdb.org/t/p/w185{profile}")),
            };
            let identifier = format!("tmdb_{id}_{person_id}_{credit_id}");
            if characters
                .iter()
                .any(|item: &BridgeCharacter| item.id == identifier)
            {
                continue;
            }
            characters.push(BridgeCharacter {
                id: identifier,
                name,
                description: format!(
                    "TMDB cast candidate: {}. Identity in the video is NOT verified.",
                    reference.actor_name
                ),
                anilist: None,
                tmdb: Some(reference),
                provisional: true,
            });
        }
    }
    characters.truncate(96);
    if characters.is_empty() {
        return Err(i18n::tr(language, "audio_description.tmdb.no_cast"));
    }
    Ok(characters)
}

/// The first entry retains the existing whole-season/unknown-episode option.
pub(super) fn episode_options(
    series: i64,
    season: i64,
    language: Language,
) -> Result<Vec<(i64, String)>, String> {
    let details = CinemaClient::default().character_lookup(
        "season_details",
        language,
        &[
            ("series_id", series.to_string()),
            ("season_number", season.to_string()),
        ],
    )?;
    let mut options = vec![(0, i18n::tr(language, "audio_description.tmdb.whole_season"))];
    options.extend(entries(&details, "episodes").iter().map(|episode| {
        let number = number(episode, "episode_number");
        (number, format!("{} — {}", number, text(episode, "name")))
    }));
    Ok(options)
}

#[cfg(test)]
mod live_tests {
    #[test]
    #[ignore = "live TMDB and AniList integration"]
    fn selected_productions_follow_expected_provider() {
        for (name, year, anime) in [
            ("Titanic", "1997", None),
            ("Dragon Ball Z", "1989", Some(813)),
            ("Dragon Ball GT", "1996", Some(225)),
            ("Death Note", "2006", Some(1535)),
            ("Spirited Away", "2001", Some(199)),
            ("The Simpsons", "1989", Some(0)),
        ] {
            let cast = super::lookup(
                name,
                year,
                crate::settings::Language::English,
                |_, options| {
                    let kind = if matches!(name, "Spirited Away" | "Titanic") {
                        "movie"
                    } else {
                        "tv"
                    };
                    let label = crate::i18n::tr(
                        crate::settings::Language::English,
                        &format!("audio_description.tmdb.{kind}"),
                    );
                    options
                        .iter()
                        .position(|s| s == &format!("{name} ({year}) — {label}"))
                        .ok_or_else(|| format!("missing {name}"))
                },
                |_, _| Err("Unexpected TV prompt".into()),
            )
            .unwrap();
            match anime {
                None => {
                    assert!(cast.iter().any(|c| {
                        c.tmdb.as_ref().is_some_and(|r| {
                            r.actor_name == "Leonardo DiCaprio" && r.profile_image_url.is_some()
                        })
                    }))
                }
                Some(0) => assert!(cast.is_empty()),
                Some(id) => assert!(
                    !cast.is_empty()
                        && cast.iter().all(|c| c
                            .anilist
                            .as_ref()
                            .is_some_and(|r| r.media_id == id && !r.image_urls.is_empty())),
                    "{name}"
                ),
            }
        }
    }
}
