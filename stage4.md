# nooboard 阶段 4 开发计划（AI 执行版，聚焦 `nooboard-app`）

更新时间：2026-02-26

## 1. 目标与边界
1. 新建 crate：`nooboard-app`，统一封装 `nooboard-storage`、`nooboard-sync`、`nooboard-platform`。
2. GUI 仅调用 `nooboard-app`，不直接调用 storage/sync/platform。
3. 本文只定义 `stage4`（`nooboard-app`）执行计划。

## 2. 业务语义冻结
### 2.1 A. 剪切板业务
1. 本地剪切板更新 -> 写数据库 -> 向指定目标设备广播。
2. 给定 `event_id`，仅在“最新 N 条历史”中查找匹配项；若找到则写入本地剪切板，不广播。
3. 支持 `HistoryRecord` 游标分页查询，顺序新到旧。
4. 给定 `event_id`，仅在“最新 N 条历史”中查找匹配项；若找到则向指定目标设备广播文本。
5. 接收远端文本并写入数据库。
6. 接收远端文本并写入剪切板。
7. 第 5 与第 6 必须解耦，由 app 显式调用。
8. N 可以配置，默认值为 50。

### 2.2 B. 文件业务
1. `nooboard-app` 只做编排，包装 `nooboard-sync` 文件发送、接收决策、进度订阅。

### 2.3 C. 广播业务
1. 开启/关闭 mDNS。
2. 添加/删除手动 peer。
3. 开启/关闭联网业务。
4. 持久化策略：修改配置文件后调用 `restart_engine` 应用变更。

## 3. 关键技术决策
1. `noob_id` 与 `device_id` 分离：
   1. `noob_id` 用于连接去重与路由。
   2. `device_id` 为人类可读标识，不要求唯一。
2. 配置变更统一走“配置事务”流程：
   1. 读取配置。
   2. 应用 patch。
   3. 校验。
   4. 原子写回（tmp + rename）。
   5. `restart_engine` 应用。
   6. 若失败，回滚配置并恢复旧引擎。
3. 所有“改配置+重启”操作必须串行化（单互斥或单线程命令队列）。
4. A2/A4 不新增 storage 主键点查接口；在 app 层复用 `list_history(limit=N)` 后内存匹配 `event_id`。

