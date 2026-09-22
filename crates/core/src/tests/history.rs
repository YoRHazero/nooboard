use super::*;
#[tokio::test]
async fn history_is_independent_of_sending_and_filters_nontext() {
    let clipboard = FakeClipboard::new();
    clipboard.text("old clipboard");
    let app = app(&clipboard).await;
    assert!(app.history("".into(), 100, 0).await.unwrap().is_empty());
    clipboard.text(" 中文 🦀\r\n ");
    settle().await;
    let rows = app.history("".into(), 100, 0).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text, " 中文 🦀\r\n ");
    for content in [
        ReadState::Skipped(SkipReason::Sensitive),
        ReadState::Skipped(SkipReason::Unsupported),
        ReadState::Skipped(SkipReason::TooLarge),
    ] {
        clipboard.copy(content, Origin::External);
        settle().await;
    }
    assert_eq!(app.history("".into(), 100, 0).await.unwrap().len(), 1);
    let mut settings = app.status().settings;
    settings.paused = true;
    app.set_settings(settings.clone()).await.unwrap();
    clipboard.text("paused history");
    settle().await;
    assert_eq!(app.history("".into(), 100, 0).await.unwrap().len(), 2);
    assert!(matches!(app.send_current().await, Err(Error::Paused)));
    settings.history = false;
    app.set_settings(settings).await.unwrap();
    clipboard.text("not recorded");
    settle().await;
    assert_eq!(app.history("".into(), 100, 0).await.unwrap().len(), 2);
    app.copy_history(rows[0].id).await.unwrap();
    assert_eq!(
        clipboard.current(),
        ReadState::Ready(Payload::Text(rows[0].text.clone()))
    );
    app.clear_history().await.unwrap();
    assert!(app.history("".into(), 100, 0).await.unwrap().is_empty());
    app.shutdown().await.unwrap();
}
