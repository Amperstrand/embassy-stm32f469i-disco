//! Complex display test for STM32F469I-DISCO (portrait 480x800)
//!
//! Tests `DisplayCtrl::new()` BSP API with raw DSI/LTDC register diagnostics.
//! Draws color bars, checkerboard, stripes, and blink loop to verify display.
//! Dumps LTDC/DSI registers after init for hardware debugging.
//!
//! Build:
//!   cargo build --release --target thumbv7em-none-eabihf \
//!     --manifest-path /home/ubuntu/src/embassy-stm32f469i-disco/Cargo.toml \
//!     --example async_display_test
//!
//! Flash (RTT debug — USB will NOT work):
//!   arm-none-eabi-objcopy -O binary target/thumbv7em-none-eabihf/release/async_display_test /tmp/async_display_test.bin
//!   st-flash --connect-under-reset write /tmp/async_display_test.bin 0x08000000
//!   st-flash --connect-under-reset reset

#![no_std]
#![no_main]

extern crate alloc;

#[cfg(feature = "defmt")]
use defmt_rtt as _;

#[cfg(feature = "defmt")]
use panic_probe as _;

#[cfg(not(feature = "defmt"))]
use panic_halt as _;

use linked_list_allocator::LockedHeap;

#[global_allocator]
static mut HEAP: LockedHeap = LockedHeap::empty();

const HEAP_SIZE: usize = 64 * 1024;
static mut HEAP_MEMORY: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

use embassy_stm32::rcc::{
    mux, AHBPrescaler, APBPrescaler, Hse, HseMode, Pll, PllMul, PllPDiv, PllPreDiv,
    PllQDiv, PllRDiv, PllSource, Sysclk,
};
use embassy_stm32::Config;
use embassy_stm32f469i_disco::display::SdramCtrl;

const GPIOD: usize = 0x40020C00;
const GPIOG: usize = 0x40021800;
const AHB1ENR: usize = 0x40023830;
const DSI_BASE: usize = 0x4001_6C00;
const LTDC_BASE: usize = 0x4001_6800;

unsafe fn led(gpio: usize, pin: u8, on: bool) {
    let moder = core::ptr::read_volatile((gpio) as *const u32);
    core::ptr::write_volatile((gpio) as *mut u32,
        (moder & !(0x3 << (pin * 2))) | (0x1 << (pin * 2)));
    let odr = core::ptr::read_volatile((gpio + 0x14) as *const u32);
    if on {
        core::ptr::write_volatile((gpio + 0x14) as *mut u32, odr | (1 << pin));
    } else {
        core::ptr::write_volatile((gpio + 0x14) as *mut u32, odr & !(1 << pin));
    }
}

unsafe fn setup_leds() {
    let ahb1 = core::ptr::read_volatile(AHB1ENR as *const u32);
    core::ptr::write_volatile(AHB1ENR as *mut u32, ahb1 | (1 << 5) | (1 << 6));
}

unsafe fn reg32(base: usize, offset: usize) -> u32 {
    core::ptr::read_volatile((base + offset) as *const u32)
}
unsafe fn reg32_write(base: usize, offset: usize, val: u32) {
    core::ptr::write_volatile((base + offset) as *mut u32, val);
}

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn LTDC() { cortex_m::asm::nop(); }
#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn DMA2D() { cortex_m::asm::nop(); }
#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn FMC() { cortex_m::asm::nop(); }

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn HardFault() -> ! {
    defmt::error!("HARDFAULT!");
    cortex_m::asm::delay(168_000_000);
    loop {
        led(GPIOD, 5, true);
        for _ in 0..500_000 {}
        led(GPIOD, 5, false);
        for _ in 0..500_000 {}
    }
}

