//! The keycodes the remap panel offers, grouped by category, each under the name
//! `Keycode::name` gives it. `bindings.rs` writes them into `app/src/lib/bindings.ts` as
//! `KEYCODE_CATEGORIES`, so the UI names a keycode exactly as the keyboard drawing does.
//!
//! Only keycodes QMK names are offered: no custom, lighting or Keychron keycode, and no
//! modifier combination, mod-tap or layer-tap, which take more than one choice to build.

use openkeychron_via::{Keycode, KeycodeKind};
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
    use openkeychron_via::Keycode;

    use super::categories;

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
}
