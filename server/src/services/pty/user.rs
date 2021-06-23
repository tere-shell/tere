use async_io::Async;
use smol::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::Arc;
use thiserror::Error;

use crate::ipc;
use crate::ipc::handshake;
use crate::proto::pty as p;
use crate::pty_master::PtyMaster;

#[derive(Error, Debug)]
pub(super) enum ServeUserError {
    #[error("error handshaking: {0}")]
    Handshake(#[source] handshake::Error),

    #[error("socket receive error: {0}")]
    Receive(#[source] ipc::ReceiveError),

    #[error("socket send error: {0}")]
    Send(#[source] ipc::SendError),

    #[error("PTY I/O error: {0}")]
    PtyIo(#[source] std::io::Error),
}

pub(super) async fn serve_user(
    pty: async_dup::Arc<Async<PtyMaster>>,
    conn: impl ipc::IPC + Sync + Send + 'static,
) -> Result<(), ServeUserError> {
    handshake::handshake_as_server(&conn, p::user::CLIENT_INTENT, p::user::SERVER_INTENT)
        .await
        .map_err(ServeUserError::Handshake)?;

    let conn = Arc::new(conn);

    let input = {
        let mut pty = pty.clone();
        let conn = conn.clone();
        let task: smol::Task<Result<(), ServeUserError>> = smol::spawn(async move {
            loop {
                let message: p::user::Input = conn
                    .receive_with_fds()
                    .await
                    .map_err(ServeUserError::Receive)?;
                match &message {
                    p::user::Input::KeyboardInput(input) => {
                        // TODO this currently blocks further input processing.
                        // Backpressure is good, but we probably need to handle resizes and control-C even when the process in the session is not consuming standard input.
                        pty.write_all(input).await.map_err(ServeUserError::PtyIo)?;
                    }
                };
            }
        });
        task
    };

    let output = {
        let mut pty = pty.clone();
        let task: smol::Task<Result<(), ServeUserError>> = smol::spawn(async move {
            loop {
                let mut buf = vec![0; 1024];
                let n = pty.read(&mut buf).await.map_err(ServeUserError::PtyIo)?;
                buf.truncate(n);
                let message = p::user::Output::SessionOutput(buf);
                conn.send_with_fds(&message)
                    .await
                    .map_err(ServeUserError::Send)?;
            }
        });
        task
    };

    // TODO when one fails, cancel the other -- but try_zip takes ownership away from me, so I can't cancel them after this.
    // (Cancel not just drop, so we know it's gone.)
    let (_, _) = smol::future::try_zip(input, output).await?;
    Ok(())
}
