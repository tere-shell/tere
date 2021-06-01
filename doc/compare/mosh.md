# Mosh

[Mosh](https://mosh.org/) is an SSH wrapper that uses a custom UDP protocol after a handshake conducted via SSH.
It's claim to fame with low latency typing feedback, including a predictive mode where your input is echoed in the local terminal even before it has been transmitted to the server.

Mosh is purely an interactive tool, port forwarding and file transfers would still be done with [SSH](openssh.md).

Mosh requires a unique open UDP port for every session.
Firewalls typically forbid this, and for good reasons.
Tere looks like regular web traffic, and servers can be behind any HTTP reverse proxy that supports WebSockets over TLS.

## Similarities

- survive temporary network failures and IP address changes

## Tere current limitations

These might be lifted with work, later, but for now, Tere definitely cannot do  these things:

- we don't even attempt to do predictive input

## Tere architectural limitations

We probably won't (be able to) do anything about these, but external factors might change:

- no UDP low latency tricks

  We're aiming to support browser clients, and thus use WebSocket as transport, not a custom UDP protocol, and this puts some limits on our latency.
  HTTP/3 may help with this, later.
  A standalone client could avoid browser restrictions, but that doesn't seem worth the effort.
