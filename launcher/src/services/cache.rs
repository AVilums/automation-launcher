use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

use crate::error::LauncherError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub tool_name: String,
    pub version: String,
    pub size_bytes: u64,
    pub last_used: chrono::DateTime<chrono::Utc>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheIndex {
    pub entries: Vec<CacheEntry>,
}

pub struct CacheManager {
    cache_dir: PathBuf,
    max_size_bytes: u64,
    index_path: PathBuf,
}

impl CacheManager {
    pub fn new(cache_dir: PathBuf, max_size_mb: u64) -> Self {
        let index_path = cache_dir.join("cache_index.json");
        Self {
            cache_dir,
            max_size_bytes: max_size_mb * 1024 * 1024,
            index_path,
        }
    }

    pub fn init(&self) -> Result<(), LauncherError> {
        std::fs::create_dir_all(&self.cache_dir)?;
        if !self.index_path.exists() {
            self.save_index(&CacheIndex::default())?;
        }
        Ok(())
    }

    pub fn get_cached_path(&self, tool_name: &str, version: &str) -> Option<PathBuf> {
        let index = self.load_index().ok()?;
        index
            .entries
            .iter()
            .find(|e| e.tool_name == tool_name && e.version == version)
            .map(|e| e.path.clone())
            .filter(|p| p.exists())
    }

    pub fn store(
        &self,
        tool_name: &str,
        version: &str,
        source_path: &Path,
    ) -> Result<PathBuf, LauncherError> {
        let tool_dir = self.cache_dir.join(tool_name).join(version);
        std::fs::create_dir_all(&tool_dir)?;

    let dest_path = if source_path.is_dir() {
        // When storing a directory, the canonical path is the versioned directory itself
        tool_dir.clone()
    } else {
        let filename = source_path
            .file_name()
            .ok_or_else(|| LauncherError::Cache("Invalid source filename".into()))?;
        tool_dir.join(filename)
    };

    if source_path != dest_path {
        if source_path.is_dir() {
            // If it's a directory, copy its contents to tool_dir
            for entry in std::fs::read_dir(source_path)? {
                let entry = entry?;
                let dest = tool_dir.join(entry.file_name());
                if entry.path().is_dir() {
                    copy_dir_all(&entry.path(), &dest)?;
                } else {
                    std::fs::copy(entry.path(), &dest)?;
                }
            }
        } else {
            std::fs::copy(source_path, &dest_path)?;
        }
    }

        let size_bytes = if tool_dir.exists() {
            get_dir_size(&tool_dir).unwrap_or(0)
        } else {
            0
        };

        let mut index = self.load_index().unwrap_or_default();

        index
            .entries
            .retain(|e| !(e.tool_name == tool_name && e.version == version));

        index.entries.push(CacheEntry {
            tool_name: tool_name.to_string(),
            version: version.to_string(),
            size_bytes,
            last_used: chrono::Utc::now(),
            path: dest_path.clone(),
        });

        self.enforce_size_limit(&mut index)?;
        self.save_index(&index)?;

        info!("Cached {} v{} ({} bytes)", tool_name, version, size_bytes);
        Ok(dest_path)
    }

    pub fn touch(&self, tool_name: &str, version: &str) -> Result<(), LauncherError> {
        let mut index = self.load_index()?;
        if let Some(entry) = index
            .entries
            .iter_mut()
            .find(|e| e.tool_name == tool_name && e.version == version)
        {
            entry.last_used = chrono::Utc::now();
            self.save_index(&index)?;
        }
        Ok(())
    }

    pub fn total_size(&self) -> Result<u64, LauncherError> {
        let index = self.load_index()?;
        Ok(index.entries.iter().map(|e| e.size_bytes).sum())
    }

    pub fn list_entries(&self) -> Result<Vec<CacheEntry>, LauncherError> {
        let index = self.load_index()?;
        Ok(index.entries)
    }

    pub fn remove(&self, tool_name: &str, version: &str) -> Result<(), LauncherError> {
        let mut index = self.load_index()?;
        let tool_dir = self.cache_dir.join(tool_name).join(version);

        if tool_dir.exists() {
            std::fs::remove_dir_all(&tool_dir)?;
        }

        index
            .entries
            .retain(|e| !(e.tool_name == tool_name && e.version == version));
        self.save_index(&index)?;

        info!("Removed {} v{} from cache", tool_name, version);
        Ok(())
    }

