//! Mixagem do microfone com as vozes do soundboard (roda no callback de saída).

use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;

use super::clip::Clip;
use super::shared::{Shared, MAX_SLOTS};

pub enum Command {
    Play { slot: usize, clip: Arc<Clip> },
    Stop(usize),
    StopAll,
}

struct Voice {
    slot: usize,
    clip: Arc<Clip>,
    /// Posição em frames.
    pos: usize,
    done: bool,
}

pub struct Mixer {
    voices: Vec<Voice>,
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

impl Mixer {
    pub fn new() -> Self {
        Self {
            voices: Vec::with_capacity(MAX_SLOTS),
        }
    }

    pub fn handle(&mut self, cmd: Command, s: &Shared) {
        match cmd {
            Command::Play { slot, clip } => {
                if let Some(v) = self.voices.iter_mut().find(|v| v.slot == slot) {
                    v.clip = clip;
                    v.pos = 0;
                    v.done = false;
                } else {
                    self.voices.push(Voice { slot, clip, pos: 0, done: false });
                }
                s.set_progress(slot, Some(0.0));
            }
            Command::Stop(slot) => {
                self.voices.retain(|v| v.slot != slot);
                s.set_progress(slot, None);
            }
            Command::StopAll => {
                self.voices.clear();
                s.reset_progress();
            }
        }
    }

    /// `mic` é mono; `out` e `monitor` são estéreo intercalados com `mic.len()` frames.
    pub fn render(&mut self, mic: &[f32], out: &mut [f32], monitor: &mut [f32], s: &Shared) {
        let frames = mic.len();
        let out = &mut out[..frames * 2];
        let board = &mut monitor[..frames * 2];
        board.fill(0.0);

        for v in &mut self.voices {
            let gain = s.slot_gain[v.slot].get();
            let looping = s.slot_loop[v.slot].load(Relaxed);
            let samples = &v.clip.samples;
            let len = v.clip.frames();
            for f in 0..frames {
                if v.pos >= len {
                    if looping && len > 0 {
                        v.pos = 0;
                    } else {
                        v.done = true;
                        break;
                    }
                }
                board[f * 2] += samples[v.pos * 2] * gain;
                board[f * 2 + 1] += samples[v.pos * 2 + 1] * gain;
                v.pos += 1;
            }
            if v.done {
                s.set_progress(v.slot, None);
            } else if len > 0 {
                s.set_progress(v.slot, Some(v.pos as f32 / len as f32));
            }
        }
        self.voices.retain(|v| !v.done);

        let mic_gain = if s.mic_muted.load(Relaxed) { 0.0 } else { s.mic_gain.get() };
        let board_gain = s.board_gain.get();
        let master = s.master_gain.get();
        let monitor_gain = s.monitor_gain.get();
        let monitor_mic = s.monitor_mic.load(Relaxed);
        let mut mic_peak = 0f32;
        let mut out_peak = 0f32;
        for f in 0..frames {
            let m = mic[f] * mic_gain;
            mic_peak = mic_peak.max(m.abs());
            let mon_m = if monitor_mic { m } else { 0.0 };
            for ch in 0..2 {
                let b = board[f * 2 + ch] * board_gain;
                let o = soft_clip((m + b) * master);
                out_peak = out_peak.max(o.abs());
                out[f * 2 + ch] = o;
                board[f * 2 + ch] = soft_clip((b + mon_m) * monitor_gain);
            }
        }
        s.mic_level.store_max(mic_peak);
        s.out_level.store_max(out_peak);
    }
}

/// Linear até 0.8 e satura suavemente até 1.0 (contínuo e com derivada contínua).
pub fn soft_clip(x: f32) -> f32 {
    const KNEE: f32 = 0.8;
    let a = x.abs();
    if a <= KNEE {
        x
    } else {
        let y = KNEE + (1.0 - KNEE) * ((a - KNEE) / (1.0 - KNEE)).tanh();
        y.copysign(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(frames: usize, value: f32) -> Arc<Clip> {
        Arc::new(Clip { samples: vec![value; frames * 2], sample_rate: 48_000 })
    }

    fn render(m: &mut Mixer, s: &Shared, mic: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let mut out = vec![0.0; mic.len() * 2];
        let mut mon = vec![0.0; mic.len() * 2];
        m.render(mic, &mut out, &mut mon, s);
        (out, mon)
    }

    #[test]
    fn mixes_mic_and_clip() {
        let s = Shared::new();
        let mut m = Mixer::new();
        m.handle(Command::Play { slot: 0, clip: clip(100, 0.25) }, &s);
        let (out, mon) = render(&mut m, &s, &[0.25; 10]);
        assert!((out[0] - 0.5).abs() < 1e-6);
        assert!((out[1] - 0.5).abs() < 1e-6);
        // Monitor só com o soundboard por padrão.
        assert!((mon[0] - 0.25).abs() < 1e-6);
        assert!(s.is_playing(0));
    }

    #[test]
    fn voice_ends_and_clears_progress() {
        let s = Shared::new();
        let mut m = Mixer::new();
        m.handle(Command::Play { slot: 3, clip: clip(5, 0.1) }, &s);
        let (out, _) = render(&mut m, &s, &[0.0; 10]);
        assert!(out[8] > 0.0);
        assert_eq!(out[10], 0.0);
        assert!(!s.is_playing(3));
    }

    #[test]
    fn loop_and_stop() {
        let s = Shared::new();
        let mut m = Mixer::new();
        s.slot_loop[1].store(true, Relaxed);
        m.handle(Command::Play { slot: 1, clip: clip(4, 0.1) }, &s);
        let (out, _) = render(&mut m, &s, &[0.0; 10]);
        assert!(out[18] > 0.0);
        assert!(s.is_playing(1));
        m.handle(Command::Stop(1), &s);
        let (out, _) = render(&mut m, &s, &[0.0; 4]);
        assert!(out.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn mute_and_gain() {
        let s = Shared::new();
        let mut m = Mixer::new();
        s.mic_muted.store(true, Relaxed);
        let (out, _) = render(&mut m, &s, &[0.5; 4]);
        assert!(out.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn soft_clip_is_bounded_and_linear_below_knee() {
        assert_eq!(soft_clip(0.5), 0.5);
        assert!(soft_clip(10.0) <= 1.0);
        assert!(soft_clip(-10.0) >= -1.0);
        assert!(soft_clip(0.9) < 0.9 && soft_clip(0.9) > 0.8);
    }
}
