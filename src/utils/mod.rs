//! Shared byte and file helpers.
//!
//! These utilities centralize endian decoding, hex conversion, file-range
//! reads, and file-at-offset writes used by multiple parser and pipeline
//! modules.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

const U16_SIZE: usize = 2;
const U32_SIZE: usize = 4;

/// Reads a big-endian `u16` from a byte slice.
///
/// # Examples
///
/// ```
/// use tpmt::utils::read_u16_be;
/// assert_eq!(read_u16_be(&[0x12, 0x34], 0), 0x1234);
/// ```
pub fn read_u16_be(data: &[u8], offset: usize) -> u16 {
    let b = &data[offset..offset + U16_SIZE];
    u16::from_be_bytes([b[0], b[1]])
}

/// Reads a big-endian `u32` from a byte slice.
///
/// # Examples
///
/// ```
/// use tpmt::utils::read_u32_be;
/// assert_eq!(read_u32_be(&[0x12, 0x34, 0x56, 0x78], 0), 0x12345678);
/// ```
pub fn read_u32_be(data: &[u8], offset: usize) -> u32 {
    let b = &data[offset..offset + U32_SIZE];
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// Writes a big-endian `u16` into a mutable byte slice.
///
/// # Examples
///
/// ```
/// use tpmt::utils::write_u16_be;
/// let mut buf = [0u8; 2];
/// write_u16_be(&mut buf, 0, 0x1234);
/// assert_eq!(buf, [0x12, 0x34]);
/// ```
pub fn write_u16_be(data: &mut [u8], offset: usize, value: u16) {
    if offset + U16_SIZE <= data.len() {
        data[offset..offset + U16_SIZE].copy_from_slice(&value.to_be_bytes());
    }
}

/// Writes a big-endian `u32` into a mutable byte slice.
///
/// # Examples
///
/// ```
/// use tpmt::utils::write_u32_be;
/// let mut buf = [0u8; 4];
/// write_u32_be(&mut buf, 0, 0x12345678);
/// assert_eq!(buf, [0x12, 0x34, 0x56, 0x78]);
/// ```
pub fn write_u32_be(data: &mut [u8], offset: usize, value: u32) {
    if offset + U32_SIZE <= data.len() {
        data[offset..offset + U32_SIZE].copy_from_slice(&value.to_be_bytes());
    }
}

/// Reads a byte at `offset` without panicking when the slice is too short.
pub fn read_u8_at(data: &[u8], offset: usize) -> Option<u8> {
    data.get(offset).copied()
}

/// Converts raw bytes to lowercase hex.
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("")
}

/// Parses a hex string into bytes.
///
/// # Examples
///
/// ```
/// use tpmt::utils::hex_to_bytes;
/// assert_eq!(hex_to_bytes("abc").unwrap(), vec![0x0a, 0xbc]);
/// ```
pub fn hex_to_bytes(hex_str: &str) -> Result<Vec<u8>, String> {
    // Accept odd-length hex by padding a leading zero. This makes the importer
    // more tolerant of accidental single-digit nibbles produced by editors.
    let mut s = hex_str.to_string();
    if s.len() % 2 != 0 {
        s = format!("0{}", s);
    }

    let mut bytes = Vec::new();
    for i in (0..s.len()).step_by(2) {
        let chunk = &s[i..i + 2];
        let byte = u8::from_str_radix(chunk, 16)
            .map_err(|e| format!("Invalid hex '{}' at pos {}: {}", hex_str, i, e))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

/// Computes the SHA-1 hex digest for a byte slice.
///
/// # Examples
///
/// ```
/// use tpmt::utils::sha1_hex;
/// assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
/// ```
pub fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = sha1::Sha1::new();
    hasher.update(bytes);
    hasher.digest().to_string()
}

/// Reads a byte range from a file at a specific offset.
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use tpmt::utils::read_bytes_at;
/// let mut file = File::open("input.bin").unwrap();
/// let bytes = read_bytes_at(&mut file, 0, 4).unwrap();
/// assert_eq!(bytes.len(), 4);
/// ```
pub fn read_bytes_at(file: &mut File, offset: u64, size: u64) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Seek failed: {e}"))?;
    let mut out = vec![0u8; size as usize];
    file.read_exact(&mut out)
        .map_err(|e| format!("Read failed: {e}"))?;
    Ok(out)
}

