mod device;
mod profile;

use std::io::{Cursor, Write};
use std::time::Duration;

use eyre::ensure;
use tracing::debug;

use crate::driver::device::{Device, TimeoutError};
use crate::hid::Hid;

pub use profile::*;

pub struct Cyclone2<'a> {
    device: Device<'a>,
}

pub struct FirmwareVersion {
    pub controller: String,
    pub dongle: String,
}

impl<'a> Cyclone2<'a> {
    pub fn connect(hid: &'a Hid) -> eyre::Result<Cyclone2<'a>> {
        Ok(Cyclone2 {
            device: Device::connect(hid, 0x3537, 0x100b)?,
        })
    }

    pub fn get_firmware_version(&self) -> eyre::Result<FirmwareVersion> {
        let res = self.write_acked_with_retry(&[0x0f, 0x09])?;

        let controller_version = str::from_utf8(&res[4..=8])?.replace('\0', ".");
        let dongle_version = str::from_utf8(&res[12..=16])?.replace('\0', ".");

        Ok(FirmwareVersion {
            controller: controller_version,
            dongle: dongle_version,
        })
    }

    pub fn get_control_profile(&self, num: ProfileNum) -> eyre::Result<ControlProfile> {
        let profile_bytes = self.read_profile(ProfileId::Num(num), 680)?;
        let mut cursor = Cursor::new(&profile_bytes);
        ControlProfile::read(&mut cursor)
    }

    pub fn set_control_profile(
        &self,
        num: ProfileNum,
        profile: &ControlProfile,
    ) -> eyre::Result<()> {
        let mut bytes = Vec::with_capacity(680);
        profile.write(&mut bytes)?;
        self.write_profile(ProfileId::Num(num), &bytes)
    }

    pub fn get_light_profile(&self) -> eyre::Result<LightProfile> {
        let profile_bytes = self.read_profile(ProfileId::Light, 635)?;
        let mut cursor = Cursor::new(&profile_bytes);
        LightProfile::read(&mut cursor)
    }

    pub fn set_light_profile(&self, profile: &LightProfile) -> eyre::Result<()> {
        let mut bytes = Vec::with_capacity(635);
        profile.write(&mut bytes)?;
        self.write_profile(ProfileId::Light, &bytes)
    }

    fn read_profile(&self, id: ProfileId, size: usize) -> eyre::Result<Vec<u8>> {
        let profile_size = size as u16;

        let chunk_size = 58u16;
        let chunk_count = profile_size.div_ceil(chunk_size);

        let mut profile_bytes = Vec::with_capacity(profile_size as usize);

        for i in 0..chunk_count {
            let byte_offset = chunk_size * i;

            let chunk_size = if i == chunk_count - 1 {
                profile_size - chunk_size * i
            } else {
                chunk_size
            };

            let req = &[
                0x0f,
                0x04,
                id.index(),
                (byte_offset >> 8) as u8,
                (byte_offset & 0xff) as u8,
                chunk_size as u8,
            ];

            let res = self.write_acked_with_retry(req)?;

            ensure!(&res[0..2] == &[0x10, 0x05]);
            ensure!(&res[2..6] == &req[2..6]);

            profile_bytes.extend_from_slice(&res[6..6 + chunk_size as usize]);
        }

        Ok(profile_bytes)
    }

    fn write_profile(&self, id: ProfileId, bytes: &[u8]) -> eyre::Result<()> {
        let profile_size = bytes.len();

        let chunk_size = 58usize;
        let chunk_count = profile_size.div_ceil(chunk_size);

        for i in 0..chunk_count {
            let byte_offset = chunk_size * i;

            let chunk_size = if i == chunk_count - 1 {
                profile_size - chunk_size * i
            } else {
                chunk_size
            };

            let mut cmd = [0; 64];
            let mut cursor = Cursor::new(cmd.as_mut_slice());

            cursor.write_all(&[
                0x0f,
                0x03,
                id.index(),
                (byte_offset >> 8) as u8,
                (byte_offset & 0xff) as u8,
                chunk_size as u8,
            ])?;

            cursor.write_all(&bytes[byte_offset..byte_offset + chunk_size])?;

            let res = self.write_acked_with_retry(&cmd)?;

            ensure!(&res[0..2] == &[0x10, 0x06]);
        }

        Ok(())
    }

    /// Sends a command, expecting an ack. If no ack is received within the
    /// time limit, the command is resent.
    fn write_acked_with_retry(&self, req: &[u8]) -> eyre::Result<[u8; 64]> {
        loop {
            self.device.write(req)?;
            match self.device.read_timeout(Duration::from_millis(200)) {
                Ok(res) => return Ok(res),
                Err(TimeoutError::Timeout) => debug!("Request timed out, retrying"),
                Err(TimeoutError::Other(e)) => return Err(e),
            };
        }
    }
}
