use crate::{
    Result,
    contracts::{
        PortFuture,
        clipboard::{ImageData, Payload},
        network::{OutgoingContent, ReceivedContent},
    },
};
pub(crate) trait ContentPort: Send + Sync {
    fn outgoing(&self, payload: Payload) -> PortFuture<'_, OutgoingContent>;
    fn incoming(&self, content: ReceivedContent) -> Result<Payload>;
    fn thumbnail(&self, image: &ImageData) -> Result<(Vec<u8>, u32, u32)>;
}
