use russh::{Channel, ChannelMsg, client::Msg};

pub(crate) trait ReplySource {
    fn receive(&mut self) -> impl Future<Output = Option<ChannelMsg>> + Send;
}

impl ReplySource for Channel<Msg> {
    async fn receive(&mut self) -> Option<ChannelMsg> {
        self.wait().await
    }
}

/// Flow-control notifications can precede a channel request's reply (notably
/// with OpenSSH). russh has already applied them to the channel's send window.
/// Keep the caller's timeout around the whole wait, including these messages.
pub(crate) async fn request_reply(source: &mut impl ReplySource) -> Option<ChannelMsg> {
    loop {
        match source.receive().await {
            Some(ChannelMsg::WindowAdjusted { .. }) => {}
            message => return message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::{sync::mpsc, time::timeout};

    impl ReplySource for mpsc::Receiver<ChannelMsg> {
        async fn receive(&mut self) -> Option<ChannelMsg> {
            self.recv().await
        }
    }

    #[tokio::test]
    async fn window_updates_do_not_complete_the_request_or_consume_following_data() {
        let (sender, mut receiver) = mpsc::channel(8);
        sender
            .send(ChannelMsg::WindowAdjusted { new_size: 65536 })
            .await
            .unwrap();
        {
            let mut reply = std::pin::pin!(request_reply(&mut receiver));
            assert!(
                timeout(Duration::from_millis(20), &mut reply)
                    .await
                    .is_err()
            );
            sender
                .send(ChannelMsg::WindowAdjusted { new_size: 131072 })
                .await
                .unwrap();
            sender.send(ChannelMsg::Success).await.unwrap();
            sender
                .send(ChannelMsg::Data {
                    data: b"shell prompt".to_vec().into(),
                })
                .await
                .unwrap();
            assert!(matches!(reply.await, Some(ChannelMsg::Success)));
        }
        // The channel stream / terminal loop still owns all application bytes.
        assert!(
            matches!(receiver.recv().await, Some(ChannelMsg::Data { data }) if data == b"shell prompt"[..])
        );
    }

    #[tokio::test]
    async fn window_updates_never_hide_rejection_or_channel_closure() {
        for response in [ChannelMsg::Failure, ChannelMsg::Close, ChannelMsg::Eof] {
            let (sender, mut receiver) = mpsc::channel(2);
            sender
                .send(ChannelMsg::WindowAdjusted { new_size: 65536 })
                .await
                .unwrap();
            let expected = std::mem::discriminant(&response);
            sender.send(response).await.unwrap();
            let reply = request_reply(&mut receiver).await.unwrap();
            assert_eq!(std::mem::discriminant(&reply), expected);
        }
        let (sender, mut receiver) = mpsc::channel(1);
        sender
            .send(ChannelMsg::WindowAdjusted { new_size: 65536 })
            .await
            .unwrap();
        drop(sender);
        assert!(request_reply(&mut receiver).await.is_none());
    }
}
