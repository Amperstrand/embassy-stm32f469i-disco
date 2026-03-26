#![no_std]

#[cfg(feature = "display")]
pub mod display;

#[cfg(feature = "touch")]
pub mod touch;

#[cfg(feature = "uart")]
pub mod uart;

#[cfg(feature = "display")]
pub use display::{DisplayCtrl, FramebufferView, FB_HEIGHT, FB_WIDTH};

#[cfg(feature = "touch")]
pub use touch::{TouchCtrl, TouchPoint};

#[cfg(feature = "uart")]
pub use uart::UartCtrl;
