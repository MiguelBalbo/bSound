//! Reamostragem linear: em tempo real (mic) e offline (clipes estéreo).

/// Resampler linear de streaming para um canal.
pub struct LinearResampler {
    /// Amostras de entrada por amostra de saída.
    ratio: f64,
    pos: f64,
    prev: f32,
}

impl LinearResampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        Self {
            ratio: in_rate as f64 / out_rate as f64,
            pos: 0.0,
            prev: 0.0,
        }
    }

    pub fn is_passthrough(&self) -> bool {
        self.ratio == 1.0
    }

    pub fn push(&mut self, x: f32, mut out: impl FnMut(f32)) {
        if self.is_passthrough() {
            out(x);
            return;
        }
        while self.pos < 1.0 {
            let t = self.pos as f32;
            out(self.prev + (x - self.prev) * t);
            self.pos += self.ratio;
        }
        self.pos -= 1.0;
        self.prev = x;
    }
}

/// Reamostra áudio estéreo intercalado de `in_rate` para `out_rate`.
pub fn resample_stereo(input: &[f32], in_rate: u32, out_rate: u32) -> Vec<f32> {
    if in_rate == out_rate || input.len() < 2 {
        return input.to_vec();
    }
    let in_frames = input.len() / 2;
    let out_frames = (in_frames as u64 * out_rate as u64 / in_rate as u64) as usize;
    let step = in_rate as f64 / out_rate as f64;
    let mut out = Vec::with_capacity(out_frames * 2);
    for i in 0..out_frames {
        let src = i as f64 * step;
        let idx = src as usize;
        let t = (src - idx as f64) as f32;
        let next = (idx + 1).min(in_frames - 1);
        for ch in 0..2 {
            let a = input[idx * 2 + ch];
            let b = input[next * 2 + ch];
            out.push(a + (b - a) * t);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_ratio_preserves_rate() {
        let mut r = LinearResampler::new(44_100, 48_000);
        let mut n = 0;
        for i in 0..44_100 {
            r.push((i as f32 * 0.01).sin(), |_| n += 1);
        }
        assert!((n as i32 - 48_000).abs() <= 2, "gerou {n}");
    }

    #[test]
    fn offline_doubles_frames() {
        let input: Vec<f32> = (0..2000).map(|i| (i / 2) as f32).collect();
        let out = resample_stereo(&input, 24_000, 48_000);
        assert_eq!(out.len(), 4000);
        // Interpolação linear entre frames 0 e 1.
        assert!((out[2] - 0.5).abs() < 1e-6);
    }
}
