#!/usr/bin/env bash
set -euo pipefail

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    echo "repair: install $1 and rerun scripts/smoke/telegram-approval-live-simulated.sh" >&2
    exit 2
  fi
}

need_cmd cargo

cargo test -p runlane-core telegram -- --nocapture
cargo run -p runlane -- telegram approval live-simulated-smoke
