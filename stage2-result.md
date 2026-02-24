# nooboard Stage2 开发总结报告

日期：2026-02-24

## 1. 范围与基线
1. 本次开发严格按 `stage2.md` 推进，仅覆盖 Stage2。
2. 保持 Stage3+ crate 删除基线，不恢复 `nooboard-sync` / `nooboard-app` / `nooboard-gui`。
3. Stage1 基线命令 `get` / `set` / `watch` 代码路径保留，并在 CLI 上继续可构建。

## 2. 主要完成项
1. 重建 `nooboard-storage` 并接回 workspace。
2. 依赖统一收敛到根 `Cargo.toml` 的 `[workspace.dependencies]`，子 crate 使用 `workspace = true`。
3. 实现 Stage2 配置模型：
   - `db_root` + `schema_version` 版本目录规则
   - `retain_old_versions` 清理策略
   - lifecycle 参数校验（`history_window_days` / `dedup_window_days` / `gc_every_inserts` / `gc_batch_size`）
4. 实现 SQL 外置加载：
   - `sql/bootstrap/schema.sql`
   - `sql/queries/*.sql`
   - `nooboard-storage/src` 中不内嵌 SQL 语句
5. 实现 repository 核心能力：
   - `init_storage()`
   - `append_local_text(...) -> bool`
   - `list_history(limit)`
   - `search_history(limit, keyword)`
   - `run_gc_if_needed(now_ms)`
6. 事件状态改为枚举驱动：`EventState::{Active, Tombstone}`，SQL 中 `state` 改为参数绑定。
7. CLI 完成 Stage2 接入：
   - 恢复全局 `--config`
   - 新增 `history` 命令（支持 `--limit` / `--keyword`）
   - `watch` 在输出事件同时写入 Stage2 存储
8. 更新运行配置与脚本：
   - `configs/dev.toml`
   - `configs/prod.toml`
   - `scripts/reset_db.sh`（按版本目录重建 DB）

## 3. 关键数据与生命周期实现
1. 单表 `events`：`event_id`、`origin_device_id`、`created_at_ms`、`applied_at_ms`、`content`、`state`。
2. 生命周期：
   - 超过 `history_window_days`：`active -> tombstone`（内容清空）
   - 超过 `dedup_window_days`：删除 tombstone
3. 版本目录：`{db_root}/{schema_version}/nooboard.db`。
4. `retain_old_versions = 0` 时仅保留当前版本目录。

## 4. 变更文件
1. Workspace/依赖：
   - `Cargo.toml`
   - `Cargo.lock`
2. Stage2 存储：
   - `crates/nooboard-storage/Cargo.toml`
   - `crates/nooboard-storage/src/lib.rs`
   - `crates/nooboard-storage/src/error.rs`
   - `crates/nooboard-storage/src/config.rs`
   - `crates/nooboard-storage/src/model.rs`
   - `crates/nooboard-storage/src/sql_catalog.rs`
   - `crates/nooboard-storage/src/repository.rs`
3. SQL：
   - `sql/bootstrap/schema.sql`
   - `sql/queries/insert_event.sql`
   - `sql/queries/select_latest_active_content.sql`
   - `sql/queries/list_history.sql`
   - `sql/queries/search_history.sql`
   - `sql/queries/gc_mark_tombstone.sql`
   - `sql/queries/gc_delete_expired_tombstone.sql`
   - 删除旧文件：`sql/schema.sql`
4. CLI/配置/脚本：
   - `crates/nooboard-cli/Cargo.toml`
   - `crates/nooboard-cli/src/main.rs`
   - `configs/dev.toml`
   - `configs/prod.toml`
   - `scripts/reset_db.sh`
5. 保持 Stage3+ 删除基线（未恢复相关 crates）：
   - `crates/nooboard-sync/*`
   - `crates/nooboard-app/*`
   - `crates/nooboard-gui/*`

## 5. 验证结果
1. `cargo check`：通过。
2. `cargo test -p nooboard-storage`：通过（7/7）。
3. `cargo run -p nooboard-cli -- --config configs/dev.toml history --limit 3`：通过（空库返回 `no history records`）。
4. `bash scripts/reset_db.sh`：通过（重建 `.../.dev-data/v0.1.0/nooboard.db`）。
5. 代码检索：未发现 `clipboard_history` / `sync_seen_events` / `schema_path` 残留。

## 6. DoD 对照
1. Stage1 不回归（`get` / `set` / `watch` 命令仍保留）：通过。
2. Stage2 `history` 可查询：通过。
3. 无 Stage3 旧字段残留：通过。
4. `nooboard-storage/src` 无 SQL 字面量：通过。
5. `retain_old_versions=0` 仅保留当前目录：通过（单测覆盖）。
6. 生命周期参数受 TOML 控制：通过。
7. `cargo check`：通过。
8. `cargo test -p nooboard-storage`：通过。

## 7. 结论
Stage2 已按计划完成并可作为当前稳定基线。
