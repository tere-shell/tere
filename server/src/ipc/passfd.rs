//! Serde helpers for FD passing.
//!
//! See [ipc](super) for documentation.

// This module gently abuses thread-local state to pass context to passfd::{serialize,deserialize}.
// Technically, serde's DeserializeSeed would be enough for deserialize, but we'd have to re-implement #[derive(Deserialize)] and the result isn't pretty.
// And that doesn't solve Serialize, anyway.
//
// (The `serde_state` crate tried to provide both, and ended up as a largely-outdated fork of `serde`, only used by its own author, with no clear plan on merging back.)
//
// See also <https://github.com/serde-rs/serde/issues/881>.

use scopeguard::guard;
use serde::{Deserializer, Serializer};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use super::ownedfd::OwnedFd;

thread_local! {
    static OOB_SER: RefCell<Option<Vec<RawFd>>> = RefCell::new(None);
}

/// Gather FDs from [serde] serializing done by `ser`.
///
/// The FDs returned are not owned and must not be closed.
///
/// Use `#[serde(with="ipc::passfd")]` on the fields containing FDs.
///
/// `out` is an out parameter to allow caller to preallocate a buffer, and to return `ser`'s return value more naturally.
///
/// Panics in debug builds if called from inside a gather call.
pub fn gather_fds_to_vec<S, R>(out: &mut Vec<RawFd>, ser: S) -> R
where
    S: FnOnce() -> R,
{
    OOB_SER.with(|r| {
        let mut opt = r.borrow_mut();
        let old = opt.replace(std::mem::take(out));
        debug_assert!(old.is_none(), "gather_fds_to_vec called from inside gather");
    });
    let _guard = guard((), |_| {
        OOB_SER.with(|r| {
            // Hand over whatever has been collected, error or not.
            // They're just AsRawFd results, so caller dropping them doesn't matter.
            let mut opt = r.borrow_mut();
            if let Some(gathered) = opt.take() {
                // Discard old value, it's just the Default vec we put in earlier with std::mem::take.
                let _ = std::mem::replace(out, gathered);
            }
        });
    });

    ser()
}

pub fn serialize<F, S>(f: &F, serializer: S) -> Result<S::Ok, S::Error>
where
    F: AsRawFd,
    S: Serializer,
{
    OOB_SER.with(|r| {
        let mut borrow = r.borrow_mut();
        let oob = borrow.as_mut().ok_or_else(|| {
            serde::ser::Error::custom("PassFd can only be serialized via passfd::gather_*")
        })?;
        oob.push(f.as_raw_fd());
        Ok(())
    })?;
    serializer.serialize_unit()
}

thread_local! {
    static OOB_DE: RefCell<Option<VecDeque<OwnedFd>>> = RefCell::new(None);
}

/// Scatter FDs for [serde] deserializing done by `de`.
///
/// Use `#[serde(with="ipc::passfd")]` on the fields containing FDs.
///
/// `fds` may not be fully consumed, caller may want to consider that an error scenario.
///
/// Panics in debug builds if called from inside a scatter call.
pub fn scatter_fds_from_vec_deque<D, R>(fds: &mut VecDeque<OwnedFd>, de: D) -> R
where
    D: FnOnce() -> R,
{
    // This could have been written to take Vec<F: IntoRawFd> or Iterator<Item=IntoRawFd> and not force OwnedFd on the caller, but:
    //
    // OOB_DE isn't going to be generic for F, so we'd need to copy the items out to a Vec<OwnedFd>; can't use e.g. a FnMut to pop items because OOB_DE requires owned data or 'static and the closure would have a borrow in it.
    //
    // IntoRawFd would be tricky to use correctly, as RawFd implements it too.
    // RawFd as IntoRawFd would:
    //
    // - leak FDs on error, if that was the only reference to the FD (could be more cautious and only peek at the head of the line before we're ready to take ownership? that would require Peekable.)
    // - lead to two values unsafely owning the same FD, if it came from AsRawFd
    //
    // Later, see if requiring a 'static lifetime is enough, though that will do nothing for the RawFd trap.

    OOB_DE.with(|r| {
        let mut opt = r.borrow_mut();
        let old = opt.replace(std::mem::take(fds));
        debug_assert!(
            old.is_none(),
            "scatter_fds_from_vec_deque called from inside scatter"
        );
    });
    let _guard = guard((), |_| {
        OOB_DE.with(|r| {
            // Hand back ownership of whatever was left unprocessed, error or not.
            let mut opt = r.borrow_mut();
            if let Some(left) = opt.take() {
                // Discard old value, it's just the Default vec we put in earlier with std::mem::take.
                let _ = std::mem::replace(fds, left);
            }
        });
    });

    de()
}

pub fn deserialize<'de, F, D>(deserializer: D) -> Result<F, D::Error>
where
    D: Deserializer<'de>,
    F: FromRawFd,
{
    // Do this just in case the deserializer cares.
    // Bincode does not serialize unit values at all, so it shouldn't.
    // Still, some edge case might mean the deserializer wants to report an error here.
    deserializer.deserialize_unit(serde::de::IgnoredAny)?;
    let file = OOB_DE.with(|r| {
        let mut borrow = r.borrow_mut();
        let oob = borrow.as_mut().ok_or_else(|| {
            serde::de::Error::custom("PassFd can only be deserialized via receive_with_fds")
        })?;
        oob.pop_front()
            .ok_or_else(|| serde::de::Error::custom("received too few FDs"))
    })?;
    // F may or may not be a file, roundtrip via RawFd.
    let fd = file.into_raw_fd();
    let f = unsafe { F::from_raw_fd(fd) };
    Ok(f)
}
