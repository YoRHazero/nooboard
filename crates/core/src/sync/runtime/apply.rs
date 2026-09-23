use super::*;
use nooboard_clipboard::Payload;
impl Runtime {
    pub(super) async fn apply(
        &mut self,
        payload: Payload,
        source: Option<String>,
    ) -> Result<Snapshot> {
        // Observe an external copy before overwriting it; only observed snapshots are promised.
        self.observe(self.clipboard.read().await?, true).await?;
        let snapshot = self.clipboard.write_content(payload).await?;
        self.observed = self.observed.max(snapshot.revision);
        self.state.send_modify(|s| {
            s.current = crate::CurrentClipboard::from_native(snapshot.clone(), source)
        });
        if let nooboard_clipboard::ReadState::Ready(Payload::Image(image)) = &snapshot.content {
            self.pending_preview = Some((snapshot.revision, image.clone()));
            self.start_preview();
        }
        Ok(snapshot)
    }
}
