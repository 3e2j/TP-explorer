/*
RARC parsing utilities.

This module provides a small parser for RARC archives. The public surface is
intentionally minimal: Rarc::parse, Rarc::list_files and Rarc::node_path.
*/

// RARC magic
const MAGIC_RARC: &[u8; 4] = b"RARC";
const RARC_MAGIC_LEN: usize = 4;

// Top-level header offsets
const HEADER_DATA_OFFSET: usize = 0x20;
const MAIN_DATA_HEADER_OFFSET_FIELD: usize = 0x08;
const MAIN_FILE_DATA_LIST_OFFSET_FIELD: usize = 0x0C;

// Data header fields
const DATA_HDR_NUM_NODES_FIELD: usize = 0x00;
const DATA_HDR_NODE_LIST_OFFSET_FIELD: usize = 0x04;
const DATA_HDR_TOTAL_FILE_ENTRIES_FIELD: usize = 0x08;
const DATA_HDR_FILE_ENTRIES_LIST_OFFSET_FIELD: usize = 0x0C;
const DATA_HDR_STRING_LIST_OFFSET_FIELD: usize = 0x14;
const DATA_HDR_MIN_BYTES: usize = 0x20;

// Node entry fields
const NODE_ENTRY_SIZE: usize = 0x10;
const NODE_NAME_OFFSET_FIELD: usize = 0x04;
const NODE_NUM_FILES_FIELD: usize = 0x0A;
const NODE_FIRST_FILE_INDEX_FIELD: usize = 0x0C;

// File entry fields
const FILE_ENTRY_SIZE: usize = 0x14;
const FILE_ENTRY_NAME_HASH_FIELD: usize = 0x02;
const FILE_ENTRY_TYPE_AND_NAME_OFFSET_FIELD: usize = 0x04;
const FILE_ENTRY_DATA_OFFSET_OR_NODE_INDEX_FIELD: usize = 0x08;
const FILE_ENTRY_DATA_SIZE_FIELD: usize = 0x0C;
const FILE_ENTRY_TYPE_SHIFT: u32 = 24;
const FILE_ENTRY_NAME_OFFSET_MASK: u32 = 0x00FF_FFFF;
const FILE_ENTRY_TYPE_DIR: u8 = 0x02;

// Sentinels
const U32_SIZE: usize = 4;
const INVALID_NODE_INDEX: u32 = 0xFFFF_FFFF;
use crate::formats::compression::yaz0;
use crate::utils::{read_u16_be, read_u32_be, write_u16_be, write_u32_be};

/// Represents a directory node inside a RARC archive. Some fields are public
/// because callers may want to inspect names or indices.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Node {
    pub type_str: String,
    pub name_offset: u32,
    pub name: String,
    pub num_files: u16,
    pub first_file_index: u32,
    pub dir_entry_index: Option<usize>,
}

/// Represents a file or directory entry in the archive. Public so callers can
/// iterate entries and access names/data.
#[derive(Debug)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub node_index_for_dir: Option<u32>,
    pub parent_node_index: Option<usize>,
    pub data: Option<Vec<u8>>,
    pub data_size: u32,
}

/// Parsed archive structure with nodes and file entries.
#[derive(Debug)]
pub struct Rarc {
    pub nodes: Vec<Node>,
    pub file_entries: Vec<FileEntry>,
}

struct HeaderOffsets {
    file_data_list_offset: usize,
    num_nodes: usize,
    node_list_offset: usize,
    total_num_file_entries: usize,
    file_entries_list_offset: usize,
    string_list_offset: usize,
}

impl Rarc {
    /// Parse raw bytes into a Rarc. If bytes are Yaz0-compressed the caller
    /// expects the function to handle decompression.
    pub fn parse(mut data: Vec<u8>) -> Option<Self> {
        if data.len() < RARC_MAGIC_LEN {
            return None;
        }

        // Early handle compression to reduce nesting later.
        // If top-level compression is Yaz0, decompress using the sibling module.
        if &data[0..RARC_MAGIC_LEN] == yaz0::MAGIC_YAZ0 {
            data = match yaz0::yaz0_decompress(&data) {
                Some(d) => d,
                None => {
                    eprintln!("Failed to decompress Yaz0 archive");
                    return None;
                }
            };
        }

        if data.len() < RARC_MAGIC_LEN || &data[0..RARC_MAGIC_LEN] != MAGIC_RARC {
            return None;
        }

        let hdr = parse_header_offsets(&data)?;

        let mut nodes = parse_nodes(&data, &hdr);
        let mut file_entries = parse_file_entries(&data, &hdr);

        assign_parent_node_indices(&mut nodes, &mut file_entries);
        link_directory_entries(&mut nodes, &mut file_entries);

        Some(Rarc {
            nodes,
            file_entries,
        })
    }

