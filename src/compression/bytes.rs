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
