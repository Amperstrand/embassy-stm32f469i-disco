//! Display subsystem: SDRAM controller (FMC), DSI/LTDC display driver (NT35510/OTM8009A),
//! panel detection, and framebuffer management.

use embassy_stm32::ltdc::Ltdc;
use embassy_stm32::{dsihost, peripherals, Peri};
use embassy_time::{block_for, Duration};
use embedded_display_controller::dsi::DsiHostCtrlIo;
use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::{Rgb565 as EgRgb565, Rgb888, RgbColor},
    prelude::*,
    primitives::Rectangle,
};
use embedded_hal::delay::DelayNs;
use nt35510::Nt35510;
#[cfg(feature = "display")]
use otm8009a::Otm8009A;
use stm32_metapac::LTDC;

use crate::dsi::{configure_dsi_host, DsiHostAdapter};
pub use crate::sdram::{SdramCtrl, SDRAM_SIZE_BYTES};

#[cfg(feature = "defmt")]
use defmt as _;

struct BusyDelay;

impl DelayNs for BusyDelay {
    fn delay_ns(&mut self, ns: u32) {
        block_for(Duration::from_nanos(ns as u64));
    }
}

/// Adapter for OTM8009A (requires embedded-hal 0.2 blocking delay traits).
#[cfg(feature = "display")]
struct DelayMsAdapter;

#[cfg(feature = "display")]
impl embedded_hal_02::blocking::delay::DelayMs<u32> for DelayMsAdapter {
    fn delay_ms(&mut self, ms: u32) {
        block_for(Duration::from_millis(ms as u64));
    }
}

/// Panel height in pixels (portrait). Re-exported from nt35510.
pub use nt35510::PANEL_HEIGHT as FB_HEIGHT;
/// Panel width in pixels (portrait). Re-exported from nt35510.
pub use nt35510::PANEL_WIDTH as FB_WIDTH;

pub const FB_SIZE: usize = FB_WIDTH as usize * FB_HEIGHT as usize;

pub trait DisplayFormat: Copy + 'static {
    type Pixel: Copy + PartialEq;

    type Color: RgbColor;

    fn ltdc_pf() -> u8;

    fn dsi_color_coding() -> u8;

    fn nt35510_color_format() -> nt35510::ColorFormat;

    fn bpp() -> usize;

    fn encode(color: Self::Color) -> Self::Pixel;

    fn black() -> Self::Pixel;

    fn default_color() -> Self::Color;
}

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
pub enum DisplayInitError {
    /// DSI host initialization timed out (regulator or PLL).
    DsiTimeout,
    /// DSI command write to the panel failed.
    DsiWrite,
    /// Panel initialization sequence failed.
    PanelInit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DisplayOrientation {
    Portrait,
    Landscape,
}

impl DisplayOrientation {
    pub const fn width(self) -> u16 {
        match self {
            DisplayOrientation::Portrait => FB_WIDTH,
            DisplayOrientation::Landscape => FB_HEIGHT,
        }
    }

    pub const fn height(self) -> u16 {
        match self {
            DisplayOrientation::Portrait => FB_HEIGHT,
            DisplayOrientation::Landscape => FB_WIDTH,
        }
    }

    pub const fn fb_size(self) -> usize {
        (self.width() as usize) * (self.height() as usize)
    }

