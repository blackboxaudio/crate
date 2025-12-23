use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbDevice {
    /// Unique identifier: volume UUID if available, otherwise mount point
    pub id: String,
    pub name: String,
    pub mount_point: String,
    /// Volume UUID for stable identification across reconnections (platform-specific)
    pub volume_uuid: Option<String>,
    pub total_space_bytes: u64,
    pub available_space_bytes: u64,
    pub is_removable: bool,
    pub file_system: String,
    pub disk_kind: String,
}
