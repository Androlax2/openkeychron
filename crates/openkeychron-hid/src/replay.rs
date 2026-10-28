use std::time::Duration;

use crate::hex;
use crate::{Event, Fixture, HidTransport, TransportError};

/// Plays a [`Fixture`] back in order and fails on the first report that differs from it.
///
/// Call [`ReplayTransport::finish`] at the end of a test: a session that stops early is as
/// much a regression as one that sends the wrong bytes.
#[derive(Debug)]
pub struct ReplayTransport {
    events: Vec<Event>,
    position: usize,
}

impl ReplayTransport {
    #[must_use]
    pub fn new(fixture: Fixture) -> Self {
        Self {
            events: fixture.events,
            position: 0,
        }
    }

    /// Fails with the events the code never asked for.
    pub fn finish(self) -> Result<(), Vec<Event>> {
        let remaining = self.events[self.position..].to_vec();
        if remaining.is_empty() {
            Ok(())
        } else {
            Err(remaining)
        }
    }

    fn describe_next(&self) -> String {
        match self.events.get(self.position) {
            None => "the end of the recording".to_owned(),
            Some(Event::Write { data }) => format!("write [{}]", hex::encode(data)),
            Some(Event::Read { data }) => format!("read [{}]", hex::encode(data)),
            Some(Event::ReadTimeout) => "a read timeout".to_owned(),
        }
    }
}

impl HidTransport for ReplayTransport {
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        match self.events.get(self.position) {
            Some(Event::Write { data }) if data == report => {
                self.position += 1;
                Ok(())
            }
            _ => Err(TransportError::UnexpectedWrite {
                position: self.position,
                expected: self.describe_next(),
                actual: hex::encode(report),
            }),
        }
    }

    fn read(&mut self, _timeout: Duration) -> Result<Vec<u8>, TransportError> {
        match self.events.get(self.position) {
            Some(Event::Read { data }) => {
                let data = data.clone();
                self.position += 1;
                Ok(data)
            }
            Some(Event::ReadTimeout) => {
                self.position += 1;
                Err(TransportError::Timeout)
            }
            _ => Err(TransportError::UnexpectedRead {
                position: self.position,
                expected: self.describe_next(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::ReplayTransport;
    use crate::{DeviceIdentity, Event, Fixture, HidTransport, TransportError};

    const TIMEOUT: Duration = Duration::from_millis(100);

    fn replay(events: Vec<Event>) -> ReplayTransport {
        let device = DeviceIdentity {
            name: "test".to_owned(),
            vendor_id: 0x3434,
            product_id: 0x0001,
            usage_page: 0xff60,
            usage: 0x61,
        };
        ReplayTransport::new(Fixture::new(device, events))
    }

    #[test]
    fn answers_a_recorded_write_with_the_recorded_read() {
        let mut transport = replay(vec![
            Event::Write { data: vec![0x01] },
            Event::Read {
                data: vec![0x01, 0x00, 0x0c],
            },
        ]);
        transport.write(&[0x01]).unwrap();
        assert_eq!(transport.read(TIMEOUT), Ok(vec![0x01, 0x00, 0x0c]));
        assert_eq!(transport.finish(), Ok(()));
    }

    #[test]
    fn rejects_a_write_with_different_bytes() {
        let mut transport = replay(vec![Event::Write { data: vec![0x01] }]);
        assert_eq!(
            transport.write(&[0x02]),
            Err(TransportError::UnexpectedWrite {
                position: 0,
                expected: "write [01]".to_owned(),
                actual: "02".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_a_read_where_a_write_was_recorded() {
        let mut transport = replay(vec![Event::Write { data: vec![0x01] }]);
        assert_eq!(
            transport.read(TIMEOUT),
            Err(TransportError::UnexpectedRead {
                position: 0,
                expected: "write [01]".to_owned(),
            })
        );
    }

    #[test]
    fn rejects_a_write_past_the_end_of_the_recording() {
        let mut transport = replay(vec![]);
        assert!(matches!(
            transport.write(&[0x01]),
            Err(TransportError::UnexpectedWrite { expected, .. }) if expected == "the end of the recording"
        ));
    }

    #[test]
    fn replays_a_recorded_timeout() {
        let mut transport = replay(vec![Event::ReadTimeout]);
        assert_eq!(transport.read(TIMEOUT), Err(TransportError::Timeout));
    }

    #[test]
    fn finish_reports_events_the_code_never_reached() {
        let transport = replay(vec![Event::Write { data: vec![0x01] }]);
        assert_eq!(
            transport.finish(),
            Err(vec![Event::Write { data: vec![0x01] }])
        );
    }
}
