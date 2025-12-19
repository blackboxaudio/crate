use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbDevice {
    pub id: String,
    pub name: String,
    pub mount_point: String,
    pub total_space_bytes: u64,
    pub available_space_bytes: u64,
    pub is_removable: bool,
}
