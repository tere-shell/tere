//! Handshake performed between an IPC client and server to ensure there is no version mismatch.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ipc;

fn identify(context: &'static str) -> blake3::Hash {
    let mut out = [0u8; 32];
    blake3::derive_key(context, env!("TERE_PROTOCOL_IDENTITY").as_bytes(), &mut out);
    blake3::Hash::from(out)
}

#[derive(Debug, Serialize, Deserialize)]
struct Handshake {
    intent: String,
    // TODO make my own type that combines blake3::Hash and Serialize/Deserialize with nicer output
    build_id: [u8; 32],
}

impl Handshake {
    fn new(intent: &'static str) -> Self {
        Self {
            intent: intent.to_string(),
            build_id: *identify(intent).as_bytes(),
        }
    }
}

impl ipc::Message for Handshake {}

#[derive(Error, Debug)]
pub enum Error {
    #[error("socket send error: {0}")]
    Send(#[source] ipc::SendError),

    #[error("socket receive error: {0}")]
    Receive(#[source] ipc::ReceiveError),

    #[error("peer is running the wrong version of this software")]
    WrongVersion,

    #[error("peer is trying to talk to some other service")]
    WrongService,
}

pub fn handshake_as_client(
    conn: &impl ipc::IPC,
    client_intent: &'static str,
    server_intent: &'static str,
) -> Result<(), Error> {
    conn.send_with_fds(&Handshake::new(client_intent))
        .map_err(Error::Send)?;
    let msg: Handshake = conn.receive_with_fds().map_err(Error::Receive)?;
    let server_build_id = identify(server_intent);
    if server_build_id != msg.build_id {
        return Err(Error::WrongVersion);
    }
    if msg.intent != server_intent {
        return Err(Error::WrongService);
    }
    Ok(())
}

pub fn handshake_as_server(
    conn: &impl ipc::IPC,
    client_intent: &'static str,
    server_intent: &'static str,
) -> Result<(), Error> {
    let msg: Handshake = conn.receive_with_fds().map_err(Error::Receive)?;
    let client_build_id = identify(client_intent);
    if client_build_id != msg.build_id {
        return Err(Error::WrongVersion);
    }
    if msg.intent != client_intent {
        return Err(Error::WrongService);
    }
    conn.send_with_fds(&Handshake::new(server_intent))
        .map_err(Error::Send)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::ipc::fakeipc::FakeIpc;

    use super::*;

    #[test]
    fn client_simple() {
        let conn = FakeIpc::new();
        {
            let c2 = conn.clone();
            conn.expect(move |a| {
                let message: &Handshake = a.downcast_ref().expect("Message must be a Handshake");
                assert_eq!(message.intent, "tere 2021-06-10T13:38:10 testing client");
                assert_eq!(
                    blake3::Hash::from(message.build_id),
                    identify("tere 2021-06-10T13:38:10 testing client")
                );
                c2.add(Handshake::new("tere 2021-06-10T13:38:43 testing server"));
            });
        }

        handshake_as_client(
            &conn,
            "tere 2021-06-10T13:38:10 testing client",
            "tere 2021-06-10T13:38:43 testing server",
        )
        .expect("handshake_as_client");
    }

    #[test]
    fn client_simple_disconnected() {
        let conn = FakeIpc::new();
        conn.shutdown();

        let error = handshake_as_client(
            &conn,
            "tere 2021-06-10T13:38:10 testing client",
            "tere 2021-06-10T13:38:43 testing server",
        )
        .expect_err("handshake should have failed in this test");
        match error {
            Error::Receive(ipc::ReceiveError::Socket(socket_error)) => {
                assert_eq!(
                    socket_error.kind(),
                    std::io::ErrorKind::UnexpectedEof,
                    "wrong error kind: {:?}",
                    socket_error.kind(),
                );
            }
            _ => panic!("wrong error: {:?}", error),
        }
    }

    #[test]
    fn server_simple() {
        let conn = FakeIpc::new();
        conn.add(Handshake::new("tere 2021-06-10T13:38:10 testing client"));

        handshake_as_server(
            &conn,
            "tere 2021-06-10T13:38:10 testing client",
            "tere 2021-06-10T13:38:43 testing server",
        )
        .expect("handshake_as_client");
    }
}
