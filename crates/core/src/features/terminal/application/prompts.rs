use super::*;
impl Runtime {
    pub(super) fn prompt(
        &mut self,
        target: SshTargetConfigId,
        owner: SshConnectionOwnerRunId,
        kind: TerminalPromptKind,
        cancellation: CancellationToken,
        reply: PromptReply,
    ) {
        let id = TerminalPromptId(uuid::Uuid::new_v4());
        let done = CancellationToken::new();
        let finished = done.clone();
        let cancelled = cancellation.clone();
        self.prompts.spawn(async move {
            tokio::select! {_=finished.cancelled()=>None,_=cancelled.cancelled()=>Some(id)}
        });
        self.pending.insert(
            id,
            Pending {
                view: TerminalPrompt {
                    id,
                    target_id: target,
                    owner,
                    kind,
                },
                reply: Some(reply),
                cancellation,
                done,
            },
        );
    }
    pub(super) fn verify(
        &mut self,
        request: SshHostVerificationRequest,
        fingerprint: String,
        cancellation: CancellationToken,
        reply: Reply<()>,
    ) {
        if let Err(error) = self.admit().and_then(|()| {
            if cancellation.is_cancelled() {
                Err(cancelled())
            } else {
                Ok(())
            }
        }) {
            let _ = reply.send(Err(error));
            return;
        }
        match self.catalog.trusted_hosts.get(&request.target_config_id) {
            Some(key) if key == &request.server_key => {
                let _ = reply.send(Ok(()));
            }
            Some(key) => {
                self.issue = Some(TerminalIssue::HostKeyChanged {
                    target_id: request.target_config_id,
                    previous: key.clone(),
                    received: request.server_key,
                });
                let _ = reply.send(Err(TerminalError::new(
                    TerminalErrorKind::HostKeyRejected,
                    "SSH host key changed; update trust before starting again",
                )
                .into()));
            }
            None => self.prompt(
                request.target_config_id,
                request.owner_run_id,
                TerminalPromptKind::HostTrust {
                    address: request.address,
                    server_key: request.server_key,
                    fingerprint,
                },
                cancellation,
                PromptReply::Host(reply),
            ),
        }
    }
    pub(super) async fn confirm_host(&mut self, id: TerminalPromptId, accept: bool) -> Result<()> {
        let pending = self.pending.get(&id).ok_or(Error::NotFound)?;
        let TerminalPromptKind::HostTrust { server_key, .. } = &pending.view.kind else {
            return Err(Error::Configuration);
        };
        let target = pending.view.target_id;
        let key = server_key.clone();
        let cancel = pending.cancellation.clone();
        let result = async {
            self.admit()?;
            if cancel.is_cancelled() {
                return Err(cancelled());
            }
            if !accept {
                return Err(TerminalError::new(
                    TerminalErrorKind::HostKeyRejected,
                    "SSH host was not trusted",
                )
                .into());
            }
            self.catalog.ssh(target)?;
            if let Some(existing) = self.catalog.trusted_hosts.get(&target) {
                if existing != &key {
                    return Err(TerminalError::new(
                        TerminalErrorKind::HostKeyRejected,
                        "another host key was trusted concurrently",
                    )
                    .into());
                }
                return Ok(());
            }
            let mut next = self.catalog.clone();
            next.trusted_hosts.insert(target, key);
            self.commit(next).await
        }
        .await;
        let action = result.as_ref().map(|_| ()).map_err(|e| {
            TerminalError::new(TerminalErrorKind::HostKeyRejected, e.to_string()).into()
        });
        if let Some(mut pending) = self.pending.remove(&id)
            && let Some(PromptReply::Host(reply)) = pending.reply.take()
        {
            let _ = reply.send(result);
        }
        action
    }
    pub(super) fn answer_password(
        &mut self,
        id: TerminalPromptId,
        password: SecretInput,
        remember: bool,
    ) -> Result<()> {
        self.admit()?;
        let pending = self.pending.get(&id).ok_or(Error::NotFound)?;
        if !matches!(pending.view.kind, TerminalPromptKind::Password { .. }) {
            return Err(Error::Configuration);
        }
        if pending.cancellation.is_cancelled() {
            self.cancel(id);
            return Err(cancelled());
        }
        let mut pending = self.pending.remove(&id).unwrap();
        if let Some(PromptReply::Password(reply)) = pending.reply.take() {
            reply
                .send(Ok(PasswordAnswer { password, remember }))
                .map_err(|_| cancelled())?;
        }
        Ok(())
    }
    pub(super) fn cancel(&mut self, id: TerminalPromptId) {
        if let Some(mut pending) = self.pending.remove(&id) {
            pending.cancellation.cancel();
            match pending.reply.take() {
                Some(PromptReply::Host(reply)) => {
                    let _ = reply.send(Err(cancelled()));
                }
                Some(PromptReply::Password(reply)) => {
                    let _ = reply.send(Err(cancelled()));
                }
                None => {}
            }
        }
    }
    pub(super) fn cancel_all(&mut self) {
        let ids = self.pending.keys().copied().collect::<Vec<_>>();
        for id in ids {
            self.cancel(id);
        }
    }
}
