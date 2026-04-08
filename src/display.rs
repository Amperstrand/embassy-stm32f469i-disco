//! Display subsystem: SDRAM controller (FMC), DSI/LTDC display driver (NT35510/OTM8009A),
//! panel detection, and framebuffer management.

use embassy_stm32::gpio::{AfType, Flex, OutputType, Pull, Speed};
use embassy_stm32::rcc;
use embedded_display_controller::dsi::{DsiHostCtrlIo, DsiReadCommand, DsiWriteCommand};
use embedded_graphics::{
    draw_target::DrawTarget, pixelcolor::Rgb565, prelude::*, primitives::Rectangle,
};
use embedded_hal::delay::DelayNs;
use nt35510::Nt35510;
#[cfg(feature = "display")]
use otm8009a::Otm8009A;
use stm32_fmc::devices::is42s32400f_6::Is42s32400f6;
use stm32_fmc::{FmcPeripheral, Sdram, SdramTargetBank};

#[cfg(feature = "defmt")]
use defmt as _;

struct BusyDelay;

impl DelayNs for BusyDelay {
    fn delay_ns(&mut self, ns: u32) {
        let ticks = (ns as u64 * 168) / 1000;
        cortex_m::asm::delay(ticks.max(1) as u32);
    }
}

/// Adapter for OTM8009A (requires embedded-hal 0.2 blocking delay traits).
#[cfg(feature = "display")]
struct DelayMsAdapter;

#[cfg(feature = "display")]
impl embedded_hal_02::blocking::delay::DelayMs<u32> for DelayMsAdapter {
    fn delay_ms(&mut self, ms: u32) {
        BusyDelay.delay_ms(ms);
    }
}

pub const SDRAM_SIZE_BYTES: usize = 16 * 1024 * 1024;
pub const FB_WIDTH: u16 = 480;
pub const FB_HEIGHT: u16 = 800;
pub const FB_SIZE: usize = FB_WIDTH as usize * FB_HEIGHT as usize;

const FMC_AF12: AfType = AfType::output_pull(OutputType::PushPull, Speed::VeryHigh, Pull::Up);

const DSI_BASE: usize = 0x4001_6C00;
const LTDC_BASE: usize = 0x4001_6800;

// ── SDRAM ──────────────────────────────────────────────────────────────

struct EmbassyFmc {
    source_clock: u32,
}

unsafe impl Send for EmbassyFmc {}
unsafe impl FmcPeripheral for EmbassyFmc {
    const REGISTERS: *const () = 0xa000_0000 as *const ();
    fn enable(&mut self) {
        rcc::enable_and_reset::<embassy_stm32::peripherals::FMC>();
    }
    fn source_clock_hz(&self) -> u32 {
        self.source_clock
    }
}

fn sdram_pin(pin: embassy_stm32::Peri<'_, impl embassy_stm32::gpio::Pin>) {
    let mut flex = Flex::new(pin);
    flex.set_as_af_unchecked(12, FMC_AF12);
    core::mem::forget(flex);
}

pub struct SdramCtrl {
    mem: *mut u32,
}

