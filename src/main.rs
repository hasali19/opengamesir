#![feature(if_let_guard, try_blocks)]

use clap::Parser;
use eyre::bail;
use opengamesir::driver::{Cyclone2, ProfileNum};
use opengamesir::hid::Hid;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

#[derive(clap::Parser)]
enum Command {
    GetLightProfile,
    GetProfile { profile_id: u8 },
    GetFirmwareVersion,
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

    let hid = Hid::new()?;
    let c2 = Cyclone2::connect(&hid)?;

    match command {
        Command::GetLightProfile => {
            let profile = c2.get_light_profile();
            println!("{profile:#?}");
        }
        Command::GetProfile { profile_id } => {
            let profile_num = match profile_id {
                1 => ProfileNum::P1,
                2 => ProfileNum::P2,
                3 => ProfileNum::P3,
                4 => ProfileNum::P4,
                _ => bail!("invalid profile id: {profile_id}"),
            };
            let profile = c2.get_control_profile(profile_num)?;
            println!("{profile:#?}");
        }
        Command::GetFirmwareVersion => {
            let version = c2.get_firmware_version()?;
            println!("controller: {}", version.controller);
            println!("dongle:     {}", version.dongle);
        }
    }

    Ok(())
}
