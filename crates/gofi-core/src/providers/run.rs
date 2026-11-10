//! `$PATH` executable discovery.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::model::{Action, Item, ItemId, ItemKind, Loaded, Provider, ProviderError};
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
        let mut loaded = Loaded::default();
        let mut seen: HashSet<String> = HashSet::new();
        let mut scanned: HashSet<&Path> = HashSet::new();

        for dir in &self.env.path {
            // A repeated PATH entry cannot add a new executable; skip it so it
            // also does not repeat the missing-directory warning.
            if !scanned.insert(dir.as_path()) {
                continue;
            }
            let entries = match fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(error) => {
                    loaded
                        .warnings
                        .push(format!("PATH directory {}: {error}", dir.display()));
                    continue;
                }
            };

            let mut names = Vec::new();
            for entry in entries {
                match entry {
                    Ok(entry) => names.push(entry.file_name()),
                    Err(error) => loaded
                        .warnings
                        .push(format!("PATH directory {}: {error}", dir.display())),
                }
            }
            names.sort();

            for name in names {
                let name = name.to_string_lossy().into_owned();
                if name.starts_with('.') || name == "gofi" {
                    continue;
                }

                let path = dir.join(&name);
                // `metadata` follows symlinks; a broken link or a directory is skipped.
                let Ok(metadata) = fs::metadata(&path) else {
                    continue;
                };
                if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
                    continue;
                }
                if !seen.insert(name.clone()) {
                    continue;
                }

                loaded.items.push(Item {
                    id: ItemId(format!("run:exec:{name}")),
                    title: name.clone(),
                    subtitle: None,
                    icon: None,
                    search_text: name.clone(),
                    kind: ItemKind::Executable,
                    action: Action::Launch {
                        argv: vec![OsString::from(name)],
                        cwd: None,
                    },
                });
            }
        }

        Ok(loaded)
    }
}
