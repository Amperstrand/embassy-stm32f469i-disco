//! Display subsystem: SDRAM controller (FMC), DSI/LTDC display driver (NT35510/OTM8009A),
//! panel detection, and framebuffer management.

use embassy_stm32::gpio::{AfType, Flex, OutputType, Pull, Speed};
use embassy_stm32::ltdc::{
    Ltdc, LtdcConfiguration, LtdcLayer, LtdcLayerConfig, PixelFormat, PolarityActive, PolarityEdge,
};
use embassy_stm32::rcc;
use embassy_stm32::{dsihost, peripherals, Peri};
use embassy_time::{block_for, Duration};
use embedded_display_controller::dsi::{DsiHostCtrlIo, DsiReadCommand, DsiWriteCommand};
use embedded_graphics::{
    draw_target::DrawTarget, pixelcolor::Rgb565, prelude::*, primitives::Rectangle,
};
use embedded_hal::delay::DelayNs;
use nt35510::Nt35510;
#[cfg(feature = "display")]
use otm8009a::Otm8009A;
use stm32_fmc::devices::is42s32400f_6::Is42s32400f6;
use stm32_fmc::{FmcPeripheral, Sdram, SdramTargetBank};

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

pub const SDRAM_SIZE_BYTES: usize = 16 * 1024 * 1024;
pub const FB_WIDTH: u16 = 480;
pub const FB_HEIGHT: u16 = 800;
pub const FB_SIZE: usize = FB_WIDTH as usize * FB_HEIGHT as usize;

const FMC_AF12: AfType = AfType::output_pull(OutputType::PushPull, Speed::VeryHigh, Pull::Up);
const DSI_BASE: usize = 0x4001_6C00;
const LTDC_BASE: usize = 0x4001_6800;

// ── SDRAM ──────────────────────────────────────────────────────────────

struct EmbassyFmc {
    source_clock: u32,
}

unsafe impl Send for EmbassyFmc {}
unsafe impl FmcPeripheral for EmbassyFmc {
    const REGISTERS: *const () = 0xa000_0000 as *const ();
    fn enable(&mut self) {
        rcc::enable_and_reset::<embassy_stm32::peripherals::FMC>();
    }
    fn source_clock_hz(&self) -> u32 {
        self.source_clock
    }
}

fn sdram_pin(pin: embassy_stm32::Peri<'_, impl embassy_stm32::gpio::Pin>) {
    let mut flex = Flex::new(pin);
    flex.set_as_af_unchecked(12, FMC_AF12);
    core::mem::forget(flex);
}

pub struct SdramCtrl {
    mem: *mut u32,
}