## 4. 配置 Schema（v2，按功能分组）
```toml
[meta]
config_version = 2
profile = "dev"

[identity]
noob_id_file = "/Users/zero/study/rust/nooboard/.dev-data/noob_id"
device_id = "dev-mac"

[app.clipboard]
recent_event_lookup_limit = 50

[storage]
db_root = "/Users/zero/study/rust/nooboard/.dev-data"
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
download_dir = "/Users/zero/study/rust/nooboard/.dev-data/downloads"
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

映射约束：
1. `sync.network.enabled` -> `SyncConfig.enabled`
2. `sync.network.mdns_enabled` -> `SyncConfig.mdns_enabled`
3. `sync.network.listen_addr` -> `SyncConfig.listen_addr`
4. `sync.network.manual_peers` -> `SyncConfig.manual_peers`
5. `sync.auth.token` -> `SyncConfig.token`
6. `sync.file.download_dir` -> `SyncConfig.download_dir`
7. `sync.file.max_file_size` -> `SyncConfig.max_file_size`
8. `sync.file.chunk_size` -> `SyncConfig.file_chunk_size`
9. `sync.file.active_downloads` -> `SyncConfig.active_downloads`
10. `sync.file.decision_timeout_ms` -> `SyncConfig.file_decision_timeout_ms`
11. `sync.file.idle_timeout_ms` -> `SyncConfig.transfer_idle_timeout_ms`
12. `sync.transport.connect_timeout_ms` -> `SyncConfig.connect_timeout_ms`
13. `sync.transport.handshake_timeout_ms` -> `SyncConfig.handshake_timeout_ms`
14. `sync.transport.ping_interval_ms` -> `SyncConfig.ping_interval_ms`
15. `sync.transport.pong_timeout_ms` -> `SyncConfig.pong_timeout_ms`
16. `sync.transport.max_packet_size` -> `SyncConfig.max_packet_size`

## 5. 目标代码结构
1. `crates/nooboard-app/src/lib.rs`
2. `crates/nooboard-app/src/service.rs`
3. `crates/nooboard-app/src/config.rs`
4. `crates/nooboard-app/src/sync_runtime.rs`
5. `crates/nooboard-app/src/clipboard_runtime.rs`
6. `crates/nooboard-app/src/error.rs`
7. `crates/nooboard-app/tests/`

## 6. AppService 契约草案（对 GUI 稳定）
1. 生命周期与状态
   1. `start_engine()`
   2. `stop_engine()`
   3. `restart_engine()`
   4. `sync_status()`
   5. `connected_peers()`
2. 剪切板业务
   1. `handle_local_clipboard_change(text, targets)`
   2. `apply_history_to_clipboard(event_id)`
   3. `list_history(limit, cursor)`
   4. `rebroadcast_history_event(event_id, targets)`
   5. `accept_remote_text_to_storage(remote_text)`
   6. `accept_remote_text_to_clipboard(remote_text)`
3. 文件业务
   1. `send_file(path, targets)`
   2. `respond_file_decision(peer_noob_id, transfer_id, accept, reason)`
   3. `subscribe_transfer_updates()`
4. 广播配置业务
   1. `set_mdns_enabled(enabled)`
   2. `add_manual_peer(addr)`
   3. `remove_manual_peer(addr)`
   4. `set_network_enabled(enabled)`

## 7. AI 执行任务清单
### T0. 工程骨架
1. 输入：workspace 现有 crates。
2. 动作：创建 `crates/nooboard-app`，加入 workspace，补最小 `lib.rs`、`service.rs`、`error.rs`。
3. 输出：`cargo check --workspace` 通过。

### T1. 错误模型、DTO 与配置解析
1. 输入：`nooboard-core/nooboard-storage/nooboard-sync` 错误与模型，v2 配置 schema。
2. 动作：定义 `AppError`、`AppResult`、公开 DTO（历史分页、远端文本、广播配置更新结果）；实现 v2 配置解析，并完成 `sync.network/auth/file/transport` 到 `SyncConfig` 的映射。
3. 输出：`nooboard-app` 公共类型可编译、配置可加载。

### T2. Sync 生命周期封装
1. 输入：`SyncEngineHandle` 与事件流。
2. 动作：在 app 层封装 start/stop/restart/status，维护状态快照与错误收敛。
3. 输出：引擎状态流稳定，可重复启停。

### T3. 剪切板本地链路（A1）
1. 输入：平台剪切板监听事件。
2. 动作：本地变更入库并按 targets 广播。
3. 输出：A1 可调用，失败路径可观察。

### T4. 历史检索与 event_id 复用（A2/A3/A4）
1. 输入：`list_history(limit, cursor)`。
2. 动作：
   1. 实现历史分页（A3）。
   2. 实现 app 内部 helper：读取最新 N 条并按 `event_id` 匹配。
   3. A2：匹配成功则仅写剪切板。
   4. A4：匹配成功则仅广播。
3. 输出：A2/A4 对外语义明确为 `recent-N`；找不到返回业务错误（如 `NotFoundInRecentWindow`）。

### T5. 远端文本解耦处理（A5/A6）
1. 输入：sync `TextReceived { event_id, content, source_device_id }`。
2. 动作：提供两个独立 API：仅入库、仅写剪切板。
3. 输出：A5 与 A6 无隐式联动。

### T6. 文件业务包装（B）
1. 输入：sync 文件发送/决策/进度事件。
2. 动作：app 层对外暴露稳定接口并透传关键字段。
3. 输出：GUI 无需依赖 sync 内部类型。

### T7. 广播配置事务（C）
1. 输入：当前配置文件与更新请求。
2. 动作：串行化执行“读-改-校验-原子写-重启-回滚”。
3. 输出：`set_mdns_enabled/add_peer/remove_peer/set_network_enabled` 均持久化且可通过重启生效。

### T8. 测试与回归
1. 输入：T0~T7 产物。
2. 动作：
   1. 单测覆盖 A/B/C 正常与错误路径。
   2. 集成测试覆盖双节点文本来源标识与配置重启生效。
3. 输出：测试通过并记录在 `stage4-1.md`。

## 8. 验证矩阵（必须执行）
1. `cargo check --workspace`
2. `cargo test -p nooboard-app`
3. `cargo test -p nooboard-sync --test p2p_file_transfer`

## 9. Stage4 完成标准（DoD）
1. `nooboard-app` 对外导出稳定：`AppService/AppServiceImpl/AppError/DTO`。
2. A/B/C 业务均有 API、测试和文档化语义。
3. C 业务采用“配置持久化 + restart_engine 应用 + 失败回滚”。
4. A2/A4 明确为“仅最近 N 条按 `event_id` 匹配”。
5. 验证矩阵命令全部通过。
