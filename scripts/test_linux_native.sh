#!/usr/bin/env bash
# Isolated desktop/credential tests. Needs Xvfb, Sway, dbus-run-session and gnome-keyring.
set -euo pipefail
test_root=$(mktemp -d)
pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  for pid in "${pids[@]}"; do wait "$pid" 2>/dev/null || true; done
  rm -rf -- "$test_root"
}
trap cleanup EXIT

Xvfb -displayfd 3 -screen 0 1280x720x24 -nolisten tcp 3>"$test_root/display" >"$test_root/xvfb.log" 2>&1 &
pids+=("$!")
for _ in {1..100}; do
  [[ -s "$test_root/display" ]] && break
  sleep 0.05
done
[[ -s "$test_root/display" ]] || { cat "$test_root/xvfb.log"; exit 1; }
DISPLAY=":$(cat "$test_root/display")" NOOBOARD_LINUX_BACKEND=x11 \
  cargo test -p nooboard-clipboard --locked -- --ignored --test-threads=1

mkdir -m 700 "$test_root/wayland"
printf 'output * mode 1280x720\nxwayland disable\nseat seat0 fallback true\n' > "$test_root/sway.conf"
XDG_RUNTIME_DIR="$test_root/wayland" WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
  sway --unsupported-gpu -c "$test_root/sway.conf" >"$test_root/sway.log" 2>&1 &
pids+=("$!")
for _ in {1..100}; do
  [[ -S "$test_root/wayland/wayland-1" ]] && break
  sleep 0.05
done
[[ -S "$test_root/wayland/wayland-1" ]] || { cat "$test_root/sway.log"; exit 1; }
XDG_RUNTIME_DIR="$test_root/wayland" WAYLAND_DISPLAY=wayland-1 NOOBOARD_LINUX_BACKEND=wayland \
  cargo test -p nooboard-clipboard --locked -- --ignored --test-threads=1

mkdir -m 700 "$test_root/keyring" "$test_root/data"
XDG_RUNTIME_DIR="$test_root/keyring" XDG_DATA_HOME="$test_root/data" \
  dbus-run-session -- bash scripts/test_secret_service.sh
