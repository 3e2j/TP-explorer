/*
Decode pipeline for export.

Sequence (add new decoders below by calling process_<format>_entries):
1) All ISO entries are registered as direct ISO manifest entries
2) ARC archive decoder
3) BMG decoder (processes BMG files inside ARC archives)
4) Future: add more decoders here (e.g., AW, DAE, etc.)
*/

use crate::formats::bmg::Bmg;
use crate::formats::iso;
use crate::formats::rarc::Rarc;
use serde_json::{json, Map, Value};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub struct ProcessContext<'a> {
    entries: &'a mut Map<String, Value>,
    arcs: &'a mut Vec<String>,
    output_dir: &'a Path,
    bmg_count: &'a mut usize,
}

struct ParsedArchive {
    iso_path: String,
    stem: String,
    rarc: Rarc,
}

pub fn export_entries(
    iso_path: &Path,
    output_dir: &Path,
) -> Result<(Map<String, Value>, Vec<String>), String> {
    fs::create_dir_all(output_dir).map_err(|e| format!("Create dir failed: {}", e))?;

    let files = iso::parse_iso_files(iso_path)?;
    let mut entries = Map::new();
    let mut arcs = Vec::new();
    let mut iso_file =
        std::fs::File::open(iso_path).map_err(|e| format!("Failed to open ISO: {e}"))?;
    let mut bmg_count = 0usize;

    for file in files {
        let rel_iso_path = file
            .path
            .strip_prefix("files/")
            .unwrap_or(&file.path)
            .to_string();
        let bytes = read_iso_entry_bytes(&mut iso_file, file.offset, file.size)?;

        insert_direct_iso_entry(&mut entries, &rel_iso_path, &bytes);

        // Single ARC parse pass: build type-specific entry stacks for bulk processing
        if file.path.ends_with(".arc") {
            let rarc = match Rarc::parse(bytes.clone()) {
                Some(r) => r,
                None => {
                    eprintln!("  Warning: failed to parse ARC {}", file.path);
                    continue;
                }
            };

            let stem = match archive_stem(&file.path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let archive = ParsedArchive {
                iso_path: file.path.clone(),
                stem,
                rarc,
            };

            // Track this arc
            arcs.push(archive.iso_path.clone());

            // Organize entries by type during single parse pass
            let mut bmg_entries = Vec::new();
            let mut other_entries = Vec::new();
            // Future: build other type stacks here (aw_entries, dae_entries, etc.)

            for entry in &archive.rarc.file_entries {
                if entry.is_dir {
                    continue;
                }
                let Some(_data) = &entry.data else {
                    continue;
                };

                let internal_path = rarc_entry_path(&archive.rarc, entry);

                if internal_path.ends_with(".bmg") {
                    bmg_entries.push(internal_path);
                } else {
                    // Collect all other files for manifest
                    other_entries.push(internal_path);
                }
                // Future: add other type checks here
            }

            // Process all organized entry stacks in bulk
            let mut ctx = ProcessContext {
                entries: &mut entries,
                arcs: &mut arcs,
                output_dir,
                bmg_count: &mut bmg_count,
            };

            process_bmg_entries(&mut ctx, &archive, &bmg_entries)?;
            process_other_arc_files(&mut ctx, &archive, &other_entries)?;
            // Future: call process_aw_entries_bulk(&mut ctx, &archive, &aw_entries)?;
        }
    }

    println!("Exported {} BMG files", bmg_count);
    Ok((entries, arcs))
}

fn insert_direct_iso_entry(entries: &mut Map<String, Value>, rel_iso_path: &str, bytes: &[u8]) {
    entries.insert(
        rel_iso_path.to_string(),
        json!({
            "iso": rel_iso_path,
            "sha1": { "base": sha1_hex(bytes) }
        }),
    );
}

// Decoders

/// BMG decoder: converts BMG files to JSON and saves them.
/// Processes all BMG entries from a single ARC parse in bulk.
fn process_bmg_entries(
    ctx: &mut ProcessContext,
    archive: &ParsedArchive,
    bmg_entry_paths: &[String],
) -> Result<(), String> {
    for internal_path in bmg_entry_paths {
        let entry = archive
            .rarc
            .file_entries
            .iter()
            .find(|e| !e.is_dir && rarc_entry_path(&archive.rarc, e) == *internal_path);

        let Some(entry) = entry else {
            continue;
        };
        let Some(data) = &entry.data else {
            continue;
        };

        let bmg = match Bmg::parse(data) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "  Warning: failed to parse BMG {}/{}: {}",
                    archive.iso_path, internal_path, e
                );
                continue;
            }
        };

        let json_val = crate::formats::bmg::to_json::bmg_to_json(&bmg)?;
        let json_bytes = serde_json::to_vec_pretty(&json_val)
            .map_err(|e| format!("Serialize JSON failed: {}", e))?;

        let friendly_path = match get_friendly_path(&archive.iso_path, &archive.stem, internal_path)
        {
            Some(p) => p,
            None => continue,
        };
        write_output_file(ctx.output_dir, &friendly_path, &json_bytes)?;

        ctx.entries.insert(
            friendly_path,
            json!({
                "archive": archive.iso_path,
                "path": internal_path,
                "sha1": { "base": sha1_hex(&json_bytes) }
            }),
        );

        *ctx.bmg_count += 1;
    }

    Ok(())
}

