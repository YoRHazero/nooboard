# nooboard-app Stage4 API 参考文档

本文件描述 `crates/nooboard-app` 当前对 `desktop` 暴露的接入面。

目标读者：
- `nooboard-desktop` 接入方
- 需要在测试或宿主环境中嵌入 `nooboard-app` 的调用方

本文件只描述当前状态，不描述演进过程。

---

## 1. 定位

`nooboard-app` 是 desktop 的后端 service。

当前 contract 的核心原则：
- `AppState` 是当前状态的唯一权威读模型。
- `AppEvent` 只表达边沿事件，不承载完整状态。
- `subscribe_state()` 和 `subscribe_events()` 都是 app-lifetime 订阅，不依赖 sync session 生命周期。
- clipboard 对外只暴露已提交 record，不暴露 raw local clipboard watch。
- peers 只承诺当前 connected peers。
- transfers 对外只暴露 `incoming_pending / active / recent_completed` 三段读模型。

当前不对 desktop 暴露：
- raw local clipboard 文本流
- discovered/offline peer directory
- session id / rebinding / opened / closed 一类桥接概念

---

## 2. 入口

调用方通常直接从 `nooboard_app` crate root 引入：
- `DesktopAppService`
- `DesktopAppServiceImpl`
- `AppState`
- `AppEvent`
- `SettingsPatch`
- clipboard / transfer / identity 相关 DTO
- `AppError`

默认构造入口：

```rust
pub fn DesktopAppServiceImpl::new(
    config_path: impl AsRef<std::path::Path>,
) -> AppResult<Self>
```

说明：
- 该入口会加载并校验 `AppConfig`
- 会创建 storage runtime、clipboard runtime、sync runtime 和 control actor
- 默认平台 clipboard backend 目前依赖宿主平台实现；在非 macOS 环境下，默认构造可能返回 `AppError::Platform`
- 测试或特殊嵌入场景可使用 `new_with_clipboard(...)` 注入自定义 `ClipboardPort`

---

## 3. 服务接口

当前 desktop-facing service trait 为：

```rust
#[allow(async_fn_in_trait)]
pub trait DesktopAppService {
    async fn shutdown(&self) -> AppResult<()>;

    async fn get_state(&self) -> AppResult<AppState>;
    async fn subscribe_state(&self) -> AppResult<StateSubscription>;
    async fn subscribe_events(&self) -> AppResult<EventSubscription>;

    async fn set_sync_desired_state(&self, desired: SyncDesiredState) -> AppResult<()>;
    async fn patch_settings(&self, patch: SettingsPatch) -> AppResult<()>;

    async fn submit_text(&self, request: SubmitTextRequest) -> AppResult<EventId>;
    async fn get_clipboard_record(&self, event_id: EventId) -> AppResult<ClipboardRecord>;
    async fn list_clipboard_history(
        &self,
        request: ListClipboardHistoryRequest,
    ) -> AppResult<ClipboardHistoryPage>;
    async fn adopt_clipboard_record(&self, event_id: EventId) -> AppResult<()>;
    async fn rebroadcast_clipboard_record(
        &self,
        request: RebroadcastClipboardRequest,
    ) -> AppResult<()>;

    async fn send_files(&self, request: SendFilesRequest) -> AppResult<Vec<TransferId>>;
    async fn decide_incoming_transfer(&self, request: IncomingTransferDecision) -> AppResult<()>;
    async fn cancel_transfer(&self, transfer_id: TransferId) -> AppResult<()>;
}
```

总体语义：
- 读接口不产生副作用
- 写接口成功返回时，业务侧状态已经提交，后续 `get_state()` 和 `subscribe_state()` 可观察到结果
- `shutdown()` 后 service 不应继续被复用

---

## 4. 状态订阅与事件订阅

### 4.1 `StateSubscription`

```rust
pub struct StateSubscription {
    pub async fn recv(&mut self) -> Result<AppState, StateRecvError>;
    pub fn latest(&self) -> &AppState;
}
```

