//! VIA requests answered from the mock keyboard's state, driven through the real VIA client.

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use openkeychron_hid::{DeviceIdentity, HidTransport, TransportError};
    use openkeychron_mock::{MockKeyboard, MockTransport};
    use openkeychron_via::{EncoderDirection, Keycode, Via};

    /// 4 layers of the V5 Max ISO's 6 × 19 matrix.
    const MATRIX_KEYS: usize = 4 * 6 * 19;

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

    fn via(keyboard: &MockKeyboard) -> Via<MockTransport> {
        Via::new(keyboard.connect())
    }

    fn request(prefix: &[u8]) -> [u8; 32] {
        let mut report = [0; 32];
        report[..prefix.len()].copy_from_slice(prefix);
        report
    }

    #[test]
    fn reports_the_via_protocol_and_keyboard_values() {
        let keyboard = v5_max_iso();
        let mut via = via(&keyboard);
        assert_eq!(via.protocol_version(), Ok(0x000c));
        assert_eq!(via.via_firmware_version(), Ok(0));
        assert_eq!(via.layout_options(), Ok(0));
        assert_eq!(via.uptime(), Ok(Duration::from_millis(12_345)));
    }

    #[test]
    fn starts_with_four_layers_of_zero_keycodes() {
        let keyboard = v5_max_iso();
        let mut via = via(&keyboard);
        assert_eq!(via.layer_count(), Ok(4));
        assert_eq!(
            via.keymap(MATRIX_KEYS),
            Ok(vec![Keycode::from(0); MATRIX_KEYS])
        );
    }

    #[test]
    fn a_keycode_written_through_one_connection_reads_back_through_the_next() {
        let keyboard = v5_max_iso();
        via(&keyboard)
            .write_keycode(1, 2, 3, Keycode::from(0x0004))
            .unwrap();
        let keymap = via(&keyboard).keymap(MATRIX_KEYS).unwrap();
        assert_eq!(keymap[(6 + 2) * 19 + 3], Keycode::from(0x0004));
        assert_eq!(
            keymap
                .iter()
                .filter(|key| **key != Keycode::from(0))
                .count(),
            1
        );
    }

    #[test]
    fn a_whole_keymap_written_reads_back_key_for_key() {
        let keyboard = v5_max_iso();
        let wanted: Vec<_> = (0..MATRIX_KEYS)
            .map(|index| Keycode::from(u16::try_from(index).unwrap()))
            .collect();
        via(&keyboard).write_keymap(&wanted).unwrap();
        assert_eq!(via(&keyboard).keymap(MATRIX_KEYS), Ok(wanted));
    }

    #[test]
    fn an_encoder_keycode_written_reads_back_for_its_direction_only() {
        let keyboard = v5_max_iso();
        via(&keyboard)
            .write_encoder_keycode(2, 0, EncoderDirection::Clockwise, Keycode::from(0x00e9))
            .unwrap();
        let mut via = via(&keyboard);
        assert_eq!(
            via.encoder_keycode(2, 0, EncoderDirection::Clockwise),
            Ok(Keycode::from(0x00e9))
        );
        assert_eq!(
            via.encoder_keycode(2, 0, EncoderDirection::CounterClockwise),
            Ok(Keycode::from(0))
        );
    }

    #[test]
    fn a_keymap_reset_restores_the_seeded_keycodes() {
        let keyboard = v5_max_iso();
        via(&keyboard)
            .write_keycode(0, 0, 0, Keycode::from(0x0029))
            .unwrap();
        via(&keyboard)
            .write_encoder_keycode(0, 0, EncoderDirection::Clockwise, Keycode::from(0x00e9))
            .unwrap();
        via(&keyboard).reset_keymap().unwrap();
        let mut via = via(&keyboard);
        assert_eq!(via.keycode(0, 0, 0), Ok(Keycode::from(0)));
        assert_eq!(
            via.encoder_keycode(0, 0, EncoderDirection::Clockwise),
            Ok(Keycode::from(0))
        );
    }

    #[test]
    fn starts_with_sixteen_empty_macros_in_a_512_byte_buffer() {
        let keyboard = v5_max_iso();
        let mut via = via(&keyboard);
        assert_eq!(via.macro_count(), Ok(16));
        assert_eq!(via.macro_buffer_size(), Ok(512));
        assert_eq!(via.macro_buffer(512), Ok(vec![0; 512]));
    }

    #[test]
    fn a_macro_buffer_written_reads_back_through_the_next_connection() {
        let keyboard = v5_max_iso();
        let mut wanted = b"hello\0world".to_vec();
        wanted.resize(512, 0);
        via(&keyboard).write_macro_buffer(&wanted).unwrap();
        assert_eq!(via(&keyboard).macro_buffer(512), Ok(wanted));
    }

    #[test]
    fn refuses_a_keymap_read_past_the_last_layer() {
        let keyboard = v5_max_iso();
        let error = via(&keyboard).keymap(MATRIX_KEYS + 1).unwrap_err();
        assert_eq!(
            error.to_string(),
            "device I/O failed: the mock keyboard refuses VIA command 0x12: bytes 896..914 are \
             outside its 912-byte keymap"
        );
    }

    #[test]
    fn refuses_a_keycode_outside_the_matrix() {
        let keyboard = v5_max_iso();
        let error = via(&keyboard).keycode(0, 6, 0).unwrap_err();
        assert_eq!(
            error.to_string(),
            "device I/O failed: the mock keyboard refuses VIA command 0x04: layer 0 row 6 column \
             0 is outside its 4 layers of 6 × 19 keys"
        );
    }

    #[test]
    fn refuses_a_request_it_does_not_implement_and_names_it() {
        let mut transport = v5_max_iso().connect();
        assert_eq!(
            transport.write(&request(&[0x03, 0x01])),
            Err(TransportError::Device(
                "the mock keyboard does not answer request 03 01".to_owned()
            ))
        );
    }

    #[test]
    fn refuses_a_report_that_is_not_32_bytes() {
        let mut transport = v5_max_iso().connect();
        assert_eq!(
            transport.write(&[0x01]),
            Err(TransportError::Device(
                "the mock keyboard takes 32-byte reports, got 1 bytes".to_owned()
            ))
        );
    }

    #[test]
    fn a_read_without_a_request_times_out() {
        let mut transport = v5_max_iso().connect();
        assert_eq!(
            transport.read(Duration::from_millis(1)),
            Err(TransportError::Timeout)
        );
    }
}
