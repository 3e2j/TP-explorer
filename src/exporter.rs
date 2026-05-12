/*
Main ISO export pipeline: ISO → ARC → BMG → JSON
*/

use crate::bmg_export;
use crate::formats::bmg::Bmg;
use crate::formats::iso;
use crate::formats::rarc::Rarc;
use std::fs;

pub struct BmgExportMeta {
    pub iso_path: String,
    pub arc_path: String,
    pub bmg_filename: String,
}

pub fn export_bmg_from_iso(iso_path: &str, output_dir: &str) -> Result<Vec<BmgExportMeta>, String> {
    fs::create_dir_all(output_dir).map_err(|e| format!("Create dir failed: {}", e))?;

    let mut exported = Vec::new();
    let files = iso::parse_iso_files(std::path::Path::new(iso_path))?;

    println!("Found {} files in ISO", files.len());

    for file in files {
        if !file.path.ends_with(".arc") {
            continue;
        }
        if let Err(e) = process_arc_file(&file, iso_path, output_dir, &mut exported) {
            eprintln!("  Error processing {}: {}", file.path, e);
        }
    }

    println!("Exported {} BMG files", exported.len());
    Ok(exported)
}

fn process_arc_file(
    file: &iso::IsoFileEntry,
    iso_path: &str,
    output_dir: &str,
    exported: &mut Vec<BmgExportMeta>,
) -> Result<(), String> {
    let rel_path = file.path.strip_prefix("files/").unwrap_or(&file.path);
    let arc_data = iso::read_iso_file_bytes(std::path::Path::new(iso_path), rel_path)?;

    let rarc = Rarc::parse(arc_data).ok_or("Failed to parse RARC")?;

    let bmg_files: Vec<_> = rarc
        .list_files()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".bmg"))
        .collect();

    for (bmg_path, _) in bmg_files {
        if let Err(e) = export_single_bmg(&rarc, &bmg_path, &file.path, output_dir, exported) {
            eprintln!("    {}: {}", bmg_path, e);
        }
    }

    Ok(())
}

fn export_single_bmg(
    rarc: &Rarc,
    bmg_path: &str,
    arc_path: &str,
    output_dir: &str,
    exported: &mut Vec<BmgExportMeta>,
) -> Result<(), String> {
    println!("  Found BMG: {}", bmg_path);

    let entry = find_entry_in_rarc(rarc, bmg_path).ok_or("BMG not found in archive")?;
    let data = entry.data.as_ref().ok_or("No data for BMG")?;

    let bmg = Bmg::parse(data)?;
    let json = bmg_export::bmg_to_json(&bmg)?;

    let out_name = bmg_path.replace(".bmg", "").replace('/', "_");
    let out_path = format!("{}/{}.json", output_dir, out_name);

    bmg_export::write_json(&json, &out_path)?;

    println!("    Exported: {}", out_path);
    exported.push(BmgExportMeta {
        iso_path: arc_path.to_string(),
        arc_path: arc_path.to_string(),
        bmg_filename: bmg_path.to_string(),
    });

    Ok(())
}

fn find_entry_in_rarc<'a>(rarc: &'a Rarc, bmg_path: &str) -> Option<&'a crate::formats::rarc::FileEntry> {
    rarc.file_entries.iter().find(|e| {
        if e.is_dir {
            return false;
        }
        e.parent_node_index.and_then(|parent| {
            let node_path = rarc.node_path(parent);
            let full = if node_path.is_empty() {
                e.name.clone()
            } else {
                format!("{}/{}", node_path, e.name)
            };
            if full == bmg_path {
                Some(true)
            } else {
                None
            }
        }).is_some()
    })
}

