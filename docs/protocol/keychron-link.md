# VIA traffic through the Keychron Link receiver

Research for #15: how a host reaches a 2.4 GHz V5 Max, and which commands can work through
the Keychron Link receiver instead of a USB cable.

**Short answer:** the path exists in Keychron's firmware design, but the public V5 Max
source compiles it out. Whether the firmware on Theo's keyboard has it is unknown until the
first hardware recording (#20). Until then, treat the **USB cable as the only supported path**
and the receiver as "to be confirmed".

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
host ──USB──▶ Keychron Link (3434:d030) ──2.4 GHz──▶ LKBT51 module ──UART──▶ STM32 (QMK)
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
does not. Only the hardware can settle it.

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

## How #20 settles it

With the keyboard on the receiver, and Chrome, Launcher and VIA closed, the CLI sends VIA
`get protocol version` (`01` followed by 31 zero bytes) to interface 1.3 and records the
session:

| Result | Meaning | What we do |
|---|---|---|
| A reply whose bytes 3–6 are `34 34 09 51` (VID `0x3434`, V5 Max ISO PID `0x0951`) | The shipped firmware has wireless raw HID, and the keyboard answered | Support the receiver; record more fixtures over it |
| A reply with other bytes 3–8, or zeros | The receiver or module answered for itself | Investigate before sending any write |
| A timeout | Wireless raw HID is off in the shipped firmware | Cable only; the CLI says so when it sees the receiver alone |

Then the same command over the USB cable, also recorded, gives the reference answer.
`get protocol version` only reads, so it is safe to send.

## Consequences for the code

- Discovery (#16) lists both the receiver's interface 1.3 and a cable-connected V5 Max
  (`3434:0951` for ISO, `3434:0950` for ANSI), and says which is which.
- Until #20 confirms the receiver, writes go only over the cable.
- A receiver session identifies the keyboard from the protocol-version reply (bytes 3–8), not
  from the receiver's USB IDs.
