use crate as _; // global logger + panicking-behavior + memory layout
use aligned::{Aligned, A4};
use nrf_softdevice_s112 as sd;

#[no_mangle]
extern "C" fn nrf_fault_handler(id: u32, pc: u32, info: u32) {
    defmt::println!(
        "nrf hard fault! ID: 0x{:08x}, PC: 0x{:08x}, INFO: 0x{:08x}",
        id,
        pc,
        info
    );
}

pub fn init() -> bool {
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
            return false;
        }
    };

    let mut app_ram_base: u32 = 0x20000000 + 0x1AE0;
    defmt::println!("Enabling BLE stack...");

    match unsafe { sd::sd_ble_enable(&mut app_ram_base) } {
        sd::NRF_SUCCESS => defmt::println!("Success!"),
        _ => {
            defmt::println!("Error!");
            return false;
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
            return false;
        }
    }

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
            defmt::println!("Advertisement config fail0x00,ed: BLE_ERROR_GAP_UUID_LIST_MISMATCH")
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
        return false;
    }

    match unsafe { sd::sd_ble_gap_adv_start(adv_handle, sd::BLE_CONN_CFG_TAG_DEFAULT as u8) } {
        sd::NRF_SUCCESS => defmt::println!("Advertisement started successfully!"),
        _ => defmt::println!("Error starting advertisement!"),
    }

    true
}

pub fn handle_evt_notify() {
    defmt::println!("SoftDevice notification!");
    //let mut evt_buf: Aligned<A4, [u8; 128]> = Aligned([0u8; 128]);
    //let mut buf_len: u16 = evt_buf.len().try_into().unwrap();
    let mut evt: Aligned<A4, sd::ble_evt_t> = Aligned(sd::ble_evt_t {
        header: sd::ble_evt_hdr_t {
            evt_id: 0,
            evt_len: 0,
        },
        evt: sd::ble_evt_t__bindgen_ty_1 {
            common_evt: Default::default(),
            gap_evt: Default::default(),
            gattc_evt: Default::default(),
            gatts_evt: Default::default(),
            bindgen_union_field: Default::default(),
        },
    });
    debug_assert!(sd::BLE_EVT_PTR_ALIGNMENT <= 4);
    let evt_buf = &mut evt as *mut Aligned<A4, sd::ble_evt_t> as *mut u8;
    let mut buf_len: u16 = core::mem::size_of::<sd::ble_evt_t>() as u16;
    loop {
        match unsafe { sd::sd_ble_evt_get(evt_buf, &mut buf_len) } {
            sd::NRF_SUCCESS => {
                defmt::println!(
                    "sd_ble_evt_get: Event read!\n\
                      \x20   header:\n\
                      \x20       evt_id: {}\n\
                      \x20       evt_len: {}",
                    evt.header.evt_id,
                    evt.header.evt_len
                );
                // ToDo: Dispatch handlers depending of evt_id range. See BLE_*_EVT_BASE and BLE_*_EVT_LAST
            }
            sd::NRF_ERROR_INVALID_ADDR => defmt::println!("sd_ble_evt_get: Invalid address!"),
            sd::NRF_ERROR_NOT_FOUND => {
                //defmt::println!("sd_ble_evt_get: No more events!");
                break;
            }
            sd::NRF_ERROR_DATA_SIZE => defmt::println!("sd_ble_evt_get: Buffer too small!"),
            _ => defmt::println!("sd_ble_evt_get: Invalid return value!"),
        }
    }
}
