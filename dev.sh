#!/bin/bash
# YoYo dev launcher with automatic crash logging
# Usage: ./dev.sh

LOG_DIR="$HOME/.yoyo"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/dev-$(date +%Y%m%d-%H%M%S).log"

export RUST_BACKTRACE=full

echo "=== YoYo Dev ==="
echo "Log: $LOG_FILE"
echo "Rust log: $LOG_DIR/yoyo.log"
echo ""

# Run tauri dev, tee all output to log file
cargo tauri dev 2>&1 | tee "$LOG_FILE"
EXIT_CODE=${PIPESTATUS[0]}

# Check for macOS crash report
if [ $EXIT_CODE -ne 0 ]; then
    echo ""
    echo "=== Exit code: $EXIT_CODE ==="
    CRASH=$(ls -t ~/Library/Logs/DiagnosticReports/yoyo-*.ips 2>/dev/null | head -1)
    if [ -n "$CRASH" ]; then
        echo "Crash report: $CRASH"
        cp "$CRASH" "$LOG_DIR/last-crash.ips"
        echo "Copied to: $LOG_DIR/last-crash.ips"
    fi
fi
