# GameSir Cyclone 2 (C2) — HID Protocol Reference

Reverse-engineered from `GameSir Connect` (Electron app, `main.js`).

---

## 1. Device Identification

**Vendor ID:** `0x3537` (GameSir)

| Product | PID | Notes |
|---|---|---|
| C2 Wired | `0x101D` | Standard wired mode |
| C2 Wireless (dongle) | `0x102A` | Via USB dongle |
| C2 Wired ADC | `0x1053` | ADC firmware variant |
| C2 Wireless (alt dongle) | `0x100B` | Alt dongle firmware |
| C2 Pro (dongle) | `0x1050` | C2 Pro variant |

**Config HID interface:** Usage Page `0xFFF0` (vendor-defined)
**Gamepad input interface:** Usage Page `0xFF00`, Report ID `0x12`

---

## 2. Packet Format

All config packets are **64-byte HID reports**. The first two bytes identify the message:

```
byte[0] = 0x0F          — always (report/category prefix)
byte[1] = command_id    — identifies the specific command
byte[2..] = parameters
```

**Response packets** from the device use the same structure:
- `byte[0]` = `0x0F`
- `byte[1]` = response type code (see command table below)

---

## 3. Command Types

| ID (hex) | ID (dec) | Name | Direction | Used by C2? |
|---|---|---|---|---|
| `0x01` | 1 | `EnterProfileConfig` | Host → Device | No — defined, never sent |
| `0x02` | 2 | `ExitProfileConfig` | Host → Device | No — defined, never sent |
| `0x03` | 3 | `WriteProfile` | Host → Device | Yes |
| `0x04` | 4 | `ReadProfile` | Host → Device | Yes |
| `0x05` | 5 | `ReadProfileAck` | Device → Host | Yes |
| `0x06` | 6 | `Ack` | Device → Host | Yes (implicit) |
| `0x07` | 7 | `SwitchProfile` | Host → Device | Yes |
| `0x08` | 8 | `WriteProfileToEEPRom` | Host → Device | No — decoder only |
| `0x09` | 9 | `ReadFirmwareVersion` | Host → Device | Yes |
| `0x0A` | 10 | `ReadFirmwareVersionAck` | Device → Host | Yes |
| `0x0B` | 11 | `ReadCurrentProfile` | Host → Device | Yes |
| `0x0C` | 12 | `ReadCurrentProfileAck` | Device → Host | Yes |
| `0x0D` | 13 | `ReadRGB` / `SetRGB` | Host → Device | Yes |
| `0x0E` | 14 | `ReadRGBAck` | Device → Host | No — explicitly skipped |
| `0x0F` | 15 | `ProfileChanged` | Device → Host (unsolicited) | Yes |
| `0x10` | 16 | `RefreshProfile` | Host → Device | Yes |
| `0x11` | 17 | `RefreshProfileAck` | Device → Host | Yes |
| `0x12` | 18 | `WriteEEPRom` | Host → Device | No — decoder only |
| `0x13` | 19 | `WriteEEPRomAck` | Device → Host | No — decoder only |
| `0x14` | 20 | `ReadEEPRom` | Host → Device | No — decoder only |
| `0x15` | 21 | `ReadEEPRomAck` | Device → Host | No — decoder only |
| `0x16` | 22 | `SetMacroStatus` | Host → Device | Yes |
| `0x17` | 23 | `QuickUpdate` | Host → Device | Yes (fire-and-forget) |
| `0x20` | 32 | `Vibration` | Host → Device | Yes |
| `0xF0` | 240 | `Download` | Host → Device | No — decoder only |
| `0xF1` | 241 | `DownloadAck` | Device → Host | No — decoder only |
| `0xF2` | 242 | `HeartBeat` | Host → Device | Yes |
| `0xF3` | 243 | `ReadKeyStatus` | Host → Device | No — decoder only |
| `0xF4` | 244 | `ReadKeyStatusAck` | Device → Host | No — decoder only |
| `0xFC` | 252 | `RequestToUpgrade` | Host → Device | No — decoder only |
| `0xFD` | 253 | `SetCalibrationState` | Host → Device | No — decoder only |

### Special Response Reclassifications

The app applies additional logic when parsing `ReadProfileAck` (0x05) and `Ack` (0x06):