    /// Return list of (path, size) for regular files in the archive.
    pub fn list_files(&self) -> Vec<(String, u32)> {
        let mut out = Vec::new();
        for fe in &self.file_entries {
            if fe.is_dir {
                continue;
            }
            if let Some(parent_idx) = fe.parent_node_index {
                let node_path = self.node_path(parent_idx);
                let full = if node_path.is_empty() {
                    fe.name.clone()
                } else {
                    format!("{}/{}", node_path, fe.name)
                };
                out.push((full, fe.data_size));
            }
        }
        out
    }

    /// Build a path for a node by following directory entries upwards.
    /// Returns empty string for root node (index 0).
    pub fn node_path(&self, node_idx: usize) -> String {
        if node_idx == 0 {
            return String::new();
        }
        let mut parts: Vec<String> = Vec::new();
        let mut curr = node_idx;
        loop {
            let node = &self.nodes[curr];
            if let Some(dir_entry_index) = node.dir_entry_index {
                let name = &self.file_entries[dir_entry_index].name;
                if name != "." && name != ".." {
                    parts.push(name.clone());
                }
                if let Some(parent_node) = self.file_entries[dir_entry_index].parent_node_index {
                    if parent_node == curr || parent_node == 0 {
                        break;
                    }
                    curr = parent_node;
                    continue;
                }
            }
            break;
        }
        parts.reverse();
        parts.join("/")
    }

    /// Serialize Rarc back to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        // Build string table first to get all string offsets
        let string_table = build_string_table(&self.nodes, &self.file_entries);

        // Calculate section sizes and offsets
        let data_header_offset = HEADER_DATA_OFFSET;
        let node_list_offset = data_header_offset + DATA_HDR_MIN_BYTES;
        let file_entries_list_offset = node_list_offset + (self.nodes.len() * NODE_ENTRY_SIZE);
        let string_list_offset =
            file_entries_list_offset + (self.file_entries.len() * FILE_ENTRY_SIZE);

        // Calculate file data offset (after all metadata)
        // Align to 32-byte boundary
        let file_data_offset = ((string_list_offset + string_table.len() + 31) / 32) * 32;

        // Build file data section and track offsets
        let mut file_data = Vec::new();
        let mut file_offsets: Vec<u32> = vec![0; self.file_entries.len()];

        for (i, entry) in self.file_entries.iter().enumerate() {
            if !entry.is_dir {
                if let Some(data) = &entry.data {
                    file_offsets[i] = file_data.len() as u32;
                    file_data.extend_from_slice(data);
                }
            }
        }

        // Calculate total size
        let total_size = file_data_offset + file_data.len();

        // Build header (first 0x20 bytes)
        let mut header = vec![0u8; HEADER_DATA_OFFSET];
        header[0..4].copy_from_slice(MAGIC_RARC);
        write_u32_be(&mut header, 0x04, total_size as u32);
        write_u32_be(&mut header, 0x08, data_header_offset as u32);
        // 0x0C: file data list offset relative to data_header_offset (matches Python)
        write_u32_be(
            &mut header,
            0x0C,
            (file_data_offset - data_header_offset) as u32,
        );
        // 0x10: total file data size
        write_u32_be(&mut header, 0x10, file_data.len() as u32);
        // 0x14: mram file data size (all files treated as MRAM here)
        write_u32_be(&mut header, 0x14, file_data.len() as u32);
        // 0x18: aram file data size (0 - no ARAM files)
        write_u32_be(&mut header, 0x18, 0u32);
        // 0x1C: unknown_1, always 0

        // Build data header (0x20 bytes to match Python's full data header)
        let mut data_header = vec![0u8; DATA_HDR_MIN_BYTES];
        write_u32_be(
            &mut data_header,
            DATA_HDR_NUM_NODES_FIELD,
            self.nodes.len() as u32,
        );
        write_u32_be(
            &mut data_header,
            DATA_HDR_NODE_LIST_OFFSET_FIELD,
            (node_list_offset - data_header_offset) as u32,
        );
        write_u32_be(
            &mut data_header,
            DATA_HDR_TOTAL_FILE_ENTRIES_FIELD,
            self.file_entries.len() as u32,
        );
        write_u32_be(
            &mut data_header,
            DATA_HDR_FILE_ENTRIES_LIST_OFFSET_FIELD,
            (file_entries_list_offset - data_header_offset) as u32,
        );
        // 0x10: string_list_size - filled after we know where file data starts
        write_u32_be(
            &mut data_header,
            DATA_HDR_STRING_LIST_OFFSET_FIELD,
            (string_list_offset - data_header_offset) as u32,
        );
        // 0x18: next_free_file_id = number of file entries (keep_synced = 1)
        let next_free_file_id = self.file_entries.len() as u16;
        data_header[0x18] = (next_free_file_id >> 8) as u8;
        data_header[0x19] = (next_free_file_id & 0xFF) as u8;
        data_header[0x1A] = 1; // keep_file_ids_synced_with_indexes
                               // 0x1B..0x1F: zero (unknown_2, unknown_3)

