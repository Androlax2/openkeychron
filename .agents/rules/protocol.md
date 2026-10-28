# Protocol rules (VIA, Keychron commands, fixtures)

Applies to `crates/openkeychron-via`, `crates/openkeychron-keychron`, the transports in
`crates/openkeychron-hid`, and `fixtures/`.

## Sources

- VIA command IDs and payloads come from QMK's `quantum/via.h` and `via.c`. Keychron's
  extra commands come from https://github.com/Keychron/qmk_firmware. Every constant links
  to the file it came from.
- Never guess a command ID or a payload layout. If the source does not settle it, open or
  comment on a `type:research` issue instead of writing code.
- Findings that are not obvious from the source go in `docs/protocol/`.

## Writes to the keyboard

The keyboard is Theo's daily keyboard. A bad write can make it unusable until reset.

- Every write has a read-back: write, read the value again, and fail loudly on a mismatch.
- Every write has an undo: before changing the keymap, macros or a setting, save the
  current value (to a backup file in the CLI, to memory in the app) and offer a restore.
- Never flash firmware, enter the bootloader or reset EEPROM without Theo's explicit yes
  for that exact action.
- Commands that only read are safe to run at any time; say in code which commands write.

## Reports

- VIA reports are 32 bytes. Fixture `data` is the report without the leading report ID
  byte some HID APIs need; the real transport adds and strips it.
- The Keychron Link receiver may forward reports differently from a cable connection.
  Until #15 settles it, do not assume a command that works over cable works through the
  receiver.

## Fixtures

- Format, privacy and review: `docs/FIXTURES.md`.
- Any change to the fixture schema bumps `FIXTURE_FORMAT_VERSION` and updates every
  committed fixture in the same PR.
- A test that replays a fixture calls `finish()`, so a session that stops early fails.
- Never edit a real recording by hand. Record it again, or add a synthetic fixture.
