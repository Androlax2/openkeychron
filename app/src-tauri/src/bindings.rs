//! Keeps `app/src/lib/bindings.ts`, the TypeScript types of what the commands take and return
//! and the remap panel's keycode catalog, equal to the Rust side. A change to either fails this
//! test until the file is regenerated:
//!
//! ```sh
//! OPENKEYCHRON_WRITE_BINDINGS=1 cargo test -p openkeychron-app bindings
//! ```

use std::fmt::Write;

use ts_rs::{Config, TS};

use crate::device::Device;
use crate::info::{DeviceInfo, KeychronFirmware};
use crate::keycode_catalog::{
    self, KeycodeCategory, KeycodeChoice, KeycodeWrapperChoice, ModifierChoice, WrapperBuilder,
};
use crate::keymap::{Bounds, Key, Keymap, Layer};
use crate::keymap_edit::{EditableKeymap, KeyAssignment};
use crate::macros::{Action, MacroContent, MacroList, MacroSlot};
use crate::settings::{
    Debounce, DebounceAlgorithm, Settings, SnapClickMode, SnapClickPair, WirelessTimers,
};

const HEADER: &str = "// Generated from app/src-tauri by `OPENKEYCHRON_WRITE_BINDINGS=1 cargo test -p \
                      openkeychron-app bindings`. Do not edit.\n";

fn bindings() -> String {
    let config = Config::new();
    let declarations = [
        Device::decl(&config),
        DeviceInfo::decl(&config),
        KeychronFirmware::decl(&config),
        Keymap::decl(&config),
        Layer::decl(&config),
        Key::decl(&config),
        Bounds::decl(&config),
        EditableKeymap::decl(&config),
        KeyAssignment::decl(&config),
        KeycodeCategory::decl(&config),
        KeycodeChoice::decl(&config),
        WrapperBuilder::decl(&config),
        KeycodeWrapperChoice::decl(&config),
        ModifierChoice::decl(&config),
        MacroList::decl(&config),
        MacroSlot::decl(&config),
        MacroContent::decl(&config),
        Action::decl(&config),
        Settings::decl(&config),
        Debounce::decl(&config),
        DebounceAlgorithm::decl(&config),
        SnapClickPair::decl(&config),
        SnapClickMode::decl(&config),
        WirelessTimers::decl(&config),
    ];
    let mut text = HEADER.to_owned();
    for declaration in declarations {
        text.push_str("\nexport ");
        text.push_str(&declaration);
        text.push('\n');
    }
    text.push_str(&catalog());
    text.push_str(&wrappers());
    text.push_str(&modifier_choices());
    text.push_str(&named_only());
    text
}

fn number_or_null(value: Option<u8>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}

/// `KEYCODE_WRAPPERS`, one wrapper per line.
fn wrappers() -> String {
    let mut text = "\n/** The wrappers the remap panel builds two-part keycodes with. */\n\
                    export const KEYCODE_WRAPPERS: readonly KeycodeWrapperChoice[] = [\n"
        .to_owned();
    for wrapper in keycode_catalog::wrappers() {
        writeln!(
            text,
            "  {{ builder: {}, keycode: 0x{:04x}, before: {}, after: {}, modifiers: {}, layer: {} }},",
            serde_json::to_string(&wrapper.builder).unwrap(),
            wrapper.keycode,
            serde_json::to_string(&wrapper.before).unwrap(),
            serde_json::to_string(&wrapper.after).unwrap(),
            wrapper
                .modifiers
                .map_or_else(|| "null".to_owned(), |modifiers| format!("0x{modifiers:02x}")),
            number_or_null(wrapper.layer),
        )
        .unwrap();
    }
    text.push_str("];\n");
    text
}

/// `MODIFIER_CHOICES`, one modifier set per line.
fn modifier_choices() -> String {
    let mut text = "\n/** The modifier sets QMK encodes, left-hand ones first. */\n\
                    export const MODIFIER_CHOICES: readonly ModifierChoice[] = [\n"
        .to_owned();
    for choice in keycode_catalog::modifier_choices() {
        writeln!(
            text,
            "  {{ modifiers: 0x{:02x}, name: {}, ctrl: {}, shift: {}, alt: {}, gui: {}, rightHand: {} }},",
            choice.modifiers,
            serde_json::to_string(&choice.name).unwrap(),
            choice.ctrl,
            choice.shift,
            choice.alt,
            choice.gui,
            choice.right_hand,
        )
        .unwrap();
    }
    text.push_str("];\n");
    text
}

/// `NAMED_ONLY_KEYCODES`, one keycode per line.
fn named_only() -> String {
    let mut text =
        "\n/** Named keycodes the remap panel neither lists nor builds, to name a raw value. */\n\
                    export const NAMED_ONLY_KEYCODES: readonly KeycodeChoice[] = [\n"
            .to_owned();
    for choice in keycode_catalog::named_only() {
        writeln!(
            text,
            "  {{ keycode: 0x{:04x}, name: {}, layer: {} }},",
            choice.keycode,
            serde_json::to_string(&choice.name).unwrap(),
            number_or_null(choice.layer),
        )
        .unwrap();
    }
    text.push_str("];\n");
    text
}

/// `KEYCODE_CATEGORIES`, one keycode per line.
fn catalog() -> String {
    let mut text = "\n/** The keycodes the remap panel offers, by category. */\n\
                    export const KEYCODE_CATEGORIES: readonly KeycodeCategory[] = [\n"
        .to_owned();
    for category in keycode_catalog::categories() {
        writeln!(
            text,
            "  {{\n    name: {},\n    keycodes: [",
            serde_json::to_string(category.name).unwrap()
        )
        .unwrap();
        for choice in category.keycodes {
            let layer = choice
                .layer
                .map_or_else(|| "null".to_owned(), |layer| layer.to_string());
            writeln!(
                text,
                "      {{ keycode: 0x{:04x}, name: {}, layer: {layer} }},",
                choice.keycode,
                serde_json::to_string(&choice.name).unwrap()
            )
            .unwrap();
        }
        text.push_str("    ],\n  },\n");
    }
    text.push_str("];\n");
    text
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::bindings;

    #[test]
    fn the_committed_typescript_bindings_match_the_rust_types() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/bindings.ts");
        let generated = bindings();
        if std::env::var_os("OPENKEYCHRON_WRITE_BINDINGS").is_some() {
            std::fs::write(&path, &generated).unwrap();
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == generated,
            "{} is out of date; regenerate it with \
             `OPENKEYCHRON_WRITE_BINDINGS=1 cargo test -p openkeychron-app bindings`",
            path.display()
        );
    }
}