    fn enforce_size_limit(&self, index: &mut CacheIndex) -> Result<(), LauncherError> {
        let total: u64 = index.entries.iter().map(|e| e.size_bytes).sum();
        if total <= self.max_size_bytes {
            return Ok(());
        }

        index
            .entries
            .sort_by(|a, b| a.last_used.cmp(&b.last_used));

        let mut current_size = total;
        while current_size > self.max_size_bytes && !index.entries.is_empty() {
            let evicted = index.entries.remove(0);
            info!(
                "Evicting {} v{} from cache (LRU)",
                evicted.tool_name, evicted.version
            );

            if evicted.path.exists() {
                if let Some(parent) = evicted.path.parent() {
                    let _ = std::fs::remove_dir_all(parent);
                }
            }

            current_size = current_size.saturating_sub(evicted.size_bytes);
        }

        Ok(())
    }

    fn load_index(&self) -> Result<CacheIndex, LauncherError> {
        if !self.index_path.exists() {
            return Ok(CacheIndex::default());
        }
        let content = std::fs::read_to_string(&self.index_path)?;
        let index: CacheIndex = serde_json::from_str(&content)?;
        Ok(index)
    }

    fn save_index(&self, index: &CacheIndex) -> Result<(), LauncherError> {
        let content = serde_json::to_string_pretty(index)?;
        std::fs::write(&self.index_path, content)?;
        Ok(())
    }
}

fn get_dir_size(path: &Path) -> std::io::Result<u64> {
    let mut size = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                size += get_dir_size(&path)?;
            } else {
                size += entry.metadata()?.len();
            }
        }
    } else {
        size = path.metadata()?.len();
    }
    Ok(size)
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), &dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_cache() -> (TempDir, CacheManager) {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let manager = CacheManager::new(cache_dir, 10);
        manager.init().unwrap();
        (tmp, manager)
    }

    #[test]
    fn test_cache_init() {
        let (tmp, _manager) = setup_cache();
        assert!(tmp.path().join("cache").exists());
        assert!(tmp.path().join("cache").join("cache_index.json").exists());
    }

    #[test]
    fn test_store_and_retrieve() {
        let (tmp, manager) = setup_cache();

        let source = tmp.path().join("tool.exe");
        std::fs::write(&source, b"fake binary content").unwrap();

        let cached = manager.store("my-tool", "1.0.0", &source).unwrap();
        assert!(cached.exists());

        let found = manager.get_cached_path("my-tool", "1.0.0");
        assert!(found.is_some());
        assert_eq!(found.unwrap(), cached);

        assert!(manager.get_cached_path("my-tool", "2.0.0").is_none());
    }

    #[test]
    fn test_total_size() {
        let (tmp, manager) = setup_cache();

        let source = tmp.path().join("tool.exe");
        std::fs::write(&source, b"12345678").unwrap();

        manager.store("tool-a", "1.0.0", &source).unwrap();
        manager.store("tool-b", "1.0.0", &source).unwrap();

        let total = manager.total_size().unwrap();
        assert_eq!(total, 16);
    }

    #[test]
    fn test_remove_entry() {
        let (tmp, manager) = setup_cache();

        let source = tmp.path().join("tool.exe");
        std::fs::write(&source, b"content").unwrap();

        manager.store("my-tool", "1.0.0", &source).unwrap();
        assert!(manager.get_cached_path("my-tool", "1.0.0").is_some());

        manager.remove("my-tool", "1.0.0").unwrap();
        assert!(manager.get_cached_path("my-tool", "1.0.0").is_none());
    }

    #[test]
    fn test_eviction_lru() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        // 1 byte max to force eviction
        let manager = CacheManager::new(cache_dir, 0);
        manager.init().unwrap();

        let source = tmp.path().join("tool.exe");
        std::fs::write(&source, b"data").unwrap();

        manager.store("old-tool", "1.0.0", &source).unwrap();
        // Storing another should evict old-tool since max is 0 MB
        manager.store("new-tool", "1.0.0", &source).unwrap();

        let entries = manager.list_entries().unwrap();
        // With 0 MB limit, even the new entry exceeds it so all get evicted
        assert!(entries.is_empty() || entries.len() == 1);
    }

    #[test]
    fn test_list_entries() {
        let (tmp, manager) = setup_cache();

        let source = tmp.path().join("tool.exe");
        std::fs::write(&source, b"content").unwrap();

        manager.store("tool-a", "1.0.0", &source).unwrap();
        manager.store("tool-b", "2.0.0", &source).unwrap();

        let entries = manager.list_entries().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_store_overwrites_same_version() {
        let (tmp, manager) = setup_cache();

        let source = tmp.path().join("tool.exe");
        std::fs::write(&source, b"v1").unwrap();
        manager.store("my-tool", "1.0.0", &source).unwrap();

        std::fs::write(&source, b"v1-updated").unwrap();
        manager.store("my-tool", "1.0.0", &source).unwrap();

        let entries = manager.list_entries().unwrap();
        let matching: Vec<_> = entries
            .iter()
            .filter(|e| e.tool_name == "my-tool" && e.version == "1.0.0")
            .collect();
        assert_eq!(matching.len(), 1);
    }
}
