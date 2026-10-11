use nooboard_ssh::*;

#[test]
fn paths_are_remote_data_and_deserialization_cannot_bypass_validation() {
    let root = RemotePath::new("/work/project".into()).unwrap();
    assert_eq!(
        root.resolve("../new folder/日本語").unwrap().as_str(),
        "/work/project/../new folder/日本語"
    );
    assert_eq!(
        root.resolve("C:\\Users\\tester").unwrap().as_str(),
        "C:/Users/tester"
    );
    assert_eq!(root.resolve("~").unwrap().as_str(), "/work/project/~");
    assert_eq!(
        root.resolve("$HOME").unwrap().as_str(),
        "/work/project/$HOME"
    );
    assert!(serde_json::from_str::<RemotePath>(r#""bad\u0000path""#).is_err());
    assert!(serde_json::from_str::<RemotePath>(r#""""#).is_err());
}

#[test]
fn password_debug_is_redacted() {
    let password = Password::new("never-print-me".into());
    assert_eq!(format!("{password:?}"), "Password([redacted])");
}
