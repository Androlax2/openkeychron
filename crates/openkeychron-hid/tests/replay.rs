//! Drives the replay transport from a fixture file on disk, the way protocol tests will.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use openkeychron_hid::{Fixture, HidTransport, ReplayTransport};

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    #[test]
    fn replay_of_a_hand_written_fixture_answers_the_protocol_version_request() {
        let fixture = Fixture::load(&fixture_path("synthetic/via-protocol-version.toml")).unwrap();
        let mut transport = ReplayTransport::new(fixture);

        let mut request = [0u8; 32];
        request[0] = 0x01;
        transport.write(&request).unwrap();
        let response = transport.read(Duration::from_millis(100)).unwrap();

        assert_eq!(u16::from_be_bytes([response[1], response[2]]), 12);
        assert_eq!(transport.finish(), Ok(()));
    }
}