impl SdramCtrl {
    pub fn new(p: &mut embassy_stm32::Peripherals, source_clock_hz: u32) -> Self {
        sdram_pin(unsafe { p.PF0.clone_unchecked() });
        sdram_pin(unsafe { p.PF1.clone_unchecked() });
        sdram_pin(unsafe { p.PF2.clone_unchecked() });
        sdram_pin(unsafe { p.PF3.clone_unchecked() });
        sdram_pin(unsafe { p.PF4.clone_unchecked() });
        sdram_pin(unsafe { p.PF5.clone_unchecked() });
        sdram_pin(unsafe { p.PF11.clone_unchecked() });
        sdram_pin(unsafe { p.PF12.clone_unchecked() });
        sdram_pin(unsafe { p.PF13.clone_unchecked() });
        sdram_pin(unsafe { p.PF14.clone_unchecked() });
        sdram_pin(unsafe { p.PF15.clone_unchecked() });
        sdram_pin(unsafe { p.PG0.clone_unchecked() });
        sdram_pin(unsafe { p.PG1.clone_unchecked() });
        sdram_pin(unsafe { p.PG4.clone_unchecked() });
        sdram_pin(unsafe { p.PG5.clone_unchecked() });
        sdram_pin(unsafe { p.PG8.clone_unchecked() });
        sdram_pin(unsafe { p.PG15.clone_unchecked() });
        sdram_pin(unsafe { p.PD0.clone_unchecked() });
        sdram_pin(unsafe { p.PD1.clone_unchecked() });
        sdram_pin(unsafe { p.PD8.clone_unchecked() });
        sdram_pin(unsafe { p.PD9.clone_unchecked() });
        sdram_pin(unsafe { p.PD10.clone_unchecked() });
        sdram_pin(unsafe { p.PD14.clone_unchecked() });
        sdram_pin(unsafe { p.PD15.clone_unchecked() });
        sdram_pin(unsafe { p.PE0.clone_unchecked() });
        sdram_pin(unsafe { p.PE1.clone_unchecked() });
        sdram_pin(unsafe { p.PE7.clone_unchecked() });
        sdram_pin(unsafe { p.PE8.clone_unchecked() });
        sdram_pin(unsafe { p.PE9.clone_unchecked() });
        sdram_pin(unsafe { p.PE10.clone_unchecked() });
        sdram_pin(unsafe { p.PE11.clone_unchecked() });
        sdram_pin(unsafe { p.PE12.clone_unchecked() });
        sdram_pin(unsafe { p.PE13.clone_unchecked() });
        sdram_pin(unsafe { p.PE14.clone_unchecked() });
        sdram_pin(unsafe { p.PE15.clone_unchecked() });
        sdram_pin(unsafe { p.PH2.clone_unchecked() });
        sdram_pin(unsafe { p.PH3.clone_unchecked() });
        sdram_pin(unsafe { p.PH8.clone_unchecked() });
        sdram_pin(unsafe { p.PH9.clone_unchecked() });
        sdram_pin(unsafe { p.PH10.clone_unchecked() });
        sdram_pin(unsafe { p.PH11.clone_unchecked() });
        sdram_pin(unsafe { p.PH12.clone_unchecked() });
        sdram_pin(unsafe { p.PH13.clone_unchecked() });
        sdram_pin(unsafe { p.PH14.clone_unchecked() });
        sdram_pin(unsafe { p.PH15.clone_unchecked() });
        sdram_pin(unsafe { p.PI0.clone_unchecked() });
        sdram_pin(unsafe { p.PI1.clone_unchecked() });
        sdram_pin(unsafe { p.PI2.clone_unchecked() });
        sdram_pin(unsafe { p.PI3.clone_unchecked() });
        sdram_pin(unsafe { p.PI4.clone_unchecked() });
        sdram_pin(unsafe { p.PI5.clone_unchecked() });
        sdram_pin(unsafe { p.PI6.clone_unchecked() });
        sdram_pin(unsafe { p.PI7.clone_unchecked() });
        sdram_pin(unsafe { p.PI9.clone_unchecked() });
        sdram_pin(unsafe { p.PI10.clone_unchecked() });
        sdram_pin(unsafe { p.PC0.clone_unchecked() });

        let fmc = EmbassyFmc {
            source_clock: source_clock_hz,
        };
        let mut sdram: Sdram<EmbassyFmc, Is42s32400f6> =
            Sdram::new_unchecked(fmc, SdramTargetBank::Bank1, Is42s32400f6 {});
        let mut delay = BusyDelay;
        let mem = sdram.init(&mut delay);
        SdramCtrl { mem }
    }

    pub fn base_address(&self) -> usize {
        self.mem as usize
    }

    pub fn subslice_mut<T>(&self, offset_bytes: usize, len: usize) -> &'static mut [T] {
        let start = (self.mem as usize) + offset_bytes;
        let end = start + len * core::mem::size_of::<T>();
        assert!(end <= (self.mem as usize) + SDRAM_SIZE_BYTES);
        unsafe { &mut *core::ptr::slice_from_raw_parts_mut(start as *mut T, len) }
    }

    #[must_use]
    pub fn test_quick(&self) -> bool {
        let words = unsafe { core::slice::from_raw_parts_mut(self.mem as *mut u32, 1024) };
        for word in words.iter_mut() {
            *word = 0xDEAD_BEEF;
        }
        for &word in words.iter() {
            if word != 0xDEAD_BEEF {
                return false;
            }
        }
        for word in words.iter_mut() {
            *word = 0;
        }
        true
    }
}

// ── Raw register helpers ──────────────────────────────────────────────

#[inline(always)]
unsafe fn reg32(base: usize, offset: usize) -> u32 {
    core::ptr::read_volatile((base + offset) as *const u32)
}

#[inline(always)]
unsafe fn reg32_set(base: usize, offset: usize, val: u32) {
    let old = core::ptr::read_volatile((base + offset) as *const u32);
    core::ptr::write_volatile((base + offset) as *mut u32, old | val);
}

#[inline(always)]
unsafe fn reg32_clear(base: usize, offset: usize, val: u32) {
    let old = core::ptr::read_volatile((base + offset) as *const u32);
    core::ptr::write_volatile((base + offset) as *mut u32, old & !val);
}

