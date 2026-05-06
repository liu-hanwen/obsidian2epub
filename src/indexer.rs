//! Phase 1 – Vault Indexer
//!
//! Recursively scans the Obsidian Vault and builds two lookup tables:
//!   • `md_index`  : stem (lowercase) → path relative to vault root, for `.md` files.
//!   • `img_index` : full filename (lowercase, with extension) → relative path, for images.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

/// Image extensions that Obsidian can embed with `![[…]]`.
pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg"];

/// Result of scanning a vault directory.
pub struct VaultIndex {
    /// Markdown files: key = stem.to_lowercase(), value = path relative to vault root.
    pub md_index: HashMap<String, PathBuf>,
    /// Image files: key = filename.to_lowercase() (includes extension), value = path relative to vault root.
    pub img_index: HashMap<String, PathBuf>,
}

/// Walk `vault_root` and build a [`VaultIndex`].
pub fn build_index(vault_root: &Path) -> VaultIndex {
    let mut md_index: HashMap<String, PathBuf> = HashMap::new();
    let mut img_index: HashMap<String, PathBuf> = HashMap::new();

    for entry in WalkDir::new(vault_root)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let rel_path = path
            .strip_prefix(vault_root)
            .unwrap_or(path)
            .to_path_buf();

        if ext == "md" {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            md_index.entry(stem).or_insert(rel_path);
        } else if IMAGE_EXTS.contains(&ext.as_str()) {
            let filename = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("")
                .to_lowercase();
            img_index.entry(filename).or_insert(rel_path);
        }
    }

    VaultIndex { md_index, img_index }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_vault() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::write(root.join("index.md"), "hello").unwrap();
        fs::write(root.join("nested").join("note_a.md"), "world").unwrap();
        fs::write(root.join("assets").join("pic.png"), b"\x89PNG").unwrap();
        dir
    }

    #[test]
    fn finds_markdown_files() {
        let vault = make_vault();
        let idx = build_index(vault.path());
        assert!(idx.md_index.contains_key("index"), "index.md should be indexed");
        assert!(idx.md_index.contains_key("note_a"), "note_a.md should be indexed");
    }

    #[test]
    fn finds_image_files() {
        let vault = make_vault();
        let idx = build_index(vault.path());
        assert!(idx.img_index.contains_key("pic.png"), "pic.png should be indexed");
    }

    #[test]
    fn md_path_is_relative() {
        let vault = make_vault();
        let idx = build_index(vault.path());
        let p = &idx.md_index["note_a"];
        assert!(p.starts_with("nested"), "path should be relative to vault root");
    }
}
