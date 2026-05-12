/*
Shared byte-reading utilities.

These helpers centralize endian decoding used by multiple parser modules.
*/

const U16_SIZE: usize = 2;
const U32_SIZE: usize = 4;

pub fn read_u16_be(data: &[u8], offset: usize) -> u16 {
    let b = &data[offset..offset + U16_SIZE];
    u16::from_be_bytes([b[0], b[1]])
}

pub fn read_u32_be(data: &[u8], offset: usize) -> u32 {
    let b = &data[offset..offset + U32_SIZE];
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

pub fn read_u8_at(data: &[u8], offset: usize) -> Option<u8> {
    data.get(offset).copied()
}

pub fn read_u24_be(data: &[u8], offset: usize) -> Option<u32> {
    if offset + 3 > data.len() {
        return None;
    }
    Some(((data[offset] as u32) << 16) | ((data[offset + 1] as u32) << 8) | (data[offset + 2] as u32))
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join("")
}

pub fn hex_to_bytes(hex_str: &str) -> Result<Vec<u8>, String> {
    if hex_str.len() % 2 != 0 {
        return Err("Hex string must have even length".to_string());
    }
    let mut bytes = Vec::new();
    for i in (0..hex_str.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex_str[i..i+2], 16)
            .map_err(|e| format!("Invalid hex: {}", e))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

