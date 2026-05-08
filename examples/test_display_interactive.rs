//! Display + touch diagnostic with USB serial log output.
//!
//! Phase 1: Border tests (1px, 2px, 3px, ruler) — touch YES or NO
//! Phase 2: Color/shape verification — tap the labeled shape
//! Phase 3: Corner taps (5 per corner, measures spread)
//! Phase 4: Swipe lines (drag finger between two markers)
//! Phase 5: Random dot chase (10 rounds, measures accuracy)
//!
//! Deploy:
//!   cargo build --release --target thumbv7em-none-eabihf --example test_display_interactive --features defmt
//!   arm-none-eabi-objcopy -O binary target/.../test_display_interactive test.bin
//!   st-flash --connect-under-reset write test.bin 0x08000000
//!   st-flash --connect-under-reset reset
//!   sleep 15
//!   python3 -c "import serial,time;s=serial.Serial('/dev/ttyACM1',115200,timeout=120);..."

#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::i2c;
use embassy_stm32::rcc::*;
use embassy_stm32::time::Hertz;
use embassy_stm32::{bind_interrupts, peripherals, usb, Config};
use embassy_stm32f469i_disco::{
    display::SdramCtrl, BoardHint, DisplayCtrl, TouchCtrl, FB_HEIGHT, FB_WIDTH,
};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::Builder;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle},
    text::{Baseline, Text},
    Pixel,
};
use panic_halt as _;
use static_cell::StaticCell;

#[no_mangle]
unsafe extern "C" fn _defmt_acquire() -> usize {
    0
}
#[no_mangle]
unsafe extern "C" fn _defmt_write(_data: *const u8, _len: usize) {}
#[no_mangle]
unsafe extern "C" fn _defmt_release(_addr: usize) {}

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

#[global_allocator]
static ALLOCATOR: linked_list_allocator::LockedHeap = linked_list_allocator::LockedHeap::empty();
static mut HEAP_MEMORY: [u8; 64 * 1024] = [0; 64 * 1024];

const W: i32 = FB_WIDTH as i32;
const H: i32 = FB_HEIGHT as i32;
const MARGIN: u16 = 3;
const TAPS_PER_CORNER: usize = 5;
const DOT_ROUNDS: usize = 10;

const YES_RECT: Rectangle = Rectangle::new(Point::new(40, H - 120), Size::new(180, 80));
const NO_RECT: Rectangle = Rectangle::new(Point::new(260, H - 120), Size::new(180, 80));

static USB_LOG: Signal<CriticalSectionRawMutex, Vec<String>> = Signal::new();

type UsbDriver = usb::Driver<'static, peripherals::USB_OTG_FS>;

