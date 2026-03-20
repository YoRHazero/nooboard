use gpui::{Context, Window};
use nooboard_core::{ConnectDirectOutcome, NetworkStatus, UpsertDirectSeedInput};

use crate::{
    ui::workspace::WorkspaceView,
    workspace::{
        actions::{network as network_actions, settings as settings_actions},
        route::WorkspaceRoute,
        view_state::NetworkDirectSeedViewState,
    },
};

use super::state::{DirectPanelTab, SeedPanelMode};

impl WorkspaceView {
    pub(super) fn request_network_open_settings(&mut self, cx: &mut Context<Self>) {
        let _ = self.controller.update(cx, |controller, cx| {
            controller.set_route(WorkspaceRoute::Settings);
            cx.notify();
        });
    }

    pub(super) fn request_network_toggle_runtime(&mut self, cx: &mut Context<Self>) {
        let Some(status) = self
            .controller
            .read(cx)
            .snapshot()
            .map(|snapshot| snapshot.network.status.clone())
        else {
            self.network
                .set_feedback("Workspace snapshot is not ready yet.");
            cx.notify();
            return;
        };

        let (task, action_label) = match status {
            NetworkStatus::Stopped => (
                network_actions::start_network_task(&self.controller, cx),
                "start",
            ),
            NetworkStatus::Starting | NetworkStatus::Running | NetworkStatus::Error(_) => (
                network_actions::stop_network_task(&self.controller, cx),
                "stop",
            ),
        };

        let Some(task) = task else {
            self.network
                .set_feedback("Network core bridge is not ready yet.");
            cx.notify();
            return;
        };

        self.network
            .set_feedback(format!("Requesting runtime to {action_label} network."));
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                let message = match result {
                    Ok(()) => format!("Network {action_label} request applied."),
                    Err(error) => format!("Failed to {action_label} network: {error}"),
                };
                this.network.set_feedback(message);
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_network_toggle_lan_enabled(&mut self, cx: &mut Context<Self>) {
        let Some(current_enabled) = self
            .controller
            .read(cx)
            .snapshot()
            .map(|snapshot| snapshot.settings.network.lan_enabled)
        else {
            self.network
                .set_feedback("Workspace snapshot is not ready yet.");
            cx.notify();
            return;
        };

        let next_enabled = !current_enabled;
        let Some(task) = settings_actions::set_lan_enabled_task(&self.controller, next_enabled, cx)
        else {
            self.network
                .set_feedback("Network core bridge is not ready yet.");
            cx.notify();
            return;
        };

        self.network.set_feedback(if next_enabled {
            "Enabling LAN auto sync."
        } else {
            "Disabling LAN auto sync."
        });
        cx.notify();

        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                let message = match result {
                    Ok(()) if next_enabled => "LAN auto sync enabled.".to_string(),
                    Ok(()) => "LAN auto sync disabled.".to_string(),
                    Err(error) if next_enabled => {
                        format!("Failed to enable LAN auto sync: {error}")
                    }
                    Err(error) => format!("Failed to disable LAN auto sync: {error}"),
                };
                this.network.set_feedback(message);
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    pub(super) fn request_network_set_direct_tab(
        &mut self,
        tab: DirectPanelTab,
        cx: &mut Context<Self>,
    ) {
        self.network.set_direct_tab(tab);
        cx.notify();
    }

    pub(super) fn request_network_set_seed_panel_mode(
        &mut self,
        mode: SeedPanelMode,
        cx: &mut Context<Self>,
    ) {
        self.network.set_seed_panel_mode(mode);
        cx.notify();
    }

    pub(super) fn request_network_toggle_token_revealed(&mut self, cx: &mut Context<Self>) {
        self.network.toggle_token_revealed();
        cx.notify();
    }

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
            .set_feedback("Cleared the direct preset editor.");
        cx.notify();
    }

    pub(super) fn request_network_toggle_seed_enabled(&mut self, cx: &mut Context<Self>) {
        self.network.toggle_draft_enabled();
        cx.notify();
    }

    pub(super) fn request_network_save_seed(&mut self, cx: &mut Context<Self>) {
        let label = self.network.seed_label(cx).trim().to_string();
        let host = self.network.seed_host(cx).trim().to_string();
        let port_text = self.network.seed_port(cx).trim().to_string();

        if label.is_empty() || host.is_empty() || port_text.is_empty() {
            self.network
                .fail_save("Label, host, and port are required for a direct preset.".to_string());
            cx.notify();
            return;
        }

        let Ok(port) = port_text.parse::<u16>() else {
            self.network
                .fail_save("Direct preset port must be a valid u16 value.".to_string());
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
                            .fail_save(format!("Failed to save direct preset: {error}"));
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
            .mark_seed_pending(seed.id, format!("Removing direct preset '{}'.", seed.label));
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
                        .finish_seed_pending(id, format!("Removed direct preset '{}'.", label)),
                    Err(error) => this.network.finish_seed_pending(
                        id,
                        format!("Failed to remove direct preset '{label}': {error}"),
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
            format!("Connecting to direct preset '{}'.", seed.label),
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
                        format!("Started direct connection for '{}'.", label)
                    }
                    Ok(ConnectDirectOutcome::AlreadyConnected(session_id)) => {
                        format!("'{}' is already connected on session {}.", label, session_id)
                    }
                    Err(error) => format!("Failed to connect direct preset '{}': {error}", label),
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
                    Ok(()) => format!("Approved request from '{}'.", device_id),
                    Err(error) => format!("Failed to approve request from '{}': {error}", device_id),
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
                    Ok(()) => format!("Rejected request from '{}'.", device_id),
                    Err(error) => format!("Failed to reject request from '{}': {error}", device_id),
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
            format!("Disconnecting direct session with '{}'.", session.peer_device_id),
        );
        cx.notify();

        let id = session.id;
        let device_id = session.peer_device_id.clone();
        let view = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                let message = match result {
                    Ok(()) => format!("Disconnected direct session with '{}'.", device_id),
                    Err(error) => format!(
                        "Failed to disconnect direct session with '{}': {error}",
                        device_id
                    ),
                };
                this.network.finish_session_pending(id, message);
                cx.notify();
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }
}
