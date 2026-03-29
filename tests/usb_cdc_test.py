#!/usr/bin/env python3
"""USB CDC test monitor for STM32F469I-DISCO BSP.

Connects to the test_usb_cdc firmware, sends echo test data, and
parses PASS/FAIL/SUMMARY output lines.

Usage:
    python3 tests/usb_cdc_test.py [--port /dev/ttyACM0] [--timeout 30]

Requirements:
    pip install pyserial

Deploy firmware (DO NOT use probe-rs):
    cargo build --release --target thumbv7em-none-eabihf --example test_usb_cdc
    arm-none-eabi-objcopy -O binary target/thumbv7em-none-eabihf/release/examples/test_usb_cdc test.bin
    st-flash --connect-under-reset write test.bin 0x08000000
    st-flash --connect-under-reset reset
    sleep 15
    python3 tests/usb_cdc_test.py
"""

import argparse
import re
import sys
import time

try:
    import serial
except ImportError:
    print("ERROR: pyserial required. Install with: pip install pyserial")
    sys.exit(2)

DEFAULT_PORT = "/dev/ttyACM0"
DEFAULT_TIMEOUT = 30
ECHO_TEST_DATA = b"HELLO\r\n"
ECHO_DELAY = 1.0


def parse_args():
    parser = argparse.ArgumentParser(description="USB CDC test monitor")
    parser.add_argument("--port", default=DEFAULT_PORT, help="Serial port")
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT, help="Total timeout (seconds)")
    return parser.parse_args()


def main():
    args = parse_args()

    print(f"Connecting to {args.port}...")
    try:
        ser = serial.Serial(args.port, baudrate=115200, timeout=1.0)
    except serial.SerialException as e:
        print(f"ERROR: Cannot open {args.port}: {e}")
        sys.exit(1)

    print(f"Connected. Waiting for test output (timeout {args.timeout}s)...")

    start = time.time()
    output_lines = []
    echo_sent = False
    passed = 0
    failed = 0
    tests = []

    while time.time() - start < args.timeout:
        if ser.in_waiting > 0:
            raw = ser.read(ser.in_waiting)
            text = raw.decode("utf-8", errors="replace")
            output_lines.append(text)
            print(text, end="", flush=True)

            for line in text.split("\r\n"):
                line = line.strip()
                if not line:
                    continue

                m = re.match(r"TEST (\S+): (PASS|FAIL)(.*)", line)
                if m:
                    name = m.group(1)
                    status = m.group(2)
                    reason = m.group(3).strip()
                    if status == "PASS":
                        passed += 1
                        tests.append((name, "PASS", ""))
                    else:
                        failed += 1
                        tests.append((name, "FAIL", reason))

                m = re.match(r"SUMMARY: (\d+)/(\d+) passed", line)
                if m:
                    print(f"\n--- Firmware Summary: {m.group(1)}/{m.group(2)} passed ---")

                if "ALL TESTS PASSED" in line:
                    print("\n*** ALL TESTS PASSED ***")

            if not echo_sent and any("usb_cdc_echo: RUNNING" in o for o in output_lines):
                time.sleep(ECHO_DELAY)
                print(f"[host] Sending echo data: {ECHO_TEST_DATA!r}")
                ser.write(ECHO_TEST_DATA)
                echo_sent = True
        else:
            time.sleep(0.1)

    ser.close()

    print(f"\n{'='*50}")
    print(f"Results: {passed} passed, {failed} failed, {passed + failed} total")
    if passed + failed == 0:
        print("ERROR: No test output received. Check USB connection.")
        sys.exit(1)
    if failed > 0:
        for name, status, reason in tests:
            if status == "FAIL":
                print(f"  FAIL {name}: {reason}")
        sys.exit(1)
    else:
        print("All tests passed!")
        sys.exit(0)


if __name__ == "__main__":
    main()
