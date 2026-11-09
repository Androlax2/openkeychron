//! The keycodes the remap panel offers, grouped by category, each under the name
//! `Keycode::name` gives it, and the parts it builds two-part keycodes from. `bindings.rs`
//! writes them into `app/src/lib/bindings.ts`, so the UI names a keycode exactly as the
//! keyboard drawing does, without a command to ask the app.
//!
//! The list offers only keycodes QMK names on their own: no custom, lighting or Keychron
//! keycode. Modifier combinations, mod-taps, layer-taps and layer-mods are built from a
//! wrapper and an inner part: a basic keycode from the list, or for a layer-mod a modifier set.
//! Every other named keycode is only named, so a raw value shows its name before it is applied.

use openkeychron_via::{Keycode, KeycodeKind, KeycodeWrapper, modifiers_name};
use serde::Serialize;

#[derive(Debug, PartialEq, Serialize, ts_rs::TS)]
pub struct KeycodeCategory {
    pub name: &'static str,
    pub keycodes: Vec<KeycodeChoice>,
}

#[derive(Debug, PartialEq, Serialize, ts_rs::TS)]
pub struct KeycodeChoice {
    pub keycode: u16,
    /// QMK's name, such as `ESC` or `MO(1)`.
    pub name: String,
    /// The layer a layer keycode switches to, so the UI can offer only the keyboard's layers.
    pub layer: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum WrapperBuilder {
    Modified,
    ModTap,
    LayerTap,
    LayerMod,
}

/// The part of a two-part keycode the panel builds around an inner part.
#[derive(Debug, PartialEq, Serialize, ts_rs::TS)]
pub struct KeycodeWrapperChoice {
    pub builder: WrapperBuilder,
    /// The bits every keycode built with it has: the built keycode is `keycode | inner`, where
    /// `inner` is a basic keycode, or a `ModifierChoice.modifiers` for a layer-mod.
    pub keycode: u16,
    /// QMK's name of the built keycode is `before`, the inner part's name, then `after`.
    pub before: String,
    pub after: String,
    /// The `ModifierChoice.modifiers` of a modifier combination or mod-tap.
    pub modifiers: Option<u8>,
    /// The layer of a layer-tap or layer-mod.
    pub layer: Option<u8>,
}

/// A set of modifiers QMK can encode: one or more of Ctrl, Shift, Alt and GUI, all left-hand
/// or all right-hand.
#[derive(Debug, PartialEq, Serialize, ts_rs::TS)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one flag for each modifier checkbox the panel matches a set by"
)]
#[serde(rename_all = "camelCase")]
pub struct ModifierChoice {
    /// QMK's five-bit modifier field.
    pub modifiers: u8,
    /// As `MT`, `LM` and `OSM` take it: `MOD_LCTL | MOD_LSFT`.
    pub name: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub gui: bool,
    pub right_hand: bool,
}

/// The basic keycodes (0x00 to 0xff) of each category, as `(first, last)` runs of the values
/// `keycode_name.rs` names.
const BASIC_CATEGORIES: &[(&str, &[(u8, u8)])] = &[
    ("Special", &[(0x00, 0x01)]),
    ("Letters", &[(0x04, 0x1d)]),
    ("Numbers", &[(0x1e, 0x27)]),
    (
        "Editing",
        &[(0x28, 0x2c), (0x39, 0x39), (0x49, 0x49), (0x4c, 0x4c)],
    ),
    ("Punctuation", &[(0x2d, 0x38), (0x64, 0x64)]),
    ("Navigation", &[(0x4a, 0x4b), (0x4d, 0x52)]),
    ("Function keys", &[(0x3a, 0x45), (0x68, 0x73)]),
    ("Modifiers", &[(0xe0, 0xe7)]),
    ("Numpad", &[(0x53, 0x63), (0x67, 0x67), (0x85, 0x86)]),
    (
        "Media and system",
        &[(0x66, 0x66), (0x7f, 0x81), (0xa5, 0xc2)],
    ),
    ("Mouse", &[(0xcd, 0xdf)]),
    ("International", &[(0x87, 0x98)]),
    (
        "Other",
        &[
            (0x46, 0x48),
            (0x65, 0x65),
            (0x74, 0x7e),
            (0x82, 0x84),
            (0x99, 0xa4),
        ],
    ),
];

