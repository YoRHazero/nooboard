#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CONFIG_FILE="$ROOT_DIR/configs/dev.toml"

DB_ROOT="$(sed -nE 's/^[[:space:]]*db_root[[:space:]]*=[[:space:]]*"([^"]+)".*$/\1/p' "$CONFIG_FILE" | head -n 1)"
SCHEMA_VERSION="$(sed -nE 's/^[[:space:]]*schema_version[[:space:]]*=[[:space:]]*"([^"]+)".*$/\1/p' "$CONFIG_FILE" | head -n 1)"

if [[ -z "$DB_ROOT" ]]; then
  echo "failed to parse db_root from $CONFIG_FILE" >&2
  exit 1
fi
if [[ -z "$SCHEMA_VERSION" ]]; then
  echo "failed to parse schema_version from $CONFIG_FILE" >&2
  exit 1
fi

CURRENT_DIR="$DB_ROOT/$SCHEMA_VERSION"
DB_PATH="$CURRENT_DIR/nooboard.db"

rm -rf "$CURRENT_DIR"
mkdir -p "$CURRENT_DIR"

cd "$ROOT_DIR"
cargo run -p nooboard-cli -- --config "$CONFIG_FILE" history --limit 1 >/dev/null

echo "database rebuilt: $DB_PATH"
