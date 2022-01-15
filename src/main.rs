#![no_main]
#![no_std]

use rtic::app;
use rusty_rover as _; // global logger + panicking-behavior + memory layout

/* Of the software interrupts, the SoftDevice reserves
 * SWI1 for radio notifications if enabled
 * SWI2 for event notifications to our app
 * SWI4 for future use
 * SWI5 for internal use
 * This leaves us with SWI0, SWI3 and probably SWI1 for our use as
 * dispatchers.
 */
#[app(device = nrf52832_hal::pac, dispatchers = [SWI0_EGU0])]
mod app {
    use dwt_systick_monotonic::{fugit::ExtU32, DwtSystick};
    use nrf52832_hal::{self as hal, gpio::*, prelude::*};
    use rusty_rover::soft_device;

    const F_CPU_HZ: u32 = 64_000_000;
    /* The NRF52832 has NVIC_PRIO_BITS = 3, so the RTIC task priorities
     * range from 1..8.
     * For the SoftDevice we have to reserve NVIC priorities 0, 1 and 4,
     * which correspond to RTIC priorities 8, 7, 4. We may only use RTIC
     * priorities 1,2,3,5,6 for our tasks!
     * If these don't suffice any more at some point, we can probably also
     * use priority 4, as it can't preemt a SoftDevice handler running at
     * the same priority.
     * One thing to keep in mind is we may only call to the SoftDevice
     * from NVIC priorities > 4, so RTIC priorities < 4, as our requests
     * will be handled in the SVC handler running at NVIC priority 4!
     */
    #[monotonic(binds = SysTick, default = true, priority = 6)]
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
        defmt::info!("HW initialization...");
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

        defmt::info!("HW initialization finished.");

        /* Note we cannot initialize the SoftDevice here, as we need SVC
         * interrupts to work for that. However, interrupts are disabled
         * right now because... RTIC.
         */

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
        /* Initialize SoftDevice here, as interrupts are enabled so we
         * can use SVC.
         */
        soft_device::init();

        loop {
            continue;
        }
    }

    #[task(binds = SWI2_EGU2)]
    fn softdev_event_notify(_: softdev_event_notify::Context) {
        rusty_rover::soft_device::handle_evt_notify();
    }
}
