use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::compression::gz2e;
use crate::formats::{aw, baa, iso, rarc};

const COPY_BUF_SIZE: usize = 1024 * 1024;
const ARC_EXTENSION: &str = "arc";
const AW_EXTENSION: &str = "aw";
const BAA_EXTENSION: &str = "baa";
const OUTPUT_DIR: &str = "output";

#[derive(Debug)]
struct ArcDiffResult {
    detail: String,
    semantic_changed: bool,
    changed_or_added: Vec<(String, Vec<u8>)>,
    removed: Vec<String>,
}

#[derive(Debug)]
struct AwWaveExport {
    index: usize,
    kind: &'static str,
    aw_name: String,
    iso_offset: Option<u32>,
    iso_size: Option<u32>,
    folder_offset: Option<u32>,
    folder_size: Option<u32>,
    iso_data: Option<Vec<u8>>,
    folder_data: Option<Vec<u8>>,
    iso_wave_info: Option<baa::AwWaveInfo>,
    folder_wave_info: Option<baa::AwWaveInfo>,
}

#[derive(Debug)]
struct AwDiffResult {
    detail: String,
    semantic_changed: bool,
    exports: Vec<AwWaveExport>,
}

fn hash_disk_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|e| format!("Failed to open file {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; COPY_BUF_SIZE];

    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read file {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn build_folder_hash_map(folder: &Path) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();

    let walk_start = Instant::now();
    let mut file_count = 0;
    for entry in WalkDir::new(folder)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        file_count += 1;
    }
    let walk_elapsed = walk_start.elapsed();
    eprintln!(
        "  [Profile] Directory walk: {:.2}s ({} files)",
        walk_elapsed.as_secs_f64(),
        file_count
    );

    let hash_start = Instant::now();
    let mut hashed = 0;
    for entry in WalkDir::new(folder)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(folder)
            .map_err(|e| format!("Failed to resolve relative path: {e}"))?
            .to_string_lossy()
            .replace('\\', "/");
        let hash = hash_disk_file(entry.path())?;
        out.insert(rel, hash);
        hashed += 1;

        if hashed % 100 == 0 {
            let elapsed = hash_start.elapsed();
            let avg_ms = elapsed.as_secs_f64() * 1000.0 / hashed as f64;
            eprintln!(
                "  [Profile] Hashed {}/{} folder files ({:.2}ms/file)",
                hashed, file_count, avg_ms
            );
        }
    }
    let hash_elapsed = hash_start.elapsed();
    eprintln!(
        "  [Profile] Folder hashing: {:.2}s",
        hash_elapsed.as_secs_f64()
    );
    Ok(out)
}

fn prepare_iso_path(iso_path: &Path) -> Result<(PathBuf, bool), String> {
    let mut file = File::open(iso_path).map_err(|e| format!("Failed to open ISO: {e}"))?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)
        .map_err(|e| format!("Failed to read ISO header: {e}"))?;

    if &magic != b"GZ2E" {
        return Ok((iso_path.to_path_buf(), false));
    }

    let temp_path = std::env::temp_dir().join(format!("iso_diff_{}.iso", std::process::id()));
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("Failed to seek ISO: {e}"))?;

    let decompressed_data =
        gz2e::decompress_gz2e(&mut file).map_err(|e| format!("GZ2E decompression failed: {e}"))?;

    let mut temp_file =
        File::create(&temp_path).map_err(|e| format!("Failed to create temp file: {e}"))?;
    temp_file
        .write_all(&decompressed_data)
        .map_err(|e| format!("Failed to write decompressed ISO: {e}"))?;
    temp_file
        .flush()
        .map_err(|e| format!("Failed to flush temp file: {e}"))?;

    Ok((temp_path, true))
}

fn find_files_dir(folder_path: &Path) -> Result<PathBuf, String> {
    if folder_path.file_name().is_some_and(|n| n == "files") {
        return Ok(folder_path.to_path_buf());
    }

    let candidate = folder_path.join("files");
    if candidate.is_dir() {
        Ok(candidate)
    } else {
        Err(format!(
            "Could not find a 'files' directory at or inside: {}",
            folder_path.display()
        ))
    }
}

fn is_archive_path(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case(ARC_EXTENSION))
}

fn is_aw_path(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case(AW_EXTENSION))
}

fn is_baa_path(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case(BAA_EXTENSION))
}

fn common_prefix_components_len(a: &str, b: &str) -> usize {
    let mut count = 0usize;
    for (ac, bc) in Path::new(a).components().zip(Path::new(b).components()) {
        if ac == bc {
            count += 1;
        } else {
            break;
        }
    }
    count
}

