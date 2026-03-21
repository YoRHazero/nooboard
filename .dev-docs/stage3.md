# nooboard Stage3 开发计划（P2P 同步与文件传输）v4.1

## 1. 当前基线与边界
1. 当前 workspace 基线为 Stage2。
2. Stage3 目标是在现有基线上增加多设备 P2P 实时同步能力（含文本与文件）。
3. Stage3 新增同步能力必须与 `nooboard-storage` 协作，但同步 crate 本身不能依赖 `nooboard-storage`。
4. 协作方式采用 Channel 适配器模式：`nooboard-sync` 通过 `mpsc/watch` 通道与上层交互，由 `nooboard-app` 负责组装。

## 2. Stage3 目标
1. 剪贴板同步：支持多设备 P2P 实时同步剪贴板文本。
2. 文件传输：支持多设备 P2P 传输文件（流式分块，不将整文件加载进内存，支持大文件），并支持接收端显式接受/拒绝。
3. 优先级调度：文件传输过程中，保证文本消息和心跳包优先发送，不阻塞交互。
4. 网络基础：TCP + TLS（临时内存证书，客户端跳过验证）+ mDNS 自动发现 + 手动 Peer + `Hello -> Challenge -> Auth`。
5. 持久化标识：`noob_id` 持久化，作为节点唯一标识。

## 3. 架构设计
### 3.1 Crate 拆分与依赖
1. 新增 `crates/nooboard-sync`。
2. 配置解耦：`nooboard-sync` 仅定义配置结构体，文件 IO 由 `nooboard-app` 处理。
3. 接口暴露：通过 `SyncEngineHandle` 暴露 Channels。
4. 约束：`cargo tree -p nooboard-sync` 不得出现 `nooboard-storage`。

### 3.2 配置模型
定义于 `nooboard-sync/src/config.rs`：

```rust
pub struct SyncConfig {
    pub enabled: bool,
    // 基础网络配置：监听地址、token、超时等

    // 传输限制与安全
    pub max_packet_size: usize, // 建议 8MB，用于防止 OOM
    pub file_chunk_size: usize, // 建议 64KB - 256KB
    pub file_decision_timeout_ms: u64, // 建议 30000

    // 文件安全沙箱
    // 所有接收文件必须落在此目录下，也作为 .tmp 接收区
    pub download_dir: PathBuf,
    pub max_file_size: u64, // 例如 10GB

    // 由上层读取 noob_id 文件后注入
    pub noob_id: String,
}
```

补充说明：
1. `noob_id` 文件路径由上层配置（如 `identity.noob_id_file`）决定，`nooboard-app` 负责读取或生成 UUID 并注入到 `SyncConfig.noob_id`。
2. `download_dir` 必须在启动阶段确保存在，并作为接收安全根目录。

### 3.3 传输与协议（Protocol）
1. 传输层分帧：`LengthDelimitedCodec`（TCP codec 层称 Frame）。
2. 应用层消息统一命名为 `Packet`，不再使用 `Frame`。
3. 序列化：`serde + bincode`。
4. 风险控制：`bincode` 对 schema 变化敏感，必须在握手阶段严格校验 `protocol_version`。
5. 协议门禁（更新）：连接处于未认证状态时，只允许 `Packet::Handshake(...)`；收到 `Packet::Ping`、`Packet::Pong`、`Packet::Data(...)` 一律直接拒绝并断开。

```rust
#[derive(Serialize, Deserialize, Debug)]
pub enum Packet {
    Handshake(HandshakePacket),
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },
    Data(DataPacket),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum HandshakePacket {
    Hello {
        protocol_version: u16,
        noob_id: String,
    },
    Challenge { nonce: String },
    AuthResponse { hash: String },
    AuthResult { ok: bool },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum DataPacket {
    ClipboardText {
        id: String,
        content: String,
    },
    FileStart {
        transfer_id: u32,
        file_name: String,
        file_size: u64,
        total_chunks: u32,
    },
    FileDecision {
        transfer_id: u32,
        accept: bool,
        reason: Option<String>,
    },
    FileChunk {
        transfer_id: u32,
        seq: u32,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    FileEnd {
        transfer_id: u32,
        checksum: String,
    },
    FileCancel {
        transfer_id: u32,
    },
}
```

### 3.4 握手与认证
1. 连接建立后，客户端先发 `HandshakePacket::Hello { protocol_version, noob_id }`。
2. 服务端首先校验 `protocol_version`，不匹配立即断开，不进入数据层处理。
3. 服务端生成单次 `nonce` 并发送 `Challenge`。
4. 服务端将 `nonce` 与当前 socket 绑定并放入 `pending_challenges`，附带超时信息。
5. 客户端计算 `HMAC(token, nonce)` 并发送 `AuthResponse`。
6. 服务端验证通过后返回 `AuthResult { ok: true }`，失败返回 `ok: false` 并断开。
7. 强约束：无论认证成功、失败还是超时，`pending_challenges` 中对应条目都必须被清理，避免内存泄漏。

### 3.5 连接建立与去重
1. 支持 mDNS 自动发现与手动 peer（地址:端口）并存。
2. mDNS 记录携带 `noob_id`。
3. 强约束：连接去重规则固定为 `noob_id` 小的节点主动连接 `noob_id` 大的节点。
4. 若 `noob_id` 相同，视为冲突，拒绝连接并记录错误。

### 3.6 调度与流控（Traffic Shaping）
1. 通道分离：`Control Queue` 产生 `Packet::Handshake`、`Packet::Ping/Pong`；`Data Queue` 产生 `Packet::Data(...)`（文本与文件块）。
2. 调度规则：每轮循环先消费 `Control Queue`，仅当 `Control Queue` 为空时才消费 `Data Queue`。
3. 文件块发送须定期 `yield`，避免占满 Data Queue。

