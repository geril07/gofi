//! Motion layer: every spring and transition lives here.
//!
//! `GOFI_MOTION=off` (also `0`, `false`, `no`, `none`) makes every change land
//! at once. The skin renders identically at rest; only the paths between states
//! differ. Deleting this module and its call sites leaves a working static UI.

use rune::{Spring, SpringPair, Transition};

/// The selection pill's longest glide, for a jump across the visible window.
/// No bounce: the travel is a slide, not an overshoot, and a one-row move lands
/// at once, so this only covers the jumps.
const SELECT_LONGEST: f32 = 0.24;
/// The shortest animated glide, two slots away.
const SELECT_SHORTEST: f32 = 0.14;
/// Slots from the first to the last visible row (seven rows).
const SELECT_SPAN: usize = 6;

/// The selection pill's spring for a move of `slots` rows.
///
/// One row lands at once: it is the arrow-key case, and a glide there trails
/// the highlight under key repeat. Further moves glide on a critically damped
/// spring whose duration grows with the square root of the distance, so two
/// rows do not crawl and a jump across the window does not snap.
pub fn select(slots: usize) -> Spring {
    if slots <= 1 {
        return Spring::NONE;
    }

    let share = (slots.min(SELECT_SPAN) as f32 / SELECT_SPAN as f32).sqrt();
    let duration = (SELECT_LONGEST * share).clamp(SELECT_SHORTEST, SELECT_LONGEST);

    Spring::new(duration, 0.0)
}

/// The tab indicator's shared-element flight.
pub const TAB: Spring = Spring::new(0.16, 0.2);
/// A mode swap: enter slightly slower than exit.
pub const MODE: SpringPair = SpringPair::new(Spring::new(0.14, 0.0), Spring::new(0.11, 0.0));
/// How far a mode list slides in and out.
pub const MODE_SHIFT: f32 = 10.0;

/// The mode swap for a forward (`1.0`) or backward (`-1.0`) tab move.
///
/// The incoming list slides from the direction of travel; the outgoing one fades
/// in place, because its transition was authored before the new direction was
/// known and would otherwise always exit the same way.
pub fn mode_transition(direction: f32) -> Transition {
    let shift = if direction < 0.0 {
        -MODE_SHIFT
    } else {
        MODE_SHIFT
    };
    let enter = Transition::offset(shift, 0.0).combined(Transition::opacity());
    Transition::asymmetric(enter, Transition::opacity()).springs(MODE)
}

pub struct Motion {
    enabled: bool,
}

impl Motion {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// The process-wide switch, read from `GOFI_MOTION` once.
    pub fn get() -> &'static Motion {
        static MOTION: std::sync::OnceLock<Motion> = std::sync::OnceLock::new();
        MOTION.get_or_init(|| Self::new(enabled_from(std::env::var("GOFI_MOTION").ok().as_deref())))
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn spring(&self, spring: Spring) -> Spring {
        if self.enabled { spring } else { Spring::NONE }
    }

    pub fn transition(&self, transition: Transition) -> Transition {
        if self.enabled {
            transition
        } else {
            Transition::none()
        }
    }
}

fn enabled_from(value: Option<&str>) -> bool {
    match value {
        Some(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "no" | "none"
        ),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_parses_and_still_lands_every_change() {
        for value in ["off", "0", "false", "no", "none", " OFF "] {
            assert!(!enabled_from(Some(value)), "{value:?}");
        }
        for value in ["on", "1", "true", "yes", "full"] {
            assert!(enabled_from(Some(value)), "{value:?}");
        }
        assert!(enabled_from(None));

        let off = Motion::new(false);
        assert_eq!(off.spring(select(2)), Spring::NONE);
        assert_eq!(off.transition(Transition::opacity()), Transition::none());
    }

    #[test]
    fn on_keeps_the_authored_motion() {
        let on = Motion::new(true);
        assert_eq!(on.spring(select(2)), select(2));
        assert_eq!(on.transition(Transition::opacity()), Transition::opacity());
    }

    #[test]
    fn one_row_lands_at_once_and_a_jump_stretches_without_bounce() {
        assert_eq!(select(0), Spring::NONE);
        assert_eq!(select(1), Spring::NONE);
        assert_eq!(select(2).duration, SELECT_SHORTEST);
        assert_eq!(select(SELECT_SPAN).duration, SELECT_LONGEST);

        let mut previous = 0.0;

        for slots in 2..=SELECT_SPAN {
            let spring = select(slots);
            assert_eq!(spring.bounce, 0.0, "{slots} slots");
            assert!(spring.duration >= previous, "{slots} slots: {spring:?}");
            assert!((SELECT_SHORTEST..=SELECT_LONGEST).contains(&spring.duration));
            previous = spring.duration;
        }

        // Past the window the glide is already at its longest.
        assert_eq!(select(60), select(SELECT_SPAN));
    }
}
