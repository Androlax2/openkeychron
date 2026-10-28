//! Keychron-specific commands: RGB, debounce, LKP and wireless timers.
//!
//! IDs, payloads and firmware behaviour are documented, with source links, in
//! `docs/protocol/keychron.md`. Every setter reads the value back and fails on a mismatch.

mod client;
mod firmware;
mod settings;
mod wireless;

pub use client::{Features, Keychron, KeychronError, MiscFeatures};
pub use firmware::{FirmwareVersion, KNOWN_PROTOCOL_VERSION, KeychronProtocol, NotReported};
pub use settings::{Debounce, DebounceAlgorithm, SnapClickMode, SnapClickPair};
pub use wireless::{MIN_BACKLIGHT_OFF_SECONDS, MIN_SLEEP_SECONDS, WirelessTimers};
