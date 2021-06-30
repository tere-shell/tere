use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;

fn hash_protocol_identity() -> String {
    let walk = ignore::WalkBuilder::new("src/ipc")
        .add("src/proto")
        // TODO If we had well-defined state machines for our protocols, we wouldn't need to include services in the hash.
        .add("src/services")
        // Deterministic output.
        .sort_by_file_name(|a, b| a.cmp(b))
        .follow_links(false)
        // Do *not* ignore hidden files.
        .hidden(false)
        // Don't respect non-typical `.ignore` files; Git wouldn't.
        .ignore(false)
        .same_file_system(true)
        .build();
    let mut hasher = blake3::Hasher::new();
    for result in walk {
        let entry = result.expect("reading source directory");
        match std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOATIME | libc::O_NOCTTY | libc::O_NOFOLLOW)
            .open(entry.path())
        {
            Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
                // It's a symlink.
                let path = entry.into_path();
                hasher.update(path.as_os_str().as_bytes());
                hasher.update(&[0]);
                let dst = path.read_link().expect("readlink");
                hasher.update(b"L");
                hasher.update(dst.as_os_str().as_bytes());
                hasher.update(&[0]);
            }
            Err(error) => {
                panic!("error reading directory: {:?}", error);
            }
            Ok(mut file) => {
                let mut file_hash = blake3::Hasher::new();
                match std::io::copy(&mut file, &mut file_hash) {
                    Err(error) if error.raw_os_error() == Some(libc::EISDIR) => {
                        // It's a directory; do nothing.
                    }
                    Err(error) => {
                        panic!("error reading file: {:?}", error);
                    }
                    Ok(_) => {
                        let path = entry.into_path();
                        hasher.update(path.as_os_str().as_bytes());
                        hasher.update(&[0]);
                        let hash = file_hash.finalize();
                        hasher.update(b"F");
                        hasher.update(hash.as_bytes());
                        hasher.update(&[0]);
                    }
                };
            }
        }
    }
    hasher.finalize().to_hex().to_string()
}

fn main() {
    // We want a *deterministic* build id, to be used for compatibility check of the unversioned IPC protocol.
    // We want a build ID for the *library* not the binary, so separately-compiled integration tests can talk to instances of the `tere-server` binary.
    // We only care about the parts that actually define the IPC behavior, this way e.g. integration tests can be edited and re-run multiple times without identity mismatch.
    //
    // For troubleshooting, you can see this output in `cargo build -vv`
    let protocol_identity = hash_protocol_identity();
    println!(
        "cargo:rustc-env=TERE_PROTOCOL_IDENTITY={}",
        protocol_identity
    );
}
