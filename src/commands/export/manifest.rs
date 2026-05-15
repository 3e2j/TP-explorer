use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Write manifest.json. This writes a hoisted "archives" map keyed by
/// archive ISO path. Each archive maps to an object of friendly_path -> entry
/// (the full manifest entry). The top-level "arcs" list is omitted.
pub fn write_manifest(output_dir: &Path, entries: Map<String, Value>) -> Result<(), String> {
    // Build a hoisted archives map: archive -> (friendly_path -> entry)
    let mut archives_map: BTreeMap<String, Map<String, Value>> = BTreeMap::new();

    // Build sanitized entries (remove 'archive' fields) for storing in both
    // top-level entries and the per-archive hoisted map.
    let mut sanitized_entries: Map<String, Value> = Map::new();
    for (friendly, val) in &entries {
        let mut cloned = val.clone();
        if let Some(obj) = cloned.as_object_mut() {
            obj.remove("archive");
            if let Some(sources) = obj.get_mut("sources").and_then(|v| v.as_array_mut()) {
                for src in sources.iter_mut() {
                    if let Some(src_obj) = src.as_object_mut() {
                        src_obj.remove("archive");
                    }
                }
            }
        }
        sanitized_entries.insert(friendly.clone(), cloned);
    }

    for (friendly, val) in &sanitized_entries {
        // If entry has a path & archive originally, find which archive(s) it belongs to
        // by checking the original entries (which included archive) via the input `entries` map.
        if let Some(orig) = entries.get(friendly).and_then(|v| v.as_object()) {
            if let Some(archive_val) = orig.get("archive").and_then(|v| v.as_str()) {
                archives_map
                    .entry(archive_val.to_string())
                    .or_insert_with(Map::new)
                    .insert(friendly.clone(), val.clone());
            }
            if let Some(sources) = orig.get("sources").and_then(|v| v.as_array()) {
                for src in sources {
                    if let Some(arc) = src.get("archive").and_then(|v| v.as_str()) {
                        archives_map
                            .entry(arc.to_string())
                            .or_insert_with(Map::new)
                            .insert(friendly.clone(), val.clone());
                    }
                }
            }
        }
    }

    let mut archives_json = Map::new();
    for (k, v) in archives_map {
        archives_json.insert(k, Value::Object(v));
    }

    let manifest = json!({
        "version": 1,
        "game": {"id": "GZ2E", "region": "NTSC-U", "platform": "gamecube"},
        // hoisted archives map for quick lookup (archive -> { friendly_path: entry })
        "archives": Value::Object(archives_json),
        "entries": Value::Object(sanitized_entries)
    });

    let manifest_path = output_dir.join("manifest.json");
    let manifest_str = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    fs::write(&manifest_path, manifest_str)
        .map_err(|e| format!("Failed to write manifest: {}", e))?;
    Ok(())
}
