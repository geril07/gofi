//! `cliphist` clipboard provider. Worker A fills this in M1.

use std::ffi::OsString;

use crate::model::{Loaded, Provider, ProviderError};

pub struct ClipboardProvider {
    pub bin: OsString,
}

impl ClipboardProvider {
    pub fn new(bin: impl Into<OsString>) -> Self {
        Self { bin: bin.into() }
    }
}

impl Provider for ClipboardProvider {
    fn load(&self) -> Result<Loaded, ProviderError> {
        let _ = &self.bin;
        Ok(Loaded::default())
    }
}
