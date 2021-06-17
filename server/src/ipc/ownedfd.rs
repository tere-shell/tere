use std::fs::File;
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::os::unix::net::{UnixDatagram, UnixStream};

#[derive(Debug)]
pub struct OwnedFd(RawFd);

impl FromRawFd for OwnedFd {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self(fd)
    }
}

impl AsRawFd for OwnedFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl IntoRawFd for OwnedFd {
    fn into_raw_fd(self) -> RawFd {
        let raw_fd = self.0;
        mem::forget(self);
        raw_fd
    }
}

impl Drop for OwnedFd {
    fn drop(&mut self) {
        let _ = unsafe { libc::close(self.0) };
    }
}

impl From<File> for OwnedFd {
    fn from(f: File) -> Self {
        let fd = f.into_raw_fd();
        Self(fd)
    }
}

impl From<OwnedFd> for File {
    fn from(f: OwnedFd) -> Self {
        let fd = f.into_raw_fd();
        unsafe { File::from_raw_fd(fd) }
    }
}

impl From<UnixStream> for OwnedFd {
    fn from(f: UnixStream) -> Self {
        let fd = f.into_raw_fd();
        Self(fd)
    }
}

impl From<UnixDatagram> for OwnedFd {
    fn from(f: UnixDatagram) -> Self {
        let fd = f.into_raw_fd();
        Self(fd)
    }
}

impl From<OwnedFd> for UnixStream {
    fn from(f: OwnedFd) -> Self {
        let fd = f.into_raw_fd();
        unsafe { UnixStream::from_raw_fd(fd) }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Read;
    use std::os::unix::fs::FileExt;

    use super::*;

    fn tempfile() -> File {
        let opts = memfd::MemfdOptions::new().close_on_exec(true);
        let m = opts.create("test").expect("memfd_create");
        m.into_file()
    }

    #[test]
    fn into_raw_fd_does_not_close() {
        let owned = {
            let file = tempfile();
            // Use write_at variant so cursor stays at 0 for later consumption.
            file.write_all_at(b"hello, world", 0)
                .expect("write to memfd");
            let fd = file.into_raw_fd();
            let owned = unsafe { OwnedFd::from_raw_fd(fd) };
            owned
        };
        // this must NOT close the FD
        let raw_fd = owned.into_raw_fd();
        // prove it by reading back the contents
        let mut file = unsafe { File::from_raw_fd(raw_fd) };
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).expect("memfd read");
        assert_eq!(buf, b"hello, world");
    }

    #[test]
    fn roundtrip_file() {
        let one = tempfile();
        // Use write_at variant so cursor stays at 0 for later consumption.
        one.write_all_at(b"hello, world", 0)
            .expect("write to memfd");
        let two = OwnedFd::from(one);
        let mut three = File::from(two);
        let mut buf = Vec::new();
        three.read_to_end(&mut buf).expect("memfd read");
        assert_eq!(buf, b"hello, world");
    }
}
