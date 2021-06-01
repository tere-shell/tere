# Inter-Process Communication (IPC)

If we do [privilege separation](privsep.md), we're going to have multiple processes that need to interact.
If we're parsing websocket messages in one process, but keeping the PTY fds safe in another, that means we're doing some form of IPC for every chunk of input & output the user experiences; likely more than once.
We need an IPC mechanism that won't become the bottleneck.
We also want to have one that can pass file descriptors, which eliminates most common RPC mechanisms, that are aimed at networking.
Let's explore our options.



## Open question: Broadcasting shell session content

### Ringbuffers in shared memory

If the mechanism natively supports multiple consumers seeing the same data, how do we kick out slow consumers?

### Just send data on UNIX domain socket

Now we need a broadcast pub/sub mechanism, either on the sender side or receiver.


## Design sketch: Circuits

`circuits` is a (TODO to-be-written) module/crate providing a simple mechanism for transporting multiple logical circuits over one (framed, message-based) stream.
If a transport is not natively framed, length-prefixed messages are an easy solution for framing messages.

Circuits are comparable to remote procedure calls where each request contains a request ID and responses to refer to those, except each circuit can operate its own multi-stage protocol and can stream data.

Since tungstenite, the websocket library used here, assumes that messages fit in memory, and on the other end bincode/other serde mechanisms often make the same assumption, the circuits API will likely assume that too; but this is not an inherent limitation of the design.

Each transport carries `max_circuits` circuits, defined by the protocol logic being generic over a suitable integer type.
All circuits are "virtually open" at all times, there are no open/close messages.
Circuits are "idle" if they are available for use, or "busy" if they are currently in use.

For a communicating pair of endpoints, previously agreed upon circuit IDs may be reserved and used for special purposes.
All other circuits are interchangeable when idle.

Circuits *must* support messages bearing ancillary data, such as UNIX domain socket file descriptor passing, when the transport is capable of such.

Backpressure may or may not be implemented, to be discovered later.
