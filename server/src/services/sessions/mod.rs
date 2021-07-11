use std::collections::HashMap;
use std::convert::TryFrom;
use std::ffi::OsStr;
use std::os::unix::net::{UnixDatagram, UnixListener};
use std::os::unix::prelude::{FromRawFd, IntoRawFd};
use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;

use crate::dbus_shell;
use crate::dbus_shell::Dbus;
use crate::ipc;
use crate::ipc::seqpacket::SeqPacket;
use crate::ipc::IPC;
use crate::proto;
use crate::proto::sessions as p;
use crate::pty_master::PtyMaster;
use crate::socket_activation;
use crate::socket_activation::SocketActivation;

#[derive(Error, Debug)]
pub enum RunError {
    #[error("D-Bus connect error: {0}")]
    Dbus(#[from] dbus_shell::ConnectError),

    #[error("socket activation error: {0}")]
    SocketActivation(#[from] socket_activation::Error),

    #[error("socket for sessions service not found")]
    NoSocketForSessions,
}

pub fn run() -> Result<(), RunError> {
    let activation = SocketActivation::new();
    let sockets = activation.parse().map_err(RunError::SocketActivation)?;
    // TODO Iterator-based API is perhaps too annoying to consumers who care about specific names?
    let mut socket_sessions = None;
    for filedesc in sockets {
        if filedesc.name() == Some(OsStr::new("tere-sessions")) {
            // RUST-WART There's no SeqPacketListener.
            // We'll just kludge from UnixListener where we convert the connections to UnixDatagram.
            let fd: UnixListener = filedesc.take_fd();
            socket_sessions.insert(fd);
            continue;
        }
        // TODO handle `s_*` saved FDs.
    }
    let listener = socket_sessions.ok_or(RunError::NoSocketForSessions)?;
    let dbus = Dbus::new().map_err(RunError::Dbus)?;
    serve(dbus, listener);
    Ok(())
}

enum Session {
    Creating,
    Ready {
        pty_master: PtyMaster,
        pty_service_conn: Arc<SeqPacket>,
    },
}

const SESSION_ID_BYTES: usize = 24;
type SessionId = [u8; SESSION_ID_BYTES];

fn serve(session_starter: dbus_shell::Dbus<'static>, listener: UnixListener) {
    // Even with `Arc`, passing this to threads forces us to insist on `'static` for the argument.
    // Good thing that happens to be true!
    let session_starter = Arc::new(session_starter);
    let sessions: Arc<Mutex<HashMap<SessionId, Arc<Mutex<Session>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let fd = stream.into_raw_fd();
                let socket = unsafe { UnixDatagram::from_raw_fd(fd) };
                let conn = SeqPacket::try_from(socket).expect("stdin is not a SOCK_SEQPACKET");
                let session_starter = session_starter.clone();
                let sessions = sessions.clone();
                std::thread::spawn(move || {
                    let result = serve_conn(session_starter, sessions, conn);
                    if let Err(error) = result {
                        // TODO Proper error logging.
                        eprintln!("error serving connection: {0}", error);
                    }
                });
            }
            Err(error) => {
                // TODO Proper error logging.
                eprintln!("error from accept: {0}", error);
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum ConnError {
    #[error("error handshaking: {0}")]
    Handshake(#[source] ipc::handshake::Error),

    #[error("socket receive error: {0}")]
    Receive(#[source] ipc::ReceiveError),
}

fn serve_conn(
    session_starter: Arc<dbus_shell::Dbus<'_>>,
    sessions: Arc<Mutex<HashMap<SessionId, Arc<Mutex<Session>>>>>,
    conn: impl ipc::IPC,
) -> Result<(), ConnError> {
    // TODO Configuration for unit testability:
    //
    // - dbus_shell via trait
    // - pty service via trait

    ipc::handshake::handshake_as_server(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
        .map_err(ConnError::Handshake)?;

    loop {
        let request: p::Request = conn.receive_with_fds().map_err(ConnError::Receive)?;
        // Handle incoming requests on one connection as run-to-completion, since they are coming from a single `policy@` instance and thus from a single user.
        println!("request: {:?}", &request);
        match request {
            p::Request::CreateShellSession(create) => {
                let machine = match &create.machine {
                    p::Machine::Host => ".host",
                    p::Machine::Container(name) => name,
                };
                let spec = dbus_shell::ShellSpec {
                    // TODO Enforce input doesn't start start with ".", force use of `Machine::Container`.
                    // Try to put that on the deserialization layer.
                    machine,
                    user: &create.user,
                    program: "",      // TODO fill
                    args: &[],        // TODO fill
                    environment: &[], // TODO fill
                };
                let (session_id, session_entry) = {
                    let mut guard = sessions
                        .lock()
                        .expect("internal: sessions map mutex poison");
                    loop {
                        let session_id: SessionId = rand::random();
                        let entry = guard.entry(session_id);
                        use std::collections::hash_map::Entry;
                        match entry {
                            Entry::Occupied(_) => continue,
                            Entry::Vacant(vacant) => {
                                let session_entry = Arc::new(Mutex::new(Session::Creating));
                                vacant.insert(session_entry.clone());
                                break (session_id, session_entry);
                            }
                        }
                    }
                };
                println!("session_id: {:?}", session_id);

                let pty_master = session_starter
                    .create_shell(&spec)
                    // TODO Should we report errors on client FD?
                    // It hasn't done handshake yet.
                    // At the very least, log and go back to loop.
                    .expect("TODO handle dbus create shell session error");
                let pty_service_location = "/run/tere/socket/pty.socket";
                let pty_conn = SeqPacket::connect(pty_service_location)
                    // TODO non-fatal error handling
                    .expect("TODO handle pty service error");
                let pty_conn = Arc::new(pty_conn);

                ipc::handshake::handshake_as_client(
                    pty_conn.as_ref(),
                    proto::pty::CLIENT_INTENT,
                    proto::pty::SERVER_INTENT,
                )
                // TODO non-fatal error handling
                .expect("TODO handle pty service error");

                // Jump through hoops to get ownership of `pty_master` back.
                let pty_master = {
                    let message = proto::pty::Init {
                        _dummy: 0,
                        pty_master,
                    };
                    pty_conn
                        .send_with_fds(&message)
                        // TODO non-fatal error handling
                        .expect("TODO handle pty service error");
                    message.pty_master
                };

                {
                    let mut guard = session_entry
                        .lock()
                        .expect("internal: session mutex poison");
                    match *guard {
                        Session::Creating => (),
                        _ => panic!("internal: someone stole our session id"),
                    }
                    *guard = Session::Ready {
                        pty_master,
                        pty_service_conn: pty_conn.clone(),
                    };
                }

                {
                    let message = proto::pty::Request::NewClient {
                        _dummy: 0,
                        fd: create.fd,
                    };
                    pty_conn
                        .send_with_fds(&message)
                        // TODO non-fatal error handling
                        .expect("TODO handle pty service error");
                }
            }
        }
    }
}
