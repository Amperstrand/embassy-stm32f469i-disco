#!/usr/bin/env python3
"""USB CDC stress test for STM32F469I-DISCO BSP.

Sends hundreds of echo packets over USB CDC and measures response times,
detecting timeouts, corrupted responses, and USB disconnects.

Usage:
    python3 tests/usb_cdc_stress.py [--port /dev/ttyACM0] [--count 600] [--timeout 5] [--payload-size 8]

Requirements:
    pip install pyserial

Deploy firmware (DO NOT use probe-rs — it breaks USB enumeration):
    cargo build --release --example test_usb_cdc_stress
    arm-none-eabi-objcopy -O binary target/thumbv7em-none-eabihf/release/examples/test_usb_cdc_stress stress.bin
    st-flash --connect-under-reset write stress.bin 0x08000000
    st-flash --connect-under-reset reset
    sleep 15  # wait for USB enumeration
    python3 tests/usb_cdc_stress.py
"""

import argparse
import json
import os
import struct
import sys
import time
from datetime import datetime, timezone

try:
    import serial
except ImportError:
    print("ERROR: pyserial required. Install with: pip install pyserial")
    sys.exit(2)

DEFAULT_PORT = "/dev/ttyACM0"
DEFAULT_COUNT = 600
DEFAULT_TIMEOUT = 5.0
DEFAULT_PAYLOAD_SIZE = 8
POST_OPEN_DELAY = 0.5

VID_PID = "16c0:27dd"
SERIAL_MATCH = "STRESS"


def find_port():
    """Try to find the stress test USB CDC port by VID:PID and serial number."""
    try:
        import serial.tools.list_ports
    except ImportError:
        return None

    ports = serial.tools.list_ports.comports()
    for p in ports:
        vid = f"{p.vid:04x}" if p.vid else ""
        pid = f"{p.pid:04x}" if p.pid else ""
        if vid + ":" + pid == VID_PID:
            if SERIAL_MATCH and SERIAL_MATCH in (p.serial_number or ""):
                return p.device
    for p in ports:
        vid = f"{p.vid:04x}" if p.vid else ""
        pid = f"{p.pid:04x}" if p.pid else ""
        if vid + ":" + pid == VID_PID:
            return p.device
    return None


