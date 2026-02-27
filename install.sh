#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
BINDIR="$PREFIX/bin"
SYSTEMD_USER_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
DESKTOP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"

echo "Building geisterhand..."
cargo build --release

echo "Installing binary to $BINDIR..."
mkdir -p "$BINDIR"
install -m 755 target/release/geisterhand "$BINDIR/geisterhand"

echo "Installing systemd user service..."
mkdir -p "$SYSTEMD_USER_DIR"
sed "s|%h/.cargo/bin/geisterhand|$BINDIR/geisterhand|g" geisterhand.service > "$SYSTEMD_USER_DIR/geisterhand.service"

echo "Installing .desktop file..."
mkdir -p "$DESKTOP_DIR"
sed "s|Exec=geisterhand|Exec=$BINDIR/geisterhand|g" geisterhand.desktop > "$DESKTOP_DIR/geisterhand.desktop"

echo ""
echo "Installation complete!"
echo ""
echo "Usage:"
echo "  geisterhand server              # Start the HTTP API server"
echo "  geisterhand run <app>            # Launch app with scoped server"
echo "  geisterhand mcp                  # Run as MCP server (stdio)"
echo "  geisterhand check <app>          # Check accessibility support"
echo ""
echo "Optional: Enable as a systemd user service:"
echo "  systemctl --user daemon-reload"
echo "  systemctl --user enable --now geisterhand"
