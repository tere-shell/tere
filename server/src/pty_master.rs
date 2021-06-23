use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

#[derive(Debug)]
pub struct PtyMaster(RawFd);

impl FromRawFd for PtyMaster {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        debug_assert!(fd >= 0);
        Self(fd)
    }
}

impl AsRawFd for PtyMaster {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl IntoRawFd for PtyMaster {
    fn into_raw_fd(self) -> RawFd {
        let raw_fd = self.0;
        std::mem::forget(self);
        raw_fd
    }
}

impl Drop for PtyMaster {
    fn drop(&mut self) {
        let _ = unsafe { libc::close(self.0) };
    }
}

impl std::io::Read for &PtyMaster {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let c_buf = buf.as_mut_ptr() as *mut libc::c_void;
        let c_len = buf.len() as libc::size_t;
        let ret = unsafe { libc::read(self.0, c_buf, c_len) };

        if ret < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let size = ret as usize;
        Ok(size)
    }
}

impl std::io::Write for &PtyMaster {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let c_buf = buf.as_ptr() as *const libc::c_void;
        let c_len = buf.len() as libc::size_t;
        let ret = unsafe { libc::write(self.0, c_buf, c_len) };

        if ret < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let size = ret as usize;
        Ok(size)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// TODO get/set terminal size
