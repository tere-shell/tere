#![cfg(feature = "internal-dangerous-tests")]

use std::collections::HashSet;
use std::convert::TryFrom;
use std::path::Path;

use tere_server::ipc;
use tere_server::ipc::seqpacket::SeqPacket;

mod systemd;

#[test]
fn pty_dynamic_user_isolation() {
    use tere_server::proto::pty as p;

    let dbus = systemd::Dbus::new().expect("dbus");

    let path = Path::new("/run/tere/socket/pty.socket");
    // Connect to pty service, twice.
    // Complete the handshake so we can be sure that the server process is actually running, and we're not still in socket activation startup.
    let _clients: Vec<SeqPacket> = (0..2)
        .map(|_| {
            let conn = SeqPacket::connect(path).expect("connect");
            ipc::handshake::handshake_as_client(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
                .expect("handshake");
            conn
        })
        .collect();

    let units = dbus
        .list_units_by_patterns(&["active"], &["tere-pty@*.service"])
        .expect("listing systemd units");
    println!("units={:#?}", units);
    if units.len() < 2 {
        panic!("expected two tere-pty@.service instances")
    }

    // Get MainPID for every matching unit, assuming it's a service.
    let pids: Vec<u32> = units
        .iter()
        .map(|unit| {
            let path = &unit.object_path.as_str();
            let service = dbus
                .get_systemd_service_for_path(path)
                .expect("getting D-Bus proxy for systemd service");
            service.main_pid().expect("getting systemd service MainPID")
        })
        // Filter out PID 0, which is Systemd's way of saying None.
        // We don't expect to see those in state active, but might race against the service dying.
        .filter(|pid| *pid != 0)
        .collect();
    println!("pids={:#?}", pids);
    if pids.len() < 2 {
        panic!("expected two tere-pty@.service instances")
    }

    // Get UIDs for those PIDs.
    let uids: Vec<u32> = pids
        .iter()
        .map(|pid| {
            // Systemd D-Bus API thinks PIDs are u32, the procfs crate thinks they're i32.
            let pid = i32::try_from(*pid).expect("PID must fit in i32 to please the procfs crate");
            let process = procfs::process::Process::new(pid)
                // maybe handle procfs::ProcError::NotFound by filtering out the PID?
                .expect("getting UID for PID");
            process.owner
        })
        .collect();
    println!("uids={:#?}", uids);
    if uids.len() < 2 {
        panic!("expected two tere-pty@.service instances")
    }

    let mut unique = HashSet::new();
    let duplicates: Vec<u32> = uids
        .iter()
        .filter(|uid| !unique.insert(*uid))
        .copied()
        .collect();
    println!("duplicates={:#?}", duplicates);
    assert!(duplicates.is_empty(), "duplicate UIDs");
}

#[test]
fn sessions_create() {
    use tere_server::ipc::IPC;
    let path = Path::new("/run/tere/socket/sessions.socket");
    let conn = SeqPacket::connect(path).expect("connect");

    let client_socket = {
        use tere_server::proto::sessions as p;
        ipc::handshake::handshake_as_client(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
            .expect("handshake");
        let (client_socket, server_socket) = ipc::seqpacket::pair().expect("socketpair");
        let message = p::Request::CreateShellSession(p::CreateShellSession {
            fd: server_socket,
            machine: p::Machine::Host,
            user: "testuser".to_string(),
            program: None,
            args: None,
            env: None,
        });
        conn.send_with_fds(&message).expect("send request");
        client_socket
    };

    // now pretend we're the client
    {
        let conn = SeqPacket::try_from(client_socket).expect("convert client socket to SeqPacket");
        use tere_server::proto::pty::user as p;
        ipc::handshake::handshake_as_client(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
            .expect("handshake");
        {
            let message = p::Input::KeyboardInput(b"printf '%sbar%s' 'foo' 'quux'\r\n".to_vec());
            conn.send_with_fds(&message).expect("send input");
        }
        let mut output: Vec<u8> = Vec::new();
        loop {
            // let result = conn.receive_with_fds::<p::Output>();
            let message: p::Output = match conn.receive_with_fds() {
                Ok(m) => m,
                Err(ipc::ReceiveError::End) => break,
                Err(error) => panic!("receive output: {:?}", error),
            };
            println!("received: {:?}", message);
            match message {
                p::Output::SessionOutput(b) => {
                    println!("output: {}", String::from_utf8_lossy(&b));
                    output.extend_from_slice(&b);
                }
            }
            // It seems control-D sent too early (before bash is reading?) is just simply ignored.
            // If that worked, we'd send one right after sending the input, before the loop.
            // For now, keep sending EOF whenever we receive something, until it is acted on.
            // This is horrible, and I would love to find documentation about the behavior.
            {
                let message = p::Input::KeyboardInput(b"\x04".to_vec());
                conn.send_with_fds(&message)
                    .or_else(|error| match error {
                        ipc::SendError::Socket(inner)
                            if inner.kind() == std::io::ErrorKind::BrokenPipe =>
                        {
                            Ok(())
                        }
                        _ => Err(error),
                    })
                    .expect("send EOF");
            }
        }
        assert!(String::from_utf8_lossy(&output).contains("foobarquux"));
    }
}
