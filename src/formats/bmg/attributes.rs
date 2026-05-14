use serde_json::{json, Map, Value};

const BOX_STYLES: &[(&str, u8)] = &[
    ("standard_dialogue", 0x00),
    ("no_background", 0x01),
    ("fullscreen_sign_forced_instant", 0x02),
    ("no_background_voiceless", 0x05),
    ("no_background_centered_credits", 0x07),
    ("standard_with_glow_effect", 0x08),
    ("get_item_box", 0x09),
    ("item_name_or_description", 0x0B),
    ("header_top_left_area_name", 0x0C),
    ("midna_dialogue_blue_text", 0x0D),
    ("animal_wolf_link_green_text", 0x0E),
    ("instant_fade_in_non_modal", 0x0F),
    ("system_message", 0x10),
    ("wolf_song_interface", 0x11),
    ("boss_name_title_card", 0x13),
];

const PRINT_STYLES: &[(&str, u8)] = &[
    ("typewriter_skippable", 0x00),
    ("forced_instant_skippable", 0x01),
    ("typewriter_no_skip", 0x02),
    ("forced_instant_first_box_fades", 0x03),
    ("typewriter_no_instant_tag_no_skip", 0x04),
    ("typewriter_slow_5x", 0x05),
    ("typewriter_skippable_alt", 0x06),
    ("typewriter_ui_emphasis", 0x07),
    ("forced_instant_fade_credits", 0x09),
];

const BOX_POSITIONS: &[(&str, u8)] = &[
    ("bottom", 0x00),
    ("top", 0x01),
    ("center", 0x02),
    ("bottom_alt_mayor_messages", 0x03),
    ("bottom_alt_system_voiceless", 0x05),
];

const ATTR_LEN: usize = 16;

const DEFAULT_EVENT_LABEL_ID: u16 = 0x0000;
const DEFAULT_SPEAKER: u8 = 0x00;
const DEFAULT_BOX_STYLE: u8 = 0x00;
const DEFAULT_PRINT_STYLE: u8 = 0x00;
const DEFAULT_BOX_POSITION: u8 = 0x00;
const DEFAULT_ITEM_ID: u8 = 0xFF;
const DEFAULT_LINE_ARRANGE: u8 = 0x00;
const DEFAULT_SOUND_MOOD: u8 = 0x00;
const DEFAULT_CAMERA_ID: u8 = 0x00;
const DEFAULT_ANIM_BASE: u8 = 0x00;
const DEFAULT_ANIM_FACE: u8 = 0x00;
const DEFAULT_FLAG: u8 = 0x04;

fn decode_with_map(val: u8, map: &[(&str, u8)]) -> Value {
    map.iter()
        .find(|entry| entry.1 == val)
        .map(|entry| json!(entry.0))
        .unwrap_or_else(|| json!(val))
}

fn encode_from_map(val: &Value, map: &[(&str, u8)]) -> Result<u8, String> {
    match val {
        Value::String(s) => map
            .iter()
            .find(|entry| entry.0 == s)
            .map(|entry| entry.1)
            .ok_or_else(|| format!("Unknown value: {}", s)),
        Value::Number(n) => n
            .as_u64()
            .map(|v| v as u8)
            .ok_or_else(|| "Invalid number".to_string()),
        _ => Err("Expected string or number".to_string()),
    }
}

