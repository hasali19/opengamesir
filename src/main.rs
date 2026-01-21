use std::collections::VecDeque;
use std::io::Cursor;

use byteorder::ReadBytesExt;
use hidapi::{HidApi, HidDevice};
use opengamesir::profile::ProfileParser;
use opengamesir::{profile, state};

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let api = HidApi::new()?;

    for device in api.device_list() {
        println!(
            "{} {:x}:{:x} - {:?} {} {:?} {:x} {:x}",
            device.product_string().unwrap_or("unknown"),
            device.vendor_id(),
            device.product_id(),
            device.path(),
            device.interface_number(),
            device.bus_type(),
            device.usage(),
            device.usage_page(),
        );
    }

    let device = api.open(0x3537, 0x100b)?;

    let device_info = device.get_device_info()?;
    println!("{device_info:?}");

    const HEARTBEAT_COMMAND: &[u8] = &[15, 242, 0];
    const READ_FW_VERSION_COMMAND: &[u8] = &[15, 9];

    // device.write(HEARTBEAT_COMMAND)?;
    // device.write(READ_FW_VERSION_COMMAND)?;

    let mut buf = vec![0u8; 2048];
    let mut gamepad_state = vec![];
    let mut profile_parser = ProfileParser::new();
    loop {
        let len = device.read(&mut buf)?;
        let buf = &buf[..len];

        const GAMEPAD_STATE_REPORT_ID: u8 = 18;
        if buf[0] == GAMEPAD_STATE_REPORT_ID {
            if buf != gamepad_state {
                gamepad_state.clear();
                gamepad_state.extend_from_slice(buf);
                state::parse_gamepad_state(&gamepad_state);
            }
            continue;
        }

        const READ_FIRMWARE_VERSION_ACK: u8 = 10;
        if buf[1] == READ_FIRMWARE_VERSION_ACK {
            let fw_version = String::from_utf8_lossy(&buf[4..9]);
            let dongle_version = String::from_utf8_lossy(&buf[12..17]);
            println!("fw_version:     {fw_version}");
            println!("dongle_version: {dongle_version}");
        }

        const READ_PROFILE_ACK: u8 = 5;
        if buf[1] == READ_PROFILE_ACK {
            let profile = match profile_parser.accept(&buf) {
                Ok(profile) => profile,
                Err(e) => {
                    eprintln!("error parsing profile data packet: {e}");
                    continue;
                }
            };

            if let Some(profile) = profile {
                println!("{profile:#?}");
            }
        }
    }
}

struct Device {
    hid_device: HidDevice,
    send_queue: VecDeque<RequestPacket>,
}

struct RequestPacket {
    data: Vec<u8>,
    state: RequestPacketState,
    needs_ack: bool,
}

enum RequestPacketState {
    Queued,
    WaitingForAck,
}

impl Device {
    pub fn new(hid_device: HidDevice) -> Device {
        Device {
            hid_device: hid_device,
            send_queue: VecDeque::new(),
        }
    }

    pub fn request(&mut self, req: Vec<u8>) {
        self.send_queue.push_back(RequestPacket {
            data: req,
            state: RequestPacketState::Queued,
            needs_ack: true,
        });
    }
}
