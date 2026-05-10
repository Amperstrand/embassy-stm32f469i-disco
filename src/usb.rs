//! USB helpers for STM32F469I-Discovery.
//!
//! # USB PHY Reset
//!
//! After a soft reset (SYSRESETREQ from st-flash), the USB OTG FS peripheral
//! can be left in an inconsistent state where the PHY doesn't re-enumerate.
//! Cycling the RCC clock, asserting peripheral reset, performing a core soft reset,
//! and power-cycling the PHY ensures a clean start.
//!
//! Call [`reset_usb_phy()`] **before** creating the USB driver with
//! `embassy_stm32::usb::Driver::new_fs()`.
//!
//! # Zero-Length Packet (ZLP) Helper
//!
//! USB bulk transfers use max-packet-size chunks. The host cannot distinguish
//! "end of transfer" from "more data coming" when the final packet is exactly
//! `max_packet_size` bytes — it will wait for another packet. Sending a
//! zero-length packet (ZLP) signals that the transfer is complete.
//!
//! [`send_with_zlp`] writes `data` in packet-sized chunks and automatically
//! appends a ZLP when the total length is a non-empty exact multiple of
//! `max_packet_size`.
//!
//! **When to use [`send_with_zlp`]:**
//! - CDC echo responses where the payload length is variable
//! - Protocol messages with known length (e.g. `"OK\r\n"` = 4 bytes)
//! - Any write where the payload size may equal a packet boundary
//!
//! **When [`CdcAcmWriter::write_packet`] alone is sufficient:**
//! - Continuous streaming where the host reads greedily (e.g. log output)
//! - Payloads guaranteed to be shorter than `max_packet_size`
//!
//! # Example
//!
//! ```rust,ignore
//! use embassy_stm32f469i_disco::{reset_usb_phy, send_with_zlp};
//!
//! // Reset USB PHY before creating driver
//! reset_usb_phy();
//!
//! // ... create CdcAcmClass ...
//!
//! // Echo with automatic ZLP handling
//! send_with_zlp(&mut class, &rx_buf[..n]).await.unwrap();
//! ```
//!
//! # References
//!
//! - micronuts#34, microfips#105, gm65-scanner#56 — USB PHY reset after st-flash
//! - USB 2.0 spec §5.8.3 — Bulk transfer termination with short packet

use embassy_usb::driver::{Driver, EndpointError};

/// Trait for USB CDC ACM packet writers.
///
/// Implemented for [`embassy_usb::class::cdc_acm::CdcAcmClass`] and
/// [`embassy_usb::class::cdc_acm::Sender`]. Use [`send_with_zlp`] with any
/// type implementing this trait.
///
/// # Implementors
///
/// | Type | Notes |
/// |------|-------|
/// | `CdcAcmClass<'_, D>` | Full CDC ACM class (read + write) |
/// | `Sender<'_, D>` | Split sender from `CdcAcmClass::split()` |
#[allow(async_fn_in_trait)]
pub trait CdcAcmWriter {
    /// Error type returned by write operations.
    type Error;

    /// Maximum packet size for the bulk IN endpoint (typically 64 for full-speed).
    /// MUST be greater than 0.
    ///
    /// # Invariant
    ///
    /// Implementations must return a non-zero value. Returning 0 is a programmer
    /// error and will trigger a debug-assertion in [`send_with_zlp`].
    fn max_packet_size(&self) -> u16;

    /// Write a single packet. `data` must be ≤ `max_packet_size()`.
    async fn write_packet(&mut self, data: &[u8]) -> Result<(), Self::Error>;
}

impl<'d, D: Driver<'d>> CdcAcmWriter for embassy_usb::class::cdc_acm::CdcAcmClass<'d, D> {
    type Error = EndpointError;

    fn max_packet_size(&self) -> u16 {
        embassy_usb::class::cdc_acm::CdcAcmClass::max_packet_size(self)
    }

    async fn write_packet(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        embassy_usb::class::cdc_acm::CdcAcmClass::write_packet(self, data).await
    }
}

impl<'d, D: Driver<'d>> CdcAcmWriter for embassy_usb::class::cdc_acm::Sender<'d, D> {
    type Error = EndpointError;

    fn max_packet_size(&self) -> u16 {
        embassy_usb::class::cdc_acm::Sender::max_packet_size(self)
    }

    async fn write_packet(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        embassy_usb::class::cdc_acm::Sender::write_packet(self, data).await
    }
}

/// Write `data` via a CDC ACM writer, sending a zero-length packet when needed.
///
/// Splits `data` into [`CdcAcmWriter::max_packet_size()`]-byte chunks and writes
/// each chunk. If `data` is non-empty and its length is an exact multiple of the
/// max packet size, an additional zero-length packet (ZLP) is sent so the host
/// processes the transfer immediately rather than waiting for more data.
///
/// # Errors
///
/// Returns the writer's error type (typically [`EndpointError::Disabled`] if
/// the USB device is not connected).
///
/// # Example
///
/// ```rust,ignore
/// send_with_zlp(&mut class, &rx_buf[..n]).await?;
/// send_with_zlp(&mut sender, b"OK\r\n").await?;
/// ```
pub async fn send_with_zlp<W: CdcAcmWriter>(writer: &mut W, data: &[u8]) -> Result<(), W::Error> {
    let max = writer.max_packet_size() as usize;
    debug_assert!(max > 0, "CdcAcmWriter::max_packet_size() returned 0; CDC endpoints must have non-zero MPS");
    let mut offset = 0;
    while offset < data.len() {
        let end = (offset + max).min(data.len());
        writer.write_packet(&data[offset..end]).await?;
        offset = end;
    }
    if !data.is_empty() && data.len().is_multiple_of(max) {
        writer.write_packet(&[]).await?;
    }
    Ok(())
}

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
