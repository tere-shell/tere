# Privilege separation

The Tere server consists of multiple co-operating, mostly mutually distrusting, processes.
(Threads are cheaper and can self-sandbox too, but that boundary doesn't prevent reading secrets.)

OpenSSH just privseps from `root` into the destination user account.
We, however, might be launching sessions in containers, and not on the host itself, we don't switch `uid` ourself, and the user might not even exist on the host.
We don't even run as `root` to be able to switch users!
We need to do things differently.


## Services and processes

```dot process
{{#include privsep-big-picture.gv}}
```

All of the below services use systemd configuration [`DynamicUser=yes`](https://www.freedesktop.org/software/systemd/man/systemd.exec.html#DynamicUser=).
Extra group memberships (via [`SupplementaryGroups=`](https://www.freedesktop.org/software/systemd/man/systemd.exec.html#SupplementaryGroups=)) are used for access control between services, for UNIX domain sockets and D-Bus.


### `tere-server` is the entry point from the network

`tere-server` serves things like static HTTP assets.
It processes the initial handshake of WebSocket connections, but not the actual WebSocket frames.
WebSocket connection handshakes state the username they are attempting to authenticate as<!-- TODO header, path or query TBD-->.
For every WebSocket connection, `tere-server` connects to the `tere-user@` service, which makes systemd spawn a new process.
It also connects to the `tere-policy@` service, and after a handshake passes this connection to the new `tere-user@` instance.[^why-server-to-policy]

In the non-TLS, non-HTTP/2 case, `tere-server` can use FD passing to completely transfer the network socket to `tere-user@`.
With TLS or WebSockets-over-HTTP/2, it will have to stay in the data path, wrapping and unwrapping the stream in these transports.

[^why-server-to-policy]:
Why connect to `tere-policy@` from `tere-server`, not from `tere-user@` where that connection is actually used?
So that `tere-user@`, which deals with more complex hostile input, can't make *other* connections and claim to be other usernames.


### `tere-user@` speaks with with clients

`tere-user@` parses most complex potentially hostile input, it is named so because it is the user representative on the server host, and most likely to come under complete user control.

It speaks the WebSocket frame protocol, though once a command-line client exists, we may prefer using a more native transport instead of WebSocket, likely a connection hijack for HTTP/1 and just proxying data for HTTP/2.

It listens on a UNIX domain socket that only group `tere-socket-user` can connect to.
It uses systemd [`Accept=yes`](https://www.freedesktop.org/software/systemd/man/systemd.socket.html#Accept=) and [`DynamicUser=yes`](https://www.freedesktop.org/software/systemd/man/systemd.exec.html#DynamicUser=) to isolate each connection from each other([1](http://0pointer.net/blog/dynamic-users-with-systemd.html)).

Authentication happens inside the WebSocket connection, with `tere-user@` parsing client messages but delegating all decision-making to `tere-policy@`.
After authentication, the worker transports shell session requests, shell session input/output streams and such between the client and the `tere-policy` worker, and via further FD passing to a `tere-sessions` worker.

### `tere-policy@` is a gatekeeper

`tere-policy@` manages both authentication and authorization.
It listens on a UNIX domain socket that only user `tere-server` can connect to.
It uses systemd [`Accept=yes`](https://www.freedesktop.org/software/systemd/man/systemd.socket.html#Accept=) and [`DynamicUser=yes`](https://www.freedesktop.org/software/systemd/man/systemd.exec.html#DynamicUser=) to isolate each connection from each other([1](http://0pointer.net/blog/dynamic-users-with-systemd.html)).
It holds an extra group membership `tere-socket-sessions`.

It reads a message from the connection stating the (still unauthenticated) username, and binds the connection to that username in its state, before parsing any user input.

It authenticates the user, and processes end user requests such as "create a new shell sessions", running them through a policy engine, and when allowed relays them to the `tere-sessions` service, relaying data and passed FDs back to the client.


### `tere-sessions` starts and manages shell sessions

`tere-sessions` listens on a UNIX domain socket that only group `tere-socket-policy` can connect to.
It uses D-Bus to talk to [`systemd-machined`](https://www.freedesktop.org/software/systemd/man/systemd-machined.service.html) to create shell sessions.
It holds an extra group membership `tere-dbus` that allows the above.

For every created shell session, `systemd-machined` returns a PTY FD.
`tere-sessions` connects to `tere-pty@`, which makes systemd spawn a new process, and hands the PTY FD to the new process.
It remembers that connection, identified by a random unique session ID.
`tere-sessions` [stores both of these FDs in systemd](https://www.freedesktop.org/software/systemd/man/sd_notify.html#FDSTORE=1) for restarts.[^store-both-fds]
Later requests to open an existing connection are served by messaging the right `tere-pty@` process.

`tere-sessions` exists as separate from `tere-pty@` for two reasons: to prevent D-Bus access after a sandbox escape, and to have a place that can store and re-serve PTY FDs after a software restart or crash.


[^store-both-fds]:
Both the PTY FD and the connection to `tere-pty@` are stored so `tere-sessions` can avoid starting a new `tere-pty@` instance when a previous one is already serving that PTY FD.
To handle crash recovery correctly, the communication must not lose track of message boundaries; we can use [`SOCK_SEQPACKET`](https://man7.org/linux/man-pages/man2/socket.2.html) for that.
The alternative would be to kill all `tere-pty@` processes on `tere-sessions` exit, e.g. via [BindsTo=](https://www.freedesktop.org/software/systemd/man/systemd.unit.html#BindsTo=) or [PartOf=](https://www.freedesktop.org/software/systemd/man/systemd.unit.html#PartOf=).


### `tere-pty@` transports data to and from a PTY

`tere-pty@` listens on a UNIX domain socket that only group `tere-socket-pty` can connect to.
It uses systemd [`Accept=yes`](https://www.freedesktop.org/software/systemd/man/systemd.socket.html#Accept=) and [`DynamicUser=yes`](https://www.freedesktop.org/software/systemd/man/systemd.exec.html#DynamicUser=) to isolate each connection from each other([1](http://0pointer.net/blog/dynamic-users-with-systemd.html)).

`tere-pty@` receives the PTY FD over the incoming connection, and later receives client requests.

It serves new clients connecting to its sessions by creating two socketpairs, one for PTY input/output and the other as a command channel, and passing these FDs to the client.

It transports data between the PTY and (once implemented) multiple clients, broadcasting the same PTY output stream to all clients.

Some clients may be read-only and are for them the PTY input side of the socket is shutdown.

The command channel is used to update the terminal size on window resize.


## Resources

- https://landlock.io/ and https://crates.io/crates/landlock
- https://github.com/openssh/openssh-portable/blob/master/README.privsep
- http://www.citi.umich.edu/u/provos/ssh/privsep.html
  ([archived](https://web.archive.org/web/20210424044821/http://www.citi.umich.edu/u/provos/ssh/privsep.html))
