use crate::formats::bmg::Bmg;
use crate::utils::{hex_to_bytes};
use serde_json::Value;

use crate::formats::bmg::parser::BmgMessage;
use crate::formats::bmg::parser::BmgSection;

pub fn json_to_bmg(val: &Value) -> Result<Bmg, String> {
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
        let id = parts[0].trim().parse::<u32>().map_err(|e| format!("Invalid ID: {}", e))?;
        let subid = parts[1].trim().parse::<u8>().map_err(|e| format!("Invalid subid: {}", e))?;

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

        messages.push(BmgMessage { id: (id, subid), attributes, text: text_parts });
    }

    Ok(Bmg {
        encoding: "shift-jis".to_string(),
        messages,
        attribute_length,
        unknown_mid_value,
        additional_sections,
    })
}

fn parse_full_message(full: &str, encoding: &str) -> Result<Vec<Vec<u8>>, String> {
    use crate::utils::hex_to_bytes;

    let mut parts: Vec<Vec<u8>> = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    let s = full;
    let bytes = s.as_bytes();
    while i < s.len() {
        match bytes[i] {
            b'\\' => {
                if i + 1 < s.len() {
                    let next = bytes[i + 1];
                    if next == b'{' || next == b'}' {
                        cur.push(next as char);
                        i += 2;
                        continue;
                    }
                }
                cur.push('\\');
                i += 1;
            }
            b'{' => {
                // flush current
                if !cur.is_empty() {
                    parts.push(encode_text(&cur, encoding)?);
                    cur.clear();
                }
                // find closing brace
                if let Some(rel) = s[i + 1..].find('}') {
                    let end = i + 1 + rel;
                    let hex = &s[i + 1..end];
                    let bin = hex_to_bytes(hex)?;
                    parts.push(bin);
                    i = end + 1;
                } else {
                    return Err("Unterminated { in text".to_string());
                }
            }
            _ => {
                // consume until next special
                let subs = &s[i..];
                let next_pos = subs.find(|c| c == '\\' || c == '{').unwrap_or(subs.len());
                cur.push_str(&subs[..next_pos]);
                i += next_pos;
            }
        }
    }
    if !cur.is_empty() {
        parts.push(encode_text(&cur, encoding)?);
    }
    if parts.is_empty() {
        Ok(vec![vec![]])
    } else {
        Ok(parts)
    }
}

fn encode_text(s: &str, encoding: &str) -> Result<Vec<u8>, String> {
    if encoding == "latin-1" {
        let mut out = Vec::new();
        for ch in s.chars() {
            let code = ch as u32;
            if code > 0xff {
                out.push(b'?');
            } else {
                out.push(code as u8);
            }
        }
        Ok(out)
    } else {
        Ok(s.as_bytes().to_vec())
    }
}
