use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::LauncherError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Favorites {
    pub items: Vec<FavoriteItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FavoriteItem {
    pub tool_name: String,
    pub pinned_version: Option<String>,
}

impl Favorites {
    pub fn load(path: &Path) -> Result<Self, LauncherError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let favs: Favorites = serde_json::from_str(&content)?;
        Ok(favs)
    }

    pub fn save(&self, path: &Path) -> Result<(), LauncherError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn add(&mut self, tool_name: &str, pinned_version: Option<String>) {
        if !self.contains(tool_name) {
            self.items.push(FavoriteItem {
                tool_name: tool_name.to_string(),
                pinned_version,
            });
        }
    }

    pub fn remove(&mut self, tool_name: &str) {
        self.items.retain(|f| f.tool_name != tool_name);
    }

    pub fn contains(&self, tool_name: &str) -> bool {
        self.items.iter().any(|f| f.tool_name == tool_name)
    }

    pub fn file_path(base_dir: &Path) -> PathBuf {
        base_dir.join("config").join("favorites.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_add_and_contains() {
        let mut favs = Favorites::default();
        assert!(!favs.contains("tool-a"));

        favs.add("tool-a", None);
        assert!(favs.contains("tool-a"));
        assert_eq!(favs.items.len(), 1);

        // Adding again should not duplicate
        favs.add("tool-a", None);
        assert_eq!(favs.items.len(), 1);
    }

    #[test]
    fn test_remove() {
        let mut favs = Favorites::default();
        favs.add("tool-a", None);
        favs.add("tool-b", Some("1.0.0".into()));

        favs.remove("tool-a");
        assert!(!favs.contains("tool-a"));
        assert!(favs.contains("tool-b"));
    }

    #[test]
    fn test_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("favs.json");

        let mut favs = Favorites::default();
        favs.add("my-tool", Some("2.0.0".into()));
        favs.save(&path).unwrap();

        let loaded = Favorites::load(&path).unwrap();
        assert_eq!(loaded.items.len(), 1);
        assert_eq!(loaded.items[0].tool_name, "my-tool");
        assert_eq!(loaded.items[0].pinned_version, Some("2.0.0".into()));
    }

    #[test]
    fn test_load_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("missing.json");
        let favs = Favorites::load(&path).unwrap();
        assert!(favs.items.is_empty());
    }
}
