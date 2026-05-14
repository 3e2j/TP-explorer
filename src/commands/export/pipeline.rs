/*
Decode pipeline for export.

Stages:
1) Parse ISO - register all unpackaged (non-arc) files as ISO manifest entries
2) Unpack ARCs - recursively unpack all .arc files into a flat ArcEntry list;
                 register all arcs (including nested) in the top-level arcs array
3) Process arc entries - run format-specific decoders over the flat ArcEntry list:
     a) Plain files  - register as archive manifest entries
     b) BMG decoder  - consolidate all .bmg files into text/messages.json
     // Future: c) AW decoder, d) DAE decoder, etc.
4) Finalize - write consolidated output files and insert multi-source manifest entries
*/

use crate::commands::export::consolidated_bmg::{BmgSource, ConsolidatedBmg};
use crate::formats::bmg::Bmg;
use crate::formats::iso::iso;
use crate::formats::rarc::Rarc;
use serde_json::{json, Map, Value};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// A single file (non-arc) unpacked from within an ARC (or nested ARC).
/// `top_level_arc` is always the ISO-addressable arc path.
/// `internal_path` is the full path inside that arc, including any nested arc segments.
struct ArcEntry {
    top_level_arc: String,
    internal_path: String,
    data: Vec<u8>,
}

pub fn export_entries(
    iso_path: &Path,
    output_dir: &Path,
) -> Result<(Map<String, Value>, Vec<String>), String> {
    fs::create_dir_all(output_dir).map_err(|e| format!("Create dir failed: {}", e))?;

    // Parse ISO
    let files = iso::parse_iso_files(iso_path)?;
    let mut iso_file =
        std::fs::File::open(iso_path).map_err(|e| format!("Failed to open ISO: {e}"))?;

    let mut entries = Map::new();
    let mut arcs: Vec<String> = Vec::new();
    let mut arc_entries: Vec<ArcEntry> = Vec::new();

    for file in files {
        let rel_iso_path = file
            .path
            .strip_prefix("files/")
            .unwrap_or(&file.path)
            .to_string();
        let bytes = read_iso_entry_bytes(&mut iso_file, file.offset, file.size)?;

        if file.path.ends_with(".arc") {
            // Unpack ARCs recursively - handled below
            unpack_arc(
                &file.path,
                &file.path,
                "",
                &bytes,
                &mut arcs,
                &mut arc_entries,
            )?;
        } else {
            insert_direct_iso_entry(&mut entries, &rel_iso_path, &bytes);
        }
    }

    // Process arc entries

    process_plain_entries(&arc_entries, &mut entries)?;

    let mut consolidated_bmg = ConsolidatedBmg::new();
    process_bmg_entries(&arc_entries, &mut consolidated_bmg)?;

    // Finalize
    finalize_bmg_export(&mut entries, output_dir, &consolidated_bmg)?;
    // Future: finalize_aw_export(...)?;

    println!(
        "Exported {} BMG sources to text/messages.json",
        consolidated_bmg.sources.len()
    );
    Ok((entries, arcs))
}

// ARC unpacking

/// Recursively unpacks an ARC, registering it in `arcs` and collecting all
/// non-arc files as ArcEntry values. Nested arcs are registered with a composite
/// path (e.g. "files/Foo.arc/bar.arc") but all entries always reference the
/// top-level ISO arc via `top_level_arc`.
fn unpack_arc(
    top_level_arc: &str,
    current_arc: &str,
    prefix: &str,
    bytes: &[u8],
    arcs: &mut Vec<String>,
    arc_entries: &mut Vec<ArcEntry>,
) -> Result<(), String> {
    let rarc = match Rarc::parse(bytes.to_vec()) {
        Some(r) => r,
        None => {
            eprintln!("  Warning: failed to parse ARC {}", current_arc);
            return Ok(());
        }
    };

    arcs.push(current_arc.to_string());

    for entry in &rarc.file_entries {
        if entry.is_dir {
            continue;
        }
        let Some(data) = &entry.data else {
            continue;
        };

        let internal_path = rarc_entry_path(&rarc, entry);
        let full_path = if prefix.is_empty() {
            internal_path.clone()
        } else {
            format!("{}/{}", prefix, internal_path)
        };

        if internal_path.ends_with(".arc") {
            // Nested arc: register with composite path, recurse with same top_level_arc
            let nested_arc_path = format!("{}/{}", current_arc, internal_path);
            unpack_arc(
                top_level_arc,
                &nested_arc_path,
                &full_path,
                data,
                arcs,
                arc_entries,
            )?;
        } else {
            arc_entries.push(ArcEntry {
                top_level_arc: top_level_arc.to_string(),
                internal_path: full_path,
                data: data.clone(),
            });
        }
    }

    Ok(())
}

