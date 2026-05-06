//! Phase 4 – EPUB Packager
//!
//! Reads every `.md` file from the vault, runs it through the metadata
//! processor and syntax transformer, converts it to XHTML, and then bundles
//! everything (text chapters + image resources) into a single `.epub` file.

use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use epub_builder::{EpubBuilder, EpubContent, ReferenceType, ZipLibrary};
use latex2mathml::{latex_to_mathml, DisplayStyle};
use pulldown_cmark::{html, Event, Options, Parser};
use walkdir::WalkDir;

use crate::metadata::strip_frontmatter;
use crate::transformer::transform_wikilinks;

/// Convert an `epub_builder::Result` (which uses `eyre::Report`) into an
/// `anyhow::Result` so callers can use `?` uniformly throughout this module.
fn eb<T>(r: epub_builder::Result<T>) -> Result<T> {
    r.map_err(|e| anyhow::anyhow!("{e}"))
}

/// Build the EPUB from the vault and write it to `output_path`.
pub fn build_epub(
    vault_root: &Path,
    output_path: &Path,
    book_title: &str,
    book_author: &str,
    md_index: &HashMap<String, PathBuf>,
    img_index: &HashMap<String, PathBuf>,
) -> Result<()> {
    // Collect all .md files in a deterministic order.
    let md_files: Vec<PathBuf> = WalkDir::new(vault_root)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file()
                && e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case("md"))
                    .unwrap_or(false)
        })
        .map(|e| e.into_path())
        .collect();

    let mut builder = eb(EpubBuilder::new(eb(ZipLibrary::new())?))?;
    eb(builder.metadata("title", book_title))?;
    eb(builder.metadata("author", book_author))?;
    eb(builder.metadata("generator", "obsidian2epub"))?;

    // Minimal stylesheet embedded in the EPUB.
    let css = "body{font-family:serif;line-height:1.6;margin:2em 3em}\
               h1,h2,h3,h4,h5,h6{line-height:1.3}\
               img{max-width:100%;height:auto}\
               a{color:#0066cc}\
               code,pre{font-family:monospace;background:#f4f4f4;border-radius:3px}\
               pre{padding:1em;overflow-x:auto}\
               blockquote{border-left:4px solid #ccc;margin-left:0;padding-left:1em;color:#555}";
    eb(builder.stylesheet(css.as_bytes()))?;

    // Process and add each Markdown file as an XHTML chapter.
    let mut first_text = true;
    for abs_path in &md_files {
        let raw = fs::read_to_string(abs_path)
            .with_context(|| format!("reading {}", abs_path.display()))?;

        let rel_path = abs_path.strip_prefix(vault_root)?.to_path_buf();
        let stem = rel_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");

        let (clean_md, title) = strip_frontmatter(&raw, stem);
        let transformed = transform_wikilinks(&clean_md, &rel_path, md_index, img_index);

        let html_body = markdown_to_html(&transformed);
        let stylesheet_href = stylesheet_rel_href(&rel_path);
        let xhtml = wrap_xhtml(&title, &stylesheet_href, &html_body);

        let xhtml_rel = rel_path.with_extension("xhtml");
        let xhtml_str = path_to_epub_str(&xhtml_rel);

        // Depth = number of parent directory components (root file → 0 → level 1,
        // one folder deep → 1 → level 2, etc.).
        let depth = rel_path
            .components()
            .filter(|c| matches!(c, Component::Normal(_)))
            .count()
            .saturating_sub(1); // subtract 1 for the file component itself
        let toc_level = (depth as i32) + 1;

        let mut content = EpubContent::new(&xhtml_str, xhtml.as_bytes())
            .title(title.as_str())
            .level(toc_level);
        if first_text {
            content = content.reftype(ReferenceType::Text);
            first_text = false;
        }
        eb(builder.add_content(content))
            .with_context(|| format!("adding chapter {xhtml_str}"))?;
    }

    // Add image resources.
    for (_, img_rel) in img_index {
        let abs_img = vault_root.join(img_rel);
        if let Ok(data) = fs::read(&abs_img) {
            let mime = mime_for(img_rel);
            let path_str = path_to_epub_str(img_rel);
            eb(builder.add_resource(&path_str, data.as_slice(), mime))
                .with_context(|| format!("adding resource {path_str}"))?;
        }
    }

    // Write the final EPUB file.
    let mut out = fs::File::create(output_path)
        .with_context(|| format!("creating output file {}", output_path.display()))?;
    eb(builder.generate(&mut out))?;

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Convert a LaTeX formula string to a MathML element string.
///
/// On conversion failure the raw LaTeX is returned as a `<code>` (inline)
/// or `<pre><code>` (block) fallback so the EPUB is never broken by
/// unsupported syntax.
fn math_to_mathml(latex: &str, display: DisplayStyle) -> String {
    latex_to_mathml(latex, display).unwrap_or_else(|_| match display {
        DisplayStyle::Inline => format!("<code>{}</code>", xml_escape(latex)),
        DisplayStyle::Block => format!("<pre><code>{}</code></pre>", xml_escape(latex)),
    })
}

