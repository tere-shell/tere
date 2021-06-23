use std::convert::TryFrom;
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::os::unix::net::UnixStream;

use crate::ipc;
use crate::ipc::seqpacket::SeqPacket;
use crate::ipc::IPC;
use crate::proto::pty as p;
use crate::pty_master::PtyMaster;

// RUST-WART https://github.com/rust-lang/rust/issues/29036
// use super as pty;
use super::super::pty;

#[smol_potat::test]
async fn init_then_eof() {
    let (client_socket, server_socket) = ipc::seqpacket::pair().expect("socketpair");
    // TODO we're gonna need an actual PTY once more code is written
    let (_fake_pty, fake_pty_master) = UnixStream::pair().expect("socketpair for fake_pty");
    let fake_pty_master = {
        let fd = fake_pty_master.into_raw_fd();
        unsafe { PtyMaster::from_raw_fd(fd) }
    };
    let server_task = smol::spawn(async {
        let conn = SeqPacket::try_from(server_socket).unwrap();
        pty::run(conn).await
    });
    let client_task = smol::spawn(async {
        let conn = SeqPacket::try_from(client_socket).unwrap();
        ipc::handshake::handshake_as_client(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
            .await
            .expect("handshake as client");
        {
            let msg = p::Init {
                _dummy: 0,
                pty_fd: fake_pty_master,
            };
            conn.send_with_fds(&msg).await.expect("send Init");
        }
    });
    client_task.await;
    let result = server_task.await;
    match result {
        Err(pty::Error::Receive(ipc::ReceiveError::Deserialize(error))) => match *error {
            bincode::ErrorKind::Io(error) => {
                assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
            }
            _ => panic!("expected eof, got inner error {:?}", error),
        },
        _ => {
            panic!("expected eof, got {:?}", result);
        }
    };
}
