fn main() {
    if let Err(error) = entrance_gui::run_desktop_bridge() {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}
