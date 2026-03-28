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

static mut SRC1: [u8; 64] = [0; 64];
static mut DST1: [u8; 64] = [0; 64];
static mut SRC2: [u8; 4096] = [0; 4096];
static mut DST2: [u8; 4096] = [0; 4096];
static mut SRC3: [u8; 256] = [0; 256];
static mut DST3: [u8; 256] = [0; 256];
static mut SRC4: [u8; 1024] = [0; 1024];
static mut DST4: [u8; 1024] = [0; 1024];

unsafe fn fill_u8(buf: *mut u8, len: usize, pattern: u8) {
    for i in 0..len {
        *buf.add(i) = (i as u8).wrapping_mul(pattern);
    }
}

unsafe fn verify_u8(src: *const u8, dst: *const u8, len: usize) -> bool {
    for i in 0..len {
        if *src.add(i) != *dst.add(i) {
            return false;
        }
    }
    true
}

unsafe fn dma2_stream0_m2m(dst: *mut u8, src: *const u8, len: usize) {
    use stm32_metapac::dma::vals;

    let dma2 = stm32_metapac::DMA2;

    dma2.st(0).cr().write(|w| {
        w.set_en(false);
        w.set_tcie(false);
        w.set_htie(false);
        w.set_teie(false);
        w.set_dir(vals::Dir::MEMORY_TO_MEMORY);
        w.set_circ(false);
        w.set_pinc(true);
        w.set_minc(true);
        w.set_psize(vals::Size::BITS8);
        w.set_msize(vals::Size::BITS8);
        w.set_pl(vals::Pl::VERY_HIGH);
        w.set_dbm(false);
    });

    dma2.st(0).fcr().write(|w| {
        w.set_dmdis(vals::Dmdis::ENABLED);
        w.set_fth(vals::Fth::FULL);
    });

    dma2.st(0).par().write_value(src as u32);
    dma2.st(0).m0ar().write_value(dst as u32);
    dma2.st(0).ndtr().write(|w| {
        w.set_ndt(len as u16);
    });

    while dma2.st(0).cr().read().en() {}

    dma2.st(0).cr().modify(|w| {
        w.set_en(true);
    });

    while !dma2.isr(0).read().tcif(0) {}

    dma2.ifcr(0).write(|w| {
        w.set_tcif(0, true);
        w.set_htif(0, true);
        w.set_feif(0, true);
        w.set_dmeif(0, true);
        w.set_teif(0, true);
    });
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let _p = embassy_stm32::init(Config::default());

    defmt::info!("=== DMA Test Suite ===");

    unsafe {
        stm32_metapac::RCC.ahb1enr().modify(|w| w.set_dma2en(true));
    }

    // Test 1: 64-byte transfer
    defmt::info!("TEST dma_64b: RUNNING");
    unsafe {
        fill_u8(SRC1.as_mut_ptr(), 64, 0xAB);
        fill_u8(DST1.as_mut_ptr(), 64, 0);
        dma2_stream0_m2m(DST1.as_mut_ptr(), SRC1.as_ptr(), 64);
        if verify_u8(SRC1.as_ptr(), DST1.as_ptr(), 64) {
            pass("dma_64b");
        } else {
            fail("dma_64b", "data mismatch");
        }
    }

    // Test 2: 4096-byte transfer
    defmt::info!("TEST dma_4096b: RUNNING");
    unsafe {
        fill_u8(SRC2.as_mut_ptr(), 4096, 1);
        fill_u8(DST2.as_mut_ptr(), 4096, 0);
        dma2_stream0_m2m(DST2.as_mut_ptr(), SRC2.as_ptr(), 4096);
        if verify_u8(SRC2.as_ptr(), DST2.as_ptr(), 4096) {
            pass("dma_4096b");
        } else {
            fail("dma_4096b", "data mismatch");
        }
    }

    // Test 3: 1024-byte transfer
    defmt::info!("TEST dma_1024b: RUNNING");
    unsafe {
        fill_u8(SRC4.as_mut_ptr(), 1024, 0xFF);
        fill_u8(DST4.as_mut_ptr(), 1024, 0);
        dma2_stream0_m2m(DST4.as_mut_ptr(), SRC4.as_ptr(), 1024);
        if verify_u8(SRC4.as_ptr(), DST4.as_ptr(), 1024) {
            pass("dma_1024b");
        } else {
            fail("dma_1024b", "data mismatch");
        }
    }

    // Test 4: Repeated transfers (10 rounds)
    defmt::info!("TEST dma_repeated: RUNNING");
    {
        let mut ok = true;
        for round in 0..10u32 {
            unsafe {
                fill_u8(SRC3.as_mut_ptr(), 256, (round & 0xFF) as u8);
                fill_u8(DST3.as_mut_ptr(), 256, 0);
                dma2_stream0_m2m(DST3.as_mut_ptr(), SRC3.as_ptr(), 256);
                if !verify_u8(SRC3.as_ptr(), DST3.as_ptr(), 256) {
                    ok = false;
                }
            }
            if !ok {
                fail("dma_repeated", "mismatch on round");
                break;
            }
        }
        if ok {
            pass("dma_repeated");
        }
    }

    // Test 5: DMA timing check
    defmt::info!("TEST dma_timing: RUNNING");
    unsafe {
        cortex_m::peripheral::Peripherals::steal()
            .DWT
            .enable_cycle_counter();
        let start = cortex_m::peripheral::DWT::cycle_count();
        fill_u8(SRC1.as_mut_ptr(), 64, 0x55);
        fill_u8(DST1.as_mut_ptr(), 64, 0);
        dma2_stream0_m2m(DST1.as_mut_ptr(), SRC1.as_ptr(), 64);
        let elapsed = cortex_m::peripheral::DWT::cycle_count().wrapping_sub(start);
        let us = elapsed / 180;
        defmt::info!("  64b M2M: {}us", us);
        if us < 10000 {
            if verify_u8(SRC1.as_ptr(), DST1.as_ptr(), 64) {
                pass("dma_timing");
            } else {
                fail("dma_timing", "data mismatch");
            }
        } else {
            fail("dma_timing", "transfer too slow");
        }
    }

    // Summary
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("=== DMA Test Summary ===");
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
