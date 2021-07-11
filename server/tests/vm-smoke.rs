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
        .map(|uid| *uid)
        .collect();
    println!("duplicates={:#?}", duplicates);
    assert!(duplicates.len() == 0, "duplicate UIDs");
}
