#!/usr/bin/env python3
"""Read hardware test results from CCMRAM on STM32F469I-Discovery.

The extensive_hw_test firmware writes results to CCMRAM at 0x10000000.
This script reads them via probe-rs.

Usage:
    python3 tests/read_test_results.py
    python3 tests/read_test_results.py --chip STM32F469NIHx
"""

import argparse
import struct
import subprocess
import sys

CCMRAM_BASE = 0x10000000
RESULT_MAGIC = 0x5245534C  # "RESL"
TEST_NAME_LEN = 48
MAX_TESTS = 64

# struct TestResultBuffer {
#     magic: u32,
#     count: u32,
#     pass_count: u32,
#     fail_count: u32,
#     entries: [TestResultEntry; MAX_TESTS],  // each: [u8; 48] + u8 + [u8; 3]
#     done: u32,
# }
ENTRY_SIZE = TEST_NAME_LEN + 4  # name + passed(u8) + pad(3)
HEADER_SIZE = 4 * 4  # magic, count, pass_count, fail_count
BUFFER_SIZE = HEADER_SIZE + (ENTRY_SIZE * MAX_TESTS) + 4  # + done


def read_memory(chip: str, address: int, size: int) -> bytes:
    """Read `size` bytes from target memory via probe-rs."""
    cmd = [
        "probe-rs", "read",
        "--chip", chip,
        "--address", hex(address),
        "--count", str(size),
        "--format", "bin",
    ]
    result = subprocess.run(cmd, capture_output=True)
    if result.returncode != 0:
        print(f"probe-rs error: {result.stderr.decode()}", file=sys.stderr)
        sys.exit(1)
    return result.stdout


def parse_results(data: bytes):
    """Parse the TestResultBuffer from raw bytes."""
    magic, count, pass_count, fail_count = struct.unpack_from("<IIII", data, 0)

    if magic != RESULT_MAGIC:
        print(f"No valid results found (magic=0x{magic:08X}, expected 0x{RESULT_MAGIC:08X})")
        print("Either the test hasn't run yet, or CCMRAM was cleared.")
        sys.exit(1)

    done_offset = HEADER_SIZE + (ENTRY_SIZE * MAX_TESTS)
    done = struct.unpack_from("<I", data, done_offset)[0]
    test_complete = done == RESULT_MAGIC

    results = []
    for i in range(min(count, MAX_TESTS)):
        offset = HEADER_SIZE + (i * ENTRY_SIZE)
        name_bytes = data[offset:offset + TEST_NAME_LEN]
        name = name_bytes.split(b'\x00')[0].decode('ascii', errors='replace')
        passed = data[offset + TEST_NAME_LEN] != 0
        results.append((name, passed))

    return count, pass_count, fail_count, results, test_complete


def main():
    parser = argparse.ArgumentParser(description="Read HW test results from CCMRAM")
    parser.add_argument("--chip", default="STM32F469NIHx", help="probe-rs chip name")
    args = parser.parse_args()

    print(f"Reading {BUFFER_SIZE} bytes from CCMRAM at 0x{CCMRAM_BASE:08X}...")
    data = read_memory(args.chip, CCMRAM_BASE, BUFFER_SIZE)

    count, pass_count, fail_count, results, complete = parse_results(data)

    print(f"\n{'='*60}")
    print(f"  Hardware Test Results  ({'COMPLETE' if complete else 'IN PROGRESS'})")
    print(f"{'='*60}\n")

    for name, passed in results:
        status = "PASS" if passed else "FAIL"
        print(f"  [{'✓' if passed else '✗'}] {name}: {status}")

    total = pass_count + fail_count
    print(f"\n{'─'*60}")
    print(f"  SUMMARY: {pass_count}/{total} passed")
    if fail_count == 0 and complete:
        print(f"  ALL TESTS PASSED")
    elif fail_count > 0:
        print(f"  {fail_count} TESTS FAILED")
    print(f"{'─'*60}\n")

    if not complete:
        print("  ⚠ Test run was not complete (done marker not set).")
        print("    The test may still be running or was interrupted.\n")

    sys.exit(0 if fail_count == 0 and complete else 1)


if __name__ == "__main__":
    main()
