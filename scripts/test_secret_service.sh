#!/usr/bin/env bash
# Called only inside a fresh D-Bus session with private XDG data/runtime directories.
set -euo pipefail
gnome-keyring-daemon --foreground --components=secrets --unlock \
  --control-directory="$XDG_RUNTIME_DIR/control" <<< 'nooboard-temporary-test-password' \
  >"$XDG_RUNTIME_DIR/daemon.log" 2>&1 &
daemon_pid=$!
trap 'result=$?; if [[ $result != 0 ]]; then cat "$XDG_RUNTIME_DIR/daemon.log"; fi; kill "$daemon_pid" 2>/dev/null || true; wait "$daemon_pid" 2>/dev/null || true' EXIT
for _ in {1..100}; do
  # Query the bus itself: pinging secrets too early auto-starts a second daemon.
  if dbus-send --session --print-reply --dest=org.freedesktop.DBus \
    /org/freedesktop/DBus org.freedesktop.DBus.NameHasOwner string:org.freedesktop.secrets \
    | grep -q 'boolean true'; then break; fi
  sleep 0.05
done
cargo test -p nooboard-storage --locked native_secret_service_roundtrip -- --ignored --test-threads=1
