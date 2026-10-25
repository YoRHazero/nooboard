//! Shared native labels reuse the frontend's bilingual resources.
use std::sync::OnceLock;
pub fn text(language: &str, key: &str) -> String {
    static EN: OnceLock<serde_json::Value> = OnceLock::new();
    static ZH: OnceLock<serde_json::Value> = OnceLock::new();
    let values = if language == "zh-CN" {
        ZH.get_or_init(|| {
            serde_json::from_str(include_str!(
                "../../src/shared/i18n/locales/zh-CN/common.json"
            ))
            .unwrap()
        })
    } else {
        EN.get_or_init(|| {
            serde_json::from_str(include_str!("../../src/shared/i18n/locales/en/common.json"))
                .unwrap()
        })
    };
    values[key].as_str().unwrap_or(key).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_labels_share_complete_bilingual_frontend_resources() {
        for language in ["en", "zh-CN"] {
            for key in [
                "starting",
                "startFailed",
                "trayOpen",
                "trayStatus",
                "trayPaused",
                "transfers",
                "settings",
                "menuQuit",
                "trayQuitTitle",
                "trayQuitMessage",
                "trayKeepRunning",
                "quitFailedMessage",
            ] {
                assert_ne!(text(language, key), key);
            }
        }
    }
}
