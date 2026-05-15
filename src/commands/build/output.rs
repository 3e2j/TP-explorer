use crate::commands::build::assemble::BuildOutput;
use crate::commands::build::compile::CompiledFile;
use crate::formats::iso::iso;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn collect_direct_outputs(compiled: &[CompiledFile]) -> Vec<BuildOutput> {
    let mut replacements = Vec::new();
    for compiled_file in compiled {
        replacements.push(BuildOutput {
            path: compiled_file.mod_file.friendly_path.clone(),
            bytes: compiled_file.compiled_bytes.clone(),
        });
    }
    replacements
}

pub fn write_outputs(
    output_dir: &Path,
    outputs: &[BuildOutput],
) -> Result<HashMap<String, Vec<u8>>, String> {
    let mut replacements = HashMap::new();

    for output in outputs {
        write_output_file(output_dir, &output.path, &output.bytes)?;
        replacements.insert(output.path.clone(), output.bytes.clone());
    }

    Ok(replacements)
}

pub fn rebuild_iso_from_outputs(
    iso_path: &Path,
    output_dir: &Path,
    arc_paths: &[String],
    direct_replacements: &HashMap<String, Vec<u8>>,
    iso_out: &str,
) -> Result<(), String> {
    println!("\nRebuilding ISO with modified files...");

    let mut replacements = HashMap::new();
    for arc_iso_path in arc_paths {
        let rebuilt_arc_path = output_dir.join(arc_iso_path);
        if rebuilt_arc_path.exists() {
            let arc_data = fs::read(&rebuilt_arc_path)
                .map_err(|e| format!("Failed to read rebuilt arc {}: {}", arc_iso_path, e))?;
            replacements.insert(arc_iso_path.clone(), arc_data);
        }
    }

    for (path, bytes) in direct_replacements {
        replacements.insert(path.clone(), bytes.clone());
    }

    let all_iso_files = iso::parse_iso_files(iso_path)?;
    let iso_out_path = Path::new(iso_out);
    crate::formats::iso::iso_rebuild::rebuild_iso_with_files(
        iso_path,
        iso_out_path,
        &replacements,
        &all_iso_files,
    )?;

    println!("ISO rebuild complete: {}", iso_out);
    Ok(())
}

fn write_output_file(output_dir: &Path, rel_path: &str, bytes: &[u8]) -> Result<(), String> {
    let output_path = output_dir.join(rel_path);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Create dir failed: {}", e))?;
    }
    fs::write(&output_path, bytes).map_err(|e| format!("Write file failed: {}", e))?;
    Ok(())
}
