use super::*;
impl Manager {
    pub(super) fn event(&mut self, event: DriverEvent) {
        match event {
            DriverEvent::Started { id, connection } => {
                if let Some(entry) = self.entries.get_mut(&id) {
                    entry.connection = connection;
                    if entry.force.is_cancelled()
                        && let Some(connection) = &entry.connection
                    {
                        connection.force_close();
                    }
                    if !entry.stopping() {
                        entry.phase = Phase::Running;
                        // A resize can arrive after the driver sampled the initial size,
                        // but before its Started event reached this manager.
                        if let Some(size) = *entry.size.lock().unwrap() {
                            let (reply, _) = tokio::sync::oneshot::channel();
                            let _ = entry.input.try_send(Input::Resize(size, reply));
                        }
                    }
                }
            }
            DriverEvent::Resolved { id, address } => {
                if let Some(entry) = self.entries.get_mut(&id)
                    && let RunTarget::Ssh(target) = &mut entry.target
                {
                    target.resolved_address = Some(address);
                }
            }
            DriverEvent::Ready(id) => self.ready(id),
            DriverEvent::AutoInputFailed { id, error } => {
                if let Some(entry) = self.entries.get_mut(&id)
                    && !entry.stopping()
                    && let Kind::Command {
                        auto_input_error, ..
                    } = &mut entry.kind
                {
                    *auto_input_error = Some(error.with_source_run(id));
                }
            }
            DriverEvent::Cleaning(id) => {
                let mut timer = false;
                if let Some(entry) = self.entries.get_mut(&id) {
                    timer = !entry.stopping();
                    entry.cleaning = true;
                    if timer {
                        entry.phase = Phase::Stopping(None);
                    }
                }
                if timer {
                    self.stop_timers(id);
                }
                self.stop_children(id);
                self.children_changed(id);
            }
            DriverEvent::Forced(id) => {
                if let Some(entry) = self.entries.get_mut(&id) {
                    entry.method = Some(StopMethod::Forced);
                }
            }
            DriverEvent::StopFailed { id, error } => self.stop_failed(id, error),
            DriverEvent::Finished { id, result } => self.finish(id, result),
            DriverEvent::FileStarted(id) => {
                if let Some(file) = self.files.get_mut(&id)
                    && matches!(file.view.state, FileOperationRunState::Starting)
                {
                    file.view.state = FileOperationRunState::Running;
                }
            }
            DriverEvent::FileProgress { id, progress } => {
                if let Some(file) = self.files.get_mut(&id) {
                    file.view.progress = progress;
                }
            }
            DriverEvent::FileFinished { id, completion } => self.finish_file(id, completion),
            DriverEvent::ShortFinished(id) => {
                if let Some(short) = self.shorts.remove(&id) {
                    self.children_changed(RunId::Terminal(short.owner));
                }
            }
        }
    }
    pub(super) fn stop_timers(&mut self, id: RunId) {
        let graceful = self.options.stop.stop_grace_timeout;
        let force = self.options.stop.force_stop_timeout;
        self.timers.spawn(async move {
            tokio::time::sleep(graceful).await;
            Timer::Force(id)
        });
        self.timers.spawn(async move {
            tokio::time::sleep(graceful + force).await;
            Timer::StopExpired(id)
        });
    }
    pub(super) fn stop(
        &mut self,
        id: RunId,
        reply: Option<Reply<()>>,
        cause: Option<TerminalError>,
    ) {
        let Some(entry) = self.entries.get_mut(&id) else {
            if let Some(reply) = reply {
                let _ = reply.send(Ok(()));
            }
            return;
        };
        if entry.stopping() {
            if let Some(reply) = reply {
                if let Phase::Stopping(Some(error)) = &entry.phase {
                    let _ = reply.send(Err(error.clone()));
                } else {
                    entry.waiters.push(reply);
                }
            }
            return;
        }
        entry.phase = Phase::Stopping(None);
        entry.stop_requested = true;
        entry.ending_error = cause;
        entry.method = Some(StopMethod::Graceful);
        if let Some(reply) = reply {
            entry.waiters.push(reply);
        }
        self.stop_timers(id);
        if let RunId::Command(command) = id {
            let cause = TerminalError::dependency(
                id,
                TerminalError::new(
                    TerminalErrorKind::Cancelled,
                    "prerequisite command was stopped",
                ),
            );
            for owner in self.users(command) {
                self.stop(owner, None, Some(cause.clone()));
            }
        }
        self.stop_children(id);
        self.activate_stop(id);
    }
    fn activate_stop(&mut self, id: RunId) {
        if let RunId::Command(command) = id
            && !self.users(command).is_empty()
        {
            return;
        }
        if self.has_children(id) {
            return;
        }
        let Some(entry) = self.entries.get_mut(&id) else {
            return;
        };
        if !entry.stopping() {
            return;
        }
        entry.children_drained.cancel();
        if entry.cleaning && !entry.stop_requested {
            return;
        }
        entry.stop.cancel();
        if !entry.launched {
            self.finish(id, Ok(()));
        }
    }
    pub(super) fn stop_failed(&mut self, id: RunId, error: TerminalError) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.phase = Phase::Stopping(Some(error.clone()));
            for waiter in entry.waiters.drain(..) {
                let _ = waiter.send(Err(error.clone()));
            }
        }
        self.pending_changed();
    }
    pub(super) fn timer(&mut self, timer: Timer) {
        match timer {
            Timer::Force(id) => self.force(id),
            Timer::StopExpired(id) => {
                if self.entries.get(&id).is_some_and(Entry::stopping) {
                    self.stop_failed(
                        id,
                        TerminalError::new(
                            TerminalErrorKind::StopTimedOut,
                            "resource stop has not been confirmed",
                        ),
                    );
                }
            }
            Timer::FileStopExpired(id) => {
                if self
                    .files
                    .get(&id)
                    .is_some_and(|f| matches!(f.view.state, FileOperationRunState::Stopping { .. }))
                {
                    self.file_stop_failed(
                        id,
                        TerminalError::new(
                            TerminalErrorKind::StopTimedOut,
                            "file I/O has not settled",
                        ),
                    );
                }
            }
            Timer::PendingChanged => self.pending_changed(),
            Timer::ForceAll => {
                let ids = self.entries.keys().copied().collect::<Vec<_>>();
                for id in ids {
                    self.force(id);
                }
            }
        }
    }
    fn force(&mut self, id: RunId) {
        if let Some(entry) = self.entries.get_mut(&id)
            && entry.stopping()
        {
            entry.method = Some(StopMethod::Forced);
            entry.stop.cancel();
            entry.force.cancel();
            if let Some(transport) = entry.transport.lock().unwrap().as_ref() {
                transport.close();
            }
            if let Some(connection) = &entry.connection {
                connection.force_close();
            }
        }
    }
    fn has_children(&self, id: RunId) -> bool {
        let RunId::Terminal(id) = id else {
            return false;
        };
        self.files.values().any(|f| f.view.terminal_run_id == id)
            || self.shorts.values().any(|s| s.owner == id)
    }
    fn stop_children(&mut self, id: RunId) {
        let RunId::Terminal(parent) = id else {
            return;
        };
        let ids = self
            .files
            .values()
            .filter(|f| f.view.terminal_run_id == parent)
            .map(|f| f.view.id)
            .collect::<Vec<_>>();
        for id in ids {
            self.stop_file(id, None);
        }
        for short in self.shorts.values().filter(|s| s.owner == parent) {
            short.stop.cancel();
        }
    }
    pub(super) fn children_changed(&mut self, id: RunId) {
        if self.has_children(id) {
            return;
        }
        let mut finished = None;
        if let Some(entry) = self.entries.get_mut(&id)
            && entry.stopping()
        {
            entry.children_drained.cancel();
            finished = entry.finished.take();
        }
        if let Some(result) = finished {
            self.finish(id, result);
        } else {
            self.activate_stop(id);
        }
    }
    pub(super) fn finish(&mut self, id: RunId, result: Result<()>) {
        if self.has_children(id) {
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.finished = Some(result);
                entry.phase = Phase::Stopping(None);
                entry.cleaning = true;
            }
            self.stop_children(id);
            return;
        }
        let Some(mut entry) = self.entries.remove(&id) else {
            return;
        };
        if let Some(output) = &entry.output {
            output.finish();
        }
        let result = if let Some(error) = entry.ending_error.take() {
            Err(error)
        } else if entry.stop_requested
            && result
                .as_ref()
                .is_err_and(|e| e.kind == TerminalErrorKind::Cancelled)
        {
            Ok(())
        } else {
            result
        };
        match entry.kind {
            Kind::Terminal(_) => {
                let mut run = entry.terminal();
                run.state = TerminalRunState::Ended {
                    result: match result.clone() {
                        Err(e) => TerminalRunResult::Failed(e),
                        Ok(()) if entry.stop_requested => TerminalRunResult::Stopped,
                        Ok(()) => TerminalRunResult::Succeeded,
                    },
                };
                self.notifications
                    .push(TerminalEvent::TerminalEnded { run });
            }
            Kind::Tunnel { .. } => {
                let mut run = entry.tunnel();
                run.state = TunnelRunState::Ended {
                    result: match result.clone() {
                        Err(e) => TunnelRunResult::Failed(e),
                        Ok(()) => TunnelRunResult::Stopped,
                    },
                };
                self.notifications.push(TerminalEvent::TunnelEnded { run });
            }
            Kind::Command { .. } => {
                let mut run = entry.command();
                let command = run.id;
                let success = result.is_ok() && !entry.stop_requested;
                run.state = CommandRunState::Ended {
                    result: match result.clone() {
                        Err(e) => CommandRunResult::Failed(e),
                        Ok(()) if entry.stop_requested => CommandRunResult::Stopped,
                        Ok(()) => CommandRunResult::Succeeded,
                    },
                };
                self.notifications.push(TerminalEvent::CommandEnded { run });
                let cause = TerminalError::dependency(
                    id,
                    result.clone().err().unwrap_or_else(|| {
                        TerminalError::new(
                            TerminalErrorKind::ProcessFailed,
                            "sustained prerequisite command ended",
                        )
                    }),
                );
                let mut released = vec![];
                let mut failed = vec![];
                for dependency in self
                    .dependencies
                    .values_mut()
                    .filter(|d| d.view.command_run_id == Some(command))
                {
                    let owner = owner_id(dependency.view.owner_run_id);
                    match dependency.view.gate_state {
                        DependencyGateState::WaitingForRelease if success => {
                            dependency.view.gate_state = DependencyGateState::Released {
                                reason: DependencyReleaseReason::SuccessfulExit,
                            };
                            released.push(owner);
                        }
                        DependencyGateState::WaitingForRelease
                        | DependencyGateState::Released {
                            reason:
                                DependencyReleaseReason::ReadyMarker
                                | DependencyReleaseReason::ManualContinue,
                        } => {
                            dependency.view.gate_state = DependencyGateState::Failed {
                                error: cause.clone(),
                            };
                            failed.push(owner);
                        }
                        _ => {}
                    }
                }
                for owner in failed {
                    self.stop(owner, None, Some(cause.clone()));
                }
                for owner in released {
                    self.progress_owner(owner);
                }
            }
        }
        let mut released_commands = vec![];
        for dependency in &entry.dependencies {
            if let Some(dependency) = self.dependencies.remove(dependency)
                && let Some(command) = dependency.view.command_run_id
            {
                released_commands.push(command);
            }
        }
        for command in released_commands {
            let id = RunId::Command(command);
            if self.users(command).is_empty() {
                if self.entries.get(&id).is_some_and(|e| {
                    matches!(
                        e.kind,
                        Kind::Command {
                            lifetime: CommandLifetimePolicy::Dependent,
                            ..
                        }
                    ) && !e.stopping()
                }) {
                    self.stop(id, None, None);
                } else {
                    self.activate_stop(id);
                }
            }
        }
        for waiter in entry.waiters {
            let _ = waiter.send(Ok(()));
        }
        self.pending_changed();
    }
    pub(super) fn begin_shutdown(&mut self, reply: Option<Reply<()>>) {
        if let Some(reply) = reply {
            self.shutdown_waiters.push(reply);
        }
        if self.closing {
            return;
        }
        self.closing = true;
        self.pending_changed();
        let ids = self.entries.keys().copied().collect::<Vec<_>>();
        for id in ids {
            self.stop(id, None, None);
        }
        let deadline = self
            .options
            .stop
            .shutdown_timeout
            .saturating_sub(self.options.stop.force_stop_timeout);
        self.timers.spawn(async move {
            tokio::time::sleep(deadline).await;
            Timer::ForceAll
        });
    }
}
