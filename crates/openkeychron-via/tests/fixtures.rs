//! VIA commands driven against fixture files, the way recordings of the V5 Max will be.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use openkeychron_hid::{Fixture, ReplayTransport};
    use openkeychron_via::Via;

    fn replay(name: &str) -> Via<ReplayTransport> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/synthetic")
            .join(name);
        Via::new(ReplayTransport::new(Fixture::load(&path).unwrap()))
    }

    #[test]
    fn reads_the_protocol_version_from_a_fixture() {
        let mut via = replay("via-protocol-version.toml");
        assert_eq!(via.protocol_version(), Ok(0x000c));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }

    #[test]
    fn reads_uptime_layout_options_and_via_firmware_version_from_a_fixture() {
        let mut via = replay("via-keyboard-values.toml");
        assert_eq!(via.uptime(), Ok(Duration::from_millis(12_345)));
        assert_eq!(via.layout_options(), Ok(0));
        assert_eq!(via.via_firmware_version(), Ok(0));
        assert_eq!(via.into_transport().finish(), Ok(()));
    }
}
