# How does Tere compare to OpenSSH?

This probably applies to the SSH protocol in general, but everyone uses OpenSSH, right?

## Tere current limitations

These might be lifted with work, later, but for now, Tere definitely cannot do  these things:

- non-interactive sessions
  - file transfer: rsync, sftp/scp
  - port forwarding
- authentication forwarding
- unattended authentication (key files)

Non-interactive use should be doable, but first [the command-line client needs to exist](../roadmap.md#command-line-client), as this is beyond the realm of a browser client.
After that rsync and such are just a question of using the `-e` option.
Port forwarding should be able to use that, even if the feature is not built in.

[Authentication forwarding](../roadmap.md#authentication-forwarding) needs to be completely rethought to be safe.
We might need to require the client to be able to connect directly to all servers for authentication, even if the data connection is from one server to another; need to experiment with proxying WebAuthn to know for sure.

We don't have a clear plan yet for [unattended authentication](../roadmap.md#unattended-authentication) and that is somewhat at odds with our security stance of not liking key files.
Maybe we'll add a mechanism that limits what can be done with a secret token.
