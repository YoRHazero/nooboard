# nooboard

使用 Rust 开发的 macOS / Windows / Ubuntu 跨设备剪贴板工具，面向同一局域网或已有 VPN 中可直接连通的设备。

当前后端已支持多设备同时配对、独立双向连接和文字并发发送，并提供最小验证示例，尚未完成真实 Mac ↔ Windows 双设备验收。`apps/desktop` 已接入 Tauri 2 与真实 Rust 后端，支持局域网自动发现、应用内一次性配对码、地址添加、设备管理、文字收发、独立回执和 SQLite 历史；普通浏览器保留独立的示例预览。

在桌面应用中，进入「设备 → 添加设备」选择附近设备，或输入对方 IP / 主机名及配对端口（默认 `24817`）。对方允许后显示 8 位配对码，在发起端输入即可完成双端配对；有效期 2 分钟，最多三次尝试。配对后文字同步继续使用双向 TLS（默认端口 `24816`）。VPN 不支持 mDNS 时使用地址添加，无需通过第三方程序交换证书。新设备的自动发送默认关闭。

## 第一轮开发

四个独立于界面的 Rust 库，通过自动化测试与最小命令行示例验证文字同步及本地文字历史：

```text
core → clipboard
     → network
     → storage
```

- `core`：组装底层模块，协调设备、同步与历史业务。
- `clipboard`：封装 macOS、Windows、Linux 原生剪贴板；本库的实现不依赖 arboard。
- `network`：连接、协议与经过身份验证的加密传输。
- `storage`：历史记录与配置持久化。

底层库之间不直接互相依赖。React + TypeScript 交互设计稿位于 `apps/desktop`，首页通过肥啾舞台上的木板、肥啾与信箱展开操作。Tauri 桌面入口只依赖 core，图片和文件手动传输留到后续轮次。

## 验证计划

