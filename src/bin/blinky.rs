#![no_main]
#![no_std]

use bluefruit_le as _; // global logger + panicking-behavior + memory layout
use nrf52832_hal::{self as hal, gpio::Level, prelude::*};

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Hello, world!");
    let core_p = cortex_m::Peripherals::take().unwrap();
    let p = hal::pac::Peripherals::take().unwrap();
    let mut delay_tim = hal::delay::Delay::new(core_p.SYST);
    let port0 = hal::gpio::p0::Parts::new(p.P0);
    let mut led1 = port0.p0_17.into_push_pull_output(Level::Low);
    let mut led2 = port0.p0_19.into_push_pull_output(Level::Low);

    loop {
        if led1.is_set_high().unwrap() {
            led1.set_low().unwrap();
            led2.set_high().unwrap();
        } else {
            led1.set_high().unwrap();
            led2.set_low().unwrap();
        }
        delay_tim.delay_ms(250u16);
    }
}
