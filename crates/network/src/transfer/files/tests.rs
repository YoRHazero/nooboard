use super::filesystem::{cleanup_staging, staging};
use super::*;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
#[test]
fn crash_cleanup_keeps_live_transactions_and_user_directories() {
    let root = tempfile::tempdir().unwrap();
    let (live, lease) = staging(root.path(), ".nooboard-receive-").unwrap();
    let (orphan, old_lease) = staging(root.path(), ".nooboard-receive-").unwrap();
    let orphan = orphan.keep();
    drop(old_lease);
    let user = root.path().join(".nooboard-receive-user");
    fs::create_dir(&user).unwrap();
    cleanup_staging(root.path(), ".nooboard-receive-");
    assert!(live.path().is_dir());
    assert!(user.is_dir());
    assert!(!orphan.exists());
    drop(lease);
}
fn spec(name: &str, bytes: &[u8]) -> FileSpec {
    FileSpec {
        name: name.into(),
        bytes: bytes.len() as u64,
        sha256: Sha256::digest(bytes).into(),
    }
}
#[test]
fn receive_directory_is_created_for_first_transfer_and_reused() {
    let downloads = tempfile::tempdir().unwrap();
    let root = downloads.path().join("Nooboard");
    assert!(!root.exists());
    for content in [b"first".as_slice(), b"second".as_slice()] {
        let mut batch = IncomingBatch::create(&root, vec![spec("file", content)]).unwrap();
        batch.write(0, 0, content).unwrap();
        let paths = batch.commit().unwrap();
        assert_eq!(
            paths[0].parent(),
            Some(root.canonicalize().unwrap().as_path())
        );
        assert_eq!(fs::read(&paths[0]).unwrap(), content);
    }
    assert_eq!(fs::read(root.join("file")).unwrap(), b"first");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
}
#[test]
fn unavailable_receive_directory_does_not_recreate_parents_or_replace_files() {
    let downloads = tempfile::tempdir().unwrap();
    let root = downloads.path().join("Nooboard");
    fs::write(&root, b"existing file").unwrap();
    assert!(IncomingBatch::create(&root, vec![spec("file", b"data")]).is_err());
    assert_eq!(fs::read(&root).unwrap(), b"existing file");
    let missing = downloads.path().join("unavailable mount");
    assert!(IncomingBatch::create(&missing.join("Nooboard"), vec![spec("file", b"data")]).is_err());
    assert!(!missing.exists());
}
#[test]
fn verified_batch_publishes_together_and_handles_case_collisions_and_empty_files() {
    let root = tempfile::tempdir().unwrap();
    let mut batch = IncomingBatch::create(
        root.path(),
        vec![
            spec("../../A.txt", b"abc"),
            spec("../../a.txt", b""),
            spec("CON", b"z"),
        ],
    )
    .unwrap();
    assert!(batch.complete().is_err());
    batch.write(0, 0, b"abc").unwrap();
    batch.write(2, 0, b"z").unwrap();
    let paths = batch.commit().unwrap();
    assert_eq!(fs::read(&paths[0]).unwrap(), b"abc");
    assert!(fs::read(&paths[1]).unwrap().is_empty());
    assert_ne!(
        paths[0].to_string_lossy().to_lowercase(),
        paths[1].to_string_lossy().to_lowercase()
    );
    assert!(
        paths
            .iter()
            .all(|p| p.starts_with(root.path().canonicalize().unwrap())
                && p.parent() == paths[0].parent())
    );
}
#[test]
fn checksum_failure_and_drop_leave_no_formal_files_and_existing_file_survives() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("a.txt"), b"original").unwrap();
    {
        let mut b = IncomingBatch::create(root.path(), vec![spec("a.txt", b"new")]).unwrap();
        assert!(b.write(0, 0, b"bad").is_err());
    }
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    let mut b = IncomingBatch::create(root.path(), vec![spec("a.txt", b"new")]).unwrap();
    b.write(0, 0, b"new").unwrap();
    let paths = b.commit().unwrap();
    assert_ne!(paths[0], root.path().join("a.txt"));
    assert_eq!(fs::read(root.path().join("a.txt")).unwrap(), b"original");
}
#[test]
fn invalid_offsets_and_preparation_cancellation_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let mut b = IncomingBatch::create(root.path(), vec![spec("a", b"abc")]).unwrap();
    assert!(b.write(0, 1, b"a").is_err());
    assert!(b.write(0, 0, b"abcd").is_err());
    let path = root.path().join("source");
    fs::write(&path, b"data").unwrap();
    assert!(matches!(
        PreparedBatch::from_paths(&[path], &AtomicBool::new(true), |_, _| {}),
        Err(Error::Cancelled)
    ));
}
#[test]
fn changing_source_during_preparation_fails_the_whole_batch() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("changing.bin");
    fs::write(&path, vec![7; CHUNK_BYTES * 2]).unwrap();
    let mut changed = false;
    let result = PreparedBatch::from_paths(
        std::slice::from_ref(&path),
        &AtomicBool::new(false),
        |done, _| {
            if done > 0 && !changed {
                OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_len(1)
                    .unwrap();
                changed = true;
            }
        },
    );
    assert!(matches!(result, Err(Error::SourceChanged)));
}
