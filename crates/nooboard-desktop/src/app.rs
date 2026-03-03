use std::{borrow::Cow, fs, path::PathBuf, sync::Arc};

use anyhow::Result;
use gpui::{
    App, AppContext, Application, AssetSource, Bounds, SharedString, WindowBounds, WindowOptions,
    px, size,
};
use gpui_component::{Root, TitleBar};

use crate::state::SharedState;
use crate::ui::{QuickPanelView, WorkspaceView};

struct DesktopAssets {
    base: PathBuf,
}

impl DesktopAssets {
    fn new() -> Self {
        Self {
            base: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        }
    }
}

impl AssetSource for DesktopAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        fs::read(self.base.join(path))
            .map(|data| Some(Cow::Owned(data)))
            .map_err(Into::into)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        fs::read_dir(self.base.join(path))
            .map(|entries| {
                entries
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .and_then(|entry| entry.file_name().into_string().ok())
                            .map(SharedString::from)
                    })
                    .collect()
            })
            .map_err(Into::into)
    }
}

pub fn run() {
    Application::new()
        .with_assets(DesktopAssets::new())
        .run(|cx| {
            gpui_component::init(cx);

            let shared = Arc::new(SharedState::demo());

            open_workspace_window(shared.clone(), cx).expect("workspace window must open");
            open_quick_panel_window(shared, cx).expect("quick panel window must open");
        });
}

fn open_workspace_window(shared: Arc<SharedState>, cx: &mut App) -> anyhow::Result<()> {
    let options = workspace_window_options(cx);
    cx.open_window(options, move |window, cx| {
        let state = shared.clone();
        let view = cx.new(|_| WorkspaceView::new(state));
        cx.new(|cx| Root::new(view, window, cx))
    })?;
    Ok(())
}

fn open_quick_panel_window(shared: Arc<SharedState>, cx: &mut App) -> anyhow::Result<()> {
    let options = quick_panel_window_options(cx);
    cx.open_window(options, move |window, cx| {
        let state = shared.clone();
        let view = cx.new(|_| QuickPanelView::new(state));
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

fn quick_panel_window_options(cx: &mut App) -> WindowOptions {
    WindowOptions {
        titlebar: Some(TitleBar::title_bar_options()),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(460.0), px(620.0)),
            cx,
        ))),
        window_min_size: Some(size(px(420.0), px(540.0))),
        ..Default::default()
    }
}
