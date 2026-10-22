//! Decodificação de arquivos de áudio para estéreo f32 na taxa da saída.

use std::fs::File;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::resample::resample_stereo;

pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a", "aac", "aiff", "aif", "caf"];

pub struct Clip {
    /// Estéreo intercalado (L, R, L, R…).
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Clip {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }
}

pub fn load(path: &Path, target_rate: u32) -> Result<Clip> {
    let (interleaved, channels, rate) = decode(path)?;
    let stereo = to_stereo(&interleaved, channels);
    Ok(Clip {
        samples: resample_stereo(&stereo, rate, target_rate),
        sample_rate: target_rate,
    })
}

fn decode(path: &Path) -> Result<(Vec<f32>, usize, u32)> {
    let file = File::open(path).with_context(|| format!("não foi possível abrir {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .context("formato de áudio não suportado")?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow!("nenhuma faixa de áudio no arquivo"))?;
    let track_id = track.id;
    let mut rate = track.codec_params.sample_rate.unwrap_or(0);
    let mut channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(0);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("codec não suportado")?;

    let mut out = Vec::new();
    let mut buf: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                rate = spec.rate;
                channels = spec.channels.count();
                let needed = decoded.capacity() as u64;
                if buf.as_ref().is_none_or(|b| (b.capacity() as u64) < needed * channels as u64) {
                    buf = Some(SampleBuffer::new(needed, spec));
                }
                let b = buf.as_mut().unwrap();
                b.copy_interleaved_ref(decoded);
                out.extend_from_slice(b.samples());
            }
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        }
    }
    if out.is_empty() || channels == 0 || rate == 0 {
        return Err(anyhow!("arquivo sem áudio"));
    }
    Ok((out, channels, rate))
}

fn to_stereo(interleaved: &[f32], channels: usize) -> Vec<f32> {
    match channels {
        2 => interleaved.to_vec(),
        1 => interleaved.iter().flat_map(|&s| [s, s]).collect(),
        _ => interleaved
            .chunks_exact(channels)
            .flat_map(|f| [f[0], f[1]])
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_wav(path: &Path, rate: u32, samples: &[i16]) {
        let data_len = (samples.len() * 2) as u32;
        let mut f = File::create(path).unwrap();
        f.write_all(b"RIFF").unwrap();
        f.write_all(&(36 + data_len).to_le_bytes()).unwrap();
        f.write_all(b"WAVEfmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
        f.write_all(&1u16.to_le_bytes()).unwrap(); // mono
        f.write_all(&rate.to_le_bytes()).unwrap();
        f.write_all(&(rate * 2).to_le_bytes()).unwrap();
        f.write_all(&2u16.to_le_bytes()).unwrap();
        f.write_all(&16u16.to_le_bytes()).unwrap();
        f.write_all(b"data").unwrap();
        f.write_all(&data_len.to_le_bytes()).unwrap();
        for s in samples {
            f.write_all(&s.to_le_bytes()).unwrap();
        }
    }

    #[test]
    fn decodes_mono_wav_and_resamples() {
        let path = std::env::temp_dir().join("soundboard_test_clip.wav");
        let samples: Vec<i16> = (0..24_000).map(|i| ((i % 100) * 300) as i16).collect();
        write_wav(&path, 24_000, &samples);
        let clip = load(&path, 48_000).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(clip.sample_rate, 48_000);
        assert_eq!(clip.frames(), 48_000);
        // Mono duplicado em L e R.
        assert_eq!(clip.samples[200], clip.samples[201]);
    }
}
