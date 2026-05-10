//! SDRAM controller for the STM32F469I-Discovery board.
//!
//! Uses embassy-stm32's type-safe FMC API to configure the IS42S32400F-6BL SDRAM
//! on Bank 1 (0xC000_0000, 16 MiB, 32-bit data bus). All FMC pins are configured
//! by the embassy FMC driver — no manual `unsafe` pin setup required.

use embassy_stm32::fmc::Fmc;
use embassy_stm32::peripherals;
use embassy_stm32::Peri;
use embassy_time::{block_for, Duration};
use embedded_hal::delay::DelayNs;
use stm32_fmc::devices::is42s32400f_6::Is42s32400f6;
use stm32_fmc::Sdram;

struct BusyDelay;

impl DelayNs for BusyDelay {
    fn delay_ns(&mut self, ns: u32) {
        block_for(Duration::from_nanos(ns as u64));
    }
}

/// Initialize SDRAM using the STM32F469I-Discovery board pin mapping.
///
/// Extracts all FMC pins from `Peripherals` and passes them to [`SdramCtrl::new`].
/// Use this in examples and tests instead of calling [`SdramCtrl::new`] directly.
#[macro_export]
macro_rules! sdram_init {
    ($p:ident) => {{
        $crate::SdramCtrl::new(
            $p.FMC, // Address (A0–A11)
            $p.PF0, $p.PF1, $p.PF2, $p.PF3, $p.PF4, $p.PF5, $p.PF12, $p.PF13, $p.PF14, $p.PF15,
            $p.PG0, $p.PG1, // Bank address (BA0, BA1)
            $p.PG4, $p.PG5, // Data (D0–D31)
            $p.PD14, $p.PD15, $p.PD0, $p.PD1, $p.PE7, $p.PE8, $p.PE9, $p.PE10, $p.PE11, $p.PE12,
            $p.PE13, $p.PE14, $p.PE15, $p.PD8, $p.PD9, $p.PD10, $p.PH8, $p.PH9, $p.PH10, $p.PH11,
            $p.PH12, $p.PH13, $p.PH14, $p.PH15, $p.PI0, $p.PI1, $p.PI2, $p.PI3, $p.PI6, $p.PI7,
            $p.PI9, $p.PI10, // Byte lane (NBL0–NBL3)
            $p.PE0, $p.PE1, $p.PI4, $p.PI5,
            // Control (SDCKE0, SDCLK, SDNCAS, SDNE0, SDNRAS, SDNWE)
            $p.PH2, $p.PG8, $p.PG15, $p.PH3, $p.PF11, $p.PC0,
        )
    }};
}

/// Total external SDRAM capacity in bytes.
pub const SDRAM_SIZE_BYTES: usize = 16 * 1024 * 1024;

/// FMC SDRAM controller for the IS42S32400F-6BL device.
///
/// The SDRAM is initialized on Bank 1 at 0xC000_0000 with 12-bit address,
/// 32-bit data bus, 4 internal banks.
pub struct SdramCtrl {
    mem: *mut u32,
}

