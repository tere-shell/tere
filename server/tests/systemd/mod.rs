use thiserror::Error;

mod proxies {
    // zbus makes these public, avoid exposing them
    // https://gitlab.freedesktop.org/dbus/zbus/-/issues/129

    use serde::{Deserialize, Serialize};
    use zbus::dbus_proxy;

    #[derive(Debug, zvariant_derive::Type, Serialize, Deserialize)]
    pub struct Unit {
        pub id: String,
        pub description: String,
        pub load_state: String,
        pub active_state: String,
        pub sub_state: String,
        pub follows: String,
        pub object_path: zvariant::OwnedObjectPath,
        pub job_id: u32,
        pub job_type: String,
        pub job_object_path: zvariant::OwnedObjectPath,
    }

    #[dbus_proxy(
        interface = "org.freedesktop.systemd1.Manager",
        default_service = "org.freedesktop.systemd1",
        default_path = "/org/freedesktop/systemd1"
    )]
    trait SystemdManager {
        fn list_units_by_patterns(
            &self,
            states: &[&str],
            patterns: &[&str],
        ) -> zbus::Result<Vec<Unit>>;
    }

    #[dbus_proxy(
        interface = "org.freedesktop.systemd1.Service",
        default_service = "org.freedesktop.systemd1"
    )]
    trait Service {
        #[dbus_proxy(property, name = "MainPID")]
        fn main_pid(&self) -> zbus::Result<u32>;
    }
}

pub use self::proxies::{ServiceProxy, Unit};

/// Client to the [`systemd`](https://man7.org/linux/man-pages/man5/org.freedesktop.systemd1.5.html) D-Bus API.
pub struct Dbus<'a> {
    proxy: proxies::SystemdManagerProxy<'a>,
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
        let proxy = proxies::SystemdManagerProxy::new(&connection)
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

impl Dbus<'_> {
    pub fn list_units_by_patterns(
        &self,
        states: &[&str],
        patterns: &[&str],
    ) -> Result<Vec<Unit>, Error> {
        self.proxy
            .list_units_by_patterns(states, patterns)
            .map_err(Error::Dbus)
    }

    // This is probably a very wrong way to do that, and zbus might have some way to do this more directly.
    // Couldn't figure it out in time.
    // Exposing ServiceProxy sort of wrecks the abstraction we've been trying to do here.
    pub fn get_systemd_service_for_path<'a>(
        &self,
        path: &'a str,
    ) -> Result<ServiceProxy<'a>, zbus::Error> {
        let connection = self.proxy.inner().connection();
        ServiceProxy::new_for_path(connection, path)
    }
}
