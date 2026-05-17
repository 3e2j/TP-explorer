//! ISO rebuilding with proper FST updates (based on GCFT approach)

use crate::formats::iso::iso::{read_u32_at, IsoFileEntry};
use crate::utils::write_u32_be_at;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

const FST_ENTRY_SIZE: usize = 12;

fn pad_to_alignment(file: &mut File, alignment: u64) -> Result<(), String> {
    let current = file
        .stream_position()
        .map_err(|e| format!("Get position failed: {e}"))?;
    let padding = (alignment - (current % alignment)) % alignment;
    if padding > 0 {
        file.write_all(&vec![0u8; padding as usize])
            .map_err(|e| format!("Write padding failed: {e}"))?;
    }
    Ok(())
}

pub fn rebuild_iso_with_files(
    source_iso: &Path,
    output_iso: &Path,
    replacements: &HashMap<String, Vec<u8>>,
    all_files: &[IsoFileEntry],
) -> Result<(), String> {
    let mut source = File::open(source_iso).map_err(|e| format!("Open source failed: {e}"))?;
    let mut output = File::create(output_iso).map_err(|e| format!("Create output failed: {e}"))?;

    // Read FST from source
    let fst_offset = read_u32_at(&mut source, 0x424)? as u64;
    let fst_size = read_u32_at(&mut source, 0x428)? as usize;

    let mut original_fst = vec![0u8; fst_size];
    source
        .seek(SeekFrom::Start(fst_offset))
        .map_err(|e| format!("Seek FST failed: {e}"))?;
    source
        .read_exact(&mut original_fst)
        .map_err(|e| format!("Read FST failed: {e}"))?;

    // Copy system area (boot, bi2)
    let mut sys_buf = vec![0u8; 0x2440];
    source
        .seek(SeekFrom::Start(0))
        .map_err(|e| format!("Seek system area failed: {e}"))?;
    source
        .read_exact(&mut sys_buf)
        .map_err(|e| format!("Read system area failed: {e}"))?;
    output
        .write_all(&sys_buf)
        .map_err(|e| format!("Write system area failed: {e}"))?;

    // Handle apploader
    let apploader_size = read_u32_at(&mut source, 0x2440 + 0x14)? as u64;
    let apploader_trailer = read_u32_at(&mut source, 0x2440 + 0x18)? as u64;
    let apploader_full = 0x20 + apploader_size + apploader_trailer;

    source
        .seek(SeekFrom::Start(0x2440))
        .map_err(|e| format!("Seek apploader failed: {e}"))?;
    let mut apploader_buf = vec![0u8; apploader_full as usize];
    source
        .read_exact(&mut apploader_buf)
        .map_err(|e| format!("Read apploader failed: {e}"))?;

    output
        .seek(SeekFrom::Start(0x2440))
        .map_err(|e| format!("Seek output apploader failed: {e}"))?;
    output
        .write_all(&apploader_buf)
        .map_err(|e| format!("Write apploader failed: {e}"))?;

    // Handle DOL
    let dol_offset_orig = read_u32_at(&mut source, 0x420)? as u64;

    // Calculate DOL size
    let mut dol_size = 0u64;
    let mut section_buf = [0u8; 4];
    for i in 0..7 {
        source
            .seek(SeekFrom::Start(dol_offset_orig + 0x90 + i * 4))
            .map_err(|e| format!("Seek DOL size failed: {e}"))?;
        source
            .read_exact(&mut section_buf)
            .map_err(|e| format!("Read DOL size failed: {e}"))?;
        let sz = u32::from_be_bytes(section_buf) as u64;
        source
            .seek(SeekFrom::Start(dol_offset_orig + i * 4))
            .map_err(|e| format!("Seek DOL offset failed: {e}"))?;
        source
            .read_exact(&mut section_buf)
            .map_err(|e| format!("Read DOL offset failed: {e}"))?;
        let off = u32::from_be_bytes(section_buf) as u64;
        dol_size = dol_size.max(off + sz);
    }
    for i in 0..11 {
        source
            .seek(SeekFrom::Start(dol_offset_orig + 0xAC + i * 4))
            .map_err(|e| format!("Seek DOL data size failed: {e}"))?;
        source
            .read_exact(&mut section_buf)
            .map_err(|e| format!("Read DOL data size failed: {e}"))?;
        let sz = u32::from_be_bytes(section_buf) as u64;
        source
            .seek(SeekFrom::Start(dol_offset_orig + 0x1C + i * 4))
            .map_err(|e| format!("Seek DOL data offset failed: {e}"))?;
        source
            .read_exact(&mut section_buf)
            .map_err(|e| format!("Read DOL data offset failed: {e}"))?;
        let off = u32::from_be_bytes(section_buf) as u64;
        dol_size = dol_size.max(off + sz);
    }

    // Position and write DOL
    output
        .seek(SeekFrom::Start(0x2440 + apploader_full))
        .map_err(|e| format!("Seek after apploader failed: {e}"))?;
    output
        .write_all(&[0u8; 0x20])
        .map_err(|e| format!("Write DOL padding failed: {e}"))?;
    pad_to_alignment(&mut output, 0x100)?;

    let new_dol_offset = output
        .stream_position()
        .map_err(|e| format!("Get DOL offset failed: {e}"))? as u32;

    source
        .seek(SeekFrom::Start(dol_offset_orig))
        .map_err(|e| format!("Seek source DOL failed: {e}"))?;
    let mut dol_buf = vec![0u8; dol_size as usize];
    source
        .read_exact(&mut dol_buf)
        .map_err(|e| format!("Read DOL failed: {e}"))?;
    output
        .write_all(&dol_buf)
        .map_err(|e| format!("Write DOL failed: {e}"))?;

    write_u32_be_at(&mut output, 0x420, new_dol_offset)?;

    // Position and write FST
    output
        .seek(SeekFrom::Start((new_dol_offset as u64) + dol_size))
        .map_err(|e| format!("Seek after DOL failed: {e}"))?;
    output
        .write_all(&[0u8; 0x20])
        .map_err(|e| format!("Write FST padding failed: {e}"))?;
    pad_to_alignment(&mut output, 0x100)?;

    let new_fst_offset = output
        .stream_position()
        .map_err(|e| format!("Get FST offset failed: {e}"))? as u32;

    output
        .write_all(&original_fst)
        .map_err(|e| format!("Write FST failed: {e}"))?;

    let new_fst_size = original_fst.len() as u32;
    write_u32_be_at(&mut output, 0x424, new_fst_offset)?;
    write_u32_be_at(&mut output, 0x428, new_fst_size)?;
    write_u32_be_at(&mut output, 0x42C, new_fst_size)?;

    // Position for files
    output
        .seek(SeekFrom::Start(
            (new_fst_offset as u64) + (new_fst_size as u64),
        ))
        .map_err(|e| format!("Seek to files failed: {e}"))?;
    pad_to_alignment(&mut output, 4)?;

    // Sort files by their ORIGINAL offset to maintain original file order (important for game performance)
    let mut sorted_files = all_files.to_vec();
    sorted_files.sort_by_key(|f| f.offset);

    // Write all files in original order (modified or original)
    for file in sorted_files {
        let rel_path = file
            .path
            .strip_prefix("files/")
            .unwrap_or(&file.path)
            .to_string();

        let current_offset = output
            .stream_position()
            .map_err(|e| format!("Get file offset failed: {e}"))?
            as u32;

        // Try to find a matching replacement (by full path, stripped path, or filename)
        let file_data = {
            let mut found = None;

            // Try exact matches first
            if let Some(data) = replacements.get(&rel_path) {
                found = Some(data.clone());
            } else if let Some(data) = replacements.get(&file.path) {
                found = Some(data.clone());
            } else {
                // Try by filename
                let filename_only = std::path::Path::new(&rel_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                for (key, value) in replacements.iter() {
                    let key_filename = std::path::Path::new(key)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");

                    if key_filename == filename_only {
                        found = Some(value.clone());
                        break;
                    }
                }
            }

            match found {
                Some(replacement) => replacement,
                None => {
                    // Copy original
                    source
                        .seek(SeekFrom::Start(file.offset))
                        .map_err(|e| format!("Seek file failed: {e}"))?;
                    let mut buf = vec![0u8; file.size as usize];
                    source
                        .read_exact(&mut buf)
                        .map_err(|e| format!("Read file failed: {e}"))?;
                    buf
                }
            }
        };

        output
            .write_all(&file_data)
            .map_err(|e| format!("Write file failed: {e}"))?;

        let file_end = output
            .stream_position()
            .map_err(|e| format!("Get position failed: {e}"))?;

        // Update FST entry by index directly
        update_fst_entry_by_index(
            &mut output,
            file.fst_index,
            current_offset,
            file_data.len() as u32,
            new_fst_offset as u64,
        )?;

        // Seek back to where we were before FST update
        output
            .seek(SeekFrom::Start(file_end))
            .map_err(|e| format!("Seek back to file position failed: {e}"))?;

        pad_to_alignment(&mut output, 4)?;

        // // Only log modified files
        // let filename = std::path::Path::new(&file.path)
        //     .file_name()
        //     .and_then(|n| n.to_str())
        //     .unwrap_or(&file.path);

        // if replacements.iter().any(|(k, _)| k.ends_with(filename)) {
        //     println!(
        //         "  Wrote: {} ({} bytes @ 0x{:x}) [modified]",
        //         file.path,
        //         file_data.len(),
        //         current_offset
        //     );
        // }
    }

    output.sync_all().map_err(|e| format!("Sync failed: {e}"))?;
    Ok(())
}

fn update_fst_entry_by_index(
    output: &mut File,
    fst_index: usize,
    new_offset: u32,
    new_size: u32,
    fst_offset: u64,
) -> Result<(), String> {
    let entry_offset = fst_offset + (fst_index as u64 * FST_ENTRY_SIZE as u64);
    output
        .seek(SeekFrom::Start(entry_offset + 4))
        .map_err(|e| format!("Seek FST entry failed: {e}"))?;
    output
        .write_all(&new_offset.to_be_bytes())
        .map_err(|e| format!("Write offset failed: {e}"))?;
    output
        .write_all(&new_size.to_be_bytes())
        .map_err(|e| format!("Write size failed: {e}"))?;
    Ok(())
}
