/*
Hash comparison stage: load manifest, find modified files.
*/

use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct ModifiedFile {
    pub friendly_path: String,
    pub mod_path: String,
    pub original_hash: String,
    pub archive: Option<String>, // arc path if this file belongs to an arc
    pub internal_path: Option<String>, // path inside arc
}

/// Load manifest and find all modified files by comparing hashes.
pub fn find_modified_files(mod_dir: &Path) -> Result<Vec<ModifiedFile>, String> {
    let manifest_path = mod_dir.join("manifest.json");
    let manifest_content =
        fs::read_to_string(&manifest_path).map_err(|e| format!("Read manifest failed: {}", e))?;
    let manifest: Value =
        serde_json::from_str(&manifest_content).map_err(|e| format!("Parse manifest failed: {}", e))?;

    let mut modified = Vec::new();

    // Get entries from manifest
    let entries = manifest
        .get("entries")
        .and_then(|e| e.as_object())
        .ok_or("Manifest missing 'entries' object")?;

    for (friendly_path, entry_val) in entries {
        let entry = entry_val
            .as_object()
            .ok_or("Entry is not an object")?;

        // Get original hash from manifest
        let original_hash = entry
            .get("sha1")
            .and_then(|s| s.get("base"))
            .and_then(|h| h.as_str())
            .ok_or(format!("Entry {} missing sha1.base", friendly_path))?
            .to_string();

        // Check if file exists in mod directory
        let mod_file_path = mod_dir.join(friendly_path);
        if mod_file_path.exists() {
            // Check if file has changed by computing new hash
            let new_hash = compute_file_hash(&mod_file_path)?;
            if new_hash != original_hash {
                let archive = entry.get("archive").and_then(|a| a.as_str()).map(|s| s.to_string());
                let internal_path = entry
                    .get("path")
                    .and_then(|p| p.as_str())
                    .map(|s| s.to_string());

                modified.push(ModifiedFile {
                    friendly_path: friendly_path.clone(),
                    mod_path: mod_file_path.to_string_lossy().to_string(),
                    original_hash,
                    archive,
                    internal_path,
                });
            }
        }
    }

    Ok(modified)
}

fn compute_file_hash(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Read file failed: {}", e))?;
    let mut hasher = sha1::Sha1::new();
    hasher.update(&bytes);
    Ok(hasher.digest().to_string())
}
