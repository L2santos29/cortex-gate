#!/bin/bash
set -e

echo "========================================"
echo "STEP 1: Killing cortex processes"
echo "========================================"
pkill -9 -f 'cortex-gate-tauri' 2>/dev/null || echo "no tauri process"
pkill -9 -f 'cortex-gate' 2>/dev/null || echo "no gate process"
sleep 2

echo ""
echo "--- Port 18801 check ---"
ss -tlnp 2>/dev/null | grep 18801 || echo "puerto libre"

echo ""
echo "--- Process check ---"
ps aux | grep -i cortex | grep -v grep || echo "no cortex processes"

echo ""
echo "========================================"
echo "STEP 2: Rebuild frontend"
echo "========================================"
cd frontend
npm run build 2>&1 | tail -5
echo "frontend build exit: ${PIPESTATUS[0]}"

echo ""
echo "========================================"
echo "STEP 3: Rebuild Tauri binary"
echo "========================================"
cd src-tauri
cargo build --release 2>&1 | tail -20
echo "tauri build exit: ${PIPESTATUS[0]}"

echo ""
echo "========================================"
echo "STEP 4: Launch backend"
echo "========================================"
cd /home/l2s/Documents/L&S\ Agent/agency\ projects/cortex-gate
cargo run --release > /tmp/cortex-backend.log 2>&1 &
BGPID=$!
echo "Backend PID: $BGPID"
sleep 6

echo ""
echo "--- Health check ---"
curl -s http://127.0.0.1:18801/health || echo "health check failed"

echo ""
echo "========================================"
echo "STEP 5: Launch Tauri UI"
echo "========================================"
DISPLAY=:0 /home/l2s/Documents/L&S\ Agent/agency\ projects/cortex-gate/frontend/src-tauri/target/release/cortex-gate-tauri > /tmp/cortex-ui.log 2>&1 &
echo "Tauri PID: $!"

sleep 3

echo ""
echo "========================================"
echo "FINAL VERIFICATION"
echo "========================================"
ps aux | grep -i cortex | grep -v grep || echo "no cortex processes running"

echo ""
echo "=== Backend log tail ==="
tail -3 /tmp/cortex-backend.log

echo ""
echo "=== ALL DONE ==="
