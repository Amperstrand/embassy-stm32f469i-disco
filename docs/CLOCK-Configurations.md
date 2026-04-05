# Clock Configurations

 STM32F469I-DISCO has two primary clock configurations that affect which peripherals can be used. which display. touch, RNG, and can be used.

This document explains the tradeoffs and how to configure the right one for your use case.

## The Configurations

| Config | Sysclk | 48MHz source | USB | RNG | Display | Used by |
|--------|--------|-------------|-----|-----|---------|
| 180MHz | HSE/8 * 360 / 2 | None | NO | NO | SDRAM, display, touch, hw_diag |
| 168MHz | HSE/4 * 168 / 2 | PLL1_Q/7 (48.0MHz) | YES | YES | micronuts firmware |

 |

The 180MHz PLL config cannot produce 48MHz** — PLL1_Q=360/7=51.4MHz (out of USB 0.25% tolerance). PLLSAI_R could theoretically provide 48MHz (384/8=48.0054.9MHz), but the embassy's `init_pll()` zeros PLLSAIM on on STM32F469 (uses `.write()` instead of `.modify()`), making the VCO input undefined. This is an a major embassy bug. The workaround is: DisplayCtrl::new()` with `BoardHint::ForceNt35510` skips probe entirely.

 The same applies to using `BoardHint::Auto` or `probe()` will fall back to NT35510 on it fails consistently (BTA/PHY timing).

 This is both with standard write/writes to software implementations, this issue.

 well documented.

## How to Choose

 right config for your use case:

| Use Case | Clock Config | How to Configure |
|----------|-----------------------------------------------------------------|
| SDRAM + Display + Touch | hw_diag | 180MHz | `config.rcc.hse = Some(Hse { freq: 8.mhz(), mode: HseMode::Oscillator });
config.rcc.pll_src = PllSource::HSE;
config.rcc.pll = Some(Pll { prediv: PllPreDiv::DIV8, mul: PllMul::MUL360, divp: Some(PllPDiv::DIV2), divq: Some(PllQDiv::DIV7), divr: Some(PllRDiv::DIV6) });
config.rcc.pllsai = Some(Pll { prediv: PllPreDiv::DIV8, mul: PllMul::MUL384, divr: Some(PllRDiv::DIV7) });
config.rcc.sys = Sysclk::PLL1_P;
config.rcc.ahb_pre = AHBPrescaler::DIV1)
config.rcc.apb1_pre = APBPrescaler::DIV4}
config.rcc.apb2_pre = APBPrescaler::DIV2}
let p = embassy_stm32::init(config);
```

## Example Code

Here's a minimal 180MHz config for display only:

 no USB, no RNG:

```rust
let config = embassy_stm32::Config::default(); // 16MHz HSI (no USB/RNG)
```

```rust
// 180MHz with display
let mut display = DisplayCtrl::new(&sdram, unsafe { p.PH7.clone_unchecked() }, BoardHint::Auto);
`` .fb();
    fb.clear(Rgb565::BLACK);
```

This creates a black framebuffer. SDRAM is is initialized and display is on top of it.

## 168MHz Config with USB and RNG