#[inline(always)]
unsafe fn reg32_write(base: usize, offset: usize, val: u32) {
    core::ptr::write_volatile((base + offset) as *mut u32, val);
}

#[inline(always)]
unsafe fn reg32_modify(base: usize, offset: usize, f: impl FnOnce(u32) -> u32) {
    let old = core::ptr::read_volatile((base + offset) as *const u32);
    core::ptr::write_volatile((base + offset) as *mut u32, f(old));
}

// ── DSI PHY init ──────────────────────────────────────────────────────

unsafe fn dsi_init() {
    const CR: usize = 0x04;
    const CCR: usize = 0x08;
    const LVCIDR: usize = 0x0C;
    const LCOLCR: usize = 0x10;
    const LPCR: usize = 0x14;
    const LPMCR: usize = 0x18;
    const PCR: usize = 0x2C;
    const VMCR: usize = 0x38;
    const VPCR: usize = 0x3C;
    const VCCR: usize = 0x40;
    const VNPCR: usize = 0x44;
    const VHSACR: usize = 0x48;
    const VHBPACR: usize = 0x4C;
    const VLCR: usize = 0x50;
    const VVSACR: usize = 0x54;
    const VVBPCR: usize = 0x58;
    const VVFPCR: usize = 0x5C;
    const VVACR: usize = 0x60;
    const CLCR: usize = 0x94;
    const CLTCR: usize = 0x98;
    const DLTCR: usize = 0x9C;
    const PCTLR: usize = 0xA0;
    const PCONFR: usize = 0xA4;
    const IER0: usize = 0xC4;
    const IER1: usize = 0xC8;
    const WRPCR: usize = 0x430;
    const WISR: usize = 0x40C;
    const WCFGR: usize = 0x400;
    const WCR: usize = 0x404;
    const WPCR0: usize = 0x418;

    let h_sync = 2u32;
    let h_back_porch = 34u32;
    let h_front_porch = 34u32;
    let v_sync = 1u32;
    let v_back_porch = 15u32;
    let v_front_porch = 16u32;
    let active_width = FB_WIDTH as u32;
    let active_height = FB_HEIGHT as u32;
    let lane_byte_clk = 500_000_000u32;

    // Shutdown
    reg32_clear(DSI_BASE, CR, 1 << 2); // CMDM=0
    reg32_clear(DSI_BASE, WCFGR, 1 << 0); // DSIM=0
    reg32_clear(DSI_BASE, CR, 1 << 0); // EN=0
    reg32_write(DSI_BASE, PCTLR, 0); // CKE=0, DEN=0
    reg32_clear(DSI_BASE, WRPCR, 1 << 0); // PLLEN=0
    reg32_clear(DSI_BASE, WRPCR, 1 << 24); // REGEN=0

    cortex_m::asm::delay(168_000);

    // Enable DSIHOST and LTDC peripheral clocks
    unsafe {
        let apb2enr_addr = 0x4002_3844usize;
        let apb2enr = core::ptr::read_volatile(apb2enr_addr as *const u32);
        core::ptr::write_volatile(apb2enr_addr as *mut u32, apb2enr | (1 << 27) | (1 << 26));
    }
    cortex_m::asm::delay(168_000);

    // Regulator
    reg32_set(DSI_BASE, WRPCR, 1 << 24); // REGEN=1
    let mut timeout = 100_000u32;
    while reg32(DSI_BASE, WISR) & (1 << 12) == 0 && timeout > 0 {
        timeout -= 1;
    }
    assert!(timeout > 0, "DSI regulator timeout");

    // PLL: VCO = (8MHz / IDF=2) * NDIV=125 = 500MHz
    reg32_modify(DSI_BASE, WRPCR, |w| {
        (w & !(0x7F << 2 | 0x0F << 11 | 0x03 << 16))
        | (125 << 2)    // NDIV=125
        | (0x02 << 11)  // IDF=2
        | (0x00 << 16) // ODF=0
    });
    reg32_set(DSI_BASE, WRPCR, 1 << 0); // PLLEN=1

    cortex_m::asm::delay(168_000 / 2);

    timeout = 100_000u32;
    while reg32(DSI_BASE, WISR) & (1 << 8) == 0 && timeout > 0 {
        timeout -= 1;
    }
    assert!(timeout > 0, "DSI PLL lock timeout");

    // PHY params
    reg32_set(DSI_BASE, PCTLR, 1 << 0 | 1 << 1); // CKE=1, DEN=1
    reg32_modify(DSI_BASE, CLCR, |w| w | (1 << 0)); // DPCC=1
    reg32_modify(DSI_BASE, PCONFR, |w| (w & !0x03) | 0x01); // NL=1 (2 data lanes)
    reg32_write(DSI_BASE, CCR, 4); // TXECKDIV=4
    reg32_write(DSI_BASE, WPCR0, 13); // UIX4=13 (4GHz / f_phy_bit=312.5MHz)
    reg32_write(DSI_BASE, IER0, 0);
    reg32_write(DSI_BASE, IER1, 0);
    reg32_set(DSI_BASE, PCR, 1 << 2); // BTAE=1

    // Video mode: burst
    reg32_clear(DSI_BASE, CR, 1 << 2); // CMDM=0
    reg32_clear(DSI_BASE, WCFGR, 1 << 0); // DSIM=0
    reg32_write(DSI_BASE, VPCR, active_width);
    reg32_write(DSI_BASE, VCCR, 1); // NUMC=1
    reg32_write(DSI_BASE, VNPCR, 0);
    reg32_write(DSI_BASE, LVCIDR, 0);
    reg32_write(DSI_BASE, LPCR, 0);
    reg32_write(DSI_BASE, LCOLCR, 0x00);
    reg32_modify(DSI_BASE, WCFGR, |w| (w & !(0x07 << 1)) | (0x00 << 1));

    // VMCR: LP transition enables + LPCE, VMT=2 (burst)
    reg32_write(
        DSI_BASE,
        VMCR,
        (0x02) | (1 << 8) | (1 << 9) | (1 << 10) | (1 << 11) | (1 << 12) | (1 << 13) | (1 << 15),
    );

    // DSI timing
    let f_pix_khz: u32 = lane_byte_clk / 1_000 / 8;
    let f_ltdc_khz: u32 = 27_429;

    let dsi_hsa = ((h_sync as u32) * f_pix_khz / f_ltdc_khz) as u32;
    let dsi_hbp = ((h_back_porch as u32) * f_pix_khz / f_ltdc_khz) as u32;
    let dsi_hline = (((active_width + h_sync + h_back_porch + h_front_porch) as u32) * f_pix_khz
        / f_ltdc_khz) as u32;

    reg32_write(DSI_BASE, VHSACR, dsi_hsa);
    reg32_write(DSI_BASE, VHBPACR, dsi_hbp);
    reg32_write(DSI_BASE, VLCR, dsi_hline);
    reg32_write(DSI_BASE, VVSACR, v_sync);
    reg32_write(DSI_BASE, VVBPCR, v_back_porch);
    reg32_write(DSI_BASE, VVFPCR, v_front_porch);
    reg32_write(DSI_BASE, VVACR, active_height);

    reg32_write(DSI_BASE, LPMCR, (64 << 0) | (64 << 8));

    reg32_write(DSI_BASE, CLTCR, (35 << 0) | (35 << 16));
    reg32_write(DSI_BASE, DLTCR, (35 << 0) | (35 << 8) | (0 << 16));
    reg32_modify(DSI_BASE, PCONFR, |w| (w & !(0x1F << 16)) | (10 << 16));

    cortex_m::asm::delay(168_000 * 10);

    // Enable DSI host and wrapper
    reg32_set(DSI_BASE, CR, 1 << 0); // EN=1
    reg32_set(DSI_BASE, WCR, 1 << 3); // DSIEN=1 (bit 3, NOT bit 0)
}

