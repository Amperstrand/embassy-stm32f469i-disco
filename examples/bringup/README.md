# Bring-Up Examples

These are historical hardware bring-up and debugging examples. They are **not recommended starting points** for new projects.

These examples contain raw register access, `unsafe` code, and workarounds that were used during initial hardware bring-up of the STM32F469I-Discovery board. They are preserved here as debugging references.

## Examples

| File | Description |
|------|-------------|
| `raw_dsi_bringup.rs` | Raw DSI/LTDC register access with panel auto-detection |
| `raw_display_minimal.rs` | Minimal DSI display test (mirrors verified embassy dsi_bsp.rs) |
| `clk48_hypothesis.rs` | CK48MSEL hypothesis test (issue #27, PLLSAI vs DCKCFGR2) |
| `nt35510_register_probe.rs` | NT35510 panel register reads, ID verification, DSI command tests |
| `sdram_raw.rs` | SDRAM initialization and quick memory test |

## New Users

Start with the idiomatic examples in the top-level `examples/` directory instead:

- `examples/board_blinky.rs` — Basic LED blink
- `examples/board_display.rs` — Display init + color cycling
- `examples/board_touch.rs` — Touch controller example

## Building

Bring-up examples are gated behind the `bringup` feature flag and are **not** built by default:

```bash
# Build only idiomatic examples (default)
cargo build --target thumbv7em-none-eabihf --examples

# Build all examples including bring-up
cargo build --target thumbv7em-none-eabihf --examples --features bringup

# Build a specific bring-up example
cargo build --target thumbv7em-none-eabihf --example raw_dsi_bringup --features bringup
```
