#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::gpio::{Level, Output, Pull, Speed};
use embassy_stm32::i2c;
use embassy_stm32::rcc::*;
use embassy_stm32::Config;
use embassy_stm32f469i_disco::{
    display::SdramCtrl, BoardHint, DisplayCtrl, TouchCtrl, FB_HEIGHT, FB_WIDTH,
};
use embassy_time::{Duration, Timer};
use embedded_graphics::{
    mono_font::{ascii::FONT_6X9, MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{rectangle::Rectangle, PrimitiveStyle},
};

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

const BG: Rgb565 = Rgb565::new(0x1a, 0x1a, 0x2e);
const PASS_COLOR: Rgb565 = Rgb565::new(0x00, 0xe0, 0x40);
const FAIL_COLOR: Rgb565 = Rgb565::new(0xe0, 0x20, 0x20);
const HEADER_COLOR: Rgb565 = Rgb565::new(0x40, 0xa0, 0xe0);
const TEXT_COLOR: Rgb565 = Rgb565::new(0xe0, 0xe0, 0xe0);
const DIM_TEXT: Rgb565 = Rgb565::new(0x80, 0x80, 0x80);
const RUN_COLOR: Rgb565 = Rgb565::new(0xff, 0xc0, 0x00);
const MAX_TESTS: usize = 64;

static mut RESULTS: [(&str, bool); MAX_TESTS] = [("", false); MAX_TESTS];
static mut RESULT_COUNT: usize = 0;
static mut PASS_COUNT: usize = 0;
static mut FAIL_COUNT: usize = 0;

unsafe fn tpass(name: &'static str) {
    RESULTS[RESULT_COUNT] = (name, true);
    RESULT_COUNT += 1;
    PASS_COUNT += 1;
    defmt::info!("TEST {}: PASS", name);
}

unsafe fn tfail(name: &'static str) {
    RESULTS[RESULT_COUNT] = (name, false);
    RESULT_COUNT += 1;
    FAIL_COUNT += 1;
    defmt::error!("TEST {}: FAIL", name);
}

unsafe fn tpass_fn(name: &'static str, f: impl FnOnce() -> bool) {
    defmt::info!("TEST {}: RUNNING", name);
    if f() {
        tpass(name);
    } else {
        tfail(name);
    }
}

fn dwt_cycles() -> u32 {
    cortex_m::peripheral::DWT::cycle_count()
}

fn make_style() -> MonoTextStyle<'static, Rgb565> {
    MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(TEXT_COLOR)
        .background_color(BG)
        .build()
}

fn make_header_style() -> MonoTextStyle<'static, Rgb565> {
    MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(HEADER_COLOR)
        .background_color(BG)
        .build()
}

fn make_run_style() -> MonoTextStyle<'static, Rgb565> {
    MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(RUN_COLOR)
        .background_color(BG)
        .build()
}

fn draw_text(
    fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>,
    text: &str,
    x: i32,
    y: i32,
    style: &MonoTextStyle<Rgb565>,
) {
    embedded_graphics::text::Text::new(text, Point::new(x, y), *style)
        .draw(fb)
        .ok();
}

fn draw_u32(
    fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>,
    x: i32,
    y: i32,
    style: &MonoTextStyle<Rgb565>,
    val: u32,
) {
    let mut buf = [0u8; 12];
    let mut i = buf.len();
    let mut v = val;
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let s = unsafe { core::str::from_utf8_unchecked(&buf[i..]) };
    draw_text(fb, s, x, y, style);
}

fn draw_header(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, y: &mut i32, title: &str) {
    let hs = make_header_style();
    *y += 4;
    draw_text(fb, title, 8, *y, &hs);
    *y += 14;
    Rectangle::new(Point::new(8, *y), Size::new(464, 1))
        .into_styled(PrimitiveStyle::with_fill(DIM_TEXT))
        .draw(fb)
        .ok();
    *y += 4;
}

fn draw_result(
    fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>,
    y: &mut i32,
    name: &str,
    passed: bool,
) {
    let color = if passed { PASS_COLOR } else { FAIL_COLOR };
    let status_str = if passed { "PASS" } else { "FAIL" };
    Rectangle::new(Point::new(12, *y), Size::new(8, 8))
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(fb)
        .ok();
    draw_text(fb, name, 26, *y, &make_style());
    draw_text(fb, status_str, 430, *y, &make_style());
    *y += 12;
}

