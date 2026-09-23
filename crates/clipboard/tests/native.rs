#![cfg(any(not(target_os = "macos"), feature = "diagnostics"))]
use nooboard_clipboard::{
    ClipboardService, ImageData, ImageEncoding, Options, Origin, Payload, ReadState, SkipReason,
};
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::{SystemTime, UNIX_EPOCH};

fn options() -> Options {
    let options = Options {
        poll_interval: Duration::from_millis(10),
        ..Options::default()
    };
    #[cfg(target_os = "macos")]
    let options = Options {
        diagnostic_name: Some(format!(
            "nooboard.diagnostic.native.{}.{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )),
        ..options
    };
    options
}
async fn observed(clipboard: &nooboard_clipboard::Clipboard, payload: Payload) {
    let mut snapshots = clipboard.subscribe();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if snapshots
                .borrow_and_update()
                .as_ref()
                .is_some_and(|s| s.content == ReadState::Ready(payload.clone()))
            {
                break;
            }
            snapshots.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires native clipboard; private macOS pasteboard, isolated Windows/Linux desktop"]
async fn native_text_events_ownership_and_large_transfer() {
    let options = options();
    let (a_service, a) = ClipboardService::start(options.clone()).await.unwrap();
    let (b_service, b) = ClipboardService::start(options.clone()).await.unwrap();
    b.read().await.unwrap();
    for text in [
        " 中文 🦀\r\nline\n ".to_owned(),
        "长文本🦀".repeat(60000),
        String::new(),
    ] {
        let payload = Payload::Text(text);
        let written = a.write(payload.clone()).await.unwrap();
        assert_eq!(written.origin, Origin::Application);
        observed(&b, payload.clone()).await;
        assert_eq!(b.read().await.unwrap().origin, Origin::External);
        assert_eq!(a.read().await.unwrap(), written);
        assert_eq!(
            b.read().await.unwrap().content,
            ReadState::Ready(payload.clone())
        );
        // Equal content from another owner must still be a new external copy.
        let rewritten = b.write(payload).await.unwrap();
        let external = a.read().await.unwrap();
        assert_eq!(external.origin, Origin::External);
        assert!(external.revision > written.revision);
        assert_eq!(external.content, rewritten.content);
        assert_eq!(b.read().await.unwrap(), rewritten);
    }
    a.write(Payload::Text("界".repeat(100000))).await.unwrap();
    let mut small_options = options;
    small_options.limits.text_bytes = 16;
    let (small_service, small) = ClipboardService::start(small_options).await.unwrap();
    assert_eq!(
        small.read().await.unwrap().content,
        ReadState::Skipped(SkipReason::TooLarge)
    );
    assert!(a.write(Payload::Text("a\0b".into())).await.is_err());
    small_service.shutdown().await.unwrap();
    b_service.shutdown().await.unwrap();
    a_service.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "requires native clipboard; private macOS pasteboard, isolated Windows/Linux desktop"]
async fn native_images_and_file_references_roundtrip() {
    let options = options();
    let (a_service, a) = ClipboardService::start(options.clone()).await.unwrap();
    let (b_service, b) = ClipboardService::start(options).await.unwrap();
    let image = ImageData::new(
        ImageEncoding::Png,
        include_bytes!("fixtures/alpha.png").to_vec(),
    )
    .unwrap();
    let written = a.write(Payload::Image(image.clone())).await.unwrap();
    let ReadState::Ready(Payload::Image(read)) = b.read().await.unwrap().content else {
        panic!("image expected");
    };
    assert_eq!(read.rgba().unwrap(), image.rgba().unwrap());
    assert_eq!(a.read().await.unwrap(), written);
    let paths = vec![
        std::env::temp_dir().join("picture # 中文.png"),
        std::env::temp_dir().join("second file.txt"),
    ];
    let written = b.write(Payload::Files(paths.clone())).await.unwrap();
    assert_eq!(
        a.read().await.unwrap().content,
        ReadState::Ready(Payload::Files(paths))
    );
    assert_eq!(b.read().await.unwrap(), written);
    a.write(Payload::Text("after files".into())).await.unwrap();
    assert_eq!(
        b.read().await.unwrap().content,
        ReadState::Ready(Payload::Text("after files".into()))
    );
    b_service.shutdown().await.unwrap();
    a_service.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "requires native clipboard; private macOS pasteboard, isolated Windows/Linux desktop"]
async fn native_concurrent_clients_and_explicit_shutdown() {
    let options = options();
    let (a_service, a) = ClipboardService::start(options.clone()).await.unwrap();
    let (b_service, b) = ClipboardService::start(options).await.unwrap();
    for index in 0..100 {
        let (written, read) =
            tokio::join!(a.write(Payload::Text(format!("text {index}"))), b.read());
        written.unwrap();
        read.unwrap();
        let (written, read) =
            tokio::join!(b.write(Payload::Text(format!("reply {index}"))), a.read());
        written.unwrap();
        read.unwrap();
    }
    b_service.shutdown().await.unwrap();
    a_service.shutdown().await.unwrap();
    assert!(a.read().await.is_err());
    assert!(b.write(Payload::Text("stopped".into())).await.is_err());
}
