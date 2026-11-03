use super::*;

struct NoSsh;
impl SshHostVerifier for NoSsh {
    fn verify(
        &self,
        _: SshHostVerificationRequest,
        _: CancellationToken,
    ) -> BoxFuture<'_, Result<()>> {
        Box::pin(async { unreachable!("scheduler test has no backend") })
    }
}

#[tokio::test]
async fn unconfirmed_stop_keeps_occupancy_and_fails_existing_and_new_shared_waiters() {
    let options = TerminalServiceOptions::default();
    let shared = Shared::new(options.clone());
    let (_requests, inbox) = mpsc::channel(options.control_queue_capacity);
    let mut manager = Manager::new(
        shared,
        options,
        Arc::new(NoSsh),
        inbox,
        CancellationToken::new(),
        None,
    );
    let command = CommandStartSpec {
        auto_inputs: vec![],
        config_id: CommandConfigId::new(),
        config_name: "unconfirmed process".into(),
        target: CommandStartTarget::Local,
        command: "never launched".into(),
    };
    let id = CommandRunId::new();
    let run = RunId::Command(id);
    // Inject a backend whose stop cannot be confirmed, without creating an unkillable OS process.
    manager.new_entry(
        run,
        command.config_name.clone(),
        RunTarget::Local,
        Kind::Command {
            config_id: command.config_id,
            command: command.command.clone(),
            lifetime: CommandLifetimePolicy::Independent,
            ready: false,
            auto_input_error: None,
        },
        DriverSpec::Local {
            program: Program::Command(command.command.clone()),
        },
        vec![],
    );
    manager.entries.get_mut(&run).unwrap().phase = Phase::Running;
    manager.event(DriverEvent::AutoInputFailed {
        id: run,
        error: TerminalError::new(TerminalErrorKind::ProcessFailed, "input write failed"),
    });
    assert!(manager.entries[&run].command().auto_input_error.is_some());
    assert!(matches!(manager.entries[&run].phase, Phase::Running));
    assert!(manager.tasks.is_empty()); // Reporting a write failure neither retries nor stops the run.
    manager.entries.get_mut(&run).unwrap().phase = Phase::Stopping(None);
    let request = StartCommandRequest {
        command,
        reuse_policy: CommandReusePolicy::Shared,
    };
    let (reply, receive) = tokio::sync::oneshot::channel();
    manager.start_command(request.clone(), CancellationToken::new(), false, reply);
    assert_eq!(manager.pending.len(), 1);
    manager.timer(Timer::StopExpired(run));
    assert_eq!(
        receive.await.unwrap().err().unwrap().kind,
        TerminalErrorKind::StopTimedOut
    );
    assert!(manager.pending.is_empty());
    let (reply, receive) = tokio::sync::oneshot::channel();
    manager.start_command(request, CancellationToken::new(), false, reply);
    assert_eq!(
        receive.await.unwrap().err().unwrap().kind,
        TerminalErrorKind::StopTimedOut
    );
    manager.ready(run); // A late marker cannot resurrect the stopping instance.
    manager.begin_shutdown(None);
    assert_eq!(manager.entries.len(), 1);
    assert!(matches!(
        manager.entries[&run].phase,
        Phase::Stopping(Some(_))
    ));
    assert!(manager.tasks.is_empty());
    assert!(!manager.entries[&run].command().ready_marker_received);
    manager.finish(run, Ok(())); // Only actual completion clears occupancy.
    assert!(manager.entries.is_empty());
}

#[tokio::test]
async fn a_confirmed_stop_releases_configuration_occupancy_before_replying() {
    type Stop = tokio::sync::oneshot::Receiver<Result<()>>;
    /// Holds the stop reply; on release, records whether that reply had already arrived.
    #[derive(Clone, Default)]
    struct Probe(Arc<Mutex<(Option<Stop>, Option<bool>)>>);
    struct Lease(Probe);
    impl Drop for Lease {
        fn drop(&mut self) {
            let (stop, replied) = &mut *self.0.0.lock().unwrap();
            *replied = Some(stop.as_mut().is_some_and(|stop| stop.try_recv().is_ok()));
        }
    }
    impl RunLeaseProvider for Probe {
        fn acquire(&self, _: RunId, _: &[TerminalConfigReference]) -> Box<dyn RunLease> {
            Box::new(Lease(self.clone()))
        }
    }
    let probe = Probe::default();
    let options = TerminalServiceOptions::default();
    let (_requests, inbox) = mpsc::channel(options.control_queue_capacity);
    let mut manager = Manager::new(
        Shared::new(options.clone()),
        options,
        Arc::new(NoSsh),
        inbox,
        CancellationToken::new(),
        Some(Arc::new(probe.clone())),
    );
    let run = RunId::Terminal(TerminalRunId::new());
    manager.new_entry(
        run,
        "configured terminal".into(),
        RunTarget::Local,
        Kind::Terminal(Some(TerminalConfigId::new())),
        DriverSpec::Local {
            program: Program::Shell,
        },
        vec![],
    );
    let (reply, stop) = tokio::sync::oneshot::channel();
    probe.0.lock().unwrap().0 = Some(stop);
    // The entry never launched, so stopping it ends and removes it at once.
    manager.stop(run, Some(reply), None);
    assert!(manager.entries.is_empty());
    let (stop, replied) = std::mem::take(&mut *probe.0.lock().unwrap());
    // A caller that hears the stop confirmed may delete the configuration right away.
    assert_eq!(
        replied,
        Some(false),
        "the stop was confirmed before its lease was released"
    );
    assert!(stop.unwrap().await.unwrap().is_ok());
}
