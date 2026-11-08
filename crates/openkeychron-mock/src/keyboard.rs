use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use openkeychron_hid::{DeviceIdentity, HidTransport, TransportError};
use openkeychron_via::{DeviceRegistry, REPORT_LEN};
use serde::Deserialize;
use thiserror::Error;

use crate::{keychron, via};

/// The only mock keyboard file format this build reads.
pub const MOCK_FORMAT_VERSION: u32 = 1;

// The seed. Every keycode and macro byte starts at zero, the way the hand-written fixtures
// leave unspecified keycodes: not the V5 Max's firmware defaults.

/// Synthetic: no V5 Max has reported its layer count to openkeychron yet.
const LAYER_COUNT: u8 = 4;
/// The pinned V5 Max ISO definition has one encoder (`openkeychron_guard::keymap`).
pub(crate) const ENCODER_COUNT: u8 = 1;
/// Synthetic: no V5 Max has reported its macro count or buffer size to openkeychron yet.
const MACRO_COUNT: u8 = 16;
const MACRO_BUFFER_SIZE: usize = 512;
/// The V5 Max holds 20 pairs (`docs/protocol/keychron.md`, misc group `0x07`).
const SNAP_CLICK_PAIRS: usize = 20;
/// Symmetric defer global at 50 ms: `v5_max/info.json`'s default time
/// (`docs/protocol/keychron.md`).
const DEBOUNCE: [u8; 2] = [0, 50];
/// The firmware defaults, 600 s and 7200 s (`docs/protocol/keychron.md`, "set wireless sleep").
const WIRELESS_TIMERS: [u16; 2] = [600, 7200];

/// Why a mock keyboard could not be made. No message names the file: the caller that chose it
/// does.
#[derive(Debug, Error)]
pub enum MockError {
    #[error("cannot read the mock keyboard: {0}")]
    Read(#[source] std::io::Error),

    #[error("invalid mock keyboard: {0}")]
    Parse(#[source] toml::de::Error),

    #[error(
        "mock keyboard format_version {found} is not supported (this build reads \
         {MOCK_FORMAT_VERSION})"
    )]
    UnsupportedVersion { found: u32 },

    #[error(
        "this build has no keyboard layout for {vendor_id:04x}:{product_id:04x}, so a mock \
         keyboard cannot size its keymap"
    )]
    NoLayout { vendor_id: u16, product_id: u16 },
}

/// A fake keyboard with the identity it is listed under. Clones share one state, so a write
/// through any of its transports is visible to every later read.
#[derive(Debug, Clone)]
pub struct MockKeyboard {
    identity: DeviceIdentity,
    state: Arc<Mutex<State>>,
}

/// What the fake keyboard holds. Keycodes are kept as the big-endian bytes VIA sends, so the
/// keymap is one buffer like the firmware's dynamic keymap.
#[derive(Debug)]
pub(crate) struct State {
    pub(crate) layer_count: u8,
    pub(crate) rows: u8,
    pub(crate) columns: u8,
    /// Layer by layer, row by row, two bytes per key.
    pub(crate) keymap: Vec<u8>,
    /// Layer by layer, encoder by encoder, counter-clockwise then clockwise, two bytes each.
    pub(crate) encoders: Vec<u8>,
    pub(crate) macro_count: u8,
    pub(crate) macros: Vec<u8>,
    /// Algorithm, then time in ms.
    pub(crate) debounce: [u8; 2],
    /// Mode, key 1, key 2.
    pub(crate) snap_click: Vec<[u8; 3]>,
    /// Backlight-off, then sleep, in seconds.
    pub(crate) wireless: [u16; 2],
}

/// One connection to a [`MockKeyboard`]. Each request queues its reply for the next read.
#[derive(Debug)]
pub struct MockTransport {
    state: Arc<Mutex<State>>,
    replies: VecDeque<Vec<u8>>,
}

/// A mock keyboard file: the identity the keyboard is listed under, nothing else.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MockFile {
    #[expect(dead_code, reason = "checked before the whole file is read")]
    format_version: u32,
    device: DeviceIdentity,
}