#[embassy_executor::task]
async fn usb_task(mut usb_dev: embassy_usb::UsbDevice<'static, UsbDriver>) {
    usb_dev.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    unsafe {
        ALLOCATOR
            .lock()
            .init(core::ptr::addr_of_mut!(HEAP_MEMORY) as *mut u8, 64 * 1024);
    }

    let mut config = Config::default();
    config.rcc.hse = Some(Hse {
        freq: Hertz(8_000_000),
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
        divp: Some(PllPDiv::DIV8),
        divq: Some(PllQDiv::DIV8),
        divr: Some(PllRDiv::DIV7),
    });
    config.rcc.mux.clk48sel = mux::Clk48sel::PLLSAI1_Q;
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.sys = Sysclk::PLL1_P;
    let mut p = embassy_stm32::init(config);

    let sdram = SdramCtrl::new(&mut p, 180_000_000);
    let framebuffer = sdram.into_bytes();
    let mut display = DisplayCtrl::new(
        framebuffer,
        p.LTDC,
        p.DSIHOST,
        p.PJ2,
        p.PH7,
        BoardHint::ForceNt35510,
    );
    let mut fb = display.fb();
    let i2c = i2c::I2c::new_blocking(p.I2C1, p.PB8, p.PB9, i2c::Config::default());
    let mut touch = TouchCtrl::new(i2c);
    let mut led = Output::new(p.PG6, Level::Low, Speed::Low);

    usb_phy_reset();

    static EP_BUF: StaticCell<[u8; 512]> = StaticCell::new();
    let ep_buf = EP_BUF.init([0u8; 512]);
    let mut usb_cfg = usb::Config::default();
    usb_cfg.vbus_detection = false;
    let driver = usb::Driver::new_fs(p.USB_OTG_FS, Irqs, p.PA12, p.PA11, ep_buf, usb_cfg);

    static CFG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CTRL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

    let mut usb_desc = embassy_usb::Config::new(0x16c0, 0x27dd);
    usb_desc.manufacturer = Some("Amperstrand");
    usb_desc.product = Some("DisplayTest");
    usb_desc.serial_number = Some("f469test");

    static USB_STATE: StaticCell<State> = StaticCell::new();
    let usb_state = USB_STATE.init(State::new());
    let mut builder = Builder::new(
        driver,
        usb_desc,
        CFG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        MSOS_DESC.init([0; 256]),
        CTRL_BUF.init([0; 64]),
    );
    let mut cdc = CdcAcmClass::new(&mut builder, usb_state, 64);
    let usb_dev = builder.build();
    let spawn_token = usb_task(usb_dev).unwrap();
    spawner.spawn(spawn_token);

    let mut log: Vec<String> = Vec::new();
    let mut results: Vec<(String, bool)> = Vec::new();
    let s_wh = MonoTextStyle::new(&FONT_10X20, Rgb888::WHITE);
    let s_gr = MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_GREEN);
    let s_rd = MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_RED);
    let s_ye = MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_YELLOW);
    let s_cy = MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_CYAN);
    let s_gy = MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_GRAY);

    log.push(String::from("Boot OK (180MHz)\n"));

    // ── Phase 1: Border tests with YES/NO ──

    log.push(String::from("\n--- PHASE 1: BORDER TESTS ---\n"));

    type BorderTestFn = fn(&mut embassy_stm32f469i_disco::FramebufferView<'_>, i32);

    let border_tests: &[(&str, BorderTestFn)] = &[
        ("1px WHITE border", draw_border_1px),
        ("2px CYAN border", draw_border_2px),
        ("3px YELLOW border", draw_border_3px),
        ("Ruler (10px ticks)", draw_ruler),
    ];

    for (i, (name, draw_fn)) in border_tests.iter().enumerate() {
        fb.clear(Rgb888::BLACK);
        draw_fn(&mut fb, W);

        let title = alloc::format!("[{}/{}] {}", i + 1, border_tests.len(), name);
        Text::with_baseline(&title, Point::new(80, 30), s_wh, Baseline::Top)
            .draw(&mut fb)
            .ok();
        Text::with_baseline("Is this correct?", Point::new(120, 60), s_ye, Baseline::Top)
            .draw(&mut fb)
            .ok();

        draw_yes_no(&mut fb);

        let answer = wait_for_yes_no(&mut touch).await;

        results.push((String::from(*name), answer));
        log.push(alloc::format!(
            "  {}: {}\n",
            name,
            if answer { "YES" } else { "NO" }
        ));
        blink(&mut led, 1).await;
    }

    // ── Phase 2: Color/shape verification ──

    log.push(String::from("\n--- PHASE 2: COLOR/SHAPE VERIFY ---\n"));

    #[derive(Clone, Copy)]
    enum Shape {
        Circle,
        Rect,
        Triangle,
    }

    struct ColorTest {
        name: &'static str,
        color: Rgb888,
        text_color: Rgb888,
        shape: Shape,
    }

    let color_tests: &[ColorTest] = &[
        ColorTest {
            name: "RED circle",
            color: Rgb888::CSS_RED,
            text_color: Rgb888::WHITE,
            shape: Shape::Circle,
        },
        ColorTest {
            name: "GREEN rect",
            color: Rgb888::CSS_GREEN,
            text_color: Rgb888::BLACK,
            shape: Shape::Rect,
        },
        ColorTest {
            name: "BLUE triangle",
            color: Rgb888::CSS_BLUE,
            text_color: Rgb888::WHITE,
            shape: Shape::Triangle,
        },
        ColorTest {
            name: "YELLOW circle",
            color: Rgb888::CSS_YELLOW,
            text_color: Rgb888::BLACK,
            shape: Shape::Circle,
        },
        ColorTest {
            name: "CYAN rect",
            color: Rgb888::CSS_CYAN,
            text_color: Rgb888::BLACK,
            shape: Shape::Rect,
        },
        ColorTest {
            name: "WHITE triangle",
            color: Rgb888::WHITE,
            text_color: Rgb888::BLACK,
            shape: Shape::Triangle,
        },
    ];

    for (i, ct) in color_tests.iter().enumerate() {
        let cx = 100 + pseudo_random((i as i32) * 7 + 3) % (W - 200);
        let cy = 150 + pseudo_random((i as i32) * 11 + 5) % (H - 350);
        let sz = 35;

        fb.clear(Rgb888::BLACK);

        match ct.shape {
            Shape::Circle => {
                Circle::new(Point::new(cx - sz, cy - sz), (sz * 2) as u32)
                    .into_styled(PrimitiveStyle::with_fill(ct.color))
                    .draw(&mut fb)
                    .ok();
            }
            Shape::Rect => {
                Rectangle::new(
                    Point::new(cx - sz, cy - sz),
                    Size::new((sz * 2) as u32, (sz * 2) as u32),
                )
                .into_styled(PrimitiveStyle::with_fill(ct.color))
                .draw(&mut fb)
                .ok();
            }
            Shape::Triangle => {
                Triangle::new(
                    Point::new(cx, cy - sz),
                    Point::new(cx - sz, cy + sz),
                    Point::new(cx + sz, cy + sz),
                )
                .into_styled(PrimitiveStyle::with_fill(ct.color))
                .draw(&mut fb)
                .ok();
            }
        }

        let label_s = MonoTextStyle::new(&FONT_10X20, ct.text_color);
        Text::with_baseline(
            ct.name,
            Point::new(cx + sz + 10, cy - 10),
            label_s,
            Baseline::Top,
        )
        .draw(&mut fb)
        .ok();

        let title = alloc::format!("[{}/{}] Tap the {}", i + 1, color_tests.len(), ct.name);
        Text::with_baseline(&title, Point::new(20, 30), s_wh, Baseline::Top)
            .draw(&mut fb)
            .ok();

        let mut last: Option<(i32, i32)> = None;
        let mut tapped = false;
        let deadline = Timer::after(Duration::from_secs(15));
        embassy_futures::select::select3(
            async {
                loop {
                    if let Some((tx, ty)) = read_touch(&mut touch, &mut last).await {
                        let dist = isqrt((tx - cx) * (tx - cx) + (ty - cy) * (ty - cy));
                        tapped = dist < sz + 30;
                        log.push(alloc::format!(
                            "  {} tap at ({},{}), target ({},{}), dist={}\n",
                            ct.name,
                            tx,
                            ty,
                            cx,
                            cy,
                            dist
                        ));
                        break;
                    }
                    Timer::after(Duration::from_millis(30)).await;
                }
            },
            async {
                deadline.await;
            },
            async {
                loop {
                    Timer::after(Duration::from_millis(500)).await;
                    led.toggle();
                }
            },
        )
        .await;

        results.push((String::from(ct.name), tapped));
        log.push(alloc::format!(
            "  {} -> {}\n",
            ct.name,
            if tapped { "PASS" } else { "FAIL/TIMEOUT" }
        ));
        blink(&mut led, 1).await;
    }

    // ── Phase 3: Corner Taps ──

    log.push(alloc::format!(
        "\n--- PHASE 3: CORNER TAPS ({} per corner) ---\n",
        TAPS_PER_CORNER
    ));

    struct Corner {
        name: &'static str,
        label: &'static str,
        hx: i32,
        hy: i32,
    }
    const CORNERS: &[Corner] = &[
        Corner {
            name: "TOP_LEFT",
            label: "TOP-LEFT",
            hx: 20,
            hy: 20,
        },
        Corner {
            name: "TOP_RIGHT",
            label: "TOP-RIGHT",
            hx: 260,
            hy: 20,
        },
        Corner {
            name: "BOTTOM_RIGHT",
            label: "BOTTOM-RIGHT",
            hx: 180,
            hy: H - 100,
        },
        Corner {
            name: "BOTTOM_LEFT",
            label: "BOTTOM-LEFT",
            hx: 20,
            hy: H - 100,
        },
    ];

    for corner in CORNERS.iter() {
        let mut taps: Vec<(i32, i32)> = Vec::new();
        let mut last: Option<(i32, i32)> = None;

        while taps.len() < TAPS_PER_CORNER {
            fb.clear(Rgb888::BLACK);

            Rectangle::new(Point::new(0, 0), Size::new(W as u32, H as u32))
                .into_styled(PrimitiveStyle::with_stroke(Rgb888::CSS_DIM_GRAY, 1))
                .draw(&mut fb)
                .ok();
            draw_corner_refs(&mut fb);

            Text::with_baseline(
                &alloc::format!("Tap: {}", corner.label),
                Point::new(corner.hx, corner.hy),
                s_ye,
                Baseline::Top,
            )
            .draw(&mut fb)
            .ok();
            Text::with_baseline(
                &alloc::format!("{}x", TAPS_PER_CORNER),
                Point::new(corner.hx, corner.hy + 25),
                s_wh,
                Baseline::Top,
            )
            .draw(&mut fb)
            .ok();

            let count_y = corner.hy + 50;
            Text::with_baseline(
                &alloc::format!("{}/{}", taps.len(), TAPS_PER_CORNER),
                Point::new(corner.hx, count_y),
                s_gr,
                Baseline::Top,
            )
            .draw(&mut fb)
            .ok();

            for &(px, py) in &taps {
                for dx in -4..=4 {
                    for dy in -4..=4 {
                        Pixel(Point::new(px + dx, py + dy), Rgb888::CSS_GREEN)
                            .draw(&mut fb)
                            .ok();
                    }
                }
            }

            if let Some((tx, ty)) = read_touch(&mut touch, &mut last).await {
                taps.push((tx, ty));
                log.push(alloc::format!(
                    "  {} tap {}/{}: ({},{})\n",
                    corner.name,
                    taps.len(),
                    TAPS_PER_CORNER,
                    tx,
                    ty
                ));
                blink(&mut led, 1).await;
            }
            Timer::after(Duration::from_millis(30)).await;
        }

        let spread = tap_spread(&taps);
        let ok = spread < 80;
        results.push((alloc::format!("CORNER_{}", corner.name), ok));
        log.push(alloc::format!(
            "  {} spread={} -> {}\n",
            corner.name,
            spread,
            if ok { "PASS" } else { "FAIL" }
        ));
    }

    // ── Phase 4: Swipe Test ──

    log.push(String::from("\n--- PHASE 4: SWIPE TEST ---\n"));

    let swipes: &[(&str, i32, i32, i32, i32)] = &[
        ("HORIZONTAL", 40, H / 2, W - 40, H / 2),
        ("VERTICAL", W / 2, 80, W / 2, H - 80),
        ("DIAGONAL_TL_BR", 40, 80, W - 40, H - 80),
        ("DIAGONAL_BL_TR", 40, H - 80, W - 40, 80),
    ];

    for (name, x1, y1, x2, y2) in swipes.iter() {
        fb.clear(Rgb888::BLACK);
        Rectangle::new(Point::new(0, 0), Size::new(W as u32, H as u32))
            .into_styled(PrimitiveStyle::with_stroke(Rgb888::CSS_DIM_GRAY, 1))
            .draw(&mut fb)
            .ok();

        Line::new(Point::new(*x1, *y1), Point::new(*x2, *y2))
            .into_styled(PrimitiveStyle::with_stroke(Rgb888::CSS_DIM_GRAY, 1))
            .draw(&mut fb)
            .ok();

        for r in 8..=15 {
            Circle::new(Point::new(*x1 - r, *y1 - r), (r * 2) as u32)
                .into_styled(PrimitiveStyle::with_stroke(Rgb888::CSS_GREEN, 1))
                .draw(&mut fb)
                .ok();
        }
        for r in 8..=15 {
            Circle::new(Point::new(*x2 - r, *y2 - r), (r * 2) as u32)
                .into_styled(PrimitiveStyle::with_stroke(Rgb888::CSS_RED, 1))
                .draw(&mut fb)
                .ok();
        }

        Text::with_baseline(
            &alloc::format!("Swipe: {}", name),
            Point::new(120, 10),
            s_ye,
            Baseline::Top,
        )
        .draw(&mut fb)
        .ok();
        Text::with_baseline(
            "Drag green -> red",
            Point::new(130, 35),
            s_wh,
            Baseline::Top,
        )
        .draw(&mut fb)
        .ok();

        log.push(alloc::format!(
            "SWIPE {}: ({},{}) -> ({},{})\n",
            name,
            x1,
            y1,
            x2,
            y2
        ));

        let mut swipe_points: Vec<(i32, i32)> = Vec::new();
        let mut last: Option<(i32, i32)> = None;
        let mut touch_active = false;
        let mut no_touch_count = 0;

        loop {
            match read_touch_raw(&mut touch) {
                Some((tx, ty))
                    if tx >= MARGIN as i32 && tx <= 476 && ty >= MARGIN as i32 && ty <= 796 =>
                {
                    let is_new = match last {
                        Some((lx, ly)) => {
                            (tx - lx).unsigned_abs() > 3 || (ty - ly).unsigned_abs() > 3
                        }
                        None => true,
                    };
                    if is_new {
                        last = Some((tx, ty));
                        swipe_points.push((tx, ty));
                        touch_active = true;
                        no_touch_count = 0;
                        for dx in -2..=2 {
                            for dy in -2..=2 {
                                Pixel(Point::new(tx + dx, ty + dy), Rgb888::CSS_YELLOW)
                                    .draw(&mut fb)
                                    .ok();
                            }
                        }
                    }
                }
                _ => {
                    if touch_active {
                        no_touch_count += 1;
                        if no_touch_count > 30 {
                            break;
                        }
                    }
                }
            }
            Timer::after(Duration::from_millis(10)).await;
        }

        let first = swipe_points.first().copied().unwrap_or((0, 0));
        let last_pt = swipe_points.last().copied().unwrap_or((0, 0));
        let d_start = isqrt((first.0 - *x1) * (first.0 - *x1) + (first.1 - *y1) * (first.1 - *y1));
        let d_end =
            isqrt((last_pt.0 - *x2) * (last_pt.0 - *x2) + (last_pt.1 - *y2) * (last_pt.1 - *y2));
        let swipe_ok = d_start < 60 && d_end < 60;

        results.push((alloc::format!("SWIPE_{}", name), swipe_ok));
        log.push(alloc::format!(
            "  {} pts, d_start={}, d_end={} -> {}\n",
            swipe_points.len(),
            d_start,
            d_end,
            if swipe_ok { "PASS" } else { "FAIL" }
        ));
        blink(&mut led, 1).await;
    }

    // ── Phase 5: Dot Chase ──

    log.push(alloc::format!(
        "\n--- PHASE 5: DOT CHASE ({} rounds) ---\n",
        DOT_ROUNDS
    ));
    let mut dot_results: Vec<(i32, i32, i32, i32, i32)> = Vec::new();

    for round in 0..DOT_ROUNDS {
        let dx = pseudo_random(round as i32 * 3 + 7) % (W - 80) + 40;
        let dy = pseudo_random(round as i32 * 5 + 13) % (H - 80) + 40;

        fb.clear(Rgb888::BLACK);
        Rectangle::new(Point::new(0, 0), Size::new(W as u32, H as u32))
            .into_styled(PrimitiveStyle::with_stroke(Rgb888::CSS_DIM_GRAY, 1))
            .draw(&mut fb)
            .ok();

        Text::with_baseline(
            &alloc::format!("[{}/{}]", round + 1, DOT_ROUNDS),
            Point::new(10, 10),
            s_gy,
            Baseline::Top,
        )
        .draw(&mut fb)
        .ok();
        Text::with_baseline("Tap the dot", Point::new(150, 10), s_wh, Baseline::Top)
            .draw(&mut fb)
            .ok();

        for r in 5..=20 {
            Circle::new(Point::new(dx - r, dy - r), (r * 2) as u32)
                .into_styled(PrimitiveStyle::with_stroke(Rgb888::CSS_YELLOW, 1))
                .draw(&mut fb)
                .ok();
        }
        for ddx in -6..=6 {
            for ddy in -6..=6 {
                Pixel(Point::new(dx + ddx, dy + ddy), Rgb888::CSS_RED)
                    .draw(&mut fb)
                    .ok();
            }
        }

        log.push(alloc::format!(
            "  DOT[{}]: target at ({},{})\n",
            round + 1,
            dx,
            dy
        ));

        let mut last: Option<(i32, i32)> = None;
        loop {
            if let Some((tx, ty)) = read_touch(&mut touch, &mut last).await {
                let dist = isqrt((tx - dx) * (tx - dx) + (ty - dy) * (ty - dy));
                dot_results.push((dx, dy, tx, ty, dist));
                log.push(alloc::format!(
                    "    touched ({},{}) dist={}\n",
                    tx,
                    ty,
                    dist
                ));
                for ddx in -4..=4 {
                    for ddy in -4..=4 {
                        Pixel(Point::new(tx + ddx, ty + ddy), Rgb888::CSS_GREEN)
                            .draw(&mut fb)
                            .ok();
                    }
                }
                Timer::after(Duration::from_millis(300)).await;
                break;
            }
            Timer::after(Duration::from_millis(30)).await;
        }
        blink(&mut led, 1).await;
    }

    let avg_dot_dist = if dot_results.is_empty() {
        999
    } else {
        dot_results.iter().map(|&(_, _, _, _, d)| d).sum::<i32>() / dot_results.len() as i32
    };
    let dot_ok = avg_dot_dist < 50;
    results.push((String::from("DOT_CHASE"), dot_ok));
    log.push(alloc::format!(
        "  avg_dist={} -> {}\n",
        avg_dot_dist,
        if dot_ok { "PASS" } else { "FAIL" }
    ));

    // ── Summary ──

    let passed = results.iter().filter(|(_, p)| *p).count();
    let total = results.len();
    log.push(String::from("\n=== SUMMARY ===\n"));
    for (name, ok) in &results {
        log.push(alloc::format!(
            "{}: {}\n",
            name,
            if *ok { "PASS" } else { "FAIL" }
        ));
    }
    log.push(alloc::format!("\nTOTAL: {}/{} passed\n", passed, total));
    if passed == total {
        log.push(String::from("ALL TESTS PASSED\n"));
    }

    fb.clear(Rgb888::BLACK);
    let header_s = if passed == total { s_gr } else { s_rd };
    Text::with_baseline(
        &alloc::format!("{}/{}", passed, total),
        Point::new(180, 40),
        header_s,
        Baseline::Top,
    )
    .draw(&mut fb)
    .ok();
    Text::with_baseline(
        "Connect USB for log",
        Point::new(80, 80),
        s_cy,
        Baseline::Top,
    )
    .draw(&mut fb)
    .ok();

    let mut y: i32 = 120;
    for (name, ok) in &results {
        if y > H - 30 {
            break;
        }
        let st = MonoTextStyle::new(
            &FONT_10X20,
            if *ok {
                Rgb888::CSS_GREEN
            } else {
                Rgb888::CSS_RED
            },
        );
        Text::with_baseline(
            &alloc::format!("{}: {}", name, if *ok { "PASS" } else { "FAIL" }),
            Point::new(10, y),
            st,
            Baseline::Top,
        )
        .draw(&mut fb)
        .ok();
        y += 22;
    }

    USB_LOG.signal(log);
    cdc.wait_connection().await;
    Timer::after(Duration::from_millis(300)).await;
    let log = USB_LOG.wait().await;
    let _ = cdc.write_packet(b"=== Display+Touch Test Log ===\n").await;
    let _ = cdc
        .write_packet(b"Board: STM32F469I-DISCO (480x800, 180MHz)\n\n")
        .await;
    for entry in log.iter() {
        let _ = cdc.write_packet(entry.as_bytes()).await;
        Timer::after(Duration::from_millis(5)).await;
    }

    loop {
        led.set_high();
        Timer::after(Duration::from_millis(500)).await;
        led.set_low();
        Timer::after(Duration::from_millis(500)).await;
    }
}

