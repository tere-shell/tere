# WebSockets

## Parsing in a different process

WebSockets (as used over HTTP/1; HTTP/2 will likely change this?) have an interesting property where no information from the handshake is needed to handle the actual WebSocket protocol.

For example, the "frame masking" XOR key is [included in each frame](https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API/Writing_WebSocket_servers), and not in the initial handshake.

(And even if there was some such data, it'd be easy to pass that state along too.)

Thus, WebSockets can be served by a different process, by "hijacking" the TCP connection (beware TLS, HTTP/2[^http2]) from the web server and using fd passing to hand it to a different process.

[WebSocket::from_raw_socket](https://docs.rs/tungstenite/0.13.0/tungstenite/protocol/struct.WebSocket.html#method.from_raw_socket)

[WebSocket::from_partially_read](https://docs.rs/tungstenite/0.13.0/tungstenite/protocol/struct.WebSocket.html#method.from_partially_read)


[^http2]:
Tungstenite issue to add support: <https://github.com/snapview/tungstenite-rs/issues/206>.
Even with HTTP/2, it should be possible to shovel bytes to a different process, and only parse WebSocket frames there.
