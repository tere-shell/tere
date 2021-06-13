use thiserror::Error;

use crate::{ipc, ipc::handshake, proto::pty as p};

#[derive(Error, Debug)]
pub enum Error {
    #[error("error handshaking: {0}")]
    Handshake(#[source] handshake::Error),

    #[error("socket receive error: {0}")]
    Receive(#[source] ipc::ReceiveError),
}

pub async fn run(conn: impl ipc::IPC) -> Result<(), Error> {
    handshake::handshake_as_server(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
        .await
        .map_err(Error::Handshake)?;

    let _pty_fd = {
        let msg: p::Init = conn.receive_with_fds().await.map_err(Error::Receive)?;
        msg.pty_fd
    };
    // TODO broadcast mechanism between PTY and multiple clients

    loop {
        let msg: p::Request = conn.receive_with_fds().await.map_err(Error::Receive)?;
        match msg {
            p::Request::NewClient { _dummy: _, fd: _fd } => {
                println!("new client");
                todo!();
            }
        }
    }
}

#[cfg(test)]
mod tests;
