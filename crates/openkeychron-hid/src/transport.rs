use std::time::Duration;

use thiserror::Error;

/// A raw HID channel to one device interface.
pub trait HidTransport {
    /// Sends one output report.
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError>;

    /// Waits up to `timeout` for one input report.
    fn read(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError>;
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransportError {
    #[error("no input report arrived before the timeout")]
    Timeout,

    #[error("replay: write #{position} sent [{actual}], the fixture expected {expected}")]
    UnexpectedWrite {
        position: usize,
        expected: String,
        actual: String,
    },

    #[error("replay: read #{position} was not expected, the fixture expected {expected}")]
    UnexpectedRead { position: usize, expected: String },
}
