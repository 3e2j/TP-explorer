//! RARC archive parsing, building, and extraction helpers.
//!
//! `Rarc` represents a parsed archive, `RarcBuilder` creates one from scratch,
//! and `extract_arc_files` unpacks `.arc` files to disk.

pub mod builder;
pub mod extractor;
pub mod parser;

pub use builder::RarcBuilder;
pub use extractor::extract_arc_files;
pub use parser::{FileEntry, Node, Rarc};
