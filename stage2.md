# nooboard 阶段 2 完成报告（SQLite 历史记录，CLI 可运行）

## 1. 阶段目标达成情况
阶段 2 目标已完成：在阶段 1 剪切板能力基础上接入 SQLite 持久化，`watch` 可入库，`history` 可查询最近记录，CLI 保持可运行。

## 2. 实际范围与非目标

### 实际范围
1. 仅支持 UTF-8 文本历史记录。
2. 新增 `nooboard-storage` crate，CLI 通过 repository 访问数据库。
3. 数据库 schema 由 `/Users/zero/study/rust/nooboard/sql/schema.sql` 统一管理。
4. 配置通过 TOML 管理：
   - `/Users/zero/study/rust/nooboard/configs/dev.toml`
   - `/Users/zero/study/rust/nooboard/configs/prod.toml`
5. 开发环境数据库路径为 `/Users/zero/study/rust/nooboard/.dev-data/nooboard.db`。
6. 新增本地去重策略：连续重复文本不重复入库。

### 非目标（本阶段未接入）
1. 跨设备同步（阶段 3）。
2. GUI（阶段 4）。
3. 非文本类型（图片/文件等）。
4. 数据库 migration 框架。

## 3. A-G 执行结果（实际完成效果）

### 任务 A：扩展 workspace 与存储 crate
完成结果：
1. workspace 新增成员 `nooboard-storage`。
2. 新增 SQL 文件 `/Users/zero/study/rust/nooboard/sql/schema.sql`。
3. `nooboard-storage` 已被 `nooboard-cli` 链接使用。
4. `cargo check --workspace` 通过。

### 任务 B：定义存储模型与 repository 接口
完成结果：
1. 定义 `ClipboardRecord { id, content, captured_at }`。
2. 定义 `ClipboardRepository` 接口：
   - `init_schema()`
   - `insert_text_event(text, captured_at)`
   - `list_recent(limit)`
3. 增加 `StorageError`，并在 CLI 映射到 `NooboardError::Storage`。
4. CLI 未出现 SQL 字符串，数据库读写全部在 `nooboard-storage`。

### 任务 C：数据库初始化与 schema 执行
完成结果：
1. 运行时通过配置读取 `db_path` 与 `schema_path`。
2. 初始化流程：
   - 自动创建 DB 父目录。
   - 打开/创建 SQLite 文件。
   - 读取并执行 `schema.sql`。
3. 删除数据库后，可通过脚本重建并继续使用。

### 任务 D：watch 入库链路接入
完成结果：
1. `watch` 监听事件后调用 `insert_text_event` 入库。
2. 控制台输出行为保持不变。
3. `Ctrl+C` 优雅退出流程保持不变。

### 任务 E：history 命令实现
完成结果：
1. 新增 `history` 命令。
2. 支持 `--limit <n>`，默认 `20`。
3. 输出格式为 `[timestamp] text`，按时间倒序。
4. 空结果提示：`no clipboard history records`。
5. 新增 CLI 全局参数 `--config <path>`，可切换配置文件。

### 任务 F：配置与脚本补齐
完成结果：
1. 新增 `configs/dev.toml` 与 `configs/prod.toml`。
2. 新增脚本 `/Users/zero/study/rust/nooboard/scripts/reset_db.sh`，按 `dev.toml` 读取 `db_path` 并重建数据库。
3. `.gitignore` 已忽略 `.dev-data/`（覆盖 `.dev-data/nooboard.db`）。

### 任务 G：测试与自检
完成结果：
1. `nooboard-storage` 单元测试已覆盖：
   - schema 初始化
   - recent 倒序查询
   - 连续重复文本去重
2. 手工链路验证通过：`set` -> `watch` 入库 -> `history` 查询。

## 4. 阶段 2 文件职责（实际）

1. `/Users/zero/study/rust/nooboard/sql/schema.sql`  
   阶段 2 数据库结构定义（`clipboard_history` + 索引）。

2. `/Users/zero/study/rust/nooboard/configs/dev.toml`  
   开发配置（`db_path`、`schema_path`）。

3. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/config.rs`  
   配置解析（TOML -> `AppConfig`）。

4. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/repository.rs`  
   SQLite 初始化、插入、查询、连续重复去重实现。

5. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/model.rs`  
   存储模型 `ClipboardRecord`。

6. `/Users/zero/study/rust/nooboard/crates/nooboard-cli/src/main.rs`  
   `watch` 入库接入、`history` 命令、`--config` 参数支持。

7. `/Users/zero/study/rust/nooboard/scripts/reset_db.sh`  
   开发环境数据库重建脚本。

## 5. 实际验证记录

1. `cargo check --workspace`：通过。  
2. `cargo test -p nooboard-storage`：通过（3 个测试通过）。  
3. `cargo run -p nooboard-cli -- set "stage2-doc-seed"`：成功写入剪切板。  
4. `cargo run -p nooboard-cli -- watch` + 多次 `set "stage2-dedup-check"`：watch 能收到变化事件。  
5. `cargo run -p nooboard-cli -- history --limit 20`：可查询最近记录；连续重复内容只保留一条。  
6. `./scripts/reset_db.sh`：可删除并重建数据库。  

说明：部分 `cargo run` 过程中出现 cargo 全局缓存清理权限警告（`Permission denied`），不影响构建与命令执行结果。

## 6. DoD 对照结论

1. 首次运行后自动创建 `.dev-data/nooboard.db` 并初始化 schema：通过。  
2. `watch` 监听变化可写入 SQLite：通过。  
3. `history --limit 20` 可查询最近记录：通过。  
4. 删除数据库后可恢复并继续使用：通过。  
5. `cargo check --workspace`：通过。  
6. CLI 无直接 SQL 耦合：通过。  

## 7. 阶段 2 收尾结论
阶段 2 已完成，可进入阶段 3（跨设备同步 MVP，CLI 可运行）。
