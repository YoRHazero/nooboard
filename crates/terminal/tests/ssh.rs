mod support;
use nooboard_terminal::*;
use std::{sync::atomic::Ordering, time::Duration};
use support::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[tokio::test]
async fn host_confirmation_can_outlast_connection_timeout_and_is_cancellable() {
    use std::sync::{Arc, atomic::AtomicUsize};
    struct WaitingTrust {
        key: SshPublicKey,
        checked: Arc<AtomicUsize>,
        entered: CancellationToken,
        allow: CancellationToken,
    }
    impl SshHostVerifier for WaitingTrust {
        fn verify(
            &self,
            request: SshHostVerificationRequest,
            cancellation: CancellationToken,
        ) -> BoxFuture<'_, Result<()>> {
            Box::pin(async move {
                assert_eq!(request.server_key, self.key);
                self.checked.fetch_add(1, Ordering::SeqCst);
                self.entered.cancel();
                tokio::select! {
                    _ = self.allow.cancelled() => Ok(()),
                    _ = cancellation.cancelled() => Err(TerminalError::new(TerminalErrorKind::Cancelled, "confirmation cancelled")),
                }
            })
        }
    }
    let server = Server::start(Settings::default()).await;
    for cancel in [false, true] {
        let checked = Arc::new(AtomicUsize::new(0));
        let auth = AuthProvider::new(checked.clone());
        let entered = CancellationToken::new();
        let allow = CancellationToken::new();
        let mut options = TerminalServiceOptions::default();
        options.io.connect_timeout = Duration::from_millis(200);
        let mut service = TerminalService::start(
            options,
            TerminalServiceDependencies {
                host_verifier: Arc::new(WaitingTrust {
                    key: server.key.clone(),
                    checked,
                    entered: entered.clone(),
                    allow: allow.clone(),
                }),
            },
        )
        .unwrap();
        let client = service.handle();
        let id = client
            .start_terminal(StartTerminalRequest {
                config_id: Some(TerminalConfigId::new()),
                name: "waiting for trust".into(),
                target: TerminalStartTarget::Ssh(server.target(auth.clone())),
                dependencies: vec![],
            })
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), entered.cancelled())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(350)).await;
        assert!(
            client
                .snapshot()
                .terminal_runs
                .iter()
                .any(|run| run.id == id && matches!(run.state, TerminalRunState::Starting))
        );
        assert_eq!(auth.prepared.load(Ordering::SeqCst), 0);
        if cancel {
            client.stop_terminal(id).await.unwrap();
            allow.cancel(); // Late confirmation must not recreate or authenticate the Run.
            assert_eq!(auth.prepared.load(Ordering::SeqCst), 0);
            assert_eq!(auth.succeeded.load(Ordering::SeqCst), 0);
        } else {
            allow.cancel();
            wait(&client, |s| {
                s.terminal_runs
                    .iter()
                    .any(|run| run.id == id && matches!(run.state, TerminalRunState::Running))
            })
            .await;
            assert_eq!(auth.succeeded.load(Ordering::SeqCst), 1);
            client.stop_terminal(id).await.unwrap();
        }
        service.shutdown().await.unwrap();
    }
}

