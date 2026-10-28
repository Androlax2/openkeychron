# Device fixtures

A fixture is one recorded HID session with a keyboard: which interface it came from and
every report that crossed it, in order. Tests replay fixtures below the protocol code, so
agents and CI can test VIA and Keychron commands without the keyboard.

## Format

Fixtures are TOML files under `fixtures/`, read and written by `openkeychron_hid::Fixture`.

```toml
format_version = 1

[device]
name = "Keychron Link"
vendor_id = 0x3434      # written back as decimal (13364) when the tool saves the file
product_id = 0xd030
usage_page = 0xff60     # VIA raw HID interface
usage = 0x61

[[event]]
kind = "write"          # a report the host sent
data = "01 00 00 ..."   # space-separated hex bytes

[[event]]
kind = "read"           # a report the device answered with
data = "01 00 0c ..."

[[event]]
kind = "read_timeout"   # the host waited and nothing arrived
```

- `data` is the report as the protocol sees it, without the leading report ID byte that
  some HID APIs need.
- Events replay in order. `ReplayTransport` fails on the first write whose bytes differ,
  on a read where a write was recorded, and, through `finish()`, on events the code never
  reached.
- `format_version` is checked first. Any change to the schema bumps
  `FIXTURE_FORMAT_VERSION`.

## Privacy

The schema has no field for a serial number or any other identifier of one unit, and
unknown keys are rejected, so a fixture cannot carry one by accident. Before committing a
recording, still check that no report payload contains personal data, such as text typed
into a macro.

## Recording a fixture

`RecordingTransport` wraps the real transport and keeps every report that crossed it;
`into_fixture(device)` turns the session into a `Fixture` and `Fixture::save` writes it.
The CLI's `--record <file>` option (issue #19) is the intended way to record. Recordings
need the real keyboard, so they are `needs-hardware` issues for Theo.

## Reviewing a fixture

1. The `[device]` table matches the interface you recorded (vendor, product, usage page).
2. The writes are the commands you meant to send, and nothing else.
3. No payload contains personal data.
4. The test that replays it passes: `make test`.

## Layout

- `fixtures/synthetic/`: hand-written fixtures that test the replay machinery itself.
- `fixtures/<model>/`: real recordings, for example `fixtures/v5-max/detect.toml`.