    pub const fn nt35510_mode(self) -> nt35510::Mode {
        match self {
            DisplayOrientation::Portrait => nt35510::Mode::Portrait,
            DisplayOrientation::Landscape => nt35510::Mode::Landscape,
        }
    }
}

// NOTE: These timing values differ from nt35510::PanelTiming::STANDARD_PORTRAIT
// (V_SYNC=1, V_BACK_PORCH=15, V_FRONT_PORCH=16). The embassy BSP uses a
// manual PLLSAI configuration producing ~54MHz pixel clock, which requires
// larger vertical blanking intervals. These values are NOT interchangeable
// with the standard timing — switching would break DSI/LTDC synchronization.
pub(crate) const H_SYNC: u16 = 2;
pub(crate) const H_BACK_PORCH: u16 = 34;
pub(crate) const H_FRONT_PORCH: u16 = 34;
pub(crate) const V_SYNC: u16 = 120;
pub(crate) const V_BACK_PORCH: u16 = 150;
pub(crate) const V_FRONT_PORCH: u16 = 150;

pub(crate) const H_SYNC_LANDSCAPE: u16 = 120;
pub(crate) const H_BACK_PORCH_LANDSCAPE: u16 = 150;
pub(crate) const H_FRONT_PORCH_LANDSCAPE: u16 = 150;
pub(crate) const V_SYNC_LANDSCAPE: u16 = 2;
pub(crate) const V_BACK_PORCH_LANDSCAPE: u16 = 34;
pub(crate) const V_FRONT_PORCH_LANDSCAPE: u16 = 34;

fn configure_ltdc(ltdc: &mut Ltdc<'_, peripherals::LTDC>, orientation: DisplayOrientation) {
    use stm32_metapac::ltdc::vals::{Depol, Hspol, Pcpol, Vspol};

    let (
        h_sync,
        h_back_porch,
        h_front_porch,
        v_sync,
        v_back_porch,
        v_front_porch,
        fb_width,
        fb_height,
    ) = match orientation {
        DisplayOrientation::Portrait => (
            H_SYNC,
            H_BACK_PORCH,
            H_FRONT_PORCH,
            V_SYNC,
            V_BACK_PORCH,
            V_FRONT_PORCH,
            FB_WIDTH,
            FB_HEIGHT,
        ),
        DisplayOrientation::Landscape => (
            H_SYNC_LANDSCAPE,
            H_BACK_PORCH_LANDSCAPE,
            H_FRONT_PORCH_LANDSCAPE,
            V_SYNC_LANDSCAPE,
            V_BACK_PORCH_LANDSCAPE,
            V_FRONT_PORCH_LANDSCAPE,
            FB_HEIGHT,
            FB_WIDTH,
        ),
    };

    ltdc.disable();
    LTDC.gcr().modify(|w| {
        w.set_hspol(Hspol::ACTIVE_HIGH);
        w.set_vspol(Vspol::ACTIVE_HIGH);
        w.set_depol(Depol::ACTIVE_LOW);
        w.set_pcpol(Pcpol::RISING_EDGE);
    });
    LTDC.sscr().modify(|w| {
        w.set_hsw(h_sync - 1);
        w.set_vsh(v_sync - 1);
    });
    LTDC.bpcr().modify(|w| {
        w.set_ahbp(h_sync + h_back_porch - 1);
        w.set_avbp(v_sync + v_back_porch - 1);
    });
    LTDC.awcr().modify(|w| {
        w.set_aah(v_sync + v_back_porch + fb_height - 1);
        w.set_aaw(fb_width + h_sync + h_back_porch - 1);
    });
    LTDC.twcr().modify(|w| {
        w.set_totalh(v_sync + v_back_porch + fb_height + v_front_porch - 1);
        w.set_totalw(fb_width + h_sync + h_back_porch + h_front_porch - 1);
    });
    LTDC.bccr().modify(|w| {
        w.set_bcred(0);
        w.set_bcgreen(0);
        w.set_bcblue(0);
    });
    LTDC.ier().modify(|w| {
        w.set_terrie(true);
        w.set_fuie(true);
    });
    ltdc.enable();
}

fn configure_ltdc_layer<F: DisplayFormat>(
    _ltdc: &mut Ltdc<'_, peripherals::LTDC>,
    fb_addr: u32,
    orientation: DisplayOrientation,
) {
    use stm32_metapac::ltdc::vals::{Bf1, Bf2, Imr, Pf};

    let window_x1 = orientation.width();
    let window_y1 = orientation.height();
    const ALPHA: u8 = 255;
    const ALPHA0: u8 = 0;
    let pixel_format = match F::ltdc_pf() {
        0 => Pf::ARGB8888,
        1 => Pf::RGB888,
        2 => Pf::RGB565,
        3 => Pf::ARGB1555,
        4 => Pf::ARGB4444,
        _ => Pf::ARGB8888,
    };
    let pixel_size = F::bpp() as u16;

    LTDC.layer(0).whpcr().write(|w| {
        w.set_whstpos(LTDC.bpcr().read().ahbp() + 1);
        w.set_whsppos(LTDC.bpcr().read().ahbp() + window_x1);
    });
    LTDC.layer(0).wvpcr().write(|w| {
        w.set_wvstpos(LTDC.bpcr().read().avbp() + 1);
        w.set_wvsppos(LTDC.bpcr().read().avbp() + window_y1);
    });
    LTDC.layer(0).pfcr().write(|w| w.set_pf(pixel_format));
    LTDC.layer(0).dccr().modify(|w| {
        w.set_dcblue(0);
        w.set_dcgreen(0);
        w.set_dcred(0);
        w.set_dcalpha(ALPHA0);
    });
    LTDC.layer(0).cacr().write(|w| w.set_consta(ALPHA));
    LTDC.layer(0).bfcr().write(|w| {
        w.set_bf1(Bf1::CONSTANT);
        w.set_bf2(Bf2::CONSTANT);
    });
    LTDC.layer(0).cfbar().write(|w| w.set_cfbadd(fb_addr));
    LTDC.layer(0).cfblr().write(|w| {
        w.set_cfbp(window_x1 * pixel_size);
        w.set_cfbll((window_x1 * pixel_size) + 3);
    });
    LTDC.layer(0).cfblnr().write(|w| w.set_cfblnbr(window_y1));
    LTDC.layer(0).cr().modify(|w| w.set_len(true));
    LTDC.srcr().modify(|w| w.set_imr(Imr::RELOAD));
}

fn framebuffer_from_bytes<F: DisplayFormat>(
    bytes: &'static mut [u8],
    len_pixels: usize,
) -> &'static mut [F::Pixel] {
    let required_bytes = len_pixels * F::bpp();
    assert!(bytes.len() >= required_bytes);
    assert_eq!(
        (bytes.as_mut_ptr() as usize) % core::mem::align_of::<F::Pixel>(),
        0
    );

