use std::io::{Cursor, Read, Write};

use array_builder::ArrayBuilder;
use byteorder::ReadBytesExt;
use eyre::{bail, eyre};
use hidapi::HidDevice;

type Packet = [u8; 64];

const PACKET_DATA_LENGTH: usize = 680;
const LIGHT_PROFILE_LENGTH: usize = 635;
const OUT_PACKET_DATA_LENGTH: usize = 58;

const LIGHT_PROFILE_NUMBER: u8 = 32;

#[derive(Debug)]
pub enum Profile {
    Light(LightProfile),
}

pub struct ProfileParser {
    color_buf: Vec<u8>,
}

impl ProfileParser {
    pub fn new() -> ProfileParser {
        ProfileParser {
            color_buf: vec![0; 635],
        }
    }

    /// Attempts to parse profile data from the specified buffer. The buffer is
    /// expected to contain a READ_PROFILE_ACK message. Returns `Ok(Some(...))`
    /// once a complete profile has been parsed, otherwise return `Ok(None)`.
    pub fn accept(&mut self, data: &[u8]) -> eyre::Result<Option<Profile>> {
        let profile_index = data[2];

        if profile_index != LIGHT_PROFILE_NUMBER {
            todo!("profile index: {profile_index}")
        }

        let start_index = 256 * data[3] as usize + data[4] as usize;
        let packet_data_length = data[5] as usize;

        println!("{start_index} {packet_data_length} {}", data.len());

        let is_complete = {
            let target_packet_length = if profile_index == LIGHT_PROFILE_NUMBER {
                635
            } else {
                680
            };

            let cumulative_packet_length = start_index + packet_data_length;

            assert!(cumulative_packet_length <= target_packet_length);

            if cumulative_packet_length == target_packet_length {
                true
            } else {
                false
            }
        };

        self.color_buf.splice(
            start_index..start_index + packet_data_length,
            data[6..6 + packet_data_length].iter().copied(),
        );

        if !is_complete {
            return Ok(None);
        }

        let mut cursor = Cursor::new(&self.color_buf);
        let light_profile = LightProfile::read(&mut cursor)?;

        Ok(Some(Profile::Light(light_profile)))
    }
}

fn get_read_profile_command(is_light_profile: bool) -> Vec<Packet> {
    let mut t = PACKET_DATA_LENGTH;
    if is_light_profile {
        t = LIGHT_PROFILE_LENGTH;
    }

    let i = t.div_ceil(OUT_PACKET_DATA_LENGTH);

    (0..i)
        .map(|i| {
            let mut packet = [0; 64];
            let mut cursor = Cursor::new(packet.as_mut_slice());

            cursor
                .write_all(&[
                    15,
                    4,
                    LIGHT_PROFILE_NUMBER,
                    ((i * OUT_PACKET_DATA_LENGTH) / 256).try_into().unwrap(),
                    ((i * OUT_PACKET_DATA_LENGTH) % 256).try_into().unwrap(),
                    t.min(OUT_PACKET_DATA_LENGTH).try_into().unwrap(),
                ])
                .unwrap();

            t = t.saturating_sub(OUT_PACKET_DATA_LENGTH);

            packet
        })
        .collect()
}

fn build_write_profile_command(data: &[u8], start_index: usize) -> Vec<Packet> {
    let num_packets = data.len().div_ceil(OUT_PACKET_DATA_LENGTH);
    let mut packets = Vec::with_capacity(num_packets);

    let mut remaining_bytes = data.len();
    for i in 0..num_packets {
        let start_index = start_index + i * OUT_PACKET_DATA_LENGTH;
        let mut packet = [0; 64];

        const PROFILE_INDEX_LIGHT: u8 = 32;

        let packet_data_size = remaining_bytes.min(OUT_PACKET_DATA_LENGTH);
        let mut cursor = Cursor::new(packet.as_mut_slice());

        cursor
            .write_all(&[
                15,
                3,
                PROFILE_INDEX_LIGHT,
                (start_index / 256).try_into().unwrap(),
                (start_index % 256).try_into().unwrap(),
                packet_data_size.try_into().unwrap(),
            ])
            .unwrap();

        let start_index = data.len() - remaining_bytes;

        cursor
            .write_all(&data[start_index..start_index + packet_data_size])
            .unwrap();

        packets.push(packet);

        remaining_bytes -= packet_data_size;
    }

    packets
}

#[derive(Debug)]
pub struct LightProfile {
    pub config_index: u8,
    pub animations: [Animation; 5],
    pub audio_reactive_mode: bool,
    pub user_effect_index: u8,
    pub profile_led: RgbColor,
    pub raise_wake_up: bool,
    pub standby_time: u8,
    pub reserved_data: [u8; 7],
}

impl LightProfile {
    pub fn read(reader: &mut impl Read) -> eyre::Result<LightProfile> {
        let config_index = reader.read_u8()?;

        if config_index > 3 {
            bail!("config index must be between 0 and 3: {config_index}");
        }

        Ok(LightProfile {
            config_index,
            animations: {
                let mut builder = ArrayBuilder::new();
                for _ in 0..5 {
                    builder.push(Animation::read(reader)?);
                }
                builder.build().map_err(|_| eyre!("array not filled"))?
            },
            audio_reactive_mode: reader.read_u8()? == 1,
            user_effect_index: reader.read_u8()?,
            profile_led: RgbColor::read(reader)?,
            raise_wake_up: reader.read_u8()? == 1,
            standby_time: reader.read_u8()?,
            reserved_data: {
                let mut reserved = [0; _];
                reader.read_exact(&mut reserved)?;
                reserved
            },
        })
    }
}

#[derive(Debug)]
pub struct Animation {
    pub key_frame_count: u8,
    pub effect_count: u8,
    pub speed: u8,
    pub brightness: u8,
    pub frames: [Frame; 8],
}

impl Animation {
    pub fn read(reader: &mut impl Read) -> eyre::Result<Animation> {
        Ok(Animation {
            key_frame_count: reader.read_u8()?,
            effect_count: reader.read_u8()?,
            speed: reader.read_u8()?,
            brightness: reader.read_u8()?,
            frames: {
                let mut builder = ArrayBuilder::new();
                for _ in 0..8 {
                    builder.push(Frame::read(reader)?);
                }
                builder.build().map_err(|_| eyre!("array not filled"))?
            },
        })
    }
}

#[derive(Debug)]
pub struct Frame {
    pub leds: [RgbColor; 5],
}

impl Frame {
    pub fn read(reader: &mut impl Read) -> eyre::Result<Frame> {
        Ok(Frame {
            leds: {
                let mut builder = ArrayBuilder::new();
                for _ in 0..5 {
                    builder.push(RgbColor::read(reader)?);
                }
                builder.build().map_err(|_| eyre!("array not filled"))?
            },
        })
    }
}

#[derive(Debug)]
pub struct RgbColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl RgbColor {
    pub fn read(reader: &mut impl Read) -> eyre::Result<RgbColor> {
        Ok(RgbColor {
            red: reader.read_u8()?,
            green: reader.read_u8()?,
            blue: reader.read_u8()?,
        })
    }
}
