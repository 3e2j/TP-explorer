use crate::formats::compression::gz2e;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub struct PreparedIso {
    path: PathBuf,
    is_temp_file: bool,
}

impl PreparedIso {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn cleanup(&self) -> Result<(), String> {
        if !self.is_temp_file {
            return Ok(());
        }
        std::fs::remove_file(&self.path)
            .map_err(|e| format!("Failed to remove temp ISO {}: {}", self.path.display(), e))
    }
}

/// Decompresses ISO file if GZ2E encoded.
pub fn prepare_for_export(iso_path: &Path) -> Result<PreparedIso, String> {
    let mut iso_file = File::open(iso_path).map_err(|e| format!("Failed to open ISO: {e}"))?;
    let mut magic = [0u8; 4];
    iso_file
        .read_exact(&mut magic)
        .map_err(|e| format!("Failed to read ISO header: {e}"))?;

    if !gz2e::is_gz2e(&magic) {
        return Ok(PreparedIso {
            path: iso_path.to_path_buf(),
            is_temp_file: false,
        });
    }

    let temp_path = std::env::temp_dir().join(format!("iso_export_{}.iso", std::process::id()));
    iso_file
        .seek(SeekFrom::Start(0))
        .map_err(|e| format!("Failed to seek ISO: {e}"))?;

    let mut temp_file =
        File::create(&temp_path).map_err(|e| format!("Failed to create temp file: {e}"))?;
    gz2e::decompress_gz2e(&mut iso_file, &mut temp_file)
        .map_err(|e| format!("GZ2E decompression failed: {e}"))?;

    Ok(PreparedIso {
        path: temp_path,
        is_temp_file: true,
    })
}
