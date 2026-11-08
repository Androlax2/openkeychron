//! Lossless QMK keycode classification through the public API.

#[cfg(test)]
mod tests {
    use openkeychron_via::{Keycode, KeycodeKind, LayerAction};

    #[test]
    fn keycode_classifies_basic_and_standalone_modifier_keys() {
        for (raw, expected) in [
            (0x0000, 0),
            (0x0001, 1),
            (0x0004, 4),
            (0x00e0, 224),
            (0x00e7, 231),
            (0x00ff, 255),
        ] {
            assert_eq!(Keycode::from(raw).kind(), KeycodeKind::Basic(expected));
        }
    }

    #[test]
    fn keycode_decodes_modifier_and_tap_bitfields_without_losing_right_side_bits() {
        let cases = [
            (
                0x0104,
                KeycodeKind::Modified {
                    modifiers: 1,
                    key: 4,
                },
            ),
            (
                0x1104,
                KeycodeKind::Modified {
                    modifiers: 17,
                    key: 4,
                },
            ),
            (
                0x1fff,
                KeycodeKind::Modified {
                    modifiers: 31,
                    key: 255,
                },
            ),
            (
                0x2000,
                KeycodeKind::ModTap {
                    modifiers: 0,
                    key: 0,
                },
            ),
            (
                0x3104,
                KeycodeKind::ModTap {
                    modifiers: 17,
                    key: 4,
                },
            ),
            (
                0x3fff,
                KeycodeKind::ModTap {
                    modifiers: 31,
                    key: 255,
                },
            ),
            (0x4000, KeycodeKind::LayerTap { layer: 0, key: 0 }),
            (0x4204, KeycodeKind::LayerTap { layer: 2, key: 4 }),
            (
                0x4fff,
                KeycodeKind::LayerTap {
                    layer: 15,
                    key: 255,
                },
            ),
            (
                0x5000,
                KeycodeKind::LayerMod {
                    layer: 0,
                    modifiers: 0,
                },
            ),
            (
                0x5051,
                KeycodeKind::LayerMod {
                    layer: 2,
                    modifiers: 17,
                },
            ),
            (
                0x51ff,
                KeycodeKind::LayerMod {
                    layer: 15,
                    modifiers: 31,
                },
            ),
            (0x52a0, KeycodeKind::OneShotModifiers(0)),
            (0x52b1, KeycodeKind::OneShotModifiers(17)),
            (0x52bf, KeycodeKind::OneShotModifiers(31)),
        ];
        for (raw, expected) in cases {
            assert_eq!(Keycode::from(raw).kind(), expected, "{raw:04x}");
        }
    }

    #[test]
    fn keycode_decodes_each_layer_action_at_its_boundaries() {
        for (first, last, action) in [
            (0x5200, 0x521f, LayerAction::To),
            (0x5220, 0x523f, LayerAction::Momentary),
            (0x5240, 0x525f, LayerAction::Default),
            (0x5260, 0x527f, LayerAction::Toggle),
            (0x5280, 0x529f, LayerAction::OneShot),
            (0x52c0, 0x52df, LayerAction::TapToggle),
            (0x52e0, 0x52ff, LayerAction::PersistentDefault),
        ] {
            assert_eq!(
                Keycode::from(first).kind(),
                KeycodeKind::Layer { action, layer: 0 }
            );
            assert_eq!(
                Keycode::from(last).kind(),
                KeycodeKind::Layer { action, layer: 31 }
            );
        }
    }

    #[test]
    fn keycode_keeps_other_ranges_including_keychron_custom_values_raw() {
        for raw in [
            0x5300, 0x5600, 0x7700, 0x7840, 0x7e00, 0x7e3f, 0x7fff, 0x8000, 0xffff,
        ] {
            assert_eq!(Keycode::from(raw).kind(), KeycodeKind::Raw(raw));
        }
    }

    #[test]
    fn keycode_preserves_every_possible_raw_value() {
        for raw in u16::MIN..=u16::MAX {
            assert_eq!(u16::from(Keycode::from(raw)), raw);
        }
    }
}
