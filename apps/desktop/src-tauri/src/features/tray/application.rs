use super::{
    domain::{Presentation, State, Summary},
    ports::TrayPort,
};
use crate::localization;
use std::sync::{Arc, Mutex};
struct Displayed {
    summary: Summary,
    language: String,
}
pub struct TrayService {
    port: Arc<dyn TrayPort>,
    displayed: Mutex<Displayed>,
}
impl TrayService {
    pub fn new(port: Arc<dyn TrayPort>, language: String) -> Self {
        Self {
            port,
            displayed: Mutex::new(Displayed {
                summary: Summary::Starting,
                language,
            }),
        }
    }
    pub fn snapshot(&self) -> State {
        self.port.state()
    }
    pub fn update(&self, summary: Summary) {
        let mut displayed = self.displayed.lock().unwrap();
        if displayed.summary != summary {
            displayed.summary = summary;
            self.port
                .render(presentation(&displayed.summary, &displayed.language));
        }
    }
    pub fn refresh(&self, language: &str) {
        let mut displayed = self.displayed.lock().unwrap();
        if displayed.language != language {
            displayed.language = language.into();
            self.port
                .render(presentation(&displayed.summary, &displayed.language));
        }
    }
}
pub fn presentation(summary: &Summary, language: &str) -> Presentation {
    let text = |key| localization::text(language, key);
    let status = match summary {
        Summary::Starting => text("starting"),
        Summary::Failed => text("startFailed"),
        Summary::Running { paused: true, .. } => text("trayPaused"),
        Summary::Running {
            connected,
            transfers,
            ..
        } => text("trayStatus")
            .replace("{{connected}}", &connected.to_string())
            .replace("{{transfers}}", &transfers.to_string()),
    };
    Presentation {
        status,
        open: text("trayOpen"),
        transfers: text("transfers"),
        settings: text("settings"),
        quit: text("menuQuit"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Tray(Mutex<Vec<String>>);
    impl TrayPort for Tray {
        fn state(&self) -> State {
            State {
                supported: true,
                available: true,
            }
        }
        fn render(&self, view: Presentation) {
            self.0.lock().unwrap().push(view.status);
        }
    }
    #[test]
    fn unchanged_backend_updates_do_not_redraw_or_revert_the_current_language() {
        let port = Arc::new(Tray::default());
        let service = TrayService::new(port.clone(), "en".into());
        service.update(Summary::Failed);
        service.update(Summary::Failed);
        service.refresh("zh-CN");
        service.update(Summary::Failed);
        let rendered = port.0.lock().unwrap();
        assert_eq!(rendered.len(), 2);
        assert_eq!(rendered[1], localization::text("zh-CN", "startFailed"));
    }
}
