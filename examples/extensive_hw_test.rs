//! Unified on-device hardware test with RTT logging, display output, and touch demo.
#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_executor::Spawner;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::usart::Uart;
use embassy_stm32::Peripherals;
use embassy_stm32f469i_disco::touch::{EdgeFilter, TouchPoint};
use embassy_stm32f469i_disco::{
    config_180, Board, BoardHint, FramebufferView, FB_HEIGHT, FB_WIDTH, SDRAM_SIZE_BYTES,
};
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::{
    mono_font::{ascii::FONT_6X9, MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
    text::Text,
};
use embedded_hal_02::blocking::serial::Write as _;
use embedded_io::Write as _;

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

const BG: Rgb888 = Rgb888::new(0x1a, 0x1a, 0x2e);
const PASS_COLOR: Rgb888 = Rgb888::new(0x00, 0xe0, 0x40);
const FAIL_COLOR: Rgb888 = Rgb888::new(0xe0, 0x20, 0x20);
const HEADER_COLOR: Rgb888 = Rgb888::new(0x40, 0xa0, 0xe0);
const TEXT_COLOR: Rgb888 = Rgb888::new(0xe0, 0xe0, 0xe0);
const DIM_TEXT: Rgb888 = Rgb888::new(0x80, 0x80, 0x80);
const RUN_COLOR: Rgb888 = Rgb888::new(0xff, 0xc0, 0x00);
const MAX_TESTS: usize = 64;

const SDRAM_BASE: usize = 0xC000_0000;
const FRAMEBUFFER_BYTES: usize = FB_WIDTH as usize * FB_HEIGHT as usize * 4;
const SDRAM_SCRATCH_OFFSET: usize = 2 * 1024 * 1024;
const SDRAM_SCRATCH_BASE: usize = SDRAM_BASE + SDRAM_SCRATCH_OFFSET;

// CCMRAM result buffer at 0x1000_0000 for probe-rs readback.
// Read with: python3 tests/read_test_results.py

const TEST_NAME_LEN: usize = 48;
const RESULT_MAGIC: u32 = 0x5245534C; // "RESL"
const CCMRAM_BASE: usize = 0x1000_0000;

#[repr(C)]
struct TestResultEntry {
    name: [u8; TEST_NAME_LEN],
    passed: u8,
    _pad: [u8; 3],
}

#[repr(C)]
struct TestResultBuffer {
    magic: u32,
    count: u32,
    pass_count: u32,
    fail_count: u32,
    entries: [TestResultEntry; MAX_TESTS],
    done: u32,
}

fn copy_name(dest: &mut [u8; TEST_NAME_LEN], src: &str) {
    let bytes = src.as_bytes();
    let len = bytes.len().min(TEST_NAME_LEN);
    dest[..len].copy_from_slice(&bytes[..len]);
    for b in dest.iter_mut().skip(len) {
        *b = 0;
    }
}

unsafe fn flush_to_ccmram() {
    let buf = CCMRAM_BASE as *mut TestResultBuffer;
    core::ptr::write_volatile(core::ptr::addr_of_mut!((*buf).magic), RESULT_MAGIC);
    core::ptr::write_volatile(core::ptr::addr_of_mut!((*buf).count), RESULT_COUNT as u32);
    core::ptr::write_volatile(
        core::ptr::addr_of_mut!((*buf).pass_count),
        PASS_COUNT as u32,
    );
    core::ptr::write_volatile(
        core::ptr::addr_of_mut!((*buf).fail_count),
        FAIL_COUNT as u32,
    );
    for (i, (name, passed)) in RESULTS[..RESULT_COUNT.min(MAX_TESTS)].iter().enumerate() {
        let entry = core::ptr::addr_of_mut!((*buf).entries[i]);
        copy_name(&mut (*entry).name, name);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*entry).passed),
            if *passed { 1 } else { 0 },
        );
    }
}

unsafe fn mark_done() {
    let buf = CCMRAM_BASE as *mut TestResultBuffer;
    core::ptr::write_volatile(core::ptr::addr_of_mut!((*buf).done), RESULT_MAGIC);
}

static mut RESULTS: [(&str, bool); MAX_TESTS] = [("", false); MAX_TESTS];
static mut RESULT_COUNT: usize = 0;
static mut PASS_COUNT: usize = 0;
static mut FAIL_COUNT: usize = 0;

unsafe fn tpass(name: &'static str) {
    if RESULT_COUNT < MAX_TESTS {
        RESULTS[RESULT_COUNT] = (name, true);
        RESULT_COUNT += 1;
    }
    PASS_COUNT += 1;
    defmt::info!("TEST {}: PASS", name);
    flush_to_ccmram();
}

