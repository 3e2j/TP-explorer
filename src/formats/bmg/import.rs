/*
Import JSON back to BMG format.
*/

use crate::formats::bmg::parser::{Bmg, BmgMessage, BmgSection};
use crate::utils::hex_to_bytes;
use serde_json::Value;
use std::fs;

pub fn json_to_bmg(data: &Value) -> Result<Bmg, String> {
    let messages_arr = data.as_array().ok_or("JSON must be array")?;
    if messages_arr.is_empty() {
        return Err("JSON empty".to_string());
    }

    let metadata = &messages_arr[0];
    let attr_len = metadata["Attribute Length"].as_u64().unwrap_or(8) as u16;
    let unknown_mid = metadata["Unknown MID1 Value"]
        .as_str()
        .and_then(|s| u16::from_str_radix(s, 16).ok())
        .unwrap_or(0x1001);
    let encoding = metadata["Encoding"]
        .as_str()
        .unwrap_or("shift-jis")
        .to_string();

    let mut bmg_msgs = Vec::new();
    let mut extra_sections = Vec::new();

    for entry in &messages_arr[1..] {
        if let Some(section_name) = entry.get("Section").and_then(|s| s.as_str()) {
            let data_hex = entry
                .get("Data")
                .and_then(|d| d.as_str())
                .ok_or("Missing data")?;
            let data_bytes = hex_to_bytes(data_hex)?;

            let mut magic = [0u8; 4];
            let name_bytes = section_name.as_bytes();
            if name_bytes.len() == 4 {
                magic.copy_from_slice(name_bytes);
            }

            extra_sections.push(BmgSection {
                magic,
                data: data_bytes,
            });
        } else {
            bmg_msgs.push(parse_message(entry)?);
        }
    }

    Ok(Bmg {
        encoding,
        messages: bmg_msgs,
        attribute_length: attr_len,
        unknown_mid_value: unknown_mid,
        additional_sections: extra_sections,
    })
}

fn parse_message(entry: &Value) -> Result<BmgMessage, String> {
    let id_str = entry["ID"].as_str().ok_or("Missing ID")?;
    let parts: Vec<&str> = id_str.split(',').collect();
    if parts.len() != 2 {
        return Err("Invalid ID format".to_string());
    }

    let id = parts[0].trim().parse::<u32>().map_err(|_| "Bad ID")?;
    let subid = parts[1].trim().parse::<u8>().map_err(|_| "Bad subid")?;
    let attr_hex = entry["attributes"].as_str().ok_or("Missing attributes")?;
    let attributes = hex_to_bytes(attr_hex)?;

    let text_arr = entry["text"].as_array().ok_or("Missing text array")?;
    let text = format_text_lines(text_arr)?;

    Ok(BmgMessage {
        id: (id, subid),
        attributes,
        text,
    })
}

fn format_text_lines(lines: &[Value]) -> Result<Vec<Vec<u8>>, String> {
    let mut parts = Vec::new();

    for line in lines {
        let line_str = line.as_str().ok_or("Bad text line")?;

        if line_str.starts_with('{') && line_str.ends_with('}') {
            let hex = &line_str[1..line_str.len() - 1];
            parts.push(hex_to_bytes(hex)?);
        } else {
            let unescaped = line_str.replace("\\{", "{").replace("\\}", "}");
            parts.push(unescaped.into_bytes());
        }
    }

    Ok(parts)
}

pub fn read_json(path: &str) -> Result<Value, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Read error: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Parse error: {}", e))
}