语义：
- 基于 `tokio::sync::watch`
- `latest()` 在订阅建立后立刻可用
- `recv()` 等待下一次状态变更
- 是最新值覆盖模型，不提供历史 replay
- engine start / stop / restart、settings patch、clipboard commit、transfer 状态推进、shutdown 前最终状态，都会经过这条流

### 4.2 `EventSubscription`

```rust
pub struct EventSubscription {
    pub async fn recv(&mut self) -> Result<AppEvent, EventRecvError>;
}
```

语义：
- 基于 `tokio::sync::broadcast`
- 不提供 replay
- 用于 toast、提示、瞬时反馈
- desktop 丢事件后，应回到 `AppState` 自愈

---

## 5. 权威状态模型

```rust
pub struct AppState {
    pub revision: u64,
    pub identity: LocalIdentity,
    pub local_connection: LocalConnectionInfo,
    pub sync: SyncState,
    pub peers: PeersState,
    pub clipboard: ClipboardState,
    pub transfers: TransfersState,
    pub settings: SettingsState,
}
```

### 5.1 `revision`

- 每次状态真正发生变化时单调递增
- 如果一次命令没有改变状态内容，不会强行推进 revision

### 5.2 `identity`

```rust
pub struct LocalIdentity {
    pub noob_id: NoobId,
    pub device_id: String,
}
```

- `noob_id` 来自配置中的 `identity.noob_id_file`
- `device_id` 来自配置中的 `identity.device_id`

### 5.3 `local_connection`

```rust
pub struct LocalConnectionInfo {
    pub device_endpoint: Option<std::net::SocketAddr>,
}
```

说明：
- `device_endpoint` 是 app 当前推荐 desktop 展示给其它设备的连接地址
- 它是只读运行时信息，不属于 `SettingsPatch`
- 当监听 host 是 `0.0.0.0` 这类 unspecified 地址时，app 会自动枚举本机 IPv4，并拼出可分享的 `ip:port`
- 选择规则固定为：
  - 首个非 loopback 的私有 IPv4
  - 否则首个非 loopback IPv4
  - 否则 loopback IPv4
  - 否则 `None`
- `device_endpoint.port` 始终使用当前生效 `listen_port`

### 5.4 `sync`

```rust
pub struct SyncState {
    pub desired: SyncDesiredState,
    pub actual: SyncActualStatus,
}

pub enum SyncDesiredState {
    Running,
    Stopped,
}

pub enum SyncActualStatus {
    Disabled,
    Starting,
    Running,
    Stopped,
    Error(String),
}
```

说明：
- `desired` 是 app 想要的目标状态
- `actual` 是 runtime 当前真实状态
- 当 `network_enabled=false` 时，`desired` 会被收敛为 `Stopped`
- 当 `network_enabled=false` 时，`actual` 会进入 `Disabled`
- 当 `network_enabled=false` 时，`set_sync_desired_state(Running)` 返回 `AppError::SyncDisabled`
- 重新开启 network setting 不会自动恢复到 `Running`；desktop 需要再次显式调用 `set_sync_desired_state(Running)`

### 5.5 `peers`

```rust
pub struct PeersState {
    pub connected: Vec<ConnectedPeer>,
}

pub struct ConnectedPeer {
    pub noob_id: NoobId,
    pub device_id: String,
    pub addresses: Vec<std::net::SocketAddr>,
    pub transport: PeerTransport,
    pub latency_ms: Option<u32>,
}

pub enum PeerTransport {
    Mdns,
    Manual,
    Mixed,
    Unknown,
}
```

说明：
- 这里只包含当前已连接 peers
- 不包含 discovered/offline peers
- `device_id` 是 peer 当前上报的人类可读设备标签，对应对端当前生效的 `identity.device_id`
- `device_id` 不保证唯一；desktop 可以对重复标签做高亮或告警，但所有业务逻辑仍必须以 `noob_id` 为准
- `transport` 是基于当前配置对连接来源的解释，不是独立发现目录

