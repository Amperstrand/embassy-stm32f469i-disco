# embassy-stm32f469i-disco

Board support package for the STM32F469I-Discovery development board, built on the Embassy async framework.

## Build

```bash
cargo build --target thumbv7em-none-eabihf
cargo build --target thumbv7em-none-eabihf --example display_blinky
cargo build --target thumbv7em-none-eabihf --examples
```

## Run Examples

Most examples require 180MHz PLL (for SDRAM/display). Use probe-rs:

```bash
probe-rs run --chip STM32F469NIHx --example display_blinky
probe-rs run --chip STM32F469NIHx --example hw_diag
```

USB CDC tests can run at 168 MHz with PLLSAI for display coexistence, or standalone at lower clock. Use `st-flash` — **do NOT use probe-rs during USB testing**:

```bash
./run_usb_tests.sh
```

## CI

GitHub Actions runs on push/PR: build library + all examples, `cargo fmt`, `cargo clippy -D warnings`. Zero warnings required.

## Architecture

```
src/
├── lib.rs       — Exports: DisplayCtrl, FramebufferView, SdramCtrl, SdramCtrl, TouchCtrl,
│                  BoardHint, LcdController, FB_HEIGHT, FB_WIDTH, SDRAM_SIZE_BYTES
└── display.rs   — SdramCtrl (FMC + IS42S32400F-6BL), DisplayCtrl (DSI/LTDC/NT35510),
                   BoardHint, LcdController, detect_panel()

examples/
├── blink.rs                 — Basic LED blink
├── display_blinky.rs        — Display init + color cycling
├── hw_diag.rs               — On-screen hardware diagnostics (~38 tests, two-phase)
│
├── test_led.rs              — LED tests (16)
├── test_gpio.rs             — GPIO tests (5)
├── test_async_timer.rs      — Timer/Ticker/PWM tests (10)
├── test_rng.rs              — Hardware RNG tests (3)
├── test_adc.rs              — ADC internal channel tests (2)
├── test_sdram.rs            — SDRAM fast tests (14)
├── test_sdram_full.rs       — SDRAM exhaustive tests (13)
├── test_display.rs          — Display/DSI/LTDC tests (15)
├── test_touch.rs            — FT6X06 touch tests (6)
├── test_uart.rs             — USART1 tests (4)
├── test_dma.rs              — DMA2 M2M tests (5)
├── test_usb.rs              — USB GPIO pin tests (3)
├── test_usb_cdc.rs          — USB CDC connectivity tests (3, 84MHz PLL, serial output)
├── test_usb_cdc_stress.rs   — USB CDC continuous echo (stress firmware)
├── test_itm_swo.rs          — ITM/SWO + USB CDC coexistence (84MHz PLL, ITM debug output)
├── test_sdram_soak.rs       — SDRAM continuous stress (soak firmware)
└── test_usb_soak.rs         — GPIO soak test (continuous toggle)

tests/
├── usb_cdc_stress.py        — Host-side USB stress test (pyserial, 600 packets)
├── usb_cdc_test.py          — Host-side USB CDC test monitor (pyserial, parses PASS/FAIL)
└── results/                 — Stress test JSON results (gitignored)

run_tests.sh                 — probe-rs based runner (all non-USB tests)
run_usb_tests.sh             — st-flash based runner (USB CDC stress test)
run_usb_cdc_test.sh          — st-flash based runner (USB CDC connectivity test)
```

## Hardware

- MCU: STM32F469NIH6 (ARM Cortex-M4F, 180MHz)
- Display: 480x800 RGB565 LCD via DSI/LTDC (NT35510 controller)
- SDRAM: 16MB via FMC (IS42S32400F-6BL)
- Touch: FT6X06 capacitive touch via I2C1 (PB8=SCL, PB9=SDA)
- USB: OTG FS (PA11=DM, PA12=DP) — CDC-ACM

## Key Dependencies

- `embassy-stm32` @ `84444a19` (upstream embassy-rs)
- `stm32-fmc` 0.4.0 — SDRAM controller
- `nt35510` 0.1.0 — DSI display controller
- `embedded-display-controller` 0.2.0
- `embedded-graphics` 0.8
- `embassy-usb` @ `84444a19` — USB CDC
- `stm32-metapac` 21 — Raw peripheral access (ADC, DMA, RNG)

## Clock Configurations

Use [`config_180()`], [`config_168()`], or [`config_usb_only()`] from `src/clock.rs` — do not manually configure PLL/PLLSAI.

