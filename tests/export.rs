mod common;

use serde_json::{json, Map};
use tpmt::commands::export::{iso_source, manifest};

// Verifies direct manifest entries stay in the top-level entries map.
#[test]
fn write_manifest_keeps_direct_entries_top_level() {
    let dir = common::temp_dir("manifest-direct");
    let mut entries = Map::new();
    entries.insert("sys/main.dol".into(), json!({"iso": "sys/main.dol", "sha1": "abc"}));
    manifest::write_manifest(&dir, entries).unwrap();
    assert!(std::fs::read_to_string(dir.join("manifest.json")).unwrap().contains("\"sys/main.dol\""));
}

// Verifies archive-backed entries are hoisted into the archives section.
#[test]
fn write_manifest_hoists_archive_entries() {
    let dir = common::temp_dir("manifest-archive");
    let mut entries = Map::new();
    entries.insert(
        "stages/forest_temple/room.dzr".into(),
        json!({"archive": "files/res/Stage/D_MN05/R00_00.arc", "path": "room.dzr", "sha1": "abc"}),
    );
    manifest::write_manifest(&dir, entries).unwrap();
    assert!(std::fs::read_to_string(dir.join("manifest.json")).unwrap().contains("\"archives\""));
}

// Verifies consolidated BMG sources are hoisted into the archive lookup map.
#[test]
fn write_manifest_hoists_consolidated_bmg_sources() {
    let dir = common::temp_dir("manifest-bmg");
    let mut entries = Map::new();
    entries.insert(
        "text/messages.json".into(),
        json!({
            "sources": [
                {"archive": "files/res/Msgus/bmgres.arc", "path": "zel_00.bmg", "sha1": "abc"}
            ]
        }),
    );
    manifest::write_manifest(&dir, entries).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    assert!(manifest["archives"]["files/res/Msgus/bmgres.arc"].get("zel_00.bmg").is_some());
}

// Verifies ordinary ISOs are passed through untouched so export uses the source path directly.
#[test]
fn prepare_for_export_returns_original_path_for_plain_iso() {
    let iso_path = common::temp_file("plain-iso", "iso", b"GAME");
    let prepared = iso_source::prepare_for_export(&iso_path).unwrap();
    assert_eq!(prepared.path(), iso_path.as_path());
}

// Verifies wrapped GZ2E ISOs are copied to a temp file and cleaned up afterward.
#[test]
fn prepare_for_export_cleans_up_temp_gz2e_copy() {
    let mut bytes = vec![0u8; 0x440];
    bytes[0..4].copy_from_slice(b"GZ2E");
    bytes[4..6].copy_from_slice(b"01");
    bytes[0x420..0x424].copy_from_slice(&0x1800u32.to_be_bytes());
    bytes[0x424..0x428].copy_from_slice(&0x2000u32.to_be_bytes());
    bytes[0x428..0x42c].copy_from_slice(&0x2000u32.to_be_bytes());
    let iso_path = common::temp_file("gz2e-iso", "iso", &bytes);
    let prepared = iso_source::prepare_for_export(&iso_path).unwrap();
    let temp_path = prepared.path().to_path_buf();
    prepared.cleanup().unwrap();
    assert!(!temp_path.exists());
}