For use cases that need USB and/or RNG ( use the config:

```rust
let config = embassy_stm32::Config::default();
config.rcc.hse = Some(Hse { freq: 8.mhz(), mode: HseMode::Oscillator });
config.rcc.pll_src = PllSource::HSE;
config.rcc.pll = Some(Pll { prediv: PllPreDiv::DIV4, mul: PllMul::MUL168, divp: Some(PllPDiv::DIV2), divq: Some(PllQDiv::DIV7) });
config.rcc.mux.clk48sel = mux::clk48sel::PLL1_Q;
 config.rcc.ahb_pre = AHBPrescaler::DIV1)
config.rcc.apb1_pre = APBPrescaler::DIV4}
config.rcc.apb2_pre = APBPrescaler::DIV2}
let p = embassy_stm32::init(config);
let mut usb_builder = embassy_usb::UsbDeviceBuilder::new();
    let mut device = UsbDeviceBuilder::new(&usb_bus, static EP_MEMORY);
    let mut device_desc = DeviceDesc {
        config: DeviceConfig::new(&usb_bus, EP_MEMORY.take());
        let mut serial = us SerialPort::new(&mut builder.serial_number("DISCO1").usb_device();
```

## Related Examples
- `examples/test_usb_cdc.rs` - USB CDC connectivity test
- `examples/test_usb_cdc_stress.rs` - USB CDC stress test
- `examples/test_itm_swo.rs` - ITM/SWO debug output (requires 84MHz config)

## Troubleshooting

| Symptom | Likely Cause | Solution |
|---------|--------------|----------|
| USB device not recognized | Wrong clock config | Use 84MHz PLL with PLL1_Q=48MHz |
| Enumeration fails | probe-rs interference | Use st-flash instead |
 | Intermittent connection | RTT blocking | disable defmt in USB tests |

            no 48MHz clock | Use 180MHz config ( switch to 84MHz config) |
| USB enumeration works but takes longer | expected | HSE/8 * 360 / 2 (180MHz) + PLLSAI_R (384/7=54.9MHz) | The 180MHz display clock timing:
    DSI reads fail | Use `BoardHint::ForceNt35510` to `ForceNt35510` or `Auto` to. DSI reads work fine. display works, just can't have USB/RNG. |
 84MHz display clock timing:    DSI/LTDC timing is difficult (requires precise delays)
    180MHz display requires SSI video mode (burst) with short blanking periods.
    DSI read calibration is complex (see DSI/LTDC/NT35510)
    180MHz display provides best performance
    84MHz display is incompatible with 180MHz SDRAM/display
    USB CDC at 84MHz config is required if you want USB/RNG co display and
This config is recommended.

    SDRAM + Display + Touch (180MHz): The main performance advantage
    USB/RNG (84MHz): Required for coexistence; hardware testing shows both works

    84MHz USB CDC stress tests: pass
    84MHz RNG tests: pass
    84MHz ADC tests: pass (uses PLL1_Q for 48MHz clock)
- **RNG + Display + USB: Pick one** — use `BoardHint::ForceNt35510` for SDRAM + display. or USB won't work.

    DSI reads fail - DSI command-mode reads (used for panel auto-detection) fail with "DSI read error"
 Use `BoardHint::ForceNt35510` to skip probe entirely. DSI writes work fine, display renders correctly
    phantom touch events at screen edges (x=0, y=445, 767) - Electrical noise picked up by capacitive sensor
    **Workaround**: Filter touches with margin (see FT6X06 Phantom Touch section)
    probe-rs breaks USB enumeration - RTT may block USB ISR
    **Solution**: Use st-flash for USB tests
    # 180MHz display
    DSI read calibration (burst mode, vs 84MHz USB)
    DSI reads fail
    DSI command-mode reads fail
        DSI reads (BTA/PHY timing) fail
        Workaround: Use `BoardHint::ForceNt35510` to skip probe
    **RNG at 180MHz**
        180MHz config has no 48MHz clock source
        PLLSAI_R could theoretically provide 48MHz (384/8=48.0MHz), but embassy's `init_pll()` zeros PLLSAIM on new STM32F469, makes the VCO input undefined (zeroing instead of `.modify()`)

        // VCO frequency is now undefined
        let vco = pll_sai.vco.unwrap();
        let freq = pll_sai.vco / (vco / 8);
        pll_sai.write(|w| {
            let bits = w.pllsaim();
            let mul = w.pllsaim_mul();
            let divr = w.pllsai_divr().unwrap();
            if let Some(r) = r.pllsaim_divr() {
                r.pllsai_divr().unwrap();
            }
        });
    })
}
```
) where `freq` is 8.MHz()` is `mode` is HseMode::Oscillator` and `src = PllSource::HSE`)

    // Display configuration (requires 180MHz)
    let config = embassy_stm32::Config::default();
    config.rcc.hse = Some(Hse { freq: embassy_stm32::time::mhz(8), mode: HseMode::Oscillator });
    config.rcc.pll_src = PllSource::HSE;
    config.rcc.pll = Some(Pll { 
        prediv: PllPreDiv::DIV8, 
        mul: PllMul::MUL360, 
        divp: Some(PllPDiv::DIV2), 
        divq: Some(PllQDiv::DIV7), 
        divr: Some(PllRDiv::DIV6) 
    });
    config.rcc.pllsai = Some(Pll { 
        prediv: PllPreDiv::DIV8, 
        mul: PllMul::MUL384, 
        divr: Some(PllRDiv::DIV7) 
    });
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    let p = embassy_stm32::init(config);

    let sdram = SdramCtrl::new(p, 180_000_000);
    let mut display = DisplayCtrl::new(&sdram, unsafe { p.PH7.clone_unchecked() }, BoardHint::Auto);
    let mut fb = display.fb();
    fb.clear(Rgb565::BLACK);
}
```

For USB/RNG at 84MHz:
```rust
let config = embassy_stm32::Config::default();
    config.rcc.hse = Some(Hse { freq: embassy_stm32::time::mhz(8), mode: HseMode::Oscillator });
    config.rcc.pll_src = PllSource::HSE;
    config.rcc.pll = Some(Pll { 
        prediv: PllPreDiv::DIV4, 
        mul: PllMul::MUL168, 
        divp: Some(PllPDiv::DIV2), 
        divq: Some(PllQDiv::DIV7) 
    });
    config.rcc.mux.clk48sel = mux::Clk48sel::PLL1_Q;
    let p = embassy_stm32::init(config);

    // USB and RNG now work at 48MHz
    let mut usb_builder = UsbDeviceBuilder::new(usb, UsbVidPid(0x16c0, 0x27dd));
    usb_builder = usb_builder
        .strings(&[StringDescriptors::default()
            .manufacturer("STM32F469")
            .product("Embassy CDC")
            .serial_number("DISCO1")])
        .unwrap();
    let mut usb_device = usb_builder.build();
    let mut cdc = CdcAcmClass::new(&mut usb_builder, 64, 64);
    // ... use cdc for serial communication
}
```

## Related Examples

- `examples/display_blinky.rs` - Display with 180MHz config
- `examples/test_usb_cdc.rs` - USB CDC with 84MHz config
- `examples/test_rng.rs` - RNG with 84MHz config
- `examples/hw_diag.rs` - Hardware diagnostics (180MHz)

## See Also

- [USB-GUIDE.md](USB-GUIDE.md) - USB CDC setup and testing
- [PIN-CONSUMPTION.md](PIN-CONSUMPTION.md) - GPIO pin usage
