//! Fuzzy ranking: a nucleo match score blended with per-item frecency.
//!
//! The policy is ADR 0004. The matcher is [`nucleo_matcher`]; this module only
//! decides what to match, how to blend frecency and how to order the result.

pub mod frecency;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::model::{Item, Mode};
use frecency::Frecency;

/// How much of the best match score frecency may add at most (ADR 0004).
const FRECENCY_WEIGHT: f64 = 0.15;

/// Scale applied to the frecency-only sort key of an empty query. Ordering is
/// unaffected; it keeps [`Ranked::score`] readable in `--dump` output.
const EMPTY_QUERY_SCORE_SCALE: f64 = 100.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub index: usize,
    /// The sort key rounded to an integer. Ordering uses the unrounded value.
    pub score: i64,
    /// Character indices in `Item::title` to underline.
    pub positions: Vec<u32>,
}

/// Reusable matcher state. Build one per query and rank many items with it so
/// the pattern is parsed once and the scratch buffers are not reallocated.
pub struct Ranker {
    matcher: Matcher,
    pattern: Pattern,
    buf: Vec<char>,
    indices: Vec<u32>,
}

impl Default for Ranker {
    fn default() -> Self {
        Self::new()
    }
}

impl Ranker {
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
            pattern: Pattern::default(),
            buf: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Rank `items` against `query` at the current time.
    pub fn rank(
        &mut self,
        items: &[Item],
        query: &str,
        mode: Mode,
        frecency: &Frecency,
    ) -> Vec<Ranked> {
        self.rank_at(items, query, mode, frecency, frecency::now_secs())
    }

    /// Rank with an explicit clock, so tests can pin decay.
    pub fn rank_at(
        &mut self,
        items: &[Item],
        query: &str,
        mode: Mode,
        frecency: &Frecency,
        now: u64,
    ) -> Vec<Ranked> {
        self.pattern
            .reparse(query, CaseMatching::Smart, Normalization::Smart);
        let empty_query = self.pattern.atoms.is_empty();

        // cliphist owns the order: filter, never reorder, never blend frecency.
        if mode == Mode::Clipboard {
            return self.filter_preserving_order(items);
        }
        if empty_query {
            return rank_by_frecency(items, frecency, now);
        }

        let mut matched = Vec::new();
        for (index, item) in items.iter().enumerate() {
            if let Some((match_score, positions)) = self.score_item(item) {
                matched.push((index, match_score, positions));
            }
        }
        let top = matched
            .iter()
            .map(|(_, score, _)| *score)
            .max()
            .unwrap_or(0);

        let mut ranked: Vec<(f64, Ranked)> = matched
            .into_iter()
            .map(|(index, match_score, positions)| {
                let weight = frecency::normalize(frecency.score(items[index].id.as_str(), now));
                let composite = f64::from(match_score) + FRECENCY_WEIGHT * f64::from(top) * weight;
                (
                    composite,
                    Ranked {
                        index,
                        score: composite.round() as i64,
                        positions,
                    },
                )
            })
            .collect();
        sort_by_composite(&mut ranked, items);
        ranked.into_iter().map(|(_, ranked)| ranked).collect()
    }

    /// Keep provider order, drop non-matches.
    fn filter_preserving_order(&mut self, items: &[Item]) -> Vec<Ranked> {
        let mut out = Vec::new();
        for (index, item) in items.iter().enumerate() {
            if let Some((score, positions)) = self.score_item(item) {
                out.push(Ranked {
                    index,
                    score: i64::from(score),
                    positions,
                });
            }
        }
        out
    }

    /// Score one item and collect the underline positions inside its title.
    ///
    /// `search_text` is what the matcher scores (title plus keywords); only the
    /// characters that also occur in the title can be underlined, so a match
    /// found solely in a keyword yields no positions.
    fn score_item(&mut self, item: &Item) -> Option<(u32, Vec<u32>)> {
        let title = item.title.as_str();
        let haystack = if item.search_text.is_empty() {
            title
        } else {
            item.search_text.as_str()
        };

        self.indices.clear();
        if haystack == title {
            let score = self.pattern.indices(
                Utf32Str::new(haystack, &mut self.buf),
                &mut self.matcher,
                &mut self.indices,
            )?;
            sort_dedup(&mut self.indices);
            return Some((score, std::mem::take(&mut self.indices)));
        }

        let score = self
            .pattern
            .score(Utf32Str::new(haystack, &mut self.buf), &mut self.matcher)?;
        self.indices.clear();
        let positions = match self.pattern.indices(
            Utf32Str::new(title, &mut self.buf),
            &mut self.matcher,
            &mut self.indices,
        ) {
            Some(_) => {
                sort_dedup(&mut self.indices);
                std::mem::take(&mut self.indices)
            }
            None => {
                self.indices.clear();
                Vec::new()
            }
        };
        Some((score, positions))
    }
}

/// Rank `items` once. Convenience for one-shot callers; reuse a [`Ranker`] in a
/// UI loop instead.
pub fn rank(items: &[Item], query: &str, mode: Mode, frecency: &Frecency) -> Vec<Ranked> {
    Ranker::new().rank(items, query, mode, frecency)
}

/// Rank at an explicit time. Tests use this to pin frecency decay.
pub fn rank_at(
    items: &[Item],
    query: &str,
    mode: Mode,
    frecency: &Frecency,
    now: u64,
) -> Vec<Ranked> {
    Ranker::new().rank_at(items, query, mode, frecency, now)
}

/// Empty query: most frecent first, then title.
fn rank_by_frecency(items: &[Item], frecency: &Frecency, now: u64) -> Vec<Ranked> {
    let mut ranked: Vec<(f64, Ranked)> = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let weight = frecency::normalize(frecency.score(item.id.as_str(), now));
            let composite = weight * EMPTY_QUERY_SCORE_SCALE;
            (
                composite,
                Ranked {
                    index,
                    score: composite.round() as i64,
                    positions: Vec::new(),
                },
            )
        })
        .collect();
    sort_by_composite(&mut ranked, items);
    ranked.into_iter().map(|(_, ranked)| ranked).collect()
}

/// Composite descending, then title ascending. Titles compare case-insensitively
/// so a mixed-case corpus reads as alphabetical (`ark` before `Zed`), with the
/// raw bytes only breaking case-insensitive equals.
fn sort_by_composite(ranked: &mut [(f64, Ranked)], items: &[Item]) {
    ranked.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| title_order(&items[left.index].title, &items[right.index].title))
    });
}

fn title_order(left: &str, right: &str) -> std::cmp::Ordering {
    left.chars()
        .flat_map(char::to_lowercase)
        .cmp(right.chars().flat_map(char::to_lowercase))
        .then_with(|| left.cmp(right))
}

fn sort_dedup(indices: &mut Vec<u32>) {
    indices.sort_unstable();
    indices.dedup();
}
