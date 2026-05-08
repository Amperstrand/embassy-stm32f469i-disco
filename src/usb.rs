//! USB OTG FS PHY reset utilities for STM32F469I-Discovery.
//!
//! # USB PHY Reset
//!
//! After a soft reset (SYSRESETREQ from st-flash), the USB OTG FS peripheral
//! can be left in an inconsistent state where the PHY doesn't re-enumerate.
//! Cycling the RCC clock, asserting peripheral reset, performing a core soft reset,
//! and power-cycling the PHY ensures a clean start.
//!
//! Call this function **before** creating the USB driver with
//! `embassy_stm32::usb::Driver::new_fs()`.
//!
//! # Example
//!
//! ```rust,ignore
//! // Reset USB PHY before creating driver
//! embassy_stm32f469i_disco::usb::reset_usb_phy();
//!
//! // Now create USB driver
//! let driver = embassy_stm32::usb::Driver::new_fs(
//!     p.USB_OTG_FS,
//!     Irqs,
//!     p.PA12,
//!     p.PA11,
//!     ep_out_buffer,
//!     usb_config,
//! );
//! ```
//!
//! # References
//!
//! - micronuts#34, microfips#105, gm65-scanner#56 — USB PHY reset after st-flash
//! - ccid-firmware-rs#15 — Original issue report

/// Resets the USB OTG FS peripheral and PHY for clean re-enumeration.
///
/// This function performs a complete reset sequence:
/// 1. Disable USB OTG FS clock
/// 2. Assert USB OTG FS peripheral reset
/// 3. Perform core soft reset (GRSTCTL.CSRST)
/// 4. Power-cycle the PHY (GCCFG.PWRDWN)
///
/// Call this **before** creating the USB driver to ensure the PHY
/// re-enumerates correctly after st-flash soft resets.
///
/// # Safety
///
/// This function accesses USB OTG FS registers directly via raw pointers
/// at address `0x5000_0000`. This is safe because:
/// - The address is fixed by the STM32F469 silicon (RM0090 §30)
/// - No other code should have taken ownership of the USB peripheral yet
/// - All register accesses are volatile to prevent compiler reordering
///
/// The register writes are side-effect-only and do not create any data races.
pub fn reset_usb_phy() {
    let rcc = stm32_metapac::RCC;

    // 1. Disable USB OTG FS clock
    rcc.ahb2enr().modify(|w| w.set_usb_otg_fsen(false));
    cortex_m::asm::delay(100);
    rcc.ahb2enr().modify(|w| w.set_usb_otg_fsen(true));

    // 2. Assert USB OTG FS peripheral reset
    rcc.ahb2rstr().modify(|w| w.set_usb_otg_fsrst(true));
    cortex_m::asm::delay(100);
    rcc.ahb2rstr().modify(|w| w.set_usb_otg_fsrst(false));
    cortex_m::asm::delay(100);

    // 3. Core soft reset (GRSTCTL.CSRST) + PHY power cycle (GCCFG.PWRDWN)
    // USB_OTG_FS_GLOBAL base: 0x5000_0000
    // GRSTCTL offset: 0x010, GCCFG offset: 0x038
    let otg_global = 0x5000_0000usize as *mut u32;

    // SAFETY: USB OTG FS register block at 0x5000_0000 is a fixed hardware
    // address on STM32F469 (RM0090 §30). We access it before the embassy USB
    // driver takes ownership, using volatile reads/writes for register-level
    // reset sequencing. No aliasing — the driver hasn't been created yet.
    unsafe {
        // GRSTCTL.AHBIDL (bit 31) — wait for AHB idle before reset
        let mut timeout = 100_000u32;
        while otg_global.add(0x010 / 4).read_volatile() & (1 << 31) == 0 {
            timeout -= 1;
            if timeout == 0 {
                break;
            }
        }

        // GRSTCTL.CSRST (bit 0) — core soft reset, self-clearing
        otg_global.add(0x010 / 4).write_volatile(1);
        timeout = 100_000u32;
        while otg_global.add(0x010 / 4).read_volatile() & 1 != 0 {
            timeout -= 1;
            if timeout == 0 {
                break;
            }
        }

        // GCCFG.PWRDWN (bit 16) — PHY power cycle
        otg_global.add(0x038 / 4).write_volatile(0);
        cortex_m::asm::delay(100);
        otg_global.add(0x038 / 4).write_volatile(1 << 16);
    }
}