// Format processors

/// 3a) Plain files: registers all arc entries that have no dedicated decoder
/// as standard archive manifest entries.
fn process_plain_entries(
    arc_entries: &[ArcEntry],
    entries: &mut Map<String, Value>,
) -> Result<(), String> {
    for entry in arc_entries
        .iter()
        .filter(|e| !is_decoded_format(&e.internal_path))
    {
        let stem = archive_stem(&entry.top_level_arc).unwrap_or_default();
        let friendly_path = get_friendly_path(&entry.top_level_arc, &stem, &entry.internal_path)
            .unwrap_or_else(|| format!("{}/{}", stem, entry.internal_path));

        entries.insert(
            friendly_path,
            json!({
                "archive": entry.top_level_arc,
                "path": entry.internal_path,
                "sha1": sha1_hex(&entry.data)
            }),
        );
    }
    Ok(())
}

/// BMG decoder: collects all .bmg files for later consolidation into
/// a single text/messages.json file.
fn process_bmg_entries(
    arc_entries: &[ArcEntry],
    consolidated_bmg: &mut ConsolidatedBmg,
) -> Result<(), String> {
    for entry in arc_entries
        .iter()
        .filter(|e| e.internal_path.ends_with(".bmg"))
    {
        let bmg = match Bmg::parse(&entry.data) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "  Warning: failed to parse BMG {}/{}: {}",
                    entry.top_level_arc, entry.internal_path, e
                );
                continue;
            }
        };

        let json_val = crate::formats::bmg::to_json::bmg_to_json(&bmg)?;
        let source = BmgSource::from_bmg(
            entry.top_level_arc.clone(),
            entry.internal_path.clone(),
            bmg.encoding.clone(),
            json_val,
        );
        consolidated_bmg.add_source(source);
    }
    Ok(())
}

// Finalize

/// Writes text/messages.json and inserts the multi-source manifest entry.
fn finalize_bmg_export(
    entries: &mut Map<String, Value>,
    output_dir: &Path,
    consolidated_bmg: &ConsolidatedBmg,
) -> Result<(), String> {
    if consolidated_bmg.sources.is_empty() {
        return Ok(());
    }

    let consolidated_json = consolidated_bmg.to_json();
    let json_bytes = serde_json::to_vec_pretty(&consolidated_json)
        .map_err(|e| format!("Serialize consolidated BMG JSON failed: {}", e))?;

    write_output_file(output_dir, "text/messages.json", &json_bytes)?;

    let sources_for_manifest: Vec<Value> = consolidated_bmg
        .sources
        .iter()
        .map(|src| {
            let source_messages_json =
                serde_json::to_vec_pretty(&serde_json::json!(src.messages)).unwrap_or_default();
            let source_hash = sha1_hex(&source_messages_json);

            json!({
                "archive": src.archive,
                "path": src.path,
                "sha1": source_hash
            })
        })
        .collect();

    entries.insert(
        "text/messages.json".to_string(),
        json!({ "sources": sources_for_manifest }),
    );

    Ok(())
}

// Helpers

fn insert_direct_iso_entry(entries: &mut Map<String, Value>, rel_iso_path: &str, bytes: &[u8]) {
    entries.insert(
        rel_iso_path.to_string(),
        json!({
            "iso": rel_iso_path,
            "sha1": sha1_hex(bytes)
        }),
    );
}

/// Returns true for file extensions that have a dedicated decoder and should
/// not be registered as plain archive entries.
fn is_decoded_format(internal_path: &str) -> bool {
    internal_path.ends_with(".bmg")
    // Future: || internal_path.ends_with(".aw")
    // Future: || internal_path.ends_with(".dae")
}

/// Generates the friendly mod-relative path for a file from an archive.
fn get_friendly_path(
    archive_iso_path: &str,
    archive_stem: &str,
    internal_path: &str,
) -> Option<String> {
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

    if internal_path.ends_with(".aw") {
        let base = internal_path.strip_suffix(".aw").unwrap_or(internal_path);
        return Some(format!("audio/waves/{}/{}.json", archive_stem, base));
    }

    if internal_path.ends_with(".dae") {
        let base = internal_path.strip_suffix(".dae").unwrap_or(internal_path);
        return Some(format!("{}/{}/{}.dae", category, archive_stem, base));
    }

    if internal_path.ends_with(".bmd") {
        return Some(format!("{}/{}/{}", category, archive_stem, internal_path));
    }

    Some(format!("{}/{}/{}", category, archive_stem, internal_path))
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
