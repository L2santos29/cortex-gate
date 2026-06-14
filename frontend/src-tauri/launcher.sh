#!/bin/bash
cd "/home/l2s/Documents/L&S Agent/agency projects/cortex-gate/frontend/src-tauri"
export DISPLAY=:0
exec ./target/release/cortex-gate-tauri
