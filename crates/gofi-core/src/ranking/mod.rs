//! Fuzzy ranking. Worker B fills this in M1.

pub mod frecency;

use crate::model::{Item, Mode};
use frecency::Frecency;

#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub index: usize,
    pub score: i64,
    /// Character indices in `Item::title` to underline.
    pub positions: Vec<u32>,
}

pub fn rank(items: &[Item], query: &str, mode: Mode, frecency: &Frecency) -> Vec<Ranked> {
    let _ = (query, mode, frecency);
    items
        .iter()
        .enumerate()
        .map(|(index, _)| Ranked {
            index,
            score: 0,
            positions: Vec::new(),
        })
        .collect()
}