### 5.6 `clipboard`

```rust
pub struct ClipboardState {
    pub latest_committed_event_id: Option<EventId>,
}
```

说明：
- 只暴露最近一次成功提交到存储的 clipboard record
- 不暴露 raw local clipboard 文本
- 不暴露未提交中间态

### 5.7 `transfers`

```rust
pub struct TransfersState {
    pub incoming_pending: Vec<IncomingTransfer>,
    pub active: Vec<Transfer>,
    pub recent_completed: Vec<CompletedTransfer>,
}

pub struct IncomingTransfer {
    pub transfer_id: TransferId,
    pub peer_noob_id: NoobId,
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub total_chunks: u32,
    pub offered_at_ms: i64,
}

pub struct Transfer {
    pub transfer_id: TransferId,
    pub direction: TransferDirection,
    pub peer_noob_id: NoobId,
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub transferred_bytes: u64,
    pub state: TransferState,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
}
```

说明：
- `incoming_pending`：等待 accept/reject 的入站传输
- `active`：当前进行中的上传/下载
- `recent_completed`：最近完成的终态记录
- `peer_device_id` 是对端当前上报的人类可读设备标签；它不保证唯一，desktop 只能用于显示，不能代替 `peer_noob_id`
- 当前实现的 completed 缓冲上限为 64 条
- transfer 状态目前不跨重启持久化

### 5.8 `settings`

```rust
pub struct SettingsState {
    pub identity: IdentitySettings,
    pub network: NetworkSettings,
    pub storage: StorageSettings,
    pub clipboard: ClipboardSettings,
    pub transfers: TransferSettings,
}

pub struct IdentitySettings {
    pub device_id: String,
}

pub struct NetworkSettings {
    pub listen_port: u16,
    pub network_enabled: bool,
    pub mdns_enabled: bool,
    pub manual_peers: Vec<std::net::SocketAddr>,
}
```

这些值都是当前生效值，不是 desktop draft。
- `identity.device_id` 是当前生效的人类可读设备标签
- `network.listen_port` 是当前生效的监听端口；desktop 只开放端口编辑，不开放 host 编辑
- `device_id` 继续属于 identity，不并入 network settings

---

## 6. 标识类型

```rust
pub struct EventId(Uuid);
pub struct NoobId(String);
pub struct TransferId {
    peer_noob_id: NoobId,
    raw_id: u32,
}
```

说明：
- `EventId::new()` 当前使用 UUID v7
- `TransferId` 是 `(peer_noob_id, raw_id)` 的组合标识
- `TransferId::to_string()` 的格式是 `{peer_noob_id}:{raw_id}`

---

## 7. 事件模型

```rust
pub enum AppEvent {
    ClipboardCommitted {
        event_id: EventId,
        source: ClipboardRecordSource,
    },
    IncomingTransferOffered {
        transfer_id: TransferId,
    },
    TransferUpdated {
        transfer_id: TransferId,
    },
    TransferCompleted {
        transfer_id: TransferId,
        outcome: TransferOutcome,
    },
    PeerConnectionError {
        peer_noob_id: Option<NoobId>,
        addr: Option<std::net::SocketAddr>,
        error: String,
    },
}
```

语义：
- `ClipboardCommitted`：record 已成功入库，并且 `AppState.clipboard.latest_committed_event_id` 已经更新
- `IncomingTransferOffered`：新的 pending 入站传输出现
- `TransferUpdated`：active transfer 的状态或进度推进
- `TransferCompleted`：transfer 已进入终态，并且已经体现在 `AppState.transfers.recent_completed`
- `PeerConnectionError`：连接错误边沿事件；真实 connected peers 仍以 `AppState.peers.connected` 为准
- `AppEvent` 不承载 desktop-local warning/error；宿主自己的桥接失败、查询失败、UI 本地诊断应进入 desktop 本地 activity/diagnostic 层，而不是反向塞进 app contract

---

## 8. Clipboard contract

### 8.1 类型

