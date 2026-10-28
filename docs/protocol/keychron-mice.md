# Keychron mice (G-series, M-series): protocol and fit

Research for #42: what protocol Keychron mice use (including Keychron's ZGM / Zephyr firmware)
and whether it fits openkeychron. Read-only, from public sources. Nothing was downloaded and
run, no device was touched, and **Theo does not own a Keychron mouse**, so nothing here was
checked on hardware.

Sources, read on 2026-10-28:

- `Keychron/zgm` at commit `e77d7383` (2026-04-13): https://github.com/Keychron/zgm
- Keychron's GitHub organisation repository list (`Keychron/*`)
- `csutcliff/keychron-battery-dkms` at commit `7cd5149b` (README says GPL-2.0-only; read for
  facts only, no code copied): https://github.com/csutcliff/keychron-battery-dkms
- libratbag's device list (`libratbag/libratbag`, `data/devices/`, branch `master`)
- foosel.net, "How to fix the Chromium WebHID connection for a Keychron G5 under Linux"
  (2026-04-28)
- `Keychron/zmk`, branch `main`, commit `ead91ca0`; the Keychron Nape Pro product page and the
  aftermath.site review of it
- Northeast Times, "Keychron bets open-source mouse firmware can unseat Logitech"
  (2026-07-30): press, not a primary source

## ZGM is not usable yet

`Keychron/zgm` contains 10 files: a project website (`index.html`, `script.js`, `styles.css`,
`CNAME` = `zgm.gg`), `README.md`, policy files and the GPL-3.0 licence. **There is no firmware
source, no protocol and no board support.** Its README, quoted exactly:

> This repository is currently in its early setup phase.
>
> At the moment, the repository mainly contains project scaffolding, policy files, and the
> initial project direction. Firmware sources, board support, build instructions, and flashing
> guidance are still being prepared.

The press article says Keychron aims to launch ZGM in Q1 2027, with the G6 HE as the first
target. That is a secondary source; Keychron's repository gives no date.

So **no G-series or M-series mouse runs published open firmware**: their firmware is closed.

One exception, outside #42's G-series scope: the **Keychron Nape Pro**, a trackball, runs ZMK and
is configurable with ZMK Studio or Keychron Launcher (keychron.com product page; aftermath.site
review). Its board source was not found on `Keychron/zmk`'s `main` branch (commit `ead91ca0`,
0 of 3,514 paths mention it); other branches were not checked. ZMK Studio speaks its own
configuration protocol, not VIA.

## What the shipped mice speak

Nothing from Keychron documents it. The only public protocol facts found come from the
community battery driver:

- The mouse is configured through a **vendor HID interface, interface 4**, not through a
  VIA-style raw HID channel (`keychron_battery.c`: `KEYCHRON_VENDOR_INTERFACE 4`).
- A command is sent as a 64-byte **feature report with report ID `0xB3`** over the control
  endpoint (`HID_REQ_SET_REPORT`). Command `0x06` asks for status.
- The reply comes back on the **interrupt endpoint as report ID `0xB4`**, echoing the command in
  byte 1; the battery level is byte 20 (0–100).
- Devices it binds to: Keychron M5 wired (`3434:d048`) and the Ultra-Link 8K receiver
  (`3434:d028`). The foosel.net post adds the G5 wired (`3434:d06f`), configured through the same
  Ultra-Link 8K receiver.
- Keychron Launcher configures the mice over WebHID (foosel.net post).

libratbag (Piper) has **no Keychron device file** among its 126.

## Compared with what openkeychron does today

| | Keychron QMK keyboards (today) | Keychron mice |
|---|---|---|
| Firmware | QMK, public source | Closed (ZGM not published) |
| Channel | Usage page `0xFF60`, usage `0x61`, 32-byte reports, no report ID | Vendor interface 4, 64-byte reports, report IDs `0xB3` / `0xB4` |
| Request path | Output report | **Feature report** (control endpoint) |
| Protocol source | QMK and Keychron source | One community driver (battery only); the rest would be reverse engineering |
| Receiver | Keychron Link `3434:d030` | Ultra-Link 8K `3434:d028` |

Our code would need, at least:

- discovery by vendor and interface rather than by the VIA usage page (#16 filters on
  `0xFF60`);
- a `HidTransport` able to send feature reports, which it cannot today;
- a separate protocol crate, since nothing in `openkeychron-via` or `openkeychron-keychron`
  applies.

## Recommendation

**Do not support Keychron mice now.** There is no published protocol beyond battery level, no
open firmware to read, no mouse to test with, and a different transport from everything built
so far. Revisit when either:

- ZGM publishes firmware source with its configuration protocol, or
- someone with a Keychron mouse wants to contribute recordings (fixtures) and is willing to
  study the protocol.

Getting further today would mean reverse engineering Keychron Launcher's web app or USB
traffic. That raises its own questions (Keychron's terms of use; the proprietary Launcher
code) that are Theo's to decide, not this research's.

## Uncertain

- Whether all G-series and M-series mice share the `0xB3` / `0xB4` protocol: the battery driver
  only claims the M5 and the Ultra-Link 8K ("other Keychron mice using the same protocol may
  also work").
- What other commands exist besides status `0x06`, and their payloads.
- Whether the Keychron Link (`3434:d030`) receiver's own channel (report IDs `0xB1` / `0xB2`,
  `docs/protocol/keychron-link.md`) belongs to the same family. The numbering is suggestive, not
  evidence.
- Whether ZGM, once published, will be configured through VIA, ZMK Studio, or a protocol of its
  own.
- The ZGM launch date and first device (press only).
- Where the Nape Pro's ZMK source lives, and whether supporting ZMK Studio's protocol would be
  worth a separate issue.
