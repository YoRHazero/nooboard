# nooboard-gui Porting Plan

This document is the execution plan and cleanup checklist for the migration from the deleted
legacy desktop stack to `crates/nooboard-gui`.

Unlike [`NOOBOARD_GUI_SPEC.md`](./NOOBOARD_GUI_SPEC.md), this file is not the architectural
contract. It is the implementation and migration checklist.

If this plan conflicts with the GUI spec, the spec wins and this plan must be updated.

## 1. Objective

Build and finish `crates/nooboard-gui` so that it:

- preserves the visual language of the legacy desktop baseline retained in git history
- preserves the existing responsive layout behavior of that legacy desktop baseline
- integrates directly with `crates/nooboard-core`
- deletes the old frontend/service stack after migration

This is not an incremental compatibility migration.

It is a clean GUI rewrite that uses the legacy desktop UI from git history as a visual reference
implementation.

## 2. Migration Strategy

The migration strategy is:

1. create `crates/nooboard-gui`
2. port only the frontend shell, components, and visuals that remain valid under `nooboard-core`
3. rewrite all state, command, bootstrap, and subscription logic from scratch
4. validate visual and responsive parity against `nooboard-desktop`
5. delete the old crates

At no point should `nooboard-gui` depend on `nooboard-app` or `nooboard-sync`.

## 3. Source Classification

The deleted legacy desktop codebase is split into two categories:

### 3.1 Non-portable source

The following legacy desktop source patterns are non-portable and MUST be rewritten:

- the deleted `state/live_app.rs` surface from legacy desktop history
- the deleted `state/live_commands.rs` surface from legacy desktop history
- the deleted `state/mod.rs` surface from legacy desktop history
- any module that directly imports `nooboard-app`
- any module that directly refers to old `AppState`, `AppEvent`, `DesktopAppService`,
  `SyncActualStatus`, `PeerTransport`, `TransferId`, or `NoobId`

These files are reference material only for:

- user-visible copy
- UX intent
- recent activity semantics
- visual grouping of state

They MUST NOT be copied as implementation.

### 3.2 Portable visual reference

The following legacy desktop source categories are visually portable and MAY be used as reference
or as a visual copy base from git history:

- legacy `ui/bootstrap/*`
- legacy `ui/workspace/*`
- legacy `ui/theme.rs`
- legacy desktop build script behavior
- legacy packaging metadata

Portable means:

- layout and styling may be reused
- component composition may be reused
- icons and assets may be reused

Portable does NOT mean:

- keep old props/state shapes
- keep old service calls
- keep old desktop-level domain assumptions

## 4. New Crate Target Layout

The new GUI crate SHOULD be introduced with this shape:

```text
crates/nooboard-gui/
  build.rs
  Cargo.toml
  src/main.rs
  src/assets.rs
  src/app.rs

  src/bootstrap/
    mod.rs
    controller.rs
    state.rs
    view_state.rs

  src/workspace/
    mod.rs
    core_bridge.rs
    controller.rs
    runtime_state.rs
    recent_activity.rs
    shell_view_state.rs
    route.rs
    view_state/
      mod.rs
      home/
        mod.rs
        recent_activity.rs
        system_core/
          mod.rs
          header.rs
          controls.rs
          radar.rs
          clipboard.rs
          components.rs
      clipboard.rs
      network/
        mod.rs
        actions.rs
        state.rs
      transfers/
        mod.rs
        actions.rs
        state.rs
      settings/
        mod.rs
        actions.rs
        state.rs
      shared.rs
    subscriptions.rs
    actions/
      mod.rs
      clipboard.rs
      network.rs
      settings.rs
      transfers.rs
      spawn.rs

  src/ui/
    mod.rs
    theme.rs
    bootstrap/
      actions.rs
      components.rs
      view.rs
    workspace/
      mod.rs
      shell.rs
      home/
        mod.rs
        recent_activity.rs
        system_core/
          mod.rs
          header.rs
          controls.rs
          radar.rs
          clipboard.rs
          components.rs
      clipboard/
        mod.rs
        actions/
          mod.rs
          lifecycle.rs
          history.rs
          editing.rs
          broadcast.rs
        state/
          mod.rs
          history.rs
          editor.rs
          targets.rs
        components.rs
        view_state.rs
        header.rs
        history.rs
        detail.rs
      network/
        mod.rs
        page.rs
        components.rs
        composer.rs
        seeds.rs
        requests.rs
        sessions.rs
        lan_peers.rs
        actions.rs
        state.rs
      transfers/
        mod.rs
        page.rs
        components.rs
        targets.rs
        composer.rs
        activity.rs
        actions.rs
        state.rs
      settings/
        mod.rs
        page.rs
        components.rs
        derived.rs
        sections/
          mod.rs
          connection.rs
          clipboard.rs
          transfers.rs
          storage.rs
          advanced.rs
        state/
          mod.rs
          connection.rs
          clipboard.rs
          transfers.rs
          storage.rs
        actions.rs
      transfer_rail.rs
      shared.rs
```

