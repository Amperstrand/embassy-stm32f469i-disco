# On-target embedded-test HIL tests

These tests use `embedded-test` plus `probe-rs` to flash and run hardware-in-the-loop checks on an STM32F469I-DISCO board.

## Prerequisites

- `probe-rs` installed (`cargo install probe-rs-tools`)
- STM32F469I-DISCO connected over ST-LINK
- Rust target installed: `rustup target add thumbv7em-none-eabihf`

## Build only

```bash
cargo test --target thumbv7em-none-eabihf --no-run
```

CI should stop here and only verify that the test binaries build.

## Run on hardware

```bash
cargo test --target thumbv7em-none-eabihf
```

`probe-rs run` is configured as the target runner in `.cargo/config.toml`, so Cargo will flash the generated `on_target` test binary and execute each test case on the board.

## Notes

- These tests require the BSP's 180MHz PLL preset because SDRAM, display, and touch are initialized together.
- The touch controller is powered from the display module, so touch checks intentionally initialize display hardware first.
- Do not use probe-rs during USB CDC testing; USB tests in this repository use the separate `st-flash`-based workflow documented in the top-level README and AGENTS.md.
