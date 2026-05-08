//! SDRAM controller and FMC pin configuration for the STM32F469I-Discovery.

use embassy_stm32::gpio::{AfType, Flex, OutputType, Pull, Speed};
use embassy_stm32::rcc;
use embassy_time::{block_for, Duration};
use embedded_hal::delay::DelayNs;
use stm32_fmc::devices::is42s32400f_6::Is42s32400f6;
use stm32_fmc::{FmcPeripheral, Sdram, SdramTargetBank};

const FMC_AF12: AfType = AfType::output_pull(OutputType::PushPull, Speed::VeryHigh, Pull::Up);

struct BusyDelay;

impl DelayNs for BusyDelay {
    fn delay_ns(&mut self, ns: u32) {
        block_for(Duration::from_nanos(ns as u64));
    }
}

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

/// Total external SDRAM capacity in bytes.
pub const SDRAM_SIZE_BYTES: usize = 16 * 1024 * 1024;

/// FMC SDRAM controller for the IS42S32400F-6BL device.
pub struct SdramCtrl {
    mem: *mut u32,
}

impl SdramCtrl {
    /// Configure FMC pins and initialize external SDRAM.
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

    /// Return the SDRAM base address.
    pub fn base_address(&self) -> usize {
        self.mem as usize
    }

    /// Borrow a typed mutable slice from SDRAM at a byte offset.
    pub fn subslice_mut<T>(&self, offset_bytes: usize, len: usize) -> &'static mut [T] {
        let start = (self.mem as usize) + offset_bytes;
        let end = start + len * core::mem::size_of::<T>();
        assert!(end <= (self.mem as usize) + SDRAM_SIZE_BYTES);
        unsafe { &mut *core::ptr::slice_from_raw_parts_mut(start as *mut T, len) }
    }

    /// Run a quick destructive SDRAM smoke test over the first 4 KiB.
    #[must_use]
    pub fn test_quick(&self) -> bool {
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
