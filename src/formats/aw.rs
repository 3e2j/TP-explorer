use sha2::{Digest, Sha256};

use crate::formats::baa::AwWaveInfo;

#[derive(Debug, Clone)]
pub enum WaveDiffKind {
    Changed,
    Added,
    Removed,
}

#[derive(Debug, Clone)]
pub struct WaveDiffEntry {
    pub index: usize,
    pub kind: WaveDiffKind,
    pub reason: &'static str,
    pub iso_offset: Option<u32>,
    pub iso_size: Option<u32>,
    pub folder_offset: Option<u32>,
    pub folder_size: Option<u32>,
}

#[derive(Debug)]
pub struct AwEntryDiff {
    pub iso_wave_count: usize,
    pub folder_wave_count: usize,
    pub changed_entries: Vec<WaveDiffEntry>,
}

const AFC_COEFS: [[i32; 2]; 16] = [
    [0, 0],
    [2048, 0],
    [0, 2048],
    [1024, 1024],
    [4096, -2048],
    [3584, -1536],
    [3072, -1024],
    [4608, -2560],
    [4200, -2248],
    [4800, -2300],
    [5120, -3072],
    [2048, -2048],
    [1024, -1024],
    [-1024, 1024],
    [-1024, 0],
    [-2048, 0],
];

const NIBBLE2_TO_INT: [i32; 4] = [0, 1, -2, -1];

pub fn diff_aw_entries(
    iso_aw_bytes: &[u8],
    folder_aw_bytes: &[u8],
    iso_info: &crate::formats::baa::AwFileInfo,
    folder_info: &crate::formats::baa::AwFileInfo,
) -> Result<AwEntryDiff, String> {
    let mut changed_entries = Vec::new();
    let max_waves = iso_info.waves.len().max(folder_info.waves.len());

    for i in 0..max_waves {
        let iso_wave = iso_info.waves.get(i);
        let folder_wave = folder_info.waves.get(i);

        match (iso_wave, folder_wave) {
            (Some(iso_wave), Some(folder_wave)) => {
                let iso_data =
                    slice_wave_data(iso_aw_bytes, iso_wave.stream_offset, iso_wave.stream_size)?;
                let folder_data = slice_wave_data(
                    folder_aw_bytes,
                    folder_wave.stream_offset,
                    folder_wave.stream_size,
                )?;

                let data_changed = hash_bytes(iso_data) != hash_bytes(folder_data);
                let metadata_changed = iso_wave.format != folder_wave.format
                    || iso_wave.sample_rate_bits != folder_wave.sample_rate_bits
                    || iso_wave.stream_offset != folder_wave.stream_offset
                    || iso_wave.stream_size != folder_wave.stream_size
                    || iso_wave.loop_flag_raw != folder_wave.loop_flag_raw
                    || iso_wave.loop_start != folder_wave.loop_start
                    || iso_wave.loop_end != folder_wave.loop_end
                    || iso_wave.num_samples != folder_wave.num_samples;

                if data_changed || metadata_changed {
                    changed_entries.push(WaveDiffEntry {
                        index: i,
                        kind: WaveDiffKind::Changed,
                        reason: if data_changed && metadata_changed {
                            "metadata+data"
                        } else if metadata_changed {
                            "metadata"
                        } else {
                            "data"
                        },
                        iso_offset: Some(iso_wave.stream_offset),
                        iso_size: Some(iso_wave.stream_size),
                        folder_offset: Some(folder_wave.stream_offset),
                        folder_size: Some(folder_wave.stream_size),
                    });
                }
            }
            (None, Some(folder_wave)) => {
                changed_entries.push(WaveDiffEntry {
                    index: i,
                    kind: WaveDiffKind::Added,
                    reason: "present only in folder AW+BAA",
                    iso_offset: None,
                    iso_size: None,
                    folder_offset: Some(folder_wave.stream_offset),
                    folder_size: Some(folder_wave.stream_size),
                });
            }
            (Some(iso_wave), None) => {
                changed_entries.push(WaveDiffEntry {
                    index: i,
                    kind: WaveDiffKind::Removed,
                    reason: "present only in ISO AW+BAA",
                    iso_offset: Some(iso_wave.stream_offset),
                    iso_size: Some(iso_wave.stream_size),
                    folder_offset: None,
                    folder_size: None,
                });
            }
            (None, None) => {}
        }
    }

    Ok(AwEntryDiff {
        iso_wave_count: iso_info.waves.len(),
        folder_wave_count: folder_info.waves.len(),
        changed_entries,
    })
}

