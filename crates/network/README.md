# nooboard-network

通过消息通信提供设备发现、身份配对、可信连接和内容传输。`Network` 是统一业务请求句柄，`NetworkService` 管理生命周期，`NetworkEvents` 独占事件接收；三者都由 `api` 提供，内部功能模块不对调用者开放。network 不依赖 clipboard、storage 或 core。

## 职责边界

| 层 | 职责 |
| --- | --- |
| `api.rs` | 定义生命周期所有者、请求句柄、单消费者事件接收端；统一导出公开数据类型 |
| 顶层 `runtime/` | 装配子 runtime，连接内部通道，聚合状态，监督退出并等待清理 |
| 功能模块的 `runtime` | 拥有该功能的实例、状态和任务，处理请求及内部事件 |
| 功能模块的 `model.rs` | 请求、结果、标识符和状态等数据定义；不创建后台任务 |
| 功能内部实现 | TLS、配对协议、mDNS、凭据、文件暂存等具体机制 |

目录本身不会产生实例。`NetworkService::start` 通过顶层 runtime 创建身份、监听器和四个子 runtime。identity 不需要常驻事件循环；它提供身份材料及凭据加载能力。

core 决定发送目标、自动/手动策略、是否接受内容、接收目录，以及如何应用到剪贴板。core 也负责持久化公开的可信设备记录。network 负责执行传输、验证身份和内容、维护连接，以及报告实际结果。文件暂存及私钥凭据属于 network；storage 继续只处理文本历史和文本配置。

## 文件结构

```text
src/
├── lib.rs                  # 私有模块声明；只执行 pub use api::*
├── api.rs                  # NetworkService / Network / NetworkEvents 与公开类型
├── options.rs              # 启动配置及约束
├── error.rs                # 公开错误类别与内部诊断
├── event.rs                # 需要上游处理的业务事件
├── runtime/
│   ├── mod.rs              # 装配、监督、关闭
│   └── model.rs            # NetworkStatus、ServiceState
├── identity/
│   ├── mod.rs              # 加载身份
│   ├── model.rs            # 身份选项、公开身份、可信设备记录
│   ├── material.rs         # 私钥/证书及稳定设备 ID
│   └── credentials/        # 凭据创建策略、系统 keyring 适配
├── discovery/
│   ├── runtime.rs          # 发现的启停、刷新和地址查询
│   ├── model.rs            # 附近设备、本机地址
│   ├── mdns.rs             # 未认证的 mDNS 地址提示
│   └── interfaces.rs       # 网卡枚举与监听地址筛选
├── pairing/
│   ├── runtime.rs          # 配对操作、状态和保存确认
│   ├── model.rs            # 配对 ID、阶段和快照
│   ├── endpoint.rs         # 配对监听与会话任务
│   ├── session.rs          # 一次性配对码流程
│   ├── crypto.rs           # PAKE 与加密信道
│   └── wire.rs             # 私有配对报文
├── connections/
│   ├── runtime.rs          # 信任、连接仲裁、独立重连
│   ├── dial.rs             # 有界连接尝试与跨轮地址轮换
│   ├── model.rs            # 连接快照
│   ├── session.rs          # 会话读写和心跳
│   ├── queue.rs            # 每个设备的有界发送队列
│   └── transport/          # 私有 TLS 1.3 传输及认证测试
└── transfer/
    ├── model.rs            # 发送请求、内容、操作 ID、结果
    ├── protocol.rs         # 私有 v3 报文、帧边界和校验
    ├── runtime/
    │   ├── mod.rs          # 状态所有者与任务生命周期
    │   ├── dispatch.rs     # API 请求和连接事件入口
    │   ├── sending.rs      # 提交、准备和取消发送
    │   ├── receiving.rs    # 接收决策、应用确认、报文路由
    │   └── state.rs        # 状态归并、期限和保留策略
    ├── outgoing.rs         # 单目标文件/图片发送流程
    ├── incoming.rs         # 接收、校验、落盘和应用确认
    └── files/
        ├── prepare.rs     # 固定发送源，所有目标共享相同字节
        ├── receive.rs     # 顺序写入、哈希校验及批次发布
        ├── filesystem.rs  # 安全文件名、租约、平台独占重命名
        └── tests.rs        # 真实临时目录测试
```

各功能目录的 `mod.rs` 只在 crate 内组织实现。调用者不需要导入 `pairing`、`connections` 或 `transfer`；需要显式标注类型时，从 crate 根导入相应公开类型。

## 工作流程

### 启动与关闭

