//! Systemd style socket activation.
//!
//! # Resources
//!
//! - <https://www.freedesktop.org/software/systemd/man/sd_listen_fds.html>
//! - <https://www.freedesktop.org/software/systemd/man/systemd.socket.html>

use std::ffi::{OsStr, OsString};
use std::marker::PhantomData;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::io::RawFd;
use std::os::unix::prelude::{FromRawFd, OsStrExt};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EnvError {
    #[error("not present")]
    NotPresent,

    #[error("not valid Unicode: {0:?}")]
    NotUnicode(OsString),

    #[error(transparent)]
    Var(#[from] std::env::VarError),

    #[error(transparent)]
    NotInt(#[from] std::num::ParseIntError),
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid environment value: ${key}: {source}")]
    Env { key: String, source: EnvError },
}

#[cfg(test)]
#[derive(Debug)]
struct TestShim {
    start: RawFd,
    prefix: String,
}

#[derive(Debug)]
pub struct SocketActivation<'a> {
    _phantom: PhantomData<&'a ()>,
    unset_env: bool,
    #[cfg(test)]
    test: Option<TestShim>,
}

impl Default for SocketActivation<'_> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
            unset_env: true,
            #[cfg(test)]
            test: None,
        }
    }
}

impl<'a> SocketActivation<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    fn env_var_os<K: AsRef<OsStr>>(&self, key: K) -> Option<OsString> {
        #[cfg(test)]
        if let Some(config) = &self.test {
            let mut test_key = OsString::from(&config.prefix);
            test_key.push(key);
            // Lifetimes fight me if I try to combine the two `var` calls.
            return std::env::var_os(test_key);
        }
        std::env::var_os(key)
    }

    fn env_var<K: AsRef<OsStr>>(&self, key: K) -> Result<String, std::env::VarError> {
        #[cfg(test)]
        if let Some(config) = &self.test {
            let mut test_key = OsString::from(&config.prefix);
            test_key.push(key);
            // Lifetimes fight me if I try to combine the two `var` calls.
            return std::env::var(test_key);
        }
        std::env::var(key)
    }

    fn env_remove_var<K: AsRef<OsStr>>(&self, key: K) {
        #[cfg(test)]
        if let Some(config) = &self.test {
            let mut test_key = OsString::from(&config.prefix);
            test_key.push(key);
            // Lifetimes fight me if I try to combine the two `remove_var` calls.
            return std::env::remove_var(test_key);
        }
        std::env::remove_var(key)
    }

    #[cfg(test)]
    fn test(&mut self, start: RawFd, prefix: String) -> &mut Self {
        assert!(
            self.test.is_none(),
            "cannot call SocketActivation::test twice"
        );
        self.test = Some(TestShim { start, prefix });
        self
    }

    fn empty(self) -> FdIter<'a> {
        FdIter {
            activation: self,
            next: -1,
            count: 0,
            names: vec![],
            name_offset: 0,
        }
    }

    fn get_start(&self) -> RawFd {
        #[cfg(test)]
        if let Some(config) = &self.test {
            return config.start;
        }
        const LISTEN_FDS_START: RawFd = 3;
        LISTEN_FDS_START
    }

    pub fn parse(self) -> Result<FdIter<'a>, Error> {
        let listen_pid = self.env_var("LISTEN_PID");
        let listen_fds = self.env_var("LISTEN_FDS");
        let listen_fdnames = self.env_var_os("LISTEN_FDNAMES");

        if self.unset_env {
            // Done whether it's a success or not.
            self.env_remove_var("LISTEN_PID");
            self.env_remove_var("LISTEN_FDS");
            self.env_remove_var("LISTEN_FDNAMES");
        }

        let for_us = listen_pid
            .map_err(EnvError::from)
            .and_then(|s| s.parse::<u32>().map_err(EnvError::NotInt))
            .map(|pid| pid == std::process::id())
            .or_else(|error| match error {
                EnvError::NotPresent => Ok(true),
                _ => Err(error),
            })
            .map_err(|error| Error::Env {
                key: "LISTEN_PID".to_string(),
                source: error,
            })?;
        if !for_us {
            // Not for us.
            return Ok(self.empty());
        }

        let num_fds = listen_fds
            .map_err(EnvError::from)
            .and_then(|s| s.parse::<usize>().map_err(EnvError::NotInt))
            .map_err(|error| Error::Env {
                key: "LISTEN_FDS".to_string(),
                source: error,
            })?;

        let fd_names = listen_fdnames
            .map(|s| s.into_vec())
            // Treat `$LISTEN_FDNAMES` as an optional extension.
            .unwrap_or_default();

        let start = self.get_start();

        Ok(FdIter {
            activation: self,
            next: start,
            count: num_fds,
            names: fd_names,
            name_offset: 0,
        })
    }
}