impl SdramCtrl {
    /// Configure FMC pins and initialize external SDRAM.
    ///
    /// Takes ownership of the FMC peripheral and all required pins.
    /// Embassy's type-safe FMC driver configures each pin to AF12 (PushPull, VeryHigh, PullUp)
    /// inside a critical section.
    ///
    /// The pin types enforce compile-time correctness: only pins that implement
    /// the correct FMC trait for this chip (STM32F469NI) are accepted.
    #[expect(clippy::too_many_arguments, reason = "matches embassy FMC constructor")]
    pub fn new(
        fmc: Peri<'_, peripherals::FMC>,
        a0: Peri<'_, impl embassy_stm32::fmc::A0Pin<peripherals::FMC>>,
        a1: Peri<'_, impl embassy_stm32::fmc::A1Pin<peripherals::FMC>>,
        a2: Peri<'_, impl embassy_stm32::fmc::A2Pin<peripherals::FMC>>,
        a3: Peri<'_, impl embassy_stm32::fmc::A3Pin<peripherals::FMC>>,
        a4: Peri<'_, impl embassy_stm32::fmc::A4Pin<peripherals::FMC>>,
        a5: Peri<'_, impl embassy_stm32::fmc::A5Pin<peripherals::FMC>>,
        a6: Peri<'_, impl embassy_stm32::fmc::A6Pin<peripherals::FMC>>,
        a7: Peri<'_, impl embassy_stm32::fmc::A7Pin<peripherals::FMC>>,
        a8: Peri<'_, impl embassy_stm32::fmc::A8Pin<peripherals::FMC>>,
        a9: Peri<'_, impl embassy_stm32::fmc::A9Pin<peripherals::FMC>>,
        a10: Peri<'_, impl embassy_stm32::fmc::A10Pin<peripherals::FMC>>,
        a11: Peri<'_, impl embassy_stm32::fmc::A11Pin<peripherals::FMC>>,
        ba0: Peri<'_, impl embassy_stm32::fmc::BA0Pin<peripherals::FMC>>,
        ba1: Peri<'_, impl embassy_stm32::fmc::BA1Pin<peripherals::FMC>>,
        d0: Peri<'_, impl embassy_stm32::fmc::D0Pin<peripherals::FMC>>,
        d1: Peri<'_, impl embassy_stm32::fmc::D1Pin<peripherals::FMC>>,
        d2: Peri<'_, impl embassy_stm32::fmc::D2Pin<peripherals::FMC>>,
        d3: Peri<'_, impl embassy_stm32::fmc::D3Pin<peripherals::FMC>>,
        d4: Peri<'_, impl embassy_stm32::fmc::D4Pin<peripherals::FMC>>,
        d5: Peri<'_, impl embassy_stm32::fmc::D5Pin<peripherals::FMC>>,
        d6: Peri<'_, impl embassy_stm32::fmc::D6Pin<peripherals::FMC>>,
        d7: Peri<'_, impl embassy_stm32::fmc::D7Pin<peripherals::FMC>>,
        d8: Peri<'_, impl embassy_stm32::fmc::D8Pin<peripherals::FMC>>,
        d9: Peri<'_, impl embassy_stm32::fmc::D9Pin<peripherals::FMC>>,
        d10: Peri<'_, impl embassy_stm32::fmc::D10Pin<peripherals::FMC>>,
        d11: Peri<'_, impl embassy_stm32::fmc::D11Pin<peripherals::FMC>>,
        d12: Peri<'_, impl embassy_stm32::fmc::D12Pin<peripherals::FMC>>,
        d13: Peri<'_, impl embassy_stm32::fmc::D13Pin<peripherals::FMC>>,
        d14: Peri<'_, impl embassy_stm32::fmc::D14Pin<peripherals::FMC>>,
        d15: Peri<'_, impl embassy_stm32::fmc::D15Pin<peripherals::FMC>>,
        d16: Peri<'_, impl embassy_stm32::fmc::D16Pin<peripherals::FMC>>,
        d17: Peri<'_, impl embassy_stm32::fmc::D17Pin<peripherals::FMC>>,
        d18: Peri<'_, impl embassy_stm32::fmc::D18Pin<peripherals::FMC>>,
        d19: Peri<'_, impl embassy_stm32::fmc::D19Pin<peripherals::FMC>>,
        d20: Peri<'_, impl embassy_stm32::fmc::D20Pin<peripherals::FMC>>,
        d21: Peri<'_, impl embassy_stm32::fmc::D21Pin<peripherals::FMC>>,
        d22: Peri<'_, impl embassy_stm32::fmc::D22Pin<peripherals::FMC>>,
        d23: Peri<'_, impl embassy_stm32::fmc::D23Pin<peripherals::FMC>>,
        d24: Peri<'_, impl embassy_stm32::fmc::D24Pin<peripherals::FMC>>,
        d25: Peri<'_, impl embassy_stm32::fmc::D25Pin<peripherals::FMC>>,
        d26: Peri<'_, impl embassy_stm32::fmc::D26Pin<peripherals::FMC>>,
        d27: Peri<'_, impl embassy_stm32::fmc::D27Pin<peripherals::FMC>>,
        d28: Peri<'_, impl embassy_stm32::fmc::D28Pin<peripherals::FMC>>,
        d29: Peri<'_, impl embassy_stm32::fmc::D29Pin<peripherals::FMC>>,
        d30: Peri<'_, impl embassy_stm32::fmc::D30Pin<peripherals::FMC>>,
        d31: Peri<'_, impl embassy_stm32::fmc::D31Pin<peripherals::FMC>>,
        nbl0: Peri<'_, impl embassy_stm32::fmc::NBL0Pin<peripherals::FMC>>,
        nbl1: Peri<'_, impl embassy_stm32::fmc::NBL1Pin<peripherals::FMC>>,
        nbl2: Peri<'_, impl embassy_stm32::fmc::NBL2Pin<peripherals::FMC>>,
        nbl3: Peri<'_, impl embassy_stm32::fmc::NBL3Pin<peripherals::FMC>>,
        sdcke: Peri<'_, impl embassy_stm32::fmc::SDCKE0Pin<peripherals::FMC>>,
        sdclk: Peri<'_, impl embassy_stm32::fmc::SDCLKPin<peripherals::FMC>>,
        sdncas: Peri<'_, impl embassy_stm32::fmc::SDNCASPin<peripherals::FMC>>,
        sdne: Peri<'_, impl embassy_stm32::fmc::SDNE0Pin<peripherals::FMC>>,
        sdnras: Peri<'_, impl embassy_stm32::fmc::SDNRASPin<peripherals::FMC>>,
        sdnwe: Peri<'_, impl embassy_stm32::fmc::SDNWEPin<peripherals::FMC>>,
    ) -> Self {
        let mut sdram: Sdram<Fmc<'_, peripherals::FMC>, Is42s32400f6> =
            Fmc::sdram_a12bits_d32bits_4banks_bank1(
                fmc,
                a0,
                a1,
                a2,
                a3,
                a4,
                a5,
                a6,
                a7,
                a8,
                a9,
                a10,
                a11,
                ba0,
                ba1,
                d0,
                d1,
                d2,
                d3,
                d4,
                d5,
                d6,
                d7,
                d8,
                d9,
                d10,
                d11,
                d12,
                d13,
                d14,
                d15,
                d16,
                d17,
                d18,
                d19,
                d20,
                d21,
                d22,
                d23,
                d24,
                d25,
                d26,
                d27,
                d28,
                d29,
                d30,
                d31,
                nbl0,
                nbl1,
                nbl2,
                nbl3,
                sdcke,
                sdclk,
                sdncas,
                sdne,
                sdnras,
                sdnwe,
                Is42s32400f6 {},
            );
        let mut delay = BusyDelay;
        let mem = sdram.init(&mut delay);
        SdramCtrl { mem }
    }

