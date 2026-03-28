#![no_std]

#[cfg(feature = "display")]
pub mod display;

#[cfg(feature = "touch")]
pub mod touch;

#[cfg(feature = "display")]
pub use display::{BoardHint, DisplayCtrl, FramebufferView, LcdController, FB_HEIGHT, FB_WIDTH};

#[cfg(feature = "touch")]
pub use touch::{TouchCtrl, TouchPoint};
