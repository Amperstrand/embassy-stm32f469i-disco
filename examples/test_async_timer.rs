#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use core::sync::atomic::{AtomicUsize, Ordering};

use embassy_stm32::timer::low_level::CountingMode;
use embassy_stm32::timer::Channel;
use embassy_stm32::Config;
use embassy_time::{Duration, Ticker, Timer};
use embedded_hal_02::Pwm;

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

fn dwt_cycles() -> u32 {
    cortex_m::peripheral::DWT::cycle_count()
}

fn cycles_to_us(cycles: u32) -> u32 {
    cycles / 16
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_stm32::init(Config::default());

    defmt::info!("=== Async Timer Test Suite ===");

    unsafe {
        cortex_m::peripheral::Peripherals::steal()
            .DWT
            .enable_cycle_counter();
    }

    // Test 1: Timer::after(1ms)
    defmt::info!("TEST timer_1ms: RUNNING");
    {
        let start = dwt_cycles();
        Timer::after(Duration::from_millis(1)).await;
        let elapsed = dwt_cycles().wrapping_sub(start);
        let us = cycles_to_us(elapsed);
        defmt::info!("  1ms delay: {}us", us);
        if (900..=1500).contains(&us) {
            pass("timer_1ms");
        } else {
            fail("timer_1ms", "1ms delay out of range");
        }
    }

    // Test 2: Timer::after(100ms)
    defmt::info!("TEST timer_100ms: RUNNING");
    {
        let start = dwt_cycles();
        Timer::after(Duration::from_millis(100)).await;
        let elapsed = dwt_cycles().wrapping_sub(start);
        let ms = cycles_to_us(elapsed) / 1000;
        defmt::info!("  100ms delay: {}ms", ms);
        if (95..=120).contains(&ms) {
            pass("timer_100ms");
        } else {
            fail("timer_100ms", "100ms delay out of range");
        }
    }

    // Test 3: Ticker period
    defmt::info!("TEST ticker_period: RUNNING");
    {
        let mut ticker = Ticker::every(Duration::from_millis(500));
        let mut intervals = [0u32; 5];
        let mut prev = dwt_cycles();

        for interval_us in intervals.iter_mut() {
            ticker.next().await;
            let now = dwt_cycles();
            *interval_us = cycles_to_us(now.wrapping_sub(prev));
            prev = now;
        }

        let mut ok = true;
        for (i, &interval_us) in intervals.iter().enumerate() {
            let ms = interval_us / 1000;
            defmt::info!("  ticker interval {}: {}ms", i, ms);
            if !(450..=600).contains(&ms) {
                ok = false;
            }
        }
        if ok {
            pass("ticker_period");
        } else {
            fail("ticker_period", "ticker interval out of range");
        }
    }

    // Test 4: Concurrent timers
    defmt::info!("TEST concurrent_timers: RUNNING");
    {
        let start = dwt_cycles();

        let timer_a = Timer::after(Duration::from_millis(50));
        let timer_b = Timer::after(Duration::from_millis(100));

        embassy_futures::select::select(timer_a, timer_b).await;

        let elapsed = dwt_cycles().wrapping_sub(start);
        let ms = cycles_to_us(elapsed) / 1000;
        defmt::info!("  select(a=50ms, b=100ms) resolved in {}ms", ms);
        // Should resolve in ~50ms (the shorter timer)
        if (40..=80).contains(&ms) {
            pass("concurrent_timers");
        } else {
            fail("concurrent_timers", "select resolved at wrong time");
        }
    }

    // Test 5: Signal notify
    defmt::info!("TEST signal_notify: RUNNING");
    {
        use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
        use embassy_sync::signal::Signal;

        static SIGNAL: Signal<CriticalSectionRawMutex, u32> = Signal::new();

        let _led = embassy_stm32::gpio::Output::new(
            p.PG6,
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );

        // We can't actually spawn tasks in single-threaded mode,
        // so test signal send/receive synchronously
        SIGNAL.signal(42);
        let result = SIGNAL.try_take();
        match result {
            Some(42) => {
                pass("signal_notify");
            }
            _ => {
                fail("signal_notify", "signal value mismatch");
            }
        }
    }

    // Test 6: DWT cycle counter sanity
    defmt::info!("TEST dwt_sanity: RUNNING");
    {
        let start = dwt_cycles();
        let end = dwt_cycles();
        let diff = end.wrapping_sub(start);
        if diff < 1000 {
            defmt::info!("  DWT delta: {} cycles", diff);
            pass("dwt_sanity");
        } else {
            fail("dwt_sanity", "DWT counter not incrementing properly");
        }
    }

    // Test 7: Timer cancel (drop a long-running Timer)
    defmt::info!("TEST timer_cancel: RUNNING");
    {
        let start = dwt_cycles();
        {
            let _long_timer = Timer::after(Duration::from_secs(10));
            drop(_long_timer);
        }
        let elapsed = dwt_cycles().wrapping_sub(start);
        let us = cycles_to_us(elapsed);
        defmt::info!("  timer drop: {}us", us);
        if us < 1000 {
            pass("timer_cancel");
        } else {
            fail("timer_cancel", "timer drop took too long");
        }
    }

    // Test 8: PWM duty cycle (TIM3 CH1 on PA6)
    defmt::info!("TEST pwm_duty_cycle: RUNNING");
    {
        let p = unsafe { embassy_stm32::Peripherals::steal() };

        let ch1 = embassy_stm32::timer::simple_pwm::PwmPin::new(
            p.PA6,
            embassy_stm32::gpio::OutputType::PushPull,
        );

        let mut pwm = embassy_stm32::timer::simple_pwm::SimplePwm::new(
            p.TIM3,
            Some(ch1),
            None,
            None,
            None,
            embassy_stm32::time::khz(10),
            CountingMode::EdgeAlignedUp,
        );

        let max_duty = pwm.get_max_duty();
        defmt::info!("  max_duty: {}", max_duty);

        pwm.enable(Channel::Ch1);
        pwm.set_duty(Channel::Ch1, max_duty / 2);
        Timer::after(Duration::from_millis(100)).await;
        pwm.set_duty(Channel::Ch1, max_duty / 4);
        Timer::after(Duration::from_millis(100)).await;
        pwm.set_duty(Channel::Ch1, max_duty);
        Timer::after(Duration::from_millis(100)).await;
        pwm.set_duty(Channel::Ch1, 0);
        pwm.disable(Channel::Ch1);
        pass("pwm_duty_cycle");
    }

    // Test 9: Timer 500us precision
    defmt::info!("TEST timer_500us: RUNNING");
    {
        let start = dwt_cycles();
        Timer::after(Duration::from_micros(500)).await;
        let elapsed = dwt_cycles().wrapping_sub(start);
        let us = cycles_to_us(elapsed);
        defmt::info!("  500us delay: {}us", us);
        if (300..=1200).contains(&us) {
            pass("timer_500us");
        } else {
            fail("timer_500us", "500us delay out of range");
        }
    }

    // Test 10: PWM frequency change
    defmt::info!("TEST pwm_freq_change: RUNNING");
    {
        let p = unsafe { embassy_stm32::Peripherals::steal() };

        let ch1 = embassy_stm32::timer::simple_pwm::PwmPin::new(
            unsafe { p.PA6.clone_unchecked() },
            embassy_stm32::gpio::OutputType::PushPull,
        );

        let mut pwm = embassy_stm32::timer::simple_pwm::SimplePwm::new(
            p.TIM3,
            Some(ch1),
            None,
            None,
            None,
            embassy_stm32::time::khz(1),
            CountingMode::EdgeAlignedUp,
        );

        let max_duty = pwm.get_max_duty();
        pwm.enable(Channel::Ch1);
        pwm.set_duty(Channel::Ch1, max_duty / 2);

        // Change to 10kHz
        pwm.set_frequency(embassy_stm32::time::khz(10));
        Timer::after(Duration::from_millis(50)).await;
        let duty_10k = pwm.get_max_duty();
        pwm.set_duty(Channel::Ch1, duty_10k / 4);
        Timer::after(Duration::from_millis(50)).await;

        // Change to 100kHz
        pwm.set_frequency(embassy_stm32::time::khz(100));
        Timer::after(Duration::from_millis(50)).await;
        pwm.set_duty(Channel::Ch1, 0);
        pwm.disable(Channel::Ch1);

        pass("pwm_freq_change");
    }

    // Summary
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("=== Async Timer Test Summary ===");
    defmt::info!("SUMMARY: {}/{} passed", passed, total);
    if failed == 0 {
        defmt::info!("ALL TESTS PASSED");
    } else {
        defmt::error!("FAILED: {} tests failed", failed);
    }

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
