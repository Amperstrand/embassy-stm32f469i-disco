#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::bind_interrupts;
use embassy_stm32::rng::{InterruptHandler, Rng};
use embassy_stm32::peripherals;
use embassy_stm32::Config;

bind_interrupts!(struct Irqs {
    HASH_RNG => InterruptHandler<peripherals::RNG>;
});

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_stm32::init(Config::default());
    defmt::info!("HIL_TEST:rng:start");

    let mut rng = Rng::new(p.RNG, Irqs);

    let mut buf = [0u8; 256];
    rng.fill_bytes(&mut buf);

    let mut zero_count = 0u32;
    let mut ff_count = 0u32;
    let mut seen = [false; 256];

    for &b in &buf {
        if b == 0 { zero_count += 1; }
        if b == 0xFF { ff_count += 1; }
        seen[b as usize] = true;
    }

    let unique = seen.iter().filter(|&&s| s).count();

    if unique > 150 && zero_count < 10 && ff_count < 10 {
        defmt::info!("HIL_RESULT:rng:PASS (256 bytes, {} unique)", unique);
    } else {
        defmt::error!("HIL_RESULT:rng:FAIL (unique={}, zeros={}, ff={})", unique, zero_count, ff_count);
    }

    embassy_time::Timer::after(embassy_time::Duration::from_millis(10)).await;
    cortex_m::asm::bkpt();
    loop { cortex_m::asm::nop(); }
}
