#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
TEST_DIR="$ROOT_DIR/MOFAM-test"
INPUT="$TEST_DIR/input/mofam.minimal-qac.toml"
OUT_DIR="$TEST_DIR/output"
TOOLS_CONFIG="$TEST_DIR/input/tools.toml"
MO2_DIR="/home/steven/Games/Wabbajack/Oblivion/MudCrab Test"
MO2_DOWNLOADS="/home/steven/Games/mod-organizer-2-oblivion/modorganizer2/downloads"
CACHE_DIR="${MOFAM_CACHE_DIR:-$MO2_DOWNLOADS}"
export GAME_DIR="${GAME_DIR:-/home/steven/.local/share/Steam/steamapps/common/Oblivion}"

mkdir -p "$OUT_DIR"
mkdir -p "$MO2_DIR" "$MO2_DOWNLOADS"

if [ -z "${NEXUS_API_KEY:-}" ]; then
	echo "Warning: NEXUS_API_KEY is not set. Cached archives will be reused; uncached downloads will fail."
fi

echo "[1/4] compile"
cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -- compile "$INPUT" --output "$OUT_DIR/compiled-minimal.json"

echo "[2/4] query"
cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -- query "$OUT_DIR/compiled-minimal.json" --output "$OUT_DIR/plan-minimal.json" --headless

echo "[3/4] download"
cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -- download "$OUT_DIR/plan-minimal.json" --cache "$CACHE_DIR" --retry 2

echo "[4/4] install (MO2 export)"
cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -- install "$OUT_DIR/plan-minimal.json" --cache "$CACHE_DIR" --mo2-instance-dir "$MO2_DIR" --profile-name "Minimal" --game-dir "$GAME_DIR" --tools-config "$TOOLS_CONFIG"

echo "done: $MO2_DIR"
