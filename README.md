# Tere — web-based shell sessions, securely

*Tere* is Estonian for hello.
*Terebra* is Latin for a mechanism to bore holes in fortified walls.
It's also a genus of sea snails with distinctive shells.
*Tere* also happens to an abbreviation of *ter*minal *e*mulator.

Tere is a radical rethinking of the terminal emulator + SSH ecosystem.
As such, it's an experiment that might go nowhere; that's okay.

We aim to replace OpenSSH with HTTPS and [WebAuthn](https://webauthn.guide/).
Those need to be made secure anyway, so this decreases what needs to be trusted.
We aim to do this with Rust, without `root`, and using strong privilege separation and sandboxing.

Any web browser will work as a client.
A command-line client can use the same protocol and codebase.

We aim to obsolete (at least) these technical limitations:

- SSH runs as `root`
- Session lifetime is bound to a single TCP connection.
- Passwords and "keys" are stored in files.
- Remote SSH hosts can [fully control](https://www.win.tue.nl/~aeb/linux/hh/hh-5.html#ss5.2) your local terminal, which is used also for other things.
  Avoiding this reduces the attack surface.
- Complete and uncontrolled delegation of authentication to remote host; SSH agent forwarding is fundamentally unable to let you see what you are allowing.


## Current status

Just about nothing exists yet.
Hold on.


## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.


## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

Copyright © 2021 Tommi Virtanen and other authors.
