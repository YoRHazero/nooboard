use super::*;
use crate::ContentStage;
use nooboard_network::{ContentKind, ContentResult, FileEntry, Manifest, TransferError};

async fn receive_in(app: &App, root: &std::path::Path) {
    app.set_settings(Settings {
        receive_directory: Some(root.into()),
        ..app.status().settings
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn files_fan_out_independently_save_bytes_and_only_publish_local_file_references() {
    let ca = FakeClipboard::new();
    let cb = FakeClipboard::new();
    let cc = FakeClipboard::new();
    let a = app(&ca).await;
    let destination = tempfile::tempdir().unwrap();
    let receive_directory = destination.path().join("Nooboard");
    let b = bootstrap::start_parts(
        test_database(),
        Identity::generate().unwrap(),
        Box::new(cb.clone()),
        Some(receive_directory.clone()),
    )
    .await
    .unwrap();
    let c = app(&cc).await;
    let source = tempfile::tempdir().unwrap();
    pair(&a, &b).await;
    pair(&a, &c).await;
    automatic(&a).await;
    automatic(&b).await;
    automatic(&c).await;
    let path = source.path().join("图片 # 1.png");
    let empty = source.path().join("empty");
    let bytes: Vec<_> = (0..900_000).map(|i| (i % 251) as u8).collect();
    std::fs::write(&path, &bytes).unwrap();
    std::fs::write(&empty, b"").unwrap();
    ca.copy(
        Content::Files(vec![path.clone(), empty.clone()]),
        Origin::External,
    );
    settle().await;
    assert!(a.history(String::new(), 20, 0).await.unwrap().is_empty());
    assert!(b.snapshot().content_transfers.is_empty());
    assert_eq!(
        b.status().settings.receive_directory,
        Some(receive_directory.clone())
    );
    assert!(!receive_directory.exists());
    a.select_targets(vec![b.status().noob_id, c.status().noob_id])
        .await
        .unwrap();
    a.send_current().await.unwrap();
    wait_for(|| {
        a.snapshot().content_transfers.len() == 2
            && a.snapshot()
                .content_transfers
                .iter()
                .all(|t| !t.stage.pending())
    })
    .await;
    let tasks = a.snapshot().content_transfers;
    assert_eq!(
        tasks
            .iter()
            .find(|t| t.peer == b.status().noob_id)
            .unwrap()
            .stage,
        ContentStage::Completed
    );
    assert_eq!(
        tasks
            .iter()
            .find(|t| t.peer == c.status().noob_id)
            .unwrap()
            .error,
        Some(TransferError::Directory)
    );
    let Content::Files(paths) = cb.current() else {
        panic!("local file references expected")
    };
    assert_eq!(std::fs::read(&paths[0]).unwrap(), bytes);
    assert!(std::fs::read(&paths[1]).unwrap().is_empty());
    assert!(
        paths
            .iter()
            .all(|p| p.starts_with(receive_directory.canonicalize().unwrap()))
    );
    assert_eq!(paths[0].parent(), paths[1].parent());
    assert!(b.history(String::new(), 20, 0).await.unwrap().is_empty());
    assert!(b.status().transfers.is_empty());
    assert!(c.status().transfers.is_empty());
    assert_eq!(cc.current(), Content::Empty);
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
    c.shutdown().await.unwrap();
}

#[tokio::test]
async fn image_content_saves_png_applies_pixels_and_can_be_copied_again_without_history() {
    let ca = FakeClipboard::new();
    let cb = FakeClipboard::new();
    let a = app(&ca).await;
    let b = app(&cb).await;
    let destination = tempfile::tempdir().unwrap();
    receive_in(&b, destination.path()).await;
    pair(&a, &b).await;
    let image = nooboard_clipboard::ImageData::new(
        nooboard_clipboard::ImageEncoding::Png,
        include_bytes!("fixtures/alpha.png").to_vec(),
    )
    .unwrap();
    ca.copy(Content::Image(image.clone()), Origin::External);
    settle().await;
    a.send_current().await.unwrap();
    wait_for(|| {
        b.snapshot()
            .content_transfers
            .first()
            .is_some_and(|r| r.stage == ContentStage::Completed)
    })
    .await;
    let Content::Image(received) = cb.current() else {
        panic!("image content expected")
    };
    assert_eq!(
        received.decode().unwrap().to_rgba8(),
        image.decode().unwrap().to_rgba8()
    );
    let task = b.snapshot().content_transfers[0].clone();
    assert_eq!(
        std::fs::read(&task.saved_paths[0]).unwrap(),
        received.bytes.as_slice()
    );
    cb.text("new local text");
    settle().await;
    let revision = cb.revision();
    b.copy_received(task.key).await.unwrap();
    wait_for(|| cb.revision() > revision).await;
    assert!(matches!(cb.current(), Content::Image(_)));
    assert_eq!(b.history(String::new(), 20, 0).await.unwrap().len(), 1);
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}

#[tokio::test]
async fn receiving_cancel_cleans_staging_and_late_finish_cannot_apply() {
    let clipboard = FakeClipboard::new();
    let app = app(&clipboard).await;
    let root = tempfile::tempdir().unwrap();
    receive_in(&app, root.path()).await;
    let mut peer = RawPeer::connect(&app).await;
    let id = MessageId {
        session: "c".repeat(32),
        sequence: 9,
    };
    peer.connection
        .send(&Message::Offer {
            id: id.clone(),
            target_epoch: peer.epoch,
            manifest: Manifest {
                kind: ContentKind::Files,
                files: vec![FileEntry {
                    name: "a".into(),
                    bytes: 100,
                    sha256: [0; 32],
                }],
            },
        })
        .await
        .unwrap();
    assert_eq!(
        next_business(&mut peer.connection).await,
        Message::Accept { id: id.clone() }
    );
    let key = app.snapshot().content_transfers[0].key.clone();
    app.cancel_transfer(key).await.unwrap();
    assert!(matches!(
        next_business(&mut peer.connection).await,
        Message::Outcome {
            result: ContentResult::Cancelled,
            ..
        }
    ));
    peer.connection.send(&Message::Finish { id }).await.unwrap();
    settle().await;
    assert_eq!(clipboard.current(), Content::Empty);
    wait_for(|| std::fs::read_dir(root.path()).unwrap().count() == 0).await;
    assert_eq!(
        app.snapshot().content_transfers[0].stage,
        ContentStage::Cancelled
    );
    app.shutdown().await.unwrap();
}

#[tokio::test]
async fn late_success_after_cancel_request_reports_actual_commit_not_false_cancellation() {
    let clipboard = FakeClipboard::new();
    let app = app(&clipboard).await;
    let mut peer = RawPeer::connect(&app).await;
    app.select_targets(vec![peer.id()]).await.unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("empty.txt");
    std::fs::write(&path, b"").unwrap();
    app.send_files(vec![path]).await.unwrap();
    let Message::Offer { id, .. } = next_business(&mut peer.connection).await else {
        panic!("offer expected");
    };
    peer.connection
        .send(&Message::Accept { id: id.clone() })
        .await
        .unwrap();
    assert_eq!(
        next_business(&mut peer.connection).await,
        Message::Finish { id: id.clone() }
    );
    wait_for(|| app.snapshot().content_transfers[0].stage == ContentStage::Verifying).await;
    let key = app.snapshot().content_transfers[0].key.clone();
    app.cancel_transfer(key).await.unwrap();
    assert_eq!(
        app.snapshot().content_transfers[0].stage,
        ContentStage::Cancelling
    );
    assert!(
        !app.snapshot()
            .activities
            .iter()
            .any(|a| a.content_node.as_deref() == Some("finished"))
    );
    assert_eq!(
        next_business(&mut peer.connection).await,
        Message::Cancel { id: id.clone() }
    );
    peer.connection
        .send(&Message::Outcome {
            id,
            result: ContentResult::Applied,
            error: None,
        })
        .await
        .unwrap();
    wait_for(|| app.snapshot().content_transfers[0].stage == ContentStage::Completed).await;
    assert_eq!(
        app.snapshot()
            .activities
            .iter()
            .filter(|a| a.content_node.as_deref() == Some("finished"))
            .count(),
        1
    );
    app.shutdown().await.unwrap();
}

#[tokio::test]
async fn an_early_applied_receipt_cannot_complete_a_file_before_data_was_sent() {
    let clipboard = FakeClipboard::new();
    let app = app(&clipboard).await;
    let mut peer = RawPeer::connect(&app).await;
    app.select_targets(vec![peer.id()]).await.unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("data.txt");
    std::fs::write(&path, b"not sent yet").unwrap();
    app.send_files(vec![path]).await.unwrap();
    let Message::Offer { id, .. } = next_business(&mut peer.connection).await else {
        panic!("offer expected");
    };
    peer.connection
        .send(&Message::Outcome {
            id,
            result: ContentResult::Applied,
            error: None,
        })
        .await
        .unwrap();
    wait_for(|| app.snapshot().content_transfers[0].stage == ContentStage::Failed).await;
    let task = &app.snapshot().content_transfers[0];
    assert_eq!(task.error, Some(TransferError::Protocol));
    assert_eq!(task.completed_bytes, 0);
    app.shutdown().await.unwrap();
}

#[tokio::test]
async fn saved_file_survives_clipboard_failure_and_recopy_never_resends_or_adds_history() {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct BusyClipboard {
        inner: FakeClipboard,
        fail: Arc<AtomicBool>,
    }
    impl ClipboardPort for BusyClipboard {
        fn subscribe(&self) -> watch::Receiver<nooboard_clipboard::Result<Snapshot>> {
            self.inner.subscribe()
        }
        fn read(&self) -> ClipboardFuture<'_> {
            self.inner.read()
        }
        fn write(&self, text: String) -> ClipboardFuture<'_> {
            self.inner.write(text)
        }
        fn write_content(&self, content: Content) -> ClipboardFuture<'_> {
            if self.fail.load(Ordering::Acquire) {
                Box::pin(async { Err(nooboard_clipboard::Error::Unavailable) })
            } else {
                self.inner.write_content(content)
            }
        }
    }
    let ca = FakeClipboard::new();
    let cb = FakeClipboard::new();
    let fail = Arc::new(AtomicBool::new(true));
    let a = app(&ca).await;
    let b = bootstrap::start_parts(
        test_database(),
        Identity::generate().unwrap(),
        Box::new(BusyClipboard {
            inner: cb.clone(),
            fail: fail.clone(),
        }),
        None,
    )
    .await
    .unwrap();
    let source = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    receive_in(&b, root.path()).await;
    pair(&a, &b).await;
    automatic(&b).await;
    let path = source.path().join("image.png");
    std::fs::write(&path, b"file bytes").unwrap();
    a.send_files(vec![path]).await.unwrap();
    wait_for(|| {
        a.snapshot()
            .content_transfers
            .first()
            .is_some_and(|r| r.stage == ContentStage::Saved)
    })
    .await;
    let task = b.snapshot().content_transfers[0].clone();
    assert_eq!(task.error, Some(TransferError::Clipboard));
    assert_eq!(std::fs::read(&task.saved_paths[0]).unwrap(), b"file bytes");
    assert_eq!(cb.current(), Content::Empty);
    fail.store(false, Ordering::Release);
    b.copy_received(task.key).await.unwrap();
    wait_for(|| b.snapshot().content_transfers[0].stage == ContentStage::Completed).await;
    assert!(matches!(cb.current(), Content::Files(_)));
    assert!(b.history(String::new(), 20, 0).await.unwrap().is_empty());
    assert!(b.status().transfers.is_empty());
    assert!(b.snapshot().content_transfers.iter().all(|r| r.incoming));
    a.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "34-second progressing transfer over localhost TLS; not a throughput benchmark"]
