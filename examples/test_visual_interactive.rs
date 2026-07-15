//! Interactive display + touch verification test.
//!
//! Renders test patterns on screen, asks the tester to confirm via touch
//! (YES/NO buttons), then runs touch accuracy tests. Results output via
//! RTT (defmt) in standard TEST/SUMMARY format for `run_hil.sh`.
//!
//! Test patterns adapted from Zephyr display harness (color rectangles),
//! IGT GPU Tools (checkerboard edges), and SMPTE color bars.
//!
//! Build:
//!   cargo build --release --target thumbv7em-none-eabihf --example test_visual_interactive
//!
//! Run:
//!   probe-rs run --chip STM32F469NIHx --example test_visual_interactive

#![no_std]
#![no_main]

use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_stm32f469i_disco::{
    config_180, Board, BoardHint, FB_HEIGHT, FB_WIDTH,
};
use embassy_time::{Duration, Timer};
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{Line, PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
    Pixel,
};
use {defmt_rtt as _, panic_probe as _};

const W: i32 = FB_WIDTH as i32;
const H: i32 = FB_HEIGHT as i32;

const YES_RECT: Rectangle = Rectangle::new(Point::new(40, H - 120), Size::new(180, 80));
const NO_RECT: Rectangle = Rectangle::new(Point::new(260, H - 120), Size::new(180, 80));

const MAX_TESTS: usize = 16;
const TOUCH_TIMEOUT_SECS: u64 = 15;
const ACCURACY_ROUNDS: usize = 3;
const ACCURACY_THRESHOLD: i32 = 40;
const CORNER_INSET: i32 = 40;

#[derive(Clone, Copy)]
struct TestResult {
    name: &'static str,
    passed: bool,
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(config_180());
    let mut board = Board::try_new(p, BoardHint::ForceNt35510).expect("board init");

    let mut results: [Option<TestResult>; MAX_TESTS] = [None; MAX_TESTS];
    let mut result_count: usize = 0;