Notes:

- `ui/bootstrap/` and `ui/workspace/` should preserve the same broad visual module layout as the
  deleted desktop baseline where practical
- `bootstrap/` and `workspace/` outside `ui/` are logic modules, not render modules
- route-specific render logic should live under `ui/workspace/*`, while top-level view types only
  assemble render models and shell layout
- once a logic-layer module like `workspace/view_state` or `workspace/actions` starts spanning
  multiple page/domain responsibilities, it should be expanded into submodules instead of growing
  as a single file
- if a route like clipboard or settings becomes materially larger than one cohesive file,
  it should be expanded into `ui/workspace/<route>/` submodules instead of pushing more unrelated
  concerns into a single route file
- the Home route is expected to keep the old desktop `system_core + recent_activity` composition;
  if the old dock contains removed concepts like desktop-only clipboard preferences, preserve the
  layout and adapt the interaction to an equivalent `nooboard-core` action instead of restoring
  the removed concept
- `network`, `transfers`, and `settings` are expected to follow the same rule once they gain route-
  local drafts, file pickers, or async interaction state; those concerns should live inside the
  route directory rather than in `WorkspaceView` or shared helpers
- shell-level transfer status chrome should stay out of route pages; if the legacy right rail is
  preserved for parity, it belongs in a dedicated `ui/workspace/transfer_rail.rs`
- settings may keep a read-only advanced/bootstrap section for parity, but it must remain a shell-
  or launch-derived view rather than a second persistent settings model

## 5. Phase Plan

### Phase 1: Crate bootstrap

Goal:

- create `crates/nooboard-gui`
- wire dependencies
- port build script and packaging metadata

Required work:

- create `Cargo.toml`
- add dependency on `nooboard-core`
- do not add dependency on `nooboard-app`
- do not add dependency on `nooboard-sync`
- port Windows icon embedding behavior from old desktop `build.rs`
- port required package metadata from old desktop `Cargo.toml`

Exit criteria:

- `cargo check -p nooboard-gui` passes with a minimal app shell

### Phase 2: Bootstrap UI and launch flow

Goal:

- replace old desktop bootstrap handling with direct `NooboardCore` bootstrap flow

Required work:

- implement bootstrap controller around:
  - `NooboardCore::resolve_bootstrap(...)`
  - `NooboardCore::prepare_default_config_from_chooser(...)`
  - `NooboardCore::launch(...)`
- port bootstrap visuals from old desktop
- preserve chooser messaging and config-path visibility where still applicable

Exit criteria:

- GUI can:
  - open chooser-required flow
  - reset incompatible default config through chooser
  - launch workspace from a valid `BootstrapLaunch`

### Phase 3: Workspace bridge

Goal:

- establish the new GUI-to-core integration boundary

Required work:

- implement `core_bridge.rs`
- hold `Arc<NooboardCore>`
- obtain:
  - initial `WorkspaceSnapshot`
  - `StateSubscription`
  - `EventSubscription`
- bridge subscriptions into GPUI-safe updates

Exit criteria:

- workspace can launch and keep a live `WorkspaceSnapshot` projection in memory

