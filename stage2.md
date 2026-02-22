# nooboard 阶段 2 开发计划（SQLite 历史记录，CLI 可运行）

## 1. 阶段目标
在阶段 1 基础上增加本地持久化能力：将剪切板变化写入 SQLite，并可通过 CLI 查询历史记录。

阶段 2 结束时应满足：
1. 自动初始化数据库并应用 `sql/schema.sql`。
2. `watch` 命令在变化时写入数据库。
3. 新增 `history` 命令查询最近记录。

## 2. 范围与非目标

### 范围
1. 仅处理文本类型历史（UTF-8）。
2. 数据库路径（开发环境）固定为：`/Users/zero/study/rust/nooboard/.dev-data/nooboard.db`。
3. 开发策略采用“重建优先”（可删除重建，不做 migration）。
4. CLI 增加 `history` 命令，按时间倒序查询。

### 非目标
1. 不做跨设备同步。
2. 不做 GUI。
3. 不做复杂索引优化和大规模性能调优。
4. 不做历史版本迁移框架。

## 3. 设计原则
1. CLI 不直接写 SQL，所有数据库读写都走 `nooboard-storage` repository。
2. 平台剪切板接口与存储层解耦，watch 事件通过应用层串联。
3. 先保证可运行与可验证，再考虑扩展字段和复杂查询。

## 4. 任务拆解（按执行顺序）

### 任务 A：扩展 workspace 与存储 crate
目标：引入阶段 2 所需工程结构。

操作：
1. 新增 crate：`nooboard-storage`。
2. 新增 SQL 文件：`/Users/zero/study/rust/nooboard/sql/schema.sql`。
3. 在 workspace 注册新成员与依赖（建议 `rusqlite`、`time` 或 `chrono` 二选一，保持最小依赖）。
4. 新增目录：`/Users/zero/study/rust/nooboard/.dev-data/`（运行时自动创建）。

完成标准：
1. `cargo check --workspace` 通过。
2. `nooboard-storage` 可被 `nooboard-cli` 链接。

### 任务 B：定义存储模型与 repository 接口
目标：明确存储边界，避免 CLI 直接操作 SQL。

操作：
1. 在 `nooboard-storage` 定义记录模型（建议 `ClipboardRecord`）：
   - `id`
   - `content`
   - `captured_at`（毫秒时间戳或 RFC3339）
2. 定义 repository 接口：
   - `init_schema()`
   - `insert_text_event(text, captured_at)`
   - `list_recent(limit)`
3. 增加存储层错误类型并映射到 `NooboardError`。

完成标准：
1. CLI 只调用 repository API，不出现 SQL 字符串。

### 任务 C：数据库初始化与 schema 执行
目标：启动时可自动完成建库建表。

操作：
1. 在 `sql/schema.sql` 维护当前阶段唯一建表脚本。
2. 实现初始化流程：
   - 确保 `.dev-data` 存在。
   - 打开/创建 `nooboard.db`。
   - 读取并执行 `schema.sql`。
3. 落地“重建优先”机制（推荐二选一）：
   - `--rebuild-db` 显式删除并重建。
   - 或开发模式默认重建。

完成标准：
1. 删除数据库后再次运行命令能自动重建并可用。

### 任务 D：watch 入库链路接入
目标：让监听到的每条剪切板文本变化落盘。

操作：
1. 在 CLI 的 `watch` 事件消费处调用 `insert_text_event`。
2. 仅在文本存在时入库；保留现有控制台输出。
3. 保持 Ctrl+C 优雅退出流程不变。

完成标准：
1. `watch` 运行期间触发多次剪切板变化后，数据库中可查询到对应记录。

### 任务 E：history 命令实现
目标：提供历史查询入口。

操作：
1. 在 `nooboard-cli` 新增命令：`history`。
2. 参数建议：`--limit <n>`（默认 20）。
3. 输出建议：`时间戳 + 文本`，按时间倒序。
4. 空结果时输出清晰提示。

完成标准：
1. `cargo run -p nooboard-cli -- history --limit 20` 可返回最近记录。

### 任务 F：配置与脚本补齐
目标：提升阶段 2 开发效率。

操作：
1. 增加脚本：`/Users/zero/study/rust/nooboard/scripts/reset_db.sh`（删除并重建 DB）。
2. 在 `.gitignore` 确保忽略 `.dev-data/nooboard.db`。
3. 更新 README 的阶段 2 命令说明（可选但建议）。

完成标准：
1. 本地可一键重置数据库并再次验证命令。

### 任务 G：测试与自检
目标：保证核心路径稳定。

操作：
1. 为 `nooboard-storage` 增加最小单元/集成测试：
   - schema 初始化
   - 插入记录
   - recent 查询顺序
2. 关键命令链路手工验证：
   - `set` -> `watch` 入库 -> `history` 读出。

完成标准：
1. `cargo test -p nooboard-storage` 通过。
2. 核心手工验证链路通过。

## 5. 文件职责（阶段 2）

1. `/Users/zero/study/rust/nooboard/sql/schema.sql`  
   当前阶段唯一数据库结构定义。

2. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/lib.rs`  
   存储层导出入口。

3. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/repository.rs`  
   数据库初始化、插入、查询实现。

4. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/model.rs`  
   数据库存储模型。

5. `/Users/zero/study/rust/nooboard/crates/nooboard-cli/src/main.rs`  
   `watch` 入库接入、`history` 命令新增与输出。

6. `/Users/zero/study/rust/nooboard/scripts/reset_db.sh`  
   开发环境数据库重建脚本。

## 6. 验收清单（DoD）

1. 首次运行后自动创建 `.dev-data/nooboard.db` 并完成 schema 初始化。
2. `cargo run -p nooboard-cli -- watch` 监听变化时可将文本写入 SQLite。
3. `cargo run -p nooboard-cli -- history --limit 20` 可查询最近记录。
4. 删除数据库后可按“重建优先”策略恢复并继续使用。
5. `cargo check --workspace` 通过。
6. CLI 无直接 SQL 耦合（通过 `nooboard-storage` 调用）。

## 7. 风险与缓解

1. 风险：`watch` 高频变化导致频繁写库。  
缓解：先保证正确性，后续再做批量写或节流。

2. 风险：数据库文件路径/权限导致初始化失败。  
缓解：启动时显式创建目录并输出可读错误。

3. 风险：重复内容记录过多。  
缓解：阶段 2 先全量记录，去重策略留到后续阶段评估。

## 8. 建议验证命令（阶段 2 完成时）

1. `cargo check --workspace`
2. `cargo run -p nooboard-cli -- set "stage2-seed"`
3. `cargo run -p nooboard-cli -- watch`
4. `cargo run -p nooboard-cli -- history --limit 20`
5. `cargo test -p nooboard-storage`