        // Build nodes
        let mut nodes_data = vec![0u8; self.nodes.len() * NODE_ENTRY_SIZE];
        for (i, node) in self.nodes.iter().enumerate() {
            let offset = i * NODE_ENTRY_SIZE;
            // type_str is 4 bytes, space-padded if shorter (matches Python)
            let mut type_bytes = [b' '; 4];
            let src = node.type_str.as_bytes();
            let copy_len = src.len().min(4);
            type_bytes[..copy_len].copy_from_slice(&src[..copy_len]);
            nodes_data[offset..offset + 4].copy_from_slice(&type_bytes);
            let node_name_offset = string_table.get(&node.name).ok_or("String not in table")?;
            write_u32_be(
                &mut nodes_data,
                offset + NODE_NAME_OFFSET_FIELD,
                node_name_offset as u32,
            );
            // Name hash: same algorithm used by Python's RARCNode.save_changes()
            let name_hash = rarc_name_hash(&node.name);
            write_u16_be(&mut nodes_data, offset + 0x08, name_hash);
            write_u16_be(
                &mut nodes_data,
                offset + NODE_NUM_FILES_FIELD,
                node.num_files,
            );
            write_u32_be(
                &mut nodes_data,
                offset + NODE_FIRST_FILE_INDEX_FIELD,
                node.first_file_index,
            );
        }

        // Build file entries
        let mut file_entries_data = vec![0u8; self.file_entries.len() * FILE_ENTRY_SIZE];
        for (i, entry) in self.file_entries.iter().enumerate() {
            let offset = i * FILE_ENTRY_SIZE;

            // File ID at offset 0x00 (u16): use index when ids are synced
            write_u16_be(&mut file_entries_data, offset + 0x00, i as u16);

            // Name hash at offset 0x02: Python computes the real hash
            let name_hash = rarc_name_hash(&entry.name);
            write_u16_be(
                &mut file_entries_data,
                offset + FILE_ENTRY_NAME_HASH_FIELD,
                name_hash,
            );

            // Type and name offset
            let name_offset = string_table.get(&entry.name).ok_or("String not in table")?;
            let entry_type = if entry.is_dir {
                FILE_ENTRY_TYPE_DIR as u32
            } else {
                0
            };
            let type_and_name = (entry_type << FILE_ENTRY_TYPE_SHIFT)
                | (name_offset as u32 & FILE_ENTRY_NAME_OFFSET_MASK);
            write_u32_be(
                &mut file_entries_data,
                offset + FILE_ENTRY_TYPE_AND_NAME_OFFSET_FIELD,
                type_and_name,
            );

            // Data offset/node index and size
            if entry.is_dir {
                if let Some(node_idx) = entry.node_index_for_dir {
                    write_u32_be(
                        &mut file_entries_data,
                        offset + FILE_ENTRY_DATA_OFFSET_OR_NODE_INDEX_FIELD,
                        node_idx,
                    );
                }
                // Python forces data_size = 0x10 for all directory entries
                write_u32_be(
                    &mut file_entries_data,
                    offset + FILE_ENTRY_DATA_SIZE_FIELD,
                    0x10,
                );
            } else {
                write_u32_be(
                    &mut file_entries_data,
                    offset + FILE_ENTRY_DATA_OFFSET_OR_NODE_INDEX_FIELD,
                    file_offsets[i],
                );
                write_u32_be(
                    &mut file_entries_data,
                    offset + FILE_ENTRY_DATA_SIZE_FIELD,
                    entry.data_size,
                );
            }
            // Offset 0x10: runtime data pointer, always 0 on disk (Python writes 0 here explicitly)
            write_u32_be(&mut file_entries_data, offset + 0x10, 0);
        }

        // Backfill string_list_size at data header +0x10 now that we know file_data_offset.
        // Python: self.string_list_size = self.file_data_list_offset - self.string_list_offset
        let string_list_size = file_data_offset - string_list_offset;
        write_u32_be(&mut data_header, 0x10, string_list_size as u32);

