mod common;

use tpmt::formats::rarc::{extract_arc_files, Rarc, RarcBuilder};

// Verifies the archive builder seeds the mandatory root directory entries.
#[test]
fn rarc_builder_starts_with_root_entries() {
    assert_eq!(RarcBuilder::new().build().file_entries.len(), 2);
}

// Verifies adding a file keeps the archive rooted where the game expects it.
#[test]
fn rarc_builder_places_files_in_the_root_directory() {
    assert_eq!(
        RarcBuilder::new().add_file("foo.txt".to_string(), b"abc".to_vec()).build().list_files()[0].0,
        "foo.txt"
    );
}

// Verifies a small archive can round-trip through Yaz0 compression and parsing.
#[test]
fn rarc_roundtrip_preserves_file_listing() {
    let rarc = RarcBuilder::new().add_file("foo.txt".to_string(), b"abc".to_vec()).build();
    let parsed = Rarc::parse(rarc.to_bytes_compressed().unwrap()).unwrap();
    assert_eq!(parsed.list_files()[0].0, "foo.txt");
}

// Verifies non-RARC bytes are rejected before header parsing begins.
#[test]
fn rarc_parse_rejects_wrong_magic() {
    assert!(Rarc::parse(b"NOPE".to_vec()).is_none());
}

// Verifies the root node path stays empty so callers can build child paths cleanly.
#[test]
fn node_path_returns_empty_for_root() {
    let rarc = Rarc {
        nodes: vec![],
        file_entries: vec![],
    };
    assert_eq!(rarc.node_path(0), "");
}

// Verifies ARC extraction writes the unpacked file to the expected output tree.
#[test]
fn extract_arc_files_writes_unpacked_contents() {
    let input = common::temp_dir("arc-input");
    let output = common::temp_dir("arc-output");
    let arc = RarcBuilder::new()
        .add_file("foo.txt".to_string(), b"abc".to_vec())
        .build()
        .to_bytes_compressed()
        .unwrap();
    std::fs::write(input.join("sample.arc"), arc).expect("write arc");
    let extracted = extract_arc_files(input.to_str().unwrap(), output.to_str().unwrap()).unwrap();
    assert_eq!(extracted[0], "foo.txt");
}

// Verifies non-directory inputs are rejected before extraction starts.
#[test]
fn extract_arc_files_rejects_non_directories() {
    let path = common::temp_file("arc-file", "bin", b"abc");
    assert!(extract_arc_files(path.to_str().unwrap(), path.to_str().unwrap()).is_err());
}
