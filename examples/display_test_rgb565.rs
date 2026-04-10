#![no_std]
#![no_main]

#[no_mangle]
unsafe extern "C" fn _defmt_acquire() -> usize {
    static mut BUF: [u8; 16] = [0; 16];
    core::ptr::addr_of_mut!(BUF) as usize
}

#[no_mangle]
unsafe extern "C" fn _defmt_write(_data: *const u8, _len: usize) {}

#[no_mangle]
unsafe extern "C" fn _defmt_release(_addr: usize) {}

use embassy_executor::Spawner;
use embassy_stm32::dsihost::{self, DsiHost};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::ltdc::Ltdc;
use embassy_stm32::rcc::{
    mux, AHBPrescaler, APBPrescaler, Hse, HseMode, Pll, PllMul, PllPDiv, PllPreDiv, PllQDiv, PllRDiv, PllSource, Sysclk,
};
use embassy_stm32::time::mhz;
use embassy_stm32::{dsihost::PacketType, peripherals};
use embassy_stm32f469i_disco::display::SdramCtrl;
use embassy_time::{block_for, Duration, Timer};
use embedded_display_controller::dsi::{DsiHostCtrlIo, DsiReadCommand, DsiWriteCommand};
use embedded_hal::delay::DelayNs;
use nt35510::{ColorMap, Mode, Nt35510};
use panic_halt as _;
use stm32_metapac::dsihost::regs::{Ier0, Ier1};
use stm32_metapac::ltdc::vals::{Bf1, Bf2, Depol, Hspol, Imr, Pcpol, Pf, Vspol};
use stm32_metapac::{DSIHOST, LTDC};

const LCD_X_SIZE: u16 = 480;
const LCD_Y_SIZE: u16 = 800;

const RED_565: u16 = 0xF800;
const GREEN_565: u16 = 0x07E0;
const BLUE_565: u16 = 0x001F;
const WHITE_565: u16 = 0xFFFF;

struct BusyDelay;

impl DelayNs for BusyDelay {
    fn delay_ns(&mut self, ns: u32) {
        block_for(Duration::from_nanos(ns as u64));
    }
}

struct DsiHostAdapter<'a, 'd> {
    dsi: &'a mut DsiHost<'d, peripherals::DSIHOST>,
}

