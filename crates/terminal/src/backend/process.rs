//! OS ownership boundary. Only processes in the newly created PTY session/job are signalled.
use crate::{Result, TerminalError, TerminalErrorKind};
use portable_pty::Child;

#[cfg(unix)]
pub(crate) struct ProcessScope {
    session: libc::pid_t,
    armed: std::sync::atomic::AtomicBool,
}
#[cfg(unix)]
impl ProcessScope {
    pub(crate) fn new(child: &dyn Child) -> Result<Self> {
        let pid = child.process_id().ok_or_else(|| {
            TerminalError::new(
                TerminalErrorKind::ProcessFailed,
                "PTY child has no process ID",
            )
        })? as libc::pid_t;
        // SAFETY: getsid only reads kernel process metadata. Reject the app's own session.
        let session = unsafe { libc::getsid(pid) };
        // SAFETY: getsid(0) queries this process and has no side effects.
        let own = unsafe { libc::getsid(0) };
        if session != pid || session <= 1 || session == own {
            return Err(TerminalError::new(
                TerminalErrorKind::ProcessFailed,
                "PTY child did not create an isolated process session",
            ));
        }
        Ok(Self {
            session,
            armed: std::sync::atomic::AtomicBool::new(true),
        })
    }
    fn members(&self) -> Result<Vec<libc::pid_t>> {
        let pids = process_ids()?;
        Ok(pids
            .into_iter()
            .filter(|pid| {
                // SAFETY: querying a PID does not access process memory. A disappeared PID returns -1.
                *pid > 1 && unsafe { libc::getsid(*pid) } == self.session
            })
            .collect())
    }
    pub(crate) fn is_empty(&self) -> Result<bool> {
        let empty = self.members()?.is_empty();
        if empty {
            self.armed
                .store(false, std::sync::atomic::Ordering::Release);
        }
        Ok(empty)
    }
    pub(crate) fn signal(&self, force: bool) -> Result<()> {
        if !self.armed.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }
        let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
        let mut failure = None;
        for pid in self.members()? {
            // SAFETY: recheck membership immediately before signalling; never signal our session.
            if unsafe { libc::getsid(pid) } != self.session {
                continue;
            }
            // SAFETY: pid was obtained from this owned PTY session, and signal is a valid constant.
            if unsafe { libc::kill(pid, signal) } != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH)
                    && (force || error.raw_os_error() != Some(libc::EPERM))
                {
                    failure = Some(TerminalError::from(error));
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }
}
#[cfg(unix)]
impl Drop for ProcessScope {
    fn drop(&mut self) {
        let _ = self.signal(true);
    }
}
#[cfg(target_os = "linux")]
fn process_ids() -> Result<Vec<libc::pid_t>> {
    Ok(std::fs::read_dir("/proc")?
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .collect())
}
#[cfg(target_os = "macos")]
fn process_ids() -> Result<Vec<libc::pid_t>> {
    // SAFETY: null buffer/zero length asks libproc for the required PID count.
    let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    if count <= 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut capacity = count as usize;
    loop {
        let mut pids: Vec<libc::pid_t> = vec![0; capacity];
        let bytes =
            i32::try_from(pids.len() * std::mem::size_of::<libc::pid_t>()).map_err(|_| {
                TerminalError::new(
                    TerminalErrorKind::StopFailed,
                    "process list exceeds platform limit",
                )
            })?;
        // SAFETY: pids is a writable aligned buffer whose byte length is passed exactly.
        let written = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
        if written < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        if (written as usize) < capacity {
            pids.truncate(written as usize);
            return Ok(pids);
        }
        capacity = capacity.checked_mul(2).ok_or_else(|| {
            TerminalError::new(
                TerminalErrorKind::StopFailed,
                "process list capacity overflow",
            )
        })?;
    }
}
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn process_ids() -> Result<Vec<libc::pid_t>> {
    Err(TerminalError::new(
        TerminalErrorKind::Unsupported,
        "PTY session supervision is not implemented for this platform",
    ))
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::System::JobObjects::*;
    pub(crate) struct ProcessScope {
        job: OwnedHandle,
    }
    impl ProcessScope {
        pub(crate) fn new(child: &dyn Child) -> Result<Self> {
            // SAFETY: no security attributes or name; the returned handle is uniquely owned.
            let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if raw.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            // SAFETY: raw was returned by CreateJobObjectW and is closed exactly once by OwnedHandle.
            let job = unsafe { OwnedHandle::from_raw_handle(raw) };
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = Default::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            // SAFETY: the structure and length correspond to JobObjectExtendedLimitInformation.
            if unsafe {
                SetInformationJobObject(
                    job.as_raw_handle(),
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of_val(&info) as u32,
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            let process = child.as_raw_handle().ok_or_else(|| {
                TerminalError::new(
                    TerminalErrorKind::ProcessFailed,
                    "PTY child has no process handle",
                )
            })?;
            // SAFETY: process is borrowed from the live child; job is owned by this scope.
            if unsafe { AssignProcessToJobObject(job.as_raw_handle(), process) } == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(Self { job })
        }
        pub(crate) fn signal(&self, force: bool) -> Result<()> {
            // Graceful console input is sent through the PTY by the local driver.
            if !force {
                return Ok(());
            }
            // SAFETY: this job contains only the command spawned for this Run and its descendants.
            if unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) } == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(())
        }
        pub(crate) fn is_empty(&self) -> Result<bool> {
            let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = Default::default();
            // SAFETY: output buffer matches the requested accounting structure and remains alive.
            if unsafe {
                QueryInformationJobObject(
                    self.job.as_raw_handle(),
                    JobObjectBasicAccountingInformation,
                    (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                    std::mem::size_of_val(&info) as u32,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(info.ActiveProcesses == 0)
        }
    }
}
#[cfg(windows)]
pub(crate) use windows::ProcessScope;
