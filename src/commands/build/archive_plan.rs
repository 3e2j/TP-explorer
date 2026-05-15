use crate::commands::build::compile::CompiledFile;
use crate::formats::iso::iso;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Clone, Debug)]
pub enum ArchiveInput {
    FromIso {
        arc_iso_path: String,
        modifications: HashMap<String, Vec<u8>>,
        arc_bytes: Vec<u8>,
    },
    FromModsOnly {
        arc_iso_path: String,
        modifications: HashMap<String, Vec<u8>>,
    },
}

pub fn plan_archive_inputs(
    compiled_archive_files: &[CompiledFile],
    mod_dir: &Path,
    iso_path: &Path,
) -> Result<Vec<ArchiveInput>, String> {
    let manifest = load_manifest(mod_dir)?;
    let manifest_archives = load_manifest_archives(&manifest)?;

    let mut arc_modifications = HashMap::new();
    for compiled_file in compiled_archive_files {
        if let Some(arc_path) = &compiled_file.mod_file.archive {
            let internal_path = compiled_file
                .mod_file
                .internal_path
                .as_ref()
                .ok_or("Compiled file missing internal_path")?;
            arc_modifications
                .entry(arc_path.clone())
                .or_insert_with(HashMap::new)
                .insert(internal_path.clone(), compiled_file.compiled_bytes.clone());
        }
    }

    if arc_modifications.is_empty() {
        return Ok(Vec::new());
    }

    let arc_modifications: HashMap<String, HashMap<String, Vec<u8>>> = arc_modifications
        .into_iter()
        .filter(|(arc, _)| {
            if manifest_archives.contains_key(arc) {
                true
            } else {
                eprintln!("  Skipping arc not present in manifest archives: {}", arc);
                false
            }
        })
        .collect();

    let mut arc_paths: Vec<String> = arc_modifications.keys().cloned().collect();
    arc_paths.sort_by(|a, b| b.matches('/').count().cmp(&a.matches('/').count()));

    let mut iso_file =
        std::fs::File::open(iso_path).map_err(|e| format!("Open ISO failed: {}", e))?;
    let iso_entries = iso::parse_iso_files(iso_path)?;
    let mut inputs = Vec::new();

    for arc_iso_path in arc_paths {
        let modifications = arc_modifications
            .get(&arc_iso_path)
            .cloned()
            .expect("arc missing");
        if can_rebuild_archive_without_iso(&arc_iso_path, &modifications, &manifest_archives)? {
            println!(
                "  Rebuilding {} from modifications only (no ISO fetch)",
                arc_iso_path
            );
            inputs.push(ArchiveInput::FromModsOnly {
                arc_iso_path,
                modifications,
            });
            continue;
        }

        let arc_iso_entry = iso_entries
            .iter()
            .find(|f| f.path == arc_iso_path)
            .ok_or(format!("Arc not found in ISO: {}", arc_iso_path))?;
        let arc_bytes =
            read_iso_entry_bytes(&mut iso_file, arc_iso_entry.offset, arc_iso_entry.size)?;

        inputs.push(ArchiveInput::FromIso {
            arc_iso_path,
            modifications,
            arc_bytes,
        });
    }

    Ok(inputs)
}

fn load_manifest(mod_dir: &Path) -> Result<Value, String> {
    let manifest_path = mod_dir.join("manifest.json");
    let manifest_content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Read manifest failed: {}", e))?;
    serde_json::from_str(&manifest_content).map_err(|e| format!("Parse manifest failed: {}", e))
}

fn load_manifest_archives(
    manifest: &Value,
) -> Result<HashMap<String, serde_json::Map<String, Value>>, String> {
    let archives_obj = manifest
        .get("archives")
        .and_then(|v| v.as_object())
        .ok_or("Manifest missing required 'archives' mapping")?;
    if archives_obj.is_empty() {
        return Err("Manifest missing required 'archives' mapping".to_string());
    }

    let mut manifest_archives = HashMap::new();
    for (arc, archive_obj) in archives_obj {
        let map = archive_obj
            .as_object()
            .ok_or("Manifest archive mapping is not an object")?;
        manifest_archives.insert(arc.clone(), map.clone());
    }

    if manifest_archives.is_empty() {
        return Err("Manifest archives mapping is empty".to_string());
    }

    Ok(manifest_archives)
}

fn can_rebuild_archive_without_iso(
    arc_iso_path: &str,
    modifications: &HashMap<String, Vec<u8>>,
    manifest_archives: &HashMap<String, serde_json::Map<String, Value>>,
) -> Result<bool, String> {
    let Some(archive_entries) = manifest_archives.get(arc_iso_path) else {
        return Ok(false);
    };

    let mut internal_paths = std::collections::HashSet::new();
    for (_friendly, entry_val) in archive_entries {
        let obj = match entry_val.as_object() {
            Some(obj) => obj,
            None => continue,
        };

        if let Some(p) = obj.get("path").and_then(|v| v.as_str()) {
            internal_paths.insert(p.to_string());
        }
        if let Some(sources) = obj.get("sources").and_then(|v| v.as_array()) {
            for src in sources {
                if let Some(src_arc) = src.get("archive").and_then(|v| v.as_str()) {
                    if src_arc == arc_iso_path {
                        if let Some(p) = src.get("path").and_then(|v| v.as_str()) {
                            internal_paths.insert(p.to_string());
                        }
                    }
                }
            }
        }
    }

    if internal_paths.is_empty() {
        return Ok(false);
    }

    Ok(internal_paths
        .iter()
        .all(|path| modifications.contains_key(path)))
}

fn read_iso_entry_bytes(
    file: &mut std::fs::File,
    offset: u64,
    size: u64,
) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Seek failed: {}", e))?;
    let mut out = vec![0u8; size as usize];
    file.read_exact(&mut out)
        .map_err(|e| format!("Read failed: {}", e))?;
    Ok(out)
}
