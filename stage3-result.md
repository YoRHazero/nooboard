# Stage3 开发总结

## 目标与范围
- 严格以 `stage3.md (v4.1)` 为依据完成 Stage3。
- 应用层消息统一使用 `Packet`。
- 完成握手约束、mDNS 去重、挑战生命周期、文件传输状态机、超时清理、异步决策、手动断连能力等要求。
- 对 `connection` 与 `engine` 做高内聚拆分与结构瘦身。

## 核心实现

### 1) 协议与握手
- 握手前仅允许 `Packet::Handshake`，其他包直接拒绝。
- 握手流程完整覆盖：`Hello -> Challenge -> AuthResponse -> AuthResult`。
- 统一以 `Packet` 作为应用层消息封装。

### 2) 认证挑战生命周期
- Challenge 与 `socket_id` 绑定。
- 成功 / 失败 / 超时路径均释放 challenge 记录。

### 3) mDNS 与去重
- 新增 mDNS 发现实现。
- 去重规则按 `node_id` 字典序：小连大（small connect large）。

### 4) 文件传输状态机
- 发送与接收严格遵循：
  - `FileStart -> FileDecision(accept=true) -> FileChunk/FileEnd`
- 拒绝、超时、取消场景均触发终止并清理 `.tmp`。
- 离线重连不补发历史传输。

### 5) 接收端超时增强
- 除 `file_decision_timeout_ms` 外，新增 `transfer_idle_timeout_ms`。
- 一旦进入文件流程，超过 idle 时间无 chunk 或状态变化，会主动清理状态与文件句柄。

### 6) 异步确认模式
- 文件接收改为异步等待调用方确认：
  - 引擎发出 `SyncEvent::FileDecisionRequired`
  - 调用方通过 `decision_tx` 回传 accept/reject

### 7) 控制通道（手动断连）
- 新增 `SyncControlCommand::DisconnectPeer { peer_node_id }`。
- `SyncEngineHandle` 暴露 `control_tx`，可手动断开指定 peer。

## 模块重构

### connection 目录重构
- 调整为：
  - `connection/actor.rs`：事件循环与路由协调
  - `connection/sender.rs`：发送状态机（只生产数据，不负责发送）
  - `connection/receiver.rs`：接收状态机（纯文件业务）
  - `connection/stream.rs`：`Framed` + 控制/数据优先队列
  - `connection/path.rs`：路径安全与命名工具
- `FileSender` 与 `stream` 解耦：
  - `FileSender` 内部 outbox 产出 `Packet`
  - actor 统一投递到 stream
- `OutgoingTransfer` 的操作收敛为 `FileSender` 私有方法。

### engine 目录瘦身（由单文件拆分）
- 由 `engine.rs` 拆为：
  - `engine/mod.rs`（门面导出）
  - `engine/types.rs`（公开类型）
  - `engine/runtime.rs`（主循环）
  - `engine/peers.rs`（PeerRegistry 状态集中管理）
  - `engine/connect.rs`（连接调度与建连）
  - `engine/ingress.rs`（accept/discovery 转发）
  - `engine/handshake.rs`（握手细节）
- 新增 `PeerRegistry` 聚合 `peers/connecting/discovered` 三类状态，降低 runtime 耦合。

## 错误体系重构
- 错误统一抽到 `crates/nooboard-sync/src/error.rs`。
- 分层结构：
  - `SyncError`
  - `ConnectionError`（含 `Transport`、`FileReceive`）
  - `TransportError`（含 `Protocol`、Rustls、Io）
  - `ProtocolError`
  - `DiscoveryError`
  - `FileReceiveError`

## 依赖与边界
- 依赖通过 `cargo add` 纳入 workspace 管理。
- 保持 `nooboard-sync` 不依赖 `nooboard-storage`。

## 关键验证结果
- `cargo check`：通过
- `cargo test -p nooboard-sync`：通过
  - 单元测试 + 集成测试（含 p2p 文件传输、拒绝清理、控制断连）
- `cargo test -p nooboard-cli`：通过

## 当前风险与后续建议
- 当前实现已满足 Stage3 硬约束并通过测试。
- 后续可继续补充：
  - 更细粒度并发压力测试（多 peer 同时传输）
  - 对 handshake 异常分支的故障注入测试
  - 控制通道的批量管理能力（如断开全部、冻结重连窗口）