- `ReadProfileAck` where `256*byte[3] + byte[4] + byte[5] == totalProfileLength` → treated as **`ReadProfileComplete`**
- `ReadProfileAck` where `byte[6]==0 && byte[7]==40 && byte[8]==5` → treated as **`ReadAudioAck`**
- `Ack` where `byte[2] == 1` → treated as **`AckWithBusy`** (device is busy, retry later)

---

## 4. Host → Device Messages

### 4.1 Enter Profile Config Mode
Defined in the command builder but **never sent** by the C2 app — `WriteProfile` and `ReadProfile` are issued directly without this preamble. The response decoder recognises the type if the device were to send it, but no handler acts on it.
```
[0] 0x0F
[1] 0x01
```

### 4.2 Exit Profile Config Mode
Likewise defined but **never sent** by the C2 app.
```
[0] 0x0F
[1] 0x02
[2] save_flag    (1 = save changes, 0 = discard)
```

### 4.3 Read Profile (paginated)
Read a chunk of profile data. Profile data is fetched in up to 58-byte chunks. Send repeatedly with increasing offsets until `ReadProfileComplete` is received.

Profile indices: `1`–`4` = normal profiles, `5` = Shift profile, `32` = Light/RGB profile.
```
[0] 0x0F
[1] 0x04
[2] profileIndex     (1–4, 5=Shift, 32=Light)
[3] offset_high      (byte offset >> 8)
[4] offset_low       (byte offset & 0xFF)
[5] length           (bytes to read, max 58)
```

Total reads for a full profile: 12 chunks for normal profiles (680 bytes), 11 chunks for light profile (635 bytes).

### 4.4 Write Profile (paginated)
Write a chunk of profile data. Only the changed byte range needs to be sent (the app diffs old vs. new bytes).
```
[0] 0x0F
[1] 0x03
[2] profileIndex
[3] offset_high
[4] offset_low
[5] length           (bytes being written, max 58)
[6..6+length-1] data
```

### 4.5 Switch Active Profile
```
[0] 0x0F
[1] 0x07
[2] profileIndex     (1–4 normal, 5 = Shift)
```

### 4.6 Read Firmware Version
```
[0] 0x0F
[1] 0x09
```

### 4.7 Read Current Active Profile Index
```
[0] 0x0F
[1] 0x0B
```

### 4.8 Read RGB/Lighting State
Sent by the host when the lighting editor UI opens (`reqSyncLED` IPC → `reqGetRgbState` event). The C2 does **not** use the `ReadRGBAck` (0x0E) response for this — it reads RGB state from the periodic gamepad input report instead (bytes 38–52, throttled to every 6th report).
```
[0] 0x0F
[1] 0x0D
```

### 4.9 Set RGB/Lighting Playback State
Uses the same command ID as Read RGB.
```
[0] 0x0F
[1] 0x0D
[2] isPlaying     (1 = playing animation, 0 = stopped)
[3] frameIndex    (starting frame, usually 0)
```

### 4.10 Refresh Profile Range (after `ProfileChanged` notification)
Re-read a specific range of a profile that the device reports has changed.
```
[0] 0x0F
[1] 0x10
[2] profileIndex
[3] offset_high
[4] offset_low
[5] length
```

### 4.11 Set Vibration Motors
```
[0] 0x0F
[1] 0x20
[2] 0x66         — magic marker
[3] 0x55         — magic marker
[4] left_motor   (0–255, 0 = off)
[5] right_motor  (0–255, 0 = off)
```

### 4.12 Set Macro Record Status
```
[0] 0x0F
[1] 0x16
[2] key           (1 = FL1, 2 = FR1)
[3] recordState
```

### 4.13 Quick Update (trigger firmware update)
Instructs device to enter firmware update mode.
```
[0] 0x0F
[1] 0x17
[2] 0x55
[3] 0x88
```

### 4.14 Heartbeat / Keep-Alive
Sent periodically to all open HID devices.
```
[0] 0x0F
[1] 0xF2
[2] testMode     (0 = normal, 1 = test/diagnostic mode)
```

