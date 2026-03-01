# nooboard Visual Spec

更新时间：2026-03-02

## 1. 目标

基于 [stage5-wireframe.md](/Users/zero/study/rust/nooboard/stage5-wireframe.md) 和 [stage5-gpui-architecture.md](/Users/zero/study/rust/nooboard/stage5-gpui-architecture.md)，定义 `nooboard` 桌面端的视觉语言。

这份文档只解决视觉层问题：

1. 整体气质是什么
2. 颜色、字体、圆角、边框、阴影怎么定
3. 哪些区域需要“未来感”，哪些区域必须克制
4. 动效如何服务常驻状态，而不是变成噱头
5. 后续如何映射到 `GPUI` / `gpui-component` 的主题与共享组件

## 2. 视觉定位

`nooboard` 不应该像网页后台，也不应该像游戏 HUD。

建议定位：

- 未来感桌面控制台
- 持续在线的数据工作台
- 有技术气质，但不赛博朋克过头

关键词：

- 深色
- 冷静
- 精密
- 常驻
- 轻发光
- 信息密度高，但层次清楚

反例：

- 大面积霓虹描边
- 过亮的蓝紫渐变
- 玻璃拟态过重导致可读性下降
- 满屏动画和跳动数字

## 3. 品牌气质

建议把 `nooboard` 做成“本地节点网络控制台”的感觉。

用户打开它时，第一感受应该是：

- 程序正在稳定运行
- 网络、历史、文件传输都在一个统一空间里
- 重点信息能被快速扫到
- 细节有科技感，但操作不花哨

## 4. 主题总览

## 4.1 主视觉方向

- 基底：深石墨蓝灰
- 强调色：冷青蓝
- 成功色：青绿
- 告警色：琥珀
- 错误色：偏珊瑚红

色彩策略：

- `80%` 视觉面积交给中性深色
- `15%` 交给功能色状态
- `5%` 交给强调色发光和焦点

这样未来感来自秩序和对比，不来自花哨配色。

## 4.2 色板

### 背景色

```text
bg.app            = #09111D
bg.canvas         = #0D1726
bg.panel          = #101C2D
bg.panel_alt      = #132235
bg.panel_muted    = #0F1A29
bg.elevated       = #16263A
bg.overlay        = rgba(6, 12, 20, 0.78)
```

### 前景色

```text
fg.primary        = #E8F1FA
fg.secondary      = #A8BBCC
fg.muted          = #71859B
fg.dim            = #5A6B7F
fg.inverse        = #08111B
```

### 强调与状态色

```text
accent.cyan       = #4DD7FF
accent.cyan_soft  = #7EE4FF
accent.green      = #40D39C
accent.amber      = #FFB84D
accent.red        = #FF6B6B
accent.blue       = #6AA7FF
```

### 边框色

```text
border.base       = rgba(138, 184, 222, 0.18)
border.strong     = rgba(138, 184, 222, 0.28)
border.focus      = rgba(77, 215, 255, 0.55)
border.danger     = rgba(255, 107, 107, 0.42)
```

### 发光色

```text
glow.cyan         = rgba(77, 215, 255, 0.22)
glow.green        = rgba(64, 211, 156, 0.18)
glow.amber        = rgba(255, 184, 77, 0.18)
glow.red          = rgba(255, 107, 107, 0.18)
```

## 4.3 状态颜色映射

| 状态 | 颜色 | 用途 |
| --- | --- | --- |
| `Running` | `accent.green` | 同步运行、健康连接 |
| `Starting` | `accent.cyan` | 启动中、等待事件流 |
| `Stopped` | `fg.muted` | 停止、空闲 |
| `Disabled` | `fg.dim` | 功能关闭 |
| `Warning` | `accent.amber` | 待处理文件、弱错误 |
| `Error` | `accent.red` | 连接失败、配置失败、致命问题 |

## 5. 字体系统

## 5.1 角色划分

需要两套字体角色：

- 界面正文：清晰、克制
- 技术字段：等宽、稳定

建议：

- UI 字体：`SF Pro` / `Inter` / 系统 UI 字体
- Mono 字体：`JetBrains Mono` / `SF Mono` / `Menlo`

如果后续想进一步拉开气质，可以仅在标题和状态标签上引入稍微更有性格的字重，而不是换一套夸张字体。

## 5.2 字级

```text
text.hero         = 28 / 34
text.title        = 22 / 28
text.section      = 16 / 22
text.body         = 13 / 18
text.small        = 12 / 16
text.mono         = 12 / 16
text.micro        = 11 / 14
```

规则：

- 主工作台页面标题用 `title`
- 卡片标题用 `section`
- 正文与列表默认 `body`
- 地址、`event_id`、速率、端口统一 `mono`

## 5.3 字重

```text
weight.regular    = 400
weight.medium     = 500
weight.semibold   = 600
weight.bold       = 700
```

建议：

- 常规文本只用 `400/500`
- 标题和关键信息用 `600`
- 避免全页面大面积 `700`

## 6. 空间与圆角

## 6.1 间距

建议使用 4px 基础网格：

