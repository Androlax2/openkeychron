//! `device_info`: the VIA and Keychron firmware values. Only reads.

use openkeychron_hid::{HidTransport, KeychronDevice};
use openkeychron_keychron::{KNOWN_PROTOCOL_VERSION, Keychron, NotReported};
use openkeychron_via::Via;
use serde::Serialize;

use crate::error::CommandError;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub via_protocol_version: u16,
    pub via_firmware_version: u32,
    pub layout_options: u32,
    pub uptime_ms: u64,
    pub keychron_firmware: KeychronFirmware,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum KeychronFirmware {
    Reported { version: String, build_date: String },
    NotReported { reason: String },
}

pub fn read<T: HidTransport>(
    transport: T,
    device: &KeychronDevice,
) -> Result<DeviceInfo, CommandError> {
    let mut via = Via::new(transport);
    let via_error = |error| CommandError::via(&error, device.is_receiver);
    let via_protocol_version = via.protocol_version().map_err(via_error)?;
    let via_firmware_version = via.via_firmware_version().map_err(via_error)?;
    let layout_options = via.layout_options().map_err(via_error)?;
    let uptime = via.uptime().map_err(via_error)?;
    let mut keychron = Keychron::new(via.into_transport());
    let keychron_firmware = match keychron.firmware_version()? {
        Ok(firmware) => KeychronFirmware::Reported {
            version: firmware.version,
            build_date: firmware.build_date,
        },
        Err(reason) => KeychronFirmware::NotReported {
            reason: not_reported(reason),
        },
    };
    Ok(DeviceInfo {
        via_protocol_version,
        via_firmware_version,
        layout_options,
        uptime_ms: u64::try_from(uptime.as_millis()).unwrap_or(u64::MAX),
        keychron_firmware,
    })
}

fn not_reported(reason: NotReported) -> String {
    match reason {
        NotReported::NoKeychronProtocol => {
            "the firmware does not answer Keychron command 0xa0".to_owned()
        }
        NotReported::UnknownProtocolVersion(version) => format!(
            "Keychron protocol version {version}; this build reads version {KNOWN_PROTOCOL_VERSION}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use openkeychron_hid::Event;

    use super::{DeviceInfo, KeychronFirmware, read};
    use crate::test_support::{lend, load_fixture, replay, replay_fixture};

    #[test]
    fn reads_the_via_values_and_the_keychron_firmware_version() {
        let mut replay = replay("cli-info.toml");
        let info = read(lend(&mut replay.transport), &replay.device).unwrap();
        assert_eq!(
            info,
            DeviceInfo {
                via_protocol_version: 12,
                via_firmware_version: 0,
                layout_options: 0,
                uptime_ms: 12_345,
                keychron_firmware: KeychronFirmware::Reported {
                    version: "v1.1.0".to_owned(),
                    build_date: "2025-03-17-10:00:00".to_owned(),
                },
            }
        );
        assert_eq!(replay.finish(), vec![]);
    }

    #[test]
    fn says_why_the_keychron_firmware_version_is_not_reported() {
        let mut replay = replay("cli-info-no-keychron.toml");
        let info = read(lend(&mut replay.transport), &replay.device).unwrap();
        assert_eq!(
            info.keychron_firmware,
            KeychronFirmware::NotReported {
                reason: "the firmware does not answer Keychron command 0xa0".to_owned()
            }
        );
        assert_eq!(replay.finish(), vec![]);
    }

    #[test]
    fn a_timeout_through_the_receiver_suggests_a_usb_cable() {
        let mut fixture = load_fixture("cli-info.toml");
        fixture.events.truncate(2);
        fixture.events[1] = Event::ReadTimeout;
        let mut replay = replay_fixture(fixture);
        let error = read(lend(&mut replay.transport), &replay.device).unwrap_err();
        assert_eq!(
            error.to_string(),
            "no input report arrived before the timeout. Through a receiver this is \
             inconclusive; try the keyboard on a USB cable (see docs/protocol/keychron-link.md)"
        );
        assert_eq!(replay.finish(), vec![]);
    }
}
