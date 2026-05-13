/*
GZ2E decompression for Twilight Princess Gamecube ISO.

Some GZ2E ISOs are actually stored with the GZ2E01 marker followed by the standard
GameCube ISO structure. This appears to be the case with the Twilight Princess ISO.
*/

use std::io::{Read, Seek, SeekFrom, Write};

const MAGIC: &[u8; 4] = b"GZ2E";

pub fn is_gz2e(data: &[u8]) -> bool {
    data.len() >= 4 && &data[0..4] == MAGIC
}

pub fn decompress_gz2e<R: Read + Seek, W: Write>(
    reader: &mut R,
    writer: &mut W,
) -> Result<(), String> {
    let mut header = [0u8; 32];
    reader
        .read_exact(&mut header)
        .map_err(|e| format!("Failed to read GZ2E header: {e}"))?;

    if &header[0..4] != MAGIC {
        return Err("Invalid GZ2E magic".to_string());
    }

    // Revision is ASCII "01" at bytes 4-5
    let revision_str = std::str::from_utf8(&header[4..6]).unwrap_or("??");
    if revision_str != "01" {
        return Err(format!("Unsupported GZ2E revision: {revision_str}"));
    }

    // Check if this looks like a standard GameCube ISO with just a GZ2E marker
    // GameCube ISOs have specific magic at offset 0x1C (after the GZ2E header, this would be at 0x20)
    // and also have the disc info starting around 0x20

    // For Twilight Princess case: check offset 0x424 (FST offset) relative to position 0x20
    reader
        .seek(SeekFrom::Start(0x424))
        .map_err(|e| format!("Failed to seek to FST offset: {e}"))?;

    let mut fst_offset_bytes = [0u8; 4];
    reader
        .read_exact(&mut fst_offset_bytes)
        .map_err(|e| format!("Failed to read FST offset: {e}"))?;

    let fst_offset = u32::from_be_bytes(fst_offset_bytes) as u64;

    // If FST offset looks valid (not zero, reasonable size), this is likely a standard ISO
    // with a GZ2E wrapper, so we can just read it as-is
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
