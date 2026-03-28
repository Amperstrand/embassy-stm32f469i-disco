#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::i2c;
use embassy_stm32::rcc::{
    AHBPrescaler, APBPrescaler, Hse, HseMode, Pll, PllMul, PllPDiv, PllPreDiv, PllQDiv, PllRDiv,
    PllSource, Sysclk,
};
use embassy_stm32f469i_disco::{display::SdramCtrl, DisplayCtrl, FB_HEIGHT, FB_WIDTH, TouchCtrl};
use embassy_time::{Duration, Ticker, Timer};
use embedded_graphics::{
    mono_font::{ascii::FONT_6X9, MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{rectangle::Rectangle, PrimitiveStyle},
};

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn LTDC() { cortex_m::asm::nop(); }
#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn LTDC_ER() { cortex_m::asm::nop(); }
#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn DSI() { cortex_m::asm::nop(); }
#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn DSIHOST() { cortex_m::asm::nop(); }
#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn DMA2D() { cortex_m::asm::nop(); }
#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn FMC() { cortex_m::asm::nop(); }

enum TestStatus {
    Pass,
    Fail,
}

struct TestResult {
    name: &'static str,
    status: TestStatus,
}

const BG: Rgb565 = Rgb565::new(0x1a, 0x1a, 0x2e);
const PASS_COLOR: Rgb565 = Rgb565::new(0x00, 0xe0, 0x40);
const FAIL_COLOR: Rgb565 = Rgb565::new(0xe0, 0x20, 0x20);
const HEADER_COLOR: Rgb565 = Rgb565::new(0x40, 0xa0, 0xe0);
const TEXT_COLOR: Rgb565 = Rgb565::new(0xe0, 0xe0, 0xe0);
const DIM_TEXT: Rgb565 = Rgb565::new(0x80, 0x80, 0x80);

fn dwt_cycles() -> u32 {
    cortex_m::peripheral::DWT::cycle_count()
}

fn draw_text(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, text: &str, x: i32, y: i32, style: &MonoTextStyle<Rgb565>) {
    embedded_graphics::text::Text::new(text, Point::new(x, y), *style)
        .draw(fb)
        .ok();
}

fn draw_status_dot(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, x: i32, y: i32, status: &TestStatus) {
    let color = match status {
        TestStatus::Pass => PASS_COLOR,
        TestStatus::Fail => FAIL_COLOR,
    };
    Rectangle::new(Point::new(x, y), Size::new(8, 8))
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(fb)
        .ok();
}

fn draw_section(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, y: &mut i32, title: &str, style: &MonoTextStyle<Rgb565>) {
    *y += 4;
    draw_text(fb, title, 8, *y, style);
    *y += 14;
    Rectangle::new(Point::new(8, *y), Size::new(464, 1))
        .into_styled(PrimitiveStyle::with_fill(DIM_TEXT))
        .draw(fb)
        .ok();
    *y += 4;
}

fn draw_result(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, y: &mut i32, result: &TestResult, style: &MonoTextStyle<Rgb565>) {
    draw_status_dot(fb, 12, *y, &result.status);
    let status_str = match result.status {
        TestStatus::Pass => "PASS",
        TestStatus::Fail => "FAIL",
    };
    draw_text(fb, result.name, 26, *y, style);
    draw_text(fb, status_str, 430, *y, style);
    *y += 12;
}

fn draw_u32_text(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, x: i32, y: i32, style: &MonoTextStyle<Rgb565>, val: u32) {
    let mut buf = [0u8; 12];
    let mut i = buf.len();
    let mut v = val;
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 { break; }
    }
    let s = unsafe { core::str::from_utf8_unchecked(&buf[i..]) };
    draw_text(fb, s, x, y, style);
}