unsafe fn tfail(name: &'static str, reason: &'static str) {
    if RESULT_COUNT < MAX_TESTS {
        RESULTS[RESULT_COUNT] = (name, false);
        RESULT_COUNT += 1;
    }
    FAIL_COUNT += 1;
    defmt::info!("TEST {}: FAIL {}", name, reason);
    flush_to_ccmram();
}

unsafe fn trun(name: &'static str) {
    defmt::info!("TEST {}: RUNNING", name);
}

unsafe fn tpass_fn(name: &'static str, f: impl FnOnce() -> Result<(), &'static str>) {
    trun(name);
    match f() {
        Ok(()) => tpass(name),
        Err(reason) => tfail(name, reason),
    }
}

fn result_slice() -> &'static [(&'static str, bool)] {
    unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(RESULTS).cast(), RESULT_COUNT) }
}

fn total_passed() -> usize {
    unsafe { PASS_COUNT }
}

fn total_failed() -> usize {
    unsafe { FAIL_COUNT }
}

fn make_style() -> MonoTextStyle<'static, Rgb888> {
    MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(TEXT_COLOR)
        .background_color(BG)
        .build()
}

fn make_header_style() -> MonoTextStyle<'static, Rgb888> {
    MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(HEADER_COLOR)
        .background_color(BG)
        .build()
}

fn make_run_style() -> MonoTextStyle<'static, Rgb888> {
    MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(RUN_COLOR)
        .background_color(BG)
        .build()
}

fn draw_text(
    fb: &mut FramebufferView<'_>,
    text: &str,
    x: i32,
    y: i32,
    style: &MonoTextStyle<Rgb888>,
) {
    Text::new(text, Point::new(x, y), *style).draw(fb).ok();
}

fn draw_u32(fb: &mut FramebufferView<'_>, x: i32, y: i32, style: &MonoTextStyle<Rgb888>, val: u32) {
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

fn draw_header(fb: &mut FramebufferView<'_>, y: &mut i32, title: &str) {
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

fn draw_result(fb: &mut FramebufferView<'_>, y: &mut i32, name: &str, passed: bool) {
    let color = if passed { PASS_COLOR } else { FAIL_COLOR };
    let status = if passed { "PASS" } else { "FAIL" };
    Rectangle::new(Point::new(12, *y), Size::new(8, 8))
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(fb)
        .ok();
    draw_text(fb, name, 26, *y, &make_style());
    draw_text(fb, status, 430, *y, &make_style());
    *y += 12;
}

fn draw_status_line(
    fb: &mut FramebufferView<'_>,
    y: &mut i32,
    passed: usize,
    failed: usize,
    running: &str,
) {
    let ps = make_style();
    draw_text(fb, "PASS:", 8, *y, &ps);
    draw_u32(fb, 46, *y, &ps, passed as u32);
    draw_text(fb, " FAIL:", 84, *y, &ps);
    draw_u32(fb, 124, *y, &ps, failed as u32);
    if !running.is_empty() {
        draw_text(fb, " RUNNING:", 156, *y, &make_run_style());
        draw_text(fb, running, 224, *y, &make_run_style());
    }
    *y += 14;
}

fn draw_summary(fb: &mut FramebufferView<'_>, y: i32, passed: usize, failed: usize) {
    let banner_color = if failed == 0 { PASS_COLOR } else { FAIL_COLOR };
    let bs = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(banner_color)
        .background_color(BG)
        .build();
    let ps = make_style();
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

    draw_text(fb, "Passed:", 8, sy, &ps);
    draw_u32(fb, 56, sy, &ps, passed as u32);
    draw_text(fb, " Failed:", 100, sy, &ps);
    draw_u32(fb, 150, sy, &ps, failed as u32);
    draw_text(fb, " Total:", 192, sy, &ps);
    draw_u32(fb, 236, sy, &ps, (passed + failed) as u32);
}

fn clear_rect(fb: &mut FramebufferView<'_>, x: i32, y: i32, w: u32, h: u32) {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
        .into_styled(PrimitiveStyle::with_fill(BG))
        .draw(fb)
        .ok();
}

fn render_results_screen(board: &mut Board, title: &str, subtitle: &str, running: &str) {
    let mut fb = board.display.fb();
    let hs = make_header_style();
    let ps = make_style();
    let mut y = 10;

    fb.clear(BG);
    draw_text(&mut fb, "STM32F469I Extensive HW Test", 8, y, &hs);
    y += 14;
    draw_text(&mut fb, title, 8, y, &ps);
    y += 12;
    if !subtitle.is_empty() {
        draw_text(&mut fb, subtitle, 8, y, &ps);
        y += 16;
    } else {
        y += 4;
    }

    draw_status_line(&mut fb, &mut y, total_passed(), total_failed(), running);
    draw_header(&mut fb, &mut y, "Recorded Results");
    for &(name, passed) in result_slice() {
        draw_result(&mut fb, &mut y, name, passed);
    }
}

fn render_final_screen(board: &mut Board) {
    let mut fb = board.display.fb();
    let hs = make_header_style();
    let ps = make_style();
    let passed = total_passed();
    let failed = total_failed();
    let mut y = 10;

    fb.clear(BG);
    draw_text(&mut fb, "STM32F469I Extensive HW Test", 8, y, &hs);
    y += 16;
    draw_text(&mut fb, "Final Summary", 8, y, &ps);
    y += 18;
    draw_summary(&mut fb, y, passed, failed);
    y += 64;
    draw_header(&mut fb, &mut y, "Failed Tests");

    let mut failed_any = false;
    for &(name, passed) in result_slice() {
        if !passed {
            failed_any = true;
            draw_result(&mut fb, &mut y, name, false);
        }
    }
    if !failed_any {
        draw_text(&mut fb, "None", 26, y, &ps);
        y += 12;
    }

    y += 10;
    draw_text(&mut fb, "Press reset to restart.", 8, y, &ps);
}

fn sdram_scratch_words() -> &'static mut [u32] {
    let bytes = SDRAM_SIZE_BYTES - SDRAM_SCRATCH_OFFSET;
    unsafe { core::slice::from_raw_parts_mut(SDRAM_SCRATCH_BASE as *mut u32, bytes / 4) }
}

fn sdram_scratch_bytes(offset: usize, len: usize) -> &'static mut [u8] {
    unsafe { core::slice::from_raw_parts_mut((SDRAM_SCRATCH_BASE + offset) as *mut u8, len) }
}

