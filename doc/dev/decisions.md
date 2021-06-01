# Decisions

These are things that have already been thought about, to various levels.
Some go into more detail, as necessary.

## Pretty firm


### Rust

Tere is programmed in Rust.
It gives a nice sense of safety, but even more importantly it allows low-level control over threads, processes & syscalls that makes Seccomp and Landlock style fine-grained sandboxing feasible.


### systemd

Why use systemd?
Because it gets us features, and it's going to be installed anyway.

- Tere doesn't even need to run as root!
- systemd will let us create sessions, on host or in container, with a simple call, not worry about dropping privileges and what not
- systemd will sandbox our whole service
- systemd will terminate sessions reliably, with cgroups, including if we crash before persisting a pty fd
- we can store pty fds in systemd and have sessions survive restarts

Not so nice trade-offs include

- need to use in-filesystem UNIX domain sockets to connect privsep components, instead of `socketpair` and `fork`
- runtime reliability depends on a whole lot of components (then again, most Linux distros these days depend on them anyway)

What about other platforms and niche Linux distributions?
We can work on compatibility later, need to get things working first.
Nothing in the big picture depends on systemd, it's just conveniences, shortcuts and extra features.


### Need multiple `uid`s or `gid`s

Since we use polkit rules to allow the equivalent of `machinectl shell` without `CAP_ADMIN`, and because those rules can only enforce based on `uid` or `gid`[^only-uid-or-gid], we want to isolate that capability in even the simplest design, as it's the highest risk.

Difficult choices ahead: start as root and fork+setuid, or expose multiple services to the admin and risk version skew.
Leaning toward multiple services and detect skew.

[^only-uid-or-gid]:
See [polkit(8)](https://www.freedesktop.org/software/polkit/docs/latest/polkit.8.html).
`pid` is ridiculously hard to use in static files.
`groups` and `user` are about as good for us.
`seat`, `session`, `local` and `active` seem to be about interactive users, not about system services.


### systemd template services instead of fork or fork+exec

We could have had parent processes fork/fork+exec worker processes.
That could have made it more sure we're using the same executable image, either not exec'ing or using `/proc/self/exe` or such (Self-exec can help purge resources like open fds, re-randomize some attack mitigations; or use a "nursery" process that spawns children without leftover state).
We chose not to, because either all the child processes would run as the same `uid`, or we'd need `CAP_ADMIN` to switch groups.

Instead, systemd [`DynamicUser=yes`](https://www.freedesktop.org/software/systemd/man/systemd.exec.html#DynamicUser=) combined with [`Accept=yes`](https://www.freedesktop.org/software/systemd/man/systemd.socket.html#Accept=) start each process as a separate `uid`, with unrelated sandboxes.
This works as long as we don't need to pass state from a parent process (or alternatively, we now have to do that via FD socket passing etc).

Downsides: admin has to see internals, might run into version skew, need to connect over UNIX domain sockets instead of just socketpair & fork.


## Soft

### async or threaded, sandbox granularity

Do we isolate data in/out of each session into their own processes, or do it all in one?

One per user would sound nice, but it's hard to do because we want to support sharing shell sessions.

Especially if and when we start [running terminal emulators on the server](../roadmap.md#keep-terminal-state) lots of small sandboxes sounds like a good idea.

Threaded means we can seccomp & landlock individual threads.
It also makes the system more manageable to ops, e.g. allowing meaningful `strace` use.
Leaning that way.

Only the non-templated services (without the `@` suffix) should be handling a large number of connections or tasks *within one sandbox*, so those are the ones that would benefit from async.
We'll likely sandbox threadpools running async code, there.

Working & secure is better than optimally fast, and it's probably plenty fast anyway.
The cost of the actual shell session should be much higher than our overhead.


### Authentication is tied to WebSocket connection

Lose a TCP connection (except in HTTP/3 world), or change IP addresses, and you need to reauthenticate.
Simplifies things greatly, avoids yet another session token that can be stolen, but might not work out well in practise.


## Undecided
