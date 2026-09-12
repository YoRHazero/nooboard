use crate::{Clipboard, Content, Origin};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires an isolated X11 or Wayland display"]
async fn native_linux_clipboard() {
    let a = Clipboard::open(Duration::from_millis(30), 1 << 20).unwrap();
    let b = Clipboard::open(Duration::from_millis(30), 1 << 20).unwrap();
    let mut events = b.subscribe();
    events.borrow_and_update();
    a.write_text("event-driven change".into()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            events.changed().await.unwrap();
            let snapshot = events.borrow_and_update().clone().unwrap();
            if snapshot.content == Content::Text("event-driven change".into()) {
                assert_eq!(snapshot.origin, Origin::External);
                break;
            }
        }
    })
    .await
    .unwrap();
    for text in [
        "你好 Ubuntu 🦀\n\r\n  ".to_owned(),
        "长文本🦀".repeat(60000),
        String::new(),
    ] {
        a.write_text(text.clone()).await.unwrap();
        let snapshot = b.read().await.unwrap();
        assert_eq!(snapshot.content, Content::Text(text.clone()));
        assert_eq!(snapshot.origin, Origin::External);
        assert_eq!(a.read().await.unwrap().origin, Origin::Application);
        b.write_text(format!("reply:{text}")).await.unwrap();
        assert_eq!(
            a.read().await.unwrap().content,
            Content::Text(format!("reply:{text}"))
        );
    }
    let small = Clipboard::open(Duration::from_millis(30), 16).unwrap();
    a.write_text("界".repeat(100000)).await.unwrap();
    assert_eq!(small.read().await.unwrap().content, Content::TooLarge);
    assert!(a.write_text("a\0b".into()).await.is_err());
    small.shutdown().await.unwrap();
    b.shutdown().await.unwrap();
    a.shutdown().await.unwrap();
}
