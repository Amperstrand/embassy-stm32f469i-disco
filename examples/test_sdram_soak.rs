#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::rcc::*;
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;
use embassy_stm32f469i_disco::display::SdramCtrl;
use embassy_time::{Duration, Ticker};

#[allow(unused_imports)]
use {defmt_rtt as _, panic_probe as _};

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn FMC() {
    cortex_m::asm::nop();
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
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
        divp: None,
        divq: None,
        divr: Some(PllRDiv::DIV7),
    });
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;

    let p = embassy_stm32::init(config);

    defmt::info!("=== SDRAM Soak Test ===");

    let sdram = SdramCtrl::new(
        &mut unsafe { embassy_stm32::Peripherals::steal() },
        180_000_000,
    );
    let base = sdram.base_address();
    let words = embassy_stm32f469i_disco::display::SDRAM_SIZE_BYTES / 4;
    let ram: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(base as *mut u32, words) };

    let window = 262144usize;
    let mut led = Output::new(p.PG6, Level::Low, Speed::Low);
    let mut heartbeat = Ticker::every(Duration::from_secs(1));
    let mut cycle = 0u32;

    let patterns: [u32; 4] = [0xAAAAAAAA, 0x55555555, 0xFFFFFFFF, 0x00000000];

    defmt::info!("SDRAM soak: {} words window, cycling 4 patterns", window);

    loop {
        for &pattern in patterns.iter() {
            for word in ram[..window].iter_mut() {
                *word = pattern;
            }
            let mut ok = true;
            for (i, word) in ram[..window].iter().enumerate() {
                if *word != pattern {
                    defmt::error!(
                        "SDRAM FAIL: cycle={} pattern={:#x} offset={} got={:#x}",
                        cycle,
                        pattern,
                        i,
                        word
                    );
                    ok = false;
                    break;
                }
            }
            if !ok {
                loop {
                    led.set_high();
                    embassy_time::Timer::after(Duration::from_millis(100)).await;
                    led.set_low();
                    embassy_time::Timer::after(Duration::from_millis(100)).await;
                }
            }
        }

        cycle += 1;
        if cycle.is_multiple_of(100) {
            defmt::info!("SDRAM soak: {} cycles OK", cycle);
        }

        embassy_futures::select::select(
            heartbeat.next(),
            embassy_time::Timer::after(Duration::from_millis(1)),
        )
        .await;
        led.toggle();
    }
}
