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
//! let config = embassy_stm32f469i_disco::config_180();
//! let p = embassy_stm32::init(config);
//! ```
//!
//! # Clock presets
//!
//! Use [`config_180`], [`config_168`], or [`config_usb_only`] instead of manually
//! configuring PLL/PLLSAI. See [`clock`] module for details.

#![no_std]

pub mod clock;
pub mod usb;

pub use clock::{config_168, config_180, config_usb_only, SYSCLK_HZ_168, SYSCLK_HZ_180};
pub use usb::reset_usb_phy;

#[cfg(feature = "display")]
mod sdram;

/// Display subsystem: SDRAM controller, DSI/LTDC display, NT35510 panel driver.
#[cfg(feature = "display")]
pub mod display;

/// Touch controller: FT6X06 capacitive touch via I2C1.
#[cfg(feature = "touch")]
pub mod touch;

#[cfg(feature = "display")]
pub use display::{
    Argb8888, BoardHint, DisplayCtrl, DisplayCtrlCtor, DisplayFormat, DisplayInitError,
    FramebufferView, LcdController, Rgb565, FB_HEIGHT, FB_WIDTH,
};

#[cfg(feature = "display")]
pub use sdram::{SdramCtrl, SDRAM_SIZE_BYTES};

#[cfg(feature = "touch")]
pub use touch::{TouchCtrl, TouchError, TouchPoint};
