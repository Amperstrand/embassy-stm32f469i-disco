# Changelog

All notable changes to this project will be documented in this file.

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

## [0.0.1] - 2026-03-01 (initial fork)

### Added
- Initial BSP fork from Amperstrand/embassy-stm32f469i-disco
- Display support via DSI/LTDC with NT35510 panel
- SDRAM controller (16 MB via FMC)
- FT6X06 capacitive touch via I2C1
- Basic examples: blink, display_blinky, test_usb_cdc, test_usb_cdc_stress
- Board auto-detection with `BoardHint::Auto` and `BoardHint::ForceNt35510`