async fn file_end(events: &mut TerminalEventStream, id: FileOperationRunId) -> FileOperationRun {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if let Some(TerminalEvent::FileOperationEnded { run }) = events.next().await.unwrap()
                && run.id == id
            {
                return run;
            }
        }
    })
    .await
    .unwrap()
}
async fn transfer(
    client: &Terminal,
    parent: TerminalRunId,
    operation: FileOperationSpec,
) -> FileOperationRun {
    let mut events = client.subscribe().events;
    let id = client
        .start_file_operation(StartFileOperationRequest {
            terminal_run_id: parent,
            operation,
        })
        .await
        .unwrap();
    file_end(&mut events, id).await
}
fn succeeded(run: &FileOperationRun) {
    assert!(
        matches!(
            run.state,
            FileOperationRunState::Ended {
                result: FileOperationRunResult::Succeeded,
                ..
            }
        ),
        "{run:?}"
    );
}
#[tokio::test]
async fn host_trust_precedes_credentials_and_rejection_sends_no_password() {
    let server = Server::start(Settings::default()).await;
    let (mut service, client, auth) = service(&server, true);
    let mut events = client.subscribe().events;
    let id = client
        .start_terminal(StartTerminalRequest {
            config_id: Some(TerminalConfigId::new()),
            name: "rejected".into(),
            target: TerminalStartTarget::Ssh(server.target(auth.clone())),
            dependencies: vec![],
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(TerminalEvent::TerminalEnded { run }) = events.next().await.unwrap()
                && run.id == id
            {
                assert!(matches!(
                    run.state,
                    TerminalRunState::Ended {
                        result: TerminalRunResult::Failed(TerminalError {
                            kind: TerminalErrorKind::HostKeyRejected,
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
    assert_eq!(auth.prepared.load(Ordering::SeqCst), 0);
    assert_eq!(server.data.passwords.load(Ordering::SeqCst), 0);
    assert_eq!(auth.succeeded.load(Ordering::SeqCst), 0);
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn terminal_and_lazy_sftp_share_one_authenticated_connection() {
    let server = Server::start(Settings::default()).await;
    std::fs::write(server.directory.path().join("file.txt"), b"test").unwrap();
    let (mut service, client, auth) = service(&server, false);
    let id = terminal(&client, server.target(auth.clone())).await;
    assert_eq!(server.data.subsystems.load(Ordering::SeqCst), 0);
    let (a, b) = tokio::join!(
        client.list_directory(id, None, CancellationToken::new()),
        client.list_directory(id, None, CancellationToken::new())
    );
    assert!(a.unwrap().entries.iter().any(|e| e.name == "file.txt"));
    b.unwrap();
    assert_eq!(server.data.subsystems.load(Ordering::SeqCst), 1);
    assert_eq!(server.data.transports.load(Ordering::SeqCst), 1);
    assert_eq!(auth.succeeded.load(Ordering::SeqCst), 1);
    let mut output = client
        .subscribe_output(TerminalIoTarget::Terminal(id))
        .await
        .unwrap();
    client
        .write_input(TerminalIoTarget::Terminal(id), b"echo-test".to_vec())
        .await
        .unwrap();
    let bytes = tokio::time::timeout(Duration::from_secs(2), output.stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .data;
    assert_eq!(bytes, b"echo-test");
    client.stop_terminal(id).await.unwrap();
    assert!(client.snapshot().is_empty());
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn recursive_transfer_conflicts_safe_overwrite_and_delete() {
    let server = Server::start(Settings {
        atomic_replace: true,
        ..Default::default()
    })
    .await;
    let (mut service, client, auth) = service(&server, false);
    let id = terminal(&client, server.target(auth)).await;
    let local = tempfile::tempdir().unwrap();
    let source = local.path().join("source");
    std::fs::create_dir_all(source.join("nested")).unwrap();
    std::fs::write(source.join("a.txt"), b"alpha").unwrap();
    std::fs::write(source.join("nested/b.txt"), b"beta").unwrap();
    let remote = RemotePath::new("/project").unwrap();
    let item = FileTransferItem {
        local_path: source.clone(),
        remote_path: remote.clone(),
    };
    succeeded(
        &transfer(
            &client,
            id,
            FileOperationSpec::Upload {
                items: vec![item.clone()],
                conflict_policy: FileConflictPolicy::Fail,
            },
        )
        .await,
    );
    assert_eq!(
        std::fs::read(server.directory.path().join("project/nested/b.txt")).unwrap(),
        b"beta"
    );
    let failed = transfer(
        &client,
        id,
        FileOperationSpec::Upload {
            items: vec![item.clone()],
            conflict_policy: FileConflictPolicy::Fail,
        },
    )
    .await;
    assert!(matches!(
        failed.state,
        FileOperationRunState::Ended {
            result: FileOperationRunResult::Failed(TerminalError {
                kind: TerminalErrorKind::Conflict,
                ..
            }),
            ..
        }
    ));
    let skipped = transfer(
        &client,
        id,
        FileOperationSpec::Upload {
            items: vec![item.clone()],
            conflict_policy: FileConflictPolicy::Skip,
        },
    )
    .await;
    succeeded(&skipped);
    assert_eq!(skipped.progress.skipped_entries, 2);
    std::fs::write(source.join("a.txt"), b"replaced").unwrap();
    succeeded(
        &transfer(
            &client,
            id,
            FileOperationSpec::Upload {
                items: vec![item],
                conflict_policy: FileConflictPolicy::Overwrite,
            },
        )
        .await,
    );
    let destination = local.path().join("download");
    succeeded(
        &transfer(
            &client,
            id,
            FileOperationSpec::Download {
                items: vec![FileTransferItem {
                    local_path: destination.clone(),
                    remote_path: remote.clone(),
                }],
                conflict_policy: FileConflictPolicy::Fail,
            },
        )
        .await,
    );
    assert_eq!(
        std::fs::read(destination.join("a.txt")).unwrap(),
        b"replaced"
    );
    client
        .rename_path(
            id,
            RemotePath::new("/project/a.txt").unwrap(),
            RemotePath::new("/project/renamed.txt").unwrap(),
        )
        .await
        .unwrap();
    client
        .create_directory(id, RemotePath::new("/empty").unwrap())
        .await
        .unwrap();
    succeeded(
        &transfer(
            &client,
            id,
            FileOperationSpec::Delete {
                paths: vec![remote, RemotePath::new("/empty").unwrap()],
            },
        )
        .await,
    );
    assert_eq!(
        std::fs::read_dir(server.directory.path()).unwrap().count(),
        0
    );
    service.shutdown().await.unwrap();
}
#[cfg(unix)]
#[tokio::test]
async fn symlinks_fail_transfer_but_delete_only_removes_the_link() {
    let server = Server::start(Settings::default()).await;
    let (mut service, client, auth) = service(&server, false);
    let id = terminal(&client, server.target(auth)).await;
    let local = tempfile::tempdir().unwrap();
    let original = local.path().join("original");
    std::fs::write(&original, b"keep").unwrap();
    let link = local.path().join("link");
    std::os::unix::fs::symlink(&original, &link).unwrap();
    let upload = transfer(
        &client,
        id,
        FileOperationSpec::Upload {
            items: vec![FileTransferItem {
                local_path: link,
                remote_path: RemotePath::new("/link").unwrap(),
            }],
            conflict_policy: FileConflictPolicy::Skip,
        },
    )
    .await;
    assert!(matches!(
        upload.state,
        FileOperationRunState::Ended {
            result: FileOperationRunResult::Failed(TerminalError {
                kind: TerminalErrorKind::Unsupported,
                ..
            }),
            ..
        }
    ));
    std::os::unix::fs::symlink(&original, server.directory.path().join("link")).unwrap();
    let download = transfer(
        &client,
        id,
        FileOperationSpec::Download {
            items: vec![FileTransferItem {
                local_path: local.path().join("download"),
                remote_path: RemotePath::new("/link").unwrap(),
            }],
            conflict_policy: FileConflictPolicy::Fail,
        },
    )
    .await;
    assert!(matches!(
        download.state,
        FileOperationRunState::Ended {
            result: FileOperationRunResult::Failed(TerminalError {
                kind: TerminalErrorKind::Unsupported,
                ..
            }),
            ..
        }
    ));
    succeeded(
        &transfer(
            &client,
            id,
            FileOperationSpec::Delete {
                paths: vec![RemotePath::new("/link").unwrap()],
            },
        )
        .await,
    );
    assert_eq!(std::fs::read(original).unwrap(), b"keep");
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn unsupported_atomic_overwrite_preserves_old_file() {
    let server = Server::start(Settings::default()).await;
    std::fs::write(server.directory.path().join("target"), b"old").unwrap();
    let (mut service, client, auth) = service(&server, false);
    let id = terminal(&client, server.target(auth)).await;
    let local = tempfile::tempdir().unwrap();
    let source = local.path().join("source");
    std::fs::write(&source, b"new").unwrap();
    let result = transfer(
        &client,
        id,
        FileOperationSpec::Upload {
            items: vec![FileTransferItem {
                local_path: source,
                remote_path: RemotePath::new("/target").unwrap(),
            }],
            conflict_policy: FileConflictPolicy::Overwrite,
        },
    )
    .await;
    assert!(matches!(
        result.state,
        FileOperationRunState::Ended {
            result: FileOperationRunResult::Failed(TerminalError {
                kind: TerminalErrorKind::Unsupported,
                ..
            }),
            ..
        }
    ));
    assert_eq!(
        std::fs::read(server.directory.path().join("target")).unwrap(),
        b"old"
    );
    assert_eq!(
        std::fs::read_dir(server.directory.path()).unwrap().count(),
        1
    );
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn lost_commit_acknowledgement_is_not_reported_as_an_untouched_target() {
    let server = Server::start(Settings {
        drop_commit_reply: true,
        ..Default::default()
    })
    .await;
    let (mut service, client, auth) = service(&server, false);
    let id = terminal(&client, server.target(auth)).await;
    let local = tempfile::tempdir().unwrap();
    let source = local.path().join("source");
    std::fs::write(&source, b"committed").unwrap();
    let result = transfer(
        &client,
        id,
        FileOperationSpec::Upload {
            items: vec![FileTransferItem {
                local_path: source,
                remote_path: RemotePath::new("/target").unwrap(),
            }],
            conflict_policy: FileConflictPolicy::Fail,
        },
    )
    .await;
    assert!(
        matches!(
            result.state,
            FileOperationRunState::Ended {
                result: FileOperationRunResult::Failed(TerminalError {
                    kind: TerminalErrorKind::CommitUnconfirmed,
                    ..
                }),
                ..
            }
        ),
        "{result:?}"
    );
    assert_eq!(
        std::fs::read(server.directory.path().join("target")).unwrap(),
        b"committed"
    );
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn sftp_failure_does_not_close_a_healthy_terminal() {
    let server = Server::start(Settings {
        deny_sftp: true,
        ..Default::default()
    })
    .await;
    let (mut service, client, auth) = service(&server, false);
    let id = terminal(&client, server.target(auth)).await;
    assert!(
        client
            .list_directory(id, None, CancellationToken::new())
            .await
            .is_err()
    );
    assert!(matches!(
        client.snapshot().terminal_runs[0].state,
        TerminalRunState::Running
    ));
    server.disconnect().await;
    wait(&client, TerminalSnapshot::is_empty).await;
    service.shutdown().await.unwrap();
}
async fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}
async fn echo_server() -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let mut tasks = tokio::task::JoinSet::new();
        loop {
            tokio::select! {accepted=listener.accept()=>{let(stream,_)=accepted.unwrap();tasks.spawn(async move{let(mut read,mut write)=stream.into_split();let _=tokio::io::copy(&mut read,&mut write).await;});},_=tasks.join_next(),if !tasks.is_empty()=>{}}
        }
    });
    (port, task)
}
#[tokio::test]
async fn local_remote_and_dynamic_tunnels_forward_real_bytes() {
    let server = Server::start(Settings::default()).await;
    let (mut service, client, auth) = service(&server, false);
    let (destination, echo) = echo_server().await;
    for mode in 0..3 {
        let port = free_port().await;
        let listen = TunnelEndpoint {
            host: "127.0.0.1".into(),
            port,
        };
        let target = TunnelEndpoint {
            host: "127.0.0.1".into(),
            port: destination,
        };
        let forward = match mode {
            0 => TunnelForwardConfig::Local {
                listen,
                destination: target,
            },
            1 => TunnelForwardConfig::Remote {
                listen,
                destination: target,
            },
            _ => TunnelForwardConfig::Dynamic { listen },
        };
        let id = client
            .start_tunnel(StartTunnelRequest {
                config_id: TunnelConfigId::new(),
                config_name: "test forwarding".into(),
                target: server.target(auth.clone()),
                forward,
                dependencies: vec![],
            })
            .await
            .unwrap();
        wait(&client, |s| {
            s.tunnel_runs
                .iter()
                .any(|r| r.id == id && matches!(r.state, TunnelRunState::Running))
        })
        .await;
        tokio::time::timeout(Duration::from_secs(4), async {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            if mode == 2 {
                stream.write_all(&[5, 1, 0]).await.unwrap();
                let mut reply = [0; 2];
                stream.read_exact(&mut reply).await.unwrap();
                assert_eq!(reply, [5, 0]);
                let mut connect = vec![5, 1, 0, 1, 127, 0, 0, 1];
                connect.extend(destination.to_be_bytes());
                stream.write_all(&connect).await.unwrap();
                let mut reply = [0; 10];
                stream.read_exact(&mut reply).await.unwrap();
                assert_eq!(reply[1], 0);
            }
            stream.write_all(b"forwarded").await.unwrap();
            let mut bytes = [0; 9];
            stream.read_exact(&mut bytes).await.unwrap();
            assert_eq!(&bytes, b"forwarded");
        })
        .await
        .unwrap();
        client.stop_tunnel(id).await.unwrap();
    }
    service.shutdown().await.unwrap();
    echo.abort();
    let _ = echo.await;
}

#[tokio::test]
async fn cancelling_upload_preserves_existing_target_and_cleans_owned_temporary_file() {
    let server = Server::start(Settings {
        atomic_replace: true,
        slow_writes: true,
        ..Default::default()
    })
    .await;
    std::fs::write(server.directory.path().join("target"), b"old contents").unwrap();
    let (mut service, client, auth) = service(&server, false);
    let parent = terminal(&client, server.target(auth)).await;
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), vec![7; 1024 * 1024]).unwrap();
    let mut events = client.subscribe().events;
    let id = client
        .start_file_operation(StartFileOperationRequest {
            terminal_run_id: parent,
            operation: FileOperationSpec::Upload {
                items: vec![FileTransferItem {
                    local_path: source.path().to_owned(),
                    remote_path: RemotePath::new("/target").unwrap(),
                }],
                conflict_policy: FileConflictPolicy::Overwrite,
            },
        })
        .await
        .unwrap();
    wait(&client, |s| {
        s.file_operation_runs
            .iter()
            .any(|r| r.id == id && r.progress.transferred_bytes > 0)
    })
    .await;
    client.stop_file_operation(id).await.unwrap();
    let result = file_end(&mut events, id).await;
    assert!(
        matches!(result.state,FileOperationRunState::Ended{result:FileOperationRunResult::Stopped,ref cleanup_issues}if cleanup_issues.is_empty()),
        "{result:?}"
    );
    assert_eq!(
        std::fs::read(server.directory.path().join("target")).unwrap(),
        b"old contents"
    );
    assert_eq!(
        std::fs::read_dir(server.directory.path()).unwrap().count(),
        1
    );
    assert!(matches!(
        client.snapshot().terminal_runs[0].state,
        TerminalRunState::Running
    ));
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn failed_authentication_never_reports_password_ready_to_save() {
    use std::sync::Arc;
    let server = Server::start(Settings::default()).await;
    let (mut service, client, auth) = service(&server, false);
    let rejected = Arc::new(AuthProvider {
        prepared: Default::default(),
        succeeded: Default::default(),
        trust_checked: auth.trust_checked.clone(),
        password: "wrong".into(),
    });
    let mut events = client.subscribe().events;
    let id = client
        .start_terminal(StartTerminalRequest {
            config_id: Some(TerminalConfigId::new()),
            name: "bad auth".into(),
            target: TerminalStartTarget::Ssh(server.target(rejected.clone())),
            dependencies: vec![],
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(TerminalEvent::TerminalEnded { run }) = events.next().await.unwrap()
                && run.id == id
            {
                assert!(matches!(
                    run.state,
                    TerminalRunState::Ended {
                        result: TerminalRunResult::Failed(TerminalError {
                            kind: TerminalErrorKind::AuthenticationFailed,
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
    assert_eq!(rejected.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(rejected.succeeded.load(Ordering::SeqCst), 0);
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn private_key_authentication_uses_in_memory_material() {
    use std::sync::{Arc, atomic::AtomicUsize};
    #[derive(Clone)]
    struct KeyAuth {
        address: std::net::SocketAddr,
        pem: String,
        confirmed: Arc<AtomicUsize>,
    }
    impl SshConnectionProvider for KeyAuth {
        fn prepare(&self) -> BoxFuture<'_, Result<SshConnectOptions>> {
            Box::pin(async move {
                Ok(SshConnectOptions {
                    address: self.address,
                    username: "tester".into(),
                    auth_provider: Arc::new(self.clone()),
                })
            })
        }
    }
    impl SshAuthProvider for KeyAuth {
        fn prepare(
            &self,
            _: SshConnectionOwnerRunId,
            _: CancellationToken,
        ) -> BoxFuture<'_, Result<SshAuthMaterial>> {
            Box::pin(async move {
                Ok(SshAuthMaterial::PrivateKey(SecretBytes::new(
                    self.pem.as_bytes().to_vec(),
                )))
            })
        }
        fn authentication_succeeded(
            &self,
            _: SshConnectionOwnerRunId,
            _: CancellationToken,
        ) -> BoxFuture<'_, ()> {
            Box::pin(async move {
                self.confirmed.fetch_add(1, Ordering::SeqCst);
            })
        }
    }
    let server = Server::start(Settings::default()).await;
    let (mut service, client, auth) = service(&server, false);
    let mut target = server.target(auth);
    let key =
        russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
    let confirmed = Arc::new(AtomicUsize::new(0));
    target.summary.auth_kind = SshAuthKind::PrivateKey;
    target.provider = Arc::new(KeyAuth {
        address: server.address,
        pem: key
            .to_openssh(russh::keys::ssh_key::LineEnding::LF)
            .unwrap()
            .to_string(),
        confirmed: confirmed.clone(),
    });
    terminal(&client, target).await;
    assert_eq!(confirmed.load(Ordering::SeqCst), 1);
    assert_eq!(server.data.passwords.load(Ordering::SeqCst), 0);
    service.shutdown().await.unwrap();
}
