//! BSP DisplayCtrl test for STM32F469I-DISCO (portrait 480x800)
//!
//! Tests `DisplayCtrl::new()` BSP API with SDRAM framebuffer.
//! Draws 4 color bands (red/green/blue/white) to verify display output.
//!
//! Build:
//!   cargo build --release --target thumbv7em-none-eabihf \
//!     --manifest-path /home/ubuntu/src/embassy-stm32f469i-disco/Cargo.toml \
//!     --example display_hybrid
//!
//! Flash (RTT debug — USB will NOT work):
//!   arm-none-eabi-objcopy -O binary target/thumbv7em-none-eabihf/release/display_hybrid /tmp/display_hybrid.bin
//!   st-flash --connect-under-reset write /tmp/display_hybrid.bin 0x08000000
//!   st-flash --connect-under-reset reset

#![no_std]
#![no_main]

extern crate alloc;

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32f469i_disco::config_180;
use embassy_time::Timer;
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use linked_list_allocator::LockedHeap;
use {defmt_rtt as _, panic_probe as _};

#[global_allocator]
static mut HEAP: LockedHeap = LockedHeap::empty();

const HEAP_SIZE: usize = 64 * 1024;
static mut HEAP_MEMORY: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

const LCD_X_SIZE: i32 = 480;
const LCD_Y_SIZE: i32 = 800;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    #[allow(static_mut_refs)]
    unsafe {
        HEAP.lock()
            .init(core::ptr::addr_of_mut!(HEAP_MEMORY) as *mut u8, HEAP_SIZE);
    }

    let p = embassy_stm32::init(config_180());
    info!("display_hybrid: starting (portrait 480x800)");

    // ── SDRAM init (must be before moving peripherals out of p) ──
    let sdram = embassy_stm32f469i_disco::sdram_init!(p);
    let framebuffer = sdram.into_bytes();
    info!("display_hybrid: SDRAM initialized");

    // ── GPIO ──
    let mut led = Output::new(p.PG6, Level::High, Speed::Low);

    let mut display = embassy_stm32f469i_disco::DisplayCtrl::new(
        framebuffer,
        p.LTDC,
        p.DSIHOST,
        p.PJ2,
        p.PH7,
        embassy_stm32f469i_disco::BoardHint::ForceNt35510,
    );
    info!("display_hybrid: DisplayCtrl::new() complete");

    let mut fb = display.fb();
    fb.clear(Rgb888::BLACK);

    let rows_per_band = LCD_Y_SIZE / 4;
    let colors = [
        Rgb888::new(255, 0, 0),
        Rgb888::new(0, 255, 0),
        Rgb888::new(0, 0, 255),
        Rgb888::new(255, 255, 255),
    ];

    for (band, color) in colors.iter().enumerate() {
        let y = band as i32 * rows_per_band;
        let height = if band == 3 {
            LCD_Y_SIZE - y
        } else {
            rows_per_band
        };
        Rectangle::new(
            Point::new(0, y),
            Size::new(LCD_X_SIZE as u32, height as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(*color))
        .draw(&mut fb)
        .unwrap();
    }

    info!("display_hybrid: framebuffer color bands drawn");

    loop {
        led.set_low();
        Timer::after_millis(1000).await;

        led.set_high();
        Timer::after_millis(1000).await;
    }
}
