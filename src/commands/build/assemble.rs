/*
Assembly stage: rebuild modified archives by:
1. Loading original arc from ISO
2. Swapping in modified files
3. Repackaging into arc format
4. Writing to output
*/

use crate::commands::build::compile::CompiledFile;
use crate::formats::iso;
use crate::formats::rarc::Rarc;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Rebuild modified archives with compiled files and dependencies.
pub fn build_archives(
    compiled: &[CompiledFile],
    iso_path: &Path,
    mod_dir: &Path,
    output_dir: &Path,
) -> Result<(), String> {
    // Load manifest to understand file->arc mapping
    let manifest_path = mod_dir.join("manifest.json");
    let manifest_content =
        fs::read_to_string(&manifest_path).map_err(|e| format!("Read manifest failed: {}", e))?;
    let manifest: Value = serde_json::from_str(&manifest_content)
        .map_err(|e| format!("Parse manifest failed: {}", e))?;

    // Group compiled files by archive
    let mut arc_modifications: HashMap<String, HashMap<String, Vec<u8>>> = HashMap::new();
    for compiled_file in compiled {
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
        return Ok(());
    }

    // Rebuild each modified arc
    let mut iso_file =
        std::fs::File::open(iso_path).map_err(|e| format!("Open ISO failed: {}", e))?;
    let iso_entries = iso::parse_iso_files(iso_path)?;

    for (arc_iso_path, modifications) in arc_modifications {
        // Load original arc from ISO
        let arc_iso_entry = iso_entries
            .iter()
            .find(|f| f.path == arc_iso_path)
            .ok_or(format!("Arc not found in ISO: {}", arc_iso_path))?;

        let arc_bytes =
            read_iso_entry_bytes(&mut iso_file, arc_iso_entry.offset, arc_iso_entry.size)?;
        let mut rarc =
            Rarc::parse(arc_bytes).ok_or(format!("Failed to parse arc: {}", arc_iso_path))?;

        // Build a map of entry paths to indices for faster lookup
        let mut entry_paths: HashMap<usize, String> = HashMap::new();
        for (idx, entry) in rarc.file_entries.iter().enumerate() {
            if !entry.is_dir {
                entry_paths.insert(idx, entry_internal_path(&rarc, entry));
            }
        }

        // Apply modifications: swap file data
        for (idx, entry) in rarc.file_entries.iter_mut().enumerate() {
            if !entry.is_dir {
                if let Some(entry_path) = entry_paths.get(&idx) {
                    if let Some(new_data) = modifications.get(entry_path) {
                        entry.data = Some(new_data.clone());
                        entry.data_size = new_data.len() as u32;
                    }
                }
            }
        }

        // Repackage and write (compressed for ISO compatibility)
        let rebuilt_bytes = rarc.to_bytes_compressed()?;
        let output_path = output_dir.join(&arc_iso_path);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Create dir failed: {}", e))?;
        }
        fs::write(&output_path, rebuilt_bytes).map_err(|e| format!("Write arc failed: {}", e))?;

        println!("  Rebuilt arc: {}", arc_iso_path);
    }

    Ok(())
}

fn entry_internal_path(rarc: &Rarc, entry: &crate::formats::rarc::FileEntry) -> String {
    if let Some(parent_idx) = entry.parent_node_index {
        let node_path = rarc.node_path(parent_idx);
        if node_path.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", node_path, entry.name)
        }
    } else {
        entry.name.clone()
    }
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
