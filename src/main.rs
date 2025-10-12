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

struct GoveeData {
    temperature_c: f64,
    humidity_percentage: f64,
    battery_percentage: u8,
}

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

impl GoveeData {
    fn from_byte_vec(v: &Vec<u8>) -> Result<GoveeData, DecodeGoveeDataError> {
        if v.len() != 6 {
            return Err(DecodeGoveeDataError);
        } else {
            let goveeencoded: u64 = (v[1] as u64 * 65536) + (v[2] as u64 * 256) + v[3] as u64;
            return Ok(GoveeData {
                temperature_c: goveeencoded as f64 / 10000.0,
                humidity_percentage: (goveeencoded % 1000) as f64 / 10.0,
                battery_percentage: v[4],
            });
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

fn detect_govee_name(name: &String) -> bool {
    // TODO make this better, hex range for btle object id?
    name.starts_with("GVH")
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
        match e {
            CentralEvent::DeviceDiscovered(id) => {
                let p = central.peripheral(&id).await?;

                // TODO better name detection
                if let Some(name) = prop_local_name(&p).await {
                    if detect_govee_name(&name) {
                        let labels = labels! {
                            "device_name" => &name,
                        };

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
                        println!("Stored {}", &name);
                    }
                }
            }

            CentralEvent::ManufacturerDataAdvertisement {
                id,
                manufacturer_data,
            } => {
                if let Some(metrics) = govee_id_map.get(&id) {
                    if let Ok(govee_data) = GoveeData::from_byte_vec(&manufacturer_data[&60552]) {
                        println!(
                            "{:?} -- Temp: {}, Humidity: {}, Battery: {}",
                            id,
                            govee_data.temperature_c,
                            govee_data.humidity_percentage,
                            govee_data.battery_percentage
                        );
                        metrics.temperature_c.set(govee_data.temperature_c);
                        metrics
                            .humidity_percentage
                            .set(govee_data.humidity_percentage);
                        metrics
                            .battery_percentage
                            .set(govee_data.battery_percentage as f64);
                        metrics.advertisements.inc();
                    }
                }
            }
            _ => {
                //println!("processing event {:?}", e)
            }
        }
    }
    Ok(())
}
