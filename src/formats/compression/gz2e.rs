//! GZ2E decompression for Twilight Princess Gamecube ISO.
//!
//! GZ2E ISOs are stored with the GZ2E magic followed by the standard ISO structure
//! This appears to be the case with the Twilight Princess ISO.
//!
//! GZ2E decoding checks for the game revision "01", file is rejected if this doesn't match.

use std::io::{Read, Seek, SeekFrom, Write};

const MAGIC: &[u8; 4] = b"GZ2E";
const DISC_INFO_OFFSET: u64 = 0x400;
const DISC_INFO_SIZE: usize = 0x40;

/// Returns true when the 4-byte header matches the GZ2E wrapper magic.
///
/// # Examples
///
/// ```
/// use tpmt::formats::compression::gz2e::is_gz2e;
/// assert!(is_gz2e(b"GZ2E"));
/// ```
pub fn is_gz2e(data: &[u8]) -> bool {
    data.len() >= 4 && &data[0..4] == MAGIC
}

/// Decompresses a GZ2E-wrapped ISO stream into the provided writer.
///
/// Returns the input unchanged when it looks like a normal ISO with a GZ2E
/// wrapper; otherwise it reports unsupported block-based GZ2E data.
pub fn decompress_gz2e<R: Read + Seek, W: Write>(
    reader: &mut R,
    writer: &mut W,
) -> Result<(), String> {
    let mut header = [0u8; 32];
    reader
        .read_exact(&mut header)
        .map_err(|e| format!("Failed to read GZ2E header: {e}"))?;

    if !is_gz2e(&header) {
        return Err("Invalid GZ2E magic".to_string());
    }

    // Revision should be ASCII "01" at bytes 4-5
    let revision_str = std::str::from_utf8(&header[4..6]).unwrap_or("??");
    if revision_str != "01" {
        return Err(format!("Unsupported GZ2E revision: {revision_str}"));
    }

    // "Yet Another Gamecube Documentation" (YAGCD) disc header places the
    // executable and FST pointers in the block starting at 0x400,
    // so collect the fields we care about (specifically fst)
    //
    // Source: https://hitmen.c02.at/files/yagcd/yagcd/chap13.html
    reader
        .seek(SeekFrom::Start(DISC_INFO_OFFSET))
        .map_err(|e| format!("Failed to seek to disc info: {e}"))?;

    // Start at 0x400
    let mut disc_info = [0u8; DISC_INFO_SIZE];
    reader
        .read_exact(&mut disc_info)
        .map_err(|e| format!("Failed to read disc info: {e}"))?;

    // let _debug_monitor_offset = u32::from_be_bytes(disc_info[0..4].try_into().unwrap());
    // let _debug_monitor_addr = u32::from_be_bytes(disc_info[4..8].try_into().unwrap());
    // let _unused_before_dol = &disc_info[8..0x20];
    // let _dol_offset = u32::from_be_bytes(disc_info[0x20..0x24].try_into().unwrap());
    let fst_offset = u32::from_be_bytes(disc_info[0x24..0x28].try_into().unwrap()) as u64;
    // let _fst_size = u32::from_be_bytes(disc_info[0x28..0x2C].try_into().unwrap());
    // let _fst_max_size = u32::from_be_bytes(disc_info[0x2C..0x30].try_into().unwrap());
    // let _user_position = u32::from_be_bytes(disc_info[0x30..0x34].try_into().unwrap());
    // let _user_length = u32::from_be_bytes(disc_info[0x34..0x38].try_into().unwrap());
    // let _unknown = u32::from_be_bytes(disc_info[0x38..0x3C].try_into().unwrap());
    // let _unused_after_unknown = &disc_info[0x3C..0x40];

    // A plausible FST offset means this is a wrapped ISO and can be copied as-is.
    if fst_offset > 0x1000 && fst_offset < 0x10000000 {
        reader
            .seek(SeekFrom::Start(0))
            .map_err(|e| format!("Failed to seek to start: {e}"))?;
        std::io::copy(reader, writer).map_err(|e| format!("Failed to copy ISO data: {e}"))?;
        return Ok(());
    }
    // Otherwise try full decompression via blocks
    decompress_gz2e_blocks(reader, writer, &header)
}

fn decompress_gz2e_blocks<R: Read + Seek, W: Write>(
    _reader: &mut R,
    _writer: &mut W,
    _header: &[u8; 32],
) -> Result<(), String> {
    Err("This GZ2E format is not yet fully supported. Please decompress manually using Dolphin or wiimm tools.".to_string())
}
