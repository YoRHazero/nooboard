# Desktop architecture

The desktop host adapts `nooboard_core::App`; core remains the authority for business state and workflows. The frontend displays snapshots and sends typed requests. It does not implement pairing, automatic sending, transport retries or history retention.

```text
src/
  app/                       composition, startup, providers, navigation and shell
  desktop/
    api.ts                   public request handle, lifecycle interface and view types
    runtime.ts               connection ownership, request dispatch, disposal
    react.tsx                provider, selectors, command feedback
    bridge/                  transport port, Tauri adapter and generated wire types
    snapshot/                pure decoding, revision ordering and structural sharing
    preferences/             appearance persistence and host language integration
    activity/                presentation cues derived from authoritative snapshots
  features/
    devices/                 device views and pairing forms
    history/                 history query lifetime, filters and pagination
    home/                    home composition and clipboard display
    transfers/               transfer views and labels
    settings/                forms for each settings category
    activity/                display summaries
    mailbox/                 session presentation and grouping
    mascot/                  animations, never business outcomes
  preview/
    bridge.ts                scripted responses implementing the same bridge port
    scenarios/               wire snapshots and example data
  i18n/                      translations and pure error/language helpers
  ui/                        reusable presentation controls

src-tauri/src/
  main.rs                    host composition and three IPC entrypoints
  host/                      core service owner and cancellable WebView subscription
  ipc/
    dto.rs                   request/reply/frame contract
    snapshot.rs              explicit core-to-IPC conversion
    commands/                request dispatch, host dialogs and patch translation
    errors.rs                stable frontend error codes
  desktop/                   native tray, visibility, navigation and preferences
```

## Boundaries and ownership

- Pages and application composition import `desktop/api.ts`. Only bootstrap selects an adapter. Features never import Tauri, a concrete runtime, or preview code.
- `createDesktop` returns `{ runtime, desktop }`. Bootstrap owns `runtime.start()` and `runtime.dispose()`. React receives only `desktop`, a stable request and subscription handle. React StrictMode does not own or stop the runtime.
- `Host` owns `AppService` independently of the WebView. Closing a frontend subscription aborts its event pump, while tray operation and core continue. Only application shutdown stops the service.
- Reconnect invalidates old callbacks and closes old subscription tokens. A late connection closes itself. Requests reject results from an obsolete connection or core session.
- Snapshots are ordered by session and full-width revision strings. Activity IDs retain backend identity. Unchanged snapshot branches retain reference identity for selectors. Recovery advances presentation watermarks without replaying historical animations.
- Sync settings are writable business configuration. Host preferences cover language and closing to tray. Appearance contains theme and reduced motion. Saved/effective configuration revisions and restart status are read-only.
- History rows belong to a feature query, not a second global history cache. Filter changes, backend revision changes and reconnection invalidate pending queries; pagination admits one request at a time.

## Request and update flow

1. A feature calls the public handle, such as `desktop.updateSyncSettings({ paused: true })`.
2. The runtime sends a discriminated `Request` through its bridge. Tauri exposes `connect`, `disconnect` and `request` only.
3. The host validates/deserializes the request, maps patches under a mutation lock, and invokes core's public API.
4. The typed `Reply` resolves the caller's operation. The authoritative snapshot stream updates rendered state; the page does not fabricate successful business outcomes.
5. The decoder creates a view model, the store rejects obsolete revisions, and presentation cues notify animation/mailbox consumers.

Preview follows the same connection, decoding and subscription path. Its fixtures cover visual states with scripted transitions, not a second implementation of core. Preview code must not serve as a business correctness oracle.

The `?mascot=business-19` presentation preview (also accepting the earlier `?mascot=motion-16` URL) uses the real home stage with a complete Blender scene and twelve manually selectable clips. Its pose slider inspects baked frames. Bootstrap selects this mode; features receive it as a prop. Preview playback does not emit business events or decide delivery outcomes. The same complete scene is now the default asset. Activity cues carry content task/node/stage metadata so Playback waits for a real file receipt, correlates send results, and coalesces concurrent performances while the mailbox preserves every activity. ClipPlayer owns prop-safe clip transitions; hidden/reduced-motion behavior stays in Mascot/BirdScene. UI gestures never decide delivery outcomes. Asset provenance, export settings and packaging are documented in [the mascot asset guide](public/mascot/README.md).

## Extending and verifying

Edit Rust DTOs first, then run `npm run ipc:generate` in `apps/desktop`. It runs the Rust schema test and generates `ipc/schema.json` and `src/desktop/bridge/generated.ts`. IDs and backend counters cross IPC as strings; JavaScript cannot represent arbitrary `u64`/`i64` values as numbers. UI byte counts and timestamps remain numbers.

The Rust schema test detects DTO/schema drift. `npm run ipc:check` detects schema/TypeScript drift. `npm run architecture:check` enforces import boundaries. CI also runs frontend tests, production build, Rust tests and clippy. Changes to the contract require updating both native dispatch and preview scenarios; TypeScript's exhaustive union checks expose missing cases.

Packaged builds enable `custom-protocol` by default so they load embedded assets. Tauri dev disables that default feature and uses the local development server. Protocol generation uses `--no-default-features`, so generating types does not require an existing frontend build.

## SSH 工作区

`features/ssh` 仅依赖 `desktop/api`，xterm 适配限定于终端组件。登录请求的密码仅发给原生 core，不进入快照或日志；是否记忆、认证与指纹决策都由 core 处理。浏览器预览使用 `preview/remote.ts` 内存场景，不调用原生 SSH 或凭据库。

`RemoteRequest` / `RemoteReply` 嵌入现有 `request` 命令；原生文件选择、下载目录选择、拖入坐标转换和已完成文件的显示属于宿主。`Remote` 状态和 `Terminal` 字节帧走独立帧类型，目录列表未变化时省略，由桌面运行时保留对应文件连接的既有缓存。重连时新文件连接不会继承旧缓存。终端订阅落后或窗口恢复时通过 `TerminalReset` 触发有偏移的回放。

关闭 WebView、切换页面和关闭终端不拥有 SSH 服务生命周期。Rust `AppService` 监督 SSH 服务，文件任务留在进程中；整个应用关闭时执行受控清理。原生拖出下载及文件夹递归传输不在第一版范围。
