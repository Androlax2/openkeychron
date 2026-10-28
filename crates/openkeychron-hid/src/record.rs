use std::time::Duration;

use crate::{DeviceIdentity, Event, Fixture, HidTransport, TransportError};

/// Wraps a transport and keeps every report that crossed it, to save as a [`Fixture`].
///
/// Only what the device actually did is kept: a failed write and any read error other than
/// a timeout are passed through without being recorded.
#[derive(Debug)]
pub struct RecordingTransport<T> {
    inner: T,
    events: Vec<Event>,
}

impl<T: HidTransport> RecordingTransport<T> {
    #[must_use]
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            events: Vec::new(),
        }
    }

    #[must_use]
    pub fn into_fixture(self, device: DeviceIdentity) -> Fixture {
        Fixture::new(device, self.events)
    }
}

impl<T: HidTransport> HidTransport for RecordingTransport<T> {
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        self.inner.write(report)?;
        self.events.push(Event::Write {
            data: report.to_vec(),
        });
        Ok(())
    }

    fn read(&mut self, timeout: Duration) -> Result<Vec<u8>, TransportError> {
        match self.inner.read(timeout) {
            Ok(data) => {
                self.events.push(Event::Read { data: data.clone() });
                Ok(data)
            }
            Err(TransportError::Timeout) => {
                self.events.push(Event::ReadTimeout);
                Err(TransportError::Timeout)
            }
            Err(other) => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::RecordingTransport;
    use crate::{DeviceIdentity, Event, Fixture, HidTransport, ReplayTransport};

    const TIMEOUT: Duration = Duration::from_millis(100);

    fn device() -> DeviceIdentity {
        DeviceIdentity {
            name: "test".to_owned(),
            vendor_id: 0x3434,
            product_id: 0x0001,
            usage_page: 0xff60,
            usage: 0x61,
        }
    }

    fn scripted(events: Vec<Event>) -> ReplayTransport {
        ReplayTransport::new(Fixture::new(device(), events))
    }

    #[test]
    fn records_writes_reads_and_timeouts_in_order() {
        let session = vec![
            Event::Write { data: vec![0x01] },
            Event::Read {
                data: vec![0x01, 0x00, 0x0c],
            },
            Event::ReadTimeout,
        ];
        let mut recorder = RecordingTransport::new(scripted(session.clone()));
        recorder.write(&[0x01]).unwrap();
        recorder.read(TIMEOUT).unwrap();
        recorder.read(TIMEOUT).unwrap_err();
        assert_eq!(recorder.into_fixture(device()).events, session);
    }

    #[test]
    fn does_not_record_a_failed_write() {
        let mut recorder = RecordingTransport::new(scripted(vec![]));
        recorder.write(&[0x01]).unwrap_err();
        assert_eq!(recorder.into_fixture(device()).events, Vec::<Event>::new());
    }

    #[test]
    fn a_recording_replays_the_same_session() {
        let mut recorder = RecordingTransport::new(scripted(vec![
            Event::Write { data: vec![0x01] },
            Event::Read {
                data: vec![0x01, 0x00, 0x0c],
            },
        ]));
        recorder.write(&[0x01]).unwrap();
        recorder.read(TIMEOUT).unwrap();

        let mut replay = ReplayTransport::new(recorder.into_fixture(device()));
        replay.write(&[0x01]).unwrap();
        assert_eq!(replay.read(TIMEOUT), Ok(vec![0x01, 0x00, 0x0c]));
        assert_eq!(replay.finish(), Ok(()));
    }
}