impl SdramCtrl {
    pub fn new(p: &mut embassy_stm32::Peripherals, source_clock_hz: u32) -> Self {
        sdram_pin(unsafe { p.PF0.clone_unchecked() });
        sdram_pin(unsafe { p.PF1.clone_unchecked() });
        sdram_pin(unsafe { p.PF2.clone_unchecked() });
        sdram_pin(unsafe { p.PF3.clone_unchecked() });
        sdram_pin(unsafe { p.PF4.clone_unchecked() });
        sdram_pin(unsafe { p.PF5.clone_unchecked() });
        sdram_pin(unsafe { p.PF11.clone_unchecked() });
        sdram_pin(unsafe { p.PF12.clone_unchecked() });
        sdram_pin(unsafe { p.PF13.clone_unchecked() });
        sdram_pin(unsafe { p.PF14.clone_unchecked() });
        sdram_pin(unsafe { p.PF15.clone_unchecked() });
        sdram_pin(unsafe { p.PG0.clone_unchecked() });
        sdram_pin(unsafe { p.PG1.clone_unchecked() });
        sdram_pin(unsafe { p.PG4.clone_unchecked() });
        sdram_pin(unsafe { p.PG5.clone_unchecked() });
        sdram_pin(unsafe { p.PG8.clone_unchecked() });
        sdram_pin(unsafe { p.PG15.clone_unchecked() });
        sdram_pin(unsafe { p.PD0.clone_unchecked() });
        sdram_pin(unsafe { p.PD1.clone_unchecked() });
        sdram_pin(unsafe { p.PD8.clone_unchecked() });
        sdram_pin(unsafe { p.PD9.clone_unchecked() });
        sdram_pin(unsafe { p.PD10.clone_unchecked() });
        sdram_pin(unsafe { p.PD14.clone_unchecked() });
        sdram_pin(unsafe { p.PD15.clone_unchecked() });
        sdram_pin(unsafe { p.PE0.clone_unchecked() });
        sdram_pin(unsafe { p.PE1.clone_unchecked() });
        sdram_pin(unsafe { p.PE7.clone_unchecked() });
        sdram_pin(unsafe { p.PE8.clone_unchecked() });
        sdram_pin(unsafe { p.PE9.clone_unchecked() });
        sdram_pin(unsafe { p.PE10.clone_unchecked() });
        sdram_pin(unsafe { p.PE11.clone_unchecked() });
        sdram_pin(unsafe { p.PE12.clone_unchecked() });
        sdram_pin(unsafe { p.PE13.clone_unchecked() });
        sdram_pin(unsafe { p.PE14.clone_unchecked() });
        sdram_pin(unsafe { p.PE15.clone_unchecked() });
        sdram_pin(unsafe { p.PH2.clone_unchecked() });
        sdram_pin(unsafe { p.PH3.clone_unchecked() });
        sdram_pin(unsafe { p.PH8.clone_unchecked() });
        sdram_pin(unsafe { p.PH9.clone_unchecked() });
        sdram_pin(unsafe { p.PH10.clone_unchecked() });
        sdram_pin(unsafe { p.PH11.clone_unchecked() });
        sdram_pin(unsafe { p.PH12.clone_unchecked() });
        sdram_pin(unsafe { p.PH13.clone_unchecked() });
        sdram_pin(unsafe { p.PH14.clone_unchecked() });
        sdram_pin(unsafe { p.PH15.clone_unchecked() });
        sdram_pin(unsafe { p.PI0.clone_unchecked() });
        sdram_pin(unsafe { p.PI1.clone_unchecked() });
        sdram_pin(unsafe { p.PI2.clone_unchecked() });
        sdram_pin(unsafe { p.PI3.clone_unchecked() });
        sdram_pin(unsafe { p.PI4.clone_unchecked() });
        sdram_pin(unsafe { p.PI5.clone_unchecked() });
        sdram_pin(unsafe { p.PI6.clone_unchecked() });
        sdram_pin(unsafe { p.PI7.clone_unchecked() });
        sdram_pin(unsafe { p.PI9.clone_unchecked() });
        sdram_pin(unsafe { p.PI10.clone_unchecked() });
        sdram_pin(unsafe { p.PC0.clone_unchecked() });

        let fmc = EmbassyFmc {
            source_clock: source_clock_hz,
        };
        let mut sdram: Sdram<EmbassyFmc, Is42s32400f6> =
            Sdram::new_unchecked(fmc, SdramTargetBank::Bank1, Is42s32400f6 {});
        let mut delay = BusyDelay;
        let mem = sdram.init(&mut delay);
        SdramCtrl { mem }
    }

    pub fn base_address(&self) -> usize {
        self.mem as usize
    }

    pub fn subslice_mut<T>(&self, offset_bytes: usize, len: usize) -> &'static mut [T] {
        let start = (self.mem as usize) + offset_bytes;
        let end = start + len * core::mem::size_of::<T>();
        assert!(end <= (self.mem as usize) + SDRAM_SIZE_BYTES);
        unsafe { &mut *core::ptr::slice_from_raw_parts_mut(start as *mut T, len) }
    }

    #[must_use]
    pub fn test_quick(&self) -> bool {
        let words = unsafe { core::slice::from_raw_parts_mut(self.mem as *mut u32, 1024) };
        for word in words.iter_mut() {
            *word = 0xDEAD_BEEF;
        }
        for &word in words.iter() {
            if word != 0xDEAD_BEEF {
                return false;
            }
        }
        for word in words.iter_mut() {
            *word = 0;
        }
        true
    }
}

