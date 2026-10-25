use super::*;
impl Manager {
    pub(super) fn start_terminal(
        &mut self,
        request: StartTerminalRequest,
        attach_output: bool,
    ) -> Result<(TerminalRunId, Option<TerminalOutputSubscription>)> {
        self.admit()?;
        name(&request.name)?;
        validate_dependencies(&request.dependencies)?;
        let (target, driver) = match &request.target {
            TerminalStartTarget::Local => (RunTarget::Local, DriverSpec::Local { command: None }),
            TerminalStartTarget::Ssh(target) => {
                validate_ssh(target)?;
                (
                    ssh_target(target),
                    DriverSpec::Ssh {
                        target: target.clone(),
                        command: None,
                    },
                )
            }
        };
        let id = TerminalRunId::new();
        let run = RunId::Terminal(id);
        self.new_entry(
            run,
            request.name,
            target,
            Kind::Terminal(request.config_id),
            driver,
            request.dependencies,
        );
        let output = attach_output.then(|| {
            self.entries[&run]
                .output
                .as_ref()
                .unwrap()
                .buffer
                .subscribe()
        });
        self.progress_owner(run);
        Ok((id, output))
    }
    pub(super) fn start_tunnel(&mut self, request: StartTunnelRequest) -> Result<TunnelRunId> {
        self.admit()?;
        name(&request.config_name)?;
        validate_ssh(&request.target)?;
        validate_dependencies(&request.dependencies)?;
        match &request.forward {
            TunnelForwardConfig::Local {
                listen,
                destination,
            }
            | TunnelForwardConfig::Remote {
                listen,
                destination,
            } => {
                endpoint(listen)?;
                endpoint(destination)?;
            }
            TunnelForwardConfig::Dynamic { listen } => endpoint(listen)?,
        }
        let id = TunnelRunId::new();
        let run = RunId::Tunnel(id);
        self.new_entry(
            run,
            request.config_name,
            ssh_target(&request.target),
            Kind::Tunnel {
                config_id: request.config_id,
                forward: request.forward.clone(),
            },
            DriverSpec::Tunnel {
                target: request.target,
                forward: request.forward,
            },
            request.dependencies,
        );
        self.progress_owner(run);
        Ok(id)
    }
    pub(super) fn start_command(
        &mut self,
        request: StartCommandRequest,
        cancel: CancellationToken,
        attach_output: bool,
        reply: Reply<(CommandRunId, Option<TerminalOutputSubscription>)>,
    ) {
        if let Err(error) = self
            .admit()
            .and_then(|()| validate_command(&request.command))
        {
            let _ = reply.send(Err(error));
            return;
        }
        if cancel.is_cancelled() {
            let _ = reply.send(Err(TerminalError::cancelled()));
            return;
        }
        match self.acquire(
            &request.command,
            request.reuse_policy,
            CommandLifetimePolicy::Independent,
        ) {
            Ok(Some(id)) => {
                let output = attach_output.then(|| {
                    self.entries[&RunId::Command(id)]
                        .output
                        .as_ref()
                        .unwrap()
                        .buffer
                        .subscribe()
                });
                let _ = reply.send(Ok((id, output)));
            }
            Err(error) => {
                let _ = reply.send(Err(error));
            }
            Ok(None) => {
                let done = CancellationToken::new();
                let watched = done.clone();
                let cancellation = cancel.clone();
                self.timers.spawn(async move {
                    tokio::select! {_=watched.cancelled()=>{},_=cancellation.cancelled()=>{}}
                    Timer::PendingChanged
                });
                self.sequence += 1;
                self.pending.push(PendingCommand {
                    order: self.sequence,
                    attach_output,
                    request,
                    cancel,
                    done,
                    reply,
                });
            }
        }
    }
    fn acquire(
        &mut self,
        spec: &CommandStartSpec,
        policy: CommandReusePolicy,
        lifetime: CommandLifetimePolicy,
    ) -> Result<Option<CommandRunId>> {
        if policy == CommandReusePolicy::Shared {
            if let Some(entry) = self
                .entries
                .values()
                .filter(|e| e.command_config() == Some(spec.config_id) && !e.stopping())
                .max_by_key(|e| e.order)
            {
                let RunId::Command(id) = entry.id else {
                    unreachable!()
                };
                return Ok(Some(id));
            }
            let stopping: Vec<_> = self
                .entries
                .values()
                .filter(|e| e.command_config() == Some(spec.config_id) && e.stopping())
                .collect();
            for entry in &stopping {
                if let Phase::Stopping(Some(error)) = &entry.phase {
                    return Err(error.clone());
                }
            }
            if !stopping.is_empty() {
                return Ok(None);
            }
        }
        let id = CommandRunId::new();
        let run = RunId::Command(id);
        let (target, driver) = match &spec.target {
            CommandStartTarget::Local => (
                RunTarget::Local,
                DriverSpec::Local {
                    command: Some(spec.command.clone()),
                },
            ),
            CommandStartTarget::Ssh(target) => (
                ssh_target(target),
                DriverSpec::Ssh {
                    target: target.clone(),
                    command: Some(spec.command.clone()),
                },
            ),
        };
        self.new_entry(
            run,
            spec.config_name.clone(),
            target,
            Kind::Command {
                config_id: spec.config_id,
                command: spec.command.clone(),
                lifetime,
                ready: false,
            },
            driver,
            vec![],
        );
        self.launch(run);
        Ok(Some(id))
    }
    pub(super) fn progress_owner(&mut self, owner: RunId) {
        loop {
            let Some(entry) = self.entries.get(&owner) else {
                return;
            };
            if entry.stopping() || entry.launched {
                return;
            }
            let next = entry
                .dependencies
                .iter()
                .find(|id| {
                    self.dependencies.get(id).is_some_and(|d| {
                        !matches!(d.view.gate_state, DependencyGateState::Released { .. })
                    })
                })
                .copied();
            let Some(id) = next else {
                self.launch(owner);
                return;
            };
            let dep = self.dependencies.get(&id).unwrap();
            if !matches!(
                dep.view.gate_state,
                DependencyGateState::Pending | DependencyGateState::WaitingForInstance
            ) {
                return;
            }
            let spec = dep.spec.clone();
            match self.acquire(
                &spec.command,
                spec.reuse_policy,
                CommandLifetimePolicy::Dependent,
            ) {
                Ok(Some(command)) => {
                    let ready = self
                        .entries
                        .get(&RunId::Command(command))
                        .is_some_and(|e| matches!(e.kind, Kind::Command { ready: true, .. }));
                    let dep = self.dependencies.get_mut(&id).unwrap();
                    dep.wait_order = None;
                    dep.view.command_run_id = Some(command);
                    dep.view.gate_state = if ready {
                        DependencyGateState::Released {
                            reason: DependencyReleaseReason::ReadyMarker,
                        }
                    } else {
                        DependencyGateState::WaitingForRelease
                    };
                    if !ready {
                        return;
                    }
                }
                Ok(None) => {
                    let dependency = self.dependencies.get_mut(&id).unwrap();
                    dependency.view.gate_state = DependencyGateState::WaitingForInstance;
                    if dependency.wait_order.is_none() {
                        self.sequence += 1;
                        dependency.wait_order = Some(self.sequence);
                    }
                    return;
                }
                Err(error) => {
                    self.dependencies.get_mut(&id).unwrap().view.gate_state =
                        DependencyGateState::Failed {
                            error: error.clone(),
                        };
                    self.stop(owner, None, Some(error));
                    return;
                }
            }
        }
    }
    pub(super) fn pending_changed(&mut self) {
        enum Waiting {
            Standalone(Box<PendingCommand>),
            Dependency(DependencyRunId),
        }
        let mut waiting: Vec<_> = std::mem::take(&mut self.pending)
            .into_iter()
            .map(|p| (p.order, Waiting::Standalone(Box::new(p))))
            .collect();
        waiting.extend(self.dependencies.values().filter_map(|d| {
            d.wait_order
                .map(|order| (order, Waiting::Dependency(d.view.id)))
        }));
        waiting.sort_by_key(|(order, _)| *order);
        for (_, request) in waiting {
            match request {
                Waiting::Standalone(p) => {
                    let p = *p;
                    if p.cancel.is_cancelled() || p.reply.is_closed() || self.closing {
                        p.done.cancel();
                        let _ = p.reply.send(Err(if self.closing {
                            TerminalError::closed()
                        } else {
                            TerminalError::cancelled()
                        }));
                        continue;
                    }
                    match self.acquire(
                        &p.request.command,
                        p.request.reuse_policy,
                        CommandLifetimePolicy::Independent,
                    ) {
                        Ok(Some(id)) => {
                            p.done.cancel();
                            let output = p.attach_output.then(|| {
                                self.entries[&RunId::Command(id)]
                                    .output
                                    .as_ref()
                                    .unwrap()
                                    .buffer
                                    .subscribe()
                            });
                            let _ = p.reply.send(Ok((id, output)));
                        }
                        Err(error) => {
                            p.done.cancel();
                            let _ = p.reply.send(Err(error));
                        }
                        Ok(None) => self.pending.push(p),
                    }
                }
                Waiting::Dependency(id) => {
                    if let Some(dependency) = self.dependencies.get(&id)
                        && matches!(
                            dependency.view.gate_state,
                            DependencyGateState::WaitingForInstance
                        )
                    {
                        self.progress_owner(owner_id(dependency.view.owner_run_id));
                    }
                }
            }
        }
    }
    pub(super) fn continue_dependency(&mut self, id: DependencyRunId) -> Result<()> {
        let dependency = self.dependencies.get(&id).ok_or_else(not_found)?;
        if matches!(
            dependency.view.gate_state,
            DependencyGateState::Released { .. }
        ) {
            return Ok(());
        }
        let owner = owner_id(dependency.view.owner_run_id);
        if !matches!(
            dependency.view.gate_state,
            DependencyGateState::WaitingForRelease
        ) || self.entries.get(&owner).is_none_or(Entry::stopping)
        {
            return Err(invalid_state());
        }
        let command = dependency.view.command_run_id.ok_or_else(invalid_state)?;
        if self
            .entries
            .get(&RunId::Command(command))
            .is_none_or(Entry::stopping)
        {
            return Err(invalid_state());
        }
        self.dependencies.get_mut(&id).unwrap().view.gate_state = DependencyGateState::Released {
            reason: DependencyReleaseReason::ManualContinue,
        };
        self.progress_owner(owner);
        Ok(())
    }
    pub(super) fn ready(&mut self, id: RunId) {
        let Some(entry) = self.entries.get_mut(&id) else {
            return;
        };
        if entry.stopping() {
            return;
        }
        let Kind::Command { ready, .. } = &mut entry.kind else {
            return;
        };
        if *ready {
            return;
        }
        *ready = true;
        let RunId::Command(command) = id else {
            return;
        };
        let mut owners = vec![];
        for dep in self.dependencies.values_mut() {
            if dep.view.command_run_id == Some(command)
                && matches!(dep.view.gate_state, DependencyGateState::WaitingForRelease)
            {
                dep.view.gate_state = DependencyGateState::Released {
                    reason: DependencyReleaseReason::ReadyMarker,
                };
                owners.push(owner_id(dep.view.owner_run_id));
            }
        }
        for owner in owners {
            self.progress_owner(owner);
        }
    }
    pub(super) fn users(&self, id: CommandRunId) -> Vec<RunId> {
        self.dependencies
            .values()
            .filter(|d| {
                d.view.command_run_id == Some(id)
                    && matches!(
                        d.view.gate_state,
                        DependencyGateState::WaitingForRelease
                            | DependencyGateState::Released {
                                reason: DependencyReleaseReason::ReadyMarker
                                    | DependencyReleaseReason::ManualContinue
                            }
                    )
            })
            .map(|d| owner_id(d.view.owner_run_id))
            .filter(|owner| self.entries.contains_key(owner))
            .collect()
    }
    pub(super) fn lifetime(
        &mut self,
        id: CommandRunId,
        policy: CommandLifetimePolicy,
    ) -> Result<()> {
        let entry = self
            .entries
            .get_mut(&RunId::Command(id))
            .ok_or_else(not_found)?;
        if entry.stopping() {
            return Err(invalid_state());
        }
        let Kind::Command { lifetime, .. } = &mut entry.kind else {
            unreachable!()
        };
        *lifetime = policy;
        if policy == CommandLifetimePolicy::Dependent && self.users(id).is_empty() {
            self.stop(RunId::Command(id), None, None);
        }
        Ok(())
    }
    pub(super) fn admit(&self) -> Result<()> {
        if self.closing {
            Err(TerminalError::closed())
        } else {
            Ok(())
        }
    }
}
fn name(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.contains('\0') {
        Err(TerminalError::invalid(
            "name must not be empty or contain NUL",
        ))
    } else {
        Ok(())
    }
}
fn endpoint(endpoint: &TunnelEndpoint) -> Result<()> {
    if endpoint.host.is_empty() || endpoint.host.contains('\0') || endpoint.port == 0 {
        Err(TerminalError::invalid(
            "endpoint requires a host and nonzero port",
        ))
    } else {
        Ok(())
    }
}
fn validate_ssh(target: &SshStartTarget) -> Result<()> {
    name(&target.config_name)?;
    if target.summary.username.is_empty()
        || target.summary.username.contains('\0')
        || target.summary.port == 0
    {
        return Err(TerminalError::invalid("invalid SSH username or port"));
    }
    match &target.summary.address {
        SshAddressSource::Host(host) => name(host),
        SshAddressSource::Device(id) => name(&id.0),
    }
}
fn validate_command(command: &CommandStartSpec) -> Result<()> {
    name(&command.config_name)?;
    if command.command.trim().is_empty() || command.command.contains('\0') {
        return Err(TerminalError::invalid(
            "command must not be empty or contain NUL",
        ));
    }
    if let CommandStartTarget::Ssh(target) = &command.target {
        validate_ssh(target)?;
    }
    Ok(())
}
fn validate_dependencies(dependencies: &[DependencyStartSpec]) -> Result<()> {
    for dependency in dependencies {
        validate_command(&dependency.command)?;
    }
    Ok(())
}
