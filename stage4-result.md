# nooboard-app 当前状态与 API 文档

更新时间：2026-02-27

## 1. crate 职责

`nooboard-app` 是应用层编排边界（当前仍以 `app` 为顶层语义），负责：

- 统一对外暴露控制 API（`AppService`）
- 控制面串行状态机（单 mailbox actor）
- 编排 `nooboard-sync` 生命周期与事件订阅
- 编排 `nooboard-storage` 历史/配置重配置/outbox 数据面接口
- 编排剪贴板读写 (`ClipboardPort`)

当前仓库内不存在 `app` 的外部调用者约束，因此 API 已按新语义硬切换。

## 2. 架构概览

### 2.1 顶层模块

- `crates/nooboard-app/src/lib.rs`
- `crates/nooboard-app/src/error.rs`
- `crates/nooboard-app/src/config/*`
- `crates/nooboard-app/src/service/*`
- `crates/nooboard-app/src/sync_runtime/*`
- `crates/nooboard-app/src/storage_runtime/*`

### 2.2 service/app 结构

- `service/app/mod.rs`：
  - `AppService` trait（对外 API）
  - `AppServiceImpl` facade/client（仅发命令 + 收回复）
- `service/app/control/*`：控制面内聚实现
  - `command.rs`：actor 命令定义（mpsc + oneshot）
  - `state.rs`：控制状态聚合
  - `actor.rs`：单 mailbox 主循环
  - `engine_reconcile.rs`：desired/actual 生命周期收敛
  - `config_patch.rs`：patch 应用 + 持久化 + 回滚聚合
  - `outbox.rs`：内部 tick 驱动 outbox 调度
  - `clipboard_history.rs` / `files.rs` / `subscriptions.rs`：业务子域命令处理

### 2.3 关键设计原则

- 控制面串行：所有控制命令在同一 actor 内顺序执行
- 对外暴露意图：调用方提交 desired state / patch，不暴露重启细节
- Facade 轻量：`AppServiceImpl` 不再承担锁编排职责
- 高内聚：控制面逻辑集中在 `service/app/control`，避免横向散落

## 3. 对外 API（当前基线）

`AppService` 的控制 API：

1. `shutdown() -> AppResult<()>`
2. `set_sync_desired_state(SyncDesiredState) -> AppResult<AppServiceSnapshot>`
3. `apply_config_patch(AppPatch) -> AppResult<AppServiceSnapshot>`
4. `snapshot() -> AppResult<AppServiceSnapshot>`

`SyncDesiredState`：

- `Running`
- `Stopped`

`AppPatch`：

- `Network(NetworkPatch)`
- `Storage(StoragePatch)`

其余业务 API（剪贴板/历史/文件/订阅）仍由 `AppService` 统一暴露。

## 4. 关键 DTO

### 4.1 `AppServiceSnapshot`

`snapshot()` 与控制 API 返回的聚合视图，当前包含：

- `desired_state`
- `actual_sync_status`
- `connected_peers`
- `network_enabled`
- `mdns_enabled`
- `manual_peers`
- `storage` (`StorageConfigView`)

### 4.2 配置 patch

`NetworkPatch`：

- `SetMdnsEnabled(bool)`
- `SetNetworkEnabled(bool)`
- `AddManualPeer(SocketAddr)`
- `RemoveManualPeer(SocketAddr)`

`StoragePatch`（全字段 `Option<T>`）：

- `db_root`
- `retain_old_versions`
- `history_window_days`
- `dedup_window_days`
- `gc_every_inserts`
- `gc_batch_size`

## 5. 控制面行为语义

### 5.1 生命周期语义

- `set_sync_desired_state(Running)`：
  - desired state 更新为 `Running`
  - reconcile 启动/保持 sync runtime
- `set_sync_desired_state(Stopped)`：
  - desired state 更新为 `Stopped`
  - 订阅关闭并停止 sync runtime
- `shutdown()`：
  - 停止 outbox ticker
  - 停止 sync runtime
  - shutdown storage runtime
  - 关闭 control actor mailbox

### 5.2 patch 语义

- `apply_config_patch` 在 actor 内串行执行：
  1. clone 当前 config
  2. 应用 patch
  3. validate
  4. 原子写回配置
  5. reconfigure storage
  6. reconcile engine（按 patch 类型触发）
- 失败时执行回滚并聚合错误：
  - 尽量执行全部 rollback 步骤
  - 返回 `AppError::ConfigRollbackFailed`（若有回滚失败）

### 5.3 outbox 语义

- 使用内部 `TickOutbox` 命令驱动
- 周期 ticker 只负责向 actor 投递 tick，不直接执行业务
- 实际 dispatch/lease/retry 全在 actor 状态机内串行执行

## 6. 当前测试状态

主要测试文件：

- `crates/nooboard-app/tests/app_service_stage4.rs`
- `crates/nooboard-app/tests/app_service_config_patch.rs`

覆盖重点：

- 剪贴板/历史/文件/订阅主链路
- desired state 幂等性
- 并发网络 patch 一致性
- patch 持久化与相对路径解析
- shutdown 终止行为
- outbox 在启动后补发链路

## 7. 建议验证命令

1. `cargo fmt`
2. `cargo check --workspace`
3. `cargo test -p nooboard-app`
4. `cargo test -p nooboard-sync --test p2p_file_transfer`
