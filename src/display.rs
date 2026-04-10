//! Display subsystem: SDRAM controller (FMC), DSI/LTDC display driver (NT35510/OTM8009A),
//! panel detection, and framebuffer management.

use embassy_stm32::gpio::{AfType, Flex, OutputType, Pull, Speed};
use embassy_stm32::ltdc::Ltdc;
use embassy_stm32::rcc;
use embassy_stm32::{dsihost, peripherals, Peri};
use embassy_time::{block_for, Duration};
use embedded_display_controller::dsi::{DsiHostCtrlIo, DsiReadCommand, DsiWriteCommand};
use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::{Rgb888, RgbColor},
    prelude::*,
    primitives::Rectangle,
};
use embedded_hal::delay::DelayNs;
use nt35510::Nt35510;
#[cfg(feature = "display")]
use otm8009a::Otm8009A;
use stm32_fmc::devices::is42s32400f_6::Is42s32400f6;
use stm32_fmc::{FmcPeripheral, Sdram, SdramTargetBank};
use stm32_metapac::dsihost::regs::{Ier0, Ier1};
use stm32_metapac::{DSIHOST, LTDC};

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
const V_SYNC: u16 = 120;
const V_BACK_PORCH: u16 = 150;
const V_FRONT_PORCH: u16 = 150;

fn scaled_dsi_cycles(pixels: u16) -> u16 {
    ((pixels as u32 * DSI_LANE_BYTE_CLK_KHZ) / LTDC_PIXEL_CLK_KHZ) as u16
}

