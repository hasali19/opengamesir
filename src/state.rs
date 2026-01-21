use std::io::Cursor;

use byteorder::ReadBytesExt;

pub fn parse_gamepad_state(buf: &[u8]) {
    let macro_record_state = is_bit_set(buf[53], 0) || is_bit_set(buf[53], 1);

    #[derive(Debug)]
    enum RecordKey {
        FL1,
        FR1,
    }

    let record_key = if is_bit_set(buf[53], 4) {
        Some(RecordKey::FL1)
    } else if is_bit_set(buf[53], 5) {
        Some(RecordKey::FR1)
    } else {
        None
    };

    let mut cursor = Cursor::new(&buf[35..53]);

    let charge_state = cursor.read_u8().unwrap();
    let battery_level = cursor.read_u8().unwrap();
    let config_index = cursor.read_u8().unwrap();

    let colors = (0..5)
        .map(|_| {
            let r = cursor.read_u8().unwrap();
            let g = cursor.read_u8().unwrap();
            let b = cursor.read_u8().unwrap();
            (r, g, b)
        })
        .collect::<Vec<_>>();

    // println!("charge_state:  {charge_state}");
    // println!("battery_level: {battery_level}");
    // println!("is_recording:  {macro_record_state}");
    // println!("record_key:    {record_key:?}");
    // println!("config_index:  {config_index}");
    // println!("colors:");
    // for color in colors {
    //     println!("  {color:?}");
    // }
}

fn is_bit_set(bits: u8, n: u8) -> bool {
    bits & (1 << n) != 0
}
