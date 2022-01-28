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
    use rusty_rover::soft_device::SoftDevice;

    const F_CPU_HZ: u32 = 64_000_000;
    /* The NRF52832 has NVIC_PRIO_BITS = 3, so the RTIC task priorities
     * range from 1..8.
     * For the SoftDevice we have to reserve NVIC priorities 0, 1 and 4,
     * which correspond to RTIC priorities 8, 7, 4. We may only use RTIC
     * priorities 1,2,3,5,6 for our tasks!
     * If these don't suffice any more at some point, we can probably also
     * use priority 4, as it can't preempt a SoftDevice handler running at
     * the same priority.
     * One thing to keep in mind is we may only call to the SoftDevice
     * from NVIC priorities > 4, so RTIC priorities < 4, as our requests
     * will be handled in the SVC handler running at NVIC priority 4!
     */
    #[monotonic(binds = SysTick, default = true, priority = 6)]
    type DwtMono = DwtSystick<F_CPU_HZ>;

    #[shared]
    struct Shared {
        sd: SoftDevice,
        blink_freq: u8, // f = (blink_freq + 1)*.5Hz
    }
    #[local]
    struct Local {
        led1: p0::P0_17<Output<PushPull>>,
        led2: p0::P0_19<Output<PushPull>>,
        pwm: hal::pwm::Pwm<hal::pac::PWM0>,
        motor_r_dir: Pin<Output<PushPull>>,
        motor_l_dir: Pin<Output<PushPull>>,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::info!("HW initialization...");
        let _hw_clocks = hal::clocks::Clocks::new(cx.device.CLOCK).enable_ext_hfosc();

        let mut dcb = cx.core.DCB;
        let dwt = cx.core.DWT;
        let systick = cx.core.SYST;
        let mono_clock = DwtSystick::new(&mut dcb, dwt, systick, F_CPU_HZ);

        let port0 = hal::gpio::p0::Parts::new(cx.device.P0);
        let led1 = port0.p0_17.into_push_pull_output(Level::Low);
        let led2 = port0.p0_19.into_push_pull_output(Level::Low);

        defmt::info!("HW initialization finished.");

        /* Note we cannot initialize the SoftDevice here, as we need SVC
         * interrupts to work for that. However, interrupts are disabled
         * right now because of how RTIC works.
         */
        init_soft_device::spawn().unwrap();

        blink::spawn_after(500u32.millis()).unwrap();

        let mut motors_stby = port0.p0_02.into_push_pull_output(Level::Low);
        let mut motors_r_dir = port0.p0_03.into_push_pull_output(Level::Low).degrade();
        let mut motors_l_dir = port0.p0_04.into_push_pull_output(Level::Low).degrade();
        let mut motors_r_pwm = port0.p0_05.into_push_pull_output(Level::Low).degrade();
        let mut motors_l_pwm = port0.p0_28.into_push_pull_output(Level::Low).degrade();

        motors_stby.set_high().unwrap();
        motors_r_dir.set_high().unwrap();
        motors_l_dir.set_high().unwrap();
        motors_r_pwm.set_high().unwrap();
        motors_l_pwm.set_high().unwrap();

        let pwm = hal::pwm::Pwm::new(cx.device.PWM0);
        pwm.set_period(2000u32.hz())
            .set_output_pin(hal::pwm::Channel::C0, motors_r_pwm)
            .set_output_pin(hal::pwm::Channel::C1, motors_l_pwm);

        let (ch0, ch1, _, _) = pwm.split_channels();
        ch0.set_duty_off(0);
        ch1.set_duty_off(0);

        pwm.enable();

        (
            Shared {
                sd: SoftDevice::new(|speed_r, speed_l| {
                    value_update_handler::spawn(speed_r, speed_l).unwrap()
                }),
                blink_freq: 0,
            },
            Local {
                led1,
                led2,
                pwm,
                motor_r_dir: motors_r_dir,
                motor_l_dir: motors_l_dir,
            },
            init::Monotonics(mono_clock),
        )
    }

    #[task(local = [led1, led2], shared = [blink_freq])]
    fn blink(mut ctx: blink::Context) {
        if ctx.local.led1.is_set_high().unwrap() {
            ctx.local.led1.set_low().unwrap();
            ctx.local.led2.set_high().unwrap();
        } else {
            ctx.local.led1.set_high().unwrap();
            ctx.local.led2.set_low().unwrap();
        }
        let mut blink_freq = 0u32;
        ctx.shared.blink_freq.lock(|freq| blink_freq = *freq as u32);
        let delay_ms = (1000u32 / (blink_freq + 1)).millis();
        blink::spawn_after(delay_ms).unwrap();
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            continue;
        }
    }

    #[task(shared = [sd])]
    fn init_soft_device(mut ctx: init_soft_device::Context) {
        /* Initialize SoftDevice here, as interrupts are enabled so we
         * can use SVC.
         */
        ctx.shared.sd.lock(|sd| sd.init());
    }

    #[task(local = [pwm, motor_r_dir, motor_l_dir])]
    fn value_update_handler(ctx: value_update_handler::Context, speed_r: i8, speed_l: i8) {
        /* This task is spawned "deep" within SoftDevice::handle_evt_notify()
         * as that it what we handed to SoftDevice::new() in app::init()
         * above.
         */
        defmt::info!("New speed value received via BLE: {} {}", speed_r, speed_l);
        let max_duty: u32 = ctx.local.pwm.max_duty().try_into().unwrap();
        let (ch0, ch1, _, _) = ctx.local.pwm.split_channels();
        if speed_r > 0 {
            ctx.local.motor_r_dir.set_high().unwrap();
        } else {
            ctx.local.motor_r_dir.set_low().unwrap();
        }
        if speed_l > 0 {
            ctx.local.motor_l_dir.set_high().unwrap();
        } else {
            ctx.local.motor_l_dir.set_low().unwrap();
        }

        let speed_r_abs: u32 = speed_r.abs().try_into().unwrap();
        let speed_l_abs: u32 = speed_l.abs().try_into().unwrap();

        let duty0: u16 = (max_duty * speed_r_abs / 128).try_into().unwrap();
        let duty1: u16 = (max_duty * speed_l_abs / 128).try_into().unwrap();

        ch0.set_duty_off(duty0);
        ch1.set_duty_off(duty1);
    }

    /* We need two tasks for handling SoftDevice events:
     * One is the actual interrupt handler triggered by the SoftDevice. The
     * other is our own task run at our own priority (currently don't care).
     * This is necessary, as otherwise the SoftDevice asserts when
     *   * GATTS writes are performed from the central to us AND
     *   * we use defmt logging inside the event notify handler
     * This might just be a workaround hiding the real problem, as the real
     * problem is hard to understand (SoftDevice assert with and address
     * and nothing more).
     */
    #[task(shared = [sd])]
    fn softdev_event_notify(mut ctx: softdev_event_notify::Context) {
        ctx.shared.sd.lock(|sd| sd.handle_evt_notify());
    }

    #[task(binds = SWI2_EGU2)]
    fn softdev_event_notify_interrupt(_ctx: softdev_event_notify_interrupt::Context) {
        softdev_event_notify::spawn().unwrap();
    }
}
