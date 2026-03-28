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

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_stm32::init(Config::default());

    defmt::info!("=== USB GPIO Test Suite ===");

    let mut led = embassy_stm32::gpio::Output::new(
        p.PG6,
        embassy_stm32::gpio::Level::Low,
        embassy_stm32::gpio::Speed::Low,
    );

    // Test 1: USB GPIO pins available
    // PA11 (DM) and PA12 (DP) are the USB OTG FS pins
    defmt::info!("TEST usb_gpio_pins: RUNNING");
    let _dm = embassy_stm32::gpio::Output::new(
        unsafe { p.PA11.clone_unchecked() },
        embassy_stm32::gpio::Level::Low,
        embassy_stm32::gpio::Speed::Low,
    );
    let _dp = embassy_stm32::gpio::Output::new(
        unsafe { p.PA12.clone_unchecked() },
        embassy_stm32::gpio::Level::Low,
        embassy_stm32::gpio::Speed::Low,
    );
    Timer::after(embassy_time::Duration::from_millis(100)).await;
    pass("usb_gpio_pins");

    // Test 2: USB GPIO toggle stress
    defmt::info!("TEST usb_gpio_stress: RUNNING");
    let mut dm = embassy_stm32::gpio::Output::new(
        unsafe { p.PA11.clone_unchecked() },
        embassy_stm32::gpio::Level::Low,
        embassy_stm32::gpio::Speed::Low,
    );
    for _ in 0..1000 {
        dm.toggle();
    }
    dm.set_low();
    pass("usb_gpio_stress");

    // Test 3: USB GPIO + LED coexistence
    defmt::info!("TEST usb_led_coexistence: RUNNING");
    for _ in 0..100 {
        led.toggle();
        dm.toggle();
        Timer::after(embassy_time::Duration::from_millis(10)).await;
    }
    led.set_low();
    dm.set_low();
    pass("usb_led_coexistence");

    // Summary
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("=== USB GPIO Test Summary ===");
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
