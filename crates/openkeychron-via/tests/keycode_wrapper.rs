//! Building QMK's two-part keycodes (`LCTL(C)`, `MT(MOD_LSFT, A)`, `LT(1, SPC)`,
//! `LM(1, MOD_LCTL)`) through the public API. Expected values are worked out by hand from the
//! macros in the pinned Keychron `quantum/quantum_keycodes.h`, `quantum/keycodes.h` and
//! `quantum/modifiers.h`:
//! <https://github.com/Keychron/qmk_firmware/tree/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum>.

#[cfg(test)]
mod tests {
    use openkeychron_via::{Keycode, KeycodeError, KeycodeKind, KeycodeWrapper, modifiers_name};

    const KC_A: u8 = 0x04;
    const KC_C: u8 = 0x06;
    const KC_ENT: u8 = 0x28;
    const KC_SPC: u8 = 0x2c;
    const MOD_LCTL: u8 = 0x01;
    const MOD_LSFT: u8 = 0x02;
    const MOD_RCTL: u8 = 0x11;
    const MOD_RSFT: u8 = 0x12;

    /// Every modifier field QMK encodes: at least one of Ctrl, Shift, Alt and GUI, either hand.
    fn modifier_fields() -> impl Iterator<Item = u8> {
        (0x01..=0x1f).filter(|field| field & 0x0f != 0)
    }

    fn wrapped(wrapper: KeycodeWrapper, inner: u8) -> u16 {
        u16::from(wrapper.wrap(inner).unwrap())
    }

    #[test]
    fn wraps_a_key_in_one_left_hand_modifier_as_lctl_does() {
        // LCTL(kc) is QK_LCTL (0x0100) | kc.
        assert_eq!(wrapped(KeycodeWrapper::Modified(MOD_LCTL), KC_C), 0x0106);
    }

    #[test]
    fn wraps_a_key_in_several_modifiers_as_nested_modifier_macros_do() {
        // LCTL(LSFT(kc)) is QK_LCTL | QK_LSFT (0x0200) | kc.
        assert_eq!(
            wrapped(KeycodeWrapper::Modified(MOD_LCTL | MOD_LSFT), KC_A),
            0x0304
        );
    }

    #[test]
    fn wraps_a_key_in_right_hand_modifiers_as_rctl_does() {
        // RCTL(kc) is QK_RCTL (0x1100) | kc.
        assert_eq!(wrapped(KeycodeWrapper::Modified(MOD_RCTL), KC_A), 0x1104);
    }

    #[test]
    fn builds_a_mod_tap_as_mt_does() {
        // MT(mod, kc) is QK_MOD_TAP (0x2000) | (mod & 0x1f) << 8 | kc.
        assert_eq!(wrapped(KeycodeWrapper::ModTap(MOD_LSFT), KC_A), 0x2204);
    }

    #[test]
    fn builds_a_right_hand_mod_tap_as_rsft_t_does() {
        // RSFT_T(kc) is MT(MOD_RSFT, kc).
        assert_eq!(wrapped(KeycodeWrapper::ModTap(MOD_RSFT), KC_ENT), 0x3228);
    }

    #[test]
    fn builds_a_layer_tap_as_lt_does() {
        // LT(layer, kc) is QK_LAYER_TAP (0x4000) | (layer & 0xf) << 8 | kc.
        assert_eq!(wrapped(KeycodeWrapper::LayerTap(1), KC_SPC), 0x412c);
    }

    #[test]
    fn builds_a_layer_tap_on_the_last_layer_it_can_hold() {
        assert_eq!(wrapped(KeycodeWrapper::LayerTap(15), 0xff), 0x4fff);
    }

    #[test]
    fn builds_a_layer_mod_as_lm_does() {
        // LM(layer, mod) is QK_LAYER_MOD (0x5000) | (layer & 0xf) << 5 | mod & 0x1f.
        assert_eq!(wrapped(KeycodeWrapper::LayerMod(1), MOD_LCTL), 0x5021);
    }

    #[test]
    fn builds_a_layer_mod_on_the_last_layer_with_every_right_hand_modifier() {
        assert_eq!(wrapped(KeycodeWrapper::LayerMod(15), 0x1f), 0x51ff);
    }

    #[test]
    fn rejects_modifiers_without_any_modifier() {
        assert_eq!(
            KeycodeWrapper::Modified(0).wrap(KC_A),
            Err(KeycodeError::InvalidModifiers(0))
        );
    }

    #[test]
    fn rejects_modifiers_holding_only_the_right_hand_flag() {
        assert_eq!(
            KeycodeWrapper::ModTap(0x10).wrap(KC_A),
            Err(KeycodeError::InvalidModifiers(0x10))
        );
    }

    #[test]
    fn rejects_modifiers_past_the_five_bit_field() {
        assert_eq!(
            KeycodeWrapper::Modified(0x21).wrap(KC_A),
            Err(KeycodeError::InvalidModifiers(0x21))
        );
    }

    #[test]
    fn rejects_a_layer_past_the_four_bits_lt_holds() {
        assert_eq!(
            KeycodeWrapper::LayerTap(16).wrap(KC_A),
            Err(KeycodeError::LayerOutOfRange(16))
        );
    }

    #[test]
    fn rejects_a_layer_mod_without_any_modifier() {
        assert_eq!(
            KeycodeWrapper::LayerMod(1).wrap(0x10),
            Err(KeycodeError::InvalidModifiers(0x10))
        );
    }

    #[test]
    fn says_what_a_modifier_field_must_hold() {
        assert_eq!(
            KeycodeError::InvalidModifiers(0x10).to_string(),
            "modifier field 0x10 holds no modifier: expected one or more of Ctrl (0x01), \
             Shift (0x02), Alt (0x04) and GUI (0x08), plus 0x10 for the right-hand ones"
        );
    }