    unsafe { &mut *core::ptr::slice_from_raw_parts_mut(bytes.as_mut_ptr().cast(), len_pixels) }
}

// ── Display panel detection ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcdController {
    Nt35510,
    Otm8009a,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardHint {
    Auto,
    ForceNt35510,
    ForceOtm8009a,
}

pub fn detect_panel(dsi: &mut impl DsiHostCtrlIo, hint: BoardHint) -> LcdController {
    if hint == BoardHint::ForceNt35510 {
        return LcdController::Nt35510;
    }
    if hint == BoardHint::ForceOtm8009a {
        return LcdController::Otm8009a;
    }

    let mut panel = Nt35510::new();
    let mut mismatch_count = 0u32;
    let mut first_mismatch_id: u8 = 0;

    for _attempt in 0..3 {
        match panel.probe(dsi) {
            Ok(()) => {
                #[cfg(feature = "defmt")]
                defmt::info!("detect_panel: NT35510 detected on attempt {}", _attempt + 1);
                return LcdController::Nt35510;
            }
            Err(nt35510::Error::ProbeMismatch(id)) => {
                #[cfg(feature = "defmt")]
                defmt::info!(
                    "detect_panel: attempt {} mismatch, RDID2=0x{:02x}",
                    _attempt + 1,
                    id
                );
                if mismatch_count == 0 {
                    first_mismatch_id = id;
                }
                mismatch_count += 1;
            }
            Err(nt35510::Error::DsiRead) => {
                #[cfg(feature = "defmt")]
                defmt::warn!("detect_panel: attempt {} DSI read error", _attempt + 1);
            }
            Err(_) => {
                #[cfg(feature = "defmt")]
                defmt::warn!("detect_panel: attempt {} unknown error", _attempt + 1);
            }
        }
        BusyDelay.delay_ms(5);
    }

    #[cfg(feature = "defmt")]
    defmt::info!(
        "detect_panel: {} mismatches, first_id=0x{:02x} — falling back to NT35510",
        mismatch_count,
        first_mismatch_id
    );

    #[cfg(feature = "display")]
    if mismatch_count >= 2 && first_mismatch_id != 0 {
        let mut otm = Otm8009A::new();
        if otm.id_matches(dsi).unwrap_or(false) {
            return LcdController::Otm8009a;
        }
    }

    LcdController::Nt35510
}

