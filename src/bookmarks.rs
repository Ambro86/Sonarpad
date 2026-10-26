use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub position: i32,
    pub snippet: String,
    pub timestamp: String,
    /// Actual last opening; absent on legacy entries where the date is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_opened: Option<String>,
    #[serde(default)]
    pub automatic: bool,
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
        }
    }

    pub fn automatic(position: i32, snippet: String, timestamp: String) -> Self {
        Self {
            position,
            snippet,
            last_opened: Some(timestamp.clone()),
            timestamp,
            automatic: true,
        }
    }

    pub fn is_visible(&self, automatic_bookmark_enabled: bool) -> bool {
        automatic_bookmark_enabled || !self.automatic
    }
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

pub fn load_bookmarks() -> BookmarkStore {
    let Some(path) = bookmark_store_path() else {
        return BookmarkStore::default();
    };
    let data = std::fs::read_to_string(path).ok();
    let Some(data) = data else {
        return BookmarkStore::default();
    };
    let mut store = serde_json::from_str(&data).unwrap_or_default();
    normalize_store(&mut store);
    store
}

/// Save atomically, so an interruption cannot leave a truncated bookmarks.json.
pub fn try_save_bookmarks(store: &BookmarkStore) -> std::io::Result<()> {
    let Some(path) = bookmark_store_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
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

/// Preserve the original file before any user-initiated batch cleanup.
pub fn backup_before_cleanup() -> std::io::Result<()> {
    let Some(path) = bookmark_store_path() else {
        return Ok(());
    };
    if !path.is_file() {
        return Ok(());
    }
    let backup_dir = path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("bookmarks-backups");
    std::fs::create_dir_all(&backup_dir)?;
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::fs::copy(&path, backup_dir.join(format!("bookmarks-{suffix}.json")))?;
    Ok(())
}

pub fn save_bookmarks(store: &BookmarkStore) {
    crate::log_if_err!(try_save_bookmarks(store));
}

#[cfg(test)]
mod tests {
    use super::{Bookmark, BookmarkStore, mark_file_opened, remove_selected_bookmarks};

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
