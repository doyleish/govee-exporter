use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Manager, Peripheral, PeripheralId};
use futures::stream::StreamExt;
use prometheus_exporter::{
    self,
    prometheus::{
        core::{AtomicF64, GenericCounter, GenericGauge},
        labels, opts, register_counter, register_gauge,
    },
};
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use tokio::spawn;

struct DecodeGoveeDataError;

struct GoveeMetricsPack {
    temperature_c: GenericGauge<AtomicF64>,
    humidity_percentage: GenericGauge<AtomicF64>,
    battery_percentage: GenericGauge<AtomicF64>,
    advertisements: GenericCounter<AtomicF64>,
}

// TODO handle device (i.e. meat probe) that has multiple data streams
// struct GoveeDevice {
//     thermometers: Vec<GoveeMetricsPack>
// }

struct GVH5075Reading {
    temperature_c: f64,
    humidity_percentage: f64,
    battery_percentage: u8,
}
struct GVH5055Reading {
    temperature_c: f64,
    battery_percentage: u8,
}
enum GoveeDeviceStatus {
    GVH5075(GVH5075Reading),
    GVH5055(
        Option<GVH5055Reading>,
        Option<GVH5055Reading>,
        Option<GVH5055Reading>,
        Option<GVH5055Reading>,
        Option<GVH5055Reading>,
        Option<GVH5055Reading>,
    ),
}

impl GVH5075Reading {
    fn from_bytes(v: &[u8]) -> Result<GVH5075Reading, DecodeGoveeDataError> {
        if v.len() < 4 {
            Err(DecodeGoveeDataError)
        } else {
            // 3 bytes forming an unsigned 24 bit integer
            // binary would look like aaaaaaaabbbbbbbbcccccccc, so first and second bytes are shifted their respective amounts
            // eg [_, 3, 112, 165, 90, ...] would decode as 00000011:01110000:10100101, or 225445 in decimal
            //    000000110111000010100101 (225445)
            //    And below be unpacked as 22.5 degrees C and 44.5% humidity
            let govee_encoded: u64 = { ((v[1] as u64) << 16) | ((v[2] as u64) << 8) | v[3] as u64 };
            // temperature and humidity packed as ttthhh where ttt is 10temp and hhh is 10humidity
            // modulo to get the humidity as remainder, integer division to get the temp
            Ok(GVH5075Reading {
                temperature_c: (govee_encoded / 1000) as f64 / 10.0,
                humidity_percentage: (govee_encoded % 1000) as f64 / 10.0,
                battery_percentage: v[4],
            })
        }
    }

    fn write_metrics_pack(self: &GVH5075Reading, mp: &GoveeMetricsPack) {
        mp.temperature_c.set(self.temperature_c);
        mp.humidity_percentage.set(self.humidity_percentage);
        mp.battery_percentage.set(self.battery_percentage as f64);
        mp.advertisements.inc();
    }
}

impl GVH5055Reading {
    fn from_bytes(v: &[u8]) -> Result<GVH5055Reading, DecodeGoveeDataError> {
        println!("{:?}", v);
        // TODO implement according to notes
        Err(DecodeGoveeDataError)
    }
}

impl GoveeDeviceStatus {
    fn from_mfg_data(d: &HashMap<u16, Vec<u8>>) -> Option<GoveeDeviceStatus> {
        if !d.is_empty() {
            match d.keys().nth(0) {
                Some(60552) => if let Ok(reading) = GVH5075Reading::from_bytes(&d[&60552]) {
                        Some(GoveeDeviceStatus::GVH5075(reading))
                    } else {
                        None
                    }
                Some(44566) => if let Ok(_) = GVH5055Reading::from_bytes(&d[&44566]) {
                        None
                    } else {
                        None
                    }
                _ => None,
            }
        } else {
            None
        }
    }
}

async fn metrics_server() -> Result<prometheus_exporter::Exporter, prometheus_exporter::Error> {
    prometheus_exporter::start(std::net::SocketAddr::from_str("0.0.0.0:8888").unwrap())
}

