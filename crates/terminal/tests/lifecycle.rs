#![cfg(unix)]
use nooboard_terminal::*;
use std::{sync::Arc, time::Duration};
struct Reject;
impl SshHostVerifier for Reject {
    fn verify(
        &self,
        _: SshHostVerificationRequest,
        _: CancellationToken,
    ) -> BoxFuture<'_, Result<()>> {
        Box::pin(async {
            Err(TerminalError::new(
                TerminalErrorKind::HostKeyRejected,
                "test does not allow SSH",
            ))
        })
    }
}
fn service() -> (TerminalService, Terminal) {
    let mut options = TerminalServiceOptions::default();
    options.stop.stop_grace_timeout = Duration::from_millis(150);
    options.stop.force_stop_timeout = Duration::from_secs(2);
    options.stop.shutdown_timeout = Duration::from_secs(5);
    let service = TerminalService::start(
        options,
        TerminalServiceDependencies {
            host_verifier: Arc::new(Reject),
        },
    )
    .unwrap();
    let terminal = service.handle();
    (service, terminal)
}
fn command(text: &str) -> CommandStartSpec {
    CommandStartSpec {
        config_id: CommandConfigId::new(),
        config_name: "test command".into(),
        target: CommandStartTarget::Local,
        command: text.into(),
    }
}
fn root(dependencies: Vec<DependencyStartSpec>) -> StartTerminalRequest {
    StartTerminalRequest {
        config_id: Some(TerminalConfigId::new()),
        name: "test terminal".into(),
        target: TerminalStartTarget::Local,
        dependencies,
    }
}
fn dependency(command: CommandStartSpec) -> DependencyStartSpec {
    DependencyStartSpec {
        command,
        reuse_policy: CommandReusePolicy::Shared,
    }
}
async fn state(terminal: &Terminal, predicate: impl Fn(&TerminalSnapshot) -> bool) {
    let mut subscription = terminal.subscribe();
    tokio::time::timeout(Duration::from_secs(6), async {
        if predicate(&subscription.initial_snapshot) {
            return;
        }
        while let Some(snapshot) = subscription.snapshots.next().await {
            if predicate(&snapshot) {
                return;
            }
        }
        panic!("service closed before expected state");
    })
    .await
    .unwrap_or_else(|_| panic!("unexpected state: {:?}", terminal.snapshot()));
}
async fn command_end(events: &mut TerminalEventStream, id: CommandRunId) -> CommandRun {
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            if let Some(TerminalEvent::CommandEnded { run }) = events.next().await.unwrap()
                && run.id == id
            {
                return run;
            }
        }
    })
    .await
    .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn successful_and_failed_commands_deliver_results_without_history() {
    let (mut service, terminal) = service();
    let mut events = terminal.subscribe().events;
    for (text, success) in [("exit 0", true), ("exit 7", false)] {
        let id = terminal
            .start_command(
                StartCommandRequest {
                    command: command(text),
                    reuse_policy: CommandReusePolicy::AlwaysNew,
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let run = command_end(&mut events, id).await;
        assert_eq!(
            matches!(
                run.state,
                CommandRunState::Ended {
                    result: CommandRunResult::Succeeded
                }
            ),
            success,
            "{run:?}"
        );
        assert!(!terminal.snapshot().command_runs.iter().any(|r| r.id == id));
        terminal.stop_command(id).await.unwrap();
    }
    service.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_dependencies_are_single_instance_and_release_on_last_owner() {
    let (mut service, terminal) = service();
    let spec = command("printf '::nooboard:v1:ready::\\n'; exec sleep 30");
    let (a, b) = tokio::join!(
        terminal.start_terminal(root(vec![dependency(spec.clone())])),
        terminal.start_terminal(root(vec![dependency(spec)]))
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    state(&terminal, |s| {
        s.terminal_runs.len() == 2
            && s.terminal_runs
                .iter()
                .all(|r| matches!(r.state, TerminalRunState::Running))
    })
    .await;
    let snapshot = terminal.snapshot();
    assert_eq!(snapshot.command_runs.len(), 1);
    assert!(snapshot.command_runs[0].ready_marker_received);
    let mut output = terminal
        .subscribe_output(TerminalIoTarget::Command(snapshot.command_runs[0].id))
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&output.initial_output.data).contains("::nooboard"));
    terminal.stop_terminal(a).await.unwrap();
    assert_eq!(terminal.snapshot().command_runs.len(), 1);
    terminal.stop_terminal(b).await.unwrap();
    state(&terminal, TerminalSnapshot::is_empty).await;
    assert!(output.stream.next().await.unwrap().is_none());
    service.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interactive_ready_prefix_releases_dependencies_before_the_line_ends() {
    let (mut service, terminal) = service();
    // The command stays alive awaiting a second input after emitting the prefix.
    // Its later CRLF becomes CRCRLF in the PTY, as with nested sudo PTYs.
    let spec = command(
        "stty -echo -echonl; printf 'INPUT_READY\\n'; read -r answer; \
         printf '::nooboard:v1:ready::'; read -r continue; \
         printf '\\r\\nAFTER_MARKER\\n'; exec sleep 30",
    );
    let (id, mut output) = terminal
        .start_command_with_output(
            StartCommandRequest {
                command: spec.clone(),
                reuse_policy: CommandReusePolicy::Shared,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let owner = terminal
        .start_terminal(root(vec![dependency(spec)]))
        .await
        .unwrap();
    state(&terminal, |s| {
        s.dependency_runs.iter().any(|dep| {
            dep.command_run_id == Some(id)
                && matches!(dep.gate_state, DependencyGateState::WaitingForRelease)
        })
    })
    .await;
    let mut data = output.initial_output.data;
    tokio::time::timeout(Duration::from_secs(6), async {
        while !String::from_utf8_lossy(&data).contains("INPUT_READY") {
            data.extend(output.stream.next().await.unwrap().unwrap().data);
        }
    })
    .await
    .unwrap();
    terminal
        .write_input(TerminalIoTarget::Command(id), b"answer\n".to_vec())
        .await
        .unwrap();
    state(&terminal, |s| {
        s.terminal_runs
            .iter()
            .any(|run| run.id == owner && matches!(run.state, TerminalRunState::Running))
    })
    .await;
    let snapshot = terminal.snapshot();
    assert!(snapshot.command_runs[0].ready_marker_received);
    assert!(matches!(
        snapshot.dependency_runs[0].gate_state,
        DependencyGateState::Released {
            reason: DependencyReleaseReason::ReadyMarker
        }
    ));
    terminal
        .write_input(TerminalIoTarget::Command(id), b"continue\n".to_vec())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(6), async {
        while !data.ends_with(b"AFTER_MARKER\r\n") {
            data.extend(output.stream.next().await.unwrap().unwrap().data);
        }
    })
    .await
    .unwrap();
    assert_eq!(data, b"INPUT_READY\r\nAFTER_MARKER\r\n");
    service.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_release_is_per_dependency_and_readiness_is_not_faked() {
    let (mut service, terminal) = service();
    let spec = command("exec sleep 30");
    let a = terminal
        .start_terminal(root(vec![dependency(spec.clone())]))
        .await
        .unwrap();
    let b = terminal
        .start_terminal(root(vec![dependency(spec)]))
        .await
        .unwrap();
    state(&terminal, |s| {
        s.dependency_runs.len() == 2
            && s.dependency_runs
                .iter()
                .all(|d| matches!(d.gate_state, DependencyGateState::WaitingForRelease))
    })
    .await;
    let dep = terminal
        .snapshot()
        .dependency_runs
        .iter()
        .find(|d| d.owner_run_id == DependencyOwnerRunId::Terminal(a))
        .unwrap()
        .id;
    terminal.continue_dependency(dep).await.unwrap();
    state(&terminal, |s| {
        s.terminal_runs
            .iter()
            .any(|r| r.id == a && matches!(r.state, TerminalRunState::Running))
    })
    .await;
    let snapshot = terminal.snapshot();
    assert!(!snapshot.command_runs[0].ready_marker_received);
    assert!(
        snapshot
            .terminal_runs
            .iter()
            .any(|r| r.id == b && matches!(r.state, TerminalRunState::WaitingForDependencies))
    );
    service.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_chooses_latest_and_does_not_change_lifetime() {
    let (mut service, terminal) = service();
    let spec = command("exec sleep 30");
    let a = terminal
        .start_command(
            StartCommandRequest {
                command: spec.clone(),
                reuse_policy: CommandReusePolicy::AlwaysNew,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let b = terminal
        .start_command(
            StartCommandRequest {
                command: spec.clone(),
                reuse_policy: CommandReusePolicy::AlwaysNew,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let reused = terminal
        .start_command(
            StartCommandRequest {
                command: spec,
                reuse_policy: CommandReusePolicy::Shared,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(reused, b);
    assert_ne!(a, b);
    terminal
        .set_command_lifetime_policy(b, CommandLifetimePolicy::Dependent)
        .await
        .unwrap();
    state(&terminal, |s| s.command_runs.iter().all(|r| r.id != b)).await;
    assert!(
        terminal
            .snapshot()
            .command_runs
            .iter()
            .any(|r| r.id == a && r.lifetime_policy == CommandLifetimePolicy::Independent)
    );
    service.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sustained_dependency_exit_even_zero_ends_owners() {
    let (mut service, terminal) = service();
    let spec = command("printf '::nooboard:v1:ready::\\n'; read -r answer; exit 0");
    let owner = terminal
        .start_terminal(root(vec![dependency(spec)]))
        .await
        .unwrap();
    state(&terminal, |s| {
        s.terminal_runs
            .iter()
            .any(|r| r.id == owner && matches!(r.state, TerminalRunState::Running))
    })
    .await;
    let command = terminal.snapshot().command_runs[0].id;
    let mut events = terminal.subscribe().events;
    terminal
        .write_input(TerminalIoTarget::Command(command), b"done\n".to_vec())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            if let Some(TerminalEvent::TerminalEnded { run }) = events.next().await.unwrap()
                && run.id == owner
            {
                assert!(matches!(
                    run.state,
                    TerminalRunState::Ended {
                        result: TerminalRunResult::Failed(TerminalError {
                            kind: TerminalErrorKind::DependencyFailed,
                            ..
                        })
                    }
                ));
                break;
            }
        }
    })
    .await
    .unwrap();
    service.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stopping_shared_instance_is_not_reused_and_cancelled_waiter_does_not_spawn() {
    let (mut service, terminal) = service();
    let spec =
        command("trap '' TERM; printf '::nooboard:v1:ready::\\n'; while :; do sleep 1; done");
    let id = terminal
        .start_command(
            StartCommandRequest {
                command: spec.clone(),
                reuse_policy: CommandReusePolicy::Shared,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    state(&terminal, |s| {
        s.command_runs
            .iter()
            .any(|r| r.id == id && r.ready_marker_received)
    })
    .await;
    let stop_handle = terminal.clone();
    let stopping = tokio::spawn(async move { stop_handle.stop_command(id).await });
    state(&terminal, |s| {
        s.command_runs
            .iter()
            .any(|r| r.id == id && matches!(r.state, CommandRunState::Stopping { .. }))
    })
    .await;
    let cancel = CancellationToken::new();
    let waiting_handle = terminal.clone();
    let request_cancel = cancel.clone();
    let waiting = tokio::spawn(async move {
        waiting_handle
            .start_command(
                StartCommandRequest {
                    command: spec,
                    reuse_policy: CommandReusePolicy::Shared,
                },
                request_cancel,
            )
            .await
    });
    tokio::task::yield_now().await;
    cancel.cancel();
    assert_eq!(
        waiting.await.unwrap().unwrap_err().kind,
        TerminalErrorKind::Cancelled
    );
    stopping.await.unwrap().unwrap();
    state(&terminal, TerminalSnapshot::is_empty).await;
    service.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_shared_requests_use_first_waiters_snapshot_and_lifetime() {
    let (mut service, terminal) = service();
    let initial =
        command("trap '' TERM; printf '::nooboard:v1:ready::\\n'; while :; do sleep 1; done");
    let id = terminal
        .start_command(
            StartCommandRequest {
                command: initial.clone(),
                reuse_policy: CommandReusePolicy::Shared,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    state(&terminal, |s| {
        s.command_runs
            .iter()
            .any(|r| r.id == id && r.ready_marker_received)
    })
    .await;
    let stopper = terminal.clone();
    let stopping = tokio::spawn(async move { stopper.stop_command(id).await });
    state(&terminal, |s| {
        s.command_runs
            .iter()
            .any(|r| r.id == id && matches!(r.state, CommandRunState::Stopping { .. }))
    })
    .await;
    let mut first = initial.clone();
    first.command = "printf '::nooboard:v1:ready::\\n'; exec sleep 30".into();
    let owner = terminal
        .start_terminal(root(vec![dependency(first.clone())]))
        .await
        .unwrap();
    let mut later = initial;
    later.command = "exit 7".into();
    let acquired = terminal
        .start_command(
            StartCommandRequest {
                command: later,
                reuse_policy: CommandReusePolicy::Shared,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    stopping.await.unwrap().unwrap();
    assert_ne!(acquired, id);
    state(&terminal, |s| {
        s.terminal_runs
            .iter()
            .any(|r| r.id == owner && matches!(r.state, TerminalRunState::Running))
    })
    .await;
    let snapshot = terminal.snapshot();
    assert_eq!(snapshot.command_runs.len(), 1);
    assert_eq!(snapshot.command_runs[0].command, first.command);
    assert_eq!(
        snapshot.command_runs[0].lifetime_policy,
        CommandLifetimePolicy::Dependent
    );
    terminal.stop_terminal(owner).await.unwrap();
    state(&terminal, TerminalSnapshot::is_empty).await;
    service.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn successful_preparation_survives_removal_of_the_command_record() {
    let (mut service, terminal) = service();
    let owner = terminal
        .start_terminal(root(vec![dependency(command("exit 0"))]))
        .await
        .unwrap();
    state(&terminal, |s| {
        s.terminal_runs
            .iter()
            .any(|r| r.id == owner && matches!(r.state, TerminalRunState::Running))
    })
    .await;
    let snapshot = terminal.snapshot();
    assert!(snapshot.command_runs.is_empty());
    assert!(matches!(
        snapshot.dependency_runs[0].gate_state,
        DependencyGateState::Released {
            reason: DependencyReleaseReason::SuccessfulExit
        }
    ));
    service.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn atomic_start_output_captures_an_immediately_exiting_command() {
    let (mut service, terminal) = service();
    let mut events = terminal.subscribe().events;
    let (id, mut output) = terminal
        .start_command_with_output(
            StartCommandRequest {
                command: command("printf 'quick output\\n'"),
                reuse_policy: CommandReusePolicy::AlwaysNew,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let mut data = output.initial_output.data;
    while let Some(chunk) = output.stream.next().await.unwrap() {
        data.extend(chunk.data);
    }
    assert!(String::from_utf8_lossy(&data).contains("quick output"));
    command_end(&mut events, id).await;
    assert!(terminal.snapshot().command_runs.is_empty());
    service.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_command_helpers_can_read_the_controlling_tty_with_piped_stdin() {
    let (mut service, terminal) = service();
    let mut events = terminal.subscribe().events;
    // Like an askpass helper: stdin carries a payload while interactive input
    // comes from /dev/tty. No credentials, privilege escalation or network I/O.
    let script = r#"printf 'test payload\n' | /bin/sh -c '
        IFS= read -r payload
        printf "HELPER_READY\n" >/dev/tty
        IFS= read -r answer </dev/tty
        [ "$payload" = "test payload" ] && [ "$answer" = "test answer" ] || exit 9
        printf "HELPER_INPUT_OK\n" >/dev/tty
    '"#;
    let (id, mut output) = terminal
        .start_command_with_output(
            StartCommandRequest {
                command: command(script),
                reuse_policy: CommandReusePolicy::AlwaysNew,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let mut data = output.initial_output.data;
    tokio::time::timeout(Duration::from_secs(6), async {
        while !String::from_utf8_lossy(&data).contains("HELPER_READY") {
            data.extend(
                output
                    .stream
                    .next()
                    .await
                    .unwrap()
                    .expect("helper exited early")
                    .data,
            );
        }
        terminal
            .write_input(TerminalIoTarget::Command(id), b"test answer\n".to_vec())
            .await
            .unwrap();
        while let Some(chunk) = output.stream.next().await.unwrap() {
            data.extend(chunk.data);
        }
    })
    .await
    .expect("interactive command timed out");
    let run = command_end(&mut events, id).await;
    assert!(
        matches!(
            run.state,
            CommandRunState::Ended {
                result: CommandRunResult::Succeeded
            }
        ),
        "{run:?}: {}",
        String::from_utf8_lossy(&data)
    );
    assert!(String::from_utf8_lossy(&data).contains("HELPER_INPUT_OK"));
    service.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slow_event_subscriber_is_explicitly_disconnected_without_blocking_runs() {
    let (mut service, terminal) = service();
    let options = TerminalServiceOptions {
        event_buffer_capacity: 1,
        ..Default::default()
    };
    terminal.update_options(options).await.unwrap();
    let mut slow = terminal.subscribe().events;
    for _ in 0..2 {
        let mut fast = terminal.subscribe().events;
        let id = terminal
            .start_command(
                StartCommandRequest {
                    command: command("exit 0"),
                    reuse_policy: CommandReusePolicy::AlwaysNew,
                },
                CancellationToken::new(),
            )
            .await
            .unwrap();
        command_end(&mut fast, id).await;
    }
    assert_eq!(
        slow.next().await.unwrap_err(),
        TerminalSubscriptionError::Lagged
    );
    assert!(slow.next().await.unwrap().is_none());
    service.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn temporary_terminals_lease_only_their_real_dependencies() {
    #[derive(Default)]
    struct Leases(std::sync::Mutex<Vec<(RunId, Vec<TerminalConfigReference>)>>);
    impl RunLeaseProvider for Leases {
        fn acquire(&self, run: RunId, references: &[TerminalConfigReference]) -> Box<dyn RunLease> {
            self.0.lock().unwrap().push((run, references.to_vec()));
            Box::new(())
        }
    }
    let leases = Arc::new(Leases::default());
    let mut service = TerminalService::start_with_leases(
        TerminalServiceOptions::default(),
        TerminalServiceDependencies {
            host_verifier: Arc::new(Reject),
        },
        leases.clone(),
    )
    .unwrap();
    let terminal = service.handle();
    let mut request = root(vec![]);
    request.config_id = None;
    let plain = terminal.start_terminal(request).await.unwrap();
    state(&terminal, |s| s.terminal_runs.iter().any(|r| r.id == plain)).await;
    assert!(leases.0.lock().unwrap().is_empty());
    let plain_view = terminal
        .snapshot()
        .terminal_runs
        .into_iter()
        .find(|r| r.id == plain)
        .unwrap();
    assert_eq!(plain_view.config_id, None);
    assert_eq!(plain_view.name, "test terminal");
    terminal.stop_terminal(plain).await.unwrap();

    let command = command("exec sleep 30");
    let command_id = command.config_id;
    let mut request = root(vec![dependency(command)]);
    request.config_id = None;
    let dependent = terminal.start_terminal(request).await.unwrap();
    assert!(leases.0.lock().unwrap().iter().any(|(run, refs)| {
        *run == RunId::Terminal(dependent)
            && refs == &[TerminalConfigReference::Command(command_id)]
    }));
    assert!(leases.0.lock().unwrap().iter().all(|(_, refs)| {
        refs.iter()
            .all(|reference| *reference == TerminalConfigReference::Command(command_id))
    }));
    service.shutdown().await.unwrap();
    assert!(terminal.snapshot().is_empty());
}
