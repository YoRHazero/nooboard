# nooboard 阶段 3 开发计划（P2P 多节点实时同步，LAN 自动发现）

## 1. 阶段目标
在阶段 2 基础上实现去中心化同步：同一局域网内的 nooboard 节点可自动发现、建立 P2P 连接，并实时同步文本剪切板。

阶段 3 完成标准：
1. 支持多节点 P2P 在线互联（不依赖中心 Hub）。
2. 不做离线补发，断线期间事件允许丢失。
3. 支持局域网自动发现（mDNS/Bonjour）+ 手动 peer 兜底。
4. 网络下行事件先走数据库判重，再决定是否写本地剪切板（`set`）。
5. 保证无明显回环刷屏，且历史记录可查询。

## 2. 范围与非目标

### 范围
1. 仅同步 UTF-8 文本。
2. 使用 WebSocket 做节点间传输。
3. 每个节点同时具备入站监听与出站连接能力。
4. 使用 `origin_device_id + origin_seq` 做全网事件去重。
5. 同步链路与存储层解耦：网络侧通过 repository 完成判重与落库。

### 非目标
1. 不做离线补发（无 watermark/replay）。
2. 不做中心化 Hub。
3. 不做 GUI。
4. 不做端到端加密（仅最小 token 鉴权）。
5. 不做图片/文件同步。

## 3. 核心设计（基于本次讨论）

### 3.1 节点模型
每个节点同时运行：
1. `Peer Listener`：接收入站连接。
2. `Peer Connector`：连接发现到的其他节点。
3. `Sync Engine`：处理本地上行和远端下行。

### 3.2 发现与连接
1. 默认启用 mDNS 广播服务：`_nooboard._tcp.local`。
2. 默认启用 mDNS 浏览，自动发现同 LAN 设备。
3. 支持 `--peer <addr>` 手动指定对端。
4. 连接保活与重连（只恢复在线连接，不补发历史事件）。
5. 小规模节点默认采用全连接直发模型（不做多跳转发）。

### 3.3 消息模型
统一 `SyncEvent` 字段：
1. `version`
2. `origin_device_id`
3. `origin_seq`
4. `captured_at`
5. `content`

### 3.4 下行处理顺序（关键）
远端事件到达后按以下顺序处理：
1. 调用存储层 `mark_seen_event(origin_device_id, origin_seq, seen_at)`。
2. 若返回 `false`（已见过）则直接丢弃，不 `set`。
3. 若返回 `true`（首次见到）则比较内容：
   - 与最新历史内容相同：跳过 `set`（减少无效本地剪切板写入）。
   - 不同：执行本地 `set`。
4. 将该事件写入本地历史（按阶段 2 策略）。

### 3.5 回环控制
1. 主判重：`origin_device_id + origin_seq` 持久化去重。
2. 本地优化：远端首次事件仅在“内容不同”时才 `set`。
3. 可选补充：短抑制窗口（remote set 后短时忽略同内容本地 watch 事件）。

## 4. 任务拆解（执行顺序）

### 任务 A：新增同步 crate 与依赖
目标：建立阶段 3 实现载体。

操作：
1. 新增 crate：`nooboard-sync`。
2. 增加依赖（建议）：
   - `tokio`
   - `tokio-tungstenite`
   - `futures-util`
   - `serde` + `serde_json`
   - `tracing`
   - `mdns-sd`
3. 模块划分：
   - `protocol`
   - `discovery`
   - `transport`
   - `engine`

完成标准：
1. `cargo check --workspace` 通过。

### 任务 B：协议与鉴权
目标：固定传输边界。

操作：
1. 定义 `SyncEvent` 与 `hello` 消息结构。
2. 握手或首包校验 token。
3. 添加协议编解码单元测试。

完成标准：
1. 协议编解码测试通过。
2. 无 token/错误 token 连接被拒绝。

### 任务 C：扩展存储层支持网络去重
目标：网络事件可幂等处理。

操作：
1. 更新 `sql/schema.sql`，新增：
   - `sync_seen_events(origin_device_id TEXT NOT NULL, origin_seq INTEGER NOT NULL, seen_at INTEGER NOT NULL, PRIMARY KEY(origin_device_id, origin_seq))`
2. 在 `nooboard-storage` 新增接口：
   - `mark_seen_event(origin_device_id, origin_seq, seen_at) -> bool`
   - `latest_content() -> Option<String>`（用于“是否需要 set”判断）
3. 增加对应测试：
   - 幂等插入
   - 重复事件返回 `false`

完成标准：
1. 同一网络事件最多通过一次。