fn step(n: u8) {
    unsafe {
        led(GPIOD, 4, false); led(GPIOD, 5, false); led(GPIOG, 6, false);
        match n {
            1 => { led(GPIOG, 6, true); }
            2 => { led(GPIOD, 4, true); }
            3 => { led(GPIOD, 5, true); }
            4 => { led(GPIOG, 6, true); led(GPIOD, 4, true); }
            5 => { led(GPIOD, 4, true); led(GPIOD, 5, true); }
            6 => { led(GPIOG, 6, true); led(GPIOD, 5, true); }
            7 => { led(GPIOG, 6, true); led(GPIOD, 4, true); led(GPIOD, 5, true); }
            _ => {}
        }
        cortex_m::asm::delay(3_000_000);
        led(GPIOD, 4, false); led(GPIOD, 5, false); led(GPIOG, 6, false);
    }
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    defmt::info!("=== async_display_test starting ===");
    // SAFETY: HEAP is only initialized once, before any concurrent access.
    #[allow(static_mut_refs)]
    unsafe {
        HEAP.lock().init(core::ptr::addr_of_mut!(HEAP_MEMORY) as *mut u8, HEAP_SIZE);
    }
    defmt::info!("HEAP initialized");
    unsafe { setup_leds(); }
    defmt::info!("LEDs set up");

    let mut config = Config::default();
    config.rcc.hse = Some(Hse { freq: embassy_stm32::time::mhz(8), mode: HseMode::Oscillator });
    config.rcc.pll_src = PllSource::HSE;
    config.rcc.pll = Some(Pll {
        prediv: PllPreDiv::DIV8, mul: PllMul::MUL360,
        divp: Some(PllPDiv::DIV2), divq: Some(PllQDiv::DIV7), divr: Some(PllRDiv::DIV6),
    });
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.mux.clk48sel = mux::Clk48sel::PLLSAI1_Q;
    config.rcc.pllsai = Some(Pll {
        prediv: PllPreDiv::DIV8,
        mul: PllMul::MUL384,
        divp: None,
        divq: Some(PllQDiv::DIV8),
        divr: Some(PllRDiv::DIV7),
    });

    let mut p = embassy_stm32::init(config);
    defmt::info!("STEP 1: embassy init done");
    step(1);

    let sdram = SdramCtrl::new(&mut p, 180_000_000);
    defmt::info!("STEP 2: SDRAM done");
    step(2);

    defmt::info!("STEP 3: calling DisplayCtrl::new()...");
    step(3);
    let _display = embassy_stm32f469i_disco::DisplayCtrl::new(
        &sdram,
        p.LTDC,
        p.DSIHOST,
        p.PJ2,
        p.PH7,
        embassy_stm32f469i_disco::BoardHint::Auto,
    );
    defmt::info!("STEP 4: DisplayCtrl::new() done!");
    step(4);

    // ── WCR.LTDCEN test: try setting bit 2 after init ──
    unsafe {
        let wcr_before = core::ptr::read_volatile((DSI_BASE + 0x404) as *const u32);
        defmt::info!("WCR before LTDCEN set: {:08x}", wcr_before);

        core::ptr::write_volatile((DSI_BASE + 0x404) as *mut u32, wcr_before | (1 << 2));

        let wcr_after = core::ptr::read_volatile((DSI_BASE + 0x404) as *const u32);
        defmt::info!("WCR after LTDCEN set:  {:08x} (LTDCEN={})", wcr_after, (wcr_after >> 2) & 1);
    }

    unsafe {
        let gcr_immediate = core::ptr::read_volatile(0x40016818 as *const u32);
        let sscr_rt = core::ptr::read_volatile(0x40016808 as *const u32);
        let bpcr_rt = core::ptr::read_volatile(0x4001680C as *const u32);
        let awcr_rt = core::ptr::read_volatile(0x40016810 as *const u32);
        let twcr_rt = core::ptr::read_volatile(0x40016814 as *const u32);
        let wcr_rt = core::ptr::read_volatile(0x40016C04 as *const u32);
        defmt::info!("RUNTIME GCR  = {:08x} SSCR={:08x} BPCR={:08x}", gcr_immediate, sscr_rt, bpcr_rt);
        defmt::info!("RUNTIME AWCR = {:08x} TWCR={:08x} WCR={:08x}", awcr_rt, twcr_rt, wcr_rt);
    }

    // ── Diagnostic: dump LTDC + DSI registers after init ──
    const L1_BASE: usize = LTDC_BASE + 0x84;

    unsafe {
        let gcr = core::ptr::read_volatile(LTDC_BASE as *const u32);
        let sscr = core::ptr::read_volatile((LTDC_BASE + 0x08) as *const u32);
        let bpcr = core::ptr::read_volatile((LTDC_BASE + 0x0C) as *const u32);
        let awcr = core::ptr::read_volatile((LTDC_BASE + 0x10) as *const u32);
        let twcr = core::ptr::read_volatile((LTDC_BASE + 0x14) as *const u32);
        let bccr = core::ptr::read_volatile((LTDC_BASE + 0x2C) as *const u32);
        let srcr = core::ptr::read_volatile((LTDC_BASE + 0x24) as *const u32);
        let l1cr = core::ptr::read_volatile(L1_BASE as *const u32);
        let l1whpcr = core::ptr::read_volatile((L1_BASE + 0x04) as *const u32);
        let l1wvpcr = core::ptr::read_volatile((L1_BASE + 0x08) as *const u32);
        let l1pfcr = core::ptr::read_volatile((L1_BASE + 0x10) as *const u32);
        let l1cacr = core::ptr::read_volatile((L1_BASE + 0x14) as *const u32);
        let l1dccr = core::ptr::read_volatile((L1_BASE + 0x18) as *const u32);
        let l1bfcr = core::ptr::read_volatile((L1_BASE + 0x1C) as *const u32);
        let l1cfbar = core::ptr::read_volatile((L1_BASE + 0x28) as *const u32);
        let l1cfblr = core::ptr::read_volatile((L1_BASE + 0x2C) as *const u32);
        let l1cfblnr = core::ptr::read_volatile((L1_BASE + 0x30) as *const u32);

        defmt::info!("LTDC GCR  = {:08x} (LTDCEN={}, DEN={})", gcr, gcr & 1, (gcr >> 1) & 1);
        defmt::info!("LTDC SSCR = {:08x}", sscr);
        defmt::info!("LTDC BPCR = {:08x}", bpcr);
        defmt::info!("LTDC AWCR = {:08x}", awcr);
        defmt::info!("LTDC TWCR = {:08x}", twcr);
        defmt::info!("LTDC BCCR = {:08x}", bccr);
        defmt::info!("LTDC SRCR = {:08x}", srcr);
        defmt::info!("LTDC L1CR   = {:08x} (LEN={})", l1cr, l1cr & 1);
        defmt::info!("LTDC L1WHPCR= {:08x}", l1whpcr);
        defmt::info!("LTDC L1WVPCR= {:08x}", l1wvpcr);
        defmt::info!("LTDC L1PFCR = {:08x}", l1pfcr);
        defmt::info!("LTDC L1CACR = {:08x}", l1cacr);
        defmt::info!("LTDC L1DCCR = {:08x}", l1dccr);
        defmt::info!("LTDC L1BFCR = {:08x}", l1bfcr);
        defmt::info!("LTDC L1CFBAR= {:08x}", l1cfbar);
        defmt::info!("LTDC L1CFBLR= {:08x}", l1cfblr);
        defmt::info!("LTDC L1CFBLN= {:08x}", l1cfblnr);

        // PLLSAI pixel clock verification (RCC base 0x4002_3800 + offset 0x88)
        let pllsaicfgr = core::ptr::read_volatile(0x4002_3888 as *const u32);
        let dckcfgr = core::ptr::read_volatile(0x4002_388C as *const u32);
        defmt::info!("RCC PLLSAICFGR = {:08x}", pllsaicfgr);
        defmt::info!("RCC DCKCFGR    = {:08x} (PLLSAIDIVR={})", dckcfgr, (dckcfgr >> 16) & 3);

        // DSI error flags
        let dsi_wisr = core::ptr::read_volatile((DSI_BASE + 0x40C) as *const u32);
        let dsi_isr0 = core::ptr::read_volatile((DSI_BASE + 0x0C4) as *const u32);
        let dsi_isr1 = core::ptr::read_volatile((DSI_BASE + 0x0C8) as *const u32);
        defmt::info!("DSI  WISR  = {:08x} (PLL lock={}, Reg ready={})", dsi_wisr, (dsi_wisr >> 8) & 1, (dsi_wisr >> 12) & 1);
        defmt::info!("DSI  ISR0  = {:08x}", dsi_isr0);
        defmt::info!("DSI  ISR1  = {:08x}", dsi_isr1);

        // DSI color coding
        let dsi_lcolcr = core::ptr::read_volatile((DSI_BASE + 0x10) as *const u32);
        defmt::info!("DSI  LCOLCR= {:08x} (DSI color mode={})", dsi_lcolcr, dsi_lcolcr & 7);

        let dsi_cr = core::ptr::read_volatile((DSI_BASE + 0x04) as *const u32);
        let dsi_wcr = core::ptr::read_volatile((DSI_BASE + 0x404) as *const u32);
        let dsi_wcfgr = core::ptr::read_volatile((DSI_BASE + 0x400) as *const u32);
        let dsi_cmcr = core::ptr::read_volatile((DSI_BASE + 0x68) as *const u32);
        let dsi_vmcr = core::ptr::read_volatile((DSI_BASE + 0x38) as *const u32);
        let dsi_vpcr = core::ptr::read_volatile((DSI_BASE + 0x3C) as *const u32);

        defmt::info!("DSI  CR    = {:08x} (EN={}, CMDM={})", dsi_cr, dsi_cr & 1, (dsi_cr >> 2) & 1);
        defmt::info!("DSI  WCR   = {:08x} (DSIEN={}, LTDCEN={})", dsi_wcr, (dsi_wcr >> 3) & 1, (dsi_wcr >> 2) & 1);
        defmt::info!("DSI  WCFGR = {:08x} (DSIM={}, VIDS={})", dsi_wcfgr, dsi_wcfgr & 1, (dsi_wcfgr >> 1) & 1);
        defmt::info!("DSI  CMCR  = {:08x}", dsi_cmcr);
        defmt::info!("DSI  VMCR  = {:08x}", dsi_vmcr);
        defmt::info!("DSI  VPCR  = {:08x} (active_w={})", dsi_vpcr, dsi_vpcr & 0xFFF);
    }

    let fb_ptr = 0xC0000000 as *mut u16;

    fn fill_fb(fb: *mut u16, val: u16) {
        unsafe {
            let fb = core::slice::from_raw_parts_mut(fb, 480 * 800);
            for p in fb.iter_mut() { *p = val; }
        }
    }

    fn phase(n: u8) {
        unsafe {
            led(GPIOD, 4, false); led(GPIOD, 5, false); led(GPIOG, 6, false);
            match n {
                1 => { led(GPIOG, 6, true); }  // green = DIAG 0b (background RED, layer off)
                2 => { led(GPIOD, 4, true); }  // orange = black/white/black
                3 => { led(GPIOD, 5, true); }  // red = checkerboard
                4 => { led(GPIOG, 6, true); led(GPIOD, 4, true); }  // green+orange = color bars
                5 => { led(GPIOD, 4, true); led(GPIOD, 5, true); }  // orange+red = vertical stripes
                6 => { led(GPIOG, 6, true); led(GPIOD, 5, true); }  // green+red = white block
                7 => { led(GPIOG, 6, true); led(GPIOD, 4, true); led(GPIOD, 5, true); }  // all = blink loop
                _ => {}
            }
        }
    }

    // ── DIAG 0: LTDC ISR (FIFO underrun / transfer error) ──
    // LTDC_ISR: LIF(0), FUIF(1), TERRIF(2), RRIF(3)
    unsafe {
        let isr = reg32(LTDC_BASE, 0x48);
        defmt::info!("LTDC ISR={:08x} FUIF={} TERRIF={}", isr, (isr >> 1) & 1, (isr >> 2) & 1);
    }

    // ── DIAG 0b: Background RED, layer disabled ──
    // Clean red = DSI/panel fine, SDRAM fetch is the problem
    // Noise = LTDC output not reaching panel at all
    phase(1);
    defmt::info!("DIAG 0b: BG RED, layer OFF — green LED");
    unsafe {
        reg32_write(LTDC_BASE, 0x84, 0); // L1CR.LEN=0
        reg32_write(LTDC_BASE, 0x2C, 0xF800); // BCCR=RED
        reg32_write(LTDC_BASE, 0x24, 0x01); // SRCR reload
    }
    cortex_m::asm::delay(168_000_000 * 5);
    unsafe {
        let bccr = reg32(LTDC_BASE, 0x2C);
        let l1cr = reg32(LTDC_BASE, 0x84);
        let wcr = reg32(DSI_BASE, 0x404);
        let cr = reg32(DSI_BASE, 0x04);
        let wcfgr = reg32(DSI_BASE, 0x400);
        defmt::info!("DIAG 0b: BCCR={:08x} L1CR={:08x} DSI.WCR={:08x} DSI.CR={:08x} DSI.WCFGR={:08x}",
            bccr, l1cr, wcr, cr, wcfgr);
    }

    // Re-enable layer
    unsafe {
        reg32_write(LTDC_BASE, 0x84, 1); // L1CR.LEN=1
        reg32_write(LTDC_BASE, 0x2C, 0);  // BCCR=black
        reg32_write(LTDC_BASE, 0x24, 0x01); // SRCR reload
    }

    // ── TEST 1: Black → White → Black ──
    phase(2);
    defmt::info!("TEST 1: BLACK → WHITE → BLACK — orange LED");
    fill_fb(fb_ptr, 0x0000);
    cortex_m::asm::delay(168_000_000 * 3);
    fill_fb(fb_ptr, 0xFFFF);
    cortex_m::asm::delay(168_000_000 * 3);
    fill_fb(fb_ptr, 0x0000);
    cortex_m::asm::delay(168_000_000 * 3);

    // ── TEST 2: Checkerboard 32px ──
    phase(3);
    defmt::info!("TEST 2: CHECKERBOARD 32px — red LED");
    unsafe {
        let fb = core::slice::from_raw_parts_mut(fb_ptr, 480 * 800);
        for y in 0..800u32 {
            for x in 0..480u32 {
                fb[(y * 480 + x) as usize] = if ((x / 32) + (y / 32)) % 2 == 0 { 0x0000 } else { 0xFFFF };
            }
        }
    }
    cortex_m::asm::delay(168_000_000 * 3);

    // ── TEST 3: 8 horizontal color bars ──
    phase(4);
    defmt::info!("TEST 3: 8 COLOR BARS — green+orange LED");
    unsafe {
        let fb = core::slice::from_raw_parts_mut(fb_ptr, 480 * 800);
        const COLORS: [u16; 8] = [0xF800, 0x07E0, 0x001F, 0xFFE0, 0xF81F, 0x07FF, 0xFFFF, 0x0000];
        for y in 0..800u32 {
            let c = COLORS[(y / 100).min(7) as usize];
            for x in 0..480u32 {
                fb[(y * 480 + x) as usize] = c;
            }
        }
    }
    cortex_m::asm::delay(168_000_000 * 3);

    // ── TEST 4: Vertical stripes 4px ──
    phase(5);
    defmt::info!("TEST 4: VERTICAL STRIPES 4px — orange+red LED");
    unsafe {
        let fb = core::slice::from_raw_parts_mut(fb_ptr, 480 * 800);
        for y in 0..800u32 {
            for x in 0..480u32 {
                fb[(y * 480 + x) as usize] = if (x / 4) % 2 == 0 { 0x0000 } else { 0xFFFF };
            }
        }
    }
    cortex_m::asm::delay(168_000_000 * 3);

    // ── TEST 5: White 16x16 block at (0,0) ──
    phase(6);
    defmt::info!("TEST 5: WHITE 16x16 top-left — green+red LED");
    fill_fb(fb_ptr, 0x0000);
    unsafe {
        let fb = core::slice::from_raw_parts_mut(fb_ptr, 480 * 800);
        for y in 0..16u32 {
            for x in 0..16u32 {
                fb[(y * 480 + x) as usize] = 0xFFFF;
            }
        }
    }
    cortex_m::asm::delay(168_000_000 * 3);

    // ── TEST 6: Blink loop 1s ──
    phase(7);
    defmt::info!("TEST 6: BLINK LOOP 1s — ALL LEDs");
    let mut white = false;
    loop {
        fill_fb(fb_ptr, if white { 0xFFFFu16 } else { 0x0000u16 });
        white = !white;
        cortex_m::asm::delay(168_000_000);
    }
}