fn resolve_baa_for_aw(aw_rel_path: &str, baa_candidates: &[String]) -> Option<String> {
    baa_candidates
        .iter()
        .max_by_key(|baa_path| common_prefix_components_len(aw_rel_path, baa_path))
        .cloned()
}

fn build_rarc_file_map(archive: &rarc::Rarc) -> HashMap<String, Vec<u8>> {
    let mut out = HashMap::new();
    for entry in &archive.file_entries {
        if entry.is_dir {
            continue;
        }
        let Some(parent_idx) = entry.parent_node_index else {
            continue;
        };
        let Some(data) = entry.data.as_ref() else {
            continue;
        };
        let node_path = archive.node_path(parent_idx);
        let full_path = if node_path.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", node_path, entry.name)
        };
        out.insert(full_path, data.clone());
    }
    out
}

fn build_archive_diff_result(
    iso_path: &Path,
    folder_file_path: &Path,
    rel_path: &str,
) -> Result<ArcDiffResult, String> {
    let iso_bytes = iso::read_iso_file_bytes(iso_path, rel_path)?;
    let folder_bytes = std::fs::read(folder_file_path).map_err(|e| {
        format!(
            "Failed to read folder archive {}: {e}",
            folder_file_path.display()
        )
    })?;

    let iso_archive = rarc::Rarc::parse(iso_bytes)
        .ok_or_else(|| format!("Failed to parse ISO archive: {rel_path}"))?;
    let folder_archive = rarc::Rarc::parse(folder_bytes)
        .ok_or_else(|| format!("Failed to parse folder archive: {rel_path}"))?;

    let iso_map = build_rarc_file_map(&iso_archive);
    let folder_map = build_rarc_file_map(&folder_archive);

    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    let mut changed_or_added_data = Vec::new();

    for (path, folder_data) in &folder_map {
        match iso_map.get(path) {
            Some(iso_data) if hash_bytes(iso_data) != hash_bytes(folder_data) => {
                changed.push(path.clone());
                changed_or_added_data.push((path.clone(), folder_data.clone()));
            }
            Some(_) => {}
            None => {
                added.push(path.clone());
                changed_or_added_data.push((path.clone(), folder_data.clone()));
            }
        }
    }

    for path in iso_map.keys() {
        if !folder_map.contains_key(path) {
            removed.push(path.clone());
        }
    }

    added.sort_unstable();
    changed.sort_unstable();
    removed.sort_unstable();
    changed_or_added_data.sort_by(|a, b| a.0.cmp(&b.0));

    let mut output = String::new();
    output.push_str("  ");
    output.push_str(rel_path);
    output.push('\n');

    if added.is_empty() && changed.is_empty() && removed.is_empty() {
        output.push_str("    no internal changes detected\n");
        return Ok(ArcDiffResult {
            detail: output,
            semantic_changed: false,
            changed_or_added: Vec::new(),
            removed,
        });
    }

    if !changed.is_empty() {
        output.push_str("    changed:\n");
        for path in &changed {
            output.push_str("      ");
            output.push_str(path);
            output.push('\n');
        }
    }
    if !added.is_empty() {
        output.push_str("    added:\n");
        for path in &added {
            output.push_str("      ");
            output.push_str(path);
            output.push('\n');
        }
    }
    if !removed.is_empty() {
        output.push_str("    removed:\n");
        for path in &removed {
            output.push_str("      ");
            output.push_str(path);
            output.push('\n');
        }
    }

    Ok(ArcDiffResult {
        detail: output,
        semantic_changed: true,
        changed_or_added: changed_or_added_data,
        removed,
    })
}

