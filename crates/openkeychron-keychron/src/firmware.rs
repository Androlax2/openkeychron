//! Keychron's protocol version (`0xA0`) and firmware version (`0xA1`), from
//! `keyboards/keychron/common/keychron_raw_hid.c` (`kc_raw_hid_rx`, `get_firmware_version`) at
//! Keychron/qmk_firmware `2b493ff9`. See `docs/protocol/keychron.md`.
//!
//! Hardware verification is pending: the reply layout comes from source only (#20).

use openkeychron_hid::HidTransport;

use crate::client::{Keychron, KeychronError};

const GET_PROTOCOL_VERSION: u8 = 0xA0;
const GET_FIRMWARE_VERSION: u8 = 0xA1;

/// The only Keychron protocol version whose `0xA1` layout has been read in source.
pub const KNOWN_PROTOCOL_VERSION: u8 = 0x02;

/// The reply to `0xA0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeychronProtocol {
    pub version: u8,
    /// `QMK_COMMAND_SET` (2 in the source read).
    pub command_set: u8,
}

/// The reply to `0xA1`: `v<version> <QMK_BUILDDATE>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareVersion {
    /// For example `v1.1.0`.
    pub version: String,
    /// The build date as the firmware prints it, for example `2025-03-17-10:00:00`.
    pub build_date: String,
}

/// Why the Keychron firmware version is not shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotReported {
    /// `0xA0` came back as `id_unhandled`: the firmware does not speak Keychron's protocol.
    NoKeychronProtocol,
    /// The firmware speaks a protocol version whose `0xA1` layout has not been read.
    UnknownProtocolVersion(u8),
}

impl<T: HidTransport> Keychron<T> {
    /// `0xA0`. `None` when the firmware does not handle it.
    pub fn protocol(&mut self) -> Result<Option<KeychronProtocol>, KeychronError> {
        match self.exchange(&[GET_PROTOCOL_VERSION]) {
            Ok(reply) => Ok(Some(KeychronProtocol {
                version: reply[1],
                command_set: reply[3],
            })),
            Err(KeychronError::Unhandled(_)) => Ok(None),
            Err(other) => Err(other),
        }
    }

    /// `0xA0` as a guard, then `0xA1`. `Ok(Err(reason))` when the firmware does not report a
    /// version this build can read; a malformed `0xA1` reply is an error.
    pub fn firmware_version(
        &mut self,
    ) -> Result<Result<FirmwareVersion, NotReported>, KeychronError> {
        let Some(protocol) = self.protocol()? else {
            return Ok(Err(NotReported::NoKeychronProtocol));
        };
        if protocol.version != KNOWN_PROTOCOL_VERSION {
            return Ok(Err(NotReported::UnknownProtocolVersion(protocol.version)));
        }
        let reply = self.exchange(&[GET_FIRMWARE_VERSION])?;
        parse_firmware_version(&reply[1..]).map(Ok)
    }
}

/// Parses `v<digits and dots> <build date>`, NUL-terminated or filling the report.
fn parse_firmware_version(bytes: &[u8]) -> Result<FirmwareVersion, KeychronError> {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    let text = std::str::from_utf8(&bytes[..end])
        .ok()
        .filter(|text| {
            text.chars()
                .all(|character| character.is_ascii_graphic() || character == ' ')
        })
        .ok_or_else(|| malformed(bytes))?;
    let (version, build_date) = text.split_once(' ').ok_or_else(|| malformed(bytes))?;
    let digits = version.strip_prefix('v').ok_or_else(|| malformed(bytes))?;
    let well_formed = !digits.is_empty()
        && digits
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_hexdigit()));
    if !well_formed || build_date.is_empty() {
        return Err(malformed(bytes));
    }
    Ok(FirmwareVersion {
        version: version.to_owned(),
        build_date: build_date.to_owned(),
    })
}

fn malformed(bytes: &[u8]) -> KeychronError {
    KeychronError::Invalid(format!(
        "the 0xa1 reply is not `v<version> <build date>`: {:02x?}",
        &bytes[..bytes.len().min(31)]
    ))
}

