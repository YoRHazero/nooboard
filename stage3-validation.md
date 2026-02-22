# nooboard Stage 3 Validation Guide

## 1. Purpose
This document verifies Stage 3 (P2P real-time sync) against `stage3.md` DoD with reproducible commands and result recording.

Validation focus:
1. `sync` command can run as a node listener.
2. Peer connectivity by manual `--peer` fallback.
3. mDNS discovery path (`--no-mdns` disabled).
4. No replay semantics after offline period.
5. Dedup behavior (`origin_device_id + origin_seq`) and no obvious loop storms.
6. Synced content is queryable in local `history`.

## 2. Test Assets in Workspace
The following temporary configs are prepared under workspace:
1. `/Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/config.toml`
2. `/Users/zero/study/rust/nooboard/tmp/nooboard-stage3/b/config.toml`
3. `/Users/zero/study/rust/nooboard/tmp/nooboard-stage3/c/config.toml`

Each node writes to its own DB file:
1. `/Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/nooboard.db`
2. `/Users/zero/study/rust/nooboard/tmp/nooboard-stage3/b/nooboard.db`
3. `/Users/zero/study/rust/nooboard/tmp/nooboard-stage3/c/nooboard.db`

## 3. Preconditions
1. Build and tests are green:
```bash
cd /Users/zero/study/rust/nooboard
cargo check --workspace
cargo test -p nooboard-storage
cargo test -p nooboard-sync
```
2. Use three terminal windows: Terminal A/B/C.
3. Device IDs must be unique: `dev-a`, `dev-b`, `dev-c`.
4. Token must be the same for all nodes, e.g. `dev-token`.

## 4. Case A: Single Node Startup (DoD-1)
Terminal A:
```bash
cd /Users/zero/study/rust/nooboard
RUST_LOG=info cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/config.toml sync --device-id dev-a --listen 127.0.0.1:8787 --token dev-token --no-mdns
```
Expected:
1. Process stays running.
2. No startup error.

Stop with `Ctrl+C` before next case.

## 5. Case B: Manual Peer Connectivity (DoD-2 fallback, DoD-3 baseline)
Start 3 nodes with manual peers and mDNS disabled.

Terminal A:
```bash
cd /Users/zero/study/rust/nooboard
RUST_LOG=debug cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/config.toml sync --device-id dev-a --listen 127.0.0.1:8787 --token dev-token --no-mdns --peer 127.0.0.1:8788 --peer 127.0.0.1:8789
```

Terminal B:
```bash
cd /Users/zero/study/rust/nooboard
RUST_LOG=debug cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/b/config.toml sync --device-id dev-b --listen 127.0.0.1:8788 --token dev-token --no-mdns --peer 127.0.0.1:8787 --peer 127.0.0.1:8789
```

Terminal C:
```bash
cd /Users/zero/study/rust/nooboard
RUST_LOG=debug cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/c/config.toml sync --device-id dev-c --listen 127.0.0.1:8789 --token dev-token --no-mdns --peer 127.0.0.1:8787 --peer 127.0.0.1:8788
```

Trigger event from node A:
```bash
cd /Users/zero/study/rust/nooboard
cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/config.toml set "stage3-peer-smoke"
```

Check B/C history:
```bash
cd /Users/zero/study/rust/nooboard
cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/b/config.toml history --limit 20
cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/c/config.toml history --limit 20
```

Expected:
1. B/C each contain `stage3-peer-smoke`.
2. No endless repeated inserts.

## 6. Case C: mDNS Auto Discovery (DoD-2)
Restart A/B/C without `--no-mdns` and without `--peer`.

Terminal A:
```bash
cd /Users/zero/study/rust/nooboard
RUST_LOG=debug cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/config.toml sync --device-id dev-a --listen 0.0.0.0:8787 --token dev-token
```

Terminal B:
```bash
cd /Users/zero/study/rust/nooboard
RUST_LOG=debug cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/b/config.toml sync --device-id dev-b --listen 0.0.0.0:8788 --token dev-token
```

Terminal C:
```bash
cd /Users/zero/study/rust/nooboard
RUST_LOG=debug cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/c/config.toml sync --device-id dev-c --listen 0.0.0.0:8789 --token dev-token
```

Trigger event again on A and inspect B/C history as in Case B.

Expected:
1. Nodes connect without manual peers.
2. New clipboard event propagates to other online nodes.

## 7. Case D: Offline-No-Replay and Reconnect (DoD-4)
1. Keep A/B running and connected.
2. Stop B.
3. On A:
```bash
cd /Users/zero/study/rust/nooboard
cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/config.toml set "stage3-offline-missed"
```
4. Restart B sync.
5. Verify B history does **not** contain `stage3-offline-missed`.
6. On A:
```bash
cd /Users/zero/study/rust/nooboard
cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/config.toml set "stage3-after-reconnect"
```
7. Verify B history contains `stage3-after-reconnect`.

Expected:
1. Offline gap is not replayed.
2. Reconnected node receives new online events.

## 8. Case E: Dedup and Loop Control (DoD-5, DoD-6, DoD-7)
With A/B/C online, run on A:
```bash
cd /Users/zero/study/rust/nooboard
cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/config.toml set "stage3-loop-same"
cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/config.toml set "stage3-loop-same"
cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/config.toml set "stage3-loop-diff-1"
cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/config.toml set "stage3-loop-diff-2"
```

Inspect history on all nodes:
```bash
cd /Users/zero/study/rust/nooboard
cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/a/config.toml history --limit 30
cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/b/config.toml history --limit 30
cargo run -p nooboard-cli -- --config /Users/zero/study/rust/nooboard/tmp/nooboard-stage3/c/config.toml history --limit 30
```

Expected:
1. No obvious loop storm in logs.
2. Repeated identical content does not explode as continuous duplicates.
3. Synced text is queryable through `history`.

## 9. DoD Recording Table
Fill this after validation:

| DoD Item | Result (Pass/Fail/Partial) | Evidence (command/log/history snippet) |
|---|---|---|
| 1. sync command can start and listen |  |  |
| 2. Two nodes auto-discover and connect on LAN |  |  |
| 3. Three nodes sync same event online |  |  |
| 4. Reconnect receives new events without replay |  |  |
| 5. Same event applied only once in full-mesh |  |  |
| 6. Downlink handles dedup before local set |  |  |
| 7. history shows synced text |  |  |
| 8. `cargo check --workspace` and `cargo test -p nooboard-sync` pass |  |  |

## 10. Cleanup
Stop running sync processes and remove temporary validation data:
```bash
cd /Users/zero/study/rust/nooboard
rm -rf /Users/zero/study/rust/nooboard/tmp/nooboard-stage3
```

Optional: recreate assets later by re-running setup commands from Section 2.
