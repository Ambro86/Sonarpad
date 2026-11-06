use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(super) struct RecentRoute {
    pub from: String,
    pub to: String,
    pub profile: String,
    pub preference: String,
    pub avoid: String,
    pub country: String,
    pub municipalities: bool,
}

pub(super) fn remember(item: RecentRoute) {
    let mut items = load();
    update(&mut items, item);
    save(&items);
}

fn update(items: &mut Vec<RecentRoute>, item: RecentRoute) {
    items.retain(|old| old != &item);
    items.insert(0, item);
    items.truncate(50);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recent_routes_preserve_options_deduplicate_and_limit() {
        let mut items = Vec::new();
        for i in 0..60 {
            update(
                &mut items,
                RecentRoute {
                    from: i.to_string(),
                    to: "Destination".into(),
                    profile: "transit".into(),
                    preference: "fastest".into(),
                    avoid: "".into(),
                    country: "it".into(),
                    municipalities: true,
                },
            );
        }
        assert_eq!(items.len(), 50);
        let item = items[20].clone();
        update(&mut items, item.clone());
        assert_eq!(items.len(), 50);
        assert_eq!(items[0], item);
        let json = serde_json::to_string(&items).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<RecentRoute>>(&json).unwrap(),
            items
        );
        assert!(
            serde_json::from_str::<Vec<RecentRoute>>("[]")
                .unwrap()
                .is_empty()
        );
    }
}

pub(super) fn load() -> Vec<RecentRoute> {
    std::fs::read_to_string(crate::settings::settings_dir().join("recent_routes.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}
pub(super) fn save(items: &[RecentRoute]) {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let dir = crate::settings::settings_dir();
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("recent_routes.json"),
            serde_json::to_string(items)?,
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        crate::log_debug(&format!("route_history: {error}"));
    }
}
