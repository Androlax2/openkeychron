# Keychron's own raw HID commands (V5 Max)

Research for #27: the commands Keychron added next to VIA's, as compiled for the Keychron V5
Max. This is a reading of public source only. Nothing here was sent to a keyboard, and **no
command in this document has been checked on hardware yet**.

Source: Keychron's QMK fork, branch `2025q3`, commit `2b493ff9` (2026-10-23). Paths below are
relative to `keyboards/keychron/` at
https://github.com/Keychron/qmk_firmware/tree/2b493ff95f2665db719b1e95c91babb5036e72f3/keyboards/keychron

## What the V5 Max build includes

From `v5_max/rules.mk`, `v5_max/info.json` and `common/keychron_common.mk`:

| Feature | Compiled in | Why |
|---|---|---|
| Dynamic debounce | Yes | `v5_max/info.json`: `"debounce_type": "custom"` pulls in `common/debounce/debounce.mk` |
| Snap Click | Yes | `common/snap_click/snap_click.mk`, included for every Keychron board |
| Language | Yes | `common/language/language.mk`, always included |
| Keychron RGB (per-key, mixed) | Yes | `v5_max/rules.mk`: `KEYCHRON_RGB_ENABLE = yes` |
| Wireless sleep timers (LPM) | Yes | `WIRELESS_CONFIG_ENABLE` from `common/wireless/wireless.mk` |
| NKRO | Yes, adaptive | `APDAPTIVE_NKRO_ENABLE` in `common/keychron_common.mk` (Keychron's spelling) |
| DFU info, factory test | Yes | always in `common/keychron_common.mk` |
| USB report rate | **No** | `USB_REPORT_INTERVAL_ENABLE` is not set for the V5 Max |
| Analog matrix (Hall effect) | **No** | not a Hall-effect board |

V5 Max ISO: 6 × 19 matrix, 98 RGB LEDs (`v5_max/iso_encoder/config.h`), USB `3434:0951`,
device version 1.1.0 (`v5_max/iso_encoder/keyboard.json`).

## Framing

Every command is a 32-byte raw HID report on the same interface as VIA, answered by one 32-byte
reply (`common/keychron_raw_hid.c`, reached through `via_command_kb`).

- Byte 0 is the command. Byte 0 of the reply is the same command, or `0xFF` if the firmware does
  not handle it.
- In the grouped commands (`0xA7`, `0xA8`), byte 1 is the sub-command and **byte 2 of the reply
  is the status**: `0` success, `1` failure. Reply data starts at byte 3.
- Multi-byte numbers are little-endian (copied with `memcpy` on a little-endian MCU), unlike
  VIA's big-endian values.

## Top-level commands

Defined in `common/keychron_raw_hid.h`, handled in `common/keychron_raw_hid.c`.

| ID | Name | Request | Reply | Changes the keyboard |
|---|---|---|---|---|
| `0xA0` | Get protocol version | `A0` | `[1]` = `0x02`, `[2]` = 0, `[3]` = `QMK_COMMAND_SET` (2) | No |
| `0xA1` | Get firmware version | `A1` | ASCII from byte 1: `v` + version from `DEVICE_VER` (e.g. `v1.1.0`), a space, then the build date, NUL-terminated | No |
| `0xA2` | Get supported features | `A2` | `[1]` = 0, `[2]` = feature bits 0–7, `[3]` = bits 8–15 (table below) | No |
| `0xA3` | Get default layer | `A3` | `[1]` = default layer, `[2]` = highest active layer | No |
| `0xA7` | Misc group | `A7 <sub>` | see below | Depends on sub-command |
| `0xA8` | Keychron RGB group | `A8 <sub>` | see below | Depends on sub-command |
| `0xAA` | Wireless-module firmware update | — | — | **Yes. Never send.** Cable only |
| `0xAB` | Factory test | — | — | **Yes. Never send** |

`0xA1` is the "real" firmware version that `openkeychron info` does not show yet (it shows
VIA's `VIA_FIRMWARE_VERSION`).

Feature bits from `0xA2`: bit 0 default layer, 1 Bluetooth, 2 2.4 GHz, 3 analog matrix,
4 state notify, 5 dynamic debounce, 6 Snap Click, 7 Keychron RGB, 8 quick start, 9 NKRO.
The V5 Max sets bits 0, 1, 2, 4, 5, 6, 7, 8 and 9.

## Misc group (`0xA7`)

Sub-commands from the `MISC_*` enum in `common/keychron_raw_hid.h`.

| Sub | Name | Request after `A7 <sub>` | Reply after status byte | Stored | Source |
|---|---|---|---|---|---|
| `0x01` | Get misc protocol | — | `[3..4]` = `0x0002` (LE), `[5]` = misc feature bits | — | `common/keychron_raw_hid.c` |
| `0x02` | Get DFU info | — | `[3]` chip tag, `[4]` length, MCU name, then type tag, `1`, bootloader type | — | `common/dfu_info.c` |
| `0x03` | Get language | — | `[3]` = language byte | — | `common/language/language.c` |
| `0x04` | Set language | `[2]` = language byte | status | **EEPROM at once** | `common/language/language.c` |
| `0x05` | Get debounce | — | `[3]` = 0, `[4]` = algorithm, `[5]` = time in ms | — | `common/debounce/keychron_debounce.c` |
| `0x06` | Set debounce | `[2]` = algorithm (0–6), `[3]` = time in ms | status; algorithm ≥ 7 fails | **EEPROM at once** | `common/debounce/keychron_debounce.c` |
| `0x07` | Snap Click info | — | `[3]` = number of pairs (20) | — | `common/snap_click/snap_click.c` |
| `0x08` | Get Snap Click pairs | `[2]` = start, `[3]` = count (≤ 9) | 3 bytes per pair from `[3]`: mode, key 1, key 2 | — | `common/snap_click/snap_click.c` |
| `0x09` | Set Snap Click pairs | `[2]` = start, `[3]` = count (≤ 9), then 3 bytes per pair | status | RAM only, until `0x0A` | `common/snap_click/snap_click.c` |
| `0x0A` | Save Snap Click | — | status | **EEPROM** (all 20 pairs) | `common/snap_click/snap_click.c` |
| `0x0B` | Get wireless sleep | — | `[3..4]` backlight-off seconds, `[5..6]` idle-sleep seconds (LE u16) | — | `common/wireless/wireless.c` |
| `0x0C` | Set wireless sleep | `[2..3]` backlight-off s, `[4..5]` idle s (LE u16) | status; backlight < 5 or idle < 60 fails and reloads the stored values | **EEPROM at once**, plus side effects below | `common/wireless/wireless.c` |
| `0x0D`, `0x0E` | Report rate | — | `0xFF` (not compiled for the V5 Max) | — | `common/usb_report_rate.c` |
| `0x0F`–`0x11` | DIP switch, factory reset | — | `0xFF`: in the enum, no handler | — | `common/keychron_raw_hid.c` |
| `0x12` | Get NKRO | — | `[3]` bit 0 = NKRO on, bit 2 = adaptive NKRO | — | `common/nkro.c` |
| `0x13` | Set NKRO | `[2]` = 0 or 1 | **always fails on the V5 Max**: adaptive NKRO cannot be set | — | `common/nkro.c` |

Debounce algorithms (`common/debounce/keychron_debounce.h`): 0 symmetric defer global, 1 symmetric
defer per row, 2 symmetric defer per key, 3 symmetric eager per row, 4 symmetric eager per key,
5 asymmetric eager/defer per key (time capped at 127 ms), 6 none. A time of 0 selects none.
`v5_max/info.json` sets the default time to 50 ms.

Snap Click modes (`common/snap_click/snap_click.c`): 0 off, 1 regular, 2 last input, 3 first key wins,
4 second key wins, 5 neutral (both released). Keys are 8-bit, so only basic keycodes fit.
Keychron's changelog calls a 1.1.0 feature "LKP"; **this is most likely Snap Click's "last
input" mode, an inference not confirmed by any source**.

**Side effects of "set wireless sleep" (`0x0C`)**: while the keyboard is connected wirelessly,
the firmware resets its backlight timer, then **sends input to the computer** to reset the
wireless module's timer: a mouse move left and right (`MOUSEKEY_ENABLE` is on for the V5 Max),
then re-initialises the module (`lkbt51_param_init`). Defaults: backlight off after 600 s
(`CONNECTED_BACKLIGHT_DISABLE_TIMEOUT`, `common/wireless/indicator.h`; the V5 Max does not
override it), idle sleep after 7200 s (`common/wireless/wireless_config.h`).

## Keychron RGB group (`0xA8`)

Sub-commands from the enum in `common/rgb/keychron_rgb.c`. The V5 Max has
`EFFECT_LAYERS` = 2 and `EFFECTS_PER_LAYER` = 5 (`common/rgb/rgb_matrix_kb_config.h`).

| Sub | Name | Request after `A8 <sub>` | Reply after status byte | Stored |
|---|---|---|---|---|
| `0x01` | Get RGB protocol | — | `[3..4]` = `0x0001` (LE) | — |
| `0x02` | Save | — | status | **EEPROM**: indicators, per-key type and colours, mixed regions and effects, all at once |
| `0x03` | Get indicator config | — | `[3]` supported lock LEDs, `[4]` disabled mask, `[5..7]` H, S, V | — |
| `0x04` | Set indicator config | `[2]` disabled mask, `[3..5]` H, S, V (V raised to ≥ 128) | status | RAM until `0x02`; applied at once |
| `0x05` | Get LED count | — | `[3]` = LED count (98 on the V5 Max ISO) | — |
| `0x06` | Get LED index | `[2]` = row, `[3..5]` = column bit mask | `[3 + col]` = LED index or `0xFF` | — |
| `0x07` | Get per-key type | — | `[3]` = type | — |
| `0x08` | Set per-key type | `[2]` = 0 solid, 1 breathing, 2–4 reactive | status; ≥ 5 fails | RAM until `0x02` |
| `0x09` | Get per-key colours | `[2]` = start, `[3]` = count (≤ 9) | H, S, V per LED from `[3]` | — |
| `0x0A` | Set per-key colours | `[2]` = start, `[3]` = count (≤ 9), then H, S, V per LED | status | RAM until `0x02` |
| `0x0B` | Mixed-effect info | — | `[3]` = layers, `[4]` = effects per layer | — |
| `0x0C` | Get regions | `[2]` = start, `[3]` = count (≤ 29) | one region byte per LED from `[3]` | — |
| `0x0D` | Set regions | `[2]` = start, `[3]` = count (≤ 28), then one byte per LED (< layers) | status | RAM until `0x02` |
| `0x0E` | Get effect list | `[2]` = region, `[3]` = start, `[4]` = count (≤ 3) | 8 bytes per effect: effect, H, S, speed, time (LE u32) | — |
| `0x0F` | Set effect list | `[2]` = region, `[3]` = start, `[4]` = count (≤ 3), then 8 bytes per effect | status | RAM until `0x02` |

The plain RGB effect, brightness, speed and colour are **not** Keychron commands: they are VIA's
lighting values (`id_custom_get_value` / `id_custom_set_value` on channel
`id_qmk_rgb_matrix_channel`, `quantum/via.h` at the repository root). That matters for #28.

## Firmware bugs a host must work around

These handlers trust the host for bounds. Our code must check them before sending, or a
request can make the firmware read or **write outside its arrays**:

- **Per-key colours (`A8 09`, `A8 0A`)**: only `count ≤ 9` is checked, never
  `start + count ≤ LED count`. Always read the LED count (`A8 05`) first and stay inside it.
- **Effect list (`A8 0E`, `A8 0F`)**: the check is `region > EFFECT_LAYERS`, so region 2 on
  the V5 Max passes and indexes past the 2 layers. Keep `region < 2`.
- **LED index (`A8 06`)**: the check is `row > MATRIX_ROWS`, so row 6 passes on a 6-row board.
  Keep `row < 6`.

## Stored at once, stored on save, or not stored

| Behaviour | Commands |
|---|---|
| Written to EEPROM by the set command itself | language `A7 04`, debounce `A7 06`, wireless sleep `A7 0C` |
| Changed in RAM; written only by a separate save | Snap Click `A7 09` → `A7 0A`; all RGB sets `A8 04`, `08`, `0A`, `0D`, `0F` → `A8 02` |
| Refused on the V5 Max | NKRO `A7 13`, report rate `A7 0D`/`0E`, DIP switch and factory reset `A7 0F`–`11` |

The V5 Max's EEPROM is emulated in flash with wear levelling (`v5_max/info.json`). Commands that
write at once should not be sent on every step of a slider; send once when the value settles.

## When each feature arrived

The source does not record which firmware version added a command. Keychron's firmware page
for the V5 Max says version 1.1.0 (March 17, 2025) "Added debounce, LKP, Per-key RGB/Mixed RGB,
as well as backlight off and Bluetooth sleep time adjustment in wireless mode"
(https://www.keychron.com/pages/firmware-and-json-files-of-the-keychron-qmk-v-and-v-max-series-keyboards).
Older firmware may lack any of them, so **the host should decide from the feature bits
(`A2`, and `A7 01`'s misc bits), not from the version string**.

## Open questions, for hardware sessions

- Whether the firmware on Theo's keyboard matches this source build (`A1` will show its
  version and build date).
- What the language byte means: the firmware stores it without interpreting it.
- Whether "LKP" is Snap Click's "last input" mode.
- Whether any of this works through the Keychron Link receiver (#20); until then, cable only,
  and no receiver writes until a write is verified by reading it back.
