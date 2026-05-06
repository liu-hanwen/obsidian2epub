//! Phase 2 – Metadata / YAML Frontmatter Processor
//!
//! Strips the optional YAML frontmatter block (delimited by `---`) from the
//! top of a Markdown document and extracts the `title` field when present.

use regex::Regex;
use std::sync::OnceLock;

/// Compiled regex for detecting a YAML frontmatter block at the top of a file.
fn frontmatter_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // `(?s)` makes `.` match newlines; `\A` anchors to the very start of input.
        Regex::new(r"(?s)\A---\r?\n(.*?)\r?\n---\r?\n?").unwrap()
    })
}

/// Strip YAML frontmatter from `content` and return `(clean_markdown, title)`.
///
/// * If a frontmatter block is present and contains a `title:` field, that
///   value is used as the title.
/// * Otherwise `fallback_title` (typically the file's stem) is used.
pub fn strip_frontmatter(content: &str, fallback_title: &str) -> (String, String) {
    if let Some(caps) = frontmatter_re().captures(content) {
        let yaml = caps.get(1).map_or("", |m| m.as_str());
        let title = extract_title(yaml).unwrap_or_else(|| fallback_title.to_string());
        let rest = &content[caps.get(0).unwrap().end()..];
        (rest.to_string(), title)
    } else {
        (content.to_string(), fallback_title.to_string())
    }
}

/// Scan the YAML block line-by-line and return the value of the `title` key.
fn extract_title(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        if let Some(rest) = line.strip_prefix("title:") {
            let t = rest.trim().trim_matches('"').trim_matches('\'');
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_frontmatter_and_extracts_title() {
        let content = "---\ntitle: My Note\ndate: 2024-01-01\n---\n# Body\n";
        let (md, title) = strip_frontmatter(content, "fallback");
        assert_eq!(title, "My Note");
        assert_eq!(md.trim(), "# Body");
    }

    #[test]
    fn falls_back_to_filename_when_no_title_field() {
        let content = "---\ndate: 2024-01-01\n---\n# Body\n";
        let (md, title) = strip_frontmatter(content, "my-note");
        assert_eq!(title, "my-note");
        assert_eq!(md.trim(), "# Body");
    }

    #[test]
    fn no_frontmatter_returns_content_unchanged() {
        let content = "# Just a heading\n\nSome text.\n";
        let (md, title) = strip_frontmatter(content, "fallback");
        assert_eq!(title, "fallback");
        assert_eq!(md, content);
    }

    #[test]
    fn title_with_quotes_is_trimmed() {
        let content = "---\ntitle: \"Quoted Title\"\n---\nBody\n";
        let (_, title) = strip_frontmatter(content, "fallback");
        assert_eq!(title, "Quoted Title");
    }
}
