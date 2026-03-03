#!/usr/bin/env bash
# tuxbox run wrapper for hfs.
#
# Executed by tuxbox as:
#   bash scripts/tbox-run.sh [args...]
#
# Working directory: ~/.tuxbox/tools/hfs/ (the cloned repo root)
# On first run: builds the release binary with cargo (Rust required).
# On subsequent runs: executes the cached binary directly.

set -e

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="$REPO_ROOT/target/release/hfs"

# Build if the release binary is absent or the source tree is newer than the binary.
if [ ! -f "$BINARY" ]; then
    echo "[hfs] First run — building from source (requires Rust stable)."
    echo "      This may take a minute..."
    echo ""
    (cd "$REPO_ROOT" && cargo build --release -p hfs)
    echo ""
    echo "[hfs] Build complete. Binary: $BINARY"
fi

exec "$BINARY" "$@"
