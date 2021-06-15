# Tasks

Not all of these *should* happen, but they should be thought of.

## Small things

- don't do TLS at all, for now, require reverse proxy
- submit mdbook-graphviz bugfixes
- bracketed paste means our input stream from user to pty should have packet types

## Vague things

- "session" is a confusing name, as that could mean a logged-in session in the web UI, or the shell session; they don't even map 1:1
- there is a *lot* of detail in [`ioctl_tty(2)`](https://man7.org/linux/man-pages/man4/tty_ioctl.4.html) that should be dealt with later
- potential extra obfuscation for DoS avoidance, if wanted:

  1. Hide the service at non-public URL paths (compare to SSH port change, port knocking, except much more powerful)
  2. CPU-burn challenge, e.g. "give me some 32 bytes that sha256 hash to prefix 0x3571" (can be overcome by botnets)
  3. Dynamic IP reputation management (can be overcome by botnets)
  4. Use a VPN for "good" users (then why are we so keen on encryption and authentication)
  5. Delegate DoS protection to proxy such as Cloudflare (may expose HTTPS contents, depending on implementation)


## Architectural decisions to make

### WebAuth challenge creation

WebAuthn authentication is based on a server-created challenge.
To be conservative, we don't want `server` creating that challenge, even if the client response was checked in a more trusted component.

`auth` would need to maintain a state table with expiry etc worries (state spill), *or* `server` and `auth` would have to use connection per authentication attempt to manage the state life cycle more naturally but more expensively.
*Or*, the challenges could be made stateless (as far as `auth` is concerned) by creating the challenge with a keyed hash from something `server` owns, or signing them.
(Can rotate random keys by also trying the previous key.)

`server` can *also* check the completion of the authentication challenge, and forbid clients early, while only being trusted to say no.
Doesn't seem worth it with a long-living connection to `auth`.

### WebAuthn register MITM or spoofing

TODO Worry about `server` faking/MITMing a WebAuthn *registration*.
Maybe that's best solved by demanding hardware attestation?
