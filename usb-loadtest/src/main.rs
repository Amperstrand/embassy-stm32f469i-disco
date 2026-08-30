// usb-spike: CDC echo + byte counter against upstream embassy main's
// rewritten OTG driver. Detects the F469 IN-endpoint hang (EPENA stuck)
// under sustained load. No defmt_rtt — it breaks USB CDC on this board.
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::time::Hertz;
use embassy_stm32::usb::Driver;
use embassy_stm32::{Config, bind_interrupts, peripherals, usb};
use embassy_time::Timer;
use embassy_usb::Builder;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use panic_halt as _;

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

#[embassy_executor::task]
async fn blink(mut led: Output<'static>) {
    loop {
        led.toggle();
        Timer::after_millis(500).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(8_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll_src = PllSource::Hse;
        config.rcc.pll = Some(Pll {
            prediv: PllPreDiv::Div4,
            mul: PllMul::Mul168,
            divp: Some(PllPDiv::Div2),
            divq: Some(PllQDiv::Div7),
            divr: None,
        });
        config.rcc.ahb_pre = AHBPrescaler::Div1;
        config.rcc.apb1_pre = APBPrescaler::Div4;
        config.rcc.apb2_pre = APBPrescaler::Div2;
        config.rcc.sys = Sysclk::Pll1P;
        config.rcc.mux.clk48sel = mux::Clk48sel::Pll1Q;
    }
    let p = embassy_stm32::init(config);

    let led = Output::new(p.PG6, Level::High, Speed::Low);
    if let Ok(token) = blink(led) {
        spawner.spawn(token);
    }

    let mut ep_out_buffer = [0u8; 256];
    let mut drv_config = embassy_stm32::usb::Config::default();
    drv_config.vbus_detection = false;
    let driver = Driver::new_fs(p.USB_OTG_FS, Irqs, p.PA12, p.PA11, &mut ep_out_buffer, drv_config);

    let mut usb_config = embassy_usb::Config::new(0xc0de, 0xcafe);
    usb_config.manufacturer = Some("Amperstrand");
    usb_config.product = Some("usb-spike embassy-main");
    usb_config.serial_number = Some("f469-spike");

    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    let mut state = State::new();
    let mut builder = Builder::new(
        driver,
        usb_config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [],
        &mut control_buf,
    );
    let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);
    let mut usb = builder.build();

    let usb_fut = usb.run();
    let echo_fut = async {
        let mut total: u64 = 0;
        let mut next_report: u64 = 64 * 1024;
        let mut buf = [0u8; 64];
        loop {
            class.wait_connection().await;
            loop {
                match class.read_packet(&mut buf).await {
                    Ok(n) => {
                        if let Err(_) = class.write_packet(&buf[..n]).await {
                            break;
                        }
                        total += n as u64;
                        if total >= next_report {
                            let mut line = [0u8; 40];
                            let mut len = 0;
                            for b in b"SPIKE bytes=" {
                                line[len] = *b;
                                len += 1;
                            }
                            let mut v = next_report;
                            let mut digits = [0u8; 20];
                            let mut dn = 0;
                            while v > 0 {
                                digits[dn] = b'0' + (v % 10) as u8;
                                dn += 1;
                                v /= 10;
                            }
                            while dn > 0 {
                                dn -= 1;
                                line[len] = digits[dn];
                                len += 1;
                            }
                            line[len] = b'\n';
                            len += 1;
                            if class.write_packet(&line[..len]).await.is_err() {
                                break;
                            }
                            next_report += 64 * 1024;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    };

    join(usb_fut, echo_fut).await;
}
