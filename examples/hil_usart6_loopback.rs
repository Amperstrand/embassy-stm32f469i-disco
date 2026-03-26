#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::Config;
use embassy_stm32::interrupt::InterruptExt;
use embassy_stm32f469i_disco::uart::UartCtrl;

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

const USART6_BASE: usize = 0x4001_1400;

async fn hil_done() -> ! {
    embassy_time::Timer::after(embassy_time::Duration::from_millis(10)).await;
    cortex_m::asm::bkpt();
    loop { cortex_m::asm::nop(); }
}

fn read_usart6_sr() -> u32 {
    unsafe { core::ptr::read_volatile(USART6_BASE as *const u32) }
}

fn clear_usart6_errors() {
    unsafe {
        let sr = core::ptr::read_volatile(USART6_BASE as *const u32);
        if sr & 0xF != 0 {
            let _ = core::ptr::read_volatile((USART6_BASE + 0x04) as *const u32);
        }
    }
}

fn check_errors(test_name: &str) -> bool {
    let sr = read_usart6_sr();
    let errors = sr & 0xF;
    if errors != 0 {
        defmt::error!("HIL_RESULT:usart6_{}:FAIL (SR={:#06X})", test_name, sr);
        clear_usart6_errors();
        false
    } else {
        defmt::info!("HIL_RESULT:usart6_{}:PASS", test_name);
        clear_usart6_errors();
        true
    }
}

async fn loopback_test(name: &str, baud: u32, data: &[u8]) -> bool {
    embassy_stm32::interrupt::USART6.disable();

    let _uart = UartCtrl::new_usart6(
        unsafe { embassy_stm32::peripherals::USART6::steal() },
        unsafe { embassy_stm32::peripherals::PG9::steal() },
        unsafe { embassy_stm32::peripherals::PG14::steal() },
        baud,
    );
    embassy_time::Timer::after(embassy_time::Duration::from_millis(1)).await;

    let mut rx_buf = [0u8; 64];
    let len = data.len();

    for (i, &byte) in data.iter().enumerate() {
        let mut tx_ok = false;
        for _ in 0..200_000u32 {
            if read_usart6_sr() & (1 << 7) != 0 {
                unsafe { core::ptr::write_volatile((USART6_BASE + 0x04) as *mut u32, byte as u32) };
                tx_ok = true;
                break;
            }
        }
        if !tx_ok {
            defmt::error!("HIL_RESULT:usart6_{}:FAIL (TX timeout at {}/{})", name, i, len);
            return false;
        }

        let mut rx_ok = false;
        for _ in 0..2_000_000u32 {
            let sr = read_usart6_sr();
            if sr & (1 << 5) != 0 {
                rx_buf[i] = unsafe { core::ptr::read_volatile((USART6_BASE + 0x04) as *const u32) } as u8;
                rx_ok = true;
                break;
            }
            if sr & 0xF != 0 {
                let _ = unsafe { core::ptr::read_volatile((USART6_BASE + 0x04) as *const u32) };
                defmt::error!("HIL_RESULT:usart6_{}:FAIL (RX error at byte {}/{}, SR={:#06X})", name, i + 1, len, sr);
                return false;
            }
        }
        if !rx_ok {
            defmt::error!("HIL_RESULT:usart6_{}:FAIL (RX timeout at byte {}/{})", name, i + 1, len);
            return false;
        }
    }

    if &rx_buf[..len] == data {
        check_errors(name)
    } else {
        defmt::error!("HIL_RESULT:usart6_{}:FAIL (data mismatch)", name);
        false
    }
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let _p = embassy_stm32::init(Config::default());
    defmt::info!("HIL_TEST:usart6:start");
    defmt::warn!("REQUIRE: Wire PG14(TX) to PG9(RX) for loopback");

    let ascii = b"HELLO_USART6";
    let protocol: &[u8] = &[0x7E, 0x00, 0x07, 0x01, 0x00, 0x0D, 0x01, 0xAB, 0xCD];

    if !loopback_test("115200_ascii", 115200, ascii).await { hil_done().await; }
    if !loopback_test("115200_protocol", 115200, protocol).await { hil_done().await; }
    if !loopback_test("9600_ascii", 9600, ascii).await { hil_done().await; }
    if !loopback_test("9600_protocol", 9600, protocol).await { hil_done().await; }
    if !loopback_test("57600_ascii", 57600, ascii).await { hil_done().await; }

    defmt::info!("HIL_TEST:usart6:ALL PASS");
    hil_done().await;
}
