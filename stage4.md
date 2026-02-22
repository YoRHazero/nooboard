# nooboard 阶段 4 开发计划（GUI：gpui + gpui-component）

## 1. 阶段目标
在阶段 1~3 CLI 能力基础上，交付可运行的桌面 GUI，用于历史浏览、搜索、回填剪切板和同步状态展示。

阶段 4 完成标准：
1. GUI 可启动并展示最近剪切板历史。
2. 支持历史搜索与点击回填剪切板。
3. 展示同步状态（监听地址、连接数、最近同步时间、错误提示）。
4. GUI 仅调用应用层服务，不直接访问 SQL 与平台细节。
5. CLI 现有命令（`get/set/watch/history/sync`）保持可用。

## 2. 范围与非目标

### 范围
1. 引入 `gpui` + `gpui-component` 构建桌面界面。
2. 新增应用层编排（建议 crate：`nooboard-app`），封装 storage/platform/sync 能力。
3. GUI 提供：
   - 历史列表（倒序）
   - 搜索框（按文本过滤）
   - 点击历史项回填剪切板
   - 同步状态面板
4. 支持从 GUI 启停同步任务（复用阶段 3 `SyncEngine`）。

### 非目标
1. 不做图片/文件历史与同步。
2. 不做账户系统与云端服务。
3. 不做端到端加密。
4. 不做复杂主题系统（先交付功能闭环）。

## 3. 设计原则
1. 分层明确：GUI -> App Service -> Storage/Platform/Sync。
2. GUI 不直接写 SQL，不直接调用 ObjC API。
3. 同步状态通过应用层状态对象暴露，避免 UI 直接操控网络细节。
4. 保持阶段 3 的关键语义不变：远端先判重再决定 `set`。

## 4. 任务拆解（执行顺序）

### 任务 A：新增 GUI 与应用层 crate 骨架
目标：建立阶段 4 承载结构。

操作：
1. 新增 crate：`nooboard-app`（应用服务层）。
2. 新增 crate：`nooboard-gui`（GPUI 前端）。
3. 接入 workspace 与基础依赖。

完成标准：
1. `cargo check --workspace` 通过。

### 任务 B：应用服务接口定义
目标：将 UI 与底层能力解耦。

操作：
1. 在 `nooboard-app` 定义 `AppService`（示例能力）：
   - `list_history(limit, keyword)`
   - `set_clipboard(text)`
   - `start_sync(config)`
   - `stop_sync()`
   - `sync_status()`
2. 提供默认实现，内部组合 `ClipboardBackend + ClipboardRepository + SyncEngine`。

完成标准：
1. GUI 通过 `AppService` 完成功能，不直接依赖 SQL 与平台实现细节。

### 任务 C：GUI 主窗口与页面骨架
目标：先可运行，再逐步补功能。

操作：
1. 搭建窗口布局：左侧历史列表，右侧详情/状态区。
2. 增加顶部搜索框与刷新按钮。
3. 增加底部状态栏（数据库路径、同步状态）。

完成标准：
1. GUI 可启动并稳定渲染基本布局。

### 任务 D：历史列表与搜索
目标：实现核心浏览能力。

操作：
1. 读取最近历史并列表展示（时间 + 单行文本）。
2. 按关键字过滤内容。
3. 支持空态与错误态提示。

完成标准：
1. 可在 GUI 中查看并搜索历史。

### 任务 E：点击历史项回填剪切板
目标：闭环“查看 -> 复用”。

操作：
1. 列表项点击后调用 `set_clipboard(text)`。
2. 成功/失败反馈到 UI。
3. 与阶段 2 去重策略保持一致（避免无意义重复写入）。

完成标准：
1. 点击任意历史项后可回填系统剪切板。

### 任务 F：同步状态与控制
目标：在 GUI 中可观测并控制同步。

操作：
1. 新增同步设置区（device_id/listen/token/peers/mdns 开关）。
2. 支持“启动同步/停止同步”。
3. 展示状态：运行中/停止、最近错误、最近同步时间、连接计数（若可获得）。

完成标准：
1. 不依赖 CLI 即可从 GUI 控制同步任务。

### 任务 G：稳定性与并发处理
目标：避免 UI 卡顿与资源泄漏。

操作：
1. 将 I/O 与网络任务放在后台异步任务。
2. UI 通过事件或状态订阅刷新。
3. 处理窗口关闭时的同步任务优雅退出。

完成标准：
1. 长时间运行无明显卡顿与泄漏。

### 任务 H：测试与验收
目标：确保阶段 4 可交付。

操作：
1. 单元测试：`nooboard-app` 服务层。
2. 基础集成测试：历史查询/回填/同步启停。
3. 手工验证：GUI 启动、搜索、回填、同步状态展示。

完成标准：
1. `cargo check --workspace` 通过。
2. `cargo test -p nooboard-app`（若创建）通过。
3. CLI 现有命令回归通过。

## 5. 阶段 4 建议文件职责

1. `/Users/zero/study/rust/nooboard/crates/nooboard-app/src/lib.rs`  
   应用服务接口与状态模型。

2. `/Users/zero/study/rust/nooboard/crates/nooboard-app/src/service.rs`  
   服务实现（封装 storage/platform/sync）。

3. `/Users/zero/study/rust/nooboard/crates/nooboard-gui/src/main.rs`  
   GUI 入口与应用生命周期。

4. `/Users/zero/study/rust/nooboard/crates/nooboard-gui/src/views/history_view.rs`  
   历史列表与搜索界面。

5. `/Users/zero/study/rust/nooboard/crates/nooboard-gui/src/views/sync_view.rs`  
   同步控制与状态界面。

## 6. DoD（阶段 4）

1. GUI 能正常启动并展示历史列表。  
2. 搜索可过滤历史内容。  
3. 点击历史项可回填剪切板。  
4. GUI 可启动/停止同步并展示状态。  
5. CLI 既有命令保持可用。  
6. `cargo check --workspace` 通过。  

## 7. 风险与缓解

1. 风险：GUI 与异步任务耦合导致 UI 卡顿。  
缓解：严格分离 UI 线程与后台任务，采用消息驱动状态更新。

2. 风险：同步状态难以统一展示。  
缓解：在 `nooboard-app` 定义稳定 `SyncStatus` 模型，GUI 只消费该模型。

3. 风险：阶段 3 联调证据不足影响 GUI 同步展示可靠性。  
缓解：阶段 4 开始前先补一轮两节点/三节点联调脚本与记录。

## 8. 建议验证命令（阶段 4 完成时）

1. `cargo check --workspace`  
2. `cargo test -p nooboard-app`  
3. `cargo run -p nooboard-gui`  
4. 回归：`cargo run -p nooboard-cli -- history --limit 20`  
5. 回归：`cargo run -p nooboard-cli -- sync --device-id dev-a --listen 0.0.0.0:8787 --token dev-token`  
