//! Panel detection and controller-specific initialization helpers.

mod nt35510;

pub(crate) use nt35510::init_panel;
pub use nt35510::{detect_panel, BoardHint, LcdController};
