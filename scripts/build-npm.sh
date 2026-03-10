#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
NPM_DIR="$PROJECT_DIR/npm"

echo "=== Building doom-mcp npm package ==="

# Build the Rust binary
echo "[1/4] Building binary..."
cd "$PROJECT_DIR"
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
case "$ARCH" in
  x86_64) ARCH="x64" ;;
  aarch64|arm64) ARCH="arm64" ;;
esac
case "$OS" in
  linux) PLATFORM="linux" ;;
  darwin) PLATFORM="darwin" ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

BINARY_NAME="doom-mcp-${PLATFORM}-${ARCH}"
echo "  Platform: ${PLATFORM}-${ARCH}"

# Copy binary
echo "[2/4] Copying binary to npm package..."
cp "$PROJECT_DIR/target/release/doom-mcp" "$NPM_DIR/engine/$BINARY_NAME"
chmod +x "$NPM_DIR/engine/$BINARY_NAME"

# Copy docs (npm includes these from the package root)
echo "[3/4] Copying docs..."
cp "$PROJECT_DIR/README.md" "$NPM_DIR/"
cp "$PROJECT_DIR/LICENSE" "$NPM_DIR/"
cp "$PROJECT_DIR/CHANGELOG.md" "$NPM_DIR/"

# Copy WAD
echo "[4/4] Copying WAD file..."
mkdir -p "$NPM_DIR/wad"
if [ -f "$PROJECT_DIR/wad/freedoom1.wad" ]; then
  cp "$PROJECT_DIR/wad/freedoom1.wad" "$NPM_DIR/wad/"
  echo "  WAD: $(du -h "$NPM_DIR/wad/freedoom1.wad" | cut -f1)"
else
  echo "  WARNING: freedoom1.wad not found. Run scripts/setup.sh first."
fi

# Show package size
echo ""
echo "=== Package contents ==="
du -sh "$NPM_DIR/engine/"* "$NPM_DIR/wad/"* 2>/dev/null
echo ""
echo "Total: $(du -sh "$NPM_DIR" | cut -f1)"
echo ""
echo "To publish: cd npm && npm publish"
echo ""
echo "Users install with:"
echo '  npx -y doom-mcp'
echo ""
echo "Or add to MCP config:"
echo '  {"mcpServers":{"doom":{"command":"npx","args":["-y","doom-mcp"]}}}'
