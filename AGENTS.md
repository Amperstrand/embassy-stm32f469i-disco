# embassy-stm32f469i-disco

Board support package for the STM32F469I-Discovery development board, built on the Embassy async framework.

## Build

```bash
cargo build
cargo build --example display_blinky
```

## Run Examples

```bash
probe-rs run --chip STM32F469NIHx --example blink
probe-rs run --chip STM32F469NIHx --example display_blinky
probe-rs run --chip STM32F469NIHx --example sdram_test
```

## Architecture

```
src/
└── lib.rs    — SdramCtrl (FMC + IS42S32400F-6BL), DisplayCtrl (DSI/LTDC/NT35510), TouchCtrl (FT6X06)

examples/
├── blink.rs              — Basic LED blink
├── display_blinky.rs     — Display init + color cycling
└── sdram_test.rs         — SDRAM write/read verification
```

## Hardware

- MCU: STM32F469NIH6 (ARM Cortex-M4F, 180MHz)
- Display: 480x800 RGB565 LCD via DSI/LTDC (NT35510 controller)
- SDRAM: 16MB via FMC (IS42S32400F-6BL)
- Touch: FT6X06 capacitive touch via I2C

## Key Dependencies

- `embassy-stm32` @ `84444a19` (upstream embassy-rs)
- `stm32-fmc` 0.4.0 — SDRAM controller
- `nt35510` 0.1.0 — DSI display controller
- `embedded-display-controller` 0.2.0
- `embedded-graphics` 0.8

## Upstream Interaction Policy

**NEVER file PRs or issues on upstream projects (embassy-rs, stm32-rs, DougAnderson444, etc.) without human review and approval.** AI-generated bug diagnoses can be confidently wrong. If you find a potential upstream bug:
1. Document your findings in an Amperstrand repo issue first
2. Include all evidence (register dumps, test results, methodology)
3. Let a human decide whether to escalate

See [Amperstrand/micronuts#19](https://github.com/Amperstrand/micronuts/issues/19) for a retrospective on how a confident misdiagnosis wasted upstream maintainer time.
