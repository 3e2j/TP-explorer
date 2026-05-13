/*
Utility to construct RARC archives from scratch.
Useful for testing and programmatic archive creation.
*/

use crate::formats::rarc::parser::{FileEntry, Node, Rarc};

/// Builder for creating RARC archives from scratch
pub struct RarcBuilder {
    nodes: Vec<Node>,
    file_entries: Vec<FileEntry>,
}

impl RarcBuilder {
    /// Create a new builder with a root node.
    /// Mirrors Python's add_root_directory(): creates the ROOT node and
    /// immediately adds the mandatory "." and ".." directory entries.
    pub fn new() -> Self {
        let root_node = Node {
            type_str: "ROOT".to_string(),
            name_offset: 0,
            name: "archive".to_string(), // Python default root name
            num_files: 0,
            first_file_index: 0,
            dir_entry_index: None,
        };

        let dot_entry = FileEntry {
            name: ".".to_string(),
            is_dir: true,
            node_index_for_dir: Some(0), // points back to root
            parent_node_index: Some(0),
            data: None,
            data_size: 0x10,
        };

        let dotdot_entry = FileEntry {
            name: "..".to_string(),
            is_dir: true,
            node_index_for_dir: Some(0xFFFF_FFFF), // no parent (Python: node=None → 0xFFFFFFFF)
            parent_node_index: Some(0),
            data: None,
            data_size: 0x10,
        };

        let mut builder = RarcBuilder {
            nodes: vec![root_node],
            file_entries: vec![dot_entry, dotdot_entry],
        };
        // Root node owns both dot entries
        builder.nodes[0].num_files = 2;
        builder
    }

    /// Add a file to the root directory.
    /// Python's regenerate_files_list_for_node() moves "." and ".." to the
    /// *end* of each node's file list, so real files come before them.
    pub fn add_file(mut self, name: String, data: Vec<u8>) -> Self {
        let data_size = data.len() as u32;

        // Insert before the trailing . and .. entries (last 2 slots)
        let insert_pos = self.file_entries.len().saturating_sub(2);
        self.file_entries.insert(
            insert_pos,
            FileEntry {
                name,
                is_dir: false,
                node_index_for_dir: None,
                parent_node_index: Some(0),
                data: Some(data),
                data_size,
            },
        );

        self.nodes[0].num_files += 1;
        self
    }

    /// Build the final RARC archive
    pub fn build(self) -> Rarc {
        Rarc {
            nodes: self.nodes,
            file_entries: self.file_entries,
        }
    }
}
