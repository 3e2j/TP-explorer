/*
Convert JSON back to BMG binary format.

The JSON format exported by bmg_to_json preserves all BMG data including:
- Message IDs (as "group, msg_id" format where msg_id is 24-bit)
- Message text (with escape sequences for control codes in {...})
- Message attributes (16 bytes when attribute_length=20)
- Additional sections (non-standard BMG sections like custom metadata)

Attributes Format (16 bytes, when attribute_length = 20):
  Byte 0:     Group (high byte, usually 0x00)
  Byte 1:     Message ID (low byte)
  Bytes 2-3:  Padding/Unused (0x00 0x00)
  Bytes 4-15: Attribute Data
    Byte 4:   Unknown (typically 0x02)
    Byte 5:   Unknown (typically 0x10)
    Byte 6:   Unknown (typically 0x01)
    Byte 7:   Unknown (usually 0x00)
    Byte 8:   Unknown (typically 0xff)
    Byte 9:   Unknown
    Byte 10:  Unknown
    Byte 11:  Unknown
    Byte 12:  Unknown
    Byte 13:  Unknown
    Byte 14:  Color/Style Flag (0x01 or 0x09 observed)
    Byte 15:  Padding (usually 0x00)

Note: Attributes are preserved as-is from the JSON export. The ID field
stores the MID1 section values (24-bit ID + 8-bit subid), which is separate
from the attribute bytes. Do not modify attributes[0:1] as they contain
important format information already correctly encoded in the JSON.
*/

use crate::formats::bmg::Bmg;
use crate::utils::hex_to_bytes;
use serde_json::Value;

use crate::formats::bmg::parser::BmgMessage;
use crate::formats::bmg::parser::BmgSection;

pub fn json_to_bmg(val: &Value, encoding: &str) -> Result<Bmg, String> {
    let arr = val.as_array().ok_or("Expected JSON array")?;
    if arr.is_empty() {
        return Err("Empty JSON".to_string());
    }

    // First element contains attribute length and unknown mid value
    let meta = &arr[0];
    let attribute_length = meta
        .get("Attribute Length")
        .and_then(|v| v.as_u64())
        .ok_or("Missing Attribute Length")? as u16;

    let unknown_mid_value = meta
        .get("Unknown MID1 Value")
        .and_then(|v| v.as_str())
        .ok_or("Missing Unknown MID1 Value")?;
    let unknown_mid_value = u16::from_str_radix(unknown_mid_value, 16)
        .map_err(|e| format!("Invalid Unknown MID1 Value hex: {}", e))?;

    let mut messages: Vec<BmgMessage> = Vec::new();
    let mut additional_sections: Vec<BmgSection> = Vec::new();

    for item in &arr[1..] {
        // Section entries have "Section" key
        if let Some(section_name) = item.get("Section").and_then(|v| v.as_str()) {
            let data_hex = item
                .get("Data")
                .and_then(|v| v.as_str())
                .ok_or("Section missing Data")?;
            let data = hex_to_bytes(data_hex)?;
            let mut magic = [0u8; 4];
            let name_bytes = section_name.as_bytes();
            for i in 0..4 {
                magic[i] = *name_bytes.get(i).unwrap_or(&0u8);
            }
            additional_sections.push(BmgSection { magic, data });
            continue;
        }

        // Otherwise it's a message
        let id_str = item
            .get("ID")
            .and_then(|v| v.as_str())
            .ok_or("Message missing ID")?;
        let parts: Vec<&str> = id_str.split(',').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid ID format: {}", id_str));
        }
        let id = parts[0]
            .trim()
            .parse::<u32>()
            .map_err(|e| format!("Invalid ID: {}", e))?;
        let subid = parts[1]
            .trim()
            .parse::<u8>()
            .map_err(|e| format!("Invalid subid: {}", e))?;

        let attributes_hex = item
            .get("attributes")
            .and_then(|v| v.as_str())
            .ok_or("Message missing attributes")?;
        let attributes = hex_to_bytes(attributes_hex)?;

        let text_arr = item
            .get("text")
            .and_then(|v| v.as_array())
            .ok_or("Message missing text array")?;

        // Reconstruct full message string by joining lines with newline
        let mut lines: Vec<String> = Vec::new();
        for l in text_arr {
            lines.push(l.as_str().unwrap_or("").to_string());
        }
        let full = lines.join("\n");

        // Default encoding (to_json doesn't store encoding)
        let encoding = "shift-jis";

        let text_parts = parse_full_message(&full, encoding)?;

        messages.push(BmgMessage {
            id: (id, subid),
            attributes,
            text: text_parts,
        });
    }

    Ok(Bmg {
        encoding: encoding.to_string(),
        messages,
        attribute_length,
        unknown_mid_value,
        additional_sections,
    })
}

fn parse_full_message(full: &str, encoding: &str) -> Result<Vec<Vec<u8>>, String> {
    let mut parts = Vec::new();
    let mut current = Vec::new();

    let mut chars = full.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Read until closing }
            let mut hex_str = String::new();
            while let Some(&c) = chars.peek() {
                if c == '}' {
                    chars.next();
                    break;
                }
                hex_str.push(c);
                chars.next();
            }
            // Flush current text
            if !current.is_empty() {
                parts.push(current);
                current = Vec::new();
            }
            // Parse hex escape
            let hex_data = hex_to_bytes(&hex_str)?;
            parts.push(hex_data);
        } else if ch == '\n' {
            // Add newline marker
            current.push(0x00);
        } else {
            // Encode character
            match encoding {
                "shift-jis" => {
                    let bytes = ch.to_string().into_bytes();
                    current.extend_from_slice(&bytes);
                }
                "latin-1" => {
                    current.push(ch as u8);
                }
                _ => {
                    current.push(ch as u8);
                }
            }
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    Ok(parts)
}
