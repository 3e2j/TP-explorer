//! Diffing helpers for comparing ISO contents against a mod folder.
//!
//! The public entry point compares file hashes and reports added or changed
//! files. Internal helpers handle ISO wrapper decoding, folder hashing, and
//! path discovery.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Instant;

use walkdir::WalkDir;

use crate::formats::compression::gz2e;
use crate::formats::iso::iso_read;

const COPY_BUF_SIZE: usize = 1024 * 1024;
const OUTPUT_DIR: &str = "output";

fn hash_disk_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|e| format!("Failed to open file {}: {e}", path.display()))?;
    let mut hasher = sha1::Sha1::new();
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

    Ok(hasher.digest().to_string())
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

/// Decompresses ISO to a temporary file if GZ2E encrypted
/// Returns `(iso_path, temp_file)`
///
/// If temp file is returned, it must be cleaned up outside of this function
fn prepare_iso_path(iso_path: &Path) -> Result<(PathBuf, bool), String> {
    let mut iso_file = File::open(iso_path).map_err(|e| format!("Failed to open ISO: {e}"))?;
    let mut magic = [0u8; 4];
    iso_file
        .read_exact(&mut magic)
        .map_err(|e| format!("Failed to read ISO header: {e}"))?;

    if &magic != b"GZ2E" {
        return Ok((iso_path.to_path_buf(), false));
    }

    let temp_path = std::env::temp_dir().join(format!("iso_diff_{}.iso", std::process::id()));
    iso_file
        .seek(SeekFrom::Start(0))
        .map_err(|e| format!("Failed to seek ISO: {e}"))?;

    let mut temp_file_decomp =
        File::create(&temp_path).map_err(|e| format!("Failed to create temp file: {e}"))?;

    gz2e::decompress_gz2e(&mut iso_file, &mut temp_file_decomp)
        .map_err(|e| format!("GZ2E decompression failed: {e}"))?;

    Ok((temp_path, true))
}

/// Walk along path until the `files` directory is found
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

/// Gets the difference between iso paths (via hash)
fn diff_with_iso_path(iso_path: &Path, files_dir: &Path) -> Result<String, String> {
    let start = Instant::now();
    let output_root = PathBuf::from(OUTPUT_DIR);

    let iso_map_start = Instant::now();
    let iso_map = iso_read::build_iso_hash_map(iso_path)?;
    let iso_map_elapsed = iso_map_start.elapsed();

    let folder_map_start = Instant::now();
    let folder_map = build_folder_hash_map(&files_dir)?;
    let folder_map_elapsed = folder_map_start.elapsed();

    let mut added = Vec::new();
    let mut changed = Vec::new();

    // Map the differences
    for (path, folder_hash) in &folder_map {
        match iso_map.get(path) {
            Some(iso_hash) if iso_hash != folder_hash => changed.push(path.clone()),
            None => added.push(path.clone()),
            _ => {}
        }
    }

    added.sort_unstable();
    changed.sort_unstable();

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

/// Main entry for convering entries ISO against folder
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
    let files_dir = find_files_dir(folder_path)?;

    let result = diff_with_iso_path(&iso_to_use, &files_dir);

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
