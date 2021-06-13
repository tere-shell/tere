use serde::{Deserialize, Serialize};
use std::os::unix::net::UnixDatagram;

use crate::ipc;
use crate::ipc::ownedfd::OwnedFd;

pub const CLIENT_INTENT: &str = "tere 2021-06-11T21:34:03 pty client";
pub const SERVER_INTENT: &str = "tere 2021-06-11T21:35:37 pty server";

#[derive(Debug, Serialize, Deserialize)]
pub struct Init {
    // Always need to transport some data, to make FD passing work.
    //
    // TODO make this the ipc module's concern.
    pub _dummy: u8,

    #[serde(with = "ipc::passfd")]
    pub pty_fd: OwnedFd,
}

impl ipc::Message for Init {
    const MAX_SIZE: usize = 1;
    const MAX_FDS: usize = 1;
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    NewClient {
        // TODO make this the ipc module's concern.
        _dummy: u8,

        /// A `SOCK_SEQPACKET` socket (not `SOCK_DATAGRAM`), regardless of our best option of how to represent it in Rust.
        #[serde(with = "ipc::passfd")]
        fd: UnixDatagram,
    },
}

impl ipc::Message for Request {
    const MAX_FDS: usize = 1;
}