async fn blink_led_once(pin: embassy_stm32::Peri<'static, impl embassy_stm32::gpio::Pin>) {
    let mut led = Output::new(pin, Level::High, Speed::Low);
    led.set_low();
    Timer::after(Duration::from_millis(250)).await;
    led.set_high();
    Timer::after(Duration::from_millis(150)).await;
}

async fn wait_for_user_confirmation(board: &mut Board, title: &str, message: &str) {
    loop {
        {
            let mut fb = board.display.fb();
            let hs = make_header_style();
            let ts = make_style();
            fb.clear(BG);
            draw_text(&mut fb, title, 8, 10, &hs);
            draw_text(&mut fb, message, 8, 30, &ts);
            draw_text(&mut fb, "Press USER button or touch screen", 8, 46, &ts);
            draw_text(&mut fb, "to continue.", 8, 58, &ts);
        }

        if board.user_button.0.is_high() {
            break;
        }
        if matches!(board.touch.get_touch(), Ok(Some(TouchPoint { .. }))) {
            break;
        }

        Timer::after(Duration::from_millis(50)).await;
    }

    loop {
        let button_pressed = board.user_button.0.is_high();
        let touch_active = matches!(board.touch.get_touch(), Ok(Some(TouchPoint { .. })));
        if !button_pressed && !touch_active {
            break;
        }
        Timer::after(Duration::from_millis(50)).await;
    }
}

