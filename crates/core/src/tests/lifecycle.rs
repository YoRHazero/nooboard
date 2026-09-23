use super::*;
#[tokio::test]
async fn owner_shutdown_is_independent_of_live_handles_and_subscribers() {
    let clipboard = Clipboard::new();
    let (service, app) = start(&clipboard).await;
    let clone = app.clone();
    let status = app.subscribe();
    let address = app.status().listen_address;
    bounded(service.shutdown()).await.unwrap();
    assert_eq!(status.borrow().status.state, AppState::Stopped);
    assert!(matches!(
        clone.refresh_discovery().await,
        Err(Error::Stopped)
    ));
    assert!(matches!(
        app.set_settings(settings()).await,
        Err(Error::Stopped)
    ));
    let _listener = tokio::net::TcpListener::bind(address).await.unwrap();
}
#[tokio::test]
async fn dropping_handles_keeps_service_alive_but_dropping_owner_stops_it() {
    let clipboard = Clipboard::new();
    let (service, app) = start(&clipboard).await;
    let mut status = app.subscribe();
    drop(app);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(status.borrow().status.state, AppState::Running);
    drop(service);
    bounded(async {
        loop {
            if status.borrow_and_update().status.state == AppState::Stopped {
                break;
            }
            status.changed().await.unwrap();
        }
    })
    .await;
}
#[tokio::test]
async fn shutdown_waits_for_application_and_history_without_blocking_unrelated_queries() {
    let a = Clipboard::new();
    let b = Clipboard::new();
    let (sa, app_a) = start(&a).await;
    let (sb, app_b) = start(&b).await;
    pair(&app_a, &app_b).await;
    b.block.store(true, Ordering::SeqCst);
    a.copy(Payload::Text("complete during shutdown".into()));
    app_a.send_current().await.unwrap();
    bounded(b.entered.notified()).await;
    assert!(
        bounded(app_b.history(String::new(), 20, 0))
            .await
            .unwrap()
            .is_empty()
    );
    let close = tokio::spawn(sb.shutdown());
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!close.is_finished());
    b.release.notify_one();
    bounded(close).await.unwrap().unwrap();
    assert_eq!(b.text().as_deref(), Some("complete during shutdown"));
    assert_eq!(app_b.snapshot().history_revision, 1);
    assert_eq!(app_b.status().state, AppState::Stopped);
    bounded(sa.shutdown()).await.unwrap();
}
#[tokio::test]
async fn failing_start_releases_already_opened_services() {
    let clipboard = Clipboard::new();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let bad = Settings {
        pairing_listen_address: address.to_string(),
        ..settings()
    };
    assert!(
        start_with(
            &clipboard,
            BackendConfig::Sqlite(SqliteOptions::in_memory()),
            bad
        )
        .await
        .is_err()
    );
    drop(listener);
    let (s, _) = start(&clipboard).await;
    bounded(s.shutdown()).await.unwrap();
}
