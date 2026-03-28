#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use core::sync::atomic::{AtomicUsize, Ordering};

use embassy_stm32::rcc::{
    AHBPrescaler, APBPrescaler, Hse, HseMode, Pll, PllMul, PllPDiv, PllPreDiv, PllQDiv, PllRDiv,
    PllSource, Sysclk,
};
use embassy_stm32f469i_disco::display::SdramCtrl;
use embassy_time::Timer;

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

fn fail(name: &str, addr: usize, expected: u32, got: u32) {
    FAILED.fetch_add(1, Ordering::Relaxed);
    defmt::error!(
        "TEST {}: FAIL addr={:#010X} expected={:#010X} got={:#010X}",
        name,
        addr,
        expected,
        got
    );
}

struct XorShift32 {
    seed: u32,
}

impl XorShift32 {
    fn new(seed: u32) -> Self {
        XorShift32 {
            seed: if seed == 0 { 1 } else { seed },
        }
    }

    fn next(&mut self) -> u32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed
    }
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

    let mut p = embassy_stm32::init(config);

    defmt::info!("=== SDRAM Fast Test Suite ===");
    defmt::info!("Initializing SDRAM...");

    let sdram = SdramCtrl::new(&mut p, 180_000_000);

    let base = sdram.base_address();
    let words = embassy_stm32f469i_disco::display::SDRAM_SIZE_BYTES / 4;
    defmt::info!("SDRAM: base={:#010X} words={}", base, words);

    let ram: &mut [u32] = unsafe { core::slice::from_raw_parts_mut(sdram.base_address() as *mut u32, words) };

    const WIN: usize = 65536;

    // Test 1: Checkerboard
    defmt::info!("TEST checkerboard: RUNNING");
    {
        let mut ok = true;
        for word in ram[..WIN].iter_mut() {
            *word = 0xAAAAAAAA;
        }
        for (i, word) in ram[..WIN].iter().enumerate() {
            if *word != 0xAAAAAAAA {
                fail("checkerboard", base + i * 4, 0xAAAAAAAA, *word);
                ok = false;
                break;
            }
        }
        if ok {
            pass("checkerboard");
        }
    }

    // Test 2: Inverse checkerboard
    defmt::info!("TEST inv_checkerboard: RUNNING");
    {
        let mut ok = true;
        for word in ram[..WIN].iter_mut() {
            *word = 0x55555555;
        }
        for (i, word) in ram[..WIN].iter().enumerate() {
            if *word != 0x55555555 {
                fail("inv_checkerboard", base + i * 4, 0x55555555, *word);
                ok = false;
                break;
            }
        }
        if ok {
            pass("inv_checkerboard");
        }
    }

    // Test 3: Address pattern
    defmt::info!("TEST addr_pattern: RUNNING");
    {
        let mut ok = true;
        for (i, word) in ram[..WIN].iter_mut().enumerate() {
            *word = (base + i * 4) as u32;
        }
        for (i, word) in ram[..WIN].iter().enumerate() {
            let expected = (base + i * 4) as u32;
            if *word != expected {
                fail("addr_pattern", base + i * 4, expected, *word);
                ok = false;
                break;
            }
        }
        if ok {
            pass("addr_pattern");
        }
    }

    // Test 4: Inverse address pattern
    defmt::info!("TEST inv_addr_pattern: RUNNING");
    {
        let mut ok = true;
        for (i, word) in ram[..WIN].iter_mut().enumerate() {
            *word = !((base + i * 4) as u32);
        }
        for (i, word) in ram[..WIN].iter().enumerate() {
            let expected = !((base + i * 4) as u32);
            if *word != expected {
                fail("inv_addr_pattern", base + i * 4, expected, *word);
                ok = false;
                break;
            }
        }
        if ok {
            pass("inv_addr_pattern");
        }
    }

    // Test 5: Random XOR-shift
    defmt::info!("TEST random_xorshift: RUNNING");
    {
        let mut rng = XorShift32::new(0xDEADBEEF);
        for word in ram[..WIN].iter_mut() {
            *word = rng.next();
        }
        let mut ok = true;
        let mut rng = XorShift32::new(0xDEADBEEF);
        for (i, word) in ram[..WIN].iter().enumerate() {
            let expected = rng.next();
            if *word != expected {
                fail("random_xorshift", base + i * 4, expected, *word);
                ok = false;
                break;
            }
        }
        if ok {
            pass("random_xorshift");
        }
    }

    // Test 6: Walking 1s
    defmt::info!("TEST walking_1s: RUNNING");
    {
        let mut ok = true;
        for bit in 0..32 {
            let pattern = 1u32 << bit;
            for word in ram[..WIN].iter_mut() {
                *word = pattern;
            }
            for (i, word) in ram[..WIN].iter().enumerate() {
                if *word != pattern {
                    fail("walking_1s", base + i * 4, pattern, *word);
                    ok = false;
                    break;
                }
            }
            if !ok {
                break;
            }
        }
        if ok {
            pass("walking_1s");
        }
    }

    // Test 7: Walking 0s
    defmt::info!("TEST walking_0s: RUNNING");
    {
        let mut ok = true;
        for bit in 0..32 {
            let pattern = !(1u32 << bit);
            for word in ram[..WIN].iter_mut() {
                *word = pattern;
            }
            for (i, word) in ram[..WIN].iter().enumerate() {
                if *word != pattern {
                    fail("walking_0s", base + i * 4, pattern, *word);
                    ok = false;
                    break;
                }
            }
            if !ok {
                break;
            }
        }
        if ok {
            pass("walking_0s");
        }
    }

    // Test 8: Solid fills
    defmt::info!("TEST solid_fills: RUNNING");
    {
        let mut ok = true;
        let fills: [u32; 4] = [0x00000000, 0xFFFFFFFF, 0xAAAAAAAA, 0x55555555];
        for &fill in &fills {
            for word in ram[..WIN].iter_mut() {
                *word = fill;
            }
            for (i, word) in ram[..WIN].iter().enumerate() {
                if *word != fill {
                    fail("solid_fills", base + i * 4, fill, *word);
                    ok = false;
                    break;
                }
            }
            if !ok {
                break;
            }
        }
        if ok {
            pass("solid_fills");
        }
    }

    // Test 9: March C-
    defmt::info!("TEST march_c: RUNNING");
    {
        let mut ok = true;
        for word in ram[..WIN].iter_mut() {
            *word = 0;
        }
        // Up: r0 w1
        for (i, word) in ram[..WIN].iter_mut().enumerate() {
            if *word != 0 {
                fail("march_c", base + i * 4, 0, *word);
                ok = false;
                break;
            }
            *word = 0xFFFFFFFF;
        }
        if ok {
            // Down: r1 w0
            for word in ram[..WIN].iter_mut().rev() {
                if *word != 0xFFFFFFFF {
                    let i = word as *const u32 as usize;
                    fail("march_c", i, 0xFFFFFFFF, *word);
                    ok = false;
                    break;
                }
                *word = 0;
            }
        }
        if ok {
            // Up: r0 w1
            for (i, word) in ram[..WIN].iter_mut().enumerate() {
                if *word != 0 {
                    fail("march_c", base + i * 4, 0, *word);
                    ok = false;
                    break;
                }
                *word = 0xFFFFFFFF;
            }
        }
        if ok {
            // Up: r1 w0
            for (i, word) in ram[..WIN].iter_mut().enumerate() {
                if *word != 0xFFFFFFFF {
                    fail("march_c", base + i * 4, 0xFFFFFFFF, *word);
                    ok = false;
                    break;
                }
                *word = 0;
            }
        }
        if ok {
            for (i, word) in ram[..WIN].iter().enumerate() {
                if *word != 0 {
                    fail("march_c", base + i * 4, 0, *word);
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            pass("march_c");
        }
    }

    // Test 10: Boundary spots
    defmt::info!("TEST boundary_spots: RUNNING");
    {
        let mut ok = true;
        let region_size = 1024;
        let num_regions = 16;
        let region_stride = words / num_regions;

        for r in 0..num_regions {
            let offset = r * region_stride;
            let pattern = 0xFEED0000 | (r as u32);
            let end = core::cmp::min(offset + region_size, words);
            for word in ram[offset..end].iter_mut() {
                *word = pattern;
            }
        }
        for r in 0..num_regions {
            let offset = r * region_stride;
            let pattern = 0xFEED0000 | (r as u32);
            let end = core::cmp::min(offset + region_size, words);
            for (i, word) in ram[offset..end].iter().enumerate() {
                if *word != pattern {
                    fail("boundary_spots", base + (offset + i) * 4, pattern, *word);
                    ok = false;
                    break;
                }
            }
            if !ok {
                break;
            }
        }
        if ok {
            pass("boundary_spots");
        }
    }

    // Test 11: Scattered random probes
    defmt::info!("TEST scattered_random: RUNNING");
    {
        let mut ok = true;
        let block_words = 1024;
        let mut rng = XorShift32::new(0xBEEFCAFE);

        for _probe in 0..32 {
            let offset = (rng.next() as usize) % (words - block_words);
            let seed_val = rng.next();

            let mut block_rng = XorShift32::new(seed_val);
            for word in ram[offset..offset + block_words].iter_mut() {
                *word = block_rng.next();
            }

            let mut block_rng = XorShift32::new(seed_val);
            for (i, word) in ram[offset..offset + block_words].iter().enumerate() {
                let expected = block_rng.next();
                if *word != expected {
                    fail("scattered_random", base + (offset + i) * 4, expected, *word);
                    ok = false;
                    break;
                }
            }
            if !ok {
                break;
            }
        }
        if ok {
            pass("scattered_random");
        }
    }

    // Test 12: Last 64K
    defmt::info!("TEST end_of_ram: RUNNING");
    {
        let mut ok = true;
        let last = 16384;
        let start = words - last;
        let mut rng = XorShift32::new(0x12345678);
        for word in ram[start..].iter_mut() {
            *word = rng.next();
        }
        let mut rng = XorShift32::new(0x12345678);
        for (i, word) in ram[start..].iter().enumerate() {
            let expected = rng.next();
            if *word != expected {
                fail("end_of_ram", base + (start + i) * 4, expected, *word);
                ok = false;
                break;
            }
        }
        if ok {
            pass("end_of_ram");
        }
    }

    // Test 13: Byte-level
    defmt::info!("TEST byte_level: RUNNING");
    {
        let mut ok = true;
        let ram_bytes: &mut [u8] =
            unsafe { core::slice::from_raw_parts_mut(sdram.base_address() as *mut u8, 4096) };
        for (i, byte) in ram_bytes.iter_mut().enumerate() {
            *byte = (i & 0xFF) as u8;
        }
        for (i, byte) in ram_bytes.iter().enumerate() {
            let expected = (i & 0xFF) as u8;
            if *byte != expected {
                fail("byte_level", base + i, expected as u32, *byte as u32);
                ok = false;
                break;
            }
        }
        if ok {
            pass("byte_level");
        }
    }

    // Test 14: Halfword-level
    defmt::info!("TEST halfword_level: RUNNING");
    {
        let mut ok = true;
        let ram_hw: &mut [u16] =
            unsafe { core::slice::from_raw_parts_mut(sdram.base_address() as *mut u16, 2048) };
        for (i, hw) in ram_hw.iter_mut().enumerate() {
            *hw = ((i & 0xFFFF) as u16).wrapping_add(1);
        }
        for (i, hw) in ram_hw.iter().enumerate() {
            let expected = ((i & 0xFFFF) as u16).wrapping_add(1);
            if *hw != expected {
                fail("halfword_level", base + i * 2, expected as u32, *hw as u32);
                ok = false;
                break;
            }
        }
        if ok {
            pass("halfword_level");
        }
    }

    // Summary
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("=== SDRAM Fast Test Summary ===");
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
