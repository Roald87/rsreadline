#!/usr/bin/env bash
set -euo pipefail

REPO="Roald87/rsreadline"
INSTALL_DIR="${RSREADLINE_INSTALL_DIR:-$HOME/.local/bin}"

os="$(uname -s)"
if [ "$os" != "Linux" ]; then
    echo "error: rsreadline only supports Linux (bash integration relies on Linux-specific syscalls)" >&2
    exit 1
fi

arch="$(uname -m)"
case "$arch" in
    x86_64) target="x86_64-unknown-linux-gnu" ;;
    *)
        echo "error: no prebuilt binary for architecture '$arch' yet — build from source instead:" >&2
        echo "  cargo install --git https://github.com/$REPO" >&2
        exit 1
        ;;
esac

asset="rsreadline-$target"
url="https://github.com/$REPO/releases/latest/download/$asset"

mkdir -p "$INSTALL_DIR"
echo "Downloading $asset..."
curl -fsSL "$url" -o "$INSTALL_DIR/rsreadline"
chmod +x "$INSTALL_DIR/rsreadline"

echo "Installed to $INSTALL_DIR/rsreadline"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        echo
        echo "warning: $INSTALL_DIR is not on your PATH. Add this to your .bashrc:"
        echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
        ;;
esac

echo
echo "Add this to your .bashrc to enable rsreadline:"
echo "  eval \"\$($INSTALL_DIR/rsreadline init bash)\""
