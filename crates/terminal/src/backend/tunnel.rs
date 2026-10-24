use super::{Context, DriverEvent, ssh};
use crate::*;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    task::JoinSet,
    time::timeout,
};

async fn socks_destination(stream: &mut TcpStream) -> Result<TunnelEndpoint> {
    if stream.read_u8().await? != 5 {
        return Err(TerminalError::invalid("only SOCKS5 is supported"));
    }
    let count = stream.read_u8().await? as usize;
    let mut methods = vec![0; count];
    stream.read_exact(&mut methods).await?;
    if !methods.contains(&0) {
        stream.write_all(&[5, 255]).await?;
        return Err(TerminalError::invalid(
            "SOCKS authentication method not supported",
        ));
    }
    stream.write_all(&[5, 0]).await?;
    let mut prefix = [0; 4];
    stream.read_exact(&mut prefix).await?;
    if prefix[0] != 5 || prefix[1] != 1 || prefix[2] != 0 {
        stream.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
        return Err(TerminalError::invalid("only SOCKS CONNECT is supported"));
    }
    let host = match prefix[3] {
        1 => {
            let mut bytes = [0; 4];
            stream.read_exact(&mut bytes).await?;
            IpAddr::V4(Ipv4Addr::from(bytes)).to_string()
        }
        4 => {
            let mut bytes = [0; 16];
            stream.read_exact(&mut bytes).await?;
            IpAddr::V6(Ipv6Addr::from(bytes)).to_string()
        }
        3 => {
            let len = stream.read_u8().await? as usize;
            let mut bytes = vec![0; len];
            stream.read_exact(&mut bytes).await?;
            String::from_utf8(bytes)
                .map_err(|_| TerminalError::invalid("invalid SOCKS hostname"))?
        }
        _ => {
            stream.write_all(&[5, 8, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            return Err(TerminalError::invalid("unsupported SOCKS address type"));
        }
    };
    let port = stream.read_u16().await?;
    if host.is_empty() || host.contains('\0') || port == 0 {
        return Err(TerminalError::invalid("invalid SOCKS destination"));
    }
    Ok(TunnelEndpoint { host, port })
}
async fn forward_local(
    mut stream: TcpStream,
    connection: Arc<ssh::Connection>,
    destination: Option<TunnelEndpoint>,
    options: TerminalServiceOptions,
    stop: CancellationToken,
) -> Result<()> {
    let dynamic = destination.is_none();
    let operation = async {
        let destination = match destination {
            Some(d) => d,
            None => timeout(options.io.request_timeout, socks_destination(&mut stream))
                .await
                .map_err(|_| TerminalError::invalid("SOCKS handshake timed out"))??,
        };
        let origin = stream.peer_addr()?;
        let channel = match ssh::request(
            options.io.request_timeout,
            connection.client.channel_open_direct_tcpip(
                destination.host,
                destination.port as u32,
                origin.ip().to_string(),
                origin.port() as u32,
            ),
        )
        .await
        {
            Ok(channel) => channel,
            Err(error) => {
                if dynamic {
                    let _ = stream.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).await;
                }
                return Err(error);
            }
        };
        if dynamic {
            stream.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
        }
        let mut channel = channel.into_stream();
        tokio::io::copy_bidirectional(&mut stream, &mut channel).await?;
        Ok(())
    };
    tokio::select! {biased;_=stop.cancelled()=>Ok(()),result=operation=>result}
}
async fn forward_remote(
    incoming: ssh::IncomingForward,
    destination: TunnelEndpoint,
    options: TerminalServiceOptions,
    stop: CancellationToken,
) -> Result<()> {
    let operation = async {
        let stream = timeout(
            options.io.connect_timeout,
            TcpStream::connect((destination.host.as_str(), destination.port)),
        )
        .await;
        let mut stream = match stream {
            Ok(Ok(stream)) => stream,
            _ => {
                incoming
                    .reply
                    .reject(russh::ChannelOpenFailure::ConnectFailed)
                    .await;
                return Ok(());
            }
        };
        incoming.reply.accept().await;
        let mut channel = incoming.channel.into_stream();
        tokio::io::copy_bidirectional(&mut stream, &mut channel).await?;
        Ok(())
    };
    tokio::select! {biased;_=stop.cancelled()=>Ok(()),result=operation=>result}
}
pub(super) async fn run(
    target: SshStartTarget,
    forward: TunnelForwardConfig,
    mut context: Context,
) -> Result<()> {
    context.input.close();
    let (incoming_tx, mut incoming_rx) = mpsc::channel(context.options.input_queue_capacity);
    let remote = matches!(forward, TunnelForwardConfig::Remote { .. });
    let connection = ssh::connect(&target, &context, remote.then_some(incoming_tx)).await?;
    let flow_stop = context.stop.child_token();
    let mut flows = JoinSet::new();
    let result = async {
        match forward {
            TunnelForwardConfig::Local { listen, destination } => {
                let listener = timeout(
                    context.options.io.connect_timeout,
                    TcpListener::bind((listen.host.as_str(), listen.port)),
                ).await.map_err(|_| ssh::error("forward listener timed out"))??;
                context.event(DriverEvent::Started { id: context.id, connection: Some(connection.clone()) });
                loop {
                    tokio::select! {
                        biased;
                        _ = context.stop.cancelled() => break,
                        _ = connection.ended.cancelled() => return Err(ssh::lost()),
                        accepted = listener.accept() => {
                            let (stream, _) = accepted?;
                            flows.spawn(forward_local(
                                stream, connection.clone(), Some(destination.clone()),
                                context.options.clone(), flow_stop.clone(),
                            ));
                        }
                        _ = flows.join_next(), if !flows.is_empty() => {},
                    }
                }
            }
            TunnelForwardConfig::Dynamic { listen } => {
                let listener = timeout(
                    context.options.io.connect_timeout,
                    TcpListener::bind((listen.host.as_str(), listen.port)),
                ).await.map_err(|_| ssh::error("dynamic listener timed out"))??;
                context.event(DriverEvent::Started { id: context.id, connection: Some(connection.clone()) });
                loop {
                    tokio::select! {
                        biased;
                        _ = context.stop.cancelled() => break,
                        _ = connection.ended.cancelled() => return Err(ssh::lost()),
                        accepted = listener.accept() => {
                            let (stream, _) = accepted?;
                            flows.spawn(forward_local(
                                stream, connection.clone(), None,
                                context.options.clone(), flow_stop.clone(),
                            ));
                        }
                        _ = flows.join_next(), if !flows.is_empty() => {},
                    }
                }
            }
            TunnelForwardConfig::Remote { listen, destination } => {
                ssh::request(
                    context.options.io.request_timeout,
                    connection.client.tcpip_forward(&listen.host, listen.port as u32),
                ).await?;
                context.event(DriverEvent::Started { id: context.id, connection: Some(connection.clone()) });
                loop {
                    tokio::select! {
                        biased;
                        _ = context.stop.cancelled() => break,
                        _ = connection.ended.cancelled() => return Err(ssh::lost()),
                        incoming = incoming_rx.recv() => {
                            let Some(incoming) = incoming else { return Err(ssh::lost()); };
                            if incoming.port != listen.port as u32 {
                                incoming.reply.reject(russh::ChannelOpenFailure::AdministrativelyProhibited).await;
                                continue;
                            }
                            flows.spawn(forward_remote(
                                incoming, destination.clone(), context.options.clone(), flow_stop.clone(),
                            ));
                        }
                        _ = flows.join_next(), if !flows.is_empty() => {},
                    }
                }
                tokio::select! {
                    _ = context.force.cancelled() => {},
                    _ = ssh::request(
                        context.options.io.request_timeout,
                        connection.client.cancel_tcpip_forward(&listen.host, listen.port as u32),
                    ) => {},
                }
            }
        }
        Ok(())
    }.await;
    context.event(DriverEvent::Cleaning(context.id));
    flow_stop.cancel();
    drop(incoming_rx);
    while flows.join_next().await.is_some() {}
    connection.close(&context).await;
    result
}