const DSI_LANE_BYTE_CLK_KHZ: u32 = 62_500;
const LTDC_PIXEL_CLK_KHZ: u32 = 27_429;
const TX_ESCAPE_CKDIV: u8 = (DSI_LANE_BYTE_CLK_KHZ / 15_620) as u8;

const H_SYNC: u16 = 2;
const H_BACK_PORCH: u16 = 34;
const H_FRONT_PORCH: u16 = 34;
const V_SYNC: u16 = 1;
const V_BACK_PORCH: u16 = 15;
const V_FRONT_PORCH: u16 = 16;

#[inline(always)]
unsafe fn reg32(base: usize, offset: usize) -> u32 {
    core::ptr::read_volatile((base + offset) as *const u32)
}

#[inline(always)]
unsafe fn reg32_set(base: usize, offset: usize, val: u32) {
    let old = core::ptr::read_volatile((base + offset) as *const u32);
    core::ptr::write_volatile((base + offset) as *mut u32, old | val);
}

#[inline(always)]
unsafe fn reg32_clear(base: usize, offset: usize, val: u32) {
    let old = core::ptr::read_volatile((base + offset) as *const u32);
    core::ptr::write_volatile((base + offset) as *mut u32, old & !val);
}

#[inline(always)]
unsafe fn reg32_write(base: usize, offset: usize, val: u32) {
    core::ptr::write_volatile((base + offset) as *mut u32, val);
}

#[inline(always)]
unsafe fn reg32_modify(base: usize, offset: usize, f: impl FnOnce(u32) -> u32) {
    let old = core::ptr::read_volatile((base + offset) as *const u32);
    core::ptr::write_volatile((base + offset) as *mut u32, f(old));
}

fn scaled_dsi_cycles(pixels: u16) -> u16 {
    ((pixels as u32 * DSI_LANE_BYTE_CLK_KHZ) / LTDC_PIXEL_CLK_KHZ) as u16
}

