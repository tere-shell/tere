use async_io::Async;
use std::convert::TryFrom;
use std::os::unix::io::FromRawFd;
use std::os::unix::net::UnixDatagram;
use structopt::StructOpt;
use thiserror::Error;

use crate::ipc;
use crate::ipc::handshake;
use crate::ipc::seqpacket;
use crate::ipc::seqpacket::SeqPacket;
use crate::proto::pty as p;

mod user;

/// Run the `pty@` service
#[derive(StructOpt, Debug)]
pub struct Command {}

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("cannot use stdin as socket: {0}")]
    StdinAsSocket(seqpacket::SocketConversionError),

    #[error(transparent)]
    Run(Error),
}

impl Command {
    pub async fn run(&self, _global: &crate::app::GlobalFlags) -> Result<(), CommandError> {
        let socket = unsafe { UnixDatagram::from_raw_fd(0) };
        let conn = SeqPacket::try_from(socket).map_err(CommandError::StdinAsSocket)?;
        serve(conn).await.map_err(CommandError::Run)?;
        Ok(())
    }
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

pub async fn serve(conn: impl ipc::IPC) -> Result<(), Error> {
    handshake::handshake_as_server(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
        .await
        .map_err(Error::Handshake)?;

    let pty = {
        let msg: p::Init = conn.receive_with_fds().await.map_err(Error::Receive)?;
        msg.pty_fd
    };
    let pty = Async::new(pty).map_err(Error::NonBlockingPty)?;
    // Kludging this via async_dup because we need to pass it to multiple tasks.
    // Proper broadcast mechanism, coming later, will deal with this better.
    let pty = async_dup::Arc::new(pty);

    // TODO broadcast mechanism between PTY and multiple clients.
    // for now, we just kick out the previous client when a new one shows up.
    let mut client: Option<smol::Task<Result<(), self::user::ServeUserError>>> = None;
    loop {
        let msg: p::Request = conn.receive_with_fds().await.map_err(Error::Receive)?;
        match msg {
            p::Request::NewClient { _dummy: _, fd } => {
                let conn = SeqPacket::try_from(fd).unwrap();
                let pty = pty.clone();
                let new_client =
                    smol::spawn(async move { self::user::serve_user(pty, conn).await });
                if let Some(old_client) = client.replace(new_client) {
                    old_client.cancel().await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