### 4.15 Request DFU / Enter Upgrade Mode
Defined in the command builder but **not sent** by the C2 proxy. The response type `RequestToUpgrade` (0xFC) is recognised by the decoder but has no handler.
```
[0] 0x0F
[1] 0xFC
```

### 4.16 Set Calibration State
Defined in the command builder but **not sent** by the C2 proxy (the call site is in a different device proxy).
```
[0] 0x0F
[1] 0xFD
[2] calibration_state
```

### 4.17 Read Device Info (special ReadProfile sub-command)
Defined but **not sent** by the C2 proxy.
```
[0] 0x0F
[1] 0x04
[2] profileIndex
[3] 0x03
[4] 0x9D         (157 — fixed offset)
[5] 0x07         (7 bytes)
```

### 4.18 Set Device ID (special WriteProfile sub-command)
Defined but **not sent** by the C2 proxy.
```
[0] 0x0F
[1] 0x03
[2] profileIndex
[3] 0x03
[4] 0x9E         (158 — fixed offset)
[5] 0x06         (6 bytes)
[6..11] device_id (ASCII, 6 bytes)
```

---

## 5. Device → Host Messages

### 5.1 Read Profile Chunk (`ReadProfileAck`)
```
[0] 0x0F
[1] 0x05
[2] profileIndex
[3] offset_high
[4] offset_low
[5] length
[6..6+length-1] data
```
Collect chunks until offset + length == total profile size (`ReadProfileComplete`).

### 5.2 Generic Ack
```
[0] 0x0F
[1] 0x06
[2] busy_flag    (0 = success, 1 = busy → retry)
```

### 5.3 Firmware Version Response
```
[0] 0x0F
[1] 0x0A
[4..8]   controller firmware version (BCD-encoded ASCII, dot-separated)
[12..16] dongle firmware version (BCD-encoded ASCII, dot-separated)
```

### 5.4 Current Profile Response
```
[0] 0x0F
[1] 0x0C
[2] profileIndex    (1–4, 5=Shift; 0 is treated as 1)
```

### 5.5 Profile Changed Notification (unsolicited)
Sent by the device when settings change on-hardware (e.g., user presses profile button).
```
[0] 0x0F
[1] 0x0F
[2] profileIndex    (1–4, 5=Shift, 0x30=48 = active profile index changed)
[3] offset_high
[4] offset_low
[5] length_high
[6] length_low
```
- If `profileIndex == 0x30`, host re-queries the current profile index via `ReadCurrentProfile`.
- Otherwise, host sends `RefreshProfile` to re-read the changed byte range.

### 5.6 Refresh Profile Ack
```
[0] 0x0F
[1] 0x11
[2] profileIndex
[3] offset_high
[4] offset_low
[5] length
[6..6+length-1] refreshed data
```

### 5.7 Calibration Response
Not handled by the C2 proxy — the calibration response handler exists only in other device proxies (C3, T3CE).
```
[0] 0x0F
[1] ... (varies)
[5] calibration_mode
[6] status_flags:
      bit 0 = LT trigger calibrated
      bit 1 = RT trigger calibrated
      bit 2 = Left stick calibrated
      bit 3 = Right stick calibrated
      bit 4 = Motion sensor calibrated
```

---

## 6. Gamepad Input Report (Report ID `0x12`)

Received on the gamepad HID interface (Usage Page `0xFF00`). This is a 64-byte report.

### 6.1 Input State (bytes 0–9)

| Byte | Content |
|---|---|
| `[0]` | Report ID = `0x12` |
| `[1]` | Left stick X (0–255, 128 = center) |
| `[2]` | Left stick Y (0–255, 128 = center) |
| `[3]` | Right stick X (0–255, 128 = center) |
| `[4]` | Right stick Y (0–255, 128 = center) |
| `[5]` bits 3–0 | D-pad: `0`=Up, `1`=Up-Right, `2`=Right, `3`=Down-Right, `4`=Down, `5`=Down-Left, `6`=Left, `7`=Up-Left, `F`=Neutral |
| `[5]` bit 4 | Square / X |
| `[5]` bit 5 | Cross / A |
| `[5]` bit 6 | Circle / B |
| `[5]` bit 7 | Triangle / Y |
| `[6]` bit 0 | L1 |
| `[6]` bit 1 | R1 |
| `[6]` bit 2 | L2 (digital) |
| `[6]` bit 3 | R2 (digital) |
| `[6]` bit 4 | Select / Share / View |
| `[6]` bit 5 | Start / Options / Menu |
| `[6]` bit 6 | L3 |
| `[6]` bit 7 | R3 |
| `[7]` bit 0 | Home / PS / Guide |
| `[7]` bit 1 | Capture / Screenshot |
| `[7]` bit 3 | FL1 (back left paddle) |
| `[7]` bit 4 | FR1 (back right paddle) |
| `[7]` bit 5 | M (mode button) |
| `[8]` | Left trigger analog (0–255) |
| `[9]` | Right trigger analog (0–255) |

