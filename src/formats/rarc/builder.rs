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
    ///
    /// # Examples
    ///
    /// ```
    /// use tpmt::formats::rarc::RarcBuilder;
    /// let rarc = RarcBuilder::new().add_file("foo.txt".to_string(), b"abc".to_vec()).build();
    /// assert_eq!(rarc.list_files()[0].0, "foo.txt");
    /// ```
    /// Creates the ROOT node and immediately adds the mandatory "." and
    /// ".." directory entries.
    pub fn new() -> Self {
        let root_node = Node {
            type_str: "ROOT".to_string(),
            name_offset: 0,
            name: "archive".to_string(),
            num_files: 0,
            first_file_index: 0,
            dir_entry_index: None,
        };

        let dot_entry = FileEntry {
            name: ".".to_string(),
            is_dir: true,
            node_index_for_dir: Some(0),
            parent_node_index: Some(0),
            data: None,
            data_size: 0x10,
        };

        let dotdot_entry = FileEntry {
            name: "..".to_string(),
            is_dir: true,
            node_index_for_dir: Some(0xFFFF_FFFF),
            parent_node_index: Some(0),
            data: None,
            data_size: 0x10,
        };

        let mut builder = RarcBuilder {
            nodes: vec![root_node],
            file_entries: vec![dot_entry, dotdot_entry],
        };
        builder.nodes[0].num_files = 2;
        builder
    }

    /// Add a file to the root directory.
    pub fn add_file(mut self, name: String, data: Vec<u8>) -> Self {
        let data_size = data.len() as u32;

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

    /// Build the final RARC archive.
    pub fn build(self) -> Rarc {
        Rarc {
            nodes: self.nodes,
            file_entries: self.file_entries,
        }
    }
}
