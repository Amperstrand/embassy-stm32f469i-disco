#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use core::sync::atomic::{AtomicUsize, Ordering};

use embassy_futures::select::{select, Either};
use embassy_stm32::rcc::*;
use embassy_stm32::time::Hertz;
use embassy_stm32::usb::{Driver, Instance};
use embassy_stm32::Config;
use embassy_stm32::{bind_interrupts, peripherals, usb};
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use embassy_usb::Builder;

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

static PASSED: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);

fn pass(name: &str) {
    PASSED.fetch_add(1, Ordering::Relaxed);
    defmt::info!("TEST {}: PASS", name);
}

fn fail(name: &str, reason: &str) {
    FAILED.fetch_add(1, Ordering::Relaxed);
    defmt::error!("TEST {}: FAIL {}", name, reason);
}

async fn run_echo<'d, T: Instance + 'd>(
    class: &mut CdcAcmClass<'d, Driver<'d, T>>,
    max_iters: usize,
) -> bool {
    class.wait_connection().await;
    defmt::info!("USB connected for echo");
    let mut buf = [0u8; 64];
    for _ in 0..max_iters {
        match class.read_packet(&mut buf).await {
            Ok(n) => {
                if class.write_packet(&buf[..n]).await.is_err() {
                    return false;
                }
            }
            Err(EndpointError::Disabled) => return false,
            Err(EndpointError::BufferOverflow) => {}
        }
    }
    true
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    defmt::info!("=== USB CDC Test Suite ===");
    defmt::info!("NOTE: Requires 84MHz sysclk (incompatible with display/SDRAM)");

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

    // Test 1: USB driver init
    defmt::info!("TEST usb_init: RUNNING");
    let driver = Driver::new_fs(
        p.USB_OTG_FS,
        Irqs,
        p.PA12,
        p.PA11,
        &mut ep_out_buffer,
        usb_config,
    );
    pass("usb_init");

    // Test 2: CDC class + device builder
    defmt::info!("TEST usb_cdc_init: RUNNING");
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
    pass("usb_cdc_init");

    // Test 3: CDC echo (wait for connection + send/receive, 5s timeout)
    defmt::info!("TEST usb_cdc_echo: RUNNING");
    {
        let _usb_fut = usb.run();
        let echo_fut = run_echo(&mut class, 5);
        let timeout = Timer::after(embassy_time::Duration::from_secs(5));

        match select(echo_fut, timeout).await {
            Either::First(echo_ok) => {
                if echo_ok {
                    pass("usb_cdc_echo");
                } else {
                    fail("usb_cdc_echo", "echo returned false");
                }
            }
            Either::Second(_) => {
                // usb.run() is still running — we just timed out waiting for echo
                // This means no host connected, which is expected in standalone mode
                fail("usb_cdc_echo", "timeout — no host connected");
            }
        }
        // usb future is cancelled here
    }

    // Test 4: Sustained poll — re-create USB stack
    defmt::info!("TEST usb_sustained: RUNNING");
    {
        let mut ep_out_buffer2 = [0u8; 256];
        let driver2 = Driver::new_fs(
            unsafe { peripherals::USB_OTG_FS::steal() },
            Irqs,
            unsafe { peripherals::PA12::steal() },
            unsafe { peripherals::PA11::steal() },
            &mut ep_out_buffer2,
            embassy_stm32::usb::Config::default(),
        );

        let mut usb_dev_config2 = embassy_usb::Config::new(0xc0de, 0xcafe);
        usb_dev_config2.manufacturer = Some("BSP-Test");
        usb_dev_config2.product = Some("STM32F469I-DISCO");
        usb_dev_config2.serial_number = Some("poll1234");

        let mut config_descriptor2 = [0u8; 256];
        let mut bos_descriptor2 = [0u8; 256];
        let mut control_buf2 = [0u8; 64];
        let mut state2 = State::new();

        let mut builder2 = Builder::new(
            driver2,
            usb_dev_config2,
            &mut config_descriptor2,
            &mut bos_descriptor2,
            &mut [],
            &mut control_buf2,
        );
        let mut class2 = CdcAcmClass::new(&mut builder2, &mut state2, 64);
        let mut usb2 = builder2.build();

        let _usb_fut2 = usb2.run();
        let poll_fut = run_echo(&mut class2, 100);
        let timeout2 = Timer::after(embassy_time::Duration::from_secs(10));

        match select(poll_fut, timeout2).await {
            Either::First(poll_ok) => {
                if poll_ok {
                    pass("usb_sustained");
                } else {
                    fail("usb_sustained", "poll returned false");
                }
            }
            Either::Second(_) => {
                fail("usb_sustained", "timeout — no host connected");
            }
        }
    }

    // Summary
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("=== USB CDC Test Summary ===");
    defmt::info!("SUMMARY: {}/{} passed", passed, total);
    if failed == 0 {
        defmt::info!("ALL TESTS PASSED");
    } else {
        defmt::error!("FAILED: {} tests failed", failed);
    }

    loop {
        Timer::after(embassy_time::Duration::from_secs(1)).await;
    }
}