```text
space.1           = 4
space.2           = 8
space.3           = 12
space.4           = 16
space.5           = 20
space.6           = 24
space.8           = 32
space.10          = 40
```

使用策略：

- 卡片内部间距：`16` 或 `20`
- 页面模块间距：`24`
- 大区域留白：`32`

## 6.2 圆角

```text
radius.sm         = 8
radius.md         = 12
radius.lg         = 16
radius.xl         = 20
radius.pill       = 999
```

规则：

- 按钮、输入框：`12`
- 面板卡片：`16`
- 快捷面板大容器：`20`
- badge 和状态胶囊：`pill`

## 7. 边框、面板与阴影

## 7.1 面板风格

所有核心面板都建议遵守同一套结构：

```text
panel.background  = bg.panel 或 bg.panel_alt
panel.border      = border.base
panel.radius      = radius.lg
panel.shadow      = 0 12 40 rgba(0, 0, 0, 0.28)
panel.inner_line  = 1px 顶部或左上微弱高光
```

视觉效果：

- 像金属感控制台面板
- 不是玻璃窗
- 不做高透明度背景

## 7.2 阴影层级

```text
shadow.none       = none
shadow.panel      = 0 10 28 rgba(0, 0, 0, 0.22)
shadow.float      = 0 18 48 rgba(0, 0, 0, 0.34)
shadow.glow_cyan  = 0 0 0 1 rgba(77, 215, 255, 0.16), 0 0 24 rgba(77, 215, 255, 0.12)
```

规则：

- 普通面板只用 `shadow.panel`
- 焦点态和运行态可以叠加轻微 `shadow.glow_cyan`
- 不要把 glow 用到所有元素

## 8. 背景设计

主背景不要纯色，建议三层：

1. 顶层超淡网格纹理
2. 左上到右下的深色渐变
3. 局部冷青径向辉光

建议描述：

```text
layer 1: linear-gradient(#09111D -> #0D1726)
layer 2: subtle grid / noise at 2%~4% opacity
layer 3: radial cyan glow behind active panels
```

注意：

- 网格必须足够淡
- 不允许像壁纸一样喧宾夺主
- 快捷面板背景可以更纯净，弱化背景纹理

## 9. 组件级视觉规范

## 9.1 TitleBar

目标：

- 看起来像桌面应用，不像网页导航栏
- 信息密度高，但不能拥挤

规范：

- 高度比普通标题栏略高
- 背景使用 `bg.canvas` 带 85%~92% 不透明度
- 下边缘一条极淡分隔线
- Logo 区域左对齐
- 右侧 badge 保持同一高度
- 命令输入框宽度固定，不随窗口过度拉伸

## 9.2 Sidebar

目标：

- 像导航轨，不像传统文件管理器边栏

规范：

- 宽度控制在 `176 ~ 208`
- 激活项使用低饱和高亮底色 + 左侧细亮条
- hover 用浅层底色，不要整块发光
- 图标统一线性、偏技术感

## 9.3 Tabs

规范：

- 不做浏览器页签样式
- 采用胶囊式分段控制更合适
- 激活项用 `bg.elevated + border.focus`
- 非激活项文本保持 `fg.secondary`

适用：

- `Peers`
- `Transfers`
- `Quick Panel`

## 9.4 Card / GroupBox

规范：

- 卡片边界清楚
- 标题区与内容区有明确层次
- 允许在重要卡片顶部加入 1px 冷青高光

推荐用于：

- 首页指标卡
- 活动流卡片
- 系统核心面板
- 文件待确认卡片

## 9.5 Buttons

按钮分三类：

- `Primary`
- `Secondary`
- `Danger`

### Primary

```text
background        = accent.cyan
foreground        = fg.inverse
hover             = accent.cyan_soft
focus             = border.focus + glow.cyan
```

用途：

- `Send`
- `Accept`
- `Open Workspace`

### Secondary

```text
background        = bg.elevated
foreground        = fg.primary
border            = border.base
hover             = bg.panel_alt
```

用途：

- `Load More`
- `Open Quick Panel`
- `View History`

### Danger

```text
background        = rgba(255, 107, 107, 0.14)
foreground        = accent.red
border            = border.danger
```

用途：

- `Reject`
- `Remove`

## 9.6 Inputs / Editor

规范：

- 输入框底色比普通面板更深一点
- 边框默认弱对比
- focus 时用 `border.focus`
- 占位符文字使用 `fg.dim`

`Editor` 特别要求：

- 文本区不要纯黑
- 内边距要充足
- 可以在左侧加极淡的行距引导，不做完整代码编辑器风格

## 9.7 Table / List / VirtualList

规范：

- 行高偏紧凑，但不能拥挤
- 选中行用 `bg.elevated`
- hover 行只做底色提亮
- 不要表格线过重

适用：

- `History`
- `Recent`
- `ActivityRail`

## 9.8 Progress

规范：

- 细长型进度条，厚度不要太大
- 底轨保持低对比
- 进度前景可带轻微流动感
- 完成态切换为 `accent.green`
- 失败态切换为 `accent.red`

## 9.9 Notification

规范：

- 浮在窗口右上或右下
- 按类型用左侧色条区分
- 文案短，动作明确
- 生命周期短于系统 toast，长于普通 tooltip

