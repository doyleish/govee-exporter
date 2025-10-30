use crate::govee::GoveeDataDecodeError;

/// btle manufacturer id
pub const MANUFACTURER_ID: u16 = 60552;

pub struct DeviceStatus {
    pub temperature_c: f64,
    pub humidity_percentage: f64,
    pub battery_percentage: u8,
}

impl DeviceStatus {
    pub fn from_mfg_data_bytes(v: &[u8]) -> Result<DeviceStatus, GoveeDataDecodeError> {
        if v.len() < 4 {
            Err(GoveeDataDecodeError {})
        } else {
            // 3 bytes forming an unsigned 24 bit integer
            // binary would look like aaaaaaaabbbbbbbbcccccccc, so first and second bytes are shifted their respective amounts
            // eg [_, 3, 112, 165, 90, ...] would decode as 00000011:01110000:10100101, or 225445 in decimal
            //    000000110111000010100101 (225445)
            //    And below be unpacked as 22.5 degrees C and 44.5% humidity
            let govee_encoded: u64 = { ((v[1] as u64) << 16) | ((v[2] as u64) << 8) | v[3] as u64 };
            // temperature and humidity packed as ttthhh where ttt is 10temp and hhh is 10humidity
            // modulo to get the humidity as remainder, integer division to get the temp
            Ok(DeviceStatus {
                temperature_c: (govee_encoded / 1000) as f64 / 10.0,
                humidity_percentage: (govee_encoded % 1000) as f64 / 10.0,
                battery_percentage: v[4],
            })
        }
    }
}
