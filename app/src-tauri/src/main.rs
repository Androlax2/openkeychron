// Without this, a Windows release build also opens a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(test)]
mod bindings;
mod commands;
mod device;
mod error;
mod info;
mod keymap;
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
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            commands::read_info,
            commands::read_keymap,
            commands::read_macros,
            commands::read_settings,
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
