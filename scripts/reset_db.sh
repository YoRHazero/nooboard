#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CONFIG_FILE="$ROOT_DIR/configs/dev.toml"

DB_PATH="$(sed -nE 's/^[[:space:]]*db_path[[:space:]]*=[[:space:]]*"([^"]+)".*$/\1/p' "$CONFIG_FILE" | head -n 1)"
if [[ -z "$DB_PATH" ]]; then
  echo "failed to parse db_path from $CONFIG_FILE" >&2
  exit 1
fi

rm -f "$DB_PATH"
mkdir -p "$(dirname "$DB_PATH")"

cd "$ROOT_DIR"
cargo run -p nooboard-cli -- history --limit 1 >/dev/null

echo "database rebuilt: $DB_PATH"