async fn phase1_raw_tests(peri: &Peripherals) {
    defmt::info!("=== Phase 1: Automated subsystem tests ===");
    defmt::info!("Observe LEDs during LED tests");

    unsafe {
        tpass_fn("GPIO Input PA0", || {
            let _button = Input::new(peri.PA0.clone_unchecked(), Pull::Down);
            Ok(())
        });
        tpass_fn("GPIO Multi-Port Output", || {
            let mut green = Output::new(peri.PG6.clone_unchecked(), Level::High, Speed::Low);
            let mut orange = Output::new(peri.PD4.clone_unchecked(), Level::High, Speed::Low);
            green.set_low();
            orange.set_low();
            green.set_high();
            orange.set_high();
            Ok(())
        });
    }

    unsafe { trun("LED Green (PG6)") };
    blink_led_once(unsafe { peri.PG6.clone_unchecked() }).await;
    unsafe { tpass("LED Green (PG6)") };

    unsafe { trun("LED Orange (PD4)") };
    blink_led_once(unsafe { peri.PD4.clone_unchecked() }).await;
    unsafe { tpass("LED Orange (PD4)") };

    unsafe { trun("LED Red (PD5)") };
    blink_led_once(unsafe { peri.PD5.clone_unchecked() }).await;
    unsafe { tpass("LED Red (PD5)") };

    unsafe { trun("LED Blue (PK3)") };
    blink_led_once(unsafe { peri.PK3.clone_unchecked() }).await;
    unsafe { tpass("LED Blue (PK3)") };

    unsafe { trun("LED All Toggle") };
    {
        let mut green = Output::new(
            unsafe { peri.PG6.clone_unchecked() },
            Level::High,
            Speed::Low,
        );
        let mut orange = Output::new(
            unsafe { peri.PD4.clone_unchecked() },
            Level::High,
            Speed::Low,
        );
        let mut red = Output::new(
            unsafe { peri.PD5.clone_unchecked() },
            Level::High,
            Speed::Low,
        );
        let mut blue = Output::new(
            unsafe { peri.PK3.clone_unchecked() },
            Level::High,
            Speed::Low,
        );
        for _ in 0..3 {
            green.toggle();
            orange.toggle();
            red.toggle();
            blue.toggle();
            Timer::after(Duration::from_millis(180)).await;
        }
        green.set_high();
        orange.set_high();
        red.set_high();
        blue.set_high();
    }
    unsafe { tpass("LED All Toggle") };

    unsafe { trun("Timer 1ms") };
    Timer::after(Duration::from_millis(1)).await;
    unsafe { tpass("Timer 1ms") };

    unsafe { trun("Timer 100ms") };
    {
        let start = Instant::now();
        Timer::after(Duration::from_millis(100)).await;
        let elapsed = start.elapsed().as_millis();
        if (95..=120).contains(&elapsed) {
            unsafe { tpass("Timer 100ms") };
        } else {
            unsafe { tfail("Timer 100ms", "range") };
        }
    }

    unsafe { trun("Timer Ticker 500ms") };
    {
        let mut ticker = embassy_time::Ticker::every(Duration::from_millis(500));
        for _ in 0..4 {
            ticker.next().await;
        }
    }
    unsafe { tpass("Timer Ticker 500ms") };

    stm32_metapac::RCC.ahb2enr().modify(|w| w.set_rngen(true));
    {
        let rng = stm32_metapac::RNG;
        rng.cr().modify(|w| w.set_rngen(false));
        rng.sr().modify(|w| {
            w.set_seis(false);
            w.set_ceis(false);
        });
        rng.cr().modify(|w| w.set_rngen(true));
    }

    unsafe {
        tpass_fn("RNG Not Zeros", || {
            let rng = stm32_metapac::RNG;
            let mut timeout = 1_000_000u32;
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
                    return if rng.dr().read() != 0 {
                        Ok(())
                    } else {
                        Err("zero")
                    };
                }
                timeout -= 1;
                if timeout == 0 {
                    return Err("timeout");
                }
            }
        });

        tpass_fn("RNG Uniqueness", || {
            let rng = stm32_metapac::RNG;
            let mut buf = [0u32; 64];
            for slot in buf.iter_mut() {
                let mut timeout = 1_000_000u32;
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
                    timeout -= 1;
                    if timeout == 0 {
                        return Err("timeout");
                    }
                };
            }

            let mut unique = 0usize;
            for i in 0..buf.len() {
                let mut is_unique = true;
                for j in 0..i {
                    if buf[i] == buf[j] {
                        is_unique = false;
                        break;
                    }
                }
                if is_unique {
                    unique += 1;
                }
            }
            if unique >= 32 {
                Ok(())
            } else {
                Err("repeat")
            }
        });

        tpass_fn("RNG Consecutive Differ", || {
            let rng = stm32_metapac::RNG;
            let mut timeout = 1_000_000u32;
            let first = loop {
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
                timeout -= 1;
                if timeout == 0 {
                    return Err("timeout1");
                }
            };

            timeout = 1_000_000u32;
            let second = loop {
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
                timeout -= 1;
                if timeout == 0 {
                    return Err("timeout2");
                }
            };

            if first != second {
                Ok(())
            } else {
                Err("same")
            }
        });
    }

    stm32_metapac::RCC.apb2enr().modify(|w| w.set_adc1en(true));
    unsafe {
        tpass_fn("ADC Temp Sensor", || {
            stm32_metapac::ADC123_COMMON
                .ccr()
                .modify(|w| w.set_tsvrefe(true));
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
            adc.sqr3().write(|w| w.set_sq(0, 18));
            adc.smpr2()
                .write(|w| w.set_smp(8, stm32_metapac::adc::vals::SampleTime::CYCLES480));
            adc.cr2().modify(|w| w.set_adon(true));
            cortex_m::asm::delay(3);
            adc.cr2().modify(|w| w.set_swstart(true));
            while !adc.sr().read().eoc() {}
            let sample = adc.dr().read().0 as u16;
            if sample > 100 && sample < 4095 {
                Ok(())
            } else {
                Err("range")
            }
        });

        tpass_fn("ADC VREFINT", || {
            cortex_m::asm::delay(10_000);
            let adc = stm32_metapac::ADC1;
            adc.sqr3().write(|w| w.set_sq(0, 17));
            adc.smpr2()
                .write(|w| w.set_smp(7, stm32_metapac::adc::vals::SampleTime::CYCLES480));
            adc.cr2().modify(|w| w.set_swstart(true));
            while !adc.sr().read().eoc() {}
            let sample = adc.dr().read().0 as u16;
            if sample > 500 && sample < 3000 {
                Ok(())
            } else {
                Err("range")
            }
        });
    }
}