// ── LTDC init ─────────────────────────────────────────────────────────

unsafe fn ltdc_init() {
    let dckcfgr = 0x4002_388Cusize;
    let dck_val = core::ptr::read_volatile(dckcfgr as *const u32);
    core::ptr::write_volatile(
        dckcfgr as *mut u32,
        (dck_val & !(0x03 << 16)) | (0b00 << 16),
    );

    const APB2RSTR: usize = 0x4002_3A24;
    const AHB1RSTR: usize = 0x4002_3808;
    let apb2rstr = APB2RSTR as *mut u32;
    let ahb1rstr = AHB1RSTR as *mut u32;
    let apb2_val = core::ptr::read_volatile(apb2rstr);
    let ahb1_val = core::ptr::read_volatile(ahb1rstr);
    core::ptr::write_volatile(apb2rstr, apb2_val | (1 << 26));
    core::ptr::write_volatile(ahb1rstr, ahb1_val | (1 << 23));
    cortex_m::asm::delay(168);
    core::ptr::write_volatile(apb2rstr, apb2_val & !(1 << 26));
    core::ptr::write_volatile(ahb1rstr, ahb1_val & !(1 << 23));

    const GCR: usize = 0x18;
    const SSCR: usize = 0x08;
    const BPCR: usize = 0x0C;
    const AWCR: usize = 0x10;
    const TWCR: usize = 0x14;
    const BCCR: usize = 0x2C;
    const IER: usize = 0x34;
    const SRCR: usize = 0x24;

    let h_sync = 2u32;
    let h_back_porch = 34u32;
    let h_front_porch = 34u32;
    let v_sync = 1u32;
    let v_back_porch = 15u32;
    let v_front_porch = 16u32;

    reg32_write(LTDC_BASE, GCR, (1 << 31) | (1 << 30) | (1 << 28));

    // STM32F4 LTDC timing: bits[12:0] = vertical, bits[27:16] = horizontal
    reg32_write(
        LTDC_BASE,
        SSCR,
        ((v_sync - 1) & 0xFFF) | (((h_sync - 1) & 0xFFF) << 16),
    );
    reg32_write(
        LTDC_BASE,
        BPCR,
        ((v_sync + v_back_porch - 1) & 0xFFF) | (((h_sync + h_back_porch - 1) & 0xFFF) << 16),
    );
    reg32_write(
        LTDC_BASE,
        AWCR,
        ((v_sync + v_back_porch + FB_HEIGHT as u32 - 1) & 0xFFF)
            | (((FB_WIDTH as u32 + h_sync + h_back_porch - 1) & 0xFFF) << 16),
    );
    reg32_write(
        LTDC_BASE,
        TWCR,
        ((v_sync + v_back_porch + FB_HEIGHT as u32 + v_front_porch - 1) & 0xFFF)
            | (((FB_WIDTH as u32 + h_sync + h_back_porch + h_front_porch - 1) & 0xFFF) << 16),
    );

    reg32_write(LTDC_BASE, BCCR, 0);
    reg32_write(LTDC_BASE, IER, (1 << 2) | (1 << 1));

    reg32_write(LTDC_BASE, SRCR, 0x01);
    while reg32(LTDC_BASE, SRCR) & 0x01 != 0 {}

    reg32_set(LTDC_BASE, GCR, (1 << 0) | (1 << 1)); // LTDCEN=1, bit 1 (reserved)
    let gcr_after_set = reg32(LTDC_BASE, GCR);
    #[cfg(feature = "defmt")]
    defmt::info!("LTDC GCR after set = {:08x}", gcr_after_set);
    reg32_write(LTDC_BASE, SRCR, 0x01);
}

