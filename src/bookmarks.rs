use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub position: i32,
    #[serde(default)]
    pub snippet: String,
    #[serde(default)]
    pub timestamp: String,
    /// Actual last opening; absent on legacy entries where the date is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_opened: Option<String>,
    #[serde(default)]
    pub automatic: bool,
    /// Original YouTube page, never an expiring media URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// Restore SonarMusic's video resolver and playback fallbacks.
    #[serde(default, skip_serializing_if = "is_false")]
    pub from_sonarmusic: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Default, Serialize, Deserialize)]
pub struct BookmarkStore {
    pub files: HashMap<String, Vec<Bookmark>>,
}

impl Bookmark {
    pub fn manual(position: i32, snippet: String, timestamp: String) -> Self {
        Self {
            position,
            snippet,
            last_opened: Some(timestamp.clone()),
            timestamp,
            automatic: false,
            source_url: None,
            from_sonarmusic: false,
        }
    }

    pub fn automatic(position: i32, snippet: String, timestamp: String) -> Self {
        Self {
            position,
            snippet,
            last_opened: Some(timestamp.clone()),
            timestamp,
            automatic: true,
            source_url: None,
            from_sonarmusic: false,
        }
    }

    pub fn is_visible(&self, automatic_bookmark_enabled: bool) -> bool {
        automatic_bookmark_enabled || !self.automatic
    }
}

/// Resolve the original watch page, never an expiring media URL.
/// Legacy bookmarks can inherit a source from another bookmark of the same media.
pub fn bookmark_stream_url(store: &BookmarkStore, key: &str, index: usize) -> Option<String> {
    let entries = store.files.get(key)?;
    let selected = entries.get(index)?;
    selected
        .source_url
        .as_deref()
        .and_then(|url| crate::sonartube::direct_watch_url(url, false))
        .or_else(|| {
            entries.iter().find_map(|bookmark| {
                bookmark
                    .source_url
                    .as_deref()
                    .and_then(|url| crate::sonartube::direct_watch_url(url, false))
            })
        })
        .or_else(|| crate::sonartube::direct_watch_url(key, false))
}

/// Records a document opening independently of its bookmark update time.
pub fn mark_file_opened(store: &mut BookmarkStore, path: &str, date: &str) -> bool {
    let Some(entries) = store.files.get_mut(path) else {
        return false;
    };
    for entry in entries.iter_mut() {
        entry.last_opened = Some(date.to_string());
    }
    !entries.is_empty()
}

/// Remove selected original indices without shifting later selections.
pub fn remove_selected_bookmarks(store: &mut BookmarkStore, selected: &[(String, usize)]) -> usize {
    let mut grouped: HashMap<&str, Vec<usize>> = HashMap::new();
    for (path, index) in selected {
        grouped.entry(path).or_default().push(*index);
    }
    let mut removed = 0;
    for (path, mut indices) in grouped {
        let Some(entries) = store.files.get_mut(path) else {
            continue;
        };
        indices.sort_unstable();
        indices.dedup();
        for index in indices.into_iter().rev() {
            if index < entries.len() {
                entries.remove(index);
                removed += 1;
            }
        }
        if entries.is_empty() {
            store.files.remove(path);
        }
    }
    removed
}

pub fn sort_bookmarks(bookmarks: &mut [Bookmark]) {
    bookmarks.sort_by_key(|bookmark| bookmark.position);
}

pub fn normalize_store(store: &mut BookmarkStore) {
    for bookmarks in store.files.values_mut() {
        sort_bookmarks(bookmarks);
    }
}

fn bookmark_store_path() -> Option<PathBuf> {
    let mut path = crate::settings::settings_dir();
    path.push("bookmarks.json");
    Some(path)
}

