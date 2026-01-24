use std::collections::VecDeque;
use std::time::{Duration, Instant};

use clap::Parser;
use hidapi::HidApi;
use opengamesir::profile::{self, ProfileParser};
use opengamesir::state;

#[derive(clap::Parser)]
enum Command {
    GetColorProfile,
    GetFirmwareVersion,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let command = Command::parse();

    let api = HidApi::new()?;
    let device = api.open(0x3537, 0x100b)?;

    const HEARTBEAT_COMMAND: &[u8] = &[0xf, 0xf2, 0];
    const READ_FW_VERSION_COMMAND: &[u8] = &[15, 9];

    let mut read_buf = vec![0u8; 2048];
    let mut write_queue = VecDeque::<RequestPacket>::new();
    let mut profile_parser = ProfileParser::new();

    let request;
    match command {
        Command::GetColorProfile => {
            request = Request::GetColorProfile;
        }
        Command::GetFirmwareVersion => {
            request = Request::GetFirmwareVersion;
        }
    }

    match request {
        Request::Heartbeat => {
            write_queue.push_back(RequestPacket {
                data: HEARTBEAT_COMMAND.to_vec(),
                state: RequestPacketState::Queued,
                needs_ack: false,
            });
        }
        Request::GetColorProfile => {
            for packet_data in profile::get_read_profile_command(true) {
                write_queue.push_back(RequestPacket {
                    data: packet_data.to_vec(),
                    state: RequestPacketState::Queued,
                    needs_ack: true,
                });
            }
        }
        Request::GetFirmwareVersion => {
            write_queue.push_back(RequestPacket {
                data: READ_FW_VERSION_COMMAND.to_vec(),
                state: RequestPacketState::Queued,
                needs_ack: true,
            });
        }
    }

    loop {
        while let Some(packet) = write_queue.front_mut() {
            match packet.state {
                RequestPacketState::Queued => {
                    device.write(&packet.data)?;
                }
                RequestPacketState::WaitingForAck { timestamp } => {
                    if Instant::now() > timestamp + Duration::from_millis(200) {
                        eprintln!("timeout, resending");
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

        let len = device.read_timeout(&mut read_buf, 20)?;
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

        match request {
            Request::Heartbeat => unreachable!(),
            Request::GetColorProfile => {
                assert_eq!(buf[1], READ_PROFILE_ACK);

                let profile = match profile_parser.accept(&buf) {
                    Ok(profile) => profile,
                    Err(e) => {
                        eprintln!("error parsing profile data packet: {e}");
                        continue;
                    }
                };

                if let Some(profile) = profile {
                    println!("{profile:#?}");
                    break;
                }
            }
            Request::GetFirmwareVersion => {
                assert_eq!(buf[1], READ_FIRMWARE_VERSION_ACK);
                let fw_version = String::from_utf8_lossy(&buf[4..9]);
                let dongle_version = String::from_utf8_lossy(&buf[12..17]);
                println!("fw_version:     {fw_version}");
                println!("dongle_version: {dongle_version}");
                break;
            }
        }
    }

    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum Request {
    Heartbeat,
    GetColorProfile,
    GetFirmwareVersion,
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
