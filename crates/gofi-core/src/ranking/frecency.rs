//! Frecency: one decayed use counter per item, plus its normalization.
//!
//! The model is in ADR 0004: `score = score * 0.5^(elapsed / HALF_LIFE) + 1.0`
//! on use, and the same decay applied on read so unused items fade without a
//! write. Constants live here so ranking and state cannot drift apart.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Frecency halves every 14 days.
pub const HALF_LIFE_SECS: u64 = 14 * 24 * 60 * 60;

/// `K` in `score / (score + K)`: the use count that normalizes to 0.5.
pub const NORMALIZE_K: f64 = 10.0;

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
    /// Decayed score at `now`. Does not write.
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

/// Exponential decay. A zero, negative or non-finite score decays to zero, and
/// a timestamp in the future (clock skew) is treated as no elapsed time.
pub fn decay(score: f64, last_used: u64, now: u64) -> f64 {
    if !score.is_finite() || score <= 0.0 {
        return 0.0;
    }
    let elapsed = now.saturating_sub(last_used) as f64;
    score * 0.5_f64.powf(elapsed / HALF_LIFE_SECS as f64)
}

/// Map an unbounded frecency score to `0.0..1.0`, saturating.
pub fn normalize(score: f64) -> f64 {
    if !score.is_finite() || score <= 0.0 {
        return 0.0;
    }
    score / (score + NORMALIZE_K)
}

/// Unix seconds. Ranking and state share this clock; tests pass their own.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}
