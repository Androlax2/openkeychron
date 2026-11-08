//! The Tauri commands the UI calls. Each `read_*` command finds the device by the path
//! `list_devices` gave, opens it and hands it to a read function. None of them writes to the
//! keyboard. Devices come from the `DeviceSource`: the connected keyboards, or the fixtures of
//! mock mode.
//!
//! A new command goes in three places: `generate_handler!` in `main.rs`, `COMMANDS` in
//! `build.rs` (which generates its `allow-` permission) and `capabilities/default.json`.

use std::sync::{Mutex, MutexGuard, PoisonError};

use tauri::State;

use crate::device::Device;
use crate::error::CommandError;
use crate::info::{self, DeviceInfo};
use crate::keymap::{self, Keymap};
use crate::macros::{self, MacroList};
use crate::settings::{self, Settings};
use crate::source::DeviceSource;

/// Lets one command talk to the keyboards at a time: two handles open on one device could
/// each read the other's replies.
#[derive(Debug, Default)]
pub struct DeviceAccess(Mutex<()>);

impl DeviceAccess {
    fn exclusive(&self) -> MutexGuard<'_, ()> {
        // The mutex guards no data, so a command that panicked while holding it left nothing
        // half-changed behind.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "an async Tauri command takes its arguments by value"
)]
pub fn list_devices(source: State<'_, DeviceSource>) -> Result<Vec<Device>, CommandError> {
    Ok(source.list()?.into_iter().map(Device::from).collect())
}

#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "an async Tauri command takes its arguments by value"
)]
pub fn read_info(
    path: String,
    access: State<'_, DeviceAccess>,
    source: State<'_, DeviceSource>,
) -> Result<DeviceInfo, CommandError> {
    let _exclusive = access.exclusive();
    let (device, transport) = source.open(&path)?;
    info::read(transport, &device)
}

#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "an async Tauri command takes its arguments by value"
)]
pub fn read_keymap(
    path: String,
    access: State<'_, DeviceAccess>,
    source: State<'_, DeviceSource>,
) -> Result<Keymap, CommandError> {
    let _exclusive = access.exclusive();
    let (device, transport) = source.open(&path)?;
    keymap::read(transport, &device)
}

#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "an async Tauri command takes its arguments by value"
)]
pub fn read_macros(
    path: String,
    access: State<'_, DeviceAccess>,
    source: State<'_, DeviceSource>,
) -> Result<MacroList, CommandError> {
    let _exclusive = access.exclusive();
    let (device, transport) = source.open(&path)?;
    macros::read(transport, &device)
}

#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "an async Tauri command takes its arguments by value"
)]
pub fn read_settings(
    path: String,
    access: State<'_, DeviceAccess>,
    source: State<'_, DeviceSource>,
) -> Result<Settings, CommandError> {
    let _exclusive = access.exclusive();
    let (_, transport) = source.open(&path)?;
    settings::read(transport)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_main_window_may_call_only_the_read_commands() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        assert_eq!(
            capability["permissions"],
            serde_json::json!([
                "allow-list-devices",
                "allow-read-info",
                "allow-read-keymap",
                "allow-read-macros",
                "allow-read-settings",
            ])
        );
    }
}