// ── Drawing helpers ──

fn draw_border_1px(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, w: i32) {
    let h = H;
    for x in 0..w {
        Pixel(Point::new(x, 0), Rgb888::WHITE).draw(fb).ok();
        Pixel(Point::new(x, h - 1), Rgb888::WHITE).draw(fb).ok();
    }
    for y in 0..h {
        Pixel(Point::new(0, y), Rgb888::WHITE).draw(fb).ok();
        Pixel(Point::new(w - 1, y), Rgb888::WHITE).draw(fb).ok();
    }
}

fn draw_border_2px(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, w: i32) {
    let h = H;
    Rectangle::new(Point::new(0, 0), Size::new(w as u32, 2))
        .into_styled(PrimitiveStyle::with_fill(Rgb888::CSS_CYAN))
        .draw(fb)
        .ok();
    Rectangle::new(Point::new(0, h - 2), Size::new(w as u32, 2))
        .into_styled(PrimitiveStyle::with_fill(Rgb888::CSS_CYAN))
        .draw(fb)
        .ok();
    Rectangle::new(Point::new(0, 0), Size::new(2, h as u32))
        .into_styled(PrimitiveStyle::with_fill(Rgb888::CSS_CYAN))
        .draw(fb)
        .ok();
    Rectangle::new(Point::new(w - 2, 0), Size::new(2, h as u32))
        .into_styled(PrimitiveStyle::with_fill(Rgb888::CSS_CYAN))
        .draw(fb)
        .ok();
}

