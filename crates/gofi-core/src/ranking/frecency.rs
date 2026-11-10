//! Frecency decay. Worker B fills this in M1.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const HALF_LIFE_SECS: u64 = 14 * 24 * 60 * 60;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Entry {
    pub score: f64,
    pub last_used: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Frecency {
    pub entries: HashMap<String, Entry>,
}

impl Frecency {
    /// Decayed score at `now`.
    pub fn score(&self, key: &str, now: u64) -> f64 {
        self.entries
            .get(key)
            .map(|entry| decay(entry.score, entry.last_used, now))
            .unwrap_or(0.0)
    }

    /// Record one use at `now`, decaying the previous score first.
    pub fn record(&mut self, key: &str, now: u64) {
        let score = self.score(key, now) + 1.0;
        self.entries.insert(
            key.to_string(),
            Entry {
                score,
                last_used: now,
            },
        );
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub fn decay(score: f64, last_used: u64, now: u64) -> f64 {
    if score <= 0.0 {
        return 0.0;
    }
    let elapsed = now.saturating_sub(last_used) as f64;
    score * 0.5_f64.powf(elapsed / HALF_LIFE_SECS as f64)
}