    #[test]
    fn says_which_layers_a_two_part_keycode_holds() {
        assert_eq!(
            KeycodeError::LayerOutOfRange(16).to_string(),
            "layer 16 does not fit a two-part keycode: it holds layers 0 to 15"
        );
    }

    #[test]
    fn sets_the_bits_the_inner_part_does_not_fill() {
        assert_eq!(KeycodeWrapper::LayerMod(1).bits(), Ok(0x5020));
    }

    #[test]
    fn every_wrapped_keycode_is_its_bits_with_the_inner_part() {
        for wrapper in every_wrapper() {
            for inner in inners(wrapper) {
                assert_eq!(
                    wrapped(wrapper, inner),
                    wrapper.bits().unwrap() | u16::from(inner),
                    "{wrapper:?} around {inner:#04x}"
                );
            }
        }
    }

    #[test]
    fn every_wrapped_keycode_decodes_back_to_its_parts() {
        for wrapper in every_wrapper() {
            for inner in inners(wrapper) {
                let expected = match wrapper {
                    KeycodeWrapper::Modified(modifiers) => KeycodeKind::Modified {
                        modifiers,
                        key: inner,
                    },
                    KeycodeWrapper::ModTap(modifiers) => KeycodeKind::ModTap {
                        modifiers,
                        key: inner,
                    },
                    KeycodeWrapper::LayerTap(layer) => KeycodeKind::LayerTap { layer, key: inner },
                    KeycodeWrapper::LayerMod(layer) => KeycodeKind::LayerMod {
                        layer,
                        modifiers: inner,
                    },
                };
                assert_eq!(wrapper.wrap(inner).unwrap().kind(), expected);
            }
        }
    }

    fn every_wrapper() -> Vec<KeycodeWrapper> {
        modifier_fields()
            .flat_map(|field| {
                [
                    KeycodeWrapper::Modified(field),
                    KeycodeWrapper::ModTap(field),
                ]
            })
            .chain((0..=15).flat_map(|layer| {
                [
                    KeycodeWrapper::LayerTap(layer),
                    KeycodeWrapper::LayerMod(layer),
                ]
            }))
            .collect()
    }

    /// Each inner part `wrapper` takes: a basic keycode, or a modifier field for `LM`.
    fn inners(wrapper: KeycodeWrapper) -> Vec<u8> {
        match wrapper {
            KeycodeWrapper::LayerMod(_) => modifier_fields().collect(),
            _ => (0..=0xff).collect(),
        }
    }

    fn name_around(wrapper: KeycodeWrapper) -> (String, String) {
        let name = wrapper.name().unwrap();
        (name.before, name.after)
    }

    #[test]
    fn names_a_modifier_wrapper_by_nesting_one_modifier_per_call() {
        assert_eq!(
            name_around(KeycodeWrapper::Modified(MOD_LCTL | MOD_LSFT)),
            ("LCTL(LSFT(".to_owned(), "))".to_owned())
        );
    }

    #[test]
    fn names_a_mod_tap_wrapper_with_one_modifier_by_its_shorthand() {
        assert_eq!(
            name_around(KeycodeWrapper::ModTap(MOD_LSFT)),
            ("LSFT_T(".to_owned(), ")".to_owned())
        );
    }

    #[test]
    fn names_a_mod_tap_wrapper_with_several_modifiers_by_its_mod_list() {
        assert_eq!(
            name_around(KeycodeWrapper::ModTap(MOD_RCTL | MOD_RSFT)),
            ("MT(MOD_RCTL | MOD_RSFT, ".to_owned(), ")".to_owned())
        );
    }

    #[test]
    fn names_a_layer_tap_wrapper() {
        assert_eq!(
            name_around(KeycodeWrapper::LayerTap(1)),
            ("LT(1, ".to_owned(), ")".to_owned())
        );
    }

    #[test]
    fn names_a_layer_mod_wrapper() {
        assert_eq!(
            name_around(KeycodeWrapper::LayerMod(2)),
            ("LM(2, ".to_owned(), ")".to_owned())
        );
    }

    #[test]
    fn gives_no_wrapper_name_to_modifiers_without_any_modifier() {
        assert_eq!(
            KeycodeWrapper::Modified(0x10).name(),
            Err(KeycodeError::InvalidModifiers(0x10))
        );
    }

    #[test]
    fn names_a_modifier_field_as_mt_lm_and_osm_take_it() {
        assert_eq!(
            modifiers_name(MOD_RCTL | MOD_RSFT),
            Ok("MOD_RCTL | MOD_RSFT".to_owned())
        );
    }

    #[test]
    fn gives_no_name_to_a_modifier_field_without_any_modifier() {
        assert_eq!(modifiers_name(0), Err(KeycodeError::InvalidModifiers(0)));
    }

    #[test]
    fn every_wrapped_keycode_is_named_by_its_wrapper_around_its_inner_part() {
        for wrapper in every_wrapper() {
            let name = wrapper.name().unwrap();
            for inner in inners(wrapper) {
                let inner_name = match wrapper {
                    KeycodeWrapper::LayerMod(_) => modifiers_name(inner).ok(),
                    _ => Keycode::from(u16::from(inner)).name(),
                };
                let expected = inner_name
                    .map(|inner_name| format!("{}{inner_name}{}", name.before, name.after));
                assert_eq!(
                    wrapper.wrap(inner).unwrap().name(),
                    expected,
                    "{wrapper:?} around {inner:#04x}"
                );
            }
        }
    }
}