unsafe fn ltdc_config_layer(fb_addr: u32) {
    const L1_BASE: usize = 0x84;
    const SRCR: usize = 0x24;

    let h_sync = 2u32;
    let h_back_porch = 34u32;
    let v_sync = 1u32;
    let v_back_porch = 15u32;

    let ahbp = h_sync + h_back_porch - 1;
    let avbp = v_sync + v_back_porch - 1;
    let line_length = FB_WIDTH as u32 * 2; // RGB565

    reg32_write(
        LTDC_BASE,
        L1_BASE + 0x04,
        ((ahbp + 1) & 0xFFF) | (((ahbp + FB_WIDTH as u32) & 0xFFF) << 16),
    );
    reg32_write(
        LTDC_BASE,
        L1_BASE + 0x08,
        ((avbp + 1) & 0xFFF) | (((avbp + FB_HEIGHT as u32) & 0xFFF) << 16),
    );
    reg32_write(LTDC_BASE, L1_BASE + 0x10, 0x02); // RGB565
    reg32_write(LTDC_BASE, L1_BASE + 0x14, 255); // CONSTA=255
    reg32_write(LTDC_BASE, L1_BASE + 0x18, 0xFFFF0000); // L1DCCR: opaque red
    reg32_write(LTDC_BASE, L1_BASE + 0x1C, 0x0407); // L1BFCR: BF1=constant(4), BF2=constant(7)
    reg32_write(LTDC_BASE, L1_BASE + 0x28, fb_addr);
    reg32_write(
        LTDC_BASE,
        L1_BASE + 0x2C,
        (line_length + 3) | (line_length << 16),
    );
    reg32_write(LTDC_BASE, L1_BASE + 0x30, FB_HEIGHT as u32);
    reg32_write(LTDC_BASE, L1_BASE + 0x00, 1 << 0); // LEN=1

    reg32_write(LTDC_BASE, SRCR, 0x01);
}

// ── DsiHostCtrlIo adapter (GHCR protocol) ─────────────────────────────

struct RawDsi;

impl RawDsi {
    const GHCR: usize = 0x6C;
    const GPDR: usize = 0x70;
    const GPSR: usize = 0x74;
    const ISR1: usize = 0xC4;

