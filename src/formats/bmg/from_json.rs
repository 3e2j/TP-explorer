/*
Convert JSON back to BMG binary format.

Attributes Format (16 bytes, when attribute_length = 20):
    Byte 1:     Group (high byte)
    Byte 2:     Message ID (low byte)
    Bytes 3-4:  event_label_id (triggers a save?)
    Byte 5:   SE Speaker
    Byte 6:   Text box display Style
    Byte 7:   Text box printing Style (slow, fast, fade-in, etc...)
    Byte 8:   Text box position
    Byte 9:   Unknown flag (some kind of item id?)
    Byte 10:  Line arrange (0 centered, 1 left/start pos the "box natural")
    Byte 11:  SE Mood
    Byte 12:  Camera ID
    Byte 13:  Base animation
    Byte 14:  Face animation
    Byte 15:  Unknown flag
    Byte 16:  Padding
*/

use crate::formats::bmg::attributes::encode_attributes;
use crate::formats::bmg::Bmg;
use crate::utils::hex_to_bytes;
use serde_json::Value;

use crate::formats::bmg::parser::BmgMessage;
use crate::formats::bmg::parser::BmgSection;

/// Converts the editable BMG JSON form back into a binary BMG structure.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use tpmt::formats::bmg::from_json::json_to_bmg;
///
/// let json = json!([
///   {"message_count": 1},
///   {"ID": "1, 0", "attributes": "00000000000000000000000000000000", "text": ["Hello"]}
/// ]);
/// assert!(json_to_bmg(&json, "shift-jis").is_ok());
/// ```
pub fn json_to_bmg(val: &Value, encoding: &str) -> Result<Bmg, String> {
    let arr = val.as_array().ok_or("Expected JSON array")?;
    if arr.is_empty() {
        return Err("Empty JSON".to_string());
    }

    // First element stores MID1 metadata we expose in JSON.
    let meta = &arr[0];
    let expected_message_count = meta
        .get("message_count")
        .and_then(|v| v.as_u64())
        .ok_or("Missing message_count")?;

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

        let mut attributes = match item.get("attributes") {
            Some(Value::Object(_)) => {
                encode_attributes(item.get("attributes").ok_or("Message missing attributes")?)?
            }
            Some(Value::String(attributes_hex)) => hex_to_bytes(attributes_hex)?,
            Some(_) => return Err("Message attributes must be an object or hex string".to_string()),
            None => return Err("Message missing attributes".to_string()),
        };
        if attributes.len() != 16 {
            return Err(format!(
                "Message attributes must be exactly 16 bytes, got {}",
                attributes.len()
            ));
        }
        let group = u8::try_from(id).map_err(|_| {
            format!(
                "ID group component must fit in one byte for attributes[0], got {}",
                id
            )
        })?;
        attributes[0] = group;
        attributes[1] = subid;

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

        let text_parts = parse_full_message(&full, encoding)?;

        messages.push(BmgMessage {
            id: (id, subid),
            attributes,
            text: text_parts,
        });
    }

    if expected_message_count != messages.len() as u64 {
        return Err(format!(
            "message_count mismatch: metadata says {}, parsed {}",
            expected_message_count,
            messages.len()
        ));
    }

    Ok(Bmg {
        encoding: encoding.to_string(),
        messages,
        attribute_length: 20,
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
            // Add newline marker (ASCII)
            current.push(0x0A);
        } else {
            // Encode character
            match encoding {
                "shift-jis" => {
                    let bytes = ch.to_string().into_bytes();
                    current.extend_from_slice(&bytes);
                }
                "windows-1252" | "latin-1" => {
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
