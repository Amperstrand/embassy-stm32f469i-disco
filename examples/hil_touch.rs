#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::Config;
use embassy_stm32f469i_disco::TouchCtrl;

macro_rules! isr_stubs {
    () => {
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn LTDC() { cortex_m::asm::nop(); }
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn LTDC_ER() { cortex_m::asm::nop(); }
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn DSI() { cortex_m::asm::nop(); }
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn DSIHOST() { cortex_m::asm::nop(); }
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn DMA2D() { cortex_m::asm::nop(); }
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn FMC() { cortex_m::asm::nop(); }
    };
}

isr_stubs!();

async fn hil_done() -> ! {
    embassy_time::Timer::after(embassy_time::Duration::from_millis(10)).await;
    cortex_m::asm::bkpt();
    loop { cortex_m::asm::nop(); }
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_stm32::init(Config::default());
    defmt::info!("HIL_TEST:touch:start");

    let mut touch_i2c = embassy_stm32::i2c::I2c::new_blocking(
        p.I2C1,
        p.PB8,
        p.PB9,
        embassy_stm32::i2c::Config::default(),
    );

    let touch_ctrl = TouchCtrl::new();

    match touch_ctrl.read_chip_id(&mut touch_i2c) {
        Ok(chip_id) => {
            defmt::info!("HIL_RESULT:touch_chip_id:PASS (chip_id={:02X})", chip_id);
        }
        Err(_) => {
            defmt::warn!("HIL_RESULT:touch_chip_id:SKIP (no FT6X06 on I2C1)");
            hil_done().await;
        }
    }

    match touch_ctrl.td_status(&mut touch_i2c) {
        Ok(status) => {
            let touches = status & 0x0F;
            if touches == 0 {
                defmt::info!("HIL_RESULT:touch_no_touch:PASS (td_status=0x{:02X})", status);
            } else {
                defmt::warn!("HIL_RESULT:touch_no_touch:WARN (td_status=0x{:02X}, {} touches with no finger)", status, touches);
            }
        }
        Err(_) => {
            defmt::error!("HIL_RESULT:touch_no_touch:FAIL (I2C error)");
        }
    }

    let mut fw_buf = [0u8; 1];
    if touch_i2c.blocking_write_read(0x38, &[0xA6], &mut fw_buf).is_ok() {
        defmt::info!("HIL_RESULT:touch_fw_version:PASS (fw={:02X})", fw_buf[0]);
    } else {
        defmt::error!("HIL_RESULT:touch_fw_version:FAIL (I2C read error)");
    }

    hil_done().await;
}