fn configure_dsi_host(dsi: &mut dsihost::DsiHost<'_, peripherals::DSIHOST>) {
    const WRPCR: usize = 0x430;
    const WISR: usize = 0x40C;
    const PCTLR: usize = 0xA0;
    const CLCR: usize = 0x94;
    const PCONFR: usize = 0xA4;
    const CCR: usize = 0x08;
    const WPCR0: usize = 0x418;
    const IER0: usize = 0xC4;
    const IER1: usize = 0xC8;
    const PCR: usize = 0x2C;
    const MCR: usize = 0x04;
    const WCFGR: usize = 0x400;
    const VMCR: usize = 0x38;
    const VPCR: usize = 0x3C;
    const VCCR: usize = 0x40;
    const VNPCR: usize = 0x44;
    const LVCIDR: usize = 0x0C;
    const LPCR: usize = 0x14;
    const LCOLCR: usize = 0x10;
    const VHSACR: usize = 0x48;
    const VHBPCR: usize = 0x4C;
    const VLCR: usize = 0x50;
    const VVSACR: usize = 0x54;
    const VVBPCR: usize = 0x58;
    const VVFPCR: usize = 0x5C;
    const VVACR: usize = 0x60;
    const LPMCR: usize = 0x18;
    const CLTCR: usize = 0x98;
    const DLTCR: usize = 0x9C;

    dsi.disable_wrapper_dsi();
    dsi.disable();

    unsafe {
        reg32_write(DSI_BASE, PCTLR, 0);
        reg32_clear(DSI_BASE, WRPCR, 1 << 0);
        reg32_clear(DSI_BASE, WRPCR, 1 << 24);
    }

    #[cfg(feature = "defmt")]
    defmt::info!("DSIHOST: enabling regulator");

    unsafe {
        reg32_set(DSI_BASE, WRPCR, 1 << 24);
    }
    for _ in 0..1000 {
        if unsafe { reg32(DSI_BASE, WISR) & (1 << 12) != 0 } {
            break;
        }
        block_for(Duration::from_millis(1));
    }
    assert!(
        unsafe { reg32(DSI_BASE, WISR) & (1 << 12) != 0 },
        "DSI regulator timeout"
    );

    unsafe {
        reg32_modify(DSI_BASE, WRPCR, |w| {
            (w & !(0x7f << 2 | 0x0f << 11 | 0x03 << 16)) | (125 << 2) | (0x02 << 11)
        });
        reg32_set(DSI_BASE, WRPCR, 1 << 0);
    }

    for _ in 0..1000 {
        if unsafe { reg32(DSI_BASE, WISR) & (1 << 8) != 0 } {
            break;
        }
        block_for(Duration::from_millis(1));
    }
    assert!(
        unsafe { reg32(DSI_BASE, WISR) & (1 << 8) != 0 },
        "DSI PLL lock timeout"
    );

    unsafe {
        reg32_write(DSI_BASE, PCTLR, 0b11);
        reg32_modify(DSI_BASE, CLCR, |w| w | 1);
        reg32_modify(DSI_BASE, PCONFR, |w| (w & !0x03) | 0x01);
        reg32_write(DSI_BASE, CCR, TX_ESCAPE_CKDIV as u32);
        reg32_write(DSI_BASE, WPCR0, 8);
        reg32_write(DSI_BASE, IER0, 0);
        reg32_write(DSI_BASE, IER1, 0);
        reg32_write(DSI_BASE, PCR, 1 << 2);

        reg32_clear(DSI_BASE, MCR, 1 << 0);
        reg32_clear(DSI_BASE, WCFGR, 1 << 0);
        reg32_write(
            DSI_BASE,
            VMCR,
            0x02 | (1 << 8) | (1 << 9) | (1 << 10) | (1 << 11) | (1 << 12) | (1 << 13) | (1 << 15),
        );
        reg32_write(DSI_BASE, VPCR, FB_WIDTH as u32);
        reg32_write(DSI_BASE, VCCR, 1);
        reg32_write(DSI_BASE, VNPCR, 0);
        reg32_write(DSI_BASE, LVCIDR, 0);
        reg32_write(DSI_BASE, LPCR, 0);
        reg32_write(DSI_BASE, LCOLCR, 0x00);
        reg32_modify(DSI_BASE, WCFGR, |w| w & !(0x07 << 1));

        reg32_write(DSI_BASE, VHSACR, scaled_dsi_cycles(H_SYNC) as u32);
        reg32_write(DSI_BASE, VHBPCR, scaled_dsi_cycles(H_BACK_PORCH) as u32);
        reg32_write(
            DSI_BASE,
            VLCR,
            scaled_dsi_cycles(FB_WIDTH + H_SYNC + H_BACK_PORCH + H_FRONT_PORCH) as u32,
        );
        reg32_write(DSI_BASE, VVSACR, V_SYNC as u32);
        reg32_write(DSI_BASE, VVBPCR, V_BACK_PORCH as u32);
        reg32_write(DSI_BASE, VVFPCR, V_FRONT_PORCH as u32);
        reg32_write(DSI_BASE, VVACR, FB_HEIGHT as u32);
        reg32_write(DSI_BASE, LPMCR, (64 << 8) | 64);
        reg32_write(DSI_BASE, CLTCR, (35 << 16) | 35);
        // DLTCR: MRD_TIME[14:0]=0, LP2HS_TIME[23:16]=35, HS2LP_TIME[31:24]=35
        reg32_write(DSI_BASE, DLTCR, (35 << 24) | (35 << 16));
        // PCONFR: NL[1:0]=1 (2 lanes), SW_TIME[15:8]=10 (stop wait time)
        reg32_modify(DSI_BASE, PCONFR, |w| (w & !0x03) | 0x01 | (10 << 8));
    }

    dsi.enable();
    dsi.enable_wrapper_dsi();
    block_for(Duration::from_millis(120));
}

