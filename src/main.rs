#![feature(if_let_guard, try_blocks, try_blocks_heterogeneous)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use eyre::Context;
use futures::StreamExt;
use opengamesir::driver::Cyclone2;
use opengamesir::hid::Hid;
use opengamesir::udev::{DeviceAction, DeviceMonitor};
use parking_lot::Mutex;
use serde::Serialize;
use tokio::sync::oneshot;
use tracing::level_filters::LevelFilter;
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;
use zbus::fdo::ObjectManager;
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
        .with_thread_names(true)
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
        .serve_at("/dev/hasali/OpenGameSir", ObjectManager)?
        .serve_at("/dev/hasali/OpenGameSir/Devices", devices_iface)?
        .build()
        .await?;

    let dbus_devices: InterfaceRef<Devices> = connection
        .object_server()
        .interface("/dev/hasali/OpenGameSir/Devices")
        .await?;

    let (hid_sender, hid_receiver) = mpsc::channel();

    thread::Builder::new()
        .name("hid".to_owned())
        .spawn(move || {
            if let Err(e) = hid_thread(hid_receiver) {
                eprintln!("{e:?}");
            }
        })?;

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

                        let battery_level= Arc::new(AtomicU8::new(0));
                        let (init_sender, init_receiver) = oneshot::channel();

                        hid_sender.send(HidReq::AddDevice {
                            syspath: event.device.syspath.clone(),
                            battery_level: battery_level.clone(),
                            reply_sender: init_sender,
                        })?;

                        init_receiver.await??;

                        connection
                            .object_server()
                            .at(
                                object_path,
                                Device {
                                    _info: device.clone(),
                                    battery_level: battery_level,
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

                        hid_sender.send(HidReq::RemoveDevice {
                            syspath: event.device.syspath.clone(),
                        })?;

                        connection
                            .object_server()
                            .remove::<Device, _>(&device.path)
                            .await?;

                        dbus_devices.device_removed(device).await?;
                    }
                }
            };

            if let Err(e) = res {
                error!("Failed to process event: {e:?}");
            }
        }

        info!("Device monitor has terminated");
    });

    tokio::signal::ctrl_c().await?;

    Ok(())
}

enum HidReq {
    AddDevice {
        syspath: PathBuf,
        battery_level: Arc<AtomicU8>,
        reply_sender: oneshot::Sender<eyre::Result<()>>,
    },
    RemoveDevice {
        syspath: PathBuf,
    },
}

fn hid_thread(receiver: mpsc::Receiver<HidReq>) -> eyre::Result<()> {
    let hid = Hid::new()?;

    struct Device<'a> {
        syspath: PathBuf,
        c2: Cyclone2<'a>,
        battery_level: Arc<AtomicU8>,
    }

    let mut devices = vec![];

    let mut next_poll = Instant::now() + Duration::from_secs(60);
    loop {
        let now = Instant::now();

        let req = match receiver.recv_timeout(next_poll - now) {
            Ok(req) => Some(req),
            Err(e) => match e {
                mpsc::RecvTimeoutError::Disconnected => break,
                mpsc::RecvTimeoutError::Timeout => None,
            },
        };

        if let Some(req) = req {
            match req {
                HidReq::AddDevice {
                    syspath,
                    battery_level,
                    reply_sender,
                } => {
                    debug!(?syspath, "Adding device");

                    // TODO: Connect using syspath
                    let mut c2 = Cyclone2::connect(&hid).wrap_err("Failed to connect to device")?;

                    // TODO: Avoid duplication with below
                    match c2.heartbeat() {
                        Ok(state) => {
                            battery_level.store(state.battery_level, Ordering::Release);

                            let _ = reply_sender.send(Ok(()));

                            devices.push(Device {
                                syspath,
                                c2,
                                battery_level,
                            });
                        }
                        Err(e) => {
                            let _ = reply_sender.send(Err(e));
                        }
                    }
                }
                HidReq::RemoveDevice { syspath } => {
                    debug!(?syspath, "Removing device");

                    devices.retain(|device| device.syspath != syspath);
                }
            }
        }

        if now >= next_poll {
            devices.retain_mut(|device| {
                debug!(?device.syspath, "Polling device");

                let Ok(state) = device.c2.heartbeat() else {
                    error!(?device.syspath, "Failed to send heartbeat");
                    return false;
                };

                device
                    .battery_level
                    .store(state.battery_level, Ordering::Release);

                true
            });

            next_poll = now + Duration::from_secs(60);
        }
    }

    debug!("HID thread terminating");

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
    battery_level: Arc<AtomicU8>,
}

#[interface(name = "dev.hasali.OpenGameSir.Device")]
impl Device {
    #[zbus(property)]
    fn battery_level(&self) -> u8 {
        self.battery_level.load(Ordering::Acquire)
    }
}
