# nooboard Stage2 开发计划（当前基线）

## 1. 当前基线
1. 当前 workspace 基线为 `nooboard-core`、`nooboard-platform`、`nooboard-platform-macos`。
2. 本计划只面向 Stage2 重建，不包含任何 Stage3+ 兼容或迁移工作。
3. Stage1 基线必须保持可用：`get` / `set` / `watch`。

## 2. Stage2 目标
1. 从零重建 Stage2 存储能力（单表 `events`）。
2. 恢复并完成 `history` 查询能力。
3. `watch` 在输出事件的同时可落库到 Stage2 存储。
4. SQL 全部外置，Rust 代码不内嵌 SQL 字面量。
5. 不做旧库 migration，采用版本目录隔离。

## 3. 数据与生命周期设计
### 3.1 单表模型
表名：`events`

字段：
1. `event_id`：`BLOB(16)`，UUIDv7，主键。
2. `origin_device_id`：来源设备 ID。
3. `created_at_ms`：事件创建时间（毫秒）。
4. `applied_at_ms`：本机落库时间（毫秒）。
5. `content`：文本内容，tombstone 时为 `NULL`。
6. `state`：`active` 或 `tombstone`。

### 3.2 生命周期策略
1. `active -> tombstone`：超过 `history_window_days`，清空 `content` 并标记 tombstone。
2. `tombstone -> delete`：超过 `dedup_window_days`，物理删除。

### 3.3 版本目录策略
1. 不做 migration。
2. 数据库路径：`{db_root}/{STORAGE_SCHEMA_VERSION}/nooboard.db`。
3. schema 变化时 bump `nooboard-storage` 内部常量 `STORAGE_SCHEMA_VERSION`，直接新建新库。
4. `retain_old_versions = 0` 时，仅保留当前版本目录。

## 4. 配置与 SQL 方案
### 4.1 配置（TOML）
```toml
[storage]
db_root = "/Users/zero/study/rust/nooboard/.dev-data"
retain_old_versions = 0

[storage.lifecycle]
history_window_days = 7
dedup_window_days = 14
gc_every_inserts = 200
gc_batch_size = 500
```

参数约束：
1. `history_window_days >= 1`。
2. `dedup_window_days >= history_window_days`。
3. `gc_every_inserts >= 1`。
4. `gc_batch_size >= 1`。

### 4.2 SQL 外置文件
1. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/sql/bootstrap/schema.sql`
2. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/sql/queries/insert_event.sql`
3. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/sql/queries/select_latest_active_content.sql`
4. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/sql/queries/list_history.sql`
5. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/sql/queries/search_history.sql`
6. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/sql/queries/gc_mark_tombstone.sql`
7. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/sql/queries/gc_delete_expired_tombstone.sql`

代码约束：
1. `nooboard-storage/src` 不允许出现 SQL 字面量。
2. repository 仅负责 SQL 使用、参数绑定、结果映射、事务控制。

## 5. 实施步骤（按顺序）
1. 重建 `crates/nooboard-storage` crate，并加回 workspace。
2. 实现 `config.rs`（配置结构、默认值、参数校验、路径规则）。
3. 建立 SQL 目录与文件，补齐 schema 与 queries。
4. 实现 `sql_catalog.rs`（统一加载外置 SQL）。
5. 实现 `repository.rs`：
   1. `init_storage()`
   2. `append_local_text(...) -> inserted`
   3. `list_history(limit)`
   4. `search_history(limit, keyword)`
   5. `run_gc_if_needed(now_ms)`
6. 调整上层调试入口：恢复 `history`、`watch` 与配置加载链路，并将 `watch` 接入存储写入。
7. 更新 `configs/dev.toml`、`configs/prod.toml` 以匹配版本目录规则。
8. 完成测试与回归，冻结基线。

## 6. 计划内文件清单
### 6.1 新增/重建
1. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/Cargo.toml`
2. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/lib.rs`
3. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/error.rs`
4. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/config.rs`
5. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/model.rs`
6. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/repository.rs`
7. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/sql_catalog.rs`
8. `/Users/zero/study/rust/nooboard/configs/dev.toml`
9. `/Users/zero/study/rust/nooboard/configs/prod.toml`
10. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/sql/bootstrap/schema.sql`
11. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/sql/queries/*.sql`

### 6.2 修改
1. `/Users/zero/study/rust/nooboard/Cargo.toml`
2. `/Users/zero/study/rust/nooboard/configs/dev.toml`
3. `/Users/zero/study/rust/nooboard/configs/prod.toml`

## 7. 验收标准（DoD）
1. Stage1 不回归：`get` / `set` / `watch` 可用。
2. Stage2 `history` 可查询最新记录。
3. 代码中不存在 `clipboard_history` / `sync_seen_events` / `schema_path`。
4. `nooboard-storage/src` 中无 SQL 字面量，SQL 全在外置文件。
5. `retain_old_versions=0` 时仅保留当前版本目录。
6. 生命周期参数由 TOML 控制并生效。
7. `cargo check` 通过。
8. `cargo test -p nooboard-storage` 通过。
