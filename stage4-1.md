# nooboard Stage4 实施记录（第一轮）

日期：2026-02-27

## 完成项
1. 新建 `crates/nooboard-app`，并加入 workspace。
2. 实现 `AppService/AppServiceImpl`，覆盖：
   - 引擎生命周期：`start/stop/restart/status/connected_peers`
   - 剪切板业务 A1~A6
   - 文件业务包装：`send_file/respond_file_decision/subscribe_transfer_updates`
   - 广播配置事务：`set_mdns_enabled/add_manual_peer/remove_manual_peer/set_network_enabled`
3. 落地 v2 配置 schema 解析与映射（按功能分组），支持：
   - `recent_event_lookup_limit`（默认 50）
   - `node_id` 文件读取/初始化
   - `sync.network/auth/file/transport -> SyncConfig` 映射
4. 广播配置修改流程实现为：读 -> 改 -> 校验 -> 原子写 -> 重启 -> 失败回滚，且串行化执行。
5. `nooboard-sync` 事件字段改名：`source_device_id -> device_id`。
6. 升级 `configs/dev.toml`、`configs/prod.toml` 到 v2 分组结构。

## 测试与验证
1. `cargo check --workspace`：通过
2. `cargo test -p nooboard-app`：通过（7 tests passed）
3. `cargo test -p nooboard-sync --test p2p_file_transfer`：通过（5 tests passed）
