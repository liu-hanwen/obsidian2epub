//! Phase 3 – Syntax Transformer
//!
//! Converts Obsidian-specific wikilink syntax into standard
//! Markdown / HTML that `pulldown-cmark` can render:
//!
//!   `[[file]]`            → `[file](file.xhtml)`
//!   `[[file|alias]]`      → `[alias](file.xhtml)`
//!   `[[file#section]]`    → `[file](file.xhtml)`  (anchor stripped)
//!   `![[image.png]]`      → `![image.png](image.png)`
//!
//! Dead-links (targets not found in the index) are degraded to plain text.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

/// Regex matching both `![[…]]` and `[[…]]` wikilinks.
fn wikilink_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"!\[\[([^\]\r\n]+)\]\]|\[\[([^\]\r\n]+)\]\]").unwrap())
}

/// Replace all Obsidian wikilinks in `markdown` with standard Markdown syntax.
///
/// * `source_rel_path` – the path of the *current* `.md` file relative to the
///   vault root (used to compute relative link targets).
/// * `md_index`  – stem (lowercase) → relative path of `.md` files.
/// * `img_index` – filename (lowercase, with ext) → relative path of images.
pub fn transform_wikilinks(
    markdown: &str,
    source_rel_path: &Path,
    md_index: &HashMap<String, PathBuf>,
    img_index: &HashMap<String, PathBuf>,
) -> String {
    let source_dir = source_rel_path.parent().unwrap_or(Path::new(""));

    let result = wikilink_re().replace_all(markdown, |caps: &regex::Captures| {
        if let Some(m) = caps.get(1) {
            // -------- Image embed:  ![[image.ext]] --------
            let img_name = m.as_str().trim();
            let key = img_name.to_lowercase();
            match img_index.get(&key) {
                Some(img_path) => {
                    let rel = relative_link(source_dir, img_path);
                    format!("![{img_name}]({rel})")
                }
                None => format!("![[{img_name}]]"), // dead link → keep as literal text
            }
        } else if let Some(m) = caps.get(2) {
            // -------- Regular wikilink:  [[file]], [[file|alias]], [[file#sec|alias]] --------
            let link_str = m.as_str().trim();

            // Split on `|` to extract optional alias.
            let (file_part, alias) = match link_str.find('|') {
                Some(pos) => (&link_str[..pos], Some(&link_str[pos + 1..])),
                None => (link_str, None),
            };

            // Strip `#section` anchor.
            let file_name = match file_part.find('#') {
                Some(pos) => &file_part[..pos],
                None => file_part,
            };

            let display = alias.unwrap_or(file_name);
            let key = file_name.to_lowercase();

            match md_index.get(&key) {
                Some(md_path) => {
                    let xhtml_path = md_path.with_extension("xhtml");
                    let rel = relative_link(source_dir, &xhtml_path);
                    format!("[{display}]({rel})")
                }
                None => display.to_string(), // dead link → plain text
            }
        } else {
            caps[0].to_string()
        }
    });

    result.into_owned()
}

/// Compute a URL-style relative path from `from_dir` (a directory) to `to_file`
/// (a file path), where both are relative to the same root (the EPUB/vault root).
///
/// Examples:
/// * from `""` to `"nested/note_a.xhtml"`  →  `"nested/note_a.xhtml"`
/// * from `"nested"` to `"index.xhtml"`    →  `"../index.xhtml"`
/// * from `"a/b"` to `"a/c/d.xhtml"`      →  `"../c/d.xhtml"`
pub fn relative_link(from_dir: &Path, to_file: &Path) -> String {
    // Normalise both paths to a Vec of plain string components, skipping any
    // empty components that arise from an empty `from_dir`.
    let from_parts: Vec<&str> = from_dir
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();

    let to_parts: Vec<&str> = to_file
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();

    // Find the length of the common prefix.
    let common = from_parts
        .iter()
        .zip(to_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // One ".." for every directory component in `from_dir` after the common prefix.
    let up = from_parts.len() - common;

    let mut parts: Vec<&str> = std::iter::repeat("..").take(up).collect();
    parts.extend_from_slice(&to_parts[common..]);

    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn md_idx(pairs: &[(&str, &str)]) -> HashMap<String, PathBuf> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), PathBuf::from(v)))
            .collect()
    }

    fn img_idx(pairs: &[(&str, &str)]) -> HashMap<String, PathBuf> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), PathBuf::from(v)))
            .collect()
    }

    // ── relative_link ────────────────────────────────────────────────────────

    #[test]
    fn relative_link_from_root() {
        assert_eq!(
            relative_link(Path::new(""), Path::new("nested/note_a.xhtml")),
            "nested/note_a.xhtml"
        );
    }

    #[test]
    fn relative_link_going_up() {
        assert_eq!(
            relative_link(Path::new("nested"), Path::new("index.xhtml")),
            "../index.xhtml"
        );
    }

    #[test]
    fn relative_link_same_dir() {
        assert_eq!(
            relative_link(Path::new("nested"), Path::new("nested/other.xhtml")),
            "other.xhtml"
        );
    }

    #[test]
    fn relative_link_cross_dirs() {
        assert_eq!(
            relative_link(Path::new("a/b"), Path::new("a/c/d.xhtml")),
            "../c/d.xhtml"
        );
    }

    // ── transform_wikilinks ──────────────────────────────────────────────────

    #[test]
    fn simple_wikilink() {
        let md = md_idx(&[("note_a", "nested/note_a.md")]);
        let img = img_idx(&[]);
        let out = transform_wikilinks("See [[note_a]]", Path::new("index.md"), &md, &img);
        assert_eq!(out, "See [note_a](nested/note_a.xhtml)");
    }

    #[test]
    fn wikilink_with_alias() {
        let md = md_idx(&[("note_a", "nested/note_a.md")]);
        let img = img_idx(&[]);
        let out =
            transform_wikilinks("See [[note_a|Go to A]]", Path::new("index.md"), &md, &img);
        assert_eq!(out, "See [Go to A](nested/note_a.xhtml)");
    }

    #[test]
    fn wikilink_with_section_stripped() {
        let md = md_idx(&[("note_a", "nested/note_a.md")]);
        let img = img_idx(&[]);
        let out =
            transform_wikilinks("See [[note_a#intro]]", Path::new("index.md"), &md, &img);
        assert_eq!(out, "See [note_a](nested/note_a.xhtml)");
    }

    #[test]
    fn wikilink_dead_link_degrades_to_text() {
        let md = md_idx(&[]);
        let img = img_idx(&[]);
        let out = transform_wikilinks("See [[missing]]", Path::new("index.md"), &md, &img);
        assert_eq!(out, "See missing");
    }

    #[test]
    fn image_embed() {
        let md = md_idx(&[]);
        let img = img_idx(&[("pic.png", "assets/pic.png")]);
        let out = transform_wikilinks("![[pic.png]]", Path::new("index.md"), &md, &img);
        assert_eq!(out, "![pic.png](assets/pic.png)");
    }

    #[test]
    fn image_embed_dead_link() {
        let md = md_idx(&[]);
        let img = img_idx(&[]);
        let out = transform_wikilinks("![[missing.png]]", Path::new("index.md"), &md, &img);
        assert_eq!(out, "![[missing.png]]");
    }

    #[test]
    fn backlink_from_nested_file() {
        let md = md_idx(&[("index", "index.md")]);
        let img = img_idx(&[]);
        let out = transform_wikilinks(
            "Back [[index]]",
            Path::new("nested/note_a.md"),
            &md,
            &img,
        );
        assert_eq!(out, "Back [index](../index.xhtml)");
    }
}
