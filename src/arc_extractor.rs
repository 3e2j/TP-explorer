/*
Extract all .arc files from a directory to an output folder.
Each ARC file's contents are extracted, preserving internal directory structure.
*/

use std::fs;
use std::path::Path;
use crate::formats::rarc::Rarc;

pub fn extract_arc_files(input_dir: &str, output_dir: &str) -> Result<Vec<String>, String> {
    let input_path = Path::new(input_dir);
    let output_path = Path::new(output_dir);

    if !input_path.is_dir() {
        return Err(format!("Input is not a directory: {}", input_dir));
    }

    fs::create_dir_all(output_path)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    let mut extracted_files = Vec::new();
    extract_arc_files_recursive(input_path, output_path, &mut extracted_files)?;

    println!("Extracted {} files total", extracted_files.len());
    Ok(extracted_files)
}

fn extract_arc_files_recursive(
    input_dir: &Path,
    output_base: &Path,
    extracted_files: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(input_dir)
        .map_err(|e| format!("Failed to read directory {}: {}", input_dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            extract_arc_files_recursive(&path, output_base, extracted_files)?;
        } else if path.extension().map_or(false, |ext| ext == "arc") {
            extract_single_arc(&path, output_base, extracted_files)?;
        }
    }

    Ok(())
}

fn extract_single_arc(
    arc_path: &Path,
    output_base: &Path,
    extracted_files: &mut Vec<String>,
) -> Result<(), String> {
    let arc_filename = arc_path
        .file_stem()
        .ok_or("Invalid arc filename")?
        .to_string_lossy()
        .to_string();

    println!("Extracting: {}", arc_path.display());

    let data = fs::read(arc_path)
        .map_err(|e| format!("Failed to read {}: {}", arc_path.display(), e))?;

    let rarc = Rarc::parse(data).ok_or(format!("Failed to parse RARC: {}", arc_path.display()))?;

    let arc_output_dir = output_base.join(&arc_filename);
    fs::create_dir_all(&arc_output_dir)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    extract_rarc_contents(&rarc, &arc_output_dir, extracted_files)?;

    println!("  Extracted {} files", extracted_files.len());
    Ok(())
}

fn extract_rarc_contents(
    rarc: &Rarc,
    output_dir: &Path,
    extracted_files: &mut Vec<String>,
) -> Result<(), String> {
    for entry in &rarc.file_entries {
        if entry.is_dir {
            continue;
        }

        let file_path = get_file_path_in_rarc(rarc, entry);
        let output_path = output_dir.join(&file_path);

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        if let Some(data) = &entry.data {
            fs::write(&output_path, data)
                .map_err(|e| format!("Failed to write {}: {}", output_path.display(), e))?;
            
            let relative_path = output_path
                .strip_prefix(&output_dir)
                .unwrap_or(&output_path)
                .to_string_lossy()
                .to_string();
            extracted_files.push(relative_path);
        }
    }

    Ok(())
}

fn get_file_path_in_rarc(rarc: &Rarc, entry: &crate::formats::rarc::FileEntry) -> String {
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
