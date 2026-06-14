#!/usr/bin/env bash
# ==========================================================================
# Cortex Gate — Desktop UI Launcher
#
# Builds (if needed) and launches the Tauri desktop application with a
# single command. Double-click this file (or run it from the terminal)
# to open the Cortex Gate UI.
#
# Usage:
#   ./launch-ui.sh              # Build (if needed) and launch
#   ./launch-ui.sh --rebuild    # Force rebuild from scratch
#   ./launch-ui.sh --help       # Show this help
# ==========================================================================

set -euo pipefail

# ── Config ──────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR"
FRONTEND_DIR="$PROJECT_DIR/frontend"
TAURI_DIR="$FRONTEND_DIR/src-tauri"
DIST_DIR="$FRONTEND_DIR/dist"

# The Tauri binary name comes from Cargo.toml [[bin]] name
BINARY_NAME="cortex-gate-tauri"
BINARY_DIR="$TAURI_DIR/target/release"
BINARY="$BINARY_DIR/$BINARY_NAME"

# ── Help ────────────────────────────────────────────────────────────────────

if [[ "${1:-}" == "--help" ]]; then
    echo "Cortex Gate — Desktop UI Launcher"
    echo ""
    echo "Usage:"
    echo "  ./launch-ui.sh              Build (if needed) and launch"
    echo "  ./launch-ui.sh --rebuild    Force rebuild from scratch"
    echo "  ./launch-ui.sh --help       Show this help"
    echo ""
    echo "The launcher automatically:"
    echo "  1. Builds the frontend (Vite) if 'dist/' is missing"
    echo "  2. Builds the Tauri binary if not found"
    echo "  3. Launches the desktop application"
    exit 0
fi

# ── Banner ──────────────────────────────────────────────────────────────────

echo ""
echo "  ╔══════════════════════════════════════════╗"
echo "  ║        🧠  Cortex Gate  —  Desktop UI    ║"
echo "  ╚══════════════════════════════════════════╝"
echo ""

# ── Step 1: Build frontend if needed ───────────────────────────────────────

if [[ ! -f "$DIST_DIR/index.html" ]] || [[ "${1:-}" == "--rebuild" ]]; then
    echo "  🔨 Building frontend (Vite)..."
    cd "$FRONTEND_DIR"

    # Ensure node_modules exist
    if [[ ! -d "node_modules" ]]; then
        echo "  📦 Installing npm dependencies..."
        npm install
    fi

    npm run build
    echo "  ✅ Frontend built"
else
    echo "  ✅ Frontend dist found (use --rebuild to force)"
fi

# ── Step 2: Build Tauri binary if needed ────────────────────────────────────

if [[ ! -f "$BINARY" ]] || [[ "${1:-}" == "--rebuild" ]]; then
    echo "  🔨 Building Tauri desktop app (cargo build --release)..."
    cd "$TAURI_DIR"

    if [[ "${1:-}" == "--rebuild" ]]; then
        # Clean only the binary, not the whole cache
        cargo clean --release 2>/dev/null || true
    fi

    cargo build --release
    echo "  ✅ Tauri app built"
else
    echo "  ✅ Tauri binary found (use --rebuild to force)"
fi

# ── Step 3: Launch ──────────────────────────────────────────────────────────

echo ""
echo "  🚀 Launching Cortex Gate..."
echo ""

"$BINARY"
