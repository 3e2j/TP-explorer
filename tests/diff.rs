mod common;

use std::path::Path;
use tpmt::diff::diff_iso_files_against_folder;

// Verifies invalid ISO paths are rejected before any hashing work starts.
#[test]
fn diff_rejects_non_file_iso_paths() {
    let folder = common::temp_dir("diff-folder");
    assert!(diff_iso_files_against_folder(Path::new("missing.iso"), &folder).is_err());
}

// Verifies the diff reports changed files when the folder copy diverges from the ISO.
#[test]
fn diff_reports_changed_files() {
    let iso_path = common::build_single_file_iso("a.txt", b"DATA");
    let folder = common::temp_dir("diff-folder-changed");
    std::fs::create_dir_all(folder.join("files")).expect("create files dir");
    std::fs::write(folder.join("files/a.txt"), b"EDIT").expect("write file");
    assert!(diff_iso_files_against_folder(&iso_path, &folder).unwrap().contains("Changed:"));
}