/// Writes a `u32` to a file at a specific offset in big-endian order.
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use tpmt::utils::write_u32_be_at;
/// let path = std::env::temp_dir().join("tpmt-write-u32.bin");
/// std::fs::write(&path, [0u8; 8]).unwrap();
/// let mut file = File::options().read(true).write(true).open(&path).unwrap();
/// write_u32_be_at(&mut file, 2, 0x12345678).unwrap();
/// ```
pub fn write_u32_be_at(file: &mut File, offset: u64, value: u32) -> Result<(), String> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Seek failed: {e}"))?;
    file.write_all(&value.to_be_bytes())
        .map_err(|e| format!("Write failed: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies big-endian u16 reads so binary parsers decode the high byte first.
    #[test]
    fn read_u16_be_decodes_big_endian_values() {
        assert_eq!(read_u16_be(&[0x12, 0x34], 0), 0x1234);
    }

    // Verifies big-endian u32 reads so larger headers are interpreted correctly.
    #[test]
    fn read_u32_be_decodes_big_endian_values() {
        assert_eq!(read_u32_be(&[0x12, 0x34, 0x56, 0x78], 0), 0x12345678);
    }

    // Verifies byte lookups stay bounds-safe instead of panicking on missing data.
    #[test]
    fn read_u8_at_returns_none_when_offset_is_out_of_bounds() {
        assert_eq!(read_u8_at(&[0xAA], 1), None);
    }

    // Verifies byte-to-hex formatting preserves ordering and lowercase output.
    #[test]
    fn bytes_to_hex_formats_all_bytes_in_order() {
        assert_eq!(bytes_to_hex(&[0x0A, 0xFF]), "0aff");
    }

    // Verifies slice writers preserve the exact big-endian byte order used by file formats.
    #[test]
    fn write_u16_be_and_u32_be_write_big_endian_values() {
        let mut u16_buf = [0u8; 2];
        let mut u32_buf = [0u8; 4];
        write_u16_be(&mut u16_buf, 0, 0x1234);
        write_u32_be(&mut u32_buf, 0, 0x12345678);
        assert_eq!(u16_buf, [0x12, 0x34]);
        assert_eq!(u32_buf, [0x12, 0x34, 0x56, 0x78]);
    }

    // Verifies odd-length hex is tolerated by padding the leading nibble.
    #[test]
    fn hex_to_bytes_pads_odd_length_input() {
        assert_eq!(hex_to_bytes("abc"), Ok(vec![0x0A, 0xBC]));
    }

    // Verifies invalid hex is surfaced as an error instead of being silently skipped.
    #[test]
    fn hex_to_bytes_reports_invalid_digits() {
        assert!(hex_to_bytes("zz").is_err());
    }

    // Verifies SHA-1 formatting matches the hex digest used by manifest hashing.
    #[test]
    fn sha1_hex_formats_bytes() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    // Verifies file-range reads extract the requested bytes without consuming the whole file.
    #[test]
    fn read_bytes_at_reads_requested_range() {
        let path = std::env::temp_dir().join(format!("tpmt-utils-{}.bin", std::process::id()));
        std::fs::write(&path, b"0123456789").unwrap();
        let mut file = File::open(&path).unwrap();
        assert_eq!(read_bytes_at(&mut file, 2, 3).unwrap(), b"234");
    }

    // Verifies file writes target the requested offset instead of appending at the end.
    #[test]
    fn write_u32_be_at_writes_to_requested_offset() {
        let path = std::env::temp_dir().join(format!("tpmt-utils-{}.bin", std::process::id()));
        std::fs::write(&path, vec![0u8; 8]).unwrap();
        let mut file = File::options().read(true).write(true).open(&path).unwrap();
        write_u32_be_at(&mut file, 2, 0x12345678).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[2..6], &[0x12, 0x34, 0x56, 0x78]);
    }
}
