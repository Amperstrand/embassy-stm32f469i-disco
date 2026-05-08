//! LTDC timing and layer configuration helpers.

use embassy_stm32::ltdc::Ltdc;
use embassy_stm32::peripherals;
use stm32_metapac::LTDC;

use crate::display::{DisplayFormat, DisplayOrientation, FB_HEIGHT, FB_WIDTH};

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

pub(crate) fn configure_ltdc(
    ltdc: &mut Ltdc<'_, peripherals::LTDC>,
    orientation: DisplayOrientation,
) {
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

pub(crate) fn configure_ltdc_layer<F: DisplayFormat>(
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
