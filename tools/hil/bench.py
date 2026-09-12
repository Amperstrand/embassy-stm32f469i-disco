"""Bench-wrapped HIL for the STM32F469I-DISCO BSP (Amperstrand bench pattern).

The BSP's own ./run_hil.sh runs the three on-target phases; this wrapper
adds the bench contract the shared F469 board demands (gm65/micronuts
tools/hil lineage — blessed copy):

    BenchLock FIRST -> labgrid place (the stlink BenchSerialToken; acquiring
    it excludes micronuts-qr-rig / gm65-qr-loopback) -> 2 MiB image backup
    -> ./run_hil.sh <args> -> image restore -> release.

The F469 is a shared board (microfips f469-mcu; micronuts/gm65 flash it
too): never skip the backup/restore. Safety gate: fips-lab boards.toml
(key "stm32f469i-disco" must permit "flash").

Usage:
    python3 tools/hil/bench.py -- --list        # passthrough info args only
    python3 tools/hil/bench.py -- --skip usb    # phases 1-2 (no USB serial)
    python3 tools/hil/bench.py -- --phase hil   # embedded-test phase only
    python3 tools/hil/bench.py --keep -- --phase rtt   # iterate, no restore
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time
import tomllib
from pathlib import Path

from tollgate_lab import acquire_bench_lock, ensure_run_headroom

REPO_ROOT = Path(__file__).resolve().parents[2]
BOARDS_TOML = Path(os.environ.get(
    "BSP_BOARDS_TOML",
    REPO_ROOT.parent / "fips-lab" / "fips_lab" / "boards.toml",
))

COORDINATOR = os.environ.get("LABGRID_COORDINATOR", "192.168.13.221:20408")
EXPORTER_NAME = os.environ.get("LABGRID_EXPORTER_NAME", "ai-legion-small-microfips")
PLACE = "bsp-f469-hil"
BACKUP_DIR = Path(__file__).parent / "results"

STM32_REGISTRY_KEY = "stm32f469i-disco"  # gitleaks:allow board alias, not a credential
STM32_FLASH_BASE = 0x08000000
STM32_FLASH_SIZE = 0x200000  # 2 MiB (STM32F469NI)

# Args that only print information — no hardware is touched, so the
# backup/restore dance is skipped for them.
INFO_ONLY_ARGS = {"--list", "--help", "-h"}


class BenchError(RuntimeError):
    pass


def require_board(key: str, op: str) -> None:
    with open(BOARDS_TOML, "rb") as f:
        data = tomllib.load(f)
    spec = data.get("boards", {}).get(key)
    if spec is None:
        raise BenchError(f"board {key!r} is not in {BOARDS_TOML} — refusing {op}")
    if op not in spec.get("ops", []):
        raise BenchError(
            f"board {key!r} does not permit {op!r} (allowed: {spec.get('ops')})"
        )


def lg(args: list[str], timeout_s: int = 15) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["labgrid-client", "-x", COORDINATOR, "-p", PLACE, *args],
        capture_output=True, text=True, timeout=timeout_s,
    )


def note(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", flush=True)


def _st_flash(*args: str, timeout: int = 300) -> subprocess.CompletedProcess:
    result = subprocess.run(
        ["st-flash", "--connect-under-reset", *args],
        capture_output=True, text=True, timeout=timeout,
    )
    if result.returncode != 0:
        raise BenchError(f"st-flash {' '.join(args)} failed: {result.stderr[-500:]}")
    return result


def backup_stm32(dest: Path) -> Path:
    require_board(STM32_REGISTRY_KEY, "flash")
    _st_flash("read", str(dest), hex(STM32_FLASH_BASE), hex(STM32_FLASH_SIZE), timeout=600)
    return dest


def flash_stm32(bin_path: Path) -> None:
    require_board(STM32_REGISTRY_KEY, "flash")
    subprocess.run(["pkill", "-9", "st-flash"], capture_output=True)
    time.sleep(1)
    _st_flash("write", str(bin_path), hex(STM32_FLASH_BASE))
    # st-flash write alone leaves the target wedged on this board (USB dead
    # until an explicit reset — gm65 bench lesson 2026-09-08/09)
    _st_flash("reset", timeout=60)


def restore_stm32(backup: Path) -> None:
    flash_stm32(backup)


def wait_for_probe(timeout_s: float = 20.0) -> None:
    """st-flash and probe-rs race on this stlink: right after an st-flash
    op, probe-rs can briefly see no probe (bench-verified 2026-09-12).
    Block until probe-rs lists the ST-Link again."""
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        r = subprocess.run(["probe-rs", "list"], capture_output=True, text=True)
        if "0483:374b" in r.stdout:
            return
        time.sleep(2)
    raise BenchError(f"ST-Link not visible to probe-rs within {timeout_s}s")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--keep", action="store_true", help="skip image restore")
    ap.add_argument("run_hil_args", nargs=argparse.REMAINDER,
                    help="args after '--' pass through to ./run_hil.sh")
    args = ap.parse_args()

    hil_args = [a for a in args.run_hil_args if a != "--"]
    info_only = hil_args and set(hil_args) <= INFO_ONLY_ARGS

    BACKUP_DIR.mkdir(parents=True, exist_ok=True)
    require_board(STM32_REGISTRY_KEY, "flash")

    with acquire_bench_lock("amperstrand-bench"):
        hygiene = ensure_run_headroom()
        if hygiene.acted:
            note(f"disk hygiene: {hygiene.actions}")

        acquired = lg(["acquire"])
        note(f"place acquire rc={acquired.returncode}")

        backup = None
        try:
            if not info_only:
                backup = BACKUP_DIR / f"f469-backup-{time.strftime('%Y%m%d-%H%M%S')}.bin"
                note(f"backing up F469 -> {backup}")
                backup_stm32(backup)
                wait_for_probe()
                note("backup done, probe visible")

            note(f"run_hil.sh {' '.join(hil_args) or '(all phases)'}")
            rc = subprocess.call(["./run_hil.sh", *hil_args], cwd=REPO_ROOT)
            note(f"run_hil.sh rc={rc}")
            return rc
        finally:
            if backup is not None and not args.keep:
                note("restoring pre-session F469 image")
                restore_stm32(backup)
                note("restore done")
            lg(["release"])
            note("place released")


if __name__ == "__main__":
    sys.exit(main())
