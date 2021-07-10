//! IPC with FD passing over UNIX domain sockets of type `SOCK_SEQPACKET`.
//!
//! Rust does not currently have a type for `SOCK_SEQPACKET`.
//! Due to similarities with `SOCK_DATAGRAM` -- mostly that messages must be received whole -- we're using [`UnixDatagram`] as the underlying type.
//! However, the sockets are of type `SOCK_SEQPACKET` and only work via connections.

// TODO SOCK_SEQPACKET means messages map 1:1 to syscalls.
// That's fine for now, and unavoidable for protocols that pass FDs in practically every message, but at some point we're probably going to want to decrease syscall overhead with a SOCK_STREAM alternative that buffers messages.
// With that, received FDs belong to the last message in the buffer, and recvmsg will give a short read on the trailing boundary of any message with FDs so there will never be two in the buffer at the same time.
// At that time, Message::MAX_SIZE and Message:MAX_FDS handling need to switch from current message to max of all possible messages.
// Or, we just avoid the whole syscall overhead issue with io_uring.

use bincode::Options;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::VecDeque;
use std::convert::TryFrom;
use std::io::{IoSlice, IoSliceMut};
use std::os::unix::io::FromRawFd;
use std::os::unix::io::RawFd;
use std::os::unix::net::UnixDatagram;
use std::os::unix::net::{AncillaryData, AncillaryError};
use thiserror::Error;

// Using unstable feature `unix_socket_ancillary_data`.
// https://github.com/rust-lang/rust/issues/76915
use std::os::unix::io::AsRawFd;
use std::os::unix::net::SocketAncillary;

use crate::ipc;
use crate::ipc::ownedfd::OwnedFd;

/// Create a pair of packet-oriented (`SOCK_SEQPACKET`) sockets that are connected to each others, using `socketpair(2)`.
///
/// Like [std::os::unix::net::UnixStream::pair], except using `SOCK_SEQPACKET`.
pub fn pair() -> std::io::Result<(UnixDatagram, UnixDatagram)> {
    let mut fds = [0, 0];
    let ret = unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            &mut fds[0],
        )
    };
    if ret < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let a = unsafe { UnixDatagram::from_raw_fd(fds[0]) };
    let b = unsafe { UnixDatagram::from_raw_fd(fds[1]) };
    Ok((a, b))
}

/// Implement the IPC abstraction for UNIX domain `SOCK_SEQPACKET` sockets.
pub struct SeqPacket {
    socket: UnixDatagram,
}

#[derive(Error, Debug)]
pub enum SocketConversionError {
    #[error("not a SOCK_SEQPACKET")]
    NotSeqPacket,
}

fn is_seq_packet(socket: &impl AsRawFd) -> bool {
    let fd = socket.as_raw_fd();
    let mut socket_type: libc::c_int = 0;
    let mut len = std::mem::size_of_val(&socket_type) as libc::socklen_t;
    let ret = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            &mut socket_type as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if ret < 0 {
        // we don't really care why it failed
        return false;
    }
    socket_type == libc::SOCK_SEQPACKET
}

/// The socket must be of `SOCK_SEQPACKET`, and connected.
impl TryFrom<UnixDatagram> for SeqPacket {
    type Error = SocketConversionError;

    fn try_from(socket: UnixDatagram) -> Result<Self, Self::Error> {
        if !is_seq_packet(&socket) {
            return Err(SocketConversionError::NotSeqPacket);
        }
        Ok(Self { socket })
    }
}

impl ipc::IPC for SeqPacket {
    fn send_with_fds<M>(&self, message: &M) -> Result<(), ipc::SendError>
    where
        M: ipc::Message + Serialize,
    {
        // It would be nicer if we could just return a SocketAncillary and let caller deal with it.
        // Unfortunately, SocketAncillary borrows the buffer, so that doesn't work.

        let config = bincode::DefaultOptions::new()
            // MUST use with_no_limit or fds are serialized twice
            .with_no_limit();

        let mut fds: Vec<RawFd> = Vec::new();
        let mut encoded = Vec::with_capacity(M::MAX_SIZE);
        ipc::passfd::gather_fds_to_vec(&mut fds, || {
            config
                .serialize_into(&mut encoded, &message)
                .map_err(ipc::SendError::Serialize)
        })?;

        // TODO rust doesn't (yet?) expose CMSG_SPACE, forcing us to either guess or probe the size.
        // TODO just call libc::CMSG_SPACE?
        // TODO this doesn't align right? https://github.com/rust-lang/rust/issues/76915#issuecomment-855368476
        let mut ancillary_buffer = Vec::with_capacity(128);
        let mut ancillary = {
            loop {
                ancillary_buffer.resize_with(ancillary_buffer.len() + 128, Default::default);
                let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
                if ancillary.add_fds(&fds) {
                    break ancillary;
                }
            }
        };
        println!("send ancillary: {:?}", ancillary);

        let iovec = &mut [IoSlice::new(&encoded[..])][..];
        self.socket
            .send_vectored_with_ancillary(iovec, &mut ancillary)
            .map_err(ipc::SendError::Socket)?;

        Ok(())
    }

