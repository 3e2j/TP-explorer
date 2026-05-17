//! BMG helpers.
//!
//! This module exposes parsing and JSON conversion for Twilight Princess text
//! messages. Use `bmg_to_json` to export editable data and `json_to_bmg` to
//! compile it back into the binary format.

pub mod attributes;
pub mod from_json;
pub mod parser;
pub mod to_json;

pub use from_json::json_to_bmg;
pub use parser::Bmg;
pub use to_json::{bmg_to_json, write_json};
