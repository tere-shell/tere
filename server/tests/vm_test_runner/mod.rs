use std::ffi::OsString;

pub fn build(name: &str) -> String {
    let cargo_bin = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut child = std::process::Command::new(cargo_bin)
        .args([
            "build",
            "--package=tere-server",
            "--features=internal-dangerous-tests",
            &["--test=", name].concat(),
            "--message-format=json",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("running cargo build");
    let stdout = child
        .stdout
        .take()
        .expect("internal error: cargo build stdout was not captured");
    let mut executable = None;
    let reader = std::io::BufReader::new(stdout);
    for result in cargo_metadata::Message::parse_stream(reader) {
        let message = result.expect("parsing cargo build output");
        match &message {
            cargo_metadata::Message::CompilerArtifact(artifact) => {
                if artifact.target.name != name {
                    continue;
                }
                if !artifact.target.kind.iter().any(|kind| kind == "test") {
                    continue;
                }
                if let Some(path) = &artifact.executable {
                    assert!(executable.is_none(), "found multiple matches for test");
                    executable.insert(path.to_path_buf().into_string());
                }
            }

            // Pass through compiler messages, to make debugging why the executable wasn't found nicer.
            // On any errors, you probably want to build things directly:
            //
            // ```
            // cargo build --tests --features internal-dangerous-tests
            // ```
            cargo_metadata::Message::CompilerMessage(message) => {
                if message.target.name != name {
                    continue;
                }
                if !message.target.kind.iter().any(|kind| kind == "test") {
                    continue;
                }
                println!("compiler error: {}", message.message);
            }
            _ => (),
        }
    }
    let exit = child.wait().expect("cargo build");
    if !exit.success() {
        panic!("cargo build failed: status {:?}", exit.code())
    }
    executable.expect("must find test executable in cargo build output")
}

pub fn run_vm_test(name: &str) {
    let executable = build(name);

    let exit = std::process::Command::new("nix-build")
        .args(&[
            "--arg",
            "vm-test-executable",
            &executable,
            "--",
            // The Nix files are at the root of the project, while we are building inside the `server/` subdirectory.
            &[env!["CARGO_MANIFEST_DIR"], "/../nix/vm-test.nix"].concat(),
        ])
        .stdin(std::process::Stdio::null())
        .status()
        .expect("running nix-build [...] nix/vm-test.nix");
    if !exit.success() {
        panic!("nix-build failed: status {:?}", exit.code())
    }
}
