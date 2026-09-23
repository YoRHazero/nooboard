# nooboard-core

core 负责用户设置、设备使用策略、同步规则和跨库工作流程。它只使用 clipboard、storage、network 的公开句柄，不持有数据库连接、TLS 会话、文件传输 worker 或私钥。desktop 只依赖 core。

## 公开入口与生命周期

```rust,no_run
use nooboard_core::{AppService, BackendConfig, Options, SqliteOptions};

# async fn example() -> nooboard_core::Result<()> {
let (service, app) = AppService::start(Options {
    storage: BackendConfig::Sqlite(SqliteOptions::file("nooboard.sqlite3")),
    profile: "nooboard.desktop.v1".into(),
    default_receive_directory: None,
}).await?;

let commands = app.clone();
let mut snapshots = app.subscribe();
let _current = app.snapshot();
// 将 commands 移交给命令入口；通过 snapshots.changed().await 观察最新状态。
service.shutdown().await?;
assert!(!commands.is_running());
# Ok(())
# }
```

- `AppService` 不可克隆，只负责启动与关闭。`Drop` 发出停止信号；`shutdown(self).await` 等待清理完成。
- `App` 可以克隆，只持有请求通道和状态接收端。释放所有句柄不会关闭服务，存活的句柄也不会阻碍所有者关闭服务。
- `AppSnapshot` 是可恢复的最新状态。观察者允许错过中间进度；重新订阅即可获得当前设备、配对、剪贴板、历史版本和传输结果。
- `subscribe_events` 是诊断用途的有界通知流，目前提供传输更新与历史变化。它可能丢失中间通知，不能充当工作队列或审计记录。

`lib.rs` 仅通过 `api` 导出类型。内部业务 runtimes 不向上游开放。后台实例由顶层 runtime 创建，模型和策略函数本身不创建实例。

## 结构与状态所有权

```text
src/
├── api.rs                  AppService、App 与公开类型
├── options.rs              启动配置，数据库选择只在此边界出现
├── error.rs                业务错误
├── ports.rs                原生剪贴板的窄测试替身边界
├── runtime/
│   ├── startup.rs          启动与失败清理
│   ├── mod.rs              监督任务、聚合快照
│   ├── routing.rs          独占 NetworkEvents，分发有界工作
│   ├── shutdown.rs         底层服务清理
│   └── message.rs          类型化请求回复与停止语义
├── configuration/
│   ├── model.rs            配置、设备策略与公开信任记录
│   ├── document.rs         文本配置的版本、读取、验证和保存
│   └── runtime.rs          持久化配置的唯一修改者
├── devices/
│   ├── model.rs            配对交互与本机网络视图
│   ├── pairing.rs          保存确认、期望身份检查、地址解析
│   └── runtime.rs          配对、解除配对、设备配置
├── sync/
│   ├── model.rs            应用操作信息与展示结果
│   ├── policy.rs           自动目标选择
│   ├── content.rs          两个库的内容类型转换
│   └── runtime/
│       ├── mod.rs          同步状态、串行应用队列、预览任务
│       ├── observe.rs      内容来源与复制观察
│       ├── send.rs         手动及自动发送
│       ├── receive.rs      接收策略与实际结果反馈
│       └── apply.rs        唯一的生产剪贴板写入路径
├── history/
│   ├── model.rs            历史查询结果与历史版本
│   ├── policy.rs           时间与保留策略
│   └── runtime.rs          记录、查询、删除与定期清理
└── snapshot/
    ├── model.rs            AppSnapshot 等应用视图
    ├── assemble.rs         从各状态源生成快照
    └── preview.rs          有界缩略图生成
```

配置变更通过具体命令合并到最新文档；设备注册、手动目标和应用设置不会互相覆盖。`set_settings` 明确替换整份用户设置，调用者并发编辑该设置对象时应合并补丁；桌面入口对此串行处理。业务模块读取已提交的配置快照。

设备和同步分别维护自己的流程状态，history 管理历史工作与保留策略。snapshot 只做投影。配对密码学、连接重试、消息去重、传输阶段、文件校验与落盘全部由 network 管理，core 不保留第二套状态机。

## 业务流程

### 配对与设备

network 配对事件 → devices 保存待确认交互 → 用户通过 App 接受或输入代码 → 收到 `PairingVerified` → configuration 保存公开信任记录 → `complete_pairing(id, true)`。

保存失败明确回复 `false`；选择附近设备时还检查已验证身份是否符合用户选中的设备。应用当前允许一个前台配对交互，其他请求明确返回忙或拒绝。隐藏已完成配对不会撤销成功状态。新设备默认不参加自动发送。

