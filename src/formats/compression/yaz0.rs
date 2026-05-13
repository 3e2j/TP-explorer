/*
Yaz0 decompression/compression utilities.

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

use crate::utils::read_u32_be;

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

/// Yaz0 compression using LZ77-style matching.
/// Searches for matching byte sequences and encodes them as back-references.
pub fn yaz0_compress(input: &[u8]) -> Option<Vec<u8>> {
    const MAX_RUN_LENGTH: usize = 0xFF + 0x12;
    const DEFAULT_SEARCH_DEPTH: usize = 0x1000;

    let mut out = Vec::new();

    // Write header: "Yaz0" + uncompressed size + 8 zero bytes
    out.extend_from_slice(MAGIC_YAZ0);
    out.extend_from_slice(&(input.len() as u32).to_be_bytes());
    out.extend_from_slice(&[0u8; 8]);

    let mut uncomp_offset = 0;
    let mut reserved_match: Option<(usize, usize)> = None; // (num_bytes, match_pos)

    while uncomp_offset < input.len() {
        let mut dst = Vec::new();
        let mut mask = 0u8;

        for bit in 0..BITS_PER_GROUP {
            if uncomp_offset >= input.len() {
                break;
            }

            let (num_bytes, match_pos) = if let Some((next_bytes, next_pos)) = reserved_match.take()
            {
                (next_bytes, next_pos)
            } else {
                find_match(input, uncomp_offset, DEFAULT_SEARCH_DEPTH, MAX_RUN_LENGTH)
            };

            if num_bytes < 3 {
                // Literal byte
                dst.push(input[uncomp_offset]);
                uncomp_offset += 1;
                mask |= 0x80 >> bit;

                // Look ahead for a better match at the *next* position — but
                // only when we are not already holding a reserved match.
                // This mirrors Python's get_num_bytes_and_match_pos logic:
                // reserve the next match only when it is >= current+2 and we
                // haven't already reserved one.
                if reserved_match.is_none() && uncomp_offset < input.len() {
                    let (next_bytes, next_pos) =
                        find_match(input, uncomp_offset, DEFAULT_SEARCH_DEPTH, MAX_RUN_LENGTH);
                    if next_bytes >= num_bytes + 2 {
                        reserved_match = Some((next_bytes, next_pos));
                    }
                }
            } else {
                // Back-reference
                let dist = uncomp_offset - match_pos - 1;

                if num_bytes >= 0x12 {
                    // 3-byte encoding: dist high byte (bits 8-11 only), dist
                    // low byte, then length - 0x12.
                    // Python: dst.append((dist & 0xFF00) >> 8)  — keeps bits
                    // 8-11 of dist in the low nibble of the first byte.
                    dst.push(((dist >> 8) & 0x0F) as u8);
                    dst.push((dist & 0xFF) as u8);
                    let len_to_encode = num_bytes.min(MAX_RUN_LENGTH) - 0x12;
                    dst.push(len_to_encode as u8);
                } else {
                    // 2-byte encoding: high nibble = len-2, low nibble of
                    // first byte = bits 8-11 of dist.
                    let byte = (((num_bytes - 2) as u8) << 4) | (((dist >> 8) & 0x0F) as u8);
                    dst.push(byte);
                    dst.push((dist & 0xFF) as u8);
                }

                uncomp_offset += num_bytes;
            }
        }

        // Write mask and data for this group.
        out.push(mask);
        out.extend_from_slice(&dst);
    }

    // Mirror the Python reference: when compression ends exactly on an 8-bit
    // group boundary (mask_bits_done == 0 after the loop), write one trailing
    // zero byte.  The while-loop above only runs while uncomp_offset < len, so
    // this byte would never be emitted otherwise.
    //
    // Python (yaz0_yay0.py lines 224-236):
    //   else:
    //       fs.write_u8(comp_data, comp_offset, 0)
    out.push(0);

    Some(out)
}

/// Find the best matching byte sequence before the given offset.
fn find_match(data: &[u8], offset: usize, search_depth: usize, max_run: usize) -> (usize, usize) {
    let search_start = if offset > search_depth {
        offset - search_depth
    } else {
        0
    };
    let max_check = (data.len() - offset).min(max_run);

    let mut best_len = 0;
    let mut best_pos = 0;

    for search_pos in search_start..offset {
        let mut len = 0;
        while len < max_check && data[search_pos + len] == data[offset + len] {
            len += 1;
        }

        if len > best_len {
            best_len = len;
            best_pos = search_pos;
        }
    }

    (best_len, best_pos)
}
