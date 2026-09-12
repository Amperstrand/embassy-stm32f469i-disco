# tools/hil — bench-wrapped HIL for the shared F469

The BSP's HIL tests (`./run_hil.sh`: embedded-test phase, RTT example
suites, USB CDC suites) run on the **bench F469**, which is a *shared*
board (microfips `f469-mcu`; micronuts/gm65 flash it too). This directory
adds the bench contract around them — same lineage as gm65-scanner's and
micronuts' `tools/hil` (blessed copy; consolidation target if ever needed:
tollgate-lab):

1. **BenchLock first** — `acquire_bench_lock("amperstrand-bench")`, the
   cross-project kernel flock (holder info in `/tmp/amperstrand-bench.lock`).
2. **labgrid place** — `bsp-f469-hil` binds the stlink `BenchSerialToken`
   from the microfips bench exporter; acquiring it *excludes*
   `micronuts-qr-rig`, `gm65-qr-loopback`, and `microfips-bench`.
3. **Image backup/restore** — 2 MiB `st-flash read` before anything,
   `st-flash write` + reset in `finally`. Never run with `--keep` on the
   shared board unless you are iterating AND the next session knows.
4. **Flash gate** — `fips-lab/fips_lab/boards.toml` key
   `stm32f469i-disco` must permit `flash`; absent op = refused.

```bash
make hil-place                 # idempotent place (re)creation
make test-hil                  # bench-wrapped full battery
make test-hil-quick            # phases 1-2 (skips the USB serial phase)
python3 tools/hil/bench.py -- --phase hil    # arbitrary run_hil.sh passthrough
```

Backups land in `tools/hil/results/` (gitignored). If the per-project
harness copies ever need a shared home, tollgate-lab is the consolidation
target.
