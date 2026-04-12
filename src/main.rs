fn main() {
    if let Err(err) = timelocked::userinterfaces::cli::run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