    fn receive_with_fds<M>(&self) -> Result<M, ipc::ReceiveError>
    where
        M: ipc::Message + DeserializeOwned,
    {
        // TODO in debug mode, validate against max_message_size on send

        let config = bincode::DefaultOptions::new()
            // MUST use with_no_limit or fds are serialized twice
            .with_no_limit();

        let mut encoded = vec![0_u8; M::MAX_SIZE];
        let iovec = &mut [IoSliceMut::new(&mut encoded)][..];

        let inner_size = (std::mem::size_of::<libc::c_int>() * M::MAX_FDS) as libc::c_uint;
        let ancillary_size = unsafe { libc::CMSG_SPACE(inner_size) } as usize;
        // TODO alignment is wrong, https://github.com/rust-lang/rust/issues/76915
        let mut ancillary_buffer = vec![0_u8; ancillary_size];
        let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);

        let (size, truncated) = self
            .socket
            .recv_vectored_with_ancillary(iovec, &mut ancillary)
            .map_err(ipc::ReceiveError::Socket)?;
        if truncated {
            // If we had flags|=MSG_TRUNC, we could report the sent size.
            return Err(ipc::ReceiveError::TooLarge);
        }
        let encoded = &encoded[..size];

        let mut fds: VecDeque<OwnedFd> = ancillary
            .messages()
            .filter_map(|r| match r {
                // TODO should we fail on ancillary data errors?
                // If choosing to fail, make sure to collect all the fds so we close them.

                // Definitely ignore AncillaryError::Unknown.
                Err(AncillaryError::Unknown { .. }) => None,
                Err(_) => None,
                Ok(AncillaryData::ScmRights(rights)) => Some(rights),
                Ok(_) => None,
            })
            .flatten()
            // We've been handed new, open, FDs by the kernel.
            // Ensure they get closed on error paths by moving them into something that takes ownership.
            .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
            .collect();

        if ancillary.truncated() {
            // Do this after we collect the FDs so we don't leak them.
            return Err(ipc::ReceiveError::AncillaryTruncated {
                max_fds: M::MAX_FDS,
                bytes_cap: ancillary.capacity(),
            });
        }

        let orig_num_fds = fds.len();
        let msg = ipc::passfd::scatter_fds_from_vec_deque(
            &mut fds,
            || -> Result<M, ipc::ReceiveError> {
                config
                    .deserialize(encoded)
                    .map_err(ipc::ReceiveError::Deserialize)
            },
        )?;
        let fds_left = fds.len();
        if fds_left != 0 {
            return Err(ipc::ReceiveError::TooManyFds {
                orig: orig_num_fds,
                extra: fds_left,
            });
        }
        Ok(msg)
    }

    fn shutdown(&self, how: std::net::Shutdown) -> Result<(), ipc::ShutdownError> {
        self.socket
            .shutdown(how)
            .map_err(|source| ipc::ShutdownError::Io { how, source })
    }
}

#[cfg(test)]
mod tests {
    use memfd;
    use serde::{Deserialize, Serialize};
    use std::convert::TryFrom;
    use std::fs::File;
    use std::io::Read;
    use std::os::unix::fs::FileExt;
    use std::os::unix::io::{AsRawFd, RawFd};

    use super::SeqPacket;
    use crate::ipc;
    use crate::ipc::IPC;
    // RUST-WART https://github.com/rust-lang/rust/issues/29036
    // use super as seqpacket;
    use super::super::seqpacket;

    #[derive(Serialize, Deserialize, Debug)]
    struct DummyMessage {
        greeting: String,
        #[serde(with = "ipc::passfd")]
        one: File,
        #[serde(with = "ipc::passfd")]
        two: File,
    }

    impl ipc::Message for DummyMessage {
        const MAX_SIZE: usize = 4000;
        const MAX_FDS: usize = 3;
    }

    fn fds_are_unique(mut fds: Vec<RawFd>) -> bool {
        let orig_len = fds.len();
        fds.dedup();
        fds.len() == orig_len
    }

    #[test]
    fn roundtrip() {
        let opts = memfd::MemfdOptions::new().close_on_exec(true);
        let send_one = opts.create("one").expect("memfd_create").into_file();
        // Use write_at variant so cursor stays at 0 for later consumption.
        send_one.write_all_at(b"one", 0).expect("write to memfd");
        let send_two = opts.create("two").expect("memfd_create").into_file();
        send_two.write_all_at(b"two", 0).expect("write to memfd");

        let msg = DummyMessage {
            greeting: "Hello, world\n".to_string(),
            one: send_one,
            two: send_two,
        };

        let (sender, receiver) = seqpacket::pair().expect("socketpair");
        let sender = SeqPacket::try_from(sender).expect("SeqPacket::try_from");
        // Assume the socket buffer is large enough that we can first send, and only then receive.

        println!("send: {:?}", msg);
        sender.send_with_fds(&msg).expect("sendmsg");

        let receiver = SeqPacket::try_from(receiver).expect("SeqPacket::try_from");
        let mut got: DummyMessage = receiver.receive_with_fds().expect("recvmsg");
        println!("receive: {:?}", got);
        assert_eq!(msg.greeting, got.greeting);
        assert!(got.one.as_raw_fd() >= 0);
        assert!(got.two.as_raw_fd() >= 0);
        assert!(fds_are_unique(vec![
            msg.one.as_raw_fd(),
            msg.two.as_raw_fd(),
            got.one.as_raw_fd(),
            got.two.as_raw_fd(),
        ]));

        let mut got_one_str = String::new();
        got.one
            .read_to_string(&mut got_one_str)
            .expect("read from memfd");
        assert_eq!(got_one_str, "one");

        let mut got_two_str = String::new();
        got.two
            .read_to_string(&mut got_two_str)
            .expect("read from memfd");
        assert_eq!(got_two_str, "two");
    }
}
