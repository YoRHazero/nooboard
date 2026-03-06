# nooboard-app API 文档

更新时间：2026-03-01

## 1. 定位

`nooboard-app` 是应用层编排 crate，统一封装：

1. `nooboard-sync`（网络同步与文件传输）
2. `nooboard-storage`（历史存储与分页查询）
3. `nooboard-platform`（剪切板能力）

上层（GUI/CLI）应优先通过 `AppService` 调用业务能力，而不是直接拼接底层 crate。

## 2. 对外入口

## 2.1 推荐入口（高层）

1. `AppService`（trait）
2. `AppServiceImpl`（默认实现）
3. `AppError` / `AppResult`
4. 业务 DTO 与事件类型（`LocalClipboardChangeRequest`、`HistoryPage`、`AppEvent` 等）

## 2.2 模块入口（低层）

1. `nooboard_app::config`：配置结构、加载、校验、落盘与映射
2. `nooboard_app::sync_runtime`：同步引擎运行时封装（高级用法）
3. `nooboard_app::clipboard_runtime`：剪切板抽象与运行时包装

## 3. 快速接入

```rust
use std::sync::Arc;
use nooboard_app::{AppService, AppServiceImpl, ClipboardPort};

struct MyClipboard;
impl ClipboardPort for MyClipboard {
    fn read_text(&self) -> nooboard_app::AppResult<Option<String>> { Ok(None) }
    fn write_text(&self, _text: &str) -> nooboard_app::AppResult<()> { Ok(()) }
}

#[tokio::main]
async fn main() -> nooboard_app::AppResult<()> {
    let service = AppServiceImpl::new("configs/app.toml", Arc::new(MyClipboard))?;
    let _snapshot = service.snapshot().await?;
    service.shutdown().await?;
    Ok(())
}
```

## 4. AppService API

```rust
#[allow(async_fn_in_trait)]
pub trait AppService {
    async fn shutdown(&self) -> AppResult<()>;
    async fn set_sync_desired_state(&self, desired_state: SyncDesiredState)
        -> AppResult<AppServiceSnapshot>;
    async fn apply_config_patch(&self, patch: AppPatch)
        -> AppResult<AppServiceSnapshot>;
    async fn snapshot(&self) -> AppResult<AppServiceSnapshot>;

    async fn apply_local_clipboard_change(
        &self,
        request: LocalClipboardChangeRequest,
    ) -> AppResult<LocalClipboardChangeResult>;
    async fn apply_history_entry_to_clipboard(&self, event_id: EventId) -> AppResult<()>;
    async fn list_history(&self, request: ListHistoryRequest) -> AppResult<HistoryPage>;
    async fn rebroadcast_history_entry(&self, request: RebroadcastHistoryRequest)
        -> AppResult<()>;
    async fn store_remote_text(&self, request: RemoteTextRequest) -> AppResult<()>;
    async fn write_remote_text_to_clipboard(&self, request: RemoteTextRequest)
        -> AppResult<()>;

    async fn send_file(&self, request: SendFileRequest) -> AppResult<()>;
    async fn respond_file_decision(&self, request: FileDecisionRequest) -> AppResult<()>;

    async fn subscribe_events(&self) -> AppResult<EventSubscription>;
}
```

## 4.1 生命周期与配置

### `shutdown()`

1. 关闭事件订阅会话（`EngineStopped`）
2. 停止 sync runtime
3. 关闭 storage runtime
4. 之后再调用多数 API 会得到 `AppError::ChannelClosed`

### `set_sync_desired_state(SyncDesiredState)`

`SyncDesiredState`：

1. `Running`
2. `Stopped`（默认）

行为：

1. 更新 desired state
2. 触发引擎状态收敛（启动/停止）
3. 返回最新 `AppServiceSnapshot`

### `apply_config_patch(AppPatch)`

`AppPatch`：

1. `AppPatch::Network(NetworkPatch)`
2. `AppPatch::Storage(StoragePatch)`

执行流程：

1. 应用 patch
2. 校验配置
3. 原子写回配置文件
4. 重配 storage runtime
5. 必要时重启/重载 sync runtime
6. 失败时执行回滚（配置、存储、引擎）

`NetworkPatch`：

1. `SetMdnsEnabled(bool)`
2. `SetNetworkEnabled(bool)`
3. `AddManualPeer(SocketAddr)`
4. `RemoveManualPeer(SocketAddr)`

`StoragePatch`（可选字段）：

1. `db_root`
2. `retain_old_versions`
3. `history_window_days`
4. `dedup_window_days`
5. `gc_every_inserts`
6. `gc_batch_size`