async fn prop_local_name(p: &Peripheral) -> Option<String> {
    let props = p.properties().await.unwrap().unwrap();
    props.local_name
}

//fn detect_govee_name(name: &String) -> bool {
//    // TODO make this better, hex range for btle object id?
//    name.starts_with("GVH")
//}

const GOVEE_IDS: [u16; 2] = [44566, 60552];

fn detect_govee_mfg_id(ids: Vec<&u16>) -> bool {
    ids.iter().any(|n| GOVEE_IDS.contains(n))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    spawn(metrics_server());

    let manager = Manager::new().await.unwrap();

    // get the first bluetooth adapter
    let adapters = manager.adapters().await?;
    let central = adapters.into_iter().nth(0).unwrap();

    let mut events = central.events().await?;

    // start scanning for devices
    central.start_scan(ScanFilter::default()).await?;

    let mut govee_id_map: HashMap<PeripheralId, GoveeMetricsPack> = HashMap::new();

    // Instantiate guagues and counters
    // TODO: probably map above to a pack of metrics labeled for the specific device
    while let Some(e) = events.next().await {
        //println!("{:?}", &e);
        match e {
            CentralEvent::DeviceDiscovered(id) => {
                let p = central.peripheral(&id).await?;
                //println!("Device Discovered: {:?}", &p);

                if let Some(p_props) = p.properties().await? {
                    let mfg_data: Vec<&u16> = p_props.manufacturer_data.keys().collect();
                    if detect_govee_mfg_id(mfg_data) {
                        let name = prop_local_name(&p).await.unwrap_or("Unknown".to_string());

                        let labels = labels! {
                            "device_name" => &name,
                        };

                        println!("Stored {}: {:?}", &name, &id);
                        // TODO metrics pack per enum?
                        govee_id_map.insert(
                            id,
                            GoveeMetricsPack {
                                temperature_c: register_gauge!(opts!(
                                    "govee_temperature_c",
                                    "Temperature in Celsius",
                                    labels
                                ))?,
                                humidity_percentage: register_gauge!(opts!(
                                    "govee_humidity_percentage",
                                    "Humidity percentage",
                                    labels
                                ))?,
                                battery_percentage: register_gauge!(opts!(
                                    "govee_battery_percentage",
                                    "Battery percentage",
                                    labels
                                ))?,
                                advertisements: register_counter!(opts!(
                                    "govee_advertisement_count",
                                    "Number of btle advertisements read",
                                    labels
                                ))?,
                            },
                        );
                    }
                }
            }

            CentralEvent::ManufacturerDataAdvertisement {
                id,
                manufacturer_data,
            } => {
                //println!("MFG DATA: {}, {:?}", &id, &manufacturer_data);
                if let Some(metrics) = govee_id_map.get(&id) {
                    match GoveeDeviceStatus::from_mfg_data(&manufacturer_data) {
                        Some(GoveeDeviceStatus::GVH5075(reading)) => {
                            println!(
                                "5075 -- Temp: {}, Humidity: {}, Battery: {}, Raw Data: {:?}",
                                reading.temperature_c,
                                reading.humidity_percentage,
                                reading.battery_percentage,
                                manufacturer_data
                            );
                            reading.write_metrics_pack(metrics);
                        }
                        Some(GoveeDeviceStatus::GVH5055(_, _, _, _, _, _)) => {}
                        _ => {}
                    }
                    //if let Ok(govee_reading) = GVH5075Reading::from_bytes(&manufacturer_data[&60552]) {
                    //    govee_reading.wr
                    //    metrics.temperature_c.set(govee_data.temperature_c);
                    //    metrics
                    //        .humidity_percentage
                    //        .set(govee_data.humidity_percentage);
                    //    metrics
                    //        .battery_percentage
                    //        .set(govee_data.battery_percentage as f64);
                    //    metrics.advertisements.inc();
                    //}
                }
            }
            _ => {}
        }
    }
    Ok(())
}
