# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-05-08

### Added

- `Board::new(p, hint)` ergonomic API: initializes SDRAM, display, touch, LEDs, and user button in one call. Exports `Board`, `Leds`, `UserButton`, `SdramRemainders` (`4992bdf`)
- Board-based examples: `board_blinky`, `board_display`, `board_touch` (`9e56fb8`)
- Generic `TouchCtrl<I2C>` parameterized over any `embedded_hal::i2c::I2c` implementation (`9c6cb06`)
- `EdgeFilter` for FT6X06 phantom-touch rejection with `default_ft6x06()` preset (`6e7f62c`)
- `TouchPoint` derives: `Clone`, `Copy`, `PartialEq`, `Eq`, `Debug`, `Default`, and `Display` (`9c6cb06`)
- `extensive_hw_test` interactive on-device test combining all subsystem tests (39 tests, two-phase) (`5276dd8`)
- CCMRAM result buffer in `extensive_hw_test` for probe-rs readback without RTT (`4e1e4d4`)
- `read_test_results.py` host-side script for reading on-device test results via probe-rs (`4e1e4d4`)
- `embedded-test` on-target HIL test runner (`tests/on_target.rs`) (`aba85eb`)
- `bringup` feature flag for historical bring-up examples in `examples/bringup/` (`8d1adb6`)
- CI feature matrix, clippy gate, doc gate, and package dry-run (`3859374`)
- Clock presets: `config_180()`, `config_168()`, `config_usb_only()` with `SYSCLK_HZ_180`, `SYSCLK_HZ_168` constants (`4278bbd`)
- LTDC register dump and quarter-fill framebuffer diagnostics (`3ebea95`)
- nt35510 dependency updated to v0.2.1 (git rev 49a372c) with derives, docs, defmt feature, new public methods (`91f84ea`)

### Changed

- `SdramCtrl::into_framebuffer(self)` now consumes `self` instead of taking `&self` and returning `&mut`. Use `into_bytes()` on the consumed controller. (`e5d5e8b`)
- `TouchCtrl::get_touch()` returns `Result<Option<TouchPoint>>` (returns `None` when no touch or edge-filtered) instead of `Result<TouchPoint>` (`75e4119`)
- `TouchCtrl::new(i2c)` now takes and stores the I2C instance instead of requiring it per-call (`9c6cb06`)
- `display.rs` split into `sdram.rs`, `dsi.rs`, `ltdc.rs`, `framebuffer.rs`, `panel/`, `display.rs` (`7dfcadb`, `bf9f6fc`, `21e7c3e`, `e5d5e8b`, `a1f1a7b`)
- Build system uses standard `cargo:rustc-link-search` pattern instead of ancestors-walk in build.rs (`f0220b3`)
- Bring-up examples relocated from `examples/` to `examples/bringup/` behind `--features bringup` (`8d1adb6`)
- Migrated all examples from inline PLL config to clock presets (`4278bbd`)

### Removed

- Broad clippy allows (`#![allow(...)]`) from lib.rs; individual warnings fixed instead (`5b3b042`)
- Stub IRQ handlers from examples (`238eec0`)
- Brittle ancestors-walk in build.rs replaced with `cargo:rustc-link-search` (`f0220b3`)

### Fixed

- hw_diag RNG init, timer measurement, DMA stream race (`07e09cd`)
- USB PHY reset added to `async_cdc_minimal` for st-flash re-enumeration (`f856b6e`)
- Clippy warnings in `extensive_hw_test` (`60b2a6f`)

## [0.1.0] - 2026-04-12

### Added

- `TouchError` enum for structured touch controller error handling
- `DisplayInitError` type for display initialization error reporting
- Display orientation support with landscape timing parameters and backward-compatible API (`c3b5f13`)
- `display_test_rgb565` example for RGB565 pixel format verification (`9e1883b`)
- `async_cdc_minimal` example ported from gm65-scanner (`df993e9`)
- `nt35510_hwtest` example for panel hardware verification (`a08abe1`)
- `embassy_display_bsp_minimal` example for minimal BSP display test (`17b2a16`)
- `async_display_test` example (`a260d72`)
- `display_hybrid` example using BSP `DisplayCtrl::new()` in embassy context (`c545d81`)
- `display_minimal` example with standalone DSI/LTDC (`f4c510f`)
- ITM/SWO example and full rustdoc (`a64458f`)
- `read_chip_model()` tests for FT6X06 (`80e287e`)
- Comprehensive docs, known-issues, and cross-references to sync BSP (`b5156e6`)
- 3-way display init comparison (sync vs async vs embassy) (`9aa6c5e`)

### Changed

- Consolidated examples from 29 to 8 dual-purpose examples (`5496d4b`)
- Migrated display examples from Rgb565 to Rgb888 pixel format (`eb2fda3`)
- Updated nt35510 dependency to git rev ea1ac3a (`24c2a9f`)
- Replaced hardcoded panel init commands with nt35510 crate calls (`2eba34b`)
- Replaced raw DSI/LTDC init with embassy DsiHost + Ltdc drivers (`8170db3`)
- Decoupled `defmt` feature from production builds (opt-in only) (`c136f11`)

### Fixed

- Corrected FLASH LENGTH from 1024K to 2048K for STM32F469NIHx (`5d8abb1`)
- Resolved build errors and warnings across diagnostic examples (`e4131af`)
- DSI config alignment with sync HAL and DsiHostAdapter raw read (`72f2357`)
- Rgb888 migration with typed register accessors and DSI timing alignment (`c95444e`)
- CFBLL+7 off-by-one, GCR reserved bit, DCCR black pixel issues (`f54c992`)
- PLLSAI pixel clock derivation and APB2RSTR/AHB1RSTR address corrections (`8ea44b5`, `70d047e`)
- DSI init bugs: VMCR bit positions, DSI timing, LTDC DEN, CMCR LP/HS mode switching (`972998f`)
- USB CDC stability: RTT non-blocking mode, echo cleanup, stress test fixes (`373a9ae`)
- LTDC timing H/V field swap (root cause of fractal noise) (`330d0e5`)
- L1BFCR BF2 constant blending and init reorder to match sync HAL (`0b85a0f`)
- 48 MHz clock enable at 180 MHz via PLLSAI_Q with build.rs for Rust 1.92 (`c136f11`)

## [0.0.1] - 2026-03-01

Initial fork from Amperstrand/embassy-stm32f469i-disco.

### Added

- Display support via DSI/LTDC with NT35510 panel
- SDRAM controller (16 MB via FMC)
- FT6X06 capacitive touch via I2C1
- Basic examples: blink, display_blinky, test_usb_cdc, test_usb_cdc_stress
- Board auto-detection with `BoardHint::Auto` and `BoardHint::ForceNt35510`