#[derive(Debug)]
pub struct FdIter<'a> {
    activation: SocketActivation<'a>,
    next: RawFd,
    count: usize,
    names: Vec<u8>,
    name_offset: usize,
}

impl<'a> Iterator for FdIter<'a> {
    type Item = FileDescriptor;

    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        // RUST-WART I'm reimplementing string handling just because I don't want to worry about UTF-8 decoding.
        // https://github.com/rust-lang/rfcs/issues/900
        let rest = &self.names[self.name_offset..];
        let colon = rest
            .iter()
            .enumerate()
            .filter(|(_i, v)| **v == b':')
            .map(|(i, _v)| i)
            .next()
            .unwrap_or(rest.len());
        let name = OsStr::from_bytes(&rest[..colon]);
        let name = if name.is_empty() {
            None
        } else {
            // Lifetimes are hurting me again, wanted to use `&OsStr` here.
            Some(name.to_os_string())
        };
        // If there's no colon at end, make sure to not index past the slice.
        self.name_offset += std::cmp::min(colon + 1, self.names.len());
        let fd = self.next;
        // TODO Verify the FD is actually open.
        // Use `fcntl(2)` `F_GETFL`.
        self.count -= 1;
        self.next += 1;
        Some(FileDescriptor { name_: name, fd })
    }
}

#[derive(Debug)]
pub struct FileDescriptor {
    name_: Option<OsString>,
    fd: RawFd,
}

impl<'a> FileDescriptor {
    pub fn name(&'a self) -> Option<&'a OsStr> {
        match &self.name_ {
            Some(r) => Some(&r),
            None => None,
        }
    }

    pub fn take_fd<T: FromRawFd>(self) -> T {
        // TODO test type first?
        unsafe { T::from_raw_fd(self.fd) }
    }
}

#[cfg(test)]
mod tests {
    use memfd;
    use std::ffi::{OsStr, OsString};
    use std::os::unix::io::AsRawFd;
    use std::os::unix::prelude::{FromRawFd, IntoRawFd, MetadataExt, RawFd};
    use std::sync::atomic;

    use crate::ipc::ownedfd::OwnedFd;

    use super::SocketActivation;

    struct EnvPrefix {
        prefix: String,
    }

    impl EnvPrefix {
        pub fn new() -> Self {
            static SEQUENCE: atomic::AtomicU64 = atomic::AtomicU64::new(1);
            let seq = SEQUENCE.fetch_add(1, atomic::Ordering::SeqCst);
            let prefix = format!("TEST_{}_", seq);
            Self { prefix }
        }

        pub fn set_var<K: AsRef<OsStr>, V: AsRef<OsStr>>(&mut self, k: K, v: V) {
            let mut key = OsString::from(&self.prefix);
            key.push(k);
            std::env::set_var(key, v);
        }
    }

    struct FdBlock {
        fds: Vec<OwnedFd>,
    }

    impl FdBlock {
        pub fn start(&self) -> RawFd {
            self.fds[0].as_raw_fd()
        }
    }

