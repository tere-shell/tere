# Privilege separation

The Tere server consists of multiple co-operating, mostly mutually distrusting, processes.
(Threads are cheaper and can self-sandbox too, but that boundary doesn't prevent reading secrets.)

OpenSSH just privseps from `root` into the destination user account.
We, however, might be launching sessions in containers, and not on the host itself, we don't switch `uid` ourself, and the user might not even exist on the host.
We don't even run as `root` to be able to switch users!
We need to do things differently.


## Services and processes

The services communicate with each other via UNIX domain sockets.
Only `tere-server` serves TCP sockets, for HTTPS and HTTP.
All listening sockets are created via systemd socket activation (see [systemd.socket](https://www.freedesktop.org/software/systemd/man/systemd.socket.html)).

All of the services use systemd configuration [`DynamicUser=yes`](https://www.freedesktop.org/software/systemd/man/systemd.exec.html#DynamicUser=) to run as a unique system account.
Extra group memberships (via [`SupplementaryGroups=`](https://www.freedesktop.org/software/systemd/man/systemd.exec.html#SupplementaryGroups=)) are used for access control between services, for UNIX domain sockets (groups `tere-socket-*`, named after the service connecting  *to*) and D-Bus (group `tere-dbus`).

Services named with a `@` suffix (after the [systemd convention](https://www.freedesktop.org/software/systemd/man/systemd.unit.html#Description)) run as multiple instances.
They use systemd [`Accept=yes`](https://www.freedesktop.org/software/systemd/man/systemd.socket.html#Accept=) to make systemd start a new instance for every incoming connection.
Additionally,  [`DynamicUser=yes`](https://www.freedesktop.org/software/systemd/man/systemd.exec.html#DynamicUser=) will isolate each connection from each other([1](http://0pointer.net/blog/dynamic-users-with-systemd.html)).


```dot process
{{#include privsep-big-picture.gv}}
```



### `tere-server` is the entry point from the network

`tere-server` serves things like static HTTP assets.
It holds an extra group membership `tere-socket-auth`.
It has a long-living connection to `tere-auth`.

`tere-server` also handles WebSocket connections.
It processes the first few initial messages that authenticate the user, and after authentication copies data between WebSocket messages and `tere-user@`[^websocket-fd-pass] (without parsing).
This is to keep unauthenticated actions lightweight, to avoid easy denial of service attacks.

Authentication happens inside the WebSocket connection and is tied to the WebSocket connection lifetime, to avoid the need for session tokens that could be stolen.

All decision-making regarding authentication is delegated to `tere-auth`.

[^websocket-fd-pass]:
In the separate TLS termination, non-HTTP/2 case, `tere-server` could use FD passing to completely transfer the WebSocket-speaking network socket to `tere-user@`.
With TLS or WebSockets-over-HTTP/2, it will have to stay in the data path, wrapping and unwrapping the stream in these transports.
We'd rather aim for the future, so we're doing that.


### `tere-auth` authenticates users

`tere-auth` makes WebAuthn authentication decisions.
It holds extra group memberships `tere-socket-user` and `tere-socket-policy`.

After successful authentication, it connects to the `tere-policy@` service, which makes systemd spawn a new process.
It also connects to the `tere-user@` service, and after a handshake passes the just established policy connection to the new `tere-user@` instance.[^why-auth-to-policy]
This indirection removes `tere-auth` from the data path for an authenticated connection.


[^why-auth-to-policy]:
Why connect to `tere-policy@` from `tere-auth`, not from `tere-user@` where that connection is actually used?
So that `tere-user@`, which deals with more complex hostile input, can't make *other* connections and claim to be other usernames.


### `tere-user@` speaks with with clients

`tere-user@` parses most complex potentially hostile input, it is named so because it is the user representative on the server host, and most likely to come under complete user control.
Each authenticated WebSocket connection gets a dedicated `tere-user@` worker.

It speaks a length-prefixed message protocol.
WebSocket frames from browser clients will be translated to these messages by `tere-server`, and once a command-line client exists, it'll probably bypass the WebSocket protocol and speak this protocol directly.

It transports shell session requests, shell session input/output streams, and such between the client and the `tere-policy@` worker, and via further FD passing to `tere-sessions` and further to `tere-pty@`.


### `tere-policy@` makes authorization decisions

`tere-policy@` makes authorization decisions based on its configuration.
It holds an extra group membership `tere-socket-sessions`.
Each authenticated WebSocket connection gets a dedicated `tere-user@` worker.

It reads a message from the connection stating the username, and binds the connection to that username in its state, before parsing any user input.

It processes authenticated end user requests such as "create a new shell sessions", running them through a policy engine, and when allowed relays them to the `tere-sessions` service, relaying data and passed FDs back to the client.


### `tere-sessions` starts and manages shell sessions

`tere-sessions` uses D-Bus to talk to [`systemd-machined`](https://www.freedesktop.org/software/systemd/man/systemd-machined.service.html) to create shell sessions.
It holds an extra group memberships `tere-dbus` (that allows the above) and `tere-socket-sessions`.

For every created shell session, `systemd-machined` returns a PTY FD.
`tere-sessions` connects to `tere-pty@`, which makes systemd spawn a new process, and hands the PTY FD to that new process.
It remembers that connection, identified by a random unique session ID.
`tere-sessions` [stores both of these FDs in systemd](https://www.freedesktop.org/software/systemd/man/sd_notify.html#FDSTORE=1) for restarts.[^store-both-fds]
Later requests to open an existing connection are served by messaging the right `tere-pty@` process.

For connecting to already existing shell sessions, `tere-sessions` proxies the request to the `tere-pty@` instance for that session.

`tere-sessions` exists as separate from `tere-pty@` for two reasons: to prevent D-Bus access after a sandbox escape, and to have a place that can store and re-serve PTY FDs after a software restart or crash.


[^store-both-fds]:
Both the PTY FD and the connection to `tere-pty@` are stored so `tere-sessions` can avoid starting a new `tere-pty@` instance when a previous one is already serving that PTY FD.
To handle crash recovery correctly, the communication must not lose track of message boundaries; we can use [`SOCK_SEQPACKET`](https://man7.org/linux/man-pages/man2/socket.2.html) for that.
The alternative would be to kill all `tere-pty@` processes on `tere-sessions` exit, e.g. via [BindsTo=](https://www.freedesktop.org/software/systemd/man/systemd.unit.html#BindsTo=) or [PartOf=](https://www.freedesktop.org/software/systemd/man/systemd.unit.html#PartOf=).


### `tere-pty@` transports data to and from a PTY

`tere-pty@` receives the PTY FD over the incoming connection, and later receives client requests.
Each shell session a dedicated `tere-pty@` worker.
Note that shell sessions and active users do not necessarily map 1:1.

`tere-pty@` serves clients connecting to its sessions by creating a socketpair, used for PTY input/output and commands, and passing this FD to the client.
This indirection removes `tere-sessions` from the bulk data path.
Commands include updating the terminal size on window resize.

It transports data between the PTY and (once implemented) multiple clients, broadcasting the same PTY output stream to all clients.


## Resources

- https://landlock.io/ and https://crates.io/crates/landlock
- https://github.com/openssh/openssh-portable/blob/master/README.privsep
- http://www.citi.umich.edu/u/provos/ssh/privsep.html
  ([archived](https://web.archive.org/web/20210424044821/http://www.citi.umich.edu/u/provos/ssh/privsep.html))
