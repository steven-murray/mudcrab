#!/usr/bin/env bash
# Drive the whole mudcrab pipeline against the real MOFAM modlist.
#
# With no arguments this builds the entire list. The usual use is one guide
# section at a time:
#
#   ./MOFAM-test/scripts/run-full.sh --section "5 - LOD"
#   ./MOFAM-test/scripts/run-full.sh --only "Evenstars Colourwheel LOD Update"
#
# --section/--only are passed to download, check, install and diff alike, so a
# section can be fetched, verified, installed and compared with one argument.
# compile and query have no filters: they always process the whole modlist, and
# the plan they produce carries each mod's section for the later stages to
# select on. Flags only `install` understands (--force, --force-merges,
# --skip-actions, --dry-run) are routed to it alone.
#
# The last stage is `diff` against the Oracle. Its output is the point of the
# exercise -- every difference has to be explained, not glanced at.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
TEST_DIR="$ROOT_DIR/MOFAM-test"
INPUT="$TEST_DIR/input/mofam.full.toml"
OUT_DIR="$TEST_DIR/output"
TOOLS_CONFIG="$TEST_DIR/input/tools.toml"

MO2_DIR="/home/steven/Games/Wabbajack/Oblivion/MudCrab Test"
ORACLE_DIR="/home/steven/Games/Wabbajack/Oblivion/MOFAM-03.25/mods"

# The cache stays where every previous run put it. Changing it mid-build would
# orphan the ~140 archives already cached under their content-key names.
MO2_DOWNLOADS="/home/steven/Games/mod-organizer-2-oblivion/modorganizer2/downloads"
CACHE_DIR="${MOFAM_CACHE_DIR:-$MO2_DOWNLOADS}"

# Archives already on disk under their own Nexus filenames. Read-only:
# `--archive-search-path` hard-links a hit into the cache rather than copying
# it, so these folders are never written to and a hit costs no extra disk.
SEARCH_PATHS=(
	"$MO2_DOWNLOADS"
	"$MO2_DIR/downloads"
)
export GAME_DIR="${GAME_DIR:-/home/steven/.local/share/Steam/steamapps/common/Oblivion}"

# Split the arguments: filters go to every stage, install-only flags to install.
FILTERS=()
INSTALL_ONLY=()
while [ $# -gt 0 ]; do
	case "$1" in
	--force | --force-merges | --skip-actions | --dry-run)
		INSTALL_ONLY+=("$1")
		shift
		;;
	*)
		FILTERS+=("$1")
		shift
		;;
	esac
done

MUDCRAB=(cargo run --release --quiet --manifest-path "$ROOT_DIR/Cargo.toml" --bin mudcrab --)

search_args=()
for path in "${SEARCH_PATHS[@]}"; do
	[ -d "$path" ] && search_args+=(--archive-search-path "$path")
done

mkdir -p "$OUT_DIR" "$CACHE_DIR"

if [ -z "${NEXUS_API_KEY:-}" ]; then
	echo "Warning: NEXUS_API_KEY is not set. Archives found in the search paths will still" >&2
	echo "be used; anything that genuinely has to be fetched will fail." >&2
fi

echo "[1/6] compile"
"${MUDCRAB[@]}" compile "$INPUT" --output "$OUT_DIR/compiled.json"

echo "[2/6] query"
"${MUDCRAB[@]}" query "$OUT_DIR/compiled.json" --output "$OUT_DIR/plan.json" --headless

echo "[3/6] download"
"${MUDCRAB[@]}" download "$OUT_DIR/plan.json" --cache "$CACHE_DIR" --retry 2 \
	"${search_args[@]}" "${FILTERS[@]+"${FILTERS[@]}"}"

echo "[4/6] check"
"${MUDCRAB[@]}" check "$OUT_DIR/plan.json" --cache "$CACHE_DIR" "${FILTERS[@]+"${FILTERS[@]}"}"

echo "[5/6] install (MO2 export)"
"${MUDCRAB[@]}" install "$OUT_DIR/plan.json" --cache "$CACHE_DIR" \
	--mo2-instance-dir "$MO2_DIR" --profile-name "Default" \
	--game-dir "$GAME_DIR" --tools-config "$TOOLS_CONFIG" \
	"${search_args[@]}" "${FILTERS[@]+"${FILTERS[@]}"}" "${INSTALL_ONLY[@]+"${INSTALL_ONLY[@]}"}"

echo "[6/6] diff against the Oracle"
"${MUDCRAB[@]}" diff --mods-dir "$MO2_DIR/mods" --oracle "$ORACLE_DIR" \
	--plan "$OUT_DIR/plan.json" "${FILTERS[@]+"${FILTERS[@]}"}"

echo "done: $MO2_DIR"
