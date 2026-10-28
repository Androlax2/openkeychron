# openkeychron

A native, local-first alternative to [Keychron Launcher](https://launcher.keychron.com):
remap keys, edit macros, and change lighting and Keychron settings from a desktop app, without
Chrome, an account or telemetry.

> **Status: early development. Nothing is usable yet.** The project is setting up its
> foundations (milestone M0). The first working command, which detects a keyboard and prints
> its firmware version, is milestone M1. Follow the
> [roadmap board](https://github.com/users/Androlax2/projects/2) for progress.

## Why

Keychron Launcher runs only in Chromium-based browsers, through WebHID, and on Linux it needs
udev tweaks before the browser can reach the keyboard. openkeychron talks to the keyboard
directly from a native app, the way [OpenLogi](https://github.com/AprilNEA/OpenLogi) does for
Logitech devices.

## Planned features

| Milestone | What you will be able to do |
|---|---|
| M1 Detect | See the keyboard, its firmware and its VIA protocol version from a CLI |
| M2 Keymap | Read the keymap and layers, remap a key and restore it, edit macros |
| M3 Keychron settings | RGB effects and per-key colours, debounce, last-key priority, wireless sleep timers |
| M4 Desktop UI | Do all of the above in a desktop app with a picture of your keyboard |

Every change to the keyboard is read back and can be undone. Firmware flashing is not planned
until it can be made safe (#40).

## Supported devices

| Device | Connection | Status |
|---|---|---|
| Keychron V5 Max, ISO (FR) | Keychron Link 2.4 GHz receiver | First target, in progress |

Other Keychron keyboards running QMK firmware come later (#41). Keychron mice are being
researched (#42).

## Platforms

Linux first (it is the developer's machine), then macOS and Windows (#43).

## Building from source

Requires Rust 1.99 or newer.

```sh
git clone https://github.com/Androlax2/openkeychron.git
cd openkeychron
cargo build --workspace
cargo test --workspace
```

The full handbook, including the CI gate (`make ci`), is
[`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). The plan lives in
[GitHub issues](https://github.com/Androlax2/openkeychron/issues) and the
[roadmap board](https://github.com/users/Androlax2/projects/2).

## License

Licensed under either of

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT license ([`LICENSE-MIT`](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion
in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions.

---

**Not affiliated with Keychron.** "Keychron" and "Keychron Launcher" are trademarks of their
respective owner.
