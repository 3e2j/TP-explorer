use std::path::Path;

const MAGIC_BAA_START: &[u8; 4] = b"AA_<";
const MAGIC_BAA_END: &[u8; 4] = b">_AA";
const MAGIC_WSYS: &[u8; 4] = b"WSYS";
const MAGIC_WINF: &[u8; 4] = b"WINF";

#[derive(Debug, Clone)]
pub struct AwWaveInfo {
    pub index: usize,
    pub format: u8,
    pub sample_rate_bits: u32,
    pub stream_offset: u32,
    pub stream_size: u32,
    pub loop_flag_raw: i32,
    pub loop_start: i32,
    pub loop_end: i32,
    pub num_samples: i32,
}

#[derive(Debug, Clone)]
pub struct AwFileInfo {
    pub aw_name: String,
    pub bank_id: u32,
    pub flags: u32,
    pub waves: Vec<AwWaveInfo>,
}

#[derive(Debug, Clone)]
pub struct BaaArchive {
    pub aw_files: Vec<AwFileInfo>,
}

impl BaaArchive {
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        if data.len() < 4 || &data[..4] != MAGIC_BAA_START {
            return Err("Invalid BAA header".to_string());
        }

        let mut out = Vec::new();
        parse_baa_block(data, 0, data.len(), 0, &mut out)?;
        Ok(Self { aw_files: out })
    }

    pub fn find_aw_by_name(&self, aw_name: &str) -> Result<&AwFileInfo, String> {
        let mut matches = self.aw_files.iter().filter(|entry| {
            entry.aw_name == aw_name
                || Path::new(&entry.aw_name)
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy() == aw_name)
        });

        let first = matches
            .next()
            .ok_or_else(|| format!("AW not found in BAA: {aw_name}"))?;
        if matches.next().is_some() {
            return Err(format!("AW name is ambiguous in BAA: {aw_name}"));
        }
        Ok(first)
    }
}

fn parse_baa_block(
    data: &[u8],
    base_offset: usize,
    max_offset: usize,
    depth: usize,
    out_aw_files: &mut Vec<AwFileInfo>,
) -> Result<(), String> {
    if depth > 16 {
        return Err("BAA nesting too deep".to_string());
    }
    if base_offset + 4 > data.len() || &data[base_offset..base_offset + 4] != MAGIC_BAA_START {
        return Err(format!("Invalid nested BAA block at 0x{base_offset:X}"));
    }

    let mut offset = base_offset + 4;
    while offset + 4 <= max_offset && offset + 4 <= data.len() {
        let chunk = &data[offset..offset + 4];
        offset += 4;

        if chunk == MAGIC_BAA_END {
            return Ok(());
        }

        match chunk {
            b"bst " | b"bstn" | b"bsc " | b"bnk " => {
                offset = offset
                    .checked_add(8)
                    .ok_or_else(|| "BAA offset overflow".to_string())?;
            }
            b"bsft" | b"bfca" => {
                offset = offset
                    .checked_add(4)
                    .ok_or_else(|| "BAA offset overflow".to_string())?;
            }
            b"bms " => {
                offset = offset
                    .checked_add(12)
                    .ok_or_else(|| "BAA offset overflow".to_string())?;
            }
            b"ws  " => {
                let bank_id = read_u32_be(data, offset)?;
                let wsys_rel = read_u32_be(data, offset + 4)? as usize;
                let flags = read_u32_be(data, offset + 8)?;
                offset = offset
                    .checked_add(12)
                    .ok_or_else(|| "BAA offset overflow".to_string())?;

                let wsys_offset = base_offset
                    .checked_add(wsys_rel)
                    .ok_or_else(|| "WSYS offset overflow".to_string())?;
                let aw_files = parse_wsys(data, wsys_offset, bank_id, flags)?;
                out_aw_files.extend(aw_files);
            }
            b"baac" => {
                let baac_rel = read_u32_be(data, offset)? as usize;
                let baac_end_rel = read_u32_be(data, offset + 4)? as usize;
                offset = offset
                    .checked_add(8)
                    .ok_or_else(|| "BAA offset overflow".to_string())?;

                let baac_offset = base_offset
                    .checked_add(baac_rel)
                    .ok_or_else(|| "BAAC offset overflow".to_string())?;
                let baac_end = base_offset
                    .checked_add(baac_end_rel)
                    .ok_or_else(|| "BAAC end overflow".to_string())?;
                if baac_end > data.len() || baac_offset >= baac_end {
                    return Err("Invalid BAAC range".to_string());
                }

                let entries = read_u32_be(data, baac_offset)? as usize;
                let mut list_offset = baac_offset + 4;
                for _ in 0..entries {
                    let child_rel = read_u32_be(data, list_offset)? as usize;
                    list_offset = list_offset
                        .checked_add(4)
                        .ok_or_else(|| "BAAC list overflow".to_string())?;
                    let child_offset = baac_offset
                        .checked_add(child_rel)
                        .ok_or_else(|| "Nested BAA offset overflow".to_string())?;
                    parse_baa_block(data, child_offset, baac_end, depth + 1, out_aw_files)?;
                }
            }
            _ => {
                return Err(format!(
                    "Unsupported BAA chunk {:?} at 0x{:X}",
                    String::from_utf8_lossy(chunk),
                    offset - 4
                ));
            }
        }

        if offset > max_offset {
            return Err("BAA command stream overflow".to_string());
        }
    }

    Err("BAA block ended without >_AA".to_string())
}

