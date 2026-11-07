# Backed-up keymap editing (#24)

The CLI adds `keymap set <layer> <ROW,COLUMN> <keycode> --backup <NEW_FILE>`,
`keymap restore <BACKUP> --backup <NEW_FILE>` and `keymap reset --backup <NEW_FILE>`. They write only through the supported
direct-cable V5 Max ISO profile (`3434:0951`, 6×19 matrix, one encoder, layout options 0).
Receiver and unknown-model writes remain refused. This work was tested only through
synthetic replay sessions, not on Theo's keyboard or receiver.

Set, restore and reset **refuse `--record`**, regardless of its path, before opening a session.
Their backend sessions cannot record either. This prevents recording finalization from
overwriting a protected safety backup or restore source, including through path aliases.
Read-only `keymap show` keeps its existing recording behavior; no recording was performed
to verify this work.

`ROW,COLUMN` is zero-based, matching the matrix grid from `keymap show`. Single-key set
requires a physical key and a reported layer; it accepts raw 16-bit decimal or `0x` hex
keycodes. Restore preserves all matrix slots, including unused/nonzero slots, rather than
restoring only the 99 physical keys. No decoded key-name catalogue is implied.

## Backup and restore guarantees

Before any mutation, the CLI reads the full matrix and both clockwise/counter-clockwise
assignments of encoder 0 for every reported layer. Unsupported encoder reads prevent the
write: a matrix-only capture is not accepted as a complete profile keymap backup.

The required backup path must be **new**. The CLI serializes the complete snapshot,
creates the file without overwriting an existing path, writes all bytes, syncs the file
and reads it back, requiring it to equal the snapshot, before issuing a mutation.
Restore also saves a separate pre-restore snapshot, never
overwriting its input file. A backup creation/write/sync failure prevents keymap writes;
an incomplete newly created file may remain and is not claimed to be a usable backup.

Backup JSON schema version 1 requires every field and rejects unknown fields at every
level. It includes model IDs, the `v5-max-fr-iso` layout ID, layout options, matrix dimensions,
reported layer count, encoder count, every raw matrix `u16` and named CW/CCW pairs.
The parser rejects malformed/incomplete data, unsupported versions, inconsistent lengths,
out-of-range values and unsupported identities/layouts. Files are bounded to 1 MiB.
The source is parsed and validated before opening a device session; target layer count and
layout options must match before restoration starts. IDs identify a **model**, not a unique
keyboard unit. No claim of per-unit identity or arbitrary custom-firmware compatibility.

Every set/restore mutation reads actual state back, not just a setter echo. Restore
writes the matrix in 28-byte chunks with per-chunk verification, then each encoder direction
with individual verification. Set and restore both finish with another complete profile
snapshot comparison. Success output is delayed until session finalization also succeeds.

These are not atomic transactions. A timeout, mismatch, later write failure or finalization
failure may leave some changed state. The CLI emits no success output, says state **may have
changed**, and retains the pre-change backup path. It never claims an automatic rollback.
Backing up a file is not a guarantee against power failure or later storage loss.

Backups cover this profile's matrix and encoder assignments only. They are **not** backups
of macros, lighting, layout-option configuration, wireless pairing/mapping, firmware,
whole EEPROM or transient pressed-key state. This code does not change those settings.

## Reset

`keymap reset --backup <NEW_FILE>` sends VIA `dynamic_keymap_reset` (`0x06`). Before sending
it, the CLI captures the complete matrix/encoder snapshot, saves it to the new file, then
**reads the file back** and compares it with the snapshot. Any failure there refuses the
reset: nothing is sent. Undo is `keymap restore <NEW_FILE> --backup <ANOTHER_NEW_FILE>`.

After the reset, the CLI captures the keymap again to check that the keyboard still answers
with the same layer count and layout options. It cannot verify the new keycodes: the
firmware's built-in defaults are not known to openkeychron.

What `0x06` changes, at pinned Keychron QMK revision `2b493ff95f2665db719b1e95c91babb5036e72f3`:

- [via.c](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.c) handles `id_dynamic_keymap_reset` (`0x06`, [via.h](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.h)) by calling `dynamic_keymap_reset()` only, and echoes the request.
- [dynamic_keymap_reset](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/dynamic_keymap.c) rewrites every matrix slot and, with encoder maps enabled, both directions of every encoder, on every layer, from the flash keymap. Macros have their own `dynamic_keymap_macro_reset()`, which `0x06` does not call; the full EEPROM reset `eeconfig_init_via()` calls both, but `0x06` is not that path.
- The [EEPROM backend](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/nvm/eeprom/nvm_dynamic_keymap.c)'s keymap erase hook is a no-op.
- Keychron's [`via_command_kb`](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/keyboards/keychron/common/keychron_raw_hid.c) handles only its own command IDs and returns `false` for `0x06`, so the stock VIA handler runs.
- The [V5 Max info](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/keyboards/keychron/v5_max/info.json) defines one encoder and [ISO metadata](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/keyboards/keychron/v5_max/iso_encoder/keyboard.json) enables encoder maps, so the backup covers everything `0x06` rewrites.

This is source evidence for that revision, not proof about the firmware installed on Theo's
keyboard. #82 confirms reset then restore on the real V5 Max.

## Synthetic verification only

`fixtures/synthetic/cli-keymap-set.toml`, `cli-keymap-restore.toml` and `cli-keymap-reset.toml`
explicitly describe capture, writes, read-back and final verification. `keymap-backup.json` is chosen synthetic
data, **not** a firmware default or a backup of a real keyboard. Never use it as one.

```sh
cargo test -p openkeychron-via --locked --test write
cargo test -p openkeychron-cli --locked --test keymap_write
```

CLI integration tests use `--fixture` on every valid mutating invocation. They cover
malformed/incomplete backups, identity/layout mismatches, backup-file refusal, partial
matrix/encoder failure, reset rejected by the firmware and no partial-success output. They call the real
binary and require the replay to finish; no hardware transports are opened by those tests.
