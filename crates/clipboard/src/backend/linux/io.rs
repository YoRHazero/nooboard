use crate::{Error, Result};
use std::{io, os::fd::RawFd, time::Duration};

pub(super) fn poll(fds: &[RawFd], timeout: Option<Duration>) -> Result<Vec<bool>> {
    let mut descriptors: Vec<libc::pollfd> = fds
        .iter()
        .map(|&fd| libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();
    let millis = timeout.map_or(-1, |v| v.as_millis().min(i32::MAX as u128) as i32);
    // SAFETY: descriptors is a valid writable array for the duration of poll.
    let result = unsafe { libc::poll(descriptors.as_mut_ptr(), descriptors.len() as _, millis) };
    if result < 0 {
        if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            return Ok(vec![false; fds.len()]);
        }
        return Err(Error::backend(
            "wait for clipboard descriptors",
            io::Error::last_os_error(),
        ));
    }
    if descriptors.iter().any(|d| d.revents & libc::POLLNVAL != 0) {
        return Err(Error::Stopped);
    }
    Ok(descriptors
        .iter()
        .map(|d| d.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0)
        .collect())
}
