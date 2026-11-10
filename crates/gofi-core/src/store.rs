//! Persistent per-mode frecency. Worker B fills this in M1.

use std::sync::OnceLock;

use crate::model::Mode;
use crate::ranking::frecency::Frecency;
use crate::xdg::Env;

#[derive(Debug)]
pub struct StoreError(pub String);

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StoreError {}

#[derive(Clone, Debug, Default)]
pub struct State {
    pub apps: Frecency,
    pub run: Frecency,
}

impl State {
    pub fn load(_env: &Env) -> Self {
        Self::default()
    }

    pub fn save(&self, _env: &Env) -> Result<(), StoreError> {
        Ok(())
    }

    pub fn frecency(&self, mode: Mode) -> &Frecency {
        match mode {
            Mode::Apps => &self.apps,
            Mode::Run => &self.run,
            Mode::Clipboard => empty_frecency(),
        }
    }

    pub fn frecency_mut(&mut self, mode: Mode) -> Option<&mut Frecency> {
        match mode {
            Mode::Apps => Some(&mut self.apps),
            Mode::Run => Some(&mut self.run),
            Mode::Clipboard => None,
        }
    }
}

fn empty_frecency() -> &'static Frecency {
    static EMPTY: OnceLock<Frecency> = OnceLock::new();
    EMPTY.get_or_init(Frecency::default)
}