### `snapshot()`

返回 `AppServiceSnapshot`：

1. `desired_state`
2. `actual_sync_status`（`Disabled | Starting | Running | Stopped | Error(String)`）
3. `connected_peers`
4. `network_enabled`
5. `mdns_enabled`
6. `manual_peers`
7. `storage`（`StorageConfigView`）

## 4.2 文本与历史

### `apply_local_clipboard_change(LocalClipboardChangeRequest)`

请求：

1. `text`
2. `targets`

行为：

1. 生成新 `event_id` 并写入历史
2. 按当前实时连接尝试发送到 sync 文本通道（非阻塞 `try_send`）
3. 返回 `LocalClipboardChangeResult { event_id, broadcast_status }`

`broadcast_status`：

1. `NotRequested`
2. `Sent`
3. `Dropped(BroadcastDropReason)`

`BroadcastDropReason`：

1. `NetworkDisabled`
2. `EngineNotRunning`
3. `NoEligiblePeer`
4. `QueueFull`
5. `QueueClosed`

### `apply_history_entry_to_clipboard(event_id)`

行为：

1. 在“最近 N 条”（`recent_event_lookup_limit`）里查找
2. 找到后写本地剪切板
3. 不做网络发送

找不到返回 `AppError::NotFoundInRecentWindow`。

### `list_history(ListHistoryRequest)`

请求：

1. `limit`
2. `cursor`（可选）

返回 `HistoryPage { records, next_cursor }`，按时间倒序（新到旧）。

### `rebroadcast_history_entry(RebroadcastHistoryRequest)`

请求：

1. `event_id`
2. `targets`

行为：

1. 在最近 N 条里找指定记录
2. 严格尝试发送到 sync 文本通道

注意：

1. `targets` 无有效目标时直接成功返回（no-op）
2. 网络禁用返回 `AppError::SyncDisabled`
3. `EngineNotRunning`、通道满/关闭会返回错误
4. 无匹配在线目标节点时返回成功（no-op）

### `store_remote_text(RemoteTextRequest)`

只写存储，不写剪切板。

### `write_remote_text_to_clipboard(RemoteTextRequest)`

只写剪切板，不写存储。

`store_remote_text` 与 `write_remote_text_to_clipboard` 语义解耦，可按业务顺序单独调用。

## 4.3 文件传输

### `send_file(SendFileRequest)`

请求：

1. `path`
2. `targets`

行为：

1. `targets` 无有效目标时 no-op 成功返回
2. 网络禁用返回 `AppError::SyncDisabled`
3. 其余通过 sync 文件通道发送

### `respond_file_decision(FileDecisionRequest)`

请求：

1. `peer_noob_id`
2. `transfer_id`
3. `accept`
4. `reason`

行为：把接收方决策回传给 sync 引擎。

## 4.4 事件订阅

### `subscribe_events() -> EventSubscription`

前置条件：sync 引擎已运行；否则返回 `AppError::EngineNotRunning`。

`EventSubscription`：

1. `session_id()`
2. `recv().await`
3. `try_recv()`

首次 `recv/try_recv` 会先返回 `Lifecycle::Opened`，之后返回真实流事件。

## 5. 关键类型

## 5.1 标识与目标

`EventId`：

1. 内部是 UUID v7
2. 支持 `EventId::new()`
3. 支持 `TryFrom<&str>`（无效字符串返回 `InvalidEventId`）

`NoobId`：

1. 字符串封装
2. `NoobId::new(...)` / `as_str()`

`Targets`：

1. `All`
2. `Nodes(Vec<NoobId>)`

`Nodes` 在发送前会做 trim、去空字符串、去重。

## 5.2 历史分页

`HistoryRecord`：

1. `event_id`
2. `origin_device_id`
3. `created_at_ms`
4. `applied_at_ms`
5. `content`

`HistoryCursor`：

1. `created_at_ms`
2. `event_id`

## 5.3 文件事件

`TransferUpdate`：

1. `transfer_id`
2. `peer_noob_id`
3. `direction`（`Incoming | Outgoing`）
4. `state`

`TransferState`：

1. `Started`
2. `Progress`
3. `Finished`
4. `Failed`
5. `Cancelled`

## 5.4 统一事件模型

`AppEvent`：

1. `AppEvent::Sync(SyncEvent)`
2. `AppEvent::Transfer(TransferUpdate)`

`SyncEvent`：

1. `TextReceived`
2. `FileDecisionRequired`
3. `ConnectionError`

## 5.5 订阅生命周期

`SubscriptionLifecycle`：