### 6.2 Device Status (bytes 35–53)

| Byte(s) | Content |
|---|---|
| `[35]` | Charge state (0 = discharging, non-zero = charging) |
| `[36]` | Battery level (0–100%) |
| `[37]` | Active profile index (0-based, i.e., 0–3) |
| `[38..40]` | Home LED: R, G, B |
| `[41..43]` | Lower-left LED: R, G, B |
| `[44..46]` | Lower-right LED: R, G, B |
| `[47..49]` | Upper-left LED: R, G, B |
| `[50..52]` | Upper-right LED: R, G, B |
| `[53]` bit 0 | Macro recording active |
| `[53]` bit 1 | Macro playback active |
| `[53]` bit 4 | FL1 is the active macro record key |
| `[53]` bit 5 | FR1 is the active macro record key |

---

## 7. Profile Data Structure (680 bytes)

Normal profiles (indices 1–4) and the Shift profile (index 5) are 680 bytes each.

| Offset | Length | Section |
|---|---|---|
| 0 | 32 | Profile name (UTF-8, null-padded; max 31 encoded bytes — see note below) |
| 32 | 32 | `Fun_Data` (miscellaneous settings — see §7.1) |
| 64 | 112 | Standard button mapping — 16 buttons × 7 bytes (see §7.2) |
| 176 | 159 | FL1 function key with macro (see §7.3) |
| 335 | 159 | FR1 function key with macro (see §7.3) |
| 494 | 28 | L2 trigger config (see §7.4) |
| 522 | 28 | R2 trigger config (see §7.4) |
| 550 | 32 | Left stick config (see §7.5) |
| 582 | 32 | Right stick config (see §7.5) |
| 614 | 33 | Aim/gyro sensor config (see §7.6) |
| 647 | 33 | Tilt/gyro sensor config (see §7.6) |

**Profile name encoding:** decoded on read as UTF-8 then trimmed of null bytes and whitespace. On write, the UTF-8-encoded name is copied into a zero-initialised 32-byte field, but the app truncates at `32 - 1 = 31` encoded bytes rather than 32. A name whose UTF-8 encoding is exactly 32 bytes will have its last byte silently dropped.

### 7.1 Fun_Data Block (32 bytes)

| Byte | Field name | Description |
|---|---|---|
| `[0]` | `motor_module.left_motor_value` | Left motor strength (0=0%, 1=25%, 2=50%, 3=75%, 4=100%) |
| `[1]` | `motor_module.right_motor_value` | Right motor strength (same scale) |
| `[2]` | `motor_module.lt_motor_value` | L2 trigger haptic motor strength |
| `[3]` | `motor_module.rt_motor_value` | R2 trigger haptic motor strength |
| `[4]` | `audio_module.profile_audio_en` | bit 0: audio enabled |
| `[5]` | `audio_module.audio_volume` | Audio volume |
| `[6]` | `audio_module.audio_mixer` | Audio mixer level |
| `[7]` | `audio_module.mic_mute` | Mic mute (1 = muted) |
| `[8]` | `audio_module.mic_sensitivity` | Mic sensitivity |
| `[9]` | `extend_module.shift_en` | bit 0: Shift mode enabled |
| `[10]` | `extend_module.shift_value` | Shift target profile index |
| `[11]` | `extend_module.dpad_diagonal_lock_en` | bit 0: D-pad diagonal lock enabled |
| `[12]` | `extend_module.xinput_abxy_change` | bit 0: Swap A/B for XInput mode |
| `[13]` | `extend_module.switch_abxy_change` | bit 0: Swap A/B for Switch mode |
| `[14]` | `extend_module.report_rates_gears` | Polling rate setting (gear index) |
| `[15..31]` | `reserved_module` | Reserved |