fn configure_dsi_host(dsi: &mut dsihost::DsiHost<'_, peripherals::DSIHOST>) {
    dsi.disable_wrapper_dsi();
    dsi.disable();

    DSIHOST.pctlr().modify(|w| {
        w.set_cke(false);
        w.set_den(false)
    });

    DSIHOST.wrpcr().modify(|w| w.set_pllen(false));
    DSIHOST.wrpcr().write(|w| w.set_regen(false));

    #[cfg(feature = "defmt")]
    defmt::info!("DSIHOST: enabling regulator");

    DSIHOST.wrpcr().write(|w| w.set_regen(true));
    for _ in 0..1000 {
        if DSIHOST.wisr().read().rrs() {
            #[cfg(feature = "defmt")]
            defmt::info!("DSIHOST Regulator ready");
            break;
        }
        block_for(Duration::from_millis(1));
    }
    assert!(DSIHOST.wisr().read().rrs(), "DSI regulator timeout");

    DSIHOST.wrpcr().modify(|w| {
        w.set_pllen(true);
        w.set_ndiv(125);
        w.set_idf(2);
        w.set_odf(0);
    });

    for _ in 0..1000 {
        block_for(Duration::from_millis(1));
        if DSIHOST.wisr().read().pllls() {
            #[cfg(feature = "defmt")]
            defmt::info!("DSIHOST PLL locked");
            break;
        }
    }
    assert!(DSIHOST.wisr().read().pllls(), "DSI PLL lock timeout");

    const DSI_PIXEL_FORMAT_RGB888: u8 = 0x05;
    const COLOR_CODING: u8 = DSI_PIXEL_FORMAT_RGB888;
    const VS_POLARITY: bool = false;
    const HS_POLARITY: bool = false;
    const DE_POLARITY: bool = false;
    const MODE: u8 = 2;
    const NULL_PACKET_SIZE: u16 = 0x0FFF;
    const NUMBER_OF_CHUNKS: u16 = 0;
    const PACKET_SIZE: u16 = FB_WIDTH;
    const LP_COMMAND_ENABLE: bool = true;
    const LP_LARGEST_PACKET_SIZE: u8 = 16;
    const LPVACT_LARGEST_PACKET_SIZE: u8 = 0;
    const LPHORIZONTAL_FRONT_PORCH_ENABLE: bool = true;
    const LPHORIZONTAL_BACK_PORCH_ENABLE: bool = true;
    const LPVERTICAL_ACTIVE_ENABLE: bool = true;
    const LPVERTICAL_FRONT_PORCH_ENABLE: bool = true;
    const LPVERTICAL_BACK_PORCH_ENABLE: bool = true;
    const LPVERTICAL_SYNC_ACTIVE_ENABLE: bool = true;
    const FRAME_BTAACKNOWLEDGE_ENABLE: bool = false;
    const CLOCK_LANE_HS2_LPTIME: u16 = 35;
    const CLOCK_LANE_LP2_HSTIME: u16 = 35;
    const DATA_LANE_HS2_LPTIME: u8 = 35;
    const DATA_LANE_LP2_HSTIME: u8 = 35;
    const DATA_LANE_MAX_READ_TIME: u16 = 0;
    const STOP_WAIT_TIME: u8 = 10;
    const MAX_TIME: u16 = if CLOCK_LANE_HS2_LPTIME > CLOCK_LANE_LP2_HSTIME {
        CLOCK_LANE_HS2_LPTIME
    } else {
        CLOCK_LANE_LP2_HSTIME
    };

    DSIHOST.pctlr().write(|w| {
        w.set_cke(true);
        w.set_den(true);
    });

    DSIHOST.clcr().modify(|w| {
        w.set_dpcc(true);
        w.set_acr(false);
    });

    DSIHOST.pconfr().modify(|w| w.set_nl(1));
    DSIHOST.ccr().modify(|w| w.set_txeckdiv(TX_ESCAPE_CKDIV));
    DSIHOST.wpcr0().modify(|w| w.set_uix4(8));
    DSIHOST.ier0().write_value(Ier0(0));
    DSIHOST.ier1().write_value(Ier1(0));
    DSIHOST.pcr().modify(|w| w.set_btae(true));

    DSIHOST.mcr().modify(|w| w.set_cmdm(false));
    DSIHOST.wcfgr().modify(|w| w.set_dsim(false));

    DSIHOST.vmcr().modify(|w| w.set_vmt(MODE));
    DSIHOST.vpcr().modify(|w| w.set_vpsize(PACKET_SIZE));
    DSIHOST.vccr().modify(|w| w.set_numc(NUMBER_OF_CHUNKS));
    DSIHOST.vnpcr().modify(|w| w.set_npsize(NULL_PACKET_SIZE));
    DSIHOST.lvcidr().modify(|w| w.set_vcid(0));

    DSIHOST.lpcr().modify(|w| {
        w.set_dep(DE_POLARITY);
        w.set_hsp(HS_POLARITY);
        w.set_vsp(VS_POLARITY);
    });

    DSIHOST.lcolcr().modify(|w| w.set_colc(COLOR_CODING));
    DSIHOST.wcfgr().modify(|w| w.set_colmux(COLOR_CODING));

    DSIHOST
        .vhsacr()
        .modify(|w| w.set_hsa(scaled_dsi_cycles(H_SYNC)));
    DSIHOST
        .vhbpcr()
        .modify(|w| w.set_hbp(scaled_dsi_cycles(H_BACK_PORCH)));
    DSIHOST.vlcr().modify(|w| {
        w.set_hline(scaled_dsi_cycles(
            FB_WIDTH + H_SYNC + H_BACK_PORCH + H_FRONT_PORCH,
        ))
    });
    DSIHOST.vvsacr().modify(|w| w.set_vsa(V_SYNC));
    DSIHOST.vvbpcr().modify(|w| w.set_vbp(V_BACK_PORCH));
    DSIHOST.vvfpcr().modify(|w| w.set_vfp(V_FRONT_PORCH));
    DSIHOST.vvacr().modify(|w| w.set_va(FB_HEIGHT));

    DSIHOST.vmcr().modify(|w| w.set_lpce(LP_COMMAND_ENABLE));
    DSIHOST
        .lpmcr()
        .modify(|w| w.set_lpsize(LP_LARGEST_PACKET_SIZE));
    DSIHOST
        .lpmcr()
        .modify(|w| w.set_vlpsize(LPVACT_LARGEST_PACKET_SIZE));

    DSIHOST
        .vmcr()
        .modify(|w| w.set_lphfpe(LPHORIZONTAL_FRONT_PORCH_ENABLE));
    DSIHOST
        .vmcr()
        .modify(|w| w.set_lphbpe(LPHORIZONTAL_BACK_PORCH_ENABLE));
    DSIHOST
        .vmcr()
        .modify(|w| w.set_lpvae(LPVERTICAL_ACTIVE_ENABLE));
    DSIHOST
        .vmcr()
        .modify(|w| w.set_lpvfpe(LPVERTICAL_FRONT_PORCH_ENABLE));
    DSIHOST
        .vmcr()
        .modify(|w| w.set_lpvbpe(LPVERTICAL_BACK_PORCH_ENABLE));
    DSIHOST
        .vmcr()
        .modify(|w| w.set_lpvsae(LPVERTICAL_SYNC_ACTIVE_ENABLE));
    DSIHOST
        .vmcr()
        .modify(|w| w.set_fbtaae(FRAME_BTAACKNOWLEDGE_ENABLE));

    DSIHOST.cltcr().modify(|w| {
        w.set_hs2lp_time(MAX_TIME);
        w.set_lp2hs_time(MAX_TIME)
    });

    DSIHOST.dltcr().modify(|w| {
        w.set_hs2lp_time(DATA_LANE_HS2_LPTIME);
        w.set_lp2hs_time(DATA_LANE_LP2_HSTIME);
        w.set_mrd_time(DATA_LANE_MAX_READ_TIME);
    });

    DSIHOST.pconfr().modify(|w| w.set_sw_time(STOP_WAIT_TIME));
}

