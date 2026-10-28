use std::time::Duration;

use openkeychron_hid::{HidTransport, TransportError};
use thiserror::Error;

use crate::command::{Command, KeyboardValue, REPORT_LEN, UNHANDLED, request};

/// How long to wait for a reply before reporting [`TransportError::Timeout`].
const REPLY_TIMEOUT: Duration = Duration::from_millis(500);

/// A VIA session over one raw HID interface.
///
/// Every method here only reads; none changes the keyboard.
#[derive(Debug)]
pub struct Via<T> {
    transport: T,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ViaError {
    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error("the keyboard does not handle VIA command 0x{command:02x} (it replied id_unhandled)")]
    Unhandled { command: u8 },

    #[error("expected a {REPORT_LEN}-byte reply, got {length} bytes")]
    WrongLength { length: usize },

    #[error("sent command 0x{expected:02x}, the reply is for command 0x{actual:02x}")]
    WrongCommand { expected: u8, actual: u8 },

    #[error("asked for keyboard value 0x{expected:02x}, the reply is for value 0x{actual:02x}")]
    WrongValue { expected: u8, actual: u8 },
}

impl<T: HidTransport> Via<T> {
    #[must_use]
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Gives the transport back, for example to call `ReplayTransport::finish` in a test.
    #[must_use]
    pub fn into_transport(self) -> T {
        self.transport
    }

    /// `id_get_protocol_version`: the VIA protocol the firmware speaks (`0x000C` today).
    pub fn protocol_version(&mut self) -> Result<u16, ViaError> {
        let reply = self.exchange(Command::GetProtocolVersion, &[])?;
        Ok(u16::from_be_bytes([reply[1], reply[2]]))
    }

    /// `id_get_keyboard_value` / `id_uptime`: time since the keyboard's firmware started.
    pub fn uptime(&mut self) -> Result<Duration, ViaError> {
        let milliseconds = self.keyboard_value(KeyboardValue::Uptime)?;
        Ok(Duration::from_millis(u64::from(milliseconds)))
    }

    /// `id_get_keyboard_value` / `id_layout_options`: the bit field of chosen layout options.
    pub fn layout_options(&mut self) -> Result<u32, ViaError> {
        self.keyboard_value(KeyboardValue::LayoutOptions)
    }

    /// `id_get_keyboard_value` / `id_firmware_version`: `VIA_FIRMWARE_VERSION`, the version of
    /// the keyboard's VIA definition. Not Keychron's firmware version.
    pub fn via_firmware_version(&mut self) -> Result<u32, ViaError> {
        self.keyboard_value(KeyboardValue::FirmwareVersion)
    }

    fn keyboard_value(&mut self, value: KeyboardValue) -> Result<u32, ViaError> {
        let reply = self.exchange(Command::GetKeyboardValue, &[value.id()])?;
        if reply[1] != value.id() {
            return Err(ViaError::WrongValue {
                expected: value.id(),
                actual: reply[1],
            });
        }
        Ok(u32::from_be_bytes([reply[2], reply[3], reply[4], reply[5]]))
    }

    /// Sends one request and checks the reply belongs to it.
    fn exchange(
        &mut self,
        command: Command,
        arguments: &[u8],
    ) -> Result<[u8; REPORT_LEN], ViaError> {
        self.transport.write(&request(command, arguments))?;
        let reply = self.transport.read(REPLY_TIMEOUT)?;
        let reply: [u8; REPORT_LEN] =
            reply
                .try_into()
                .map_err(|reply: Vec<u8>| ViaError::WrongLength {
                    length: reply.len(),
                })?;
        match reply[0] {
            id if id == command.id() => Ok(reply),
            UNHANDLED => Err(ViaError::Unhandled {
                command: command.id(),
            }),
            other => Err(ViaError::WrongCommand {
                expected: command.id(),
                actual: other,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use openkeychron_hid::{DeviceIdentity, Event, Fixture, ReplayTransport, TransportError};

    use super::{Via, ViaError};
    use crate::command::REPORT_LEN;

    fn report(prefix: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0u8; REPORT_LEN];
        bytes[..prefix.len()].copy_from_slice(prefix);
        bytes
    }

    fn via(events: Vec<Event>) -> Via<ReplayTransport> {
        let device = DeviceIdentity {
            name: "synthetic".to_owned(),
            vendor_id: 0x3434,
            product_id: 0x0000,
            usage_page: 0xff60,
            usage: 0x61,
        };
        Via::new(ReplayTransport::new(Fixture::new(device, events)))
    }

    fn exchange(request: &[u8], reply: Vec<u8>) -> Vec<Event> {
        vec![
            Event::Write {
                data: report(request),
            },
            Event::Read { data: reply },
        ]
    }

    #[test]
    fn reads_uptime_as_big_endian_milliseconds() {
        let mut via = via(exchange(
            &[0x02, 0x01],
            report(&[0x02, 0x01, 0x00, 0x01, 0x00, 0x00]),
        ));
        assert_eq!(via.uptime(), Ok(std::time::Duration::from_millis(65_536)));
    }

    #[test]
    fn reports_a_command_the_firmware_does_not_handle() {
        let mut via = via(exchange(&[0x01], report(&[0xff])));
        assert_eq!(
            via.protocol_version(),
            Err(ViaError::Unhandled { command: 0x01 })
        );
    }

    #[test]
    fn rejects_a_reply_for_another_command() {
        let mut via = via(exchange(&[0x01], report(&[0x02, 0x00, 0x0c])));
        assert_eq!(
            via.protocol_version(),
            Err(ViaError::WrongCommand {
                expected: 0x01,
                actual: 0x02,
            })
        );
    }

    #[test]
    fn rejects_a_reply_for_another_keyboard_value() {
        let mut via = via(exchange(&[0x02, 0x02], report(&[0x02, 0x01, 0, 0, 0, 1])));
        assert_eq!(
            via.layout_options(),
            Err(ViaError::WrongValue {
                expected: 0x02,
                actual: 0x01,
            })
        );
    }

    #[test]
    fn rejects_a_reply_of_the_wrong_length() {
        let mut via = via(exchange(&[0x01], vec![0x01, 0x00, 0x0c]));
        assert_eq!(
            via.protocol_version(),
            Err(ViaError::WrongLength { length: 3 })
        );
    }

    #[test]
    fn passes_a_timeout_through() {
        let mut via = via(vec![
            Event::Write {
                data: report(&[0x01]),
            },
            Event::ReadTimeout,
        ]);
        assert_eq!(
            via.protocol_version(),
            Err(ViaError::Transport(TransportError::Timeout))
        );
    }
}
