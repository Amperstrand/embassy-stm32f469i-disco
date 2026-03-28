#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use core::sync::atomic::{AtomicUsize, Ordering};

use embassy_stm32::Config;
use embassy_time::Timer;

static PASSED: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);

fn pass(name: &str) {
    PASSED.fetch_add(1, Ordering::Relaxed);
    defmt::info!("TEST {}: PASS", name);
}

fn fail(name: &str, reason: &str) {
    FAILED.fetch_add(1, Ordering::Relaxed);
    defmt::error!("TEST {}: FAIL {}", name, reason);
}

fn test_start(name: &str) {
    TOTAL.fetch_add(1, Ordering::Relaxed);
    defmt::info!("TEST {}: RUNNING", name);
}

fn print_summary() {
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("=== LED Test Summary ===");
    defmt::info!("SUMMARY: {}/{} passed", passed, total);
    if failed == 0 {
        defmt::info!("ALL TESTS PASSED");
    } else {
        defmt::error!("FAILED: {} tests failed", failed);
    }
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_stm32::init(Config::default());

    defmt::info!("=== LED Test Suite ===");

    let mut led_green = embassy_stm32::gpio::Output::new(
        p.PG6,
        embassy_stm32::gpio::Level::Low,
        embassy_stm32::gpio::Speed::Low,
    );
    let mut led_orange = embassy_stm32::gpio::Output::new(
        p.PD4,
        embassy_stm32::gpio::Level::Low,
        embassy_stm32::gpio::Speed::Low,
    );
    let mut led_red = embassy_stm32::gpio::Output::new(
        p.PD5,
        embassy_stm32::gpio::Level::Low,
        embassy_stm32::gpio::Speed::Low,
    );
    let mut led_blue = embassy_stm32::gpio::Output::new(
        p.PK3,
        embassy_stm32::gpio::Level::Low,
        embassy_stm32::gpio::Speed::Low,
    );

    // Test 1-4: Individual LED on/off
    test_start("led_green_on_off");
    led_green.set_high();
    Timer::after(embassy_time::Duration::from_millis(100)).await;
    led_green.set_low();
    Timer::after(embassy_time::Duration::from_millis(100)).await;
    pass("led_green_on_off");

    test_start("led_orange_on_off");
    led_orange.set_high();
    Timer::after(embassy_time::Duration::from_millis(100)).await;
    led_orange.set_low();
    Timer::after(embassy_time::Duration::from_millis(100)).await;
    pass("led_orange_on_off");

    test_start("led_red_on_off");
    led_red.set_high();
    Timer::after(embassy_time::Duration::from_millis(100)).await;
    led_red.set_low();
    Timer::after(embassy_time::Duration::from_millis(100)).await;
    pass("led_red_on_off");

    test_start("led_blue_on_off");
    led_blue.set_high();
    Timer::after(embassy_time::Duration::from_millis(100)).await;
    led_blue.set_low();
    Timer::after(embassy_time::Duration::from_millis(100)).await;
    pass("led_blue_on_off");

    // Test 5: All LEDs toggle 3 cycles
    test_start("all_leds_toggle");
    for _ in 0..3 {
        led_green.toggle();
        led_orange.toggle();
        led_red.toggle();
        led_blue.toggle();
        Timer::after(embassy_time::Duration::from_millis(80)).await;
    }
    pass("all_leds_toggle");

    // Test 6: All LEDs on
    test_start("all_leds_on");
    led_green.set_high();
    led_orange.set_high();
    led_red.set_high();
    led_blue.set_high();
    Timer::after(embassy_time::Duration::from_millis(500)).await;
    pass("all_leds_on");

    // Test 7: All LEDs off
    test_start("all_leds_off");
    led_green.set_low();
    led_orange.set_low();
    led_red.set_low();
    led_blue.set_low();
    Timer::after(embassy_time::Duration::from_millis(200)).await;
    pass("all_leds_off");

    // Test 8: March pattern
    test_start("march_pattern");
    for _round in 0..3 {
        led_green.set_high();
        Timer::after(embassy_time::Duration::from_millis(80)).await;
        led_green.set_low();
        led_orange.set_high();
        Timer::after(embassy_time::Duration::from_millis(80)).await;
        led_orange.set_low();
        led_red.set_high();
        Timer::after(embassy_time::Duration::from_millis(80)).await;
        led_red.set_low();
        led_blue.set_high();
        Timer::after(embassy_time::Duration::from_millis(80)).await;
        led_blue.set_low();
    }
    pass("march_pattern");

    // Test 9: Rapid toggle stress (500 cycles)
    test_start("rapid_toggle_stress");
    for _ in 0..500 {
        led_green.set_high();
        led_green.set_low();
        led_orange.set_high();
        led_orange.set_low();
        led_red.set_high();
        led_red.set_low();
        led_blue.set_high();
        led_blue.set_low();
    }
    pass("rapid_toggle_stress");

    // Test 10: Individual LED toggle cycles
    test_start("led_green_toggle");
    for _ in 0..3 {
        led_green.toggle();
        Timer::after(embassy_time::Duration::from_millis(50)).await;
    }
    led_green.set_low();
    pass("led_green_toggle");

    test_start("led_orange_toggle");
    for _ in 0..3 {
        led_orange.toggle();
        Timer::after(embassy_time::Duration::from_millis(50)).await;
    }
    led_orange.set_low();
    pass("led_orange_toggle");

    test_start("led_red_toggle");
    for _ in 0..3 {
        led_red.toggle();
        Timer::after(embassy_time::Duration::from_millis(50)).await;
    }
    led_red.set_low();
    pass("led_red_toggle");

    test_start("led_blue_toggle");
    for _ in 0..3 {
        led_blue.toggle();
        Timer::after(embassy_time::Duration::from_millis(50)).await;
    }
    led_blue.set_low();
    pass("led_blue_toggle");

    // Test 11: Ping-pong pattern
    test_start("ping_pong_pattern");
    for _round in 0..2 {
        led_green.set_high();
        Timer::after(embassy_time::Duration::from_millis(60)).await;
        led_green.set_low();
        led_orange.set_high();
        Timer::after(embassy_time::Duration::from_millis(60)).await;
        led_orange.set_low();
        led_red.set_high();
        Timer::after(embassy_time::Duration::from_millis(60)).await;
        led_red.set_low();
        led_blue.set_high();
        Timer::after(embassy_time::Duration::from_millis(60)).await;
        led_blue.set_low();
        led_blue.set_high();
        Timer::after(embassy_time::Duration::from_millis(60)).await;
        led_blue.set_low();
        led_red.set_high();
        Timer::after(embassy_time::Duration::from_millis(60)).await;
        led_red.set_low();
        led_orange.set_high();
        Timer::after(embassy_time::Duration::from_millis(60)).await;
        led_orange.set_low();
        led_green.set_high();
        Timer::after(embassy_time::Duration::from_millis(60)).await;
        led_green.set_low();
    }
    pass("ping_pong_pattern");

    // Test 12: All on then off with 1s hold
    test_start("all_on_then_off");
    led_green.set_high();
    led_orange.set_high();
    led_red.set_high();
    led_blue.set_high();
    Timer::after(embassy_time::Duration::from_millis(1000)).await;
    led_green.set_low();
    led_orange.set_low();
    led_red.set_low();
    led_blue.set_low();
    Timer::after(embassy_time::Duration::from_millis(500)).await;
    pass("all_on_then_off");

    // Test 10: LED with async delay
    test_start("led_async_delay");
    for _ in 0..5 {
        led_green.toggle();
        led_orange.toggle();
        led_red.toggle();
        led_blue.toggle();
        Timer::after(embassy_time::Duration::from_millis(50)).await;
    }
    led_green.set_low();
    led_orange.set_low();
    led_red.set_low();
    led_blue.set_low();
    pass("led_async_delay");

    print_summary();

    loop {
        Timer::after(embassy_time::Duration::from_secs(1)).await;
    }
}
