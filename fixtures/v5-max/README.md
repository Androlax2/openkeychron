# V5 Max ISO recordings

`detect-cable.toml` is a real, unmodified capture from the V5 Max ISO (`3434:0951`),
recorded over its USB cable with the Keychron Link receiver also connected. The CLI selected
the keyboard explicitly with `--device`; the recorded session contains six read-only queries:
VIA protocol version, VIA firmware version, layout options, uptime, Keychron protocol and
Keychron firmware version.

Reported values: VIA protocol 12, VIA firmware version 0, layout options 0, Keychron protocol 2
(command set 2), firmware `v1.1.0`, build date `2025-03-17-17:29:02`.

Reviewed for privacy: no serial number, key matrix, keymap or macro data. The only changing
session value is uptime. The recording confirms cable transport and the protocol/version
reply layouts; it does not verify settings writes, RGB persistence or receiver forwarding.

Replay without hardware:

```sh
cargo run -p openkeychron-cli -- info --json --fixture fixtures/v5-max/detect-cable.toml
```

The CLI integration test replays this file and checks the reported identity and versions.
Earlier receiver-only attempts timed out; receiver support remains unverified (#20).
