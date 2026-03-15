use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};
use gpui_component_assets::Assets as ComponentAssets;

pub struct GuiAssets {
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
    pub fn new() -> Self {
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

        let icon = std::str::from_utf8(icon.as_ref()).expect("component icon should be valid utf-8");
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
