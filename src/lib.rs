#![no_std]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::identity_op)]
#![allow(clippy::single_match)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::new_without_default)]
#[cfg(feature = "display")]
pub mod display;

#[cfg(feature = "touch")]
pub mod touch;

#[cfg(feature = "display")]
pub use display::{BoardHint, DisplayCtrl, FramebufferView, LcdController, FB_HEIGHT, FB_WIDTH};

#[cfg(feature = "touch")]
pub use touch::{TouchCtrl, TouchPoint};
