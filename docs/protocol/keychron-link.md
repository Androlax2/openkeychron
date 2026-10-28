# VIA traffic through the Keychron Link receiver

Research for #15: how a host reaches a 2.4 GHz V5 Max, and which commands can work through
the Keychron Link receiver instead of a USB cable.

**Short answer:** receiver-local metadata reads work, but keyboard VIA queries do not.
The official V5 Max ISO v1.1.0 image matching Theo's reported build timestamp lacks the
wireless raw-HID receive path (see [firmware evidence](#matching-official-keyboard-image)).
This is a concrete blocker in that image, not a byte-for-byte identification of the installed
firmware or a claim that the hardware can never support configuration. Treat the **USB cable
as the only supported keyboard-configuration path**; receiver writes remain blocked.

Sources, read on 2026-10-28:

- Keychron QMK fork, branch `2025q3` (default), commit `2b493ff9`:
  https://github.com/Keychron/qmk_firmware/tree/2b493ff95f2665db719b1e95c91babb5036e72f3
- Keychron's wireless-module firmware page:
  https://www.keychron.com/pages/keychron-v-max-q-max-k-max-series-bluetooth-firmware
- Keychron's help centre: https://help-center.keychron.com/en-us/article/22776472488855
- The receiver's HID report descriptors, read from sysfs on Theo's machine (read-only, nothing
  was sent to the device).

## The two links

```
host ──USB──▶ Keychron Link (3434:d030) ──2.4 GHz──▶ LKBT51 module ──SPI──▶ STM32 (QMK)
host ──USB cable──────────────────────────────────────────────────────────▶ STM32 (QMK)
```

The V5 Max has an LKBT51 wireless module next to its STM32 microcontroller
(`keyboards/keychron/v5_max/config.h`: `LKBT51_RESET_PIN`, `LK_WIRELESS_ENABLE` through
`common/wireless/wireless.mk`). QMK runs on the STM32; the module and the receiver run
Keychron's closed firmware.

## What the receiver exposes

USB device `3434:d030` ("Keychron Link"), four HID interfaces:

| Interface | Descriptor | Purpose (inferred) |
|---|---|---|
| 1.0 | Mouse, report ID 3 | Mouse input, for Keychron mice on the same receiver |
| 1.1 | Usage page `0x8C`. Input `0xB1` and output `0xB2`, 32 bytes each; feature `0x51` (20 B), `0x52` (64 B), `0x53` (1000 B); input `0x54` (20 B); a 6-axis, 16-button gamepad collection (report 4) | The receiver's own channel (pairing, updates), plus gamepad mode. Not VIA |
| 1.2 | Keyboard (1), consumer keys (2), system control (6), NKRO bitmap (`0x0C`) | Typing |
| 1.3 | Usage page `0xFF60`, usage `0x61`, 32 bytes in and out, **no report ID** | Same shape as QMK's raw HID interface: this is where VIA traffic would go |

`openkeychron-hid` (#16) should open interface 1.3, matched by usage page and usage, never by
interface number.

## The path inside the keyboard firmware

1. The module delivers raw HID from the receiver as `LKBT51_CMD_RAW_HID_RX` (`0x92`), and
   `lkbt51.c` turns it into an `EVT_RAW_HID` event.
2. `wireless.c` hands that event to the same `via_raw_hid_receive` that USB uses, tagged
   `RAW_HID_SRC_WIRELESS`. **This case is inside `#if defined(RAW_ENABLE) &&
   defined(WILRESS_RAW_ENABLE)`** (Keychron's spelling).
3. Replies go back through `kc_raw_hid_send`: over USB for `RAW_HID_SRC_USB`, otherwise
   through the module (`LKBT51_CMD_RAW_HID_TX`, `0x93`) when the wireless link is connected.

`WILRESS_RAW_ENABLE` is defined nowhere in the `2025q3` checkout: not in the V5 Max files, not
in `common/`, and not in any of the 101 keyboard folders' `config.h`, `post_config.h` or
`rules.mk`. The `wireless_playground` and `wls_2025q1` branches have no raw HID case in
`wireless.c` at all. **So a V5 Max built from public source drops VIA commands that arrive
over 2.4 GHz.**

Keychron's module firmware page says otherwise for shipped builds, quoted exactly:

> Firmware version 0.2.0 updated on July 9, 2024 … 2. Supports VIA or Launcher drivers in 2.4G
> wireless mode. … Note: Items 1 and 2 are only available if both the keyboard QMK and
> Bluetooth firmware support them.

Keychron's help centre says the opposite for the Launcher: "Ensure your keyboard is in a wired
connection. (The launcher won't detect your keyboard in Bluetooth or a 2.4G connection.)" The
Launcher's own page says: "Connect your device using a cable or 2.4G receiver (some models only
support wired mode)."

The release build on the keyboard may set `WILRESS_RAW_ENABLE` even though the public source
does not. Only a reply from the keyboard itself can show it.

## Commands: cable vs receiver

Even with wireless raw HID on, the firmware treats a few commands differently by source
(`keyboards/keychron/common/keychron_raw_hid.c`):

| Command | Over cable | Over the receiver (only if wireless raw HID is compiled in) |
|---|---|---|
| VIA `id_get_protocol_version` (`0x01`) | Answers the VIA protocol version (`0x000C`) | Same, **plus** the keyboard's own VID, PID and device version in bytes 3–8, so a host behind the receiver can tell which keyboard answered |
| All other VIA commands (keymap, layers, macros, lighting) | Yes | Same code path |
| Keychron `0xA0`–`0xA3` (protocol, firmware version, features, default layer) | Yes | Yes |
| Keychron `0xA7` misc group (debounce, Snap Click, NKRO, wireless sleep, report rate, language) | Yes | Yes |
| Keychron `0xA8` RGB | Yes | Yes (the source flag is passed but not used) |
| Keychron `0xAA` wireless-module firmware update | Yes | **No**: cable only, by design |
| Keychron `0xAB` factory test | Yes | Partly: radio test commands are refused |

Flashing the keyboard itself (STM32 DFU) is cable-only in every case and out of scope until
#40.

## How #20 tests it

The first request in both sessions is VIA `get protocol version` (`01` followed by 31 zero
bytes), which reads and changes nothing. On success, `info` continues with its other metadata
queries. Chrome, Launcher and VIA are closed, and each session is recorded.

1. **Cable reference first.** With the keyboard on the USB cable, send the command to its raw
   HID interface. A valid reply (`01 00 0c …`) proves the command and our transport work. If
   this step fails, stop: no receiver result can be interpreted until it passes.
2. **Then the receiver.** With the keyboard on 2.4 GHz, send the same command to the
   receiver's interface 1.3.

| Receiver result | What it tells us | What we do |
|---|---|---|
| A reply whose bytes 3–6 are `34 34 09 51` (VID `0x3434`, V5 Max ISO PID `0x0951`) | The reply carries the V5 Max ISO's identity, which the public source adds only on the keyboard (`keychron_raw_hid.c`, non-USB source). Strong evidence that reads reach the keyboard's firmware | Receiver reads can be supported. Receiver writes stay off until a separate write and read-back test passes |
| A reply with other bytes 3–8, or zeros | Something replied, but its identity is unverified: the bytes do not show what produced them | Treat the receiver as unsupported. No writes. Investigate |
| A timeout | Inconclusive. Wireless raw HID may be compiled out of the shipped firmware, or the request or reply may have been lost to the radio link, the receiver, the module, or our transport. With the cable reference passing, it shows only that the receiver path did not work in this session | Treat the receiver as unsupported. No writes. Repeat once the link is known to be up before concluding anything about the firmware |

In every case, **nothing is written through the receiver until a receiver write has been
verified by reading it back.**

## Consequences for the code

- Discovery (#16) lists both the receiver's interface 1.3 and a cable-connected V5 Max
  (`3434:0951` for ISO, `3434:0950` for ANSI), and says which is which.
- Writes go only over the cable until a receiver write has been verified by reading it back;
  a successful read through the receiver is not enough.
- A receiver session identifies the keyboard from the protocol-version reply (bytes 3–8), not
  from the receiver's USB IDs.

## Receiver-local reads, verified on 2026-10-29

The receiver has its own commands on the same `0xFF60/0x61` interface, distinct from
the `0xB1`/`0xB2` **report IDs on its other interface**. Launcher sends these commands as
32-byte payloads with report ID zero, followed by the ordinary keyboard VIA query:

| Payload byte 0 (remaining request bytes zero) | Launcher operation | Observed result |
|---|---|---|
| `0xB1` | `FR_GET_PROTOCOL_VERSION` | `b1 01 00 00 00 …`: receiver protocol 1, feature bytes zero |
| `0xB2` | `FR_GET_STATE` | `b2 01 34 34 51 09 01 …`: VID `0x3434`, PID `0x0951`, connected flag 1 |
| `0xB3` | `FR_GET_FW_VERSION` | `d.3.0 Jun  7 2024 12:32:18` |
| `0x01` | Keyboard VIA protocol version | No reply within five seconds |

The state reply's VID and PID are little-endian at bytes 2–3 and 4–5; connected is byte 6.
It identifies the receiver's connected device, **not** a successful round trip to the
keyboard's VIA handler. It does not authorize receiver writes.

Source: [Launcher bundle](https://launcher.keychron.com/main.9e74017f20f1eca7.js), modules
`88493` (command constants) and `9789` (receiver client and initialization order).
Bundle SHA256: `0e53d4db82a87bae42599c16661de233fc12a88d7e2de938fd268f609cb7ea84`.
The receiver client's initialization reads protocol, state and firmware, then keyboard VIA
version and JSON version. There is no forwarding-enable write in that sequence. Its gamepad
control and DFU commands are separate operations and were not sent.

Approved host request `d21ce7b6c0ca` sent the four requests above to `/dev/hidraw12`, after
checking sysfs for receiver `3434:d030` and the VIA descriptor. A standard-library Python
probe bypassed our Rust HID transport; two hardware-free tests checked its exact request
bytes and timeout recording. The raw capture is retained locally in
`.handoff/receiver-probe-result.jsonl`; no serial, keypress, keymap or macro data is present.

This confirms USB access and receiver-local metadata work. It also shows that the three
initialization reads alone do not make the keyboard answer VIA. It does **not** establish
whether the request reached the wireless module or QMK, nor rule out pairing/module state.

Keychron's [receiver release notes](https://www.keychron.com/pages/how-to-flash-the-firmware-for-the-keychron-receiver)
explicitly list wireless VIA/Launcher support for `d.3.0/c.3.0`, conditional on keyboard QMK
support. The official Type-A download is named
`keychron_link_type_d_fw_d.3.0_2406071232.kfw`, matching the receiver's version and build minute.
Thus a June build date is not evidence that this receiver predates the feature. The same
notes require re-pairing after an update; no re-pairing or update was performed here.

## Matching official keyboard image

The cable reference reports keyboard firmware `v1.1.0`, built `2025-03-17-17:29:02`.
Keychron's [V/V Max firmware page](https://www.keychron.com/pages/firmware-and-json-files-of-the-keychron-qmk-v-and-v-max-series-keyboards)
links this [official V5 Max ISO image](https://cdn.shopify.com/s/files/1/0059/0630/1017/files/v5_max_iso_encoder_v1.1.0_2503171731.bin?v=1743389929):

- Filename: `v5_max_iso_encoder_v1.1.0_2503171731.bin`.
- Size: 105188 bytes.
- SHA256: `6678bb353f9c67fa9594fc57d51163e9cb5e96d5171e9781bf5f98fcf3ae42d0`.
- Embedded build string at file offset `0x180CB`: `2025-03-17-17:29:02`, exactly matching
  the cable reply. The filename's minute is not the embedded build time.

The image was downloaded for **static inspection only**, never flashed or executed.
LLVM's binary-to-ELF conversion and Thumb disassembly used existing tools:

```sh
llvm-objcopy -I binary -O elf32-littlearm --binary-architecture=arm firmware.bin firmware.elf
llvm-objdump --triple=thumbv7em-none-eabi --mcpu=cortex-m4 --disassemble-all firmware.elf
```

Here `firmware.bin` names the downloaded image, not a device. Addresses below are file
offsets; runtime flash addresses add `0x08000000`. Independent read-only review confirmed:

1. `0x854C` enqueues three-byte wireless events into a 16-slot ring at RAM `0x20002BAF`;
   head/tail are `0x20002BDF`/`0x20002BE0`. `0x8940` consumes the same ring.
2. At `0x8AEC`, the dispatcher subtracts one from the event ID; `0x8AF2` compares against
   ten and `0x8AF4` branches on unsigned higher. The eleven-entry table at
   `0x8B00`–`0x8B28` therefore handles only events 1–11. The newer source's raw-HID event
   is 12; it has no handler here.
3. Task `0x8B58` invokes the active transport callback, then dispatcher `0x8940`.
   Startup's initialized-data mapping identifies that callback as module parser `0x9550`.
4. Crucially, the module parser's validated-packet dispatch at `0x973E`–`0x9784` handles
   `0xA1`, `0xA2`, `0xA3`, `0xB0` and `0xB4`. Raw-HID RX `0x92` falls through to the
   return at `0x9764`: there is no raw-data copy, enqueue or direct VIA call on that path.

**Conclusion for this official image:** the expected LKBT51 wireless raw-HID-to-VIA bridge
is absent. Host-side initialization cannot supply that missing keyboard-firmware behavior.
Receiver metadata can work while keyboard configuration remains unavailable.

Do not describe this image as definitely "compiled without `WILRESS_RAW_ENABLE`": its
three-byte events differ from the pointer-bearing event structure in the newer pinned public
source. It may predate those additions instead. The conclusion rests on the actual parser
and dispatcher, not an assumed build flag.

**Limits:** the installed keyboard was not read back or hashed. An exact reported version
and build-time match is strong evidence, not proof of identical bytes. The traced ingress
does not implement wireless VIA; this is not an exhaustive proof against every undocumented
protocol. The keyboard's wireless-module version and pairing state are also unverified.
No firmware change, reset, re-pairing or setting write is authorized by these findings.
Receiver configuration is therefore blocked pending a separately reviewed firmware/support
investigation, not declared physically impossible.
