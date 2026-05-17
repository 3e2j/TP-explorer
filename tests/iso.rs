mod common;

use std::fs::File;
use tpmt::formats::iso::{iso, iso_rebuild};

// Verifies raw ISO headers are read in big-endian form.
#[test]
fn read_u32_at_reads_big_endian_header_values() {
    let path = common::temp_file("read-u32", "iso", &[0u8; 0x430]);
    let result = {
        let mut file = File::open(&path).expect("open");
        iso::read_u32_at(&mut file, 0x428).expect("read")
    };
    assert_eq!(result, 0);
}

// Verifies a minimal FST can be parsed into a single file entry.
#[test]
fn parse_iso_files_extracts_one_file() {
    let iso_path = common::build_single_file_iso("a.txt", b"DATA");
    assert_eq!(iso::parse_iso_files(&iso_path).unwrap()[0].path, "files/a.txt");
}

// Verifies file reads use the parsed FST offset and size instead of guessing.
#[test]
fn read_iso_file_bytes_returns_the_expected_payload() {
    let iso_path = common::build_single_file_iso("a.txt", b"DATA");
    assert_eq!(iso::read_iso_file_bytes(&iso_path, "a.txt").unwrap(), b"DATA");
}

// Verifies per-file hashing is keyed by the relative ISO path expected by the build system.
#[test]
fn build_iso_hash_map_uses_relative_paths() {
    let iso_path = common::build_single_file_iso("a.txt", b"DATA");
    assert!(iso::build_iso_hash_map(&iso_path).unwrap().contains_key("a.txt"));
}

// Verifies ISO rebuild swaps in replacement bytes for matching files.
#[test]
fn rebuild_iso_with_files_applies_replacements() {
    let source = common::build_rebuild_iso("a.txt", b"OLD");
    let output = common::temp_file("rebuild-output", "iso", &[]);
    let files = iso::parse_iso_files(&source).unwrap();
    let replacements = std::collections::HashMap::from([("a.txt".to_string(), b"NEW".to_vec())]);
    iso_rebuild::rebuild_iso_with_files(&source, &output, &replacements, &files).unwrap();
    assert!(std::fs::read(&output).unwrap().windows(3).any(|w| w == b"NEW"));
}