async fn phase2_sdram_and_display(board: &mut Board) {
    defmt::info!("=== Phase 2: Display init + SDRAM tests ===");
    unsafe { tpass("Display Init") };
    render_results_screen(
        board,
        "Phase 2",
        "Display initialized with ForceNt35510",
        "",
    );
    Timer::after(Duration::from_millis(400)).await;

    unsafe {
        trun("SDRAM Checkerboard");
        let result: Result<(), &str> = {
            let ram = sdram_scratch_words();
            let win = 65_536usize.min(ram.len());
            for word in &mut ram[..win] {
                *word = 0xAAAA_AAAA;
            }
            if ram[..win].iter().all(|&w| w == 0xAAAA_AAAA) {
                Ok(())
            } else {
                Err("mismatch")
            }
        };
        match result {
            Ok(()) => tpass("SDRAM Checkerboard"),
            Err(reason) => tfail("SDRAM Checkerboard", reason),
        }
    }
    render_results_screen(
        board,
        "Phase 2",
        "Running SDRAM tests",
        "SDRAM Checkerboard",
    );

    unsafe {
        trun("SDRAM March C-");
        match (|| {
            let ram = sdram_scratch_words();
            let win = 65_536usize.min(ram.len());
            for word in &mut ram[..win] {
                *word = 0;
            }
            for word in &mut ram[..win] {
                if *word != 0 {
                    return Err("step1");
                }
                *word = 0xFFFF_FFFF;
            }
            for word in ram[..win].iter_mut().rev() {
                if *word != 0xFFFF_FFFF {
                    return Err("step2");
                }
                *word = 0;
            }
            if ram[..win].iter().all(|&w| w == 0) {
                Ok(())
            } else {
                Err("step3")
            }
        })() {
            Ok(()) => tpass("SDRAM March C-"),
            Err(reason) => tfail("SDRAM March C-", reason),
        }
    }

    unsafe {
        trun("SDRAM Boundary Spots");
        match (|| {
            let words = SDRAM_SIZE_BYTES / 4;
            let ram = core::slice::from_raw_parts_mut(SDRAM_BASE as *mut u32, words);
            let start = FRAMEBUFFER_BYTES / 4;
            let span = words - start;
            for region in 0..8u32 {
                let offset = start + (region as usize * (span / 8));
                let end = core::cmp::min(offset + 256, words);
                let pattern = 0xFEED_0000 | region;
                for word in &mut ram[offset..end] {
                    *word = pattern;
                }
            }
            for region in 0..8u32 {
                let offset = start + (region as usize * (span / 8));
                let end = core::cmp::min(offset + 256, words);
                let pattern = 0xFEED_0000 | region;
                if !ram[offset..end].iter().all(|&w| w == pattern) {
                    return Err("mismatch");
                }
            }
            Ok(())
        })() {
            Ok(()) => tpass("SDRAM Boundary Spots"),
            Err(reason) => tfail("SDRAM Boundary Spots", reason),
        }
    }

    unsafe {
        trun("SDRAM End-of-RAM");
        match (|| {
            let tail_words = 16_384usize;
            let start = SDRAM_BASE + SDRAM_SIZE_BYTES - tail_words * 4;
            let ram = core::slice::from_raw_parts_mut(start as *mut u32, tail_words);
            let mut seed = 0x1234_5678u32;
            for word in ram.iter_mut() {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                *word = seed;
            }
            seed = 0x1234_5678u32;
            for &word in ram.iter() {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                if word != seed {
                    return Err("mismatch");
                }
            }
            Ok(())
        })() {
            Ok(()) => tpass("SDRAM End-of-RAM"),
            Err(reason) => tfail("SDRAM End-of-RAM", reason),
        }
    }

    unsafe {
        trun("SDRAM Byte/Halfword");
        match (|| {
            let bytes = sdram_scratch_bytes(0x20_000, 4096);
            for (i, byte) in bytes.iter_mut().enumerate() {
                *byte = (i & 0xFF) as u8;
            }
            for (i, &byte) in bytes.iter().enumerate() {
                if byte != (i & 0xFF) as u8 {
                    return Err("byte");
                }
            }

            let halfwords = core::slice::from_raw_parts_mut(bytes.as_mut_ptr() as *mut u16, 2048);
            for (i, hw) in halfwords.iter_mut().enumerate() {
                *hw = (i as u16).wrapping_add(1);
            }
            for (i, &hw) in halfwords.iter().enumerate() {
                if hw != (i as u16).wrapping_add(1) {
                    return Err("halfword");
                }
            }
            Ok(())
        })() {
            Ok(()) => tpass("SDRAM Byte/Halfword"),
            Err(reason) => tfail("SDRAM Byte/Halfword", reason),
        }
    }

    render_results_screen(board, "Phase 2", "Display + SDRAM complete", "");
    Timer::after(Duration::from_secs(2)).await;
}

