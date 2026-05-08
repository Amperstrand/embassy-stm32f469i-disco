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

use core::fmt;

use embedded_hal::i2c::I2c;

const FT6X06_ADDR: u8 = 0x38;
const REG_TD_STATUS: u8 = 0x02;
const REG_TOUCH1_XH: u8 = 0x03;

/// Error type for FT6X06 touch controller operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TouchError<E> {
    /// I2C bus error (NACK, timeout, bus error, etc.).
    I2c(E),
    /// I2C read succeeded but the response was unexpected (e.g. invalid vendor ID).
    InvalidResponse,
}

impl<E: fmt::Debug> fmt::Display for TouchError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I2c(e) => write!(f, "I2C error: {:?}", e),
            Self::InvalidResponse => write!(f, "invalid response from FT6X06"),
        }
    }
}

impl<E> From<E> for TouchError<E> {
    fn from(e: E) -> Self {
        TouchError::I2c(e)
    }
}

/// A single touch coordinate from the FT6X06.
///
/// X ranges 0..479, Y ranges 0..799. Phantom touches may appear at edges —
/// filter with a 3px margin (see module docs).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TouchPoint {
    /// X coordinate of the touch event (0..479).
    pub x: u16,
    /// Y coordinate of the touch event (0..799).
    pub y: u16,
}

impl fmt::Display for TouchPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

/// Default I2C type: embassy-stm32 blocking I2C master.
type DefaultI2c =
    embassy_stm32::i2c::I2c<'static, embassy_stm32::mode::Blocking, embassy_stm32::i2c::Master>;

/// Driver for the FT6X06 capacitive touch controller over I2C.
///
/// Generic over any [`I2c`] implementation. The default type is embassy-stm32's
/// blocking I2C master, so `TouchCtrl` can be used without type parameters when
/// using the default I2C configuration.
///
/// I2C address: 0x38. The controller is powered from the display module,
/// so SDRAM + display must be initialized before touch works.
///
/// # Examples
///
/// ```rust,ignore
/// let i2c = embassy_stm32::i2c::I2c::new_blocking(
///     p.I2C1, p.PB8, p.PB9, i2c::Config::default(),
/// );
/// let mut touch = TouchCtrl::new(i2c);
/// if touch.td_status().unwrap_or(0) > 0 {
///     if let Ok(point) = touch.get_touch() {
///         info!("Touch: {}", point);
///     }
/// }
/// ```
pub struct TouchCtrl<I2C = DefaultI2c>
where
    I2C: I2c,
{
    i2c: I2C,
    i2c_addr: u8,
}

impl<I2C: I2c> TouchCtrl<I2C> {
    /// Create a new FT6X06 driver with the given I2C instance and default address (0x38).
    pub fn new(i2c: I2C) -> Self {
        Self {
            i2c,
            i2c_addr: FT6X06_ADDR,
        }
    }

    /// Read the touch detect status register (0x02).
    ///
    /// Returns the lower 4 bits: number of currently detected touch points (0..6).
    pub fn td_status(&mut self) -> Result<u8, TouchError<I2C::Error>> {
        let mut buf = [0u8; 1];
        self.i2c
            .write_read(self.i2c_addr, &[REG_TD_STATUS], &mut buf)?;
        Ok(buf[0] & 0x0F)
    }

    /// Read the first touch point coordinates from registers 0x03..0x06.
    ///
    /// Returns a [`TouchPoint`] with X (0..479) and Y (0..799).
    pub fn get_touch(&mut self) -> Result<TouchPoint, TouchError<I2C::Error>> {
        let mut buf = [0u8; 4];
        self.i2c
            .write_read(self.i2c_addr, &[REG_TOUCH1_XH], &mut buf)?;

        let x = (((buf[0] & 0x0F) as u16) << 8) | (buf[1] as u16);
        let y = (((buf[2] & 0x0F) as u16) << 8) | (buf[3] as u16);
        Ok(TouchPoint { x, y })
    }

    /// Read the FT6X06 vendor ID from register 0xA8.
    ///
    /// Returns 0x11 for all FocalTech FT62XX family chips (not chip-specific).
    pub fn read_vendor_id(&mut self) -> Result<u8, TouchError<I2C::Error>> {
        let mut buf = [0u8; 1];
        self.i2c.write_read(self.i2c_addr, &[0xA8], &mut buf)?;
        Ok(buf[0])
    }

    /// Read the FT6X06 chip model ID from register 0xA3.
    ///
    /// Known values: FT6206=0x06, FT6236=0x36, FT6236U=0x64, FT6336U=0x64.
    pub fn read_chip_model(&mut self) -> Result<u8, TouchError<I2C::Error>> {
        let mut buf = [0u8; 1];
        self.i2c.write_read(self.i2c_addr, &[0xA3], &mut buf)?;
        Ok(buf[0])
    }
}
