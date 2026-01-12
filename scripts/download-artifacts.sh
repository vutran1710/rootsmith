#!/bin/bash
# Download production artifacts for Multiple Circuit Rust Package

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ARTIFACTS_DIR="$SCRIPT_DIR/../artifacts"

mkdir -p "$ARTIFACTS_DIR"

echo "═══════════════════════════════════════════════════════════"
echo "  Multiple Circuit - Artifact Download"
echo "═══════════════════════════════════════════════════════════"
echo ""

# Production Artifact URLs
ZKEY_URL="https://cuda.network/main.zkey"
GRAPH_URL="https://cuda.network/main.graph.bin"

# Download zkey (1.2GB)
ZKEY_PATH="$ARTIFACTS_DIR/main.zkey"
if [ -f "$ZKEY_PATH" ]; then
    echo "✓ main.zkey already exists ($(du -h "$ZKEY_PATH" | cut -f1))"
else
    echo "⬇ Downloading main.zkey (1.2GB)..."
    echo "  This may take several minutes..."

    curl -L "$ZKEY_URL" \
        -o "$ZKEY_PATH" \
        --progress-bar

    echo "✓ Downloaded main.zkey ($(du -h "$ZKEY_PATH" | cut -f1))"
fi

# Download graph.bin (132MB)
GRAPH_PATH="$ARTIFACTS_DIR/main.graph.bin"
if [ -f "$GRAPH_PATH" ]; then
    echo "✓ main.graph.bin already exists ($(du -h "$GRAPH_PATH" | cut -f1))"
else
    echo "⬇ Downloading main.graph.bin (132MB)..."

    curl -L "$GRAPH_URL" \
        -o "$GRAPH_PATH" \
        --progress-bar

    echo "✓ Downloaded main.graph.bin ($(du -h "$GRAPH_PATH" | cut -f1))"
fi

echo ""
echo "═══════════════════════════════════════════════════════════"
echo "  Artifact Status"
echo "═══════════════════════════════════════════════════════════"
echo ""

check_file() {
    if [ -f "$1" ]; then
        echo "✓ $(basename "$1") ($(du -h "$1" | cut -f1))"
    else
        echo "✗ $(basename "$1") - MISSING"
    fi
}

check_file "$ARTIFACTS_DIR/main.zkey"
check_file "$ARTIFACTS_DIR/main.graph.bin"
check_file "$ARTIFACTS_DIR/vkey.json"

echo ""
echo "═══════════════════════════════════════════════════════════"
echo ""

# Check platforms
echo "Checking platform libraries..."
PLATFORMS_DIR="$SCRIPT_DIR/../platforms"
if [ -d "$PLATFORMS_DIR/aarch64-apple-darwin" ]; then
    echo "✓ aarch64-apple-darwin (macOS ARM)"
fi
if [ -d "$PLATFORMS_DIR/x86_64" ]; then
    echo "✓ x86_64 (Linux)"
fi

echo ""
echo "Setup complete!"
