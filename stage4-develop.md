# nooboard-app Stage4 开发规格

本文件是 `crates/nooboard-app` 面向 `nooboard-desktop` 的目标开发规格。

本文件不是当前实现说明，不追求兼容既有接口，也不以任何旧文档为目标。后续开发以本文件为唯一目标契约；如果现有实现与本文件冲突，应调整实现，而不是回退规格。

---

## 1. 文档定位

本规格解决的是一个明确问题：

- `nooboard-desktop` 需要一个稳定、强约束、可长期维护的 app backend。
- 这个 backend 必须把“当前真实状态”和“刚刚发生的事件”分离。
- 这个 backend 必须把 clipboard、sync、peers、transfers、settings 收敛为统一契约。
- 这个 backend 不能继续把内部运行时细节泄漏给 desktop。

本规格的适用范围：

- `crates/nooboard-app`
- 未来 `nooboard-desktop` 对 `nooboard-app` 的接入
- app 内部的 storage / clipboard runtime / sync runtime / transfer registry / settings patch 设计

本规格的非目标：

- 不描述 GPUI 页面结构。
- 不为旧 API 保留兼容层。
- 不把 desktop 的本地 UI 偏好塞进 app。
- 不承诺 discovered/offline peers 或 mesh topology。

---

## 2. 设计目标

目标不是“小修小补”，而是直接建立 desktop 能长期依赖的正确模型。

核心目标：

- 把 `nooboard-app` 定义为 desktop 的唯一 backend service。
- 为 desktop 提供一个权威状态模型 `AppState`。
- 为 desktop 提供一个独立的边沿事件流 `AppEvent`。
- 把 clipboard 对外模型收紧到“已提交记录”，不暴露 raw local clipboard watch。
- 让 sync status、connected peers、transfers 都由稳定订阅驱动，而不是让 desktop 轮询拼装。
- 让 settings 只表达生效值和可应用 patch，不表达 desktop 的 draft/review。
- 把 transfer cancel 提升为正式能力。
- 把 download directory 提升为正式 app setting。

---

## 3. 强约束原则

### 3.1 单一真相源

- `AppState` 是 desktop 对 app 当前状态的唯一权威读模型。
- `get_state()` 和 `subscribe_state()` 暴露的内容必须同构。
- desktop 不应再拼接多个接口去推断“真实状态”。

### 3.2 状态与事件严格分离

- `state stream` 表达“现在是什么”。
- `event stream` 表达“刚刚发生了什么”。
- `event stream` 不是权威状态源。
- 丢失事件后，desktop 仍必须可以仅靠 `state stream` 自愈。

### 3.3 Clipboard 对外只暴露已提交记录

- 本地系统剪贴板 watch 是 app 内部实现。
- suppression 是 app 内部实现。
- 本地 clipboard 观测到变化后，必须先入库，再对外可见。
- desktop 不应看到未提交的 clipboard 中间态。

### 3.4 App 生命周期高于 Sync Session

- service 级订阅是 app-lifetime，不是 engine-session lifetime。
- engine 停止、重启、rebind 不应强迫 desktop 重新建立 service 级订阅。
- session/rebinding/opened/closed 这类桥接概念不应出现在 desktop-facing 契约中。

### 3.5 App 只负责事实和动作

- app 负责事实、命令、持久化、运行时协调。
- desktop 负责页面路由、选中项、筛选、排序、toast、transient feedback、draft、review。
- desktop 本地偏好不应被塞进 app patch。

### 3.6 明确拒绝“假语义”

- 不允许为适配 UI 保留虚假的 transfer sender 状态。
- 不允许把 app 内部临时桥接对象暴露成公开契约。
- 不允许用 desktop 的 mock 语义倒推 backend 语义。

---

## 4. 对外入口总览

目标对外面应收敛为：

- 一个服务 trait：`DesktopAppService`
- 一个权威状态模型：`AppState`
- 一条状态订阅：`StateSubscription`
- 一条事件订阅：`EventSubscription`
- 一组只围绕业务语义的命令方法

不再对 desktop 暴露：

- `LocalClipboardObserved`
- `LocalClipboardSubscription`
- `SubscriptionLifecycle`
- `EventSubscriptionItem`
- session id
- rebinding/opened/closed 订阅包装
- 任何 raw local clipboard stream

---

## 5. 目标服务契约