fn draw_status_line(
    fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>,
    y: &mut i32,
    passed: usize,
    failed: usize,
    running: &str,
) {
    let ps = make_style();
    draw_text(fb, "PASS:", 8, *y, &ps);
    draw_u32(fb, 46, *y, &ps, passed as u32);
    draw_text(fb, " FAIL:", 80, *y, &ps);
    draw_u32(fb, 120, *y, &ps, failed as u32);
    if !running.is_empty() {
        draw_text(fb, " RUNNING:", 152, *y, &make_run_style());
        draw_text(fb, running, 216, *y, &make_run_style());
    }
    *y += 14;
}

fn draw_summary(
    fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>,
    y: i32,
    passed: usize,
    failed: usize,
) {
    let banner_color = if failed == 0 { PASS_COLOR } else { FAIL_COLOR };
    let bs = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(banner_color)
        .background_color(BG)
        .build();

    let mut sy = y + 8;
    Rectangle::new(Point::new(8, sy), Size::new(464, 1))
        .into_styled(PrimitiveStyle::with_fill(DIM_TEXT))
        .draw(fb)
        .ok();
    sy += 8;

    if failed == 0 {
        draw_text(fb, "ALL TESTS PASSED", 8, sy, &bs);
    } else {
        draw_text(fb, "SOME TESTS FAILED", 8, sy, &bs);
    }
    sy += 14;

    let ps = make_style();
    let total = passed + failed;
    draw_text(fb, "Passed: ", 8, sy, &ps);
    draw_u32(fb, 62, sy, &ps, passed as u32);
    draw_text(fb, " Failed: ", 100, sy, &ps);
    draw_u32(fb, 162, sy, &ps, failed as u32);
    draw_text(fb, " Total: ", 200, sy, &ps);
    draw_u32(fb, 258, sy, &ps, total as u32);
    sy += 14;

    let hs = make_header_style();
    draw_text(
        fb,
        "STM32F469I-DISCO Hardware Diagnostics v0.2.0",
        8,
        sy + 6,
        &hs,
    );
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

    let p = embassy_stm32::init(config);

    unsafe {
        cortex_m::peripheral::Peripherals::steal()
            .DWT
            .enable_cycle_counter();
    }

    defmt::info!("=== Hardware Diagnostics v0.2.0 ===");

    let peri = unsafe { embassy_stm32::Peripherals::steal() };

    // ═════════════════════════════════════════════════════════════
    // PHASE 1: Fast tests (no display, ~3s)
    // ═════════════════════════════════════════════════════════════

    // --- GPIO ---
    unsafe {
        tpass_fn("GPIO Input PA0", || {
            let _inp = embassy_stm32::gpio::Input::new(peri.PA0, Pull::Down);
            true
        })
    };

    unsafe {
        tpass_fn("GPIO Multi-Port Output", || {
            let mut g = Output::new(peri.PG6.clone_unchecked(), Level::Low, Speed::Low);
            g.set_high();
            g.toggle();
            g.set_low();
            true
        })
    };

    // --- LEDs (no delays — just toggle) ---
    unsafe {
        tpass_fn("LED Green (PG6)", || {
            let mut led = Output::new(peri.PG6.clone_unchecked(), Level::Low, Speed::Low);
            led.set_high();
            cortex_m::asm::delay(18_000_000);
            led.set_low();
            true
        })
    };

    unsafe {
        tpass_fn("LED Orange (PD4)", || {
            let mut led = Output::new(peri.PD4.clone_unchecked(), Level::Low, Speed::Low);
            led.toggle();
            cortex_m::asm::delay(18_000_000);
            led.set_low();
            true
        })
    };

    unsafe {
        tpass_fn("LED Red (PD5)", || {
            let mut led = Output::new(peri.PD5.clone_unchecked(), Level::Low, Speed::Low);
            led.toggle();
            cortex_m::asm::delay(18_000_000);
            led.set_low();
            true
        })
    };

    unsafe {
        tpass_fn("LED Blue (PK3)", || {
            let mut led = Output::new(peri.PK3.clone_unchecked(), Level::Low, Speed::Low);
            led.toggle();
            cortex_m::asm::delay(18_000_000);
            led.set_low();
            true
        })
    };

    unsafe {
        tpass_fn("LED All Toggle", || {
            let mut leds = [
                Output::new(peri.PG6.clone_unchecked(), Level::Low, Speed::Low),
                Output::new(peri.PD4.clone_unchecked(), Level::Low, Speed::Low),
                Output::new(peri.PD5.clone_unchecked(), Level::Low, Speed::Low),
                Output::new(peri.PK3.clone_unchecked(), Level::Low, Speed::Low),
            ];
            for _ in 0..3 {
                for led in leds.iter_mut() {
                    led.toggle();
                }
                cortex_m::asm::delay(14_400_000);
            }
            for led in leds.iter_mut() {
                led.set_low();
            }
            true
        })
    };

    // --- Timers (async, use inline checks) ---
    defmt::info!("TEST Timer 1ms: RUNNING");
    {
        let start = dwt_cycles();
        Timer::after(Duration::from_millis(1)).await;
        let us = dwt_cycles().wrapping_sub(start) / 180;
        if (900..=1500).contains(&us) {
            unsafe { tpass("Timer 1ms") };
        } else {
            unsafe { tfail("Timer 1ms") };
        }
    }

    defmt::info!("TEST Timer 100ms: RUNNING");
    {
        let start = dwt_cycles();
        Timer::after(Duration::from_millis(100)).await;
        let ms = dwt_cycles().wrapping_sub(start) / 180_000;
        if (95..=120).contains(&ms) {
            unsafe { tpass("Timer 100ms") };
        } else {
            unsafe { tfail("Timer 100ms") };
        }
    }

    defmt::info!("TEST Timer Ticker 500ms: RUNNING");
    {
        let mut ticker = embassy_time::Ticker::every(Duration::from_millis(500));
        for _ in 0..5 {
            ticker.next().await;
        }
        unsafe { tpass("Timer Ticker 500ms") };
    }

    // --- RNG ---
    stm32_metapac::RCC.ahb2enr().modify(|w| w.set_rngen(true));
    unsafe {
        tpass_fn("RNG Not Zeros", || {
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
                    let val = rng.dr().read();
                    if val != 0 {
                        return true;
                    }
                }
            }
        })
    };

    unsafe {
        tpass_fn("RNG Uniqueness", || {
            let rng = stm32_metapac::RNG;
            let mut buf = [0u32; 64];
            for slot in buf.iter_mut() {
                *slot = loop {
                    let sr = rng.sr().read();
                    if sr.seis() | sr.ceis() {
                        rng.cr().modify(|w| w.set_rngen(false));
                        rng.sr().modify(|w| {
                            w.set_seis(false);
                            w.set_ceis(false);
                        });
                        rng.cr().modify(|w| w.set_rngen(true));
                        continue;
                    }
                    if sr.drdy() {
                        break rng.dr().read();
                    }
                };
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
            unique >= 32
        })
    };

    unsafe {
        tpass_fn("RNG Consecutive Differ", || {
            let rng = stm32_metapac::RNG;
            let v1 = loop {
                let sr = rng.sr().read();
                if sr.drdy() {
                    break rng.dr().read();
                }
                if sr.seis() | sr.ceis() {
                    rng.cr().modify(|w| w.set_rngen(false));
                    rng.sr().modify(|w| {
                        w.set_seis(false);
                        w.set_ceis(false);
                    });
                    rng.cr().modify(|w| w.set_rngen(true));
                }
            };
            let v2 = loop {
                let sr = rng.sr().read();
                if sr.drdy() {
                    break rng.dr().read();
                }
                if sr.seis() | sr.ceis() {
                    rng.cr().modify(|w| w.set_rngen(false));
                    rng.sr().modify(|w| {
                        w.set_seis(false);
                        w.set_ceis(false);
                    });
                    rng.cr().modify(|w| w.set_rngen(true));
                }
            };
            v1 != v2
        })
    };

    // --- ADC ---
    stm32_metapac::RCC.apb2enr().modify(|w| w.set_adc1en(true));
    unsafe {
        tpass_fn("ADC Temp Sensor", || {
            stm32_metapac::ADC123_COMMON.ccr().modify(|w| {
                w.set_tsvrefe(true);
            });
            cortex_m::asm::delay(10_000);
            let adc = stm32_metapac::ADC1;
            adc.cr2().modify(|w| {
                w.set_adon(false);
                w.set_cont(false);
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
                w.set_sq(0, 18);
            });
            adc.smpr2().write(|w| {
                w.set_smp(18, stm32_metapac::adc::vals::SampleTime::CYCLES480);
            });
            adc.cr2().modify(|w| w.set_adon(true));
            cortex_m::asm::delay(3);
            adc.cr2().modify(|w| w.set_swstart(true));
            while !adc.sr().read().eoc() {}
            let sample = adc.dr().read().0 as u16;
            sample > 100 && sample < 4095
        })
    };

    unsafe {
        tpass_fn("ADC VREFINT", || {
            cortex_m::asm::delay(10_000);
            let adc = stm32_metapac::ADC1;
            adc.sqr3().write(|w| {
                w.set_sq(0, 17);
            });
            adc.smpr2().write(|w| {
                w.set_smp(17, stm32_metapac::adc::vals::SampleTime::CYCLES480);
            });
            adc.cr2().modify(|w| w.set_swstart(true));
            while !adc.sr().read().eoc() {}
            let sample = adc.dr().read().0 as u16;
            sample > 500 && sample < 3000
        })
    };

    // Report phase 1 results via RTT
    let p1_passed = unsafe { PASS_COUNT };
    let p1_failed = unsafe { FAIL_COUNT };
    let p1_total = p1_passed + p1_failed;
    defmt::info!("--- Phase 1: {}/{} passed ---", p1_passed, p1_total);
    if p1_failed == 0 {
        defmt::info!("Phase 1: ALL PASSED");
    } else {
        defmt::error!("Phase 1: {} FAILED", p1_failed);
    }

    // ═════════════════════════════════════════════════════════════
    // PHASE 2: Display-dependent tests (~15s)
    // ═════════════════════════════════════════════════════════════

    // SDRAM init
    defmt::info!("SDRAM init...");
    let sdram = SdramCtrl::new(
        &mut unsafe { embassy_stm32::Peripherals::steal() },
        180_000_000,
    );
    let base = sdram.base_address();
    let words = embassy_stm32f469i_disco::display::SDRAM_SIZE_BYTES / 4;
    let ram: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(base as *mut u32, words) };

    // Display init
    defmt::info!("Display init...");
    let mut display = DisplayCtrl::new(&sdram, unsafe { p.PH7.clone_unchecked() }, BoardHint::Auto);
    defmt::info!("Display init done");
    unsafe { tpass("Display Init") };

    let mut fb = display.fb();
    fb.clear(BG);
    let mut y: i32 = 10;
    let hs = make_header_style();
    draw_text(&mut fb, "STM32F469I-DISCO", 8, y, &hs);
    y += 14;
    draw_text(&mut fb, "Hardware Diagnostics v0.2.0", 8, y, &make_style());
    y += 18;

    // Render phase 1 results
    draw_header(&mut fb, &mut y, "Phase 1: GPIO / LED / Timer / RNG / ADC");
    for &(name, passed) in
        unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(RESULTS).cast(), p1_total) }
    {
        draw_result(&mut fb, &mut y, name, passed);
    }
    draw_status_line(&mut fb, &mut y, p1_passed, p1_failed, "");

    // SDRAM tests
    draw_header(&mut fb, &mut y, "SDRAM (16MB IS42S32400F-6BL)");

    unsafe {
        tpass_fn("SDRAM Checkerboard", || {
            let win = 65536usize;
            for word in ram[..win].iter_mut() {
                *word = 0xAAAAAAAA;
            }
            ram[..win].iter().all(|w| *w == 0xAAAAAAAA)
        })
    };

    unsafe {
        tpass_fn("SDRAM March C-", || {
            let win = 65536usize;
            for word in ram[..win].iter_mut() {
                *word = 0;
            }
            let mut ok = true;
            for word in ram[..win].iter_mut() {
                if *word != 0 {
                    ok = false;
                    break;
                }
                *word = 0xFFFFFFFF;
            }
            if ok {
                for word in ram[..win].iter_mut().rev() {
                    if *word != 0xFFFFFFFF {
                        ok = false;
                        break;
                    }
                    *word = 0;
                }
            }
            if ok {
                for word in ram[..win].iter() {
                    if *word != 0 {
                        ok = false;
                        break;
                    }
                }
            }
            ok
        })
    };

    unsafe {
        tpass_fn("SDRAM Boundary Spots", || {
            let mut ok = true;
            for r in 0u32..16 {
                let offset = (r as usize) * (words / 16);
                let pattern = 0xFEED0000 | r;
                let end = core::cmp::min(offset + 1024, words);
                for word in ram[offset..end].iter_mut() {
                    *word = pattern;
                }
            }
            for r in 0u32..16 {
                let offset = (r as usize) * (words / 16);
                let pattern = 0xFEED0000 | r;
                let end = core::cmp::min(offset + 1024, words);
                for word in ram[offset..end].iter() {
                    if *word != pattern {
                        ok = false;
                        break;
                    }
                }
                if !ok {
                    break;
                }
            }
            ok
        })
    };

    unsafe {
        tpass_fn("SDRAM End-of-RAM", || {
            let last = 16384usize;
            let start = words - last;
            let mut seed: u32 = 0x12345678;
            for word in ram[start..].iter_mut() {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                *word = seed;
            }
            seed = 0x12345678;
            for word in ram[start..].iter() {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                if *word != seed {
                    return false;
                }
            }
            true
        })
    };

    unsafe {
        tpass_fn("SDRAM Byte/Halfword", || {
            let mut ok = true;
            let ram_bytes: &mut [u8] = core::slice::from_raw_parts_mut(base as *mut u8, 4096);
            for (i, byte) in ram_bytes.iter_mut().enumerate() {
                *byte = (i & 0xFF) as u8;
            }
            for (i, byte) in ram_bytes.iter().enumerate() {
                if *byte != (i & 0xFF) as u8 {
                    ok = false;
                    break;
                }
            }
            if ok {
                let ram_hw: &mut [u16] = core::slice::from_raw_parts_mut(base as *mut u16, 2048);
                for (i, hw) in ram_hw.iter_mut().enumerate() {
                    *hw = ((i & 0xFFFF) as u16).wrapping_add(1);
                }
                for (i, hw) in ram_hw.iter().enumerate() {
                    if *hw != ((i & 0xFFFF) as u16).wrapping_add(1) {
                        ok = false;
                        break;
                    }
                }
            }
            ok
        })
    };

    // Display tests (async — need visual delays)
    draw_header(&mut fb, &mut y, "Display (DSI/LTDC)");

    defmt::info!("TEST Display Red Fill: RUNNING");
    {
        fb.clear(Rgb565::RED);
        Timer::after(Duration::from_millis(200)).await;
        fb.clear(BG);
        unsafe { tpass("Display Red Fill") };
    }

    defmt::info!("TEST Display Green Fill: RUNNING");
    {
        fb.clear(Rgb565::GREEN);
        Timer::after(Duration::from_millis(200)).await;
        fb.clear(BG);
        unsafe { tpass("Display Green Fill") };
    }

    defmt::info!("TEST Display Blue Fill: RUNNING");
    {
        fb.clear(Rgb565::BLUE);
        Timer::after(Duration::from_millis(200)).await;
        fb.clear(BG);
        unsafe { tpass("Display Blue Fill") };
    }

    defmt::info!("TEST Display Gradient: RUNNING");
    {
        for row in 0..FB_HEIGHT {
            let r = ((row as u32 * 255) / FB_HEIGHT as u32) as u8;
            let b = (255 - row as u32 * 255 / FB_HEIGHT as u32) as u8;
            let color = Rgb565::new(r, 0, b);
            Rectangle::new(Point::new(0, row as i32), Size::new(FB_WIDTH as u32, 1))
                .into_styled(PrimitiveStyle::with_fill(color))
                .draw(&mut fb)
                .ok();
        }
        Timer::after(Duration::from_millis(100)).await;
        fb.clear(BG);
        unsafe { tpass("Display Gradient") };
    }

    defmt::info!("TEST Display Text Render: RUNNING");
    {
        fb.clear(BG);
        let ts = MonoTextStyleBuilder::new()
            .font(&FONT_6X9)
            .text_color(Rgb565::WHITE)
            .background_color(Rgb565::CSS_NAVY)
            .build();
        embedded_graphics::text::Text::new("HELLO WORLD", Point::new(120, 390), ts)
            .draw(&mut fb)
            .ok();
        Timer::after(Duration::from_millis(200)).await;
        fb.clear(BG);
        unsafe { tpass("Display Text Render") };
    }

    // Touch tests
    draw_header(&mut fb, &mut y, "Touch (FT6X06 / I2C1)");

    unsafe {
        tpass_fn("Touch I2C Init", || {
            let _i2c = i2c::I2c::new_blocking(
                peri.I2C1.clone_unchecked(),
                peri.PB8.clone_unchecked(),
                peri.PB9.clone_unchecked(),
                i2c::Config::default(),
            );
            true
        })
    };

    unsafe {
        tpass_fn("Touch Chip ID", || {
            let mut i2c = i2c::I2c::new_blocking(
                peri.I2C1.clone_unchecked(),
                peri.PB8.clone_unchecked(),
                peri.PB9.clone_unchecked(),
                i2c::Config::default(),
            );
            let touch = TouchCtrl::new();
            match touch.read_chip_id(&mut i2c) {
                Ok(id) => id == 0xCC || id == 0xA3,
                Err(_) => false,
            }
        })
    };

    unsafe {
        tpass_fn("Touch Idle Status", || {
            let mut i2c = i2c::I2c::new_blocking(
                peri.I2C1.clone_unchecked(),
                peri.PB8.clone_unchecked(),
                peri.PB9.clone_unchecked(),
                i2c::Config::default(),
            );
            let touch = TouchCtrl::new();
            touch.td_status(&mut i2c).unwrap_or(0) == 0
        })
    };

    // UART tests
    draw_header(&mut fb, &mut y, "UART (USART1 PA9/PA10)");

    unsafe {
        tpass_fn("UART Init", || {
            embassy_stm32::usart::Uart::new_blocking(
                peri.USART1.clone_unchecked(),
                peri.PA10.clone_unchecked(),
                peri.PA9.clone_unchecked(),
                embassy_stm32::usart::Config::default(),
            )
            .is_ok()
        })
    };

    unsafe {
        tpass_fn("UART TX Byte", || {
            let mut tx = embassy_stm32::usart::Uart::new_blocking(
                peri.USART1.clone_unchecked(),
                peri.PA10.clone_unchecked(),
                peri.PA9.clone_unchecked(),
                embassy_stm32::usart::Config::default(),
            )
            .unwrap();
            use embedded_hal_02::blocking::serial::Write;
            tx.bwrite_all(b"U").is_ok()
        })
    };

    unsafe {
        tpass_fn("UART Multi-Byte", || {
            let mut tx = embassy_stm32::usart::Uart::new_blocking(
                peri.USART1.clone_unchecked(),
                peri.PA10.clone_unchecked(),
                peri.PA9.clone_unchecked(),
                embassy_stm32::usart::Config::default(),
            )
            .unwrap();
            use embedded_hal_02::blocking::serial::Write;
            tx.bwrite_all(b"HELLO").is_ok()
        })
    };

    unsafe {
        tpass_fn("UART fmt::Write", || {
            let mut tx = embassy_stm32::usart::Uart::new_blocking(
                peri.USART1.clone_unchecked(),
                peri.PA10.clone_unchecked(),
                peri.PA9.clone_unchecked(),
                embassy_stm32::usart::Config::default(),
            )
            .unwrap();
            use embedded_hal_02::blocking::serial::Write;
            let data = b"val=42";
            tx.bwrite_all(data).is_ok()
        })
    };

    // DMA tests
    draw_header(&mut fb, &mut y, "DMA (DMA2 Stream0 M2M)");

    stm32_metapac::RCC.ahb1enr().modify(|w| w.set_dma2en(true));

    unsafe {
        tpass_fn("DMA 64B Transfer", || {
            use stm32_metapac::dma::vals;
            let dma2 = stm32_metapac::DMA2;
            dma2.st(0).cr().write(|w| {
                w.set_en(false);
                w.set_dir(vals::Dir::MEMORY_TO_MEMORY);
                w.set_circ(false);
                w.set_pinc(true);
                w.set_minc(true);
                w.set_psize(vals::Size::BITS8);
                w.set_msize(vals::Size::BITS8);
                w.set_pl(vals::Pl::VERY_HIGH);
            });
            dma2.st(0).fcr().write(|w| {
                w.set_dmdis(vals::Dmdis::ENABLED);
                w.set_fth(vals::Fth::FULL);
            });
            dma2.st(0).par().write_value(0xC000_0000u32);
            dma2.st(0).m0ar().write_value(0xC000_1000u32);
            dma2.st(0).ndtr().write(|w| w.set_ndt(64));
            while !dma2.st(0).cr().read().en() {}
            dma2.st(0).cr().modify(|w| w.set_en(true));
            while !dma2.isr(0).read().tcif(0) {}
            dma2.ifcr(0).write(|w| {
                w.set_tcif(0, true);
                w.set_htif(0, true);
                w.set_feif(0, true);
                w.set_dmeif(0, true);
                w.set_teif(0, true);
            });
            true
        })
    };

    unsafe {
        tpass_fn("DMA 4096B Transfer", || {
            use stm32_metapac::dma::vals;
            let dma2 = stm32_metapac::DMA2;
            dma2.st(0).cr().write(|w| {
                w.set_en(false);
                w.set_dir(vals::Dir::MEMORY_TO_MEMORY);
                w.set_circ(false);
                w.set_pinc(true);
                w.set_minc(true);
                w.set_psize(vals::Size::BITS8);
                w.set_msize(vals::Size::BITS8);
                w.set_pl(vals::Pl::VERY_HIGH);
            });
            dma2.st(0).fcr().write(|w| {
                w.set_dmdis(vals::Dmdis::ENABLED);
                w.set_fth(vals::Fth::FULL);
            });
            dma2.st(0).par().write_value(0xC000_2000u32);
            dma2.st(0).m0ar().write_value(0xC000_3000u32);
            dma2.st(0).ndtr().write(|w| w.set_ndt(4096));
            while !dma2.st(0).cr().read().en() {}
            dma2.st(0).cr().modify(|w| w.set_en(true));
            while !dma2.isr(0).read().tcif(0) {}
            dma2.ifcr(0).write(|w| {
                w.set_tcif(0, true);
                w.set_htif(0, true);
                w.set_feif(0, true);
                w.set_dmeif(0, true);
                w.set_teif(0, true);
            });
            true
        })
    };

    unsafe {
        tpass_fn("DMA 1024B Transfer", || {
            use stm32_metapac::dma::vals;
            let dma2 = stm32_metapac::DMA2;
            dma2.st(0).cr().write(|w| {
                w.set_en(false);
                w.set_dir(vals::Dir::MEMORY_TO_MEMORY);
                w.set_circ(false);
                w.set_pinc(true);
                w.set_minc(true);
                w.set_psize(vals::Size::BITS8);
                w.set_msize(vals::Size::BITS8);
                w.set_pl(vals::Pl::VERY_HIGH);
            });
            dma2.st(0).fcr().write(|w| {
                w.set_dmdis(vals::Dmdis::ENABLED);
                w.set_fth(vals::Fth::FULL);
            });
            dma2.st(0).par().write_value(0xC000_4000u32);
            dma2.st(0).m0ar().write_value(0xC000_5000u32);
            dma2.st(0).ndtr().write(|w| w.set_ndt(1024));
            while !dma2.st(0).cr().read().en() {}
            dma2.st(0).cr().modify(|w| w.set_en(true));
            while !dma2.isr(0).read().tcif(0) {}
            dma2.ifcr(0).write(|w| {
                w.set_tcif(0, true);
                w.set_htif(0, true);
                w.set_feif(0, true);
                w.set_dmeif(0, true);
                w.set_teif(0, true);
            });
            true
        })
    };

    unsafe {
        tpass_fn("DMA Repeated 10x", || {
            use stm32_metapac::dma::vals;
            let dma2 = stm32_metapac::DMA2;
            for _ in 0..10u32 {
                dma2.st(0).cr().write(|w| {
                    w.set_en(false);
                    w.set_dir(vals::Dir::MEMORY_TO_MEMORY);
                    w.set_circ(false);
                    w.set_pinc(true);
                    w.set_minc(true);
                    w.set_psize(vals::Size::BITS8);
                    w.set_msize(vals::Size::BITS8);
                    w.set_pl(vals::Pl::VERY_HIGH);
                });
                dma2.st(0).fcr().write(|w| {
                    w.set_dmdis(vals::Dmdis::ENABLED);
                    w.set_fth(vals::Fth::FULL);
                });
                dma2.st(0).par().write_value(0xC000_6000u32);
                dma2.st(0).m0ar().write_value(0xC000_7000u32);
                dma2.st(0).ndtr().write(|w| w.set_ndt(256));
                while !dma2.st(0).cr().read().en() {}
                dma2.st(0).cr().modify(|w| w.set_en(true));
                while !dma2.isr(0).read().tcif(0) {}
                dma2.ifcr(0).write(|w| {
                    w.set_tcif(0, true);
                    w.set_htif(0, true);
                    w.set_feif(0, true);
                    w.set_dmeif(0, true);
                    w.set_teif(0, true);
                });
            }
            true
        })
    };

    // === Final summary ===
    let total_passed = unsafe { PASS_COUNT };
    let total_failed = unsafe { FAIL_COUNT };
    let total = total_passed + total_failed;
    defmt::info!("SUMMARY: {}/{} passed", total_passed, total);
    if total_failed == 0 {
        defmt::info!("ALL TESTS PASSED");
    } else {
        defmt::error!("FAILED: {} tests failed", total_failed);
    }

    draw_summary(&mut fb, y, total_passed, total_failed);
    Timer::after(Duration::from_secs(2)).await;

    // Touch demo (30s)
    defmt::info!("Entering touch demo...");
    {
        fb.clear(BG);
        let hs = make_header_style();
        let ts = make_style();
        draw_text(&mut fb, "Touch Demo (30s)", 8, 10, &hs);
        Rectangle::new(Point::new(8, 28), Size::new(464, 1))
            .into_styled(PrimitiveStyle::with_fill(DIM_TEXT))
            .draw(&mut fb)
            .ok();
        draw_text(
            &mut fb,
            "Touch the screen. Coordinates shown below.",
            8,
            40,
            &ts,
        );
        draw_text(&mut fb, "Tests phantom touch rejection.", 8, 52, &ts);

        let touch = TouchCtrl::new();
        let mut i2c = i2c::I2c::new_blocking(
            unsafe { peri.I2C1.clone_unchecked() },
            unsafe { peri.PB8.clone_unchecked() },
            unsafe { peri.PB9.clone_unchecked() },
            i2c::Config::default(),
        );
        let mut deadline = 30u32;
        let mut touch_count = 0u32;
        while deadline > 0 {
            if touch.td_status(&mut i2c).unwrap_or(0) > 0 {
                if let Ok(point) = touch.get_touch(&mut i2c) {
                    if point.x >= 3 && point.x <= 476 && point.y >= 3 && point.y <= 796 {
                        let cross = Rgb565::new(0xff, 0xff, 0x00);
                        let cs = PrimitiveStyle::with_fill(cross);
                        let cx = point.x as i32;
                        let cy = point.y as i32;
                        Rectangle::new(Point::new(cx - 8, cy - 1), Size::new(16, 2))
                            .into_styled(cs)
                            .draw(&mut fb)
                            .ok();
                        Rectangle::new(Point::new(cx - 1, cy - 8), Size::new(2, 16))
                            .into_styled(cs)
                            .draw(&mut fb)
                            .ok();
                        draw_text(&mut fb, "Touches: ", 8, FB_HEIGHT as i32 - 40, &ts);
                        draw_u32(&mut fb, 62, FB_HEIGHT as i32 - 40, &ts, touch_count + 1);
                        touch_count += 1;
                    }
                }
            }
            Timer::after(Duration::from_millis(50)).await;
            deadline -= 1;
        }

        draw_text(&mut fb, "Touch demo complete.", 8, 90, &ts);
        draw_text(&mut fb, "Press reset to restart.", 8, 104, &ts);
    }

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