常见样式：

- 信息：冷青
- 成功：青绿
- 告警：琥珀
- 错误：红色

## 9.10 Sheet / Drawer

规范：

- 适合放文件接受/拒绝、详细错误、次级设置
- 背景比主面板略亮一点
- 遮罩透明度低，不要强打断

## 10. 页面级视觉重点

## 10.1 Home

这是未来感最强的一页。

可以使用：

- 轻微发光的状态环
- 低速脉冲的同步状态指示
- 时间线列表中的细连接线
- 活动面板中的动态进度条

不能使用：

- 大量动态图表
- 会持续跳动的数字
- 强烈闪烁的状态灯

## 10.2 Clipboard

重点是输入效率和目标清晰。

视觉重点：

- 大文本框
- 右侧目标选择卡片
- 底部状态反馈条

不要把这页做成聊天界面，也不要像邮件编辑器。

## 10.3 History

重点是“档案感”和“可回溯”。

视觉重点：

- 高密度列表
- 清楚的详情面板
- 时间和设备信息的技术风格展示

不建议加入过多插画或装饰性图形。

## 10.4 Peers

重点是“节点在线状态”。

视觉重点：

- 节点卡片
- 在线状态点
- 地址、方向、连接时间这些技术字段

这页可以最理性、最工程化。

## 10.5 Transfers

重点是“进度”和“决策”。

视觉重点：

- 文件卡片
- 进度条
- ETA / 速率 / 大小
- 接受与拒绝按钮

## 10.6 Settings

这是最克制的一页。

视觉要求：

- 几乎不使用 glow
- 强调对齐和表单秩序
- 所有重点交给信息结构，而不是特效

## 10.7 Quick Panel

这页要像“飞行面板”。

视觉要求：

- 更紧凑
- 更聚焦
- 背景更干净
- 不要像缩小版主工作台

可以比主工作台更亮一点，但层级更少。

## 11. 动效规范

## 11.1 动效原则

动效只做三件事：

1. 建立层级
2. 强化状态变化
3. 帮助用户感知后台仍在运行

如果一个动效不能服务这三件事，就不该加。

## 11.2 推荐动效

### 页面进入

- 主内容切换使用 `140ms ~ 180ms` 的淡入 + 轻微上浮
- 快捷面板呼出使用 `160ms` 的 scale + fade

### 状态变化

- `Running` 指示点做极低频脉冲
- 新活动项进入活动流时做轻微高亮闪现后回落
- 进度条可有缓慢流动光带

### 交互反馈

- hover 不超过 `100ms`
- 按钮按下反馈短且明确
- 列表选中切换使用平滑背景过渡

## 11.3 禁止动效

- 高频呼吸灯
- 大范围模糊动画
- 持续旋转图形
- 页面切换滑动过长
- 闪烁告警

## 12. 图标规范

建议统一用线性图标，避免过度圆润或拟物。

图标风格：

- 细线条
- 轻科技感
- 几何结构明确

页面建议：

- Home：网格/中控图标
- Clipboard：剪贴板或发送箭头
- History：时钟/归档
- Peers：节点连接
- Transfers：文件与箭头
- Settings：滑杆，不建议老式齿轮过多出现

## 13. Theme Token 建议

后续落地到 `theme.rs` 时，建议先定义以下 token 组：

```text
ThemeTokens
├── colors
│   ├── bg
│   ├── fg
│   ├── accent
│   ├── border
│   └── glow
├── typography
│   ├── sizes
│   ├── weights
│   └── mono
├── spacing
├── radius
├── shadow
└── motion
```

推荐拆法：

- `colors.rs`
- `typography.rs`
- `motion.rs`
- `component_tokens.rs`

## 14. 共享组件的视觉抽象

建议优先抽出以下共享视觉组件：

1. `StatusBadge`
2. `MetricCard`
3. `PanelCard`
4. `SectionTitle`
5. `ActivityItemRow`
6. `PeerChip`
7. `TransferProgressRow`
8. `DangerActionButton`

原因：

- 这样未来感和一致性来自组件系统，而不是每页单独拼颜色

## 15. 首版范围建议

如果下一步开始做视觉落地，先只做这几类组件：

1. `WorkspaceRoot`
2. `TitleBar`
3. `Sidebar`
4. `MetricCard`
5. `PanelCard`
6. `StatusBadge`
7. `TransferProgressRow`
8. `QuickPanelRoot`

先把主气质立起来，再补细节页。

## 16. 总结

这套视觉方案的核心不是“炫”，而是：

- 用深色和冷色强调建立稳定的控制台气质
- 用少量发光与轻动画表现“后台常驻、系统在线”
- 用统一的卡片、状态 badge、技术字体建立未来感
- 在 `Home` 和 `Quick Panel` 上集中体现个性
- 在 `History`、`Peers`、`Settings` 上保持克制和专业

如果继续往下走，下一步最合适的是二选一：

1. 再写一份 `stage5-*.md`，把首页和快捷面板做成更细的高保真视觉说明
2. 直接开始实现 `theme.rs` 和共享组件样式骨架
