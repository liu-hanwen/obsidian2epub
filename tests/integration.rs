//! Integration test: build a mock vault and verify the EPUB is produced.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// Create the mock vault described in the problem statement:
///
/// ```text
/// mock_vault/
/// ├── index.md          (contains [[note_a|Go to A]], ![[pic.png]], and LaTeX math)
/// ├── nested/
/// │   └── note_a.md     (contains YAML frontmatter and back-link [[index]])
/// └── assets/
///     └── pic.png
/// ```
fn create_mock_vault(root: &Path) {
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::create_dir_all(root.join("assets")).unwrap();

    fs::write(
        root.join("index.md"),
        "# Index\n\nSee [[note_a|Go to A]] and the image below.\n\n![[pic.png]]\n\
        \nInline math: $E = mc^2$\n\nDisplay math:\n\n$$x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}$$\n",
    )
    .unwrap();

    fs::write(
        root.join("nested").join("note_a.md"),
        "---\ntitle: Note A\nauthor: Tester\n---\n\n# Note A\n\nBack to [[index]].\n",
    )
    .unwrap();

    // A minimal 1×1 white PNG (valid PNG bytes).
    let png_bytes: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk length + type
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // width=1, height=1
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // bit depth, color type, ...
        0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, // IDAT chunk
        0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
        0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC,
        0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, // IEND chunk
        0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    fs::write(root.join("assets").join("pic.png"), png_bytes).unwrap();
}

/// Locate the built binary.  We compile it in the test so the binary is
/// guaranteed to exist.
fn binary_path() -> PathBuf {
    // cargo test already built the binary; look in the standard target location.
    let mut p = std::env::current_exe().unwrap();
    // Walk up until we find the `target` directory.
    loop {
        p.pop();
        if p.ends_with("target") || p.file_name().map_or(false, |f| f == "target") {
            break;
        }
        if p.parent().is_none() {
            panic!("Could not locate target directory");
        }
    }
    // Look for the binary in target/debug or target/release.
    for profile in &["debug", "release"] {
        let bin = p.join(profile).join("obsidian2epub");
        if bin.exists() {
            return bin;
        }
    }
    panic!("obsidian2epub binary not found under {}", p.display());
}

#[test]
fn build_epub_from_mock_vault() {
    let vault_dir = TempDir::new().unwrap();
    create_mock_vault(vault_dir.path());

    let out_dir = TempDir::new().unwrap();
    let epub_path = out_dir.path().join("output.epub");

    let status = Command::new(binary_path())
        .args([
            "--source",
            vault_dir.path().to_str().unwrap(),
            "--output",
            epub_path.to_str().unwrap(),
            "--title",
            "Test Book",
        ])
        .status()
        .expect("failed to run obsidian2epub");

    assert!(status.success(), "CLI exited with non-zero status");
    assert!(epub_path.exists(), "EPUB file was not created");

    let metadata = fs::metadata(&epub_path).unwrap();
    assert!(metadata.len() > 0, "EPUB file is empty");
}

#[test]
fn epub_is_a_zip_file() {
    let vault_dir = TempDir::new().unwrap();
    create_mock_vault(vault_dir.path());

    let out_dir = TempDir::new().unwrap();
    let epub_path = out_dir.path().join("output.epub");

    Command::new(binary_path())
        .args([
            "--source",
            vault_dir.path().to_str().unwrap(),
            "--output",
            epub_path.to_str().unwrap(),
        ])
        .status()
        .unwrap();

    // EPUBs are ZIP files; first 4 bytes must be the PK signature.
    let bytes = fs::read(&epub_path).unwrap();
    assert!(bytes.len() >= 4);
    assert_eq!(&bytes[..4], b"PK\x03\x04", "EPUB should start with PK ZIP signature");
}

/// Verify that a vault with LaTeX math produces a valid EPUB (build must not fail).
#[test]
fn epub_with_latex_math() {
    let vault_dir = TempDir::new().unwrap();
    // index.md already contains LaTeX in create_mock_vault.
    create_mock_vault(vault_dir.path());

    let out_dir = TempDir::new().unwrap();
    let epub_path = out_dir.path().join("math.epub");

    let status = Command::new(binary_path())
        .args([
            "--source",
            vault_dir.path().to_str().unwrap(),
            "--output",
            epub_path.to_str().unwrap(),
            "--title",
            "Math Book",
        ])
        .status()
        .expect("failed to run obsidian2epub");

    assert!(status.success(), "CLI exited with non-zero status for LaTeX vault");
    assert!(epub_path.exists(), "EPUB file was not created for LaTeX vault");
    assert!(
        fs::metadata(&epub_path).unwrap().len() > 0,
        "EPUB file should not be empty"
    );
}
