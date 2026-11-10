//! `$PATH` executable discovery. Worker A fills this in M1.

use crate::model::{Loaded, Provider, ProviderError};
use crate::xdg::Env;

pub struct RunProvider {
    pub env: Env,
}

impl RunProvider {
    pub fn new(env: Env) -> Self {
        Self { env }
    }
}

impl Provider for RunProvider {
    fn load(&self) -> Result<Loaded, ProviderError> {
        let _ = &self.env;
        Ok(Loaded::default())
    }
}
