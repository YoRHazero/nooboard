# nooboard 阶段 4 开发计划（GUI：gpui + gpui-component）

## 1. 阶段目标
在阶段 1~3 CLI 能力基础上，交付可运行 GUI，并通过应用层服务解耦 UI 与底层实现。

阶段 4 目标：
1. GUI 可浏览历史、搜索、点击回填。
2. GUI 可启动/停止同步并展示状态。
3. GUI 不直接访问 SQL，不直接依赖平台细节 API。
4. 保持 CLI 命令 `get/set/watch/history/sync` 可用且无回归。

## 2. 范围与非目标

### 范围
1. 新增 `nooboard-app`（应用服务层）。
2. 新增 `nooboard-gui`（GPUI 前端）。
3. GUI 最小功能闭环：
   - 历史列表（倒序）
   - 搜索过滤
   - 点击回填剪切板
   - 同步状态与启停按钮

### 非目标
1. 图片/文件类型历史与同步。
2. 账号体系与云同步。
3. E2E 加密。
4. 多窗口复杂交互与主题系统。

## 3. 架构与边界

### 3.1 分层
1. `nooboard-gui`：仅负责 UI、交互、状态渲染。
2. `nooboard-app`：聚合业务能力（history/search/set/sync control/status）。
3. `nooboard-storage` / `nooboard-platform` / `nooboard-sync`：底层实现。

### 3.2 关键约束
1. GUI 不写 SQL。
2. GUI 不直接调用平台底层 API。
3. 同步语义沿用阶段 3（判重优先）。

## 4. 应用层接口草案（阶段 4 实现目标）

在 `nooboard-app` 中定义：

1. `AppService`（trait）
- `list_history(limit: usize, keyword: Option<&str>) -> Result<Vec<ClipboardRecord>, AppError>`
- `set_clipboard(text: &str) -> Result<(), AppError>`
- `start_sync(config: SyncStartConfig) -> Result<(), AppError>`
- `stop_sync() -> Result<(), AppError>`
- `sync_status() -> SyncStatus`

2. `SyncStartConfig`
- `device_id: String`
- `listen: SocketAddr`
- `token: String`
- `peers: Vec<SocketAddr>`
- `mdns_enabled: bool`

3. `SyncStatus`
- `state: Stopped | Starting | Running | Stopping | Error`
- `listen: Option<SocketAddr>`
- `connected_peers: usize`（阶段 4 可先占位）
- `last_error: Option<String>`
- `last_event_at: Option<i64>`

## 5. 里程碑与任务拆解（A-H）

### 任务 A：crate 骨架（M1）
目标：workspace 可编译。

操作：
1. 新增 `crates/nooboard-app`。
2. 新增 `crates/nooboard-gui`。
3. 接入 workspace 与依赖。

产出：
1. `cargo check --workspace` 通过。

### 任务 B：`nooboard-app` 基础实现（M1）
目标：先打通服务层 API。

操作：
1. 定义 `AppError` 与 `AppService`。
2. 实现 `AppServiceImpl`：
   - history 查询（复用 storage）
   - 文本回填（复用 platform）
   - sync 启停（复用 sync engine，后台 task）
3. 最小线程安全状态容器（`Arc<Mutex<SyncStatus>>` 或同等方案）。

产出：
1. `cargo test -p nooboard-app` 至少覆盖 history 过滤与状态切换。

### 任务 C：GUI 主窗口骨架（M2）
目标：可启动、可渲染、可退出。

操作：
1. 主窗口布局：
   - 左：历史列表
   - 顶：搜索栏
   - 右：同步设置与状态
2. 初始化服务实例并加载首屏数据。

产出：
1. `cargo run -p nooboard-gui` 可打开窗口。

### 任务 D：历史列表与搜索（M2）
目标：完成可用历史浏览。

操作：
1. 查询最近历史并展示。
2. 搜索框输入触发过滤（可加 200ms debounce）。
3. 空态/错误态明确展示。

产出：
1. 搜索结果准确，响应及时。

### 任务 E：点击回填剪切板（M3）
目标：完成主业务闭环。

操作：
1. 列表项点击调用 `set_clipboard`。
2. 成功/失败反馈到 UI。
3. 可选：回填后刷新历史。

产出：
1. 点击历史文本可立即回填系统剪切板。

### 任务 F：同步控制与状态面板（M3）
目标：GUI 可控同步。

操作：
1. 表单输入：`device_id/listen/token/peers/mdns`。
2. 按钮：`Start Sync` / `Stop Sync`。
3. 状态展示：当前状态、监听地址、最近错误。

产出：
1. 不依赖 CLI 可从 GUI 启停同步。

### 任务 G：稳定性与回归（M4）
目标：降低 GUI 并发风险。

操作：
1. 后台任务与 UI 线程解耦。
2. 关闭窗口时优雅停止同步任务。
3. 回归 CLI（`get/set/watch/history/sync`）行为。

产出：
1. 长时间运行无明显卡顿/泄漏。

### 任务 H：测试与验收（M4）
目标：形成阶段4可交付结论。

操作：
1. 自动化：
   - `cargo check --workspace`
   - `cargo test -p nooboard-app`
   - `cargo test -p nooboard-sync`
2. 手工：
   - GUI 启动
   - 搜索
   - 点击回填
   - 同步启停

产出：
1. 阶段4完成报告（对照 DoD）。

## 6. 建议文件结构（阶段 4）

1. `/Users/zero/study/rust/nooboard/crates/nooboard-app/src/lib.rs`
2. `/Users/zero/study/rust/nooboard/crates/nooboard-app/src/error.rs`
3. `/Users/zero/study/rust/nooboard/crates/nooboard-app/src/service.rs`
4. `/Users/zero/study/rust/nooboard/crates/nooboard-app/src/status.rs`
5. `/Users/zero/study/rust/nooboard/crates/nooboard-gui/src/main.rs`
6. `/Users/zero/study/rust/nooboard/crates/nooboard-gui/src/app.rs`
7. `/Users/zero/study/rust/nooboard/crates/nooboard-gui/src/views/history_view.rs`
8. `/Users/zero/study/rust/nooboard/crates/nooboard-gui/src/views/sync_view.rs`

## 7. DoD（阶段 4）

1. GUI 可启动并展示历史列表。  
2. 历史搜索可用。  
3. 点击历史项可回填剪切板。  
4. GUI 可启动/停止同步。  
5. 同步状态在 GUI 可见（运行态与错误态）。  
6. CLI 现有命令无回归。  
7. `cargo check --workspace` 通过。  

## 8. 风险与预防性验证

1. 风险：UI 卡顿。  
验证：开启 sync + 连续搜索输入 + 列表滚动，观察交互延迟。

2. 风险：状态不同步。  
验证：注入错误 token、停止 peer、重启 peer，检查 GUI 状态切换是否及时。

3. 风险：阶段3链路不稳导致 GUI 假阳性。  
验证：先执行 `/Users/zero/study/rust/nooboard/stage3-validation.md`，补齐 DoD 证据后再推进 GUI。

## 9. 建议执行顺序（下一轮）

1. 先做任务 A+B（服务层先于界面层）。
2. 再做任务 C+D+E（先浏览和回填闭环）。
3. 最后做 F+G+H（同步控制、稳定性、验收）。