```rust
pub struct SubmitTextRequest {
    pub content: String,
}

pub enum ClipboardRecordSource {
    LocalCapture,
    RemoteSync,
    UserSubmit,
}

pub struct ClipboardRecord {
    pub event_id: EventId,
    pub source: ClipboardRecordSource,
    pub origin_noob_id: NoobId,
    pub origin_device_id: String,
    pub created_at_ms: i64,
    pub applied_at_ms: i64,
    pub content: String,
}

pub struct ListClipboardHistoryRequest {
    pub limit: usize,
    pub cursor: Option<ClipboardHistoryCursor>,
}

pub struct ClipboardHistoryPage {
    pub records: Vec<ClipboardRecord>,
    pub next_cursor: Option<ClipboardHistoryCursor>,
}

pub enum ClipboardBroadcastTargets {
    AllConnected,
    Nodes(Vec<NoobId>),
}

pub struct RebroadcastClipboardRequest {
    pub event_id: EventId,
    pub targets: ClipboardBroadcastTargets,
}
```

### 8.2 `submit_text()`

- 创建新的 `EventId`
- 以 `ClipboardRecordSource::UserSubmit` 提交到存储
- 成功提交后更新 `AppState.clipboard.latest_committed_event_id`
- 然后发出 `AppEvent::ClipboardCommitted`

失败语义：
- 文本超过 `settings.storage.max_text_bytes` 时返回 `AppError::TextTooLarge`

### 8.3 `get_clipboard_record()` / `list_clipboard_history()`

- 都只读取已提交 record
- 历史分页当前按 `created_at_ms DESC, event_id DESC` 返回，最新优先
- history 数据是持久化的，重启后仍可读取

### 8.4 `adopt_clipboard_record()`

- 从存储读取 record
- 将内容写回本机系统剪贴板
- 不会新建 record
- 不会触发新的 committed 事件

### 8.5 `rebroadcast_clipboard_record()`

- 只能广播已提交 record
- `AllConnected` 以当前 `AppState.peers.connected` 为准
- `Nodes(Vec<NoobId>)` 要求每个目标当前都已连接
- sender 本地不会因为 rebroadcast 新建 record
- receiver 收到后会以 `ClipboardRecordSource::RemoteSync` 入库

常见错误：
- `AppError::EventNotFound`
- `AppError::PeerNotConnected`
- `AppError::SyncDisabled`
- `AppError::EngineNotRunning`

### 8.6 本地 clipboard 捕获

本地 clipboard watch 是内部实现，不是 public API。

desktop 只能通过：
- `SettingsPatch::Clipboard(SetLocalCaptureEnabled(...))`
- `AppEvent::ClipboardCommitted`
- `AppState.clipboard.latest_committed_event_id`
- `get_clipboard_record()` / `list_clipboard_history()`

来观察结果。

---

## 9. Transfer contract

### 9.1 类型

```rust
pub struct SendFileItem {
    pub path: std::path::PathBuf,
}

pub struct SendFilesRequest {
    pub targets: Vec<NoobId>,
    pub files: Vec<SendFileItem>,
}

pub enum IncomingTransferDisposition {
    Accept,
    Reject,
}

pub struct IncomingTransferDecision {
    pub transfer_id: TransferId,
    pub decision: IncomingTransferDisposition,
}

pub enum TransferDirection {
    Upload,
    Download,
}

pub enum TransferState {
    Queued,
    Starting,
    InProgress,
    Cancelling,
}

pub enum TransferOutcome {
    Succeeded,
    Rejected,
    Cancelled,
    Failed,
}
```

### 9.2 `send_files()`

- 只接受当前 connected peers 作为目标
- 会去重重复目标
- 成功返回时，返回值中的 `TransferId` 已经与 `AppState.transfers.active` 中的 active transfer 一一对应
- 返回的 `TransferId` 是 authoritative transfer id

常见错误：
- `AppError::PeerNotConnected`
- `AppError::SyncDisabled`
- `AppError::EngineNotRunning`
- `AppError::Io`

