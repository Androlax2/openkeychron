# Read-only dynamic keymap

Implemented by `openkeychron_via::Via::layer_count()` and `Via::keymap(key_count)` (#21).
These methods send only read requests. They do not remap keys, reset the keymap or change
settings. They use `HidTransport`, not device discovery or a model lookup.

## Wire format

Every request and reply is 32 bytes, excluding the HID report-ID prefix handled by the
transport. Request bytes not listed below are zero.

| Command | Request | Reply |
|---|---|---|
| `0x11`, get layer count | Command only | Byte 1: number of dynamic layers; zero is rejected |
| `0x12`, get keymap buffer | Bytes 1–2: big-endian **byte** offset; byte 3: size, at most 28 | Same header, followed by the requested data at byte 4 |

Keycodes are big-endian 16-bit values. The reader requests at most 14 whole keycodes per
exchange and exactly the remaining bytes in the final chunk. It validates echoed offset and
size before consuming payload; wrong command, wrong report length, unhandled command and
transport failures remain errors. No partially read keymap is returned on failure.

The caller must supply `key_count = layers × matrix rows × matrix columns`, including
unused matrix positions. Values are returned in layer/row/column order. The count must be
1–32768, covering the full 16-bit byte-offset space without wrapping. A larger count, or
zero, is rejected before any I/O.

**Important:** the firmware zero-fills requests beyond its actual matrix buffer. A valid
reply cannot prove that the caller's dimensions are correct, and the reader does not discard
zero-valued entries. Device definitions and FR ISO display mapping belong to #22. Encoder
maps are a separate protocol operation, not part of this buffer.

## Keycodes

`Keycode` retains the exact raw `u16`. Convert with `Keycode::from(raw)` and
`u16::from(keycode)`; every possible value round-trips unchanged.

`Keycode::kind()` provides a compact view of basic keys (including `KC_NO`, transparent
and standalone modifiers), modified keys, modifier-tap, layer-tap, layer-mod, standard
layer actions and one-shot modifiers. Modifier fields retain QMK's five-bit encoding:
bit 4 selects right-side rather than left-side modifiers. These are QMK bitfields, not
the USB report's eight-bit modifier mask.

Other families, unknown codes and Keychron custom values stay `Raw(u16)`. There is no
exhaustive key-name catalogue, locale-to-glyph conversion or lossy reconstruction from a
decoded view.

## Sources and verification

Source revision: Keychron QMK `2b493ff95f2665db719b1e95c91babb5036e72f3`:

- [Command IDs](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.h).
- [VIA request handlers](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/via.c).
- [Matrix order, endianness and zero-fill](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/nvm/eeprom/nvm_dynamic_keymap.c).
- [Keycode ranges](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/keycodes.h)
  and [bitfield definitions](https://github.com/Keychron/qmk_firmware/blob/2b493ff95f2665db719b1e95c91babb5036e72f3/quantum/quantum_keycodes.h).

`cargo test -p openkeychron-via keymap` exercises the public reader through replay fixtures,
including malformed replies and boundary offsets. `cargo test -p openkeychron-via keycode`
checks classification and exhaustive raw-value preservation. `fixtures/synthetic/via-keymap.toml`
is hand-written, not hardware evidence. No new real-device keymap capture has been made;
the existing real cable capture verifies metadata only. Receiver configuration stays
unsupported; the CLI is unchanged (#23 owns `keymap show`).
