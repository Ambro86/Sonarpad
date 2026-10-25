/// Distinguish identical combo values for screen readers that suppress an
/// unchanged value. This changes presentation only, never item IDs or order.
pub fn accessible_result_labels(labels: Vec<String>) -> Vec<String> {
    let mut counts = std::collections::HashMap::new();
    for label in &labels {
        *counts.entry(label.trim().to_lowercase()).or_insert(0usize) += 1;
    }
    let mut occurrences = std::collections::HashMap::new();
    let mut used: std::collections::HashSet<_> = labels
        .iter()
        .map(|label| label.trim().to_lowercase())
        .collect();
    labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            let key = label.trim().to_lowercase();
            let count = counts[&key];
            if count <= 1 {
                return label;
            }
            let occurrence = occurrences.entry(key).or_insert(0usize);
            *occurrence += 1;
            let mut display = format!("{label} ({occurrence}/{count})");
            while !used.insert(display.trim().to_lowercase()) {
                display = format!("{display} ({})", index + 1);
            }
            display
        })
        .collect()
}

#[cfg(test)]
mod accessible_result_label_tests {
    use super::accessible_result_labels;
    #[test]
    fn identical_tracks_are_distinct_without_reordering() {
        let title = "Fantasia — Nino D'Angelo — Brani — 3:59";
        let result = accessible_result_labels(vec![title.into(), "Other".into(), title.into()]);
        assert_eq!(
            result,
            vec![
                format!("{title} (1/2)"),
                "Other".into(),
                format!("{title} (2/2)")
            ]
        );
    }
    #[test]
    fn unique_labels_and_pagination_are_unchanged() {
        let labels = vec![
            "Previous".into(),
            "Fantasia — 3:59".into(),
            "Fantasia — 4:10".into(),
            "Next".into(),
        ];
        assert_eq!(accessible_result_labels(labels.clone()), labels);
        assert!(accessible_result_labels(Vec::new()).is_empty());
    }
    #[test]
    fn equivalent_case_and_spacing_are_distinguished() {
        assert_eq!(
            accessible_result_labels(vec!["Song".into(), " song ".into()]),
            vec!["Song (1/2)", " song  (2/2)"]
        );
    }
    #[test]
    fn existing_numbered_titles_cannot_collide() {
        let result =
            accessible_result_labels(vec!["Song".into(), "Song".into(), "Song (1/2)".into()]);
        assert_eq!(
            result
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3
        );
        assert_eq!(result[2], "Song (1/2)");
    }
    #[test]
    fn duplicate_display_selection_keeps_the_second_content() {
        let values = [("Titolo", "first-url"), ("Titolo", "second-url")];
        let labels = accessible_result_labels(values.iter().map(|v| v.0.into()).collect());
        let entries: Vec<_> = labels.iter().zip(values.iter()).collect();
        let selected = entries
            .iter()
            .find(|(label, _)| **label == labels[1])
            .unwrap();
        assert_eq!(selected.1.1, "second-url");
    }
    #[test]
    fn works_with_non_latin_titles_without_translation_keys() {
        assert_eq!(
            accessible_result_labels(vec!["音乐".into(), "音乐".into()]),
            vec!["音乐 (1/2)", "音乐 (2/2)"]
        );
    }
}

pub fn spoken_time(seconds: f64, language: crate::settings::Language) -> String {
    use crate::settings::Language;
    // Same whole-second announcements and translated units as Sonarpad Mobile.
    let total = if seconds.is_finite() && seconds > 0.0 {
        (seconds as u64).max(1)
    } else {
        0
    };
    let mut units = Vec::new();
    for (unit, value) in [
        ("hour", total / 3600),
        ("minute", total % 3600 / 60),
        ("second", total % 60),
    ] {
        if value == 0 && (unit != "second" || !units.is_empty()) {
            continue;
        }
        let form = match language {
            Language::Polish => {
                if value == 1 {
                    "one"
                } else if (2..=4).contains(&(value % 10)) && !(12..=14).contains(&(value % 100)) {
                    "few"
                } else {
                    "many"
                }
            }
            Language::Czech => {
                if value == 1 {
                    "one"
                } else if (2..=4).contains(&value) {
                    "few"
                } else {
                    "many"
                }
            }
            Language::Russian | Language::Ukrainian | Language::Serbian => {
                if value % 10 == 1 && value % 100 != 11 {
                    "one"
                } else if (2..=4).contains(&(value % 10)) && !(12..=14).contains(&(value % 100)) {
                    "few"
                } else {
                    "many"
                }
            }
            Language::Lithuanian => {
                if value % 10 == 1 && value % 100 != 11 {
                    "one"
                } else if (2..=9).contains(&(value % 10)) && !(11..=19).contains(&(value % 100)) {
                    "few"
                } else {
                    "many"
                }
            }
            _ => {
                if value == 1 {
                    "one"
                } else {
                    "many"
                }
            }
        };
        units.push(format!(
            "{value} {}",
            crate::i18n::tr(language, &format!("media_cutter.duration_{unit}_{form}"))
        ));
    }
    if units.len() == 1 {
        return units.remove(0);
    }
    let last = units.pop().unwrap_or_default();
    format!(
        "{} {} {last}",
        units.join(", "),
        crate::i18n::tr(language, "media_cutter.duration_and")
    )
}

/// Like Mobile, keep live/unknown metadata intact and expand only clock durations.
pub fn media_duration(clock: &str, language: crate::settings::Language) -> String {
    let value = clock.trim();
    let parts: Vec<_> = value.split(':').collect();
    if !(2..=3).contains(&parts.len())
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|c| c.is_ascii_digit()))
    {
        return value.into();
    }
    let mut seconds = 0u64;
    for (i, part) in parts.iter().enumerate() {
        let Ok(n) = part.parse::<u64>() else {
            return value.into();
        };
        if i > 0 && (part.len() != 2 || n >= 60) {
            return value.into();
        }
        let Some(total) = seconds.checked_mul(60).and_then(|s| s.checked_add(n)) else {
            return value.into();
        };
        seconds = total;
    }
    spoken_time(seconds as f64, language)
}

#[cfg(test)]
mod media_result_tests {
    use super::*;
    use crate::settings::Language;
    #[test]
    fn clocks_use_existing_localized_spoken_units() {
        assert_eq!(
            media_duration("1:02:03", Language::Italian),
            "1 ora, 2 minuti e 3 secondi"
        );
        assert_eq!(
            media_duration("62:03", Language::Italian),
            "1 ora, 2 minuti e 3 secondi"
        );
        assert_eq!(
            media_duration("1:01", Language::English),
            "1 minute and 1 second"
        );
        assert_eq!(media_duration("0:00", Language::Italian), "0 secondi");
        for text in [
            "LIVE",
            "1:99",
            "1:60:00",
            "5:3",
            "",
            "999999999999999999999:00",
        ] {
            assert_eq!(media_duration(text, Language::Italian), text);
        }
    }
}
