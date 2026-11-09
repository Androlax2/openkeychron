//! The guarded keymap writes and their undo, run end to end against one mock keyboard.

#[cfg(test)]
mod tests {
    use openkeychron_guard::keymap::{
        EncoderTurn, KeyPosition, restore, set_encoder_keycode, set_keycode,
    };
    use openkeychron_guard::{
        GuardError, GuardedWriteError, WriteVerified, require_write_verified,
    };
    use openkeychron_hid::{DeviceIdentity, KeychronDevice};
    use openkeychron_mock::MockKeyboard;
    use openkeychron_via::{EncoderDirection, Keycode, Via};

    const MATRIX_KEYS: usize = 4 * 6 * 19;
    /// Row 2, column 1: the key labelled `A` on the FR ISO layout.
    const ROW: u8 = 2;
    const COLUMN: u8 = 1;
    const KC_A: u16 = 0x0004;
    const KC_B: u16 = 0x0005;
    const KC_VOLU: u16 = 0x00a9;
    const KC_VOLD: u16 = 0x00aa;

    fn v5_max_iso() -> (MockKeyboard, WriteVerified) {
        let keyboard = MockKeyboard::new(DeviceIdentity {
            name: Some("mock V5 Max ISO".to_owned()),
            vendor_id: 0x3434,
            product_id: 0x0951,
            usage_page: 0xff60,
            usage: 0x61,
        })
        .unwrap();
        let device = KeychronDevice {
            name: Some("mock V5 Max ISO".to_owned()),
            vendor_id: 0x3434,
            product_id: 0x0951,
            path: "mock".to_owned(),
            is_receiver: false,
        };
        let verified = require_write_verified("keymap writes", &device).unwrap();
        (keyboard, verified)
    }

    fn keymap(keyboard: &MockKeyboard) -> Vec<Keycode> {
        Via::new(keyboard.connect()).keymap(MATRIX_KEYS).unwrap()
    }

