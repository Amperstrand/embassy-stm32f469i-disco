#![no_std]
#![no_main]

use core::slice;

use defmt_rtt as _;
use embassy_stm32f469i_disco::{config_180, Board, BoardHint, SDRAM_SIZE_BYTES};
use stm32_metapac::LTDC;

const SDRAM_BASE: usize = 0xC000_0000;
const SDRAM_SCRATCH_OFFSET: usize = 2 * 1024 * 1024;

fn sdram_scratch_words(len: usize) -> &'static mut [u32] {
    unsafe { slice::from_raw_parts_mut((SDRAM_BASE + SDRAM_SCRATCH_OFFSET) as *mut u32, len) }
}

fn sdram_scratch_bytes(offset: usize, len: usize) -> &'static mut [u8] {
    unsafe {
        slice::from_raw_parts_mut((SDRAM_BASE + SDRAM_SCRATCH_OFFSET + offset) as *mut u8, len)
    }
}

fn rng_read() -> u32 {
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
            return rng.dr().read();
        }
        timeout -= 1;
        if timeout == 0 {
            panic!("RNG timeout");
        }
    }
}

fn rng_read_buf(buf: &mut [u32]) {
    for slot in buf.iter_mut() {
        *slot = rng_read();
    }
}

fn dma_memcpy(src: *const u8, dst: *mut u8, len: usize) {
    use stm32_metapac::dma::vals;

    assert!(len > 0 && len <= u16::MAX as usize);

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
            panic!("DMA error");
        }
        timeout -= 1;
        if timeout == 0 {
            panic!("DMA timeout");
        }
    }

    dma2.ifcr(0).write(|w| {
        w.set_tcif(0, true);
        w.set_htif(0, true);
        w.set_feif(0, true);
        w.set_dmeif(0, true);
        w.set_teif(0, true);
    });
}

#[cfg(test)]
#[embedded_test::tests]
mod tests {
    use super::*;
    use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
    use embassy_stm32::usart::Uart;
    use embassy_time::{Duration, Instant, Timer};
    use embedded_graphics::{
        pixelcolor::Rgb888,
        prelude::*,
        primitives::{PrimitiveStyle, Rectangle},
    };
    use embedded_hal_02::blocking::serial::Write as _;

    // ── SDRAM ──────────────────────────────────────────────────────────

    #[test]
    #[timeout(30)]
    async fn sdram_write_read_pattern() {
        let p = embassy_stm32::init(config_180());
        let sdram = embassy_stm32f469i_disco::sdram_init!(p);
        let ram = unsafe { slice::from_raw_parts_mut(sdram.base_address() as *mut u32, 1024) };

        for (i, word) in ram.iter_mut().enumerate() {
            *word = 0xA5A5_0000 | (i as u32);
        }
        for (i, word) in ram.iter().enumerate() {
            assert_eq!(*word, 0xA5A5_0000 | (i as u32));
        }
    }

    #[test]
    #[timeout(30)]
    async fn sdram_checkerboard() {
        let p = embassy_stm32::init(config_180());
        let _sdram = embassy_stm32f469i_disco::sdram_init!(p);
        let ram = sdram_scratch_words(65_536);
        for word in &mut ram[..] {
            *word = 0xAAAA_AAAA;
        }
        for &word in ram.iter() {
            assert_eq!(word, 0xAAAA_AAAA);
        }
    }

    #[test]
    #[timeout(30)]
    async fn sdram_march_c() {
        let p = embassy_stm32::init(config_180());
        let _sdram = embassy_stm32f469i_disco::sdram_init!(p);
        let ram = sdram_scratch_words(65_536);

        for word in &mut ram[..] {
            *word = 0;
        }
        for word in &mut ram[..] {
            assert_eq!(*word, 0);
            *word = 0xFFFF_FFFF;
        }
        for word in ram[..].iter_mut().rev() {
            assert_eq!(*word, 0xFFFF_FFFF);
            *word = 0;
        }
        for &word in ram.iter() {
            assert_eq!(word, 0);
        }
    }

