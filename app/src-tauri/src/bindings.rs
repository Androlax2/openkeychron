//! Keeps `app/src/lib/bindings.ts`, the TypeScript types of what the commands return, equal to
//! the Rust types. A change to a command's types fails this test until the file is regenerated:
//!
//! ```sh
//! OPENKEYCHRON_WRITE_BINDINGS=1 cargo test -p openkeychron-app bindings
//! ```

use ts_rs::{Config, TS};

use crate::device::Device;
use crate::info::{DeviceInfo, KeychronFirmware};
use crate::keymap::{Bounds, Key, Keymap, Layer};
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
