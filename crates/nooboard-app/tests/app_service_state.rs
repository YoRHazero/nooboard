mod support;

use nooboard_app::{
    ClipboardRecordSource, DesktopAppService, NetworkSettingsPatch, SettingsPatch,
    SubmitTextRequest, SyncActualStatus, SyncDesiredState,
};
use tokio::time::{Duration, timeout};

use support::{
    TestError, connect_service_pair, new_service, new_service_pair, recv_clipboard_committed,
    wait_for_service_state, wait_for_state_update,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscriptions_are_app_lifetime_before_sync_starts() -> Result<(), TestError> {
    let env = new_service()?;
    let service = &env.service;

    let mut state_subscription = service.subscribe_state().await?;
    let mut event_subscription = service.subscribe_events().await?;
    assert_eq!(
        state_subscription.latest().sync.desired,
        SyncDesiredState::Stopped
    );
    assert_eq!(
        state_subscription
            .latest()
            .clipboard
            .latest_committed_event_id,
        None
    );

    service
        .patch_settings(SettingsPatch::Network(
            NetworkSettingsPatch::SetMdnsEnabled(false),
        ))
        .await?;

    let next_state = timeout(Duration::from_secs(2), state_subscription.recv()).await??;
    assert!(!next_state.settings.network.mdns_enabled);
    assert_eq!(next_state.sync.desired, SyncDesiredState::Stopped);

    let submitted_event_id = service
        .submit_text(SubmitTextRequest {
            content: "alpha".to_string(),
        })
        .await?;
    let (observed_event_id, source) = recv_clipboard_committed(&mut event_subscription).await?;
    assert_eq!(observed_event_id, submitted_event_id);
    assert_eq!(source, ClipboardRecordSource::UserSubmit);

    let committed_state = timeout(Duration::from_secs(2), state_subscription.recv()).await??;
    assert_eq!(
        committed_state.clipboard.latest_committed_event_id,
        Some(submitted_event_id)
    );

    service.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn state_subscription_survives_running_engine_restart_from_network_patch()
-> Result<(), TestError> {
    let (env_a, env_b) = new_service_pair()?;
    let service_a = &env_a.service;
    let service_b = &env_b.service;
    let mut state_subscription = service_a.subscribe_state().await?;
    let (_, noob_id_b) = connect_service_pair(service_a, service_b).await?;

    let running_state = wait_for_state_update(
        &mut state_subscription,
        Duration::from_secs(10),
        |state| {
            state.sync.desired == SyncDesiredState::Running
                && state.sync.actual == SyncActualStatus::Running
                && state
                    .peers
                    .connected
                    .iter()
                    .any(|peer| peer.noob_id == noob_id_b)
        },
    )
    .await?;
    let running_revision = running_state.revision;

    service_a
        .patch_settings(SettingsPatch::Network(
            NetworkSettingsPatch::SetNetworkEnabled(false),
        ))
        .await?;

    let disabled_state = wait_for_state_update(
        &mut state_subscription,
        Duration::from_secs(10),
        |state| {
            state.sync.desired == SyncDesiredState::Running
                && state.sync.actual == SyncActualStatus::Disabled
                && !state.settings.network.network_enabled
                && state.peers.connected.is_empty()
        },
    )
    .await?;
    assert!(disabled_state.revision > running_revision);

    wait_for_service_state(service_b, Duration::from_secs(10), |state| {
        state.sync.actual == SyncActualStatus::Running && state.peers.connected.is_empty()
    })
    .await?;

    service_a
        .patch_settings(SettingsPatch::Network(
            NetworkSettingsPatch::SetNetworkEnabled(true),
        ))
        .await?;

    let reenabled_state = wait_for_state_update(
        &mut state_subscription,
        Duration::from_secs(10),
        |state| {
            state.sync.desired == SyncDesiredState::Running
                && state.sync.actual == SyncActualStatus::Running
                && state.settings.network.network_enabled
                && state
                    .peers
                    .connected
                    .iter()
                    .any(|peer| peer.noob_id == noob_id_b)
        },
    )
    .await?;
    assert!(reenabled_state.revision > disabled_state.revision);

    wait_for_service_state(service_b, Duration::from_secs(10), |state| {
        state.sync.actual == SyncActualStatus::Running
            && state
                .peers
                .connected
                .iter()
                .any(|peer| peer.noob_id == reenabled_state.identity.noob_id)
    })
    .await?;

    service_a.shutdown().await?;
    service_b.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_publishes_final_state_to_existing_subscription() -> Result<(), TestError> {
    let env = new_service()?;
    let service = &env.service;

    service
        .set_sync_desired_state(SyncDesiredState::Running)
        .await?;
    wait_for_service_state(service, Duration::from_secs(10), |state| {
        state.sync.desired == SyncDesiredState::Running
            && state.sync.actual == SyncActualStatus::Running
    })
    .await?;

    let mut state_subscription = service.subscribe_state().await?;

    let (shutdown_result, final_state_result) = tokio::join!(
        service.shutdown(),
        wait_for_state_update(
            &mut state_subscription,
            Duration::from_secs(10),
            |state| {
                state.sync.desired == SyncDesiredState::Stopped
                    && state.sync.actual == SyncActualStatus::Stopped
                    && state.peers.connected.is_empty()
            }
        )
    );
    shutdown_result?;
    let _final_state = final_state_result?;

    Ok(())
}