/// Read the current wrapper and older bookmark maps without losing existing entries.
/// Saving always emits the canonical {"files": ...} shape, readable by older Sonarpad releases.
fn parse_bookmark_store(data: &str) -> serde_json::Result<BookmarkStore> {
    if let Ok(store) = serde_json::from_str::<BookmarkStore>(data) {
        return Ok(store);
    }
    let value: serde_json::Value = serde_json::from_str(data)?;
    if value.get("files").is_some() {
        // An invalid current-format file must be reported, never treated as empty.
        return serde_json::from_value(value);
    }
    if let Some(bookmarks) = value.get("bookmarks") {
        let files = serde_json::from_value(bookmarks.clone())?;
        return Ok(BookmarkStore { files });
    }
    let files = serde_json::from_value(value)?;
    Ok(BookmarkStore { files })
}

pub fn load_bookmarks() -> BookmarkStore {
    let Some(path) = bookmark_store_path() else {
        return BookmarkStore::default();
    };
    let data = match std::fs::read_to_string(&path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return BookmarkStore::default();
        }
        Err(error) => {
            crate::log_debug(&format!(
                "Cannot read bookmarks at {}: {error}",
                path.display()
            ));
            return BookmarkStore::default();
        }
    };
    match parse_bookmark_store(&data) {
        Ok(mut store) => {
            normalize_store(&mut store);
            store
        }
        Err(error) => {
            crate::log_debug(&format!(
                "Cannot parse bookmarks at {}: {error}; file preserved",
                path.display()
            ));
            BookmarkStore::default()
        }
    }
}

/// Save atomically, so an interruption cannot leave a truncated bookmarks.json.
pub fn try_save_bookmarks(store: &BookmarkStore) -> std::io::Result<()> {
    let Some(path) = bookmark_store_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // A failed load must never silently overwrite a legacy or damaged bookmark file.
    // Parse it first; this also accepts supported older on-disk formats.
    if path.is_file() {
        let existing = std::fs::read_to_string(&path)?;
        parse_bookmark_store(&existing).map_err(std::io::Error::other)?;
    }
    let mut normalized = BookmarkStore {
        files: store.files.clone(),
    };
    normalize_store(&mut normalized);
    let json = serde_json::to_vec_pretty(&normalized)?;
    let temporary = path.with_extension("json.tmp");
    let result = (|| {
        std::fs::write(&temporary, json)?;
        // std::fs::rename cannot reliably replace an existing file on Windows.
        // MoveFileExW atomically replaces the destination on the same volume.
        let source = crate::accessibility::to_wide(&temporary.to_string_lossy());
        let destination = crate::accessibility::to_wide(&path.to_string_lossy());
        unsafe {
            windows::Win32::Storage::FileSystem::MoveFileExW(
                windows::core::PCWSTR(source.as_ptr()),
                windows::core::PCWSTR(destination.as_ptr()),
                windows::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING
                    | windows::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
            )
            .map_err(std::io::Error::other)
        }
    })();
    if result.is_err() {
        crate::log_if_err!(std::fs::remove_file(&temporary));
    }
    result
}

pub fn save_bookmarks(store: &BookmarkStore) {
    crate::log_if_err!(try_save_bookmarks(store));
}

#[cfg(test)]
mod tests {
    use super::{Bookmark, BookmarkStore, mark_file_opened, remove_selected_bookmarks};

    #[test]
    fn media_bookmark_restores_canonical_url_and_legacy_entries() {
        let mut store = BookmarkStore::default();
        let mut first = Bookmark::manual(12, "first".into(), String::new());
        first.source_url = Some("https://youtu.be/4K6cvc27_CE".into());
        store.files.insert(
            "__stream_title__:Example".into(),
            vec![first, Bookmark::manual(82, "legacy".into(), String::new())],
        );
        assert_eq!(
            super::bookmark_stream_url(&store, "__stream_title__:Example", 1).as_deref(),
            Some("https://www.youtube.com/watch?v=4K6cvc27_CE")
        );
        store.files.insert(
            "https://youtu.be/4K6cvc27_CE".into(),
            vec![Bookmark::automatic(7, String::new(), String::new())],
        );
        assert_eq!(
            super::bookmark_stream_url(&store, "https://youtu.be/4K6cvc27_CE", 0).as_deref(),
            Some("https://www.youtube.com/watch?v=4K6cvc27_CE")
        );
        assert!(super::bookmark_stream_url(&store, "__stream_title__:Example", 99).is_none());
        let legacy: Bookmark = serde_json::from_str(
            r#"{"position":5,"snippet":"legacy","timestamp":"2026-04-29 10:00:00"}"#,
        )
        .unwrap();
        assert!(!legacy.from_sonarmusic);
    }

