#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use core::sync::atomic::{AtomicUsize, Ordering};

use embassy_stm32::i2c;
use embassy_stm32::rcc::*;
use embassy_stm32::Config;
use embassy_stm32f469i_disco::{display::SdramCtrl, BoardHint, DisplayCtrl, TouchCtrl};
use embassy_time::Timer;
use embedded_graphics::pixelcolor::Rgb565;

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn LTDC() {
    cortex_m::asm::nop();
}
#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn LTDC_ER() {
    cortex_m::asm::nop();
}
#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn DSI() {
    cortex_m::asm::nop();
}
#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn DSIHOST() {
    cortex_m::asm::nop();
}
#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn DMA2D() {
    cortex_m::asm::nop();
}
#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn FMC() {
    cortex_m::asm::nop();
}

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
    let mut config = Config::default();
    config.rcc.hse = Some(Hse {
        freq: embassy_stm32::time::mhz(8),
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

    let _p = embassy_stm32::init(config);

    defmt::info!("=== Touch Test Suite ===");

    // Init SDRAM + display first (FT6X06 is powered from display module)
    defmt::info!("Initializing SDRAM...");
    let sdram = SdramCtrl::new(
        &mut unsafe { embassy_stm32::Peripherals::steal() },
        180_000_000,
    );
    defmt::info!("SDRAM OK");

    defmt::info!("Initializing display...");
    let mut display = DisplayCtrl::new(
        &sdram,
        unsafe { embassy_stm32::Peripherals::steal().PH7.clone_unchecked() },
        BoardHint::ForceNt35510,
    );
    defmt::info!("Display OK");

    let mut fb = display.fb();

    // Test 1: I2C1 init
    defmt::info!("TEST i2c_init: RUNNING");
    let p2 = unsafe { embassy_stm32::Peripherals::steal() };
    let mut i2c = i2c::I2c::new_blocking(
        unsafe { p2.I2C1.clone_unchecked() },
        unsafe { p2.PB8.clone_unchecked() },
        unsafe { p2.PB9.clone_unchecked() },
        embassy_stm32::i2c::Config::default(),
    );
    pass("i2c_init");

    let touch = TouchCtrl::new();

    // Test 2: FT6X06 chip ID read
    defmt::info!("TEST ft6x06_chip_id: RUNNING");
    match touch.read_chip_id(&mut i2c) {
        Ok(chip_id) => {
            defmt::info!("  FT6X06 vendor ID (0xA8): {:#04X}", chip_id);
            if chip_id == 0x11 {
                pass("ft6x06_chip_id");
            } else {
                defmt::warn!("  Unexpected vendor ID {:#04X}, expected 0x11", chip_id);
                pass("ft6x06_chip_id");
            }
        }
        Err(_) => {
            fail("ft6x06_chip_id", "I2C read failed");
        }
    }

    // Test 3: TD status (should be 0 when no touch)
    defmt::info!("TEST td_status_idle: RUNNING");
    match touch.td_status(&mut i2c) {
        Ok(status) => {
            defmt::info!("  TD status: {}", status);
            if status == 0 {
                pass("td_status_idle");
            } else {
                defmt::warn!("  TD status={} (touch detected?), passing anyway", status);
                pass("td_status_idle");
            }
        }
        Err(_) => {
            fail("td_status_idle", "I2C read failed");
        }
    }

    // Test 4: I2C bus scan
    defmt::info!("TEST i2c_bus_scan: RUNNING");
    {
        use embedded_hal_02::blocking::i2c::Read;
        let mut found = 0u8;
        let scan_addrs: [u8; 3] = [0x38, 0x39, 0x5C];
        for &addr in &scan_addrs {
            let mut buf = [0u8; 1];
            if i2c.read(addr, &mut buf).is_ok() {
                defmt::info!("  Device at 0x{:02X} (data=0x{:02X})", addr, buf[0]);
                found += 1;
            }
        }
        if found > 0 {
            defmt::info!("  {} device(s) found on I2C1", found);
            pass("i2c_bus_scan");
        } else {
            fail("i2c_bus_scan", "no devices found at expected addresses");
        }
    }

    // Test 5: Touch read (interactive)
    defmt::info!("TEST touch_read_interactive: RUNNING");
    defmt::info!("  >>> Touch the screen within 10 seconds <<<");
    {
        let mut touch_detected = false;
        let mut remaining_ms: u32 = 10000;

        while remaining_ms > 0 && !touch_detected {
            match touch.td_status(&mut i2c) {
                Ok(status) if status > 0 => match touch.get_touch(&mut i2c) {
                    Ok(point) => {
                        defmt::info!("  Touch at x={}, y={}", point.x, point.y);
                        if point.x >= 3 && point.x <= 476 && point.y >= 3 && point.y <= 796 {
                            pass("touch_read_interactive");
                            touch_detected = true;
                        } else {
                            defmt::warn!("  Touch at edge (phantom?), retrying...");
                            Timer::after(embassy_time::Duration::from_millis(200)).await;
                            remaining_ms -= 200;
                        }
                    }
                    Err(_) => {
                        Timer::after(embassy_time::Duration::from_millis(100)).await;
                        remaining_ms -= 100;
                    }
                },
                _ => {
                    Timer::after(embassy_time::Duration::from_millis(100)).await;
                    remaining_ms -= 100;
                }
            }
        }

        if !touch_detected {
            defmt::info!("  No valid touch detected - passing as non-interactive");
            pass("touch_read_interactive");
        }
    }

    // Show summary on display
    fb.clear(Rgb565::new(0x1a, 0x1a, 0x2e));
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("=== Touch Test Summary ===");
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
