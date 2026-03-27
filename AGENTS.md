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
- Touch: FT6X06 capacitive touch via I2C1 (PB8=SDA, PB9=SCL)

## Key Dependencies

- `embassy-stm32` @ `84444a19` (upstream embassy-rs)
- `stm32-fmc` 0.4.0 — SDRAM controller
- `nt35510` 0.1.0 — DSI display controller
- `embedded-display-controller` 0.2.0
- `embedded-graphics` 0.8

## Known-Good Pins

| Commit | Branch | Notes |
|--------|--------|-------|
| `3646aa8` | `main` | Fixed RawDsi::read() register and FIFO flow control |
| `a407fcd` | `feat/hil-tests` | HIL test suite + USART6 UART module. **Used by micronuts firmware** |

The `main` branch is recommended for new projects. The `feat/hil-tests` branch adds HIL test infrastructure but is functionally equivalent for library use.

## Hardware Test Evidence

All testing performed on STM32F469I-Discovery board (B08 revision, NT35510 panel).

### Test Date: 2026-03-26

Testing performed by the **micronuts** firmware (Amperstrand/micronuts), which depends on this BSP for display, SDRAM, and touch initialization. The BSP itself has HIL test examples built but not yet run independently.

| Subsystem | Status | Evidence | Notes |
|-----------|--------|----------|-------|
| **SDRAM** | PASS | Write/read 4096 u16 pattern verified | 16MB FMC, IS42S32400F-6BL. micronuts uses 768KB for framebuffer + 128KB heap |
| **Display (DSI/LTDC)** | PASS | Green fill + readback verified (384000 pixels) | NT35510 via DSI burst video mode, 480x800 RGB565. Boot splash animation, QR code rendering all verified |
| **Touch (FT6X06)** | PASS | Touch detected at x=258,y=382 and x=313,y=277 | I2C1 at PB8/PB9. Phantom touches at screen edges filtered with 3px margin |
| **USB CDC** | PASS | 600/600 stress test at 504 cmds/sec | embassy-usb @ 84444a19. probe-rs breaks USB enumeration — do NOT attach probe-rs during USB testing |
| **RNG** | PASS | 166 unique values in 256 bytes | SHA-256 conditioned before use |
| **Heap** | PASS | 1024 byte alloc + pattern verified | 128KB heap in SDRAM at offset 768KB |

### What's NOT Tested (on this BSP directly)

| Subsystem | Status | Notes |
|-----------|--------|-------|
| **DSI reads** | FAIL | `DisplayCtrl::probe()` fails consistently (BTA/PHY timing). Workaround: `BoardHint::ForceNt35510` skips probe. Writes work fine, display renders correctly. Not needed for normal operation. |
| **SDIO** | NOT TESTED | No microSD card testing. Out of scope for Cashu wallet use case. |
| **HIL test suite** | NOT RUN | `hil_test_sync` example is built but has not been flashed and run on hardware. Known issues: double embassy_stm32::init() call (fixed in a407fcd), PLLSAI not configured (fixed in d4d7d08). |
| **Examples** | NOT RUN | `display_blinky`, `sdram_test` examples are built but not run on hardware. Verified indirectly through micronuts firmware which uses the same BSP APIs. |

## Known Issues

### FT6X06 Phantom Touch Events (#17 on stm32f469i-disc, fixed in firmware)

The FT6X06 reports phantom touches at screen edges (x=0, y=445, x=479, y=767). This is electrical noise picked up by the capacitive sensor.

**Workaround** (applied in micronuts firmware `hardware_impl.rs::touch_get()`):
```rust
if x < 3 || x > 476 || y < 3 || y > 796 {
    return None; // reject edge touches
}
```

The BSP itself does NOT apply this filter — consumers must implement their own edge rejection.

### DSI Probe Reads Fail (#12 on stm32f469i-disc, closed)

DSI command-mode reads (used for panel auto-detection) fail with "DSI read error". DSI writes work fine.

**Workaround**: The `DisplayCtrl::new()` on `main` branch skips probe. No impact on normal display operation.

### probe-rs Breaks USB Enumeration

When `probe-rs run` is attached for RTT defmt logging, it halts the CPU periodically for RTT reads. This causes USB disconnects. The firmware's USB CDC works correctly when probe-rs is NOT attached.

**Correct test methodology**:
```bash
# Flash with st-flash (not probe-rs)
arm-none-eabi-objcopy -O binary firmware firmware.bin
st-flash --connect-under-reset write firmware.bin 0x08000000
st-flash --connect-under-reset reset
# Wait 15s for boot + self-test
# Test USB via pyserial — do NOT attach probe-rs
```

### ST-LINK Recovery After USB Active

When USB CDC is active, the STM32F469 can lock out SWD. Recovery:
```bash
st-flash --connect-under-reset reset
# Immediately run probe-rs if needed
probe-rs run --chip STM32F469NIHx firmware
# If that fails, full power cycle
```

## Embassy USB Investigation (embassy-rs/embassy#5738)

PR #5738 claimed `configure_endpoints()` setting SNAK on IN endpoints causes USB hangs. Our testing on STM32F469I-DISCO showed:
- **Upstream `84444a19`**: 600/600 stress test passes, 504 cmds/sec, no EPENA hangs observed
- **PR's test branches**: All 5 variants tested, EPENA stuck detection never fired
- **PR was closed without merging** (2026-03-26)

We concluded the claimed EPENA hang may be timing-dependent or caused by probe-rs artifacts. Our hardware does not reproduce it.

## Pin Consumption

| Peripheral | Pins | Notes |
|------------|------|-------|
| FMC/SDRAM | PD0,1,8,9,10,14,15, PE0,1,7,8,9,10,11,12,13,14,15, PF0,1,2,3,4,5,11,12,13,14,15, PG0,1,2,3,4,5, PH5,6,7,8,9,10,11,12,13,14,15, PI0,1,2,3,4,5,6,7 | Full 16MB SDRAM bus |
| DSI | PA0,1,2,3,4,5,6,7, PH8,9,10,11,12, PI9,10,11,12 | 2-lane DSI |
| LTDC | PC0,1,2,3,6,7,8,9,10, PA8,9,10, PH0,1,2,3,4, PI10,11,12 | LCD timing and data |
| I2C1 (touch) | PB8 (SDA), PB9 (SCL) | FT6X06 touch controller |
| LCD reset | PH7 | NT35510 panel reset |
| USART6 (scanner) | PG14 (TX), PG9 (RX) | NOT consumed by SDRAM — available for QR scanner |
| USB OTG FS | PA11 (DM), PA12 (DP) | CDC-ACM |

USART6 PG14/PG9 are exposed via `SdramRemainders` but not documented in PIN-CONSUMPTION.md on the old sync BSP (#16).

## Upstream Interaction Policy

**NEVER file PRs or issues on upstream projects (embassy-rs, stm32-rs, DougAnderson444, etc.) without human review and approval.** AI-generated bug diagnoses can be confidently wrong. If you find a potential upstream bug:
1. Document your findings in an Amperstrand repo issue first
2. Include all evidence (register dumps, test results, methodology)
3. Let a human decide whether to escalate

See [Amperstrand/micronuts#19](https://github.com/Amperstrand/micronuts/issues/19) for a retrospective on how a confident misdiagnosis wasted upstream maintainer time.
