//! The guarded keymap write and its undo, run end to end against one mock keyboard.

#[cfg(test)]
mod tests {
    use openkeychron_guard::keymap::{KeyPosition, restore, set_keycode};
    use openkeychron_guard::{GuardError, WriteVerified, require_write_verified};
    use openkeychron_hid::{DeviceIdentity, KeychronDevice};
    use openkeychron_mock::MockKeyboard;
    use openkeychron_via::{Keycode, Via};

    const MATRIX_KEYS: usize = 4 * 6 * 19;
    /// Row 2, column 1: the key labelled `Q` on the FR ISO layout.
    const ROW: u8 = 2;
    const COLUMN: u8 = 1;
    const KC_A: u16 = 0x0004;

    fn v5_max_iso() -> (MockKeyboard, WriteVerified) {
        let keyboard = MockKeyboard::new(DeviceIdentity {
            name: "mock V5 Max ISO".to_owned(),
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
}
