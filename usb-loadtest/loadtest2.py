#!/usr/bin/env python3
"""usb-spike load test v2: full-duplex reader/writer threads."""
import serial, sys, time, threading

PORT = sys.argv[1] if len(sys.argv) > 1 else "/dev/ttyACM2"
ROUNDS = int(sys.argv[2]) if len(sys.argv) > 2 else 64
CHUNK = 512

TOTAL = ROUNDS * 65536

def pattern_byte(i):
    return (i * 7 + (i >> 16) * 31) & 0xFF

s = serial.Serial(PORT, timeout=0.05, write_timeout=5)
s.dtr = True
time.sleep(0.4)
s.reset_input_buffer()

received = bytearray()
recv_lock = threading.Lock()
stop = threading.Event()

def reader():
    while not stop.is_set():
        try:
            got = s.read(4096)
        except serial.SerialException:
            break
        if got:
            with recv_lock:
                received.extend(got)

rt = threading.Thread(target=reader, daemon=True)
rt.start()

expected = bytearray()
sent = 0
t0 = time.time()
last_recv = 0
last_recv_t = t0
try:
    for i in range(0, TOTAL, CHUNK):
        blk = bytes(pattern_byte(i + k) for k in range(CHUNK))
        expected.extend(blk)
        s.write(blk)
        sent += CHUNK
        # stall watchdog: if nothing received for 10s while sending is incomplete
        with recv_lock:
            cur = len(received)
        if cur > last_recv:
            last_recv = cur
            last_recv_t = time.time()
        elif time.time() - last_recv_t > 10:
            print(f"STALL: sent={sent} recv={cur} (no data for 10s)")
            sys.exit(2)
finally:
    stop.set()
    rt.join(timeout=1)

# wait for echo to finish
deadline = time.time() + 30
while time.time() < deadline:
    with recv_lock:
        if len(received) >= TOTAL:
            break
    time.sleep(0.1)
elapsed = time.time() - t0

def strip_status(buf):
    out = bytearray()
    lines = 0
    i = 0
    while i < len(buf):
        if buf[i:i + 6] == b"SPIKE ":
            j = buf.find(b"\n", i)
            if j == -1:
                break
            lines += 1
            i = j + 1
        else:
            out.append(buf[i])
            i += 1
    return bytes(out), lines

clean, lines = strip_status(bytes(received))
print(f"sent={sent} recv={len(received)} echo={len(clean)} status_lines={lines}")
print(f"elapsed={elapsed:.1f}s throughput={sent/elapsed/1024:.1f} KiB/s")
if clean == bytes(expected):
    print("RESULT: PASS")
    sys.exit(0)
n = min(len(clean), len(expected))
for k in range(n):
    if clean[k] != expected[k]:
        print(f"RESULT: FAIL mismatch at byte {k}: {clean[k]:02x} != {expected[k]:02x}")
        sys.exit(1)
print(f"RESULT: FAIL length {len(clean)} != {len(expected)}")
sys.exit(1)
