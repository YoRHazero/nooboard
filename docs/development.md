# 开发与贡献

本文面向源码开发者。安装和使用应用请从 [README](../README.md) 开始；全部文档见 [文档索引](README.md)。

## 环境

- Rust 工具链由仓库根目录的 `rust-toolchain.toml` 固定，目前为 1.93.0，包含 rustfmt 与 clippy。
- 完整桌面工程需要 Node.js 22.12 或更新版本；CI 使用 22.14.0。
- macOS 需要 Xcode Command Line Tools；Windows 需要 MSVC C++ Build Tools 与 Windows SDK。
- Ubuntu 的原生桌面依赖与 CI 保持一致：

```sh
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential libssl-dev libayatana-appindicator3-dev librsvg2-dev patchelf xdg-utils
```

仅构建后端库不需要 Node.js。原生剪贴板功能需要图形会话和系统凭据服务；测试环境与隔离方式见 [测试文档](testing.md)。

## 启动开发环境

从仓库根目录进入桌面工程：

```sh
cd apps/desktop
npm ci
npm run desktop:dev
```

Tauri 会启动 Vite 和原生宿主。宿主使用真实剪贴板、配对配置、历史和系统凭据，运行前应清楚它与浏览器预览的区别。

仅查看界面可以运行：

```sh
npm run dev
```

打开控制台提供的本机地址。普通浏览器使用脚本化示例，不连接真实设备，不写系统剪贴板。预览与原生应用使用同一前端公共接口；预览通过不等于业务或跨平台验收通过。

## 本地打包

在 `apps/desktop` 执行：

```sh
npm run desktop:build
```

未指定交叉编译 target 时，产物位于仓库根目录的 `target/release/bundle/`。显式指定 target 时，路径为 `target/<target>/release/bundle/`。各平台由本机工具链构建，不能把一次 macOS 构建当作 Windows/Linux 打包验证。

macOS 仅生成本机测试 app，可使用临时签名：

```sh
APPLE_SIGNING_IDENTITY=- npm run desktop:build -- --bundles app
```

产物是 `target/release/bundle/macos/nooboard.app`。临时签名不等于开发者签名或 Apple 公证。构建完成后可在仓库根目录运行 `codesign --verify --deep --strict target/release/bundle/macos/nooboard.app` 检查包完整性。

## 工程职责

```text
apps/desktop        React 页面、桌面运行时、Tauri 宿主
crates/core         用户配置、设备策略、同步与跨库流程
crates/identity     APP 身份、签名与用途隔离的本地加解密
crates/clipboard    原生剪贴板
crates/network      nooboard 配对、发现与内容传输
crates/storage      文字历史和文本配置
crates/terminal     独立终端、命令依赖、SSH 转发和 SFTP（由 Core 装配）
scripts             架构检查、测试工具与发布工具
```

桌面宿主只依赖 core；Core 按 feature 的 domain/application/ports/adapters 分层，由 app 装配 clipboard、network、storage、identity、terminal。Core 将 Identity 的签名能力适配注入 Network，将加解密能力适配到 Terminal 配置，底层库不直接互相依赖。独占服务所有者管理生命周期，可克隆句柄提交请求、读取或订阅状态。释放界面或句柄不等于停止服务。

| 修改内容                     | 首先阅读                                                                       |
| ---------------------------- | ------------------------------------------------------------------------------ |
| 配置、同步策略、历史协调     | [core 接口与持久化](../crates/core/README.md)                                  |
| 系统剪贴板                   | [clipboard 接口与平台实现](../crates/clipboard/README.md)                      |
| 配对、设备发现、普通文件传输 | [network 接口与协议](../crates/network/README.md)                              |
| 数据库与迁移                 | [storage 接口](../crates/storage/README.md)                                    |
| APP 身份、签名与秘密加密     | [identity 接口与迁移约定](../crates/identity/README.md)                       |
| 终端、命令依赖、SSH 与 SFTP   | [terminal 接口与运行示例](../crates/terminal/README.md)                        |
| 页面、IPC、宿主集成          | [桌面架构](../apps/desktop/ARCHITECTURE.md)                                    |
| 品牌、图标和 3D 动画         | [资源维护文档](../apps/desktop/public/mascot/README.md)                        |

