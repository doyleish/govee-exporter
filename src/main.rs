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
    let mut device_map: HashMap<PeripheralId, Box<dyn govee::GoveeDevice>> = HashMap::new();

    // BTLE event loop
    while let Some(e) = events.next().await {
        match e {
            CentralEvent::DeviceDiscovered(id) => {
                let p = central.peripheral(&id).await?;

                if let Some(p_props) = p.properties().await? {
                    let mfg_data: Vec<&u16> = p_props.manufacturer_data.keys().collect();
                    let name = prop_local_name(&p).await.unwrap_or("Unknown".to_string());

                    for mfg_id in mfg_data {
                        if let Ok(result) = govee::from_id_and_name(mfg_id, &name) {
                            println!(
                                "Discovered {} device: {}",
                                result.get_model(),
                                result.get_name()
                            );
                            device_map.insert(id, result);
                            break;
                        }
                    }
                }
            }

            CentralEvent::ManufacturerDataAdvertisement {
                id,
                manufacturer_data,
            } => {
                if let Some(device) = device_map.get(&id) {
                    for (k, v) in manufacturer_data {
                        match device.update_metrics_from_mfg_bytes(&k, &v) {
                            Some(err) => {
                                println!("Warning, error while attempting update: {:?}", err);
                            }
                            None => {
                                println!("Updated device: {}", device.get_name());
                                break;
                            }
                        }
                    }
                };
                ()
            }
            _ => {}
        }
    }
    Ok(())
}
