use super::domain::{Presentation, State};
pub trait TrayPort: Send + Sync {
    fn state(&self) -> State;
    fn render(&self, view: Presentation);
}
