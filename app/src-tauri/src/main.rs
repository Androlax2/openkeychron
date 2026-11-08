// Without this, a Windows release build also opens a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod device;
mod error;
mod info;
mod keymap;
mod macros;
mod settings;
#[cfg(test)]
mod test_support;

fn main() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .manage(commands::DeviceAccess::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            commands::read_info,
            commands::read_keymap,
            commands::read_macros,
            commands::read_settings,
        ])
        .run(tauri::generate_context!())
}
