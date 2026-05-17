use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Instant;

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

#[derive(Debug, Clone)]
pub struct IsoFileEntry {
    pub path: String,
    pub offset: u64,
    pub size: u64,
    pub fst_index: usize, // FST entry index for direct FST updates
}

pub fn read_u32_at(file: &mut File, offset: u64) -> Result<u32, String> {
    let mut buf = [0u8; 4];
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Failed to seek ISO: {e}"))?;
    file.read_exact(&mut buf)
        .map_err(|e| format!("Failed to read ISO header: {e}"))?;
    Ok(u32::from_be_bytes(buf))
}

fn parse_fst_entry(fst: &[u8], index: usize) -> Option<FstEntry> {
    let base = index.checked_mul(FST_ENTRY_SIZE)?;
    let flags_and_name = u32::from_be_bytes(fst.get(base..base + 4)?.try_into().ok()?);
    let data_offset_or_parent = u32::from_be_bytes(fst.get(base + 4..base + 8)?.try_into().ok()?);
    let size_or_next_index = u32::from_be_bytes(fst.get(base + 8..base + 12)?.try_into().ok()?);
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
                fst_index: i, // Track the FST entry index
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
    let mut hasher = sha1::Sha1::new();
    let mut buffer = vec![0u8; COPY_BUF_SIZE];

    while remaining > 0 {
        let read_len = remaining.min(COPY_BUF_SIZE as u64) as usize;
        file.read_exact(&mut buffer[..read_len])
            .map_err(|e| format!("Failed to read ISO data: {e}"))?;
        hasher.update(&buffer[..read_len]);
        remaining -= read_len as u64;
    }

    Ok(hasher.digest().to_string())
}

fn read_file_region(file: &mut File, offset: u64, size: u64) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Failed to seek ISO data: {e}"))?;
    let mut out = vec![0u8; size as usize];
    file.read_exact(&mut out)
        .map_err(|e| format!("Failed to read ISO data: {e}"))?;
    Ok(out)
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

pub fn read_iso_file_bytes(iso_path: &Path, relative_path: &str) -> Result<Vec<u8>, String> {
    let files = parse_iso_files(iso_path)?;
    let target_path = format!("files/{relative_path}");
    let file_entry = files
        .iter()
        .find(|entry| entry.path == target_path)
        .ok_or_else(|| format!("File not found in ISO: {relative_path}"))?;

    let mut iso_file = File::open(iso_path).map_err(|e| format!("Failed to open ISO: {e}"))?;
    read_file_region(&mut iso_file, file_entry.offset, file_entry.size)
}
