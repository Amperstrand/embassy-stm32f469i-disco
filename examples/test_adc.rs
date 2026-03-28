#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use core::sync::atomic::{AtomicUsize, Ordering};

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

unsafe fn adc1_read_channel(channel: u8) -> u16 {
    let adc = stm32_metapac::ADC1;

    adc.cr2().modify(|w| {
        w.set_adon(false);
        w.set_cont(false);
        w.set_extsel(0);
        w.set_exten(stm32_metapac::adc::vals::Exten::DISABLED);
        w.set_align(stm32_metapac::adc::vals::Align::RIGHT);
    });

    adc.cr1().modify(|w| {
        w.set_res(stm32_metapac::adc::vals::Res::BITS12);
        w.set_scan(false);
    });

    adc.sqr1().write(|w| {
        w.set_l(0);
        w.set_sq(0, 0);
    });
    adc.sqr3().write(|w| {
        w.set_sq(0, channel);
    });

    adc.smpr2().write(|w| {
        w.set_smp(channel as usize, stm32_metapac::adc::vals::SampleTime::CYCLES480);
    });

    adc.cr2().modify(|w| w.set_adon(true));
    cortex_m::asm::delay(3);

    adc.cr2().modify(|w| w.set_swstart(true));
    while !adc.sr().read().eoc() {}

    let val = adc.dr().read().0 as u16;
    val
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let _p = embassy_stm32::init(Config::default());

    defmt::info!("=== ADC Test Suite ===");

    unsafe {
        stm32_metapac::RCC.apb2enr().modify(|w| w.set_adc1en(true));
    }

    // Test 1: Read temperature sensor (channel 18 on F469)
    defmt::info!("TEST adc_temp_read: RUNNING");
    {
        unsafe {
            stm32_metapac::ADC123_COMMON.ccr().modify(|w| {
                w.set_tsvrefe(true);
            });
        }
        cortex_m::asm::delay(10_000);

        let sample = unsafe { adc1_read_channel(18) };
        defmt::info!("  ADC temp raw: {}", sample);
        // 12-bit ADC, room temp typically 500-800
        if sample > 100 && sample < 4095 {
            pass("adc_temp_read");
        } else {
            fail("adc_temp_read", "out of range");
        }
    }

    // Test 2: Read VREFINT (channel 17)
    defmt::info!("TEST adc_vrefint_read: RUNNING");
    {
        cortex_m::asm::delay(10_000);

        let sample = unsafe { adc1_read_channel(17) };
        defmt::info!("  ADC vrefint raw: {}", sample);
        // VREFINT ~1.21V / 3.3V * 4095 ≈ 1500
        if sample > 500 && sample < 3000 {
            pass("adc_vrefint_read");
        } else {
            fail("adc_vrefint_read", "out of range");
        }
    }

    unsafe {
        stm32_metapac::ADC123_COMMON.ccr().modify(|w| {
            w.set_tsvrefe(false);
        });
    }

    // Summary
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("=== ADC Test Summary ===");
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
