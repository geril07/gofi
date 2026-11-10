//! `cliphist` clipboard provider.
//!
//! `cliphist` owns clipboard history; gofi only lists previews and copies a
//! selected entry. Previews and decoded content must never reach logs or state,
//! so errors carry the command and exit status only.

use std::ffi::OsString;
use std::process::Command;

use crate::model::{Action, Item, ItemId, ItemKind, Loaded, Provider, ProviderError};

/// `cliphist` stores at most 750 entries; cap defensively in case it grows.
const MAX_ENTRIES: usize = 750;

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
        let bin = self.bin.to_string_lossy();
        let output = Command::new(&self.bin)
            .arg("list")
            .output()
            .map_err(|error| ProviderError::new(format!("{bin} list failed: {error}")))?;

        if !output.status.success() {
            return Err(ProviderError::new(format!(
                "{bin} list failed: {}",
                output.status
            )));
        }

        let mut loaded = Loaded::default();
        let text = String::from_utf8_lossy(&output.stdout);

        for line in text.lines() {
            if loaded.items.len() >= MAX_ENTRIES {
                break;
            }
            if line.is_empty() {
                continue;
            }
            let Some((id, preview)) = line.split_once('\t') else {
                loaded
                    .warnings
                    .push(format!("{bin}: skipping a line without an id"));
                continue;
            };
            if id.is_empty() {
                loaded
                    .warnings
                    .push(format!("{bin}: skipping a line without an id"));
                continue;
            }

            loaded.items.push(Item {
                id: ItemId(format!("clipboard:{id}")),
                title: preview.to_string(),
                subtitle: None,
                icon: None,
                search_text: preview.to_string(),
                kind: ItemKind::Clipboard,
                action: Action::CopyClipboard { id: id.to_string() },
            });
        }

        Ok(loaded)
    }
}
