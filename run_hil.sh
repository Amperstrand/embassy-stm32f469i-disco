#!/bin/bash
# Unified HIL Test Runner — STM32F469I-DISCO
#
# Runs all hardware-in-the-loop tests in sequence:
#   Phase 1: embedded-test HIL (cargo test, 26 tests via probe-rs)
#   Phase 2: RTT example tests (probe-rs flash + RTT capture, 10 suites)
#   Phase 3: USB CDC tests (st-flash + serial, 2 suites)
#
# Usage:
#   ./run_hil.sh                # run everything
#   ./run_hil.sh --phase hil    # only embedded-test HIL
#   ./run_hil.sh --phase rtt    # only probe-rs RTT examples
#   ./run_hil.sh --phase usb    # only USB CDC tests
#   ./run_hil.sh --skip usb     # run everything except USB
#   ./run_hil.sh --list         # list available phases and tests
#   ./run_hil.sh --json         # JSON results to stdout
#   ./run_hil.sh --find         # auto-detect USB serial port
#   ./run_hil.sh --help
#
# Exit codes: 0 = all pass, 1 = failures, 2 = prerequisites missing

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

CHIP="STM32F469NIHx"
TARGET="thumbv7em-none-eabihf"
RESULTS_DIR="hil-results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_DIR="${RESULTS_DIR}/logs_${TIMESTAMP}"
REPORT_FILE="${RESULTS_DIR}/report_${TIMESTAMP}.txt"

# ── Defaults ──────────────────────────────────────────────────────

RUN_HIL=true
RUN_RTT=true
RUN_USB=true
JSON_OUTPUT=false
USB_PORT=""
FIND_PORT=false
TOTAL_PASSED=0
TOTAL_FAILED=0
TOTAL_SKIPPED=0
declare -a PHASE_RESULTS=()
JSON_HIL_TESTS="[]"
JSON_RTT_TESTS="[]"
JSON_USB_TESTS="[]"
JSON_HIL_PASSED=0
JSON_HIL_FAILED=0
JSON_RTT_PASSED=0
JSON_RTT_FAILED=0
JSON_USB_PASSED=0
JSON_USB_FAILED=0

# ── Functions (must be defined before arg parsing) ────────────────

log()  { echo -e "${CYAN}>>> $1${NC}" >&2; }
ok()   { echo -e "${GREEN}>>> $1${NC}" >&2; }
warn() { echo -e "${YELLOW}>>> $1${NC}" >&2; }
fail() { echo -e "${RED}>>> $1${NC}" >&2; }

do_help() {
    cat <<'EOF'
Usage: ./run_hil.sh [OPTIONS]

Unified HIL test runner for STM32F469I-DISCO BSP.

Phases:
  Phase 1 (hil): embedded-test HIL — cargo test, probe-rs, per-test reset (26 tests)
  Phase 2 (rtt): RTT example tests — probe-rs flash + RTT capture (10 suites)
  Phase 3 (usb): USB CDC tests — st-flash + serial communication (2 suites)

Options:
  --phase PHASE   Run only one phase (hil|rtt|usb)
  --skip PHASE    Skip a phase
  --json          Output JSON results to stdout (logs to stderr)
  --find          Auto-detect USB serial port by VID:PID
  --port PORT     USB serial port (default: /dev/ttyACM0)
  --list          List available phases and test names
  --help, -h      Show this help

Examples:
  ./run_hil.sh                  # run everything
  ./run_hil.sh --skip usb      # skip USB tests (no board USB needed)
  ./run_hil.sh --phase hil     # only embedded-test HIL
  ./run_hil.sh --phase usb --find  # only USB, auto-detect port

Requirements:
  probe-rs, st-flash, arm-none-eabi-objcopy, pyserial
EOF
}

