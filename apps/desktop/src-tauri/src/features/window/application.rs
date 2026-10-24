use super::{
    domain::{Page, State},
    ports::WindowPort,
};
use std::sync::Arc;
use tokio::sync::watch;
pub struct WindowService {
    port: Arc<dyn WindowPort>,
    state: watch::Sender<State>,
}
impl WindowService {
    pub fn new(port: Arc<dyn WindowPort>) -> Self {
        Self {
            port,
            state: watch::channel(State::default()).0,
        }
    }
    pub fn snapshot(&self) -> State {
        self.state.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<State> {
        self.state.subscribe()
    }
    pub fn show(&self, page: Option<Page>) {
        if let Some(page) = page {
            self.state.send_modify(|s| s.navigate(page));
        }
        if self.port.show() {
            self.visible(true);
        }
    }
    pub fn hide(&self) -> bool {
        if self.port.hide() {
            self.visible(false);
            true
        } else {
            false
        }
    }
    pub fn visible(&self, visible: bool) {
        self.state.send_if_modified(|s| {
            if s.visible == visible {
                false
            } else {
                s.visible = visible;
                true
            }
        });
    }
    pub fn acknowledge(&self, id: u32) {
        self.state.send_if_modified(|s| {
            let old = s.navigation.as_ref().map(|n| n.id);
            s.acknowledge(id);
            old != s.navigation.as_ref().map(|n| n.id)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    struct Window(AtomicBool);
    impl WindowPort for Window {
        fn show(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
        fn hide(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }
    #[test]
    fn stale_navigation_ack_and_failed_window_operations_preserve_state() {
        let port = Arc::new(Window(AtomicBool::new(false)));
        let service = WindowService::new(port.clone());
        assert!(!service.hide());
        assert!(service.snapshot().visible);
        service.show(Some(Page::Transfers));
        let old = service.snapshot().navigation.unwrap().id;
        service.show(Some(Page::Settings));
        let current = service.snapshot().navigation.unwrap().id;
        service.acknowledge(old);
        assert_eq!(service.snapshot().navigation.unwrap().id, current);
        service.acknowledge(current);
        assert!(service.snapshot().navigation.is_none());
        port.0.store(true, Ordering::SeqCst);
        assert!(service.hide());
        assert!(!service.snapshot().visible);
        port.0.store(false, Ordering::SeqCst);
        service.show(None);
        assert!(!service.snapshot().visible);
    }
}
