use crate::ipc::{contracts::*, errors};

pub fn snapshot(value: nooboard_core::AppSnapshot) -> BackendSnapshot {
    let status = value.status;
    let local_device = super::devices::local_device(&status, value.local_network);
    let settings = status.settings;
    BackendSnapshot {
        terminal: super::terminal::workspace(value.terminal),
        local_device,
        session: value.session,
        revision: value.revision.to_string(),
        history_revision: value.history_revision.to_string(),
        state: status.state.into(),
        configuration: ConfigurationStatus {
            saved_revision: status.configuration_revision.to_string(),
            effective_revision: status.effective_revision.to_string(),
            restart_required: status.restart_required,
        },
        settings: super::settings::view(settings),
        current: super::clipboard::current(value.current),
        peers: super::devices::peers(status.peers),
        manual_targets: status.manual_targets,
        transfers: super::transfers::transfers(status.transfers),
        content_transfers: super::transfers::content_transfers(value.content_transfers),
        onboarding: super::devices::onboarding(value.onboarding),
        activities: super::clipboard::activities(value.activities),
        fault: value.fault.map(|f| Fault {
            sequence: f.sequence.to_string(),
            message: errors::ui("serviceFault"),
        }),
    }
}