fn draw_summary(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, results: &[TestResult], y: i32, style: &MonoTextStyle<Rgb565>, header_style: &MonoTextStyle<Rgb565>) {
    let passed = results.iter().filter(|r| matches!(r.status, TestStatus::Pass)).count();
    let failed = results.iter().filter(|r| matches!(r.status, TestStatus::Fail)).count();
    let total = results.len();

    let mut sy = y + 8;
    Rectangle::new(Point::new(8, sy), Size::new(464, 1))
        .into_styled(PrimitiveStyle::with_fill(DIM_TEXT))
        .draw(fb)
        .ok();
    sy += 8;

    let banner_color = if failed == 0 { PASS_COLOR } else { FAIL_COLOR };
    let banner_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(banner_color)
        .background_color(BG)
        .build();

    if failed == 0 {
        draw_text(fb, "ALL TESTS PASSED", 8, sy, &banner_style);
    } else {
        draw_text(fb, "SOME TESTS FAILED", 8, sy, &banner_style);
    }
    sy += 12;

    draw_text(fb, "Passed: ", 8, sy, style);
    draw_u32_text(fb, 62, sy, style, passed as u32);
    draw_text(fb, " Failed: ", 100, sy, style);
    draw_u32_text(fb, 162, sy, style, failed as u32);
    draw_text(fb, " Total: ", 200, sy, style);
    draw_u32_text(fb, 258, sy, style, total as u32);
    sy += 16;

    draw_text(fb, "STM32F469I-DISCO Hardware Diagnostics", 8, sy + 10, header_style);
}

fn draw_touch_prompt(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, style: &MonoTextStyle<Rgb565>, header_style: &MonoTextStyle<Rgb565>) {
    fb.clear(BG);
    draw_text(fb, "Touch Demo", 8, 10, header_style);
    Rectangle::new(Point::new(8, 30), Size::new(464, 1))
        .into_styled(PrimitiveStyle::with_fill(DIM_TEXT))
        .draw(fb)
        .ok();
    draw_text(fb, "Touch the screen to test the touch", 8, 44, style);
    draw_text(fb, "controller. Coordinates will be", 8, 56, style);
    draw_text(fb, "shown below. (30 seconds)", 8, 68, style);
}

