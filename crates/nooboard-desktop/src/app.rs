use std::{borrow::Cow, fs, io::ErrorKind, path::PathBuf, sync::Arc};

use anyhow::Result;
use gpui::{
    App, AppContext, AssetSource, AsyncApp, Bounds, SharedString, WindowBounds, WindowOptions, px,
    size,
};
use gpui_component::{Root, TitleBar};
use gpui_component_assets::Assets as ComponentAssets;
use gpui_platform::application;

use crate::state::{SharedState, install_desktop_live_app};
use crate::ui::WorkspaceView;

struct DesktopAssets {
    component: ComponentAssets,
    local: LocalAssets,
}

struct LocalAssets {
    base: PathBuf,
}

impl DesktopAssets {
    fn new() -> Self {
        Self {
            component: ComponentAssets,
            local: LocalAssets::new(),
        }
    }
}

impl LocalAssets {
    fn new() -> Self {
        Self {
            base: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        }
    }
}

impl AssetSource for LocalAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        match fs::read(self.base.join(path)) {
            Ok(data) => Ok(Some(Cow::Owned(data))),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let base = self.base.join(path);
        match fs::read_dir(base) {
            Ok(entries) => Ok(entries
                .filter_map(|entry| {
                    entry.ok().and_then(|entry| {
                        entry.file_name().into_string().ok().map(|name| {
                            if path.is_empty() {
                                SharedString::from(name)
                            } else {
                                SharedString::from(format!("{path}/{name}"))
                            }
                        })
                    })
                })
                .collect()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(vec![]),
            Err(error) => Err(error.into()),
        }
    }
}

impl AssetSource for DesktopAssets {
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

pub fn run() {
    application().with_assets(DesktopAssets::new()).run(|cx| {
        gpui_component::init(cx);
        install_desktop_live_app(desktop_config_path(), cx)
            .expect("desktop live app bootstrap must succeed");

        let shared = Arc::new(SharedState::demo());
        let workspace_options = workspace_window_options(cx);

        cx.spawn(async move |cx| {
            open_workspace_window(shared, workspace_options, cx)?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}

fn desktop_config_path() -> PathBuf {
    std::env::var_os("NOOBOARD_DESKTOP_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("configs")
                .join("dev.toml")
        })
}

fn open_workspace_window(
    shared: Arc<SharedState>,
    options: WindowOptions,
    cx: &AsyncApp,
) -> anyhow::Result<()> {
    cx.open_window(options, move |window, cx| {
        let state = shared.clone();
        let view = cx.new(|cx| WorkspaceView::new(state, cx));
        cx.new(|cx| Root::new(view, window, cx))
    })?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_assets_load_gpui_component_icons() {
        let assets = DesktopAssets::new();
        let icon = assets
            .load("icons/copy.svg")
            .expect("component icon should load")
            .expect("component icon should exist");

        let icon =
            std::str::from_utf8(icon.as_ref()).expect("component icon should be valid utf-8");
        assert!(icon.contains("<svg"), "component icon should be svg data");
    }

    #[test]
    fn desktop_assets_load_local_system_core_icons() {
        let assets = DesktopAssets::new();
        let icon = assets
            .load("system_core/radar_scan_line.svg")
            .expect("local asset should load")
            .expect("local asset should exist");

        let icon = std::str::from_utf8(icon.as_ref()).expect("local asset should be valid utf-8");
        assert!(icon.contains("<svg"), "local asset should be svg data");
    }

    #[test]
    fn desktop_assets_missing_asset_returns_none() {
        let assets = DesktopAssets::new();

        let missing = assets
            .load("system_core/does-not-exist.svg")
            .expect("missing asset lookup should not error");

        assert!(missing.is_none(), "missing asset should return none");
    }

    #[test]
    fn desktop_assets_list_both_component_and_local_assets() {
        let assets = DesktopAssets::new();

        let component_assets = assets.list("icons").expect("component icons should list");
        assert!(
            component_assets
                .iter()
                .any(|path| path.as_ref() == "icons/copy.svg"),
            "component icon should be present in listing"
        );

        let local_assets = assets
            .list("system_core")
            .expect("local assets should list");
        assert!(
            local_assets
                .iter()
                .any(|path| path.as_ref() == "system_core/radar_scan_line.svg"),
            "local asset should be present in listing"
        );
    }
}
