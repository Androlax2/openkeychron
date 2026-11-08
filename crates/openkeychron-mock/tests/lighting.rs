//! Lighting answered from the mock keyboard's state, driven through the real clients: VIA's RGB
//! matrix values, and Keychron's per-key and mixed RGB.

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use openkeychron_hid::{DeviceIdentity, HidTransport};
    use openkeychron_keychron::{Keychron, KeychronError, MatrixSize, MixedEffect, PerKeyColor};
    use openkeychron_mock::{MockKeyboard, MockTransport};
    use openkeychron_via::{RgbMatrix, RgbValue, Via};

    const V5_MAX_ISO_MATRIX: MatrixSize = MatrixSize {
        rows: 6,
        columns: 19,
    };

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

    fn keychron(keyboard: &MockKeyboard) -> Keychron<MockTransport> {
        Keychron::new(keyboard.connect())
    }

    fn request(prefix: &[u8]) -> [u8; 32] {
        let mut report = [0; 32];
        report[..prefix.len()].copy_from_slice(prefix);
        report
    }

    /// Sends one raw request and returns the reply's status byte.
    fn status(keyboard: &MockKeyboard, prefix: &[u8]) -> u8 {
        let mut transport = keyboard.connect();
        transport.write(&request(prefix)).unwrap();
        transport.read(Duration::from_millis(1)).unwrap()[2]
    }

    fn refusal(keyboard: &MockKeyboard, prefix: &[u8]) -> String {
        keyboard
            .connect()
            .write(&request(prefix))
            .unwrap_err()
            .to_string()
    }

    /// Sets `value` through the VIA client, then reads the whole matrix on a new connection.
    fn set_then_read(value: RgbValue) -> RgbMatrix {
        let keyboard = v5_max_iso();
        via(&keyboard).set_rgb_value(value).unwrap();
        via(&keyboard).rgb_matrix().unwrap()
    }

    const SEED: RgbMatrix = RgbMatrix {
        brightness: 0,
        effect: 1,
        speed: 0,
        hue: 0,
        saturation: 0,
    };

    #[test]
    fn starts_lit_with_effect_1_and_every_other_value_at_zero() {
        assert_eq!(via(&v5_max_iso()).rgb_matrix(), Ok(SEED));
    }

    #[test]
    fn a_brightness_set_reads_back_scaled_as_the_firmware_stores_it() {
        assert_eq!(
            set_then_read(RgbValue::Brightness(128)),
            RgbMatrix {
                brightness: 127,
                ..SEED
            }
        );
    }

    #[test]
    fn an_effect_set_reads_back() {
        assert_eq!(
            set_then_read(RgbValue::Effect(6)),
            RgbMatrix { effect: 6, ..SEED }
        );
    }

    #[test]
    fn a_speed_set_reads_back() {
        assert_eq!(
            set_then_read(RgbValue::Speed(200)),
            RgbMatrix { speed: 200, ..SEED }
        );
    }

    #[test]
    fn a_colour_set_reads_back() {
        assert_eq!(
            set_then_read(RgbValue::Color {
                hue: 85,
                saturation: 200
            }),
            RgbMatrix {
                hue: 85,
                saturation: 200,
                ..SEED
            }
        );
    }

    #[test]
    fn effect_0_turns_lighting_off() {
        assert_eq!(
            set_then_read(RgbValue::Effect(0)),
            RgbMatrix { effect: 0, ..SEED }
        );
    }

    #[test]
    fn refuses_brightness_speed_and_colour_sets_while_lighting_is_off_which_the_firmware_ignores() {
        let keyboard = v5_max_iso();
        via(&keyboard).set_rgb_value(RgbValue::Effect(0)).unwrap();
        let ignored = "device I/O failed: the mock keyboard refuses VIA command 0x07: the firmware \
                       ignores brightness, speed and colour while lighting is off, which the mock \
                       does not model";
        for set in [
            [0x07, 0x03, 0x01, 128, 0],
            [0x07, 0x03, 0x03, 200, 0],
            [0x07, 0x03, 0x04, 85, 200],
        ] {
            assert_eq!(refusal(&keyboard, &set), ignored, "request {set:02x?}");
        }
    }

    #[test]
    fn refuses_an_effect_past_the_last_which_the_firmware_clamps() {
        assert_eq!(
            refusal(&v5_max_iso(), &[0x07, 0x03, 0x02, 25]),
            "device I/O failed: the mock keyboard refuses VIA command 0x07: the firmware clamps \
             effect 25 to its last effect, 24, which the mock does not model"
        );
    }

    #[test]
    fn does_not_answer_a_lighting_value_it_does_not_hold() {
        assert_eq!(
            refusal(&v5_max_iso(), &[0x08, 0x03, 0x05]),
            "device I/O failed: the mock keyboard does not answer request 08 03 05"
        );
    }

    #[test]
    fn does_not_answer_another_lighting_channel() {
        assert_eq!(
            refusal(&v5_max_iso(), &[0x08, 0x02, 0x01]),
            "device I/O failed: the mock keyboard does not answer request 08 02 01"
        );
    }

    #[test]
    fn reports_debounce_snap_click_and_keychron_rgb_as_its_features() {
        let features = keychron(&v5_max_iso()).features().unwrap();
        assert_eq!(features.0, 0b1110_0000);
    }

    #[test]
    fn opens_per_key_rgb_with_the_v5_max_iso_led_count() {
        let mut keychron = keychron(&v5_max_iso());
        assert_eq!(keychron.per_key_rgb().map(|rgb| rgb.led_count()), Ok(98));
    }

    #[test]
    fn starts_with_every_per_key_colour_at_zero() {
        let zero = PerKeyColor {
            hue: 0,
            saturation: 0,
            value: 0,
        };
        assert_eq!(
            keychron(&v5_max_iso()).per_key_rgb().unwrap().colors(),
            Ok(vec![zero; 98])
        );
    }

    #[test]
    fn a_per_key_colour_set_reads_back_at_its_led_only() {
        let keyboard = v5_max_iso();
        let green = PerKeyColor {
            hue: 85,
            saturation: 255,
            value: 200,
        };
        let mut session = keychron(&keyboard);
        let mut rgb = session.per_key_rgb().unwrap();
        rgb.set_color(97, green).unwrap();
        rgb.save().unwrap();
        let colors = keychron(&keyboard).per_key_rgb().unwrap().colors().unwrap();
        assert_eq!(colors[97], green);
        assert_eq!(colors.iter().filter(|color| **color == green).count(), 1);
    }

    #[test]
    fn maps_matrix_positions_to_the_v5_max_iso_leds() {
        let mut session = keychron(&v5_max_iso());
        let mut rgb = session.per_key_rgb().unwrap();
        let leds: Vec<_> = [(0, 0), (0, 1), (1, 0), (5, 6), (5, 17)]
            .into_iter()
            .map(|(row, column)| rgb.led_index(V5_MAX_ISO_MATRIX, row, column).unwrap())
            .collect();
        assert_eq!(leds, [Some(0), None, Some(16), Some(89), Some(97)]);
    }

    #[test]
    fn an_led_lookup_answers_no_led_for_every_column_outside_its_mask() {
        let mut transport = v5_max_iso().connect();
        transport
            .write(&request(&[0xa8, 0x06, 1, 0b0000_0100]))
            .unwrap();
        let reply = transport.read(Duration::from_millis(1)).unwrap();
        let mut wanted = [0xff; 19];
        wanted[2] = 18;
        assert_eq!(reply[3..22], wanted);
    }

    #[test]
    fn fails_an_effect_list_set_with_mixed_rgb_for_the_region_past_the_last_with_the_failure_status()
     {
        assert_eq!(status(&v5_max_iso(), &[0xa8, 0x0f, 2, 0, 1, 24]), 1);
    }

    #[test]
    fn fails_an_led_lookup_past_the_last_row_with_the_failure_status() {
        assert_eq!(status(&v5_max_iso(), &[0xa8, 0x06, 7, 0xff, 0xff, 0x07]), 1);
    }

    #[test]
    fn refuses_an_led_lookup_on_the_row_the_firmware_reads_past_its_matrix() {
        assert_eq!(
            refusal(&v5_max_iso(), &[0xa8, 0x06, 6, 0x01]),
            "device I/O failed: the mock keyboard refuses Keychron command 0xa8 0x06: row 6 is \
             outside its 6 rows, which the firmware reads past its matrix"
        );
    }

    #[test]
    fn fails_a_per_key_colour_request_of_more_than_nine_leds_with_the_failure_status() {
        assert_eq!(status(&v5_max_iso(), &[0xa8, 0x09, 0, 10]), 1);
    }

    #[test]
    fn refuses_a_per_key_colour_request_past_the_last_led() {
        assert_eq!(
            refusal(&v5_max_iso(), &[0xa8, 0x0a, 96, 3]),
            "device I/O failed: the mock keyboard refuses Keychron command 0xa8 0x0a: LEDs \
             96..99 are outside its 98 LEDs, which the firmware reads and writes past its array"
        );
    }

    #[test]
    fn opens_mixed_rgb_with_two_regions_of_five_effects() {
        let mut session = keychron(&v5_max_iso());
        let rgb = session.mixed_rgb().unwrap();
        assert_eq!((rgb.region_count(), rgb.effects_per_region()), (2, 5));
    }

    #[test]
    fn starts_with_every_led_in_region_0() {
        assert_eq!(
            keychron(&v5_max_iso()).mixed_rgb().unwrap().regions(),
            Ok(vec![0; 98])
        );
    }

    #[test]
    fn a_region_set_reads_back_through_the_next_connection() {
        let keyboard = v5_max_iso();
        let mut session = keychron(&keyboard);
        let mut rgb = session.mixed_rgb().unwrap();
        rgb.set_regions(20, &[1; 30]).unwrap();
        rgb.save().unwrap();
        let regions = keychron(&keyboard).mixed_rgb().unwrap().regions().unwrap();
        let mut wanted = vec![0; 98];
        wanted[20..50].fill(1);
        assert_eq!(regions, wanted);
    }

    const EMPTY_SLOT: MixedEffect = MixedEffect {
        effect: 0,
        hue: 0,
        saturation: 0,
        speed: 0,
        duration_ms: 0,
    };

    #[test]
    fn starts_with_every_effect_slot_empty() {
        assert_eq!(
            keychron(&v5_max_iso()).mixed_rgb().unwrap().effects(1),
            Ok(vec![EMPTY_SLOT; 5])
        );
    }

    #[test]
    fn an_effect_list_set_reads_back_in_its_region_only() {
        let keyboard = v5_max_iso();
        let breathing = MixedEffect {
            effect: 3,
            hue: 170,
            saturation: 255,
            speed: 127,
            duration_ms: 70_000,
        };
        let mut session = keychron(&keyboard);
        let mut rgb = session.mixed_rgb().unwrap();
        rgb.set_effects(1, 1, &[breathing; 4]).unwrap();
        rgb.save().unwrap();
        let mut session = keychron(&keyboard);
        let mut rgb = session.mixed_rgb().unwrap();
        assert_eq!(
            (rgb.effects(0), rgb.effects(1)),
            (
                Ok(vec![EMPTY_SLOT; 5]),
                Ok(vec![EMPTY_SLOT, breathing, breathing, breathing, breathing])
            )
        );
    }

    #[test]
    fn refuses_mixed_rgb_inside_an_effect_list_with_the_failure_status() {
        let mixed_rgb = MixedEffect {
            effect: 24,
            hue: 0,
            saturation: 0,
            speed: 0,
            duration_ms: 1000,
        };
        let mut session = keychron(&v5_max_iso());
        assert_eq!(
            session.mixed_rgb().unwrap().set_effects(0, 0, &[mixed_rgb]),
            Err(KeychronError::Refused("0xa8 0x0f".to_owned()))
        );
    }

    #[test]
    fn fails_a_region_set_naming_a_region_past_the_last_with_the_failure_status() {
        assert_eq!(status(&v5_max_iso(), &[0xa8, 0x0d, 0, 1, 2]), 1);
    }

    #[test]
    fn refuses_an_effect_list_request_for_the_region_the_firmware_indexes_past() {
        assert_eq!(
            refusal(&v5_max_iso(), &[0xa8, 0x0e, 2, 0, 1]),
            "device I/O failed: the mock keyboard refuses Keychron command 0xa8 0x0e: region 2 \
             is outside its 2 regions, which the firmware indexes past its array"
        );
    }
}