    let s_white = MonoTextStyle::new(&FONT_10X20, Rgb888::WHITE);
    let s_black = MonoTextStyle::new(&FONT_10X20, Rgb888::BLACK);
    let s_yellow = MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_YELLOW);

    info!("test_visual_interactive: starting");

    // ── Phase 1: Display verification (YES/NO) ──────────────────────

    info!("Phase 1: Display verification");

    // Test 1: Solid RED
    {
        let mut fb = board.display.fb();
        fb.clear(Rgb888::RED);
        draw_question(&mut fb, "1/5: Solid RED", "Is the screen completely red?", s_white);
        draw_yes_no(&mut fb, s_black, s_white);
        let pass = wait_yes_no(&mut board).await;
        record(&mut results, &mut result_count, "display_red", pass);
    }

    // Test 2: Solid GREEN
    {
        let mut fb = board.display.fb();
        fb.clear(Rgb888::GREEN);
        draw_question(&mut fb, "2/5: Solid GREEN", "Is the screen completely green?", s_black);
        draw_yes_no(&mut fb, s_black, s_white);
        let pass = wait_yes_no(&mut board).await;
        record(&mut results, &mut result_count, "display_green", pass);
    }

    // Test 3: Solid BLUE
    {
        let mut fb = board.display.fb();
        fb.clear(Rgb888::BLUE);
        draw_question(&mut fb, "3/5: Solid BLUE", "Is the screen completely blue?", s_white);
        draw_yes_no(&mut fb, s_black, s_white);
        let pass = wait_yes_no(&mut board).await;
        record(&mut results, &mut result_count, "display_blue", pass);
    }

    // Test 4: Greyscale gradient
    {
        let mut fb = board.display.fb();
        fb.clear(Rgb888::BLACK);
        for y in 0..H {
            let v = (y as u32 * 255 / H as u32) as u8;
            let color = Rgb888::new(v, v, v);
            Line::new(Point::new(0, y), Point::new(W - 1, y))
                .into_styled(PrimitiveStyle::with_stroke(color, 1))
                .draw(&mut fb)
                .ok();
        }
        draw_question(&mut fb, "4/5: Gradient", "Smooth gradient black to white?", s_yellow);
        draw_yes_no(&mut fb, s_black, s_white);
        let pass = wait_yes_no(&mut board).await;
        record(&mut results, &mut result_count, "display_gradient", pass);
    }

    // Test 5: Text + borders
    {
        let mut fb = board.display.fb();
        fb.clear(Rgb888::BLACK);

        Rectangle::new(Point::new(0, 0), Size::new(W as u32, H as u32))
            .into_styled(PrimitiveStyle::with_stroke(Rgb888::WHITE, 1))
            .draw(&mut fb)
            .ok();
        Rectangle::new(Point::new(2, 2), Size::new(W as u32 - 4, H as u32 - 4))
            .into_styled(PrimitiveStyle::with_stroke(Rgb888::CSS_CYAN, 1))
            .draw(&mut fb)
            .ok();

        let s_gr = MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_GREEN);
        Text::with_baseline("STM32F469I-DISCO", Point::new(100, 100), s_gr, Baseline::Top)
            .draw(&mut fb)
            .ok();
        Text::with_baseline("480 x 800 RGB888", Point::new(110, 130), s_white, Baseline::Top)
            .draw(&mut fb)
            .ok();
        Text::with_baseline("NT35510 via DSI/LTDC", Point::new(90, 160), s_white, Baseline::Top)
            .draw(&mut fb)
            .ok();

        for (i, label) in ["TOP", "BOTTOM", "LEFT", "RIGHT"].iter().enumerate() {
            let pos = match i {
                0 => Point::new(200, 20),
                1 => Point::new(190, H - 40),
                2 => Point::new(20, H / 2),
                _ => Point::new(W - 80, H / 2),
            };
            Text::with_baseline(label, pos, s_yellow, Baseline::Top)
                .draw(&mut fb)
                .ok();
        }

        draw_question(&mut fb, "5/5: Text+Borders", "Readable text + straight borders?", s_yellow);
        draw_yes_no(&mut fb, s_black, s_white);
        let pass = wait_yes_no(&mut board).await;
        record(&mut results, &mut result_count, "display_text_borders", pass);
    }

    // ── Phase 2: Touch verification (automated) ─────────────────────

    info!("Phase 2: Touch verification");

    // Test 6: Touch detect
    {
        let mut fb = board.display.fb();
        fb.clear(Rgb888::BLACK);
        Text::with_baseline("Tap anywhere to continue", Point::new(40, H / 2 - 10), s_white, Baseline::Top)
            .draw(&mut fb)
            .ok();
        info!("TEST touch_detect: RUNNING");

        let detected = wait_any_touch(&mut board, TOUCH_TIMEOUT_SECS).await.is_some();
        record(&mut results, &mut result_count, "touch_detect", detected);
    }

    // Test 7: Touch accuracy (tap the dot)
    {
        let mut total_dist: i32 = 0;
        let mut hits = 0;

        for round in 0..ACCURACY_ROUNDS {
            let tx = pseudo_random((round * 7 + 3) as i32) % (W - 120) + 60;
            let ty = pseudo_random((round * 11 + 5) as i32) % (H - 250) + 60;

            let mut fb = board.display.fb();
            fb.clear(Rgb888::BLACK);

            let label = if ACCURACY_ROUNDS > 1 {
                let buf = fmt_round(round + 1, ACCURACY_ROUNDS);
                Text::with_baseline(&buf, Point::new(10, 10), s_white, Baseline::Top)
                    .draw(&mut fb)
                    .ok();
                "Tap the red dot"
            } else {
                "Tap the red dot"
            };
            Text::with_baseline(label, Point::new(140, 10), s_white, Baseline::Top)
                .draw(&mut fb)
                .ok();

            for dx in -8..=8 {
                for dy in -8..=8 {
                    Pixel(Point::new(tx + dx, ty + dy), Rgb888::CSS_RED)
                        .draw(&mut fb)
                        .ok();
                }
            }

            info!("TEST touch_accuracy: RUNNING round {}", round + 1);

            if let Some((ux, uy)) = wait_any_touch(&mut board, TOUCH_TIMEOUT_SECS).await {
                let dx = ux - tx;
                let dy = uy - ty;
                let dist = isqrt(dx * dx + dy * dy);
                total_dist += dist;
                hits += 1;

                let mut fb2 = board.display.fb();
                for ddx in -4..=4 {
                    for ddy in -4..=4 {
                        Pixel(Point::new(ux + ddx, uy + ddy), Rgb888::CSS_GREEN)
                            .draw(&mut fb2)
                            .ok();
                    }
                }
                Timer::after(Duration::from_millis(300)).await;
            }
        }

        let pass = hits == ACCURACY_ROUNDS && (total_dist / hits as i32) < ACCURACY_THRESHOLD;
        record(&mut results, &mut result_count, "touch_accuracy", pass);
    }

    // Test 8: Touch corners
    {
        let corners: &[(&str, i32, i32)] = &[
            ("TOP-LEFT", CORNER_INSET, CORNER_INSET),
            ("TOP-RIGHT", W - CORNER_INSET, CORNER_INSET),
            ("BOTTOM-RIGHT", W - CORNER_INSET, H - CORNER_INSET),
            ("BOTTOM-LEFT", CORNER_INSET, H - CORNER_INSET),
        ];

        let mut corner_hits = 0;

        for (idx, (label, cx, cy)) in corners.iter().enumerate() {
            let mut fb = board.display.fb();
            fb.clear(Rgb888::BLACK);

            let buf = fmt_corner(idx + 1, corners.len(), label);
            Text::with_baseline(&buf, Point::new(10, 10), s_white, Baseline::Top)
                .draw(&mut fb)
                .ok();

            for dx in -10..=10 {
                for dy in -10..=10 {
                    Pixel(Point::new(*cx + dx, *cy + dy), Rgb888::CSS_YELLOW)
                        .draw(&mut fb)
                        .ok();
                }
            }

            info!("TEST touch_corners: RUNNING {} {}", idx + 1, label);

            if let Some((ux, uy)) = wait_any_touch(&mut board, TOUCH_TIMEOUT_SECS).await {
                let dx = ux - *cx;
                let dy = uy - *cy;
                let dist = isqrt(dx * dx + dy * dy);
                if dist < 60 {
                    corner_hits += 1;
                }
                let mut fb2 = board.display.fb();
                for ddx in -4..=4 {
                    for ddy in -4..=4 {
                        Pixel(Point::new(ux + ddx, uy + ddy), Rgb888::CSS_GREEN)
                            .draw(&mut fb2)
                            .ok();
                    }
                }
                Timer::after(Duration::from_millis(300)).await;
            }
        }

        record(&mut results, &mut result_count, "touch_corners", corner_hits == 4);
    }

    // ── Summary ─────────────────────────────────────────────────────

    let passed = results[..result_count]
        .iter()
        .filter(|r| r.as_ref().map_or(false, |t| t.passed))
        .count();
    let total = result_count;

    {
        let mut fb = board.display.fb();
        fb.clear(Rgb888::BLACK);

        let header_s = if passed == total {
            MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_GREEN)
        } else {
            MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_RED)
        };

        let hdr = fmt_summary(passed, total);
        Text::with_baseline(&hdr, Point::new(140, 40), header_s, Baseline::Top)
            .draw(&mut fb)
            .ok();

        let mut y: i32 = 80;
        for slot in &results[..result_count] {
            if y > H - 30 {
                break;
            }
            if let Some(t) = slot {
                let st = if t.passed {
                    MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_GREEN)
                } else {
                    MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_RED)
                };
                let line = if t.passed {
                    fmt_pass(t.name)
                } else {
                    fmt_fail(t.name)
                };
                Text::with_baseline(&line, Point::new(10, y), st, Baseline::Top)
                    .draw(&mut fb)
                    .ok();
                y += 24;
            }
        }
    }

    info!("SUMMARY: {}/{} passed", passed, total);
    if passed == total {
        info!("ALL TESTS PASSED");
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn draw_question<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    title: &str,
    question: &str,
    style: MonoTextStyle<Rgb888>,
) {
    let s_title = MonoTextStyle::new(&FONT_10X20, Rgb888::CSS_YELLOW);
    Text::with_baseline(title, Point::new(80, 30), s_title, Baseline::Top)
        .draw(target)
        .ok();
    Text::with_baseline(question, Point::new(60, 60), style, Baseline::Top)
        .draw(target)
        .ok();
}