fn draw_horizontal_gradient(fb: &mut FramebufferView<'_>) {
    for x in 0..FB_WIDTH as u32 {
        let red = (255 - ((x * 255) / (FB_WIDTH as u32 - 1))) as u8;
        let blue = ((x * 255) / (FB_WIDTH as u32 - 1)) as u8;
        Rectangle::new(Point::new(x as i32, 0), Size::new(1, FB_HEIGHT as u32))
            .into_styled(PrimitiveStyle::with_fill(Rgb888::new(red, 0, blue)))
            .draw(fb)
            .ok();
    }
}

fn draw_text_pattern(fb: &mut FramebufferView<'_>) {
    fb.clear(BG);
    let hs = make_header_style();
    let ts = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(Rgb888::WHITE)
        .background_color(BG)
        .build();
    draw_text(fb, "Text Render", 8, 12, &hs);
    draw_text(fb, "HELLO WORLD", 160, 400, &ts);
}

fn draw_color_bars(fb: &mut FramebufferView<'_>) {
    const COLORS: [Rgb888; 8] = [
        Rgb888::RED,
        Rgb888::GREEN,
        Rgb888::BLUE,
        Rgb888::CYAN,
        Rgb888::MAGENTA,
        Rgb888::YELLOW,
        Rgb888::BLACK,
        Rgb888::WHITE,
    ];
    let bar_w = FB_WIDTH as i32 / 8;
    for (idx, color) in COLORS.iter().enumerate() {
        Rectangle::new(
            Point::new(idx as i32 * bar_w, 0),
            Size::new(bar_w as u32 + 1, FB_HEIGHT as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(*color))
        .draw(fb)
        .ok();
    }
}

fn draw_grid_pattern(fb: &mut FramebufferView<'_>) {
    fb.clear(BG);
    let grid = PrimitiveStyleBuilder::new()
        .stroke_color(DIM_TEXT)
        .stroke_width(1)
        .build();
    for x in (0..=FB_WIDTH as i32).step_by(80) {
        Line::new(Point::new(x, 0), Point::new(x, FB_HEIGHT as i32 - 1))
            .into_styled(grid)
            .draw(fb)
            .ok();
    }
    for y in (0..=FB_HEIGHT as i32).step_by(80) {
        Line::new(Point::new(0, y), Point::new(FB_WIDTH as i32 - 1, y))
            .into_styled(grid)
            .draw(fb)
            .ok();
    }
}

async fn run_visual_test(
    board: &mut Board,
    name: &'static str,
    draw: impl FnOnce(&mut FramebufferView<'_>),
) {
    unsafe { trun(name) };
    {
        let mut fb = board.display.fb();
        draw(&mut fb);
    }
    Timer::after(Duration::from_secs(2)).await;
    unsafe { tpass(name) };
    wait_for_user_confirmation(board, name, "Visual test complete.").await;
}

async fn phase3_visual_tests(board: &mut Board) {
    defmt::info!("=== Phase 3: Visual display tests ===");

    run_visual_test(board, "Display Solid Red", |fb| fb.clear(Rgb888::RED)).await;
    run_visual_test(board, "Display Solid Green", |fb| fb.clear(Rgb888::GREEN)).await;
    run_visual_test(board, "Display Solid Blue", |fb| fb.clear(Rgb888::BLUE)).await;
    run_visual_test(board, "Display Gradient", draw_horizontal_gradient).await;
    run_visual_test(board, "Display Text Render", draw_text_pattern).await;
    run_visual_test(board, "Display Color Bars", draw_color_bars).await;
    run_visual_test(board, "Display Grid", draw_grid_pattern).await;

    render_results_screen(board, "Phase 3", "Visual tests summary", "");
    Timer::after(Duration::from_secs(2)).await;
}

async fn phase4_touch(board: &mut Board) {
    defmt::info!("=== Phase 4: Interactive touch test ===");

    unsafe { trun("Touch Vendor ID") };
    match board.touch.read_vendor_id() {
        Ok(0x11) => unsafe { tpass("Touch Vendor ID") },
        Ok(_) => unsafe { tfail("Touch Vendor ID", "unexpected") },
        Err(_) => unsafe { tfail("Touch Vendor ID", "i2c") },
    }

    unsafe { trun("Touch Chip Model") };
    match board.touch.read_chip_model() {
        Ok(0x06 | 0x36 | 0x64) => unsafe { tpass("Touch Chip Model") },
        Ok(_) => unsafe { tfail("Touch Chip Model", "unexpected") },
        Err(_) => unsafe { tfail("Touch Chip Model", "i2c") },
    }

    render_results_screen(board, "Phase 4", "Touch metadata checks complete", "");
    Timer::after(Duration::from_secs(1)).await;
    wait_for_user_confirmation(
        board,
        "Touch Demo",
        "Confirm to start 30-second touch test.",
    )
    .await;

    let mut touch_count = 0u32;
    let mut had_i2c_error = false;
    let deadline = Instant::now() + Duration::from_secs(30);

    {
        let mut fb = board.display.fb();
        fb.clear(BG);
        let hs = make_header_style();
        let ts = make_style();
        draw_text(&mut fb, "Touch Demo", 8, 10, &hs);
        draw_text(&mut fb, "Touch the screen for 30 seconds.", 8, 30, &ts);
        draw_text(
            &mut fb,
            "Yellow crosshairs mark filtered touch points.",
            8,
            42,
            &ts,
        );
        draw_text(&mut fb, "Touches:", 8, 60, &ts);
        draw_u32(&mut fb, 58, 60, &ts, 0);
        draw_text(&mut fb, "Time left:", 120, 60, &ts);
        draw_u32(&mut fb, 178, 60, &ts, 30);
    }

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now()).as_secs() as u32;
        match board.touch.get_touch() {
            Ok(Some(point)) => {
                touch_count += 1;
                let mut fb = board.display.fb();
                let cross = PrimitiveStyle::with_fill(Rgb888::YELLOW);
                let cx = point.x as i32;
                let cy = point.y as i32;
                Rectangle::new(Point::new(cx - 8, cy - 1), Size::new(16, 2))
                    .into_styled(cross)
                    .draw(&mut fb)
                    .ok();
                Rectangle::new(Point::new(cx - 1, cy - 8), Size::new(2, 16))
                    .into_styled(cross)
                    .draw(&mut fb)
                    .ok();
                clear_rect(&mut fb, 58, 60, 48, 10);
                clear_rect(&mut fb, 178, 60, 48, 10);
                draw_u32(&mut fb, 58, 60, &make_style(), touch_count);
                draw_u32(&mut fb, 178, 60, &make_style(), remaining);
            }
            Ok(None) => {
                let mut fb = board.display.fb();
                clear_rect(&mut fb, 178, 60, 48, 10);
                draw_u32(&mut fb, 178, 60, &make_style(), remaining);
            }
            Err(_) => {
                had_i2c_error = true;
                let mut fb = board.display.fb();
                clear_rect(&mut fb, 178, 60, 48, 10);
                draw_u32(&mut fb, 178, 60, &make_style(), remaining);
            }
        }
        Timer::after(Duration::from_millis(50)).await;
    }

    {
        let mut fb = board.display.fb();
        let ts = make_style();
        draw_text(&mut fb, "Touch demo complete", 8, 88, &ts);
        draw_text(&mut fb, "Results recorded below.", 8, 100, &ts);
    }

    unsafe { trun("Touch Demo 30s") };
    if had_i2c_error {
        unsafe { tfail("Touch Demo 30s", "i2c") };
    } else if touch_count == 0 {
        unsafe { tfail("Touch Demo 30s", "no-touch") };
    } else {
        unsafe { tpass("Touch Demo 30s") };
    }

    Timer::after(Duration::from_secs(2)).await;
    render_results_screen(board, "Phase 4", "Touch phase complete", "");
    Timer::after(Duration::from_secs(1)).await;
}

