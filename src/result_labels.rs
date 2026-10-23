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
