use crate::govee::GoveeDataDecodeError;

/// btle manufacturer id
pub const MANUFACTURER_ID: u16 = 44566;

pub struct DeviceStatus {}

impl DeviceStatus {
    pub fn from_mfg_data_bytes(_: &[u8]) -> Result<DeviceStatus, GoveeDataDecodeError> {
        return Err(GoveeDataDecodeError {});
    }
}
