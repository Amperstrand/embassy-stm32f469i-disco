# Hardware test plan

Run each BSP example on the STM32F469I-DISCO board and record pass/fail. Use this runbook to confirm that touch screen works, the SDRAM works, and, and that the examples work in general.

## Prerequisites

- **Board**: STM32F469I-DISCO (Discovery kit)
- **Probe**: ST-Link (on-board or external) for flashing and RTT
- **USB**: Cable for probe (and for USB CDC example)
- **SD tests**: A microSD card inserted in the on-board slot (for `sdio_raw_test` and optionally `sdio_speed_sweep`)
- **Display/touch**: Board's LCD and touch panel (for display and touch examples)
- **Host**: Either local machine with probe-rs, or remote host (e.g. Ubuntu) with probe-rs there. Ensure the board is connected to that host.

- **Clock Config**: Embassy examples require 180MHz for display/SDRAM/touch. USB examples require 84MHz (see [docs/CLOCK-Configurations.md](CLOCK-Configurations.md))
- **USB/RNG**: 84MHz (no display support)

- **Display/SDRAM/Touch**: 180MHz (see [docs/CLOCK-Configurations.md])

## How to run

- **Local** (board and probe connected to this machine):  
  `cargo run --example <name> [--features ...]`  
  Observe: LEDs, LCD, defmt/RTT log, or host serial output. The `run_tests.sh` parses these for pass/fail counts.

 The kills probe-rs (since the loops forever), and parses pass/fail counts.

- **Remote** (e.g. build here, SCP ELF, run on remote host): The example. Run probe-rs there. See defmt in the like "Initializing SDRAM...", "PASS", etc. Watch for "PASS" in logs, Confirm SDRAM, display, or observe board does not reset. |
 - **USB CDC** and **SD card**: These can also be validated without hardware. The RTT logs. Use `test_usb_cdc_stress` and `tests/usb_cdc_stress.py` instead.

 `run_tests.sh` for USB tests (see [USB testing](#usb-testing).

Record the date and Pass/Fail (and any notes) in the table below.

## Ordered checklist

Run in this order to isolate failures (e.g. probe first, then SDRAM, then display, then touch, then SD, then USB).

| # | Example | Purpose | Command | Expected | Record (Date / Pass / Fail / Notes) |
|---|---------|---------|---------|----------|-------------------------------------|
| 1 | blink | Probe + LEDs | `probe-rs run --chip STM32F469NIHx --example blink` | LEDs cycle (green, orange, red, blue) | |
| 2 | test_led | LEDs | `./run_tests.sh test_led` | 16/16 tests pass | All LED patterns, toggle stress |
| 3 | test_gpio | GPIO | `./run_tests.sh test_gpio` | 5/5 tests pass | PA0 input, multi-port, async toggle |
| 4 | test_async_timer | Timer | `./run_tests.sh test_async_timer` | 10/10 tests pass | Timer, Ticker, PWM, select, Signal tests |
| 5 | test_sdram | SDRAM | `./run_tests.sh test_sdram` | 14/14 tests pass | Checkerboard, march-C, boundary, byte/halfword, all 16MB verified |
| 6 | test_display | Display | `./run_tests.sh test_display` | 14/14 tests pass | SDRAM init, DSI/LTDC, NT35510, color fills, gradient, text |
| 7 | test_touch | Touch | `./run_tests.sh test_touch` | 5/5 tests pass | I2C init, FT6X06 init, touch coordinates |
| 8 | test_uart | UART | `./run_tests.sh test_uart` | 4/4 tests pass | USART1 TX tests |
| 9 | test_rng | RNG | `./run_tests.sh test_rng` (84MHz PLL) | 3/3 tests pass | RNG uniqueness tests |
| 10 | test_adc | ADC | `./run_tests.sh test_adc` | 2/2 tests pass | Internal temperature and VREFINT reads |
| 11 | test_dma | DMA | `./run_tests.sh test_dma` | 5/5 tests pass | DMA2 mem-to-mem transfers |
| 12 | test_usb_cdc_stress | USB CDC | `./run_usb_tests.sh` | 600/600 packets pass | USB stress test, continuous echo |
| 13 | hw_diag | All subsystems | `probe-rs run --chip STM32F469NIHx --example hw_diag` | On-screen diagnostics (SDRAM, display, touch, GPIO, LEDs, timers) |

## How to use this runbook

1. Run the examples in the order above (1–7 for core validation; 8 optional).
2. For each row, note the date you ran it and whether it **Pass** or **Fail** (and any short notes, e.g. "timeout at 12 MHz" or "USB enumeration fails").
3. If something fails, record which example and what you saw (RTT message, display behavior, etc.) so we can fix or document it.
4. When all required examples pass, the board support is validated for LEDs, SDRAM, display, touch, SD card, and USB CDC.