    unsafe fn wait_cmd_fifo_empty(&self) -> Result<(), ()> {
        for _ in 0..100_000 {
            if reg32(DSI_BASE, Self::GPSR) & (1 << 0) != 0 {
                return Ok(());
            }
        }
        Err(())
    }

    unsafe fn wait_read_not_busy(&self) -> Result<(), ()> {
        for _ in 0..100_000 {
            if reg32(DSI_BASE, Self::GPSR) & (1 << 6) == 0 {
                return Ok(());
            }
        }
        Err(())
    }

    unsafe fn wait_payload_fifo_not_empty(&self) -> Result<(), ()> {
        for _ in 0..100_000 {
            if reg32(DSI_BASE, Self::GPSR) & (1 << 4) == 0 {
                return Ok(());
            }
        }
        Err(())
    }

    unsafe fn ghcr_write(&mut self, wcmsb: u8, wclsb: u8, dt: u8) {
        let _ = self.wait_cmd_fifo_empty();
        reg32_write(
            DSI_BASE,
            Self::GHCR,
            ((wcmsb as u32) << 16) | ((wclsb as u32) << 8) | (dt as u32),
        );
    }
}

impl DsiHostCtrlIo for RawDsi {
    type Error = ();

    fn write(&mut self, cmd: DsiWriteCommand) -> Result<(), Self::Error> {
        match cmd {
            DsiWriteCommand::DcsShortP0 { arg } => unsafe {
                self.ghcr_write(0, arg, 0x05);
                Ok(())
            },
            DsiWriteCommand::DcsShortP1 { arg, data } => unsafe {
                self.ghcr_write(data, arg, 0x15);
                Ok(())
            },
            DsiWriteCommand::DcsLongWrite { arg, data } => unsafe {
                self.wait_cmd_fifo_empty()?;

                let mut fifoword = arg as u32;
                for (i, byte) in data.iter().take(3).enumerate() {
                    fifoword |= (*byte as u32) << (8 + 8 * i);
                }
                reg32_write(DSI_BASE, Self::GPDR, fifoword);

                if data.len() > 3 {
                    let mut i = 3;
                    while i + 4 <= data.len() {
                        let w: [u8; 4] = data[i..i + 4].try_into().unwrap();
                        reg32_write(DSI_BASE, Self::GPDR, u32::from_ne_bytes(w));
                        i += 4;
                    }
                    let mut fw = 0u32;
                    let mut j = 0;
                    while i < data.len() {
                        fw |= (data[i] as u32) << (j * 8);
                        i += 1;
                        j += 1;
                    }
                    if j > 0 {
                        reg32_write(DSI_BASE, Self::GPDR, fw);
                    }
                }

                let len = (data.len() + 1) as u16;
                self.ghcr_write(((len >> 8) & 0xFF) as u8, (len & 0xFF) as u8, 0x39);

                self.wait_cmd_fifo_empty()?;
                Ok(())
            },
            DsiWriteCommand::SetMaximumReturnPacketSize(len) => unsafe {
                self.ghcr_write(((len >> 8) & 0xFF) as u8, (len & 0xFF) as u8, 0x37);
                Ok(())
            },
            DsiWriteCommand::GenericShortP0 => unsafe {
                self.ghcr_write(0, 0, 0x03);
                Ok(())
            },
            DsiWriteCommand::GenericShortP1 => unsafe {
                self.ghcr_write(0, 0, 0x13);
                Ok(())
            },
            DsiWriteCommand::GenericShortP2 => unsafe {
                self.ghcr_write(0, 0, 0x23);
                Ok(())
            },
            DsiWriteCommand::GenericLongWrite { arg, data } => unsafe {
                self.wait_cmd_fifo_empty()?;

                let mut fifoword = arg as u32;
                for (i, byte) in data.iter().take(3).enumerate() {
                    fifoword |= (*byte as u32) << (8 + 8 * i);
                }
                reg32_write(DSI_BASE, Self::GPDR, fifoword);

                if data.len() > 3 {
                    let mut i = 3;
                    while i + 4 <= data.len() {
                        let w: [u8; 4] = data[i..i + 4].try_into().unwrap();
                        reg32_write(DSI_BASE, Self::GPDR, u32::from_ne_bytes(w));
                        i += 4;
                    }
                    let mut fw = 0u32;
                    let mut j = 0;
                    while i < data.len() {
                        fw |= (data[i] as u32) << (j * 8);
                        i += 1;
                        j += 1;
                    }
                    if j > 0 {
                        reg32_write(DSI_BASE, Self::GPDR, fw);
                    }
                }

                let len = (data.len() + 1) as u16;
                self.ghcr_write(((len >> 8) & 0xFF) as u8, (len & 0xFF) as u8, 0x29);

                self.wait_cmd_fifo_empty()?;
                Ok(())
            },
            _ => Ok(()),
        }
    }