pub fn format_aw_entry_diff(
    rel_path: &str,
    baa_path: &str,
    iso_info: &crate::formats::baa::AwFileInfo,
    folder_info: &crate::formats::baa::AwFileInfo,
    diff: &AwEntryDiff,
) -> String {
    let mut out = String::new();
    out.push_str("  ");
    out.push_str(rel_path);
    out.push('\n');
    out.push_str("    baa: ");
    out.push_str(baa_path);
    out.push('\n');
    out.push_str("    iso_bank: ");
    out.push_str(&iso_info.bank_id.to_string());
    out.push_str(", folder_bank: ");
    out.push_str(&folder_info.bank_id.to_string());
    out.push('\n');
    out.push_str("    iso_wave_count: ");
    out.push_str(&diff.iso_wave_count.to_string());
    out.push_str(", folder_wave_count: ");
    out.push_str(&diff.folder_wave_count.to_string());
    out.push('\n');

    if diff.changed_entries.is_empty() {
        out.push_str("    no internal wave entry changes detected\n");
        return out;
    }

    out.push_str("    changed_wave_entries:\n");
    for entry in &diff.changed_entries {
        out.push_str("      wave[");
        out.push_str(&entry.index.to_string());
        out.push_str("] ");
        out.push_str(match entry.kind {
            WaveDiffKind::Changed => "changed",
            WaveDiffKind::Added => "added",
            WaveDiffKind::Removed => "removed",
        });
        out.push_str(" (");
        out.push_str(entry.reason);
        out.push(')');

        if let (Some(iso_off), Some(iso_size)) = (entry.iso_offset, entry.iso_size) {
            out.push_str(", iso(off=0x");
            out.push_str(&format!("{iso_off:X}"));
            out.push_str(", size=");
            out.push_str(&iso_size.to_string());
            out.push(')');
        }
        if let (Some(folder_off), Some(folder_size)) = (entry.folder_offset, entry.folder_size) {
            out.push_str(", folder(off=0x");
            out.push_str(&format!("{folder_off:X}"));
            out.push_str(", size=");
            out.push_str(&folder_size.to_string());
            out.push(')');
        }
        out.push('\n');
    }

    out
}

pub fn extract_wave_bytes(data: &[u8], offset: u32, size: u32) -> Result<Vec<u8>, String> {
    Ok(slice_wave_data(data, offset, size)?.to_vec())
}

pub fn wave_to_wav_bytes(aw_data: &[u8], wave: &AwWaveInfo) -> Result<Vec<u8>, String> {
    let wave_data = slice_wave_data(aw_data, wave.stream_offset, wave.stream_size)?;
    wave_data_to_wav_bytes(wave_data, wave)
}

pub fn wave_data_to_wav_bytes(wave_data: &[u8], wave: &AwWaveInfo) -> Result<Vec<u8>, String> {
    let pcm = decode_wave_data_to_pcm16(wave_data, wave)?;
    let sample_rate = wave_sample_rate_hz(wave.sample_rate_bits)?;
    build_wav_mono16(
        &pcm,
        sample_rate,
        wave.loop_flag_raw == -1,
        wave.loop_start.max(0) as u32,
        wave.loop_end.max(0) as u32,
    )
}

fn decode_wave_data_to_pcm16(data: &[u8], wave: &AwWaveInfo) -> Result<Vec<i16>, String> {
    match wave.format {
        0x00 => decode_afc_4bit(data, wave.num_samples),
        0x01 => decode_afc_2bit(data, wave.num_samples),
        0x02 => Ok(decode_pcm8(data, wave.num_samples)),
        0x03 => Ok(decode_pcm16be(data, wave.num_samples)),
        other => Err(format!("Unsupported AW wave format: {other}")),
    }
}

fn decode_afc_4bit(data: &[u8], num_samples: i32) -> Result<Vec<i16>, String> {
    const BYTES_PER_FRAME: usize = 0x09;
    const SAMPLES_PER_FRAME: usize = 16;

    let expected = if num_samples > 0 {
        num_samples as usize
    } else {
        data.len() / BYTES_PER_FRAME * SAMPLES_PER_FRAME
    };

    let mut out = Vec::with_capacity(expected);
    let mut hist1 = 0i32;
    let mut hist2 = 0i32;
    let mut pos = 0usize;
    while pos + BYTES_PER_FRAME <= data.len() && out.len() < expected {
        let frame = &data[pos..pos + BYTES_PER_FRAME];
        pos += BYTES_PER_FRAME;

        let scale = 1i32 << ((frame[0] >> 4) & 0x0F);
        let index = (frame[0] & 0x0F) as usize;
        let coef1 = AFC_COEFS[index][0];
        let coef2 = AFC_COEFS[index][1];

        for i in 0..SAMPLES_PER_FRAME {
            if out.len() >= expected {
                break;
            }
            let code = frame[1 + i / 2];
            let nibble = if i & 1 == 0 {
                ((code >> 4) & 0x0F) as i32
            } else {
                (code & 0x0F) as i32
            };
            let signed = if nibble >= 8 { nibble - 16 } else { nibble };

            let mut sample = (signed * scale) << 11;
            sample = (sample + coef1 * hist1 + coef2 * hist2) >> 11;
            let clamped = clamp16(sample);
            out.push(clamped);
            hist2 = hist1;
            hist1 = clamped as i32;
        }
    }

    if out.is_empty() {
        return Err("Failed to decode AFC 4-bit data".to_string());
    }
    Ok(out)
}

