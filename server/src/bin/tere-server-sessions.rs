fn main() {
    match tere_server::services::sessions::run() {
        Ok(()) => {}
        Err(error) => {
            eprintln!("{}", error);
            std::process::exit(1);
        }
    };
}