### 9.3 `decide_incoming_transfer()`

- 只能作用于 `incoming_pending` 中的 transfer
- `Accept` 后该 transfer 会离开 pending，并进入 active 或后续 completed
- `Reject` 后该 transfer 会离开 pending，并进入 completed，`outcome=Rejected`

常见错误：
- `AppError::TransferNotFound`
- `AppError::SyncDisabled`
- `AppError::EngineNotRunning`

### 9.4 `cancel_transfer()`

- 只对当前可取消的 active transfer 有效
- 成功后 transfer 会离开 active，并进入 completed，`outcome=Cancelled`
- 已完成 transfer 和 pending incoming transfer 不能通过该接口取消

常见错误：
- `AppError::TransferNotFound`
- `AppError::TransferNotCancelable`
- `AppError::EngineNotRunning`

### 9.5 `CompletedTransfer`

```rust
pub struct CompletedTransfer {
    pub transfer_id: TransferId,
    pub direction: TransferDirection,
    pub peer_noob_id: NoobId,
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub outcome: TransferOutcome,
    pub started_at_ms: Option<i64>,
    pub finished_at_ms: i64,
    pub saved_path: Option<std::path::PathBuf>,
    pub message: Option<String>,
}
```

说明：
- 下载成功时，`saved_path` 通常有值
- 上传成功时，`saved_path` 通常为 `None`
- 失败、拒绝、取消时，`message` 可能携带原因

---

## 10. Settings contract

### 10.1 当前 settings 读模型

```rust
pub struct NetworkSettings {
    pub network_enabled: bool,
    pub mdns_enabled: bool,
    pub manual_peers: Vec<std::net::SocketAddr>,
}

pub struct StorageSettings {
    pub db_root: std::path::PathBuf,
    pub history_window_days: u32,
    pub dedup_window_days: u32,
    pub max_text_bytes: usize,
    pub gc_batch_size: usize,
}

pub struct ClipboardSettings {
    pub local_capture_enabled: bool,
}

pub struct TransferSettings {
    pub download_dir: std::path::PathBuf,
}
```

### 10.2 当前 settings patch

```rust
pub enum SettingsPatch {
    Identity(IdentitySettingsPatch),
    Network(NetworkSettingsPatch),
    Storage(StorageSettingsPatch),
    Clipboard(ClipboardSettingsPatch),
    Transfers(TransferSettingsPatch),
}

pub enum IdentitySettingsPatch {
    SetDeviceId(String),
}

pub enum NetworkSettingsPatch {
    SetListenPort(u16),
    SetNetworkEnabled(bool),
    SetMdnsEnabled(bool),
    SetManualPeers(Vec<std::net::SocketAddr>),
}

pub struct StorageSettingsPatch {
    pub db_root: Option<std::path::PathBuf>,
    pub history_window_days: Option<u32>,
    pub dedup_window_days: Option<u32>,
    pub max_text_bytes: Option<usize>,
    pub gc_batch_size: Option<usize>,
}

pub enum ClipboardSettingsPatch {
    SetLocalCaptureEnabled(bool),
}

pub enum TransferSettingsPatch {
    SetDownloadDir(std::path::PathBuf),
}
```

### 10.3 `patch_settings()` 的行为

- patch 先应用到配置对象
- 然后做 config validation
- 校验通过后，会原子写回配置文件
- 接着按需要重配 storage runtime、clipboard watch、sync runtime
- 如果运行期应用失败，会尝试回滚；回滚也失败时返回 `AppError::ConfigRollbackFailed`

持久化语义：
- settings 是真实后端 setting
- 配置改动会跨重启保留

路径语义：
- `db_root` 和 `download_dir` 如果传相对路径，会按配置文件所在目录解析成绝对路径