| Config | Sysclk | 48MHz source | USB | RNG | Display | Used by |
|--------|--------|-------------|-----|-----|---------|---------|
| 180MHz | HSE/8 * 360 / 2 | PLLSAI_Q (384/8=48MHz) | YES | YES | PLLSAI_R=54.86MHz | micronuts, gm65-scanner |
| 168MHz | HSE/4 * 168 / 2 | PLL1_Q (168/7=48MHz) | YES | YES | PLLSAI_R=54.86MHz | microfips |
| USB-only | 168MHz | PLL1_Q | YES | YES | NO | USB CDC without display |

**Hardware-verified** (2026-05-07, issue #27): At 180MHz, PLLSAI_Q provides 48MHz via `divq: DIV8`.
`clk48sel = PLLSAI1_Q` routes this to USB/RNG. Embassy writes CK48MSEL to DCKCFGR (correct register).
DCKCFGR2 does not exist on STM32F469 — the "DCKCFGR2 workaround" was a harmless no-op.

## Test Output Format

All test examples output RTT-compatible lines (except `test_usb_cdc` which uses USB CDC serial):
```
TEST <name>: PASS
TEST <name>: FAIL <reason>
SUMMARY: N/M passed
ALL TESTS PASSED
```

`run_tests.sh` parses these for automated pass/fail reporting.
`tests/usb_cdc_test.py` does the same for USB CDC serial output.

## USB CDC Stress Test

The stress test validates USB reliability without debugger interference:

```bash
# Full pipeline: build → flash → reset → stress test
./run_usb_tests.sh

# Individual steps
./run_usb_tests.sh --build-only    # build firmware
./run_usb_tests.sh --flash-only    # flash via st-flash
./run_usb_tests.sh --test-only     # run host-side test (already flashed)
./run_usb_tests.sh --count 1000    # send 1000 packets
./run_usb_tests.sh --find          # auto-detect port by VID:PID

# Standalone host-side script
python3 tests/usb_cdc_stress.py --port /dev/ttyACM0 --count 600
```

Requirements: `st-flash` (stlink-tools), `arm-none-eabi-objcopy`, `pyserial`.

## USB CDC Test

The USB CDC test validates USB init, CDC class creation, and echo functionality. Results are output over USB serial (no probe-rs needed):

```bash
# Build and flash (st-flash, NOT probe-rs)
cargo build --release --target thumbv7em-none-eabihf --example test_usb_cdc
arm-none-eabi-objcopy -O binary target/thumbv7em-none-eabihf/release/examples/test_usb_cdc test.bin
st-flash --connect-under-reset write test.bin 0x08000000
st-flash --connect-under-reset reset
sleep 15

# Run host-side monitor (sends echo data, parses results)
python3 tests/usb_cdc_test.py --port /dev/ttyACM0
```

Tests: `usb_init`, `usb_cdc_init`, `usb_cdc_echo`. The echo test requires the host script to send data — without it, echo times out after 5s (expected FAIL).

## Known-Good Pins

| Commit | Branch | Notes |
|--------|--------|-------|
| `4278bbd` | `main` | All examples migrated to clock presets, DsiHostCtrlIo casing fix |
| `57c20e3` | `main` | docs: known-good pins, clock config adoption status. **Current HEAD** |
| `07e09cd` | `main` | hw_diag RNG/Timer/DMA fixes (pending HW verification, issue #29) |
| `f856b6e` | `main` | USB PHY reset, clock docs corrected. **Used by micronuts + microfips** |
| `d28c859` | `main` | docs: correct 48MHz clock mechanism (PLLSAI_Q not PLLSAI_P) |
| `cdd9e91` | `main` | Clock presets module, CK48MSEL hypothesis test |
| `151b5ba` | `main` | Embassy deps migrated from git rev to crates.io |
| `e202e9a` | `main` | docs: update AGENTS.md with ITM example, re-exports, known-good pins |
| `a64458f` | `main` | ITM/SWO example, full rustdoc, SdramCtrl/SDRAM_SIZE_BYTES re-export |
| `a50f241` | `main` | Stress firmware stale buffer fix, read_vendor_id/read_chip_model, run_usb_cdc_test.sh |
| `25d5ecb` | `main` | USB CDC test serial output, host-side monitor, clippy fixes |
| `3646aa8` | `main` | Fixed RawDsi::read() register and FIFO flow control |
| `a407fcd` | `feat/hil-tests` | HIL test suite + USART6 UART module. **Used by micronuts firmware** |
| `31b81d4` | `main` | Full test suite, CI, USB stress test, zero warnings |

The `main` branch is recommended for new projects. The `feat/hil-tests` branch adds HIL test infrastructure but is functionally equivalent for library use.

## Hardware Test Evidence

All testing performed on STM32F469I-Discovery board (B08 revision, NT35510 panel).

### Test Date: 2026-03-26

Testing performed by the **micronuts** firmware (Amperstrand/micronuts), which depends on this BSP for display, SDRAM, and touch initialization. The BSP's own test suite is built but not yet run independently on hardware.

| Subsystem | Status | Evidence | Notes |
|-----------|--------|----------|-------|
| **SDRAM** | PASS | Write/read 4096 u16 pattern verified | 16MB FMC, IS42S32400F-6BL. micronuts uses 768KB for framebuffer + 128KB heap |
| **Display (DSI/LTDC)** | PASS | Green fill + readback verified (384000 pixels) | NT35510 via DSI burst video mode, 480x800 RGB565. Boot splash animation, QR code rendering all verified |
| **Touch (FT6X06)** | PASS | Touch detected at x=258,y=382 and x=313,y=277 | I2C1 at PB8/PB9. Phantom touches at screen edges filtered with 3px margin |
| **USB CDC** | PASS | 600/600 stress test at 504 cmds/sec | embassy-usb @ 84444a19. probe-rs breaks USB enumeration — do NOT attach probe-rs during USB testing |
| **RNG** | PASS | 166 unique values in 256 bytes | SHA-256 conditioned before use |
| **Heap** | PASS | 1024 byte alloc + pattern verified | 128KB heap in SDRAM at offset 768KB |

### Test Date: 2026-03-29

BSP's own test suite run independently on hardware via probe-rs.

| Test Example | Result | Tests | Notes |
|-------------|--------|-------|-------|
| **test_led** | PASS | 16/16 | All 4 LEDs, patterns, toggle stress |
| **test_gpio** | PASS | 5/5 | PA0 input, stability, multi-port, async toggle |
| **test_async_timer** | PASS | 10/10 | Timer, Ticker, select, Signal, PWM, DWT. Runs at 16MHz HSI (Config::default) |
| **test_rng** | PASS | 3/3 | Requires 84MHz PLL (48MHz on PLL1_Q). 64/64 unique values |
| **test_adc** | PASS | 2/2 | Temp sensor (936 raw), VREFINT (1501 raw). Fixed SMPR2 register index |
| **test_uart** | PASS | 4/4 | USART1 init, TX byte, multi-byte, fmt::Write adapter |
| **test_dma** | PASS | 5/5 | DMA2 M2M: 64B, 4096B, 1024B, repeated, timing (147us for 64B) |
| **test_sdram** | PASS | 14/14 | Full 16MB SDRAM: checkerboard, march-C, boundary, byte/halfword |
| **test_sdram_full** | PASS | 13/13 | Exhaustive 16MB: walking bits, random xorshift, solid fills, multi-pass |
| **test_display** | PASS | 14/14 | SDRAM init, DSI/LTDC, NT35510 detect, color fills, gradient, text, rapid refresh |
| **test_touch** | PASS | 5/5 | Requires SDRAM+display init (FT6X06 powered from display module). Vendor ID=0x11 at reg 0xA8 |
| **hw_diag** | 33/38 | Phase 1: 12/15, Phase 2: 21/23 | RNG x3 FAIL (no 48MHz clock at 180MHz PLL). All other subsystems pass |
| **test_usb_cdc** | PENDING | 3/3 | Rewritten for USB serial output (25d5ecb). Requires 84MHz PLL, st-flash deploy, host-side monitor. Not yet tested on hardware with new serial output approach |
| **USB CDC stress** | 591/600 | 98.5% | Phase 3 first packet timeout, Phase 4 stale buffer (test firmware issue, not USB stack) |

### Bugs Found and Fixed During Hardware Testing

| Issue | Root Cause | Fix |
|-------|-----------|-----|
| `test_async_timer` timing failures (5/10) | `cycles_to_us()` divided by 180 but `Config::default()` runs at 16MHz HSI | Changed divisor to 16 |
| `test_adc` HardFault | `Smpr2::set_smp(channel)` — channel 17/18 out of 0-9 range (SMPR2 covers channels 10-18, 0-indexed) | Use `channel - 10` as index |
| `test_rng` hang | RNG requires 48MHz clock; `Config::default()` (16MHz HSI) provides none | Added 84MHz PLL config, init timeout |
| `test_touch` I2C failures (3/5) | FT6X06 is powered from display module; no SDRAM/display init = no power to touch | Init SDRAM+display before I2C |
| `hw_diag` RNG hang | Same 48MHz clock issue as test_rng at 180MHz PLL | Added timeout counters to RNG busy-wait loops |
| `hw_diag` ADC crash | Same SMPR2 register index bug as test_adc | Fixed SMPR2 indices |
| `run_usb_tests.sh` arg parsing | `for arg in "$@"` + `shift` doesn't work in bash | Changed to `while [ $# -gt 0 ]` loop |
| FT6X06 chip ID mismatch | BSP `read_chip_id()` reads reg 0xA8 (vendor ID=0x11), not reg 0xA3 (chip model). Test expected wrong values 0xCC/0xA3 | Renamed to `read_vendor_id()`, added `read_chip_model()`. See #9 (closed) |

### What's NOT Tested (on this BSP directly)

| Subsystem | Status | Notes |
|-----------|--------|-------|
| **DSI reads** | FAIL | `DisplayCtrl::probe()` fails consistently (BTA/PHY timing). Workaround: `BoardHint::ForceNt35510` skips probe. Writes work fine, display renders correctly. Not needed for normal operation. |
| **RNG at 180MHz PLL** | PASS | 180MHz config: PLL1_Q=51.4MHz (wrong for USB/RNG). Fix: PLLSAI_Q=48MHz via `divq: Some(PllQDiv::DIV8)`, CK48MSEL routed via DCKCFGR (not DCKCFGR2 — that register does not exist on F469). See #27 (hardware-verified) |
| **SDIO** | NOT TESTED | No microSD card testing. Out of scope for Cashu wallet use case. |

## Known Issues

### STM32F469 Clock Configuration Pitfall (recurring across Amperstrand projects)

The STM32F469 clock tree has a non-obvious constraint: **no single PLL config produces both 180 MHz SYSCLK and 48 MHz USB from PLL1 alone**. PLL1_Q at 180 MHz = 51.4 MHz (7.1% off, USB requires ±0.25%).

**This issue has been rediscovered independently in at least 4 projects** (gm65-scanner, micronuts, microfips, this BSP). The BSP's own README previously stated "USB requires a separate config (incompatible with display)" — this is **incorrect**.

**Two working solutions** (both available as presets in `src/clock.rs`):

| Config | SYSCLK | USB Clock | LTDC Pixel Clock | Used By |
|--------|--------|-----------|-----------------|---------|
| 168 MHz + PLLSAI | 168 MHz | PLL_Q = 48 MHz | PLLSAI_R = 54.86 MHz | microfips (#113) |
| 180 MHz + PLLSAI | 180 MHz | PLLSAI_Q = 48 MHz | PLLSAI_R = 54.86 MHz | micronuts |

**~~embassy-stm32 register bug~~ (CORRECTED — see #27):** We previously claimed embassy writes `clk48sel` to the wrong register (`RCC.DCKCFGR` instead of `RCC.DCKCFGR2`). This is **incorrect**. DCKCFGR2 does not exist on STM32F469 — the register at offset 0x94 reads back 0 after write. CK48MSEL is correctly in DCKCFGR bit 27, and embassy writes to the right register. Hardware testing (test_clk48_hypothesis, conditions A-D, 2026-05-07) confirmed: RNG passes in all conditions, DCKCFGR2 writes never stick. The "DCKCFGR2 workaround" was always a no-op.

**sync HAL bug (still valid):** `stm32f4xx-hal`'s `DisplayController::new()` uses `.write()` on PLLSAICFGR, destroying PLLSAI_P and PLLSAI_Q. The embassy BSP avoids this by configuring PLLSAI through `config.rcc.pllsai`. See gm65-scanner#47.

**Issue chain:**
- embassy-stm32f469i-disco#1, #25 — PLLSAI not configured (display crashes)
- embassy-stm32f469i-disco#13, #14 — 48MHz clock at 180MHz (definitive workaround)
- embassy-stm32f469i-disco#27 — DCKCFGR2 claim disproven by hardware test (48MHz comes from PLLSAI_Q)
- gm65-scanner#23 — DCKCFGR2 workaround was a no-op (corrected in comment)
- gm65-scanner#47, #50 — sync HAL PLLSAI destruction + ARGB8888 column shift

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

**Workaround**: `DisplayCtrl::new()` with `BoardHint::Auto` attempts probe up to 3 retries, falls back to NT35510 if inconclusive. Use `BoardHint::ForceNt35510` to skip probe entirely.

### probe-rs Breaks USB Enumeration

When `probe-rs run` is attached for RTT defmt logging, RTT may be left in blocking mode on disconnect (probe-rs#2425). Any subsequent `defmt::info!()` call acquires a critical section and spin-loops waiting for the probe to drain the buffer. This blocks the USB ISR, causing host-side disconnects.

**Correct USB test methodology** (used by `run_usb_tests.sh`):
```bash
# Flash with st-flash (not probe-rs)
arm-none-eabi-objcopy -O binary firmware firmware.bin
st-flash --connect-under-reset write firmware.bin 0x08000000
st-flash --connect-under-reset reset
sleep 15  # wait for USB enumeration
python3 tests/usb_cdc_stress.py --port /dev/ttyACM0
```

### ST-LINK Recovery After USB Active

When USB CDC is active, the STM32F469 can lock out SWD. Recovery:
```bash
st-flash --connect-under-reset reset
# Immediately run probe-rs if needed
probe-rs run --chip STM32F469NIHx firmware
# If that fails, full power cycle
```

### USB PHY Reset After st-flash Soft Reset

When `st-flash --connect-under-reset reset` performs a soft reset (SYSRESETREQ), the USB OTG FS peripheral can be left in an inconsistent state where the PHY doesn't re-enumerate on subsequent boots. This affects all USB CDC firmware.

**Fix**: Call `embassy_stm32f469i_disco::reset_usb_phy()` **before** creating the USB driver. The BSP provides this function that performs a complete reset sequence:
1. Disable USB OTG FS clock
2. Assert USB OTG FS peripheral reset
3. Perform core soft reset (GRSTCTL.CSRST)
4. Power-cycle the PHY (GCCFG.PWRDWN)

```rust
let p = embassy_stm32::init(config_168());

// Reset USB PHY for clean re-enumeration
embassy_stm32f469i_disco::reset_usb_phy();

// Now create USB driver
let driver = embassy_stm32::usb::Driver::new_fs(
    p.USB_OTG_FS,
    Irqs,
    p.PA12,
    p.PA11,
    ep_out_buffer,
    usb_config,
);
```

**Affected:** micronuts#34, microfips#105, gm65-scanner#56 — all three projects independently discovered this issue.
**Reference:** `src/usb.rs` module in this BSP

## Embassy USB Investigation (embassy-rs/embassy#5738)

PR #5738 claimed `configure_endpoints()` setting SNAK on IN endpoints causes USB hangs. Our testing on STM32F469I-DISCO showed:
- **Upstream `84444a19`**: 600/600 stress test passes, 504 cmds/sec, no EPENA hangs observed
- **PR's test branches**: All 5 variants tested, EPENA stuck detection never fired
- **PR was closed without merging** (2026-03-26)

We concluded the claimed EPENA hang may be timing-dependent or caused by probe-rs artifacts. Our hardware does not reproduce it.

## Cross-Project Recurring Patterns

Patterns that have recurred across Amperstrand STM32F469 projects. Each was independently rediscovered at least twice.

### USB CDC + probe-rs Interference (4+ projects)

When `probe-rs run` is attached for RTT logging, RTT may be left in blocking mode on disconnect. Any `defmt::info!()` call acquires a critical section and blocks the USB ISR, causing disconnects.

**Affected:** gm65-scanner#39, micronuts#18/#19, microfips#2/#3, embassy BSP (probe-rs breaks USB)
**Fix:** Use `st-flash` for USB firmware deployment. Never `probe-rs run` during USB testing.
**Reference:** embassy BSP Known Issues, gm65-scanner#19

### USB OTG FS Re-enumeration After Soft Reset (3 projects)

`st-flash` soft reset doesn't properly reset USB PHY. Device doesn't re-enumerate after SYSRESETREQ.

**Affected:** micronuts#34, microfips#105, gm65-scanner#56
**Fix:** Full power cycle, or explicit USB PHY reset sequence in firmware.
**Reference:** embassy BSP `src/usb.rs::reset_usb_phy()`

### FT6X06 Phantom Touch Events (3 projects)

FT6X06 reports phantom touches at screen edges (x=0, y=445, x=479, y=767). Electrical noise on capacitive sensor.

**Affected:** embassy BSP#16, stm32f469i-disc#17, gm65-scanner (touch integration)
**Fix:** Filter with 3px margin: `if x < 3 || x > 476 || y < 3 || y > 796 { return None; }`
**Reference:** stm32f469i-disc#17

### DSI Command-Mode Reads Fail (2 BSPs)

DSI reads (for panel auto-detection) fail with BTA/PHY timing issues. Writes work fine.

**Affected:** embassy BSP#15, stm32f469i-disc#12
**Fix:** Use `BoardHint::ForceNt35510` to skip probe. Don't rely on DSI reads.
**Reference:** stm32f469i-disc#12

### memory.x Wrong Flash/RAM Sizes (3 projects)

STM32F469NIHx has 2048K flash and 320K SRAM (not 384K, not 1024K). Wrong values cause crashes.

**Affected:** embassy BSP#2 (384K→320K), embassy BSP#19 (1024K→2048K), micronuts#5 (heap overlap)
**Fix:** Use BSP-provided memory.x. Verify against datasheet.

### defmt Feature Leak in Embassy Deps (2 projects)

`defmt` compiled unconditionally into embassy deps breaks USB CDC. Must be feature-gated.

**Affected:** stm32f469i-disc#23, microfips#36
**Fix:** Ensure `defmt` is behind a feature flag in all embassy dependency declarations.

### ST-LINK / SWD Lockout Recovery (3 projects)

USB or SDRAM active can lock out SWD. Universal fix: `st-flash --connect-under-reset reset`.

**Affected:** embassy BSP, gm65-scanner, micronuts
**Fix:** `st-flash --connect-under-reset reset`, then immediately run probe-rs. Full power cycle if needed.

### Scanner UART Pitfalls (gm65-scanner, micronuts)

- `drain_uart()` inside `send_command()` destroys queued scan data (gm65-scanner#12)
- `AsyncUart::read` silently retries on framing/overrun errors (gm65-scanner#54)
- Baud-upgrade path leaves scanner unusable (micronuts#8)

**Fix:** Don't drain UART before reading scan data. Handle framing errors explicitly. Re-init scanner after baud change.

### Simulator SIGSEGV on NVIDIA + SDL2 (2 projects)

Desktop simulator crashes on NVIDIA GPU setups with SDL2.

**Affected:** micronuts#4, specter-diy#16
**Fix:** Use software renderer or run on non-NVIDIA hardware for simulator testing.

### USB OTG FS Re-enumeration After Soft Reset (2 projects)

`st-flash` soft reset doesn't properly reset USB PHY. Device doesn't re-enumerate.

**Affected:** micronuts#34, microfips#105
**Fix:** Full power cycle, or explicit USB PHY reset sequence in firmware.

## Pin Consumption

| Peripheral | Pins | Notes |
|------------|------|-------|
| FMC/SDRAM | PD0,1,8,9,10,14,15, PE0,1,7,8,9,10,11,12,13,14,15, PF0,1,2,3,4,5,11,12,13,14,15, PG0,1,2,3,4,5, PH5,6,7,8,9,10,11,12,13,14,15, PI0,1,2,3,4,5,6,7 | Full 16MB SDRAM bus |
| DSI | PA0,1,2,3,4,5,6,7, PH8,9,10,11,12, PI9,10,11,12 | 2-lane DSI |
| LTDC | PC0,1,2,3,6,7,8,9,10, PA8,9,10, PH0,1,2,3,4, PI10,11,12 | LCD timing and data |
| I2C1 (touch) | PB8 (SCL), PB9 (SDA) | FT6X06 touch controller |
| LCD reset | PH7 | NT35510 panel reset |
| USART1 (test) | PA9 (TX), PA10 (RX) | Test UART |
| USART6 (scanner) | PG14 (TX), PG9 (RX) | NOT consumed by SDRAM — available for QR scanner |
| USB OTG FS | PA11 (DM), PA12 (DP) | CDC-ACM |
| LEDs | PG6 (green), PD4 (orange), PD5 (red), PK3 (blue) | Active low |

USART6 PG14/PG9 are exposed via `SdramRemainders` but not documented in PIN-CONSUMPTION.md on the old sync BSP (#16).

## Upstream Interaction Policy

**NEVER file PRs or issues on upstream projects (embassy-rs, stm32-rs, DougAnderson444, etc.) without human review and approval.** AI-generated bug diagnoses can be confidently wrong. If you find a potential upstream bug:
1. Document your findings in an Amperstrand repo issue first
2. Include all evidence (register dumps, test results, methodology)
3. Let a human decide whether to escalate

See [Amperstrand/micronuts#19](https://github.com/Amperstrand/micronuts/issues/19) for a retrospective on how a confident misdiagnosis wasted upstream maintainer time.
