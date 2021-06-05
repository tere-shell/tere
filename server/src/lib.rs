// TODO once we have something complete and runnable, come back to this.
#![allow(dead_code)]

// RUST-WART All of our modules need to be public to get doctests to work right.
// They are *not* implied to be usable by others, and if rustdoc improves they will be made private again.
// Anything ready for external users will be split into different crates (or even repos).
//
// https://github.com/rust-lang/rust/issues/50784

pub mod dbus_shell;
