use super::runtime::Command;
use crate::{
    connection::{channel::request_reply, client::Connection},
    runtime::Context,
    *,
};
use russh::{Channel, ChannelMsg, client::Msg};
use tokio::{sync::mpsc, time::timeout};

async fn accepted(channel: &mut Channel<Msg>) -> Result<()> {
    match request_reply(channel).await {
        Some(ChannelMsg::Success) => Ok(()),
        _ => Err(Error::Connection(
            "PTY or interactive shell request was rejected".into(),
        )),
    }
}
pub(super) async fn run(
    id: &str,
    connection: Connection,
    size: PtySize,
    mut inbox: mpsc::Receiver<Command>,
    context: Context,
) {
    let mut stop = context.stop.clone();
    let operation = async {
        if context
            .state
            .borrow()
            .sessions
            .iter()
            .any(|s| s.id == id && s.terminal == ConnectionState::Closed)
        {
            return Ok(ConnectionState::Closed);
        }
        let mut channel = timeout(context.options.request_timeout, async {
            let mut channel = connection.channel_open_session().await?;
            channel
                .request_pty(true, "xterm-256color", size.columns, size.rows, 0, 0, &[])
                .await?;
            accepted(&mut channel).await?;
            channel.request_shell(true).await?;
            accepted(&mut channel).await?;
            Ok::<_, Error>(channel)
        })
        .await
        .map_err(|_| Error::Timeout)??;
        context.session(id, |s| s.terminal = ConnectionState::Ready);
        loop {
            tokio::select! {
                Some(command) = inbox.recv() => match command {
                    Command::Input(bytes) => timeout(context.options.request_timeout, channel.data(&bytes[..])).await.map_err(|_| Error::Timeout)??,
                    Command::Resize(size) => timeout(context.options.request_timeout, channel.window_change(size.columns, size.rows, 0, 0)).await.map_err(|_| Error::Timeout)??,
                    Command::Close => { let _ = channel.close().await; return Ok(ConnectionState::Closed); },
                },
                message = channel.wait() => match message {
                    Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => context.output.push(id, &data),
                    Some(ChannelMsg::Eof | ChannelMsg::Close) => return Ok(ConnectionState::Ended),
                    None if connection.is_closed() => return Err(Error::Connection("terminal transport disconnected".into())),
                    None => return Ok(ConnectionState::Ended),
                    _ => {},
                },
            }
        }
    };
    let result = tokio::select! { biased; _ = stop.changed() => Ok(ConnectionState::Closed), result = operation => result };
    context.session(id, |s| {
        s.terminal = result.unwrap_or_else(ConnectionState::Failed)
    });
    let _ = timeout(
        context.options.request_timeout,
        connection.disconnect(russh::Disconnect::ByApplication, "terminal closed", "en"),
    )
    .await;
    let _ = timeout(context.options.request_timeout, connection).await;
}