fn configure_ltdc(ltdc: &mut Ltdc<'_, peripherals::LTDC>) {
    let config = LtdcConfiguration {
        active_width: FB_WIDTH,
        active_height: FB_HEIGHT,
        h_back_porch: H_BACK_PORCH,
        h_front_porch: H_FRONT_PORCH,
        v_back_porch: V_BACK_PORCH,
        v_front_porch: V_FRONT_PORCH,
        h_sync: H_SYNC,
        v_sync: V_SYNC,
        h_sync_polarity: PolarityActive::ActiveLow,
        v_sync_polarity: PolarityActive::ActiveLow,
        data_enable_polarity: PolarityActive::ActiveLow,
        pixel_clock_polarity: PolarityEdge::RisingEdge,
    };

    ltdc.disable();
    ltdc.init(&config);
    unsafe {
        reg32_write(LTDC_BASE, 0x34, 0x0000_00AA);
        reg32_write(LTDC_BASE, 0x24, 0x01);
        reg32_modify(LTDC_BASE, 0x18, |w| w | (1 << 0) | (1 << 1));
        reg32_write(LTDC_BASE, 0x24, 0x01);
    }
}

fn configure_ltdc_layer(ltdc: &mut Ltdc<'_, peripherals::LTDC>, fb_addr: u32) {
    let layer = LtdcLayerConfig {
        layer: LtdcLayer::Layer1,
        pixel_format: PixelFormat::RGB565,
        window_x0: 0,
        window_x1: FB_WIDTH,
        window_y0: 0,
        window_y1: FB_HEIGHT,
    };

    ltdc.init_layer(&layer, None);
    unsafe {
        reg32_write(LTDC_BASE, 0x84 + 0x28, fb_addr);
        reg32_write(LTDC_BASE, 0x24, 0x01);
        reg32_set(DSI_BASE, 0x404, 1 << 2);
    }
}

struct DsiHostAdapter<'a, 'd> {
    dsi: &'a mut dsihost::DsiHost<'d, peripherals::DSIHOST>,
}

impl<'a, 'd> DsiHostAdapter<'a, 'd> {
    const GHCR: usize = 0x6C;
    const GPDR: usize = 0x70;
    const GPSR: usize = 0x74;
    const ISR1: usize = 0xC8;

    fn new(dsi: &'a mut dsihost::DsiHost<'d, peripherals::DSIHOST>) -> Self {
        Self { dsi }
    }

    fn wait_command_fifo_empty(&self) -> Result<(), dsihost::Error> {
        for _ in 0..1000 {
            if unsafe { reg32(DSI_BASE, Self::GPSR) & (1 << 0) } != 0 {
                return Ok(());
            }
            block_for(Duration::from_millis(1));
        }
        Err(dsihost::Error::FifoTimeout)
    }

    fn raw_ghcr_write(&self, dt: u8, wclsb: u8, wcmsb: u8) {
        unsafe {
            reg32_write(
                DSI_BASE,
                Self::GHCR,
                ((dt as u32) << 24) | (wclsb as u32) | ((wcmsb as u32) << 8),
            );
        }
    }

    fn raw_dcs_short_read(&mut self, arg: u8, buf: &mut [u8]) -> Result<(), dsihost::Error> {
        if buf.len() > u16::MAX as usize {
            return Err(dsihost::Error::InvalidReadSize);
        }

        self.wait_command_fifo_empty()?;

        if buf.len() > 2 {
            self.raw_ghcr_write(
                0x37,
                (buf.len() & 0xff) as u8,
                ((buf.len() >> 8) & 0xff) as u8,
            );
            self.wait_command_fifo_empty()?;
        }

        self.raw_ghcr_write(0x06, arg, 0);

        let mut idx = 0usize;
        let mut bytes_left = buf.len();
        for _ in 0..1000 {
            if bytes_left > 0 {
                let gpsr = unsafe { reg32(DSI_BASE, Self::GPSR) };
                if gpsr & (1 << 3) == 0 {
                    let fifoword = unsafe { reg32(DSI_BASE, Self::GPDR) };
                    for b in fifoword.to_ne_bytes().iter().take(bytes_left.min(4)) {
                        buf[idx] = *b;
                        bytes_left -= 1;
                        idx += 1;
                    }
                }
                if gpsr & (1 << 6) == 0 && unsafe { reg32(DSI_BASE, Self::ISR1) & (1 << 24) } != 0 {
                    break;
                }
                block_for(Duration::from_millis(1));
            } else {
                break;
            }
        }

        if bytes_left > 0 {
            return Err(dsihost::Error::ReadError);
        }
        Ok(())
    }
}