部分 crate README 由 `include_str!` 直接作为 Rust 文档引入，其中的示例属于公开接口契约。

## 修改 IPC 或文案

先修改 Rust DTO 与原生分发，再在 `apps/desktop` 执行：

```sh
npm run ipc:generate
npm run ipc:check
npm run architecture:check
```

生成物为 `ipc/schema.json` 与 `src/desktop/bridge/generated.ts`。同时更新预览实现；不要手工维护第二套接口定义。前端功能模块通过 `desktop/api.ts` 访问宿主，不直接调用 Tauri。

用户文案同时维护 `src/i18n/locales/zh-CN` 与 `en`，运行 `npm run i18n:check`。完整检查与原生验收命令见 [测试文档](testing.md)。

## 命令行后端示例

下列示例用于调试后端，不是用户安装流程。在仓库根目录运行：

```sh
cargo run -p nooboard-core --example backend -- --help
cargo run -p nooboard-core --example backend -- /path/to/private/nooboard.db my-device
```

第二个位置参数是稳定的系统凭据配置名。示例使用真实剪贴板和指定数据库，不应与生产实例共享测试配置。默认手动发送、允许接收、记录最多 1000 条且保留 30 天的文字历史。

| 命令                                               | 用途                         |
| -------------------------------------------------- | ---------------------------- |
| `discover` / `pair <address>` / `pairing`          | 发现、发起配对、查看交互状态 |
| `accept <id>` / `code <id> <code>` / `cancel <id>` | 接受、输入代码或取消当前配对 |
| `targets <noob_id>...` / `send [noob_id ...]`      | 设置目标、发送当前剪贴板     |
| `peer-auto <noob_id> on/off` / `auto` / `manual`   | 目标自动发送与全局模式       |
| `pause` / `resume` / `receive on/off`              | 收发控制                     |
| `history on/off` / `list [query]` / `copy <id>`    | 文字历史记录、查询与复制     |
| `delete <id>` / `clear`                            | 删除历史                     |
| `status` / `unpair <noob_id>` / `quit`             | 状态、解除配对、关闭         |

`name`、`listen` 和 `pair-listen` 修改启动配置，需重启生效；完整参数以 `--help` 与交互帮助为准。`pairing` 会显示本次配对码，不应把其交互输出收集为普通日志。

## 数据兼容性

应用配置文档由 core 管理，目前为 v4，兼容 v3，原样保留退役功能配置，也处理受支持的更早格式。SQLite 数据库 schema 由 storage 独立迁移；两者版本号含义不同。

未知版本或损坏数据应明确报错并保留原文，不通过重置掩盖问题。迁移及旧格式支持边界见 [core](../crates/core/README.md#持久化与兼容范围) 和 [storage](../crates/storage/README.md)。修改持久化格式需要覆盖保存、重启、迁移与失败回滚。

## 发布版本

工作流定义是构建平台与权限的依据：[CI](../.github/workflows/ci.yml)、[Release](../.github/workflows/release.yml)。推送分支会触发 CI，推送 `v*` 标签会额外触发 Release。

1. 确定新版本，统一更新根 `Cargo.toml`、桌面 `package.json`、`package-lock.json` 和 `tauri.conf.json`，同步 `Cargo.lock` 中六个 workspace 包的版本。
2. 执行测试文档中的相关检查，并通过版本验证脚本。下方 `vX.Y.Z` 需替换成实际的新标签。
3. 提交代码，创建新标签，推送提交及标签。已有发布标签不移动、不覆盖。

```sh
node scripts/check_release_version.mjs vX.Y.Z
node --test scripts/check_release_version.test.mjs scripts/publish_release.test.mjs
```

Release 在 Windows x64、macOS Apple Silicon、Ubuntu x64 分别构建 NSIS、DMG、deb 和 AppImage。所有平台构建成功后才收集四种安装包并发布 Release；只推送标签并不意味着安装包已经生成。

当前 Windows 安装包未签名，macOS 使用临时签名且未公证。自动更新元数据不由现有工作流上传。用户下载入口统一链接 GitHub Releases，避免在 README 中保留过期安装包地址。