### 3.7 接收端状态机
1. `Packet::Handshake(_)` 交给 `HandshakeStateMachine`。
2. `Packet::Ping/Pong` 直接处理并更新活跃时间。
3. `Packet::Data(ClipboardText)` 发送给上层应用。
4. `Packet::Data(FileStart/FileDecision/FileChunk/FileEnd/FileCancel)` 交给 `FileReceiverStateMachine`。

### 3.8 文件传输决策（FileDecision）
1. 发送端发出 `FileStart` 后进入 `PendingDecision` 状态。
2. 接收端必须在 `file_decision_timeout_ms` 内回复 `FileDecision`（`accept=true/false`）。
3. 强约束：发送端仅在收到 `FileDecision { accept: true }` 后才允许发送 `FileChunk`/`FileEnd`。
4. 若收到 `accept=false` 或超时未决策，发送端必须终止该传输并清理状态；已创建的 `.tmp` 文件必须删除。
5. 接收端可按策略自动拒绝（如大小超限、路径非法），也可由上层应用交互后返回决策。

### 3.9 同步语义与保活
1. 强约束：仅同步在线期间新事件，离线后重连不补发历史数据。
2. 已连接空闲时发送 `Ping`，收到 `Ping` 必须回 `Pong`。
3. `pong_timeout_ms` 超时则主动断开并重连。

### 3.10 文件安全
1. `FileStart.file_name` 必须清洗为纯文件名（`Path::file_name()`）。
2. 拒绝 `..`、`/`、`\` 等路径穿越符号。
3. 接收文件路径必须在 `download_dir` 下。
4. 限制 `max_packet_size`、`max_file_size`、`active_downloads`，防止资源耗尽。

### 3.11 接口设计（Channels）
```rust
pub struct SyncEngineHandle {
    pub text_tx: mpsc::Sender<String>,
    pub file_tx: mpsc::Sender<PathBuf>,
    pub event_rx: mpsc::Receiver<SyncEvent>,
    pub status_rx: watch::Receiver<SyncStatus>,
    pub shutdown_tx: broadcast::Sender<()>,
}

pub enum SyncEvent {
    TextReceived(String),
    FileDownloaded { path: PathBuf, size: u64 },
}
```

## 4. 实施步骤
### Phase 1: 基础设施
1. 创建 `crates/nooboard-sync`。
2. 实现 `config.rs`。
3. 实现 `protocol.rs`（`Packet` 定义 + 握手期门禁）。
4. 实现 `transport.rs`（TLS + LengthDelimitedCodec）。
5. 实现 `auth.rs`（HMAC challenge-response + nonce 清理机制）。

### Phase 2: 核心引擎
1. 实现 `discovery.rs`（mDNS）。
2. 实现 `connection.rs`：嵌套枚举优先级调度器 + 文件接收状态机（含 `FileDecision` 流程与路径清洗）。
3. 实现 `engine.rs`（连接去重、生命周期、重连）。

### Phase 3: 集成与验证
1. 改造 `nooboard-app`：读取 TOML 配置，管理 `noob_id` 文件读取/生成，确保 `download_dir` 存在（默认 `~/Downloads/nooboard`），桥接 `SyncEngineHandle`。
2. 实现 app 层接口与测试。
3. 联调测试：文本同步、大文件传输、`FileDecision` 接受/拒绝与超时、心跳优先、断网重连与清理。

## 5. 计划内文件清单
### 5.1 新增
1. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/Cargo.toml`
2. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/lib.rs`
3. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/config.rs`
4. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/protocol.rs`
5. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/transport.rs`
6. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/auth.rs`
7. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/discovery.rs`
8. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/engine.rs`
9. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/connection/mod.rs`
10. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/connection/actor.rs`
11. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/connection/file_handler.rs`
12. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/tests/p2p_file_transfer.rs`

### 5.2 修改
1. `/Users/zero/study/rust/nooboard/Cargo.toml`
2. `/Users/zero/study/rust/nooboard/crates/nooboard-app/src/config.rs`
3. `/Users/zero/study/rust/nooboard/crates/nooboard-app/src/service.rs`
4. `/Users/zero/study/rust/nooboard/configs/dev.toml`

## 6. 验收标准（DoD）
1. 协议命名统一：应用层消息统一使用 `Packet`。
2. 两台设备可同时进行文本同步和文件传输。
3. 握手阶段必须严格校验 `protocol_version`，版本不一致立即断开。
4. 强约束（更新）：未完成握手认证前，只允许 `Packet::Handshake(...)`；收到 `Ping/Pong/Data` 必须直接拒绝并断开。
5. 强约束：mDNS 去重按 `noob_id` 小连大规则执行，且可在自动发现与手动 peer 并存场景下稳定去重。
6. 强约束：Challenge 必须与 socket 绑定，且在成功/失败/超时三种路径都能释放内存。
7. 强约束：离线期间不缓存待补发事件，重连后只同步新事件。
8. 强约束：`FileStart` 后必须收到 `FileDecision { accept: true }` 才允许发送 `FileChunk`/`FileEnd`。
9. 强约束：`FileDecision { accept: false }` 或决策超时时，传输必须终止并清理状态与 `.tmp` 文件。
10. 路径安全：发送 `../../` 等恶意文件名时，接收端仍只能落在 `download_dir` 沙箱内。
11. 非阻塞体验：文件传输期间心跳正常，文本消息低延迟可达。
12. `noob_id` 可持久化复用，文件缺失时可自动生成并写入。
13. `nooboard-sync` 不依赖 `nooboard-storage`（依赖图检查通过）。
14. `cargo check` 与关键测试通过（至少 `nooboard-sync`、`nooboard-app`）。