impl MockKeyboard {
    /// A seeded keyboard listed as `identity`. Its matrix size comes from the built-in layout
    /// for that model, which it must have.
    pub fn new(identity: DeviceIdentity) -> Result<Self, MockError> {
        let registry = DeviceRegistry::built_in();
        let layout = registry
            .layout(identity.vendor_id, identity.product_id)
            .ok_or(MockError::NoLayout {
                vendor_id: identity.vendor_id,
                product_id: identity.product_id,
            })?;
        let state = State::seeded(layout.rows(), layout.columns());
        Ok(Self {
            identity,
            state: Arc::new(Mutex::new(state)),
        })
    }

    /// A seeded keyboard from a mock keyboard file (`docs/DEVELOPMENT.md`, mock mode).
    pub fn load(path: &Path) -> Result<Self, MockError> {
        // The version is read on its own first, so a newer file is refused for its version
        // rather than for a field this build does not know.
        #[derive(Deserialize)]
        struct VersionProbe {
            format_version: u32,
        }

        let text = std::fs::read_to_string(path).map_err(MockError::Read)?;
        let probe: VersionProbe = toml::from_str(&text).map_err(MockError::Parse)?;
        if probe.format_version != MOCK_FORMAT_VERSION {
            return Err(MockError::UnsupportedVersion {
                found: probe.format_version,
            });
        }
        let file: MockFile = toml::from_str(&text).map_err(MockError::Parse)?;
        Self::new(file.device)
    }

    #[must_use]
    pub fn identity(&self) -> &DeviceIdentity {
        &self.identity
    }

    /// A new connection to this keyboard's state.
    #[must_use]
    pub fn connect(&self) -> MockTransport {
        MockTransport {
            state: Arc::clone(&self.state),
            replies: VecDeque::new(),
        }
    }
}

impl State {
    fn seeded(rows: u8, columns: u8) -> Self {
        let mut state = Self {
            layer_count: LAYER_COUNT,
            rows,
            columns,
            keymap: Vec::new(),
            encoders: Vec::new(),
            macro_count: MACRO_COUNT,
            macros: vec![0; MACRO_BUFFER_SIZE],
            debounce: DEBOUNCE,
            snap_click: vec![[0; 3]; SNAP_CLICK_PAIRS],
            wireless: WIRELESS_TIMERS,
        };
        state.reset_keymap();
        state
    }

    /// Puts every matrix and encoder key back to the seed, as `dynamic_keymap_reset` puts them
    /// back to the firmware defaults.
    pub(crate) fn reset_keymap(&mut self) {
        let layers = usize::from(self.layer_count);
        self.keymap = vec![0; layers * usize::from(self.rows) * usize::from(self.columns) * 2];
        self.encoders = vec![0; layers * usize::from(ENCODER_COUNT) * 2 * 2];
    }

    fn answer(&mut self, request: &[u8]) -> Result<Vec<u8>, String> {
        // The firmware answers in the request's own buffer, so unanswered bytes echo it.
        let mut reply: [u8; REPORT_LEN] = request.try_into().map_err(|_| {
            format!(
                "the mock keyboard takes {REPORT_LEN}-byte reports, got {} bytes",
                request.len()
            )
        })?;
        // Keychron's commands are 0xA0 to 0xAB (`docs/protocol/keychron.md`); VIA's are below.
        match reply[0] {
            0xa0..=0xaf => keychron::answer(self, &mut reply)?,
            _ => via::answer(self, &mut reply)?,
        }
        Ok(reply.to_vec())
    }
}

/// The refusal for a request the mock keyboard does not implement: the request's bytes up to
/// the last non-zero one.
pub(crate) fn not_answered(request: &[u8]) -> String {
    let length = request
        .iter()
        .rposition(|byte| *byte != 0)
        .map_or(1, |last| last + 1);
    let bytes: Vec<_> = request[..length]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!(
        "the mock keyboard does not answer request {}",
        bytes.join(" ")
    )
}

impl HidTransport for MockTransport {
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        let mut state = self.state.lock().map_err(|_| {
            TransportError::Device(
                "the mock keyboard's state was left half-changed by an earlier panic".to_owned(),
            )
        })?;
        let reply = state.answer(report).map_err(TransportError::Device)?;
        self.replies.push_back(reply);
        Ok(())
    }

    /// The reply to the oldest unanswered request. Without one, the keyboard has nothing to
    /// say, as a real one would not.
    fn read(&mut self, _timeout: Duration) -> Result<Vec<u8>, TransportError> {
        self.replies.pop_front().ok_or(TransportError::Timeout)
    }
}