解除配对先持久化移除记录与发送目标，再撤销运行时信任。运行时应用失败会留下错误和未生效的配置版本，并重试协调；重启不重新加载已经移除的信任。两个服务不是跨服务数据库事务。

对端通过已认证连接报告新名称后，devices 通过 configuration 仅更新并保存该设备名称，保留信任、设备策略和发送目标。离线或本机重启后继续显示最后保存的名称；局域网发现中的名称不用于改写已配对设备。

### 剪贴板与传输

外部复制 → sync 观察来源及 revision → 按策略提交文字历史 → 选择已就绪的自动目标 → `Network::send`。自动发送仅处理文字，手动发送支持文字、图片和文件。一次发送固定一份内容，目标结果独立。

应用自身写入不会自动发送，也不会由观察回调重复记入历史。启动、恢复自动模式、为单个设备启用自动发送和设备重新就绪时，分别为新获得发送资格的目标建立复制基线。处理剪贴板通知前也会核对目标并补齐基线，因此通知处理顺序不会导致启用前的内容被补发。clipboard 的 watch 订阅可能合并快速变化，core 只承诺处理实际观察到的内容。

configuration 为各目标记录最近一次启用的配置版本，sync 将它与复制基线关联。即使快速关闭再开启造成 watch 合并通知，也会重新建立基线；名称等无关配置变更不会重置它。启用版本只用于当前运行期，不写入持久化文档。

远端 Offer → 检查接收设置和文件目录 → network 接收、验证、落盘 → ContentReady → sync 串行应用剪贴板 → 回复实际 ApplicationOutcome。图片字节、文件内容和收到的文件路径不写入 storage；历史只记录文字。

文件已经保存但剪贴板写入失败时，结果仍是 Saved，并展示应用失败原因。用户可以再次复制保存的文件路径；这不会改写原传输的历史结果。关闭、取消和历史保存失败都不能抹掉已经发生的应用或落盘。传输状态与事件通过显式传输 ID 关联。

复制历史同样经过 sync 的应用队列。history 只查询记录，不写剪贴板。历史写入使用独立有界队列，失败或拥塞会报告故障，但不阻止发送或把已成功的应用改成失败。

缩略图最多执行一个后台任务，并保留一个待处理的最新请求。结果绑定剪贴板 revision，过期结果丢弃。内容活动摘要从 network 的实际状态生成。

### 配置生效与关闭

`configuration_revision` 表示已提交的配置；`effective_revision` 表示业务 runtimes 已应用的配置版本。设备名称和监听地址属于 network 启动参数，保存后通过 `restart_required` 明确提示重启，界面不宣称已热切换端口。

关闭顺序：停止接受请求及自动工作 → 完成已开始的应用并反馈结果、拒绝待处理接收工作 → 关闭 network 并等待文件发布任务 → 排空已接受的配置与历史工作 → 关闭 clipboard 与 storage。停止不通过可能饱和的业务队列发送。

## 持久化与兼容范围

应用配置使用 storage 的文本设置接口，键为 `core.configuration`，文档版本为 3。存储驱动只在启动时选择，业务不使用 SQL。未知文档版本或损坏配置明确失败，不重置身份或覆盖原记录。

加载或迁移配置后，若接收目录仍为空，使用宿主提供的默认目录并随配置保存；用户已设置的目录优先。宿主也未提供默认值时保持为空。

本轮不保留旧 Rust API 的兼容层。旧 `configuration_v2` 文档由 configuration 迁移：保留设备策略、手动目标和已确认的公开证书，并通过 network 的公开证书解析接口恢复身份信息。新文档写入与旧键删除在 storage 的一次原子设置操作中提交。无设备的旧 settings 文档也可迁移；更早的单设备 peer 格式明确拒绝，避免静默改变信任。损坏或版本未知的文档保留原样。

旧数据库不会被自动删除或覆盖；storage 继续负责自己的数据库结构与文本历史迁移。系统私钥仅由 network 的凭据后端管理。

## 验证

```sh
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo fmt --all -- --check
python3 scripts/check_architecture.py
```

core 测试使用内存剪贴板、临时 SQLite 和真实回环网络，覆盖生命周期、关闭时完成应用、持久化失败、实际配对、文本/图片/文件、防回传和恢复基线。诊断入口通过 `nooboard_core::diagnostics` 的公开导出提供，普通测试不访问系统凭据或原生剪贴板。

原生三实例验收使用三个独立 macOS pasteboard：

```sh
cargo build -p nooboard-core --example link_probe --features diagnostics --locked
python3 -B scripts/test_three_instances.py
```

跨机 LAN/VPN、Windows 和 Linux 原生桌面行为仍由相应平台环境验收。