```rust
#[allow(async_fn_in_trait)]
pub trait DesktopAppService {
    async fn shutdown(&self) -> AppResult<()>;

    // Read side
    async fn get_state(&self) -> AppResult<AppState>;
    async fn subscribe_state(&self) -> AppResult<StateSubscription>;
    async fn subscribe_events(&self) -> AppResult<EventSubscription>;

    // Runtime / settings
    async fn set_sync_desired_state(
        &self,
        desired: SyncDesiredState,
    ) -> AppResult<()>;

    async fn patch_settings(
        &self,
        patch: SettingsPatch,
    ) -> AppResult<()>;

    // Clipboard
    async fn submit_text(
        &self,
        request: SubmitTextRequest,
    ) -> AppResult<EventId>;

    async fn get_clipboard_record(
        &self,
        event_id: EventId,
    ) -> AppResult<ClipboardRecord>;

    async fn list_clipboard_history(
        &self,
        request: ListClipboardHistoryRequest,
    ) -> AppResult<ClipboardHistoryPage>;

    async fn adopt_clipboard_record(
        &self,
        event_id: EventId,
    ) -> AppResult<()>;

    async fn rebroadcast_clipboard_record(
        &self,
        request: RebroadcastClipboardRequest,
    ) -> AppResult<()>;

    // Transfers
    async fn send_files(
        &self,
        request: SendFilesRequest,
    ) -> AppResult<Vec<TransferId>>;

    async fn decide_incoming_transfer(
        &self,
        request: IncomingTransferDecision,
    ) -> AppResult<()>;

    async fn cancel_transfer(
        &self,
        transfer_id: TransferId,
    ) -> AppResult<()>;
}
```

### 5.1 总体语义

- 任何读接口都不得产生副作用。
- 任何写接口成功返回时，都必须已经完成业务侧状态提交。
- “状态提交” 的含义是：内部权威状态中心已更新，后续 `get_state()` 和 `subscribe_state()` 必须能观测到更新后的结果。
- 写接口不要求在返回前等待 desktop 收到订阅消息，但必须保证订阅消息最终与 `get_state()` 一致。

### 5.2 失败语义

- `subscribe_state()` 和 `subscribe_events()` 只应在 service 已不可用时失败。
- engine 未运行不是订阅失败理由。
- peer 未连接不是 service 级订阅失败理由。
- validation error 必须在修改状态前返回。
- `event_id` / `transfer_id` / path / patch 不合法时必须返回结构化错误，而不是沉默忽略。

---

## 6. 读侧契约

### 6.1 `get_state()`

```rust
async fn get_state(&self) -> AppResult<AppState>;
```

语义：

- 返回当前完整权威状态。
- 主要用于 bootstrap、恢复、调试。
- desktop 不应把它当常规刷新手段。

### 6.2 `subscribe_state()`

```rust
pub struct StateSubscription {
    pub async fn recv(&mut self) -> Result<AppState, StateRecvError>;
    pub fn latest(&self) -> &AppState;
}
```

强约束：

- 必须是 app-lifetime 订阅。
- 建立订阅后必须立刻可见当前状态。
- 必须是“最新值覆盖”语义，适合 `watch`。
- engine restart、config patch、transfer registry 更新、clipboard commit、shutdown 前的状态变化，都必须通过这条流体现。
- desktop 不应因为 engine restart 重建这条订阅。

### 6.3 `subscribe_events()`

```rust
pub struct EventSubscription {
    pub async fn recv(&mut self) -> Result<AppEvent, EventRecvError>;
}
```

强约束：

- 必须是 app-lifetime 订阅。
- 不提供 replay，不补发历史。
- 只承载边沿事件，不承载权威状态。
- desktop 丢事件后必须仍能靠 `get_state()` 或 `subscribe_state()` 自愈。

---

## 7. 权威状态模型

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

### 7.1 `revision`

- `revision` 必须单调递增。
- 任何会改变 `AppState` 的业务变化都必须推进 `revision`。
- `revision` 是状态版本号，不是事件计数器。
- desktop 可以用它判断状态是否推进，但不应依赖步长。

### 7.2 Identity

```rust
pub struct LocalIdentity {
    pub noob_id: NoobId,
    pub device_id: String,
}
```

强约束：

- `noob_id` 和 `device_id` 是当前生效身份。
- 不存在 identity draft。
- 如果未来允许 identity 变化，变化必须通过 `AppState` 完整体现。

### 7.3 Local Connection