/// Every category, the layer keycodes after the modifiers.
pub fn categories() -> Vec<KeycodeCategory> {
    let mut categories: Vec<KeycodeCategory> = BASIC_CATEGORIES
        .iter()
        .map(|(name, runs)| KeycodeCategory {
            name,
            keycodes: runs
                .iter()
                .flat_map(|&(first, last)| first..=last)
                .map(|key| choice(u16::from(key), None))
                .collect(),
        })
        .collect();
    let after_modifiers = 1 + categories
        .iter()
        .position(|category| category.name == "Modifiers")
        .unwrap();
    categories.insert(after_modifiers, layer_category());
    categories
}

/// `TO(n)`, `MO(n)` and every other keycode QMK's layer range names, for each layer it encodes.
fn layer_category() -> KeycodeCategory {
    KeycodeCategory {
        name: "Layers",
        keycodes: (0x5200..=0x52ff)
            .filter_map(|raw| match Keycode::from(raw).kind() {
                KeycodeKind::Layer { layer, .. } => Some(choice(raw, Some(layer))),
                _ => None,
            })
            .collect(),
    }
}

/// Every wrapper the panel builds with: modifier combinations and mod-taps for each modifier
/// set, layer-taps and layer-mods for each layer they hold.
pub fn wrappers() -> Vec<KeycodeWrapperChoice> {
    let modifier_wrappers = [
        (
            WrapperBuilder::Modified,
            KeycodeWrapper::Modified as fn(u8) -> _,
        ),
        (WrapperBuilder::ModTap, KeycodeWrapper::ModTap),
    ]
    .into_iter()
    .flat_map(|(builder, wrapper)| {
        modifier_fields().map(move |modifiers| wrapper_choice(builder, wrapper(modifiers)))
    });
    let layer_wrappers = [
        (
            WrapperBuilder::LayerTap,
            KeycodeWrapper::LayerTap as fn(u8) -> _,
        ),
        (WrapperBuilder::LayerMod, KeycodeWrapper::LayerMod),
    ]
    .into_iter()
    .flat_map(|(builder, wrapper)| {
        (0..=LAST_WRAPPED_LAYER).map(move |layer| wrapper_choice(builder, wrapper(layer)))
    });
    modifier_wrappers.chain(layer_wrappers).collect()
}

/// Every modifier set, left-hand ones first.
pub fn modifier_choices() -> Vec<ModifierChoice> {
    modifier_fields()
        .map(|modifiers| ModifierChoice {
            modifiers,
            name: modifiers_name(modifiers)
                .unwrap_or_else(|error| panic!("the catalog offers modifiers that {error}")),
            ctrl: modifiers & MOD_CTRL != 0,
            shift: modifiers & MOD_SHIFT != 0,
            alt: modifiers & MOD_ALT != 0,
            gui: modifiers & MOD_GUI != 0,
            right_hand: modifiers & RIGHT_HAND != 0,
        })
        .collect()
}

/// Every named keycode the panel neither lists nor builds, such as `QK_BOOT` or
/// `OSM(MOD_LSFT)`.
pub fn named_only() -> Vec<KeycodeChoice> {
    (0..=u16::MAX)
        .filter(|&raw| {
            matches!(
                Keycode::from(raw).kind(),
                KeycodeKind::OneShotModifiers(_) | KeycodeKind::Raw(_)
            )
        })
        .filter_map(|raw| {
            Some(KeycodeChoice {
                keycode: raw,
                name: Keycode::from(raw).name()?,
                layer: None,
            })
        })
        .collect()
}

// QMK's modifier bits from `quantum/modifiers.h`: `MOD_LCTL` to `MOD_LGUI`, and the bit
// that makes them right-hand ones.
const MOD_CTRL: u8 = 0x01;
const MOD_SHIFT: u8 = 0x02;
const MOD_ALT: u8 = 0x04;
const MOD_GUI: u8 = 0x08;
const RIGHT_HAND: u8 = 0x10;
const LAST_WRAPPED_LAYER: u8 = 15;

/// Every five-bit modifier field with at least one modifier in it.
fn modifier_fields() -> impl Iterator<Item = u8> {
    (0x01..=0x1f).filter(|field| field & !RIGHT_HAND != 0)
}

fn wrapper_choice(builder: WrapperBuilder, wrapper: KeycodeWrapper) -> KeycodeWrapperChoice {
    let fail = |error| -> ! { panic!("the catalog offers {wrapper:?}, but {error}") };
    let name = wrapper.name().unwrap_or_else(|error| fail(error));
    let (modifiers, layer) = match wrapper {
        KeycodeWrapper::Modified(modifiers) | KeycodeWrapper::ModTap(modifiers) => {
            (Some(modifiers), None)
        }
        KeycodeWrapper::LayerTap(layer) | KeycodeWrapper::LayerMod(layer) => (None, Some(layer)),
    };
    KeycodeWrapperChoice {
        builder,
        keycode: wrapper.bits().unwrap_or_else(|error| fail(error)),
        before: name.before,
        after: name.after,
        modifiers,
        layer,
    }
}

