# nooboard 阶段 3 完成报告（P2P 多节点实时同步，LAN 自动发现）

## 1. 阶段目标达成情况
阶段 3 已完成“最小可运行闭环”：已具备 P2P 同步基础架构、协议、去重、CLI `sync` 命令接入，并通过编译与单元测试。

当前状态结论：
1. 代码层面已落地 P2P 架构（无中心 Hub）。
2. 已明确并实现“无离线补发”语义。
3. 已实现 mDNS 自动发现基础能力，并保留 `--peer` 手动兜底。
4. 已实现网络下行事件“先判重，再决定是否 set”的关键顺序。
5. 多机 LAN 实测与三节点联调尚未在本阶段报告内完成闭环验证。

## 2. 实际范围与非目标

### 实际范围
1. 新增 `nooboard-sync` crate，包含 `protocol/discovery/transport/engine` 四层。
2. 同步消息使用 WebSocket + JSON（`serde`）。
3. 使用 `origin_device_id + origin_seq` 持久化去重。
4. 数据库新增 `sync_seen_events` 去重表。
5. CLI 新增 `sync` 命令参数：
   - `--device-id`
   - `--listen`
   - `--token`
   - `--peer`（可多次）
   - `--no-mdns`
6. 引擎实现本地上行广播与远端下行处理主链路。

### 非目标（本阶段未接入）
1. 离线补发（replay/watermark）。
2. 端到端加密。
3. GUI。
4. 图片/文件等非文本同步。

## 3. A-H 执行结果（实际）

### 任务 A：新增同步 crate 与依赖
完成结果：
1. 新增 crate：`/Users/zero/study/rust/nooboard/crates/nooboard-sync`。
2. 接入依赖：`tokio`、`tokio-tungstenite`、`futures-util`、`serde`、`serde_json`、`mdns-sd`、`tracing`。
3. 模块拆分完成：`protocol` / `discovery` / `transport` / `engine`。
4. `cargo check --workspace` 通过。

### 任务 B：协议与鉴权
完成结果：
1. 已定义 `HelloMessage`、`SyncEvent`、`WireMessage`。
2. WebSocket 首包 `hello` 校验 token，不匹配直接拒绝。
3. 协议编解码单元测试已补齐并通过（`nooboard-sync`）。

### 任务 C：扩展存储层支持网络去重
完成结果：
1. `sql/schema.sql` 新增 `sync_seen_events`。
2. `nooboard-storage` 新增 repository 接口：
   - `mark_seen_event(origin_device_id, origin_seq, seen_at) -> bool`
   - `latest_content() -> Option<String>`
3. 新增测试覆盖：
   - 去重幂等（同事件第二次返回 `false`）
   - `latest_content` 返回最新文本

### 任务 D：实现 LAN 自动发现
完成结果：
1. 已实现 mDNS 注册与浏览：服务类型 `_nooboard._tcp.local.`。
2. 可将发现到的 peer 地址推送给传输层连接队列。
3. 已处理 `0.0.0.0` 监听时的广播地址问题（改为自动发布可解析地址）。

### 任务 E：实现 P2P 传输层
完成结果：
1. 已实现 WebSocket 入站监听。
2. 已实现对手工/发现 peers 的出站连接。
3. 已实现断线后自动重连（固定退避）。
4. 当前限制：尚未完成“同 peer 严格仅保留一个活跃连接”的最终策略。

### 任务 F：同步引擎接入 CLI
完成结果：
1. CLI 已新增 `sync` 命令参数骨架并可解析。
2. 引擎已接入并行主循环：
   - 本地 watch -> 事件化 -> 广播
   - 远端收包 -> 判重 -> 条件 `set` -> 入历史
3. 已支持 `Ctrl+C` 退出。

### 任务 G：回环与重复压制
完成结果：
1. 已实现“判重优先、`set` 后置”的远端处理顺序。
2. 已见事件直接丢弃，不会再次应用。
3. 已实现短抑制窗口（remote set 后短时抑制同内容本地回传）。

### 任务 H：测试与联调
完成结果：
1. 已完成并通过：
   - 协议编解码单元测试（`nooboard-sync`）
   - `mark_seen_event` 幂等测试（`nooboard-storage`）
2. 未完成：
   - 三节点集成测试自动化
   - 同 LAN 多机手工联调记录

## 4. 阶段 3 文件职责（实际）

1. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/protocol.rs`  
   协议结构、版本、JSON 编解码。

2. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/discovery.rs`  
   mDNS 广播与发现、peer 地址上报。

3. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/transport.rs`  
   WebSocket 入站/出站、hello/token 校验、收发通道与重连。

4. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/engine.rs`  
   同步主流程编排、远端判重顺序、回环抑制。

5. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/repository.rs`  
   `mark_seen_event` 与 `latest_content` 实现。

6. `/Users/zero/study/rust/nooboard/sql/schema.sql`  
   `sync_seen_events` 表结构。

7. `/Users/zero/study/rust/nooboard/crates/nooboard-cli/src/main.rs`  
   `sync` 命令参数与引擎启动入口。

## 5. 实际验证记录

1. `cargo check --workspace`：通过。  
2. `cargo test -p nooboard-storage`：通过（5 tests passed）。  
3. `cargo test -p nooboard-sync`：通过（2 tests passed）。  
4. `cargo run -p nooboard-cli -- sync --help`：参数骨架可用。  

说明：当前运行环境对网络绑定有限制，直接启动 `sync` 实例出现 `Operation not permitted`，因此本报告不包含同机多端口/多机实网联调结论。

## 6. DoD 对照结论

1. `sync` 命令可在单节点启动并监听端口：部分通过（代码就绪，当前环境未完成实网验证）。
2. 同 LAN 两节点可自动发现并连通：未验证。
3. 支持三节点在线同步同一事件：未验证。
4. 无补发语义下，重连后可继续接收新事件：部分通过（代码实现，缺实网验证）。
5. 同一事件在多节点全连接场景下仅应用一次：部分通过（持久化判重实现，缺三节点验证）。
6. 网络下行事件先判重，再决定是否 `set`：通过。
7. `history` 可看到同步后的文本记录：部分通过（代码路径已接入，缺联调验证）。
8. `cargo check --workspace` 与 `cargo test -p nooboard-sync`：通过。

## 7. 阶段 3 收尾结论
阶段 3 的“代码落地与最小闭环”已完成；“LAN 多节点联调证据与集成测试补齐”作为下一轮收尾工作继续推进。