### 7.2 Standard Button Mapping (7 bytes per button, 16 buttons)

Button order: Up, Down, Left, Right, L1, R1, L3, R3, Cross/A, Circle/B, Square/X, Triangle/Y, Home, Select, Start, Capture

| Byte | Field name | Description |
|---|---|---|
| `[0]` | `turbo_module.turbo_en` | bit 0: turbo enabled |
| `[1]` | `turbo_module.turbo_speed` | Turbo speed (0 = off) |
| `[2]` | `map_module.map_en` | bit 0: remapping enabled |
| `[3]` | `map_module.map_value1` | Mapped key code 1 (see §9) |
| `[4]` | `map_module.map_value2` | Mapped key code 2 |
| `[5]` | `map_module.map_value3` | Mapped key code 3 |
| `[6]` | `toggle_en` | bit 0: toggle mode (press to hold/release) |

### 7.3 Function Key Packet — FL1 / FR1 (159 bytes)

| Byte | Field name | Description |
|---|---|---|
| `[0]` | `turbo_module.turbo_en` | bit 0: turbo enabled |
| `[1]` | `turbo_module.turbo_speed` | Turbo speed |
| `[2]` | `map_module.map_en` | bit 0: remapping enabled |
| `[3]` | `map_module.map_value1` | Mapped key code 1 |
| `[4]` | `map_module.map_value2` | Mapped key code 2 |
| `[5]` | `map_module.map_value3` | Mapped key code 3 |
| `[6]` | `toggle_en` | bit 0: toggle mode |
| `[7]` | `macro_fun.marco_open_status` | bit 0: `marco_en`; bit 1: `marco_key_open` (press-to-trigger); bit 2: `marco_cycle_en` (loop) |
| `[8]` | `macro_fun.marco_cycle_time_h` | Macro cycle interval high byte (ms, big-endian) |
| `[9]` | `macro_fun.marco_cycle_time_l` | Macro cycle interval low byte |
| `[10]` | `macro_fun.step_num` | Number of active macro steps (0–30) |
| `[11..155]` | `macro_fun.steps[0..28]` | Macro steps 0–28 (5 bytes each — see below) |
| `[156..158]` | `macro_fun.steps[29]` | Macro step 29 — key code (1 byte) + hold time (2 bytes) |

**Each macro step (5 bytes):**
| Byte | Field name | Description |
|---|---|---|
| `[0]` | `step_data` | Key code |
| `[1]` | `step_hold_time_h` | Hold duration high byte (ms, big-endian) |
| `[2]` | `step_hold_time_l` | Hold duration low byte |
| `[3]` | `step_delay_time_h` | Delay before next step high byte (ms, big-endian) |
| `[4]` | `step_delay_time_l` | Delay before next step low byte |

### 7.4 Trigger Config — L2 / R2 (28 bytes)

| Byte | Field name | Description |
|---|---|---|
| `[0]` | `turbo_module.turbo_en` | bit 0: turbo enabled |
| `[1]` | `turbo_module.turbo_speed` | Turbo speed |
| `[2]` | `dead_module.dead_en` | bit 0: deadzone enabled |
| `[3]` | `dead_module.front_dead` | Deadzone start % |
| `[4]` | `dead_module.back_dead` | Deadzone end % |
| `[5]` | `dead_module.anti_front_dead` | Anti-deadzone start % |
| `[6]` | `dead_module.anti_back_dead` | Anti-deadzone end % |
| `[7]` | `map_module.map_en` | bit 0: remapping enabled |
| `[8]` | `map_module.map_value1` | Mapped key code 1 |
| `[9]` | `map_module.map_value2` | Mapped key code 2 |
| `[10]` | `map_module.map_value3` | Mapped key code 3 |
| `[11]` | `toggle_en` | bit 0: toggle mode |
| `[12]` | `quick_trigger.quick_trigger_status` | `0`=off; `0x80\|mode` = enabled (bit 0=mode 1, bit 1=mode 2) |
| `[13]` | `quick_trigger.quick_trigger_start_value` | Hair trigger activation start % |
| `[14]` | `quick_trigger.quick_trigger_end_value` | Hair trigger activation end % |
| `[15]` | `linear_module.linear_module_en` | bit 0: response curve enabled |
| `[16]` | `linear_module.linear_status` | Curve type |
| `[17]` | `linear_module.linear_data` | Curvature value |
| `[18]` | `linear_module.original_data1` | Curve point 1 input |
| `[19]` | `linear_module.target_data1` | Curve point 1 output |
| `[20]` | `linear_module.original_data2` | Curve point 2 input |
| `[21]` | `linear_module.target_data2` | Curve point 2 output |
| `[22]` | `linear_module.original_data3` | Curve point 3 input |
| `[23]` | `linear_module.target_data3` | Curve point 3 output |
| `[24]` | `linear_module.original_data4` | Curve point 4 input |
| `[25]` | `linear_module.target_data4` | Curve point 4 output |
| `[26]` | `linear_module.original_data5` | Curve point 5 input |
| `[27]` | `linear_module.target_data5` | Curve point 5 output |

