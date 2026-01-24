#![feature(try_blocks)]

use std::collections::VecDeque;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use futures::channel::oneshot;
use futures::executor;
use hidapi::HidApi;
use opengamesir::profile::{self, ProfileParser};
use opengamesir::state;
use parking_lot::Mutex;
use tracing::level_filters::LevelFilter;
use tracing::{debug, error, warn};
use tracing_subscriber::EnvFilter;

#[derive(clap::Parser)]
enum Command {
    GetColorProfile,
    GetFirmwareVersion,
}

#[derive(Clone)]
struct RequestQueue {
    queue: Arc<Mutex<VecDeque<Request>>>,
}

impl RequestQueue {
    fn new() -> Self {
        RequestQueue {
            queue: Default::default(),
        }
    }

    fn push(&self, req: Request) {
        self.queue.lock().push_back(req);
    }

    fn pop(&self) -> Option<Request> {
        self.queue.lock().pop_front()
    }
}

struct Controller {
    request_queue: RequestQueue,
}

impl Controller {
    async fn get_light_color_profile(&self) -> profile::Profile {
        self.request(|result_sender| Request::GetColorProfile { result_sender })
            .await
    }

    async fn get_firmware_version(&self) -> FirmwareVersion {
        self.request(|result_sender| Request::GetFirmwareVersion { result_sender })
            .await
    }

    async fn request<T>(&self, f: impl FnOnce(oneshot::Sender<T>) -> Request) -> T {
        let (sender, receiver) = oneshot::channel();
        let req = f(sender);
        self.request_queue.push(req);
        receiver.await.unwrap()
    }
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .from_env_lossy(),
        )
        .init();

    let command = Command::parse();

    let api = HidApi::new()?;
    let device = api.open(0x3537, 0x100b)?;

    const HEARTBEAT_COMMAND: &[u8] = &[0xf, 0xf2, 0];
    const READ_FW_VERSION_COMMAND: &[u8] = &[15, 9];

    let request_queue = RequestQueue::new();
    let controller = Controller {
        request_queue: request_queue.clone(),
    };

    thread::spawn(move || {
        let mut read_buf = vec![0u8; 2048];
        let mut write_queue = VecDeque::<RequestPacket>::new();
        let mut profile_parser = ProfileParser::new();
        let mut current_req = None;

        loop {
            if current_req.is_none()
                && let Some(req) = request_queue.pop()
            {
                match req {
                    Request::Heartbeat => {
                        write_queue.push_back(RequestPacket {
                            data: HEARTBEAT_COMMAND.to_vec(),
                            state: RequestPacketState::Queued,
                            needs_ack: false,
                        });
                    }
                    Request::GetColorProfile { .. } => {
                        for packet_data in profile::get_read_profile_command(true) {
                            write_queue.push_back(RequestPacket {
                                data: packet_data.to_vec(),
                                state: RequestPacketState::Queued,
                                needs_ack: true,
                            });
                        }
                    }
                    Request::GetFirmwareVersion { .. } => {
                        write_queue.push_back(RequestPacket {
                            data: READ_FW_VERSION_COMMAND.to_vec(),
                            state: RequestPacketState::Queued,
                            needs_ack: true,
                        });
                    }
                }

                current_req = Some(req);
            }

            let res = try {
                while let Some(packet) = write_queue.front_mut() {
                    match packet.state {
                        RequestPacketState::Queued => {
                            device.write(&packet.data)?;
                        }
                        RequestPacketState::WaitingForAck { timestamp } => {
                            if Instant::now() > timestamp + Duration::from_millis(200) {
                                debug!("timeout waiting for ack, resending packet");
                                device.write(&packet.data)?;
                            } else {
                                break;
                            }
                        }
                    }

                    if packet.needs_ack {
                        packet.state = RequestPacketState::WaitingForAck {
                            timestamp: Instant::now(),
                        };
                        break;
                    }

                    write_queue.pop_front();
                }
            };

            if let Err(e) = res {
                error!("failed to write to device: {e}");
                break;
            }

            let len = match device.read_timeout(&mut read_buf, 20) {
                Ok(len) => len,
                Err(e) => {
                    error!("failed to read from device: {e}");
                    break;
                }
            };

            let buf = &read_buf[..len];

            if buf.is_empty() {
                continue;
            }

            const GAMEPAD_STATE_REPORT_ID: u8 = 18;
            if buf[0] == GAMEPAD_STATE_REPORT_ID {
                state::parse_gamepad_state(buf);
                continue;
            }

            let Some(_) = write_queue
                .pop_front_if(|p| matches!(p.state, RequestPacketState::WaitingForAck { .. }))
            else {
                unreachable!("unexpected message from device: {buf:?}");
            };

            const READ_FIRMWARE_VERSION_ACK: u8 = 10;
            const READ_PROFILE_ACK: u8 = 5;

            match buf[1] {
                READ_PROFILE_ACK => {
                    if !matches!(current_req, Some(Request::GetColorProfile { .. })) {
                        warn!("unexpected READ_PROFILE_ACK");
                        continue;
                    }

                    let profile = match profile_parser.accept(&buf) {
                        Ok(profile) => profile,
                        Err(e) => {
                            eprintln!("error parsing profile data packet: {e}");
                            continue;
                        }
                    };

                    if let Some(profile) = profile {
                        let Some(Request::GetColorProfile { result_sender }) = current_req.take()
                        else {
                            unreachable!()
                        };

                        let _ = result_sender.send(profile);
                    }
                }
                READ_FIRMWARE_VERSION_ACK => {
                    let Some(Request::GetFirmwareVersion { result_sender }) = current_req.take()
                    else {
                        warn!("unexpected READ_FIRMWARE_VERSION_ACK");
                        continue;
                    };

                    let fw_version = String::from_utf8_lossy(&buf[4..9]).into_owned();
                    let dongle_version = String::from_utf8_lossy(&buf[12..17]).into_owned();
                    let _ = result_sender.send(FirmwareVersion {
                        fw_version,
                        dongle_version,
                    });
                }
                _ => {}
            }
        }
    });

    executor::block_on(async {
        match command {
            Command::GetColorProfile => {
                controller.get_light_color_profile().await;
            }
            Command::GetFirmwareVersion => {
                let version = controller.get_firmware_version().await;
                println!("fw_version:     {}", version.fw_version);
                println!("dongle_version: {}", version.dongle_version);
            }
        }
    });

    Ok(())
}

enum Request {
    Heartbeat,
    GetColorProfile {
        result_sender: oneshot::Sender<profile::Profile>,
    },
    GetFirmwareVersion {
        result_sender: oneshot::Sender<FirmwareVersion>,
    },
}

struct FirmwareVersion {
    fw_version: String,
    dongle_version: String,
}

struct RequestPacket {
    data: Vec<u8>,
    state: RequestPacketState,
    needs_ack: bool,
}

#[derive(Debug, Clone, Copy)]
enum RequestPacketState {
    Queued,
    WaitingForAck { timestamp: Instant },
}
