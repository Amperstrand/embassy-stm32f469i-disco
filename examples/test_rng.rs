#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use core::sync::atomic::{AtomicUsize, Ordering};

use embassy_stm32::rcc::*;
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;
use embassy_time::Timer;

static PASSED: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);

fn pass(name: &str) {
    PASSED.fetch_add(1, Ordering::Relaxed);
    defmt::info!("TEST {}: PASS", name);
}

fn fail(name: &str, reason: &str) {
    FAILED.fetch_add(1, Ordering::Relaxed);
    defmt::error!("TEST {}: FAIL {}", name, reason);
}

unsafe fn rng_init() -> bool {
    let rcc = stm32_metapac::RCC;
    rcc.ahb2enr().modify(|w| w.set_rngen(true));

    let rng = stm32_metapac::RNG;
    rng.cr().write(|w| w.set_rngen(false));
    rng.sr().modify(|w| {
        w.set_seis(false);
        w.set_ceis(false);
    });
    rng.cr().modify(|w| w.set_rngen(true));

    let mut timeout = 100_000u32;
    while !rng.sr().read().drdy() {
        timeout -= 1;
        if timeout == 0 {
            return false;
        }
    }
    let _ = rng.dr().read();
    true
}

unsafe fn rng_next_u32() -> u32 {
    let rng = stm32_metapac::RNG;
    loop {
        let sr = rng.sr().read();
        if sr.seis() | sr.ceis() {
            rng.cr().modify(|w| w.set_rngen(false));
            rng.sr().modify(|w| {
                w.set_seis(false);
                w.set_ceis(false);
            });
            rng.cr().modify(|w| w.set_rngen(true));
        } else if sr.drdy() {
            return rng.dr().read();
        }
    }
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
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL168,
        divp: Some(PllPDiv::DIV2),
        divq: Some(PllQDiv::DIV7),
        divr: None,
    });
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.mux.clk48sel = mux::Clk48sel::PLL1_Q;

    let _p = embassy_stm32::init(config);

    defmt::info!("=== RNG Test Suite ===");

    let rng_ok = unsafe { rng_init() };

    if !rng_ok {
        defmt::error!("RNG not ready (no 48MHz clock). Skipping RNG tests.");
        defmt::error!("SUMMARY: 0/3 passed");
        defmt::error!("FAILED: RNG requires 48MHz clock (use PLL config)");
        loop {
            Timer::after(embassy_time::Duration::from_secs(1)).await;
        }
    }

    // Test 1: Read 8 words, verify not all zeros
    defmt::info!("TEST rng_not_zeros: RUNNING");
    {
        let mut all_zero = true;
        for _ in 0..8 {
            let val = unsafe { rng_next_u32() };
            if val != 0 {
                all_zero = false;
                break;
            }
        }
        if !all_zero {
            pass("rng_not_zeros");
        } else {
            fail("rng_not_zeros", "all zeros");
        }
    }

    // Test 2: Read 64 words, check uniqueness
    defmt::info!("TEST rng_uniqueness: RUNNING");
    {
        let mut buf = [0u32; 64];
        for slot in buf.iter_mut() {
            *slot = unsafe { rng_next_u32() };
        }
        let mut unique = 0usize;
        let mut i = 0;
        while i < 64 {
            let mut is_unique = true;
            let mut j = 0;
            while j < i {
                if buf[j] == buf[i] {
                    is_unique = false;
                    break;
                }
                j += 1;
            }
            if is_unique {
                unique += 1;
            }
            i += 1;
        }
        defmt::info!("  unique: {}/64", unique);
        if unique >= 32 {
            pass("rng_uniqueness");
        } else {
            fail("rng_uniqueness", "low uniqueness");
        }
    }

    // Test 3: Consecutive reads differ
    defmt::info!("TEST rng_consecutive_differ: RUNNING");
    {
        let v1 = unsafe { rng_next_u32() };
        let v2 = unsafe { rng_next_u32() };
        if v1 != v2 {
            pass("rng_consecutive_differ");
        } else {
            fail("rng_consecutive_differ", "identical reads");
        }
    }

    // Summary
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("=== RNG Test Summary ===");
    defmt::info!("SUMMARY: {}/{} passed", passed, total);
    if failed == 0 {
        defmt::info!("ALL TESTS PASSED");
    } else {
        defmt::error!("FAILED: {} tests failed", failed);
    }

    loop {
        Timer::after(embassy_time::Duration::from_secs(1)).await;
    }
}
