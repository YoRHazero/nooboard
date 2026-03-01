# nooboard Home / Quick Panel 高保真说明

更新时间：2026-03-02

## 1. 目标

基于 [stage5-wireframe.md](/Users/zero/study/rust/nooboard/stage5-wireframe.md)、[stage5-gpui-architecture.md](/Users/zero/study/rust/nooboard/stage5-gpui-architecture.md) 和 [stage5-visual-spec.md](/Users/zero/study/rust/nooboard/stage5-visual-spec.md)，进一步细化两个最关键的页面：

1. 主工作台 `Home`
2. 快捷面板 `Quick Panel`

这份文档回答的问题是：

- 用户打开窗口时第一眼看到什么
- 哪些区域最亮，哪些区域必须退后
- 信息密度如何分层
- 页面内有哪些轻交互和状态反馈

## 2. 总体原则

这两个页面承担的角色不同：

- `Home` 是控制台首页，负责建立“系统在线”的第一印象
- `Quick Panel` 是即时操作入口，负责在最短时间内完成一件事

因此：

- `Home` 可以更有舞台感
- `Quick Panel` 必须更短、更紧、更利落

## 3. Home 高保真说明

## 3.1 页面角色

`Home` 不是欢迎页，也不是报表页。

它的角色是：

- 打开主工作台后的系统驾驶舱
- 把运行状态、待处理项、最新活动和高频动作集中到一个界面
- 让用户在 3 秒内完成一次整体扫描

## 3.2 页面框架

建议 `Home` 采用三层结构：

```text
Top Metrics
Core Overview + Quick Actions
Activity Timeline + Active Transfers
```

垂直比例建议：

- 第一层：`18%`
- 第二层：`42%`
- 第三层：`40%`

## 3.3 Top Metrics

顶部 4 张卡片是第一视觉层。

顺序建议：

1. `Sync Status`
2. `Online Peers`
3. `Pending Files`
4. `Today History`

### 每张卡片结构

```text
Label
Primary Value
Secondary Hint
Status Accent
```

### 视觉规则

- 卡片高度统一
- `Primary Value` 大字号、高对比
- `Label` 小号、次级文本
- 卡片顶部可以有一条极淡状态色高光
- 激活中的核心卡片允许带一点 glow

### 各卡的重点

`Sync Status`
- 这是最重要的卡
- 允许使用绿色状态点或小型环形指示
- `Running` 时带轻微脉冲

`Online Peers`
- 数字为主
- 次级信息显示 `manual peers` 数量或最近连接变化

`Pending Files`
- 如果大于 `0`，用琥珀色突出
- 是驱动用户动作的核心指标

`Today History`
- 保持克制
- 更像系统统计，不抢主焦点

## 3.4 Core Overview

第二层左侧是 `System Core Panel`，这是页面视觉重心。

### 结构

```text
Panel Title
Large Status Ring
Runtime Facts
Mini Network Summary
```

### 状态环

状态环不是大型图表，而是一个稳定、简洁的系统指示器。

内容建议：

- 中心文字：`Running` / `Stopped` / `Error`
- 外环颜色：随状态切换
- 外环轻微呼吸，仅在运行时开启
- 错误态不闪烁，只改为红色静态高亮

### Runtime Facts

列出 4 到 6 个关键事实：

- `network_enabled`
- `mdns_enabled`
- `desired_state`
- `actual_sync_status`
- `manual_peers`
- `connected_peers`

这些信息要用技术化排版：

- label 左
- value 右
- value 可用 mono 字体

### Mini Network Summary

在面板底部放一条小摘要：

- 最近连接变化
- 最近错误概况
- 当前 session / runtime 状态

这一块字体更小，作为补充信息。

## 3.5 Quick Actions

第二层右侧是 `Quick Actions`，视觉权重要次于 `System Core Panel`，但操作入口要足够明显。

### 建议动作

1. `Send Clipboard`
2. `Open Quick Panel`
3. `View Recent History`
4. `Open Transfers Inbox`

### 视觉结构

- 每个动作是一张扁平卡片或宽按钮
- 左侧图标，右侧文字
- 主动作用 `Primary`
- 其他用 `Secondary`

### 交互

- hover 时卡片边框提亮
- 按下后立即反馈
- 不做复杂展开

## 3.6 Recent Activity Timeline

底部左侧是动态信息层。

### 结构

每条活动项包含：

- 时间
- 类型
- 主信息
- 次信息

例如：

```text
12:03
Text Received
device-a sent "alpha"
stored to history
```

### 视觉规则

- 左侧细时间线
- 每种类型一个小图标或状态点
- 新活动进入时有一次极淡高亮
- 列表整体保持高密度，但每行仍有足够留白

### 类型配色

- `TextReceived`：冷青
- `TransferFinished`：青绿
- `ConnectionError`：红色
- `FileDecisionRequired`：琥珀

## 3.7 Active Transfers

底部右侧是当前传输概览。

### 每项结构

```text
File Name
Progress Bar
Bytes / Total
Speed
ETA
```

### 视觉规则

- 文件名优先
- 进度条细而长
- 完成度数字和速度用 mono 字体
- 进度前景允许轻微流动感

### 空状态

无传输时不要空白，显示一条轻描述：

`No active transfers`

并配一个低对比图标。

## 3.8 Home 的视觉层级

建议层级顺序：

1. `System Core Panel`
2. `Top Metrics`
3. `Pending Files` 相关状态
4. `Quick Actions`
5. `Recent Activity`
6. `Active Transfers`

