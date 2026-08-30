# USB CDC load-test harness (upstream embassy main)

Standalone CDC echo + byte-counter firmware that builds against UPSTREAM
embassy main via [patch.crates-io] git revs — NOT the released 0.6 line —
plus a full-duplex host verifier (loadtest2.py).

Verified 2026-08-30 on the F469I-DISCO: 64 MiB sustained round-trip
(205 s, 319 KiB/s steady, byte-perfect, zero stalls). The rewritten OTG
driver does NOT exhibit the IN-endpoint hang that Amperstrand/embassy
wip/usb-cdc-fix patched against the pre-rewrite driver — our fix is
obsolete. Re-run this harness when bumping the BSP to the 0.7 release.

API deltas vs 0.6 encountered here (for the BSP refresh): executor needs
`platform-cortex-m` + `executor-thread/interrupt` features; task fns
return Result<SpawnToken, SpawnError>; `Output<'static>` lost its pin
generic; cortex-m needs `critical-section-single-core`; git-dep ties
against registry versions require the [patch.crates-io] pattern in this
Cargo.toml.

Flash: cargo build --release, objcopy -O binary, st-flash --connect-under-reset
write at 0x08000000. NO defmt_rtt in any USB firmware on this board.
