#[derive(Debug)]
pub enum GoveeError {
    DataDecode,
    UnsupportedDevice,
    ManufacturerIdMismatch,
}

pub mod gvh5055;
pub mod gvh5075;

pub fn from_id_and_name(id: &u16, name: &str) -> Result<Box<dyn GoveeDevice>, GoveeError> {
    match id {
        &gvh5075::MANUFACTURER_ID => Ok(Box::new(gvh5075::GVH5075::from_name(name))),
        _ => Err(GoveeError::UnsupportedDevice),
    }
}

pub trait GoveeDevice {
    fn get_name(&self) -> String;
    fn get_model(&self) -> String;
    fn update_metrics_from_mfg_bytes(&self, id: &u16, bytes: &[u8]) -> Option<GoveeError>;
}
