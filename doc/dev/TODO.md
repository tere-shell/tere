# Tasks

Not all of these *should* happen, but they should be thought of.

## Small things

- explain `foo@` notation
- don't do TLS at all, for now, require reverse proxy
- submit mdbook-graphviz bugfixes
- bracketed paste means our input stream from user to pty should have packet types

## Vague things

- "session" is a confusing name, as that could mean a logged-in session in the web UI, or the shell session; they don't even map 1:1
- there is a *lot* of detail in [`ioctl_tty(2)`](https://man7.org/linux/man-pages/man4/tty_ioctl.4.html) that should be dealt with later

## Architectural decisions to make

### Protect against denial-of-service (DoS) attacks

In the first draft, a new `user@` instance is spawned for every WebSocket connection.
This is, in the first draft, easy for an attacker to trigger.
Simply rate limiting, on limiting max concurrent connections, can lock the intended users out.

Alternatives:

1. Don't pay that cost until after a user is authenticated
2. Hide the service at non-public URL paths (compare to SSH port change, port knocking, except much more powerful)
3. CPU-burn challenge, e.g. "give me some 32 bytes that sha256 hash to prefix 0x3571" (can be overcome by botnets)
4. Dynamic IP reputation management (can be overcome by botnets)
5. Use a VPN for "good" users (then why are we so keen on encryption and authenticatio n)
6. Delegate DoS protection to proxy such as Cloudflare (may expose HTTPS contents, depending on implementation)

Of these, paying the cost only after authentication sounds like the only real solution.
Our `server` process is relatively untrusted, we don't want to rely on its word on authentication.
But, even as untrusted, it can filter out unsuccessful authentication attempts, and thus not start paying the `user@` cost!

WebAuthn authentication is based on a server-created challenge.
To be conservative, we don't want `server` creating that challenge, even if the client response was checked in a more trusted component.
Add a new service `webauthn`, let `server` connect to it.
Process WebSocket frames in `server`, handle first few initial messages to complete WebAuthn dance (delegate all decisionmaking to `webauthn`), and only after successful authentication spawn a `user@` instance.
`webauthn` is the now the party that connects to `policy@` and `user@`, and `server` is unable to do those without successful authentication.

`server` can *also* check the completion of the authentication challenge, and forbid clients early, while only being trusted to say no.

`webauthn` would need to maintain a state table with expiry etc worries (state spill), *or* `server` and `webauthn` would have to use connection per authentication attempt to manage the state life cycle more naturally but more expensively.
*Or*, the challenges could be made stateless (as far as `webauthn` is concerned) by creating the challenge with a keyed hash from something `server` owns.
(Can rotate random keys by also trying the previous key.)

Switch `user@` protocol to be simpler length-prefixed frames, instead of WebSocket frames.
(Later, the command-line client can directly speak that protocol, avoiding WebSocket overhead, letting `server` proxy that to `user@` as raw bytes.)

TODO Worry about `server` faking/MITMing a WebAuthn *registration*.
Maybe that's best solved by demanding hardware attestation?

### `Accept=yes`

Ponder systemd `foo@.socket`, `Accept=yes`, `DynamicUser=yes` to easily sandbox connections from each other.
Probably too heavy to use for the HTTPS service, but we could use fd passing to pass the websocket fd to a dedicated process.
(TLS ruins that, but otherwise we might end up with client<->reverse proxy<->tere-service<->foo@x.)
Or, just fork and seccomp+landlock.
