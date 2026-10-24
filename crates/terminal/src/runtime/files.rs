use super::*;
impl Manager {
    fn file_connection(&self, id: TerminalRunId) -> Result<Arc<backend::ssh::Connection>> {
        self.admit()?;
        let entry = self
            .entries
            .get(&RunId::Terminal(id))
            .ok_or_else(not_found)?;
        if !matches!(entry.phase, Phase::Running) {
            return Err(invalid_state());
        }
        entry.connection.clone().ok_or_else(|| {
            TerminalError::new(
                TerminalErrorKind::Unsupported,
                "file operations require an SSH terminal",
            )
        })
    }
    pub(super) fn start_file(
        &mut self,
        request: StartFileOperationRequest,
    ) -> Result<FileOperationRunId> {
        let connection = self.file_connection(request.terminal_run_id)?;
        match &request.operation {
            FileOperationSpec::Upload { items, .. } | FileOperationSpec::Download { items, .. } => {
                if items.is_empty() || items.iter().any(|item| !item.local_path.is_absolute()) {
                    return Err(TerminalError::invalid(
                        "file transfer requires nonempty items with absolute local paths",
                    ));
                }
            }
            FileOperationSpec::Delete { paths } => {
                if paths.is_empty() {
                    return Err(TerminalError::invalid("delete requires at least one path"));
                }
            }
        }
        let id = FileOperationRunId::new();
        let stop = CancellationToken::new();
        self.files.insert(
            id,
            FileEntry {
                view: FileOperationRun {
                    id,
                    terminal_run_id: request.terminal_run_id,
                    operation: request.operation.clone(),
                    state: FileOperationRunState::Starting,
                    stop_method: None,
                    progress: Default::default(),
                },
                stop: stop.clone(),
                waiters: vec![],
            },
        );
        let options = self.options.clone();
        let tx = self.tx.clone();
        let task = self.tasks.spawn(async move {
            let completion =
                backend::files::run(id, request.operation, connection, options, stop, tx.clone())
                    .await;
            let _ = tx.send(DriverEvent::FileFinished { id, completion });
        });
        self.owners.insert(task.id(), TaskOwner::File(id));
        Ok(id)
    }
    pub(super) fn stop_file(&mut self, id: FileOperationRunId, reply: Option<Reply<()>>) {
        let Some(file) = self.files.get_mut(&id) else {
            if let Some(reply) = reply {
                let _ = reply.send(Ok(()));
            }
            return;
        };
        if let Some(reply) = reply {
            if let FileOperationRunState::Stopping { error: Some(error) } = &file.view.state {
                let _ = reply.send(Err(error.clone()));
            } else {
                file.waiters.push(reply);
            }
        }
        if matches!(file.view.state, FileOperationRunState::Stopping { .. }) {
            return;
        }
        file.view.state = FileOperationRunState::Stopping { error: None };
        file.view.stop_method = Some(StopMethod::Graceful);
        file.stop.cancel();
        let duration = self.options.stop.stop_grace_timeout + self.options.stop.force_stop_timeout;
        self.timers.spawn(async move {
            tokio::time::sleep(duration).await;
            Timer::FileStopExpired(id)
        });
    }
    pub(super) fn file_stop_failed(&mut self, id: FileOperationRunId, error: TerminalError) {
        if let Some(file) = self.files.get_mut(&id) {
            file.view.state = FileOperationRunState::Stopping {
                error: Some(error.clone()),
            };
            for waiter in file.waiters.drain(..) {
                let _ = waiter.send(Err(error.clone()));
            }
        }
    }
    pub(super) fn finish_file(
        &mut self,
        id: FileOperationRunId,
        completion: backend::files::FileCompletion,
    ) {
        let Some(mut file) = self.files.remove(&id) else {
            return;
        };
        let owner = file.view.terminal_run_id;
        let stopped = file.stop.is_cancelled();
        let result = match completion.result {
            Err(error) if error.kind == TerminalErrorKind::Cancelled && stopped => {
                FileOperationRunResult::Stopped
            }
            Err(error) => FileOperationRunResult::Failed(error),
            Ok(()) if stopped => FileOperationRunResult::Stopped,
            Ok(()) => FileOperationRunResult::Succeeded,
        };
        file.view.progress = completion.progress;
        file.view.state = FileOperationRunState::Ended {
            result,
            cleanup_issues: completion.cleanup_issues,
        };
        self.notifications
            .push(TerminalEvent::FileOperationEnded { run: file.view });
        for waiter in file.waiters {
            let _ = waiter.send(Ok(()));
        }
        self.children_changed(RunId::Terminal(owner));
    }
    fn short(
        &mut self,
        owner: TerminalRunId,
    ) -> Result<(u64, CancellationToken, Arc<backend::ssh::Connection>)> {
        let connection = self.file_connection(owner)?;
        self.sequence += 1;
        let id = self.sequence;
        let stop = CancellationToken::new();
        self.shorts.insert(
            id,
            ShortOperation {
                owner,
                stop: stop.clone(),
            },
        );
        Ok((id, stop, connection))
    }
    pub(super) fn list(
        &mut self,
        owner: TerminalRunId,
        path: Option<RemotePath>,
        cancel: CancellationToken,
        reply: Reply<RemoteDirectoryListing>,
    ) {
        let (id, stop, connection) = match self.short(owner) {
            Ok(values) => values,
            Err(error) => {
                let _ = reply.send(Err(error));
                return;
            }
        };
        let options = self.options.clone();
        let tx = self.tx.clone();
        let task=self.tasks.spawn(async move{
            let operation=backend::files::list(connection,options,path,stop.clone());tokio::pin!(operation);
            let result=tokio::select!{biased;_=cancel.cancelled()=>{stop.cancel();operation.await},result=&mut operation=>result};
            let _=reply.send(result);let _=tx.send(DriverEvent::ShortFinished(id));
        });
        self.owners.insert(task.id(), TaskOwner::Short(id));
    }
    pub(super) fn mkdir(&mut self, owner: TerminalRunId, path: RemotePath, reply: Reply<()>) {
        let (id, stop, connection) = match self.short(owner) {
            Ok(values) => values,
            Err(error) => {
                let _ = reply.send(Err(error));
                return;
            }
        };
        let options = self.options.clone();
        let tx = self.tx.clone();
        let task = self.tasks.spawn(async move {
            let result = backend::files::mkdir(connection, options, path, stop).await;
            let _ = reply.send(result);
            let _ = tx.send(DriverEvent::ShortFinished(id));
        });
        self.owners.insert(task.id(), TaskOwner::Short(id));
    }
    pub(super) fn rename(
        &mut self,
        owner: TerminalRunId,
        source: RemotePath,
        destination: RemotePath,
        reply: Reply<()>,
    ) {
        let (id, stop, connection) = match self.short(owner) {
            Ok(values) => values,
            Err(error) => {
                let _ = reply.send(Err(error));
                return;
            }
        };
        let options = self.options.clone();
        let tx = self.tx.clone();
        let task = self.tasks.spawn(async move {
            let result =
                backend::files::rename(connection, options, source, destination, stop).await;
            let _ = reply.send(result);
            let _ = tx.send(DriverEvent::ShortFinished(id));
        });
        self.owners.insert(task.id(), TaskOwner::Short(id));
    }
}
