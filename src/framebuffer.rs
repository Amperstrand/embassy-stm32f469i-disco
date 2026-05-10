//! Framebuffer view and typed pixel-slice helpers.

use embedded_graphics::{draw_target::DrawTarget, prelude::*, primitives::Rectangle};

use crate::display::{Argb8888, DisplayFormat, DisplayInitError};

pub(crate) fn framebuffer_from_bytes<F: DisplayFormat>(
    bytes: &'static mut [u8],
    len_pixels: usize,
) -> Result<&'static mut [F::Pixel], DisplayInitError> {
    let required_bytes = len_pixels
        .checked_mul(F::bpp())
        .ok_or(DisplayInitError::FramebufferSizeOverflow)?;
    if bytes.len() < required_bytes {
        return Err(DisplayInitError::FramebufferTooSmall {
            provided_bytes: bytes.len(),
            required_bytes,
        });
    }
    let ptr_addr = bytes.as_mut_ptr() as usize;
    let required_align = core::mem::align_of::<F::Pixel>();
    if !ptr_addr.is_multiple_of(required_align) {
        return Err(DisplayInitError::FramebufferMisaligned {
            ptr_addr,
            required_align,
        });
    }

    // SAFETY: bytes comes from SDRAM (aligned, non-null, exclusive access).
    // Alignment and length are checked above and return Err if violated.
    Ok(unsafe { &mut *core::ptr::slice_from_raw_parts_mut(bytes.as_mut_ptr().cast(), len_pixels) })
}

/// View over a pixel buffer implementing [`embedded_graphics::draw_target::DrawTarget`].
///
/// Created by [`DisplayCtrl::fb()`](crate::DisplayCtrl::fb). Supports
/// [`Argb8888`](crate::Argb8888) and [`Rgb565`](crate::Rgb565) pixel formats.
pub struct FramebufferView<'a, F: DisplayFormat = Argb8888> {
    buffer: &'a mut [F::Pixel],
    width: usize,
    height: usize,
}

impl<'a, F: DisplayFormat> FramebufferView<'a, F> {
    pub(crate) fn new(buffer: &'a mut [F::Pixel], width: usize, height: usize) -> Self {
        Self {
            buffer,
            width,
            height,
        }
    }

    /// Fill the entire framebuffer with a single color.
    pub fn clear(&mut self, color: F::Color) {
        let raw = F::encode(color);
        for pixel in self.buffer.iter_mut() {
            *pixel = raw;
        }
    }

    /// Fill framebuffer with 4 distinct colors (one per quarter) and verify readback.
    ///
    /// Display shows red/green/blue/yellow quarters for visual inspection.
    /// Returns mismatched pixel count (0 = pass). Useful for validating
    /// SDRAM framebuffer integrity and display alignment.
    #[cfg(feature = "defmt")]
    pub fn verify_quarter_fill(&mut self) -> usize
    where
        F::Color: embedded_graphics::pixelcolor::RgbColor,
    {
        let qh = self.height / 4;
        let colors: [F::Pixel; 4] = [
            F::encode(F::Color::RED),
            F::encode(F::Color::GREEN),
            F::encode(F::Color::BLUE),
            F::encode(F::Color::YELLOW),
        ];

        for (q, &color) in colors.iter().enumerate() {
            let start = q * qh * self.width;
            let end = if q == 3 {
                self.buffer.len()
            } else {
                (q + 1) * qh * self.width
            };
            for px in self.buffer[start..end].iter_mut() {
                *px = color;
            }
        }

        let mut mismatches = 0usize;
        for (q, &color) in colors.iter().enumerate() {
            let start = q * qh * self.width;
            let end = if q == 3 {
                self.buffer.len()
            } else {
                (q + 1) * qh * self.width
            };
            for &px in &self.buffer[start..end] {
                if px != color {
                    mismatches += 1;
                }
            }
        }
        mismatches
    }
}

impl<F: DisplayFormat> DrawTarget for FramebufferView<'_, F> {
    type Color = F::Color;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for pixel in pixels {
            let x = pixel.0.x as usize;
            let y = pixel.0.y as usize;
            if x < self.width && y < self.height {
                self.buffer[y * self.width + x] = F::encode(pixel.1);
            }
        }
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, color: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let mut colors = color.into_iter();
        let area_width = area.size.width as i32;
        let area_height = area.size.height as i32;

        for dy in 0..area_height {
            for dx in 0..area_width {
                let raw = F::encode(colors.next().unwrap_or_else(F::default_color));
                let x = area.top_left.x + dx;
                let y = area.top_left.y + dy;

                if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
                    self.buffer[y as usize * self.width + x as usize] = raw;
                }
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        FramebufferView::clear(self, color);
        Ok(())
    }
}

impl<F: DisplayFormat> OriginDimensions for FramebufferView<'_, F> {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}