    // Duplicate FD at or after `offset`, without closing any existing FDs.
    fn dupfd_at<F: AsRawFd>(fd: &F, offset: usize) -> Result<OwnedFd, std::io::Error> {
        let ret = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, offset) };
        if ret < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let new = unsafe { OwnedFd::from_raw_fd(ret) };
        Ok(new)
    }

    /// Duplicate FDs as a dense array at some offset.
    ///
    ///
    fn dup_pack_fds<F>(fds: &[F]) -> Result<FdBlock, std::io::Error>
    where
        F: AsRawFd,
    {
        // Attempt to prepare a dense sequence of FDs by simply picking a unique offset that doesn't overlap with other test use and gracefully handle stumbling on pre-existing FDs.
        // Since `F_DUPFD_CLOEXEC` just finds the first unused FD, tests can accidentally create an FD in a range reserved for another test, but this is handled gracefully exactly the same way as other pre-existing FDs.
        // We assume tests won't hit any FD limits, and never reuse any FD numbers.
        // This is far from ideal but given the limits of UNIX APIs and libraries owning FDs, there's not much more to be done.
        static OFFSET: atomic::AtomicUsize = atomic::AtomicUsize::new(100);
        'top: loop {
            let offset = OFFSET.fetch_add(fds.len(), atomic::Ordering::SeqCst);
            let mut packed = Vec::new();
            for (idx, fd) in fds.iter().enumerate() {
                let fd_num = offset + idx;
                let new = dupfd_at(fd, fd_num)?;
                if new.as_raw_fd() as usize != fd_num {
                    // The FD was already in use, retry.
                    continue 'top;
                }
                packed.push(new);
            }
            return Ok(FdBlock { fds: packed });
        }
    }

    #[test]
    fn simple() {
        let fd_factory = memfd::MemfdOptions::new().close_on_exec(true);
        let fd = fd_factory.create("test").expect("memfd_create").into_file();
        let want_metadata = fd.metadata().expect("memfd metadata");
        let mut env = EnvPrefix::new();
        env.set_var("LISTEN_PID", format!("{}", std::process::id()));
        env.set_var("LISTEN_FDS", "1");
        env.set_var("LISTEN_FDNAMES", "xyzzy");
        let mut activation = SocketActivation::new();
        activation.test(fd.into_raw_fd(), env.prefix);

        let mut iter = activation.parse().expect("parse");
        let filedesc = iter.next().expect("must receive FD");
        assert!(matches!(iter.next(), None));
        assert_eq!(filedesc.name(), Some(OsStr::new("xyzzy")));
        let got_file: std::fs::File = filedesc.take_fd();
        let got_metadata = got_file.metadata().expect("received fd metadata");
        assert_eq!(
            (want_metadata.dev(), want_metadata.ino()),
            (got_metadata.dev(), got_metadata.ino()),
        );
    }

    #[test]
    fn two() {
        let fd_factory = memfd::MemfdOptions::new().close_on_exec(true);
        let one = fd_factory.create("one").expect("memfd_create").into_file();
        let two = fd_factory.create("two").expect("memfd_create").into_file();
        let want_one_metadata = one.metadata().expect("memfd metadata");
        let want_two_metadata = two.metadata().expect("memfd metadata");
        let fd_block = dup_pack_fds(&[one, two]).expect("pack_fds");
        let mut env = EnvPrefix::new();
        env.set_var("LISTEN_PID", format!("{}", std::process::id()));
        env.set_var("LISTEN_FDS", "2");
        env.set_var("LISTEN_FDNAMES", "xyzzy:thud");
        let mut activation = SocketActivation::new();
        activation.test(fd_block.start(), env.prefix);

        let mut iter = activation.parse().expect("parse");
        let got_one = iter.next().expect("must receive FD");
        assert_eq!(got_one.name(), Some(OsStr::new("xyzzy")));
        let got_two = iter.next().expect("must receive FD");
        assert_eq!(got_two.name(), Some(OsStr::new("thud")));
        assert!(matches!(iter.next(), None));

        let got_one_file: std::fs::File = got_one.take_fd();
        assert_eq!(got_one_file.as_raw_fd(), fd_block.start());
        let got_one_metadata = got_one_file.metadata().expect("received fd metadata");
        assert_eq!(
            (want_one_metadata.dev(), want_one_metadata.ino()),
            (got_one_metadata.dev(), got_one_metadata.ino()),
        );

        let got_two_file: std::fs::File = got_two.take_fd();
        assert_eq!(got_two_file.as_raw_fd(), fd_block.start() + 1);
        let got_two_metadata = got_two_file.metadata().expect("received fd metadata");
        assert_eq!(
            (want_two_metadata.dev(), want_two_metadata.ino()),
            (got_two_metadata.dev(), got_two_metadata.ino()),
        );
    }

    #[test]
    fn not_our_pid() {
        let fd_factory = memfd::MemfdOptions::new().close_on_exec(true);
        let fd = fd_factory.create("test").expect("memfd_create").into_file();
        let mut env = EnvPrefix::new();
        // Base this on our PID so it can't accidentally be correct.
        let other_pid = std::process::id() + 1;
        env.set_var("LISTEN_PID", format!("{}", other_pid));
        env.set_var("LISTEN_FDS", "1");
        env.set_var("LISTEN_FDNAMES", "xyzzy");
        let mut activation = SocketActivation::new();
        activation.test(fd.into_raw_fd(), env.prefix);

        let mut iter = activation.parse().expect("parse");
        assert!(matches!(iter.next(), None));
    }

    #[test]
    fn empty_name() {
        let fd_factory = memfd::MemfdOptions::new().close_on_exec(true);
        let fd = fd_factory.create("test").expect("memfd_create").into_file();
        let want_metadata = fd.metadata().expect("memfd metadata");
        let mut env = EnvPrefix::new();
        env.set_var("LISTEN_PID", format!("{}", std::process::id()));
        env.set_var("LISTEN_FDS", "1");
        env.set_var("LISTEN_FDNAMES", "");
        let mut activation = SocketActivation::new();
        activation.test(fd.into_raw_fd(), env.prefix);

        let mut iter = activation.parse().expect("parse");
        let filedesc = iter.next().expect("must receive FD");
        assert!(matches!(iter.next(), None));
        assert_eq!(filedesc.name(), None);
        let got_file: std::fs::File = filedesc.take_fd();
        let got_metadata = got_file.metadata().expect("received fd metadata");
        assert_eq!(
            (want_metadata.dev(), want_metadata.ino()),
            (got_metadata.dev(), got_metadata.ino()),
        );
    }

    #[test]
    fn unset_name() {
        let fd_factory = memfd::MemfdOptions::new().close_on_exec(true);
        let fd = fd_factory.create("test").expect("memfd_create").into_file();
        let want_metadata = fd.metadata().expect("memfd metadata");
        let mut env = EnvPrefix::new();
        env.set_var("LISTEN_PID", format!("{}", std::process::id()));
        env.set_var("LISTEN_FDS", "1");
        // Not setting `LISTEN_FDNAMES`.
        let mut activation = SocketActivation::new();
        activation.test(fd.into_raw_fd(), env.prefix);

        let mut iter = activation.parse().expect("parse");
        let filedesc = iter.next().expect("must receive FD");
        assert!(matches!(iter.next(), None));
        assert_eq!(filedesc.name(), None);
        let got_file: std::fs::File = filedesc.take_fd();
        let got_metadata = got_file.metadata().expect("received fd metadata");
        assert_eq!(
            (want_metadata.dev(), want_metadata.ino()),
            (got_metadata.dev(), got_metadata.ino()),
        );
    }

    #[test]
    fn names_short() {
        let fd_factory = memfd::MemfdOptions::new().close_on_exec(true);
        let one = fd_factory.create("one").expect("memfd_create").into_file();
        let two = fd_factory.create("two").expect("memfd_create").into_file();
        let fd_block = dup_pack_fds(&[one, two]).expect("pack_fds");
        let mut env = EnvPrefix::new();
        env.set_var("LISTEN_PID", format!("{}", std::process::id()));
        env.set_var("LISTEN_FDS", "2");
        env.set_var("LISTEN_FDNAMES", "xyzzy");
        let mut activation = SocketActivation::new();
        activation.test(fd_block.start(), env.prefix);

        let mut iter = activation.parse().expect("parse");
        let got_one = iter.next().expect("must receive FD");
        assert_eq!(got_one.name(), Some(OsStr::new("xyzzy")));
        let got_two = iter.next().expect("must receive FD");
        assert_eq!(got_two.name(), None);
        assert!(matches!(iter.next(), None));
    }
}
