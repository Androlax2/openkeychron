//! Runs the commands' read functions against a synthetic fixture instead of hardware.

use std::path::Path;
use std::time::Duration;

use openkeychron_hid::{
    Event, Fixture, HidInterface, HidTransport, KeychronDevice, ReplayTransport, TransportError,
    keychron_via_interfaces,
};

/// A fixture's device and its replay. The read functions take their transport by value, so a
/// test lends them the replay with [`lend`] and still calls [`Replay::finish`] afterwards.
pub struct Replay {
    pub device: KeychronDevice,
    pub transport: ReplayTransport,
}

pub fn replay(name: &str) -> Replay {
    replay_fixture(load_fixture(name))
}

/// For a test that changes the recorded events, for example to make a reply time out.
pub fn replay_fixture(fixture: Fixture) -> Replay {
    let identity = &fixture.device;
    let interface = HidInterface {
        name: Some(identity.name.clone()),
        vendor_id: identity.vendor_id,
        product_id: identity.product_id,
        usage_page: identity.usage_page,
        usage: identity.usage,
        path: "fixture".to_owned(),
    };
    let device = keychron_via_interfaces([interface]).remove(0);
    Replay {
        device,
        transport: ReplayTransport::new(fixture),
    }
}

pub fn load_fixture(name: &str) -> Fixture {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/synthetic")
        .join(name);
    Fixture::load(&path).unwrap()
}

impl Replay {
    /// The events the read never reached; empty when it used the whole fixture.
    pub fn finish(self) -> Vec<Event> {
        self.transport.finish().err().unwrap_or_default()
    }
}

pub fn lend(transport: &mut ReplayTransport) -> Lent<'_> {
    Lent(transport)
}

pub struct Lent<'a>(&'a mut ReplayTransport);

impl HidTransport for Lent<'_> {
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        self.0.write(report)
    }

    fn read(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError> {
        self.0.read(timeout)
    }
}
