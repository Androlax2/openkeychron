// Without this, a Windows release build also opens a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> Result<(), tauri::Error> {
    tauri::Builder::default().run(tauri::generate_context!())
}
