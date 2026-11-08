//! QMK's names for keycodes, as a QMK keymap writes them but without the `KC_` prefix:
//! `A`, `LCTL(C)`, `MO(1)`. Names and macro spellings come from `quantum/keycodes.h`,
//! `quantum/quantum_keycodes.h` and `quantum/modifiers.h` at
//! <https://github.com/Keychron/qmk_firmware/tree/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum>.
//! A basic keycode uses its first `KC_` or `MS_` alias there (`ENT` for `KC_ENTER`), else its
//! full name; 0x00 and 0x01 use `NO` and `TRNS`, not the `XXXXXXX` and `_______` aliases.

use crate::keycode::{Keycode, KeycodeKind, LayerAction};

/// QMK's five-bit modifier field: bits 0 to 3 pick Ctrl, Shift, Alt and GUI, bit 4 makes
/// them all right-hand ones.
const RIGHT_HAND: u8 = 0x10;
const MODIFIER_SUFFIXES: [(u8, &str); 4] =
    [(0x01, "CTL"), (0x02, "SFT"), (0x04, "ALT"), (0x08, "GUI")];

impl Keycode {
    /// QMK's name for this keycode, or `None` when QMK gives it none this crate knows:
    /// quantum, lighting and Keychron's custom keycodes, unassigned basic codes, and a
    /// modifier field with no modifier in it. Callers show the raw value then, never a guess.
    #[must_use]
    pub fn name(self) -> Option<String> {
        match self.kind() {
            KeycodeKind::Basic(key) => basic_name(key).map(str::to_owned),
            KeycodeKind::Modified { modifiers, key } => {
                let key = basic_name(key)?;
                let names = modifier_names(modifiers)?;
                Some(names.iter().rev().fold(key.to_owned(), |inner, modifier| {
                    format!("{modifier}({inner})")
                }))
            }
            KeycodeKind::ModTap { modifiers, key } => {
                let key = basic_name(key)?;
                match modifier_names(modifiers)?.as_slice() {
                    [modifier] => Some(format!("{modifier}_T({key})")),
                    _ => Some(format!("MT({}, {key})", modifier_list(modifiers)?)),
                }
            }
            KeycodeKind::LayerTap { layer, key } => {
                Some(format!("LT({layer}, {})", basic_name(key)?))
            }
            KeycodeKind::LayerMod { layer, modifiers } => {
                Some(format!("LM({layer}, {})", modifier_list(modifiers)?))
            }
            KeycodeKind::Layer { action, layer } => {
                Some(format!("{}({layer})", layer_macro(action)))
            }
            KeycodeKind::OneShotModifiers(modifiers) => {
                Some(format!("OSM({})", modifier_list(modifiers)?))
            }
            KeycodeKind::Raw(_) => None,
        }
    }
}

/// `LCTL`, `RSFT`…, in QMK's bit order; `None` when no modifier bit is set.
fn modifier_names(modifiers: u8) -> Option<Vec<String>> {
    let hand = if modifiers & RIGHT_HAND == 0 {
        'L'
    } else {
        'R'
    };
    let names: Vec<String> = MODIFIER_SUFFIXES
        .iter()
        .filter(|(bit, _)| modifiers & bit != 0)
        .map(|(_, suffix)| format!("{hand}{suffix}"))
        .collect();
    (!names.is_empty()).then_some(names)
}

/// `MOD_LCTL | MOD_LSFT`, as `MT`, `LM` and `OSM` take it.
fn modifier_list(modifiers: u8) -> Option<String> {
    let names = modifier_names(modifiers)?;
    Some(
        names
            .iter()
            .map(|name| format!("MOD_{name}"))
            .collect::<Vec<_>>()
            .join(" | "),
    )
}

fn layer_macro(action: LayerAction) -> &'static str {
    match action {
        LayerAction::To => "TO",
        LayerAction::Momentary => "MO",
        LayerAction::Default => "DF",
        LayerAction::Toggle => "TG",
        LayerAction::OneShot => "OSL",
        LayerAction::TapToggle => "TT",
        LayerAction::PersistentDefault => "PDF",
    }
}