def run_stress(port, count, timeout, payload_size):
    print(f"=== USB CDC Stress Test ===")
    print(f"Port: {port}")
    print(f"Echo count: {count}")
    print(f"Payload size: {payload_size} bytes")
    print(f"Timeout: {timeout}s")
    print(f"Time: {datetime.now(timezone.utc).isoformat()}")
    print()

    results = {
        "port": port,
        "vid_pid": VID_PID,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "payload_size": payload_size,
        "total_sent": 0,
        "successes": 0,
        "failures": 0,
        "timeouts": 0,
        "corrupted": 0,
        "disconnects": 0,
        "timings_ms": [],
        "errors": [],
    }

    ser = serial.Serial(
        port=port,
        baudrate=115200,
        timeout=timeout,
        dsrdtr=False,
        rtscts=False,
    )
    time.sleep(POST_OPEN_DELAY)

    payload = bytes(range(payload_size % 256)) * (payload_size // 256 + 1)
    payload = payload[:payload_size]

    phases = [
        ("Phase 1: small packets", payload[:4], count // 4),
        ("Phase 2: medium packets", payload[:payload_size], count // 4),
        ("Phase 3: max CDC packet", b"\xAA" * 64, count // 4),
        ("Phase 4: alternating", payload[:payload_size], count - 3 * (count // 4)),
    ]

    total_start = time.monotonic()
    sent = 0

    for phase_name, phase_payload, phase_count in phases:
        phase_start = time.monotonic()
        phase_ok = 0
        phase_fail = 0

        for i in range(phase_count):
            if sent >= count:
                break

            start = time.monotonic()
            try:
                ser.write(phase_payload)
                ser.flush()
            except OSError as e:
                results["disconnects"] += 1
                results["failures"] += 1
                results["errors"].append(f"{phase_name} #{i}: write failed: {e}")
                phase_fail += 1
                sent += 1
                results["timings_ms"].append(timeout * 1000)
                break

            try:
                response = ser.read(len(phase_payload))
            except OSError as e:
                results["disconnects"] += 1
                results["failures"] += 1
                results["errors"].append(f"{phase_name} #{i}: read failed: {e}")
                phase_fail += 1
                sent += 1
                results["timings_ms"].append(timeout * 1000)
                break

            elapsed = time.monotonic() - start
            results["timings_ms"].append(round(elapsed * 1000, 3))
            results["total_sent"] += 1
            sent += 1

            if len(response) == 0:
                results["timeouts"] += 1
                results["failures"] += 1
                results["errors"].append(f"{phase_name} #{i}: timeout (0 bytes)")
                phase_fail += 1
            elif len(response) != len(phase_payload):
                results["corrupted"] += 1
                results["failures"] += 1
                results["errors"].append(
                    f"{phase_name} #{i}: length mismatch "
                    f"(sent={len(phase_payload)}, recv={len(response)})"
                )
                phase_fail += 1
            elif response != phase_payload:
                results["corrupted"] += 1
                results["failures"] += 1
                results["errors"].append(
                    f"{phase_name} #{i}: data mismatch "
                    f"(sent={phase_payload.hex()}, recv={response.hex()})"
                )
                phase_fail += 1
            else:
                results["successes"] += 1
                phase_ok += 1

        phase_elapsed = time.monotonic() - phase_start
        print(f"  {phase_name}: {phase_ok}/{phase_ok + phase_fail} OK ({phase_elapsed:.1f}s)")

    total_elapsed = time.monotonic() - total_start
    timings = sorted(results["timings_ms"])

    print()
    print(f"=== RESULTS ===")
    print(f"Total sent:     {results['total_sent']}")
    print(f"Successes:      {results['successes']}")
    print(f"Failures:       {results['failures']}")
    print(f"  Timeouts:     {results['timeouts']}")
    print(f"  Corrupted:    {results['corrupted']}")
    print(f"  Disconnects:  {results['disconnects']}")
    print(f"Total time:     {total_elapsed:.1f}s")

    if results["total_sent"] > 0:
        print(f"Commands/sec:   {results['total_sent']/total_elapsed:.1f}")
        if timings:
            print(f"Timing (median): {timings[len(timings)//2]:.2f}ms")
            print(f"Timing (p95):    {timings[int(len(timings)*0.95)]:.2f}ms")
            print(f"Timing (p99):    {timings[int(len(timings)*0.99)]:.2f}ms")
            print(f"Timing (max):    {timings[-1]:.2f}ms")

    if results["errors"]:
        print(f"\nFirst 10 errors:")
        for e in results["errors"][:10]:
            print(f"  - {e}")

    ser.close()

    results["summary"] = {
        "total_time_s": round(total_elapsed, 1),
        "cmds_per_sec": round(results["total_sent"] / total_elapsed, 1) if total_elapsed > 0 else 0,
        "passed": results["failures"] == 0,
    }
    if timings:
        results["summary"]["median_ms"] = timings[len(timings) // 2]
        results["summary"]["p95_ms"] = timings[int(len(timings) * 0.95)]
        results["summary"]["p99_ms"] = timings[int(len(timings) * 0.99)]
        results["summary"]["max_ms"] = timings[-1]

    script_dir = os.path.dirname(os.path.abspath(__file__))
    results_dir = os.path.join(script_dir, "results")
    os.makedirs(results_dir, exist_ok=True)
    outfile = os.path.join(
        results_dir,
        f"stress_{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}.json",
    )
    with open(outfile, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to: {outfile}")

    return results["failures"] == 0


def main():
    parser = argparse.ArgumentParser(description="USB CDC stress test for STM32F469I-DISCO BSP")
    parser.add_argument("--port", default=DEFAULT_PORT, help=f"Serial port (default: {DEFAULT_PORT})")
    parser.add_argument("--count", type=int, default=DEFAULT_COUNT, help=f"Number of echo packets (default: {DEFAULT_COUNT})")
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT, help=f"Read timeout in seconds (default: {DEFAULT_TIMEOUT})")
    parser.add_argument("--payload-size", type=int, default=DEFAULT_PAYLOAD_SIZE, help=f"Payload size in bytes (default: {DEFAULT_PAYLOAD_SIZE})")
    parser.add_argument("--find", action="store_true", help="Auto-detect port by VID:PID")
    args = parser.parse_args()

    port = args.port
    if args.find:
        found = find_port()
        if found:
            print(f"Found stress test device at: {found}")
            port = found
        else:
            print(f"ERROR: No device with VID:PID {VID_PID} found")
            sys.exit(2)

    ok = run_stress(port, args.count, args.timeout, args.payload_size)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
