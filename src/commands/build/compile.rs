/*
Compilation stage: convert JSON→BMG and other format conversions.
*/

use crate::commands::build::hash_check::ModifiedFile;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct CompiledFile {
    pub mod_file: ModifiedFile,
    pub compiled_bytes: Vec<u8>,
}

/// Compile all modified files (JSON→BMG, etc.)
pub fn compile_modified_files(
    modified_files: &[ModifiedFile],
    mod_dir: &Path,
) -> Result<Vec<CompiledFile>, String> {
    let mut compiled = Vec::new();

    for mod_file in modified_files {
        let bytes = fs::read(&mod_file.mod_path)
            .map_err(|e| format!("Read modified file failed: {}", e))?;

        let compiled_bytes = match mod_file.friendly_path.as_str() {
            // JSON → BMG conversion
            path if path.starts_with("text/") && path.ends_with(".json") => {
                let json_str =
                    String::from_utf8(bytes).map_err(|e| format!("JSON not UTF-8: {}", e))?;
                let json_val: serde_json::Value = serde_json::from_str(&json_str)
                    .map_err(|e| format!("Parse JSON failed: {}", e))?;

                let bmg = crate::formats::bmg::from_json::json_to_bmg(&json_val)?;
                bmg.to_bytes()?
            }
            // Future: AW → JSON conversion, DAE → BMD, etc.
            _ => {
                // No conversion needed, use as-is
                bytes
            }
        };

        compiled.push(CompiledFile {
            mod_file: mod_file.clone(),
            compiled_bytes,
        });
    }

    Ok(compiled)
}