fn draw_border_3px(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, w: i32) {
    let h = H;
    Rectangle::new(Point::new(0, 0), Size::new(w as u32, 3))
        .into_styled(PrimitiveStyle::with_fill(Rgb888::CSS_YELLOW))
        .draw(fb)
        .ok();
    Rectangle::new(Point::new(0, h - 3), Size::new(w as u32, 3))
        .into_styled(PrimitiveStyle::with_fill(Rgb888::CSS_YELLOW))
        .draw(fb)
        .ok();
    Rectangle::new(Point::new(0, 0), Size::new(3, h as u32))
        .into_styled(PrimitiveStyle::with_fill(Rgb888::CSS_YELLOW))
        .draw(fb)
        .ok();
    Rectangle::new(Point::new(w - 3, 0), Size::new(3, h as u32))
        .into_styled(PrimitiveStyle::with_fill(Rgb888::CSS_YELLOW))
        .draw(fb)
        .ok();
}

fn draw_ruler(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>, w: i32) {
    let h = H;
    for x in (0..w).step_by(10) {
        let (len, color) = if x % 100 == 0 {
            (20, Rgb888::WHITE)
        } else if x % 50 == 0 {
            (12, Rgb888::CSS_LIGHT_GRAY)
        } else {
            (6, Rgb888::CSS_DIM_GRAY)
        };
        Line::new(Point::new(x, 0), Point::new(x, len))
            .into_styled(PrimitiveStyle::with_stroke(color, 1))
            .draw(fb)
            .ok();
        Line::new(Point::new(x, h - 1), Point::new(x, h - 1 - len))
            .into_styled(PrimitiveStyle::with_stroke(color, 1))
            .draw(fb)
            .ok();
    }
    for y in (0..h).step_by(10) {
        let (len, color) = if y % 100 == 0 {
            (20, Rgb888::WHITE)
        } else if y % 50 == 0 {
            (12, Rgb888::CSS_LIGHT_GRAY)
        } else {
            (6, Rgb888::CSS_DIM_GRAY)
        };
        Line::new(Point::new(0, y), Point::new(len, y))
            .into_styled(PrimitiveStyle::with_stroke(color, 1))
            .draw(fb)
            .ok();
        Line::new(Point::new(w - 1, y), Point::new(w - 1 - len, y))
            .into_styled(PrimitiveStyle::with_stroke(color, 1))
            .draw(fb)
            .ok();
    }
    let s = MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_GRAY);
    for &v in &[0, 100, 200, 300, 400] {
        Text::with_baseline(
            &alloc::format!("{}", v),
            Point::new(v.max(10) - 10, 24),
            s,
            Baseline::Top,
        )
        .draw(fb)
        .ok();
    }
    for &v in &[0, 100, 200, 300, 400, 500, 600, 700] {
        Text::with_baseline(
            &alloc::format!("{}", v),
            Point::new(24, v.min(h - 30)),
            s,
            Baseline::Top,
        )
        .draw(fb)
        .ok();
    }
}

