fn main() {
    match tere_server::services::pty::run() {
        Ok(()) => {}
        Err(error) => {
            eprintln!("{}", error);
            std::process::exit(1);
        }
    };
}
