use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Manager, Peripheral, PeripheralId};
use std::error::Error;
use std::str::FromStr;
use tokio::spawn;
use futures::stream::StreamExt;
use prometheus_exporter::{
    self,
    prometheus::{
        register_gauge,
        register_counter,
        opts,
    }
};

struct DecodeGoveeDataError;

struct GoveeData {
    temperature_c: f64,
    humidity_percentage: f64,
    battery_percentage: u8,
}

impl GoveeData {
    fn from_byte_vec(v: &Vec<u8>) -> Result<GoveeData, DecodeGoveeDataError> {
        if v.len() != 6 {
            return Err(DecodeGoveeDataError);
        } else {
            let goveeencoded: u64 = (v[1] as u64 * 65536) + (v[2] as u64 * 256) + v[3] as u64;
            return Ok(GoveeData {
                temperature_c: goveeencoded as f64 / 10000.0,
                humidity_percentage: (goveeencoded % 1000) as f64 /10.0,
                battery_percentage: v[4] 
            })
        }
    }
}

async fn metrics_server() -> Result<prometheus_exporter::Exporter, prometheus_exporter::Error> {
    prometheus_exporter::start(std::net::SocketAddr::from_str("0.0.0.0:8888").unwrap())
}

//async fn print_peripheral(p: &Peripheral) {
//    let props = p.properties().await.unwrap().unwrap();
//    let local_name = props.local_name.unwrap_or("Unknown".to_string());
//    let id = p.id();
//    println!("====== {}: {}", local_name, id);
//}


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

    let mut govee_ids: Vec<PeripheralId> = Vec::new();

    // Instantiate guagues and counters
    // TODO: probably map above to a pack of metrics labeled for the specific device

    let temp_guage = register_gauge!(opts!("govee_temperature_c", "Yep"))?;
    let humidity_guage = register_gauge!(opts!("govee_humidity_percentage", "Yep"))?;
    let battery_guage = register_gauge!(opts!("govee_battery_percentage", "Yep"))?;
    let therm_hit = register_counter!(opts!("govee_advertisement_count", "Yep"))?;


    while let Some(e) = events.next().await {
        match e {
            CentralEvent::DeviceDiscovered(id) => {
                let p = central.peripheral(&id).await?;

                // TODO better name detection 
                if let Some(name) = prop_local_name(&p).await {
                    if detect_govee_name(&name) {
                        println!("Stored {}", &name);
                        govee_ids.push(id);
                    }
                }
            }

            CentralEvent::ManufacturerDataAdvertisement { id, manufacturer_data } => {
                if govee_ids.contains(&id) {
                    println!("decoding...");
                    if let Ok(govee_data) = GoveeData::from_byte_vec(&manufacturer_data[&60552]) {
                        println!("{:?} -- Temp: {}, Humidity: {}, Battery: {}", id, govee_data.temperature_c, govee_data.humidity_percentage, govee_data.battery_percentage);
                        temp_guage.set(govee_data.temperature_c);
                        humidity_guage.set(govee_data.humidity_percentage);
                        battery_guage.set(govee_data.battery_percentage as f64);
                        therm_hit.inc();
                    }
                }
            }
            _ => {
                println!("processing event")
            }
        }
    }
    Ok(())
}