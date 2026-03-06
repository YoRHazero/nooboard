# nooboard-app Stage4 API 参考文档

本文件描述 `crates/nooboard-app` 当前可供调用方使用的接口、数据结构、典型用法、内部 actor/worker 以及关键调用链。

适用对象：
- 集成 `nooboard-app` 的 CLI/Desktop/服务端调用方
- 需要实现剪贴板后端（`ClipboardPort`）的宿主应用

---

## 1. 外部入口

### 1.1 crate 导出
调用方通常直接从 `nooboard_app` 引入：
- 服务入口：`AppService`、`AppServiceImpl`
- 错误与返回：`AppError`、`AppResult<T>`
- 配置：`AppConfig`、`APP_CONFIG_VERSION`、`DEFAULT_RECENT_EVENT_LOOKUP_LIMIT`
- 剪贴板接口：`ClipboardPort`、`LocalClipboardObserved`、`LocalClipboardSubscription`
- 业务类型：`EventId`、`NoobId`、`Targets`、`IngestTextRequest`、`TextSource`、`RebroadcastEventRequest`
- 历史与快照：`HistoryRecord`、`HistoryPage`、`ListHistoryRequest`、`AppServiceSnapshot`
- 事件订阅：`AppEvent`、`SyncEvent`、`EventSubscription`、`EventSubscriptionItem`、`SubscriptionLifecycle`
- 文件传输：`SendFileRequest`、`FileDecisionRequest`、`TransferUpdate`
- 配置补丁：`AppPatch`、`NetworkPatch`、`StoragePatch`

### 1.2 服务初始化
```rust
pub fn AppServiceImpl::new(
    config_path: impl AsRef<std::path::Path>,
) -> AppResult<AppServiceImpl>
```

初始化阶段会：
- 加载并校验 `AppConfig`
- 初始化 StorageRuntime
- 初始化默认平台 ClipboardRuntime（内部创建）
- 初始化 SyncRuntime
- 启动 control actor

补充：`AppServiceImpl::new_with_clipboard(...)` 仍可用于测试或特殊嵌入场景，但常规调用方应使用 `new(config_path)`。

---

## 2. 剪贴板后端接口

当调用方需要自定义剪贴板后端（例如测试或嵌入式场景）时，可提供 `ClipboardPort` 实现：

```rust
pub trait ClipboardPort: Send + Sync {
    fn read_text(&self) -> AppResult<Option<String>>;
    fn write_text(&self, text: &str) -> AppResult<()>;
    fn watch_changes(
        &self,
        sender: nooboard_platform::ClipboardEventSender,
        shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
        interval: std::time::Duration,
    ) -> AppResult<std::thread::JoinHandle<()>>;
}
```

说明：
- `watch_changes` 可不支持；默认实现返回错误。
- 如后端实现了 `nooboard_platform::ClipboardBackend`，可自动适配为 `ClipboardPort`。

本地剪贴板订阅类型：

```rust
pub struct LocalClipboardObserved {
    pub event_id: EventId,
    pub text: String,
    pub observed_at_ms: i64,
}

pub struct LocalClipboardSubscription {
    pub async fn recv(&mut self) -> Result<LocalClipboardObserved, tokio::sync::broadcast::error::RecvError>;
    pub fn try_recv(&mut self) -> Result<LocalClipboardObserved, tokio::sync::broadcast::error::TryRecvError>;
}
```

---

## 3. AppService 外部接口

```rust
#[allow(async_fn_in_trait)]
pub trait AppService {
    async fn shutdown(&self) -> AppResult<()>;
    async fn set_sync_desired_state(&self, desired_state: SyncDesiredState) -> AppResult<AppServiceSnapshot>;
    async fn apply_config_patch(&self, patch: AppPatch) -> AppResult<AppServiceSnapshot>;
    async fn snapshot(&self) -> AppResult<AppServiceSnapshot>;

    async fn ingest_text_event(&self, request: IngestTextRequest) -> AppResult<()>;
    async fn write_event_to_clipboard(&self, event_id: EventId) -> AppResult<()>;
    async fn list_history(&self, request: ListHistoryRequest) -> AppResult<HistoryPage>;
    async fn rebroadcast_event(&self, request: RebroadcastEventRequest) -> AppResult<()>;
    async fn set_local_watch_enabled(&self, enabled: bool) -> AppResult<()>;

    async fn send_file(&self, request: SendFileRequest) -> AppResult<()>;
    async fn respond_file_decision(&self, request: FileDecisionRequest) -> AppResult<()>;

    async fn subscribe_events(&self) -> AppResult<EventSubscription>;
    async fn subscribe_local_clipboard(&self) -> AppResult<LocalClipboardSubscription>;
}
```

