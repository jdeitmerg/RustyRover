#![no_std]
#![no_main]

use nrf52832_hal::pac::interrupt;
use rusty_rover as _; // global logger + panicking-behavior + memory layout
use rusty_rover::soft_device::SoftDevice;

static mut SD: SoftDevice = SoftDevice::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    let mut core_periph = nrf52832_hal::pac::CorePeripherals::take().unwrap();

    unsafe {
        core_periph.NVIC.set_priority(interrupt::SWI2_EGU2, 0xff); // lowest possible priority
        cortex_m::peripheral::NVIC::unmask(interrupt::SWI2_EGU2);
    }

    //core_periph.NVIC.request(interrupt::SWI2_EGU2);

    unsafe {
        SD.init();
    }

    loop {
        continue;
    }
}

#[interrupt]
fn SWI2_EGU2() {
    unsafe {
        SD.handle_evt_notify();
    }
}
