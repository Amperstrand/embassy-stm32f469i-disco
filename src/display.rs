//! Display subsystem: SDRAM controller (FMC), DSI/LTDC display driver (NT35510/OTM8009A),
//! panel detection, and framebuffer management.

use embassy_stm32::ltdc::Ltdc;
use embassy_stm32::{dsihost, peripherals, Peri};
use embassy_time::{block_for, Duration};
use embedded_graphics::{
    pixelcolor::{Rgb565 as EgRgb565, Rgb888, RgbColor},
    prelude::IntoStorage,
};
use embedded_hal::delay::DelayNs;
#[cfg(feature = "defmt")]
use stm32_metapac::LTDC;

use crate::dsi::{configure_dsi_host, DsiHostAdapter};
use crate::framebuffer::framebuffer_from_bytes;
use crate::framebuffer::FramebufferView;
use crate::ltdc::{configure_ltdc, configure_ltdc_layer};
use crate::panel::{detect_panel, init_panel, BoardHint, LcdController};
pub use crate::sdram::{SdramCtrl, SDRAM_SIZE_BYTES};

#[cfg(feature = "defmt")]
use defmt as _;

struct BusyDelay;

impl DelayNs for BusyDelay {
    fn delay_ns(&mut self, ns: u32) {
        block_for(Duration::from_nanos(ns as u64));
    }
}

/// Panel height in pixels (portrait). Re-exported from nt35510.
pub use nt35510::PANEL_HEIGHT as FB_HEIGHT;
/// Panel width in pixels (portrait). Re-exported from nt35510.
pub use nt35510::PANEL_WIDTH as FB_WIDTH;

/// Total framebuffer pixel count (480 × 800 = 384,000).
pub const FB_SIZE: usize = FB_WIDTH as usize * FB_HEIGHT as usize;

/// Pixel format trait for display controller configuration.
///
/// Implementors define how colors are encoded for LTDC, DSI, and the panel IC.
/// Two built-in formats are provided: [`Argb8888`] (4 bytes/pixel) and [`Rgb565`] (2 bytes/pixel).
pub trait DisplayFormat: Copy + 'static {
    /// Raw pixel type stored in the framebuffer.
    type Pixel: Copy + PartialEq;

    /// Color type from `embedded-graphics`.
    type Color: RgbColor;

    /// LTDC pixel format register value (0=ARGB8888, 2=RGB565, etc.).
    fn ltdc_pf() -> u8;

    /// DSI color coding byte.
    fn dsi_color_coding() -> u8;

    /// NT35510 color format enum variant.
    fn nt35510_color_format() -> nt35510::ColorFormat;

    /// Bytes per pixel.
    fn bpp() -> usize;

    /// Convert an `embedded-graphics` color to a raw pixel value.
    fn encode(color: Self::Color) -> Self::Pixel;

    /// Raw pixel value representing black.
    fn black() -> Self::Pixel;

    /// Default `embedded-graphics` color (typically black).
    fn default_color() -> Self::Color;
}

/// ARGB8888 pixel format (4 bytes/pixel, 32-bit).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Argb8888;

impl DisplayFormat for Argb8888 {
    type Pixel = u32;
    type Color = Rgb888;

    fn ltdc_pf() -> u8 {
        0
    }

    fn dsi_color_coding() -> u8 {
        0x05
    }

    fn nt35510_color_format() -> nt35510::ColorFormat {
        nt35510::ColorFormat::Rgb888
    }

    fn bpp() -> usize {
        4
    }

    fn encode(color: Self::Color) -> Self::Pixel {
        0xFF00_0000 | ((color.r() as u32) << 16) | ((color.g() as u32) << 8) | (color.b() as u32)
    }

    fn black() -> Self::Pixel {
        0xFF00_0000
    }

    fn default_color() -> Self::Color {
        Rgb888::BLACK
    }
}

/// RGB565 pixel format (2 bytes/pixel, 16-bit).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Rgb565;

impl DisplayFormat for Rgb565 {
    type Pixel = u16;
    type Color = EgRgb565;