    #[test]
    fn legacy_bookmarks_default_to_manual() {
        let bookmark: Bookmark = serde_json::from_str(
            r#"{"position":42,"snippet":"sample","timestamp":"2026-04-29 10:00:00"}"#,
        )
        .expect("legacy bookmark should deserialize");

        assert!(!bookmark.automatic);
        assert!(bookmark.last_opened.is_none());
        assert!(bookmark.is_visible(false));
    }

    #[test]
    fn migrates_legacy_bookmark_stores_without_losing_entries() {
        let entry = r#"{"position":42,"snippet":"old bookmark"}"#;
        for old_json in [
            format!(r#"{{"C:\\legacy.txt":[{entry}]}}"#),
            format!(r#"{{"bookmarks":{{"C:\\legacy.txt":[{entry}]}}}}"#),
            format!(r#"{{"files":{{"C:\\legacy.txt":[{entry}]}}}}"#),
        ] {
            let store = super::parse_bookmark_store(&old_json).unwrap();
            assert_eq!(store.files.len(), 1);
            let bookmark = &store.files["C:\\legacy.txt"][0];
            assert_eq!(bookmark.position, 42);
            assert_eq!(bookmark.snippet, "old bookmark");
            assert!(bookmark.timestamp.is_empty());
            assert!(!bookmark.automatic);
            assert!(!bookmark.from_sonarmusic);
            assert!(bookmark.source_url.is_none());
            let persisted = serde_json::to_string(&store).unwrap();
            assert!(persisted.contains("\"files\""));
            let restored = super::parse_bookmark_store(&persisted).unwrap();
            assert_eq!(restored.files["C:\\legacy.txt"].len(), 1);
        }
    }

    #[test]
    fn malformed_current_bookmark_file_cannot_be_migrated_as_empty() {
        let invalid = r#"{"files":{"old.txt":[{"position":"invalid"}]}}"#;
        assert!(super::parse_bookmark_store(invalid).is_err());
    }

    #[test]
    fn last_opened_does_not_overwrite_bookmark_date() {
        let entry = Bookmark::automatic(18, "text".into(), "2026-10-01 10:00:00".into());
        let mut store = BookmarkStore::default();
        store.files.insert("story.txt".into(), vec![entry]);
        assert!(mark_file_opened(
            &mut store,
            "story.txt",
            "2026-10-26 11:00:00"
        ));
        let entry = &store.files["story.txt"][0];
        assert_eq!(entry.timestamp, "2026-10-01 10:00:00");
        assert_eq!(entry.last_opened.as_deref(), Some("2026-10-26 11:00:00"));
        let restored: BookmarkStore =
            serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
        assert_eq!(
            restored.files["story.txt"][0].last_opened,
            entry.last_opened
        );
    }

    #[test]
    fn selective_removal_preserves_other_indices() {
        let mut store = BookmarkStore::default();
        store.files.insert(
            "a.txt".into(),
            (0..4)
                .map(|i| Bookmark::manual(i, String::new(), String::new()))
                .collect(),
        );
        store.files.insert(
            "b.txt".into(),
            vec![Bookmark::automatic(7, String::new(), String::new())],
        );
        let deleted = remove_selected_bookmarks(
            &mut store,
            &[
                ("a.txt".into(), 1),
                ("a.txt".into(), 3),
                ("a.txt".into(), 1),
                ("b.txt".into(), 0),
            ],
        );
        assert_eq!(deleted, 3);
        assert_eq!(
            store.files["a.txt"]
                .iter()
                .map(|b| b.position)
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert!(!store.files.contains_key("b.txt"));
    }
}
