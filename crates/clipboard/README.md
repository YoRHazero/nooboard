# Clipboard

独立于网络、存储和界面的原生剪贴板服务。提供文本、图片和本地文件引用，支持 macOS、Windows、X11 和 Wayland，不依赖 arboard。

## 公共接口

```rust,no_run
use nooboard_clipboard::{ClipboardService, Options, Payload};

# async fn example() -> nooboard_clipboard::Result<()> {
let (service, clipboard) = ClipboardService::start(Options::default()).await?;
let snapshots = clipboard.subscribe();
let health = clipboard.subscribe_status();

let current = clipboard.read().await?;
let receipt = clipboard.write(Payload::Text("hello".into())).await?;

service.shutdown().await?;
# Ok(())
# }
```

- `ClipboardService` 拥有专用原生线程。显式 `shutdown()` 等待原生资源释放；释放所有者会发送停止信号，但不会阻塞调用线程。
- `Clipboard` 是可克隆的异步调用句柄。服务停止后，已有句柄的请求会失败。
- `Payload` 只表示可发布的数据。`ReadState` 区分数据、空剪贴板和跳过原因。
- `subscribe()` 返回最新观察的 `Option<Snapshot>`，初始 `None` 表示尚未观察。快速的中间变化可能合并；初始内容是否参与自动同步由应用决定。
- `subscribe_status()` 单独报告启动、可用、暂时失败和停止状态。暂时失败不会覆盖最后一次有效快照。
- `Snapshot.revision` 是单个服务实例内的观察序号，不是操作系统版本号。不同服务、不同启动周期的序号不能比较。
- `Origin::Application` 表示通过当前服务实例确认的写入；其余内容为 `Origin::External`，包括启动时的已有内容。平台原生版本变化后，旧写入关联失效。
- 文件列表是本地路径引用；文件读取、暂存、传输和落盘属于上层。图片存储有界，字段私有；构造函数检查编码字节大小，`validate()` 完整解码校验。`rgba()` 和 `png()` 是同步纯计算，应在后台任务调用。写入服务会在后台准备并校验图片。
- Windows DIB 必须同时满足原生读取上限（256 MiB 加 DIB 头部）和通用图片上限 `Limits.image_bytes`（默认 64 MiB）；任一超限都跳过。通用上限按 DIB 封装成 BMP 后的实际字节数检查，即使转换为 PNG 后很小，也可能得到 `Skipped(TooLarge)`。

## 内部职责

```text
api → runtime → backend → OS
          ↘ formats ↗
```

`api.rs` 负责启动握手、调用句柄和生命周期所有者。`runtime/` 拥有有界请求队列、响应通道、取消判断、观察序号、健康状态和操作期限。

`backend/` 的私有接口只接收原生读写操作。后端在线程内创建和释放，可以持有非 Send 的原生对象。`begin` 开始操作，`poll` 有界推进操作并维护原生事件，`wait` 等待系统事件或中立唤醒信号。后端不引用公共请求类型和响应通道。

X11 的读取请求窗口、INCR 分块进度，以及 Wayland 的数据管道，均由进行中的操作持有。主循环可以在等待数据时处理停止、取消和期限。取消读取会释放对应请求资源。写入完成后，Linux 后端继续为其他应用提供剪贴板数据。

`formats/` 统一敏感标记、文件、图片、文本的选择顺序及校验。平台目录负责原生格式映射；Windows DIB 和句柄管理留在 Windows 后端。固定尺寸预览由 core 生成。

## 调度与关闭

请求按队列顺序进入操作阶段，每轮仍会推进原生事件。图片准备一次只处理一个任务，不持有原生句柄。丢弃请求 future 会跳过排队请求并取消进行中的读取；已提交给操作系统的写入不回滚。

停止信号独立于普通队列，关闭不会排在积压的读写请求后面。运行时拒绝新请求，结束排队工作、释放原生操作，最后销毁后端。纯 CPU 图片准备可能在关闭信号后短暂继续运行，结果被丢弃；它不持有原生资源。操作期限覆盖原生操作及读取重试，无法强制中断阻塞在系统库内部的调用。

## 验证

```sh
cargo test -p nooboard-clipboard --all-features --locked
cargo clippy -p nooboard-clipboard --all-targets --all-features --locked -- -D warnings
python3 scripts/check_architecture.py
```

公共运行时用私有测试后端验证取消、关闭、队列饱和、版本回绕和错误恢复。原生测试通过公共接口验证变化通知、来源、Unicode、较大文本、图片和文件引用。

macOS 原生测试使用独立命名 pasteboard：

```sh
cargo test -p nooboard-clipboard --all-features --locked --test native -- --ignored --test-threads=1
```

Windows 上相同命令需要隔离桌面会话。Linux 使用仓库根目录的 `bash scripts/test_linux_native.sh` 创建隔离 X11、Wayland 会话。
