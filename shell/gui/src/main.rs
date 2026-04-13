#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    entrance_gui::run_tauri_app();
}
