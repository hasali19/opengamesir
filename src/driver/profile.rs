use std::io::{Read, Seek, Write};

use array_builder::ArrayBuilder;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use eyre::bail;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileId {
    Num(ProfileNum),
    Shift,
    Light,
}

impl ProfileId {
    pub fn index(&self) -> u8 {
        match self {
            ProfileId::Num(profile_num) => profile_num.index(),
            ProfileId::Shift => 5,
            ProfileId::Light => 32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileNum {
    P1,
    P2,
    P3,
    P4,
}

impl ProfileNum {
    pub fn index(&self) -> u8 {
        match self {
            ProfileNum::P1 => 1,
            ProfileNum::P2 => 2,
            ProfileNum::P3 => 3,
            ProfileNum::P4 => 4,
        }
    }
}

macro_rules! array_of {
    (|| $e:expr) => {array_of!(|_| $e)};
    (|$i:tt| $e:expr) => {{
        let mut builder = ArrayBuilder::new();
        fn len<T, const N: usize>(_: &ArrayBuilder<T, N>) -> usize {
            N
        }
        for $i in 0..len(&builder) {
            builder.push($e);
        }
        builder.build().unwrap()
    }};
}

#[derive(Debug)]
pub struct ControlProfile {
    pub name: String,
    // Fun_Data
    pub left_motor_value: u8,
    pub right_motor_value: u8,
    pub lt_motor_value: u8,
    pub rt_motor_value: u8,
    pub profile_audio_en: u8,
    pub audio_volume: u8,
    pub audio_mixer: u8,
    pub mic_mute: u8,
    pub mic_sensitivity: u8,
    pub shift_en: u8,
    pub shift_value: u8,
    pub dpad_diagonal_lock_en: u8,
    pub xinput_abxy_change: u8,
    pub switch_abxy_change: u8,
    pub report_rates_gears: u8,
    pub mappings: [ButtonMapping; 16],
    pub fn_mappings: [FunctionKeyConfig; 2],
    pub left_trigger: TriggerConfig,
    pub right_trigger: TriggerConfig,
    pub left_stick: StickConfig,
    pub right_stick: StickConfig,
    pub aim_sensor: MotionConfig,
    pub tilt_sensor: MotionConfig,
}

impl ControlProfile {
    pub fn read(reader: &mut (impl Read + Seek)) -> eyre::Result<ControlProfile> {
        Ok(ControlProfile {
            name: {
                let mut buf = [0; 32];
                reader.read_exact(&mut buf)?;
                String::from_utf8_lossy(&buf).into_owned()
            },
            left_motor_value: reader.read_u8()?,
            right_motor_value: reader.read_u8()?,
            lt_motor_value: reader.read_u8()?,
            rt_motor_value: reader.read_u8()?,
            profile_audio_en: reader.read_u8()?,
            audio_volume: reader.read_u8()?,
            audio_mixer: reader.read_u8()?,
            mic_mute: reader.read_u8()?,
            mic_sensitivity: reader.read_u8()?,
            shift_en: reader.read_u8()?,
            shift_value: reader.read_u8()?,
            dpad_diagonal_lock_en: reader.read_u8()?,
            xinput_abxy_change: reader.read_u8()?,
            switch_abxy_change: reader.read_u8()?,
            report_rates_gears: reader.read_u8()?,
            mappings: {
                // Skip 17 reserved bytes
                reader.seek_relative(17)?;
                array_of!(|| ButtonMapping::read(reader)?)
            },
            fn_mappings: array_of!(|| FunctionKeyConfig::read(reader)?),
            left_trigger: TriggerConfig::read(reader)?,
            right_trigger: TriggerConfig::read(reader)?,
            left_stick: StickConfig::read(reader)?,
            right_stick: StickConfig::read(reader)?,
            aim_sensor: MotionConfig::read(reader)?,
            tilt_sensor: MotionConfig::read(reader)?,
        })
    }
}

#[derive(Debug)]
pub struct ButtonMapping {
    pub turbo_en: u8,
    pub turbo_speed: u8,
    pub map_en: u8,
    pub map: [u8; 3],
    pub toggle_en: u8,
}

impl ButtonMapping {
    pub fn read(reader: &mut impl Read) -> eyre::Result<ButtonMapping> {
        Ok(ButtonMapping {
            turbo_en: reader.read_u8()?,
            turbo_speed: reader.read_u8()?,
            map_en: reader.read_u8()?,
            map: array_of!(|| reader.read_u8()?),
            toggle_en: reader.read_u8()?,
        })
    }
}

#[derive(Debug)]
pub struct FunctionKeyConfig {
    pub mapping: ButtonMapping,
    pub macro_open_status: u8,
    pub macro_cycle_time: u16,
    pub step_num: u8,
    pub steps: [MacroStep; 30],
}

impl FunctionKeyConfig {
    pub fn read(reader: &mut impl Read) -> eyre::Result<FunctionKeyConfig> {
        Ok(FunctionKeyConfig {
            mapping: ButtonMapping::read(reader)?,
            macro_open_status: reader.read_u8()?,
            macro_cycle_time: reader.read_u16::<BigEndian>()?,
            step_num: reader.read_u8()?,
            steps: array_of!(|i| MacroStep {
                step_data: reader.read_u8()?,
                step_hold_time: reader.read_u16::<BigEndian>()?,
                step_delay_time: if i < 29 {
                    reader.read_u16::<BigEndian>()?
                } else {
                    0
                },
            }),
        })
    }
}

#[derive(Debug)]
pub struct MacroStep {
    pub step_data: u8,
    pub step_hold_time: u16,
    // Not set for the final step
    pub step_delay_time: u16,
}

#[derive(Debug)]
pub struct TriggerConfig {
    // turbo_module
    pub turbo_en: u8,
    pub turbo_speed: u8,
    // dead_module
    pub dead_en: u8,
    pub front_dead: u8,
    pub back_dead: u8,
    pub anti_front_dead: u8,
    pub anti_back_dead: u8,
    pub map_en: u8,
    pub map: [u8; 3],
    pub toggle_en: u8,
    // quick_trigger
    pub quick_trigger_status: u8,
    pub quick_trigger_start_value: u8,
    pub quick_trigger_end_value: u8,
    // linear_module
    pub linear_module_en: u8,
    pub linear_status: u8,
    pub linear_data: u8,
    pub linear_control_points: [(u8, u8); 5],
}

impl TriggerConfig {
    pub fn read(reader: &mut impl Read) -> eyre::Result<TriggerConfig> {
        Ok(TriggerConfig {
            turbo_en: reader.read_u8()?,
            turbo_speed: reader.read_u8()?,
            dead_en: reader.read_u8()?,
            front_dead: reader.read_u8()?,
            back_dead: reader.read_u8()?,
            anti_front_dead: reader.read_u8()?,
            anti_back_dead: reader.read_u8()?,
            map_en: reader.read_u8()?,
            map: array_of!(|| reader.read_u8()?),
            toggle_en: reader.read_u8()?,
            quick_trigger_status: reader.read_u8()?,
            quick_trigger_start_value: reader.read_u8()?,
            quick_trigger_end_value: reader.read_u8()?,
            linear_module_en: reader.read_u8()?,
            linear_status: reader.read_u8()?,
            linear_data: reader.read_u8()?,
            linear_control_points: array_of!(|| (reader.read_u8()?, reader.read_u8()?)),
        })
    }
}

#[derive(Debug)]
pub struct StickConfig {
    pub stick_en: u8,
    pub stick_square: u8,
    pub dead_en: u8,
    pub front_dead: u8,
    pub back_dead: u8,
    pub anti_front_dead: u8,
    pub anti_back_dead: u8,
    pub linear_module_en: u8,
    pub linear_status: u8,
    pub linear_data: u8,
    pub linear_control_points: [(u8, u8); 5],
    pub map_en: u8,
    pub x_flip: u8,
    pub y_flip: u8,
    pub axis_ratio: u8,
    pub mouse_dpi: u8,
    pub map_index: u8,
    pub map_cross: u8,
    pub map_up_value: u8,
    pub map_down_value: u8,
    pub map_left_value: u8,
    pub map_right_value: u8,
    pub map_dead_value: u8,
}

impl StickConfig {
    pub fn read(reader: &mut impl Read) -> eyre::Result<StickConfig> {
        Ok(StickConfig {
            stick_en: reader.read_u8()?,
            stick_square: reader.read_u8()?,
            dead_en: reader.read_u8()?,
            front_dead: reader.read_u8()?,
            back_dead: reader.read_u8()?,
            anti_front_dead: reader.read_u8()?,
            anti_back_dead: reader.read_u8()?,
            linear_module_en: reader.read_u8()?,
            linear_status: reader.read_u8()?,
            linear_data: reader.read_u8()?,
            linear_control_points: array_of!(|| (reader.read_u8()?, reader.read_u8()?)),
            map_en: reader.read_u8()?,
            x_flip: reader.read_u8()?,
            y_flip: reader.read_u8()?,
            axis_ratio: reader.read_u8()?,
            mouse_dpi: reader.read_u8()?,
            map_index: reader.read_u8()?,
            map_cross: reader.read_u8()?,
            map_up_value: reader.read_u8()?,
            map_down_value: reader.read_u8()?,
            map_left_value: reader.read_u8()?,
            map_right_value: reader.read_u8()?,
            map_dead_value: reader.read_u8()?,
        })
    }
}

#[derive(Debug)]
pub struct MotionConfig {
    pub sensor_profile_status: u8,
    pub sensor_quick_key_value: u8,
    pub active_axis: u8,
    pub dead_en: u8,
    pub front_dead: u8,
    pub back_dead: u8,
    pub anti_front_dead: u8,
    pub anti_back_dead: u8,
    pub linear_module_en: u8,
    pub linear_status: u8,
    pub linear_data: u8,
    pub linear_control_points: [(u8, u8); 5],
    pub map_en: u8,
    pub x_flip: u8,
    pub y_flip: u8,
    pub axis_ratio: u8,
    pub mouse_dpi: u8,
    pub map_index: u8,
    pub map_cross: u8,
    pub map_up_value: u8,
    pub map_down_value: u8,
    pub map_left_value: u8,
    pub map_right_value: u8,
    pub map_dead_value: u8,
}

impl MotionConfig {
    pub fn read(reader: &mut impl Read) -> eyre::Result<MotionConfig> {
        Ok(MotionConfig {
            sensor_profile_status: reader.read_u8()?,
            sensor_quick_key_value: reader.read_u8()?,
            active_axis: reader.read_u8()?,
            dead_en: reader.read_u8()?,
            front_dead: reader.read_u8()?,
            back_dead: reader.read_u8()?,
            anti_front_dead: reader.read_u8()?,
            anti_back_dead: reader.read_u8()?,
            linear_module_en: reader.read_u8()?,
            linear_status: reader.read_u8()?,
            linear_data: reader.read_u8()?,
            linear_control_points: array_of!(|| (reader.read_u8()?, reader.read_u8()?)),
            map_en: reader.read_u8()?,
            x_flip: reader.read_u8()?,
            y_flip: reader.read_u8()?,
            axis_ratio: reader.read_u8()?,
            mouse_dpi: reader.read_u8()?,
            map_index: reader.read_u8()?,
            map_cross: reader.read_u8()?,
            map_up_value: reader.read_u8()?,
            map_down_value: reader.read_u8()?,
            map_left_value: reader.read_u8()?,
            map_right_value: reader.read_u8()?,
            map_dead_value: reader.read_u8()?,
        })
    }
}

#[derive(Debug)]
pub struct LightProfile {
    pub config_index: u8,
    pub animations: [Animation; 5],
    pub audio_reactive_mode: bool,
    pub user_effect_index: u8, // Doesn't appear to be used for anything
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
            animations: array_of!(|| Animation::read(reader)?),
            audio_reactive_mode: reader.read_u8()? == 1,
            user_effect_index: reader.read_u8()?,
            profile_led: RgbColor::read(reader)?,
            raise_wake_up: reader.read_u8()? == 1,
            standby_time: reader.read_u8()?,
            reserved_data: array_of!(|| reader.read_u8()?),
        })
    }

