# nooboard 阶段 3 完成报告（P2P 多节点实时同步，LAN 自动发现）

## 1. 阶段目标与当前结论
阶段 3 已完成可运行实现，并在同机多实例场景完成主链路联调。

当前结论：
1. 已实现去中心化 P2P 同步（无 Hub）。
2. 已实现“在线实时同步，无离线补发”语义。
3. 已实现 mDNS 自动发现与 `--peer` 手动兜底。
4. 已实现“远端事件先判重，再决定是否 set”的关键顺序。
5. 已针对联调暴露问题完成二次优化（地址过滤、连接方向规则、重复连接优雅关闭）。

## 2. 已交付能力（代码落地）

### 2.1 新增 crate 与模块
新增 `/Users/zero/study/rust/nooboard/crates/nooboard-sync`，模块如下：
1. `protocol.rs`：`HelloMessage`、`SyncEvent`、`WireMessage`、JSON 编解码。
2. `discovery.rs`：mDNS 注册/浏览、peer 发现、地址筛选与优选。
3. `transport.rs`：WebSocket 入站/出站、鉴权握手、重连、连接去重。
4. `engine.rs`：本地上行与远端下行业务编排（判重优先）。

### 2.2 存储层扩展
1. `sql/schema.sql` 新增 `sync_seen_events` 表。
2. `nooboard-storage` 新增：
   - `mark_seen_event(origin_device_id, origin_seq, seen_at) -> bool`
   - `latest_content() -> Option<String>`
3. 对应单测已补齐并通过。

### 2.3 CLI 接入
`nooboard-cli` 新增命令：
`sync --device-id --listen --token [--peer ...] [--no-mdns]`

## 3. 关键技术实现（以最新代码为准）

### 3.1 鉴权与协议
1. WebSocket 首包必须是 `hello`，并校验 token。
2. token 不匹配时拒绝连接。
3. 协议版本与字段固定，满足阶段 3 MVP。

### 3.2 远端事件处理顺序（强约束）
在 `engine.rs` 中，远端消息严格按以下顺序：
1. `mark_seen_event(...)` 持久化判重。
2. 若已见过则丢弃。
3. 若首次见到，比较 `latest_content()`：
   - 相同：跳过 `set`
   - 不同：执行本地 `set`
4. `insert_text_event(...)` 入历史。

### 3.3 mDNS 地址策略优化（新增）
针对同机与 LAN 场景，`discovery.rs` 已实现：
1. 场景判定：`listen` 是否为 loopback。
2. 同机模式：仅保留 loopback 地址，优先 `127.0.0.1`。
3. LAN 模式：过滤 `loopback/unspecified/multicast`，并过滤 `fe80::/10`（IPv6 link-local）。
4. 同一 peer 仅上报“最佳地址”，且地址未变化则不重复上报。

### 3.4 连接风暴与噪声优化（新增）
针对全连接双向互拨造成的重复连接，`transport.rs` 已实现：
1. 连接方向规则（按 `device_id` 字典序）：
   - `local < peer` 才主动出站连接。
   - 入站只接受 `peer < local`。
2. 同一 `device_id` 仅保留一个活跃连接（`PeerSlot`）。
3. 方向拒绝/重复连接/自连场景采用优雅 `Close` 帧，减少 `reset without closing handshake` 噪声。

## 4. 本轮验证记录

### 4.1 自动化验证
1. `cargo check --workspace`：通过。
2. `cargo test -p nooboard-storage`：通过（5 passed）。
3. `cargo test -p nooboard-sync`：通过（2 passed）。

### 4.2 手工联调（同机三实例）
基于 `/Users/zero/study/rust/nooboard/stage3-validation.md` 执行：
1. Case B（手工 peer）已验证：A `set` 后 B/C `history` 可见同步文本。
2. Case C（mDNS 自动发现）已验证：日志可见 `connected peer dev-b/dev-c`、`accepted peer ...`，并可传播事件。
3. 联调中发现的高噪声连接问题已完成代码修复（见 3.3、3.4）。

说明：cargo 全局缓存目录存在权限告警（`pcap-2.2.0/Cargo.toml permission denied`），不影响构建与测试结果。

## 5. DoD 对照（阶段 3）

1. `sync` 命令可在单节点启动并监听端口：通过。  
2. 同 LAN 两节点可自动发现并连通：部分通过（同机 mDNS 已通过，跨设备 LAN 待补证据）。  
3. 支持三节点在线同步同一事件：通过（同机三实例验证通过）。  
4. 无补发语义下，重连后可继续接收新事件：部分通过（代码路径具备，待按文档补完整证据）。  
5. 同一事件在全连接场景下仅应用一次：部分通过（存储判重+方向规则已实现，待补统计证据）。  
6. 网络下行事件先判重，再决定是否 `set`：通过。  
7. `history` 可看到同步后的文本记录：通过（同机联调已验证）。  
8. `cargo check --workspace` 与 `cargo test -p nooboard-sync`：通过。  

## 6. 阶段 3 收尾与遗留

已完成：
1. 阶段 3 代码与最小闭环交付。
2. 同机三实例联调可运行并具备可复现验证文档。

遗留（建议在阶段 4 前补齐）:
1. 跨设备 LAN（至少两台机器）联调证据。
2. Case D（离线无补发 + 重连收新事件）与 Case E（重复应用次数统计）的记录化结果。
3. 将 `stage3-validation.md` 的 DoD 表格填充为最终验收记录。