1. `Opened`
2. `Rebinding`
3. `Lagged`
4. `RecoverableError`
5. `Fatal`
6. `Closed`

`Closed` 的原因 `SubscriptionCloseReason`：

1. `EngineStopped`
2. `Rebinding { next_session_id }`
3. `UpstreamClosed { stream }`
4. `Fatal`

## 6. 配置 API（AppConfig）

`AppConfig` 提供：

1. `AppConfig::load(path)`
2. `save_atomically(path)`
3. `validate()`
4. `to_storage_config()`
5. `to_sync_config()`
6. `recent_event_lookup_limit()`
7. `noob_id()`
8. `regenerate_node_id(config_path)`

## 6.1 常量

1. `APP_CONFIG_VERSION = 2`
2. `DEFAULT_RECENT_EVENT_LOOKUP_LIMIT = 50`

## 6.2 加载行为

`load` 时会：

1. 解析 TOML
2. 将相对路径转成配置文件目录下绝对路径（`identity.noob_id_file`、`storage.db_root`、`sync.file.download_dir`）
3. 读取或初始化 `noob_id` 文件
4. 执行校验

## 6.3 校验规则（摘要）

1. `meta.config_version` 必须等于 `2`
2. `identity.device_id` 不能为空
3. `app.clipboard.recent_event_lookup_limit > 0`
4. `history_window_days >= 1`
5. `dedup_window_days >= history_window_days`
6. `gc_every_inserts >= 1`
7. `gc_batch_size >= 1`
8. `sync.network.manual_peers` 不允许重复
9. `to_sync_config()` 必须通过 sync 层校验

## 6.4 配置示例

```toml
[meta]
config_version = 2
profile = "dev"

[identity]
noob_id_file = ".dev-data/noob_id"
device_id = "dev-mac"

[app.clipboard]
recent_event_lookup_limit = 50

[storage]
db_root = ".dev-data"
retain_old_versions = 0

[storage.lifecycle]
history_window_days = 7
dedup_window_days = 14
gc_every_inserts = 200
gc_batch_size = 500

[sync.network]
enabled = true
mdns_enabled = true
listen_addr = "0.0.0.0:17890"
manual_peers = []

[sync.auth]
token = "dev-sync-token"

[sync.file]
download_dir = ".dev-data/downloads"
max_file_size = 10737418240
chunk_size = 65536
active_downloads = 8
decision_timeout_ms = 30000
idle_timeout_ms = 15000

[sync.transport]
connect_timeout_ms = 5000
handshake_timeout_ms = 5000
ping_interval_ms = 5000
pong_timeout_ms = 15000
max_packet_size = 8388608
```

## 7. 剪切板抽象（clipboard_runtime）

`ClipboardPort`：

1. `read_text() -> AppResult<Option<String>>`
2. `write_text(text: &str) -> AppResult<()>`

`ClipboardRuntime`：

1. `new(Arc<dyn ClipboardPort>)`
2. `read_text()`
3. `write_text(text)`

任何实现了 `nooboard_platform::ClipboardBackend` 的类型都可自动作为 `ClipboardPort` 使用。

## 8. SyncRuntime（高级入口）

`SyncRuntime` 公开能力：

1. 生命周期：`new`、`start`、`stop`、`restart`、`has_engine`
2. 状态读取：`status`、`connected_peers`
3. 发送通道：`text_sender`、`file_sender`、`decision_sender`、`control_sender`
4. 订阅通道：`subscribe_events`、`subscribe_transfer_updates`、`subscribe_status`

该模块更接近底层运行时，常规业务建议优先走 `AppService`。

## 9. 错误模型（AppError）

主要错误类别：

1. `Io` / `Storage` / `Sync` / `Platform`
2. `ConfigParse` / `ConfigSerialize` / `InvalidConfig`
3. `EngineNotRunning` / `EngineAlreadyRunning` / `SyncDisabled`
4. `ChannelClosed`
5. `NotFoundInRecentWindow`
6. `InvalidEventId`
7. `ManualPeerExists` / `ManualPeerNotFound`
8. `ConfigRollbackFailed`

## 10. 运行语义约定

1. 控制面为单 actor 串行处理，所有 `AppService` 调用进入同一命令队列。
2. 本地文本广播是“实时尝试”，不会为离线场景做自动补发。
3. 历史回放相关操作（应用到剪切板、重发）只在最近 N 条窗口内查找。
4. `store_remote_text` 与 `write_remote_text_to_clipboard` 无隐式联动。
5. 事件订阅基于广播通道，慢消费者会收到 `Lifecycle::Lagged`。
