# Known Issues

This document consolidates hardware-specific known issues for the STM32F469I-DISCO board. These issues affect both the sync BSP ([stm32f469i-disc](https://github.com/Amperstrand/stm32f469i-disc)) and this async BSP ([embassy-stm32f469i-disco](https://github.com/Amperstrand/embassy-stm32f469i-disco)).

## DSI Panel Auto-Detection

**Issue:** DSI reads may fail during panel auto-detection (3/3 retries).

**Workaround:** Use `BoardHint::ForceNt35510` or `BoardHint::ForceOtm8009a` to skip the probe and use a known panel type.

```rust
// Async BSP
let display = DisplayCtrl::new(&sdram, p.PH7, BoardHint::ForceNt35510);

// Sync BSP
let display = DisplayCtrl::new(p.DSI, p.LTDC, &sdram, BoardHint::ForceNt35510);
```

## FT6X06 Phantom Touches

**Issue:** The FT6X06 touch controller reports phantom touches at screen edges.

**Workaround:** Filter edge touches with a margin:

```rust
const MARGIN: u16 = 3;
if point.x < MARGIN || point.x > (FB_WIDTH - MARGIN) || 
   point.y < MARGIN || point.y > (FB_HEIGHT - MARGIN) {
    // Ignore phantom touch
}
```

Consumers must apply this filter themselves.

## probe-rs USB Timing

**Issue:** probe-rs halts the CPU periodically for RTT reads, breaking USB timing and causing enumeration failures.

**Workaround:** Use `st-flash` instead of `probe-rs` for USB CDC testing:

```bash
# Build USB test
cargo build --release --example test_usb_cdc_stress --target thumbv7em-none-eabihf

# Flash with st-flash (NOT probe-rs)
st-flash write target/thumbv7em-none-eabihf/release/examples/test_usb_cdc_stress.bin 0x08000000

# Connect USB cable and test
python3 tests/usb_cdc_stress.py --port /dev/ttyACM0
```

See [USB-GUIDE.md](USB-GUIDE.md) for full USB CDC setup instructions.

## SDIO

**Status:** Not tested on this board.

The `sdio_raw_test` example exists but has not been verified on hardware. SDIO is out of scope for the primary wallet use case.

## Related Documentation

- [USB Guide](USB-GUIDE.md) - USB OTG FS setup and CDC-ACM
- [Hardware Test Plan](HARDWARE-TEST-PLAN.md) - Test procedures
- [Pin Consumption](PIN-CONSUMPTION.md) - Which pins SDRAM consumes
- [Clock Configurations](CLOCK-Configurations.md) - PLL settings for different use cases
