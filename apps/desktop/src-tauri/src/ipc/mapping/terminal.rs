use crate::ipc::contracts::*;
use nooboard_core::terminal as core;
pub fn target(kind: TerminalResourceKind, id: impl ToString) -> TerminalTarget {
    TerminalTarget {
        kind,
        id: id.to_string(),
    }
}
fn key(value: core::SshPublicKey) -> TerminalHostKey {
    TerminalHostKey {
        algorithm: value.algorithm,
        bytes: value.bytes,
    }
}
fn address(value: core::SshAddressSource) -> TerminalAddress {
    match value {
        core::SshAddressSource::Host(value) => TerminalAddress {
            kind: TerminalAddressKind::Host,
            value,
        },
        core::SshAddressSource::Device(value) => TerminalAddress {
            kind: TerminalAddressKind::Device,
            value: value.0,
        },
    }
}
fn dependencies(values: Vec<core::DependencyConfig>) -> Vec<TerminalDependency> {
    values
        .into_iter()
        .map(|d| TerminalDependency {
            command_id: d.command_config_id.to_string(),
            reuse: match d.reuse_policy {
                core::CommandReusePolicy::Shared => TerminalReuse::Shared,
                core::CommandReusePolicy::AlwaysNew => TerminalReuse::AlwaysNew,
            },
        })
        .collect()
}
fn forward(value: core::TunnelForwardConfig) -> TerminalForward {
    let (kind, listen, destination) = match value {
        core::TunnelForwardConfig::Local {
            listen,
            destination,
        } => (TerminalForwardKind::Local, listen, Some(destination)),
        core::TunnelForwardConfig::Remote {
            listen,
            destination,
        } => (TerminalForwardKind::Remote, listen, Some(destination)),
        core::TunnelForwardConfig::Dynamic { listen } => {
            (TerminalForwardKind::Dynamic, listen, None)
        }
    };
    TerminalForward {
        kind,
        listen_host: listen.host,
        listen_port: listen.port,
        destination_host: destination.as_ref().map(|d| d.host.clone()),
        destination_port: destination.map(|d| d.port),
    }
}
fn run_target(value: core::RunTarget) -> (Option<String>, Option<String>) {
    match value {
        core::RunTarget::Local => (None, None),
        core::RunTarget::Ssh(s) => (
            Some(s.config_id.to_string()),
            s.resolved_address.map(|a| a.to_string()),
        ),
    }
}
fn owner(value: core::SshConnectionOwnerRunId) -> TerminalTarget {
    match value {
        core::SshConnectionOwnerRunId::Terminal(id) => target(TerminalResourceKind::Terminal, id),
        core::SshConnectionOwnerRunId::Command(id) => target(TerminalResourceKind::Command, id),
        core::SshConnectionOwnerRunId::Tunnel(id) => target(TerminalResourceKind::Tunnel, id),
    }
}
pub fn workspace(value: core::TerminalWorkspaceSnapshot) -> TerminalWorkspaceView {
    let mut configs = vec![];
    for c in value.ssh_targets {
        let (auth, password_saved) = match c.auth {
            core::SshAuthSummary::Password { saved } => (TerminalAuthKind::Password, saved),
            core::SshAuthSummary::PrivateKey => (TerminalAuthKind::PrivateKey, false),
        };
        configs.push(TerminalConfigView {
            id: c.id.to_string(),
            config: TerminalConfigData::Ssh(TerminalSshView {
                name: c.name,
                address: address(c.address),
                port: c.port,
                username: c.username,
                auth,
                password_saved,
                trusted_key: c.trusted_key.map(key),
            }),
        });
    }
    for c in value.commands {
        configs.push(TerminalConfigView {
            id: c.id.to_string(),
            config: TerminalConfigData::Command(TerminalCommandInput {
                name: c.name,
                ssh_target: match c.target {
                    core::CommandTarget::Local => None,
                    core::CommandTarget::Ssh(id) => Some(id.to_string()),
                },
                command: c.command,
            }),
        });
    }
    for c in value.terminals {
        configs.push(TerminalConfigView {
            id: c.id.to_string(),
            config: TerminalConfigData::Terminal(TerminalSessionInput {
                name: c.name,
                ssh_target: match c.target {
                    core::TerminalTarget::Local => None,
                    core::TerminalTarget::Ssh(id) => Some(id.to_string()),
                },
                dependencies: dependencies(c.dependencies),
            }),
        });
    }
    for c in value.tunnels {
        configs.push(TerminalConfigView {
            id: c.id.to_string(),
            config: TerminalConfigData::Tunnel(TerminalTunnelInput {
                name: c.name,
                ssh_target: c.target.to_string(),
                dependencies: dependencies(c.dependencies),
                forward: forward(c.forward),
            }),
        });
    }
    let mut runs = vec![];
    for r in value.runs.terminal_runs {
        let (phase, error) = match r.state {
            core::TerminalRunState::WaitingForDependencies => (TerminalPhase::Waiting, None),
            core::TerminalRunState::Starting => (TerminalPhase::Starting, None),
            core::TerminalRunState::Running => (TerminalPhase::Running, None),
            core::TerminalRunState::Stopping { error } => {
                (TerminalPhase::Stopping, error.map(|e| e.to_string()))
            }
            core::TerminalRunState::Ended { .. } => continue,
        };
        let (ssh_target, address) = run_target(r.target);
        runs.push(TerminalRunView {
            target: target(TerminalResourceKind::Terminal, r.id),
            config_id: r.config_id.map(|id| id.to_string()),
            name: r.name,
            phase,
            ssh_target,
            address,
            command: None,
            lifetime: None,
            forward: None,
            error,
        });
    }
    for r in value.runs.command_runs {
        let (phase, error) = match r.state {
            core::CommandRunState::Starting => (TerminalPhase::Starting, None),
            core::CommandRunState::Running => (TerminalPhase::Running, None),
            core::CommandRunState::Stopping { error } => {
                (TerminalPhase::Stopping, error.map(|e| e.to_string()))
            }
            core::CommandRunState::Ended { .. } => continue,
        };
        let (ssh_target, address) = run_target(r.target);
        runs.push(TerminalRunView {
            target: target(TerminalResourceKind::Command, r.id),
            config_id: Some(r.config_id.to_string()),
            name: r.config_name,
            phase,
            ssh_target,
            address,
            command: Some(r.command),
            lifetime: Some(match r.lifetime_policy {
                core::CommandLifetimePolicy::Dependent => TerminalLifetime::Dependent,
                core::CommandLifetimePolicy::Independent => TerminalLifetime::Independent,
            }),
            forward: None,
            error,
        });
    }
    for r in value.runs.tunnel_runs {
        let (phase, error) = match r.state {
            core::TunnelRunState::WaitingForDependencies => (TerminalPhase::Waiting, None),
            core::TunnelRunState::Starting => (TerminalPhase::Starting, None),
            core::TunnelRunState::Running => (TerminalPhase::Running, None),
            core::TunnelRunState::Stopping { error } => {
                (TerminalPhase::Stopping, error.map(|e| e.to_string()))
            }
            core::TunnelRunState::Ended { .. } => continue,
        };
        runs.push(TerminalRunView {
            target: target(TerminalResourceKind::Tunnel, r.id),
            config_id: Some(r.config_id.to_string()),
            name: r.config_name,
            phase,
            ssh_target: Some(r.target.config_id.to_string()),
            address: r.target.resolved_address.map(|a| a.to_string()),
            command: None,
            lifetime: None,
            forward: Some(forward(r.forward)),
            error,
        });
    }
    let dependencies = value
        .runs
        .dependency_runs
        .into_iter()
        .map(|d| {
            let (gate, reason) = match d.gate_state {
                core::DependencyGateState::Pending => (TerminalGate::Pending, None),
                core::DependencyGateState::WaitingForInstance => {
                    (TerminalGate::WaitingForInstance, None)
                }
                core::DependencyGateState::WaitingForRelease => {
                    (TerminalGate::WaitingForRelease, None)
                }
                core::DependencyGateState::Released { reason } => {
                    (TerminalGate::Released, Some(format!("{reason:?}")))
                }
                core::DependencyGateState::Failed { error } => {
                    (TerminalGate::Failed, Some(error.to_string()))
                }
                core::DependencyGateState::Cancelled => (TerminalGate::Cancelled, None),
            };
            TerminalDependencyView {
                id: d.id.to_string(),
                owner: match d.owner_run_id {
                    core::DependencyOwnerRunId::Terminal(id) => {
                        target(TerminalResourceKind::Terminal, id)
                    }
                    core::DependencyOwnerRunId::Tunnel(id) => {
                        target(TerminalResourceKind::Tunnel, id)
                    }
                },
                command_id: d.command_config_id.to_string(),
                run_id: d.command_run_id.map(|id| id.to_string()),
                gate,
                reason,
            }
        })
        .collect();
    let prompts = value
        .prompts
        .into_iter()
        .map(|p| TerminalPromptView {
            id: p.id.to_string(),
            target_id: p.target_id.to_string(),
            owner: owner(p.owner),
            prompt: match p.kind {
                core::TerminalPromptKind::HostTrust {
                    address,
                    server_key,
                    fingerprint,
                } => TerminalPromptData::Host {
                    address: address.to_string(),
                    key: key(server_key),
                    fingerprint,
                },
                core::TerminalPromptKind::Password { username } => {
                    TerminalPromptData::Password { username }
                }
            },
        })
        .collect();
    let issue = value.issue.map(|issue| match issue {
        core::TerminalIssue::CredentialNotSaved { target_id, message } => TerminalIssueView {
            kind: "credentialNotSaved".into(),
            target_id: Some(target_id.to_string()),
            message: Some(message),
            previous: None,
            received: None,
        },
        core::TerminalIssue::HostKeyChanged {
            target_id,
            previous,
            received,
        } => TerminalIssueView {
            kind: "hostKeyChanged".into(),
            target_id: Some(target_id.to_string()),
            message: None,
            previous: Some(key(previous)),
            received: Some(key(received)),
        },
        core::TerminalIssue::Service { message } => TerminalIssueView {
            kind: "service".into(),
            target_id: None,
            message: Some(message),
            previous: None,
            received: None,
        },
    });
    let files = value
        .runs
        .file_operation_runs
        .into_iter()
        .filter_map(|f| {
            let (phase, error) = match f.state {
                core::FileOperationRunState::Starting => (TerminalPhase::Starting, None),
                core::FileOperationRunState::Running => (TerminalPhase::Running, None),
                core::FileOperationRunState::Stopping { error } => {
                    (TerminalPhase::Stopping, error.map(|e| e.to_string()))
                }
                core::FileOperationRunState::Ended { .. } => return None,
            };
            Some(TerminalFileRun {
                id: f.id.to_string(),
                terminal_id: f.terminal_run_id.to_string(),
                operation: match f.operation {
                    core::FileOperationSpec::Upload { .. } => "upload",
                    core::FileOperationSpec::Download { .. } => "download",
                    core::FileOperationSpec::Delete { .. } => "delete",
                }
                .into(),
                phase,
                completed: f.progress.completed_entries.to_string(),
                total: f.progress.total_entries.map(|v| v.to_string()),
                bytes: f.progress.transferred_bytes.to_string(),
                total_bytes: f.progress.total_bytes.map(|v| v.to_string()),
                error,
            })
        })
        .collect();
    let options = TerminalOptionsView {
        graceful_stop_ms: value
            .options
            .stop
            .stop_grace_timeout
            .as_millis()
            .to_string(),
        force_stop_ms: value
            .options
            .stop
            .force_stop_timeout
            .as_millis()
            .to_string(),
        shutdown_ms: value.options.stop.shutdown_timeout.as_millis().to_string(),
        connect_ms: value.options.io.connect_timeout.as_millis().to_string(),
        request_ms: value.options.io.request_timeout.as_millis().to_string(),
        replay_bytes: value.options.output_buffer_bytes.to_string(),
    };
    TerminalWorkspaceView {
        revision: value.revision.to_string(),
        closing: value.closing,
        configs,
        runs,
        dependencies,
        prompts,
        files,
        issue,
        options,
    }
}
pub fn directory(value: core::RemoteDirectoryListing) -> TerminalDirectory {
    TerminalDirectory {
        path: value.path.to_string(),
        entries: value
            .entries
            .into_iter()
            .map(|e| TerminalDirectoryEntry {
                name: e.name,
                kind: match e.kind {
                    core::RemoteEntryKind::File => "file",
                    core::RemoteEntryKind::Directory => "directory",
                    core::RemoteEntryKind::SymbolicLink => "link",
                    core::RemoteEntryKind::Other => "other",
                    core::RemoteEntryKind::Unknown => "unknown",
                }
                .into(),
                size: e.size.map(|v| v.to_string()),
            })
            .collect(),
    }
}
pub fn ended(event: core::TerminalEvent) -> TerminalEnded {
    let (kind, id, name, result, error, cleanup) = match event {
        core::TerminalEvent::TerminalEnded { run } => {
            let (result, error) = match run.state {
                core::TerminalRunState::Ended {
                    result: core::TerminalRunResult::Succeeded,
                } => ("succeeded", None),
                core::TerminalRunState::Ended {
                    result: core::TerminalRunResult::Failed(error),
                } => ("failed", Some(error.to_string())),
                _ => ("stopped", None),
            };
            (
                "terminal",
                run.id.to_string(),
                run.name,
                result,
                error,
                vec![],
            )
        }
        core::TerminalEvent::CommandEnded { run } => {
            let (result, error) = match run.state {
                core::CommandRunState::Ended {
                    result: core::CommandRunResult::Succeeded,
                } => ("succeeded", None),
                core::CommandRunState::Ended {
                    result: core::CommandRunResult::Failed(error),
                } => ("failed", Some(error.to_string())),
                _ => ("stopped", None),
            };
            (
                "command",
                run.id.to_string(),
                run.config_name,
                result,
                error,
                vec![],
            )
        }
        core::TerminalEvent::TunnelEnded { run } => {
            let (result, error) = match run.state {
                core::TunnelRunState::Ended {
                    result: core::TunnelRunResult::Failed(error),
                } => ("failed", Some(error.to_string())),
                _ => ("stopped", None),
            };
            (
                "tunnel",
                run.id.to_string(),
                run.config_name,
                result,
                error,
                vec![],
            )
        }
        core::TerminalEvent::FileOperationEnded { run } => {
            let (result, error, cleanup) = match run.state {
                core::FileOperationRunState::Ended {
                    result,
                    cleanup_issues,
                } => {
                    let (result, error) = match result {
                        core::FileOperationRunResult::Succeeded => ("succeeded", None),
                        core::FileOperationRunResult::Failed(error) => {
                            ("failed", Some(error.to_string()))
                        }
                        core::FileOperationRunResult::Stopped => ("stopped", None),
                    };
                    (
                        result,
                        error,
                        cleanup_issues
                            .into_iter()
                            .map(|i| {
                                format!(
                                    "{}: {}",
                                    match i.path {
                                        core::FileOperationPath::Local(p) =>
                                            p.display().to_string(),
                                        core::FileOperationPath::Remote(p) => p.to_string(),
                                    },
                                    i.error
                                )
                            })
                            .collect(),
                    )
                }
                _ => ("stopped", None, vec![]),
            };
            (
                "file",
                run.id.to_string(),
                String::new(),
                result,
                error,
                cleanup,
            )
        }
    };
    TerminalEnded {
        kind: kind.into(),
        id,
        name,
        result: result.into(),
        error,
        cleanup,
    }
}
