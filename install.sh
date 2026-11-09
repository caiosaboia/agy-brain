#!/usr/bin/env bash
# Shell installation script for brain on Linux and macOS
set -euo pipefail

REPO="caiosaboia/agy-brain"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${1:-latest}"

echo "Installing brain..."
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
        if [ -f "$INSTALL_DIR/agy-brain" ] && [ ! -f "$INSTALL_DIR/brain" ]; then
            cp "$INSTALL_DIR/agy-brain" "$INSTALL_DIR/brain"
        fi
        chmod +x "$INSTALL_DIR/brain" 2>/dev/null || true
        chmod +x "$INSTALL_DIR/agy-brain" 2>/dev/null || true
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

# Ensure both brain and agy-brain exist in INSTALL_DIR
if [ -f "$INSTALL_DIR/brain" ] && [ ! -f "$INSTALL_DIR/agy-brain" ]; then
    ln -sf "$INSTALL_DIR/brain" "$INSTALL_DIR/agy-brain"
elif [ -f "$INSTALL_DIR/agy-brain" ] && [ ! -f "$INSTALL_DIR/brain" ]; then
    ln -sf "$INSTALL_DIR/agy-brain" "$INSTALL_DIR/brain"
fi

# Determine running shell
DETECTED_SHELL="$(basename "$SHELL")"
if command -v brain >/dev/null 2>&1; then
    brain install "$DETECTED_SHELL"
elif [ -x "$INSTALL_DIR/brain" ]; then
    "$INSTALL_DIR/brain" install "$DETECTED_SHELL"
elif command -v agy-brain >/dev/null 2>&1; then
    agy-brain install "$DETECTED_SHELL"
elif [ -x "$INSTALL_DIR/agy-brain" ]; then
    "$INSTALL_DIR/agy-brain" install "$DETECTED_SHELL"
fi

echo "brain installation completed successfully."
echo "Restart your shell or run 'source ~/.${DETECTED_SHELL}rc' to begin."