        // Assemble everything
        let mut result = header;
        result.extend_from_slice(&data_header);
        result.extend_from_slice(&nodes_data);
        result.extend_from_slice(&file_entries_data);
        result.extend_from_slice(&string_table.get_raw_bytes());

        // Pad to file_data_offset
        while result.len() < file_data_offset {
            result.push(0);
        }

        result.extend_from_slice(&file_data);
        Ok(result)
    }

    /// Serialize Rarc back to bytes and apply Yaz0 compression.
    /// This matches the format used in GameCube ISOs.
    pub fn to_bytes_compressed(&self) -> Result<Vec<u8>, String> {
        let uncompressed = self.to_bytes()?;
        yaz0::yaz0_compress(&uncompressed).ok_or("Yaz0 compression failed".to_string())
    }
}

/// Name hash algorithm used by RARC for both nodes and file entries.
/// Python: hash *= 3; hash += ord(char); hash &= 0xFFFF
fn rarc_name_hash(name: &str) -> u16 {
    let mut hash: u32 = 0;
    for c in name.chars() {
        hash = hash.wrapping_mul(3).wrapping_add(c as u32) & 0xFFFF;
    }
    hash as u16
}

fn parse_header_offsets(data: &[u8]) -> Option<HeaderOffsets> {
    if !has_bytes(data, MAIN_DATA_HEADER_OFFSET_FIELD, U32_SIZE)
        || !has_bytes(data, MAIN_FILE_DATA_LIST_OFFSET_FIELD, U32_SIZE)
    {
        return None;
    }

    let raw_data_header = read_u32_be(data, MAIN_DATA_HEADER_OFFSET_FIELD) as usize;
    let data_header_offset = if raw_data_header == 0 {
        HEADER_DATA_OFFSET
    } else {
        raw_data_header
    };

    if !has_bytes(data, data_header_offset, DATA_HDR_MIN_BYTES) {
        return None;
    }

    let file_data_list_offset = read_u32_be(data, MAIN_FILE_DATA_LIST_OFFSET_FIELD) as usize;
    let file_data_list_offset = file_data_list_offset.saturating_add(data_header_offset);

    let num_nodes = read_u32_be(data, data_header_offset + DATA_HDR_NUM_NODES_FIELD) as usize;
    let node_list_offset =
        read_u32_be(data, data_header_offset + DATA_HDR_NODE_LIST_OFFSET_FIELD) as usize;
    let node_list_offset = node_list_offset.saturating_add(data_header_offset);
    let total_num_file_entries =
        read_u32_be(data, data_header_offset + DATA_HDR_TOTAL_FILE_ENTRIES_FIELD) as usize;
    let file_entries_list_offset = read_u32_be(
        data,
        data_header_offset + DATA_HDR_FILE_ENTRIES_LIST_OFFSET_FIELD,
    ) as usize;
    let file_entries_list_offset = file_entries_list_offset.saturating_add(data_header_offset);
    let string_list_offset =
        read_u32_be(data, data_header_offset + DATA_HDR_STRING_LIST_OFFSET_FIELD) as usize;
    let string_list_offset = string_list_offset.saturating_add(data_header_offset);

    Some(HeaderOffsets {
        file_data_list_offset,
        num_nodes,
        node_list_offset,
        total_num_file_entries,
        file_entries_list_offset,
        string_list_offset,
    })
}

fn parse_nodes(data: &[u8], hdr: &HeaderOffsets) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(hdr.num_nodes);
    for i in 0..hdr.num_nodes {
        let offset = hdr.node_list_offset + i * NODE_ENTRY_SIZE;
        if offset + NODE_ENTRY_SIZE > data.len() {
            break;
        }
        let type_str = String::from_utf8_lossy(&data[offset..offset + 4]).to_string();
        let name_offset = read_u32_be(data, offset + NODE_NAME_OFFSET_FIELD);
        let name = read_cstring(&data, hdr.string_list_offset + name_offset as usize);
        let num_files = read_u16_be(data, offset + NODE_NUM_FILES_FIELD);
        let first_file_index = read_u32_be(data, offset + NODE_FIRST_FILE_INDEX_FIELD);
        nodes.push(Node {
            type_str,
            name_offset,
            name,
            num_files,
            first_file_index,
            dir_entry_index: None,
        });
    }
    nodes
}

