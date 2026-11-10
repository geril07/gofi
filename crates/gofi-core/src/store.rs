//! Persistent per-mode frecency: one JSON file, atomic replacement, advisory lock.
//!
//! The format and its rules are ADR 0005. Only Apps and Run are persisted;
//! clipboard IDs and previews are never written because cliphist owns that
//! history and its content is sensitive.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::OnceLock;

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::model::Mode;
use crate::ranking::frecency::{self, Entry, Frecency};
use crate::xdg::Env;

/// The only schema this build reads or writes.
pub const STATE_VERSION: u32 = 1;

/// Per-mode entry cap, so the file cannot grow without limit.
pub const MAX_ENTRIES_PER_MODE: usize = 2_000;

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

#[derive(Serialize, Deserialize)]
struct StateFile {
    version: u32,
    #[serde(default)]
    modes: BTreeMap<String, BTreeMap<String, Entry>>,
}

impl State {
    /// Load state, printing a warning on stderr for a corrupt or unknown file.
    pub fn load(env: &Env) -> Self {
        let (state, warnings) = Self::load_with_warnings(env);
        for warning in warnings {
            eprintln!("gofi: warning: {warning}");
        }
        state
    }

    /// Load state and return the warnings instead of printing them. A missing
    /// file is not a warning; a corrupt one, an unreadable one or an unknown
    /// version yields an empty state and one warning.
    pub fn load_with_warnings(env: &Env) -> (Self, Vec<String>) {
        let path = env.state_file();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return (Self::default(), Vec::new());
            }
            Err(error) => {
                return (
                    Self::default(),
                    vec![format!("could not read {}: {error}", path.display())],
                );
            }
        };

        let file: StateFile = match serde_json::from_slice(&bytes) {
            Ok(file) => file,
            Err(error) => {
                return (
                    Self::default(),
                    vec![format!("ignoring corrupt {}: {error}", path.display())],
                );
            }
        };
        if file.version != STATE_VERSION {
            return (
                Self::default(),
                vec![format!(
                    "ignoring {} with unsupported version {}",
                    path.display(),
                    file.version
                )],
            );
        }

        let mut state = Self::default();
        for (name, entries) in file.modes {
            // Unknown mode names are ignored; clipboard can never be populated.
            let Some(mode) = Mode::parse(&name) else {
                continue;
            };
            if let Some(frecency) = state.frecency_mut(mode) {
                frecency.entries = entries.into_iter().collect();
            }
        }
        (state, Vec::new())
    }

    /// Write state atomically. Creates the parent directory, takes an advisory
    /// lock, writes and fsyncs a temp file, then renames it over the target.
    pub fn save(&self, env: &Env) -> Result<(), StoreError> {
        let path = env.state_file();
        let dir = path
            .parent()
            .ok_or_else(|| StoreError(format!("state path has no parent: {}", path.display())))?;
        fs::create_dir_all(dir)
            .map_err(|error| StoreError(format!("could not create {}: {error}", dir.display())))?;

        // Lock a sidecar file: the state file itself is replaced by rename, so a
        // lock held on it would guard an inode that is no longer the target.
        let lock_path = dir.join("state.json.lock");
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| {
                StoreError(format!("could not open {}: {error}", lock_path.display()))
            })?;
        lock.lock_exclusive().map_err(|error| {
            StoreError(format!("could not lock {}: {error}", lock_path.display()))
        })?;

        let file = StateFile {
            version: STATE_VERSION,
            modes: self.bounded_modes(frecency::now_secs()),
        };
        let json = serde_json::to_vec_pretty(&file)
            .map_err(|error| StoreError(format!("could not encode state: {error}")))?;

        let tmp = dir.join("state.json.tmp");
        let result = write_atomic(&tmp, &path, &json);
        // Dropping the handle releases the lock; the kernel does it on exit too.
        drop(lock);
        result
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

    /// Both persisted modes, capped, in a stable key order.
    fn bounded_modes(&self, now: u64) -> BTreeMap<String, BTreeMap<String, Entry>> {
        let mut modes = BTreeMap::new();
        for (mode, frecency) in [(Mode::Apps, &self.apps), (Mode::Run, &self.run)] {
            modes.insert(mode.as_str().to_string(), bounded(&frecency.entries, now));
        }
        modes
    }
}

/// Cap one mode's entries, keeping the highest decayed scores.
fn bounded(entries: &HashMap<String, Entry>, now: u64) -> BTreeMap<String, Entry> {
    let mut kept: Vec<(&String, &Entry)> = entries.iter().collect();
    if kept.len() > MAX_ENTRIES_PER_MODE {
        kept.sort_by(|(left_key, left), (right_key, right)| {
            let left_score = frecency::decay(left.score, left.last_used, now);
            let right_score = frecency::decay(right.score, right.last_used, now);
            right_score
                .total_cmp(&left_score)
                .then_with(|| left_key.cmp(right_key))
        });
        kept.truncate(MAX_ENTRIES_PER_MODE);
    }
    kept.into_iter()
        .map(|(key, entry)| (key.clone(), entry.clone()))
        .collect()
}

fn write_atomic(tmp: &Path, path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let write = || -> std::io::Result<()> {
        let mut file = File::create(tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(tmp, path)?;
        // Best effort: the rename itself is atomic, this only makes it durable.
        if let Some(dir) = path.parent()
            && let Ok(dir) = File::open(dir)
        {
            let _ = dir.sync_all();
        }
        Ok(())
    };
    write().map_err(|error| {
        let _ = fs::remove_file(tmp);
        StoreError(format!("could not write {}: {error}", path.display()))
    })
}

fn empty_frecency() -> &'static Frecency {
    static EMPTY: OnceLock<Frecency> = OnceLock::new();
    EMPTY.get_or_init(Frecency::default)
}
