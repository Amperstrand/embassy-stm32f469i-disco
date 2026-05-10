# On-target embedded-test HIL tests

26 hardware-in-the-loop tests using `embedded-test` + `probe-rs`. Each test gets a fresh device reset, runs independently, and reports results via semihosting.

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
cargo test --target thumbv7em-none-eabihf --test on_target
```

`probe-rs run` is configured as the target runner in `.cargo/config.toml`. Cargo flashes the test binary and `embedded-test` executes each test case on the board with per-test device reset.

## Test coverage

| Subsystem | Tests | What's verified |
|-----------|-------|-----------------|
| SDRAM (6) | `sdram_write_read_pattern`, `sdram_checkerboard`, `sdram_march_c`, `sdram_end_of_ram`, `sdram_byte_halfword` | Write/read, checkerboard, march-C algorithm, xorshift at end of 16MB, byte and halfword access |
| Display (2) | `display_init`, `display_color_fill` | LTDC enabled + layer active; color fill + rectangle draw |
| Touch (2) | `touch_vendor_id`, `touch_chip_model` | FT6X06 vendor ID = 0x11; chip model = 0x06/0x36/0x64 |
| LED (1) | `led_toggle` | All 4 LEDs (PG6, PD4, PD5, PK3) toggle 3x |
| GPIO (2) | `gpio_pa0_input`, `gpio_multi_port_output` | PA0 user button input; multi-port output (PA, PG, PD) |
| Timer (3) | `timer_1ms`, `timer_100ms_accuracy`, `timer_ticker` | Basic delay; 95–120ms accuracy window; 5-tick ticker at 100ms |
| RNG (3) | `rng_not_zero`, `rng_uniqueness`, `rng_consecutive_differ` | Non-zero value; ≥32 unique in 64 samples; consecutive reads differ |
| ADC (2) | `adc_temp_sensor`, `adc_vrefint` | Ch18 temp sensor 100–4095 range; Ch17 VREFINT 500–3000 range |
| UART (3) | `uart_init`, `uart_tx_byte`, `uart_tx_multi_byte` | USART1 (PA9/PA10) init, single byte TX, multi-byte TX |
| DMA (3) | `dma_64b`, `dma_4096b`, `dma_repeated` | DMA2 stream 0 M2M: 64B, 4096B, 10× repeated transfers |

## Notes

- All tests require `config_180()` (180MHz PLL with PLLSAI for 48MHz USB/RNG clock).
- Touch tests initialize display first — FT6X06 is powered from the display module.
- UART tests are TX-only (no loopback wire on the board). They verify init + write succeeds.
- `defmt_rtt` is imported in the test harness to satisfy probe-rs defmt version symbol requirements.
- Do not use probe-rs during USB CDC testing; USB tests use the separate `st-flash`-based workflow.
