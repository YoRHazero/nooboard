# nooboard GPUI / gpui-component 架构方案

更新时间：2026-03-02

## 1. 目标

基于 [stage5-wireframe.md](/Users/zero/study/rust/nooboard/stage5-wireframe.md)，把界面方案拆成可以直接落到 `GPUI` 和 `gpui-component` 的实现结构。

本文档回答四件事：

1. 双窗口如何组织
2. 每个窗口用什么组件拼
3. `nooboard-app` 的状态与事件如何进入 UI
4. 下一步如果开工，crate 和模块应该怎么划分

## 2. 设计原则

### 2.1 交互分层

- 托盘图标负责快操作
- 主工作台负责深操作
- 后台常驻不是“永远置顶”，而是“窗口可隐藏但进程和服务持续运行”

### 2.2 UI 分层

- `Workspace Window`：完整控制台
- `Quick Panel Window`：即时操作面板
- `Tray / Notification / Sheet`：打断式提醒与短流程处理

### 2.3 状态分层

- `AppService` 提供业务状态和事件流
- `AppStore` 保存快照、历史、传输、待处理事项
- `UiStore` 保存窗口、选中页、选中项、输入态

## 3. 官方组件边界

当前官方文档已经覆盖本方案需要的核心能力：

- `Sidebar`
- `Tabs`
- `Resizable`
- `Editor`
- `Table`
- `List`
- `VirtualList`
- `GroupBox`
- `DescriptionList`
- `Notification`
- `Sheet`
- `Settings`
- `Progress`
- `Input` / `NumberInput` / `Switch` / `Select`
- `TitleBar`

因此这套方案不需要自造基础导航组件，重点是做状态组织和页面组合。

## 4. 顶层窗口结构

## 4.1 应用根

```text
Application
└── AppController
    ├── AppRuntime
    │   ├── AppServiceImpl
    │   ├── EventSubscription task
    │   └── Tray / Notification bridge
    ├── WorkspaceWindow
    └── QuickPanelWindow
```

职责：

- `Application`：启动 GPUI 应用
- `AppController`：窗口管理、后台常驻、命令路由
- `AppRuntime`：连接 `AppService`，消费订阅事件，更新全局 store
- `WorkspaceWindow`：主工作台窗口
- `QuickPanelWindow`：快捷面板窗口

## 4.2 Root View

两个窗口都建议遵守同一套 Root 结构：

```text
Root
├── TitleBar
├── WindowContent
├── NotificationLayer
└── SheetLayer
```

理由：

- `NotificationLayer` 处理连接错误、传输完成、发送结果
- `SheetLayer` 处理文件接受/拒绝、次级详情、快捷确认流

## 5. 主工作台组件树

## 5.1 Workspace Shell

```text
WorkspaceRoot
├── TitleBar
│   ├── AppLogo
│   ├── SyncStatusBadge
│   ├── PeerCountBadge
│   ├── InboxBadge
│   └── CommandInput
├── Resizable(Horizontal)
│   ├── LeftSidebar
│   │   ├── SidebarHeader
│   │   ├── SidebarMenu
│   │   │   ├── Home
│   │   │   ├── Clipboard
│   │   │   ├── History
│   │   │   ├── Peers
│   │   │   ├── Transfers
│   │   │   └── Settings
│   │   └── SidebarFooter
│   ├── MainContent
│   └── ActivityRail
└── StatusBar
```

组件映射：

- `TitleBar`
- `Sidebar`
- `Resizable`
- `Badge`
- `Input`
- `GroupBox`

## 5.2 MainContent 页面切换

```text
MainContent
└── match active_route
    ├── HomePage
    ├── ClipboardPage
    ├── HistoryPage
    ├── PeersPage
    ├── TransfersPage
    └── SettingsPage
```

建议路由枚举：

```rust
enum WorkspaceRoute {
    Home,
    Clipboard,
    History,
    Peers,
    Transfers,
    Settings,
}
```

## 5.3 ActivityRail

```text
ActivityRail
├── LiveFeedCard
├── PendingFileCardList
├── ErrorCardList
└── ActiveTransferMiniList
```

特点：

- 不跟随主页面切换
- 永远显示“系统正在发生什么”
- 承担常驻应用最重要的“即时可见性”

推荐组件：

