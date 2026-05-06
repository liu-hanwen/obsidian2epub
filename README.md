# obsidian2epub

A lightweight, high-performance CLI tool written in **Rust** that compiles an [Obsidian](https://obsidian.md/) Vault into a standard EPUB e-book — ready for offline readers such as KOReader on e-ink devices.

---

## Features

| Capability | Detail |
|---|---|
| **Vault scanning** | Recursively indexes all `.md` files and common image formats (`png`, `jpg`, `jpeg`, `gif`, `webp`, `svg`) |
| **YAML frontmatter** | Strips `---` … `---` blocks; extracts the `title:` field as the chapter title |
| **Wikilinks** | Converts `[[file]]`, `[[file\|alias]]`, and `[[file#section\|alias]]` to standard HTML links |
| **Image embeds** | Converts `![[image.png]]` to standard `<img>` tags; bundles the image inside the EPUB |
| **Dead links** | Gracefully degrades to plain text when a link target is not found |
| **Section anchors** | Strips `#section` suffixes (e.g. `[[file#header]]` → links to `file`) |
| **EPUB output** | Produces a valid EPUB 2 file with auto-generated TOC, stylesheet, and correct relative paths |

---

## Installation

```bash
# Clone the repository
git clone https://github.com/liu-hanwen/obsidian2epub.git
cd obsidian2epub

# Build in release mode
cargo build --release

# The binary is at:
./target/release/obsidian2epub
```

---

## Usage

```
obsidian2epub --source <VAULT_DIR> --output <OUTPUT.epub> [OPTIONS]

Options:
  -s, --source <SOURCE>    Path to the Obsidian Vault directory
  -o, --output <OUTPUT>    Output path for the generated .epub file
  -t, --title <TITLE>      Title of the EPUB book [default: "Obsidian Vault"]
  -a, --author <AUTHOR>    Author of the EPUB book [default: "Unknown"]
  -h, --help               Print help
  -V, --version            Print version
```

### Example

```bash
obsidian2epub \
  --source ~/Documents/MyVault \
  --output ~/Desktop/MyBook.epub \
  --title "My Notes" \
  --author "Jane Doe"
```

---

## Mock Vault Structure (for testing)

```text
mock_vault/
├── index.md          ([[note_a|Go to A]], ![[pic.png]])
├── nested/
│   └── note_a.md     (YAML frontmatter, [[index]] back-link)
└── assets/
    └── pic.png
```

```bash
obsidian2epub --source mock_vault --output out.epub --title "Test Book"
```

---

## Running Tests

```bash
cargo test
```

---

## Architecture

```
src/
├── main.rs          CLI entry point (clap argument parsing, top-level orchestration)
├── indexer.rs       Phase 1 – Walk vault, build md_index + img_index HashMaps
├── metadata.rs      Phase 2 – Strip YAML frontmatter, extract title
├── transformer.rs   Phase 3 – Convert Obsidian wikilinks to standard Markdown
└── packager.rs      Phase 4 – Render Markdown→XHTML, embed images, write EPUB
```

### Dependencies

| Crate | Purpose |
|---|---|
| `clap` | CLI argument parsing |
| `walkdir` | Recursive directory traversal |
| `regex` | Wikilink pattern matching |
| `pulldown-cmark` | Markdown → HTML rendering |
| `epub-builder` | EPUB file generation |
| `anyhow` | Ergonomic error handling |

---

## License

MIT
