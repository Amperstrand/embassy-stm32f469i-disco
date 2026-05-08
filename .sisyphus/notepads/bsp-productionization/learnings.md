# Learnings — bsp-productionization

## [2026-05-08] Session start

### Codebase state
- `src/display.rs` is a 1332-line monolith containing SdramCtrl, DisplayCtrl, DSI, LTDC, NT35510 init, FramebufferView
- `src/touch.rs` 116 lines, hardcoded to `embassy_stm32::i2c::Error`
- `src/usb.rs` 103 lines, `reset_usb_phy()` — DO NOT TOUCH register sequence
- `src/clock.rs` — DO NOT TOUCH presets (hardware-verified)
- `build.rs` uses ancestors-walk to find cortex-m-rt OUT_DIRs — brittle
- `.cargo/config.toml` emits `-Tlink.x -Tdefmt.x` — likely duplicate with cortex-m-rt
- `Cargo.toml` has 6 keywords (max 5), nt35510 on git rev, no categories
- CI is minimal — no feature matrix, no clippy gate, no doc gate

### Key constraints
- NEVER change DSI/LTDC/NT35510 register sequences
- NEVER change clock.rs presets
- NEVER change reset_usb_phy() register sequence
- NEVER change memory.x
- USB CDC tests use st-flash (NOT probe-rs)
- Non-USB HW tests use probe-rs run

### Pin map (from AGENTS.md)
- LEDs: PG6 (green), PD4 (orange), PD5 (red), PK3 (blue) — active LOW
- Touch I2C1: PB8 (SCL), PB9 (SDA)
- USB OTG FS: PA11 (DM), PA12 (DP)
- User button: PA0
- USART6 (scanner): PG14 (TX), PG9 (RX) — NOT consumed by SDRAM

## [2026-05-08] Task 1: Cargo.toml metadata cleanup

### Changes made
- Trimmed `keywords` from 6 to 5 items: removed "discovery" (kept: embedded, stm32, embassy, async, no-std)
- Added `rust-version = "1.94.0"` (matches current toolchain)
- Added `documentation = "https://docs.rs/embassy-stm32f469i-disco"`
- Added `homepage = "https://github.com/Amperstrand/embassy-stm32f469i-disco"`
- Polished `description` to: "Embassy async BSP for the STM32F469I-Discovery board (display, SDRAM, touch, USB)" (78 chars)
- Reordered `categories` for consistency: embedded, no-std, hardware-support (all valid crates.io slugs)
- Verified `license = "MIT OR Apache-2.0"` matches README (no LICENSE file in repo)

### Verification
- Build succeeded: `cargo build --target thumbv7em-none-eabihf` in 0.44s
- Metadata validated via `cargo metadata --format-version 1 --no-deps`
- Evidence saved to `.sisyphus/evidence/task-1-metadata-valid.txt` and `.sisyphus/evidence/task-1-build.txt`

### Lessons
- crates.io requires ≤ 5 keywords (had 6, blocking `cargo publish`)
- `rust-version` prevents accidental use of older compilers
- `documentation` field auto-links to docs.rs
- `homepage` and `repository` can point to same URL (common for GitHub projects)
- `MIT OR Apache-2.0` is a valid SPDX expression (dual license)
- No LICENSE file in repo, but README declares license and Cargo.toml matches — acceptable for publishing

## [2026-05-08] Task 2: Replace ancestors-walk in build.rs

### Changes made
- Replaced 97-line `build.rs` (ancestor directory walking to find cortex-m-rt OUT_DIRs) with 7-line standard pattern
- New build.rs: copies `memory.x` to own OUT_DIR, emits `cargo:rustc-link-search=$OUT_DIR`
- Added clarifying comments to `.cargo/config.toml` explaining why `-Tlink.x` and `-Tdefmt.x` are needed there

### Key finding: old build.rs comment was WRONG
- cortex-m-rt 0.7.5 emits `cargo:rustc-link-search` but does NOT emit `cargo:rustc-link-arg=-Tlink.x`
- defmt 1.0.1 similarly only emits `cargo:rustc-link-search`, NOT the `-T` flag
- The `-Tlink.x` and `-Tdefmt.x` in `.cargo/config.toml` are the ONLY source of these flags — NOT duplicates
- Old comment "cortex-m-rt already emits -Tlink.x -Tdefmt.x via cargo:rustc-link-arg" was incorrect

### How memory.x discovery works (post-change)
1. BSP build.rs copies `memory.x` to BSP's OUT_DIR, emits `cargo:rustc-link-search=$OUT_DIR`
2. cortex-m-rt build.rs generates `link.x` in its OUT_DIR, emits `cargo:rustc-link-search=$OUT_DIR`
3. defmt build.rs generates `defmt.x` in its OUT_DIR, emits `cargo:rustc-link-search=$OUT_DIR`
4. `.cargo/config.toml` provides `-Tlink.x -Tdefmt.x` (tells linker to USE these as scripts)
5. Linker searches all registered paths, finds `link.x`, `defmt.x`, and `memory.x`
6. `link.x` includes `INCLUDE memory.x` — linker finds it via the BSP's link-search path

