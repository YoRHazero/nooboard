use crate::ipc::{
    errors::{self, UiError},
    remote::{RemoteReplay, RemoteReply, RemoteRequest},
};
use nooboard_core::{PtySize, Remote, RemoteLogin, RemotePath, SshPassword};
pub async fn run(remote: Remote, request: RemoteRequest) -> Result<RemoteReply, UiError> {
    let path = |p| RemotePath::new(p).map_err(errors::ssh);
    match request {
        RemoteRequest::Connect {
            workspace,
            device,
            username,
            port,
            password,
            remember,
        } => {
            if password.as_ref().is_some_and(|p| p.len() > 4096) {
                return Err(errors::ui("configuration"));
            }
            let id = remote
                .connect(RemoteLogin {
                    workspace,
                    device_id: device,
                    username,
                    port,
                    password: password.map(SshPassword::new),
                    remember,
                })
                .await
                .map_err(errors::remote)?;
            return Ok(RemoteReply::Connected { id });
        }
        RemoteRequest::HostKey { id, accept } => remote.decide_host_key(id, accept).await,
        RemoteRequest::ForgetHostKey { device, port } => remote.forget_host_key(device, port).await,
        RemoteRequest::Close { workspace } => remote.close_workspace(workspace).await,
        RemoteRequest::Input {
            workspace,
            session,
            bytes,
        } => remote.write_terminal(workspace, session, bytes).await,
        RemoteRequest::Resize {
            workspace,
            session,
            columns,
            rows,
        } => {
            remote
                .resize_terminal(workspace, session, PtySize { columns, rows })
                .await
        }
        RemoteRequest::Replay {
            workspace,
            session,
            after,
        } => {
            let after = after.parse().map_err(|_| errors::ui("configuration"))?;
            let replay = remote
                .terminal_replay(workspace, session, after)
                .await
                .map_err(errors::remote)?;
            return Ok(RemoteReply::Replay(RemoteReplay {
                start: replay.start.to_string(),
                end: replay.end.to_string(),
                gap: replay.gap,
                bytes: replay.bytes,
            }));
        }
        RemoteRequest::Navigate {
            target,
            path: input,
        } => remote.navigate(target.into_core()?, input).await,
        RemoteRequest::List {
            target,
            path: p,
            more,
        } => {
            remote
                .list_directory(target.into_core()?, path(p)?, more)
                .await
        }
        RemoteRequest::Collapse { target, path: p } => {
            remote
                .collapse_directory(target.into_core()?, path(p)?)
                .await
        }
        RemoteRequest::SelectUpload { target, directory } => {
            // Freeze the destination before opening the native picker.
            let directory = path(directory)?;
            let Some(files) = rfd::AsyncFileDialog::new().pick_files().await else {
                return Ok(RemoteReply::Done);
            };
            remote
                .upload(
                    target.into_core()?,
                    directory,
                    files.into_iter().map(|f| f.path().to_owned()).collect(),
                )
                .await
        }
        RemoteRequest::Upload {
            target,
            directory,
            paths,
        } => {
            remote
                .upload(
                    target.into_core()?,
                    path(directory)?,
                    paths.into_iter().map(Into::into).collect(),
                )
                .await
        }
        RemoteRequest::Download { target, paths } => {
            remote
                .download(
                    target.into_core()?,
                    paths.into_iter().map(path).collect::<Result<_, _>>()?,
                )
                .await
        }
        RemoteRequest::SelectDownloadDirectory => {
            let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await else {
                return Ok(RemoteReply::Done);
            };
            remote
                .set_download_directory(folder.path().to_owned())
                .await
        }
        RemoteRequest::Cancel { id } => remote.cancel_transfer(id).await,
        RemoteRequest::Retry { id } => remote.retry_transfer(id).await,
        RemoteRequest::ForgetTransfer { id } => remote.forget_transfer(id).await,
        RemoteRequest::Reveal { id } => {
            let file = remote.downloaded_file(&id).map_err(errors::remote)?;
            reveal(file).await.map_err(|_| errors::ui("sshIo"))?;
            return Ok(RemoteReply::Done);
        }
    }
    .map_err(errors::remote)?;
    Ok(RemoteReply::Done)
}
async fn reveal(path: std::path::PathBuf) -> std::io::Result<()> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .status();
        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", path.display()))
            .status();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let result = std::process::Command::new("xdg-open")
            .arg(path.parent().unwrap_or(&path))
            .status();
        match result {
            Ok(status) if status.success() => Ok(()),
            Ok(_) => Err(std::io::Error::other("file manager failed")),
            Err(e) => Err(e),
        }
    })
    .await
    .map_err(std::io::Error::other)?
}