### 3.1 文本相关接口
- `ingest_text_event`：统一文本入口，负责入库；成功入库后发 `AppEvent::TextIngested`。
- `write_event_to_clipboard`：按 `event_id` 从存储取文本并写入系统剪贴板。
- `rebroadcast_event`：按 `event_id` 从存储取文本并发送到 sync 网络目标。
- `set_local_watch_enabled`：开启/关闭本地剪贴板 watch 流。

### 3.2 历史与状态接口
- `list_history`：按分页参数返回历史记录。
- `snapshot`：返回当前快照，含 `local_noob_id`、连接状态、网络开关、存储配置视图。

### 3.3 事件订阅接口
- `subscribe_events`：订阅统一事件流（sync/transfer/lifecycle + TextIngested）。
- `subscribe_local_clipboard`：订阅本地剪贴板观测流（文本事件）。

### 3.4 文件相关接口
- `send_file`：向目标节点发送文件。
- `respond_file_decision`：对远端文件请求进行接受/拒绝决策。

---

## 4. 主要数据结构

## 4.1 标识与目标

```rust
pub struct EventId; // UUID v7 封装
impl EventId {
    pub fn new() -> Self;
    pub fn as_uuid(self) -> uuid::Uuid;
}
impl TryFrom<&str> for EventId;

pub struct NoobId(String);
impl NoobId {
    pub fn new(value: impl Into<String>) -> Self;
    pub fn as_str(&self) -> &str;
}

pub enum Targets {
    All,
    Nodes(Vec<NoobId>),
}
impl Targets {
    pub fn all() -> Self;
    pub fn nodes(nodes: Vec<NoobId>) -> Self;
}
```

## 4.2 文本 ingest DTO

```rust
pub struct IngestTextRequest {
    pub event_id: EventId,
    pub content: String,
    pub origin_noob_id: NoobId,
    pub origin_device_id: String,
    pub source: TextSource,
}

pub enum TextSource {
    LocalWatch,
    LocalManual,
    RemoteSync,
}

pub struct RebroadcastEventRequest {
    pub event_id: EventId,
    pub targets: Targets,
}
```

## 4.3 历史 DTO

```rust
pub struct HistoryRecord {
    pub event_id: EventId,
    pub origin_noob_id: String,
    pub origin_device_id: String,
    pub created_at_ms: i64,
    pub applied_at_ms: i64,
    pub content: String,
}

pub struct HistoryCursor {
    pub created_at_ms: i64,
    pub event_id: EventId,
}

pub struct HistoryPage {
    pub records: Vec<HistoryRecord>,
    pub next_cursor: Option<HistoryCursor>,
}

pub struct ListHistoryRequest {
    pub limit: usize,
    pub cursor: Option<HistoryCursor>,
}
```

## 4.4 快照与配置补丁 DTO

```rust
pub enum NetworkPatch {
    SetMdnsEnabled(bool),
    SetNetworkEnabled(bool),
    AddManualPeer(std::net::SocketAddr),
    RemoveManualPeer(std::net::SocketAddr),
}

pub struct StoragePatch {
    pub db_root: Option<std::path::PathBuf>,
    pub retain_old_versions: Option<usize>,
    pub history_window_days: Option<u32>,
    pub dedup_window_days: Option<u32>,
    pub gc_every_inserts: Option<u32>,
    pub gc_batch_size: Option<u32>,
}

pub enum AppPatch {
    Network(NetworkPatch),
    Storage(StoragePatch),
}

pub enum AppSyncStatus {
    Disabled,
    Starting,
    Running,
    Stopped,
    Error(String),
}

pub struct AppServiceSnapshot {
    pub local_noob_id: NoobId,
    pub desired_state: SyncDesiredState,
    pub actual_sync_status: AppSyncStatus,
    pub connected_peers: Vec<ConnectedPeer>,
    pub network_enabled: bool,
    pub mdns_enabled: bool,
    pub manual_peers: Vec<std::net::SocketAddr>,
    pub storage: StorageConfigView,
}
```

## 4.5 事件订阅 DTO

```rust
pub enum SyncEvent {
    TextReceived { event_id: EventId, content: String, noob_id: NoobId, device_id: String },
    FileDecisionRequired { peer_noob_id: NoobId, transfer_id: u32, file_name: String, file_size: u64, total_chunks: u32 },
    ConnectionError { peer_noob_id: Option<NoobId>, addr: Option<std::net::SocketAddr>, error: String },
}

pub enum AppEvent {
    Sync(SyncEvent),
    Transfer(TransferUpdate),
    TextIngested {
        event_id: EventId,
        origin_noob_id: NoobId,
        origin_device_id: String,
        source: TextSource,
        created_at_ms: i64,
    },
}

pub enum EventSubscriptionItem {
    Lifecycle(SubscriptionLifecycle),
    Event { session_id: u64, event: AppEvent },
}
```

