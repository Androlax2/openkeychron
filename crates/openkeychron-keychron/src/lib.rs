//! Keychron-specific commands: RGB, debounce, LKP and wireless timers.
//!
//! IDs, payloads and firmware behaviour are documented, with source links, in
//! `docs/protocol/keychron.md`. Every setter reads the value back and fails on a mismatch.

mod client;
mod settings;

pub use client::{Features, Keychron, KeychronError, MiscFeatures};
pub use settings::{Debounce, DebounceAlgorithm, SnapClickMode, SnapClickPair};
