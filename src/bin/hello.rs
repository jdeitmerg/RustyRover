#![no_main]
#![no_std]

use bluefruit_le as _; // global logger + panicking-behavior + memory layout

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Hello, world!");

    bluefruit_le::exit()
}