fn draw_corner_refs(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>) {
    let s = MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_DIM_GRAY);
    Text::with_baseline("(0,0)", Point::new(5, 5), s, Baseline::Top)
        .draw(fb)
        .ok();
    Text::with_baseline("(479,0)", Point::new(W - 80, 5), s, Baseline::Top)
        .draw(fb)
        .ok();
    Text::with_baseline("(0,799)", Point::new(5, H - 20), s, Baseline::Top)
        .draw(fb)
        .ok();
    Text::with_baseline("(479,799)", Point::new(W - 100, H - 20), s, Baseline::Top)
        .draw(fb)
        .ok();
}

fn draw_yes_no(fb: &mut embassy_stm32f469i_disco::FramebufferView<'_>) {
    let yes_rect = YES_RECT;
    let no_rect = NO_RECT;

    Rectangle::new(yes_rect.top_left, yes_rect.size)
        .into_styled(PrimitiveStyle::with_fill(Rgb888::CSS_GREEN))
        .draw(fb)
        .ok();
    Text::with_baseline(
        "YES",
        Point::new(yes_rect.top_left.x + 60, yes_rect.top_left.y + 28),
        MonoTextStyle::new(&FONT_10X20, Rgb888::BLACK),
        Baseline::Top,
    )
    .draw(fb)
    .ok();

    Rectangle::new(no_rect.top_left, no_rect.size)
        .into_styled(PrimitiveStyle::with_fill(Rgb888::CSS_RED))
        .draw(fb)
        .ok();
    Text::with_baseline(
        "NO",
        Point::new(no_rect.top_left.x + 70, no_rect.top_left.y + 28),
        MonoTextStyle::new(&FONT_10X20, Rgb888::WHITE),
        Baseline::Top,
    )
    .draw(fb)
    .ok();
}

