//! Where a command's reports go: real hardware, hardware while recording, or a fixture.

use std::path::{Path, PathBuf};
use std::time::Duration;

use openkeychron_hid::{
    DeviceIdentity, Event, Fixture, FixtureError, HidInterface, HidTransport, HidapiTransport,
    KeychronDevice, RecordingTransport, ReplayTransport, TransportError, VIA_USAGE, VIA_USAGE_PAGE,
    keychron_via_interfaces,
};
use openkeychron_via::ViaError;

#[derive(Debug)]
pub enum SessionError {
    Usage(String),
    Fixture(FixtureError),
    Discovery(openkeychron_hid::DiscoveryError),
    Open(openkeychron_hid::OpenError),
    Via(String),
    FixtureNotFinished { count: usize },
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) | Self::Via(message) => formatter.write_str(message),
            Self::Fixture(error) => error.fmt(formatter),
            Self::Discovery(error) => error.fmt(formatter),
            Self::Open(error) => error.fmt(formatter),
            Self::FixtureNotFinished { count } => write!(
                formatter,
                "the session stopped before the end of the fixture; {count} recorded events \
                 were never reached"
            ),
        }
    }
}

impl From<FixtureError> for SessionError {
    fn from(error: FixtureError) -> Self {
        Self::Fixture(error)
    }
}

impl From<openkeychron_hid::DiscoveryError> for SessionError {
    fn from(error: openkeychron_hid::DiscoveryError) -> Self {
        Self::Discovery(error)
    }
}

impl From<openkeychron_hid::OpenError> for SessionError {
    fn from(error: openkeychron_hid::OpenError) -> Self {
        Self::Open(error)
    }
}

impl SessionError {
    /// Adds the receiver caveat to a timeout: through the Keychron Link, a timeout alone does
    /// not show why nothing answered (docs/protocol/keychron-link.md).
    pub fn via(error: &ViaError, through_receiver: bool) -> Self {
        let timed_out = *error == ViaError::Transport(TransportError::Timeout);
        if timed_out && through_receiver {
            Self::Via(format!(
                "{error}. Through a receiver this is inconclusive; try the keyboard on a USB \
                 cable (see docs/protocol/keychron-link.md)"
            ))
        } else {
            Self::Via(error.to_string())
        }
    }
}

/// Lists devices: the fixture's device when replaying, otherwise the connected ones.
pub fn list(fixture: Option<&Path>) -> Result<Vec<KeychronDevice>, SessionError> {
    match fixture {
        Some(path) => Ok(vec![fixture_device(&Fixture::load(path)?, path)?]),
        None => Ok(openkeychron_hid::list_keychron_devices()?),
    }
}

/// One device's VIA channel, ready for commands.
#[derive(Debug)]
pub struct Session {
    device: KeychronDevice,
    link: Link,
}

#[derive(Debug)]
enum Link {
    Replay(ReplayTransport),
    Hardware(HidapiTransport),
    Recording {
        transport: RecordingTransport<HidapiTransport>,
        path: PathBuf,
    },
}

impl Session {
    pub fn start(fixture: Option<&Path>, record: Option<&Path>) -> Result<Self, SessionError> {
        if let Some(path) = fixture {
            let fixture = Fixture::load(path)?;
            return Ok(Self {
                device: fixture_device(&fixture, path)?,
                link: Link::Replay(ReplayTransport::new(fixture)),
            });
        }

        let device = only_device(openkeychron_hid::list_keychron_devices()?)?;
        let transport = openkeychron_hid::open(&device)?;
        let link = match record {
            Some(path) => Link::Recording {
                transport: RecordingTransport::new(transport),
                path: path.to_owned(),
            },
            None => Link::Hardware(transport),
        };
        Ok(Self { device, link })
    }

    pub fn device(&self) -> &KeychronDevice {
        &self.device
    }

    /// Ends the session: a replay must have used every recorded event, and a recording is
    /// written to its file.
    pub fn finish(self) -> Result<(), SessionError> {
        match self.link {
            Link::Hardware(_) => Ok(()),
            Link::Replay(replay) => {
                replay
                    .finish()
                    .map_err(|remaining: Vec<Event>| SessionError::FixtureNotFinished {
                        count: remaining.len(),
                    })
            }
            Link::Recording { transport, path } => {
                let identity = DeviceIdentity {
                    name: self
                        .device
                        .name
                        .clone()
                        .unwrap_or_else(|| "(no product name)".to_owned()),
                    vendor_id: self.device.vendor_id,
                    product_id: self.device.product_id,
                    usage_page: VIA_USAGE_PAGE,
                    usage: VIA_USAGE,
                };
                transport.into_fixture(identity).save(&path)?;
                Ok(())
            }
        }
    }
}

impl HidTransport for Session {
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        match &mut self.link {
            Link::Replay(transport) => transport.write(report),
            Link::Hardware(transport) => transport.write(report),
            Link::Recording { transport, .. } => transport.write(report),
        }
    }

    fn read(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError> {
        match &mut self.link {
            Link::Replay(transport) => transport.read(timeout),
            Link::Hardware(transport) => transport.read(timeout),
            Link::Recording { transport, .. } => transport.read(timeout),
        }
    }
}

/// Commands that talk to a device need exactly one; picking between several is not built yet.
fn only_device(mut devices: Vec<KeychronDevice>) -> Result<KeychronDevice, SessionError> {
    match devices.len() {
        1 => Ok(devices.remove(0)),
        0 => Err(SessionError::Usage(
            "no Keychron device with a VIA interface found; run `openkeychron list`".to_owned(),
        )),
        count => Err(SessionError::Usage(format!(
            "{count} Keychron devices found; connect only one (see `openkeychron list`)"
        ))),
    }
}

/// The device a fixture was recorded from, as `list` would have shown it. A fixture from
/// anything other than a Keychron VIA interface is refused rather than replayed.
fn fixture_device(fixture: &Fixture, path: &Path) -> Result<KeychronDevice, SessionError> {
    let identity = &fixture.device;
    let interface = HidInterface {
        name: Some(identity.name.clone()),
        vendor_id: identity.vendor_id,
        product_id: identity.product_id,
        usage_page: identity.usage_page,
        usage: identity.usage,
        path: format!("fixture:{}", path.display()),
    };
    keychron_via_interfaces([interface])
        .into_iter()
        .next()
        .ok_or_else(|| {
            SessionError::Usage(format!(
                "{} was not recorded from a Keychron VIA interface ({:04x}:{:04x}, usage page \
             0x{:04x}, usage 0x{:02x})",
                path.display(),
                identity.vendor_id,
                identity.product_id,
                identity.usage_page,
                identity.usage
            ))
        })
}