    fn ltdc_pf() -> u8 {
        2
    }

    fn dsi_color_coding() -> u8 {
        0x00
    }

    fn nt35510_color_format() -> nt35510::ColorFormat {
        nt35510::ColorFormat::Rgb565
    }

    fn bpp() -> usize {
        2
    }

    fn encode(color: Self::Color) -> Self::Pixel {
        color.into_storage()
    }

    fn black() -> Self::Pixel {
        0
    }

    fn default_color() -> Self::Color {
        EgRgb565::BLACK
    }
}

/// Errors that can occur during display initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum DisplayInitError {
    /// DSI host voltage regulator did not become ready within the timeout.
    DsiRegulatorTimeout,
    /// DSI host PLL did not lock within the timeout.
    DsiPllTimeout,
    /// DSI command write to the panel failed.
    DsiWrite,
    /// Panel initialization sequence failed.
    PanelInit,
    /// Caller-supplied framebuffer byte slice is too small for the requested resolution.
    ///
    /// Returned when the slice length is less than `len_pixels * F::bpp()`.
    FramebufferTooSmall {
        /// Actual number of bytes provided by the caller.
        provided_bytes: usize,
        /// Minimum number of bytes required for the requested resolution.
        required_bytes: usize,
    },
    /// Caller-supplied framebuffer byte slice is not properly aligned.
    ///
    /// Returned when the slice start address is not aligned to `align_of::<F::Pixel>()`.
    FramebufferMisaligned {
        /// Actual address of the slice pointer.
        ptr_addr: usize,
        /// Required alignment for the pixel type.
        required_align: usize,
    },
}

/// Display orientation (portrait or landscape).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DisplayOrientation {
    /// Portrait mode: 480 wide × 800 tall.
    Portrait,
    /// Landscape mode: 800 wide × 480 tall.
    Landscape,
}

impl DisplayOrientation {
    /// Pixel width for this orientation.
    pub const fn width(self) -> u16 {
        match self {
            DisplayOrientation::Portrait => FB_WIDTH,
            DisplayOrientation::Landscape => FB_HEIGHT,
        }
    }

    /// Pixel height for this orientation.
    pub const fn height(self) -> u16 {
        match self {
            DisplayOrientation::Portrait => FB_HEIGHT,
            DisplayOrientation::Landscape => FB_WIDTH,
        }
    }

    /// Total framebuffer pixel count for this orientation.
    pub const fn fb_size(self) -> usize {
        (self.width() as usize) * (self.height() as usize)
    }

    /// NT35510 panel mode corresponding to this orientation.
    pub const fn nt35510_mode(self) -> nt35510::Mode {
        match self {
            DisplayOrientation::Portrait => nt35510::Mode::Portrait,
            DisplayOrientation::Landscape => nt35510::Mode::Landscape,
        }
    }
}

// ── Display init (orchestrator) ────────────────────────────────────────

/// DSI/LTDC display controller with embedded-graphics support.
///
/// Owns the framebuffer, LTDC peripheral, and DSI host. Use [`fb()`](Self::fb)
/// to obtain a [`FramebufferView`] for drawing. The default pixel format is
/// [`Argb8888`]; use `DisplayCtrl<'d, Rgb565>` for 16-bit color.
pub struct DisplayCtrl<'d, F: DisplayFormat = Argb8888> {
    framebuffer: &'static mut [F::Pixel],
    _ltdc: Ltdc<'d, peripherals::LTDC>,
    dsi: dsihost::DsiHost<'d, peripherals::DSIHOST>,
    orientation: DisplayOrientation,
    _format: core::marker::PhantomData<F>,
}

