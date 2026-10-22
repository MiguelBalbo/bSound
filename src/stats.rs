//! Quantas vezes cada pad foi tocado (persistido em plays.json, ao lado da config).
//! Fica fora do Config porque o menu da barra de menus também registra toques,
//! mesmo com a janela escondida.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use crate::audio::shared::MAX_SLOTS;
use crate::config::Config;

pub struct Stats {
    counts: Mutex<Vec<u32>>,
    path: Option<PathBuf>,
}

impl Stats {
    pub fn load() -> Self {
        let path = Config::dir().map(|d| d.join("plays.json"));
        let mut counts: Vec<u32> = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        counts.resize(MAX_SLOTS, 0);
        Self { counts: Mutex::new(counts), path }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<u32>> {
        self.counts.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get(&self, slot: usize) -> u32 {
        self.lock()[slot]
    }

    pub fn record(&self, slot: usize) {
        let mut c = self.lock();
        c[slot] = c[slot].saturating_add(1);
        self.save(&c);
    }

    pub fn reset(&self, slot: usize) {
        let mut c = self.lock();
        if c[slot] != 0 {
            c[slot] = 0;
            self.save(&c);
        }
    }

    fn save(&self, counts: &[u32]) {
        let Some(path) = &self.path else { return };
        let result = std::fs::create_dir_all(path.parent().unwrap())
            .and_then(|_| std::fs::write(path, serde_json::to_string(counts).unwrap_or_default()));
        if let Err(e) = result {
            log::error!("falha ao salvar plays.json: {e}");
        }
    }
}