    fn read(&mut self, cmd: DsiReadCommand, buf: &mut [u8]) -> Result<(), Self::Error> {
        if buf.len() > 2 && buf.len() <= 65_535 {
            self.write(DsiWriteCommand::SetMaximumReturnPacketSize(buf.len() as u16))?;
        }
        match cmd {
            DsiReadCommand::DcsShort { arg } => unsafe {
                self.ghcr_write(0, arg, 0x06);
                self.wait_read_not_busy()?;
                let mut idx = 0;
                let mut left = buf.len();
                while left > 0 {
                    self.wait_payload_fifo_not_empty()?;
                    let val = reg32(DSI_BASE, Self::GPDR);
                    let chunk = core::cmp::min(left, 4);
                    for (i, byte) in buf[idx..idx + chunk].iter_mut().enumerate() {
                        *byte = ((val >> (i * 8)) & 0xFF) as u8;
                    }
                    idx += chunk;
                    left -= chunk;
                }
                Ok(())
            },
            _ => Ok(()),
        }
    }
}

// ── Display panel detection ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcdController {
    Nt35510,
    Otm8009a,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardHint {
    Auto,
    ForceNt35510,
    ForceOtm8009a,
}

pub fn detect_panel(hint: BoardHint) -> LcdController {
    if hint == BoardHint::ForceNt35510 {
        return LcdController::Nt35510;
    }
    if hint == BoardHint::ForceOtm8009a {
        return LcdController::Otm8009a;
    }

    let mut panel = Nt35510::new();
    let mut dsi_adapter = RawDsi;
    let mut delay = BusyDelay;
    let mut mismatch_count = 0u32;
    let mut first_mismatch_id: u8 = 0;

    for _ in 0..3 {
        match panel.probe(&mut dsi_adapter, &mut delay) {
            Ok(()) => return LcdController::Nt35510,
            Err(nt35510::Error::ProbeMismatch(id)) => {
                if mismatch_count == 0 {
                    first_mismatch_id = id;
                }
                mismatch_count += 1;
            }
            Err(_) => {}
        }
        BusyDelay.delay_ms(5);
    }

    #[cfg(feature = "display")]
    if mismatch_count >= 2 && first_mismatch_id != 0 {
        let mut otm = Otm8009A::new();
        if otm.id_matches(&mut dsi_adapter).unwrap_or(false) {
            return LcdController::Otm8009a;
        }
    }

    LcdController::Nt35510
}

// ── DSI command mode helpers ───────────────────────────────────────────

unsafe fn dsi_set_lp_command_mode() {
    reg32_set(DSI_BASE, 0x41C, 1 << 22); // WPCR1: force_rx_low_power(true)
    reg32_set(
        DSI_BASE,
        0x68,
        (0x7FF << 7)
            | (1 << 18)
            | (1 << 19)
            | (1 << 20)
            | (1 << 22)
            | (1 << 23)
            | (1 << 24)
            | (1 << 25)
            | (1 << 26),
    );
}

unsafe fn dsi_set_hs_command_mode() {
    reg32_clear(DSI_BASE, 0x41C, 1 << 22); // WPCR1: force_rx_low_power(false)
    reg32_clear(
        DSI_BASE,
        0x68,
        (0x7FF << 7)
            | (1 << 18)
            | (1 << 19)
            | (1 << 20)
            | (1 << 22)
            | (1 << 23)
            | (1 << 24)
            | (1 << 25)
            | (1 << 26)
            | (1 << 0), // also clear ARE
    );
}

// ── Display init (orchestrator) ────────────────────────────────────────

pub struct DisplayCtrl {
    framebuffer: &'static mut [u16],
}

