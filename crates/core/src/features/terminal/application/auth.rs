use super::requests::{Coordinator, Request};
use crate::{
    Error, Result,
    contracts::terminal::*,
    features::terminal::{
        domain::{config::*, inputs::SecretInput, views::TerminalIssue},
        ports::secrets::{SecretField, SecretsPort},
    },
};
use std::sync::{Arc, Mutex};

pub(crate) struct PasswordAnswer {
    pub password: SecretInput,
    pub remember: bool,
}
pub(crate) enum Credentials {
    Password(SecretInput),
    PrivateKey(SecretInput),
}
pub(crate) struct AuthSession {
    pub target: SshTargetConfig,
    pub secrets: Arc<dyn SecretsPort>,
    pub coordinator: Coordinator,
    pub password_to_save: Mutex<Option<SecretInput>>,
}
impl AuthSession {
    pub async fn prepare(
        &self,
        owner: SshConnectionOwnerRunId,
        cancellation: CancellationToken,
    ) -> Result<Credentials> {
        let operation = async {
            match &self.target.auth {
                SshAuthConfig::Password {
                    password: Some(secret),
                } => self
                    .secrets
                    .open(self.target.id, SecretField::Password, secret.clone())
                    .await
                    .map(Credentials::Password),
                SshAuthConfig::Password { password: None } => {
                    let answer = self
                        .coordinator
                        .call(|reply| {
                            Request::Password(
                                self.target.id,
                                owner,
                                self.target.username.clone(),
                                cancellation.clone(),
                                reply,
                            )
                        })
                        .await?;
                    if answer.remember {
                        *self.password_to_save.lock().unwrap() =
                            Some(SecretInput::new(answer.password.expose().to_vec()));
                    }
                    Ok(Credentials::Password(answer.password))
                }
                SshAuthConfig::PrivateKey { private_key } => self
                    .secrets
                    .open(self.target.id, SecretField::PrivateKey, private_key.clone())
                    .await
                    .map(Credentials::PrivateKey),
            }
        };
        tokio::select! { biased; _=cancellation.cancelled()=>Err(cancelled()), result=operation=>result }
    }
    pub async fn succeeded(&self, cancellation: CancellationToken) {
        let secret = self.password_to_save.lock().unwrap().take();
        let Some(secret) = secret else {
            return;
        };
        let operation = async {
            let secret = self
                .secrets
                .seal(self.target.id, SecretField::Password, secret)
                .await?;
            self.coordinator
                .call(|reply| {
                    Request::SavePassword(
                        self.target.id,
                        self.target.auth_revision,
                        secret,
                        cancellation.clone(),
                        reply,
                    )
                })
                .await
        };
        let result =
            tokio::select! { biased; _=cancellation.cancelled()=>return, result=operation=>result };
        if let Err(error) = result {
            self.coordinator
                .issue(TerminalIssue::CredentialNotSaved {
                    target_id: self.target.id,
                    message: error.to_string(),
                })
                .await;
        }
    }
}
pub(crate) fn cancelled() -> Error {
    TerminalError::new(
        TerminalErrorKind::Cancelled,
        "terminal interaction cancelled",
    )
    .into()
}
