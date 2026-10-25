use crate::{
    app::runtime::Runtime,
    ipc::{
        contracts::*,
        errors::{self, UiError},
        mapping,
    },
};
use nooboard_core::{Error, terminal as core};
use std::{str::FromStr, time::Duration};
use tauri::ipc::Channel;
fn invalid() -> Error {
    Error::Configuration
}
fn id<T: FromStr>(value: &str) -> Result<T, Error> {
    value.parse().map_err(|_| invalid())
}
fn dependencies(values: Vec<TerminalDependency>) -> Result<Vec<core::DependencyConfig>, Error> {
    values
        .into_iter()
        .map(|d| {
            Ok(core::DependencyConfig {
                command_config_id: id(&d.command_id)?,
                reuse_policy: reuse(d.reuse),
            })
        })
        .collect()
}
fn reuse(value: TerminalReuse) -> core::CommandReusePolicy {
    match value {
        TerminalReuse::Shared => core::CommandReusePolicy::Shared,
        TerminalReuse::AlwaysNew => core::CommandReusePolicy::AlwaysNew,
    }
}
fn conflict(value: TerminalConflict) -> core::FileConflictPolicy {
    match value {
        TerminalConflict::Fail => core::FileConflictPolicy::Fail,
        TerminalConflict::Skip => core::FileConflictPolicy::Skip,
        TerminalConflict::Overwrite => core::FileConflictPolicy::Overwrite,
    }
}
fn reference(value: TerminalTarget) -> Result<core::TerminalConfigReference, Error> {
    Ok(match value.kind {
        TerminalResourceKind::Terminal => core::TerminalConfigReference::Terminal(id(&value.id)?),
        TerminalResourceKind::Command => core::TerminalConfigReference::Command(id(&value.id)?),
        TerminalResourceKind::Tunnel => core::TerminalConfigReference::Tunnel(id(&value.id)?),
        TerminalResourceKind::Ssh => core::TerminalConfigReference::SshTarget(id(&value.id)?),
    })
}
fn io_target(value: TerminalTarget) -> Result<core::TerminalIoTarget, Error> {
    Ok(match value.kind {
        TerminalResourceKind::Terminal => core::TerminalIoTarget::Terminal(id(&value.id)?),
        TerminalResourceKind::Command => core::TerminalIoTarget::Command(id(&value.id)?),
        _ => return Err(invalid()),
    })
}
fn key(value: TerminalHostKey) -> core::SshPublicKey {
    core::SshPublicKey {
        algorithm: value.algorithm,
        bytes: value.bytes,
    }
}
async fn save(
    workspace: &core::TerminalWorkspace,
    config_id: Option<String>,
    config: TerminalConfigInput,
) -> Result<String, Error> {
    match config {
        TerminalConfigInput::Terminal(input) => {
            let input = core::TerminalInput {
                name: input.name,
                target: input
                    .ssh_target
                    .map(|s| id(&s).map(core::TerminalTarget::Ssh))
                    .transpose()?
                    .unwrap_or(core::TerminalTarget::Local),
                dependencies: dependencies(input.dependencies)?,
            };
            match config_id {
                Some(value) => {
                    workspace.update_terminal(id(&value)?, input).await?;
                    Ok(value)
                }
                None => Ok(workspace.create_terminal(input).await?.to_string()),
            }
        }
        TerminalConfigInput::Command(input) => {
            let input = core::CommandInput {
                name: input.name,
                target: input
                    .ssh_target
                    .map(|s| id(&s).map(core::CommandTarget::Ssh))
                    .transpose()?
                    .unwrap_or(core::CommandTarget::Local),
                command: input.command,
            };
            match config_id {
                Some(value) => {
                    workspace.update_command(id(&value)?, input).await?;
                    Ok(value)
                }
                None => Ok(workspace.create_command(input).await?.to_string()),
            }
        }
        TerminalConfigInput::Tunnel(input) => {
            let listen = core::TunnelEndpoint {
                host: input.forward.listen_host,
                port: input.forward.listen_port,
            };
            let forward = match input.forward.kind {
                TerminalForwardKind::Dynamic => core::TunnelForwardConfig::Dynamic { listen },
                kind => {
                    let destination = core::TunnelEndpoint {
                        host: input.forward.destination_host.ok_or_else(invalid)?,
                        port: input.forward.destination_port.ok_or_else(invalid)?,
                    };
                    match kind {
                        TerminalForwardKind::Local => core::TunnelForwardConfig::Local {
                            listen,
                            destination,
                        },
                        _ => core::TunnelForwardConfig::Remote {
                            listen,
                            destination,
                        },
                    }
                }
            };
            let input = core::TunnelInput {
                name: input.name,
                target: id(&input.ssh_target)?,
                dependencies: dependencies(input.dependencies)?,
                forward,
            };
            match config_id {
                Some(value) => {
                    workspace.update_tunnel(id(&value)?, input).await?;
                    Ok(value)
                }
                None => Ok(workspace.create_tunnel(input).await?.to_string()),
            }
        }
        TerminalConfigInput::Ssh(input) => {
            let input = core::SshTargetInput {
                name: input.name,
                address: match input.address.kind {
                    TerminalAddressKind::Host => core::SshAddressSource::Host(input.address.value),
                    TerminalAddressKind::Device => {
                        core::SshAddressSource::Device(core::DeviceId(input.address.value))
                    }
                },
                port: input.port,
                username: input.username,
                auth: match input.auth {
                    TerminalAuthEdit::Keep => core::SshAuthInput::Keep,
                    TerminalAuthEdit::Password => core::SshAuthInput::Password,
                    TerminalAuthEdit::PrivateKey => core::SshAuthInput::ImportPrivateKey {
                        private_key: core::SecretInput::text(
                            input.private_key.ok_or_else(invalid)?,
                        ),
                        passphrase: input.passphrase.map(core::SecretInput::text),
                    },
                },
            };
            match config_id {
                Some(value) => {
                    workspace.update_ssh_target(id(&value)?, input).await?;
                    Ok(value)
                }
                None => Ok(workspace.create_ssh_target(input).await?.to_string()),
            }
        }
    }
}
pub async fn run(
    runtime: &Runtime,
    owner: String,
    action: TerminalAction,
    output: Option<Channel<TerminalOutputFrame>>,
) -> Result<Reply, UiError> {
    let action = match action {
        TerminalAction::Detach { subscription } => {
            runtime.terminal_io.detach(&owner, &subscription);
            return Ok(Reply::Done);
        }
        TerminalAction::Ack {
            subscription,
            sequence,
        } => {
            runtime.terminal_io.acknowledge(
                &owner,
                &subscription,
                sequence.parse().map_err(|_| errors::ui("subscription"))?,
            );
            return Ok(Reply::Done);
        }
        other => other,
    };
    let workspace = runtime
        .backend
        .app()
        .await
        .map_err(errors::desktop)?
        .terminal();
    let operation = async {
        match action {
            TerminalAction::SaveConfig { id, config } => {
                return Ok(Reply::TerminalSaved {
                    id: save(&workspace, id, config).await?,
                });
            }
            TerminalAction::CopyConfig { target } => {
                let copied = workspace.copy_config(reference(target)?).await?;
                let value = match copied {
                    core::TerminalConfigReference::Terminal(id) => id.to_string(),
                    core::TerminalConfigReference::Command(id) => id.to_string(),
                    core::TerminalConfigReference::Tunnel(id) => id.to_string(),
                    core::TerminalConfigReference::SshTarget(id) => id.to_string(),
                };
                return Ok(Reply::TerminalSaved { id: value });
            }
            TerminalAction::DeleteConfig { target } => {
                workspace.delete_config(reference(target)?).await?
            }
            TerminalAction::Stop { target } => match target.kind {
                TerminalResourceKind::Terminal => workspace.stop_terminal(id(&target.id)?).await?,
                TerminalResourceKind::Command => workspace.stop_command(id(&target.id)?).await?,
                TerminalResourceKind::Tunnel => workspace.stop_tunnel(id(&target.id)?).await?,
                _ => return Err(invalid()),
            },
            TerminalAction::StartLocal { name, subscription } => {
                return start(
                    runtime,
                    &workspace,
                    owner,
                    StartSource::Local(name),
                    subscription,
                    output,
                )
                .await;
            }
            TerminalAction::Start {
                target,
                reuse: policy,
                subscription,
            } => {
                // Native transport registration is handled below so UI ownership
                // is checked before admitting a run or waiting for Shared reuse.
                return start(
                    runtime,
                    &workspace,
                    owner,
                    StartSource::Configuration(target, policy),
                    subscription,
                    output,
                )
                .await;
            }
            TerminalAction::Attach {
                target,
                subscription,
            } => {
                let channel = output.ok_or_else(invalid)?;
                let io = io_target(target.clone())?;
                let lease = runtime
                    .terminal_io
                    .begin(
                        &owner,
                        &subscription,
                        workspace.snapshot().options.control_queue_capacity,
                    )
                    .map_err(|_| Error::Stopped)?;
                let result = workspace.subscribe_output(io).await;
                match result {
                    Ok(stream) => {
                        runtime
                            .terminal_io
                            .pump(owner, subscription, lease, stream, channel)
                    }
                    Err(error) => {
                        runtime.terminal_io.release(&owner, &subscription, lease);
                        return Err(error);
                    }
                }
                return Ok(Reply::TerminalOpened { target });
            }
            TerminalAction::Input { target, bytes } => {
                workspace.write_input(io_target(target)?, bytes).await?
            }
            TerminalAction::Resize { target, rows, cols } => {
                workspace
                    .resize(io_target(target)?, core::TerminalSize { rows, cols })
                    .await?
            }
            TerminalAction::Continue { id: step } => {
                workspace.continue_dependency(id(&step)?).await?
            }
            TerminalAction::Lifetime { id: run, policy } => {
                workspace
                    .set_command_lifetime_policy(
                        id(&run)?,
                        match policy {
                            TerminalLifetime::Dependent => core::CommandLifetimePolicy::Dependent,
                            TerminalLifetime::Independent => {
                                core::CommandLifetimePolicy::Independent
                            }
                        },
                    )
                    .await?
            }
            TerminalAction::ConfirmHost { id: prompt, accept } => {
                workspace.confirm_host(id(&prompt)?, accept).await?
            }
            TerminalAction::Password {
                id: prompt,
                password,
                remember,
            } => {
                workspace
                    .answer_password(id(&prompt)?, core::SecretInput::text(password), remember)
                    .await?
            }
            TerminalAction::CancelPrompt { id: prompt } => {
                workspace.cancel_prompt(id(&prompt)?).await?
            }
            TerminalAction::ReplaceHost {
                target_id,
                previous,
                replacement,
            } => {
                workspace
                    .replace_host_key(id(&target_id)?, previous.map(key), key(replacement))
                    .await?
            }
            TerminalAction::Browse { id: run, path } => {
                return Ok(Reply::TerminalDirectory(mapping::terminal::directory(
                    workspace
                        .list_directory(
                            id(&run)?,
                            path.map(core::RemotePath::new).transpose()?,
                            core::CancellationToken::new(),
                        )
                        .await?,
                )));
            }
            TerminalAction::Mkdir { id: run, path } => {
                workspace
                    .create_directory(id(&run)?, core::RemotePath::new(path)?)
                    .await?
            }
            TerminalAction::Rename {
                id: run,
                source,
                destination,
            } => {
                workspace
                    .rename_path(
                        id(&run)?,
                        core::RemotePath::new(source)?,
                        core::RemotePath::new(destination)?,
                    )
                    .await?
            }
            TerminalAction::Upload {
                folder,
                id: run,
                directory,
                conflict: policy,
            } => runtime
                .terminal_upload(
                    workspace.clone(),
                    id(&run)?,
                    directory,
                    folder,
                    conflict(policy),
                )
                .await
                .map_err(|e| match e {
                    crate::error::Error::Core(error) => error,
                    _ => invalid(),
                })?,
            TerminalAction::Download {
                id: run,
                paths,
                conflict: policy,
            } => runtime
                .terminal_download(workspace.clone(), id(&run)?, paths, conflict(policy))
                .await
                .map_err(|e| match e {
                    crate::error::Error::Core(error) => error,
                    _ => invalid(),
                })?,
            TerminalAction::DeleteFiles { id: run, paths } => {
                workspace
                    .start_file_operation(core::StartFileOperationRequest {
                        terminal_run_id: id(&run)?,
                        operation: core::FileOperationSpec::Delete {
                            paths: paths
                                .into_iter()
                                .map(core::RemotePath::new)
                                .collect::<Result<_, _>>()?,
                        },
                    })
                    .await?;
            }
            TerminalAction::StopFiles { id: run } => {
                workspace.stop_file_operation(id(&run)?).await?
            }
            TerminalAction::Options { options } => {
                let mut current = workspace.snapshot().options;
                current.stop.stop_grace_timeout =
                    Duration::from_millis(id(&options.graceful_stop_ms)?);
                current.stop.force_stop_timeout =
                    Duration::from_millis(id(&options.force_stop_ms)?);
                current.stop.shutdown_timeout = Duration::from_millis(id(&options.shutdown_ms)?);
                current.io.connect_timeout = Duration::from_millis(id(&options.connect_ms)?);
                current.io.request_timeout = Duration::from_millis(id(&options.request_ms)?);
                current.output_buffer_bytes = id(&options.replay_bytes)?;
                workspace.update_options(current).await?;
            }
            TerminalAction::Detach { .. } | TerminalAction::Ack { .. } => unreachable!(),
        }
        Ok(Reply::Done)
    };
    operation.await.map_err(errors::core)
}
enum StartSource {
    Configuration(TerminalTarget, TerminalReuse),
    Local(String),
}
async fn start(
    runtime: &Runtime,
    workspace: &core::TerminalWorkspace,
    owner: String,
    source: StartSource,
    subscription: String,
    output: Option<Channel<TerminalOutputFrame>>,
) -> Result<Reply, Error> {
    let tunnel = matches!(
        &source,
        StartSource::Configuration(
            TerminalTarget {
                kind: TerminalResourceKind::Tunnel,
                ..
            },
            _
        )
    );
    if !tunnel && output.is_none() {
        return Err(invalid());
    }
    let lease = runtime
        .terminal_io
        .begin(
            &owner,
            &subscription,
            workspace.snapshot().options.control_queue_capacity,
        )
        .map_err(|_| Error::Stopped)?;
    let result = async {
        match source {
            StartSource::Local(name) => {
                let (run, stream) = workspace.start_local_terminal_with_output(name).await?;
                Ok((
                    mapping::terminal::target(TerminalResourceKind::Terminal, run),
                    Some(stream),
                ))
            }
            StartSource::Configuration(target, policy) => match target.kind {
                TerminalResourceKind::Terminal => {
                    let (run, stream) = workspace
                        .start_terminal_with_output(id(&target.id)?)
                        .await?;
                    Ok((mapping::terminal::target(target.kind, run), Some(stream)))
                }
                TerminalResourceKind::Command => {
                    let (run, stream) = workspace
                        .start_command_with_output(
                            id(&target.id)?,
                            reuse(policy),
                            lease.token.clone(),
                        )
                        .await?;
                    Ok((mapping::terminal::target(target.kind, run), Some(stream)))
                }
                TerminalResourceKind::Tunnel => Ok((
                    mapping::terminal::target(
                        target.kind,
                        workspace.start_tunnel(id(&target.id)?).await?,
                    ),
                    None,
                )),
                _ => Err(invalid()),
            },
        }
    }
    .await;
    match result {
        Ok((target, stream)) => {
            if lease.token.is_cancelled() {
                match target.kind {
                    TerminalResourceKind::Terminal => {
                        let _ = workspace.stop_terminal(id(&target.id)?).await;
                    }
                    TerminalResourceKind::Tunnel => {
                        let _ = workspace.stop_tunnel(id(&target.id)?).await;
                    }
                    _ => {}
                }
                runtime.terminal_io.release(&owner, &subscription, lease);
                return Err(Error::Stopped);
            }
            if let (Some(stream), Some(output)) = (stream, output) {
                runtime
                    .terminal_io
                    .pump(owner, subscription, lease, stream, output);
            } else {
                runtime.terminal_io.release(&owner, &subscription, lease);
            }
            Ok(Reply::TerminalOpened { target })
        }
        Err(error) => {
            runtime.terminal_io.release(&owner, &subscription, lease);
            Err(error)
        }
    }
}
