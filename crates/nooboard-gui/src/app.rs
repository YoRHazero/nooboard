use std::path::PathBuf;

use clap::Parser;
use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::{Root, Theme, ThemeMode, TitleBar};
use gpui_platform::application;
use nooboard_core::{
    BootstrapChooserContext, BootstrapDecision, BootstrapLaunch, BootstrapRequest,
    inspect_repo_development, resolve_bootstrap,
};
use tokio::sync::oneshot;

use crate::{
    assets::GuiAssets,
    ui::{BootstrapChooserView, WorkspaceView},
    workspace::LaunchHandle,
};

#[derive(Clone, Debug, Parser)]
#[command(name = "nooboard-gui")]
pub struct GuiCli {
    #[arg(long)]
    pub choose_config: bool,
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub dev: bool,
}

impl GuiCli {
    fn bootstrap_request(&self) -> BootstrapRequest {
        BootstrapRequest {
            cli_choose_config: self.choose_config,
            cli_config_path: self.config.clone(),
            cli_use_repo_dev: self.dev,
        }
    }
}

pub fn run(cli: GuiCli) {
    application().with_assets(GuiAssets::new()).run(move |cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        let decision =
            resolve_bootstrap(&cli.bootstrap_request()).expect("gui bootstrap must resolve");

        match decision {
            BootstrapDecision::Launch(launch) => {
                open_workspace_window(LaunchHandle::Ready(launch), cx)
                    .expect("gui workspace window must open");
            }
            BootstrapDecision::NeedsChooser(context) => {
                open_bootstrap_window(context, cx).expect("gui bootstrap chooser must open");
            }
        }
    });
}

fn open_bootstrap_window(context: BootstrapChooserContext, cx: &mut App) -> anyhow::Result<()> {
    let (launch_tx, launch_rx) = oneshot::channel::<BootstrapLaunch>();
    let can_use_repo_development = inspect_repo_development()
        .is_ok_and(|probe| matches!(probe, nooboard_core::RepoDevelopmentProbe::Available { .. }));
    let options = bootstrap_window_options(cx);
    let workspace_options = workspace_window_options(cx);

    cx.open_window(options, move |window, cx| {
        let view = cx.new(|_| {
            BootstrapChooserView::new(context.clone(), can_use_repo_development, launch_tx)
        });
        cx.new(|cx| Root::new(view, window, cx))
    })?;

    cx.spawn(async move |cx| {
        let Ok(launch) = launch_rx.await else {
            return Ok::<_, anyhow::Error>(());
        };

        cx.update(|cx| -> anyhow::Result<()> {
            open_workspace_window_with_options(LaunchHandle::Ready(launch), workspace_options, cx)
        })?;

        Ok::<_, anyhow::Error>(())
    })
    .detach();

    Ok(())
}

fn open_workspace_window(launch: LaunchHandle, cx: &mut App) -> anyhow::Result<()> {
    open_workspace_window_with_options(launch, workspace_window_options(cx), cx)
}

fn open_workspace_window_with_options(
    launch: LaunchHandle,
    options: WindowOptions,
    cx: &mut App,
) -> anyhow::Result<()> {
    cx.open_window(options, move |window, cx| {
        let view = cx.new(|cx| WorkspaceView::new(launch.clone(), window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    })?;
    Ok(())
}

fn bootstrap_window_options(cx: &mut App) -> WindowOptions {
    WindowOptions {
        titlebar: Some(TitleBar::title_bar_options()),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(620.0), px(520.0)),
            cx,
        ))),
        window_min_size: Some(size(px(620.0), px(520.0))),
        is_resizable: false,
        ..Default::default()
    }
}

fn workspace_window_options(cx: &mut App) -> WindowOptions {
    WindowOptions {
        titlebar: Some(TitleBar::title_bar_options()),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1200.0), px(820.0)),
            cx,
        ))),
        window_min_size: Some(size(px(1080.0), px(600.0))),
        ..Default::default()
    }
}
