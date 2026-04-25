#![feature(if_let_guard, try_blocks, try_blocks_heterogeneous)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use opengamesir::udev::{DeviceAction, DeviceMonitor};
use parking_lot::Mutex;
use serde::Serialize;
use tracing::level_filters::LevelFilter;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;
use zbus::interface;
use zbus::object_server::{InterfaceRef, SignalEmitter};
use zbus::zvariant::{OwnedObjectPath, Type};

#[derive(clap::Parser)]
enum Command {
    GetLightProfile,
    GetProfile { profile_id: u8 },
    GetFirmwareVersion,
}

static DEVICE_IDS: &[(u16, u16)] = &[(0x3537, 0x100b)];

#[tokio::main(flavor = "local")]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .from_env_lossy(),
        )
        .init();

    let devices = Arc::new(Mutex::new(BTreeMap::new()));
    let devices_iface = Devices {
        devices: devices.clone(),
    };

    let connection = zbus::connection::Builder::session()?
        .name("dev.hasali.OpenGameSir")?
        .serve_at("/dev/hasali/OpenGameSir/Devices", devices_iface)?
        .build()
        .await?;

    let dbus_devices: InterfaceRef<Devices> = connection
        .object_server()
        .interface("/dev/hasali/OpenGameSir/Devices")
        .await?;

    tokio::spawn(async move {
        let monitor = match DeviceMonitor::new(0x3537) {
            Ok(monitor) => monitor,
            Err(e) => {
                error!("Failed to create device monitor: {e}");
                return;
            }
        };

        let mut events = monitor.monitor_events();

        while let Some(event) = events.next().await {
            let event = match event {
                Ok(event) => event,
                Err(e) => {
                    error!("Error while monitoring devices: {e}");
                    continue;
                }
            };

            if !DEVICE_IDS.contains(&(event.device.vendor_id, event.device.product_id)) {
                continue;
            }

            let res = try bikeshed eyre::Result<()> {
                match event.action {
                    DeviceAction::Add => {
                        let vid = event.device.vendor_id;
                        let pid = event.device.product_id;
                        let product = event.device.product_name;

                        info!("Added {product}, vid={vid}, pid={pid}");

                        let uuid = Uuid::new_v4().simple();
                        let object_path = OwnedObjectPath::try_from(format!(
                            "/dev/hasali/OpenGameSir/Devices/{uuid}"
                        ))?;

                        let device = DeviceInfo {
                            path: object_path.clone(),
                            syspath: event.device.syspath.clone(),
                            vendor_id: vid,
                            product_id: pid,
                            friendly_name: product,
                        };

                        connection
                            .object_server()
                            .at(
                                object_path,
                                Device {
                                    _info: device.clone(),
                                },
                            )
                            .await?;

                        devices.lock().insert(event.device.syspath, device.clone());
                        dbus_devices.device_added(device).await?;
                    }
                    DeviceAction::Remove => {
                        let Some(device) = devices.lock().remove(&event.device.syspath) else {
                            continue;
                        };

                        let vid = event.device.vendor_id;
                        let pid = event.device.product_id;
                        let product = event.device.product_name;

                        info!("Removed {product}, vid={vid}, pid={pid}");

                        connection
                            .object_server()
                            .remove::<Device, _>(&device.path)
                            .await?;

                        dbus_devices.device_removed(device).await?;
                    }
                }
            };

            if let Err(e) = res {
                error!("Failed to process event: {e}");
            }
        }

        info!("Device monitor has terminated");
    });

    tokio::signal::ctrl_c().await?;

    Ok(())
}

struct Devices {
    devices: Arc<Mutex<BTreeMap<PathBuf, DeviceInfo>>>,
}

#[interface(name = "dev.hasali.OpenGameSir.Devices")]
impl Devices {
    fn get_devices(&self) -> Vec<DeviceInfo> {
        self.devices.lock().values().cloned().collect()
    }

    #[zbus(signal)]
    async fn device_added(emitter: &SignalEmitter<'_>, info: DeviceInfo) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn device_removed(emitter: &SignalEmitter<'_>, info: DeviceInfo) -> zbus::Result<()>;
}

#[derive(Clone, Serialize, Type)]
struct DeviceInfo {
    path: OwnedObjectPath,
    syspath: PathBuf,
    vendor_id: u16,
    product_id: u16,
    friendly_name: String,
}

struct Device {
    _info: DeviceInfo,
}

#[interface(name = "dev.hasali.OpenGameSir.Device")]
impl Device {}