impl<'d, F: DisplayFormat> DisplayCtrl<'d, F> {
    fn try_new_internal(
        framebuffer_bytes: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
        orientation: DisplayOrientation,
    ) -> Result<Self, DisplayInitError> {
        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: enter");

        let mut reset_pin = embassy_stm32::gpio::Output::new(
            lcd_reset,
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );
        BusyDelay.delay_ms(20);
        reset_pin.set_high();
        BusyDelay.delay_ms(140);
        // SAFETY: `reset_pin` must not be dropped — dropping an `Output` reconfigures
        // the GPIO to floating input, which would de-assert the panel reset line and
        // corrupt the display. The panel hardware owns this pin for the lifetime of
        // the display controller.
        core::mem::forget(reset_pin);

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: LCD reset done");

        let mut ltdc = Ltdc::new(ltdc);
        let mut dsi = dsihost::DsiHost::new(dsi_host, dsi_te);
        configure_dsi_host(&mut dsi, orientation, F::dsi_color_coding())?;

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: dsi init done (not yet enabled)");

        let fb_slice = framebuffer_from_bytes::<F>(framebuffer_bytes, orientation.fb_size())?;
        let fb_addr = fb_slice.as_mut_ptr() as u32;
        configure_ltdc(&mut ltdc, orientation);

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: ltdc init done");

        dsi.enable();
        dsi.enable_wrapper_dsi();
        block_for(Duration::from_millis(120));

        let controller = match hint {
            BoardHint::ForceOtm8009a => LcdController::Otm8009a,
            _ => detect_panel(&mut DsiHostAdapter::new(&mut dsi), hint),
        };

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: detect_panel done");

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: GCR before panel = {:08x}", LTDC.twcr().read().0);

        init_panel::<F>(&mut dsi, controller, orientation)?;

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: GCR={:08x}", LTDC.twcr().read().0);

        configure_ltdc_layer::<F>(&mut ltdc, fb_addr, orientation);

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: layer config done");

        Ok(DisplayCtrl {
            framebuffer: fb_slice,
            _ltdc: ltdc,
            dsi,
            orientation,
            _format: core::marker::PhantomData,
        })
    }

    /// Obtain a [`FramebufferView`] for drawing via `embedded-graphics`.
    ///
    /// The returned view borrows `self` mutably, so only one view can exist at a time.
    #[must_use]
    pub fn fb(&mut self) -> FramebufferView<'_, F> {
        FramebufferView::new(
            self.framebuffer,
            self.orientation.width() as usize,
            self.orientation.height() as usize,
        )
    }
    /// Access the underlying DSI host peripheral.
    pub fn dsi(&mut self) -> &mut dsihost::DsiHost<'d, peripherals::DSIHOST> {
        &mut self.dsi
    }
    /// Returns the current display orientation.
    pub fn orientation(&self) -> DisplayOrientation {
        self.orientation
    }

    /// Dump all LTDC layer 0 registers via defmt for debugging display issues.
    ///
    /// Call after `DisplayCtrl::new()` to verify the LTDC configuration.
    /// Logs framebuffer address, pixel format, window position, and timing.
    #[cfg(feature = "defmt")]
    pub fn log_ltdc_config(&self) {
        let ltdc = LTDC;
        let layer = ltdc.layer(0);
        defmt::info!(
            "LTDC: CFBAR={:#x}, PFCR={}, CFBP={}, CFBLL={}, CFBLNR={}",
            layer.cfbar().read().cfbadd(),
            layer.pfcr().read().pf() as u8,
            layer.cfblr().read().cfbp(),
            layer.cfblr().read().cfbll(),
            layer.cfblnr().read().cfblnbr(),
        );
        defmt::info!(
            "LTDC: WHPCR={}..{}, WVPCR={}..{}",
            layer.whpcr().read().whstpos(),
            layer.whpcr().read().whsppos(),
            layer.wvpcr().read().wvstpos(),
            layer.wvpcr().read().wvsppos(),
        );
        defmt::info!(
            "LTDC: SSCR hsw={}, vsh={}, BPCR ahbp={}, avbp={}",
            ltdc.sscr().read().hsw(),
            ltdc.sscr().read().vsh(),
            ltdc.bpcr().read().ahbp(),
            ltdc.bpcr().read().avbp(),
        );
        defmt::info!(
            "LTDC: AWCR aah={}, aaw={}, TWCR totalh={}, totalw={}",
            ltdc.awcr().read().aah(),
            ltdc.awcr().read().aaw(),
            ltdc.twcr().read().totalh(),
            ltdc.twcr().read().totalw(),
        );
        defmt::info!(
            "LTDC: layer CR len={}, CACR alpha={}",
            layer.cr().read().len() as u8,
            layer.cacr().read().consta(),
        );
    }
}