这能避免首页像“六块平均卡片拼盘”。

## 3.9 Home 的动效

允许的动效：

- 状态环低频脉冲
- 新活动项淡入
- 传输条缓慢流动
- 卡片 hover 微提亮

禁止：

- 大面积粒子背景
- 快速闪烁告警
- 指标数字不断跳动

## 4. Quick Panel 高保真说明

## 4.1 页面角色

`Quick Panel` 是一块“飞行面板”。

它不是第二个小主窗口，而是一个完成短动作的工具：

- 快速发送文本
- 处理待确认文件
- 回看最近几条历史

## 4.2 整体外观

窗口建议：

- 比主工作台更亮一层
- 背景更干净
- 层级更少
- 阴影更集中

它应该像从系统里“弹出”的一块操作面板，而不是常规应用页。

## 4.3 面板结构

```text
Compact Title
Segmented Tabs
Focused Content
Persistent Footer
```

垂直比例建议：

- 标题区：`12%`
- Tabs：`10%`
- 内容区：`68%`
- Footer：`10%`

## 4.4 Compact Title

标题栏信息保持最少：

- Logo / App 名
- `Running` 状态
- `Inbox` 数量

视觉要求：

- 不堆额外 badge
- 整体横向紧凑
- 标题栏更像设备状态条

## 4.5 Segmented Tabs

使用胶囊式切换：

- `Send`
- `Inbox`
- `Recent`

规则：

- 激活 tab 明显高亮
- 未激活 tab 只保留文字和淡底
- tab 宽度均匀

## 4.6 Send Tab

这是快捷面板默认页。

### 结构

```text
Short Intro
Editor
Target Selector
Primary Action
Result Feedback
```

### 视觉重点

- 大输入框是绝对主角
- `Send` 按钮是唯一强主按钮
- 目标选择只保留最短路径，默认 `All`

### 输入框表现

- 比主工作台里的编辑器更紧凑
- 圆角更明显
- 内边距充足
- 占位符直接告诉用户能做什么

占位文案建议类似：

`Paste or type a message to broadcast`

### 结果反馈

发送结果放在按钮下方或底部：

- `Sent`
- `Dropped: NoEligiblePeer`
- `Dropped: NetworkDisabled`

不建议只弹 toast，因为快捷面板关闭前用户需要确认结果。

## 4.7 Inbox Tab

这是快捷面板里最重要的系统事件入口。

### 卡片结构

每一项待确认文件包含：

- 文件名
- 来源节点
- 文件大小
- `Accept`
- `Reject`

### 视觉规则

- 文件卡片之间留白明确
- 如果还有错误项，错误块和文件块分开
- `Accept` 用主按钮
- `Reject` 用危险次按钮

### 行为

- 点击 `Accept` 后立即进入处理中状态
- 处理中状态替换按钮为小进度提示
- 完成后该项淡出

## 4.8 Recent Tab

这是一个迷你历史面板。

### 内容范围

- 最近 `5 ~ 10` 条
- 每条只显示一行摘要
- 右侧一个 `Copy` 或 `Apply` 动作

### 视觉规则

- 不要显示过多元数据
- 更像快捷列表，而不是历史页缩略图

### 跳转

底部固定一个：

`Open Full History`

点击后打开主工作台并跳到 `History`。

## 4.9 Footer

Footer 始终存在，承担两个作用：

1. 告诉用户这是临时面板
2. 提供进入主工作台的统一出口

建议内容：

- 左侧：`Open Workspace`
- 右侧：`Esc to close`

视觉规则：

- 上边界一条极淡分隔线
- 背景略深于内容区

## 4.10 Quick Panel 的层级

建议层级顺序：

1. 输入框或待处理文件卡片
2. 主操作按钮
3. tab 高亮
4. 页脚入口
5. 次要状态文本

它的重心必须高度集中，不能像主工作台那样多中心。

## 4.11 Quick Panel 的动效

允许：

- 呼出时轻微 scale + fade
- tab 切换平滑过渡
- 待处理项处理完成后的淡出

禁止：

- 复杂的层叠滑出动画
- 反复脉冲的按钮
- 多个区域同时运动

## 5. 两个页面的关系

`Home` 和 `Quick Panel` 不是同一个内容的大小变体。

区别应当非常明确：

| 页面 | 定位 | 信息量 | 视觉重心 | 主要用途 |
| --- | --- | --- | --- | --- |
| `Home` | 驾驶舱 | 高 | 系统状态与活动 | 观察和调度 |
| `Quick Panel` | 飞行面板 | 低 | 当前动作 | 快速完成任务 |

因此：

- `Home` 可以有多块信息并行存在
- `Quick Panel` 一次只允许一个主要动作占据注意力

## 6. 可直接抽象的高保真组件

建议后续优先抽这几个共享组件：

1. `StatusRing`
2. `MetricCard`
3. `ActionCard`
4. `ActivityTimelineItem`
5. `TransferMiniCard`
6. `QuickPanelSection`
7. `PendingFileCard`
8. `InlineResultBanner`

这些组件一旦定下来，整体风格会非常稳定。

## 7. 下一步建议

如果继续推进，最合理的两个方向是：

1. 直接开始实现 `theme.rs` 与共享视觉组件
2. 再补一份 `stage5-*.md`，把 `Home` 和 `Quick Panel` 分别写成组件级 wireframe + 状态枚举
