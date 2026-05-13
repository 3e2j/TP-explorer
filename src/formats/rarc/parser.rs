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
const DATA_HDR_MIN_BYTES: usize = 0x18;

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
use crate::utils::{read_u16_be, read_u32_be};

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
