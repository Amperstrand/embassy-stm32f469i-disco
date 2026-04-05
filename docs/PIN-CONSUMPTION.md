# GPIO Pin Consumption - STM32F469I-DISCO

## Overview

The STM32F469I-DISCO development board routes many GPIO pins to the on-board 16MB SDRAM memory. This consumes approximately 50 pins across six GPIO ports (C, D, E, F, G, H, I), leaving limited pins available for other peripherals.

Understanding which pins are consumed and which remain available is critical when designing applications that need to use additional peripherals like SDIO, touch input, or custom GPIO functions.

### Key Takeaway

After SDRAM initialization, the Embassy BSP provides:
- `SdramCtrl` for memory controller use
- PH7 for LCD reset
- USART6 pins (PG14/PG9) available for QR scanner or other UART use

## Pin Consumption by Port

### Port A

| Pin | Function | Notes |
|-----|----------|-------|
| PA0 | User Button | Active high, use pull-down |
| PA9 | USART1 TX | Test UART TX; not routed to onboard peripherals |
| PA10 | USART1 RX | Test UART RX; not routed to onboard peripherals |
| PA11 | USB DM | USB OTG FS |
| PA12 | USB DP | USB OTG FS |

**Available:** PA1-PA8, PA13-PA15 (not routed to onboard peripherals)

### Port B

| Pin | Function | Notes |
|-----|----------|-------|
| PB8 | I2C1 SCL | Touch controller I2C clock |
| PB9 | I2C1 SDA | Touch controller I2C data |

**Available:** PB0-PB7, PB10-PB15 (not routed to onboard peripherals)

### Port C

| Pin | SDRAM | Other | Notes |
|-----|-------|-------|-------|
| PC0 | SDNWE | - | SDRAM write enable |
| PC1 | - | Touch INT | FT6X06 interrupt (active low) |
| PC8 | - | SDIO D0 | SD card data 0 |
| PC9 | - | SDIO D1 | SD card data 1 |
| PC10 | - | SDIO D2 | SD card data 2 |
| PC11 | - | SDIO D3 | SD card data 3 |
| PC12 | - | SDIO CLK | SD card clock |

**Available:** PC2-PC7, PC13-PC15

### Port D

| Pin | SDRAM | Other | Notes |
|-----|-------|-------|-------|
| PD0 | D2 | - | SDRAM data bit 2 |
| PD1 | D3 | - | SDRAM data bit 3 |
| PD2 | - | SDIO CMD | SD card command |
| PD4 | - | LED LD2 | Orange LED |
| PD5 | - | LED LD3 | Red LED |
| PD8 | D13 | - | SDRAM data bit 13 |
| PD9 | D14 | - | SDRAM data bit 14 |
| PD10 | D15 | - | SDRAM data bit 15 |
| PD14 | D0 | - | SDRAM data bit 0 |
| PD15 | D1 | - | SDRAM data bit 1 |

**Available:** PD3, PD6, PD7, PD11-PD13

### Port E

| Pin | SDRAM | Other | Notes |
|-----|-------|-------|-------|
| PE0 | NBL0 | - | SDRAM byte lane 0 enable |
| PE1 | NBL1 | - | SDRAM byte lane 1 enable |
| PE7 | D4 | - | SDRAM data bit 4 |
| PE8 | D5 | - | SDRAM data bit 5 |
| PE9 | D6 | - | SDRAM data bit 6 |
| PE10 | D7 | - | SDRAM data bit 7 |
| PE11 | D8 | - | SDRAM data bit 8 |
| PE12 | D9 | - | SDRAM data bit 9 |
| PE13 | D10 | - | SDRAM data bit 10 |
| PE14 | D11 | - | SDRAM data bit 11 |
| PE15 | D12 | - | SDRAM data bit 12 |

**Available:** PE2-PE6

### Port F

