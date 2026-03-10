use std::collections::BTreeSet;
use std::net::SocketAddr;

use nooboard_app::{
    ClipboardSettingsPatch, NetworkSettingsPatch, SettingsPatch, StorageSettingsPatch,
    TransferSettingsPatch,
};

use super::snapshot::{
    ClipboardSettingsValue, NetworkSettingsValue, StorageSettingsValue, TransferSettingsValue,
};

pub(super) fn network_patch_labels(
    current: &NetworkSettingsValue,
    draft: &NetworkSettingsValue,
) -> Vec<&'static str> {
    let mut labels = Vec::new();

    if current.network_enabled != draft.network_enabled {
        labels.push("Network service");
    }
    if current.mdns_enabled != draft.mdns_enabled {
        labels.push("Local discovery (mDNS)");
    }
    if current.manual_peers != draft.manual_peers {
        labels.push("Manual peers");
    }

    labels
}

pub(super) fn storage_patch_labels(
    current: &StorageSettingsValue,
    draft: &StorageSettingsValue,
) -> Vec<&'static str> {
    let mut labels = Vec::new();

    if current.db_root != draft.db_root {
        labels.push("Database root path");
    }
    if current.history_window_days != draft.history_window_days {
        labels.push("History retention window");
    }
    if current.dedup_window_days != draft.dedup_window_days {
        labels.push("Deduplication window");
    }
    if current.max_text_bytes != draft.max_text_bytes {
        labels.push("Maximum text bytes");
    }
    if current.gc_batch_size != draft.gc_batch_size {
        labels.push("Cleanup batch size");
    }

    labels
}

pub(super) fn clipboard_patch_labels(
    current: &ClipboardSettingsValue,
    draft: &ClipboardSettingsValue,
) -> Vec<&'static str> {
    if current.local_capture_enabled != draft.local_capture_enabled {
        vec!["Local clipboard capture"]
    } else {
        Vec::new()
    }
}

pub(super) fn transfer_patch_labels(
    current: &TransferSettingsValue,
    draft: &TransferSettingsValue,
) -> Vec<&'static str> {
    if current.download_dir != draft.download_dir {
        vec!["Download directory"]
    } else {
        Vec::new()
    }
}

pub(super) fn build_network_patches(
    current: &NetworkSettingsValue,
    draft: &NetworkSettingsValue,
) -> Vec<SettingsPatch> {
    let mut patches = Vec::new();

    if current.mdns_enabled != draft.mdns_enabled {
        patches.push(SettingsPatch::Network(
            NetworkSettingsPatch::SetMdnsEnabled(draft.mdns_enabled),
        ));
    }
    if current.manual_peers != draft.manual_peers {
        patches.push(SettingsPatch::Network(
            NetworkSettingsPatch::SetManualPeers(draft.manual_peers.clone()),
        ));
    }
    if current.network_enabled != draft.network_enabled {
        patches.push(SettingsPatch::Network(
            NetworkSettingsPatch::SetNetworkEnabled(draft.network_enabled),
        ));
    }

    patches
}

pub(super) fn build_storage_patch(
    current: &StorageSettingsValue,
    draft: &StorageSettingsValue,
) -> Option<SettingsPatch> {
    let mut patch = StorageSettingsPatch::default();
    let mut changed = false;

    if current.db_root != draft.db_root {
        patch.db_root = Some(draft.db_root.clone());
        changed = true;
    }
    if current.history_window_days != draft.history_window_days {
        patch.history_window_days = Some(draft.history_window_days);
        changed = true;
    }
    if current.dedup_window_days != draft.dedup_window_days {
        patch.dedup_window_days = Some(draft.dedup_window_days);
        changed = true;
    }
    if current.max_text_bytes != draft.max_text_bytes {
        patch.max_text_bytes = Some(draft.max_text_bytes);
        changed = true;
    }
    if current.gc_batch_size != draft.gc_batch_size {
        patch.gc_batch_size = Some(draft.gc_batch_size);
        changed = true;
    }

    changed.then_some(SettingsPatch::Storage(patch))
}

pub(super) fn build_clipboard_patch(
    current: &ClipboardSettingsValue,
    draft: &ClipboardSettingsValue,
) -> Option<SettingsPatch> {
    (current.local_capture_enabled != draft.local_capture_enabled).then_some(
        SettingsPatch::Clipboard(ClipboardSettingsPatch::SetLocalCaptureEnabled(
            draft.local_capture_enabled,
        )),
    )
}

pub(super) fn build_transfer_patch(
    current: &TransferSettingsValue,
    draft: &TransferSettingsValue,
) -> Option<SettingsPatch> {
    (current.download_dir != draft.download_dir).then_some(SettingsPatch::Transfers(
        TransferSettingsPatch::SetDownloadDir(draft.download_dir.clone()),
    ))
}

pub(super) fn network_validation_issues(value: &NetworkSettingsValue) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();

    for addr in &value.manual_peers {
        if !seen.insert(*addr) {
            duplicates.insert(*addr);
        }
    }

    duplicates
        .into_iter()
        .map(|addr| format!("Manual peer {addr} appears more than once"))
        .collect()
}

pub(super) fn storage_validation_issues(value: &StorageSettingsValue) -> Vec<String> {
    let mut issues = Vec::new();

    if value.db_root.as_os_str().is_empty() {
        issues.push("Database root path cannot be empty".to_string());
    }
    if value.history_window_days == 0 {
        issues.push("History retention window must be at least 1 day".to_string());
    }
    if value.dedup_window_days < value.history_window_days {
        issues.push(
            "Deduplication window must be greater than or equal to history retention".to_string(),
        );
    }
    if value.max_text_bytes == 0 {
        issues.push("Maximum text bytes must be at least 1".to_string());
    }
    if value.gc_batch_size == 0 {
        issues.push("Cleanup batch size must be at least 1".to_string());
    }

    issues
}