### 任务 D：实现 LAN 自动发现
目标：自动探查同网段节点。

操作：
1. 节点启动时注册 mDNS 服务（附带 `device_id` 与监听端口）。
2. 浏览同服务类型，实时更新 peer 列表。
3. 输出发现/下线日志。

完成标准：
1. 两台设备同 LAN 运行后可自动发现彼此。

### 任务 E：实现 P2P 传输层
目标：建立多节点实时通信。

操作：
1. WebSocket 入站监听。
2. 对发现到 peers 发起出站连接。
3. 连接去重（同 peer 仅保留一个活跃连接）。
4. 连接断开自动重连（指数退避）。

完成标准：
1. 2~3 节点可同时在线收发消息。

### 任务 F：同步引擎接入 CLI
目标：提供可运行命令。

操作：
1. 新增命令：
   - `sync --device-id <id> --listen <ip:port> --token <token> [--peer <addr> ...] [--no-mdns]`
2. 引擎并行任务：
   - 本地 watch -> 生成本地事件 -> 广播 peers
   - 远端收包 -> 存储判重 -> 条件 `set` -> 入历史
3. 保持 `Ctrl+C` 优雅退出。

完成标准：
1. 命令可持续运行并支持多节点实时同步。

### 任务 G：回环与重复压制
目标：稳定多节点传播。

操作：
1. 对首次远端事件执行“判重优先、`set` 后置”。
2. 已见事件不应用。
3. 需要时增加短抑制窗口，避免远端 `set` 引发即时回传。

完成标准：
1. 三节点场景无明显回环刷屏。

### 任务 H：测试与联调
目标：验证核心路径。

操作：
1. 单元测试：
   - 协议编解码
   - `mark_seen_event` 幂等
2. 集成测试：
   - 三节点全连接场景下，同一事件只应用一次
3. 手工联调：
   - 两台或三台同 LAN 设备验证发现与实时同步

完成标准：
1. `cargo test -p nooboard-sync` 通过。
2. 联调通过。

## 5. 文件职责（阶段 3）

1. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/protocol.rs`  
   同步协议结构与版本。

2. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/discovery.rs`  
   mDNS 广播与发现。

3. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/transport.rs`  
   WebSocket 连接管理与消息收发。

4. `/Users/zero/study/rust/nooboard/crates/nooboard-sync/src/engine.rs`  
   同步主流程（上行/下行/判重/条件 set）。

5. `/Users/zero/study/rust/nooboard/crates/nooboard-storage/src/repository.rs`  
   `sync_seen_events` 与 `latest_content` 等接口实现。

6. `/Users/zero/study/rust/nooboard/sql/schema.sql`  
   同步去重表定义。

7. `/Users/zero/study/rust/nooboard/crates/nooboard-cli/src/main.rs`  
   `sync` 命令参数与启动逻辑。

## 6. DoD（验收清单）

1. `sync` 命令可在单节点启动并监听端口。  
2. 同 LAN 两节点可自动发现并连通。  
3. 支持三节点在线同步同一事件。  
4. 无补发语义下，重连后可继续接收新事件。  
5. 同一事件在多节点全连接场景下仅应用一次。  
6. 网络下行事件先判重，再决定是否 `set`。  
7. `history` 可看到同步后的文本记录。  
8. `cargo check --workspace` 与 `cargo test -p nooboard-sync` 通过。  

## 7. 风险与缓解

1. 风险：mDNS 在部分网络不可用。  
缓解：支持 `--peer` 手工指定地址。

2. 风险：无补发导致离线期间事件丢失。  
缓解：明确产品语义为“在线实时同步”。

3. 风险：远端 `set` 触发本地 watch 回传，出现重复与回环。  
缓解：`origin_device_id + origin_seq` 持久化去重 + 条件 `set` + 可选抑制窗口。

4. 风险：连接规模增大后开销上升。  
缓解：阶段 3 先面向小规模 LAN 节点（如 2~10 台）。

## 8. 建议验证命令（阶段 3 完成时）

1. `cargo check --workspace`  
2. `cargo test -p nooboard-sync`  
3. 设备 A：`cargo run -p nooboard-cli -- sync --device-id dev-a --listen 0.0.0.0:8787 --token dev-token`  
4. 设备 B：`cargo run -p nooboard-cli -- sync --device-id dev-b --listen 0.0.0.0:8787 --token dev-token`  
5. 设备 C（可选）：`cargo run -p nooboard-cli -- sync --device-id dev-c --listen 0.0.0.0:8787 --token dev-token`  
6. 任一设备复制文本，其他在线设备应实时收到并可通过 `history --limit 20` 查到记录。  
