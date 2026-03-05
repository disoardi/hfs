#!/usr/bin/env bash
# tuxbox run wrapper for hfs.
#
# Executed by tuxbox as:
#   bash scripts/tbox-run.sh [args...]
#
# Working directory: ~/.tuxbox/tools/hfs/ (the cloned repo root)
# Builds (or rebuilds) the release binary when:
#   - the binary is absent, OR
#   - the git HEAD has changed since the binary was last built

set -e

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="$REPO_ROOT/target/release/hfs"
STAMP="$REPO_ROOT/target/.hfs-built-commit"

# Detect current git commit (empty string if git unavailable)
CURRENT_COMMIT=""
if command -v git >/dev/null 2>&1; then
    CURRENT_COMMIT=$(cd "$REPO_ROOT" && git rev-parse HEAD 2>/dev/null || true)
fi

BUILT_COMMIT=""
if [ -f "$STAMP" ]; then
    BUILT_COMMIT=$(cat "$STAMP")
fi

NEED_BUILD=0
if [ ! -f "$BINARY" ]; then
    NEED_BUILD=1
elif [ -n "$CURRENT_COMMIT" ] && [ "$CURRENT_COMMIT" != "$BUILT_COMMIT" ]; then
    NEED_BUILD=1
fi

if [ "$NEED_BUILD" = "1" ]; then
    if [ ! -f "$BINARY" ]; then
        echo "[hfs] First run — building from source (requires Rust stable)."
    else
        echo "[hfs] Source updated ($(echo "$CURRENT_COMMIT" | cut -c1-7)) — rebuilding..."
    fi
    echo "      This may take a minute..."
    echo ""
    (cd "$REPO_ROOT" && cargo build --release -p hfs)
    # Record the commit we built from
    [ -n "$CURRENT_COMMIT" ] && echo "$CURRENT_COMMIT" > "$STAMP"
    echo ""
    echo "[hfs] Build complete. Binary: $BINARY"
fi

exec "$BINARY" "$@"
