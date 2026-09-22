#!/bin/bash
# Hardware regression: with Bluetooth as the system input, select the built-in mic.
# Captures three seconds in memory only; requires microphone permission.
# Usage: bash scripts/test-microphone.sh [device-uid]
set -euo pipefail
cd "$(dirname "$0")/.."
TEST_DIR="$(mktemp -d)"
trap 'rm -rf "$TEST_DIR"' EXIT
cat > "$TEST_DIR/main.swift" <<'SWIFT'
import Foundation
import AVFoundation
let mic = MicCapture()
let lock = NSLock()
var count = 0
try mic.start(deviceUID: CommandLine.arguments[1], onSamples: { samples in
    lock.lock(); count += samples.count; lock.unlock()
}, onError: { error in
    fputs("Capture error: \(error)\n", stderr)
    exit(1)
})
RunLoop.current.run(until: Date().addingTimeInterval(3))
mic.stop()
lock.lock(); let received = count; lock.unlock()
print("Received \(received) samples")
exit(received > 0 ? 0 : 1)
SWIFT
swiftc Sources/Minutes/Audio/AudioCapture.swift Sources/Minutes/Audio/MicrophoneDevice.swift \
    "$TEST_DIR/main.swift" -o "$TEST_DIR/check"
"$TEST_DIR/check" "${1:-BuiltInMicrophoneDevice}"
