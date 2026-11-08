//! Keychron requests answered from the mock keyboard's state, driven through the real Keychron
//! client.

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use openkeychron_hid::{DeviceIdentity, HidTransport};
    use openkeychron_keychron::{
        Debounce, DebounceAlgorithm, FirmwareVersion, Keychron, SnapClickMode, SnapClickPair,
        WirelessTimers,
    };
    use openkeychron_mock::{MockKeyboard, MockTransport};

    fn v5_max_iso() -> MockKeyboard {
        MockKeyboard::new(DeviceIdentity {
            name: "mock V5 Max ISO".to_owned(),
            vendor_id: 0x3434,
            product_id: 0x0951,
            usage_page: 0xff60,
            usage: 0x61,
        })
        .unwrap()
    }

    fn keychron(keyboard: &MockKeyboard) -> Keychron<MockTransport> {
        Keychron::new(keyboard.connect())
    }

    /// Sends one raw request and returns the reply's status byte.
    fn status(keyboard: &MockKeyboard, prefix: &[u8]) -> u8 {
        let mut request = [0; 32];
        request[..prefix.len()].copy_from_slice(prefix);
        let mut transport = keyboard.connect();
        transport.write(&request).unwrap();
        transport.read(Duration::from_millis(1)).unwrap()[2]
    }

    #[test]
    fn reports_a_firmware_version_that_says_it_is_a_mock() {
        let keyboard = v5_max_iso();
        assert_eq!(
            keychron(&keyboard).firmware_version(),
            Ok(Ok(FirmwareVersion {
                version: "v0.0.0".to_owned(),
                build_date: "mock".to_owned(),
            }))
        );
    }

    #[test]
    fn reports_debounce_snap_click_and_wireless_timers_as_supported() {
        let features = keychron(&v5_max_iso()).misc_features().unwrap();
        assert_eq!(
            (
                features.protocol_version,
                features.debounce(),
                features.snap_click(),
                features.wireless_timers()
            ),
            (2, true, true, true)
        );
    }

    #[test]
    fn starts_with_symmetric_defer_global_debounce_at_50_ms() {
        assert_eq!(
            keychron(&v5_max_iso()).debounce(),
            Ok(Debounce {
                algorithm: DebounceAlgorithm::SymmetricDeferGlobal,
                time_ms: 50,
            })
        );
    }

    #[test]
    fn a_debounce_set_reads_back_through_the_next_connection() {
        let keyboard = v5_max_iso();
        let wanted = Debounce {
            algorithm: DebounceAlgorithm::SymmetricEagerPerKey,
            time_ms: 8,
        };
        keychron(&keyboard).set_debounce(wanted).unwrap();
        assert_eq!(keychron(&keyboard).debounce(), Ok(wanted));
    }

    #[test]
    fn refuses_an_unknown_debounce_algorithm_with_the_failure_status() {
        assert_eq!(status(&v5_max_iso(), &[0xa7, 0x06, 7, 5]), 1);
    }

    #[test]
    fn starts_with_twenty_snap_click_pairs_turned_off() {
        assert_eq!(
            keychron(&v5_max_iso()).snap_click_pairs(),
            Ok(vec![
                SnapClickPair {
                    mode: SnapClickMode::Off,
                    keys: [0, 0],
                };
                20
            ])
        );
    }

    #[test]
    fn a_snap_click_pair_set_reads_back_at_its_index_only() {
        let keyboard = v5_max_iso();
        let wanted = SnapClickPair {
            mode: SnapClickMode::LastInput,
            keys: [0x04, 0x07],
        };
        let mut session = keychron(&keyboard);
        session.set_snap_click_pair(11, wanted).unwrap();
        session.save_snap_click().unwrap();
        let pairs = keychron(&keyboard).snap_click_pairs().unwrap();
        assert_eq!(pairs[11], wanted);
        assert_eq!(
            pairs
                .iter()
                .filter(|pair| pair.mode != SnapClickMode::Off)
                .count(),
            1
        );
    }

    #[test]
    fn wireless_timers_set_read_back_through_the_next_connection() {
        let keyboard = v5_max_iso();
        let wanted = WirelessTimers {
            backlight_off_seconds: 30,
            sleep_seconds: 900,
        };
        keychron(&keyboard).set_wireless_timers(wanted).unwrap();
        assert_eq!(keychron(&keyboard).wireless_timers(), Ok(wanted));
    }

    #[test]
    fn refuses_a_sleep_time_under_a_minute_and_keeps_the_stored_timers() {
        let keyboard = v5_max_iso();
        let [backlight_low, backlight_high] = 30u16.to_le_bytes();
        let [sleep_low, sleep_high] = 59u16.to_le_bytes();
        assert_eq!(
            status(
                &keyboard,
                &[
                    0xa7,
                    0x0c,
                    backlight_low,
                    backlight_high,
                    sleep_low,
                    sleep_high
                ]
            ),
            1
        );
        assert_eq!(
            keychron(&keyboard).wireless_timers(),
            Ok(WirelessTimers {
                backlight_off_seconds: 600,
                sleep_seconds: 7200,
            })
        );
    }

    #[test]
    fn refuses_a_snap_click_request_past_the_last_pair() {
        let mut transport = v5_max_iso().connect();
        let mut request = [0; 32];
        request[..4].copy_from_slice(&[0xa7, 0x08, 18, 3]);
        assert_eq!(
            transport.write(&request).unwrap_err().to_string(),
            "device I/O failed: the mock keyboard refuses Keychron command 0xa7 0x08: pairs \
             18..21 are outside its 20 pairs"
        );
    }

    #[test]
    fn refuses_a_keychron_request_it_does_not_implement_and_names_it() {
        let mut transport = v5_max_iso().connect();
        let mut request = [0; 32];
        request[..2].copy_from_slice(&[0xa8, 0x01]);
        assert_eq!(
            transport.write(&request).unwrap_err().to_string(),
            "device I/O failed: the mock keyboard does not answer request a8 01"
        );
    }
}
