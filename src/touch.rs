//! FT6X06 capacitive touch controller driver.
//!
//! The FT6X06 is connected via I2C1 at address 0x38 (PB8=SCL, PB9=SDA).
//! It is powered from the display module — SDRAM and display must be initialized
//! before touch works.
//!
//! # Phantom touches
//!
//! The FT6X06 reports phantom touches at screen edges (x=0, y=445, x=479, y=767)
//! due to electrical noise. Consumers should filter edge touches:
//!
//! ```rust,ignore
//! if x < 3 || x > 476 || y < 3 || y > 796 {
//!     return None; // reject edge touches
//! }
//! ```

use embassy_stm32::i2c;

const FT6X06_ADDR: u8 = 0x38;
const REG_TD_STATUS: u8 = 0x02;
const REG_TOUCH1_XH: u8 = 0x03;

/// A single touch coordinate from the FT6X06.
///
/// X ranges 0..479, Y ranges 0..799. Phantom touches may appear at edges —
/// filter with a 3px margin (see module docs).
pub struct TouchPoint {
    /// X coordinate of the touch event (0..479).
    pub x: u16,
    /// Y coordinate of the touch event (0..799).
    pub y: u16,
}

/// Driver for the FT6X06 capacitive touch controller over I2C.
///
/// I2C address: 0x38. The controller is powered from the display module,
/// so SDRAM + display must be initialized before touch works.
pub struct TouchCtrl {
    i2c_addr: u8,
}

impl TouchCtrl {
    /// Create a new FT6X06 driver with the default I2C address (0x38).
    pub fn new() -> Self {
        Self {
            i2c_addr: FT6X06_ADDR,
        }
    }

    /// Read the touch detect status register (0x02).
    ///
    /// Returns the lower 4 bits: number of currently detected touch points (0..6).
    pub fn td_status(
        &self,
        i2c: &mut i2c::I2c<'_, embassy_stm32::mode::Blocking, i2c::Master>,
    ) -> Result<u8, ()> {
        let mut buf = [0u8; 1];
        i2c.blocking_write_read(self.i2c_addr, &[REG_TD_STATUS], &mut buf)
            .map_err(|_| ())?;
        Ok(buf[0] & 0x0F)
    }

    /// Read the first touch point coordinates from registers 0x03..0x06.
    ///
    /// Returns a `TouchPoint` with X (0..479) and Y (0..799).
    pub fn get_touch(
        &self,
        i2c: &mut i2c::I2c<'_, embassy_stm32::mode::Blocking, i2c::Master>,
    ) -> Result<TouchPoint, ()> {
        let mut buf = [0u8; 4];
        i2c.blocking_write_read(self.i2c_addr, &[REG_TOUCH1_XH], &mut buf)
            .map_err(|_| ())?;

        let x = (((buf[0] & 0x0F) as u16) << 8) | (buf[1] as u16);
        let y = (((buf[2] & 0x0F) as u16) << 8) | (buf[3] as u16);
        Ok(TouchPoint { x, y })
    }

    /// Read the FT6X06 vendor ID from register 0xA8.
    ///
    /// Returns 0x11 for all FocalTech FT62XX family chips (not chip-specific).
    pub fn read_vendor_id(
        &self,
        i2c: &mut i2c::I2c<'_, embassy_stm32::mode::Blocking, i2c::Master>,
    ) -> Result<u8, ()> {
        let mut buf = [0u8; 1];
        i2c.blocking_write_read(self.i2c_addr, &[0xA8], &mut buf)
            .map_err(|_| ())?;
        Ok(buf[0])
    }

    /// Read the FT6X06 chip model ID from register 0xA3.
    ///
    /// Known values: FT6206=0x06, FT6236=0x36, FT6236U=0x64, FT6336U=0x64.
    pub fn read_chip_model(
        &self,
        i2c: &mut i2c::I2c<'_, embassy_stm32::mode::Blocking, i2c::Master>,
    ) -> Result<u8, ()> {
        let mut buf = [0u8; 1];
        i2c.blocking_write_read(self.i2c_addr, &[0xA3], &mut buf)
            .map_err(|_| ())?;
        Ok(buf[0])
    }
}
