use super::*;
impl Runtime {
    pub(super) async fn save_ssh(
        &mut self,
        id: Option<SshTargetConfigId>,
        input: SshTargetInput,
    ) -> Result<SshTargetConfigId> {
        self.admit()?;
        let old = id.map(|id| self.catalog.ssh(id).cloned()).transpose()?;
        let id = id.unwrap_or_default();
        let auth_unchanged = matches!(&input.auth, SshAuthInput::Keep);
        let auth = match input.auth {
            SshAuthInput::Keep => old.as_ref().ok_or(Error::Configuration)?.auth.clone(),
            SshAuthInput::Password => SshAuthConfig::Password { password: None },
            SshAuthInput::ImportPrivateKey {
                private_key,
                passphrase,
            } => {
                let key = self.keys.unlock(private_key, passphrase).await?;
                SshAuthConfig::PrivateKey {
                    private_key: self.secrets.seal(id, SecretField::PrivateKey, key).await?,
                }
            }
        };
        let auth_revision = match old {
            Some(old)
                if auth_unchanged
                    && old.username == input.username
                    && old.address == input.address
                    && old.port == input.port =>
            {
                old.auth_revision
            }
            Some(old) => old
                .auth_revision
                .checked_add(1)
                .ok_or(Error::Configuration)?,
            None => 0,
        };
        let mut next = self.catalog.clone();
        next.ssh_targets.insert(
            id,
            SshTargetConfig {
                id,
                name: input.name,
                address: input.address,
                port: input.port,
                username: input.username,
                auth,
                auth_revision,
            },
        );
        self.commit(next).await?;
        Ok(id)
    }
    pub(super) async fn save_command(
        &mut self,
        id: Option<CommandConfigId>,
        input: CommandInput,
    ) -> Result<CommandConfigId> {
        self.admit()?;
        if id.is_some_and(|id| !self.catalog.commands.contains_key(&id)) {
            return Err(Error::NotFound);
        }
        let id = id.unwrap_or_default();
        let mut next = self.catalog.clone();
        next.commands.insert(
            id,
            CommandConfig {
                id,
                name: input.name,
                target: input.target,
                command: input.command,
            },
        );
        self.commit(next).await?;
        Ok(id)
    }
    pub(super) async fn save_terminal(
        &mut self,
        id: Option<TerminalConfigId>,
        input: TerminalInput,
    ) -> Result<TerminalConfigId> {
        self.admit()?;
        if id.is_some_and(|id| !self.catalog.terminals.contains_key(&id)) {
            return Err(Error::NotFound);
        }
        let id = id.unwrap_or_default();
        let mut next = self.catalog.clone();
        next.terminals.insert(
            id,
            TerminalConfig {
                id,
                name: input.name,
                target: input.target,
                dependencies: input.dependencies,
            },
        );
        self.commit(next).await?;
        Ok(id)
    }
    pub(super) async fn save_tunnel(
        &mut self,
        id: Option<TunnelConfigId>,
        input: TunnelInput,
    ) -> Result<TunnelConfigId> {
        self.admit()?;
        if id.is_some_and(|id| !self.catalog.tunnels.contains_key(&id)) {
            return Err(Error::NotFound);
        }
        let id = id.unwrap_or_default();
        let mut next = self.catalog.clone();
        next.tunnels.insert(
            id,
            TunnelConfig {
                id,
                name: input.name,
                target: input.target,
                dependencies: input.dependencies,
                forward: input.forward,
            },
        );
        self.commit(next).await?;
        Ok(id)
    }
    pub(super) async fn copy(
        &mut self,
        reference: TerminalConfigReference,
    ) -> Result<TerminalConfigReference> {
        self.admit()?;
        let mut next = self.catalog.clone();
        let copied = match reference {
            TerminalConfigReference::SshTarget(id) => {
                let mut c = self.catalog.ssh(id)?.clone();
                c.id = SshTargetConfigId::new();
                c.auth_revision = 0;
                c.auth = match c.auth {
                    SshAuthConfig::Password {
                        password: Some(secret),
                    } => {
                        let plain = self.secrets.open(id, SecretField::Password, secret).await?;
                        SshAuthConfig::Password {
                            password: Some(
                                self.secrets
                                    .seal(c.id, SecretField::Password, plain)
                                    .await?,
                            ),
                        }
                    }
                    SshAuthConfig::PrivateKey { private_key } => {
                        let plain = self
                            .secrets
                            .open(id, SecretField::PrivateKey, private_key)
                            .await?;
                        SshAuthConfig::PrivateKey {
                            private_key: self
                                .secrets
                                .seal(c.id, SecretField::PrivateKey, plain)
                                .await?,
                        }
                    }
                    auth => auth,
                };
                let new = c.id;
                next.ssh_targets.insert(new, c);
                TerminalConfigReference::SshTarget(new)
            }
            TerminalConfigReference::Command(id) => {
                let mut c = next.commands.get(&id).cloned().ok_or(Error::NotFound)?;
                c.id = CommandConfigId::new();
                let new = c.id;
                next.commands.insert(new, c);
                TerminalConfigReference::Command(new)
            }
            TerminalConfigReference::Terminal(id) => {
                let mut c = next.terminals.get(&id).cloned().ok_or(Error::NotFound)?;
                c.id = TerminalConfigId::new();
                let new = c.id;
                next.terminals.insert(new, c);
                TerminalConfigReference::Terminal(new)
            }
            TerminalConfigReference::Tunnel(id) => {
                let mut c = next.tunnels.get(&id).cloned().ok_or(Error::NotFound)?;
                c.id = TunnelConfigId::new();
                let new = c.id;
                next.tunnels.insert(new, c);
                TerminalConfigReference::Tunnel(new)
            }
        };
        self.commit(next).await?;
        Ok(copied)
    }
    pub(super) async fn delete(&mut self, reference: TerminalConfigReference) -> Result<()> {
        self.admit()?;
        let referenced_by = self.catalog.referrers(reference);
        let uses = self.references.uses(reference);
        if !referenced_by.is_empty() || !uses.is_empty() {
            return Err(Error::ConfigurationInUse {
                configuration: reference,
                referenced_by,
                active_runs: uses.iter().filter_map(|id| *id).collect(),
                pending: uses.contains(&None),
            });
        }
        let mut next = self.catalog.clone();
        let removed = match reference {
            TerminalConfigReference::SshTarget(id) => {
                next.trusted_hosts.remove(&id);
                next.ssh_targets.remove(&id).is_some()
            }
            TerminalConfigReference::Command(id) => next.commands.remove(&id).is_some(),
            TerminalConfigReference::Terminal(id) => next.terminals.remove(&id).is_some(),
            TerminalConfigReference::Tunnel(id) => next.tunnels.remove(&id).is_some(),
        };
        if !removed {
            return Err(Error::NotFound);
        }
        self.commit(next).await
    }
    pub(super) async fn save_password(
        &mut self,
        id: SshTargetConfigId,
        revision: u64,
        password: EncryptedSecret,
        cancel: CancellationToken,
    ) -> Result<()> {
        self.admit()?;
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        let mut next = self.catalog.clone();
        let target = next.ssh_targets.get_mut(&id).ok_or(Error::NotFound)?;
        if target.auth_revision != revision
            || !matches!(target.auth, SshAuthConfig::Password { password: None })
        {
            return Err(Error::Configuration);
        }
        target.auth = SshAuthConfig::Password {
            password: Some(password),
        };
        target.auth_revision = target
            .auth_revision
            .checked_add(1)
            .ok_or(Error::Configuration)?;
        self.commit(next).await
    }
    pub(super) async fn replace_host(
        &mut self,
        id: SshTargetConfigId,
        expected: Option<SshPublicKey>,
        key: SshPublicKey,
    ) -> Result<()> {
        self.admit()?;
        self.catalog.ssh(id)?;
        if self.catalog.trusted_hosts.get(&id) != expected.as_ref() {
            return Err(Error::Configuration);
        }
        let mut next = self.catalog.clone();
        next.trusted_hosts.insert(id, key);
        self.commit(next).await
    }
    pub(super) async fn options(&mut self, options: TerminalServiceOptions) -> Result<()> {
        self.admit()?;
        if options.control_queue_capacity != self.catalog.options.control_queue_capacity {
            return Err(Error::Configuration);
        }
        let mut next = self.catalog.clone();
        next.options = options.clone();
        self.commit(next).await?;
        let result = self.engine.update_options(options).await;
        if let Err(error) = &result {
            self.issue = Some(TerminalIssue::Service {
                message: error.to_string(),
            });
        }
        result
    }
}