fn parse_wsys(
    data: &[u8],
    wsys_offset: usize,
    bank_id: u32,
    flags: u32,
) -> Result<Vec<AwFileInfo>, String> {
    ensure_bytes(data, wsys_offset, 0x18)?;
    if &data[wsys_offset..wsys_offset + 4] != MAGIC_WSYS {
        return Err(format!("Invalid WSYS magic at 0x{wsys_offset:X}"));
    }

    let winf_rel = read_u32_be(data, wsys_offset + 0x10)? as usize;
    let winf_offset = wsys_offset
        .checked_add(winf_rel)
        .ok_or_else(|| "WINF offset overflow".to_string())?;
    ensure_bytes(data, winf_offset, 8)?;
    if &data[winf_offset..winf_offset + 4] != MAGIC_WINF {
        return Err(format!("Invalid WINF magic at 0x{winf_offset:X}"));
    }

    let aw_count = read_u32_be(data, winf_offset + 4)? as usize;
    let mut aw_ptr = winf_offset + 8;
    let mut out = Vec::with_capacity(aw_count);
    for _ in 0..aw_count {
        let aw_rel = read_u32_be(data, aw_ptr)? as usize;
        aw_ptr = aw_ptr
            .checked_add(4)
            .ok_or_else(|| "AW list overflow".to_string())?;
        let aw_offset = wsys_offset
            .checked_add(aw_rel)
            .ok_or_else(|| "AW info offset overflow".to_string())?;

        let aw_name = read_cstring(data, aw_offset, 0x70)?;
        let wave_table_offset = aw_offset
            .checked_add(0x70)
            .ok_or_else(|| "AW wave table offset overflow".to_string())?;
        let wave_count = read_u32_be(data, wave_table_offset)? as usize;

        let mut waves = Vec::with_capacity(wave_count);
        let mut wave_rel_offset = wave_table_offset + 4;
        for index in 0..wave_count {
            let wave_rel = read_u32_be(data, wave_rel_offset)? as usize;
            wave_rel_offset = wave_rel_offset
                .checked_add(4)
                .ok_or_else(|| "Wave offset list overflow".to_string())?;
            let wave_offset = wsys_offset
                .checked_add(wave_rel)
                .ok_or_else(|| "Wave entry offset overflow".to_string())?;
            ensure_bytes(data, wave_offset, 0x20)?;

            waves.push(AwWaveInfo {
                index,
                format: data[wave_offset + 0x01],
                sample_rate_bits: read_u32_be(data, wave_offset + 0x04)?,
                stream_offset: read_u32_be(data, wave_offset + 0x08)?,
                stream_size: read_u32_be(data, wave_offset + 0x0C)?,
                loop_flag_raw: read_i32_be(data, wave_offset + 0x10)?,
                loop_start: read_i32_be(data, wave_offset + 0x14)?,
                loop_end: read_i32_be(data, wave_offset + 0x18)?,
                num_samples: read_i32_be(data, wave_offset + 0x1C)?,
            });
        }

        out.push(AwFileInfo {
            aw_name,
            bank_id,
            flags,
            waves,
        });
    }

    Ok(out)
}

fn ensure_bytes(data: &[u8], offset: usize, len: usize) -> Result<(), String> {
    match offset.checked_add(len) {
        Some(end) if end <= data.len() => Ok(()),
        _ => Err(format!(
            "Out-of-bounds read at 0x{offset:X} for 0x{len:X} bytes"
        )),
    }
}

fn read_u32_be(data: &[u8], offset: usize) -> Result<u32, String> {
    ensure_bytes(data, offset, 4)?;
    Ok(u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

fn read_i32_be(data: &[u8], offset: usize) -> Result<i32, String> {
    Ok(read_u32_be(data, offset)? as i32)
}

fn read_cstring(data: &[u8], offset: usize, max_len: usize) -> Result<String, String> {
    ensure_bytes(data, offset, max_len)?;
    let end = (offset..offset + max_len)
        .find(|&i| data[i] == 0)
        .unwrap_or(offset + max_len);
    Ok(String::from_utf8_lossy(&data[offset..end]).to_string())
}
