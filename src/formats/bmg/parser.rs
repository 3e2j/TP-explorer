/*
BMG file format parser and writer.

BMG (Binary Message Graphics) files contain localized game text with support for
multiple encodings and special control sequences.
*/

use crate::utils::{read_u16_be, read_u32_be};

const MAGIC_BMG: &[u8; 8] = b"MESGbmg1";
const ENCODING_LEGACY: u32 = 0x00000000;
const ENCODING_WINDOWS_1252: u32 = 0x01000000;
const ENCODING_UTF16: u32 = 0x02000000;
const ENCODING_SHIFT_JIS: u32 = 0x03000000;
const ENCODING_UTF8: u32 = 0x04000000;

#[derive(Debug, Clone)]
pub struct BmgMessage {
    pub id: (u32, u8),
    pub attributes: Vec<u8>,
    pub text: Vec<Vec<u8>>,
}

#[derive(Debug)]
pub struct BmgSection {
    pub magic: [u8; 4],
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct Bmg {
    pub encoding: String,
    pub messages: Vec<BmgMessage>,
    pub attribute_length: u16,
    pub additional_sections: Vec<BmgSection>,
}

impl Bmg {
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        if data.len() < 32 {
            return Err("BMG data too short".to_string());
        }

        if &data[0..8] != MAGIC_BMG {
            return Err("Invalid BMG magic".to_string());
        }

        let _filesize = read_u32_be(data, 8);
        let section_count = read_u32_be(data, 12) as usize;
        let encoding_val = read_u32_be(data, 16);

        let encoding = match encoding_val {
            ENCODING_LEGACY => "legacy-bmg".to_string(),
            ENCODING_WINDOWS_1252 => "windows-1252".to_string(),
            ENCODING_UTF16 => "utf-16be".to_string(),
            ENCODING_SHIFT_JIS => "shift-jis".to_string(),
            ENCODING_UTF8 => "utf-8".to_string(),
            _ => "shift-jis".to_string(),
        };

        // Parse sections sequentially (no padding assumptions)
        let mut offset = 32;
        let mut sections = Vec::new();

        for _ in 0..section_count {
            if offset + 8 > data.len() {
                return Err("Section header out of bounds".to_string());
            }

            let magic = [
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ];
            let section_size = read_u32_be(data, offset + 4) as usize;
            offset += 8;

            if section_size < 8 {
                return Err("Section size too small".to_string());
            }

            let data_size = section_size - 8;
            // Read what we can, even if section is truncated
            let actual_data_size = if offset + data_size > data.len() {
                // Section is truncated, read what's available
                data.len() - offset
            } else {
                data_size
            };

            let section_data = data[offset..offset + actual_data_size].to_vec();
            sections.push((magic, section_data));
            offset += actual_data_size;
        }

        if sections.len() < 3 {
            return Err("BMG must have at least 3 sections".to_string());
        }

        // Find the required sections (INF1, DAT1, MID1) by scanning instead of assuming order
        let mut inf1_data = None;
        let mut dat1_data = None;
        let mut mid1_data = None;
        let mut additional_sections = Vec::new();

        for (magic, section_data) in sections.iter() {
            match magic {
                b"INF1" => inf1_data = Some(section_data),
                b"DAT1" => dat1_data = Some(section_data),
                b"MID1" => mid1_data = Some(section_data),
                _ => additional_sections.push(BmgSection {
                    magic: *magic,
                    data: section_data.clone(),
                }),
            }
        }

        let inf1_data = inf1_data.ok_or("Missing INF1 section")?;
        let dat1_data = dat1_data.ok_or("Missing DAT1 section")?;
        let mid1_data = mid1_data.ok_or("Missing MID1 section")?;

        let (messages, attribute_length) = parse_messages(inf1_data, dat1_data, mid1_data)?;

