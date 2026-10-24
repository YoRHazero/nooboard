# 测试与实机验收

[开发与贡献](development.md) · [文档索引](README.md)

## 日常检查

在 `apps/desktop` 安装依赖后执行：

```sh
npm run format:check
npm run i18n:check
npm run architecture:check
npm run ipc:check
npm test
npm run build
```

再在仓库根目录执行：

```sh
cargo fmt --all -- --check
python3 scripts/check_architecture.py
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
node --test scripts/check_release_version.test.mjs scripts/publish_release.test.mjs
```

Tauri 的 `custom-protocol` 构建需要 `apps/desktop/dist`，所以完整 Rust 检查前先构建前端。IPC 生成使用 `--no-default-features`，不要求预先构建前端。

普通测试使用内存剪贴板、测试凭据、临时数据库与本机回环连接，不读写用户的真实密码库。测试环境必须允许绑定本机回环端口。Rust 文档示例也由 workspace 测试执行。

## 库级检查

各库 README 列出具体覆盖范围与命令：[core](../crates/core/README.md)、[clipboard](../crates/clipboard/README.md)、[network](../crates/network/README.md)、[storage](../crates/storage/README.md)、[identity](../crates/identity/README.md)、[terminal](../crates/terminal/README.md)。

Terminal 的独立测试使用本地 PTY、自建回环 SSH/SFTP 服务和临时文件，不启动真实 VPN、不连接实际设备。可单独执行 `cargo test -p nooboard-terminal --all-targets --locked`；实际服务器互操作与各平台进程清理仍需原生验收。

涉及可选后端或系统凭据时，另验证无默认 feature 的接口：

```sh
cargo test -p nooboard-storage --no-default-features --locked
cargo test -p nooboard-identity --no-default-features --locked
```

修改影响哪个边界，就运行对应检查；只有存在新的失败或未解决风险时才扩大或重复验证。

## 原生剪贴板

macOS 原生测试使用独立命名 pasteboard：

```sh
cargo test -p nooboard-clipboard --all-features --locked --test native -- --ignored --test-threads=1
```

Windows 上相同测试会改写运行会话的剪贴板，只在隔离测试桌面执行。

Linux 需要 Xvfb 与 sway；脚本创建隔离的 X11、Wayland 会话：

```sh
bash scripts/test_linux_native.sh
```

X11 使用 XFixes 监听 CLIPBOARD，支持 UTF-8、STRING 与 INCR；不处理 PRIMARY。Wayland 优先使用 `ext-data-control-v1`，其次 `wlr-data-control-v1`。只有基本 `wl_data_device` 的会话会明确报错。

默认有 `WAYLAND_DISPLAY` 时选择 Wayland，否则使用 `DISPLAY` 的 X11。测试时可用 `NOOBOARD_LINUX_BACKEND=x11` 或 `wayland` 明确选择。失败不会静默退回 XWayland，不把 SSH 转发的 X11 剪贴板当作远端原生桌面。

## 发现与多实例

mDNS 测试需要支持组播回环：

```sh
cargo test -p nooboard-network --locked mdns_discovers_updates_and_withdraws_a_peer_without_trusting_it -- --ignored
```

macOS 三实例脚本使用三个独立原生 pasteboard、内存身份与数据库，验证全连接、双目标发送、回执和防回传：

```sh
cargo build -p nooboard-core --example link_probe --features diagnostics --locked
python3 -B scripts/test_three_instances.py
```

跨机测试两端先构建同一 `link_probe`，再查看脚本参数：

```sh
python3 scripts/test_two_hosts.py --help
```

该脚本的 SSH 用于控制测试进程及交换公开证书；被测同步通过 LAN/VPN 上的双向 TLS 连接进行。日志只保留测试数据长度与摘要。`--listener local` 可改变连接发起方向，不改变内容的双向同步能力。Windows/Linux 原生剪贴板测试需在隔离桌面运行。

## 已有验证的边界

已有测试记录不代表 Windows/Linux 原生应用、系统凭据弹窗、外部 OpenSSH 服务、跨机网络或原生拖拽已经完成实机验收。CI 的具体结果以相应提交的 [GitHub Actions](https://github.com/YoRHazero/nooboard/actions) 为准，runner 测试也不能替代用户实际桌面环境。
