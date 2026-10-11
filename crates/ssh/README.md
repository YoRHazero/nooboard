# nooboard-ssh

库接口参考，面向调用者与维护者。[应用使用](../../README.md) · [开发指南](../../docs/development.md) · [测试与实机验收](../../docs/testing.md)

独立的 SSH 终端和 SFTP 服务，不依赖 core、network、storage 或桌面框架。由 core 装配，通过 Tauri 类型化 IPC 接入桌面页面。跨模块契约见 [SSH 工作区设计](../../docs/ssh-workspace-design.md)，应用操作见 [SSH 使用指南](../../docs/ssh-guide.md)。

## 所有权与模块

`SshService::start` 返回 `(SshService, Ssh, SshEvents)`。`SshService` 是不可克隆的生命周期所有者；`Ssh` 是可克隆的请求句柄；`SshEvents` 是主机密钥决策的独占可靠接收端。句柄和订阅的释放不会终止服务，`shutdown` 等待受监督的工作结束与文件清理，所有者 Drop 发出停止信号。

```text
src/
  api.rs                 唯一公开边界
  runtime/               装配、监督、连接尝试和关闭
  connection/            TCP/SSH 握手、主机身份核验和密码认证
  terminal/              独立终端连接、PTY、输入输出、重放缓冲
  sftp/
    runtime.rs           文件连接所有者；创建和监督下列两个 runtime
    client.rs            SFTP 协议适配
    path.rs              远端路径与属性验证
    directory/
      model.rs           条目、批次及游标
      runtime.rs         查询注册、并发、取消、过期和回收
      reader.rs          按需 READDIR，仅缓存未交付条目
    transfer/
      model.rs           固定的源/目标、进度和结果
      runtime.rs         排队、并发、取消、重试和状态
      upload.rs          普通文件上传
      download.rs        普通文件下载
      publish.rs         不覆盖发布和重名编号
  credentials/           秘密包装与系统凭据库适配
```

每个服务只有一套 directory runtime 和 transfer runtime。具体查询和任务使用受监督的 Tokio 任务，不为每个目录创建线程或整套服务。文件任务持有连接租约，关闭文件连接时仍有租约会返回 `Busy`，不能取消正在进行的传输。

## 调用契约

- `connect(Connect)` 接受已解析的地址候选、设备标识、账号、`Password`、可信主机密钥、PTY 行列与连接范围，立即返回连接尝试 ID。真实结果通过快照提供。`Both` 建立两条独立 SSH 连接；`Terminal` / `Files` 支持上层仅替换出错的一侧，旧连接 ID 不复用。
- 未提供可信主机密钥时，认证前发出 `HostKeyRequired`。调用者核验并持久化用户明确同意的公开密钥后，才能调用 `decide_host_key`。不匹配的已知密钥直接失败，不降级为首次确认。SSH 主机证书的 CA 信任暂不支持。
- `subscribe()` 提供可合并的状态快照。终端字节通过 `subscribe_terminal()` 独立传输，附带字节偏移。先订阅再取 `terminal_replay()`，忽略已重放的字节；订阅落后时重新取重放，尊重 `gap` 标记。默认每个终端保留 1 MiB。
- 文件连接建立只执行初始路径解析与属性验证。`validate_directory` 使用 REALPATH/STAT，不列目录。`read_directory` 才打开句柄，默认交付最多 200 项；`next_directory_batch` 消费不透明游标，游标每批更新，旧游标不能重放。取消、EOF、错误和空闲过期均关闭句柄。
- 远端路径不是本地 `PathBuf`，不会做 shell 展开。相对路径基于调用者传入的浏览根路径。Windows 盘符绝对输入转换成斜杠表达后交给服务端解析；服务端的 SFTP 命名空间始终是最终依据。
- 每个 `transfer` 调用创建一个普通文件任务。多选由上层逐个提交，目录和符号链接不递归传输。任务固定绝对源路径、目标目录、会话和设备身份。
- 本地下载采用同目录临时文件、关闭文件后通过排他硬链接发布；不支持硬链接的目标文件系统明确报错，不降级为覆盖。远端采用 SFTP v3 不覆盖 RENAME，不调用覆盖式 POSIX rename 扩展。重名自动编号。
- 发布请求无确定回包时标记 `Unconfirmed`，保留目标与临时路径供核查，禁止直接重试。普通失败和取消可以显式从头重试；未确认清理的临时路径仍保留在任务记录中。
- `close_terminal` 不关闭文件连接。`close_files` 需要查询与传输租约已释放。`forget_session` 清理已经结束的会话及重放缓冲；`forget_transfer` 清理完成记录。容量耗尽明确返回 `Limit`。

取消是协作式的：已经发出的文件协议请求有超时上限，先取得结果再关闭句柄。已开始的发布必须等待其结果；取消不能把成功发布改写为“文件不存在”。不提供断点续传或进程重启后的任务恢复。

## 凭据与上层职责

`Password` 不可序列化，Debug 固定脱敏，应用持有的字符串在释放时清零；第三方认证库内部可能存在短暂副本。密码不会进入 SSH 快照、终端帧或主机密钥事件。

`CredentialStore` 提供异步 load/save/delete，系统调用放在阻塞工作线程。默认 `native-credentials` 使用 macOS Keychain、Windows Credential Manager、Linux Secret Service；关闭该 feature 时凭据调用明确返回 `Credentials`，不会写入明文文件。自动测试不读写用户的真实凭据库。

是否记忆、认证成功后保存、失败后的凭据禁用、设备发现地址更新、公开主机密钥持久化、浏览根路径/已交付目录缓存及下载目录设置，由 core 的 `remote` 工作区模块实现。本 crate 不读取 nooboard 配对数据库，不因为设备已配对就自动信任 SSH 密钥。

## 验证与限制

```sh
cargo test -p nooboard-ssh --all-features --locked
cargo test -p nooboard-ssh --no-default-features --locked
cargo clippy -p nooboard-ssh --all-targets --all-features --locked -- -D warnings
```

测试服务器只监听 `127.0.0.1` 的随机端口，使用真实 SSH 加密与认证和内存 SFTP 文件系统。覆盖认证前确认、主机密钥变化、两条连接分别断开、终端重放缺口、目录请求计数与游标过期、上传下载重名、取消/关闭清理以及发布回包丢失。

这些测试不等同于真实 OpenSSH/Windows 服务、Windows/Linux 客户端构建或系统凭据库的跨平台验收。桌面页面、原生拖入适配与文件选择入口已经实现；原生拖拽、各平台系统凭据库和外部 SSH 服务仍需实机验收。