impl<'a, 'd> DsiHostAdapter<'a, 'd> {
    fn new(dsi: &'a mut DsiHost<'d, peripherals::DSIHOST>) -> Self {
        Self { dsi }
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

    fn read(&mut self, cmd: DsiReadCommand, _buf: &mut [u8]) -> Result<(), Self::Error> {
        match cmd {
            DsiReadCommand::DcsShort { .. }
            | DsiReadCommand::GenericShortP0
            | DsiReadCommand::GenericShortP1 { .. }
            | DsiReadCommand::GenericShortP2 { .. } => Ok(()),
        }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.hse = Some(Hse {
        freq: mhz(8),
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
        divp: None,
        divq: Some(PllQDiv::DIV8),
        divr: Some(PllRDiv::DIV7),
    });
    config.rcc.mux.clk48sel = mux::Clk48sel::PLLSAI1_Q;

    let mut p = embassy_stm32::init(config);
    let sdram = SdramCtrl::new(&mut p, 180_000_000);

    let mut led = Output::new(p.PG6, Level::High, Speed::Low);

    let mut reset = Output::new(p.PH7, Level::Low, Speed::High);
    block_for(Duration::from_millis(20));
    reset.set_high();
    block_for(Duration::from_millis(140));
    core::mem::forget(reset);

    let mut ltdc = Ltdc::new(p.LTDC);
    let mut dsi = DsiHost::new(p.DSIHOST, p.PJ2);

    dsi.disable_wrapper_dsi();
    dsi.disable();

    DSIHOST.pctlr().modify(|w| {
        w.set_cke(false);
        w.set_den(false)
    });
    DSIHOST.wrpcr().modify(|w| w.set_pllen(false));
    DSIHOST.wrpcr().write(|w| w.set_regen(false));
    DSIHOST.wrpcr().write(|w| w.set_regen(true));
    for _ in 0..1000 {
        if DSIHOST.wisr().read().rrs() {
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

    const LANE_BYTE_CLK_K_HZ: u16 = 62_500;
    const TX_ESCAPE_CKDIV: u8 = (LANE_BYTE_CLK_K_HZ / 15_620) as u8;

    for _ in 0..1000 {
        block_for(Duration::from_millis(1));
        if DSIHOST.wisr().read().pllls() {
            break;
        }
    }
    assert!(DSIHOST.wisr().read().pllls(), "DSI PLL lock timeout");

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

    const DSI_PIXEL_FORMAT_RGB565: u8 = 0x00;
    const HACT: u16 = LCD_X_SIZE;
    const VACT: u16 = LCD_Y_SIZE;
    const VSA: u16 = 120;
    const VBP: u16 = 150;
    const VFP: u16 = 150;
    const HSA: u16 = 2;
    const HBP: u16 = 34;
    const HFP: u16 = 34;
    const NULL_PACKET_SIZE: u16 = 0x0FFF;
    const NUMBER_OF_CHUNKS: u16 = 0;
    const PACKET_SIZE: u16 = HACT;

    DSIHOST.mcr().modify(|w| w.set_cmdm(false));
    DSIHOST.wcfgr().modify(|w| w.set_dsim(false));
    DSIHOST.vmcr().modify(|w| w.set_vmt(2));
    DSIHOST.vpcr().modify(|w| w.set_vpsize(PACKET_SIZE));
    DSIHOST.vccr().modify(|w| w.set_numc(NUMBER_OF_CHUNKS));
    DSIHOST.vnpcr().modify(|w| w.set_npsize(NULL_PACKET_SIZE));
    DSIHOST.lvcidr().modify(|w| w.set_vcid(0));
    DSIHOST.lpcr().modify(|w| {
        w.set_dep(false);
        w.set_hsp(false);
        w.set_vsp(false);
    });
    DSIHOST.lcolcr().modify(|w| w.set_colc(DSI_PIXEL_FORMAT_RGB565));
    DSIHOST.wcfgr().modify(|w| w.set_colmux(DSI_PIXEL_FORMAT_RGB565));

    DSIHOST.vhsacr().modify(|w| w.set_hsa(4));
    DSIHOST.vhbpcr().modify(|w| w.set_hbp(77));
    DSIHOST.vlcr().modify(|w| w.set_hline(1253));
    DSIHOST.vvsacr().modify(|w| w.set_vsa(VSA));
    DSIHOST.vvbpcr().modify(|w| w.set_vbp(VBP));
    DSIHOST.vvfpcr().modify(|w| w.set_vfp(VFP));
    DSIHOST.vvacr().modify(|w| w.set_va(VACT));

    DSIHOST.vmcr().modify(|w| {
        w.set_lpce(true);
        w.set_lphfpe(true);
        w.set_lphbpe(true);
        w.set_lpvae(true);
        w.set_lpvfpe(true);
        w.set_lpvbpe(true);
        w.set_lpvsae(true);
        w.set_fbtaae(false);
    });
    DSIHOST.lpmcr().modify(|w| {
        w.set_lpsize(16);
        w.set_vlpsize(0);
    });
    DSIHOST.cltcr().modify(|w| {
        w.set_hs2lp_time(35);
        w.set_lp2hs_time(35)
    });
    DSIHOST.dltcr().modify(|w| {
        w.set_hs2lp_time(35);
        w.set_lp2hs_time(35);
        w.set_mrd_time(0);
    });
    DSIHOST.pconfr().modify(|w| w.set_sw_time(10));

    ltdc.disable();
    LTDC.gcr().modify(|w| {
        w.set_hspol(Hspol::ACTIVE_HIGH);
        w.set_vspol(Vspol::ACTIVE_HIGH);
        w.set_depol(Depol::ACTIVE_LOW);
        w.set_pcpol(Pcpol::RISING_EDGE);
    });
    LTDC.sscr().modify(|w| {
        w.set_hsw(HSA - 1);
        w.set_vsh(VSA - 1);
    });
    LTDC.bpcr().modify(|w| {
        w.set_ahbp(HSA + HBP - 1);
        w.set_avbp(VSA + VBP - 1);
    });
    LTDC.awcr().modify(|w| {
        w.set_aaw(LCD_X_SIZE + HSA + HBP - 1);
        w.set_aah(VSA + VBP + VACT - 1);
    });
    LTDC.twcr().modify(|w| {
        w.set_totalw(LCD_X_SIZE + HSA + HBP + HFP - 1);
        w.set_totalh(VSA + VBP + VACT + VFP - 1);
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

    dsi.enable();
    dsi.enable_wrapper_dsi();
    block_for(Duration::from_millis(120));

    let mut panel = Nt35510::new();
    let mut dsi_adapter = DsiHostAdapter::new(&mut dsi);
    let mut delay = BusyDelay;
    panel
        .init_rgb565(&mut dsi_adapter, &mut delay, Mode::Portrait, ColorMap::Rgb)
        .expect("NT35510 RGB565 init failed");

    let fb: &'static mut [u16] = sdram.subslice_mut(0, LCD_X_SIZE as usize * LCD_Y_SIZE as usize);
    let fb_addr = fb.as_mut_ptr() as u32;
    for (i, pixel) in fb.iter_mut().enumerate() {
        let row = i / LCD_X_SIZE as usize;
        *pixel = match row {
            0..200 => RED_565,
            200..400 => GREEN_565,
            400..600 => BLUE_565,
            _ => WHITE_565,
        };
    }

    LTDC.layer(0).whpcr().write(|w| {
        w.set_whstpos(LTDC.bpcr().read().ahbp() + 1);
        w.set_whsppos(LTDC.bpcr().read().ahbp() + LCD_X_SIZE);
    });
    LTDC.layer(0).wvpcr().write(|w| {
        w.set_wvstpos(LTDC.bpcr().read().avbp() + 1);
        w.set_wvsppos(LTDC.bpcr().read().avbp() + LCD_Y_SIZE);
    });
    LTDC.layer(0).pfcr().write(|w| w.set_pf(Pf::RGB565));
    LTDC.layer(0).dccr().modify(|w| {
        w.set_dcblue(0);
        w.set_dcgreen(0);
        w.set_dcred(0);
        w.set_dcalpha(0);
    });
    LTDC.layer(0).cacr().write(|w| w.set_consta(255));
    LTDC.layer(0).bfcr().write(|w| {
        w.set_bf1(Bf1::CONSTANT);
        w.set_bf2(Bf2::CONSTANT);
    });
    LTDC.layer(0).cfbar().write(|w| w.set_cfbadd(fb_addr));
    LTDC.layer(0).cfblr().write(|w| {
        w.set_cfbp(LCD_X_SIZE * 2);
        w.set_cfbll((LCD_X_SIZE * 2) + 3);
    });
    LTDC.layer(0).cfblnr().write(|w| w.set_cfblnbr(LCD_Y_SIZE));
    LTDC.layer(0).cr().modify(|w| w.set_len(true));
    LTDC.srcr().modify(|w| w.set_imr(Imr::RELOAD));

    let mut id = [0u8; 1];
    let _ = dsi.read(0, PacketType::DcsShortPktRead(0xDA), 1, &mut id);

    loop {
        led.set_low();
        Timer::after_millis(1000).await;
        led.set_high();
        Timer::after_millis(1000).await;
    }
}