- `List`
- `Scrollable`
- `Tag`
- `Progress`
- `Button`

## 6. 各页面实现拆分

## 6.1 HomePage

```text
HomePage
├── TopMetricsRow
│   ├── SyncMetricCard
│   ├── PeersMetricCard
│   ├── PendingFilesMetricCard
│   └── HistoryMetricCard
├── MiddleRow
│   ├── SystemCorePanel
│   └── QuickActionsPanel
└── BottomRow
    ├── RecentActivityTimeline
    └── ActiveTransfersPanel
```

推荐组件：

- `GroupBox`
- `DescriptionList`
- `Badge`
- `List`
- `Progress`

数据来源：

- `snapshot()`
- 事件订阅聚合后的 `activity feed`
- 当前内存中的 `transfer updates`

## 6.2 ClipboardPage

```text
ClipboardPage
├── ComposePanel
│   └── Editor
├── TargetPanel
│   ├── TargetModeSelect
│   └── PeerChecklist
└── ActionBar
    ├── BroadcastStatusInline
    ├── WriteLocalOnlyButton
    └── SendButton
```

推荐组件：

- `Editor`
- `Select`
- `Checkbox`
- `Button`
- `Alert`

动作映射：

- `Send` -> `apply_local_clipboard_change(LocalClipboardChangeRequest)`
- `Write Local Only`
  - 方案 A：调用 `apply_local_clipboard_change`，但目标设为空并以 UI 标记为本地动作
  - 方案 B：后续在 `nooboard-app` 增补显式本地写入接口

说明：

- 从当前 API 看，最稳妥的是仍走 `apply_local_clipboard_change`
- 发送结果必须显示 `BroadcastStatus`

## 6.3 HistoryPage

```text
HistoryPage
├── HistoryToolbar
├── Resizable(Horizontal)
│   ├── HistoryListPane
│   └── HistoryDetailPane
└── PaginationFooter
```

推荐组件：

- `Table` 或 `VirtualList`
- `DescriptionList`
- `Button`
- `Scrollable`

动作映射：

- 初始加载 / 继续加载 -> `list_history(ListHistoryRequest)`
- 复制回本地 -> `apply_history_entry_to_clipboard(event_id)`
- 重新广播 -> `rebroadcast_history_entry(RebroadcastHistoryRequest)`

数据结构建议：

```rust
struct HistoryState {
    records: Vec<HistoryRecord>,
    next_cursor: Option<HistoryCursor>,
    selected_event_id: Option<EventId>,
    loading: bool,
}
```

## 6.4 PeersPage

```text
PeersPage
├── TabBar
│   ├── Connected
│   ├── ManualPeers
│   └── Runtime
└── ActiveTabContent
```

### Connected tab

```text
ConnectedPeersView
└── PeerCardGrid
    └── PeerCard[]
```

### ManualPeers tab

```text
ManualPeersView
├── ExistingPeerList
└── AddPeerForm
```

### Runtime tab

```text
RuntimeView
├── DesiredStateControl
├── NetworkSwitch
├── MdnsSwitch
└── RuntimeSummary
```

动作映射：

- 开关同步状态 -> `set_sync_desired_state(SyncDesiredState)`
- 开关网络 -> `apply_config_patch(AppPatch::Network(SetNetworkEnabled))`
- 开关 mdns -> `apply_config_patch(AppPatch::Network(SetMdnsEnabled))`
- 增删 manual peers -> `apply_config_patch(AppPatch::Network(AddManualPeer / RemoveManualPeer))`

## 6.5 TransfersPage

```text
TransfersPage
├── TabBar
│   ├── Inbox
│   ├── Active
│   └── Completed
└── ActiveTabContent
```

### Inbox tab

```text
TransferInboxView
└── FileDecisionCard[]
```

### Active tab

```text
ActiveTransfersView
└── TransferProgressCard[]
```

### Completed tab

```text
CompletedTransfersView
└── TransferResultList
```

动作映射：

- 发送文件 -> `send_file(SendFileRequest)`
- 接受 / 拒绝文件 -> `respond_file_decision(FileDecisionRequest)`

数据来源：

- `AppEvent::Sync(SyncEvent::FileDecisionRequired { .. })`
- `AppEvent::Transfer(TransferUpdate)`

