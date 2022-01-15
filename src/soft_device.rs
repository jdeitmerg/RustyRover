use crate as _; // global logger + panicking-behavior + memory layout
use aligned::{Aligned, A4};
use nrf_softdevice_s112 as sd;

#[no_mangle]
extern "C" fn nrf_fault_handler(id: u32, pc: u32, info: u32) {
    defmt::error!(
        "nrf hard fault! ID: 0x{:08x}, PC: 0x{:08x}, INFO: 0x{:08x}",
        id,
        pc,
        info
    );
}

pub struct SoftDevice {
    adv_data: [u8; 10],
    scan_resp: [u8; 12],
    dev_name: [u8; 10],
    conn_sec_mode: sd::ble_gap_conn_sec_mode_t,
}

impl SoftDevice {
    pub fn new() -> SoftDevice {
        SoftDevice {
            #[rustfmt::skip]
            adv_data: [
                2, 0x01, 0x06, // flags: 0b00000110 (LE General Discoverable Mode, BR/EDR not supported)
                //Shortened Local Name:
                6, 0x08, 'R' as u8, 'u' as u8, 's' as u8, 't' as u8, 'y' as u8,
                //Complete Local Name. Apparently mustn't be too long:
                //6, 0x09, 'R' as u8, 'u' as u8, 's' as u8, 't' as u8, 'y' as u8,
                ],

            #[rustfmt::skip]
            scan_resp: [
                // Complete Local Name:
                11, 0x09, 'R' as u8, 'u' as u8, 's' as u8, 't' as u8, 'y' as u8, 'R' as u8, 'o' as u8, 'v' as u8, 'e' as u8, 'r' as u8,
                //1, 0x03, // Complete list of 16-bit Service UUIDs (empty)
                //1, 0x05, // Complete list of 32-bit Service UUIDs (empty)
                //1, 0x07, // Complete list of 128-bit Service UUIDs (empty)
                ],

            #[rustfmt::skip]
            dev_name: ['R' as u8, 'u' as u8, 's' as u8, 't' as u8, 'y' as u8, 'R' as u8, 'o' as u8, 'v' as u8, 'e' as u8, 'r' as u8],
            conn_sec_mode: sd::ble_gap_conn_sec_mode_t {
                // Security Mode 1, Level 1 = Open Link
                _bitfield_1: sd::ble_gap_conn_sec_mode_t::new_bitfield_1(1, 1),
            },
        }
    }

