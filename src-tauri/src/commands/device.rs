use tauri::State;

use crate::error::CrateError;
use crate::models::UsbDevice;
use crate::services::DeviceService;

#[tauri::command]
pub async fn get_devices(device_service: State<'_, DeviceService>) -> Result<Vec<UsbDevice>, ()> {
    Ok(device_service.get_removable_devices())
}

#[tauri::command]
pub async fn eject_device(
    mount_point: String,
    device_service: State<'_, DeviceService>,
) -> Result<(), CrateError> {
    device_service.eject_device(&mount_point)
}
