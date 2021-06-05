# PTY to stdin/stdout plumbing

This file contains notes about what's needed to properly relay input/output/out-of-band events between an existing tty and a pty.
Concurrently copying data in and out from the PTY is harder than it might seem at first.
It's made a lot harder by limitations of stdin/stdout.
This document is still, and probably forever will be, woefully incomplete.


## Raw mode

Stdin needs to be switched to raw mode.
In raw mode, for example control-D is read as byte 0x04 = EOT = control-D, not as EOF.
The crate `raw_tty` works fine for this.
`raw_tty::TtyWithGuard` is not `Send` and is difficult to deal with for threads or async, so do the simpler thing instead and use

```rust
let mut guard = raw_tty::TtyModeGuard::new(stdin.as_raw_fd())?;
guard.set_raw_mode()?;
```


## Rust buffers stdout

Hideous wart and a historical waste of person-years of troubleshooting effort, faithfully copied into a greenfield system (*sigh*).
<https://github.com/rust-lang/rust/issues/58326>

Your choices are:

1. bypass Rust stdout

```rust
let stdout = std::io::stdout();
let mut stdout_unbuffered: File = unsafe { FromRawFd::from_raw_fd(stdout.as_raw_fd()) };
```

2. avoid std::io::copy and write your own loop that flushes the buffer every time.

Probably choose #2, because that buys us cancel-on-interrupt, see [Canceling reads](#canceling-reads).


## Stdin/stdout cannot be made non-blocking

Stdin and stdout are file descriptors sharing the open file between many processes, while flags like non-blocking are per-file (not per-fd) properties.
It's an old UNIX wart, and we're stuck with it.
What that means is that stdin/stdout **must not** be made non-blocking, as that will break unrelated programs that just happen to use them.


## Canceling reads

So if stdin is in blocking mode, a `read(2)` on it will block until data is available.
When the PTY closes (child exits), we need to clean that up somehow.

We can do it with pthreads and signals.
We need to reimplement our own copy loop because std::io::copy retries on EINTR.


## Using a `std::fs::File` for PTY FD is too hard

Rust lifetimes won't let a `File` be handled by separate readers and writers.
You can `dup(2)` the FD but that might lead to other traps, and is wasteful at scale.
Probably need to write our own type implementing `Read` and `Write`.


## SIGWINCH

Relay window size from tty to pty.
Also remember to set initial window size.
