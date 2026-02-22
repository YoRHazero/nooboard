# nooboard 开发蓝图（Blueprint）

## 1. 项目目标
nooboard 是一个使用 Rust 开发的跨设备剪切板工具，目标包含：

1. 监听并操作系统剪切板（读取 + 写入）。
2. 使用 SQLite 记录剪切板历史。
3. 支持跨设备共享剪切板内容。
4. 使用 `gpui` + `gpui-component` 构建 GUI。

约束与方向：

1. macOS 优先，且剪切板层使用较底层方案（`objc2` + `objc2-app-kit`）。
2. 开发阶段数据库采用“重建优先”策略，不做历史版本迁移。
3. 每个阶段结束时，程序应保持可运行、可验证。

## 2. 分阶段开发计划

### 阶段 1：剪切板基础能力（CLI 可运行）
目标：完成本地剪切板文本的读、写、监听。

交付：

1. `nooboard-cli get`：读取当前剪切板文本。
2. `nooboard-cli set <text>`：写入文本到剪切板。
3. `nooboard-cli watch`：监听剪切板变化并输出新文本。

关键实现：

1. 平台抽象 trait（`read_text`/`write_text`/`watch_changes`）。
2. macOS 实现层使用 `NSPasteboard` + `changeCount`。

### 阶段 2：SQLite 历史记录（CLI 可运行）
目标：把剪切板变化持久化并可查询。

交付：

1. SQLite 初始化逻辑（读取并执行 `sql/schema.sql`）。
2. `nooboard-cli watch` 在变化时入库。
3. `nooboard-cli history` 查询最近历史。

关键实现：

1. SQL 文件集中存放于 `sql/schema.sql`。
2. 数据库文件路径（开发）为 `.dev-data/nooboard.db`。
3. 开发模式重建数据库（删除 DB 后重建或执行 drop/create）。

### 阶段 3：跨设备同步 MVP（CLI 可运行）
目标：在多设备之间同步剪切板事件。

交付：

1. `nooboard-sync`（Hub/Server）。
2. `nooboard-cli` 作为同步客户端连接服务。
3. 远端内容写入本地剪切板并入本地库。

关键实现：

1. WebSocket + `serde` 消息协议。
2. 基于 `device_id + seq` 的去重。
3. 最小鉴权（token）与断线重连。

### 阶段 4：GUI（gpui + gpui-component）
目标：将核心功能图形化。

交付：

1. 历史列表浏览与搜索。
2. 点击历史项回填剪切板。
3. 同步状态展示（连接状态、最近同步时间）。

关键实现：

1. GUI 仅调用应用层，不直接依赖底层平台细节。
2. 复用阶段 1~3 的 core/storage/sync 能力。

### 阶段 5：稳定性与扩展
目标：提升可用性与可维护性。

交付：

1. 增加非文本类型支持（图片/文件引用，按优先级逐步做）。
2. 更完整测试（单测/集成测试/基础端到端）。
3. 打包发布与文档完善。

## 3. 当前草拟文件架构（参考）

```text
/Users/zero/study/rust/nooboard/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── README.md
├── blueprint.md
├── stage1.md
├── .gitignore
├── configs/
│   ├── dev.toml
│   └── prod.toml
├── sql/
│   └── schema.sql
├── .dev-data/
│   └── nooboard.db
├── scripts/
│   ├── reset_db.sh
│   └── dev_watch.sh
├── docs/
│   ├── architecture.md
│   └── sync-protocol.md
└── crates/
    ├── nooboard-core/
    ├── nooboard-platform/
    ├── nooboard-platform-macos/
    ├── nooboard-storage/
    ├── nooboard-sync/
    ├── nooboard-app/
    ├── nooboard-cli/
    ├── nooboard-gui/
    └── nooboard-tests/
```

说明：

1. `sql/schema.sql` 维护当前开发阶段唯一建表脚本。
2. `.dev-data/nooboard.db` 是开发环境运行时数据库（应加入 `.gitignore`）。
3. 阶段推进时优先补齐 `nooboard-cli`，再接入 `nooboard-gui`。

## 4. 推荐代码风格（Rust）

### 4.1 通用约定
1. 使用 `rustfmt` + `clippy`，以 CI 检查为准。
2. 文件和模块命名使用 `snake_case`，类型命名使用 `PascalCase`。
3. 避免 `unwrap/expect` 出现在可恢复路径；统一返回 `Result`。
4. 公共接口先定义 trait，再做平台实现，减少耦合。
5. 变量命名应清晰表达意图，避免过度缩写（例如 `clipboard_text` 而非 `cb_txt`）。

### 4.2 错误处理
1. 应用层统一错误类型（建议 `thiserror`）。
2. 平台错误向上转换为领域错误，不泄漏 ObjC 细节到 CLI/GUI。
3. 命令行输出用户可理解错误；日志保留调试上下文。

### 4.3 并发与异步
1. I/O 与网络使用 `tokio`。
2. 监听任务与业务处理分离，通过 channel 传递事件。
3. 避免在异步上下文中持有长生命周期锁。

### 4.4 数据与协议
1. 剪切板领域模型集中在 `nooboard-core`，禁止重复定义。
2. 同步协议显式版本字段（例如 `version`），为后续升级留入口。
3. 数据库访问通过 repository 层，不在 CLI/GUI 直接写 SQL。

### 4.5 测试与可观测性
1. 核心逻辑（去重、事件转换）写单元测试。
2. 存储和同步写集成测试。
3. 关键流程添加结构化日志（建议 `tracing`）。

## 5. 开发推进原则
1. 任何阶段新增功能前，先保证上一阶段命令仍可运行。
2. 阶段内优先“最小可用路径”，避免过早优化。
3. 所有新增模块先写接口边界，再写实现。

## 6. 项目依赖管理
1. 新增依赖请使用命令行工具（如 `cargo add`）添加依赖以保持 `Cargo.toml` 整洁。
2. 请尽量使用最新版本，如果不能使用最新版本，请说明原因。
3. 项目依赖应尽量避免版本冲突，优先使用 `cargo update` 更新依赖树。