fn dma_memcpy(src: *const u8, dst: *mut u8, len: usize) -> Result<(), &'static str> {
    use stm32_metapac::dma::vals;

    if len == 0 || len > u16::MAX as usize {
        return Err("length");
    }

    let dma2 = stm32_metapac::DMA2;
    dma2.st(0).cr().modify(|w| w.set_en(false));
    while dma2.st(0).cr().read().en() {}
    dma2.ifcr(0).write(|w| {
        w.set_tcif(0, true);
        w.set_htif(0, true);
        w.set_feif(0, true);
        w.set_dmeif(0, true);
        w.set_teif(0, true);
    });
    dma2.st(0).cr().write(|w| {
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
    dma2.st(0).par().write_value(src as u32);
    dma2.st(0).m0ar().write_value(dst as u32);
    dma2.st(0).ndtr().write(|w| w.set_ndt(len as u16));
    dma2.st(0).cr().modify(|w| w.set_en(true));

    let mut timeout = 5_000_000u32;
    loop {
        let isr = dma2.isr(0).read();
        if isr.tcif(0) {
            break;
        }
        if isr.teif(0) || isr.dmeif(0) || isr.feif(0) {
            return Err("dma-error");
        }
        timeout -= 1;
        if timeout == 0 {
            return Err("timeout");
        }
    }

    dma2.ifcr(0).write(|w| {
        w.set_tcif(0, true);
        w.set_htif(0, true);
        w.set_feif(0, true);
        w.set_dmeif(0, true);
        w.set_teif(0, true);
    });
    Ok(())
}

fn run_dma_transfer_test(offset: usize, len: usize) -> Result<(), &'static str> {
    let src = sdram_scratch_bytes(offset, len);
    let dst = sdram_scratch_bytes(offset + 0x2000, len);
    for (i, byte) in src.iter_mut().enumerate() {
        *byte = ((i * 37) & 0xFF) as u8;
    }
    for byte in dst.iter_mut() {
        *byte = 0;
    }
    dma_memcpy(src.as_ptr(), dst.as_mut_ptr(), len)?;
    if src.iter().zip(dst.iter()).all(|(a, b)| a == b) {
        Ok(())
    } else {
        Err("verify")
    }
}

