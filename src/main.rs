use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Manager, Peripheral, PeripheralId};
use futures::stream::StreamExt;
use prometheus_exporter;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use tokio::spawn;

// GOVEE device modules
mod govee;
use crate::govee::gvh5075;

// TODO break this up, possible roll alongside device implementations
mod metrics {
    // only supports 5075 right now
    use crate::govee::gvh5075;
    use prometheus_exporter::{
        self,
        prometheus::{
            core::{AtomicF64, GenericCounter, GenericGauge},
            labels, opts, register_counter, register_gauge,
        },
    };

    pub struct GVH5075 {
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
            };
            GVH5075 {
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
        pub fn update(self: &GVH5075, data: &gvh5075::DeviceStatus) {
            self.temperature_c.set(data.temperature_c);
            self.humidity_percentage.set(data.humidity_percentage);
            self.battery_percentage.set(data.battery_percentage as f64);
            self.advertisements.inc();
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

    // Set up stores for metrics
    // TODO do better
    let mut gvh5075_id_map: HashMap<PeripheralId, metrics::GVH5075> = HashMap::new();
    //let mut gvh5055_id_map: HashMap<PeripheralId, metrics::GVH5055> = HashMap::new();

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
                    let name = prop_local_name(&p).await.unwrap_or("Unknown".to_string());

                    // TODO look at more than first index in case payloads collated
                    if mfg_data.len() > 0 {
                        match mfg_data[0] {
                            &gvh5075::MANUFACTURER_ID => {
                                println!("Detected a GVH5075 by the name of {}", &name);
                                gvh5075_id_map.insert(id, metrics::GVH5075::from_name(&name));
                            }
                            _ => {}
                        }
                    }
                }
            }

            CentralEvent::ManufacturerDataAdvertisement {
                id,
                manufacturer_data,
            } => {
                if let Some(device_status) =
                    govee::GoveeDeviceStatus::from_mfg_data(&manufacturer_data)
                {
                    match device_status {
                        govee::GoveeDeviceStatus::GVH5075(status) => {
                            println!("Detected a GVH5075 payload and updating metrics");
                            if let Some(metrics) = gvh5075_id_map.get(&id) {
                                metrics.update(&status);
                            };
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
