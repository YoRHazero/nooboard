//! Terminal wire values contain neither credential ciphertext nor secret outputs in snapshots.
use super::*;
macro_rules! value_enum {($name:ident {$($v:ident),+})=>{
    #[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize,JsonSchema)]
    #[serde(rename_all="camelCase")]
    pub enum $name {$($v),+}
};}
value_enum!(TerminalResourceKind {
    Terminal,
    Command,
    Tunnel,
    Ssh
});
value_enum!(TerminalReuse { Shared, AlwaysNew });
value_enum!(TerminalLifetime {
    Dependent,
    Independent
});
value_enum!(TerminalPhase {
    Waiting,
    Starting,
    Running,
    Stopping
});
value_enum!(TerminalGate {
    Pending,
    WaitingForInstance,
    WaitingForRelease,
    Released,
    Failed,
    Cancelled
});
value_enum!(TerminalAddressKind { Host, Device });
value_enum!(TerminalAuthKind {
    Password,
    PrivateKey
});
value_enum!(TerminalAuthEdit {
    Keep,
    Password,
    PrivateKey
});
value_enum!(TerminalForwardKind {
    Local,
    Remote,
    Dynamic
});
value_enum!(TerminalConflict {
    Fail,
    Skip,
    Overwrite
});
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalTarget {
    pub kind: TerminalResourceKind,
    pub id: String,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalDependency {
    pub command_id: String,
    pub reuse: TerminalReuse,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalAddress {
    pub kind: TerminalAddressKind,
    pub value: String,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalForward {
    pub kind: TerminalForwardKind,
    pub listen_host: String,
    pub listen_port: u16,
    pub destination_host: Option<String>,
    pub destination_port: Option<u16>,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TerminalHostKey {
    pub algorithm: String,
    pub bytes: Vec<u8>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalSshInput {
    pub name: String,
    pub address: TerminalAddress,
    pub port: u16,
    pub username: String,
    pub auth: TerminalAuthEdit,
    pub private_key: Option<String>,
    pub passphrase: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalCommandInput {
    pub name: String,
    pub ssh_target: Option<String>,
    pub command: String,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalSessionInput {
    pub name: String,
    pub ssh_target: Option<String>,
    pub dependencies: Vec<TerminalDependency>,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalTunnelInput {
    pub name: String,
    pub ssh_target: String,
    pub dependencies: Vec<TerminalDependency>,
    pub forward: TerminalForward,
}
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum TerminalConfigInput {
    Terminal(TerminalSessionInput),
    Command(TerminalCommandInput),
    Tunnel(TerminalTunnelInput),
    Ssh(TerminalSshInput),
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSshView {
    pub name: String,
    pub address: TerminalAddress,
    pub port: u16,
    pub username: String,
    pub auth: TerminalAuthKind,
    pub password_saved: bool,
    pub trusted_key: Option<TerminalHostKey>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum TerminalConfigData {
    Terminal(TerminalSessionInput),
    Command(TerminalCommandInput),
    Tunnel(TerminalTunnelInput),
    Ssh(TerminalSshView),
}
#[derive(Clone, Serialize, JsonSchema)]
pub struct TerminalConfigView {
    pub id: String,
    #[serde(flatten)]
    pub config: TerminalConfigData,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalRunView {
    pub target: TerminalTarget,
    pub config_id: Option<String>,
    pub name: String,
    pub phase: TerminalPhase,
    pub ssh_target: Option<String>,
    pub address: Option<String>,
    pub command: Option<String>,
    pub lifetime: Option<TerminalLifetime>,
    pub forward: Option<TerminalForward>,
    pub error: Option<String>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalDependencyView {
    pub id: String,
    pub owner: TerminalTarget,
    pub command_id: String,
    pub run_id: Option<String>,
    pub gate: TerminalGate,
    pub reason: Option<String>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum TerminalPromptData {
    Host {
        address: String,
        key: TerminalHostKey,
        fingerprint: String,
    },
    Password {
        username: String,
    },
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalPromptView {
    pub id: String,
    pub target_id: String,
    pub owner: TerminalTarget,
    pub prompt: TerminalPromptData,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalIssueView {
    pub kind: String,
    pub message: Option<String>,
    pub target_id: Option<String>,
    pub previous: Option<TerminalHostKey>,
    pub received: Option<TerminalHostKey>,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalOptionsView {
    pub graceful_stop_ms: String,
    pub force_stop_ms: String,
    pub shutdown_ms: String,
    pub connect_ms: String,
    pub request_ms: String,
    pub replay_bytes: String,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalFileRun {
    pub id: String,
    pub terminal_id: String,
    pub operation: String,
    pub phase: TerminalPhase,
    pub completed: String,
    pub total: Option<String>,
    pub bytes: String,
    pub total_bytes: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalWorkspaceView {
    pub revision: String,
    pub closing: bool,
    pub configs: Vec<TerminalConfigView>,
    pub runs: Vec<TerminalRunView>,
    pub dependencies: Vec<TerminalDependencyView>,
    pub prompts: Vec<TerminalPromptView>,
    pub files: Vec<TerminalFileRun>,
    pub issue: Option<TerminalIssueView>,
    pub options: TerminalOptionsView,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalDirectoryEntry {
    pub name: String,
    pub kind: String,
    pub size: Option<String>,
}
#[derive(Serialize, JsonSchema)]
pub struct TerminalDirectory {
    pub path: String,
    pub entries: Vec<TerminalDirectoryEntry>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalEnded {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub result: String,
    pub error: Option<String>,
    pub cleanup: Vec<String>,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum TerminalOutputFrame {
    Data {
        sequence: String,
        offset: String,
        bytes: Vec<u8>,
    },
    Gap {
        sequence: String,
        offset: String,
    },
    Closed,
}
#[derive(Deserialize, JsonSchema)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalAction {
    SaveConfig {
        id: Option<String>,
        config: TerminalConfigInput,
    },
    CopyConfig {
        target: TerminalTarget,
    },
    DeleteConfig {
        target: TerminalTarget,
    },
    StartLocal {
        name: String,
        subscription: String,
    },
    Start {
        target: TerminalTarget,
        reuse: TerminalReuse,
        subscription: String,
    },
    Attach {
        target: TerminalTarget,
        subscription: String,
    },
    Ack {
        subscription: String,
        sequence: String,
    },
    Detach {
        subscription: String,
    },
    Stop {
        target: TerminalTarget,
    },
    Input {
        target: TerminalTarget,
        bytes: Vec<u8>,
    },
    Resize {
        target: TerminalTarget,
        rows: u16,
        cols: u16,
    },
    Continue {
        id: String,
    },
    Lifetime {
        id: String,
        policy: TerminalLifetime,
    },
    ConfirmHost {
        id: String,
        accept: bool,
    },
    Password {
        id: String,
        password: String,
        remember: bool,
    },
    CancelPrompt {
        id: String,
    },
    ReplaceHost {
        #[serde(rename = "targetId")]
        target_id: String,
        previous: Option<TerminalHostKey>,
        replacement: TerminalHostKey,
    },
    Browse {
        id: String,
        path: Option<String>,
    },
    Mkdir {
        id: String,
        path: String,
    },
    Rename {
        id: String,
        source: String,
        destination: String,
    },
    Upload {
        folder: bool,
        id: String,
        directory: String,
        conflict: TerminalConflict,
    },
    Download {
        id: String,
        paths: Vec<String>,
        conflict: TerminalConflict,
    },
    DeleteFiles {
        id: String,
        paths: Vec<String>,
    },
    StopFiles {
        id: String,
    },
    Options {
        options: TerminalOptionsView,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_requests_use_nested_data_and_camel_case_ids() {
        let request: crate::ipc::contracts::Request = serde_json::from_value(serde_json::json!({
            "type": "terminal", "data": { "owner": "view", "action": {
                "type": "replaceHost", "data": { "targetId": "target", "previous": null,
                    "replacement": {"algorithm": "ssh-ed25519", "bytes": [1,2,3]} }
            }}
        }))
        .unwrap();
        match request {
            crate::ipc::contracts::Request::Terminal {
                owner,
                action: TerminalAction::ReplaceHost { target_id, .. },
            } => {
                assert_eq!(owner, "view");
                assert_eq!(target_id, "target");
            }
            _ => panic!("wrong request variant"),
        }
    }
}
