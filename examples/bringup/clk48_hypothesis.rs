//! CK48MSEL Hypothesis Test — Issue #27
//!
//! Tests 4 conditions to isolate whether the 48MHz clock fix is:
//!   (a) PLLSAI_P configuration (divp: DIV8), or
//!   (b) DCKCFGR2 register write (claimed workaround)
//!
//! CONDITIONS (change CONDITION below, rebuild, reflash):
//!   A = divp:None,  no DCKCFGR2 write  → Expected: RNG FAIL (PLLSAI_P=192MHz)
//!   B = divp:DIV8, no DCKCFGR2 write  → Expected: RNG PASS (PLLSAI_P=48MHz, embassy DCKCFGR correct)
//!   C = divp:DIV8, DCKCFGR2 write     → Expected: RNG PASS (same as B, DCKCFGR2 is irrelevant)
//!   D = divp:None, DCKCFGR2 write     → Expected: RNG FAIL (DCKCFGR2 can't fix PLLSAI_P=192MHz)
//!
//! Key predictions:
//!   - A vs B: divp is the causal variable
//!   - B vs C: DCKCFGR2 write makes no difference (no-op on F469)
//!   - A vs D: DCKCFGR2 cannot compensate for missing PLLSAI_P config
//!
//! Run: probe-rs run --chip STM32F469NIHx --example test_clk48_hypothesis

#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use core::sync::atomic::{AtomicUsize, Ordering};

use embassy_stm32::rcc::*;
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;
use embassy_time::Timer;

// ── CHANGE THIS FOR EACH CONDITION ──────────────────────────────────
const CONDITION: char = 'A';
// A = divp:None,  no DCKCFGR2   (expected FAIL)
// B = divp:DIV8, no DCKCFGR2   (expected PASS)
// C = divp:DIV8, DCKCFGR2 write (expected PASS)
// D = divp:None,  DCKCFGR2 write (expected FAIL)

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

unsafe fn rng_init() -> bool {
    let rcc = stm32_metapac::RCC;
    rcc.ahb2enr().modify(|w| w.set_rngen(true));

    let rng = stm32_metapac::RNG;
    rng.cr().write(|w| w.set_rngen(false));
    rng.sr().modify(|w| {
        w.set_seis(false);
        w.set_ceis(false);
    });
    rng.cr().modify(|w| w.set_rngen(true));

    let mut timeout = 100_000u32;
    while !rng.sr().read().drdy() {
        timeout -= 1;
        if timeout == 0 {
            return false;
        }
    }
    let _ = rng.dr().read();
    true
}

unsafe fn rng_next_u32() -> u32 {
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
            return rng.dr().read();
        }
    }
}

