//! Board support package for the STM32F469I-Discovery board with Embassy async framework.
//!
//! # Features
//!
//! - `display` (default) — DSI/LTDC display via NT35510, embedded-graphics support, SDRAM controller
//! - `touch` (default) — FT6X06 capacitive touch via I2C1
//!
//! # Quick start
//!
//! ```rust,ignore
//! let p = embassy_stm32::init(embassy_stm32::Config::default());
//! let mut led = embassy_stm32::gpio::Output::new(p.PG6, embassy_stm32::gpio::Level::Low, embassy_stm32::gpio::Speed::Low);
//! loop {
//!     Timer::after(embassy_time::Duration::from_secs(1)).await;
//!     led.toggle();
//! }
//! ```
//!
//! # Display + SDRAM
//!
//! Display and SDRAM require a 180 MHz PLL configuration and `unsafe { Peripherals::steal() }`
//! for pin reuse. See `examples/display_blinky.rs` for a complete working example.
//!
//! # USB CDC
//!
//! USB requires a separate 84 MHz clock config (incompatible with display).
//! See `examples/test_usb_cdc.rs` for a complete test example.

#![no_std]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::identity_op)]
#![allow(clippy::single_match)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::new_without_default)]

/// Display subsystem: SDRAM controller, DSI/LTDC display, NT35510 panel driver.
#[cfg(feature = "display")]
pub mod display;

/// Touch controller: FT6X06 capacitive touch via I2C1.
#[cfg(feature = "touch")]
pub mod touch;

#[cfg(feature = "display")]
pub use display::{
    BoardHint, DisplayCtrl, FramebufferView, LcdController, SdramCtrl, FB_HEIGHT, FB_WIDTH,
    SDRAM_SIZE_BYTES,
};

#[cfg(feature = "touch")]
pub use touch::{TouchCtrl, TouchPoint};
