#!/usr/bin/env bash
# Shell installation script for agy-brain on Linux and macOS
set -euo pipefail

REPO="caiosaboia/agy-brain"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${1:-latest}"

echo "Installing agy-brain..."
mkdir -p "$INSTALL_DIR"

OS="$(uname -s)"
ARCH="$(uname -m)"
ASSET_NAME=""

case "$OS" in
    Linux)
        if [ "$ARCH" = "x86_64" ]; then
            ASSET_NAME="agy-brain-x86_64-unknown-linux-gnu.tar.gz"
        fi
        ;;
    Darwin)
        if [ "$ARCH" = "arm64" ]; then
            ASSET_NAME="agy-brain-aarch64-apple-darwin.tar.gz"
        else
            ASSET_NAME="agy-brain-x86_64-apple-darwin.tar.gz"
        fi
        ;;
esac

INSTALLED=0

if [ -n "$ASSET_NAME" ]; then
    if [ "$VERSION" = "latest" ]; then
        URL="https://github.com/$REPO/releases/latest/download/$ASSET_NAME"
    else
        URL="https://github.com/$REPO/releases/download/$VERSION/$ASSET_NAME"
    fi

    echo "Attempting to download binary from $URL..."
    TMP_DIR="$(mktemp -d)"
    if curl -sSLf "$URL" -o "$TMP_DIR/$ASSET_NAME"; then
        tar -xzf "$TMP_DIR/$ASSET_NAME" -C "$INSTALL_DIR"
        chmod +x "$INSTALL_DIR/agy-brain"
        rm -rf "$TMP_DIR"
        INSTALLED=1
    else
        echo "Release download skipped or unavailable."
        rm -rf "$TMP_DIR"
    fi
fi

if [ "$INSTALLED" -eq 0 ]; then
    if command -v cargo >/dev/null 2>&1; then
        echo "Building from source using Cargo..."
        cargo install --git "https://github.com/$REPO" --force
        INSTALLED=1
    else
        echo "Error: Pre-compiled binary unavailable and cargo is not installed." >&2
        exit 1
    fi
fi

# Determine running shell
DETECTED_SHELL="$(basename "$SHELL")"
if command -v agy-brain >/dev/null 2>&1; then
    agy-brain install "$DETECTED_SHELL"
elif [ -x "$INSTALL_DIR/agy-brain" ]; then
    "$INSTALL_DIR/agy-brain" install "$DETECTED_SHELL"
fi

echo "agy-brain installation completed successfully."
echo "Restart your shell or run 'source ~/.${DETECTED_SHELL}rc' to begin."
