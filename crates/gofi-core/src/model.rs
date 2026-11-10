//! Core model shared by providers, ranking, state and the UI.
//!
//! The UI must not know where an item came from. It renders [`Item`]s and
//! executes [`Action`]s; providers own discovery and argv construction.

use std::ffi::OsString;
use std::path::PathBuf;

/// The launcher's mode. Clipboard is transient and never persisted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Apps,
    Run,
    Clipboard,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apps => "apps",
            Self::Run => "run",
            Self::Clipboard => "clipboard",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "apps" | "app" | "drun" => Some(Self::Apps),
            "run" | "exec" | "executables" => Some(Self::Run),
            "clipboard" | "clip" => Some(Self::Clipboard),
            _ => None,
        }
    }
}

/// Stable identity used by frecency. Clipboard IDs are transient.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ItemId(pub String);

impl ItemId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    App,
    Executable,
    Clipboard,
}

/// One row in the launcher.
#[derive(Clone, Debug)]
pub struct Item {
    pub id: ItemId,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<PathBuf>,
    /// The text the fuzzy matcher scores. Usually title + keywords; never shown.
    pub search_text: String,
    pub kind: ItemKind,
    pub action: Action,
}

/// What Enter/click does. Providers build the argv; the UI never shells out.
#[derive(Clone, Debug)]
pub enum Action {
    Launch {
        argv: Vec<OsString>,
        cwd: Option<PathBuf>,
    },
    CopyClipboard {
        id: String,
    },
}

/// Provider output. Per-entry problems are warnings, not fatal errors.
#[derive(Debug, Default)]
pub struct Loaded {
    pub items: Vec<Item>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct ProviderError(pub String);

impl ProviderError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProviderError {}

impl From<std::io::Error> for ProviderError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

pub trait Provider {
    fn load(&self) -> Result<Loaded, ProviderError>;
}
