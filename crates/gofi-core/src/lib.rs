//! gofi core: providers, ranking, state and the shared item model.
//!
//! This crate has no UI dependency so its tests stay fast and headless.

pub mod icons;
pub mod model;
pub mod providers;
pub mod ranking;
pub mod store;
pub mod xdg;

pub use model::{Action, Item, ItemId, ItemKind, Loaded, Mode, Provider, ProviderError};
pub use xdg::Env;