`EventSubscription` 特性：
- 第一次 `recv()/try_recv()` 会先返回 `Lifecycle::Opened { session_id }`。

## 4.6 文件传输 DTO

```rust
pub struct SendFileRequest {
    pub path: std::path::PathBuf,
    pub targets: Targets,
}

pub struct FileDecisionRequest {
    pub peer_noob_id: NoobId,
    pub transfer_id: u32,
    pub accept: bool,
    pub reason: Option<String>,
}

pub struct TransferUpdate {
    pub transfer_id: u32,
    pub peer_noob_id: NoobId,
    pub direction: TransferDirection,
    pub state: TransferState,
}
```

---

## 5. 配置 API（AppConfig）

调用方可直接操作 `AppConfig`：

```rust
impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> AppResult<Self>;
    pub fn save_atomically(&self, path: impl AsRef<Path>) -> AppResult<()>;
    pub fn regenerate_noob_id(config_path: impl AsRef<Path>) -> AppResult<String>;
    pub fn validate(&self) -> AppResult<()>;

    pub fn to_storage_config(&self) -> nooboard_storage::AppConfig;
    pub fn to_sync_config(&self) -> AppResult<nooboard_sync::SyncConfig>;

    pub fn recent_event_lookup_limit(&self) -> usize;
    pub fn noob_id(&self) -> Option<&str>;
}
```

配置结构主干（调用方常用字段）：

```rust
pub struct AppConfig {
    pub meta: MetaConfig,             // 配置版本/profile
    pub identity: IdentityConfig,     // noob_id_file/device_id
    pub app: AppSection,              // 应用级参数（clipboard 等）
    pub storage: StorageSection,      // 存储参数
    pub sync: SyncSection,            // 网络/认证/文件/传输参数
}

pub struct IdentityConfig {
    pub noob_id_file: PathBuf,
    pub device_id: String,
}

pub struct AppSection {
    pub clipboard: ClipboardAppConfig,
}

pub struct ClipboardAppConfig {
    pub recent_event_lookup_limit: usize,
}

pub struct StorageSection {
    pub db_root: PathBuf,
    pub retain_old_versions: usize,
    pub lifecycle: StorageLifecycleConfig,
}

pub struct SyncSection {
    pub network: SyncNetworkConfig,
    pub auth: SyncAuthConfig,
    pub file: SyncFileConfig,
    pub transport: SyncTransportConfig,
}
```

行为要点：
- `load` 会把相对路径解析为相对配置文件目录的绝对路径。
- `load` 会自动读取/初始化 `identity.noob_id_file`，并加载 `noob_id`。
- `save_atomically` 采用临时文件 + rename 原子替换。
- `APP_CONFIG_VERSION = 2`，默认 `DEFAULT_RECENT_EVENT_LOOKUP_LIMIT = 50`。

---

## 6. 典型用例

### 6.1 初始化并启动同步
```rust
let service = AppServiceImpl::new("/path/to/app.toml")?;
service.set_sync_desired_state(SyncDesiredState::Running).await?;
```

### 6.2 手动 ingest 一条本地文本
```rust
let snapshot = service.snapshot().await?;
let req = IngestTextRequest {
    event_id: EventId::new(),
    content: "hello".to_string(),
    origin_noob_id: snapshot.local_noob_id,
    origin_device_id: "desktop-a".to_string(),
    source: TextSource::LocalManual,
};
service.ingest_text_event(req).await?;
```

### 6.3 开启本地剪贴板 watch
```rust
service.set_local_watch_enabled(true).await?;
let mut local_rx = service.subscribe_local_clipboard().await?;
let observed = local_rx.recv().await?;
```

### 6.4 根据 event_id 写回剪贴板
```rust
service.write_event_to_clipboard(event_id).await?;
```

### 6.5 根据 event_id 重广播
```rust
service.rebroadcast_event(RebroadcastEventRequest {
    event_id,
    targets: Targets::all(),
}).await?;
```

### 6.6 分页拉取历史
```rust
let page1 = service.list_history(ListHistoryRequest { limit: 50, cursor: None }).await?;
let page2 = service.list_history(ListHistoryRequest {
    limit: 50,
    cursor: page1.next_cursor,
}).await?;
```

### 6.7 文件发送与决策
```rust
service.send_file(SendFileRequest {
    path: "/tmp/demo.bin".into(),
    targets: Targets::all(),
}).await?;

service.respond_file_decision(FileDecisionRequest {
    peer_noob_id: NoobId::new("peer-1"),
    transfer_id: 1,
    accept: true,
    reason: None,
}).await?;
```

---

## 7. 内部 actors / workers

`nooboard-app` 内部主要并发单元如下：

1. Control Actor（Tokio 任务）
- 入口：`spawn_control_actor`
- 通道：`mpsc<ControlCommand>`（容量 256）
- 作用：串行处理所有 AppService 请求；协调 storage/sync/clipboard。

