pub trait WindowPort: Send + Sync {
    fn show(&self) -> bool;
    fn hide(&self) -> bool;
}
