// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = entrance_lib::dispatch_cli_or_run() {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}
