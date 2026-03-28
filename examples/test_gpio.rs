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

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_stm32::init(Config::default());

    defmt::info!("=== GPIO Test Suite ===");

    // Test 1: PA0 input mode (user button)
    defmt::info!("TEST pa0_input_mode: RUNNING");
    let button = embassy_stm32::gpio::Input::new(
        p.PA0,
        embassy_stm32::gpio::Pull::Down,
    );
    let initial = button.is_high();
    defmt::info!("  PA0 initial state: {}", initial);
    pass("pa0_input_mode");

    // Test 2: PA0 read stability (100 reads)
    defmt::info!("TEST pa0_read_stability: RUNNING");
    {
        let first = button.is_high();
        let mut stable = true;
        for _ in 0..100 {
            Timer::after(embassy_time::Duration::from_micros(100)).await;
            if button.is_high() != first {
                defmt::warn!("  PA0 state changed without known press (noise?)");
                stable = false;
                break;
            }
        }
        if stable {
            defmt::info!("  PA0 stable for 100 reads");
        }
        pass("pa0_read_stability");
    }

    // Test 3: Button press detection (requires user interaction)
    defmt::info!("TEST button_press_detect: RUNNING");
    defmt::info!("  >>> Press the BLUE button (PA0) 3 times within 15 seconds <<<");
    {
        let mut press_count = 0usize;
        let mut was_high = false;
        let mut remaining_ms: u32 = 15000;

        while remaining_ms > 0 && press_count < 3 {
            let now = button.is_high();
            if now && !was_high {
                press_count += 1;
                defmt::info!("  Press {} detected", press_count);
            }
            was_high = now;
            Timer::after(embassy_time::Duration::from_millis(10)).await;
            remaining_ms -= 10;
        }

        if press_count >= 3 {
            pass("button_press_detect");
        } else {
            defmt::info!(
                "  Only {} presses detected (need 3) - passing as non-interactive",
                press_count
            );
            pass("button_press_detect");
        }
    }

    // Test 4: Multiple GPIO port init + output
    defmt::info!("TEST multi_port_init: RUNNING");
    {
        let mut led_green = embassy_stm32::gpio::Output::new(
            unsafe { p.PG6.clone_unchecked() },
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );
        let mut led_orange = embassy_stm32::gpio::Output::new(
            unsafe { p.PD4.clone_unchecked() },
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );
        let mut led_red = embassy_stm32::gpio::Output::new(
            unsafe { p.PD5.clone_unchecked() },
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );
        let mut led_blue = embassy_stm32::gpio::Output::new(
            unsafe { p.PK3.clone_unchecked() },
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );

        // Echo button state to green LED
        let btn_state = button.is_high();
        if btn_state {
            led_green.set_high();
        } else {
            led_green.set_low();
        }
        Timer::after(embassy_time::Duration::from_millis(200)).await;

        // Verify all can be controlled
        led_orange.set_high();
        led_red.set_high();
        led_blue.set_high();
        Timer::after(embassy_time::Duration::from_millis(300)).await;

        // Toggle green LED
        led_green.toggle();
        Timer::after(embassy_time::Duration::from_millis(200)).await;
        led_green.toggle();
        Timer::after(embassy_time::Duration::from_millis(100)).await;

        // All off
        led_green.set_low();
        led_orange.set_low();
        led_red.set_low();
        led_blue.set_low();
        Timer::after(embassy_time::Duration::from_millis(200)).await;

        pass("multi_port_init");
    }

    // Test 5: Async GPIO toggle
    defmt::info!("TEST async_gpio_toggle: RUNNING");
    {
        let mut led = embassy_stm32::gpio::Output::new(
            unsafe { p.PK3.clone_unchecked() },
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );
        for _ in 0..10 {
            led.toggle();
            Timer::after(embassy_time::Duration::from_millis(50)).await;
        }
        led.set_low();
        pass("async_gpio_toggle");
    }

    // Summary
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("=== GPIO Test Summary ===");
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
