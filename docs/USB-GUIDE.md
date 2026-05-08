# USB Device Guide (Embassy)

The STM32F469I-DISCO board includes a USB OTG Full-Speed peripheral that can operate in device mode. This guide covers how to use the Embassy BSP's USB support to create USB devices such as CDC-ACM serial ports.

## Overview

- **Peripheral**: USB OTG FS (Full-Speed)
- **Pins**: PA11 (DM), PA12 (DP)
- **Clock Requirement**: 48MHz from PLL1_Q (84MHz config only)

**Important**: USB requires a 48MHz clock which is only available with the 84MHz PLL configuration. The 180MHz config (used for display/SDRAM) cannot produce 48MHz and is incompatible with USB.

## Quick Start

### Clock Configuration

USB requires 84MHz PLL with PLL1_Q providing 48MHz:

```rust
use embassy_stm32::rcc::*;
use embassy_stm32::Config;

let mut config = Config::default();
config.rcc.hse = Some(Hse { freq: hz(8_000_000), mode: HseMode::Oscillator });
config.rcc.pll_src = PllSource::HSE;
config.rcc.pll = Some(Pll { 
    prediv: PllPreDiv::DIV4,   // 8MHz / 4 = 2MHz
    mul: PllMul::MUL168,        // 2MHz * 168 = 336MHz
    divp: Some(PllPDiv::DIV2),  // 336MHz / 2 = 168MHz (sysclk)
    divq: Some(PllQDiv::DIV7),  // 336MHz / 7 = 48MHz (USB clock)
    divr: Some(PllRDiv::DIV6),  // 336MHz / 6 = 56MHz (DSI)
});
config.rcc.mux.clk48sel = embassy_stm32::pac::rcc::vals::Clk48sel::PLL1_Q;
```

### Basic CDC-ACM Example

```rust
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_stm32::usb::Usb;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config};
use static_cell::StaticCell;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Config::default());
    
    // Create USB driver
    static EP_OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();
    let ep_out_buffer = EP_OUT_BUFFER.init([0; 256]);
    
    let usb = Usb::new(p.OTG_FS, p.PA11, p.PA12, ep_out_buffer);
    
    // Create USB device
    static CONFIG: StaticCell<Config> = StaticCell::new();
    let config = CONFIG.init(Config::new(0x16c0, 0x27dd));
    config.device_class = 0xEF; // Miscellaneous
    config.device_protocol = 0x01;
    config.device_sub_class = 0x02;
    config.max_packet_size_0 = 64;
    
    static DEVICE_DESC: StaticCell<[u8; 8]> = StaticCell::new();
    let device_desc = DEVICE_DESC.init([]);
    
    static CONFIG_DESC: StaticCell<[u8; 128]> = StaticCell::new();
    let config_desc = CONFIG_DESC.init([0; 128]);
    
    static BOS_DESC: StaticCell<[u8; 16]> = StaticCell::new();
    let bos_desc = BOS_DESC.init([]);
    
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    let control_buf = CONTROL_BUF.init([0; 64]);
    
    let mut builder = Builder::new(
        usb,
        config,
        device_desc,
        config_desc,
        bos_desc,
        &mut [], // msos_desc
        control_buf,
    );
    
    // Create CDC-ACM class
    static STATE: StaticCell<State> = StaticCell::new();
    let state = STATE.init(State::new());
    
    let mut cdc = CdcAcmClass::new(&mut builder, state, 64);
    
    // Build and run
    static USB_DEVICE: StaticCell<embassy_usb::UsbDevice<'_, Usb>> = StaticCell::new();
    let usb_device = USB_DEVICE.init(builder.build());
    
    embassy_futures::join::join(usb_device.run(), async {
        loop {
            let mut buf = [0u8; 64];
            match cdc.read_packet(&mut buf).await {
                Ok(n) => {
                    // Echo back
                    let _ = cdc.write_packet(&buf[..n]).await;
                }
                Err(_) => {}
            }
        }
    }).await;
}
```

## Clock Configuration Trade-offs

| Config | Sysclk | USB | RNG | Display | SDRAM | Use Case |
|--------|--------|-----|-----|---------|-------|----------|
| 180MHz | 180MHz | NO | NO | YES | YES | Display-intensive apps |
| 84MHz | 168MHz | YES | YES | NO* | NO | USB/RNG apps |

*Display can work at 168MHz but requires different PLLSAI config.

## Common Issues

### probe-rs Breaks USB Enumeration

When probe-rs is attached for RTT logging, it may block the USB ISR causing disconnects.

**Solution**: Use st-flash for USB tests:

```bash
# Flash with st-flash (not probe-rs)
arm-none-eabi-objcopy -O binary firmware firmware.bin
st-flash --connect-under-reset write firmware.bin 0x08000000
st-flash --connect-under-reset reset
sleep 15
python3 tests/usb_cdc_stress.py --port /dev/ttyACM0
```

### ST-LINK Recovery After USB

When USB CDC is active, the STM32F469 can lock out SWD. Recovery:

```bash
st-flash --connect-under-reset reset
# Immediately run probe-rs if needed
probe-rs run --chip STM32F469NIHx firmware
```

### No 48MHz Clock at 180MHz

The 180MHz PLL config cannot produce 48MHz:
- PLL1_Q = 360/7 = 51.4MHz (out of USB 0.25% tolerance)
- PLLSAI_R = 384/7 = 54.9MHz (also out of tolerance)

**Solution**: Use 84MHz config for USB, or accept display won't work.

## Related Examples

- `examples/test_usb_cdc_stress.rs` - USB CDC stress test
- `tests/usb_cdc_stress.py` - Host-side stress test
- `tests/usb_cdc_test.py` - Host-side test monitor

## Troubleshooting

| Symptom | Likely Cause | Solution |
|---------|--------------|----------|
| Device not recognized | Wrong clock config | Use 84MHz PLL with PLL1_Q=48MHz |
| Enumeration fails | probe-rs interference | Use st-flash instead |
| Intermittent connection | RTT blocking | Disable defmt for USB tests |
| No 48MHz clock | 180MHz config | Switch to 84MHz config |