    /// Return the SDRAM base address.
    pub fn base_address(&self) -> usize {
        self.mem as usize
    }

    fn into_slice<T>(self) -> &'static mut [T] {
        let len = SDRAM_SIZE_BYTES / core::mem::size_of::<T>();
        // SAFETY: self.mem points to the full 16MB SDRAM region. The slice length
        // is computed from SDRAM_SIZE_BYTES which matches the hardware size.
        unsafe { &mut *core::ptr::slice_from_raw_parts_mut(self.mem.cast::<T>(), len) }
    }

    /// Consume the controller and yield the full SDRAM region as a raw `u16` slice.
    ///
    /// This is the entire 16 MiB SDRAM region — not a framebuffer view.
    /// Callers are responsible for sub-slicing into framebuffers and any DMA/cache coherency.
    ///
    /// This is a one-shot operation. The SDRAM is not partitionable after this call.
    /// If multiple regions are needed, a future API may add `into_partitions()`.
    ///
    /// # Safety
    /// The returned slice covers the entire SDRAM region. The caller must ensure
    /// no other code aliases this memory.
    ///
    /// ```compile_fail
    /// # use embassy_stm32f469i_disco::SdramCtrl;
    /// # fn demo(sdram: SdramCtrl) {
    /// let fb1 = sdram.into_raw_slice();
    /// let fb2 = sdram.into_raw_slice(); // ERROR: use of moved value
    /// # let _ = (fb1, fb2);
    /// # }
    /// ```
    #[must_use]
    pub fn into_raw_slice(self) -> &'static mut [u16] {
        self.into_slice()
    }

    /// Consume the controller and yield the full SDRAM region as raw bytes.
    #[must_use]
    pub fn into_bytes(self) -> &'static mut [u8] {
        self.into_slice()
    }

    /// Run a quick destructive SDRAM smoke test over the first 4 KiB.
    #[must_use]
    pub fn test_quick(&mut self) -> bool {
        // SAFETY: &mut self guarantees exclusive access; no other live &mut [u32] view exists.
        // Pointer and length come from the FMC mapping established in SdramCtrl::new.
        // 1024 u32 words = 4 KiB, well within the 16MB SDRAM region.
        let words = unsafe { core::slice::from_raw_parts_mut(self.mem, 1024) };
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
