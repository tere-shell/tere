use std::convert::TryFrom;
use std::os::unix::io::FromRawFd;
use std::os::unix::net::UnixDatagram;
use std::sync::Arc;
use thiserror::Error;

use crate::ipc;
use crate::ipc::handshake;
use crate::ipc::seqpacket;
use crate::ipc::seqpacket::SeqPacket;
use crate::proto::pty as p;

mod user;

#[derive(Error, Debug)]
pub enum RunError {
    #[error("cannot use stdin as socket: {0}")]
    StdinAsSocket(seqpacket::SocketConversionError),

    #[error(transparent)]
    Run(Error),
}

pub fn run() -> Result<(), RunError> {
    let socket = unsafe { UnixDatagram::from_raw_fd(0) };
    let conn = SeqPacket::try_from(socket).map_err(RunError::StdinAsSocket)?;
    serve(conn).map_err(RunError::Run)?;
    Ok(())
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("error handshaking: {0}")]
    Handshake(#[source] handshake::Error),

    #[error("socket receive error: {0}")]
    Receive(#[source] ipc::ReceiveError),

    #[error("error making PTY master non-blocking: {0}")]
    NonBlockingPty(#[source] std::io::Error),
}

pub fn serve(conn: impl ipc::IPC) -> Result<(), Error> {
    handshake::handshake_as_server(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
        .map_err(Error::Handshake)?;

    let pty = {
        let msg: p::Init = conn.receive_with_fds().map_err(Error::Receive)?;
        msg.pty_master
    };
    // Kludging this via Arc because we need to pass it to multiple tasks.
    // Proper broadcast mechanism, coming later, will deal with this better.
    let pty = Arc::new(pty);

    // TODO broadcast mechanism between PTY and multiple clients.
    // For now, we absolutely mishandle multiple clients, smearing data over them unpredictably.
    loop {
        let msg: p::Request = conn.receive_with_fds().map_err(Error::Receive)?;
        match msg {
            p::Request::NewClient { _dummy: _, fd } => {
                let conn = SeqPacket::try_from(fd).unwrap();
                std::thread::spawn({
                    let pty = pty.clone();
                    move || self::user::serve_user(pty, conn)
                });
            }
        }
    }
}

#[cfg(test)]
mod tests;