### Verification
- All 13 examples build successfully (clean build)
- `-Tlink.x` appears exactly once per final link step (no duplication)
- `memory.x` correctly placed at `target/.../build/embassy-stm32f469i-disco-*/out/memory.x`
- Evidence: `.sisyphus/evidence/task-2-examples-build.txt`, `.sisyphus/evidence/task-2-link-args.txt`

### Lessons
- The ancestors-walk pattern was a workaround for an old cargo bug (pre-1.0) — no longer needed
- `cargo:rustc-link-search` is additive — all build scripts' OUT_DIRs are searched by the linker
- cortex-m-rt and defmt only emit link-search, NOT link-arg — the `-T` flags must come from config.toml
- This is a recurring confusion point across Amperstrand projects (gm65-scanner, micronuts, microfips)

## [2026-05-08] Task 4: Remove broad clippy allows and fix warnings

### Changes made
- Removed 4 broad `#![allow(clippy::*)]` attributes from `src/lib.rs`:
  - `#![allow(clippy::unnecessary_cast)]`
  - `#![allow(clippy::identity_op)]`
  - `#![allow(clippy::single_match)]`
  - `#![allow(clippy::new_without_default)]`
- Fixed pre-existing compilation error in `verify_quarter_fill()`: `F::Color::new()` doesn't exist for `RgbColor` trait
  - Solution: Use `RgbColor` trait constants (`RED`, `GREEN`, `BLUE`, `YELLOW`) instead of custom RGB values
- Fixed clippy `needless_range_loop` warnings in `verify_quarter_fill()`: converted `for q in 0..4` to `for (q, &color) in colors.iter().enumerate()`

### Pre-existing bugs discovered
1. **`verify_quarter_fill()` compilation error**: Code tried to call `F::Color::new(0xFF, 0x00, 0x00)` which doesn't exist for `RgbColor` trait. This was hidden by broad clippy allows — the code never compiled cleanly.
   - **Root cause**: `RgbColor` trait provides color constants (`RED`, `GREEN`, etc.) but not a generic constructor
   - **Impact**: Feature-gated method (`#[cfg(feature = "defmt")]`) was never tested on hardware
   - **Fix**: Use `RgbColor` trait constants instead of custom RGB values

### Verification
- `cargo clippy --target thumbv7em-none-eabihf --all-features --lib -- -D warnings` exits 0
- No remaining `#![allow(clippy::*)]` in `src/lib.rs`
- Evidence saved to `.sisyphus/evidence/task-4-clippy.txt` (empty = success)
- Evidence saved to `.sisyphus/evidence/task-4-no-allows.txt` (contains "no broad allows")

### Lessons
- Broad `#![allow(clippy::*)]` attributes hide compilation errors, not just warnings
- `RgbColor` trait from embedded-graphics provides constants (`RED`, `GREEN`, `BLUE`, `YELLOW`, etc.) but no generic `new()` method
- Clippy's `needless_range_loop` catches anti-patterns like `for i in 0..n` where `i` is only used for array indexing
- Feature-gated code (`#[cfg(feature = "defmt")]`) can rot silently if not compiled regularly
- CI should run clippy with `-D warnings` to prevent such bugs from accumulating

## [2026-05-08] Task 5: Extract SDRAM module

### Changes made
- Moved `SdramCtrl`, `SDRAM_SIZE_BYTES`, `EmbassyFmc`, `sdram_pin`, and FMC pin/timing setup from `src/display.rs` into new `src/sdram.rs`
- Kept display-only items (`FB_HEIGHT`, `FB_WIDTH`, `FB_SIZE`, display formats, DSI/LTDC logic) in `src/display.rs`
- Preserved existing public API by re-exporting SDRAM symbols from `display` and crate root
- Added rustdoc to moved public SDRAM items in the new module

### Verification
- `lsp_diagnostics` clean for `src/sdram.rs`, `src/display.rs`, and `src/lib.rs`
- `cargo build --target thumbv7em-none-eabihf --examples` exits 0
- `cargo clippy --target thumbv7em-none-eabihf --all-features --lib -- -D warnings` exits 0
- Evidence saved to `.sisyphus/evidence/task-5-build.txt`, `.sisyphus/evidence/task-5-clippy.txt`, and `.sisyphus/evidence/task-5-pub-api.txt`

### Lessons
- Pure structural module splits can preserve downstream paths by re-exporting moved items from the original module
- `BusyDelay` must remain duplicated across display/SDRAM modules unless delay concerns are extracted separately in a later task
- `src/display.rs` depends on `SdramCtrl` only through framebuffer allocation, so the split is low-risk when FMC helpers move with the controller
