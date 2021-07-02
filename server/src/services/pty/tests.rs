use std::convert::TryFrom;
use std::ffi::OsStr;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::net::UnixStream;
use thiserror::Error;

use crate::ipc;
use crate::ipc::seqpacket::SeqPacket;
use crate::ipc::IPC;
use crate::proto::pty as p;
use crate::pty_master::PtyMaster;

// RUST-WART https://github.com/rust-lang/rust/issues/29036
// use super as pty;
use super::super::pty;

#[test]
fn init_then_eof() {
    let (client_socket, server_socket) = ipc::seqpacket::pair().expect("socketpair");
    // TODO we're gonna need an actual PTY once more code is written
    let (_fake_pty, fake_pty_master) = UnixStream::pair().expect("socketpair for fake_pty");
    let fake_pty_master = {
        let fd = fake_pty_master.into_raw_fd();
        unsafe { PtyMaster::from_raw_fd(fd) }
    };
    let server_task = std::thread::spawn(|| {
        let conn = SeqPacket::try_from(server_socket).unwrap();
        pty::serve(conn)
    });
    let client_task = std::thread::spawn(|| {
        let conn = SeqPacket::try_from(client_socket).unwrap();
        ipc::handshake::handshake_as_client(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
            .expect("handshake as client");
        {
            let msg = p::Init {
                _dummy: 0,
                pty_fd: fake_pty_master,
            };
            conn.send_with_fds(&msg).expect("send Init");
        }
    });
    client_task.join().unwrap();
    let result = server_task.join().unwrap();
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

#[derive(Error, Debug)]
enum MakePtyError {
    #[error("posix_openpt: {0}")]
    Open(#[source] std::io::Error),

    #[error("grantpt: {0}")]
    Grant(#[source] std::io::Error),

    #[error("unlockpt: {0}")]
    Unlock(#[source] std::io::Error),

    #[error("ptsname: {0}")]
    PtsName(#[source] std::io::Error),

    #[error("cannot open child PTY: {0}")]
    OpenChildPty(#[source] std::io::Error),
}

fn path_from_nul_terminated_buf(buf: &[u8]) -> Option<&std::path::Path> {
    // std::ffi::CStr and friends all assume the buffer contains no interior NULs, can't use them here.
    let end = buf.iter().position(|c| *c == 0u8)?;
    let s = OsStr::from_bytes(&buf[..end]);
    Some(std::path::Path::new(s))
}

fn make_pty() -> Result<(PtyMaster, std::fs::File), MakePtyError> {
    let ret = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
    if ret < 0 {
        return Err(MakePtyError::Open(std::io::Error::last_os_error()));
    }
    let pty_master = unsafe { PtyMaster::from_raw_fd(ret) };
    let ret = unsafe { libc::grantpt(pty_master.as_raw_fd()) };
    if ret < 0 {
        return Err(MakePtyError::Grant(std::io::Error::last_os_error()));
    }
    let ret = unsafe { libc::unlockpt(pty_master.as_raw_fd()) };
    if ret < 0 {
        return Err(MakePtyError::Unlock(std::io::Error::last_os_error()));
    }

    let mut out = [0u8; 30];
    let path = {
        let c_buf = out.as_mut_ptr() as *mut libc::c_char;
        let c_len = out.len() as libc::size_t;
        let ret = unsafe { libc::ptsname_r(pty_master.as_raw_fd(), c_buf, c_len) };
        if ret < 0 {
            return Err(MakePtyError::PtsName(std::io::Error::last_os_error()));
        }
        path_from_nul_terminated_buf(&out).expect("ptsname returned a string without NUL")
    };
    dbg!(&path);

    let pty_child = std::fs::OpenOptions::new()
        .custom_flags(libc::O_NOCTTY | libc::O_CLOEXEC)
        .read(true)
        .write(true)
        .open(path)
        .map_err(MakePtyError::OpenChildPty)?;
    Ok((pty_master, pty_child))
}

#[test]
fn pty_io() {
    let (client_socket, server_socket) = ipc::seqpacket::pair().expect("socketpair");
    let (pty_master, mut pty_child) = make_pty().expect("make_pty");
    let (user_client_socket, user_server_socket) = ipc::seqpacket::pair().expect("socketpair");

    let server_task = std::thread::spawn(|| {
        let conn = SeqPacket::try_from(server_socket).unwrap();
        pty::serve(conn)
    });
    let client_task = std::thread::spawn(move || {
        let conn = SeqPacket::try_from(client_socket).unwrap();
        ipc::handshake::handshake_as_client(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
            .expect("handshake as client");
        {
            let msg = p::Init {
                _dummy: 0,
                pty_fd: pty_master,
            };
            conn.send_with_fds(&msg).expect("send Init");
        }
        {
            let msg = p::Request::NewClient {
                _dummy: 0,
                fd: user_server_socket,
            };
            conn.send_with_fds(&msg).expect("send Request");
        }

        // Now acting as pty_user client
        let user_conn = SeqPacket::try_from(user_client_socket).unwrap();
        ipc::handshake::handshake_as_client(
            &user_conn,
            p::user::CLIENT_INTENT,
            p::user::SERVER_INTENT,
        )
        .expect("handshake as pty_user client");
        const GREETING: &[u8] = b"hello, world\n";
        {
            let msg = p::user::Input::KeyboardInput(Vec::from(GREETING));
            user_conn.send_with_fds(&msg).expect("send KeyboardInput");
        }

        // Read our input from the PTY child.
        {
            let mut buf = [0u8; GREETING.len()];
            pty_child.read_exact(&mut buf).expect("PTY child read");
            assert_eq!(&buf, GREETING);
        }
    });
    client_task.join().unwrap();
    let result = server_task.join().unwrap();
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
