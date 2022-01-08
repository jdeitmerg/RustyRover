#![no_main]
#![no_std]

use bluefruit_le as _; // global logger + panicking-behavior + memory layout
use rtic::app;

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
    use nrf_softdevice_s112 as sd;

    const F_CPU_HZ: u32 = 64_000_000;
    /* The NRF52832 has NVIC_PRIO_BITS = 3, so the RTIC task priorities
     * range from 1..8.
     * For the SoftDevice we have to reserve NVIC priorities 0, 1 and 4,
     * which correspond to RTIC priorities 8, 7, 4. We may only use RTIC
     * priorities 1,2,3,5,6 for our tasks!
     * If these don't suffice any more at some point, we can probably also
     * use priority 4, as it can't preemt a SoftDevice handler running at
     * the same priority.
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

    #[no_mangle]
    extern "C" fn nrf_fault_handler(id: u32, pc: u32, info: u32) {
        defmt::println!(
            "nrf hard fault! ID: 0x{:08x}, PC: 0x{:08x}, INFO: 0x{:08x}",
            id,
            pc,
            info
        );
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
        defmt::println!("idle task");

        /* Initialize SoftDevice here, as interrupts are enabled so we
         * can use SVC.
         */
        defmt::println!("Enabling SoftDevice...");
        const SD_CLK_CONF: sd::nrf_clock_lf_cfg_t = sd::nrf_clock_lf_cfg_t {
            source: sd::NRF_CLOCK_LF_SRC_XTAL as u8,
            rc_ctiv: 0,
            rc_temp_ctiv: 0,
            accuracy: sd::NRF_CLOCK_LF_ACCURACY_50_PPM as u8,
        };
        let retval = unsafe { sd::sd_softdevice_enable(&SD_CLK_CONF, Some(nrf_fault_handler)) };
        match retval {
            sd::NRF_SUCCESS => defmt::println!("Success!"),
            _ => defmt::println!("Error!"),
        };

        /*
        let mut app_ram_base = 0u32;
        defmt::println!("Enabling BLE stack...");
        let retval = unsafe { sd::sd_ble_enable(&mut app_ram_base) };
        match retval {
            sd::NRF_SUCCESS => defmt::println!("Success!"),
            _ => defmt::println!("Error!"),
        };
        defmt::println!("App RAM base address: 0x{:08x}", app_ram_base);
        */

        loop {
            continue;
        }
    }

    #[task(binds = SWI2_EGU2)]
    fn softdev_event_notify(_: softdev_event_notify::Context) {
        defmt::println!("SoftDevice notification!")
    }
}