| Pin | SDRAM | Other | Notes |
|-----|-------|-------|-------|
| PF0 | A0 | - | SDRAM address bit 0 |
| PF1 | A1 | - | SDRAM address bit 1 |
| PF2 | A2 | - | SDRAM address bit 2 |
| PF3 | A3 | - | SDRAM address bit 3 |
| PF4 | A4 | - | SDRAM address bit 4 |
| PF5 | A5 | - | SDRAM address bit 5 |
| PF11 | SDNRAS | - | SDRAM row address strobe |
| PF12 | A6 | - | SDRAM address bit 6 |
| PF13 | A7 | - | SDRAM address bit 7 |
| PF14 | A8 | - | SDRAM address bit 8 |
| PF15 | A9 | - | SDRAM address bit 9 |

**Available:** PF6-PF10

### Port G

| Pin | SDRAM | Other | Notes |
|-----|-------|-------|-------|
| PG0 | A10 | - | SDRAM address bit 10 |
| PG1 | A11 | - | SDRAM address bit 11 |
| PG4 | BA0 | - | SDRAM bank address 0 |
| PG5 | BA1 | - | SDRAM bank address 1 |
| PG6 | - | LED LD1 | Green LED |
| PG8 | SDCLK | - | SDRAM clock |
| PG9 | - | USART6 RX | Available; alternate function for USART6 RX |
| PG14 | - | USART6 TX | Available; alternate function for USART6 TX |
| PG15 | SDNCAS | - | SDRAM column address strobe |

**Available:** PG2, PG3, PG7, PG10-PG13

### Port H

| Pin | SDRAM | Other | Notes |
|-----|-------|-------|-------|
| PH2 | SDCKE0 | - | SDRAM clock enable |
| PH3 | SDNE0 | - | SDRAM chip select |
| PH7 | - | LCD RST | LCD reset (output) |
| PH8 | D16 | - | SDRAM data bit 16 |
| PH9 | D17 | - | SDRAM data bit 17 |
| PH10 | D18 | - | SDRAM data bit 18 |
| PH11 | D19 | - | SDRAM data bit 19 |
| PH12 | D20 | - | SDRAM data bit 20 |
| PH13 | D21 | - | SDRAM data bit 21 |
| PH14 | D22 | - | SDRAM data bit 22 |
| PH15 | D23 | - | SDRAM data bit 23 |

**Available:** PH0, PH1, PH4-PH6

### Port I

| Pin | SDRAM | Other | Notes |
|-----|-------|-------|-------|
| PI0 | D24 | - | SDRAM data bit 24 |
| PI1 | D25 | - | SDRAM data bit 25 |
| PI2 | D26 | - | SDRAM data bit 26 |
| PI3 | D27 | - | SDRAM data bit 27 |
| PI4 | NBL2 | - | SDRAM byte lane 2 enable |
| PI5 | NBL3 | - | SDRAM byte lane 3 enable |
| PI6 | D28 | - | SDRAM data bit 28 |
| PI7 | D29 | - | SDRAM data bit 29 |
| PI9 | D30 | - | SDRAM data bit 30 |
| PI10 | D31 | - | SDRAM data bit 31 |

**Available:** PI8, PI11

### Port K

| Pin | Function | Notes |
|-----|----------|-------|
| PK3 | LED LD4 | Blue LED |

**Available:** PK0-PK2, PK4-PK7

## SDRAM Pin Categories

### Address Bus (12 pins)

```
A0-A5:  PF0, PF1, PF2, PF3, PF4, PF5
A6-A9:  PF12, PF13, PF14, PF15
A10-A11: PG0, PG1
```

### Bank Select (2 pins)

```
BA0: PG4
BA1: PG5
```

### Data Bus (32 pins)

