#![no_std]
#![no_main]

use core::sync::atomic::{AtomicUsize, Ordering};

use cortex_m::iprintln;
use cortex_m::peripheral::{DCB, ITM};
use embassy_futures::join::join;
use embassy_stm32::rcc::*;
use embassy_stm32::time::Hertz;
use embassy_stm32::Config;
use embassy_stm32::{bind_interrupts, peripherals, usb};
use embassy_time::{Duration, Ticker};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use embassy_usb::Builder;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});

fn itm_port0() -> Option<&'static mut cortex_m::peripheral::itm::Stim> {
    if DCB::is_debugger_attached() {
        Some(unsafe { &mut (*ITM::PTR).stim[0] })
    } else {
        None
    }
}

fn init_itm() {
    let mut cp = unsafe { cortex_m::peripheral::Peripherals::steal() };

    cp.DCB.enable_trace();

    unsafe {
        cp.ITM.lar.write(0xC5AC_CE55);
        cp.ITM.tcr.write(0x0001_0001);
        cp.ITM.ter[0].write(0xFFFFFFFF);
    }

    stm32_metapac::DBGMCU.cr().modify(|cr| {
        cr.set_trace_ioen(true);
        cr.set_trace_mode(0x01);
    });

    if let Some(stim) = itm_port0() {
        iprintln!(stim, "ITM initialized — SWO on PB3 (TRACECLK=84MHz)");
    }
}

static ITM_MSGS: AtomicUsize = AtomicUsize::new(0);

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

    init_itm();

    let mut ep_out_buffer = [0u8; 256];
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

    let mut usb_dev_config = embassy_usb::Config::new(0xc0de, 0xcafe);
    usb_dev_config.manufacturer = Some("BSP-Test");
    usb_dev_config.product = Some("STM32F469I-DISCO ITM Test");
    usb_dev_config.serial_number = Some("ITM-SWO");

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

    if let Some(stim) = itm_port0() {
        iprintln!(stim, "USB CDC initialized — waiting for connection");
    }

    let usb_fut = usb.run();
    let mut heartbeat = Ticker::every(Duration::from_secs(5));

    let echo_fut = async {
        class.wait_connection().await;

        if let Some(stim) = itm_port0() {
            iprintln!(stim, "USB connected — echo loop active");
            let _ = class.write_packet(b"ITM+USB active\r\n").await;
        }

        let mut rx_buf = [0u8; 64];
        loop {
            rx_buf.fill(0xFF);
            match embassy_futures::select::select(class.read_packet(&mut rx_buf), heartbeat.next())
                .await
            {
                embassy_futures::select::Either::First(result) => match result {
                    Ok(n) => {
                        if let Some(stim) = itm_port0() {
                            let msgs = ITM_MSGS.fetch_add(1, Ordering::Relaxed) + 1;
                            iprintln!(stim, "echo #{}: {} bytes", msgs, n);
                        }
                        if class.write_packet(&rx_buf[..n]).await.is_err() {
                            if let Some(stim) = itm_port0() {
                                iprintln!(stim, "write_packet failed");
                            }
                        }
                    }
                    Err(EndpointError::BufferOverflow) => {
                        if let Some(stim) = itm_port0() {
                            iprintln!(stim, "rx buffer overflow");
                        }
                    }
                    Err(EndpointError::Disabled) => {
                        if let Some(stim) = itm_port0() {
                            iprintln!(stim, "USB disconnected");
                        }
                        break;
                    }
                },
                embassy_futures::select::Either::Second(_) => {
                    if let Some(stim) = itm_port0() {
                        let msgs = ITM_MSGS.load(Ordering::Relaxed);
                        iprintln!(stim, "heartbeat: {} echoes so far", msgs);
                        let _ = class.write_packet(b"heartbeat\r\n").await;
                    } else {
                        let _ = class.write_packet(b"heartbeat\r\n").await;
                    }
                }
            }
        }
    };

    join(usb_fut, echo_fut).await;
}
