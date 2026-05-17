use crate::commands::build::compile::CompiledFile;
use crate::formats::iso::iso_read;
use crate::utils::read_bytes_at;
use serde_json::Value;
use std::collections::HashMap;
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
    let iso_entries = iso_read::parse_iso_files(iso_path)?;
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
        let arc_bytes = read_bytes_at(&mut iso_file, arc_iso_entry.offset, arc_iso_entry.size)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::build::compile::CompiledFile;
    use crate::commands::build::hash_check::ModifiedFile;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("tpmt-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn build_single_file_iso(file_name: &str, file_bytes: &[u8]) -> std::path::PathBuf {
        let fst_offset = 0x1000usize;
        let file_offset = 0x1200usize;
        let name_bytes = format!("{file_name}\0").into_bytes();
        let fst_size = 24 + name_bytes.len();
        let mut bytes = vec![0u8; file_offset + file_bytes.len()];
        bytes[0x424..0x428].copy_from_slice(&(fst_offset as u32).to_be_bytes());
        bytes[0x428..0x42C].copy_from_slice(&(fst_size as u32).to_be_bytes());
        bytes[fst_offset..fst_offset + 12].copy_from_slice(&[
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
        ]);
        bytes[fst_offset + 12..fst_offset + 24].copy_from_slice(&[
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x12,
            0x00,
            ((file_bytes.len() as u32) >> 24) as u8,
            ((file_bytes.len() as u32) >> 16) as u8,
            ((file_bytes.len() as u32) >> 8) as u8,
            (file_bytes.len() as u32) as u8,
        ]);
        bytes[fst_offset + 24..fst_offset + 24 + name_bytes.len()].copy_from_slice(&name_bytes);
        bytes[file_offset..file_offset + file_bytes.len()].copy_from_slice(file_bytes);
        let path =
            std::env::temp_dir().join(format!("tpmt-archive-plan-{}.iso", std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, bytes).expect("write iso");
        path
    }

    // Verifies archive entries with complete manifest coverage can be rebuilt from mods only.
    #[test]
    fn plan_archive_inputs_uses_mods_only_when_manifest_is_complete() {
        let mod_dir = temp_dir("archive-plan-mod");
        std::fs::write(
            mod_dir.join("manifest.json"),
            r#"{"archives":{"files/a.arc":{"foo.txt":{"path":"foo.txt","sha1":"abc"}}},"entries":{}}"#,
        )
        .expect("manifest");
        let iso_path = build_single_file_iso("a.arc", b"ARC");
        let compiled = vec![CompiledFile {
            mod_file: ModifiedFile {
                friendly_path: "a.arc/foo.txt".to_string(),
                mod_path: "ignored".to_string(),
                archive: Some("files/a.arc".to_string()),
                internal_path: Some("foo.txt".to_string()),
            },
            compiled_bytes: b"new".to_vec(),
        }];
        assert!(matches!(
            plan_archive_inputs(&compiled, &mod_dir, &iso_path).unwrap()[0],
            ArchiveInput::FromModsOnly { .. }
        ));
    }

    // Verifies missing archive mappings fail early so the build does not guess at archive layout.
    #[test]
    fn plan_archive_inputs_rejects_missing_manifest_archives() {
        let mod_dir = temp_dir("archive-plan-missing");
        std::fs::write(mod_dir.join("manifest.json"), r#"{"entries":{}}"#).expect("manifest");
        let iso_path = build_single_file_iso("a.arc", b"ARC");
        let compiled = vec![CompiledFile {
            mod_file: ModifiedFile {
                friendly_path: "a.arc/foo.txt".to_string(),
                mod_path: "ignored".to_string(),
                archive: Some("files/a.arc".to_string()),
                internal_path: Some("foo.txt".to_string()),
            },
            compiled_bytes: b"new".to_vec(),
        }];
        assert!(plan_archive_inputs(&compiled, &mod_dir, &iso_path).is_err());
    }
}
