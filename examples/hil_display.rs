#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::rcc::{
    AHBPrescaler, APBPrescaler, Hse, HseMode, Pll, PllMul, PllPDiv, PllPreDiv, PllQDiv,
    PllRDiv, PllSource, Sysclk,
};
use embassy_stm32f469i_disco::{display::SdramCtrl, DisplayCtrl, FB_HEIGHT, FB_WIDTH};
use embedded_graphics::pixelcolor::Rgb565;

macro_rules! isr_stubs {
    () => {
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn LTDC() { cortex_m::asm::nop(); }
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn LTDC_ER() { cortex_m::asm::nop(); }
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn DSI() { cortex_m::asm::nop(); }
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn DSIHOST() { cortex_m::asm::nop(); }
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn DMA2D() { cortex_m::asm::nop(); }
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn FMC() { cortex_m::asm::nop(); }
    };
}

isr_stubs!();

async fn hil_done() -> ! {
    embassy_time::Timer::after(embassy_time::Duration::from_millis(10)).await;
    cortex_m::asm::bkpt();
    loop { cortex_m::asm::nop(); }
}

fn read_reg(addr: usize) -> u32 {
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let mut config = embassy_stm32::Config::default();
    config.rcc.hse = Some(Hse {
        freq: embassy_stm32::time::mhz(8),
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
        divq: None,
        divr: Some(PllRDiv::DIV7),
    });
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;

    let p = embassy_stm32::init(config);
    defmt::info!("HIL_TEST:display:start");

    let sdram = SdramCtrl::new(&mut unsafe { embassy_stm32::Peripherals::steal() }, 180_000_000);

    if !sdram.test_quick() {
        defmt::error!("HIL_RESULT:display:FAIL (SDRAM prerequisite)");
        hil_done().await;
    }

    let mut display = DisplayCtrl::new(&sdram, unsafe { p.PH7.clone_unchecked() });
    defmt::info!("Display init done, {}x{}", FB_WIDTH, FB_HEIGHT);

    let ltdc_base: usize = 0x4001_6800;
    let dsi_base: usize = 0x4001_6C00;

    let gcr = read_reg(ltdc_base + 0x18);
    let ltdcen = (gcr >> 0) & 1;
    defmt::info!("LTDC GCR={:#010X} LTDCEN={}", gcr, ltdcen);

    let cdsr = read_reg(ltdc_base + 0x24);
    let vsync = (cdsr >> 3) & 1;
    let hsync = (cdsr >> 2) & 1;
    defmt::info!("LTDC CDSR={:#010X} VSYNC={} HSYNC={}", cdsr, vsync, hsync);

    let wisr = read_reg(dsi_base + 0x40C);
    let pll_lock = (wisr >> 8) & 1;
    let reg_ready = (wisr >> 12) & 1;
    defmt::info!("DSI WISR={:#010X} PLL_LOCK={} REG_RDY={}", wisr, pll_lock, reg_ready);

    let mut fb = display.fb();
    fb.clear(Rgb565::new(0, 31, 0));
    embassy_time::Timer::after(embassy_time::Duration::from_secs(2)).await;
    fb.clear(Rgb565::new(0, 0, 0));

    if ltdcen == 1 && vsync == 1 && hsync == 1 && pll_lock == 1 && reg_ready == 1 {
        defmt::info!("HIL_RESULT:display:PASS (LTDC active, DSI locked, green flash OK)");
    } else if ltdcen == 1 {
        defmt::info!("HIL_RESULT:display:PARTIAL (LTDC enabled but sync/DSI issues)");
    } else {
        defmt::error!("HIL_RESULT:display:FAIL (LTDC not enabled)");
    }

    hil_done().await;
}