    #[test]
    fn a_guarded_keycode_write_is_visible_in_the_next_read() {
        let (keyboard, verified) = v5_max_iso();
        set_keycode::<_, GuardError>(
            &mut Via::new(keyboard.connect()),
            &verified,
            0,
            KeyPosition::new(ROW, COLUMN).unwrap(),
            KC_A,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(
            keymap(&keyboard)[usize::from(ROW) * 19 + usize::from(COLUMN)],
            Keycode::from(KC_A)
        );
    }

    #[test]
    fn restoring_the_backup_of_a_guarded_write_undoes_it() {
        let (keyboard, verified) = v5_max_iso();
        // A non-zero key, so a restore that wrote the zero seed back would not pass.
        Via::new(keyboard.connect())
            .write_keycode(0, ROW, COLUMN, Keycode::from(KC_B))
            .unwrap();
        let before = keymap(&keyboard);
        let backup = set_keycode::<_, GuardError>(
            &mut Via::new(keyboard.connect()),
            &verified,
            0,
            KeyPosition::new(ROW, COLUMN).unwrap(),
            KC_A,
            |_| Ok(()),
        )
        .unwrap();
        restore::<_, GuardError>(
            &mut Via::new(keyboard.connect()),
            &verified,
            &backup,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(keymap(&keyboard), before);
    }

    fn encoder_keycode(keyboard: &MockKeyboard, layer: u8, direction: EncoderDirection) -> Keycode {
        Via::new(keyboard.connect())
            .encoder_keycode(layer, 0, direction)
            .unwrap()
    }

    fn turn(direction: EncoderDirection) -> EncoderTurn {
        EncoderTurn::new(0, direction).unwrap()
    }

    #[test]
    fn a_guarded_encoder_write_is_visible_in_the_next_read() {
        let (keyboard, verified) = v5_max_iso();
        set_encoder_keycode::<_, GuardError>(
            &mut Via::new(keyboard.connect()),
            &verified,
            1,
            turn(EncoderDirection::Clockwise),
            KC_VOLU,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(
            encoder_keycode(&keyboard, 1, EncoderDirection::Clockwise),
            Keycode::from(KC_VOLU)
        );
    }

    #[test]
    fn a_guarded_encoder_write_leaves_the_other_direction_as_it_was() {
        let (keyboard, verified) = v5_max_iso();
        set_encoder_keycode::<_, GuardError>(
            &mut Via::new(keyboard.connect()),
            &verified,
            1,
            turn(EncoderDirection::Clockwise),
            KC_VOLU,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(
            encoder_keycode(&keyboard, 1, EncoderDirection::CounterClockwise),
            Keycode::from(0)
        );
    }

    #[test]
    fn a_guarded_encoder_write_returns_the_keymap_from_before_it() {
        let (keyboard, verified) = v5_max_iso();
        Via::new(keyboard.connect())
            .write_encoder_keycode(
                0,
                0,
                EncoderDirection::CounterClockwise,
                Keycode::from(KC_VOLD),
            )
            .unwrap();
        let backup = set_encoder_keycode::<_, GuardError>(
            &mut Via::new(keyboard.connect()),
            &verified,
            0,
            turn(EncoderDirection::CounterClockwise),
            KC_VOLU,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(backup.encoders[0].counter_clockwise, KC_VOLD);
    }

    #[test]
    fn restoring_the_backup_of_a_guarded_encoder_write_undoes_it() {
        let (keyboard, verified) = v5_max_iso();
        // A non-zero turn, so a restore that wrote the zero seed back would not pass.
        Via::new(keyboard.connect())
            .write_encoder_keycode(0, 0, EncoderDirection::Clockwise, Keycode::from(KC_VOLD))
            .unwrap();
        let backup = set_encoder_keycode::<_, GuardError>(
            &mut Via::new(keyboard.connect()),
            &verified,
            0,
            turn(EncoderDirection::Clockwise),
            KC_VOLU,
            |_| Ok(()),
        )
        .unwrap();
        restore::<_, GuardError>(
            &mut Via::new(keyboard.connect()),
            &verified,
            &backup,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(
            encoder_keycode(&keyboard, 0, EncoderDirection::Clockwise),
            Keycode::from(KC_VOLD)
        );
    }

    #[test]
    fn a_guarded_encoder_write_sends_nothing_when_the_backup_is_not_kept() {
        let (keyboard, verified) = v5_max_iso();
        let result = set_encoder_keycode(
            &mut Via::new(keyboard.connect()),
            &verified,
            0,
            turn(EncoderDirection::Clockwise),
            KC_VOLU,
            |_| Err(GuardError::KeymapReadBackMismatch),
        );
        assert_eq!(
            result,
            Err(GuardedWriteError::NothingWritten(
                GuardError::KeymapReadBackMismatch
            ))
        );
        assert_eq!(
            encoder_keycode(&keyboard, 0, EncoderDirection::Clockwise),
            Keycode::from(0)
        );
    }

    #[test]
    fn a_guarded_encoder_write_refuses_a_missing_layer_before_the_backup() {
        let (keyboard, verified) = v5_max_iso();
        let result = set_encoder_keycode::<_, GuardError>(
            &mut Via::new(keyboard.connect()),
            &verified,
            4,
            turn(EncoderDirection::Clockwise),
            KC_VOLU,
            |_| panic!("kept a backup for a layer the keyboard does not have"),
        );
        assert_eq!(
            result,
            Err(GuardedWriteError::NothingWritten(
                GuardError::LayerUnavailable {
                    layer: 4,
                    last_layer: 3
                }
            ))
        );
    }

    #[test]
    fn an_encoder_turn_refuses_an_encoder_the_layout_does_not_have() {
        assert_eq!(
            EncoderTurn::new(1, EncoderDirection::Clockwise),
            Err(GuardError::NoEncoder { encoder: 1 })
        );
    }
}
