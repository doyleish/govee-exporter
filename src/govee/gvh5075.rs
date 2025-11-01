use crate::govee::{GoveeDevice, GoveeError};
use prometheus_exporter::{
    self,
    prometheus::{
        core::{AtomicF64, GenericCounter, GenericGauge},
        labels, opts, register_counter, register_gauge,
    },
};

/// btle manufacturer id
pub const MANUFACTURER_ID: u16 = 60552;
pub const MODEL_NAME: &str = "GVH5075";

pub struct GVH5075 {
    name: String,
    temperature_c: GenericGauge<AtomicF64>,
    humidity_percentage: GenericGauge<AtomicF64>,
    battery_percentage: GenericGauge<AtomicF64>,
    advertisements: GenericCounter<AtomicF64>,
}

impl GVH5075 {
    pub fn from_name(name: &str) -> GVH5075 {
        let err_msg = "hit an unknown error constructing the metrics and their labels";
        let labels = labels! {
            "device_name" => name,
            "device_model" => MODEL_NAME,
        };
        GVH5075 {
            name: String::from(name),
            temperature_c: register_gauge!(opts!(
                "govee_temperature_c",
                "Temperature in Celsius",
                labels
            ))
            .expect(name),
            humidity_percentage: register_gauge!(opts!(
                "govee_humidity_percentage",
                "Humidity percentage",
                labels
            ))
            .expect(err_msg),
            battery_percentage: register_gauge!(opts!(
                "govee_battery_percentage",
                "Battery percentage",
                labels
            ))
            .expect(err_msg),
            advertisements: register_counter!(opts!(
                "govee_advertisement_count",
                "Number of btle advertisements read",
                labels
            ))
            .expect(err_msg),
        }
    }
}

impl GoveeDevice for GVH5075 {
    fn get_name(&self) -> String {
        self.name.clone()
    }
    fn get_model(&self) -> String {
        String::from(MODEL_NAME)
    }
    fn update_metrics_from_mfg_bytes(&self, id: &u16, v: &[u8]) -> Option<GoveeError> {
        if id != &MANUFACTURER_ID {
            return Some(GoveeError::ManufacturerIdMismatch);
        }
        if v.len() < 4 {
            Some(GoveeError::DataDecode)
        } else {
            // 3 bytes forming an unsigned 24 bit integer
            // binary would look like aaaaaaaabbbbbbbbcccccccc, so first and second bytes are shifted their respective amounts
            // eg [_, 3, 112, 165, 90, ...] would decode as 00000011:01110000:10100101, or 225445 in decimal
            //    000000110111000010100101 (225445)
            //    And below be unpacked as 22.5 degrees C and 44.5% humidity
            let govee_encoded: u64 = { ((v[1] as u64) << 16) | ((v[2] as u64) << 8) | v[3] as u64 };
            // temperature and humidity packed as ttthhh where ttt is 10temp and hhh is 10humidity
            // modulo to get the humidity as remainder, integer division to get the temp
            self.advertisements.inc();
            self.temperature_c.set((govee_encoded / 1000) as f64 / 10.0);
            self.humidity_percentage
                .set((govee_encoded % 1000) as f64 / 10.0);
            self.battery_percentage.set(v[4] as f64);
            None
        }
    }
}
