//! Run one local command through the service. Ctrl+C shuts down its owned processes.
use nooboard_terminal::*;
use std::{io::Write, sync::Arc};

struct LocalOnly;
impl SshHostVerifier for LocalOnly {
    fn verify(
        &self,
        _: SshHostVerificationRequest,
        _: CancellationToken,
    ) -> BoxFuture<'_, Result<()>> {
        Box::pin(async {
            Err(TerminalError::new(
                TerminalErrorKind::HostKeyRejected,
                "this example only permits local commands",
            ))
        })
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let command = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "echo hello".into());
    let options = match std::env::var_os("NOOBOARD_TERMINAL_OPTIONS") {
        Some(path) => TerminalServiceOptions::from_json(&std::fs::read_to_string(path)?)?,
        None => TerminalServiceOptions::default(),
    };
    let mut service = TerminalService::start(
        options,
        TerminalServiceDependencies {
            host_verifier: Arc::new(LocalOnly),
        },
    )?;
    let terminal = service.handle();
    let result = async {
        // Subscribe before starting: ended records are not retained in snapshots.
        let mut events = terminal.subscribe().events;
        let (id, mut output) = terminal
            .start_command_with_output(
                StartCommandRequest {
                    command: CommandStartSpec {
                        auto_inputs: vec![],
                        config_id: CommandConfigId::new(),
                        config_name: "example command".into(),
                        target: CommandStartTarget::Local,
                        command,
                    },
                    reuse_policy: CommandReusePolicy::Shared,
                },
                CancellationToken::new(),
            )
            .await?;
        std::io::stdout().write_all(&output.initial_output.data)?;
        let observe = async {
            while let Some(chunk) = output
                .stream
                .next()
                .await
                .map_err(|e| std::io::Error::other(format!("output subscription: {e:?}")))?
            {
                std::io::stdout().write_all(&chunk.data)?;
                std::io::stdout().flush()?;
            }
            while let Some(event) = events
                .next()
                .await
                .map_err(|e| std::io::Error::other(format!("event subscription: {e:?}")))?
            {
                if let TerminalEvent::CommandEnded { run } = event
                    && run.id == id
                {
                    if let CommandRunState::Ended {
                        result: CommandRunResult::Failed(error),
                    } = run.state
                    {
                        return Err(error.into());
                    }
                    return Ok(());
                }
            }
            Err(std::io::Error::other("service closed without an end event").into())
        };
        tokio::select! {
            result = observe => result,
            signal = tokio::signal::ctrl_c() => {
                signal?;
                Ok::<(), Box<dyn std::error::Error>>(())
            }
        }
    }
    .await;
    service.shutdown().await?;
    result
}
