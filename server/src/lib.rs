// TODO once we have something complete and runnable, come back to this.
#![allow(dead_code)]
// Using unstable feature `unix_socket_ancillary_data`.
// https://github.com/rust-lang/rust/issues/76915
#![feature(unix_socket_ancillary_data)]
// Using unstable feature `once_cell` for `std::lazy::Lazy`.
// https://github.com/rust-lang/rust/issues/74465
#![feature(once_cell)]

// RUST-WART All of our modules need to be public to get doctests to work right.
// They are *not* implied to be usable by others, and if rustdoc improves they will be made private again.
// Anything ready for external users will be split into different crates (or even repos).
//
// https://github.com/rust-lang/rust/issues/50784

pub mod app;
pub mod dbus_shell;
pub mod ipc;
pub mod proto;
pub mod pty_master;
pub mod services;
pub mod socket_activation;