```rust
pub struct LocalConnectionInfo {
    pub device_endpoint: Option<std::net::SocketAddr>,
}
```

强约束：

- `device_endpoint` 是 app 当前推荐 desktop 分享给其它设备的连接地址。
- 它是只读运行时信息，不属于 `SettingsPatch`。
- 当监听 host 是 `0.0.0.0` 这类 unspecified 地址时，app 必须自动检测本机 IPv4，并拼出可分享的 `ip:port`。
- 地址选择规则固定为：
  - 首个非 loopback 的私有 IPv4。
  - 否则首个非 loopback IPv4。
  - 否则 loopback IPv4。
  - 否则 `None`。
- `device_endpoint.port` 必须始终使用当前生效的 `listen_port`。

### 7.4 Sync

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

强约束：

- `desired` 是 app 目标态。
- `actual` 是 sync runtime 观测态。
- 两者可以不一致。
- `actual` 必须直接由 runtime 权威状态派生，不得由页面推断。
- 当 `network_enabled=false` 时，`desired` 必须收敛为 `Stopped`。
- 当 `network_enabled=false` 时，`actual` 必须为 `Disabled`。
- 当 `network_enabled=false` 时，`set_sync_desired_state(Running)` 必须返回 `AppError::SyncDisabled`。
- network setting 重新开启后，不允许隐式恢复之前的 `Running` 意图；需要 desktop 再次显式调用 `set_sync_desired_state(Running)`。

### 7.5 Peers

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

强约束：

- 只承诺当前已连接 peers。
- 不承诺 discovered peers。
- 不承诺 offline peer 目录。
- 不承诺 mesh topology。
- `device_id` 是 peer 当前上报的人类可读设备标签，对应对端当前生效的 `identity.device_id`。
- `device_id` 不保证唯一，desktop 可以对重复标签做高亮或告警，但任何匹配、目标选择、状态归属都必须继续以 `noob_id` 为准。
- peers 变化必须出现在 `AppState` 中，不要求单独 peer event。

### 7.5 Clipboard

```rust
pub struct ClipboardState {
    pub latest_committed_event_id: Option<EventId>,
}
```

强约束：

- 对外只承认已提交 record。
- `latest_committed_event_id` 指向最近一次成功写入存储的 clipboard record。
- “最近提交” 包括本地捕获、远端同步写入、用户手动提交。
- 这里不暴露 raw local clipboard 文本，不暴露未提交中间态。

### 7.6 Transfers

```rust
pub struct TransfersState {
    pub incoming_pending: Vec<IncomingTransfer>,
    pub active: Vec<Transfer>,
    pub recent_completed: Vec<CompletedTransfer>,
}
```

#### 7.6.1 Incoming Pending

```rust
pub struct IncomingTransfer {
    pub transfer_id: TransferId,
    pub peer_noob_id: NoobId,
    pub peer_device_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub total_chunks: u32,
    pub offered_at_ms: i64,
}
```

强约束：

- 只表示等待本机决策的 incoming offer。
- `peer_device_id` 是对端当前上报的人类可读设备标签；它不保证唯一，只用于 UI 展示。
- 一旦 accept 或 reject，这条记录必须从 `incoming_pending` 移除。
- reject 后如需保留历史，应进入 `recent_completed`，而不是继续留在 pending。

#### 7.6.2 Active

```rust
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
```

强约束：

- `active` 只保留未结束传输。
- `peer_device_id` 只用于显示；任何匹配、取消、归属都必须继续使用 `transfer_id` / `peer_noob_id`。
- 不允许出现 UI-style 的 `Accepted` / `Rejected` sender 状态。
- `Rejected` 不是 active state，而是 completed outcome。

#### 7.6.3 Recent Completed

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

pub enum TransferOutcome {
    Succeeded,
    Rejected,
    Cancelled,
    Failed,
}
```

强约束：

- `recent_completed` 是会话内 read model，不承诺跨重启持久化。
- `Rejected`、`Cancelled`、`Failed` 都必须有明确 outcome。
- 如果有可展示的错误文本，放在 `message`。

### 7.8 Settings

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
```

#### 7.8.1 Network

```rust
pub struct NetworkSettings {
    pub listen_port: u16,
    pub network_enabled: bool,
    pub mdns_enabled: bool,
    pub manual_peers: Vec<std::net::SocketAddr>,
}
```

#### 7.8.2 Storage

