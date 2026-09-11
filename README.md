# nooboard

使用 Rust 开发的 macOS ↔ Windows 跨设备剪贴板工具，面向同一局域网或已有 VPN 中可直接连通的设备。

当前已实现第一轮后端与最小验证示例，尚未完成真实 Mac ↔ Windows 双设备验收。桌面界面尚未开发。

## 第一轮开发

四个独立于界面的 Rust 库，通过自动化测试与最小命令行示例验证文字同步及本地文字历史：

```text
core → clipboard
     → network
     → storage
```

- `core`：组装底层模块，协调设备、同步与历史业务。
- `clipboard`：封装 macOS、Windows 原生剪贴板 API；本库的实现不依赖 arboard。
- `network`：连接、协议与经过身份验证的加密传输。
- `storage`：历史记录与配置持久化。

底层库之间不直接互相依赖。Tauri、React、TypeScript 桌面程序，以及图片和文件手动传输留到后续轮次。

## 验证计划

工具链固定为 Rust 1.93.0，无需 Node.js 或前端依赖。Windows 构建需要 MSVC C++ Build Tools 与 Windows SDK，macOS 需要 Xcode Command Line Tools。

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 scripts/check_architecture.py
```

GitHub Actions 在 macOS 14 与 Windows Server 2022 runner 上执行构建、测试和架构检查；Windows 上使用 `python` 命令。普通测试使用内存剪贴板、临时数据库和本机 TLS 连接。

原生测试单独运行：macOS 使用独立 pasteboard，Windows 会改写测试会话剪贴板，仅在隔离测试会话中执行。CI 也单独运行这一步，结果仅代表 runner 会话。

```sh
cargo test -p nooboard-clipboard --locked -- --ignored --test-threads=1
```

## 最小示例

```sh
cargo run -p nooboard-core --example backend -- --help
cargo run -p nooboard-core --example backend -- /path/to/private/nooboard.db my-device
```

第二个参数是稳定的身份配置名；私钥保存在 macOS Keychain 或 Windows Credential Store。历史和设置保存在指定 SQLite 文件中，**历史目前为明文**，请使用当前用户的私有目录。默认仅手动发送、允许接收并记录文字历史，最多 1000 条、保留 30 天。

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