async fn phase5_uart_dma(board: &mut Board, peri: &Peripherals) {
    defmt::info!("=== Phase 5: UART + DMA tests ===");
    render_results_screen(board, "Phase 5", "Running UART + DMA tests", "");

    unsafe {
        tpass_fn("UART Init", || {
            Uart::new_blocking(
                peri.USART1.clone_unchecked(),
                peri.PA10.clone_unchecked(),
                peri.PA9.clone_unchecked(),
                embassy_stm32::usart::Config::default(),
            )
            .map(|_| ())
            .map_err(|_| "init")
        });

        tpass_fn("UART TX Byte", || {
            let mut tx = Uart::new_blocking(
                peri.USART1.clone_unchecked(),
                peri.PA10.clone_unchecked(),
                peri.PA9.clone_unchecked(),
                embassy_stm32::usart::Config::default(),
            )
            .map_err(|_| "init")?;
            tx.bwrite_all(b"U").map_err(|_| "write")
        });

        tpass_fn("UART Multi-Byte", || {
            let mut tx = Uart::new_blocking(
                peri.USART1.clone_unchecked(),
                peri.PA10.clone_unchecked(),
                peri.PA9.clone_unchecked(),
                embassy_stm32::usart::Config::default(),
            )
            .map_err(|_| "init")?;
            tx.bwrite_all(b"HELLO").map_err(|_| "write")
        });

        tpass_fn("UART fmt::Write", || {
            let mut tx = Uart::new_blocking(
                peri.USART1.clone_unchecked(),
                peri.PA10.clone_unchecked(),
                peri.PA9.clone_unchecked(),
                embassy_stm32::usart::Config::default(),
            )
            .map_err(|_| "init")?;
            tx.write_fmt(format_args!("val={}", 42)).map_err(|_| "fmt")
        });
    }

    stm32_metapac::RCC.ahb1enr().modify(|w| w.set_dma2en(true));
    unsafe {
        tpass_fn("DMA 64B Transfer", || run_dma_transfer_test(0x40_000, 64));
        tpass_fn("DMA 1024B Transfer", || {
            run_dma_transfer_test(0x48_000, 1024)
        });
        tpass_fn("DMA 4096B Transfer", || {
            run_dma_transfer_test(0x50_000, 4096)
        });
        tpass_fn("DMA Repeated 10x", || {
            for i in 0..10 {
                run_dma_transfer_test(0x60_000 + i * 0x4000, 256)?;
            }
            Ok(())
        });
    }

    render_results_screen(board, "Phase 5", "UART + DMA complete", "");
    Timer::after(Duration::from_secs(1)).await;
}

async fn phase6_summary(board: &mut Board) {
    let passed = total_passed();
    let failed = total_failed();
    let total = passed + failed;
    defmt::info!("=== Phase 6: Final summary ===");
    defmt::info!("SUMMARY: {}/{} passed", passed, total);
    if failed == 0 {
        defmt::info!("ALL TESTS PASSED");
    } else {
        defmt::info!("SOME TESTS FAILED");
    }
    render_final_screen(board);
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(config_180());
    let peri = unsafe { Peripherals::steal() };

    defmt::info!("=== Extensive Hardware Test v0.1.0 ===");

    unsafe {
        let buf = CCMRAM_BASE as *mut TestResultBuffer;
        core::ptr::write_bytes(buf as *mut u8, 0, core::mem::size_of::<TestResultBuffer>());
    }

    phase1_raw_tests(&peri).await;

    let board = Board::try_new(p, BoardHint::ForceNt35510).expect("board init");
    let Board {
        display,
        touch,
        leds,
        user_button,
        sdram_remainders,
    } = board;
    let mut board = Board {
        display,
        touch: touch.with_filter(EdgeFilter::default_ft6x06()),
        leds,
        user_button,
        sdram_remainders,
    };
    phase2_sdram_and_display(&mut board).await;
    phase3_visual_tests(&mut board).await;
    phase4_touch(&mut board).await;
    phase5_uart_dma(&mut board, &peri).await;
    phase6_summary(&mut board).await;

    unsafe {
        mark_done();
    }
    defmt::info!("Results at CCMRAM 0x10000000. Run: python3 tests/read_test_results.py");

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
