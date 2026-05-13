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
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn run(iso_path: &str, output_dir: &str) -> Result<(), String> {
    fs::create_dir_all(output_dir).map_err(|e| format!("Create dir failed: {}", e))?;

    println!("Extracting ISO: {}", iso_path);
    println!("Output directory: {}", output_dir);

    // Parse ISO to get all files
    let iso_files = iso::parse_iso_files(Path::new(iso_path))?;
    println!("Found {} files in ISO", iso_files.len());

    // Build manifest entries (iso hashes)
    let iso_map = iso::build_iso_hash_map(Path::new(iso_path))?;

    // Export BMG JSON files into \ folders and collect metadata for manifest
    let mut bmg_metas = Vec::new();
    match crate::commands::exporter::export_bmg_from_iso(iso_path, output_dir) {
        Ok(m) => bmg_metas = m,
        Err(e) => eprintln!("Warning: BMG export failed: {}", e),
    }

    // Build entries map
    let mut entries_map = serde_json::Map::new();
    for (rel, hash) in iso_map {
        entries_map.insert(
            rel.clone(),
            json!({"iso": rel.clone(), "sha1": {"base": hash}}),
        );
    }

    // Add exported BMGs (archive internal files) so they appear in the manifest
    for meta in bmg_metas {
        // friendly path: text/<arc_stem>/<internal_no_ext>.json
        let arc_stem = std::path::Path::new(&meta.arc_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let internal = meta.bmg_filename.trim_start_matches('/');
        let internal_no_ext = internal.strip_suffix(".bmg").unwrap_or(internal);
        let friendly = format!("text/{}/{}.json", arc_stem, internal_no_ext);

        entries_map.insert(
            friendly,
            json!({
                "archive": meta.arc_path,
                "path": meta.bmg_filename,
                "sha1": {"base": meta.sha1}
            }),
        );
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
    fs::write(&manifest_path, manifest_str)
        .map_err(|e| format!("Failed to write manifest: {}", e))?;

    println!(
        "Export complete. Manifest written to {}/manifest.json",
        output_dir
    );

    Ok(())
}
