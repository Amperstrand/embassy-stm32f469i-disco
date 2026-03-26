#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::Config;
use embassy_stm32f469i_disco::display::SdramCtrl;

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

async fn test_region(name: &str, words: &mut [u32], pattern: u32) -> bool {
    for word in words.iter_mut() {
        *word = pattern;
    }
    embassy_time::Timer::after(embassy_time::Duration::from_micros(100)).await;
    for &word in words.iter() {
        if word != pattern {
            defmt::error!("HIL_RESULT:sdram_{}:FAIL", name);
            return false;
        }
    }
    for word in words.iter_mut() {
        *word = 0;
    }
    defmt::info!("HIL_RESULT:sdram_{}:PASS ({} bytes)", name, words.len() * 4);
    true
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let mut p = embassy_stm32::init(Config::default());
    defmt::info!("HIL_TEST:sdram:start");

    let sdram = SdramCtrl::new(&mut p, 180_000_000);

    if !sdram.test_quick() {
        defmt::error!("HIL_RESULT:sdram_base:FAIL");
        hil_done().await;
    }
    defmt::info!("HIL_RESULT:sdram_base:PASS (4096 bytes)");

    let fb_words = unsafe {
        let ptr = 0xC000_0000 as *mut u32;
        core::slice::from_raw_parts_mut(ptr, 480 * 800 / 2)
    };
    if !test_region("fb", fb_words, 0xDEAD_BEEF).await {
        hil_done().await;
    }

    let mid_words = unsafe {
        let ptr = (0xC000_0000u32 + 8 * 1024 * 1024) as *mut u32;
        core::slice::from_raw_parts_mut(ptr, 4096)
    };
    if !test_region("mid", mid_words, 0xCAFEBABE).await {
        hil_done().await;
    }

    hil_done().await;
}
