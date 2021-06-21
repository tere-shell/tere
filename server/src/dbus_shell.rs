//! Open new shell sessions via [`systemd-machined`](https://www.freedesktop.org/software/systemd/man/systemd-machined.service.html).
//!
//! The primary entry point is [Dbus::new].

// TODO switch to async API when zbus v2 is released

// TODO wait on signal MachineRemoved: https://github.com/systemd/systemd/blob/ed056c560b47f84a0aa0289151f4ec91f786d24a/src/machine/machinectl.c#L1403-L1408

use std::os::unix::io::{AsRawFd, FromRawFd};
use thiserror::Error;

mod proxies {
    // zbus makes these public, avoid exposing them
    // https://gitlab.freedesktop.org/dbus/zbus/-/issues/129

    use zbus::dbus_proxy;

    #[dbus_proxy(
        interface = "org.freedesktop.machine1.Manager",
        default_service = "org.freedesktop.machine1",
        default_path = "/org/freedesktop/machine1"
    )]
    trait MachineManager {
        fn open_machine_shell(
            &self,
            name: &str,
            user: &str,
            path: &str,
            args: &[&str],
            environment: &[&str],
        ) -> zbus::Result<(zvariant::Fd, String)>;
    }
}

/// Client to the [`systemd-machined`](https://www.freedesktop.org/software/systemd/man/systemd-machined.service.html) D-Bus API.
pub struct Dbus<'a> {
    proxy: proxies::MachineManagerProxy<'a>,
}

#[derive(Error, Debug)]
pub enum ConnectError {
    #[error("cannot connect to D-Bus: {0}")]
    Connect(zbus::Error),
}

impl Dbus<'_> {
    /// Open a new D-Bus client.
    pub fn new() -> Result<Self, ConnectError> {
        let connection = zbus::Connection::new_system().map_err(ConnectError::Connect)?;
        let proxy = proxies::MachineManagerProxy::new(&connection)
            // zbus v1.9.1 Proxy::new never fails, but still returns a Result.
            .map_err(ConnectError::Connect)?;
        Ok(Self { proxy })
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Dbus(#[from] zbus::Error),
}

/// Specification for a shell session requested.
pub struct ShellSpec<'a> {
    /// Name of container to connect to, or `".host"`.
    pub machine: &'a str,
    /// Username to start the session as.
    pub user: &'a str,
    /// Absolute path to shell to run.
    /// Leave empty for default.
    //
    // TODO probably should use Option.
    //
    // TODO use Path? That's tricky in general because current platform might not match target platform, in the long run; we might add more PTY/PTY-like providers.
    pub program: &'a str,
    /// Arguments to the shell.
    /// First argument should be the name of the program.
    /// Ignored if program is not set.
    pub args: &'a [&'a str],
    /// Environment variables to pass.
    pub environment: &'a [&'a str],
}

impl Dbus<'_> {
    /// Create a new shell session.
    ///
    /// Returns the PTY master.
    pub fn create_shell(&self, spec: &ShellSpec) -> Result<std::fs::File, Error> {
        // TODO File can be awkward for callers, doesn't allow e.g. separate threads/async tasks for read and write.
        // Maybe provide some more socket-like FD wrapper.
        // Then can provide ioctls as methods on that?

        // we don't have a use for the pty name
        let (fd, _pty_name) = self.proxy.open_machine_shell(
            spec.machine,
            spec.user,
            spec.program,
            spec.args,
            spec.environment,
        )?;
        let f = unsafe { std::fs::File::from_raw_fd(fd.as_raw_fd()) };
        Ok(f)
    }
}
