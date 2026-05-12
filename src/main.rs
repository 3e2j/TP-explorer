use std::collections::HashMap;
use std::env;
use std::fs::read;

use sha2::{Digest, Sha256};

pub mod compression;

use compression::bytes;
use compression::rarc::Rarc;

type FileMap = HashMap<String, (u32, String)>;

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn full_path(rarc: &Rarc, parent_idx: usize, file_name: &str) -> String {
    let node_path = rarc.node_path(parent_idx);
    if node_path.is_empty() {
        file_name.to_string()
    } else {
        format!("{}/{}", node_path, file_name)
    }
}

fn file_hash(data: &Option<Vec<u8>>) -> String {
    data.as_deref().map_or_else(String::new, sha256_hex)
}

fn build_file_map(rarc: &Rarc) -> FileMap {
    let mut file_map = FileMap::new();
    for file_entry in &rarc.file_entries {
        if file_entry.is_dir {
            continue;
        }
        let Some(parent_idx) = file_entry.parent_node_index else {
            continue;
        };

        let path = full_path(rarc, parent_idx, &file_entry.name);
        let hash = file_hash(&file_entry.data);
        file_map.insert(path, (file_entry.data_size, hash));
    }
    file_map
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: arc_diff <arc-file> [other-arc-file]");
        return;
    }

    let path_a = &args[1];
    let raw_a = match read(path_a) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to read {}: {}", path_a, e);
            return;
        }
    };
    let rarc_a = match Rarc::parse(raw_a) {
        Some(r) => r,
        None => {
            eprintln!("{} is not a RARC or unsupported", path_a);
            return;
        }
    };

    if args.len() == 2 {
        for (p, size) in rarc_a.list_files() {
            println!("{} ({})", p, size);
        }
        return;
    }

    let path_b = &args[2];
    let raw_b = match read(path_b) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to read {}: {}", path_b, e);
            return;
        }
    };
    let rarc_b = match Rarc::parse(raw_b) {
        Some(r) => r,
        None => {
            eprintln!("{} is not a RARC or unsupported", path_b);
            return;
        }
    };

    let map_a = build_file_map(&rarc_a);
    let map_b = build_file_map(&rarc_b);

    let mut added: Vec<String> = Vec::new();
    let mut removed: Vec<String> = Vec::new();
    let mut changed: Vec<String> = Vec::new();

    for (path, (_size_a, hash_a)) in &map_a {
        if let Some((_size_b, hash_b)) = map_b.get(path) {
            if hash_a != hash_b {
                changed.push(path.clone());
            }
        } else {
            removed.push(path.clone());
        }
    }
    for path in map_b.keys() {
        if !map_a.contains_key(path) {
            added.push(path.clone());
        }
    }

    if !added.is_empty() {
        println!("Added:");
        for a in &added {
            println!("  {}", a);
        }
    }
    if !removed.is_empty() {
        println!("Removed:");
        for r in &removed {
            println!("  {}", r);
        }
    }
    if !changed.is_empty() {
        println!("Changed:");
        for c in &changed {
            println!("  {}", c);
        }
    }
    if added.is_empty() && removed.is_empty() && changed.is_empty() {
        println!("No differences detected.");
    }
}