#[cfg(test)]
mod tests {
    use openkeychron_hid::{DeviceIdentity, Event, Fixture, ReplayTransport};

    use super::{FirmwareVersion, NotReported};
    use crate::client::{Keychron, KeychronError, REPORT_LEN};

    fn report(prefix: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0u8; REPORT_LEN];
        bytes[..prefix.len()].copy_from_slice(prefix);
        bytes
    }

    fn exchange(request: &[u8], reply: &[u8]) -> Vec<Event> {
        vec![
            Event::Write {
                data: report(request),
            },
            Event::Read {
                data: report(reply),
            },
        ]
    }

    fn version_reply(text: &str) -> Vec<u8> {
        let mut reply = vec![0xa1];
        reply.extend_from_slice(text.as_bytes());
        reply
    }

    fn keychron(sessions: &[Vec<Event>]) -> Keychron<ReplayTransport> {
        let device = DeviceIdentity {
            name: "synthetic".to_owned(),
            vendor_id: 0x3434,
            product_id: 0x0951,
            usage_page: 0xff60,
            usage: 0x61,
        };
        Keychron::new(ReplayTransport::new(Fixture::new(
            device,
            sessions.concat(),
        )))
    }

    #[test]
    fn firmware_version_is_read_after_the_protocol_guard() {
        let mut keychron = keychron(&[
            exchange(&[0xa0], &[0xa0, 0x02, 0x00, 0x02]),
            exchange(&[0xa1], &version_reply("v1.1.0 2025-03-17-10:00:00")),
        ]);
        assert_eq!(
            keychron.firmware_version(),
            Ok(Ok(FirmwareVersion {
                version: "v1.1.0".to_owned(),
                build_date: "2025-03-17-10:00:00".to_owned(),
            }))
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn firmware_version_is_not_reported_without_the_keychron_protocol() {
        let mut keychron = keychron(&[exchange(&[0xa0], &[0xff])]);
        assert_eq!(
            keychron.firmware_version(),
            Ok(Err(NotReported::NoKeychronProtocol))
        );
        // 0xA1 was never sent: the fixture ends after the guard.
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn firmware_version_is_not_reported_for_an_unknown_protocol_version() {
        let mut keychron = keychron(&[exchange(&[0xa0], &[0xa0, 0x03, 0x00, 0x02])]);
        assert_eq!(
            keychron.firmware_version(),
            Ok(Err(NotReported::UnknownProtocolVersion(3)))
        );
        assert_eq!(keychron.into_transport().finish(), Ok(()));
    }

    #[test]
    fn firmware_version_rejects_a_reply_without_a_build_date() {
        let mut keychron = keychron(&[
            exchange(&[0xa0], &[0xa0, 0x02, 0x00, 0x02]),
            exchange(&[0xa1], &version_reply("v1.1.0")),
        ]);
        assert!(matches!(
            keychron.firmware_version(),
            Err(KeychronError::Invalid(_))
        ));
    }

    #[test]
    fn firmware_version_rejects_a_reply_that_is_not_text() {
        let mut keychron = keychron(&[
            exchange(&[0xa0], &[0xa0, 0x02, 0x00, 0x02]),
            exchange(&[0xa1], &[0xa1, 0x76, 0x01, 0xfe, 0x20, 0x32]),
        ]);
        assert!(matches!(
            keychron.firmware_version(),
            Err(KeychronError::Invalid(_))
        ));
    }

    #[test]
    fn firmware_version_accepts_a_two_digit_major_version() {
        // DEVICE_VER >= 0x1000 prints an extra leading hex digit.
        let mut keychron = keychron(&[
            exchange(&[0xa0], &[0xa0, 0x02, 0x00, 0x02]),
            exchange(&[0xa1], &version_reply("v12.0.3 2026-01-02-03:04:05")),
        ]);
        assert_eq!(
            keychron
                .firmware_version()
                .map(|version| version.map(|v| v.version)),
            Ok(Ok("v12.0.3".to_owned()))
        );
    }
}