fn draw_yes_no<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    yes_text: MonoTextStyle<Rgb888>,
    no_text: MonoTextStyle<Rgb888>,
) {
    Rectangle::new(YES_RECT.top_left, YES_RECT.size)
        .into_styled(PrimitiveStyle::with_fill(Rgb888::CSS_GREEN))
        .draw(target)
        .ok();
    Text::with_baseline(
        "YES",
        Point::new(YES_RECT.top_left.x + 60, YES_RECT.top_left.y + 28),
        yes_text,
        Baseline::Top,
    )
    .draw(target)
    .ok();

    Rectangle::new(NO_RECT.top_left, NO_RECT.size)
        .into_styled(PrimitiveStyle::with_fill(Rgb888::CSS_RED))
        .draw(target)
        .ok();
    Text::with_baseline(
        "NO",
        Point::new(NO_RECT.top_left.x + 70, NO_RECT.top_left.y + 28),
        no_text,
        Baseline::Top,
    )
    .draw(target)
    .ok();
}

async fn wait_yes_no(board: &mut Board) -> bool {
    loop {
        if let Ok(Some(pt)) = board.touch.get_touch() {
            let p = Point::new(pt.x as i32, pt.y as i32);
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

async fn wait_any_touch(board: &mut Board, timeout_secs: u64) -> Option<(i32, i32)> {
    let deadline = embassy_time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        if embassy_time::Instant::now() >= deadline {
            return None;
        }
        if let Ok(Some(pt)) = board.touch.get_touch() {
            return Some((pt.x as i32, pt.y as i32));
        }
        Timer::after(Duration::from_millis(30)).await;
    }
}

fn record(results: &mut [Option<TestResult>; MAX_TESTS], count: &mut usize, name: &'static str, pass: bool) {
    if pass {
        info!("TEST {}: PASS", name);
    } else {
        error!("TEST {}: FAIL", name);
    }
    if *count < MAX_TESTS {
        results[*count] = Some(TestResult { name, passed: pass });
        *count += 1;
    }
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

fn fmt_round(current: usize, total: usize) -> heapless::String<32> {
    let mut s = heapless::String::new();
    s.push_str("[").ok();
    utoa(current, &mut s);
    s.push_str("/").ok();
    utoa(total, &mut s);
    s.push_str("] Tap the red dot").ok();
    s
}

fn fmt_corner(idx: usize, total: usize, label: &str) -> heapless::String<48> {
    let mut s = heapless::String::new();
    s.push_str("[").ok();
    utoa(idx, &mut s);
    s.push_str("/").ok();
    utoa(total, &mut s);
    s.push_str("] Tap ").ok();
    s.push_str(label).ok();
    s
}

fn fmt_summary(passed: usize, total: usize) -> heapless::String<32> {
    let mut s = heapless::String::new();
    utoa(passed, &mut s);
    s.push_str("/").ok();
    utoa(total, &mut s);
    s.push_str(" passed").ok();
    s
}

fn fmt_pass(name: &str) -> heapless::String<48> {
    let mut s = heapless::String::new();
    s.push_str(name).ok();
    s.push_str(": PASS").ok();
    s
}

fn fmt_fail(name: &str) -> heapless::String<48> {
    let mut s = heapless::String::new();
    s.push_str(name).ok();
    s.push_str(": FAIL").ok();
    s
}

fn utoa<const N: usize>(v: usize, s: &mut heapless::String<N>) {
    if v == 0 {
        s.push('0').ok();
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 0;
    let mut n = v;
    while n > 0 {
        buf[i] = (n % 10) as u8 + b'0';
        n /= 10;
        i += 1;
    }
    for c in buf[..i].iter().rev() {
        s.push(*c as char).ok();
    }
}
