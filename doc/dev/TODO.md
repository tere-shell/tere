# Tasks

Not all of these *should* happen, but they should be thought of.

## Small things

- don't do TLS at all, for now, require reverse proxy
- submit mdbook-graphviz bugfixes
- bracketed paste means our input stream from user to pty should have packet types

## Vague things

- "session" is a confusing name, as that could mean a logged-in session in the web UI, or the shell session; they don't even map 1:1
- there is a *lot* of detail in [`ioctl_tty(2)`](https://man7.org/linux/man-pages/man4/tty_ioctl.4.html) that should be dealt with later

## Architectural decisions to make

### `Accept=yes`

Ponder systemd `foo@.socket`, `Accept=yes`, `DynamicUser=yes` to easily sandbox connections from each other.
Probably too heavy to use for the HTTPS service, but we could use fd passing to pass the websocket fd to a dedicated process.
(TLS ruins that, but otherwise we might end up with client<->reverse proxy<->tere-service<->foo@x.)
Or, just fork and seccomp+landlock.
