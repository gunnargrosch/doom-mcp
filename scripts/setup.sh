#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ENGINE_DIR="$PROJECT_DIR/engine"
WAD_DIR="$PROJECT_DIR/wad"

FREEDOOM_VERSION="0.13.0"
FREEDOOM_URL="https://github.com/freedoom/freedoom/releases/download/v${FREEDOOM_VERSION}/freedoom-${FREEDOOM_VERSION}.zip"

# --- Helpers ---

check_cmd() {
  if ! command -v "$1" &>/dev/null; then
    echo "Error: $1 is required but not installed."
    exit 1
  fi
}

echo "=== doom-mcp setup ==="
echo ""

# --- Check prerequisites ---
echo "[prereqs] Checking build tools..."
check_cmd git
check_cmd gcc
check_cmd make
check_cmd curl
check_cmd unzip
echo "  All build tools found."
echo ""

# --- Clone doomgeneric (compiled into Rust binary via build.rs) ---
echo "[1/2] Setting up doomgeneric..."
mkdir -p "$ENGINE_DIR"
if [ ! -d "$ENGINE_DIR/doomgeneric" ]; then
  git clone https://github.com/ozkl/doomgeneric.git "$ENGINE_DIR/doomgeneric"
  cd "$ENGINE_DIR/doomgeneric"
  git checkout fc601639494e089702a1ada082eb51aaafc03722 # pin to known-good commit
  cd "$PROJECT_DIR"
else
  echo "  Already cloned."
fi

# Apply patches if any exist
if [ -d "$PROJECT_DIR/patches/doomgeneric" ]; then
  echo "  Applying patches..."
  cd "$ENGINE_DIR/doomgeneric"
  for patch in "$PROJECT_DIR/patches/doomgeneric"/*.patch; do
    [ -f "$patch" ] && git apply --check "$patch" 2>/dev/null && git apply "$patch" && echo "    Applied $(basename "$patch")"
  done
  cd "$PROJECT_DIR"
fi

# --- Download Freedoom WAD ---
echo "[2/2] Downloading Freedoom WAD..."
mkdir -p "$WAD_DIR"
if [ ! -f "$WAD_DIR/freedoom1.wad" ]; then
  TEMP_ZIP="$(mktemp /tmp/freedoom-XXXXXX.zip)"
  echo "  Downloading Freedoom v${FREEDOOM_VERSION}..."
  curl -fSL "$FREEDOOM_URL" -o "$TEMP_ZIP"
  unzip -q -o "$TEMP_ZIP" -d /tmp/freedoom-extract-$$
  mv "/tmp/freedoom-extract-$$/freedoom-${FREEDOOM_VERSION}/freedoom1.wad" "$WAD_DIR/"
  mv "/tmp/freedoom-extract-$$/freedoom-${FREEDOOM_VERSION}/freedoom2.wad" "$WAD_DIR/" 2>/dev/null || true
  rm -rf "/tmp/freedoom-extract-$$" "$TEMP_ZIP"

  # Verify WAD integrity via SHA256 checksum
  EXPECTED_SHA256="7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d"
  ACTUAL_SHA256=$(sha256sum "$WAD_DIR/freedoom1.wad" | cut -d' ' -f1)
  if [ "$ACTUAL_SHA256" != "$EXPECTED_SHA256" ]; then
    echo "  WARNING: WAD checksum mismatch. File may be corrupted."
    echo "    Expected: $EXPECTED_SHA256"
    echo "    Actual:   $ACTUAL_SHA256"
  fi

  echo "  Downloaded freedoom1.wad"
else
  echo "  Already present."
fi

echo ""
echo "=== Setup complete ==="
echo ""
echo "Next steps:"
echo "  cargo build --release    # Compile the MCP server + Doom engine"
echo ""
echo "Register with Claude Code:"
echo "  claude mcp add doom -- $PROJECT_DIR/target/release/doom-mcp"
echo ""
echo "Then say: \"Let's play DOOM\""
