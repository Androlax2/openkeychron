use std::time::Duration;

use openkeychron_hid::{HidTransport, TransportError};
use thiserror::Error;

/// Size of every request and reply, shared with VIA.
pub(crate) const REPORT_LEN: usize = 32;
const UNHANDLED: u8 = 0xFF;
const REPLY_TIMEOUT: Duration = Duration::from_millis(500);

pub(crate) const GET_SUPPORT_FEATURE: u8 = 0xA2;
pub(crate) const MISC_GROUP: u8 = 0xA7;
const MISC_GET_PROTOCOL_VER: u8 = 0x01;

/// A session speaking Keychron's raw HID commands over one VIA interface.
#[derive(Debug)]
pub struct Keychron<T> {
    transport: T,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KeychronError {
    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error("the keyboard does not handle command {0}")]
    Unhandled(String),

    #[error("expected a {REPORT_LEN}-byte reply, got {length} bytes")]
    WrongLength { length: usize },

    #[error("sent {expected}, the reply is for {actual}")]
    WrongReply { expected: String, actual: String },

    #[error("the keyboard refused {0} (status byte 1)")]
    Refused(String),

    #[error("this firmware does not report support for {0}")]
    Unsupported(&'static str),

    #[error("{0}")]
    Invalid(String),

    #[error("wrote {written}, but the keyboard now reports {read}")]
    ReadBackMismatch { written: String, read: String },
}

/// Feature bits from `0xA2` (`docs/protocol/keychron.md`, "Top-level commands").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Features(pub u16);

impl Features {
    #[must_use]
    pub const fn dynamic_debounce(self) -> bool {
        self.0 & (1 << 5) != 0
    }

    #[must_use]
    pub const fn snap_click(self) -> bool {
        self.0 & (1 << 6) != 0
    }
}

/// The misc group's protocol version and feature bits, from `A7 01`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MiscFeatures {
    pub protocol_version: u16,
    pub bits: u8,
}

impl MiscFeatures {
    #[must_use]
    pub const fn debounce(self) -> bool {
        self.bits & (1 << 2) != 0
    }

    #[must_use]
    pub const fn snap_click(self) -> bool {
        self.bits & (1 << 3) != 0
    }

    #[must_use]
    pub const fn wireless_timers(self) -> bool {
        self.bits & (1 << 4) != 0
    }
}

impl<T: HidTransport> Keychron<T> {
    #[must_use]
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    #[must_use]
    pub fn into_transport(self) -> T {
        self.transport
    }

    /// `0xA2`: which Keychron features this firmware has.
    pub fn features(&mut self) -> Result<Features, KeychronError> {
        let reply = self.exchange(&[GET_SUPPORT_FEATURE])?;
        Ok(Features(u16::from_le_bytes([reply[2], reply[3]])))
    }

    /// `A7 01`: the misc group's protocol version and feature bits.
    pub fn misc_features(&mut self) -> Result<MiscFeatures, KeychronError> {
        let reply = self.misc(MISC_GET_PROTOCOL_VER, &[])?;
        Ok(MiscFeatures {
            protocol_version: u16::from_le_bytes([reply[3], reply[4]]),
            bits: reply[5],
        })
    }

    /// One misc-group exchange (`A7 <sub> <arguments>`), checking the echoed sub-command and
    /// the status byte.
    pub(crate) fn misc(
        &mut self,
        sub: u8,
        arguments: &[u8],
    ) -> Result<[u8; REPORT_LEN], KeychronError> {
        let mut request = vec![MISC_GROUP, sub];
        request.extend_from_slice(arguments);
        let reply = self.exchange(&request)?;
        if reply[1] != sub {
            return Err(KeychronError::WrongReply {
                expected: describe(MISC_GROUP, Some(sub)),
                actual: describe(reply[0], Some(reply[1])),
            });
        }
        if reply[2] != 0 {
            return Err(KeychronError::Refused(describe(MISC_GROUP, Some(sub))));
        }
        Ok(reply)
    }

    /// Sends `request` padded to 32 bytes and checks the reply is for the same command.
    pub(crate) fn exchange(&mut self, request: &[u8]) -> Result<[u8; REPORT_LEN], KeychronError> {
        let sub = request.get(1).copied().filter(|_| request[0] == MISC_GROUP);
        let mut report = [0u8; REPORT_LEN];
        report[..request.len()].copy_from_slice(request);
        self.transport.write(&report)?;
        let reply: [u8; REPORT_LEN] =
            self.transport
                .read(REPLY_TIMEOUT)?
                .try_into()
                .map_err(|reply: Vec<u8>| KeychronError::WrongLength {
                    length: reply.len(),
                })?;
        match reply[0] {
            command if command == request[0] => Ok(reply),
            UNHANDLED => Err(KeychronError::Unhandled(describe(request[0], sub))),
            other => Err(KeychronError::WrongReply {
                expected: describe(request[0], sub),
                actual: describe(other, None),
            }),
        }
    }
}

fn describe(command: u8, sub: Option<u8>) -> String {
    match sub {
        Some(sub) => format!("0x{command:02x} 0x{sub:02x}"),
        None => format!("0x{command:02x}"),
    }
}