fn configure_ltdc(ltdc: &mut Ltdc<'_, peripherals::LTDC>) {
    use stm32_metapac::ltdc::vals::{Depol, Hspol, Pcpol, Vspol};

    ltdc.disable();
    LTDC.gcr().modify(|w| {
        w.set_hspol(Hspol::ACTIVE_HIGH);
        w.set_vspol(Vspol::ACTIVE_HIGH);
        w.set_depol(Depol::ACTIVE_LOW);
        w.set_pcpol(Pcpol::RISING_EDGE);
    });
    LTDC.sscr().modify(|w| {
        w.set_hsw(H_SYNC - 1);
        w.set_vsh(V_SYNC - 1);
    });
    LTDC.bpcr().modify(|w| {
        w.set_ahbp(H_SYNC + H_BACK_PORCH - 1);
        w.set_avbp(V_SYNC + V_BACK_PORCH - 1);
    });
    LTDC.awcr().modify(|w| {
        w.set_aah(V_SYNC + V_BACK_PORCH + FB_HEIGHT - 1);
        w.set_aaw(FB_WIDTH + H_SYNC + H_BACK_PORCH - 1);
    });
    LTDC.twcr().modify(|w| {
        w.set_totalh(V_SYNC + V_BACK_PORCH + FB_HEIGHT + V_FRONT_PORCH - 1);
        w.set_totalw(FB_WIDTH + H_SYNC + H_BACK_PORCH + H_FRONT_PORCH - 1);
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

fn configure_ltdc_layer(_ltdc: &mut Ltdc<'_, peripherals::LTDC>, fb_addr: u32) {
    use stm32_metapac::ltdc::vals::{Bf1, Bf2, Imr, Pf};

    const WINDOW_X0: u16 = 0;
    const WINDOW_X1: u16 = FB_WIDTH;
    const WINDOW_Y0: u16 = 0;
    const WINDOW_Y1: u16 = FB_HEIGHT;
    const ALPHA: u8 = 255;
    const ALPHA0: u8 = 0;
    const PIXEL_SIZE: u8 = 4u8;

    LTDC.layer(0).whpcr().write(|w| {
        w.set_whstpos(LTDC.bpcr().read().ahbp() + 1 + WINDOW_X0);
        w.set_whsppos(LTDC.bpcr().read().ahbp() + WINDOW_X1);
    });
    LTDC.layer(0).wvpcr().write(|w| {
        w.set_wvstpos(LTDC.bpcr().read().avbp() + 1 + WINDOW_Y0);
        w.set_wvsppos(LTDC.bpcr().read().avbp() + WINDOW_Y1);
    });
    LTDC.layer(0).pfcr().write(|w| w.set_pf(Pf::ARGB8888));
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
        w.set_cfbp(WINDOW_X1 * PIXEL_SIZE as u16);
        w.set_cfbll(((WINDOW_X1 - WINDOW_X0) * PIXEL_SIZE as u16) + 3);
    });
    LTDC.layer(0).cfblnr().write(|w| w.set_cfblnbr(WINDOW_Y1));
    LTDC.layer(0).cr().modify(|w| w.set_len(true));
    LTDC.srcr().modify(|w| w.set_imr(Imr::RELOAD));
}

struct DsiHostAdapter<'a, 'd> {
    dsi: &'a mut dsihost::DsiHost<'d, peripherals::DSIHOST>,
}

impl<'a, 'd> DsiHostAdapter<'a, 'd> {
    fn new(dsi: &'a mut dsihost::DsiHost<'d, peripherals::DSIHOST>) -> Self {
        Self { dsi }
    }

    fn wait_command_fifo_empty(&self) -> Result<(), dsihost::Error> {
        for _ in 0..1000 {
            if DSIHOST.gpsr().read().cmdfe() {
                return Ok(());
            }
            block_for(Duration::from_millis(1));
        }
        Err(dsihost::Error::FifoTimeout)
    }

    fn raw_ghcr_write(&self, dt: u8, wclsb: u8, wcmsb: u8) {
        DSIHOST.ghcr().write(|w| {
            w.set_dt(dt);
            w.set_vcid(0);
            w.set_wclsb(wclsb);
            w.set_wcmsb(wcmsb);
        });
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
                let gpsr = DSIHOST.gpsr().read();
                if !gpsr.prdfe() {
                    let gpdr = DSIHOST.gpdr().read();
                    for b in [gpdr.data1(), gpdr.data2(), gpdr.data3(), gpdr.data4()]
                        .iter()
                        .take(bytes_left.min(4))
                    {
                        buf[idx] = *b;
                        bytes_left -= 1;
                        idx += 1;
                    }
                }
                if !gpsr.rcb() && (DSIHOST.isr1().read().0 & (1 << 24)) != 0 {
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
                    defmt::warn!("DsiHostAdapter::read err={:#?}", defmt::Debug2Format(&e));
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

    for _attempt in 0..3 {
        match panel.probe(dsi, &mut delay) {
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

pub struct DisplayCtrl<'d> {
    framebuffer: &'static mut [u32],
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
        defmt::info!("DC::new: dsi init done (not yet enabled)");

        let fb_slice: &'static mut [u32] = sdram.subslice_mut(0, FB_SIZE);
        let fb_addr = fb_slice.as_mut_ptr() as u32;
        configure_ltdc(&mut ltdc);

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
                    mode: nt35510::Mode::Portrait,
                    color_map: nt35510::ColorMap::Rgb,
                    color_format: nt35510::ColorFormat::Rgb888,
                    cols: FB_WIDTH,
                    rows: FB_HEIGHT,
                };
                panel
                    .init_with_config(&mut dsi_adapter, &mut delay, config)
                    .expect("NT35510 init failed");

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
                    .expect("OTM8009A init failed");
            }
        }

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: GCR={:08x}", LTDC.twcr().read().0);

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
    buffer: &'a mut [u32],
}

impl<'a> FramebufferView<'a> {
    fn encode(color: Rgb888) -> u32 {
        0xFF00_0000 | ((color.r() as u32) << 16) | ((color.g() as u32) << 8) | (color.b() as u32)
    }

    pub fn clear(&mut self, color: Rgb888) {
        let raw = Self::encode(color);
        for pixel in self.buffer.iter_mut() {
            *pixel = raw;
        }
    }
}

impl<'a> DrawTarget for FramebufferView<'a> {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for pixel in pixels {
            let x = pixel.0.x as usize;
            let y = pixel.0.y as usize;
            if x < FB_WIDTH as usize && y < FB_HEIGHT as usize {
                self.buffer[y * FB_WIDTH as usize + x] = Self::encode(pixel.1);
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

        let flat_color = color.into_iter().next().unwrap_or(Rgb888::BLACK);
        let raw = Self::encode(flat_color);

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
