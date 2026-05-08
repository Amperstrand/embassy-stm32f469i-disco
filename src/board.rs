//! Ergonomic board-level constructor for STM32F469I-Discovery.

use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::peripherals;
use embassy_stm32::{rcc, Peri, Peripherals};

use crate::{BoardHint, DisplayCtrl, SdramCtrl, TouchCtrl};

/// User LEDs on the STM32F469I-Discovery board.
///
/// LEDs are active-low: drive low to turn on, high to turn off.
pub struct Leds {
    pub green: Output<'static>,
    pub orange: Output<'static>,
    pub red: Output<'static>,
    pub blue: Output<'static>,
}

/// User button on PA0.
pub struct UserButton(pub Input<'static>);

/// Pins left free after SDRAM/display bring-up.
pub struct SdramRemainders {
    /// USART6 TX (scanner use).
    pub usart6_tx: Peri<'static, peripherals::PG14>,
    /// USART6 RX (scanner use).
    pub usart6_rx: Peri<'static, peripherals::PG9>,
}

/// Ergonomic entry point for the STM32F469I-Discovery board.
///
/// # Example
/// ```no_run
/// use embassy_stm32f469i_disco::{Board, BoardHint};
///
/// let p = embassy_stm32::init(embassy_stm32f469i_disco::config_180());
/// let board = Board::new(p, BoardHint::Auto);
/// let _ = board;
/// ```
pub struct Board {
    pub display: DisplayCtrl<'static>,
    pub touch: TouchCtrl,
    pub leds: Leds,
    pub user_button: UserButton,
    pub sdram_remainders: SdramRemainders,
}

impl Board {
    /// Initialize SDRAM, display, touch controller handle, LEDs, and user button.
    #[must_use]
    pub fn new(mut p: Peripherals, hint: BoardHint) -> Self {
        let source_clock_hz = rcc::clocks(&p.RCC)
            .hclk1
            .to_hertz()
            .expect("HCLK unavailable")
            .0;

        let sdram = SdramCtrl::new(&mut p, source_clock_hz);
        let framebuffer = sdram.into_bytes();
        let display = DisplayCtrl::new(framebuffer, p.LTDC, p.DSIHOST, p.PJ2, p.PH7, hint);
        let touch = TouchCtrl::new();

        let leds = Leds {
            green: Output::new(p.PG6, Level::High, Speed::Low),
            orange: Output::new(p.PD4, Level::High, Speed::Low),
            red: Output::new(p.PD5, Level::High, Speed::Low),
            blue: Output::new(p.PK3, Level::High, Speed::Low),
        };

        let user_button = UserButton(Input::new(p.PA0, Pull::Down));

        let sdram_remainders = SdramRemainders {
            usart6_tx: p.PG14,
            usart6_rx: p.PG9,
        };

        Self {
            display,
            touch,
            leds,
            user_button,
            sdram_remainders,
        }
    }
}