### Phase 4: Top-level shell and routing

Goal:

- recreate the current desktop shell around the new core bridge

Required work:

- port top-level shell layout from old desktop workspace UI
- port route/tab structure
- preserve responsive shell behavior
- do not wire deep feature actions yet

Exit criteria:

- user can reach major sections with stable shell navigation
- shell matches old desktop layout closely

### Phase 5: Clipboard/history surfaces

Goal:

- reconnect clipboard views to `WorkspaceSnapshot` and core commands

Required work:

- port clipboard history and detail visuals
- derive rendered clipboard state from snapshot and clipboard-history queries
- wire:
  - `list_clipboard_history`
  - `get_clipboard_record`
  - `submit_text`
  - `adopt_clipboard_record`
  - `rebroadcast_clipboard_record`

Exit criteria:

- clipboard history behaves correctly
- visual parity with old desktop is retained
- no old service/state model remains in clipboard UI

Current status:

- basic clipboard history paging and detail queries are complete
- scalable history behavior is still a follow-up item; the current page appends loaded rows and is
  not yet virtualized or bounded by page-window eviction
- the follow-up implementation MUST adopt the directional `direction + anchor + limit` paging
  contract defined in [`NOOBOARD_STORAGE_HISTORY_PAGING_PLAN.md`](./NOOBOARD_STORAGE_HISTORY_PAGING_PLAN.md)
  rather than extending the old single-`next_cursor` model

### Phase 6: Network and direct-connect surfaces

Goal:

- reconnect LAN/direct network views to `WorkspaceSnapshot.network`

Required work:

- port network/home/settings visuals that remain valid
- render network status, LAN peers, direct seeds, pending requests, and sessions from snapshot
- wire:
  - `start_network`
  - `stop_network`
  - `set_lan_enabled`
  - `upsert_direct_seed`
  - `remove_direct_seed`
  - `search_direct_seeds`
  - `connect_direct_seed`
  - `approve_direct_request`
  - `reject_direct_request`
  - `disconnect_session`

Exit criteria:

- old `PeerTransport`/`manual_peers` vocabulary is gone
- UI reflects the new core/network model directly
- `ui/workspace/network/` stays split by page/panel/components instead of returning to one route
  render hotspot

### Phase 7: Transfer surfaces

Goal:

- reconnect file transfer UI to `WorkspaceSnapshot.network.transfers`

Required work:

- port transfer list/offer/progress visuals
- wire:
  - `send_files`
  - `decide_incoming_transfer`
  - `cancel_transfer`
- render transfer state from snapshot, not event accumulation

Exit criteria:

- transfer UI behaves correctly under `nooboard-core`
- no local transfer truth store exists in the GUI
- `ui/workspace/transfers/` stays split by targets/composer/activity/components

### Phase 8: Settings surfaces

Goal:

- reconnect settings UI directly to core configuration commands

Required work:

- port settings visuals
- wire:
  - `set_device_id`
  - `set_network_token`
  - `set_network_listen_port`
  - `set_lan_enabled`
  - `set_local_capture_enabled`
  - `set_download_dir`
  - `set_storage_settings`
  - keep `db_root` as read-only display until `nooboard-core` exposes a dedicated setter

Exit criteria:

- settings edits reflect immediately through `WorkspaceSnapshot`
- GUI does not maintain a second persistent settings model
- `ui/workspace/settings/` stays split into section renderers plus shared chrome/derived status
- settings draft state stays as a thin aggregate over section-local substates

### Phase 9: Parity and cleanup

Goal:

- validate parity and delete old crates

Required work:

- compare visual parity with old desktop
- compare responsive behavior with old desktop
- remove any leftover direct or indirect dependencies on `nooboard-app`
- remove any leftover direct or indirect dependencies on `nooboard-sync`
- delete old crates after cutover

Exit criteria:

- `nooboard-gui` is the only frontend crate
- `nooboard-desktop`, `nooboard-app`, and `nooboard-sync` are deleted

## 6. What Must Be Rewritten From Scratch

