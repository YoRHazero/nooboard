# 桌面架构

本文面向维护者。应用使用见 [根 README](../../README.md)，环境与构建见 [开发指南](../../docs/development.md)，检查命令见 [测试文档](../../docs/testing.md)。

桌面宿主适配 `nooboard_core::App`，业务状态和工作流程由 core 决定。前端展示快照并发送类型化请求，不实现配对、自动发送、传输重试或历史保留策略。

## 文件结构

```text
src/
  app/                       启动、装配与应用外壳
    routes/                  TanStack Router 文件路由（页面挂载与搜索参数）
    router.ts                hash history 与路由实例
    routeTree.gen.ts          插件生成的路由树
    ui/AppShell.tsx           导航、全局反馈、认证与设备交互
  pages/
    home/                    首页内容、面板交互
    history/                 历史查询、筛选与分页
    devices/                 设备页、详情与编辑对话框
    transfers/               内容传输页面
    settings/                设置页面
    terminal/                配置编辑、运行工作区、xterm、远端文件面板
  features/
    devices/                 跨页面设备展示和配对交互
    terminal/                全局 SSH 认证提示
    transfers/               传输展示、目标选择
    activity/                活动摘要
    mailbox/                 信箱会话与分组
    mascot/                  动画呈现
  client/
    api.ts                   公共句柄、视图类型与纯函数
    react.tsx                provider、selector 与命令反馈
    create.ts                bootstrap 使用的构造入口
    runtime.ts               连接所有权、请求分发、订阅与释放
    terminal.ts              输出订阅、有限回放缓存、选中项和待处理请求
    bridge/                  传输接口、Tauri 适配与生成的协议类型
    snapshot/                解码、版本排序与结构共享
    preferences/             外观持久化与宿主语言
    activity/                从业务状态投影界面活动
  shared/
    i18n/                    双语文案、错误与语言工具
    ui/                      通用显示组件
  preview/                   同一 bridge 接口下的脚本化场景

src-tauri/src/
  main.rs                    进入桌面装配入口
  app/
    bootstrap.rs             构建服务、注入端口、注册 Tauri 入口
    runtime.rs               持有服务与投影/托盘订阅任务
    events.rs                原生事件入口，转换为窗口或退出意图
    snapshot.rs              桌面状态聚合类型
    adapters/core.rs         Core 所有者、关闭端口、配置修改串行化
    adapters/terminal_files.rs 原生选择结果 → Core 文件任务
  features/
    lifecycle/               退出状态、确认、清理、失败后恢复
    window/                  可见性、窗口显示/隐藏、导航确认
    preferences/             语言和关闭到托盘偏好
    tray/                    状态摘要、菜单展示与意图
    file_selection/          原生选择后发送文件或修改接收目录
  ipc/
    entry.rs                 connect / disconnect / request
    router.rs                请求分发与回复
    contracts/               按设备、剪贴板、传输、历史等拆分协议值类型
    mapping/                 Core/桌面状态转换与纯设置补丁转换
    routes/                  设置和历史的 IPC 参数适配
    subscription.rs          WebView 订阅所有权、取消、恢复
    terminal_requests.rs     Terminal 请求与输出通道的宿主适配
    terminal_io.rs           WebView 输出租约、取消、单帧确认与背压
    errors.rs                内部错误 → 稳定 IPC 错误代码
  localization.rs            原生菜单/对话框使用的双语文案
  diagnostics.rs             开发环境验证入口
```

## 边界与生命周期

- 页面以路由为边界组织，页面内部按实际职责拆文件，不套用后端 domain/application 分层。跨页面能力放 features，共用组件与国际化放 shared。
- 页面与功能通过 `client/api.ts` 和 `client/react.tsx` 使用稳定句柄与 hooks；bootstrap 从 `client/create.ts` 装配适配器。跨 feature 导入使用其 `api.ts`，features 不导入页面、Tauri、具体 runtime 或 preview。
- TanStack Router 使用文件路由和 hash history，兼容 Tauri 的内嵌资源协议。路由仅负责挂载与参数，后台 runtime 位于 RouterProvider 外。`vite build` 先生成路由树，再执行 TypeScript 检查。
- `createDesktop` 返回 runtime 所有者和稳定 desktop 句柄。bootstrap 负责启动、释放；React StrictMode 不负责停止服务。
- `app::Runtime` 持有各功能及后台任务，Core 所有者封装在 `app/adapters/core.rs`。IPC 的 `Subscriptions` 只拥有 WebView 事件泵，取消订阅不会关闭 Core。
- 重连使旧回调与订阅 token 失效；迟到连接会被关闭。请求不能把旧连接结果应用到新会话。
- 快照使用会话与字符串版本号排序，保留后端活动 ID；未变化分支复用引用，恢复不重播旧动画。
- 同步配置属于业务；宿主偏好管理语言与关闭到托盘；外观管理主题和减少动态效果。保存版本、生效版本和重启提示只读。
- 历史记录由功能查询管理，不建立第二份全局历史缓存。筛选、后端历史版本或连接变化使旧查询失效，同一查询只允许一个分页请求进行。

## Rust 宿主分层

各桌面 feature 使用 `domain.rs / application.rs / ports.rs / adapters.rs`：纯规则与值类型、用例及状态、所需能力、具体实现。文件选择复用标准路径值类型，只有 application/ports/adapters。平台操作位于适配器，内层不依赖 Tauri、IPC 或 Core 服务。跨层错误在 IPC 出口统一转换。

