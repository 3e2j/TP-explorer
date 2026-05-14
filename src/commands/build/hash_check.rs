/*
Hash comparison stage: load manifest, find modified files.

Special handling for consolidated BMG (text/messages.json):
- Compares per-source BMG message hashes instead of the combined JSON hash
- Only files that actually changed are marked for rebuild
*/

use crate::commands::export::consolidated_bmg::ConsolidatedBmg;
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct ModifiedFile {
    pub friendly_path: String,
    pub mod_path: String,
    pub archive: Option<String>, // arc path if this file belongs to an arc
    pub internal_path: Option<String>, // path inside arc
}

/// Load manifest and find all modified files by comparing hashes.
///
/// Special handling for text/messages.json (consolidated BMG):
/// - Compares each BMG source individually instead of the combined JSON
/// - Only sources with changed hashes are marked as modified
pub fn find_modified_files(mod_dir: &Path) -> Result<Vec<ModifiedFile>, String> {
    let manifest_path = mod_dir.join("manifest.json");
    let manifest_content =
        fs::read_to_string(&manifest_path).map_err(|e| format!("Read manifest failed: {}", e))?;
    let manifest: Value = serde_json::from_str(&manifest_content)
        .map_err(|e| format!("Parse manifest failed: {}", e))?;

    let mut modified = Vec::new();

    // Get entries from manifest
    let entries = manifest
        .get("entries")
        .and_then(|e| e.as_object())
        .ok_or("Manifest missing 'entries' object")?;

    for (friendly_path, entry_val) in entries {
        let entry = entry_val.as_object().ok_or("Entry is not an object")?;

        // Special handling for consolidated BMG
        if friendly_path == "text/messages.json" {
            check_consolidated_bmg_changes(mod_dir, &mut modified, entry)?;
            continue;
        }

        // Standard file comparison
        let original_hash = entry
            .get("sha1")
            .and_then(read_manifest_sha1)
            .ok_or(format!("Entry {} missing sha1", friendly_path))?
            .to_string();

        let mod_file_path = mod_dir.join(friendly_path);
        if mod_file_path.exists() {
            let new_hash = compute_file_hash(&mod_file_path)?;

            if new_hash != original_hash {
                let archive = entry
                    .get("archive")
                    .and_then(|a| a.as_str())
                    .map(|s| s.to_string());
                let internal_path = entry
                    .get("path")
                    .and_then(|p| p.as_str())
                    .map(|s| s.to_string());

                modified.push(ModifiedFile {
                    friendly_path: friendly_path.clone(),
                    mod_path: mod_file_path.to_string_lossy().to_string(),
                    archive,
                    internal_path,
                });
            }
        }
    }

    Ok(modified)
}

/// Check consolidated BMG file for per-source changes.
/// Returns ModifiedFile entries only for sources with changed hashes.
fn check_consolidated_bmg_changes(
    mod_dir: &Path,
    modified: &mut Vec<ModifiedFile>,
    manifest_entry: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let mod_messages_path = mod_dir.join("text/messages.json");
    if !mod_messages_path.exists() {
        return Ok(());
    }

    // Read modified consolidated JSON
    let json_str = fs::read_to_string(&mod_messages_path)
        .map_err(|e| format!("Read messages.json failed: {}", e))?;
    let consolidated: Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Parse messages.json failed: {}", e))?;

    // Get manifest sources with their original hashes
    let manifest_sources = manifest_entry
        .get("sources")
        .and_then(|s| s.as_array())
        .ok_or("Consolidated BMG missing sources in manifest")?;

    // Create a map of (archive, path) -> original hash for fast lookup
    let mut original_hashes: std::collections::HashMap<(String, String), String> =
        std::collections::HashMap::new();
    for src in manifest_sources {
        let archive = src
            .get("archive")
            .and_then(|a| a.as_str())
            .ok_or("Source missing archive in manifest")?
            .to_string();
        let path = src
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or("Source missing path in manifest")?
            .to_string();
        let hash = src
            .get("sha1")
            .and_then(read_manifest_sha1)
            .ok_or("Source missing sha1 in manifest")?
            .to_string();

        original_hashes.insert((archive.clone(), path.clone()), hash);
    }

    // Convert to individual BMGs and compute per-source hashes
    let individual_bmgs = ConsolidatedBmg::to_individual_bmgs(&consolidated)?;

    for ((archive, path), (bmg_json, encoding)) in individual_bmgs {
        // Compute hash of this source's editable identity (encoding + messages)
        let source_hash = serde_json::to_vec_pretty(&serde_json::json!({
            "encoding": encoding,
            "messages": bmg_json
        }))
        .map_err(|e| format!("Serialize source BMG failed: {}", e))?;
        let new_hash = sha1_hex(&source_hash);

        // Look up original hash
        let original_hash = original_hashes
            .get(&(archive.clone(), path.clone()))
            .cloned()
            .unwrap_or_default();

        // If hash changed, mark as modified
        if new_hash != original_hash {
            modified.push(ModifiedFile {
                friendly_path: "text/messages.json".to_string(),
                mod_path: mod_messages_path.to_string_lossy().to_string(),
                archive: Some(archive),
                internal_path: Some(path),
            });
        }
    }

    Ok(())
}

fn compute_file_hash(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Read file failed: {}", e))?;
    Ok(sha1_hex(&bytes))
}

fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = sha1::Sha1::new();
    hasher.update(bytes);
    hasher.digest().to_string()
}

fn read_manifest_sha1(v: &Value) -> Option<&str> {
    match v {
        Value::String(s) => Some(s),
        Value::Object(obj) => obj.get("base").and_then(|h| h.as_str()),
        _ => None,
    }
}