/// Read DCKCFGR and DCKCFGR2 registers for diagnostic output
unsafe fn dump_clk48_regs() {
    let dckcfgr_val = stm32_metapac::RCC.dckcfgr().read().0;
    defmt::info!("  DCKCFGR  = {:#010X}", dckcfgr_val);
    defmt::info!("  CK48MSEL bit27 = {}", (dckcfgr_val >> 27) & 1);

    // Read offset 0x94 directly — may be undefined on F469
    let dckcfgr2_val = stm32_metapac::RCC.dckcfgr2().read().0;
    defmt::info!(
        "  DCKCFGR2 = {:#010X} (may be undefined on F469)",
        dckcfgr2_val
    );
    defmt::info!("  CK48MSEL bit27 = {}", (dckcfgr2_val >> 27) & 1);
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    // ── 180MHz PLL config ────────────────────────────────────────────
    let mut config = Config::default();
    config.rcc.hse = Some(Hse {
        freq: Hertz(8_000_000),
        mode: HseMode::Oscillator,
    });
    config.rcc.pll_src = PllSource::HSE;
    config.rcc.pll = Some(Pll {
        prediv: PllPreDiv::DIV8,
        mul: PllMul::MUL360,
        divp: Some(PllPDiv::DIV2), // 180 MHz sysclk
        divq: Some(PllQDiv::DIV7), // 51.4 MHz (unused for USB)
        divr: Some(PllRDiv::DIV6),
    });

    // ── PLLSAI config depends on CONDITION ───────────────────────────
    let divp_setting = match CONDITION {
        'A' | 'D' => None,                // PLLSAI_P = 384/2 = 192 MHz (default)
        'B' | 'C' => Some(PllPDiv::DIV8), // PLLSAI_P = 384/8 = 48 MHz
        _ => None,
    };

    config.rcc.pllsai = Some(Pll {
        prediv: PllPreDiv::DIV8,
        mul: PllMul::MUL384,
        divp: divp_setting,
        divq: Some(PllQDiv::DIV8), // 48 MHz (embassy freq table)
        divr: Some(PllRDiv::DIV7), // 54.86 MHz (LTDC pixel clock)
    });

    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.sys = Sysclk::PLL1_P;

    // Embassy writes clk48sel to DCKCFGR (correct for F469)
    config.rcc.mux.clk48sel = mux::Clk48sel::PLLSAI1_Q;

    let _p = embassy_stm32::init(config);

    // ── DCKCFGR2 write depends on CONDITION ──────────────────────────
    let dckcfgr2_write = matches!(CONDITION, 'C' | 'D');
    if dckcfgr2_write {
        stm32_metapac::RCC.dckcfgr2().modify(|w| {
            w.set_clk48sel(mux::Clk48sel::PLLSAI1_Q);
        });
    }

    // ── Report condition ──────────────────────────────────────────────
    defmt::info!("╔══════════════════════════════════════════╗");
    defmt::info!("║  CK48MSEL Hypothesis Test — Condition {}  ║", CONDITION);
    defmt::info!("╚══════════════════════════════════════════╝");
    defmt::info!(
        "  PLLSAI divp = {}",
        match divp_setting {
            Some(PllPDiv::DIV8) => "DIV8 (48 MHz)",
            Some(_) => "other",
            None => "None (192 MHz default)",
        }
    );
    defmt::info!("  DCKCFGR2 write = {}", dckcfgr2_write);
    defmt::info!("  Embassy clk48sel → DCKCFGR (upstream default)");

    // ── Dump register state ───────────────────────────────────────────
    defmt::info!("Register state after init:");
    unsafe {
        dump_clk48_regs();
    }

    // ── Test 1: RNG init (48MHz clock proxy) ─────────────────────────
    defmt::info!("TEST rng_init_48mhz: RUNNING");
    let rng_ok = unsafe { rng_init() };
    if rng_ok {
        pass("rng_init_48mhz");
    } else {
        fail(
            "rng_init_48mhz",
            "RNG not ready — no 48MHz clock reaching RNG",
        );
    }

    // ── Test 2: RNG not-zeros (if init passed) ────────────────────────
    if rng_ok {
        defmt::info!("TEST rng_not_zeros: RUNNING");
        let mut all_zero = true;
        for _ in 0..8 {
            let val = unsafe { rng_next_u32() };
            if val != 0 {
                all_zero = false;
                break;
            }
        }
        if !all_zero {
            pass("rng_not_zeros");
        } else {
            fail("rng_not_zeros", "all zeros — clock present but RNG faulty");
        }
    }

    // ── Test 3: RNG uniqueness (if init passed) ──────────────────────
    if rng_ok {
        defmt::info!("TEST rng_uniqueness: RUNNING");
        let mut buf = [0u32; 64];
        for slot in buf.iter_mut() {
            *slot = unsafe { rng_next_u32() };
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
        defmt::info!("  unique: {}/64", unique);
        if unique >= 32 {
            pass("rng_uniqueness");
        } else {
            fail("rng_uniqueness", "low uniqueness");
        }
    }

    // ── Test 4: DCKCFGR2 register readback ───────────────────────────
    defmt::info!("TEST dckcfgr2_readback: RUNNING");
    {
        let dckcfgr2_val = stm32_metapac::RCC.dckcfgr2().read().0;
        let clk48_bit = (dckcfgr2_val >> 27) & 1;
        if dckcfgr2_write {
            // We wrote to DCKCFGR2 — check if the write "stuck"
            if clk48_bit == 1 {
                pass("dckcfgr2_readback");
                defmt::info!("  DCKCFGR2 write stuck (register exists at 0x94)");
            } else {
                fail(
                    "dckcfgr2_readback",
                    "write did not stick — register may not exist",
                );
            }
        } else {
            // We didn't write — just report what we see
            defmt::info!("  DCKCFGR2 bit27 = {} (no write performed)", clk48_bit);
            pass("dckcfgr2_readback");
        }
    }

    // ── Summary ──────────────────────────────────────────────────────
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("══════════════════════════════════════════");
    defmt::info!(
        "Condition {} SUMMARY: {}/{} passed",
        CONDITION,
        passed,
        total
    );

    // ── Verdict ───────────────────────────────────────────────────────
    let rng_passed = rng_ok;
    match CONDITION {
        'A' => {
            if !rng_passed {
                defmt::info!("VERDICT A: CONFIRMED — divp:None → RNG fails (PLLSAI_P=192MHz)");
            } else {
                defmt::error!("VERDICT A: UNEXPECTED — RNG passed with divp:None!");
            }
        }
        'B' => {
            if rng_passed {
                defmt::info!("VERDICT B: CONFIRMED — divp:DIV8 + embassy DCKCFGR → RNG passes");
            } else {
                defmt::error!("VERDICT B: UNEXPECTED — RNG failed with divp:DIV8!");
            }
        }
        'C' => {
            if rng_passed {
                defmt::info!(
                    "VERDICT C: CONFIRMED — divp:DIV8 + DCKCFGR2 write → RNG passes (same as B)"
                );
            } else {
                defmt::error!("VERDICT C: UNEXPECTED — RNG failed with divp:DIV8 + DCKCFGR2!");
            }
        }
        'D' => {
            if !rng_passed {
                defmt::info!("VERDICT D: CONFIRMED — divp:None + DCKCFGR2 write → RNG still fails");
            } else {
                defmt::error!("VERDICT D: UNEXPECTED — RNG passed with divp:None + DCKCFGR2!");
            }
        }
        _ => defmt::error!("Invalid condition: set CONDITION to A, B, C, or D"),
    }

    if failed == 0 {
        defmt::info!("ALL TESTS PASSED");
    }

    loop {
        Timer::after(embassy_time::Duration::from_secs(1)).await;
    }
}
