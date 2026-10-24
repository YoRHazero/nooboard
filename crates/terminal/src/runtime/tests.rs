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
        },
        DriverSpec::Local {
            command: Some(command.command.clone()),
        },
        vec![],
    );
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
