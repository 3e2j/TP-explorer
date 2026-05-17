mod common;

use tpmt::utils::*;

// Verifies big-endian reads preserve parser byte order.
#[test]
fn read_u16_be_decodes_big_endian_values() {
    assert_eq!(read_u16_be(&[0x12, 0x34], 0), 0x1234);
}

// Verifies the 32-bit reader matches the file format headers used elsewhere.
#[test]
fn read_u32_be_decodes_big_endian_values() {
    assert_eq!(read_u32_be(&[0x12, 0x34, 0x56, 0x78], 0), 0x12345678);
}

// Verifies out-of-bounds byte access stays safe for truncated data.
#[test]
fn read_u8_at_returns_none_out_of_bounds() {
    assert_eq!(read_u8_at(&[0xAA], 1), None);
}

// Verifies hex formatting preserves byte order and lowercase output.
#[test]
fn bytes_to_hex_formats_all_bytes() {
    assert_eq!(bytes_to_hex(&[0x0A, 0xFF]), "0aff");
}

// Verifies odd-length hex is accepted by padding the leading nibble.
#[test]
fn hex_to_bytes_pads_odd_length_input() {
    assert_eq!(hex_to_bytes("abc"), Ok(vec![0x0A, 0xBC]));
}

// Verifies invalid hex digits still fail loudly instead of being ignored.
#[test]
fn hex_to_bytes_rejects_invalid_digits() {
    assert!(hex_to_bytes("zz").is_err());
}
