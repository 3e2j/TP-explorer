use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

pub fn write_manifest(
    output_dir: &Path,
    entries: Map<String, Value>,
    arcs: Vec<String>,
) -> Result<(), String> {
    let manifest = json!({
        "version": 1,
        "game": {"id": "GZ2E", "region": "NTSC-U", "platform": "gamecube"},
        "arcs": arcs,
        "entries": Value::Object(entries)
    });

    let manifest_path = output_dir.join("manifest.json");
    let manifest_str = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    fs::write(&manifest_path, manifest_str)
        .map_err(|e| format!("Failed to write manifest: {}", e))?;
    Ok(())
}
