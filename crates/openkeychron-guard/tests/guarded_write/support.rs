//! Shared by the guarded write tests: synthetic fixtures, the device they were made for, and an
//! error type that tells the caller's own `keep` failure apart from the library's errors.

use std::path::PathBuf;

use openkeychron_guard::{GuardError, WriteVerified, require_write_verified};
use openkeychron_hid::{Fixture, KeychronDevice, ReplayTransport};

/// The V5 Max ISO on a USB cable, as every synthetic write fixture records it.
pub fn v5_max_on_cable() -> KeychronDevice {
    KeychronDevice {
        name: Some("synthetic V5 Max ISO".to_owned()),
        vendor_id: 0x3434,
        product_id: 0x0951,
        path: "fixture".to_owned(),
        is_receiver: false,
    }
}

pub fn verified() -> WriteVerified {
    require_write_verified("test writes", &v5_max_on_cable()).unwrap()
}

pub fn fixture(name: &str) -> Fixture {
    Fixture::load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/synthetic")
            .join(name),
    )
    .unwrap()
}

/// The first `events` of `name`: the reads a write makes before it may send anything.
pub fn first_events(name: &str, events: usize) -> ReplayTransport {
    let mut fixture = fixture(name);
    fixture.events.truncate(events);
    ReplayTransport::new(fixture)
}

/// Fails unless `transport`, replaying the whole of `name`, stopped right after its first
/// `events`: a write sent too early would have used the events after them.
pub fn assert_stopped_after(transport: ReplayTransport, name: &str, events: usize) {
    assert_eq!(
        transport.finish(),
        Err(fixture(name).events[events..].to_vec())
    );
}

pub fn backup_json<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/synthetic")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[derive(Debug, PartialEq, Eq)]
pub enum TestError {
    Guard(GuardError),
    /// What a `keep` step returns when it could not keep the backup.
    NotKept,
    Other(String),
}

impl From<GuardError> for TestError {
    fn from(error: GuardError) -> Self {
        Self::Guard(error)
    }
}

impl From<openkeychron_keychron::KeychronError> for TestError {
    fn from(error: openkeychron_keychron::KeychronError) -> Self {
        Self::Other(error.to_string())
    }
}

impl From<openkeychron_via::ViaError> for TestError {
    fn from(error: openkeychron_via::ViaError) -> Self {
        Self::Other(error.to_string())
    }
}

pub fn refuse_to_keep<B>(_: &B) -> Result<(), TestError> {
    Err(TestError::NotKept)
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "a keep step returns a Result, and this one always succeeds"
)]
pub fn keep_nothing<B>(_: &B) -> Result<(), TestError> {
    Ok(())
}
