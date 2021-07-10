//! Wrap a serde mechanism so that it can be used over UNIX domain sockets with ancillary data.
//! Adds specific support for FD passing.
//!
//! This module uses nightly-only experimental APIs.
//! It may need to be updated regularly to keep up.
//! This still seems a better journey than 3rd party crates at a time when std is adding native support.
//! <https://github.com/rust-lang/rust/issues/76915>
//!
//! Mark fields intended for FD passing with
//!
//! ```
//! # use std::fs::File;
//! # use serde::{Serialize,Deserialize};
//! # use tere_server::ipc;
//! #
//! #[derive(Debug, Serialize, Deserialize)]
//! struct MyMessage {
//!     #[serde(with = "ipc::passfd")]
//!     demo: File,
//! }
//! ```
//!
//! This module is currently tied to [bincode], but that's mostly for ease of implementation.
//! One hard limitation is that the [serde::ser::Serializer]/[serde::de::Deserializer] must not visit the same item multiple times.
//! [bincode] obeys this when configured correctly.
//!
//! We also assume serializing units (as in `()`) takes no space in the encoded message.
//! [bincode] obeys this.
//! Anything transported via FD passing is encoded as a unit.
//! The relevant APIs do not offer a "skip" mechanism at that level.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::os::unix::net::AncillaryError;
use thiserror::Error;

pub mod handshake;
pub mod ownedfd;
pub mod passfd;
pub mod seqpacket;

#[cfg(test)]
mod fakeipc;

/// Expose information about messages to the transport.
pub trait Message: Debug {
    /// Maximum size of the encoded message.
    /// Must not depend on message contents, this is the worst case.
    const MAX_SIZE: usize = 8192;
    /// Maximum number of FDs this kind of a message may contain.
    /// Must not depend on message contents, this is the worst case.
    const MAX_FDS: usize = 0;
}

#[derive(Error, Debug)]
pub enum SendError {
    #[error("serialize failed: {0}")]
    Serialize(#[source] bincode::Error),

    #[error("socket sendmsg failed: {0}")]
    Socket(#[source] std::io::Error),
}

#[derive(Error, Debug)]
pub enum ReceiveError {
    #[error("end of stream")]
    End,

    #[error("received more data than expected for this message")]
    TooLarge,

    #[error("deserialize failed: {0}")]
    Deserialize(#[source] bincode::Error),

    #[error("socket recvmsg failed: {0}")]
    Socket(#[source] std::io::Error),

    #[error("cannot parse socket ancillary data: {0:?}")]
    Ancillary(AncillaryError),

    #[error("ancillary data was truncated: reserved {bytes_cap} bytes for {max_fds} fds")]
    AncillaryTruncated { max_fds: usize, bytes_cap: usize },

    #[error("received too many FDs: got {orig}, {extra} too many")]
    TooManyFds { orig: usize, extra: usize },
}

#[derive(Error, Debug)]
pub enum ShutdownError {
    #[error("error shutting down IPC socket for {how:?}: {source}")]
    Io {
        how: std::net::Shutdown,
        source: std::io::Error,
    },
}

/// The IPC trait is a unit-testable abstraction.
pub trait IPC {
    /// Send a [Message] with the included file descriptors.
    fn send_with_fds<M>(&self, message: &M) -> Result<(), SendError>
    where
        M: 'static + Message + Serialize;

    /// Receive a [Message] and included file descriptors.
    fn receive_with_fds<M>(&self) -> Result<M, ReceiveError>
    where
        M: 'static + Message + DeserializeOwned;

    /// Shuts down the read, write, or both halves of this connection.
    fn shutdown(&self, how: std::net::Shutdown) -> Result<(), ShutdownError>;
}
