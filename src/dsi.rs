//! DSI host register access and configuration helpers.
//!
//! DSI command-mode reads fail with BTA/PHY timing issues. Use
//! [`crate::BoardHint::ForceNt35510`] to skip probe.

use embassy_stm32::{dsihost, peripherals};
use embassy_time::{block_for, Duration};
use embedded_display_controller::dsi::{DsiHostCtrlIo, DsiReadCommand, DsiWriteCommand};
use stm32_metapac::dsihost::regs::{Ier0, Ier1};
use stm32_metapac::DSIHOST;

use crate::display::{DisplayInitError, DisplayOrientation, FB_HEIGHT, FB_WIDTH};
use crate::ltdc::{
    H_BACK_PORCH, H_BACK_PORCH_LANDSCAPE, H_FRONT_PORCH, H_FRONT_PORCH_LANDSCAPE, H_SYNC,
    H_SYNC_LANDSCAPE, V_BACK_PORCH, V_BACK_PORCH_LANDSCAPE, V_FRONT_PORCH, V_FRONT_PORCH_LANDSCAPE,
    V_SYNC, V_SYNC_LANDSCAPE,
};

const DSI_LANE_BYTE_CLK_KHZ: u32 = 62_500;
const LTDC_PIXEL_CLK_KHZ: u32 = 27_429;
const TX_ESCAPE_CKDIV: u8 = (DSI_LANE_BYTE_CLK_KHZ / 15_620) as u8;

fn scaled_dsi_cycles(pixels: u16) -> u16 {
    ((pixels as u32 * DSI_LANE_BYTE_CLK_KHZ) / LTDC_PIXEL_CLK_KHZ) as u16
}

pub(crate) fn configure_dsi_host(
    dsi: &mut dsihost::DsiHost<'_, peripherals::DSIHOST>,
    orientation: DisplayOrientation,
    color_coding: u8,
) -> Result<(), DisplayInitError> {
    dsi.disable_wrapper_dsi();
    dsi.disable();

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
    if !DSIHOST.wisr().read().rrs() {
        return Err(DisplayInitError::DsiTimeout);
    }

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
    if !DSIHOST.wisr().read().pllls() {
        return Err(DisplayInitError::DsiTimeout);
    }

    const VS_POLARITY: bool = false;
    const HS_POLARITY: bool = false;
    const DE_POLARITY: bool = false;
    const MODE: u8 = 2;
    const NULL_PACKET_SIZE: u16 = 0x0FFF;
    const NUMBER_OF_CHUNKS: u16 = 0;
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
    DSIHOST.vpcr().modify(|w| w.set_vpsize(fb_width));
    DSIHOST.vccr().modify(|w| w.set_numc(NUMBER_OF_CHUNKS));
    DSIHOST.vnpcr().modify(|w| w.set_npsize(NULL_PACKET_SIZE));
    DSIHOST.lvcidr().modify(|w| w.set_vcid(0));

    DSIHOST.lpcr().modify(|w| {
        w.set_dep(DE_POLARITY);
        w.set_hsp(HS_POLARITY);
        w.set_vsp(VS_POLARITY);
    });

    DSIHOST.lcolcr().modify(|w| w.set_colc(color_coding));
    DSIHOST.wcfgr().modify(|w| w.set_colmux(color_coding));

    DSIHOST
        .vhsacr()
        .modify(|w| w.set_hsa(scaled_dsi_cycles(h_sync)));
    DSIHOST
        .vhbpcr()
        .modify(|w| w.set_hbp(scaled_dsi_cycles(h_back_porch)));
    DSIHOST.vlcr().modify(|w| {
        w.set_hline(scaled_dsi_cycles(
            fb_width + h_sync + h_back_porch + h_front_porch,
        ))
    });
    DSIHOST.vvsacr().modify(|w| w.set_vsa(v_sync));
    DSIHOST.vvbpcr().modify(|w| w.set_vbp(v_back_porch));
    DSIHOST.vvfpcr().modify(|w| w.set_vfp(v_front_porch));
    DSIHOST.vvacr().modify(|w| w.set_va(fb_height));

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
    Ok(())
}

pub(crate) struct DsiHostAdapter<'a, 'd> {
    dsi: &'a mut dsihost::DsiHost<'d, peripherals::DSIHOST>,
}

impl<'a, 'd> DsiHostAdapter<'a, 'd> {
    pub(crate) fn new(dsi: &'a mut dsihost::DsiHost<'d, peripherals::DSIHOST>) -> Self {
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
