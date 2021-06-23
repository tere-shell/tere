use serde::{Deserialize, Serialize};

use crate::ipc;

pub const CLIENT_INTENT: &str = "tere 2021-06-22T12:12:30 pty_user client";
pub const SERVER_INTENT: &str = "tere 2021-06-22T12:12:51 pty_user server";

#[derive(Debug, Serialize, Deserialize)]
pub enum Output {
    SessionOutput(Vec<u8>),
}

impl ipc::Message for Output {}

#[derive(Debug, Serialize, Deserialize)]
pub enum Input {
    KeyboardInput(Vec<u8>),
    // TODO
    // PasteInput(Vec<u8>),
    // Resize{
    //     Rows: u16,
    //     Columns: u16,
    // },
}

impl ipc::Message for Input {}