impl DsiHostCtrlIo for DsiHostAdapter<'_, '_> {
    type Error = dsihost::Error;

    fn write(&mut self, cmd: DsiWriteCommand) -> Result<(), Self::Error> {
        match cmd {
            DsiWriteCommand::DcsShortP0 { arg } => self.dsi.write_cmd(0, arg, &[]),
            DsiWriteCommand::DcsShortP1 { arg, data } => self.dsi.write_cmd(0, arg, &[data]),
            DsiWriteCommand::DcsLongWrite { arg, data } => self.dsi.write_cmd(0, arg, data),
            DsiWriteCommand::SetMaximumReturnPacketSize(_) => Ok(()),
            DsiWriteCommand::GenericShortP0
            | DsiWriteCommand::GenericShortP1
            | DsiWriteCommand::GenericShortP2
            | DsiWriteCommand::GenericLongWrite { .. } => Ok(()),
        }
    }

    fn read(&mut self, cmd: DsiReadCommand, buf: &mut [u8]) -> Result<(), Self::Error> {
        match cmd {
            DsiReadCommand::DcsShort { arg } => {
                #[cfg(feature = "defmt")]
                defmt::info!("DsiHostAdapter::read DcsShort arg=0x{:02x}", arg);
                let result = self.raw_dcs_short_read(arg, buf);
                #[cfg(feature = "defmt")]
                if let Err(e) = &result {
                    defmt::warn!("DsiHostAdapter::read err={:?}", e);
                }
                result
            }
            DsiReadCommand::GenericShortP0
            | DsiReadCommand::GenericShortP1 { .. }
            | DsiReadCommand::GenericShortP2 { .. } => Ok(()),
        }
    }
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
    let mut delay = BusyDelay;
    let mut mismatch_count = 0u32;
    let mut first_mismatch_id: u8 = 0;

