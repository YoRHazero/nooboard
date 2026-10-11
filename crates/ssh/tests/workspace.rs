mod support;
use nooboard_ssh::*;
use std::{sync::atomic::Ordering, time::Duration};
use support::{Data, Server, wait};

#[tokio::test]
async fn wrong_password_is_an_authentication_failure_and_never_opens_a_second_transport() {
    let server = Server::start(Data::default()).await;
    let (service, ssh, _) = SshService::start(Options::default()).await.unwrap();
    let mut options = server.options();
    options.password = Password::new("incorrect-secret".into());
    ssh.connect(options).await.unwrap();
    wait(&ssh, |s| s.sessions[0].attempt_finished).await;
    assert_eq!(
        ssh.snapshot().sessions[0].terminal,
        ConnectionState::Failed(Error::Authentication)
    );
    assert_eq!(server.data.transports.load(Ordering::SeqCst), 1);
    assert!(
        !serde_json::to_string(&ssh.snapshot())
            .unwrap()
            .contains("incorrect-secret")
    );
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn unavailable_sftp_is_a_file_feature_error_and_preserves_the_terminal() {
    let server = Server::start(Data {
        deny_sftp: true,
        ..Default::default()
    })
    .await;
    let (service, ssh, _) = SshService::start(Options::default()).await.unwrap();
    ssh.connect(server.options()).await.unwrap();
    wait(&ssh, |s| {
        s.sessions[0].attempt_finished && s.sessions[0].terminal == ConnectionState::Ready
    })
    .await;
    assert_eq!(
        ssh.snapshot().sessions[0].files,
        ConnectionState::Failed(Error::SftpUnavailable)
    );
    assert_eq!(server.data.passwords.load(Ordering::SeqCst), 2);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn stalled_key_exchange_timeout_closes_the_underlying_socket() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let peer = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream.write_all(b"SSH-2.0-stalled-test\r\n").await.unwrap();
        let mut input = Vec::new();
        stream.read_to_end(&mut input).await.unwrap();
        assert!(input.starts_with(b"SSH-2.0-"));
    });
    let (service, ssh, _) = SshService::start(Options {
        connect_timeout: Duration::from_millis(100),
        ..Default::default()
    })
    .await
    .unwrap();
    ssh.connect(Connect {
        device_id: "stalled".into(),
        addresses: vec![address],
        username: "tester".into(),
        password: Password::new("unused".into()),
        trusted_key: Some(HostKey {
            algorithm: "ssh-ed25519".into(),
            fingerprint: "unused".into(),
            public_key: "unused".into(),
        }),
        size: PtySize::default(),
        scope: ConnectScope::Terminal,
    })
    .await
    .unwrap();
    wait(&ssh, |s| {
        s.sessions[0].terminal == ConnectionState::Failed(Error::Timeout)
    })
    .await;
    tokio::time::timeout(Duration::from_secs(3), peer)
        .await
        .unwrap()
        .unwrap();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn owner_drop_stops_live_handles_and_wakes_the_event_consumer() {
    let server = Server::start(Data::default()).await;
    let (service, ssh, mut events) = SshService::start(Options::default()).await.unwrap();
    server.connect(&ssh).await;
    drop(service);
    assert!(
        tokio::time::timeout(Duration::from_secs(3), events.next_event())
            .await
            .unwrap()
            .is_none()
    );
    wait(&ssh, |s| {
        s.sessions[0].terminal == ConnectionState::Closed
            && s.sessions[0].files == ConnectionState::Closed
    })
    .await;
    assert!(!ssh.is_running());
    assert_eq!(
        ssh.connect(server.options()).await.unwrap_err(),
        Error::Stopped
    );
}

