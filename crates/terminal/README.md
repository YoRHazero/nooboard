# nooboard-terminal

独立的后台 Terminal Service：本地与 SSH 终端、可共享的固定命令、SSH 隧道及终端所属的 SFTP 文件操作。使用 `russh 0.64.1`、`russh-sftp 3.0.1`、`portable-pty 0.9.0`，不启动系统 `ssh`。

本 crate 已实现运行后端与调度，并通过 Core 的 `app.terminal()` 接入应用；新桌面 IPC 和终端页面尚待实现。它不依赖其他 nooboard crate，也不读取应用配置、保存密码或维护主机信任数据库。[Identity Service](../identity/README.md) 已独立，由 Core 使用其加解密能力向 Terminal 提供认证材料。完整行为约定见 [Terminal Service 设计](../../docs/terminal-service-design.md)。

## 快速运行

在仓库根目录执行：

```sh
cargo run -p nooboard-terminal --example command -- 'echo hello'
```

示例将一个完整字符串作为本地命令运行，读取输出和结束结果，最后显式关闭 Service。Ctrl+C 也进入关闭流程。可通过 `NOOBOARD_TERMINAL_OPTIONS` 环境变量传入 JSON 配置文件路径；默认配置位于 [config/defaults.json](config/defaults.json)。示例不连接 SSH，也不启动 VPN。

本地固定命令使用 Unix `/bin/sh -c` 或 Windows `cmd.exe /C`；普通本地终端使用平台默认交互 shell。命令应是调用方明确授权执行的内容。终端输入可通过 `write_input` 写入；`resize` 是运行期 I/O 操作，不是持久化配置字段。

## 公共接口

所有公共类型从 crate 根导出，第三方协议句柄和实现模块不对外暴露。`TerminalService::start(options, dependencies)` 要求处于 Tokio runtime 内，返回不可克隆的 Service 所有者；`handle()` 返回可克隆的 `Terminal` 操作入口。

| 接口 | 用途 |
| --- | --- |
| `start_terminal` / `stop_terminal` | 本地或 SSH 终端；关闭时结束所属会话及文件操作 |
| `start_command` / `stop_command` | 独立命令；支持 Shared 获取或 AlwaysNew 创建 |
| `start_tunnel` / `stop_tunnel` | 单条 Local、Remote 或 Dynamic 转发规则 |
| `continue_dependency` | 仅放行指定依赖步骤 |
| `set_command_lifetime_policy` | 在 Dependent 与 Independent 间切换 |
| `snapshot` / `subscribe` | 活动状态、合并快照流和独立结束事件流 |
| `subscribe_output` / `write_input` / `resize` | 终端与命令的字节流 I/O |
| `start_terminal_with_output` / `start_command_with_output` | 接纳启动和订阅输出合为一个操作，捕获立即退出的程序输出 |
| `list_directory` / `create_directory` / `rename_path` | SSH 终端的目录浏览与短文件操作 |
| `start_file_operation` / `stop_file_operation` | 递归上传、下载、删除任务 |
| `update_options` | 更新服务参数，生效时点见下文 |
| `TerminalService::shutdown` | 等待全部资源结束；失败保留所有者和活动记录 |

启动返回 ID 表示请求已接纳，不表示连接或依赖已经成功。Shared 等待旧实例停止时，`start_command` 保持等待；其 cancellation token 只取消尚未取得实例的请求。取得 ID 后使用 stop 接口。

`subscribe()` 在同一个临界区取得初始快照并注册后续流。快照只保留活动资源，结束结果通过事件发送。事件或输出订阅者落后时明确返回 `Lagged`，不能用于必须可靠执行的业务副作用。调用方应在启动前订阅结束事件；短命令的输出使用 `*_with_output`，避免先启动后订阅之间的空隙。输出包含字节偏移，前端自行增量解码和处理终端控制序列。

Core 使用 `TerminalService::start_with_leases` 注入 `RunLeaseProvider`。管理器同步登记每个 Run 的配置引用并持有返回的 guard，包含主配置、SSH 目标及尚未执行的依赖；只有实际 Run 移除才释放。Shared 复用不会替换已有 guard。提供者必须快速返回、不能阻塞或 panic；持久化不在该钩子内执行。独立调用方可以继续使用无钩子的 `start`。

## 依赖与停止

终端和隧道接受已展开的有序前置命令。每次请求冻结参数；Shared 按 `CommandConfigId` 选择最新创建且可使用的实例，复用时保留该实例实际参数与生命周期。若只剩 Stopping 实例，等待确认结束后，最早等待者登记新实例，其他请求再复用。停止无法确认时保留占位并使等待者失败。

依赖创建的首个命令为 Dependent，最后一个使用者结束后停止；独立创建的首个命令为 Independent。再次复用不改变此属性。自动放行只接受成功退出或完整行 `::nooboard:v1:ready::`（支持 LF / CRLF，标识从显示输出移除）。手动放行只作用于当前依赖。持续依赖随后退出会终止其使用者；已成功退出的一次性前置步骤不受影响。没有自动重试。

