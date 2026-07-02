//! Strict reader for the WAV files produced by `eval-harness prep`
//! (RIFF/WAVE, PCM format 1, mono, 16 kHz, 16-bit LE). Anything else is
//! rejected with a hint to re-run prep — the engines expect exactly this shape.

use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};

pub const SAMPLE_RATE: u32 = 16_000;

/// Read a prep-produced WAV into f32 samples in [-1.0, 1.0].
pub fn read_wav_16k_mono_s16(path: &Path) -> Result<Vec<f32>> {
    let bytes =
        std::fs::read(path).with_context(|| format!("WAV okunamadı: {}", path.display()))?;
    parse_wav_16k_mono_s16(&bytes).with_context(|| {
        format!(
            "{}: 16 kHz mono s16 WAV bekleniyor — önce `eval-harness prep` çalıştırın",
            path.display()
        )
    })
}

fn parse_wav_16k_mono_s16(bytes: &[u8]) -> Result<Vec<f32>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        bail!("RIFF/WAVE başlığı yok");
    }
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;
    let mut off = 12usize;
    while off + 8 <= bytes.len() {
        let id = &bytes[off..off + 4];
        let size = u32::from_le_bytes([
            bytes[off + 4],
            bytes[off + 5],
            bytes[off + 6],
            bytes[off + 7],
        ]) as usize;
        let body_start = off + 8;
        let body_end = body_start
            .checked_add(size)
            .ok_or_else(|| anyhow!("bozuk chunk boyutu"))?;
        if body_end > bytes.len() {
            bail!("chunk dosya sonunu aşıyor (kesik dosya?)");
        }
        let body = &bytes[body_start..body_end];
        match id {
            b"fmt " => {
                if body.len() < 16 {
                    bail!("fmt chunk çok kısa");
                }
                let format = u16::from_le_bytes([body[0], body[1]]);
                let channels = u16::from_le_bytes([body[2], body[3]]);
                let rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
                let bits = u16::from_le_bytes([body[14], body[15]]);
                fmt = Some((format, channels, rate, bits));
            }
            b"data" => data = Some(body),
            _ => {}
        }
        // chunks are word-aligned: odd sizes carry one pad byte
        off = body_end + (size & 1);
    }
    let (format, channels, rate, bits) = fmt.ok_or_else(|| anyhow!("fmt chunk yok"))?;
    if format != 1 {
        bail!("PCM (format=1) bekleniyor, format={format} bulundu");
    }
    if channels != 1 {
        bail!("mono bekleniyor, {channels} kanal bulundu");
    }
    if rate != SAMPLE_RATE {
        bail!("{SAMPLE_RATE} Hz bekleniyor, {rate} Hz bulundu");
    }
    if bits != 16 {
        bail!("16-bit bekleniyor, {bits}-bit bulundu");
    }
    let data = data.ok_or_else(|| anyhow!("data chunk yok"))?;
    if data.len() % 2 != 0 {
        bail!("data chunk uzunluğu tek sayı");
    }
    Ok(data
        .chunks_exact(2)
        .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_wav(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data_len).to_le_bytes());
        b.extend_from_slice(b"WAVE");
        b.extend_from_slice(b"fmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes()); // PCM
        b.extend_from_slice(&channels.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        let byte_rate = rate * u32::from(channels) * 2;
        b.extend_from_slice(&byte_rate.to_le_bytes());
        b.extend_from_slice(&(channels * 2).to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            b.extend_from_slice(&s.to_le_bytes());
        }
        b
    }

    #[test]
    fn parses_valid_16k_mono_s16() {
        let bytes = make_wav(16_000, 1, &[0, 16_384, -32_768, 32_767]);
        let samples = parse_wav_16k_mono_s16(&bytes).expect("valid wav");
        assert_eq!(samples.len(), 4);
        assert!((samples[0] - 0.0).abs() < 1e-6);
        assert!((samples[1] - 0.5).abs() < 1e-6);
        assert!((samples[2] + 1.0).abs() < 1e-6);
    }

    #[test]
    fn rejects_wrong_shape() {
        assert!(parse_wav_16k_mono_s16(&make_wav(48_000, 1, &[0])).is_err()); // wrong rate
        assert!(parse_wav_16k_mono_s16(&make_wav(16_000, 2, &[0, 0])).is_err()); // stereo
        assert!(parse_wav_16k_mono_s16(b"not a wav file").is_err());
        let mut truncated = make_wav(16_000, 1, &[0, 0, 0, 0]);
        truncated.truncate(truncated.len() - 3);
        assert!(parse_wav_16k_mono_s16(&truncated).is_err());
    }
}