fn decode_afc_2bit(data: &[u8], num_samples: i32) -> Result<Vec<i16>, String> {
    const BYTES_PER_FRAME: usize = 0x05;
    const SAMPLES_PER_FRAME: usize = 16;

    let expected = if num_samples > 0 {
        num_samples as usize
    } else {
        data.len() / BYTES_PER_FRAME * SAMPLES_PER_FRAME
    };

    let mut out = Vec::with_capacity(expected);
    let mut hist1 = 0i32;
    let mut hist2 = 0i32;
    let mut pos = 0usize;
    while pos + BYTES_PER_FRAME <= data.len() && out.len() < expected {
        let frame = &data[pos..pos + BYTES_PER_FRAME];
        pos += BYTES_PER_FRAME;

        let scale = 8192i32 << ((frame[0] >> 4) & 0x0F);
        let index = (frame[0] & 0x0F) as usize;
        let coef1 = AFC_COEFS[index][0];
        let coef2 = AFC_COEFS[index][1];

        for i in 0..SAMPLES_PER_FRAME {
            if out.len() >= expected {
                break;
            }
            let code = frame[1 + i / 4];
            let shift = 6 - ((i & 0x03) * 2);
            let nibble2 = ((code >> shift) & 0x03) as usize;
            let signed = NIBBLE2_TO_INT[nibble2];
            let mut sample = signed * scale;
            sample = (sample + coef1 * hist1 + coef2 * hist2) >> 11;
            let clamped = clamp16(sample);
            out.push(clamped);
            hist2 = hist1;
            hist1 = clamped as i32;
        }
    }

    if out.is_empty() {
        return Err("Failed to decode AFC 2-bit data".to_string());
    }
    Ok(out)
}

fn decode_pcm8(data: &[u8], num_samples: i32) -> Vec<i16> {
    let expected = if num_samples > 0 {
        num_samples as usize
    } else {
        data.len()
    };
    let mut out = Vec::with_capacity(expected);
    for &b in data.iter().take(expected) {
        let s = (b as i8 as i16) << 8;
        out.push(s);
    }
    out
}

fn decode_pcm16be(data: &[u8], num_samples: i32) -> Vec<i16> {
    let expected = if num_samples > 0 {
        num_samples as usize
    } else {
        data.len() / 2
    };
    let mut out = Vec::with_capacity(expected);
    let mut i = 0usize;
    while i + 1 < data.len() && out.len() < expected {
        out.push(i16::from_be_bytes([data[i], data[i + 1]]));
        i += 2;
    }
    out
}

fn wave_sample_rate_hz(bits: u32) -> Result<u32, String> {
    let hz = f32::from_bits(bits);
    if !hz.is_finite() || hz <= 0.0 {
        return Err(format!("Invalid sample rate bits: 0x{bits:08X}"));
    }
    Ok(hz.round() as u32)
}

fn build_wav_mono16(
    pcm: &[i16],
    sample_rate: u32,
    loop_enabled: bool,
    loop_start: u32,
    loop_end: u32,
) -> Result<Vec<u8>, String> {
    let channels = 1u16;
    let bits_per_sample = 16u16;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate
        .checked_mul(block_align as u32)
        .ok_or_else(|| "WAV byte rate overflow".to_string())?;
    let data_size = (pcm.len() * 2) as u32;

    let smpl = if loop_enabled && loop_end > loop_start {
        Some(build_smpl_chunk(
            sample_rate,
            loop_start,
            loop_end.saturating_sub(1),
        ))
    } else {
        None
    };
    let smpl_size = smpl.as_ref().map_or(0u32, |v| v.len() as u32);

    let riff_size = 4 + (8 + 16) + (8 + data_size) + smpl_size;
    let mut out = Vec::with_capacity((riff_size + 8) as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    for &s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }

    if let Some(mut smpl_chunk) = smpl {
        out.append(&mut smpl_chunk);
    }

    Ok(out)
}

fn build_smpl_chunk(sample_rate: u32, loop_start: u32, loop_end_inclusive: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 60);
    out.extend_from_slice(b"smpl");
    out.extend_from_slice(&60u32.to_le_bytes());

    let sample_period_ns = if sample_rate == 0 {
        0
    } else {
        (1_000_000_000u64 / sample_rate as u64) as u32
    };

    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&sample_period_ns.to_le_bytes());
    out.extend_from_slice(&60u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&loop_start.to_le_bytes());
    out.extend_from_slice(&loop_end_inclusive.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

fn slice_wave_data(data: &[u8], offset: u32, size: u32) -> Result<&[u8], String> {
    let start = offset as usize;
    let len = size as usize;
    let end = start
        .checked_add(len)
        .ok_or_else(|| "Wave slice overflow".to_string())?;
    data.get(start..end)
        .ok_or_else(|| format!("Wave slice out of bounds: offset=0x{start:X}, size={len}"))
}

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn clamp16(v: i32) -> i16 {
    if v > i16::MAX as i32 {
        i16::MAX
    } else if v < i16::MIN as i32 {
        i16::MIN
    } else {
        v as i16
    }
}
