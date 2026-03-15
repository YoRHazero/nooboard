mod clipboard;
mod home;
mod network;
mod settings;
mod shared;
mod shell;
mod transfers;

use gpui::{AppContext as _, Context, Entity, IntoElement, Render, Window};

use crate::workspace::{
    LaunchHandle,
    controller::WorkspaceController,
    recent_activity::RecentActivityItem,
    route::WorkspaceRoute,
    shell_view_state::{WorkspaceShellViewState, build_workspace_shell_view_state},
    view_state::{WorkspaceViewState, build_workspace_view_state},
};

pub struct WorkspaceView {
    controller: Entity<WorkspaceController>,
    clipboard: clipboard::ClipboardPageState,
}

pub(super) struct WorkspaceRenderModel {
    route: WorkspaceRoute,
    shell: WorkspaceShellViewState,
    page: Option<WorkspaceViewState>,
}

impl WorkspaceView {
    pub fn new(launch: LaunchHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let controller = cx.new(|_| WorkspaceController::new(launch.clone()));
        let clipboard = clipboard::ClipboardPageState::new(window, cx);
        let edit_input = clipboard.edit_input();
        cx.observe(&edit_input, |_, _, cx| cx.notify()).detach();
        cx.observe(&controller, |_, _, cx| cx.notify()).detach();
        WorkspaceController::initialize(&controller, launch, cx);

        Self {
            controller,
            clipboard,
        }
    }

    pub(super) fn render_model(&self, cx: &Context<Self>) -> WorkspaceRenderModel {
        let controller = self.controller.read(cx);
        let recent_activity = controller
            .recent_activity()
            .iter()
            .cloned()
            .collect::<Vec<RecentActivityItem>>();
        let page = controller.snapshot().map(|snapshot| {
            build_workspace_view_state(snapshot, controller.latest_committed_record())
        });
        let shell = build_workspace_shell_view_state(
            controller.load_state(),
            page.as_ref(),
            &recent_activity,
            controller.bridge_state(),
            controller.launch().config_path.display().to_string(),
        );

        WorkspaceRenderModel {
            route: controller.route(),
            shell,
            page,
        }
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.render_model(cx);
        self.sync_clipboard_from_workspace(
            model.page.as_ref().map(|page| &page.clipboard),
            window,
            cx,
        );
        if model.route == WorkspaceRoute::Clipboard {
            self.bootstrap_clipboard_history_if_needed(cx);
        }
        self.render_root(model, cx)
    }
}
