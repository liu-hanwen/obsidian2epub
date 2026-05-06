mod indexer;
mod metadata;
mod packager;
mod transformer;

use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::Parser;

use indexer::build_index;
use packager::build_epub;

/// Convert an Obsidian Vault to a standard EPUB e-book.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the Obsidian Vault directory (source)
    #[arg(short, long)]
    source: PathBuf,

    /// Output path for the generated `.epub` file
    #[arg(short, long)]
    output: PathBuf,

    /// Title of the EPUB book
    #[arg(short, long, default_value = "Obsidian Vault")]
    title: String,

    /// Author of the EPUB book
    #[arg(short, long, default_value = "Unknown")]
    author: String,
}

fn run(args: Args) -> Result<()> {
    let vault_root = args.source.canonicalize().unwrap_or(args.source.clone());

    if !vault_root.is_dir() {
        anyhow::bail!("Source path '{}' is not a directory", vault_root.display());
    }

    // Phase 1 – build the global file index.
    eprintln!("Scanning vault: {}", vault_root.display());
    let idx = build_index(&vault_root);
    eprintln!(
        "  Found {} Markdown file(s), {} image(s)",
        idx.md_index.len(),
        idx.img_index.len()
    );

    // Phases 2-4 – process files and write EPUB.
    eprintln!("Building EPUB…");
    build_epub(
        &vault_root,
        &args.output,
        &args.title,
        &args.author,
        &idx.md_index,
        &idx.img_index,
    )?;

    eprintln!("Done → {}", args.output.display());
    Ok(())
}

fn main() {
    let args = Args::parse();
    if let Err(e) = run(args) {
        eprintln!("Error: {e:#}");
        process::exit(1);
    }
}