`Options` → `NetworkService::start` → 加载身份 → 绑定监听器 → 启动各子 runtime → 返回 `(NetworkService, Network, NetworkEvents)`。

服务需要在 Tokio runtime 内运行。默认使用系统凭据和固定监听端口；测试应显式选择 `Ephemeral`、关闭发现并使用回环临时端口。禁用 `native-credentials` 后，临时身份仍可运行，系统身份明确返回凭据错误。凭据丢失以外的读取失败、损坏或写入失败不会静默切换身份。配置中的展示名称及监听地址在启动时确定；更改这些选项需要关闭后重启。

| 对象 | 持有的能力 | 共享方式 |
| --- | --- | --- |
| `NetworkService` | 停止信号和监督任务；`shutdown(self)` 等待清理 | 由 core 持有，不可克隆 |
| `Network` | 请求通道、状态快照与订阅；业务方法都在这里 | 可克隆给多个任务，共享同一运行实例 |
| `NetworkEvents` | 需要上游处理的事件队列；`next_event(&mut self)` | 移交给一个事件处理任务，不可克隆 |

`NetworkService::shutdown(self).await` 通知子 runtime 停止并等待监听、会话、配对、发现和文件任务清理。它不依赖请求句柄或事件接收端被释放。文件提交一旦开始会等待其实际结果，已开始的请求仍可能返回成功；新的请求和仍在等待队列容量的请求返回 `Stopped`。事件接收等待被唤醒并返回 `None`，不再交付尚未处理的队列内容。请求句柄仍可读取最终状态。

`NetworkService` 的 `Drop` 发出停止信号；需要确认资源已释放时必须等待 `shutdown`。释放部分或全部 `Network` 句柄不会关闭服务。释放 `NetworkEvents` 也不关闭服务，此后需要上游处理的新入站工作会被拒绝；已交付或排队的工作继续遵守原来的超时、取消和落盘结果语义。

### 配对与信任

1. `start_pairing(addresses)` 返回配对 ID；接收端得到 `PairingOffered`，决定是否 `accept_pairing`。
2. 接收端从状态获取一次性代码，发起端调用 `submit_pairing_code`。发现信息本身不构成信任。
3. 验证成功后收到 `PairingVerified { peer, .. }`。这是公开证书、设备 ID 和地址组成的记录，可以序列化为文本。
4. 调用者保存该记录，再调用 `complete_pairing(id, true)`；保存失败则传 `false`。只有确认保存后，network 才激活本端信任。
5. 重启时通过 `Options.trusted_peers` 恢复记录。`trust_peer` 也可安装调用者已保存并授权的记录。`revoke_peer` 立即撤销运行时信任；调用者同时负责删除持久化记录。

已结束的配对无法取消，接口返回 `NotFound` 并保持原来的完成状态或失败原因。

两端保存不是跨设备数据库事务；一端在另一端失败前已经保存时，其公开记录可能仍存在，后续可重新配对或撤销。私钥只由 identity/credentials 管理，不通过 API 返回。

### 发送与接收

```text
core ── SendRequest ──> api ── 请求通道 ──> transfer runtime
                                              │
                         固定内容 → 每目标队列 → 已认证连接
                                              │
core <── 状态快照 <─────────────────────────────┘

已认证连接 → 接收文字 ───────────────────> ContentReady
           → 图片/文件 Offer → IncomingOffer → core 接受/拒绝
                                      接受 → 接收与校验 → ContentReady
core 应用内容 → complete_incoming → network 回执 → 发送端更新结果
```

`IncomingOffer` 和 `ContentReady` 同时携带本端决策 ID 与传输 ID，供上游可靠关联状态。接收方可以用 `cancel_incoming(peer, transfer_id)` 请求取消尚未交付应用的接收工作，已经应用或发布的结果不会回滚。

文字直接交付 `ContentReady`；图片和文件先通过 `IncomingOffer` 请求决策。接受文件必须提供目录，接受图片使用 `directory: None`。network 检查 PNG 签名和大小，不负责图像解码或剪贴板格式转换。

文件发送前暂存一份固定内容，再分发给各目标。源文件变化会使准备失败。接收端使用临时目录、顺序偏移及 SHA-256 校验；单文件独占重命名，多文件发布为一个完整子目录，已有文件不会被覆盖。图片只在临时目录中校验，最终作为 PNG 字节交给上游。当前限制为文字 1 MiB、图片 64 MiB、最多 256 个普通文件、单文件 2 GiB、批次 8 GiB。

