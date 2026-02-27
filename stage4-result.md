# nooboard-app 当前代码文档（供下一阶段开发参考）

更新时间：2026-02-27

## 1. 目标与职责

`nooboard-app` 是 GUI 层的唯一业务入口，负责编排：
- `nooboard-storage`：历史存储、分页查询、重配置
- `nooboard-sync`：网络生命周期、文本广播、文件传输、事件流
- `nooboard-platform`：剪贴板读写

GUI 不直接依赖 storage/sync/platform 的内部 API，通过 `AppService` 完成所有调用。

## 2. 模块结构

### 2.1 顶层

- `src/lib.rs`：统一导出 `AppService`、DTO、错误类型
- `src/error.rs`：`AppError` / `AppResult`
- `src/clipboard_runtime.rs`：剪贴板端口抽象 `ClipboardPort`

### 2.2 配置层（`src/config/`）

- `schema.rs`：配置 Schema（meta/identity/app/storage/sync）
- `io.rs`：加载、原子写回（tmp + rename）、相对路径解析
- `mapping.rs`：`AppConfig -> StorageConfig/SyncConfig` 映射
- `validate.rs`：业务校验（版本、手动 peer 去重、生命周期约束等）
- `node_id.rs`：`noob_id_file` 读取/初始化/重生成
- `defaults.rs`：默认值常量

### 2.3 服务层（`src/service/`）

- `app/mod.rs`：`AppService` trait + `AppServiceImpl`
- `app/engine.rs`：start/stop/restart/status/connected_peers
- `app/clipboard_history.rs`：A1~A6 语义（本地写入、历史复用、远端解耦）
- `app/files.rs`：文件发送与决策
- `app/subscriptions.rs`：事件订阅入口
- `app/config_patch_network.rs`：网络配置 patch
- `app/config_patch_storage.rs`：存储配置 patch
- `app/config_transcation.rs`：配置事务骨架（当前文件名保持现状）
- `events.rs`：`SubscriptionHub`，聚合 sync/transfer 两路事件
- `mappers.rs`：sync/storage 类型到 app DTO 的映射
- `types/`：按领域拆分的对外 DTO

### 2.4 Runtime 层

- `src/sync_runtime/`：sync 引擎句柄生命周期、桥接、命令发送
- `src/storage_runtime/`：storage actor 线程、命令队列、重配置

### 2.5 测试

- `tests/app_service_stage4.rs`：核心业务链路（剪贴板、文件、订阅、网络 patch）
- `tests/app_service_config_patch.rs`：配置 patch 专项（network/storage）

## 3. AppService 接口基线

`AppService` 当前公开接口分为五组：

1. 生命周期与状态  
- `start_engine` / `stop_engine` / `restart_engine`
- `sync_status`
- `connected_peers`

2. 剪贴板与历史  
- `apply_local_clipboard_change`
- `apply_history_entry_to_clipboard`
- `list_history`
- `rebroadcast_history_entry`
- `store_remote_text`
- `write_remote_text_to_clipboard`

3. 文件业务  
- `send_file`
- `respond_file_decision`

4. 事件订阅  
- `subscribe_events`（统一输出 `AppEvent`）

5. 配置 patch  
- `apply_network_patch(NetworkPatch) -> BroadcastConfig`
- `apply_storage_patch(StoragePatch) -> StorageConfigView`

## 4. 关键数据模型

### 4.1 标识与目标

- `EventId`：UUID v7
- `NodeId`：节点标识字符串
- `Targets`：
  - `All`
  - `Nodes(Vec<NodeId>)`
- `Targets` 内含标准化逻辑：trim + 去空 + 去重

### 4.2 事件模型

- `AppEvent`：
  - `Sync(SyncEvent)`
  - `Transfer(TransferUpdate)`
- `SyncEvent` 当前包含：
  - `TextReceived`
  - `FileDecisionRequired`
  - `ConnectionError`

### 4.3 配置 patch 模型

