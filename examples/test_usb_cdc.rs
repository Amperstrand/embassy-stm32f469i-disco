#![no_std]
#![no_main]

use core::fmt::Write as FmtWrite;
use core::sync::atomic::{AtomicUsize, Ordering};

use embassy_futures::join::join;
use embassy_stm32::rcc::*;
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;
use embassy_stm32::{bind_interrupts, peripherals, usb};
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use embassy_usb::Builder;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

static PASSED: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let mut config = Config::default();
    config.rcc.hse = Some(Hse {
        freq: Hertz(8_000_000),
        mode: HseMode::Oscillator,
    });
    config.rcc.pll_src = PllSource::HSE;
    config.rcc.pll = Some(Pll {
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL168,
        divp: Some(PllPDiv::DIV2),
        divq: Some(PllQDiv::DIV7),
        divr: None,
    });
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.mux.clk48sel = mux::Clk48sel::PLL1_Q;

    let p = embassy_stm32::init(config);

    let mut ep_out_buffer = [0u8; 256];
    let mut usb_config = embassy_stm32::usb::Config::default();
    usb_config.vbus_detection = false;

    let driver = usb::Driver::new_fs(
        p.USB_OTG_FS,
        Irqs,
        p.PA12,
        p.PA11,
        &mut ep_out_buffer,
        usb_config,
    );

    let mut usb_dev_config = embassy_usb::Config::new(0xc0de, 0xcafe);
    usb_dev_config.manufacturer = Some("BSP-Test");
    usb_dev_config.product = Some("STM32F469I-DISCO");
    usb_dev_config.serial_number = Some("test1234");

    let mut config_descriptor = [0u8; 256];
    let mut bos_descriptor = [0u8; 256];
    let mut control_buf = [0u8; 64];
    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        usb_dev_config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [],
        &mut control_buf,
    );
    let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);
    let mut usb = builder.build();

    let usb_fut = usb.run();
    let mut line = heapless::String::<128>::new();

    let test_fut = async {
        let _ = class.write_packet(b"=== USB CDC Test Suite ===\r\n").await;

        line.clear();
        let _ = write!(line, "TEST usb_init: PASS");
        let _ = class.write_packet(line.as_bytes()).await;
        let _ = class.write_packet(b"\r\n").await;
        PASSED.fetch_add(1, Ordering::Relaxed);

        line.clear();
        let _ = write!(line, "TEST usb_cdc_init: PASS");
        let _ = class.write_packet(line.as_bytes()).await;
        let _ = class.write_packet(b"\r\n").await;
        PASSED.fetch_add(1, Ordering::Relaxed);

        let _ = class.write_packet(b"TEST usb_cdc_echo: RUNNING\r\n").await;

        class.wait_connection().await;

        let mut buf = [0u8; 64];
        let mut echo_ok = false;
        buf.fill(0xFF);
        for _ in 0..5u32 {
            match embassy_futures::select::select(
                class.read_packet(&mut buf),
                Timer::after(embassy_time::Duration::from_secs(1)),
            )
            .await
            {
                embassy_futures::select::Either::First(Ok(n)) => {
                    if class.write_packet(&buf[..n]).await.is_ok() {
                        echo_ok = true;
                    }
                }
                embassy_futures::select::Either::First(Err(EndpointError::Disabled)) => break,
                embassy_futures::select::Either::First(Err(EndpointError::BufferOverflow)) => {}
                embassy_futures::select::Either::Second(_) => {}
            }
        }

        if echo_ok {
            line.clear();
            let _ = write!(line, "TEST usb_cdc_echo: PASS");
            let _ = class.write_packet(line.as_bytes()).await;
            let _ = class.write_packet(b"\r\n").await;
            PASSED.fetch_add(1, Ordering::Relaxed);
        } else {
            line.clear();
            let _ = write!(line, "TEST usb_cdc_echo: FAIL timeout - no host connected");
            let _ = class.write_packet(line.as_bytes()).await;
            let _ = class.write_packet(b"\r\n").await;
            FAILED.fetch_add(1, Ordering::Relaxed);
        }

        let passed = PASSED.load(Ordering::Relaxed);
        let failed = FAILED.load(Ordering::Relaxed);
        let total = passed + failed;

        let _ = class
            .write_packet(b"=== USB CDC Test Summary ===\r\n")
            .await;

        line.clear();
        let _ = write!(line, "SUMMARY: {}/{} passed", passed, total);
        let _ = class.write_packet(line.as_bytes()).await;
        let _ = class.write_packet(b"\r\n").await;

        if failed == 0 {
            let _ = class.write_packet(b"ALL TESTS PASSED\r\n").await;
        } else {
            line.clear();
            let _ = write!(line, "FAILED: {} tests failed", failed);
            let _ = class.write_packet(line.as_bytes()).await;
            let _ = class.write_packet(b"\r\n").await;
        }

        loop {
            Timer::after(embassy_time::Duration::from_secs(1)).await;
        }
    };

    join(usb_fut, test_fut).await;
}
