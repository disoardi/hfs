#!/bin/sh
# hfs installer — detects platform, downloads prebuilt binary from GitHub Releases.
# Falls back to building from source if no prebuilt binary is available for the platform.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/disoardi/hfs/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/disoardi/hfs/main/install.sh | HFS_INSTALL_DIR=~/.local/bin sh

set -e

REPO="disoardi/hfs"
BINARY="hfs"
INSTALL_DIR="${HFS_INSTALL_DIR:-/usr/local/bin}"

# ── Platform detection ────────────────────────────────────────────────────────

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        case "$ARCH" in
            x86_64)          ASSET="hfs-linux-x86_64" ;;
            aarch64|arm64)   ASSET="hfs-linux-arm64" ;;
            *)
                echo "Unsupported Linux architecture: $ARCH"
                echo "Please build from source: https://github.com/${REPO}#build-from-source"
                exit 1
                ;;
        esac
        ;;
    Darwin)
        case "$ARCH" in
            arm64)           ASSET="hfs-macos-arm64" ;;
            x86_64)          ASSET="hfs-macos-x86_64" ;;
            *)
                echo "Unsupported macOS architecture: $ARCH"
                exit 1
                ;;
        esac
        ;;
    *)
        echo "Unsupported OS: $OS"
        echo "Please build from source: https://github.com/${REPO}#build-from-source"
        exit 1
        ;;
esac

DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ASSET}"

# ── Install directory ─────────────────────────────────────────────────────────

if [ ! -w "$INSTALL_DIR" ] 2>/dev/null; then
    FALLBACK_DIR="$HOME/.local/bin"
    printf '\n[hfs] No write access to %s\n' "$INSTALL_DIR"
    printf '      Installing to %s instead.\n' "$FALLBACK_DIR"
    printf '      Make sure %s is in your PATH.\n\n' "$FALLBACK_DIR"
    INSTALL_DIR="$FALLBACK_DIR"
    mkdir -p "$INSTALL_DIR"
fi

# ── Download ──────────────────────────────────────────────────────────────────

TMP_FILE="$(mktemp)"
trap 'rm -f "$TMP_FILE"' EXIT

printf 'Downloading hfs for %s/%s...\n' "$OS" "$ARCH"

if command -v curl >/dev/null 2>&1; then
    if ! curl -fsSL "$DOWNLOAD_URL" -o "$TMP_FILE" 2>/dev/null; then
        echo "Download failed (release not yet published?)."
        echo "Build from source: https://github.com/${REPO}#build-from-source"
        exit 1
    fi
elif command -v wget >/dev/null 2>&1; then
    if ! wget -qO "$TMP_FILE" "$DOWNLOAD_URL" 2>/dev/null; then
        echo "Download failed (release not yet published?)."
        echo "Build from source: https://github.com/${REPO}#build-from-source"
        exit 1
    fi
else
    echo "Neither curl nor wget found. Please install one and retry."
    exit 1
fi

# ── Install ───────────────────────────────────────────────────────────────────

chmod +x "$TMP_FILE"
mv "$TMP_FILE" "${INSTALL_DIR}/${BINARY}"

printf '\nhfs installed successfully!\n'
printf '  Location : %s/%s\n' "$INSTALL_DIR" "$BINARY"
printf '  Version  : %s\n\n' "$("${INSTALL_DIR}/${BINARY}" --version 2>/dev/null || echo 'unknown')"
printf 'Run: hfs --help\n'
