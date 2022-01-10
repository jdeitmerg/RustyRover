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
            _ => {
                defmt::println!("Error!");
                loop {
                    continue;
                }
            }
        };

        let mut app_ram_base: u32 = 0x20000000 + 0x1AE0;
        defmt::println!("Enabling BLE stack...");

        match unsafe { sd::sd_ble_enable(&mut app_ram_base) } {
            sd::NRF_SUCCESS => defmt::println!("Success!"),
            _ => {
                defmt::println!("Error!");
                loop {
                    continue;
                }
            }
        };
        defmt::println!("App RAM base address: 0x{:08x}", app_ram_base);

        let mut gap_addr = sd::ble_gap_addr_t {
            _bitfield_1: sd::__BindgenBitfieldUnit::default(),
            addr: [0u8; 6],
        };
        match unsafe { sd::sd_ble_gap_addr_get(&mut gap_addr) } {
            sd::NRF_SUCCESS => defmt::println!(
                "BLE MAC addr: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                gap_addr.addr[0],
                gap_addr.addr[1],
                gap_addr.addr[2],
                gap_addr.addr[3],
                gap_addr.addr[4],
                gap_addr.addr[5],
            ),
            _ => defmt::println!("Error getting BLE MAC addr!"),
        }

        let mut appearance = 0u16;
        match unsafe { sd::sd_ble_gap_appearance_get(&mut appearance) } {
            sd::NRF_SUCCESS => defmt::println!("GAP appearance: {}", appearance),
            _ => defmt::println!("Error getting GAP appearance!"),
        }

        #[rustfmt::skip]
        let dev_name: [u8; 10] = ['R' as u8, 'u' as u8, 's' as u8, 't' as u8, 'y' as u8, 'R' as u8, 'o' as u8, 'v' as u8, 'e' as u8, 'r' as u8];
        let conn_sec_mode: sd::ble_gap_conn_sec_mode_t = sd::ble_gap_conn_sec_mode_t {
            // Security Mode 1, Level 1 = Open Link
            _bitfield_1: sd::ble_gap_conn_sec_mode_t::new_bitfield_1(1, 1),
        };
        match unsafe {
            sd::sd_ble_gap_device_name_set(&conn_sec_mode, &dev_name[0], dev_name.len() as u16)
        } {
            sd::NRF_SUCCESS => defmt::println!("Device name set successfully."),
            _ => {
                defmt::println!("Error setting device name!");
                loop {
                    continue;
                }
            }
        }

        let peer_addr: sd::ble_gap_addr_t = sd::ble_gap_addr_t {
            // add_id_peer is only valid for peer addresses. Which this is not.
            _bitfield_1: sd::ble_gap_addr_t::new_bitfield_1(0, sd::BLE_GAP_ADDR_TYPE_PUBLIC as u8),
            addr: gap_addr.addr,
        };
        let mut adv_handle = sd::BLE_GAP_ADV_SET_HANDLE_NOT_SET as u8;
        // advertisement type: BLE_GAP_ADV_TYPE_CONNECTABLE_SCANNABLE_UNDIRECTED
        let adv_params: sd::ble_gap_adv_params_t = sd::ble_gap_adv_params_t {
            properties: sd::ble_gap_adv_properties_t {
                // Undirected means non-paired in BLE speak
                type_: sd::BLE_GAP_ADV_TYPE_CONNECTABLE_SCANNABLE_UNDIRECTED as u8,
                // See https://infocenter.nordicsemi.com/index.jsp?topic=%2Fcom.nordic.infocenter.s132.api.v7.3.0%2Fstructble__gap__adv__properties__t.html
                _bitfield_1: sd::ble_gap_adv_properties_t::new_bitfield_1(0, 0),
            },
            p_peer_addr: core::ptr::null(), // as in NordicBlinky, was &peer_addr
            interval: 64, // as in NordicBlinky, was 480,                                // 300ms / 625µs = 480
            duration: sd::BLE_GAP_ADV_TIMEOUT_GENERAL_UNLIMITED as u16, // as in NordicBlinky, was 10000,                              // 100s / 10ms = 10000
            max_adv_evts: 0,                                            // no limit
            // mask is inverted (for my logic): a 0 enabled the channel, a 1 disables it. Enable all channels:
            channel_mask: [0x00, 0x00, 0x00, 0x00, 0x00],
            filter_policy: sd::BLE_GAP_ADV_FP_ANY as u8,
            primary_phy: sd::BLE_GAP_PHY_AUTO as u8,
            secondary_phy: sd::BLE_GAP_PHY_NOT_SET as u8,
            // set_id is only relevant for exteded advertising types
            // scan_req_notification: Raise GAP event when scanned
            _bitfield_1: sd::ble_gap_adv_params_t::new_bitfield_1(0, 1),
        };

        #[rustfmt::skip]
        let mut adv_data: [u8; 10] = [
            2, 0x01, 0x06, // flags: 0b00000110 (LE General Discoverable Mode, BR/EDR not supported)
            //Shortened Local Name:
            6, 0x08, 'R' as u8, 'u' as u8, 's' as u8, 't' as u8, 'y' as u8,
            //Complete Local Name. Apparently mustn't be too long:
            //6, 0x09, 'R' as u8, 'u' as u8, 's' as u8, 't' as u8, 'y' as u8,
        ];

        #[rustfmt::skip]
        let mut scan_resp: [u8; 12] = [
            // Complete Local Name:
            11, 0x09, 'R' as u8, 'u' as u8, 's' as u8, 't' as u8, 'y' as u8, 'R' as u8, 'o' as u8, 'v' as u8, 'e' as u8, 'r' as u8,
            //1, 0x03, // Complete list of 16-bit Service UUIDs (empty)
            //1, 0x05, // Complete list of 32-bit Service UUIDs (empty)
            //1, 0x07, // Complete list of 128-bit Service UUIDs (empty)
        ];

        let adv_data_handle: sd::ble_gap_adv_data_t = sd::ble_gap_adv_data_t {
            adv_data: sd::ble_data_t {
                p_data: &mut adv_data[0],
                len: adv_data.len() as u16,
            },
            scan_rsp_data: sd::ble_data_t {
                p_data: &mut scan_resp[0],
                len: scan_resp.len() as u16,
            },
        };

        let mut config_ok = false;
        match unsafe {
            sd::sd_ble_gap_adv_set_configure(&mut adv_handle, &adv_data_handle, &adv_params)
        } {
            sd::NRF_SUCCESS => {
                defmt::println!("Advertisement config successful!");
                config_ok = true
            }
            sd::NRF_ERROR_INVALID_LENGTH => {
                defmt::println!("Advertisement config failed: NRF_ERROR_INVALID_LENGTH")
            }
            sd::NRF_ERROR_NOT_SUPPORTED => {
                defmt::println!("Advertisement config failed: NRF_ERROR_NOT_SUPPORTED")
            }
            sd::NRF_ERROR_NO_MEM => {
                defmt::println!("Advertisement config failed: NRF_ERROR_NO_MEM")
            }
            sd::BLE_ERROR_GAP_UUID_LIST_MISMATCH => {
                defmt::println!(
                    "Advertisement config fail0x00,ed: BLE_ERROR_GAP_UUID_LIST_MISMATCH"
                )
            }
            sd::NRF_ERROR_INVALID_ADDR => {
                defmt::println!("Advertisement config failed: NRF_ERROR_INVALID_ADDR")
            }
            sd::NRF_ERROR_INVALID_PARAM => {
                defmt::println!("Advertisement config failed: NRF_ERROR_INVALID_PARAM")
            }
            sd::BLE_ERROR_GAP_INVALID_BLE_ADDR => {
                defmt::println!("Advertisement config failed: BLE_ERROR_GAP_INVALID_BLE_ADDR")
            }
            sd::NRF_ERROR_INVALID_STATE => {
                defmt::println!("Advertisement config failed: NRF_ERROR_INVALID_STATE")
            }
            sd::BLE_ERROR_GAP_DISCOVERABLE_WITH_WHITELIST => {
                defmt::println!(
                    "Advertisement config failed: BLE_ERROR_GAP_DISCOVERABLE_WITH_WHITELIST"
                )
            }
            sd::BLE_ERROR_INVALID_ADV_HANDLE => {
                defmt::println!("Advertisement config failed: BLE_ERROR_INVALID_ADV_HANDLE")
            }
            sd::NRF_ERROR_INVALID_FLAGS => {
                defmt::println!("Advertisement config failed: NRF_ERROR_INVALID_FLAGS")
            }
            sd::NRF_ERROR_INVALID_DATA => {
                defmt::println!("Advertisement config failed: NRF_ERROR_INVALID_DATA")
            }
            other => defmt::println!("Advertisement config failed: {}", other),
        }
        if !config_ok {
            loop {
                continue;
            }
        }

        match unsafe { sd::sd_ble_gap_adv_start(adv_handle, sd::BLE_CONN_CFG_TAG_DEFAULT as u8) } {
            sd::NRF_SUCCESS => defmt::println!("Advertisement started successfully!"),
            _ => defmt::println!("Error starting advertisement!"),
        }

        loop {
            continue;
        }
    }

    #[task(binds = SWI2_EGU2)]
    fn softdev_event_notify(_: softdev_event_notify::Context) {
        defmt::println!("SoftDevice notification!")
    }
}