The following concepts MUST be rewritten from scratch in `nooboard-gui`:

- bootstrap controller logic
- workspace launch logic
- core bridge
- subscriptions bridge
- action routing
- view-state derivation
- toast/notice handling from `WorkspaceEvent`
- settings command integration

The old desktop implementation MUST NOT be used as a code-copy baseline for these.

## 7. What May Be Visually Ported

The following SHOULD be treated as visual reference first:

- bootstrap page composition
- workspace shell layout
- clipboard page layout
- home/dashboard cards
- settings page layout
- shared UI chrome
- icon usage
- theming and spacing rhythm

When porting these, the preferred workflow is:

1. copy or restate the visual structure
2. remove old state/service assumptions
3. reconnect props and actions to the new `WorkspaceSnapshot`/`NooboardCore` model

## 8. Old Vocabulary Elimination Checklist

The new GUI is not complete if any of the following still exist in `crates/nooboard-gui`:

- `DesktopAppService`
- `DesktopAppServiceImpl`
- `AppState`
- `AppEvent`
- `SyncActualStatus`
- `SyncDesiredState`
- `ConnectedPeer`
- `PeersState`
- `PeerTransport`
- `TransferId`
- `manual_peers`
- `mdns_enabled`

If one of these appears during implementation, it is a regression toward compatibility mode.

## 9. Responsive Parity Checklist

Before migration is considered complete, verify:

- bootstrap screen still works at narrow and wide widths
- workspace shell still behaves correctly when resized
- sidebars and content panes still collapse/stack appropriately
- clipboard history/detail still remain usable on narrower windows
- settings forms still remain usable on narrower windows
- network and transfer surfaces still remain readable at narrower widths

Responsive parity is required, not optional polish.

## 10. Post-Cutover Follow-Up: Clipboard History Scaling

This follow-up begins only after the baseline GUI cutover is stable.

Goal:

- make clipboard history scale in both rendering cost and memory footprint without reintroducing a
  second durable state model

Required work:

- evolve `nooboard-core` history querying to the directional `direction + anchor + limit`
  contract defined in [`NOOBOARD_STORAGE_HISTORY_PAGING_PLAN.md`](./NOOBOARD_STORAGE_HISTORY_PAGING_PLAN.md)
- keep `ClipboardRecord` as the history row/detail model unless a later spec change proves a
  second DTO is necessary
- convert GUI clipboard history from an ever-growing flat row list into a bounded page cache
- represent unloaded history outside the active window with explicit newer/older gap rows
- retain only the selected record or other minimal continuity state needed when bounded page
  eviction would otherwise break the detail panel
- render clipboard history in a fixed-height internal scroll region with virtualized/windowed row
  rendering
- preserve scroll position when new head records arrive; if the user is away from the top, show a
  pending-new-items affordance instead of force-prepending visible rows
- fill unloaded gaps with the matching `Newer` or `Older` request instead of forcing a rebuild
  from the head

Exit criteria:

- rendered history row count remains bounded as history grows
- GUI clipboard-history memory use is bounded by page-window and detail-cache limits
- scrolling older/newer history remains stable when new clipboard commits arrive
- no compatibility layer or second business-state source is introduced

## 11. Verification Checklist

The port is only complete if all items below are true:

- `cargo check -p nooboard-gui` passes
- GUI launches using `NooboardCore`
- GUI no longer imports `nooboard-app`
- GUI no longer imports `nooboard-sync`
- bootstrap chooser flow works
- workspace opens with a valid `WorkspaceSnapshot`
- clipboard actions work
- direct seed CRUD works
- LAN/network controls work
- transfer actions work
- settings edits work
- visual parity is acceptable
- responsive parity is acceptable

## 12. Final Deletion Checklist

After successful cutover:

- delete `/Users/zero/study/rust/nooboard/crates/nooboard-desktop`
- delete `/Users/zero/study/rust/nooboard/crates/nooboard-app`
- delete `/Users/zero/study/rust/nooboard/crates/nooboard-sync`
- update workspace metadata and developer docs

This deletion is part of the plan, not a future optional cleanup.
