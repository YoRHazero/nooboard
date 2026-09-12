# nooboard

使用 Rust 开发的 macOS / Windows / Ubuntu 跨设备剪贴板工具，面向同一局域网或已有 VPN 中可直接连通的设备。

当前已实现第一轮后端与最小验证示例，尚未完成真实 Mac ↔ Windows 双设备验收。桌面界面尚未开发。

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

底层库之间不直接互相依赖。Tauri、React、TypeScript 桌面程序，以及图片和文件手动传输留到后续轮次。

## 验证计划

工具链固定为 Rust 1.93.0，无需 Node.js 或前端依赖。Windows 构建需要 MSVC C++ Build Tools 与 Windows SDK，macOS 需要 Xcode Command Line Tools。

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

两端启动后，各自执行 `export mac.der` 或 `export windows.der`，交换公开证书文件并核对终端显示的 SHA-256 指纹。然后分别输入：

```text
# Mac：监听本机 LAN/VPN 地址
pair windows.der <Windows指纹> listen <Mac地址>:24816
# Windows：连接 Mac
pair mac.der <Mac指纹> connect <Mac地址>:24816
```

两端都确认后才建立双向认证连接。示例命令按空白分词，证书文件路径暂不支持空格；监听端需要放行所选端口。

- `auto` / `manual`：切换发送模式；`send`：明确发送当前文字。
- `pause` / `resume`：暂停或恢复同步；`receive on/off`：独立控制接收。
- `history on/off`：控制文字历史；`list [关键词]`：明确显示历史正文。
- `copy <id>` / `delete <id>` / `clear`：重新复制、删除或清空历史。
- `status` / `unpair` / `quit`：查看状态、解除配对、关闭后端。

每条文字最多 1 MiB，保留 Unicode、空白和原有换行，拒绝嵌入 NUL。图片、文件及已识别的敏感剪贴板格式会被跳过。断线与暂停不排队补发；重连后从新的复制继续同步。`Sent` 表示传输写入，`Applied` 才表示对端已写入剪贴板。

真实桌面复制粘贴、系统凭据持久化、LAN/VPN 延迟和睡眠唤醒仍需实机验证。Windows 远程访问配置不作为开工条件。

## 双机验证程序

两端先执行 `cargo build -p nooboard-core --example link_probe --features diagnostics --locked`。这个显式启用的诊断入口使用真实剪贴板和完整 core/network，身份及 SQLite 仅驻留内存；macOS 使用独立命名 pasteboard，Linux/Windows 必须使用隔离测试桌面。

`python3 scripts/test_two_hosts.py --help` 查看双机脚本参数。SSH 只控制测试进程和交换公开证书；同步数据通过指定 LAN/VPN 地址上的双向认证 TLS 1.3 连接传输。日志只包含测试数据的长度与 SHA-256，不输出剪贴板正文。`--listener local` 可让 Ubuntu 主动连接 Mac，不改变数据的双向同步能力。