### 7.5 Stick Config — Left / Right (32 bytes)

| Byte | Field name | Description |
|---|---|---|
| `[0]` | `stick_en` | Always 1 (hardcoded on write) |
| `[1]` | `stick_square` | bit 0: square gate (raw mode, no circular normalization) |
| `[2]` | `dead_module.dead_en` | bit 0: deadzone enabled |
| `[3]` | `dead_module.front_dead` | Inner deadzone % |
| `[4]` | `dead_module.back_dead` | Outer deadzone % |
| `[5]` | `dead_module.anti_front_dead` | Anti-deadzone inner % |
| `[6]` | `dead_module.anti_back_dead` | Anti-deadzone outer % |
| `[7]` | `linear_module.linear_module_en` | bit 0: response curve enabled |
| `[8]` | `linear_module.linear_status` | Curve type |
| `[9]` | `linear_module.linear_data` | Curvature value |
| `[10]` | `linear_module.original_data1` | Curve point 1 input |
| `[11]` | `linear_module.target_data1` | Curve point 1 output |
| `[12]` | `linear_module.original_data2` | Curve point 2 input |
| `[13]` | `linear_module.target_data2` | Curve point 2 output |
| `[14]` | `linear_module.original_data3` | Curve point 3 input |
| `[15]` | `linear_module.target_data3` | Curve point 3 output |
| `[16]` | `linear_module.original_data4` | Curve point 4 input |
| `[17]` | `linear_module.target_data4` | Curve point 4 output |
| `[18]` | `linear_module.original_data5` | Curve point 5 input |
| `[19]` | `linear_module.target_data5` | Curve point 5 output |
| `[20]` | `map_module.map_en` | bit 0: advanced axis mapping enabled |
| `[21]` | `map_module.x_flip` | bit 0: reverse horizontal axis |
| `[22]` | `map_module.y_flip` | bit 0: reverse vertical axis |
| `[23]` | `map_module.axis_ratio` | Sensitivity ratio (0–100) |
| `[24]` | `map_module.mouse_dpi` | Mouse DPI (when mapped to mouse) |
| `[25]` | `map_module.map_index` | Output type: `1`=Left Stick, `2`=Right Stick, `3`=Wheel, `4`=Mouse |
| `[26]` | `map_module.map_cross` | Cross-axis coupling amount |
| `[27]` | `map_module.map_up_value` | Up key code (wheel/directional mapping) |
| `[28]` | `map_module.map_down_value` | Down key code |
| `[29]` | `map_module.map_left_value` | Left key code |
| `[30]` | `map_module.map_right_value` | Right key code |
| `[31]` | `map_module.map_dead_value` | Center/deadzone key code |

### 7.6 Motion Sensor Config — Aim / Tilt (33 bytes)

