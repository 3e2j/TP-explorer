/*
Main ISO export pipeline: ISO → ARC → BMG → JSON
*/

use crate::formats::bmg::Bmg;
use crate::formats::iso;
use crate::formats::rarc::Rarc;
use std::fs;
use std::path::Path;

pub struct BmgExportMeta {
    pub iso_path: String,
    pub arc_path: String,
    pub bmg_filename: String,
    pub sha1: String,
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
    let rarc_data = iso::read_iso_file_bytes(std::path::Path::new(iso_path), rel_path)?;

    let rarc = Rarc::parse(rarc_data).ok_or("Failed to parse RARC")?;

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
    let json = crate::formats::bmg::to_json::bmg_to_json(&bmg)?;

    // Build output path: output_dir/text/<arc_basename>/<internal_path>.json
    let arc_stem = Path::new(arc_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Invalid arc path")?;

    let internal = bmg_path.trim_start_matches('/');
    let internal_no_ext = internal.strip_suffix(".bmg").unwrap_or(internal);

    let out_path = Path::new(output_dir)
        .join("text")
        .join(arc_stem)
        .join(internal_no_ext.to_string() + ".json");

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Create dir failed: {}", e))?;
    }

    // Write JSON text and compute its SHA1
    let json_str =
        serde_json::to_string_pretty(&json).map_err(|e| format!("Serialize JSON failed: {}", e))?;
    crate::formats::bmg::to_json::write_json(&json, &out_path.to_string_lossy())?;

    println!("    Exported: {}", out_path.display());

    // Compute SHA-1 of the original BMG bytes for the manifest
    let mut hasher = sha1::Sha1::new();
    hasher.update(data);
    let hash = hasher.digest().to_string();

    // Compute SHA-1 of the exported JSON (so build can detect changes by comparing JSON hashes)
    let mut jhasher = sha1::Sha1::new();
    jhasher.update(json_str.as_bytes());

    exported.push(BmgExportMeta {
        iso_path: arc_path.to_string(),
        arc_path: arc_path.to_string(),
        bmg_filename: bmg_path.to_string(),
        sha1: hash,
    });

    Ok(())
}

fn find_entry_in_rarc<'a>(
    rarc: &'a Rarc,
    bmg_path: &str,
) -> Option<&'a crate::formats::rarc::FileEntry> {
    rarc.file_entries.iter().find(|e| {
        if e.is_dir {
            return false;
        }
        e.parent_node_index
            .and_then(|parent| {
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
            })
            .is_some()
    })
}
