# D-Bus

D-Bus is the systemd-affiliated "message bus".
It's basically a baroque IPC mechanism transported over UNIX domain sockets, with a "hub" process in the middle.

We're required to use it in order to programmatically do what `machinectl shell` does, which lets us open new session the right way, either on the host or in containers.
The ability to use D-Bus will be sandboxed and privilege separated.<!-- TODO linkify privsep -->

As this allows our service to start shells as arbitrary users, it requires elevated privileges.
Instead of running as `root`, or needing `CAP_ADMIN`, we instead simply configure our system account to be allowed to do that.
This happens in two steps.

1. Tell `dbus-daemon` to let us make the relevant D-Bus method call, via `/usr/share/dbus-1/system.d/50-tere.conf`.

<!-- TODO mdbook include mechanism can't cope with indentation -->
```xml
{{#include ../../server/systemd/share/dbus-1/system.d/50-tere.conf}}
```

```shell
# systemctl reload dbus.service
```

2. `dbus-daemon` confirms that this is allowed via `polkit`, yet another service.
    Allow it via `/usr/share/polkit-1/rules.d/50-tere.rules`.

```javascript
{{#include ../../server/systemd/share/polkit-1/rules.d/50-tere.rules}}
```

(Reload is automatic.)


## Limiting Tere sessions

A site admin can add or adjust polkit rules to suit their needs.
For example, you could

- prevent Tere from starting sessions on the host, only allow containers
- prevent Tere from starting sessions as root (or any system account)
- enforce what shell program Tere starts

Starting from systemd v247[^commit], polkit rules can use `action.lookup(key)`, and `systemd-machined` defines keys `machine`, `user`, `program` for what session is being started.

It would be nice to be able to pass more Tere-specific metadata to the polkit rules, but that won't happen without `systemd-machined` changes.

[^commit]:
See commit [09364a](https://github.com/systemd/systemd/commit/09364a8043a2f9b698e49a172094d658ae289ac6).


## Resources

<https://www.freedesktop.org/wiki/Software/dbus/>

<https://dbus.freedesktop.org/doc/dbus-specification.html>

<https://dbus.freedesktop.org/doc/dbus-daemon.1.html>