2. Local Watch Bridge Task（Tokio 任务）
- 由 `set_local_watch_enabled(true)` 创建。
- 消费 `LocalClipboardSubscription`，转发为 `InternalLocalClipboardObserved`。

3. Sync Ingest Bridge Task（Tokio 任务）
- 在 sync engine 可用时创建。
- 消费 sync runtime 的 `SyncEvent`，转发为 `InternalSyncEvent`。

4. Clipboard Watch Worker（平台线程）
- 由 `ClipboardRuntime::start_watch` 调用后端 `watch_changes` 创建。
- 负责监听系统剪贴板变化。

5. Clipboard Forward Task（Tokio 任务）
- 把平台剪贴板事件转为 `LocalClipboardObserved` 广播。
- 包含 suppression 过滤（text fingerprint + event_id + TTL，当前 TTL=3s）。

6. Storage Actor（独立线程）
- 入口：`storage_runtime::actor::run_actor`
- 通道：`std::sync::mpsc<StorageCommand>`
- 作用：串行执行 `append_text/list_history/get_event_by_id/reconfigure`。

7. Sync Runtime Bridges（Tokio 任务）
- `spawn_event_bridge`：sync 引擎事件 `mpsc` -> `broadcast`
- `spawn_transfer_bridge`：transfer `broadcast` -> app 内 `broadcast`

8. SubscriptionHub Session Bridge（Tokio 任务）
- 在 `subscriptions.activate()` 时创建会话。
- 汇聚 sync/transfer/status，并产生 lifecycle 事件（Opened/Rebinding/Lagged/Fatal/Closed）。

---

## 8. 关键调用链

### 8.1 文本统一入口（手动/本地/远端）
1. 外部或内部构造 `IngestTextRequest`
2. `AppService::ingest_text_event`
3. control actor -> `clipboard_history::ingest_text_event`
4. `StorageRuntime::append_text`
5. 入库成功时发布 `AppEvent::TextIngested`

### 8.2 本地剪贴板 watch 链
1. `set_local_watch_enabled(true)`
2. `ClipboardRuntime::start_watch` 启动平台监听
3. 平台事件 -> forward task -> suppression 过滤
4. 广播 `LocalClipboardObserved`
5. local watch bridge 收到后 -> `InternalLocalClipboardObserved`
6. control actor 转换为 `IngestTextRequest { source: LocalWatch }`
7. 调用 `ingest_text_event` 入库

### 8.3 远端文本链
1. sync runtime 产生 `SyncEvent::TextReceived`
2. sync ingest bridge -> `InternalSyncEvent`
3. control actor 解析 `event_id/noob_id/device_id`
4. 构建 `IngestTextRequest { source: RemoteSync }`
5. 调用 `ingest_text_event` 入库

### 8.4 event_id 写回剪贴板链
1. `write_event_to_clipboard(event_id)`
2. control actor -> `load_record_by_event_id`
3. `StorageRuntime::get_event_by_id`
4. `ClipboardRuntime::write_text_with_event`
5. 先登记 suppression，再执行后端系统写入

### 8.5 event_id 重广播链
1. `rebroadcast_event(event_id, targets)`
2. control actor -> `load_record_by_event_id`
3. `StorageRuntime::get_event_by_id`
4. 构造 `SendTextRequest { event_id, content, targets }`
5. `sync_runtime.text_sender().try_send(...)`

### 8.6 事件订阅链
1. `subscribe_events()` 返回 `EventSubscription`
2. 首次 `recv/try_recv` 返回 `Lifecycle::Opened`
3. 后续接收：
- sync 映射事件 `AppEvent::Sync(...)`
- transfer 映射事件 `AppEvent::Transfer(...)`
- app 内发布事件 `AppEvent::TextIngested(...)`
- lifecycle（Lagged/Rebinding/Fatal/Closed）

### 8.7 配置补丁链
1. `apply_config_patch(AppPatch)`
2. 更新内存配置并校验
3. 原子落盘配置文件
4. 重配 storage runtime
5. 必要时重启/重载 sync runtime
6. 任一步失败时触发回滚（storage + config + sync）

---

## 9. 错误语义（调用方常见）

- `AppError::EngineNotRunning`：需要运行中的 sync 引擎但当前未运行。
- `AppError::SyncDisabled`：网络开关已关闭，发送/决策/重广播等被拒绝。
- `AppError::EventNotFound`：按 `event_id` 未查询到历史文本记录。
- `AppError::InvalidEventId`：事件 ID 字符串不是合法 UUID。
- `AppError::ChannelClosed`：内部 actor/runtime 通道关闭或阻塞异常。
- `AppError::ConfigRollbackFailed`：配置更新失败且回滚失败。
