/*
Export command: Extracts files from ISO into mod-friendly directory structure.

This command:
1. Reads a vanilla ISO
2. Extracts all game files into a organized, human-readable folder structure
3. Generates manifest.json mapping mod paths to their ISO locations

The manifest.json is never hand-edited by modders and is used by the compile command
to resolve mod files back to their ISO paths.
*/

use crate::formats::iso;
use std::fs;
use std::path::Path;
use serde_json::json;

pub fn run(iso_path: &str, output_dir: &str) -> Result<(), String> {
    fs::create_dir_all(output_dir).map_err(|e| format!("Create dir failed: {}", e))?;

    println!("Extracting ISO: {}", iso_path);
    println!("Output directory: {}", output_dir);

    // Parse ISO to get all files
    let iso_files = iso::parse_iso_files(Path::new(iso_path))?;
    println!("Found {} files in ISO", iso_files.len());

    // Build manifest entries (iso hashes)
    let iso_map = iso::build_iso_hash_map(Path::new(iso_path))?;

    // Export BMG JSON files into \ folders
    if let Err(e) = crate::commands::exporter::export_bmg_from_iso(iso_path, output_dir) {
        eprintln!("Warning: BMG export failed: {}", e);
    }

    // Build entries map
    let mut entries_map = serde_json::Map::new();
    for (rel, hash) in iso_map {
        entries_map.insert(rel.clone(), json!({"iso": rel.clone(), "sha1": {"base": hash}}));
    }

    // Write manifest
    let manifest = json!({
        "version": 1,
        "game": {"id": "GZ2E", "region": "NTSC-U", "platform": "gamecube"},
        "entries": serde_json::Value::Object(entries_map)
    });

    let manifest_path = Path::new(output_dir).join("manifest.json");
    let manifest_str = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    fs::write(&manifest_path, manifest_str).map_err(|e| format!("Failed to write manifest: {}", e))?;

    println!("Export complete. Manifest written to {}/manifest.json", output_dir);

    Ok(())
}