Service 由宿主持有，因此收起窗口或退出到托盘不影响运行。完全退出应用前必须 `shutdown().await`；普通 Drop 只发出停止请求，不能替代等待清理。停止超时返回错误并保留 Stopping；再次关闭继续等待剩余资源，不会把未确认结束的实例当作已结束。

macOS / Linux 监督新建 PTY session 中的进程，Windows 使用 Job Object。普通子进程一起清理；主动脱离会话的进程、远端守护进程和远端机器失联后的进程追踪不在范围内。没有权限强制结束的本地进程会产生停止错误，不能报告清理成功。

## SSH 适配边界

调用方提供三个接口：

- `SshConnectionProvider`：在依赖放行后解析地址，返回地址、冻结的用户名和认证提供者。动态设备地址可在此解析；无地址直接失败。
- `SshHostVerifier`：收到服务器公钥后核验或请求用户确认。Terminal 不自动接受未知密钥。
- `SshAuthProvider`：主机验证通过后取得密码或已解锁的内存私钥材料；认证成功后调用 `authentication_succeeded`，调用方才可保存新密码。保存失败由调用方报告，回调不返回连接错误。

provider 必须保留该请求冻结的配置，不能在回调中重新读取已编辑的命令或认证选择。用户交互等待可取消，不计入握手超时；网络请求有独立超时。`SecretString` / `SecretBytes` 不支持序列化，Debug 隐去内容，拥有的缓冲在释放时清零；这不是对第三方库内部所有副本的保证。

每个 SSH Run 独占连接，TerminalRun 的 shell 与按需创建的 SFTP 子系统共用该连接。命令、隧道不提供 SFTP，也不跨 Run 共用连接。Dynamic 提供 SOCKS5 的无认证 TCP CONNECT，不支持 SOCKS4、UDP 或 BIND；监听地址由调用方显式配置。没有自动读取 `~/.ssh/config`、Agent、ProxyJump 或系统 known_hosts。

## 文件操作

文件面板路径由调用方持有，独立于 shell 当前目录。支持浏览、单层 mkdir、服务器原生 rename，以及递归上传、下载、删除。远端路径为经过验证的绝对 UTF-8 路径。传输遇到源符号链接即失败；删除符号链接只删链接。

上传/下载用目标同目录临时文件，成功后提交。同名目录合并；文件冲突按 Fail、Skip、Overwrite 处理；文件与目录类型不匹配失败。远端覆盖要求服务器支持 `posix-rename@openssh.com`，否则保留原文件并报告 Unsupported。提交请求的回执丢失报告 `CommitUnconfirmed`，不宣称目标未改变。

取消或失败清理已知归属的临时文件，并报告可能残留的路径；批处理中已完成的条目不回滚。短操作、文件任务和共享 SFTP 连接均归属于 TerminalRun，结束终端前等待这些操作结算。

## 参数更新

默认秒数、轮询间隔、缓冲和队列容量集中在 JSON 中，均可由调用方替换：

- 输出缓冲大小立即作用于活动资源；无法跟上的订阅者收到 Lagged。
- 事件队列容量作用于新订阅；输入队列容量在登记 Run 时固定。
- 网络请求、传输缓冲与进度间隔在对应 worker 启动时固定；共享 SFTP 会话的请求超时在其首次创建时固定。
- 停止和整体关闭的预算在开始对应操作时固定。管理任务控制强制停止时点；本地 worker 兜底清理也保留其启动时的参数。
- `control_queue_capacity` 在 Service 创建时固定，运行中改变会被拒绝。

## 验证

```sh
cargo test -p nooboard-terminal --all-targets --locked
cargo clippy -p nooboard-terminal --all-targets --locked -- -D warnings
```

测试使用自建回环 SSH/SFTP 服务、临时目录和本地 PTY，不需要生产凭据。覆盖依赖共享与停止、状态/输出流、主机验证与认证时序、真实转发字节、文件冲突、取消、临时文件清理与提交结果不确定。

当前实测平台为 macOS。Linux 与 Windows 的进程监督实现仍需在各自平台验收。Windows 当前在 portable-pty 创建进程后将其加入 Job Object，创建到加入之间可能存在早期子进程的归属空隙；该启动边界需要在 Windows 后端进一步解决，尚不能宣称 Windows 已满足完整进程树清理契约。回环测试也不能替代与实际 SSH 服务器、VPN 命令的最终互操作验收。

终端启动请求 `StartTerminalRequest` 的 `config_id` 为 `Option<TerminalConfigId>`，`name` 是本次会话的显示名称。`None` 表示没有已保存配置的临时会话；仍返回独立 `TerminalRunId`，沿用输出、停止与 shutdown 的统一生命周期。临时会话不登记终端配置占用；如果调用方提供 SSH 目标或前置命令，仍持有这些实际引用的占用。命令和隧道的配置 ID 继续为必填。
