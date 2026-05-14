/*
Compilation stage: convert JSON→BMG and other format conversions.

Note: When text/messages.json is modified (consolidated BMG format), this function
splits it back into individual BMG files by source archive for proper arc reassembly.
Each source archive gets its own compiled BMG file in the output.
*/

use crate::commands::build::hash_check::ModifiedFile;
use crate::commands::export::consolidated_bmg::ConsolidatedBmg;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct CompiledFile {
    pub mod_file: ModifiedFile,
    pub compiled_bytes: Vec<u8>,
}

/// Compile all modified files (JSON→BMG, etc.)
/// 
/// Special handling for text/messages.json (consolidated BMG format):
/// - Parses consolidated format {version, sources}
/// - Splits back into individual BMG files by source
/// - Returns multiple CompiledFile entries (one per source archive) IF not all sources are specified
/// - For consolidated BMG with specific modified sources, returns only those sources
pub fn compile_modified_files(
    modified_files: &[ModifiedFile],
    _mod_dir: &Path,
) -> Result<Vec<CompiledFile>, String> {
    let mut compiled = Vec::new();

    // Check if we have consolidated BMG entries
    let consolidated_bmg_entries: Vec<_> = modified_files
        .iter()
        .filter(|m| m.friendly_path == "text/messages.json")
        .collect();

    // If we have consolidated BMG entries, read the file once and process selectively
    if !consolidated_bmg_entries.is_empty() {
        // All consolidated entries should point to the same file
        let mod_path = &consolidated_bmg_entries[0].mod_path;
        let bytes = fs::read(mod_path)
            .map_err(|e| format!("Read messages.json failed: {}", e))?;
        let json_str = String::from_utf8(bytes)
            .map_err(|e| format!("JSON not UTF-8: {}", e))?;
        let json_val: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("Parse consolidated BMG JSON failed: {}", e))?;

        let individual_bmgs = ConsolidatedBmg::to_individual_bmgs(&json_val)?;

        // Only compile the sources that were explicitly marked as modified
        for mod_file in consolidated_bmg_entries {
            if let (Some(archive), Some(path)) = (mod_file.archive.as_ref(), mod_file.internal_path.as_ref()) {
                // Look up this specific source in the consolidated BMG
                let key = (archive.clone(), path.clone());
                if let Some((bmg_json, encoding)) = individual_bmgs.get(&key) {
                    let mut bmg = crate::formats::bmg::from_json::json_to_bmg(bmg_json, encoding)?;
                    let compiled_bytes = bmg.to_bytes()?;

                    let mut source_mod_file = mod_file.clone();
                    source_mod_file.friendly_path = format!("{}/{}", archive.trim_start_matches("files/"), path);

                    compiled.push(CompiledFile {
                        mod_file: source_mod_file,
                        compiled_bytes,
                    });
                }
            }
        }
    }

    // Process non-consolidated files normally
    for mod_file in modified_files {
        // Skip consolidated BMG files (already handled above)
        if mod_file.friendly_path == "text/messages.json" {
            continue;
        }

        let bytes = fs::read(&mod_file.mod_path)
            .map_err(|e| format!("Read modified file failed: {}", e))?;

        let compiled_results = match mod_file.friendly_path.as_str() {
            // Standard JSON → BMG conversion (non-consolidated)
            path if path.starts_with("text/") && path.ends_with(".json") => {
                let json_str =
                    String::from_utf8(bytes).map_err(|e| format!("JSON not UTF-8: {}", e))?;
                let json_val: serde_json::Value = serde_json::from_str(&json_str)
                    .map_err(|e| format!("Parse JSON failed: {}", e))?;

                let bmg = crate::formats::bmg::from_json::json_to_bmg(&json_val, "shift-jis")?;
                let compiled_bytes = bmg.to_bytes()?;

                vec![CompiledFile {
                    mod_file: mod_file.clone(),
                    compiled_bytes,
                }]
            }
            // Future: AW → JSON conversion, DAE → BMD, etc.
            _ => {
                // No conversion needed, use as-is
                vec![CompiledFile {
                    mod_file: mod_file.clone(),
                    compiled_bytes: bytes,
                }]
            }
        };

        compiled.extend(compiled_results);
    }

    Ok(compiled)
}
