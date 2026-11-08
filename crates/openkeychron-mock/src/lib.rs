//! A stateful fake Keychron keyboard behind [`openkeychron_hid::HidTransport`].
//!
//! A [`MockKeyboard`] holds a keymap, encoder keys, macros, the Keychron settings and the
//! lighting. Every [`MockTransport`] it hands out answers VIA and Keychron requests from that
//! one state and applies writes to it, so a write is visible to the next read, even through
//! another transport. The app's mock mode uses it to serve a whole UI session on one device; tests use
//! it to drive a write and its undo end to end.
//!
//! It answers only the requests listed in `docs/protocol/` that openkeychron sends for the
//! keymap, macros, `info`, the settings and the lighting. Where `docs/protocol/keychron.md` says the firmware
//! refuses a value with status 1, it does the same. Any other request, any request out of range,
//! and any value the firmware would silently change, fails the transport write with
//! [`TransportError::Device`](openkeychron_hid::TransportError) naming it: the fake never makes
//! up a firmware answer.

mod keyboard;
mod keychron;
mod lighting;
mod via;

pub use keyboard::{MOCK_FORMAT_VERSION, MockError, MockKeyboard, MockTransport};
