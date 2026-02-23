# nooboard 阶段 4-1：`nooboard-app` 已完成验收记录（2026-02-23）

## 1. 范围确认与结论
1. 本轮只完成 `stage4-1`（`nooboard-app`），未进入 `stage4-2`，未开发/恢复 `nooboard-gui`。
2. `AppService` 契约已冻结并与实现对齐，`stop_sync` 幂等语义已明确且有测试。
3. 同步状态可观测性已完善：`connected_peers` 已接入 `nooboard-sync` 真实路径，`last_event_at` 在状态更新路径统一刷新。
4. 生命周期健壮性已收敛：覆盖 `runtime` 构建失败、engine 失败、worker panic、异常后重启。
5. `stage4-1 DoD` 全部通过，可进入 `stage4-2`。

## 2. `AppService` 契约冻结版（与实现一致）
1. `list_history(limit, keyword)`
   - `keyword` 先 `trim`，空字符串视为 `None`。
   - 查询顺序与存储层一致（最近优先）。
2. `set_clipboard(text)`
   - 只写系统剪贴板，不直接写历史库。
   - 平台写入失败映射为 `AppError::Platform`。
3. `start_sync(config)`
   - 若有活动 worker，返回 `AppError::SyncAlreadyRunning`。
   - 启动时先进入 `Starting`，随后进入 `Running`/`Error`。
   - runtime 构建失败时，立即返回 `AppError::Runtime` 且状态收敛到 `Error`。
4. `stop_sync()`
   - 幂等：无 worker 也返回 `Ok(())`，最终状态收敛为 `Stopped`。
   - 有 worker 时先置 `Stopping`，发送停机信号并 `join`。
   - 若 worker panic，映射为 `AppError::Runtime`，状态收敛 `Error`。
5. `sync_status()`
   - 返回快照副本，不暴露内部锁。
   - 包含 `state/listen/connected_peers/last_error/last_event_at`。

## 3. 实现落地与证据

### 3.1 同步状态可观测性
1. `connected_peers` 实际来源接入：
   - `TransportRuntime` 新增 `connected_peer_count()`，读取活跃 peer 集合大小。
   - `SyncEngine` 新增 `run_with_shutdown_and_peer_observer(...)`，周期轮询并在 peer 数变化时回调上报。
   - `AppServiceImpl` 在 `start_sync` 中接入 observer 回调，将 peer 数写回 `SyncStatus.connected_peers`。
2. `last_event_at` 一致性：
   - 所有状态更新入口（含 peer 数变化）统一通过 `update_status`/`update_status_arc`，自动刷新 `last_event_at`。

涉及文件：
1. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/transport.rs`
2. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/engine.rs`
3. `/Users/zero/study/rust/nooboard/crates/nooboard-app/src/service.rs`

### 3.2 生命周期健壮性
1. runtime 构建失败：
   - 新增 runtime factory 路径，构建失败时立即映射为 `AppError::Runtime`，并写入 `SyncStatus::Error`。
2. engine 失败：
   - worker 内同步引擎错误统一落为 `SyncStatus::Error` + `last_error`。
3. worker panic：
   - `stop_sync` 与 `reap_finished_worker` 都会 `join` 并解析 panic payload，映射为 `AppError::Runtime`（或状态 `Error`）。
4. 异常后可重启：
   - `start_sync` 前先 `reap_finished_worker`，清理已退出 worker，允许再次启动。

涉及文件：
1. `/Users/zero/study/rust/nooboard/crates/nooboard-app/src/service.rs`

### 3.3 阶段 3 语义保持
1. 远端事件处理保持“先判重，再决定是否 set”：
   - `mark_seen_event(...)` 返回非首次则直接 `return Ok(())`。
   - 首次事件再比较 `latest_content()` 决定是否 `write_text(...)`。
2. 该路径未被本轮改动破坏。

涉及文件：
1. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/engine.rs`

## 4. 测试覆盖结果（`nooboard-app`）
`cargo test -p nooboard-app`：通过（10 passed, 0 failed）。

关键新增/增强测试：
1. `stop_sync_when_not_running_is_idempotent`
2. `runtime_build_failure_is_mapped_and_sync_can_restart`
3. `sync_worker_panic_is_mapped_to_runtime_error`
4. `sync_can_restart_after_engine_error_or_stop`
5. `last_event_at_updates_on_status_changes`
6. `sync_status_exposes_connected_peers_path`

原有回归测试保持通过：
1. `list_history_supports_keyword_filter`
2. `set_clipboard_maps_backend_write_error`
3. `start_sync_rejects_duplicate_start`
4. `sync_status_switches_between_start_and_stop`

## 5. 命令验收记录（本轮必跑）
执行日期：2026-02-23

1. `cargo check -p nooboard-app`：通过
2. `cargo test -p nooboard-app`：通过（10 passed）
3. `cargo check --workspace`：通过
4. `cargo run -p nooboard-cli -- get`：通过（成功打印当前剪贴板文本）
5. `cargo run -p nooboard-cli -- set "stage4-1-smoke"`：通过（`clipboard updated`）
6. `cargo run -p nooboard-cli -- history --limit 5`：通过（成功打印最近 5 条历史）
7. `cargo run -p nooboard-cli -- watch`：通过（成功启动并 `Ctrl+C` 正常退出）
8. `cargo run -p nooboard-cli -- sync --device-id stage4-1-smoke --listen 127.0.0.1:0 --token dev-token --no-mdns`：通过（成功启动并 `Ctrl+C` 正常退出）

补充说明（不阻塞验收）：
1. 若干 `cargo run` 出现 cargo 全局缓存自动清理权限告警（`Permission denied`），不影响构建、命令执行与退出码，均为 `0`。

## 6. Stage4-1 DoD 对照
1. `nooboard-app` 稳定导出（`AppService/AppServiceImpl/SyncStartConfig/SyncState/SyncStatus/AppError`）：通过
2. `list_history/set_clipboard/start_sync/stop_sync/sync_status` 契约文档化并覆盖核心分支：通过
3. `cargo test -p nooboard-app`：通过
4. `cargo check --workspace`：通过
5. CLI 五命令烟雾回归：通过
6. 未完成项收敛到“可接受剩余项”：通过（当前无阻塞剩余项）

## 7. 收口结论
`stage4-1` 已完成，可进入 `stage4-2`（`nooboard-gui`）。