async fn progressing_transfer_outlives_the_text_receipt_deadline() {
    let clipboard = FakeClipboard::new();
    let app = app(&clipboard).await;
    let root = tempfile::tempdir().unwrap();
    receive_in(&app, root.path()).await;
    let mut peer = RawPeer::connect(&app).await;
    let bytes = vec![27; 34 * 65536];
    let prepared = nooboard_storage::files::PreparedBatch::from_bytes("slow.bin", &bytes).unwrap();
    let id = MessageId {
        session: "d".repeat(32),
        sequence: 1,
    };
    peer.connection
        .send(&Message::Offer {
            id: id.clone(),
            target_epoch: peer.epoch,
            manifest: Manifest {
                kind: ContentKind::Files,
                files: vec![FileEntry {
                    name: "slow.bin".into(),
                    bytes: bytes.len() as u64,
                    sha256: prepared.files[0].sha256,
                }],
            },
        })
        .await
        .unwrap();
    assert_eq!(
        next_business(&mut peer.connection).await,
        Message::Accept { id: id.clone() }
    );
    let started = std::time::Instant::now();
    for (index, chunk) in bytes.chunks(65536).enumerate() {
        tokio::time::sleep(Duration::from_secs(1)).await;
        peer.connection
            .send(&Message::Chunk {
                id: id.clone(),
                file: 0,
                offset: (index * 65536) as u64,
                bytes: chunk.to_vec(),
            })
            .await
            .unwrap();
        assert_eq!(
            next_business(&mut peer.connection).await,
            Message::ChunkAck {
                id: id.clone(),
                file: 0,
                offset: ((index + 1) * 65536) as u64
            }
        );
        assert_eq!(clipboard.current(), Content::Empty);
    }
    assert!(started.elapsed() > Duration::from_secs(30));
    peer.connection
        .send(&Message::Finish { id: id.clone() })
        .await
        .unwrap();
    assert_eq!(
        next_business(&mut peer.connection).await,
        Message::Outcome {
            id,
            result: ContentResult::Applied,
            error: None
        }
    );
    let Content::Files(paths) = clipboard.current() else {
        panic!("files expected");
    };
    assert_eq!(std::fs::read(&paths[0]).unwrap(), bytes);
    app.shutdown().await.unwrap();
}
