# nooboard-storage

文字历史与文本配置的持久化服务。上游持有可克隆的 `Storage` 句柄，通过有界请求队列与内部 runtime 通信；连接、SQL、事务和数据库迁移由后端私有实现。

本轮是破坏性重构：旧 `Database` 接口已删除，现有 core 尚未接入新接口。`files`、`secrets` 及其依赖已删除，未迁入 network；跨端文件传输和凭据管理留到 network 重构。

## 职责

- 保存、查询、删除文字历史；以完整文本相等去重，并原子执行调用者指定的保留规则。
- 保存不解释内容的 UTF-8 配置文档；多键更新/删除原子提交，多键读取来自同一快照。
- 管理请求排队、服务生命周期、数据库资源和 schema 迁移。

上游决定何时记录、来源、时间戳、保留数量与时间边界，并负责 JSON 等配置内容的序列化、版本与业务校验。storage 不观察剪贴板、不发送网络消息、不管理传输文件、证书或密钥，也不定时计算保留策略。

## 文件结构

```text
src/
├── lib.rs                    # 公共导出
├── api.rs                    # StorageService 所有者、Storage/History/Settings 请求代理
├── options.rs                # 队列配置、启动时后端选择
├── error.rs                  # 稳定错误类别，不要求调用者依赖数据库驱动
├── model/
│   ├── mod.rs                # 公共文本约束
│   ├── history.rs            # 历史请求、结果、来源、保留与查询条件
│   └── settings.rs           # 文本配置及批量变更
├── runtime/
│   ├── mod.rs                # 请求调度、停止、收尾
│   ├── request.rs            # 类型化请求与一次性回复
│   └── tests.rs              # 用假后端验证生命周期、取消和背压
└── backend/
    ├── mod.rs                # 启动工厂
    ├── contract.rs           # 与数据库无关的异步操作契约
    └── sqlite/
        ├── mod.rs            # SQLite 适配器、阻塞操作隔离
        ├── connection.rs     # 连接与 SQLite 参数
        ├── error.rs          # 驱动错误映射
        ├── history.rs        # 历史 SQL 与事务
        ├── settings.rs       # 配置 SQL 与事务
        ├── migration.rs      # schema 版本检查与迁移
        └── migrations/
            └── 0002_text_storage.sql
tests/
├── contract/                 # 不依赖驱动的共同行为测试，供所有后端复用
│   ├── mod.rs
│   ├── history.rs
│   └── settings.rs
└── sqlite.rs                 # 落盘重启、并发连接、锁冲突、迁移与回滚
```

`api.rs` 提供调用入口并校验参数；`runtime/` 执行这些请求、协调生命周期。`StorageService` 是 runtime 的生命周期所有者，不是另一个业务服务层。`backend/contract.rs` 按完整操作定义事务边界，不向上游暴露通用 SQL 执行器或连接锁。

## 工作流程

```text
启动：配置 → 后端打开连接并迁移 → 启动 runtime → 返回所有者和句柄
操作：调用者 → Storage 请求代理 → 参数校验 → 有界队列
     → runtime → Backend → 数据库事务 → 类型化结果 → 调用者
关闭：所有者发出停止信号 → 完成正在执行的操作 → 拒绝排队请求
     → 关闭后端 → shutdown 返回
```

需要运行中的 Tokio runtime。`start` 返回成功时，后端和 schema 已就绪；迁移失败不会返回可用句柄。

```rust,no_run
# #[cfg(feature = "sqlite")]
# async fn example() -> nooboard_storage::Result<()> {
use nooboard_storage::{
    BackendConfig, HistoryQuery, HistorySource, NewHistoryEntry, Options,
    RecordHistory, Retention, Setting, SettingsChanges, SqliteOptions,
    StorageService,
};

let (service, storage) = StorageService::start(
    BackendConfig::Sqlite(SqliteOptions::file("data/history.db")),
    Options::default(),
).await?;

let result = storage.history().record(RecordHistory {
    entry: NewHistoryEntry {
        text: "你好 🦀".into(),
        source: HistorySource::Local,
        copied_at_ms: 1_800_000_000_000,
    },
    retention: Retention { max_entries: 1000, oldest_ms: 0 },
}).await?;
assert!(result.retained);

storage.settings().apply(SettingsChanges {
    put: vec![Setting { key: "app".into(), value: r#"{"version":2}"#.into() }],
    delete: vec!["legacy_app".into()],
}).await?;
let page = storage.history().query(HistoryQuery::default()).await?;
assert_eq!(page.entries[0].id, result.id);

service.shutdown().await?;
# Ok(())
# }
```

所有 `Storage`、`History`、`Settings` 克隆共享同一队列；默认容纳 32 个待处理请求，范围 1–1024，满时异步等待。runtime 按入队顺序逐个完成操作，不阻塞 Tokio 执行线程；SQLite 自行将同步驱动调用放入 `spawn_blocking`。异步数据库后端可以直接等待其驱动。