fn build_aw_diff_result(
    iso_path: &Path,
    files_dir: &Path,
    folder_file_path: &Path,
    rel_path: &str,
    baa_candidates: &[String],
) -> Result<AwDiffResult, String> {
    let baa_rel_path = resolve_baa_for_aw(rel_path, baa_candidates)
        .ok_or_else(|| format!("No companion BAA found for AW file: {rel_path}"))?;

    let iso_baa_bytes = iso::read_iso_file_bytes(iso_path, &baa_rel_path)?;
    let folder_baa_path = files_dir.join(&baa_rel_path);
    let folder_baa_bytes = std::fs::read(&folder_baa_path).map_err(|e| {
        format!(
            "Failed to read folder BAA {}: {e}",
            folder_baa_path.display()
        )
    })?;
    let iso_baa = baa::BaaArchive::parse(&iso_baa_bytes)?;
    let folder_baa = baa::BaaArchive::parse(&folder_baa_bytes)?;

    let aw_name = Path::new(rel_path)
        .file_name()
        .ok_or_else(|| format!("Invalid AW path: {rel_path}"))?
        .to_string_lossy()
        .to_string();

    let iso_aw_info = iso_baa.find_aw_by_name(&aw_name)?;
    let folder_aw_info = folder_baa.find_aw_by_name(&aw_name)?;

    let iso_bytes = iso::read_iso_file_bytes(iso_path, rel_path)?;
    let folder_bytes = std::fs::read(folder_file_path).map_err(|e| {
        format!(
            "Failed to read folder AW {}: {e}",
            folder_file_path.display()
        )
    })?;

    let diff = aw::diff_aw_entries(&iso_bytes, &folder_bytes, iso_aw_info, folder_aw_info)?;
    let detail =
        aw::format_aw_entry_diff(rel_path, &baa_rel_path, iso_aw_info, folder_aw_info, &diff);

    let mut exports = Vec::new();
    for entry in &diff.changed_entries {
        let iso_wave = iso_aw_info.waves.get(entry.index);
        let folder_wave = folder_aw_info.waves.get(entry.index);
        let iso_data = match iso_wave {
            Some(wave) => Some(aw::extract_wave_bytes(
                &iso_bytes,
                wave.stream_offset,
                wave.stream_size,
            )?),
            None => None,
        };
        let folder_data = match folder_wave {
            Some(wave) => Some(aw::extract_wave_bytes(
                &folder_bytes,
                wave.stream_offset,
                wave.stream_size,
            )?),
            None => None,
        };
        exports.push(AwWaveExport {
            index: entry.index,
            kind: match entry.kind {
                aw::WaveDiffKind::Changed => "changed",
                aw::WaveDiffKind::Added => "added",
                aw::WaveDiffKind::Removed => "removed",
            },
            aw_name: aw_name.clone(),
            iso_offset: iso_wave.map(|w| w.stream_offset),
            iso_size: iso_wave.map(|w| w.stream_size),
            folder_offset: folder_wave.map(|w| w.stream_offset),
            folder_size: folder_wave.map(|w| w.stream_size),
            iso_data,
            folder_data,
            iso_wave_info: iso_wave.cloned(),
            folder_wave_info: folder_wave.cloned(),
        });
    }

    Ok(AwDiffResult {
        detail,
        semantic_changed: !diff.changed_entries.is_empty(),
        exports,
    })
}

fn ensure_clean_output_dir(output_root: &Path) -> Result<(), String> {
    if output_root.exists() {
        fs::remove_dir_all(output_root).map_err(|e| {
            format!(
                "Failed to clear output directory {}: {e}",
                output_root.display()
            )
        })?;
    }
    fs::create_dir_all(output_root).map_err(|e| {
        format!(
            "Failed to create output directory {}: {e}",
            output_root.display()
        )
    })
}

fn write_bytes(path: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {e}", parent.display()))?;
    }
    fs::write(path, data).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

fn copy_raw_files(files_dir: &Path, output_root: &Path, paths: &[String]) -> Result<(), String> {
    for rel_path in paths {
        let src = files_dir.join(rel_path);
        let dst = output_root.join("raw").join(rel_path);
        let data = fs::read(&src).map_err(|e| format!("Failed to read {}: {e}", src.display()))?;
        write_bytes(&dst, &data)?;
    }
    Ok(())
}

fn export_arc_changes(
    output_root: &Path,
    arc_rel_path: &str,
    changed_or_added: &[(String, Vec<u8>)],
    removed: &[String],
) -> Result<(), String> {
    let base = output_root.join("arc").join(arc_rel_path);
    for (internal_path, data) in changed_or_added {
        write_bytes(&base.join("changed").join(internal_path), data)?;
    }
    if !removed.is_empty() {
        let mut removed_txt = String::new();
        for path in removed {
            removed_txt.push_str(path);
            removed_txt.push('\n');
        }
        write_bytes(&base.join("removed.txt"), removed_txt.as_bytes())?;
    }
    Ok(())
}

