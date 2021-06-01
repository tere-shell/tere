# Roadmap

## Soon

- rewrite the (unpublished) prototype into publishable form
- reach working code
- privilege separation and sandboxing

## Later

- reconnect
- sessions survive restart
- TLS without external proxy
- <span id="command-line-client">command-line client
- non-interactive sessions

## Maybe

- <span id="keep-terminal-state">keep terminal state for seamless reconnects
- session sharing
- <span id="authentication-forwarding">authenticate from initial server to further one
  - either we can connect directly to final destination, or we can't ("bastion host")
  - can only WebAuthn if browser can connect to final destination
- <span id="unattended-authentication">unattended authentication
  - limit what commands can be run
- <span id="wasm-terminal">switch from [hterm](https://hterm.org) to [alacritty_terminal](https://crates.io/crates/alacritty_terminal) in WASM

  This would especially help with [keeping terminal state](roadmap.md#keep-terminal-state), as we could have the same terminal emulator on both sides, and avoid confusion.