async fn wait_for_yes_no(
    touch: &mut TouchCtrl,
) -> bool {
    loop {
        if let Some((tx, ty)) = read_touch_raw(touch) {
            let p = Point::new(tx, ty);
            if YES_RECT.contains(p) {
                return true;
            }
            if NO_RECT.contains(p) {
                return false;
            }
        }
        Timer::after(Duration::from_millis(50)).await;
    }
}

// ── Touch helpers ──

fn read_touch_raw(
    touch: &mut TouchCtrl,
) -> Option<(i32, i32)> {
    match touch.td_status() {
        Ok(s) if s > 0 => match touch.get_touch() {
            Ok(p) => Some((p.x as i32, p.y as i32)),
            _ => None,
        },
        _ => None,
    }
}

async fn read_touch(
    touch: &mut TouchCtrl,
    last: &mut Option<(i32, i32)>,
) -> Option<(i32, i32)> {
    let (tx, ty) = read_touch_raw(touch)?;
    if tx < MARGIN as i32 || tx > 476 || ty < MARGIN as i32 || ty > 796 {
        return None;
    }
    let is_new = match *last {
        Some((lx, ly)) => (tx - lx).unsigned_abs() > 8 || (ty - ly).unsigned_abs() > 8,
        None => true,
    };
    if is_new {
        *last = Some((tx, ty));
        return Some((tx, ty));
    }
    None
}

