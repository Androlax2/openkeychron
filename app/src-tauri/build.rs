/// The app's own commands get `allow-<command>` permissions, which `capabilities/` grants; a
/// command missing here cannot be called from the UI.
const COMMANDS: &[&str] = &[
    "list_devices",
    "read_info",
    "read_keymap",
    "read_macros",
    "read_settings",
    "set_keycode",
    "undo_keymap_change",
    "redo_keymap_change",
    "restore_keymap_backup",
];

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))?;
    Ok(())
}
