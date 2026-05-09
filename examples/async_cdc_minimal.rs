//! Minimal USB CDC echo example without defmt (production-safe).
#![no_main]
#![no_std]

extern crate alloc;

use panic_halt as _;

#[cfg(feature = "defmt")]
mod defmt_stubs {
    #[no_mangle]
    unsafe extern "C" fn _defmt_acquire() -> usize {
        0
    }
    #[no_mangle]
    unsafe extern "C" fn _defmt_write(_data: *const u8, _len: usize) {}
    #[no_mangle]
    unsafe extern "C" fn _defmt_release(_addr: usize) {}
}

use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::{interrupt::InterruptExt, peripherals, usb};
use embassy_time::{Duration, Ticker, Timer};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::Builder;

use embassy_stm32f469i_disco::{config_168, display::SdramCtrl, SYSCLK_HZ_168};

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

#[global_allocator]
static ALLOCATOR: linked_list_allocator::LockedHeap = linked_list_allocator::LockedHeap::empty();
static mut HEAP_MEMORY: [u8; 64 * 1024] = [0; 64 * 1024];

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    unsafe {
        ALLOCATOR
            .lock()
            .init(core::ptr::addr_of_mut!(HEAP_MEMORY) as *mut u8, 64 * 1024);
    }

    let mut p = embassy_stm32::init(config_168());

    embassy_stm32f469i_disco::reset_usb_phy();

    let mut sdram = SdramCtrl::new(&mut p, SYSCLK_HZ_168);
    let _sdram_base = sdram.base_address();
    let _sdram_ok = sdram.test_quick();
    let framebuffer = sdram.into_bytes();

    let display = embassy_stm32f469i_disco::DisplayCtrl::new(
        framebuffer,
        p.LTDC,
        p.DSIHOST,
        p.PJ2,
        p.PH7,
        embassy_stm32f469i_disco::BoardHint::Auto,
    );
    let _ = display;

    embassy_stm32::interrupt::USART6.disable();
    let mut uart_config = embassy_stm32::usart::Config::default();
    uart_config.baudrate = 115200;
    let _uart =
        embassy_stm32::usart::Uart::new_blocking(p.USART6, p.PG9, p.PG14, uart_config).unwrap();

    let i2c_config = embassy_stm32::i2c::Config::default();
    let _touch_i2c = embassy_stm32::i2c::I2c::new_blocking(p.I2C2, p.PB10, p.PB11, i2c_config);

    let mut led = Output::new(p.PG6, Level::Low, Speed::Low);

    let mut ep_out_buffer = [0u8; 256];
    let mut usb_config = usb::Config::default();
    usb_config.vbus_detection = false;
    let usb_driver = usb::Driver::new_fs(
        p.USB_OTG_FS,
        Irqs,
        p.PA12,
        p.PA11,
        &mut ep_out_buffer,
        usb_config,
    );

    let mut usb_config_desc = embassy_usb::Config::new(0xc0de, 0xcafe);
    usb_config_desc.manufacturer = Some("stm32f469i-disco");
    usb_config_desc.product = Some("CDC+I2Conly");
    usb_config_desc.serial_number = Some("f469test");

    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut msos_descriptor = [0; 256];
    let mut control_buf = [0; 64];

    let mut usb_state = State::new();
    let mut usb_builder = Builder::new(
        usb_driver,
        usb_config_desc,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
    );
    let mut cdc = CdcAcmClass::new(&mut usb_builder, &mut usb_state, 64);
    let mut usb_dev = usb_builder.build();

    let usb_task = async {
        usb_dev.run().await;
    };

    let cdc_task = async {
        loop {
            cdc.wait_connection().await;

            let mut rx_buf = [0u8; 256];
            let mut heartbeat = Ticker::every(Duration::from_secs(5));
            loop {
                match cdc.read_packet(&mut rx_buf).await {
                    Ok(n) if n > 0 => {
                        let mut tx = [0u8; 260];
                        tx[0] = b'E';
                        tx[1] = b'C';
                        tx[2] = b'H';
                        tx[3] = b'O';
                        tx[4] = b':';
                        let copy_len = n.min(254);
                        tx[5..copy_len + 5].copy_from_slice(&rx_buf[..copy_len]);
                        if cdc.write_packet(&tx[..copy_len + 5]).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {
                        heartbeat.next().await;
                        if cdc.write_packet(b"ALIVE\n").await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            Timer::after(Duration::from_millis(100)).await;
        }
    };

    let led_task = async {
        loop {
            led.set_high();
            Timer::after(Duration::from_millis(500)).await;
            led.set_low();
            Timer::after(Duration::from_millis(500)).await;
        }
    };

    embassy_futures::join::join3(usb_task, cdc_task, led_task).await;
}
