# 桌面架构

本文面向维护者。应用使用见 [根 README](../../README.md)，环境与构建见 [开发指南](../../docs/development.md)，检查命令见 [测试文档](../../docs/testing.md)。

桌面宿主适配 `nooboard_core::App`，业务状态和工作流程由 core 决定。前端展示快照并发送类型化请求，不实现配对、自动发送、传输重试或历史保留策略。

## 文件结构

```text
src/
  app/                       启动、装配、导航与应用外壳
  desktop/
    api.ts                   公共请求句柄、生命周期接口与视图类型
    runtime.ts               连接所有权、请求分发、订阅与释放
    react.tsx                provider、selector 与命令反馈
    bridge/                  传输接口、Tauri 适配与生成的协议类型
    snapshot/                解码、版本排序与结构共享
    preferences/             外观持久化与宿主语言
    activity/                从业务状态投影界面活动
  features/
    devices/                 设备与配对表单
    history/                 查询、筛选、分页
    home/                    首页与剪贴板内容
    transfers/               普通内容传输
    settings/                设置表单
    activity/                活动摘要
    mailbox/                 信箱会话与分组
    mascot/                  动画呈现
  preview/                   同一 bridge 接口下的脚本化场景
  i18n/                      双语文案、错误与语言工具
  ui/                        通用显示组件

src-tauri/src/
  main.rs                    宿主装配及三个 IPC 入口
  host/                      core 服务所有者与可取消 WebView 订阅
  ipc/
    dto.rs                   请求、回复与帧契约
    snapshot.rs              core 到 IPC 的显式转换
    commands/                分发、原生对话框与配置补丁
    errors.rs                稳定错误代码
  desktop/                   托盘、窗口、导航与偏好
```

## 边界与生命周期

- 页面及应用装配只导入 `desktop/api.ts`。只有 bootstrap 选择实际适配器；features 不导入 Tauri、具体 runtime 或 preview。
- `createDesktop` 返回 runtime 所有者和稳定 desktop 句柄。bootstrap 负责启动、释放；React StrictMode 不负责停止服务。
- `Host` 拥有 `AppService`，与 WebView 订阅独立。关闭订阅只终止对应事件泵，完整应用退出才停止底层服务。
- 重连使旧回调与订阅 token 失效；迟到连接会被关闭。请求不能把旧连接结果应用到新会话。
- 快照使用会话与字符串版本号排序，保留后端活动 ID；未变化分支复用引用，恢复不重播旧动画。
- 同步配置属于业务；宿主偏好管理语言与关闭到托盘；外观管理主题和减少动态效果。保存版本、生效版本和重启提示只读。
- 历史记录由功能查询管理，不建立第二份全局历史缓存。筛选、后端历史版本或连接变化使旧查询失效，同一查询只允许一个分页请求进行。

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

Core 已装配 Terminal Service，但新终端页面与 IPC 尚未接入。桌面退出持有后端所有者直到 `shutdown(&mut self)` 成功；失败保留订阅和状态、恢复退出入口并提示用户重试。关闭到托盘只隐藏窗口。