do_list() {
    echo "HIL Test Phases:"
    echo ""
    echo "Phase 1: embedded-test HIL (26 tests)"
    echo "  sdram_write_read_pattern, sdram_checkerboard, sdram_march_c,"
    echo "  sdram_end_of_ram, sdram_byte_halfword, sdram_misaligned,"
    echo "  display_init, display_color_fill,"
    echo "  touch_vendor_id, touch_chip_model,"
    echo "  led_toggle,"
    echo "  gpio_pa0_input, gpio_multi_port_output,"
    echo "  timer_1ms, timer_100ms_accuracy, timer_ticker,"
    echo "  rng_not_zero, rng_uniqueness, rng_consecutive_differ,"
    echo "  adc_temp_sensor, adc_vrefint,"
    echo "  uart_init, uart_tx_byte, uart_tx_multi_byte,"
    echo "  dma_64b, dma_4096b, dma_repeated"
    echo ""
    echo "Phase 2: RTT Example Tests (10 suites)"
    echo "  test_led (16), test_gpio (5), test_async_timer (10),"
    echo "  test_rng (3), test_adc (2), test_sdram (14),"
    echo "  test_uart (4), test_dma (5), test_display (14), test_touch (5)"
    echo ""
    echo "Phase 3: USB CDC Tests (2 suites)"
    echo "  usb_cdc_test (3 tests, st-flash + serial)"
    echo "  usb_cdc_stress (600 echo packets, st-flash + serial)"
}

