//! Configuração persistida em ~/Library/Application Support/soundboard/config.json.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::audio::shared::MAX_SLOTS;

const DEFAULT_KEYS: &[&str] = &[
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "A",
    "S", "D", "F", "G", "H", "J", "K", "L", "Z", "X", "C", "V", "B", "N", "M",
];

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Slot {
    pub name: String,
    pub path: PathBuf,
    pub volume: f32,
    pub looping: bool,
    /// Clicar enquanto toca reinicia o som em vez de parar.
    pub restart: bool,
    pub key: Option<String>,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: PathBuf::new(),
            volume: 1.0,
            looping: false,
            restart: false,
            key: None,
        }
    }
}

impl Slot {
    pub fn from_path(path: PathBuf) -> Self {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Som".into());
        Self { name, path, ..Default::default() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub rows: usize,
    pub cols: usize,
    pub slots: Vec<Option<Slot>>,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub monitor_device: Option<String>,
    pub mic_gain: f32,
    pub board_gain: f32,
    pub master_gain: f32,
    pub monitor_gain: f32,
    pub mic_muted: bool,
    pub monitor_mic: bool,
    pub hide_wizard: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rows: 4,
            cols: 4,
            slots: vec![None; MAX_SLOTS],
            input_device: None,
            output_device: None,
            monitor_device: None,
            mic_gain: 1.0,
            board_gain: 0.8,
            master_gain: 1.0,
            monitor_gain: 0.8,
            mic_muted: false,
            monitor_mic: false,
            hide_wizard: false,
        }
    }
}

impl Config {
    fn path() -> Option<PathBuf> {
        directories::BaseDirs::new().map(|d| d.config_dir().join("soundboard").join("config.json"))
    }

    pub fn load() -> Self {
        let mut cfg: Config = Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        cfg.slots.resize(MAX_SLOTS, None);
        cfg.rows = cfg.rows.clamp(1, 8);
        cfg.cols = cfg.cols.clamp(1, 8);
        cfg
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        let result = (|| -> anyhow::Result<()> {
            std::fs::create_dir_all(path.parent().unwrap())?;
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
            std::fs::rename(tmp, &path)?;
            Ok(())
        })();
        if let Err(e) = result {
            log::error!("falha ao salvar config: {e}");
        }
    }

    pub fn visible_slots(&self) -> usize {
        (self.rows * self.cols).min(MAX_SLOTS)
    }

    pub fn key_for(&self, i: usize) -> Option<String> {
        self.slots[i]
            .as_ref()
            .and_then(|s| s.key.clone())
            .or_else(|| DEFAULT_KEYS.get(i).map(|k| k.to_string()))
    }
}
