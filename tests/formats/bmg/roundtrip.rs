use serde_json::json;
use serde_json::Value;

#[test]
fn bmg_json_roundtrip_custom() {
    // Build a small JSON representation matching bmg_to_json output
    let v: Value = json!([
        { "Attribute Length": 6, "Unknown MID1 Value": "1001" },
        { "ID": "1, 0", "index": "0x0", "attributes": "0a0b", "text": ["My boy!"] },
        { "ID": "2, 1", "index": "0x1", "attributes": "00ff", "text": ["This peace is what all true {1a0a77617272696f7273} strive for!"] },
        { "Section": "ABCD", "Data": "020f0ab2c3" }
    ]);

    // Convert JSON -> BMG, bytes/parse, BMG -> JSON; compare
    let bmg = arc_diff::formats::bmg::from_json::json_to_bmg(&v).expect("json_to_bmg");
    let bytes = bmg.to_bytes().expect("to_bytes");
    let parsed = arc_diff::formats::bmg::parser::Bmg::parse(&bytes).expect("parse bytes");
    let v2 = arc_diff::formats::bmg::to_json::bmg_to_json(&parsed).expect("bmg_to_json");

    assert_eq!(v, v2, "Roundtrip JSON does not match for custom test");
}
