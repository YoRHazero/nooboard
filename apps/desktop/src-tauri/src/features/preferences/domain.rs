use serde::{Deserialize, Serialize};
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Serialize, schemars::JsonSchema, PartialEq, Eq,
)]
pub enum Language {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    Chinese,
}
impl Language {
    pub fn resolved(self, locale: &str) -> &'static str {
        match self {
            Self::Chinese => "zh-CN",
            Self::English => "en",
            Self::System => {
                let locale = locale.to_lowercase();
                if locale == "zh" || locale.starts_with("zh-") || locale.starts_with("zh_") {
                    "zh-CN"
                } else {
                    "en"
                }
            }
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub close_to_tray: bool,
    // None means the pre-tray localStorage language has not been imported yet.
    pub language: Option<Language>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            language: None,
        }
    }
}
#[derive(Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    pub close_to_tray: Option<bool>,
    pub language: Option<Language>,
}

#[derive(Clone, Debug)]
pub struct State {
    pub close_to_tray: bool,
    pub language: Language,
    pub resolved_language: String,
    pub failed: bool,
}