pub fn decode_attributes(attrs: &[u8]) -> Result<Value, String> {
    if attrs.len() != ATTR_LEN {
        return Err(format!(
            "Expected {} attribute bytes, got {}",
            ATTR_LEN,
            attrs.len()
        ));
    }
    let get = |offset: usize| attrs[offset];
    let mut obj = Map::new();

    if get(0x05) != DEFAULT_BOX_STYLE {
        obj.insert("box_style".into(), decode_with_map(get(0x05), BOX_STYLES));
    }
    if get(0x06) != DEFAULT_PRINT_STYLE {
        obj.insert(
            "print_style".into(),
            decode_with_map(get(0x06), PRINT_STYLES),
        );
    }
    if get(0x07) != DEFAULT_BOX_POSITION {
        obj.insert(
            "box_position".into(),
            decode_with_map(get(0x07), BOX_POSITIONS),
        );
    }

    let event_label_id = ((get(0x02) as u16) << 8) | (get(0x03) as u16);
    if event_label_id != DEFAULT_EVENT_LABEL_ID {
        obj.insert("event_label_id".into(), json!(event_label_id));
    }

    if get(0x04) != DEFAULT_SPEAKER {
        obj.insert("speaker".into(), json!(get(0x04)));
    }
    if get(0x08) != DEFAULT_ITEM_ID {
        obj.insert("item_id".into(), json!(get(0x08)));
    }
    if get(0x09) != DEFAULT_LINE_ARRANGE {
        obj.insert("line_arrange".into(), json!(get(0x09)));
    }
    if get(0x0A) != DEFAULT_SOUND_MOOD {
        obj.insert("sound_mood".into(), json!(get(0x0A)));
    }
    if get(0x0B) != DEFAULT_CAMERA_ID {
        obj.insert("camera_id".into(), json!(get(0x0B)));
    }
    if get(0x0C) != DEFAULT_ANIM_BASE {
        obj.insert("anim_base".into(), json!(get(0x0C)));
    }
    if get(0x0D) != DEFAULT_ANIM_FACE {
        obj.insert("anim_face".into(), json!(get(0x0D)));
    }
    if get(0x0E) != DEFAULT_FLAG {
        obj.insert("flag".into(), json!(get(0x0E)));
    }

    Ok(Value::Object(obj))
}

// Encode string/int values into a u8 value
pub fn encode_attributes(attr: &Value) -> Result<Vec<u8>, String> {
    let attr = attr
        .as_object()
        .ok_or("Attributes must be a JSON object".to_string())?;
    let mut out = vec![0u8; 16];
    let get_u8 = |key: &str, default: u8| -> Result<u8, String> {
        match attr.get(key) {
            Some(v) => v
                .as_u64()
                .map(|n| n as u8)
                .ok_or_else(|| format!("Invalid numeric attribute field: {}", key)),
            None => Ok(default),
        }
    };
    let get_mapped_u8 = |key: &str, map: &[(&str, u8)], default: u8| -> Result<u8, String> {
        match attr.get(key) {
            Some(v) => encode_from_map(v, map).map_err(|e| format!("Invalid {}: {}", key, e)),
            None => Ok(default),
        }
    };
    let event_label_id = attr
        .get("event_label_id")
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_EVENT_LABEL_ID as u64) as u16;

    out[0] = 0;
    out[1] = 0;
    out[2] = (event_label_id >> 8) as u8;
    out[3] = event_label_id as u8;
    out[4] = get_u8("speaker", DEFAULT_SPEAKER)?;
    out[5] = get_mapped_u8("box_style", BOX_STYLES, DEFAULT_BOX_STYLE)?;
    out[6] = get_mapped_u8("print_style", PRINT_STYLES, DEFAULT_PRINT_STYLE)?;
    out[7] = get_mapped_u8("box_position", BOX_POSITIONS, DEFAULT_BOX_POSITION)?;
    out[8] = get_u8("item_id", DEFAULT_ITEM_ID)?;
    out[9] = get_u8("line_arrange", DEFAULT_LINE_ARRANGE)?;
    out[10] = get_u8("sound_mood", DEFAULT_SOUND_MOOD)?;
    out[11] = get_u8("camera_id", DEFAULT_CAMERA_ID)?;
    out[12] = get_u8("anim_base", DEFAULT_ANIM_BASE)?;
    out[13] = get_u8("anim_face", DEFAULT_ANIM_FACE)?;
    out[14] = get_u8("flag", DEFAULT_FLAG)?;
    out[15] = 0;

    Ok(out)
}