impl<'d> DisplayCtrl<'d> {
    /// Create a display controller in portrait orientation with ARGB8888 format.
    ///
    /// Panics on initialization failure. Use [`try_new()`](Self::try_new) for fallible init.
    pub fn new(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
    ) -> Self {
        Self::try_new(framebuffer, ltdc, dsi_host, dsi_te, lcd_reset, hint)
            .expect("display init failed")
    }

    /// Try to create a display controller in portrait orientation with ARGB8888 format.
    ///
    /// Returns `Err(DisplayInitError)` if DSI init times out or panel init fails.
    pub fn try_new(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
    ) -> Result<Self, DisplayInitError> {
        Self::try_new_with_orientation(
            framebuffer,
            ltdc,
            dsi_host,
            dsi_te,
            lcd_reset,
            hint,
            DisplayOrientation::Portrait,
        )
    }

    /// Create a display controller with ARGB8888 format and the specified orientation.
    ///
    /// Panics on initialization failure. Use [`try_new_with_orientation()`](Self::try_new_with_orientation) for fallible init.
    pub fn new_with_orientation(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
        orientation: DisplayOrientation,
    ) -> Self {
        Self::try_new_with_orientation(
            framebuffer,
            ltdc,
            dsi_host,
            dsi_te,
            lcd_reset,
            hint,
            orientation,
        )
        .expect("display init failed")
    }

    /// Try to create a display controller with ARGB8888 format and the specified orientation.
    pub fn try_new_with_orientation(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
        orientation: DisplayOrientation,
    ) -> Result<Self, DisplayInitError> {
        DisplayCtrl::<'d, Argb8888>::try_new_internal(
            framebuffer,
            ltdc,
            dsi_host,
            dsi_te,
            lcd_reset,
            hint,
            orientation,
        )
    }
}

/// Constructor trait for [`DisplayCtrl`] with a specific pixel format.
///
/// Allows abstracting over [`Argb8888`] and [`Rgb565`] formats when calling constructors.
pub trait DisplayCtrlCtor<'d>: Sized {
    /// Create a display controller in portrait orientation. Panics on failure.
    fn new(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
    ) -> Self;

    /// Try to create a display controller in portrait orientation.
    fn try_new(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
    ) -> Result<Self, DisplayInitError>;

    /// Create a display controller with the specified orientation. Panics on failure.
    fn new_with_orientation(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
        orientation: DisplayOrientation,
    ) -> Self;

    /// Try to create a display controller with the specified orientation.
    fn try_new_with_orientation(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
        orientation: DisplayOrientation,
    ) -> Result<Self, DisplayInitError>;
}

impl<'d> DisplayCtrlCtor<'d> for DisplayCtrl<'d, Rgb565> {
    fn new(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
    ) -> Self {
        Self::try_new(framebuffer, ltdc, dsi_host, dsi_te, lcd_reset, hint)
            .expect("display init failed")
    }

    fn try_new(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
    ) -> Result<Self, DisplayInitError> {
        Self::try_new_with_orientation(
            framebuffer,
            ltdc,
            dsi_host,
            dsi_te,
            lcd_reset,
            hint,
            DisplayOrientation::Portrait,
        )
    }

    fn new_with_orientation(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
        orientation: DisplayOrientation,
    ) -> Self {
        Self::try_new_with_orientation(
            framebuffer,
            ltdc,
            dsi_host,
            dsi_te,
            lcd_reset,
            hint,
            orientation,
        )
        .expect("display init failed")
    }

    fn try_new_with_orientation(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
        orientation: DisplayOrientation,
    ) -> Result<Self, DisplayInitError> {
        DisplayCtrl::<'d, Rgb565>::try_new_internal(
            framebuffer,
            ltdc,
            dsi_host,
            dsi_te,
            lcd_reset,
            hint,
            orientation,
        )
    }
}