/// Render CommonMark Markdown (with math extensions) to an HTML fragment string.
/// LaTeX formulas (`$…$` inline, `$$…$$` display) are converted to MathML.
fn markdown_to_html(markdown: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_MATH);
    let parser = Parser::new_ext(markdown, opts);

    // Convert InlineMath / DisplayMath events to MathML; pass everything else through.
    let events = parser.map(|event| -> Event<'static> {
        match event {
            Event::InlineMath(latex) => {
                Event::Html(math_to_mathml(latex.as_ref(), DisplayStyle::Inline).into())
            }
            Event::DisplayMath(latex) => {
                Event::Html(math_to_mathml(latex.as_ref(), DisplayStyle::Block).into())
            }
            e => e.into_static(),
        }
    });

    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}

/// Wrap an HTML body fragment in a complete XHTML document.
fn wrap_xhtml(title: &str, stylesheet_href: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head>
  <meta charset="UTF-8"/>
  <title>{title}</title>
  <link rel="stylesheet" type="text/css" href="{stylesheet_href}"/>
</head>
<body>
{body}
</body>
</html>
"#,
        title = xml_escape(title),
        stylesheet_href = stylesheet_href,
        body = body,
    )
}

/// Compute the relative href to `stylesheet.css` from the depth of `xhtml_rel_path`.
///
/// A file at the root (depth 0) uses `"stylesheet.css"`.
/// A file one level deep (e.g. `nested/note.xhtml`) uses `"../stylesheet.css"`.
fn stylesheet_rel_href(xhtml_rel_path: &Path) -> String {
    let depth = xhtml_rel_path
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .count()
        .saturating_sub(1); // subtract 1 for the file itself

    if depth == 0 {
        "stylesheet.css".to_string()
    } else {
        let prefix: Vec<&str> = std::iter::repeat("..").take(depth).collect();
        format!("{}/stylesheet.css", prefix.join("/"))
    }
}

/// Convert a `PathBuf` to a forward-slash string suitable for use inside an EPUB.
fn path_to_epub_str(path: &Path) -> String {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Return the MIME type string for the given image path.
fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

/// Escape characters that are special in XML attribute values and content.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stylesheet_href_at_root() {
        assert_eq!(stylesheet_rel_href(Path::new("index.xhtml")), "stylesheet.css");
    }

    #[test]
    fn stylesheet_href_one_deep() {
        assert_eq!(
            stylesheet_rel_href(Path::new("nested/note.xhtml")),
            "../stylesheet.css"
        );
    }

    #[test]
    fn stylesheet_href_two_deep() {
        assert_eq!(
            stylesheet_rel_href(Path::new("a/b/note.xhtml")),
            "../../stylesheet.css"
        );
    }

    #[test]
    fn path_to_epub_str_converts_slashes() {
        // On Windows, Path might use backslashes; we always want forward slashes.
        let p = PathBuf::from("nested").join("note.xhtml");
        assert_eq!(path_to_epub_str(&p), "nested/note.xhtml");
    }

    #[test]
    fn xml_escape_special_chars() {
        assert_eq!(xml_escape("a & b < c > d \"e\""), "a &amp; b &lt; c &gt; d &quot;e&quot;");
    }

    #[test]
    fn markdown_to_html_inline_math_produces_mathml() {
        let html = markdown_to_html("Inline: $E = mc^2$");
        assert!(
            html.contains("<math"),
            "inline math should produce a <math> element; got: {html}"
        );
    }

    #[test]
    fn markdown_to_html_display_math_produces_mathml() {
        let html = markdown_to_html("$$x = 1$$");
        assert!(
            html.contains("<math"),
            "display math should produce a <math> element; got: {html}"
        );
    }
}
