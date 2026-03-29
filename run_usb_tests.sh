#!/bin/bash
# USB CDC stress test runner for STM32F469I-DISCO
#
# Uses st-flash deployment (NOT probe-rs) to avoid breaking USB enumeration.
# See AGENTS.md "probe-rs Breaks USB Enumeration" section.
#
# Requirements: st-flash (stlink-tools), arm-none-eabi-objcopy, pyserial, cargo
#
# Usage:
#   ./run_usb_tests.sh                    # build + flash + stress test
#   ./run_usb_tests.sh --build-only       # just build the firmware
#   ./run_usb_tests.sh --flash-only       # flash pre-built firmware
#   ./run_usb_tests.sh --test-only        # run host-side test (assumes already flashed)
#   ./run_usb_tests.sh --count 1000       # send 1000 echo packets
#   ./run_usb_tests.sh --port /dev/ttyACM1

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

CHIP="STM32F469NIHx"
TARGET="thumbv7em-none-eabihf"
EXAMPLE="test_usb_cdc_stress"
ELF_DIR="target/${TARGET}/release/examples"
ELF="${ELF_DIR}/${EXAMPLE}"
BIN="${ELF_DIR}/${EXAMPLE}.bin"
BOOT_DELAY=15
DEFAULT_COUNT=600

BUILD_ONLY=false
FLASH_ONLY=false
TEST_ONLY=false
COUNT=$DEFAULT_COUNT
PORT=""
EXTRA_PYTHON_ARGS=""

while [ $# -gt 0 ]; do
    arg="$1"
    case "$arg" in
        --build-only) BUILD_ONLY=true; shift ;;
        --flash-only) FLASH_ONLY=true; shift ;;
        --test-only) TEST_ONLY=true; shift ;;
        --count) shift; COUNT="${1:-$DEFAULT_COUNT}"; shift ;;
        --port) shift; PORT="${1:-}"; shift ;;
        --find) EXTRA_PYTHON_ARGS="--find"; shift ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "USB CDC stress test runner (st-flash deployment, no probe-rs)"
            echo ""
            echo "Options:"
            echo "  --build-only   Build firmware only"
            echo "  --flash-only   Flash pre-built firmware only"
            echo "  --test-only    Run host-side test only (assumes already flashed)"
            echo "  --count N      Send N echo packets (default: ${DEFAULT_COUNT})"
            echo "  --port PORT    Serial port (default: auto-detect or /dev/ttyACM0)"
            echo "  --find         Auto-detect port by VID:PID"
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

find_port() {
    if [ -n "$PORT" ]; then
        echo "$PORT"
        return
    fi
    if python3 -c "import serial.tools.list_ports" 2>/dev/null; then
        found=$(python3 -c "
import serial.tools.list_ports
for p in serial.tools.list_ports.comports():
    vid = f'{p.vid:04x}' if p.vid else ''
    pid = f'{p.pid:04x}' if p.pid else ''
    if vid == '16c0' and pid == '27dd':
        print(p.device)
        break
" 2>/dev/null || true)
        if [ -n "$found" ]; then
            echo "$found"
            return
        fi
    fi
    if [ -e "/dev/ttyACM0" ]; then
        echo "/dev/ttyACM0"
        return
    fi
    echo ""
}

run_stress_test() {
    local test_port
    test_port=$(find_port)

    if [ -z "$test_port" ]; then
        fail "No USB CDC device found"
        echo "  Expected VID:PID 16c0:27dd (STRESS serial)"
        echo "  Check: python3 -c 'import serial.tools.list_ports; [print(p) for p in serial.tools.list_ports.comports()]'"
        exit 1
    fi

    step "Running stress test on ${test_port} (${COUNT} packets)..."
    local py_args="--count $COUNT"
    if [ -n "$test_port" ]; then
        py_args="$py_args --port $test_port"
    fi
    if [ -n "$EXTRA_PYTHON_ARGS" ]; then
        py_args="$py_args $EXTRA_PYTHON_ARGS"
    fi

    if python3 tests/usb_cdc_stress.py $py_args; then
        ok "Stress test PASSED"
        return 0
    else
        fail "Stress test FAILED"
        return 1
    fi
}

check_deps

echo "=========================================="
echo "  USB CDC Stress Test — STM32F469I-DISCO"
echo "  $(date)"
echo "=========================================="
echo ""

if [ "$TEST_ONLY" = true ]; then
    run_stress_test
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
run_stress_test
exit $?
