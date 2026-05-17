mod common;

use std::collections::HashMap;
use tpmt::commands::build::{
    self,
    archive_plan::{self, ArchiveInput},
    assemble,
    compile,
    hash_check::{self, ModifiedFile},
    output,
};
use tpmt::formats::rarc::RarcBuilder;

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = sha1::Sha1::new();
    hasher.update(bytes);
    hasher.digest().to_string()
}

// Verifies unchanged files stay out of the modified set so rebuilds remain sparse.
#[test]
fn find_modified_files_ignores_unchanged_files() {
    let dir = common::temp_dir("hash-unchanged");
    std::fs::create_dir_all(dir.join("text")).expect("create dir");
    std::fs::write(
        dir.join("manifest.json"),
        format!(r#"{{"entries":{{"text/a.txt":{{"sha1":"{}"}}}}}}"#, hash_bytes(b"same")),
    )
    .expect("manifest");
    std::fs::write(dir.join("text/a.txt"), b"same").expect("write file");
    assert!(hash_check::find_modified_files(&dir).unwrap().is_empty());
}

// Verifies changed files are surfaced with their friendly path.
#[test]
fn find_modified_files_reports_changed_files() {
    let dir = common::temp_dir("hash-changed");
    std::fs::create_dir_all(dir.join("text")).expect("create dir");
    std::fs::write(dir.join("manifest.json"), r#"{"entries":{"text/a.txt":{"sha1":"0000000000000000000000000000000000000000"}}}"#).expect("manifest");
    std::fs::write(dir.join("text/a.txt"), b"edit").expect("write file");
    assert_eq!(hash_check::find_modified_files(&dir).unwrap()[0].friendly_path, "text/a.txt");
}

// Verifies consolidated messages.json compares per-source BMG hashes instead of one blob hash.
#[test]
fn find_modified_files_handles_consolidated_bmg_sources() {
    let dir = common::temp_dir("hash-consolidated");
    std::fs::create_dir_all(dir.join("text")).expect("create dir");
    std::fs::write(
        dir.join("manifest.json"),
        r#"{"archives":{"files/res/Msgus/bmgres.arc":{"zel_00.bmg":{"path":"zel_00.bmg","sha1":"0000000000000000000000000000000000000000"}}},"entries":{"text/messages.json":{"sources":[{"archive":"files/res/Msgus/bmgres.arc","path":"zel_00.bmg","sha1":"0000000000000000000000000000000000000000"}]}}}"#,
    )
    .expect("manifest");
    std::fs::write(
        dir.join("text/messages.json"),
        r#"{"sources":[{"archive":"files/res/Msgus/bmgres.arc","path":"zel_00.bmg","encoding":"shift-jis","messages":[{"message_count":1},{"ID":"1, 0","attributes":{"box_style":"standard_dialogue"},"text":["Hi"]}]}]}"#,
    )
    .expect("messages");
    assert_eq!(hash_check::find_modified_files(&dir).unwrap().len(), 1);
}

// Verifies standard JSON text files compile into BMG bytes.
#[test]
fn compile_modified_files_converts_text_json_to_bmg() {
    let path = common::temp_json_file(
        "compile-json",
        r#"[{"message_count":1},{"ID":"1, 0","attributes":{"box_style":"standard_dialogue"},"text":["Hi"]}]"#,
    );
    let modified = vec![ModifiedFile {
        friendly_path: "text/a.json".to_string(),
        mod_path: path.to_string_lossy().to_string(),
        archive: None,
        internal_path: None,
    }];
    assert_eq!(compile::compile_modified_files(&modified, std::path::Path::new("unused")).unwrap().len(), 1);
}

// Verifies consolidated messages.json is split back into the modified source archive entry.
#[test]
fn compile_modified_files_splits_consolidated_bmg_sources() {
    let path = common::temp_file(
        "compile-consolidated",
        "json",
        br#"{"sources":[{"archive":"files/res/Msgus/bmgres.arc","path":"zel_00.bmg","encoding":"shift-jis","messages":[{"message_count":1},{"ID":"1, 0","attributes":{"box_style":"standard_dialogue"},"text":["Hi"]}]}]}"#,
    );
    let modified = vec![ModifiedFile {
        friendly_path: "text/messages.json".to_string(),
        mod_path: path.to_string_lossy().to_string(),
        archive: Some("files/res/Msgus/bmgres.arc".to_string()),
        internal_path: Some("zel_00.bmg".to_string()),
    }];
    assert_eq!(compile::compile_modified_files(&modified, std::path::Path::new("unused")).unwrap()[0].mod_file.friendly_path, "res/Msgus/bmgres.arc/zel_00.bmg");
}

// Verifies archives can be rebuilt directly when the manifest proves all internal files are present.
#[test]
fn plan_archive_inputs_uses_mods_only_when_manifest_is_complete() {
    let mod_dir = common::temp_dir("archive-plan-mod");
    std::fs::write(
        mod_dir.join("manifest.json"),
        r#"{"archives":{"files/a.arc":{"foo.txt":{"path":"foo.txt","sha1":"abc"}}},"entries":{}}"#,
    )
    .expect("manifest");
    let iso_path = common::build_single_file_iso("other.txt", b"DATA");
    let compiled = vec![compile::CompiledFile {
        mod_file: ModifiedFile {
            friendly_path: "a.arc/foo.txt".to_string(),
            mod_path: "ignored".to_string(),
            archive: Some("files/a.arc".to_string()),
            internal_path: Some("foo.txt".to_string()),
        },
        compiled_bytes: b"new".to_vec(),
    }];
    assert!(matches!(archive_plan::plan_archive_inputs(&compiled, &mod_dir, &iso_path).unwrap()[0], ArchiveInput::FromModsOnly { .. }));
}

// Verifies the build rejects archive edits when the manifest lacks archive mappings.
#[test]
fn plan_archive_inputs_rejects_missing_manifest_archives() {
    let mod_dir = common::temp_dir("archive-plan-missing");
    std::fs::write(mod_dir.join("manifest.json"), r#"{"entries":{}}"#).expect("manifest");
    let iso_path = common::build_single_file_iso("other.txt", b"DATA");
    let compiled = vec![compile::CompiledFile {
        mod_file: ModifiedFile {
            friendly_path: "a.arc/foo.txt".to_string(),
            mod_path: "ignored".to_string(),
            archive: Some("files/a.arc".to_string()),
            internal_path: Some("foo.txt".to_string()),
        },
        compiled_bytes: b"new".to_vec(),
    }];
    assert!(archive_plan::plan_archive_inputs(&compiled, &mod_dir, &iso_path).is_err());
}

// Verifies modification-only archives are rebuilt directly without source bytes.
#[test]
fn assemble_archives_rebuilds_mod_only_archives() {
    let inputs = vec![ArchiveInput::FromModsOnly {
        arc_iso_path: "files/res/Stage/D_MN05/R00_00.arc".to_string(),
        modifications: HashMap::from([("foo.txt".to_string(), b"new".to_vec())]),
    }];
    assert_eq!(assemble::assemble_archives(&inputs).unwrap()[0].path, "files/res/Stage/D_MN05/R00_00.arc");
}

// Verifies source-backed archives replace the modified file while keeping archive structure intact.
#[test]
fn assemble_archives_applies_modifications_to_existing_arcs() {
    let arc_bytes = RarcBuilder::new().add_file("foo.txt".to_string(), b"old".to_vec()).build().to_bytes_compressed().unwrap();
    let inputs = vec![ArchiveInput::FromIso {
        arc_iso_path: "files/res/Stage/D_MN05/R00_00.arc".to_string(),
        modifications: HashMap::from([("foo.txt".to_string(), b"new".to_vec())]),
        arc_bytes,
    }];
    let rebuilt = assemble::assemble_archives(&inputs).unwrap();
    assert_eq!(tpmt::formats::rarc::Rarc::parse(rebuilt[0].bytes.clone()).unwrap().file_entries.iter().find(|e| e.name == "foo.txt").unwrap().data, Some(b"new".to_vec()));
}

// Verifies compiled direct files become build outputs without changing paths.
#[test]
fn collect_direct_outputs_preserves_paths() {
    let compiled = vec![compile::CompiledFile {
        mod_file: ModifiedFile {
            friendly_path: "sys/main.dol".to_string(),
            mod_path: "ignored".to_string(),
            archive: None,
            internal_path: None,
        },
        compiled_bytes: vec![1, 2, 3],
    }];
    assert_eq!(output::collect_direct_outputs(&compiled)[0].path, "sys/main.dol");
}

// Verifies writing build outputs actually creates the target file on disk.
#[test]
fn write_outputs_creates_files_on_disk() {
    let dir = common::temp_dir("build-output");
    let outputs = vec![assemble::BuildOutput {
        path: "sys/main.dol".to_string(),
        bytes: vec![1, 2, 3],
    }];
    output::write_outputs(&dir, &outputs).unwrap();
    assert!(dir.join("sys/main.dol").exists());
}

// Verifies an empty manifest exits cleanly without trying to compile or rebuild anything.
#[test]
fn build_run_returns_ok_when_nothing_changed() {
    let mod_dir = common::temp_dir("build-empty-mod");
    std::fs::write(mod_dir.join("manifest.json"), r#"{"entries":{}}"#).expect("manifest");
    let output_dir = common::temp_dir("build-empty-out");
    assert!(build::run(mod_dir.to_str().unwrap(), "ignored.iso", Some(output_dir.to_str().unwrap()), None).is_ok());
}
