//! USB CDC stress test: rapid bidirectional data transfer without defmt.
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::usb;
use embassy_stm32::peripherals;
use embassy_time::{Duration, Ticker};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use embassy_usb::Builder;
use embassy_stm32f469i_disco::config_usb_only;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(config_usb_only());

    let mut ep_out_buffer = [0u8; 1024];
    let mut usb_config = usb::Config::default();
    usb_config.vbus_detection = false;
    let driver = usb::Driver::new_fs(
        p.USB_OTG_FS,
        Irqs,
        p.PA12,
        p.PA11,
        &mut ep_out_buffer,
        usb_config,
    );

    let mut usb_config_desc = embassy_usb::Config::new(0x16c0, 0x27dd);
    usb_config_desc.manufacturer = Some("BSP-Test");
    usb_config_desc.product = Some("STM32F469I-DISCO USB Stress");
    usb_config_desc.serial_number = Some("STRESS");

    let mut config_descriptor = [0u8; 256];
    let mut bos_descriptor = [0u8; 256];
    let mut control_buf = [0u8; 64];
    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        usb_config_desc,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [],
        &mut control_buf,
    );

    let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);
    let mut usb = builder.build();

    let usb_fut = usb.run();

    let mut led = Output::new(p.PG6, Level::Low, Speed::Low);
    let mut heartbeat = Ticker::every(Duration::from_secs(1));

    let echo_fut = async {
        class.wait_connection().await;
        defmt::info!("USB connected — starting echo");
        led.set_high();
        let mut rx_buf = [0u8; 256];
        loop {
            rx_buf.fill(0xFF);
            match embassy_futures::select::select(class.read_packet(&mut rx_buf), heartbeat.next())
                .await
            {
                embassy_futures::select::Either::First(result) => match result {
                    Ok(n) => {
                        let _ = class.write_packet(&rx_buf[..n]).await;
                    }
                    Err(EndpointError::BufferOverflow) => {}
                    Err(EndpointError::Disabled) => {
                        break;
                    }
                },
                embassy_futures::select::Either::Second(_) => {
                    led.toggle();
                }
            }
        }
    };

    embassy_futures::join::join(usb_fut, echo_fut).await;
}