find_usb_port() {
    if [ -n "$USB_PORT" ]; then
        echo "$USB_PORT"
        return
    fi
    if python3 -c "import serial.tools.list_ports" 2>/dev/null; then
        local found
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

# ── Prerequisites check ──────────────────────────────────────────

check_prereqs() {
    local missing=""

    if [ "$RUN_HIL" = true ] || [ "$RUN_RTT" = true ]; then
        if ! command -v probe-rs &>/dev/null; then
            missing="$missing probe-rs"
        fi
    fi

    if [ "$RUN_USB" = true ]; then
        if ! command -v st-flash &>/dev/null; then
            missing="$missing st-flash"
        fi
        if ! command -v arm-none-eabi-objcopy &>/dev/null; then
            missing="$missing arm-none-eabi-objcopy"
        fi
        if ! python3 -c "import serial" 2>/dev/null; then
            missing="$missing pyserial"
        fi
    fi

    if [ -n "$missing" ]; then
        fail "Missing prerequisites:$missing" >&2
        echo "  probe-rs:       cargo install probe-rs-tools" >&2
        echo "  st-flash:       apt install stlink-tools" >&2
        echo "  arm-none-eabi:  apt install gcc-arm-none-eabi" >&2
        echo "  pyserial:       pip install pyserial" >&2
        exit 2
    fi

    # Check probe connection for phases that need it
    if [ "$RUN_HIL" = true ] || [ "$RUN_RTT" = true ]; then
        if ! probe-rs list 2>/dev/null | grep -qi "stm32\|probe\|st-link"; then
            fail "No debug probe detected. Connect ST-LINK and retry." >&2
            exit 2
        fi
    fi
}

# ── Phase 1: embedded-test HIL ───────────────────────────────────

run_phase_hil() {
    log "Phase 1: embedded-test HIL (cargo test)"
    local log_file="${LOG_DIR}/phase1_hil.log"
    local phase_passed=0
    local phase_failed=0

    if ! cargo test --target "$TARGET" --test on_target > "$log_file" 2>&1; then
        # Parse failures from cargo test output
        while IFS= read -r line; do
            if echo "$line" | grep -qP '^test \S+ \.\.\. FAILED'; then
                local tname
                tname=$(echo "$line" | grep -oP '^test \K\S+')
                echo -e "  ${RED}[FAIL]${NC} $tname" >&2
                phase_failed=$((phase_failed + 1))
                JSON_HIL_TESTS=$(echo "$JSON_HIL_TESTS" | python3 -c "
import json,sys
tests=json.load(sys.stdin)
tests.append({'name':'$tname','status':'FAIL'})
print(json.dumps(tests))" 2>/dev/null || echo "$JSON_HIL_TESTS")
            fi
        done < "$log_file"

        # Count passes
        while IFS= read -r line; do
            if echo "$line" | grep -qP '^test \S+ \.\.\. ok'; then
                local tname
                tname=$(echo "$line" | grep -oP '^test \K\S+')
                echo -e "  ${GREEN}[PASS]${NC} $tname" >&2
                phase_passed=$((phase_passed + 1))
                JSON_HIL_TESTS=$(echo "$JSON_HIL_TESTS" | python3 -c "
import json,sys
tests=json.load(sys.stdin)
tests.append({'name':'$tname','status':'PASS'})
print(json.dumps(tests))" 2>/dev/null || echo "$JSON_HIL_TESTS")
            fi
        done < "$log_file"

        # Check for summary line
        local summary
        summary=$(grep -oP 'test result: \K.*' "$log_file" 2>/dev/null | head -1 || true)
        if [ -n "$summary" ]; then
            log "Cargo test summary: $summary" >&2
        fi
    else
        # All passed — parse from log
        while IFS= read -r line; do
            if echo "$line" | grep -qP '^test \S+ \.\.\. ok'; then
                local tname
                tname=$(echo "$line" | grep -oP '^test \K\S+')
                echo -e "  ${GREEN}[PASS]${NC} $tname" >&2
                phase_passed=$((phase_passed + 1))
                JSON_HIL_TESTS=$(echo "$JSON_HIL_TESTS" | python3 -c "
import json,sys
tests=json.load(sys.stdin)
tests.append({'name':'$tname','status':'PASS'})
print(json.dumps(tests))" 2>/dev/null || echo "$JSON_HIL_TESTS")
            fi
        done < "$log_file"
    fi

    JSON_HIL_PASSED=$phase_passed
    JSON_HIL_FAILED=$phase_failed

    TOTAL_PASSED=$((TOTAL_PASSED + phase_passed))
    TOTAL_FAILED=$((TOTAL_FAILED + phase_failed))

    if [ $phase_failed -eq 0 ]; then
        ok "Phase 1: ${phase_passed} passed" >&2
        PHASE_RESULTS+=("PASS:hil")
    else
        fail "Phase 1: ${phase_passed} passed, ${phase_failed} failed" >&2
        PHASE_RESULTS+=("FAIL:hil")
    fi
    echo "" >&2
}

# ── Phase 2: RTT Example Tests ───────────────────────────────────

declare -A RTT_TIMEOUTS
RTT_TIMEOUTS[test_led]=30
RTT_TIMEOUTS[test_gpio]=30
RTT_TIMEOUTS[test_async_timer]=30
RTT_TIMEOUTS[test_rng]=30
RTT_TIMEOUTS[test_adc]=30
RTT_TIMEOUTS[test_sdram]=60
RTT_TIMEOUTS[test_uart]=30
RTT_TIMEOUTS[test_dma]=30
RTT_TIMEOUTS[test_display]=120
RTT_TIMEOUTS[test_touch]=30

RTT_TESTS=(test_led test_gpio test_async_timer test_rng test_adc test_sdram test_uart test_dma test_display test_touch)

run_rtt_test() {
    local example=$1
    local timeout=${RTT_TIMEOUTS[$example]:-60}
    local log_file="${LOG_DIR}/phase2_${example}.log"
    local build_log="${LOG_DIR}/phase2_${example}_build.log"
    local elf_path="target/${TARGET}/release/examples/${example}"

    # Build
    if ! cargo build --release --example "$example" --target "$TARGET" > "$build_log" 2>&1; then
        fail "$example: BUILD FAILED" >&2
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
        JSON_RTT_TESTS=$(echo "$JSON_RTT_TESTS" | python3 -c "
import json,sys
tests=json.load(sys.stdin)
tests.append({'name':'$example','status':'BUILD_FAILED'})
print(json.dumps(tests))" 2>/dev/null || echo "$JSON_RTT_TESTS")
        PHASE_RESULTS+=("FAIL:$example")
        return 1
    fi

    # Flash + capture RTT
    timeout "$timeout" probe-rs run \
        --chip "$CHIP" \
        --protocol Swd \
        "$elf_path" \
        > "$log_file" 2>&1 &
    local pid=$!

    local waited=0
    while kill -0 "$pid" 2>/dev/null; do
        sleep 1
        waited=$((waited + 1))
        if [ "$waited" -ge "$timeout" ]; then
            break
        fi
        if grep -q "SUMMARY:\|ALL TESTS PASSED\|FAILED:" "$log_file" 2>/dev/null; then
            sleep 1
            kill "$pid" 2>/dev/null
            wait "$pid" 2>/dev/null
            break
        fi
    done
    kill "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null || true

    # Check probe errors
    if grep -qi "interface is busy\|Failed to open probe\|no probe found" "$log_file" 2>/dev/null; then
        fail "$example: PROBE ERROR (busy/missing)" >&2
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
        JSON_RTT_TESTS=$(echo "$JSON_RTT_TESTS" | python3 -c "
import json,sys
tests=json.load(sys.stdin)
tests.append({'name':'$example','status':'PROBE_ERROR'})
print(json.dumps(tests))" 2>/dev/null || echo "$JSON_RTT_TESTS")
        PHASE_RESULTS+=("FAIL:$example")
        return 1
    fi

    # Parse results
    local passed=0
    local failed=0
    local total=0

    while IFS= read -r line; do
        if echo "$line" | grep -qP 'TEST\s+\S+:\s+PASS'; then
            passed=$((passed + 1))
        elif echo "$line" | grep -qP 'TEST\s+\S+:\s+FAIL'; then
            failed=$((failed + 1))
        fi
    done < "$log_file"
    total=$((passed + failed))

    # Prefer SUMMARY line if present
    local summary_match
    summary_match=$(grep -oP 'SUMMARY:\s+\K(\d+)/(\d+)' "$log_file" 2>/dev/null | head -1 || true)
    if [ -n "$summary_match" ]; then
        passed=$(echo "$summary_match" | cut -d'/' -f1)
        total=$(echo "$summary_match" | cut -d'/' -f2)
        failed=$((total - passed))
    fi

    if grep -q "ALL TESTS PASSED" "$log_file" 2>/dev/null; then
        failed=0
    fi

    # Check for crash
    if grep -qi "HardFault\|panicked\|panic\|exception" "$log_file" 2>/dev/null; then
        if [ $failed -eq 0 ]; then
            failed=1
        fi
    fi

    JSON_RTT_TESTS=$(echo "$JSON_RTT_TESTS" | python3 -c "
import json,sys
tests=json.load(sys.stdin)
tests.append({'name':'$example','status':'PASS' if $failed == 0 else 'FAIL','passed':$passed,'failed':$failed,'total':$total})
print(json.dumps(tests))" 2>/dev/null || echo "$JSON_RTT_TESTS")

    if [ $failed -eq 0 ]; then
        echo -e "  ${GREEN}[PASS]${NC} $example ($passed/$total)" >&2
        TOTAL_PASSED=$((TOTAL_PASSED + 1))
        PHASE_RESULTS+=("PASS:$example")
    else
        echo -e "  ${RED}[FAIL]${NC} $example ($passed/$total passed)" >&2
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
        PHASE_RESULTS+=("FAIL:$example")
    fi
}

run_phase_rtt() {
    log "Phase 2: RTT Example Tests (${#RTT_TESTS[@]} suites)"
    local phase_passed=0
    local phase_failed=0

    for test in "${RTT_TESTS[@]}"; do
        run_rtt_test "$test" || true
        sleep 3  # Release USB interface
    done

    # Count from PHASE_RESULTS
    for r in "${PHASE_RESULTS[@]}"; do
        local status="${r%%:*}"
        local name="${r#*:}"
        case "$name" in
            test_*)
                if [ "$status" = "PASS" ]; then
                    phase_passed=$((phase_passed + 1))
                else
                    phase_failed=$((phase_failed + 1))
                fi
                ;;
        esac
    done

    JSON_RTT_PASSED=$phase_passed
    JSON_RTT_FAILED=$phase_failed

    if [ $phase_failed -eq 0 ]; then
        ok "Phase 2: ${phase_passed}/${#RTT_TESTS[@]} suites passed" >&2
    else
        fail "Phase 2: ${phase_passed} passed, ${phase_failed} failed (of ${#RTT_TESTS[@]})" >&2
    fi
    echo "" >&2
}

# ── Phase 3: USB CDC Tests ───────────────────────────────────────

run_usb_test() {
    local example=$1
    local python_script=$2
    local py_args=$3
    local log_file="${LOG_DIR}/phase3_${example}.log"
    local build_log="${LOG_DIR}/phase3_${example}_build.log"
    local elf_path="target/${TARGET}/release/examples/${example}"
    local bin_path
    bin_path=$(mktemp /tmp/hil_usb_XXXXXX.bin)

    # Build
    if ! cargo build --release --example "$example" --target "$TARGET" > "$build_log" 2>&1; then
        fail "$example: BUILD FAILED" >&2
        rm -f "$bin_path"
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
        JSON_USB_TESTS=$(echo "$JSON_USB_TESTS" | python3 -c "
import json,sys
tests=json.load(sys.stdin)
tests.append({'name':'$example','status':'BUILD_FAILED'})
print(json.dumps(tests))" 2>/dev/null || echo "$JSON_USB_TESTS")
        PHASE_RESULTS+=("FAIL:$example")
        return 1
    fi

    # Convert to binary
    arm-none-eabi-objcopy -O binary "$elf_path" "$bin_path" 2>&1 | tee -a "$log_file" >&2

    # Flash
    if ! st-flash --connect-under-reset write "$bin_path" 0x08000000 >> "$log_file" 2>&1; then
        fail "$example: FLASH FAILED" >&2
        rm -f "$bin_path"
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
        JSON_USB_TESTS=$(echo "$JSON_USB_TESTS" | python3 -c "
import json,sys
tests=json.load(sys.stdin)
tests.append({'name':'$example','status':'FLASH_FAILED'})
print(json.dumps(tests))" 2>/dev/null || echo "$JSON_USB_TESTS")
        PHASE_RESULTS+=("FAIL:$example")
        return 1
    fi
    rm -f "$bin_path"

    # Reset + wait for USB enumeration
    st-flash --connect-under-reset reset >> "$log_file" 2>&1 || true
    warn "Waiting 15s for USB enumeration..." >&2
    sleep 15

    # Find port
    local test_port
    if [ -n "$USB_PORT" ]; then
        test_port="$USB_PORT"
    elif [ "$FIND_PORT" = true ]; then
        test_port=$(find_usb_port)
    else
        test_port="${USB_PORT:-$(find_usb_port)}"
    fi

    if [ -z "$test_port" ]; then
        fail "$example: No USB CDC port found" >&2
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
        JSON_USB_TESTS=$(echo "$JSON_USB_TESTS" | python3 -c "
import json,sys
tests=json.load(sys.stdin)
tests.append({'name':'$example','status':'NO_PORT'})
print(json.dumps(tests))" 2>/dev/null || echo "$JSON_USB_TESTS")
        PHASE_RESULTS+=("FAIL:$example")
        return 1
    fi

    # Run host-side test
    log "Running $python_script on $test_port..." >&2
    local full_py_args="--port $test_port $py_args"
    if python3 "$python_script" $full_py_args >> "$log_file" 2>&1; then
        echo -e "  ${GREEN}[PASS]${NC} $example" >&2
        TOTAL_PASSED=$((TOTAL_PASSED + 1))
        JSON_USB_TESTS=$(echo "$JSON_USB_TESTS" | python3 -c "
import json,sys
tests=json.load(sys.stdin)
tests.append({'name':'$example','status':'PASS'})
print(json.dumps(tests))" 2>/dev/null || echo "$JSON_USB_TESTS")
        PHASE_RESULTS+=("PASS:$example")
    else
        echo -e "  ${RED}[FAIL]${NC} $example" >&2
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
        JSON_USB_TESTS=$(echo "$JSON_USB_TESTS" | python3 -c "
import json,sys
tests=json.load(sys.stdin)
tests.append({'name':'$example','status':'FAIL'})
print(json.dumps(tests))" 2>/dev/null || echo "$JSON_USB_TESTS")
        PHASE_RESULTS+=("FAIL:$example")
    fi
}

run_phase_usb() {
    log "Phase 3: USB CDC Tests"

    # 3a: USB CDC connectivity test
    run_usb_test "test_usb_cdc" "tests/usb_cdc_test.py" "--timeout 30" || true

    # 3b: USB CDC stress test
    run_usb_test "test_usb_cdc_stress" "tests/usb_cdc_stress.py" "--count 600" || true

    local phase_passed=0
    local phase_failed=0
    for r in "${PHASE_RESULTS[@]}"; do
        local status="${r%%:*}"
        local name="${r#*:}"
        case "$name" in
            test_usb_cdc|test_usb_cdc_stress)
                if [ "$status" = "PASS" ]; then
                    phase_passed=$((phase_passed + 1))
                else
                    phase_failed=$((phase_failed + 1))
                fi
                ;;
        esac
    done

    JSON_USB_PASSED=$phase_passed
    JSON_USB_FAILED=$phase_failed

    if [ $phase_failed -eq 0 ]; then
        ok "Phase 3: ${phase_passed}/2 passed" >&2
    else
        fail "Phase 3: ${phase_passed} passed, ${phase_failed} failed" >&2
    fi
    echo "" >&2
}

# ── Final report ──────────────────────────────────────────────────

print_report() {
    {
        echo ""
        echo "=========================================="
        echo "  HIL Test Results"
        echo "  $(date)"
        echo "  Commit: $(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')"
        echo "=========================================="
        echo ""

        for r in "${PHASE_RESULTS[@]}"; do
            local status="${r%%:*}"
            local name="${r#*:}"
            if [ "$status" = "PASS" ]; then
                echo -e "  ${GREEN}[PASS]${NC} $name"
            else
                echo -e "  ${RED}[FAIL]${NC} $name"
            fi
        done

        echo ""
        echo "  Total passed: $TOTAL_PASSED"
        echo "  Total failed: $TOTAL_FAILED"
        echo ""

        if [ $TOTAL_FAILED -eq 0 ]; then
            echo -e "  ${GREEN}ALL TESTS PASSED${NC}"
        else
            echo -e "  ${RED}${TOTAL_FAILED} TEST(S) FAILED${NC}"
        fi
        echo ""
        echo "  Report: $REPORT_FILE"
        echo "  Logs:   $LOG_DIR/"
        echo "=========================================="
    } | tee /dev/stderr >&2
}

write_report_file() {
    {
        echo "HIL Test Report"
        echo "Date: $(date)"
        echo "Commit: $(git rev-parse HEAD 2>/dev/null || echo 'unknown')"
        echo ""
        for r in "${PHASE_RESULTS[@]}"; do
            local status="${r%%:*}"
            local name="${r#*:}"
            echo "  [$status] $name"
        done
        echo ""
        echo "Total passed: $TOTAL_PASSED"
        echo "Total failed: $TOTAL_FAILED"
        if [ $TOTAL_FAILED -eq 0 ]; then
            echo "ALL TESTS PASSED"
        else
            echo "${TOTAL_FAILED} TEST(S) FAILED"
        fi
    } > "$REPORT_FILE"
}

print_json() {
    local commit
    commit=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
    local ts
    ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    python3 -c "
import json, sys
result = {
    'timestamp': '$ts',
    'git_commit': '$commit',
    'phases': {
        'hil': {'passed': $JSON_HIL_PASSED, 'failed': $JSON_HIL_FAILED, 'tests': $JSON_HIL_TESTS},
        'rtt': {'passed': $JSON_RTT_PASSED, 'failed': $JSON_RTT_FAILED, 'tests': $JSON_RTT_TESTS},
        'usb': {'passed': $JSON_USB_PASSED, 'failed': $JSON_USB_FAILED, 'tests': $JSON_USB_TESTS},
    },
    'total_passed': $TOTAL_PASSED,
    'total_failed': $TOTAL_FAILED,
    'success': $TOTAL_FAILED == 0,
}
print(json.dumps(result, indent=2))
"
}

# ── Argument parsing ──────────────────────────────────────────────

while [ $# -gt 0 ]; do
    case "$1" in
        --phase)
            RUN_HIL=false; RUN_RTT=false; RUN_USB=false
            case "${2:-}" in
                hil) RUN_HIL=true ;;
                rtt) RUN_RTT=true ;;
                usb) RUN_USB=true ;;
                *) echo "Unknown phase: ${2:-} (hil|rtt|usb)"; exit 1 ;;
            esac
            shift 2
            ;;
        --skip)
            case "${2:-}" in
                hil) RUN_HIL=false ;;
                rtt) RUN_RTT=false ;;
                usb) RUN_USB=false ;;
                *) echo "Unknown phase: ${2:-} (hil|rtt|usb)"; exit 1 ;;
            esac
            shift 2
            ;;
        --json)    JSON_OUTPUT=true; shift ;;
        --find)    FIND_PORT=true; shift ;;
        --port)    shift; USB_PORT="${1:-}"; shift ;;
        --list)    do_list; exit 0 ;;
        --help|-h) do_help; exit 0 ;;
        *) echo "Unknown option: $1"; do_help; exit 1 ;;
    esac
done

# ── Main ──────────────────────────────────────────────────────────

mkdir -p "$LOG_DIR"

check_prereqs

echo "=========================================="  >&2
echo "  HIL Test Runner — STM32F469I-DISCO"       >&2
echo "  $(date)"                                    >&2
echo "=========================================="  >&2
echo "" >&2

if [ "$RUN_HIL" = true ]; then
    run_phase_hil
fi

if [ "$RUN_RTT" = true ]; then
    run_phase_rtt
fi

if [ "$RUN_USB" = true ]; then
    run_phase_usb
fi

write_report_file
print_report

if [ "$JSON_OUTPUT" = true ]; then
    print_json
fi

if [ $TOTAL_FAILED -gt 0 ]; then
    exit 1
fi
exit 0
