use crate::{
    error::{Error, Future},
    features::file_selection::ports::SelectedTransfers,
};
use nooboard_core::terminal as core;
use std::path::PathBuf;
pub struct TerminalFiles {
    pub workspace: core::TerminalWorkspace,
    pub run: core::TerminalRunId,
    pub remote: Vec<String>,
    pub conflict: core::FileConflictPolicy,
}
fn name(value: &str) -> Result<&str, Error> {
    if value.is_empty() || matches!(value, "." | "..") || value.contains(['\\', ':', '\0']) {
        Err(nooboard_core::Error::Configuration.into())
    } else {
        Ok(value)
    }
}
impl SelectedTransfers for TerminalFiles {
    fn upload(&self, paths: Vec<PathBuf>) -> Future<'_, ()> {
        Box::pin(async move {
            let directory = self
                .remote
                .first()
                .ok_or(nooboard_core::Error::Configuration)?;
            let directory = core::RemotePath::new(directory.clone())?;
            let mut items = vec![];
            for local_path in paths {
                let filename = name(
                    local_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .ok_or(nooboard_core::Error::Configuration)?,
                )?;
                let remote_path = core::RemotePath::new(format!(
                    "{}/{}",
                    directory.as_str().trim_end_matches('/'),
                    filename
                ))?;
                items.push(core::FileTransferItem {
                    local_path,
                    remote_path,
                });
            }
            self.workspace
                .start_file_operation(core::StartFileOperationRequest {
                    terminal_run_id: self.run,
                    operation: core::FileOperationSpec::Upload {
                        items,
                        conflict_policy: self.conflict,
                    },
                })
                .await?;
            Ok(())
        })
    }
    fn download(&self, directory: PathBuf) -> Future<'_, ()> {
        Box::pin(async move {
            let mut items = vec![];
            for path in &self.remote {
                let filename = name(
                    path.rsplit('/')
                        .next()
                        .ok_or(nooboard_core::Error::Configuration)?,
                )?;
                items.push(core::FileTransferItem {
                    local_path: directory.join(filename),
                    remote_path: core::RemotePath::new(path.clone())?,
                });
            }
            self.workspace
                .start_file_operation(core::StartFileOperationRequest {
                    terminal_run_id: self.run,
                    operation: core::FileOperationSpec::Download {
                        items,
                        conflict_policy: self.conflict,
                    },
                })
                .await?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_remote_names_that_escape_selected_directory_on_supported_platforms() {
        for filename in ["", ".", "..", "..\\escape", "C:escape", "nul\0byte"] {
            assert!(name(filename).is_err(), "accepted {filename:?}");
        }
        assert_eq!(name("report.txt").unwrap(), "report.txt");
    }
}
