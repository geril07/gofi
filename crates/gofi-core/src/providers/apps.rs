//! XDG `.desktop` application discovery. Worker A fills this in M1.

use crate::model::{Loaded, Provider, ProviderError};
use crate::xdg::Env;

pub struct AppsProvider {
    pub env: Env,
}

impl AppsProvider {
    pub fn new(env: Env) -> Self {
        Self { env }
    }
}

impl Provider for AppsProvider {
    fn load(&self) -> Result<Loaded, ProviderError> {
        let _ = &self.env;
        Ok(Loaded::default())
    }
}
