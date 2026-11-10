//! Icon name → file path resolution. Worker A fills this in M1.

use std::path::PathBuf;

use crate::xdg::Env;

pub fn resolve_icon(_name: &str, _env: &Env) -> Option<PathBuf> {
    None
}
