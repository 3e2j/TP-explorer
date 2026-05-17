mod common;

use std::io::Cursor;
use tpmt::formats::compression::{gz2e, yaz0};

// Verifies the GZ2E header check only accepts the documented wrapper magic.
#[test]
fn is_gz2e_recognizes_the_wrapper_magic() {
    assert!(gz2e::is_gz2e(b"GZ2E"));
}

// Verifies invalid GZ2E magic is rejected before any copy or decode work begins.
#[test]
fn decompress_gz2e_rejects_invalid_magic() {
    let mut input = Cursor::new(vec![0u8; 32]);
    let mut output = Vec::new();
    assert!(gz2e::decompress_gz2e(&mut input, &mut output).is_err());
}

// Verifies passthrough GZ2E ISOs are copied intact when the FST offset looks valid.
#[test]
fn decompress_gz2e_copies_passthrough_isos() {
    let mut bytes = vec![0u8; 0x440];
    bytes[0..4].copy_from_slice(b"GZ2E");
    bytes[4..6].copy_from_slice(b"01");
    bytes[0x420..0x424].copy_from_slice(&0x1800u32.to_be_bytes());
    bytes[0x424..0x428].copy_from_slice(&0x2000u32.to_be_bytes());
    bytes[0x428..0x42c].copy_from_slice(&0x2000u32.to_be_bytes());
    let mut input = Cursor::new(bytes.clone());
    let mut output = Vec::new();
    gz2e::decompress_gz2e(&mut input, &mut output).expect("decompress");
    assert_eq!(output, bytes);
}

// Verifies compression and decompression round-trip repeated data without corruption.
#[test]
fn yaz0_roundtrip_preserves_input_bytes() {
    let input = b"ABABABABABABABAB".to_vec();
    let compressed = yaz0::yaz0_compress(&input).expect("compress");
    assert_eq!(yaz0::yaz0_decompress(&compressed), Some(input));
}

// Verifies non-Yaz0 input is rejected before the decoder touches payload bytes.
#[test]
fn yaz0_decompress_rejects_wrong_magic() {
    assert_eq!(yaz0::yaz0_decompress(b"NOTYAZ0"), None);
}
