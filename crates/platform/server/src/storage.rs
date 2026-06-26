//! Cloud file storage: save, load, list, and delete project files.
//!
//! Uses filesystem storage under a configurable root directory.
//! Each user gets their own directory: `{root}/{user_id}/`.
//! File metadata is stored alongside the content as `.meta.json`.

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// Metadata for a stored file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMeta {
    pub id: String,
    pub name: String,
    pub user_id: String,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// File storage backend.
#[derive(Debug, Clone)]
pub struct FileStore {
    root: PathBuf,
}

impl FileStore {
    /// Create a new file store rooted at the given directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        fs::create_dir_all(&root).ok();
        Self { root }
    }

    /// Save a file for a user. Returns the file metadata.
    pub fn save(&self, user_id: &str, name: &str, data: &[u8]) -> Result<FileMeta, String> {
        let user_dir = self.user_dir(user_id);
        fs::create_dir_all(&user_dir).map_err(|e| format!("Failed to create dir: {e}"))?;

        let file_id = Uuid::new_v4().to_string();
        let file_path = user_dir.join(&file_id);
        let meta_path = user_dir.join(format!("{file_id}.meta.json"));

        fs::write(&file_path, data).map_err(|e| format!("Write failed: {e}"))?;

        let meta = FileMeta {
            id: file_id,
            name: name.to_string(),
            user_id: user_id.to_string(),
            size_bytes: data.len() as u64,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let meta_json = serde_json::to_string_pretty(&meta)
            .map_err(|e| format!("Serialize error: {e}"))?;
        fs::write(&meta_path, meta_json).map_err(|e| format!("Meta write failed: {e}"))?;

        Ok(meta)
    }

    /// Update an existing file's content.
    pub fn update(&self, user_id: &str, file_id: &str, data: &[u8]) -> Result<FileMeta, String> {
        let user_dir = self.user_dir(user_id);
        let file_path = user_dir.join(file_id);
        let meta_path = user_dir.join(format!("{file_id}.meta.json"));

        if !file_path.exists() {
            return Err("File not found".into());
        }

        // Read existing meta
        let meta_str = fs::read_to_string(&meta_path)
            .map_err(|e| format!("Read meta failed: {e}"))?;
        let mut meta: FileMeta = serde_json::from_str(&meta_str)
            .map_err(|e| format!("Parse meta failed: {e}"))?;

        // Verify ownership
        if meta.user_id != user_id {
            return Err("Access denied".into());
        }

        fs::write(&file_path, data).map_err(|e| format!("Write failed: {e}"))?;
        meta.size_bytes = data.len() as u64;
        meta.updated_at = Utc::now();

        let meta_json = serde_json::to_string_pretty(&meta)
            .map_err(|e| format!("Serialize error: {e}"))?;
        fs::write(&meta_path, meta_json).map_err(|e| format!("Meta write failed: {e}"))?;

        Ok(meta)
    }

    /// Load a file's contents.
    pub fn load(&self, user_id: &str, file_id: &str) -> Result<Vec<u8>, String> {
        let file_path = self.user_dir(user_id).join(file_id);
        // Prevent path traversal
        if file_id.contains("..") || file_id.contains('/') {
            return Err("Invalid file ID".into());
        }
        fs::read(&file_path).map_err(|e| format!("Read failed: {e}"))
    }

    /// List all files for a user.
    pub fn list(&self, user_id: &str) -> Result<Vec<FileMeta>, String> {
        let user_dir = self.user_dir(user_id);
        if !user_dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        let entries = fs::read_dir(&user_dir).map_err(|e| format!("Read dir failed: {e}"))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("Dir entry error: {e}"))?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json")
                && path.file_name().map_or(false, |n| n.to_str().map_or(false, |s| s.ends_with(".meta.json")))
            {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(meta) = serde_json::from_str::<FileMeta>(&content) {
                        files.push(meta);
                    }
                }
            }
        }

        files.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(files)
    }

    /// Delete a file.
    pub fn delete(&self, user_id: &str, file_id: &str) -> Result<(), String> {
        if file_id.contains("..") || file_id.contains('/') {
            return Err("Invalid file ID".into());
        }
        let user_dir = self.user_dir(user_id);
        let file_path = user_dir.join(file_id);
        let meta_path = user_dir.join(format!("{file_id}.meta.json"));

        if !file_path.exists() {
            return Err("File not found".into());
        }

        fs::remove_file(&file_path).map_err(|e| format!("Delete failed: {e}"))?;
        fs::remove_file(&meta_path).ok(); // meta might not exist
        Ok(())
    }

    fn user_dir(&self, user_id: &str) -> PathBuf {
        self.root.join(user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::Path;

    fn temp_store() -> (FileStore, PathBuf) {
        let dir = env::temp_dir().join(format!("openie-test-{}", Uuid::new_v4()));
        let store = FileStore::new(&dir);
        (store, dir)
    }

    fn cleanup(dir: &Path) {
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn save_and_load() {
        let (store, dir) = temp_store();
        let data = b"hello world";
        let meta = store.save("user1", "test.oie", data).unwrap();
        assert_eq!(meta.name, "test.oie");
        assert_eq!(meta.size_bytes, 11);

        let loaded = store.load("user1", &meta.id).unwrap();
        assert_eq!(loaded, data);
        cleanup(&dir);
    }

    #[test]
    fn list_files() {
        let (store, dir) = temp_store();
        store.save("user1", "file1.oie", b"aaa").unwrap();
        store.save("user1", "file2.oie", b"bbb").unwrap();
        store.save("user2", "other.oie", b"ccc").unwrap();

        let user1_files = store.list("user1").unwrap();
        assert_eq!(user1_files.len(), 2);

        let user2_files = store.list("user2").unwrap();
        assert_eq!(user2_files.len(), 1);
        cleanup(&dir);
    }

    #[test]
    fn delete_file() {
        let (store, dir) = temp_store();
        let meta = store.save("user1", "delete_me.oie", b"data").unwrap();
        assert!(store.load("user1", &meta.id).is_ok());

        store.delete("user1", &meta.id).unwrap();
        assert!(store.load("user1", &meta.id).is_err());
        cleanup(&dir);
    }

    #[test]
    fn update_file() {
        let (store, dir) = temp_store();
        let meta = store.save("user1", "update.oie", b"v1").unwrap();
        let meta2 = store.update("user1", &meta.id, b"v2-longer").unwrap();
        assert_eq!(meta2.size_bytes, 9);

        let loaded = store.load("user1", &meta.id).unwrap();
        assert_eq!(loaded, b"v2-longer");
        cleanup(&dir);
    }

    #[test]
    fn path_traversal_blocked() {
        let (store, dir) = temp_store();
        assert!(store.load("user1", "../../../etc/passwd").is_err());
        assert!(store.delete("user1", "../../secret").is_err());
        cleanup(&dir);
    }

    #[test]
    fn empty_user_list() {
        let (store, dir) = temp_store();
        let files = store.list("nobody").unwrap();
        assert!(files.is_empty());
        cleanup(&dir);
    }
}