| Byte | Field name | Description |
|---|---|---|
| `[0]` | `sensor_profile_status` | Activation mode: `0`=off, `1`=always on, `2`=hold-to-activate |
| `[1]` | `sensor_quick_key_value` | Activation trigger key code |
| `[2]` | `active_axis` | bit 0=vertical (tilt X), bit 1=horizontal (tilt Y) |
| `[3]` | `dead_module.dead_en` | bit 0: deadzone enabled |
| `[4]` | `dead_module.front_dead` | Deadzone start % |
| `[5]` | `dead_module.back_dead` | Deadzone end % |
| `[6]` | `dead_module.anti_front_dead` | Anti-deadzone start % |
| `[7]` | `dead_module.anti_back_dead` | Anti-deadzone end % |
| `[8]` | `linear_module.linear_module_en` | bit 0: response curve enabled |
| `[9]` | `linear_module.linear_status` | Curve type |
| `[10]` | `linear_module.linear_data` | Curvature value |
| `[11]` | `linear_module.original_data1` | Curve point 1 input |
| `[12]` | `linear_module.target_data1` | Curve point 1 output |
| `[13]` | `linear_module.original_data2` | Curve point 2 input |
| `[14]` | `linear_module.target_data2` | Curve point 2 output |
| `[15]` | `linear_module.original_data3` | Curve point 3 input |
| `[16]` | `linear_module.target_data3` | Curve point 3 output |
| `[17]` | `linear_module.original_data4` | Curve point 4 input |
| `[18]` | `linear_module.target_data4` | Curve point 4 output |
| `[19]` | `linear_module.original_data5` | Curve point 5 input |
| `[20]` | `linear_module.target_data5` | Curve point 5 output |
| `[21]` | `map_module.map_en` | bit 0: axis mapping enabled |
| `[22]` | `map_module.x_flip` | bit 0: reverse horizontal |
| `[23]` | `map_module.y_flip` | bit 0: reverse vertical |
| `[24]` | `map_module.axis_ratio` | Sensitivity ratio (0–100) |
| `[25]` | `map_module.mouse_dpi` | Mouse DPI |
| `[26]` | `map_module.map_index` | Output type: `1`=LS, `2`=RS, `3`=Wheel, `4`=Mouse |
| `[27]` | `map_module.map_cross` | Cross-axis coupling |
| `[28]` | `map_module.map_up_value` | Up key code |
| `[29]` | `map_module.map_down_value` | Down key code |
| `[30]` | `map_module.map_left_value` | Left key code |
| `[31]` | `map_module.map_right_value` | Right key code |
| `[32]` | `map_module.map_dead_value` | Center key code |

---

## 8. Light Profile Data Structure (635 bytes)

Accessed via profile index `32` (`0x20`).

| Offset | Length | Field name | Description |
|---|---|---|---|
| 0 | 1 | `Light.ConfigIndex` | Active animation config index (0–3) |
| 1 | 620 | *(animations)* | 5 animation configs × 124 bytes (see §8.1) |
| 621 | 1 | `Light.AudioReactiveMode` | bit 0: audio-reactive mode enabled |
| 622 | 1 | `Light.UserEffectIndex` | Custom effect slot index |
| 623 | 3 | `Light.ProfileLed` | Profile indicator LED: R, G, B |
| 626 | 1 | `Light.RaiseWakeUp` | bit 0: wake on motion |
| 627 | 1 | `Light.StandbyTime` | Auto-sleep timeout |
| 628 | 7 | `Light.ReservedData` | Reserved |

### 8.1 Animation Config (124 bytes each)

| Offset | Length | Field name | Description |
|---|---|---|---|
| 0 | 1 | `KeyFrameCount` | Keyframe count |
| 1 | 1 | `EffectCount` | Effect count |
| 2 | 1 | `Speed` | Speed (stored as `20 - speed`, 0–20) |
| 3 | 1 | `Brightness` | Brightness (0–100) |
| 4 | 120 | `Frame0`..`Frame7` | 8 keyframes × 15 bytes |

**Each keyframe (15 bytes) — 5 RGB zones stored as `RGB0`..`RGB4` (hex strings internally):**

| Bytes | Field name | LED Zone |
|---|---|---|
| `[0..2]` | `RGB0` | Home button LED |
| `[3..5]` | `RGB1` | Lower-left LED |
| `[6..8]` | `RGB2` | Lower-right LED |
| `[9..11]` | `RGB3` | Upper-left LED |
| `[12..14]` | `RGB4` | Upper-right LED |

---

## 9. Key / Button Codes

Used in mapping fields (`map_value`, `step_data`, `map_up_value`, etc.).

