//! Ergonomic board-level constructor for STM32F469I-Discovery.

use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::peripherals;
use embassy_stm32::{rcc, Peri, Peripherals};

use crate::bist::{BootTestResults, TestResult};
use crate::{BoardHint, DisplayCtrl, SdramCtrl, TouchCtrl};

/// Errors that can occur during [`Board::try_new`] initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum BoardInitError {
    /// HCLK frequency could not be determined from the RCC clock tree.
    /// This should never happen after a valid `config_180()` / `config_168()` call.
    HclkUnavailable,
    /// Display initialization failed.
    Display(crate::DisplayInitError),
}

impl From<crate::DisplayInitError> for BoardInitError {
    fn from(e: crate::DisplayInitError) -> Self {
        BoardInitError::Display(e)
    }
}

/// User LEDs on the STM32F469I-Discovery board.
///
/// LEDs are active-low: drive low to turn on, high to turn off.
pub struct Leds {
    /// Green LED on PG6.
    pub green: Output<'static>,
    /// Orange LED on PD4.
    pub orange: Output<'static>,
    /// Red LED on PD5.
    pub red: Output<'static>,
    /// Blue LED on PK3.
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
/// let board = Board::try_new(p, BoardHint::Auto).expect("board init");
/// let _ = board;
/// ```
pub struct Board {
    /// DSI/LTDC display controller.
    pub display: DisplayCtrl<'static>,
    /// FT6X06 touch controller.
    pub touch: TouchCtrl,
    /// User LEDs.
    pub leds: Leds,
    /// User button on PA0.
    pub user_button: UserButton,
    /// Pins left free after SDRAM/display bring-up.
    pub sdram_remainders: SdramRemainders,
    /// Hardware self-test results from board initialization.
    pub test_results: BootTestResults,
}

impl Board {
    /// Initialize SDRAM, display, touch controller, LEDs, and user button.
    ///
    /// Consumes I2C1 (PB8/PB9) for the touch controller.
    ///
    /// # Errors
    ///
    /// Returns [`BoardInitError::HclkUnavailable`] if the HCLK frequency
    /// cannot be determined (should never happen after a valid
    /// [`config_180()`](crate::config_180) /
    /// [`config_168()`](crate::config_168) call).
    ///
    /// Returns [`BoardInitError::Display`] if display initialization fails.
    pub fn try_new(mut p: Peripherals, hint: BoardHint) -> Result<Self, BoardInitError> {
        let source_clock_hz = rcc::clocks(&p.RCC)
            .hclk1
            .to_hertz()
            .ok_or(BoardInitError::HclkUnavailable)?
            .0;

        let mut sdram = SdramCtrl::new(&mut p, source_clock_hz);
        let sdram_ok = sdram.test_quick();
        let framebuffer = sdram.into_bytes();

        let display = DisplayCtrl::try_new(framebuffer, p.LTDC, p.DSIHOST, p.PJ2, p.PH7, hint)?;
        // Display init succeeded if we reach here — try_new returns Err otherwise.
        let display_result = TestResult::Pass;

        let i2c = embassy_stm32::i2c::I2c::new_blocking(
            p.I2C1,
            p.PB8,
            p.PB9,
            embassy_stm32::i2c::Config::default(),
        );
        let mut touch = TouchCtrl::new(i2c);

        let touch_vendor_id_result = touch.read_vendor_id();
        let touch_chip_model_result = touch.read_chip_model();
        let touch_idle_result = touch.td_status();

        let touch_i2c_result = if touch_vendor_id_result.is_ok() || touch_chip_model_result.is_ok() {
            TestResult::Pass
        } else {
            TestResult::Fail
        };

        let touch_vendor_id = match touch_vendor_id_result {
            Ok(0x11) => TestResult::Pass,
            Ok(_) => TestResult::Fail,
            Err(_) => TestResult::Fail,
        };

        let touch_chip_model = match touch_chip_model_result {
            Ok(0x06 | 0x36 | 0x64) => TestResult::Pass,
            Ok(_) => TestResult::Fail,
            Err(_) => TestResult::Fail,
        };

        let touch_idle = match touch_idle_result {
            Ok(0) => TestResult::Pass,
            Ok(_) => TestResult::Fail,
            Err(_) => TestResult::Fail,
        };

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

        let test_results = BootTestResults {
            sdram: if sdram_ok { TestResult::Pass } else { TestResult::Fail },
            display: display_result,
            touch_i2c: touch_i2c_result,
            touch_vendor_id,
            touch_chip_model,
            touch_idle,
            leds: TestResult::Pass,
            user_button: TestResult::Pass,
        };

        Ok(Self {
            display,
            touch,
            leds,
            user_button,
            sdram_remainders,
            test_results,
        })
    }
}
