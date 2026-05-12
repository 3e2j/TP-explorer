use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::compression::gz2e;
use crate::formats::{iso, rarc};

const COPY_BUF_SIZE: usize = 1024 * 1024;
const ARC_EXTENSION: &str = "arc";

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

fn build_rarc_hash_map(archive: &rarc::Rarc) -> HashMap<String, String> {
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
        let mut hasher = Sha256::new();
        hasher.update(data);
        out.insert(full_path, hex::encode(hasher.finalize()));
    }
    out
}

fn build_archive_diff_detail(
    iso_path: &Path,
    folder_file_path: &Path,
    rel_path: &str,
) -> Result<String, String> {
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

    let iso_map = build_rarc_hash_map(&iso_archive);
    let folder_map = build_rarc_hash_map(&folder_archive);

    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut removed = Vec::new();

    for (path, folder_hash) in &folder_map {
        match iso_map.get(path) {
            Some(iso_hash) if iso_hash != folder_hash => changed.push(path.clone()),
            Some(_) => {}
            None => added.push(path.clone()),
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

    let mut output = String::new();
    output.push_str("  ");
    output.push_str(rel_path);
    output.push('\n');

    if added.is_empty() && changed.is_empty() && removed.is_empty() {
        output.push_str("    no internal changes detected\n");
        return Ok(output);
    }

    if !changed.is_empty() {
        output.push_str("    changed:\n");
        for path in changed {
            output.push_str("      ");
            output.push_str(&path);
            output.push('\n');
        }
    }
    if !added.is_empty() {
        output.push_str("    added:\n");
        for path in added {
            output.push_str("      ");
            output.push_str(&path);
            output.push('\n');
        }
    }
    if !removed.is_empty() {
        output.push_str("    removed:\n");
        for path in removed {
            output.push_str("      ");
            output.push_str(&path);
            output.push('\n');
        }
    }

    Ok(output)
}

fn diff_with_iso_path(iso_path: &Path, folder_path: &Path) -> Result<String, String> {
    let start = Instant::now();

    let iso_map_start = Instant::now();
    let iso_map = iso::build_iso_hash_map(iso_path)?;
    let iso_map_elapsed = iso_map_start.elapsed();

    let files_dir = find_files_dir(folder_path)?;

    let folder_map_start = Instant::now();
    let folder_map = build_folder_hash_map(&files_dir)?;
    let folder_map_elapsed = folder_map_start.elapsed();

    let mut added = Vec::new();
    let mut changed = Vec::new();

    for (path, folder_hash) in &folder_map {
        if let Some(iso_hash) = iso_map.get(path) {
            if iso_hash != folder_hash {
                changed.push(path.clone());
            }
        } else {
            added.push(path.clone());
        }
    }

    added.sort_unstable();
    changed.sort_unstable();

    let mut archive_details = Vec::new();
    for path in &changed {
        if !is_archive_path(path) {
            continue;
        }
        let folder_file_path = files_dir.join(path);
        let detail = match build_archive_diff_detail(iso_path, &folder_file_path, path) {
            Ok(detail) => detail,
            Err(err) => format!("  {path}\n    failed to diff internal archive contents: {err}\n"),
        };
        archive_details.push(detail);
    }

    if added.is_empty() && changed.is_empty() {
        let total = start.elapsed();
        return Ok(format!(
            "No changes detected.\n\n[Timing] ISO: {:.2}s, Folder: {:.2}s, Total: {:.2}s",
            iso_map_elapsed.as_secs_f64(),
            folder_map_elapsed.as_secs_f64(),
            total.as_secs_f64()
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
    if !archive_details.is_empty() {
        output.push_str("Archive internals:\n");
        for detail in archive_details {
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
