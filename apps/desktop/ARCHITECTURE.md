# 桌面架构

本文面向维护者。应用使用见 [根 README](../../README.md)，环境与构建见 [开发指南](../../docs/development.md)，检查命令见 [测试文档](../../docs/testing.md)。

桌面宿主适配 `nooboard_core::App`，业务状态和工作流程由 core 决定。前端展示快照并发送类型化请求，不实现配对、自动发送、传输重试或历史保留策略。

## 文件结构

```text
src/
  app/                       启动、路由、全局界面和资源装配
    bootstrap.tsx            创建 QueryClient、选择真实/预览通信、注册清理
    backend-subscription.ts  连接、重连、消息代次检查和终端订阅协调
    snapshot-projection.ts   协议转换、版本检查、活动提示和 Query 缓存更新
    query-client.ts          Query / mutation 默认配置
    routes/                  路由挂载与搜索参数
    ui/AppShell.tsx           导航、全局反馈、认证与设备交互
  pages/                     平铺的页面文件；只组装功能区域和布局
    TerminalPage.tsx          配置树、终端输出和远程文件区域
    HomePage.tsx              首页概览和交互场景
    DevicesPage.tsx           本机设备和设备列表
    HistoryPage.tsx / TransfersPage.tsx / SettingsPage.tsx
  features/                  按业务功能组织，不区分单页面/跨页面功能
    terminal/
      *.tsx / *.css          功能组件与样式直接放在根目录，没有 components 层
      queries/               每项 query/mutation 独立文件
      store/                 session、tree、files 前端交互 slices
      io/                    终端输出流、背压与有限回放缓冲
      index.ts               外部可用的区域组件
    history/                 历史组件、Infinite Query、历史修改 mutations
    devices/ / transfers/    设备和传输组件、各操作 mutations
    settings/                设置组件、偏好保存、外观及已应用语言状态
    activity/ / mailbox/ / mascot/
  shared/
    backend.ts               类型化 IPC 请求和原生通道适配
    backend-contract.ts      真实/预览通信共同满足的端口
    contracts.generated.ts   从 Rust DTO schema 生成的协议类型
    queries/                 共享快照 Query 和公共操作 mutations
    store/                   前端连接状态、全局错误反馈；不保存后端业务快照
    models.ts / types.ts     前端视图类型和公共类型出口
    decode-snapshot.ts       协议快照到前端视图的纯转换
    ui/ / i18n/              通用组件、双语文案和错误格式化
  preview/                   模拟通信和预览场景

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

- `pages` 只声明布局和组合 feature 的公共区域组件，不读取 Query、Zustand 或 IPC。功能组件直接位于 feature 根目录；内部小组件仍可通过 props 接收数据。跨 feature 的区域组件与展示工具通过 `index.ts` 导入；独立 query/mutation hook 可以直接从其文件导入，以避免 UI barrel 产生循环依赖。feature 不依赖 pages、app 或 preview。
- 后端业务状态统一由 TanStack Query 持有。`connect` 返回的初始快照、后续推送和恢复快照经过连接代次及完整宽度版本检查后，写入唯一的 `['backend', 'snapshot']` 缓存项。各组件通过 selector 读取同一份数据，不向 Zustand 复制设备、运行列表或传输状态。
- 共享快照 Query 使用 `skipToken`、无限 freshness / GC 保留，由应用级订阅更新；挂载页面不会创建额外的快照请求。应用释放时清理 QueryClient。后端仍是权威来源，前端缓存不是持久化数据库。
- 历史分页和远程目录是按需 Query。query key 包含后端 session、筛选条件或运行 ID/路径；历史版本变化使相关查询失效，Core session 切换和会话结束清除失效缓存。IPC 不支持的取消不会被伪装成后端操作已取消；迟到结果通过 Query signal 和连接代次检查丢弃。
- 每个 query/mutation 位于所属 feature 的 `queries/` 独立文件。类型化请求经过 `shared/backend.ts`，组件不直接导入 Tauri。IPC 使用 `networkMode: 'always'`，可用性由真实连接状态控制；mutation 不自动重试。按钮和表单反馈从 Query 的操作状态派生。
- Zustand 按 feature 内聚性拆成 `store/` slices，只管理前端交互和展示状态：终端选择/收起、文件面板开关、外观、已应用语言、信箱分组等。信箱保留已展示的会话摘要以支持未确认分组，不作为当前传输结果的查询来源。单组件表单保留本地状态。
- 终端输出字节、回放缓存和取消句柄由 `features/terminal/io` 管理，不逐帧写入 Query 或 Zustand。选择与收起动作在 session slice 中，I/O 只据此释放输出订阅；所有标签只最小化，真正的停止操作仅位于配置树实例行。
- TanStack Router 使用文件路由和 hash history。应用订阅在 RouterProvider 外创建和释放，React StrictMode 及页面切换不拥有 Core 服务生命周期。预览实现使用相同通信端口、Query 缓存与状态逻辑。
- Rust `app::Runtime` 持有宿主功能与后台任务，Core 所有者封装在 `app/adapters/core.rs`。IPC 的订阅只管理 WebView 消息泵，取消订阅不会关闭 Core。

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

1. 功能组件调用所属 query/mutation hook，例如同步设置 mutation。
2. 操作通过 `shared/backend.ts` 发送带判别字段的请求。Tauri 仅暴露 `connect`、`disconnect`、`request`。
3. 宿主反序列化和校验，按需串行合并补丁，再调用 core 公共 API。
4. 类型化 Reply 表示命令结果，权威快照更新页面；界面不自行推断业务成功。
5. 解码器建立视图，store 拒绝旧版本，活动呈现层消费状态变化。

业务快照与宿主状态分别传递。大整数 ID、版本和偏移用字符串传递；界面时间戳与有界字节计数用数字。

## 预览与动画

普通浏览器走相同解码、Query 缓存和订阅接口，使用内存 fixture。预览不是另一套 core，也不能作为业务正确性的依据。

首页默认加载完整 `bird-business-19.glb` 场景。活动关联实际传输 ID 与结果，动画完成不表示交付成功。`Playback` 协调活动，`ClipPlayer` 管理带道具的过渡，`BirdScene` 处理渲染、隐藏和减少动态效果。

`?mascot=business-19` 打开动画检查，兼容旧 `?mascot=motion-16` 入口；手动播放与姿态滑块不发送业务事件。图标、Blender 导出、动作约束与测试范围见 [资源文档](public/mascot/README.md)。

## 修改契约

先修改 Rust DTO，再从 `apps/desktop` 运行 `npm run ipc:generate`，提交 schema 与 TypeScript 生成物。Rust schema 测试和 `npm run ipc:check` 检查漂移，`npm run architecture:check` 约束模块导入。

协议变化同时更新原生分发及预览场景，利用 TypeScript 穷尽检查暴露遗漏。打包默认启用 `custom-protocol` 以加载内嵌资源；Tauri dev 关闭该默认 feature 并使用本机 Vite。生成协议也关闭默认 feature，因此无需已有前端构建。

Terminal 页面通过 Core 管理终端会话、独立命令、SSH 目标和隧道。桌面退出持有后端所有者直到 `shutdown(&mut self)` 成功；失败保留订阅和状态、恢复退出入口并提示用户重试。关闭到托盘只隐藏窗口。

## Terminal 工作区

终端会话、独立命令和隧道分别管理，终端会话与命令共用 Ghostty 组件。切换路由、切换输出视图或关闭 WebView 订阅不停止已经登记的资源；所有类型的标签只提供最小化，配置树的实例行提供停止操作。完整退出沿 Core 的关闭流程等待清理，托盘隐藏继续运行。

快捷入口“打开本机终端”通过 `startLocal` 请求调用 Core 的直接启动接口，仅创建会话，不写配置仓库。运行视图的 `configId` 可为空；运行 ID 始终存在。配置面板的启动操作保留按配置启动的路径。

前端只保存配置引用和当前视图状态；Shared 实例选择、依赖放行、生命周期、停止串行化和配置冻结均由后端决定。密码在认证时输入，选择保存后仍由 Core 在成功认证后加密持久化；导入私钥不保留原路径。快照不会包含明文密码、私钥或密文。主机密钥变更必须显式核对并替换，之后由用户重新启动。

输出注册与启动在同一 Core 请求中完成。每个 WebView 输出订阅最多有一帧等待确认，Ghostty 解析完成后确认；界面未挂载时只保留有限字节缓存。慢消费者导致后端回放缺口时显示提示。重连会使旧租约失效，迟到完成不能清理新租约；结束事件用于即时反馈，权威快照负责清除遗漏结束事件的缓存，不保存已结束运行历史。

SFTP 只附着于 SSH 终端会话，文件目录不跟随 shell 工作目录。支持浏览、文件和文件夹上传下载、新建目录、重命名、删除、冲突策略与任务取消；符号链接交给后端拒绝。选择本地文件或目录由桌面 FileSelection 端口处理，前端不持有本地路径授权。运行超时与回放大小来自可保存的参数配置。

### 终端页面交互约定

命令编辑器支持按保存顺序排列的自动输入规则。已有键只读，值仅显示已设置，可保留、覆盖或删除；新规则追加到末尾，不提供排序。规则列表在弹窗内独立滚动，命令信息、添加按钮及保存操作固定可见；说明通过标题旁的 Toggle tip 展示，新增规则自动聚焦并滚入列表可视区域。请求用 `value: null` 保留值，字符串覆盖值（空串表示只回车），省略规则表示删除。快照仅返回提示键，预览也不回传输入值。Core 加密保存并在启动时解密，Terminal 匹配输出行首并消费规则；`autoInputError` 触发错误通知，保留实例和手动输入。

页面上方是可折叠的「目标 → 配置 → 实例」树，下方是主要终端区及可收起的右侧文件区。内置本机目标和默认 shell 不需要保存配置。新建 SSH 目标在树头部；新建会话、命令和隧道在所属目标的菜单中，四类编辑表单独立，表单不切换类型或所属目标。

`configuration.ts` 根据后端配置引用、运行实例和启动中的计划计算可修改性。被实例占用或被其他配置引用时，修改和删除禁用；mutation 发送前重新检查。克隆始终可用，只复制当前配置，不复制子配置和实例；副本创建后立即编辑，取消编辑不删除副本。前端限制用于此界面的一致操作；Core 仍以其引用登记器保护删除，更新接口没有新增跨客户端锁定保证。

终止或退出的实例自动移除，当前实例结束时仅切换到未最小化的其他实例，没有则显示空状态。命令就绪不会自动最小化。实例在树中点击后恢复并选中，标签不负责终止。

文件区跟随当前 SSH 终端，其他实例禁用文件按钮，但保留文件区打开偏好。每个会话的目录、展开节点和选中路径保存在 `files` slice；目录内容只在 Query 中。目录点击可设置为进入或原地展开，路径补全只查询输入路径的父目录并匹配目录名前缀。上传使用已加载目录的 `path`，不使用未提交的输入或展开节点。选择模式开启后才显示勾选框；新建目录和重命名放在右键菜单，删除需确认。下载每次使用原生目录选择器，宿主将上次下载目录写入应用数据目录的 `download-directory.json`。

通知由应用级 Toaster 承载。普通通知在右下，成功自动消失，错误及需处理的通知保留；认证和密钥确认仍使用交互弹窗。终端文件操作在左下用同一个任务 ID 更新进度、取消和结果，切换路由或隐藏文件区不影响通知。结果由 `terminalEnded` 事件确认，不把任务从快照消失视为成功；丢失结果时提示无法确认。