fn export_aw_changes(
    output_root: &Path,
    aw_rel_path: &str,
    exports: &[AwWaveExport],
) -> Result<(), String> {
    let original_base = output_root.join("audio").join("original").join(aw_rel_path);
    let modded_base = output_root.join("audio").join("modded").join(aw_rel_path);
    let info_base = output_root.join("audio").join("meta").join(aw_rel_path);
    let mut manifest = String::new();
    for entry in exports {
        let stem = Path::new(&entry.aw_name)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| entry.aw_name.clone());
        let file_name = format!("{}_wave_{:04}_{}.wav", stem, entry.index, entry.kind);
        if let (Some(data), Some(wave)) = (&entry.iso_data, &entry.iso_wave_info) {
            let wav = aw::wave_data_to_wav_bytes(data, wave)?;
            write_bytes(&original_base.join(&file_name), &wav)?;
        }
        if let (Some(data), Some(wave)) = (&entry.folder_data, &entry.folder_wave_info) {
            let wav = aw::wave_data_to_wav_bytes(data, wave)?;
            write_bytes(&modded_base.join(&file_name), &wav)?;
        }
        manifest.push_str(&format!(
            "{} iso(off={},size={},fmt={},sr_bits=0x{:08X},loop=[{},{}]) folder(off={},size={},fmt={},sr_bits=0x{:08X},loop=[{},{}])\n",
            file_name,
            entry
                .iso_offset
                .map(|v| format!("0x{v:X}"))
                .unwrap_or_else(|| "-".to_string()),
            entry
                .iso_size
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string()),
            entry
                .iso_wave_info
                .as_ref()
                .map(|w| w.format.to_string())
                .unwrap_or_else(|| "-".to_string()),
            entry
                .iso_wave_info
                .as_ref()
                .map(|w| w.sample_rate_bits)
                .unwrap_or(0),
            entry
                .iso_wave_info
                .as_ref()
                .map(|w| w.loop_start.to_string())
                .unwrap_or_else(|| "-".to_string()),
            entry
                .iso_wave_info
                .as_ref()
                .map(|w| w.loop_end.to_string())
                .unwrap_or_else(|| "-".to_string()),
            entry
                .folder_offset
                .map(|v| format!("0x{v:X}"))
                .unwrap_or_else(|| "-".to_string()),
            entry
                .folder_size
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string()),
            entry
                .folder_wave_info
                .as_ref()
                .map(|w| w.format.to_string())
                .unwrap_or_else(|| "-".to_string()),
            entry
                .folder_wave_info
                .as_ref()
                .map(|w| w.sample_rate_bits)
                .unwrap_or(0),
            entry
                .folder_wave_info
                .as_ref()
                .map(|w| w.loop_start.to_string())
                .unwrap_or_else(|| "-".to_string()),
            entry
                .folder_wave_info
                .as_ref()
                .map(|w| w.loop_end.to_string())
                .unwrap_or_else(|| "-".to_string()),
        ));
    }
    write_bytes(&info_base.join("manifest.txt"), manifest.as_bytes())
}

