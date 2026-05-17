//! Export pipeline for TPMT.
//!
//! This command extracts a vanilla ISO into the mod folder layout, decodes
//! supported formats into editable representations, and writes
//! `manifest.json` describing how the exported files map back to the disc.
//!
//! The submodules handle source preparation, per-file decoding, consolidated
//! BMG export, and manifest generation.

pub mod consolidated_bmg;
pub mod iso_source;
pub mod manifest;
pub mod pipeline;

use crate::formats::iso::iso_read;
use std::fs;
use std::path::Path;

pub fn run(iso_path: &str, output_dir: &str) -> Result<(), String> {
    fs::create_dir_all(output_dir).map_err(|e| format!("Create dir failed: {}", e))?;

    println!("Extracting ISO: {}", iso_path);
    println!("Output directory: {}", output_dir);

    let prepared = iso_source::prepare_for_export(Path::new(iso_path))?;
    let result = run_export_pipeline(prepared.path(), Path::new(output_dir));

    if let Err(e) = prepared.cleanup() {
        eprintln!(
            "Warning: failed to remove temporary ISO {}: {}",
            prepared.path().display(),
            e
        );
    }

    result
}

fn run_export_pipeline(iso_path: &Path, output_dir: &Path) -> Result<(), String> {
    let iso_files = iso_read::parse_iso_files(iso_path)?;
    println!("Found {} files in ISO", iso_files.len());

    let entries = pipeline::export_entries(iso_path, output_dir)?;
    manifest::write_manifest(output_dir, entries)?;

    println!(
        "Export complete. Manifest written to {}/manifest.json",
        output_dir.display()
    );
    Ok(())
}