pub(super) fn transfer_validation_issues(value: &TransferSettingsValue) -> Vec<String> {
    if value.download_dir.as_os_str().is_empty() {
        vec!["Download directory cannot be empty".to_string()]
    } else {
        Vec::new()
    }
}

pub(super) fn parse_manual_peer_input(input: &str) -> Result<SocketAddr, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Enter an IP:port address first.".to_string());
    }

    trimmed
        .parse()
        .map_err(|_| "Manual peers must use the form IP:port.".to_string())
}

pub(super) fn add_manual_peer(
    manual_peers: &mut Vec<SocketAddr>,
    addr: SocketAddr,
) -> Result<(), String> {
    if manual_peers.contains(&addr) {
        return Err(format!("Manual peer {addr} is already in the draft."));
    }

    manual_peers.push(addr);
    manual_peers.sort_unstable();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::path::PathBuf;

    use nooboard_app::{ClipboardSettingsPatch, NetworkSettingsPatch, StorageSettingsPatch};

    use super::*;

    fn network_value() -> NetworkSettingsValue {
        NetworkSettingsValue {
            network_enabled: true,
            mdns_enabled: true,
            manual_peers: vec![],
        }
    }

    fn storage_value() -> StorageSettingsValue {
        StorageSettingsValue {
            db_root: PathBuf::from("/tmp/db"),
            history_window_days: 7,
            dedup_window_days: 14,
            max_text_bytes: 4096,
            gc_batch_size: 64,
        }
    }

    #[test]
    fn network_patches_follow_stable_apply_order() {
        let current = network_value();
        let draft = NetworkSettingsValue {
            network_enabled: false,
            mdns_enabled: false,
            manual_peers: vec!["127.0.0.1:24001".parse().unwrap()],
        };

        let patches = build_network_patches(&current, &draft);

        assert_eq!(
            patches,
            vec![
                SettingsPatch::Network(NetworkSettingsPatch::SetMdnsEnabled(false)),
                SettingsPatch::Network(NetworkSettingsPatch::SetManualPeers(vec![
                    "127.0.0.1:24001".parse().unwrap()
                ])),
                SettingsPatch::Network(NetworkSettingsPatch::SetNetworkEnabled(false)),
            ]
        );
    }

    #[test]
    fn storage_patch_only_includes_changed_fields() {
        let current = storage_value();
        let draft = StorageSettingsValue {
            db_root: PathBuf::from("/var/db"),
            history_window_days: 7,
            dedup_window_days: 21,
            max_text_bytes: 4096,
            gc_batch_size: 128,
        };

        let patch = build_storage_patch(&current, &draft);

        assert_eq!(
            patch,
            Some(SettingsPatch::Storage(StorageSettingsPatch {
                db_root: Some(PathBuf::from("/var/db")),
                history_window_days: None,
                dedup_window_days: Some(21),
                max_text_bytes: None,
                gc_batch_size: Some(128),
            }))
        );
    }

    #[test]
    fn clipboard_and_transfer_patches_emit_single_field_updates() {
        let clipboard = build_clipboard_patch(
            &ClipboardSettingsValue {
                local_capture_enabled: true,
            },
            &ClipboardSettingsValue {
                local_capture_enabled: false,
            },
        );
        let transfer = build_transfer_patch(
            &TransferSettingsValue {
                download_dir: PathBuf::from("/tmp/a"),
            },
            &TransferSettingsValue {
                download_dir: PathBuf::from("/tmp/b"),
            },
        );

        assert_eq!(
            clipboard,
            Some(SettingsPatch::Clipboard(
                ClipboardSettingsPatch::SetLocalCaptureEnabled(false)
            ))
        );
        assert_eq!(
            transfer,
            Some(SettingsPatch::Transfers(
                TransferSettingsPatch::SetDownloadDir(PathBuf::from("/tmp/b"))
            ))
        );
    }

    #[test]
    fn storage_validation_matches_app_constraints() {
        let issues = storage_validation_issues(&StorageSettingsValue {
            db_root: PathBuf::new(),
            history_window_days: 0,
            dedup_window_days: 0,
            max_text_bytes: 0,
            gc_batch_size: 0,
        });

        assert_eq!(issues.len(), 4);
        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("Database root path"))
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("History retention"))
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("Maximum text bytes"))
        );
    }

    #[test]
    fn parse_and_add_manual_peer_prevents_duplicates() {
        let addr = parse_manual_peer_input("127.0.0.1:24001").expect("socket addr should parse");
        let mut manual_peers = vec![addr];

        let duplicate = add_manual_peer(&mut manual_peers, addr).expect_err("duplicate rejected");
        assert!(duplicate.contains("already"));

        let invalid =
            parse_manual_peer_input("not-an-addr").expect_err("invalid manual peer rejected");
        assert!(invalid.contains("IP:port"));
    }

    #[test]
    fn network_validation_reports_duplicate_manual_peers() {
        let addr: SocketAddr = "127.0.0.1:24001".parse().unwrap();
        let issues = network_validation_issues(&NetworkSettingsValue {
            network_enabled: true,
            mdns_enabled: true,
            manual_peers: vec![addr, addr],
        });

        assert_eq!(
            issues,
            vec!["Manual peer 127.0.0.1:24001 appears more than once"]
        );
    }
}