    pub fn init(&mut self) -> bool {
        const SD_CLK_CONF: sd::nrf_clock_lf_cfg_t = sd::nrf_clock_lf_cfg_t {
            source: sd::NRF_CLOCK_LF_SRC_XTAL as u8,
            rc_ctiv: 0,
            rc_temp_ctiv: 0,
            accuracy: sd::NRF_CLOCK_LF_ACCURACY_50_PPM as u8,
        };
        let retval = unsafe { sd::sd_softdevice_enable(&SD_CLK_CONF, Some(nrf_fault_handler)) };
        match retval {
            sd::NRF_SUCCESS => defmt::debug!("SoftDevice enabled successfully!"),
            _ => {
                defmt::error!("Failed to eanble SoftDevice!");
                return false;
            }
        };

        let mut app_ram_base: u32 = 0x20000000 + 0x1AE0;
        match unsafe { sd::sd_ble_enable(&mut app_ram_base) } {
            sd::NRF_SUCCESS => defmt::debug!("BLE stack enabled successfully!"),
            _ => {
                defmt::error!("Failed to enable BLE stack!");
                return false;
            }
        };
        defmt::info!("App RAM base address: 0x{:08x}", app_ram_base);

        let mut gap_addr: sd::ble_gap_addr_t = sd::ble_gap_addr_t {
            _bitfield_1: sd::__BindgenBitfieldUnit::default(),
            addr: [0u8; 6],
        };
        match unsafe { sd::sd_ble_gap_addr_get(&mut gap_addr) } {
            sd::NRF_SUCCESS => defmt::debug!(
                "BLE MAC addr: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                gap_addr.addr[0],
                gap_addr.addr[1],
                gap_addr.addr[2],
                gap_addr.addr[3],
                gap_addr.addr[4],
                gap_addr.addr[5],
            ),
            _ => defmt::error!("Error getting BLE MAC addr!"),
        }

        let mut appearance = 0u16;
        match unsafe { sd::sd_ble_gap_appearance_get(&mut appearance) } {
            sd::NRF_SUCCESS => defmt::debug!("GAP appearance: {}", appearance),
            _ => defmt::error!("Error getting GAP appearance!"),
        }

        match unsafe {
            sd::sd_ble_gap_device_name_set(
                &self.conn_sec_mode,
                &self.dev_name[0],
                self.dev_name.len() as u16,
            )
        } {
            sd::NRF_SUCCESS => defmt::debug!("Device name set successfully."),
            _ => {
                defmt::error!("Error setting device name!");
                return false;
            }
        }

        let mut adv_handle = sd::BLE_GAP_ADV_SET_HANDLE_NOT_SET as u8;

        let adv_data_handle: sd::ble_gap_adv_data_t = sd::ble_gap_adv_data_t {
            adv_data: sd::ble_data_t {
                p_data: &mut self.adv_data[0],
                len: self.adv_data.len() as u16,
            },
            scan_rsp_data: sd::ble_data_t {
                p_data: &mut self.scan_resp[0],
                len: self.scan_resp.len() as u16,
            },
        };

        let adv_params = sd::ble_gap_adv_params_t {
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

        let mut config_ok = false;
        match unsafe {
            sd::sd_ble_gap_adv_set_configure(&mut adv_handle, &adv_data_handle, &adv_params)
        } {
            sd::NRF_SUCCESS => {
                defmt::debug!("Advertisement config successful!");
                config_ok = true
            }
            sd::NRF_ERROR_INVALID_LENGTH => {
                defmt::error!("Advertisement config failed: NRF_ERROR_INVALID_LENGTH")
            }
            sd::NRF_ERROR_NOT_SUPPORTED => {
                defmt::error!("Advertisement config failed: NRF_ERROR_NOT_SUPPORTED")
            }
            sd::NRF_ERROR_NO_MEM => {
                defmt::error!("Advertisement config failed: NRF_ERROR_NO_MEM")
            }
            sd::BLE_ERROR_GAP_UUID_LIST_MISMATCH => {
                defmt::error!("Advertisement config fail0x00,ed: BLE_ERROR_GAP_UUID_LIST_MISMATCH")
            }
            sd::NRF_ERROR_INVALID_ADDR => {
                defmt::error!("Advertisement config failed: NRF_ERROR_INVALID_ADDR")
            }
            sd::NRF_ERROR_INVALID_PARAM => {
                defmt::error!("Advertisement config failed: NRF_ERROR_INVALID_PARAM")
            }
            sd::BLE_ERROR_GAP_INVALID_BLE_ADDR => {
                defmt::error!("Advertisement config failed: BLE_ERROR_GAP_INVALID_BLE_ADDR")
            }
            sd::NRF_ERROR_INVALID_STATE => {
                defmt::error!("Advertisement config failed: NRF_ERROR_INVALID_STATE")
            }
            sd::BLE_ERROR_GAP_DISCOVERABLE_WITH_WHITELIST => {
                defmt::error!(
                    "Advertisement config failed: BLE_ERROR_GAP_DISCOVERABLE_WITH_WHITELIST"
                )
            }
            sd::BLE_ERROR_INVALID_ADV_HANDLE => {
                defmt::error!("Advertisement config failed: BLE_ERROR_INVALID_ADV_HANDLE")
            }
            sd::NRF_ERROR_INVALID_FLAGS => {
                defmt::error!("Advertisement config failed: NRF_ERROR_INVALID_FLAGS")
            }
            sd::NRF_ERROR_INVALID_DATA => {
                defmt::error!("Advertisement config failed: NRF_ERROR_INVALID_DATA")
            }
            other => defmt::error!("Advertisement config failed: {}", other),
        }
        if !config_ok {
            return false;
        }

        match unsafe { sd::sd_ble_gap_adv_start(adv_handle, sd::BLE_CONN_CFG_TAG_DEFAULT as u8) } {
            sd::NRF_SUCCESS => defmt::debug!("Advertisement started successfully!"),
            _ => defmt::error!("Error starting advertisement!"),
        }

        true
    }

    pub fn handle_evt_notify(&self) {
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
        // * dereferences Aligned<ble_evt_t> to get ble_evt_t
        let evt_buf = &mut *evt as *mut sd::ble_evt_t as *mut u8;
        let mut buf_len: u16 = core::mem::size_of::<sd::ble_evt_t>() as u16;
        loop {
            match unsafe { sd::sd_ble_evt_get(evt_buf, &mut buf_len) } {
                sd::NRF_SUCCESS => {
                    // * dereferences Aligned<ble_evt_t> to get ble_evt_t
                    self.dispatch_event(&*evt);
                }
                sd::NRF_ERROR_INVALID_ADDR => defmt::error!("sd_ble_evt_get: Invalid address!"),
                sd::NRF_ERROR_NOT_FOUND => {
                    // Queue is empty, no more events to process
                    break;
                }
                sd::NRF_ERROR_DATA_SIZE => defmt::error!("sd_ble_evt_get: Buffer too small!"),
                _ => defmt::error!("sd_ble_evt_get: Invalid return value!"),
            }
        }
    }

    fn dispatch_event(&self, evt: &sd::ble_evt_t) {
        let evt_id = evt.header.evt_id as u32;
        match evt_id {
            sd::BLE_EVT_BASE..=sd::BLE_EVT_LAST => {
                let common_evt = unsafe { evt.evt.common_evt.as_ref() };
                self.handle_common_evt(evt_id, common_evt);
            }
            sd::BLE_GAP_EVT_BASE..=sd::BLE_GAP_EVT_LAST => {
                let gap_evt = unsafe { evt.evt.gap_evt.as_ref() };
                self.handle_gap_evt(evt_id, gap_evt);
            }
            sd::BLE_GATTC_EVT_BASE..=sd::BLE_GATTC_EVT_LAST => {
                let gattc_evt = unsafe { evt.evt.gattc_evt.as_ref() };
                self.handle_gattc_evt(evt_id, gattc_evt);
            }
            sd::BLE_GATTS_EVT_BASE..=sd::BLE_GATTS_EVT_LAST => {
                let gatts_evt = unsafe { evt.evt.gatts_evt.as_ref() };
                self.handle_gatts_evt(evt_id, gatts_evt);
            }
            _ => defmt::error!("dispatch_event: Invalid event ID: {}", evt_id),
        }
    }

    fn handle_common_evt(&self, evt_id: u32, _evt: &sd::ble_common_evt_t) {
        match evt_id {
            sd::BLE_COMMON_EVTS_BLE_EVT_USER_MEM_REQUEST => {
                defmt::error!("Common event: Memory request not handled!")
            }
            sd::BLE_COMMON_EVTS_BLE_EVT_USER_MEM_RELEASE => {
                defmt::error!("Common event: Memory release not handled!")
            }
            _ => defmt::error!("Common event: Invalid event ID: {}!", evt_id),
        }
    }
    fn handle_gap_evt(&self, evt_id: u32, _evt: &sd::ble_gap_evt_t) {
        match evt_id {
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_ADV_SET_TERMINATED => {
                defmt::debug!("GAP event: Advertising set terminated.")
            }
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_AUTH_KEY_REQUEST => {
                defmt::debug!("GAP event: Authentication key request.")
            }
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_AUTH_STATUS => {
                defmt::debug!("GAP event: Authentication completed.")
            }
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_CONNECTED => defmt::info!("GAP event: Connected."),
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_CONN_PARAM_UPDATE => {
                defmt::debug!("GAP event: Connection parameters updated.")
            }
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_CONN_SEC_UPDATE => {
                defmt::debug!("GAP event: Connection security updated.")
            }
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_DISCONNECTED => defmt::info!("GAP event: Disconnected."),
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_KEY_PRESSED => defmt::debug!("GAP event: Key pressed."),
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_PASSKEY_DISPLAY => {
                defmt::debug!("GAP event: Passkey display request.")
            }
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_PHY_UPDATE => {
                defmt::debug!("GAP event: PHY update completed.")
            }
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_PHY_UPDATE_REQUEST => {
                defmt::debug!("GAP event: PHY update request.")
            }
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_RSSI_CHANGED => defmt::debug!("GAP event: RSSI report."),
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_SCAN_REQ_REPORT => {
                //defmt::debug!("GAP event: Scan request report.")
            }
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_SEC_INFO_REQUEST => {
                defmt::debug!("GAP event: Security information request.")
            }
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_SEC_PARAMS_REQUEST => {
                defmt::debug!("GAP event: Security parameter request.")
            }
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_SEC_REQUEST => {
                defmt::debug!("GAP event: Security request.")
            }
            sd::BLE_GAP_EVTS_BLE_GAP_EVT_TIMEOUT => defmt::debug!("GAP event: Timeout."),
            _ => defmt::error!("GAP event: Invalid event ID: {}!", evt_id),
        }
    }
    fn handle_gatts_evt(&self, evt_id: u32, _evt: &sd::ble_gatts_evt_t) {
        match evt_id {
            sd::BLE_GATTS_EVTS_BLE_GATTS_EVT_EXCHANGE_MTU_REQUEST => {
                defmt::debug!("GATTS event: MTU exchange request.")
            }
            sd::BLE_GATTS_EVTS_BLE_GATTS_EVT_HVC => {
                defmt::debug!("GATTS event: Handle value confirmation.")
            }
            sd::BLE_GATTS_EVTS_BLE_GATTS_EVT_HVN_TX_COMPLETE => {
                defmt::debug!("GATTS event: Handle value notification completed.")
            }
            sd::BLE_GATTS_EVTS_BLE_GATTS_EVT_RW_AUTHORIZE_REQUEST => {
                defmt::debug!("GATTS event: RW authorization request.")
            }
            sd::BLE_GATTS_EVTS_BLE_GATTS_EVT_SC_CONFIRM => {
                defmt::debug!("GATTS event: Service change confirmation.")
            }
            sd::BLE_GATTS_EVTS_BLE_GATTS_EVT_SYS_ATTR_MISSING => {
                defmt::debug!("GATTS event: Pending access to persistent system attribute.")
            }
            sd::BLE_GATTS_EVTS_BLE_GATTS_EVT_TIMEOUT => {
                defmt::error!("GATTS event: Response timeout.")
            }
            sd::BLE_GATTS_EVTS_BLE_GATTS_EVT_WRITE => {
                defmt::debug!("GATTS event: Write operation performed.")
            }
            _ => defmt::error!("GATTS event: Invalid event ID: {}!", evt_id),
        }
    }
    fn handle_gattc_evt(&self, evt_id: u32, _evt: &sd::ble_gattc_evt_t) {
        defmt::error!(
            "GATTC event handling not implemented! Ignoring ID: {}",
            evt_id
        );
    }
}