### Gamepad Buttons (1–30)

| Code | Button |
|---|---|
| 0 | No mapping (pass-through) |
| 1 | D-pad Up |
| 2 | D-pad Down |
| 3 | D-pad Left |
| 4 | D-pad Right |
| 5 | L1 |
| 6 | R1 |
| 7 | L3 |
| 8 | R3 |
| 9 | Cross / A |
| 10 | Circle / B |
| 11 | Square / X |
| 12 | Triangle / Y |
| 13 | Home / PS / Guide |
| 14 | Select / Share / View |
| 15 | Start / Options / Menu |
| 16 | Capture / Screenshot |
| 17 | FL1 (back left paddle) |
| 18 | FR1 (back right paddle) |
| 19 | L2 |
| 20 | R2 |
| 21 | Left stick Up |
| 22 | Left stick Down |
| 23 | Left stick Left |
| 24 | Left stick Right |
| 25 | Right stick Up |
| 26 | Right stick Down |
| 27 | Right stick Left |
| 28 | Right stick Right |
| 29 | Left touchpad |
| 30 | Right touchpad |

### Keyboard Keys (50–155)

Starting at code 50: Esc, F1–F12, `` ` ``, 1–0, `-`, `=`, Backspace, Tab, Q–P, `[`, `]`, `\`, CapsLock, A–L, `;`, `'`, Enter, LShift, Z–M, `,`, `.`, `/`, RShift, LCtrl, LAlt, Space, RAlt, RCtrl, Left, Up, Down, Right, Ins, Del, Home, End, PgUp, PgDn, PrintScr, NumLk, Num0–9, Num`.`, Num`+`, Num`-`, Num`*`, Num`/`, NumEnter.

### Mouse Buttons (200–206)

| Code | Button |
|---|---|
| 200 | Left mouse button |
| 201 | Middle mouse button |
| 202 | Right mouse button |
| 203 | Mouse button 5 (forward) |
| 204 | Mouse button 4 (back) |
| 205 | Scroll wheel up |
| 206 | Scroll wheel down |

### Special Codes

| Code | Meaning |
|---|---|
| 230 | Mute |
| 231 | Shift (activate shift profile) |
| 255 | No-op / disabled |

---

## 10. Protocol Session Flow

### Startup / Device Connection

1. Enumerate HID devices matching VID `0x3537` and one of the C2 PIDs, with Usage Page `0xFFF0`.
2. Open the device; send **Heartbeat** (`[0x0F, 0xF2, 0x00]`).
3. Read all profile data:
   - For each of profiles 1–4, Shift (5), and Light (32):
     - Send repeated **ReadProfile** commands with increasing offsets.
     - Collect **ReadProfileAck** chunks until **ReadProfileComplete**.
4. Send **ReadFirmwareVersion** (`[0x0F, 0x09]`); await `ReadFirmwareVersionAck`.
5. Send **ReadCurrentProfile** (`[0x0F, 0x0B]`); await `ReadCurrentProfileAck`.
6. Continue sending Heartbeat periodically.

### Writing Profile Changes

1. Serialize the modified profile to its 680-byte binary representation.
2. Diff against the previously known bytes; identify the changed byte range.
3. Send paginated **WriteProfile** commands covering only the changed range (no Enter/Exit config wrapper is used).

### Handling On-Device Changes

When the device sends an unsolicited **ProfileChanged** notification:
- If `byte[2] == 0x30`: re-read the active profile index via **ReadCurrentProfile**.
- Otherwise: send **RefreshProfile** for the indicated byte range; process **RefreshProfileAck**.

---

## 11. Notes

- **Profile file encryption:** `.profile` files exported to disk are AES-256-CBC encrypted with the hardcoded key `"HJC2021"`. The HID protocol itself transfers raw binary.
- **Firmware update:** Uses a vendor DLL (`jl_firmware_upgrade_x86`) via FFI. The `QuickUpdate` command (`[0x0F, 0x17, 0x55, 0x88]`) triggers an in-app update flow; `RequestToUpgrade` (`[0x0F, 0xFC]`) enters DFU mode directly.
- **Wireless battery / RGB state** is reported inline in the gamepad input report (bytes 35–53), not via a dedicated config command.