## 6.6 SettingsPage

```text
SettingsPage
├── StorageSettingsPanel
└── NetworkSettingsPanel
```

推荐组件：

- `Settings`
- `Form`
- `Input`
- `NumberInput`
- `Switch`

动作映射：

- 保存存储配置 -> `apply_config_patch(AppPatch::Storage(StoragePatch))`
- 网络长期配置也可落在这里，但实时控制仍以 `PeersPage/Runtime` 为主

## 7. 快捷面板组件树

## 7.1 Quick Panel Shell

```text
QuickPanelRoot
├── TitleBarLite
│   ├── AppLogo
│   ├── SyncStatusBadge
│   └── InboxBadge
├── TabBar
│   ├── Send
│   ├── Inbox
│   └── Recent
├── ActiveTabContent
└── FooterBar
    ├── OpenWorkspaceButton
    └── EscHint
```

推荐组件：

- `TitleBar`
- `Tabs`
- `Editor`
- `List`
- `Button`

## 7.2 Send tab

```text
QuickSendView
├── Editor
├── TargetSelect
└── SendButton
```

目标：

- 5 秒内完成发送
- 不展示复杂配置
- 默认 `Targets::All`

## 7.3 Inbox tab

```text
QuickInboxView
├── PendingFileList
└── CriticalAlertList
```

目标：

- 只保留需要立即动作的内容
- 点一次即可完成接受/拒绝，或跳转主工作台

## 7.4 Recent tab

```text
QuickRecentView
└── RecentHistoryMiniList
```

目标：

- 只显示最近 5 到 10 条
- 一键复制回剪贴板
- 可跳到完整历史页

## 8. 全局状态模型

## 8.1 AppStore

```rust
struct AppStore {
    snapshot: Option<AppServiceSnapshot>,
    subscription_state: SubscriptionState,
    connected_peers: Vec<ConnectedPeer>,
    pending_file_decisions: Vec<PendingFileDecision>,
    transfer_items: Vec<TransferItem>,
    recent_activity: Vec<ActivityItem>,
    history_cache: HistoryState,
    notifications: Vec<UiNotification>,
}
```

职责：

- 保存从 `snapshot()` 与事件订阅得到的业务状态
- 作为两个窗口的共享数据源
- 提供 selector 给页面读取

## 8.2 UiStore

```rust
struct UiStore {
    workspace_route: WorkspaceRoute,
    quick_panel_tab: QuickPanelTab,
    selected_history_event: Option<EventId>,
    selected_peer_ids: Vec<NoobId>,
    clipboard_draft: String,
    quick_send_draft: String,
    workspace_visible: bool,
    quick_panel_visible: bool,
}
```

职责：

- 保存纯 UI 状态
- 不直接承载业务对象
- 窗口之间共享部分输入态，但不要所有页面都强耦合

## 8.3 派生状态

建议把以下都做成 selector 或 view model，而不是每个页面重复计算：

- `sync_badge_model`
- `peer_count`
- `pending_inbox_count`
- `active_transfer_count`
- `latest_errors`
- `quick_recent_history`

## 9. 事件流设计

## 9.1 启动流程

```text
App launch
-> create AppServiceImpl
-> load snapshot()
-> desired state apply if needed
-> try subscribe_events()
-> populate AppStore
-> open Workspace or stay background
```

规则：

- 如果设置为后台启动，只显示托盘，不立即打开主窗口
- 如果 `actual_sync_status == Running`，启动事件订阅任务
- 如果未运行，不把它当错误，UI 显示为待启动态

## 9.2 订阅事件进入 UI

```text
EventSubscriptionItem
-> AppRuntime event handler
-> normalize to ActivityItem / TransferItem / PendingDecision
-> update AppStore
-> optionally raise Notification
-> optionally open QuickPanel or Workspace
```

映射规则：

- `TextReceived` -> recent activity + optional notification
- `FileDecisionRequired` -> pending inbox + notification + open quick panel inbox
- `ConnectionError` -> error feed + warning/error notification
- `TransferUpdate::Progress` -> active transfers
- `TransferUpdate::Finished/Failed/Cancelled` -> completed transfers + notification

## 9.3 文件待确认处理

