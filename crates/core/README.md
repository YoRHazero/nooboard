# nooboard-core

库接口参考，面向调用者与维护者。[应用使用](../../README.md) · [开发指南](../../docs/development.md) · [测试与实机验收](../../docs/testing.md)

core 负责用户设置、设备使用策略、同步规则和跨库工作流程。它通过适配器使用 clipboard、storage、network、identity、terminal 的公开句柄，不持有数据库连接、TLS 会话、文件传输 worker 或 APP 身份私钥。desktop 只依赖 core。

## 公开入口与生命周期

```rust,no_run
use nooboard_core::{AppService, BackendConfig, Options, SqliteOptions};

# async fn example() -> nooboard_core::Result<()> {
let (mut service, app) = AppService::start(Options {
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

- `AppService` 不可克隆，只负责启动与关闭。`shutdown(&mut self).await` 等待清理完成；Terminal 停止失败会保留所有者和底层依赖，可再次调用。`Drop` 在仍有 Tokio runtime 时安排有序清理，不替代显式等待。
- `App` 可以克隆，只持有请求通道和状态接收端。释放所有句柄不会关闭服务，存活的句柄也不会阻碍所有者关闭服务。
- `AppSnapshot` 是可恢复的最新状态。观察者允许错过中间进度；重新订阅即可获得当前设备、配对、剪贴板、历史版本、传输结果与 Terminal 配置、运行及待答提示。
- `subscribe_events` 是诊断用途的有界通知流，目前提供传输更新与历史变化。它可能丢失中间通知，不能充当工作队列或审计记录。

`lib.rs` 仅通过 `api` 导出类型。内部业务 runtimes 不向上游开放。后台实例由 app 装配创建，模型和策略函数本身不创建实例。

## 结构与状态所有权

```text
src/
├── api.rs                       AppService、App、terminal 公共门面
├── app/                         跨 feature 的装配与监督
│   ├── startup.rs / shutdown.rs 服务创建、逆序清理和失败保留
│   ├── supervisor.rs            监督 feature、聚合全局状态
│   ├── routing.rs               独占 NetworkEvents 并分发
│   ├── adapters/                Identity → Network 签名适配
│   └── snapshot/                应用快照与纯投影
├── configuration/
│   ├── domain/                  应用设置、配置变更、已配对设备
│   ├── application/             配置唯一写入者、请求队列
│   ├── ports/                   配置仓库与配置访问能力
│   └── adapters/                文档迁移、Storage 访问、句柄适配
├── features/
│   ├── devices/                 配对、设备设置与发现视图
│   ├── clipboard/               复制观察、同步、接收应用
│   ├── history/                 历史记录、查询和保留策略
│   └── terminal/                配置、启动快照、认证和信任交互
│       ├── domain/              config、inputs、views
│       ├── application/         runtime、use cases、状态与门面
│       ├── ports/               engine、repository、secrets、devices、keys
│       └── adapters/            Terminal、Storage、Identity、Network、私钥导入
├── contracts/                   底层公开的值类型和能力契约，不导入服务句柄
├── message.rs / clock.rs         请求回复与时钟辅助
└── tests/                       应用流程、生命周期和集成回归
```

所有 feature 均按 `domain / application / ports / adapters` 分层。`domain` 放纯数据和约束；`application` 放用例、actor 与其运行状态；`ports` 定义本 feature 为完成用例所需的能力；`adapters` 把具体服务接到这些能力上。原来每个 feature 的 runtime 归入 `application`，没有移动到 domain 或统一塞进 app。

跨 feature 的配置/历史访问也通过端口连接。稳定的底层值类型通过 `contracts` 复用，不重复定义 ID、状态和错误。`app` 是装配根，持有所有者并监督生命周期；不会实现 SSH 协议、进程监督、数据库或身份密码学。架构检查脚本限制内层依赖具体服务或外层适配器。


配置变更通过具体命令合并到最新文档；设备注册、手动目标和应用设置不会互相覆盖。`set_settings` 明确替换整份用户设置，调用者并发编辑该设置对象时应合并补丁；桌面入口对此串行处理。业务模块读取已提交的配置快照。

设备和同步分别维护自己的流程状态，history 管理历史工作与保留策略。snapshot 只做投影。配对密码学、连接重试、消息去重、传输阶段、文件校验与落盘全部由 network 管理，core 不保留第二套状态机。

Core 用原 profile 启动独立 Identity Service，再把公开证书与签名适配器注入 Network。Identity 继续读取原 `nooboard.identity.v1` 系统条目，不重建证书或身份。Core 同时把 Identity 的用途隔离 `seal/open` 能力接入 Terminal 配置仓库，Terminal 只获得本次认证所需材料。Network 和 Terminal 都不依赖 Identity crate。启动失败清理已经创建的服务。

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

关闭顺序：冻结 Terminal 启动和交互 → 等待全部终端、命令、隧道和文件任务结束 → 停止其他业务工作并完成已开始的应用 → 关闭 Network → 排空配置与历史工作 → 关闭 Clipboard、Storage、Identity。Terminal 停止失败时返回错误，保留管理器和依赖，调用者可以重试。监督器已在清理时，显式关闭返回 Busy，避免无限等待同一所有者锁。后台故障或 Drop 清理同样保留依赖直到 Terminal 确认结束；无法确认的资源不会因外层固定超时而被视作已结束。桌面隐藏到托盘不触发这一流程。

## Terminal 集成

`app.terminal()` 返回可克隆的 `TerminalWorkspace`。配置 CRUD、复制、删除、参数更新和交互应从这一入口进入；配置 ID 由后端生成 UUID，新执行按当前配置 ID 提交。运行时输入输出、停止、手动继续、生命周期切换、SFTP 和文件任务也从此门面访问。

`start_local_terminal(name)` 与 `start_local_terminal_with_output(name)` 直接打开本机默认 shell，生成 `config_id: None` 的普通 `TerminalRun`，不创建、修改或查找已保存终端配置。后一接口原子登记会话与输出订阅。两者仍经过协调器的启动准入和关闭检查；原有 `start_terminal(config_id)` 系列继续按配置冻结输入并持有占用。


```rust,no_run
use nooboard_core::terminal::{CommandInput, CommandTarget, CommandReusePolicy, CancellationToken};
# async fn example(app: nooboard_core::App) -> nooboard_core::Result<()> {
let terminal = app.terminal();
let id = terminal.create_command(CommandInput {
    name: "准备环境".into(), target: CommandTarget::Local,
    command: "printf ready".into(),
    auto_inputs: vec![],
}).await?;
let (_run, _output) = terminal.start_command_with_output(
    id, CommandReusePolicy::Shared, CancellationToken::new(),
).await?;
# Ok(())
# }
```

- **配置与启动**：一次启动冻结主配置及全部依赖；编辑只作用于后续执行。Shared 复用旧实例时显示旧实例的真实参数。配置被其他配置、活动 Run 或待处理启动引用时拒绝删除。
- **自动输入**：`CommandInput.auto_inputs` 提交有序规则，值用 `AutoInputValue::Keep` 保留或 `Set(SecretInput)` 覆盖，省略的键删除。现有键保持位置，新键追加。Core 通过 Identity 加密全部值并绑定命令 ID 与提示键；复制命令重新加密。持久化使用 `IndexMap` 的有序键值对数组，旧配置默认空映射。快照只返回提示键，不包含值或密文。启动计划冻结规则，解密后交给 Terminal；Shared 复用不重复发送。命令回显按原样展示，加密保证针对配置存储。
- **可靠占用**：Core 串行登记启动预留，Terminal 管理器在登记真实 Run 时取得 `RunLease`，直到确认结束才释放。Shared 等待保留预留，独立存活的命令保留自己的 lease，不依赖可落后的 UI 结束事件。
- **设备地址**：依赖放行后读取 Network 当前发现或活动连接地址，使用配置的 SSH 端口。无需配对；无可用地址直接失败，不使用历史地址兜底。
- **主机信任**：按 SshTargetConfigId 保存公钥。未知密钥产生 HostTrust 提示，变化则失败并记录旧/新密钥。更换信任需要提交预期旧值，避免并发覆盖。复制配置不复制信任。
- **认证**：主机确认后才产生 Password 提示。`SshAuthInput::Password` 选择密码方式或清除已保存密码，实际输入通过 `answer_password` 提交；只有认证成功且用户勾选保存，才加密持久化。旧认证回报不会覆盖后来修改的认证或目标参数。保存失败报告 `CredentialNotSaved`，已成功连接仍可使用。
- **导入私钥**：接收内存内容与可选 passphrase，验证并解锁后交由 Identity 加密；不保留文件路径、原 passphrase 或未加密私钥。用途、配置 UUID、字段和版本参与加密绑定；复制配置重新加密。公开快照只包含认证方式/是否保存，不包含明文或密文。
- **交互与订阅**：待答提示保留在快照中，可恢复订阅、回答或取消；过期响应不继续执行。`subscribe_runs()` 的结束事件与字节输出保持有界流语义，活动快照不保存结束历史。
- **参数**：默认来自 Terminal 的 JSON 配置；Core 保存 `TerminalServiceOptions` 并应用。控制队列容量在本次启动固定，运行中修改被拒绝。保存成功但运行时应用失败会报告 Service issue，保存值用于下次启动；不存在跨数据库与运行时的原子事务。

Core 已接入真实 Terminal Service。桌面通过生成的 IPC 契约调用这一门面，提供 Terminal 页面、认证交互、输出回放与文件面板；具体组织见[桌面架构](../../apps/desktop/ARCHITECTURE.md)。

## 持久化与兼容范围

应用配置使用 storage 的文本设置接口，键为 `core.configuration`，文档版本为 4，兼容 v3 文档，并原样保留退役功能的数据以供后续显式迁移。存储驱动只在启动时选择，业务不使用 SQL。未知文档版本或损坏配置明确失败，不重置身份或覆盖原记录。

加载或迁移配置后，若接收目录仍为空，使用宿主提供的默认目录并随配置保存；用户已设置的目录优先。宿主也未提供默认值时保持为空。

Rust API 与持久化格式的兼容性分别管理。旧 `configuration_v2` 文档由 configuration 迁移：保留设备策略、手动目标和已确认的公开证书，并通过 network 的公开证书解析接口恢复身份信息。新文档写入与旧键删除在 storage 的一次原子设置操作中提交。无设备的旧 settings 文档也可迁移；更早的单设备 peer 格式明确拒绝，避免静默改变信任。损坏或版本未知的文档保留原样。

旧数据库不会被自动删除或覆盖；storage 继续负责自己的数据库结构与文本历史迁移。APP 身份私钥由 Identity 的凭据后端管理。SSH 密码及导入私钥使用同一数据库中的加密文档，不创建额外系统凭据条目。

## 验证

```sh
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo fmt --all -- --check
python3 scripts/check_architecture.py
```

core 测试使用临时身份、内存剪贴板、临时 SQLite 和真实回环网络/SSH，覆盖生命周期、关闭时完成应用、持久化失败、实际配对、文本/图片/文件、防回传和恢复基线。诊断入口通过 `nooboard_core::diagnostics` 的公开导出提供，普通测试不访问系统凭据或原生剪贴板。Terminal 集成覆盖冻结配置、配置占用、成功后保存、旧回调防覆盖、信任取消/变更、私钥导入与密文绑定、SFTP 共用连接及失败关闭重试。

原生三实例验收使用三个独立 macOS pasteboard：

```sh
cargo build -p nooboard-core --example link_probe --features diagnostics --locked
python3 -B scripts/test_three_instances.py
```

跨机 LAN/VPN、Windows 和 Linux 原生桌面行为仍由相应平台环境验收。
