use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Instant;

use crate::compression::gz2e;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

const FST_OFFSET_OFFSET: u64 = 0x424;
const FST_SIZE_OFFSET: u64 = 0x428;
const FST_ENTRY_SIZE: usize = 0x0C;
const COPY_BUF_SIZE: usize = 1024 * 1024;

#[derive(Debug)]
struct FstEntry {
    is_dir: bool,
    name_offset: usize,
    data_offset_or_parent: u32,
    size_or_next_index: u32,
}

#[derive(Debug)]
struct IsoFileEntry {
    path: String,
    offset: u64,
    size: u64,
}

fn read_u32_be(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let slice = bytes.get(offset..end)?;
    Some(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u32_at(file: &mut File, offset: u64) -> Result<u32, String> {
    let mut buf = [0u8; 4];
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Failed to seek ISO: {e}"))?;
    file.read_exact(&mut buf)
        .map_err(|e| format!("Failed to read ISO header: {e}"))?;
    Ok(u32::from_be_bytes(buf))
}

fn parse_fst_entry(fst: &[u8], index: usize) -> Option<FstEntry> {
    let base = index.checked_mul(FST_ENTRY_SIZE)?;
    let flags_and_name = read_u32_be(fst, base)?;
    let data_offset_or_parent = read_u32_be(fst, base + 4)?;
    let size_or_next_index = read_u32_be(fst, base + 8)?;
    Some(FstEntry {
        is_dir: (flags_and_name & 0xFF00_0000) != 0,
        name_offset: (flags_and_name & 0x00FF_FFFF) as usize,
        data_offset_or_parent,
        size_or_next_index,
    })
}

fn read_name(fst: &[u8], names_offset: usize, name_offset: usize) -> Option<String> {
    let start = names_offset.checked_add(name_offset)?;
    if start >= fst.len() {
        return None;
    }
    let mut end = start;
    while end < fst.len() && fst[end] != 0 {
        end += 1;
    }
    Some(String::from_utf8_lossy(&fst[start..end]).to_string())
}

fn walk_directory(
    fst: &[u8],
    names_offset: usize,
    dir_index: usize,
    dir_path: &str,
    end_index: usize,
    out: &mut Vec<IsoFileEntry>,
) -> Result<(), String> {
    let mut i = dir_index + 1;
    while i < end_index {
        let entry = parse_fst_entry(fst, i).ok_or_else(|| "Invalid FST entry".to_string())?;
        let name = read_name(fst, names_offset, entry.name_offset)
            .ok_or_else(|| "Invalid FST name offset".to_string())?;

        if entry.is_dir {
            let child_end = entry.size_or_next_index as usize;
            if child_end <= i {
                return Err("Corrupt FST directory range".to_string());
            }
            let subdir_path = format!("{dir_path}/{name}");
            walk_directory(fst, names_offset, i, &subdir_path, child_end, out)?;
            i = child_end;
        } else {
            out.push(IsoFileEntry {
                path: format!("{dir_path}/{name}"),
                offset: entry.data_offset_or_parent as u64,
                size: entry.size_or_next_index as u64,
            });
            i += 1;
        }
    }
    Ok(())
}

pub fn parse_iso_files(iso_path: &Path) -> Result<Vec<IsoFileEntry>, String> {
    let mut file = File::open(iso_path).map_err(|e| format!("Failed to open ISO: {e}"))?;
    let fst_offset = read_u32_at(&mut file, FST_OFFSET_OFFSET)? as u64;
    let fst_size = read_u32_at(&mut file, FST_SIZE_OFFSET)? as usize;

    if fst_size < FST_ENTRY_SIZE {
        return Err("Invalid FST size".to_string());
    }

    let mut fst = vec![0u8; fst_size];
    file.seek(SeekFrom::Start(fst_offset))
        .map_err(|e| format!("Failed to seek FST: {e}"))?;
    file.read_exact(&mut fst)
        .map_err(|e| format!("Failed to read FST: {e}"))?;

    let root = parse_fst_entry(&fst, 0).ok_or_else(|| "Invalid root FST entry".to_string())?;
    if !root.is_dir {
        return Err("FST root is not a directory".to_string());
    }

    let num_entries = root.size_or_next_index as usize;
    let names_offset = num_entries
        .checked_mul(FST_ENTRY_SIZE)
        .ok_or_else(|| "FST size overflow".to_string())?;
    if names_offset > fst.len() {
        return Err("Invalid FST entry count".to_string());
    }

    let mut files = Vec::new();
    walk_directory(&fst, names_offset, 0, "files", num_entries, &mut files)?;
    Ok(files)
}

fn hash_file_region(file: &mut File, offset: u64, size: u64) -> Result<String, String> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Failed to seek ISO data: {e}"))?;
    let mut remaining = size;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; COPY_BUF_SIZE];

    while remaining > 0 {
        let read_len = remaining.min(COPY_BUF_SIZE as u64) as usize;
        file.read_exact(&mut buffer[..read_len])
            .map_err(|e| format!("Failed to read ISO data: {e}"))?;
        hasher.update(&buffer[..read_len]);
        remaining -= read_len as u64;
    }

    Ok(hex::encode(hasher.finalize()))
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

pub fn build_iso_hash_map(iso_path: &Path) -> Result<HashMap<String, String>, String> {
    let parse_start = Instant::now();
    let files = parse_iso_files(iso_path)?;
    let parse_elapsed = parse_start.elapsed();
    eprintln!(
        "  [Profile] FST parsing: {:.2}s ({} files)",
        parse_elapsed.as_secs_f64(),
        files.len()
    );

    let mut iso_file = File::open(iso_path).map_err(|e| format!("Failed to open ISO: {e}"))?;
    let mut out = HashMap::with_capacity(files.len());

    let hash_start = Instant::now();
    for (i, file) in files.iter().enumerate() {
        let rel = file
            .path
            .strip_prefix("files/")
            .ok_or_else(|| "Unexpected ISO file root".to_string())?
            .to_string();
        let hash = hash_file_region(&mut iso_file, file.offset, file.size)?;
        out.insert(rel, hash);

        if (i + 1) % 500 == 0 {
            let elapsed = hash_start.elapsed();
            let avg_ms = elapsed.as_secs_f64() * 1000.0 / (i + 1) as f64;
            eprintln!(
                "  [Profile] Hashed {}/{} files ({:.2}ms/file)",
                i + 1,
                files.len(),
                avg_ms
            );
        }
    }
    let hash_elapsed = hash_start.elapsed();
    eprintln!(
        "  [Profile] ISO hashing: {:.2}s",
        hash_elapsed.as_secs_f64()
    );

    Ok(out)
}

pub fn build_folder_hash_map(folder: &Path) -> Result<HashMap<String, String>, String> {
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

    // Check if ISO is compressed (GZ2E format) and decompress if needed
    let mut file = std::fs::File::open(iso_path).map_err(|e| format!("Failed to open ISO: {e}"))?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)
        .map_err(|e| format!("Failed to read ISO header: {e}"))?;

    let iso_to_use = if &magic == b"GZ2E" {
        // Decompress GZ2E to a temporary file
        let temp_path = std::env::temp_dir().join(format!("iso_diff_{}.iso", std::process::id()));

        file.seek(SeekFrom::Start(0))
            .map_err(|e| format!("Failed to seek ISO: {e}"))?;

        let decompressed_data = gz2e::decompress_gz2e(&mut file)
            .map_err(|e| format!("GZ2E decompression failed: {e}"))?;

        let mut temp_file = std::fs::File::create(&temp_path)
            .map_err(|e| format!("Failed to create temp file: {e}"))?;
        temp_file
            .write_all(&decompressed_data)
            .map_err(|e| format!("Failed to write decompressed ISO: {e}"))?;
        temp_file
            .flush()
            .map_err(|e| format!("Failed to flush temp file: {e}"))?;

        temp_path
    } else {
        iso_path.to_path_buf()
    };

    // Perform the diff
    let result = diff_with_iso_path(&iso_to_use, folder_path);

    // Clean up temp file if we created one
    if iso_to_use != iso_path {
        let _ = std::fs::remove_file(&iso_to_use);
    }

    result
}