工具链固定为 Rust 1.93.0。仅构建四个库无需 Node.js；完整桌面工程需要 Node.js 22.12+，先在 `apps/desktop` 执行 `npm ci` 和 `npm run build`。Windows 构建需要 MSVC C++ Build Tools 与 Windows SDK，macOS 需要 Xcode Command Line Tools。

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
python3 scripts/check_architecture.py
```

GitHub Actions 在 macOS 14、Windows Server 2022、Ubuntu 24.04 runner 上执行构建、测试和架构检查；Windows 上使用 `python` 命令。普通测试使用内存剪贴板、临时数据库和本机 TLS 连接。

原生测试单独运行：macOS 使用独立 pasteboard，Windows 会改写测试会话剪贴板，仅在隔离测试会话中执行。CI 也单独运行这一步，结果仅代表 runner 会话。

```sh
cargo test -p nooboard-clipboard --all-features --locked -- --ignored --test-threads=1
```

Ubuntu 原生验证使用 `bash scripts/test_linux_native.sh`，需要 `Xvfb`、`sway`、`dbus-run-session`、`dbus-send` 和 `gnome-keyring-daemon`。脚本创建并清理独立 X11、Wayland、D-Bus 与密钥环会话。

## Ubuntu 会话要求

- X11：使用 `x11rb` 和 XFixes 监听 CLIPBOARD 所有权变化，支持 UTF-8、STRING 及 INCR 分块传输；不使用 PRIMARY 选区。
- Wayland：优先使用 `ext-data-control-v1`，其次使用 `wlr-data-control-v1`。合成器必须暴露相应协议和 seat；只有普通 `wl_data_device` 的会话会明确报错。具体可用性取决于合成器版本和授权配置。
- 自动选择：有 `WAYLAND_DISPLAY` 时选择 Wayland，否则使用 `DISPLAY` 对应的 X11。可以显式设置 `NOOBOARD_LINUX_BACKEND=x11` 或 `wayland`；Wayland 不可用时不会静默改用 XWayland。
- 身份存储：需要当前 D-Bus 会话中的、可解锁的 Secret Service，例如 GNOME Keyring。没有服务时明确失败，不使用内存假密钥环代替持久存储。
- SSH/headless 会话通常没有桌面剪贴板；开发测试应另建虚拟显示，不能把 SSH 转发的 X11 剪贴板当成远端独立桌面。

普通使用不依赖 `xclip`、`wl-copy` 或外部剪贴板进程。Linux 后端通过协议事件响应变化，并以短周期维护超时传输。

## 最小示例

```sh
cargo run -p nooboard-core --example backend -- --help
cargo run -p nooboard-core --example backend -- /path/to/private/nooboard.db my-device
```

第二个参数是稳定的身份配置名；私钥保存在 macOS Keychain、Windows Credential Store 或 Linux Secret Service。历史和设置保存在指定 SQLite 文件中，**历史目前为明文**，请使用当前用户的私有目录。默认仅手动发送、允许接收并记录文字历史，最多 1000 条、保留 30 天。

命令行示例与桌面应用使用同一套配对码流程。默认同步监听 `0.0.0.0:24816`，配对监听 `0.0.0.0:24817`，分别用 `listen` 和 `pair-listen` 修改。

```text
# A 发起；也可先 discover，再用 pairing 查看附近设备与会话
pair <B地址>:24817
pairing
# B 查看会话 ID，允许后再次查看本地显示的配对码
pairing
accept <B会话ID>
pairing
# A 输入 B 上显示的配对码
code <A会话ID> <8位数字>
# 任一端可取消自己的会话
cancel <本端会话ID>
```

`pairing` 明确显示当前本地配对状态和码，不应将这项交互输出作为普通应用日志收集。验证完成后双方自动保存身份，每对设备维持一条双向 TLS 连接，断线独立重连；重连不要求重新输入配对码。`status` 显示公钥派生的稳定 `noob_id`、允许重复的设备名称和连接状态；内部操作使用完整 ID。

- `targets <noob_id>...`：保存手动发送目标；`send` 使用该选择，`send <noob_id>...` 指定并记住目标。
- `peer-auto <noob_id> on/off`：各设备是否参与自动发送，新配对默认关闭；`auto` / `manual` 控制本机发送模式。
- `name <设备名称>`：修改本机展示名称，身份 ID 不变。
- `pause` / `resume`：暂停或恢复同步；`receive on/off` 独立控制接收。
- `history on/off` / `list [关键词]` / `copy <id>` / `delete <id>` / `clear`：文字历史管理。
- `status` / `unpair <noob_id>` / `quit`：查看状态、解除指定设备配对、关闭后端。

一次发送固定同一份正文，分别记录各目标结果；提交成功不代表对端已应用，目标状态 `Applied` 才表示曾成功写入远端剪切板。不同来源的消息按本机接收队列串行写入，最后成功写入的内容保留。收到的文字不会再次自动转发。

每条文字最多 1 MiB，保留 Unicode、空白和换行，拒绝嵌入 NUL；图片、文件及已识别的敏感格式跳过。各目标只合并连续、尚未开始发送的自动任务；手动任务保留。离线和暂停期间不补发旧内容，重连从新的复制继续。未收到回执标记结果未确认，不自动重发。旧单设备配置自动迁移，v1 网络协议需要所有对端一起升级至 v2。

真实桌面复制粘贴、系统凭据持久化、LAN/VPN 延迟和睡眠唤醒仍需实机验证。Windows 远程访问配置不作为开工条件。

## 双机验证程序

两端先执行 `cargo build -p nooboard-core --example link_probe --features diagnostics --locked`。这个显式启用的诊断入口使用真实剪贴板和完整 core/network，身份及 SQLite 仅驻留内存；macOS 使用独立命名 pasteboard，Linux/Windows 必须使用隔离测试桌面。

`python3 scripts/test_two_hosts.py --help` 查看双机脚本参数。SSH 只控制测试进程和交换公开证书；同步数据通过指定 LAN/VPN 地址上的双向认证 TLS 1.3 连接传输。日志只包含测试数据的长度与 SHA-256，不输出剪贴板正文。`--listener local` 可让 Ubuntu 主动连接 Mac，不改变数据的双向同步能力。

macOS 可运行 `python3 -B scripts/test_three_instances.py` 验证三个独立后端和三个隔离原生 pasteboard 的全连接、双目标发送、回执及防回传。它不会操作系统剪切板，CI 的 macOS 任务也包含此验证。
