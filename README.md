# embassy-stm32f469i-disco

Board support package for the STM32F469I-Discovery development board, built on the Embassy async framework.

## Hardware

- MCU: STM32F469NIH6 (ARM Cortex-M4F, 180MHz)
- Display: 480x800 RGB565 LCD via DSI/LTDC (NT35510 controller)
- SDRAM: 16MB via FMC (IS42S32400F-6BL)
- Touch: FT6X06 capacitive touch via I2C

## Features

| Feature | Description |
|---------|-------------|
| `display` | DSI/LTDC/NT35510 display with SDRAM framebuffer |
| `touch` | FT6X06 touch controller |

Both features are enabled by default.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
embassy-stm32f469i-disco = { git = "https://github.com/Amperstrand/embassy-stm32f469i-disco" }
```

### Display

```rust,ignore
use embassy_stm32f469i_disco::{SdramCtrl, DisplayCtrl};

// Initialize SDRAM (16MB)
let sdram = SdramCtrl::new(&mut p, 180_000_000);

// Initialize display (480x800 RGB565)
let mut display = DisplayCtrl::new(&sdram, p.PH7);
let mut fb = display.fb();

// Use embedded-graphics
use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::Rgb565;

fb.clear(Rgb565::BLACK);
```

### Touch

```rust,ignore
use embassy_stm32f469i_disco::TouchCtrl;

let touch = TouchCtrl::new();
let count = touch.td_status(&mut i2c)?;
let point = touch.get_touch(&mut i2c)?;
```

## License

MIT OR Apache-2.0
