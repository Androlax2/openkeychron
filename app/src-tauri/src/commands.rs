//! The Tauri commands the UI calls. Each command but `list_devices` finds the device by the path
//! `list_devices` gave, opens it and hands it to a function. The `read_*` commands only read.
//! `set_keycode`, `set_encoder_keycode`, `undo_keymap_change`, `redo_keymap_change` and
//! `restore_keymap_backup` write the keymap, through the guarded write path in `keymap_edit`. The
//! keymap commands use the history of the device's connection, not of its path, so a keyboard
//! plugged in again on that path starts a new one. Devices come from the `DeviceSource`: the connected keyboards, or the
//! fixtures of mock mode.
//!
//! A new command goes in three places: `generate_handler!` in `main.rs`, `COMMANDS` in
//! `build.rs` (which generates its `allow-` permission) and `capabilities/default.json`.

use std::sync::{Mutex, MutexGuard, PoisonError};

use tauri::State;

use crate::device::Device;
use crate::error::CommandError;
use crate::info::{self, DeviceInfo};
use crate::keymap_edit::{self, EditableKeymap, EncoderAssignment, KeyAssignment, KeymapHistories};
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
    histories: State<'_, KeymapHistories>,
) -> Result<EditableKeymap, CommandError> {
    let _exclusive = access.exclusive();
    let (device, transport, connection) = source.open_connection(&path)?;
    keymap_edit::read(transport, &device, histories.lock().of(connection))
}

/// Writes. Sets one key and returns the keymap read again after the write.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "an async Tauri command takes its arguments by value"
)]
pub fn set_keycode(
    path: String,
    assignment: KeyAssignment,
    access: State<'_, DeviceAccess>,
    source: State<'_, DeviceSource>,
    histories: State<'_, KeymapHistories>,
) -> Result<EditableKeymap, CommandError> {
    let _exclusive = access.exclusive();
    let (device, transport, connection) = source.open_connection(&path)?;
    keymap_edit::set_keycode(
        transport,
        &device,
        histories.lock().of(connection),
        assignment,
    )
}

/// Writes. Sets what one turn of an encoder sends and returns the keymap read again after the
/// write.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "an async Tauri command takes its arguments by value"
)]
pub fn set_encoder_keycode(
    path: String,
    assignment: EncoderAssignment,
    access: State<'_, DeviceAccess>,
    source: State<'_, DeviceSource>,
    histories: State<'_, KeymapHistories>,
) -> Result<EditableKeymap, CommandError> {
    let _exclusive = access.exclusive();
    let (device, transport, connection) = source.open_connection(&path)?;
    keymap_edit::set_encoder_keycode(
        transport,
        &device,
        histories.lock().of(connection),
        assignment,
    )
}

/// Writes. Takes back the newest keymap change the app made on the device.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "an async Tauri command takes its arguments by value"
)]
pub fn undo_keymap_change(
    path: String,
    access: State<'_, DeviceAccess>,
    source: State<'_, DeviceSource>,
    histories: State<'_, KeymapHistories>,
) -> Result<EditableKeymap, CommandError> {
    let _exclusive = access.exclusive();
    let (device, transport, connection) = source.open_connection(&path)?;
    keymap_edit::undo(transport, &device, histories.lock().of(connection))
}

/// Writes. Puts back the keymap change the newest undo on the device took back.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "an async Tauri command takes its arguments by value"
)]
pub fn redo_keymap_change(
    path: String,
    access: State<'_, DeviceAccess>,
    source: State<'_, DeviceSource>,
    histories: State<'_, KeymapHistories>,
) -> Result<EditableKeymap, CommandError> {
    let _exclusive = access.exclusive();
    let (device, transport, connection) = source.open_connection(&path)?;
    keymap_edit::redo(transport, &device, histories.lock().of(connection))
}

/// Writes. Puts back the keymap from before the app's first change on the device.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "an async Tauri command takes its arguments by value"
)]
pub fn restore_keymap_backup(
    path: String,
    access: State<'_, DeviceAccess>,
    source: State<'_, DeviceSource>,
    histories: State<'_, KeymapHistories>,
) -> Result<EditableKeymap, CommandError> {
    let _exclusive = access.exclusive();
    let (device, transport, connection) = source.open_connection(&path)?;
    keymap_edit::restore_backup(transport, &device, histories.lock().of(connection))
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
    fn the_main_window_may_call_the_read_commands_and_the_keymap_writes_only() {
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
                "allow-set-keycode",
                "allow-set-encoder-keycode",
                "allow-undo-keymap-change",
                "allow-redo-keymap-change",
                "allow-restore-keymap-backup",
            ])
        );
    }
}
