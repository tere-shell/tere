# Architecture

**Caution:**
This document contains wishful thinking.
Not everything is true yet.

The Tere server is written in Rust.
It does *not* run as `root` or need `CAP_ADMIN`.

It uses systemd socket activation ([1](http://0pointer.de/blog/projects/socket-activation.html), [2](https://www.freedesktop.org/software/systemd/man/systemd.socket.html)) to serve HTTPS[^https].
TLS termination may be done by a proxy in front of the service, if wanted.

The browser client uses [hterm](https://hterm.org/), at least for now[^wasm].
User sessions are transported over a WebSocket connection.
Sessions survive TCP connection loss and IP address changes.

User authentication is done with [WebAuthn](https://webauthn.guide/).
This means user must have suitable hardware, for example a [YubiKey](https://www.yubico.com/products/yubikey-5-overview/).

Sessions are run via systemd, with the [same mechanism](https://man7.org/linux/man-pages/man5/org.freedesktop.machine1.5.html) as [`machinectl shell`](https://www.freedesktop.org/software/systemd/man/machinectl.html#shell%20%5B%5BNAME@%5DNAME%20%5BPATH%20%5BARGUMENTS%E2%80%A6%5D%5D%5D%20).


[^https]:
For testing, `localhost` connections can use HTTP.
WebAuthn prevents us from allowing plaintext in production, and this is a good thing.

[^wasm]:
We might [switch to WASM](../roadmap.md#wasm-terminal), one day.