```rust
pub struct StorageSettings {
    pub db_root: std::path::PathBuf,
    pub history_window_days: u32,
    pub dedup_window_days: u32,
    pub max_text_bytes: usize,
    pub gc_batch_size: usize,
}
```

#### 7.8.3 Clipboard

```rust
pub struct ClipboardSettings {
    pub local_capture_enabled: bool,
}
```

#### 7.8.4 Transfers

```rust
pub struct TransferSettings {
    pub download_dir: std::path::PathBuf,
}
```

强约束：

- `SettingsState` 只表示当前生效值。
- 不包含 dirty/draft/review/reset 语义。
- `identity.device_id` 是当前生效的人类可读设备标签。
- `network.listen_port` 是当前生效的监听端口；desktop 只开放端口编辑，不开放 host 编辑。
- `device_id` 继续属于 identity，不并入 network settings。
- `download_dir` 是正式 app setting，不再作为 desktop 本地假设置。
- `local_capture_enabled` 是正式 app setting，不再暴露单独 raw watch 开关接口。

---

## 8. 事件模型

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

强约束：

- `AppEvent` 只描述业务边沿。
- 事件不承载完整权威状态。
- 事件允许只带主键或小型摘要；完整展示数据应从 `AppState` 或业务查询中读取。
- `ClipboardCommitted` 只在 record 成功入库后发出。
- `TransferCompleted` 发出时，相应结果必须已经体现在 `AppState.transfers.recent_completed` 中。
- desktop-local warning/error 不属于 `AppEvent`；宿主自己的桥接失败、查询失败、UI 本地诊断要进入 desktop 本地 edge-feedback 层。

---

## 9. Clipboard 契约

clipboard 是本规格中约束最强的一部分。

### 9.1 外部只看已提交 record

目标外部接口：

```rust
pub struct SubmitTextRequest {
    pub content: String,
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

pub enum ClipboardRecordSource {
    LocalCapture,
    RemoteSync,
    UserSubmit,
}

pub struct ListClipboardHistoryRequest {
    pub limit: usize,
    pub cursor: Option<ClipboardHistoryCursor>,
}

pub struct ClipboardHistoryCursor {
    pub created_at_ms: i64,
    pub event_id: EventId,
}

pub struct ClipboardHistoryPage {
    pub records: Vec<ClipboardRecord>,
    pub next_cursor: Option<ClipboardHistoryCursor>,
}
```

强约束：

- `submit_text()` 必须创建一条已提交 record 并返回 `event_id`。
- `get_clipboard_record(event_id)` 必须按主键读取已提交 record。
- `list_clipboard_history()` 必须按存储顺序分页读取已提交 record。
- `ClipboardCommitted` 只对应已提交 record。

### 9.2 本地 clipboard watch 是内部实现

app 内部可以保留：

- 系统 clipboard backend
- watch worker
- suppression
- ingest pipeline

但这些都是内部实现，不能作为 desktop-facing 契约。

明确禁止：

- 不对 desktop 暴露 `LocalClipboardObserved`
- 不对 desktop 暴露 raw local clipboard 文本订阅
- 不让 desktop 参与 suppression 决策
- 不让 desktop 直接驱动本地 watch pipeline

### 9.3 `submit_text()`

```rust
async fn submit_text(
    &self,
    request: SubmitTextRequest,
) -> AppResult<EventId>;
```

强约束：

- 用于 desktop 主动提交文本。
- 成功返回时文本必须已经写入存储。
- 成功返回时必须已经有对应 `ClipboardRecord` 可读。
- 成功返回后必须最终发出 `ClipboardCommitted` 事件，并推进 `AppState.clipboard.latest_committed_event_id`。

### 9.4 `adopt_clipboard_record()`

```rust
async fn adopt_clipboard_record(
    &self,
    event_id: EventId,
) -> AppResult<()>;
```

强约束：

- 作用是把已提交 record 写回本机系统剪贴板。
- 必须注册 suppression，避免回写后再次被当成本地新内容重复入库。
- 该操作本身不创建新 record。
- 该操作失败时不得产生新的 clipboard record。

### 9.5 `rebroadcast_clipboard_record()`

```rust
pub struct RebroadcastClipboardRequest {
    pub event_id: EventId,
    pub targets: ClipboardBroadcastTargets,
}

pub enum ClipboardBroadcastTargets {
    AllConnected,
    Nodes(Vec<NoobId>),
}
```

强约束：

