#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

fn unique_suffix() -> usize {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn temp_dir(prefix: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "tpmt-{prefix}-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

pub fn temp_file(prefix: &str, ext: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "tpmt-{prefix}-{}-{}.{}",
        std::process::id(),
        unique_suffix(),
        ext
    ));
    let _ = fs::remove_file(&path);
    fs::write(&path, bytes).expect("write temp file");
    path
}

pub fn temp_json_file(prefix: &str, json: &str) -> PathBuf {
    temp_file(prefix, "json", json.as_bytes())
}

pub fn build_single_file_iso(file_name: &str, file_bytes: &[u8]) -> PathBuf {
    let fst_offset = 0x1000usize;
    let file_offset = 0x1200usize;
    let name_bytes = format!("{file_name}\0").into_bytes();
    let fst_size = 24 + name_bytes.len();
    let mut bytes = vec![0u8; file_offset + file_bytes.len()];

    bytes[0x424..0x428].copy_from_slice(&(fst_offset as u32).to_be_bytes());
    bytes[0x428..0x42C].copy_from_slice(&(fst_size as u32).to_be_bytes());

    bytes[fst_offset..fst_offset + 12].copy_from_slice(&[
        0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x02,
    ]);
    bytes[fst_offset + 12..fst_offset + 24].copy_from_slice(&[
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x12, 0x00,
        ((file_bytes.len() as u32) >> 24) as u8,
        ((file_bytes.len() as u32) >> 16) as u8,
        ((file_bytes.len() as u32) >> 8) as u8,
        (file_bytes.len() as u32) as u8,
    ]);
    bytes[fst_offset + 24..fst_offset + 24 + name_bytes.len()].copy_from_slice(&name_bytes);
    bytes[file_offset..file_offset + file_bytes.len()].copy_from_slice(file_bytes);

    temp_file("iso", "iso", &bytes)
}

pub fn build_rebuild_iso(file_name: &str, file_bytes: &[u8]) -> PathBuf {
    let dol_offset = 0x2460usize;
    let fst_offset = 0x2800usize;
    let file_offset = 0x2A00usize;
    let name_bytes = format!("{file_name}\0").into_bytes();
    let fst_size = 24 + name_bytes.len();
    let mut bytes = vec![0u8; file_offset + file_bytes.len()];

    bytes[0x420..0x424].copy_from_slice(&(dol_offset as u32).to_be_bytes());
    bytes[0x424..0x428].copy_from_slice(&(fst_offset as u32).to_be_bytes());
    bytes[0x428..0x42C].copy_from_slice(&(fst_size as u32).to_be_bytes());
    bytes[0x42C..0x430].copy_from_slice(&(fst_size as u32).to_be_bytes());

    bytes[fst_offset..fst_offset + 12].copy_from_slice(&[
        0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x02,
    ]);
    bytes[fst_offset + 12..fst_offset + 24].copy_from_slice(&[
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x12, 0x00,
        ((file_bytes.len() as u32) >> 24) as u8,
        ((file_bytes.len() as u32) >> 16) as u8,
        ((file_bytes.len() as u32) >> 8) as u8,
        (file_bytes.len() as u32) as u8,
    ]);
    bytes[fst_offset + 24..fst_offset + 24 + name_bytes.len()].copy_from_slice(&name_bytes);
    bytes[file_offset..file_offset + file_bytes.len()].copy_from_slice(file_bytes);

    temp_file("rebuild-iso", "iso", &bytes)
}
