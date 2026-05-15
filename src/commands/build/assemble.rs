/*
Assembly stage: rebuild modified archives by:
1. Loading original arc from ISO
2. Swapping in modified files
3. Repackaging into arc format
*/

use crate::commands::build::archive_plan::ArchiveInput;
use crate::formats::rarc::builder::RarcBuilder;
use crate::formats::rarc::Rarc;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct BuildOutput {
    pub path: String,
    pub bytes: Vec<u8>,
}

pub fn assemble_archives(inputs: &[ArchiveInput]) -> Result<Vec<BuildOutput>, String> {
    let mut built = Vec::new();

    for input in inputs {
        match input {
            ArchiveInput::FromModsOnly {
                arc_iso_path,
                modifications,
            } => {
                let rebuilt_bytes = rebuild_arc_from_modifications(modifications)?;
                built.push(BuildOutput {
                    path: arc_iso_path.clone(),
                    bytes: rebuilt_bytes,
                });
            }
            ArchiveInput::FromIso {
                arc_iso_path,
                modifications,
                arc_bytes,
            } => {
                let mut rarc = Rarc::parse(arc_bytes.clone())
                    .ok_or(format!("Failed to parse arc: {}", arc_iso_path))?;
                let entry_paths = map_entry_paths(&rarc);
                apply_modifications_to_rarc(&mut rarc, &entry_paths, modifications);
                let rebuilt_bytes = rarc.to_bytes_compressed()?;
                built.push(BuildOutput {
                    path: arc_iso_path.clone(),
                    bytes: rebuilt_bytes,
                });
            }
        }
    }

    Ok(built)
}

fn map_entry_paths(rarc: &Rarc) -> HashMap<usize, String> {
    let mut entry_paths = HashMap::new();
    for (idx, entry) in rarc.file_entries.iter().enumerate() {
        if !entry.is_dir {
            let path = if let Some(parent_idx) = entry.parent_node_index {
                let node_path = rarc.node_path(parent_idx);
                if node_path.is_empty() {
                    entry.name.clone()
                } else {
                    format!("{}/{}", node_path, entry.name)
                }
            } else {
                entry.name.clone()
            };
            entry_paths.insert(idx, path);
        }
    }
    entry_paths
}

fn apply_modifications_to_rarc(
    rarc: &mut Rarc,
    entry_paths: &HashMap<usize, String>,
    modifications: &HashMap<String, Vec<u8>>,
) {
    for (idx, entry) in rarc.file_entries.iter_mut().enumerate() {
        if entry.is_dir {
            continue;
        }
        if let Some(path) = entry_paths.get(&idx) {
            if let Some(new_data) = modifications.get(path) {
                entry.data = Some(new_data.clone());
                entry.data_size = new_data.len() as u32;
            }
        }
    }
}

fn rebuild_arc_from_modifications(
    modifications: &HashMap<String, Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let mut builder = RarcBuilder::new();
    for (internal_path, data) in modifications {
        builder = builder.add_file(internal_path.clone(), data.clone());
    }
    let rarc = builder.build();
    rarc.to_bytes_compressed()
}
