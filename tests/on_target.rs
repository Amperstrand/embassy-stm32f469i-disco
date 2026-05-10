#![no_std]
#![no_main]

use core::slice;

use defmt_rtt as _;
use embassy_stm32f469i_disco::{config_180, Board, BoardHint, SdramCtrl, SYSCLK_HZ_180};
use stm32_metapac::LTDC;

#[cfg(test)]
#[embedded_test::tests]
mod tests {
    use super::*;

    #[test]
    #[timeout(30)]
    async fn sdram_write_read_pattern() {
        let mut p = embassy_stm32::init(config_180());
        let sdram = SdramCtrl::new(&mut p, SYSCLK_HZ_180);
        let ram = unsafe { slice::from_raw_parts_mut(sdram.base_address() as *mut u32, 1024) };

        for (index, word) in ram.iter_mut().enumerate() {
            *word = 0xA5A5_0000 | (index as u32);
        }

        for (index, word) in ram.iter().enumerate() {
            assert_eq!(*word, 0xA5A5_0000 | (index as u32));
        }
    }

    #[test]
    #[timeout(30)]
    async fn display_init() {
        let p = embassy_stm32::init(config_180());
        let _board = Board::try_new(p, BoardHint::ForceNt35510).expect("board init");

        assert!(LTDC.gcr().read().ltdcen());
        assert!(LTDC.layer(0).cr().read().len());
    }

    #[test]
    #[timeout(30)]
    async fn touch_vendor_id() {
        let p = embassy_stm32::init(config_180());
        let mut board = Board::try_new(p, BoardHint::ForceNt35510).expect("board init");

        assert_eq!(board.touch.read_vendor_id().unwrap(), 0x11);
    }
}
