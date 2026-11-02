use super::{
    Context, DriverEvent, Input, Program, driver_error,
    process::{ProcessScope, StopRequest},
};
use crate::*;
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
};
use tokio::task::JoinSet;

#[cfg(all(test, unix))]
mod tests;

struct Prepared {
    scope: ProcessScope,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    master: Box<dyn MasterPty + Send>,
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
}
fn command_builder(program: &Program) -> CommandBuilder {
    let mut builder = match program {
        Program::Shell => CommandBuilder::new_default_prog(),
        #[cfg(unix)]
        Program::Command(command) => {
            let mut builder = CommandBuilder::new("/bin/sh");
            // Finder-launched apps have a minimal PATH. Load the POSIX login
            // environment inside the same managed PTY before running the command.
            builder.arg("-l");
            builder.arg("-c");
            builder.arg(command);
            builder
        }
        #[cfg(windows)]
        Program::Command(command) => {
            let mut builder = CommandBuilder::new(
                std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into()),
            );
            builder.arg("/C");
            builder.arg(command);
            builder
        }
    };
    builder.env("TERM", "xterm-256color");
    builder
}
/// The graceful requests that end a session, in order. Both are always made,
/// so the order only decides which kind of program ends first.
fn stop_plan(program: &Program) -> [StopRequest; 2] {
    match program {
        // As closing a terminal window does: interactive shells save their history and exit
        // on SIGHUP, but ignore SIGTERM. SIGTERM then ends jobs that ignore SIGHUP, such as nohup.
        Program::Shell => [StopRequest::HangUp, StopRequest::Terminate],
        // As a process manager does, because some programs reload on SIGHUP. What survives
        // SIGTERM is typically an interactive shell the command started, which SIGHUP ends.
        Program::Command(_) => [StopRequest::Terminate, StopRequest::HangUp],
    }
}

fn prepare(program: &Program, size: Option<TerminalSize>) -> Result<Prepared> {
    let pair = portable_pty::native_pty_system()
        .openpty(size.map(pty_size).unwrap_or_default())
        .map_err(driver_error)?;
    let builder = command_builder(program);
    // Acquire all PTY handles before spawning, so allocation errors cannot orphan a child.
    let reader = pair.master.try_clone_reader().map_err(driver_error)?;
    let writer = pair.master.take_writer().map_err(driver_error)?;
    let mut child = pair.slave.spawn_command(builder).map_err(driver_error)?;
    drop(pair.slave);
    let scope = match ProcessScope::new(&*child) {
        Ok(scope) => scope,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    Ok(Prepared {
        scope,
        child,
        master: pair.master,
        reader,
        writer,
    })
}
fn pty_size(size: TerminalSize) -> PtySize {
    PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

pub(super) async fn run(program: Program, mut context: Context) -> Result<()> {
    let plan = stop_plan(&program);
    let size = *context.size.lock().unwrap();
    // Never discard a pending spawn: cancellation is applied to the child as soon as it is owned.
    let prepared = tokio::task::spawn_blocking(move || prepare(&program, size))
        .await
        .map_err(driver_error)??;
    let Prepared {
        scope,
        mut child,
        master,
        mut reader,
        writer,
    } = prepared;
    let master = Arc::new(Mutex::new(master));
    let writer = Arc::new(Mutex::new(writer));
    let output = context
        .output
        .clone()
        .expect("local process has an output sink");
    let capacity = context.options.transfer_buffer_bytes;
    let mut reading = tokio::task::spawn_blocking(move || -> Result<()> {
        let mut buffer = vec![0; capacity];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => return Ok(()),
                Ok(n) => output.push(&buffer[..n]),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                #[cfg(unix)]
                Err(error) if error.raw_os_error() == Some(libc::EIO) => return Ok(()),
                Err(error) => return Err(error.into()),
            }
        }
    });
    let mut waiting = tokio::task::spawn_blocking(move || child.wait());
    context.event(DriverEvent::Started {
        id: context.id,
        connection: None,
    });
    let mut writes = JoinSet::new();
    let mut read_result = None;
    let mut status = None;
    // Running: forward input until a stop is requested (a forced stop requests one too),
    // or until the session leader exits or its output fails.
    loop {
        tokio::select! {
            result = &mut waiting => {
                status = Some(result);
                break;
            }
            result = &mut reading, if read_result.is_none() => {
                let result = result.map_err(driver_error).and_then(|r| r);
                let failed = result.is_err();
                read_result = Some(result);
                if failed {
                    context.stop.cancel();
                    break;
                }
            }
            _ = context.stop.cancelled() => break,
            input = context.input.recv(), if writes.is_empty() => match input {
                Some(Input::Write(data, reply)) => {
                    let writer = writer.clone();
                    writes.spawn_blocking(move || { let result = writer.lock().unwrap().write_all(&data).map_err(Into::into); let _ = reply.send(result); });
                }
                Some(Input::Resize(size, reply)) => {
                    let master = master.clone();
                    writes.spawn_blocking(move || { let result = master.lock().unwrap().resize(pty_size(size)).map_err(driver_error); let _ = reply.send(result); });
                }
                None => {
                    context.stop.cancel();
                    break;
                }
            },
            Some(value) = context.auto_input.recv(), if writes.is_empty() => {
                let writer = writer.clone();
                let events = context.events.clone();
                let id = context.id;
                writes.spawn_blocking(move || {
                    if let Err(error) = writer.lock().unwrap().write_all(value.expose().as_bytes()) {
                        let _ = events.send(DriverEvent::AutoInputFailed { id, error: error.into() });
                    }
                });
            }
            _ = writes.join_next(), if !writes.is_empty() => {}
        }
    }
    let requested_stop = context.stop.is_cancelled();
    // Ending: keep ownership until every process in the session has exited,
    // because a shell may leave background jobs behind.
    context.event(DriverEvent::Cleaning(context.id));
    context.input.close();
    while let Ok(input) = context.input.try_recv() {
        match input {
            Input::Write(_, reply) | Input::Resize(_, reply) => {
                let _ = reply.send(Err(TerminalError::cancelled()));
            }
        }
    }
    // Windows has no signals, so a requested stop types Ctrl+C into the console instead.
    #[cfg(windows)]
    {
        if requested_stop {
            let writer = writer.clone();
            writes.spawn_blocking(move || {
                let _ = writer.lock().unwrap().write_all(b"\x03");
            });
        }
    }
    end_session(&scope, plan, &context).await;
    // Collecting: the session has ended, so its I/O and the leader's status are final.
    while writes.join_next().await.is_some() {}
    drop(writer);
    drop(master);
    match read_result {
        Some(result) => result,
        None => reading.await.map_err(driver_error).and_then(|r| r),
    }?;
    let status = match status {
        Some(result) => result,
        None => waiting.await,
    }
    .map_err(driver_error)
    .and_then(|r| r.map_err(Into::into))?;
    if requested_stop {
        return Ok(());
    }
    if !status.success() {
        return Err(TerminalError::new(
            TerminalErrorKind::ProcessFailed,
            format!(
                "command exited with status {}{}",
                status.exit_code(),
                status
                    .signal()
                    .map(|s| format!(" ({s})"))
                    .unwrap_or_default()
            ),
        ));
    }
    Ok(())
}