        Ok(Bmg {
            encoding,
            messages,
            attribute_length,
            additional_sections,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut output = Vec::new();
        output.extend_from_slice(MAGIC_BMG);
        output.extend_from_slice(&[0u8; 4]); // Placeholder filesize

        let section_count = 3 + self.additional_sections.len();
        output.extend_from_slice(&(section_count as u32).to_be_bytes());

        let encoding_val = match self.encoding.as_str() {
            "legacy-bmg" => ENCODING_LEGACY,
            "windows-1252" | "latin-1" => ENCODING_WINDOWS_1252,
            "utf-16be" => ENCODING_UTF16,
            "shift-jis" => ENCODING_SHIFT_JIS,
            "utf-8" => ENCODING_UTF8,
            _ => ENCODING_SHIFT_JIS,
        };
        output.extend_from_slice(&encoding_val.to_be_bytes());
        output.extend_from_slice(&[0u8; 12]);

        let mut inf_data = Vec::new();
        let mut dat_data = Vec::new();
        let mut mid_data = Vec::new();

        write_inf1(&mut inf_data, self)?;
        write_dat1(&mut dat_data, self)?;
        write_mid1(&mut mid_data, self)?;

        write_section(&mut output, b"INF1", &inf_data)?;
        write_section(&mut output, b"DAT1", &dat_data)?;
        write_section(&mut output, b"MID1", &mid_data)?;

        for section in &self.additional_sections {
            write_section(&mut output, &section.magic, &section.data)?;
        }

        let filesize = output.len() as u32;
        output[8..12].copy_from_slice(&filesize.to_be_bytes());

        Ok(output)
    }
}

fn parse_messages(inf1: &[u8], dat1: &[u8], mid1: &[u8]) -> Result<(Vec<BmgMessage>, u16), String> {
    if inf1.len() < 8 {
        return Err("INF1 too small".to_string());
    }

    let message_count = read_u16_be(inf1, 0) as usize;
    let attribute_length = read_u16_be(inf1, 2) as usize;

    let mut messages = Vec::new();
    let mut offset = 8;

    for i in 0..message_count {
        if offset + 4 > inf1.len() {
            return Err("INF1 entry out of bounds".to_string());
        }

        let dat_offset = read_u32_be(inf1, offset) as usize;
        let attr_len = attribute_length.saturating_sub(4);

        if offset + 4 + attr_len > inf1.len() {
            return Err("INF1 attributes out of bounds".to_string());
        }

        let attributes = inf1[offset + 4..offset + 4 + attr_len].to_vec();
        offset += 4 + attr_len;

        let text = parse_message_text(dat1, dat_offset)?;

        let (id, subid) = if mid1.len() >= 8 + (i * 4) + 3 {
            let msg_offset = 8 + (i * 4);
            let id = read_u24_be(mid1, msg_offset).unwrap_or(0);
            let subid = mid1[msg_offset + 3];
            (id, subid)
        } else {
            (0, 0)
        };

        messages.push(BmgMessage {
            id: (id, subid),
            attributes,
            text,
        });
    }

    Ok((messages, attribute_length as u16))
}

fn parse_message_text(dat1: &[u8], start: usize) -> Result<Vec<Vec<u8>>, String> {
    if start >= dat1.len() {
        return Ok(vec![vec![]]);
    }

    let mut text_parts = Vec::new();
    let mut current = Vec::new();
    let mut offset = start;

    while offset < dat1.len() {
        let byte = dat1[offset];

        if byte == 0x00 {
            text_parts.push(current);
            break;
        } else if byte == 0x1A {
            if !current.is_empty() {
                text_parts.push(current);
                current = Vec::new();
            }

            offset += 1;
            if offset >= dat1.len() {
                return Err("Escape sequence truncated".to_string());
            }

            let arg_len = dat1[offset] as usize;
            // arg_len is the total escape length including the 0x1A and length byte
            // So we need arg_len-1 more bytes from offset (we already have 0x1A)
            if offset + arg_len - 1 > dat1.len() {
                return Err("Escape data truncated".to_string());
            }

            let mut escape = vec![0x1A];
            escape.extend_from_slice(&dat1[offset..offset + arg_len - 1]);
            text_parts.push(escape);
            offset += arg_len - 2;
        } else {
            current.push(byte);
        }

        offset += 1;
    }

    Ok(text_parts)
}

fn read_u24_be(data: &[u8], offset: usize) -> Option<u32> {
    if offset + 3 > data.len() {
        return None;
    }
    Some(
        ((data[offset] as u32) << 16)
            | ((data[offset + 1] as u32) << 8)
            | (data[offset + 2] as u32),
    )
}

fn write_inf1(out: &mut Vec<u8>, bmg: &Bmg) -> Result<(), String> {
    out.extend_from_slice(&(bmg.messages.len() as u16).to_be_bytes());
    out.extend_from_slice(&bmg.attribute_length.to_be_bytes());
    out.extend_from_slice(&[0u8; 4]);

    let mut dat_pos = 1;
    let mut offsets = Vec::new();
    for msg in &bmg.messages {
        offsets.push(dat_pos as u32);
        for part in &msg.text {
            dat_pos += part.len();
        }
        dat_pos += 1;
    }

    for (msg, offset) in bmg.messages.iter().zip(offsets) {
        out.extend_from_slice(&offset.to_be_bytes());
        out.extend_from_slice(&msg.attributes);
    }

    Ok(())
}

fn write_dat1(out: &mut Vec<u8>, bmg: &Bmg) -> Result<(), String> {
    out.push(0u8);
    for msg in &bmg.messages {
        for part in &msg.text {
            out.extend_from_slice(part);
        }
        out.push(0u8);
    }
    Ok(())
}

fn write_mid1(out: &mut Vec<u8>, bmg: &Bmg) -> Result<(), String> {
    out.extend_from_slice(&(bmg.messages.len() as u16).to_be_bytes());
    out.extend_from_slice(&[0u8; 6]);

    for msg in &bmg.messages {
        let (id, subid) = msg.id;
        out.push((id >> 16) as u8);
        out.push((id >> 8) as u8);
        out.push(id as u8);
        out.push(subid);
    }

    Ok(())
}

fn write_section(out: &mut Vec<u8>, magic: &[u8; 4], data: &[u8]) -> Result<(), String> {
    out.extend_from_slice(magic);
    out.extend_from_slice(&((data.len() + 8) as u32).to_be_bytes());
    out.extend_from_slice(data);
    Ok(())
}
