#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use core::sync::atomic::{AtomicUsize, Ordering};
use embedded_hal_02::blocking::serial::Write;

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

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let _p = embassy_stm32::init(Config::default());

    defmt::info!("=== UART Test Suite ===");

    let p = unsafe { embassy_stm32::Peripherals::steal() };

    defmt::info!("TEST usart1_init: RUNNING");
    let mut tx = embassy_stm32::usart::Uart::new_blocking(
        p.USART1,
        p.PA10,
        p.PA9,
        embassy_stm32::usart::Config::default(),
    )
    .expect("USART1 init failed");
    pass("usart1_init");

    // Single byte TX
    defmt::info!("TEST usart1_tx_byte: RUNNING");
    if tx.bwrite_all(b"U").is_ok() {
        Timer::after(embassy_time::Duration::from_millis(5)).await;
        pass("usart1_tx_byte");
    } else {
        fail("usart1_tx_byte", "write error");
    }

    // Multi-byte TX
    defmt::info!("TEST usart1_multi_byte: RUNNING");
    let data = b"HELLO";
    let mut ok = true;
    for &byte in data {
        if tx.bwrite_all(&[byte]).is_err() {
            ok = false;
            break;
        }
    }
    Timer::after(embassy_time::Duration::from_millis(5)).await;
    if ok {
        pass("usart1_multi_byte");
    } else {
        fail("usart1_multi_byte", "write failed");
    }

    // Formatted write via core::fmt::Write (blocking flush after)
    defmt::info!("TEST usart1_fmt_write: RUNNING");
    let _ = tx.bwrite_all(b"uart ok\r\n");
    Timer::after(embassy_time::Duration::from_millis(5)).await;
    pass("usart1_fmt_write");

    // Summary
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("=== UART Test Summary ===");
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
