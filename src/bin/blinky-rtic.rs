#![no_main]
#![no_std]

use bluefruit_le as _; // global logger + panicking-behavior + memory layout
use rtic::app;

#[app(device = nrf52832_hal::pac, dispatchers = [SWI0_EGU0])]
mod app {
    use dwt_systick_monotonic::{fugit::ExtU32, DwtSystick};
    use nrf52832_hal::{self as hal, gpio::*, prelude::*};

    const F_CPU_HZ: u32 = 64_000_000;
    #[monotonic(binds = SysTick, default = true)]
    type DwtMono = DwtSystick<F_CPU_HZ>;

    #[shared]
    struct Shared {}
    #[local]
    struct Local {
        led1: p0::P0_17<Output<PushPull>>,
        led2: p0::P0_19<Output<PushPull>>,
    }
    #[init]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::println!("Initializing...");
        let _hw_clocks = hal::clocks::Clocks::new(cx.device.CLOCK).enable_ext_hfosc();

        let mut dcb = cx.core.DCB;
        let dwt = cx.core.DWT;
        let systick = cx.core.SYST;
        let mono_clock = DwtSystick::new(&mut dcb, dwt, systick, F_CPU_HZ);

        /*
        let core_p = cortex_m::Peripherals::take().unwrap();
        let p = hal::pac::Peripherals::take().unwrap();
        let mut delay_tim = hal::delay::Delay::new(core_p.SYST);
        */
        let port0 = hal::gpio::p0::Parts::new(cx.device.P0);
        let led1 = port0.p0_17.into_push_pull_output(Level::Low);
        let led2 = port0.p0_19.into_push_pull_output(Level::Low);

        blink::spawn_after(500u32.millis()).unwrap();

        (
            Shared {},
            Local { led1, led2 },
            init::Monotonics(mono_clock),
        )
    }

    #[task(local = [led1, led2])]
    fn blink(cx: blink::Context) {
        if cx.local.led1.is_set_high().unwrap() {
            cx.local.led1.set_low().unwrap();
            cx.local.led2.set_high().unwrap();
        } else {
            cx.local.led1.set_high().unwrap();
            cx.local.led2.set_low().unwrap();
        }
        blink::spawn_after(500u32.millis()).unwrap();
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        defmt::println!("idle task");
        loop {
            continue;
        }
    }
}