/// Track all non-exported files from archives in the manifest.
/// These files are not exported but are tracked as dependencies for rebuilding.
fn process_other_arc_files(
    ctx: &mut ProcessContext,
    archive: &ParsedArchive,
    other_entry_paths: &[String],
) -> Result<(), String> {
    for internal_path in other_entry_paths {
        // Generate a friendly path for reference (even though file isn't exported)
        let friendly_path = format!("{}/{}", archive.stem, internal_path);

        ctx.entries.insert(
            friendly_path,
            json!({
                "archive": archive.iso_path,
                "path": internal_path,
            }),
        );
    }

    Ok(())
}

fn archive_stem(archive_iso_path: &str) -> Result<String, String> {
    Path::new(archive_iso_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .ok_or("Invalid arc path".to_string())
}

fn read_iso_entry_bytes(
    file: &mut std::fs::File,
    offset: u64,
    size: u64,
) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Failed to seek ISO data: {e}"))?;
    let mut out = vec![0u8; size as usize];
    file.read_exact(&mut out)
        .map_err(|e| format!("Failed to read ISO data: {e}"))?;
    Ok(out)
}

fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = sha1::Sha1::new();
    hasher.update(bytes);
    hasher.digest().to_string()
}

fn write_output_file(output_dir: &Path, rel_path: &str, bytes: &[u8]) -> Result<(), String> {
    let out_path = output_dir.join(rel_path);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Create dir failed: {}", e))?;
    }
    fs::write(&out_path, bytes)
        .map_err(|e| format!("Write error ({}): {}", out_path.display(), e))?;
    Ok(())
}

fn rarc_entry_path(rarc: &Rarc, entry: &crate::formats::rarc::FileEntry) -> String {
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

/// Generates the friendly path for a file exported from an archive.
/// Handles format conversions (.bmg → .json, .aw → .json, .dae, etc.)
/// and assigns to the appropriate top-level directory based on ISO location.
fn get_friendly_path(
    archive_iso_path: &str,
    archive_stem: &str,
    internal_path: &str,
) -> Option<String> {
    // Determine category and output format based on file type and archive location
    let category = if archive_iso_path.contains("Object/") {
        "actors"
    } else if archive_iso_path.contains("Stage/") {
        "stages"
    } else if archive_iso_path.contains("Audiores/") {
        "audio"
    } else if archive_iso_path.contains("misc/") {
        "ui"
    } else {
        "res"
    };

    // Format conversions: binary → editable
    if internal_path.ends_with(".bmg") {
        let internal_no_ext = internal_path.strip_suffix(".bmg").unwrap_or(internal_path);
        return Some(format!("text/{}/{}.json", archive_stem, internal_no_ext));
    }

    if internal_path.ends_with(".aw") {
        let internal_no_ext = internal_path.strip_suffix(".aw").unwrap_or(internal_path);
        return Some(format!(
            "audio/waves/{}/{}.json",
            archive_stem, internal_no_ext
        ));
    }

    if internal_path.ends_with(".dae") {
        let internal_no_ext = internal_path.strip_suffix(".dae").unwrap_or(internal_path);
        return Some(format!(
            "{}/{}/{}.dae",
            category, archive_stem, internal_no_ext
        ));
    }

    if internal_path.ends_with(".bmd") {
        return Some(format!("{}/{}/{}", category, archive_stem, internal_path));
    }

    // Default: preserve inside arc folder structure
    Some(format!("{}/{}/{}", category, archive_stem, internal_path))
}
