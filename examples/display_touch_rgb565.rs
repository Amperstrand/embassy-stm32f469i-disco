#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32::i2c;
use embassy_stm32f469i_disco::{
    config_180, display::SdramCtrl, BoardHint, DisplayCtrl, DisplayCtrlCtor, Rgb565, TouchCtrl,
    SYSCLK_HZ_180,
};
use embassy_time::{Duration, Timer};
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::{Rgb565 as EgRgb565, WebColors},
    prelude::*,
    primitives::{Line, PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
    Pixel,
};
use {defmt_rtt as _, panic_probe as _};

const WIDTH: i32 = 480;
const HEIGHT: i32 = 800;
const BAND_HEIGHT: i32 = 200;
const TOUCH_MARGIN: u16 = 3;
const TOUCH_MAX_X: u16 = 476;
const TOUCH_MAX_Y: u16 = 796;
const IDLE_CLEAR_MS: u32 = 2_000;
const POLL_MS: u32 = 50;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("display_touch_rgb565: init...");

    let mut p = embassy_stm32::init(config_180());

    info!("display_touch_rgb565: init SDRAM...");
    let sdram = SdramCtrl::new(&mut p, SYSCLK_HZ_180);
    let framebuffer = sdram.into_bytes();

    info!("display_touch_rgb565: init display...");
    let mut display = DisplayCtrl::<Rgb565>::new(
        framebuffer,
        p.LTDC,
        p.DSIHOST,
        p.PJ2,
        p.PH7,
        BoardHint::ForceNt35510,
    );

    info!("display_touch_rgb565: init framebuffer...");
    let mut fb = display.fb();
    fb.clear(EgRgb565::BLACK);
    draw_background(&mut fb);
    draw_overlay(&mut fb);

    info!("display_touch_rgb565: init touch...");
    let i2c =
        embassy_stm32::i2c::I2c::new_blocking(p.I2C1, p.PB8, p.PB9, i2c::Config::default());
    let mut touch = TouchCtrl::new(i2c);

    let mut last_touch = None::<Point>;
    let mut idle_ms = IDLE_CLEAR_MS;

    loop {
        let mut touched = false;

        match touch.td_status() {
            Ok(status) if status > 0 => match touch.get_touch() {
                Ok(point) if valid_touch(point.x, point.y) => {
                    let point = Point::new(point.x as i32, point.y as i32);
                    info!("Touch: x={}, y={}", point.x, point.y);

                    if let Some(previous) = last_touch {
                        clear_crosshair(&mut fb, previous);
                        draw_overlay(&mut fb);
                    }

                    draw_crosshair(&mut fb, point, EgRgb565::CSS_YELLOW);
                    last_touch = Some(point);
                    idle_ms = 0;
                    touched = true;
                }
                _ => {}
            },
            _ => {}
        }

        if !touched {
            if let Some(previous) = last_touch {
                idle_ms = idle_ms.saturating_add(POLL_MS);
                if idle_ms >= IDLE_CLEAR_MS {
                    clear_crosshair(&mut fb, previous);
                    draw_overlay(&mut fb);
                    last_touch = None;
                }
            }
        }

        Timer::after(Duration::from_millis(POLL_MS as u64)).await;
    }
}

fn draw_background<D>(target: &mut D)
where
    D: DrawTarget<Color = EgRgb565>,
{
    for (band, color) in [
        EgRgb565::RED,
        EgRgb565::GREEN,
        EgRgb565::BLUE,
        EgRgb565::WHITE,
    ]
    .into_iter()
    .enumerate()
    {
        Rectangle::new(
            Point::new(0, band as i32 * BAND_HEIGHT),
            Size::new(WIDTH as u32, BAND_HEIGHT as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(target)
        .ok();
    }
}

fn draw_overlay<D>(target: &mut D)
where
    D: DrawTarget<Color = EgRgb565>,
{
    Rectangle::new(Point::new(0, 0), Size::new(WIDTH as u32, 72))
        .into_styled(PrimitiveStyle::with_fill(EgRgb565::BLACK))
        .draw(target)
        .ok();

    let text_style = MonoTextStyle::new(&FONT_10X20, EgRgb565::WHITE);

    Text::with_baseline(
        "RGB565 + Touch",
        Point::new(164, 8),
        text_style,
        Baseline::Top,
    )
    .draw(target)
    .ok();
    Text::with_baseline(
        "u16 / 2bpp / 480x800",
        Point::new(140, 36),
        text_style,
        Baseline::Top,
    )
    .draw(target)
    .ok();
}

fn draw_crosshair<D>(target: &mut D, point: Point, color: EgRgb565)
where
    D: DrawTarget<Color = EgRgb565>,
{
    let style = PrimitiveStyle::with_stroke(color, 1);
    Line::new(Point::new(0, point.y), Point::new(WIDTH - 1, point.y))
        .into_styled(style)
        .draw(target)
        .ok();
    Line::new(Point::new(point.x, 0), Point::new(point.x, HEIGHT - 1))
        .into_styled(style)
        .draw(target)
        .ok();
}

fn clear_crosshair<D>(target: &mut D, point: Point)
where
    D: DrawTarget<Color = EgRgb565>,
{
    Line::new(Point::new(0, point.y), Point::new(WIDTH - 1, point.y))
        .into_styled(PrimitiveStyle::with_stroke(band_color(point.y), 1))
        .draw(target)
        .ok();
    for y in 0..HEIGHT {
        Pixel(Point::new(point.x, y), band_color(y))
            .draw(target)
            .ok();
    }
}

fn band_color(y: i32) -> EgRgb565 {
    match y {
        ..200 => EgRgb565::RED,
        200..400 => EgRgb565::GREEN,
        400..600 => EgRgb565::BLUE,
        _ => EgRgb565::WHITE,
    }
}

fn valid_touch(x: u16, y: u16) -> bool {
    (TOUCH_MARGIN..=TOUCH_MAX_X).contains(&x) && (TOUCH_MARGIN..=TOUCH_MAX_Y).contains(&y)
}
