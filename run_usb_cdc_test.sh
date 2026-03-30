#!/bin/bash
# USB CDC connectivity test runner for STM32F469I-DISCO
#
# Uses st-flash deployment (NOT probe-rs) to avoid breaking USB enumeration.
# See AGENTS.md "probe-rs Breaks USB Enumeration" section.
#
# Requirements: st-flash (stlink-tools), arm-none-eabi-objcopy, pyserial, cargo
#
# Usage:
#   ./run_usb_cdc_test.sh                     # build + flash + test
#   ./run_usb_cdc_test.sh --build-only        # just build the firmware
#   ./run_usb_cdc_test.sh --flash-only        # flash pre-built firmware
#   ./run_usb_cdc_test.sh --test-only         # run host-side test (assumes already flashed)
#   ./run_usb_cdc_test.sh --port /dev/ttyACM1
#   ./run_usb_cdc_test.sh --timeout 30

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

CHIP="STM32F469NIHx"
TARGET="thumbv7em-none-eabihf"
EXAMPLE="test_usb_cdc"
ELF_DIR="target/${TARGET}/release/examples"
ELF="${ELF_DIR}/${EXAMPLE}"
BIN="${ELF_DIR}/${EXAMPLE}.bin"
BOOT_DELAY=15
DEFAULT_TIMEOUT=30

BUILD_ONLY=false
FLASH_ONLY=false
TEST_ONLY=false
PORT=""
TIMEOUT=$DEFAULT_TIMEOUT

while [ $# -gt 0 ]; do
    arg="$1"
    case "$arg" in
        --build-only) BUILD_ONLY=true; shift ;;
        --flash-only) FLASH_ONLY=true; shift ;;
        --test-only) TEST_ONLY=true; shift ;;
        --port) shift; PORT="${1:-}"; shift ;;
        --timeout) shift; TIMEOUT="${1:-$DEFAULT_TIMEOUT}"; shift ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "USB CDC connectivity test runner (st-flash deployment, no probe-rs)"
            echo ""
            echo "Options:"
            echo "  --build-only   Build firmware only"
            echo "  --flash-only   Flash pre-built firmware only"
            echo "  --test-only    Run host-side test only (assumes already flashed)"
            echo "  --port PORT    Serial port (default: /dev/ttyACM0)"
            echo "  --timeout N    Host-side read timeout in seconds (default: ${DEFAULT_TIMEOUT})"
            echo ""
            echo "Requirements:"
            echo "  - st-flash (stlink-tools)"
            echo "  - arm-none-eabi-objcopy"
            echo "  - pyserial (pip install pyserial)"
            echo "  - cargo + ARM target"
            exit 0
            ;;
    esac
done

step() {
    echo -e "${CYAN}>>> $1${NC}"
}

ok() {
    echo -e "${GREEN}>>> $1${NC}"
}

fail() {
    echo -e "${RED}>>> $1${NC}"
}

check_deps() {
    local missing=""
    if ! command -v st-flash &>/dev/null; then
        missing="$missing st-flash"
    fi
    if ! command -v arm-none-eabi-objcopy &>/dev/null; then
        missing="$missing arm-none-eabi-objcopy"
    fi
    if ! python3 -c "import serial" 2>/dev/null; then
        missing="$missing pyserial"
    fi
    if [ -n "$missing" ]; then
        fail "Missing dependencies:$missing"
        echo "  st-flash:      apt install stlink-tools"
        echo "  arm-none-eabi:  apt install gcc-arm-none-eabi"
        echo "  pyserial:       pip install pyserial"
        exit 2
    fi
}

build_firmware() {
    step "Building ${EXAMPLE} (release, ${TARGET})..."
    if ! cargo build --release --example "$EXAMPLE" --target "$TARGET" 2>&1; then
        fail "Build failed"
        exit 1
    fi
    if [ ! -f "$ELF" ]; then
        fail "ELF not found: $ELF"
        exit 1
    fi
    ok "Build OK: $ELF"
}

convert_to_bin() {
    step "Converting to binary..."
    arm-none-eabi-objcopy -O binary "$ELF" "$BIN"
    ok "Binary: $BIN ($(wc -c < "$BIN") bytes)"
}

flash_firmware() {
    step "Flashing via st-flash (connect-under-reset)..."
    if ! st-flash --connect-under-reset write "$BIN" 0x08000000 2>&1; then
        fail "Flash failed — try: st-flash --connect-under-reset reset && retry"
        exit 1
    fi
    ok "Flash OK"
}

reset_board() {
    step "Resetting board..."
    st-flash --connect-under-reset reset 2>&1 || true
    echo -e "${YELLOW}Waiting ${BOOT_DELAY}s for USB enumeration...${NC}"
    sleep "$BOOT_DELAY"
    ok "Board reset complete"
}

run_cdc_test() {
    local test_port="${PORT:-/dev/ttyACM0}"

    step "Running USB CDC test on ${test_port} (timeout ${TIMEOUT}s)..."
    local py_args="--port $test_port --timeout $TIMEOUT"

    if python3 tests/usb_cdc_test.py $py_args; then
        ok "USB CDC test PASSED"
        return 0
    else
        fail "USB CDC test FAILED"
        return 1
    fi
}

check_deps

echo "=========================================="
echo "  USB CDC Test — STM32F469I-DISCO"
echo "  $(date)"
echo "=========================================="
echo ""

if [ "$TEST_ONLY" = true ]; then
    run_cdc_test
    exit $?
fi

build_firmware
convert_to_bin

if [ "$BUILD_ONLY" = true ]; then
    exit 0
fi

flash_firmware

if [ "$FLASH_ONLY" = true ]; then
    exit 0
fi

reset_board
run_cdc_test
exit $?