impl DisplayCtrl {
    pub fn new(
        sdram: &SdramCtrl,
        lcd_reset: embassy_stm32::Peri<'_, impl embassy_stm32::gpio::Pin>,
        hint: BoardHint,
    ) -> Self {
        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: enter");

        let mut reset_pin = embassy_stm32::gpio::Output::new(
            lcd_reset,
            embassy_stm32::gpio::Level::Low,
            embassy_stm32::gpio::Speed::Low,
        );
        BusyDelay.delay_ms(20);
        reset_pin.set_high();
        BusyDelay.delay_ms(140);
        core::mem::forget(reset_pin);

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: LCD reset done");

        unsafe {
            dsi_init();
        }

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: dsi_init done");

        let fb_slice: &'static mut [u16] = sdram.subslice_mut(0, FB_SIZE);
        let fb_addr = fb_slice.as_mut_ptr() as u32;
        unsafe {
            ltdc_init();
        }

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: ltdc_init done");

        let controller = detect_panel(hint);

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: detect_panel done");

        unsafe {
            let gcr_before_panel = reg32(LTDC_BASE, 0x18);
            #[cfg(feature = "defmt")]
            defmt::info!("DC::new: GCR before panel = {:08x}", gcr_before_panel);
            dsi_set_lp_command_mode();
        }

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: LP mode set");

        match controller {
            LcdController::Nt35510 => {
                BusyDelay.delay_ms(120);
                #[cfg(feature = "defmt")]
                defmt::info!("DC::new: starting NT35510 init");
                let mut panel = Nt35510::new();
                let mut dsi_adapter = RawDsi;
                let mut delay = BusyDelay;
                panel
                    .init_rgb565(
                        &mut dsi_adapter,
                        &mut delay,
                        nt35510::Mode::Portrait,
                        nt35510::ColorMap::Rgb,
                    )
                    .expect("NT35510 init failed");
                #[cfg(feature = "defmt")]
                defmt::info!("DC::new: init_rgb565 done");
            }
            #[cfg(feature = "display")]
            LcdController::Otm8009a => {
                use otm8009a::{ColorMap, FrameRate, Mode, Otm8009AConfig};

                BusyDelay.delay_ms(120);
                let mut panel = Otm8009A::new();
                let mut dsi_adapter = RawDsi;
                let mut delay = DelayMsAdapter;
                let config = Otm8009AConfig {
                    frame_rate: FrameRate::_60Hz,
                    mode: Mode::Landscape,
                    color_map: ColorMap::Rgb,
                    cols: FB_WIDTH,
                    rows: FB_HEIGHT,
                };
                panel
                    .init(&mut dsi_adapter, config, &mut delay)
                    .expect("OTM8009A init failed");
            }
        }

        unsafe {
            dsi_set_hs_command_mode();
            let gcr_after_panel = reg32(LTDC_BASE, 0x18);
            #[cfg(feature = "defmt")]
            defmt::info!("DC::new: GCR after panel = {:08x}", gcr_after_panel);
            reg32_set(DSI_BASE, 0x404, 1 << 2); // WCR.LTDCEN=1
            let gcr_after_wcr = reg32(LTDC_BASE, 0x18);
            #[cfg(feature = "defmt")]
            defmt::info!("DC::new: GCR after WCR.LTDCEN = {:08x}", gcr_after_wcr);
            ltdc_config_layer(fb_addr);
            let gcr_final = reg32(LTDC_BASE, 0x18);
            #[cfg(feature = "defmt")]
            defmt::info!("DC::new: GCR final = {:08x}", gcr_final);
        }

        #[cfg(feature = "defmt")]
        defmt::info!("DC::new: all done");

        DisplayCtrl {
            framebuffer: fb_slice,
        }
    }

    #[must_use]
    pub fn fb(&mut self) -> FramebufferView<'_> {
        FramebufferView {
            buffer: self.framebuffer,
        }
    }
}

/// Borrowed view into the raw RGB565 framebuffer.
pub struct FramebufferView<'a> {
    buffer: &'a mut [u16],
}

impl<'a> FramebufferView<'a> {
    pub fn clear(&mut self, color: Rgb565) {
        let raw = color.into_storage();
        for pixel in self.buffer.iter_mut() {
            *pixel = raw;
        }
    }
}

impl<'a> DrawTarget for FramebufferView<'a> {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for pixel in pixels {
            let x = pixel.0.x as usize;
            let y = pixel.0.y as usize;
            if x < FB_WIDTH as usize && y < FB_HEIGHT as usize {
                self.buffer[y * FB_WIDTH as usize + x] = pixel.1.into_storage();
            }
        }
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, color: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let top = area.top_left.y.max(0) as usize;
        let bottom = (area.top_left.y + area.size.height as i32).min(FB_HEIGHT as i32) as usize;
        let left = area.top_left.x.max(0) as usize;
        let right = (area.top_left.x + area.size.width as i32).min(FB_WIDTH as i32) as usize;

        let flat_color = color.into_iter().next().unwrap_or(Rgb565::BLACK);
        let raw = flat_color.into_storage();

        for y in top..bottom {
            for x in left..right {
                self.buffer[y * FB_WIDTH as usize + x] = raw;
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.clear(color);
        Ok(())
    }
}

impl<'a> OriginDimensions for FramebufferView<'a> {
    fn size(&self) -> Size {
        Size::new(FB_WIDTH as u32, FB_HEIGHT as u32)
    }
}
