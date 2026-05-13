/*
Export BMG to JSON format.
*/

use crate::formats::bmg::Bmg;
use crate::utils::bytes_to_hex;
use serde_json::{json, Value};
use std::fs;

pub fn bmg_to_json(bmg: &Bmg) -> Result<Value, String> {
    let mut messages = vec![json!({
        "Attribute Length": bmg.attribute_length,
        "Unknown MID1 Value": format!("{:x}", bmg.unknown_mid_value),
    })];

    for (idx, msg) in bmg.messages.iter().enumerate() {
        let text_lines = format_text_parts(&msg.text, &bmg.encoding)?;
        messages.push(json!({
            "ID": format!("{}, {}", msg.id.0, msg.id.1),
            "index": format!("0x{:x}", idx),
            "attributes": bytes_to_hex(&msg.attributes),
            "text": text_lines,
        }));
    }

    for section in &bmg.additional_sections {
        let section_name = String::from_utf8_lossy(&section.magic).to_string();
        messages.push(json!({
            "Section": section_name,
            "Data": bytes_to_hex(&section.data),
        }));
    }

    Ok(Value::Array(messages))
}

fn format_text_parts(parts: &[Vec<u8>], encoding: &str) -> Result<Vec<String>, String> {
    // First, concatenate all parts into a single message string
    let mut full_message = String::new();

    for part in parts {
        if part.is_empty() {
            continue;
        }

        if part[0] == 0x1A {
            // Escape sequence - convert to hex format inline
            full_message.push_str(&format!("{{{}}}", bytes_to_hex(part)));
        } else {
            // Regular text - decode and add
            let text = decode_text(part, encoding)?;
            let text_escaped = text.replace("{", "\\{").replace("}", "\\}");
            full_message.push_str(&text_escaped);
        }
    }

    // Now split on newlines to create the array
    let lines: Vec<String> = full_message.split('\n').map(|s| s.to_string()).collect();

    // If we have no lines, return empty line
    if lines.is_empty() {
        Ok(vec![String::new()])
    } else {
        Ok(lines)
    }
}

fn decode_text(bytes: &[u8], encoding: &str) -> Result<String, String> {
    match encoding {
        "latin-1" => Ok(bytes.iter().map(|&b| b as char).collect()),
        _ => Ok(String::from_utf8_lossy(bytes).to_string()),
    }
}

pub fn write_json(json: &Value, path: &str) -> Result<(), String> {
    let json_str =
        serde_json::to_string_pretty(json).map_err(|e| format!("Serialize error: {}", e))?;
    fs::write(path, json_str).map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}
