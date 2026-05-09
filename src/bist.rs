//! Built-in self-test (BIST) results for STM32F469I-Discovery board.
//!
//! Automatically populated by [`Board::try_new`](crate::Board::try_new) during
//! initialization. Each subsystem test runs inline during board bring-up —
//! zero extra boot time, zero configuration.
//!
//! # Usage
//!
//! ```rust,ignore
//! let board = Board::try_new(p, BoardHint::ForceNt35510)?;
//! // board.test_results is always available
//! defmt::info!("BIST: {}/{}", board.test_results.passed_count(), board.test_results.total());
//! ```
//!
//! The application decides how to report results (display, USB CDC, LEDs, etc.).
//! The BSP only collects them.

/// Result of an individual hardware test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TestResult {
    /// Test passed.
    Pass,
    /// Test failed.
    Fail,
    /// Test was not applicable (e.g. no hardware connected).
    Skip,
}

impl TestResult {
    /// Returns `true` for `Pass`.
    pub const fn is_pass(self) -> bool {
        matches!(self, Self::Pass)
    }
}

/// A named test entry for iteration.
#[derive(Clone, Copy)]
pub struct TestEntry {
    /// Human-readable test name.
    pub name: &'static str,
    /// Test result.
    pub result: TestResult,
}

/// Hardware self-test results collected during [`Board::try_new`](crate::Board::try_new).
///
/// Every field corresponds to a subsystem initialized by the board constructor.
/// Tests run inline during init — no extra time or configuration needed.
#[derive(Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BootTestResults {
    /// SDRAM quick test: write/verify 4 KiB pattern.
    pub sdram: TestResult,
    /// DSI/LTDC display controller initialized successfully.
    pub display: TestResult,
    /// FT6X06 touch I2C bus responds.
    pub touch_i2c: TestResult,
    /// FT6X06 vendor ID matches expected value (0x11).
    pub touch_vendor_id: TestResult,
    /// FT6X06 chip model is a known value (0x06, 0x36, or 0x64).
    pub touch_chip_model: TestResult,
    /// FT6X06 reports no active touches at boot (td_status == 0).
    pub touch_idle: TestResult,
    /// User LEDs configured successfully.
    pub leds: TestResult,
    /// User button (PA0) configured successfully.
    pub user_button: TestResult,
}

impl Default for BootTestResults {
    fn default() -> Self {
        Self {
            sdram: TestResult::Skip,
            display: TestResult::Skip,
            touch_i2c: TestResult::Skip,
            touch_vendor_id: TestResult::Skip,
            touch_chip_model: TestResult::Skip,
            touch_idle: TestResult::Skip,
            leds: TestResult::Skip,
            user_button: TestResult::Skip,
        }
    }
}

impl BootTestResults {
    /// Returns all test entries as a slice for iteration.
    pub const fn entries(&self) -> [TestEntry; 8] {
        [
            TestEntry { name: "SDRAM", result: self.sdram },
            TestEntry { name: "Display", result: self.display },
            TestEntry { name: "Touch I2C", result: self.touch_i2c },
            TestEntry { name: "Touch Vendor ID", result: self.touch_vendor_id },
            TestEntry { name: "Touch Chip Model", result: self.touch_chip_model },
            TestEntry { name: "Touch Idle", result: self.touch_idle },
            TestEntry { name: "LEDs", result: self.leds },
            TestEntry { name: "User Button", result: self.user_button },
        ]
    }

    /// Number of tests that passed.
    pub fn passed_count(&self) -> usize {
        self.entries().iter().filter(|e| e.result.is_pass()).count()
    }

    /// Total number of tests.
    pub const fn total(&self) -> usize {
        8
    }

    /// Returns `true` if all tests passed.
    pub fn all_passed(&self) -> bool {
        self.passed_count() == self.total()
    }
}