    pub fn write(&self, writer: &mut impl Write) -> eyre::Result<()> {
        writer.write_u8(self.config_index)?;
        for animation in &self.animations {
            animation.write(writer)?;
        }
        writer.write_u8(self.audio_reactive_mode as u8)?;
        writer.write_u8(self.user_effect_index)?;
        self.profile_led.write(writer)?;
        writer.write_u8(self.raise_wake_up as u8)?;
        writer.write_u8(self.standby_time)?;
        writer.write_all(&self.reserved_data)?;
        Ok(())
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
            frames: array_of!(|| Frame::read(reader)?),
        })
    }

    pub fn write(&self, writer: &mut impl Write) -> eyre::Result<()> {
        writer.write_u8(self.key_frame_count)?;
        writer.write_u8(self.effect_count)?;
        writer.write_u8(self.speed)?;
        writer.write_u8(self.brightness)?;
        for frame in &self.frames {
            frame.write(writer)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Frame {
    pub leds: [RgbColor; 5],
}

impl Frame {
    pub fn read(reader: &mut impl Read) -> eyre::Result<Frame> {
        Ok(Frame {
            leds: array_of!(|| RgbColor::read(reader)?),
        })
    }

    pub fn write(&self, writer: &mut impl Write) -> eyre::Result<()> {
        for led in &self.leds {
            led.write(writer)?;
        }
        Ok(())
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

    pub fn write(&self, writer: &mut impl Write) -> eyre::Result<()> {
        writer.write_u8(self.red)?;
        writer.write_u8(self.green)?;
        writer.write_u8(self.blue)?;
        Ok(())
    }
}
