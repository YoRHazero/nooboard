use std::{borrow::Cow, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use gpui::{
    App, AppContext, AssetSource, Bounds, SharedString, WindowBounds, WindowOptions, px, size,
};
use gpui_component::{Root, Theme, ThemeMode, TitleBar};
use gpui_component_assets::Assets as ComponentAssets;
use gpui_platform::application;
use nooboard_core::{
    BootstrapChooserContext, BootstrapDecision, BootstrapLaunch, BootstrapRequest,
    inspect_repo_development, resolve_bootstrap,
};
use tokio::sync::oneshot;

use crate::{
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

struct GuiAssets {
    component: ComponentAssets,
    local: LocalAssets,
}

struct LocalAssets;

struct LocalAsset {
    path: &'static str,
    bytes: &'static [u8],
}

const LOCAL_ASSETS: &[LocalAsset] = &[
    LocalAsset {
        path: "system_core/arc_port_signal.svg",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/system_core/arc_port_signal.svg"
        )),
    },
    LocalAsset {
        path: "system_core/arc_port_socket.svg",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/system_core/arc_port_socket.svg"
        )),
    },
    LocalAsset {
        path: "system_core/arc_port_track.svg",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/system_core/arc_port_track.svg"
        )),
    },
    LocalAsset {
        path: "system_core/power.svg",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/system_core/power.svg"
        )),
    },
    LocalAsset {
        path: "system_core/radar_scan_line.svg",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/system_core/radar_scan_line.svg"
        )),
    },
];

impl GuiAssets {
    fn new() -> Self {
        Self {
            component: ComponentAssets,
            local: LocalAssets,
        }
    }
}

impl AssetSource for LocalAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Ok(LOCAL_ASSETS
            .iter()
            .find(|asset| asset.path == path)
            .map(|asset| Cow::Borrowed(asset.bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(list_local_assets(path))
    }
}

impl AssetSource for GuiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match self.component.load(path) {
            Ok(Some(asset)) => Ok(Some(asset)),
            Ok(None) | Err(_) => self.local.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = self.component.list(path)?;
        assets.extend(self.local.list(path)?);
        assets.sort_by(|left, right| left.as_ref().cmp(right.as_ref()));
        assets.dedup_by(|left, right| left.as_ref() == right.as_ref());
        Ok(assets)
    }
}

pub fn run(cli: GuiCli) {
    application()
        .with_assets(GuiAssets::new())
        .run(move |cx| {
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
    let can_use_repo_development = inspect_repo_development().is_ok_and(|probe| {
        matches!(probe, nooboard_core::RepoDevelopmentProbe::Available { .. })
    });
    let options = bootstrap_window_options(cx);
    let workspace_options = workspace_window_options(cx);

    cx.open_window(options, move |window, cx| {
        let view =
            cx.new(|_| BootstrapChooserView::new(context.clone(), can_use_repo_development, launch_tx));
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

fn list_local_assets(path: &str) -> Vec<SharedString> {
    let mut assets = LOCAL_ASSETS
        .iter()
        .filter_map(|asset| {
            if path.is_empty() {
                asset
                    .path
                    .split_once('/')
                    .map(|(head, _)| SharedString::from(head))
            } else {
                let prefix = format!("{path}/");
                asset
                    .path
                    .strip_prefix(prefix.as_str())
                    .filter(|suffix| !suffix.contains('/'))
                    .map(|suffix| SharedString::from(format!("{path}/{suffix}")))
            }
        })
        .collect::<Vec<_>>();
    assets.sort_by(|left, right| left.as_ref().cmp(right.as_ref()));
    assets.dedup_by(|left, right| left.as_ref() == right.as_ref());
    assets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_assets_load_gpui_component_icons() {
        let assets = GuiAssets::new();
        let icon = assets
            .load("icons/copy.svg")
            .expect("component icon should load")
            .expect("component icon should exist");

        let icon =
            std::str::from_utf8(icon.as_ref()).expect("component icon should be valid utf-8");
        assert!(icon.contains("<svg"), "component icon should be svg data");
    }

    #[test]
    fn gui_assets_load_local_system_core_icons() {
        let assets = GuiAssets::new();
        let icon = assets
            .load("system_core/radar_scan_line.svg")
            .expect("local asset should load")
            .expect("local asset should exist");

        let icon = std::str::from_utf8(icon.as_ref()).expect("local asset should be valid utf-8");
        assert!(icon.contains("<svg"), "local asset should be svg data");
    }

    #[test]
    fn gui_assets_missing_asset_returns_none() {
        let assets = GuiAssets::new();

        let missing = assets
            .load("system_core/does-not-exist.svg")
            .expect("missing asset lookup should not error");

        assert!(missing.is_none(), "missing asset should return none");
    }
}
