//! Estado compartilhado entre a UI e os callbacks de áudio (somente atômicos).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

/// Pads por página (grade máxima 8×8). O índice global de um pad é `página * PAGE_SIZE + posição`.
pub const PAGE_SIZE: usize = 64;
pub const MAX_PAGES: usize = 16;
pub const MAX_SLOTS: usize = PAGE_SIZE * MAX_PAGES;

pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub fn new(v: f32) -> Self {
        Self(AtomicU32::new(v.to_bits()))
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Relaxed))
    }

    pub fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Relaxed);
    }

    /// Lê o valor e zera (usado pelos medidores de pico).
    pub fn take(&self) -> f32 {
        f32::from_bits(self.0.swap(0f32.to_bits(), Relaxed))
    }

    /// Guarda o maior entre o valor atual e `v`.
    pub fn store_max(&self, v: f32) {
        if v > self.get() {
            self.set(v);
        }
    }
}

pub struct Shared {
    pub mic_gain: AtomicF32,
    pub board_gain: AtomicF32,
    pub master_gain: AtomicF32,
    pub monitor_gain: AtomicF32,
    pub mic_muted: AtomicBool,
    pub monitor_mic: AtomicBool,
    pub slot_gain: [AtomicF32; MAX_SLOTS],
    pub slot_loop: [AtomicBool; MAX_SLOTS],
    /// 0 = parado; 1..=10001 = tocando, com progresso (valor - 1) / 10000.
    pub slot_progress: [AtomicU32; MAX_SLOTS],
    pub mic_level: AtomicF32,
    pub out_level: AtomicF32,
    pub stream_error: AtomicBool,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            mic_gain: AtomicF32::new(1.0),
            board_gain: AtomicF32::new(1.0),
            master_gain: AtomicF32::new(1.0),
            monitor_gain: AtomicF32::new(1.0),
            mic_muted: AtomicBool::new(false),
            monitor_mic: AtomicBool::new(false),
            slot_gain: std::array::from_fn(|_| AtomicF32::new(1.0)),
            slot_loop: std::array::from_fn(|_| AtomicBool::new(false)),
            slot_progress: std::array::from_fn(|_| AtomicU32::new(0)),
            mic_level: AtomicF32::new(0.0),
            out_level: AtomicF32::new(0.0),
            stream_error: AtomicBool::new(false),
        }
    }

    pub fn is_playing(&self, slot: usize) -> bool {
        self.slot_progress[slot].load(Relaxed) > 0
    }

    pub fn progress(&self, slot: usize) -> Option<f32> {
        match self.slot_progress[slot].load(Relaxed) {
            0 => None,
            v => Some((v - 1) as f32 / 10_000.0),
        }
    }

    pub fn set_progress(&self, slot: usize, fraction: Option<f32>) {
        let v = match fraction {
            None => 0,
            Some(f) => (f.clamp(0.0, 1.0) * 10_000.0) as u32 + 1,
        };
        self.slot_progress[slot].store(v, Relaxed);
    }

    pub fn reset_progress(&self) {
        for p in &self.slot_progress {
            p.store(0, Relaxed);
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}
