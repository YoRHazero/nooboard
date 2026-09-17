fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "desktop_discover",
            "desktop_begin_pairing",
            "desktop_accept_pairing",
            "desktop_pairing_code",
            "desktop_dismiss_pairing",
            "desktop_connect",
            "desktop_settings",
            "desktop_peer_settings",
            "desktop_select_targets",
            "desktop_send",
            "desktop_select_files",
            "desktop_receive_directory",
            "desktop_transfer_action",
            "desktop_unpair",
            "desktop_history",
            "desktop_history_action",
            "desktop_probe",
        ]),
    ))
    .expect("failed to build application permissions");
}
