//! Clock configuration presets for STM32F469I-Discovery.
//!
//! Eliminates the need to copy PLL/PLLSAI boilerplate into every project.
//! All configs produce correct 48MHz for USB/RNG where applicable.
//!
//! # Quick reference
//!
//! | Preset | Sysclk | USB/RNG | Display | Use case |
//! |--------|--------|---------|---------|----------|
//! | [`config_180`] | 180 MHz | 48 MHz via PLLSAI_P | 54.86 MHz LTDC | Full speed, all peripherals |
//! | [`config_168`] | 168 MHz | 48 MHz via PLL1_Q | 54.86 MHz LTDC | USB+display, simpler clock |
//! | [`config_usb_only`] | 168 MHz | 48 MHz via PLL1_Q | unavailable | USB CDC without display |
//!
//! # Usage
//!
//! ```rust,ignore
//! let config = embassy_stm32f469i_disco::config_180();
//! let p = embassy_stm32::init(config);
//! ```
//!
//! # 48MHz clock background
//!
//! USB OTG FS and RNG both require 48MHz ±0.25%. At 180MHz sysclk, PLL1_Q
//! produces 51.4MHz (unusable). PLLSAI provides 48MHz via PLLSAI_P = VCO/8.
//! The CK48MSEL mux (DCKCFGR bit 27) selects between PLL1_Q and PLLSAI as
//! the 48MHz source. Embassy writes this to DCKCFGR, which is the correct
//! register for STM32F469 (DCKCFGR2 does not exist on this MCU — see
//! issue #27).

use embassy_stm32::rcc::*;
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;

pub const SYSCLK_HZ_180: u32 = 180_000_000;
pub const SYSCLK_HZ_168: u32 = 168_000_000;

/// 180 MHz sysclk + 48 MHz USB/RNG via PLLSAI_P + 54.86 MHz LTDC via PLLSAI_R.
///
/// All peripherals work simultaneously. Recommended for full-featured firmware.
///
/// ```rust,ignore
/// let p = embassy_stm32::init(embassy_stm32f469i_disco::config_180());
/// let sdram = SdramCtrl::new(&mut unsafe { embassy_stm32::Peripherals::steal() }, 180_000_000);
/// ```
pub fn config_180() -> Config {
    let mut config = Config::default();
    config.rcc.hse = Some(Hse {
        freq: Hertz(8_000_000),
        mode: HseMode::Oscillator,
    });
    config.rcc.pll_src = PllSource::HSE;
    config.rcc.pll = Some(Pll {
        prediv: PllPreDiv::DIV8,
        mul: PllMul::MUL360,
        divp: Some(PllPDiv::DIV2),
        divq: Some(PllQDiv::DIV7),
        divr: Some(PllRDiv::DIV6),
    });
    config.rcc.pllsai = Some(Pll {
        prediv: PllPreDiv::DIV8,
        mul: PllMul::MUL384,
        divp: Some(PllPDiv::DIV8),
        divq: Some(PllQDiv::DIV8),
        divr: Some(PllRDiv::DIV7),
    });
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.mux.clk48sel = mux::Clk48sel::PLLSAI1_Q;
    config
}

/// 168 MHz sysclk + 48 MHz USB/RNG via PLL1_Q + 54.86 MHz LTDC via PLLSAI_R.
///
/// Simpler clock tree: PLL1_Q produces exact 48MHz, so no PLLSAI needed for USB.
/// Recommended when USB+display are needed and 12MHz less CPU speed is acceptable.
///
/// ```rust,ignore
/// let p = embassy_stm32::init(embassy_stm32f469i_disco::config_168());
/// ```
pub fn config_168() -> Config {
    let mut config = Config::default();
    config.rcc.hse = Some(Hse {
        freq: Hertz(8_000_000),
        mode: HseMode::Oscillator,
    });
    config.rcc.pll_src = PllSource::HSE;
    config.rcc.pll = Some(Pll {
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL168,
        divp: Some(PllPDiv::DIV2),
        divq: Some(PllQDiv::DIV7),
        divr: None,
    });
    config.rcc.pllsai = Some(Pll {
        prediv: PllPreDiv::DIV8,
        mul: PllMul::MUL384,
        divp: Some(PllPDiv::DIV8),
        divq: Some(PllQDiv::DIV8),
        divr: Some(PllRDiv::DIV7),
    });
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.mux.clk48sel = mux::Clk48sel::PLL1_Q;
    config
}

/// 168 MHz sysclk + 48 MHz USB/RNG via PLL1_Q, no display/PLLSAI.
///
/// For USB-only firmware that doesn't need SDRAM or display.
pub fn config_usb_only() -> Config {
    let mut config = Config::default();
    config.rcc.hse = Some(Hse {
        freq: Hertz(8_000_000),
        mode: HseMode::Oscillator,
    });
    config.rcc.pll_src = PllSource::HSE;
    config.rcc.pll = Some(Pll {
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL168,
        divp: Some(PllPDiv::DIV2),
        divq: Some(PllQDiv::DIV7),
        divr: None,
    });
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.mux.clk48sel = mux::Clk48sel::PLL1_Q;
    config
}
