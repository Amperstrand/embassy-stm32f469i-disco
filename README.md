# embassy-stm32f469i-disco

Board support package for the [STM32F469I-Discovery](https://www.st.com/en/evaluation-tools/stm32f469i-discovery.html) board, built on the [Embassy](https://embassy.dev/) async framework.

## Quick Start

```bash
# Install ARM target
rustup target add thumbv7em-none-eabihf

# Build
cargo build --target thumbv7em-none-eabihf

# Flash and run
probe-rs run --chip STM32F469NIHx --target thumbv7em-none-eabihf --example blink
```

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `display` | DSI/LTDC display via NT35510, embedded-graphics support | Yes |
| `touch` | FT6X06 capacitive touch via I2C1 | Yes |

```toml
[dependencies]
embassy-stm32f469i-disco = { git = "https://github.com/Amperstrand/embassy-stm32f469i-disco", branch = "main" }
```

## Hardware

- **MCU**: STM32F469NIH6 (Cortex-M4F, 180 MHz)
- **Display**: 480x800 RGB565 LCD via DSI/LTDC (NT35510)
- **SDRAM**: 16 MB via FMC (IS42S32400F-6BL)
- **Touch**: FT6X06 capacitive touch via I2C1
- **USB**: OTG FS (CDC-ACM)
- **LEDs**: 4 user LEDs (green, orange, red, blue)

## Examples

### Blink an LED

```rust
let p = embassy_stm32::init(embassy_stm32::Config::default());
let mut led = embassy_stm32::gpio::Output::new(
    p.PG6,
    embassy_stm32::gpio::Level::Low,
    embassy_stm32::gpio::Speed::Low,
);
loop {
    Timer::after(embassy_time::Duration::from_secs(1)).await;
    led.toggle();
}
```

### Display + SDRAM

Display and SDRAM require the 180 MHz PLL configuration:

```rust
let mut config = embassy_stm32::Config::default();
config.rcc.hse = Some(Hse { freq: embassy_stm32::time::mhz(8), mode: HseMode::Oscillator });
config.rcc.pll_src = PllSource::HSE;
config.rcc.pll = Some(Pll { prediv: PllPreDiv::DIV8, mul: PllMul::MUL360, divp: Some(PllPDiv::DIV2), divq: Some(PllQDiv::DIV7), divr: Some(PllRDiv::DIV6) });
config.rcc.pllsai = Some(Pll { prediv: PllPreDiv::DIV8, mul: PllMul::MUL384, divr: Some(PllRDiv::DIV7) });
config.rcc.sys = Sysclk::PLL1_P;
config.rcc.ahb_pre = AHBPrescaler::DIV1;
config.rcc.apb1_pre = APBPrescaler::DIV4;
config.rcc.apb2_pre = APBPrescaler::DIV2;
let p = embassy_stm32::init(config);

let sdram = SdramCtrl::new(&mut unsafe { embassy_stm32::Peripherals::steal() }, 180_000_000);
let mut display = DisplayCtrl::new(&sdram, unsafe { p.PH7.clone_unchecked() }, BoardHint::Auto);
let mut fb = display.fb();
fb.clear(Rgb565::BLACK);
```

See `examples/display_blinky.rs` for a complete working example.

### Touch

```rust
let mut i2c = embassy_stm32::i2c::I2c::new_blocking(
    p.I2C1, p.PB8, p.PB9,
    embassy_stm32::i2c::Config::default(),
);
let touch = TouchCtrl::new();
if touch.td_status(&mut i2c).unwrap_or(0) > 0 {
    if let Ok(point) = touch.get_touch(&mut i2c) {
        // point.x, point.y are u16 coordinates (0-479, 0-799)
    }
}
```

### USB CDC

USB requires a separate 84 MHz clock config (incompatible with display):

```rust
config.rcc.pll = Some(Pll { prediv: PllPreDiv::DIV4, mul: PllMul::MUL168, divp: Some(PllPDiv::DIV2), divq: Some(PllQDiv::DIV7) });
config.rcc.mux.clk48sel = mux::Clk48sel::PLL1_Q;
```

See `examples/test_usb_cdc_stress.rs` for a complete example.

## API

| Export | Type | Description |
|--------|------|-------------|
| `DisplayCtrl` | struct | DSI/LTDC display controller |
| `FramebufferView` | struct | DrawTarget for embedded-graphics |
| `SdramCtrl` | struct | FMC SDRAM controller (16 MB) |
| `TouchCtrl` | struct | FT6X06 touch controller |
| `BoardHint` | enum | `Auto`, `ForceNt35510`, `ForceOtm8009a` |
| `FB_HEIGHT` | const | 800 |
| `FB_WIDTH` | const | 480 |

## Known Issues

- **DSI reads** may fail during panel auto-detection. Use `BoardHint::ForceNt35510` to skip probe.
- **probe-rs** breaks USB enumeration. Use `st-flash` for USB CDC testing.
- **FT6X06** reports phantom touches at screen edges — consumers should filter with a margin.

See [AGENTS.md](AGENTS.md) for detailed hardware evidence and upstream interaction policy.

## License

MIT OR Apache-2.0