```text
SyncEvent::FileDecisionRequired
-> store PendingFileDecision
-> show Notification
-> open QuickPanel(Inbox)
-> user Accept/Reject
-> respond_file_decision(...)
-> remove pending item after ack or final transfer event
```

## 9.4 发送文本处理

```text
user click Send
-> create LocalClipboardChangeRequest
-> apply_local_clipboard_change(...)
-> append result to history summary
-> show inline status
-> on error, also push notification
```

## 9.5 历史分页处理

```text
HistoryPage opened
-> if cache empty: list_history(limit, None)
-> append records
-> keep next_cursor
-> user load more
-> list_history(limit, next_cursor)
```

## 10. 页面与 API 对照表

| 页面/区域 | 读接口 | 写接口 |
| --- | --- | --- |
| 全局标题栏 | `snapshot()` | 无 |
| Home | `snapshot()` + event subscription | 无 |
| Clipboard | `snapshot()` | `apply_local_clipboard_change(...)` |
| History | `list_history(...)` | `apply_history_entry_to_clipboard(...)`, `rebroadcast_history_entry(...)` |
| Peers / Runtime | `snapshot()` | `set_sync_desired_state(...)`, `apply_config_patch(...)` |
| Transfers | event subscription | `send_file(...)`, `respond_file_decision(...)` |
| Settings | `snapshot()` | `apply_config_patch(...)` |
| ActivityRail | event subscription + snapshot | 无 |
| Quick Send | `snapshot()` | `apply_local_clipboard_change(...)` |
| Quick Inbox | event subscription | `respond_file_decision(...)` |
| Quick Recent | history cache | `apply_history_entry_to_clipboard(...)` |

## 11. 建议的数据模型补充

当前 `nooboard-app` 已足够驱动首版 UI，但为了让桌面端更顺手，后续可以考虑补充：

1. 历史全文搜索接口
2. “仅写本地剪贴板，不入广播流程”的显式接口
3. 传输记录分页查询接口
4. 持久化的待处理任务快照

这些不是首版阻塞项，但会提升主工作台的完整度。

## 12. crate 与模块建议

建议新增桌面 crate：

```text
crates/nooboard-desktop
├── Cargo.toml
└── src
    ├── main.rs
    ├── app.rs
    ├── controller
    │   ├── mod.rs
    │   ├── commands.rs
    │   ├── tray.rs
    │   └── windows.rs
    ├── runtime
    │   ├── mod.rs
    │   ├── service.rs
    │   ├── events.rs
    │   └── notifications.rs
    ├── state
    │   ├── mod.rs
    │   ├── app_store.rs
    │   ├── ui_store.rs
    │   └── selectors.rs
    ├── ui
    │   ├── theme.rs
    │   ├── workspace
    │   │   ├── mod.rs
    │   │   ├── shell.rs
    │   │   ├── home.rs
    │   │   ├── clipboard.rs
    │   │   ├── history.rs
    │   │   ├── peers.rs
    │   │   ├── transfers.rs
    │   │   └── settings.rs
    │   ├── quick_panel
    │   │   ├── mod.rs
    │   │   ├── shell.rs
    │   │   ├── send.rs
    │   │   ├── inbox.rs
    │   │   └── recent.rs
    │   └── shared
    │       ├── activity_rail.rs
    │       ├── badges.rs
    │       ├── cards.rs
    │       ├── peer_chip.rs
    │       └── transfer_row.rs
    └── bridge
        ├── mod.rs
        ├── mappers.rs
        └── actions.rs
```

模块原则：

- `runtime` 只处理服务与事件
- `state` 只处理 store
- `ui` 只处理视图组合
- `bridge` 负责 DTO 到 view model 的映射

## 13. 首批可实现范围

如果按最短路径做第一版，我建议优先落这 6 项：

1. 主工作台 shell
2. 快捷面板 shell
3. Home 页面
4. Clipboard 页面
5. ActivityRail
6. 文件待确认通知链路

这样最快能验证：

- 双窗口是否顺手
- 后台常驻是否合理
- 事件驱动 UI 是否稳定
- GPUI / gpui-component 是否足够承载核心流程

## 14. 下一步

下一步建议进入更接近实现的阶段，二选一：

1. 继续写 `stage5-` 文档，产出视觉规格和主题 token
2. 直接开始 `nooboard-desktop` crate 的 shell 骨架
