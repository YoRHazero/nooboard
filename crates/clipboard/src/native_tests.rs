#![cfg(any(not(target_os = "macos"), feature = "diagnostics"))]
use crate::{Clipboard, Content, ImageData, ImageEncoding};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires native clipboard; private macOS pasteboard, isolated Windows/Linux desktop"]
async fn native_image_and_file_references_roundtrip() {
    #[cfg(target_os = "macos")]
    let open = || {
        Clipboard::open_diagnostic(
            format!("nooboard.diagnostic.content.{}", std::process::id()),
            Duration::from_millis(20),
            1024 * 1024,
        )
    };
    #[cfg(not(target_os = "macos"))]
    let open = || Clipboard::open(Duration::from_millis(20), 1024 * 1024);
    let a = open().unwrap();
    let b = open().unwrap();
    let image = ImageData::new(
        ImageEncoding::Png,
        include_bytes!("fixtures/alpha.png").to_vec(),
    )
    .unwrap();
    a.write_content(Content::Image(image.clone()))
        .await
        .unwrap();
    let Content::Image(read) = b.read().await.unwrap().content else {
        panic!("image expected")
    };
    assert_eq!(
        read.decode().unwrap().to_rgba8(),
        image.decode().unwrap().to_rgba8()
    );
    let paths = vec![
        std::env::temp_dir().join("picture # 中文.png"),
        std::env::temp_dir().join("second file.txt"),
    ];
    b.write_content(Content::Files(paths.clone()))
        .await
        .unwrap();
    assert_eq!(a.read().await.unwrap().content, Content::Files(paths));
    a.write_text("after files".into()).await.unwrap();
    assert_eq!(
        b.read().await.unwrap().content,
        Content::Text("after files".into())
    );
    b.shutdown().await.unwrap();
    a.shutdown().await.unwrap();
}
