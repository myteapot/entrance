fn main() {
    if let Err(error) = entrance_cli::dispatch_cli() {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}
