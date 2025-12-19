use std::collections::HashSet;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sysinfo::Disks;
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

use crate::error::{CrateError, Result};
use crate::models::UsbDevice;

pub struct DeviceService {
    devices: Arc<Mutex<Vec<UsbDevice>>>,
    stop_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
}

impl DeviceService {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(Mutex::new(Vec::new())),
            stop_tx: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_removable_devices(&self) -> Vec<UsbDevice> {
        let disks = Disks::new_with_refreshed_list();

        disks
            .iter()
            .filter(|disk| disk.is_removable())
            .filter(|disk| {
                // Skip system volumes on macOS
                let mount = disk.mount_point().to_string_lossy();
                !mount.starts_with("/System") && mount != "/"
            })
            .map(|disk| {
                let mount_point = disk.mount_point().to_string_lossy().to_string();
                let name = disk.name().to_string_lossy().to_string();

                UsbDevice {
                    id: mount_point.clone(),
                    name: if name.is_empty() {
                        // Use mount point as name if no name available
                        mount_point
                            .split('/')
                            .last()
                            .unwrap_or("USB Device")
                            .to_string()
                    } else {
                        name
                    },
                    mount_point,
                    total_space_bytes: disk.total_space(),
                    available_space_bytes: disk.available_space(),
                    is_removable: true,
                }
            })
            .collect()
    }

    pub fn start_monitoring(&self, app_handle: AppHandle) {
        let devices = self.devices.clone();
        let (stop_tx, mut stop_rx) = watch::channel(false);

        // Store the stop sender
        if let Ok(mut guard) = self.stop_tx.lock() {
            *guard = Some(stop_tx);
        }

        // Get initial device list
        let initial_devices = self.get_removable_devices();
        if let Ok(mut guard) = devices.lock() {
            *guard = initial_devices.clone();
        }

        // Emit initial device list
        let _ = app_handle.emit("devices-changed", &initial_devices);

        // Spawn polling task
        let devices_clone = devices.clone();
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let disks = Disks::new_with_refreshed_list();

                        let current_devices: Vec<UsbDevice> = disks
                            .iter()
                            .filter(|disk| disk.is_removable())
                            .filter(|disk| {
                                let mount = disk.mount_point().to_string_lossy();
                                !mount.starts_with("/System") && mount != "/"
                            })
                            .map(|disk| {
                                let mount_point = disk.mount_point().to_string_lossy().to_string();
                                let name = disk.name().to_string_lossy().to_string();

                                UsbDevice {
                                    id: mount_point.clone(),
                                    name: if name.is_empty() {
                                        mount_point.split('/').last().unwrap_or("USB Device").to_string()
                                    } else {
                                        name
                                    },
                                    mount_point,
                                    total_space_bytes: disk.total_space(),
                                    available_space_bytes: disk.available_space(),
                                    is_removable: true,
                                }
                            })
                            .collect();

                        // Compare with previous state
                        let previous_ids: HashSet<String> = {
                            if let Ok(guard) = devices_clone.lock() {
                                guard.iter().map(|d| d.id.clone()).collect()
                            } else {
                                HashSet::new()
                            }
                        };

                        let current_ids: HashSet<String> =
                            current_devices.iter().map(|d| d.id.clone()).collect();

                        // Only emit if changed
                        if current_ids != previous_ids {
                            log::info!("Device list changed: {:?}", current_devices.iter().map(|d| &d.name).collect::<Vec<_>>());

                            // Update stored devices
                            if let Ok(mut guard) = devices_clone.lock() {
                                *guard = current_devices.clone();
                            }

                            // Emit event to frontend
                            let _ = app_handle.emit("devices-changed", &current_devices);
                        }
                    }
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            log::info!("Stopping device monitoring");
                            break;
                        }
                    }
                }
            }
        });

        log::info!("Device monitoring started");
    }

    #[allow(dead_code)]
    pub fn stop_monitoring(&self) {
        if let Ok(guard) = self.stop_tx.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(true);
            }
        }
    }

    /// Eject a device by its mount point
    pub fn eject_device(&self, mount_point: &str) -> Result<()> {
        log::info!("Ejecting device at: {}", mount_point);

        #[cfg(target_os = "macos")]
        {
            let output = Command::new("diskutil")
                .args(["eject", mount_point])
                .output()
                .map_err(|e| CrateError::Device(format!("Failed to run diskutil: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CrateError::Device(format!("Failed to eject: {}", stderr)));
            }

            log::info!("Successfully ejected device at: {}", mount_point);
            Ok(())
        }

        #[cfg(target_os = "linux")]
        {
            let output = Command::new("umount")
                .arg(mount_point)
                .output()
                .map_err(|e| CrateError::Device(format!("Failed to run umount: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CrateError::Device(format!("Failed to eject: {}", stderr)));
            }

            log::info!("Successfully ejected device at: {}", mount_point);
            Ok(())
        }

        #[cfg(target_os = "windows")]
        {
            // Windows requires a more complex approach using DeviceIoControl
            // For now, return an error indicating this is not yet implemented
            Err(CrateError::Device(
                "Eject is not yet supported on Windows".to_string(),
            ))
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            Err(CrateError::Device(
                "Eject is not supported on this platform".to_string(),
            ))
        }
    }
}