fn diff_with_iso_path(iso_path: &Path, folder_path: &Path) -> Result<String, String> {
    let start = Instant::now();
    let output_root = PathBuf::from(OUTPUT_DIR);

    let iso_map_start = Instant::now();
    let iso_map = iso::build_iso_hash_map(iso_path)?;
    let iso_map_elapsed = iso_map_start.elapsed();

    let files_dir = find_files_dir(folder_path)?;

    let folder_map_start = Instant::now();
    let folder_map = build_folder_hash_map(&files_dir)?;
    let folder_map_elapsed = folder_map_start.elapsed();

    let mut added = Vec::new();
    let mut raw_changed = Vec::new();

    for (path, folder_hash) in &folder_map {
        if let Some(iso_hash) = iso_map.get(path) {
            if iso_hash != folder_hash {
                raw_changed.push(path.clone());
            }
        } else {
            added.push(path.clone());
        }
    }

    added.sort_unstable();
    raw_changed.sort_unstable();

    let baa_candidates: Vec<String> = folder_map
        .keys()
        .chain(iso_map.keys())
        .filter(|path| is_baa_path(path))
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let mut changed = Vec::new();
    let mut ignored_semantic = Vec::new();
    let mut archive_details = Vec::new();
    let mut aw_details = Vec::new();
    let mut structured_handled = HashSet::new();

    let mut arc_exports: Vec<(String, Vec<(String, Vec<u8>)>, Vec<String>)> = Vec::new();
    let mut aw_exports: Vec<(String, Vec<AwWaveExport>)> = Vec::new();

    for path in &raw_changed {
        let folder_file_path = files_dir.join(path);
        if is_archive_path(path) {
            match build_archive_diff_result(iso_path, &folder_file_path, path) {
                Ok(result) => {
                    if result.semantic_changed {
                        changed.push(path.clone());
                        archive_details.push(result.detail);
                        arc_exports.push((path.clone(), result.changed_or_added, result.removed));
                        structured_handled.insert(path.clone());
                    } else {
                        ignored_semantic.push(path.clone());
                    }
                }
                Err(err) => {
                    changed.push(path.clone());
                    archive_details.push(format!(
                        "  {path}\n    failed to diff internal archive contents: {err}\n"
                    ));
                }
            }
        } else if is_aw_path(path) {
            match build_aw_diff_result(
                iso_path,
                &files_dir,
                &folder_file_path,
                path,
                &baa_candidates,
            ) {
                Ok(result) => {
                    if result.semantic_changed {
                        changed.push(path.clone());
                        aw_details.push(result.detail);
                        aw_exports.push((path.clone(), result.exports));
                        structured_handled.insert(path.clone());
                    } else {
                        ignored_semantic.push(path.clone());
                    }
                }
                Err(err) => {
                    changed.push(path.clone());
                    aw_details.push(format!(
                        "  {path}\n    failed to diff AW internal data: {err}\n"
                    ));
                }
            }
        } else {
            changed.push(path.clone());
        }
    }

    changed.sort_unstable();
    ignored_semantic.sort_unstable();

    let mut raw_export_paths: Vec<String> = added
        .iter()
        .filter(|path| !is_baa_path(path))
        .cloned()
        .collect();
    for path in &changed {
        if !structured_handled.contains(path) && !is_baa_path(path) {
            raw_export_paths.push(path.clone());
        }
    }
    raw_export_paths.sort_unstable();
    raw_export_paths.dedup();

    ensure_clean_output_dir(&output_root)?;
    copy_raw_files(&files_dir, &output_root, &raw_export_paths)?;
    for (arc_rel, changed_or_added, removed) in &arc_exports {
        export_arc_changes(&output_root, arc_rel, changed_or_added, removed)?;
    }
    for (aw_rel, exports) in &aw_exports {
        export_aw_changes(&output_root, aw_rel, exports)?;
    }

    if added.is_empty() && changed.is_empty() {
        let total = start.elapsed();
        return Ok(format!(
            "No changes detected.\n\n[Timing] ISO: {:.2}s, Folder: {:.2}s, Total: {:.2}s\n[Output] {}",
            iso_map_elapsed.as_secs_f64(),
            folder_map_elapsed.as_secs_f64(),
            total.as_secs_f64(),
            output_root.display()
        ));
    }

    let mut output = String::new();
    if !changed.is_empty() {
        output.push_str("Changed:\n");
        for path in &changed {
            output.push_str("  ");
            output.push_str(path);
            output.push('\n');
        }
    }
    if !added.is_empty() {
        output.push_str("Added:\n");
        for path in &added {
            output.push_str("  ");
            output.push_str(path);
            output.push('\n');
        }
    }
    if !ignored_semantic.is_empty() {
        output.push_str("Ignored (raw-only differences):\n");
        for path in &ignored_semantic {
            output.push_str("  ");
            output.push_str(path);
            output.push('\n');
        }
    }
    if !archive_details.is_empty() {
        output.push_str("Archive internals:\n");
        for detail in archive_details {
            output.push_str(&detail);
        }
    }
    if !aw_details.is_empty() {
        output.push_str("AW internals:\n");
        for detail in aw_details {
            output.push_str(&detail);
        }
    }

    let total = start.elapsed();
    output.push_str(&format!(
        "\n[Timing] ISO: {:.2}s, Folder: {:.2}s, Total: {:.2}s",
        iso_map_elapsed.as_secs_f64(),
        folder_map_elapsed.as_secs_f64(),
        total.as_secs_f64()
    ));
    output.push_str(&format!("\n[Output] {}", output_root.display()));

    Ok(output)
}

pub fn diff_iso_files_against_folder(
    iso_path: &Path,
    folder_path: &Path,
) -> Result<String, String> {
    if !iso_path.is_file() {
        return Err(format!("ISO path is not a file: {}", iso_path.display()));
    }
    if !folder_path.is_dir() {
        return Err(format!(
            "Comparison path is not a directory: {}",
            folder_path.display()
        ));
    }

    let (iso_to_use, is_temp_file) = prepare_iso_path(iso_path)?;
    let result = diff_with_iso_path(&iso_to_use, folder_path);

    if is_temp_file {
        if let Err(e) = std::fs::remove_file(&iso_to_use) {
            eprintln!(
                "Warning: failed to remove temporary ISO {}: {}",
                iso_to_use.display(),
                e
            );
        }
    }

    result
}
