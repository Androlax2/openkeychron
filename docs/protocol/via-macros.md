# VIA macros (#25)

The CLI adds `macro list`, `macro set <SLOT> [ACTION]... --backup <NEW_FILE>` and
`macro restore <BACKUP> --backup <NEW_FILE>`. Like keymap edits (#24), writes go only to the
V5 Max ISO on a USB cable (`3434:0951`); the receiver and unknown models are refused before
any report is sent, and `--record` is refused for both write commands. `macro list` only
reads and is not refused on the receiver, but whether the receiver forwards these commands
is unproven until #15. This work was tested only with synthetic
replay sessions, not on Theo's keyboard or receiver.

## What the commands touch

Source: Keychron QMK at pinned revision `2b493ff95f2665db719b1e95c91babb5036e72f3`.

| ID | `via.h` name | Effect in `via.c` |
|---|---|---|
| `0x0C` | `id_dynamic_keymap_macro_get_count` | Returns `DYNAMIC_KEYMAP_MACRO_COUNT` (default 16): slots the buffer *can* hold. Reads only. |
| `0x0D` | `id_dynamic_keymap_macro_get_buffer_size` | Returns the macro region size, big-endian. Reads only. |
| `0x0E` | `id_dynamic_keymap_macro_get_buffer` | `offset` (2 bytes, big-endian), `size` (≤ 28), then data. Reads only; bytes past the region read as 0. |
| `0x0F` | `id_dynamic_keymap_macro_set_buffer` | Same layout. Writes only bytes inside the macro region; bytes past it are dropped. |
| `0x10` | `id_dynamic_keymap_macro_reset` | Erases and zeroes the **whole** macro region. openkeychron never sends it. |

- [via.h](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.h)
  defines the IDs; [via.c](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.c)
  calls one `dynamic_keymap_macro_*` function per command and echoes the request.
- [dynamic_keymap.c](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/dynamic_keymap.c)
  forwards those to the NVM layer. The
  [EEPROM backend](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/nvm/eeprom/nvm_dynamic_keymap.c)
  places the macro region after the keymap and encoder maps
  (`DYNAMIC_KEYMAP_MACRO_EEPROM_ADDR`) up to the end of EEPROM, and every read or update
  checks `offset + i < DYNAMIC_KEYMAP_MACRO_EEPROM_SIZE`. So `0x0F` cannot reach the keymap,
  encoders, layout options or other settings.
- Keychron's [`via_command_kb`](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/keyboards/keychron/common/keychron_raw_hid.c)
  returns `false` for `0x0C`–`0x10`, so the stock handler above runs.
- The [V5 Max](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/keyboards/keychron/v5_max/info.json)
  uses 3072 bytes of wear-levelled EEPROM. Its `config.h`, `iso_encoder/config.h` and
  `keymaps/keychron/rules.mk` set no `DYNAMIC_KEYMAP_MACRO_COUNT`, so the default 16
  applies. The CLI still reads count and size from the keyboard rather than assuming them.

This is source evidence for that revision, not proof about the firmware on Theo's keyboard.

## Buffer layout

[`dynamic_keymap.h`](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/dynamic_keymap.h)
and `dynamic_keymap_macro_send`:

- The buffer holds NUL-terminated macros. Macro *n* starts after the *n*-th 0 byte; a slot
  the buffer ends before plays nothing. Bytes after the last counted slot are ignored.
- The **last byte is a write flag**. While it is non-zero the firmware plays no macro at all.
  A writer sets it to `0xFF`, writes the buffer, then sets it back to 0.

Inside a macro, [`send_string_with_delay_impl`](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/send_string/send_string.c)
reads, with codes from [`send_string_keycodes.h`](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/send_string/send_string_keycodes.h):

| Bytes | Action | CLI syntax |
|---|---|---|
| `01 01 KK` | tap 8-bit keycode `KK` | `tap:0x28` |
| `01 02 KK` | press | `down:0xe1` |
| `01 03 KK` | release | `up:0xe1` |
| `01 04 <digits> 7C` | wait `<digits>` ms; the firmware skips the byte after the digits, VIA writes `\|` | `delay:100` |
| any other byte `0x02`–`0x7F` | type that ASCII character | `text:hello` |

This firmware handles action codes 1–4 only, and keycodes are `uint8_t`. A keycode of 0 is refused because the firmware would read it as the
end of the macro. Text goes through `ascii_to_keycode_lut`, which is **US-ANSI**: the host's
layout decides what appears, so on a French AZERTY host `text:a` types `q`.

openkeychron only decodes what it would encode byte for byte (for example delays without
leading zeros, ending in `|`). Anything else is shown as *unreadable* with its raw bytes,
never dropped, and `macro set` keeps those bytes unchanged in the other slots.

## Writes

Before any write, the CLI reads the count, size and the whole buffer, refuses to continue if
the last byte is non-zero (an earlier write did not finish), and saves the snapshot to a
**new** backup file, synced and read back (`backup_file.rs`, shared with #24). Any failure
there stops before a write is sent.

`macro set` re-joins the slots with the new macro and pads with zeros, so bytes after the
last counted slot (which the firmware ignores) become 0. It refuses a macro that does not
fit before saving the backup. `macro restore` writes the backup's bytes exactly, after
checking that the keyboard reports the same count and size; a backup with a non-zero last
byte is refused.

Both write the whole buffer. `write_macro_buffer` first asks the keyboard for its buffer size
and refuses a buffer of any other length, so the flag always lands on the real last byte.
Then: flag `0xFF` at the last byte, 28-byte chunks, flag 0.
Every chunk is read back with `0x0E`, and a final full read must equal the expected buffer.
This is not atomic: a failure can leave the buffer half written with every macro disabled.
The CLI then prints no success, says state may have changed, and names the backup to
restore.

The backup covers the macro buffer only, not keymaps, lighting, settings or EEPROM.

## Synthetic verification only

`fixtures/synthetic/cli-macro-list.toml`, `cli-macro-set.toml` and `cli-macro-restore.toml`
describe a hand-written 4-slot, 40-byte buffer. `macro-backup.json` is that chosen data, not
a backup of a real keyboard.

```sh
cargo test -p openkeychron-via --locked macro
cargo test -p openkeychron-cli --locked --test macros
```