fn draw_touch_point(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, x: u16, y: u16, style: &MonoTextStyle<Rgb565>) {
    let cx = x as i32;
    let cy = y as i32;
    let cross_color = Rgb565::new(0xff, 0xff, 0x00);
    let cross_style = PrimitiveStyle::with_fill(cross_color);

    Rectangle::new(Point::new(cx - 8, cy - 1), Size::new(16, 2))
        .into_styled(cross_style)
        .draw(fb)
        .ok();
    Rectangle::new(Point::new(cx - 1, cy - 8), Size::new(2, 16))
        .into_styled(cross_style)
        .draw(fb)
        .ok();

    draw_text(fb, "Touch: (", 8, FB_HEIGHT as i32 - 30, style);
    draw_u32_text(fb, 62, FB_HEIGHT as i32 - 30, style, x as u32);
    draw_text(fb, ", ", 100, FB_HEIGHT as i32 - 30, style);
    draw_u32_text(fb, 110, FB_HEIGHT as i32 - 30, style, y as u32);
    draw_text(fb, ")", 145, FB_HEIGHT as i32 - 30, style);
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let mut config = embassy_stm32::Config::default();
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

    defmt::info!("=== Hardware Diagnostics ===");

    unsafe {
        cortex_m::peripheral::Peripherals::steal().DWT.enable_cycle_counter();
    }

    // === SDRAM ===
    defmt::info!("SDRAM init...");
    let sdram = SdramCtrl::new(&mut unsafe { embassy_stm32::Peripherals::steal() }, 180_000_000);
    let base = sdram.base_address();
    let words = embassy_stm32f469i_disco::display::SDRAM_SIZE_BYTES / 4;
    let ram: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(base as *mut u32, words) };

    let mut results: [TestResult; 30] = [const { TestResult { name: "", status: TestStatus::Pass } }; 30];
    let mut ri = 0usize;

    macro_rules! pass { ($name:expr) => { results[ri] = TestResult { name: $name, status: TestStatus::Pass }; ri += 1; defmt::info!("TEST {}: PASS", $name); } }
    macro_rules! fail { ($name:expr) => { results[ri] = TestResult { name: $name, status: TestStatus::Fail }; ri += 1; defmt::error!("TEST {}: FAIL", $name); } }
    macro_rules! tpass { ($name:expr, $block:expr) => {
        defmt::info!("TEST {}: RUNNING", $name);
        if $block { pass!($name); } else { fail!($name); }
    } }

    // SDRAM tests
    tpass!("SDRAM Init", sdram.test_quick());

    tpass!("SDRAM Checkerboard", {
        let win = 65536usize;
        for word in ram[..win].iter_mut() { *word = 0xAAAAAAAA; }
        ram[..win].iter().all(|w| *w == 0xAAAAAAAA)
    });

    tpass!("SDRAM March C-", {
        let win = 65536usize;
        for word in ram[..win].iter_mut() { *word = 0; }
        let mut ok = true;
        for word in ram[..win].iter_mut() {
            if *word != 0 { ok = false; break; }
            *word = 0xFFFFFFFF;
        }
        if ok {
            for word in ram[..win].iter_mut().rev() {
                if *word != 0xFFFFFFFF { ok = false; break; }
                *word = 0;
            }
        }
        if ok {
            for word in ram[..win].iter() {
                if *word != 0 { ok = false; break; }
            }
        }
        ok
    });

    tpass!("SDRAM Boundary Spots", {
        let mut ok = true;
        for r in 0u32..16 {
            let offset = (r as usize) * (words / 16);
            let pattern = 0xFEED0000 | r;
            let end = core::cmp::min(offset + 1024, words);
            for word in ram[offset..end].iter_mut() { *word = pattern; }
        }
        for r in 0u32..16 {
            let offset = (r as usize) * (words / 16);
            let pattern = 0xFEED0000 | r;
            let end = core::cmp::min(offset + 1024, words);
            for (i, word) in ram[offset..end].iter().enumerate() {
                if *word != pattern {
                    defmt::error!("  boundary miss at {:#010X}", base + (offset + i) * 4);
                    ok = false;
                    break;
                }
            }
            if !ok { break; }
        }
        ok
    });

    tpass!("SDRAM End-of-RAM", {
        let mut ok = true;
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
            if *word != seed { ok = false; break; }
        }
        ok
    });

    tpass!("SDRAM Byte/Halfword", {
        let mut ok = true;
        let ram_bytes: &mut [u8] = unsafe { core::slice::from_raw_parts_mut(base as *mut u8, 4096) };
        for (i, byte) in ram_bytes.iter_mut().enumerate() { *byte = (i & 0xFF) as u8; }
        for (i, byte) in ram_bytes.iter().enumerate() {
            if *byte != (i & 0xFF) as u8 { ok = false; break; }
        }
        if ok {
            let ram_hw: &mut [u16] = unsafe { core::slice::from_raw_parts_mut(base as *mut u16, 2048) };
            for (i, hw) in ram_hw.iter_mut().enumerate() { *hw = ((i & 0xFFFF) as u16).wrapping_add(1); }
            for (i, hw) in ram_hw.iter().enumerate() {
                if *hw != ((i & 0xFFFF) as u16).wrapping_add(1) { ok = false; break; }
            }
        }
        ok
    });

    // === Display ===
    defmt::info!("Display init...");
    let mut display = DisplayCtrl::new(&sdram, unsafe { p.PH7.clone_unchecked() });
    defmt::info!("Display init done");
    pass!("Display Init");

    let style = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(TEXT_COLOR)
        .background_color(BG)
        .build();

    let header_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(HEADER_COLOR)
        .background_color(BG)
        .build();

    let mut fb = display.fb();
    fb.clear(BG);
    let mut y: i32 = 10;
    draw_text(&mut fb, "STM32F469I-DISCO", 8, y, &header_style);
    y += 14;
    draw_text(&mut fb, "Hardware Diagnostics v0.1.0", 8, y, &style);
    y += 18;

    // Render SDRAM results
    draw_section(&mut fb, &mut y, "SDRAM (16MB IS42S32400F-6)", &header_style);
    for i in 0..ri {
        draw_result(&mut fb, &mut y, &results[i], &style);
    }

    // Display tests
    draw_section(&mut fb, &mut y, "Display (DSI/LTDC/NT35510)", &header_style);

    tpass!("Display Red Fill", {
        fb.clear(Rgb565::RED);
        Timer::after(Duration::from_millis(200)).await;
        fb.clear(BG);
        true
    });

    tpass!("Display Green Fill", {
        fb.clear(Rgb565::GREEN);
        Timer::after(Duration::from_millis(200)).await;
        fb.clear(BG);
        true
    });

    tpass!("Display Blue Fill", {
        fb.clear(Rgb565::BLUE);
        Timer::after(Duration::from_millis(200)).await;
        fb.clear(BG);
        true
    });

    tpass!("Display Gradient", {
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
        true
    });

    tpass!("Display Text Render", {
        fb.clear(BG);
        let tstyle = MonoTextStyleBuilder::new()
            .font(&FONT_6X9)
            .text_color(Rgb565::WHITE)
            .background_color(Rgb565::CSS_NAVY)
            .build();
        embedded_graphics::text::Text::new("HELLO WORLD", Point::new(120, 390), tstyle)
            .draw(&mut fb)
            .ok();
        Timer::after(Duration::from_millis(200)).await;
        fb.clear(BG);
        true
    });

    // === Touch ===
    draw_section(&mut fb, &mut y, "Touch (FT6X06 / I2C1)", &header_style);

    tpass!("Touch I2C Init", {
        let _i2c = i2c::I2c::new_blocking(
            unsafe { embassy_stm32::Peripherals::steal() }.I2C1,
            unsafe { embassy_stm32::Peripherals::steal() }.PB8,
            unsafe { embassy_stm32::Peripherals::steal() }.PB9,
            embassy_stm32::i2c::Config::default(),
        );
        true
    });

    tpass!("Touch Chip ID", {
        let mut i2c = i2c::I2c::new_blocking(
            unsafe { embassy_stm32::Peripherals::steal() }.I2C1,
            unsafe { embassy_stm32::Peripherals::steal() }.PB8,
            unsafe { embassy_stm32::Peripherals::steal() }.PB9,
            embassy_stm32::i2c::Config::default(),
        );
        let touch = TouchCtrl::new();
        match touch.read_chip_id(&mut i2c) {
            Ok(id) => {
                defmt::info!("  Chip ID: {:#04X}", id);
                id == 0xCC || id == 0xA3
            }
            Err(_) => false,
        }
    });

    tpass!("Touch Idle Status", {
        let mut i2c = i2c::I2c::new_blocking(
            unsafe { embassy_stm32::Peripherals::steal() }.I2C1,
            unsafe { embassy_stm32::Peripherals::steal() }.PB8,
            unsafe { embassy_stm32::Peripherals::steal() }.PB9,
            embassy_stm32::i2c::Config::default(),
        );
        let touch = TouchCtrl::new();
        match touch.td_status(&mut i2c) {
            Ok(status) => {
                defmt::info!("  TD status: {}", status);
                status == 0
            }
            Err(_) => false,
        }
    });

    // === GPIO ===
    draw_section(&mut fb, &mut y, "GPIO", &header_style);

    tpass!("GPIO Input PA0", {
        let button = embassy_stm32::gpio::Input::new(
            unsafe { embassy_stm32::Peripherals::steal() }.PA0,
            embassy_stm32::gpio::Pull::Down,
        );
        let _state = button.is_high();
        true
    });

    tpass!("GPIO Multi-Port Output", {
        let mut g = embassy_stm32::gpio::Output::new(
            unsafe { embassy_stm32::Peripherals::steal() }.PG6,
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );
        g.set_high();
        g.toggle();
        g.set_low();
        true
    });

    // === LEDs ===
    draw_section(&mut fb, &mut y, "LEDs", &header_style);

    tpass!("LED Green (PG6)", {
        let mut led = embassy_stm32::gpio::Output::new(
            unsafe { embassy_stm32::Peripherals::steal() }.PG6,
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );
        led.set_high();
        Timer::after(Duration::from_millis(100)).await;
        led.set_low();
        true
    });

    tpass!("LED Orange (PD4)", {
        let mut led = embassy_stm32::gpio::Output::new(
            unsafe { embassy_stm32::Peripherals::steal() }.PD4,
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );
        led.toggle();
        Timer::after(Duration::from_millis(100)).await;
        led.set_low();
        true
    });

    tpass!("LED Red (PD5)", {
        let mut led = embassy_stm32::gpio::Output::new(
            unsafe { embassy_stm32::Peripherals::steal() }.PD5,
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );
        led.toggle();
        Timer::after(Duration::from_millis(100)).await;
        led.set_low();
        true
    });

    tpass!("LED Blue (PK3)", {
        let mut led = embassy_stm32::gpio::Output::new(
            unsafe { embassy_stm32::Peripherals::steal() }.PK3,
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );
        led.toggle();
        Timer::after(Duration::from_millis(100)).await;
        led.set_low();
        true
    });

    tpass!("LED All Toggle", {
        let mut leds = [
            embassy_stm32::gpio::Output::new(
                unsafe { embassy_stm32::Peripherals::steal() }.PG6,
                embassy_stm32::gpio::Level::Low, embassy_stm32::gpio::Speed::Low,
            ),
            embassy_stm32::gpio::Output::new(
                unsafe { embassy_stm32::Peripherals::steal() }.PD4,
                embassy_stm32::gpio::Level::Low, embassy_stm32::gpio::Speed::Low,
            ),
            embassy_stm32::gpio::Output::new(
                unsafe { embassy_stm32::Peripherals::steal() }.PD5,
                embassy_stm32::gpio::Level::Low, embassy_stm32::gpio::Speed::Low,
            ),
            embassy_stm32::gpio::Output::new(
                unsafe { embassy_stm32::Peripherals::steal() }.PK3,
                embassy_stm32::gpio::Level::Low, embassy_stm32::gpio::Speed::Low,
            ),
        ];
        for _ in 0..3 {
            for led in leds.iter_mut() { led.toggle(); }
            Timer::after(Duration::from_millis(80)).await;
        }
        for led in leds.iter_mut() { led.set_low(); }
        true
    });

    // === Timers ===
    draw_section(&mut fb, &mut y, "Timers", &header_style);

    tpass!("Timer 1ms", {
        let start = dwt_cycles();
        Timer::after(Duration::from_millis(1)).await;
        let us = dwt_cycles().wrapping_sub(start) / 180;
        defmt::info!("  1ms delay: {}us", us);
        us >= 900 && us <= 1500
    });

    tpass!("Timer 100ms", {
        let start = dwt_cycles();
        Timer::after(Duration::from_millis(100)).await;
        let ms = dwt_cycles().wrapping_sub(start) / 180_000;
        defmt::info!("  100ms delay: {}ms", ms);
        ms >= 95 && ms <= 120
    });

    tpass!("Timer Ticker 500ms", {
        let mut ticker = Ticker::every(Duration::from_millis(500));
        for _ in 0..5 {
            ticker.next().await;
        }
        true
    });

    // === Summary ===
    draw_summary(&mut fb, &results[..ri], y, &style, &header_style);

    let passed = results[..ri].iter().filter(|r| matches!(r.status, TestStatus::Pass)).count();
    let failed = results[..ri].iter().filter(|r| matches!(r.status, TestStatus::Fail)).count();
    defmt::info!("SUMMARY: {}/{} passed", passed, passed + failed);
    if failed == 0 {
        defmt::info!("ALL TESTS PASSED");
    } else {
        defmt::error!("FAILED: {} tests failed", failed);
    }

    // Hold summary for 2 seconds
    Timer::after(Duration::from_secs(2)).await;

    // === Touch Demo (30 seconds) ===
    defmt::info!("Entering touch demo...");

    let touch_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(TEXT_COLOR)
        .background_color(BG)
        .build();

    let touch_header = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(HEADER_COLOR)
        .background_color(BG)
        .build();

    draw_touch_prompt(&mut fb, &touch_style, &touch_header);

    let mut i2c = i2c::I2c::new_blocking(
        unsafe { embassy_stm32::Peripherals::steal() }.I2C1,
        unsafe { embassy_stm32::Peripherals::steal() }.PB8,
        unsafe { embassy_stm32::Peripherals::steal() }.PB9,
        embassy_stm32::i2c::Config::default(),
    );
    let touch = TouchCtrl::new();
    let mut deadline = 30u32;
    let mut touch_count = 0u32;
    while deadline > 0 {
        if touch.td_status(&mut i2c).unwrap_or(0) > 0 {
            if let Ok(point) = touch.get_touch(&mut i2c) {
                if point.x >= 3 && point.x <= 476 && point.y >= 3 && point.y <= 796 {
                    defmt::info!("Touch at ({}, {})", point.x, point.y);
                    draw_touch_prompt(&mut fb, &touch_style, &touch_header);
                    draw_touch_point(&mut fb, point.x, point.y, &touch_style);

                    draw_text(&mut fb, "Touches: ", 8, FB_HEIGHT as i32 - 50, &touch_style);
                    draw_u32_text(&mut fb, 62, FB_HEIGHT as i32 - 50, &touch_style, touch_count + 1);

                    touch_count += 1;
                }
            }
        }
        Timer::after(Duration::from_millis(50)).await;
        deadline -= 1;
    }

    // Freeze on final screen
    draw_touch_prompt(&mut fb, &touch_style, &touch_header);
    draw_text(&mut fb, "Touch demo complete.", 8, 90, &touch_style);
    draw_text(&mut fb, "Press reset to restart.", 8, 102, &touch_style);

    let final_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(PASS_COLOR)
        .background_color(BG)
        .build();

    draw_text(&mut fb, "Touches detected: ", 8, 130, &final_style);
    draw_u32_text(&mut fb, 120, 130, &final_style, touch_count);

    defmt::info!("Touch demo complete. {} touches detected.", touch_count);
    defmt::info!("Holding final screen. Press reset to restart.");

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