`send` 返回 ID 只表示接受操作。发送到离线或未就绪的目标会得到该目标的失败状态；其他目标继续执行。`Applied` 只来自接收方应用成功的明确确认；`Saved` 表示文件已发布，不能据此推断已写入剪贴板。超时或失联可能得到 `Unconfirmed`，network 不自动重发。尚保留的操作可以用迟到的真实结果消除不确定性。`cancel_transfer` 取消整个发送操作，`cancel_delivery(peer, id)` 只取消一个发送目标。取消不能撤销已发生的应用或落盘。

`QueuePolicy::ReplaceTail(key)` 只合并相邻、同 key、尚未开始发送的文字；`Append` 形成不可跨越的队列边界。是否合并由调用者决定。连接重新建立后不会重放旧内容。每轮连接尝试最多 10 秒，并为每个设备轮换下一轮的起始地址，避免前面的失效地址长期阻塞后面的可用地址。

### 事件与状态

`NetworkEvents::next_event(&mut self)` 使用有界、单消费者工作队列，承载配对确认、接收决策和内容应用任务。队列满时拒绝新工作；已发布的文件仍保留真实的 `Saved` 结果。正文不进入状态广播，也不出现在默认 Debug 输出中。

`Network::subscribe` 返回最新状态的 watch 订阅，多个观察者可以独立订阅，中间进度可能合并。它不是审计日志；传输按结果产生顺序保留最多 64 条已结束记录，迟到确认会刷新保留顺序；配对保留有限的结束记录。需要长期历史的调用者应自行保存。队列容量、并发操作和对端数量均有限制。

## 调用示例

下面展示公开入口和文字应用回执。示例中的“应用函数”由 core 提供；生产环境应持续消费事件，并为配对、图片及文件提供对应处理。

```rust,no_run
use nooboard_network::{
    ApplicationOutcome, IdentityOptions, NetworkEvent, NetworkService,
    Options, ReceivedContent, SendRequest,
};

# async fn example() -> nooboard_network::Result<()> {
let (service, network, mut events) = NetworkService::start(Options {
    identity: IdentityOptions::Ephemeral,
    listen: "127.0.0.1:0".parse().unwrap(),
    pairing_listen: "127.0.0.1:0".parse().unwrap(),
    discovery: false,
    ..Options::default()
}).await?;

// 请求句柄可克隆；事件接收端整体移动到一个处理任务。
let requests = network.clone();
let consumer = tokio::spawn(async move {
    while let Some(event) = events.next_event().await {
        if let NetworkEvent::ContentReady { id, content: ReceivedContent::Text(text), .. } = event {
            // core 根据 clipboard 的真实应用结果回复。
            let applied = apply_text_in_core(text).await;
            let outcome = if applied { ApplicationOutcome::Applied } else { ApplicationOutcome::Failed };
            if requests.complete_incoming(id, outcome).await.is_err() {
                break;
            }
        }
    }
});

if let Some(peer) = network.status().connections.iter().find(|p| p.accepting) {
    let _operation = network.send(SendRequest::text(vec![peer.peer.clone()], "hello")).await?;
}

// 用户退出时直接关闭；consumer 仍持有请求句柄并等待事件也不会阻碍关闭。
service.shutdown().await?;
consumer.await.expect("event consumer panicked");
assert_eq!(network.status().state, nooboard_network::ServiceState::Stopped);
# Ok(())
# }
# async fn apply_text_in_core(_text: String) -> bool { false }
```

## 验证

```sh
cargo test -p nooboard-network --all-features --locked
cargo test -p nooboard-network --no-default-features --locked
cargo clippy -p nooboard-network --all-targets --all-features --locked -- -D warnings
cargo clippy -p nooboard-network --all-targets --no-default-features --locked -- -D warnings
python3 scripts/check_architecture.py
```

普通测试通过公共 API 使用临时身份、临时文件和回环 TCP，覆盖 TLS 信任、配对重试和保存确认、多目标隔离、文本/PNG/文件传输、取消与迟到回执、撤销信任、队列压力，以及句柄仍存活时的关闭、所有者 Drop 和事件接收端释放。凭据创建策略使用替代读写函数测试，普通测试不会访问系统钥匙串。

mDNS 需要支持组播回环的环境，单独执行：

```sh
cargo test -p nooboard-network --locked mdns_discovers_updates_and_withdraws_a_peer_without_trusting_it -- --ignored
```

系统凭据弹窗、Linux Secret Service 会话以及跨机 LAN/VPN 仍需对应环境验收。当前保留 TLS/PAKE 和 v3 传输协议，旧 Rust 接口已删除；core 已接入新的服务所有者、请求句柄和独占事件接收端；全工作区验证见根目录 README。
