use std::io::{Read, Write};
use std::sync::Arc;
use thiserror::Error;

use crate::ipc;
use crate::ipc::handshake;
use crate::proto::pty::user as p;
use crate::pty_master::PtyMaster;

#[derive(Error, Debug)]
pub(super) enum ServeUserError {
    #[error("error handshaking: {0}")]
    Handshake(#[source] handshake::Error),

    #[error("socket receive error: {0}")]
    Receive(#[source] ipc::ReceiveError),

    #[error("socket send error: {0}")]
    Send(#[source] ipc::SendError),

    #[error("error shutting down IPC socket: {0}")]
    SocketShutdown(#[from] ipc::ShutdownError),

    #[error("PTY I/O error: {0}")]
    PtyIo(#[source] std::io::Error),
}

pub(super) fn serve_user(
    pty: Arc<PtyMaster>,
    conn: impl ipc::IPC + Sync + Send + 'static,
) -> Result<(), ServeUserError> {
    handshake::handshake_as_server(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
        .map_err(ServeUserError::Handshake)?;

    let conn = Arc::new(conn);

    let input: std::thread::JoinHandle<Result<(), ServeUserError>> = std::thread::spawn({
        let pty = pty.clone();
        let conn = conn.clone();
        move || {
            loop {
                let message: p::Input = conn.receive_with_fds().map_err(ServeUserError::Receive)?;
                match &message {
                    p::Input::KeyboardInput(input) => {
                        // TODO this currently blocks further input processing.
                        // Backpressure is good, but we probably need to handle resizes and control-C even when the process in the session is not consuming standard input.
                        (&*pty).write_all(input).map_err(ServeUserError::PtyIo)?;
                    }
                };
            }
        }
    });

    let output: std::thread::JoinHandle<Result<(), ServeUserError>> = std::thread::spawn({
        let conn = conn.clone();
        move || loop {
            let mut buf = vec![0; 1024];
            let n = {
                let result = (&*pty).read(&mut buf);
                // Map EIO to a regular return.
                // All PTY interactions end at EIO, there's no EOF.
                if let Err(error) = &result {
                    if let Some(errno) = error.raw_os_error() {
                        if errno == libc::EIO {
                            return Ok(());
                        }
                    }
                }
                result.map_err(ServeUserError::PtyIo)?
            };
            buf.truncate(n);
            let message = p::Output::SessionOutput(buf);
            conn.send_with_fds(&message).map_err(ServeUserError::Send)?;
        }
    });

    match output.join() {
        Err(panicked) => std::panic::resume_unwind(panicked),
        Ok(result) => {
            println!("pty closed: {:?}", result);
            // Shutdown the IPC socket so the reading thread will exit.
            //
            // This will cause sending IPC clients to see EPIPE, but they'll just have to handle that.
            // The PTY is gone, there's nothing this service can do for them anymore.
            let how = std::net::Shutdown::Read;
            conn.shutdown(how).map_err(ServeUserError::SocketShutdown)?;
        }
    };
    match input.join() {
        Err(panicked) => std::panic::resume_unwind(panicked),
        Ok(result) => result?,
    };
    Ok(())
}
