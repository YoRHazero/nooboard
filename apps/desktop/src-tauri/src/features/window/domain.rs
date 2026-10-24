use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Page {
    Home,
    Transfers,
    Settings,
}
#[derive(Clone, Serialize, schemars::JsonSchema)]
pub struct Navigation {
    pub id: u32,
    pub page: Page,
}
#[derive(Clone)]
pub struct State {
    pub visible: bool,
    pub navigation: Option<Navigation>,
    pub sequence: u32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            visible: true,
            navigation: None,
            sequence: 0,
        }
    }
}
impl State {
    pub fn navigate(&mut self, page: Page) {
        self.sequence = self.sequence.wrapping_add(1);
        self.navigation = Some(Navigation {
            id: self.sequence,
            page,
        });
    }
    pub fn acknowledge(&mut self, id: u32) {
        if self.navigation.as_ref().is_some_and(|n| n.id == id) {
            self.navigation = None;
        }
    }
}
