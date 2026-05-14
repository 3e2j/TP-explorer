/*
Export command entrypoint.

High-level sequence:
1) Prepare ISO source (decode wrapper when needed)
2) Decode files via ordered pipeline
3) Write manifest.json from decoded entries
*/

mod iso_source;
mod manifest;
mod pipeline;
pub mod consolidated_bmg;

use crate::formats::iso;
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
    let iso_files = iso::parse_iso_files(iso_path)?;
    println!("Found {} files in ISO", iso_files.len());

    let (entries, arcs) = pipeline::export_entries(iso_path, output_dir)?;
    manifest::write_manifest(output_dir, entries, arcs)?;

    println!(
        "Export complete. Manifest written to {}/manifest.json",
        output_dir.display()
    );
    Ok(())
}