fn tap_spread(taps: &[(i32, i32)]) -> i32 {
    if taps.is_empty() {
        return 0;
    }
    let min_x = taps.iter().map(|&(x, _)| x).min().unwrap();
    let max_x = taps.iter().map(|&(x, _)| x).max().unwrap();
    let min_y = taps.iter().map(|&(_, y)| y).min().unwrap();
    let max_y = taps.iter().map(|&(_, y)| y).max().unwrap();
    isqrt((max_x - min_x) * (max_x - min_x) + (max_y - min_y) * (max_y - min_y))
}

fn pseudo_random(seed: i32) -> i32 {
    let mut x = seed.wrapping_abs();
    x = x.wrapping_mul(1103515245).wrapping_add(12345);
    x = x.wrapping_mul(1103515245).wrapping_add(12345);
    x.unsigned_abs() as i32
}

fn isqrt(n: i32) -> i32 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

fn usb_phy_reset() {
    let rcc = stm32_metapac::RCC;
    rcc.ahb2enr().modify(|w| w.set_usb_otg_fsen(false));
    cortex_m::asm::delay(100);
    rcc.ahb2enr().modify(|w| w.set_usb_otg_fsen(true));
    rcc.ahb2rstr().modify(|w| w.set_usb_otg_fsrst(true));
    cortex_m::asm::delay(100);
    rcc.ahb2rstr().modify(|w| w.set_usb_otg_fsrst(false));
    cortex_m::asm::delay(100);
    let otg = 0x5000_0000usize as *mut u32;
    unsafe {
        let mut t = 100_000u32;
        while otg.add(0x010 / 4).read_volatile() & (1 << 31) == 0 {
            t -= 1;
            if t == 0 {
                break;
            }
        }
        otg.add(0x010 / 4).write_volatile(1);
        t = 100_000u32;
        while otg.add(0x010 / 4).read_volatile() & 1 != 0 {
            t -= 1;
            if t == 0 {
                break;
            }
        }
        otg.add(0x038 / 4).write_volatile(0);
        cortex_m::asm::delay(100);
        otg.add(0x038 / 4).write_volatile(1 << 16);
    }
}

async fn blink(led: &mut Output<'_>, count: usize) {
    for _ in 0..count {
        led.set_high();
        Timer::after(Duration::from_millis(80)).await;
        led.set_low();
        Timer::after(Duration::from_millis(80)).await;
    }
}