// ── Display init (orchestrator) ────────────────────────────────────────

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
        core::mem::forget(reset_pin);

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: LCD reset done");

        let mut ltdc = Ltdc::new(ltdc);
        let mut dsi = dsihost::DsiHost::new(dsi_host, dsi_te);
        configure_dsi_host(&mut dsi, orientation, F::dsi_color_coding())?;

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: dsi init done (not yet enabled)");

        let fb_slice = framebuffer_from_bytes::<F>(framebuffer_bytes, orientation.fb_size());
        let fb_addr = fb_slice.as_mut_ptr() as u32;
        configure_ltdc(&mut ltdc, orientation);

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: ltdc init done");

        dsi.enable();
        dsi.enable_wrapper_dsi();
        block_for(Duration::from_millis(120));

        let controller = match hint {
            BoardHint::ForceOtm8009a => LcdController::Otm8009a,
            _ => LcdController::Nt35510,
        };

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: detect_panel done");

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: GCR before panel = {:08x}", LTDC.twcr().read().0);

        match controller {
            LcdController::Nt35510 => {
                BusyDelay.delay_ms(120);
                #[cfg(feature = "defmt")]
                defmt::info!("DC::new: starting NT35510 init via nt35510 crate");

                let mut panel = Nt35510::new();
                let mut dsi_adapter = DsiHostAdapter::new(&mut dsi);
                let mut delay = BusyDelay;
                let config = nt35510::Nt35510Config {
                    mode: orientation.nt35510_mode(),
                    color_map: nt35510::ColorMap::Rgb,
                    color_format: F::nt35510_color_format(),
                    cols: FB_WIDTH,
                    rows: FB_HEIGHT,
                };
                panel
                    .init_with_config(&mut dsi_adapter, &mut delay, config)
                    .map_err(|_| DisplayInitError::PanelInit)?;

                #[cfg(feature = "defmt")]
                defmt::info!("DC::new: NT35510 init done");
            }
            #[cfg(feature = "display")]
            LcdController::Otm8009a => {
                use otm8009a::{ColorMap, FrameRate, Mode, Otm8009AConfig};

                BusyDelay.delay_ms(120);
                let mut panel = Otm8009A::new();
                let mut dsi_adapter = DsiHostAdapter::new(&mut dsi);
                let mut delay = DelayMsAdapter;
                let config = Otm8009AConfig {
                    frame_rate: FrameRate::_60Hz,
                    mode: Mode::Landscape,
                    color_map: ColorMap::Rgb,
                    cols: FB_WIDTH,
                    rows: FB_HEIGHT,
                };
                panel
                    .init(&mut dsi_adapter, config, &mut delay)
                    .map_err(|_| DisplayInitError::PanelInit)?;
            }
        }

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

    #[must_use]
    pub fn fb(&mut self) -> FramebufferView<'_, F> {
        FramebufferView {
            buffer: self.framebuffer,
            width: self.orientation.width() as usize,
            height: self.orientation.height() as usize,
        }
    }
    pub fn dsi(&mut self) -> &mut dsihost::DsiHost<'d, peripherals::DSIHOST> {
        &mut self.dsi
    }
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

pub trait DisplayCtrlCtor<'d>: Sized {
    fn new(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
    ) -> Self;

    fn try_new(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
    ) -> Result<Self, DisplayInitError>;

    fn new_with_orientation(
        framebuffer: &'static mut [u8],
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
        orientation: DisplayOrientation,
    ) -> Self;

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

pub struct FramebufferView<'a, F: DisplayFormat = Argb8888> {
    buffer: &'a mut [F::Pixel],
    width: usize,
    height: usize,
}

impl<F: DisplayFormat> FramebufferView<'_, F> {
    pub fn clear(&mut self, color: F::Color) {
        let raw = F::encode(color);
        for pixel in self.buffer.iter_mut() {
            *pixel = raw;
        }
    }

    /// Fill framebuffer with 4 distinct colors (one per quarter) and verify readback.
    ///
    /// Display shows red/green/blue/yellow quarters for visual inspection.
    /// Returns mismatched pixel count (0 = pass). Useful for validating
    /// SDRAM framebuffer integrity and display alignment.
    #[cfg(feature = "defmt")]
    pub fn verify_quarter_fill(&mut self) -> usize
    where
        F::Color: embedded_graphics::pixelcolor::RgbColor,
    {
        let qh = self.height / 4;
        let colors: [F::Pixel; 4] = [
            F::encode(F::Color::RED),
            F::encode(F::Color::GREEN),
            F::encode(F::Color::BLUE),
            F::encode(F::Color::YELLOW),
        ];

        for (q, &color) in colors.iter().enumerate() {
            let start = q * qh * self.width;
            let end = if q == 3 {
                self.buffer.len()
            } else {
                (q + 1) * qh * self.width
            };
            for px in self.buffer[start..end].iter_mut() {
                *px = color;
            }
        }

        let mut mismatches = 0usize;
        for (q, &color) in colors.iter().enumerate() {
            let start = q * qh * self.width;
            let end = if q == 3 {
                self.buffer.len()
            } else {
                (q + 1) * qh * self.width
            };
            for &px in &self.buffer[start..end] {
                if px != color {
                    mismatches += 1;
                }
            }
        }
        mismatches
    }
}

impl<F: DisplayFormat> DrawTarget for FramebufferView<'_, F> {
    type Color = F::Color;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for pixel in pixels {
            let x = pixel.0.x as usize;
            let y = pixel.0.y as usize;
            if x < self.width && y < self.height {
                self.buffer[y * self.width + x] = F::encode(pixel.1);
            }
        }
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, color: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let mut colors = color.into_iter();
        let area_width = area.size.width as i32;
        let area_height = area.size.height as i32;

        for dy in 0..area_height {
            for dx in 0..area_width {
                let raw = F::encode(colors.next().unwrap_or_else(F::default_color));
                let x = area.top_left.x + dx;
                let y = area.top_left.y + dy;

                if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
                    self.buffer[y as usize * self.width + x as usize] = raw;
                }
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        FramebufferView::clear(self, color);
        Ok(())
    }
}

impl<F: DisplayFormat> OriginDimensions for FramebufferView<'_, F> {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}