运行时语义：
- `SetLocalCaptureEnabled` 会立刻启动或停止本地 clipboard watch
- `StorageSettingsPatch` 会立刻重配 storage runtime；如果 `db_root` 改了，history 读取会切到新数据库
- `IdentitySettingsPatch` 在 `sync.desired=Running` 时会触发 engine reconcile/restart
- `NetworkSettingsPatch` 和 `SetDownloadDir` 在 `sync.desired=Running` 时会触发 engine reconcile/restart
- `SetListenPort` 只修改监听端口，不开放 host 编辑

### 10.4 当前主要校验约束

当前 `patch_settings()` 会继承配置校验规则，常见约束包括：
- `identity.device_id` 不能为空
- `listen_port` 必须在 `1..=65535`
- `max_text_bytes > 0`
- `history_window_days >= 1`
- `dedup_window_days >= history_window_days`
- `gc_batch_size >= 1`
- `manual_peers` 不允许重复地址
- sync 派生配置必须通过 `nooboard-sync` 自身校验

---

## 11. 错误模型

当前公开错误类型：

```rust
pub enum AppError {
    Io(std::io::Error),
    Storage(nooboard_storage::StorageError),
    Sync(nooboard_sync::SyncError),
    Platform(nooboard_platform::NooboardError),
    ConfigParse { .. },
    ConfigSerialize(..),
    InvalidConfig(String),
    EngineNotRunning,
    EngineAlreadyRunning,
    SyncDisabled,
    ChannelClosed(String),
    EventNotFound { event_id: String },
    InvalidEventId { event_id: String },
    TextTooLarge { actual_bytes: usize, max_bytes: usize },
    PeerNotConnected { peer_noob_id: String },
    TransferNotFound { transfer_id: String },
    TransferNotCancelable { transfer_id: String },
    ManualPeerExists { peer: String },
    ManualPeerNotFound { peer: String },
    ConfigRollbackFailed { restart_error: String, rollback_error: String },
}
```

desktop 集成中最常见的是：
- `InvalidConfig`
- `SyncDisabled`
  - `set_sync_desired_state(Running)` 且 network setting 禁用
  - 发送/广播等依赖 sync network 的动作发生在 network disabled 下
- `EngineNotRunning`
- `EventNotFound`
- `TextTooLarge`
- `PeerNotConnected`
- `TransferNotFound`
- `TransferNotCancelable`

说明：
- `ManualPeerExists` / `ManualPeerNotFound` 当前未作为 `patch_settings()` 的主路径错误使用，因为当前 network patch 是整表替换 `SetManualPeers`

---

## 12. 当前推荐的 desktop 接线方式

推荐的接入顺序：

```rust
let service = DesktopAppServiceImpl::new(config_path)?;

let mut state_sub = service.subscribe_state().await?;
let mut event_sub = service.subscribe_events().await?;

let initial = state_sub.latest().clone();
```

推荐原则：
- 页面主体直接渲染 `AppState`
- toast、一次性提示、瞬时反馈走 `AppEvent`
- recent activity 建议做成 desktop-local normalized feed：接收 `AppEvent`，再按需要补充 state edge 和 desktop diagnostic 项
- clipboard 页面主体读 `list_clipboard_history()` 和 `AppState.clipboard.latest_committed_event_id`
- transfer 页面主体直接读 `AppState.transfers`
- peers 页面主体直接读 `AppState.peers.connected`
- settings 页面直接读 `AppState.settings`，apply 时调用 `patch_settings()`

desktop 不应自行假设：
- 会拿到 raw local clipboard stream
- 会拿到 discovered/offline peers
- 会拿到跨重启持久化的 transfer 历史
- 事件流可以替代状态流

---

## 13. 当前公开接入面的边界

当前 `nooboard-app` 已明确提供：
- app-lifetime `subscribe_state()`
- app-lifetime `subscribe_events()`
- commit-only clipboard contract
- connected-peers only peer contract
- authoritative transfer id 与正式 cancel 能力
- 真实后端 settings：`local_capture_enabled`、`download_dir`、`db_root`、`max_text_bytes` 等

当前 desktop 应以这些公开 contract 为唯一依据接入。