关闭信号独立于队列，队列满也可关闭。`shutdown` 拒绝未开始的工作并等待活动操作、连接关闭；`Drop` 只发出停止信号，需等待完成或获知关闭错误时必须调用 `shutdown`。所有请求句柄被释放时 runtime 也会收尾。应在 Tokio runtime 退出前完成关闭。

调用者取消尚未开始的请求时，该请求会被跳过；操作一旦开始就会执行到结束。取消等待、超时或关闭不能当作事务回滚证明。storage 不自动重试错误，尤其不重试结果不确定的提交。

## 数据契约

历史正文、查询字串和配置值最多 1 MiB，保留大小写、Unicode、空白和换行；接受空正文/空配置值。所有输入文本拒绝 NUL，以便 SQLite 与 PostgreSQL 保持相同语义。配置键非空且最多 256 字节；远端来源 ID 非空且最多 1024 字节。

`HistorySource::Local` 与 `Remote("local")` 是不同来源。时间戳是调用者提供的非负 Unix 毫秒数。完全相同的正文刷新已有记录的来源与时间、保留 ID；storage 不把过期到达的时间改成当前时间。`HistoryId` 是当前存储内的正整数标识，不用作跨端身份或排序依据。

记录与裁剪在同一事务提交；先删除严格早于 `oldest_ms` 的记录，再按时间降序保留 `max_entries` 条。同时间的顺序由后端稳定确定。写入较旧的记录可能立即被数量限制裁掉，`RecordOutcome.retained` 明确表示结果。记录时数量必须大于零且截止时间不晚于记录时间；单独 `prune` 允许数量为零。

查询按字面、区分大小写的子串匹配，不解释 `%`、`_`，不做文本正规化。来源和文本过滤先于分页，每页 1–1000 条，`next_offset` 表示本次快照是否还有结果。跨页是不同快照，并发修改可能使 offset 分页重复或跳过记录；本接口不承诺跨请求的一致快照。

每次配置读写最多 1024 个键；批量变更拒绝重复键和同时更新/删除同一键。`get_many` 按传入顺序返回值，重复键保留位置，不存在返回 `None`。配置内容是文本，解析失败由调用者处理。

`ErrorKind` 提供输入错误、忙、冲突、不可用、权限、损坏、schema 过新、已停止和内部错误等类别。默认错误格式不输出驱动诊断或正文；原始诊断可通过 `std::error::Error::source` 取得，调用者自行决定如何处理。

## SQLite 与后续 PostgreSQL

SQLite 是默认 Cargo feature；驱动和摘要依赖均为可选。`--no-default-features` 可构建公共模型和 runtime 并运行假后端测试，此时没有可启动的生产后端。内存测试使用真实 SQLite；生产可传入文件位置与锁等待时限（默认 2 秒，最多 30 秒）。

SQLite 使用 WAL 和 `BEGIN IMMEDIATE` 写事务，跨连接串行完成去重、写入与保留。SHA-256 索引只缩小候选范围，仍比较完整正文，摘要碰撞不会合并不同文字，也不对大文本建立唯一 B-tree 索引。业务数据为文本；摘要、来源、ID、时间属于后端内部索引与字段。

新建数据库或旧 v1 数据库会迁移到 v2：保留历史 ID、正文、来源、时间与 ID 分配水位，将合法 UTF-8 配置由旧 BLOB 转为 TEXT。schema、数据和版本号在同一事务修改；无法表示的旧数据导致迁移失败并回滚，不丢弃记录。高于支持版本的数据库会拒绝打开。旧版本应用不能打开已升级的数据库。

数据库仍是明文，新建 Unix 文件权限为 `0600`；`clear` 是逻辑删除，不承诺物理安全擦除。

接入 PostgreSQL 时：

1. 在 `backend/postgres/` 实现同一 `Backend`，将连接池、SQL、迁移、错误映射留在该目录。
2. 在 `options.rs` 增加启动配置，在 `backend/mod.rs` 增加工厂分支，并提供可选依赖/feature。
3. 用 PostgreSQL 的锁与事务隔离落实跨连接去重、裁剪原子性和批量读取快照；不能仅依赖本进程队列或摘要唯一约束。
4. 运行 `tests/contract/` 的同一套测试，补充该驱动的并发、失败和迁移测试。上游仅更换启动配置，保留 `Storage` 请求接口。

本轮未实现 PostgreSQL，也未提供 SQLite 到 PostgreSQL 的数据搬迁工具。不同数据库的 schema 和迁移分别由各自适配器维护。

## 验证

```sh
cargo test -p nooboard-storage --all-features --locked
cargo test -p nooboard-storage --no-default-features --locked
cargo clippy -p nooboard-storage --all-targets --all-features --locked -- -D warnings
python3 scripts/check_architecture.py
```

测试覆盖去重与摘要碰撞、来源过滤/分页、批量原子性、跨连接并发、持久化重启、旧 schema 迁移与失败回滚，以及队列饱和、取消、关闭和错误恢复。架构检查限制驱动依赖只能出现在相应适配器内。