`app` 是装配根。Tauri 的全局状态只保存一个组合后的 Runtime；原生事件在 `app/events.rs` 获取它，IPC 在 `entry.rs` 注入它。功能模块通过构造器获得依赖，托盘原生回调通过 Action 通道提交意图。

| 状态                     | 唯一管理位置                           |
| ------------------------ | -------------------------------------- |
| Core 实例及配置修改锁    | app 的 Core 适配器                     |
| 退出阶段和确认代次       | lifecycle；Core 启动读取同一份退出状态 |
| 窗口可见性和待确认导航   | window                                 |
| 已保存偏好和解析后的语言 | preferences                            |
| 托盘摘要与当前显示语言   | tray                                   |
| WebView 订阅代次和任务   | ipc/subscription                       |
| 聚合快照版本             | app 的投影任务及同步发布入口           |

退出分为确认、关闭中和完成。原生退出可以取代尚未完成的用户确认；确认结果携带代次，失败恢复后的旧结果不能再次触发退出或取消新确认。Core 关闭失败时保留所有者，退出阶段恢复为可重试状态。

偏好先持久化，再发布状态；系统语言经端口注入，查询不写入配置文件。导航确认只清除对应 ID，窗口操作失败不会伪造可见性。聚合快照读取最新功能状态，延迟的订阅回调不会覆盖较新的同步结果。

IPC 保留 `connect / disconnect / request` 三个入口。Terminal 在现有带判别字段的协议中增加配置、运行、认证、文件操作与输出通道；Rust DTO 生成 schema 和 TypeScript 类型。

## 请求与订阅

1. 功能调用公共句柄，如 `desktop.updateSyncSettings({ paused: true })`。
2. runtime 经 bridge 发送带判别字段的请求。Tauri 仅暴露 `connect`、`disconnect`、`request`。
3. 宿主反序列化和校验，按需串行合并补丁，再调用 core 公共 API。
4. 类型化 Reply 表示命令结果，权威快照更新页面；界面不自行推断业务成功。
5. 解码器建立视图，store 拒绝旧版本，活动呈现层消费状态变化。

业务快照与宿主状态分别传递。大整数 ID、版本和偏移用字符串传递；界面时间戳与有界字节计数用数字。

## 预览与动画

普通浏览器走相同 runtime、解码和订阅接口，使用内存 fixture。预览不是另一套 core，也不能作为业务正确性的依据。

首页默认加载完整 `bird-business-19.glb` 场景。活动关联实际传输 ID 与结果，动画完成不表示交付成功。`Playback` 协调活动，`ClipPlayer` 管理带道具的过渡，`BirdScene` 处理渲染、隐藏和减少动态效果。

`?mascot=business-19` 打开动画检查，兼容旧 `?mascot=motion-16` 入口；手动播放与姿态滑块不发送业务事件。图标、Blender 导出、动作约束与测试范围见 [资源文档](public/mascot/README.md)。

## 修改契约

先修改 Rust DTO，再从 `apps/desktop` 运行 `npm run ipc:generate`，提交 schema 与 TypeScript 生成物。Rust schema 测试和 `npm run ipc:check` 检查漂移，`npm run architecture:check` 约束模块导入。

协议变化同时更新原生分发及预览场景，利用 TypeScript 穷尽检查暴露遗漏。打包默认启用 `custom-protocol` 以加载内嵌资源；Tauri dev 关闭该默认 feature 并使用本机 Vite。生成协议也关闭默认 feature，因此无需已有前端构建。

Terminal 页面通过 Core 管理终端会话、独立命令、SSH 目标和隧道。桌面退出持有后端所有者直到 `shutdown(&mut self)` 成功；失败保留订阅和状态、恢复退出入口并提示用户重试。关闭到托盘只隐藏窗口。

## Terminal 工作区

终端会话、独立命令和隧道分别管理，终端会话与命令共用 xterm 组件。切换路由、切换输出视图或关闭 WebView 订阅不停止已经登记的资源；关闭终端标签调用停止会话，命令与隧道有独立停止操作。完整退出沿 Core 的关闭流程等待清理，托盘隐藏继续运行。

快捷入口“打开本机终端”通过 `startLocal` 请求调用 Core 的直接启动接口，仅创建会话，不写配置仓库。运行视图的 `configId` 可为空；运行 ID 始终存在。配置面板的启动操作保留按配置启动的路径。

前端只保存配置引用和当前视图状态；Shared 实例选择、依赖放行、生命周期、停止串行化和配置冻结均由后端决定。密码在认证时输入，选择保存后仍由 Core 在成功认证后加密持久化；导入私钥不保留原路径。快照不会包含明文密码、私钥或密文。主机密钥变更必须显式核对并替换，之后由用户重新启动。

输出注册与启动在同一 Core 请求中完成。每个 WebView 输出订阅最多有一帧等待确认，xterm 解析完成后确认；界面未挂载时只保留有限字节缓存。慢消费者导致后端回放缺口时显示提示。重连会使旧租约失效，迟到完成不能清理新租约；结束事件用于即时反馈，权威快照负责清除遗漏结束事件的缓存，不保存已结束运行历史。

SFTP 只附着于 SSH 终端会话，文件目录不跟随 shell 工作目录。支持浏览、文件和文件夹上传下载、新建目录、重命名、删除、冲突策略与任务取消；符号链接交给后端拒绝。选择本地文件或目录由桌面 FileSelection 端口处理，前端不持有本地路径授权。运行超时与回放大小来自可保存的参数配置。