    #[test]
    #[timeout(30)]
    async fn sdram_end_of_ram() {
        let p = embassy_stm32::init(config_180());
        let _sdram = embassy_stm32f469i_disco::sdram_init!(p);

        let tail_words = 16_384usize;
        let start = SDRAM_BASE + SDRAM_SIZE_BYTES - tail_words * 4;
        let ram = unsafe { slice::from_raw_parts_mut(start as *mut u32, tail_words) };

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
            assert_eq!(word, seed);
        }
    }

    #[test]
    #[timeout(30)]
    async fn sdram_byte_halfword() {
        let p = embassy_stm32::init(config_180());
        let _sdram = embassy_stm32f469i_disco::sdram_init!(p);

        let bytes = sdram_scratch_bytes(0, 4096);
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = (i & 0xFF) as u8;
        }
        for (i, &byte) in bytes.iter().enumerate() {
            assert_eq!(byte, (i & 0xFF) as u8);
        }

        let halfwords = unsafe { slice::from_raw_parts_mut(bytes.as_mut_ptr() as *mut u16, 2048) };
        for (i, hw) in halfwords.iter_mut().enumerate() {
            *hw = (i as u16).wrapping_add(1);
        }
        for (i, &hw) in halfwords.iter().enumerate() {
            assert_eq!(hw, (i as u16).wrapping_add(1));
        }
    }

    // ── Display ────────────────────────────────────────────────────────

    #[test]
    #[timeout(30)]
    async fn display_init() {
        let p = embassy_stm32::init(config_180());
        let _board = Board::try_new(p, BoardHint::ForceNt35510).expect("board init");

        assert!(LTDC.gcr().read().ltdcen());
        assert!(LTDC.layer(0).cr().read().len());
    }

    #[test]
    #[timeout(30)]
    async fn display_color_fill() {
        let p = embassy_stm32::init(config_180());
        let mut board = Board::try_new(p, BoardHint::ForceNt35510).expect("board init");

        {
            let mut fb = board.display.fb();
            fb.clear(Rgb888::GREEN);
        }
        Timer::after(Duration::from_millis(50)).await;

        let mut fb = board.display.fb();
        Rectangle::new(Point::new(0, 0), Size::new(100, 100))
            .into_styled(PrimitiveStyle::with_fill(Rgb888::RED))
            .draw(&mut fb)
            .ok();
    }

    // ── Touch ──────────────────────────────────────────────────────────

    #[test]
    #[timeout(30)]
    async fn touch_vendor_id() {
        let p = embassy_stm32::init(config_180());
        let mut board = Board::try_new(p, BoardHint::ForceNt35510).expect("board init");

        assert_eq!(board.touch.read_vendor_id().unwrap(), 0x11);
    }

    #[test]
    #[timeout(30)]
    async fn touch_chip_model() {
        let p = embassy_stm32::init(config_180());
        let mut board = Board::try_new(p, BoardHint::ForceNt35510).expect("board init");

        let model = board.touch.read_chip_model().unwrap();
        assert!(matches!(model, 0x06 | 0x36 | 0x64));
    }

    // ── LED ────────────────────────────────────────────────────────────

    #[test]
    #[timeout(30)]
    async fn led_toggle() {
        let p = embassy_stm32::init(config_180());

        let mut green = Output::new(p.PG6, Level::High, Speed::Low);
        let mut orange = Output::new(p.PD4, Level::High, Speed::Low);
        let mut red = Output::new(p.PD5, Level::High, Speed::Low);
        let mut blue = Output::new(p.PK3, Level::High, Speed::Low);

        for _ in 0..3 {
            green.toggle();
            orange.toggle();
            red.toggle();
            blue.toggle();
            Timer::after(Duration::from_millis(50)).await;
        }

        green.set_high();
        orange.set_high();
        red.set_high();
        blue.set_high();
    }

    // ── GPIO ───────────────────────────────────────────────────────────

    #[test]
    #[timeout(30)]
    async fn gpio_pa0_input() {
        let p = embassy_stm32::init(config_180());
        let _button = Input::new(p.PA0, Pull::Down);
    }

    #[test]
    #[timeout(30)]
    async fn gpio_multi_port_output() {
        let p = embassy_stm32::init(config_180());
        let mut pa = Output::new(p.PA0, Level::High, Speed::Low);
        let mut pg = Output::new(p.PG6, Level::High, Speed::Low);
        let mut pd = Output::new(p.PD4, Level::High, Speed::Low);

        pa.set_low();
        pg.set_low();
        pd.set_low();
        Timer::after(Duration::from_millis(10)).await;
        pa.set_high();
        pg.set_high();
        pd.set_high();
    }

    // ── Timer ──────────────────────────────────────────────────────────

    #[test]
    #[timeout(30)]
    async fn timer_1ms() {
        embassy_stm32::init(config_180());
        Timer::after(Duration::from_millis(1)).await;
    }

    #[test]
    #[timeout(30)]
    async fn timer_100ms_accuracy() {
        embassy_stm32::init(config_180());
        let start = Instant::now();
        Timer::after(Duration::from_millis(100)).await;
        let elapsed = start.elapsed().as_millis();
        assert!((95..=120).contains(&elapsed), "elapsed={}", elapsed);
    }

    #[test]
    #[timeout(30)]
    async fn timer_ticker() {
        embassy_stm32::init(config_180());
        let mut ticker = embassy_time::Ticker::every(Duration::from_millis(100));
        let start = Instant::now();
        for _ in 0..5 {
            ticker.next().await;
        }
        let elapsed = start.elapsed().as_millis();
        assert!((450..=600).contains(&elapsed), "elapsed={}", elapsed);
    }

    // ── RNG ────────────────────────────────────────────────────────────

    #[test]
    #[timeout(30)]
    async fn rng_not_zero() {
        embassy_stm32::init(config_180());
        stm32_metapac::RCC.ahb2enr().modify(|w| w.set_rngen(true));
        let rng = stm32_metapac::RNG;
        rng.cr().modify(|w| w.set_rngen(false));
        rng.sr().modify(|w| {
            w.set_seis(false);
            w.set_ceis(false);
        });
        rng.cr().modify(|w| w.set_rngen(true));

        let val = rng_read();
        assert_ne!(val, 0);
    }

    #[test]
    #[timeout(30)]
    async fn rng_uniqueness() {
        embassy_stm32::init(config_180());
        stm32_metapac::RCC.ahb2enr().modify(|w| w.set_rngen(true));
        let rng = stm32_metapac::RNG;
        rng.cr().modify(|w| w.set_rngen(false));
        rng.sr().modify(|w| {
            w.set_seis(false);
            w.set_ceis(false);
        });
        rng.cr().modify(|w| w.set_rngen(true));

        let mut buf = [0u32; 64];
        rng_read_buf(&mut buf);

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
        assert!(unique >= 32, "only {} unique values", unique);
    }

    #[test]
    #[timeout(30)]
    async fn rng_consecutive_differ() {
        embassy_stm32::init(config_180());
        stm32_metapac::RCC.ahb2enr().modify(|w| w.set_rngen(true));
        let rng = stm32_metapac::RNG;
        rng.cr().modify(|w| w.set_rngen(false));
        rng.sr().modify(|w| {
            w.set_seis(false);
            w.set_ceis(false);
        });
        rng.cr().modify(|w| w.set_rngen(true));

        let first = rng_read();
        let second = rng_read();
        assert_ne!(first, second);
    }

    // ── ADC ────────────────────────────────────────────────────────────

    #[test]
    #[timeout(30)]
    async fn adc_temp_sensor() {
        embassy_stm32::init(config_180());
        stm32_metapac::RCC.apb2enr().modify(|w| w.set_adc1en(true));
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
        adc.smpr1()
            .write(|w| w.set_smp(8, stm32_metapac::adc::vals::SampleTime::CYCLES480));
        adc.cr2().modify(|w| w.set_adon(true));
        cortex_m::asm::delay(3);
        adc.cr2().modify(|w| w.set_swstart(true));
        while !adc.sr().read().eoc() {}
        let sample = adc.dr().read().0 as u16;
        assert!(sample > 100 && sample < 4095, "temp_raw={}", sample);
    }

    #[test]
    #[timeout(30)]
    async fn adc_vrefint() {
        embassy_stm32::init(config_180());
        stm32_metapac::RCC.apb2enr().modify(|w| w.set_adc1en(true));
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
        adc.sqr3().write(|w| w.set_sq(0, 17));
        adc.smpr1()
            .write(|w| w.set_smp(7, stm32_metapac::adc::vals::SampleTime::CYCLES480));
        adc.cr2().modify(|w| w.set_adon(true));
        cortex_m::asm::delay(3);
        adc.cr2().modify(|w| w.set_swstart(true));
        while !adc.sr().read().eoc() {}
        let sample = adc.dr().read().0 as u16;
        assert!(sample > 500 && sample < 3000, "vref_raw={}", sample);
    }

    // ── UART ───────────────────────────────────────────────────────────

    #[test]
    #[timeout(30)]
    async fn uart_init() {
        let p = embassy_stm32::init(config_180());
        let uart = Uart::new_blocking(
            p.USART1,
            p.PA10,
            p.PA9,
            embassy_stm32::usart::Config::default(),
        );
        assert!(uart.is_ok());
    }

    #[test]
    #[timeout(30)]
    async fn uart_tx_byte() {
        let p = embassy_stm32::init(config_180());
        let mut tx = Uart::new_blocking(
            p.USART1,
            p.PA10,
            p.PA9,
            embassy_stm32::usart::Config::default(),
        )
        .expect("uart init");
        tx.bwrite_all(b"U").expect("uart write");
    }

    #[test]
    #[timeout(30)]
    async fn uart_tx_multi_byte() {
        let p = embassy_stm32::init(config_180());
        let mut tx = Uart::new_blocking(
            p.USART1,
            p.PA10,
            p.PA9,
            embassy_stm32::usart::Config::default(),
        )
        .expect("uart init");
        tx.bwrite_all(b"HELLO").expect("uart write");
    }

    // ── DMA ────────────────────────────────────────────────────────────

    #[test]
    #[timeout(30)]
    async fn dma_64b() {
        let p = embassy_stm32::init(config_180());
        let _sdram = embassy_stm32f469i_disco::sdram_init!(p);
        stm32_metapac::RCC.ahb1enr().modify(|w| w.set_dma2en(true));

        let src = sdram_scratch_bytes(0, 64);
        let dst = sdram_scratch_bytes(0x2000, 64);
        for (i, byte) in src.iter_mut().enumerate() {
            *byte = ((i * 37) & 0xFF) as u8;
        }
        for byte in dst.iter_mut() {
            *byte = 0;
        }
        dma_memcpy(src.as_ptr(), dst.as_mut_ptr(), 64);

        for (a, b) in src.iter().zip(dst.iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    #[timeout(30)]
    async fn dma_4096b() {
        let p = embassy_stm32::init(config_180());
        let _sdram = embassy_stm32f469i_disco::sdram_init!(p);
        stm32_metapac::RCC.ahb1enr().modify(|w| w.set_dma2en(true));

        let src = sdram_scratch_bytes(0, 4096);
        let dst = sdram_scratch_bytes(0x2000, 4096);
        for (i, byte) in src.iter_mut().enumerate() {
            *byte = ((i * 37) & 0xFF) as u8;
        }
        for byte in dst.iter_mut() {
            *byte = 0;
        }
        dma_memcpy(src.as_ptr(), dst.as_mut_ptr(), 4096);

        for (a, b) in src.iter().zip(dst.iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    #[timeout(30)]
    async fn dma_repeated() {
        let p = embassy_stm32::init(config_180());
        let _sdram = embassy_stm32f469i_disco::sdram_init!(p);
        stm32_metapac::RCC.ahb1enr().modify(|w| w.set_dma2en(true));

        for i in 0..10 {
            let off = i * 0x4000;
            let src = sdram_scratch_bytes(off, 256);
            let dst = sdram_scratch_bytes(off + 0x2000, 256);
            for (j, byte) in src.iter_mut().enumerate() {
                *byte = ((j * 37 + i) & 0xFF) as u8;
            }
            for byte in dst.iter_mut() {
                *byte = 0;
            }
            dma_memcpy(src.as_ptr(), dst.as_mut_ptr(), 256);
            for (a, b) in src.iter().zip(dst.iter()) {
                assert_eq!(a, b);
            }
        }
    }
}
