use std::collections::HashMap;

pub struct GoveeDataDecodeError {}

pub mod gvh5055;
pub mod gvh5075;

pub enum GoveeDeviceStatus {
    GVH5075(gvh5075::DeviceStatus),
    GVH5055(gvh5055::DeviceStatus),
}

impl GoveeDeviceStatus {
    pub fn from_mfg_data(d: &HashMap<u16, Vec<u8>>) -> Option<GoveeDeviceStatus> {
        if !d.is_empty() {
            match d.keys().nth(0) {
                Some(&gvh5075::MANUFACTURER_ID) => {
                    match gvh5075::DeviceStatus::from_mfg_data_bytes(&d[&gvh5075::MANUFACTURER_ID])
                    {
                        Ok(status) => Some(GoveeDeviceStatus::GVH5075(status)),
                        _ => None,
                    }
                }
                Some(&gvh5055::MANUFACTURER_ID) => {
                    match gvh5055::DeviceStatus::from_mfg_data_bytes(&d[&gvh5055::MANUFACTURER_ID])
                    {
                        Ok(status) => Some(GoveeDeviceStatus::GVH5055(status)),
                        _ => None,
                    }
                }
                // TODO support more devices
                _ => None,
            }
        } else {
            None
        }
    }
}
