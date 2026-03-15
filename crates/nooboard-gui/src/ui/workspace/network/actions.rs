use gpui::{Context, Window};
use nooboard_core::{ConnectDirectOutcome, UpsertDirectSeedInput};

use crate::{
    ui::workspace::WorkspaceView,
    workspace::{actions::network as network_actions, view_state::NetworkDirectSeedViewState},
};

use super::state::NetworkSearchResult;

impl WorkspaceView {
    pub(super) fn request_network_edit_seed(
        &mut self,
        seed: &NetworkDirectSeedViewState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.network.load_seed_into_draft(seed, window, cx);
        cx.notify();
    }

    pub(super) fn request_network_clear_seed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.network.clear_seed_draft(window, cx);
        self.network
            .set_feedback("Cleared the direct seed composer.");
        cx.notify();
    }

    pub(super) fn request_network_toggle_seed_enabled(&mut self, cx: &mut Context<Self>) {
        self.network.toggle_draft_enabled();
        cx.notify();
    }

    pub(super) fn request_network_search(&mut self, cx: &mut Context<Self>) {
        let query = self.network.search_query(cx).trim().to_string();
        if query.is_empty() {
            self.network
                .set_feedback("Enter a search term before querying direct seeds.");
            cx.notify();
            return;
        }

        let Some(task) = network_actions::search_direct_seeds_task(&self.controller, query, cx)
        else {
            self.network
                .fail_search("Network core bridge is not ready yet.".to_string());
            cx.notify();
            return;
        };

        self.network.begin_search();
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(results) => this.network.finish_search(results),
                    Err(error) => {
                        this.network
                            .fail_search(format!("Failed to search direct seeds: {error}"));
                    }
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_network_load_search_result(
        &mut self,
        seed: &NetworkSearchResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let projected = NetworkDirectSeedViewState {
            id: seed.id,
            label: seed.label.clone(),
            host: seed.host.clone(),
            port: seed.port,
            endpoint_label: seed.endpoint_label.clone(),
            enabled: seed.enabled,
            learned_device_id: seed.learned_device_id.clone(),
            last_connected_addr_label: None,
        };
        self.network.load_seed_into_draft(&projected, window, cx);
        cx.notify();
    }

    pub(super) fn request_network_save_seed(&mut self, cx: &mut Context<Self>) {
        let label = self.network.seed_label(cx).trim().to_string();
        let host = self.network.seed_host(cx).trim().to_string();
        let port_text = self.network.seed_port(cx).trim().to_string();

        if label.is_empty() || host.is_empty() || port_text.is_empty() {
            self.network
                .fail_save("Label, host, and port are required for a direct seed.".to_string());
            cx.notify();
            return;
        }

        let Ok(port) = port_text.parse::<u16>() else {
            self.network
                .fail_save("Direct seed port must be a valid u16 value.".to_string());
            cx.notify();
            return;
        };

        let Some(task) = network_actions::upsert_direct_seed_task(
            &self.controller,
            UpsertDirectSeedInput {
                id: self.network.editing_seed_id(),
                label,
                host,
                port,
                enabled: self.network.draft_enabled(),
            },
            cx,
        ) else {
            self.network
                .fail_save("Network core bridge is not ready yet.".to_string());
            cx.notify();
            return;
        };

        self.network.begin_save();
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(saved_id) => this.network.finish_save(saved_id),
                    Err(error) => {
                        this.network
                            .fail_save(format!("Failed to save direct seed: {error}"));
                    }
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_network_remove_seed(
        &mut self,
        seed: &NetworkDirectSeedViewState,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = network_actions::remove_direct_seed_task(&self.controller, seed.id, cx)
        else {
            self.network
                .set_feedback("Network core bridge is not ready yet.");
            cx.notify();
            return;
        };

        self.network
            .mark_seed_pending(seed.id, format!("Removing direct seed '{}'.", seed.label));
        cx.notify();

        let id = seed.id;
        let label = seed.label.clone();
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(()) => this
                        .network
                        .finish_seed_pending(id, format!("Removed direct seed '{}'.", label)),
                    Err(error) => this.network.finish_seed_pending(
                        id,
                        format!("Failed to remove direct seed '{label}': {error}"),
                    ),
                }
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_network_connect_seed(
        &mut self,
        seed: &NetworkDirectSeedViewState,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = network_actions::connect_direct_seed_task(&self.controller, seed.id, cx)
        else {
            self.network
                .set_feedback("Network core bridge is not ready yet.");
            cx.notify();
            return;
        };

        self.network.mark_seed_pending(
            seed.id,
            format!("Connecting to direct seed '{}'.", seed.label),
        );
        cx.notify();

        let id = seed.id;
        let label = seed.label.clone();
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                let message = match result {
                    Ok(ConnectDirectOutcome::Started) => {
                        format!("Started direct connection for '{label}'.")
                    }
                    Ok(ConnectDirectOutcome::AlreadyConnected(session_id)) => {
                        format!("'{label}' is already connected on session {session_id}.")
                    }
                    Err(error) => format!("Failed to connect direct seed '{label}': {error}"),
                };
                this.network.finish_seed_pending(id, message);
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_network_approve_request(
        &mut self,
        request: &crate::workspace::view_state::NetworkPendingRequestViewState,
        cx: &mut Context<Self>,
    ) {
        let Some(task) =
            network_actions::approve_direct_request_task(&self.controller, request.id, cx)
        else {
            self.network
                .set_feedback("Network core bridge is not ready yet.");
            cx.notify();
            return;
        };

        self.network.mark_request_pending(
            request.id,
            format!("Approving request from '{}'.", request.peer_device_id),
        );
        cx.notify();

        let id = request.id;
        let device_id = request.peer_device_id.clone();
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                let message = match result {
                    Ok(()) => format!("Approved request from '{device_id}'."),
                    Err(error) => format!("Failed to approve request from '{device_id}': {error}"),
                };
                this.network.finish_request_pending(id, message);
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_network_reject_request(
        &mut self,
        request: &crate::workspace::view_state::NetworkPendingRequestViewState,
        cx: &mut Context<Self>,
    ) {
        let Some(task) =
            network_actions::reject_direct_request_task(&self.controller, request.id, cx)
        else {
            self.network
                .set_feedback("Network core bridge is not ready yet.");
            cx.notify();
            return;
        };

        self.network.mark_request_pending(
            request.id,
            format!("Rejecting request from '{}'.", request.peer_device_id),
        );
        cx.notify();

        let id = request.id;
        let device_id = request.peer_device_id.clone();
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                let message = match result {
                    Ok(()) => format!("Rejected request from '{device_id}'."),
                    Err(error) => format!("Failed to reject request from '{device_id}': {error}"),
                };
                this.network.finish_request_pending(id, message);
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_network_disconnect_session(
        &mut self,
        session: &crate::workspace::view_state::NetworkSessionViewState,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = network_actions::disconnect_session_task(&self.controller, session.id, cx)
        else {
            self.network
                .set_feedback("Network core bridge is not ready yet.");
            cx.notify();
            return;
        };

        self.network.mark_session_pending(
            session.id,
            format!("Disconnecting session with '{}'.", session.peer_device_id),
        );
        cx.notify();

        let id = session.id;
        let device_id = session.peer_device_id.clone();
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                let message = match result {
                    Ok(()) => format!("Disconnected session with '{device_id}'."),
                    Err(error) => {
                        format!("Failed to disconnect session with '{device_id}': {error}")
                    }
                };
                this.network.finish_session_pending(id, message);
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }
}
