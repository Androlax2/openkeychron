//! Raw HID transport and Keychron device discovery.
//!
//! Protocol crates talk to a device only through [`HidTransport`], so the same code runs
//! against real hardware, a [`ReplayTransport`] built from a recorded [`Fixture`], or a
//! [`RecordingTransport`] that captures a session into a new fixture.

mod fixture;
mod hex;
mod record;
mod replay;
mod transport;

pub use fixture::{DeviceIdentity, Event, FIXTURE_FORMAT_VERSION, Fixture, FixtureError};
pub use record::RecordingTransport;
pub use replay::ReplayTransport;
pub use transport::{HidTransport, TransportError};
