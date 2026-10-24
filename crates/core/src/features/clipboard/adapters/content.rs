use crate::{Error, Result};
use nooboard_clipboard::{ImageData, ImageEncoding, Payload};
use nooboard_network::{OutgoingContent, ReceivedContent};
pub(crate) async fn outgoing(payload: Payload) -> Result<OutgoingContent> {
    Ok(match payload {
        Payload::Text(text) => OutgoingContent::Text(text),
        Payload::Files(paths) => OutgoingContent::Files(paths),
        Payload::Image(image) => OutgoingContent::Image(
            tokio::task::spawn_blocking(move || image.png().map(|image| image.bytes().to_vec()))
                .await
                .map_err(|_| Error::Internal)??,
        ),
    })
}
pub(crate) fn incoming(content: ReceivedContent) -> Result<Payload> {
    Ok(match content {
        ReceivedContent::Text(text) => Payload::Text(text),
        ReceivedContent::Files(paths) => Payload::Files(paths),
        ReceivedContent::Image(bytes) => Payload::Image(ImageData::new(ImageEncoding::Png, bytes)?),
    })
}

pub(crate) struct ContentAdapter;
impl crate::features::clipboard::ports::content::ContentPort for ContentAdapter {
    fn outgoing(&self, payload: Payload) -> crate::contracts::PortFuture<'_, OutgoingContent> {
        Box::pin(outgoing(payload))
    }
    fn incoming(&self, content: ReceivedContent) -> Result<Payload> {
        incoming(content)
    }
    fn thumbnail(&self, image: &nooboard_clipboard::ImageData) -> Result<(Vec<u8>, u32, u32)> {
        super::preview::thumbnail(image).map_err(Into::into)
    }
}
