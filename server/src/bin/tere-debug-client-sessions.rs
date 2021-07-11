use tere_server::ipc;
use tere_server::ipc::seqpacket::SeqPacket;
use tere_server::ipc::IPC;

fn main() {
    let path = "/run/tere/socket/sessions.socket";
    let conn = SeqPacket::connect(path).expect("connect");

    let client_conn = {
        use tere_server::proto::sessions as p;
        ipc::handshake::handshake_as_client(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
            .expect("handshake");
        let (client_conn, server_socket) = SeqPacket::pair().expect("socketpair");
        let message = p::Request::CreateShellSession(p::CreateShellSession {
            fd: server_socket,
            machine: p::Machine::Host,
            user: "testuser".to_string(),
            program: None,
            args: None,
            env: None,
        });
        conn.send_with_fds(&message).expect("send request");
        client_conn
    };

    // now pretend we're the client
    {
        let conn = client_conn;
        use tere_server::proto::pty::user as p;
        ipc::handshake::handshake_as_client(&conn, p::CLIENT_INTENT, p::SERVER_INTENT)
            .expect("handshake");
        {
            let message = p::Input::KeyboardInput(b"date\r\n".to_vec());
            conn.send_with_fds(&message).expect("send input");
        }
        loop {
            let message: p::Output = conn.receive_with_fds().expect("receive output");
            println!("output: {:?}", message);
            match message {
                p::Output::SessionOutput(b) => println!("output: {}", String::from_utf8_lossy(&b)),
            }
            {
                let message = p::Input::KeyboardInput(b"\x04".to_vec());
                conn.send_with_fds(&message).expect("send input");
            }
        }
    }
}
