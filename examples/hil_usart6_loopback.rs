#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::Config;
use embassy_stm32::interrupt::InterruptExt;
use embassy_stm32f469i_disco::uart::UartCtrl;
use nb::block;

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
        #[allow(non_snake_case)]
        #[no_mangle]
        unsafe extern "C" fn USART6() { cortex_m::asm::nop(); }
    };
}

isr_stubs!();

const TEST_DATA: &[u8] = b"HIL_USART6_LOOPBACK_OK!";

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_stm32::init(Config::default());
    defmt::info!("HIL_TEST:usart6_loopback:start");
    defmt::warn!("REQUIRE: Wire PG14(TX) to PG9(RX) for loopback");

    embassy_stm32::interrupt::USART6.disable();

    let mut uart = UartCtrl::new_usart6(p.USART6, p.PG9, p.PG14, 115200);

    embassy_time::Timer::after(embassy_time::Duration::from_millis(1)).await;

    let mut rx_buf = [0u8; 32];

    for (i, &byte) in TEST_DATA.iter().enumerate() {
        block!(uart.write_byte(byte)).ok();
        match block!(uart.read_byte()) {
            Ok(received) => {
                rx_buf[i] = received;
            }
            Err(_) => {
                defmt::error!("HIL_RESULT:usart6_loopback:FAIL (RX error at byte {}/{})", i, TEST_DATA.len());
                embassy_time::Timer::after(embassy_time::Duration::from_millis(10)).await;
                cortex_m::asm::bkpt();
                loop { cortex_m::asm::nop(); }
            }
        }
    }

    block!(uart.flush()).ok();

    if &rx_buf[..TEST_DATA.len()] == TEST_DATA {
        defmt::info!("HIL_RESULT:usart6_loopback:PASS ({} bytes verified)", TEST_DATA.len());
    } else {
        defmt::error!("HIL_RESULT:usart6_loopback:FAIL (data mismatch)");
    }

    embassy_time::Timer::after(embassy_time::Duration::from_millis(10)).await;
    cortex_m::asm::bkpt();
    loop { cortex_m::asm::nop(); }
}
