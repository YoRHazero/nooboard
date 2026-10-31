use super::{Context, DriverEvent, Input, driver_error, process::ProcessScope};
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
fn command_builder(command: Option<String>) -> CommandBuilder {
    let mut builder = CommandBuilder::new_default_prog();
    if let Some(command) = command {
        #[cfg(unix)]
        {
            builder = CommandBuilder::new("/bin/sh");
            // Finder-launched apps have a minimal PATH. Load the POSIX login
            // environment inside the same managed PTY before running the command.
            builder.arg("-l");
            builder.arg("-c");
            builder.arg(command);
        }
        #[cfg(windows)]
        {
            builder = CommandBuilder::new(
                std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into()),
            );
            builder.arg("/C");
            builder.arg(command);
        }
    }
    builder.env("TERM", "xterm-256color");
    builder
}

fn prepare(command: Option<String>, size: Option<TerminalSize>) -> Result<Prepared> {
    let pair = portable_pty::native_pty_system()
        .openpty(size.map(pty_size).unwrap_or_default())
        .map_err(driver_error)?;
    let builder = command_builder(command);
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

pub(super) async fn run(command: Option<String>, mut context: Context) -> Result<()> {
    let size = *context.size.lock().unwrap();
    // Never discard a pending spawn: cancellation is applied to the child as soon as it is owned.
    let prepared = tokio::task::spawn_blocking(move || prepare(command, size))
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
    let mut stop_sent = false;
    let mut force_sent = false;
    let mut read_result = None;
    let status = loop {
        tokio::select! {
            result = &mut waiting => break result.map_err(driver_error).and_then(|r| r.map_err(Into::into)),
            result = &mut reading, if read_result.is_none() => {
                let result = result.map_err(driver_error).and_then(|r| r);
                if result.is_err() {
                    context.stop.cancel();
                    context.event(DriverEvent::Cleaning(context.id));
                }
                read_result = Some(result);
            }
            _ = context.stop.cancelled(), if !stop_sent => {
                stop_sent = true;
                if let Err(error) = scope.signal(false) { context.event(DriverEvent::StopFailed { id: context.id, error }); }
                #[cfg(windows)] {
                    let writer = writer.clone();
                    writes.spawn_blocking(move || { let _ = writer.lock().unwrap().write_all(b"\x03"); });
                }
            }
            _ = context.force.cancelled(), if !force_sent => {
                force_sent = true;
                context.event(DriverEvent::Forced(context.id));
                if let Err(error) = scope.signal(true) { context.event(DriverEvent::StopFailed { id: context.id, error }); }
            }
            input = context.input.recv(), if !stop_sent && writes.is_empty() => {
                match input {
                    Some(Input::Write(data, reply)) => {
                        let writer = writer.clone();
                        writes.spawn_blocking(move || { let result = writer.lock().unwrap().write_all(&data).map_err(Into::into); let _ = reply.send(result); });
                    }
                    Some(Input::Resize(size, reply)) => {
                        let master = master.clone();
                        writes.spawn_blocking(move || { let result = master.lock().unwrap().resize(pty_size(size)).map_err(driver_error); let _ = reply.send(result); });
                    }
                    None => { context.stop.cancel(); }
                }
            }
            Some(value) = context.auto_input.recv(), if !stop_sent && writes.is_empty() => {
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
    };
    let requested_stop = context.stop.is_cancelled();
    context.event(DriverEvent::Cleaning(context.id));
    context.input.close();
    while let Ok(input) = context.input.try_recv() {
        match input {
            Input::Write(_, reply) | Input::Resize(_, reply) => {
                let _ = reply.send(Err(TerminalError::cancelled()));
            }
        }
    }
    // A shell may exit while background jobs still belong to its session.
    // Keep ownership until the entire managed session/job and its I/O have ended.
    if let Err(error) = scope.signal(false) {
        context.event(DriverEvent::StopFailed {
            id: context.id,
            error,
        });
    }
    let cleanup_started = tokio::time::Instant::now();
    let mut reported = false;
    loop {
        match scope.is_empty() {
            Ok(true) => break,
            Ok(false) => {}
            Err(error) if !reported => {
                reported = true;
                context.event(DriverEvent::StopFailed {
                    id: context.id,
                    error,
                });
            }
            Err(_) => {}
        }
        if !force_sent
            && (context.force.is_cancelled()
                || cleanup_started.elapsed() >= context.options.stop.stop_grace_timeout)
        {
            force_sent = true;
            context.event(DriverEvent::Forced(context.id));
            if let Err(error) = scope.signal(true) {
                context.event(DriverEvent::StopFailed {
                    id: context.id,
                    error,
                });
            }
        }
        if !reported
            && cleanup_started.elapsed()
                >= context.options.stop.stop_grace_timeout + context.options.stop.force_stop_timeout
        {
            reported = true;
            context.event(DriverEvent::StopFailed {
                id: context.id,
                error: TerminalError::new(
                    TerminalErrorKind::StopTimedOut,
                    "owned process session has not ended",
                ),
            });
        }
        tokio::time::sleep(context.options.process_poll_interval).await;
    }
    while writes.join_next().await.is_some() {}
    drop(writer);
    drop(master);
    let reading = match read_result {
        Some(result) => result,
        None => reading.await.map_err(driver_error)?,
    };
    reading?;
    let status = status?;
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