```
D0-D1:   PD14, PD15
D2-D3:   PD0, PD1
D4-D12:  PE7, PE8, PE9, PE10, PE11, PE12, PE13, PE14, PE15
D13-D15: PD8, PD9, PD10
D16-D23: PH8, PH9, PH10, PH11, PH12, PH13, PH14, PH15
D24-D27: PI0, PI1, PI2, PI3
D28-D29: PI6, PI7
D30-D31: PI9, PI10
```

### Byte Lane Enables (4 pins)

```
NBL0: PE0
NBL1: PE1
NBL2: PI4
NBL3: PI5
```

### Control Signals (6 pins)

```
SDNWE:   PC0   (Write enable)
SDNRAS:  PF11  (Row address strobe)
SDNCAS:  PG15  (Column address strobe)
SDCLK:   PG8   (Clock)
SDCKE0:  PH2   (Clock enable)
SDNE0:   PH3   (Chip select)
```

## Pin Conflict Matrix

| Feature | Required Pins | Conflicts With |
|---------|---------------|----------------|
| SDRAM | PC0, PD0,1,8,9,10,14,15, PE0,1,7-15, PF0-5,11-15, PG0,1,4,5,8,15, PH2,3,8-15, PI0-7,9,10 | Any use of these pins |
| LCD | PH7 | - (returned separately) |
| Touch | PB8, PB9 (I2C), PC1 (INT) | - |
| SDIO | PC8-12, PD2 | - |
| USB FS | PA11, PA12 | - |
| USART1 | PA9 (TX), PA10 (RX) | - (not routed to onboard peripherals) |
| USART6 | PG14 (TX), PG9 (RX) | - (not consumed by SDRAM) |
| User Button | PA0 | - |
| LEDs | PD4, PD5, PG6, PK3 | - |

### Compatible Peripherals

All on-board peripherals can coexist since their pins do not overlap:
- SDRAM + Touch + SDIO + USB + LEDs + Button = All work together

### What Cannot Be Used Together

Nothing on this board conflicts! The BSP design ensures all on-board peripherals use non-overlapping pins.

## Usage Example (Embassy)

```rust
use embassy_stm32::Config;
use embassy_stm469i_disco::{DisplayCtrl, SdramCtrl, TouchCtrl, BoardHint};

// 180MHz config required for SDRAM/display
let mut config = Config::default();
config.rcc.hse = Some(Hse { freq: hz(8_000_000), mode: HseMode::Oscillator });
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

let p = embassy_stm32::init(config);

// Initialize SDRAM
let sdram = SdramCtrl::new(&mut unsafe { embassy_stm32::Peripherals::steal() }, 180_000_000);

// Initialize display (uses PH7 for LCD reset)
let mut display = DisplayCtrl::new(&sdram, unsafe { p.PH7.clone_unchecked() }, BoardHint::Auto);
let mut fb = display.fb();
fb.clear(Rgb565::BLACK);

// Touch uses I2C1 on PB8/PB9
let mut i2c = embassy_stm32::i2c::I2c::new_blocking(
    p.I2C1, p.PB8, p.PB9,
    embassy_stm32::i2c::Config::default(),
);
let touch = TouchCtrl::new();
```

## Pin Summary

| Port | Total Pins | SDRAM | Other Used | Available |
|------|-----------|-------|------------|-----------|
| A | 16 | 0 | 5 (BTN, UART, USB) | 11 |
| B | 16 | 0 | 2 (I2C) | 14 |
| C | 16 | 1 | 6 (Touch, SDIO) | 9 |
| D | 16 | 7 | 3 (SDIO, LED) | 6 |
| E | 16 | 11 | 0 | 5 |
| F | 16 | 11 | 0 | 5 |
| G | 16 | 6 | 3 (LED, UART) | 7 |
| H | 16 | 10 | 1 (LCD) | 5 |
| I | 12 | 10 | 0 | 2 |
| K | 8 | 0 | 1 (LED) | 7 |
| **Total** | **148** | **52** | **21** | **71** |

The SDRAM consumes 52 pins, leaving 96 pins for other uses (75 of which are on ports A-K).
