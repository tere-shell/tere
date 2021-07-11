use serde::{Deserialize, Serialize};
use std::os::unix::net::UnixDatagram;

use crate::ipc;

pub const CLIENT_INTENT: &str = "tere 2021-07-01T19:41:51 sessions client";
pub const SERVER_INTENT: &str = "tere 2021-07-01T19:42:20 sessions server";

#[derive(Debug, Serialize, Deserialize)]
pub enum Machine {
    Host,
    /// Name of container to connect to.
    //
    // TODO Ensure this does not start with ".", to force use of Machine::Host.
    Container(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateShellSession {
    /// Client for this session.
    #[serde(with = "ipc::passfd")]
    pub fd: UnixDatagram,
    pub machine: Machine,
    /// Username to start the session as.
    pub user: String,
    /// Absolute path to shell to run.
    pub program: Option<String>,
    /// Arguments to the shell.
    /// First argument should be the name of the program.
    pub args: Option<Vec<String>>,
    /// Environment variables to pass.
    pub env: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    CreateShellSession(CreateShellSession),
}

impl ipc::Message for Request {
    const MAX_FDS: usize = 1;
}