    for attempt in 0..3 {
        match panel.probe(dsi, &mut delay) {
            Ok(()) => {
                #[cfg(feature = "defmt")]
                defmt::info!("detect_panel: NT35510 detected on attempt {}", attempt + 1);
                return LcdController::Nt35510;
            }
            Err(nt35510::Error::ProbeMismatch(id)) => {
                #[cfg(feature = "defmt")]
                defmt::info!(
                    "detect_panel: attempt {} mismatch, RDID2=0x{:02x}",
                    attempt + 1,
                    id
                );
                if mismatch_count == 0 {
                    first_mismatch_id = id;
                }
                mismatch_count += 1;
            }
            Err(nt35510::Error::DsiRead) => {
                #[cfg(feature = "defmt")]
                defmt::warn!("detect_panel: attempt {} DSI read error", attempt + 1);
            }
            Err(_) => {
                #[cfg(feature = "defmt")]
                defmt::warn!("detect_panel: attempt {} unknown error", attempt + 1);
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

// ── DSI command mode helpers ───────────────────────────────────────────

fn dsi_set_lp_command_mode() {
    const SYNC_CMCR_LP_BITS: u32 = 0x010F_7F00;
    const CMCR_ARE_BIT: u32 = 1 << 1;

    unsafe {
        reg32_set(DSI_BASE, 0x41C, 1 << 22);
        reg32_modify(DSI_BASE, 0x68, |w| (w | SYNC_CMCR_LP_BITS) & !CMCR_ARE_BIT);
    }
}

fn dsi_set_hs_command_mode() {
    const SYNC_CMCR_LP_BITS: u32 = 0x010F_7F00;
    const CMCR_ARE_BIT: u32 = 1 << 1;

    unsafe {
        reg32_clear(DSI_BASE, 0x41C, 1 << 22);
        reg32_modify(DSI_BASE, 0x68, |w| (w & !SYNC_CMCR_LP_BITS) & !CMCR_ARE_BIT);
    }
}

// ── Display init (orchestrator) ────────────────────────────────────────

pub struct DisplayCtrl<'d> {
    framebuffer: &'static mut [u16],
    _ltdc: Ltdc<'d, peripherals::LTDC>,
    dsi: dsihost::DsiHost<'d, peripherals::DSIHOST>,
}

impl<'d> DisplayCtrl<'d> {
    pub fn new(
        sdram: &SdramCtrl,
        ltdc: Peri<'d, peripherals::LTDC>,
        dsi_host: Peri<'d, peripherals::DSIHOST>,
        dsi_te: Peri<'d, impl dsihost::TePin<peripherals::DSIHOST>>,
        lcd_reset: embassy_stm32::Peri<'d, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
    ) -> Self {
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
        configure_dsi_host(&mut dsi);

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: dsi init done");

        let fb_slice: &'static mut [u16] = sdram.subslice_mut(0, FB_SIZE);
        let fb_addr = fb_slice.as_mut_ptr() as u32;
        configure_ltdc(&mut ltdc);

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: ltdc init done");

        dsi_set_lp_command_mode();

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: LP mode set (before probe)");

        let controller = {
            let mut dsi_adapter = DsiHostAdapter::new(&mut dsi);
            detect_panel(&mut dsi_adapter, hint)
        };

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: detect_panel done");

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: GCR before panel = {:08x}", unsafe {
            reg32(LTDC_BASE, 0x18)
        });

        match controller {
            LcdController::Nt35510 => {
                BusyDelay.delay_ms(120);
                #[cfg(feature = "defmt")]
                defmt::info!("DC::new: starting NT35510 init");
                let mut panel = Nt35510::new();
                let mut dsi_adapter = DsiHostAdapter::new(&mut dsi);
                let mut delay = BusyDelay;
                panel
                    .init_rgb565(
                        &mut dsi_adapter,
                        &mut delay,
                        nt35510::Mode::Portrait,
                        nt35510::ColorMap::Rgb,
                    )
                    .expect("NT35510 init failed");
                #[cfg(feature = "defmt")]
                defmt::info!("DC::new: init_rgb565 done");
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
                    .expect("OTM8009A init failed");
            }
        }

        // H2: Switch DSI from LP command mode to HS command mode after panel init.
        // Matches sync BSP: force_rx_low_power(false) + AllInHighSpeed.
        dsi_set_hs_command_mode();

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: HS mode set");

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: GCR={:08x}", unsafe { reg32(LTDC_BASE, 0x18) });

        configure_ltdc_layer(&mut ltdc, fb_addr);

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: layer config done");

        DisplayCtrl {
            framebuffer: fb_slice,
            _ltdc: ltdc,
            dsi,
        }
    }

    #[must_use]
    pub fn fb(&mut self) -> FramebufferView<'_> {
        FramebufferView {
            buffer: self.framebuffer,
        }
    }
    pub fn dsi(&mut self) -> &mut dsihost::DsiHost<'d, peripherals::DSIHOST> {
        &mut self.dsi
    }
}

pub struct FramebufferView<'a> {
    buffer: &'a mut [u16],
}

impl<'a> FramebufferView<'a> {
    pub fn clear(&mut self, color: Rgb565) {
        let raw = color.into_storage();
        for pixel in self.buffer.iter_mut() {
            *pixel = raw;
        }
    }
}

impl<'a> DrawTarget for FramebufferView<'a> {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for pixel in pixels {
            let x = pixel.0.x as usize;
            let y = pixel.0.y as usize;
            if x < FB_WIDTH as usize && y < FB_HEIGHT as usize {
                self.buffer[y * FB_WIDTH as usize + x] = pixel.1.into_storage();
            }
        }
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, color: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let top = area.top_left.y.max(0) as usize;
        let bottom = (area.top_left.y + area.size.height as i32).min(FB_HEIGHT as i32) as usize;
        let left = area.top_left.x.max(0) as usize;
        let right = (area.top_left.x + area.size.width as i32).min(FB_WIDTH as i32) as usize;

        let flat_color = color.into_iter().next().unwrap_or(Rgb565::BLACK);
        let raw = flat_color.into_storage();

        for y in top..bottom {
            for x in left..right {
                self.buffer[y * FB_WIDTH as usize + x] = raw;
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.clear(color);
        Ok(())
    }
}

impl<'a> OriginDimensions for FramebufferView<'a> {
    fn size(&self) -> Size {
        Size::new(FB_WIDTH as u32, FB_HEIGHT as u32)
    }
}
