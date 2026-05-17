//! ISO parsing and rebuild helpers.
//!
//! `iso_read` reads the GameCube filesystem table and file contents, while
//! `iso_rebuild` writes replacements back into a patched disc image.

pub mod iso_read;
pub mod iso_rebuild;