fn find_common_root<'a>(base: &'a Path, target_component: &str) -> &'a Path {
    for ancestor in base.ancestors() {
        if ancestor
            .file_name()
            .map_or(false, |n| n == target_component)
        {
            return ancestor;
        }
    }
    base
}

fn diff_with_iso_path(iso_path: &Path, folder_path: &Path) -> Result<String, String> {
    let start = Instant::now();

    let iso_map_start = Instant::now();
    let iso_map = build_iso_hash_map(iso_path)?;
    let iso_map_elapsed = iso_map_start.elapsed();

    // Find the "files" dir: if the given path ends in "files", use it directly.
    // If it contains a "files" subdir, use that. Otherwise error clearly.
    let files_dir = if folder_path.file_name().map_or(false, |n| n == "files") {
        folder_path.to_path_buf()
    } else {
        let candidate = folder_path.join("files");
        if candidate.is_dir() {
            candidate
        } else {
            return Err(format!(
                "Could not find a 'files' directory at or inside: {}",
                folder_path.display()
            ));
        }
    };

    let folder_map_start = Instant::now();
    let folder_map = build_folder_hash_map(&files_dir)?;
    let folder_map_elapsed = folder_map_start.elapsed();

    let mut added = Vec::new();
    let mut changed = Vec::new();

    // Only iterate over files in the comparison folder
    for (path, folder_hash) in &folder_map {
        if let Some(iso_hash) = iso_map.get(path) {
            // File exists in both - check if it changed
            if iso_hash != folder_hash {
                eprintln!("  [MISMATCH] {}", path);
                eprintln!("    ISO:    {}", iso_hash);
                eprintln!("    Folder: {}", folder_hash);
                changed.push(path.clone());
            }
        } else {
            // File exists only in folder - it's been added
            added.push(path.clone());
        }
    }

    added.sort_unstable();
    changed.sort_unstable();

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

    let total = start.elapsed();
    output.push_str(&format!(
        "\n[Timing] ISO: {:.2}s, Folder: {:.2}s, Total: {:.2}s",
        iso_map_elapsed.as_secs_f64(),
        folder_map_elapsed.as_secs_f64(),
        total.as_secs_f64()
    ));

    Ok(output)
}