- 按 `event_id` 从已提交存储读取内容再发送。
- 该操作本身不创建新 record。
- 广播目标只以 connected peers 为准。
- 如果指定 target 不在线，必须返回明确错误或部分失败结果；不能静默成功。

### 9.6 本地捕获开关

本地 clipboard 捕获的开启/关闭属于 settings，不是独立 clipboard subscribe API。

也就是说：

- 是否监听本机系统剪贴板，是 `SettingsState.clipboard.local_capture_enabled`
- 是否真正启动/停止 watch runtime，由 app 内部根据设置驱动
- desktop 不直接调用 `set_local_watch_enabled()` 这类低层接口

---

## 10. Transfer 契约

### 10.1 `send_files()`

```rust
pub struct SendFilesRequest {
    pub targets: Vec<NoobId>,
    pub files: Vec<SendFileItem>,
}

pub struct SendFileItem {
    pub path: std::path::PathBuf,
}
```

强约束：

- 成功返回时，必须已经创建对应 transfer 记录并进入 `TransfersState.active`。
- 返回值中的 `TransferId` 必须与 `AppState` 中的 active transfer 一一对应。
- 发送前的文件选择、拖拽、分组属于 desktop 本地 staging，不属于 app 状态。

### 10.2 `decide_incoming_transfer()`

```rust
pub struct IncomingTransferDecision {
    pub transfer_id: TransferId,
    pub decision: IncomingTransferDisposition,
}

pub enum IncomingTransferDisposition {
    Accept,
    Reject,
}
```

强约束：

- 只能对 `incoming_pending` 中的 transfer 生效。
- `Accept` 后该 transfer 必须离开 pending，并进入 active 或后续 completed。
- `Reject` 后该 transfer 必须离开 pending，并进入 completed outcome=`Rejected`。

### 10.3 `cancel_transfer()`

```rust
async fn cancel_transfer(
    &self,
    transfer_id: TransferId,
) -> AppResult<()>;
```

强约束：

- 是正式能力，不是 UI 本地删除。
- 只对当前可取消的 active transfer 有效。
- 成功取消后，transfer 必须离开 active，并进入 completed outcome=`Cancelled`。
- 如果 transfer 已完成或不存在，必须返回明确错误。

### 10.4 Transfer 状态推进

强约束：

- `IncomingTransferOffered` 事件只表示“有新待决策 incoming offer”。
- `TransferUpdated` 只表示 active transfer 的状态/进度推进。
- `TransferCompleted` 只表示 transfer 已进入终态。
- `TransfersState` 是真相源；事件只是通知。

### 10.5 Recent Completed 的保留策略

本规格允许：

- 会话内 ring buffer
- 固定容量列表

本规格不要求：

- completed transfers 跨重启持久化

但必须保证：

- 在当前 app 运行期间，desktop 可以可靠渲染最近完成项。

---

## 11. Settings 契约

### 11.1 Patch 形状

```rust
pub enum SettingsPatch {
    Identity(IdentitySettingsPatch),
    Network(NetworkSettingsPatch),
    Storage(StorageSettingsPatch),
    Clipboard(ClipboardSettingsPatch),
    Transfers(TransferSettingsPatch),
}
```