fn parse_file_entries(data: &[u8], hdr: &HeaderOffsets) -> Vec<FileEntry> {
    let mut file_entries: Vec<FileEntry> = Vec::with_capacity(hdr.total_num_file_entries);
    for i in 0..hdr.total_num_file_entries {
        let offset = hdr.file_entries_list_offset + i * FILE_ENTRY_SIZE;
        if offset + FILE_ENTRY_SIZE > data.len() {
            break;
        }
        let _name_hash = read_u16_be(data, offset + FILE_ENTRY_NAME_HASH_FIELD);
        let type_and_name_offset =
            read_u32_be(data, offset + FILE_ENTRY_TYPE_AND_NAME_OFFSET_FIELD);
        let data_offset_or_node_index =
            read_u32_be(data, offset + FILE_ENTRY_DATA_OFFSET_OR_NODE_INDEX_FIELD);
        let data_size = read_u32_be(data, offset + FILE_ENTRY_DATA_SIZE_FIELD);

        let typ = (type_and_name_offset >> FILE_ENTRY_TYPE_SHIFT) as u8;
        let name_offset = (type_and_name_offset & FILE_ENTRY_NAME_OFFSET_MASK) as usize;
        let name = read_cstring(&data, hdr.string_list_offset + name_offset);
        let is_dir = (typ & FILE_ENTRY_TYPE_DIR) != 0;

        let (node_index_for_dir, file_data) = if is_dir {
            (Some(data_offset_or_node_index), None)
        } else {
            let abs = hdr.file_data_list_offset + data_offset_or_node_index as usize;
            let end = abs.saturating_add(data_size as usize);
            let dat = if end <= data.len() {
                Some(data[abs..end].to_vec())
            } else {
                None
            };
            (None, dat)
        };

        file_entries.push(FileEntry {
            name,
            is_dir,
            node_index_for_dir,
            parent_node_index: None,
            data: file_data,
            data_size,
        });
    }
    file_entries
}

fn assign_parent_node_indices(nodes: &mut [Node], file_entries: &mut [FileEntry]) {
    for node_idx in 0..nodes.len() {
        let node = &nodes[node_idx];
        let start = node.first_file_index as usize;
        let count = node.num_files as usize;
        for fi in start..(start + count) {
            if fi < file_entries.len() {
                file_entries[fi].parent_node_index = Some(node_idx);
            }
        }
    }
}

fn link_directory_entries(nodes: &mut [Node], file_entries: &mut [FileEntry]) {
    for (i, fe) in file_entries.iter_mut().enumerate() {
        if !fe.is_dir {
            continue;
        }
        if let Some(node_idx) = fe.node_index_for_dir {
            if (node_idx as usize) < nodes.len() && node_idx != INVALID_NODE_INDEX {
                nodes[node_idx as usize].dir_entry_index = Some(i);
            }
        }
    }
}

fn read_cstring(data: &[u8], start: usize) -> String {
    if start >= data.len() {
        return String::new();
    }
    let mut end = start;
    while end < data.len() && data[end] != 0 {
        end += 1;
    }
    String::from_utf8_lossy(&data[start..end]).to_string()
}

fn has_bytes(data: &[u8], offset: usize, len: usize) -> bool {
    offset.checked_add(len).is_some_and(|end| end <= data.len())
}

/// String table builder for arc repacking
struct StringTable {
    strings: std::collections::BTreeMap<String, usize>,
    data: Vec<u8>,
}

impl StringTable {
    fn new() -> Self {
        let mut table = StringTable {
            strings: std::collections::BTreeMap::new(),
            data: Vec::new(),
        };
        // Python always writes "." at offset 0 and ".." at offset 2 first,
        // before any node/entry names.  Games rely on these fixed positions.
        table.add("."); // offset 0: "." + NUL = 2 bytes
        table.add(".."); // offset 2: ".." + NUL = 3 bytes
                         // next_string_offset in Python starts at 5 after this.
        table
    }

    fn add(&mut self, s: &str) -> usize {
        if let Some(offset) = self.strings.get(s) {
            return *offset;
        }
        let offset = self.data.len();
        self.strings.insert(s.to_string(), offset);
        self.data.extend_from_slice(s.as_bytes());
        self.data.push(0); // null terminator
        offset
    }

    fn get(&self, s: &str) -> Option<usize> {
        self.strings.get(s).copied()
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn get_raw_bytes(&self) -> &[u8] {
        &self.data
    }
}

/// Build string table from nodes and file entries
fn build_string_table(nodes: &[Node], file_entries: &[FileEntry]) -> StringTable {
    let mut table = StringTable::new();
    for node in nodes {
        table.add(&node.name);
    }
    for entry in file_entries {
        table.add(&entry.name);
    }
    table
}
