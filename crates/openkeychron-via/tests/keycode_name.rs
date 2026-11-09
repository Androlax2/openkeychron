//! QMK names for keycodes through the public API, and no name where QMK gives none.
//! Keychron's custom keycodes are named through a registered model, since only a model's
//! firmware build settles them. Expected names are read by hand from the pinned Keychron
//! `quantum/keycodes.h` and `keyboards/keychron/common/keychron_common.h`.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use openkeychron_via::{DeviceRegistry, Keycode, RegisteredModel, ViaDefinition};

    fn name(raw: u16) -> Option<String> {
        Keycode::from(raw).name()
    }

    fn v5_max_iso_name(raw: u16) -> Option<String> {
        let registry = DeviceRegistry::built_in();
        let model = registry.model(0x3434, 0x0951).unwrap();
        model.keycode_name(Keycode::from(raw))
    }

    fn definition_model_name(raw: u16) -> Option<String> {
        let text = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/synthetic/via-definition-second-model.json"),
        )
        .unwrap();
        let definition = ViaDefinition::from_json(&text).unwrap();
        let (vendor_id, product_id) = (definition.vendor_id, definition.product_id);
        let mut registry = DeviceRegistry::built_in();
        registry.register(definition).unwrap();
        let model: &RegisteredModel = registry.model(vendor_id, product_id).unwrap();
        model.keycode_name(Keycode::from(raw))
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
    fn names_quantum_keycodes_by_their_qmk_alias() {
        for (raw, expected) in [
            (0x7c00, "QK_BOOT"),
            (0x7c01, "QK_RBT"),
            (0x7c03, "EE_CLR"),
            (0x7c16, "QK_GESC"),
            (0x7c1a, "SC_LSPO"),
            (0x7c73, "CW_TOGG"),
            (0x7c7b, "QK_LLCK"),
        ] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn names_quantum_keycodes_without_an_alias_by_their_full_qmk_name() {
        for (raw, expected) in [(0x7c04, "QK_MAKE"), (0x7c59, "QK_LOCK")] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn names_lighting_keycodes() {
        for (raw, expected) in [
            (0x7800, "BL_ON"),
            (0x7806, "BL_BRTG"),
            (0x7810, "LM_ON"),
            (0x7820, "UG_TOGG"),
            (0x7827, "QK_UNDERGLOW_VALUE_UP"),
            (0x782b, "RGB_M_P"),
            (0x7834, "RGB_M_TW"),
            (0x7842, "RM_TOGG"),
            (0x784c, "RM_SPDD"),
        ] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn names_swap_hands_magic_macro_and_connection_keycodes() {
        for (raw, expected) in [
            (0x56f0, "SH_TOGG"),
            (0x7000, "CL_SWAP"),
            (0x7013, "NK_TOGG"),
            (0x7700, "MC_0"),
            (0x771f, "MC_31"),
            (0x7785, "OU_2P4G"),
            (0x7793, "BT_PRF1"),
        ] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn names_midi_sequencer_joystick_button_audio_and_steno_keycodes() {
        for (raw, expected) in [
            (0x7103, "MI_C"),
            (0x7104, "MI_Cs"),
            (0x718f, "MI_BNDU"),
            (0x7208, "SQ_SCLR"),
            (0x741f, "JS_31"),
            (0x7440, "PB_1"),
            (0x7495, "AU_PREV"),
            (0x74f0, "QK_STENO_BOLT"),
        ] {
            assert_eq!(name(raw).as_deref(), Some(expected), "{raw:#06x}");
        }
    }

    #[test]
    fn gives_no_name_to_values_between_named_quantum_keycodes() {
        for raw in [
            0x5300, 0x56f7, 0x7023, 0x7190, 0x7720, 0x7787, 0x7798, 0x7807, 0x7835, 0x784d, 0x7c05,
            0x7c7c, 0x7dff,
        ] {
            assert_eq!(name(raw), None, "{raw:#06x}");
        }
    }

    #[test]
    fn gives_no_name_to_keycodes_that_wrap_a_parameter() {
        // SH_T(A), TD(0), UC(0x41) and UP(0, 1): openkeychron does not decode them yet.
        for raw in [0x5604, 0x5700, 0x8041, 0xc001] {
            assert_eq!(name(raw), None, "{raw:#06x}");
        }
    }

    #[test]
    fn gives_no_name_to_keyboard_and_user_keycodes_without_a_model() {
        for raw in [0x7e00, 0x7e0f, 0x7e3f, 0x7e40, 0x7f00, 0x7fff] {
            assert_eq!(name(raw), None, "{raw:#06x}");
        }
    }

    #[test]
    fn names_the_v5_max_iso_keychron_keycodes_in_its_firmware_order() {
        let names: Vec<_> = (0x7e00..=0x7e0f).map(v5_max_iso_name).collect();
        assert_eq!(
            names,
            [
                "LOPTN", "ROPTN", "LCMMD", "RCMMD", "MCTRL", "LNPAD", "TASK", "FILE", "SNAP",
                "CTANA", "SIRI", "BT_HST1", "BT_HST2", "BT_HST3", "P2P4G", "BAT_LVL",
            ]
            .map(|name| Some(name.to_owned()))
        );
    }

    #[test]
    fn gives_no_name_to_keyboard_keycodes_past_the_v5_max_iso_ones() {
        for raw in [0x7e10, 0x7e1f, 0x7e3f, 0x7e40] {
            assert_eq!(v5_max_iso_name(raw), None, "{raw:#06x}");
        }
    }

    #[test]
    fn names_qmk_keycodes_through_a_model_as_without_one() {
        for raw in [0x0004, 0x0106, 0x5221, 0x7c00, 0x7820, 0x0003, 0xbeef] {
            assert_eq!(v5_max_iso_name(raw), name(raw), "{raw:#06x}");
        }
    }

    #[test]
    fn gives_no_name_to_keyboard_keycodes_of_a_model_from_a_definition_file() {
        assert_eq!(definition_model_name(0x7e00), None);
    }

    #[test]
    fn names_qmk_keycodes_of_a_model_from_a_definition_file() {
        assert_eq!(definition_model_name(0x7c00).as_deref(), Some("QK_BOOT"));
    }
}