fn basic_name(key: u8) -> Option<&'static str> {
    BASIC_NAMES
        .iter()
        .find(|(code, _)| *code == key)
        .map(|(_, name)| *name)
}

/// Every `KC_*` and `QK_MOUSE_*` value from 0x00 to 0xff in `keycodes.h`.
const BASIC_NAMES: &[(u8, &str)] = &[
    (0x00, "NO"),
    (0x01, "TRNS"),
    (0x04, "A"),
    (0x05, "B"),
    (0x06, "C"),
    (0x07, "D"),
    (0x08, "E"),
    (0x09, "F"),
    (0x0a, "G"),
    (0x0b, "H"),
    (0x0c, "I"),
    (0x0d, "J"),
    (0x0e, "K"),
    (0x0f, "L"),
    (0x10, "M"),
    (0x11, "N"),
    (0x12, "O"),
    (0x13, "P"),
    (0x14, "Q"),
    (0x15, "R"),
    (0x16, "S"),
    (0x17, "T"),
    (0x18, "U"),
    (0x19, "V"),
    (0x1a, "W"),
    (0x1b, "X"),
    (0x1c, "Y"),
    (0x1d, "Z"),
    (0x1e, "1"),
    (0x1f, "2"),
    (0x20, "3"),
    (0x21, "4"),
    (0x22, "5"),
    (0x23, "6"),
    (0x24, "7"),
    (0x25, "8"),
    (0x26, "9"),
    (0x27, "0"),
    (0x28, "ENT"),
    (0x29, "ESC"),
    (0x2a, "BSPC"),
    (0x2b, "TAB"),
    (0x2c, "SPC"),
    (0x2d, "MINS"),
    (0x2e, "EQL"),
    (0x2f, "LBRC"),
    (0x30, "RBRC"),
    (0x31, "BSLS"),
    (0x32, "NUHS"),
    (0x33, "SCLN"),
    (0x34, "QUOT"),
    (0x35, "GRV"),
    (0x36, "COMM"),
    (0x37, "DOT"),
    (0x38, "SLSH"),
    (0x39, "CAPS"),
    (0x3a, "F1"),
    (0x3b, "F2"),
    (0x3c, "F3"),
    (0x3d, "F4"),
    (0x3e, "F5"),
    (0x3f, "F6"),
    (0x40, "F7"),
    (0x41, "F8"),
    (0x42, "F9"),
    (0x43, "F10"),
    (0x44, "F11"),
    (0x45, "F12"),
    (0x46, "PSCR"),
    (0x47, "SCRL"),
    (0x48, "PAUS"),
    (0x49, "INS"),
    (0x4a, "HOME"),
    (0x4b, "PGUP"),
    (0x4c, "DEL"),
    (0x4d, "END"),
    (0x4e, "PGDN"),
    (0x4f, "RGHT"),
    (0x50, "LEFT"),
    (0x51, "DOWN"),
    (0x52, "UP"),
    (0x53, "NUM"),
    (0x54, "PSLS"),
    (0x55, "PAST"),
    (0x56, "PMNS"),
    (0x57, "PPLS"),
    (0x58, "PENT"),
    (0x59, "P1"),
    (0x5a, "P2"),
    (0x5b, "P3"),
    (0x5c, "P4"),
    (0x5d, "P5"),
    (0x5e, "P6"),
    (0x5f, "P7"),
    (0x60, "P8"),
    (0x61, "P9"),
    (0x62, "P0"),
    (0x63, "PDOT"),
    (0x64, "NUBS"),
    (0x65, "APP"),
    (0x66, "KB_POWER"),
    (0x67, "PEQL"),
    (0x68, "F13"),
    (0x69, "F14"),
    (0x6a, "F15"),
    (0x6b, "F16"),
    (0x6c, "F17"),
    (0x6d, "F18"),
    (0x6e, "F19"),
    (0x6f, "F20"),
    (0x70, "F21"),
    (0x71, "F22"),
    (0x72, "F23"),
    (0x73, "F24"),
    (0x74, "EXEC"),
    (0x75, "HELP"),
    (0x76, "MENU"),
    (0x77, "SLCT"),
    (0x78, "STOP"),
    (0x79, "AGIN"),
    (0x7a, "UNDO"),
    (0x7b, "CUT"),
    (0x7c, "COPY"),
    (0x7d, "PSTE"),
    (0x7e, "FIND"),
    (0x7f, "KB_MUTE"),
    (0x80, "KB_VOLUME_UP"),
    (0x81, "KB_VOLUME_DOWN"),
    (0x82, "LCAP"),
    (0x83, "LNUM"),
    (0x84, "LSCR"),
    (0x85, "PCMM"),
    (0x86, "KP_EQUAL_AS400"),
    (0x87, "INT1"),
    (0x88, "INT2"),
    (0x89, "INT3"),
    (0x8a, "INT4"),
    (0x8b, "INT5"),
    (0x8c, "INT6"),
    (0x8d, "INT7"),
    (0x8e, "INT8"),
    (0x8f, "INT9"),
    (0x90, "LNG1"),
    (0x91, "LNG2"),
    (0x92, "LNG3"),
    (0x93, "LNG4"),
    (0x94, "LNG5"),
    (0x95, "LNG6"),
    (0x96, "LNG7"),
    (0x97, "LNG8"),
    (0x98, "LNG9"),
    (0x99, "ERAS"),
    (0x9a, "SYRQ"),
    (0x9b, "CNCL"),
    (0x9c, "CLR"),
    (0x9d, "PRIR"),
    (0x9e, "RETN"),
    (0x9f, "SEPR"),
    (0xa0, "OUT"),
    (0xa1, "OPER"),
    (0xa2, "CLAG"),
    (0xa3, "CRSL"),
    (0xa4, "EXSL"),
    (0xa5, "PWR"),
    (0xa6, "SLEP"),
    (0xa7, "WAKE"),
    (0xa8, "MUTE"),
    (0xa9, "VOLU"),
    (0xaa, "VOLD"),
    (0xab, "MNXT"),
    (0xac, "MPRV"),
    (0xad, "MSTP"),
    (0xae, "MPLY"),
    (0xaf, "MSEL"),
    (0xb0, "EJCT"),
    (0xb1, "MAIL"),
    (0xb2, "CALC"),
    (0xb3, "MYCM"),
    (0xb4, "WSCH"),
    (0xb5, "WHOM"),
    (0xb6, "WBAK"),
    (0xb7, "WFWD"),
    (0xb8, "WSTP"),
    (0xb9, "WREF"),
    (0xba, "WFAV"),
    (0xbb, "MFFD"),
    (0xbc, "MRWD"),
    (0xbd, "BRIU"),
    (0xbe, "BRID"),
    (0xbf, "CPNL"),
    (0xc0, "ASST"),
    (0xc1, "MCTL"),
    (0xc2, "LPAD"),
    (0xcd, "MS_UP"),
    (0xce, "MS_DOWN"),
    (0xcf, "MS_LEFT"),
    (0xd0, "MS_RGHT"),
    (0xd1, "MS_BTN1"),
    (0xd2, "MS_BTN2"),
    (0xd3, "MS_BTN3"),
    (0xd4, "MS_BTN4"),
    (0xd5, "MS_BTN5"),
    (0xd6, "MS_BTN6"),
    (0xd7, "MS_BTN7"),
    (0xd8, "MS_BTN8"),
    (0xd9, "MS_WHLU"),
    (0xda, "MS_WHLD"),
    (0xdb, "MS_WHLL"),
    (0xdc, "MS_WHLR"),
    (0xdd, "MS_ACL0"),
    (0xde, "MS_ACL1"),
    (0xdf, "MS_ACL2"),
    (0xe0, "LCTL"),
    (0xe1, "LSFT"),
    (0xe2, "LALT"),
    (0xe3, "LGUI"),
    (0xe4, "RCTL"),
    (0xe5, "RSFT"),
    (0xe6, "RALT"),
    (0xe7, "RGUI"),
];