#[tokio::test]
async fn reject_host_key_and_shutdown_pending_approval_never_send_password() {
    let server = Server::start(Data::default()).await;
    let (service, ssh, mut events) = SshService::start(Options::default()).await.unwrap();
    let mut options = server.options();
    options.trusted_key = None;
    let id = ssh.connect(options).await.unwrap();
    let Event::HostKeyRequired { request_id, .. } = events.next_event().await.unwrap();
    ssh.decide_host_key(request_id, false).await.unwrap();
    wait(&ssh, |s| {
        s.sessions
            .iter()
            .any(|s| s.id == id && s.terminal == ConnectionState::Failed(Error::HostKeyRejected))
    })
    .await;
    let mut options = server.options();
    options.trusted_key = None;
    ssh.connect(options).await.unwrap();
    let Event::HostKeyRequired { request_id, .. } = events.next_event().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), service.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert!(ssh.decide_host_key(request_id, true).await.is_err());
    assert_eq!(server.data.passwords.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn file_transport_failure_keeps_terminal_and_replacement_is_independent() {
    let server = Server::start(Data::default()).await;
    let (service, ssh, _) = SshService::start(Options::default()).await.unwrap();
    let id = server.connect(&ssh).await;
    let mut output = ssh.subscribe_terminal();
    let files = server.data.file_connections.lock().unwrap()[0].clone();
    files
        .disconnect(
            russh::Disconnect::ByApplication,
            "files only".into(),
            "en".into(),
        )
        .await
        .unwrap();
    wait(&ssh, |s| s.sessions[0].files != ConnectionState::Ready).await;
    ssh.write_terminal(id.clone(), b"still alive".to_vec())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let frame = output.recv().await.unwrap();
            if frame.bytes == b"still alive" {
                break;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(ssh.snapshot().sessions[0].terminal, ConnectionState::Ready);
    let mut options = server.options();
    options.scope = ConnectScope::Files;
    let second = ssh.connect(options).await.unwrap();
    wait(&ssh, |s| {
        s.sessions
            .iter()
            .any(|s| s.id == second && s.files == ConnectionState::Ready)
    })
    .await;
    assert_eq!(server.data.transports.load(Ordering::SeqCst), 3);
    assert_eq!(ssh.snapshot().sessions[0].terminal, ConnectionState::Ready);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn concurrent_downloads_publish_distinct_files_and_finished_sessions_release_capacity() {
    let server = Server::start(Data::default()).await;
    server
        .data
        .files
        .lock()
        .unwrap()
        .insert("/root/shared.txt".into(), b"content".to_vec());
    let (service, ssh, _) = SshService::start(Options {
        max_sessions: 1,
        ..Default::default()
    })
    .await
    .unwrap();
    let id = server.connect(&ssh).await;
    let directory = tempfile::tempdir().unwrap();
    let spec = TransferSpec::Download {
        remote: RemotePath::new("/root/shared.txt".into()).unwrap(),
        local_directory: directory.path().into(),
    };
    let (first, second) = tokio::join!(
        ssh.transfer(id.clone(), spec.clone()),
        ssh.transfer(id.clone(), spec)
    );
    let first = first.unwrap();
    let second = second.unwrap();
    wait(&ssh, |s| {
        s.transfers.len() == 2 && s.transfers.iter().all(|t| t.state.finished())
    })
    .await;
    assert!(
        ssh.snapshot()
            .transfers
            .iter()
            .all(|t| t.state == TransferState::Succeeded)
    );
    assert_eq!(
        tokio::fs::read(directory.path().join("shared.txt"))
            .await
            .unwrap(),
        b"content"
    );
    assert_eq!(
        tokio::fs::read(directory.path().join("shared (1).txt"))
            .await
            .unwrap(),
        b"content"
    );
    ssh.forget_transfer(first).await.unwrap();
    ssh.forget_transfer(second).await.unwrap();
    assert_eq!(
        ssh.forget_session(id.clone()).await.unwrap_err(),
        Error::Busy
    );
    ssh.close_terminal(id.clone()).await.unwrap();
    ssh.close_files(id.clone()).await.unwrap();
    wait(&ssh, |s| {
        s.sessions[0].terminal == ConnectionState::Closed && s.sessions[0].attempt_finished
    })
    .await;
    ssh.forget_session(id).await.unwrap();
    assert!(ssh.snapshot().sessions.is_empty());
    server.connect(&ssh).await;
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn idle_directory_closes_handle_and_stale_cursor_cannot_resume() {
    let server = Server::start(Data::default()).await;
    let (service, ssh, _) = SshService::start(Options {
        directory_idle_timeout: Duration::from_millis(40),
        ..Default::default()
    })
    .await
    .unwrap();
    let id = server.connect(&ssh).await;
    let batch = ssh
        .read_directory(id, RemotePath::new("/root".into()).unwrap())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        ssh.next_directory_batch(batch.cursor.unwrap())
            .await
            .unwrap_err(),
        Error::StaleCursor
    );
    assert_eq!(server.data.closes.load(Ordering::SeqCst), 1);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_during_upload_drains_cleanup_and_releases_connections() {
    let server = Server::start(Data {
        slow_writes: true,
        ..Default::default()
    })
    .await;
    let (service, ssh, _) = SshService::start(Options::default()).await.unwrap();
    let id = server.connect(&ssh).await;
    let directory = tempfile::tempdir().unwrap();
    let local = directory.path().join("shutdown.dat");
    tokio::fs::write(&local, vec![1; 512 * 1024]).await.unwrap();
    ssh.transfer(
        id,
        TransferSpec::Upload {
            local,
            remote_directory: RemotePath::new("/root".into()).unwrap(),
        },
    )
    .await
    .unwrap();
    wait(&ssh, |s| s.transfers[0].transferred > 0).await;
    tokio::time::timeout(Duration::from_secs(3), service.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ssh.snapshot().transfers[0].state, TransferState::Cancelled);
    assert!(server.data.files.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unknown_host_requires_approval_before_any_password_and_changed_key_stops() {
    let server = Server::start(Data::default()).await;
    let (service, ssh, mut events) = SshService::start(Options::default()).await.unwrap();
    let mut options = server.options();
    options.trusted_key = None;
    let id = ssh.connect(options).await.unwrap();
    let Event::HostKeyRequired {
        request_id, key, ..
    } = tokio::time::timeout(Duration::from_secs(5), events.next_event())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(key, server.key);
    assert_eq!(server.data.passwords.load(Ordering::SeqCst), 0);
    ssh.decide_host_key(request_id, true).await.unwrap();
    wait(&ssh, |s| {
        s.sessions
            .iter()
            .any(|s| s.id == id && s.files == ConnectionState::Ready)
    })
    .await;
    assert_eq!(server.data.passwords.load(Ordering::SeqCst), 2);
    let mut options = server.options();
    options.trusted_key.as_mut().unwrap().public_key = "different".into();
    let second = ssh.connect(options).await.unwrap();
    wait(&ssh, |s| {
        s.sessions.iter().any(|s| {
            s.id == second && matches!(s.terminal, ConnectionState::Failed(Error::HostKeyChanged))
        })
    })
    .await;
    assert_eq!(server.data.passwords.load(Ordering::SeqCst), 2);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn directory_validation_does_not_list_and_batches_are_demand_driven() {
    let server = Server::start(Data::default()).await;
    let (service, ssh, _) = SshService::start(Options::default()).await.unwrap();
    let id = server.connect(&ssh).await;
    let root = RemotePath::new("/root".into()).unwrap();
    assert_eq!(
        ssh.validate_directory(id.clone(), root.clone(), ".".into())
            .await
            .unwrap(),
        root
    );
    assert_eq!(server.data.reads.load(Ordering::SeqCst), 0);
    assert_eq!(
        ssh.validate_directory(id.clone(), root.clone(), "/missing".into())
            .await
            .unwrap_err(),
        Error::NotFound
    );
    assert_eq!(
        ssh.validate_directory(id.clone(), root.clone(), "/denied".into())
            .await
            .unwrap_err(),
        Error::PermissionDenied
    );
    let first = ssh.read_directory(id, root).await.unwrap();
    assert_eq!(first.entries.len(), 200);
    assert_eq!(server.data.reads.load(Ordering::SeqCst), 2);
    let cursor = first.cursor.unwrap();
    let second = ssh.next_directory_batch(cursor.clone()).await.unwrap();
    assert_eq!(second.entries.len(), 200);
    assert_eq!(server.data.reads.load(Ordering::SeqCst), 3);
    assert_eq!(
        ssh.next_directory_batch(cursor).await.unwrap_err(),
        Error::StaleCursor
    );
    let final_batch = ssh
        .next_directory_batch(second.cursor.unwrap())
        .await
        .unwrap();
    assert!(final_batch.complete);
    assert_eq!(final_batch.entries.len(), 50);
    assert!(final_batch.cursor.is_none());
    service.shutdown().await.unwrap();
    assert_eq!(server.data.closes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn terminal_transport_failure_does_not_cancel_upload_and_names_never_overwrite() {
    let server = Server::start(Data {
        slow_writes: true,
        ..Default::default()
    })
    .await;
    let (service, ssh, _) = SshService::start(Options::default()).await.unwrap();
    let id = server.connect(&ssh).await;
    assert_eq!(server.data.transports.load(Ordering::SeqCst), 2);
    let local = tempfile::tempdir().unwrap();
    let source = local.path().join("example.txt");
    let bytes = vec![42; 128 * 1024];
    tokio::fs::write(&source, &bytes).await.unwrap();
    server
        .data
        .files
        .lock()
        .unwrap()
        .insert("/root/example.txt".into(), b"original".to_vec());
    let task = ssh
        .transfer(
            id.clone(),
            TransferSpec::Upload {
                local: source,
                remote_directory: RemotePath::new("/root".into()).unwrap(),
            },
        )
        .await
        .unwrap();
    wait(&ssh, |s| {
        s.transfers
            .iter()
            .any(|t| t.id == task && t.transferred > 0)
    })
    .await;
    let terminal = server.data.terminals.lock().unwrap()[0].clone();
    terminal
        .disconnect(
            russh::Disconnect::ByApplication,
            "terminal only".into(),
            "en".into(),
        )
        .await
        .unwrap();
    wait(&ssh, |s| {
        s.transfers
            .iter()
            .any(|t| t.id == task && t.state.finished())
    })
    .await;
    assert_eq!(ssh.snapshot().transfers[0].state, TransferState::Succeeded);
    assert_eq!(
        server.data.files.lock().unwrap()["/root/example.txt"],
        b"original"
    );
    assert_eq!(
        server.data.files.lock().unwrap()["/root/example (1).txt"],
        bytes
    );
    let download_dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        download_dir.path().join("example (1).txt"),
        "local-original",
    )
    .await
    .unwrap();
    let task = ssh
        .transfer(
            id,
            TransferSpec::Download {
                remote: RemotePath::new("/root/example (1).txt".into()).unwrap(),
                local_directory: download_dir.path().into(),
            },
        )
        .await
        .unwrap();
    wait(&ssh, |s| {
        s.transfers
            .iter()
            .any(|t| t.id == task && t.state.finished())
    })
    .await;
    assert_eq!(ssh.snapshot().transfers[1].state, TransferState::Succeeded);
    assert_eq!(
        tokio::fs::read(download_dir.path().join("example (1) (1).txt"))
            .await
            .unwrap(),
        bytes
    );
    assert_eq!(
        tokio::fs::read(download_dir.path().join("example (1).txt"))
            .await
            .unwrap(),
        b"local-original"
    );
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancellation_cleans_only_its_temporary_file_and_keeps_service_alive() {
    let server = Server::start(Data {
        slow_writes: true,
        ..Default::default()
    })
    .await;
    let (service, ssh, _) = SshService::start(Options::default()).await.unwrap();
    let id = server.connect(&ssh).await;
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("cancel.dat");
    tokio::fs::write(&source, vec![8; 512 * 1024])
        .await
        .unwrap();
    let task = ssh
        .transfer(
            id.clone(),
            TransferSpec::Upload {
                local: source,
                remote_directory: RemotePath::new("/root".into()).unwrap(),
            },
        )
        .await
        .unwrap();
    wait(&ssh, |s| {
        s.transfers
            .iter()
            .any(|t| t.id == task && t.transferred > 0)
    })
    .await;
    ssh.cancel_transfer(task.clone()).await.unwrap();
    wait(&ssh, |s| {
        s.transfers
            .iter()
            .any(|t| t.id == task && t.state.finished())
    })
    .await;
    let task = &ssh.snapshot().transfers[0];
    assert_eq!(task.state, TransferState::Cancelled);
    assert!(task.temporary_path.is_none());
    assert!(server.data.files.lock().unwrap().is_empty());
    assert_eq!(ssh.snapshot().sessions[0].files, ConnectionState::Ready);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn lost_publication_reply_is_unconfirmed_and_cannot_be_blindly_retried() {
    let server = Server::start(Data {
        drop_rename_reply: true,
        ..Default::default()
    })
    .await;
    let (service, ssh, _) = SshService::start(Options {
        request_timeout: Duration::from_secs(1),
        ..Default::default()
    })
    .await
    .unwrap();
    let id = server.connect(&ssh).await;
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("result.txt");
    tokio::fs::write(&source, "finished").await.unwrap();
    let task = ssh
        .transfer(
            id,
            TransferSpec::Upload {
                local: source,
                remote_directory: RemotePath::new("/root".into()).unwrap(),
            },
        )
        .await
        .unwrap();
    wait(&ssh, |s| {
        s.transfers
            .iter()
            .any(|t| t.id == task && t.state.finished())
    })
    .await;
    assert_eq!(
        ssh.snapshot().transfers[0].state,
        TransferState::Unconfirmed
    );
    assert_eq!(
        server.data.files.lock().unwrap()["/root/result.txt"],
        b"finished"
    );
    assert!(ssh.retry_transfer(task).await.is_err());
    service.shutdown().await.unwrap();
}
