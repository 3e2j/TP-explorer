/*
Yaz0 decompression utilities.

This module implements the minimal Yaz0 decoder needed by the archive parser.
*/

// Yaz0 header
pub const MAGIC_YAZ0: &[u8; 4] = b"Yaz0";
const YAZ0_HEADER_SIZE: usize = 16;
const YAZ0_SIZE_FIELD_OFFSET: usize = 4;

// Group/code byte
const BITS_PER_GROUP: usize = 8;
const CODE_START_MASK: u8 = 0x80;

// Back-reference decoding
const BACKREF_PAIR_SIZE: usize = 2;
const BACKREF_NIBBLE_MASK: usize = 0x0F;
const BACKREF_COPY_LEN_BASE: usize = 2;
const BACKREF_COPY_LEN_EXTENDED_SENTINEL: usize = 0;
const BACKREF_COPY_LEN_EXTENDED_BASE: usize = 0x12;

use crate::bytes::read_u32_be;

pub fn yaz0_decompress(input: &[u8]) -> Option<Vec<u8>> {
    if input.len() < YAZ0_HEADER_SIZE || &input[0..MAGIC_YAZ0.len()] != MAGIC_YAZ0 {
        return None;
    }
    let uncompressed_size = read_u32_be(input, YAZ0_SIZE_FIELD_OFFSET) as usize;
    let mut out = Vec::with_capacity(uncompressed_size);

    let mut src = YAZ0_HEADER_SIZE;
    while out.len() < uncompressed_size {
        if src >= input.len() {
            break;
        }
        let code = input[src];
        src += 1;
        for bit in 0..BITS_PER_GROUP {
            if (code & (CODE_START_MASK >> bit)) != 0 {
                if src >= input.len() {
                    return None;
                }
                out.push(input[src]);
                src += 1;
            } else {
                if src + 1 >= input.len() {
                    return None;
                }
                let b1 = input[src];
                let b2 = input[src + 1];
                src += BACKREF_PAIR_SIZE;
                let dist = (((b1 as usize) & BACKREF_NIBBLE_MASK) << 8) | (b2 as usize);
                let mut copy_len = (b1 >> 4) as usize;
                if copy_len == BACKREF_COPY_LEN_EXTENDED_SENTINEL {
                    if src >= input.len() {
                        return None;
                    }
                    let extra = input[src] as usize;
                    src += 1;
                    copy_len = extra + BACKREF_COPY_LEN_EXTENDED_BASE;
                } else {
                    copy_len += BACKREF_COPY_LEN_BASE;
                }
                let backref_pos = out.len().checked_sub(dist + 1)?;
                // Byte-wise copying preserves correct behavior for overlapping backrefs.
                for i in 0..copy_len {
                    let val = out[backref_pos + i];
                    out.push(val);
                }
            }
            if out.len() >= uncompressed_size {
                break;
            }
        }
    }

    if out.len() != uncompressed_size {
        None
    } else {
        Some(out)
    }
}
