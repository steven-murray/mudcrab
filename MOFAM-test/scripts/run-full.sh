#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
TEST_DIR="$ROOT_DIR/MOFAM-test"
INPUT="$TEST_DIR/input/mofam.full.toml"
OUT_DIR="$TEST_DIR/output"
TOOLS_CONFIG="$TEST_DIR/input/tools.toml"
MO2_DIR="/home/steven/Games/Wabbajack/Oblivion/MudCrab Test"
# A directory holding pre-existing downloads that we will use for this run so
# that we don't actually have to download (many) files. This is different than 
# the MO2_DIR/downloads because it is a pre-existing directory that I want to re-use.
MO2_DOWNLOADS="/home/steven/Games/mod-organizer-2-oblivion/modorganizer2/downloads"
CACHE_DIR="${MOFAM_CACHE_DIR:-$MO2_DOWNLOADS}"
export GAME_DIR="${GAME_DIR:-/home/steven/.local/share/Steam/steamapps/common/Oblivion}"

mkdir -p "$OUT_DIR"
mkdir -p "$MO2_DIR" "$MO2_DOWNLOADS"

# Pre-cache archives only when using a separate cache directory.
pre_cache() {
	local cache_name="$1"
	local src_file="$2"
	local orig_name="$3"
	if [ "$CACHE_DIR" = "$MO2_DOWNLOADS" ]; then
		return 0
	fi
	mkdir -p "$CACHE_DIR"
	if [ ! -f "$CACHE_DIR/$cache_name" ]; then
		echo "Pre-caching $cache_name from MO2 downloads..."
		if [ -f "$MO2_DOWNLOADS/$cache_name" ]; then
			cp "$MO2_DOWNLOADS/$cache_name" "$CACHE_DIR/$cache_name"
		elif [ -f "$MO2_DOWNLOADS/$src_file" ]; then
			cp "$MO2_DOWNLOADS/$src_file" "$CACHE_DIR/$cache_name"
		else
			echo "Warning: could not pre-cache $cache_name; no source file found in $MO2_DOWNLOADS" >&2
			return 0
		fi
		echo "$orig_name" > "$CACHE_DIR/$cache_name.orig-name"
	fi
}

pre_cache "xOBSE_0_1731777885" \
	"xOBSE 22.11-37952-22-11-1731777885.zip" \
	"xOBSE 22.11-37952-22-11-1731777885.zip"

if [ -z "${NEXUS_API_KEY:-}" ]; then
	echo "Warning: NEXUS_API_KEY is not set. Cached archives will be reused; uncached downloads will fail."
fi

echo "[1/5] compile"
cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -- compile "$INPUT" --output "$OUT_DIR/compiled.json"

echo "[2/5] query"
cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -- query "$OUT_DIR/compiled.json" --output "$OUT_DIR/plan.json" --headless

echo "[3/5] download"
cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -- download "$OUT_DIR/plan.json" --cache "$CACHE_DIR" --retry 2

echo "[4/5] check"
cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -- check "$OUT_DIR/plan.json" --cache "$CACHE_DIR"

echo "[5/5] install (MO2 export)"
cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -- install "$OUT_DIR/plan.json" --cache "$CACHE_DIR" --mo2-instance-dir "$MO2_DIR" --profile-name "Default" --game-dir "$GAME_DIR" --tools-config "$TOOLS_CONFIG"

echo "done: $MO2_DIR"