```rust
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

### 11.2 语义

- `patch_settings()` 只作用于生效配置。
- patch 成功后，新的 `SettingsState` 必须反映生效值。
- patch 失败时不得部分提交。
- validation 失败必须明确返回。
- `IdentitySettingsPatch::SetDeviceId` 修改 `identity.device_id`。
- `NetworkSettingsPatch::SetListenPort` 只修改监听端口，不开放 host 编辑。
- `IdentitySettingsPatch`、`NetworkSettingsPatch`、`SetDownloadDir` 在 `sync.desired=Running` 时都必须触发 engine reconcile/restart。

### 11.3 Validation

validation 应由 app 统一负责，desktop 只能做辅助校验。

app 至少必须保证：

- `db_root` 合法
- `history_window_days` 合法
- `dedup_window_days` 合法
- `dedup_window_days >= history_window_days`
- `max_text_bytes` 合法
- `gc_batch_size` 合法
- `download_dir` 合法
- manual peer 地址格式合法
- `identity.device_id` 非空
- `listen_port` 在 `1..=65535`

### 11.4 明确不属于 app patch 的内容

以下内容属于 desktop 本地偏好，不应进入 `SettingsPatch`：

- 当前页面选中标签
- 是否展开某个 panel
- clipboard history 当前选中项
- recent activity 的过滤方式
- upload staging 列表
- transfer 完成后的 “Move to” 预设
- “自动把远端文本 adopt 到本地 clipboard” 这类纯 UI/desktop 偏好

---

## 12. Sync 与 Peers 契约

### 12.1 `set_sync_desired_state()`

```rust
async fn set_sync_desired_state(
    &self,
    desired: SyncDesiredState,
) -> AppResult<()>;
```

强约束：

- 只修改目标态，不伪造实际态。
- `actual` 的变化由 runtime 观测推进。
- `network_enabled=false` 时，`desired` 必须被收敛为 `Stopped`。
- `network_enabled=false` 时，`actual` 必须为 `Disabled`。
- `network_enabled=false` 时，`set_sync_desired_state(Running)` 必须返回 `AppError::SyncDisabled`。
- network setting 重新开启后，不允许隐式恢复之前的 `Running` 意图；需要 desktop 再次显式调用 `set_sync_desired_state(Running)`。

### 12.2 Peers 只承诺 connected

这点必须写死：

- app 对 desktop 只承诺当前 connected peers。
- desktop 页面中的 discovered/offline peer 视图不能反向要求 app 提供不存在的语义。
- 如果产品未来确实需要 peer directory，那是新的显式能力，不应从当前 contract 暗推。

### 12.3 Peer 错误事件

`PeerConnectionError` 的作用：

- 提示桌面端 recent activity / toast
- 提供诊断信息

但它不是 peer state 的替代品。真实 connected peers 仍然只看 `AppState.peers.connected`。

---

## 13. Desktop 集成工作流

### 13.1 启动流程

desktop 启动时的推荐顺序：

1. 创建进程级 Tokio runtime
2. 初始化 `DesktopAppServiceImpl`
3. 调用 `get_state()` 拿到 bootstrap 状态
4. 建立 `subscribe_state()`
5. 建立 `subscribe_events()`
6. 将 `AppState` 镜像到一个共享 service-state store
7. 页面只读取这个共享 store，再叠加本地 UI state

### 13.2 UI state 与 service state 分层

service state：

- `AppState`
- 由 `subscribe_state()` 驱动
- 是所有页面共享真相源

UI local state：

- route
- filter
- panel open/close
- 选中 transfer
- 选中 clipboard history
- settings draft
- toast 可见性
- 最近交互反馈

### 13.3 事件流使用场景

`subscribe_events()` 只用于：

- recent activity
- toast
- 一次性反馈
- 日志/调试面板

页面主内容不要直接依赖 event stream 维持正确性。

recent activity 的基础约束：

- 它是 desktop-local normalized feed，不等同于 `AppEvent` 原样列表。
- 允许由三类来源合成：`AppEvent`、`AppState` 的边沿变化、desktop 本地 diagnostic。
- 默认应覆盖的种类至少包括：
  - `ClipboardCommitted`
  - `IncomingTransferOffered`
  - `TransferCompleted`
  - `PeerConnectionError`
  - `SyncStarting`
  - `SyncRunning`
  - `SyncStopped`
  - `SyncDisabledBySettings`
  - `SyncError`
  - `DesktopWarning`
  - `DesktopError`
- `TransferUpdated` 默认不应直接进入 Home recent activity，否则噪音过高；它更适合 transfer 进度区或 toast。

### 13.4 Clipboard 页面工作流

- 列表：`list_clipboard_history()`
- 最新状态提示：`AppState.clipboard.latest_committed_event_id`
- 详情：`get_clipboard_record(event_id)`
- 写回本机 clipboard：`adopt_clipboard_record(event_id)`
- 广播：`rebroadcast_clipboard_record(...)`
- 手动新建文本：`submit_text(...)`

### 13.5 Transfers 页面工作流

- 页面主体直接读 `AppState.transfers`
- send：`send_files(...)`
- accept/reject：`decide_incoming_transfer(...)`
- cancel：`cancel_transfer(...)`

### 13.6 Settings 页面工作流

- 当前值：直接读 `AppState.settings`
- draft/review/reset：desktop 本地做
- apply：`patch_settings(...)`
- apply 后不依赖返回值补 UI，只等 `AppState` 推进

---

## 14. App 内部边界

本规格不限制内部模块拆分，但必须遵守下面边界。

### 14.1 状态中心

app 内部必须有一个唯一状态中心，负责：

- 持有当前 `AppState`
- 推进 `revision`
- 对外提供 `subscribe_state()`

这个状态中心应是 app-lifetime 对象。

### 14.2 事件总线

app 内部必须有独立事件总线，负责：

- 发出 `AppEvent`
- 对外提供 `subscribe_events()`

事件总线不应依赖某个 active sync session 才存在。

### 14.3 Clipboard runtime

clipboard runtime 负责：

- 监听系统 clipboard
- suppression
- 回写系统 clipboard

但它不应直接成为 desktop 的公开 API。

### 14.4 Storage access

clipboard record 的创建和查询必须经过 app service 统一入口。

不允许 desktop 越过 app service 直接连 storage。

### 14.5 Sync runtime

sync runtime 负责：

- actual status
- connected peers
- sync text/network event
- file transfer runtime

但 desktop 不应直接订阅 sync runtime 自身对象。

---

## 15. 必须删除或重构的旧思路

以下设计不应进入最终实现：

### 15.1 Raw local clipboard outward

禁止：

- 对 desktop 暴露 `subscribe_local_clipboard()`
- 对 desktop 暴露 `set_local_watch_enabled()`

这些都应被 `SettingsPatch::Clipboard(SetLocalCaptureEnabled)` 和 commit-only clipboard model 取代。

### 15.2 Session-based service subscription

禁止：

- `subscribe_events()` 因 engine 未运行而失败
- 让 desktop 处理 session id / rebinding / lifecycle 包装

### 15.3 UI-style transfer sender states

禁止：

- `Accepted`
- `Rejected`
- 其他仅服务于 mock UI 的 sender-side 假状态

### 15.4 Fake settings

禁止：

- download dir 只做 desktop 本地展示
- local capture 开关只做 desktop 本地展示

它们都必须成为真实 app setting。

### 15.5 从 UI 反推 peer directory

禁止：

- 因为 desktop 设计了 offline/discovered peers，就让 app 在没有稳定语义时输出伪 peer 列表

---

## 16. 推荐实施顺序

不追求最小改动时，建议按下面顺序开发 `nooboard-app`。

### 阶段 1. 建立新 contract 骨架

目标：

- 引入 `DesktopAppService`
- 引入 `AppState`
- 引入 app-lifetime `subscribe_state()` / `subscribe_events()`

完成标志：

- service 级订阅不再绑定 engine session
- `get_state()` 与 `subscribe_state()` 同构

### 阶段 2. 收敛 clipboard 到 commit-only 模型

目标：

- 删除公开 raw local clipboard outward API
- 建立 `submit_text/get_clipboard_record/list_clipboard_history/adopt/rebroadcast`

完成标志：

- desktop 不再需要知道本地 clipboard watch 细节
- `ClipboardCommitted` 只在入库成功后发出

### 阶段 3. 让 settings 成为真实后端能力

目标：

- `patch_settings()`
- 把 `download_dir`、`local_capture_enabled` 纳入正式 setting

完成标志：

- `SettingsState` 可完整驱动 Settings 页面
- app validation 统一收口

### 阶段 4. 建立 transfer registry

目标：

- `incoming_pending`
- `active`
- `recent_completed`
- `cancel_transfer()`

完成标志：

- transfer 页面可以只读 `AppState.transfers` 渲染主体

### 阶段 5. 清理旧桥接抽象

目标：

- 删除公开 lifecycle/session/rebinding 概念
- 清理仅为旧 Stage4 临时存在的接口

完成标志：

- 对 desktop 暴露的面只剩本规格中的契约

---

## 17. 验收标准

当以下条件全部满足时，可认为 app 侧已经达到本规格：

- `get_state()` 返回完整 `AppState`
- `subscribe_state()` 与 engine 生命周期解耦
- `subscribe_events()` 与 engine 生命周期解耦
- clipboard 对外只暴露已提交 record
- desktop 不再需要 raw local clipboard subscription
- settings 包含 `download_dir` 和 `local_capture_enabled`
- transfers 支持正式 `cancel`
- peers 只承诺 connected peers
- 所有页面主体都能以 `AppState` 为真相源
- desktop 本地偏好没有被污染进 app patch

---

## 18. 一句话总结

最终目标不是“把 desktop 接到一堆 app API 上”，而是让 `nooboard-app` 成为一个语义干净、状态明确、对 desktop 只暴露业务真相的 backend。