/// Makes each request of the stop plan, then kills the session once the runtime forces the stop.
/// The runtime owns the deadlines; the driver only spaces out its requests.
async fn end_session(scope: &ProcessScope, [first, second]: [StopRequest; 2], context: &Context) {
    let mut ending = Ending {
        scope,
        context,
        poll_failed: false,
    };
    let force = &context.force;
    if !force.is_cancelled() {
        ending.request(first);
        // Half of the grace period lets handlers, such as an editor's emergency save, finish.
        let pause = context.options.stop.stop_grace_timeout / 2;
        let paused = async {
            let _ = tokio::time::timeout(pause, force.cancelled()).await;
        };
        if ending.until_empty(paused).await {
            return;
        }
    }
    if !force.is_cancelled() {
        ending.request(second);
        if ending.until_empty(force.cancelled()).await {
            return;
        }
    }
    ending.kill();
    ending.until_empty(std::future::pending()).await;
}
/// One session ending, which reports each failure as a stop failure, a failing poll only once.
struct Ending<'a> {
    scope: &'a ProcessScope,
    context: &'a Context,
    poll_failed: bool,
}
impl Ending<'_> {
    fn request(&self, request: StopRequest) {
        self.report(self.scope.request(request));
    }
    fn kill(&self) {
        self.context.event(DriverEvent::Forced(self.context.id));
        self.report(self.scope.kill());
    }
    /// Polls until every process in the session has exited, which returns true,
    /// or until `interrupt` completes first, which returns false.
    async fn until_empty(&mut self, interrupt: impl Future<Output = ()>) -> bool {
        let mut interrupt = std::pin::pin!(interrupt);
        loop {
            match self.scope.is_empty() {
                Ok(true) => return true,
                Ok(false) => {}
                Err(error) => {
                    if !std::mem::replace(&mut self.poll_failed, true) {
                        self.report(Err(error));
                    }
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(self.context.options.process_poll_interval) => {}
                _ = &mut interrupt => return false,
            }
        }
    }
    fn report(&self, result: Result<()>) {
        if let Err(error) = result {
            self.context.event(DriverEvent::StopFailed {
                id: self.context.id,
                error,
            });
        }
    }
}
