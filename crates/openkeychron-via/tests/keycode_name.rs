//! QMK names for keycodes through the public API, and no name where QMK gives none.

#[cfg(test)]
mod tests {
    use openkeychron_via::Keycode;

    fn name(raw: u16) -> Option<String> {
        Keycode::from(raw).name()
    }

    #[test]
    fn names_basic_keycodes_by_their_short_qmk_alias_without_the_kc_prefix() {
        for (raw, expected) in [
            (0x0000, "NO"),
            (0x0001, "TRNS"),
            (0x0004, "A"),
            (0x0014, "Q"),
            (0x0028, "ENT"),
            (0x0033, "SCLN"),
            (0x0047, "SCRL"),
            (0x0048, "PAUS"),
            (0x00a9, "VOLU"),
            (0x00e2, "LALT"),
            (0x00e6, "RALT"),
        ] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn names_basic_keycodes_without_an_alias_by_their_full_qmk_name() {
        for (raw, expected) in [
            (0x0037, "DOT"),
            (0x004a, "HOME"),
            (0x007f, "KB_MUTE"),
            (0x00a0, "OUT"),
        ] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn keeps_the_ms_prefix_of_mouse_keys() {
        for (raw, expected) in [(0x00cd, "MS_UP"), (0x00d1, "MS_BTN1"), (0x00df, "MS_ACL2")] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn gives_no_name_to_basic_keycodes_qmk_leaves_unassigned() {
        let unassigned = [0x02, 0x03]
            .into_iter()
            .chain(0xc3..=0xcc)
            .chain(0xe8..=0xff);
        for raw in unassigned {
            assert_eq!(name(raw), None, "{raw:#06x}");
        }
    }

    #[test]
    fn names_a_modified_key_by_nesting_one_modifier_per_call() {
        for (raw, expected) in [
            (0x0106, "LCTL(C)"),
            (0x0206, "LSFT(C)"),
            (0x0406, "LALT(C)"),
            (0x0806, "LGUI(C)"),
            (0x0306, "LCTL(LSFT(C))"),
            (0x0f04, "LCTL(LSFT(LALT(LGUI(A))))"),
            (0x1106, "RCTL(C)"),
            (0x1c1f, "RALT(RGUI(2))"),
        ] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn names_a_mod_tap_with_one_modifier_by_its_shorthand() {
        for (raw, expected) in [
            (0x2104, "LCTL_T(A)"),
            (0x2829, "LGUI_T(ESC)"),
            (0x322c, "RSFT_T(SPC)"),
        ] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn names_a_mod_tap_with_several_modifiers_by_its_mod_list() {
        for (raw, expected) in [
            (0x2304, "MT(MOD_LCTL | MOD_LSFT, A)"),
            (0x3c04, "MT(MOD_RALT | MOD_RGUI, A)"),
        ] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn names_layer_keycodes() {
        for (raw, expected) in [
            (0x412c, "LT(1, SPC)"),
            (0x4f04, "LT(15, A)"),
            (0x5021, "LM(1, MOD_LCTL)"),
            (0x51e3, "LM(15, MOD_LCTL | MOD_LSFT)"),
            (0x5200, "TO(0)"),
            (0x5221, "MO(1)"),
            (0x5242, "DF(2)"),
            (0x5263, "TG(3)"),
            (0x5284, "OSL(4)"),
            (0x52c5, "TT(5)"),
            (0x52e6, "PDF(6)"),
            (0x523f, "MO(31)"),
        ] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn names_one_shot_modifiers() {
        for (raw, expected) in [
            (0x52a2, "OSM(MOD_LSFT)"),
            (0x52b3, "OSM(MOD_RCTL | MOD_RSFT)"),
        ] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn gives_no_name_when_a_modifier_field_holds_only_the_right_hand_flag_or_nothing() {
        for raw in [0x1004, 0x3004, 0x5020, 0x52a0, 0x52b0] {
            assert_eq!(name(raw), None, "{raw:#06x}");
        }
    }

    #[test]
    fn gives_no_name_when_the_wrapped_key_has_none() {
        for raw in [0x0103, 0x21e8, 0x41ff] {
            assert_eq!(name(raw), None, "{raw:#06x}");
        }
    }

    #[test]
    fn gives_no_name_to_raw_keycodes_including_keychron_custom_ones() {
        for raw in [0x5300, 0x7c00, 0x7e00, 0xbeef, 0xdead, 0xffff] {
            assert_eq!(name(raw), None, "{raw:#06x}");
        }
    }
}