- `NetworkPatch`
  - `SetMdnsEnabled(bool)`
  - `SetNetworkEnabled(bool)`
  - `AddManualPeer(SocketAddr)`
  - `RemoveManualPeer(SocketAddr)`
- `StoragePatch`（部分更新，字段全为 `Option<T>`）
  - `db_root`
  - `retain_old_versions`
  - `history_window_days`
  - `dedup_window_days`
  - `gc_every_inserts`
  - `gc_batch_size`

## 5. 运行时流程

### 5.1 Sync Runtime

- `SyncRuntime::start` 调用 `start_sync_engine`，保存 `SyncEngineHandle`
- 内部保留两条桥接链路（实现细节，不是对外接口）：
  - `event_rx -> event_tx`（sync 事件）
  - `progress_rx -> transfer_tx`（transfer 进度事件）
- `stop` 触发 shutdown 并等待状态落到终态，再中止 bridge 任务

### 5.2 Storage Runtime

- 采用单独 actor 线程执行存储命令
- `append_text/list_history/reconfigure` 通过命令队列 + oneshot 回包
- 支持运行中重配 storage 配置

### 5.3 事件订阅

- `SubscriptionHub` 首次订阅时启动聚合任务
- 通过 `tokio::select!` 同时消费 sync/transfer 两路
- 向外统一广播 `AppEvent`（`subscribe_events` 的唯一输出）
- 对 lagged 事件采用跳过策略（继续消费新事件）
- 结论：内部是“双通道桥接”，外部是“单订阅接口（`AppEvent`）”

## 6. 配置系统与事务

### 6.1 配置加载与持久化

- `AppConfig::load`：
  - TOML 解析
  - 相对路径绝对化
  - node_id 文件读取/初始化
  - 配置校验
- `AppConfig::save_atomically`：
  - 写临时文件
  - rename 覆盖原文件

### 6.2 配置事务骨架（`config_transcation.rs`）

两条事务入口：
- `execute_network_config_transcation`
- `execute_storage_config_transcation`

公共步骤：
1. `config_update_lock` 串行化
2. 读取旧配置
3. 应用 patch
4. `validate`
5. 写回 + 应用运行时变更
6. 失败回滚（配置文件 + 对应 runtime）

网络应用函数：
- `persist_and_restart_sync_with_rollback`

存储应用函数：
- `persist_and_reconfigure_storage_with_rollback`

## 7. 当前业务语义要点

1. 本地剪贴板变更  
- 先写 storage，生成 `event_id`
- 仅当 `sync.network.enabled == true` 且 `targets` 可发送时尝试广播

2. 历史复用（按 recent-N）  
- `apply_history_entry_to_clipboard` 与 `rebroadcast_history_entry`
- 都基于 `recent_event_lookup_limit` 窗口内按 `event_id` 匹配

3. 远端文本处理解耦  
- `store_remote_text`：仅入库
- `write_remote_text_to_clipboard`：仅写剪贴板

4. 文件业务  
- `send_file` 对空目标直接返回 `Ok(())`
- `respond_file_decision` 透传到 sync runtime

## 8. 下一阶段扩展建议（基于当前结构）

1. 新增配置项时
- 在 `config/schema.rs` 加字段
- 在 `validate.rs` 补约束
- 在 `mapping.rs` 补映射
- 在 `types/network.rs` 增加 patch/view 字段（如需对外）
- 在 `config_patch_*` 与 `config_transcation.rs` 接入应用流程

2. 新增事件类型时
- 在 `service/types/events.rs` 定义
- 在 `mappers.rs` 做映射
- 在 `tests/app_service_stage4.rs` 增加链路用例

3. 新增业务 API 时
- `service/app/mod.rs` 扩展 trait
- 在 `service/app/*.rs` 落地 usecase
- 优先复用现有 runtime，不直接穿透到 sync/storage 内部类型

## 9. 验证命令

推荐基线命令：
1. `cargo check --workspace`
2. `cargo test -p nooboard-app`
3. `cargo test -p nooboard-sync --test p2p_file_transfer`
