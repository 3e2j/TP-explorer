mod common;

use serde_json::json;
use tpmt::formats::bmg::{
    attributes::{decode_attributes, encode_attributes},
    from_json::json_to_bmg,
    parser::{Bmg, BmgMessage},
    to_json::{bmg_to_json, write_json},
};

fn sample_bmg() -> Bmg {
    Bmg {
        encoding: "shift-jis".to_string(),
        messages: vec![BmgMessage {
            id: (1, 0),
            attributes: vec![0u8; 16],
            text: vec![b"Hello".to_vec()],
        }],
        attribute_length: 20,
        additional_sections: vec![],
    }
}

// Verifies default attribute bytes still decode into the JSON fields the parser preserves.
#[test]
fn decode_attributes_preserves_default_item_fields() {
    assert_eq!(decode_attributes(&[0u8; 16]).unwrap()["item_id"], 0);
}

// Verifies named attribute values decode to the documented human-readable labels.
#[test]
fn decode_attributes_maps_named_values() {
    let mut attrs = [0u8; 16];
    attrs[0x05] = 0x0D;
    assert_eq!(decode_attributes(&attrs).unwrap()["box_style"], "midna_dialogue_blue_text");
}

// Verifies encoding a named attribute writes the correct byte value back out.
#[test]
fn encode_attributes_writes_named_values() {
    assert_eq!(encode_attributes(&json!({"box_style": "boss_name_title_card"})).unwrap()[0x05], 0x13);
}

// Verifies invalid attribute names fail instead of silently falling back.
#[test]
fn encode_attributes_rejects_unknown_named_values() {
    assert!(encode_attributes(&json!({"box_style": "not_real"})).is_err());
}

// Verifies non-object attributes are rejected because the schema is field-based.
#[test]
fn encode_attributes_rejects_non_object_input() {
    assert!(encode_attributes(&json!(null)).is_err());
}

// Verifies a single JSON message compiles into one BMG message.
#[test]
fn json_to_bmg_parses_a_single_message() {
    let json = json!([
        {"message_count": 1},
        {
            "ID": "1, 0",
            "attributes": {"box_style": "standard_dialogue"},
            "text": ["Hello"]
        }
    ]);
    assert_eq!(json_to_bmg(&json, "shift-jis").unwrap().messages.len(), 1);
}

// Verifies consolidated extra sections survive JSON-to-BMG conversion.
#[test]
fn json_to_bmg_preserves_section_data() {
    let json = json!([
        {"message_count": 0},
        {"Section": "ABCD", "Data": "0a0b"}
    ]);
    assert_eq!(json_to_bmg(&json, "shift-jis").unwrap().additional_sections.len(), 1);
}

// Verifies message_count mismatches are reported instead of being ignored.
#[test]
fn json_to_bmg_rejects_message_count_mismatch() {
    let json = json!([
        {"message_count": 2},
        {
            "ID": "1, 0",
            "attributes": "00000000000000000000000000000000",
            "text": ["Hello"]
        }
    ]);
    assert!(json_to_bmg(&json, "shift-jis").is_err());
}

// Verifies malformed IDs fail fast so message coordinates stay unambiguous.
#[test]
fn json_to_bmg_rejects_invalid_id_format() {
    let json = json!([
        {"message_count": 1},
        {
            "ID": "1",
            "attributes": "00000000000000000000000000000000",
            "text": [""]
        }
    ]);
    assert!(json_to_bmg(&json, "shift-jis").is_err());
}

// Verifies BMG messages serialize back into the documented JSON array shape.
#[test]
fn bmg_to_json_emits_a_message_array() {
    assert_eq!(bmg_to_json(&sample_bmg()).unwrap()[1]["ID"], "1, 0");
}

// Verifies escape sequences remain inline as hex braces during export.
#[test]
fn bmg_to_json_formats_escape_sequences_inline() {
    let bmg = Bmg {
        encoding: "shift-jis".to_string(),
        messages: vec![BmgMessage {
            id: (1, 0),
            attributes: vec![0u8; 16],
            text: vec![vec![0x1A, 0x02, 0x00]],
        }],
        attribute_length: 20,
        additional_sections: vec![],
    };
    assert_eq!(bmg_to_json(&bmg).unwrap()[1]["text"][0], "{1a0200}");
}

// Verifies the alternate single-byte decoder keeps legacy text readable.
#[test]
fn bmg_to_json_decodes_windows_1252_text() {
    let bmg = Bmg {
        encoding: "windows-1252".to_string(),
        messages: vec![BmgMessage {
            id: (1, 0),
            attributes: vec![0u8; 16],
            text: vec![vec![0xE9]],
        }],
        attribute_length: 20,
        additional_sections: vec![],
    };
    assert_eq!(bmg_to_json(&bmg).unwrap()[1]["text"][0], "é");
}

// Verifies a parsed BMG can be serialized and parsed again without losing messages.
#[test]
fn bmg_roundtrip_preserves_message_count() {
    assert_eq!(Bmg::parse(&sample_bmg().to_bytes().unwrap()).unwrap().messages.len(), 1);
}

// Verifies invalid magic is rejected before any section parsing happens.
#[test]
fn bmg_parse_rejects_invalid_magic() {
    assert!(Bmg::parse(b"bad").is_err());
}

// Verifies write_json persists the pretty-printed output to disk.
#[test]
fn write_json_writes_pretty_output() {
    let path = common::temp_file("bmg-json", "json", b"");
    write_json(&json!({"ok": true}), path.to_str().unwrap()).expect("write json");
    assert!(std::fs::read_to_string(path).unwrap().contains("\"ok\": true"));
}
