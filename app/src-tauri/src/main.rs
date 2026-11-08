// Without this, a Windows release build also opens a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(test)]
mod bindings;
mod commands;
mod device;
mod error;
mod info;
#[cfg(test)]
mod keycode_catalog;
mod keymap;
mod keymap_edit;
mod macros;
mod settings;
mod source;
#[cfg(test)]
mod test_support;

use std::process::ExitCode;

use source::{DeviceSource, FIXTURE_VARIABLE};

fn main() -> ExitCode {
    let source = match DeviceSource::from_variable(std::env::var_os(FIXTURE_VARIABLE)) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("openkeychron: {error}");
            return ExitCode::FAILURE;
        }
    };
    let app = tauri::Builder::default()
        .manage(commands::DeviceAccess::default())
        .manage(source)
        .manage(keymap_edit::KeymapHistories::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            commands::read_info,
            commands::read_keymap,
            commands::read_macros,
            commands::read_settings,
            commands::set_keycode,
            commands::undo_keymap_change,
            commands::restore_keymap_backup,
        ])
        .run(tauri::generate_context!());
    match app {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("openkeychron: {error}");
            ExitCode::FAILURE
        }
    }
}
