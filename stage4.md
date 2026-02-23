# nooboard 阶段 4 开发策略（拆分版）

## 1. 阶段 4 总目标
阶段 4 只做两件事：
1. 先完成 `nooboard-app`（应用服务层）并稳定接口。
2. 再完成 `nooboard-gui`（GPUI 前端）并接入 `nooboard-app`。

本阶段明确拆分为：
1. `stage4-1`：开发与补齐 `nooboard-app`
2. `stage4-2`：开发 `nooboard-gui`

## 2. 全局约束（4-1/4-2 全部必须满足）
1. GUI 不直接写 SQL，数据库访问只经 `nooboard-storage`（通过 `nooboard-app`）。
2. GUI 不直接调用平台底层 API。
3. 同步语义保持阶段 3：远端事件“先判重，再决定是否 set”。
4. CLI 命令 `get/set/watch/history/sync` 不得回归。
5. 每个子步骤结束后至少保持可编译。

## 3. Stage4-1（`nooboard-app`）范围与完成定义

### 3.1 目标
交付稳定、可测试、可被 GUI 直接复用的应用服务层，作为阶段 4 的唯一业务入口。

### 3.2 范围
1. `AppService` 接口与实现稳定化。
2. 历史查询、剪切板回填、同步启停、同步状态查询四类能力。
3. 错误模型统一（`AppError`）及底层错误映射。
4. 并发与生命周期：同步 worker 的启动、停止、异常收敛。
5. 单测与最小回归验证。

### 3.3 Stage4-1 DoD
1. `nooboard-app` 提供并稳定导出：
   - `AppService` / `AppServiceImpl`
   - `SyncStartConfig`
   - `SyncState` / `SyncStatus`
   - `AppError`
2. `list_history/set_clipboard/start_sync/stop_sync/sync_status` 行为文档化且测试覆盖核心分支。
3. `cargo test -p nooboard-app` 通过。
4. `cargo check --workspace` 通过。
5. CLI 五命令烟雾回归通过（至少一次完整记录）。
6. `stage4-1.md` 中未完成项收敛到“可接受剩余项”（不阻塞 GUI 开发）。

## 4. Stage4-2（`nooboard-gui`）范围与完成定义

### 4.1 启动前置条件
只有当 `stage4-1` DoD 全部满足后，才能进入 `stage4-2`。

### 4.2 目标
基于 `nooboard-app` 完成 GUI 闭环，不在 GUI 层引入业务与存储耦合。

### 4.3 范围
1. 窗口与基础布局。
2. 历史列表与搜索。
3. 点击历史项回填剪切板。
4. 同步启停控制与状态展示。
5. 关闭行为与稳定性处理（不导致异常退出、不破坏同步生命周期）。

### 4.4 Stage4-2 DoD
1. GUI 可启动并正常渲染。
2. 历史搜索可用。
3. 点击历史项可回填剪切板。
4. GUI 可启动/停止同步。
5. 同步状态在 GUI 可见（运行态/错误态）。
6. `cargo check -p nooboard-gui`、`cargo check --workspace` 通过。
7. CLI 五命令再次回归通过。

## 5. 阶段 4 总体验收（合并条件）
阶段 4 完成条件 = `stage4-1 DoD` + `stage4-2 DoD` 全部通过。

## 6. 文档分工
1. `/Users/zero/study/rust/nooboard/stage4-1.md`
   - 只记录 `nooboard-app` 的实现、缺口与验证。
2. `/Users/zero/study/rust/nooboard/stage4-2.md`
   - 只记录 `nooboard-gui` 的实现、缺口与验证。