fn choice(keycode: u16, layer: Option<u8>) -> KeycodeChoice {
    let name = Keycode::from(keycode)
        .name()
        .unwrap_or_else(|| panic!("the catalog offers {keycode:#06x}, which QMK does not name"));
    KeycodeChoice {
        keycode,
        name,
        layer,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use openkeychron_hid::fixture_device;
    use openkeychron_mock::MockKeyboard;
    use openkeychron_via::Keycode;

    use super::{
        KeycodeChoice, WrapperBuilder, categories, modifier_choices, named_only, wrappers,
    };
    use crate::keymap_edit::{KeyAssignment, KeymapHistory, set_keycode};
    use crate::test_support::synthetic_path;

    fn offered() -> Vec<u16> {
        categories()
            .into_iter()
            .flat_map(|category| category.keycodes)
            .map(|choice| choice.keycode)
            .collect()
    }

    #[test]
    fn offers_every_named_basic_keycode_exactly_once() {
        let mut basic: Vec<u16> = offered()
            .into_iter()
            .filter(|keycode| *keycode <= 0xff)
            .collect();
        basic.sort_unstable();
        let named: Vec<u16> = (0..=0xff)
            .filter(|keycode| Keycode::from(*keycode).name().is_some())
            .collect();
        assert_eq!(basic, named);
    }

    #[test]
    fn names_each_keycode_as_the_keyboard_drawing_does() {
        let escape = categories()
            .into_iter()
            .find(|category| category.name == "Editing")
            .unwrap()
            .keycodes
            .into_iter()
            .find(|choice| choice.keycode == 0x29)
            .unwrap();
        assert_eq!(
            (escape.name, escape.layer),
            (Keycode::from(0x29).name().unwrap(), None)
        );
    }

    #[test]
    fn offers_the_layer_keycodes_with_the_layer_they_switch_to() {
        let layers = categories()
            .into_iter()
            .find(|category| category.name == "Layers")
            .unwrap();
        let momentary_one = layers
            .keycodes
            .iter()
            .find(|choice| choice.name == "MO(1)")
            .unwrap();
        assert_eq!(
            (
                momentary_one.keycode,
                momentary_one.layer,
                layers.keycodes.len()
            ),
            (0x5221, Some(1), 7 * 32)
        );
    }

    #[test]
    fn lists_the_categories_in_the_panel_order() {
        let names: Vec<&str> = categories().iter().map(|category| category.name).collect();
        assert_eq!(
            names,
            [
                "Special",
                "Letters",
                "Numbers",
                "Editing",
                "Punctuation",
                "Navigation",
                "Function keys",
                "Modifiers",
                "Layers",
                "Numpad",
                "Media and system",
                "Mouse",
                "International",
                "Other",
            ]
        );
    }

    fn basic_choices() -> Vec<KeycodeChoice> {
        categories()
            .into_iter()
            .flat_map(|category| category.keycodes)
            .filter(|choice| choice.layer.is_none())
            .collect()
    }

    /// Every keycode the panel builds, as the UI builds it: a wrapper's bits with an inner
    /// part's value, named by the wrapper's text around the inner part's name.
    fn built() -> Vec<(u16, String)> {
        let basics = basic_choices();
        let modifiers = modifier_choices();
        wrappers()
            .into_iter()
            .flat_map(|wrapper| {
                let inners: Vec<(u16, String)> = match wrapper.builder {
                    WrapperBuilder::LayerMod => modifiers
                        .iter()
                        .map(|choice| (u16::from(choice.modifiers), choice.name.clone()))
                        .collect(),
                    _ => basics
                        .iter()
                        .map(|choice| (choice.keycode, choice.name.clone()))
                        .collect(),
                };
                inners.into_iter().map(move |(value, name)| {
                    (
                        wrapper.keycode | value,
                        format!("{}{name}{}", wrapper.before, wrapper.after),
                    )
                })
            })
            .collect()
    }

    #[test]
    fn offers_only_basic_keycodes_without_a_layer() {
        assert!(basic_choices().iter().all(|choice| choice.keycode <= 0xff));
    }

    #[test]
    fn names_every_built_keycode_as_keycode_name_does() {
        for (keycode, name) in built() {
            assert_eq!(Some(name), Keycode::from(keycode).name(), "{keycode:#06x}");
        }
    }

    #[test]
    fn names_every_named_keycode_exactly_once_between_the_list_the_builders_and_the_rest() {
        let listed = categories()
            .into_iter()
            .flat_map(|category| category.keycodes)
            .chain(named_only())
            .map(|choice| (choice.keycode, choice.name));
        let mut known: BTreeMap<u16, String> = BTreeMap::new();
        for (keycode, name) in listed.chain(built()) {
            assert_eq!(known.insert(keycode, name), None, "{keycode:#06x} twice");
        }
        let named: BTreeMap<u16, String> = (0..=u16::MAX)
            .filter_map(|keycode| Some((keycode, Keycode::from(keycode).name()?)))
            .collect();
        assert_eq!(known, named);
    }

    #[test]
    fn offers_each_wrapper_for_every_modifier_set_and_layer_it_takes() {
        let count = |builder| {
            wrappers()
                .iter()
                .filter(|wrapper| wrapper.builder == builder)
                .count()
        };
        assert_eq!(
            [
                count(WrapperBuilder::Modified),
                count(WrapperBuilder::ModTap),
                count(WrapperBuilder::LayerTap),
                count(WrapperBuilder::LayerMod),
            ],
            [30, 30, 16, 16]
        );
    }

    #[test]
    fn describes_a_mod_tap_wrapper_by_its_modifiers_and_text() {
        let left_shift_tap = wrappers()
            .into_iter()
            .find(|wrapper| {
                wrapper.builder == WrapperBuilder::ModTap && wrapper.modifiers == Some(0x02)
            })
            .unwrap();
        assert_eq!(
            (
                left_shift_tap.keycode,
                left_shift_tap.before,
                left_shift_tap.after,
                left_shift_tap.layer
            ),
            (0x2200, "LSFT_T(".to_owned(), ")".to_owned(), None)
        );
    }

    #[test]
    fn describes_a_layer_tap_wrapper_by_its_layer() {
        let layer_one_tap = wrappers()
            .into_iter()
            .find(|wrapper| wrapper.builder == WrapperBuilder::LayerTap && wrapper.layer == Some(1))
            .unwrap();
        assert_eq!(
            (
                layer_one_tap.keycode,
                layer_one_tap.before,
                layer_one_tap.modifiers
            ),
            (0x4100, "LT(1, ".to_owned(), None)
        );
    }

    #[test]
    fn spells_out_each_modifier_set() {
        let right_ctrl_shift = modifier_choices()
            .into_iter()
            .find(|choice| choice.modifiers == 0x13)
            .unwrap();
        assert_eq!(
            (
                right_ctrl_shift.name.as_str(),
                right_ctrl_shift.ctrl,
                right_ctrl_shift.shift,
                right_ctrl_shift.alt,
                right_ctrl_shift.gui,
                right_ctrl_shift.right_hand
            ),
            ("MOD_RCTL | MOD_RSFT", true, true, false, false, true)
        );
    }

    #[test]
    fn names_the_quantum_keycodes_the_panel_does_not_offer() {
        let boot = named_only()
            .into_iter()
            .find(|choice| choice.keycode == 0x7c00)
            .unwrap();
        assert_eq!((boot.name.as_str(), boot.layer), ("QK_BOOT", None));
    }

    #[test]
    fn the_mock_keyboard_reads_back_a_built_keycode_under_the_name_the_panel_built() {
        let keyboard = MockKeyboard::load(&synthetic_path("v5-max-iso.mock.toml")).unwrap();
        let device = fixture_device(keyboard.identity(), Path::new("mock")).unwrap();
        let mut history = KeymapHistory::default();
        let built: BTreeMap<u16, String> = built().into_iter().collect();
        // LCTL(C), LSFT_T(A), LT(1, SPC) and LM(1, MOD_LCTL) on the key labelled A.
        for keycode in [0x0106, 0x2204, 0x412c, 0x5021] {
            let assignment = KeyAssignment {
                layer: 0,
                row: 2,
                column: 1,
                keycode,
            };
            let edited =
                set_keycode(keyboard.connect(), &device, &mut history, assignment).unwrap();
            let key = edited.keymap.layers[0]
                .keys
                .iter()
                .find(|key| (key.row, key.column) == (2, 1))
                .unwrap();
            assert_eq!(
                (key.keycode, key.name.as_ref()),
                (keycode, built.get(&keycode))
            );
        }
    }
}
