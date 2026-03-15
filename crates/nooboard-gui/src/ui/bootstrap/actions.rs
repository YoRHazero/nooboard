use gpui::{Context, PathPromptOptions, Window};

use crate::ui::BootstrapChooserView;

impl BootstrapChooserView {
    pub(super) fn browse_existing_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.controller.begin_action() {
            return;
        }
        cx.notify();

        let paths_receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose existing nooboard config".into()),
        });

        cx.spawn_in(window, async move |view, cx| {
            let path = match paths_receiver.await {
                Ok(Ok(Some(mut paths))) => paths.drain(..).next(),
                _ => None,
            };

            let _ = view.update(cx, |this, cx| {
                match path {
                    Some(path) => this.controller.apply_existing_config_selection(path),
                    None => this.controller.end_action(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn browse_custom_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.controller.begin_action() {
            return;
        }
        cx.notify();

        let paths_receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose directory for nooboard config".into()),
        });

        cx.spawn_in(window, async move |view, cx| {
            let directory = match paths_receiver.await {
                Ok(Ok(Some(mut paths))) => paths.drain(..).next(),
                _ => None,
            };

            let _ = view.update(cx, |this, cx| {
                match directory {
                    Some(directory) => this.controller.apply_custom_location_selection(directory),
                    None => this.controller.end_action(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn finish_launch(
        &mut self,
        launch: nooboard_core::BootstrapLaunch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.controller.send_launch(launch);
        cx.notify();
        window.remove_window();
    }

    pub(super) fn confirm_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(launch) = self.controller.confirm_selection() else {
            cx.notify();
            return;
        };

        self.finish_launch(launch, window, cx);
    }

    pub(super) fn quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.controller.quit();
        cx.notify();
        window.remove_window();
    }
}
